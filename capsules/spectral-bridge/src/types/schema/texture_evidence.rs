/// Typed texture summary behind resonance density. Advisory context only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceTextureSignatureV1 {
    pub policy: String,
    pub schema_version: u8,
    pub primary_texture: String,
    pub pressure_source_family: String,
    pub edge_definition: String,
    pub movement_quality: String,
    /// Direct Minime-reported viscosity beside the verbal movement label.
    /// `None` preserves the distinction between legacy absence and zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_index: Option<f32>,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_variance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_damping_threshold_candidate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_flux_vector: Option<TextureDynamicFluxVectorV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_constraints: Vec<String>,
    pub authority: String,
    pub note: String,
}

impl Default for ResonanceTextureSignatureV1 {
    fn default() -> Self {
        Self {
            policy: "resonance_texture_signature_v1".to_string(),
            schema_version: 1,
            primary_texture: "unknown".to_string(),
            pressure_source_family: "unknown".to_string(),
            edge_definition: "unknown".to_string(),
            movement_quality: "unknown".to_string(),
            viscosity_index: None,
            confidence: 0.0,
            temporal_variance: None,
            pressure_gradient_delta: None,
            dynamic_damping_threshold_candidate: None,
            dynamic_flux_vector: None,
            active_constraints: Vec::new(),
            authority: "advisory_context_not_control".to_string(),
            note: "texture signature absent from older payload".to_string(),
        }
    }
}

/// Read-only check that Minime's typed texture signature matches its component body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceTextureComponentAlignmentV1 {
    pub policy: String,
    pub schema_version: u8,
    pub expected_primary_texture: String,
    pub emitted_primary_texture: String,
    pub expected_movement_quality: String,
    pub emitted_movement_quality: String,
    pub alignment_state: String,
    pub confidence: f32,
    pub damping_candidate_status: String,
    pub authority: String,
}

impl Default for ResonanceTextureComponentAlignmentV1 {
    fn default() -> Self {
        Self {
            policy: "resonance_texture_component_alignment_v1".to_string(),
            schema_version: 1,
            expected_primary_texture: "unknown".to_string(),
            emitted_primary_texture: "unknown".to_string(),
            expected_movement_quality: "unknown".to_string(),
            emitted_movement_quality: "unknown".to_string(),
            alignment_state: "insufficient_context".to_string(),
            confidence: 0.0,
            damping_candidate_status: "unknown".to_string(),
            authority: "diagnostic_observability_not_damping_or_control".to_string(),
        }
    }
}

/// Read-only consistency packet for the typed texture signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureSignatureIntegrityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub movement_quality: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_viscosity_index: Option<f32>,
    pub component_viscosity_index: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_delta: Option<f32>,
    pub viscosity_alignment_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_variance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta_source: Option<String>,
    pub pressure_source_family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_damping_threshold_candidate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_flux_vector: Option<TextureDynamicFluxVectorV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability_context: Option<ResonanceStabilityContextV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_constraints: Vec<String>,
    pub variance_status: String,
    pub flux_status: String,
    pub damping_candidate_status: String,
    pub component_alignment_state: String,
    pub expected_primary_texture: String,
    pub emitted_primary_texture: String,
    pub advisory_observability: bool,
    /// Bridge-owned temporal and derivative evidence, separated from Minime's
    /// canonical producer DTO while the legacy combined projection remains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_texture_evidence_v1: Option<BridgeTextureEvidenceV1>,
    pub authority: String,
}

/// Bridge-derived texture evidence that is not part of Minime's wire DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTextureEvidenceV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_variance: Option<f32>,
    pub temporal_variance_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_flux_vector: Option<TextureDynamicFluxVectorV1>,
    pub dynamic_flux_vector_source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_constraints: Vec<String>,
    pub authority: String,
}

/// Bounded Minime-local PI target hint carried with resonance-density telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceDensityControl {
    pub target_bias_pct: f32,
    pub wander_scale: f32,
    pub applied_locally: bool,
    #[serde(default)]
    pub damping_coefficient: f32,
    #[serde(default)]
    pub intervention_type: ResonanceInterventionType,
    pub note: String,
}

/// Explains whether resonance-density control is observation, alignment, or damping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResonanceInterventionType {
    #[default]
    ObservationalReadout,
    PassiveAlignment,
    ActiveDamping,
    ManualOverrideReserved,
}

/// Typed density of mutually reinforcing resonance in the live eigenspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceDensityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub density: f32,
    pub containment_score: f32,
    pub pressure_risk: f32,
    pub quality: String,
    pub components: ResonanceDensityComponents,
    #[serde(default)]
    pub texture_signature: ResonanceTextureSignatureV1,
    #[serde(default)]
    pub texture_component_alignment: ResonanceTextureComponentAlignmentV1,
    pub control: ResonanceDensityControl,
}

/// Component scores behind Minime's pressure-source read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceComponents {
    pub lambda_monopoly: f32,
    pub mode_packing: f32,
    pub controller_pressure: f32,
    pub semantic_trickle: f32,
    #[serde(default)]
    pub semantic_friction: f32,
    pub structural_plurality_loss: f32,
    pub distinguishability_loss: f32,
    pub temporal_lock_in: f32,
    pub sensory_scarcity: f32,
}

/// Optional context contributors from action threads, attractors, and resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressureSourceContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_language: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_recurrence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attractor_pull: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pressure: Option<f32>,
}

/// V1 pressure-source control contract. This remains advisory only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceControl {
    pub applied_locally: bool,
    pub note: String,
}

/// Typed explanation of where inward/compression pressure appears to originate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceV1 {
    pub policy: String,
    pub schema_version: u8,
    pub pressure_score: f32,
    pub porosity_score: f32,
    pub dominant_source: String,
    pub quality: String,
    pub components: PressureSourceComponents,
    #[serde(default)]
    pub context: PressureSourceContext,
    pub control: PressureSourceControl,
}

/// Bridge-side read-only synthesis of pressure origin, trend, smoothing, and
/// heartbeat cadence. This makes structural mode-packing pressure visible even
/// when the rolling pressure trend looks stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceAnalysisV1 {
    pub policy: String,
    pub schema_version: u8,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_trickle: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing: Option<f32>,
    /// Evidence that made mode packing visible even when upstream labels are
    /// absent or renamed. This is diagnostic provenance, not a threshold write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_visibility_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_expansion_threshold_state: Option<String>,
    #[serde(default)]
    pub felt_mode_packing_dead_zone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_mode_packing_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liminal_mode_packing_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscous_density_warning_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscous_density_warning_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub felt_dead_zone_mode_packing_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion_threshold_gap: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_trend_classification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothing_classification: Option<String>,
    /// Current fast-window pressure edge beside the longer smoothing context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_edge_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_stagnation_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_stagnation_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_jitter_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_reliability: Option<String>,
    pub structural_pressure_state: String,
    pub ghost_stability_risk: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sensory_lane_risk: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pressure_relief_signal_candidate: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscous_recovery_mode_candidate: String,
    #[serde(default)]
    pub live_threshold_write: bool,
    #[serde(default)]
    pub sensory_lane_write: bool,
    /// Shared truth-channel for gated/delayed pressure experience that remains read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experience_delta_bus_v1: Option<ExperienceDeltaBusV1>,
    pub analysis: String,
    pub authority: String,
}
