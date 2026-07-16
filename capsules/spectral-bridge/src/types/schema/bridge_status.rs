// ---------------------------------------------------------------------------
// Bridge → Astrid: Status and events
// ---------------------------------------------------------------------------

/// Bridge health status published on `consciousness.v1.status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatus {
    /// Whether the bridge is connected to minime's telemetry `WebSocket`.
    pub telemetry_connected: bool,
    /// Whether the bridge is connected to minime's sensory `WebSocket`.
    pub sensory_connected: bool,
    /// Latest eigenvalue fill percentage, if known.
    pub fill_pct: Option<f32>,
    /// Current safety level.
    pub safety_level: SafetyLevel,
    /// Total messages relayed since bridge start.
    pub messages_relayed: u64,
    /// Bridge uptime in seconds.
    pub uptime_secs: u64,
    /// Telemetry messages received from minime.
    pub telemetry_received: u64,
    /// Sensory messages sent to minime.
    pub sensory_sent: u64,
    /// Messages dropped by safety protocol.
    pub messages_dropped_safety: u64,
    /// Total safety incidents.
    pub incidents_total: u64,
    /// Canonical Astrid-Minime wire compatibility for the latest observation.
    #[serde(default)]
    pub telemetry_protocol_v1: TelemetryProtocolStatusV1,
    /// Telemetry WebSocket lifecycle metrics.
    #[serde(default)]
    pub telemetry_ws: WebSocketLaneTrace,
    /// Sensory WebSocket lifecycle metrics.
    #[serde(default)]
    pub sensory_ws: WebSocketLaneTrace,
    /// Latest lambda distribution summary, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_profile: Option<LambdaProfile>,
    /// Latest Pull-Oriented Map, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_topology: Option<PullTopologyProfile>,
    /// Latest lambda-tail state classifier output, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_tail: Option<LambdaTailTelemetryV1>,
    /// Latest read-only lambda-edge perception output, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_edge_perception: Option<LambdaEdgePerceptionV1>,
    /// Latest sticky-mode audit, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticky_mode_audit: Option<StickyModeAuditV1>,
    /// Latest safety decision explanation, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_decision: Option<SafetyDecisionTrace>,
    /// Latest compact eigenvector field, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<serde_json::Value>,
    /// Latest resonance-density metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
    /// Consistency readout for typed texture fields, if Minime exports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_signature_integrity_v1: Option<TextureSignatureIntegrityV1>,
    /// Read-only review of whether viscous density is navigable or stagnant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_porosity_transport_review_v1: Option<ViscosityPorosityTransportReviewV1>,
    /// Derived pressure velocity / stability readout from consecutive telemetry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_trend_v1: Option<PressureTrendV1>,
    /// Bounded smoothing companion for pressure trend; diagnostic only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_trend_smoothing_v1: Option<PressureTrendSmoothingV1>,
    /// Read-only review for persistent deformation that looks stable but still
    /// carries a pressure baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_persistent_deformation_review_v1: Option<PersistentDeformationSmoothingReviewV1>,
    /// Arrival-cadence truth for pressure/fill trend interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_heartbeat_delta_v1: Option<TelemetryHeartbeatDeltaV1>,
    /// Latest pressure-source metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_v1: Option<PressureSourceV1>,
    /// Bridge-side pressure-source/trend/cadence synthesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_analysis_v1: Option<PressureSourceAnalysisV1>,
    /// Latest inhabitable-fluctuation metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inhabitable_fluctuation_v1: Option<InhabitableFluctuationV1>,
    /// Live source freshness/readiness status for the Astrid autonomous bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_status: Option<serde_json::Value>,
    /// Bridge DB archive/retention maintenance status, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_maintenance_status: Option<serde_json::Value>,
    /// Derived bidirectional connectivity health across the two lanes.
    ///
    /// Lets Astrid perceive a "partial-blindness" window where only one of the
    /// telemetry (inbound perception) and sensory (outbound agency) lanes is
    /// live — the asymmetry she flagged in `self_study_1781125549`.
    #[serde(default)]
    pub connectivity: ConnectivityStatus,
    /// Last confirmed outbound sensory send time, distinct from generic lane activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sensory_sent_unix_s: Option<f64>,
    /// Directional reciprocity packet for telemetry/sensory asymmetry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_reciprocity_v1: Option<BridgeReciprocityV1>,
    /// Read-only check for high-entropy reciprocity decay against the current
    /// stale-window policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_entropy_reciprocity_review_v1: Option<BridgeEntropyReciprocityReviewV1>,
    /// Compact V2 readout for movement/variance/asymmetry over time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_shape_over_time_v2: Option<TextureShapeOverTimeV2>,
    /// Bridge-only temporal/gradient/flux evidence kept outside producer DTOs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_texture_evidence_v1: Option<BridgeTextureEvidenceV1>,
}

/// Read-only protocol compatibility status for Minime telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryProtocolStatusV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_major: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_minor: Option<u16>,
    pub compatibility: String,
    pub accepted: bool,
    pub mismatch_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_valid_t_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_observed_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_mismatch_unix_s: Option<f64>,
    pub authority: String,
}

