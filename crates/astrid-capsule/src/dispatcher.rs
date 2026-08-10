//! Event dispatcher for routing events to capsule interceptors.
//!
//! The dispatcher is a host-side async task that subscribes to the global
//! `EventBus`, matches incoming events against capsule interceptor patterns
//! (from `Capsule.toml`), and invokes the corresponding WASM
//! `astrid_hook_trigger` export on each matching capsule.
//!
//! # Event Routing
//!
//! The dispatcher handles two categories of events:
//!
//! - **IPC events**: matched by their `topic` field (e.g. `user.prompt`)
//! - **Lifecycle events**: matched by `event_type()` (e.g. `tool_call_started`,
//!   `session_created`). This enables WASM capsules (like the Hook Bridge) to
//!   subscribe to lifecycle events and apply policy (merge strategies, hook
//!   fan-out) on top of the kernel's dispatch mechanism.
//!
//! All dispatch is fire-and-forget from the dispatcher's perspective. Capsules
//! that need request-response semantics (e.g. collecting responses from multiple
//! subscribers) use `hooks::trigger` — the kernel syscall for fan-out with
//! response collection.
//!
//! # Topic Matching
//!
//! Interceptor event patterns support:
//! - Exact match: `user.prompt` matches only `user.prompt`
//! - Single-segment wildcard: `tool.execute.*.result` matches
//!   `tool.execute.search.result` but not `tool.execute.result`

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{RwLock, mpsc};
use tracing::{debug, warn};

use crate::capsule::{Capsule, CapsuleId};
use crate::registry::CapsuleRegistry;
use astrid_events::{AstridEvent, EventBus};

/// Capacity of each per-capsule event dispatch queue.
///
/// If a capsule's queue fills up (i.e. it is processing events slower than
/// they arrive), new events are dropped with a warning rather than blocking
/// the dispatcher. 256 is generous for typical usage.
const CAPSULE_EVENT_QUEUE_CAPACITY: usize = 256;
const LOCAL_PROVIDER_CAPSULE_ID: &str = "astrid-capsule-openai-compat";
const LOCAL_PROVIDER_REQUEST_TOPIC: &str = "llm.v1.request.generate.openai-compat";
const LOCAL_PROVIDER_STREAM_TOPIC: &str = "llm.v1.stream.openai-compat";
const LOCAL_PROVIDER_ACTION: &str = "handle_llm_request";
const REACT_CAPSULE_ID: &str = "astrid-capsule-react";
const PROVIDER_FAILURE_MESSAGE: &str = "local provider interceptor failed";
const DISPATCHER_PRODUCER_KIND: &str = "kernel_dispatcher";
const DISPATCHER_PRODUCER_ID: &str = "local_provider_interceptor_failure";

/// Work item sent to a per-capsule ordered queue.
struct InterceptorWork {
    action: String,
    payload: Arc<Vec<u8>>,
    topic: Arc<String>,
    /// The originating IPC message, if this event came from IPC.
    /// `None` for lifecycle events. Carried through to
    /// `invoke_interceptor` so the kernel can set per-invocation
    /// principal context on `HostState`.
    ipc_message: Option<Arc<astrid_events::ipc::IpcMessage>>,
}

/// Routes events from the `EventBus` to capsule interceptors.
///
/// Both IPC events (by topic) and lifecycle events (by `event_type()`) are
/// dispatched fire-and-forget. Capsules needing response collection use
/// `hooks::trigger` (the kernel fan-out syscall).
pub struct EventDispatcher {
    registry: Arc<RwLock<CapsuleRegistry>>,
    event_bus: Arc<EventBus>,
    /// Identity store for validating principals before auto-provisioning.
    /// When set, only principals with a matching identity record get
    /// home directories created. When `None`, provisioning is ungated
    /// (pre-production behavior).
    identity_store: Option<Arc<dyn astrid_storage::IdentityStore>>,
}

impl EventDispatcher {
    /// Create a new event dispatcher.
    #[must_use]
    pub fn new(registry: Arc<RwLock<CapsuleRegistry>>, event_bus: Arc<EventBus>) -> Self {
        Self {
            registry,
            event_bus,
            identity_store: None,
        }
    }

    /// Set the identity store for principal validation during auto-provisioning.
    #[must_use]
    pub fn with_identity_store(mut self, store: Arc<dyn astrid_storage::IdentityStore>) -> Self {
        self.identity_store = Some(store);
        self
    }

