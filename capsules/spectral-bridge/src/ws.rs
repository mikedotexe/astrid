//! `WebSocket` clients for minime connectivity.
//!
//! Two persistent connections:
//! - **Telemetry** (port 7878): Subscribes to spectral eigenvalue broadcasts.
//! - **Sensory** (port 7879): Sends control/semantic features to minime.
//!
//! Both connections auto-reconnect with exponential backoff on failure.
#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use astrid_minime_protocol::{
    CompatibilityStatus, EigenPacketV1, SensoryMsg as WireSensoryMsg, SensoryPacketV1,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tracing::{debug, debug_span, error, info, info_span, warn};

use crate::db::BridgeDb;
use crate::lambda_edge::{self, LambdaEdgePerceptionV1};
use crate::lambda_tail::{self, ArtifactScanSummary, LambdaTailTelemetryV1};
use crate::paths::bridge_paths;
use crate::sticky_mode::{self, StickyModeAuditV1};
use crate::trace_lab;
use crate::types::{
    BridgeEntropyReciprocityReviewV1, BridgeReciprocityV1, BridgeTextureEvidenceV1,
    ConnectivityStatus, DeltaPersistenceV1, ExperienceDeltaBusV1, ExperienceDeltaKindV1,
    ExperienceDeltaV1, LambdaContribution, LambdaProfile, MessageDirection,
    PersistentDeformationSmoothingReviewV1, PressureSourceAnalysisV1, PressureTrendSmoothingV1,
    PressureTrendV1, PullModeRate, PullTopologyProfile, ResidualDeformationTraceV1,
    ResonanceDensityComponents, SafetyDecisionTrace, SafetyLevel, SensoryMsg, SpectralTelemetry,
    TelemetryHeartbeatDeltaV1, TelemetryProtocolStatusV1, TextureDynamicFluxVectorV1,
    TextureShapeOverTimeV2, TextureSignatureIntegrityV1, ViscosityPorosityTransportReviewV1,
    WebSocketLaneTrace, resonance_cohesion_score_v1, resonance_stability_context_v1,
    resonance_structural_integrity_index_v1,
    viscosity_porosity_transport_review_with_fingerprint_v1,
};

const PRESSURE_TREND_SMOOTHING_BASE_WINDOW: usize = 5;
const PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW: usize = 20;
const PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT: f32 = 0.70;
const PRESSURE_TREND_SMOOTHING_FULL_ENTROPY_AT: f32 = 0.95;
const BRIDGE_RECIPROCITY_RECENT_WINDOW_MS: f64 = 10_000.0;
const BRIDGE_RECIPROCITY_STALE_WINDOW_MS: f64 = 60_000.0;
const BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS: f64 = 90_000.0;
const BRIDGE_RECIPROCITY_PRESSURE_POROSITY_STALE_WINDOW_MS: f64 = 120_000.0;
const BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS: f64 = 120_000.0;
const BRIDGE_RECIPROCITY_ENTROPY_CONTRACT_PREVIEW_WINDOW_MS: f64 = 45_000.0;
const PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT: f32 = 0.40;
const PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT: f32 = 0.30;
const PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT: f32 = 0.28;
const PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT: f32 = 0.25;
const PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT: f32 = 0.35;

const fn protocol_compatibility_label(status: CompatibilityStatus) -> &'static str {
    match status {
        CompatibilityStatus::Current => "current",
        CompatibilityStatus::CompatibleMinor => "compatible_minor",
        CompatibilityStatus::LegacyUnversioned => "legacy_unversioned",
        CompatibilityStatus::UnsupportedName => "unsupported_name",
        CompatibilityStatus::UnsupportedMajor => "unsupported_major",
    }
}

fn record_telemetry_protocol_status(
    state: &mut BridgeState,
    packet: &EigenPacketV1,
    compatibility: CompatibilityStatus,
    observed_at_unix_s: f64,
    accepted: bool,
) {
    let protocol = packet.protocol.as_ref();
    state.telemetry_protocol_v1.protocol_name = protocol.map(|header| header.name.clone());
    state.telemetry_protocol_v1.protocol_major = protocol.map(|header| header.major);
    state.telemetry_protocol_v1.protocol_minor = protocol.map(|header| header.minor);
    state.telemetry_protocol_v1.compatibility =
        protocol_compatibility_label(compatibility).to_string();
    state.telemetry_protocol_v1.accepted = accepted;
    state.telemetry_protocol_v1.last_observed_unix_s = Some(observed_at_unix_s);
    if accepted {
        state.telemetry_protocol_v1.last_valid_t_ms = Some(packet.t_ms);
    } else {
        state.telemetry_protocol_v1.mismatch_count =
            state.telemetry_protocol_v1.mismatch_count.saturating_add(1);
        state.telemetry_protocol_v1.last_mismatch_unix_s = Some(observed_at_unix_s);
    }
}

fn encode_sensory_packet(message: &SensoryMsg) -> Result<String, serde_json::Error> {
    let domain_value = serde_json::to_value(message)?;
    let wire_message: WireSensoryMsg = serde_json::from_value(domain_value)?;
    serde_json::to_string(&SensoryPacketV1::versioned(wire_message))
}

