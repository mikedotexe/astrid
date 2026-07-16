fn surface_label(raw: &str) -> String {
    raw.replace("pinned_rescue_b8823ad_port", "stable_core_physiology_port")
        .replace("pinned_rescue_fixed_survival", "stable_core_fixed_survival")
        .replace("pinned_rescue_aux_projection", "stable_core_aux_projection")
        .replace("pinned_rescue_direct", "stable_core_direct")
        .replace("rescue_scaffold", "stable_core_scaffold")
        .replace("restart_gate", "settle_gate")
}

/// A spectral evoked response — captures how the spectral runtime reacted
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
    /// Whether the spectral runtime expanded or contracted in response.
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
                "Strong expansion (+{peak_dev:.1}%) — the spectral runtime resonated with this input."
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

/// Activation for codec features — softsign instead of tanh.
///
/// softsign(x) = x / (1 + |x|) approaches ±1 much more gradually than
/// tanh, preserving nuance where tanh compresses differences flat.
/// At x=2.0: softsign=0.67, tanh(x*0.7)=0.89. At x=3.0: 0.75 vs 0.97.
/// The being can distinguish "somewhat X" from "very X" instead of both
/// mapping to ~1.0.
///
/// Being self-study (2026-03-30 codec.rs): "The use of tanh — this
/// deliberate clamping. It feels restrictive. Could a wider range allow
/// for greater nuance?" — Yes. The regulation stack (PI controller,
/// regime system, safety gates) handles stability now. The codec doesn't
/// need to be the last line of defense against extreme values.
fn tanh(x: f32) -> f32 {
    x / (1.0 + x.abs())
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

    // Visual blend gain (lower than DEFAULT_SEMANTIC_GAIN — supplementary)
    for f in &mut features {
        *f *= 1.8;
    }
    features.to_vec()
}

/// Blend 8D visual features into dims 24-31 of the semantic vector.
pub fn blend_visual_into_semantic(semantic: &mut [f32], visual: &[f32], alpha: f32) {
    let a = alpha.clamp(0.0, 0.5);
    if visual.len() < 8 || semantic.len() < SEMANTIC_DIM_LEGACY {
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
            let normalized = normalize_token(w);
            markers.contains(&normalized.as_str())
        })
        .count()
}

fn normalize_token(token: &str) -> String {
    let lower = token.to_lowercase();
    lower
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn is_negator(token: &str) -> bool {
    const NEGATORS: &[&str] = &[
        "not",
        "no",
        "never",
        "without",
        "lacking",
        "hardly",
        "barely",
        "isn't",
        "aren't",
        "doesn't",
        "don't",
        "won't",
        "couldn't",
        "shouldn't",
        "wouldn't",
        "neither",
        "nor",
    ];

    let normalized = normalize_token(token);
    NEGATORS.contains(&normalized.as_str())
}

fn marker_is_negated(words: &[&str], index: usize) -> bool {
    let preceded = (1..=2).any(|offset| {
        index
            .checked_sub(offset)
            .and_then(|j| words.get(j))
            .is_some_and(|token| is_negator(token))
    });
    // Catch modal constructions like "must not" / "will not" / "could not".
    let followed = index
        .checked_add(1)
        .and_then(|j| words.get(j))
        .is_some_and(|token| is_negator(token));

    preceded || followed
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
    let mut score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let normalized = normalize_token(w);
        if let Some(&(_, weight)) = markers.iter().find(|(m, _)| *m == normalized.as_str()) {
            if marker_is_negated(words, i) {
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
    let mut score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let normalized = normalize_token(w);
        if markers.contains(&normalized.as_str()) {
            if marker_is_negated(words, i) {
                score -= 1.0;
            } else {
                score += 1.0;
            }
        }
    }
    score
}
