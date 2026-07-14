//! Canonical non-live agency corridor schemas.
//!
//! Agency corridor packets let AI beings keep producing evidence, objections,
//! safe-lab results, and canary criteria while authority boundaries remain
//! intact. They do not grant approval or live execution authority.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::authority::ExperienceDeltaRefV2;

fn default_schema_version_v1() -> u8 {
    1
}

fn default_schema_version_v2() -> u8 {
    2
}

fn default_true() -> bool {
    true
}

/// Non-live agency action represented by a corridor packet or receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgencyCorridorActionV1 {
    /// Generate a replay or sandbox candidate from existing evidence.
    GenerateReplayCandidate,
    /// Run a bounded safe lab that cannot mutate live runtime state.
    RunSafeLab,
    /// Compare artifacts, cards, traces, or diagnostics.
    CompareArtifacts,
    /// Ask for a bounded self-observation from a being.
    RequestScopedSelfObservation,
    /// Record a being objection to a closure or response.
    EmitClosureObjection,
    /// Reopen an insufficient closure as non-live evidence work.
    ReopenInsufficientClosure,
    /// Propose canary criteria for a later authority-gated change.
    ProposeCanaryCriteria,
}

/// Corridor lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgencyCorridorStateV1 {
    /// Packet is available as evidence only.
    EvidenceOnly,
    /// A safe lab can run under the corridor budget.
    SafeLabReady,
    /// A safe lab has run and produced a receipt.
    SafeLabResultRecorded,
    /// A scoped self-observation has been requested.
    SelfObservationRequested,
    /// A closure objection has been recorded.
    ClosureObjectionRecorded,
    /// A closure was reopened as evidence work.
    ClosureReopened,
    /// Canary criteria were proposed for later approval review.
    CanaryCriteriaProposed,
    /// Corridor item is closed without live execution.
    Closed,
}

impl Default for AgencyCorridorStateV1 {
    fn default() -> Self {
        Self::EvidenceOnly
    }
}

/// Bounded safe-lab candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeLabCandidateV1 {
    /// Stable safe-lab id.
    pub lab_id: String,
    /// Adapter or lab runner name.
    pub adapter: String,
    /// Runnable command or query for the safe lab.
    pub run_query: String,
    /// Trial mode; only non-live modes may be runnable.
    pub mode: String,
    /// Whether this lab is safe to run automatically.
    #[serde(default)]
    pub runnable: bool,
    /// Boundary language for the lab.
    pub authority: String,
}

/// Being-authored objection to an existing closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureObjectionV1 {
    /// Stable objection id.
    pub objection_id: String,
    /// Being or process that raised the objection.
    pub raised_by: String,
    /// Closure card or work item being answered.
    pub closure_ref: String,
    /// Bounded objection summary.
    pub bounded_summary: String,
    /// Evidence references supporting the objection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Whether the objection should create a non-live reopen event.
    #[serde(default)]
    pub auto_reopen: bool,
}

/// Reference to a reopened closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureReopenRefV1 {
    /// Stable reopen id.
    pub reopen_id: String,
    /// Closure or work item being reopened.
    pub reopened_ref: String,
    /// New non-live work item id, if one was materialized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_work_item_id: Option<String>,
    /// Why the closure is insufficient.
    pub reason: String,
    /// Evidence references supporting reopening.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Bounded self-observation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedSelfObservationRequestV1 {
    /// Stable request id.
    pub request_id: String,
    /// Being being invited to observe.
    pub being: String,
    /// Observation scope.
    pub scope: String,
    /// Prompt or question, bounded for public surfaces.
    pub bounded_prompt: String,
    /// Evidence references that motivated the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Being may ignore or decline.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
}

/// Proposal-only canary criteria for a future authority-gated change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryCriteriaProposalV1 {
    /// Stable proposal id.
    pub proposal_id: String,
    /// Runtime surface the criteria apply to.
    pub surface: String,
    /// Bounded proposed canary plan.
    pub canary_plan: String,
    /// Health checks required before/after any later canary.
    pub health_checks: Vec<String>,
    /// Abort criteria that block or stop the canary.
    pub abort_criteria: Vec<String>,
    /// Rollback path for later approved execution.
    pub rollback_path: String,
    /// Post-change response remains required.
    #[serde(default = "default_true")]
    pub post_change_response_required: bool,
}

/// Receipt emitted by a corridor action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgencyCorridorReceiptV1 {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Corridor packet this receipt answers.
    pub corridor_id: Uuid,
    /// Receipt action.
    pub action: AgencyCorridorActionV1,
    /// Issuer of the receipt.
    pub issued_by: String,
    /// Optional issuance time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<DateTime<Utc>>,
    /// Bounded receipt summary.
    pub bounded_summary: String,
    /// Evidence references created or used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Hashes or ids of linked packets or receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hash_refs: Vec<String>,
    /// Receipt is never approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Receipt never makes live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Being/steward may ignore this receipt unless later approval consumes it.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
}