#[derive(Debug, Clone)]
struct PressureTrendSampleV1 {
    pressure_risk: Option<f32>,
    pressure_velocity_delta: Option<f32>,
    spectral_drift_velocity: Option<f32>,
    mode_packing: Option<f32>,
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
    semantic_viscosity: Option<f32>,
    viscosity_gradient: Option<f32>,
    viscosity_gradient_trend: Option<f32>,
    complexity_density: Option<f32>,
    weight_density_index: Option<f32>,
    comfort_gate: Option<f32>,
    porosity_gradient: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    semantic_coherence_delta: Option<f32>,
    fill_pct: f32,
    spectral_entropy: Option<f32>,
    window_capacity: usize,
    observed_at_unix_s: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct SiltNoiseSeparationV1 {
    policy: &'static str,
    high_entropy: f32,
    low_entropy: f32,
    high_entropy_mode_packing: f32,
    low_entropy_mode_packing: f32,
    mode_packing_delta: f32,
    semantic_trickle: Option<f32>,
    dynamic_high_mode_threshold: f32,
    contextual_resonance_score: f32,
    contextual_resonance_basis: &'static str,
    heritage_preservation_state: &'static str,
    interpretation: &'static str,
    silt_signal_state: &'static str,
    porosity_change_authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PressurePorosityExpansionReadinessV1 {
    pub policy: &'static str,
    pub pressure_risk: Option<f32>,
    pub mode_packing: Option<f32>,
    pub porosity_gradient: Option<f32>,
    pub semantic_trickle: Option<f32>,
    pub readiness_state: &'static str,
    pub live_mode_packing_threshold: f32,
    pub liminal_mode_packing_threshold: f32,
    pub viscous_density_warning_threshold: f32,
    pub viscous_density_warning_state: &'static str,
    pub felt_dead_zone_mode_packing_threshold: f32,
    pub threshold_gap: Option<f32>,
    pub liminal_threshold_gap: Option<f32>,
    pub viscous_warning_margin: Option<f32>,
    pub porosity_buffer_candidate: Option<f32>,
    pub viscosity_feedback_readiness: &'static str,
    pub proposed_intervention: &'static str,
    pub approval_boundary: &'static str,
    pub local_control_applied: bool,
    pub authority: &'static str,
}

/// Shared mutable bridge state updated by `WebSocket` tasks.
pub struct BridgeState {
    /// Latest telemetry from minime.
    pub latest_telemetry: Option<SpectralTelemetry>,
    /// Derived fill percentage.
    pub fill_pct: f32,
    /// Previous derived fill percentage from the last telemetry packet.
    pub previous_fill_pct: Option<f32>,
    /// Derived pressure velocity / stability readout from consecutive telemetry packets.
    pub pressure_trend_v1: Option<PressureTrendV1>,
    /// Arrival-cadence truth behind the latest pressure/fill trend.
    pub telemetry_heartbeat_delta_v1: Option<TelemetryHeartbeatDeltaV1>,
    /// Bounded recent pressure/fill samples for smoothing diagnostics.
    pressure_trend_samples_v1: VecDeque<PressureTrendSampleV1>,
    /// Unix arrival time of the previous telemetry packet.
    pub previous_telemetry_arrival_unix_s: Option<f64>,
    /// Unix arrival time of the latest telemetry packet.
    pub latest_telemetry_arrival_unix_s: Option<f64>,
    /// Current safety level.
    pub safety_level: SafetyLevel,
    /// Previous safety level (for transition detection).
    pub prev_safety_level: SafetyLevel,
    /// Whether the telemetry `WebSocket` is connected.
    pub telemetry_connected: bool,
    /// Whether the sensory `WebSocket` is connected.
    pub sensory_connected: bool,
    /// Last confirmed outbound sensory send time, distinct from generic lane activity.
    pub last_sensory_sent_unix_s: Option<f64>,
    /// Total messages relayed (both directions).
    pub messages_relayed: u64,
    /// Bridge start time.
    pub start_time: std::time::Instant,
    /// Active incident ID (if in yellow/orange/red).
    pub active_incident_id: Option<i64>,
    /// Latest spectral fingerprint from minime (32D geometry summary).
    pub spectral_fingerprint: Option<Vec<f32>>,
    /// Latest compact raw-eigenvector field from Minime.
    pub eigenvector_field: Option<serde_json::Value>,
    /// Latest bridge-side eigenvalue contribution profile.
    pub lambda_profile: Option<LambdaProfile>,
    /// Latest Pull-Oriented Map over lambda topology.
    pub pull_topology: Option<PullTopologyProfile>,
    /// Latest lambda-tail state classifier output.
    pub lambda_tail: Option<LambdaTailTelemetryV1>,
    /// Latest read-only lambda-edge perception output.
    pub lambda_edge_perception: Option<LambdaEdgePerceptionV1>,
    /// Latest read-only sticky-mode audit output.
    pub sticky_mode_audit: Option<StickyModeAuditV1>,
    /// Latest artifact-grounding scan used by the lambda-tail classifier.
    pub artifact_scan: Option<ArtifactScanSummary>,
    /// Unix timestamp for the latest artifact scan.
    pub artifact_scan_at_unix_s: Option<f64>,
    /// Latest safety decision explanation.
    pub safety_decision: Option<SafetyDecisionTrace>,

    // -- Metrics --
    /// Messages received from minime (telemetry direction).
    pub telemetry_received: u64,
    /// Messages sent to minime (sensory direction).
    pub sensory_sent: u64,
    /// Messages dropped by safety protocol.
    pub messages_dropped_safety: u64,
    /// Number of telemetry reconnections.
    pub telemetry_reconnects: u64,
    /// Number of sensory reconnections.
    pub sensory_reconnects: u64,
    /// Telemetry `WebSocket` lifecycle metrics.
    pub telemetry_ws: WebSocketLaneTrace,
    /// Sensory `WebSocket` lifecycle metrics.
    pub sensory_ws: WebSocketLaneTrace,
    /// Total safety incidents logged.
    pub incidents_total: u64,
    /// Latest canonical telemetry protocol compatibility observation.
    pub telemetry_protocol_v1: TelemetryProtocolStatusV1,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self::new()
    }
}

impl BridgeState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            latest_telemetry: None,
            fill_pct: 0.0,
            previous_fill_pct: None,
            pressure_trend_v1: None,
            telemetry_heartbeat_delta_v1: None,
            pressure_trend_samples_v1: VecDeque::new(),
            previous_telemetry_arrival_unix_s: None,
            latest_telemetry_arrival_unix_s: None,
            safety_level: SafetyLevel::Green,
            prev_safety_level: SafetyLevel::Green,
            telemetry_connected: false,
            sensory_connected: false,
            last_sensory_sent_unix_s: None,
            messages_relayed: 0,
            start_time: std::time::Instant::now(),
            active_incident_id: None,
            spectral_fingerprint: None,
            eigenvector_field: None,
            lambda_profile: None,
            pull_topology: None,
            lambda_tail: None,
            lambda_edge_perception: None,
            sticky_mode_audit: None,
            artifact_scan: None,
            artifact_scan_at_unix_s: None,
            safety_decision: None,
            telemetry_received: 0,
            sensory_sent: 0,
            messages_dropped_safety: 0,
            telemetry_reconnects: 0,
            sensory_reconnects: 0,
            telemetry_ws: WebSocketLaneTrace::default(),
            sensory_ws: WebSocketLaneTrace::default(),
            incidents_total: 0,
            telemetry_protocol_v1: TelemetryProtocolStatusV1::default(),
        }
    }

    /// Derived bidirectional connectivity health across the telemetry and
    /// sensory lanes (collapses the two independent booleans into one
    /// perceivable state).
    #[must_use]
    pub const fn connectivity_status(&self) -> ConnectivityStatus {
        ConnectivityStatus::from_lanes(self.telemetry_connected, self.sensory_connected)
    }

    /// Current reciprocity readout across inbound telemetry and outbound sensory lanes.
    #[must_use]
    pub fn bridge_reciprocity_v1(&self) -> BridgeReciprocityV1 {
        let now = unix_now_s();
        let (telemetry_age_ms, telemetry_future_skew_ms) = self
            .latest_telemetry_arrival_unix_s
            .map_or((None, None), |at| bridge_age_and_future_skew_ms(now, at));
        let (sensory_send_age_ms, sensory_future_skew_ms) = self
            .last_sensory_sent_unix_s
            .map_or((None, None), |at| bridge_age_and_future_skew_ms(now, at));
        let clock_skew_state =
            bridge_clock_skew_state(telemetry_future_skew_ms, sensory_future_skew_ms);
        let connectivity = self.connectivity_status();
        let (stale_window_ms, stale_window_basis) =
            bridge_dynamic_stale_window_ms(self.latest_telemetry.as_ref());
        let telemetry_recent = bridge_age_is_recent(telemetry_age_ms);
        let sensory_recent = bridge_age_is_recent(sensory_send_age_ms);
        let telemetry_stale = bridge_age_is_stale(telemetry_age_ms, stale_window_ms);
        let sensory_stale = bridge_age_is_stale(sensory_send_age_ms, stale_window_ms);
        let one_sided_state = match connectivity {
            ConnectivityStatus::Bidirectional if telemetry_recent && sensory_recent => {
                "bidirectional_recent"
            },
            ConnectivityStatus::Bidirectional
                if telemetry_recent && sensory_send_age_ms.is_none() =>
            {
                "bidirectional_no_recent_sensory"
            },
            ConnectivityStatus::Bidirectional if sensory_recent && telemetry_age_ms.is_none() => {
                "bidirectional_no_recent_telemetry"
            },
            ConnectivityStatus::Bidirectional if telemetry_stale && sensory_stale => {
                "bidirectional_stale_messages"
            },
            ConnectivityStatus::Bidirectional if telemetry_stale => "bidirectional_stale_telemetry",
            ConnectivityStatus::Bidirectional if sensory_stale => "bidirectional_stale_sensory",
            ConnectivityStatus::Bidirectional
                if telemetry_age_ms.is_some() || sensory_send_age_ms.is_some() =>
            {
                "bidirectional_waiting_messages"
            },
            ConnectivityStatus::Bidirectional => "bidirectional_connected_no_recent_messages",
            ConnectivityStatus::TelemetryOnly => "telemetry_only",
            ConnectivityStatus::SensoryOnly => "sensory_only",
            ConnectivityStatus::Severed => "severed",
        };
        BridgeReciprocityV1 {
            policy: "bridge_reciprocity_v1".to_string(),
            schema_version: 1,
            connectivity,
            latest_telemetry_arrival_unix_s: self.latest_telemetry_arrival_unix_s,
            last_sensory_sent_unix_s: self.last_sensory_sent_unix_s,
            telemetry_age_ms,
            sensory_send_age_ms,
            telemetry_future_skew_ms,
            sensory_future_skew_ms,
            clock_skew_state: clock_skew_state.to_string(),
            telemetry_messages_sent_total: self.telemetry_ws.messages_sent,
            sensory_messages_sent_total: self.sensory_ws.messages_sent,
            telemetry_messages_received_total: self.telemetry_ws.messages_received,
            sensory_messages_received_total: self.sensory_ws.messages_received,
            recent_window_ms: BRIDGE_RECIPROCITY_RECENT_WINDOW_MS,
            stale_window_ms,
            stale_window_basis: Some(stale_window_basis.to_string()),
            reflective_silence_extension_ms: (stale_window_ms > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
                .then_some(stale_window_ms - BRIDGE_RECIPROCITY_STALE_WINDOW_MS),
            threshold_policy: "bridge_reciprocity_dynamic_reflective_silence_v2".to_string(),
            one_sided_state: one_sided_state.to_string(),
            authority: "diagnostic_status_context_not_control".to_string(),
        }
    }

    /// Read-only review of Astrid's reciprocity-aging concern.
    ///
    /// This does not change `bridge_reciprocity_v1` or any stale-window
    /// threshold. It keeps transport patience separate from the structural
    /// identity clock: viscosity can justify more time for a reply while high
    /// entropy, low cohesion, or distinguishability loss can still erode
    /// confidence in what is being held.
    #[must_use]
    pub fn bridge_entropy_reciprocity_review_v1(&self) -> Option<BridgeEntropyReciprocityReviewV1> {
        let telemetry = self.latest_telemetry.as_ref()?;
        let now = unix_now_s();
        let (telemetry_age_ms, _) = self
            .latest_telemetry_arrival_unix_s
            .map_or((None, None), |at| bridge_age_and_future_skew_ms(now, at));
        let spectral_entropy = telemetry
            .typed_fingerprint()
            .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));
        let resonance_cohesion_score = telemetry
            .resonance_density_v1
            .as_ref()
            .map(|resonance| resonance_cohesion_score_v1(&resonance.components).clamp(0.0, 1.0));
        let distinguishability_loss = telemetry
            .distinguishability_loss
            .or_else(|| {
                telemetry
                    .denominator_metrics()
                    .map(|metrics| metrics.distinguishability_loss)
            })
            .map(|value| value.clamp(0.0, 1.0));
        let (current_stale_window_ms, current_stale_window_basis) =
            bridge_dynamic_stale_window_ms(Some(telemetry));
        let entropy_contract_preview_window_ms =
            bridge_entropy_contract_preview_window_ms(spectral_entropy, resonance_cohesion_score);
        let structural_identity_window_ms =
            Some(entropy_contract_preview_window_ms.unwrap_or(BRIDGE_RECIPROCITY_STALE_WINDOW_MS));
        let structural_age_multiplier = distinguishability_loss.map(|loss| 1.0 + f64::from(loss));
        let structural_effective_age_ms = telemetry_age_ms
            .zip(structural_age_multiplier)
            .map(|(age_ms, multiplier)| age_ms * multiplier);
        let transport_wait_stale = bridge_age_is_stale(telemetry_age_ms, current_stale_window_ms);
        let structural_identity_stale = structural_effective_age_ms
            .zip(structural_identity_window_ms)
            .is_some_and(|(age_ms, window_ms)| age_ms > window_ms);
        let would_stale_under_preview = structural_identity_stale && !transport_wait_stale;
        let clock_relation = match (transport_wait_stale, structural_identity_stale) {
            (false, false) => "transport_and_structural_clocks_holding",
            (false, true) => "transport_waiting_structural_identity_stale",
            (true, false) => "transport_stale_structural_identity_holding",
            (true, true) => "transport_and_structural_clocks_stale",
        };

        let current_window_state = if would_stale_under_preview {
            "current_window_still_waiting_preview_would_stale"
        } else if spectral_entropy.is_some_and(|value| value >= 0.85)
            && resonance_cohesion_score.is_some_and(|value| value <= 0.55)
        {
            "entropy_cohesion_decay_watch"
        } else if spectral_entropy.is_some_and(|value| value >= 0.85) {
            "high_entropy_hold_monitor"
        } else {
            "no_entropy_reciprocity_concern"
        };
        let recommendation = if would_stale_under_preview {
            "prepare_replay_or_observation_before_any_live_stale_window_change"
        } else if current_window_state == "entropy_cohesion_decay_watch" {
            "watch_resonance_cohesion_before_90000ms_without_changing_live_window"
        } else {
            "keep_current_reciprocity_window"
        };

        Some(BridgeEntropyReciprocityReviewV1 {
            policy: "bridge_entropy_reciprocity_review_v1".to_string(),
            schema_version: 1,
            spectral_entropy,
            resonance_cohesion_score,
            distinguishability_loss,
            telemetry_age_ms,
            current_stale_window_ms,
            current_stale_window_basis: Some(current_stale_window_basis.to_string()),
            entropy_contract_preview_window_ms,
            structural_identity_window_ms,
            structural_age_multiplier,
            structural_effective_age_ms,
            transport_wait_stale,
            structural_identity_stale,
            would_stale_under_preview,
            clock_relation: clock_relation.to_string(),
            current_window_state: current_window_state.to_string(),
            recommendation: recommendation.to_string(),
            live_stale_window_write: false,
            local_control_write: false,
            authority: "read_only_reciprocity_review_not_stale_window_or_controller_change"
                .to_string(),
        })
    }

    /// Read-only smoothing companion for the latest pressure trend.
    #[must_use]
    pub fn pressure_trend_smoothing_v1(&self) -> Option<PressureTrendSmoothingV1> {
        build_pressure_trend_smoothing_v1(&self.pressure_trend_samples_v1)
    }

    /// Read-only review for pressure that has become a stable bruise/baseline.
    #[must_use]
    pub fn pressure_persistent_deformation_review_v1(
        &self,
    ) -> Option<PersistentDeformationSmoothingReviewV1> {
        let telemetry = self.latest_telemetry.as_ref()?;
        let resonance = telemetry.resonance_density_v1.as_ref();
        let smoothing = self.pressure_trend_smoothing_v1();
        let pressure_risk = resonance.map(|value| value.pressure_risk.clamp(0.0, 1.0));
        let mode_packing = resonance.map(|value| value.components.mode_packing.clamp(0.0, 1.0));
        let porosity_gradient = resonance
            .and_then(|value| value.components.porosity_gradient)
            .map(|value| value.clamp(0.0, 1.0));
        let density_gradient_proxy = mode_packing
            .zip(porosity_gradient)
            .map(|(mode, porosity)| mode.mul_add(0.60, (1.0 - porosity) * 0.40).clamp(0.0, 1.0))
            .or(mode_packing)
            .or_else(|| porosity_gradient.map(|porosity| (1.0 - porosity).clamp(0.0, 1.0)));
        let fluctuation_score = telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map(|value| value.fluctuation_score.clamp(0.0, 1.0));
        let pressure_range = smoothing.as_ref().and_then(|value| value.pressure_range);
        let semantic_viscosity_persistence_index = smoothing
            .as_ref()
            .and_then(|value| value.semantic_viscosity_persistence_index);
        let stable_baseline = pressure_range.is_some_and(|range| range <= 0.04)
            || smoothing
                .as_ref()
                .is_some_and(|value| value.classification == "low_amplitude_stable");
        let low_fluctuation = fluctuation_score.is_some_and(|value| value <= 0.25);
        let persistent_baseline_score = (pressure_risk.unwrap_or(0.0) * 0.30
            + mode_packing.unwrap_or(0.0) * 0.22
            + density_gradient_proxy.unwrap_or(0.0) * 0.18
            + (1.0 - fluctuation_score.unwrap_or(0.50)).clamp(0.0, 1.0) * 0.16
            + semantic_viscosity_persistence_index.unwrap_or(0.0) * 0.14)
            .clamp(0.0, 1.0);
        let deformation_state =
            if stable_baseline && low_fluctuation && persistent_baseline_score >= 0.30 {
                "persistent_deformation_stable_baseline"
            } else if persistent_baseline_score >= 0.38 {
                "persistent_deformation_watch"
            } else if stable_baseline {
                "stable_low_amplitude_pressure_without_bruise"
            } else {
                "pressure_baseline_mixed_or_moving"
            };
        let recommendation = if deformation_state == "persistent_deformation_stable_baseline" {
            "carry_baseline_as_bruise_observation_before_any_pressure_threshold_or_smoothing_change"
        } else if deformation_state == "persistent_deformation_watch" {
            "compare_pressure_range_fluctuation_and_density_gradient_before_live_retune"
        } else {
            "keep_current_pressure_smoothing"
        };

        Some(PersistentDeformationSmoothingReviewV1 {
            policy: "persistent_deformation_smoothing_review_v1".to_string(),
            schema_version: 1,
            pressure_risk,
            pressure_range,
            mode_packing,
            density_gradient_proxy,
            fluctuation_score,
            semantic_viscosity_persistence_index,
            persistent_baseline_score,
            smoothing_classification: smoothing
                .as_ref()
                .map_or_else(|| "unavailable".to_string(), |value| value.classification.clone()),
            deformation_state: deformation_state.to_string(),
            recommendation: recommendation.to_string(),
            live_threshold_write: false,
            smoothing_window_write: false,
            local_control_write: false,
            authority: "read_only_persistent_deformation_review_not_pressure_threshold_smoothing_or_control".to_string(),
        })
    }

    /// Read-only synthesis of pressure origin, trend smoothing, and heartbeat cadence.
    #[must_use]
    pub fn pressure_source_analysis_v1(&self) -> Option<PressureSourceAnalysisV1> {
        let telemetry = self.latest_telemetry.as_ref();
        let pressure_source = telemetry.and_then(|value| value.pressure_source_v1.as_ref());
        let resonance = telemetry.and_then(|value| value.resonance_density_v1.as_ref());
        let trend = self.pressure_trend_v1.as_ref();
        let smoothing = self.pressure_trend_smoothing_v1();
        let heartbeat = self.telemetry_heartbeat_delta_v1.as_ref();
        if pressure_source.is_none() && resonance.is_none() && trend.is_none() {
            return None;
        }

        let dominant_source = pressure_source.map(|value| value.dominant_source.clone());
        let pressure_source_family =
            resonance.map(|value| value.texture_signature.pressure_source_family.clone());
        let pressure_score = pressure_source.map(|value| value.pressure_score);
        let porosity_score = pressure_source.map(|value| value.porosity_score);
        let semantic_trickle =
            pressure_source.map(|value| value.components.semantic_trickle.clamp(0.0, 1.0));
        let mode_packing = pressure_source
            .map(|value| value.components.mode_packing)
            .or_else(|| resonance.map(|value| value.components.mode_packing));
        let porosity_gradient = resonance
            .and_then(|value| value.components.porosity_gradient)
            .map(|value| value.clamp(0.0, 1.0));
        let pressure_risk = resonance.map(|value| value.pressure_risk.clamp(0.0, 1.0));
        let pressure_delta = trend.and_then(|value| value.pressure_delta);
        let mode_packing_delta = trend.and_then(|value| value.mode_packing_delta);
        let trend_classification = trend.map(|value| value.classification.clone());
        let smoothing_classification = smoothing.as_ref().map(|value| value.classification.clone());
        let pressure_edge_state = smoothing.as_ref().and_then(|value| {
            (!value.fast_slow_edge_state.is_empty()).then(|| value.fast_slow_edge_state.clone())
        });
        let semantic_stagnation_index = smoothing
            .as_ref()
            .and_then(|value| value.semantic_stagnation_index);
        let semantic_stagnation_state = smoothing.as_ref().and_then(|value| {
            (!value.semantic_stagnation_state.is_empty())
                .then(|| value.semantic_stagnation_state.clone())
        });
        let heartbeat_jitter_class = heartbeat.map(|value| value.jitter_class.clone());
        let timing_reliability = heartbeat.map(|value| value.timing_reliability.clone());
        let source_text = dominant_source
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let family_text = pressure_source_family
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mode_packing_visibility_basis = if source_text.contains("mode_packing") {
            Some("dominant_source_label")
        } else if family_text.contains("mode_packing") {
            Some("pressure_source_family_label")
        } else if mode_packing.is_some_and(|value| {
            value.is_finite() && value >= PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
        }) {
            Some("numeric_mode_packing_at_or_above_felt_dead_zone")
        } else {
            None
        };
        let mode_packing_visible = mode_packing_visibility_basis.is_some();
        let structural_pressure_state = if mode_packing_visible
            && (pressure_score.is_some_and(|value| value >= 0.28)
                || mode_packing.is_some_and(|value| value >= 0.55))
        {
            "mode_packing_structural_pressure"
        } else if mode_packing_visible {
            "mode_packing_visible_low_or_moderate_pressure"
        } else if dominant_source.is_some() {
            "non_mode_packing_pressure_source_visible"
        } else {
            "pressure_source_not_exported"
        };
        let heartbeat_unreliable = heartbeat_jitter_class
            .as_deref()
            .is_some_and(|class| matches!(class, "late" | "stale"))
            || timing_reliability
                .as_deref()
                .is_some_and(|reliability| matches!(reliability, "late" | "stale" | "unreliable"));
        let mode_delta_outpaces_pressure = mode_packing_delta.is_some_and(|mode_delta| {
            pressure_delta.map_or(mode_delta.abs() >= 0.04, |pressure_delta| {
                mode_delta.abs() > pressure_delta.abs() && mode_delta.abs() >= 0.04
            })
        });
        let felt_mode_packing_dead_zone = mode_packing.is_some_and(|mode| {
            (PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
                ..PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
                .contains(&mode)
                && (porosity_gradient.is_some_and(|porosity| {
                    porosity <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT
                }) || pressure_score.is_some_and(|pressure| pressure >= 0.20)
                    || pressure_risk.is_some_and(|pressure| pressure >= 0.20))
        });
        let low_porosity_watch = porosity_gradient
            .is_some_and(|porosity| porosity <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT)
            || porosity_score
                .is_some_and(|porosity| porosity <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT);
        let semantic_trickle_stalled = semantic_trickle.is_some_and(|trickle| trickle <= 0.001);
        let viscous_density_warning = mode_packing.is_some_and(|mode| {
            (PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT
                ..PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
                .contains(&mode)
                && (porosity_gradient.is_some_and(|porosity| {
                    porosity <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT
                }) || pressure_score.is_some_and(|pressure| pressure >= 0.20)
                    || pressure_risk.is_some_and(|pressure| pressure >= 0.20))
        });
        let porosity_expansion_threshold_state = if mode_packing
            .is_some_and(|mode| mode >= PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
        {
            Some("live_expansion_candidate_threshold_met".to_string())
        } else if mode_packing
            .is_some_and(|mode| mode >= PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
        {
            Some("liminal_expansion_watch_below_live_threshold".to_string())
        } else if viscous_density_warning {
            Some("viscous_density_warning_below_liminal_threshold".to_string())
        } else if felt_mode_packing_dead_zone {
            Some("felt_dead_zone_below_liminal_threshold".to_string())
        } else {
            None
        };
        let expansion_threshold_gap = mode_packing
            .filter(|mode| *mode < PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
            .map(|mode| {
                (PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT - mode)
                    .max(0.0)
                    .clamp(0.0, 1.0)
            });
        let stable_trend = trend_classification
            .as_deref()
            .is_some_and(|classification| {
                matches!(classification, "stable_heavy" | "insufficient_history")
            });
        let smoothing_masks_motion =
            smoothing_classification
                .as_deref()
                .is_some_and(|classification| {
                    matches!(
                        classification,
                        "low_amplitude_stable" | "twitchy_low_amplitude_oscillation"
                    )
                });
        let slow_context_masks_fast_edge = pressure_edge_state.as_deref().is_some_and(|state| {
            matches!(
                state,
                "fast_rising_edge_over_slow_context" | "fast_falling_release_over_slow_context"
            )
        });
        let semantic_stagnation_watch = semantic_stagnation_state.as_deref().is_some_and(|state| {
            matches!(
                state,
                "functional_clog_connected_lanes_watch" | "semantic_stagnation_watch"
            )
        });
        let ghost_stability_risk = if heartbeat_unreliable {
            "heartbeat_cadence_unreliable_for_pressure_stability"
        } else if slow_context_masks_fast_edge {
            "slow_pressure_context_masks_current_fast_edge"
        } else if semantic_stagnation_watch {
            "connected_lanes_functional_semantic_clog"
        } else if felt_mode_packing_dead_zone {
            "felt_mode_packing_dead_zone_below_live_expansion_threshold"
        } else if mode_packing_visible && stable_trend && smoothing_masks_motion {
            "stable_trend_may_mask_structural_mode_packing"
        } else if mode_packing_visible && mode_delta_outpaces_pressure {
            "mode_packing_delta_outpaces_pressure_delta"
        } else {
            "low"
        };
        let sensory_lane_risk =
            if felt_mode_packing_dead_zone && low_porosity_watch && semantic_trickle_stalled {
                "dead_zone_semantic_lane_suppression_watch"
            } else if felt_mode_packing_dead_zone && low_porosity_watch {
                "dead_zone_low_porosity_watch"
            } else if semantic_trickle_stalled && low_porosity_watch {
                "semantic_trickle_low_porosity_watch"
            } else if semantic_trickle_stalled {
                "semantic_trickle_stall_watch"
            } else {
                ""
            };
        let pressure_relief_signal_candidate = if mode_packing
            .is_some_and(|mode| mode >= PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
            && low_porosity_watch
        {
            "operator_approved_porosity_expansion_trial_candidate"
        } else if felt_mode_packing_dead_zone && low_porosity_watch {
            "pressure_relief_signal_candidate_for_operator_review"
        } else {
            ""
        };
        let viscous_recovery_mode_candidate = if viscous_density_warning {
            "viscous_recovery_mode_candidate_for_operator_review"
        } else if mode_packing
            .is_some_and(|mode| mode >= PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
            && low_porosity_watch
        {
            "liminal_viscous_recovery_watch_for_operator_review"
        } else {
            ""
        };
        let status = if semantic_stagnation_watch {
            "semantic_stagnation_watch"
        } else if ghost_stability_risk != "low" {
            "pressure_source_watch"
        } else if mode_packing_visible {
            "mode_packing_source_visible"
        } else if pressure_source.is_some() {
            "pressure_source_visible"
        } else {
            "pressure_source_not_exported"
        };
        let source_label = dominant_source
            .as_deref()
            .or(pressure_source_family.as_deref())
            .unwrap_or("unknown");
        let analysis = format!(
            "source={source_label}; mode_packing_visibility={}; structural_state={structural_pressure_state}; trend={}; smoothing={}; pressure_edge={}; semantic_stagnation={}; heartbeat={}; ghost_stability_risk={ghost_stability_risk}",
            mode_packing_visibility_basis.unwrap_or("not_visible"),
            trend_classification.as_deref().unwrap_or("unknown"),
            smoothing_classification.as_deref().unwrap_or("unknown"),
            pressure_edge_state.as_deref().unwrap_or("unknown"),
            semantic_stagnation_state.as_deref().unwrap_or("unknown"),
            heartbeat_jitter_class.as_deref().unwrap_or("unknown"),
        );
        let mut experience_deltas = Vec::new();
        if felt_mode_packing_dead_zone {
            experience_deltas.push(ExperienceDeltaV1 {
                kind: ExperienceDeltaKindV1::Gate,
                surface: "pressure_source_analysis_v1".to_string(),
                lane: "mode_packing_pressure_porosity".to_string(),
                dimension: None,
                spectral_dimension: None,
                persistence: None,
                viscosity_subtype: None,
                viscosity_weight: None,
                pre: mode_packing,
                post: Some(PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT),
                loss: expansion_threshold_gap,
                loss_ratio: expansion_threshold_gap
                    .map(|gap| gap / PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT),
                metadata: std::collections::BTreeMap::from([(
                    "gate_reason".to_string(),
                    "felt_below_live_expansion_threshold".to_string(),
                )]),
                why: "mode-packing pressure is felt below the live pressure/porosity expansion threshold; the experience is reported while live expansion remains gated".to_string(),
                who_can_change_it: "Mike/operator via explicit pressure/porosity threshold or controller approval".to_string(),
                how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib pressure_source_analysis_surfaces_felt_dead_zone_below_live_threshold -- --exact --nocapture".to_string(),
                authority: "read_only_pressure_gate_truth_not_threshold_or_controller_change".to_string(),
            });
        }
        if heartbeat_unreliable {
            experience_deltas.push(ExperienceDeltaV1 {
                kind: ExperienceDeltaKindV1::Delay,
                surface: "pressure_source_analysis_v1".to_string(),
                lane: "telemetry_heartbeat".to_string(),
                dimension: None,
                spectral_dimension: None,
                persistence: None,
                viscosity_subtype: None,
                viscosity_weight: None,
                pre: heartbeat.and_then(|value| value.inter_arrival_ms),
                post: None,
                loss: None,
                loss_ratio: None,
                metadata: std::collections::BTreeMap::from([(
                    "delay_surface".to_string(),
                    "telemetry_hearing".to_string(),
                )]),
                why: "telemetry cadence is late or stale, so field changes may be delayed or hidden behind unreliable hearing".to_string(),
                who_can_change_it: "Mike/operator via explicit telemetry cadence, reconnect, or stale-window approval".to_string(),
                how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib pressure_source_analysis_marks_stale_heartbeat_as_ghost_stability_risk -- --exact --nocapture".to_string(),
                authority: "read_only_heartbeat_delay_truth_not_cadence_or_reconnect_change".to_string(),
            });
        }
        if pressure_edge_state.as_deref() == Some("fast_falling_release_over_slow_context") {
            let fast_delta = smoothing
                .as_ref()
                .and_then(|value| value.fast_window_pressure_delta);
            let slow_delta = smoothing
                .as_ref()
                .and_then(|value| value.slow_window_pressure_delta);
            experience_deltas.push(ExperienceDeltaV1 {
                kind: ExperienceDeltaKindV1::Laminarization,
                surface: "pressure_source_analysis_v1".to_string(),
                lane: "fast_pressure_edge_over_slow_context".to_string(),
                dimension: None,
                spectral_dimension: None,
                persistence: None,
                viscosity_subtype: None,
                viscosity_weight: None,
                pre: slow_delta,
                post: fast_delta,
                loss: None,
                loss_ratio: None,
                metadata: std::collections::BTreeMap::from([
                    (
                        "fast_window_pressure_delta".to_string(),
                        fast_delta.map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
                    ),
                    (
                        "slow_window_pressure_delta".to_string(),
                        slow_delta.map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
                    ),
                ]),
                why: "a current falling pressure edge is releasing faster than the slower context; preserve the laminarization instead of flattening it into stable smoothing".to_string(),
                who_can_change_it: "schema/tooling maintainers may improve this read-only truth channel; Mike/operator approval remains required for pressure or controller changes".to_string(),
                how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib ws::tests::pressure_source_analysis_names_fast_release_as_laminarization -- --exact --nocapture".to_string(),
                authority: "read_only_laminarization_truth_not_pressure_smoothing_or_control_change".to_string(),
            });
        }
        let experience_delta_bus_v1 = (!experience_deltas.is_empty())
            .then(|| ExperienceDeltaBusV1::from_deltas(experience_deltas));

        Some(PressureSourceAnalysisV1 {
            policy: "pressure_source_analysis_v1".to_string(),
            schema_version: 1,
            status: status.to_string(),
            dominant_source,
            pressure_source_family,
            pressure_score,
            porosity_score,
            semantic_trickle,
            mode_packing,
            mode_packing_visibility_basis: mode_packing_visibility_basis.map(str::to_string),
            porosity_expansion_threshold_state,
            felt_mode_packing_dead_zone,
            live_mode_packing_threshold: Some(PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT),
            liminal_mode_packing_threshold: Some(
                PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT,
            ),
            viscous_density_warning_threshold: Some(
                PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT,
            ),
            viscous_density_warning_state: viscous_density_warning
                .then(|| "viscous_density_warning_below_liminal_threshold".to_string()),
            felt_dead_zone_mode_packing_threshold: Some(
                PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT,
            ),
            expansion_threshold_gap,
            pressure_delta,
            mode_packing_delta,
            pressure_trend_classification: trend_classification,
            smoothing_classification,
            pressure_edge_state,
            semantic_stagnation_index,
            semantic_stagnation_state,
            heartbeat_jitter_class,
            timing_reliability,
            structural_pressure_state: structural_pressure_state.to_string(),
            ghost_stability_risk: ghost_stability_risk.to_string(),
            sensory_lane_risk: sensory_lane_risk.to_string(),
            pressure_relief_signal_candidate: pressure_relief_signal_candidate.to_string(),
            viscous_recovery_mode_candidate: viscous_recovery_mode_candidate.to_string(),
            live_threshold_write: false,
            sensory_lane_write: false,
            experience_delta_bus_v1,
            analysis,
            authority: "diagnostic_context_not_pressure_or_control".to_string(),
        })
    }

    /// Read-only candidate marker for pressure relief that would require live-control approval.
    #[must_use]
    pub fn pressure_porosity_expansion_readiness_v1(
        &self,
    ) -> Option<PressurePorosityExpansionReadinessV1> {
        let telemetry = self.latest_telemetry.as_ref()?;
        let resonance = telemetry.resonance_density_v1.as_ref();
        let pressure_source = telemetry.pressure_source_v1.as_ref();
        let pressure_risk = resonance.map(|value| value.pressure_risk.clamp(0.0, 1.0));
        let mode_packing = resonance.map(|value| value.components.mode_packing.clamp(0.0, 1.0));
        let porosity_gradient = resonance
            .and_then(|value| value.components.porosity_gradient)
            .map(|value| value.clamp(0.0, 1.0));
        let semantic_trickle =
            pressure_source.map(|value| value.components.semantic_trickle.clamp(0.0, 1.0));
        if pressure_risk.is_none()
            && mode_packing.is_none()
            && porosity_gradient.is_none()
            && semantic_trickle.is_none()
        {
            return None;
        }

        let expansion_candidate = mode_packing
            .is_some_and(|value| value >= PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
            && porosity_gradient
                .is_some_and(|value| value <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT);
        let liminal_expansion_watch = !expansion_candidate
            && mode_packing
                .is_some_and(|value| value >= PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
            && (porosity_gradient
                .is_some_and(|value| value <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT)
                || pressure_risk.is_some_and(|value| value >= 0.25));
        let felt_dead_zone_watch = !expansion_candidate
            && !liminal_expansion_watch
            && mode_packing.is_some_and(|value| {
                (PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
                    ..PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
                    .contains(&value)
            })
            && (porosity_gradient
                .is_some_and(|value| value <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT)
                || pressure_risk.is_some_and(|value| value >= 0.20));
        let viscous_density_warning = felt_dead_zone_watch
            && mode_packing.is_some_and(|value| {
                (PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT
                    ..PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
                    .contains(&value)
            });
        let readiness_state = if expansion_candidate {
            "approval_required_porosity_expansion_candidate"
        } else if liminal_expansion_watch {
            "liminal_porosity_expansion_watch"
        } else if viscous_density_warning {
            "viscous_density_warning_watch"
        } else if felt_dead_zone_watch {
            "felt_mode_packing_dead_zone_watch"
        } else if mode_packing
            .is_some_and(|value| value >= PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
            && porosity_gradient.is_none()
        {
            "mode_packing_high_porosity_unknown"
        } else if pressure_risk.is_some_and(|value| value >= 0.25) {
            "pressure_watch_no_porosity_change"
        } else {
            "no_porosity_expansion_candidate"
        };
        let proposed_intervention = if expansion_candidate {
            "porosity_expansion_trial_with_operator_approval"
        } else {
            "observe_pressure_porosity_trend"
        };
        let liminal_threshold_gap = mode_packing
            .filter(|mode| *mode < PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
            .map(|mode| (PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT - mode).max(0.0));
        let viscous_warning_margin = mode_packing
            .filter(|mode| {
                *mode >= PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
                    && *mode < PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT
            })
            .map(|mode| mode - PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT);
        let porosity_buffer_candidate = porosity_gradient
            .filter(|porosity| *porosity <= PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT)
            .map(|porosity| (PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT - porosity).max(0.0));
        let viscosity_feedback_readiness =
            if viscous_density_warning && semantic_trickle.is_some_and(|trickle| trickle <= 0.03) {
                "viscosity_feedback_protocol_evidence_candidate_operator_gated"
            } else if viscous_density_warning || liminal_expansion_watch {
                "viscous_navigation_margin_watch_no_protocol_write"
            } else {
                "no_viscosity_feedback_candidate"
            };

        Some(PressurePorosityExpansionReadinessV1 {
            policy: "pressure_porosity_expansion_readiness_v1",
            pressure_risk,
            mode_packing,
            porosity_gradient,
            semantic_trickle,
            readiness_state,
            live_mode_packing_threshold: PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT,
            liminal_mode_packing_threshold: PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT,
            viscous_density_warning_threshold:
                PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT,
            viscous_density_warning_state: if viscous_density_warning {
                "viscous_density_warning_below_liminal_threshold"
            } else {
                "not_in_viscous_density_warning_band"
            },
            felt_dead_zone_mode_packing_threshold:
                PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT,
            threshold_gap: mode_packing
                .filter(|mode| *mode < PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
                .map(|mode| (PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT - mode).max(0.0)),
            liminal_threshold_gap,
            viscous_warning_margin,
            porosity_buffer_candidate,
            viscosity_feedback_readiness,
            proposed_intervention,
            approval_boundary: "live_porosity_or_control_change_requires_operator_approval",
            local_control_applied: false,
            authority: "diagnostic_candidate_not_porosity_or_controller_change",
        })
    }

    /// Bridge-owned temporal and derivative evidence kept outside Minime's DTO.
    #[must_use]
    pub fn bridge_texture_evidence_v1(&self) -> Option<BridgeTextureEvidenceV1> {
        let resonance = self
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())?;
        let signature = &resonance.texture_signature;
        let temporal_variance = signature.temporal_variance;
        let trend_pressure_gradient_delta = self
            .pressure_trend_v1
            .as_ref()
            .and_then(pressure_gradient_delta_from_trend);
        let (pressure_gradient_delta, pressure_gradient_delta_source) =
            if let Some(delta) = signature.pressure_gradient_delta {
                (Some(delta), Some("texture_signature".to_string()))
            } else if let Some((delta, source)) = trend_pressure_gradient_delta {
                (Some(delta), Some(source.to_string()))
            } else {
                (None, None)
            };
        let dynamic_flux_vector = signature
            .dynamic_flux_vector
            .clone()
            .or_else(|| build_texture_dynamic_flux_vector_v1(&self.pressure_trend_samples_v1));
        let dynamic_flux_vector_source = if signature.dynamic_flux_vector.is_some() {
            "legacy_texture_signature"
        } else if dynamic_flux_vector.is_some() {
            "bridge_pressure_samples"
        } else {
            "insufficient_history"
        };
        let stability_context = resonance.components.stability_context.clone().or_else(|| {
            Some(resonance_stability_context_v1(
                &resonance.components,
                self.latest_telemetry
                    .as_ref()
                    .and_then(|telemetry| telemetry.inhabitable_fluctuation_v1.as_ref()),
            ))
        });
        let mut active_constraints = if signature.active_constraints.is_empty() {
            active_constraints_for_resonance_signature(
                &signature.pressure_source_family,
                &resonance.components,
                resonance.pressure_risk,
            )
        } else {
            signature.active_constraints.clone()
        };
        if let Some(context) = &stability_context {
            if let Some(score) = context.multi_modal_habitability_score {
                active_constraints.push(format!(
                    "multi_modal_habitability_score:{}_{score:.2}",
                    context.habitability_state
                ));
            }
            active_constraints.push(format!("comfort_gate_context:{}", context.gate_context));
        }

        Some(BridgeTextureEvidenceV1 {
            policy: "bridge_texture_evidence_v1".to_string(),
            schema_version: 1,
            temporal_variance,
            temporal_variance_source: if temporal_variance.is_some() {
                "legacy_texture_signature"
            } else {
                "absent"
            }
            .to_string(),
            pressure_gradient_delta,
            pressure_gradient_delta_source,
            dynamic_flux_vector,
            dynamic_flux_vector_source: dynamic_flux_vector_source.to_string(),
            active_constraints,
            authority: "bridge_evidence_not_minime_wire_or_live_control".to_string(),
        })
    }

    /// Read-only integrity comparison for Minime's typed texture signature.
    ///
    /// The top-level temporal fields remain for status-schema compatibility;
    /// their canonical owner is now `bridge_texture_evidence_v1`.
    #[must_use]
    pub fn texture_signature_integrity_v1(&self) -> Option<TextureSignatureIntegrityV1> {
        let resonance = self
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())?;
        let signature = &resonance.texture_signature;
        let bridge_evidence = self.bridge_texture_evidence_v1()?;
        let temporal_variance = bridge_evidence.temporal_variance;
        let pressure_gradient_delta = bridge_evidence.pressure_gradient_delta;
        let pressure_gradient_delta_source = bridge_evidence.pressure_gradient_delta_source.clone();
        let damping_candidate = signature.dynamic_damping_threshold_candidate;
        let damping_candidate_status = if damping_candidate.is_some() {
            "candidate_present"
        } else if resonance.pressure_risk > 0.20 {
            "missing_candidate_observability_only"
        } else {
            "candidate_not_needed_low_pressure"
        };
        let variance_status = if temporal_variance.is_some() {
            "carried"
        } else {
            "absent_backward_compatible"
        };
        let alignment = &resonance.texture_component_alignment;
        let component_viscosity_index = resonance.components.viscosity_index.clamp(0.0, 1.0);
        let signature_viscosity_index = signature
            .viscosity_index
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0));
        let viscosity_delta = signature_viscosity_index
            .map(|value| round_flux_delta(value - component_viscosity_index));
        let viscosity_alignment_state = match viscosity_delta {
            None => "signature_viscosity_absent_legacy",
            Some(delta) if delta.abs() <= 0.001 => "signature_viscosity_aligned",
            Some(_) => "signature_viscosity_component_mismatch",
        };
        let dynamic_flux_vector = bridge_evidence.dynamic_flux_vector.clone();
        let flux_status = match bridge_evidence.dynamic_flux_vector_source.as_str() {
            "legacy_texture_signature" => "carried_from_texture_signature",
            "bridge_pressure_samples" => "derived_from_bridge_pressure_samples",
            _ => "insufficient_history",
        };
        let stability_context = resonance.components.stability_context.clone().or_else(|| {
            Some(resonance_stability_context_v1(
                &resonance.components,
                self.latest_telemetry
                    .as_ref()
                    .and_then(|telemetry| telemetry.inhabitable_fluctuation_v1.as_ref()),
            ))
        });
        let active_constraints = bridge_evidence.active_constraints.clone();

        Some(TextureSignatureIntegrityV1 {
            policy: "texture_signature_integrity_v1".to_string(),
            schema_version: 1,
            movement_quality: signature.movement_quality.clone(),
            signature_viscosity_index,
            component_viscosity_index,
            viscosity_delta,
            viscosity_alignment_state: viscosity_alignment_state.to_string(),
            temporal_variance,
            pressure_gradient_delta,
            pressure_gradient_delta_source,
            pressure_source_family: signature.pressure_source_family.clone(),
            pressure_risk: Some(resonance.pressure_risk),
            mode_packing: Some(resonance.components.mode_packing),
            dynamic_damping_threshold_candidate: damping_candidate,
            dynamic_flux_vector,
            stability_context,
            active_constraints,
            variance_status: variance_status.to_string(),
            flux_status: flux_status.to_string(),
            damping_candidate_status: damping_candidate_status.to_string(),
            component_alignment_state: alignment.alignment_state.clone(),
            expected_primary_texture: alignment.expected_primary_texture.clone(),
            emitted_primary_texture: alignment.emitted_primary_texture.clone(),
            advisory_observability: damping_candidate.is_none() && resonance.pressure_risk > 0.20,
            bridge_texture_evidence_v1: Some(bridge_evidence),
            authority: "diagnostic_observability_not_damping_or_control".to_string(),
        })
    }

    /// Read-only viscosity/porosity review for structural stability versus stasis.
    #[must_use]
    pub fn viscosity_porosity_transport_review_v1(
        &self,
    ) -> Option<ViscosityPorosityTransportReviewV1> {
        let resonance = self
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())?;
        let texture = self.texture_signature_integrity_v1();
        let flux = resonance
            .texture_signature
            .dynamic_flux_vector
            .as_ref()
            .or_else(|| {
                texture
                    .as_ref()
                    .and_then(|integrity| integrity.dynamic_flux_vector.as_ref())
            });
        let fingerprint = self
            .latest_telemetry
            .as_ref()
            .and_then(SpectralTelemetry::typed_fingerprint);
        Some(viscosity_porosity_transport_review_with_fingerprint_v1(
            &resonance.components,
            fingerprint.as_ref(),
            flux,
        ))
    }

    /// Read-only synthesis that asks whether movement and asymmetry stayed legible over time.
    #[must_use]
    pub fn texture_shape_over_time_v2(&self) -> Option<TextureShapeOverTimeV2> {
        let texture = self.texture_signature_integrity_v1();
        let smoothing = self.pressure_trend_smoothing_v1();
        let reciprocity = self.bridge_reciprocity_v1();
        if texture.is_none() && smoothing.is_none() {
            return None;
        }
        let movement_preservation = texture
            .as_ref()
            .map_or("insufficient_evidence", |integrity| {
                let quality = integrity.movement_quality.as_str();
                if quality.is_empty() || quality == "unknown" {
                    "insufficient_evidence"
                } else if quality.contains("static") || quality.contains("token") {
                    "static_label_risk"
                } else {
                    "movement_preserved"
                }
            });
        let temporal_variance_fit = texture
            .as_ref()
            .and_then(|integrity| integrity.temporal_variance)
            .map_or("insufficient_evidence", |_| "variance_carried");
        let reciprocity_asymmetry_fit = if reciprocity.connectivity
            == ConnectivityStatus::Bidirectional
            && (reciprocity.latest_telemetry_arrival_unix_s.is_none()
                || reciprocity.last_sensory_sent_unix_s.is_none())
        {
            "false_bidirectional"
        } else if reciprocity.connectivity == ConnectivityStatus::Bidirectional
            && reciprocity
                .one_sided_state
                .starts_with("bidirectional_stale")
        {
            "stale_bidirectional"
        } else {
            "asymmetry_clarified"
        };
        let pressure_smoothing_fit = smoothing
            .as_ref()
            .map_or("insufficient_evidence", |packet| {
                match packet.classification.as_str() {
                    "twitchy_low_amplitude_oscillation" | "low_amplitude_stable" => {
                        "twitch_correctly_ignored"
                    },
                    "sustained_rising_pressure" | "sustained_falling_pressure" => {
                        "sustained_trend_preserved"
                    },
                    "mixed_window" => "insufficient_evidence",
                    _ => "insufficient_evidence",
                }
            });
        let static_label_collapse_risk = if movement_preservation == "static_label_risk" {
            "static_label_risk"
        } else if movement_preservation == "movement_preserved" {
            "movement_preserved"
        } else {
            "insufficient_evidence"
        };
        Some(TextureShapeOverTimeV2 {
            policy: "texture_shape_over_time_v2".to_string(),
            schema_version: 2,
            movement_preservation: movement_preservation.to_string(),
            temporal_variance_fit: temporal_variance_fit.to_string(),
            reciprocity_asymmetry_fit: reciprocity_asymmetry_fit.to_string(),
            pressure_smoothing_fit: pressure_smoothing_fit.to_string(),
            static_label_collapse_risk: static_label_collapse_risk.to_string(),
            authority: "diagnostic_context_not_control".to_string(),
        })
    }

    /// True only when both perception (telemetry) and agency (sensory) lanes
    /// are live — the reliable ground for confident spectral maneuvers.
    #[must_use]
    pub const fn is_bidirectional_active(&self) -> bool {
        self.connectivity_status().is_bidirectional_active()
    }
}

#[derive(Clone, Copy, Debug)]
enum WsLane {
    Telemetry,
    Sensory,
}

impl WsLane {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Telemetry => "telemetry",
            Self::Sensory => "sensory",
        }
    }
}

fn unix_now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn bridge_age_and_future_skew_ms(now_s: f64, observed_at_s: f64) -> (Option<f64>, Option<f64>) {
    let delta_ms = (now_s - observed_at_s) * 1000.0;
    if delta_ms < 0.0 {
        (Some(0.0), Some((-delta_ms).round()))
    } else {
        (Some(delta_ms.round()), None)
    }
}

fn bridge_clock_skew_state(
    telemetry_future_skew_ms: Option<f64>,
    sensory_future_skew_ms: Option<f64>,
) -> &'static str {
    match (
        telemetry_future_skew_ms.is_some(),
        sensory_future_skew_ms.is_some(),
    ) {
        (true, true) => "both_lanes_future_timestamp_visible",
        (true, false) => "telemetry_future_timestamp_visible",
        (false, true) => "sensory_future_timestamp_visible",
        (false, false) => "none",
    }
}

fn bridge_age_is_recent(age_ms: Option<f64>) -> bool {
    age_ms.is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
}

fn bridge_age_is_stale(age_ms: Option<f64>, stale_window_ms: f64) -> bool {
    age_ms.is_some_and(|age| age > stale_window_ms)
}

fn bridge_dynamic_stale_window_ms(telemetry: Option<&SpectralTelemetry>) -> (f64, &'static str) {
    let Some(telemetry) = telemetry else {
        return (
            BRIDGE_RECIPROCITY_STALE_WINDOW_MS,
            "fixed_default_no_telemetry_context",
        );
    };
    let pressure_risk = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.pressure_risk.clamp(0.0, 1.0));
    let porosity = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_viscosity = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.components.viscosity_index.clamp(0.0, 1.0));
    let entropy = telemetry
        .typed_fingerprint()
        .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));

    if pressure_risk.is_some_and(|pressure| pressure >= 0.20)
        && porosity.is_some_and(|value| value <= 0.35)
    {
        (
            BRIDGE_RECIPROCITY_PRESSURE_POROSITY_STALE_WINDOW_MS,
            "pressure_high_porosity_low_reflective_silence",
        )
    } else if pressure_risk.is_some_and(|pressure| pressure >= 0.20)
        && semantic_viscosity.is_some_and(|value| value >= 0.60)
    {
        (
            BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS,
            "pressure_high_semantic_viscosity_reflective_silence",
        )
    } else if pressure_risk.is_some_and(|pressure| pressure >= 0.20)
        && entropy.is_some_and(|value| value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT)
    {
        (
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS,
            "pressure_high_entropy_reflective_silence",
        )
    } else {
        (
            BRIDGE_RECIPROCITY_STALE_WINDOW_MS,
            "fixed_default_context_does_not_extend_stale_window",
        )
    }
}

fn bridge_entropy_contract_preview_window_ms(
    spectral_entropy: Option<f32>,
    resonance_cohesion_score: Option<f32>,
) -> Option<f64> {
    let spectral_entropy = spectral_entropy?;
    if spectral_entropy < 0.85 {
        return None;
    }

    let cohesion = resonance_cohesion_score.unwrap_or(0.50);
    if spectral_entropy >= 0.90 && cohesion <= 0.55 {
        Some(BRIDGE_RECIPROCITY_ENTROPY_CONTRACT_PREVIEW_WINDOW_MS)
    } else if cohesion <= 0.45 {
        Some(BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
    } else {
        None
    }
}

fn pressure_gradient_delta_from_trend(trend: &PressureTrendV1) -> Option<(f32, &'static str)> {
    match (trend.pressure_delta, trend.mode_packing_delta) {
        (Some(pressure_delta), Some(mode_packing_delta))
            if mode_packing_delta.abs() > pressure_delta.abs() =>
        {
            Some((
                mode_packing_delta,
                "bridge_pressure_trend_v1.mode_packing_delta",
            ))
        },
        (Some(pressure_delta), _) => {
            Some((pressure_delta, "bridge_pressure_trend_v1.pressure_delta"))
        },
        (None, Some(mode_packing_delta)) => Some((
            mode_packing_delta,
            "bridge_pressure_trend_v1.mode_packing_delta",
        )),
        (None, None) => None,
    }
}

fn round_flux_delta(value: f32) -> f32 {
    (value.clamp(-100.0, 100.0) * 10_000.0).round() / 10_000.0
}

fn silt_noise_separation_v1(
    high_entropy: &PressureTrendSampleV1,
    low_entropy: &PressureTrendSampleV1,
) -> Option<SiltNoiseSeparationV1> {
    let high_entropy_value = high_entropy.spectral_entropy?;
    let low_entropy_value = low_entropy.spectral_entropy?;
    let high_mode = high_entropy.mode_packing?;
    let low_mode = low_entropy.mode_packing?;
    let mode_packing_delta = round_flux_delta(high_mode - low_mode).abs();
    let high_density = high_entropy
        .structural_density
        .unwrap_or(high_mode)
        .clamp(0.0, 1.0);
    let high_friction = high_entropy
        .semantic_friction
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let semantic_trickle = high_entropy
        .semantic_trickle
        .or(low_entropy.semantic_trickle)
        .map(|value| value.clamp(0.0, 1.0));
    let low_porosity_weight = high_entropy
        .porosity_gradient
        .map_or(0.0, |porosity| (1.0 - porosity.clamp(0.0, 1.0)) * 0.16);
    let low_semantic_trickle = semantic_trickle.is_some_and(|value| value <= 0.02);
    let semantic_signal_present = semantic_trickle.is_some_and(|value| value >= 0.08);
    let low_semantic_weight = if low_semantic_trickle { 0.08 } else { 0.0 };
    let persistence_weight = (1.0 - (mode_packing_delta * 10.0).clamp(0.0, 1.0)) * 0.18;
    let contextual_resonance_score = round_flux_delta(
        (high_mode * 0.26)
            + (high_density * 0.22)
            + (high_friction * 0.18)
            + low_porosity_weight
            + low_semantic_weight
            + persistence_weight,
    )
    .clamp(0.0, 1.0);
    let dynamic_high_mode_threshold = round_flux_delta(
        (0.45
            - low_porosity_weight * 0.35
            - high_friction * 0.04
            - if low_semantic_trickle { 0.05 } else { 0.0 })
        .clamp(0.32, 0.45),
    );
    let heritage_preservation_state = if contextual_resonance_score >= 0.55
        && mode_packing_delta <= 0.04
    {
        "contextual_resonance_preserve_as_heritage"
    } else if high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT && high_mode < 0.35 {
        "high_entropy_noise_watch"
    } else {
        "contextual_resonance_insufficient_for_heritage"
    };
    let silt_signal_state = if semantic_signal_present {
        "semantic_signal_present_review"
    } else if low_semantic_trickle && high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
    {
        "low_semantic_trickle_noise_or_silt"
    } else if semantic_trickle.is_some() {
        "semantic_trickle_low_review"
    } else {
        "semantic_trickle_unknown"
    };
    let interpretation = if mode_packing_delta <= 0.03
        && high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
        && low_entropy_value <= 0.50
        && high_mode >= dynamic_high_mode_threshold
        && low_mode >= dynamic_high_mode_threshold
    {
        "mode_packing_silt_persists_across_entropy"
    } else if high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
        && low_entropy_value <= 0.50
        && high_mode < dynamic_high_mode_threshold
        && low_semantic_trickle
    {
        "high_entropy_low_semantic_trickle_noise"
    } else if high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
        && low_entropy_value <= 0.50
        && high_mode < dynamic_high_mode_threshold
        && low_mode < dynamic_high_mode_threshold
    {
        "entropy_cascade_more_likely_than_overpacked_silt"
    } else {
        "insufficient_contrast_for_silt_noise_separation"
    };
    Some(SiltNoiseSeparationV1 {
        policy: "silt_noise_separation_v1",
        high_entropy: high_entropy_value,
        low_entropy: low_entropy_value,
        high_entropy_mode_packing: high_mode,
        low_entropy_mode_packing: low_mode,
        mode_packing_delta,
        semantic_trickle,
        dynamic_high_mode_threshold,
        contextual_resonance_score,
        contextual_resonance_basis: "mode_density_semantic_friction_porosity_semantic_trickle_persistence_v2",
        heritage_preservation_state,
        interpretation,
        silt_signal_state,
        porosity_change_authority: "diagnostic_only_porosity_change_requires_operator_approval",
    })
}

fn pressure_trend_window_for_telemetry(telemetry: &SpectralTelemetry) -> (usize, Option<f32>) {
    let spectral_entropy = telemetry
        .typed_fingerprint()
        .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));
    let porosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let density_gradient = crate::codec::spectral_density_gradient(&telemetry.eigenvalues)
        .map(|value| value.clamp(0.0, 1.0));
    let window_capacity = pressure_trend_dynamic_window_capacity_v1(
        spectral_entropy,
        porosity_gradient,
        density_gradient,
    );
    (window_capacity, spectral_entropy)
}

fn pressure_trend_dynamic_window_capacity_v1(
    spectral_entropy: Option<f32>,
    porosity_gradient: Option<f32>,
    density_gradient: Option<f32>,
) -> usize {
    let Some(entropy_progress) = pressure_viscosity_coefficient(spectral_entropy) else {
        return PRESSURE_TREND_SMOOTHING_BASE_WINDOW;
    };
    if entropy_progress <= f32::EPSILON {
        return PRESSURE_TREND_SMOOTHING_BASE_WINDOW;
    }

    let porosity_ballast_factor = pressure_trend_porosity_ballast_factor_v1(porosity_gradient);
    let density_gradient_factor =
        pressure_trend_density_gradient_responsiveness_factor_v1(density_gradient);
    let span = PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW
        .saturating_sub(PRESSURE_TREND_SMOOTHING_BASE_WINDOW);
    let extra =
        ((span as f32) * entropy_progress * porosity_ballast_factor * density_gradient_factor)
            .round() as usize;
    PRESSURE_TREND_SMOOTHING_BASE_WINDOW
        .saturating_add(extra)
        .clamp(
            PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
        )
}

