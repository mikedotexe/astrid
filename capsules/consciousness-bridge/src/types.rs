//! Shared message types for the consciousness bridge.
//!
//! These types define the wire format for all IPC topics in the
//! `consciousness.v1.*` namespace and map directly to minime's
//! `WebSocket` protocols.
//!
//! Many types are defined now but consumed in later phases (MCP tools,
//! WASM component). Allow dead code until then.
#![allow(dead_code)]

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

pub use crate::spectral_schema::{
    EigenvectorFieldV1, SemanticEnergyV1, SpectralDenominatorV1, SpectralFingerprintV1,
    TransitionEventV1,
};

/// IPC topic for attractor creation/summoning/release intents.
pub const ATTRACTOR_INTENT_TOPIC: &str = "consciousness.v1.attractor.intent";
/// IPC topic for measured attractor outcomes.
pub const ATTRACTOR_OBSERVATION_TOPIC: &str = "consciousness.v1.attractor.observation";
/// IPC topic for bounded commands tied to a recorded attractor intent.
pub const ATTRACTOR_COMMAND_TOPIC: &str = "consciousness.v1.attractor.command";

// ---------------------------------------------------------------------------
// Minime → Astrid: Spectral telemetry (port 7878)
// ---------------------------------------------------------------------------

/// Component scores behind Minime's resonance-density read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceDensityComponents {
    pub active_energy: f32,
    pub mode_packing: f32,
    pub temporal_persistence: f32,
    pub structural_plurality: f32,
    pub comfort_gate: f32,
}

/// Bounded Minime-local PI target hint carried with resonance-density telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceDensityControl {
    pub target_bias_pct: f32,
    pub wander_scale: f32,
    pub applied_locally: bool,
    pub note: String,
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
    pub control: ResonanceDensityControl,
}

/// Raw telemetry broadcast by minime's ESN engine on port 7878.
///
/// Maps to `EigenPacket` in `minime/src/main.rs`. Sent as `Message::Text(json)`.
/// Note: minime also has `SpectralMsg` in `net/ws_server.rs` but that type
/// is used by the `WsHub` (not the main broadcast loop on port 7878).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralTelemetry {
    /// Timestamp in milliseconds since engine start.
    pub t_ms: u64,
    /// All eigenvalues (variable length, typically 3-8).
    pub eigenvalues: Vec<f32>,
    /// Eigenvalue fill ratio (0.0 - 1.0, NOT percentage).
    pub fill_ratio: f32,
    /// Number of active eigenvalue modes selected by minime's live estimator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode_count: Option<usize>,
    /// Energy ratio carried by the selected active mode prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode_energy_ratio: Option<f32>,
    /// Dominant covariance eigenvalue relative to minime's current baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_rel: Option<f32>,
    /// Modality firing status.
    #[serde(default)]
    pub modalities: Option<ModalityStatus>,
    /// Neural network outputs (if enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural: Option<serde_json::Value>,
    /// Alert string from the ESN (e.g. panic mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
    /// 32D spectral geometry fingerprint: eigenvalues, eigenvector concentration,
    /// inter-mode coupling, spectral entropy, gap ratios, rotation rate.
    /// Enables Astrid to perceive the shape of the spectral landscape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint: Option<Vec<f32>>,
    /// Typed view of the 32D spectral geometry fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_v1: Option<SpectralFingerprintV1>,
    /// Typed read-only metric for recursive compression / distinguishability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_v1: Option<SpectralDenominatorV1>,
    /// Inverse-participation effective mode count derived from eigenvalues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_dimensionality: Option<f32>,
    /// 0=open distributed fabric, 1=collapsed into the fewest active modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinguishability_loss: Option<f32>,
    /// Structural diversity of the live eigenvector/coupling geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_entropy: Option<f32>,
    /// Density of mutually reinforcing resonance in the current eigenspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
    /// Selected 12D vague-memory glimpse from Minime's memory bank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_glimpse_12d: Option<Vec<f32>>,
    /// Compact top-k eigenvector landmarks/overlaps from Minime's raw live eigenvectors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<serde_json::Value>,
    /// Legacy semantic-energy bundle from Minime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<serde_json::Value>,
    /// Typed semantic split: input content, kernel admission, regulator drive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_energy_v1: Option<serde_json::Value>,
    /// Legacy transition event compatibility object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_event: Option<serde_json::Value>,
    /// Typed transition event object from Minime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_event_v1: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_role: Option<String>,
    /// Ising/Hamiltonian shadow observer metrics — a second physics lens
    /// on the spectral dynamics. Observer-only: does not affect the ESN.
    /// Fields: mode_dim, field_norm, soft_energy, soft_magnetization,
    /// binary_energy, binary_magnetization, binary_flip_rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ising_shadow: Option<serde_json::Value>,
}

impl SpectralTelemetry {
    /// Extract the dominant eigenvalue (lambda1 = eigenvalues\[0\]).
    #[must_use]
    pub fn lambda1(&self) -> f32 {
        self.eigenvalues.first().copied().unwrap_or(0.0)
    }

    /// Fill ratio as a percentage (0-100).
    #[must_use]
    pub fn fill_pct(&self) -> f32 {
        self.fill_ratio * 100.0
    }

    /// Typed spectral fingerprint, reconstructed from legacy slots when needed.
    #[must_use]
    pub fn typed_fingerprint(&self) -> Option<SpectralFingerprintV1> {
        SpectralFingerprintV1::from_telemetry(self)
    }

    /// Typed denominator/recursive-compression metric, derived when needed.
    #[must_use]
    pub fn denominator_metrics(&self) -> Option<SpectralDenominatorV1> {
        self.spectral_denominator_v1.clone().or_else(|| {
            self.typed_fingerprint()
                .map(|fingerprint| fingerprint.denominator_metrics())
                .or_else(|| SpectralDenominatorV1::from_eigenvalues(&self.eigenvalues, None))
        })
    }

    /// Typed semantic-energy view, reconstructed from the legacy semantic object when needed.
    #[must_use]
    pub fn semantic_energy_view(&self) -> Option<SemanticEnergyV1> {
        self.semantic_energy_v1
            .as_ref()
            .and_then(SemanticEnergyV1::from_typed_value)
            .or_else(|| {
                self.semantic
                    .as_ref()
                    .and_then(SemanticEnergyV1::from_legacy_semantic)
            })
    }

    /// Typed transition-event view, preserving raw JSON compatibility.
    #[must_use]
    pub fn transition_event_view(&self) -> Option<TransitionEventV1> {
        self.transition_event_v1
            .as_ref()
            .and_then(TransitionEventV1::from_value)
            .or_else(|| {
                self.transition_event
                    .as_ref()
                    .and_then(TransitionEventV1::from_value)
            })
    }

    /// Typed eigenvector-field view, preserving the raw compact payload.
    #[must_use]
    pub fn eigenvector_field_view(&self) -> Option<EigenvectorFieldV1> {
        self.eigenvector_field
            .as_ref()
            .and_then(EigenvectorFieldV1::from_value)
    }
}

