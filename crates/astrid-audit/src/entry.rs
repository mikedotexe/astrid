//! Audit entry types and actions.
//!
//! Every security-relevant operation is recorded as an audit entry.
//! Entries are chain-linked (each contains the hash of the previous)
//! and signed by the runtime.

use astrid_capabilities::AuditEntryId;
use astrid_core::{Permission, SessionId, Timestamp, TokenId};
use astrid_crypto::{ContentHash, KeyPair, PublicKey, Signature};
use astrid_types::agency_corridor::{
    AgencyCorridorActionV1, AgencyCorridorStateV1, AgencyProgramReceiptKindV1,
    AgencyWorkProgramStatusV1,
};
use astrid_types::authority::{
    AuthorityClass, AuthorityGateStateV1, AuthorityLifecycleReceiptKindV2,
    AuthorityLifecycleStateV2, ReplayResultClassificationV2,
};
use serde::{Deserialize, Serialize};

use crate::error::{AuditError, AuditResult};

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry identifier.
    pub id: AuditEntryId,
    /// When this entry was created.
    pub timestamp: Timestamp,
    /// Session this entry belongs to.
    pub session_id: SessionId,
    /// The principal (user identity) this action was performed on behalf of.
    /// `None` for system actions that have no user context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<astrid_core::PrincipalId>,
    /// The action being audited.
    pub action: AuditAction,
    /// Authorization proof for this action.
    pub authorization: AuthorizationProof,
    /// Outcome of the action.
    pub outcome: AuditOutcome,
    /// Hash of the previous entry (chain linking).
    pub previous_hash: ContentHash,
    /// Runtime public key that signed this entry.
    pub runtime_key: PublicKey,
    /// Signature over entry contents.
    pub signature: Signature,
}

impl AuditEntry {
    /// Create a new audit entry (unsigned).
    fn new_unsigned(
        session_id: SessionId,
        action: AuditAction,
        authorization: AuthorizationProof,
        outcome: AuditOutcome,
        previous_hash: ContentHash,
        runtime_key: PublicKey,
    ) -> Self {
        Self {
            id: AuditEntryId::new(),
            timestamp: Timestamp::now(),
            session_id,
            principal: None,
            action,
            authorization,
            outcome,
            previous_hash,
            runtime_key,
            signature: Signature::from_bytes([0u8; 64]), // Placeholder
        }
    }

    /// Create and sign a new audit entry.
    #[must_use]
    pub fn create(
        session_id: SessionId,
        action: AuditAction,
        authorization: AuthorizationProof,
        outcome: AuditOutcome,
        previous_hash: ContentHash,
        runtime_key: &KeyPair,
    ) -> Self {
        let mut entry = Self::new_unsigned(
            session_id,
            action,
            authorization,
            outcome,
            previous_hash,
            runtime_key.export_public_key(),
        );

        let signing_data = entry.signing_data();
        entry.signature = runtime_key.sign(&signing_data);

        entry
    }

    /// Create and sign a new audit entry with a principal.
    ///
    /// Used when audit entries need to record which principal an action
    /// was performed on behalf of. Call sites will be wired when the
    /// kernel audit integration is updated.
    #[must_use]
    pub fn create_with_principal(
        session_id: SessionId,
        principal: astrid_core::PrincipalId,
        action: AuditAction,
        authorization: AuthorizationProof,
        outcome: AuditOutcome,
        previous_hash: ContentHash,
        runtime_key: &KeyPair,
    ) -> Self {
        let mut entry = Self::new_unsigned(
            session_id,
            action,
            authorization,
            outcome,
            previous_hash,
            runtime_key.export_public_key(),
        );
        entry.principal = Some(principal);

        let signing_data = entry.signing_data();
        entry.signature = runtime_key.sign(&signing_data);

        entry
    }

