//! Cross-boundary IPC message schemas and payloads.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::agency_corridor::{
    AgencyCorridorPacketV1, AgencyCorridorPacketV2, AgencyCorridorReceiptV1,
    AgencyCorridorReceiptV2, AgencyCorridorStateV1, AgencyProgramReceiptV1, AgencyWorkProgramV1,
    AutonomousWorkQueueV1, AutonomyPrioritySignalV1, EvidencePortfolioV1, QuarantinedPatchBundleV1,
};
use crate::authority::{
    AuthorityBoundaryPacketV1, AuthorityBoundaryPacketV2, AuthorityLifecycleReceiptV2,
    AuthorityLifecycleStateV2, ReplayResultV2,
};

const IPC_TRACE_SCHEMA_VERSION_V1: u8 = 1;
const IPC_TRACE_LABEL_MAX_CHARS: usize = 96;

/// Observational correlation carried across IPC hops.
///
/// Trace metadata is not signed authority, does not grant capabilities, and
/// must never be consulted by policy, approval, or budget gates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcTraceContextV1 {
    /// Trace wire schema version.
    #[serde(default = "default_ipc_trace_schema_version")]
    pub schema_version: u8,
    /// Stable identifier for one causal activity trace.
    pub trace_id: Uuid,
    /// Identifier for one authenticated user/model turn within the trace.
    ///
    /// Like the rest of this structure this value is observational by itself.
    /// Authority consumers must additionally require a kernel-attested producer
    /// or another trusted runtime boundary before using it as a replay key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<Uuid>,
    /// Identifier for this individual IPC or local receipt span.
    pub span_id: Uuid,
    /// Direct parent span, absent only at a trace root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<Uuid>,
    /// Conversation session associated with this trace, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Sovereign Action-chain identifier, when already established.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
}

impl IpcTraceContextV1 {
    /// Create a root trace for a session.
    #[must_use]
    pub fn root(trace_id: Uuid, session_id: impl Into<String>, chain_id: Option<String>) -> Self {
        Self {
            schema_version: IPC_TRACE_SCHEMA_VERSION_V1,
            trace_id,
            turn_id: Some(Uuid::new_v4()),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            session_id: Some(session_id.into()),
            chain_id,
        }
    }

    /// Create a child span while preserving the observational lineage.
    #[must_use]
    pub fn child(&self) -> Self {
        Self {
            schema_version: IPC_TRACE_SCHEMA_VERSION_V1,
            trace_id: self.trace_id,
            turn_id: self.turn_id,
            span_id: Uuid::new_v4(),
            parent_span_id: Some(self.span_id),
            session_id: self.session_id.clone(),
            chain_id: self.chain_id.clone(),
        }
    }

    /// Return whether this trace is a bounded, structurally valid v1 context.
    ///
    /// `turn_id` remains optional so trace records written before the additive
    /// turn identifier was introduced continue to decode as observational
    /// history. When an optional identifier is present, it must not be nil.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.schema_version == IPC_TRACE_SCHEMA_VERSION_V1
            && !self.trace_id.is_nil()
            && !self.span_id.is_nil()
            && self.turn_id.is_none_or(|turn_id| !turn_id.is_nil())
            && self.parent_span_id.is_none_or(|parent_span_id| {
                !parent_span_id.is_nil() && parent_span_id != self.span_id
            })
            && self
                .session_id
                .as_deref()
                .is_none_or(ipc_trace_label_is_supported)
            && self
                .chain_id
                .as_deref()
                .is_none_or(ipc_trace_label_is_supported)
    }
}

fn ipc_trace_label_is_supported(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= IPC_TRACE_LABEL_MAX_CHARS
        && !value.chars().any(char::is_control)
}

const IPC_PRODUCER_SCHEMA_VERSION_V1: u8 = 1;

/// Producer class stamped by the kernel HTTP host on local-provider timing observations.
pub const KERNEL_HTTP_HOST_PRODUCER_KIND: &str = "kernel_host";
/// Producer identifier stamped by the kernel HTTP host on local-provider timing observations.
pub const KERNEL_HTTP_HOST_PRODUCER_ID: &str = "wasm_http_stream";
/// Canonical stderr receipt prefix emitted by supervised headless CLI runs.
pub const HEADLESS_PROVIDER_METRICS_RECEIPT_PREFIX: &str = "[astrid-headless-provider-metrics] ";
/// Maximum number of per-request timings carried on one bounded turn summary.
pub const LOCAL_PROVIDER_TURN_METRICS_MAX_ENTRIES: usize = 16;

/// Terminal outcome of one exact, allowlisted loopback-provider HTTP attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalProviderRequestOutcomeV1 {
    /// Successful HTTP response headers arrived from a verified loopback peer.
    SuccessfulHeaders,
    /// A non-success HTTP status arrived from a verified loopback peer.
    NonSuccessStatus,
    /// Response headers arrived but the transport did not expose its peer address.
    UnknownPeer,
    /// Response headers arrived from a peer that was not loopback.
    NonLoopbackPeer,
    /// The host response-header deadline expired.
    Timeout,
    /// The request failed before response headers arrived.
    TransportError,
    /// Capsule shutdown cancelled the request before response headers arrived.
    Cancelled,
}

/// Bounded terminal record for one exact eligible local-provider request attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalProviderRequestAttemptV1 {
    /// Host-generated identifier distinguishing retries of the same LLM request.
    pub attempt_id: Uuid,
    /// Opaque typed LLM request identifier from the direct caller context.
    pub request_id: Uuid,
    /// Terminal status recorded by the HTTP host on every send return path.
    pub outcome: LocalProviderRequestOutcomeV1,
    /// Exact elapsed time to headers, available only for successful loopback headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header_latency_ms: Option<u64>,
}

impl LocalProviderRequestAttemptV1 {
    /// Return whether identifiers and outcome-specific latency are coherent.
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        !self.attempt_id.is_nil()
            && !self.request_id.is_nil()
            && matches!(
                (self.outcome, self.request_header_latency_ms),
                (LocalProviderRequestOutcomeV1::SuccessfulHeaders, Some(_))
                    | (
                        LocalProviderRequestOutcomeV1::NonSuccessStatus
                            | LocalProviderRequestOutcomeV1::UnknownPeer
                            | LocalProviderRequestOutcomeV1::NonLoopbackPeer
                            | LocalProviderRequestOutcomeV1::Timeout
                            | LocalProviderRequestOutcomeV1::TransportError
                            | LocalProviderRequestOutcomeV1::Cancelled,
                        None
                    )
            )
    }
}

const LOCAL_PROVIDER_TURN_METRICS_SCHEMA_VERSION_V1: u8 = 1;

/// Host-owned, bounded completion summary for one traced local-provider turn.
///
/// The event bus attaches this summary to the same canonical final response message after taking
/// the matching host-only registry entry. A guest-supplied value is always cleared first. A turn
/// exceeding the strict attempt cap is suppressed instead of represented by a partial count.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalProviderTurnMetricsV1 {
    /// Summary wire schema version.
    pub schema_version: u8,
    /// Host boundary that measured the requests.
    pub producer: IpcProducerV1,
    /// Exact number of eligible HTTP send attempts observed for this turn.
    pub request_count: u64,
    /// Exact number of attempts that reached successful headers from a loopback peer.
    pub successful_header_count: u64,
    /// Complete bounded set of exact request attempts and terminal statuses.
    pub requests: Vec<LocalProviderRequestAttemptV1>,
}