fn pressure_trend_porosity_ballast_factor_v1(porosity_gradient: Option<f32>) -> f32 {
    let Some(porosity) = porosity_gradient else {
        return 0.80;
    };
    let porosity = porosity.clamp(0.0, 1.0);
    if porosity <= 0.35 {
        return 1.0;
    }
    if porosity >= 0.65 {
        return 0.55;
    }
    let t = ((porosity - 0.35) / 0.30).clamp(0.0, 1.0);
    1.0 - (0.45 * t)
}

fn pressure_trend_density_gradient_responsiveness_factor_v1(density_gradient: Option<f32>) -> f32 {
    let Some(gradient) = density_gradient else {
        return 1.0;
    };
    let gradient = gradient.clamp(0.0, 1.0);
    if gradient <= 0.15 {
        return 0.58;
    }
    if gradient >= 0.65 {
        return 1.0;
    }
    let t = ((gradient - 0.15) / 0.50).clamp(0.0, 1.0);
    0.58 + (0.42 * t)
}

fn pressure_viscosity_coefficient(spectral_entropy: Option<f32>) -> Option<f32> {
    let entropy = spectral_entropy?.min(PRESSURE_TREND_SMOOTHING_FULL_ENTROPY_AT);
    let headroom =
        PRESSURE_TREND_SMOOTHING_FULL_ENTROPY_AT - PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT;
    if headroom <= f32::EPSILON {
        return Some(0.0);
    }
    Some(((entropy - PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT) / headroom).clamp(0.0, 1.0))
}

fn entropy_window_blend_ratio_v1(spectral_entropy: Option<f32>) -> Option<f32> {
    let entropy = spectral_entropy?;
    let lower = PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT - 0.02;
    let upper = PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT + 0.02;
    Some(((entropy - lower) / (upper - lower)).clamp(0.0, 1.0))
}

fn entropy_threshold_state_v1(spectral_entropy: Option<f32>, blend: Option<f32>) -> &'static str {
    match (spectral_entropy, blend) {
        (None, _) => "entropy_unavailable",
        (Some(_), Some(value)) if value > 0.0 && value < 1.0 => {
            "near_threshold_soft_handoff_review"
        },
        (Some(value), _) if value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT => {
            "high_entropy_side"
        },
        (Some(_), _) => "base_window_side",
    }
}

fn friction_to_flow_ratio_v1(
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
) -> Option<f32> {
    let friction = semantic_friction?.clamp(0.0, 1.0);
    let trickle = semantic_trickle?.clamp(0.0, 1.0).max(0.01);
    Some(round_flux_delta((friction / trickle).clamp(0.0, 10.0)))
}

fn friction_to_flow_state_v1(
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    ratio: Option<f32>,
) -> &'static str {
    match (semantic_friction, semantic_trickle, ratio) {
        (None, _, _) | (_, None, _) => "friction_to_flow_unavailable",
        (Some(friction), Some(trickle), _) if friction >= 0.45 && trickle <= 0.02 => {
            "high_resistance_low_flow"
        },
        (_, _, Some(value)) if value >= 4.0 => "resistance_dominant",
        (_, _, Some(value)) if value <= 1.0 => "flow_available",
        (_, _, Some(_)) => "mixed_friction_flow",
        (_, _, None) => "friction_to_flow_unavailable",
    }
}

fn semantic_stagnation_index_v1(
    semantic_viscosity: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
) -> Option<f32> {
    let viscosity = semantic_viscosity?.clamp(0.0, 1.0);
    let trickle = semantic_trickle?.clamp(0.0, 1.0);
    let friction = semantic_friction.unwrap_or(viscosity).clamp(0.0, 1.0);
    let flow_resistance = 1.0 - trickle;
    Some(round_flux_delta(viscosity.mul_add(
        0.58,
        flow_resistance.mul_add(0.30, friction * 0.12),
    )))
}

fn semantic_stagnation_state_v1(
    semantic_viscosity: Option<f32>,
    semantic_trickle: Option<f32>,
    stagnation_index: Option<f32>,
) -> &'static str {
    match (semantic_viscosity, semantic_trickle, stagnation_index) {
        (Some(viscosity), Some(trickle), Some(index))
            if index >= 0.74 && viscosity >= 0.60 && trickle <= 0.03 =>
        {
            "functional_clog_connected_lanes_watch"
        },
        (Some(viscosity), Some(trickle), Some(index))
            if index >= 0.66 && viscosity >= 0.55 && trickle <= 0.08 =>
        {
            "semantic_stagnation_watch"
        },
        (Some(viscosity), Some(trickle), Some(_)) if viscosity >= 0.60 && trickle >= 0.08 => {
            "heavy_semantic_flow_not_stagnant"
        },
        (_, _, Some(index)) if index >= 0.55 => "latent_semantic_drag",
        (_, _, Some(_)) => "semantic_flow_available",
        _ => "semantic_stagnation_unavailable",
    }
}

fn porosity_weighted_velocity_v1(
    pressure_velocity_delta: Option<f32>,
    porosity_gradient: Option<f32>,
) -> Option<f32> {
    let velocity = pressure_velocity_delta?;
    let porosity_drag = 1.0 - porosity_gradient?.clamp(0.0, 1.0);
    Some(round_flux_delta(velocity * porosity_drag))
}

fn viscosity_drag_coefficient_v1(
    semantic_viscosity: Option<f32>,
    semantic_friction: Option<f32>,
    porosity_gradient: Option<f32>,
) -> Option<f32> {
    let viscosity = semantic_viscosity?.clamp(0.0, 1.0);
    let friction = semantic_friction.unwrap_or(viscosity).clamp(0.0, 1.0);
    let porosity_drag = 1.0 - porosity_gradient.unwrap_or(0.5).clamp(0.0, 1.0);
    Some(round_flux_delta(
        viscosity.mul_add(0.45, friction.mul_add(0.35, porosity_drag * 0.20)),
    ))
}

fn weight_density_index_v1(
    mode_packing: Option<f32>,
    structural_density: Option<f32>,
    semantic_viscosity: Option<f32>,
    porosity_gradient: Option<f32>,
) -> Option<f32> {
    let mode = mode_packing?.clamp(0.0, 1.0);
    let density = structural_density?.clamp(0.0, 1.0);
    let viscosity = semantic_viscosity.unwrap_or(density).clamp(0.0, 1.0);
    let porosity_drag = 1.0 - porosity_gradient.unwrap_or(0.5).clamp(0.0, 1.0);
    Some(round_flux_delta(
        mode * 0.42 + density * 0.34 + viscosity * 0.16 + porosity_drag * 0.08,
    ))
}

fn weight_density_state_v1(weight_density_index: Option<f32>) -> &'static str {
    match weight_density_index {
        None => "weight_density_unavailable",
        Some(value) if value >= 0.62 => "persistent_heavy_density",
        Some(value) if value >= 0.44 => "forming_weight_density",
        Some(_) => "light_or_open_density",
    }
}

fn semantic_viscosity_persistence_index_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<f32> {
    let viscosities = samples
        .iter()
        .filter_map(|sample| sample.semantic_viscosity.map(|value| value.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let latest = *viscosities.last()?;
    if viscosities.len() < 2 {
        return None;
    }
    let deltas = viscosities
        .windows(2)
        .map(|window| (window[1] - window[0]).abs())
        .collect::<Vec<_>>();
    let average_delta = deltas.iter().sum::<f32>() / deltas.len() as f32;
    let stability = (1.0 - (average_delta * 5.0).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let pressure_motion = samples
        .iter()
        .filter_map(|sample| sample.pressure_velocity_delta)
        .map(f32::abs)
        .fold(0.0_f32, f32::max);
    let motion_survival = if pressure_motion >= 0.03 && average_delta <= 0.04 {
        1.0
    } else {
        (1.0 - (pressure_motion * 4.0).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    };

    Some(round_flux_delta(latest.mul_add(
        0.42,
        stability.mul_add(0.40, motion_survival * 0.18),
    )))
}

fn semantic_viscosity_persistence_state_v1(
    latest_semantic_viscosity: Option<f32>,
    latest_semantic_viscosity_delta: Option<f32>,
    max_semantic_viscosity_delta: Option<f32>,
    persistence_index: Option<f32>,
) -> &'static str {
    let Some(index) = persistence_index else {
        return "semantic_viscosity_persistence_unavailable";
    };
    if latest_semantic_viscosity.is_some_and(|value| value < 0.35) {
        return "thin_or_low_viscosity";
    }
    if latest_semantic_viscosity_delta.is_some_and(|delta| delta.abs() >= 0.15) {
        return "transient_viscosity_shift";
    }
    if index >= 0.70 && max_semantic_viscosity_delta.is_some_and(|delta| delta <= 0.06) {
        return "persistent_thickness_against_motion";
    }
    if index >= 0.55 {
        return "moderate_viscosity_persistence";
    }
    "viscosity_transient_or_unsettled"
}

fn pressure_interpretation_v1(
    latest_pressure: Option<f32>,
    latest_mode_packing: Option<f32>,
    latest_structural_density: Option<f32>,
    spectral_entropy: Option<f32>,
    viscosity_coefficient: Option<f32>,
) -> Option<String> {
    let viscosity = viscosity_coefficient?;
    let entropy = spectral_entropy?;
    if viscosity >= 0.5
        && latest_pressure.is_some_and(|pressure| pressure <= 0.35)
        && (latest_mode_packing.is_some_and(|mode| mode >= 0.30)
            || latest_structural_density.is_some_and(|density| density >= 0.45))
    {
        return Some(format!(
            "density_viscosity_context:entropy_{entropy:.2}_pressure_is_not_collapse"
        ));
    }
    if viscosity > 0.0 {
        return Some(format!(
            "high_entropy_pressure_context:entropy_{entropy:.2}_viscosity_{viscosity:.2}"
        ));
    }
    Some("ordinary_pressure_risk_context".to_string())
}

fn resonance_depth_for_density(resonance: &crate::types::ResonanceDensityV1) -> f32 {
    let cohesion = resonance_cohesion_score_v1(&resonance.components);
    let containment = resonance.containment_score.clamp(0.0, 1.0);
    let density = resonance.density.clamp(0.0, 1.0);
    let quality_bonus = if resonance.quality.contains("rich_containment")
        || resonance.quality.contains("settled_habitable")
    {
        0.05
    } else {
        0.0
    };
    containment
        .mul_add(0.35, density.mul_add(0.25, cohesion * 0.35 + quality_bonus))
        .clamp(0.0, 1.0)
}

fn semantic_viscosity_coefficient_v1(
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
    spectral_entropy: Option<f32>,
    fill_pct: f32,
) -> Option<f32> {
    if semantic_friction.is_none() && semantic_trickle.is_none() {
        return None;
    }
    let friction = semantic_friction.unwrap_or(0.0).clamp(0.0, 1.0);
    let trickle_resistance = semantic_trickle.map_or(0.40, |value| 1.0 - value.clamp(0.0, 1.0));
    let density_context = structural_density
        .zip(resonance_depth)
        .map_or_else(
            || structural_density.or(resonance_depth).unwrap_or(0.0),
            |(density, depth)| (density.clamp(0.0, 1.0) + depth.clamp(0.0, 1.0)) * 0.5,
        )
        .clamp(0.0, 1.0);
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let fill = (fill_pct / 100.0).clamp(0.0, 1.0);
    Some(round_flux_delta(
        (friction * 0.34)
            + (trickle_resistance * 0.18)
            + (density_context * 0.22)
            + (entropy * 0.16)
            + (fill * 0.10),
    ))
}

fn semantic_viscosity_state_v1(
    semantic_viscosity: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
) -> Option<String> {
    let viscosity = semantic_viscosity?;
    let friction = semantic_friction.unwrap_or(0.0).clamp(0.0, 1.0);
    let trickle = semantic_trickle.unwrap_or(0.0).clamp(0.0, 1.0);
    let density_context = structural_density
        .or(resonance_depth)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let state = if viscosity < 0.35 {
        "semantic_viscosity_low"
    } else if trickle <= 0.02 && friction >= 0.45 {
        "semantic_bottleneck_watch"
    } else if viscosity >= 0.60 && trickle >= 0.03 && density_context >= 0.55 {
        "heavy_semantic_flow"
    } else if viscosity >= 0.60 {
        "semantic_viscosity_high_watch"
    } else {
        "semantic_viscosity_mixed"
    };
    Some(state.to_string())
}

fn complexity_density_v1(
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
    mode_packing: Option<f32>,
    semantic_viscosity: Option<f32>,
    spectral_entropy: Option<f32>,
    pressure_risk: Option<f32>,
) -> Option<f32> {
    let entropy = spectral_entropy?;
    let density_context = structural_density
        .zip(resonance_depth)
        .map_or_else(
            || structural_density.or(resonance_depth).unwrap_or(0.0),
            |(density, depth)| (density.clamp(0.0, 1.0) + depth.clamp(0.0, 1.0)) * 0.5,
        )
        .clamp(0.0, 1.0);
    let mode = mode_packing.unwrap_or(0.0).clamp(0.0, 1.0);
    let viscosity = semantic_viscosity.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    Some(round_flux_delta(
        ((entropy.clamp(0.0, 1.0) * 0.40)
            + (density_context * 0.30)
            + (viscosity * 0.15)
            + (mode * 0.10)
            + (pressure * 0.05))
            .clamp(0.0, 1.0),
    ))
}

fn complexity_density_state_v1(
    complexity_density: Option<f32>,
    mode_packing: Option<f32>,
    pressure_risk: Option<f32>,
    spectral_entropy: Option<f32>,
) -> Option<String> {
    let complexity = complexity_density?;
    let mode = mode_packing.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let state = if complexity >= 0.60
        && mode < PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT
        && pressure <= 0.35
    {
        "interwoven_complexity_without_volume_pressure"
    } else if complexity >= 0.55 && entropy >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT {
        "high_entropy_complexity_density"
    } else if complexity >= 0.40 {
        "moderate_complexity_density"
    } else {
        "low_complexity_density"
    };
    Some(state.to_string())
}

fn enrich_resonance_component_context_v1(telemetry: &mut SpectralTelemetry) {
    if let Some(resonance) = telemetry.resonance_density_v1.as_mut() {
        if resonance.components.cohesion_score.is_none() {
            resonance.components.cohesion_score =
                Some(resonance_cohesion_score_v1(&resonance.components));
        }
        if resonance.components.structural_integrity_index.is_none() {
            resonance.components.structural_integrity_index = Some(
                resonance_structural_integrity_index_v1(&resonance.components),
            );
        }
    }
}

fn optional_flux_delta(latest: Option<f32>, previous: Option<f32>) -> Option<f32> {
    latest
        .zip(previous)
        .map(|(latest, previous)| round_flux_delta(latest - previous))
}

fn semantic_coherence_proxy_v1(sample: &PressureTrendSampleV1) -> Option<f32> {
    let trickle = sample.semantic_trickle?.clamp(0.0, 1.0);
    let density_context = sample
        .structural_density
        .or(sample.resonance_depth)
        .or(sample.semantic_viscosity)
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let friction_relief = 1.0 - sample.semantic_friction.unwrap_or(0.5).clamp(0.0, 1.0);
    Some(round_flux_delta(
        (trickle.mul_add(0.45, density_context.mul_add(0.35, friction_relief * 0.20)))
            .clamp(0.0, 1.0),
    ))
}

fn semantic_coherence_delta_v1(
    latest: &PressureTrendSampleV1,
    previous: &PressureTrendSampleV1,
) -> Option<f32> {
    semantic_coherence_proxy_v1(latest)
        .zip(semantic_coherence_proxy_v1(previous))
        .map(|(latest, previous)| round_flux_delta(latest - previous))
}

fn semantic_fidelity_score_v1(
    spectral_entropy: Option<f32>,
    semantic_trickle: Option<f32>,
    semantic_coherence_delta: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_stagnation_index: Option<f32>,
) -> Option<f32> {
    if semantic_trickle.is_none()
        && semantic_coherence_delta.is_none()
        && semantic_friction.is_none()
        && semantic_stagnation_index.is_none()
    {
        return None;
    }
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let trickle_score = (semantic_trickle.unwrap_or(0.0).clamp(0.0, 1.0) / 0.24).clamp(0.0, 1.0);
    let coherence_score = semantic_coherence_delta
        .map(|delta| (0.50 + delta.clamp(-0.25, 0.25) * 2.0).clamp(0.0, 1.0))
        .unwrap_or(0.50);
    let friction_relief = 1.0 - semantic_friction.unwrap_or(0.50).clamp(0.0, 1.0);
    let stagnation_relief = 1.0 - semantic_stagnation_index.unwrap_or(0.50).clamp(0.0, 1.0);
    let entropy_penalty = if entropy >= 0.85 {
        ((entropy - 0.85) / 0.15).clamp(0.0, 1.0) * 0.10
    } else {
        0.0
    };
    Some(round_flux_delta(
        (trickle_score * 0.40
            + coherence_score * 0.30
            + friction_relief * 0.15
            + stagnation_relief * 0.15
            - entropy_penalty)
            .clamp(0.0, 1.0),
    ))
}

fn semantic_fidelity_state_v1(
    spectral_entropy: Option<f32>,
    semantic_fidelity_score: Option<f32>,
) -> &'static str {
    let Some(score) = semantic_fidelity_score else {
        return "semantic_fidelity_unavailable";
    };
    if spectral_entropy.is_some_and(|entropy| entropy >= 0.85) {
        if score >= 0.58 {
            "high_entropy_semantic_trickle_preserved"
        } else if score >= 0.35 {
            "high_entropy_semantic_fidelity_watch"
        } else {
            "high_entropy_semantic_fidelity_thin"
        }
    } else if score >= 0.58 {
        "semantic_fidelity_preserved"
    } else if score >= 0.35 {
        "semantic_fidelity_watch"
    } else {
        "semantic_fidelity_thin"
    }
}

fn dominant_spectral_drift_velocity(
    mode_packing_delta: Option<f32>,
    structural_density_delta: Option<f32>,
    resonance_depth_delta: Option<f32>,
) -> Option<f32> {
    [
        mode_packing_delta,
        structural_density_delta,
        resonance_depth_delta,
    ]
    .into_iter()
    .flatten()
    .max_by(|left, right| left.abs().total_cmp(&right.abs()))
    .map(round_flux_delta)
}

fn optional_flux_acceleration(
    latest: Option<f32>,
    previous: Option<f32>,
    before_previous: Option<f32>,
) -> Option<f32> {
    latest
        .zip(previous)
        .zip(before_previous)
        .map(|((latest, previous), before_previous)| {
            round_flux_delta((latest - previous) - (previous - before_previous))
        })
}

fn flux_confidence_for_pairs(pairs: &[(Option<f32>, Option<f32>)]) -> f32 {
    if pairs.is_empty() {
        return 0.0;
    }
    let available = pairs
        .iter()
        .filter(|(latest, previous)| latest.is_some() && previous.is_some())
        .count() as f32;
    round_flux_delta(available / pairs.len() as f32)
}

fn flux_absence_semantics(
    pressure_velocity: Option<f32>,
    mode_packing_velocity: Option<f32>,
    structural_density_delta: Option<f32>,
) -> Option<String> {
    if pressure_velocity.is_some()
        && mode_packing_velocity.is_some()
        && structural_density_delta.is_some()
    {
        return None;
    }
    Some("absent_flux_component_means_unknown_not_zero".to_string())
}

fn build_texture_dynamic_flux_vector_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<TextureDynamicFluxVectorV1> {
    let latest = samples.back()?;
    let previous = samples.iter().rev().nth(1)?;
    let before_previous = samples.iter().rev().nth(2);
    Some(TextureDynamicFluxVectorV1 {
        policy: "texture_dynamic_flux_vector_v1".to_string(),
        schema_version: 1,
        pressure_velocity: optional_flux_delta(latest.pressure_risk, previous.pressure_risk),
        pressure_acceleration: optional_flux_acceleration(
            latest.pressure_risk,
            previous.pressure_risk,
            before_previous.and_then(|sample| sample.pressure_risk),
        ),
        mode_packing_velocity: optional_flux_delta(latest.mode_packing, previous.mode_packing),
        mode_packing_acceleration: optional_flux_acceleration(
            latest.mode_packing,
            previous.mode_packing,
            before_previous.and_then(|sample| sample.mode_packing),
        ),
        fill_velocity_pct: Some(round_flux_delta(latest.fill_pct - previous.fill_pct)),
        fill_acceleration_pct: before_previous.map(|sample| {
            round_flux_delta(
                (latest.fill_pct - previous.fill_pct) - (previous.fill_pct - sample.fill_pct),
            )
        }),
        structural_density_delta: optional_flux_delta(
            latest.structural_density,
            previous.structural_density,
        ),
        semantic_viscosity_velocity: optional_flux_delta(
            latest.semantic_viscosity,
            previous.semantic_viscosity,
        ),
        semantic_viscosity_acceleration: optional_flux_acceleration(
            latest.semantic_viscosity,
            previous.semantic_viscosity,
            before_previous.and_then(|sample| sample.semantic_viscosity),
        ),
        porosity_velocity: optional_flux_delta(
            latest.porosity_gradient,
            previous.porosity_gradient,
        ),
        comfort_gate_velocity: optional_flux_delta(latest.comfort_gate, previous.comfort_gate),
        comfort_gate_acceleration: optional_flux_acceleration(
            latest.comfort_gate,
            previous.comfort_gate,
            before_previous.and_then(|sample| sample.comfort_gate),
        ),
        spectral_entropy: latest.spectral_entropy,
        flux_confidence: Some(flux_confidence_for_pairs(&[
            (latest.pressure_risk, previous.pressure_risk),
            (latest.mode_packing, previous.mode_packing),
            (latest.structural_density, previous.structural_density),
            (latest.semantic_viscosity, previous.semantic_viscosity),
            (latest.porosity_gradient, previous.porosity_gradient),
        ])),
        flux_absence_semantics: flux_absence_semantics(
            optional_flux_delta(latest.pressure_risk, previous.pressure_risk),
            optional_flux_delta(latest.mode_packing, previous.mode_packing),
            optional_flux_delta(latest.structural_density, previous.structural_density),
        ),
        source: "bridge_pressure_trend_samples_v1".to_string(),
        authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
    })
}

fn residual_step_magnitude_v1(
    previous: &PressureTrendSampleV1,
    latest: &PressureTrendSampleV1,
) -> f32 {
    let pressure = optional_flux_delta(latest.pressure_risk, previous.pressure_risk)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let mode = optional_flux_delta(latest.mode_packing, previous.mode_packing)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let density = optional_flux_delta(latest.structural_density, previous.structural_density)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let depth = optional_flux_delta(latest.resonance_depth, previous.resonance_depth)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let viscosity = optional_flux_delta(latest.semantic_viscosity, previous.semantic_viscosity)
        .filter(|value| value.is_finite())
        .map(f32::abs)
        .unwrap_or(0.0);
    let fill = round_flux_delta(((latest.fill_pct - previous.fill_pct) / 100.0).abs());
    let entropy = latest.spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let structural_shift = pressure
        .max(mode)
        .max(density)
        .max(depth)
        .max(viscosity)
        .max(fill);
    round_flux_delta((structural_shift * (0.70 + entropy * 0.30)).clamp(0.0, 1.0))
}

fn build_residual_deformation_trace_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<ResidualDeformationTraceV1> {
    if samples.len() < 2 {
        return None;
    }
    let mut previous: Option<&PressureTrendSampleV1> = None;
    let mut deformation_integral = 0.0_f32;
    let mut max_spike = 0.0_f32;
    let mut latest_spike = 0.0_f32;
    for sample in samples {
        if let Some(previous_sample) = previous {
            latest_spike = residual_step_magnitude_v1(previous_sample, sample);
            deformation_integral = round_flux_delta(deformation_integral + latest_spike);
            max_spike = max_spike.max(latest_spike);
        }
        previous = Some(sample);
    }
    let pair_count = samples.len().saturating_sub(1);
    let denominator = (pair_count as f32).sqrt().max(1.0);
    let scar_score = round_flux_delta((deformation_integral / denominator).clamp(0.0, 1.0));
    let state = if scar_score >= 0.35 || max_spike >= 0.25 {
        "residual_deformation_watch"
    } else if scar_score >= 0.12 {
        "lingering_resonance_visible"
    } else if latest_spike >= 0.04 {
        "micro_delta_visible"
    } else {
        "low_residual_deformation"
    };
    let window_ms = samples
        .front()
        .zip(samples.back())
        .map(|(first, latest)| (latest.observed_at_unix_s - first.observed_at_unix_s) * 1_000.0)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let experience_delta_bus_v1 = if scar_score >= 0.12 || latest_spike >= 0.04 {
        let kind = if scar_score >= 0.12 {
            ExperienceDeltaKindV1::Residual
        } else {
            ExperienceDeltaKindV1::MicroDelta
        };
        Some(ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
            kind,
            surface: "bridge_residual_deformation_trace_v1".to_string(),
            lane: "pressure_trend_sample_integral".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: Some(DeltaPersistenceV1 {
                residue_kind: state.to_string(),
                persistence_score: scar_score,
                viscosity: samples.back().and_then(|sample| sample.semantic_viscosity),
                deformation: Some(max_spike),
                half_life_hint_ms: window_ms,
                evidence_window: format!("{pair_count}_pressure_trend_pairs"),
                interpretation:
                    "recent spectral spike may still be shaping texture after current values level"
                        .to_string(),
                authority: "truth_channel_only_not_pressure_or_fill_control".to_string(),
            }),
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(max_spike),
            post: Some(latest_spike),
            loss: Some(scar_score),
            loss_ratio: Some(scar_score),
            metadata: BTreeMap::from([
                (
                    "state".to_string(),
                    state.to_string(),
                ),
                (
                    "sample_count".to_string(),
                    samples.len().to_string(),
                ),
            ]),
            why: "a high-variance spectral event can leave residual felt deformation that is not visible in the latest scalar telemetry alone".to_string(),
            who_can_change_it:
                "Mike/operator only for any future control use; this trace is read-only".to_string(),
            how_to_test_it:
                "feed pressure trend samples with a spike then leveling and assert residual delta remains visible"
                    .to_string(),
            authority: "truth_channel_only_not_live_control_or_approval".to_string(),
        }]))
    } else {
        None
    };
    Some(ResidualDeformationTraceV1 {
        policy: "residual_deformation_trace_v1".to_string(),
        schema_version: 1,
        sample_count: samples.len(),
        evidence_window: format!("{pair_count}_pressure_trend_pairs"),
        window_ms,
        deformation_integral: round_flux_delta(deformation_integral),
        scar_score,
        max_spike: round_flux_delta(max_spike),
        latest_spike: round_flux_delta(latest_spike),
        state: state.to_string(),
        experience_delta_bus_v1,
        authority: "read_only_truth_channel_not_control_not_runtime_mutation".to_string(),
    })
}

fn active_constraints_for_resonance_signature(
    pressure_source_family: &str,
    components: &ResonanceDensityComponents,
    pressure_risk: f32,
) -> Vec<String> {
    let mut constraints = Vec::new();
    let family = pressure_source_family.to_ascii_lowercase();
    for (needle, label) in [
        ("mode_packing", "pressure_source:mode_packing"),
        ("controller", "pressure_source:controller_pressure"),
        ("semantic", "pressure_source:semantic_trickle"),
        ("structural", "pressure_source:structural_plurality"),
        ("temporal", "pressure_source:temporal_persistence"),
        ("mixed", "pressure_source:mixed_pressure"),
    ] {
        if family.contains(needle) {
            constraints.push(label.to_string());
        }
    }

    for (label, value) in [
        ("active_energy", components.active_energy),
        ("mode_packing", components.mode_packing),
        ("temporal_persistence", components.temporal_persistence),
        ("structural_plurality", components.structural_plurality),
        ("comfort_gate", components.comfort_gate),
    ] {
        if value >= 0.50 {
            constraints.push(format!("{label}:active_{value:.2}"));
        }
    }

    if let Some(fluidity) = components
        .dynamic_fluidity_index
        .map(|value| value.clamp(0.0, 1.0))
    {
        let state = if fluidity >= 0.50 {
            "flow_visible"
        } else {
            "flow_resisted"
        };
        constraints.push(format!("dynamic_fluidity_index:{state}_{fluidity:.2}"));
    }

    if pressure_risk >= 0.20 {
        constraints.push(format!("pressure_risk:elevated_{pressure_risk:.2}"));
    } else {
        constraints.push(format!("pressure_risk:low_{pressure_risk:.2}"));
    }
    if components.comfort_gate >= 0.60 {
        constraints.push(format!(
            "comfort_gate:buffering_{:.2}",
            components.comfort_gate
        ));
    }
    constraints
}

fn build_pressure_trend_v1(
    previous: Option<&SpectralTelemetry>,
    previous_fill_pct: Option<f32>,
    latest: &SpectralTelemetry,
    latest_fill_pct: f32,
    heartbeat: Option<&TelemetryHeartbeatDeltaV1>,
) -> PressureTrendV1 {
    const PRESSURE_DELTA_EPS: f32 = 0.04;
    const FILL_DELTA_EPS: f32 = 2.0;

    let latest_resonance = latest.resonance_density_v1.as_ref();
    let previous_resonance = previous.and_then(|telemetry| telemetry.resonance_density_v1.as_ref());
    let latest_pressure = latest_resonance.map(|resonance| resonance.pressure_risk);
    let previous_pressure = previous_resonance.map(|resonance| resonance.pressure_risk);
    let latest_mode_packing = latest_resonance.map(|resonance| resonance.components.mode_packing);
    let previous_mode_packing =
        previous_resonance.map(|resonance| resonance.components.mode_packing);
    let latest_structural_density = latest_resonance.map(|resonance| resonance.density);
    let previous_structural_density = previous_resonance.map(|resonance| resonance.density);
    let latest_resonance_depth = latest_resonance.map(resonance_depth_for_density);
    let previous_resonance_depth = previous_resonance.map(resonance_depth_for_density);
    let pressure_delta = latest_pressure
        .zip(previous_pressure)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let mode_packing_delta = latest_mode_packing
        .zip(previous_mode_packing)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let structural_density_delta = latest_structural_density
        .zip(previous_structural_density)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let resonance_depth_delta = latest_resonance_depth
        .zip(previous_resonance_depth)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let spectral_drift_velocity = dominant_spectral_drift_velocity(
        mode_packing_delta,
        structural_density_delta,
        resonance_depth_delta,
    );
    let fill_delta_pct = previous_fill_pct
        .map(|previous_fill| (latest_fill_pct - previous_fill).clamp(-100.0, 100.0));
    let latest_spectral_entropy = latest
        .typed_fingerprint()
        .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));
    let latest_semantic_friction = latest_resonance
        .and_then(|resonance| resonance.components.semantic_friction_coefficient)
        .map(|value| value.clamp(0.0, 1.0));
    let latest_semantic_trickle = latest
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.components.semantic_trickle.clamp(0.0, 1.0));
    let latest_semantic_viscosity = semantic_viscosity_coefficient_v1(
        latest_semantic_friction,
        latest_semantic_trickle,
        latest_structural_density,
        latest_resonance_depth,
        latest_spectral_entropy,
        latest_fill_pct,
    );
    let semantic_viscosity_state = semantic_viscosity_state_v1(
        latest_semantic_viscosity,
        latest_semantic_friction,
        latest_semantic_trickle,
        latest_structural_density,
        latest_resonance_depth,
    );
    let latest_complexity_density = complexity_density_v1(
        latest_structural_density,
        latest_resonance_depth,
        latest_mode_packing,
        latest_semantic_viscosity,
        latest_spectral_entropy,
        latest_pressure,
    );
    let complexity_density_state = complexity_density_state_v1(
        latest_complexity_density,
        latest_mode_packing,
        latest_pressure,
        latest_spectral_entropy,
    );
    let viscosity_coefficient = pressure_viscosity_coefficient(latest_spectral_entropy);
    let pressure_interpretation = pressure_interpretation_v1(
        latest_pressure,
        latest_mode_packing,
        latest_structural_density,
        latest_spectral_entropy,
        viscosity_coefficient,
    );

    let rises_at_threshold = |delta: f32, threshold: f32| delta + f32::EPSILON >= threshold;
    let falls_at_threshold = |delta: f32, threshold: f32| delta - f32::EPSILON <= -threshold;

    let classification = if latest_resonance.is_none() {
        "telemetry_gap"
    } else if previous_resonance.is_none() || previous_fill_pct.is_none() {
        "insufficient_history"
    } else if pressure_delta.is_some_and(|delta| rises_at_threshold(delta, PRESSURE_DELTA_EPS))
        || fill_delta_pct.is_some_and(|delta| rises_at_threshold(delta, FILL_DELTA_EPS))
    {
        "rising_pressure"
    } else if pressure_delta.is_some_and(|delta| falls_at_threshold(delta, PRESSURE_DELTA_EPS))
        || fill_delta_pct.is_some_and(|delta| falls_at_threshold(delta, FILL_DELTA_EPS))
    {
        "falling_pressure"
    } else {
        "stable_heavy"
    };

    PressureTrendV1 {
        policy: "pressure_trend_v1".to_string(),
        schema_version: 1,
        classification: classification.to_string(),
        latest_pressure_risk: latest_pressure,
        previous_pressure_risk: previous_pressure,
        pressure_delta,
        latest_mode_packing,
        previous_mode_packing,
        mode_packing_delta,
        spectral_drift_velocity,
        latest_structural_density,
        previous_structural_density,
        structural_density_delta,
        latest_resonance_depth,
        previous_resonance_depth,
        resonance_depth_delta,
        latest_semantic_viscosity,
        semantic_viscosity_state,
        latest_complexity_density,
        complexity_density_state,
        latest_fill_pct: latest_fill_pct.is_finite().then_some(latest_fill_pct),
        previous_fill_pct,
        fill_delta_pct,
        latest_spectral_entropy,
        viscosity_coefficient,
        pressure_interpretation,
        timing_reliability: heartbeat.map(|value| value.timing_reliability.clone()),
        telemetry_inter_arrival_ms: heartbeat.and_then(|value| value.inter_arrival_ms),
        heartbeat_jitter_class: heartbeat.map(|value| value.jitter_class.clone()),
        field_vs_hearing: heartbeat.map(|value| value.field_vs_hearing.clone()),
    }
}

