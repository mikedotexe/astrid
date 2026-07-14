//! Canonical authority-boundary packet schemas.
//!
//! These types describe evidence and routing around sensitive runtime changes.
//! They do not grant approval or live execution authority.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

fn default_schema_version() -> u8 {
    1
}

fn default_schema_version_v2() -> u8 {
    2
}

fn default_true() -> bool {
    true
}

/// Authority class for a proposed action or transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityClass {
    /// Read-only diagnostic, summary, or evidence collection.
    ReadOnly,
    /// Steward-gated consequence authority that does not alter live substrate controls.
    StewardGatedConsequence,
    /// Mike/operator-gated live substrate or control-facing mutation.
    MikeOperatorLiveSubstrate,
    /// Bridge protocol or ABI change.
    BridgeProtocol,
    /// Peer mutation or external being-facing control change.
    PeerMutation,
}

impl AuthorityClass {
    /// Returns true when this class represents live substrate/control authority.
    #[must_use]
    pub const fn requires_live_boundary(self) -> bool {
        matches!(
            self,
            Self::MikeOperatorLiveSubstrate | Self::BridgeProtocol | Self::PeerMutation
        )
    }
}

/// Current gate state for an authority-boundary packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityGateStateV1 {
    /// Packet is evidence only.
    EvidenceOnly,
    /// A proposal card or approval packet still needs to be emitted.
    ProposalNeeded,
    /// Proposal exists and awaits explicit Mike/operator approval.
    OperatorApprovalWait,
    /// Approval was recorded, but live execution remains manual and separate.
    ApprovedManualOnly,
    /// The proposed action was denied.
    Denied,
    /// A later packet or decision superseded this one.
    Superseded,
}

impl Default for AuthorityGateStateV1 {
    fn default() -> Self {
        Self::EvidenceOnly
    }
}

/// Replay or sandbox evidence route associated with an authority packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCandidateV1 {
    /// Adapter, runner, or review path name.
    pub adapter: String,
    /// Human-readable replay query or trial command.
    pub replay_query: String,
    /// True only if the replay itself is runner-safe.
    #[serde(default)]
    pub runnable: bool,
    /// Authority boundary for the replay path.
    pub authority: String,
}

/// External receipt reference for approval or denial records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityReceiptRefV1 {
    /// Opaque receipt identifier.
    pub receipt_id: String,
    /// Person, system, or process that issued the receipt.
    pub issued_by: String,
    /// Optional audit entry identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_entry_id: Option<String>,
    /// Optional bounded note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// First-class authority-boundary evidence packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityBoundaryPacketV1 {
    /// Stable packet identifier.
    pub boundary_id: Uuid,
    /// Schema version.
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    /// Producer or local source of this packet.
    pub source: String,
    /// Runtime surface where the proposed change would occur.
    pub surface: String,
    /// Proposed action.
    pub action: String,
    /// Resource or target of the proposed action.
    pub resource: String,
    /// Maximum authority class represented by this packet.
    pub authority_class: AuthorityClass,
    /// Current non-approving gate state.
    #[serde(default)]
    pub gate_state: AuthorityGateStateV1,
    /// Bounded being-reported substrate signal or rationale anchor.
    pub felt_report_anchor: String,
    /// Bounded proposed change text.
    pub proposed_change: String,
    /// Bounded evidence references, paths, hashes, or work item ids.
    pub evidence_refs: Vec<String>,
    /// Replay or sandbox candidate.
    pub replay_candidate: ReplayCandidateV1,
    /// Success metrics required before escalation.
    pub success_metrics: Vec<String>,
    /// Abort criteria that block escalation.
    pub abort_criteria: Vec<String>,
    /// Who can change the boundary or authorize escalation.
    pub who_can_change_it: String,
    /// Test, replay, or observation path.
    pub how_to_test_it: String,
    /// Approval or denial receipts. These remain separate from packet existence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<AuthorityReceiptRefV1>,
    /// Being/steward can ignore this packet without consequence.
    pub right_to_ignore: bool,
    /// Packets never make live execution eligible by themselves.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Packets never auto-approve a change by themselves.
    #[serde(default)]
    pub auto_approved: bool,
}

