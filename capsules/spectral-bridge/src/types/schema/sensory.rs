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
    #[serde(default)]
    pub audio_freshness_class: Option<String>,
    #[serde(default)]
    pub video_freshness_class: Option<String>,
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
        /// Runtime PI integrator leak. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_integrator_leak: Option<f32>,
        /// Gated one-shot direct ESN leak override. Authority-gate execution only.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_authority_request_id: Option<String>,
        /// Being-driven spectral *dispersal* ("PERTURB SPREAD" / porosity).
        /// Strength in `[0.0, 1.0]`; the minime engine synthesizes a broadband,
        /// zero-mean perturbation that spills λ₁ energy into λ₂–λ₅, applied
        /// through the bounded, self-decaying, fill-suspending shadow-influence
        /// machinery. Field names mirror minime's `sensory_ws.rs` Control variant
        /// exactly so the JSON keys round-trip across the 7879 sensory channel.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse: Option<f32>,
        /// Optional dispersal window length (ticks held at full strength before
        /// linear release). Clamped by the engine's shadow-influence path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_duration_ticks: Option<u32>,
        /// Optional dispersal release length (ticks of linear fade to zero).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_decay_ticks: Option<u32>,
    },
}
