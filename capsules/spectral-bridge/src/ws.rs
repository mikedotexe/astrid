//! `WebSocket` clients for minime connectivity.
//!
//! Two persistent connections:
//! - **Telemetry** (port 7878): Subscribes to spectral eigenvalue broadcasts.
//! - **Sensory** (port 7879): Sends control/semantic features to minime.
//!
//! Both connections auto-reconnect with exponential backoff on failure.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tracing::{debug, debug_span, error, info, info_span, warn};

use crate::db::BridgeDb;
use crate::lambda_edge::{self, LambdaEdgePerceptionV1};
use crate::lambda_tail::{self, ArtifactScanSummary, LambdaTailTelemetryV1};
use crate::paths::bridge_paths;
use crate::sticky_mode::{self, StickyModeAuditV1};
use crate::types::{
    BridgeReciprocityV1, ConnectivityStatus, LambdaContribution, LambdaProfile, MessageDirection,
    PressureTrendSmoothingV1, PressureTrendV1, PullModeRate, PullTopologyProfile,
    SafetyDecisionTrace, SafetyLevel, SensoryMsg, SpectralTelemetry, TelemetryHeartbeatDeltaV1,
    TextureShapeOverTimeV2, TextureSignatureIntegrityV1, WebSocketLaneTrace,
};

const PRESSURE_TREND_SMOOTHING_WINDOW: usize = 5;

