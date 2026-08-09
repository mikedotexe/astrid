//! Event bus for broadcasting events to subscribers.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, trace, warn};

use crate::event::AstridEvent;
use crate::ipc::{
    IpcMessage, IpcPayload, IpcTraceContextV1, LOCAL_PROVIDER_TURN_METRICS_MAX_ENTRIES,
    LocalProviderRequestAttemptV1, LocalProviderRequestOutcomeV1, LocalProviderTurnMetricsV1,
};
use crate::subscriber::SubscriberRegistry;

/// Default channel capacity for the event bus.
pub(crate) const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

const MAX_TRACE_CORRELATIONS: usize = 4096;
// Full-key tombstones live until process restart so an old turn can never be re-admitted as a
// partial suffix. At 96 scheduled turns/day this bound covers more than 170 days.
const MAX_LOCAL_PROVIDER_TURNS: usize = 16_384;
const LOCAL_PROVIDER_TURN_TTL: Duration = Duration::from_secs(30 * 60);
const CANONICAL_AGENT_RESPONSE_TOPIC: &str = "agent.v1.response";
const REACT_CAPSULE_ID: &str = "astrid-capsule-react";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::struct_field_names)]
struct LocalProviderTurnKey {
    trace_id: uuid::Uuid,
    turn_id: uuid::Uuid,
    session_id: String,
    chain_id: Option<String>,
}

