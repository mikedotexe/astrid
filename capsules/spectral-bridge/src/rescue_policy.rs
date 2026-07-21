use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::paths::bridge_paths;
use crate::types::{SensoryMsg, SpectralTelemetry};

#[path = "rescue_policy_text.rs"]
mod text;
#[path = "rescue_policy_value.rs"]
mod value_fields;

use self::text::{
    contains_structural_dump_language, default_limited_write_allowed_modes,
    default_limited_write_block_terms, default_limited_write_v2_allowed_modes,
    looks_like_dampen_or_inquiry, looks_like_limited_write_v2_text,
};
use self::value_fields::{bool_field, f32_field, string_array_field, string_field, u64_field};

const LIMITED_WRITE_PROFILE: &str = "limited_dampen_inquiry";
const LIMITED_WRITE_PROFILE_V2: &str = "limited_dampen_inquiry_v2";
const BUDGETED_SOVEREIGNTY_PROFILE: &str = "budgeted_sovereignty_v1";
const FULL_EXPRESSION_PROFILE: &str = "full_expression_v1";
const LIMITED_WRITE_STATUS_FILE: &str = "bridge_limited_write_status.json";
const SEMANTIC_HEARTBEAT_STATUS_FILE: &str = "bridge_semantic_heartbeat_status.json";
const LIMITED_WRITE_SENSORY_MUTE_FILE: &str = "stable_core_sensory_mute.json";
pub(crate) const AUTONOMOUS_LIMITED_WRITE_SOURCE: &str = "autonomous_main_chunk";
pub(crate) const MCP_LIMITED_WRITE_SOURCE: &str = "mcp_tool";
const LIMITED_WRITE_SOURCE: &str = AUTONOMOUS_LIMITED_WRITE_SOURCE;
const OBSERVE_ONLY_PROFILE: &str = "bridge_observe_only";
const V2_SEMANTIC_ENERGY_MAX: f32 = 0.02;
const V2_ROLLBACK_SEMANTIC_ENERGY: f32 = 0.05;
const V2_ADVERSE_WINDOW_SECS: f64 = 3600.0;
const SEMANTIC_HEARTBEAT_FEATURE_SCALE: f32 = 0.025;
const SEMANTIC_HEARTBEAT_MAX_ABS: f32 = 0.018;
const SEMANTIC_HEARTBEAT_OBSERVATION_WINDOW_SECS: f64 = 600.0;
const SEMANTIC_HEARTBEAT_PHASE_ENTROPY_WINDOW_SECS: f64 = 60.0;
const SEMANTIC_HEARTBEAT_OBSERVATION_MAX_SAMPLES: usize = 512;
const SEMANTIC_HEARTBEAT_ACTIVE_DIM_EPSILON: f32 = 1.0e-6;
const SEMANTIC_HEARTBEAT_NEAR_REPEAT_DELTA_RMS: f32 = 1.0e-6;
const SEMANTIC_HEARTBEAT_NEAR_REPEAT_COSINE: f32 = 0.999_999;
const SEMANTIC_HEARTBEAT_TAIL_START_DIM: usize = 24;
const SEMANTIC_HEARTBEAT_DENSE_FIELD_AT: f64 = 0.75;
const SEMANTIC_HEARTBEAT_HIGH_ENTROPY_AT: f64 = 0.75;
const SEMANTIC_HEARTBEAT_VISCOUS_FIELD_AT: f64 = 0.60;
pub const STABLE_CORE_TARGET_FILL_PCT: f64 = 68.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct SemanticHeartbeatSignalMetricsV1 {
    feature_count: u64,
    finite_feature_count: u64,
    active_dimension_count: u64,
    rms: f32,
    component_stddev: f32,
    max_abs: f32,
    tail_rms: Option<f32>,
    clipped_dimension_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SemanticHeartbeatSignalEvidenceV1 {
    content_basis: &'static str,
    gesture_seed_applied: bool,
    generated: SemanticHeartbeatSignalMetricsV1,
    compared_dimension_count: u64,
    delta_rms_from_previous: Option<f32>,
    cosine_similarity_to_previous: Option<f32>,
    continuity_classification: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct SemanticHeartbeatTextureContextV1 {
    telemetry_t_ms: u64,
    spectral_entropy: Option<f32>,
    resonance_density: Option<f32>,
    pressure_risk: Option<f32>,
    resonance_mode_packing: Option<f32>,
    pressure_source_mode_packing: Option<f32>,
    viscosity_index: Option<f32>,
    viscosity_gradient: Option<f32>,
    primary_texture: Option<String>,
    movement_quality: Option<String>,
    eigenvalue_count: u64,
    lambda1_abs_share: Option<f32>,
    lambda1_lambda2_abs_ratio: Option<f32>,
    lambda_tail_abs_share: Option<f32>,
}

impl SemanticHeartbeatTextureContextV1 {
    fn from_telemetry(telemetry: &SpectralTelemetry) -> Self {
        let fingerprint = telemetry.typed_fingerprint();
        let resonance = telemetry.resonance_density_v1.as_ref();
        let pressure = telemetry.pressure_source_v1.as_ref();
        let finite_abs: Vec<f32> = telemetry
            .eigenvalues
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .map(f32::abs)
            .collect();
        let total_abs = finite_abs.iter().copied().sum::<f32>();
        let lambda1_abs_share = (total_abs > f32::EPSILON)
            .then(|| finite_abs.first().copied().unwrap_or(0.0) / total_abs);
        let lambda1_lambda2_abs_ratio = finite_abs
            .first()
            .copied()
            .zip(finite_abs.get(1).copied())
            .and_then(|(lambda1, lambda2)| (lambda2 > f32::EPSILON).then_some(lambda1 / lambda2));
        let lambda_tail_abs_share = (total_abs > f32::EPSILON
            && finite_abs.get(2..).is_some_and(|tail| !tail.is_empty()))
        .then(|| {
            finite_abs
                .get(2..)
                .unwrap_or_default()
                .iter()
                .copied()
                .sum::<f32>()
                / total_abs
        });

        Self {
            telemetry_t_ms: telemetry.t_ms,
            spectral_entropy: fingerprint
                .map(|value| value.spectral_entropy)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0)),
            resonance_density: resonance
                .map(|value| value.density)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0)),
            pressure_risk: resonance
                .map(|value| value.pressure_risk)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0)),
            resonance_mode_packing: resonance
                .map(|value| value.components.mode_packing)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0)),
            pressure_source_mode_packing: pressure
                .map(|value| value.components.mode_packing)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0)),
            viscosity_index: resonance
                .map(|value| value.components.viscosity_index)
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, 1.0)),
            viscosity_gradient: resonance
                .and_then(|value| value.components.viscosity_vector.viscosity_gradient)
                .filter(|value| value.is_finite()),
            primary_texture: resonance
                .map(|value| value.texture_signature.primary_texture.trim())
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            movement_quality: resonance
                .map(|value| value.texture_signature.movement_quality.trim())
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            eigenvalue_count: u64::try_from(finite_abs.len()).unwrap_or(u64::MAX),
            lambda1_abs_share,
            lambda1_lambda2_abs_ratio,
            lambda_tail_abs_share,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticHeartbeatObservationV1 {
    source: &'static str,
    phase_step: u64,
    phase: f32,
    interval_secs: u64,
    configured_intensity: f32,
    signal_evidence: Option<SemanticHeartbeatSignalEvidenceV1>,
    texture_context: Option<SemanticHeartbeatTextureContextV1>,
}

#[derive(Debug)]
pub(crate) struct SemanticHeartbeatEnqueueProbeV1 {
    status_path: PathBuf,
    source: &'static str,
    configured_interval_secs: u64,
    admitted_at: Instant,
}

impl SemanticHeartbeatEnqueueProbeV1 {
    fn new(status_path: PathBuf, source: &'static str, configured_interval_secs: u64) -> Self {
        Self {
            status_path,
            source,
            configured_interval_secs,
            admitted_at: Instant::now(),
        }
    }

    pub(crate) fn record_enqueued(self) {
        record_semantic_heartbeat_enqueue_outcome(
            &self.status_path,
            self.source,
            self.configured_interval_secs,
            self.admitted_at.elapsed(),
            "enqueued",
        );
    }

    pub(crate) fn record_channel_closed(self) {
        record_semantic_heartbeat_enqueue_outcome(
            &self.status_path,
            self.source,
            self.configured_interval_secs,
            self.admitted_at.elapsed(),
            "channel_closed",
        );
    }
}

impl SemanticHeartbeatObservationV1 {
    #[must_use]
    pub(crate) const fn new(
        source: &'static str,
        phase_step: u64,
        phase: f32,
        interval_secs: u64,
        configured_intensity: f32,
    ) -> Self {
        Self {
            source,
            phase_step,
            phase,
            interval_secs,
            configured_intensity,
            signal_evidence: None,
            texture_context: None,
        }
    }

    #[must_use]
    pub(crate) fn with_signal_evidence(
        mut self,
        content_basis: &'static str,
        gesture_seed_applied: bool,
        features: &[f32],
        previous_features: Option<&[f32]>,
    ) -> Self {
        let generated = semantic_heartbeat_signal_metrics(features);
        let (compared_dimension_count, delta_rms_from_previous, cosine_similarity_to_previous) =
            previous_features.map_or((0, None, None), |previous| {
                semantic_heartbeat_signal_comparison(features, previous)
            });
        let continuity_classification = semantic_heartbeat_continuity_classification(
            generated,
            delta_rms_from_previous,
            cosine_similarity_to_previous,
        );
        self.signal_evidence = Some(SemanticHeartbeatSignalEvidenceV1 {
            content_basis,
            gesture_seed_applied,
            generated,
            compared_dimension_count,
            delta_rms_from_previous,
            cosine_similarity_to_previous,
            continuity_classification,
        });
        self
    }