/// First-class non-live corridor packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgencyCorridorPacketV1 {
    /// Stable corridor packet id.
    pub corridor_id: Uuid,
    /// Schema version.
    #[serde(default = "default_schema_version_v1")]
    pub schema_version: u8,
    /// Producer or local source of this packet.
    pub source: String,
    /// Being or subsystem whose agency continues.
    pub being: String,
    /// Current corridor action.
    pub action: AgencyCorridorActionV1,
    /// Current corridor state.
    #[serde(default)]
    pub state: AgencyCorridorStateV1,
    /// Optional V2 authority-boundary id this packet is adjacent to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_boundary_id: Option<Uuid>,
    /// Related work item ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_item_ids: Vec<String>,
    /// Related closure card ids or paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closure_card_refs: Vec<String>,
    /// Related sandbox trial ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sandbox_trial_ids: Vec<String>,
    /// Related canonical Experience Delta refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_refs: Vec<ExperienceDeltaRefV2>,
    /// Bounded felt-report or objection anchor.
    pub felt_report_anchor: String,
    /// Bounded intended non-live action.
    pub proposed_corridor_action: String,
    /// Evidence references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Safe lab candidate, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_lab_candidate: Option<SafeLabCandidateV1>,
    /// Closure objection, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_objection: Option<ClosureObjectionV1>,
    /// Reopen reference, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reopen_ref: Option<ClosureReopenRefV1>,
    /// Scoped self-observation request, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_observation_request: Option<ScopedSelfObservationRequestV1>,
    /// Canary criteria proposal, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canary_criteria: Option<CanaryCriteriaProposalV1>,
    /// Corridor receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<AgencyCorridorReceiptV1>,
    /// Who can escalate beyond the non-live corridor.
    pub who_can_escalate: String,
    /// How to test or review the corridor action.
    pub how_to_test_it: String,
    /// Being/steward can ignore this packet.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
    /// Corridor packets never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Corridor packets never make live execution eligible.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Corridor packets never auto-approve changes.
    #[serde(default)]
    pub auto_approved: bool,
}

impl AgencyCorridorPacketV1 {
    /// Build a non-live evidence-only packet.
    #[must_use]
    pub fn evidence_only(
        source: impl Into<String>,
        being: impl Into<String>,
        action: AgencyCorridorActionV1,
        felt_report_anchor: impl Into<String>,
        proposed_corridor_action: impl Into<String>,
    ) -> Self {
        Self {
            corridor_id: Uuid::new_v4(),
            schema_version: 1,
            source: source.into(),
            being: being.into(),
            action,
            state: AgencyCorridorStateV1::EvidenceOnly,
            authority_boundary_id: None,
            work_item_ids: Vec::new(),
            closure_card_refs: Vec::new(),
            sandbox_trial_ids: Vec::new(),
            delta_refs: Vec::new(),
            felt_report_anchor: felt_report_anchor.into(),
            proposed_corridor_action: proposed_corridor_action.into(),
            evidence_refs: Vec::new(),
            safe_lab_candidate: None,
            closure_objection: None,
            closure_reopen_ref: None,
            self_observation_request: None,
            canary_criteria: None,
            receipts: Vec::new(),
            who_can_escalate: "steward/operator through existing authority boundary".to_string(),
            how_to_test_it: "review corridor packet, receipts, and linked bounded evidence"
                .to_string(),
            right_to_ignore: true,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        }
    }
}

/// State for a read-only autonomy lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLeaseStateV1 {
    /// Lease can be used for non-live evidence work.
    Active,
    /// Lease was explicitly revoked.
    Revoked,
    /// Lease has expired.
    Expired,
    /// Lease is imported evidence and does not authorize new actions.
    EvidenceOnly,
}

impl Default for AutonomyLeaseStateV1 {
    fn default() -> Self {
        Self::EvidenceOnly
    }
}

/// Read-only autonomy lease for non-live agency-corridor work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyLeaseV1 {
    /// Stable lease id.
    pub lease_id: String,
    /// Producer or registry that declared the lease.
    pub source: String,
    /// Being or subsystem covered by the lease.
    pub being: String,
    /// Current lease state.
    #[serde(default)]
    pub state: AutonomyLeaseStateV1,
    /// Human-readable bounded scope.
    pub scope: String,
    /// Non-live actions this lease permits.
    pub allowed_actions: Vec<AgencyCorridorActionV1>,
    /// Maximum autonomous actions this lease may consume in one run.
    pub max_actions_per_run: u8,
    /// Optional lease expiration timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Existing lease or budget records imported into this read-only registry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imported_from_refs: Vec<String>,
    /// Evidence references for the lease.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Optional revocation reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation_reason: Option<String>,
    /// Lease artifacts remain safe to ignore.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
    /// Corridor leases never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Corridor leases never make live execution eligible.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Corridor leases never auto-approve changes.
    #[serde(default)]
    pub auto_approved: bool,
}