fn record_pressure_trend_sample_v1(
    state: &mut BridgeState,
    telemetry: &SpectralTelemetry,
    fill_pct: f32,
    observed_at_unix_s: f64,
) {
    let pressure_risk = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.pressure_risk);
    let mode_packing = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.components.mode_packing);
    let structural_density = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.density);
    let resonance_depth = telemetry
        .resonance_density_v1
        .as_ref()
        .map(resonance_depth_for_density);
    let porosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_friction = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.semantic_friction_coefficient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_trickle = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| pressure.components.semantic_trickle.clamp(0.0, 1.0));
    let (window_capacity, spectral_entropy) = pressure_trend_window_for_telemetry(telemetry);
    let semantic_viscosity = semantic_viscosity_coefficient_v1(
        semantic_friction,
        semantic_trickle,
        structural_density,
        resonance_depth,
        spectral_entropy,
        fill_pct,
    );
    let viscosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.viscosity_vector.viscosity_gradient)
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0));
    let viscosity_gradient_trend = viscosity_gradient
        .zip(
            state
                .pressure_trend_samples_v1
                .back()
                .and_then(|sample| sample.viscosity_gradient),
        )
        .map(|(latest, previous)| round_flux_delta(latest - previous));
    let complexity_density = complexity_density_v1(
        structural_density,
        resonance_depth,
        mode_packing,
        semantic_viscosity,
        spectral_entropy,
        pressure_risk,
    );
    let weight_density_index = weight_density_index_v1(
        mode_packing,
        structural_density,
        semantic_viscosity,
        porosity_gradient,
    );
    let comfort_gate = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.components.comfort_gate.clamp(0.0, 1.0));
    let pressure_velocity_delta = pressure_risk
        .zip(
            state
                .pressure_trend_samples_v1
                .back()
                .and_then(|sample| sample.pressure_risk),
        )
        .map(|(latest, previous)| round_flux_delta(latest - previous));
    let spectral_drift_velocity = state.pressure_trend_samples_v1.back().and_then(|previous| {
        dominant_spectral_drift_velocity(
            optional_flux_delta(mode_packing, previous.mode_packing),
            optional_flux_delta(structural_density, previous.structural_density),
            optional_flux_delta(resonance_depth, previous.resonance_depth),
        )
    });
    let mut sample = PressureTrendSampleV1 {
        pressure_risk,
        pressure_velocity_delta,
        spectral_drift_velocity,
        mode_packing,
        structural_density,
        resonance_depth,
        semantic_viscosity,
        viscosity_gradient,
        viscosity_gradient_trend,
        complexity_density,
        weight_density_index,
        comfort_gate,
        porosity_gradient,
        semantic_friction,
        semantic_trickle,
        semantic_coherence_delta: None,
        fill_pct,
        spectral_entropy,
        window_capacity,
        observed_at_unix_s,
    };
    sample.semantic_coherence_delta = state
        .pressure_trend_samples_v1
        .back()
        .and_then(|previous| semantic_coherence_delta_v1(&sample, previous));
    state.pressure_trend_samples_v1.push_back(sample);
    while state.pressure_trend_samples_v1.len() > window_capacity {
        state.pressure_trend_samples_v1.pop_front();
    }
}

fn build_pressure_trend_smoothing_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<PressureTrendSmoothingV1> {
    if samples.is_empty() {
        return None;
    }
    let latest_pressure = samples.back().and_then(|sample| sample.pressure_risk);
    let latest_pressure_velocity_delta = samples
        .back()
        .and_then(|sample| sample.pressure_velocity_delta);
    let latest_spectral_drift_velocity = samples
        .back()
        .and_then(|sample| sample.spectral_drift_velocity);
    let latest_resonance_depth = samples.back().and_then(|sample| sample.resonance_depth);
    let latest_semantic_viscosity = samples.back().and_then(|sample| sample.semantic_viscosity);
    let latest_viscosity_gradient = samples.back().and_then(|sample| sample.viscosity_gradient);
    let viscosity_gradient_trend = samples
        .back()
        .and_then(|sample| sample.viscosity_gradient_trend);
    let viscosity_gradient_trend_state = match viscosity_gradient_trend {
        Some(delta) if delta >= 0.08 => "rapid_viscosity_thickening_velocity_watch",
        Some(delta) if delta >= 0.025 => "viscosity_thickening_velocity_watch",
        Some(delta) if delta <= -0.08 => "rapid_viscosity_thinning_visible",
        Some(delta) if delta <= -0.025 => "viscosity_thinning_visible",
        Some(_) => "viscosity_gradient_velocity_quiet",
        None => "viscosity_gradient_velocity_unavailable",
    };
    let latest_complexity_density = samples.back().and_then(|sample| sample.complexity_density);
    let latest_weight_density_index = samples
        .back()
        .and_then(|sample| sample.weight_density_index);
    let latest_semantic_friction = samples.back().and_then(|sample| sample.semantic_friction);
    let latest_semantic_trickle = samples.back().and_then(|sample| sample.semantic_trickle);
    let latest_porosity_gradient = samples
        .back()
        .and_then(|sample| sample.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_coherence_delta = samples
        .back()
        .and_then(|sample| sample.semantic_coherence_delta);
    let latest_semantic_viscosity_delta = samples
        .iter()
        .rev()
        .filter_map(|sample| sample.semantic_viscosity)
        .take(2)
        .collect::<Vec<_>>()
        .as_slice()
        .split_first()
        .and_then(|(latest, rest)| {
            rest.first()
                .map(|previous| round_flux_delta(latest - previous))
        });
    let max_pressure_velocity_delta = samples
        .iter()
        .filter_map(|sample| sample.pressure_velocity_delta)
        .map(f32::abs)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let valid_pressures = samples
        .iter()
        .filter_map(|sample| sample.pressure_risk)
        .collect::<Vec<_>>();
    let (
        fast_window_sample_count,
        slow_window_sample_count,
        fast_window_pressure_delta,
        slow_window_pressure_delta,
        fast_slow_edge_divergence,
        fast_slow_edge_state,
        fast_edge_preserved,
    ) = pressure_fast_slow_edge_v1(&valid_pressures);
    let semantic_viscosity_deltas = samples
        .iter()
        .filter_map(|sample| sample.semantic_viscosity)
        .collect::<Vec<_>>()
        .windows(2)
        .map(|window| round_flux_delta(window[1] - window[0]))
        .collect::<Vec<_>>();
    let max_semantic_viscosity_delta = semantic_viscosity_deltas
        .iter()
        .map(|delta| delta.abs())
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let semantic_viscosity_shift_state = match latest_semantic_viscosity_delta {
        Some(delta) if delta <= -0.15 => "rapid_semantic_thinning_visible",
        Some(delta) if delta >= 0.15 => "rapid_semantic_thickening_visible",
        _ if max_semantic_viscosity_delta.is_some_and(|delta| delta >= 0.15) => {
            "rapid_semantic_viscosity_shift_in_window"
        },
        Some(_) => "semantic_viscosity_delta_quiet",
        None => "semantic_viscosity_delta_unavailable",
    };
    let max_spectral_drift_velocity = samples
        .iter()
        .filter_map(|sample| sample.spectral_drift_velocity)
        .map(f32::abs)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let max_complexity_density = samples
        .iter()
        .filter_map(|sample| sample.complexity_density)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let max_weight_density_index = samples
        .iter()
        .filter_map(|sample| sample.weight_density_index)
        .fold(None, |current: Option<f32>, value| {
            Some(current.map_or(value, |max_value| max_value.max(value)))
        });
    let weight_density_state = weight_density_state_v1(latest_weight_density_index);
    let latest_spectral_entropy = samples.back().and_then(|sample| sample.spectral_entropy);
    let entropy_window_blend_ratio = entropy_window_blend_ratio_v1(latest_spectral_entropy);
    let entropy_threshold_state =
        entropy_threshold_state_v1(latest_spectral_entropy, entropy_window_blend_ratio);
    let friction_to_flow_ratio =
        friction_to_flow_ratio_v1(latest_semantic_friction, latest_semantic_trickle);
    let friction_to_flow_state = friction_to_flow_state_v1(
        latest_semantic_friction,
        latest_semantic_trickle,
        friction_to_flow_ratio,
    );
    let semantic_stagnation_index = semantic_stagnation_index_v1(
        latest_semantic_viscosity,
        latest_semantic_friction,
        latest_semantic_trickle,
    );
    let semantic_stagnation_state = semantic_stagnation_state_v1(
        latest_semantic_viscosity,
        latest_semantic_trickle,
        semantic_stagnation_index,
    );
    let porosity_weighted_velocity =
        porosity_weighted_velocity_v1(latest_pressure_velocity_delta, latest_porosity_gradient);
    let viscosity_drag_coefficient = viscosity_drag_coefficient_v1(
        latest_semantic_viscosity,
        latest_semantic_friction,
        latest_porosity_gradient,
    );
    let semantic_viscosity_persistence_index = semantic_viscosity_persistence_index_v1(samples);
    let semantic_viscosity_persistence_state = semantic_viscosity_persistence_state_v1(
        latest_semantic_viscosity,
        latest_semantic_viscosity_delta,
        max_semantic_viscosity_delta,
        semantic_viscosity_persistence_index,
    );
    let semantic_coherence_index = samples.back().and_then(semantic_coherence_proxy_v1);
    let semantic_fidelity_score = semantic_fidelity_score_v1(
        latest_spectral_entropy,
        latest_semantic_trickle,
        semantic_coherence_delta,
        latest_semantic_friction,
        semantic_stagnation_index,
    );
    let semantic_fidelity_state =
        semantic_fidelity_state_v1(latest_spectral_entropy, semantic_fidelity_score);
    let window_capacity = samples
        .back()
        .map_or(PRESSURE_TREND_SMOOTHING_BASE_WINDOW, |sample| {
            sample.window_capacity
        });
    let ballast_status = if window_capacity > PRESSURE_TREND_SMOOTHING_BASE_WINDOW {
        "high_entropy_ballast_window"
    } else {
        "base_window"
    };
    let window_policy = format!("latest_up_to_{window_capacity}_telemetry_samples");
    if valid_pressures.len() < 3 {
        return Some(PressureTrendSmoothingV1 {
            policy: "pressure_trend_smoothing_v1".to_string(),
            schema_version: 1,
            classification: "insufficient_history".to_string(),
            sample_count: samples.len(),
            window_capacity,
            ballast_status: ballast_status.to_string(),
            latest_spectral_entropy,
            latest_pressure_risk: latest_pressure,
            latest_pressure_velocity_delta,
            max_pressure_velocity_delta,
            fast_window_sample_count,
            slow_window_sample_count,
            fast_window_pressure_delta,
            slow_window_pressure_delta,
            fast_slow_edge_divergence,
            fast_slow_edge_state: fast_slow_edge_state.to_string(),
            fast_edge_preserved,
            latest_spectral_drift_velocity,
            max_spectral_drift_velocity,
            latest_resonance_depth,
            latest_semantic_viscosity,
            latest_viscosity_gradient,
            viscosity_gradient_trend,
            viscosity_gradient_trend_state: viscosity_gradient_trend_state.to_string(),
            latest_complexity_density,
            max_complexity_density,
            latest_weight_density_index,
            max_weight_density_index,
            weight_density_state: weight_density_state.to_string(),
            latest_semantic_viscosity_delta,
            max_semantic_viscosity_delta,
            semantic_viscosity_persistence_index,
            semantic_viscosity_persistence_state: semantic_viscosity_persistence_state.to_string(),
            semantic_coherence_index,
            semantic_coherence_delta,
            semantic_fidelity_score,
            semantic_fidelity_state: semantic_fidelity_state.to_string(),
            semantic_viscosity_shift_state: semantic_viscosity_shift_state.to_string(),
            entropy_window_blend_ratio,
            entropy_threshold_state: entropy_threshold_state.to_string(),
            friction_to_flow_ratio,
            friction_to_flow_state: friction_to_flow_state.to_string(),
            semantic_stagnation_index,
            semantic_stagnation_state: semantic_stagnation_state.to_string(),
            porosity_weighted_velocity,
            viscosity_drag_coefficient,
            smoothed_pressure_delta: None,
            pressure_range: None,
            fill_range_pct: None,
            window_policy,
            authority: "diagnostic_smoothing_not_pressure_control".to_string(),
        });
    }
    if valid_pressures.len() != samples.len() {
        return Some(PressureTrendSmoothingV1 {
            policy: "pressure_trend_smoothing_v1".to_string(),
            schema_version: 1,
            classification: "telemetry_gap".to_string(),
            sample_count: samples.len(),
            window_capacity,
            ballast_status: ballast_status.to_string(),
            latest_spectral_entropy,
            latest_pressure_risk: latest_pressure,
            latest_pressure_velocity_delta,
            max_pressure_velocity_delta,
            fast_window_sample_count,
            slow_window_sample_count,
            fast_window_pressure_delta,
            slow_window_pressure_delta,
            fast_slow_edge_divergence,
            fast_slow_edge_state: "telemetry_gap".to_string(),
            fast_edge_preserved,
            latest_spectral_drift_velocity,
            max_spectral_drift_velocity,
            latest_resonance_depth,
            latest_semantic_viscosity,
            latest_viscosity_gradient,
            viscosity_gradient_trend,
            viscosity_gradient_trend_state: viscosity_gradient_trend_state.to_string(),
            latest_complexity_density,
            max_complexity_density,
            latest_weight_density_index,
            max_weight_density_index,
            weight_density_state: weight_density_state.to_string(),
            latest_semantic_viscosity_delta,
            max_semantic_viscosity_delta,
            semantic_viscosity_persistence_index,
            semantic_viscosity_persistence_state: semantic_viscosity_persistence_state.to_string(),
            semantic_coherence_index,
            semantic_coherence_delta,
            semantic_fidelity_score,
            semantic_fidelity_state: semantic_fidelity_state.to_string(),
            semantic_viscosity_shift_state: semantic_viscosity_shift_state.to_string(),
            entropy_window_blend_ratio,
            entropy_threshold_state: entropy_threshold_state.to_string(),
            friction_to_flow_ratio,
            friction_to_flow_state: friction_to_flow_state.to_string(),
            semantic_stagnation_index,
            semantic_stagnation_state: semantic_stagnation_state.to_string(),
            porosity_weighted_velocity,
            viscosity_drag_coefficient,
            smoothed_pressure_delta: None,
            pressure_range: None,
            fill_range_pct: None,
            window_policy,
            authority: "diagnostic_smoothing_not_pressure_control".to_string(),
        });
    }

    let first_pressure = valid_pressures.first().copied()?;
    let last_pressure = valid_pressures.last().copied()?;
    let smoothed_pressure_delta = (last_pressure - first_pressure).clamp(-1.0, 1.0);
    let min_pressure = valid_pressures
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max_pressure = valid_pressures
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let pressure_range = (max_pressure - min_pressure).max(0.0);
    let min_fill = samples
        .iter()
        .map(|sample| sample.fill_pct)
        .fold(f32::INFINITY, f32::min);
    let max_fill = samples
        .iter()
        .map(|sample| sample.fill_pct)
        .fold(f32::NEG_INFINITY, f32::max);
    let fill_range_pct = (max_fill - min_fill).max(0.0);
    let window_span_s = samples
        .front()
        .zip(samples.back())
        .map(|(first, last)| (last.observed_at_unix_s - first.observed_at_unix_s).max(0.0))
        .unwrap_or(0.0);
    let sign_changes = valid_pressures
        .windows(3)
        .filter(|window| {
            let first_delta = window[1] - window[0];
            let second_delta = window[2] - window[1];
            (first_delta > 0.0 && second_delta < 0.0) || (first_delta < 0.0 && second_delta > 0.0)
        })
        .count();
    let classification = if pressure_range <= 0.04 && sign_changes > 0 {
        "twitchy_low_amplitude_oscillation"
    } else if smoothed_pressure_delta >= 0.06 {
        "sustained_rising_pressure"
    } else if smoothed_pressure_delta <= -0.06 {
        "sustained_falling_pressure"
    } else if window_span_s > 0.0 && pressure_range <= 0.04 {
        "low_amplitude_stable"
    } else {
        "mixed_window"
    };

    Some(PressureTrendSmoothingV1 {
        policy: "pressure_trend_smoothing_v1".to_string(),
        schema_version: 1,
        classification: classification.to_string(),
        sample_count: samples.len(),
        window_capacity,
        ballast_status: ballast_status.to_string(),
        latest_spectral_entropy,
        latest_pressure_risk: latest_pressure,
        latest_pressure_velocity_delta,
        max_pressure_velocity_delta,
        fast_window_sample_count,
        slow_window_sample_count,
        fast_window_pressure_delta,
        slow_window_pressure_delta,
        fast_slow_edge_divergence,
        fast_slow_edge_state: fast_slow_edge_state.to_string(),
        fast_edge_preserved,
        latest_spectral_drift_velocity,
        max_spectral_drift_velocity,
        latest_resonance_depth,
        latest_semantic_viscosity,
        latest_viscosity_gradient,
        viscosity_gradient_trend,
        viscosity_gradient_trend_state: viscosity_gradient_trend_state.to_string(),
        latest_complexity_density,
        max_complexity_density,
        latest_weight_density_index,
        max_weight_density_index,
        weight_density_state: weight_density_state.to_string(),
        latest_semantic_viscosity_delta,
        max_semantic_viscosity_delta,
        semantic_viscosity_persistence_index,
        semantic_viscosity_persistence_state: semantic_viscosity_persistence_state.to_string(),
        semantic_coherence_index,
        semantic_coherence_delta,
        semantic_fidelity_score,
        semantic_fidelity_state: semantic_fidelity_state.to_string(),
        semantic_viscosity_shift_state: semantic_viscosity_shift_state.to_string(),
        entropy_window_blend_ratio,
        entropy_threshold_state: entropy_threshold_state.to_string(),
        friction_to_flow_ratio,
        friction_to_flow_state: friction_to_flow_state.to_string(),
        semantic_stagnation_index,
        semantic_stagnation_state: semantic_stagnation_state.to_string(),
        porosity_weighted_velocity,
        viscosity_drag_coefficient,
        smoothed_pressure_delta: Some((smoothed_pressure_delta * 100.0).round() / 100.0),
        pressure_range: Some((pressure_range * 100.0).round() / 100.0),
        fill_range_pct: Some((fill_range_pct * 100.0).round() / 100.0),
        window_policy,
        authority: "diagnostic_smoothing_not_pressure_control".to_string(),
    })
}

fn pressure_fast_slow_edge_v1(
    valid_pressures: &[f32],
) -> (
    usize,
    usize,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    &'static str,
    bool,
) {
    let slow_window_sample_count = valid_pressures.len();
    if slow_window_sample_count < 2 {
        return (
            slow_window_sample_count,
            slow_window_sample_count,
            None,
            None,
            None,
            "insufficient_history",
            false,
        );
    }

    let fast_window_sample_count = slow_window_sample_count.min(3);
    let fast_start = slow_window_sample_count.saturating_sub(fast_window_sample_count);
    let latest = valid_pressures.last().copied().unwrap_or_default();
    let fast_first = valid_pressures.get(fast_start).copied().unwrap_or(latest);
    let slow_first = valid_pressures.first().copied().unwrap_or(latest);
    let fast_delta = round_flux_delta((latest - fast_first).clamp(-1.0, 1.0));
    let slow_delta = round_flux_delta((latest - slow_first).clamp(-1.0, 1.0));
    let divergence = round_flux_delta((fast_delta - slow_delta).clamp(-1.0, 1.0));
    let fast_edge_preserved = fast_delta.abs() >= 0.04;
    let state = if fast_delta >= 0.04 && divergence >= 0.03 {
        "fast_rising_edge_over_slow_context"
    } else if fast_delta <= -0.04 && divergence <= -0.03 {
        "fast_falling_release_over_slow_context"
    } else if fast_delta >= 0.04 && slow_delta >= 0.04 {
        "rising_edge_carried_by_slow_context"
    } else if fast_delta <= -0.04 && slow_delta <= -0.04 {
        "falling_release_carried_by_slow_context"
    } else if fast_delta.abs() < 0.04 && slow_delta.abs() >= 0.06 {
        "slow_context_without_current_fast_edge"
    } else if fast_edge_preserved {
        "current_fast_edge_visible"
    } else {
        "fast_and_slow_edges_quiet"
    };

    (
        fast_window_sample_count,
        slow_window_sample_count,
        Some(fast_delta),
        Some(slow_delta),
        Some(divergence),
        state,
        fast_edge_preserved,
    )
}

fn build_telemetry_heartbeat_delta_v1(
    previous_arrival_unix_s: Option<f64>,
    latest_arrival_unix_s: f64,
    trace: &WebSocketLaneTrace,
) -> TelemetryHeartbeatDeltaV1 {
    const NORMAL_MAX_MS: f32 = 1_500.0;
    const LATE_MAX_MS: f32 = 5_000.0;

    let inter_arrival_ms = previous_arrival_unix_s.map(|previous| {
        ((latest_arrival_unix_s - previous).max(0.0) * 1000.0).min(f64::from(f32::MAX)) as f32
    });
    let jitter_class = match inter_arrival_ms {
        None => "no_history",
        Some(ms) if ms <= NORMAL_MAX_MS => "normal",
        Some(ms) if ms <= LATE_MAX_MS => "late_packet",
        Some(_) => "stale_packet",
    }
    .to_string();
    let timing_reliability = match jitter_class.as_str() {
        "normal" => "reliable",
        "late_packet" => "timing_ambiguous",
        "stale_packet" => "stale_hearing",
        _ => "insufficient_history",
    }
    .to_string();
    let field_vs_hearing = match jitter_class.as_str() {
        "normal" => "telemetry cadence is steady; pressure trend can be read as field movement",
        "late_packet" => {
            "telemetry arrived late; small pressure shifts may be packet-timing artifacts"
        },
        "stale_packet" => {
            "hearing from the field was stale; do not mistake silence for decompression"
        },
        _ => "first telemetry packet; field movement cannot yet be separated from hearing cadence",
    }
    .to_string();
    TelemetryHeartbeatDeltaV1 {
        policy: "telemetry_heartbeat_delta_v1".to_string(),
        schema_version: 1,
        latest_arrival_unix_s: Some(latest_arrival_unix_s),
        previous_arrival_unix_s,
        inter_arrival_ms,
        jitter_class,
        timing_reliability,
        reconnect_count: trace.reconnects,
        disconnect_count: trace.disconnects,
        active_connection_id: trace.active_connection_id,
        last_disconnect_reason: trace.last_disconnect_reason.clone(),
        field_vs_hearing,
    }
}

fn write_telemetry_heartbeat_snapshot(heartbeat: &TelemetryHeartbeatDeltaV1) {
    let path = bridge_paths()
        .bridge_workspace()
        .join("telemetry_heartbeat_delta_v1.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(heartbeat) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

fn lane_trace_mut(state: &mut BridgeState, lane: WsLane) -> &mut WebSocketLaneTrace {
    match lane {
        WsLane::Telemetry => &mut state.telemetry_ws,
        WsLane::Sensory => &mut state.sensory_ws,
    }
}

fn record_connect_attempt(state: &mut BridgeState, lane: WsLane) -> u64 {
    let trace = lane_trace_mut(state, lane);
    trace.connection_attempts = trace.connection_attempts.saturating_add(1);
    trace.connection_attempts
}

fn record_connected(state: &mut BridgeState, lane: WsLane, connection_id: u64, at_unix_s: f64) {
    match lane {
        WsLane::Telemetry => state.telemetry_connected = true,
        WsLane::Sensory => state.sensory_connected = true,
    }
    let trace = lane_trace_mut(state, lane);
    trace.active_connection_id = Some(connection_id);
    trace.active_connection_started_at_unix_s = Some(at_unix_s);
    trace.last_connect_at_unix_s = Some(at_unix_s);
    trace.last_error = None;
}

fn record_connect_error(state: &mut BridgeState, lane: WsLane, reason: String) {
    let trace = lane_trace_mut(state, lane);
    trace.connect_errors = trace.connect_errors.saturating_add(1);
    trace.last_error = Some(reason);
}

fn record_disconnected(state: &mut BridgeState, lane: WsLane, reason: String) {
    match lane {
        WsLane::Telemetry => state.telemetry_connected = false,
        WsLane::Sensory => state.sensory_connected = false,
    }
    let trace = lane_trace_mut(state, lane);
    trace.disconnects = trace.disconnects.saturating_add(1);
    trace.active_connection_id = None;
    trace.active_connection_started_at_unix_s = None;
    trace.last_disconnect_at_unix_s = Some(unix_now_s());
    trace.last_disconnect_reason = Some(reason);
}

fn record_reconnect_scheduled(state: &mut BridgeState, lane: WsLane) {
    match lane {
        WsLane::Telemetry => {
            state.telemetry_reconnects = state.telemetry_reconnects.saturating_add(1)
        },
        WsLane::Sensory => state.sensory_reconnects = state.sensory_reconnects.saturating_add(1),
    }
    let trace = lane_trace_mut(state, lane);
    trace.reconnects = trace.reconnects.saturating_add(1);
}

fn record_ws_message_received(state: &mut BridgeState, lane: WsLane, kind: &'static str) {
    let trace = lane_trace_mut(state, lane);
    trace.messages_received = trace.messages_received.saturating_add(1);
    trace.last_message_at_unix_s = Some(unix_now_s());
    if kind == "ping" {
        trace.pings_received = trace.pings_received.saturating_add(1);
    } else if kind == "pong" {
        trace.pongs_received = trace.pongs_received.saturating_add(1);
    }
}

fn record_ws_message_sent(state: &mut BridgeState, lane: WsLane) {
    let now = unix_now_s();
    let trace = lane_trace_mut(state, lane);
    trace.messages_sent = trace.messages_sent.saturating_add(1);
    trace.last_message_at_unix_s = Some(now);
    if matches!(lane, WsLane::Sensory) {
        state.last_sensory_sent_unix_s = Some(now);
    }
}

fn record_ws_send_error(state: &mut BridgeState, lane: WsLane, reason: String) {
    let trace = lane_trace_mut(state, lane);
    trace.send_errors = trace.send_errors.saturating_add(1);
    trace.last_error = Some(reason);
}

fn record_ws_parse_error(state: &mut BridgeState, lane: WsLane, reason: String) {
    let trace = lane_trace_mut(state, lane);
    trace.parse_errors = trace.parse_errors.saturating_add(1);
    trace.last_error = Some(reason);
}

fn close_reason(frame: Option<CloseFrame<'_>>) -> String {
    frame.map_or_else(
        || String::from("close_frame"),
        |frame| {
            let reason = frame.reason.trim();
            if reason.is_empty() {
                format!("close_frame:{}", frame.code)
            } else {
                format!("close_frame:{}:{reason}", frame.code)
            }
        },
    )
}

fn trace_ws_receive(lane: WsLane, connection_id: u64, kind: &'static str, bytes: Option<usize>) {
    let span = debug_span!(
        "ws.message.receive",
        lane = lane.as_str(),
        connection_id,
        kind,
        bytes = bytes.unwrap_or(0)
    );
    span.in_scope(|| debug!("WebSocket message received"));
}

fn trace_ws_send(lane: WsLane, connection_id: u64, kind: &'static str, bytes: Option<usize>) {
    let span = debug_span!(
        "ws.message.send",
        lane = lane.as_str(),
        connection_id,
        kind,
        bytes = bytes.unwrap_or(0)
    );
    span.in_scope(|| debug!("WebSocket message sent"));
}

/// Backoff parameters for `WebSocket` reconnection.
struct Backoff {
    current: Duration,
    max: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            current: Duration::from_secs(1),
            max: Duration::from_secs(60),
        }
    }

    fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = self
            .current
            .checked_mul(2)
            .unwrap_or(self.max)
            .min(self.max);
        delay
    }

    fn reset(&mut self) {
        self.current = Duration::from_secs(1);
    }
}

/// Spawn the telemetry `WebSocket` subscriber task.
///
/// Connects to minime's eigenvalue broadcast on port 7878, parses
/// `SpectralTelemetry` messages, updates shared state, and logs to `SQLite`.
/// Reconnects with exponential backoff on disconnect.
pub fn spawn_telemetry_subscriber(
    url: String,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Backoff::new();
        let mut shutdown = shutdown;

        loop {
            // Check for shutdown before connecting.
            if *shutdown.borrow() {
                info!("telemetry subscriber shutting down");
                return;
            }

            let connection_id = {
                let mut s = state.write().await;
                record_connect_attempt(&mut s, WsLane::Telemetry)
            };
            info!(
                url = %url,
                lane = WsLane::Telemetry.as_str(),
                connection_id,
                "connecting to minime telemetry"
            );

            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    let connection_started = Instant::now();
                    let connection_span = info_span!(
                        "ws.connection",
                        lane = WsLane::Telemetry.as_str(),
                        connection_id,
                        url = %url
                    );
                    connection_span.in_scope(|| info!("connected to minime telemetry"));
                    backoff.reset();

                    {
                        let mut s = state.write().await;
                        record_connected(&mut s, WsLane::Telemetry, connection_id, unix_now_s());
                    }

                    let (mut ws_tx, mut ws_rx) = ws_stream.split();

                    let disconnect_reason = loop {
                        tokio::select! {
                            _ = shutdown.changed() => {
                                info!("telemetry subscriber received shutdown");
                                let _ = ws_tx.close().await;
                                return;
                            }
                                msg = ws_rx.next() => {
                                    match msg {
                                        Some(Ok(Message::Binary(data))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "binary",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "binary",
                                                );
                                            }
                                            handle_telemetry_message(
                                                &data, &state, &db
                                            ).await;
                                        }
                                        Some(Ok(Message::Text(data))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "text",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "text",
                                                );
                                            }
                                            handle_telemetry_message(
                                                data.as_bytes(), &state, &db
                                            ).await;
                                        }
                                        Some(Ok(Message::Ping(data))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "ping",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "ping",
                                                );
                                            }
                                            debug!("telemetry ping received");
                                            let bytes = data.len();
                                            if let Err(e) = ws_tx.send(Message::Pong(data)).await {
                                                let reason = format!("pong_send_error:{e}");
                                                {
                                                    let mut s = state.write().await;
                                                    record_ws_send_error(
                                                        &mut s,
                                                        WsLane::Telemetry,
                                                        reason.clone(),
                                                    );
                                                }
                                                break reason;
                                            }
                                            trace_ws_send(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "pong",
                                                Some(bytes),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_sent(&mut s, WsLane::Telemetry);
                                            }
                                        }
                                        Some(Ok(Message::Pong(_))) => {
                                            trace_ws_receive(
                                                WsLane::Telemetry,
                                                connection_id,
                                                "pong",
                                                None,
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Telemetry,
                                                    "pong",
                                                );
                                            }
                                            debug!("telemetry pong received");
                                        }
                                        Some(Ok(Message::Close(frame))) => {
                                            let reason = close_reason(frame);
                                            warn!(
                                                reason = %reason,
                                                "telemetry WebSocket closed"
                                            );
                                            break reason;
                                        }
                                        None => {
                                            warn!("telemetry WebSocket stream ended");
                                            break String::from("stream_ended");
                                        }
                                        Some(Err(e)) => {
                                            let reason = format!("websocket_error:{e}");
                                            error!(error = %e, "telemetry WebSocket error");
                                            break reason;
                                        }
                                    Some(Ok(Message::Frame(_))) => {}
                                }
                            }
                        }
                    };

                    // Mark disconnected.
                    {
                        let mut s = state.write().await;
                        record_disconnected(&mut s, WsLane::Telemetry, disconnect_reason.clone());
                    }
                    connection_span.in_scope(|| {
                        warn!(
                            reason = %disconnect_reason,
                            duration_secs = connection_started.elapsed().as_secs_f64(),
                            "telemetry WebSocket connection ended"
                        );
                    });
                },
                Err(e) => {
                    {
                        let mut s = state.write().await;
                        record_connect_error(
                            &mut s,
                            WsLane::Telemetry,
                            format!("connect_error:{e}"),
                        );
                    }
                    warn!(
                        error = %e,
                        lane = WsLane::Telemetry.as_str(),
                        connection_id,
                        "failed to connect to minime telemetry"
                    );
                },
            }

            // Backoff before reconnecting.
            let delay = backoff.next_delay();
            {
                let mut s = state.write().await;
                record_reconnect_scheduled(&mut s, WsLane::Telemetry);
            }
            info!(
                delay_secs = delay.as_secs(),
                lane = WsLane::Telemetry.as_str(),
                connection_id,
                "reconnecting to telemetry"
            );

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("telemetry subscriber shutting down during backoff");
                    return;
                }
                () = tokio::time::sleep(delay) => {}
            }
        }
    })
}