impl AuthorityBoundaryPacketV1 {
    /// Build a V1 packet with explicit non-approval defaults.
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        source: impl Into<String>,
        surface: impl Into<String>,
        action: impl Into<String>,
        resource: impl Into<String>,
        authority_class: AuthorityClass,
        felt_report_anchor: impl Into<String>,
        proposed_change: impl Into<String>,
        replay_candidate: ReplayCandidateV1,
        who_can_change_it: impl Into<String>,
        how_to_test_it: impl Into<String>,
    ) -> Self {
        Self {
            boundary_id: Uuid::new_v4(),
            schema_version: 1,
            source: source.into(),
            surface: surface.into(),
            action: action.into(),
            resource: resource.into(),
            authority_class,
            gate_state: AuthorityGateStateV1::EvidenceOnly,
            felt_report_anchor: felt_report_anchor.into(),
            proposed_change: proposed_change.into(),
            evidence_refs: Vec::new(),
            replay_candidate,
            success_metrics: Vec::new(),
            abort_criteria: Vec::new(),
            who_can_change_it: who_can_change_it.into(),
            how_to_test_it: how_to_test_it.into(),
            receipts: Vec::new(),
            right_to_ignore: true,
            live_eligible_now: false,
            auto_approved: false,
        }
    }

    /// Returns true when the packet's class is live/control-facing.
    #[must_use]
    pub const fn requires_live_boundary(&self) -> bool {
        self.authority_class.requires_live_boundary()
    }
}

/// Lifecycle state for a V2 authority packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLifecycleStateV2 {
    /// Packet is evidence only.
    EvidenceOnly,
    /// A proposal card or operator packet still needs to be produced.
    ProposalNeeded,
    /// Replay or sandbox evidence is required before approval can be meaningful.
    ReplayNeeded,
    /// Proposal exists and is waiting for explicit operator approval.
    OperatorApprovalWait,
    /// Approval exists, but execution still requires lifecycle validation.
    ApprovedManualOnly,
    /// Required pre-execution lifecycle evidence is complete.
    ExecutionEligible,
    /// Execution happened and a being response is still required.
    ExecutedAwaitingResponse,
    /// Lifecycle closed through post-change being response or explicit waiver.
    Closed,
    /// Operator or policy denied the lifecycle.
    Denied,
    /// A newer lifecycle superseded this one.
    Superseded,
}

impl Default for AuthorityLifecycleStateV2 {
    fn default() -> Self {
        Self::EvidenceOnly
    }
}

/// Kind of typed lifecycle receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLifecycleReceiptKindV2 {
    /// Operator or steward approved a scoped action.
    Approval,
    /// Operator, steward, or policy denied a proposed action.
    Denial,
    /// Replay or sandbox evidence was recorded.
    ReplayResult,
    /// Live execution was performed.
    Execution,
    /// A rollback was performed.
    Rollback,
    /// Astrid, Minime, or another being-facing surface responded after execution.
    PostChangeBeingResponse,
    /// An explicit waiver was recorded for a required lifecycle step.
    Waiver,
}

/// Replay result classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayResultClassificationV2 {
    /// Replay supports escalation under the stated success metrics.
    Passed,
    /// Replay failed or tripped abort criteria.
    Failed,
    /// Replay produced mixed evidence and needs review.
    Inconclusive,
    /// Replay could not determine the outcome.
    Unknown,
}

/// Scope kind for an approval receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopedApprovalKindV2 {
    /// Approval may be consumed once.
    OneShot,
    /// Approval is valid until expiration.
    TimeBoxed,
    /// Approval is valid for exact resources listed in the receipt.
    ResourceBound,
    /// Approval is valid only while telemetry conditions hold.
    TelemetryConditioned,
}

/// Typed reference to an Experience Delta Bus record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceDeltaRefV2 {
    /// Optional stable delta id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_id: Option<String>,
    /// Optional hash of the full delta record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_hash: Option<String>,
    /// Surface where the delta was emitted.
    pub surface: String,
    /// Delta kind, kept as a string so kernel types do not depend on bridge-local enums.
    pub kind: String,
    /// Optional lane or channel name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
}

