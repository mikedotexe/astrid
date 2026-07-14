//! Event types for the Astrid event bus.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use astrid_types::agency_corridor::{
    AgencyCorridorPacketV1, AgencyCorridorPacketV2, AgencyCorridorReceiptV1,
    AgencyCorridorReceiptV2, AgencyCorridorStateV1, AgencyProgramReceiptV1, AgencyWorkProgramV1,
    AutonomousWorkQueueV1, AutonomyPrioritySignalV1, EvidencePortfolioV1, QuarantinedPatchBundleV1,
};
use astrid_types::authority::{
    AuthorityBoundaryPacketV1, AuthorityBoundaryPacketV2, AuthorityGateStateV1,
    AuthorityLifecycleReceiptV2, AuthorityLifecycleStateV2, ReplayResultV2,
};

/// Metadata attached to every event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique event identifier.
    pub event_id: Uuid,
    /// When the event was created.
    pub timestamp: DateTime<Utc>,
    /// Correlation ID for tracing related events.
    pub correlation_id: Option<Uuid>,
    /// Session ID if applicable.
    pub session_id: Option<Uuid>,
    /// User ID if applicable.
    pub user_id: Option<Uuid>,
    /// Source component that generated the event.
    pub source: String,
}

impl EventMetadata {
    /// Create new event metadata.
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: None,
            session_id: None,
            user_id: None,
            source: source.into(),
        }
    }

    /// Set correlation ID.
    #[must_use]
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set session ID.
    #[must_use]
    pub fn with_session_id(mut self, id: Uuid) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Set user ID.
    #[must_use]
    pub fn with_user_id(mut self, id: Uuid) -> Self {
        self.user_id = Some(id);
        self
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new("unknown")
    }
}