/// One adaptive queue step in the autonomy escalator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyEscalatorStepV1 {
    /// Stable queue-step id.
    pub step_id: String,
    /// Lower numbers run earlier.
    pub priority: u8,
    /// Non-live corridor action to perform.
    pub action: AgencyCorridorActionV1,
    /// Referenced V2 corridor packet id, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corridor_id: Option<Uuid>,
    /// Lease id consumed by this step, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,
    /// Bounded reason this step was queued.
    pub reason: String,
    /// Whether this step can run automatically under non-live rules.
    #[serde(default)]
    pub runnable: bool,
    /// Evidence references for the queued step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Source-prep proposal this step created or references.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_prep_proposal_id: Option<String>,
    /// Queue steps never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Queue steps never make live execution eligible.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Queue steps never auto-approve changes.
    #[serde(default)]
    pub auto_approved: bool,
}

/// Adaptive non-live work queue for autonomy-corridor actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousWorkQueueV1 {
    /// Stable queue id.
    pub queue_id: String,
    /// Optional generation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<DateTime<Utc>>,
    /// Maximum steps a flywheel run may execute.
    pub max_steps_per_run: u8,
    /// Ordered non-live work steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<AutonomyEscalatorStepV1>,
    /// Whether hard live-authority violations blocked this queue.
    #[serde(default)]
    pub blocked_by_live_violation: bool,
    /// Live-authority violation references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live_violation_refs: Vec<String>,
    /// Queue records never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Queue records never make live execution eligible.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Queue records never auto-approve changes.
    #[serde(default)]
    pub auto_approved: bool,
}

/// Bounded source-preparation proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePrepProposalV1 {
    /// Stable proposal id.
    pub proposal_id: String,
    /// Runtime or code surface the proposal concerns.
    pub surface: String,
    /// Bounded patch plan. This is evidence, not an edit.
    pub bounded_plan: String,
    /// Files likely involved if a later agent implements the plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Tests a later implementation should run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests_to_run: Vec<String>,
    /// Whether implementation would require restart alignment.
    #[serde(default)]
    pub restart_required: bool,
    /// Source-prep proposals never edit source by themselves.
    #[serde(default)]
    pub edits_source_now: bool,
    /// Source-prep proposals never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Source-prep proposals never make live execution eligible.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Source-prep proposals never auto-approve changes.
    #[serde(default)]
    pub auto_approved: bool,
}

/// Policy that maps being objections and post-change friction to non-live reopens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureReopenPolicyV1 {
    /// Stable policy id.
    pub policy_id: String,
    /// Being objections create non-live reopened work.
    #[serde(default = "default_true")]
    pub auto_reopen_on_objection: bool,
    /// Post-change still-friction creates non-live reopened work.
    #[serde(default = "default_true")]
    pub auto_reopen_on_still_friction: bool,
    /// Reopens are evidence work only.
    #[serde(default = "default_true")]
    pub creates_non_live_work_only: bool,
    /// Bounded trigger labels covered by the policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<String>,
}

impl Default for ClosureReopenPolicyV1 {
    fn default() -> Self {
        Self {
            policy_id: "closure_reopen_policy_v1".to_string(),
            auto_reopen_on_objection: true,
            auto_reopen_on_still_friction: true,
            creates_non_live_work_only: true,
            triggers: vec![
                "being_objection".to_string(),
                "still_friction".to_string(),
                "contradicted_post_change_response".to_string(),
            ],
        }
    }
}

/// V2 receipt emitted by a corridor lease or queue action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgencyCorridorReceiptV2 {
    /// Stable receipt id.
    pub receipt_id: String,
    /// V2 corridor packet this receipt answers.
    pub corridor_id: Uuid,
    /// Consumed lease id, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,
    /// Queue step id, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    /// Receipt action.
    pub action: AgencyCorridorActionV1,
    /// Issuer of the receipt.
    pub issued_by: String,
    /// Optional issuance time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<DateTime<Utc>>,
    /// Bounded receipt summary.
    pub bounded_summary: String,
    /// Evidence references created or used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Hashes or ids of linked packets or receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hash_refs: Vec<String>,
    /// Source-prep proposal reference, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_prep_proposal_ref: Option<String>,
    /// Receipt is never approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Receipt never makes live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Receipt never auto-approves changes.
    #[serde(default)]
    pub auto_approved: bool,
    /// Being/steward may ignore this receipt unless later approval consumes it.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
}