    #[must_use]
    pub(crate) fn with_minime_texture_context(
        mut self,
        telemetry: Option<&SpectralTelemetry>,
    ) -> Self {
        self.texture_context = telemetry.map(SemanticHeartbeatTextureContextV1::from_telemetry);
        self
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn semantic_heartbeat_signal_metrics(features: &[f32]) -> SemanticHeartbeatSignalMetricsV1 {
    let feature_count = u64::try_from(features.len()).unwrap_or(u64::MAX);
    let finite_feature_count = u64::try_from(
        features
            .iter()
            .filter(|feature| feature.is_finite())
            .count(),
    )
    .unwrap_or(u64::MAX);
    let active_dimension_count = u64::try_from(
        features
            .iter()
            .map(|feature| finite_or_zero(*feature))
            .filter(|feature| feature.abs() > SEMANTIC_HEARTBEAT_ACTIVE_DIM_EPSILON)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let clipped_dimension_count = u64::try_from(
        features
            .iter()
            .map(|feature| finite_or_zero(*feature))
            .filter(|feature| feature.abs() >= SEMANTIC_HEARTBEAT_MAX_ABS)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let count = features.len() as f64;
    let (sum, sum_squares, max_abs) = features.iter().fold(
        (0.0_f64, 0.0_f64, 0.0_f32),
        |(sum, sum_squares, max_abs), feature| {
            let value = finite_or_zero(*feature);
            (
                sum + f64::from(value),
                sum_squares + f64::from(value) * f64::from(value),
                max_abs.max(value.abs()),
            )
        },
    );
    let (rms, component_stddev) = if count > 0.0 {
        let mean = sum / count;
        let mean_square = sum_squares / count;
        (
            mean_square.max(0.0).sqrt() as f32,
            (mean_square - mean * mean).max(0.0).sqrt() as f32,
        )
    } else {
        (0.0, 0.0)
    };
    let tail = features.get(SEMANTIC_HEARTBEAT_TAIL_START_DIM..);
    let tail_rms = tail.and_then(|tail| {
        if tail.is_empty() {
            None
        } else {
            let mean_square = tail
                .iter()
                .map(|feature| {
                    let value = f64::from(finite_or_zero(*feature));
                    value * value
                })
                .sum::<f64>()
                / tail.len() as f64;
            Some(mean_square.max(0.0).sqrt() as f32)
        }
    });
    SemanticHeartbeatSignalMetricsV1 {
        feature_count,
        finite_feature_count,
        active_dimension_count,
        rms,
        component_stddev,
        max_abs,
        tail_rms,
        clipped_dimension_count,
    }
}

fn semantic_heartbeat_signal_comparison(
    features: &[f32],
    previous_features: &[f32],
) -> (u64, Option<f32>, Option<f32>) {
    let compared_count = features.len().min(previous_features.len());
    if compared_count == 0 {
        return (0, None, None);
    }
    let (delta_squares, dot, current_squares, previous_squares) = features
        .iter()
        .zip(previous_features)
        .fold((0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64), |acc, pair| {
            let current = f64::from(finite_or_zero(*pair.0));
            let previous = f64::from(finite_or_zero(*pair.1));
            let delta = current - previous;
            (
                acc.0 + delta * delta,
                acc.1 + current * previous,
                acc.2 + current * current,
                acc.3 + previous * previous,
            )
        });
    let delta_rms = (delta_squares / compared_count as f64).max(0.0).sqrt() as f32;
    let norm_product = (current_squares * previous_squares).max(0.0).sqrt();
    let cosine =
        (norm_product > f64::EPSILON).then(|| (dot / norm_product).clamp(-1.0, 1.0) as f32);
    (
        u64::try_from(compared_count).unwrap_or(u64::MAX),
        Some(delta_rms),
        cosine,
    )
}

fn semantic_heartbeat_continuity_classification(
    metrics: SemanticHeartbeatSignalMetricsV1,
    delta_rms: Option<f32>,
    cosine: Option<f32>,
) -> &'static str {
    if metrics.feature_count == 0 {
        "no_features_observed"
    } else if metrics.active_dimension_count <= 1
        || metrics.component_stddev <= SEMANTIC_HEARTBEAT_ACTIVE_DIM_EPSILON
    {
        "low_component_variance_observed"
    } else if delta_rms.is_some_and(|delta| delta <= SEMANTIC_HEARTBEAT_NEAR_REPEAT_DELTA_RMS)
        && cosine.is_some_and(|similarity| similarity >= SEMANTIC_HEARTBEAT_NEAR_REPEAT_COSINE)
    {
        "near_repeat_observed"
    } else if delta_rms.is_some() {
        "variation_observed_across_consecutive_pulses"
    } else {
        "first_sample_variation_baseline"
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RescueBridgePolicy {
    profile_name: String,
    bridge_enabled: bool,
    bridge_write_enabled: bool,
    bridge_autonomous_enabled: bool,
    bridge_write_profile: String,
    limited_write_enabled: bool,
    limited_write_policy_version: u64,
    limited_write_cooldown_secs: u64,
    limited_write_feature_scale: f32,
    limited_write_max_abs: f32,
    limited_write_min_fill_pct: f32,
    limited_write_max_fill_pct: f32,
    limited_write_rising_epsilon_pct: f32,
    limited_write_semantic_energy_rising_epsilon_pct: f32,
    limited_write_rollback_semantic_energy: f32,
    limited_write_health_max_age_secs: u64,
    limited_write_peak_fill_max_pct: f32,
    limited_write_required_stage: Option<String>,
    limited_write_allowed_stages: Vec<String>,
    limited_write_post_send_eval_secs: u64,
    limited_write_adverse_fill_rise_pct: f32,
    limited_write_adverse_cooldown_secs: u64,
    limited_write_rollback_target: Option<String>,
    limited_write_rollback_fill_pct: f32,
    limited_write_rollback_adverse_count: u64,
    limited_write_rollback_on_elevated_peak: bool,
    limited_write_require_zero_live_divisors: bool,
    limited_write_require_dampen_inquiry_text: bool,
    limited_write_block_structural_dump_language: bool,
    limited_write_block_terms_always: bool,
    limited_write_block_terms_on_rising: bool,
    limited_write_mute_live_intake_secs: u64,
    limited_write_pre_mute_live_intake_secs: u64,
    limited_write_require_pre_muted_live_intake: bool,
    limited_write_block_terms: Vec<String>,
    limited_write_allowed_modes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SemanticWriteContext<'a> {
    pub source: &'a str,
    pub mode: Option<&'a str>,
    pub text: Option<&'a str>,
    pub fill_pct: Option<f32>,
    pub previous_fill_pct: Option<f32>,
}

impl RescueBridgePolicy {
    fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let profile_name = object
            .get("profile")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let bridge_enabled = bool_field(value, "effective_bridge_enabled")
            .or_else(|| bool_field(value, "bridge_enabled"))
            .unwrap_or(true);
        let bridge_write_enabled = bool_field(value, "effective_bridge_write_enabled")
            .or_else(|| bool_field(value, "bridge_write_enabled"))
            .unwrap_or_else(|| {
                profile_name != "bridge_observe_only"
                    && profile_name != "bridge_telemetry_only"
                    && bridge_enabled
            });
        let bridge_autonomous_enabled = bool_field(value, "effective_bridge_autonomous_enabled")
            .or_else(|| bool_field(value, "bridge_autonomous_enabled"))
            .unwrap_or_else(|| profile_name != "bridge_telemetry_only" && bridge_enabled);
        let bridge_write_profile = string_field(value, "bridge_write_profile")
            .unwrap_or_else(|| "unrestricted".to_string());
        let is_limited_write_v2 = bridge_write_profile == LIMITED_WRITE_PROFILE_V2
            || bridge_write_profile == BUDGETED_SOVEREIGNTY_PROFILE
            || bridge_write_profile == FULL_EXPRESSION_PROFILE
            || profile_name == "bridge_limited_write_v2"
            || profile_name == "bridge_budgeted_sovereignty_v1"
            || profile_name == "bridge_full_expression_v1"
            || profile_name == "stable_core_v1";
        let limited_write_enabled = bool_field(value, "limited_write_enabled").unwrap_or(false)
            || bridge_write_profile == LIMITED_WRITE_PROFILE
            || is_limited_write_v2
            || profile_name == "bridge_limited_write";
        let inferred_policy_version = if is_limited_write_v2 {
            2
        } else if limited_write_enabled {
            1
        } else {
            0
        };
        let limited_write_required_stage = string_field(value, "limited_write_required_stage");
        let limited_write_allowed_stages =
            string_array_field(value, "limited_write_allowed_stages").unwrap_or_else(|| {
                limited_write_required_stage
                    .clone()
                    .map_or_else(|| vec!["hold".to_string()], |stage| vec![stage])
            });
        Some(Self {
            profile_name,
            bridge_enabled,
            bridge_write_enabled,
            bridge_autonomous_enabled,
            bridge_write_profile,
            limited_write_enabled,
            limited_write_policy_version: u64_field(value, "limited_write_policy_version")
                .unwrap_or(inferred_policy_version),
            limited_write_cooldown_secs: u64_field(value, "limited_write_cooldown_secs")
                .unwrap_or(300),
            limited_write_feature_scale: f32_field(value, "limited_write_feature_scale")
                .unwrap_or(0.08),
            limited_write_max_abs: f32_field(value, "limited_write_max_abs").unwrap_or(0.18),
            limited_write_min_fill_pct: f32_field(value, "limited_write_min_fill_pct")
                .unwrap_or(58.0),
            limited_write_max_fill_pct: f32_field(value, "limited_write_max_fill_pct")
                .unwrap_or(68.0),
            limited_write_rising_epsilon_pct: f32_field(value, "limited_write_rising_epsilon_pct")
                .unwrap_or(0.5),
            limited_write_semantic_energy_rising_epsilon_pct: f32_field(
                value,
                "limited_write_semantic_energy_rising_epsilon_pct",
            )
            .or_else(|| f32_field(value, "limited_write_rising_epsilon_pct"))
            .unwrap_or(0.5),
            limited_write_rollback_semantic_energy: f32_field(
                value,
                "limited_write_rollback_semantic_energy",
            )
            .unwrap_or(V2_ROLLBACK_SEMANTIC_ENERGY),
            limited_write_health_max_age_secs: u64_field(
                value,
                "limited_write_health_max_age_secs",
            )
            .unwrap_or(5),
            limited_write_peak_fill_max_pct: f32_field(value, "limited_write_peak_fill_max_pct")
                .unwrap_or(68.0),
            limited_write_required_stage,
            limited_write_allowed_stages,
            limited_write_post_send_eval_secs: u64_field(
                value,
                "limited_write_post_send_eval_secs",
            )
            .unwrap_or(120),
            limited_write_adverse_fill_rise_pct: f32_field(
                value,
                "limited_write_adverse_fill_rise_pct",
            )
            .unwrap_or(3.0),
            limited_write_adverse_cooldown_secs: u64_field(
                value,
                "limited_write_adverse_cooldown_secs",
            )
            .unwrap_or(1800),
            limited_write_rollback_target: string_field(value, "limited_write_rollback_target"),
            limited_write_rollback_fill_pct: f32_field(value, "limited_write_rollback_fill_pct")
                .unwrap_or(74.0),
            limited_write_rollback_adverse_count: u64_field(
                value,
                "limited_write_rollback_adverse_count",
            )
            .unwrap_or(2),
            limited_write_rollback_on_elevated_peak: bool_field(
                value,
                "limited_write_rollback_on_elevated_peak",
            )
            .unwrap_or(true),
            limited_write_require_zero_live_divisors: bool_field(
                value,
                "limited_write_require_zero_live_divisors",
            )
            .unwrap_or(true),
            limited_write_require_dampen_inquiry_text: bool_field(
                value,
                "limited_write_require_dampen_inquiry_text",
            )
            .unwrap_or(true),
            limited_write_block_structural_dump_language: bool_field(
                value,
                "limited_write_block_structural_dump_language",
            )
            .unwrap_or(true),
            limited_write_block_terms_always: bool_field(value, "limited_write_block_terms_always")
                .unwrap_or(false),
            limited_write_block_terms_on_rising: bool_field(
                value,
                "limited_write_block_terms_on_rising",
            )
            .unwrap_or(true),
            limited_write_mute_live_intake_secs: u64_field(
                value,
                "limited_write_mute_live_intake_secs",
            )
            .unwrap_or(0),
            limited_write_pre_mute_live_intake_secs: u64_field(
                value,
                "limited_write_pre_mute_live_intake_secs",
            )
            .unwrap_or(0),
            limited_write_require_pre_muted_live_intake: bool_field(
                value,
                "limited_write_require_pre_muted_live_intake",
            )
            .unwrap_or(false),
            limited_write_block_terms: string_array_field(value, "limited_write_block_terms")
                .unwrap_or_else(default_limited_write_block_terms),
            limited_write_allowed_modes: string_array_field(value, "limited_write_allowed_modes")
                .unwrap_or_else(|| {
                    if is_limited_write_v2 {
                        default_limited_write_v2_allowed_modes()
                    } else {
                        default_limited_write_allowed_modes()
                    }
                }),
        })
    }

    fn semantic_ingress_block_reason(&self) -> Option<String> {
        if !self.bridge_enabled {
            return Some(format!(
                "rescue profile '{}' has bridge ingress disabled",
                self.profile_name
            ));
        }
        if !self.bridge_write_enabled {
            return Some(format!(
                "rescue profile '{}' blocks semantic ingress",
                self.profile_name
            ));
        }
        if self.limited_write_active() {
            return Some(format!(
                "rescue profile '{}' requires the limited dampen/inquiry semantic gate",
                self.profile_name
            ));
        }
        None
    }

    fn autonomous_enabled(&self) -> bool {
        self.bridge_enabled && self.bridge_autonomous_enabled
    }

    fn sensory_connection_enabled(&self) -> bool {
        self.bridge_enabled && (self.bridge_write_enabled || self.bridge_autonomous_enabled)
    }

    fn limited_write_active(&self) -> bool {
        self.bridge_write_enabled
            && self.limited_write_enabled
            && matches!(
                self.bridge_write_profile.as_str(),
                LIMITED_WRITE_PROFILE
                    | LIMITED_WRITE_PROFILE_V2
                    | BUDGETED_SOVEREIGNTY_PROFILE
                    | FULL_EXPRESSION_PROFILE
            )
    }

    fn limited_write_v2_active(&self) -> bool {
        self.limited_write_active()
            && self.limited_write_policy_version == 2
            && matches!(
                self.bridge_write_profile.as_str(),
                LIMITED_WRITE_PROFILE_V2 | BUDGETED_SOVEREIGNTY_PROFILE | FULL_EXPRESSION_PROFILE
            )
    }

    fn limited_write_block_reason(
        &self,
        context: &SemanticWriteContext<'_>,
        profile_path: &Path,
        status_path: &Path,
    ) -> Option<String> {
        if !self.bridge_enabled {
            return Some(format!(
                "rescue profile '{}' has bridge ingress disabled",
                self.profile_name
            ));
        }
        if !self.bridge_write_enabled {
            return Some(format!(
                "rescue profile '{}' blocks semantic ingress",
                self.profile_name
            ));
        }
        if !self.limited_write_active() {
            return None;
        }
        if !limited_write_source_allowed(context.source) {
            return Some(format!(
                "limited-write profile only allows source '{LIMITED_WRITE_SOURCE}' or '{MCP_LIMITED_WRITE_SOURCE}'"
            ));
        }

        let now = now_unix_s();
        let mut status = read_status(status_path);
        let health = if self.limited_write_v2_active() {
            match load_limited_write_health(profile_path, self.limited_write_health_max_age_secs) {
                Ok(health) => {
                    if let Some(reason) = self.evaluate_v2_previous_send(
                        profile_path,
                        status_path,
                        &mut status,
                        &health,
                        now,
                    ) {
                        return Some(reason);
                    }
                    Some(health)
                },
                Err(reason) => return Some(reason),
            }
        } else {
            None
        };

        if let Some(reason) = self.cooldown_block_reason(&status, now) {
            return Some(reason);
        }

        let fill_pct = health
            .as_ref()
            .map_or(context.fill_pct, |health| Some(health.fill_pct));
        let Some(fill_pct) = fill_pct else {
            return Some("limited-write profile requires current fill".to_string());
        };
        if fill_pct < self.limited_write_min_fill_pct {
            return Some(format!(
                "limited-write profile blocks semantic ingress below {:.1}% fill",
                self.limited_write_min_fill_pct
            ));
        }
        if fill_pct > self.limited_write_max_fill_pct {
            return Some(format!(
                "limited-write profile blocks semantic ingress above {:.1}% fill",
                self.limited_write_max_fill_pct
            ));
        }

        let mode = context.mode.unwrap_or_default();
        if !mode.is_empty()
            && !self
                .limited_write_allowed_modes
                .iter()
                .any(|allowed| allowed == mode)
        {
            return Some(format!(
                "limited-write profile blocks mode '{mode}' outside dampen/inquiry lane"
            ));
        }

        let text = context.text.unwrap_or_default();
        let lower = text.to_lowercase();
        if self.limited_write_v2_active() {
            if self.limited_write_block_terms_always
                && let Some(term) = self
                    .limited_write_block_terms
                    .iter()
                    .find(|term| lower.contains(&term.to_lowercase()))
            {
                return Some(format!(
                    "limited-write profile blocks trigger language '{term}'"
                ));
            }
            if let Some(reason) = self.v2_health_block_reason(context, health.as_ref()?) {
                return Some(reason);
            }
            if self.limited_write_block_structural_dump_language
                && contains_structural_dump_language(text)
            {
                return Some(
                    "limited-write v2 blocks structural spectral dump language".to_string(),
                );
            }
            if self.limited_write_require_dampen_inquiry_text
                && !looks_like_limited_write_v2_text(text, mode)
            {
                return Some(
                    "limited-write v2 allows only dampening or inquiry-shaped text".to_string(),
                );
            }
            if self.limited_write_require_pre_muted_live_intake {
                let health = health.as_ref()?;
                if !health.semantic_mute_active
                    || health.live_audio_divisor != 0
                    || health.live_video_divisor != 0
                {
                    write_limited_write_sensory_mute(
                        status_path,
                        self,
                        now,
                        self.limited_write_pre_mute_live_intake_secs
                            .max(self.limited_write_mute_live_intake_secs),
                        "limited_write_pre_mute_before_semantic_send",
                    );
                    return Some(
                        "limited-write v2 pre-muted live audio/video before semantic send"
                            .to_string(),
                    );
                }
            }
        } else if !looks_like_dampen_or_inquiry(text, mode) {
            return Some(
                "limited-write profile allows only dampening or inquiry-shaped text".to_string(),
            );
        }

        if self.limited_write_block_terms_always
            && let Some(term) = self
                .limited_write_block_terms
                .iter()
                .find(|term| lower.contains(&term.to_lowercase()))
        {
            return Some(format!(
                "limited-write profile blocks trigger language '{term}'"
            ));
        }

        let fill_rising = context
            .previous_fill_pct
            .is_some_and(|previous| fill_pct - previous > self.limited_write_rising_epsilon_pct);
        if fill_rising
            && self.limited_write_block_terms_on_rising
            && let Some(term) = self
                .limited_write_block_terms
                .iter()
                .find(|term| lower.contains(&term.to_lowercase()))
        {
            return Some(format!(
                "limited-write profile blocks rising-fill trigger language '{term}'"
            ));
        }

        None
    }

    fn apply_limited_write_shape(&self, features: &mut [f32]) {
        let scale = self.limited_write_feature_scale.clamp(0.0, 1.0);
        let max_abs = self.limited_write_max_abs.clamp(0.0, 5.0);
        for feature in features {
            *feature = (*feature * scale).clamp(-max_abs, max_abs);
        }
    }

    fn heartbeat_block_reason(&self, profile_path: &Path) -> Option<String> {
        if !self.bridge_enabled {
            return Some(format!(
                "rescue profile '{}' has bridge ingress disabled",
                self.profile_name
            ));
        }
        if !self.bridge_write_enabled && !self.bridge_autonomous_enabled {
            return Some(format!(
                "rescue profile '{}' blocks semantic heartbeat ingress",
                self.profile_name
            ));
        }
        if !self.limited_write_v2_active() {
            return None;
        }

        let health =
            match load_limited_write_health(profile_path, self.limited_write_health_max_age_secs) {
                Ok(health) => health,
                Err(reason) => return Some(reason),
            };
        if health.semantic_mute_active {
            return Some("semantic heartbeat blocked while semantic mute is active".to_string());
        }
        if health.stage == "discharge" {
            return Some("semantic heartbeat blocked during discharge".to_string());
        }
        if health.fill_pct >= self.limited_write_peak_fill_max_pct
            || health.peak_fill_pct_60s >= self.limited_write_peak_fill_max_pct
        {
            return Some(format!(
                "semantic heartbeat blocked when 60s peak guard is {:.1}% or higher",
                self.limited_write_peak_fill_max_pct
            ));
        }
        if let Some(watchdog_state) = health.watchdog_state.as_deref()
            && !(watchdog_state == "monitoring"
                || watchdog_state == "warmup"
                || watchdog_state == "monitoring:degraded")
        {
            return Some(format!(
                "semantic heartbeat blocked by watchdog state '{watchdog_state}'"
            ));
        }
        None
    }

    fn apply_semantic_heartbeat_shape(features: &mut [f32]) {
        for feature in features {
            *feature = (*feature * SEMANTIC_HEARTBEAT_FEATURE_SCALE)
                .clamp(-SEMANTIC_HEARTBEAT_MAX_ABS, SEMANTIC_HEARTBEAT_MAX_ABS);
        }
    }

    fn cooldown_block_reason(&self, status: &Value, now: f64) -> Option<String> {
        if !status_matches_policy(status, self) {
            return None;
        }
        if self.limited_write_v2_active()
            && let Some(cooldown_until) =
                status.get("cooldown_until_unix_s").and_then(Value::as_f64)
        {
            let cooldown_remaining = cooldown_until - now;
            if cooldown_remaining > 0.0 {
                return Some(format!(
                    "limited-write cooldown active for {:.0}s",
                    cooldown_remaining.ceil()
                ));
            }
        }
        if let Some(last_sent_at) = status.get("last_sent_at_unix_s").and_then(Value::as_f64) {
            let cooldown_remaining = last_sent_at + self.limited_write_cooldown_secs as f64 - now;
            if cooldown_remaining > 0.0 {
                return Some(format!(
                    "limited-write cooldown active for {:.0}s",
                    cooldown_remaining.ceil()
                ));
            }
        }
        None
    }

    fn v2_health_block_reason(
        &self,
        context: &SemanticWriteContext<'_>,
        health: &LimitedWriteHealth,
    ) -> Option<String> {
        if let Some(watchdog_state) = health.watchdog_state.as_deref()
            && watchdog_state != "monitoring"
        {
            return Some(format!(
                "limited-write v2 requires watchdog monitoring; saw '{watchdog_state}'"
            ));
        }
        if !self
            .limited_write_allowed_stages
            .iter()
            .any(|stage| stage == &health.stage)
        {
            let required_stage = self
                .limited_write_required_stage
                .as_deref()
                .unwrap_or("hold");
            return Some(format!(
                "limited-write v2 requires rescue stage '{required_stage}'"
            ));
        }
        if health.peak_fill_pct_60s >= self.limited_write_peak_fill_max_pct {
            return Some(format!(
                "limited-write v2 blocks semantic ingress when 60s peak is {:.1}% or higher",
                self.limited_write_peak_fill_max_pct
            ));
        }
        if health.semantic_active {
            return Some("limited-write v2 requires inactive semantic state".to_string());
        }
        if health.semantic_energy > V2_SEMANTIC_ENERGY_MAX {
            return Some(format!(
                "limited-write v2 blocks semantic ingress while semantic energy exceeds {:.2}",
                V2_SEMANTIC_ENERGY_MAX
            ));
        }
        if self.limited_write_require_zero_live_divisors
            && (health.live_audio_divisor != 0 || health.live_video_divisor != 0)
        {
            return Some(
                "limited-write v2 requires live audio/video divisors to remain zero".to_string(),
            );
        }
        let Some(previous_fill_pct) = context.previous_fill_pct else {
            return Some("limited-write v2 requires previous fill sample".to_string());
        };
        let fill_delta = health.fill_pct - previous_fill_pct;
        if fill_delta > self.limited_write_rising_epsilon_pct {
            return Some(format!(
                "limited-write v2 blocks rising fill delta {:.2}%",
                fill_delta
            ));
        }
        None
    }

    fn evaluate_v2_previous_send(
        &self,
        profile_path: &Path,
        status_path: &Path,
        status: &mut Value,
        health: &LimitedWriteHealth,
        now: f64,
    ) -> Option<String> {
        if !status_matches_policy(status, self) {
            return None;
        }
        let last_sent_at = status.get("last_sent_at_unix_s").and_then(Value::as_f64)?;
        let last_sent_fill_pct = status
            .get("last_sent_fill_pct")
            .and_then(Value::as_f64)
            .map(|value| value as f32)?;
        let eval_window = self.limited_write_post_send_eval_secs as f64;
        let elapsed = now - last_sent_at;
        let already_final = status
            .get("last_send_evaluation")
            .and_then(|value| value.get("sent_at_unix_s"))
            .and_then(Value::as_f64)
            .is_some_and(|sent_at| (sent_at - last_sent_at).abs() < f64::EPSILON)
            && status
                .get("last_send_evaluation")
                .and_then(|value| value.get("state"))
                .and_then(Value::as_str)
                .is_some_and(|state| matches!(state, "adverse" | "healthy"));

        if already_final && elapsed > eval_window {
            return None;
        }

        let fill_delta = health.fill_pct - last_sent_fill_pct;
        if elapsed <= eval_window {
            if let Some(watchdog_state) = health.watchdog_state.as_deref()
                && watchdog_state != "monitoring"
            {
                if matches!(watchdog_state, "warmup" | "monitoring:degraded") {
                    if !already_final {
                        status["last_send_evaluation"] = json!({
                            "state": "watching",
                            "sent_at_unix_s": last_sent_at,
                            "evaluated_at_unix_s": now,
                            "seconds_since_send": elapsed,
                            "health_fill_pct": health.fill_pct,
                            "watchdog_state": watchdog_state
                        });
                        write_status(status_path, status);
                    }
                    return Some(format!(
                        "limited-write v2 waiting for watchdog monitoring; saw '{watchdog_state}'"
                    ));
                }
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    &format!("post-write watchdog state became '{watchdog_state}'"),
                    now,
                );
            }
            if health.fill_pct >= self.limited_write_rollback_fill_pct {
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    &format!(
                        "post-write fill reached {:.1}% after limited-write v2 send",
                        health.fill_pct
                    ),
                    now,
                );
            }
            let rollback_stage = self.limited_write_rollback_on_elevated_peak
                && (health.stage == "discharge"
                    || (health.stage == "elevated"
                        && health.fill_pct >= self.limited_write_peak_fill_max_pct));
            if rollback_stage {
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    &format!(
                        "post-write rescue stage entered '{}' after limited-write v2 send",
                        health.stage
                    ),
                    now,
                );
            }
            if health.semantic_energy > self.limited_write_rollback_semantic_energy
                && fill_delta > self.limited_write_semantic_energy_rising_epsilon_pct
            {
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    "post-write semantic energy rose while fill was rising",
                    now,
                );
            }
            if already_final {
                return None;
            }
            if fill_delta >= self.limited_write_adverse_fill_rise_pct && !already_final {
                let adverse_count = increment_adverse_count(status, now);
                status["last_send_evaluation"] = json!({
                    "state": "adverse",
                    "sent_at_unix_s": last_sent_at,
                    "evaluated_at_unix_s": now,
                    "seconds_since_send": elapsed,
                    "fill_delta_pct": fill_delta,
                    "health_fill_pct": health.fill_pct,
                    "reason": "fill_rise"
                });
                status["cooldown_secs"] = json!(self.limited_write_adverse_cooldown_secs);
                status["cooldown_until_unix_s"] =
                    json!(now + self.limited_write_adverse_cooldown_secs as f64);
                write_status(status_path, status);
                if adverse_count >= self.limited_write_rollback_adverse_count {
                    return self.rollback_v2(
                        profile_path,
                        status_path,
                        status,
                        "limited-write v2 saw repeated adverse fill rises",
                        now,
                    );
                }
                return None;
            }

            status["last_send_evaluation"] = json!({
                "state": "watching",
                "sent_at_unix_s": last_sent_at,
                "evaluated_at_unix_s": now,
                "seconds_since_send": elapsed,
                "fill_delta_pct": fill_delta,
                "health_fill_pct": health.fill_pct
            });
            write_status(status_path, status);
        } else if !already_final {
            status["last_send_evaluation"] = json!({
                "state": "healthy",
                "sent_at_unix_s": last_sent_at,
                "evaluated_at_unix_s": now,
                "seconds_since_send": elapsed,
                "fill_delta_pct": fill_delta,
                "health_fill_pct": health.fill_pct
            });
            write_status(status_path, status);
        }
        None
    }