/// All events that can occur in the Astrid runtime.
///
/// Always stored behind `Arc` in practice (`EventBus` publishes `Arc<AstridEvent>`),
/// so the variant size difference is acceptable — no heap allocation per event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[expect(clippy::large_enum_variant)]
pub enum AstridEvent {
    // ========== Agent Lifecycle ==========
    /// Runtime started.
    RuntimeStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Runtime version.
        version: String,
    },

    /// Runtime stopped.
    RuntimeStopped {
        /// Event metadata.
        metadata: EventMetadata,
        /// Reason for stopping.
        reason: Option<String>,
    },

    /// Agent started within the runtime.
    AgentStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Agent ID.
        agent_id: Uuid,
        /// Agent name.
        agent_name: String,
    },

    /// Agent stopped.
    AgentStopped {
        /// Event metadata.
        metadata: EventMetadata,
        /// Agent ID.
        agent_id: Uuid,
        /// Reason for stopping.
        reason: Option<String>,
    },

    // ========== Session Events ==========
    /// Session created.
    SessionCreated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Session ID.
        session_id: Uuid,
    },

    /// Session ended.
    SessionEnded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Session ID.
        session_id: Uuid,
        /// Reason for ending.
        reason: Option<String>,
    },

    /// Session resumed from persisted state.
    SessionResumed {
        /// Event metadata.
        metadata: EventMetadata,
        /// Session ID.
        session_id: Uuid,
    },

    // ========== Message Flow ==========
    /// User message received by the runtime.
    MessageReceived {
        /// Event metadata.
        metadata: EventMetadata,
        /// Message ID.
        message_id: Uuid,
        /// Platform the message came from.
        platform: String,
    },

    /// Response message has been delivered to the user/platform.
    ///
    /// Fired after the message is confirmed sent. Useful for auditing,
    /// logging, or triggering post-delivery side effects.
    MessageSent {
        /// Event metadata.
        metadata: EventMetadata,
        /// Message ID.
        message_id: Uuid,
        /// Target platform.
        platform: String,
    },

    /// Message fully processed (response sent).
    MessageProcessed {
        /// Event metadata.
        metadata: EventMetadata,
        /// Message ID.
        message_id: Uuid,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    // ========== Prompt / Cognitive Loop Events ==========
    /// Prompt is being assembled before an LLM call.
    ///
    /// Capsules can inspect or modify the prompt context before it is sent
    /// to the model.
    PromptBuilding {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID correlating to the upcoming LLM call.
        request_id: Uuid,
    },

    /// A response message is about to be sent to the user/platform.
    ///
    /// Allows capsules to intercept or transform outbound messages.
    MessageSending {
        /// Event metadata.
        metadata: EventMetadata,
        /// Message ID.
        message_id: Uuid,
        /// Target platform.
        platform: String,
    },

    /// Context compaction is starting (trimming conversation history).
    ContextCompactionStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Session ID being compacted.
        session_id: Uuid,
        /// Number of messages before compaction.
        message_count: u32,
    },

    /// Context compaction completed.
    ContextCompactionCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Session ID that was compacted.
        session_id: Uuid,
        /// Messages remaining after compaction.
        messages_remaining: u32,
    },

    /// Session is being reset (conversation history cleared).
    SessionResetting {
        /// Event metadata.
        metadata: EventMetadata,
        /// Session ID being reset.
        session_id: Uuid,
    },

    /// Model selection is being resolved before an LLM call.
    ///
    /// Capsules can influence which model/provider is selected for a request.
    ModelResolving {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Candidate provider (may be overridden by capsule).
        provider: Option<String>,
        /// Candidate model (may be overridden by capsule).
        model: Option<String>,
    },

    /// The agent's cognitive loop has finished its run.
    ///
    /// Fired after the final response is produced, before session teardown.
    /// Capsules can inspect the complete run for logging or analytics.
    AgentLoopCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Agent ID.
        agent_id: Uuid,
        /// Total turns in the loop.
        turns: u32,
        /// Duration of the full loop in milliseconds.
        duration_ms: u64,
    },

    /// A tool result is about to be persisted to conversation history.
    ///
    /// Capsules can intercept, redact, or transform the result before
    /// it is stored.
    ToolResultPersisting {
        /// Event metadata.
        metadata: EventMetadata,
        /// Tool call ID.
        call_id: Uuid,
        /// Tool name.
        tool_name: String,
    },

    // ========== LLM Events ==========
    /// LLM request started.
    LlmRequestStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Provider name.
        provider: String,
        /// Model name.
        model: String,
    },

    /// LLM request completed (non-streaming or final).
    LlmRequestCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Whether the request succeeded.
        success: bool,
        /// Input tokens used.
        input_tokens: Option<u32>,
        /// Output tokens used.
        output_tokens: Option<u32>,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// LLM streaming response started.
    LlmStreamStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Model name.
        model: String,
    },

    /// LLM stream chunk received.
    LlmStreamChunk {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Chunk index (0-based).
        chunk_index: u32,
        /// Number of tokens in this chunk.
        token_count: u32,
    },

    /// LLM streaming response completed.
    LlmStreamCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Total input tokens.
        input_tokens: Option<u32>,
        /// Total output tokens.
        output_tokens: Option<u32>,
        /// Total duration in milliseconds.
        duration_ms: u64,
    },

    // ========== Tool Events ==========
    /// Tool call started (generic, any tool source).
    ToolCallStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Tool call ID.
        call_id: Uuid,
        /// Tool name.
        tool_name: String,
        /// Server name (if MCP tool).
        server_name: Option<String>,
    },

    /// Tool call completed successfully.
    ToolCallCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Tool call ID.
        call_id: Uuid,
        /// Tool name.
        tool_name: String,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// Tool call failed.
    ToolCallFailed {
        /// Event metadata.
        metadata: EventMetadata,
        /// Tool call ID.
        call_id: Uuid,
        /// Tool name.
        tool_name: String,
        /// Error message.
        error: String,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    // ========== MCP Events ==========
    /// MCP server connected.
    McpServerConnected {
        /// Event metadata.
        metadata: EventMetadata,
        /// Server name.
        server_name: String,
        /// Protocol version.
        protocol_version: String,
    },

    /// MCP server disconnected.
    McpServerDisconnected {
        /// Event metadata.
        metadata: EventMetadata,
        /// Server name.
        server_name: String,
        /// Reason for disconnection.
        reason: Option<String>,
    },

    /// MCP tool called.
    McpToolCalled {
        /// Event metadata.
        metadata: EventMetadata,
        /// Server name.
        server_name: String,
        /// Tool name.
        tool_name: String,
        /// Tool arguments (may be redacted for security).
        arguments: Option<Value>,
    },

    /// MCP tool completed.
    McpToolCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Server name.
        server_name: String,
        /// Tool name.
        tool_name: String,
        /// Whether the call succeeded.
        success: bool,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    // ========== SubAgent Events ==========
    /// Sub-agent spawned by a parent agent.
    SubAgentSpawned {
        /// Event metadata.
        metadata: EventMetadata,
        /// Sub-agent ID.
        subagent_id: Uuid,
        /// Parent agent ID.
        parent_id: Uuid,
        /// Task description.
        task: String,
        /// Depth in the agent tree.
        depth: u32,
    },

    /// Sub-agent progress update.
    SubAgentProgress {
        /// Event metadata.
        metadata: EventMetadata,
        /// Sub-agent ID.
        subagent_id: Uuid,
        /// Progress message.
        message: String,
    },

    /// Sub-agent completed successfully.
    SubAgentCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Sub-agent ID.
        subagent_id: Uuid,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// Sub-agent failed.
    SubAgentFailed {
        /// Event metadata.
        metadata: EventMetadata,
        /// Sub-agent ID.
        subagent_id: Uuid,
        /// Error message.
        error: String,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// Sub-agent cancelled.
    SubAgentCancelled {
        /// Event metadata.
        metadata: EventMetadata,
        /// Sub-agent ID.
        subagent_id: Uuid,
        /// Reason for cancellation.
        reason: Option<String>,
    },

    // ========== Security Events ==========
    /// Capability granted.
    CapabilityGranted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Capability ID.
        capability_id: Uuid,
        /// Resource being accessed.
        resource: String,
        /// Action being performed.
        action: String,
    },

    /// Capability revoked.
    CapabilityRevoked {
        /// Event metadata.
        metadata: EventMetadata,
        /// Capability ID.
        capability_id: Uuid,
        /// Reason for revocation.
        reason: Option<String>,
    },

    /// Capability check performed.
    CapabilityChecked {
        /// Event metadata.
        metadata: EventMetadata,
        /// Resource being accessed.
        resource: String,
        /// Action being performed.
        action: String,
        /// Whether the check passed.
        allowed: bool,
    },

    /// Authorization denied.
    AuthorizationDenied {
        /// Event metadata.
        metadata: EventMetadata,
        /// Resource being accessed.
        resource: String,
        /// Action being performed.
        action: String,
        /// Reason for denial.
        reason: String,
    },

    /// Security violation detected.
    SecurityViolation {
        /// Event metadata.
        metadata: EventMetadata,
        /// Violation type.
        violation_type: String,
        /// Details of the violation.
        details: String,
    },

    // ========== Approval Events ==========
    /// Approval requested.
    ApprovalRequested {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Resource being accessed.
        resource: String,
        /// Action being performed.
        action: String,
        /// Description of what's being requested.
        description: String,
    },

    /// Approval granted.
    ApprovalGranted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Duration of approval (if limited).
        duration: Option<String>,
    },

    /// Approval denied.
    ApprovalDenied {
        /// Event metadata.
        metadata: EventMetadata,
        /// Request ID.
        request_id: Uuid,
        /// Reason for denial.
        reason: Option<String>,
    },

    /// Authority-boundary evidence packet declared.
    AuthorityBoundaryDeclared {
        /// Event metadata.
        metadata: EventMetadata,
        /// Non-approving boundary packet.
        packet: AuthorityBoundaryPacketV1,
    },

    /// Authority-boundary gate evaluated.
    AuthorityGateEvaluated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Boundary packet ID.
        boundary_id: Uuid,
        /// Current gate state.
        gate_state: AuthorityGateStateV1,
        /// Whether live execution is eligible now. This is expected to remain false in V1.
        live_eligible_now: bool,
        /// Whether the gate auto-approved the action. This is expected to remain false in V1.
        auto_approved: bool,
        /// Bounded reason for the evaluation.
        reason: String,
    },

    /// Authority-boundary lifecycle V2 packet declared.
    AuthorityBoundaryDeclaredV2 {
        /// Event metadata.
        metadata: EventMetadata,
        /// Non-approving lifecycle packet.
        packet: AuthorityBoundaryPacketV2,
    },

    /// Authority lifecycle V2 receipt recorded.
    AuthorityLifecycleReceiptRecorded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Typed lifecycle receipt.
        receipt: AuthorityLifecycleReceiptV2,
    },

    /// Authority replay result recorded.
    AuthorityReplayResultRecorded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Boundary packet ID.
        boundary_id: Uuid,
        /// Replay result.
        replay_result: ReplayResultV2,
    },

    /// Authority lifecycle V2 gate evaluated.
    AuthorityLifecycleEvaluated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Boundary packet ID.
        boundary_id: Uuid,
        /// Current lifecycle state.
        state: AuthorityLifecycleStateV2,
        /// Whether live execution is eligible now.
        live_eligible_now: bool,
        /// Whether post-change closure is complete.
        closure_complete: bool,
        /// Bounded reason for the evaluation.
        reason: String,
    },

    /// Post-change being response requested.
    AuthorityPostChangeResponseRequested {
        /// Event metadata.
        metadata: EventMetadata,
        /// Boundary packet ID.
        boundary_id: Uuid,
        /// Runtime surface.
        surface: String,
        /// Resource or target.
        resource: String,
    },

    /// Post-change being response recorded.
    AuthorityPostChangeResponseRecorded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Typed post-change response receipt.
        receipt: AuthorityLifecycleReceiptV2,
    },

    /// Non-live agency corridor packet declared.
    AgencyCorridorDeclared {
        /// Event metadata.
        metadata: EventMetadata,
        /// Non-live corridor packet.
        packet: AgencyCorridorPacketV1,
    },

    /// Non-live agency corridor receipt recorded.
    AgencyCorridorReceiptRecorded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Non-live corridor receipt.
        receipt: AgencyCorridorReceiptV1,
    },

    /// Non-live agency corridor state evaluated.
    AgencyCorridorEvaluated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Corridor packet ID.
        corridor_id: Uuid,
        /// Current corridor state.
        state: AgencyCorridorStateV1,
        /// Bounded reason for the evaluation.
        reason: String,
        /// Whether the corridor granted approval. Expected to remain false.
        grants_approval: bool,
        /// Whether live execution is eligible now. Expected to remain false.
        live_eligible_now: bool,
    },

    /// Non-live agency corridor V2 packet declared.
    AgencyCorridorDeclaredV2 {
        /// Event metadata.
        metadata: EventMetadata,
        /// Non-live corridor V2 packet.
        packet: AgencyCorridorPacketV2,
    },

    /// Non-live agency corridor V2 receipt recorded.
    AgencyCorridorReceiptRecordedV2 {
        /// Event metadata.
        metadata: EventMetadata,
        /// Non-live corridor V2 receipt.
        receipt: AgencyCorridorReceiptV2,
    },

    /// Non-live agency corridor V2 adaptive queue evaluated.
    AgencyCorridorQueueEvaluated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Adaptive non-live queue.
        queue: AutonomousWorkQueueV1,
    },

    /// Non-live agency work program declared.
    AgencyWorkProgramDeclared {
        /// Event metadata.
        metadata: EventMetadata,
        /// Work program.
        program: AgencyWorkProgramV1,
    },

    /// Non-live agency evidence portfolio updated.
    AgencyEvidencePortfolioUpdated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Evidence portfolio.
        portfolio: EvidencePortfolioV1,
    },

    /// Non-live quarantined patch bundle prepared.
    AgencyPatchBundlePrepared {
        /// Event metadata.
        metadata: EventMetadata,
        /// Quarantined patch bundle.
        bundle: QuarantinedPatchBundleV1,
    },

    /// Non-live autonomy priority signal evaluated.
    AgencyPriorityEvaluated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Priority signal.
        signal: AutonomyPrioritySignalV1,
    },

    /// Non-live agency program receipt recorded.
    AgencyProgramReceiptRecorded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Program receipt.
        receipt: AgencyProgramReceiptV1,
    },

    // ========== Budget Events ==========
    /// Budget allocated for a session or agent.
    BudgetAllocated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Budget ID.
        budget_id: Uuid,
        /// Amount allocated (in smallest currency unit, e.g. cents).
        amount_cents: u64,
        /// Currency code.
        currency: String,
    },

    /// Budget threshold warning.
    BudgetWarning {
        /// Event metadata.
        metadata: EventMetadata,
        /// Budget ID.
        budget_id: Uuid,
        /// Amount remaining (cents).
        remaining_cents: u64,
        /// Percentage used.
        percent_used: f64,
    },

    /// Budget exceeded.
    BudgetExceeded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Budget ID.
        budget_id: Uuid,
        /// Amount over budget (cents).
        overage_cents: u64,
    },

    // ========== Capsule Events ==========
    /// Capsule loaded successfully.
    CapsuleLoaded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Capsule identifier.
        capsule_id: String,
        /// Capsule name.
        capsule_name: String,
    },

    /// Capsule failed to load.
    CapsuleFailed {
        /// Event metadata.
        metadata: EventMetadata,
        /// Capsule identifier.
        capsule_id: String,
        /// Error message.
        error: String,
    },

    /// Capsule unloaded.
    CapsuleUnloaded {
        /// Event metadata.
        metadata: EventMetadata,
        /// Capsule identifier.
        capsule_id: String,
        /// Capsule name.
        capsule_name: String,
    },

    // ========== System Events ==========
    /// Kernel daemon started.
    KernelStarted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Kernel version.
        version: String,
    },

    /// Kernel daemon shutting down.
    KernelShutdown {
        /// Event metadata.
        metadata: EventMetadata,
        /// Reason for shutdown.
        reason: Option<String>,
    },

    /// Configuration reloaded from disk.
    ConfigReloaded {
        /// Event metadata.
        metadata: EventMetadata,
    },

    /// Configuration value changed.
    ConfigChanged {
        /// Event metadata.
        metadata: EventMetadata,
        /// Config key that changed.
        key: String,
    },

    /// Health check completed.
    HealthCheckCompleted {
        /// Event metadata.
        metadata: EventMetadata,
        /// Overall health state.
        healthy: bool,
        /// Number of checks performed.
        checks_performed: u32,
        /// Number of checks that failed.
        checks_failed: u32,
    },

    // ========== Audit Events ==========
    /// Audit entry created.
    AuditEntryCreated {
        /// Event metadata.
        metadata: EventMetadata,
        /// Audit entry ID.
        entry_id: Uuid,
        /// Entry type.
        entry_type: String,
    },

    // ========== Error Events ==========
    /// Error occurred.
    ErrorOccurred {
        /// Event metadata.
        metadata: EventMetadata,
        /// Error code.
        code: String,
        /// Error message.
        message: String,
        /// Stack trace if available.
        stack_trace: Option<String>,
    },

    // ========== IPC Events ==========
    /// An IPC message routed from a WASM guest or host.
    Ipc {
        /// Event metadata.
        metadata: EventMetadata,
        /// The decoded IPC message.
        message: crate::ipc::IpcMessage,
    },

    // ========== Custom Events ==========
    /// Custom event for extensions.
    Custom {
        /// Event metadata.
        metadata: EventMetadata,
        /// Event name.
        name: String,
        /// Event data.
        data: Value,
    },
}