    /// Get the data used for signing.
    #[must_use]
    pub fn signing_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(self.id.0.as_bytes());
        data.extend_from_slice(&self.timestamp.0.timestamp().to_le_bytes());
        data.extend_from_slice(self.session_id.0.as_bytes());
        // Include principal in signing data with length-delimited encoding
        // to prevent ambiguity between None and adjacent field boundaries.
        // 0xFF marker + 4-byte length + bytes for Some, 0x00 marker for None.
        if let Some(ref p) = self.principal {
            let bytes = p.as_str().as_bytes();
            data.push(0xFF); // presence marker
            // PrincipalId is max 64 bytes — safe truncation.
            #[expect(clippy::cast_possible_truncation)]
            let len = bytes.len() as u32;
            data.extend_from_slice(&len.to_le_bytes());
            data.extend_from_slice(bytes);
        } else {
            data.push(0x00); // absence marker
        }
        // Action is serialized to JSON for consistent hashing
        if let Ok(action_json) = serde_json::to_vec(&self.action) {
            data.extend_from_slice(&action_json);
        }
        if let Ok(auth_json) = serde_json::to_vec(&self.authorization) {
            data.extend_from_slice(&auth_json);
        }
        // Outcome: include success/failure indicator
        data.push(u8::from(matches!(
            self.outcome,
            AuditOutcome::Success { .. }
        )));
        data.extend_from_slice(self.previous_hash.as_bytes());
        data.extend_from_slice(self.runtime_key.as_bytes());
        data
    }

    /// Compute the content hash of this entry.
    #[must_use]
    pub fn content_hash(&self) -> ContentHash {
        ContentHash::hash(&self.signing_data())
    }

    /// Verify the entry's signature.
    ///
    /// # Errors
    ///
    /// Returns [`AuditError::InvalidSignature`] if the signature does not match
    /// the entry contents.
    pub fn verify_signature(&self) -> AuditResult<()> {
        let signing_data = self.signing_data();
        self.runtime_key
            .verify(&signing_data, &self.signature)
            .map_err(|_| AuditError::InvalidSignature {
                entry_id: self.id.to_string(),
            })
    }

    /// Check if this entry follows another (chain linking).
    #[must_use]
    pub fn follows(&self, previous: &AuditEntry) -> bool {
        self.previous_hash == previous.content_hash()
    }
}