    fn rollback_v2(
        &self,
        profile_path: &Path,
        status_path: &Path,
        status: &mut Value,
        reason: &str,
        now: f64,
    ) -> Option<String> {
        let target = self
            .limited_write_rollback_target
            .as_deref()
            .unwrap_or(OBSERVE_ONLY_PROFILE);
        if let Err(error) = rollback_profile_to_observe_only(profile_path, target, reason, now) {
            return Some(format!("limited-write v2 rollback failed: {error}"));
        }
        status["rollback_at_unix_s"] = json!(now);
        status["rollback_reason"] = json!(reason);
        status["rolled_back_from_profile"] = json!(self.profile_name);
        status["rolled_back_to_profile"] = json!(target);
        status["last_block_reason"] = json!(format!("rolled back: {reason}"));
        write_status(status_path, status);
        Some(format!(
            "limited-write v2 rolled back to {target}: {reason}"
        ))
    }
}

#[derive(Debug, Clone)]
struct LimitedWriteHealth {
    fill_pct: f32,
    stage: String,
    peak_fill_pct_60s: f32,
    semantic_active: bool,
    semantic_energy: f32,
    live_audio_divisor: i64,
    live_video_divisor: i64,
    semantic_mute_active: bool,
    watchdog_state: Option<String>,
    age_secs: f64,
    pressure_risk: Option<f32>,
    spectral_entropy: Option<f32>,
    shadow_dispersal_potential: Option<f32>,
}