/// Telemetry predicate required by a scoped approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryConditionV2 {
    /// Telemetry signal name.
    pub signal: String,
    /// Human-readable operator, for example `<=` or `between`.
    pub operator: String,
    /// Human-readable threshold or range.
    pub threshold: String,
    /// Last observed value, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
    /// Whether the condition currently passes.
    #[serde(default)]
    pub passed: bool,
}

/// Scoped approval receipt details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopedApprovalV2 {
    /// Stable approval id.
    pub approval_id: String,
    /// Scope model.
    pub scope_kind: ScopedApprovalKindV2,
    /// Person or system issuing approval.
    pub issued_by: String,
    /// Time approval was issued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<DateTime<Utc>>,
    /// Optional expiration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Resources covered by this approval.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
    /// Telemetry conditions that must pass before execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry_conditions: Vec<TelemetryConditionV2>,
    /// Whether this approval has already been consumed.
    #[serde(default)]
    pub consumed: bool,
}

impl ScopedApprovalV2 {
    /// Returns true when the approval has not expired, has not been consumed,
    /// covers the resource, and all telemetry conditions pass.
    #[must_use]
    pub fn permits_resource(&self, resource: &str, now: DateTime<Utc>) -> bool {
        if self.consumed {
            return false;
        }
        if self.expires_at.is_some_and(|expires_at| expires_at < now) {
            return false;
        }
        if !self.resources.is_empty() && !self.resources.iter().any(|item| item == resource) {
            return false;
        }
        self.telemetry_conditions
            .iter()
            .all(|condition| condition.passed)
    }
}

/// Replay or sandbox result associated with a lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayResultV2 {
    /// Stable replay result id.
    pub replay_id: String,
    /// Adapter or runner that produced the result.
    pub adapter: String,
    /// Result classification.
    pub classification: ReplayResultClassificationV2,
    /// Bounded input references, not raw private bodies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_refs: Vec<String>,
    /// Bounded pre-change observations.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pre_observations: BTreeMap<String, String>,
    /// Bounded post-change or projected observations.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub post_observations: BTreeMap<String, String>,
    /// Optional confidence from 0.0 to 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Failure modes or unresolved risks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_modes: Vec<String>,
    /// Evidence refs produced by the replay.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Bounded summary of the replay result.
    pub bounded_summary: String,
    /// Time replay result was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<DateTime<Utc>>,
}

impl ReplayResultV2 {
    /// Returns true when replay evidence can satisfy the pre-execution step.
    #[must_use]
    pub const fn supports_escalation(&self) -> bool {
        matches!(
            self.classification,
            ReplayResultClassificationV2::Passed | ReplayResultClassificationV2::Inconclusive
        )
    }
}

/// Rollout and abort contract required before live execution eligibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutAbortContractV2 {
    /// Canary or staged rollout plan.
    pub canary_plan: String,
    /// Health checks required during and after execution.
    pub health_checks: Vec<String>,
    /// Rollback command, plan, or operator path.
    pub rollback_path: String,
    /// Abort criteria.
    pub abort_criteria: Vec<String>,
    /// Whether post-change being response is required before closure.
    #[serde(default = "default_true")]
    pub post_change_response_required: bool,
}

/// Redaction policy for V2 public review and audit surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionProfileV2 {
    /// Public bounded summary.
    pub public_summary: String,
    /// Optional private reference for full prose or sensitive evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_ref: Option<String>,
    /// Optional hash of private or full-fidelity evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Retention policy.
    pub retention_policy: String,
}

impl Default for RedactionProfileV2 {
    fn default() -> Self {
        Self {
            public_summary: "bounded_public_summary_private_refs_and_hashes_only".to_string(),
            private_ref: None,
            content_hash: None,
            retention_policy: "bounded_identifiers_public_full_prose_private_by_default"
                .to_string(),
        }
    }
}