/// Actions that can be audited.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditAction {
    /// MCP tool was called.
    McpToolCall {
        /// Server name.
        server: String,
        /// Tool name.
        tool: String,
        /// Hash of the arguments (not the args themselves for privacy).
        args_hash: ContentHash,
    },

    /// Capsule tool was called.
    CapsuleToolCall {
        /// Capsule ID.
        capsule_id: String,
        /// Tool name.
        tool: String,
        /// Hash of the arguments (not the args themselves for privacy).
        args_hash: ContentHash,
    },

    /// MCP resource was read.
    McpResourceRead {
        /// Server name.
        server: String,
        /// Resource URI.
        uri: String,
    },

    /// MCP prompt was retrieved.
    McpPromptGet {
        /// Server name.
        server: String,
        /// Prompt name.
        name: String,
    },

    /// MCP elicitation (server requested user input).
    McpElicitation {
        /// Request ID.
        request_id: String,
        /// Schema type (text, select, confirm, etc.).
        schema: String,
    },

    /// MCP URL elicitation (OAuth, payments).
    McpUrlElicitation {
        /// URL presented to user.
        url: String,
        /// Interaction type (oauth, payment, verification, custom).
        interaction_type: String,
    },

    /// MCP sampling (server-initiated LLM call).
    McpSampling {
        /// Model used.
        model: String,
        /// Prompt token count.
        prompt_tokens: usize,
    },

    /// File was read.
    FileRead {
        /// File path.
        path: String,
    },

    /// File was written.
    FileWrite {
        /// File path.
        path: String,
        /// Hash of the written content.
        content_hash: ContentHash,
    },

    /// File was deleted.
    FileDelete {
        /// File path.
        path: String,
    },

    /// Capability token was created.
    CapabilityCreated {
        /// Token ID.
        token_id: TokenId,
        /// Resource pattern.
        resource: String,
        /// Permissions granted.
        permissions: Vec<Permission>,
        /// Token scope.
        scope: ApprovalScope,
    },

    /// Capability token was revoked.
    CapabilityRevoked {
        /// Token ID.
        token_id: TokenId,
        /// Reason for revocation.
        reason: String,
    },

    /// Approval was requested from the user.
    ApprovalRequested {
        /// Type of action being requested.
        action_type: String,
        /// Resource being accessed.
        resource: String,
    },

    /// User granted approval.
    ApprovalGranted {
        /// What was approved.
        action: String,
        /// Resource being accessed.
        resource: Option<String>,
        /// Scope of approval.
        scope: ApprovalScope,
    },

    /// User denied approval.
    ApprovalDenied {
        /// What was denied.
        action: String,
        /// Reason given.
        reason: Option<String>,
    },

    /// Authority-boundary evidence packet was declared.
    AuthorityBoundaryDeclared {
        /// Boundary packet ID.
        boundary_id: String,
        /// Packet producer.
        source: String,
        /// Runtime surface.
        surface: String,
        /// Proposed action.
        action: String,
        /// Resource or target.
        resource: String,
        /// Maximum authority class.
        authority_class: AuthorityClass,
        /// Hash of the full packet payload.
        packet_hash: ContentHash,
    },

    /// Authority-boundary gate was evaluated.
    AuthorityGateEvaluated {
        /// Boundary packet ID.
        boundary_id: String,
        /// Current gate state.
        gate_state: AuthorityGateStateV1,
        /// Whether live execution is eligible now.
        live_eligible_now: bool,
        /// Whether the gate auto-approved the action.
        auto_approved: bool,
        /// Hash of the full packet payload when available.
        packet_hash: Option<ContentHash>,
    },

    /// Authority-boundary lifecycle V2 packet was declared.
    AuthorityBoundaryDeclaredV2 {
        /// Boundary packet ID.
        boundary_id: String,
        /// Packet producer.
        source: String,
        /// Runtime surface.
        surface: String,
        /// Proposed action.
        action: String,
        /// Resource or target.
        resource: String,
        /// Maximum authority class.
        authority_class: AuthorityClass,
        /// Hash of the full packet payload.
        packet_hash: ContentHash,
    },

    /// Authority lifecycle V2 receipt was recorded.
    AuthorityLifecycleReceiptRecorded {
        /// Boundary packet ID.
        boundary_id: String,
        /// Receipt ID.
        receipt_id: String,
        /// Receipt kind.
        receipt_kind: AuthorityLifecycleReceiptKindV2,
        /// Hash of the full receipt payload.
        receipt_hash: ContentHash,
    },

    /// Authority replay result was recorded.
    AuthorityReplayResultRecorded {
        /// Boundary packet ID.
        boundary_id: String,
        /// Replay result ID.
        replay_id: String,
        /// Replay classification.
        classification: ReplayResultClassificationV2,
        /// Hash of the full replay payload.
        result_hash: ContentHash,
    },

    /// Authority lifecycle V2 gate was evaluated.
    AuthorityLifecycleEvaluated {
        /// Boundary packet ID.
        boundary_id: String,
        /// Current lifecycle state.
        state: AuthorityLifecycleStateV2,
        /// Whether live execution is eligible now.
        live_eligible_now: bool,
        /// Whether post-change closure is complete.
        closure_complete: bool,
        /// Hash of the full packet payload when available.
        packet_hash: Option<ContentHash>,
    },

    /// Post-change being response was requested.
    AuthorityPostChangeResponseRequested {
        /// Boundary packet ID.
        boundary_id: String,
        /// Runtime surface.
        surface: String,
        /// Resource or target.
        resource: String,
    },

    /// Post-change being response was recorded.
    AuthorityPostChangeResponseRecorded {
        /// Boundary packet ID.
        boundary_id: String,
        /// Receipt ID.
        receipt_id: String,
        /// Hash of the full receipt payload.
        receipt_hash: ContentHash,
    },

    /// Non-live agency corridor packet was declared.
    AgencyCorridorDeclared {
        /// Corridor packet ID.
        corridor_id: String,
        /// Being or subsystem whose agency continues.
        being: String,
        /// Corridor action.
        action: AgencyCorridorActionV1,
        /// Current corridor state.
        state: AgencyCorridorStateV1,
        /// Hash of the full corridor packet payload.
        packet_hash: ContentHash,
    },

    /// Non-live agency corridor receipt was recorded.
    AgencyCorridorReceiptRecorded {
        /// Corridor packet ID.
        corridor_id: String,
        /// Receipt ID.
        receipt_id: String,
        /// Corridor action.
        action: AgencyCorridorActionV1,
        /// Hash of the full receipt payload.
        receipt_hash: ContentHash,
    },

    /// Non-live closure was reopened as evidence work.
    AgencyCorridorClosureReopened {
        /// Corridor packet ID.
        corridor_id: String,
        /// Closure or work item reference that was reopened.
        reopened_ref: String,
        /// New non-live work item ID, if one was materialized.
        new_work_item_id: Option<String>,
        /// Hash of the bounded reopen evidence payload.
        reopen_hash: ContentHash,
    },

    /// Non-live agency corridor V2 packet was declared.
    AgencyCorridorDeclaredV2 {
        /// Corridor packet ID.
        corridor_id: String,
        /// Being or subsystem whose agency continues.
        being: String,
        /// Corridor action.
        action: AgencyCorridorActionV1,
        /// Current corridor state.
        state: AgencyCorridorStateV1,
        /// Autonomy lease ID attached to the packet, if any.
        lease_id: Option<String>,
        /// Hash of the full corridor packet payload.
        packet_hash: ContentHash,
    },

    /// Non-live agency corridor V2 receipt was recorded.
    AgencyCorridorReceiptRecordedV2 {
        /// Corridor packet ID.
        corridor_id: String,
        /// Receipt ID.
        receipt_id: String,
        /// Autonomy lease ID consumed by the receipt, if any.
        lease_id: Option<String>,
        /// Corridor action.
        action: AgencyCorridorActionV1,
        /// Hash of the full receipt payload.
        receipt_hash: ContentHash,
    },

    /// Non-live agency corridor V2 adaptive queue was evaluated.
    AgencyCorridorQueueEvaluated {
        /// Queue ID.
        queue_id: String,
        /// Total queued steps.
        step_count: u64,
        /// Runnable non-live queued steps.
        ready_count: u64,
        /// Whether live-authority violations blocked the queue.
        blocked_by_live_violation: bool,
        /// Hash of the full queue payload.
        queue_hash: ContentHash,
    },

    /// Non-live agency work program was declared.
    AgencyWorkProgramDeclared {
        /// Program ID.
        program_id: String,
        /// Being or source family whose work is represented.
        being: String,
        /// Program status.
        status: AgencyWorkProgramStatusV1,
        /// Hash of the full program payload.
        program_hash: ContentHash,
    },

    /// Non-live evidence portfolio was updated.
    AgencyEvidencePortfolioUpdated {
        /// Portfolio ID.
        portfolio_id: String,
        /// Program ID.
        program_id: String,
        /// Bounded closure state label.
        closure_state: String,
        /// Hash of the full portfolio payload.
        portfolio_hash: ContentHash,
    },

    /// Non-live quarantined patch bundle was prepared.
    AgencyPatchBundlePrepared {
        /// Patch bundle ID.
        bundle_id: String,
        /// Program ID.
        program_id: String,
        /// Source/runtime surface label.
        surface: String,
        /// Hash of the full patch bundle payload.
        bundle_hash: ContentHash,
    },

    /// Non-live agency work priority was evaluated.
    AgencyPriorityEvaluated {
        /// Program ID.
        program_id: String,
        /// Deterministic priority score, basis points 0-1000.
        deterministic_score: u16,
        /// Hash of the full priority signal payload.
        signal_hash: ContentHash,
    },

    /// Non-live agency program receipt was recorded.
    AgencyProgramReceiptRecorded {
        /// Receipt ID.
        receipt_id: String,
        /// Program ID.
        program_id: String,
        /// Receipt kind.
        kind: AgencyProgramReceiptKindV1,
        /// Hash of the full receipt payload.
        receipt_hash: ContentHash,
    },

    /// Session started.
    SessionStarted {
        /// User ID (key ID bytes).
        user_id: [u8; 8],
        /// Platform the session started from.
        platform: String,
    },

    /// Session ended.
    SessionEnded {
        /// Reason for ending.
        reason: String,
        /// Duration in seconds.
        duration_secs: u64,
    },

    /// Context was summarized (messages evicted).
    ContextSummarized {
        /// Number of messages evicted.
        evicted_count: usize,
        /// Approximate tokens freed.
        tokens_freed: usize,
    },

    /// LLM request was made.
    LlmRequest {
        /// Model used.
        model: String,
        /// Input token count.
        input_tokens: usize,
        /// Output token count.
        output_tokens: usize,
    },

    /// Server was started.
    ServerStarted {
        /// Server name.
        name: String,
        /// Transport type.
        transport: String,
        /// Binary hash (if verified).
        binary_hash: Option<ContentHash>,
    },

    /// Server was stopped.
    ServerStopped {
        /// Server name.
        name: String,
        /// Reason.
        reason: String,
    },

    /// Elicitation request sent to user.
    ElicitationSent {
        /// Request ID.
        request_id: String,
        /// Server requesting.
        server: String,
        /// Type of elicitation.
        elicitation_type: String,
    },

    /// Elicitation response received.
    ElicitationReceived {
        /// Request ID.
        request_id: String,
        /// Action taken (submit/cancel/dismiss).
        action: String,
    },

    /// Security policy violation detected.
    SecurityViolation {
        /// Type of violation.
        violation_type: String,
        /// Details.
        details: String,
    },

    /// Sub-agent was spawned (parent→child linkage).
    SubAgentSpawned {
        /// Parent session ID.
        parent_session_id: String,
        /// Child session ID.
        child_session_id: String,
        /// Task description.
        description: String,
    },

    /// Configuration was reloaded.
    ConfigReloaded,
}