fn load_policy(path: &Path) -> Option<RescueBridgePolicy> {
    let payload = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&payload).ok()?;
    RescueBridgePolicy::from_value(&value)
}

fn health_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("health.json")
}

fn rescue_status_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("rescue_status.json")
}

fn load_limited_write_health(
    profile_path: &Path,
    max_age_secs: u64,
) -> Result<LimitedWriteHealth, String> {
    let health_path = health_path_for_profile(profile_path);
    let metadata = std::fs::metadata(&health_path)
        .map_err(|_| "limited-write v2 requires fresh health.json".to_string())?;
    let modified = metadata
        .modified()
        .map_err(|_| "limited-write v2 could not read health.json mtime".to_string())?;
    let age_secs = SystemTime::now()
        .duration_since(modified)
        .map_err(|_| "limited-write v2 health.json mtime is in the future".to_string())?
        .as_secs_f64();
    if age_secs > max_age_secs as f64 {
        return Err(format!(
            "limited-write v2 requires fresh health.json; age {:.1}s exceeds {max_age_secs}s",
            age_secs
        ));
    }

    let payload = std::fs::read_to_string(&health_path)
        .map_err(|_| "limited-write v2 could not read health.json".to_string())?;
    let value: Value = serde_json::from_str(&payload)
        .map_err(|_| "limited-write v2 could not parse health.json".to_string())?;
    let rescue_status_path = rescue_status_path_for_profile(profile_path);
    let watchdog_state = std::fs::read_to_string(&rescue_status_path)
        .ok()
        .and_then(|payload| serde_json::from_str::<Value>(&payload).ok())
        .and_then(|value| {
            value
                .get("watchdog_state")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });
    let fill_pct = f32_required(&value, &["fill_pct"])?;
    Ok(LimitedWriteHealth {
        fill_pct,
        stage: str_optional(&value, &["rescue", "stage"])
            .or_else(|| str_optional(&value, &["stable_core", "stage"]))
            .ok_or_else(|| {
                "limited-write v2 health.json missing rescue.stage or stable_core.stage".to_string()
            })?,
        peak_fill_pct_60s: f32_optional(&value, &["rescue", "peak_fill_pct_60s"])
            .or_else(|| f32_optional(&value, &["stable_core", "peak_fill_pct_60s"]))
            .unwrap_or(fill_pct),
        semantic_active: bool_optional(&value, &["semantic_energy_v1", "kernel_active"])
            .or_else(|| bool_optional(&value, &["semantic", "kernel_active"]))
            .or_else(|| bool_optional(&value, &["semantic", "active"]))
            .unwrap_or(false),
        semantic_energy: f32_optional(&value, &["semantic_energy_v1", "regulator_drive_energy"])
            .or_else(|| f32_optional(&value, &["semantic", "regulator_drive_energy"]))
            .or_else(|| f32_optional(&value, &["semantic", "kernel_energy"]))
            .or_else(|| f32_optional(&value, &["semantic", "energy"]))
            .unwrap_or(0.0),
        live_audio_divisor: i64_required(&value, &["sensory", "live_audio_divisor"])?,
        live_video_divisor: i64_required(&value, &["sensory", "live_video_divisor"])?,
        semantic_mute_active: bool_optional(
            &value,
            &["stable_core", "sensory_budget", "semantic_mute_active"],
        )
        .unwrap_or(false),
        watchdog_state,
        age_secs,
        pressure_risk: finite_f32_optional(&value, &["resonance_density_v1", "pressure_risk"])
            .or_else(|| finite_f32_optional(&value, &["pressure_source_status", "pressure_score"])),
        spectral_entropy: finite_f32_optional(&value, &["transition_event_v1", "spectral_entropy"]),
        shadow_dispersal_potential: finite_f32_optional(
            &value,
            &["shadow_preservation_mode_v1", "dispersal_potential"],
        ),
    })
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn f32_required(value: &Value, path: &[&str]) -> Result<f32, String> {
    f32_optional(value, path)
        .ok_or_else(|| format!("limited-write v2 health.json missing {}", path.join(".")))
}

fn i64_required(value: &Value, path: &[&str]) -> Result<i64, String> {
    value_at_path(value, path)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("limited-write v2 health.json missing {}", path.join(".")))
}

fn f32_optional(value: &Value, path: &[&str]) -> Option<f32> {
    value_at_path(value, path)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
}

fn finite_f32_optional(value: &Value, path: &[&str]) -> Option<f32> {
    f32_optional(value, path).filter(|value| value.is_finite())
}

fn bool_optional(value: &Value, path: &[&str]) -> Option<bool> {
    value_at_path(value, path).and_then(Value::as_bool)
}

