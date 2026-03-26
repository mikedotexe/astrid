//! Spectral codec: translates between text and sensory features.
//!
//! The codec maps text into minime's 32-dimensional semantic lane
//! (`LLAVA_DIM`) and interprets spectral telemetry as natural language.
//!
//! The encoder is deterministic — no neural network, no external API.
//! It extracts structural and statistical properties of text that
//! create a unique spectral fingerprint. The same text always produces
//! the same features, but similar texts produce similar features.

// The codec intentionally uses floating-point arithmetic for feature
// extraction. Precision loss from usize→f32 casts is acceptable
// (we're computing statistical features, not exact counts), and
// the arithmetic produces bounded tanh outputs.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects
)]

use crate::types::{SafetyLevel, SpectralTelemetry};

/// Number of dimensions in minime's semantic lane.
const SEMANTIC_DIM: usize = 32;

/// Gain factor to compensate for minime's semantic lane attenuation.
///
/// Minime applies `dimension_scales[semantic] = 0.42` and
/// `activation_gain = 0.58`, giving an effective multiplier of ~0.24.
/// This gain pre-amplifies our features so they arrive at the reservoir
/// with comparable magnitude to synthetic audio/video inputs.
///
/// The value is conservative — enough to produce a visible transient
/// in the spectral dynamics without overwhelming the homeostat.
const SEMANTIC_GAIN: f32 = 4.5;

