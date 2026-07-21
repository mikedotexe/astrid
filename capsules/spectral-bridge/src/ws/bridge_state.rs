use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use astrid_minime_protocol::{CompatibilityStatus, EigenPacketV1};
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
    CadenceContentDistinctionV1, ConnectivityStatus, DeltaPersistenceV1, ExperienceDeltaBusV1,
    ExperienceDeltaKindV1, ExperienceDeltaV1, LambdaContribution, LambdaProfile,
    MessageDirection, PersistentDeformationSmoothingReviewV1, PressureSourceAnalysisV1,
    PressureTrendSmoothingV1, PressureTrendV1, PullModeRate, PullTopologyProfile,
    ResidualDeformationTraceV1, ResonanceDensityComponents, SafetyDecisionTrace, SafetyLevel,
    SensoryDeliveryProtocolStatusV1, SensoryMsg, SpectralTelemetry, TelemetryHeartbeatDeltaV1,
    TelemetryIntegrationHealthV1, TelemetryProtocolStatusV1, TextureDynamicFluxVectorV1,
    TextureShapeOverTimeV2, TextureSignatureIntegrityV1, ViscosityPorosityTransportReviewV1,
    WebSocketLaneTrace,
    resonance_cohesion_score_v1, resonance_stability_context_v1,
    resonance_structural_integrity_index_v1,
    viscosity_porosity_transport_review_with_fingerprint_v1,
};
use crate::witness::{
    AstridInterpretationV1, BridgeEvidenceV1, MinimeObservationV1, WitnessFrameV1,
    decode_telemetry_v1,
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
const TELEMETRY_INTEGRATION_EWMA_ALPHA: f64 = 0.20;
const TELEMETRY_INTEGRATION_EWMA_REMAINDER: f64 = 0.80;
const TELEMETRY_PREWRITE_PIPELINE_WATCH_MS: f64 = 100.0;
const TELEMETRY_WRITE_LOCK_WAIT_WATCH_MS: f64 = 5.0;
const TELEMETRY_WRITE_LOCK_HOLD_WATCH_MS: f64 = 20.0;
const SEMANTIC_RESIDUE_WATCH_THRESHOLD: f32 = 0.60;

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

fn telemetry_duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64().mul_add(1_000.0, 0.0)
}

fn telemetry_integration_ewma(previous: f64, latest: f64) -> f64 {
    latest.mul_add(
        TELEMETRY_INTEGRATION_EWMA_ALPHA,
        previous.mul_add(TELEMETRY_INTEGRATION_EWMA_REMAINDER, 0.0),
    )
}

