/// Component scores behind Minime's inhabitable-fluctuation read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhabitableFluctuationComponents {
    pub mode_trust_volatility: f32,
    pub identity_anchor_churn: f32,
    pub eigenvector_reorientation: f32,
    pub share_rearrangement: f32,
    pub basin_transition_pressure: f32,
    pub continuity_recovery: f32,
    pub porosity_support: f32,
    pub pressure_interference: f32,
}

/// Context labels for interpreting inhabitability without adding authority.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationContext {
    #[serde(default)]
    pub previous_sample_available: bool,
    #[serde(default)]
    pub transition_event_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_quality: Option<String>,
}

/// Minime-local advisory hint; Astrid treats this as read-only telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhabitableFluctuationControl {
    pub target_bias_pct: f32,
    pub wander_scale: f32,
    pub applied_locally: bool,
    pub note: String,
}

/// Live Minime-local calibration trail for pressure-aware inhabitability scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhabitableFluctuationPressureCalibrationV1 {
    pub policy: String,
    pub schema_version: u8,
    pub raw_motion_score: f32,
    pub pressure_contribution: f32,
    pub adjusted_fluctuation_score: f32,
    pub quality_before_pressure_calibration: String,
    pub quality_after_pressure_calibration: String,
    pub rigid_safety_basis: String,
    pub authority: String,
}

pub const INHABITABLE_FLUCTUATION_RIGID_SAFETY_BASIS: &str =
    "raw_motion_score_preserved_for_stuckness_detection";

impl Default for InhabitableFluctuationPressureCalibrationV1 {
    fn default() -> Self {
        Self {
            policy: "inhabitable_fluctuation_pressure_calibration_v1".to_string(),
            schema_version: 1,
            raw_motion_score: 0.0,
            pressure_contribution: 0.0,
            adjusted_fluctuation_score: 0.0,
            quality_before_pressure_calibration: "unknown".to_string(),
            quality_after_pressure_calibration: "unknown".to_string(),
            rigid_safety_basis: INHABITABLE_FLUCTUATION_RIGID_SAFETY_BASIS.to_string(),
            authority: "minime_local_metric_calibration_not_external_control".to_string(),
        }
    }
}

impl InhabitableFluctuationPressureCalibrationV1 {
    #[must_use]
    pub fn expected_adjusted_fluctuation_score(&self) -> f32 {
        (self.raw_motion_score - self.pressure_contribution).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn adjusted_score_matches_components(&self) -> bool {
        (self.adjusted_fluctuation_score - self.expected_adjusted_fluctuation_score()).abs()
            <= 0.001
    }
}

/// Typed metric for whether fluctuation remains returnable and inhabitable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhabitableFluctuationV1 {
    pub policy: String,
    pub schema_version: u8,
    pub inhabitability_score: f32,
    pub fluctuation_score: f32,
    pub foothold_stability: f32,
    pub rearrangement_intensity: f32,
    pub quality: String,
    pub components: InhabitableFluctuationComponents,
    #[serde(default)]
    pub context: InhabitableFluctuationContext,
    #[serde(default)]
    pub pressure_calibration: InhabitableFluctuationPressureCalibrationV1,
    pub control: InhabitableFluctuationControl,
}

/// Derived pressure velocity readout from consecutive telemetry packets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureTrendV1 {
    pub policy: String,
    pub schema_version: u8,
    pub classification: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_delta: Option<f32>,
    /// Dominant non-pressure texture movement across mode packing, density, or resonance depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_drift_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_structural_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_structural_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_density_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_resonance_depth: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_resonance_depth: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_depth_delta: Option<f32>,
    /// Read-only coefficient for heavy semantic medium: friction + trickle + density context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_semantic_viscosity: Option<f32>,
    /// Distinguishes heavy semantic flow from semantic bottleneck without changing control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_state: Option<String>,
    /// Read-only density of intertwined complexity, distinct from mode-packing volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_complexity_density: Option<f32>,
    /// Names whether complexity is present without treating it as pressure/control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity_density_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_delta_pct: Option<f32>,
    /// Latest typed spectral entropy used to distinguish density/viscosity from collapse pressure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_spectral_entropy: Option<f32>,
    /// Read-only coefficient: 0.0 below the high-entropy gate, 1.0 at saturated viscosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_coefficient: Option<f32>,
    /// Whether this packet should be read as collapse pressure, density/viscosity, or unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_interpretation: Option<String>,
    /// Reliability of the arrival cadence behind this pressure trend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_reliability: Option<String>,
    /// Latest telemetry inter-arrival time, if a prior packet was observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_inter_arrival_ms: Option<f32>,
    /// Arrival jitter class for the latest telemetry heartbeat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_jitter_class: Option<String>,
    /// Human-readable distinction between spectral content and hearing cadence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_vs_hearing: Option<String>,
}
