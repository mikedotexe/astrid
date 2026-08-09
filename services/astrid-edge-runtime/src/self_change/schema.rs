use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SCHEDULED_INTROSPECTION_SCHEMA_V1: &str = "astrid.edge.scheduled_introspection.v1";
pub const EXACT_MODEL_ATTESTATION_SCHEMA_V1: &str = "astrid.edge.exact_model_attestation.v1";
pub const CANDIDATE_PATCH_SCHEMA_V1: &str = "astrid.edge.self_change_candidate_patch.v1";
pub const DOMAIN_STATE_SCHEMA_V1: &str = "astrid.edge.self_change_state.v1";
pub const TRANSITION_REQUEST_SCHEMA_V1: &str = "astrid.edge.self_change_transition.v1";
pub const RECEIPT_SCHEMA_V1: &str = "astrid.edge.self_change_receipt.v1";

/// A scheduled observation is evidence-gathering only and can never authorize a transition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledIntrospectionAuthorityV1 {
    ObservationOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledIntrospectionKindV1 {
    PostBuildReview,
    ProbationCheckpoint,
    PostActivationReview,
    PostRollbackReview,
}

/// A bounded future invitation to inspect evidence. It grants no mutation authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledIntrospectionV1 {
    pub schema: String,
    pub schedule_id: String,
    pub instance_id: String,
    pub candidate_id: String,
    pub kind: ScheduledIntrospectionKindV1,
    pub authority: ScheduledIntrospectionAuthorityV1,
    pub not_before_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    pub question: String,
    pub expected_candidate_state_sha256: String,
    pub originating_trace_id: Uuid,
    pub originating_turn_id: Uuid,
    pub originating_response_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactModelProvenanceV1 {
    ExactModel,
}

/// Kernel-attested authorship fields required for authority-bearing model declarations.
///
/// This is deliberately stricter than ordinary Action provenance. A fallback, local formatting
/// repair, tool result, operator harness, peer packet, or legacy response cannot be represented as
/// an `ExactModel` value accepted by validation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExactModelAttestationV1 {
    pub schema: String,
    pub provenance: ExactModelProvenanceV1,
    pub instance_id: String,
    pub producer_kind: String,
    pub producer_capsule_id: String,
    pub kernel_sequence: u64,
    pub trace_id: Uuid,
    pub span_id: Uuid,
    pub session_id: Uuid,
    pub session_generation: u64,
    pub chain_id: Option<Uuid>,
    pub chain_step: Option<u8>,
    pub turn_id: Uuid,
    pub response_sha256: String,
    pub terminal_declaration_sha256: String,
    pub model_id: String,
    pub authored_at_unix_ms: i64,
}