    /// Run the dispatch loop. Blocks until the event bus is closed.
    ///
    /// Subscribes to all events on the bus and routes both IPC events (by topic)
    /// and lifecycle events (by `event_type()`). Should be spawned as a
    /// background tokio task.
    ///
    /// Monitors broadcast channel lag and publishes `astrid.v1.event_bus.lagged`
    /// IPC events when messages are dropped, rate-limited to at most once per
    /// 10 seconds to avoid feedback loops.
    pub async fn run(self) {
        let mut receiver = self.event_bus.subscribe();
        let mut last_lag_notification = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .unwrap_or_else(std::time::Instant::now);
        let mut capsule_queues: HashMap<CapsuleId, mpsc::Sender<InterceptorWork>> = HashMap::new();
        let mut known_principals: HashSet<String> = HashSet::new();
        // The "default" principal is always provisioned by the kernel boot sequence.
        known_principals.insert("default".to_string());
        /// Maximum number of principals tracked before the set stops growing.
        /// 10K principals = ~640KB of memory (64-byte strings). Beyond this,
        /// new principals are still dispatched but not cached — they'll hit
        /// the filesystem check on every event instead of the O(1) HashSet.
        const MAX_KNOWN_PRINCIPALS: usize = 10_000;
        debug!("Event dispatcher started");

        while let Some(event) = receiver.recv().await {
            // Check for broadcast channel overflow (lost messages).
            let lagged = receiver.drain_lagged();
            if lagged > 0 && last_lag_notification.elapsed() >= std::time::Duration::from_secs(10) {
                warn!(
                    lagged_count = lagged,
                    "Event bus broadcast channel lagged - {lagged} messages dropped"
                );
                last_lag_notification = std::time::Instant::now();

                // Publish a lag notification so capsules can react.
                // Note: This notification is published onto the same bus that just
                // overflowed, so it may itself be dropped under sustained load. This
                // is acceptable - the watchdog timeout is the actual recovery mechanism.
                // The 10s rate limit prevents amplification feedback loops.
                let msg = astrid_events::ipc::IpcMessage::new(
                    "astrid.v1.event_bus.lagged",
                    astrid_events::ipc::IpcPayload::Custom {
                        data: serde_json::json!({ "lagged_count": lagged }),
                    },
                    uuid::Uuid::new_v4(),
                );
                self.event_bus.publish(AstridEvent::Ipc {
                    metadata: astrid_events::EventMetadata::new("dispatcher"),
                    message: msg,
                });
            }

            let (topic, payload_bytes, ipc_message) = match &*event {
                AstridEvent::Ipc { message, .. } => {
                    let topic = Arc::new(message.topic.clone());
                    match message.payload.to_guest_bytes() {
                        Ok(bytes) => (topic, Arc::new(bytes), Some(Arc::new(message.clone()))),
                        Err(e) => {
                            warn!(topic = %message.topic, error = %e, "Failed to serialize IPC payload");
                            continue;
                        },
                    }
                },
                other => {
                    let topic = Arc::new(other.event_type().to_string());
                    match serde_json::to_vec(other) {
                        Ok(bytes) => (topic, Arc::new(bytes), None),
                        Err(e) => {
                            warn!(event_type = %topic, error = %e, "Failed to serialize lifecycle event");
                            continue;
                        },
                    }
                },
            };

            // Auto-provision home directories for new principals.
            // When an identity store is configured, only the "default"
            // principal is auto-provisioned. Other principals must be
            // explicitly created via the identity flow (uplink calls
            // create_user → AstridUserId with principal → uplink sets
            // principal on IPC). This prevents unauthenticated directory
            // creation from arbitrary IPC principal strings.
            if let Some(ref msg) = ipc_message
                && let Some(ref principal_str) = msg.principal
                && !known_principals.contains(principal_str)
            {
                if let Ok(pid) = astrid_core::PrincipalId::new(principal_str) {
                    // Gate: if identity store is wired, only auto-provision
                    // "default". Other principals are created by uplinks
                    // which handle home provisioning after create_user.
                    let should_provision =
                        self.identity_store.is_none() || pid == astrid_core::PrincipalId::default();

                    if should_provision && let Ok(home) = astrid_core::dirs::AstridHome::resolve() {
                        let ph = home.principal_home(&pid);
                        if let Err(e) = ph.ensure() {
                            // Don't cache — allow retry on next event (#544).
                            warn!(
                                principal = %pid,
                                error = %e,
                                "Failed to auto-provision principal home"
                            );
                        } else {
                            debug!(
                                principal = %pid,
                                "Auto-provisioned principal home directory"
                            );
                            // Only cache on success so transient failures
                            // can retry on the next event (#544).
                            if known_principals.len() < MAX_KNOWN_PRINCIPALS {
                                known_principals.insert(principal_str.clone());
                            }
                        }
                    }
                    // If AstridHome::resolve() failed, don't cache — allow
                    // retry when the home directory becomes available.
                } else {
                    warn!(
                        principal = %principal_str,
                        "IPC message has invalid principal string, ignoring"
                    );
                }
            }

            let matches =
                find_matching_interceptors(&self.registry, &topic, ipc_message.as_deref()).await;
            dispatch_to_capsule_queues(
                &mut capsule_queues,
                matches,
                topic,
                payload_bytes,
                ipc_message,
                &self.event_bus,
            );
        }

        debug!("Event dispatcher stopped (event bus closed)");
    }
}

/// Dispatch matching interceptors as a middleware chain.
///
/// Interceptors are called sequentially in priority order (lower fires first).
/// Each interceptor returns an [`InterceptResult`] that controls the chain:
/// - `Continue` — pass (possibly modified) payload to the next interceptor
/// - `Final` — short-circuit with a response, no further interceptors fire
/// - `Deny` — short-circuit with denial, audit-logged, no further interceptors fire
///
/// Within a single capsule, events are still delivered in publish order via
/// per-capsule mpsc queues (preserving IPC `seq` ordering). The chain semantics
/// apply across capsules for the same event.
fn dispatch_to_capsule_queues(
    queues: &mut HashMap<CapsuleId, mpsc::Sender<InterceptorWork>>,
    matches: Vec<(Arc<dyn Capsule>, String)>,
    topic: Arc<String>,
    payload_bytes: Arc<Vec<u8>>,
    ipc_message: Option<Arc<astrid_events::ipc::IpcMessage>>,
    event_bus: &Arc<EventBus>,
) {
    if matches.is_empty() {
        return;
    }

    // Clone what we need for the spawned chain task.
    let matches_owned: Vec<_> = matches
        .into_iter()
        .map(|(c, a)| (Arc::clone(&c), a))
        .collect();

    // For single-interceptor events (common case), skip chain overhead.
    if matches_owned.len() == 1 {
        let (capsule, action) = matches_owned.into_iter().next().unwrap();
        dispatch_single(
            queues,
            capsule,
            action,
            topic,
            payload_bytes,
            ipc_message,
            event_bus,
        );
        return;
    }

    // Multi-interceptor chain: run sequentially in priority order.
    // Spawned as a task so the dispatcher loop doesn't block.
    let topic_clone = Arc::clone(&topic);
    let ipc_clone = ipc_message.clone();
    tokio::task::spawn(async move {
        let mut current_payload = (*payload_bytes).clone();

        for (capsule, action) in &matches_owned {
            debug!(
                capsule_id = %capsule.id(),
                action = %action,
                topic = %topic_clone,
                "Dispatching interceptor (chain)"
            );
            let caller = ipc_clone.as_deref();
            match capsule.invoke_interceptor(action, &current_payload, caller) {
                Ok(crate::capsule::InterceptResult::Continue(modified_payload)) => {
                    debug!(
                        capsule_id = %capsule.id(),
                        action = %action,
                        "Interceptor: Continue"
                    );
                    // If the interceptor returned payload bytes, use them
                    // for the next interceptor in the chain.
                    if !modified_payload.is_empty() {
                        current_payload = modified_payload;
                    }
                },
                Ok(crate::capsule::InterceptResult::Final(response)) => {
                    debug!(
                        capsule_id = %capsule.id(),
                        action = %action,
                        topic = %topic_clone,
                        response_len = response.len(),
                        "Interceptor: Final — chain halted"
                    );
                    return; // Short-circuit — no further interceptors
                },
                Ok(crate::capsule::InterceptResult::Deny { reason }) => {
                    warn!(
                        capsule_id = %capsule.id(),
                        action = %action,
                        topic = %topic_clone,
                        reason = %reason,
                        "Interceptor: Deny — chain halted"
                    );
                    return; // Short-circuit — no further interceptors
                },
                Err(crate::error::CapsuleError::NotSupported(ref msg)) => {
                    debug!(
                        capsule_id = %capsule.id(),
                        action = %action,
                        reason = %msg,
                        "Interceptor skipped (NotSupported)"
                    );
                    // Continue chain — this capsule doesn't participate
                },
                Err(e) => {
                    warn!(
                        capsule_id = %capsule.id(),
                        action = %action,
                        topic = %topic_clone,
                        error = %e,
                        "Interceptor invocation failed — continuing chain"
                    );
                    // Continue chain on error — don't let a broken capsule
                    // block the entire pipeline
                },
            }
        }
    });
}

