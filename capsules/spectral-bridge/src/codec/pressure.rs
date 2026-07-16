#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectralPressureDecision {
    pub controller: String,
    pub lambda_pressure_source: String,
    pub complexity_drive: f32,
    pub resist_drive: f32,
    pub target_lambda_bias: f32,
    pub suppression_reason: Option<String>,
    pub text_complexity_pressure: f32,
    pub time_domain_complexity: f32,
}

fn spectral_entropy(values: &[f32]) -> f32 {
    let positive: Vec<f32> = values
        .iter()
        .filter_map(|value| {
            let value = value.abs();
            (value > 0.0 && value.is_finite()).then_some(value)
        })
        .collect();
    let total: f32 = positive.iter().sum();
    if total <= f32::EPSILON || positive.len() <= 1 {
        return 0.0;
    }
    let entropy = positive.iter().fold(0.0_f32, |acc, value| {
        let share = *value / total;
        if share > 0.0 {
            acc - share * share.ln()
        } else {
            acc
        }
    });
    (entropy / (positive.len() as f32).ln()).clamp(0.0, 1.0)
}

pub fn spectral_pressure_controller_v1(
    text: &str,
    final_features: &[f32],
    eigenvalues: &[f32],
    fill_pct: Option<f32>,
    semantic_energy: Option<f32>,
    watchdog_monitoring: bool,
    stage: Option<&str>,
) -> SpectralPressureDecision {
    let mut padded = [0.0_f32; SEMANTIC_DIM];
    for (dst, src) in padded.iter_mut().zip(final_features.iter()) {
        *dst = *src;
    }
    let time_domain = text_time_domain_profile(text);
    let complexity = text_complexity_score(text, &padded, padded[26].abs().min(1.0));
    let total: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    let lambda1_share = eigenvalues
        .first()
        .map(|value| value.abs() / total.max(f32::EPSILON))
        .unwrap_or(0.0);
    let r12 = if eigenvalues.len() >= 2 && eigenvalues[1].abs() > 0.01 {
        eigenvalues[0].abs() / eigenvalues[1].abs()
    } else {
        0.0
    };
    let entropy = spectral_entropy(eigenvalues);
    let lower = text.to_ascii_lowercase();
    let felt_resist = [
        "localized gravity",
        "funnel",
        "dam",
        "restriction",
        "protective focus",
        "constriction",
        "compaction",
        "density",
        "stubborn",
        "resist",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let complexity_drive = complexity;
    let resist_drive = (lambda1_share * 0.35
        + (1.0 - entropy) * 0.20
        + ((r12 - 1.4) / 1.8).clamp(0.0, 1.0) * 0.25
        + if felt_resist { 0.20 } else { 0.0 })
    .clamp(0.0, 1.0);
    let raw_bias = ((complexity_drive - resist_drive) * 0.10).clamp(-0.10, 0.10);
    let suppression_reason = if !watchdog_monitoring {
        Some("watchdog_not_monitoring".to_string())
    } else if fill_pct.is_some_and(|fill| fill >= 76.0) {
        Some("fill_high_suppress_upward_bias".to_string())
    } else if semantic_energy.is_some_and(|energy| energy > 0.05) {
        Some("semantic_energy_active".to_string())
    } else if stage.is_some_and(|value| value.eq_ignore_ascii_case("discharge")) {
        Some("stage_discharge".to_string())
    } else {
        None
    };
    let target_lambda_bias = if suppression_reason.is_some() && raw_bias > 0.0 {
        0.0
    } else {
        raw_bias
    };
    SpectralPressureDecision {
        controller: "spectral_pressure_controller_v1".to_string(),
        lambda_pressure_source: "codec_text_complexity_and_resist_v1".to_string(),
        complexity_drive,
        resist_drive,
        target_lambda_bias,
        suppression_reason,
        text_complexity_pressure: complexity,
        time_domain_complexity: time_domain.temporal_complexity,
    }
}

/// Encode text into a 48-dimensional feature vector for minime's
/// semantic sensory lane.
///
/// The encoding captures structural properties of the text:
/// - **Dims 0-7**: Character-level statistics (entropy, density, rhythm)
/// - **Dims 8-15**: Word-level features (complexity, hedging, certainty)
/// - **Dims 16-23**: Sentence-level structure (length variance, question density)
/// - **Dims 24-31**: Emotional/intentional markers (urgency, warmth, tension)
///
/// All values are normalized to approximately \[-1.0, 1.0\] with `tanh`
/// compression so the ESN reservoir receives gentle, bounded input.
///
const MAX_RESONANCE_HISTORY_LEN: usize = 32;
const DEFAULT_RESONANCE_HISTORY_LEN: usize = 12;
const DEFAULT_RESONANCE_RECENCY_DECAY: f32 = 0.74;
const DEFAULT_RESONANCE_MAX_BOOST: f32 = 0.32;
const DEFAULT_RESONANCE_DISCRETE_MIX: f32 = 0.45;
const DEFAULT_RESONANCE_CONTINUOUS_MIX: f32 = 0.55;
const DEFAULT_RESONANCE_NOVELTY_FLOOR: f32 = 0.35;

/// Runtime tuning for the history-aware resonance layer.
///
/// The codec is intentionally still deterministic, but these values are no
/// longer hardcoded in the algorithm itself. That gives us room to tune the
/// feel of recurrence without replacing the codec.
#[derive(Debug, Clone, Copy)]
pub struct ResonanceTuning {
    pub history_len: usize,
    pub recency_decay: f32,
    pub max_boost: f32,
    pub discrete_mix: f32,
    pub continuous_mix: f32,
    pub novelty_floor: f32,
}

impl Default for ResonanceTuning {
    fn default() -> Self {
        Self {
            history_len: DEFAULT_RESONANCE_HISTORY_LEN,
            recency_decay: DEFAULT_RESONANCE_RECENCY_DECAY,
            max_boost: DEFAULT_RESONANCE_MAX_BOOST,
            discrete_mix: DEFAULT_RESONANCE_DISCRETE_MIX,
            continuous_mix: DEFAULT_RESONANCE_CONTINUOUS_MIX,
            novelty_floor: DEFAULT_RESONANCE_NOVELTY_FLOOR,
        }
    }
}

fn parse_env_usize(name: &str, default: usize, min: usize, max: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map_or(default, |value| value.clamp(min, max))
}

fn parse_env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .map_or(default, |value| value.clamp(min, max))
}