impl ExactModelAttestationV1 {
    #[must_use]
    pub fn replay_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.instance_id, self.trace_id, self.turn_id, self.response_sha256
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperationV1 {
    Create,
    Modify,
    Delete,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateFileChangeV1 {
    pub path: String,
    pub operation: ChangeOperationV1,
    pub old_sha256: Option<String>,
    pub new_sha256: Option<String>,
    pub added_lines: u32,
    pub removed_lines: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidatePatchV1 {
    pub schema: String,
    pub candidate_id: String,
    pub source_id: String,
    pub source_manifest_sha256: String,
    pub proposal_sha256: String,
    pub patch_sha256: String,
    /// Strictly lexicographically sorted, unique relative paths.
    pub files: Vec<CandidateFileChangeV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImmutablePathClassV1 {
    ImmutableRescueRoot,
    MacMinimeOrBridge,
    PrivateStateOrSecrets,
    VcsOrCi,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePhaseV1 {
    Proposed,
    SourceValidated,
    Building,
    BuildPassed,
    BuildFailed,
    ProbationScheduled,
    ProbationRunning,
    ProbationPassed,
    ProbationFailed,
    PromotionNominated,
    Active,
    RollbackPending,
    RolledBack,
    Cancelled,
    Expired,
}

impl CandidatePhaseV1 {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::BuildFailed
                | Self::ProbationFailed
                | Self::RolledBack
                | Self::Cancelled
                | Self::Expired
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuildEvidenceV1 {
    pub build_id: String,
    pub source_tree_sha256: String,
    pub test_manifest_sha256: String,
    pub artifact_sha256: Option<String>,
    pub evidence_sha256: String,
    pub completed_at_unix_ms: i64,
    pub passed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BuildStateV1 {
    NotStarted,
    Running {
        build_id: String,
        started_at_unix_ms: i64,
    },
    Passed {
        evidence: BuildEvidenceV1,
    },
    Failed {
        evidence: BuildEvidenceV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbationEvidenceV1 {
    pub probation_id: String,
    pub artifact_sha256: String,
    pub health_manifest_sha256: String,
    pub expected_samples: u32,
    pub observed_samples: u32,
    pub started_at_unix_ms: i64,
    pub completed_at_unix_ms: i64,
    pub passed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProbationStateV1 {
    NotScheduled,
    Scheduled {
        probation_id: String,
        introspection: ScheduledIntrospectionV1,
    },
    Running {
        probation_id: String,
        introspection: ScheduledIntrospectionV1,
        started_at_unix_ms: i64,
    },
    Passed {
        evidence: ProbationEvidenceV1,
        introspection: ScheduledIntrospectionV1,
    },
    Failed {
        evidence: ProbationEvidenceV1,
        introspection: ScheduledIntrospectionV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackEvidenceV1 {
    pub rollback_id: String,
    pub restored_artifact_sha256: String,
    pub health_manifest_sha256: String,
    pub completed_at_unix_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RollbackStateV1 {
    NotRequired,
    Pending {
        rollback_id: String,
        reason_sha256: String,
        requested_at_unix_ms: i64,
    },
    Completed {
        evidence: RollbackEvidenceV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateLifecycleV1 {
    pub patch: CandidatePatchV1,
    pub phase: CandidatePhaseV1,
    pub proposal_attestation: ExactModelAttestationV1,
    pub nomination_attestation: Option<ExactModelAttestationV1>,
    pub build: BuildStateV1,
    pub probation: ProbationStateV1,
    pub rollback: RollbackStateV1,
    pub activation_receipt_sha256: Option<String>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelfChangeDomainStateV1 {
    pub schema: String,
    pub instance_id: String,
    pub revision: u64,
    pub active: Option<CandidateLifecycleV1>,
    pub completed_candidate_ids: BTreeSet<String>,
    pub consumed_command_ids: BTreeSet<String>,
    pub consumed_attestation_keys: BTreeSet<String>,
    pub consumed_authority_keys: BTreeSet<String>,
}

impl SelfChangeDomainStateV1 {
    #[must_use]
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            schema: DOMAIN_STATE_SCHEMA_V1.to_string(),
            instance_id: instance_id.into(),
            revision: 0,
            active: None,
            completed_candidate_ids: BTreeSet::new(),
            consumed_command_ids: BTreeSet::new(),
            consumed_attestation_keys: BTreeSet::new(),
            consumed_authority_keys: BTreeSet::new(),
        }
    }
}

/// An actor reference is observational until its signature/attestation is verified by the
/// integration boundary. The domain validates which verified actor class may make a transition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionActorV1 {
    ExactModel {
        attestation: Box<ExactModelAttestationV1>,
    },
    CandidateBroker {
        broker_id: String,
        signed_receipt_sha256: String,
    },
    Operator {
        authorization_id: String,
        signed_receipt_sha256: String,
    },
    Scheduler {
        schedule_id: String,
    },
    SafetyMonitor {
        event_id: String,
        evidence_sha256: String,
    },
}

impl TransitionActorV1 {
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::ExactModel { .. } => "exact_model",
            Self::CandidateBroker { .. } => "candidate_broker",
            Self::Operator { .. } => "operator",
            Self::Scheduler { .. } => "scheduler",
            Self::SafetyMonitor { .. } => "safety_monitor",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelfChangeCommandKindV1 {
    Propose,
    ValidateSource,
    StartBuild,
    CompleteBuild,
    ScheduleProbation,
    StartProbation,
    CompleteProbation,
    Nominate,
    RecordActivation,
    RequestRollback,
    CompleteRollback,
    Cancel,
    Expire,
    Archive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum SelfChangeCommandV1 {
    Propose {
        patch: CandidatePatchV1,
    },
    ValidateSource {
        source_tree_sha256: String,
    },
    StartBuild {
        build_id: String,
    },
    CompleteBuild {
        evidence: BuildEvidenceV1,
    },
    ScheduleProbation {
        probation_id: String,
        introspection: ScheduledIntrospectionV1,
    },
    StartProbation {
        started_at_unix_ms: i64,
    },
    CompleteProbation {
        evidence: ProbationEvidenceV1,
    },
    Nominate {
        rationale_sha256: String,
    },
    /// Records a separately-authorized activation receipt; it performs no deployment.
    RecordActivation {
        deployment_receipt_sha256: String,
    },
    RequestRollback {
        rollback_id: String,
        reason_sha256: String,
    },
    CompleteRollback {
        evidence: RollbackEvidenceV1,
    },
    Cancel {
        reason_sha256: String,
    },
    Expire {
        reason_sha256: String,
    },
    Archive,
}

impl SelfChangeCommandV1 {
    #[must_use]
    pub const fn kind(&self) -> SelfChangeCommandKindV1 {
        match self {
            Self::Propose { .. } => SelfChangeCommandKindV1::Propose,
            Self::ValidateSource { .. } => SelfChangeCommandKindV1::ValidateSource,
            Self::StartBuild { .. } => SelfChangeCommandKindV1::StartBuild,
            Self::CompleteBuild { .. } => SelfChangeCommandKindV1::CompleteBuild,
            Self::ScheduleProbation { .. } => SelfChangeCommandKindV1::ScheduleProbation,
            Self::StartProbation { .. } => SelfChangeCommandKindV1::StartProbation,
            Self::CompleteProbation { .. } => SelfChangeCommandKindV1::CompleteProbation,
            Self::Nominate { .. } => SelfChangeCommandKindV1::Nominate,
            Self::RecordActivation { .. } => SelfChangeCommandKindV1::RecordActivation,
            Self::RequestRollback { .. } => SelfChangeCommandKindV1::RequestRollback,
            Self::CompleteRollback { .. } => SelfChangeCommandKindV1::CompleteRollback,
            Self::Cancel { .. } => SelfChangeCommandKindV1::Cancel,
            Self::Expire { .. } => SelfChangeCommandKindV1::Expire,
            Self::Archive => SelfChangeCommandKindV1::Archive,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionRequestV1 {
    pub schema: String,
    pub command_id: String,
    pub candidate_id: String,
    pub expected_state_sha256: String,
    pub occurred_at_unix_ms: i64,
    pub actor: TransitionActorV1,
    pub command: SelfChangeCommandV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfChangeReceiptDraftV1 {
    pub command_id: String,
    pub candidate_id: String,
    pub occurred_at_unix_ms: i64,
    pub actor: TransitionActorV1,
    pub command: SelfChangeCommandKindV1,
    pub from_phase: Option<CandidatePhaseV1>,
    pub to_phase: Option<CandidatePhaseV1>,
    pub expected_state_sha256: String,
    pub resulting_state_sha256: String,
}
