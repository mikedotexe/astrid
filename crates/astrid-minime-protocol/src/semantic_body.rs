use serde::{Deserialize, Serialize};

pub const SEMANTIC_BODY_SCHEMA_V2: &str = "semantic_body.v2";
pub const SEMANTIC_BODY_BASE_DIMENSIONS_V2: usize = 48;
pub const SEMANTIC_BODY_COMPANION_DIMENSIONS_V2: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticLaneRoleV2 {
    LegacyCompatible,
    CompanionObservation,
    MigrationProbe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticBodyProvenanceV2 {
    pub source: String,
    pub source_sha256: String,
    pub producer_process_identity: String,
    pub producer_deployment_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub introspection_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticBodyFidelityV2 {
    pub codec: String,
    pub companion_mix: f32,
    pub base_transport_exact: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconstruction_error: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fidelity_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticBodyV2 {
    pub schema: String,
    pub body_id: String,
    pub base_features_48: Vec<f32>,
    pub companion_features_12: Vec<f32>,
    pub lane_role: SemanticLaneRoleV2,
    pub projection_basis_sha256: String,
    pub provenance: SemanticBodyProvenanceV2,
    pub timestamp_unix_ms: u64,
    pub fidelity: SemanticBodyFidelityV2,
}

impl SemanticBodyV2 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.schema == SEMANTIC_BODY_SCHEMA_V2
            && valid_identifier(&self.body_id)
            && self.base_features_48.len() == SEMANTIC_BODY_BASE_DIMENSIONS_V2
            && self.companion_features_12.len() == SEMANTIC_BODY_COMPANION_DIMENSIONS_V2
            && self
                .base_features_48
                .iter()
                .chain(&self.companion_features_12)
                .all(|value| value.is_finite())
            && valid_sha256(&self.projection_basis_sha256)
            && valid_identifier(&self.provenance.source)
            && valid_sha256(&self.provenance.source_sha256)
            && valid_identifier(&self.provenance.producer_process_identity)
            && valid_identifier(&self.provenance.producer_deployment_identity)
            && valid_identifier(&self.fidelity.codec)
            && self.fidelity.companion_mix.is_finite()
            && (0.0..=1.0).contains(&self.fidelity.companion_mix)
            && self
                .fidelity
                .reconstruction_error
                .is_none_or(f32::is_finite)
            && self
                .fidelity
                .fidelity_note
                .as_deref()
                .is_none_or(valid_bounded_text)
    }

    /// The legacy semantic lane is deliberately independent of the companion
    /// sidecar. A zero companion mix therefore cannot perturb these bytes.
    #[must_use]
    pub fn legacy_features(&self) -> &[f32] {
        &self.base_features_48
    }

    #[must_use]
    pub fn companion_is_active(&self) -> bool {
        self.fidelity.companion_mix > 0.0
    }
}

fn valid_identifier(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.len() <= 256
}

fn valid_bounded_text(value: &str) -> bool {
    value.len() <= 2_048
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
