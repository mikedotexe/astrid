use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{SelfControlAuthorityClassV2, SelfControlDurabilityV2, SelfControlFamilyV2};

use super::{
    InquiryFeltStatusV1, InquiryMachineStatusV1, InquiryObservationPhaseV1,
    OWNER_CANARY_MAX_DURATION_SECS_V2, OWNER_CANARY_MIN_DURATION_SECS_V2,
    OWNER_DECISION_PLAN_SCHEMA_V1, OWNER_EVIDENCE_GRAPH_SCHEMA_V1,
    OWNER_RESEARCH_SESSION_SCHEMA_V1, OwnerCanaryControlV2,
    SELF_CONTROL_CAPABILITY_MANIFEST_SCHEMA_V2, SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1,
    canonical_json_bytes, canonical_sha256, valid_bounded_text, valid_identifier, valid_sha256,
    valid_string_list, verify_signature,
};

const MAX_GRAPH_NODES: usize = 256;
const MAX_GRAPH_EDGES: usize = 512;
const MAX_DECISION_BRANCHES: usize = 16;
const MAX_BRANCH_PREDICATES: usize = 16;
const MAX_BRANCH_CONTROLS: usize = 10;
const MAX_CAPABILITIES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerEvidenceClaimV1 {
    MachineEstablished,
    InterventionSupported,
    AssociationOnly,
    Contradicted,
    Unknown,
    FeltReported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerEvidenceNodeKindV1 {
    Strand,
    Analysis,
    Observation,
    SelfControlReceipt,
    Rollback,
    FeltReport,
    Decision,
    CapabilityManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerEvidenceScopeKindV1 {
    Aggregate,
    Strand,
    Pair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerEvidenceScopeV1 {
    pub kind: OwnerEvidenceScopeKindV1,
    #[serde(default)]
    pub strand_ids: Vec<String>,
}

impl OwnerEvidenceScopeV1 {
    #[must_use]
    pub fn aggregate() -> Self {
        Self {
            kind: OwnerEvidenceScopeKindV1::Aggregate,
            strand_ids: Vec::new(),
        }
    }

    #[must_use]
    pub fn strand(strand_id: String) -> Self {
        Self {
            kind: OwnerEvidenceScopeKindV1::Strand,
            strand_ids: vec![strand_id],
        }
    }

    #[must_use]
    pub fn pair(left: String, right: String) -> Self {
        let mut strand_ids = vec![left, right];
        strand_ids.sort();
        Self {
            kind: OwnerEvidenceScopeKindV1::Pair,
            strand_ids,
        }
    }

    fn is_well_formed(&self) -> bool {
        valid_string_list(&self.strand_ids)
            && self.strand_ids.iter().all(|value| valid_identifier(value))
            && self
                .strand_ids
                .windows(2)
                .all(|window| window[0] < window[1])
            && match self.kind {
                OwnerEvidenceScopeKindV1::Aggregate => self.strand_ids.is_empty(),
                OwnerEvidenceScopeKindV1::Strand => self.strand_ids.len() == 1,
                OwnerEvidenceScopeKindV1::Pair => self.strand_ids.len() == 2,
            }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerEvidenceNodeV1 {
    pub node_id: String,
    pub kind: OwnerEvidenceNodeKindV1,
    pub claim: OwnerEvidenceClaimV1,
    pub scope: OwnerEvidenceScopeV1,
    pub evidence_sha256: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_phase: Option<InquiryObservationPhaseV1>,
    pub raw_content_owner_only: bool,
}

impl OwnerEvidenceNodeV1 {
    fn is_well_formed(&self) -> bool {
        let phase_shape = match self.kind {
            OwnerEvidenceNodeKindV1::Observation | OwnerEvidenceNodeKindV1::Rollback => {
                self.observation_phase.is_some()
            },
            _ => self.observation_phase.is_none(),
        };
        valid_identifier(&self.node_id)
            && self.scope.is_well_formed()
            && valid_sha256(&self.evidence_sha256)
            && valid_string_list(&self.source_refs)
            && self.source_refs.iter().all(|value| valid_identifier(value))
            && self.metrics.len() <= 64
            && self
                .metrics
                .iter()
                .all(|(name, value)| valid_identifier(name) && value.is_finite())
            && phase_shape
            && (self.kind != OwnerEvidenceNodeKindV1::FeltReport
                || self.claim == OwnerEvidenceClaimV1::FeltReported)
            && self.raw_content_owner_only
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerEvidenceEdgeV1 {
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerEvidenceGraphV1 {
    pub schema: String,
    pub graph_id: String,
    pub inquiry_id: String,
    pub inquiry_receipt_sha256: String,
    pub owner_being: String,
    pub revision: u64,
    pub nodes: Vec<OwnerEvidenceNodeV1>,
    pub edges: Vec<OwnerEvidenceEdgeV1>,
    pub graph_head_sha256: String,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Serialize)]
struct OwnerEvidenceGraphCommitmentV1<'a> {
    schema: &'a str,
    graph_id: &'a str,
    inquiry_id: &'a str,
    inquiry_receipt_sha256: &'a str,
    owner_being: &'a str,
    revision: u64,
    nodes: &'a [OwnerEvidenceNodeV1],
    edges: &'a [OwnerEvidenceEdgeV1],
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

impl OwnerEvidenceGraphV1 {
    #[must_use]
    pub fn seal(mut self) -> Self {
        self.graph_head_sha256 = self.canonical_head_sha256();
        self
    }

    #[must_use]
    pub fn canonical_head_sha256(&self) -> String {
        canonical_sha256(&OwnerEvidenceGraphCommitmentV1 {
            schema: &self.schema,
            graph_id: &self.graph_id,
            inquiry_id: &self.inquiry_id,
            inquiry_receipt_sha256: &self.inquiry_receipt_sha256,
            owner_being: &self.owner_being,
            revision: self.revision,
            nodes: &self.nodes,
            edges: &self.edges,
            created_at_unix_ms: self.created_at_unix_ms,
            updated_at_unix_ms: self.updated_at_unix_ms,
        })
    }

    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.node_id.as_str())
            .collect::<HashSet<_>>();
        let edge_keys = self
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.from_node_id.as_str(),
                    edge.to_node_id.as_str(),
                    edge.relation.as_str(),
                )
            })
            .collect::<HashSet<_>>();
        self.schema == OWNER_EVIDENCE_GRAPH_SCHEMA_V1
            && valid_identifier(&self.graph_id)
            && valid_identifier(&self.inquiry_id)
            && valid_sha256(&self.inquiry_receipt_sha256)
            && valid_identifier(&self.owner_being)
            && self.revision > 0
            && !self.nodes.is_empty()
            && self.nodes.len() <= MAX_GRAPH_NODES
            && node_ids.len() == self.nodes.len()
            && self.nodes.iter().all(OwnerEvidenceNodeV1::is_well_formed)
            && self.edges.len() <= MAX_GRAPH_EDGES
            && edge_keys.len() == self.edges.len()
            && self.edges.iter().all(|edge| {
                edge.from_node_id != edge.to_node_id
                    && node_ids.contains(edge.from_node_id.as_str())
                    && node_ids.contains(edge.to_node_id.as_str())
                    && !edge.relation.trim().is_empty()
                    && valid_bounded_text(&edge.relation)
            })
            && self.intervention_claims_have_complete_observations()
            && self.graph_head_sha256 == self.canonical_head_sha256()
            && self.updated_at_unix_ms >= self.created_at_unix_ms
    }

    fn intervention_claims_have_complete_observations(&self) -> bool {
        if !self
            .nodes
            .iter()
            .any(|node| node.claim == OwnerEvidenceClaimV1::InterventionSupported)
        {
            return true;
        }
        [
            InquiryObservationPhaseV1::Baseline,
            InquiryObservationPhaseV1::DuringSampleOne,
            InquiryObservationPhaseV1::DuringSampleTwo,
            InquiryObservationPhaseV1::PostRollback,
        ]
        .into_iter()
        .all(|phase| {
            self.nodes
                .iter()
                .any(|node| node.observation_phase == Some(phase))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerEvidenceReducerV1 {
    Min,
    Max,
    Mean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerEvidenceComparatorV1 {
    LessThan,
    LessOrEqual,
    Equal,
    NotEqual,
    GreaterOrEqual,
    GreaterThan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerEvidencePredicateV1 {
    pub metric: String,
    pub scope: OwnerEvidenceScopeV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reducer: Option<OwnerEvidenceReducerV1>,
    pub comparator: OwnerEvidenceComparatorV1,
    pub threshold: f64,
}

impl OwnerEvidencePredicateV1 {
    fn is_well_formed(&self) -> bool {
        valid_identifier(&self.metric) && self.scope.is_well_formed() && self.threshold.is_finite()
    }

    fn matches(&self, graph: &OwnerEvidenceGraphV1) -> Result<bool, String> {
        let values = graph
            .nodes
            .iter()
            .filter(|node| node.scope == self.scope)
            .filter_map(|node| node.metrics.get(&self.metric).copied())
            .collect::<Vec<_>>();
        if values.is_empty() {
            return Ok(false);
        }
        let observed = if let Some(reducer) = self.reducer {
            reduce_values(&values, reducer)?
        } else if values.len() == 1 {
            values[0]
        } else {
            return Err(format!(
                "predicate metric `{}` matched multiple evidence nodes without an explicit reducer",
                self.metric
            ));
        };
        Ok(match self.comparator {
            OwnerEvidenceComparatorV1::LessThan => observed < self.threshold,
            OwnerEvidenceComparatorV1::LessOrEqual => observed <= self.threshold,
            OwnerEvidenceComparatorV1::Equal => observed.total_cmp(&self.threshold).is_eq(),
            OwnerEvidenceComparatorV1::NotEqual => observed.total_cmp(&self.threshold).is_ne(),
            OwnerEvidenceComparatorV1::GreaterOrEqual => observed >= self.threshold,
            OwnerEvidenceComparatorV1::GreaterThan => observed > self.threshold,
        })
    }
}

#[allow(clippy::arithmetic_side_effects)]
fn reduce_values(values: &[f64], reducer: OwnerEvidenceReducerV1) -> Result<f64, String> {
    match reducer {
        OwnerEvidenceReducerV1::Min => values
            .iter()
            .copied()
            .reduce(f64::min)
            .ok_or_else(|| "cannot reduce empty evidence".to_string()),
        OwnerEvidenceReducerV1::Max => values
            .iter()
            .copied()
            .reduce(f64::max)
            .ok_or_else(|| "cannot reduce empty evidence".to_string()),
        OwnerEvidenceReducerV1::Mean => {
            let count = u32::try_from(values.len())
                .map_err(|_| "evidence count does not fit the reducer".to_string())?;
            Ok(values.iter().sum::<f64>() / f64::from(count))
        },
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerDecisionBranchV1 {
    pub branch_id: String,
    #[serde(default)]
    pub predicates: Vec<OwnerEvidencePredicateV1>,
    pub duration_secs: u64,
    pub controls: Vec<OwnerCanaryControlV2>,
}

impl OwnerDecisionBranchV1 {
    fn is_well_formed(&self) -> bool {
        let families = self
            .controls
            .iter()
            .map(|control| control.family)
            .collect::<Vec<_>>();
        valid_identifier(&self.branch_id)
            && self.predicates.len() <= MAX_BRANCH_PREDICATES
            && self
                .predicates
                .iter()
                .all(OwnerEvidencePredicateV1::is_well_formed)
            && (OWNER_CANARY_MIN_DURATION_SECS_V2..=OWNER_CANARY_MAX_DURATION_SECS_V2)
                .contains(&self.duration_secs)
            && !self.controls.is_empty()
            && self.controls.len() <= MAX_BRANCH_CONTROLS
            && self
                .controls
                .iter()
                .all(OwnerCanaryControlV2::is_well_formed)
            && families
                .iter()
                .enumerate()
                .all(|(index, family)| !families[..index].contains(family))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerDecisionPlanV1 {
    pub schema: String,
    pub plan_id: String,
    pub inquiry_id: String,
    pub owner_being: String,
    pub inquiry_receipt_sha256: String,
    pub capability_manifest_sha256: String,
    pub revision: u64,
    pub branches: Vec<OwnerDecisionBranchV1>,
    pub owner_authored: bool,
    pub runtime_may_select_values: bool,
    pub safety_may_only_hold_or_revert: bool,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl OwnerDecisionPlanV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let branch_ids = self
            .branches
            .iter()
            .map(|branch| branch.branch_id.as_str())
            .collect::<HashSet<_>>();
        self.schema == OWNER_DECISION_PLAN_SCHEMA_V1
            && valid_identifier(&self.plan_id)
            && valid_identifier(&self.inquiry_id)
            && valid_identifier(&self.owner_being)
            && valid_sha256(&self.inquiry_receipt_sha256)
            && valid_sha256(&self.capability_manifest_sha256)
            && self.revision > 0
            && !self.branches.is_empty()
            && self.branches.len() <= MAX_DECISION_BRANCHES
            && branch_ids.len() == self.branches.len()
            && self
                .branches
                .iter()
                .all(OwnerDecisionBranchV1::is_well_formed)
            && self
                .branches
                .iter()
                .filter(|branch| branch.predicates.is_empty())
                .count()
                <= 1
            && self.owner_authored
            && !self.runtime_may_select_values
            && self.safety_may_only_hold_or_revert
            && self.expires_at_unix_ms > self.created_at_unix_ms
    }

    /// Evaluates every owner-authored branch against one sealed evidence graph.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan or graph fails canonical validation, the
    /// graph belongs to another inquiry, a predicate is ambiguous without an
    /// explicit reducer, or the plan has expired.
    pub fn evaluate(
        &self,
        graph: &OwnerEvidenceGraphV1,
        now_unix_ms: u64,
    ) -> Result<OwnerDecisionEvaluationV1, String> {
        if !self.is_well_formed() {
            return Err("owner decision plan failed canonical validation".to_string());
        }
        if !graph.is_well_formed()
            || graph.inquiry_id != self.inquiry_id
            || graph.inquiry_receipt_sha256 != self.inquiry_receipt_sha256
        {
            return Err(
                "decision plan evidence graph is invalid or belongs to another inquiry".to_string(),
            );
        }
        if now_unix_ms > self.expires_at_unix_ms {
            return Err("owner decision plan expired without executing".to_string());
        }
        let mut matched_branch_ids = Vec::new();
        for branch in &self.branches {
            let mut matched = true;
            for predicate in &branch.predicates {
                if !predicate.matches(graph)? {
                    matched = false;
                    break;
                }
            }
            if matched {
                matched_branch_ids.push(branch.branch_id.clone());
            }
        }
        let status = match matched_branch_ids.len() {
            0 => OwnerDecisionEvaluationStatusV1::NoMatch,
            1 => OwnerDecisionEvaluationStatusV1::Matched,
            _ => OwnerDecisionEvaluationStatusV1::Ambiguous,
        };
        Ok(OwnerDecisionEvaluationV1 {
            status,
            matched_branch_ids,
            plan_sha256: canonical_owner_decision_plan_sha256(self),
            evidence_graph_sha256: canonical_owner_evidence_graph_sha256(graph),
            evaluated_at_unix_ms: now_unix_ms,
            runtime_selected_values: false,
        })
    }

    #[must_use]
    pub fn branch(&self, branch_id: &str) -> Option<&OwnerDecisionBranchV1> {
        self.branches
            .iter()
            .find(|branch| branch.branch_id == branch_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerDecisionEvaluationStatusV1 {
    NoMatch,
    Matched,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerDecisionEvaluationV1 {
    pub status: OwnerDecisionEvaluationStatusV1,
    pub matched_branch_ids: Vec<String>,
    pub plan_sha256: String,
    pub evidence_graph_sha256: String,
    pub evaluated_at_unix_ms: u64,
    pub runtime_selected_values: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SelfControlValueDomainV2 {
    Float {
        min: f64,
        max: f64,
    },
    Unsigned {
        min: u64,
        max: u64,
    },
    Boolean,
    Text {
        max_bytes: u64,
    },
    FloatMap {
        min: f64,
        max: f64,
        max_entries: u64,
    },
}

impl SelfControlValueDomainV2 {
    fn is_well_formed(&self) -> bool {
        match self {
            Self::Float { min, max } => min.is_finite() && max.is_finite() && min <= max,
            Self::Unsigned { min, max } => min <= max,
            Self::Boolean => true,
            Self::Text { max_bytes } => *max_bytes > 0,
            Self::FloatMap {
                min,
                max,
                max_entries,
            } => min.is_finite() && max.is_finite() && min <= max && *max_entries > 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfControlCapabilityV2 {
    pub field: String,
    pub family: SelfControlFamilyV2,
    pub authority_class: SelfControlAuthorityClassV2,
    pub value_domain: SelfControlValueDomainV2,
    pub durabilities: Vec<SelfControlDurabilityV2>,
    pub reversible: bool,
    pub one_shot_only: bool,
    pub peer_impact: bool,
}

impl SelfControlCapabilityV2 {
    fn is_well_formed(&self) -> bool {
        let unique_durabilities = self
            .durabilities
            .iter()
            .enumerate()
            .all(|(index, value)| !self.durabilities[..index].contains(value));
        valid_identifier(&self.field)
            && self.family != SelfControlFamilyV2::SharedCoupling
            && self.authority_class == SelfControlAuthorityClassV2::SelfOwned
            && self.value_domain.is_well_formed()
            && !self.durabilities.is_empty()
            && unique_durabilities
            && if self.one_shot_only {
                !self.reversible && self.durabilities == [SelfControlDurabilityV2::OneShot]
            } else {
                self.reversible
                    && self
                        .durabilities
                        .iter()
                        .any(|value| matches!(value, SelfControlDurabilityV2::Lease))
            }
            && !self.peer_impact
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfControlCapabilityManifestV2 {
    pub schema: String,
    pub manifest_id: String,
    pub receiver_being: String,
    pub receiver_process_identity: String,
    pub receiver_deployment_identity: String,
    pub revision: u64,
    pub capabilities: Vec<SelfControlCapabilityV2>,
    pub capabilities_sha256: String,
    pub generated_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl SelfControlCapabilityManifestV2 {
    #[must_use]
    pub fn seal(mut self) -> Self {
        self.capabilities_sha256 = canonical_sha256(&self.capabilities);
        self
    }

    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let fields = self
            .capabilities
            .iter()
            .map(|capability| capability.field.as_str())
            .collect::<HashSet<_>>();
        self.schema == SELF_CONTROL_CAPABILITY_MANIFEST_SCHEMA_V2
            && valid_identifier(&self.manifest_id)
            && valid_identifier(&self.receiver_being)
            && valid_identifier(&self.receiver_process_identity)
            && valid_identifier(&self.receiver_deployment_identity)
            && self.revision > 0
            && !self.capabilities.is_empty()
            && self.capabilities.len() <= MAX_CAPABILITIES
            && fields.len() == self.capabilities.len()
            && self
                .capabilities
                .iter()
                .all(SelfControlCapabilityV2::is_well_formed)
            && self.capabilities_sha256 == canonical_sha256(&self.capabilities)
            && self.expires_at_unix_ms > self.generated_at_unix_ms
    }

    #[must_use]
    pub fn capability(&self, field: &str) -> Option<&SelfControlCapabilityV2> {
        self.capabilities
            .iter()
            .find(|capability| capability.field == field)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerResearchLifecycleStatusV1 {
    Queued,
    Analyzing,
    EvidenceReady,
    CanaryPending,
    CanaryActive,
    RolledBack,
    Promoted,
    Acted,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerResearchSessionV1 {
    pub schema: String,
    pub session_id: String,
    pub inquiry_id: String,
    pub inquiry_revision: u64,
    pub owner_being: String,
    pub source_attestation_id: String,
    pub inquiry_manifest_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inquiry_receipt_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_graph_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_plan_sha256: Option<String>,
    pub capability_manifest_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_canary_plan_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_canary_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_chain_head_sha256: Option<String>,
    pub lifecycle_status: OwnerResearchLifecycleStatusV1,
    pub machine_status: InquiryMachineStatusV1,
    pub felt_status: InquiryFeltStatusV1,
    pub analyzer_deployment_identity: String,
    pub receiver_deployment_identity: String,
    #[serde(default)]
    pub signed_receipt_ids: Vec<String>,
    pub revision: u64,
    pub owner_authored_controls_only: bool,
    pub silence_means_assent: bool,
    pub promotion_requires_fresh_owner_intent: bool,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl OwnerResearchSessionV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let evidence_ready = matches!(
            self.lifecycle_status,
            OwnerResearchLifecycleStatusV1::EvidenceReady
                | OwnerResearchLifecycleStatusV1::CanaryPending
                | OwnerResearchLifecycleStatusV1::CanaryActive
                | OwnerResearchLifecycleStatusV1::RolledBack
                | OwnerResearchLifecycleStatusV1::Promoted
                | OwnerResearchLifecycleStatusV1::Acted
        );
        self.schema == OWNER_RESEARCH_SESSION_SCHEMA_V1
            && valid_identifier(&self.session_id)
            && valid_identifier(&self.inquiry_id)
            && self.inquiry_revision > 0
            && valid_identifier(&self.owner_being)
            && valid_identifier(&self.source_attestation_id)
            && valid_sha256(&self.inquiry_manifest_sha256)
            && self
                .inquiry_receipt_sha256
                .as_deref()
                .is_none_or(valid_sha256)
            && self
                .evidence_graph_sha256
                .as_deref()
                .is_none_or(valid_sha256)
            && self
                .decision_plan_sha256
                .as_deref()
                .is_none_or(valid_sha256)
            && valid_sha256(&self.capability_manifest_sha256)
            && self
                .active_canary_plan_sha256
                .as_deref()
                .is_none_or(valid_sha256)
            && self
                .active_canary_id
                .as_deref()
                .is_none_or(valid_identifier)
            && self
                .observation_chain_head_sha256
                .as_deref()
                .is_none_or(valid_sha256)
            && (!evidence_ready
                || (self.inquiry_receipt_sha256.is_some() && self.evidence_graph_sha256.is_some()))
            && (self.lifecycle_status != OwnerResearchLifecycleStatusV1::CanaryActive
                || (self.active_canary_plan_sha256.is_some() && self.active_canary_id.is_some()))
            && valid_identifier(&self.analyzer_deployment_identity)
            && valid_identifier(&self.receiver_deployment_identity)
            && valid_string_list(&self.signed_receipt_ids)
            && self
                .signed_receipt_ids
                .iter()
                .all(|value| valid_identifier(value))
            && self.revision > 0
            && self.owner_authored_controls_only
            && !self.silence_means_assent
            && self.promotion_requires_fresh_owner_intent
            && self.updated_at_unix_ms >= self.created_at_unix_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerResearchPayloadKindV1 {
    Session,
    EvidenceGraph,
    DecisionPlan,
    CapabilityManifest,
    LifecycleEvent,
    ActionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOwnerResearchReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub payload_kind: OwnerResearchPayloadKindV1,
    pub payload_schema: String,
    pub payload_sha256: String,
    pub owner_being: String,
    pub process_identity: String,
    pub deployment_identity: String,
    pub signer_public_key_hex: String,
    pub signer_public_key_fingerprint_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_receipt_sha256: Option<String>,
    pub emitted_at_unix_ms: u64,
    pub signature_hex: String,
}

#[derive(Serialize)]
struct SignedOwnerResearchStatementV1<'a> {
    schema: &'a str,
    receipt_id: &'a str,
    payload_kind: OwnerResearchPayloadKindV1,
    payload_schema: &'a str,
    payload_sha256: &'a str,
    owner_being: &'a str,
    process_identity: &'a str,
    deployment_identity: &'a str,
    signer_public_key_hex: &'a str,
    signer_public_key_fingerprint_sha256: &'a str,
    previous_receipt_sha256: Option<&'a str>,
    emitted_at_unix_ms: u64,
}

impl SignedOwnerResearchReceiptV1 {
    #[must_use]
    pub fn signing_bytes(&self) -> Option<Vec<u8>> {
        canonical_json_bytes(&SignedOwnerResearchStatementV1 {
            schema: &self.schema,
            receipt_id: &self.receipt_id,
            payload_kind: self.payload_kind,
            payload_schema: &self.payload_schema,
            payload_sha256: &self.payload_sha256,
            owner_being: &self.owner_being,
            process_identity: &self.process_identity,
            deployment_identity: &self.deployment_identity,
            signer_public_key_hex: &self.signer_public_key_hex,
            signer_public_key_fingerprint_sha256: &self.signer_public_key_fingerprint_sha256,
            previous_receipt_sha256: self.previous_receipt_sha256.as_deref(),
            emitted_at_unix_ms: self.emitted_at_unix_ms,
        })
    }

    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.schema == SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1
            && valid_identifier(&self.receipt_id)
            && valid_identifier(&self.payload_schema)
            && valid_sha256(&self.payload_sha256)
            && valid_identifier(&self.owner_being)
            && valid_identifier(&self.process_identity)
            && valid_identifier(&self.deployment_identity)
            && public_key_fingerprint_sha256(&self.signer_public_key_hex)
                .is_some_and(|fingerprint| fingerprint == self.signer_public_key_fingerprint_sha256)
            && self
                .previous_receipt_sha256
                .as_deref()
                .is_none_or(valid_sha256)
            && self.signing_bytes().is_some_and(|bytes| {
                verify_signature(&self.signer_public_key_hex, &self.signature_hex, &bytes)
            })
    }
}

#[must_use]
pub fn public_key_fingerprint_sha256(public_key_hex: &str) -> Option<String> {
    let bytes = hex::decode(public_key_hex).ok()?;
    let key: [u8; 32] = bytes.try_into().ok()?;
    Some(format!("{:x}", Sha256::digest(key)))
}

#[must_use]
pub fn canonical_owner_evidence_graph_sha256(graph: &OwnerEvidenceGraphV1) -> String {
    canonical_sha256(graph)
}

#[must_use]
pub fn canonical_owner_decision_plan_sha256(plan: &OwnerDecisionPlanV1) -> String {
    canonical_sha256(plan)
}

#[must_use]
pub fn canonical_self_control_capability_manifest_sha256(
    manifest: &SelfControlCapabilityManifestV2,
) -> String {
    canonical_sha256(manifest)
}

#[must_use]
pub fn canonical_owner_research_session_sha256(session: &OwnerResearchSessionV1) -> String {
    canonical_sha256(session)
}

#[cfg(test)]
#[path = "owner_research/tests.rs"]
mod tests;