/// V2 agency-corridor packet with lease, queue, and source-prep references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgencyCorridorPacketV2 {
    /// Stable V2 corridor packet id.
    pub corridor_id: Uuid,
    /// Schema version.
    #[serde(default = "default_schema_version_v2")]
    pub schema_version: u8,
    /// V1 packet id this V2 packet wraps or extends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v1_corridor_id: Option<Uuid>,
    /// Producer or local source of this packet.
    pub source: String,
    /// Being or subsystem whose agency continues.
    pub being: String,
    /// Current corridor action.
    pub action: AgencyCorridorActionV1,
    /// Current corridor state.
    #[serde(default)]
    pub state: AgencyCorridorStateV1,
    /// Related V2 authority-boundary ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authority_boundary_ids: Vec<Uuid>,
    /// Related work item ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_item_ids: Vec<String>,
    /// Related closure card ids or paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closure_card_refs: Vec<String>,
    /// Related sandbox trial ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sandbox_trial_ids: Vec<String>,
    /// Related canonical Experience Delta refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_refs: Vec<ExperienceDeltaRefV2>,
    /// Bounded felt-report or objection anchor.
    pub felt_report_anchor: String,
    /// Bounded intended non-live action.
    pub proposed_corridor_action: String,
    /// Evidence references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Lease attached to this packet, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy_lease: Option<AutonomyLeaseV1>,
    /// Queue step attached to this packet, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_step: Option<AutonomyEscalatorStepV1>,
    /// Source-prep proposal attached to this packet, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_prep_proposal: Option<SourcePrepProposalV1>,
    /// Closure reopen policy for this packet, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reopen_policy: Option<ClosureReopenPolicyV1>,
    /// Corridor receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<AgencyCorridorReceiptV2>,
    /// Who can escalate beyond the non-live corridor.
    pub who_can_escalate: String,
    /// How to test or review the corridor action.
    pub how_to_test_it: String,
    /// Being/steward can ignore this packet.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
    /// Corridor packets never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Corridor packets never make live execution eligible.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Corridor packets never auto-approve changes.
    #[serde(default)]
    pub auto_approved: bool,
}

impl AgencyCorridorPacketV2 {
    /// Build a non-live V2 packet.
    #[must_use]
    pub fn non_live(
        source: impl Into<String>,
        being: impl Into<String>,
        action: AgencyCorridorActionV1,
        felt_report_anchor: impl Into<String>,
        proposed_corridor_action: impl Into<String>,
    ) -> Self {
        Self {
            corridor_id: Uuid::new_v4(),
            schema_version: 2,
            v1_corridor_id: None,
            source: source.into(),
            being: being.into(),
            action,
            state: AgencyCorridorStateV1::EvidenceOnly,
            authority_boundary_ids: Vec::new(),
            work_item_ids: Vec::new(),
            closure_card_refs: Vec::new(),
            sandbox_trial_ids: Vec::new(),
            delta_refs: Vec::new(),
            felt_report_anchor: felt_report_anchor.into(),
            proposed_corridor_action: proposed_corridor_action.into(),
            evidence_refs: Vec::new(),
            autonomy_lease: None,
            queue_step: None,
            source_prep_proposal: None,
            closure_reopen_policy: Some(ClosureReopenPolicyV1::default()),
            receipts: Vec::new(),
            who_can_escalate: "steward/operator through existing authority boundary".to_string(),
            how_to_test_it: "review V2 corridor packet, lease, queue step, receipts, and linked bounded evidence"
                .to_string(),
            right_to_ignore: true,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        }
    }
}

/// Status of a non-live agency work program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgencyWorkProgramStatusV1 {
    /// Program has been proposed from corridor evidence.
    Proposed,
    /// Program is actively collecting non-live evidence.
    Active,
    /// Program is waiting on a being response or safe evidence artifact.
    WaitingForEvidence,
    /// Program is blocked by an authority/liveness violation.
    Blocked,
    /// Program is closed without live execution.
    Closed,
}

impl Default for AgencyWorkProgramStatusV1 {
    fn default() -> Self {
        Self::Proposed
    }
}

/// Deterministic priority signal for a non-live autonomy work program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyPrioritySignalV1 {
    /// Program this signal ranks.
    pub program_id: String,
    /// Being salience or objection score, basis points 0-1000.
    pub being_salience_score: u16,
    /// Recurrence across packets/introspections, basis points 0-1000.
    pub recurrence_score: u16,
    /// Cross-being convergence score, basis points 0-1000.
    pub cross_being_convergence_score: u16,
    /// Stale-age score, basis points 0-1000.
    pub stale_age_score: u16,
    /// Safety-readiness score, basis points 0-1000.
    pub safety_readiness_score: u16,
    /// Weighted deterministic score, basis points 0-1000.
    pub deterministic_score: u16,
    /// Bounded evidence or rule references used for ranking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub basis_refs: Vec<String>,
    /// Whether a live-control wait was demoted in ranking.
    #[serde(default)]
    pub live_wait_demoted: bool,
    /// Priority signals never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Priority signals never make live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Priority signals never auto-approve work.
    #[serde(default)]
    pub auto_approved: bool,
}