fn str_optional(value: &Value, path: &[&str]) -> Option<String> {
    value_at_path(value, path)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn limited_write_status_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime")
        .join(LIMITED_WRITE_STATUS_FILE)
}

fn semantic_heartbeat_status_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime")
        .join(SEMANTIC_HEARTBEAT_STATUS_FILE)
}

fn limited_write_sensory_mute_path_for_status(status_path: &Path) -> PathBuf {
    status_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LIMITED_WRITE_SENSORY_MUTE_FILE)
}

fn now_unix_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn limited_write_source_allowed(source: &str) -> bool {
    matches!(source, LIMITED_WRITE_SOURCE | MCP_LIMITED_WRITE_SOURCE)
}

fn read_status(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|payload| serde_json::from_str(&payload).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_status(path: &Path, status: &Value) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(payload) = serde_json::to_string_pretty(status) {
        let _ = std::fs::write(path, payload);
    }
}

fn write_limited_write_sensory_mute(
    status_path: &Path,
    policy: &RescueBridgePolicy,
    now: f64,
    duration_secs: u64,
    reason: &str,
) -> Option<f64> {
    if duration_secs == 0 {
        return None;
    }
    let mute_until = now + duration_secs as f64;
    let mute_path = limited_write_sensory_mute_path_for_status(status_path);
    if let Some(parent) = mute_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = json!({
        "active_until_unix_s": mute_until,
        "duration_secs": duration_secs,
        "reason": reason,
        "source_profile": policy.profile_name,
        "last_semantic_sent_at_unix_s": now,
    });
    if let Ok(pretty) = serde_json::to_string_pretty(&payload) {
        let _ = std::fs::write(mute_path, pretty);
    }
    Some(mute_until)
}

fn status_matches_policy(status: &Value, policy: &RescueBridgePolicy) -> bool {
    if status.get("profile").and_then(Value::as_str) != Some(policy.profile_name.as_str()) {
        return false;
    }
    match status.get("policy_version").and_then(Value::as_u64) {
        Some(version) => version == policy.limited_write_policy_version,
        None => policy.limited_write_policy_version <= 1,
    }
}