impl LocalProviderTurnKey {
    fn from_trace(trace: &IpcTraceContextV1) -> Option<Self> {
        if !trace.is_supported() {
            return None;
        }
        Some(Self {
            trace_id: trace.trace_id,
            turn_id: trace.turn_id?,
            session_id: trace.session_id.clone()?,
            chain_id: trace.chain_id.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalProviderTraceState {
    Active,
    Taken,
    Poisoned,
}

#[derive(Debug, Clone)]
struct LocalProviderTraceClaim {
    state: LocalProviderTraceState,
    touched_at: Instant,
}

#[derive(Debug)]
struct PendingLocalProviderAttempt {
    request_id: uuid::Uuid,
    outcome: Option<LocalProviderRequestOutcomeV1>,
    request_header_latency_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct LocalProviderTurnEntry {
    attempt_count: u64,
    successful_header_count: u64,
    attempts: BTreeMap<uuid::Uuid, PendingLocalProviderAttempt>,
    attempt_order: Vec<uuid::Uuid>,
}

/// Host-private take-once request registry. Capacity exhaustion disables attribution until process
/// restart rather than evicting state and risking a false suffix count.
#[derive(Debug, Default)]
struct LocalProviderMetricsRegistry {
    claims: BTreeMap<LocalProviderTurnKey, LocalProviderTraceClaim>,
    turns: BTreeMap<LocalProviderTurnKey, LocalProviderTurnEntry>,
    disabled: bool,
}

impl LocalProviderMetricsRegistry {
    fn cleanup(&mut self, now: Instant) {
        // Only active entries can expire. Process-lifetime Taken/Poisoned tombstones are never
        // scanned on the request path, so cost scales with in-flight turns rather than uptime.
        let expired_active = self
            .turns
            .keys()
            .filter(|key| {
                self.claims.get(*key).is_some_and(|claim| {
                    claim.state == LocalProviderTraceState::Active
                        && now.duration_since(claim.touched_at) >= LOCAL_PROVIDER_TURN_TTL
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in expired_active {
            self.turns.remove(&key);
            if let Some(claim) = self.claims.get_mut(&key) {
                // Retain the bounded full-key tombstone for process lifetime. Re-admitting this
                // abandoned turn later could report only a suffix of its real attempts.
                claim.state = LocalProviderTraceState::Poisoned;
                claim.touched_at = now;
            }
        }
    }

    fn is_disabled(&mut self, now: Instant) -> bool {
        self.cleanup(now);
        self.disabled
    }

    fn disable_for_process(&mut self, now: Instant) {
        for claim in self.claims.values_mut() {
            if claim.state == LocalProviderTraceState::Active {
                claim.state = LocalProviderTraceState::Poisoned;
                claim.touched_at = now;
            }
        }
        self.turns.clear();
        // A process-lifetime stop is the only bounded response that cannot later re-admit an
        // evicted turn and publish a false suffix count. Restart starts a new observation epoch.
        self.disabled = true;
    }

    fn poison_claim(&mut self, key: &LocalProviderTurnKey, now: Instant) {
        self.turns.remove(key);
        self.claims.insert(
            key.clone(),
            LocalProviderTraceClaim {
                state: LocalProviderTraceState::Poisoned,
                touched_at: now,
            },
        );
    }

    fn begin(
        &mut self,
        trace: &IpcTraceContextV1,
        request_id: uuid::Uuid,
        now: Instant,
    ) -> Option<uuid::Uuid> {
        if request_id.is_nil() || self.is_disabled(now) {
            return None;
        }
        let key = LocalProviderTurnKey::from_trace(trace)?;
        if let Some(claim) = self.claims.get(&key) {
            if claim.state != LocalProviderTraceState::Active {
                return None;
            }
        } else {
            if self.claims.len() >= MAX_LOCAL_PROVIDER_TURNS {
                self.disable_for_process(now);
                return None;
            }
            self.claims.insert(
                key.clone(),
                LocalProviderTraceClaim {
                    state: LocalProviderTraceState::Active,
                    touched_at: now,
                },
            );
            self.turns
                .insert(key.clone(), LocalProviderTurnEntry::default());
        }

        let attempt_id = uuid::Uuid::new_v4();
        let Some(entry) = self.turns.get_mut(&key) else {
            self.poison_claim(&key, now);
            return None;
        };
        if entry.attempt_order.len() >= LOCAL_PROVIDER_TURN_METRICS_MAX_ENTRIES {
            self.poison_claim(&key, now);
            return None;
        }
        let Some(attempt_count) = entry.attempt_count.checked_add(1) else {
            self.poison_claim(&key, now);
            return None;
        };
        entry.attempt_count = attempt_count;
        entry.attempts.insert(
            attempt_id,
            PendingLocalProviderAttempt {
                request_id,
                outcome: None,
                request_header_latency_ms: None,
            },
        );
        entry.attempt_order.push(attempt_id);
        if let Some(claim) = self.claims.get_mut(&key) {
            claim.touched_at = now;
        }
        Some(attempt_id)
    }

    fn finish(
        &mut self,
        trace: &IpcTraceContextV1,
        attempt_id: uuid::Uuid,
        outcome: LocalProviderRequestOutcomeV1,
        request_header_latency_ms: Option<u64>,
        now: Instant,
    ) -> bool {
        if attempt_id.is_nil() || self.is_disabled(now) {
            return false;
        }
        let Some(key) = LocalProviderTurnKey::from_trace(trace) else {
            return false;
        };
        let Some(claim) = self.claims.get(&key) else {
            return false;
        };
        if claim.state != LocalProviderTraceState::Active {
            return false;
        }
        let Some(entry) = self.turns.get_mut(&key) else {
            self.poison_claim(&key, now);
            return false;
        };
        let Some(attempt) = entry.attempts.get_mut(&attempt_id) else {
            self.poison_claim(&key, now);
            return false;
        };
        if attempt.outcome.is_some()
            || matches!(
                (outcome, request_header_latency_ms),
                (LocalProviderRequestOutcomeV1::SuccessfulHeaders, None)
                    | (
                        LocalProviderRequestOutcomeV1::NonSuccessStatus
                            | LocalProviderRequestOutcomeV1::UnknownPeer
                            | LocalProviderRequestOutcomeV1::NonLoopbackPeer
                            | LocalProviderRequestOutcomeV1::Timeout
                            | LocalProviderRequestOutcomeV1::TransportError
                            | LocalProviderRequestOutcomeV1::Cancelled,
                        Some(_)
                    )
            )
        {
            self.poison_claim(&key, now);
            return false;
        }
        attempt.outcome = Some(outcome);
        attempt.request_header_latency_ms = request_header_latency_ms;
        if outcome == LocalProviderRequestOutcomeV1::SuccessfulHeaders {
            let Some(count) = entry.successful_header_count.checked_add(1) else {
                self.poison_claim(&key, now);
                return false;
            };
            entry.successful_header_count = count;
        }
        if let Some(claim) = self.claims.get_mut(&key) {
            claim.touched_at = now;
        }
        true
    }

    fn take(
        &mut self,
        trace: &IpcTraceContextV1,
        now: Instant,
    ) -> Option<LocalProviderTurnMetricsV1> {
        if self.is_disabled(now) {
            return None;
        }
        let key = LocalProviderTurnKey::from_trace(trace)?;
        let Some(claim) = self.claims.get(&key) else {
            if self.claims.len() >= MAX_LOCAL_PROVIDER_TURNS {
                self.disable_for_process(now);
            } else {
                // Even a zero-attempt final consumes the key. A late request plus replayed final
                // must never manufacture post-final metrics for an already completed turn.
                self.claims.insert(
                    key,
                    LocalProviderTraceClaim {
                        state: LocalProviderTraceState::Taken,
                        touched_at: now,
                    },
                );
            }
            return None;
        };
        if claim.state != LocalProviderTraceState::Active {
            return None;
        }
        let entry = self.turns.remove(&key)?;
        if let Some(claim) = self.claims.get_mut(&key) {
            claim.state = LocalProviderTraceState::Taken;
            claim.touched_at = now;
        }
        if entry
            .attempts
            .values()
            .any(|attempt| attempt.outcome.is_none())
        {
            return None;
        }
        let requests = entry
            .attempt_order
            .iter()
            .take(LOCAL_PROVIDER_TURN_METRICS_MAX_ENTRIES)
            .filter_map(|attempt_id| {
                let attempt = entry.attempts.get(attempt_id)?;
                Some(LocalProviderRequestAttemptV1 {
                    attempt_id: *attempt_id,
                    request_id: attempt.request_id,
                    outcome: attempt.outcome?,
                    request_header_latency_ms: attempt.request_header_latency_ms,
                })
            })
            .collect();
        let summary = LocalProviderTurnMetricsV1::new(
            entry.attempt_count,
            entry.successful_header_count,
            requests,
        );
        summary.is_supported().then_some(summary)
    }
}

/// Bounded, observational-only correlations used to repair context lost by
/// asynchronous capsule boundaries. Keys are protocol identifiers, never
/// timestamps, and none of this state participates in authorization.
#[derive(Debug, Default)]
struct IpcTraceRegistry {
    llm_requests: BTreeMap<uuid::Uuid, IpcTraceContextV1>,
    tool_calls: BTreeMap<String, IpcTraceContextV1>,
    local_provider_metrics: LocalProviderMetricsRegistry,
}

impl IpcTraceRegistry {
    fn enrich(&mut self, message: &mut IpcMessage) {
        // This field is host-owned even on native socket ingress. Never preserve a value supplied
        // by a guest; a canonical final React response may receive a take-once replacement below.
        message.local_provider_metrics = None;
        if message
            .trace
            .as_ref()
            .is_some_and(|trace| !trace.is_supported())
        {
            message.trace = None;
        }

        Self::normalize_user_input_root(message);
        Self::reject_mismatched_session(message);
        if message
            .trace
            .as_ref()
            .is_some_and(|trace| !trace.is_supported())
        {
            message.trace = None;
        }

        if message.trace.is_none() {
            message.trace = self.exact_parent(message).map(IpcTraceContextV1::child);
        }

        let Some(trace) = message
            .trace
            .as_ref()
            .filter(|trace| trace.is_supported())
            .cloned()
        else {
            return;
        };

        match &message.payload {
            IpcPayload::LlmRequest { request_id, .. } => {
                insert_bounded(&mut self.llm_requests, *request_id, trace.clone());
            },
            IpcPayload::ToolExecuteRequest { call_id, .. } => {
                insert_bounded(&mut self.tool_calls, call_id.clone(), trace.clone());
            },
            _ => {},
        }

        if message.topic == CANONICAL_AGENT_RESPONSE_TOPIC
            && message.producer.as_ref().is_some_and(|producer| {
                producer.is_supported()
                    && producer.kind == "wasm_capsule"
                    && producer.id == REACT_CAPSULE_ID
            })
            && matches!(
                &message.payload,
                IpcPayload::AgentResponse {
                    is_final: true,
                    session_id,
                    ..
                } if trace.session_id.as_deref() == Some(session_id.as_str())
            )
        {
            message.local_provider_metrics =
                self.local_provider_metrics.take(&trace, Instant::now());
        }
    }

    fn normalize_user_input_root(message: &mut IpcMessage) {
        let IpcPayload::UserInput { session_id, .. } = &message.payload else {
            return;
        };
        let trace_id = message
            .trace
            .as_ref()
            .map_or_else(uuid::Uuid::new_v4, |trace| trace.trace_id);
        let chain_id = message
            .trace
            .as_ref()
            .and_then(|trace| trace.chain_id.clone());
        let needs_root = message.trace.as_ref().is_none_or(|trace| {
            trace.parent_span_id.is_some() || trace.session_id.as_deref() != Some(session_id)
        });
        if needs_root {
            message.trace = Some(IpcTraceContextV1::root(
                trace_id,
                session_id.clone(),
                chain_id,
            ));
        }
    }

    fn reject_mismatched_session(message: &mut IpcMessage) {
        let IpcPayload::AgentResponse { session_id, .. } = &message.payload else {
            return;
        };
        if message.trace.as_ref().is_some_and(|trace| {
            trace
                .session_id
                .as_deref()
                .is_some_and(|id| id != session_id)
        }) {
            message.trace = None;
        }
    }

    fn exact_parent(&self, message: &IpcMessage) -> Option<&IpcTraceContextV1> {
        match &message.payload {
            IpcPayload::LlmStreamEvent { request_id, .. }
            | IpcPayload::LlmResponse { request_id, .. } => self.llm_requests.get(request_id),
            IpcPayload::ToolExecuteResult { call_id, .. } => self.tool_calls.get(call_id),
            _ => None,
        }
    }
}

fn insert_bounded<K: Ord + Clone, V>(map: &mut BTreeMap<K, V>, key: K, value: V) {
    if map.len() >= MAX_TRACE_CORRELATIONS
        && !map.contains_key(&key)
        && let Some(oldest_key) = map.keys().next().cloned()
    {
        map.remove(&oldest_key);
    }
    map.insert(key, value);
}

/// Event bus for broadcasting events to all subscribers.
///
/// The event bus uses a broadcast channel to deliver events to all
/// connected receivers. Events are delivered asynchronously and in order.
///
/// **WARNING:** Synchronous subscribers (`SubscriberRegistry`) are shared
/// across clones. Storing a cloned `EventBus` inside a synchronous subscriber
/// will create a memory leak via an `Arc` reference cycle. If a synchronous
/// subscriber needs to publish events, store a `std::sync::Weak<EventBus>`
/// or communicate via a separate channel.
#[derive(Debug)]
pub struct EventBus {
    /// Sender for broadcasting events.
    sender: broadcast::Sender<Arc<AstridEvent>>,
    /// Registry for synchronous subscribers.
    registry: Arc<SubscriberRegistry>,
    /// Channel capacity.
    capacity: usize,
    /// Monotonic sequence counter for IPC message ordering.
    ipc_seq: Arc<AtomicU64>,
    /// Exact protocol-identifier trace correlations.
    ipc_traces: Arc<Mutex<IpcTraceRegistry>>,
}

impl EventBus {
    fn lock_ipc_traces(&self) -> std::sync::MutexGuard<'_, IpcTraceRegistry> {
        match self.ipc_traces.lock() {
            Ok(registry) => registry,
            Err(poisoned) => {
                let mut registry = poisoned.into_inner();
                registry
                    .local_provider_metrics
                    .disable_for_process(Instant::now());
                warn!("IPC trace registry poisoned; local-provider attribution disabled");
                registry
            },
        }
    }

    /// Create a new event bus with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new event bus with specified capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            registry: Arc::new(SubscriberRegistry::new()),
            capacity,
            ipc_seq: Arc::new(AtomicU64::new(1)),
            ipc_traces: Arc::new(Mutex::new(IpcTraceRegistry::default())),
        }
    }

    /// Register an exact eligible local-provider send before network dispatch.
    ///
    /// The returned host-generated attempt ID must be terminalized with
    /// [`Self::finish_local_provider_request`]. This state is private to the event bus and is
    /// never broadcast independently of the canonical final response.
    #[must_use]
    pub fn begin_local_provider_request(
        &self,
        trace: &IpcTraceContextV1,
        request_id: uuid::Uuid,
    ) -> Option<uuid::Uuid> {
        self.lock_ipc_traces()
            .local_provider_metrics
            .begin(trace, request_id, Instant::now())
    }

    /// Terminalize a previously registered local-provider request on every send return path.
    #[must_use]
    pub fn finish_local_provider_request(
        &self,
        trace: &IpcTraceContextV1,
        attempt_id: uuid::Uuid,
        outcome: LocalProviderRequestOutcomeV1,
        request_header_latency_ms: Option<u64>,
    ) -> bool {
        self.lock_ipc_traces().local_provider_metrics.finish(
            trace,
            attempt_id,
            outcome,
            request_header_latency_ms,
            Instant::now(),
        )
    }

    /// Publish an event to all subscribers.
    ///
    /// This method broadcasts the event to all async subscribers and
    /// notifies all synchronous subscribers in the registry.
    ///
    /// Returns the number of async receivers that received the event.
    pub fn publish(&self, mut event: AstridEvent) -> usize {
        // Stamp IPC messages with a monotonic sequence number for ordered delivery.
        if let AstridEvent::Ipc {
            ref mut metadata,
            ref mut message,
        } = event
        {
            self.lock_ipc_traces().enrich(message);
            message.seq = self.ipc_seq.fetch_add(1, Ordering::Relaxed);
            if let Some(ipc_trace) = message.trace.as_ref() {
                metadata.correlation_id = Some(ipc_trace.trace_id);
                if metadata.session_id.is_none() {
                    metadata.session_id = ipc_trace
                        .session_id
                        .as_deref()
                        .and_then(|session_id| uuid::Uuid::parse_str(session_id).ok());
                }
            }
        }
        let event = Arc::new(event);

        trace!(event_type = %event.event_type(), "Publishing event");

        // Broadcast to async subscribers first so they don't wait for synchronous subscribers
        let count = if let Ok(c) = self.sender.send(Arc::clone(&event)) {
            debug!(
                event_type = %event.event_type(),
                receiver_count = c,
                "Event published"
            );
            c
        } else {
            // No receivers - this is fine
            trace!(event_type = %event.event_type(), "No receivers for event");
            0
        };

        // Notify synchronous subscribers
        self.registry.notify(&event, self);

        count
    }

    /// Subscribe to events.
    ///
    /// Returns a receiver that will receive all published events.
    #[must_use]
    pub fn subscribe(&self) -> EventReceiver {
        EventReceiver::new(self.sender.subscribe(), None)
    }

    /// Subscribe to IPC events matching a specific topic pattern.
    ///
    /// The pattern can be an exact match (e.g. `astrid.cli.input`)
    /// or end with a trailing `*` (e.g. `astrid.v1.request.*`) which matches
    /// one or more remaining dot-separated segments up to a maximum depth of 20.
    /// Middle wildcards (e.g. `astrid.*.event`) match exactly one segment.
    #[must_use]
    pub fn subscribe_topic(&self, topic_pattern: impl Into<String>) -> EventReceiver {
        EventReceiver::new(self.sender.subscribe(), Some(topic_pattern.into()))
    }

    /// Get the synchronous subscriber registry (test-only).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn registry(&self) -> &SubscriberRegistry {
        &self.registry
    }

    /// Get the current number of active subscribers (both async and synchronous).
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.sender
            .receiver_count()
            .saturating_add(self.registry.len())
    }

    /// Get the channel capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        // Create a new bus that shares the same sender,
        // subscriber registry, and sequence counter.
        Self {
            sender: self.sender.clone(),
            registry: Arc::clone(&self.registry),
            capacity: self.capacity,
            ipc_seq: Arc::clone(&self.ipc_seq),
            ipc_traces: Arc::clone(&self.ipc_traces),
        }
    }
}

/// Receiver for events from the event bus.
pub struct EventReceiver {
    receiver: broadcast::Receiver<Arc<AstridEvent>>,
    /// Optional topic pattern. If specified, only `AstridEvent::Ipc` messages matching
    /// this pattern will be yielded (non-IPC events will be strictly filtered out).
    topic_pattern: Option<String>,
    /// Cumulative count of messages lost due to broadcast channel lag.
    /// Incremented each time the receiver falls behind the sender.
    lagged_count: u64,
}

impl EventReceiver {
    /// Create a new receiver with an optional topic filter.
    pub(crate) fn new(
        receiver: broadcast::Receiver<Arc<AstridEvent>>,
        topic_pattern: Option<String>,
    ) -> Self {
        Self {
            receiver,
            topic_pattern,
            lagged_count: 0,
        }
    }

    /// Maximum allowed topic depth (dot-separated segments).
    const MAX_TOPIC_DEPTH: usize = 20;

    /// Check if an event matches our topic pattern.
    ///
    /// Uses segment-aware matching. A `*` in a non-trailing position matches
    /// exactly one segment. A trailing `*` (last segment) matches one or more
    /// remaining segments, enabling namespace-level subscriptions (e.g.
    /// `astrid.v1.lifecycle.*` matches all lifecycle events regardless of depth).
    ///
    /// Note: this differs from `dispatcher::topic_matches` used for interceptor
    /// routing, where `*` always matches exactly one segment (equal segment
    /// count is required). Topics deeper than 20 segments are rejected.
    fn matches(&self, event: &AstridEvent) -> bool {
        let Some(pattern) = &self.topic_pattern else {
            return true;
        };

        let AstridEvent::Ipc { message, .. } = event else {
            // If a topic pattern is set, we ONLY care about matching IPC events.
            return false;
        };

        let topic = &message.topic;

        // Reject topics deeper than the maximum allowed depth.
        if topic.split('.').count() > Self::MAX_TOPIC_DEPTH {
            return false;
        }

        // Trailing wildcard: last segment is `*` and matches 1+ remaining segments.
        if let Some(prefix_pat) = pattern.strip_suffix(".*") {
            let mut prefix_segs = prefix_pat.split('.');
            let mut topic_segs = topic.split('.');

            // All prefix segments must match (with single-segment `*` support).
            let prefix_matched = prefix_segs
                .by_ref()
                .zip(topic_segs.by_ref())
                .all(|(p, t)| p == "*" || p == t);

            // Prefix must be fully consumed and topic must have 1+ remaining
            // segments (the trailing `*` matches 1+ segments).
            prefix_matched && prefix_segs.next().is_none() && topic_segs.next().is_some()
        } else {
            // Exact segment-count match with single-segment `*` wildcards.
            let mut pat_segs = pattern.split('.');
            let mut topic_segs = topic.split('.');

            let all_matched = pat_segs
                .by_ref()
                .zip(topic_segs.by_ref())
                .all(|(p, t)| p == "*" || p == t);

            // Both iterators must be exhausted (equal segment count).
            all_matched && pat_segs.next().is_none() && topic_segs.next().is_none()
        }
    }

    /// Returns and resets the cumulative count of messages lost due to
    /// broadcast channel lag since the last call.
    pub fn drain_lagged(&mut self) -> u64 {
        std::mem::take(&mut self.lagged_count)
    }

    /// Receive the next event.
    ///
    /// Returns `None` if the channel is closed or if events were dropped
    /// due to the receiver being too slow.
    pub async fn recv(&mut self) -> Option<Arc<AstridEvent>> {
        let mut skipped: usize = 0;
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    if self.matches(&event) {
                        return Some(event);
                    }
                    skipped = skipped.wrapping_add(1);
                    if skipped.is_multiple_of(100) {
                        #[cfg(not(target_os = "wasi"))]
                        tokio::task::yield_now().await;
                        #[cfg(target_os = "wasi")]
                        std::hint::spin_loop();
                    }
                },
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    warn!(skipped = count, "Event receiver lagged, events dropped");
                    self.lagged_count = self.lagged_count.saturating_add(count);
                    // Continue receiving
                },
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    /// Try to receive the next event without blocking.
    ///
    /// Returns `Some(event)` if an event is available, or `None` if no event
    /// is available or the channel is closed.
    pub fn try_recv(&mut self) -> Option<Arc<AstridEvent>> {
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    if self.matches(&event) {
                        return Some(event);
                    }
                },
                Err(broadcast::error::TryRecvError::Lagged(count)) => {
                    warn!(skipped = count, "Event receiver lagged, events dropped");
                    self.lagged_count = self.lagged_count.saturating_add(count);
                    // Continue receiving
                },
                Err(
                    broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed,
                ) => return None,
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::similar_names)]
mod tests {
    use super::*;
    use crate::event::EventMetadata;