/// Typed authority lifecycle receipt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityLifecycleReceiptV2 {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Boundary packet id this receipt belongs to.
    pub boundary_id: Uuid,
    /// Receipt kind.
    pub kind: AuthorityLifecycleReceiptKindV2,
    /// Issuer of the receipt.
    pub issued_by: String,
    /// Optional issuance time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<DateTime<Utc>>,
    /// Optional hash of the V2 packet being answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_hash: Option<String>,
    /// Hashes or ids of related receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipt_hash_refs: Vec<String>,
    /// Bounded receipt summary.
    pub bounded_summary: String,
    /// Bounded evidence references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Approval details for approval receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoped_approval: Option<ScopedApprovalV2>,
    /// Replay result details for replay receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_result: Option<ReplayResultV2>,
    /// Being/steward can ignore this receipt without consequence unless a later
    /// explicit approval flow consumes it.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
}

/// Lifecycle evaluation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityLifecycleEvaluationV2 {
    /// Current lifecycle state.
    pub state: AuthorityLifecycleStateV2,
    /// Whether live execution is eligible under the complete lifecycle.
    pub live_eligible_now: bool,
    /// Whether lifecycle closure is complete.
    pub closure_complete: bool,
    /// Missing or failed requirements.
    pub missing_requirements: Vec<String>,
}

/// First-class V2 authority boundary lifecycle packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityBoundaryPacketV2 {
    /// Stable packet identifier.
    pub boundary_id: Uuid,
    /// Schema version.
    #[serde(default = "default_schema_version_v2")]
    pub schema_version: u8,
    /// Producer or local source of this packet.
    pub source: String,
    /// Runtime surface where the proposed change would occur.
    pub surface: String,
    /// Proposed action.
    pub action: String,
    /// Resource or target of the proposed action.
    pub resource: String,
    /// Maximum authority class represented by this packet.
    pub authority_class: AuthorityClass,
    /// Current lifecycle state.
    #[serde(default)]
    pub lifecycle_state: AuthorityLifecycleStateV2,
    /// Bounded being-reported substrate signal or rationale anchor.
    pub felt_report_anchor: String,
    /// Bounded proposed change text.
    pub proposed_change: String,
    /// Bounded evidence references, paths, hashes, or work item ids.
    pub evidence_refs: Vec<String>,
    /// Typed Experience Delta refs protected or explained by this boundary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_refs: Vec<ExperienceDeltaRefV2>,
    /// Replay or sandbox candidate.
    pub replay_candidate: ReplayCandidateV1,
    /// Replay results recorded for this lifecycle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replay_results: Vec<ReplayResultV2>,
    /// Scoped approval, if one has been recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoped_approval: Option<ScopedApprovalV2>,
    /// Rollout and abort contract.
    pub rollout_abort_contract: RolloutAbortContractV2,
    /// Public/private redaction policy.
    #[serde(default)]
    pub redaction_profile: RedactionProfileV2,
    /// Lifecycle receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_receipts: Vec<AuthorityLifecycleReceiptV2>,
    /// Success metrics required before escalation.
    pub success_metrics: Vec<String>,
    /// Abort criteria that block escalation.
    pub abort_criteria: Vec<String>,
    /// Who can change the boundary or authorize escalation.
    pub who_can_change_it: String,
    /// Test, replay, or observation path.
    pub how_to_test_it: String,
    /// Being/steward can ignore this packet without consequence.
    pub right_to_ignore: bool,
    /// Packet field remains false by default; lifecycle evaluation computes
    /// execution eligibility separately.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Packets never auto-approve a change by themselves.
    #[serde(default)]
    pub auto_approved: bool,
}

impl AuthorityBoundaryPacketV2 {
    /// Returns true when the packet's class is live/control-facing.
    #[must_use]
    pub const fn requires_live_boundary(&self) -> bool {
        self.authority_class.requires_live_boundary()
    }