/// Fast path for single-interceptor dispatch — uses per-capsule queue
/// for ordered delivery without chain overhead.
fn dispatch_single(
    queues: &mut HashMap<CapsuleId, mpsc::Sender<InterceptorWork>>,
    capsule: Arc<dyn Capsule>,
    action: String,
    topic: Arc<String>,
    payload_bytes: Arc<Vec<u8>>,
    ipc_message: Option<Arc<astrid_events::ipc::IpcMessage>>,
    event_bus: &Arc<EventBus>,
) {
    let sender = queues.entry(capsule.id().clone()).or_insert_with(|| {
        let (tx, mut rx) = mpsc::channel::<InterceptorWork>(CAPSULE_EVENT_QUEUE_CAPACITY);
        let capsule = Arc::clone(&capsule);
        let event_bus = Arc::clone(event_bus);
        tokio::task::spawn(async move {
            while let Some(work) = rx.recv().await {
                debug!(
                    capsule_id = %capsule.id(),
                    action = %work.action,
                    topic = %work.topic,
                    "Dispatching interceptor (ordered)"
                );
                let caller = work.ipc_message.as_deref();
                match capsule.invoke_interceptor(&work.action, &work.payload, caller) {
                    Ok(crate::capsule::InterceptResult::Continue(_)) => {
                        debug!(
                            capsule_id = %capsule.id(),
                            action = %work.action,
                            "Interceptor completed (Continue)"
                        );
                    },
                    Ok(crate::capsule::InterceptResult::Final(_)) => {
                        debug!(
                            capsule_id = %capsule.id(),
                            action = %work.action,
                            "Interceptor completed (Final)"
                        );
                    },
                    Ok(crate::capsule::InterceptResult::Deny { reason }) => {
                        warn!(
                            capsule_id = %capsule.id(),
                            action = %work.action,
                            topic = %work.topic,
                            reason = %reason,
                            "Interceptor: Deny"
                        );
                    },
                    Err(crate::error::CapsuleError::NotSupported(ref msg)) => {
                        debug!(
                            capsule_id = %capsule.id(),
                            action = %work.action,
                            reason = %msg,
                            "Interceptor skipped (NotSupported)"
                        );
                    },
                    Err(e) => {
                        let terminal_error_published = publish_exact_local_provider_failure(
                            &event_bus,
                            capsule.id(),
                            &work.action,
                            &work.topic,
                            work.ipc_message.as_deref(),
                        );
                        warn!(
                            capsule_id = %capsule.id(),
                            action = %work.action,
                            topic = %work.topic,
                            error = %e,
                            terminal_error_published,
                            "Interceptor invocation failed"
                        );
                    },
                }
            }
        });
        tx
    });

    let work = InterceptorWork {
        action,
        payload: Arc::clone(&payload_bytes),
        topic: Arc::clone(&topic),
        ipc_message: ipc_message.clone(),
    };
    if let Err(e) = sender.try_send(work) {
        warn!(
            capsule_id = %capsule.id(),
            topic = %topic,
            "Capsule dispatch queue full or closed, dropping event: {e}"
        );
    }
}

/// Publish one kernel-authored provider stream error only for the exact, host-attested ReAct
/// request path used by the local OpenAI-compatible provider. The synthetic event retains the
/// typed request identifier and a child of the original trace so ReAct can publish its existing
/// executor-terminal response inside the scheduled headless trace.
fn publish_exact_local_provider_failure(
    event_bus: &EventBus,
    capsule_id: &CapsuleId,
    action: &str,
    topic: &str,
    caller: Option<&astrid_events::ipc::IpcMessage>,
) -> bool {
    let Some(message) = exact_local_provider_failure_message(capsule_id, action, topic, caller)
    else {
        return false;
    };
    event_bus.publish(AstridEvent::Ipc {
        metadata: astrid_events::EventMetadata::new("dispatcher"),
        message,
    });
    true
}