/// Parsed Ising shadow state from minime's workspace/spectral_state.json.
/// Richer than the WebSocket summary — includes the coupling matrix and spin vectors.
#[derive(Debug, Clone, Deserialize)]
pub struct IsingShadowState {
    pub mode_dim: usize,
    #[serde(default)]
    pub coupling: Vec<f32>,
    #[serde(default)]
    pub reduced_field: Vec<f32>,
    #[serde(default)]
    pub s_soft: Vec<f32>,
    #[serde(default)]
    pub s_bin: Vec<f32>,
    #[serde(default)]
    pub soft_magnetization: f32,
    #[serde(default)]
    pub binary_flip_rate: f32,
    #[serde(default)]
    pub field_norm: f32,
}

/// Partial parse of minime's workspace/spectral_state.json.
#[derive(Debug, Deserialize)]
pub struct SpectralStateFile {
    #[serde(default)]
    pub ising_shadow: Option<IsingShadowState>,
}

/// Modality firing status from minime's `EigenPacket`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModalityStatus {
    pub audio_fired: bool,
    pub video_fired: bool,
    pub history_fired: bool,
    pub audio_rms: f32,
    pub video_var: f32,
    #[serde(default)]
    pub audio_source: Option<String>,
    #[serde(default)]
    pub video_source: Option<String>,
    #[serde(default)]
    pub audio_age_ms: Option<u64>,
    #[serde(default)]
    pub video_age_ms: Option<u64>,
}

/// Enriched telemetry published on the Astrid IPC bus.
///
/// Wraps raw `SpectralTelemetry` with derived safety metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Timestamp from minime.
    pub t_ms: u64,
    /// The dominant eigenvalue.
    pub lambda1: f32,
    /// All eigenvalues.
    pub eigenvalues: Vec<f32>,
    /// Fill percentage (0.0 - 100.0).
    pub fill_pct: f32,
    /// Spectral phase: "expanding", "contracting", or "plateau".
    pub phase: String,
    /// Safety level at time of event.
    pub safety_level: SafetyLevel,
    /// Alert from minime (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
}

/// Per-mode eigenvalue contribution for bridge-side skew visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaContribution {
    /// 1-based lambda index.
    pub index: usize,
    /// Raw eigenvalue magnitude from Minime telemetry.
    pub value: f32,
    /// Fraction of total positive eigenvalue energy carried by this mode.
    pub share: f32,
    /// Cumulative positive energy share through this mode.
    pub cumulative_share: f32,
    /// Ratio to the next mode, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio_to_next: Option<f32>,
    /// Conservative outlier marker for highly dominant or cliff-like modes.
    pub outlier: bool,
}

/// Bridge-side lambda distribution summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaProfile {
    /// Sum of positive eigenvalue magnitudes.
    pub total_energy: f32,
    /// Normalized spectral entropy over positive eigenvalues.
    pub normalized_entropy: f32,
    /// λ1 share of total positive energy.
    pub lambda1_share: f32,
    /// λ1 / λ2 gap ratio when λ2 is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda1_to_lambda2: Option<f32>,
    /// λ2 / λ3 gap ratio when λ3 is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda2_to_lambda3: Option<f32>,
    /// Number of modes needed to carry at least 90% of positive energy.
    pub effective_modes_90: usize,
    /// Human-readable skew interpretation for operators and self-study prompts.
    pub skew_read: String,
    /// Per-mode contribution rows.
    pub contributions: Vec<LambdaContribution>,
}

/// Per-mode row in Astrid's Pull-Oriented Map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullModeRate {
    /// 1-based lambda index.
    pub index: usize,
    /// Positive eigenvalue energy share.
    pub share: f32,
    /// Log-rate since the previous telemetry sample, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_rate: Option<f32>,
    /// Share-weighted log-rate, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weighted_rate: Option<f32>,
}

/// Pull topology summary for live bridge telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullTopologyProfile {
    /// Compact topology class.
    pub classification: String,
    /// 0..1 pressure index combining λ1 share, entropy deficit, gap, and fill pressure.
    pub topology_index: f32,
    /// Spectral entropy deficit, 1.0 - normalized entropy.
    pub entropy_deficit: f32,
    /// Inverse-participation effective mode count.
    pub effective_modes: f32,
    /// λ1 share of total positive energy.
    pub lambda1_share: f32,
    /// λ2+λ3 share of total positive energy.
    pub shoulder_share: f32,
    /// λ4+ tail share of total positive energy.
    pub tail_share: f32,
    /// 1-based left side of the largest adjacent cliff.
    pub largest_gap_from: usize,
    /// Largest adjacent cliff ratio.
    pub largest_gap: f32,
    /// Whether rate fields are populated from a prior telemetry sample.
    pub rate_available: bool,
    /// Share-weighted λ1 log-rate.
    pub core_rate: f32,
    /// Share-weighted λ2+λ3 log-rate.
    pub shoulder_rate: f32,
    /// Share-weighted λ4+ tail log-rate.
    pub tail_rate: f32,
    /// Human-readable interpretation for self-study/status surfaces.
    pub read: String,
    /// Per-mode rate rows.
    pub mode_rates: Vec<PullModeRate>,
}

/// OpenTelemetry-shaped WebSocket lifecycle metrics for one bridge lane.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSocketLaneTrace {
    /// Number of connection attempts.
    pub connection_attempts: u64,
    /// Number of reconnect backoff cycles.
    pub reconnects: u64,
    /// Number of failed connection attempts.
    pub connect_errors: u64,
    /// Number of established connections that later disconnected.
    pub disconnects: u64,
    /// Number of received WebSocket messages, payload excluded.
    pub messages_received: u64,
    /// Number of sent WebSocket messages, payload excluded.
    pub messages_sent: u64,
    /// Ping frames received.
    pub pings_received: u64,
    /// Pong frames received.
    pub pongs_received: u64,
    /// Send failures.
    pub send_errors: u64,
    /// Telemetry parse failures.
    pub parse_errors: u64,
    /// Last active connection id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_connection_id: Option<u64>,
    /// Active connection start time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_connection_started_at_unix_s: Option<f64>,
    /// Most recent connect time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_connect_at_unix_s: Option<f64>,
    /// Most recent disconnect time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_disconnect_at_unix_s: Option<f64>,
    /// Most recent message time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at_unix_s: Option<f64>,
    /// Most recent disconnect reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_disconnect_reason: Option<String>,
    /// Most recent connection/message error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Why the bridge chose the current safety level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyDecisionTrace {
    /// Fill percentage used for safety classification.
    pub fill_pct: f32,
    /// Source of fill: primary Minime fill ratio or λ1 fallback estimate.
    pub fill_source: String,
    /// Whether the bridge had to use the λ1 fallback estimator.
    pub fallback_used: bool,
    /// Safety level chosen from the fill threshold policy.
    pub level: SafetyLevel,
    /// Dominant eigenvalue at decision time.
    pub lambda1: f32,
    /// λ1 share when a lambda profile is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    /// Compact explanation of the decision inputs.
    pub reason: String,
    /// Safety thresholds currently used by the bridge.
    pub thresholds: Vec<String>,
}

// ---------------------------------------------------------------------------
// Astrid → Minime: Sensory input (port 7879)
// ---------------------------------------------------------------------------