fn increment_adverse_count(status: &mut Value, now: f64) -> u64 {
    let window_started = status
        .get("adverse_window_started_at_unix_s")
        .and_then(Value::as_f64)
        .filter(|started| now - *started <= V2_ADVERSE_WINDOW_SECS)
        .unwrap_or(now);
    let previous_count = if (window_started - now).abs() < f64::EPSILON {
        0
    } else {
        status
            .get("adverse_response_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let count = previous_count.saturating_add(1);
    status["adverse_window_started_at_unix_s"] = json!(window_started);
    status["adverse_response_count"] = json!(count);
    count
}

fn matched_watch_terms(text: &str, policy: &RescueBridgePolicy) -> Vec<String> {
    let lower = text.to_lowercase();
    policy
        .limited_write_block_terms
        .iter()
        .filter(|term| lower.contains(&term.to_lowercase()))
        .cloned()
        .collect()
}

fn rollback_profile_to_observe_only(
    profile_path: &Path,
    target: &str,
    reason: &str,
    now: f64,
) -> Result<(), String> {
    let payload = std::fs::read_to_string(profile_path)
        .map_err(|error| format!("read profile failed: {error}"))?;
    let mut profile: Value =
        serde_json::from_str(&payload).map_err(|error| format!("parse profile failed: {error}"))?;
    let runtime_dir = profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime");
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("create runtime dir failed: {error}"))?;
    let archive_path =
        runtime_dir.join(format!("bridge_limited_write_v2_rollback_{:.0}.json", now));
    let pretty_original = serde_json::to_string_pretty(&profile)
        .map_err(|error| format!("serialize rollback archive failed: {error}"))?;
    std::fs::write(&archive_path, pretty_original)
        .map_err(|error| format!("write rollback archive failed: {error}"))?;
    let rolled_back_from_profile = profile
        .get("profile")
        .and_then(Value::as_str)
        .unwrap_or("bridge_limited_write_v2")
        .to_string();

    let Some(object) = profile.as_object_mut() else {
        return Err("profile root is not a JSON object".to_string());
    };
    object.insert("profile".to_string(), json!(target));
    object.insert("bridge_enabled".to_string(), json!(true));
    object.insert("effective_bridge_enabled".to_string(), json!(true));
    object.insert("bridge_write_enabled".to_string(), json!(false));
    object.insert("effective_bridge_write_enabled".to_string(), json!(false));
    object.insert("bridge_autonomous_enabled".to_string(), json!(true));
    object.insert(
        "effective_bridge_autonomous_enabled".to_string(),
        json!(true),
    );
    object.insert("bridge_write_profile".to_string(), json!("observe_only"));
    object.insert("limited_write_enabled".to_string(), json!(false));
    object.insert(
        "rolled_back_from_profile".to_string(),
        json!(rolled_back_from_profile),
    );
    object.insert("rolled_back_to_profile".to_string(), json!(target));
    object.insert("rollback_reason".to_string(), json!(reason));
    object.insert("rollback_at_unix_s".to_string(), json!(now));

    let pretty = serde_json::to_string_pretty(&profile)
        .map_err(|error| format!("serialize rolled-back profile failed: {error}"))?;
    std::fs::write(profile_path, pretty)
        .map_err(|error| format!("write rolled-back profile failed: {error}"))
}

fn record_limited_write_block(path: &Path, policy: &RescueBridgePolicy, reason: &str) {
    let mut status = read_status(path);
    if !status.is_object() || !status_matches_policy(&status, policy) {
        status = json!({});
    }
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["last_block_at_unix_s"] = json!(now_unix_s());
    status["last_block_reason"] = json!(reason);
    write_status(path, &status);
}

fn record_semantic_heartbeat_observation(
    status: &mut Value,
    observation: SemanticHeartbeatObservationV1,
    outcome: &str,
    health: Option<&LimitedWriteHealth>,
    delivered_signal: Option<SemanticHeartbeatSignalMetricsV1>,
) -> f64 {
    let now = now_unix_s();
    let prior_send_count = status
        .get("send_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prior_block_count = status
        .get("block_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prior_attempt_count = status
        .get("attempt_count")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| prior_send_count.saturating_add(prior_block_count));
    let send_count = if outcome == "sent" {
        prior_send_count.saturating_add(1)
    } else {
        prior_send_count
    };
    let block_count = if outcome == "blocked" {
        prior_block_count.saturating_add(1)
    } else {
        prior_block_count
    };
    let attempt_count = prior_attempt_count.saturating_add(1);

    let mut window_samples: Vec<Value> = status
        .get("window_samples_v1")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|sample| {
            sample
                .get("at_unix_s")
                .and_then(Value::as_f64)
                .is_some_and(|at| {
                    at.is_finite()
                        && now >= at
                        && now - at <= SEMANTIC_HEARTBEAT_OBSERVATION_WINDOW_SECS
                })
        })
        .cloned()
        .collect();
    let mut sample = json!({
        "at_unix_s": now,
        "outcome": outcome,
        "source": observation.source,
        "phase_step": observation.phase_step,
        "phase": if observation.phase.is_finite() {
            observation.phase.clamp(0.0, 1.0)
        } else {
            0.0
        },
        "configured_interval_secs": observation.interval_secs,
        "configured_intensity": if observation.configured_intensity.is_finite() {
            observation.configured_intensity.max(0.0)
        } else {
            0.0
        }
    });
    if let Some(health) = health {
        sample["fill_pct"] = json!(health.fill_pct);
        sample["rescue_stage"] = json!(health.stage);
        sample["pressure_risk"] = json!(health.pressure_risk);
        sample["spectral_entropy"] = json!(health.spectral_entropy);
        sample["shadow_dispersal_potential"] = json!(health.shadow_dispersal_potential);
    }
    if let Some(signal) = observation.signal_evidence {
        sample["signal_evidence_v1"] =
            semantic_heartbeat_signal_evidence_json(signal, delivered_signal);
    }
    if let Some(context) = observation.texture_context.as_ref() {
        sample["texture_context_v1"] = semantic_heartbeat_texture_context_json(context);
    }
    window_samples.push(sample);
    let window_samples_truncated =
        window_samples.len() > SEMANTIC_HEARTBEAT_OBSERVATION_MAX_SAMPLES;
    if window_samples_truncated {
        let keep_from = window_samples
            .len()
            .saturating_sub(SEMANTIC_HEARTBEAT_OBSERVATION_MAX_SAMPLES);
        window_samples.drain(..keep_from);
    }
    let window_attempt_count = u64::try_from(window_samples.len()).unwrap_or(u64::MAX);
    let window_send_count = u64::try_from(
        window_samples
            .iter()
            .filter(|sample| sample.get("outcome").and_then(Value::as_str) == Some("sent"))
            .count(),
    )
    .unwrap_or(u64::MAX);
    let window_block_count = u64::try_from(
        window_samples
            .iter()
            .filter(|sample| sample.get("outcome").and_then(Value::as_str) == Some("blocked"))
            .count(),
    )
    .unwrap_or(u64::MAX);
    let window_started = window_samples
        .first()
        .and_then(|sample| sample.get("at_unix_s"))
        .and_then(Value::as_f64)
        .unwrap_or(now);
    let window_skip_rate = if window_attempt_count == 0 {
        0.0
    } else {
        window_block_count as f64 / window_attempt_count as f64
    };
    let mean_pressure_when_blocked =
        semantic_heartbeat_window_mean(&window_samples, "blocked", "pressure_risk");
    let mean_pressure_when_sent =
        semantic_heartbeat_window_mean(&window_samples, "sent", "pressure_risk");
    let blocked_minus_sent_mean_pressure = mean_pressure_when_blocked
        .zip(mean_pressure_when_sent)
        .map(|(blocked, sent)| blocked - sent);
    let pressure_context_sample_count = window_samples
        .iter()
        .filter(|sample| {
            sample
                .get("pressure_risk")
                .and_then(Value::as_f64)
                .is_some()
        })
        .count();
    let comparison_state =
        if mean_pressure_when_blocked.is_some() && mean_pressure_when_sent.is_some() {
            "comparison_available_causation_not_inferred"
        } else {
            "insufficient_cross_outcome_pressure_context"
        };
    let signal_samples: Vec<&Value> = window_samples
        .iter()
        .filter_map(|sample| sample.get("signal_evidence_v1"))
        .collect();
    let signal_sample_count = u64::try_from(signal_samples.len()).unwrap_or(u64::MAX);
    let signal_variation_count = semantic_heartbeat_signal_class_count(
        &signal_samples,
        "variation_observed_across_consecutive_pulses",
    );
    let signal_near_repeat_count =
        semantic_heartbeat_signal_class_count(&signal_samples, "near_repeat_observed");
    let signal_low_variance_count =
        semantic_heartbeat_signal_class_count(&signal_samples, "low_component_variance_observed");
    let signal_mean_delta_rms = semantic_heartbeat_nested_mean(
        &signal_samples,
        &["consecutive_comparison", "delta_rms_from_previous"],
    );
    let signal_mean_generated_component_stddev =
        semantic_heartbeat_nested_mean(&signal_samples, &["generated", "component_stddev"]);
    let signal_mean_delivered_component_stddev =
        semantic_heartbeat_nested_mean(&signal_samples, &["delivered", "component_stddev"]);
    let latest_signal_classification = signal_samples.last().and_then(|signal| {
        signal
            .get("continuity_classification")
            .and_then(Value::as_str)
    });
    let phase_entropy_review = semantic_heartbeat_phase_entropy_review_v1(&window_samples, now);
    let signal_texture_review = semantic_heartbeat_signal_texture_review_v1(
        &window_samples,
        window_attempt_count,
        window_block_count,
        window_skip_rate,
    );

    status["attempt_count"] = json!(attempt_count);
    status["send_count"] = json!(send_count);
    status["block_count"] = json!(block_count);
    status["last_attempt_at_unix_s"] = json!(now);
    status["last_outcome"] = json!(outcome);
    status["last_source"] = json!(observation.source);
    status["last_phase_step"] = json!(observation.phase_step);
    status["last_phase"] = json!(if observation.phase.is_finite() {
        observation.phase.clamp(0.0, 1.0)
    } else {
        0.0
    });
    status["configured_interval_secs"] = json!(observation.interval_secs);
    status["configured_intensity"] = json!(if observation.configured_intensity.is_finite() {
        observation.configured_intensity.max(0.0)
    } else {
        0.0
    });
    status["window_started_at_unix_s"] = json!(window_started);
    status["window_duration_secs"] = json!(SEMANTIC_HEARTBEAT_OBSERVATION_WINDOW_SECS);
    status["window_sample_capacity"] = json!(SEMANTIC_HEARTBEAT_OBSERVATION_MAX_SAMPLES);
    status["window_samples_truncated"] = json!(window_samples_truncated);
    status["window_samples_v1"] = json!(window_samples);
    status["window_attempt_count"] = json!(window_attempt_count);
    status["window_send_count"] = json!(window_send_count);
    status["window_block_count"] = json!(window_block_count);
    status["window_skip_rate"] = json!(window_skip_rate);
    status["last_pressure_risk"] = json!(health.and_then(|value| value.pressure_risk));
    status["last_spectral_entropy"] = json!(health.and_then(|value| value.spectral_entropy));
    status["last_shadow_dispersal_potential"] =
        json!(health.and_then(|value| value.shadow_dispersal_potential));
    status["pressure_texture_review_v1"] = json!({
        "schema": "semantic_heartbeat_pressure_texture_review_v1",
        "schema_version": 1,
        "window_pressure_context_sample_count": pressure_context_sample_count,
        "mean_pressure_risk_when_blocked": mean_pressure_when_blocked,
        "mean_pressure_risk_when_sent": mean_pressure_when_sent,
        "blocked_minus_sent_mean_pressure_risk": blocked_minus_sent_mean_pressure,
        "comparison_state": comparison_state,
        "phase_mapping": "linear_64_step_cycle_observed_not_retuned",
        "pressure_source": "minime.health.resonance_density_v1.pressure_risk",
        "entropy_source": "minime.health.transition_event_v1.spectral_entropy",
        "shadow_dispersal_source": "minime.health.shadow_preservation_mode_v1.dispersal_potential",
        "interpretation": "rolling_context_comparison_only_causal_pressure_skip_link_and_texture_mismatch_not_inferred",
        "runtime_effect_applied": false,
        "authority": "read_only_heartbeat_pressure_texture_evidence_not_cadence_phase_intensity_rescue_or_dispatch_control"
    });
    status["phase_entropy_review_v1"] = phase_entropy_review;
    status["signal_texture_comparison_v1"] = signal_texture_review;
    status["signal_continuity_review_v1"] = json!({
        "schema": "semantic_heartbeat_signal_continuity_review_v1",
        "schema_version": 1,
        "window_signal_sample_count": signal_sample_count,
        "variation_observed_count": signal_variation_count,
        "near_repeat_observed_count": signal_near_repeat_count,
        "low_component_variance_observed_count": signal_low_variance_count,
        "mean_delta_rms_from_previous": signal_mean_delta_rms,
        "mean_generated_component_stddev": signal_mean_generated_component_stddev,
        "mean_delivered_component_stddev": signal_mean_delivered_component_stddev,
        "latest_continuity_classification": latest_signal_classification,
        "tail_start_dimension": SEMANTIC_HEARTBEAT_TAIL_START_DIM,
        "private_source_content_copied": false,
        "interpretation": "bounded_signal_aggregates_only_flatness_or_lived_quality_not_inferred",
        "runtime_effect_applied": false,
        "authority": "read_only_heartbeat_signal_evidence_not_vector_cadence_intensity_shaping_rescue_or_dispatch_control"
    });
    status["observability_authority"] =
        json!("read_only_heartbeat_continuity_evidence_not_cadence_intensity_or_dispatch_control");
    now
}

fn semantic_heartbeat_signal_evidence_json(
    signal: SemanticHeartbeatSignalEvidenceV1,
    delivered: Option<SemanticHeartbeatSignalMetricsV1>,
) -> Value {
    json!({
        "schema": "semantic_heartbeat_signal_evidence_v1",
        "schema_version": 1,
        "content_basis": signal.content_basis,
        "gesture_seed_applied": signal.gesture_seed_applied,
        "generated": semantic_heartbeat_signal_metrics_json(signal.generated),
        "consecutive_comparison": {
            "compared_dimension_count": signal.compared_dimension_count,
            "delta_rms_from_previous": signal.delta_rms_from_previous,
            "cosine_similarity_to_previous": signal.cosine_similarity_to_previous
        },
        "delivered": delivered.map(semantic_heartbeat_signal_metrics_json),
        "continuity_classification": signal.continuity_classification,
        "private_source_content_copied": false,
        "runtime_effect_applied": false
    })
}

fn semantic_heartbeat_signal_metrics_json(metrics: SemanticHeartbeatSignalMetricsV1) -> Value {
    json!({
        "feature_count": metrics.feature_count,
        "finite_feature_count": metrics.finite_feature_count,
        "active_dimension_count": metrics.active_dimension_count,
        "rms": metrics.rms,
        "component_stddev": metrics.component_stddev,
        "max_abs": metrics.max_abs,
        "tail_rms": metrics.tail_rms,
        "clipped_dimension_count": metrics.clipped_dimension_count
    })
}

fn semantic_heartbeat_texture_context_json(context: &SemanticHeartbeatTextureContextV1) -> Value {
    json!({
        "schema": "semantic_heartbeat_texture_context_v1",
        "schema_version": 1,
        "minime_observation_v1": {
            "origin": "minime",
            "source_id": format!("eigenpacket_t_ms:{}", context.telemetry_t_ms),
            "telemetry_t_ms": context.telemetry_t_ms,
            "spectral_entropy": context.spectral_entropy,
            "resonance_density": context.resonance_density,
            "pressure_risk": context.pressure_risk,
            "resonance_mode_packing": context.resonance_mode_packing,
            "pressure_source_mode_packing": context.pressure_source_mode_packing,
            "viscosity_index": context.viscosity_index,
            "viscosity_gradient": context.viscosity_gradient,
            "primary_texture": context.primary_texture.as_deref(),
            "movement_quality": context.movement_quality.as_deref(),
            "field_paths": [
                "spectral_fingerprint_v1.spectral_entropy",
                "resonance_density_v1.density",
                "resonance_density_v1.pressure_risk",
                "resonance_density_v1.components.mode_packing",
                "resonance_density_v1.components.viscosity_index",
                "resonance_density_v1.components.viscosity_vector.viscosity_gradient",
                "resonance_density_v1.texture_signature.primary_texture",
                "resonance_density_v1.texture_signature.movement_quality",
                "pressure_source_v1.components.mode_packing"
            ]
        },
        "bridge_derivation_v1": {
            "origin": "astrid_spectral_bridge",
            "parent_source_id": format!("eigenpacket_t_ms:{}", context.telemetry_t_ms),
            "eigenvalue_count": context.eigenvalue_count,
            "lambda1_abs_share": context.lambda1_abs_share,
            "lambda1_lambda2_abs_ratio": context.lambda1_lambda2_abs_ratio,
            "lambda_tail_abs_share": context.lambda_tail_abs_share,
            "derivation": "finite absolute eigenvalue magnitudes; lambda tail begins at index 2",
            "producer_truth_mutated": false
        },
        "private_source_content_copied": false,
        "runtime_effect_applied": false,
        "authority": "read_only_minime_observation_and_bridge_derivation_not_heartbeat_vector_cadence_intensity_rescue_dispatch_or_control"
    })
}

fn semantic_heartbeat_signal_texture_sample_comparison_v1(sample: &Value) -> Value {
    let signal = sample.get("signal_evidence_v1");
    let context = sample.get("texture_context_v1");
    let observed = context.and_then(|value| value.get("minime_observation_v1"));
    let derived = context.and_then(|value| value.get("bridge_derivation_v1"));
    let density = observed
        .and_then(|value| value.get("resonance_density"))
        .and_then(Value::as_f64);
    let entropy = observed
        .and_then(|value| value.get("spectral_entropy"))
        .and_then(Value::as_f64);
    let viscosity = observed
        .and_then(|value| value.get("viscosity_index"))
        .and_then(Value::as_f64);
    let primary_texture = observed
        .and_then(|value| value.get("primary_texture"))
        .and_then(Value::as_str);
    let dense_field = density.is_some_and(|value| value >= SEMANTIC_HEARTBEAT_DENSE_FIELD_AT);
    let high_entropy_field =
        entropy.is_some_and(|value| value >= SEMANTIC_HEARTBEAT_HIGH_ENTROPY_AT);
    let viscous_field = viscosity.is_some_and(|value| value >= SEMANTIC_HEARTBEAT_VISCOUS_FIELD_AT)
        || primary_texture.is_some_and(|value| value.to_ascii_lowercase().contains("viscous"));
    let delivered_max_abs = signal
        .and_then(|value| value.get("delivered"))
        .and_then(|value| value.get("max_abs"))
        .and_then(Value::as_f64);
    let bounded_delivery = delivered_max_abs
        .is_some_and(|value| value <= f64::from(SEMANTIC_HEARTBEAT_MAX_ABS) + f64::EPSILON);
    let comparison_state = if dense_field && high_entropy_field && viscous_field {
        "dense_viscous_high_entropy_field_with_bounded_heartbeat_signal_observed"
    } else {
        "signal_and_field_context_available_without_dense_viscous_high_entropy_convergence"
    };

    json!({
        "at_unix_s": sample.get("at_unix_s"),
        "source": sample.get("source"),
        "outcome": sample.get("outcome"),
        "phase_step": sample.get("phase_step"),
        "phase": sample.get("phase"),
        "configured_interval_secs": sample.get("configured_interval_secs"),
        "configured_intensity": sample.get("configured_intensity"),
        "signal_v1": {
            "content_basis": signal.and_then(|value| value.get("content_basis")),
            "generated_rms": signal
                .and_then(|value| value.pointer("/generated/rms")),
            "generated_component_stddev": signal
                .and_then(|value| value.pointer("/generated/component_stddev")),
            "generated_max_abs": signal
                .and_then(|value| value.pointer("/generated/max_abs")),
            "generated_tail_rms": signal
                .and_then(|value| value.pointer("/generated/tail_rms")),
            "delivered_rms": signal
                .and_then(|value| value.pointer("/delivered/rms")),
            "delivered_component_stddev": signal
                .and_then(|value| value.pointer("/delivered/component_stddev")),
            "delivered_max_abs": delivered_max_abs,
            "delivered_tail_rms": signal
                .and_then(|value| value.pointer("/delivered/tail_rms")),
            "bounded_by_existing_rescue_shape": bounded_delivery
        },
        "minime_observation_v1": observed,
        "bridge_derivation_v1": derived,
        "dense_field_threshold": SEMANTIC_HEARTBEAT_DENSE_FIELD_AT,
        "high_entropy_threshold": SEMANTIC_HEARTBEAT_HIGH_ENTROPY_AT,
        "viscous_field_threshold": SEMANTIC_HEARTBEAT_VISCOUS_FIELD_AT,
        "dense_field_observed": dense_field,
        "high_entropy_field_observed": high_entropy_field,
        "viscous_field_observed": viscous_field,
        "comparison_state": comparison_state,
        "spectral_code_mismatch_state": "not_established_cross_domain_texture_decoder_absent",
        "texture_marker_test_state": "raw_heartbeat_features_have_no_authoritative_viscosity_marker_decoder",
        "scalar_equivalence_inferred": false,
        "runtime_effect_applied": false
    })
}

fn semantic_heartbeat_signal_texture_review_v1(
    samples: &[Value],
    window_attempt_count: u64,
    window_block_count: u64,
    window_skip_rate: f64,
) -> Value {
    let has_signal_and_context = |sample: &&Value| {
        sample.get("signal_evidence_v1").is_some() && sample.get("texture_context_v1").is_some()
    };
    let context_sample_count = u64::try_from(
        samples
            .iter()
            .filter(|sample| sample.get("texture_context_v1").is_some())
            .count(),
    )
    .unwrap_or(u64::MAX);
    let latest_comparison = samples
        .iter()
        .rev()
        .find(has_signal_and_context)
        .map(semantic_heartbeat_signal_texture_sample_comparison_v1);
    let latest_phase_zero_comparison = samples
        .iter()
        .rev()
        .filter(|sample| {
            sample.get("source").and_then(Value::as_str) == Some("steady_semantic_heartbeat")
                && sample.get("phase_step").and_then(Value::as_u64) == Some(0)
        })
        .find(has_signal_and_context)
        .map(semantic_heartbeat_signal_texture_sample_comparison_v1);
    let persistence_check_state = if window_attempt_count == 0 {
        "no_heartbeat_attempts_observed"
    } else if window_block_count == 0 {
        "no_rescue_skips_observed_in_window"
    } else if window_block_count == window_attempt_count {
        "all_observed_heartbeats_blocked_by_existing_rescue_policy"
    } else {
        "intermittent_rescue_skips_observed_in_window"
    };
    let comparison_state = if latest_comparison.is_some() {
        "signal_and_field_comparison_available"
    } else if context_sample_count > 0 {
        "field_context_available_signal_evidence_missing"
    } else {
        "minime_field_context_unavailable"
    };
    let phase_zero_comparison_state = if latest_phase_zero_comparison.is_some() {
        "available"
    } else {
        "awaiting_steady_heartbeat_phase_zero_sample"
    };

    json!({
        "schema": "semantic_heartbeat_signal_texture_comparison_v1",
        "schema_version": 1,
        "window_texture_context_sample_count": context_sample_count,
        "comparison_state": comparison_state,
        "latest_comparison_v1": latest_comparison,
        "latest_phase_zero_comparison_v1": latest_phase_zero_comparison,
        "phase_zero_comparison_state": phase_zero_comparison_state,
        "persistence_check_v1": {
            "window_attempt_count": window_attempt_count,
            "window_block_count": window_block_count,
            "window_skip_rate": window_skip_rate,
            "state": persistence_check_state,
            "causal_pressure_skip_link_inferred": false
        },
        "interpretation": "pulse_shape_and_minime_field_texture_are_shown_side_by_side; lived_texture_match_and_causation_require_felt_review",
        "intensity_or_cadence_change_authority": "requires_separate_mike_operator_approval",
        "runtime_effect_applied": false,
        "authority": "read_only_signal_to_field_comparison_not_vector_cadence_intensity_shaping_rescue_dispatch_or_control"
    })
}

fn semantic_heartbeat_signal_class_count(samples: &[&Value], classification: &str) -> u64 {
    u64::try_from(
        samples
            .iter()
            .filter(|signal| {
                signal
                    .get("continuity_classification")
                    .and_then(Value::as_str)
                    == Some(classification)
            })
            .count(),
    )
    .unwrap_or(u64::MAX)
}

fn semantic_heartbeat_nested_mean(samples: &[&Value], path: &[&str]) -> Option<f64> {
    let values: Vec<f64> = samples
        .iter()
        .filter_map(|sample| value_at_path(sample, path).and_then(Value::as_f64))
        .filter(|value| value.is_finite())
        .collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn semantic_heartbeat_window_mean(samples: &[Value], outcome: &str, field: &str) -> Option<f64> {
    let values: Vec<f64> = samples
        .iter()
        .filter(|sample| sample.get("outcome").and_then(Value::as_str) == Some(outcome))
        .filter_map(|sample| sample.get(field).and_then(Value::as_f64))
        .filter(|value| value.is_finite())
        .collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn semantic_heartbeat_phase_entropy_review_v1(samples: &[Value], now: f64) -> Value {
    let pairs: Vec<(f64, f64)> = samples
        .iter()
        .filter(|sample| {
            sample
                .get("at_unix_s")
                .and_then(Value::as_f64)
                .is_some_and(|at| {
                    at.is_finite()
                        && now >= at
                        && now - at <= SEMANTIC_HEARTBEAT_PHASE_ENTROPY_WINDOW_SECS
                })
        })
        .filter_map(|sample| {
            let phase = sample.get("phase").and_then(Value::as_f64)?;
            let entropy = sample.get("spectral_entropy").and_then(Value::as_f64)?;
            (phase.is_finite() && entropy.is_finite()).then_some((phase, entropy))
        })
        .collect();
    let pair_count = u64::try_from(pairs.len()).unwrap_or(u64::MAX);
    let phase_wrap_observed = pairs.windows(2).any(|pair| pair[1].0 + 0.5 < pair[0].0);
    let phase_min = pairs.iter().map(|(phase, _)| *phase).reduce(f64::min);
    let phase_max = pairs.iter().map(|(phase, _)| *phase).reduce(f64::max);
    let entropy_min = pairs.iter().map(|(_, entropy)| *entropy).reduce(f64::min);
    let entropy_max = pairs.iter().map(|(_, entropy)| *entropy).reduce(f64::max);
    let correlation = if pairs.len() >= 3 && !phase_wrap_observed {
        let count = pairs.len() as f64;
        let phase_mean = pairs.iter().map(|(phase, _)| phase).sum::<f64>() / count;
        let entropy_mean = pairs.iter().map(|(_, entropy)| entropy).sum::<f64>() / count;
        let (covariance, phase_variance, entropy_variance) = pairs.iter().fold(
            (0.0_f64, 0.0_f64, 0.0_f64),
            |(covariance, phase_variance, entropy_variance), (phase, entropy)| {
                let phase_delta = phase - phase_mean;
                let entropy_delta = entropy - entropy_mean;
                (
                    phase_delta.mul_add(entropy_delta, covariance),
                    phase_delta.mul_add(phase_delta, phase_variance),
                    entropy_delta.mul_add(entropy_delta, entropy_variance),
                )
            },
        );
        let denominator = (phase_variance * entropy_variance).sqrt();
        (denominator > f64::EPSILON).then(|| (covariance / denominator).clamp(-1.0, 1.0))
    } else {
        None
    };
    let state = if pairs.is_empty() {
        "no_paired_phase_entropy_samples"
    } else if pairs.len() < 3 {
        "insufficient_paired_samples"
    } else if phase_wrap_observed {
        "phase_wrap_observed_correlation_withheld"
    } else if correlation.is_some() {
        "paired_correlation_available_causation_not_inferred"
    } else {
        "insufficient_phase_or_entropy_variation"
    };

    json!({
        "schema": "semantic_heartbeat_phase_entropy_review_v1",
        "schema_version": 1,
        "window_duration_secs": SEMANTIC_HEARTBEAT_PHASE_ENTROPY_WINDOW_SECS,
        "paired_sample_count": pair_count,
        "phase_min": phase_min,
        "phase_max": phase_max,
        "spectral_entropy_min": entropy_min,
        "spectral_entropy_max": entropy_max,
        "phase_wrap_observed": phase_wrap_observed,
        "phase_entropy_pearson_correlation": correlation,
        "state": state,
        "interpretation": "paired_60_second_observation_only_tick_restlessness_or_entropy_causation_not_inferred",
        "runtime_effect_applied": false,
        "authority": "read_only_phase_entropy_evidence_not_phase_cadence_intensity_rescue_or_dispatch_control"
    })
}

fn record_semantic_heartbeat_block(
    path: &Path,
    policy: &RescueBridgePolicy,
    reason: &str,
    observation: SemanticHeartbeatObservationV1,
    health: Option<&LimitedWriteHealth>,
) {
    let mut status = read_status(path);
    if !status.is_object() {
        status = json!({});
    }
    let now =
        record_semantic_heartbeat_observation(&mut status, observation, "blocked", health, None);
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["last_block_at_unix_s"] = json!(now);
    status["last_block_reason"] = json!(reason);
    write_status(path, &status);
}

fn record_semantic_heartbeat_sent(
    path: &Path,
    policy: &RescueBridgePolicy,
    health: Option<&LimitedWriteHealth>,
    observation: SemanticHeartbeatObservationV1,
    delivered_signal: SemanticHeartbeatSignalMetricsV1,
) {
    let mut status = read_status(path);
    if !status.is_object() {
        status = json!({});
    }
    let now = record_semantic_heartbeat_observation(
        &mut status,
        observation,
        "sent",
        health,
        Some(delivered_signal),
    );
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["last_sent_at_unix_s"] = json!(now);
    status["feature_scale"] = json!(SEMANTIC_HEARTBEAT_FEATURE_SCALE);
    status["max_abs"] = json!(SEMANTIC_HEARTBEAT_MAX_ABS);
    if let Some(health) = health {
        status["last_sent_fill_pct"] = json!(health.fill_pct);
        status["last_sent_stage"] = json!(health.stage);
        status["last_sent_health_age_secs"] = json!(health.age_secs);
    }
    write_status(path, &status);
}

fn record_semantic_heartbeat_enqueue_outcome(
    path: &Path,
    source: &str,
    configured_interval_secs: u64,
    queue_wait: Duration,
    outcome: &str,
) {
    let mut status = read_status(path);
    if !status.is_object() {
        status = json!({});
    }
    let now = now_unix_s();
    let prior_success_at = status
        .get("last_enqueue_success_at_unix_s")
        .and_then(Value::as_f64)
        .filter(|at| at.is_finite() && now >= *at);
    let inter_enqueue_gap_secs = if outcome == "enqueued" {
        prior_success_at.map(|at| now - at)
    } else {
        None
    };
    let queue_wait_ms = queue_wait.as_secs_f64() * 1_000.0;
    let enqueue_attempt_count = status
        .get("enqueue_attempt_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    let enqueue_success_count = status
        .get("enqueue_success_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(outcome == "enqueued"));
    let enqueue_closed_count = status
        .get("enqueue_closed_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(u64::from(outcome == "channel_closed"));

    let mut samples: Vec<Value> = status
        .get("enqueue_samples_v1")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|sample| {
            sample
                .get("completed_at_unix_s")
                .and_then(Value::as_f64)
                .is_some_and(|at| {
                    at.is_finite()
                        && now >= at
                        && now - at <= SEMANTIC_HEARTBEAT_OBSERVATION_WINDOW_SECS
                })
        })
        .cloned()
        .collect();
    samples.push(json!({
        "completed_at_unix_s": now,
        "source": source,
        "outcome": outcome,
        "queue_wait_ms": queue_wait_ms,
        "inter_enqueue_gap_secs": inter_enqueue_gap_secs,
        "configured_interval_secs": configured_interval_secs,
    }));
    let samples_truncated = samples.len() > SEMANTIC_HEARTBEAT_OBSERVATION_MAX_SAMPLES;
    if samples_truncated {
        let keep_from = samples
            .len()
            .saturating_sub(SEMANTIC_HEARTBEAT_OBSERVATION_MAX_SAMPLES);
        samples.drain(..keep_from);
    }
    let queue_waits: Vec<f64> = samples
        .iter()
        .filter_map(|sample| sample.get("queue_wait_ms").and_then(Value::as_f64))
        .filter(|value| value.is_finite())
        .collect();
    let inter_enqueue_gaps: Vec<f64> = samples
        .iter()
        .filter_map(|sample| sample.get("inter_enqueue_gap_secs").and_then(Value::as_f64))
        .filter(|value| value.is_finite())
        .collect();
    let mean_queue_wait_ms = (!queue_waits.is_empty())
        .then(|| queue_waits.iter().sum::<f64>() / queue_waits.len() as f64);
    let max_queue_wait_ms = queue_waits.iter().copied().reduce(f64::max);
    let mean_inter_enqueue_gap_secs = (!inter_enqueue_gaps.is_empty())
        .then(|| inter_enqueue_gaps.iter().sum::<f64>() / inter_enqueue_gaps.len() as f64);
    let max_inter_enqueue_gap_secs = inter_enqueue_gaps.iter().copied().reduce(f64::max);
    let delayed_gap_count = u64::try_from(
        samples
            .iter()
            .filter(|sample| {
                let Some(gap) = sample.get("inter_enqueue_gap_secs").and_then(Value::as_f64) else {
                    return false;
                };
                let interval = sample
                    .get("configured_interval_secs")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as f64;
                interval > 0.0 && gap > interval * 2.0
            })
            .count(),
    )
    .unwrap_or(u64::MAX);

    status["enqueue_attempt_count"] = json!(enqueue_attempt_count);
    status["enqueue_success_count"] = json!(enqueue_success_count);
    status["enqueue_closed_count"] = json!(enqueue_closed_count);
    status["last_enqueue_outcome"] = json!(outcome);
    status["last_enqueue_source"] = json!(source);
    status["last_enqueue_completed_at_unix_s"] = json!(now);
    status["last_enqueue_wait_ms"] = json!(queue_wait_ms);
    status["last_inter_enqueue_gap_secs"] = json!(inter_enqueue_gap_secs);
    if outcome == "enqueued" {
        status["last_enqueue_success_at_unix_s"] = json!(now);
    }
    status["enqueue_samples_v1"] = json!(samples);
    status["enqueue_samples_truncated"] = json!(samples_truncated);
    status["delivery_health_v1"] = json!({
        "schema": "semantic_heartbeat_delivery_health_v1",
        "schema_version": 1,
        "window_duration_secs": SEMANTIC_HEARTBEAT_OBSERVATION_WINDOW_SECS,
        "sample_count": queue_waits.len(),
        "mean_queue_wait_ms": mean_queue_wait_ms,
        "max_queue_wait_ms": max_queue_wait_ms,
        "mean_inter_enqueue_gap_secs": mean_inter_enqueue_gap_secs,
        "max_inter_enqueue_gap_secs": max_inter_enqueue_gap_secs,
        "gap_over_twice_configured_interval_count": delayed_gap_count,
        "latest_outcome": outcome,
        "send_count_compatibility_semantics": "rescue_policy_admission_before_channel_enqueue",
        "interpretation": "bounded_channel_enqueue_timing_only; downstream_processing_or_minime_arrival_not_inferred",
        "runtime_effect_applied": false,
        "authority": "read_only_enqueue_evidence_not_cadence_intensity_rescue_dispatch_or_control"
    });
    write_status(path, &status);
}

fn record_limited_write_sent(
    path: &Path,
    policy: &RescueBridgePolicy,
    context: &SemanticWriteContext<'_>,
    health: Option<&LimitedWriteHealth>,
    cooldown_secs: u64,
) {
    let mut status = read_status(path);
    if !status.is_object() || !status_matches_policy(&status, policy) {
        status = json!({});
    }
    let send_count = status
        .get("send_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    let text_preview = context
        .text
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect::<String>();
    let now = now_unix_s();
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["send_count"] = json!(send_count);
    status["last_sent_at_unix_s"] = json!(now);
    status["last_sent_source"] = json!(context.source);
    status["last_sent_mode"] = json!(context.mode.unwrap_or_default());
    status["last_sent_fill_pct"] =
        json!(health.map_or(context.fill_pct, |health| Some(health.fill_pct)));
    status["last_sent_previous_fill_pct"] = json!(context.previous_fill_pct);
    if let Some(health) = health {
        status["last_sent_stage"] = json!(health.stage);
        status["last_sent_peak_fill_pct_60s"] = json!(health.peak_fill_pct_60s);
        status["last_sent_semantic_energy"] = json!(health.semantic_energy);
        status["last_sent_health_age_secs"] = json!(health.age_secs);
    }
    status["last_sent_text_preview"] = json!(text_preview);
    status["last_sent_watch_terms"] = json!(matched_watch_terms(
        context.text.unwrap_or_default(),
        policy
    ));
    status["cooldown_secs"] = json!(cooldown_secs);
    status["cooldown_until_unix_s"] = json!(now + cooldown_secs as f64);
    if let Some(mute_until) = write_limited_write_sensory_mute(
        path,
        policy,
        now,
        policy.limited_write_mute_live_intake_secs,
        "limited_write_semantic_send",
    ) {
        status["live_intake_mute_secs"] = json!(policy.limited_write_mute_live_intake_secs);
        status["live_intake_mute_until_unix_s"] = json!(mute_until);
        status["live_intake_mute_file"] = json!(LIMITED_WRITE_SENSORY_MUTE_FILE);
    }
    if policy.limited_write_v2_active() {
        status["last_send_evaluation"] = json!({
            "state": "pending",
            "sent_at_unix_s": now
        });
    }
    status["last_block_reason"] = Value::Null;
    write_status(path, &status);
}

pub fn bridge_autonomous_enabled_for_path(path: &Path) -> bool {
    load_policy(path)
        .map(|policy| policy.autonomous_enabled())
        .unwrap_or(true)
}

pub fn bridge_autonomous_enabled() -> bool {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    bridge_autonomous_enabled_for_path(&path)
}

pub fn bridge_sensory_enabled_for_path(path: &Path) -> bool {
    load_policy(path)
        .map(|policy| policy.sensory_connection_enabled())
        .unwrap_or(true)
}

pub fn bridge_sensory_enabled() -> bool {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    bridge_sensory_enabled_for_path(&path)
}

pub(crate) fn semantic_write_block_reason_for_path(
    msg: &SensoryMsg,
    path: &Path,
) -> Option<String> {
    if !matches!(msg, SensoryMsg::Semantic { .. }) {
        return None;
    }
    load_policy(path)?.semantic_ingress_block_reason()
}

pub(crate) fn semantic_write_block_reason(msg: &SensoryMsg) -> Option<String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    semantic_write_block_reason_for_path(msg, &path)
}

pub(crate) fn prepare_semantic_write_for_path(
    msg: &mut SensoryMsg,
    path: &Path,
    context: &SemanticWriteContext<'_>,
) -> Result<(), String> {
    if !matches!(msg, SensoryMsg::Semantic { .. }) {
        return Ok(());
    }
    let Some(policy) = load_policy(path) else {
        return Ok(());
    };
    let status_path = limited_write_status_path_for_profile(path);
    if let Some(reason) = policy.limited_write_block_reason(context, path, &status_path) {
        record_limited_write_block(&status_path, &policy, &reason);
        return Err(reason);
    }
    if policy.limited_write_active() {
        let health = if policy.limited_write_v2_active() {
            Some(load_limited_write_health(
                path,
                policy.limited_write_health_max_age_secs,
            )?)
        } else {
            None
        };
        if let SensoryMsg::Semantic { features, .. } = msg {
            policy.apply_limited_write_shape(features);
        }
        record_limited_write_sent(
            &status_path,
            &policy,
            context,
            health.as_ref(),
            policy.limited_write_cooldown_secs,
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn prepare_semantic_heartbeat_for_path_with_observation(
    msg: &mut SensoryMsg,
    path: &Path,
    observation: SemanticHeartbeatObservationV1,
) -> Result<(), String> {
    prepare_semantic_heartbeat_for_path_with_enqueue_probe(msg, path, observation).map(drop)
}

fn prepare_semantic_heartbeat_for_path_with_enqueue_probe(
    msg: &mut SensoryMsg,
    path: &Path,
    observation: SemanticHeartbeatObservationV1,
) -> Result<SemanticHeartbeatEnqueueProbeV1, String> {
    let status_path = semantic_heartbeat_status_path_for_profile(path);
    let enqueue_probe = SemanticHeartbeatEnqueueProbeV1::new(
        status_path.clone(),
        observation.source,
        observation.interval_secs,
    );
    if !matches!(msg, SensoryMsg::Semantic { .. }) {
        return Ok(enqueue_probe);
    }
    let Some(policy) = load_policy(path) else {
        return Ok(enqueue_probe);
    };
    let health = if policy.limited_write_v2_active() {
        load_limited_write_health(path, policy.limited_write_health_max_age_secs).ok()
    } else {
        None
    };
    if let Some(reason) = policy.heartbeat_block_reason(path) {
        record_semantic_heartbeat_block(
            &status_path,
            &policy,
            &reason,
            observation,
            health.as_ref(),
        );
        return Err(reason);
    }
    let delivered_signal = if let SensoryMsg::Semantic { features, .. } = msg {
        RescueBridgePolicy::apply_semantic_heartbeat_shape(features);
        semantic_heartbeat_signal_metrics(features)
    } else {
        unreachable!("semantic heartbeat was checked as a semantic message")
    };
    record_semantic_heartbeat_sent(
        &status_path,
        &policy,
        health.as_ref(),
        observation,
        delivered_signal,
    );
    Ok(enqueue_probe)
}

pub(crate) fn prepare_semantic_write(
    msg: &mut SensoryMsg,
    context: &SemanticWriteContext<'_>,
) -> Result<(), String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    prepare_semantic_write_for_path(msg, &path, context)
}

pub(crate) fn prepare_semantic_heartbeat_with_enqueue_probe(
    msg: &mut SensoryMsg,
    observation: SemanticHeartbeatObservationV1,
) -> Result<SemanticHeartbeatEnqueueProbeV1, String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    prepare_semantic_heartbeat_for_path_with_enqueue_probe(msg, &path, observation)
}

#[cfg(test)]
#[path = "rescue_policy_tests.rs"]
mod rescue_policy_tests;
