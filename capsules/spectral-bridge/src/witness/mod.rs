//! Typed self/other provenance for the Minime-to-Astrid witness path.

mod provenance;
mod telemetry;

pub use provenance::{
    AstridInterpretationV1, BridgeEvidenceV1, MinimeObservationV1, ProvenanceOriginV1,
    ProvenanceRefV1, WireReceiptV1, WitnessFrameV1, WitnessSelfOtherDistinctionV1,
};
pub(crate) use telemetry::decode_telemetry_v1;
