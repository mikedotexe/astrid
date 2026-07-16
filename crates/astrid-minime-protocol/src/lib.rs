//! Versioned JSON contracts for the Astrid/Minime telemetry and sensory lanes.
//!
//! This crate contains transport DTOs only. Regulator calculations, bridge
//! evidence derivation, authority checks, and runtime behavior remain owned by
//! their respective services.

#![deny(unsafe_code)]

mod sensory;
mod telemetry;
mod version;

pub use sensory::{SensoryMsg, SensoryPacketV1};
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
    classify_protocol, current_protocol,
};