impl AuditAction {
    /// Get a human-readable description of the action.
    #[must_use]
    // Keep this exhaustive registry together so new audit variants cannot miss
    // their reviewer-facing description during a split-table refactor.
    #[allow(clippy::too_many_lines)]
    pub fn description(&self) -> String {
        match self {
            Self::McpToolCall { server, tool, .. } => {
                format!("Called tool {server}:{tool}")
            },
            Self::CapsuleToolCall {
                capsule_id, tool, ..
            } => {
                format!("Called capsule tool {capsule_id}:{tool}")
            },
            Self::McpResourceRead { server, uri } => {
                format!("Read resource {server}:{uri}")
            },
            Self::McpPromptGet { server, name } => {
                format!("Got prompt {server}:{name}")
            },
            Self::McpElicitation { request_id, schema } => {
                format!("Elicitation {request_id} ({schema})")
            },
            Self::McpUrlElicitation {
                interaction_type, ..
            } => {
                format!("URL elicitation ({interaction_type})")
            },
            Self::McpSampling { model, .. } => {
                format!("Sampling request to {model}")
            },
            Self::FileRead { path } => {
                format!("Read file {path}")
            },
            Self::FileWrite { path, .. } => {
                format!("Wrote file {path}")
            },
            Self::FileDelete { path } => {
                format!("Deleted file {path}")
            },
            Self::CapabilityCreated { resource, .. } => {
                format!("Created capability for {resource}")
            },
            Self::CapabilityRevoked { token_id, .. } => {
                format!("Revoked capability {token_id}")
            },
            Self::ApprovalRequested {
                action_type,
                resource,
                ..
            } => {
                format!("Approval requested: {action_type} on {resource}")
            },
            Self::ApprovalGranted { action, .. } => {
                format!("Approved: {action}")
            },
            Self::ApprovalDenied { action, .. } => {
                format!("Denied: {action}")
            },
            Self::AuthorityBoundaryDeclared {
                boundary_id,
                surface,
                action,
                ..
            } => {
                format!("Authority boundary declared: {boundary_id} {surface}:{action}")
            },
            Self::AuthorityGateEvaluated {
                boundary_id,
                gate_state,
                live_eligible_now,
                auto_approved,
                ..
            } => {
                format!(
                    "Authority gate evaluated: {boundary_id} state={gate_state:?} \
                     live_eligible_now={live_eligible_now} auto_approved={auto_approved}"
                )
            },
            Self::AuthorityBoundaryDeclaredV2 {
                boundary_id,
                surface,
                action,
                ..
            } => {
                format!("Authority boundary V2 declared: {boundary_id} {surface}:{action}")
            },
            Self::AuthorityLifecycleReceiptRecorded {
                boundary_id,
                receipt_id,
                receipt_kind,
                ..
            } => {
                format!(
                    "Authority lifecycle receipt: {boundary_id} {receipt_id} kind={receipt_kind:?}"
                )
            },
            Self::AuthorityReplayResultRecorded {
                boundary_id,
                replay_id,
                classification,
                ..
            } => {
                format!(
                    "Authority replay result: {boundary_id} {replay_id} class={classification:?}"
                )
            },
            Self::AuthorityLifecycleEvaluated {
                boundary_id,
                state,
                live_eligible_now,
                closure_complete,
                ..
            } => {
                format!(
                    "Authority lifecycle evaluated: {boundary_id} state={state:?} \
                     live_eligible_now={live_eligible_now} closure_complete={closure_complete}"
                )
            },
            Self::AuthorityPostChangeResponseRequested {
                boundary_id,
                surface,
                ..
            } => {
                format!("Authority post-change response requested: {boundary_id} {surface}")
            },
            Self::AuthorityPostChangeResponseRecorded {
                boundary_id,
                receipt_id,
                ..
            } => {
                format!("Authority post-change response recorded: {boundary_id} {receipt_id}")
            },
            Self::AgencyCorridorDeclared {
                corridor_id,
                being,
                action,
                state,
                ..
            } => {
                format!(
                    "Agency corridor declared: {corridor_id} being={being} action={action:?} state={state:?}"
                )
            },
            Self::AgencyCorridorReceiptRecorded {
                corridor_id,
                receipt_id,
                action,
                ..
            } => {
                format!("Agency corridor receipt: {corridor_id} {receipt_id} action={action:?}")
            },
            Self::AgencyCorridorClosureReopened {
                corridor_id,
                reopened_ref,
                new_work_item_id,
                ..
            } => {
                let new_ref = new_work_item_id.as_deref().unwrap_or("none");
                format!(
                    "Agency corridor closure reopened: {corridor_id} reopened={reopened_ref} new_work_item={new_ref}"
                )
            },
            Self::AgencyCorridorDeclaredV2 {
                corridor_id,
                being,
                action,
                state,
                lease_id,
                ..
            } => {
                let lease = lease_id.as_deref().unwrap_or("none");
                format!(
                    "Agency corridor V2 declared: {corridor_id} being={being} action={action:?} state={state:?} lease={lease}"
                )
            },
            Self::AgencyCorridorReceiptRecordedV2 {
                corridor_id,
                receipt_id,
                lease_id,
                action,
                ..
            } => {
                let lease = lease_id.as_deref().unwrap_or("none");
                format!(
                    "Agency corridor V2 receipt: {corridor_id} {receipt_id} lease={lease} action={action:?}"
                )
            },
            Self::AgencyCorridorQueueEvaluated {
                queue_id,
                step_count,
                ready_count,
                blocked_by_live_violation,
                ..
            } => {
                format!(
                    "Agency corridor V2 queue evaluated: {queue_id} steps={step_count} ready={ready_count} blocked_by_live_violation={blocked_by_live_violation}"
                )
            },
            Self::AgencyWorkProgramDeclared {
                program_id,
                being,
                status,
                ..
            } => {
                format!(
                    "Agency work program declared: {program_id} being={being} status={status:?}"
                )
            },
            Self::AgencyEvidencePortfolioUpdated {
                portfolio_id,
                program_id,
                closure_state,
                ..
            } => {
                format!(
                    "Agency evidence portfolio updated: {portfolio_id} program={program_id} closure_state={closure_state}"
                )
            },
            Self::AgencyPatchBundlePrepared {
                bundle_id,
                program_id,
                surface,
                ..
            } => {
                format!(
                    "Agency patch bundle prepared: {bundle_id} program={program_id} surface={surface}"
                )
            },
            Self::AgencyPriorityEvaluated {
                program_id,
                deterministic_score,
                ..
            } => {
                format!(
                    "Agency priority evaluated: {program_id} deterministic_score={deterministic_score}"
                )
            },
            Self::AgencyProgramReceiptRecorded {
                receipt_id,
                program_id,
                kind,
                ..
            } => {
                format!("Agency program receipt: {receipt_id} program={program_id} kind={kind:?}")
            },
            Self::SessionStarted { platform, .. } => {
                format!("Session started via {platform}")
            },
            Self::SessionEnded { reason, .. } => {
                format!("Session ended: {reason}")
            },
            Self::ContextSummarized { evicted_count, .. } => {
                format!("Summarized {evicted_count} messages")
            },
            Self::LlmRequest { model, .. } => {
                format!("LLM request to {model}")
            },
            Self::ServerStarted { name, .. } => {
                format!("Started server {name}")
            },
            Self::ServerStopped { name, .. } => {
                format!("Stopped server {name}")
            },
            Self::ElicitationSent { server, .. } => {
                format!("Elicitation from {server}")
            },
            Self::ElicitationReceived { action, .. } => {
                format!("Elicitation response: {action}")
            },
            Self::SecurityViolation { violation_type, .. } => {
                format!("Security violation: {violation_type}")
            },
            Self::SubAgentSpawned { description, .. } => {
                format!("Spawned sub-agent: {description}")
            },
            Self::ConfigReloaded => "Configuration reloaded".to_string(),
        }
    }
}