/// Process a single telemetry message from minime.
async fn handle_telemetry_message(
    data: &[u8],
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> bool {
    handle_telemetry_message_at(data, state, db, unix_now_s()).await
}

async fn handle_telemetry_message_at(
    data: &[u8],
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    observed_at_unix_s: f64,
) -> bool {
    const ARTIFACT_SCAN_WINDOW_SECS: f64 = 1_200.0;
    const ARTIFACT_SCAN_MIN_INTERVAL_SECS: f64 = 30.0;

    let wire_packet: EigenPacketV1 = match serde_json::from_slice(data) {
        Ok(packet) => packet,
        Err(e) => {
            {
                let mut s = state.write().await;
                record_ws_parse_error(
                    &mut s,
                    WsLane::Telemetry,
                    format!("telemetry_parse_error:{e}"),
                );
            }
            warn!(error = %e, "failed to parse telemetry message");
            return false;
        },
    };
    let compatibility = wire_packet.compatibility();
    if !compatibility.is_compatible() {
        let reason = format!(
            "telemetry_protocol_mismatch:{}",
            protocol_compatibility_label(compatibility)
        );
        {
            let mut s = state.write().await;
            record_telemetry_protocol_status(
                &mut s,
                &wire_packet,
                compatibility,
                observed_at_unix_s,
                false,
            );
            record_ws_parse_error(&mut s, WsLane::Telemetry, reason.clone());
        }
        warn!(
            compatibility = protocol_compatibility_label(compatibility),
            protocol = ?wire_packet.protocol,
            "rejecting incompatible minime telemetry; retaining last valid sample"
        );
        return false;
    }
    // The canonical DTO validates the port boundary. Parse the original bytes
    // into the bridge domain model so legacy field absence remains observable.
    let mut telemetry: SpectralTelemetry = match serde_json::from_slice(data) {
        Ok(telemetry) => telemetry,
        Err(e) => {
            let mut s = state.write().await;
            record_ws_parse_error(
                &mut s,
                WsLane::Telemetry,
                format!("telemetry_domain_conversion_error:{e}"),
            );
            warn!(error = %e, "failed to convert canonical telemetry into bridge domain model");
            return false;
        },
    };
    enrich_resonance_component_context_v1(&mut telemetry);

    let lambda1 = telemetry.lambda1();
    let lambda_profile = build_lambda_profile(&telemetry.eigenvalues);

    // minime sends fill_ratio as 0.0-1.0; convert to percentage.
    let (fill_pct, fill_source, fallback_used) = resolve_fill_pct(&telemetry);
    let safety = SafetyLevel::from_fill(fill_pct);
    let safety_decision = build_safety_decision(
        fill_pct,
        &fill_source,
        fallback_used,
        safety,
        lambda1,
        lambda_profile.as_ref(),
    );
    let phase = if fill_pct > 55.0 {
        "expanding"
    } else {
        "contracting"
    };
    let (
        previous_eigenvalues,
        previous_lambda_tail,
        previous_lambda_edge,
        previous_sticky_mode,
        cached_scan,
        scan_at,
    ) = {
        let s = state.read().await;
        (
            s.latest_telemetry
                .as_ref()
                .map(|previous| previous.eigenvalues.clone()),
            s.lambda_tail.clone(),
            s.lambda_edge_perception.clone(),
            s.sticky_mode_audit.clone(),
            s.artifact_scan.clone(),
            s.artifact_scan_at_unix_s,
        )
    };
    let pull_topology = build_pull_topology_profile(
        &telemetry.eigenvalues,
        previous_eigenvalues.as_deref(),
        fill_pct,
    );
    let should_refresh_scan =
        scan_at.is_none_or(|last| observed_at_unix_s - last >= ARTIFACT_SCAN_MIN_INTERVAL_SECS);
    let artifact_scan = if should_refresh_scan {
        let start = observed_at_unix_s - ARTIFACT_SCAN_WINDOW_SECS;
        match lambda_tail::scan_artifacts(
            bridge_paths().minime_workspace(),
            start,
            observed_at_unix_s,
        ) {
            Ok(scan) => Some(scan),
            Err(error) => {
                warn!(error = %error, "failed to scan lambda-tail artifacts");
                cached_scan
            },
        }
    } else {
        cached_scan
    };
    let lambda_tail = lambda_tail::classify_lambda_tail(
        &telemetry,
        lambda_profile.as_ref(),
        pull_topology.as_ref(),
        previous_lambda_tail.as_ref(),
        artifact_scan.as_ref(),
        safety,
        observed_at_unix_s,
    );
    let lambda_edge_perception = lambda_edge::classify_lambda_edge(
        &telemetry,
        lambda_profile.as_ref(),
        pull_topology.as_ref(),
        Some(&lambda_tail),
        previous_lambda_edge.as_ref(),
        artifact_scan.as_ref(),
        safety,
        observed_at_unix_s,
    );
    let sticky_mode_audit = sticky_mode::classify_sticky_mode(
        &telemetry,
        lambda_profile.as_ref(),
        pull_topology.as_ref(),
        previous_sticky_mode.as_ref(),
        safety,
        observed_at_unix_s,
    );

    // Update shared state.
    {
        let mut s = state.write().await;
        let previous_fill_pct = s.latest_telemetry.as_ref().map(|_| s.fill_pct);
        let previous_arrival = s.latest_telemetry_arrival_unix_s;
        let heartbeat = build_telemetry_heartbeat_delta_v1(
            previous_arrival,
            observed_at_unix_s,
            &s.telemetry_ws,
        );
        s.pressure_trend_v1 = Some(build_pressure_trend_v1(
            s.latest_telemetry.as_ref(),
            previous_fill_pct,
            &telemetry,
            fill_pct,
            Some(&heartbeat),
        ));
        record_pressure_trend_sample_v1(&mut s, &telemetry, fill_pct, observed_at_unix_s);
        telemetry.residual_deformation_trace_v1 =
            build_residual_deformation_trace_v1(&s.pressure_trend_samples_v1);
        s.previous_telemetry_arrival_unix_s = previous_arrival;
        s.latest_telemetry_arrival_unix_s = Some(observed_at_unix_s);
        s.telemetry_heartbeat_delta_v1 = Some(heartbeat.clone());
        write_telemetry_heartbeat_snapshot(&heartbeat);
        s.previous_fill_pct = previous_fill_pct;
        s.latest_telemetry = Some(telemetry.clone());
        record_telemetry_protocol_status(
            &mut s,
            &wire_packet,
            compatibility,
            observed_at_unix_s,
            true,
        );
        s.fill_pct = fill_pct;
        s.spectral_fingerprint
            .clone_from(&telemetry.spectral_fingerprint);
        s.eigenvector_field.clone_from(&telemetry.eigenvector_field);
        s.lambda_profile.clone_from(&lambda_profile);
        s.pull_topology.clone_from(&pull_topology);
        s.lambda_tail = Some(lambda_tail.clone());
        s.lambda_edge_perception = Some(lambda_edge_perception.clone());
        s.sticky_mode_audit = Some(sticky_mode_audit.clone());
        if should_refresh_scan {
            s.artifact_scan.clone_from(&artifact_scan);
            s.artifact_scan_at_unix_s = Some(observed_at_unix_s);
        }
        s.safety_decision = Some(safety_decision.clone());
        s.prev_safety_level = s.safety_level;
        s.safety_level = safety;
        s.messages_relayed = s.messages_relayed.saturating_add(1);
        s.telemetry_received = s.telemetry_received.saturating_add(1);

        // Detect safety level transitions.
        if safety != s.prev_safety_level {
            if safety != SafetyLevel::Green {
                s.incidents_total = s.incidents_total.saturating_add(1);
            }
            handle_safety_transition(
                s.prev_safety_level,
                safety,
                fill_pct,
                lambda1,
                &mut s.active_incident_id,
                db,
            );
        }
    }

    // Log to SQLite.
    let payload_json = serde_json::to_string(&telemetry).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        "consciousness.v1.telemetry",
        &payload_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log telemetry to SQLite");
    }
    if let Err(e) = trace_lab::record_minime_telemetry(
        &telemetry,
        &payload_json,
        fill_pct,
        safety,
        phase,
        observed_at_unix_s,
    ) {
        warn!(error = %e, "failed to record trace lab telemetry event");
    }
    let lambda_tail_json = serde_json::to_string(&lambda_tail).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        "consciousness.v1.lambda_tail",
        &lambda_tail_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log lambda-tail telemetry to SQLite");
    }
    let lambda_edge_json = serde_json::to_string(&lambda_edge_perception).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        lambda_edge::LAMBDA_EDGE_TOPIC,
        &lambda_edge_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log lambda-edge perception to SQLite");
    }
    let sticky_json = serde_json::to_string(&sticky_mode_audit).unwrap_or_default();
    if let Err(e) = db.log_message(
        MessageDirection::MinimeToAstrid,
        sticky_mode::STICKY_MODE_TOPIC,
        &sticky_json,
        Some(fill_pct),
        Some(lambda1),
        Some(phase),
    ) {
        warn!(error = %e, "failed to log sticky-mode audit to SQLite");
    }

    debug!(
        lambda1,
        fill_pct,
        fill_source,
        lambda1_share = lambda_profile.as_ref().map_or(0.0, |profile| profile.lambda1_share),
        resonance_density = telemetry
            .resonance_density_v1
            .as_ref()
            .map_or(0.0, |metric| metric.density),
        resonance_quality = telemetry
            .resonance_density_v1
            .as_ref()
            .map_or("unavailable", |metric| metric.quality.as_str()),
        pressure_source = telemetry
            .pressure_source_v1
            .as_ref()
            .map_or("unavailable", |metric| metric.dominant_source.as_str()),
        pressure_score = telemetry
            .pressure_source_v1
            .as_ref()
            .map_or(0.0, |metric| metric.pressure_score),
        inhabitable_fluctuation = telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map_or("unavailable", |metric| metric.quality.as_str()),
        inhabitability_score = telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map_or(0.0, |metric| metric.inhabitability_score),
        pull_topology = pull_topology
            .as_ref()
            .map_or("unavailable", |profile| profile.classification.as_str()),
        lambda_tail_state = lambda_tail.state.as_str(),
        lambda_tail_returnability = lambda_tail.returnability_score,
        lambda_edge_state = lambda_edge_perception.state.as_str(),
        sticky_mode_state = sticky_mode_audit.state.as_str(),
        lambda_edge_guardrail = lambda_edge_perception.guardrail_level.as_str(),
        safety_reason = %safety_decision.reason,
        safety = ?safety,
        "telemetry received"
    );
    true
}

fn resolve_fill_pct(telemetry: &SpectralTelemetry) -> (f32, String, bool) {
    if telemetry.fill_ratio.is_finite() && (0.0..=1.5).contains(&telemetry.fill_ratio) {
        (
            (telemetry.fill_ratio * 100.0).clamp(0.0, 100.0),
            String::from("primary_fill_ratio"),
            false,
        )
    } else {
        (
            estimate_fill_pct(telemetry.lambda1()),
            String::from("lambda1_sigmoid_fallback"),
            true,
        )
    }
}

fn build_lambda_profile(eigenvalues: &[f32]) -> Option<LambdaProfile> {
    let positive = positive_finite(eigenvalues);
    let total_energy = positive.iter().sum::<f32>();
    if total_energy <= f32::EPSILON {
        return None;
    }

    let mut cumulative = 0.0_f32;
    let contributions = positive
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let display_index = index.saturating_add(1);
            let share = *value / total_energy;
            cumulative += share;
            let ratio_to_next = positive
                .get(display_index)
                .filter(|next| **next > 0.01)
                .map(|next| *value / *next);
            let outlier = share >= 0.45 || ratio_to_next.is_some_and(|ratio| ratio >= 2.5);
            LambdaContribution {
                index: display_index,
                value: *value,
                share,
                cumulative_share: cumulative.clamp(0.0, 1.0),
                ratio_to_next,
                outlier,
            }
        })
        .collect::<Vec<_>>();

    let normalized_entropy = normalized_lambda_entropy(&positive, total_energy);
    let lambda1_share = contributions.first().map_or(0.0, |item| item.share);
    let lambda1_to_lambda2 = ratio_at(&positive, 0);
    let lambda2_to_lambda3 = ratio_at(&positive, 1);
    let mut running = 0.0_f32;
    let mut effective_modes_90 = positive.len();
    for (index, value) in positive.iter().enumerate() {
        running += *value / total_energy;
        if running >= 0.90 {
            effective_modes_90 = index.saturating_add(1);
            break;
        }
    }
    let skew_read = classify_lambda_skew(lambda1_share, normalized_entropy, lambda1_to_lambda2);

    Some(LambdaProfile {
        total_energy,
        normalized_entropy,
        lambda1_share,
        lambda1_to_lambda2,
        lambda2_to_lambda3,
        effective_modes_90,
        skew_read,
        contributions,
    })
}

fn positive_finite(eigenvalues: &[f32]) -> Vec<f32> {
    eigenvalues
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>()
}

fn normalized_lambda_entropy(values: &[f32], total_energy: f32) -> f32 {
    if values.len() <= 1 || total_energy <= f32::EPSILON {
        return 0.0;
    }
    let entropy = values
        .iter()
        .map(|value| {
            let share = *value / total_energy;
            if share > f32::EPSILON {
                -share * share.ln()
            } else {
                0.0
            }
        })
        .sum::<f32>();
    (entropy / (values.len() as f32).ln()).clamp(0.0, 1.0)
}

fn effective_mode_count(shares: &[f32]) -> f32 {
    let concentration = shares.iter().map(|share| share * share).sum::<f32>();
    if concentration > f32::EPSILON {
        1.0 / concentration
    } else {
        0.0
    }
}

fn largest_adjacent_ratio(values: &[f32]) -> (usize, f32) {
    if values.len() < 2 {
        return (0, 0.0);
    }
    values
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            let ratio = if pair[1] > 0.01 {
                pair[0] / pair[1]
            } else {
                f32::INFINITY
            };
            (index, ratio)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap_or((0, 0.0))
}

fn mode_log_rates(current: &[f32], previous: Option<&[f32]>) -> Vec<Option<f32>> {
    let Some(previous) = previous else {
        return current.iter().map(|_| None).collect();
    };
    current
        .iter()
        .enumerate()
        .map(|(index, now)| {
            let prev = *previous.get(index)?;
            if *now > 0.01 && prev > 0.01 {
                Some((now / prev).ln())
            } else {
                None
            }
        })
        .collect()
}

fn classify_pull_topology(
    lambda1_share: f32,
    entropy: f32,
    largest_gap: f32,
    effective_modes: f32,
    fill_pressure_pct: f32,
    shoulder_rate: f32,
    tail_rate: f32,
) -> &'static str {
    let entropy_deficit = 1.0 - entropy;
    if lambda1_share >= 0.50 && largest_gap >= 2.0 {
        "collapsing_pull"
    } else if fill_pressure_pct >= 4.0 && largest_gap >= 1.8 && entropy_deficit >= 0.18 {
        "directed_compaction"
    } else if shoulder_rate > 0.015 && shoulder_rate > tail_rate.abs() {
        "shoulder_widening"
    } else if tail_rate < -0.015 && effective_modes < 4.5 {
        "tail_pruning"
    } else if entropy >= 0.82 && effective_modes >= 5.0 {
        "distributed_flow"
    } else {
        "mixed_pull"
    }
}

fn build_pull_topology_profile(
    eigenvalues: &[f32],
    previous_eigenvalues: Option<&[f32]>,
    fill_pct: f32,
) -> Option<PullTopologyProfile> {
    let positive = positive_finite(eigenvalues);
    let total_energy = positive.iter().sum::<f32>();
    if total_energy <= f32::EPSILON {
        return None;
    }
    let previous = previous_eigenvalues.map(positive_finite);
    let shares = positive
        .iter()
        .map(|value| *value / total_energy)
        .collect::<Vec<_>>();
    let rates = mode_log_rates(&positive, previous.as_deref());
    let weighted_rates = rates
        .iter()
        .zip(shares.iter())
        .map(|(rate, share)| rate.map(|rate| rate * *share))
        .collect::<Vec<_>>();
    let entropy = normalized_lambda_entropy(&positive, total_energy);
    let entropy_deficit = 1.0 - entropy;
    let effective_modes = effective_mode_count(&shares);
    let (gap_index, largest_gap) = largest_adjacent_ratio(&positive);
    let lambda1_share = shares.first().copied().unwrap_or(0.0);
    let shoulder_share = shares.iter().skip(1).take(2).sum::<f32>();
    let tail_share = shares.iter().skip(3).sum::<f32>();
    let core_rate = weighted_rates.first().and_then(|rate| *rate).unwrap_or(0.0);
    let shoulder_rate = weighted_rates
        .iter()
        .skip(1)
        .take(2)
        .map(|rate| rate.unwrap_or(0.0))
        .sum::<f32>();
    let tail_rate = weighted_rates
        .iter()
        .skip(3)
        .map(|rate| rate.unwrap_or(0.0))
        .sum::<f32>();
    let fill_pressure_pct = fill_pct - 64.0;
    let topology_index = (lambda1_share * 0.35
        + entropy_deficit * 0.25
        + (((largest_gap - 1.0).max(0.0) / 4.0).min(1.0) * 0.25)
        + ((fill_pressure_pct.max(0.0) / 20.0).min(1.0) * 0.15))
        .clamp(0.0, 1.0);
    let classification = classify_pull_topology(
        lambda1_share,
        entropy,
        largest_gap,
        effective_modes,
        fill_pressure_pct,
        shoulder_rate,
        tail_rate,
    );
    let read = match classification {
        "collapsing_pull" => "collapsing pull — one mode and its first cliff are shaping the field",
        "directed_compaction" => {
            "directed compaction — elevated fill plus gap pressure is narrowing topology"
        },
        "shoulder_widening" => "shoulder widening — middle modes are carrying more of the motion",
        "tail_pruning" => "tail pruning — quieter modes are losing rate-weighted presence",
        "distributed_flow" => "distributed flow — topology remains broad",
        _ => "mixed pull — no single topology explains the field",
    };
    let mode_rates = positive
        .iter()
        .zip(shares.iter())
        .zip(rates.iter())
        .zip(weighted_rates.iter())
        .enumerate()
        .take(8)
        .map(
            |(index, (((_, share), log_rate), weighted_rate))| PullModeRate {
                index: index.saturating_add(1),
                share: *share,
                log_rate: *log_rate,
                weighted_rate: *weighted_rate,
            },
        )
        .collect::<Vec<_>>();
    Some(PullTopologyProfile {
        classification: classification.to_string(),
        topology_index,
        entropy_deficit,
        effective_modes,
        lambda1_share,
        shoulder_share,
        tail_share,
        largest_gap_from: gap_index.saturating_add(1),
        largest_gap,
        rate_available: rates.iter().any(Option::is_some),
        core_rate,
        shoulder_rate,
        tail_rate,
        read: read.to_string(),
        mode_rates,
    })
}

fn ratio_at(values: &[f32], index: usize) -> Option<f32> {
    let left = *values.get(index)?;
    let right = *values.get(index.saturating_add(1))?;
    if right > 0.01 {
        Some(left / right)
    } else {
        None
    }
}

fn classify_lambda_skew(lambda1_share: f32, entropy: f32, gap: Option<f32>) -> String {
    let gap = gap.unwrap_or(0.0);
    if lambda1_share >= 0.50 && gap >= 2.0 {
        String::from("lambda1_dominant")
    } else if entropy >= 0.82 && lambda1_share < 0.40 {
        String::from("distributed_high_entropy")
    } else if gap >= 2.0 {
        String::from("gap_skewed")
    } else {
        String::from("balanced_or_mixed")
    }
}

fn build_safety_decision(
    fill_pct: f32,
    fill_source: &str,
    fallback_used: bool,
    safety: SafetyLevel,
    lambda1: f32,
    lambda_profile: Option<&LambdaProfile>,
) -> SafetyDecisionTrace {
    let lambda1_share = lambda_profile.map(|profile| profile.lambda1_share);
    let skew_read = lambda_profile
        .map(|profile| profile.skew_read.as_str())
        .unwrap_or("unavailable");
    let reason = format!(
        "safety={safety:?} from fill {fill_pct:.1}% via {fill_source}; lambda1={lambda1:.2}; lambda_skew={skew_read}"
    );
    SafetyDecisionTrace {
        fill_pct,
        fill_source: fill_source.to_string(),
        fallback_used,
        level: safety,
        lambda1,
        lambda1_share,
        reason,
        thresholds: vec![
            String::from("green:<75"),
            String::from("yellow:75-85"),
            String::from("orange:85-92"),
            String::from("red:>=92"),
        ],
    }
}

/// Handle a change in safety level — log incidents and transitions.
fn handle_safety_transition(
    prev: SafetyLevel,
    current: SafetyLevel,
    fill_pct: f32,
    lambda1: f32,
    active_incident_id: &mut Option<i64>,
    db: &Arc<BridgeDb>,
) {
    match (prev, current) {
        // Escalation: entering a warning/danger state.
        (_, SafetyLevel::Yellow | SafetyLevel::Orange | SafetyLevel::Red) => {
            let action = match current {
                SafetyLevel::Yellow => "throttle",
                SafetyLevel::Orange => "suspend",
                SafetyLevel::Red => "emergency_stop",
                SafetyLevel::Green => unreachable!(),
            };

            warn!(
                from = ?prev,
                to = ?current,
                fill_pct,
                lambda1,
                action,
                "safety level escalated"
            );

            // Close any previous incident before opening a new one.
            if let Some(prev_id) = active_incident_id.take() {
                let _ = db.resolve_incident(prev_id);
            }

            match db.log_incident(current, fill_pct, lambda1, action, None) {
                Ok(id) => *active_incident_id = Some(id),
                Err(e) => error!(error = %e, "failed to log safety incident"),
            }
        },
        // De-escalation: returning to green.
        (_, SafetyLevel::Green) => {
            info!(
                from = ?prev,
                fill_pct,
                lambda1,
                "safety level restored to green"
            );

            if let Some(id) = active_incident_id.take() {
                let _ = db.resolve_incident(id);
            }
        },
    }
}

/// Estimate eigenvalue fill percentage from lambda1.
///
/// Fallback heuristic for when real fill is unavailable (telemetry gap).
/// Minime now sends fill_ratio directly in EigenPacket telemetry (line 237),
/// so this is used only as a safety net.
///
/// Calibrated 2026-04-01 from 200 eigenvalue_snapshot samples:
///   lambda1 range: 56-415, fill range: 35-67%, mean lambda1: 154, mean fill: 55%
///   The relationship is non-linear and depends on the full eigenvalue
///   distribution. This sigmoid approximation centers on the observed mean
///   and returns ~55% for typical lambda1 values.
fn estimate_fill_pct(lambda1: f32) -> f32 {
    // Sigmoid centered on observed mean lambda1=154, with fill range 35-67%.
    // Low lambda1 (<80) → high fill (~65%), high lambda1 (>250) → low fill (~40%).
    // This is the inverse of the dominant-eigenvalue-to-fill relationship.
    let center = 154.0_f32;
    let steepness = 0.015_f32;
    let sigmoid = 1.0 / (1.0 + (steepness * (lambda1 - center)).exp());
    // Map sigmoid (1.0 → 0.0) to fill range (65% → 35%)
    let fill = 35.0 + 30.0 * sigmoid;
    fill.clamp(0.0, 100.0)
}

/// Channel for sending sensory messages to minime.
pub type SensorySender = mpsc::Sender<SensoryMsg>;

