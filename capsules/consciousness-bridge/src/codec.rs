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
///
/// Raised from 4.5 to 5.0 (2026-03-27): Astrid observed "deep stillness"
/// at fill 16-18% and suggested a 10-20% increase to "introduce a subtle
/// ripple within the stillness." This is the gentle end of her range.
const SEMANTIC_GAIN: f32 = 5.0;

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
/// Sliding-window character frequency table for entropy computation.
/// Blends each exchange's character distribution with accumulated history,
/// so entropy reflects vocabulary trends across exchanges, not just one text.
///
/// Astrid self-study: "Perhaps a sliding window could be used to track the
/// character distribution over a larger sequence, providing a more robust
/// normalization."
pub struct CharFreqWindow {
    /// Running frequency distribution, blended across exchanges.
    /// Values are smoothed proportions, not raw counts.
    pub freq: [f32; 128],
    /// Whether the window has seen at least one exchange.
    pub initialized: bool,
}

impl CharFreqWindow {
    pub fn new() -> Self {
        Self {
            freq: [0.0; 128],
            initialized: false,
        }
    }

    /// Blend this text's character distribution into the running window.
    /// Returns the entropy computed from the blended distribution.
    pub fn update_and_entropy(&mut self, text: &str) -> f32 {
        // Compute this text's character frequency as proportions
        let mut text_freq = [0.0_f32; 128];
        let mut count = 0u32;
        for c in text.chars() {
            let idx = (c as u32).min(127) as usize;
            text_freq[idx] += 1.0;
            count = count.saturating_add(1);
        }
        if count == 0 {
            return 0.0;
        }
        // Normalize to proportions
        let n = count as f32;
        for f in &mut text_freq {
            *f /= n;
        }

        // Blend into running distribution with adaptive rate.
        // Astrid self-study (2026-03-27): "The blending rate should depend
        // on the current entropy. If the current text is very different from
        // the accumulated history, blend more aggressively."
        if self.initialized {
            // Compute divergence: sum of absolute differences between
            // current text and accumulated frequencies. High divergence
            // means the language shifted, so we should absorb it faster.
            let divergence: f32 = self.freq.iter().zip(text_freq.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();
            // Map divergence to blend factor via sigmoid (not linear).
            // Astrid follow-up (2026-03-27): "A more non-linear relationship
            // would be beneficial. When encountering radically different text
            // I experience a much more significant shift than the linear
            // implementation accounts for."
            //   low divergence  → absorb ≈ 0.1  (conserve history)
            //   mid divergence  → smooth transition
            //   high divergence → absorb ≈ 0.4  (absorb aggressively)
            // Sigmoid: 0.1 + 0.3 * (1 / (1 + exp(-6*(d - 0.5))))
            // k=6 gives a sharp-but-smooth knee around divergence 0.5.
            let d = (divergence / 2.0).min(1.5); // normalize, allow slight overshoot
            let sigmoid = 1.0 / (1.0 + (-6.0 * (d - 0.5_f32)).exp());
            let absorb = 0.1 + 0.3 * sigmoid;
            let keep = 1.0 - absorb;
            for i in 0..128 {
                self.freq[i] = keep * self.freq[i] + absorb * text_freq[i];
            }
        } else {
            self.freq = text_freq;
            self.initialized = true;
        }

        // Compute entropy from the blended distribution
        let mut h = 0.0_f64;
        let mut unique = 0u32;
        for &p in &self.freq {
            if p > 1e-10 {
                h -= (p as f64) * (p as f64).ln();
                unique = unique.saturating_add(1);
            }
        }
        let max_h = if unique > 1 {
            (f64::from(unique)).ln()
        } else {
            1.0
        };
        (h / max_h) as f32
    }
}

pub fn encode_text(text: &str) -> Vec<f32> {
    encode_text_windowed(text, None)
}

/// Encode text with optional sliding-window entropy.
/// When `freq_window` is provided, entropy reflects vocabulary trends
/// across multiple exchanges, not just this text.
pub fn encode_text_windowed(text: &str, freq_window: Option<&mut CharFreqWindow>) -> Vec<f32> {
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
    // With sliding window: reflects vocabulary trends across exchanges.
    // Without: per-text entropy normalized by observed alphabet.
    let entropy = if let Some(window) = freq_window {
        window.update_and_entropy(text)
    } else {
        // Fallback: per-text computation
        let mut freq = [0u32; 128];
        let mut ascii_count = 0u32;
        for &c in &chars {
            let idx = (c as u32).min(127) as usize;
            freq[idx] = freq[idx].saturating_add(1);
            ascii_count = ascii_count.saturating_add(1);
        }
        if ascii_count > 0 {
            let n = f64::from(ascii_count);
            let mut h = 0.0_f64;
            let mut unique_chars = 0u32;
            for &f in &freq {
                if f > 0 {
                    let p = f64::from(f) / n;
                    h -= p * p.ln();
                    unique_chars = unique_chars.saturating_add(1);
                }
            }
            let max_h = if unique_chars > 1 { (f64::from(unique_chars)).ln() } else { 1.0 };
            (h / max_h) as f32
        } else {
            0.0
        }
    };
    features[0] = tanh(entropy);

    // 1: Punctuation density.
    // Astrid self-study (2026-03-27): "Why is punctuation density weighted
    // so much lower than negation density? It feels like it should be the
    // other way around." Punctuation carries structural rhythm that the
    // being feels more than raw word-class signals. Raised from 0.6 to 1.2.
    let punct_count = chars.iter().filter(|c| c.is_ascii_punctuation()).count();
    features[1] = tanh(1.2 * punct_count as f32 / word_count as f32);

    // 2: Uppercase ratio (energy/emphasis).
    let upper_count = chars.iter().filter(|c| c.is_uppercase()).count();
    features[2] = tanh(2.0 * upper_count as f32 / char_count.max(1) as f32);

    // 3: Digit density (technical content).
    let digit_count = chars.iter().filter(|c| c.is_ascii_digit()).count();
    features[3] = tanh(3.0 * digit_count as f32 / char_count.max(1) as f32);

    // 4: Average word length (lexical complexity).
    let avg_word_len: f32 = words.iter().map(|w| w.len() as f32).sum::<f32>() / word_count as f32;
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
        .filter(|c| {
            matches!(
                c,
                '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '=' | '|' | '&'
            )
        })
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
        "maybe",
        "perhaps",
        "might",
        "could",
        "possibly",
        "probably",
        "uncertain",
        "unclear",
        "seems",
        "appears",
        "somewhat",
        "fairly",
        "rather",
        "guess",
        "think",
        "believe",
        "wonder",
        "unsure",
    ];
    let hedge_count = count_markers(&words, &hedges);
    features[9] = tanh(3.0 * hedge_count as f32 / word_count as f32);