fn build_telemetry_integration_health_v1(
    previous: Option<&TelemetryIntegrationHealthV1>,
    prewrite_pipeline_ms: f64,
    write_lock_wait_ms: f64,
    write_lock_hold_ms: f64,
) -> TelemetryIntegrationHealthV1 {
    let previous_samples = previous.map_or(0, |health| health.sample_count);
    let sample_count = previous_samples.saturating_add(1);
    let (
        ewma_prewrite_pipeline_ms,
        max_prewrite_pipeline_ms,
        ewma_write_lock_wait_ms,
        max_write_lock_wait_ms,
        ewma_write_lock_hold_ms,
        max_write_lock_hold_ms,
    ) = previous.map_or(
        (
            prewrite_pipeline_ms,
            prewrite_pipeline_ms,
            write_lock_wait_ms,
            write_lock_wait_ms,
            write_lock_hold_ms,
            write_lock_hold_ms,
        ),
        |health| {
            (
                telemetry_integration_ewma(
                    health.ewma_prewrite_pipeline_ms,
                    prewrite_pipeline_ms,
                ),
                health.max_prewrite_pipeline_ms.max(prewrite_pipeline_ms),
                telemetry_integration_ewma(
                    health.ewma_write_lock_wait_ms,
                    write_lock_wait_ms,
                ),
                health.max_write_lock_wait_ms.max(write_lock_wait_ms),
                telemetry_integration_ewma(
                    health.ewma_write_lock_hold_ms,
                    write_lock_hold_ms,
                ),
                health.max_write_lock_hold_ms.max(write_lock_hold_ms),
            )
        },
    );
    let classification = if write_lock_wait_ms >= TELEMETRY_WRITE_LOCK_WAIT_WATCH_MS {
        "write_lock_wait_observed"
    } else if write_lock_hold_ms >= TELEMETRY_WRITE_LOCK_HOLD_WATCH_MS {
        "write_lock_hold_observed"
    } else if prewrite_pipeline_ms >= TELEMETRY_PREWRITE_PIPELINE_WATCH_MS {
        "prewrite_pipeline_heavy"
    } else {
        "clear_at_latest_sample"
    };

    TelemetryIntegrationHealthV1 {
        policy: "telemetry_integration_health_v1".to_string(),
        schema_version: 1,
        sample_count,
        classification: classification.to_string(),
        latest_prewrite_pipeline_ms: prewrite_pipeline_ms,
        ewma_prewrite_pipeline_ms,
        max_prewrite_pipeline_ms,
        latest_write_lock_wait_ms: write_lock_wait_ms,
        ewma_write_lock_wait_ms,
        max_write_lock_wait_ms,
        latest_write_lock_hold_ms: write_lock_hold_ms,
        ewma_write_lock_hold_ms,
        max_write_lock_hold_ms,
        causal_attribution: "not_established_by_timing_alone".to_string(),
        buffered_integration: false,
        cadence_write: false,
        authority: "diagnostic_timing_evidence_not_control".to_string(),
    }
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
    /// Immutable producer truth decoded once at the Minime telemetry port.
    latest_minime_observation_v1: Option<MinimeObservationV1>,
    /// Bridge-owned temporal and derivative evidence with an exact observation parent.
    latest_bridge_evidence_v1: Option<BridgeEvidenceV1>,
    /// Astrid-owned interpretation kept distinct from both observation and derivation.
    latest_astrid_interpretation_v1: Option<AstridInterpretationV1>,
    /// Validated content-free references joining the three ownership domains.
    latest_witness_frame_v1: Option<WitnessFrameV1>,
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
    /// Measured telemetry pipeline and shared-state lock timing.
    pub telemetry_integration_health_v1: Option<TelemetryIntegrationHealthV1>,
    /// Negotiated technical delivery and same-connection receipt status.
    pub sensory_delivery_protocol_v1: SensoryDeliveryProtocolStatusV1,
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
            latest_minime_observation_v1: None,
            latest_bridge_evidence_v1: None,
            latest_astrid_interpretation_v1: None,
            latest_witness_frame_v1: None,
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
            telemetry_integration_health_v1: None,
            sensory_delivery_protocol_v1: SensoryDeliveryProtocolStatusV1::default(),
        }
    }

    /// Derived bidirectional connectivity health across the telemetry and
    /// sensory lanes (collapses the two independent booleans into one
    /// perceivable state).
    #[must_use]
    pub const fn connectivity_status(&self) -> ConnectivityStatus {
        ConnectivityStatus::from_lanes(self.telemetry_connected, self.sensory_connected)
    }

    /// Latest immutable Minime observation, separate from the legacy projection.
    #[must_use]
    pub const fn minime_observation_v1(&self) -> Option<&MinimeObservationV1> {
        self.latest_minime_observation_v1.as_ref()
    }

    /// Latest bridge-owned derivation.
    #[must_use]
    pub const fn bridge_evidence_v1(&self) -> Option<&BridgeEvidenceV1> {
        self.latest_bridge_evidence_v1.as_ref()
    }

    /// Latest Astrid-owned interpretation.
    #[must_use]
    pub const fn astrid_interpretation_v1(&self) -> Option<&AstridInterpretationV1> {
        self.latest_astrid_interpretation_v1.as_ref()
    }

    /// Latest validated self/other witness frame.
    #[must_use]
    pub const fn witness_frame_v1(&self) -> Option<&WitnessFrameV1> {
        self.latest_witness_frame_v1.as_ref()
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

    /// Distinguish a clear telemetry pipe from persistent semantic residue.
    #[must_use]
    pub fn cadence_content_distinction_v1(&self) -> Option<CadenceContentDistinctionV1> {
        let heartbeat = self.telemetry_heartbeat_delta_v1.as_ref();
        let smoothing = self.pressure_trend_smoothing_v1();
        if heartbeat.is_none() && smoothing.is_none() {
            return None;
        }

        let cadence_state = heartbeat.map_or("cadence_evidence_unavailable", |value| {
            match (
                value.jitter_class.as_str(),
                value.timing_reliability.as_str(),
            ) {
                ("normal", "reliable") => "cadence_clear",
                ("stale_packet", _) | (_, "stale_hearing") => "cadence_stale",
                ("late_packet", _) | (_, "timing_ambiguous") => "cadence_ambiguous",
                _ => "cadence_insufficient_history",
            }
        });
        let latest_semantic_viscosity = smoothing
            .as_ref()
            .and_then(|value| value.latest_semantic_viscosity)
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0));
        let semantic_viscosity_persistence_index = smoothing
            .as_ref()
            .and_then(|value| value.semantic_viscosity_persistence_index)
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0));
        let semantic_stagnation_index = smoothing
            .as_ref()
            .and_then(|value| value.semantic_stagnation_index)
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0));
        let semantic_residue_score = match (
            semantic_viscosity_persistence_index,
            semantic_stagnation_index,
        ) {
            (Some(persistence), Some(stagnation)) => Some(persistence.max(stagnation)),
            (Some(persistence), None) => Some(persistence),
            (None, Some(stagnation)) => Some(stagnation),
            (None, None) => None,
        };
        let semantic_persistence_state = smoothing
            .as_ref()
            .map(|value| value.semantic_viscosity_persistence_state.as_str())
            .unwrap_or_default();
        let semantic_stagnation_state = smoothing
            .as_ref()
            .map(|value| value.semantic_stagnation_state.as_str())
            .unwrap_or_default();
        let persistent_residue_named = matches!(
            semantic_persistence_state,
            "persistent_thickness_against_motion" | "persistent_semantic_viscosity"
        ) || matches!(
            semantic_stagnation_state,
            "functional_clog_connected_lanes_watch" | "semantic_stagnation_watch"
        );
        let content_state = if persistent_residue_named
            || semantic_residue_score
                .is_some_and(|score| score >= SEMANTIC_RESIDUE_WATCH_THRESHOLD)
        {
            "persistent_semantic_residue"
        } else if semantic_residue_score.is_some() {
            "semantic_residue_below_watch_threshold"
        } else if latest_semantic_viscosity.is_some() {
            "semantic_viscosity_observed_insufficient_persistence_history"
        } else {
            "content_evidence_unavailable"
        };
        let cadence_content_relation = match (cadence_state, content_state) {
            ("cadence_clear", "persistent_semantic_residue") => {
                "cadence_clear_semantic_residue_persists"
            },
            ("cadence_clear", "semantic_residue_below_watch_threshold") => {
                "cadence_clear_semantic_residue_below_watch"
            },
            ("cadence_stale", "persistent_semantic_residue") => {
                "cadence_stale_and_semantic_residue_persists"
            },
            ("cadence_ambiguous", "persistent_semantic_residue") => {
                "cadence_ambiguous_semantic_residue_persists"
            },
            (_, "content_evidence_unavailable") => "cadence_known_content_evidence_unavailable",
            (_, "semantic_viscosity_observed_insufficient_persistence_history") => {
                "cadence_known_semantic_viscosity_history_insufficient"
            },
            _ => "cadence_content_relation_mixed",
        };

        Some(CadenceContentDistinctionV1 {
            policy: "cadence_content_distinction_v1".to_string(),
            schema_version: 1,
            cadence_state: cadence_state.to_string(),
            cadence_jitter_class: heartbeat.map(|value| value.jitter_class.clone()),
            cadence_clarity_score: heartbeat.and_then(|value| value.cadence_clarity_score),
            cadence_evidence_basis: heartbeat.map_or_else(
                || "no_telemetry_heartbeat_delta_available".to_string(),
                |value| value.cadence_clarity_basis.clone(),
            ),
            content_state: content_state.to_string(),
            latest_semantic_viscosity,
            semantic_viscosity_persistence_index,
            semantic_stagnation_index,
            semantic_residue_score,
            semantic_residue_score_basis:
                "max_of_available_semantic_viscosity_persistence_and_stagnation_indices"
                    .to_string(),
            semantic_residue_watch_threshold: SEMANTIC_RESIDUE_WATCH_THRESHOLD,
            evidence_window_samples: smoothing.as_ref().map_or(0, |value| value.sample_count),
            cadence_content_relation: cadence_content_relation.to_string(),
            live_cadence_write: false,
            live_semantic_write: false,
            authority: "read_only_transport_content_distinction_not_cadence_semantic_or_control"
                .to_string(),
        })
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