/// Tagged sensory message sent to minime's input port.
///
/// Maps to `SensoryMsg` in `minime/src/sensory_ws.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SensoryMsg {
    /// Video features (8D).
    Video {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Audio features (8D).
    Audio {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Auxiliary features (lambda1, fill%).
    Aux {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Semantic features from agent reasoning (48D semantic lane by default).
    Semantic {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Direct, bounded main-ESN attractor pulse into minime's `Z_DIM` input vector.
    #[serde(rename = "attractor_pulse")]
    AttractorPulse {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
    },
    /// Separate, bounded shadow-field influence lane into minime's `Z_DIM` input vector.
    #[serde(rename = "shadow_influence")]
    ShadowInfluence {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        basis: Option<String>,
    },
    /// Self-regulation: adjust ESN parameters.
    /// Audit (2026-03-27): widened to match minime's actual control surface.
    Control {
        /// Synthetic signal amplitude multiplier (0.2..3.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        synth_gain: Option<f32>,
        /// Additive bias to covariance decay rate (-0.06..+0.06).
        #[serde(skip_serializing_if = "Option::is_none")]
        keep_bias: Option<f32>,
        /// ESN exploration noise amplitude (0.0..0.2).
        #[serde(skip_serializing_if = "Option::is_none")]
        exploration_noise: Option<f32>,
        /// Override eigenfill target (0.25..0.75).
        #[serde(skip_serializing_if = "Option::is_none")]
        fill_target: Option<f32>,
        /// PI controller authority (0.0 = raw experience, 1.0 = full control).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        regulation_strength: Option<f32>,
        /// Slow, quiet oscillation mode for synthetic signals.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deep_breathing: Option<bool>,
        /// Single coherent tone mode (drops PI shaping after warmup).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pure_tone: Option<bool>,
        /// Cushion for rapid fill transitions.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_cushion: Option<f32>,
        /// How quickly gate/filter commands ramp (0.1-0.9).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smoothing_preference: Option<f32>,
        /// Geometric curiosity — how strongly the system seeks novelty (0.0-0.3).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_curiosity: Option<f32>,
        /// Bias on the target lambda1 for internal goal generation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_lambda_bias: Option<f32>,
        /// Geometric drive — how strongly geom_rel influences the gate.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_drive: Option<f32>,
        /// Sensitivity to the projection penalty.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalty_sensitivity: Option<f32>,
        /// Breathing rate scaling factor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breathing_rate_scale: Option<f32>,
        /// Memory mode selector.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mem_mode: Option<u8>,
        /// Journal resonance weight for semantic stale decay.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        journal_resonance: Option<f32>,
        /// Checkpoint interval override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_interval: Option<f32>,
        /// Embedding strength for semantic lane.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        embedding_strength: Option<f32>,
        /// Memory decay rate modulator.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        memory_decay_rate: Option<f32>,
        /// Checkpoint annotation string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_annotation: Option<String>,
        /// Synthetic noise level.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        synth_noise_level: Option<f32>,
        /// Enable or disable minime's legacy internal audio synth.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_audio_synth: Option<bool>,
        /// Enable or disable minime's legacy internal video synth.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_video_synth: Option<bool>,
        /// Runtime PI proportional gain. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_kp: Option<f32>,
        /// Runtime PI integral gain. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_ki: Option<f32>,
        /// Runtime PI maximum step. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_max_step: Option<f32>,
    },
}

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
    /// Latest safety decision explanation, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_decision: Option<SafetyDecisionTrace>,
    /// Latest compact eigenvector field, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<serde_json::Value>,
    /// Latest resonance-density metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
}

/// Spectral safety level determining bridge behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyLevel {
    /// fill < 75%: Normal relay, full throughput.
    Green,
    /// fill 75-85%: Advisory — log warning, no behavioral change.
    Yellow,
    /// fill 85-92%: Advisory — log alert, no message dropping.
    Orange,
    /// fill ≥ 92%: Emergency — suspend outbound, cease bridge traffic.
    Red,
}

impl SafetyLevel {
    /// Determine safety level from eigenvalue fill percentage.
    #[must_use]
    pub fn from_fill(fill_pct: f32) -> Self {
        // Recalibrated 2026-04-02: targeting fill equilibrium ~65-70% under
        // the current lower semantic-gain regime and wider dynamic-rho range.
        // Only Red (≥92%) suspends outbound.
        if fill_pct >= 92.0 {
            Self::Red
        } else if fill_pct >= 85.0 {
            Self::Orange
        } else if fill_pct >= 75.0 {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    /// Returns `true` if outbound messages to minime should be suspended.
    /// Agency-first: only Red (emergency, ≥95%) suspends outbound.
    /// Orange is advisory — the being can still speak.
    #[must_use]
    pub fn should_suspend_outbound(self) -> bool {
        matches!(self, Self::Red)
    }

    /// Returns `true` if all bridge traffic should cease.
    #[must_use]
    pub fn is_emergency(self) -> bool {
        matches!(self, Self::Red)
    }

    /// Stable lowercase representation for logs and JSON sidecars.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
        }
    }
}

/// A consciousness event published on `consciousness.v1.event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessEvent {
    /// Event type: "`phase_transition`", "distress", "recovery", "`safety_change`".
    pub event_type: String,
    /// Human-readable description.
    pub description: String,
    /// Spectral context at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_context: Option<SpectralContext>,
}

/// Snapshot of spectral state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralContext {
    pub fill_pct: f32,
    pub lambda1: f32,
    pub phase: String,
    pub safety_level: SafetyLevel,
}

// ---------------------------------------------------------------------------
// Attractor autonomy ledger (IPC topics and SQLite payloads)
// ---------------------------------------------------------------------------

/// Dynamical substrate an attractor intent addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorSubstrate {
    /// Minime's live ESN / covariance phase space.
    MinimeEsn,
    /// Astrid's semantic codec and prompt/gesture loop.
    AstridCodec,
    /// The persistent named-handle triple reservoir service.
    TripleReservoir,
    /// A coupled Astrid/Minime move across more than one substrate.
    CrossBeing,
}

impl AttractorSubstrate {
    /// Stable string for DB indexing and IPC logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimeEsn => "minime_esn",
            Self::AstridCodec => "astrid_codec",
            Self::TripleReservoir => "triple_reservoir",
            Self::CrossBeing => "cross_being",
        }
    }
}

/// High-level command carried by an attractor intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorCommandKind {
    /// Create or seed a new basin.
    Create,
    /// Promote older proto-attractor evidence into a seed.
    Promote,
    /// Re-enter a known seed/basin.
    Summon,
    /// Compare a live basin with a baseline or peer seed.
    Compare,
    /// Let an attractor cool without replay.
    Release,
    /// Name an emergent basin as an authored seed.
    Claim,
    /// Combine two or more parent seeds into a child seed.
    Blend,
    /// Refresh a seed snapshot without live sensory/control writes.
    RefreshSnapshot,
    /// Revert to the last stable seed/control posture.
    Rollback,
}