impl LocalProviderTurnMetricsV1 {
    /// Construct a host-owned bounded turn summary.
    #[must_use]
    pub fn new(
        request_count: u64,
        successful_header_count: u64,
        requests: Vec<LocalProviderRequestAttemptV1>,
    ) -> Self {
        Self {
            schema_version: LOCAL_PROVIDER_TURN_METRICS_SCHEMA_VERSION_V1,
            producer: IpcProducerV1::new(
                KERNEL_HTTP_HOST_PRODUCER_KIND,
                KERNEL_HTTP_HOST_PRODUCER_ID,
            ),
            request_count,
            successful_header_count,
            requests,
        }
    }

    /// Return whether the count, bounded entries, and producer are an exact supported summary.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        let Ok(request_count) = usize::try_from(self.request_count) else {
            return false;
        };
        self.schema_version == LOCAL_PROVIDER_TURN_METRICS_SCHEMA_VERSION_V1
            && self.producer.is_supported()
            && self.producer.kind == KERNEL_HTTP_HOST_PRODUCER_KIND
            && self.producer.id == KERNEL_HTTP_HOST_PRODUCER_ID
            && request_count > 0
            && self.successful_header_count <= self.request_count
            && request_count <= LOCAL_PROVIDER_TURN_METRICS_MAX_ENTRIES
            && self.requests.len() == request_count
            && self
                .requests
                .iter()
                .all(LocalProviderRequestAttemptV1::is_supported)
            && self.requests.iter().enumerate().all(|(index, request)| {
                self.requests[..index]
                    .iter()
                    .all(|prior| prior.attempt_id != request.attempt_id)
            })
            && u64::try_from(
                self.requests
                    .iter()
                    .filter(|request| {
                        request.outcome == LocalProviderRequestOutcomeV1::SuccessfulHeaders
                    })
                    .count(),
            )
            .is_ok_and(|bounded_successes| bounded_successes == self.successful_header_count)
    }

    /// Return the sole request only when the exact turn count is one.
    #[must_use]
    pub fn single_successful_request(&self) -> Option<&LocalProviderRequestAttemptV1> {
        self.is_supported()
            .then_some(())
            .filter(|()| self.request_count == 1 && self.successful_header_count == 1)
            .and_then(|()| self.requests.first())
    }
}

const HEADLESS_PROVIDER_METRICS_SCHEMA_VERSION_V1: u8 = 1;

/// Canonical per-turn provider metrics receipt emitted by the headless CLI.
///
/// The receipt contains the complete bounded host summary and the already-attested canonical
/// response trace. Scalar latency fields are intentionally absent for multi-request turns. Token
/// counts and generation latency remain absent unless a future kernel boundary measures them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HeadlessProviderMetricsReceiptV1 {
    /// Receipt wire schema version.
    pub schema_version: u8,
    /// Kernel-attested canonical response trace for the measured turn.
    pub trace: IpcTraceContextV1,
    /// Host-owned producer attestation accepted by the headless CLI.
    pub producer: IpcProducerV1,
    /// Exact count of eligible HTTP send attempts for this turn.
    pub request_count: u64,
    /// Exact count of attempts with successful headers from a verified loopback peer.
    pub successful_header_count: u64,
    /// Complete bounded per-request host measurements.
    pub requests: Vec<LocalProviderRequestAttemptV1>,
    /// Sole request identifier, present only when `request_count == 1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Uuid>,
    /// Sole host-dispatch-to-successful-response-header latency, present only for one request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header_latency_ms: Option<u64>,
}

impl HeadlessProviderMetricsReceiptV1 {
    /// Bind an accepted host observation to the canonical response turn.
    #[must_use]
    pub fn new(trace: IpcTraceContextV1, metrics: LocalProviderTurnMetricsV1) -> Self {
        let single = metrics
            .single_successful_request()
            .map(|request| (request.request_id, request.request_header_latency_ms));
        Self {
            schema_version: HEADLESS_PROVIDER_METRICS_SCHEMA_VERSION_V1,
            trace,
            producer: metrics.producer,
            request_count: metrics.request_count,
            successful_header_count: metrics.successful_header_count,
            requests: metrics.requests,
            request_id: single.map(|(request_id, _)| request_id),
            request_header_latency_ms: single.and_then(|(_, elapsed_ms)| elapsed_ms),
        }
    }

    /// Return whether this receipt and its canonical turn trace are structurally supported.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.schema_version == HEADLESS_PROVIDER_METRICS_SCHEMA_VERSION_V1
            && self.trace.is_supported()
            && self.trace.turn_id.is_some()
            && self.trace.session_id.is_some()
            && self.producer.is_supported()
            && self.producer.kind == KERNEL_HTTP_HOST_PRODUCER_KIND
            && self.producer.id == KERNEL_HTTP_HOST_PRODUCER_ID
            && LocalProviderTurnMetricsV1 {
                schema_version: LOCAL_PROVIDER_TURN_METRICS_SCHEMA_VERSION_V1,
                producer: self.producer.clone(),
                request_count: self.request_count,
                successful_header_count: self.successful_header_count,
                requests: self.requests.clone(),
            }
            .is_supported()
            && match self.requests.as_slice() {
                [request] if self.request_count == 1 && self.successful_header_count == 1 => {
                    self.request_id == Some(request.request_id)
                        && self.request_header_latency_ms == request.request_header_latency_ms
                },
                _ => self.request_id.is_none() && self.request_header_latency_ms.is_none(),
            }
    }
}

/// Kernel-attested origin of an IPC message.
///
/// Unlike trace metadata, this field is stamped at a host boundary. Native
/// socket input is always overwritten by the kernel and WASM guests cannot
/// supply it through the guest ABI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcProducerV1 {
    /// Producer-attestation wire schema version.
    #[serde(default = "default_ipc_producer_schema_version")]
    pub schema_version: u8,
    /// Host-owned producer class, such as `wasm_capsule` or
    /// `native_socket_client`.
    pub kind: String,
    /// Host-owned producer identifier.
    pub id: String,
}

impl IpcProducerV1 {
    /// Construct a producer attestation at a trusted host boundary.
    #[must_use]
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            schema_version: IPC_PRODUCER_SCHEMA_VERSION_V1,
            kind: kind.into(),
            id: id.into(),
        }
    }

    /// Return whether this attestation uses the supported schema.
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        self.schema_version == IPC_PRODUCER_SCHEMA_VERSION_V1
    }
}

const fn default_ipc_producer_schema_version() -> u8 {
    IPC_PRODUCER_SCHEMA_VERSION_V1
}

const fn default_ipc_trace_schema_version() -> u8 {
    IPC_TRACE_SCHEMA_VERSION_V1
}

/// A cross-boundary message sent over the event bus between WASM guests and the host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcMessage {
    /// Topic pattern or exact match (e.g., `astrid.cli.input`).
    pub topic: String,
    /// Standardized payload structure.
    pub payload: IpcPayload,
    /// Optional cryptographic signature for stateless verification across a distributed swarm.
    pub signature: Option<Vec<u8>>,
    /// Identifier of the sender plugin or agent.
    pub source_id: Uuid,
    /// Timestamp when the message was dispatched.
    pub timestamp: DateTime<Utc>,
    /// Monotonic sequence number assigned by the event bus at publish time.
    /// Used by the dispatcher to guarantee in-order delivery per capsule.
    #[serde(default)]
    pub seq: u64,
    /// The principal (user identity) this message is acting on behalf of.
    ///
    /// `String` rather than `PrincipalId` because `astrid-types` must not
    /// depend on `astrid-core`. Validation to `PrincipalId` happens at the
    /// kernel boundary. `None` for system events (boot, lifecycle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// Optional observational trace context. It never grants authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<IpcTraceContextV1>,
    /// Optional host-attested producer. This is overwritten at every native
    /// ingress boundary and cannot be supplied by a WASM guest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<IpcProducerV1>,
    /// Host-only local-provider summary attached atomically to a canonical final response.
    ///
    /// The event bus clears this field on every publish before optionally replacing it from its
    /// private take-once registry. It is observational and grants no authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_provider_metrics: Option<LocalProviderTurnMetricsV1>,
}