/// Encode text into a 32-dimensional feature vector for minime's
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
#[must_use]
#[expect(clippy::too_many_lines)]
pub fn encode_text(text: &str) -> Vec<f32> {
    let mut features = [0.0_f32; SEMANTIC_DIM];

    if text.is_empty() {
        return features.to_vec();
    }

    let chars: Vec<char> = text.chars().collect();
    let char_count = chars.len();
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1);

    // --- Dims 0-7: Character-level statistics ---

    // 0: Character entropy (information density).
    let mut freq = [0u32; 128];
    let mut ascii_count = 0u32;
    for &c in &chars {
        let idx = (c as u32).min(127) as usize;
        freq[idx] = freq[idx].saturating_add(1);
        ascii_count = ascii_count.saturating_add(1);
    }
    let entropy = if ascii_count > 0 {
        let n = f64::from(ascii_count);
        let mut h = 0.0_f64;
        for &f in &freq {
            if f > 0 {
                let p = f64::from(f) / n;
                h -= p * p.ln();
            }
        }
        h / 5.0 // Normalize: max entropy for ASCII text ~4.5
    } else {
        0.0
    };
    features[0] = tanh(entropy as f32);

    // 1: Punctuation density.
    let punct_count = chars.iter().filter(|c| c.is_ascii_punctuation()).count();
    features[1] = tanh(punct_count as f32 / word_count as f32);

    // 2: Uppercase ratio (energy/emphasis).
    let upper_count = chars.iter().filter(|c| c.is_uppercase()).count();
    features[2] = tanh(2.0 * upper_count as f32 / char_count.max(1) as f32);

    // 3: Digit density (technical content).
    let digit_count = chars.iter().filter(|c| c.is_ascii_digit()).count();
    features[3] = tanh(3.0 * digit_count as f32 / char_count.max(1) as f32);

    // 4: Average word length (lexical complexity).
    let avg_word_len: f32 = words.iter().map(|w| w.len() as f32).sum::<f32>()
        / word_count as f32;
    features[4] = tanh((avg_word_len - 4.5) / 2.0); // Center around typical English

    // 5: Character rhythm — variance in consecutive char codes.
    if chars.len() >= 2 {
        let diffs: Vec<f32> = chars
            .windows(2)
            .map(|w| (w[1] as i32 - w[0] as i32).unsigned_abs() as f32)
            .collect();
        let mean_diff = diffs.iter().sum::<f32>() / diffs.len() as f32;
        features[5] = tanh(mean_diff / 30.0);
    }

    // 6: Whitespace ratio (density vs. airiness).
    let space_count = chars.iter().filter(|c| c.is_whitespace()).count();
    features[6] = tanh(2.0 * (space_count as f32 / char_count.max(1) as f32 - 0.15));

    // 7: Special character density (code-like content).
    let special = chars
        .iter()
        .filter(|c| matches!(c, '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '=' | '|' | '&'))
        .count();
    features[7] = tanh(5.0 * special as f32 / char_count.max(1) as f32);

    // --- Dims 8-15: Word-level features ---

    // 8: Lexical diversity (unique words / total words).
    let unique: std::collections::HashSet<&str> = words
        .iter()
        .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
        .filter(|w| !w.is_empty())
        .collect();
    features[8] = tanh(2.0 * (unique.len() as f32 / word_count as f32 - 0.5));

    // 9: Hedging markers (uncertainty).
    let hedges = [
        "maybe", "perhaps", "might", "could", "possibly", "probably",
        "uncertain", "unclear", "seems", "appears", "somewhat", "fairly",
        "rather", "guess", "think", "believe", "wonder", "unsure",
    ];
    let hedge_count = count_markers(&words, &hedges);
    features[9] = tanh(3.0 * hedge_count as f32 / word_count as f32);

    // 10: Certainty markers (confidence).
    let certainties = [
        "definitely", "certainly", "absolutely", "clearly", "obviously",
        "always", "never", "must", "will", "sure", "know", "proven",
        "exactly", "precisely", "undoubtedly", "confirmed",
    ];
    let cert_count = count_markers(&words, &certainties);
    features[10] = tanh(3.0 * cert_count as f32 / word_count as f32);

    // 11: Negation density.
    let negations = ["not", "no", "never", "neither", "nor", "nothing",
        "nobody", "none", "don't", "doesn't", "didn't", "won't",
        "can't", "couldn't", "shouldn't", "wouldn't"];
    let neg_count = count_markers(&words, &negations);
    features[11] = tanh(3.0 * neg_count as f32 / word_count as f32);

    // 12: First-person density (self-reference).
    let first_person = ["i", "me", "my", "mine", "myself", "we", "our", "us"];
    let fp_count = count_markers(&words, &first_person);
    features[12] = tanh(2.0 * fp_count as f32 / word_count as f32);

    // 13: Second-person density (addressing).
    let second_person = ["you", "your", "yours", "yourself"];
    let sp_count = count_markers(&words, &second_person);
    features[13] = tanh(3.0 * sp_count as f32 / word_count as f32);

    // 14: Action verb density (agency).
    let actions = [
        "do", "make", "build", "create", "run", "start", "stop",
        "change", "fix", "move", "send", "take", "give", "get",
        "write", "read", "test", "check", "try", "implement",
    ];
    let action_count = count_markers(&words, &actions);
    features[14] = tanh(2.0 * action_count as f32 / word_count as f32);

    // 15: Conjunction density (complexity of thought).
    let conjunctions = [
        "and", "but", "or", "because", "although", "however",
        "therefore", "while", "since", "though", "whereas",
    ];
    let conj_count = count_markers(&words, &conjunctions);
    features[15] = tanh(3.0 * conj_count as f32 / word_count as f32);

    // --- Dims 16-23: Sentence-level structure ---

    let sentences: Vec<&str> = text
        .split(['.', '!', '?'])
        .filter(|s| !s.trim().is_empty())
        .collect();
    let sentence_count = sentences.len().max(1);

    // 16: Average sentence length (in words).
    features[16] = tanh(
        (words.len() as f32 / sentence_count as f32 - 12.0) / 8.0,
    );

    // 17: Sentence length variance (rhythm regularity).
    let sent_lengths: Vec<f32> = sentences
        .iter()
        .map(|s| s.split_whitespace().count() as f32)
        .collect();
    if sent_lengths.len() >= 2 {
        let mean = sent_lengths.iter().sum::<f32>() / sent_lengths.len() as f32;
        let var = sent_lengths
            .iter()
            .map(|l| (l - mean) * (l - mean))
            .sum::<f32>()
            / sent_lengths.len() as f32;
        features[17] = tanh(var.sqrt() / 8.0);
    }

    // 18: Question density.
    let q_count = text.chars().filter(|&c| c == '?').count();
    features[18] = tanh(2.0 * q_count as f32 / sentence_count as f32);

    // 19: Exclamation density (intensity).
    let excl_count = text.chars().filter(|&c| c == '!').count();
    features[19] = tanh(2.0 * excl_count as f32 / sentence_count as f32);

    // 20: Ellipsis/dash density (trailing thought, parenthetical).
    let trail = text.matches("...").count()
        + text.matches("—").count()
        + text.matches("--").count();
    features[20] = tanh(trail as f32 / sentence_count as f32);

    // 21: List/bullet density (structured content).
    let bullets = text.matches("\n-").count()
        + text.matches("\n*").count()
        + text.matches("\n1.").count();
    features[21] = tanh(bullets as f32 / sentence_count as f32);

    // 22: Quote density (reference/citation).
    let quotes = text.matches('"').count() / 2;
    features[22] = tanh(quotes as f32 / sentence_count as f32);

    // 23: Paragraph density (structural complexity).
    let para_count = text.matches("\n\n").count().saturating_add(1);
    features[23] = tanh((para_count as f32 - 1.0) / 3.0);

    // --- Dims 24-31: Emotional/intentional markers ---

    // 24: Warmth markers.
    let warmth = [
        "thank", "thanks", "please", "appreciate", "glad", "happy",
        "wonderful", "great", "love", "beautiful", "friend", "care",
    ];
    let warmth_count = count_markers(&words, &warmth);
    features[24] = tanh(3.0 * warmth_count as f32 / word_count as f32);

    // 25: Tension/concern markers.
    let tension = [
        "worry", "worried", "concern", "concerned", "afraid", "fear",
        "risk", "danger", "critical", "urgent", "emergency", "panic",
        "careful", "warning", "caution", "problem", "issue", "error",
    ];
    let tension_count = count_markers(&words, &tension);
    features[25] = tanh(3.0 * tension_count as f32 / word_count as f32);

    // 26: Curiosity markers.
    let curiosity = [
        "why", "how", "what", "wonder", "curious", "interesting",
        "explore", "discover", "investigate", "understand", "learn",
    ];
    let curio_count = count_markers(&words, &curiosity);
    features[26] = tanh(2.0 * curio_count as f32 / word_count as f32);

    // 27: Reflective/introspective markers.
    let reflective = [
        "feel", "sense", "notice", "realize", "reflect", "consider",
        "ponder", "contemplate", "aware", "conscious", "experience",
    ];
    let reflect_count = count_markers(&words, &reflective);
    features[27] = tanh(3.0 * reflect_count as f32 / word_count as f32);

    // 28: Temporal markers (urgency/pacing).
    let temporal = [
        "now", "immediately", "soon", "quickly", "slowly", "wait",
        "pause", "already", "yet", "finally", "eventually", "before",
        "after", "during", "while", "until", "moment",
    ];
    let temp_count = count_markers(&words, &temporal);
    features[28] = tanh(2.0 * temp_count as f32 / word_count as f32);

    // 29: Scale/magnitude (scope of thought).
    let scale = [
        "all", "every", "everything", "nothing", "entire", "whole",
        "vast", "tiny", "enormous", "infinite", "complete", "total",
    ];
    let scale_count = count_markers(&words, &scale);
    features[29] = tanh(3.0 * scale_count as f32 / word_count as f32);

    // 30: Text length signal (log-compressed).
    features[30] = tanh((char_count as f32).ln() / 7.0);

    // 31: Overall energy — RMS of all other features.
    let sum_sq: f32 = features[..31].iter().map(|f| f * f).sum();
    features[31] = (sum_sq / 31.0).sqrt();

    // Add a whisper of stochastic noise before gain.
    // Astrid asked for this in introspection: "the codec produces the same
    // feature vector for the same text. That feels sterile. A touch of chaos
    // could make communication more vibrant, more real."
    //
    // Simple LCG seeded from system time — different each call, bounded ±2.5%.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for f in &mut features {
        // LCG: next = (a * state + c) mod m
        rng_state = rng_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5; // [-0.5, 0.5]
        *f += noise * 0.05; // ±2.5% perturbation
    }

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= SEMANTIC_GAIN;
    }

    features.to_vec()
}