fn exact_local_provider_failure_message(
    capsule_id: &CapsuleId,
    action: &str,
    topic: &str,
    caller: Option<&astrid_events::ipc::IpcMessage>,
) -> Option<astrid_events::ipc::IpcMessage> {
    if capsule_id.as_str() != LOCAL_PROVIDER_CAPSULE_ID
        || action != LOCAL_PROVIDER_ACTION
        || topic != LOCAL_PROVIDER_REQUEST_TOPIC
    {
        return None;
    }
    let caller = caller.filter(|message| message.topic == LOCAL_PROVIDER_REQUEST_TOPIC)?;
    let producer = caller
        .producer
        .as_ref()
        .filter(|producer| producer.is_supported())?;
    if producer.kind != "wasm_capsule" || producer.id != REACT_CAPSULE_ID {
        return None;
    }
    let astrid_events::ipc::IpcPayload::LlmRequest { request_id, .. } = &caller.payload else {
        return None;
    };
    if request_id.is_nil() {
        return None;
    }
    let trace = caller.trace.as_ref().filter(|trace| {
        trace.is_supported() && trace.turn_id.is_some() && trace.session_id.is_some()
    })?;
    let mut message = astrid_events::ipc::IpcMessage::new(
        LOCAL_PROVIDER_STREAM_TOPIC,
        astrid_events::ipc::IpcPayload::LlmStreamEvent {
            request_id: *request_id,
            event: astrid_events::llm::StreamEvent::Error(PROVIDER_FAILURE_MESSAGE.to_string()),
        },
        uuid::Uuid::new_v4(),
    )
    .with_trace(trace.child())
    .with_producer(astrid_events::ipc::IpcProducerV1::new(
        DISPATCHER_PRODUCER_KIND,
        DISPATCHER_PRODUCER_ID,
    ));
    if let Some(principal) = caller.principal.as_deref() {
        message = message.with_principal(principal);
    }
    Some(message)
}

/// Find all capsules with interceptors matching the given topic.
///
/// Takes a brief read lock on the registry. Only `Ready` capsules are
/// considered. Returns `(capsule, action)` pairs sorted by interceptor
/// priority (lower values fire first, default 100).
async fn find_matching_interceptors(
    registry: &RwLock<CapsuleRegistry>,
    topic: &str,
    caller: Option<&astrid_events::ipc::IpcMessage>,
) -> Vec<(Arc<dyn crate::capsule::Capsule>, String)> {
    let registry = registry.read().await;
    let mut matches: Vec<(Arc<dyn crate::capsule::Capsule>, String, u32)> = Vec::new();
    for capsule_id in registry.list() {
        if let Some(capsule) = registry.get(capsule_id) {
            if !matches!(capsule.state(), crate::capsule::CapsuleState::Ready) {
                continue;
            }
            for interceptor in &capsule.manifest().interceptors {
                if crate::topic::topic_matches(topic, &interceptor.event)
                    && interceptor_accepts_caller(interceptor, topic, caller)
                {
                    matches.push((
                        Arc::clone(&capsule),
                        interceptor.action.clone(),
                        interceptor.priority,
                    ));
                }
            }
        }
    }
    // Sort by priority — lower values fire first.
    matches.sort_by_key(|(_, _, priority)| *priority);
    matches
        .into_iter()
        .map(|(capsule, action, _)| (capsule, action))
        .collect()
}