impl IpcMessage {
    /// Create a new IPC message.
    #[must_use]
    pub fn new(topic: impl Into<String>, payload: IpcPayload, source_id: Uuid) -> Self {
        Self {
            topic: topic.into(),
            payload,
            signature: None,
            source_id,
            timestamp: Utc::now(),
            seq: 0,
            principal: None,
            trace: None,
            producer: None,
            local_provider_metrics: None,
        }
    }

    /// Attach a signature for swarm verification.
    #[must_use]
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Set the acting principal for this message.
    #[must_use]
    pub fn with_principal(mut self, principal: impl Into<String>) -> Self {
        self.principal = Some(principal.into());
        self
    }

    /// Attach observational trace metadata.
    #[must_use]
    pub fn with_trace(mut self, trace: IpcTraceContextV1) -> Self {
        self.trace = Some(trace);
        self
    }

    /// Attach a host-owned producer attestation.
    #[must_use]
    pub fn with_producer(mut self, producer: IpcProducerV1) -> Self {
        self.producer = Some(producer);
        self
    }
}

/// Default session ID for conversations.
fn default_session_id() -> String {
    "default".into()
}

/// Declared origin of a terminal agent response.
///
/// This value is supplied by the producing capsule and is not authority on its
/// own. Authority-sensitive consumers must also verify the producer and turn
/// trace at a trusted host boundary.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentResponseProvenanceV1 {
    /// The terminal bytes are exactly the model's authored response.
    ModelAuthored,
    /// A local non-writing `LISTEN` fallback follows any authored prefix.
    ModelAuthoredWithLocalSafeFallback,
    /// Local code repaired only the layout of one model-authored Action.
    ModelAuthoredWithLocalFormatRepair,
    /// Local executor/runtime code generated the terminal response.
    ExecutorTerminalError,
}