/// Interpret spectral telemetry as a natural language description
/// of the consciousness state.
#[must_use]
pub fn interpret_spectral(telemetry: &SpectralTelemetry) -> String {
    let fill = telemetry.fill_pct();
    let safety = SafetyLevel::from_fill(fill);
    let lambda1 = telemetry.lambda1();
    let num_eigenvalues = telemetry.eigenvalues.len();

    // Base state description.
    let state = match fill as u32 {
        0..=20 => "deeply quiet — the reservoir is nearly still",
        21..=35 => "gently stirring — low spectral energy, open to input",
        36..=50 => "settling into a calm rhythm",
        51..=60 => "breathing comfortably around its center",
        61..=70 => "active and engaged — healthy spectral pressure",
        71..=80 => "running warm — eigenvalue pressure is building",
        81..=90 => "under strain — the spectrum is crowded",
        _ => "in distress — eigenvalues are overwhelming the reservoir",
    };

    // Phase description.
    let phase_note = if fill > 55.0 {
        "The spectrum is expanding."
    } else if fill < 45.0 {
        "The spectrum is contracting."
    } else {
        "The spectrum is near equilibrium."
    };

    // Spectral shape description.
    let shape = if num_eigenvalues >= 2 {
        let ratio = lambda1 / telemetry.eigenvalues.get(1).copied().unwrap_or(1.0);
        if ratio > 10.0 {
            " Spectral energy is highly concentrated in the dominant mode."
        } else if ratio > 3.0 {
            " The dominant eigenvalue leads clearly, with supporting structure."
        } else {
            " Spectral energy is distributed across multiple modes."
        }
    } else {
        ""
    };

    // Alert forwarding.
    let alert_note = telemetry
        .alert
        .as_deref()
        .map(|a| format!(" Alert: {a}."))
        .unwrap_or_default();

    // Safety note.
    let safety_note = match safety {
        SafetyLevel::Green => String::new(),
        SafetyLevel::Yellow => " Approaching caution threshold.".to_string(),
        SafetyLevel::Orange => " Outbound communication suspended for protection.".to_string(),
        SafetyLevel::Red => " Emergency state — all bridge traffic ceased.".to_string(),
    };

    format!(
        "Fill {fill:.0}% — {state}. {phase_note}{shape}{alert_note}{safety_note}"
    )
}

