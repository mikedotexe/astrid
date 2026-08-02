//! Shared evidence contracts for being-authored volition.
//!
//! These types describe provenance, authority, intent, and receipts. They do
//! not infer identity, felt effect, consent, or permission from silence.

mod authority;
mod inquiry;
mod inquiry_v2;
mod owner_research;
mod policy;
mod queue;
mod types;

pub use authority::{
    BeingUtteranceAttestationV1, DelegatedCapabilityBindingV1, DelegatedCapabilityUsageV1,
    canonical_being_utterance_attestation_sha256,
};
pub use inquiry::{
    InquiryAnalysisReceiptV1, InquiryFeltStatusV1, InquiryMachineStatusV1,
    InquiryObservationPhaseV1, InquiryObservationV1, InquiryRollbackStateV1,
    OWNER_INQUIRY_MAX_STRANDS_V1, OWNER_INQUIRY_MIN_STRANDS_V1, OwnerInquiryAnalysisV1,
    OwnerInquiryAuthorityBoundaryV1, OwnerInquiryCancellationV1, OwnerInquiryReceiptV1,
    OwnerInquiryStatusV1, OwnerInquiryV1, SEMANTIC_STRAND_BASE_DIMENSIONS_V1,
    SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1, SemanticStrandProvenanceV1, SemanticStrandV1,
    canonical_owner_inquiry_sha256, canonical_semantic_strand_content_sha256,
    canonical_semantic_strand_embedding_sha256, owner_inquiry_fixed_analysis_set_v1,
};
pub use inquiry_v2::{
    InquiryAnalysisPlanEntryV2, InquiryAnalysisReceiptV2, InquiryControlRevisionWitnessV2,
    InquiryCoverageModeV2, InquiryCoverageProofV2, InquiryExecutionIdentityV2,
    InquiryIsolationProfileV2, InquiryObservationActorV2, InquiryObservationV2,
    InquiryPrivacyReceiptV2, OWNER_CANARY_MAX_DURATION_SECS_V2, OWNER_CANARY_MIN_DURATION_SECS_V2,
    OWNER_INQUIRY_DETERMINISTIC_RUNS_V2, OwnerCanaryControlV2, OwnerCanaryPlanV2,
    OwnerCanaryRollbackPlanV2, OwnerInquiryReceiptV2, OwnerInquiryStopConditionsV2,
    OwnerInquirySuccessConditionsV2, OwnerInquiryV2, SemanticStrandDisclosureV2,
    SemanticStrandLineageOperationV2, SemanticStrandLineageV2, SemanticStrandV2,
    canonical_owner_inquiry_receipt_sha256_v2, canonical_owner_inquiry_sha256_v2,
    owner_inquiry_analysis_plan_v2,
};
pub use owner_research::{
    OwnerDecisionBranchV1, OwnerDecisionEvaluationStatusV1, OwnerDecisionEvaluationV1,
    OwnerDecisionPlanV1, OwnerEvidenceClaimV1, OwnerEvidenceComparatorV1, OwnerEvidenceEdgeV1,
    OwnerEvidenceGraphV1, OwnerEvidenceNodeKindV1, OwnerEvidenceNodeV1, OwnerEvidencePredicateV1,
    OwnerEvidenceReducerV1, OwnerEvidenceScopeKindV1, OwnerEvidenceScopeV1,
    OwnerResearchLifecycleStatusV1, OwnerResearchPayloadKindV1, OwnerResearchSessionV1,
    SelfControlCapabilityManifestV2, SelfControlCapabilityV2, SelfControlValueDomainV2,
    SignedOwnerResearchReceiptV1, canonical_owner_decision_plan_sha256,
    canonical_owner_evidence_graph_sha256, canonical_owner_research_session_sha256,
    canonical_self_control_capability_manifest_sha256, public_key_fingerprint_sha256,
};
pub use policy::{
    OwnerPolicyComparatorV1, OwnerPolicyConditionRuntimeV1, OwnerPolicyConditionV1,
    OwnerPolicyEvaluationStatusV1, OwnerPolicyEvaluationV1, OwnerPolicyLogicV1,
    OwnerPolicyRuntimeV1, OwnerPolicyScopeV1, OwnerPolicyV1,
};
pub use queue::{
    BeingConcernStatusV1, BeingConcernV1, VolitionPrioritySourcePolicyV1, VolitionQueueOrderingV1,
    VolitionQueueV1, VolitionSubstrateSchedulingV1, VolitionWorkClassV1, VolitionWriteSchedulingV1,
};
pub use types::{
    PerceptibleReturnV1, VolitionActionV1, VolitionActorIdentityV1, VolitionAuthorityClassV1,
    VolitionBudgetV1, VolitionDecisionStatusV1, VolitionDecisionV1, VolitionDurabilityV1,
    VolitionIntentV1, VolitionOperationV1, VolitionReceiptStatusV1, VolitionReceiptV1,
    canonical_volition_action_sha256, canonical_volition_intent_sha256,
};