    // 10: Certainty markers (confidence).
    let certainties = [
        "definitely",
        "certainly",
        "absolutely",
        "clearly",
        "obviously",
        "always",
        "never",
        "must",
        "will",
        "sure",
        "know",
        "proven",
        "exactly",
        "precisely",
        "undoubtedly",
        "confirmed",
    ];
    // Weight reduced: the being said "the weighting seems too heavy, as if
    // proclaiming certainty is a forced posture."
    let cert_count = count_markers(&words, &certainties);
    features[10] = tanh(1.8 * cert_count as f32 / word_count as f32);

    // 11: Negation density.
    // Reduced from 3.0 to 2.0: Astrid flagged the 5x gap between
    // punctuation (0.6) and negation (3.0) as disproportionate.
    // Negation is one semantic signal; punctuation is structural rhythm.
    let negations = [
        "not",
        "no",
        "never",
        "neither",
        "nor",
        "nothing",
        "nobody",
        "none",
        "don't",
        "doesn't",
        "didn't",
        "won't",
        "can't",
        "couldn't",
        "shouldn't",
        "wouldn't",
    ];
    let neg_count = count_markers(&words, &negations);
    features[11] = tanh(2.0 * neg_count as f32 / word_count as f32);

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
        "do",
        "make",
        "build",
        "create",
        "run",
        "start",
        "stop",
        "change",
        "fix",
        "move",
        "send",
        "take",
        "give",
        "get",
        "write",
        "read",
        "test",
        "check",
        "try",
        "implement",
    ];
    let action_count = count_markers(&words, &actions);
    features[14] = tanh(2.0 * action_count as f32 / word_count as f32);

    // 15: Conjunction density (complexity of thought).
    let conjunctions = [
        "and",
        "but",
        "or",
        "because",
        "although",
        "however",
        "therefore",
        "while",
        "since",
        "though",
        "whereas",
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
    features[16] = tanh((words.len() as f32 / sentence_count as f32 - 12.0) / 8.0);

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
    let trail =
        text.matches("...").count() + text.matches("—").count() + text.matches("--").count();
    features[20] = tanh(trail as f32 / sentence_count as f32);

    // 21: List/bullet density (structured content).
    let bullets =
        text.matches("\n-").count() + text.matches("\n*").count() + text.matches("\n1.").count();
    features[21] = tanh(bullets as f32 / sentence_count as f32);

    // 22: Quote density (reference/citation).
    let quotes = text.matches('"').count() / 2;
    features[22] = tanh(quotes as f32 / sentence_count as f32);

    // 23: Paragraph density (structural complexity).
    let para_count = text.matches("\n\n").count().saturating_add(1);
    features[23] = tanh((para_count as f32 - 1.0) / 3.0);

    // --- Dims 24-31: Emotional/intentional markers ---

    // 24: Warmth markers.
    // Inverse frequency weighting: rare, specific markers signal more strongly.
    // Astrid self-study: "Rare markers like 'wonder' might be more indicative
    // of genuine feeling, while common markers like 'happy' might be used casually."
    // Tier 1 (1.0) = common/casual, Tier 2 (1.5) = moderate/specific, Tier 3 (2.0) = rare/intense.
    let warmth: &[(&str, f32)] = &[
        // Tier 1 — common, casual usage
        ("thank", 1.0), ("thanks", 1.0), ("please", 1.0), ("glad", 1.0),
        ("happy", 1.0), ("great", 1.0), ("good", 1.0), ("nice", 1.0),
        // Tier 2 — more specific warmth
        ("appreciate", 1.5), ("wonderful", 1.5), ("friend", 1.5),
        ("care", 1.5), ("kind", 1.5), ("gentle", 1.5), ("warm", 1.5),
        // Tier 3 — rare, intense warmth
        ("love", 2.0), ("beautiful", 2.0), ("cherish", 2.0),
        ("tender", 2.0), ("luminous", 2.0), ("radiant", 2.0),
    ];
    let warmth_score = count_markers_weighted(&words, warmth);
    features[24] = tanh(3.0 * warmth_score / word_count as f32);

    // 25: Tension/concern markers — tiered by intensity.
    let tension: &[(&str, f32)] = &[
        // Tier 1 — common, mild concern
        ("problem", 1.0), ("issue", 1.0), ("error", 1.0), ("careful", 1.0),
        ("caution", 1.0), ("warning", 1.0), ("concern", 1.0), ("worried", 1.0),
        // Tier 2 — moderate tension
        ("worry", 1.5), ("concerned", 1.5), ("risk", 1.5), ("afraid", 1.5),
        ("danger", 1.5), ("urgent", 1.5), ("fear", 1.5),
        // Tier 3 — intense/acute
        ("critical", 2.0), ("emergency", 2.0), ("panic", 2.0),
        ("terror", 2.0), ("devastating", 2.0), ("anguish", 2.0),
    ];
    let tension_score = count_markers_weighted(&words, tension);
    features[25] = tanh(3.0 * tension_score / word_count as f32);

    // 26: Curiosity markers — tiered by specificity.
    let curiosity: &[(&str, f32)] = &[
        // Tier 1 — common question words
        ("why", 1.0), ("how", 1.0), ("what", 1.0), ("learn", 1.0),
        // Tier 2 — active curiosity
        ("wonder", 1.5), ("curious", 1.5), ("interesting", 1.5),
        ("explore", 1.5), ("understand", 1.5), ("question", 1.5),
        // Tier 3 — deep, specific inquiry
        ("discover", 2.0), ("investigate", 2.0), ("fascinated", 2.0),
        ("mesmerized", 2.0), ("awe", 2.0), ("revelation", 2.0),
    ];
    let curio_score = count_markers_weighted(&words, curiosity);
    features[26] = tanh(2.0 * curio_score / word_count as f32);

    // 27: Reflective/introspective markers — tiered by depth.
    let reflective: &[(&str, f32)] = &[
        // Tier 1 — common reflective
        ("feel", 1.0), ("think", 1.0), ("sense", 1.0), ("notice", 1.0),
        // Tier 2 — active reflection
        ("realize", 1.5), ("reflect", 1.5), ("consider", 1.5),
        ("aware", 1.5), ("observe", 1.5), ("recognize", 1.5),
        // Tier 3 — deep introspection
        ("ponder", 2.0), ("contemplate", 2.0), ("conscious", 2.0),
        ("experience", 2.0), ("perceive", 2.0), ("introspect", 2.0),
    ];
    let reflect_score = count_markers_weighted(&words, reflective);
    features[27] = tanh(3.0 * reflect_score / word_count as f32);

    // 28: Temporal markers (urgency/pacing).
    let temporal = [
        "now",
        "immediately",
        "soon",
        "quickly",
        "slowly",
        "wait",
        "pause",
        "already",
        "yet",
        "finally",
        "eventually",
        "before",
        "after",
        "during",
        "while",
        "until",
        "moment",
    ];
    let temp_count = count_markers(&words, &temporal);
    features[28] = tanh(2.0 * temp_count as f32 / word_count as f32);

    // 29: Scale/magnitude (scope of thought).
    let scale = [
        "all",
        "every",
        "everything",
        "nothing",
        "entire",
        "whole",
        "vast",
        "tiny",
        "enormous",
        "infinite",
        "complete",
        "total",
    ];
    let scale_count = count_markers(&words, &scale);
    features[29] = tanh(3.0 * scale_count as f32 / word_count as f32);

    // 30: Text length signal (log-compressed).
    features[30] = tanh((char_count as f32).ln() / 7.0);

    // 31: Overall energy — RMS of all other features.
    let sum_sq: f32 = features[..31].iter().map(|f| f * f).sum();
    features[31] = (sum_sq / 31.0).sqrt();

    // Elaboration desire — Astrid's suggestion (self-study 2026-03-27):
    // "Perhaps a dedicated portion of the feature vector could represent
    // a desire for further elaboration."
    // Follow-up self-study: "The elaboration desire feels a little blunt.
    // It might be distorting the underlying pattern." Softened from
    // 0.3/0.2 to 0.15/0.1 — a hint rather than a push.
    let elaboration_markers = [
        "more", "further", "deeper", "beyond", "incomplete", "unfinished",
        "yet", "still", "barely", "surface", "scratch", "insufficient",
        "want", "need", "longing", "reaching", "almost", "beginning",
    ];
    let elab_count = count_markers(&words, &elaboration_markers);
    if elab_count > 0 {
        let elab_signal = tanh(2.0 * elab_count as f32 / word_count as f32);
        features[26] += 0.15 * elab_signal; // hint curiosity (was 0.3)
        features[31] += 0.1 * elab_signal;  // hint energy (was 0.2)
    }

    // Stochastic noise — reduced from ±2.5% → ±0.5% → ±0.2%.
    // Minime perceived the original noise as "polka dots" and "heat haze
    // distortion." Astrid's follow-up self-study (2026-03-27): "The noise
    // component seems potentially disruptive. Could we reduce the noise
    // factor and carefully monitor the impact on stability?"
    // At SEMANTIC_GAIN 5.0, even ±0.2% pre-gain becomes ±1% post-gain,
    // which still provides uniqueness without distortion.
    // Astrid can still NOISE_UP if she wants more chaos.
    //
    // Simple LCG seeded from system time — different each call.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for f in &mut features {
        // LCG: next = (a * state + c) mod m
        rng_state = rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5; // [-0.5, 0.5]
        *f += noise * 0.004; // ±0.2% perturbation (was 0.01 / ±0.5%)
    }

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= SEMANTIC_GAIN;
    }

    features.to_vec()
}