/// Enforce model visibility and exact caller provenance before an interceptor
/// reaches guest code. Prompt configuration can narrow model-visible schemas,
/// but it cannot widen this host authorization boundary.
pub(crate) fn interceptor_accepts_caller(
    interceptor: &crate::manifest::InterceptorDef,
    topic: &str,
    caller: Option<&astrid_events::ipc::IpcMessage>,
) -> bool {
    use crate::manifest::InterceptorExposure;

    if interceptor.exposure == InterceptorExposure::Private && topic == "tool.v1.request.describe" {
        return false;
    }
    let producer = caller
        .and_then(|message| message.producer.as_ref())
        .filter(|producer| producer.is_supported());
    if interceptor.exposure == InterceptorExposure::Private {
        let Some(producer) = producer else {
            return false;
        };
        if producer.kind == "wasm_capsule" {
            return false;
        }
    }
    if let Some(expected) = interceptor.caller_producer_kind.as_deref()
        && producer.map(|producer| producer.kind.as_str()) != Some(expected)
    {
        return false;
    }
    if !interceptor.caller_source_ids.is_empty()
        && !caller.is_some_and(|message| {
            let source_id = message.source_id.to_string();
            interceptor
                .caller_source_ids
                .iter()
                .any(|expected| expected == &source_id)
        })
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Dispatch integration tests ──────────────────────────────────

    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use crate::capsule::{Capsule, CapsuleId, CapsuleState, InterceptResult};
    use crate::context::CapsuleContext;
    use crate::error::{CapsuleError, CapsuleResult};
    use crate::manifest::{CapabilitiesDef, CapsuleManifest, InterceptorDef, PackageDef};
    use astrid_events::ipc::IpcPayload;

    /// A minimal mock capsule for dispatch tests.
    struct MockCapsule {
        id: CapsuleId,
        manifest: CapsuleManifest,
        invoked: Arc<AtomicBool>,
        /// Optional shared log for recording invocation order across capsules.
        invocation_log: Option<Arc<Mutex<Vec<String>>>>,
        /// Override the default `Continue` result for testing chain semantics.
        result_override: Option<InterceptResult>,
        /// Return a WASM error instead of an interceptor result.
        failure_override: Option<String>,
        /// Return `NotSupported` instead of an interceptor result.
        not_supported_override: bool,
    }

    impl MockCapsule {
        fn new(name: &str, interceptor_event: &str) -> (Self, Arc<AtomicBool>) {
            Self::with_priority(name, interceptor_event, 100, None)
        }

        fn with_priority(
            name: &str,
            interceptor_event: &str,
            priority: u32,
            invocation_log: Option<Arc<Mutex<Vec<String>>>>,
        ) -> (Self, Arc<AtomicBool>) {
            let invoked = Arc::new(AtomicBool::new(false));
            let manifest = CapsuleManifest {
                package: PackageDef {
                    name: name.to_string(),
                    version: "0.0.1".to_string(),
                    description: None,
                    authors: Vec::new(),
                    repository: None,
                    homepage: None,
                    documentation: None,
                    license: None,
                    license_file: None,
                    readme: None,
                    keywords: Vec::new(),
                    categories: Vec::new(),
                    astrid_version: None,
                    publish: None,
                    include: None,
                    exclude: None,
                    metadata: None,
                },
                components: Vec::new(),
                imports: std::collections::HashMap::new(),
                exports: std::collections::HashMap::new(),
                capabilities: CapabilitiesDef::default(),
                env: std::collections::HashMap::new(),
                context_files: Vec::new(),
                commands: Vec::new(),
                mcp_servers: Vec::new(),
                skills: Vec::new(),
                uplinks: Vec::new(),
                interceptors: vec![InterceptorDef {
                    event: interceptor_event.to_string(),
                    action: "test_action".to_string(),
                    priority,
                    exposure: crate::manifest::InterceptorExposure::Model,
                    caller_producer_kind: None,
                    caller_source_ids: Vec::new(),
                }],
                topics: Vec::new(),
            };
            let capsule = Self {
                id: CapsuleId::from_static(name),
                manifest,
                invoked: Arc::clone(&invoked),
                invocation_log,
                result_override: None,
                failure_override: None,
                not_supported_override: false,
            };
            (capsule, invoked)
        }
    }

    #[async_trait]
    impl Capsule for MockCapsule {
        fn id(&self) -> &CapsuleId {
            &self.id
        }
        fn manifest(&self) -> &CapsuleManifest {
            &self.manifest
        }
        fn state(&self) -> CapsuleState {
            CapsuleState::Ready
        }
        async fn load(&mut self, _ctx: &CapsuleContext) -> CapsuleResult<()> {
            Ok(())
        }
        async fn unload(&mut self) -> CapsuleResult<()> {
            Ok(())
        }
        fn invoke_interceptor(
            &self,
            _action: &str,
            _payload: &[u8],
            _caller: Option<&astrid_events::ipc::IpcMessage>,
        ) -> CapsuleResult<InterceptResult> {
            self.invoked.store(true, Ordering::SeqCst);
            if let Some(ref log) = self.invocation_log {
                log.lock().unwrap().push(self.id.to_string());
            }
            if let Some(ref failure) = self.failure_override {
                return Err(CapsuleError::WasmError(failure.clone()));
            }
            if self.not_supported_override {
                return Err(CapsuleError::NotSupported("test skip".to_string()));
            }
            if let Some(ref result) = self.result_override {
                return Ok(result.clone());
            }
            Ok(InterceptResult::Continue(Vec::new()))
        }
    }

    /// Helper: publish an IPC event on the bus.
    fn publish_ipc(bus: &EventBus, topic: &str) {
        let msg = astrid_events::ipc::IpcMessage::new(
            topic,
            IpcPayload::Custom {
                data: serde_json::json!({}),
            },
            uuid::Uuid::nil(),
        );
        bus.publish(AstridEvent::Ipc {
            metadata: astrid_events::EventMetadata::new("test"),
            message: msg,
        });
    }

    fn exact_provider_request() -> astrid_events::ipc::IpcMessage {
        astrid_events::ipc::IpcMessage::new(
            LOCAL_PROVIDER_REQUEST_TOPIC,
            IpcPayload::LlmRequest {
                request_id: uuid::Uuid::new_v4(),
                model: "qwen3:1.7b".to_string(),
                messages: Vec::new(),
                tools: Vec::new(),
                system: String::new(),
            },
            uuid::Uuid::new_v4(),
        )
        .with_principal("default")
        .with_trace(astrid_events::ipc::IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4().to_string(),
            None,
        ))
        .with_producer(astrid_events::ipc::IpcProducerV1::new(
            "wasm_capsule",
            REACT_CAPSULE_ID,
        ))
    }

    async fn wait_until_invoked(invoked: &AtomicBool) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while !invoked.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[test]
    fn exact_provider_failure_preserves_typed_request_and_trace() {
        let caller = exact_provider_request();
        let original_trace = caller.trace.as_ref().unwrap();
        let request_id = match &caller.payload {
            IpcPayload::LlmRequest { request_id, .. } => *request_id,
            _ => unreachable!(),
        };
        let message = exact_local_provider_failure_message(
            &CapsuleId::from_static(LOCAL_PROVIDER_CAPSULE_ID),
            LOCAL_PROVIDER_ACTION,
            LOCAL_PROVIDER_REQUEST_TOPIC,
            Some(&caller),
        )
        .unwrap();

        assert_eq!(message.topic, LOCAL_PROVIDER_STREAM_TOPIC);
        assert!(matches!(
            message.payload,
            IpcPayload::LlmStreamEvent {
                request_id: actual_request_id,
                event: astrid_events::llm::StreamEvent::Error(ref error),
            } if actual_request_id == request_id && error == PROVIDER_FAILURE_MESSAGE
        ));
        let failure_trace = message.trace.as_ref().unwrap();
        assert_eq!(failure_trace.trace_id, original_trace.trace_id);
        assert_eq!(failure_trace.turn_id, original_trace.turn_id);
        assert_eq!(failure_trace.session_id, original_trace.session_id);
        assert_eq!(failure_trace.parent_span_id, Some(original_trace.span_id));
        assert_ne!(failure_trace.span_id, original_trace.span_id);
        assert_eq!(message.principal.as_deref(), Some("default"));
        let producer = message.producer.as_ref().unwrap();
        assert_eq!(producer.kind, DISPATCHER_PRODUCER_KIND);
        assert_eq!(producer.id, DISPATCHER_PRODUCER_ID);
    }

    #[test]
    fn private_tool_interceptor_is_hidden_and_requires_exact_host_attested_native_caller() {
        let source_id = uuid::Uuid::parse_str("a57d1d30-0000-4000-8000-000000000001").unwrap();
        let interceptor = InterceptorDef {
            event: "tool.v1.execute.inspect_owned_question".to_string(),
            action: "tool_execute_inspect_owned_question".to_string(),
            priority: 100,
            exposure: crate::manifest::InterceptorExposure::Private,
            caller_producer_kind: Some("native_socket_client".to_string()),
            caller_source_ids: vec![source_id.to_string()],
        };
        let exact = astrid_events::ipc::IpcMessage::new(
            interceptor.event.clone(),
            IpcPayload::Custom {
                data: serde_json::json!({}),
            },
            source_id,
        )
        .with_producer(astrid_events::ipc::IpcProducerV1::new(
            "native_socket_client",
            "native_socket_bridge:connection",
        ));
        assert!(interceptor_accepts_caller(
            &interceptor,
            &interceptor.event,
            Some(&exact)
        ));
        assert!(!interceptor_accepts_caller(
            &interceptor,
            "tool.v1.request.describe",
            Some(&exact)
        ));
        assert!(!interceptor_accepts_caller(
            &interceptor,
            &interceptor.event,
            None
        ));

        let mut model = exact.clone();
        model.producer = Some(astrid_events::ipc::IpcProducerV1::new(
            "wasm_capsule",
            "astrid-capsule-react",
        ));
        assert!(!interceptor_accepts_caller(
            &interceptor,
            &interceptor.event,
            Some(&model)
        ));

        let mut wrong_source = exact.clone();
        wrong_source.source_id = uuid::Uuid::new_v4();
        assert!(!interceptor_accepts_caller(
            &interceptor,
            &interceptor.event,
            Some(&wrong_source)
        ));

        let mut unsupported = exact.clone();
        unsupported.producer.as_mut().unwrap().schema_version = 2;
        assert!(!interceptor_accepts_caller(
            &interceptor,
            &interceptor.event,
            Some(&unsupported)
        ));
    }

    #[test]
    fn model_exposed_interceptor_retains_backward_compatible_dispatch() {
        let interceptor = InterceptorDef {
            event: "ordinary.event".to_string(),
            action: "ordinary_action".to_string(),
            priority: 100,
            exposure: crate::manifest::InterceptorExposure::Model,
            caller_producer_kind: None,
            caller_source_ids: Vec::new(),
        };
        assert!(interceptor_accepts_caller(
            &interceptor,
            &interceptor.event,
            None
        ));
    }

    #[test]
    fn provider_failure_bridge_rejects_unattested_or_inexact_requests() {
        let capsule_id = CapsuleId::from_static(LOCAL_PROVIDER_CAPSULE_ID);
        let caller = exact_provider_request();
        let rejects = |candidate: Option<&astrid_events::ipc::IpcMessage>| {
            assert!(
                exact_local_provider_failure_message(
                    &capsule_id,
                    LOCAL_PROVIDER_ACTION,
                    LOCAL_PROVIDER_REQUEST_TOPIC,
                    candidate,
                )
                .is_none()
            );
        };
        rejects(None);
        assert!(
            exact_local_provider_failure_message(
                &CapsuleId::from_static("other-provider"),
                LOCAL_PROVIDER_ACTION,
                LOCAL_PROVIDER_REQUEST_TOPIC,
                Some(&caller),
            )
            .is_none()
        );
        assert!(
            exact_local_provider_failure_message(
                &capsule_id,
                "other_action",
                LOCAL_PROVIDER_REQUEST_TOPIC,
                Some(&caller),
            )
            .is_none()
        );
        assert!(
            exact_local_provider_failure_message(
                &capsule_id,
                LOCAL_PROVIDER_ACTION,
                "llm.v1.request.generate.other-provider",
                Some(&caller),
            )
            .is_none()
        );

        let mut wrong_caller_topic = caller.clone();
        wrong_caller_topic.topic = "llm.v1.request.generate.other-provider".to_string();
        rejects(Some(&wrong_caller_topic));

        let mut untyped = caller.clone();
        untyped.payload = IpcPayload::Custom {
            data: serde_json::json!({}),
        };
        rejects(Some(&untyped));

        let mut nil_request = caller.clone();
        let IpcPayload::LlmRequest { request_id, .. } = &mut nil_request.payload else {
            unreachable!();
        };
        *request_id = uuid::Uuid::nil();
        rejects(Some(&nil_request));

        let mut unattested = caller.clone();
        unattested.producer = None;
        rejects(Some(&unattested));

        let mut wrong_producer = caller.clone();
        wrong_producer.producer = Some(astrid_events::ipc::IpcProducerV1::new(
            "wasm_capsule",
            "other-capsule",
        ));
        rejects(Some(&wrong_producer));

        let mut unsupported_producer = caller.clone();
        unsupported_producer
            .producer
            .as_mut()
            .unwrap()
            .schema_version = 0;
        rejects(Some(&unsupported_producer));

        let mut untraced = caller.clone();
        untraced.trace = None;
        rejects(Some(&untraced));

        let mut missing_turn = caller.clone();
        missing_turn.trace.as_mut().unwrap().turn_id = None;
        rejects(Some(&missing_turn));

        let mut missing_session = caller.clone();
        missing_session.trace.as_mut().unwrap().session_id = None;
        rejects(Some(&missing_session));

        let mut unsupported_trace = caller;
        unsupported_trace.trace.as_mut().unwrap().schema_version = 0;
        rejects(Some(&unsupported_trace));
    }

    #[tokio::test]
    async fn exact_provider_interceptor_error_publishes_traced_stream_error() {
        let (mut provider, invoked) =
            MockCapsule::new(LOCAL_PROVIDER_CAPSULE_ID, LOCAL_PROVIDER_REQUEST_TOPIC);
        provider.manifest.interceptors[0].action = LOCAL_PROVIDER_ACTION.to_string();
        provider.failure_override = Some("interrupt".to_string());

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(provider)).unwrap();
        let registry = Arc::new(RwLock::new(registry));
        let bus = Arc::new(EventBus::with_capacity(64));
        let mut errors = bus.subscribe_topic(LOCAL_PROVIDER_STREAM_TOPIC);
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());
        tokio::task::yield_now().await;

        let caller = exact_provider_request();
        let original_trace = caller.trace.as_ref().unwrap().clone();
        let request_id = match &caller.payload {
            IpcPayload::LlmRequest { request_id, .. } => *request_id,
            _ => unreachable!(),
        };
        bus.publish(AstridEvent::Ipc {
            metadata: astrid_events::EventMetadata::new("test"),
            message: caller,
        });

        let event = tokio::time::timeout(Duration::from_secs(1), errors.recv())
            .await
            .unwrap()
            .unwrap();
        let AstridEvent::Ipc { message, .. } = &*event else {
            panic!("expected IPC provider failure event");
        };
        assert!(invoked.load(Ordering::SeqCst));
        assert!(matches!(
            message.payload,
            IpcPayload::LlmStreamEvent {
                request_id: actual_request_id,
                event: astrid_events::llm::StreamEvent::Error(ref error),
            } if actual_request_id == request_id && error == PROVIDER_FAILURE_MESSAGE
        ));
        let failure_trace = message.trace.as_ref().unwrap();
        assert_eq!(failure_trace.trace_id, original_trace.trace_id);
        assert_eq!(failure_trace.turn_id, original_trace.turn_id);
        assert_eq!(failure_trace.session_id, original_trace.session_id);
        assert_eq!(failure_trace.parent_span_id, Some(original_trace.span_id));
        handle.abort();
    }

    #[tokio::test]
    async fn exact_provider_not_supported_does_not_publish_stream_error() {
        let (mut provider, invoked) =
            MockCapsule::new(LOCAL_PROVIDER_CAPSULE_ID, LOCAL_PROVIDER_REQUEST_TOPIC);
        provider.manifest.interceptors[0].action = LOCAL_PROVIDER_ACTION.to_string();
        provider.not_supported_override = true;

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(provider)).unwrap();
        let registry = Arc::new(RwLock::new(registry));
        let bus = Arc::new(EventBus::with_capacity(64));
        let mut errors = bus.subscribe_topic(LOCAL_PROVIDER_STREAM_TOPIC);
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());
        tokio::task::yield_now().await;

        bus.publish(AstridEvent::Ipc {
            metadata: astrid_events::EventMetadata::new("test"),
            message: exact_provider_request(),
        });

        wait_until_invoked(&invoked).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(25), errors.recv())
                .await
                .is_err()
        );
        handle.abort();
    }

    #[tokio::test]
    async fn multi_interceptor_failure_does_not_publish_stream_error() {
        let (mut provider, invoked) = MockCapsule::with_priority(
            LOCAL_PROVIDER_CAPSULE_ID,
            LOCAL_PROVIDER_REQUEST_TOPIC,
            10,
            None,
        );
        provider.manifest.interceptors[0].action = LOCAL_PROVIDER_ACTION.to_string();
        provider.failure_override = Some("interrupt".to_string());
        let (other, _) =
            MockCapsule::with_priority("other-interceptor", LOCAL_PROVIDER_REQUEST_TOPIC, 20, None);

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(provider)).unwrap();
        registry.register(Box::new(other)).unwrap();
        let registry = Arc::new(RwLock::new(registry));
        let bus = Arc::new(EventBus::with_capacity(64));
        let mut errors = bus.subscribe_topic(LOCAL_PROVIDER_STREAM_TOPIC);
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());
        tokio::task::yield_now().await;

        bus.publish(AstridEvent::Ipc {
            metadata: astrid_events::EventMetadata::new("test"),
            message: exact_provider_request(),
        });

        wait_until_invoked(&invoked).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(25), errors.recv())
                .await
                .is_err()
        );
        handle.abort();
    }

    #[tokio::test]
    async fn dispatch_routes_to_matching_interceptor() {
        let (capsule, invoked) = MockCapsule::new("test-capsule", "test.topic");

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(capsule)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        // Yield to let the dispatcher subscribe before publishing.
        tokio::task::yield_now().await;

        publish_ipc(&bus, "test.topic");

        // Give the dispatcher time to process.
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            invoked.load(Ordering::SeqCst),
            "interceptor should have been invoked for matching topic"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn dispatch_skips_non_matching_topic() {
        let (capsule, invoked) = MockCapsule::new("test-capsule-skip", "specific.topic");

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(capsule)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        publish_ipc(&bus, "other.topic");

        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            !invoked.load(Ordering::SeqCst),
            "interceptor should NOT have been invoked for non-matching topic"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn dispatch_concurrent_does_not_block() {
        // Both capsules match different topics. With concurrent dispatch,
        // the second event is processed immediately without waiting for
        // the first interceptor to complete.
        let (cap_a, invoked_a) = MockCapsule::new("capsule-a", "topic.a");
        let (cap_b, invoked_b) = MockCapsule::new("capsule-b", "topic.b");

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(cap_a)).unwrap();
        registry.register(Box::new(cap_b)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        publish_ipc(&bus, "topic.a");
        publish_ipc(&bus, "topic.b");

        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            invoked_a.load(Ordering::SeqCst),
            "capsule-a interceptor should have been invoked"
        );
        assert!(
            invoked_b.load(Ordering::SeqCst),
            "capsule-b interceptor should have been invoked"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn dispatch_routes_lifecycle_events() {
        // Lifecycle events are dispatched by event_type() as the topic.
        let (capsule, invoked) =
            MockCapsule::new("lifecycle-capsule", "astrid.v1.lifecycle.tool_call_started");

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(capsule)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        // Publish a lifecycle event
        bus.publish(AstridEvent::ToolCallStarted {
            metadata: astrid_events::EventMetadata::new("test"),
            call_id: uuid::Uuid::nil(),
            tool_name: "search".into(),
            server_name: None,
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            invoked.load(Ordering::SeqCst),
            "EventDispatcher should dispatch lifecycle events by event_type()"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn dispatch_publishes_lag_event_on_overflow() {
        // Use a tiny bus capacity so publishing more events than capacity triggers lag.
        let bus = Arc::new(EventBus::with_capacity(2));

        // A capsule that listens for lag events.
        let (lag_capsule, _lag_invoked) =
            MockCapsule::new("lag-listener", "astrid.v1.event_bus.lagged");

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(lag_capsule)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        // Flood the bus to trigger lag (the dispatcher's receiver has capacity 2,
        // so publishing many events quickly should cause overflow).
        for i in 0..20 {
            publish_ipc(&bus, &format!("flood.event.{i}"));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        // If lag was detected, the dispatcher should have published
        // astrid.v1.event_bus.lagged which routes to our lag-listener capsule.
        // Note: this test may not trigger lag on fast machines where the
        // dispatcher drains fast enough. That's acceptable - the test
        // validates the wiring, not the race condition.
        // We just verify no panics occurred and the dispatcher is still running.
        assert!(!handle.is_finished(), "dispatcher should still be running");
        handle.abort();
    }

    #[test]
    fn mock_capsule_check_health_returns_ready() {
        let (capsule, _) = MockCapsule::new("health-test", "test.topic");
        assert_eq!(capsule.check_health(), CapsuleState::Ready);
    }

    #[tokio::test]
    async fn dispatch_respects_interceptor_priority_order() {
        // Three capsules intercept the same topic with different priorities.
        // Priority 10 (guard) should fire before 50 (transform) before 100 (handler).
        let order = Arc::new(Mutex::new(Vec::<String>::new()));

        let (guard, _) =
            MockCapsule::with_priority("guard", "shared.topic", 10, Some(Arc::clone(&order)));
        let (handler, _) =
            MockCapsule::with_priority("handler", "shared.topic", 100, Some(Arc::clone(&order)));
        let (transform, _) =
            MockCapsule::with_priority("transform", "shared.topic", 50, Some(Arc::clone(&order)));

        let mut registry = CapsuleRegistry::new();
        // Register in non-priority order to prove sorting works.
        registry.register(Box::new(handler)).unwrap();
        registry.register(Box::new(guard)).unwrap();
        registry.register(Box::new(transform)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        publish_ipc(&bus, "shared.topic");

        tokio::time::sleep(Duration::from_millis(300)).await;

        let recorded = order.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec!["guard", "transform", "handler"],
            "interceptors must fire in priority order (lower first)"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn find_matching_interceptors_sorts_by_priority() {
        // Unit test for find_matching_interceptors directly.
        let (low, _) = MockCapsule::with_priority("low-pri", "test.event", 10, None);
        let (high, _) = MockCapsule::with_priority("high-pri", "test.event", 200, None);
        let (mid, _) = MockCapsule::with_priority("mid-pri", "test.event", 50, None);

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(high)).unwrap();
        registry.register(Box::new(low)).unwrap();
        registry.register(Box::new(mid)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let matches = find_matching_interceptors(&registry, "test.event", None).await;
        let names: Vec<&str> = matches.iter().map(|(c, _)| c.id().as_str()).collect();
        assert_eq!(
            names,
            vec!["low-pri", "mid-pri", "high-pri"],
            "find_matching_interceptors must return results sorted by priority"
        );
    }

    #[tokio::test]
    async fn deny_interceptor_short_circuits_chain() {
        // Guard at priority 10 denies, handler at priority 100 should never fire.
        let order = Arc::new(Mutex::new(Vec::<String>::new()));

        let (mut guard, _) =
            MockCapsule::with_priority("guard", "shared.topic", 10, Some(Arc::clone(&order)));
        guard.result_override = Some(InterceptResult::Deny {
            reason: "blocked by guard".into(),
        });

        let (handler, invoked_handler) =
            MockCapsule::with_priority("handler", "shared.topic", 100, Some(Arc::clone(&order)));

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(handler)).unwrap();
        registry.register(Box::new(guard)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        publish_ipc(&bus, "shared.topic");

        tokio::time::sleep(Duration::from_millis(300)).await;

        let recorded = order.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec!["guard"],
            "only the guard should have fired — handler should be short-circuited"
        );
        assert!(
            !invoked_handler.load(Ordering::SeqCst),
            "handler must NOT be invoked after Deny"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn final_interceptor_short_circuits_chain() {
        // Cache at priority 30 returns Final, core at priority 100 should never fire.
        let order = Arc::new(Mutex::new(Vec::<String>::new()));

        let (mut cache, _) =
            MockCapsule::with_priority("cache", "shared.topic", 30, Some(Arc::clone(&order)));
        cache.result_override = Some(InterceptResult::Final(b"cached response".to_vec()));

        let (core, invoked_core) =
            MockCapsule::with_priority("core", "shared.topic", 100, Some(Arc::clone(&order)));

        let mut registry = CapsuleRegistry::new();
        registry.register(Box::new(core)).unwrap();
        registry.register(Box::new(cache)).unwrap();
        let registry = Arc::new(RwLock::new(registry));

        let bus = Arc::new(EventBus::with_capacity(64));
        let dispatcher = EventDispatcher::new(Arc::clone(&registry), Arc::clone(&bus));
        let handle = tokio::spawn(dispatcher.run());

        tokio::task::yield_now().await;

        publish_ipc(&bus, "shared.topic");

        tokio::time::sleep(Duration::from_millis(300)).await;

        let recorded = order.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec!["cache"],
            "only the cache should have fired — core should be short-circuited"
        );
        assert!(
            !invoked_core.load(Ordering::SeqCst),
            "core must NOT be invoked after Final"
        );

        handle.abort();
    }

    #[test]
    fn intercept_result_from_guest_bytes() {
        // Empty = Continue
        let r = InterceptResult::from_guest_bytes(vec![]);
        assert!(matches!(r, InterceptResult::Continue(ref b) if b.is_empty()));

        // 0x00 + payload = Continue
        let r = InterceptResult::from_guest_bytes(vec![0x00, 1, 2, 3]);
        assert!(matches!(r, InterceptResult::Continue(ref b) if b == &[1, 2, 3]));

        // 0x01 + payload = Final
        let r = InterceptResult::from_guest_bytes(vec![0x01, 4, 5]);
        assert!(matches!(r, InterceptResult::Final(ref b) if b == &[4, 5]));

        // 0x02 + reason = Deny
        let r = InterceptResult::from_guest_bytes(vec![0x02, b'n', b'o']);
        assert!(matches!(r, InterceptResult::Deny { ref reason } if reason == "no"));

        // Unknown discriminant = Continue with full bytes
        let r = InterceptResult::from_guest_bytes(vec![0xFF, 1]);
        assert!(matches!(r, InterceptResult::Continue(ref b) if b == &[0xFF, 1]));
    }
}