    #[tokio::test]
    async fn test_event_bus_creation() {
        let bus = EventBus::new();
        assert_eq!(bus.capacity(), DEFAULT_CHANNEL_CAPACITY);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_event_bus_with_capacity() {
        let bus = EventBus::with_capacity(100);
        assert_eq!(bus.capacity(), 100);
    }

    #[tokio::test]
    async fn test_publish_and_receive() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();

        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".to_string(),
        };

        let count = bus.publish(event);
        assert_eq!(count, 1);

        let msg = receiver.recv().await.unwrap();
        assert_eq!(msg.event_type(), "astrid.v1.lifecycle.runtime_started");
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut receiver1 = bus.subscribe();
        let mut receiver2 = bus.subscribe();

        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".to_string(),
        };

        let count = bus.publish(event);
        assert_eq!(count, 2);

        let obj1 = receiver1.recv().await.unwrap();
        let obj2 = receiver2.recv().await.unwrap();

        assert_eq!(obj1.event_type(), "astrid.v1.lifecycle.runtime_started");
        assert_eq!(obj2.event_type(), "astrid.v1.lifecycle.runtime_started");
    }

    #[tokio::test]
    async fn test_no_subscribers() {
        let bus = EventBus::new();

        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".to_string(),
        };

        let count = bus.publish(event);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_try_recv_empty() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();

        let result = receiver.try_recv();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_try_recv_with_event() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();

        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".to_string(),
        };

        bus.publish(event);

        let result = receiver.try_recv();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        let receiver1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _receiver2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(receiver1);
        // Note: subscriber count may not immediately reflect dropped receivers
    }

    #[tokio::test]
    async fn test_cloned_bus_synchronous_subscriber() {
        use crate::subscriber::FilterSubscriber;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let bus = EventBus::new();
        let cloned_bus = bus.clone();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let subscriber = FilterSubscriber::new("test_sync", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Register on the cloned bus
        cloned_bus.registry().register(Arc::new(subscriber));

        // Publish on the original bus
        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".to_string(),
        };
        bus.publish(event);

        // The subscriber registered on the cloned bus should have received it
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_event_bus_drop_cleans_up_registry() {
        use crate::subscriber::FilterSubscriber;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct DropNotify(Arc<AtomicUsize>);
        impl Drop for DropNotify {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let drop_count = Arc::new(AtomicUsize::new(0));
        let drop_count_clone = Arc::clone(&drop_count);

        let notifier = DropNotify(drop_count_clone);
        let bus = EventBus::new();

        let subscriber = FilterSubscriber::new("test_drop", move |_| {
            let _ = &notifier; // Capture notifier so it drops when the subscriber drops
        });

        bus.registry().register(Arc::new(subscriber));

        // The subscriber shouldn't drop until the bus drops
        assert_eq!(drop_count.load(Ordering::SeqCst), 0);

        drop(bus);

        // Dropping the bus should drop the registry, dropping the subscriber, triggering DropNotify
        assert_eq!(drop_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_reentrancy_unregister_from_on_event() {
        use crate::subscriber::{EventSubscriber, SubscriberId};
        use std::sync::Mutex;

        struct UnregisteringSubscriber {
            my_id: Mutex<Option<SubscriberId>>,
        }

        impl EventSubscriber for UnregisteringSubscriber {
            fn on_event(&self, _event: &AstridEvent, bus: &EventBus) {
                let id = self.my_id.lock().unwrap().expect("id not set");
                // This shouldn't deadlock against notify's read lock
                bus.registry().unregister(id);
            }
        }

        let bus = EventBus::new();

        let subscriber = Arc::new(UnregisteringSubscriber {
            my_id: Mutex::new(None),
        });

        let id = bus
            .registry()
            .register(Arc::clone(&subscriber) as Arc<dyn EventSubscriber>);
        *subscriber.my_id.lock().unwrap() = Some(id);

        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".to_string(),
        };

        // This will trigger on_event, which calls unregister.
        bus.publish(event);

        assert_eq!(bus.registry().len(), 0);
    }

    #[tokio::test]
    async fn test_drop_deadlock_publish_from_drop() {
        use crate::subscriber::EventSubscriber;

        struct DroppingSubscriber {
            bus: EventBus,
        }

        impl EventSubscriber for DroppingSubscriber {
            fn on_event(&self, _event: &AstridEvent, _bus: &EventBus) {}
        }

        impl Drop for DroppingSubscriber {
            fn drop(&mut self) {
                let event = AstridEvent::RuntimeStarted {
                    metadata: EventMetadata::new("test"),
                    version: "0.1.0".to_string(),
                };
                // If unregister holds the write lock while dropping us, this will deadlock
                // when notify tries to get the read lock.
                self.bus.publish(event);
            }
        }

        let bus = EventBus::new();

        let id = bus
            .registry()
            .register(Arc::new(DroppingSubscriber { bus: bus.clone() }));

        // This shouldn't deadlock
        bus.registry().unregister(id);
    }

    #[tokio::test]
    async fn test_topic_subscription_exact() {
        let bus = EventBus::new();
        let mut all_receiver = bus.subscribe();
        let mut specific_receiver = bus.subscribe_topic("astrid.cli.input");

        let msg = crate::ipc::IpcMessage::new(
            "astrid.cli.input",
            crate::ipc::IpcPayload::UserInput {
                text: "hello".into(),
                session_id: "default".into(),
                context: None,
            },
            uuid::Uuid::new_v4(),
        );

        let event = AstridEvent::Ipc {
            metadata: EventMetadata::new("test"),
            message: msg,
        };

        bus.publish(event);

        assert!(all_receiver.try_recv().is_some());
        assert!(specific_receiver.try_recv().is_some());

        // Publish to a different topic
        let msg2 = crate::ipc::IpcMessage::new(
            "astrid.telegram.input",
            crate::ipc::IpcPayload::UserInput {
                text: "hello".into(),
                session_id: "default".into(),
                context: None,
            },
            uuid::Uuid::new_v4(),
        );

        let event2 = AstridEvent::Ipc {
            metadata: EventMetadata::new("test"),
            message: msg2,
        };

        bus.publish(event2);

        assert!(all_receiver.try_recv().is_some());
        // Specific receiver should ignore this
        assert!(specific_receiver.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_topic_subscription_wildcard() {
        let bus = EventBus::new();
        // Trailing `*` matches 1+ segments; "astrid.*" is a namespace subscription
        // that matches any topic starting with "astrid." regardless of depth.
        let mut wildcard_receiver = bus.subscribe_topic("astrid.*");

        let msg1 = crate::ipc::IpcMessage::new(
            "astrid.cli.input",
            crate::ipc::IpcPayload::UserInput {
                text: "hello".into(),
                session_id: "default".into(),
                context: None,
            },
            uuid::Uuid::new_v4(),
        );
        let event1 = AstridEvent::Ipc {
            metadata: EventMetadata::new("test"),
            message: msg1,
        };

        let msg2 = crate::ipc::IpcMessage::new(
            "system.log",
            crate::ipc::IpcPayload::UserInput {
                text: "hello".into(),
                session_id: "default".into(),
                context: None,
            },
            uuid::Uuid::new_v4(),
        );
        let event2 = AstridEvent::Ipc {
            metadata: EventMetadata::new("test"),
            message: msg2,
        };

        bus.publish(event1);
        bus.publish(event2);

        // Should receive the matching one, but not the non-matching one
        let received = wildcard_receiver.try_recv().unwrap();
        if let AstridEvent::Ipc { message, .. } = &*received {
            assert_eq!(message.topic, "astrid.cli.input");
        } else {
            panic!("Expected IPC event");
        }

        assert!(wildcard_receiver.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_topic_subscription_ignores_non_ipc() {
        let bus = EventBus::new();
        let mut specific_receiver = bus.subscribe_topic("astrid.cli.input");

        // Publish a non-IPC event
        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("test"),
            version: "0.1.0".into(),
        };

        bus.publish(event);

        // Specific receiver should strictly ignore non-IPC events
        assert!(specific_receiver.try_recv().is_none());
    }

    /// Helper to create an IPC event with a given topic.
    fn ipc_event(topic: &str) -> AstridEvent {
        AstridEvent::Ipc {
            metadata: EventMetadata::new("test"),
            message: crate::ipc::IpcMessage::new(
                topic,
                crate::ipc::IpcPayload::UserInput {
                    text: "x".into(),
                    session_id: "default".into(),
                    context: None,
                },
                uuid::Uuid::new_v4(),
            ),
        }
    }

    #[tokio::test]
    async fn test_wildcard_matches_multiple_depths() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe_topic("astrid.v1.request.*");

        // 4 segments: should match (1 segment after prefix)
        bus.publish(ipc_event("astrid.v1.request.list_capsules"));
        assert!(receiver.try_recv().is_some());

        // 5 segments: should also match (trailing * = 1+ segments)
        bus.publish(ipc_event("astrid.v1.request.foo.bar"));
        assert!(receiver.try_recv().is_some());

        // 3 segments (fewer than prefix + 1): should NOT match
        bus.publish(ipc_event("astrid.v1.request"));
        assert!(receiver.try_recv().is_none());

        // Different prefix: should NOT match
        bus.publish(ipc_event("system.v1.request.foo"));
        assert!(receiver.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_wildcard_rejects_deep_topics() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe_topic("a.*");

        // 21 segments: exceeds MAX_TOPIC_DEPTH of 20
        let deep = (0..21)
            .map(|i| format!("s{i}"))
            .collect::<Vec<_>>()
            .join(".");
        let topic = format!("a.{deep}");
        bus.publish(ipc_event(&topic));
        assert!(receiver.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_middle_wildcard_matches_one_segment() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe_topic("astrid.*.input");

        // Exact match with one middle segment
        bus.publish(ipc_event("astrid.cli.input"));
        assert!(receiver.try_recv().is_some());

        // Different middle segment also matches
        bus.publish(ipc_event("astrid.telegram.input"));
        assert!(receiver.try_recv().is_some());

        // Wrong last segment: should NOT match
        bus.publish(ipc_event("astrid.cli.output"));
        assert!(receiver.try_recv().is_none());

        // Extra segment: should NOT match (segment count mismatch)
        bus.publish(ipc_event("astrid.cli.sub.input"));
        assert!(receiver.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_drain_lagged_initially_zero() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        assert_eq!(receiver.drain_lagged(), 0);
    }

    #[tokio::test]
    async fn test_drain_lagged_resets_after_read() {
        // Use a tiny channel so we can force lag easily.
        let bus = EventBus::with_capacity(2);
        let mut receiver = bus.subscribe();

        // Publish 5 events into a capacity-2 channel — the receiver will lag.
        for i in 0..5 {
            let event = AstridEvent::RuntimeStarted {
                metadata: EventMetadata::new("test"),
                version: format!("{i}"),
            };
            bus.publish(event);
        }

        // try_recv will encounter the Lagged error and accumulate it.
        let _ = receiver.try_recv();

        let lagged = receiver.drain_lagged();
        assert!(lagged > 0, "expected lag count > 0, got {lagged}");

        // Second drain should be zero — it was reset.
        assert_eq!(receiver.drain_lagged(), 0);
    }

    #[tokio::test]
    async fn test_drain_lagged_accumulates_across_calls() {
        let bus = EventBus::with_capacity(2);
        let mut receiver = bus.subscribe();

        // First burst: overflow the channel.
        for _ in 0..4 {
            bus.publish(AstridEvent::RuntimeStarted {
                metadata: EventMetadata::new("test"),
                version: "v1".into(),
            });
        }
        // Drain available messages to trigger the Lagged error.
        while receiver.try_recv().is_some() {}

        let lag1 = receiver.drain_lagged();

        // Second burst: overflow again.
        for _ in 0..4 {
            bus.publish(AstridEvent::RuntimeStarted {
                metadata: EventMetadata::new("test"),
                version: "v2".into(),
            });
        }
        while receiver.try_recv().is_some() {}

        let lag2 = receiver.drain_lagged();

        // Both bursts should have caused lag independently.
        assert!(lag1 > 0, "first burst should lag");
        assert!(lag2 > 0, "second burst should lag");
    }

    #[tokio::test]
    async fn test_recv_blocking_with_timeout() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();

        // With no messages, recv should return None after timeout.
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv()).await;

        // Timeout should fire — no messages published.
        assert!(result.is_err(), "expected timeout, got a message");
    }

    #[tokio::test]
    async fn test_recv_blocking_wakes_on_message() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();

        // Spawn a task that publishes after a short delay.
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            bus_clone.publish(AstridEvent::RuntimeStarted {
                metadata: EventMetadata::new("test"),
                version: "wake".into(),
            });
        });

        // recv should wake when the message arrives, well before 5s.
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv()).await;

        assert!(result.is_ok(), "recv should have woken up");
        let event = result.unwrap().unwrap();
        assert_eq!(event.event_type(), "astrid.v1.lifecycle.runtime_started");
    }

    #[tokio::test]
    async fn test_try_recv_drains_burst() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();

        // Publish 10 messages in a burst.
        for i in 0..10 {
            bus.publish(AstridEvent::RuntimeStarted {
                metadata: EventMetadata::new("test"),
                version: format!("{i}"),
            });
        }

        // Drain all with try_recv.
        let mut count = 0;
        while receiver.try_recv().is_some() {
            count += 1;
        }
        assert_eq!(count, 10);

        // No more messages.
        assert!(receiver.try_recv().is_none());
    }

    #[tokio::test]
    async fn ipc_bus_mints_root_and_preserves_explicit_agent_response_trace() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let session_id = uuid::Uuid::new_v4().to_string();
        let input = crate::ipc::IpcMessage::new(
            "user.v1.prompt",
            crate::ipc::IpcPayload::UserInput {
                text: "hello".into(),
                session_id: session_id.clone(),
                context: None,
            },
            uuid::Uuid::new_v4(),
        )
        .with_principal("operator");
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("socket"),
            message: input,
        });
        let root_event = receiver.recv().await.unwrap();
        let AstridEvent::Ipc {
            metadata,
            message: root_message,
        } = &*root_event
        else {
            panic!("expected IPC event");
        };
        let root = root_message.trace.as_ref().unwrap().clone();
        assert!(root.parent_span_id.is_none());
        assert_eq!(root.session_id.as_deref(), Some(session_id.as_str()));
        assert_eq!(metadata.correlation_id, Some(root.trace_id));
        assert_eq!(
            metadata.session_id.map(|id| id.to_string()),
            Some(session_id.clone())
        );
        assert_eq!(root_message.principal.as_deref(), Some("operator"));

        let response = crate::ipc::IpcMessage::new(
            "agent.v1.response",
            crate::ipc::IpcPayload::AgentResponse {
                text: "hi".into(),
                is_final: true,
                session_id,
                response_provenance: None,
            },
            uuid::Uuid::new_v4(),
        )
        .with_principal("astrid")
        .with_trace(root.child());
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("react"),
            message: response,
        });
        let response_event = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { metadata, message } = &*response_event else {
            panic!("expected IPC event");
        };
        let child = message.trace.as_ref().unwrap();
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
        assert_eq!(metadata.correlation_id, Some(root.trace_id));
        assert_eq!(message.principal.as_deref(), Some("astrid"));
    }

    #[tokio::test]
    async fn ipc_bus_keeps_concurrent_sessions_isolated() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let sessions = ["session-one", "session-two"];
        let mut roots = BTreeMap::new();

        for session_id in sessions {
            bus.publish(AstridEvent::Ipc {
                metadata: EventMetadata::new("socket"),
                message: crate::ipc::IpcMessage::new(
                    "user.v1.prompt",
                    crate::ipc::IpcPayload::UserInput {
                        text: session_id.into(),
                        session_id: session_id.into(),
                        context: None,
                    },
                    uuid::Uuid::new_v4(),
                ),
            });
            let event = receiver.recv().await.unwrap();
            let AstridEvent::Ipc { message, .. } = &*event else {
                panic!("expected IPC event");
            };
            roots.insert(session_id, message.trace.as_ref().unwrap().clone());
        }

        for session_id in sessions.into_iter().rev() {
            bus.publish(AstridEvent::Ipc {
                metadata: EventMetadata::new("react"),
                message: crate::ipc::IpcMessage::new(
                    "agent.v1.response",
                    crate::ipc::IpcPayload::AgentResponse {
                        text: session_id.into(),
                        is_final: true,
                        session_id: session_id.into(),
                        response_provenance: None,
                    },
                    uuid::Uuid::new_v4(),
                )
                .with_trace(roots[session_id].child()),
            });
            let event = receiver.recv().await.unwrap();
            let AstridEvent::Ipc { message, .. } = &*event else {
                panic!("expected IPC event");
            };
            assert_eq!(
                message.trace.as_ref().unwrap().trace_id,
                roots[session_id].trace_id
            );
        }
    }

    #[tokio::test]
    async fn late_same_session_response_never_inherits_the_newest_turn() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let session_id = "reused-session";
        let mut roots = Vec::new();

        for text in ["old turn", "new turn"] {
            bus.publish(AstridEvent::Ipc {
                metadata: EventMetadata::new("socket"),
                message: crate::ipc::IpcMessage::new(
                    "user.v1.prompt",
                    crate::ipc::IpcPayload::UserInput {
                        text: text.into(),
                        session_id: session_id.into(),
                        context: None,
                    },
                    uuid::Uuid::new_v4(),
                ),
            });
            let event = receiver.recv().await.unwrap();
            let AstridEvent::Ipc { message, .. } = &*event else {
                panic!("expected IPC event");
            };
            roots.push(message.trace.as_ref().unwrap().clone());
        }
        assert_ne!(roots[0].trace_id, roots[1].trace_id);

        let late_response = || {
            crate::ipc::IpcMessage::new(
                "agent.v1.response",
                crate::ipc::IpcPayload::AgentResponse {
                    text: "late old response".into(),
                    is_final: true,
                    session_id: session_id.into(),
                    response_provenance: None,
                },
                uuid::Uuid::new_v4(),
            )
        };
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("react"),
            message: late_response(),
        });
        let untraced = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*untraced else {
            panic!("expected IPC event");
        };
        assert!(message.trace.is_none());

        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("react"),
            message: late_response().with_trace(roots[0].child()),
        });
        let explicitly_old = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*explicitly_old else {
            panic!("expected IPC event");
        };
        assert_eq!(message.trace.as_ref().unwrap().trace_id, roots[0].trace_id);
        assert_ne!(message.trace.as_ref().unwrap().trace_id, roots[1].trace_id);
    }

    #[tokio::test]
    async fn ipc_bus_correlates_tool_result_only_by_exact_call_id() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let root = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session", None);
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("react"),
            message: crate::ipc::IpcMessage::new(
                "tool.v1.execute",
                crate::ipc::IpcPayload::ToolExecuteRequest {
                    call_id: "call-one".into(),
                    tool_name: "web_search".into(),
                    arguments: serde_json::json!({"query": "echo state network"}),
                },
                uuid::Uuid::new_v4(),
            )
            .with_trace(root.clone()),
        });
        let _ = receiver.recv().await.unwrap();

        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("web"),
            message: crate::ipc::IpcMessage::new(
                "tool.v1.result",
                crate::ipc::IpcPayload::ToolExecuteResult {
                    call_id: "call-other".into(),
                    result: crate::llm::ToolCallResult::success("call-other", "unrelated"),
                },
                uuid::Uuid::new_v4(),
            ),
        });
        let unrelated = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*unrelated else {
            panic!("expected IPC event");
        };
        assert!(message.trace.is_none());

        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("web"),
            message: crate::ipc::IpcMessage::new(
                "tool.v1.result",
                crate::ipc::IpcPayload::ToolExecuteResult {
                    call_id: "call-one".into(),
                    result: crate::llm::ToolCallResult::success("call-one", "bounded result"),
                },
                uuid::Uuid::new_v4(),
            ),
        });
        let correlated = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*correlated else {
            panic!("expected IPC event");
        };
        let child = message.trace.as_ref().unwrap();
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
    }

    #[tokio::test]
    async fn malformed_trace_is_not_propagated() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let mut malformed =
            crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session", None);
        malformed.schema_version = 99;
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("unknown"),
            message: crate::ipc::IpcMessage::new(
                "agent.v1.response",
                crate::ipc::IpcPayload::AgentResponse {
                    text: "unattributed".into(),
                    is_final: true,
                    session_id: "session".into(),
                    response_provenance: None,
                },
                uuid::Uuid::new_v4(),
            )
            .with_trace(malformed),
        });
        let event = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*event else {
            panic!("expected IPC event");
        };
        assert!(message.trace.is_none());
    }

    #[tokio::test]
    async fn oversized_user_session_does_not_emit_an_invalid_root_trace() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("cli"),
            message: crate::ipc::IpcMessage::new(
                "user.v1.prompt",
                crate::ipc::IpcPayload::UserInput {
                    text: "hello".into(),
                    session_id: "s".repeat(97),
                    context: None,
                },
                uuid::Uuid::new_v4(),
            ),
        });
        let event = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*event else {
            panic!("expected IPC event");
        };
        assert!(message.trace.is_none());
    }

    fn canonical_final(trace: IpcTraceContextV1) -> AstridEvent {
        let session_id = trace.session_id.clone().unwrap();
        AstridEvent::Ipc {
            metadata: EventMetadata::new("wasm_guest"),
            message: crate::ipc::IpcMessage::new(
                CANONICAL_AGENT_RESPONSE_TOPIC,
                crate::ipc::IpcPayload::AgentResponse {
                    text: "NEXT: LISTEN".to_string(),
                    is_final: true,
                    session_id,
                    response_provenance: Some(crate::ipc::AgentResponseProvenanceV1::ModelAuthored),
                },
                uuid::Uuid::new_v4(),
            )
            .with_trace(trace)
            .with_producer(crate::ipc::IpcProducerV1::new(
                "wasm_capsule",
                REACT_CAPSULE_ID,
            )),
        }
    }

    #[tokio::test]
    async fn local_provider_attempts_attach_atomically_and_take_once() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let trace = crate::ipc::IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            "session-one",
            Some("chain-one".to_string()),
        );
        let failed_request = uuid::Uuid::new_v4();
        let failed_attempt = bus
            .begin_local_provider_request(&trace, failed_request)
            .unwrap();
        assert!(bus.finish_local_provider_request(
            &trace,
            failed_attempt,
            LocalProviderRequestOutcomeV1::Timeout,
            None,
        ));
        let successful_request = uuid::Uuid::new_v4();
        let successful_attempt = bus
            .begin_local_provider_request(&trace, successful_request)
            .unwrap();
        assert!(bus.finish_local_provider_request(
            &trace,
            successful_attempt,
            LocalProviderRequestOutcomeV1::SuccessfulHeaders,
            Some(288_001),
        ));

        bus.publish(canonical_final(trace.clone()));
        let received = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*received else {
            panic!("expected IPC event");
        };
        let metrics = message.local_provider_metrics.as_ref().unwrap();
        assert!(metrics.is_supported());
        assert_eq!(metrics.request_count, 2);
        assert_eq!(metrics.successful_header_count, 1);
        assert_eq!(metrics.requests[0].attempt_id, failed_attempt);
        assert_eq!(metrics.requests[1].attempt_id, successful_attempt);
        assert!(metrics.single_successful_request().is_none());

        bus.publish(canonical_final(trace));
        let duplicate = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*duplicate else {
            panic!("expected IPC event");
        };
        assert!(message.local_provider_metrics.is_none());
    }

    #[tokio::test]
    async fn local_provider_full_turn_keys_allow_next_chain_turns_sharing_trace() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let first = crate::ipc::IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            "session-one",
            Some("chain-one".to_string()),
        );
        let mut second = crate::ipc::IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            "session-one",
            Some("chain-one".to_string()),
        );
        second.trace_id = first.trace_id;
        assert_ne!(first.turn_id, second.turn_id);

        for (trace, latency) in [(&first, 11), (&second, 22)] {
            let attempt = bus
                .begin_local_provider_request(trace, uuid::Uuid::new_v4())
                .unwrap();
            assert!(bus.finish_local_provider_request(
                trace,
                attempt,
                LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                Some(latency),
            ));
        }
        bus.publish(canonical_final(first));
        bus.publish(canonical_final(second));
        for expected_latency in [11, 22] {
            let received = receiver.recv().await.unwrap();
            let AstridEvent::Ipc { message, .. } = &*received else {
                panic!("expected IPC event");
            };
            assert_eq!(
                message
                    .local_provider_metrics
                    .as_ref()
                    .and_then(LocalProviderTurnMetricsV1::single_successful_request)
                    .and_then(|request| request.request_header_latency_ms),
                Some(expected_latency)
            );
        }
    }

    #[tokio::test]
    async fn canonical_final_without_attempt_tombstones_late_requests() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        bus.publish(canonical_final(trace.clone()));
        assert!(
            bus.begin_local_provider_request(&trace, uuid::Uuid::new_v4())
                .is_none()
        );
        bus.publish(canonical_final(trace));
        for _ in 0..2 {
            let received = receiver.recv().await.unwrap();
            let AstridEvent::Ipc { message, .. } = &*received else {
                panic!("expected IPC event");
            };
            assert!(message.local_provider_metrics.is_none());
        }
    }

    #[tokio::test]
    async fn guest_supplied_provider_summary_is_always_stripped() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        let mut event = canonical_final(trace);
        let AstridEvent::Ipc { message, .. } = &mut event else {
            unreachable!();
        };
        message.producer = Some(crate::ipc::IpcProducerV1::new(
            "native_socket_client",
            "spoofed-react",
        ));
        message.local_provider_metrics = Some(LocalProviderTurnMetricsV1::new(
            1,
            1,
            vec![LocalProviderRequestAttemptV1 {
                attempt_id: uuid::Uuid::new_v4(),
                request_id: uuid::Uuid::new_v4(),
                outcome: LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                request_header_latency_ms: Some(1),
            }],
        ));
        bus.publish(event);
        let received = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*received else {
            panic!("expected IPC event");
        };
        assert!(message.local_provider_metrics.is_none());
    }

    #[tokio::test]
    async fn incomplete_attempt_final_is_fail_closed_and_take_once() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        let attempt = bus
            .begin_local_provider_request(&trace, uuid::Uuid::new_v4())
            .unwrap();
        bus.publish(canonical_final(trace.clone()));
        assert!(!bus.finish_local_provider_request(
            &trace,
            attempt,
            LocalProviderRequestOutcomeV1::SuccessfulHeaders,
            Some(1),
        ));
        bus.publish(canonical_final(trace));
        for _ in 0..2 {
            let received = receiver.recv().await.unwrap();
            let AstridEvent::Ipc { message, .. } = &*received else {
                panic!("expected IPC event");
            };
            assert!(message.local_provider_metrics.is_none());
        }
    }

    #[tokio::test]
    async fn supplied_summary_cannot_replace_the_real_host_entry() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        let real_request = uuid::Uuid::new_v4();
        let attempt = bus
            .begin_local_provider_request(&trace, real_request)
            .unwrap();
        assert!(bus.finish_local_provider_request(
            &trace,
            attempt,
            LocalProviderRequestOutcomeV1::SuccessfulHeaders,
            Some(77),
        ));
        let mut event = canonical_final(trace);
        let AstridEvent::Ipc { message, .. } = &mut event else {
            unreachable!();
        };
        message.local_provider_metrics = Some(LocalProviderTurnMetricsV1::new(
            1,
            1,
            vec![LocalProviderRequestAttemptV1 {
                attempt_id: uuid::Uuid::new_v4(),
                request_id: uuid::Uuid::new_v4(),
                outcome: LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                request_header_latency_ms: Some(999),
            }],
        ));
        bus.publish(event);
        let received = receiver.recv().await.unwrap();
        let AstridEvent::Ipc { message, .. } = &*received else {
            panic!("expected IPC event");
        };
        let real = message
            .local_provider_metrics
            .as_ref()
            .and_then(LocalProviderTurnMetricsV1::single_successful_request)
            .unwrap();
        assert_eq!(real.attempt_id, attempt);
        assert_eq!(real.request_id, real_request);
        assert_eq!(real.request_header_latency_ms, Some(77));
    }

    #[tokio::test]
    async fn noncanonical_finals_do_not_take_the_correct_turn_entry() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        let attempt = bus
            .begin_local_provider_request(&trace, uuid::Uuid::new_v4())
            .unwrap();
        assert!(bus.finish_local_provider_request(
            &trace,
            attempt,
            LocalProviderRequestOutcomeV1::SuccessfulHeaders,
            Some(33),
        ));

        let mut wrong_producer = canonical_final(trace.clone());
        let AstridEvent::Ipc { message, .. } = &mut wrong_producer else {
            unreachable!();
        };
        message.producer = Some(crate::ipc::IpcProducerV1::new("wasm_capsule", "other"));
        bus.publish(wrong_producer);

        let mut wrong_topic = canonical_final(trace.clone());
        let AstridEvent::Ipc { message, .. } = &mut wrong_topic else {
            unreachable!();
        };
        message.topic = "agent.v1.other".to_string();
        bus.publish(wrong_topic);

        let mut wrong_session = canonical_final(trace.clone());
        let AstridEvent::Ipc { message, .. } = &mut wrong_session else {
            unreachable!();
        };
        let crate::ipc::IpcPayload::AgentResponse { session_id, .. } = &mut message.payload else {
            unreachable!();
        };
        *session_id = "other-session".to_string();
        bus.publish(wrong_session);

        bus.publish(canonical_final(trace));
        let mut received = Vec::new();
        for _ in 0..4 {
            received.push(receiver.recv().await.unwrap());
        }
        for event in &received[..3] {
            let AstridEvent::Ipc { message, .. } = &**event else {
                panic!("expected IPC event");
            };
            assert!(message.local_provider_metrics.is_none());
        }
        let AstridEvent::Ipc { message, .. } = &*received[3] else {
            panic!("expected IPC event");
        };
        assert_eq!(
            message
                .local_provider_metrics
                .as_ref()
                .and_then(LocalProviderTurnMetricsV1::single_successful_request)
                .and_then(|request| request.request_header_latency_ms),
            Some(33)
        );
    }

    #[test]
    fn expired_active_provider_turn_is_poisoned_not_readmitted() {
        let mut registry = LocalProviderMetricsRegistry::default();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        let now = Instant::now();
        assert!(registry.begin(&trace, uuid::Uuid::new_v4(), now).is_some());
        let expired = now.checked_add(LOCAL_PROVIDER_TURN_TTL).unwrap();
        assert!(
            registry
                .begin(&trace, uuid::Uuid::new_v4(), expired)
                .is_none()
        );
        assert!(registry.take(&trace, expired).is_none());
    }

    #[test]
    fn provider_attempt_cap_poisons_the_whole_turn() {
        let mut registry = LocalProviderMetricsRegistry::default();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        let now = Instant::now();
        for _ in 0..LOCAL_PROVIDER_TURN_METRICS_MAX_ENTRIES {
            let attempt = registry.begin(&trace, uuid::Uuid::new_v4(), now).unwrap();
            assert!(registry.finish(
                &trace,
                attempt,
                LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                Some(1),
                now,
            ));
        }
        assert!(registry.begin(&trace, uuid::Uuid::new_v4(), now).is_none());
        assert!(registry.take(&trace, now).is_none());
    }

    #[test]
    fn provider_registry_capacity_disables_attribution_for_process_lifetime() {
        let mut registry = LocalProviderMetricsRegistry::default();
        let now = Instant::now();
        for index in 0..MAX_LOCAL_PROVIDER_TURNS {
            let key = LocalProviderTurnKey {
                trace_id: uuid::Uuid::from_u128(u128::try_from(index).unwrap().saturating_add(1)),
                turn_id: uuid::Uuid::new_v4(),
                session_id: "session".to_string(),
                chain_id: None,
            };
            registry.claims.insert(
                key,
                LocalProviderTraceClaim {
                    state: LocalProviderTraceState::Taken,
                    touched_at: now,
                },
            );
        }
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-new", None);
        assert!(registry.begin(&trace, uuid::Uuid::new_v4(), now).is_none());
        let much_later = now
            .checked_add(LOCAL_PROVIDER_TURN_TTL.saturating_mul(2))
            .unwrap();
        assert!(
            registry
                .begin(&trace, uuid::Uuid::new_v4(), much_later)
                .is_none()
        );
        assert!(registry.disabled);
    }

    #[test]
    fn poisoned_trace_registry_disables_exact_provider_attribution() {
        let bus = EventBus::new();
        let shared = Arc::clone(&bus.ipc_traces);
        let _ = std::thread::spawn(move || {
            let _guard = shared.lock().unwrap();
            panic!("intentional registry poison");
        })
        .join();
        let trace = crate::ipc::IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session-one", None);
        assert!(
            bus.begin_local_provider_request(&trace, uuid::Uuid::new_v4())
                .is_none()
        );
        assert!(bus.lock_ipc_traces().local_provider_metrics.disabled);
    }
}