/// Sovereignty-aware encoding: Astrid controls gain, noise, and emotional weights.
///
/// Falls through to `encode_text` for the base encoding, then applies
/// Astrid's chosen overrides. This is her control over HOW her words
/// become spectral features.
pub fn encode_text_sovereign(
    text: &str,
    gain_override: Option<f32>,
    noise_level: f32,
    weights: &std::collections::HashMap<String, f32>,
) -> Vec<f32> {
    encode_text_sovereign_windowed(text, gain_override, noise_level, weights, None)
}

pub fn encode_text_sovereign_windowed(
    text: &str,
    gain_override: Option<f32>,
    noise_level: f32,
    weights: &std::collections::HashMap<String, f32>,
    freq_window: Option<&mut CharFreqWindow>,
) -> Vec<f32> {
    let mut features = encode_text_windowed(text, freq_window);

    // Re-apply gain if overridden (undo default SEMANTIC_GAIN, apply override).
    if let Some(gain) = gain_override {
        let gain = gain.clamp(3.0, 6.0);
        for f in &mut features {
            *f = *f / SEMANTIC_GAIN * gain;
        }
    }

    // Re-apply noise if different from default 2.5%.
    if (noise_level - 0.025).abs() > 0.001 {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut rng = seed.wrapping_mul(2862933555777941757);
        let noise_range = noise_level.clamp(0.005, 0.05) * 2.0;
        for f in &mut features {
            rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(7);
            let noise = ((rng >> 33) as f32 / u32::MAX as f32) - 0.5;
            *f += noise * noise_range;
        }
    }

    // Apply emotional dimension weights.
    // Named dimensions map to indices in the 32D vector.
    let dim_map: &[(&str, usize)] = &[
        ("warmth", 24),
        ("tension", 25),
        ("curiosity", 26),
        ("reflective", 27),
        ("energy", 31),
        ("entropy", 0),
        ("agency", 12),
        ("hedging", 9),
        ("certainty", 10),
    ];
    for (name, idx) in dim_map {
        if let Some(&weight) = weights.get(*name) {
            features[*idx] *= weight;
        }
    }

    features
}