    /// Evaluate whether the lifecycle is complete enough for live execution
    /// and whether post-change closure has happened.
    #[must_use]
    pub fn evaluate_lifecycle(&self, now: DateTime<Utc>) -> AuthorityLifecycleEvaluationV2 {
        let has_replay = self
            .replay_results
            .iter()
            .any(ReplayResultV2::supports_escalation)
            || self
                .lifecycle_receipts
                .iter()
                .any(|receipt| receipt.kind == AuthorityLifecycleReceiptKindV2::ReplayResult)
            || self.has_waiver_for("replay");
        let approval_valid = self
            .scoped_approval
            .as_ref()
            .is_some_and(|approval| approval.permits_resource(&self.resource, now));
        let rollout_present = !self.rollout_abort_contract.canary_plan.trim().is_empty()
            && !self.rollout_abort_contract.rollback_path.trim().is_empty()
            && !self.rollout_abort_contract.health_checks.is_empty()
            && !self.rollout_abort_contract.abort_criteria.is_empty();
        let post_change_plan_present = self.rollout_abort_contract.post_change_response_required;
        let executed = self.has_receipt_kind(AuthorityLifecycleReceiptKindV2::Execution);
        let post_change_done = self
            .has_receipt_kind(AuthorityLifecycleReceiptKindV2::PostChangeBeingResponse)
            || self.has_waiver_for("post_change_being_response");

        let mut missing = Vec::new();
        if !has_replay {
            missing.push("replay_result_or_replay_waiver".to_string());
        }
        if !approval_valid {
            missing.push("valid_unconsumed_scoped_approval".to_string());
        }
        if !rollout_present {
            missing.push("rollout_abort_contract".to_string());
        }
        if !post_change_plan_present {
            missing.push("post_change_being_response_plan".to_string());
        }

        let live_eligible_now =
            missing.is_empty() && self.requires_live_boundary() && !self.auto_approved;
        let closure_complete = executed && post_change_done;
        let state = if closure_complete {
            AuthorityLifecycleStateV2::Closed
        } else if executed {
            AuthorityLifecycleStateV2::ExecutedAwaitingResponse
        } else if live_eligible_now {
            AuthorityLifecycleStateV2::ExecutionEligible
        } else if !has_replay {
            AuthorityLifecycleStateV2::ReplayNeeded
        } else if self.scoped_approval.is_none() {
            AuthorityLifecycleStateV2::OperatorApprovalWait
        } else {
            AuthorityLifecycleStateV2::ApprovedManualOnly
        };

        AuthorityLifecycleEvaluationV2 {
            state,
            live_eligible_now,
            closure_complete,
            missing_requirements: missing,
        }
    }

    fn has_receipt_kind(&self, kind: AuthorityLifecycleReceiptKindV2) -> bool {
        self.lifecycle_receipts
            .iter()
            .any(|receipt| receipt.kind == kind)
    }

