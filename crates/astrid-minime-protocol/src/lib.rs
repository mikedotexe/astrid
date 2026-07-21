//! Versioned JSON contracts for the Astrid/Minime telemetry and sensory lanes.
//!
//! This crate contains transport DTOs only. Regulator calculations, bridge
//! evidence derivation, authority checks, and runtime behavior remain owned by
//! their respective services.

#![deny(unsafe_code)]

mod division;
mod sensory;
mod telemetry;
mod version;

pub use division::{
    DIVISION_ACTION_AVAILABILITY_SCHEMA_V1, DIVISION_COMMAND_SCHEMA_V1, DIVISION_COMMIT_SCOPE_V1,
    DIVISION_EVENT_SCHEMA_V1, DIVISION_READINESS_POLICY_V1, DIVISION_RECEIPT_SCHEMA_V1,
    DIVISION_ROLLBACK_SCOPE_V1, DIVISION_STATUS_SCHEMA_V1, DivisionActionAvailabilityV1,
    DivisionActionV1, DivisionAvailableActionV1, DivisionBlockedActionV1, DivisionCapabilityRefV1,
    DivisionCommandV1, DivisionEventV1, DivisionLifecycleV1, DivisionReadinessV1,
    DivisionReceiptStatusV1, DivisionReceiptV1, DivisionSourceIdentityV1, DivisionStatusV1,
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
    SemanticEnergyV1, SemanticViscosityCoefficientV1, SettledMobilityReviewV1, ShadowClassV3,
    ShadowFieldModeV2, ShadowFieldV2, ShadowFieldV3, ShadowInfluenceResponseV3,
    ShadowPhaseTransitionV3, ShadowPreservationModeV1, ShadowSnapshotV3, SiltGranularityV1,
    SpectralDampingWarmStartReviewV1, SpectralDenominatorV1, SpectralFingerprintV1,
    ViscosityVector,
};
pub use version::{
    CompatibilityStatus, PROTOCOL_MAJOR, PROTOCOL_MINOR, PROTOCOL_NAME, ProtocolHeaderV1,
    TELEMETRY_PROTOCOL_MINOR, classify_protocol, current_protocol, telemetry_protocol,
};