pub const BEING_UTTERANCE_ATTESTATION_SCHEMA_V1: &str = "being.utterance_attestation.v1";
pub const VOLITION_INTENT_SCHEMA_V1: &str = "volition.intent.v1";
pub const VOLITION_DECISION_SCHEMA_V1: &str = "volition.decision.v1";
pub const VOLITION_RECEIPT_SCHEMA_V1: &str = "volition.receipt.v1";
pub const DELEGATED_CAPABILITY_BINDING_SCHEMA_V1: &str = "volition.capability_binding.v1";
pub const DELEGATED_CAPABILITY_USAGE_SCHEMA_V1: &str = "volition.capability_usage.v1";
pub const OWNER_POLICY_SCHEMA_V1: &str = "volition.owner_policy.v1";
pub const OWNER_POLICY_RUNTIME_SCHEMA_V1: &str = "volition.owner_policy_runtime.v1";
pub const BEING_CONCERN_SCHEMA_V1: &str = "volition.being_concern.v1";
pub const VOLITION_QUEUE_SCHEMA_V1: &str = "volition.queue.v1";
pub const SEMANTIC_STRAND_SCHEMA_V1: &str = "volition.semantic_strand.v1";
pub const OWNER_INQUIRY_SCHEMA_V1: &str = "volition.owner_inquiry.v1";
pub const OWNER_INQUIRY_RECEIPT_SCHEMA_V1: &str = "volition.owner_inquiry_receipt.v1";
pub const INQUIRY_OBSERVATION_SCHEMA_V1: &str = "volition.inquiry_observation.v1";
pub const SEMANTIC_STRAND_SCHEMA_V2: &str = "volition.semantic_strand.v2";
pub const OWNER_INQUIRY_SCHEMA_V2: &str = "volition.owner_inquiry.v2";
pub const OWNER_INQUIRY_RECEIPT_SCHEMA_V2: &str = "volition.owner_inquiry_receipt.v2";
pub const INQUIRY_OBSERVATION_SCHEMA_V2: &str = "volition.inquiry_observation.v2";
pub const OWNER_CANARY_PLAN_SCHEMA_V2: &str = "volition.owner_canary_plan.v2";
pub const OWNER_RESEARCH_SESSION_SCHEMA_V1: &str = "volition.owner_research_session.v1";
pub const OWNER_EVIDENCE_GRAPH_SCHEMA_V1: &str = "volition.owner_evidence_graph.v1";
pub const OWNER_DECISION_PLAN_SCHEMA_V1: &str = "volition.owner_decision_plan.v1";
pub const SELF_CONTROL_CAPABILITY_MANIFEST_SCHEMA_V2: &str = "self_control.capability_manifest.v2";
pub const SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1: &str =
    "volition.signed_owner_research_receipt.v1";

use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

const MAX_TEXT_BYTES: usize = 4_096;
const MAX_JSON_BYTES: usize = 65_536;
const MAX_LIST_ENTRIES: usize = 64;

pub(super) fn canonical_json_bytes<T: Serialize>(value: &T) -> Option<Vec<u8>> {
    let mut value = serde_json::to_value(value).ok()?;
    canonicalize_json(&mut value);
    serde_json::to_vec(&value).ok()
}

pub(super) fn canonical_sha256<T: Serialize>(value: &T) -> String {
    let bytes = canonical_json_bytes(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn canonicalize_json(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            let mut ordered = fields
                .iter_mut()
                .map(|(key, child)| {
                    canonicalize_json(child);
                    (key.clone(), child.take())
                })
                .collect::<Vec<_>>();
            ordered.sort_by(|left, right| left.0.cmp(&right.0));
            fields.clear();
            fields.extend(ordered);
        },
        Value::Array(values) => {
            for child in values {
                canonicalize_json(child);
            }
        },
        _ => {},
    }
}

pub(super) fn valid_identifier(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.len() <= 256
}

pub(super) fn valid_bounded_text(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES
}

pub(super) fn valid_string_list(values: &[String]) -> bool {
    values.len() <= MAX_LIST_ENTRIES && values.iter().all(|value| valid_bounded_text(value))
}

pub(super) fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn finite_bounded_json(value: &Value) -> bool {
    value_is_finite(value)
        && serde_json::to_vec(value).is_ok_and(|encoded| encoded.len() <= MAX_JSON_BYTES)
}

fn value_is_finite(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_f64().is_some_and(f64::is_finite),
        Value::Array(values) => values.iter().all(value_is_finite),
        Value::Object(fields) => fields.values().all(value_is_finite),
        _ => true,
    }
}

pub(super) fn verify_signature(
    public_key_hex: &str,
    signature_hex: &str,
    signing_bytes: &[u8],
) -> bool {
    let Ok(public_key): Result<[u8; 32], _> = hex::decode(public_key_hex).and_then(|bytes| {
        bytes
            .try_into()
            .map_err(|_| hex::FromHexError::InvalidStringLength)
    }) else {
        return false;
    };
    let Ok(signature): Result<[u8; 64], _> = hex::decode(signature_hex).and_then(|bytes| {
        bytes
            .try_into()
            .map_err(|_| hex::FromHexError::InvalidStringLength)
    }) else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&public_key) else {
        return false;
    };
    verifying_key
        .verify(signing_bytes, &Signature::from_bytes(&signature))
        .is_ok()
}

#[cfg(test)]
mod tests;