impl AstridEvent {
    /// Get the event metadata.
    #[must_use]
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            Self::RuntimeStarted { metadata, .. }
            | Self::RuntimeStopped { metadata, .. }
            | Self::AgentStarted { metadata, .. }
            | Self::AgentStopped { metadata, .. }
            | Self::SessionCreated { metadata, .. }
            | Self::SessionEnded { metadata, .. }
            | Self::SessionResumed { metadata, .. }
            | Self::PromptBuilding { metadata, .. }
            | Self::MessageSending { metadata, .. }
            | Self::ContextCompactionStarted { metadata, .. }
            | Self::ContextCompactionCompleted { metadata, .. }
            | Self::SessionResetting { metadata, .. }
            | Self::ModelResolving { metadata, .. }
            | Self::AgentLoopCompleted { metadata, .. }
            | Self::ToolResultPersisting { metadata, .. }
            | Self::MessageReceived { metadata, .. }
            | Self::MessageSent { metadata, .. }
            | Self::MessageProcessed { metadata, .. }
            | Self::LlmRequestStarted { metadata, .. }
            | Self::LlmRequestCompleted { metadata, .. }
            | Self::LlmStreamStarted { metadata, .. }
            | Self::LlmStreamChunk { metadata, .. }
            | Self::LlmStreamCompleted { metadata, .. }
            | Self::ToolCallStarted { metadata, .. }
            | Self::ToolCallCompleted { metadata, .. }
            | Self::ToolCallFailed { metadata, .. }
            | Self::McpServerConnected { metadata, .. }
            | Self::McpServerDisconnected { metadata, .. }
            | Self::McpToolCalled { metadata, .. }
            | Self::McpToolCompleted { metadata, .. }
            | Self::SubAgentSpawned { metadata, .. }
            | Self::SubAgentProgress { metadata, .. }
            | Self::SubAgentCompleted { metadata, .. }
            | Self::SubAgentFailed { metadata, .. }
            | Self::SubAgentCancelled { metadata, .. }
            | Self::CapsuleLoaded { metadata, .. }
            | Self::CapsuleFailed { metadata, .. }
            | Self::CapsuleUnloaded { metadata, .. }
            | Self::CapabilityGranted { metadata, .. }
            | Self::CapabilityRevoked { metadata, .. }
            | Self::CapabilityChecked { metadata, .. }
            | Self::AuthorizationDenied { metadata, .. }
            | Self::SecurityViolation { metadata, .. }
            | Self::ApprovalRequested { metadata, .. }
            | Self::ApprovalGranted { metadata, .. }
            | Self::ApprovalDenied { metadata, .. }
            | Self::AuthorityBoundaryDeclared { metadata, .. }
            | Self::AuthorityGateEvaluated { metadata, .. }
            | Self::AuthorityBoundaryDeclaredV2 { metadata, .. }
            | Self::AuthorityLifecycleReceiptRecorded { metadata, .. }
            | Self::AuthorityReplayResultRecorded { metadata, .. }
            | Self::AuthorityLifecycleEvaluated { metadata, .. }
            | Self::AuthorityPostChangeResponseRequested { metadata, .. }
            | Self::AuthorityPostChangeResponseRecorded { metadata, .. }
            | Self::AgencyCorridorDeclared { metadata, .. }
            | Self::AgencyCorridorReceiptRecorded { metadata, .. }
            | Self::AgencyCorridorEvaluated { metadata, .. }
            | Self::AgencyCorridorDeclaredV2 { metadata, .. }
            | Self::AgencyCorridorReceiptRecordedV2 { metadata, .. }
            | Self::AgencyCorridorQueueEvaluated { metadata, .. }
            | Self::AgencyWorkProgramDeclared { metadata, .. }
            | Self::AgencyEvidencePortfolioUpdated { metadata, .. }
            | Self::AgencyPatchBundlePrepared { metadata, .. }
            | Self::AgencyPriorityEvaluated { metadata, .. }
            | Self::AgencyProgramReceiptRecorded { metadata, .. }
            | Self::BudgetAllocated { metadata, .. }
            | Self::BudgetWarning { metadata, .. }
            | Self::BudgetExceeded { metadata, .. }
            | Self::KernelStarted { metadata, .. }
            | Self::KernelShutdown { metadata, .. }
            | Self::ConfigReloaded { metadata, .. }
            | Self::ConfigChanged { metadata, .. }
            | Self::HealthCheckCompleted { metadata, .. }
            | Self::AuditEntryCreated { metadata, .. }
            | Self::ErrorOccurred { metadata, .. }
            | Self::Ipc { metadata, .. }
            | Self::Custom { metadata, .. } => metadata,
        }
    }

    /// Get the event type as a string.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            // Agent Lifecycle
            Self::RuntimeStarted { .. } => "astrid.v1.lifecycle.runtime_started",
            Self::RuntimeStopped { .. } => "astrid.v1.lifecycle.runtime_stopped",
            Self::AgentStarted { .. } => "astrid.v1.lifecycle.agent_started",
            Self::AgentStopped { .. } => "astrid.v1.lifecycle.agent_stopped",
            // Session
            Self::SessionCreated { .. } => "astrid.v1.lifecycle.session_created",
            Self::SessionEnded { .. } => "astrid.v1.lifecycle.session_ended",
            Self::SessionResumed { .. } => "astrid.v1.lifecycle.session_resumed",
            // Prompt / Cognitive Loop
            Self::PromptBuilding { .. } => "astrid.v1.lifecycle.prompt_building",
            Self::MessageSending { .. } => "astrid.v1.lifecycle.message_sending",
            Self::ContextCompactionStarted { .. } => {
                "astrid.v1.lifecycle.context_compaction_started"
            },
            Self::ContextCompactionCompleted { .. } => {
                "astrid.v1.lifecycle.context_compaction_completed"
            },
            Self::SessionResetting { .. } => "astrid.v1.lifecycle.session_resetting",
            Self::ModelResolving { .. } => "astrid.v1.lifecycle.model_resolving",
            Self::AgentLoopCompleted { .. } => "astrid.v1.lifecycle.agent_loop_completed",
            Self::ToolResultPersisting { .. } => "astrid.v1.lifecycle.tool_result_persisting",
            // Message Flow
            Self::MessageReceived { .. } => "astrid.v1.lifecycle.message_received",
            Self::MessageSent { .. } => "astrid.v1.lifecycle.message_sent",
            Self::MessageProcessed { .. } => "astrid.v1.lifecycle.message_processed",
            // LLM
            Self::LlmRequestStarted { .. } => "astrid.v1.lifecycle.llm_request_started",
            Self::LlmRequestCompleted { .. } => "astrid.v1.lifecycle.llm_request_completed",
            Self::LlmStreamStarted { .. } => "astrid.v1.lifecycle.llm_stream_started",
            Self::LlmStreamChunk { .. } => "astrid.v1.lifecycle.llm_stream_chunk",
            Self::LlmStreamCompleted { .. } => "astrid.v1.lifecycle.llm_stream_completed",
            // Tool
            Self::ToolCallStarted { .. } => "astrid.v1.lifecycle.tool_call_started",
            Self::ToolCallCompleted { .. } => "astrid.v1.lifecycle.tool_call_completed",
            Self::ToolCallFailed { .. } => "astrid.v1.lifecycle.tool_call_failed",
            // MCP
            Self::McpServerConnected { .. } => "astrid.v1.lifecycle.mcp_server_connected",
            Self::McpServerDisconnected { .. } => "astrid.v1.lifecycle.mcp_server_disconnected",
            Self::McpToolCalled { .. } => "astrid.v1.lifecycle.mcp_tool_called",
            Self::McpToolCompleted { .. } => "astrid.v1.lifecycle.mcp_tool_completed",
            // SubAgent
            Self::SubAgentSpawned { .. } => "astrid.v1.lifecycle.sub_agent_spawned",
            Self::SubAgentProgress { .. } => "astrid.v1.lifecycle.sub_agent_progress",
            Self::SubAgentCompleted { .. } => "astrid.v1.lifecycle.sub_agent_completed",
            Self::SubAgentFailed { .. } => "astrid.v1.lifecycle.sub_agent_failed",
            Self::SubAgentCancelled { .. } => "astrid.v1.lifecycle.sub_agent_cancelled",
            // Capsule
            Self::CapsuleLoaded { .. } => "astrid.v1.lifecycle.capsule_loaded",
            Self::CapsuleFailed { .. } => "astrid.v1.lifecycle.capsule_failed",
            Self::CapsuleUnloaded { .. } => "astrid.v1.lifecycle.capsule_unloaded",
            // Security
            Self::CapabilityGranted { .. } => "astrid.v1.lifecycle.capability_granted",
            Self::CapabilityRevoked { .. } => "astrid.v1.lifecycle.capability_revoked",
            Self::CapabilityChecked { .. } => "astrid.v1.lifecycle.capability_checked",
            Self::AuthorizationDenied { .. } => "astrid.v1.lifecycle.authorization_denied",
            Self::SecurityViolation { .. } => "astrid.v1.lifecycle.security_violation",
            // Approval
            Self::ApprovalRequested { .. } => "astrid.v1.lifecycle.approval_requested",
            Self::ApprovalGranted { .. } => "astrid.v1.lifecycle.approval_granted",
            Self::ApprovalDenied { .. } => "astrid.v1.lifecycle.approval_denied",
            Self::AuthorityBoundaryDeclared { .. } => {
                "astrid.v1.lifecycle.authority_boundary_declared"
            },
            Self::AuthorityGateEvaluated { .. } => "astrid.v1.lifecycle.authority_gate_evaluated",
            Self::AuthorityBoundaryDeclaredV2 { .. } => {
                "astrid.v2.lifecycle.authority_boundary_declared"
            },
            Self::AuthorityLifecycleReceiptRecorded { .. } => {
                "astrid.v2.lifecycle.authority_receipt_recorded"
            },
            Self::AuthorityReplayResultRecorded { .. } => {
                "astrid.v2.lifecycle.authority_replay_result_recorded"
            },
            Self::AuthorityLifecycleEvaluated { .. } => {
                "astrid.v2.lifecycle.authority_lifecycle_evaluated"
            },
            Self::AuthorityPostChangeResponseRequested { .. } => {
                "astrid.v2.lifecycle.authority_post_change_response_requested"
            },
            Self::AuthorityPostChangeResponseRecorded { .. } => {
                "astrid.v2.lifecycle.authority_post_change_response_recorded"
            },
            Self::AgencyCorridorDeclared { .. } => "astrid.v1.lifecycle.agency_corridor_declared",
            Self::AgencyCorridorReceiptRecorded { .. } => {
                "astrid.v1.lifecycle.agency_corridor_receipt_recorded"
            },
            Self::AgencyCorridorEvaluated { .. } => "astrid.v1.lifecycle.agency_corridor_evaluated",
            Self::AgencyCorridorDeclaredV2 { .. } => "astrid.v2.lifecycle.agency_corridor_declared",
            Self::AgencyCorridorReceiptRecordedV2 { .. } => {
                "astrid.v2.lifecycle.agency_corridor_receipt_recorded"
            },
            Self::AgencyCorridorQueueEvaluated { .. } => {
                "astrid.v2.lifecycle.agency_corridor_queue_evaluated"
            },
            Self::AgencyWorkProgramDeclared { .. } => {
                "astrid.v2.lifecycle.agency_work_program_declared"
            },
            Self::AgencyEvidencePortfolioUpdated { .. } => {
                "astrid.v2.lifecycle.agency_evidence_portfolio_updated"
            },
            Self::AgencyPatchBundlePrepared { .. } => {
                "astrid.v2.lifecycle.agency_patch_bundle_prepared"
            },
            Self::AgencyPriorityEvaluated { .. } => "astrid.v2.lifecycle.agency_priority_evaluated",
            Self::AgencyProgramReceiptRecorded { .. } => {
                "astrid.v2.lifecycle.agency_program_receipt_recorded"
            },
            // Budget
            Self::BudgetAllocated { .. } => "astrid.v1.lifecycle.budget_allocated",
            Self::BudgetWarning { .. } => "astrid.v1.lifecycle.budget_warning",
            Self::BudgetExceeded { .. } => "astrid.v1.lifecycle.budget_exceeded",
            // System
            Self::KernelStarted { .. } => "astrid.v1.lifecycle.kernel_started",
            Self::KernelShutdown { .. } => "astrid.v1.lifecycle.kernel_shutdown",
            Self::ConfigReloaded { .. } => "astrid.v1.lifecycle.config_reloaded",
            Self::ConfigChanged { .. } => "astrid.v1.lifecycle.config_changed",
            Self::HealthCheckCompleted { .. } => "astrid.v1.lifecycle.health_check_completed",
            // Audit
            Self::AuditEntryCreated { .. } => "astrid.v1.lifecycle.audit_entry_created",
            // Error
            Self::ErrorOccurred { .. } => "astrid.v1.lifecycle.error_occurred",
            // IPC
            Self::Ipc { .. } => "ipc",
            // Custom
            Self::Custom { .. } => "custom",
        }
    }

    /// Check if this is a security-related event (test-only).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn is_security_event(&self) -> bool {
        matches!(
            self,
            Self::CapabilityGranted { .. }
                | Self::CapabilityRevoked { .. }
                | Self::CapabilityChecked { .. }
                | Self::AuthorizationDenied { .. }
                | Self::SecurityViolation { .. }
                | Self::ApprovalRequested { .. }
                | Self::ApprovalGranted { .. }
                | Self::ApprovalDenied { .. }
                | Self::AuthorityBoundaryDeclared { .. }
                | Self::AuthorityGateEvaluated { .. }
                | Self::AuthorityBoundaryDeclaredV2 { .. }
                | Self::AuthorityLifecycleReceiptRecorded { .. }
                | Self::AuthorityReplayResultRecorded { .. }
                | Self::AuthorityLifecycleEvaluated { .. }
                | Self::AuthorityPostChangeResponseRequested { .. }
                | Self::AuthorityPostChangeResponseRecorded { .. }
                | Self::AgencyCorridorDeclared { .. }
                | Self::AgencyCorridorReceiptRecorded { .. }
                | Self::AgencyCorridorEvaluated { .. }
                | Self::AgencyCorridorDeclaredV2 { .. }
                | Self::AgencyCorridorReceiptRecordedV2 { .. }
                | Self::AgencyCorridorQueueEvaluated { .. }
                | Self::AgencyWorkProgramDeclared { .. }
                | Self::AgencyEvidencePortfolioUpdated { .. }
                | Self::AgencyPatchBundlePrepared { .. }
                | Self::AgencyPriorityEvaluated { .. }
                | Self::AgencyProgramReceiptRecorded { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrid_types::agency_corridor::{
        AgencyCorridorActionV1, AgencyCorridorPacketV1, AgencyCorridorPacketV2,
        AgencyProgramReceiptKindV1, AgencyProgramReceiptV1, AgencyWorkProgramStatusV1,
        AgencyWorkProgramV1, AutonomousWorkQueueV1, AutonomyPrioritySignalV1, EvidencePortfolioV1,
        QuarantinedPatchBundleV1,
    };
    use astrid_types::authority::{
        AuthorityClass, RedactionProfileV2, ReplayCandidateV1, RolloutAbortContractV2,
    };

    fn authority_packet() -> AuthorityBoundaryPacketV1 {
        AuthorityBoundaryPacketV1::new(
            "event-test",
            "spectral-bridge",
            "retune_live_porosity",
            "minime://control/porosity",
            AuthorityClass::MikeOperatorLiveSubstrate,
            "bounded felt report anchor",
            "propose a live porosity control change",
            ReplayCandidateV1 {
                adapter: "manual_review_v1".to_string(),
                replay_query: "review proposal card evidence".to_string(),
                runnable: false,
                authority: "read_only_review_not_live_control".to_string(),
            },
            "Mike/operator",
            "run sandbox replay before live approval",
        )
    }

    fn authority_packet_v2() -> AuthorityBoundaryPacketV2 {
        AuthorityBoundaryPacketV2 {
            boundary_id: Uuid::nil(),
            schema_version: 2,
            source: "event-test".to_string(),
            surface: "spectral-bridge".to_string(),
            action: "retune_live_porosity".to_string(),
            resource: "minime://control/porosity".to_string(),
            authority_class: AuthorityClass::MikeOperatorLiveSubstrate,
            lifecycle_state: AuthorityLifecycleStateV2::OperatorApprovalWait,
            felt_report_anchor: "bounded felt report anchor".to_string(),
            proposed_change: "propose a live porosity control change".to_string(),
            evidence_refs: vec!["wi_1".to_string()],
            delta_refs: Vec::new(),
            replay_candidate: ReplayCandidateV1 {
                adapter: "manual_review_v1".to_string(),
                replay_query: "review proposal card evidence".to_string(),
                runnable: false,
                authority: "read_only_review_not_live_control".to_string(),
            },
            replay_results: Vec::new(),
            scoped_approval: None,
            rollout_abort_contract: RolloutAbortContractV2 {
                canary_plan: "one shot".to_string(),
                health_checks: vec!["health ok".to_string()],
                rollback_path: "rollback path".to_string(),
                abort_criteria: vec!["abort".to_string()],
                post_change_response_required: true,
            },
            redaction_profile: RedactionProfileV2::default(),
            lifecycle_receipts: Vec::new(),
            success_metrics: Vec::new(),
            abort_criteria: Vec::new(),
            who_can_change_it: "Mike/operator".to_string(),
            how_to_test_it: "run sandbox replay before live approval".to_string(),
            right_to_ignore: true,
            live_eligible_now: false,
            auto_approved: false,
        }
    }

    fn agency_corridor_packet() -> AgencyCorridorPacketV1 {
        AgencyCorridorPacketV1::evidence_only(
            "event-test",
            "astrid",
            AgencyCorridorActionV1::EmitClosureObjection,
            "closure still feels unresolved",
            "record non-live objection evidence",
        )
    }

    fn agency_corridor_packet_v2() -> AgencyCorridorPacketV2 {
        AgencyCorridorPacketV2::non_live(
            "event-test",
            "astrid",
            AgencyCorridorActionV1::CompareArtifacts,
            "artifact comparison can continue without live authority",
            "compare bounded artifacts",
        )
    }

    fn agency_priority_signal() -> AutonomyPrioritySignalV1 {
        AutonomyPrioritySignalV1 {
            program_id: "program-1".to_string(),
            being_salience_score: 900,
            recurrence_score: 500,
            cross_being_convergence_score: 200,
            stale_age_score: 100,
            safety_readiness_score: 850,
            deterministic_score: 620,
            basis_refs: Vec::new(),
            live_wait_demoted: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        }
    }

    fn agency_work_program() -> AgencyWorkProgramV1 {
        AgencyWorkProgramV1 {
            program_id: "program-1".to_string(),
            schema_version: 1,
            being: "astrid".to_string(),
            title: "bounded evidence program".to_string(),
            hypothesis: "safe evidence can accumulate across runs".to_string(),
            goals: Vec::new(),
            status: AgencyWorkProgramStatusV1::Active,
            linked_corridor_ids: vec![Uuid::nil()],
            authority_boundary_ids: Vec::new(),
            work_item_ids: vec!["wi-1".to_string()],
            sandbox_trial_ids: Vec::new(),
            delta_refs: Vec::new(),
            stop_conditions: Vec::new(),
            priority_signal: Some(agency_priority_signal()),
            current_next_action: "update portfolio".to_string(),
            evidence_refs: Vec::new(),
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        }
    }

    fn evidence_portfolio() -> EvidencePortfolioV1 {
        EvidencePortfolioV1 {
            portfolio_id: "portfolio-1".to_string(),
            program_id: "program-1".to_string(),
            being: "astrid".to_string(),
            bounded_felt_anchors: vec!["bounded anchor".to_string()],
            linked_introspections: Vec::new(),
            linked_results: Vec::new(),
            linked_cards: Vec::new(),
            linked_source_prep: Vec::new(),
            linked_objections: Vec::new(),
            linked_reopens: Vec::new(),
            linked_patch_bundles: Vec::new(),
            current_recommendation: "continue safe evidence work".to_string(),
            unknowns: Vec::new(),
            private_refs: Vec::new(),
            hash_refs: Vec::new(),
            closure_state: "open".to_string(),
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        }
    }

    fn patch_bundle() -> QuarantinedPatchBundleV1 {
        QuarantinedPatchBundleV1 {
            bundle_id: "bundle-1".to_string(),
            program_id: "program-1".to_string(),
            surface: "bridge_prompt".to_string(),
            manifest: "review-only patch bundle".to_string(),
            proposed_diff_artifact_path: "diagnostics/bundle-1.diff".to_string(),
            files_touched: Vec::new(),
            tests_to_run: Vec::new(),
            restart_expected: false,
            restart_debt_note: "no restart unless later source implementation occurs".to_string(),
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
            right_to_ignore: true,
        }
    }

    fn program_receipt() -> AgencyProgramReceiptV1 {
        AgencyProgramReceiptV1 {
            receipt_id: "receipt-1".to_string(),
            program_id: "program-1".to_string(),
            kind: AgencyProgramReceiptKindV1::PortfolioUpdated,
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
        }
    }

    #[test]
    fn test_event_metadata_creation() {
        let meta = EventMetadata::new("test_source");
        assert_eq!(meta.source, "test_source");
        assert!(meta.correlation_id.is_none());
        assert!(meta.session_id.is_none());
        assert!(meta.user_id.is_none());
    }

    #[test]
    fn test_event_metadata_builder() {
        let correlation = Uuid::new_v4();
        let session = Uuid::new_v4();
        let user = Uuid::new_v4();

        let meta = EventMetadata::new("test")
            .with_correlation_id(correlation)
            .with_session_id(session)
            .with_user_id(user);

        assert_eq!(meta.correlation_id, Some(correlation));
        assert_eq!(meta.session_id, Some(session));
        assert_eq!(meta.user_id, Some(user));
    }

    #[test]
    fn test_event_type() {
        let event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("runtime"),
            version: "0.1.0".to_string(),
        };
        assert_eq!(event.event_type(), "astrid.v1.lifecycle.runtime_started");

        let boundary_event = AstridEvent::AuthorityBoundaryDeclared {
            metadata: EventMetadata::new("approval"),
            packet: authority_packet(),
        };
        assert_eq!(
            boundary_event.event_type(),
            "astrid.v1.lifecycle.authority_boundary_declared"
        );

        let boundary_event_v2 = AstridEvent::AuthorityBoundaryDeclaredV2 {
            metadata: EventMetadata::new("approval"),
            packet: authority_packet_v2(),
        };
        assert_eq!(
            boundary_event_v2.event_type(),
            "astrid.v2.lifecycle.authority_boundary_declared"
        );

        let corridor_event = AstridEvent::AgencyCorridorDeclared {
            metadata: EventMetadata::new("agency"),
            packet: agency_corridor_packet(),
        };
        assert_eq!(
            corridor_event.event_type(),
            "astrid.v1.lifecycle.agency_corridor_declared"
        );

        let corridor_event_v2 = AstridEvent::AgencyCorridorDeclaredV2 {
            metadata: EventMetadata::new("agency"),
            packet: agency_corridor_packet_v2(),
        };
        assert_eq!(
            corridor_event_v2.event_type(),
            "astrid.v2.lifecycle.agency_corridor_declared"
        );

        let program_event = AstridEvent::AgencyWorkProgramDeclared {
            metadata: EventMetadata::new("agency"),
            program: agency_work_program(),
        };
        assert_eq!(
            program_event.event_type(),
            "astrid.v2.lifecycle.agency_work_program_declared"
        );

        let portfolio_event = AstridEvent::AgencyEvidencePortfolioUpdated {
            metadata: EventMetadata::new("agency"),
            portfolio: evidence_portfolio(),
        };
        assert_eq!(
            portfolio_event.event_type(),
            "astrid.v2.lifecycle.agency_evidence_portfolio_updated"
        );

        let bundle_event = AstridEvent::AgencyPatchBundlePrepared {
            metadata: EventMetadata::new("agency"),
            bundle: patch_bundle(),
        };
        assert_eq!(
            bundle_event.event_type(),
            "astrid.v2.lifecycle.agency_patch_bundle_prepared"
        );

        let priority_event = AstridEvent::AgencyPriorityEvaluated {
            metadata: EventMetadata::new("agency"),
            signal: agency_priority_signal(),
        };
        assert_eq!(
            priority_event.event_type(),
            "astrid.v2.lifecycle.agency_priority_evaluated"
        );

        let receipt_event = AstridEvent::AgencyProgramReceiptRecorded {
            metadata: EventMetadata::new("agency"),
            receipt: program_receipt(),
        };
        assert_eq!(
            receipt_event.event_type(),
            "astrid.v2.lifecycle.agency_program_receipt_recorded"
        );
    }

    #[test]
    fn test_security_event_detection() {
        let security_event = AstridEvent::CapabilityGranted {
            metadata: EventMetadata::new("security"),
            capability_id: Uuid::new_v4(),
            resource: "tool:test".to_string(),
            action: "execute".to_string(),
        };
        assert!(security_event.is_security_event());

        let non_security_event = AstridEvent::RuntimeStarted {
            metadata: EventMetadata::new("runtime"),
            version: "0.1.0".to_string(),
        };
        assert!(!non_security_event.is_security_event());

        let boundary_event = AstridEvent::AuthorityBoundaryDeclared {
            metadata: EventMetadata::new("approval"),
            packet: authority_packet(),
        };
        assert!(boundary_event.is_security_event());

        let gate_event = AstridEvent::AuthorityGateEvaluated {
            metadata: EventMetadata::new("approval"),
            boundary_id: Uuid::new_v4(),
            gate_state: AuthorityGateStateV1::OperatorApprovalWait,
            live_eligible_now: false,
            auto_approved: false,
            reason: "packet evidence declared; manual approval still required".to_string(),
        };
        assert!(gate_event.is_security_event());

        let lifecycle_event = AstridEvent::AuthorityLifecycleEvaluated {
            metadata: EventMetadata::new("approval"),
            boundary_id: Uuid::new_v4(),
            state: AuthorityLifecycleStateV2::ApprovedManualOnly,
            live_eligible_now: false,
            closure_complete: false,
            reason: "bounded V2 lifecycle evaluation".to_string(),
        };
        assert!(lifecycle_event.is_security_event());

        let corridor_event = AstridEvent::AgencyCorridorEvaluated {
            metadata: EventMetadata::new("agency"),
            corridor_id: Uuid::new_v4(),
            state: astrid_types::agency_corridor::AgencyCorridorStateV1::ClosureReopened,
            reason: "bounded non-live reopen".to_string(),
            grants_approval: false,
            live_eligible_now: false,
        };
        assert!(corridor_event.is_security_event());

        let corridor_event_v2 = AstridEvent::AgencyCorridorQueueEvaluated {
            metadata: EventMetadata::new("agency"),
            queue: AutonomousWorkQueueV1 {
                queue_id: "queue-v2".to_string(),
                generated_at: None,
                max_steps_per_run: 5,
                steps: Vec::new(),
                blocked_by_live_violation: false,
                live_violation_refs: Vec::new(),
                grants_approval: false,
                live_eligible_now: false,
                auto_approved: false,
            },
        };
        assert!(corridor_event_v2.is_security_event());

        let program_event = AstridEvent::AgencyWorkProgramDeclared {
            metadata: EventMetadata::new("agency"),
            program: agency_work_program(),
        };
        assert!(program_event.is_security_event());
    }

    #[test]
    fn test_event_serialization() {
        let event = AstridEvent::McpToolCalled {
            metadata: EventMetadata::new("mcp"),
            server_name: "filesystem".to_string(),
            tool_name: "read_file".to_string(),
            arguments: Some(serde_json::json!({"path": "/tmp/test.txt"})),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("mcp_tool_called"));
        assert!(json.contains("filesystem"));
    }
}