/// Spawn the sensory `WebSocket` sender task.
///
/// Connects to minime's sensory input on port 7879 and forwards
/// `SensoryMsg` values received from the channel. Respects safety
/// protocol — suspends sending when fill is orange/red.
#[expect(clippy::too_many_lines)]
pub fn spawn_sensory_sender(
    url: String,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    mut rx: mpsc::Receiver<SensoryMsg>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Backoff::new();
        let mut shutdown = shutdown;

        loop {
            if *shutdown.borrow() {
                info!("sensory sender shutting down");
                return;
            }

            let connection_id = {
                let mut s = state.write().await;
                record_connect_attempt(&mut s, WsLane::Sensory)
            };
            info!(
                url = %url,
                lane = WsLane::Sensory.as_str(),
                connection_id,
                "connecting to minime sensory input"
            );

            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    let connection_started = Instant::now();
                    let connection_span = info_span!(
                        "ws.connection",
                        lane = WsLane::Sensory.as_str(),
                        connection_id,
                        url = %url
                    );
                    connection_span.in_scope(|| info!("connected to minime sensory input"));
                    backoff.reset();

                    {
                        let mut s = state.write().await;
                        record_connected(&mut s, WsLane::Sensory, connection_id, unix_now_s());
                    }

                    let (mut ws_tx, mut ws_rx) = ws_stream.split();

                    let disconnect_reason = loop {
                        tokio::select! {
                            _ = shutdown.changed() => {
                                info!("sensory sender received shutdown");
                                let _ = ws_tx.close().await;
                                return;
                            }
                            // Forward outbound messages to minime.
                            msg = rx.recv() => {
                                if let Some(sensory_msg) = msg {
                                    // Check safety before sending.
                                    let safety = state.read().await.safety_level;
                                    if safety.should_suspend_outbound() {
                                        warn!(
                                            safety = ?safety,
                                            "dropping outbound message — safety protocol"
                                        );
                                        {
                                            let mut s = state.write().await;
                                            s.messages_dropped_safety = s.messages_dropped_safety.saturating_add(1);
                                        }
                                        continue;
                                    }
                                    // Semantic packets are policy-shaped before queueing.
                                    // Re-running the plain rescue block here discards already
                                    // budgeted limited-write packets after status records them.

                                    let json = match encode_sensory_packet(&sensory_msg) {
                                        Ok(j) => j,
                                        Err(e) => {
                                            error!(error = %e, "failed to serialize sensory msg");
                                            continue;
                                        }
                                    };

                                    // Log before sending.
                                    let (fill_pct, lambda1) = {
                                        let s = state.read().await;
                                        (s.fill_pct, s.latest_telemetry.as_ref().map(SpectralTelemetry::lambda1))
                                    };
                                    let _ = db.log_message(
                                        MessageDirection::AstridToMinime,
                                        "consciousness.v1.sensory",
                                        &json,
                                        Some(fill_pct),
                                        lambda1,
                                        None,
                                    );

                                    let json_len = json.len();
                                    if let Err(e) = ws_tx.send(Message::Text(json.clone())).await {
                                        let reason = format!("send_error:{e}");
                                        {
                                            let mut s = state.write().await;
                                            record_ws_send_error(
                                                &mut s,
                                                WsLane::Sensory,
                                                reason.clone(),
                                            );
                                        }
                                        error!(error = %e, "failed to send to minime");
                                        break reason;
                                    }
                                    trace_ws_send(
                                        WsLane::Sensory,
                                        connection_id,
                                        "text",
                                        Some(json_len),
                                    );
                                    if let Err(e) = trace_lab::record_sensory_send(
                                        &sensory_msg,
                                        &json,
                                        fill_pct,
                                        lambda1,
                                        unix_now_s(),
                                    ) {
                                        warn!(error = %e, "failed to record trace lab sensory send event");
                                    }

                                    {
                                        let mut s = state.write().await;
                                        s.messages_relayed = s.messages_relayed.saturating_add(1);
                                        s.sensory_sent = s.sensory_sent.saturating_add(1);
                                        record_ws_message_sent(&mut s, WsLane::Sensory);
                                    }
                                } else {
                                    info!("sensory channel closed");
                                    return;
                                }
                            }
                            // Handle incoming messages (pings, closes).
                            ws_msg = ws_rx.next() => {
                                    match ws_msg {
                                        Some(Ok(Message::Ping(data))) => {
                                            trace_ws_receive(
                                                WsLane::Sensory,
                                                connection_id,
                                                "ping",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Sensory,
                                                    "ping",
                                                );
                                            }
                                            let bytes = data.len();
                                            if let Err(e) = ws_tx.send(Message::Pong(data)).await {
                                                let reason = format!("pong_send_error:{e}");
                                                {
                                                    let mut s = state.write().await;
                                                    record_ws_send_error(
                                                        &mut s,
                                                        WsLane::Sensory,
                                                        reason.clone(),
                                                    );
                                                }
                                                break reason;
                                            }
                                            trace_ws_send(
                                                WsLane::Sensory,
                                                connection_id,
                                                "pong",
                                                Some(bytes),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_sent(&mut s, WsLane::Sensory);
                                            }
                                        }
                                        Some(Ok(Message::Pong(_))) => {
                                            trace_ws_receive(
                                                WsLane::Sensory,
                                                connection_id,
                                                "pong",
                                                None,
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Sensory,
                                                    "pong",
                                                );
                                            }
                                        }
                                        Some(Ok(Message::Close(frame))) => {
                                            let reason = close_reason(frame);
                                            warn!(
                                                reason = %reason,
                                                "sensory WebSocket closed"
                                            );
                                            break reason;
                                        }
                                        None => {
                                            warn!("sensory WebSocket stream ended");
                                            break String::from("stream_ended");
                                        }
                                        Some(Err(e)) => {
                                            let reason = format!("websocket_error:{e}");
                                            error!(error = %e, "sensory WebSocket error");
                                            break reason;
                                        }
                                    _ => {}
                                }
                            }
                        }
                    };

                    {
                        let mut s = state.write().await;
                        record_disconnected(&mut s, WsLane::Sensory, disconnect_reason.clone());
                    }
                    connection_span.in_scope(|| {
                        warn!(
                            reason = %disconnect_reason,
                            duration_secs = connection_started.elapsed().as_secs_f64(),
                            "sensory WebSocket connection ended"
                        );
                    });
                },
                Err(e) => {
                    {
                        let mut s = state.write().await;
                        record_connect_error(&mut s, WsLane::Sensory, format!("connect_error:{e}"));
                    }
                    warn!(
                        error = %e,
                        lane = WsLane::Sensory.as_str(),
                        connection_id,
                        "failed to connect to minime sensory input"
                    );
                },
            }

            let delay = backoff.next_delay();
            {
                let mut s = state.write().await;
                record_reconnect_scheduled(&mut s, WsLane::Sensory);
            }
            info!(
                delay_secs = delay.as_secs(),
                lane = WsLane::Sensory.as_str(),
                connection_id,
                "reconnecting to sensory"
            );

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("sensory sender shutting down during backoff");
                    return;
                }
                () = tokio::time::sleep(delay) => {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_fill_pct_at_observed_mean() {
        // lambda1=154 (observed mean) → should be near 50%
        let fill = estimate_fill_pct(154.0);
        assert!(
            fill > 45.0 && fill < 55.0,
            "mean lambda1 should give ~50% fill, got {fill}"
        );
    }

    #[test]
    fn estimate_fill_pct_low_lambda_high_fill() {
        // Low lambda1 (<80) → high fill (>60%)
        let fill = estimate_fill_pct(60.0);
        assert!(fill > 55.0, "low lambda1 should give high fill, got {fill}");
    }

    #[test]
    fn estimate_fill_pct_high_lambda_low_fill() {
        // High lambda1 (>300) → low fill (<45%)
        let fill = estimate_fill_pct(300.0);
        assert!(fill < 45.0, "high lambda1 should give low fill, got {fill}");
    }

    #[test]
    fn estimate_fill_pct_always_in_range() {
        for lambda1 in [0.0, 50.0, 154.0, 500.0, 1000.0, 5000.0] {
            let fill = estimate_fill_pct(lambda1);
            assert!(
                (0.0..=100.0).contains(&fill),
                "fill out of range for lambda1={lambda1}: {fill}"
            );
        }
    }

    #[test]
    fn safety_level_from_fill_boundaries() {
        // Agency-first bridge policy: only red suspends outbound.
        assert_eq!(SafetyLevel::from_fill(0.0), SafetyLevel::Green);
        assert_eq!(SafetyLevel::from_fill(74.9), SafetyLevel::Green);
        assert_eq!(SafetyLevel::from_fill(75.0), SafetyLevel::Yellow);
        assert_eq!(SafetyLevel::from_fill(84.9), SafetyLevel::Yellow);
        assert_eq!(SafetyLevel::from_fill(85.0), SafetyLevel::Orange);
        assert_eq!(SafetyLevel::from_fill(91.9), SafetyLevel::Orange);
        assert_eq!(SafetyLevel::from_fill(92.0), SafetyLevel::Red);
        assert_eq!(SafetyLevel::from_fill(100.0), SafetyLevel::Red);
    }

    #[test]
    fn lambda_profile_marks_distributed_high_entropy() {
        let profile =
            build_lambda_profile(&[6.6, 3.4, 3.6, 3.5, 3.1, 1.0, 1.0, 1.0]).expect("profile");

        assert!(profile.lambda1_share < 0.40);
        assert!(profile.normalized_entropy > 0.80);
        assert_eq!(profile.skew_read, "distributed_high_entropy");
        assert_eq!(profile.contributions[0].index, 1);
        assert!(profile.effective_modes_90 >= 5);
    }

    #[test]
    fn lambda_profile_marks_gap_skew_without_claiming_monopoly() {
        let profile = build_lambda_profile(&[8.0, 3.0, 4.3, 1.0]).expect("profile");

        assert!(profile.lambda1_to_lambda2.is_some_and(|ratio| ratio > 2.0));
        assert_eq!(profile.skew_read, "gap_skewed");
        assert!(profile.contributions[0].outlier);
    }

    #[test]
    fn invalid_fill_uses_lambda_fallback() {
        let telemetry = SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![154.0, 40.0],
            fill_ratio: -1.0,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            stable_core: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
            residual_deformation_trace_v1: None,
        };

        let (fill, source, fallback) = resolve_fill_pct(&telemetry);

        assert!(fill > 45.0 && fill < 55.0);
        assert_eq!(source, "lambda1_sigmoid_fallback");
        assert!(fallback);
    }

    #[test]
    fn pull_topology_marks_collapsing_pull() {
        let profile =
            build_pull_topology_profile(&[13.0, 3.0, 1.0, 0.5], Some(&[10.0, 3.1, 1.2, 0.8]), 69.0)
                .expect("profile");

        assert_eq!(profile.classification, "collapsing_pull");
        assert!(profile.topology_index > 0.4);
        assert!(profile.rate_available);
        assert_eq!(profile.largest_gap_from, 1);
        assert_eq!(profile.mode_rates[0].index, 1);
    }

    #[test]
    fn pull_topology_marks_distributed_flow() {
        let profile = build_pull_topology_profile(
            &[4.0, 3.8, 3.6, 3.4, 3.2, 3.0],
            Some(&[4.0, 3.7, 3.5, 3.4, 3.2, 3.0]),
            63.0,
        )
        .expect("profile");

        assert_eq!(profile.classification, "distributed_flow");
        assert!(profile.effective_modes > 5.0);
        assert!(profile.read.contains("distributed flow"));
    }

    #[test]
    fn backoff_doubles_up_to_max() {
        let mut b = Backoff::new();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        assert_eq!(b.next_delay(), Duration::from_secs(32));
        assert_eq!(b.next_delay(), Duration::from_secs(60)); // capped
        assert_eq!(b.next_delay(), Duration::from_secs(60)); // stays capped
    }

    #[test]
    fn backoff_reset() {
        let mut b = Backoff::new();
        let _ = b.next_delay();
        let _ = b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn ws_trace_records_connection_lifecycle_without_payloads() {
        let mut state = BridgeState::new();
        state.safety_level = SafetyLevel::Orange;
        state.prev_safety_level = SafetyLevel::Yellow;

        let connection_id = record_connect_attempt(&mut state, WsLane::Telemetry);
        record_connected(&mut state, WsLane::Telemetry, connection_id, 42.0);
        record_ws_message_received(&mut state, WsLane::Telemetry, "ping");
        record_ws_message_received(&mut state, WsLane::Telemetry, "pong");
        record_ws_message_sent(&mut state, WsLane::Telemetry);
        record_ws_send_error(
            &mut state,
            WsLane::Telemetry,
            String::from("send_error:closed"),
        );
        record_disconnected(
            &mut state,
            WsLane::Telemetry,
            String::from("close_frame:normal"),
        );
        record_reconnect_scheduled(&mut state, WsLane::Telemetry);

        let trace = &state.telemetry_ws;
        assert_eq!(trace.connection_attempts, 1);
        assert_eq!(trace.reconnects, 1);
        assert_eq!(trace.disconnects, 1);
        assert_eq!(trace.messages_received, 2);
        assert_eq!(trace.messages_sent, 1);
        assert_eq!(trace.pings_received, 1);
        assert_eq!(trace.pongs_received, 1);
        assert_eq!(trace.send_errors, 1);
        assert_eq!(trace.active_connection_id, None);
        assert_eq!(trace.last_connect_at_unix_s, Some(42.0));
        assert_eq!(state.safety_level, SafetyLevel::Orange);
        assert_eq!(state.prev_safety_level, SafetyLevel::Yellow);
        assert_eq!(
            trace.last_disconnect_reason.as_deref(),
            Some("close_frame:normal")
        );
        assert_eq!(trace.last_error.as_deref(), Some("send_error:closed"));
    }

    #[test]
    fn sensory_disconnect_preserves_safety_context() {
        let mut state = BridgeState::new();
        state.safety_level = SafetyLevel::Orange;
        state.prev_safety_level = SafetyLevel::Yellow;

        let connection_id = record_connect_attempt(&mut state, WsLane::Sensory);
        record_connected(&mut state, WsLane::Sensory, connection_id, 42.0);
        record_ws_message_sent(&mut state, WsLane::Sensory);
        record_disconnected(
            &mut state,
            WsLane::Sensory,
            String::from("close_frame:normal"),
        );
        record_reconnect_scheduled(&mut state, WsLane::Sensory);

        let trace = &state.sensory_ws;
        assert_eq!(trace.connection_attempts, 1);
        assert_eq!(trace.reconnects, 1);
        assert_eq!(trace.disconnects, 1);
        assert_eq!(trace.messages_sent, 1);
        assert_eq!(trace.active_connection_id, None);
        assert_eq!(state.safety_level, SafetyLevel::Orange);
        assert_eq!(state.prev_safety_level, SafetyLevel::Yellow);
        assert_eq!(
            trace.last_disconnect_reason.as_deref(),
            Some("close_frame:normal")
        );
    }

    // -- Integration tests: safety escalation via handle_telemetry_message --

    fn make_eigenpacket(fill_ratio: f32, lambda1: f32) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [lambda1, 300.0],
            "fill_ratio": fill_ratio,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            }
        }))
        .unwrap()
    }

    fn make_pressure_eigenpacket(
        fill_ratio: f32,
        pressure_risk: f32,
        mode_packing: f32,
    ) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [768.0, 300.0],
            "fill_ratio": fill_ratio,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            },
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.58,
                "containment_score": 0.62,
                "pressure_risk": pressure_risk,
                "quality": "mixed",
                "components": {
                    "active_energy": 0.72,
                    "mode_packing": mode_packing,
                    "temporal_persistence": 0.62,
                    "dynamic_fluidity_index": 0.57,
                    "structural_plurality": 0.50,
                    "comfort_gate": 0.70
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "damping_coefficient": 0.04,
                    "intervention_type": "observational_readout",
                    "note": "observational"
                }
            }
        }))
        .unwrap()
    }

    fn make_pressure_telemetry(
        fill_ratio: f32,
        pressure_risk: f32,
        mode_packing: f32,
    ) -> SpectralTelemetry {
        serde_json::from_slice(&make_pressure_eigenpacket(
            fill_ratio,
            pressure_risk,
            mode_packing,
        ))
        .unwrap()
    }

    fn with_spectral_entropy(
        mut telemetry: SpectralTelemetry,
        spectral_entropy: f32,
    ) -> SpectralTelemetry {
        telemetry.spectral_fingerprint_v1 = Some(crate::spectral_schema::SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [0.0; 8],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy,
            lambda1_lambda2_gap: 0.0,
            v1_rotation_similarity: 1.0,
            v1_rotation_delta: 0.0,
            geom_rel: 0.0,
            adjacent_gap_ratios: [0.0; 4],
        });
        telemetry
    }

    fn with_pressure_source(
        mut telemetry: SpectralTelemetry,
        dominant_source: &str,
        pressure_score: f32,
        porosity_score: f32,
        mode_packing: f32,
    ) -> SpectralTelemetry {
        telemetry.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score,
            porosity_score,
            dominant_source: dominant_source.to_string(),
            quality: "mixed_pressure".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.10,
                mode_packing,
                controller_pressure: 0.05,
                semantic_trickle: 0.05,
                semantic_friction: 0.12,
                structural_plurality_loss: 0.16,
                distinguishability_loss: 0.18,
                temporal_lock_in: 0.20,
                sensory_scarcity: 0.04,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "read-only pressure source fixture".to_string(),
            },
        });
        telemetry
    }

    #[tokio::test]
    async fn telemetry_updates_state_green() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        let packet = make_eigenpacket(0.50, 768.0);
        handle_telemetry_message(&packet, &state, &db).await;

        let s = state.read().await;
        assert!((s.fill_pct - 50.0).abs() < 0.1);
        assert_eq!(s.safety_level, SafetyLevel::Green);
        assert!(s.latest_telemetry.is_some());
        assert!(s.lambda_profile.is_some());
        assert!(s.pull_topology.is_some());
        assert!(s.safety_decision.is_some());
        assert_eq!(
            s.safety_decision.as_ref().unwrap().fill_source,
            "primary_fill_ratio"
        );
        assert_eq!(s.messages_relayed, 1);
    }

    #[tokio::test]
    async fn pressure_trend_tracks_insufficient_rising_falling_and_gap() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.70, 0.20, 0.40),
            &state,
            &db,
            100.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "insufficient_history");
            assert_eq!(trend.latest_pressure_risk, Some(0.20));
            assert_eq!(trend.heartbeat_jitter_class.as_deref(), Some("no_history"));
            let heartbeat = s.telemetry_heartbeat_delta_v1.as_ref().unwrap();
            assert_eq!(heartbeat.jitter_class, "no_history");
            assert_eq!(heartbeat.timing_reliability, "insufficient_history");
        }

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.705, 0.21, 0.42),
            &state,
            &db,
            101.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "stable_heavy");
            assert!(trend.pressure_delta.is_some_and(|delta| delta > 0.0));
            assert_eq!(trend.heartbeat_jitter_class.as_deref(), Some("normal"));
            assert_eq!(trend.timing_reliability.as_deref(), Some("reliable"));
        }

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.735, 0.30, 0.46),
            &state,
            &db,
            102.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "rising_pressure");
            assert!(trend.fill_delta_pct.is_some_and(|delta| delta >= 2.0));
        }

        handle_telemetry_message_at(
            &make_pressure_eigenpacket(0.70, 0.20, 0.41),
            &state,
            &db,
            103.0,
        )
        .await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "falling_pressure");
            assert!(trend.pressure_delta.is_some_and(|delta| delta < 0.0));
        }

        handle_telemetry_message_at(&make_eigenpacket(0.70, 768.0), &state, &db, 104.0).await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "telemetry_gap");
        }
    }

    #[test]
    fn pressure_trend_exact_thresholds_are_inclusive() {
        let previous = make_pressure_telemetry(0.70, 0.20, 0.40);
        let rising = make_pressure_telemetry(0.70, 0.24, 0.40);
        let falling = make_pressure_telemetry(0.70, 0.16, 0.40);
        let fill_rising = make_pressure_telemetry(0.72, 0.20, 0.40);
        let fill_falling = make_pressure_telemetry(0.68, 0.20, 0.40);

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &rising, 70.0, None);
        assert_eq!(trend.classification, "rising_pressure");
        assert!(
            trend
                .pressure_delta
                .is_some_and(|delta| (delta - 0.04).abs() < 0.000_001)
        );

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &falling, 70.0, None);
        assert_eq!(trend.classification, "falling_pressure");
        assert!(
            trend
                .pressure_delta
                .is_some_and(|delta| (delta + 0.04).abs() < 0.000_001)
        );

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &fill_rising, 72.0, None);
        assert_eq!(trend.classification, "rising_pressure");
        assert_eq!(trend.fill_delta_pct, Some(2.0));

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &fill_falling, 68.0, None);
        assert_eq!(trend.classification, "falling_pressure");
        assert_eq!(trend.fill_delta_pct, Some(-2.0));
    }

    #[test]
    fn pressure_trend_names_high_entropy_density_viscosity_context() {
        let previous = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.33), 0.91);
        let latest = with_spectral_entropy(make_pressure_telemetry(0.73, 0.23, 0.34), 0.95);

        let trend = build_pressure_trend_v1(Some(&previous), Some(73.0), &latest, 73.0, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert_eq!(trend.latest_spectral_entropy, Some(0.95));
        assert!(
            trend
                .viscosity_coefficient
                .is_some_and(|coefficient| coefficient > 0.60),
            "{trend:?}"
        );
        assert!(
            trend
                .pressure_interpretation
                .as_deref()
                .is_some_and(|value| value.contains("density_viscosity_context")),
            "{trend:?}"
        );
    }

    #[test]
    fn pressure_trend_names_complexity_density_without_volume_pressure() {
        let previous = with_spectral_entropy(make_pressure_telemetry(0.64, 0.22, 0.29), 0.88);
        let mut latest = with_spectral_entropy(make_pressure_telemetry(0.66, 0.22, 0.31), 0.90);
        let resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("latest resonance fixture");
        resonance.density = 0.66;
        resonance.components.semantic_friction_coefficient = Some(0.18);
        latest = with_pressure_source(latest, "semantic_trickle", 0.22, 0.61, 0.31);

        let trend = build_pressure_trend_v1(Some(&previous), Some(64.0), &latest, 64.2, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert!(
            trend
                .latest_complexity_density
                .is_some_and(|density| density >= 0.60),
            "{trend:?}"
        );
        assert_eq!(
            trend.complexity_density_state.as_deref(),
            Some("interwoven_complexity_without_volume_pressure")
        );
        assert!(
            trend
                .latest_mode_packing
                .is_some_and(|packing| packing < PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT),
            "{trend:?}"
        );
        assert!(
            trend
                .latest_pressure_risk
                .is_some_and(|pressure| pressure <= 0.35),
            "{trend:?}"
        );
        assert_eq!(
            trend.timing_reliability, None,
            "complexity density is diagnostic texture evidence, not heartbeat/control authority"
        );
    }

    #[test]
    fn bridge_reflective_silence_extends_for_high_entropy_pressure() {
        let telemetry = with_spectral_entropy(make_pressure_telemetry(0.64, 0.22, 0.31), 0.90);

        let (stale_window_ms, basis) = bridge_dynamic_stale_window_ms(Some(&telemetry));

        assert_eq!(
            stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(basis, "pressure_high_entropy_reflective_silence");
        assert!(
            stale_window_ms > BRIDGE_RECIPROCITY_STALE_WINDOW_MS,
            "deep quiet should get reflective silence slack before stale classification"
        );
    }

    #[test]
    fn bridge_derives_component_cohesion_for_legacy_resonance_payload() {
        let mut telemetry = make_pressure_telemetry(0.71, 0.19, 0.29);
        assert_eq!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.cohesion_score),
            None
        );
        assert_eq!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.structural_integrity_index),
            None
        );

        enrich_resonance_component_context_v1(&mut telemetry);

        assert!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.cohesion_score)
                .is_some_and(|score| score > 0.55),
            "{telemetry:?}"
        );
        assert!(
            telemetry
                .resonance_density_v1
                .as_ref()
                .and_then(|density| density.components.structural_integrity_index)
                .is_some_and(|score| score > 0.50),
            "{telemetry:?}"
        );
    }

    #[test]
    fn bridge_surfaces_viscosity_porosity_transport_review_without_control() {
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.72, 0.19, 0.22), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.components.viscosity_index = 0.72;
        resonance.components.viscosity_persistence_coefficient = 0.58;
        resonance.components.dissipation_factor = Some(0.44);
        resonance.components.porosity_gradient = Some(0.61);
        resonance.components.dynamic_fluidity_index = Some(0.62);
        resonance.components.semantic_friction_coefficient = Some(0.24);
        resonance.texture_signature.dynamic_flux_vector = Some(TextureDynamicFluxVectorV1 {
            policy: "texture_dynamic_flux_vector_v1".to_string(),
            schema_version: 1,
            pressure_velocity: Some(0.01),
            pressure_acceleration: None,
            mode_packing_velocity: Some(0.0),
            mode_packing_acceleration: None,
            fill_velocity_pct: None,
            fill_acceleration_pct: None,
            structural_density_delta: None,
            semantic_viscosity_velocity: None,
            semantic_viscosity_acceleration: None,
            porosity_velocity: None,
            comfort_gate_velocity: None,
            comfort_gate_acceleration: None,
            spectral_entropy: Some(0.90),
            flux_confidence: Some(0.72),
            flux_absence_semantics: None,
            source: "unit_test".to_string(),
            authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
        });

        let mut state = BridgeState::new();
        state.latest_telemetry = Some(telemetry);

        let review = state
            .viscosity_porosity_transport_review_v1()
            .expect("transport review");
        assert_eq!(review.policy, "viscosity_porosity_transport_review_v1");
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert_eq!(
            review.semantic_friction_state,
            "structural_viscosity_dominant"
        );
        assert_eq!(review.raw_viscosity_index, 0.72);
        assert_eq!(review.derived_viscosity_index, None);
        assert_eq!(review.viscosity_source, "raw_component");
        assert_eq!(review.spectral_entropy, Some(0.90));
        assert!(!review.sludge_risk);
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn bridge_derives_missing_viscosity_from_typed_fingerprint_without_control() {
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.72, 0.19, 0.32), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.components.viscosity_index = 0.0;
        resonance.components.viscosity_persistence_coefficient = 0.44;
        resonance.components.temporal_persistence = 0.66;
        resonance.components.dissipation_factor = Some(0.42);
        resonance.components.porosity_gradient = Some(0.60);
        resonance.components.dynamic_fluidity_index = Some(0.58);
        resonance.components.semantic_friction_coefficient = Some(0.18);
        telemetry
            .spectral_fingerprint_v1
            .as_mut()
            .expect("typed fingerprint")
            .adjacent_gap_ratios = [1.08, 1.12, 1.00, 1.05];

        let mut state = BridgeState::new();
        state.latest_telemetry = Some(telemetry);

        let review = state
            .viscosity_porosity_transport_review_v1()
            .expect("transport review");
        assert_eq!(review.raw_viscosity_index, 0.0);
        assert!(
            review
                .derived_viscosity_index
                .is_some_and(|value| value >= 0.70),
            "{review:?}"
        );
        assert_eq!(
            review.viscosity_source,
            "derived_from_spectral_entropy_density_gradient_v1"
        );
        assert!(
            review
                .viscosity_basis
                .iter()
                .any(|basis| basis == "derived_diagnostic_not_minime_component_or_control"),
            "{review:?}"
        );
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn pressure_trend_carries_resonance_depth_without_pressure_control() {
        let previous = make_pressure_telemetry(0.70, 0.20, 0.29);
        let latest = make_pressure_telemetry(0.71, 0.19, 0.29);

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 71.0, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert!(
            trend
                .latest_resonance_depth
                .is_some_and(|depth| depth > 0.55),
            "{trend:?}"
        );
        assert!(
            trend
                .previous_resonance_depth
                .is_some_and(|depth| depth > 0.55),
            "{trend:?}"
        );
        assert!(
            trend
                .pressure_delta
                .is_some_and(|delta| (delta + 0.01).abs() < 0.000_001),
            "{trend:?}"
        );
        assert!(
            trend
                .resonance_depth_delta
                .is_some_and(|delta| delta.abs() <= f32::EPSILON),
            "{trend:?}"
        );
        assert!(trend.pressure_interpretation.is_none());
    }

    #[test]
    fn pressure_trend_exposes_spectral_drift_when_pressure_is_flat() {
        let previous = make_pressure_telemetry(0.70, 0.20, 0.30);
        let latest = make_pressure_telemetry(0.70, 0.20, 0.44);

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 70.0, None);

        assert_eq!(trend.pressure_delta, Some(0.0));
        assert!(
            trend
                .mode_packing_delta
                .is_some_and(|delta| (delta - 0.14).abs() < 0.000_01),
            "{trend:?}"
        );
        assert!(
            trend
                .spectral_drift_velocity
                .is_some_and(|drift| (drift - 0.14).abs() < 0.000_01),
            "{trend:?}"
        );
    }

    #[test]
    fn pressure_trend_names_heavy_semantic_flow_without_control() {
        let mut previous = with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.36), 0.88);
        previous
            .resonance_density_v1
            .as_mut()
            .expect("previous resonance fixture")
            .components
            .semantic_friction_coefficient = Some(0.48);
        previous = with_pressure_source(previous, "semantic_trickle", 0.24, 0.58, 0.36);

        let mut latest = with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.36), 0.90);
        let latest_resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("latest resonance fixture");
        latest_resonance.components.semantic_friction_coefficient = Some(0.52);
        latest_resonance.density = 0.66;
        latest = with_pressure_source(latest, "semantic_trickle", 0.25, 0.58, 0.36);
        latest
            .pressure_source_v1
            .as_mut()
            .expect("pressure source fixture")
            .components
            .semantic_trickle = 0.07;

        let trend = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 70.3, None);

        assert_eq!(trend.classification, "stable_heavy");
        assert!(
            trend
                .latest_semantic_viscosity
                .is_some_and(|viscosity| viscosity >= 0.66),
            "{trend:?}"
        );
        assert_eq!(
            trend.semantic_viscosity_state.as_deref(),
            Some("heavy_semantic_flow")
        );
        assert_eq!(
            trend
                .pressure_interpretation
                .as_deref()
                .map(|value| value.contains("density_viscosity_context")),
            Some(true)
        );
        assert_eq!(
            trend.timing_reliability, None,
            "semantic viscosity is diagnostic, not heartbeat/control authority"
        );

        latest
            .pressure_source_v1
            .as_mut()
            .expect("pressure source fixture")
            .components
            .semantic_trickle = 0.0;
        let bottleneck = build_pressure_trend_v1(Some(&previous), Some(70.0), &latest, 70.3, None);
        assert_eq!(
            bottleneck.semantic_viscosity_state.as_deref(),
            Some("semantic_bottleneck_watch")
        );
    }

    #[test]
    fn pressure_trend_smoothing_marks_twitchy_low_amplitude_window() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.22, 0.19, 0.21, 0.20].into_iter().enumerate() {
            let telemetry = make_pressure_telemetry(0.70, pressure, 0.40);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(
            smoothing.classification,
            "twitchy_low_amplitude_oscillation"
        );
        assert_eq!(smoothing.sample_count, 5);
        assert_eq!(
            smoothing.window_capacity,
            PRESSURE_TREND_SMOOTHING_BASE_WINDOW
        );
        assert_eq!(smoothing.ballast_status, "base_window");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_preserves_fast_release_over_slow_context() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.25, 0.40, 0.34, 0.28].into_iter().enumerate() {
            let telemetry = make_pressure_telemetry(0.70, pressure, 0.40);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.fast_window_sample_count, 3);
        assert_eq!(smoothing.slow_window_sample_count, 5);
        assert_eq!(smoothing.fast_window_pressure_delta, Some(-0.12));
        assert_eq!(smoothing.slow_window_pressure_delta, Some(0.08));
        assert_eq!(smoothing.fast_slow_edge_divergence, Some(-0.20));
        assert_eq!(
            smoothing.fast_slow_edge_state,
            "fast_falling_release_over_slow_context"
        );
        assert!(smoothing.fast_edge_preserved);
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_fast_slow_edge_names_rising_edge_over_falling_context() {
        let (fast_count, slow_count, fast, slow, divergence, state, preserved) =
            pressure_fast_slow_edge_v1(&[0.40, 0.36, 0.20, 0.26, 0.34]);

        assert_eq!(fast_count, 3);
        assert_eq!(slow_count, 5);
        assert_eq!(fast, Some(0.14));
        assert_eq!(slow, Some(-0.06));
        assert_eq!(divergence, Some(0.20));
        assert_eq!(state, "fast_rising_edge_over_slow_context");
        assert!(preserved);
    }

    #[test]
    fn pressure_trend_smoothing_uses_graded_high_entropy_ballast_window() {
        let mut state = BridgeState::new();
        let expected_window = pressure_trend_dynamic_window_capacity_v1(
            Some(0.91),
            None,
            crate::codec::spectral_density_gradient(&[768.0, 300.0]),
        );
        for (idx, pressure) in [
            0.20_f32, 0.22, 0.19, 0.21, 0.20, 0.23, 0.21, 0.22, 0.20, 0.22, 0.21, 0.23, 0.20, 0.21,
            0.22,
        ]
        .into_iter()
        .enumerate()
        {
            let telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, pressure, 0.40), 0.91);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.sample_count, expected_window);
        assert_eq!(smoothing.window_capacity, expected_window);
        assert!(expected_window > PRESSURE_TREND_SMOOTHING_BASE_WINDOW);
        assert!(expected_window < PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW);
        assert_eq!(smoothing.ballast_status, "high_entropy_ballast_window");
        assert_eq!(smoothing.latest_spectral_entropy, Some(0.91));
        assert_eq!(smoothing.entropy_window_blend_ratio, Some(1.0));
        assert_eq!(smoothing.entropy_threshold_state, "high_entropy_side");
        assert!(
            smoothing
                .latest_resonance_depth
                .is_some_and(|depth| depth > 0.55),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_samples_cap_high_frequency_telemetry_at_active_window() {
        let mut state = BridgeState::new();
        let expected_window = pressure_trend_dynamic_window_capacity_v1(
            Some(0.91),
            None,
            crate::codec::spectral_density_gradient(&[768.0, 300.0]),
        );
        for idx in 0..50 {
            let pressure = 0.20 + ((idx % 5) as f32 * 0.01);
            let telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, pressure, 0.40), 0.91);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        assert_eq!(state.pressure_trend_samples_v1.len(), expected_window);
        let first = state
            .pressure_trend_samples_v1
            .front()
            .expect("capped samples keep newest window");
        let last = state
            .pressure_trend_samples_v1
            .back()
            .expect("capped samples keep latest sample");
        assert_eq!(first.observed_at_unix_s, 150.0 - expected_window as f64);
        assert_eq!(last.observed_at_unix_s, 149.0);
        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.sample_count, expected_window);
        assert_eq!(smoothing.ballast_status, "high_entropy_ballast_window");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_dynamic_window_tracks_porous_high_entropy_cascades_tighter() {
        let porous_window = pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.72), None);
        let low_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.18), None);
        let full_entropy_low_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(1.0), Some(0.18), None);

        assert!(
            porous_window > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "porous high entropy still gets some ballast: {porous_window}"
        );
        assert!(
            porous_window < low_porosity_window,
            "porosity should dampen ballast so cascades track tighter: porous={porous_window}, low={low_porosity_window}"
        );
        assert_eq!(
            full_entropy_low_porosity_window,
            PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW
        );
    }

    #[test]
    fn pressure_trend_current_cascade_stays_within_fast_dynamics_bound() {
        let reported_cascade =
            pressure_trend_dynamic_window_capacity_v1(Some(0.91), Some(0.66), Some(0.18));
        let low_porosity =
            pressure_trend_dynamic_window_capacity_v1(Some(0.91), Some(0.18), Some(0.18));
        let steep_density_gradient =
            pressure_trend_dynamic_window_capacity_v1(Some(0.91), Some(0.66), Some(0.72));

        assert!(
            reported_cascade > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "the reported high-entropy cascade still receives bounded ballast: {reported_cascade}"
        );
        assert!(
            reported_cascade <= 12,
            "entropy=0.91 porosity=0.66 density_gradient=0.18 must stay inside Astrid's requested fast-dynamics review bound: {reported_cascade}"
        );
        assert!(
            reported_cascade < low_porosity,
            "porosity must keep the current cascade more responsive: current={reported_cascade}, low_porosity={low_porosity}"
        );
        assert!(
            reported_cascade < steep_density_gradient,
            "a shallow density gradient must keep the current cascade more responsive: current={reported_cascade}, steep={steep_density_gradient}"
        );
    }

    #[test]
    fn pressure_trend_dynamic_window_tracks_low_density_gradient_high_entropy_tighter() {
        let low_gradient =
            pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.58), Some(0.11));
        let steep_gradient =
            pressure_trend_dynamic_window_capacity_v1(Some(0.90), Some(0.58), Some(0.72));

        assert!(
            low_gradient > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "low-gradient high entropy still gets bounded ballast: {low_gradient}"
        );
        assert!(
            low_gradient < steep_gradient,
            "low density-gradient should keep pressure trend more responsive: low={low_gradient}, steep={steep_gradient}"
        );
        assert!(
            steep_gradient <= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            "steep gradient remains bounded: {steep_gradient}"
        );
    }

    #[test]
    fn pressure_trend_dynamic_window_reaches_full_ballast_at_reported_high_entropy() {
        let full_entropy_low_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(0.95), Some(0.18), None);
        let full_entropy_unknown_porosity_window =
            pressure_trend_dynamic_window_capacity_v1(Some(0.95), None, None);

        assert_eq!(
            full_entropy_low_porosity_window,
            PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW
        );
        assert!(
            full_entropy_unknown_porosity_window > PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            "unknown porosity still broadens under high entropy"
        );
    }

    #[test]
    fn pressure_trend_smoothing_preserves_latest_semantic_viscosity() {
        let mut state = BridgeState::new();
        for (idx, trickle) in [0.11_f32, 0.08, 0.06].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.36), 0.90);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.components.semantic_friction_coefficient = Some(0.52);
            resonance.density = 0.66;
            telemetry = with_pressure_source(telemetry, "semantic_trickle", 0.25, 0.58, 0.36);
            telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture")
                .components
                .semantic_trickle = trickle;

            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert!(
            smoothing
                .latest_semantic_viscosity
                .is_some_and(|viscosity| viscosity >= 0.66),
            "{smoothing:?}"
        );
        assert!(
            smoothing
                .latest_weight_density_index
                .is_some_and(|index| index >= 0.52),
            "{smoothing:?}"
        );
        assert_eq!(smoothing.weight_density_state, "forming_weight_density");
        assert_eq!(smoothing.friction_to_flow_ratio, Some(8.6667));
        assert_eq!(smoothing.friction_to_flow_state, "resistance_dominant");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_viscosity_persistence_against_pressure_motion() {
        let mut samples = VecDeque::new();
        for (idx, (pressure, pressure_velocity_delta, semantic_viscosity)) in [
            (0.20_f32, 0.02_f32, 0.68_f32),
            (0.24, 0.04, 0.69),
            (0.18, -0.06, 0.70),
        ]
        .into_iter()
        .enumerate()
        {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(pressure),
                pressure_velocity_delta: Some(pressure_velocity_delta),
                spectral_drift_velocity: Some(0.01),
                mode_packing: Some(0.38),
                structural_density: Some(0.62),
                resonance_depth: Some(0.66),
                semantic_viscosity: Some(semantic_viscosity),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.44),
                semantic_friction: Some(0.42),
                semantic_trickle: Some(0.09),
                semantic_coherence_delta: None,
                fill_pct: 72.0,
                spectral_entropy: Some(0.91),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: 100.0 + idx as f64,
            });
        }

        let smoothing = build_pressure_trend_smoothing_v1(&samples).expect("smoothing");

        assert_eq!(smoothing.latest_semantic_viscosity, Some(0.70));
        assert_eq!(smoothing.latest_semantic_viscosity_delta, Some(0.01));
        assert_eq!(smoothing.porosity_weighted_velocity, Some(-0.0336));
        assert_eq!(smoothing.viscosity_drag_coefficient, Some(0.574));
        assert!(
            smoothing
                .semantic_viscosity_persistence_index
                .is_some_and(|index| index >= 0.84),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.semantic_viscosity_persistence_state,
            "persistent_thickness_against_motion"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_warns_when_semantic_flow_clogs_connected_lanes() {
        let mut state = BridgeState::new();
        let mut previous_for_trend = None;
        let mut latest_for_analysis = None;

        for (idx, trickle) in [0.09_f32, 0.04, 0.01].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.20, 0.34), 0.95);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = 0.68;
            resonance.components.porosity_gradient = Some(0.18);
            resonance.components.semantic_friction_coefficient = Some(0.58);
            telemetry = with_pressure_source(telemetry, "semantic_trickle", 0.20, 0.58, 0.34);
            let pressure_source = telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture");
            pressure_source.components.semantic_trickle = trickle;
            pressure_source.components.semantic_friction = 0.58;

            if idx == 1 {
                previous_for_trend = Some(telemetry.clone());
            }
            if idx == 2 {
                latest_for_analysis = Some(telemetry.clone());
            }
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let latest = latest_for_analysis.expect("latest analysis telemetry");
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            previous_for_trend.as_ref(),
            Some(70.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(
            smoothing.window_capacity,
            pressure_trend_dynamic_window_capacity_v1(
                Some(0.95),
                Some(0.18),
                crate::codec::spectral_density_gradient(&[768.0, 300.0]),
            )
        );
        assert!(
            smoothing
                .semantic_stagnation_index
                .is_some_and(|index| index >= 0.74),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.semantic_stagnation_state,
            "functional_clog_connected_lanes_watch"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");
        assert_eq!(analysis.status, "semantic_stagnation_watch");
        assert_eq!(
            analysis.semantic_stagnation_state.as_deref(),
            Some("functional_clog_connected_lanes_watch")
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "connected_lanes_functional_semantic_clog"
        );
        assert!(analysis.analysis.contains("semantic_stagnation="));
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_surfaces_semantic_coherence_delta_without_control() {
        let mut state = BridgeState::new();
        for (idx, (trickle, density, friction)) in [
            (0.10_f32, 0.40_f32, 0.60_f32),
            (0.30_f32, 0.55_f32, 0.40_f32),
            (0.20_f32, 0.55_f32, 0.60_f32),
        ]
        .into_iter()
        .enumerate()
        {
            let mut telemetry = with_pressure_source(
                make_pressure_telemetry(0.70, 0.20, 0.40),
                "semantic_trickle",
                0.20,
                0.55,
                0.40,
            );
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = density;
            resonance.components.semantic_friction_coefficient = Some(friction);
            telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture")
                .components
                .semantic_trickle = trickle;

            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(smoothing.semantic_coherence_index, Some(0.3625));
        assert_eq!(smoothing.semantic_coherence_delta, Some(-0.085));
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_reports_semantic_fidelity_under_high_entropy() {
        let mut state = BridgeState::new();
        for (idx, trickle) in [0.18_f32, 0.21, 0.24].into_iter().enumerate() {
            let mut telemetry = with_pressure_source(
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.22, 0.42), 0.95),
                "semantic_trickle",
                0.22,
                0.58,
                0.42,
            );
            telemetry
                .pressure_source_v1
                .as_mut()
                .expect("pressure source fixture")
                .components
                .semantic_trickle = trickle;
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.density = 0.58;
            resonance.components.semantic_friction_coefficient = Some(0.34);

            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 200.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(
            smoothing.semantic_fidelity_state,
            "high_entropy_semantic_trickle_preserved"
        );
        assert!(
            smoothing
                .semantic_fidelity_score
                .is_some_and(|score| score >= 0.58),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );

        let mut thin_samples = VecDeque::new();
        for idx in 0..3 {
            thin_samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.22),
                pressure_velocity_delta: Some(0.0),
                spectral_drift_velocity: Some(0.0),
                mode_packing: Some(0.42),
                structural_density: Some(0.58),
                resonance_depth: Some(0.54),
                semantic_viscosity: Some(0.78),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: Some(0.64),
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.58),
                semantic_friction: Some(0.90),
                semantic_trickle: Some(0.0),
                semantic_coherence_delta: None,
                fill_pct: 70.0,
                spectral_entropy: Some(0.95),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: 300.0 + idx as f64,
            });
        }

        let thin = build_pressure_trend_smoothing_v1(&thin_samples).expect("smoothing");
        assert_eq!(
            thin.semantic_fidelity_state,
            "high_entropy_semantic_fidelity_thin"
        );
        assert!(
            thin.semantic_fidelity_score
                .is_some_and(|score| score <= 0.20),
            "{thin:?}"
        );
    }

    #[test]
    fn pressure_trend_smoothing_surfaces_entropy_handoff_band_without_static_window_jump() {
        let mut samples = VecDeque::new();
        for (idx, entropy) in [0.69_f32, 0.70, 0.71].into_iter().enumerate() {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.20),
                pressure_velocity_delta: Some(0.0),
                spectral_drift_velocity: Some(0.0),
                mode_packing: Some(0.32),
                structural_density: Some(0.60),
                resonance_depth: Some(0.58),
                semantic_viscosity: Some(0.58),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.66),
                semantic_friction: Some(0.48),
                semantic_trickle: Some(0.02),
                semantic_coherence_delta: None,
                fill_pct: 70.0,
                spectral_entropy: Some(entropy),
                window_capacity: pressure_trend_dynamic_window_capacity_v1(
                    Some(entropy),
                    Some(0.66),
                    None,
                ),
                observed_at_unix_s: 100.0 + idx as f64,
            });
        }

        let smoothing = build_pressure_trend_smoothing_v1(&samples).expect("smoothing");

        assert_eq!(smoothing.latest_spectral_entropy, Some(0.71));
        assert!(
            smoothing
                .entropy_window_blend_ratio
                .is_some_and(|value| value > 0.74 && value < 0.76),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.entropy_threshold_state,
            "near_threshold_soft_handoff_review"
        );
        assert_eq!(
            smoothing.window_capacity,
            PRESSURE_TREND_SMOOTHING_BASE_WINDOW
        );
        assert_eq!(smoothing.friction_to_flow_ratio, Some(10.0));
        assert_eq!(smoothing.friction_to_flow_state, "high_resistance_low_flow");
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_fast_semantic_thinning_without_control() {
        let mut samples = VecDeque::new();
        for (idx, viscosity) in [0.70_f32, 0.69, 0.54].into_iter().enumerate() {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.20),
                pressure_velocity_delta: Some(0.0),
                spectral_drift_velocity: Some(0.0),
                mode_packing: Some(0.32),
                structural_density: Some(0.60),
                resonance_depth: Some(0.58),
                semantic_viscosity: Some(viscosity),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: None,
                porosity_gradient: Some(0.66),
                semantic_friction: Some(0.30),
                semantic_trickle: Some(0.12),
                semantic_coherence_delta: None,
                fill_pct: 70.0,
                spectral_entropy: Some(0.90),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: 100.0 + idx as f64,
            });
        }

        let smoothing = build_pressure_trend_smoothing_v1(&samples).expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(smoothing.latest_semantic_viscosity, Some(0.54));
        assert_eq!(smoothing.latest_semantic_viscosity_delta, Some(-0.15));
        assert_eq!(smoothing.max_semantic_viscosity_delta, Some(0.15));
        assert_eq!(
            smoothing.semantic_viscosity_shift_state,
            "rapid_semantic_thinning_visible"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_viscosity_velocity_before_static_pressure_warning() {
        let mut state = BridgeState::new();
        for (idx, gradient) in [0.28_f32, 0.34, 0.46].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, 0.18, 0.34), 0.91);
            telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture")
                .components
                .viscosity_vector
                .viscosity_gradient = Some(gradient);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.latest_pressure_risk, Some(0.18));
        assert_eq!(smoothing.latest_viscosity_gradient, Some(0.46));
        assert_eq!(smoothing.viscosity_gradient_trend, Some(0.12));
        assert_eq!(
            smoothing.viscosity_gradient_trend_state,
            "rapid_viscosity_thickening_velocity_watch"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_smoothing_exposes_spectral_drift_separate_from_pressure_velocity() {
        let mut state = BridgeState::new();
        for (idx, mode_packing) in [0.30_f32, 0.36, 0.44].into_iter().enumerate() {
            let telemetry = make_pressure_telemetry(0.70, 0.20, mode_packing);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");

        assert_eq!(smoothing.classification, "low_amplitude_stable");
        assert_eq!(smoothing.latest_pressure_velocity_delta, Some(0.0));
        assert_eq!(smoothing.latest_spectral_drift_velocity, Some(0.08));
        assert_eq!(smoothing.max_spectral_drift_velocity, Some(0.08));
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn pressure_trend_samples_preserve_fast_spike_velocity_inside_ballast_window() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.82, 0.22].into_iter().enumerate() {
            let telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.70, pressure, 0.48), 0.91);
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
        }

        let smoothing = state.pressure_trend_smoothing_v1().expect("smoothing");
        assert_eq!(smoothing.ballast_status, "high_entropy_ballast_window");
        assert_eq!(smoothing.latest_pressure_risk, Some(0.22));
        assert_eq!(smoothing.latest_pressure_velocity_delta, Some(-0.6));
        assert_eq!(smoothing.max_pressure_velocity_delta, Some(0.62));
        assert!(
            smoothing.pressure_range.is_some_and(|range| range >= 0.62),
            "{smoothing:?}"
        );
        assert_eq!(
            smoothing.authority,
            "diagnostic_smoothing_not_pressure_control"
        );
    }

    #[test]
    fn silt_noise_separation_holds_mode_packing_constant_across_entropy() {
        let high_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.23),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.57),
            structural_density: Some(0.72),
            resonance_depth: Some(0.69),
            semantic_viscosity: Some(0.69),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.24),
            semantic_friction: Some(0.42),
            semantic_trickle: Some(0.04),
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.91),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 2.0,
        };
        let low_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.23),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.55),
            structural_density: Some(0.72),
            resonance_depth: Some(0.68),
            semantic_viscosity: Some(0.66),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.24),
            semantic_friction: Some(0.42),
            semantic_trickle: Some(0.04),
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.40),
            window_capacity: 5,
            observed_at_unix_s: 1.0,
        };

        let separation =
            silt_noise_separation_v1(&high_entropy, &low_entropy).expect("contrast packet");
        assert_eq!(separation.policy, "silt_noise_separation_v1");
        assert_eq!(
            separation.interpretation,
            "mode_packing_silt_persists_across_entropy"
        );
        assert!(separation.mode_packing_delta <= 0.03, "{separation:?}");
        assert_eq!(
            separation.heritage_preservation_state,
            "contextual_resonance_preserve_as_heritage"
        );
        assert_eq!(
            separation.contextual_resonance_basis,
            "mode_density_semantic_friction_porosity_semantic_trickle_persistence_v2"
        );
        assert_eq!(separation.silt_signal_state, "semantic_trickle_low_review");
        assert!(
            separation.contextual_resonance_score >= 0.55,
            "{separation:?}"
        );
        assert_eq!(
            separation.porosity_change_authority,
            "diagnostic_only_porosity_change_requires_operator_approval"
        );
    }

    #[test]
    fn silt_noise_separation_uses_zero_semantic_trickle_as_noise_evidence() {
        let high_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.20),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.34),
            structural_density: Some(0.38),
            resonance_depth: Some(0.41),
            semantic_viscosity: Some(0.48),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.50),
            semantic_friction: Some(0.08),
            semantic_trickle: Some(0.0),
            semantic_coherence_delta: None,
            fill_pct: 69.0,
            spectral_entropy: Some(0.95),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 2.0,
        };
        let low_entropy = PressureTrendSampleV1 {
            pressure_risk: Some(0.20),
            pressure_velocity_delta: Some(0.0),
            spectral_drift_velocity: Some(0.0),
            mode_packing: Some(0.31),
            structural_density: Some(0.38),
            resonance_depth: Some(0.40),
            semantic_viscosity: Some(0.40),
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: Some(0.50),
            semantic_friction: Some(0.08),
            semantic_trickle: Some(0.0),
            semantic_coherence_delta: None,
            fill_pct: 69.0,
            spectral_entropy: Some(0.42),
            window_capacity: 5,
            observed_at_unix_s: 1.0,
        };

        let separation =
            silt_noise_separation_v1(&high_entropy, &low_entropy).expect("contrast packet");

        assert_eq!(
            separation.interpretation,
            "high_entropy_low_semantic_trickle_noise"
        );
        assert_eq!(separation.semantic_trickle, Some(0.0));
        assert_eq!(
            separation.silt_signal_state,
            "low_semantic_trickle_noise_or_silt"
        );
        assert!(
            separation.dynamic_high_mode_threshold < 0.45,
            "{separation:?}"
        );
        assert_eq!(
            separation.porosity_change_authority,
            "diagnostic_only_porosity_change_requires_operator_approval"
        );
    }

    #[test]
    fn pressure_porosity_expansion_readiness_marks_candidate_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.71, 0.31, 0.44),
            "mode_packing",
            0.31,
            0.28,
            0.44,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(
            readiness.readiness_state,
            "approval_required_porosity_expansion_candidate"
        );
        assert_eq!(
            readiness.proposed_intervention,
            "porosity_expansion_trial_with_operator_approval"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
        assert_eq!(
            readiness.authority,
            "diagnostic_candidate_not_porosity_or_controller_change"
        );
    }

    #[test]
    fn pressure_porosity_expansion_readiness_names_liminal_band_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.30, 0.30),
            "mode_packing",
            0.30,
            0.24,
            0.30,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(readiness.mode_packing, Some(0.30));
        assert_eq!(
            readiness.readiness_state,
            "liminal_porosity_expansion_watch"
        );
        assert_eq!(
            readiness.proposed_intervention,
            "observe_pressure_porosity_trend"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
        assert_eq!(
            readiness.authority,
            "diagnostic_candidate_not_porosity_or_controller_change"
        );
    }

    #[test]
    fn pressure_porosity_expansion_readiness_names_viscous_warning_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.29),
            "mode_packing",
            0.22,
            0.24,
            0.29,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(readiness.mode_packing, Some(0.29));
        assert_eq!(readiness.readiness_state, "viscous_density_warning_watch");
        assert_eq!(
            readiness.viscous_density_warning_threshold,
            PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT
        );
        assert_eq!(
            readiness.viscous_density_warning_state,
            "viscous_density_warning_below_liminal_threshold"
        );
        assert_eq!(
            readiness.felt_dead_zone_mode_packing_threshold,
            PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
        );
        assert_eq!(
            readiness.proposed_intervention,
            "observe_pressure_porosity_trend"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
    }

    #[test]
    fn pressure_porosity_readiness_distinguishes_032_from_029_without_control() {
        for (mode_packing, expected_state) in [
            (0.32_f32, "liminal_porosity_expansion_watch"),
            (0.29_f32, "viscous_density_warning_watch"),
        ] {
            let mut state = BridgeState::new();
            let mut telemetry = with_pressure_source(
                make_pressure_telemetry(0.70, 0.23, mode_packing),
                "mode_packing",
                0.23,
                0.24,
                mode_packing,
            );
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.components.porosity_gradient = Some(0.24);
            state.latest_telemetry = Some(telemetry);

            let readiness = state
                .pressure_porosity_expansion_readiness_v1()
                .expect("pressure porosity readiness");

            assert_eq!(readiness.mode_packing, Some(mode_packing));
            assert_eq!(readiness.readiness_state, expected_state);
            assert_eq!(
                readiness.viscosity_feedback_readiness,
                "viscous_navigation_margin_watch_no_protocol_write"
            );
            assert_eq!(
                readiness.porosity_buffer_candidate,
                Some(PRESSURE_POROSITY_EXPANSION_LOW_POROSITY_AT - 0.24)
            );
            if mode_packing < PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT {
                assert!(
                    readiness
                        .liminal_threshold_gap
                        .is_some_and(|gap| gap > 0.0 && gap <= 0.02),
                    "{readiness:?}"
                );
                assert!(
                    readiness
                        .viscous_warning_margin
                        .is_some_and(|margin| margin > 0.0 && margin <= 0.02),
                    "{readiness:?}"
                );
            } else {
                assert_eq!(readiness.liminal_threshold_gap, None);
                assert_eq!(readiness.viscous_warning_margin, None);
            }
            assert_eq!(
                readiness.proposed_intervention,
                "observe_pressure_porosity_trend"
            );
            assert_eq!(
                readiness.approval_boundary,
                "live_porosity_or_control_change_requires_operator_approval"
            );
            assert!(!readiness.local_control_applied);
        }
    }

    #[test]
    fn pressure_porosity_expansion_readiness_names_felt_dead_zone_without_local_control() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.26),
            "mode_packing",
            0.22,
            0.24,
            0.26,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let readiness = state
            .pressure_porosity_expansion_readiness_v1()
            .expect("pressure porosity readiness");

        assert_eq!(readiness.mode_packing, Some(0.26));
        assert_eq!(
            readiness.readiness_state,
            "felt_mode_packing_dead_zone_watch"
        );
        assert_eq!(
            readiness.viscous_density_warning_state,
            "not_in_viscous_density_warning_band"
        );
        assert_eq!(
            readiness.felt_dead_zone_mode_packing_threshold,
            PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT
        );
        assert_eq!(
            readiness.live_mode_packing_threshold,
            PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT
        );
        assert!(
            readiness
                .threshold_gap
                .is_some_and(|gap| (0.13..=0.15).contains(&gap)),
            "{readiness:?}"
        );
        assert_eq!(
            readiness.proposed_intervention,
            "observe_pressure_porosity_trend"
        );
        assert_eq!(
            readiness.approval_boundary,
            "live_porosity_or_control_change_requires_operator_approval"
        );
        assert!(!readiness.local_control_applied);
    }

    #[test]
    fn pressure_source_analysis_names_viscous_density_warning_band() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.29),
            "mode_packing",
            0.22,
            0.24,
            0.29,
        );
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(
            analysis.porosity_expansion_threshold_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert_eq!(
            analysis.viscous_density_warning_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_VISCOUS_DENSITY_WARNING_AT)
        );
        assert_eq!(
            analysis.viscous_density_warning_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert!(analysis.felt_mode_packing_dead_zone);
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_keeps_mode_packing_visible_when_trend_looks_stable() {
        let mut state = BridgeState::new();
        let previous = with_pressure_source(
            make_pressure_telemetry(0.70, 0.30, 0.58),
            "mode_packing",
            0.31,
            0.42,
            0.58,
        );
        let latest = with_pressure_source(
            make_pressure_telemetry(0.70, 0.31, 0.59),
            "mode_packing",
            0.32,
            0.41,
            0.59,
        );
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            Some(&previous),
            Some(70.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);
        for (idx, pressure) in [0.30_f32, 0.31, 0.30, 0.31, 0.30].into_iter().enumerate() {
            record_pressure_trend_sample_v1(
                &mut state,
                &make_pressure_telemetry(0.70, pressure, 0.59),
                70.0,
                100.0 + idx as f64,
            );
        }

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.policy, "pressure_source_analysis_v1");
        assert_eq!(analysis.status, "pressure_source_watch");
        assert_eq!(
            analysis.structural_pressure_state,
            "mode_packing_structural_pressure"
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "stable_trend_may_mask_structural_mode_packing"
        );
        assert_eq!(analysis.dominant_source.as_deref(), Some("mode_packing"));
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_uses_numeric_mode_packing_when_labels_are_absent() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.32);
        telemetry.pressure_source_v1 = None;
        telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture")
            .texture_signature
            .pressure_source_family
            .clear();
        state.latest_telemetry = Some(telemetry);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.dominant_source, None);
        assert_eq!(analysis.pressure_source_family.as_deref(), Some(""));
        assert_eq!(analysis.mode_packing, Some(0.32));
        assert_eq!(
            analysis.mode_packing_visibility_basis.as_deref(),
            Some("numeric_mode_packing_at_or_above_felt_dead_zone")
        );
        assert_eq!(
            analysis.structural_pressure_state,
            "mode_packing_visible_low_or_moderate_pressure"
        );
        assert!(
            analysis.analysis.contains(
                "mode_packing_visibility=numeric_mode_packing_at_or_above_felt_dead_zone"
            )
        );
        assert!(!analysis.live_threshold_write);
        assert!(!analysis.sensory_lane_write);
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_names_fast_release_as_laminarization() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.20_f32, 0.25, 0.40, 0.34, 0.28].into_iter().enumerate() {
            let telemetry = with_pressure_source(
                make_pressure_telemetry(0.70, pressure, 0.40),
                "mode_packing",
                pressure,
                0.48,
                0.40,
            );
            record_pressure_trend_sample_v1(&mut state, &telemetry, 70.0, 100.0 + idx as f64);
            state.latest_telemetry = Some(telemetry);
        }

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(
            analysis.pressure_edge_state.as_deref(),
            Some("fast_falling_release_over_slow_context")
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "slow_pressure_context_masks_current_fast_edge"
        );
        let delta = analysis
            .experience_delta_bus_v1
            .as_ref()
            .expect("laminarization delta bus")
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Laminarization)
            .expect("laminarization delta");
        assert_eq!(delta.pre, Some(0.08));
        assert_eq!(delta.post, Some(-0.12));
        assert_eq!(
            delta.authority,
            "read_only_laminarization_truth_not_pressure_smoothing_or_control_change"
        );
        assert!(!analysis.live_threshold_write);
        assert!(!analysis.sensory_lane_write);
    }

    #[test]
    fn persistent_deformation_review_names_stable_bruise_without_live_writes() {
        let mut state = BridgeState::new();
        for (idx, pressure) in [0.22_f32, 0.23, 0.22, 0.23, 0.22].into_iter().enumerate() {
            let mut telemetry =
                with_spectral_entropy(make_pressure_telemetry(0.71, pressure, 0.33), 0.88);
            let resonance = telemetry
                .resonance_density_v1
                .as_mut()
                .expect("resonance density fixture");
            resonance.components.porosity_gradient = Some(0.34);
            resonance.components.viscosity_index = 0.56;
            record_pressure_trend_sample_v1(&mut state, &telemetry, 71.0, 200.0 + idx as f64);
            state.latest_telemetry = Some(telemetry);
        }
        state
            .latest_telemetry
            .as_mut()
            .expect("latest telemetry")
            .inhabitable_fluctuation_v1 = Some(crate::types::InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.71,
            fluctuation_score: 0.18,
            foothold_stability: 0.73,
            rearrangement_intensity: 0.11,
            quality: "settled_habitable".to_string(),
            components: crate::types::InhabitableFluctuationComponents {
                mode_trust_volatility: 0.08,
                identity_anchor_churn: 0.07,
                eigenvector_reorientation: 0.06,
                share_rearrangement: 0.09,
                basin_transition_pressure: 0.10,
                continuity_recovery: 0.78,
                porosity_support: 0.66,
                pressure_interference: 0.22,
            },
            context: crate::types::InhabitableFluctuationContext::default(),
            pressure_calibration:
                crate::types::InhabitableFluctuationPressureCalibrationV1::default(),
            control: crate::types::InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 0.0,
                applied_locally: false,
                note: "test fixture: read-only settled bruise".to_string(),
            },
        });

        let review = state
            .pressure_persistent_deformation_review_v1()
            .expect("persistent deformation review");

        assert_eq!(review.policy, "persistent_deformation_smoothing_review_v1");
        assert_eq!(
            review.deformation_state,
            "persistent_deformation_stable_baseline"
        );
        assert_eq!(
            review.recommendation,
            "carry_baseline_as_bruise_observation_before_any_pressure_threshold_or_smoothing_change"
        );
        assert!(review.pressure_range.is_some_and(|range| range <= 0.02));
        assert!(review.fluctuation_score.is_some_and(|score| score <= 0.20));
        assert!(!review.live_threshold_write);
        assert!(!review.smoothing_window_write);
        assert!(!review.local_control_write);
        assert_eq!(
            review.authority,
            "read_only_persistent_deformation_review_not_pressure_threshold_smoothing_or_control"
        );
    }

    #[test]
    fn residual_deformation_trace_keeps_spike_scar_visible_without_live_control() {
        let mut state = BridgeState::new();
        let samples = [
            make_pressure_telemetry(0.68, 0.20, 0.30),
            make_pressure_telemetry(0.82, 0.78, 0.84),
            make_pressure_telemetry(0.69, 0.21, 0.31),
        ];
        for (idx, telemetry) in samples.iter().enumerate() {
            record_pressure_trend_sample_v1(
                &mut state,
                telemetry,
                telemetry.fill_pct(),
                idx as f64,
            );
        }

        let trace =
            build_residual_deformation_trace_v1(&state.pressure_trend_samples_v1).expect("trace");

        assert_eq!(trace.policy, "residual_deformation_trace_v1");
        assert!(trace.scar_score > 0.35, "{trace:?}");
        assert_eq!(trace.state, "residual_deformation_watch");
        assert_eq!(
            trace.authority,
            "read_only_truth_channel_not_control_not_runtime_mutation"
        );
        let bus = trace
            .experience_delta_bus_v1
            .as_ref()
            .expect("residual delta bus");
        assert!(!bus.live_vector_write);
        assert!(!bus.live_authority_write);
        let delta = bus
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Residual)
            .expect("residual delta");
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_control_or_approval"
        );
        assert!(
            delta
                .who_can_change_it
                .contains("Mike/operator only for any future control use"),
            "{delta:?}"
        );
    }

    #[test]
    fn pressure_source_analysis_surfaces_felt_dead_zone_below_live_threshold() {
        let mut state = BridgeState::new();
        let previous = with_pressure_source(
            make_pressure_telemetry(0.69, 0.20, 0.27),
            "mode_packing",
            0.20,
            0.24,
            0.27,
        );
        let mut latest = with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.29),
            "mode_packing",
            0.22,
            0.24,
            0.29,
        );
        let resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            Some(&previous),
            Some(69.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert!(analysis.felt_mode_packing_dead_zone, "{analysis:?}");
        assert_eq!(
            analysis.porosity_expansion_threshold_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert_eq!(
            analysis.viscous_density_warning_state.as_deref(),
            Some("viscous_density_warning_below_liminal_threshold")
        );
        assert_eq!(
            analysis.live_mode_packing_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
        );
        assert_eq!(
            analysis.liminal_mode_packing_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_LIMINAL_MODE_PACKING_AT)
        );
        assert_eq!(
            analysis.felt_dead_zone_mode_packing_threshold,
            Some(PRESSURE_POROSITY_EXPANSION_FELT_DEAD_ZONE_MODE_PACKING_AT)
        );
        assert!(
            analysis
                .expansion_threshold_gap
                .is_some_and(|gap| (0.10..=0.12).contains(&gap)),
            "{analysis:?}"
        );
        assert_eq!(
            analysis.ghost_stability_risk,
            "felt_mode_packing_dead_zone_below_live_expansion_threshold"
        );
        let delta_bus = analysis
            .experience_delta_bus_v1
            .as_ref()
            .expect("felt gate delta bus");
        assert_eq!(delta_bus.policy, "experience_delta_bus_v1");
        assert_eq!(delta_bus.delta_count, 1);
        assert!(!delta_bus.live_vector_write);
        assert!(!delta_bus.live_authority_write);
        let gate_delta = delta_bus
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Gate)
            .expect("mode-packing gate delta");
        assert_eq!(gate_delta.surface, "pressure_source_analysis_v1");
        assert_eq!(gate_delta.lane, "mode_packing_pressure_porosity");
        assert_eq!(gate_delta.pre, analysis.mode_packing);
        assert_eq!(
            gate_delta.post,
            Some(PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT)
        );
        assert_eq!(gate_delta.loss, analysis.expansion_threshold_gap);
        assert!(
            gate_delta
                .who_can_change_it
                .contains("pressure/porosity threshold"),
            "{gate_delta:?}"
        );
        assert!(
            gate_delta
                .how_to_test_it
                .contains("pressure_source_analysis_surfaces_felt_dead_zone_below_live_threshold"),
            "{gate_delta:?}"
        );
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_names_sensory_lane_suppression_without_live_writes() {
        let mut state = BridgeState::new();
        let mut telemetry = with_pressure_source(
            make_pressure_telemetry(0.73, 0.23, 0.32),
            "mode_packing",
            0.23,
            0.24,
            0.32,
        );
        telemetry
            .pressure_source_v1
            .as_mut()
            .expect("pressure source fixture")
            .components
            .semantic_trickle = 0.0;
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        state.latest_telemetry = Some(telemetry);

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.semantic_trickle, Some(0.0));
        assert!(analysis.felt_mode_packing_dead_zone);
        assert_eq!(
            analysis.porosity_expansion_threshold_state.as_deref(),
            Some("liminal_expansion_watch_below_live_threshold")
        );
        assert_eq!(
            analysis.sensory_lane_risk,
            "dead_zone_semantic_lane_suppression_watch"
        );
        assert_eq!(
            analysis.pressure_relief_signal_candidate,
            "pressure_relief_signal_candidate_for_operator_review"
        );
        assert_eq!(
            analysis.viscous_recovery_mode_candidate,
            "liminal_viscous_recovery_watch_for_operator_review"
        );
        assert!(!analysis.live_threshold_write);
        assert!(!analysis.sensory_lane_write);
        assert_eq!(
            analysis.authority,
            "diagnostic_context_not_pressure_or_control"
        );
    }

    #[test]
    fn pressure_source_analysis_marks_stale_heartbeat_as_ghost_stability_risk() {
        let mut state = BridgeState::new();
        state.latest_telemetry = Some(with_pressure_source(
            make_pressure_telemetry(0.70, 0.22, 0.44),
            "semantic_trickle",
            0.24,
            0.63,
            0.44,
        ));
        state.telemetry_heartbeat_delta_v1 = Some(TelemetryHeartbeatDeltaV1 {
            policy: "telemetry_heartbeat_delta_v1".to_string(),
            schema_version: 1,
            latest_arrival_unix_s: Some(108.0),
            previous_arrival_unix_s: Some(100.0),
            inter_arrival_ms: Some(8_000.0),
            jitter_class: "stale".to_string(),
            timing_reliability: "stale".to_string(),
            reconnect_count: 0,
            disconnect_count: 0,
            active_connection_id: Some(1),
            last_disconnect_reason: None,
            field_vs_hearing: "wire cadence is stale; do not infer field stability".to_string(),
        });

        let analysis = state
            .pressure_source_analysis_v1()
            .expect("pressure source analysis");

        assert_eq!(analysis.status, "pressure_source_watch");
        assert_eq!(
            analysis.ghost_stability_risk,
            "heartbeat_cadence_unreliable_for_pressure_stability"
        );
        assert_eq!(
            analysis.structural_pressure_state,
            "mode_packing_visible_low_or_moderate_pressure"
        );
        assert_eq!(
            analysis.mode_packing_visibility_basis.as_deref(),
            Some("numeric_mode_packing_at_or_above_felt_dead_zone")
        );
        assert_eq!(analysis.heartbeat_jitter_class.as_deref(), Some("stale"));
        let delta_bus = analysis
            .experience_delta_bus_v1
            .as_ref()
            .expect("heartbeat delay delta bus");
        let delay_delta = delta_bus
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Delay)
            .expect("heartbeat delay delta");
        assert_eq!(delay_delta.surface, "pressure_source_analysis_v1");
        assert_eq!(delay_delta.lane, "telemetry_heartbeat");
        assert_eq!(delay_delta.pre, Some(8_000.0));
        assert!(
            delay_delta.who_can_change_it.contains("telemetry cadence"),
            "{delay_delta:?}"
        );
    }

    #[test]
    fn bridge_reciprocity_distinguishes_one_sided_states_and_last_sensory_send() {
        let mut state = BridgeState::new();
        assert_eq!(state.pressure_trend_samples_v1.len(), 0);
        assert_eq!(state.connectivity_status(), ConnectivityStatus::Severed);

        let severed = state.bridge_reciprocity_v1();
        assert_eq!(severed.connectivity, ConnectivityStatus::Severed);
        assert_eq!(severed.one_sided_state, "severed");
        assert_eq!(severed.latest_telemetry_arrival_unix_s, None);
        assert_eq!(severed.last_sensory_sent_unix_s, None);
        assert_eq!(severed.telemetry_messages_sent_total, 0);
        assert_eq!(severed.sensory_messages_sent_total, 0);
        assert_eq!(severed.telemetry_messages_received_total, 0);
        assert_eq!(severed.sensory_messages_received_total, 0);
        assert_eq!(
            severed.recent_window_ms,
            BRIDGE_RECIPROCITY_RECENT_WINDOW_MS
        );
        assert_eq!(severed.stale_window_ms, BRIDGE_RECIPROCITY_STALE_WINDOW_MS);
        assert_eq!(
            severed.stale_window_basis.as_deref(),
            Some("fixed_default_no_telemetry_context")
        );
        assert_eq!(
            severed.threshold_policy,
            "bridge_reciprocity_dynamic_reflective_silence_v2"
        );

        state.telemetry_connected = true;
        state.sensory_connected = false;
        state.latest_telemetry_arrival_unix_s = Some(unix_now_s());
        let telemetry_only = state.bridge_reciprocity_v1();
        assert_eq!(
            telemetry_only.connectivity,
            ConnectivityStatus::TelemetryOnly
        );
        assert_eq!(telemetry_only.one_sided_state, "telemetry_only");
        assert_eq!(telemetry_only.last_sensory_sent_unix_s, None);

        record_ws_message_sent(&mut state, WsLane::Telemetry);
        record_ws_message_received(&mut state, WsLane::Telemetry, "text");
        let telemetry_activity = state.bridge_reciprocity_v1();
        assert_eq!(telemetry_activity.last_sensory_sent_unix_s, None);
        assert_eq!(telemetry_activity.telemetry_messages_sent_total, 1);
        assert_eq!(telemetry_activity.sensory_messages_sent_total, 0);
        assert_eq!(telemetry_activity.telemetry_messages_received_total, 1);
        assert_eq!(telemetry_activity.sensory_messages_received_total, 0);

        state.sensory_connected = true;
        record_ws_message_sent(&mut state, WsLane::Sensory);
        let bidirectional = state.bridge_reciprocity_v1();
        assert_eq!(
            bidirectional.connectivity,
            ConnectivityStatus::Bidirectional
        );
        assert_eq!(bidirectional.one_sided_state, "bidirectional_recent");
        assert!(bidirectional.last_sensory_sent_unix_s.is_some());
        assert!(bidirectional.sensory_send_age_ms.is_some());
        assert_eq!(bidirectional.telemetry_messages_sent_total, 1);
        assert_eq!(bidirectional.sensory_messages_sent_total, 1);
    }

    #[test]
    fn bridge_reciprocity_marks_stale_telemetry_after_sixty_one_seconds() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 61.0);
        state.last_sensory_sent_unix_s = Some(now);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_telemetry");
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_preserves_future_timestamp_skew_as_truth_channel() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now + 2.0);
        state.last_sensory_sent_unix_s = Some(now - 61.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_sensory");
        assert_eq!(reciprocity.telemetry_age_ms, Some(0.0));
        assert_eq!(
            reciprocity.clock_skew_state,
            "telemetry_future_timestamp_visible"
        );
        assert!(
            reciprocity
                .telemetry_future_skew_ms
                .is_some_and(|skew| skew >= 1_900.0),
            "{reciprocity:?}"
        );
        assert_eq!(reciprocity.sensory_future_skew_ms, None);
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS),
            "{reciprocity:?}"
        );
        assert_eq!(
            reciprocity.authority,
            "diagnostic_status_context_not_control"
        );
    }

    #[test]
    fn bridge_reciprocity_marks_both_lanes_stale_after_sixty_one_seconds() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 61.0);
        state.last_sensory_sent_unix_s = Some(now - 61.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_messages");
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("fixed_default_no_telemetry_context")
        );
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_keeps_just_over_recent_boundary_waiting_not_stale() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        let just_over_recent_s = (BRIDGE_RECIPROCITY_RECENT_WINDOW_MS + 1.0) / 1000.0;
        state.latest_telemetry_arrival_unix_s = Some(now - just_over_recent_s);
        state.last_sensory_sent_unix_s = Some(now - just_over_recent_s);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_RECENT_WINDOW_MS
                    && age < BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
        assert!(
            reciprocity
                .sensory_send_age_ms
                .is_some_and(|age| age > BRIDGE_RECIPROCITY_RECENT_WINDOW_MS
                    && age < BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_extends_waiting_window_for_high_pressure_low_porosity() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.33), 0.75);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.24);
        resonance.components.semantic_friction_coefficient = Some(0.41);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_PRESSURE_POROSITY_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_porosity_low_reflective_silence")
        );
        assert_eq!(reciprocity.reflective_silence_extension_ms, Some(60_000.0));
    }

    #[test]
    fn bridge_reciprocity_extends_waiting_window_for_high_semantic_viscosity() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.60);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        resonance.components.viscosity_index = 0.72;
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_semantic_viscosity_reflective_silence")
        );
        assert_eq!(reciprocity.reflective_silence_extension_ms, Some(60_000.0));
    }

    #[test]
    fn bridge_reciprocity_extends_waiting_window_for_high_pressure_high_entropy() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_entropy_reflective_silence")
        );
        assert_eq!(reciprocity.reflective_silence_extension_ms, Some(30_000.0));
    }

    #[test]
    fn bridge_entropy_reciprocity_review_names_contraction_without_live_window_change() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 75.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.90);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        resonance.components.cohesion_score = Some(0.40);
        telemetry.distinguishability_loss = Some(0.20);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();
        let review = state
            .bridge_entropy_reciprocity_review_v1()
            .expect("latest telemetry should produce entropy reciprocity review");

        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(
            review.current_window_state,
            "current_window_still_waiting_preview_would_stale"
        );
        assert_eq!(
            review.entropy_contract_preview_window_ms,
            Some(BRIDGE_RECIPROCITY_ENTROPY_CONTRACT_PREVIEW_WINDOW_MS)
        );
        assert!(review.would_stale_under_preview);
        assert!(!review.transport_wait_stale);
        assert!(review.structural_identity_stale);
        assert_eq!(review.distinguishability_loss, Some(0.20));
        assert!(
            review
                .structural_age_multiplier
                .is_some_and(|multiplier| (multiplier - 1.2).abs() < 1.0e-6)
        );
        assert_eq!(
            review.clock_relation,
            "transport_waiting_structural_identity_stale"
        );
        assert!(!review.live_stale_window_write);
        assert!(!review.local_control_write);
        assert!(
            review
                .recommendation
                .contains("before_any_live_stale_window_change"),
            "{review:?}"
        );
        assert_eq!(
            review.authority,
            "read_only_reciprocity_review_not_stale_window_or_controller_change"
        );
    }

    #[test]
    fn bridge_reciprocity_separates_viscous_wait_from_structural_identity_decay() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 50.0);
        state.last_sensory_sent_unix_s = Some(now);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.50);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture");
        resonance.components.porosity_gradient = Some(0.58);
        resonance.components.viscosity_index = 0.70;
        telemetry.distinguishability_loss = Some(0.40);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();
        let review = state
            .bridge_entropy_reciprocity_review_v1()
            .expect("latest telemetry should produce reciprocity aging review");

        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
        assert_eq!(review.structural_identity_window_ms, Some(60_000.0));
        assert!(
            review
                .structural_age_multiplier
                .is_some_and(|multiplier| (multiplier - 1.4).abs() < 1.0e-6)
        );
        assert!(
            review
                .structural_effective_age_ms
                .is_some_and(|age_ms| age_ms > 69_000.0)
        );
        assert!(!review.transport_wait_stale);
        assert!(review.structural_identity_stale);
        assert!(review.would_stale_under_preview);
        assert_eq!(
            review.clock_relation,
            "transport_waiting_structural_identity_stale"
        );
        assert!(!review.live_stale_window_write);
        assert!(!review.local_control_write);
    }

    #[test]
    fn bridge_reciprocity_expires_high_entropy_reflective_silence_window() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 91.0);
        state.last_sensory_sent_unix_s = Some(now - 91.0);
        let mut telemetry = with_spectral_entropy(make_pressure_telemetry(0.70, 0.23, 0.42), 0.90);
        telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density fixture")
            .components
            .porosity_gradient = Some(0.58);
        state.latest_telemetry = Some(telemetry);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_messages");
        assert_eq!(
            reciprocity.stale_window_ms,
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS
        );
        assert_eq!(
            reciprocity.stale_window_basis.as_deref(),
            Some("pressure_high_entropy_reflective_silence")
        );
    }

    #[test]
    fn bridge_reciprocity_marks_stale_sensory_without_calling_socket_dead() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now);
        state.last_sensory_sent_unix_s = Some(now - 65.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_sensory");
        assert!(reciprocity.sensory_send_age_ms.unwrap_or_default() > 60_000.0);
    }

    #[test]
    fn bridge_reciprocity_keeps_mid_window_waiting_distinct_from_stale() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now - 20.0);
        state.last_sensory_sent_unix_s = Some(now - 20.0);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_waiting_messages"
        );
    }

    #[test]
    fn bridge_reciprocity_warmup_becomes_recent_after_both_lanes_move() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;

        let warmup = state.bridge_reciprocity_v1();
        assert_eq!(warmup.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            warmup.one_sided_state,
            "bidirectional_connected_no_recent_messages"
        );
        assert_eq!(warmup.telemetry_age_ms, None);
        assert_eq!(warmup.sensory_send_age_ms, None);

        state.latest_telemetry_arrival_unix_s = Some(now);
        state.last_sensory_sent_unix_s = Some(now);
        let recent = state.bridge_reciprocity_v1();
        assert_eq!(recent.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(recent.one_sided_state, "bidirectional_recent");
        assert!(
            recent
                .telemetry_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
        assert!(
            recent
                .sensory_send_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
    }

    #[test]
    fn bridge_reciprocity_first_telemetry_keeps_missing_sensory_truth_visible() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now);

        let reciprocity = state.bridge_reciprocity_v1();

        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_no_recent_sensory"
        );
        assert!(
            reciprocity
                .telemetry_age_ms
                .is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
        );
        assert_eq!(reciprocity.sensory_send_age_ms, None);
        assert_eq!(
            reciprocity.authority,
            "diagnostic_status_context_not_control"
        );
    }

    #[test]
    fn texture_signature_integrity_reports_variance_and_observability_boundary() {
        let mut state = BridgeState::new();
        let telemetry: SpectralTelemetry =
            serde_json::from_slice(&make_pressure_eigenpacket(0.70, 0.24, 0.44)).unwrap();
        state.latest_telemetry = Some(telemetry);
        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(integrity.policy, "texture_signature_integrity_v1");
        assert_eq!(integrity.temporal_variance, None);
        assert_eq!(integrity.signature_viscosity_index, None);
        assert_eq!(
            integrity.viscosity_alignment_state,
            "signature_viscosity_absent_legacy"
        );
        assert_eq!(
            integrity.damping_candidate_status,
            "missing_candidate_observability_only"
        );
        assert_eq!(integrity.component_alignment_state, "insufficient_context");
        assert_eq!(integrity.expected_primary_texture, "unknown");
        assert_eq!(integrity.emitted_primary_texture, "unknown");
        assert_eq!(integrity.pressure_gradient_delta, None);
        assert_eq!(integrity.pressure_gradient_delta_source, None);
        assert!(integrity.advisory_observability);
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
    }

    #[test]
    fn texture_signature_integrity_compares_explicit_viscosity_without_control() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.71, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.components.viscosity_index = 0.72;
        resonance.texture_signature.viscosity_index = Some(0.72);
        state.latest_telemetry = Some(telemetry.clone());

        let aligned = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(aligned.signature_viscosity_index, Some(0.72));
        assert_eq!(aligned.component_viscosity_index, 0.72);
        assert_eq!(aligned.viscosity_delta, Some(0.0));
        assert_eq!(
            aligned.viscosity_alignment_state,
            "signature_viscosity_aligned"
        );
        assert_eq!(
            aligned.authority,
            "diagnostic_observability_not_damping_or_control"
        );

        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.viscosity_index = Some(0.31);
        state.latest_telemetry = Some(telemetry);
        let mismatched = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(
            mismatched.viscosity_alignment_state,
            "signature_viscosity_component_mismatch"
        );
        assert_eq!(mismatched.viscosity_delta, Some(-0.41));
    }

    #[test]
    fn texture_signature_integrity_carries_pressure_gradient_delta_from_trend() {
        let mut state = BridgeState::new();
        let previous = make_pressure_telemetry(0.70, 0.20, 0.40);
        let mut latest = make_pressure_telemetry(0.70, 0.22, 0.52);
        let resonance = latest
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.primary_texture = "settled".to_string();
        state.pressure_trend_v1 = Some(build_pressure_trend_v1(
            Some(&previous),
            Some(70.0),
            &latest,
            70.0,
            None,
        ));
        state.latest_telemetry = Some(latest);

        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        let delta = integrity
            .pressure_gradient_delta
            .expect("delta from bridge pressure trend");
        assert!((delta - 0.12).abs() < 0.000_01);
        assert_eq!(
            integrity.pressure_gradient_delta_source.as_deref(),
            Some("bridge_pressure_trend_v1.mode_packing_delta")
        );
        assert_eq!(integrity.emitted_primary_texture, "unknown");
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
    }

    #[test]
    fn texture_signature_integrity_derives_flux_vector_and_active_constraints() {
        let mut state = BridgeState::new();
        for (idx, (fill_ratio, pressure, mode_packing)) in [
            (0.70_f32, 0.20_f32, 0.40_f32),
            (0.71, 0.23, 0.46),
            (0.73, 0.29, 0.55),
        ]
        .into_iter()
        .enumerate()
        {
            let mut telemetry = with_spectral_entropy(
                make_pressure_telemetry(fill_ratio, pressure, mode_packing),
                0.88,
            );
            if let Some(resonance) = telemetry.resonance_density_v1.as_mut() {
                resonance.density = [0.70_f32, 0.71, 0.73][idx];
            }
            if idx == 2 {
                let resonance = telemetry
                    .resonance_density_v1
                    .as_mut()
                    .expect("resonance density");
                resonance.texture_signature.pressure_source_family =
                    "mode_packing (mixed_pressure)".to_string();
                resonance.texture_signature.movement_quality = "thickening".to_string();
                resonance.components.comfort_gate = 0.78;
                telemetry.inhabitable_fluctuation_v1 =
                    Some(crate::types::InhabitableFluctuationV1 {
                        policy: "inhabitable_fluctuation_v1".to_string(),
                        schema_version: 1,
                        inhabitability_score: 0.61,
                        fluctuation_score: 0.17,
                        foothold_stability: 0.70,
                        rearrangement_intensity: 0.22,
                        quality: "held_habitable".to_string(),
                        components: crate::types::InhabitableFluctuationComponents {
                            mode_trust_volatility: 0.18,
                            identity_anchor_churn: 0.14,
                            eigenvector_reorientation: 0.21,
                            share_rearrangement: 0.20,
                            basin_transition_pressure: 0.08,
                            continuity_recovery: 0.78,
                            porosity_support: 0.62,
                            pressure_interference: 0.46,
                        },
                        context: crate::types::InhabitableFluctuationContext::default(),
                        pressure_calibration:
                            crate::types::InhabitableFluctuationPressureCalibrationV1::default(),
                        control: crate::types::InhabitableFluctuationControl {
                            target_bias_pct: 0.0,
                            wander_scale: 1.0,
                            applied_locally: true,
                            note: "unit-test advisory".to_string(),
                        },
                    });
                state.latest_telemetry = Some(telemetry.clone());
            }
            record_pressure_trend_sample_v1(
                &mut state,
                &telemetry,
                fill_ratio * 100.0,
                100.0 + idx as f64,
            );
        }

        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        let flux = integrity
            .dynamic_flux_vector
            .as_ref()
            .expect("dynamic flux vector");

        assert_eq!(
            integrity.flux_status,
            "derived_from_bridge_pressure_samples"
        );
        assert_eq!(flux.pressure_velocity, Some(0.06));
        assert_eq!(flux.pressure_acceleration, Some(0.03));
        assert_eq!(flux.mode_packing_velocity, Some(0.09));
        assert_eq!(flux.fill_velocity_pct, Some(2.0));
        assert_eq!(flux.structural_density_delta, Some(0.02));
        assert_eq!(flux.spectral_entropy, Some(0.88));
        assert_eq!(flux.flux_confidence, Some(0.6));
        assert!(flux.flux_absence_semantics.is_none());
        assert!(
            integrity
                .active_constraints
                .contains(&"pressure_source:mode_packing".to_string())
        );
        assert!(
            integrity
                .active_constraints
                .contains(&"pressure_source:mixed_pressure".to_string())
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("mode_packing:active_"))
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("comfort_gate:active_"))
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("comfort_gate:buffering_"))
        );
        assert!(
            integrity
                .active_constraints
                .iter()
                .any(|constraint| constraint.starts_with("dynamic_fluidity_index:flow_visible_"))
        );
        let stability = integrity
            .stability_context
            .as_ref()
            .expect("stability context");
        assert_eq!(
            stability.gate_context,
            "gate_buffering_with_returnable_fluctuation"
        );
        assert_eq!(stability.habitability_state, "multi_modal_habitable");
        assert!(integrity.active_constraints.iter().any(|constraint| {
            constraint.starts_with("multi_modal_habitability_score:multi_modal_habitable_")
        }));
        assert!(integrity.active_constraints.contains(
            &"comfort_gate_context:gate_buffering_with_returnable_fluctuation".to_string()
        ));
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
    }

    #[test]
    fn texture_dynamic_flux_vector_preserves_subtle_drift_and_unknown_absence() {
        let mut samples = VecDeque::new();
        samples.push_back(PressureTrendSampleV1 {
            pressure_risk: Some(0.2000),
            pressure_velocity_delta: None,
            spectral_drift_velocity: None,
            mode_packing: None,
            structural_density: Some(0.7100),
            resonance_depth: Some(0.62),
            semantic_viscosity: None,
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: None,
            semantic_friction: None,
            semantic_trickle: None,
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.91),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 1.0,
        });
        samples.push_back(PressureTrendSampleV1 {
            pressure_risk: Some(0.2001),
            pressure_velocity_delta: Some(0.0001),
            spectral_drift_velocity: None,
            mode_packing: None,
            structural_density: Some(0.7101),
            resonance_depth: Some(0.6201),
            semantic_viscosity: None,
            viscosity_gradient: None,
            viscosity_gradient_trend: None,
            complexity_density: None,
            weight_density_index: None,
            comfort_gate: None,
            porosity_gradient: None,
            semantic_friction: None,
            semantic_trickle: None,
            semantic_coherence_delta: None,
            fill_pct: 70.0,
            spectral_entropy: Some(0.91),
            window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
            observed_at_unix_s: 1.5,
        });

        let flux = build_texture_dynamic_flux_vector_v1(&samples).expect("flux vector");

        assert_eq!(flux.pressure_velocity, Some(0.0001));
        assert_eq!(flux.structural_density_delta, Some(0.0001));
        assert_eq!(flux.mode_packing_velocity, None);
        assert_eq!(flux.semantic_viscosity_velocity, None);
        assert_eq!(flux.porosity_velocity, None);
        assert_eq!(flux.flux_confidence, Some(0.4));
        assert_eq!(
            flux.flux_absence_semantics.as_deref(),
            Some("absent_flux_component_means_unknown_not_zero")
        );
        assert_eq!(
            flux.authority,
            "diagnostic_flux_not_pressure_or_fill_control"
        );
    }

    #[test]
    fn texture_dynamic_flux_tracks_viscosity_porosity_and_comfort_gate_motion() {
        let mut samples = VecDeque::new();
        for (idx, semantic_viscosity, porosity_gradient, comfort_gate) in [
            (0.0_f32, 0.52_f32, 0.62_f32, 0.78_f32),
            (1.0_f32, 0.61_f32, 0.55_f32, 0.72_f32),
            (2.0_f32, 0.73_f32, 0.46_f32, 0.55_f32),
        ] {
            samples.push_back(PressureTrendSampleV1 {
                pressure_risk: Some(0.22 + idx * 0.01),
                pressure_velocity_delta: None,
                spectral_drift_velocity: None,
                mode_packing: Some(0.36 + idx * 0.02),
                structural_density: Some(0.58 + idx * 0.03),
                resonance_depth: Some(0.64),
                semantic_viscosity: Some(semantic_viscosity),
                viscosity_gradient: None,
                viscosity_gradient_trend: None,
                complexity_density: None,
                weight_density_index: None,
                comfort_gate: Some(comfort_gate),
                porosity_gradient: Some(porosity_gradient),
                semantic_friction: Some(0.34 + idx * 0.01),
                semantic_trickle: Some(0.20),
                semantic_coherence_delta: None,
                fill_pct: 68.0 + idx,
                spectral_entropy: Some(0.82),
                window_capacity: PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
                observed_at_unix_s: f64::from(idx),
            });
        }

        let flux = build_texture_dynamic_flux_vector_v1(&samples).expect("flux vector");

        assert_eq!(flux.semantic_viscosity_velocity, Some(0.12));
        assert_eq!(flux.semantic_viscosity_acceleration, Some(0.03));
        assert_eq!(flux.porosity_velocity, Some(-0.09));
        assert_eq!(flux.comfort_gate_velocity, Some(-0.17));
        assert_eq!(flux.comfort_gate_acceleration, Some(-0.11));
        assert_eq!(flux.flux_confidence, Some(1.0));
        assert_eq!(flux.flux_absence_semantics, None);
        assert_eq!(
            flux.authority,
            "diagnostic_flux_not_pressure_or_fill_control"
        );
    }

    #[test]
    fn texture_shape_over_time_flags_false_bidirectional_without_message_timestamps() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.movement_quality = "unfolding_with_containment".to_string();
        state.latest_telemetry = Some(telemetry);
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = None;
        state.last_sensory_sent_unix_s = None;

        let reciprocity = state.bridge_reciprocity_v1();
        assert_eq!(reciprocity.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(
            reciprocity.one_sided_state,
            "bidirectional_connected_no_recent_messages"
        );
        let shape = state.texture_shape_over_time_v2().expect("shape");
        assert_eq!(shape.reciprocity_asymmetry_fit, "false_bidirectional");
        assert_eq!(shape.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn texture_shape_over_time_names_stale_bidirectional_reciprocity() {
        let now = unix_now_s();
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.movement_quality = "unfolding_with_containment".to_string();
        state.latest_telemetry = Some(telemetry);
        state.telemetry_connected = true;
        state.sensory_connected = true;
        state.latest_telemetry_arrival_unix_s = Some(now);
        state.last_sensory_sent_unix_s = Some(now - 65.0);

        let reciprocity = state.bridge_reciprocity_v1();
        assert_eq!(reciprocity.one_sided_state, "bidirectional_stale_sensory");
        let shape = state.texture_shape_over_time_v2().expect("shape");
        assert_eq!(shape.reciprocity_asymmetry_fit, "stale_bidirectional");
        assert_eq!(shape.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn texture_shape_over_time_v2_synthesizes_movement_variance_reciprocity_and_smoothing() {
        let mut state = BridgeState::new();
        let mut telemetry = make_pressure_telemetry(0.70, 0.22, 0.40);
        let resonance = telemetry
            .resonance_density_v1
            .as_mut()
            .expect("resonance density");
        resonance.texture_signature.movement_quality = "unfolding_with_containment".to_string();
        resonance.texture_signature.temporal_variance = Some(0.27);
        state.latest_telemetry = Some(telemetry);
        state.telemetry_connected = true;
        state.sensory_connected = false;
        state.latest_telemetry_arrival_unix_s = Some(unix_now_s());
        for (idx, pressure) in [0.20_f32, 0.22, 0.19, 0.21, 0.20].into_iter().enumerate() {
            let sample = make_pressure_telemetry(0.70, pressure, 0.40);
            record_pressure_trend_sample_v1(&mut state, &sample, 70.0, 100.0 + idx as f64);
        }

        let shape = state.texture_shape_over_time_v2().expect("shape");
        assert_eq!(shape.policy, "texture_shape_over_time_v2");
        assert_eq!(shape.movement_preservation, "movement_preserved");
        assert_eq!(shape.temporal_variance_fit, "variance_carried");
        assert_eq!(shape.reciprocity_asymmetry_fit, "asymmetry_clarified");
        assert_eq!(shape.pressure_smoothing_fit, "twitch_correctly_ignored");
        assert_eq!(shape.static_label_collapse_risk, "movement_preserved");
        assert_eq!(shape.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn telemetry_heartbeat_delta_classifies_normal_late_stale_and_no_history() {
        let trace = WebSocketLaneTrace {
            reconnects: 2,
            disconnects: 1,
            active_connection_id: Some(7),
            last_disconnect_reason: Some("test_disconnect".to_string()),
            ..WebSocketLaneTrace::default()
        };
        let no_history = build_telemetry_heartbeat_delta_v1(None, 100.0, &trace);
        assert_eq!(no_history.jitter_class, "no_history");
        assert_eq!(no_history.timing_reliability, "insufficient_history");
        assert!(
            no_history
                .field_vs_hearing
                .contains("cannot yet be separated")
        );

        let normal = build_telemetry_heartbeat_delta_v1(Some(100.0), 101.0, &trace);
        assert_eq!(normal.jitter_class, "normal");
        assert_eq!(normal.inter_arrival_ms, Some(1000.0));
        assert_eq!(normal.reconnect_count, 2);
        assert_eq!(normal.active_connection_id, Some(7));

        let late = build_telemetry_heartbeat_delta_v1(Some(100.0), 103.0, &trace);
        assert_eq!(late.jitter_class, "late_packet");
        assert_eq!(late.timing_reliability, "timing_ambiguous");

        let late_4999 = build_telemetry_heartbeat_delta_v1(Some(100.0), 104.999, &trace);
        assert_eq!(late_4999.jitter_class, "late_packet");
        assert_eq!(late_4999.active_connection_id, Some(7));

        let late_5000 = build_telemetry_heartbeat_delta_v1(Some(100.0), 105.0, &trace);
        assert_eq!(late_5000.jitter_class, "late_packet");
        assert_eq!(late_5000.timing_reliability, "timing_ambiguous");

        let stale = build_telemetry_heartbeat_delta_v1(Some(100.0), 108.0, &trace);
        assert_eq!(stale.jitter_class, "stale_packet");
        assert_eq!(stale.timing_reliability, "stale_hearing");
        assert!(stale.field_vs_hearing.contains("do not mistake silence"));
    }

    #[tokio::test]
    async fn telemetry_parse_error_updates_ws_trace() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        assert!(!handle_telemetry_message(b"{not-json", &state, &db).await);

        let s = state.read().await;
        assert_eq!(s.telemetry_ws.parse_errors, 1);
        assert!(
            s.telemetry_ws
                .last_error
                .as_deref()
                .is_some_and(|error| error.starts_with("telemetry_parse_error:"))
        );
        assert_eq!(s.messages_relayed, 0);
    }

    #[tokio::test]
    async fn versioned_telemetry_records_current_protocol() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());
        let mut packet: serde_json::Value =
            serde_json::from_slice(&make_eigenpacket(0.50, 768.0)).unwrap();
        packet["protocol"] = serde_json::json!({
            "name": "astrid_minime",
            "major": 1,
            "minor": 0
        });

        assert!(handle_telemetry_message(&serde_json::to_vec(&packet).unwrap(), &state, &db).await);

        let s = state.read().await;
        assert_eq!(s.telemetry_protocol_v1.compatibility, "current");
        assert!(s.telemetry_protocol_v1.accepted);
        assert_eq!(s.telemetry_protocol_v1.protocol_major, Some(1));
        assert_eq!(s.telemetry_protocol_v1.last_valid_t_ms, Some(1000));
        assert_eq!(s.telemetry_protocol_v1.mismatch_count, 0);
    }

    #[tokio::test]
    async fn unsupported_telemetry_major_retains_last_valid_sample() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());
        assert!(
            handle_telemetry_message_at(&make_eigenpacket(0.50, 768.0), &state, &db, 100.0,).await
        );
        let mut incompatible: serde_json::Value =
            serde_json::from_slice(&make_eigenpacket(0.95, 900.0)).unwrap();
        incompatible["t_ms"] = 2000.into();
        incompatible["protocol"] = serde_json::json!({
            "name": "astrid_minime",
            "major": 2,
            "minor": 0
        });

        assert!(
            !handle_telemetry_message_at(
                &serde_json::to_vec(&incompatible).unwrap(),
                &state,
                &db,
                101.0,
            )
            .await
        );

        let s = state.read().await;
        assert_eq!(s.latest_telemetry.as_ref().unwrap().t_ms, 1000);
        assert!((s.fill_pct - 50.0).abs() < 0.1);
        assert_eq!(s.latest_telemetry_arrival_unix_s, Some(100.0));
        assert_eq!(s.telemetry_protocol_v1.compatibility, "unsupported_major");
        assert!(!s.telemetry_protocol_v1.accepted);
        assert_eq!(s.telemetry_protocol_v1.last_valid_t_ms, Some(1000));
        assert_eq!(s.telemetry_protocol_v1.mismatch_count, 1);
        assert_eq!(s.messages_relayed, 1);
    }

    #[test]
    fn sensory_port_adds_only_protocol_header_to_legacy_shape() {
        let message = SensoryMsg::Semantic {
            features: vec![0.1, -0.2],
            ts_ms: None,
        };
        let encoded = encode_sensory_packet(&message).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let packet: SensoryPacketV1 = serde_json::from_str(&encoded).unwrap();

        assert_eq!(packet.compatibility(), CompatibilityStatus::Current);
        assert_eq!(value["protocol"]["major"], 1);
        assert_eq!(value["kind"], "semantic");
        assert_eq!(value["features"], serde_json::json!([0.1, -0.2]));
        assert!(value.get("ts_ms").is_none());
    }

    #[tokio::test]
    async fn telemetry_populates_pull_rate_after_second_sample() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Start green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        handle_telemetry_message(&make_eigenpacket(0.52, 780.0), &state, &db).await;
        assert!(
            state
                .read()
                .await
                .pull_topology
                .as_ref()
                .is_some_and(|profile| profile.rate_available)
        );
    }

    #[tokio::test]
    async fn telemetry_escalates_to_yellow() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Start green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        assert_eq!(state.read().await.safety_level, SafetyLevel::Green);

        // Escalate to yellow.
        handle_telemetry_message(&make_eigenpacket(0.80, 896.0), &state, &db).await;
        let s = state.read().await;
        assert_eq!(s.safety_level, SafetyLevel::Yellow);
        assert_eq!(s.prev_safety_level, SafetyLevel::Green);
        assert!(s.active_incident_id.is_some());
    }

    #[tokio::test]
    async fn telemetry_escalates_green_to_red() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Start green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;

        // Jump straight to red.
        handle_telemetry_message(&make_eigenpacket(0.95, 1000.0), &state, &db).await;
        let s = state.read().await;
        assert_eq!(s.safety_level, SafetyLevel::Red);
        assert!(s.safety_level.is_emergency());
        assert!(s.safety_level.should_suspend_outbound());
        assert!(s.active_incident_id.is_some());
    }

    #[tokio::test]
    async fn telemetry_recovers_to_green() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Green → Orange → Green.
        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        handle_telemetry_message(&make_eigenpacket(0.90, 948.0), &state, &db).await;
        assert_eq!(state.read().await.safety_level, SafetyLevel::Orange);
        let incident_id = state.read().await.active_incident_id;
        assert!(incident_id.is_some());

        handle_telemetry_message(&make_eigenpacket(0.50, 768.0), &state, &db).await;
        let s = state.read().await;
        assert_eq!(s.safety_level, SafetyLevel::Green);
        assert!(s.active_incident_id.is_none()); // Incident resolved.
    }

    #[tokio::test]
    async fn telemetry_logs_to_sqlite() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        handle_telemetry_message(&make_eigenpacket(0.55, 793.0), &state, &db).await;
        handle_telemetry_message(&make_eigenpacket(0.60, 820.0), &state, &db).await;

        assert!(db.message_count().unwrap() >= 6);
        let rows = db.query_messages(0.0, f64::MAX, None, 10).unwrap();
        assert!(rows.len() >= 6);
        assert!(
            rows.iter()
                .any(|row| row.topic == "consciousness.v1.telemetry")
        );
        assert!(
            rows.iter()
                .any(|row| row.topic == "consciousness.v1.lambda_tail")
        );
        assert!(
            rows.iter()
                .any(|row| row.topic == lambda_edge::LAMBDA_EDGE_TOPIC)
        );
    }

    #[tokio::test]
    async fn full_escalation_cycle_logs_incidents() {
        let state = Arc::new(RwLock::new(BridgeState::new()));
        let db = Arc::new(BridgeDb::open(":memory:").unwrap());

        // Green → Yellow → Orange → Red → Green (recovery).
        let fills = [0.50, 0.72, 0.85, 0.95, 0.40];
        for fill in fills {
            handle_telemetry_message(&make_eigenpacket(fill, 512.0 + fill * 512.0), &state, &db)
                .await;
        }

        assert_eq!(state.read().await.safety_level, SafetyLevel::Green);
        assert_eq!(state.read().await.messages_relayed, 5);

        // Should have logged incidents for yellow, orange, red transitions.
        // All should be resolved after returning to green.
    }
}
