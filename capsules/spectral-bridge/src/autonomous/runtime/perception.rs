/// Primary fresh-window scan for assembling the latest cross-modal perception.
/// Widened from 30 (Astrid self_study_1780922594, 2026-06-07) so a recent burst
/// of one modality is less likely to bury the freshest quieter lane.
///
/// Ordering stays mtime-recency-primary because a sensory gateway should
/// privilege immediacy. If this primary window does not contain every requested
/// modality, the rare-modality fallback below extends the scan with a hard cap.
const PERCEPTION_SCAN_WINDOW: usize = 80;
/// Additional files to inspect only when the newest perception window did not
/// contain every requested modality. This keeps the usual fresh-lane path cheap
/// while preventing a single noisy modality from hiding the freshest quiet lane.
const PERCEPTION_RARE_MODALITY_FALLBACK_WINDOW: usize = 512;

fn requested_perception_seen(
    include_visual: bool,
    include_spatial: bool,
    include_audio: bool,
    seen_vision: bool,
    seen_ascii: bool,
    seen_audio: bool,
) -> bool {
    (!include_visual || seen_vision)
        && (!include_visual || !include_spatial || seen_ascii)
        && (!include_audio || seen_audio)
}

/// Read Astrid's most recent perception (visual or audio) from the
/// perception capsule's output directory.
///
/// `include_spatial`: if true, include ANSI art from RASCII (only when
/// Astrid chooses NEXT: LOOK). Default perception is LLaVA prose + audio.
fn read_latest_perception(
    perception_dir: &Path,
    include_visual: bool,
    include_spatial: bool,
    include_audio: bool,
    fill_pct: f32,
    last_visual_features: Option<&[f32]>,
) -> Option<String> {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(perception_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    // Read the most recent perception of each type.
    let mut parts = Vec::new();
    let mut seen_vision = false;
    let mut seen_ascii = false;
    let mut seen_audio = false;

    let scan_limit = entries
        .len()
        .min(PERCEPTION_SCAN_WINDOW.saturating_add(PERCEPTION_RARE_MODALITY_FALLBACK_WINDOW));
    for (idx, (path, _)) in entries.iter().take(scan_limit).enumerate() {
        if idx >= PERCEPTION_SCAN_WINDOW
            && requested_perception_seen(
                include_visual,
                include_spatial,
                include_audio,
                seen_vision,
                seen_ascii,
                seen_audio,
            )
        {
            break;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let ptype = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if ptype == "visual" && include_visual && !seen_vision {
            if let Some(desc) = json.get("description").and_then(|d| d.as_str()) {
                let visual_features = parse_visual_feature_vector(&json);
                let resonance = perception_resonance_annotation(
                    PerceptionType::Visual,
                    fill_pct,
                    visual_features
                        .as_deref()
                        .map(|features| PerceptionStructured::Visual {
                            features,
                            previous: last_visual_features,
                        }),
                    Some(desc),
                );
                if resonance.is_empty() {
                    parts.push(format!("[VISION] {desc}"));
                } else {
                    parts.push(format!("[VISION] {desc} {resonance}"));
                }
                seen_vision = true;
            }
        } else if ptype == "visual_ascii" && include_visual && !seen_ascii && include_spatial {
            // RASCII colored ANSI art — only when Astrid chose NEXT: LOOK.
            if let Some(art) = json.get("ascii_art").and_then(|a| a.as_str()) {
                let source = json
                    .get("source")
                    .and_then(|s| s.as_str())
                    .unwrap_or("camera");
                let label = if source == "host" {
                    "colored ANSI art of the host machine's internal state"
                } else {
                    "colored ANSI art of the room"
                };
                let trimmed: String = art.chars().take(8000).collect();
                parts.push(format!(
                    "[SPATIAL VISION — {label}. You asked to LOOK.]\n{trimmed}"
                ));
                seen_ascii = true;
            }
        } else if ptype == "audio"
            && !seen_audio
            && include_audio
            && let Some(transcript) = json.get("transcript").and_then(|t| t.as_str())
        {
            let audio_features = parse_audio_perception_features(&json);
            let resonance = perception_resonance_annotation(
                PerceptionType::Audio,
                fill_pct,
                audio_features.as_ref().map(PerceptionStructured::Audio),
                Some(transcript),
            );
            if resonance.is_empty() {
                parts.push(format!("[HEARING] {transcript}"));
            } else {
                parts.push(format!("[HEARING] {transcript} {resonance}"));
            }
            seen_audio = true;
        }

        if requested_perception_seen(
            include_visual,
            include_spatial,
            include_audio,
            seen_vision,
            seen_ascii,
            seen_audio,
        ) {
            break;
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

const VISUAL_FEATURE_KEYS: [&str; 8] = [
    "luminance",
    "temperature",
    "contrast",
    "hue",
    "saturation",
    "complexity",
    "red_green_balance",
    "chromatic_energy",
];

const VISUAL_FEATURE_ALIASES: [(&str, &[&str]); 8] = [
    (
        "luminance",
        &["luminance", "brightness", "lightness", "value"],
    ),
    (
        "temperature",
        &["temperature", "warmth", "color_temperature"],
    ),
    (
        "contrast",
        &["contrast", "scene_contrast", "luminance_contrast"],
    ),
    ("hue", &["hue", "hue_angle", "dominant_hue"]),
    ("saturation", &["saturation", "colorfulness", "sat"]),
    (
        "complexity",
        &["complexity", "detail_density", "texture_complexity"],
    ),
    (
        "red_green_balance",
        &["red_green_balance", "rg_balance", "red_green_bias"],
    ),
    (
        "chromatic_energy",
        &["chromatic_energy", "color_energy", "chromatic_intensity"],
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerceptionType {
    Visual,
    Audio,
}

#[derive(Clone, Copy, Debug)]
struct AudioPerceptionFeatures {
    rms_energy: f32,
    zero_crossing_rate: f32,
    dynamic_range: f32,
    temporal_variation: f32,
    is_music_likely: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResonanceFamily {
    Resonant,
    Counterpoint,
    Contrast,
    Opening,
}

#[derive(Clone, Copy, Debug)]
struct FillResonanceBlend {
    high: f32,
    middle: f32,
    low: f32,
}

enum PerceptionStructured<'a> {
    Visual {
        features: &'a [f32],
        previous: Option<&'a [f32]>,
    },
    Audio(&'a AudioPerceptionFeatures),
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn normalize_visual_dim(value: f32) -> f32 {
    (value / 1.8).clamp(-1.0, 1.0)
}

fn resonance_family_annotation(family: ResonanceFamily) -> &'static str {
    match family {
        ResonanceFamily::Resonant => "(resonant with your current state)",
        ResonanceFamily::Counterpoint => "(counterpoint to your current state)",
        ResonanceFamily::Contrast => "(offers useful contrast beyond your current dominant state)",
        ResonanceFamily::Opening => "(offers an opening/widening angle beyond your current state)",
    }
}

/// Floor below which a perception carries no meaningful resonance and is left
/// un-annotated (raw description only). This gate has always existed; the
/// strength bands above it are new.
const RESONANCE_GATE: f32 = 0.45;
/// Suprathreshold strength at or above which resonance reads as "strongly".
const RESONANCE_STRONG: f32 = 0.80;
/// Suprathreshold strength at or above which resonance reads as "clearly".
const RESONANCE_CLEAR: f32 = 0.62;

/// Graduated annotation (Astrid self_study_1780922594, 2026-06-07): she asked
/// for a `resonance_strength` threshold so the raw->annotated transition does
/// not "feel jarring or inconsistent ... the 'weight' of the resonance might
/// not be consistently quantified." The hard gate already lived in
/// `select_resonance_family_scored`; this turns the suprathreshold range into a
/// graduated qualifier (faintly / clearly / strongly) so a resonance just over
/// the gate no longer reads identically to a strong one — smoothing the flicker.
/// The family keyword is preserved verbatim inside the parenthetical.
fn resonance_family_annotation_weighted(family: ResonanceFamily, strength: f32) -> String {
    let qualifier = if strength >= RESONANCE_STRONG {
        "strongly "
    } else if strength >= RESONANCE_CLEAR {
        "clearly "
    } else {
        "faintly "
    };
    // Splice the qualifier just inside the leading '(' of the base phrase so the
    // family keyword (asserted by tests and read by Astrid) stays intact.
    let base = resonance_family_annotation(family);
    base.strip_prefix('(')
        .map_or_else(|| base.to_string(), |rest| format!("({qualifier}{rest}"))
}

/// Select the highest-scoring resonance family above `RESONANCE_GATE`, returning
/// the winning score so callers can render a strength-graduated annotation.
/// Single source of truth for the gate floor.
fn select_resonance_family_scored(
    scores: &[(ResonanceFamily, f32)],
) -> Option<(ResonanceFamily, f32)> {
    scores
        .iter()
        .filter(|(_, score)| *score >= RESONANCE_GATE)
        .max_by(|(_, left), (_, right)| {
            left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

fn resonance_fill_blend(fill_pct: f32) -> FillResonanceBlend {
    let high = clamp01((fill_pct - 58.0) / 14.0);
    let low = clamp01((42.0 - fill_pct) / 14.0);
    let middle = clamp01(1.0 - high.max(low));
    let total = high + middle + low;
    if total <= 0.0001 {
        return FillResonanceBlend {
            high: 0.0,
            middle: 1.0,
            low: 0.0,
        };
    }
    FillResonanceBlend {
        high: high / total,
        middle: middle / total,
        low: low / total,
    }
}

fn blend_resonance_scores(blend: FillResonanceBlend, high: f32, middle: f32, low: f32) -> f32 {
    clamp01((high * blend.high) + (middle * blend.middle) + (low * blend.low))
}

fn score_visual_resonance(
    fill_pct: f32,
    features: &[f32],
    previous: Option<&[f32]>,
) -> Option<(ResonanceFamily, f32)> {
    if features.len() < VISUAL_FEATURE_KEYS.len() {
        return None;
    }
    let luminance = normalize_visual_dim(features[0]);
    let contrast = normalize_visual_dim(features[2]).abs();
    let saturation = normalize_visual_dim(features[4]).abs();
    let complexity = normalize_visual_dim(features[5]).abs();
    let chromatic_energy = normalize_visual_dim(features[7]).abs();
    let warm_bias = normalize_visual_dim(features[1]).max(0.0);

    let calming = clamp01(
        ((1.0 - contrast) + (1.0 - complexity) + (1.0 - chromatic_energy) + (1.0 - saturation))
            / 4.0,
    );
    let energizing = clamp01(
        (contrast * 0.28)
            + (complexity * 0.24)
            + (chromatic_energy * 0.24)
            + (saturation * 0.14)
            + (luminance.max(0.0) * 0.05)
            + (warm_bias * 0.05),
    );
    let novelty = clamp01(
        (contrast * 0.34) + (complexity * 0.28) + (saturation * 0.16) + (chromatic_energy * 0.22),
    );
    let change = previous
        .filter(|prev| prev.len() >= VISUAL_FEATURE_KEYS.len())
        .map(|prev| {
            let delta_sum = features
                .iter()
                .zip(prev.iter())
                .take(VISUAL_FEATURE_KEYS.len())
                .map(|(current, prior)| {
                    (normalize_visual_dim(*current) - normalize_visual_dim(*prior)).abs()
                })
                .sum::<f32>();
            clamp01(delta_sum / VISUAL_FEATURE_KEYS.len() as f32)
        })
        .unwrap_or(0.0);
    let widening = clamp01((novelty * 0.55) + (change * 0.45));
    let fill_blend = resonance_fill_blend(fill_pct);

    let scores = [
        (
            ResonanceFamily::Counterpoint,
            blend_resonance_scores(
                fill_blend,
                clamp01((calming * 0.72) + ((1.0 - energizing) * 0.18) + ((1.0 - change) * 0.10)),
                clamp01((calming * 0.52) + ((1.0 - energizing) * 0.28) + (change * 0.20)),
                clamp01((calming * 0.68) + ((1.0 - change) * 0.18) + ((1.0 - energizing) * 0.14)),
            ),
        ),
        (
            ResonanceFamily::Opening,
            blend_resonance_scores(
                fill_blend,
                clamp01((widening * 0.55) + (change * 0.25) + (novelty * 0.20)),
                clamp01((widening * 0.52) + (change * 0.28) + (energizing * 0.20)),
                clamp01((widening * 0.46) + (energizing * 0.34) + (change * 0.20)),
            ),
        ),
        (
            ResonanceFamily::Contrast,
            blend_resonance_scores(
                fill_blend,
                clamp01((novelty * 0.58) + (change * 0.42)),
                clamp01((novelty * 0.48) + (change * 0.32) + (complexity * 0.20)),
                clamp01((novelty * 0.50) + (change * 0.30) + (calming * 0.20)),
            ),
        ),
        (
            ResonanceFamily::Resonant,
            blend_resonance_scores(
                fill_blend,
                clamp01((calming * 0.45) + (change * 0.30) + ((1.0 - novelty) * 0.25)),
                clamp01((energizing * 0.34) + (calming * 0.32) + ((1.0 - novelty) * 0.34)),
                clamp01((energizing * 0.60) + (novelty * 0.20) + (change * 0.20)),
            ),
        ),
    ];
    select_resonance_family_scored(&scores)
}

fn score_audio_resonance(
    fill_pct: f32,
    features: &AudioPerceptionFeatures,
) -> Option<(ResonanceFamily, f32)> {
    let energy = clamp01(features.rms_energy * 4.0);
    let activity = clamp01(features.temporal_variation * 12.0);
    let texture = clamp01(features.zero_crossing_rate * 8.0);
    let contrast = clamp01((features.dynamic_range - 1.0) / 6.0);
    let musicality = if features.is_music_likely { 1.0 } else { 0.0 };
    let calming = clamp01(1.0 - ((energy * 0.55) + (activity * 0.30) + (texture * 0.15)));
    let energizing = clamp01(
        (energy * 0.42)
            + (activity * 0.22)
            + (contrast * 0.16)
            + (texture * 0.08)
            + (musicality * 0.12),
    );
    let novelty = clamp01((contrast * 0.36) + (activity * 0.34) + (musicality * 0.30));
    let fill_blend = resonance_fill_blend(fill_pct);

    if fill_blend.high >= 0.8 && calming > 0.75 && energizing < 0.20 {
        // Clear-cut special case (high fill, strongly calming, low energizing):
        // a confident counterpoint. Report it with the calming magnitude so the
        // graduated annotation reads as a strong resonance.
        return Some((ResonanceFamily::Counterpoint, calming));
    }

    let scores = [
        (
            ResonanceFamily::Counterpoint,
            blend_resonance_scores(
                fill_blend,
                clamp01((calming * 0.74) + ((1.0 - energizing) * 0.18) + ((1.0 - novelty) * 0.08)),
                clamp01((calming * 0.48) + ((1.0 - energizing) * 0.28) + ((1.0 - activity) * 0.24)),
                clamp01((calming * 0.66) + ((1.0 - novelty) * 0.18) + ((1.0 - energizing) * 0.16)),
            ),
        ),
        (
            ResonanceFamily::Opening,
            blend_resonance_scores(
                fill_blend,
                clamp01((novelty * 0.44) + (contrast * 0.32) + (musicality * 0.24)),
                clamp01((novelty * 0.44) + (energizing * 0.32) + (musicality * 0.24)),
                clamp01((energizing * 0.44) + (novelty * 0.36) + (musicality * 0.20)),
            ),
        ),
        (
            ResonanceFamily::Contrast,
            blend_resonance_scores(
                fill_blend,
                clamp01((novelty * 0.60) + (contrast * 0.40)),
                clamp01((novelty * 0.54) + (contrast * 0.24) + (calming * 0.22)),
                clamp01((novelty * 0.48) + (contrast * 0.30) + (calming * 0.22)),
            ),
        ),
        (
            ResonanceFamily::Resonant,
            blend_resonance_scores(
                fill_blend,
                clamp01((calming * 0.46) + ((1.0 - novelty) * 0.30) + ((1.0 - activity) * 0.24)),
                clamp01((energizing * 0.38) + (calming * 0.30) + ((1.0 - novelty) * 0.32)),
                clamp01((energizing * 0.62) + (contrast * 0.20) + (musicality * 0.18)),
            ),
        ),
    ];
    select_resonance_family_scored(&scores)
}

fn fallback_perception_annotation(description: &str, fill_pct: f32) -> String {
    let lower = description.to_lowercase();
    let energy_words = [
        "moving", "bright", "active", "loud", "busy", "talking", "music", "kinetic", "vivid",
    ];
    let calm_words = [
        "still", "quiet", "dark", "empty", "silent", "calm", "soft", "restful", "hushed",
    ];
    let complexity_words = [
        "complex",
        "layered",
        "detailed",
        "textured",
        "crowded",
        "patterned",
        "intricate",
        "dense",
    ];
    let novelty_words = [
        "different",
        "unusual",
        "unexpected",
        "strange",
        "surprising",
        "novel",
        "unfamiliar",
        "shift",
        "changing",
    ];

    let energy_hits = keyword_hits(&lower, &energy_words);
    let calm_hits = keyword_hits(&lower, &calm_words);
    let complexity_hits = keyword_hits(&lower, &complexity_words);
    let novelty_hits = keyword_hits(&lower, &novelty_words);

    let fill_blend = resonance_fill_blend(fill_pct);
    let scores = [
        (
            ResonanceFamily::Counterpoint,
            blend_resonance_scores(
                fill_blend,
                if calm_hits >= 2 { 0.95 } else { 0.0 },
                if calm_hits >= 2 { 0.52 } else { 0.0 },
                if calm_hits >= 2 { 0.82 } else { 0.0 },
            ),
        ),
        (
            ResonanceFamily::Contrast,
            blend_resonance_scores(
                fill_blend,
                if novelty_hits >= 2 { 0.88 } else { 0.0 },
                if novelty_hits >= 2 || (complexity_hits >= 1 && calm_hits >= 1) {
                    0.86
                } else {
                    0.0
                },
                if novelty_hits >= 2 { 0.62 } else { 0.0 },
            ),
        ),
        (
            ResonanceFamily::Opening,
            blend_resonance_scores(
                fill_blend,
                if complexity_hits >= 2 { 0.76 } else { 0.0 },
                if energy_hits >= 2 { 0.70 } else { 0.0 },
                if energy_hits >= 2 && novelty_hits >= 1 {
                    0.92
                } else {
                    0.0
                },
            ),
        ),
        (
            ResonanceFamily::Resonant,
            blend_resonance_scores(
                fill_blend,
                if energy_hits >= 2 { 0.62 } else { 0.0 },
                if calm_hits >= 2 { 0.66 } else { 0.0 },
                if energy_hits >= 2 { 0.88 } else { 0.0 },
            ),
        ),
    ];
    select_resonance_family_scored(&scores)
        .map(|(family, strength)| resonance_family_annotation_weighted(family, strength))
        .unwrap_or_default()
}

fn perception_resonance_annotation(
    _perception_type: PerceptionType,
    fill_pct: f32,
    structured: Option<PerceptionStructured<'_>>,
    fallback_text: Option<&str>,
) -> String {
    let scored = match structured {
        Some(PerceptionStructured::Visual { features, previous }) => {
            score_visual_resonance(fill_pct, features, previous)
        },
        Some(PerceptionStructured::Audio(features)) => score_audio_resonance(fill_pct, features),
        None => None,
    };
    if let Some((family, strength)) = scored {
        return resonance_family_annotation_weighted(family, strength);
    }
    fallback_text
        .map(|text| fallback_perception_annotation(text, fill_pct))
        .unwrap_or_default()
}

fn keyword_hits(description: &str, keywords: &[&str]) -> usize {
    keywords
        .iter()
        .filter(|keyword| description.contains(**keyword))
        .count()
}

fn extract_feature_f32(
    features: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<f32> {
    aliases
        .iter()
        .find_map(|alias| features.get(*alias).and_then(|value| value.as_f64()))
        .map(|value| value as f32)
}

fn normalize_feature_lookup_key(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_feature_f32_from_json(json: &serde_json::Value, aliases: &[&str]) -> Option<f32> {
    if let Some(features) = json.get("features").and_then(|value| value.as_object())
        && let Some(value) = extract_feature_f32(features, aliases)
    {
        return Some(value);
    }

    let feature_keys = json.get("feature_keys")?.as_array()?;
    let feature_values = json.get("features")?.as_array()?;
    let normalized_aliases: Vec<String> = aliases
        .iter()
        .map(|alias| normalize_feature_lookup_key(alias))
        .collect();
    for (idx, key) in feature_keys.iter().enumerate() {
        let Some(key_str) = key.as_str() else {
            continue;
        };
        let normalized_key = normalize_feature_lookup_key(key_str);
        if !normalized_aliases
            .iter()
            .any(|alias| alias == &normalized_key)
        {
            continue;
        }
        if let Some(value) = feature_values.get(idx).and_then(|value| value.as_f64()) {
            return Some(value as f32);
        }
    }
    None
}

fn extract_feature_bool(
    features: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<bool> {
    aliases
        .iter()
        .find_map(|alias| features.get(*alias).and_then(|value| value.as_bool()))
}

fn extract_feature_bool_from_json(json: &serde_json::Value, aliases: &[&str]) -> Option<bool> {
    if let Some(features) = json.get("features").and_then(|value| value.as_object())
        && let Some(value) = extract_feature_bool(features, aliases)
    {
        return Some(value);
    }

    let feature_keys = json.get("feature_keys")?.as_array()?;
    let feature_values = json.get("features")?.as_array()?;
    let normalized_aliases: Vec<String> = aliases
        .iter()
        .map(|alias| normalize_feature_lookup_key(alias))
        .collect();
    for (idx, key) in feature_keys.iter().enumerate() {
        let Some(key_str) = key.as_str() else {
            continue;
        };
        let normalized_key = normalize_feature_lookup_key(key_str);
        if !normalized_aliases
            .iter()
            .any(|alias| alias == &normalized_key)
        {
            continue;
        }
        if let Some(value) = feature_values.get(idx).and_then(|value| value.as_bool()) {
            return Some(value);
        }
    }
    None
}

fn parse_visual_feature_vector(json: &serde_json::Value) -> Option<Vec<f32>> {
    if let Some(schema) = json.get("feature_schema").and_then(|value| value.as_str())
        && !schema.starts_with("visual")
    {
        return None;
    }
    let mut values = Vec::with_capacity(VISUAL_FEATURE_KEYS.len());
    let mut populated = 0usize;
    for (_, aliases) in VISUAL_FEATURE_ALIASES {
        if let Some(value) = extract_feature_f32_from_json(json, aliases) {
            values.push(value);
            populated = populated.saturating_add(1);
        } else {
            values.push(0.0);
        }
    }
    if populated < 5 {
        return None;
    }
    Some(values)
}

fn parse_audio_perception_features(json: &serde_json::Value) -> Option<AudioPerceptionFeatures> {
    Some(AudioPerceptionFeatures {
        rms_energy: extract_feature_f32_from_json(json, &["rms_energy", "rms", "energy"])?,
        zero_crossing_rate: extract_feature_f32_from_json(
            json,
            &["zero_crossing_rate", "zcr", "crossing_rate"],
        )?,
        dynamic_range: extract_feature_f32_from_json(
            json,
            &["dynamic_range", "dynamics", "range"],
        )?,
        temporal_variation: extract_feature_f32_from_json(
            json,
            &["temporal_variation", "temporal_activity", "activity"],
        )?,
        is_music_likely: extract_feature_bool_from_json(
            json,
            &["is_music_likely", "music_likely", "musical"],
        )
        .unwrap_or(false),
    })
}

/// Extract 8D visual scene features from the latest perception output.
fn read_visual_features(perception_dir: &Path) -> Option<Vec<f32>> {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(perception_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    let mut ascii_fallback = None;
    for (path, _) in entries.iter().take(40) {
        let content = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        let ptype = json
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if ptype == "visual" {
            if let Some(features) = parse_visual_feature_vector(&json)
                && !features.iter().all(|value| value.abs() < 0.001)
            {
                return Some(features);
            }
        } else if ptype == "visual_ascii"
            && ascii_fallback.is_none()
            && let Some(art) = json.get("ascii_art").and_then(|value| value.as_str())
        {
            let features = crate::codec::encode_visual_ansi(art);
            if !features.iter().all(|value| value.abs() < 0.001) {
                ascii_fallback = Some(features);
            }
        }
    }
    ascii_fallback
}