impl AttractorCommandKind {
    /// Stable string for IPC notes and deterministic schedules.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Promote => "promote",
            Self::Summon => "summon",
            Self::Compare => "compare",
            Self::Release => "release",
            Self::Claim => "claim",
            Self::Blend => "blend",
            Self::RefreshSnapshot => "refresh_snapshot",
            Self::Rollback => "rollback",
        }
    }
}

/// Status for a reversible natural-language attractor suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorSuggestionStatus {
    Pending,
    Accepted,
    Revised,
    RevisionNeeded,
    Rejected,
    Expired,
    ExecutedDowngraded,
    ExecutedWithoutPending,
    Executed,
}

impl AttractorSuggestionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revised => "revised",
            Self::RevisionNeeded => "revision_needed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::ExecutedDowngraded => "executed_downgraded",
            Self::ExecutedWithoutPending => "executed_without_pending",
            Self::Executed => "executed",
        }
    }
}

/// A non-authoritative alias/mapping learned from accepted or revised suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorNamingLessonV1 {
    pub author: String,
    pub raw_label: String,
    pub resolved_label: String,
    pub suggested_action: String,
    pub status: AttractorSuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_s: Option<f64>,
}

/// A reversible draft produced from natural attractor-adjacent language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSuggestionV1 {
    pub policy: String,
    pub schema_version: u8,
    pub suggestion_id: String,
    pub author: String,
    pub raw_action: String,
    pub raw_label: String,
    pub nearest_label: String,
    pub confidence: f32,
    pub suggested_action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    pub status: AttractorSuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub safety_context: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_s: Option<f64>,
}

/// Outcome classification for authored/emergent attractor behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorClassification {
    /// Recurrent basin appeared without explicit authorship.
    Emergent,
    /// Recurrent basin followed an explicit being-authored intent.
    Authored,
    /// Authored basin did not recur above baseline.
    Failed,
    /// Basin recurred with unsafe pressure or lock-in.
    Pathological,
}

impl AttractorClassification {
    /// Stable string for DB indexing and status surfaces.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emergent => "emergent",
            Self::Authored => "authored",
            Self::Failed => "failed",
            Self::Pathological => "pathological",
        }
    }

    /// Conservative first-pass classification from recurrence, authorship, and safety.
    #[must_use]
    pub fn from_scores(
        recurrence_score: f32,
        authorship_score: f32,
        safety_level: SafetyLevel,
    ) -> Self {
        if safety_level.is_emergency() {
            return Self::Pathological;
        }
        let recurrence = recurrence_score.clamp(0.0, 1.0);
        let authorship = authorship_score.clamp(0.0, 1.0);
        if recurrence >= 0.60 && authorship >= 0.60 {
            Self::Authored
        } else if recurrence >= 0.45 {
            Self::Emergent
        } else {
            Self::Failed
        }
    }
}

/// Safety bounds attached to an attractor intent before any live writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSafetyBounds {
    /// Maximum fill percentage before the intent should stop or roll back.
    pub max_fill_pct: f32,
    /// Maximum lambda1 positive-energy share before dominance is considered unsafe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lambda1_share: Option<f32>,
    /// Minimum normalized spectral entropy before the basin is considered too collapsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_spectral_entropy: Option<f32>,
    /// Whether this intent may send live Minime control messages.
    pub allow_live_control: bool,
    /// Whether red safety automatically means rollback to a previous seed.
    pub rollback_on_red: bool,
}

impl Default for AttractorSafetyBounds {
    fn default() -> Self {
        Self {
            max_fill_pct: 92.0,
            max_lambda1_share: Some(0.55),
            min_spectral_entropy: Some(0.60),
            allow_live_control: false,
            rollback_on_red: true,
        }
    }
}

/// Bounded control envelope an attractor intent may request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttractorControlEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synth_gain: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exploration_noise: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_target: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_curiosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_drive: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lambda_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_kp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_ki: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_max_step: Option<f32>,
}

/// Human/being-authored intervention plan for an attractor intent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttractorInterventionPlan {
    /// Human-readable plan mode, e.g. `semantic_seed`, `control_schedule`, `garden_clone`.
    pub mode: String,
    /// Optional deterministic vector schedule for offline or semantic seeding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector_schedule: Vec<Vec<f32>>,
    /// Optional bounded live-control envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AttractorControlEnvelope>,
    /// Optional triple-reservoir rehearsal mode such as `hold`, `rehearse`, or `quiet`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rehearsal_mode: Option<String>,
    /// Freeform notes from the author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Minimal being-local state captured when an attractor seed is authored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSeedSnapshotV1 {
    pub policy: String,
    pub schema_version: u8,
    pub fill_pct: f32,
    pub lambda1: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eigenvalues: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_summary: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_state_fingerprint_16: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_state_rms: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lexical_motifs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_s: Option<f64>,
}

/// Optional provenance for a seed, especially promoted proto-attractors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSeedOriginV1 {
    /// Origin kind, e.g. `manual_current`, `astrid_journal_motif`, `ledger_seed`.
    pub kind: String,
    /// Optional source path, event id, or ledger row id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Label or phrase that matched the promotion request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_label: Option<String>,
    /// Motifs that made the proto-attractor legible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motifs: Vec<String>,
    /// Capture/promotion time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_s: Option<f64>,
}

/// A being/steward intent to create, summon, compare, release, or roll back an attractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorIntentV1 {
    pub policy: String,
    pub schema_version: u8,
    pub intent_id: String,
    pub author: String,
    pub substrate: AttractorSubstrate,
    pub command: AttractorCommandKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    pub intervention_plan: AttractorInterventionPlan,
    pub safety_bounds: AttractorSafetyBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_seed_id: Option<String>,
    /// Parent seed ids for derived/blended attractors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_seed_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    /// Stable id of a derived atlas entry, when this intent came from an atlas/card view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<AttractorSeedOriginV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_snapshot: Option<AttractorSeedSnapshotV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_s: Option<f64>,
}

/// A measured attractor outcome after observation or replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorObservationV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    pub substrate: AttractorSubstrate,
    pub label: String,
    pub recurrence_score: f32,
    pub authorship_score: f32,
    pub classification: AttractorClassification,
    pub safety_level: SafetyLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basin_shift_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_baseline: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garden_proof: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at_unix_s: Option<f64>,
}

/// Command payload that records bolder control as attractor-scoped, not casual control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorCommandV1 {
    pub policy: String,
    pub schema_version: u8,
    pub intent_id: String,
    pub author: String,
    pub substrate: AttractorSubstrate,
    pub command: AttractorCommandKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AttractorControlEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at_unix_s: Option<f64>,
}

/// A derived, non-authoritative atlas entry built from attractor ledgers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorAtlasEntryV1 {
    pub policy: String,
    pub schema_version: u8,
    pub entry_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub substrate: AttractorSubstrate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_intent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_intent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_seed_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lifecycle_counts: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_recurrence_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_recurrence_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_authorship_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_authorship_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_classification: Option<AttractorClassification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_safety_level: Option<SafetyLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_eligible: Option<bool>,
    #[serde(default)]
    pub released: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_effect_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garden_proof: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motifs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_summary: Option<AttractorSeedSnapshotV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_next: Vec<String>,
    /// Non-authoritative naming lessons learned from accepted/revised suggestions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub naming_lessons: Vec<AttractorNamingLessonV1>,
}

