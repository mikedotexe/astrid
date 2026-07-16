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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_integrator_leak: Option<f32>,
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
            || self.pi_integrator_leak.is_some()
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
            pi_integrator_leak: self.pi_integrator_leak,
            esn_leak_override: None,
            esn_leak_override_ticks: None,
            esn_leak_authority_request_id: None,
            mode_disperse: None,
            mode_disperse_duration_ticks: None,
            mode_disperse_decay_ticks: None,
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