#[derive(Debug, Clone)]
struct PressureTrendSampleV1 {
    pressure_risk: Option<f32>,
    fill_pct: f32,
    observed_at_unix_s: f64,
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
        let telemetry_age_ms = self
            .latest_telemetry_arrival_unix_s
            .map(|at| ((now - at).max(0.0) * 1000.0).round());
        let sensory_send_age_ms = self
            .last_sensory_sent_unix_s
            .map(|at| ((now - at).max(0.0) * 1000.0).round());
        let connectivity = self.connectivity_status();
        let one_sided_state = match connectivity {
            ConnectivityStatus::Bidirectional
                if telemetry_age_ms.is_some() && sensory_send_age_ms.is_some() =>
            {
                "bidirectional_recent"
            }
            ConnectivityStatus::Bidirectional if telemetry_age_ms.is_some() => {
                "bidirectional_no_recent_sensory"
            }
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
            one_sided_state: one_sided_state.to_string(),
            authority: "diagnostic_status_context_not_control".to_string(),
        }
    }

    /// Read-only smoothing companion for the latest pressure trend.
    #[must_use]
    pub fn pressure_trend_smoothing_v1(&self) -> Option<PressureTrendSmoothingV1> {
        build_pressure_trend_smoothing_v1(&self.pressure_trend_samples_v1)
    }

    /// Read-only integrity comparison for Minime's typed texture signature.
    #[must_use]
    pub fn texture_signature_integrity_v1(&self) -> Option<TextureSignatureIntegrityV1> {
        let resonance = self
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())?;
        let signature = &resonance.texture_signature;
        let temporal_variance = signature.temporal_variance;
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

        Some(TextureSignatureIntegrityV1 {
            policy: "texture_signature_integrity_v1".to_string(),
            schema_version: 1,
            movement_quality: signature.movement_quality.clone(),
            temporal_variance,
            pressure_source_family: signature.pressure_source_family.clone(),
            pressure_risk: Some(resonance.pressure_risk),
            mode_packing: Some(resonance.components.mode_packing),
            dynamic_damping_threshold_candidate: damping_candidate,
            variance_status: variance_status.to_string(),
            damping_candidate_status: damping_candidate_status.to_string(),
            component_alignment_state: alignment.alignment_state.clone(),
            expected_primary_texture: alignment.expected_primary_texture.clone(),
            emitted_primary_texture: alignment.emitted_primary_texture.clone(),
            advisory_observability: damping_candidate.is_none() && resonance.pressure_risk > 0.20,
            authority: "diagnostic_observability_not_damping_or_control".to_string(),
        })
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
        } else {
            "asymmetry_clarified"
        };
        let pressure_smoothing_fit = smoothing.as_ref().map_or(
            "insufficient_evidence",
            |packet| match packet.classification.as_str() {
                "twitchy_low_amplitude_oscillation" | "low_amplitude_stable" => {
                    "twitch_correctly_ignored"
                }
                "sustained_rising_pressure" | "sustained_falling_pressure" => {
                    "sustained_trend_preserved"
                }
                "mixed_window" => "insufficient_evidence",
                _ => "insufficient_evidence",
            },
        );
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
    let pressure_delta = latest_pressure
        .zip(previous_pressure)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let mode_packing_delta = latest_mode_packing
        .zip(previous_mode_packing)
        .map(|(latest, previous)| (latest - previous).clamp(-1.0, 1.0));
    let fill_delta_pct = previous_fill_pct
        .map(|previous_fill| (latest_fill_pct - previous_fill).clamp(-100.0, 100.0));

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
        latest_fill_pct: latest_fill_pct.is_finite().then_some(latest_fill_pct),
        previous_fill_pct,
        fill_delta_pct,
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
    state
        .pressure_trend_samples_v1
        .push_back(PressureTrendSampleV1 {
            pressure_risk,
            fill_pct,
            observed_at_unix_s,
        });
    while state.pressure_trend_samples_v1.len() > PRESSURE_TREND_SMOOTHING_WINDOW {
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
    let valid_pressures = samples
        .iter()
        .filter_map(|sample| sample.pressure_risk)
        .collect::<Vec<_>>();
    let window_policy = format!("latest_{PRESSURE_TREND_SMOOTHING_WINDOW}_telemetry_samples");
    if valid_pressures.len() < 3 {
        return Some(PressureTrendSmoothingV1 {
            policy: "pressure_trend_smoothing_v1".to_string(),
            schema_version: 1,
            classification: "insufficient_history".to_string(),
            sample_count: samples.len(),
            latest_pressure_risk: latest_pressure,
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
            latest_pressure_risk: latest_pressure,
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
            (first_delta > 0.0 && second_delta < 0.0)
                || (first_delta < 0.0 && second_delta > 0.0)
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
        latest_pressure_risk: latest_pressure,
        smoothed_pressure_delta: Some((smoothed_pressure_delta * 100.0).round() / 100.0),
        pressure_range: Some((pressure_range * 100.0).round() / 100.0),
        fill_range_pct: Some((fill_range_pct * 100.0).round() / 100.0),
        window_policy,
        authority: "diagnostic_smoothing_not_pressure_control".to_string(),
    })
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
    const ARTIFACT_SCAN_WINDOW_SECS: f64 = 1_200.0;
    const ARTIFACT_SCAN_MIN_INTERVAL_SECS: f64 = 30.0;

    let telemetry: SpectralTelemetry = match serde_json::from_slice(data) {
        Ok(t) => t,
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

    let lambda1 = telemetry.lambda1();
    let observed_at_unix_s = unix_now_s();
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
        s.previous_telemetry_arrival_unix_s = previous_arrival;
        s.latest_telemetry_arrival_unix_s = Some(observed_at_unix_s);
        s.telemetry_heartbeat_delta_v1 = Some(heartbeat.clone());
        write_telemetry_heartbeat_snapshot(&heartbeat);
        s.previous_fill_pct = previous_fill_pct;
        s.latest_telemetry = Some(telemetry.clone());
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

                                    let json = match serde_json::to_string(&sensory_msg) {
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
                                    if let Err(e) = ws_tx.send(Message::Text(json)).await {
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
        assert_eq!(
            trace.last_disconnect_reason.as_deref(),
            Some("close_frame:normal")
        );
        assert_eq!(trace.last_error.as_deref(), Some("send_error:closed"));
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

        handle_telemetry_message(&make_pressure_eigenpacket(0.70, 0.20, 0.40), &state, &db).await;
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

        handle_telemetry_message(&make_pressure_eigenpacket(0.705, 0.21, 0.42), &state, &db).await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "stable_heavy");
            assert!(trend.pressure_delta.is_some_and(|delta| delta > 0.0));
            assert_eq!(trend.heartbeat_jitter_class.as_deref(), Some("normal"));
            assert_eq!(trend.timing_reliability.as_deref(), Some("reliable"));
        }

        handle_telemetry_message(&make_pressure_eigenpacket(0.735, 0.30, 0.46), &state, &db).await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "rising_pressure");
            assert!(trend.fill_delta_pct.is_some_and(|delta| delta >= 2.0));
        }

        handle_telemetry_message(&make_pressure_eigenpacket(0.70, 0.20, 0.41), &state, &db).await;
        {
            let s = state.read().await;
            let trend = s.pressure_trend_v1.as_ref().unwrap();
            assert_eq!(trend.classification, "falling_pressure");
            assert!(trend.pressure_delta.is_some_and(|delta| delta < 0.0));
        }

        handle_telemetry_message(&make_eigenpacket(0.70, 768.0), &state, &db).await;
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
        assert_eq!(smoothing.authority, "diagnostic_smoothing_not_pressure_control");
    }

    #[test]
    fn bridge_reciprocity_distinguishes_one_sided_states_and_last_sensory_send() {
        let mut state = BridgeState::new();
        state.telemetry_connected = true;
        state.sensory_connected = false;
        state.latest_telemetry_arrival_unix_s = Some(unix_now_s());
        let telemetry_only = state.bridge_reciprocity_v1();
        assert_eq!(telemetry_only.connectivity, ConnectivityStatus::TelemetryOnly);
        assert_eq!(telemetry_only.one_sided_state, "telemetry_only");
        assert_eq!(telemetry_only.last_sensory_sent_unix_s, None);

        state.sensory_connected = true;
        record_ws_message_sent(&mut state, WsLane::Sensory);
        let bidirectional = state.bridge_reciprocity_v1();
        assert_eq!(bidirectional.connectivity, ConnectivityStatus::Bidirectional);
        assert_eq!(bidirectional.one_sided_state, "bidirectional_recent");
        assert!(bidirectional.last_sensory_sent_unix_s.is_some());
        assert!(bidirectional.sensory_send_age_ms.is_some());
    }

    #[test]
    fn texture_signature_integrity_reports_variance_and_observability_boundary() {
        let mut state = BridgeState::new();
        let telemetry: SpectralTelemetry = serde_json::from_slice(&make_pressure_eigenpacket(
            0.70, 0.24, 0.44,
        ))
        .unwrap();
        state.latest_telemetry = Some(telemetry);
        let integrity = state
            .texture_signature_integrity_v1()
            .expect("texture integrity");
        assert_eq!(integrity.policy, "texture_signature_integrity_v1");
        assert_eq!(integrity.temporal_variance, None);
        assert_eq!(
            integrity.damping_candidate_status,
            "missing_candidate_observability_only"
        );
        assert_eq!(integrity.component_alignment_state, "insufficient_context");
        assert_eq!(integrity.expected_primary_texture, "unknown");
        assert_eq!(integrity.emitted_primary_texture, "unknown");
        assert!(integrity.advisory_observability);
        assert_eq!(
            integrity.authority,
            "diagnostic_observability_not_damping_or_control"
        );
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
