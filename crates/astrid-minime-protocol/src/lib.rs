//! Versioned JSON contracts for the Astrid/Minime telemetry and sensory lanes.
//!
//! This crate contains transport DTOs only. Regulator calculations, bridge
//! evidence derivation, authority checks, and runtime behavior remain owned by
//! their respective services.

#![deny(unsafe_code)]

mod division;
mod self_control;
mod semantic_body;
mod sensory;
mod telemetry;
mod version;
mod volition;

pub use division::{
    DIVISION_ACTION_AVAILABILITY_SCHEMA_V1, DIVISION_COMMAND_SCHEMA_V1, DIVISION_COMMIT_SCOPE_V1,
    DIVISION_EVENT_SCHEMA_V1, DIVISION_READINESS_POLICY_V1, DIVISION_RECEIPT_SCHEMA_V1,
    DIVISION_ROLLBACK_SCOPE_V1, DIVISION_STATUS_SCHEMA_V1, DivisionActionAvailabilityV1,
    DivisionActionV1, DivisionAvailableActionV1, DivisionBlockedActionV1, DivisionCapabilityRefV1,
    DivisionCommandV1, DivisionEventV1, DivisionLifecycleV1, DivisionReadinessV1,
    DivisionReceiptStatusV1, DivisionReceiptV1, DivisionSourceIdentityV1, DivisionStatusV1,
};
pub use self_control::{
    SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1, SELF_CONTROL_COMMAND_SCHEMA_V2,
    SELF_CONTROL_INTENT_SCHEMA_V2, SELF_CONTROL_RECEIPT_SCHEMA_V2, SelfControlActionV2,
    SelfControlAuthorityClassV2, SelfControlAuthorityProofV1, SelfControlCommandV2,
    SelfControlDurabilityV2, SelfControlFamilyV2, SelfControlIntentV2, SelfControlReceiptStatusV2,
    SelfControlReceiptV2, SelfControlSourceIdentityV1, SelfControlValuesV2,
    canonical_self_control_intent_sha256,
};
pub use semantic_body::{
    SEMANTIC_BODY_BASE_DIMENSIONS_V2, SEMANTIC_BODY_COMPANION_DIMENSIONS_V2,
    SEMANTIC_BODY_SCHEMA_V2, SemanticBodyFidelityV2, SemanticBodyProvenanceV2, SemanticBodyV2,
    SemanticLaneRoleV2,
};
pub use sensory::{
    DeliveryEnvelopeV1, MutualAddressEnvelopeV1, SensoryDeliveryReceiptV1, SensoryDeliveryStatusV1,
    SensoryMsg, SensoryPacketV1, SensoryServerHelloV1, canonical_sensory_payload_sha256,
};
pub use telemetry::{
    EigenPacketPayloadBudgetReviewV1, EigenPacketV1, EigenvectorComponentV1,
    EigenvectorFieldSummaryV1, EigenvectorFieldV1, EigenvectorModeV1, EigenvectorPairwiseOverlapV1,
    EsnLeakOverrideStatus, HardResetTexturePreservationReviewV1, InhabitableFluctuationComponents,
    InhabitableFluctuationContext, InhabitableFluctuationControl,
    InhabitableFluctuationPressureCalibrationV1, InhabitableFluctuationV1, IsingShadowSummary,
    ModalityStatus, ModePartners, NeuralOutputs, PressureSourceComponents, PressureSourceContext,
    PressureSourceControl, PressureSourceProfileEntry, PressureSourceV1,
    ResonanceDensityComponents, ResonanceDensityControl, ResonanceDensityV1,
    ResonanceInterventionType, ResonanceTextureComponentAlignmentV1, ResonanceTextureSignatureV1,
    SPECTRAL_SUBSTRATE_POLICY_V1, SemanticEnergyV1, SemanticViscosityCoefficientV1,
    SettledMobilityReviewV1, ShadowClassV3, ShadowFieldModeV2, ShadowFieldV2, ShadowFieldV3,
    ShadowInfluenceResponseV3, ShadowPhaseTransitionV3, ShadowPreservationModeV1, ShadowSnapshotV3,
    SiltGranularityV1, SpectralDampingWarmStartReviewV1, SpectralDenominatorV1,
    SpectralFillSemanticsV1, SpectralFillSmoothingV1, SpectralFingerprintV1,
    SpectralSubstrateKindV1, SpectralSubstrateV1, SpectrumCoverageV1, ViscosityVector,
};
pub use version::{
    CompatibilityStatus, PROTOCOL_MAJOR, PROTOCOL_MINOR, PROTOCOL_NAME, ProtocolHeaderV1,
    TELEMETRY_PROTOCOL_MINOR, classify_protocol, current_protocol, telemetry_protocol,
};
pub use volition::{
    BEING_CONCERN_SCHEMA_V1, BEING_UTTERANCE_ATTESTATION_SCHEMA_V1, BeingConcernStatusV1,
    BeingConcernV1, BeingUtteranceAttestationV1, DELEGATED_CAPABILITY_BINDING_SCHEMA_V1,
    DELEGATED_CAPABILITY_USAGE_SCHEMA_V1, DelegatedCapabilityBindingV1, DelegatedCapabilityUsageV1,
    INQUIRY_OBSERVATION_SCHEMA_V1, INQUIRY_OBSERVATION_SCHEMA_V2, InquiryAnalysisPlanEntryV2,
    InquiryAnalysisReceiptV1, InquiryAnalysisReceiptV2, InquiryControlRevisionWitnessV2,
    InquiryCoverageModeV2, InquiryCoverageProofV2, InquiryExecutionIdentityV2, InquiryFeltStatusV1,
    InquiryIsolationProfileV2, InquiryMachineStatusV1, InquiryObservationActorV2,
    InquiryObservationPhaseV1, InquiryObservationV1, InquiryObservationV2, InquiryPrivacyReceiptV2,
    InquiryRollbackStateV1, OWNER_CANARY_MAX_DURATION_SECS_V2, OWNER_CANARY_MIN_DURATION_SECS_V2,
    OWNER_CANARY_PLAN_SCHEMA_V2, OWNER_DECISION_PLAN_SCHEMA_V1, OWNER_EVIDENCE_GRAPH_SCHEMA_V1,
    OWNER_INQUIRY_DETERMINISTIC_RUNS_V2, OWNER_INQUIRY_MAX_STRANDS_V1,
    OWNER_INQUIRY_MIN_STRANDS_V1, OWNER_INQUIRY_RECEIPT_SCHEMA_V1, OWNER_INQUIRY_RECEIPT_SCHEMA_V2,
    OWNER_INQUIRY_SCHEMA_V1, OWNER_INQUIRY_SCHEMA_V2, OWNER_POLICY_RUNTIME_SCHEMA_V1,
    OWNER_POLICY_SCHEMA_V1, OWNER_RESEARCH_SESSION_SCHEMA_V1, OwnerCanaryControlV2,
    OwnerCanaryPlanV2, OwnerCanaryRollbackPlanV2, OwnerDecisionBranchV1,
    OwnerDecisionEvaluationStatusV1, OwnerDecisionEvaluationV1, OwnerDecisionPlanV1,
    OwnerEvidenceClaimV1, OwnerEvidenceComparatorV1, OwnerEvidenceEdgeV1, OwnerEvidenceGraphV1,
    OwnerEvidenceNodeKindV1, OwnerEvidenceNodeV1, OwnerEvidencePredicateV1, OwnerEvidenceReducerV1,
    OwnerEvidenceScopeKindV1, OwnerEvidenceScopeV1, OwnerInquiryAnalysisV1,
    OwnerInquiryAuthorityBoundaryV1, OwnerInquiryCancellationV1, OwnerInquiryReceiptV1,
    OwnerInquiryReceiptV2, OwnerInquiryStatusV1, OwnerInquiryStopConditionsV2,
    OwnerInquirySuccessConditionsV2, OwnerInquiryV1, OwnerInquiryV2, OwnerPolicyComparatorV1,
    OwnerPolicyConditionRuntimeV1, OwnerPolicyConditionV1, OwnerPolicyEvaluationStatusV1,
    OwnerPolicyEvaluationV1, OwnerPolicyLogicV1, OwnerPolicyRuntimeV1, OwnerPolicyScopeV1,
    OwnerPolicyV1, OwnerResearchLifecycleStatusV1, OwnerResearchPayloadKindV1,
    OwnerResearchSessionV1, PerceptibleReturnV1, SELF_CONTROL_CAPABILITY_MANIFEST_SCHEMA_V2,
    SEMANTIC_STRAND_BASE_DIMENSIONS_V1, SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1,
    SEMANTIC_STRAND_SCHEMA_V1, SEMANTIC_STRAND_SCHEMA_V2, SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1,
    SelfControlCapabilityManifestV2, SelfControlCapabilityV2, SelfControlValueDomainV2,
    SemanticStrandDisclosureV2, SemanticStrandLineageOperationV2, SemanticStrandLineageV2,
    SemanticStrandProvenanceV1, SemanticStrandV1, SemanticStrandV2, SignedOwnerResearchReceiptV1,
    VOLITION_DECISION_SCHEMA_V1, VOLITION_INTENT_SCHEMA_V1, VOLITION_QUEUE_SCHEMA_V1,
    VOLITION_RECEIPT_SCHEMA_V1, VolitionActionV1, VolitionActorIdentityV1,
    VolitionAuthorityClassV1, VolitionBudgetV1, VolitionDecisionStatusV1, VolitionDecisionV1,
    VolitionDurabilityV1, VolitionIntentV1, VolitionOperationV1, VolitionPrioritySourcePolicyV1,
    VolitionQueueOrderingV1, VolitionQueueV1, VolitionReceiptStatusV1, VolitionReceiptV1,
    VolitionSubstrateSchedulingV1, VolitionWorkClassV1, VolitionWriteSchedulingV1,
    canonical_being_utterance_attestation_sha256, canonical_owner_decision_plan_sha256,
    canonical_owner_evidence_graph_sha256, canonical_owner_inquiry_receipt_sha256_v2,
    canonical_owner_inquiry_sha256, canonical_owner_inquiry_sha256_v2,
    canonical_owner_research_session_sha256, canonical_self_control_capability_manifest_sha256,
    canonical_semantic_strand_content_sha256, canonical_semantic_strand_embedding_sha256,
    canonical_volition_action_sha256, canonical_volition_intent_sha256,
    owner_inquiry_analysis_plan_v2, owner_inquiry_fixed_analysis_set_v1,
    public_key_fingerprint_sha256,
};