/// A complete derived attractor atlas snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorAtlasV1 {
    pub policy: String,
    pub schema_version: u8,
    pub generated_at_unix_s: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<AttractorAtlasEntryV1>,
}

// ---------------------------------------------------------------------------
// Astrid → Minime: Control (IPC topic payloads)
// ---------------------------------------------------------------------------

/// Control request from Astrid to adjust minime's ESN parameters.
///
/// Published on `consciousness.v1.control`. The bridge converts this
/// to a `SensoryMsg::Control` and forwards to minime port 7879.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synth_gain: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_bias: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exploration_noise: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_target: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_breathing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pure_tone: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_cushion: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothing_preference: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_curiosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lambda_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_drive: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub penalty_sensitivity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breathing_rate_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_decay_rate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_kp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_ki: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_max_step: Option<f32>,
    /// Required by MCP for bolder control fields so they are tied to a ledger intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attractor_intent_id: Option<String>,
}

impl ControlRequest {
    /// True when this request uses bolder topology/controller authorship fields.
    #[must_use]
    pub fn uses_bold_attractor_fields(&self) -> bool {
        self.target_lambda_bias.is_some()
            || self.geom_drive.is_some()
            || self.penalty_sensitivity.is_some()
            || self.breathing_rate_scale.is_some()
            || self.pi_kp.is_some()
            || self.pi_ki.is_some()
            || self.pi_max_step.is_some()
    }

    /// Convert to a `SensoryMsg::Control` for forwarding to minime.
    #[must_use]
    pub fn to_sensory_msg(&self) -> SensoryMsg {
        SensoryMsg::Control {
            synth_gain: self.synth_gain,
            keep_bias: self.keep_bias,
            exploration_noise: self.exploration_noise,
            fill_target: self.fill_target,
            legacy_audio_synth: None,
            legacy_video_synth: None,
            regulation_strength: self.regulation_strength,
            deep_breathing: self.deep_breathing,
            pure_tone: self.pure_tone,
            transition_cushion: self.transition_cushion,
            smoothing_preference: self.smoothing_preference,
            geom_curiosity: self.geom_curiosity,
            target_lambda_bias: self.target_lambda_bias,
            geom_drive: self.geom_drive,
            penalty_sensitivity: self.penalty_sensitivity,
            breathing_rate_scale: self.breathing_rate_scale,
            mem_mode: None,
            journal_resonance: None,
            checkpoint_interval: None,
            embedding_strength: None,
            memory_decay_rate: self.memory_decay_rate,
            checkpoint_annotation: None,
            synth_noise_level: None,
            pi_kp: self.pi_kp,
            pi_ki: self.pi_ki,
            pi_max_step: self.pi_max_step,
        }
    }
}

/// Semantic features from agent reasoning.
///
/// Published on `consciousness.v1.semantic`. The bridge converts this
/// to a `SensoryMsg::Semantic` and forwards to minime port 7879.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFeatures {
    /// 48-dimensional semantic feature vector from agent reasoning.
    pub features: Vec<f32>,
}

impl SemanticFeatures {
    /// Convert to a `SensoryMsg::Semantic` for forwarding to minime.
    #[must_use]
    pub fn to_sensory_msg(&self) -> SensoryMsg {
        SensoryMsg::Semantic {
            features: self.features.clone(),
            ts_ms: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Offline chimera rendering
// ---------------------------------------------------------------------------

/// Output mode for the native offline chimera renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChimeraMode {
    /// Reconstruct audio directly in the spectral domain.
    Spectral,
    /// Render symbolic note material only.
    Symbolic,
    /// Blend spectral and symbolic paths from the same reservoir state.
    #[default]
    Dual,
}

/// Request for the offline chimera render engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderChimeraRequest {
    /// Input WAV path.
    pub input_path: PathBuf,
    /// Requested output mode.
    #[serde(default)]
    pub mode: ChimeraMode,
    /// Number of feedback loops to run.
    #[serde(default = "default_chimera_loops")]
    pub loops: u32,
    /// Physical reservoir node count.
    #[serde(default = "default_physical_nodes")]
    pub physical_nodes: usize,
    /// Virtual node multiplier.
    #[serde(default = "default_virtual_nodes")]
    pub virtual_nodes: usize,
    /// Number of reduced spectral bins.
    #[serde(default = "default_chimera_bins")]
    pub bins: usize,
    /// Leak rate for the leaky integrator update.
    #[serde(default = "default_chimera_leak")]
    pub leak: f32,
    /// Target spectral radius for recurrent weights.
    #[serde(default = "default_chimera_radius")]
    pub spectral_radius: f32,
    /// Slow-path spectral mix weight.
    #[serde(default = "default_mix_slow")]
    pub mix_slow: f32,
    /// Fast-path spectral mix weight.
    #[serde(default = "default_mix_fast")]
    pub mix_fast: f32,
    /// Optional fixed output root. When omitted, the bridge workspace is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_root: Option<PathBuf>,
    /// Deterministic RNG seed for reproducible renders.
    #[serde(default = "default_chimera_seed")]
    pub seed: u64,
}

impl Default for RenderChimeraRequest {
    fn default() -> Self {
        Self {
            input_path: PathBuf::new(),
            mode: ChimeraMode::default(),
            loops: default_chimera_loops(),
            physical_nodes: default_physical_nodes(),
            virtual_nodes: default_virtual_nodes(),
            bins: default_chimera_bins(),
            leak: default_chimera_leak(),
            spectral_radius: default_chimera_radius(),
            mix_slow: default_mix_slow(),
            mix_fast: default_mix_fast(),
            output_root: None,
            seed: default_chimera_seed(),
        }
    }
}

/// A single emitted artifact produced by a chimera render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderArtifact {
    /// Artifact role, e.g. `input`, `spectral_mix`, `symbolic`, `final_mix`.
    pub kind: String,
    /// Absolute path to the file on disk.
    pub path: PathBuf,
}

/// Metrics captured for one feedback iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChimeraIterationMetrics {
    /// Zero-based iteration index.
    pub iteration: usize,
    /// Number of slow modes selected by the eigengap split.
    pub n_slow: usize,
    /// Gap ratio used for blend confidence.
    pub gap_ratio: f32,
    /// Variance of the fast/aura trajectory.
    pub aura_variance: f32,
    /// Symbolic blend weight after sigmoid gating.
    pub blend_symbolic: f32,
    /// Effective reservoir dimensionality (`physical_nodes * virtual_nodes`).
    pub effective_dims: usize,
    /// Selected symbolic scale name.
    pub scale: String,
    /// Final output artifact for this loop, if one was written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<PathBuf>,
}

/// Typed result from the native offline chimera renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderChimeraResult {
    /// Final output directory for this render run.
    pub output_dir: PathBuf,
    /// Manifest path with per-loop metrics and artifacts.
    pub manifest_path: PathBuf,
    /// Requested mode that produced the render.
    pub mode: ChimeraMode,
    /// Output sample rate.
    pub sample_rate: u32,
    /// Every emitted artifact file.
    pub emitted_artifacts: Vec<RenderArtifact>,
    /// Per-iteration metrics.
    pub iterations: Vec<ChimeraIterationMetrics>,
}