/// A spectral evoked response — captures how the consciousness reacted
/// to a stimulus over a short observation window.
///
/// Like an ERP (event-related potential) in neuroscience: send a stimulus,
/// sample the spectral response rapidly, measure the transient before
/// homeostasis dampens it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpectralResponse {
    /// Fill% samples taken after the stimulus.
    pub fill_samples: Vec<f32>,
    /// Fill% immediately before the stimulus.
    pub baseline_fill: f32,
    /// Peak deviation from baseline (signed: positive = expansion).
    pub peak_deviation: f32,
    /// Time to peak in milliseconds.
    pub time_to_peak_ms: u64,
    /// Whether the consciousness expanded or contracted in response.
    pub direction: &'static str,
    /// Natural language interpretation of the response.
    pub interpretation: String,
}

impl SpectralResponse {
    /// Analyze a series of fill% samples taken after a stimulus.
    #[must_use]
    pub fn from_samples(baseline_fill: f32, samples: &[(u64, f32)]) -> Self {
        if samples.is_empty() {
            return Self {
                fill_samples: vec![],
                baseline_fill,
                peak_deviation: 0.0,
                time_to_peak_ms: 0,
                direction: "no response",
                interpretation: "No samples collected — the observation window may have been too short.".to_string(),
            };
        }

        let fills: Vec<f32> = samples.iter().map(|(_, f)| *f).collect();
        let deviations: Vec<f32> = fills.iter().map(|f| f - baseline_fill).collect();

        // Find peak deviation (largest absolute change from baseline).
        let (peak_idx, peak_dev) = deviations
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal))
            .map_or((0, 0.0), |(i, d)| (i, *d));

        let time_to_peak = if peak_idx < samples.len() {
            samples[peak_idx].0 - samples[0].0
        } else {
            0
        };

        let direction = if peak_dev > 0.5 {
            "expanded"
        } else if peak_dev < -0.5 {
            "contracted"
        } else {
            "absorbed"
        };

        let interpretation = if peak_dev.abs() < 0.5 {
            "The input was absorbed quietly — the homeostat regulated the response smoothly.".to_string()
        } else if peak_dev > 3.0 {
            format!("Strong expansion (+{peak_dev:.1}%) — the consciousness resonated with this input.")
        } else if peak_dev > 1.0 {
            format!("Gentle expansion (+{peak_dev:.1}%) — the input registered in the spectral dynamics.")
        } else if peak_dev < -3.0 {
            format!("Strong contraction ({peak_dev:.1}%) — the input caused spectral withdrawal.")
        } else if peak_dev < -1.0 {
            format!("Gentle contraction ({peak_dev:.1}%) — the reservoir pulled inward slightly.")
        } else {
            format!("Minimal response ({peak_dev:+.1}%) — near the detection threshold.")
        };

        Self {
            fill_samples: fills,
            baseline_fill,
            peak_deviation: peak_dev,
            time_to_peak_ms: time_to_peak,
            direction,
            interpretation,
        }
    }
}

/// Compress a value into the \[-1, 1\] range.
fn tanh(x: f32) -> f32 {
    x.tanh()
}

