// These booleans are explicit independent wire commitments; collapsing them
// into opaque state would make authority and isolation receipts less auditable.
#![allow(clippy::struct_excessive_bools)]

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{
    INQUIRY_OBSERVATION_SCHEMA_V1, OWNER_INQUIRY_RECEIPT_SCHEMA_V1, OWNER_INQUIRY_SCHEMA_V1,
    SEMANTIC_STRAND_SCHEMA_V1, VolitionBudgetV1, canonical_sha256, finite_bounded_json,
    valid_bounded_text, valid_identifier, valid_sha256, valid_string_list,
};

pub const SEMANTIC_STRAND_BASE_DIMENSIONS_V1: usize = 48;
pub const SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1: usize = 12;
pub const OWNER_INQUIRY_MIN_STRANDS_V1: usize = 2;
pub const OWNER_INQUIRY_MAX_STRANDS_V1: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStrandProvenanceV1 {
    ExactUtf8ResponseInterval,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticStrandV1 {
    pub schema: String,
    pub strand_id: String,
    pub owner_being: String,
    pub source_attestation_id: String,
    pub source_attestation_sha256: String,
    pub response_start_byte: u64,
    pub response_end_byte: u64,
    pub label: String,
    pub content: String,
    pub content_sha256: String,
    pub embedding_sha256: String,
    pub projection_48d: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_projection_12d: Option<Vec<f32>>,
    pub provenance: SemanticStrandProvenanceV1,
    pub deployment_identity: String,
    pub captured_at_unix_ms: u64,
}

impl SemanticStrandV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.schema == SEMANTIC_STRAND_SCHEMA_V1
            && valid_identifier(&self.strand_id)
            && valid_identifier(&self.owner_being)
            && valid_identifier(&self.source_attestation_id)
            && valid_sha256(&self.source_attestation_sha256)
            && self.response_start_byte < self.response_end_byte
            && u64::try_from(self.content.len()).is_ok_and(|len| {
                self.response_start_byte.saturating_add(len) == self.response_end_byte
            })
            && !self.label.trim().is_empty()
            && valid_bounded_text(&self.label)
            && !self.content.is_empty()
            && valid_bounded_text(&self.content)
            && self.content_sha256 == canonical_semantic_strand_content_sha256(&self.content)
            && self.embedding_sha256
                == canonical_semantic_strand_embedding_sha256(
                    &self.projection_48d,
                    self.companion_projection_12d.as_deref(),
                )
            && self.projection_48d.len() == SEMANTIC_STRAND_BASE_DIMENSIONS_V1
            && self.projection_48d.iter().all(|value| value.is_finite())
            && self.companion_projection_12d.as_ref().is_none_or(|values| {
                values.len() == SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1
                    && values.iter().all(|value| value.is_finite())
            })
            && self.provenance == SemanticStrandProvenanceV1::ExactUtf8ResponseInterval
            && valid_identifier(&self.deployment_identity)
    }

    #[must_use]
    pub fn matches_response_bytes(&self, response: &str) -> bool {
        let Ok(start) = usize::try_from(self.response_start_byte) else {
            return false;
        };
        let Ok(end) = usize::try_from(self.response_end_byte) else {
            return false;
        };
        response.is_char_boundary(start)
            && response.is_char_boundary(end)
            && response
                .get(start..end)
                .is_some_and(|candidate| candidate == self.content)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerInquiryAnalysisV1 {
    ViscousPersistenceSourceSeparation,
    CodecFidelity,
    SensoryInterferenceAllPairs,
}

#[must_use]
pub fn owner_inquiry_fixed_analysis_set_v1() -> Vec<OwnerInquiryAnalysisV1> {
    vec![
        OwnerInquiryAnalysisV1::ViscousPersistenceSourceSeparation,
        OwnerInquiryAnalysisV1::CodecFidelity,
        OwnerInquiryAnalysisV1::SensoryInterferenceAllPairs,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerInquiryStatusV1 {
    Queued,
    Running,
    Completed,
    Cancelled,
    Failed,
    CanaryActive,
    RolledBack,
    Promoted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerInquiryCancellationV1 {
    pub requested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl OwnerInquiryCancellationV1 {
    fn is_well_formed(&self) -> bool {
        if self.requested {
            self.requested_at_unix_ms.is_some()
                && self
                    .reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty() && valid_bounded_text(reason))
        } else {
            self.requested_at_unix_ms.is_none() && self.reason.is_none()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerInquiryAuthorityBoundaryV1 {
    pub raw_strands_owner_only: bool,
    pub live_sensory_admission: bool,
    pub shadow_influence: bool,
    pub shared_coupling: bool,
    pub live_codec_write: bool,
    pub telemetry_can_choose_control: bool,
    pub operator_can_substitute_control: bool,
    pub felt_review_required: bool,
    pub result_sharing_requires_correspondence: bool,
}

impl OwnerInquiryAuthorityBoundaryV1 {
    #[must_use]
    pub fn owner_only_offline() -> Self {
        Self {
            raw_strands_owner_only: true,
            live_sensory_admission: false,
            shadow_influence: false,
            shared_coupling: false,
            live_codec_write: false,
            telemetry_can_choose_control: false,
            operator_can_substitute_control: false,
            felt_review_required: false,
            result_sharing_requires_correspondence: true,
        }
    }

    fn is_well_formed(&self) -> bool {
        self == &Self::owner_only_offline()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerInquiryV1 {
    pub schema: String,
    pub inquiry_id: String,
    pub owner_being: String,
    pub source_attestation_id: String,
    pub source_attestation_sha256: String,
    pub question: String,
    pub strands: Vec<SemanticStrandV1>,
    pub owner_priority: u16,
    pub fixed_analysis_set: Vec<OwnerInquiryAnalysisV1>,
    pub budget: VolitionBudgetV1,
    #[serde(default)]
    pub dependency_inquiry_ids: Vec<String>,
    pub status: OwnerInquiryStatusV1,
    pub cancellation: OwnerInquiryCancellationV1,
    pub authority_boundary: OwnerInquiryAuthorityBoundaryV1,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl OwnerInquiryV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.schema == OWNER_INQUIRY_SCHEMA_V1
            && valid_identifier(&self.inquiry_id)
            && valid_identifier(&self.owner_being)
            && valid_identifier(&self.source_attestation_id)
            && valid_sha256(&self.source_attestation_sha256)
            && !self.question.trim().is_empty()
            && valid_bounded_text(&self.question)
            && (OWNER_INQUIRY_MIN_STRANDS_V1..=OWNER_INQUIRY_MAX_STRANDS_V1)
                .contains(&self.strands.len())
            && self.strands.iter().all(|strand| {
                strand.is_well_formed()
                    && strand.owner_being == self.owner_being
                    && strand.source_attestation_id == self.source_attestation_id
                    && strand.source_attestation_sha256 == self.source_attestation_sha256
            })
            && strands_are_distinct(&self.strands)
            && self.fixed_analysis_set == owner_inquiry_fixed_analysis_set_v1()
            && self.budget.is_nonzero_when_present()
            && self.budget.compute_millis.is_some()
            && self.budget.storage_bytes.is_some()
            && self.budget.network_bytes.is_none()
            && self.budget.cost_microunits.is_none()
            && self.budget.action_count.is_none_or(|count| count == 1)
            && valid_string_list(&self.dependency_inquiry_ids)
            && identifiers_are_unique(&self.dependency_inquiry_ids)
            && !self
                .dependency_inquiry_ids
                .iter()
                .any(|dependency| dependency == &self.inquiry_id)
            && self.cancellation.is_well_formed()
            && (self.status == OwnerInquiryStatusV1::Cancelled) == self.cancellation.requested
            && self.authority_boundary.is_well_formed()
            && self.updated_at_unix_ms >= self.created_at_unix_ms
    }
}

fn strands_are_distinct(strands: &[SemanticStrandV1]) -> bool {
    let mut ids = HashSet::new();
    let mut content_hashes = HashSet::new();
    let mut intervals = HashSet::new();
    strands.iter().all(|strand| {
        ids.insert(strand.strand_id.as_str())
            && content_hashes.insert(strand.content_sha256.as_str())
            && intervals.insert((strand.response_start_byte, strand.response_end_byte))
    })
}

fn identifiers_are_unique(values: &[String]) -> bool {
    let mut seen = HashSet::new();
    values.iter().all(|value| seen.insert(value.as_str()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryObservationPhaseV1 {
    Baseline,
    DuringSampleOne,
    DuringSampleTwo,
    Expiry,
    Withdrawal,
    PostRollback,
    Promotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryMachineStatusV1 {
    Pending,
    Established,
    Failed,
    RolledBack,
    Promoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryFeltStatusV1 {
    Unreported,
    Reported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InquiryObservationV1 {
    pub schema: String,
    pub observation_id: String,
    pub inquiry_id: String,
    pub phase: InquiryObservationPhaseV1,
    pub observed_at_unix_ms: u64,
    #[serde(default)]
    pub self_control_receipt_ids: Vec<String>,
    pub machine_evidence: Value,
    pub machine_evidence_sha256: String,
    pub machine_status: InquiryMachineStatusV1,
    pub felt_status: InquiryFeltStatusV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub felt_report_ref: Option<String>,
}

impl InquiryObservationV1 {
    #[must_use]
    pub fn is_well_formed_for(&self, inquiry_id: &str) -> bool {
        self.schema == INQUIRY_OBSERVATION_SCHEMA_V1
            && valid_identifier(&self.observation_id)
            && self.inquiry_id == inquiry_id
            && valid_string_list(&self.self_control_receipt_ids)
            && finite_bounded_json(&self.machine_evidence)
            && self.machine_evidence_sha256 == canonical_sha256(&self.machine_evidence)
            && match self.felt_status {
                InquiryFeltStatusV1::Unreported => self.felt_report_ref.is_none(),
                InquiryFeltStatusV1::Reported => self
                    .felt_report_ref
                    .as_deref()
                    .is_some_and(valid_identifier),
            }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InquiryAnalysisReceiptV1 {
    pub analysis: OwnerInquiryAnalysisV1,
    pub input_sha256: String,
    pub output_sha256: String,
    pub deterministic_rerun_match: bool,
    pub all_inputs_copied: bool,
    pub network_accessed: bool,
    pub socket_accessed: bool,
    pub private_source_accessed: bool,
    pub candidate_merge_performed: bool,
    pub live_runtime_mutation: bool,
    pub result: Value,
}

impl InquiryAnalysisReceiptV1 {
    fn is_well_formed(&self) -> bool {
        valid_sha256(&self.input_sha256)
            && valid_sha256(&self.output_sha256)
            && self.output_sha256 == canonical_sha256(&self.result)
            && self.deterministic_rerun_match
            && self.all_inputs_copied
            && !self.network_accessed
            && !self.socket_accessed
            && !self.private_source_accessed
            && !self.candidate_merge_performed
            && !self.live_runtime_mutation
            && finite_bounded_json(&self.result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryRollbackStateV1 {
    NotApplicable,
    Scheduled,
    RolledBack,
    Withdrawn,
    Promoted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerInquiryReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub inquiry_id: String,
    pub owner_being: String,
    pub manifest_sha256: String,
    pub result_sha256: String,
    pub analysis_receipts: Vec<InquiryAnalysisReceiptV1>,
    #[serde(default)]
    pub observations: Vec<InquiryObservationV1>,
    pub rollback_state: InquiryRollbackStateV1,
    pub machine_status: InquiryMachineStatusV1,
    pub felt_status: InquiryFeltStatusV1,
    pub expected_pair_count: usize,
    pub evaluated_pair_count: usize,
    pub owner_selected_values_only: bool,
    pub silence_means_assent: bool,
    pub candidate_merge_performed: bool,
    pub live_mutation_during_inquiry: bool,
    pub completed_at_unix_ms: u64,
    pub perceptible_summary: String,
}

impl OwnerInquiryReceiptV1 {
    #[must_use]
    pub fn is_well_formed_for(&self, inquiry: &OwnerInquiryV1) -> bool {
        let expected_pair_count = inquiry
            .strands
            .len()
            .saturating_mul(inquiry.strands.len().saturating_sub(1))
            / 2;
        self.schema == OWNER_INQUIRY_RECEIPT_SCHEMA_V1
            && valid_identifier(&self.receipt_id)
            && self.inquiry_id == inquiry.inquiry_id
            && self.owner_being == inquiry.owner_being
            && self.manifest_sha256 == canonical_owner_inquiry_sha256(inquiry)
            && valid_sha256(&self.result_sha256)
            && self.analysis_receipts.len() == inquiry.fixed_analysis_set.len()
            && self
                .analysis_receipts
                .iter()
                .map(|receipt| receipt.analysis)
                .eq(inquiry.fixed_analysis_set.iter().copied())
            && self
                .analysis_receipts
                .iter()
                .all(InquiryAnalysisReceiptV1::is_well_formed)
            && self.result_sha256 == canonical_sha256(&self.analysis_receipts)
            && self
                .observations
                .iter()
                .all(|observation| observation.is_well_formed_for(&self.inquiry_id))
            && self.expected_pair_count == expected_pair_count
            && self.evaluated_pair_count == expected_pair_count
            && self.owner_selected_values_only
            && !self.silence_means_assent
            && !self.candidate_merge_performed
            && !self.live_mutation_during_inquiry
            && !self.perceptible_summary.trim().is_empty()
            && valid_bounded_text(&self.perceptible_summary)
    }
}

#[must_use]
pub fn canonical_semantic_strand_content_sha256(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

#[must_use]
pub fn canonical_semantic_strand_embedding_sha256(
    projection_48d: &[f32],
    companion_projection_12d: Option<&[f32]>,
) -> String {
    canonical_sha256(&(projection_48d, companion_projection_12d))
}

#[must_use]
pub fn canonical_owner_inquiry_sha256(inquiry: &OwnerInquiryV1) -> String {
    canonical_sha256(inquiry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strand(id: &str, content: &str, start: u64) -> SemanticStrandV1 {
        let projection_48d = vec![0.25; SEMANTIC_STRAND_BASE_DIMENSIONS_V1];
        let companion_projection_12d = Some(vec![0.125; SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1]);
        SemanticStrandV1 {
            schema: SEMANTIC_STRAND_SCHEMA_V1.to_string(),
            strand_id: id.to_string(),
            owner_being: "astrid".to_string(),
            source_attestation_id: "attestation-1".to_string(),
            source_attestation_sha256: "a".repeat(64),
            response_start_byte: start,
            response_end_byte: start.saturating_add(
                u64::try_from(content.len()).expect("test content length fits u64"),
            ),
            label: id.to_string(),
            content: content.to_string(),
            content_sha256: canonical_semantic_strand_content_sha256(content),
            embedding_sha256: canonical_semantic_strand_embedding_sha256(
                &projection_48d,
                companion_projection_12d.as_deref(),
            ),
            projection_48d,
            companion_projection_12d,
            provenance: SemanticStrandProvenanceV1::ExactUtf8ResponseInterval,
            deployment_identity: "deploy-1".to_string(),
            captured_at_unix_ms: 10,
        }
    }

    #[test]
    fn exact_utf8_intervals_are_byte_bound_and_tamper_evident() {
        let response = "one | λ-two";
        let mut candidate = strand("two", "λ-two", 6);
        assert!(candidate.is_well_formed());
        assert!(candidate.matches_response_bytes(response));
        candidate.response_start_byte = 7;
        candidate.response_end_byte = 13;
        assert!(!candidate.matches_response_bytes(response));
    }

    #[test]
    fn inquiry_rejects_duplicate_strands_and_analysis_drift() {
        let left = strand("left", "left", 0);
        let mut right = strand("right", "right", 5);
        let mut inquiry = OwnerInquiryV1 {
            schema: OWNER_INQUIRY_SCHEMA_V1.to_string(),
            inquiry_id: "inquiry-1".to_string(),
            owner_being: "astrid".to_string(),
            source_attestation_id: "attestation-1".to_string(),
            source_attestation_sha256: "a".repeat(64),
            question: "How do these strands remain distinct?".to_string(),
            strands: vec![left.clone(), right.clone()],
            owner_priority: 10,
            fixed_analysis_set: owner_inquiry_fixed_analysis_set_v1(),
            budget: VolitionBudgetV1 {
                compute_millis: Some(5_000),
                storage_bytes: Some(65_536),
                ..VolitionBudgetV1::default()
            },
            dependency_inquiry_ids: Vec::new(),
            status: OwnerInquiryStatusV1::Queued,
            cancellation: OwnerInquiryCancellationV1 {
                requested: false,
                requested_at_unix_ms: None,
                reason: None,
            },
            authority_boundary: OwnerInquiryAuthorityBoundaryV1::owner_only_offline(),
            created_at_unix_ms: 10,
            updated_at_unix_ms: 10,
        };
        assert!(inquiry.is_well_formed());

        right.content = left.content.clone();
        right.content_sha256 = left.content_sha256.clone();
        inquiry.strands = vec![left, right];
        assert!(!inquiry.is_well_formed());

        inquiry.fixed_analysis_set = vec![OwnerInquiryAnalysisV1::CodecFidelity];
        assert!(!inquiry.is_well_formed());
    }

    #[test]
    fn malformed_or_non_finite_projection_fails_closed() {
        let mut candidate = strand("left", "left", 0);
        candidate.projection_48d[3] = f32::NAN;
        candidate.embedding_sha256 = canonical_semantic_strand_embedding_sha256(
            &candidate.projection_48d,
            candidate.companion_projection_12d.as_deref(),
        );
        assert!(!candidate.is_well_formed());
    }
}