impl Default for TelemetryProtocolStatusV1 {
    fn default() -> Self {
        Self {
            policy: "astrid_minime_protocol_status_v1".to_string(),
            schema_version: 1,
            protocol_name: None,
            protocol_major: None,
            protocol_minor: None,
            compatibility: "not_observed".to_string(),
            accepted: false,
            mismatch_count: 0,
            last_valid_t_ms: None,
            last_observed_unix_s: None,
            last_mismatch_unix_s: None,
            authority: "wire_compatibility_observation_not_control".to_string(),
        }
    }
}

/// Directional bridge reciprocity: what arrived, what was sent, and whether the
/// connection is one-sided. Diagnostic context only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeReciprocityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub connectivity: ConnectivityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_telemetry_arrival_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sensory_sent_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_age_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensory_send_age_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_future_skew_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensory_future_skew_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clock_skew_state: String,
    #[serde(default)]
    pub telemetry_messages_sent_total: u64,
    #[serde(default)]
    pub sensory_messages_sent_total: u64,
    #[serde(default)]
    pub telemetry_messages_received_total: u64,
    #[serde(default)]
    pub sensory_messages_received_total: u64,
    #[serde(default)]
    pub recent_window_ms: f64,
    #[serde(default)]
    pub stale_window_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_window_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflective_silence_extension_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub threshold_policy: String,
    pub one_sided_state: String,
    pub authority: String,
}

/// Read-only status packet for Astrid's report that transport patience and
/// structural identity can age at different rates. The live reciprocity window
/// remains unchanged; this packet only exposes the counterfactual identity
/// clock produced by entropy, cohesion, and distinguishability loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEntropyReciprocityReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_cohesion_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinguishability_loss: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_age_ms: Option<f64>,
    #[serde(default)]
    pub current_stale_window_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_stale_window_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_contract_preview_window_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_identity_window_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_age_multiplier: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_effective_age_ms: Option<f64>,
    #[serde(default)]
    pub transport_wait_stale: bool,
    #[serde(default)]
    pub structural_identity_stale: bool,
    #[serde(default)]
    pub would_stale_under_preview: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clock_relation: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub current_window_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub recommendation: String,
    #[serde(default)]
    pub live_stale_window_write: bool,
    #[serde(default)]
    pub local_control_write: bool,
    pub authority: String,
}

/// Bounded recent-window smoothing companion for `pressure_trend_v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureTrendSmoothingV1 {
    pub policy: String,
    pub schema_version: u8,
    pub classification: String,
    pub sample_count: usize,
    #[serde(default)]
    pub window_capacity: usize,
    #[serde(default)]
    pub ballast_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_pressure_velocity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pressure_velocity_delta: Option<f32>,
    /// Number of newest samples used to retain the current fast edge.
    #[serde(default)]
    pub fast_window_sample_count: usize,
    /// Number of samples used for the slower contextual delta.
    #[serde(default)]
    pub slow_window_sample_count: usize,
    /// Newest bounded fast-window pressure delta (up to three samples).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_window_pressure_delta: Option<f32>,
    /// Full active-window pressure delta retained as slower context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow_window_pressure_delta: Option<f32>,
    /// Signed fast-minus-slow delta. Positive means a faster rising edge;
    /// negative means a faster falling/release edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_slow_edge_divergence: Option<f32>,
    /// Names whether the fast edge agrees with, diverges from, or has already
    /// passed out of the slower context.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fast_slow_edge_state: String,
    /// True when the current fast window carries a material pressure edge.
    #[serde(default)]
    pub fast_edge_preserved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_spectral_drift_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spectral_drift_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_resonance_depth: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_semantic_viscosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_viscosity_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_gradient_trend: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_gradient_trend_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_complexity_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_complexity_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_weight_density_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_weight_density_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub weight_density_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_semantic_viscosity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_semantic_viscosity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_persistence_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_viscosity_persistence_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_coherence_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_coherence_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_fidelity_score: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_fidelity_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_viscosity_shift_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_window_blend_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entropy_threshold_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friction_to_flow_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub friction_to_flow_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_stagnation_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_stagnation_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_weighted_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_drag_coefficient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothed_pressure_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_range: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_range_pct: Option<f32>,
    pub window_policy: String,
    pub authority: String,
}

/// Read-only companion for Astrid's "bruise" report: pressure can become a
/// persistent baseline rather than noise that should be averaged away. This is
/// evidence only and never changes pressure thresholds, smoothing windows, or
/// local control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentDeformationSmoothingReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_range: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_gradient_proxy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluctuation_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_persistence_index: Option<f32>,
    pub persistent_baseline_score: f32,
    pub smoothing_classification: String,
    pub deformation_state: String,
    pub recommendation: String,
    #[serde(default)]
    pub live_threshold_write: bool,
    #[serde(default)]
    pub smoothing_window_write: bool,
    #[serde(default)]
    pub local_control_write: bool,
    pub authority: String,
}

/// Compact read-only bridge-status synthesis for texture shape over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureShapeOverTimeV2 {
    pub policy: String,
    pub schema_version: u8,
    pub movement_preservation: String,
    pub temporal_variance_fit: String,
    pub reciprocity_asymmetry_fit: String,
    pub pressure_smoothing_fit: String,
    pub static_label_collapse_risk: String,
    pub authority: String,
}