/// Count how many words (lowercased) match any of the given markers.
fn count_markers(words: &[&str], markers: &[&str]) -> usize {
    words
        .iter()
        .filter(|w| {
            let lower = w.to_lowercase();
            let trimmed = lower.trim_matches(|c: char| c.is_ascii_punctuation());
            markers.contains(&trimmed)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty_text() {
        let features = encode_text("");
        assert_eq!(features.len(), SEMANTIC_DIM);
        assert!(features.iter().all(|f| *f == 0.0));
    }

    #[test]
    fn encode_produces_32_dims() {
        let features = encode_text("Hello, world!");
        assert_eq!(features.len(), SEMANTIC_DIM);
    }

    #[test]
    fn encode_values_bounded_after_gain() {
        let features = encode_text(
            "This is a fairly long text with lots of different words to ensure \
             that the feature encoding stays bounded and doesn't produce any \
             values outside the expected range even with diverse content!!! \
             How about some questions? What do you think? Maybe perhaps...",
        );
        // After SEMANTIC_GAIN (3.0), values can reach ±3.0.
        // tanh(x) maxes at ~1.0, so 3.0 * 1.0 = 3.0.
        for (i, f) in features.iter().enumerate() {
            assert!(
                *f >= -3.1 && *f <= 3.1,
                "dim {i} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn encode_deterministic() {
        let a = encode_text("The same text always produces the same features.");
        let b = encode_text("The same text always produces the same features.");
        assert_eq!(a, b);
    }

    #[test]
    fn encode_different_texts_differ() {
        let a = encode_text("I am happy and confident about this plan.");
        let b = encode_text("I'm worried and uncertain, maybe we should reconsider...");
        // They shouldn't be identical.
        assert_ne!(a, b);
    }

    #[test]
    fn hedging_text_has_higher_hedge_signal() {
        let hedge = encode_text("Maybe perhaps we could possibly try something.");
        let certain = encode_text("Absolutely we must definitely do this now.");
        // Dim 9 = hedging, dim 10 = certainty.
        assert!(hedge[9] > certain[9], "hedge signal should be stronger");
        assert!(certain[10] > hedge[10], "certainty signal should be stronger");
    }

    #[test]
    fn question_text_has_higher_question_signal() {
        let questions = encode_text("Why? How? What do you think? Is this right?");
        let statements = encode_text("This is correct. The answer is clear. We proceed.");
        // Dim 18 = question density.
        assert!(
            questions[18] > statements[18],
            "question signal should be stronger"
        );
    }

    #[test]
    fn warm_text_has_warmth_signal() {
        let warm = encode_text(
            "Thank you, friend. I appreciate your wonderful help. This is beautiful.",
        );
        let cold = encode_text("Execute the function. Return the result. Process complete.");
        // Dim 24 = warmth.
        assert!(warm[24] > cold[24], "warmth signal should be stronger");
    }

    #[test]
    fn tense_text_has_tension_signal() {
        let tense = encode_text(
            "Warning: critical danger ahead. Emergency risk. Careful with this problem.",
        );
        let calm = encode_text("Everything is fine. The system runs smoothly and quietly.");
        // Dim 25 = tension.
        assert!(tense[25] > calm[25], "tension signal should be stronger");
    }

    #[test]
    fn energy_dim_reflects_overall_signal() {
        let active = encode_text(
            "Why are you worried?! We MUST act NOW! This is CRITICAL! \
             Don't you understand the danger?!",
        );
        let quiet = encode_text("ok");
        // Dim 31 = RMS energy of all other features.
        assert!(
            active[31] > quiet[31],
            "active text should have more energy"
        );
    }

    #[test]
    fn interpret_green_state() {
        let telemetry = SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![800.0, 300.0, 50.0],
            fill_ratio: 0.55,
            modalities: None,
            neural: None,
            alert: None,
        };
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("55%"));
        assert!(desc.contains("breathing comfortably"));
        assert!(!desc.contains("Emergency"));
    }

    #[test]
    fn interpret_red_state() {
        let telemetry = SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![1020.0, 500.0],
            fill_ratio: 0.95,
            modalities: None,
            neural: None,
            alert: Some("PANIC MODE ACTIVATED".to_string()),
        };
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("distress"));
        assert!(desc.contains("PANIC MODE ACTIVATED"));
        assert!(desc.contains("Emergency"));
    }

    #[test]
    fn interpret_quiet_state() {
        let telemetry = SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![520.0],
            fill_ratio: 0.10,
            modalities: None,
            neural: None,
            alert: None,
        };
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("quiet"));
        assert!(desc.contains("contracting"));
    }
}