/// How an action was authorized.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthorizationProof {
    /// Authorized by a verified user message.
    User {
        /// User ID (key ID).
        user_id: [u8; 8],
        /// The message that triggered the action.
        message_id: String,
    },
    /// Authorized by capability token.
    Capability {
        /// Token ID.
        token_id: TokenId,
        /// Token content hash.
        token_hash: ContentHash,
    },
    /// Authorized by user approval.
    UserApproval {
        /// User ID (key ID).
        user_id: [u8; 8],
        /// Audit entry ID of the prior approval decision that authorized this
        /// action. `None` when this entry IS the root approval decision
        /// (i.e. the user just said "yes" — there is no earlier entry).
        approval_entry_id: Option<AuditEntryId>,
    },
    /// No authorization required (low-risk operation).
    NotRequired {
        /// Reason no auth needed.
        reason: String,
    },
    /// System-initiated action.
    System {
        /// Reason for system action.
        reason: String,
    },
    /// Authorization was denied.
    Denied {
        /// Reason for denial.
        reason: String,
    },
}

/// Scope of an approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope {
    /// This one time only.
    Once,
    /// For the current session.
    Session,
    /// For the current workspace (persists beyond session).
    Workspace,
    /// Persistent (creates capability).
    Always,
}

impl std::fmt::Display for ApprovalScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Once => write!(f, "once"),
            Self::Session => write!(f, "session"),
            Self::Workspace => write!(f, "workspace"),
            Self::Always => write!(f, "always"),
        }
    }
}