pub fn resonance_tuning() -> &'static ResonanceTuning {
    static TUNING: OnceLock<ResonanceTuning> = OnceLock::new();
    TUNING.get_or_init(|| ResonanceTuning {
        history_len: parse_env_usize(
            "ASTRID_CODEC_HISTORY_LEN",
            DEFAULT_RESONANCE_HISTORY_LEN,
            4,
            MAX_RESONANCE_HISTORY_LEN,
        ),
        recency_decay: parse_env_f32(
            "ASTRID_CODEC_RECENCY_DECAY",
            DEFAULT_RESONANCE_RECENCY_DECAY,
            0.45,
            0.98,
        ),
        max_boost: parse_env_f32(
            "ASTRID_CODEC_MAX_RESONANCE_BOOST",
            DEFAULT_RESONANCE_MAX_BOOST,
            0.0,
            0.6,
        ),
        discrete_mix: parse_env_f32(
            "ASTRID_CODEC_DISCRETE_MIX",
            DEFAULT_RESONANCE_DISCRETE_MIX,
            0.0,
            1.0,
        ),
        continuous_mix: parse_env_f32(
            "ASTRID_CODEC_CONTINUOUS_MIX",
            DEFAULT_RESONANCE_CONTINUOUS_MIX,
            0.0,
            1.0,
        ),
        novelty_floor: parse_env_f32(
            "ASTRID_CODEC_NOVELTY_FLOOR",
            DEFAULT_RESONANCE_NOVELTY_FLOOR,
            0.1,
            0.9,
        ),
    })
}