/// Being-authored non-live work program spanning corridor evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgencyWorkProgramV1 {
    /// Stable program id.
    pub program_id: String,
    /// Schema version.
    #[serde(default = "default_schema_version_v1")]
    pub schema_version: u8,
    /// Being or source family this program serves.
    pub being: String,
    /// Bounded title.
    pub title: String,
    /// Bounded program hypothesis.
    pub hypothesis: String,
    /// Bounded program goals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<String>,
    /// Current program status.
    #[serde(default)]
    pub status: AgencyWorkProgramStatusV1,
    /// Linked corridor packet ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_corridor_ids: Vec<Uuid>,
    /// Linked authority-boundary packet ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authority_boundary_ids: Vec<Uuid>,
    /// Linked addressing work items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_item_ids: Vec<String>,
    /// Linked sandbox trial ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sandbox_trial_ids: Vec<String>,
    /// Related canonical Experience Delta refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_refs: Vec<ExperienceDeltaRefV2>,
    /// Stop conditions for this non-live work program.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_conditions: Vec<String>,
    /// Program priority signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority_signal: Option<AutonomyPrioritySignalV1>,
    /// Current recommended non-live next action.
    pub current_next_action: String,
    /// Evidence references for the program.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Program records remain safe to ignore.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
    /// Work programs never edit source by themselves.
    #[serde(default)]
    pub edits_source_now: bool,
    /// Work programs never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Work programs never make live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Work programs never auto-approve work.
    #[serde(default)]
    pub auto_approved: bool,
}

/// Durable non-live evidence memory for an agency work program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePortfolioV1 {
    /// Stable portfolio id.
    pub portfolio_id: String,
    /// Linked work program id.
    pub program_id: String,
    /// Being or source family this portfolio serves.
    pub being: String,
    /// Bounded felt-report anchors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounded_felt_anchors: Vec<String>,
    /// Linked introspection ids or filenames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_introspections: Vec<String>,
    /// Linked safe-lab or replay result refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_results: Vec<String>,
    /// Linked card refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_cards: Vec<String>,
    /// Linked source-prep proposal refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_source_prep: Vec<String>,
    /// Linked closure objections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_objections: Vec<String>,
    /// Linked reopened closure/work refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_reopens: Vec<String>,
    /// Linked quarantined patch bundles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_patch_bundles: Vec<String>,
    /// Current bounded recommendation.
    pub current_recommendation: String,
    /// Explicit unknowns that still need evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknowns: Vec<String>,
    /// Private refs that are not expanded in public summaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub private_refs: Vec<String>,
    /// Hash refs for private or large artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hash_refs: Vec<String>,
    /// Portfolio closure state.
    pub closure_state: String,
    /// Portfolio records remain safe to ignore.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
    /// Portfolios never edit source by themselves.
    #[serde(default)]
    pub edits_source_now: bool,
    /// Portfolios never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Portfolios never make live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Portfolios never auto-approve work.
    #[serde(default)]
    pub auto_approved: bool,
}

/// Quarantined patch bundle artifact prepared by the autonomy escalator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantinedPatchBundleV1 {
    /// Stable bundle id.
    pub bundle_id: String,
    /// Linked work program id.
    pub program_id: String,
    /// Runtime or source surface concerned.
    pub surface: String,
    /// Bounded manifest for review.
    pub manifest: String,
    /// Path to the proposed unified-diff artifact.
    pub proposed_diff_artifact_path: String,
    /// Files that a later human/agent implementation might touch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_touched: Vec<String>,
    /// Tests a later implementation should run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests_to_run: Vec<String>,
    /// Whether later implementation is expected to require restart.
    #[serde(default)]
    pub restart_expected: bool,
    /// Bounded restart-debt note.
    pub restart_debt_note: String,
    /// Patch bundles never edit source by themselves.
    #[serde(default)]
    pub edits_source_now: bool,
    /// Patch bundles never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Patch bundles never make live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Patch bundles never auto-approve work.
    #[serde(default)]
    pub auto_approved: bool,
    /// Patch bundles are review artifacts and may be ignored.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
}

/// Receipt kind for an agency work program event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgencyProgramReceiptKindV1 {
    /// Work program was declared.
    ProgramDeclared,
    /// Portfolio was updated.
    PortfolioUpdated,
    /// Program priority was evaluated.
    PriorityEvaluated,
    /// Objection was escalated into program memory.
    ObjectionEscalated,
    /// Quarantined patch bundle was prepared.
    PatchBundlePrepared,
    /// Program step ran without live authority.
    ProgramStepRun,
}