/// Outcome of an audited action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuditOutcome {
    /// Action succeeded.
    Success {
        /// Optional details.
        details: Option<String>,
    },
    /// Action failed.
    Failure {
        /// Error message.
        error: String,
    },
}

impl AuditOutcome {
    /// Create a success outcome.
    #[must_use]
    pub fn success() -> Self {
        Self::Success { details: None }
    }

    /// Create a success outcome with details.
    #[must_use]
    pub fn success_with(details: impl Into<String>) -> Self {
        Self::Success {
            details: Some(details.into()),
        }
    }

    /// Create a failure outcome.
    #[must_use]
    pub fn failure(error: impl Into<String>) -> Self {
        Self::Failure {
            error: error.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrid_crypto::KeyPair;

    fn test_keypair() -> KeyPair {
        KeyPair::generate()
    }

    #[test]
    fn test_entry_creation() {
        let keypair = test_keypair();
        let session_id = SessionId::new();

        let entry = AuditEntry::create(
            session_id,
            AuditAction::SessionStarted {
                user_id: keypair.key_id(),
                platform: "cli".to_string(),
            },
            AuthorizationProof::System {
                reason: "session start".to_string(),
            },
            AuditOutcome::success(),
            ContentHash::zero(),
            &keypair,
        );

        assert!(entry.verify_signature().is_ok());
    }

    #[test]
    fn test_chain_linking() {
        let keypair = test_keypair();
        let session_id = SessionId::new();

        let entry1 = AuditEntry::create(
            session_id.clone(),
            AuditAction::SessionStarted {
                user_id: keypair.key_id(),
                platform: "cli".to_string(),
            },
            AuthorizationProof::System {
                reason: "session start".to_string(),
            },
            AuditOutcome::success(),
            ContentHash::zero(),
            &keypair,
        );

        let entry2 = AuditEntry::create(
            session_id,
            AuditAction::McpToolCall {
                server: "test".to_string(),
                tool: "tool".to_string(),
                args_hash: ContentHash::hash(b"args"),
            },
            AuthorizationProof::NotRequired {
                reason: "test".to_string(),
            },
            AuditOutcome::success(),
            entry1.content_hash(),
            &keypair,
        );

        assert!(entry2.follows(&entry1));
        assert!(!entry1.follows(&entry2));
    }

    #[test]
    fn test_signature_tampering() {
        let keypair = test_keypair();
        let session_id = SessionId::new();

        let mut entry = AuditEntry::create(
            session_id,
            AuditAction::SessionStarted {
                user_id: keypair.key_id(),
                platform: "cli".to_string(),
            },
            AuthorizationProof::System {
                reason: "session start".to_string(),
            },
            AuditOutcome::success(),
            ContentHash::zero(),
            &keypair,
        );

        // Valid signature
        assert!(entry.verify_signature().is_ok());

        // Tamper with the entry
        entry.action = AuditAction::SessionEnded {
            reason: "tampered".to_string(),
            duration_secs: 0,
        };

        // Signature should now fail
        assert!(entry.verify_signature().is_err());
    }

    #[test]
    fn test_action_description() {
        let action = AuditAction::McpToolCall {
            server: "filesystem".to_string(),
            tool: "read_file".to_string(),
            args_hash: ContentHash::zero(),
        };

        assert!(action.description().contains("filesystem:read_file"));
    }

    #[test]
    fn test_authority_boundary_descriptions_are_bounded() {
        let packet_hash = ContentHash::hash(b"packet");
        let declared = AuditAction::AuthorityBoundaryDeclared {
            boundary_id: "boundary-1".to_string(),
            source: "test".to_string(),
            surface: "spectral-bridge".to_string(),
            action: "retune_live_porosity".to_string(),
            resource: "minime://control/porosity".to_string(),
            authority_class: AuthorityClass::MikeOperatorLiveSubstrate,
            packet_hash,
        };
        let description = declared.description();
        assert!(description.contains("boundary-1"));
        assert!(description.contains("spectral-bridge:retune_live_porosity"));
        assert!(!description.contains("felt report"));

        let evaluated = AuditAction::AuthorityGateEvaluated {
            boundary_id: "boundary-1".to_string(),
            gate_state: AuthorityGateStateV1::OperatorApprovalWait,
            live_eligible_now: false,
            auto_approved: false,
            packet_hash: Some(packet_hash),
        };
        let description = evaluated.description();
        assert!(description.contains("OperatorApprovalWait"));
        assert!(description.contains("live_eligible_now=false"));
        assert!(description.contains("auto_approved=false"));

        let declared_v2 = AuditAction::AuthorityBoundaryDeclaredV2 {
            boundary_id: "boundary-v2".to_string(),
            source: "test".to_string(),
            surface: "spectral-bridge".to_string(),
            action: "retune_live_porosity".to_string(),
            resource: "minime://control/porosity".to_string(),
            authority_class: AuthorityClass::MikeOperatorLiveSubstrate,
            packet_hash,
        };
        let description = declared_v2.description();
        assert!(description.contains("boundary-v2"));
        assert!(description.contains("spectral-bridge:retune_live_porosity"));
        assert!(!description.contains("full felt prose"));

        let receipt = AuditAction::AuthorityLifecycleReceiptRecorded {
            boundary_id: "boundary-v2".to_string(),
            receipt_id: "receipt-1".to_string(),
            receipt_kind: AuthorityLifecycleReceiptKindV2::Approval,
            receipt_hash: ContentHash::hash(b"receipt"),
        };
        let description = receipt.description();
        assert!(description.contains("receipt-1"));
        assert!(description.contains("Approval"));

        let lifecycle = AuditAction::AuthorityLifecycleEvaluated {
            boundary_id: "boundary-v2".to_string(),
            state: AuthorityLifecycleStateV2::ExecutedAwaitingResponse,
            live_eligible_now: false,
            closure_complete: false,
            packet_hash: Some(packet_hash),
        };
        let description = lifecycle.description();
        assert!(description.contains("ExecutedAwaitingResponse"));
        assert!(description.contains("closure_complete=false"));

        let corridor = AuditAction::AgencyCorridorDeclared {
            corridor_id: "corridor-1".to_string(),
            being: "astrid".to_string(),
            action: AgencyCorridorActionV1::EmitClosureObjection,
            state: AgencyCorridorStateV1::ClosureObjectionRecorded,
            packet_hash: ContentHash::hash(b"corridor packet with private prose elsewhere"),
        };
        let description = corridor.description();
        assert!(description.contains("corridor-1"));
        assert!(description.contains("astrid"));
        assert!(description.contains("EmitClosureObjection"));
        assert!(!description.contains("private prose"));

        let receipt = AuditAction::AgencyCorridorReceiptRecorded {
            corridor_id: "corridor-1".to_string(),
            receipt_id: "corridor-receipt-1".to_string(),
            action: AgencyCorridorActionV1::RunSafeLab,
            receipt_hash: ContentHash::hash(b"bounded receipt"),
        };
        let description = receipt.description();
        assert!(description.contains("corridor-receipt-1"));
        assert!(description.contains("RunSafeLab"));

        let reopened = AuditAction::AgencyCorridorClosureReopened {
            corridor_id: "corridor-1".to_string(),
            reopened_ref: "closure-card-1".to_string(),
            new_work_item_id: Some("wi_reopened".to_string()),
            reopen_hash: ContentHash::hash(b"bounded reopen"),
        };
        let description = reopened.description();
        assert!(description.contains("closure-card-1"));
        assert!(description.contains("wi_reopened"));

        let corridor_v2 = AuditAction::AgencyCorridorDeclaredV2 {
            corridor_id: "corridor-v2-1".to_string(),
            being: "astrid".to_string(),
            action: AgencyCorridorActionV1::CompareArtifacts,
            state: AgencyCorridorStateV1::EvidenceOnly,
            lease_id: Some("lease-safe-labs".to_string()),
            packet_hash: ContentHash::hash(b"corridor v2 private packet"),
        };
        let description = corridor_v2.description();
        assert!(description.contains("corridor-v2-1"));
        assert!(description.contains("lease-safe-labs"));
        assert!(description.contains("CompareArtifacts"));
        assert!(!description.contains("private packet"));

        let receipt_v2 = AuditAction::AgencyCorridorReceiptRecordedV2 {
            corridor_id: "corridor-v2-1".to_string(),
            receipt_id: "receipt-v2-1".to_string(),
            lease_id: Some("lease-safe-labs".to_string()),
            action: AgencyCorridorActionV1::RunSafeLab,
            receipt_hash: ContentHash::hash(b"receipt private prose"),
        };
        let description = receipt_v2.description();
        assert!(description.contains("receipt-v2-1"));
        assert!(description.contains("lease-safe-labs"));
        assert!(!description.contains("private prose"));

        let queue_v2 = AuditAction::AgencyCorridorQueueEvaluated {
            queue_id: "queue-v2-1".to_string(),
            step_count: 5,
            ready_count: 3,
            blocked_by_live_violation: false,
            queue_hash: ContentHash::hash(b"queue private payload"),
        };
        let description = queue_v2.description();
        assert!(description.contains("queue-v2-1"));
        assert!(description.contains("steps=5"));
        assert!(description.contains("ready=3"));
        assert!(!description.contains("private payload"));

        let program = AuditAction::AgencyWorkProgramDeclared {
            program_id: "program-1".to_string(),
            being: "astrid".to_string(),
            status: AgencyWorkProgramStatusV1::Active,
            program_hash: ContentHash::hash(b"full program hypothesis private prose"),
        };
        let description = program.description();
        assert!(description.contains("program-1"));
        assert!(description.contains("astrid"));
        assert!(description.contains("Active"));
        assert!(!description.contains("private prose"));

        let portfolio = AuditAction::AgencyEvidencePortfolioUpdated {
            portfolio_id: "portfolio-1".to_string(),
            program_id: "program-1".to_string(),
            closure_state: "open".to_string(),
            portfolio_hash: ContentHash::hash(b"full portfolio body private felt anchor"),
        };
        let description = portfolio.description();
        assert!(description.contains("portfolio-1"));
        assert!(description.contains("program-1"));
        assert!(!description.contains("private felt anchor"));

        let bundle = AuditAction::AgencyPatchBundlePrepared {
            bundle_id: "bundle-1".to_string(),
            program_id: "program-1".to_string(),
            surface: "bridge_prompt".to_string(),
            bundle_hash: ContentHash::hash(b"full unified diff private body"),
        };
        let description = bundle.description();
        assert!(description.contains("bundle-1"));
        assert!(description.contains("bridge_prompt"));
        assert!(!description.contains("unified diff"));

        let priority = AuditAction::AgencyPriorityEvaluated {
            program_id: "program-1".to_string(),
            deterministic_score: 720,
            signal_hash: ContentHash::hash(b"priority basis private text"),
        };
        let description = priority.description();
        assert!(description.contains("program-1"));
        assert!(description.contains("720"));
        assert!(!description.contains("private text"));

        let receipt = AuditAction::AgencyProgramReceiptRecorded {
            receipt_id: "program-receipt-1".to_string(),
            program_id: "program-1".to_string(),
            kind: AgencyProgramReceiptKindV1::PatchBundlePrepared,
            receipt_hash: ContentHash::hash(b"receipt private body"),
        };
        let description = receipt.description();
        assert!(description.contains("program-receipt-1"));
        assert!(description.contains("PatchBundlePrepared"));
        assert!(!description.contains("private body"));
    }
}
