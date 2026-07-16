use spectral_bridge_server::types::{
    BridgeTextureEvidenceV1, PressureTrendV1, ResidualDeformationTraceV1,
};
use spectral_bridge_server::witness::{BridgeEvidenceV1, ProvenanceRefV1};

fn forge(
    provenance: ProvenanceRefV1,
    texture: Option<BridgeTextureEvidenceV1>,
    residual_deformation: Option<ResidualDeformationTraceV1>,
    pressure_trend: Option<PressureTrendV1>,
) -> BridgeEvidenceV1 {
    BridgeEvidenceV1 {
        provenance,
        texture,
        residual_deformation,
        pressure_trend,
    }
}

fn main() {}