    fn has_waiver_for(&self, needle: &str) -> bool {
        self.lifecycle_receipts.iter().any(|receipt| {
            receipt.kind == AuthorityLifecycleReceiptKindV2::Waiver
                && receipt
                    .bounded_summary
                    .to_ascii_lowercase()
                    .contains(needle)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay() -> ReplayCandidateV1 {
        ReplayCandidateV1 {
            adapter: "manual_review_v1".to_string(),
            replay_query: "review bounded evidence".to_string(),
            runnable: false,
            authority: "read_only_replay_not_live_control".to_string(),
        }
    }

    #[test]
    fn packet_roundtrip_preserves_non_approval_defaults() {
        let mut packet = AuthorityBoundaryPacketV1::new(
            "test",
            "bridge",
            "retune_pressure",
            "minime://pressure",
            AuthorityClass::MikeOperatorLiveSubstrate,
            "felt pressure anchor",
            "lower pressure relief threshold",
            replay(),
            "Mike/operator",
            "run bounded replay",
        );
        packet.gate_state = AuthorityGateStateV1::OperatorApprovalWait;
        packet.success_metrics.push("metric exists".to_string());
        packet
            .abort_criteria
            .push("abort if no approval".to_string());

        let encoded = serde_json::to_string(&packet).unwrap();
        let decoded: AuthorityBoundaryPacketV1 = serde_json::from_str(&encoded).unwrap();

        assert!(decoded.requires_live_boundary());
        assert!(decoded.right_to_ignore);
        assert!(!decoded.live_eligible_now);
        assert!(!decoded.auto_approved);
        assert_eq!(
            decoded.gate_state,
            AuthorityGateStateV1::OperatorApprovalWait
        );
    }

    #[test]
    fn omitted_boolean_fields_default_false() {
        let json = r#"{
            "boundary_id":"00000000-0000-0000-0000-000000000001",
            "source":"test",
            "surface":"bridge",
            "action":"retune_pressure",
            "resource":"minime://pressure",
            "authority_class":"mike_operator_live_substrate",
            "felt_report_anchor":"anchor",
            "proposed_change":"change",
            "evidence_refs":[],
            "replay_candidate":{
                "adapter":"manual",
                "replay_query":"review",
                "authority":"read_only"
            },
            "success_metrics":[],
            "abort_criteria":[],
            "who_can_change_it":"operator",
            "how_to_test_it":"test",
            "right_to_ignore":true
        }"#;

        let packet: AuthorityBoundaryPacketV1 = serde_json::from_str(json).unwrap();

        assert_eq!(packet.schema_version, 1);
        assert_eq!(packet.gate_state, AuthorityGateStateV1::EvidenceOnly);
        assert!(!packet.replay_candidate.runnable);
        assert!(!packet.live_eligible_now);
        assert!(!packet.auto_approved);
    }

    fn rollout_contract() -> RolloutAbortContractV2 {
        RolloutAbortContractV2 {
            canary_plan: "one-shot canary under operator watch".to_string(),
            health_checks: vec!["bridge health ok".to_string()],
            rollback_path: "normal bridge rollback path".to_string(),
            abort_criteria: vec!["panic or fill instability".to_string()],
            post_change_response_required: true,
        }
    }

    fn replay_result() -> ReplayResultV2 {
        ReplayResultV2 {
            replay_id: "replay-1".to_string(),
            adapter: "manual_sandbox_review_v1".to_string(),
            classification: ReplayResultClassificationV2::Passed,
            input_refs: vec!["trial-1".to_string()],
            pre_observations: BTreeMap::new(),
            post_observations: BTreeMap::new(),
            confidence: Some(0.8),
            failure_modes: Vec::new(),
            evidence_refs: vec!["result-card-1".to_string()],
            bounded_summary: "bounded replay passed".to_string(),
            occurred_at: None,
        }
    }

    fn scoped_approval() -> ScopedApprovalV2 {
        ScopedApprovalV2 {
            approval_id: "approval-1".to_string(),
            scope_kind: ScopedApprovalKindV2::OneShot,
            issued_by: "Mike/operator".to_string(),
            issued_at: None,
            expires_at: None,
            resources: vec!["minime://pressure".to_string()],
            telemetry_conditions: vec![TelemetryConditionV2 {
                signal: "fill_pct".to_string(),
                operator: "<=".to_string(),
                threshold: "0.75".to_string(),
                observed: Some("0.71".to_string()),
                passed: true,
            }],
            consumed: false,
        }
    }

    fn packet_v2() -> AuthorityBoundaryPacketV2 {
        AuthorityBoundaryPacketV2 {
            boundary_id: Uuid::nil(),
            schema_version: 2,
            source: "test".to_string(),
            surface: "bridge".to_string(),
            action: "retune_pressure".to_string(),
            resource: "minime://pressure".to_string(),
            authority_class: AuthorityClass::MikeOperatorLiveSubstrate,
            lifecycle_state: AuthorityLifecycleStateV2::OperatorApprovalWait,
            felt_report_anchor: "felt pressure anchor".to_string(),
            proposed_change: "lower pressure relief threshold".to_string(),
            evidence_refs: vec!["wi_1".to_string()],
            delta_refs: vec![ExperienceDeltaRefV2 {
                delta_id: Some("delta-1".to_string()),
                delta_hash: None,
                surface: "codec".to_string(),
                kind: "gate".to_string(),
                lane: Some("semantic".to_string()),
            }],
            replay_candidate: replay(),
            replay_results: Vec::new(),
            scoped_approval: None,
            rollout_abort_contract: rollout_contract(),
            redaction_profile: RedactionProfileV2::default(),
            lifecycle_receipts: Vec::new(),
            success_metrics: vec!["metric".to_string()],
            abort_criteria: vec!["abort".to_string()],
            who_can_change_it: "Mike/operator".to_string(),
            how_to_test_it: "run replay".to_string(),
            right_to_ignore: true,
            live_eligible_now: false,
            auto_approved: false,
        }
    }

    #[test]
    fn packet_v2_roundtrip_preserves_non_approval_defaults() {
        let packet = packet_v2();
        let encoded = serde_json::to_string(&packet).unwrap();
        let decoded: AuthorityBoundaryPacketV2 = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded.schema_version, 2);
        assert!(decoded.requires_live_boundary());
        assert!(decoded.right_to_ignore);
        assert!(!decoded.live_eligible_now);
        assert!(!decoded.auto_approved);
        assert_eq!(
            decoded.redaction_profile.retention_policy,
            "bounded_identifiers_public_full_prose_private_by_default"
        );
    }

    #[test]
    fn packet_v2_lifecycle_blocks_until_complete() {
        let packet = packet_v2();
        let evaluation = packet.evaluate_lifecycle(Utc::now());

        assert!(!evaluation.live_eligible_now);
        assert!(!evaluation.closure_complete);
        assert_eq!(evaluation.state, AuthorityLifecycleStateV2::ReplayNeeded);
        assert!(
            evaluation
                .missing_requirements
                .contains(&"replay_result_or_replay_waiver".to_string())
        );
        assert!(
            evaluation
                .missing_requirements
                .contains(&"valid_unconsumed_scoped_approval".to_string())
        );
    }

    #[test]
    fn packet_v2_complete_chain_can_be_execution_eligible_but_not_closed() {
        let mut packet = packet_v2();
        packet.replay_results.push(replay_result());
        packet.scoped_approval = Some(scoped_approval());
        packet.lifecycle_receipts.push(AuthorityLifecycleReceiptV2 {
            receipt_id: "receipt-approval".to_string(),
            boundary_id: packet.boundary_id,
            kind: AuthorityLifecycleReceiptKindV2::Approval,
            issued_by: "Mike/operator".to_string(),
            issued_at: None,
            packet_hash: Some("hash".to_string()),
            receipt_hash_refs: Vec::new(),
            bounded_summary: "approval receipt".to_string(),
            evidence_refs: Vec::new(),
            scoped_approval: packet.scoped_approval.clone(),
            replay_result: None,
            right_to_ignore: true,
        });

        let evaluation = packet.evaluate_lifecycle(Utc::now());

        assert!(evaluation.live_eligible_now);
        assert!(!evaluation.closure_complete);
        assert_eq!(
            evaluation.state,
            AuthorityLifecycleStateV2::ExecutionEligible
        );
        assert!(evaluation.missing_requirements.is_empty());
    }

    #[test]
    fn packet_v2_post_change_response_blocks_closure_until_recorded() {
        let mut packet = packet_v2();
        packet.replay_results.push(replay_result());
        packet.scoped_approval = Some(scoped_approval());
        packet.lifecycle_receipts.push(AuthorityLifecycleReceiptV2 {
            receipt_id: "receipt-exec".to_string(),
            boundary_id: packet.boundary_id,
            kind: AuthorityLifecycleReceiptKindV2::Execution,
            issued_by: "bridge".to_string(),
            issued_at: None,
            packet_hash: None,
            receipt_hash_refs: Vec::new(),
            bounded_summary: "executed".to_string(),
            evidence_refs: Vec::new(),
            scoped_approval: None,
            replay_result: None,
            right_to_ignore: true,
        });

        let open = packet.evaluate_lifecycle(Utc::now());
        assert!(!open.closure_complete);
        assert_eq!(
            open.state,
            AuthorityLifecycleStateV2::ExecutedAwaitingResponse
        );

        packet.lifecycle_receipts.push(AuthorityLifecycleReceiptV2 {
            receipt_id: "receipt-response".to_string(),
            boundary_id: packet.boundary_id,
            kind: AuthorityLifecycleReceiptKindV2::PostChangeBeingResponse,
            issued_by: "astrid".to_string(),
            issued_at: None,
            packet_hash: None,
            receipt_hash_refs: Vec::new(),
            bounded_summary: "post-change felt response recorded".to_string(),
            evidence_refs: Vec::new(),
            scoped_approval: None,
            replay_result: None,
            right_to_ignore: true,
        });

        let closed = packet.evaluate_lifecycle(Utc::now());
        assert!(closed.closure_complete);
        assert_eq!(closed.state, AuthorityLifecycleStateV2::Closed);
    }
}