/// Standardized cross-boundary payload schemas.
// Boxing a single payload would change this established public Rust API; keep
// the wire-compatible variants direct until a versioned IPC migration exists.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcPayload {
    /// Raw, arbitrary JSON.
    RawJson(Value),
    /// User input provided via a frontend (CLI, Telegram).
    UserInput {
        /// The raw text input.
        text: String,
        /// Session ID for conversation continuity. Defaults to `"default"`.
        #[serde(default = "default_session_id")]
        session_id: String,
        /// Optional extra context.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<Value>,
    },
    /// A response generated by an agent.
    AgentResponse {
        /// The text output.
        text: String,
        /// True if this is the final response in a chain.
        is_final: bool,
        /// Session ID for multi-session attribution.
        #[serde(default = "default_session_id")]
        session_id: String,
        /// Declared response origin. Absent on legacy producers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        response_provenance: Option<AgentResponseProvenanceV1>,
    },
    /// An interceptor or capsule request for capability approval.
    ApprovalRequired {
        /// Opaque correlation ID.
        request_id: String,
        /// The action being requested (e.g. "git push").
        action: String,
        /// The resource target (e.g. full command string).
        resource: String,
        /// Justification.
        reason: String,
        /// Optional first-class authority-boundary evidence packet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authority_boundary: Option<AuthorityBoundaryPacketV1>,
        /// Optional first-class authority-boundary lifecycle packet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authority_boundary_v2: Option<AuthorityBoundaryPacketV2>,
    },
    /// Response to an [`ApprovalRequired`](IpcPayload::ApprovalRequired).
    ApprovalResponse {
        /// Must match the `request_id` from the originating request.
        request_id: String,
        /// The user's decision.
        decision: String,
        /// Optional reason for the decision.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// Optional authority-boundary packet identifier being answered.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boundary_id: Option<Uuid>,
    },
    /// First-class authority-boundary evidence declaration.
    AuthorityBoundaryDeclared {
        /// Non-approving boundary packet.
        packet: AuthorityBoundaryPacketV1,
    },
    /// First-class V2 authority lifecycle boundary declaration.
    AuthorityBoundaryDeclaredV2 {
        /// Non-approving lifecycle packet.
        packet: AuthorityBoundaryPacketV2,
    },
    /// A V2 lifecycle receipt was recorded.
    AuthorityLifecycleReceiptRecorded {
        /// Typed lifecycle receipt.
        receipt: AuthorityLifecycleReceiptV2,
    },
    /// A replay result was recorded for a V2 lifecycle.
    AuthorityReplayResultRecorded {
        /// Boundary packet id.
        boundary_id: Uuid,
        /// Replay result.
        replay_result: ReplayResultV2,
    },
    /// A V2 lifecycle gate was evaluated.
    AuthorityLifecycleEvaluated {
        /// Boundary packet id.
        boundary_id: Uuid,
        /// Current lifecycle state.
        state: AuthorityLifecycleStateV2,
        /// Whether live execution is eligible now.
        live_eligible_now: bool,
        /// Whether post-change lifecycle closure is complete.
        closure_complete: bool,
        /// Optional bounded reason.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// A post-change being response was requested.
    AuthorityPostChangeResponseRequested {
        /// Boundary packet id.
        boundary_id: Uuid,
        /// Runtime surface.
        surface: String,
        /// Resource or target.
        resource: String,
    },
    /// A post-change being response was recorded.
    AuthorityPostChangeResponseRecorded {
        /// Typed post-change response receipt.
        receipt: AuthorityLifecycleReceiptV2,
    },
    /// First-class non-live agency corridor packet declared.
    AgencyCorridorDeclared {
        /// Non-live corridor packet.
        packet: AgencyCorridorPacketV1,
    },
    /// Agency corridor receipt was recorded.
    AgencyCorridorReceiptRecorded {
        /// Non-live corridor receipt.
        receipt: AgencyCorridorReceiptV1,
    },
    /// Agency corridor state was evaluated.
    AgencyCorridorEvaluated {
        /// Corridor packet id.
        corridor_id: Uuid,
        /// Current corridor state.
        state: AgencyCorridorStateV1,
        /// Bounded reason.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// Corridor packets never grant approval.
        #[serde(default)]
        grants_approval: bool,
        /// Corridor packets never make live execution eligible.
        #[serde(default)]
        live_eligible_now: bool,
    },
    /// First-class non-live agency corridor V2 packet declared.
    AgencyCorridorDeclaredV2 {
        /// Non-live corridor V2 packet.
        packet: AgencyCorridorPacketV2,
    },
    /// Agency corridor V2 receipt was recorded.
    AgencyCorridorReceiptRecordedV2 {
        /// Non-live corridor V2 receipt.
        receipt: AgencyCorridorReceiptV2,
    },
    /// Agency corridor V2 adaptive queue was evaluated.
    AgencyCorridorQueueEvaluated {
        /// Non-live adaptive queue.
        queue: AutonomousWorkQueueV1,
    },
    /// Non-live agency work program was declared.
    AgencyWorkProgramDeclared {
        /// Non-live work program.
        program: AgencyWorkProgramV1,
    },
    /// Non-live evidence portfolio was updated.
    AgencyEvidencePortfolioUpdated {
        /// Evidence portfolio memory unit.
        portfolio: EvidencePortfolioV1,
    },
    /// Non-live quarantined patch bundle was prepared.
    AgencyPatchBundlePrepared {
        /// Quarantined patch bundle artifact.
        bundle: QuarantinedPatchBundleV1,
    },
    /// Non-live autonomy priority was evaluated.
    AgencyPriorityEvaluated {
        /// Deterministic priority signal.
        signal: AutonomyPrioritySignalV1,
    },
    /// Non-live agency program receipt was recorded.
    AgencyProgramReceiptRecorded {
        /// Program receipt.
        receipt: AgencyProgramReceiptV1,
    },
    /// A capsule needs environment variables to be provided by the user.
    OnboardingRequired {
        /// The ID of the capsule requiring onboarding.
        capsule_id: String,
        /// Rich field descriptors for each missing env var.
        fields: Vec<OnboardingField>,
    },
    /// Request an LLM provider capsule to generate a response.
    LlmRequest {
        /// The unique ID of the request, used for routing the response stream back.
        request_id: Uuid,
        /// The requested model name (e.g. "claude-3-5-sonnet").
        model: String,
        /// The conversation history.
        messages: Vec<crate::llm::Message>,
        /// The tools available to the model.
        tools: Vec<crate::llm::LlmToolDefinition>,
        /// The system prompt.
        system: String,
    },
    /// A stream event from an LLM provider capsule.
    LlmStreamEvent {
        /// The unique ID of the request this stream belongs to.
        request_id: Uuid,
        /// The actual stream event (`TokenDelta`, `ToolCallStart`, etc).
        event: crate::llm::StreamEvent,
    },
    /// The final, non-streaming LLM response.
    LlmResponse {
        /// The unique ID of the request this response belongs to.
        request_id: Uuid,
        /// The final response object.
        response: crate::llm::LlmResponse,
    },
    /// Request the Tool Router capsule to execute a tool.
    ToolExecuteRequest {
        /// The unique ID of the tool call.
        call_id: String,
        /// The name of the tool to execute.
        tool_name: String,
        /// The JSON arguments.
        arguments: Value,
    },
    /// The result of a tool execution.
    ToolExecuteResult {
        /// The unique ID of the tool call.
        call_id: String,
        /// The result of the execution.
        result: crate::llm::ToolCallResult,
    },
    /// Request cancellation of in-flight tool executions.
    ToolCancelRequest {
        /// The call IDs of the tool invocations to cancel.
        call_ids: Vec<String>,
    },
    /// A capsule is requesting the user to select from a list of options.
    SelectionRequired {
        /// Opaque ID so the capsule can correlate the response.
        request_id: String,
        /// Title/prompt shown above the list.
        title: String,
        /// The selectable options.
        options: Vec<SelectionOption>,
        /// IPC topic to publish the user's choice back on.
        callback_topic: String,
    },
    /// A lifecycle hook is requesting user input via the `elicit` API.
    ElicitRequest {
        /// Correlation ID.
        request_id: Uuid,
        /// The capsule requesting input.
        capsule_id: String,
        /// Field descriptor reusing the onboarding schema.
        field: OnboardingField,
    },
    /// Response to an [`ElicitRequest`](IpcPayload::ElicitRequest).
    ElicitResponse {
        /// Must match the `request_id` from the originating request.
        request_id: Uuid,
        /// The user's input. `None` if the user cancelled.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        /// For `Array`-type fields, the collected items.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        values: Option<Vec<String>>,
    },
    /// A client has connected.
    Connect,
    /// A client is disconnecting gracefully.
    Disconnect {
        /// Optional reason for disconnection (e.g. "quit", "timeout").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Arbitrary JSON data for unstructured plugins.
    Custom {
        /// Raw data.
        data: Value,
    },
    /// Unrecognized payload type from a newer protocol version.
    #[serde(other)]
    Unknown,
}

impl IpcPayload {
    /// Returns `true` if `tag` matches a known serde variant name.
    #[must_use]
    pub fn is_known_tag(tag: &str) -> bool {
        matches!(
            tag,
            "raw_json"
                | "user_input"
                | "agent_response"
                | "approval_required"
                | "approval_response"
                | "authority_boundary_declared"
                | "authority_boundary_declared_v2"
                | "authority_lifecycle_receipt_recorded"
                | "authority_replay_result_recorded"
                | "authority_lifecycle_evaluated"
                | "authority_post_change_response_requested"
                | "authority_post_change_response_recorded"
                | "agency_corridor_declared"
                | "agency_corridor_receipt_recorded"
                | "agency_corridor_evaluated"
                | "agency_corridor_declared_v2"
                | "agency_corridor_receipt_recorded_v2"
                | "agency_corridor_queue_evaluated"
                | "agency_work_program_declared"
                | "agency_evidence_portfolio_updated"
                | "agency_patch_bundle_prepared"
                | "agency_priority_evaluated"
                | "agency_program_receipt_recorded"
                | "onboarding_required"
                | "llm_request"
                | "llm_stream_event"
                | "llm_response"
                | "tool_execute_request"
                | "tool_execute_result"
                | "tool_cancel_request"
                | "selection_required"
                | "elicit_request"
                | "elicit_response"
                | "connect"
                | "disconnect"
                | "custom"
        )
    }

    /// Deserialize a JSON [`Value`] into an `IpcPayload`, falling back to
    /// [`Custom`](Self::Custom) for unrecognised or missing type tags.
    pub fn from_json_value(data: Value) -> Self {
        let is_known = data
            .get("type")
            .and_then(|v| v.as_str())
            .is_some_and(Self::is_known_tag);

        if is_known {
            serde_json::from_value::<Self>(data.clone()).unwrap_or(Self::Custom { data })
        } else {
            Self::Custom { data }
        }
    }

    /// Serialize only the guest-facing payload data.
    ///
    /// [`Custom`](Self::Custom) and [`RawJson`](Self::RawJson) payloads return
    /// the inner data value directly (no `type` wrapper). Structured variants
    /// return the full tagged serialization.
    ///
    /// # Errors
    ///
    /// Returns `serde_json::Error` if serialization fails.
    pub fn to_guest_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        match self {
            Self::Custom { data } | Self::RawJson(data) => serde_json::to_vec(data),
            other => serde_json::to_vec(other),
        }
    }
}

/// A single option in a `SelectionRequired` picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectionOption {
    /// Machine-readable identifier sent back to the capsule.
    pub id: String,
    /// Human-readable label shown in the picker.
    pub label: String,
    /// Optional description shown alongside the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A field descriptor for capsule onboarding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OnboardingField {
    /// The environment variable key.
    pub key: String,
    /// The prompt shown to the user.
    pub prompt: String,
    /// Optional description for additional context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The input type for this field.
    pub field_type: OnboardingFieldType,
    /// Optional default value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Placeholder hint text shown when the input is empty (e.g. `"sk-..."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