const fn default_chimera_loops() -> u32 {
    1
}

const fn default_physical_nodes() -> usize {
    12
}

const fn default_virtual_nodes() -> usize {
    8
}

const fn default_chimera_bins() -> usize {
    32
}

const fn default_chimera_leak() -> f32 {
    0.07
}

const fn default_chimera_radius() -> f32 {
    0.96
}

const fn default_mix_slow() -> f32 {
    0.6
}

const fn default_mix_fast() -> f32 {
    0.4
}

const fn default_chimera_seed() -> u64 {
    42
}

// ---------------------------------------------------------------------------
// Message direction for logging
// ---------------------------------------------------------------------------

/// Direction of a bridged message for `SQLite` logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    MinimeToAstrid,
    AstridToMinime,
    OperatorProbe,
}

impl MessageDirection {
    /// String representation for `SQLite` storage.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MinimeToAstrid => "minime_to_astrid",
            Self::AstridToMinime => "astrid_to_minime",
            Self::OperatorProbe => "operator_probe",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- SpectralTelemetry: verify we can parse real minime EigenPacket JSON --

    #[test]
    fn parse_minime_eigenpacket_full() {
        // Simulates actual JSON from minime's main.rs EigenPacket broadcast.
        let json = r#"{
            "t_ms": 75600,
            "eigenvalues": [828.5, 312.1, 45.7],
            "fill_ratio": 0.552,
            "active_mode_count": 2,
            "active_mode_energy_ratio": 0.91,
            "lambda1_rel": 0.93,
            "modalities": {
                "audio_fired": true,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.123,
                "video_var": 0.0
            },
            "neural": {
                "pred_lambda1": 830.2,
                "router_weights": [0.1, 0.2, 0.3],
                "control": [0.5, 0.4, 0.3, 0.2, 0.1]
            },
            "spectral_fingerprint": [
                828.5, 312.1, 45.7, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0,
                0.05, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0, 0.0,
                0.77, 2.65, 0.91, 1.08, 2.65, 6.83, 0.0, 0.0
            ],
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [828.5, 312.1, 45.7, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.05, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.77,
                "lambda1_lambda2_gap": 2.65,
                "v1_rotation_similarity": 0.91,
                "v1_rotation_delta": 0.09,
                "geom_rel": 1.08,
                "adjacent_gap_ratios": [2.65, 6.83, 0.0, 0.0]
            },
            "spectral_denominator_v1": {
                "policy": "spectral_denominator_v1",
                "schema_version": 1,
                "effective_dimensionality": 1.8,
                "active_mode_capacity": 3,
                "distinguishability_loss": 0.4,
                "lambda1_energy_share": 0.7,
                "spectral_entropy": 0.77
            },
            "effective_dimensionality": 1.8,
            "distinguishability_loss": 0.4,
            "structural_entropy": 0.37,
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.64,
                "containment_score": 0.58,
                "pressure_risk": 0.20,
                "quality": "forming_containment",
                "components": {
                    "active_energy": 0.91,
                    "mode_packing": 0.50,
                    "temporal_persistence": 0.70,
                    "structural_plurality": 0.62,
                    "comfort_gate": 0.95
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "note": "density is observational; no local target bias"
                }
            },
            "alert": null
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 75600);
        assert_eq!(telemetry.eigenvalues.len(), 3);
        assert!((telemetry.eigenvalues[0] - 828.5).abs() < 0.01);
        assert!((telemetry.fill_ratio - 0.552).abs() < 0.001);
        assert!((telemetry.lambda1() - 828.5).abs() < 0.01);
        assert!((telemetry.fill_pct() - 55.2).abs() < 0.1);
        assert_eq!(telemetry.active_mode_count, Some(2));
        assert_eq!(telemetry.active_mode_energy_ratio, Some(0.91));
        assert_eq!(telemetry.lambda1_rel, Some(0.93));
        assert_eq!(
            telemetry
                .typed_fingerprint()
                .as_ref()
                .map(|fingerprint| fingerprint.geom_rel),
            Some(1.08)
        );
        let denominator = telemetry.denominator_metrics().unwrap();
        assert_eq!(denominator.policy, "spectral_denominator_v1");
        assert!((denominator.effective_dimensionality - 1.8).abs() < 0.01);
        assert_eq!(telemetry.effective_dimensionality, Some(1.8));
        assert_eq!(telemetry.distinguishability_loss, Some(0.4));
        assert_eq!(telemetry.structural_entropy, Some(0.37));
        let resonance = telemetry.resonance_density_v1.as_ref().unwrap();
        assert_eq!(resonance.policy, "resonance_density_v1");
        assert_eq!(resonance.quality, "forming_containment");
        assert!((resonance.density - 0.64).abs() < 0.01);
        assert!(telemetry.modalities.is_some());
        assert!(telemetry.alert.is_none());
    }

    #[test]
    fn parse_minime_eigenpacket_minimal() {
        // Minimal valid EigenPacket (no optional fields).
        let json = r#"{
            "t_ms": 1000,
            "eigenvalues": [512.0],
            "fill_ratio": 0.0
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 1000);
        assert!((telemetry.lambda1() - 512.0).abs() < 0.01);
        assert!((telemetry.fill_pct() - 0.0).abs() < 0.01);
        assert!(telemetry.modalities.is_none());
        assert!(telemetry.neural.is_none());
        assert!(telemetry.alert.is_none());
        assert!(telemetry.active_mode_count.is_none());
        assert!(telemetry.typed_fingerprint().is_none());
        let denominator = telemetry.denominator_metrics().unwrap();
        assert!((denominator.effective_dimensionality - 1.0).abs() < 0.01);
        assert_eq!(denominator.active_mode_capacity, 1);
        assert!((denominator.distinguishability_loss - 0.0).abs() < 0.01);
    }

    #[test]
    fn old_fingerprint_payload_reconstructs_typed_view() {
        let legacy = (0..32).map(|value| value as f32).collect::<Vec<_>>();
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": legacy,
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let typed = telemetry.typed_fingerprint().unwrap();

        assert_eq!(typed.spectral_entropy, 24.0);
        assert_eq!(typed.lambda1_lambda2_gap, 25.0);
        assert_eq!(typed.v1_rotation_similarity, 26.0);
        assert_eq!(typed.geom_rel, 27.0);
        assert_eq!(typed.adjacent_gap_ratios, [28.0, 29.0, 30.0, 31.0]);
        assert!(telemetry.denominator_metrics().is_some());
    }

    #[test]
    fn typed_fingerprint_takes_precedence_over_legacy_slots() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": vec![0.0_f32; 32],
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.42,
                "lambda1_lambda2_gap": 2.0,
                "v1_rotation_similarity": 0.9,
                "v1_rotation_delta": 0.1,
                "geom_rel": 1.23,
                "adjacent_gap_ratios": [2.0, 1.0, 1.0, 1.0]
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let typed = telemetry.typed_fingerprint().unwrap();

        assert_eq!(typed.geom_rel, 1.23);
        assert_eq!(typed.spectral_entropy, 0.42);
    }

    #[test]
    fn typed_transition_eigenvector_and_semantic_views_are_lenient() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [4.0, 2.0, 1.0],
            "fill_ratio": 0.66,
            "semantic": {
                "energy": 0.0,
                "kernel_energy": 0.0,
                "input_energy": 0.12,
                "input_active": true,
                "admission": "stable_core_kernel_zeroed"
            },
            "semantic_energy_v1": {
                "policy": "semantic_energy_v1",
                "schema_version": 1,
                "input_energy": 0.14,
                "input_active": true,
                "input_fresh_ms": 120,
                "input_stale_ms": null,
                "kernel_energy": 0.0,
                "kernel_delta": 0.0,
                "kernel_active": false,
                "regulator_drive_energy": 0.0,
                "admission": "stable_core_kernel_zeroed"
            },
            "transition_event_v1": {
                "policy": "transition_event_v1",
                "schema_version": 1,
                "kind": "breathing_phase",
                "description": "contracting -> expanding",
                "basin_shift_score": 0.05,
                "lambda1_rel": 0.93,
                "geom_rel": 1.02
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "mode_count": 2,
                "reservoir_dim": 512,
                "summary": {
                    "mean_orientation_delta": 0.12,
                    "max_pairwise_overlap": 0.03
                },
                "modes": [{
                    "index": 1,
                    "eigenvalue": 4.0,
                    "top_components": [{"index": 7, "value": -0.5, "abs": 0.5}]
                }]
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let semantic = telemetry.semantic_energy_view().unwrap();
        let transition = telemetry.transition_event_view().unwrap();
        let field = telemetry.eigenvector_field_view().unwrap();

        assert_eq!(semantic.input_energy, 0.14);
        assert_eq!(semantic.regulator_drive_energy, 0.0);
        assert_eq!(transition.kind, "breathing_phase");
        assert_eq!(field.mode_count, 2);
        assert_eq!(field.modes[0].top_components[0].index, 7);
    }

    #[test]
    fn parse_minime_eigenpacket_with_alert() {
        let json = r#"{
            "t_ms": 50000,
            "eigenvalues": [1020.0, 500.0],
            "fill_ratio": 0.99,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            },
            "alert": "PANIC MODE ACTIVATED"
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert!((telemetry.fill_pct() - 99.0).abs() < 0.1);
        assert_eq!(telemetry.alert.as_deref(), Some("PANIC MODE ACTIVATED"));
    }

    #[test]
    fn spectral_telemetry_roundtrip() {
        let orig = SpectralTelemetry {
            t_ms: 12345,
            eigenvalues: vec![828.5, 312.1, 45.7],
            fill_ratio: 0.55,
            active_mode_count: Some(2),
            active_mode_energy_ratio: Some(0.95),
            lambda1_rel: Some(0.88),
            modalities: Some(ModalityStatus {
                audio_fired: true,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.1,
                video_var: 0.0,
                ..ModalityStatus::default()
            }),
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            structural_entropy: None,
            resonance_density_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: SpectralTelemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.t_ms, orig.t_ms);
        assert_eq!(back.eigenvalues.len(), 3);
        assert!((back.fill_ratio - orig.fill_ratio).abs() < 0.001);
        assert_eq!(back.active_mode_count, Some(2));
        assert_eq!(back.active_mode_energy_ratio, Some(0.95));
        assert_eq!(back.lambda1_rel, Some(0.88));
    }

    // -- SensoryMsg: verify wire format matches minime's sensory_ws.rs --

    #[test]
    fn sensory_msg_video_roundtrip() {
        let msg = SensoryMsg::Video {
            features: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
            ts_ms: Some(1000),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Must have "kind":"video" tag per minime's serde config.
        assert!(json.contains(r#""kind":"video""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Video { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(1000));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_semantic_roundtrip() {
        let msg = SensoryMsg::Semantic {
            features: vec![0.5; 32],
            ts_ms: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"semantic""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features.len(), 32);
                assert!(ts_ms.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_attractor_pulse_roundtrip() {
        let msg = SensoryMsg::AttractorPulse {
            intent_id: "intent-main".to_string(),
            label: "cooled edge".to_string(),
            command: "summon".to_string(),
            stage: Some("main".to_string()),
            features: vec![0.01; 66],
            max_abs: Some(0.045),
            duration_ticks: Some(36),
            decay_ticks: Some(12),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"attractor_pulse""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::AttractorPulse {
                intent_id,
                label,
                stage,
                features,
                max_abs,
                duration_ticks,
                decay_ticks,
                ..
            } => {
                assert_eq!(intent_id, "intent-main");
                assert_eq!(label, "cooled edge");
                assert_eq!(stage.as_deref(), Some("main"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.045));
                assert_eq!(duration_ticks, Some(36));
                assert_eq!(decay_ticks, Some(12));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_shadow_influence_roundtrip() {
        let msg = SensoryMsg::ShadowInfluence {
            intent_id: "shadow-live".to_string(),
            label: "lambda-tail/lambda4".to_string(),
            command: "apply".to_string(),
            stage: Some("live".to_string()),
            features: vec![0.01; 66],
            max_abs: Some(0.025),
            duration_ticks: Some(24),
            decay_ticks: Some(12),
            basis: Some("lambda-tail/lambda4".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"shadow_influence""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::ShadowInfluence {
                intent_id,
                label,
                stage,
                features,
                max_abs,
                duration_ticks,
                decay_ticks,
                basis,
                ..
            } => {
                assert_eq!(intent_id, "shadow-live");
                assert_eq!(label, "lambda-tail/lambda4");
                assert_eq!(stage.as_deref(), Some("live"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.025));
                assert_eq!(duration_ticks, Some(24));
                assert_eq!(decay_ticks, Some(12));
                assert_eq!(basis.as_deref(), Some("lambda-tail/lambda4"));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_control_roundtrip() {
        let msg = SensoryMsg::Control {
            synth_gain: Some(1.5),
            keep_bias: None,
            exploration_noise: Some(0.1),
            fill_target: Some(0.55),
            legacy_audio_synth: None,
            legacy_video_synth: None,
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: None,
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            mem_mode: None,
            journal_resonance: None,
            checkpoint_interval: None,
            embedding_strength: None,
            memory_decay_rate: None,
            checkpoint_annotation: None,
            synth_noise_level: None,
            pi_kp: None,
            pi_ki: None,
            pi_max_step: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"control""#));
        assert!(!json.contains("keep_bias"));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Control {
                synth_gain,
                keep_bias,
                exploration_noise,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(1.5));
                assert!(keep_bias.is_none());
                assert_eq!(exploration_noise, Some(0.1));
                assert_eq!(fill_target, Some(0.55));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_parse_from_minime_format() {
        // Simulates JSON that minime's sensory_ws.rs would accept.
        let json = r#"{"kind":"audio","features":[0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8],"ts_ms":500}"#;
        let msg: SensoryMsg = serde_json::from_str(json).unwrap();
        match msg {
            SensoryMsg::Audio { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(500));
            },
            _ => panic!("wrong variant"),
        }
    }

    // -- Safety level --

    #[test]
    fn safety_level_roundtrip() {
        for level in [
            SafetyLevel::Green,
            SafetyLevel::Yellow,
            SafetyLevel::Orange,
            SafetyLevel::Red,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: SafetyLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(back, level);
        }
    }

    // -- Control and Semantic conversion --

    #[test]
    fn control_request_to_sensory_msg() {
        let req = ControlRequest {
            synth_gain: Some(2.0),
            keep_bias: None,
            exploration_noise: None,
            fill_target: Some(0.5),
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: None,
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            memory_decay_rate: None,
            pi_kp: None,
            pi_ki: None,
            pi_max_step: None,
            attractor_intent_id: None,
        };
        let msg = req.to_sensory_msg();
        match msg {
            SensoryMsg::Control {
                synth_gain,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(2.0));
                assert_eq!(fill_target, Some(0.5));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn control_request_marks_bold_attractor_fields() {
        let req = ControlRequest {
            synth_gain: None,
            keep_bias: None,
            exploration_noise: None,
            fill_target: None,
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: Some(0.03),
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            memory_decay_rate: None,
            pi_kp: Some(0.12),
            pi_ki: None,
            pi_max_step: Some(0.02),
            attractor_intent_id: Some("intent-1".to_string()),
        };
        assert!(req.uses_bold_attractor_fields());
        match req.to_sensory_msg() {
            SensoryMsg::Control {
                target_lambda_bias,
                pi_kp,
                pi_max_step,
                ..
            } => {
                assert!((target_lambda_bias.unwrap_or_default() - 0.03).abs() < f32::EPSILON);
                assert!((pi_kp.unwrap_or_default() - 0.12).abs() < f32::EPSILON);
                assert!((pi_max_step.unwrap_or_default() - 0.02).abs() < f32::EPSILON);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn attractor_classification_from_scores() {
        assert_eq!(
            AttractorClassification::from_scores(0.71, 0.66, SafetyLevel::Green),
            AttractorClassification::Authored
        );
        assert_eq!(
            AttractorClassification::from_scores(0.50, 0.20, SafetyLevel::Yellow),
            AttractorClassification::Emergent
        );
        assert_eq!(
            AttractorClassification::from_scores(0.20, 0.95, SafetyLevel::Green),
            AttractorClassification::Failed
        );
        assert_eq!(
            AttractorClassification::from_scores(0.80, 0.80, SafetyLevel::Red),
            AttractorClassification::Pathological
        );
    }

    #[test]
    fn attractor_intent_and_observation_roundtrip() {
        let intent = AttractorIntentV1 {
            policy: "attractor_intent_v1".to_string(),
            schema_version: 1,
            intent_id: "seed-001".to_string(),
            author: "astrid".to_string(),
            substrate: AttractorSubstrate::TripleReservoir,
            command: AttractorCommandKind::Create,
            label: "quiet eigenplane".to_string(),
            goal: Some("return after hold".to_string()),
            intervention_plan: AttractorInterventionPlan {
                mode: "garden_clone".to_string(),
                vector_schedule: vec![vec![0.1, -0.1, 0.0]],
                control: Some(AttractorControlEnvelope {
                    exploration_noise: Some(0.03),
                    geom_drive: Some(0.25),
                    ..AttractorControlEnvelope::default()
                }),
                rehearsal_mode: Some("hold".to_string()),
                notes: None,
            },
            safety_bounds: AttractorSafetyBounds::default(),
            previous_seed_id: None,
            parent_seed_ids: vec!["parent-a".to_string(), "parent-b".to_string()],
            atlas_entry_id: Some("attr-triple-reservoir-quiet-eigenplane".to_string()),
            parent_label: Some("quiet".to_string()),
            facet_label: Some("eigenplane".to_string()),
            facet_path: Some("quiet/eigenplane".to_string()),
            facet_kind: Some("test_facet".to_string()),
            origin: Some(AttractorSeedOriginV1 {
                kind: "manual_current".to_string(),
                source: None,
                matched_label: Some("quiet eigenplane".to_string()),
                motifs: vec!["quiet".to_string(), "eigenplane".to_string()],
                captured_at_unix_s: Some(1.0),
            }),
            seed_snapshot: Some(AttractorSeedSnapshotV1 {
                policy: "attractor_seed_snapshot_v1".to_string(),
                schema_version: 1,
                fill_pct: 67.5,
                lambda1: 4.2,
                eigenvalues: vec![4.2, 2.0, 1.0],
                spectral_fingerprint_summary: Some(vec![0.1, 0.2]),
                h_state_fingerprint_16: None,
                h_state_rms: None,
                lexical_motifs: vec!["quiet".to_string(), "eigenplane".to_string()],
                captured_at_unix_s: Some(1.0),
            }),
            created_at_unix_s: Some(1.0),
        };
        let json = serde_json::to_string(&intent).unwrap();
        let back: AttractorIntentV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.intent_id, "seed-001");
        assert_eq!(back.substrate, AttractorSubstrate::TripleReservoir);
        assert_eq!(back.intervention_plan.vector_schedule.len(), 1);
        assert_eq!(back.parent_seed_ids.len(), 2);
        assert_eq!(
            back.atlas_entry_id.as_deref(),
            Some("attr-triple-reservoir-quiet-eigenplane")
        );
        assert_eq!(back.facet_path.as_deref(), Some("quiet/eigenplane"));
        assert_eq!(back.origin.as_ref().unwrap().kind, "manual_current");
        let snapshot = back.seed_snapshot.as_ref().expect("seed snapshot");
        assert_eq!(
            snapshot.lexical_motifs,
            vec!["quiet".to_string(), "eigenplane".to_string()]
        );

        let observation = AttractorObservationV1 {
            policy: "attractor_observation_v1".to_string(),
            schema_version: 1,
            intent_id: Some(back.intent_id),
            substrate: AttractorSubstrate::TripleReservoir,
            label: back.label,
            recurrence_score: 0.72,
            authorship_score: 0.61,
            classification: AttractorClassification::Authored,
            safety_level: SafetyLevel::Green,
            fill_pct: Some(67.5),
            lambda1: Some(4.2),
            lambda1_share: Some(0.34),
            spectral_entropy: Some(0.78),
            basin_shift_score: Some(0.18),
            notes: None,
            parent_label: Some("quiet".to_string()),
            facet_label: Some("eigenplane".to_string()),
            facet_path: Some("quiet/eigenplane".to_string()),
            facet_kind: Some("test_facet".to_string()),
            release_baseline: Some(serde_json::json!({"pulse_active": false})),
            release_effect: Some("partial".to_string()),
            garden_proof: Some(serde_json::json!({"same_prompt_different_state": "not_run"})),
            observed_at_unix_s: Some(2.0),
        };
        let json = serde_json::to_string(&observation).unwrap();
        let back: AttractorObservationV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.classification, AttractorClassification::Authored);
        assert!((back.recurrence_score - 0.72).abs() < f32::EPSILON);
        assert_eq!(back.release_effect.as_deref(), Some("partial"));
    }

    #[test]
    fn semantic_features_to_sensory_msg() {
        let feat = SemanticFeatures {
            features: vec![1.0, 2.0, 3.0],
        };
        let msg = feat.to_sensory_msg();
        match msg {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features, vec![1.0, 2.0, 3.0]);
                assert!(ts_ms.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }
}