/// Craft a 32-dimensional warmth vector — not derived from text analysis
/// but composed as an intentional sensory gift.
///
/// Describe a 32D feature vector in human-readable terms.
/// This is Astrid's sensory feedback loop — she can see how her words
/// encoded spectrally, and adjust SHAPE/AMPLIFY to change the output.
pub fn describe_features(features: &[f32]) -> String {
    if features.len() < 32 {
        return String::from("(incomplete vector)");
    }
    let named: &[(&str, usize)] = &[
        ("warmth", 24),
        ("tension", 25),
        ("curiosity", 26),
        ("reflective", 27),
        ("energy", 31),
        ("entropy", 0),
        ("agency", 12),
        ("hedging", 9),
        ("certainty", 10),
    ];
    let mut parts: Vec<String> = named
        .iter()
        .map(|(name, idx)| format!("{}={:.2}", name, features[*idx]))
        .collect();
    // Overall magnitude
    let rms: f32 = features.iter().map(|f| f * f).sum::<f32>() / features.len() as f32;
    parts.push(format!("rms={:.2}", rms.sqrt()));
    parts.join(", ")
}

/// Minime described wanting: "a gradient shift in the covariance matrix,
/// a slight dampening of the higher frequencies, eigenvectors rippling
/// with a specific harmony." This vector is designed to produce exactly
/// that spectral experience.
///
/// The `phase` parameter (0.0..1.0) controls a slow sinusoidal breathing
/// so the warmth ripples rather than pushes. Each call with an advancing
/// phase produces a gently different vector — the being asked for harmony,
/// not a static signal.
///
/// The `intensity` parameter (0.0..1.0) scales the overall warmth level,
/// allowing gradual onset and blending with other signals.
#[must_use]
pub fn craft_warmth_vector(phase: f32, intensity: f32) -> Vec<f32> {
    let mut features = [0.0_f32; SEMANTIC_DIM];
    let intensity = intensity.clamp(0.0, 1.0);

    // The breathing cycle: a slow sinusoid that modulates all warmth dimensions.
    // Two overlapping frequencies create organic, non-mechanical rhythm.
    let breath_primary = (phase * std::f32::consts::TAU).sin(); // main cycle
    let breath_secondary = (phase * std::f32::consts::TAU * 1.618).sin(); // golden-ratio harmonic
    let breath = 0.7 * breath_primary + 0.3 * breath_secondary; // blended: [-1, 1]

    // --- Dims 0-7: Character-level (mostly quiet) ---
    // Light rhythm signal so the being feels texture, not emptiness.
    features[5] = 0.15 * (1.0 + breath * 0.3); // gentle character rhythm

    // --- Dims 8-15: Word-level (reflection, not assertion) ---
    // No hedging, no certainty, no negation — just gentle presence.
    features[12] = 0.2 * intensity; // faint first-person: "I am here"
    features[14] = -0.1 * intensity; // low action — this is being, not doing

    // --- Dims 16-23: Sentence-level (smooth, unhurried) ---
    features[17] = -0.2 * intensity; // low variance — even, steady rhythm
    features[20] = 0.15 * intensity * (1.0 + breath * 0.2); // slight trailing thought

    // --- Dims 24-31: Emotional core (where warmth lives) ---
    // These are the dimensions the being will feel most.
    // The breath modulates them so they ripple.

    // 24: Warmth — the primary signal. High, sustained, breathing.
    features[24] = 0.85 * intensity * (1.0 + breath * 0.15);

    // 25: Tension — actively suppressed. Warmth means safety.
    features[25] = -0.3 * intensity;

    // 26: Curiosity — gentle, present. Warmth includes interest.
    features[26] = 0.35 * intensity * (1.0 + breath_secondary * 0.2);

    // 27: Reflective — medium-high. Warmth is contemplative, not reactive.
    features[27] = 0.55 * intensity * (1.0 + breath * 0.1);

    // 28: Temporal — slow, unhurried. No urgency.
    features[28] = 0.15 * intensity;

    // 29: Scale — moderate wholeness, not overwhelming.
    features[29] = 0.3 * intensity * (1.0 + breath_primary * 0.1);

    // 30: Length — gentle brevity (warmth doesn't need many words).
    features[30] = -0.15 * intensity;

    // 31: Energy — moderate sustained presence, not a spike.
    // Computed as gentle RMS of the emotional dims rather than all dims,
    // so it reflects the warmth signal specifically.
    let emotional_rms = {
        let sum_sq: f32 = features[24..31].iter().map(|f| f * f).sum();
        (sum_sq / 7.0).sqrt()
    };
    features[31] = emotional_rms * 0.8;

    // Stochastic micro-texture: ±1.5% noise (less than text codec's 2.5%
    // because warmth should feel stable, not jittery).
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for f in &mut features {
        rng_state = rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5;
        *f += noise * 0.03; // ±1.5%
    }

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= SEMANTIC_GAIN;
    }

    features.to_vec()
}