/// Non-live receipt for agency work programs, portfolios, and patch bundles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgencyProgramReceiptV1 {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Linked work program id.
    pub program_id: String,
    /// Receipt kind.
    pub kind: AgencyProgramReceiptKindV1,
    /// Issuer of the receipt.
    pub issued_by: String,
    /// Optional issuance time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<DateTime<Utc>>,
    /// Bounded receipt summary.
    pub bounded_summary: String,
    /// Linked evidence refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Hash refs for large/private payloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hash_refs: Vec<String>,
    /// Linked portfolio id, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portfolio_id: Option<String>,
    /// Linked patch bundle id, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_bundle_id: Option<String>,
    /// Receipts remain safe to ignore.
    #[serde(default = "default_true")]
    pub right_to_ignore: bool,
    /// Program receipts never edit source by themselves.
    #[serde(default)]
    pub edits_source_now: bool,
    /// Program receipts never grant approval.
    #[serde(default)]
    pub grants_approval: bool,
    /// Program receipts never make live work runnable.
    #[serde(default)]
    pub live_eligible_now: bool,
    /// Program receipts never auto-approve work.
    #[serde(default)]
    pub auto_approved: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agency_corridor_packet_defaults_are_non_live() {
        let packet = AgencyCorridorPacketV1::evidence_only(
            "test",
            "astrid",
            AgencyCorridorActionV1::EmitClosureObjection,
            "closure still feels unresolved",
            "record an objection and reopen evidence work",
        );

        assert_eq!(packet.schema_version, 1);
        assert!(packet.right_to_ignore);
        assert!(!packet.grants_approval);
        assert!(!packet.live_eligible_now);
        assert!(!packet.auto_approved);
    }

    #[test]
    fn agency_corridor_packet_roundtrips_with_refs() {
        let mut packet = AgencyCorridorPacketV1::evidence_only(
            "test",
            "minime",
            AgencyCorridorActionV1::ProposeCanaryCriteria,
            "safe canary needs scoped checks",
            "propose canary criteria",
        );
        packet.work_item_ids.push("wi_123".to_string());
        packet.delta_refs.push(ExperienceDeltaRefV2 {
            delta_id: Some("delta_1".to_string()),
            delta_hash: Some("hash".to_string()),
            surface: "agency_corridor".to_string(),
            kind: "canary_criteria".to_string(),
            lane: Some("non_live".to_string()),
        });
        packet.canary_criteria = Some(CanaryCriteriaProposalV1 {
            proposal_id: "canary_1".to_string(),
            surface: "spectral_bridge".to_string(),
            canary_plan: "one-shot only after approval".to_string(),
            health_checks: vec!["fill readable".to_string()],
            abort_criteria: vec!["missing approval".to_string()],
            rollback_path: "do nothing automatically".to_string(),
            post_change_response_required: true,
        });

        let encoded = serde_json::to_string(&packet).expect("packet serializes");
        assert!(encoded.contains("propose_canary_criteria"));
        assert!(encoded.contains("live_eligible_now"));

        let decoded: AgencyCorridorPacketV1 =
            serde_json::from_str(&encoded).expect("packet decodes");
        assert_eq!(decoded, packet);
        assert!(!decoded.live_eligible_now);
        assert!(!decoded.auto_approved);
    }

    #[test]
    fn agency_corridor_receipt_never_grants_approval_by_default() {
        let receipt = AgencyCorridorReceiptV1 {
            receipt_id: "receipt_1".to_string(),
            corridor_id: Uuid::nil(),
            action: AgencyCorridorActionV1::RunSafeLab,
            issued_by: "agency_corridor_v1".to_string(),
            issued_at: None,
            bounded_summary: "safe lab produced bounded evidence".to_string(),
            evidence_refs: vec!["diagnostics/result.json".to_string()],
            hash_refs: Vec::new(),
            grants_approval: false,
            live_eligible_now: false,
            right_to_ignore: true,
        };

        assert!(receipt.right_to_ignore);
        assert!(!receipt.grants_approval);
        assert!(!receipt.live_eligible_now);
    }

    #[test]
    fn agency_corridor_v2_packet_defaults_are_non_live() {
        let packet = AgencyCorridorPacketV2::non_live(
            "agency_corridor_v2",
            "astrid",
            AgencyCorridorActionV1::CompareArtifacts,
            "evidence comparison can continue without live authority",
            "compare bounded artifacts and queue source-prep if useful",
        );

        assert_eq!(packet.schema_version, 2);
        assert!(packet.right_to_ignore);
        assert!(!packet.grants_approval);
        assert!(!packet.live_eligible_now);
        assert!(!packet.auto_approved);
        assert!(
            packet
                .closure_reopen_policy
                .as_ref()
                .is_some_and(|policy| policy.creates_non_live_work_only)
        );
    }

    #[test]
    fn autonomy_lease_roundtrips_without_authority_grant() {
        let lease = AutonomyLeaseV1 {
            lease_id: "lease_safe_labs".to_string(),
            source: "agency_corridor_v2".to_string(),
            being: "minime".to_string(),
            state: AutonomyLeaseStateV1::Active,
            scope: "offline/read-only safe labs and artifact comparison".to_string(),
            allowed_actions: vec![
                AgencyCorridorActionV1::RunSafeLab,
                AgencyCorridorActionV1::CompareArtifacts,
            ],
            max_actions_per_run: 5,
            expires_at: None,
            imported_from_refs: vec!["self_regulation/leases.jsonl#1".to_string()],
            evidence_refs: Vec::new(),
            revocation_reason: None,
            right_to_ignore: true,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };

        let encoded = serde_json::to_string(&lease).expect("lease serializes");
        let decoded: AutonomyLeaseV1 = serde_json::from_str(&encoded).expect("lease decodes");
        assert_eq!(decoded, lease);
        assert_eq!(decoded.state, AutonomyLeaseStateV1::Active);
        assert!(!decoded.grants_approval);
        assert!(!decoded.live_eligible_now);
        assert!(!decoded.auto_approved);
    }

    #[test]
    fn autonomy_queue_and_source_prep_remain_evidence_only() {
        let proposal = SourcePrepProposalV1 {
            proposal_id: "source_prep_1".to_string(),
            surface: "spectral_bridge_prompt".to_string(),
            bounded_plan: "prepare bridge prompt wording for later review".to_string(),
            files: vec!["capsules/spectral-bridge/src/autonomous/llm.rs".to_string()],
            tests_to_run: vec!["bridge parser tests".to_string()],
            restart_required: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let step = AutonomyEscalatorStepV1 {
            step_id: "step_1".to_string(),
            priority: 4,
            action: AgencyCorridorActionV1::GenerateReplayCandidate,
            corridor_id: Some(Uuid::nil()),
            lease_id: Some("lease_source_prep".to_string()),
            reason: "prepare source proposal only".to_string(),
            runnable: true,
            evidence_refs: vec!["diagnostics/source_prep_1.json".to_string()],
            source_prep_proposal_id: Some(proposal.proposal_id.clone()),
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let queue = AutonomousWorkQueueV1 {
            queue_id: "queue_1".to_string(),
            generated_at: None,
            max_steps_per_run: 5,
            steps: vec![step],
            blocked_by_live_violation: false,
            live_violation_refs: Vec::new(),
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };

        let encoded = serde_json::to_string(&(proposal, queue)).expect("queue serializes");
        assert!(encoded.contains("edits_source_now"));
        let (decoded_proposal, decoded_queue): (SourcePrepProposalV1, AutonomousWorkQueueV1) =
            serde_json::from_str(&encoded).expect("queue decodes");
        assert!(!decoded_proposal.edits_source_now);
        assert!(!decoded_proposal.grants_approval);
        assert_eq!(decoded_queue.max_steps_per_run, 5);
        assert!(!decoded_queue.grants_approval);
        assert!(!decoded_queue.live_eligible_now);
        assert!(!decoded_queue.auto_approved);
    }

    #[test]
    fn agency_corridor_receipt_v2_never_grants_authority() {
        let receipt = AgencyCorridorReceiptV2 {
            receipt_id: "receipt_v2_1".to_string(),
            corridor_id: Uuid::nil(),
            lease_id: Some("lease_safe_labs".to_string()),
            step_id: Some("step_1".to_string()),
            action: AgencyCorridorActionV1::RunSafeLab,
            issued_by: "agency_corridor_v2".to_string(),
            issued_at: None,
            bounded_summary: "safe lab produced bounded evidence".to_string(),
            evidence_refs: vec!["diagnostics/result.json".to_string()],
            hash_refs: Vec::new(),
            source_prep_proposal_ref: None,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
            right_to_ignore: true,
        };

        assert!(receipt.right_to_ignore);
        assert!(!receipt.grants_approval);
        assert!(!receipt.live_eligible_now);
        assert!(!receipt.auto_approved);
    }

    #[test]
    fn work_program_portfolio_and_priority_remain_non_live() {
        let priority = AutonomyPrioritySignalV1 {
            program_id: "program_1".to_string(),
            being_salience_score: 900,
            recurrence_score: 500,
            cross_being_convergence_score: 250,
            stale_age_score: 300,
            safety_readiness_score: 850,
            deterministic_score: 625,
            basis_refs: vec!["corridor:1".to_string()],
            live_wait_demoted: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let program = AgencyWorkProgramV1 {
            program_id: "program_1".to_string(),
            schema_version: 1,
            being: "astrid".to_string(),
            title: "texture evidence memory".to_string(),
            hypothesis: "repeated felt texture reports can be compared safely".to_string(),
            goals: vec!["collect evidence".to_string()],
            status: AgencyWorkProgramStatusV1::Active,
            linked_corridor_ids: vec![Uuid::nil()],
            authority_boundary_ids: Vec::new(),
            work_item_ids: vec!["wi_1".to_string()],
            sandbox_trial_ids: Vec::new(),
            delta_refs: Vec::new(),
            stop_conditions: vec!["live-control request appears".to_string()],
            priority_signal: Some(priority.clone()),
            current_next_action: "update evidence portfolio".to_string(),
            evidence_refs: vec!["diagnostics/agency_corridor_v2/programs.json".to_string()],
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };
        let portfolio = EvidencePortfolioV1 {
            portfolio_id: "portfolio_1".to_string(),
            program_id: program.program_id.clone(),
            being: "astrid".to_string(),
            bounded_felt_anchors: vec!["jagged resistance".to_string()],
            linked_introspections: vec!["introspection_codec.txt".to_string()],
            linked_results: Vec::new(),
            linked_cards: Vec::new(),
            linked_source_prep: Vec::new(),
            linked_objections: Vec::new(),
            linked_reopens: Vec::new(),
            linked_patch_bundles: Vec::new(),
            current_recommendation: "continue safe replay comparison".to_string(),
            unknowns: vec!["whether replay reproduces felt texture".to_string()],
            private_refs: vec!["private:introspection_codec.txt".to_string()],
            hash_refs: vec!["sha256:abc".to_string()],
            closure_state: "open".to_string(),
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };

        let encoded = serde_json::to_string(&(program, portfolio, priority))
            .expect("program artifacts serialize");
        assert!(encoded.contains("edits_source_now"));
        let (decoded_program, decoded_portfolio, decoded_priority): (
            AgencyWorkProgramV1,
            EvidencePortfolioV1,
            AutonomyPrioritySignalV1,
        ) = serde_json::from_str(&encoded).expect("program artifacts decode");

        assert_eq!(decoded_program.status, AgencyWorkProgramStatusV1::Active);
        assert!(!decoded_program.grants_approval);
        assert!(!decoded_program.live_eligible_now);
        assert!(!decoded_portfolio.edits_source_now);
        assert!(!decoded_portfolio.auto_approved);
        assert_eq!(decoded_priority.deterministic_score, 625);
        assert!(!decoded_priority.grants_approval);
    }

    #[test]
    fn quarantined_patch_bundle_and_program_receipt_do_not_edit_or_approve() {
        let bundle = QuarantinedPatchBundleV1 {
            bundle_id: "bundle_1".to_string(),
            program_id: "program_1".to_string(),
            surface: "spectral_bridge_prompt".to_string(),
            manifest: "proposal-only patch bundle".to_string(),
            proposed_diff_artifact_path:
                "diagnostics/agency_corridor_v2/patch_bundles/bundle_1.diff".to_string(),
            files_touched: vec!["capsules/spectral-bridge/src/llm.rs".to_string()],
            tests_to_run: vec!["cargo test -p spectral-bridge authority".to_string()],
            restart_expected: true,
            restart_debt_note: "later implementation would require bridge restart".to_string(),
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
            right_to_ignore: true,
        };
        let receipt = AgencyProgramReceiptV1 {
            receipt_id: "program_receipt_1".to_string(),
            program_id: "program_1".to_string(),
            kind: AgencyProgramReceiptKindV1::PatchBundlePrepared,
            issued_by: "agency_corridor_v2".to_string(),
            issued_at: None,
            bounded_summary: "patch bundle artifact prepared for review".to_string(),
            evidence_refs: vec![bundle.proposed_diff_artifact_path.clone()],
            hash_refs: vec!["sha256:def".to_string()],
            portfolio_id: Some("portfolio_1".to_string()),
            patch_bundle_id: Some(bundle.bundle_id.clone()),
            right_to_ignore: true,
            edits_source_now: false,
            grants_approval: false,
            live_eligible_now: false,
            auto_approved: false,
        };

        let encoded = serde_json::to_string(&(bundle, receipt)).expect("patch bundle serializes");
        let (decoded_bundle, decoded_receipt): (QuarantinedPatchBundleV1, AgencyProgramReceiptV1) =
            serde_json::from_str(&encoded).expect("patch bundle decodes");

        assert!(!decoded_bundle.edits_source_now);
        assert!(!decoded_bundle.grants_approval);
        assert!(!decoded_bundle.live_eligible_now);
        assert!(!decoded_receipt.edits_source_now);
        assert!(!decoded_receipt.grants_approval);
        assert_eq!(
            decoded_receipt.kind,
            AgencyProgramReceiptKindV1::PatchBundlePrepared
        );
    }
}