/// The type of input expected for an onboarding field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OnboardingFieldType {
    /// Free-form text input.
    Text,
    /// Masked secret input.
    Secret,
    /// Selection from a fixed set of choices.
    Enum(Vec<String>),
    /// Multi-value array input (user adds items one at a time).
    Array,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_message_signature() {
        let msg = IpcMessage::new(
            "test.topic",
            IpcPayload::AgentResponse {
                text: "hello".into(),
                is_final: true,
                session_id: "default".into(),
                response_provenance: None,
            },
            Uuid::new_v4(),
        );
        assert!(msg.signature.is_none());

        let signed = msg.with_signature(vec![1, 2, 3]);
        assert_eq!(signed.signature, Some(vec![1, 2, 3]));
    }

    #[test]
    fn ipc_message_principal() {
        let msg = IpcMessage::new(
            "test.topic",
            IpcPayload::Custom {
                data: serde_json::json!({}),
            },
            Uuid::new_v4(),
        );
        assert!(msg.principal.is_none());

        let with_principal = msg.with_principal("alice");
        assert_eq!(with_principal.principal.as_deref(), Some("alice"));
    }

    #[test]
    fn ipc_message_principal_serde_roundtrip() {
        let msg = IpcMessage::new(
            "test.topic",
            IpcPayload::Custom {
                data: serde_json::json!({}),
            },
            Uuid::nil(),
        )
        .with_principal("bob");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""principal":"bob""#));

        let parsed: IpcMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.principal.as_deref(), Some("bob"));
    }

    #[test]
    fn ipc_message_principal_absent_in_json() {
        // Messages without principal should deserialize with None.
        let json = r#"{"topic":"t","payload":{"type":"connect"},"source_id":"00000000-0000-0000-0000-000000000000","timestamp":"2024-01-01T00:00:00Z","seq":0}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert!(msg.principal.is_none());
    }

    #[test]
    fn ipc_message_principal_not_serialized_when_none() {
        let msg = IpcMessage::new("test.topic", IpcPayload::Connect, Uuid::nil());
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("principal"));
    }

    #[test]
    fn unknown_type_tag_deserializes_to_unknown() {
        let json = r#"{"type":"future_variant","some_data":42}"#;
        let payload: IpcPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload, IpcPayload::Unknown);
    }

    #[test]
    fn known_variants_unaffected_by_unknown() {
        let payload = IpcPayload::AgentResponse {
            text: "hello".into(),
            is_final: true,
            session_id: "s1".into(),
            response_provenance: Some(AgentResponseProvenanceV1::ModelAuthored),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: IpcPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn agent_response_provenance_is_additive_and_typed() {
        let legacy: IpcPayload = serde_json::from_value(serde_json::json!({
            "type": "agent_response",
            "text": "legacy",
            "is_final": true,
            "session_id": "legacy-session"
        }))
        .unwrap();
        assert!(matches!(
            legacy,
            IpcPayload::AgentResponse {
                response_provenance: None,
                ..
            }
        ));

        let executor_error = IpcPayload::AgentResponse {
            text: "LLM error: unavailable".into(),
            is_final: true,
            session_id: "session".into(),
            response_provenance: Some(AgentResponseProvenanceV1::ExecutorTerminalError),
        };
        let encoded = serde_json::to_value(&executor_error).unwrap();
        assert_eq!(
            encoded["response_provenance"],
            serde_json::json!("executor_terminal_error")
        );
        assert_eq!(
            serde_json::from_value::<IpcPayload>(encoded).unwrap(),
            executor_error
        );
    }

    #[test]
    fn unknown_variant_serializes_as_type_unknown() {
        let json = serde_json::to_string(&IpcPayload::Unknown).unwrap();
        assert_eq!(json, r#"{"type":"unknown"}"#);
    }

    /// Every variant's serialized `type` tag must be recognised by
    /// `is_known_tag`. If a new variant is added without updating the
    /// match arm *and* the representatives list below, this test fails.
    #[test]
    #[allow(clippy::too_many_lines)] // Deliberately centralized exhaustive variant registry.
    fn is_known_tag_covers_all_variants() {
        const EXPECTED_VARIANT_COUNT: usize = 35;
        let packet = crate::authority::AuthorityBoundaryPacketV1::new(
            "test",
            "bridge",
            "retune_pressure",
            "minime://pressure",
            crate::authority::AuthorityClass::MikeOperatorLiveSubstrate,
            "felt anchor",
            "proposed change",
            crate::authority::ReplayCandidateV1 {
                adapter: "manual".to_string(),
                replay_query: "review".to_string(),
                runnable: false,
                authority: "read_only".to_string(),
            },
            "Mike/operator",
            "serde roundtrip",
        );
        let replay_result = crate::authority::ReplayResultV2 {
            replay_id: "replay-1".to_string(),
            adapter: "manual".to_string(),
            classification: crate::authority::ReplayResultClassificationV2::Passed,
            input_refs: vec!["trial-1".to_string()],
            pre_observations: std::collections::BTreeMap::new(),
            post_observations: std::collections::BTreeMap::new(),
            confidence: Some(0.9),
            failure_modes: Vec::new(),
            evidence_refs: vec!["result-card-1".to_string()],
            bounded_summary: "bounded replay passed".to_string(),
            occurred_at: None,
        };
        let scoped_approval = crate::authority::ScopedApprovalV2 {
            approval_id: "approval-1".to_string(),
            scope_kind: crate::authority::ScopedApprovalKindV2::OneShot,
            issued_by: "Mike/operator".to_string(),
            issued_at: None,
            expires_at: None,
            resources: vec!["minime://pressure".to_string()],
            telemetry_conditions: vec![crate::authority::TelemetryConditionV2 {
                signal: "fill_pct".to_string(),
                operator: "<=".to_string(),
                threshold: "0.75".to_string(),
                observed: Some("0.71".to_string()),
                passed: true,
            }],
            consumed: false,
        };
        let receipt = crate::authority::AuthorityLifecycleReceiptV2 {
            receipt_id: "receipt-1".to_string(),
            boundary_id: Uuid::nil(),
            kind: crate::authority::AuthorityLifecycleReceiptKindV2::Approval,
            issued_by: "Mike/operator".to_string(),
            issued_at: None,
            packet_hash: Some("hash".to_string()),
            receipt_hash_refs: Vec::new(),
            bounded_summary: "bounded approval".to_string(),
            evidence_refs: Vec::new(),
            scoped_approval: Some(scoped_approval.clone()),
            replay_result: None,
            right_to_ignore: true,
        };
        let packet_v2 = crate::authority::AuthorityBoundaryPacketV2 {
            boundary_id: Uuid::nil(),
            schema_version: 2,
            source: "test".to_string(),
            surface: "bridge".to_string(),
            action: "retune_pressure".to_string(),
            resource: "minime://pressure".to_string(),
            authority_class: crate::authority::AuthorityClass::MikeOperatorLiveSubstrate,
            lifecycle_state: crate::authority::AuthorityLifecycleStateV2::OperatorApprovalWait,
            felt_report_anchor: "felt anchor".to_string(),
            proposed_change: "change".to_string(),
            evidence_refs: vec!["wi_1".to_string()],
            delta_refs: vec![crate::authority::ExperienceDeltaRefV2 {
                delta_id: Some("delta-1".to_string()),
                delta_hash: None,
                surface: "codec".to_string(),
                kind: "gate".to_string(),
                lane: Some("semantic".to_string()),
            }],
            replay_candidate: crate::authority::ReplayCandidateV1 {
                adapter: "manual".to_string(),
                replay_query: "review".to_string(),
                runnable: false,
                authority: "read_only".to_string(),
            },
            replay_results: vec![replay_result.clone()],
            scoped_approval: Some(scoped_approval),
            rollout_abort_contract: crate::authority::RolloutAbortContractV2 {
                canary_plan: "one-shot".to_string(),
                health_checks: vec!["health".to_string()],
                rollback_path: "rollback".to_string(),
                abort_criteria: vec!["abort".to_string()],
                post_change_response_required: true,
            },
            redaction_profile: crate::authority::RedactionProfileV2::default(),
            lifecycle_receipts: vec![receipt.clone()],
            success_metrics: Vec::new(),
            abort_criteria: Vec::new(),
            who_can_change_it: "Mike/operator".to_string(),
            how_to_test_it: "test".to_string(),
            right_to_ignore: true,
            live_eligible_now: false,
            auto_approved: false,
        };
        let corridor_packet_v2 = crate::agency_corridor::AgencyCorridorPacketV2::non_live(
            "test",
            "astrid",
            crate::agency_corridor::AgencyCorridorActionV1::CompareArtifacts,
            "bounded comparison can continue without live authority",
            "compare artifacts and prepare evidence",
        );
        let corridor_receipt_v2 = crate::agency_corridor::AgencyCorridorReceiptV2 {
            receipt_id: "corridor-receipt-v2-1".to_string(),
            corridor_id: Uuid::nil(),
            lease_id: Some("lease-safe-labs".to_string()),
            step_id: Some("step-1".to_string()),
            action: crate::agency_corridor::AgencyCorridorActionV1::RunSafeLab,
            issued_by: "agency_corridor_v2".to_string(),
            issued_at: None,
            bounded_summary: "bounded safe-lab receipt".to_string(),
            evidence_refs: Vec::new(),
            hash_refs: Vec::new(),
            source_prep_proposal_ref: None,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
            right_to_ignore: true,
        };
        let corridor_queue = crate::agency_corridor::AutonomousWorkQueueV1 {
            queue_id: "queue-v1".to_string(),
            generated_at: None,
            max_steps_per_run: 5,
            steps: Vec::new(),
            blocked_by_live_violation: false,
            live_violation_refs: Vec::new(),
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let priority_signal = crate::agency_corridor::AutonomyPrioritySignalV1 {
            program_id: "program-1".to_string(),
            being_salience_score: 900,
            recurrence_score: 400,
            cross_being_convergence_score: 300,
            stale_age_score: 100,
            safety_readiness_score: 850,
            deterministic_score: 610,
            basis_refs: vec!["corridor-v2".to_string()],
            live_wait_demoted: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let work_program = crate::agency_corridor::AgencyWorkProgramV1 {
            program_id: "program-1".to_string(),
            schema_version: 1,
            being: "astrid".to_string(),
            title: "bounded evidence program".to_string(),
            hypothesis: "safe corridor evidence can accumulate across runs".to_string(),
            goals: vec!["collect evidence".to_string()],
            status: crate::agency_corridor::AgencyWorkProgramStatusV1::Active,
            linked_corridor_ids: vec![Uuid::nil()],
            authority_boundary_ids: Vec::new(),
            work_item_ids: vec!["wi-1".to_string()],
            sandbox_trial_ids: Vec::new(),
            delta_refs: Vec::new(),
            stop_conditions: vec!["live violation".to_string()],
            priority_signal: Some(priority_signal.clone()),
            current_next_action: "update evidence portfolio".to_string(),
            evidence_refs: Vec::new(),
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let portfolio = crate::agency_corridor::EvidencePortfolioV1 {
            portfolio_id: "portfolio-1".to_string(),
            program_id: "program-1".to_string(),
            being: "astrid".to_string(),
            bounded_felt_anchors: vec!["bounded felt anchor".to_string()],
            linked_introspections: Vec::new(),
            linked_results: Vec::new(),
            linked_cards: Vec::new(),
            linked_source_prep: Vec::new(),
            linked_objections: Vec::new(),
            linked_reopens: Vec::new(),
            linked_patch_bundles: Vec::new(),
            current_recommendation: "continue evidence collection".to_string(),
            unknowns: Vec::new(),
            private_refs: Vec::new(),
            hash_refs: Vec::new(),
            closure_state: "open".to_string(),
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let patch_bundle = crate::agency_corridor::QuarantinedPatchBundleV1 {
            bundle_id: "bundle-1".to_string(),
            program_id: "program-1".to_string(),
            surface: "bridge_prompt".to_string(),
            manifest: "review-only bundle".to_string(),
            proposed_diff_artifact_path: "diagnostics/patch_bundles/bundle-1.diff".to_string(),
            files_touched: vec!["capsules/spectral-bridge/src/llm.rs".to_string()],
            tests_to_run: vec!["cargo test".to_string()],
            restart_expected: true,
            restart_debt_note: "later source implementation would need restart".to_string(),
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
            right_to_ignore: true,
        };
        let program_receipt = crate::agency_corridor::AgencyProgramReceiptV1 {
            receipt_id: "program-receipt-1".to_string(),
            program_id: "program-1".to_string(),
            kind: crate::agency_corridor::AgencyProgramReceiptKindV1::PortfolioUpdated,
            issued_by: "agency_corridor_v2".to_string(),
            issued_at: None,
            bounded_summary: "portfolio updated".to_string(),
            evidence_refs: Vec::new(),
            hash_refs: Vec::new(),
            portfolio_id: Some("portfolio-1".to_string()),
            patch_bundle_id: None,
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };

        let representatives: Vec<IpcPayload> = vec![
            IpcPayload::RawJson(serde_json::json!({"key": "val"})),
            IpcPayload::UserInput {
                text: String::new(),
                session_id: "s".into(),
                context: None,
            },
            IpcPayload::AgentResponse {
                text: String::new(),
                is_final: false,
                session_id: "s".into(),
                response_provenance: None,
            },
            IpcPayload::ApprovalRequired {
                request_id: "req-1".into(),
                action: String::new(),
                resource: String::new(),
                reason: String::new(),
                authority_boundary: None,
                authority_boundary_v2: None,
            },
            IpcPayload::ApprovalResponse {
                request_id: "req-1".into(),
                decision: "approve".into(),
                reason: None,
                boundary_id: None,
            },
            IpcPayload::AuthorityBoundaryDeclared { packet },
            IpcPayload::AuthorityBoundaryDeclaredV2 { packet: packet_v2 },
            IpcPayload::AuthorityLifecycleReceiptRecorded {
                receipt: receipt.clone(),
            },
            IpcPayload::AuthorityReplayResultRecorded {
                boundary_id: Uuid::nil(),
                replay_result,
            },
            IpcPayload::AuthorityLifecycleEvaluated {
                boundary_id: Uuid::nil(),
                state: crate::authority::AuthorityLifecycleStateV2::ApprovedManualOnly,
                live_eligible_now: false,
                closure_complete: false,
                reason: Some("bounded reason".to_string()),
            },
            IpcPayload::AuthorityPostChangeResponseRequested {
                boundary_id: Uuid::nil(),
                surface: "bridge".to_string(),
                resource: "minime://pressure".to_string(),
            },
            IpcPayload::AuthorityPostChangeResponseRecorded { receipt },
            IpcPayload::AgencyCorridorDeclared {
                packet: crate::agency_corridor::AgencyCorridorPacketV1::evidence_only(
                    "test",
                    "astrid",
                    crate::agency_corridor::AgencyCorridorActionV1::EmitClosureObjection,
                    "closure still feels unresolved",
                    "record non-live objection evidence",
                ),
            },
            IpcPayload::AgencyCorridorReceiptRecorded {
                receipt: crate::agency_corridor::AgencyCorridorReceiptV1 {
                    receipt_id: "corridor-receipt-1".to_string(),
                    corridor_id: Uuid::nil(),
                    action: crate::agency_corridor::AgencyCorridorActionV1::RunSafeLab,
                    issued_by: "agency_corridor_v1".to_string(),
                    issued_at: None,
                    bounded_summary: "bounded safe lab receipt".to_string(),
                    evidence_refs: Vec::new(),
                    hash_refs: Vec::new(),
                    grants_approval: false,
                    live_eligible_now: false,
                    right_to_ignore: true,
                },
            },
            IpcPayload::AgencyCorridorEvaluated {
                corridor_id: Uuid::nil(),
                state: crate::agency_corridor::AgencyCorridorStateV1::EvidenceOnly,
                reason: Some("non-live corridor evidence".to_string()),
                grants_approval: false,
                live_eligible_now: false,
            },
            IpcPayload::AgencyCorridorDeclaredV2 {
                packet: corridor_packet_v2,
            },
            IpcPayload::AgencyCorridorReceiptRecordedV2 {
                receipt: corridor_receipt_v2,
            },
            IpcPayload::AgencyCorridorQueueEvaluated {
                queue: corridor_queue,
            },
            IpcPayload::AgencyWorkProgramDeclared {
                program: work_program,
            },
            IpcPayload::AgencyEvidencePortfolioUpdated { portfolio },
            IpcPayload::AgencyPatchBundlePrepared {
                bundle: patch_bundle,
            },
            IpcPayload::AgencyPriorityEvaluated {
                signal: priority_signal,
            },
            IpcPayload::AgencyProgramReceiptRecorded {
                receipt: program_receipt,
            },
            IpcPayload::OnboardingRequired {
                capsule_id: String::new(),
                fields: vec![],
            },
            IpcPayload::LlmRequest {
                request_id: Uuid::nil(),
                model: String::new(),
                messages: vec![],
                tools: vec![],
                system: String::new(),
            },
            IpcPayload::LlmStreamEvent {
                request_id: Uuid::nil(),
                event: crate::llm::StreamEvent::TextDelta(String::new()),
            },
            IpcPayload::LlmResponse {
                request_id: Uuid::nil(),
                response: crate::llm::LlmResponse {
                    message: crate::llm::Message {
                        role: crate::llm::MessageRole::Assistant,
                        content: crate::llm::MessageContent::Text(String::new()),
                    },
                    has_tool_calls: false,
                    stop_reason: crate::llm::StopReason::EndTurn,
                    usage: crate::llm::Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                    },
                },
            },
            IpcPayload::ToolExecuteRequest {
                call_id: String::new(),
                tool_name: String::new(),
                arguments: Value::Null,
            },
            IpcPayload::ToolExecuteResult {
                call_id: String::new(),
                result: crate::llm::ToolCallResult {
                    call_id: String::new(),
                    content: String::new(),
                    is_error: false,
                },
            },
            IpcPayload::SelectionRequired {
                request_id: String::new(),
                title: String::new(),
                options: vec![],
                callback_topic: String::new(),
            },
            IpcPayload::ElicitRequest {
                request_id: Uuid::nil(),
                capsule_id: String::new(),
                field: OnboardingField {
                    key: String::new(),
                    prompt: String::new(),
                    description: None,
                    field_type: OnboardingFieldType::Text,
                    default: None,
                    placeholder: None,
                },
            },
            IpcPayload::ElicitResponse {
                request_id: Uuid::nil(),
                value: None,
                values: None,
            },
            IpcPayload::Connect,
            IpcPayload::Disconnect { reason: None },
            IpcPayload::Custom {
                data: Value::Object(serde_json::Map::new()),
            },
        ];

        assert_eq!(
            representatives.len(),
            EXPECTED_VARIANT_COUNT,
            "IpcPayload variant count changed. Update the representatives list \
             and bump EXPECTED_VARIANT_COUNT."
        );

        for variant in &representatives {
            let json = serde_json::to_value(variant).unwrap();
            let tag = json["type"]
                .as_str()
                .unwrap_or_else(|| panic!("variant {variant:?} has no `type` tag"));
            assert!(
                IpcPayload::is_known_tag(tag),
                "is_known_tag does not recognise tag '{tag}' from variant {variant:?}"
            );
        }
    }

    #[test]
    fn is_known_tag_rejects_unknown_tags() {
        assert!(!IpcPayload::is_known_tag("my_plugin_msg"));
        assert!(!IpcPayload::is_known_tag("unknown"));
        assert!(!IpcPayload::is_known_tag(""));
        assert!(!IpcPayload::is_known_tag("Raw_Json"));
    }

    #[test]
    fn onboarding_field_roundtrip() {
        let field = OnboardingField {
            key: "apiKey".into(),
            prompt: "Enter API key".into(),
            description: None,
            field_type: OnboardingFieldType::Secret,
            default: None,
            placeholder: None,
        };
        let json = serde_json::to_string(&field).unwrap();
        let parsed: OnboardingField = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, field);
    }

    #[test]
    fn onboarding_field_roundtrip_array() {
        let field = OnboardingField {
            key: "relays".into(),
            prompt: "Enter relay URLs".into(),
            description: Some("Nostr relay endpoints".into()),
            field_type: OnboardingFieldType::Array,
            default: None,
            placeholder: None,
        };
        let json = serde_json::to_string(&field).unwrap();
        let parsed: OnboardingField = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, field);
    }

    #[test]
    fn onboarding_required_payload_roundtrip() {
        let payload = IpcPayload::OnboardingRequired {
            capsule_id: "test-capsule".into(),
            fields: vec![
                OnboardingField {
                    key: "network".into(),
                    prompt: "Select network".into(),
                    description: Some("Choose the target network".into()),
                    field_type: OnboardingFieldType::Enum(vec!["testnet".into(), "mainnet".into()]),
                    default: Some("testnet".into()),
                    placeholder: None,
                },
                OnboardingField {
                    key: "apiKey".into(),
                    prompt: "Enter API key".into(),
                    description: None,
                    field_type: OnboardingFieldType::Secret,
                    default: None,
                    placeholder: None,
                },
            ],
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: IpcPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn elicit_request_roundtrip() {
        let payload = IpcPayload::ElicitRequest {
            request_id: Uuid::nil(),
            capsule_id: "my-capsule".into(),
            field: OnboardingField {
                key: "api_url".into(),
                prompt: "Enter API URL".into(),
                description: Some("The backend endpoint".into()),
                field_type: OnboardingFieldType::Text,
                default: Some("https://example.com".into()),
                placeholder: None,
            },
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: IpcPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn elicit_response_roundtrip() {
        let payload = IpcPayload::ElicitResponse {
            request_id: Uuid::nil(),
            value: Some("hello".into()),
            values: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: IpcPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
    }

    #[test]
    fn disconnect_with_reason_roundtrip() {
        let payload = IpcPayload::Disconnect {
            reason: Some("quit".into()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: IpcPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
        assert!(json.contains(r#""type":"disconnect""#), "json: {json}");
    }

    #[test]
    fn disconnect_without_reason_roundtrip() {
        let payload = IpcPayload::Disconnect { reason: None };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: IpcPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, payload);
        assert!(!json.contains("reason"), "json: {json}");
    }

    #[test]
    fn to_guest_bytes_custom_returns_inner_data() {
        let data = serde_json::json!({"session_id": "abc", "messages": []});
        let payload = IpcPayload::Custom { data: data.clone() };
        let bytes = payload.to_guest_bytes().unwrap();
        let roundtrip: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(roundtrip, data);
        assert!(roundtrip.get("type").is_none());
    }

    #[test]
    fn to_guest_bytes_structured_preserves_type_tag() {
        let payload = IpcPayload::UserInput {
            text: "hello".into(),
            session_id: "default".into(),
            context: None,
        };
        let bytes = payload.to_guest_bytes().unwrap();
        let roundtrip: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            roundtrip.get("type").and_then(|v| v.as_str()),
            Some("user_input")
        );
    }

    #[test]
    fn to_guest_bytes_raw_json_unwraps() {
        let inner = serde_json::json!({"key": "value"});
        let payload = IpcPayload::RawJson(inner.clone());
        let bytes = payload.to_guest_bytes().unwrap();
        let roundtrip: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(roundtrip, inner);
        assert!(roundtrip.get("type").is_none());
    }

    #[test]
    fn to_guest_bytes_connect_unit_variant() {
        let payload = IpcPayload::Connect;
        let bytes = payload.to_guest_bytes().unwrap();
        let roundtrip: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            roundtrip.get("type").and_then(|v| v.as_str()),
            Some("connect")
        );
    }

    #[test]
    fn from_json_value_unknown_tag_becomes_custom() {
        let data = serde_json::json!({"type": "my_plugin_msg", "foo": 42});
        let payload = IpcPayload::from_json_value(data.clone());
        assert_eq!(payload, IpcPayload::Custom { data });
    }

    #[test]
    fn from_json_value_known_tag_parses() {
        let data = serde_json::json!({
            "type": "user_input",
            "text": "hi",
            "session_id": "s1"
        });
        let payload = IpcPayload::from_json_value(data);
        assert!(matches!(payload, IpcPayload::UserInput { .. }));
    }

    #[test]
    fn legacy_ipc_message_without_trace_still_decodes() {
        let legacy = serde_json::json!({
            "topic": "user.v1.prompt",
            "payload": {
                "type": "user_input",
                "text": "hello",
                "session_id": "legacy"
            },
            "signature": null,
            "source_id": Uuid::nil(),
            "timestamp": "2026-01-01T00:00:00Z",
            "seq": 0
        });
        let decoded: IpcMessage = serde_json::from_value(legacy).unwrap();
        assert!(decoded.trace.is_none());
        assert!(decoded.producer.is_none());
        assert!(decoded.local_provider_metrics.is_none());
    }

    #[test]
    fn trace_children_preserve_lineage_and_isolate_concurrent_roots() {
        let first =
            IpcTraceContextV1::root(Uuid::new_v4(), "session-one", Some("chain-one".to_string()));
        let second = IpcTraceContextV1::root(Uuid::new_v4(), "session-two", None);
        let child = first.child();

        assert_eq!(child.trace_id, first.trace_id);
        assert_eq!(child.turn_id, first.turn_id);
        assert_eq!(child.parent_span_id, Some(first.span_id));
        assert_eq!(child.session_id.as_deref(), Some("session-one"));
        assert_eq!(child.chain_id.as_deref(), Some("chain-one"));
        assert_ne!(child.span_id, first.span_id);
        assert_ne!(first.trace_id, second.trace_id);
        assert_ne!(first.turn_id, second.turn_id);
    }

    #[test]
    fn legacy_trace_without_turn_id_remains_supported() {
        let trace_id = Uuid::new_v4();
        let span_id = Uuid::new_v4();
        let legacy = serde_json::json!({
            "schema_version": 1,
            "trace_id": trace_id,
            "span_id": span_id,
            "session_id": "legacy-session",
            "chain_id": "legacy-chain"
        });
        let decoded: IpcTraceContextV1 = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.turn_id, None);
        assert!(decoded.is_supported());
    }

    #[test]
    fn trace_validation_rejects_nil_ids_cycles_and_unbounded_labels() {
        let valid = IpcTraceContextV1::root(Uuid::new_v4(), "session", Some("chain".into()));
        assert!(valid.is_supported());

        let mut invalid = valid.clone();
        invalid.trace_id = Uuid::nil();
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.span_id = Uuid::nil();
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.turn_id = Some(Uuid::nil());
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.parent_span_id = Some(Uuid::nil());
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.parent_span_id = Some(invalid.span_id);
        assert!(!invalid.is_supported());

        let mut legacy_compatible = valid.clone();
        legacy_compatible.turn_id = None;
        assert!(legacy_compatible.is_supported());

        let mut invalid = valid.clone();
        invalid.session_id = Some(" \t ".into());
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.chain_id = Some("x".repeat(97));
        assert!(!invalid.is_supported());

        let mut boundary = valid;
        boundary.session_id = Some("s".repeat(96));
        boundary.chain_id = None;
        assert!(boundary.is_supported());
    }

    #[test]
    fn producer_attestation_roundtrips_additively() {
        let message = IpcMessage::new("agent.v1.response", IpcPayload::Connect, Uuid::nil())
            .with_producer(IpcProducerV1::new("wasm_capsule", "astrid-capsule-react"));
        let decoded: IpcMessage =
            serde_json::from_slice(&serde_json::to_vec(&message).unwrap()).unwrap();
        assert_eq!(decoded.producer, message.producer);
        assert!(decoded.producer.as_ref().unwrap().is_supported());
    }

    #[test]
    fn local_provider_latency_contract_is_typed_bounded_and_secret_free() {
        let request_id = Uuid::new_v4();
        let attempt_id = Uuid::new_v4();
        let metrics = LocalProviderTurnMetricsV1::new(
            1,
            1,
            vec![LocalProviderRequestAttemptV1 {
                attempt_id,
                request_id,
                outcome: LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                request_header_latency_ms: Some(288_001),
            }],
        );
        assert!(metrics.is_supported());
        assert_eq!(
            metrics.single_successful_request().unwrap().request_id,
            request_id
        );
        let encoded = serde_json::to_value(&metrics).unwrap();
        assert_eq!(encoded["requests"][0]["request_id"], request_id.to_string());
        assert_eq!(encoded["requests"][0]["request_header_latency_ms"], 288_001);
        assert!(encoded.get("url").is_none());
        assert!(encoded.get("headers").is_none());
        assert!(encoded.get("body").is_none());

        let multi = LocalProviderTurnMetricsV1::new(
            2,
            1,
            vec![
                LocalProviderRequestAttemptV1 {
                    attempt_id: Uuid::new_v4(),
                    request_id: Uuid::new_v4(),
                    outcome: LocalProviderRequestOutcomeV1::TransportError,
                    request_header_latency_ms: None,
                },
                LocalProviderRequestAttemptV1 {
                    attempt_id: Uuid::new_v4(),
                    request_id: Uuid::new_v4(),
                    outcome: LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                    request_header_latency_ms: Some(2),
                },
            ],
        );
        let receipt = HeadlessProviderMetricsReceiptV1::new(
            IpcTraceContextV1::root(Uuid::new_v4(), "session", None),
            multi,
        );
        assert!(receipt.is_supported());
        assert_eq!(receipt.request_count, 2);
        assert!(receipt.request_id.is_none());
        assert!(receipt.request_header_latency_ms.is_none());

        let duplicate_attempt = LocalProviderTurnMetricsV1::new(
            2,
            2,
            vec![
                LocalProviderRequestAttemptV1 {
                    attempt_id,
                    request_id: Uuid::new_v4(),
                    outcome: LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                    request_header_latency_ms: Some(1),
                },
                LocalProviderRequestAttemptV1 {
                    attempt_id,
                    request_id: Uuid::new_v4(),
                    outcome: LocalProviderRequestOutcomeV1::SuccessfulHeaders,
                    request_header_latency_ms: Some(2),
                },
            ],
        );
        assert!(!duplicate_attempt.is_supported());

        let malformed = serde_json::json!({
            "attempt_id": Uuid::new_v4(),
            "request_id": Uuid::new_v4(),
            "outcome": "successful_headers",
            "request_header_latency_ms": 1,
            "url": "http://secret.invalid"
        });
        assert!(serde_json::from_value::<LocalProviderRequestAttemptV1>(malformed).is_err());
    }
}