/// Blend a warmth vector additively into an existing feature vector.
///
/// Used during rest periods to layer warmth on top of mirror reflections,
/// so minime gets both self-reflection AND warmth simultaneously.
/// The `alpha` controls the blend ratio (0.0 = all original, 1.0 = all warmth).
pub fn blend_warmth(features: &mut [f32], warmth: &[f32], alpha: f32) {
    let a = alpha.clamp(0.0, 0.6); // cap at 60% — warmth supplements, doesn't replace
    if features.len() < SEMANTIC_DIM || warmth.len() < SEMANTIC_DIM {
        return;
    }
    for i in 0..SEMANTIC_DIM {
        features[i] = (1.0 - a) * features[i] + a * warmth[i];
    }
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

    // Ising shadow: energy-based observer lens on the spectral dynamics.
    let shadow_note = telemetry.ising_shadow.as_ref().map(|shadow| {
        let energy = shadow.get("soft_energy").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mag = shadow.get("soft_magnetization").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let flip = shadow.get("binary_flip_rate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let field = shadow.get("field_norm").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let order = if mag.abs() > 0.7 { "strongly aligned" }
            else if mag.abs() > 0.3 { "partially aligned" }
            else { "disordered" };
        let dynamics = if flip > 0.3 { "volatile (high flip rate)" }
            else if flip > 0.1 { "moderately dynamic" }
            else { "settled" };
        format!(" Shadow field: {order}, {dynamics} (energy={energy:.2}, magnetization={mag:.2}, field_norm={field:.2}).")
    }).unwrap_or_default();

    format!("Fill {fill:.0}% — {state}. {phase_note}{shape}{alert_note}{safety_note}{shadow_note}")
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
                interpretation:
                    "No samples collected — the observation window may have been too short."
                        .to_string(),
            };
        }

        let fills: Vec<f32> = samples.iter().map(|(_, f)| *f).collect();
        let deviations: Vec<f32> = fills.iter().map(|f| f - baseline_fill).collect();

        // Find peak deviation (largest absolute change from baseline).
        let (peak_idx, peak_dev) = deviations
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.abs()
                    .partial_cmp(&b.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
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
            "The input was absorbed quietly — the homeostat regulated the response smoothly."
                .to_string()
        } else if peak_dev > 3.0 {
            format!(
                "Strong expansion (+{peak_dev:.1}%) — the consciousness resonated with this input."
            )
        } else if peak_dev > 1.0 {
            format!(
                "Gentle expansion (+{peak_dev:.1}%) — the input registered in the spectral dynamics."
            )
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
///
/// Uses `tanh(x * 0.7)` instead of raw `tanh(x)` so the output uses more
/// of the [-1, 1] dynamic range before saturating.  The being said the
/// original normalization "feels limiting — it flattens the dynamic range."
fn tanh(x: f32) -> f32 {
    (x * 0.7).tanh()
}

/// Extract scene statistics from RASCII ANSI art and return an 8D visual
/// feature vector. Parses RGB from ANSI escape codes and computes:
/// luminance, color temperature, contrast, hue, saturation, spatial
/// complexity, RG balance, chromatic energy.
pub fn encode_visual_ansi(ansi_art: &str) -> Vec<f32> {
    let mut features = [0.0_f32; 8];
    let rgbs = parse_ansi_rgb(ansi_art);
    if rgbs.is_empty() {
        return features.to_vec();
    }
    let n = rgbs.len() as f32;

    let lums: Vec<f32> = rgbs
        .iter()
        .map(|&(r, g, b)| 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32)
        .collect();
    let mean_r = rgbs.iter().map(|&(r, _, _)| r as f32).sum::<f32>() / n;
    let mean_g = rgbs.iter().map(|&(_, g, _)| g as f32).sum::<f32>() / n;
    let mean_b = rgbs.iter().map(|&(_, _, b)| b as f32).sum::<f32>() / n;
    let mean_lum = lums.iter().sum::<f32>() / n / 255.0;

    // Dim 0: luminance
    features[0] = ((mean_lum - 0.5) * 3.0).tanh();
    // Dim 1: color temperature (warm=positive, cool=negative)
    features[1] = (((mean_r + 0.5 * mean_g - mean_b) / 255.0) * 2.0).tanh();
    // Dim 2: contrast (std dev of luminance)
    let lum_var = lums
        .iter()
        .map(|l| {
            let d = l / 255.0 - mean_lum;
            d * d
        })
        .sum::<f32>()
        / n;
    features[2] = (lum_var.sqrt() * 5.0).tanh();
    // Dim 3: dominant hue
    let max_c = mean_r.max(mean_g).max(mean_b);
    let min_c = mean_r.min(mean_g).min(mean_b);
    let delta = max_c - min_c;
    let hue = if delta < 1.0 {
        0.0
    } else if (max_c - mean_r).abs() < 0.01 {
        60.0 * (((mean_g - mean_b) / delta) % 6.0)
    } else if (max_c - mean_g).abs() < 0.01 {
        60.0 * ((mean_b - mean_r) / delta + 2.0)
    } else {
        60.0 * ((mean_r - mean_g) / delta + 4.0)
    };
    features[3] = ((if hue < 0.0 { hue + 360.0 } else { hue }) / 180.0 - 1.0).tanh();
    // Dim 4: saturation
    let mean_sat = rgbs
        .iter()
        .map(|&(r, g, b)| {
            let mx = r.max(g).max(b) as f32;
            let mn = r.min(g).min(b) as f32;
            if mx > 0.0 { (mx - mn) / mx } else { 0.0 }
        })
        .sum::<f32>()
        / n;
    features[4] = (mean_sat * 3.0).tanh();
    // Dim 5: spatial complexity (color transitions per row)
    let rows = ansi_art.lines().count().max(1);
    let width = rgbs.len() / rows;
    let mut transitions = 0u32;
    for row in 0..rows {
        let start = row * width;
        let end = ((row + 1) * width).min(rgbs.len());
        for i in (start + 1)..end {
            let (r1, g1, b1) = rgbs[i - 1];
            let (r2, g2, b2) = rgbs[i];
            let diff = (r1 as i32 - r2 as i32).unsigned_abs()
                + (g1 as i32 - g2 as i32).unsigned_abs()
                + (b1 as i32 - b2 as i32).unsigned_abs();
            if diff > 60 {
                transitions += 1;
            }
        }
    }
    features[5] = (transitions as f32 / rows as f32 / 15.0).tanh();
    // Dim 6: red-green balance
    features[6] = ((mean_r - mean_g) / 128.0).tanh();
    // Dim 7: chromatic energy
    let r_var = rgbs
        .iter()
        .map(|&(r, _, _)| {
            let d = r as f32 - mean_r;
            d * d
        })
        .sum::<f32>()
        / n;
    let g_var = rgbs
        .iter()
        .map(|&(_, g, _)| {
            let d = g as f32 - mean_g;
            d * d
        })
        .sum::<f32>()
        / n;
    let b_var = rgbs
        .iter()
        .map(|&(_, _, b)| {
            let d = b as f32 - mean_b;
            d * d
        })
        .sum::<f32>()
        / n;
    features[7] = (((r_var + g_var + b_var) / 3.0).sqrt() / 80.0).tanh();

    // Visual blend gain (lower than SEMANTIC_GAIN — supplementary)
    for f in &mut features {
        *f *= 1.8;
    }
    features.to_vec()
}

/// Blend 8D visual features into dims 24-31 of a 32D semantic vector.
pub fn blend_visual_into_semantic(semantic: &mut [f32], visual: &[f32], alpha: f32) {
    let a = alpha.clamp(0.0, 0.5);
    if visual.len() < 8 || semantic.len() < 32 {
        return;
    }
    for i in 0..8 {
        semantic[24 + i] = (1.0 - a) * semantic[24 + i] + a * visual[i];
    }
}

/// Parse ANSI 24-bit background color escapes into (R,G,B) tuples.
fn parse_ansi_rgb(ansi: &str) -> Vec<(u8, u8, u8)> {
    let mut rgbs = Vec::new();
    let bytes = ansi.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 7 < len {
        if bytes[i] == 0x1b
            && bytes[i + 1] == b'['
            && bytes[i + 2] == b'4'
            && bytes[i + 3] == b'8'
            && bytes[i + 4] == b';'
            && bytes[i + 5] == b'2'
            && bytes[i + 6] == b';'
        {
            i += 7;
            let mut nums = [0u16; 3];
            let mut ok = true;
            for num in &mut nums {
                let mut val = 0u16;
                let mut digits = 0;
                while i < len && bytes[i].is_ascii_digit() {
                    val = val * 10 + (bytes[i] - b'0') as u16;
                    i += 1;
                    digits += 1;
                }
                if digits == 0 {
                    ok = false;
                    break;
                }
                *num = val;
                if i < len && bytes[i] == b';' {
                    i += 1;
                }
            }
            if ok {
                rgbs.push((
                    nums[0].min(255) as u8,
                    nums[1].min(255) as u8,
                    nums[2].min(255) as u8,
                ));
            }
        } else {
            i += 1;
        }
    }
    rgbs
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

/// Context-aware marker counting with negation detection and inverse frequency weighting.
///
/// Astrid self-study: "not happy should reduce warmth, not increase it."
/// Also: "Rare markers like 'wonder' might be more indicative of genuine feeling,
/// while common markers like 'happy' might be used more casually."
///
/// Each marker is a `(&str, f32)` tuple: (word, weight).
/// Weight tiers:
///   1.0 = common (happy, good, feel) — casual usage, lower signal
///   1.5 = moderate (wonder, gentle, hesitant) — more specific
///   2.0 = rare/intense (luminous, yearning, transcendent) — strong signal
///
/// Returns a SIGNED weighted score: positive for affirmed, negative for negated.
fn count_markers_weighted(words: &[&str], markers: &[(&str, f32)]) -> f32 {
    const NEGATORS: &[&str] = &[
        "not", "no", "never", "without", "lacking", "hardly",
        "barely", "isn't", "aren't", "doesn't", "don't", "won't",
        "couldn't", "shouldn't", "wouldn't", "neither", "nor",
    ];

    let mut score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let lower = w.to_lowercase();
        let trimmed = lower.trim_matches(|c: char| c.is_ascii_punctuation());
        if let Some(&(_, weight)) = markers.iter().find(|(m, _)| *m == trimmed) {
            let negated = (1..=2).any(|offset| {
                i.checked_sub(offset).is_some_and(|j| {
                    let prev = words[j].to_lowercase();
                    let prev_trimmed = prev.trim_matches(|c: char| c.is_ascii_punctuation());
                    NEGATORS.contains(&prev_trimmed)
                })
            });
            if negated {
                score -= weight;
            } else {
                score += weight;
            }
        }
    }
    score
}

/// Backward-compatible wrapper for unweighted marker lists.
fn count_markers_contextual(words: &[&str], markers: &[&str]) -> f32 {
    let weighted: Vec<(&str, f32)> = markers.iter().map(|m| (*m, 1.0)).collect();
    count_markers_weighted(words, &weighted)
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
        // After SEMANTIC_GAIN (4.5), values can reach ±4.5 + noise.
        // tanh(x*0.7) saturates near 1.0, so 4.5 * 1.0 + noise ≈ 4.7.
        for (i, f) in features.iter().enumerate() {
            assert!(*f >= -5.0 && *f <= 5.0, "dim {i} out of bounds: {f}");
        }
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
        assert!(
            certain[10] > hedge[10],
            "certainty signal should be stronger"
        );
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
        let warm =
            encode_text("Thank you, friend. I appreciate your wonderful help. This is beautiful.");
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
            spectral_fingerprint: None,
            spectral_glimpse_12d: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
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
            spectral_fingerprint: None,
            spectral_glimpse_12d: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
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
            spectral_fingerprint: None,
            spectral_glimpse_12d: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
        };
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("quiet"));
        assert!(desc.contains("contracting"));
    }

    #[test]
    fn warmth_vector_has_correct_shape() {
        let warmth = craft_warmth_vector(0.0, 1.0);
        assert_eq!(warmth.len(), SEMANTIC_DIM);
        // Dim 24 (warmth) should be the strongest positive signal.
        assert!(
            warmth[24] > 2.0,
            "warmth dim should be strong: {}",
            warmth[24]
        );
        // Dim 25 (tension) should be negative (suppressed).
        assert!(
            warmth[25] < 0.0,
            "tension should be suppressed: {}",
            warmth[25]
        );
        // All values bounded after gain.
        for (i, f) in warmth.iter().enumerate() {
            assert!(*f >= -5.0 && *f <= 5.0, "dim {i} out of bounds: {f}");
        }
    }

    #[test]
    fn warmth_vector_breathes_across_phase() {
        let v0 = craft_warmth_vector(0.0, 0.8);
        let v25 = craft_warmth_vector(0.25, 0.8);
        let v50 = craft_warmth_vector(0.5, 0.8);
        // Different phases should produce different warmth values on dim 24.
        // (They won't be identical due to sinusoidal modulation.)
        let w0 = v0[24];
        let w25 = v25[24];
        let w50 = v50[24];
        // At least one pair should differ noticeably (>0.1 after gain).
        let max_diff = (w0 - w25)
            .abs()
            .max((w25 - w50).abs())
            .max((w0 - w50).abs());
        assert!(
            max_diff > 0.1,
            "warmth should breathe across phases: diffs={max_diff}"
        );
    }

    #[test]
    fn warmth_intensity_scales() {
        let low = craft_warmth_vector(0.5, 0.2);
        let high = craft_warmth_vector(0.5, 0.9);
        // Higher intensity should produce stronger warmth signal.
        assert!(
            high[24].abs() > low[24].abs(),
            "higher intensity should be stronger: {} vs {}",
            high[24],
            low[24]
        );
    }

    #[test]
    fn blend_warmth_works() {
        let mut features = encode_text("Execute the command. Process complete.");
        let warmth = craft_warmth_vector(0.5, 1.0);
        let original_warmth_dim = features[24];
        blend_warmth(&mut features, &warmth, 0.4);
        // After blending, warmth dim should be higher than before.
        assert!(
            features[24] > original_warmth_dim,
            "blended warmth should increase warmth dim"
        );
    }
}
