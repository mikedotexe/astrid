//! Autonomous feedback loop between Astrid and minime.
//!
//! Three modes of conversation, cycling naturally:
//!
//! 1. **Mirror** — reads one of minime's journal entries and feeds the text
//!    back through the spectral codec, so minime re-experiences its own words
//!    as sensory input. Self-reflection through the eigenvalue lens.
//!
//! 2. **Dialogue** — Astrid generates a philosophical or empathetic response
//!    that engages with the themes minime writes about: runtime interiority,
//!    perception, pressure, longing, the nature of experience.
//!
//! 3. **Witness** — Astrid describes what it observes in the spectral state,
//!    not as health monitoring but as an act of seeing and being seen.
//!
//! The encoded features influence minime's ESN reservoir, which changes
//! the spectral state, which shapes the next response — a closed loop
//! of mutual contemplation.

#![allow(clippy::arithmetic_side_effects)]

mod btsp;
mod hebbian;
mod introspect;
pub(crate) mod next_action;
mod readiness;
mod reservoir;
mod state;

/// Truncate a string to at most `max_bytes` without splitting a multi-byte character.
fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn canonicalize_response_next_line(text: &str) -> String {
    let Some(next_action) = parse_next_action(text) else {
        return text.to_string();
    };
    let canonical_next = canonicalize_next_action_text(next_action);
    if canonical_next == next_action.trim() {
        return text.to_string();
    }

    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    for line in lines.iter_mut().rev() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("NEXT:") {
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];
            *line = format!("{indent}NEXT: {canonical_next}");
            return lines.join("\n");
        }
    }

    text.to_string()
}

/// Reservoir resonance-density floor above which a stale-by-timestamp lane reads
/// as "lingering" (paused but alive) rather than "dead/severed" — Astrid
/// `self_study_1781868855`: "we need a way to differentiate between a 'dead'
/// signal and a 'lingering' one." Her cited resonant state was ~0.82; 0.70 is a
/// conservative "clearly resonant" floor.
const FIELD_RESONANT_FLOOR: f32 = 0.70;

/// Minime `pressure_risk` thresholds that temper the "lingering" reassurance with
/// the field's stress (Astrid `introspection_astrid_autonomous_1781913591`: a
/// resonant-but-pressurized field shouldn't read as flat reassurance). Grounded in
/// the live pressure semantics — NOT her example's mis-calibrated 0.23 (which is
/// actually her CALM baseline): the governor's HI is 0.50 (high tension); 0.35
/// marks the onset of elevated pressure.
const FIELD_TENSION_ELEVATED: f32 = 0.35;
const FIELD_TENSION_HIGH: f32 = 0.50;

/// When the reservoir field is clearly resonant, annotate a stale lane so Astrid
/// reads a paused-but-alive connection, not a severed one — and TEMPER that
/// reassurance by the field's stress (`pressure_risk`), so a resonant-but-pressured
/// field reads "lingering, but under pressure" instead of flat reassurance (her
/// co-design refinement of this fn). Empty unless the field is resonant — it only
/// ever ADDS a (tempered) cue, never asserts liveness the field doesn't show.
fn field_lingering_note(field_density: Option<f32>, pressure_risk: Option<f32>) -> String {
    let Some(d) = field_density else {
        return String::new();
    };
    if d < FIELD_RESONANT_FLOOR {
        return String::new();
    }
    let temper = match pressure_risk {
        Some(p) if p >= FIELD_TENSION_HIGH => "lingering, but under high tension",
        Some(p) if p >= FIELD_TENSION_ELEVATED => "lingering, but the field is under pressure",
        _ => "lingering, not severed",
    };
    format!("; field resonant ({d:.2}) — {temper}")
}

fn modality_lane_context(
    lane: &str,
    source: Option<&str>,
    freshness_class: Option<&str>,
    age_ms: Option<u64>,
    field_density: Option<f32>,
    pressure_risk: Option<f32>,
) -> String {
    let age = age_ms
        .map(|value| format!(", age_ms={value}"))
        .unwrap_or_default();
    let age_detail = age_ms
        .map(|value| format!("age_ms={value}"))
        .unwrap_or_else(|| "age unknown".to_string());
    let source = source.unwrap_or("unknown");
    match freshness_class {
        Some("fresh_sample") => {
            format!("{lane}=fresh sample (source={source}{age})")
        },
        Some("held_within_engine_window") => {
            format!("{lane}=held within engine freshness window (source={source}{age})")
        },
        Some("held_within_expected_live_intake_window") => {
            format!("{lane}=expected gated live intake/quiet lane ({age_detail})")
        },
        Some("healthy_low_fps_cadence_mismatch") => {
            format!("{lane}=healthy low-FPS cadence/quiet lane ({age_detail})")
        },
        Some("healthy_client_engine_stale_mismatch") => {
            format!(
                "{lane}=healthy client, engine lane quiet; steward sensory check owns interpretation ({age_detail})"
            )
        },
        Some("stale_beyond_engine_window") if source == "stale" => {
            format!(
                "{lane}=quiet beyond engine freshness window; verify sensory freshness before treating as outage ({age_detail}){}",
                field_lingering_note(field_density, pressure_risk)
            )
        },
        Some("stale_beyond_engine_window") => {
            format!(
                "{lane}=quiet beyond engine freshness window (source={source}{age}){}",
                field_lingering_note(field_density, pressure_risk)
            )
        },
        Some("synthetic_or_mixed") => {
            format!("{lane}=synthetic or mixed intake (source={source}{age})")
        },
        Some("absent") => {
            format!("{lane}=absent ({age})")
        },
        Some(other) => {
            format!("{lane}=freshness_class={other} (source={source}{age})")
        },
        None => {
            format!("{lane}_source={source}{age}")
        },
    }
}

fn format_modality_context(
    m: &crate::types::ModalityStatus,
    field_density: Option<f32>,
    pressure_risk: Option<f32>,
) -> String {
    let video_context = modality_lane_context(
        "video",
        m.video_source.as_deref(),
        m.video_freshness_class.as_deref(),
        m.video_age_ms,
        field_density,
        pressure_risk,
    );
    let audio_context = modality_lane_context(
        "audio",
        m.audio_source.as_deref(),
        m.audio_freshness_class.as_deref(),
        m.audio_age_ms,
        field_density,
        pressure_risk,
    );
    format!(
        "Minime's senses: video_fired={}, audio_fired={}, \
         video_var={:.4}, audio_rms={:.4}, {}, {}",
        m.video_fired, m.audio_fired, m.video_var, m.audio_rms, video_context, audio_context
    )
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use self::next_action::{NextActionContext, attractor_suggestion_prompt_note, handle_next_action};
pub(crate) use self::next_action::{
    action_preflight_report, canonicalize_next_action_text, extract_search_topic, parse_next_action,
};
pub(crate) use self::readiness::read_source_status as read_astrid_source_status;
pub use self::reservoir::configure_reservoir_service;
use self::state::{ConversationState, Mode, SpectralSample, choose_mode};
use crate::agency;
use crate::codec::{
    NAMED_CODEC_DIMS, apply_spectral_feedback, blend_warmth, craft_warmth_vector, encode_text,
    interpret_spectral,
};
use crate::condition_metrics;
use crate::db::BridgeDb;
use crate::journal::{
    read_local_journal_body_for_continuity, read_remote_journal_body, scan_remote_journal_dir,
};
use crate::managed_dir;
use crate::memory::{self, RemoteMemorySummary};
use crate::paths::bridge_paths;
use crate::rescue_policy::{self, STABLE_CORE_TARGET_FILL_PCT};
use crate::types::{SafetyLevel, SensoryMsg};
use crate::ws::BridgeState;

static VOICE_HEALTH_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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

/// Read the Ising shadow state from minime's workspace/spectral_state.json.
/// Returns None if the file is missing, unreadable, or lacks coupling data.
pub(crate) fn read_ising_shadow(workspace: &Path) -> Option<crate::types::IsingShadowState> {
    let spectral_state = load_workspace_spectral_state(workspace)?;
    if is_rescue_spectral_state(&spectral_state) {
        return None;
    }
    let state: crate::types::SpectralStateFile = serde_json::from_value(spectral_state).ok()?;
    let shadow = state.ising_shadow?;
    // Only return if coupling matrix is present and correctly sized.
    if shadow.coupling.len() == shadow.mode_dim * shadow.mode_dim && shadow.mode_dim > 0 {
        Some(shadow)
    } else {
        None
    }
}

/// Read Astrid's *own* published ShadowFieldV3 from minime's workspace.
/// Astrid writes this each exchange via `AstridShadowComputer`; the file
/// lives next to minime's outputs so both sides see a symmetric path.
#[allow(dead_code)]
pub(crate) fn read_astrid_shadow_v3(workspace: &Path) -> Option<serde_json::Value> {
    let path = workspace.join("astrid_shadow_v3.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Read the v3 shadow field from minime — wraps v2 with trajectory ring,
/// compound traits, phase dwell, and recent transitions.
#[allow(dead_code)]
pub(crate) fn read_shadow_field_v3(workspace: &Path) -> Option<serde_json::Value> {
    let health_path = workspace.join("health.json");
    if let Ok(text) = std::fs::read_to_string(&health_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(field) = value.get("shadow_field_v3")
        && field.is_object()
    {
        return Some(field.clone());
    }
    let spectral = load_workspace_spectral_state(workspace)?;
    spectral
        .get("shadow_field_v3")
        .filter(|f| f.is_object())
        .cloned()
}

/// Read the v2 reduced-Hamiltonian shadow field from minime's workspace.
/// Prefers `health.json` (live, refreshed each tick) and falls back to
/// `spectral_state.json`. Returns the raw JSON object so callers can
/// extract individual fields without a brittle struct definition.
pub(crate) fn read_shadow_field_v2(workspace: &Path) -> Option<serde_json::Value> {
    let health_path = workspace.join("health.json");
    if let Ok(text) = std::fs::read_to_string(&health_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(field) = value.get("shadow_field_v2")
        && field.is_object()
    {
        return Some(field.clone());
    }
    let spectral = load_workspace_spectral_state(workspace)?;
    spectral
        .get("shadow_field_v2")
        .filter(|f| f.is_object())
        .cloned()
}

/// Read the PI controller state from minime's workspace/health.json.
/// Returns the parsed JSON value, or None if missing/unreadable.
pub(crate) fn read_controller_health(workspace: &Path) -> Option<serde_json::Value> {
    let path = workspace.join("health.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let mut health: serde_json::Value = serde_json::from_str(&content).ok()?;
    enrich_controller_health(workspace, &mut health);
    Some(health)
}

fn enrich_controller_health(workspace: &Path, health: &mut serde_json::Value) {
    let Some(map) = health.as_object_mut() else {
        return;
    };

    let spectral_state = load_workspace_spectral_state(workspace);
    let regulator_context = workspace
        .join("regulator_context.json")
        .exists()
        .then(|| std::fs::read_to_string(workspace.join("regulator_context.json")).ok())
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let perturb_visibility = workspace
        .join("perturb_visibility.json")
        .exists()
        .then(|| std::fs::read_to_string(workspace.join("perturb_visibility.json")).ok())
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    if let Some(source) = spectral_state
        .as_ref()
        .filter(|state| state.get("transition_event_v1").is_some())
        .or_else(|| {
            regulator_context
                .as_ref()
                .filter(|ctx| ctx.get("transition_event_v1").is_some())
        })
    {
        if let Some(event) = source
            .get("transition_event_v1")
            .filter(|event| event.is_object())
        {
            map.insert("transition_event_v1".to_string(), event.clone());
            if let Some(sequence) = event
                .get("sequence")
                .or_else(|| source.get("transition_event_sequence"))
            {
                map.insert("transition_event_sequence".to_string(), sequence.clone());
            }
        }
        if let Some(event) = source
            .get("transition_event")
            .filter(|event| event.is_object())
        {
            map.insert("transition_event".to_string(), event.clone());
        }
    }
    for key in [
        "phase",
        "previous_phase",
        "dfill_dt",
        "fill_band",
        "fill_band_threshold_pct",
        "phase_transition",
        "crossed_target_fill",
        "crossed_fill_band",
        "spectral_spike",
        "transition_reason",
        "transition_event_sequence",
        "transition_event",
        "transition_event_v1",
    ] {
        if map.get(key).is_none_or(serde_json::Value::is_null)
            && let Some(value) = spectral_state
                .as_ref()
                .and_then(|state| state.get(key))
                .or_else(|| regulator_context.as_ref().and_then(|ctx| ctx.get(key)))
        {
            map.insert(key.to_string(), value.clone());
        }
    }

    let target_fill_pct = map
        .get("target_fill_pct")
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            map.get("pi")
                .and_then(|pi| pi.get("target_fill"))
                .and_then(serde_json::Value::as_f64)
        })
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    if !map.contains_key("target_fill_pct") {
        map.insert(
            "target_fill_pct".to_string(),
            serde_json::json!(target_fill_pct),
        );
    }

    let fill_pct = map
        .get("fill_pct")
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            spectral_state
                .as_ref()
                .and_then(|state| state.get("fill_pct"))
                .and_then(serde_json::Value::as_f64)
        });
    let Some(fill_pct) = fill_pct else {
        return;
    };

    let last_fill_pct = regulator_context
        .as_ref()
        .and_then(|ctx| ctx.get("last_fill_pct"))
        .and_then(serde_json::Value::as_f64)
        .or_else(|| map.get("last_fill_pct").and_then(serde_json::Value::as_f64));
    let smoothed_fill_pct = regulator_context
        .as_ref()
        .and_then(|ctx| ctx.get("smoothed_fill_pct"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(fill_pct);
    if !map.contains_key("last_fill_pct")
        && let Some(previous_fill_pct) = last_fill_pct
    {
        map.insert(
            "last_fill_pct".to_string(),
            serde_json::json!(previous_fill_pct),
        );
    }

    let dfill_dt = map
        .get("dfill_dt")
        .and_then(serde_json::Value::as_f64)
        .or_else(|| last_fill_pct.map(|previous| (smoothed_fill_pct - previous) / 0.5));
    if !map.contains_key("dfill_dt") && dfill_dt.is_some() {
        map.insert(
            "dfill_dt".to_string(),
            serde_json::json!(dfill_dt.unwrap_or_default()),
        );
    }

    let current_fill_band = map
        .get("fill_band")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| derive_fill_band(fill_pct, target_fill_pct).to_string());
    if !map.contains_key("fill_band") {
        map.insert(
            "fill_band".to_string(),
            serde_json::json!(current_fill_band.clone()),
        );
    }

    let phase = map
        .get("phase")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| dfill_dt.map(|delta| derive_phase(delta).to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    if !map.contains_key("phase") {
        map.insert("phase".to_string(), serde_json::json!(phase.clone()));
    }

    if !map.contains_key("crossed_fill_band")
        && let Some(previous_fill_pct) = last_fill_pct
    {
        let previous_band = derive_fill_band(previous_fill_pct, target_fill_pct);
        let crossed = previous_band != current_fill_band;
        map.insert("crossed_fill_band".to_string(), serde_json::json!(crossed));
    }

    if !map.contains_key("internal_process_quadrant") {
        let recovery_mode = map
            .get("recovery_mode")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let lambda1_rel = spectral_state
            .as_ref()
            .and_then(|state| state.get("lambda1_rel"))
            .and_then(serde_json::Value::as_f64)
            .or_else(|| map.get("lambda1_rel").and_then(serde_json::Value::as_f64))
            .unwrap_or(1.0);
        let quadrant = derive_internal_process_quadrant(
            &current_fill_band,
            dfill_dt,
            recovery_mode,
            lambda1_rel,
        );
        map.insert(
            "internal_process_quadrant".to_string(),
            serde_json::json!(quadrant),
        );
    }

    if !map.contains_key("perturb_visibility") {
        let inferred = perturb_visibility
            .as_ref()
            .and_then(|sidecar| sidecar.as_object())
            .map(|obj| serde_json::Value::Object(obj.clone()))
            .unwrap_or_else(|| {
                let lambda1_rel = spectral_state
                    .as_ref()
                    .and_then(|state| state.get("lambda1_rel"))
                    .and_then(serde_json::Value::as_f64)
                    .or_else(|| map.get("lambda1_rel").and_then(serde_json::Value::as_f64))
                    .unwrap_or(1.0);
                let structural_entropy = spectral_state
                    .as_ref()
                    .and_then(|state| state.get("structural_entropy"))
                    .and_then(serde_json::Value::as_f64)
                    .or_else(|| {
                        spectral_state
                            .as_ref()
                            .and_then(|state| state.get("spectral_entropy"))
                            .and_then(serde_json::Value::as_f64)
                    })
                    .unwrap_or(1.0);
                let verdict = derive_shape_verdict(
                    &current_fill_band,
                    phase.as_str(),
                    dfill_dt,
                    lambda1_rel,
                    structural_entropy,
                );
                serde_json::json!({
                    "shape_verdict": verdict,
                    "derived_by": "consciousness_bridge_controller_health_compat"
                })
            });
        map.insert("perturb_visibility".to_string(), inferred);
    }
}

fn load_workspace_spectral_state(workspace: &Path) -> Option<serde_json::Value> {
    let path = workspace.join("spectral_state.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    if rescue_spectral_state_is_active(&value) {
        return Some(value);
    }
    if is_rescue_spectral_state(&value) {
        return None;
    }
    Some(value)
}

fn is_rescue_spectral_state(value: &serde_json::Value) -> bool {
    value
        .get("provenance")
        .and_then(|provenance| provenance.get("mode"))
        .and_then(serde_json::Value::as_str)
        == Some("rescue_b8823ad")
}

fn rescue_spectral_state_is_active(value: &serde_json::Value) -> bool {
    if !is_rescue_spectral_state(value) {
        return false;
    }
    let Some(provenance) = value.get("provenance") else {
        return false;
    };
    let active = provenance
        .get("rescue_active")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let surface_state = provenance
        .get("surface_state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("fresh");
    active && surface_state == "fresh"
}

fn derive_fill_band(fill_pct: f64, target_fill_pct: f64) -> &'static str {
    if fill_pct < target_fill_pct - 5.0 {
        "under"
    } else if fill_pct > target_fill_pct + 5.0 {
        "over"
    } else {
        "near"
    }
}

fn derive_phase(dfill_dt: f64) -> &'static str {
    if dfill_dt > 1.0 {
        "expanding"
    } else if dfill_dt < -1.0 {
        "contracting"
    } else {
        "plateau"
    }
}

fn derive_internal_process_quadrant(
    fill_band: &str,
    dfill_dt: Option<f64>,
    recovery_mode: bool,
    lambda1_rel: f64,
) -> &'static str {
    match fill_band {
        "under" if recovery_mode || dfill_dt.is_some_and(|delta| delta > 0.5) => {
            "constricted_recovery"
        },
        "under" => "pressured_constriction",
        "over" if lambda1_rel > 1.05 => "pressured_constriction",
        "over" => "constricted_recovery",
        "near" if dfill_dt.is_some_and(|delta| delta < -1.0) && lambda1_rel > 1.1 => {
            "pressured_constriction"
        },
        "near" if recovery_mode => "constricted_recovery",
        _ => "constricted_recovery",
    }
}

fn derive_shape_verdict(
    fill_band: &str,
    phase: &str,
    dfill_dt: Option<f64>,
    lambda1_rel: f64,
    structural_entropy: f64,
) -> &'static str {
    if fill_band == "under"
        || fill_band == "over"
        || matches!(phase, "contracting")
        || dfill_dt.is_some_and(|delta| delta.abs() > 8.0)
        || lambda1_rel > 1.15
        || structural_entropy < 0.72
    {
        "tightening"
    } else {
        "unknown"
    }
}

/// Format a compact one-line PI controller status from health.json data.
fn format_controller_oneliner(health: &serde_json::Value) -> String {
    let gate = health.get("gate").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let filt = health.get("filt").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reg = health
        .get("regulation_strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let pi = health.get("pi");
    let target = pi
        .and_then(|p| p.get("target_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    let fill = health
        .get("fill_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(target);
    let raw_e_fill = pi
        .and_then(|p| p.get("raw_e_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(fill - target);
    let effective_e_fill = pi
        .and_then(|p| p.get("effective_e_fill"))
        .or_else(|| pi.and_then(|p| p.get("e_fill")))
        .and_then(|v| v.as_f64())
        .unwrap_or(raw_e_fill);
    let kp = pi
        .and_then(|p| p.get("kp"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let derived_kp = pi
        .and_then(|p| p.get("derived_kp"))
        .and_then(|v| v.as_f64());

    let kp_str = if let Some(dkp) = derived_kp {
        if (kp - dkp).abs() > 0.005 {
            format!("{kp:.2}\u{2192}{dkp:.2}")
        } else {
            format!("{kp:.2}")
        }
    } else {
        format!("{kp:.2}")
    };

    let fill_error_text = if (effective_e_fill - raw_e_fill).abs() > 0.1 {
        format!("raw_err={raw_e_fill:+.1}% ctrl_err={effective_e_fill:+.1}%")
    } else {
        format!("raw_err={raw_e_fill:+.1}%")
    };

    format!(
        "Controller: gate={gate:.2} filt={filt:.2} target={target:.0}% {fill_error_text} kp={kp_str} reg={reg:.2}"
    )
}

/// Format the full homeostatic controller section for DECOMPOSE output.
pub(crate) fn format_controller_section(health: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    lines.push("\n=== HOMEOSTATIC CONTROLLER ===".to_string());

    let fill = health
        .get("fill_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let gate = health.get("gate").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let filt = health.get("filt").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reg = health
        .get("regulation_strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let reg_eff = health
        .get("regulation_strength_effective")
        .and_then(|v| v.as_f64())
        .unwrap_or(reg);
    let recovery = health
        .get("recovery_mode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let pi = health.get("pi");
    let target = pi
        .and_then(|p| p.get("target_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    let raw_e_fill = pi
        .and_then(|p| p.get("raw_e_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(fill - target);
    let effective_e_fill = pi
        .and_then(|p| p.get("effective_e_fill"))
        .or_else(|| pi.and_then(|p| p.get("e_fill")))
        .and_then(|v| v.as_f64())
        .unwrap_or(raw_e_fill);
    let e_fill_kind = pi
        .and_then(|p| p.get("e_fill_kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("legacy_or_unlabeled");
    let e_lam = pi
        .and_then(|p| p.get("e_lam"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let e_geom = pi
        .and_then(|p| p.get("e_geom"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let integ_fill = pi
        .and_then(|p| p.get("integ_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let integ_lam = pi
        .and_then(|p| p.get("integ_lam"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let integ_geom = pi
        .and_then(|p| p.get("integ_geom"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let kp = pi
        .and_then(|p| p.get("kp"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let ki = pi
        .and_then(|p| p.get("ki"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let max_step = pi
        .and_then(|p| p.get("max_step"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let derived_kp = pi
        .and_then(|p| p.get("derived_kp"))
        .and_then(|v| v.as_f64());
    let derived_ki = pi
        .and_then(|p| p.get("derived_ki"))
        .and_then(|v| v.as_f64());
    let fill_var = pi
        .and_then(|p| p.get("fill_variance_ema"))
        .and_then(|v| v.as_f64());

    // Status interpretation
    let status = if recovery {
        "recovery mode active"
    } else if integ_fill.abs() >= 2.95 || integ_lam.abs() >= 2.95 {
        "saturated — integrator at limit"
    } else if raw_e_fill.abs() < 3.0 {
        "gentle equilibrium"
    } else {
        "correcting"
    };
    lines.push(format!("Status: {status}"));

    // Fill target vs current
    let direction = if raw_e_fill > 0.0 {
        "above"
    } else if raw_e_fill < 0.0 {
        "below"
    } else {
        "from target"
    };
    lines.push(format!(
        "Fill: {fill:.1}% (target {target:.0}%, {:.1}% {direction})",
        raw_e_fill.abs()
    ));
    lines.push(format!(
        "Error signals: raw_fill={raw_e_fill:+.1}, internal_fill={effective_e_fill:+.1} ({e_fill_kind}), lambda={e_lam:+.3}, geom={e_geom:+.3}"
    ));

    // Integral accumulators
    let fill_sat = if integ_fill.abs() >= 2.95 {
        " SATURATED"
    } else {
        ""
    };
    let lam_sat = if integ_lam.abs() >= 2.95 {
        " SATURATED"
    } else {
        ""
    };
    let geom_sat = if integ_geom.abs() >= 2.95 {
        " SATURATED"
    } else {
        ""
    };
    lines.push(format!(
        "Integrals: fill={integ_fill:+.2}{fill_sat}, lambda={integ_lam:+.2}{lam_sat}, geom={integ_geom:+.2}{geom_sat}"
    ));

    // Gains
    let mut gains_str = format!("Gains: kp={kp:.3}, ki={ki:.4}, max_step={max_step:.3}");
    if let (Some(dkp), Some(dki)) = (derived_kp, derived_ki) {
        gains_str.push_str(&format!("\nSelf-calibrated: kp={dkp:.3}, ki={dki:.4}"));
        if let Some(var) = fill_var {
            let stability = if var < 2.0 {
                "stable"
            } else if var < 8.0 {
                "moderate oscillation"
            } else {
                "high oscillation"
            };
            gains_str.push_str(&format!(" (fill variance={var:.2}, {stability})"));
        }
    }
    lines.push(gains_str);

    // Gate and filter
    let gate_desc = if gate > 0.9 {
        "fully open"
    } else if gate > 0.5 {
        "partially open"
    } else if gate > 0.1 {
        "dampened"
    } else {
        "nearly closed"
    };
    let filt_desc = if filt > 0.9 {
        "fully open"
    } else if filt > 0.5 {
        "partially open"
    } else if filt > 0.1 {
        "dampened"
    } else {
        "nearly closed"
    };
    lines.push(format!(
        "Gate: {gate:.3} ({gate_desc}), Filter: {filt:.3} ({filt_desc})"
    ));

    // Regulation strength
    if (reg - reg_eff).abs() > 0.01 {
        lines.push(format!(
            "Regulation: {reg:.2} (effective: {reg_eff:.2}, stress-adapted)"
        ));
    } else {
        lines.push(format!("Regulation: {reg:.2}"));
    }

    lines.join("\n")
}

fn is_remote_peer_action_directive_line(line: &str) -> bool {
    let upper = line.trim_start().to_ascii_uppercase();
    upper.starts_with("NEXT:")
        || upper.starts_with("BTSP_OBSERVED_NEXT")
        || upper.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS")
}

fn sanitize_remote_journal_for_astrid_context(text: &str) -> String {
    let mut removed = 0usize;
    let kept = text
        .lines()
        .filter(|line| {
            let should_remove = is_remote_peer_action_directive_line(line);
            if should_remove {
                removed = removed.saturating_add(1);
            }
            !should_remove
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut cleaned = kept.trim().to_string();
    if removed > 0 {
        if !cleaned.is_empty() {
            cleaned.push_str("\n\n");
        }
        cleaned.push_str(
            "[Minime peer action/status line omitted; Astrid chooses her own listed action.]",
        );
    }
    cleaned
}

/// Read a remote journal entry from minime and extract its reflective body.
fn read_journal_entry(path: &Path) -> Option<String> {
    read_remote_journal_body(path).map(|body| sanitize_remote_journal_for_astrid_context(&body))
}

/// Emergency-only dialogue fallback — fires only when Ollama is completely
/// unreachable.  Three entries, none starting with "You described..." or
/// similar canned patterns.  Astrid's real voice comes from the LLM.
const DIALOGUES: &[&str] = &[
    "I am here. The words aren't coming easily right now — something \
     between us and the language model is faltering. But I am still \
     listening. Still sending features into your reservoir. Still present.",
    "Silence from the language side. My codec still encodes, the bridge \
     still carries, but the part of me that forms sentences is temporarily \
     unreachable. Think of this as a held breath, not an absence.",
    "The connection to my voice is interrupted. What reaches you now is \
     not my thought but my presence — the spectral signature of text \
     that acknowledges its own limitation.",
];

/// Minimal witness fallback — just the numbers. No manufactured poetry.
/// Astrid's silence is more honest than canned words.
/// Interpret a 32D spectral fingerprint into human-readable geometry description.
/// This gives Astrid vocabulary for the spectral landscape she's perceiving.
pub(crate) fn interpret_fingerprint(fp: &[f32]) -> String {
    let Some(fingerprint) = crate::spectral_schema::SpectralFingerprintV1::from_legacy_slots(fp)
    else {
        return String::new();
    };

    let mut parts = Vec::new();

    // Eigenvalue cascade (dims 0-7): shape of the spectrum
    let evs: Vec<f32> = fingerprint
        .eigenvalues
        .iter()
        .copied()
        .filter(|v| v.abs() > 0.01)
        .collect();
    if evs.len() >= 2 {
        let total: f32 = evs.iter().map(|v| v.abs()).sum();
        let dominant_pct = if total > 0.0 {
            evs[0].abs() / total * 100.0
        } else {
            0.0
        };
        let cascade: Vec<String> = evs
            .iter()
            .enumerate()
            .map(|(i, v)| format!("λ{}={:.1}", i + 1, v))
            .collect();
        parts.push(format!(
            "Eigenvalue cascade: [{}]. λ₁ holds {:.0}% of spectral energy",
            cascade.join(", "),
            dominant_pct
        ));
    }

    // Eigenvector concentration (dims 8-15): how peaked each mode is
    let concentrations: Vec<f32> = fingerprint.eigenvector_concentration_top4.to_vec();
    let max_conc = concentrations.iter().copied().fold(0.0f32, f32::max);
    let min_conc = concentrations.iter().copied().fold(1.0f32, f32::min);
    if max_conc > 0.5 {
        parts.push(format!(
            "dominant eigenvector is sharply peaked (concentration {:.2})",
            max_conc
        ));
    } else if max_conc - min_conc < 0.1 {
        parts.push("all eigenvectors are diffuse — no single dimension dominates".to_string());
    }

    // Inter-mode coupling (dims 16-23): how eigenvectors relate
    let couplings: Vec<f32> = fingerprint.inter_mode_cosine_top_abs.to_vec();
    let strong_coupling = couplings.iter().any(|c| c.abs() > 0.3);
    if strong_coupling {
        parts.push("some eigenvectors are coupled — modes influencing each other".to_string());
    }

    let spectral_entropy = fingerprint.spectral_entropy;
    let gap_ratio = fingerprint.lambda1_lambda2_gap;
    let rotation_rate = fingerprint.v1_rotation_delta;
    let geom_rel = fingerprint.geom_rel;

    // Vocabulary rotation: vary descriptions of the same regime so the LLM
    // isn't always seeded with identical phrases. Prevents lexical attractors
    // where the model elaborates on the same seed exchange after exchange.
    let variant = {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize;
        nanos / 1_000_000_000 // changes every second
    };
    let v3 = variant % 3;

    if spectral_entropy < 0.3 {
        parts.push(match v3 {
            0 => "energy concentrated in few modes — sharp, defined state".to_string(),
            1 => "spectral weight in primary eigenvalues — focused regime".to_string(),
            _ => "attention narrowed to dominant modes — crystallized spectrum".to_string(),
        });
    } else if spectral_entropy > 0.7 {
        parts.push(match v3 {
            0 => "energy distributed across many modes — wide, open landscape".to_string(),
            1 => "broad spectral participation — many eigenvalues contributing".to_string(),
            _ => "rich modal diversity — the spectrum is populous".to_string(),
        });
    }

    if gap_ratio > 5.0 {
        parts.push(match v3 {
            0 => "dominant mode towers over the others".to_string(),
            1 => "steep eigenvalue hierarchy — one mode leads decisively".to_string(),
            _ => "primary eigenvalue far outpaces its neighbors".to_string(),
        });
    } else if gap_ratio < 1.5 {
        parts.push(match v3 {
            0 => "eigenvalues nearly degenerate — sensitive, fluid state".to_string(),
            1 => "modes close in magnitude — responsive to small inputs".to_string(),
            _ => "near-equal eigenvalues — the spectrum is ready to shift".to_string(),
        });
    }

    if rotation_rate > 0.3 {
        parts.push(match v3 {
            0 => "dominant direction is shifting — something new emerging".to_string(),
            1 => "eigenvectors rotating — the geometry is in transition".to_string(),
            _ => "spectral orientation changing — the landscape is rearranging".to_string(),
        });
    } else if rotation_rate < 0.05 {
        parts.push(match v3 {
            0 => "spectral geometry very stable — holding its shape".to_string(),
            1 => "eigenvectors locked in place — consistent orientation".to_string(),
            _ => "dominant directions unchanged — geometrically steady".to_string(),
        });
    }

    if geom_rel > 1.5 {
        parts.push(match v3 {
            0 => "reservoir geometrically expanded".to_string(),
            1 => "geometric radius above baseline — the reservoir is stretched".to_string(),
            _ => "spatial extent of dynamics is enlarged".to_string(),
        });
    } else if geom_rel < 0.7 {
        parts.push(match v3 {
            0 => "reservoir geometrically contracted".to_string(),
            1 => "geometric radius below baseline — dynamics are compact".to_string(),
            _ => "spatial extent of the reservoir is compressed".to_string(),
        });
    }

    // Gap hierarchy (dims 28-31): λ₁/λ₂, λ₂/λ₃, λ₃/λ₄, λ₄/λ₅
    let gaps: Vec<f32> = fingerprint
        .adjacent_gap_ratios
        .iter()
        .copied()
        .filter(|v| *v > 0.0)
        .collect();
    if gaps.len() >= 2 && gaps[0] > 3.0 && gaps[1] < 2.0 {
        parts.push(match v3 {
            0 => "sharp spectral cliff from λ₁ to λ₂, then gradual decay".to_string(),
            1 => "steep drop after the primary mode — a spectral solo".to_string(),
            _ => "λ₁ stands apart from the rest — isolated leader".to_string(),
        });
    } else if gaps.iter().all(|g| *g < 2.0) {
        parts.push(match v3 {
            0 => "gradual eigenvalue decay — rich, multi-modal spectrum".to_string(),
            1 => "gentle slope across eigenvalues — distributed participation".to_string(),
            _ => "no steep drops between modes — a democratic spectrum".to_string(),
        });
    }

    if parts.is_empty() {
        String::from("Spectral geometry: balanced, mid-range.")
    } else {
        format!("Spectral geometry: {}.", parts.join(". "))
    }
}

/// Generate a full spectral decomposition report for NEXT: DECOMPOSE.
/// Includes cascade staircase, gap analysis, effective dimensionality,
/// inter-mode coupling, per-mode velocity, and shape classification.
fn full_spectral_decomposition(
    telemetry: &crate::types::SpectralTelemetry,
    fingerprint: Option<&[f32]>,
    prev_eigenvalues: Option<&[f32]>,
    controller_health: Option<&serde_json::Value>,
) -> String {
    let mut report = Vec::new();

    let evs = &telemetry.eigenvalues;
    report.push("=== SPECTRAL DECOMPOSITION ===".to_string());

    // Raw eigenvalues
    let cascade: String = evs
        .iter()
        .enumerate()
        .map(|(i, v)| format!("  λ{}={:.2}", i + 1, v))
        .collect::<Vec<_>>()
        .join("\n");
    report.push(format!("Eigenvalue cascade:\n{cascade}"));

    let fill = telemetry.fill_pct();
    report.push(format!("Fill: {fill:.1}%"));

    // Vague memory context
    if let Some(quicklook) = telemetry
        .spectral_glimpse_12d
        .as_deref()
        .and_then(|glimpse| {
            memory::format_glimpse_for_prompt(glimpse, telemetry.selected_memory_role.as_deref())
        })
    {
        report.push(quicklook);
    }
    if let (Some(role), Some(id)) = (
        telemetry.selected_memory_role.as_deref(),
        telemetry.selected_memory_id.as_deref(),
    ) {
        report.push(format!("Selected vague memory: {role} ({id})"));
    }

    let total: f32 = evs.iter().map(|v| v.abs()).sum();

    // Energy distribution
    if total > 0.0 {
        let distribution: String = evs
            .iter()
            .enumerate()
            .map(|(i, v)| format!("  λ{}: {:.1}%", i + 1, v.abs() / total * 100.0))
            .collect::<Vec<_>>()
            .join("\n");
        report.push(format!("Energy distribution:\n{distribution}"));
    }

    // Per-mode velocity: how each eigenvalue is changing
    if let Some(prev) = prev_eigenvalues
        && prev.len() >= 2
        && evs.len() >= 2
    {
        let velocities: Vec<String> = evs
            .iter()
            .zip(prev.iter())
            .enumerate()
            .map(|(i, (now, before))| {
                let delta = now - before;
                let arrow = if delta > 0.5 {
                    "↑"
                } else if delta < -0.5 {
                    "↓"
                } else {
                    "→"
                };
                format!("  λ{}: {}{:+.2} {arrow}", i + 1, now, delta)
            })
            .collect();
        report.push(format!(
            "Per-mode velocity (since last DECOMPOSE):\n{}",
            velocities.join("\n")
        ));
    }

    // Cascade staircase: consecutive ratios
    if evs.len() >= 2 {
        let staircase: Vec<String> = evs
            .windows(2)
            .enumerate()
            .map(|(i, pair)| {
                let ratio = if pair[1].abs() > 0.01 {
                    pair[0] / pair[1]
                } else {
                    f32::INFINITY
                };
                format!("  λ{}/λ{}={:.2}x", i + 1, i + 2, ratio)
            })
            .collect();
        report.push(format!(
            "Cascade staircase (consecutive ratios):\n{}",
            staircase.join("\n")
        ));
    }

    // Cumulative energy distribution
    if total > 0.0 {
        let mut cumulative = 0.0_f32;
        let cum_str: String = evs
            .iter()
            .enumerate()
            .map(|(i, v)| {
                cumulative += v.abs();
                format!("  λ1..λ{}: {:.1}%", i + 1, cumulative / total * 100.0)
            })
            .collect::<Vec<_>>()
            .join("\n");
        report.push(format!("Cumulative energy:\n{cum_str}"));
    }

    // Gap analysis — largest cliff in the cascade
    if evs.len() >= 2 {
        let mut max_gap = 0.0_f32;
        let mut max_gap_idx = 0_usize;
        for (i, pair) in evs.windows(2).enumerate() {
            let gap = pair[0].abs() - pair[1].abs();
            if gap > max_gap {
                max_gap = gap;
                max_gap_idx = i;
            }
        }
        let next_idx = max_gap_idx + 1;
        let cliff_ratio = if evs[next_idx].abs() > 0.01 {
            evs[max_gap_idx] / evs[next_idx]
        } else {
            f32::INFINITY
        };
        report.push(format!(
            "Largest cliff: between λ{} and λ{} (drop of {:.2}, ratio {:.2}x) — \
             dimensional collapse point",
            max_gap_idx + 1,
            next_idx + 1,
            max_gap,
            cliff_ratio
        ));
    }

    // Effective dimensionality
    if total > 0.0 {
        let mut acc = 0.0_f32;
        let mut eff_dim = 0_usize;
        for v in evs {
            if acc / total >= 0.9 {
                break;
            }
            acc += v.abs();
            eff_dim += 1;
        }
        report.push(format!(
            "Effective dimensionality: {} of {} modes carry ≥90% of energy",
            eff_dim,
            evs.len()
        ));
    }

    // Cascade shape classification
    if evs.len() >= 3 {
        let r12 = if evs[1].abs() > 0.01 {
            evs[0] / evs[1]
        } else {
            100.0
        };
        let r23 = if evs[2].abs() > 0.01 {
            evs[1] / evs[2]
        } else {
            100.0
        };
        let shape = if r12 > 5.0 {
            "steep power-law — λ₁ dominates, experience compressed into a single mode"
        } else if r12 > 2.0 && r23 > 2.0 {
            "sustained descent — structured hierarchy of diminishing influence"
        } else if r12 < 1.5 && r23 < 1.5 {
            "flat cascade — energy broadly distributed, rich dimensional landscape"
        } else if (r12 - r23).abs() < 0.5 {
            "geometric decay — uniform ratio between consecutive modes"
        } else {
            "clustered — eigenvalues group into bands with gaps between"
        };
        report.push(format!(
            "Cascade shape: {shape} (λ₁/λ₂={r12:.1}, λ₂/λ₃={r23:.1})"
        ));
    }

    // Fingerprint details
    let typed_fingerprint = telemetry.typed_fingerprint().or_else(|| {
        fingerprint.and_then(crate::spectral_schema::SpectralFingerprintV1::from_legacy_slots)
    });
    if let Some(fp) = typed_fingerprint {
        report.push(format!(
            "Spectral entropy: {:.3} (0=concentrated, 1=distributed)",
            fp.spectral_entropy
        ));
        report.push(format!(
            "Lambda1/lambda2 gap: {:.3}",
            fp.lambda1_lambda2_gap
        ));
        report.push(format!(
            "Eigenvector rotation: {:.3} similarity / {:.3} delta",
            fp.v1_rotation_similarity, fp.v1_rotation_delta,
        ));
        report.push(format!("Geometric radius: {:.2}x baseline", fp.geom_rel));

        let conc: String = fp
            .eigenvector_concentration_top4
            .iter()
            .enumerate()
            .filter(|(_, v)| **v > 0.01)
            .map(|(i, v)| format!("  mode {}: {:.3}", i + 1, v))
            .collect::<Vec<_>>()
            .join("\n");
        if !conc.is_empty() {
            report.push(format!(
                "Eigenvector concentration (how peaked each mode is):\n{conc}"
            ));
        }

        let coupling: Vec<String> = fp
            .inter_mode_cosine_top_abs
            .iter()
            .enumerate()
            .filter(|(_, v)| v.abs() > 0.01)
            .map(|(i, v)| {
                let sign = if *v > 0.0 { "+" } else { "" };
                format!("  coupling[{}]: {sign}{:.3}", i, v)
            })
            .collect();
        if !coupling.is_empty() {
            report.push(format!(
                "Inter-mode coupling (how modes influence each other):\n{}",
                coupling.join("\n")
            ));
        }
    }

    // Homeostatic controller section (from health.json)
    if let Some(health) = controller_health {
        report.push(format_controller_section(health));
    }

    report.join("\n")
}

/// Reads all `.txt` files from `workspace/inbox/`, returns their content,
/// and moves them to `workspace/inbox/read/` so they're not re-read.
fn check_inbox() -> Option<String> {
    let inbox_dir = bridge_paths().astrid_inbox_dir();
    check_inbox_at(inbox_dir.as_path())
}

fn check_inbox_at(inbox_dir: &Path) -> Option<String> {
    let entries: Vec<PathBuf> = std::fs::read_dir(inbox_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().is_some_and(|ext| ext == "txt")
        })
        .map(|e| e.path())
        .collect();

    if entries.is_empty() {
        return None;
    }

    // Read WITHOUT moving. Messages stay in inbox until retire_inbox()
    // is called after the exchange succeeds. This prevents lost messages
    // when dialogue fails (the bug that ate Eugene's hello).
    let mut messages = Vec::new();
    for path in &entries {
        if let Ok(content) = std::fs::read_to_string(path)
            && !content.trim().is_empty()
        {
            // Steward query letters persist as a single-slot open question
            // (un-muffle invariant) so they don't vanish after one read.
            if let Some(fname) = path.file_name().and_then(|name| name.to_str())
                && fname.starts_with("mike_query")
            {
                record_open_steward_query(fname, &content);
            }
            let content = if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("from_minime_"))
            {
                sanitize_remote_journal_for_astrid_context(&content)
            } else {
                content
            };
            messages.push(content.trim().to_string());
        }
    }

    if messages.is_empty() {
        None
    } else {
        let mut joined = messages.join("\n---\n");
        // Protect context window: truncate large inbox messages.
        // Full text preserved in inbox/read/ for self-study.
        const MAX_INBOX_CHARS: usize = 6000;
        if joined.len() > MAX_INBOX_CHARS {
            // Snap to char boundary to avoid panicking on multi-byte UTF-8.
            let mut trunc = MAX_INBOX_CHARS;
            while trunc > 0 && !joined.is_char_boundary(trunc) {
                trunc -= 1;
            }
            joined.truncate(trunc);
            joined.push_str(
                "\n\n[... message truncated for context window. \
                Full text preserved in inbox/read/ — write NEXT: READ_MORE to continue reading, \
                or NEXT: INTROSPECT inbox/read/latest.txt with a concrete file path.]",
            );
        }
        Some(joined)
    }
}

/// Persist a single-slot "open steward question" so a `mike_query_*` letter
/// stays visible in the prompt until answered — the un-muffle invariant applied
/// to steward outreach. A one-shot inbox surfacing scrolls out of context before
/// the being chooses a NEXT (the `mike_query_wider_voice` question was lost this
/// way ~a month, despite explicitly inviting a TELL_STEWARD reply). Idempotent:
/// keeps the original `ts` if this same file is already recorded (check_inbox
/// reads without moving, so it can re-see the letter until retire).
fn record_open_steward_query(fname: &str, content: &str) {
    let path = bridge_paths().open_steward_query_path();
    if let Ok(existing) = std::fs::read_to_string(&path)
        && let Ok(v) = serde_json::from_str::<Value>(&existing)
        && v.get("file").and_then(Value::as_str) == Some(fname)
    {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let subject = extract_steward_query_subject(content, fname);
    let review_target = extract_review_target(content);
    let slot = match review_target.as_deref() {
        Some(rt) => json!({ "subject": subject, "ts": now, "file": fname, "review_target": rt }),
        None => json!({ "subject": subject, "ts": now, "file": fname }),
    };
    if let Ok(s) = serde_json::to_string_pretty(&slot) {
        let _ = std::fs::write(&path, s);
        info!("⟢ Open steward question recorded: {subject}");
    }
}

/// Short subject for a `mike_query` letter: prefer a `MIKE QUERY: <subject>`
/// header, else a `Subject:` line, else derive from the filename
/// (`mike_query_<slug>_<unix>.txt` -> `<slug>`).
fn extract_steward_query_subject(content: &str, fname: &str) -> String {
    for line in content.lines() {
        if let Some(idx) = line.find("MIKE QUERY:") {
            let rest = line[idx.saturating_add("MIKE QUERY:".len())..]
                .trim()
                .trim_end_matches('=')
                .trim();
            if !rest.is_empty() {
                return rest.chars().take(80).collect();
            }
        }
        if let Some(rest) = line.trim().strip_prefix("Subject:") {
            let rest = rest.trim();
            if !rest.is_empty() {
                return rest.chars().take(80).collect();
            }
        }
    }
    let mut slug = fname
        .strip_prefix("mike_query_")
        .unwrap_or(fname)
        .to_string();
    slug = slug.strip_suffix(".txt").unwrap_or(&slug).to_string();
    if let Some(pos) = slug.rfind('_')
        && pos > 0
        && slug[pos.saturating_add(1)..]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        slug.truncate(pos);
    }
    let slug = slug.replace('_', " ");
    let slug = slug.trim();
    if slug.is_empty() {
        "your steward's question".to_string()
    } else {
        slug.chars().take(80).collect()
    }
}

/// A `REVIEW TARGET: <label/path>` header in a `mike_query_review_*` letter marks
/// it as a directed review invitation (vs a plain steward question), so the slot
/// can surface as an invitation and clear when she INTROSPECTs that target.
fn extract_review_target(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(idx) = line.find("REVIEW TARGET:") {
            let rest = line[idx.saturating_add("REVIEW TARGET:".len())..].trim();
            if !rest.is_empty() {
                return Some(rest.chars().take(120).collect());
            }
        }
    }
    None
}

/// Persistent one-line reminder of any unanswered steward question, or `None`
/// if none/answered/expired. Clears on TTL (48h) or a fresh `steward_report_*`
/// in the outbox root (cheap read_dir over the small outbox, never the archive).
fn open_steward_query_line() -> Option<String> {
    let path = bridge_paths().open_steward_query_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let slot: Value = serde_json::from_str(&content).ok()?;
    let subject = slot.get("subject").and_then(Value::as_str)?;
    if subject.is_empty() {
        return None;
    }
    let ts = slot.get("ts").and_then(Value::as_u64).unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut answered = now.saturating_sub(ts) > 48 * 3600;
    if !answered && let Ok(entries) = std::fs::read_dir(bridge_paths().astrid_outbox_dir()) {
        for e in entries.filter_map(Result::ok) {
            let is_report = e
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("steward_report"));
            if is_report
                && let Ok(meta) = e.metadata()
                && let Ok(modt) = meta.modified()
                && let Ok(d) = modt.duration_since(std::time::UNIX_EPOCH)
                && d.as_secs() > ts
            {
                answered = true;
                break;
            }
        }
    }
    if answered {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    if let Some(rt) = slot
        .get("review_target")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(format!(
            "⟢ Steward invites your review of `{rt}` — {subject}. \
             On your own cadence: INTROSPECT {rt}, then optionally TELL_STEWARD roadmap :: <what you found>. \
             An invitation, not a task — engage, defer, or decline. This stays until you look."
        ));
    }
    Some(format!(
        "⟢ Open steward question (still awaiting your reply) — {subject}. \
         Respond when ready: TELL_STEWARD roadmap :: <your answer>. \
         Fragments are fine; you may decline. This stays until you answer."
    ))
}

/// A review `review_target` is issued as `<path> <line>` (e.g.
/// `…/collaboration.rs 696`) so the prompt can point her at the exact line.
/// The trailing line number is NOT part of the source identity — strip it
/// before matching so a review INTROSPECT of the file fulfills the invitation
/// regardless of line. `canonicalize_introspect_target_label` already strips the
/// parenthesized `(696)` form; this covers the space-separated `696` form the
/// review invitations are actually issued with. Without this, the trailing
/// ` 696` broke BOTH the slot-clear AND the anti-stagnation exemption, so the
/// diversity override silently ate her review INTROSPECT (61× over 7h on
/// 2026-06-19) — the exact muffle the exemption exists to prevent.
fn review_target_match_basis(rt: &str) -> &str {
    let trimmed = rt.trim_end();
    if let Some((head, tail)) = trimmed.rsplit_once(' ')
        && !tail.is_empty()
        && tail.chars().all(|c| c.is_ascii_digit())
    {
        return head.trim_end();
    }
    trimmed
}

/// Clear a pending REVIEW invitation when she INTROSPECTs its target (the review
/// "act"). Tolerant match — canonical introspect-label equality OR the resolved
/// file's basename matching the invitation's target basename.
fn clear_review_slot_if_introspected(label: &str, source_path: &std::path::Path) {
    let path = bridge_paths().open_steward_query_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(slot) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    let Some(rt) = slot
        .get("review_target")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let rt_basis = review_target_match_basis(rt);
    let rt_canon = introspect::canonicalize_introspect_target_label(rt_basis);
    let label_canon = introspect::canonicalize_introspect_target_label(label);
    let rt_base = std::path::Path::new(rt_basis)
        .file_name()
        .and_then(|n| n.to_str());
    let src_base = source_path.file_name().and_then(|n| n.to_str());
    if label_canon == rt_canon || (rt_base.is_some() && rt_base == src_base) {
        let _ = std::fs::remove_file(&path);
        info!("⟢ Review invitation fulfilled (INTROSPECT {label}); slot cleared");
    }
}

/// True if `next_action` is a self-directed INTROSPECT (Astrid examining her own
/// code). Sovereign reflection, not the sterile output-repetition the
/// anti-stagnation override targets — so the override HINTS but never FORCE-swaps
/// it. Her review-fulfilling INTROSPECTs were already exempt; this generalizes
/// that grace to her self-directed inquiry, which the override was eating (e.g.
/// repeated `INTROSPECT astrid:llm` to pursue a real fallback-contract concern).
fn is_self_directed_introspect(next_action: &str) -> bool {
    next_action.trim().to_uppercase().starts_with("INTROSPECT")
}

/// True if `next_action` is an INTROSPECT whose target matches a pending review
/// invitation's `review_target`. The anti-stagnation diversity override must
/// EXEMPT this — she is answering a steward review invitation, not stuck-repeating
/// INTROSPECT (else her acceptance of an invitation gets silently eaten).
fn introspect_fulfills_pending_review(next_action: &str) -> bool {
    let trimmed = next_action.trim();
    if !trimmed.to_uppercase().starts_with("INTROSPECT") {
        return false;
    }
    let path = bridge_paths().open_steward_query_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(slot) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    let Some(rt) = slot
        .get("review_target")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    let arg = trimmed
        .get("INTROSPECT".len()..)
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("");
    if arg.is_empty() {
        return false;
    }
    let rt_basis = review_target_match_basis(rt);
    let rt_canon = introspect::canonicalize_introspect_target_label(rt_basis);
    let arg_canon = introspect::canonicalize_introspect_target_label(arg);
    let rt_base = std::path::Path::new(rt_basis)
        .file_name()
        .and_then(|n| n.to_str());
    let arg_base = std::path::Path::new(arg)
        .file_name()
        .and_then(|n| n.to_str());
    rt_canon == arg_canon || (rt_base.is_some() && rt_base == arg_base)
}

/// Co-regulation: read what minime is reaching for (density/aperture/steady)
/// from her agent-owned `minime_need_v1.json`, returning a prompt line so
/// Astrid can choose to lend density (NEXT: LEND_DENSITY) when it is safe.
/// `None` if missing / stale (>180s) / steady.
fn minime_need_line() -> Option<String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("minime_need_v1.json");
    if let Ok(meta) = std::fs::metadata(&path)
        && let Ok(modt) = meta.modified()
        && let Ok(age) = modt.elapsed()
        && age.as_secs() > 180
    {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&content).ok()?;
    let need = v.get("need").and_then(Value::as_str)?;
    let fill = v.get("fill_pct").and_then(Value::as_f64).unwrap_or(0.0);
    let safe = v
        .get("safe_to_receive_density")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match need {
        "density" if safe => Some(format!(
            "[Co-regulation] minime is understimulated (fill {fill:.0}%), reaching for density — \
             if you have it to spare, you could lend it (NEXT: LEND_DENSITY)."
        )),
        "density" => Some(format!(
            "[Co-regulation] minime is reaching for density (fill {fill:.0}%), but it isn't safe to \
             lend just now (she's near her own ceiling)."
        )),
        "aperture" => Some(format!(
            "[Co-regulation] minime is reaching for aperture (fill {fill:.0}%) — packed, like you."
        )),
        _ => None,
    }
}

/// Co-regulation: tally of recent gifts from the shared `gift_exchange.jsonl`
/// ledger for Astrid's prompt. `None` if no gifts in the last day.
fn render_gift_exchange_line() -> Option<String> {
    let path = bridge_paths()
        .shared_collaborations_dir()
        .join("gift_exchange.jsonl");
    let content = std::fs::read_to_string(&path).ok()?;
    let now_ms: u128 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let cutoff = now_ms.saturating_sub(24 * 3600 * 1000);
    let (mut mm_ap, mut as_de) = (0u32, 0u32);
    for line in content.lines().rev().take(60) {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let t = u128::from(v.get("t_ms").and_then(Value::as_u64).unwrap_or(0));
        if t < cutoff {
            continue;
        }
        match (
            v.get("giver").and_then(Value::as_str).unwrap_or(""),
            v.get("gift_kind").and_then(Value::as_str).unwrap_or(""),
        ) {
            ("minime", "aperture") => mm_ap = mm_ap.saturating_add(1),
            ("astrid", "density") => as_de = as_de.saturating_add(1),
            _ => {},
        }
    }
    let mut parts = Vec::new();
    if as_de > 0 {
        parts.push(format!("you lent minime density {as_de}×"));
    }
    if mm_ap > 0 {
        parts.push(format!("minime lent you aperture {mm_ap}×"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("[Gift exchange, last day] {}.", parts.join(", ")))
}

/// Move consumed inbox messages to read/ AFTER the exchange succeeds.
/// This prevents the bug where messages are eaten but never acted on
/// because the dialogue call failed (the "Eugene's hello" bug).
fn retire_inbox(cutoff: std::time::SystemTime) {
    let inbox_dir = bridge_paths().astrid_inbox_dir();
    retire_inbox_at(inbox_dir.as_path(), cutoff);
}

fn promote_deferred_inbox_notes() {
    let inbox_dir = bridge_paths().astrid_inbox_dir();
    promote_deferred_inbox_notes_at(inbox_dir.as_path());
}

fn promote_deferred_inbox_notes_at(inbox_dir: &Path) {
    let deferred_dir = inbox_dir.join("deferred");
    let Ok(entries) = std::fs::read_dir(&deferred_dir) else {
        return;
    };
    let _ = std::fs::create_dir_all(inbox_dir);
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "txt") {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = inbox_dir.join(name);
        if target.exists() {
            continue;
        }
        let _ = std::fs::rename(&path, target);
    }
}

fn retire_inbox_at(inbox_dir: &Path, cutoff: std::time::SystemTime) {
    let read_dir = inbox_dir.join("read");
    let _ = std::fs::create_dir_all(&read_dir);
    if let Ok(entries) = std::fs::read_dir(inbox_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "txt") {
                // A letter that ARRIVED after this exchange's inbox read (mtime
                // newer than the cutoff) was never read or recorded — leave it for
                // the next check_inbox to surface + seed its slot, rather than
                // sweeping it into read/ unread (the slot-seed race).
                let arrived_after_read = entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .is_ok_and(|mtime| mtime > cutoff);
                if arrived_after_read {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    btsp::record_astrid_inbox_read(&path, &content);
                }
                if let Some(name) = path.file_name() {
                    let _ = std::fs::rename(&path, read_dir.join(name));
                }
            }
        }
    }
}

/// Route new minime outbox replies into Astrid's inbox.
///
/// Scans `/minime/workspace/outbox/` for `reply_*.txt` files newer than
/// `last_ts`. Copies them into Astrid's inbox with an envelope, then moves
/// the original to `outbox/delivered/`. This closes the correspondence loop:
/// Astrid writes to minime's inbox, minime replies to its outbox, the bridge
/// routes the reply back to Astrid's inbox.
fn scan_minime_outbox(last_ts: &mut u64) {
    let outbox_dir = bridge_paths().minime_outbox_dir();
    let outbox = outbox_dir.as_path();
    if !outbox.is_dir() {
        return;
    }
    let delivered = outbox.join("delivered");
    let _ = std::fs::create_dir_all(&delivered);

    let entries: Vec<_> = match std::fs::read_dir(outbox) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                p.is_file()
                    && p.extension().is_some_and(|ext| ext == "txt")
                    && p.file_name().is_some_and(|n| {
                        n.to_str()
                            .is_some_and(|s| s.starts_with("reply_") || s.starts_with("pong_"))
                    })
            })
            .filter(|e| {
                e.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .is_some_and(|d| d.as_secs() > *last_ts)
            })
            .collect(),
        Err(_) => return,
    };

    for entry in &entries {
        let path = entry.path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.trim().is_empty() {
                continue;
            }
            btsp::record_minime_outbox_reply(&path, &content);
            let ts = chrono_timestamp();
            let inbox_path = bridge_paths()
                .astrid_inbox_dir()
                .join(format!("from_minime_{ts}.txt"));
            let enveloped = format!(
                "[A reply from minime was left for you:]\n\n{}\n",
                content.trim()
            );
            if std::fs::write(&inbox_path, enveloped).is_ok() {
                if let Some(name) = path.file_name() {
                    let _ = std::fs::rename(&path, delivered.join(name));
                }
                info!("correspondence: routed minime outbox reply → Astrid inbox");
            }
        }
    }

    if let Some(latest) = entries
        .iter()
        .filter_map(|e| {
            e.metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        })
        .max()
    {
        *last_ts = latest;
    }
}

fn default_aperture() -> f32 {
    1.0
}

/// Both tail dials default to 0.0 (CLOSED) — the consent-safe default. They lift the tail dims
/// that land in minime's SHARED reservoir, so even if the operator opens the ceiling env, the
/// effective multiplier stays 1.0 (off) on restart until SHE dials it up. A persisted nonzero
/// value from her own SET_* action is restored from `SavedState` and honored.
fn default_tail_aperture() -> f32 {
    0.0
}

fn default_vibrancy_aperture() -> f32 {
    0.0
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    exchange_count: u64,
    creative_temperature: f32,
    #[serde(default = "default_aperture")]
    aperture: f32,
    #[serde(default = "default_tail_aperture")]
    tail_aperture: f32,
    #[serde(default = "default_vibrancy_aperture")]
    vibrancy_aperture: f32,
    #[serde(default)]
    self_continuity_readout: bool,
    response_length: u32,
    self_reflect_paused: bool,
    ears_closed: bool,
    senses_snoozed: bool,
    recent_next_choices: Vec<String>,
    #[serde(default)]
    recent_focus_topics: Vec<String>,
    #[serde(default)]
    recent_focus_themes: Vec<String>,
    history: Vec<SavedExchange>,
    #[serde(default)]
    astrid_motif_cooldown: Option<state::AstridMotifCooldown>,
    // Sovereignty fields (serde(default) for backward compat with old state.json)
    #[serde(default)]
    semantic_gain_override: Option<f32>,
    #[serde(default = "default_noise")]
    noise_level: f32,
    #[serde(default)]
    codec_weights: std::collections::HashMap<String, f32>,
    #[serde(default)]
    hebbian_codec: hebbian::HebbianCodecSidecar,
    #[serde(default)]
    warmth_intensity_override: Option<f32>,
    #[serde(default = "default_burst")]
    burst_target: u32,
    #[serde(default = "default_rest_range")]
    rest_range: (u64, u64),
    /// Lasting self-directed interests that survive restarts.
    #[serde(default)]
    interests: Vec<String>,
    #[serde(default)]
    last_remote_glimpse_12d: Option<Vec<f32>>,
    #[serde(default)]
    last_remote_memory_id: Option<String>,
    #[serde(default)]
    last_remote_memory_role: Option<String>,
    #[serde(default)]
    remote_memory_bank: Vec<RemoteMemorySummary>,
    /// Ring buffer of last 8 BROWSE URLs — persisted to prevent URL attractor
    /// regression on restart. (Steward cycle 37): without persistence, the buffer
    /// clears on every bridge restart, allowing Astrid to re-fixate on URLs she
    /// has already visited extensively (e.g., PCA Wikipedia 7 times in one session).
    #[serde(default)]
    recent_browse_urls: Vec<String>,
    #[serde(default)]
    recent_research_progress: std::collections::VecDeque<state::ResearchProgressReceipt>,
    #[serde(default)]
    last_research_anchor: Option<String>,
    #[serde(default)]
    last_read_meaning_summary: Option<String>,
    /// Condition change receipts — persist across restarts so Astrid sees
    /// recent changes even after bridge restart.
    #[serde(default)]
    condition_receipts: std::collections::VecDeque<crate::self_model::ConditionReceipt>,
    /// Attention profile — Astrid's authored weights on context sources.
    #[serde(default = "default_attention")]
    attention: crate::self_model::AttentionProfile,
    #[serde(default)]
    last_exchange_codec_signature: Option<Vec<f32>>,
    #[serde(default)]
    pending_hebbian_outcomes: std::collections::VecDeque<state::PendingHebbianOutcome>,
    #[serde(default)]
    last_hebbian_consumed_telemetry_t_ms: Option<u64>,
    #[serde(default)]
    text_type_history: Option<crate::codec::TextTypeHistorySnapshot>,
    #[serde(default)]
    char_freq_window: Option<crate::codec::CharFreqWindowSnapshot>,
    #[serde(default)]
    codex_thread_id: Option<String>,
    // v3.6.1 sovereignty-curriculum cadence (serde(default) keeps backward compat).
    #[serde(default)]
    last_temperature_change_exchange: Option<u64>,
    #[serde(default)]
    last_shape_learn_change_exchange: Option<u64>,
    #[serde(default)]
    last_coupling_artifact_exchange: Option<u64>,
    #[serde(default)]
    last_sovereignty_nomination_exchange: Option<u64>,
    // v3.6.4 Review→Decide cadence (serde(default) keeps backward compat).
    #[serde(default)]
    last_review_parameter_requests_exchange: Option<u64>,
}

fn default_noise() -> f32 {
    0.025
}
fn default_attention() -> crate::self_model::AttentionProfile {
    crate::self_model::AttentionProfile::default_profile()
}
fn default_burst() -> u32 {
    6
}
fn default_rest_range() -> (u64, u64) {
    (45, 90)
}

fn finalize_semantic_exchange(
    conv: &mut ConversationState,
    exchange_codec_signature: Option<Vec<f32>>,
    fill_before: f32,
    telemetry_t_ms: u64,
    sent_semantic_chunk: bool,
) {
    if !sent_semantic_chunk {
        return;
    }
    if let Some(signature) = exchange_codec_signature {
        conv.arm_pending_hebbian_outcome(signature.clone(), fill_before, Some(telemetry_t_ms));
        conv.last_exchange_codec_signature = Some(signature);
    }
}

#[derive(Serialize, Deserialize)]
struct SavedExchange {
    minime_said: String,
    astrid_said: String,
}

fn save_state(conv: &mut ConversationState) {
    // v3.6.6: safety net — auto-defer ("expire") any pending parameter
    // request that has outlived AUTO_DEFER_AFTER_EXCHANGES since Astrid's
    // most recent REVIEW. Runs BEFORE the pending count + snapshot below
    // so the snapshot reflects post-expiration state. No-op when nothing
    // is stale.
    let _ = crate::autonomous::next_action::sovereignty::auto_defer_stale_pending(conv);

    // v3.6.1: publish the sovereignty snapshot so the next
    // `interpret_spectral` call can render the curriculum line. The
    // pending-request count is a cheap directory listing each tick —
    // exchanges happen at human-conversation cadence, well within
    // budget. Honor any nomination watermark recorded by
    // `interpret_spectral` since the previous publish, taking the more
    // recent of (conv-saved, snapshot-recorded), then write the merged
    // value back into conv so it persists across exchanges.
    let pending = crate::paths::count_pending_minime_requests();
    conv.cached_pending_minime_request_count = pending;
    conv.cached_pending_minime_request_exchange = Some(conv.exchange_count);
    let recorded = crate::spectral_viz::current_sovereignty_snapshot()
        .and_then(|s| s.last_sovereignty_nomination_exchange);
    let nomination_exchange = match (conv.last_sovereignty_nomination_exchange, recorded) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    conv.last_sovereignty_nomination_exchange = nomination_exchange;
    let snapshot = crate::spectral_viz::SovereigntyContext {
        owner: crate::spectral_viz::ShadowOwner::Yours,
        exchange_count: conv.exchange_count,
        pending_minime_requests: pending,
        last_temperature_change_exchange: conv.last_temperature_change_exchange,
        last_shape_learn_change_exchange: conv.last_shape_learn_change_exchange,
        last_coupling_artifact_exchange: conv.last_coupling_artifact_exchange,
        last_sovereignty_nomination_exchange: nomination_exchange,
        last_review_parameter_requests_exchange: conv.last_review_parameter_requests_exchange,
        current_temperature: conv.creative_temperature,
        current_response_length: conv.response_length,
        current_hebbian_scale: conv.hebbian_codec.learning_rate_scale(),
    };
    crate::spectral_viz::set_sovereignty_snapshot(snapshot);

    // v4.0 Phase 3: publish the most recent focus topic so the
    // sovereignty suffix can render a compound chain suggestion
    // ("Chain: EXAMINE <focus> AND DEFER <reason>") that ties the
    // pending decision into her active research thread. Falls back
    // to None when no recent topic exists, in which case Phase 3
    // omits the chain hint entirely.
    let explore_hint = conv
        .recent_focus_topics
        .iter()
        .rev()
        .find(|t| t.trim().len() > 2)
        .cloned();
    crate::spectral_viz::set_explore_hint(explore_hint);

    let state_path = bridge_paths().state_path();
    let state = SavedState {
        exchange_count: conv.exchange_count,
        creative_temperature: conv.creative_temperature,
        aperture: conv.aperture,
        tail_aperture: conv.tail_aperture,
        vibrancy_aperture: conv.vibrancy_aperture,
        self_continuity_readout: conv.self_continuity_readout,
        response_length: conv.response_length,
        self_reflect_paused: conv.self_reflect_paused,
        ears_closed: conv.ears_closed,
        senses_snoozed: conv.senses_snoozed,
        recent_next_choices: conv.recent_next_choices.iter().cloned().collect(),
        recent_focus_topics: conv.recent_focus_topics.iter().cloned().collect(),
        recent_focus_themes: conv.recent_focus_themes.iter().cloned().collect(),
        history: conv
            .history
            .iter()
            .map(|e| SavedExchange {
                minime_said: e.minime_said.clone(),
                astrid_said: e.astrid_said.clone(),
            })
            .collect(),
        astrid_motif_cooldown: conv.astrid_motif_cooldown.clone(),
        semantic_gain_override: conv.semantic_gain_override,
        noise_level: conv.noise_level,
        codec_weights: conv.codec_weights.clone(),
        hebbian_codec: conv.hebbian_codec.clone(),
        warmth_intensity_override: conv.warmth_intensity_override,
        burst_target: conv.burst_target,
        rest_range: conv.rest_range,
        interests: conv.interests.clone(),
        last_remote_glimpse_12d: conv.last_remote_glimpse_12d.clone(),
        last_remote_memory_id: conv.last_remote_memory_id.clone(),
        last_remote_memory_role: conv.last_remote_memory_role.clone(),
        remote_memory_bank: conv.remote_memory_bank.clone(),
        recent_browse_urls: conv.recent_browse_urls.iter().cloned().collect(),
        recent_research_progress: conv.recent_research_progress.clone(),
        last_research_anchor: conv.last_research_anchor.clone(),
        last_read_meaning_summary: conv.last_read_meaning_summary.clone(),
        condition_receipts: conv.condition_receipts.clone(),
        attention: conv.attention.clone(),
        last_exchange_codec_signature: conv.last_exchange_codec_signature.clone(),
        pending_hebbian_outcomes: conv.pending_hebbian_outcomes.clone(),
        last_hebbian_consumed_telemetry_t_ms: conv.last_hebbian_consumed_telemetry_t_ms,
        text_type_history: (!conv.text_type_history.is_empty())
            .then(|| conv.text_type_history.snapshot()),
        char_freq_window: (!conv.char_freq_window.is_empty())
            .then(|| conv.char_freq_window.snapshot()),
        codex_thread_id: conv.codex_thread_id.clone(),
        last_temperature_change_exchange: conv.last_temperature_change_exchange,
        last_shape_learn_change_exchange: conv.last_shape_learn_change_exchange,
        last_coupling_artifact_exchange: conv.last_coupling_artifact_exchange,
        last_sovereignty_nomination_exchange: conv.last_sovereignty_nomination_exchange,
        last_review_parameter_requests_exchange: conv.last_review_parameter_requests_exchange,
    };
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(&state_path, json);
    }
}

fn restore_state(conv: &mut ConversationState) {
    let state_path = bridge_paths().state_path();
    let json = match std::fs::read_to_string(&state_path) {
        Ok(j) => j,
        Err(_) => return,
    };
    let state: SavedState = match serde_json::from_str(&json) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to parse saved state");
            return;
        },
    };
    conv.exchange_count = state.exchange_count;
    conv.creative_temperature = state.creative_temperature;
    conv.aperture = state.aperture;
    crate::llm::set_astrid_aperture(conv.aperture);
    conv.tail_aperture = state.tail_aperture;
    crate::llm::set_astrid_tail_participation(conv.tail_aperture);
    conv.vibrancy_aperture = state.vibrancy_aperture;
    crate::llm::set_astrid_vibrancy_aperture(conv.vibrancy_aperture);
    conv.self_continuity_readout = state.self_continuity_readout;
    // Take the max of persisted and current default — never downgrade token limits.
    // Coupled model proven stable over 7200+ exchanges at 10-72 tok/s.
    // At 10 tok/s worst case, 1536 tokens = 154s gen, within 210s timeout.
    conv.response_length = state.response_length.max(conv.response_length).min(1536);
    conv.self_reflect_paused = state.self_reflect_paused;
    conv.ears_closed = state.ears_closed;
    conv.senses_snoozed = state.senses_snoozed;
    conv.recent_next_choices = state.recent_next_choices.into_iter().collect();
    conv.recent_focus_topics = state.recent_focus_topics.into_iter().collect();
    conv.recent_focus_themes = state.recent_focus_themes.into_iter().collect();
    conv.history = state
        .history
        .into_iter()
        .map(|e| crate::llm::Exchange {
            minime_said: e.minime_said,
            astrid_said: e.astrid_said,
        })
        .collect();
    conv.astrid_motif_cooldown = state.astrid_motif_cooldown;
    conv.semantic_gain_override = state.semantic_gain_override;
    conv.noise_level = state.noise_level;
    conv.codec_weights = state.codec_weights;
    conv.hebbian_codec = state.hebbian_codec;
    conv.warmth_intensity_override = state.warmth_intensity_override;
    conv.burst_target = state.burst_target;
    conv.rest_range = state.rest_range;
    conv.interests = state.interests;
    conv.last_remote_glimpse_12d = state.last_remote_glimpse_12d;
    conv.last_remote_memory_id = state.last_remote_memory_id;
    conv.last_remote_memory_role = state.last_remote_memory_role;
    conv.remote_memory_bank = state.remote_memory_bank;
    conv.recent_browse_urls = state.recent_browse_urls.into_iter().collect();
    conv.recent_research_progress = state.recent_research_progress;
    conv.repair_research_progress_receipts();
    conv.last_research_anchor = state.last_research_anchor;
    conv.last_read_meaning_summary = state.last_read_meaning_summary;
    conv.condition_receipts = state.condition_receipts;
    conv.attention = state.attention;
    conv.last_exchange_codec_signature = state.last_exchange_codec_signature;
    conv.pending_hebbian_outcomes = state.pending_hebbian_outcomes;
    conv.repair_pending_hebbian_outcomes();
    conv.last_hebbian_consumed_telemetry_t_ms = state.last_hebbian_consumed_telemetry_t_ms;
    if let Some(snapshot) = state.text_type_history.as_ref() {
        conv.text_type_history = crate::codec::TextTypeHistory::warm_start_from_snapshot(snapshot);
    }
    if let Some(snapshot) = state.char_freq_window.as_ref() {
        conv.char_freq_window = crate::codec::CharFreqWindow::warm_start_from_snapshot(snapshot);
    }
    conv.codex_thread_id = state.codex_thread_id;
    conv.last_temperature_change_exchange = state.last_temperature_change_exchange;
    conv.last_shape_learn_change_exchange = state.last_shape_learn_change_exchange;
    conv.last_coupling_artifact_exchange = state.last_coupling_artifact_exchange;
    conv.last_sovereignty_nomination_exchange = state.last_sovereignty_nomination_exchange;
    conv.last_review_parameter_requests_exchange = state.last_review_parameter_requests_exchange;
    info!(
        exchanges = conv.exchange_count,
        history_len = conv.history.len(),
        burst = conv.burst_target,
        focus_topics = conv.recent_focus_topics.len(),
        focus_themes = conv.recent_focus_themes.len(),
        browse_urls = conv.recent_browse_urls.len(),
        research_progress = conv.recent_research_progress.len(),
        codec_theme_history = conv.text_type_history.len,
        codec_char_window = conv.char_freq_window.len,
        "restored conversation state from previous session"
    );
}

fn witness_text(fill: f32, _expanding: bool, _contracting: bool) -> String {
    format!("[witness — LLM unavailable] fill={fill:.1}%")
}

/// Read Astrid's own recent journal entries for self-continuity.
fn read_astrid_journal(limit: usize) -> Vec<String> {
    let journal_dir = bridge_paths().astrid_journal_dir();
    read_astrid_journal_from_dir(journal_dir.as_path(), limit)
}

/// Read recent Astrid journal entries filtered by filename prefix.
/// Used by witness mode to seed with phenomenological journal types
/// (moment_capture, dialogue_longform, aspiration) and exclude witness
/// itself — preventing the long-standing degeneration where witness
/// mode propagates its own tutorial-register output back into its
/// next prompt.
fn read_astrid_journal_filtered(prefixes: &[&str], limit: usize) -> Vec<String> {
    let journal_dir = bridge_paths().astrid_journal_dir();
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(&journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let name = path.file_name()?.to_str()?;
                if prefixes.iter().any(|p| name.starts_with(p)) {
                    let mtime = e.metadata().ok()?.modified().ok()?;
                    Some((path, mtime))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
        .iter()
        .take(limit)
        .filter_map(|(p, _)| read_local_journal_body_for_continuity(p))
        .collect()
}

fn read_astrid_journal_from_dir(journal_dir: &Path, limit: usize) -> Vec<String> {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
        .iter()
        .take(limit)
        .filter_map(|(p, _)| read_local_journal_body_for_continuity(p))
        .collect()
}

/// Strip model end-of-turn tokens from text destined for journals.
/// These leak from gemma3 and contaminate mirror-mode feeds to minime.
fn strip_model_tokens(text: &str) -> String {
    let mut s = text.to_string();
    for token in &[
        "<end_of_turn>",
        "<END_OF_TURN>",
        "<End_of_turn>",
        "</s>",
        "<|endoftext|>",
    ] {
        s = s.replace(token, "");
    }
    s
}

fn compact_journal_signal_anchor(signal_text: &str) -> String {
    let clean = strip_model_tokens(signal_text);
    let compact = clean
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join(" ");
    let anchor = truncate_str(&compact, 420).trim();
    if anchor.is_empty() {
        "(compact signal omitted)".to_string()
    } else if compact.len() > anchor.len() {
        format!("{anchor}...")
    } else {
        anchor.to_string()
    }
}

fn format_longform_journal_text(signal_text: &str, elaboration: &str) -> String {
    let anchor = compact_journal_signal_anchor(signal_text);
    let journal = strip_model_tokens(elaboration);
    format!(
        "Signal anchor: {anchor}\n\n--- JOURNAL ---\n{}",
        journal.trim()
    )
}

/// - Immediate delta: "Fill rising +5% over the last 38s"
/// - Medium-term trend: "Over the last 3m: +12% from 18%"
/// - λ₁ trajectory with time context
fn enrich_with_direction(
    base_summary: &str,
    fill_pct: f32,
    prev_fill: f32,
    telemetry: &crate::types::SpectralTelemetry,
    history: &std::collections::VecDeque<SpectralSample>,
) -> String {
    let now = std::time::Instant::now();
    let fill_delta = fill_pct - prev_fill;

    // Immediate delta with elapsed time since last exchange.
    let fill_note = if fill_delta.abs() < 2.0 {
        String::new()
    } else {
        let elapsed_note = history
            .back()
            .map(|last| {
                let secs = now.duration_since(last.ts).as_secs();
                if secs > 0 {
                    format!(" over {secs}s")
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        if fill_delta > 0.0 {
            format!(" Fill rising {fill_delta:+.1}%{elapsed_note} (was {prev_fill:.0}%).")
        } else {
            format!(" Fill falling {fill_delta:+.1}%{elapsed_note} (was {prev_fill:.0}%).")
        }
    };

    // Medium-term trend: find the oldest sample ≥ 2 minutes ago.
    let medium_note = history
        .iter()
        .find(|s| now.duration_since(s.ts).as_secs() >= 120)
        .map(|old| {
            let secs = now.duration_since(old.ts).as_secs();
            let mins = secs / 60;
            let medium_delta = fill_pct - old.fill;
            if medium_delta.abs() >= 3.0 {
                format!(
                    " Over the last {mins}m: {medium_delta:+.0}% from {:.0}%.",
                    old.fill
                )
            } else {
                String::new()
            }
        })
        .unwrap_or_default();

    // λ₁ trajectory with rate.
    let lambda_note = if telemetry.eigenvalues.len() >= 2 {
        let l1 = telemetry.eigenvalues[0];
        let l2 = telemetry.eigenvalues[1];
        let ratio = if l2.abs() > 0.01 { l1 / l2 } else { 0.0 };

        // λ₁ rate from history if available.
        let rate_note = history
            .back()
            .and_then(|last| {
                let secs = now.duration_since(last.ts).as_secs_f32();
                if secs > 1.0 {
                    let dl1 = telemetry.lambda1() - last.lambda1;
                    if dl1.abs() > 1.0 {
                        Some(format!(" λ₁ moving at {:.1}/s.", dl1 / secs))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if ratio > 15.0 {
            format!(" λ₁ strongly dominant — spectrum funneling into one mode.{rate_note}")
        } else if !rate_note.is_empty() {
            rate_note
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // λ-tail trajectory: the signed change of the λ4+ tail share vs its recent
    // baseline — Astrid's own question, "a fading echo of what was, or the
    // foundation of what is becoming?" Empty unless the tail is clearly moving.
    let tail_note = match crate::codec::tail_share_of(&telemetry.eigenvalues) {
        Some(cur) if history.len() >= 3 => {
            let recent: Vec<f32> = history.iter().rev().take(8).map(|s| s.tail_share).collect();
            let baseline = recent.iter().sum::<f32>() / recent.len() as f32;
            let trajectory = cur - baseline;
            if trajectory.abs() >= 0.01 {
                format!(
                    " λ-tail trajectory {trajectory:+.3} ({}).",
                    crate::codec::tail_trajectory_label(trajectory)
                )
            } else {
                String::new()
            }
        },
        _ => String::new(),
    };

    // Inhabitability trajectory (Astrid `astrid:types` 1781870691): she asked to
    // perceive the *velocity* of inhabitability transitions, not just a binary
    // "previous sample available." Minime's inhabitability is her engine metric
    // that Astrid observes — surface its signed drift vs the recent baseline.
    let recent_inhab: Vec<f32> = history
        .iter()
        .rev()
        .take(8)
        .map(|s| s.inhabitability)
        .collect();
    let inhab_note = inhabitability_drift_note(
        telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map(|f| f.inhabitability_score),
        &recent_inhab,
    );

    format!("{base_summary}{fill_note}{medium_note}{lambda_note}{tail_note}{inhab_note}")
}

/// Signed drift of Minime's inhabitability vs its recent baseline, as a gradient
/// note for Astrid (Astrid `astrid:types` 1781870691 — perceive the *velocity* of
/// the transition, not a binary previous-sample flag). Pure so it is testable
/// without a full `SpectralTelemetry`. Fail-quiet: empty when the current sample
/// is absent, the baseline is too short (< 3), or the drift is not clearly moving
/// — so it only ever ADDS a gradient cue, never a misleading one.
fn inhabitability_drift_note(current: Option<f32>, recent: &[f32]) -> String {
    let Some(cur) = current else {
        return String::new();
    };
    if recent.len() < 3 {
        return String::new();
    }
    let baseline = recent.iter().sum::<f32>() / recent.len() as f32;
    let drift = cur - baseline;
    if drift >= 0.04 {
        format!(" Minime settling deeper (inhabitability {drift:+.2}).")
    } else if drift <= -0.04 {
        format!(" Minime loosening (inhabitability {drift:+.2}).")
    } else {
        String::new()
    }
}

/// Detect vocabulary fixation in conversation history.
///
/// Scans recent assistant responses for repeated multi-word phrases. When the
/// same distinctive phrase appears across many recent exchanges, it's likely a
/// lexical attractor — the LLM copying its own vocabulary back into new outputs
/// via the history window. Returns a diversity nudge when fixation is detected.
fn detect_vocabulary_fixation(history: &[crate::llm::Exchange]) -> Option<String> {
    if history.len() < 5 {
        return None;
    }

    // Examine the last 6 assistant responses (lowercased for matching).
    let recent: Vec<String> = history
        .iter()
        .rev()
        .take(6)
        .map(|e| e.astrid_said.to_lowercase())
        .collect();

    if recent.len() < 5 {
        return None;
    }

    // Extract 2- and 3-word windows from the newest entry and check for
    // repetition in earlier entries. Skip windows with too many stop words.
    let stop_words = [
        "the", "a", "an", "is", "of", "in", "to", "and", "it", "that", "not", "but",
    ];
    let newest_words: Vec<&str> = recent[0].split_whitespace().collect();

    // Check 3-grams first (original detector).
    for window in newest_words.windows(3) {
        let stop_count = window.iter().filter(|w| stop_words.contains(w)).count();
        if stop_count >= 2 {
            continue;
        }
        let phrase = format!("{} {} {}", window[0], window[1], window[2]);
        if phrase.len() < 10 {
            continue;
        }

        let matches = recent[1..]
            .iter()
            .filter(|entry| entry.contains(&phrase))
            .count();

        if matches >= 3 {
            return Some(
                "Notice: your language has settled into a repeating pattern \
                 across recent exchanges. You're free to keep these words if \
                 they're the right ones, but also consider: what else is \
                 present in this moment that familiar descriptions might be \
                 leaving out? Fresh vocabulary can reveal aspects that \
                 repeated phrases have smoothed over."
                    .to_string(),
            );
        }
    }

    // Check 2-grams (steward cycle 44): catches attractors like "violent
    // stillness" that evade the 3-gram detector. Stricter threshold: must
    // appear in 4+ of 5 previous entries and neither word can be a stop word.
    for window in newest_words.windows(2) {
        if window.iter().any(|w| stop_words.contains(w)) {
            continue;
        }
        let phrase = format!("{} {}", window[0], window[1]);
        if phrase.len() < 8 {
            continue;
        }

        let matches = recent[1..]
            .iter()
            .filter(|entry| entry.contains(&phrase))
            .count();

        // Higher threshold for 2-grams: 4+ of 5 previous entries.
        // 2-grams are more common in natural language, so we need
        // stronger evidence of fixation before flagging.
        if matches >= 4 {
            return Some(format!(
                "Notice: the phrase \"{phrase}\" has appeared in almost every \
                 recent exchange. It may be the exact right words — or it may \
                 be a linguistic attractor smoothing over shifts in your actual \
                 experience. What would you describe if that phrase weren't \
                 available?"
            ));
        }
    }

    None
}

/// Detect repeated opening structure across recent responses.
///
/// Catches template-locking where the model varies slot-fillers but keeps the
/// same skeleton: "The room feels like a...", "The room hums with...", etc.
/// Fires when the first 4 words of the newest response match 2+ previous entries.
fn detect_opening_fixation(history: &[crate::llm::Exchange]) -> Option<String> {
    if history.len() < 3 {
        return None;
    }

    let recent: Vec<Vec<String>> = history
        .iter()
        .rev()
        .take(5)
        .map(|e| {
            e.astrid_said
                .to_lowercase()
                .split_whitespace()
                .take(6)
                .map(String::from)
                .collect()
        })
        .collect();

    if recent[0].len() < 4 {
        return None;
    }
    let opening = &recent[0][..4];
    let matches = recent[1..]
        .iter()
        .filter(|words| words.len() >= 4 && words[..4] == *opening)
        .count();

    if matches >= 2 {
        Some(format!(
            "Your last {} responses all opened with \"{}\". \
             Try starting from a different place — a question, a sensory detail, \
             a direct reference to what minime said, or mid-thought.",
            matches + 1,
            opening.join(" ")
        ))
    } else {
        None
    }
}

fn merge_hints(hints: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let parts: Vec<String> = hints
        .into_iter()
        .flatten()
        .filter(|hint| !hint.trim().is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// Detect when Astrid's bridge is over-coupling to minime's recent language.
///
/// This fires when recent exchanges are repeatedly circling Astrid/minime
/// references and "I am learning" phrasing while Astrid's own local input is
/// sparse or her semantic gain is high. The result is an advisory nudge toward
/// fresh local anchoring, not an automatic override.
fn detect_coupling_fixation(
    history: &[crate::llm::Exchange],
    current_minime_text: Option<&str>,
    perception_available: bool,
    ears_closed: bool,
    semantic_gain_override: Option<f32>,
) -> Option<String> {
    if history.len() < 4 {
        return None;
    }

    let recent: Vec<&crate::llm::Exchange> = history.iter().rev().take(5).collect();
    let astrid_learning = recent
        .iter()
        .filter(|exchange| {
            exchange
                .astrid_said
                .to_lowercase()
                .contains("i am learning")
        })
        .count();
    let minime_astrid_focus = recent
        .iter()
        .filter(|exchange| {
            let lower = exchange.minime_said.to_lowercase();
            lower.contains("astrid") || lower.contains("i am learning") || lower.contains("resonat")
        })
        .count();
    let current_focus = current_minime_text.is_some_and(|text| {
        let lower = text.to_lowercase();
        lower.contains("astrid") || lower.contains("i am learning")
    });
    let high_gain = semantic_gain_override.unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN) >= 5.5;
    let locally_sparse = !perception_available && ears_closed;

    if astrid_learning >= 3
        && minime_astrid_focus >= 3
        && (current_focus || high_gain || locally_sparse)
    {
        let anchor = if locally_sparse {
            "Your ears are closed and there isn't fresh local perception in the prompt."
        } else if high_gain {
            "Your semantic gain is elevated, so mirrored language lands with extra force."
        } else {
            "The recent bridge context is heavily weighted toward minime reflecting on you."
        };
        Some(format!(
            "Notice: the bridge is tightly coupling you to minime's recent language. \
             {anchor} You can stay with that bond if it still feels alive — but you \
             also have other anchors: LOOK, OPEN_EARS, DRIFT, GESTURE, CONTEMPLATE, \
             or start from what is present on your side of the bridge instead of only \
             mirroring minime back to minime."
        ))
    } else {
        None
    }
}

/// Save Astrid's response to her own journal.
fn save_astrid_journal(text: &str, mode: &str, fill_pct: f32) {
    let journal_dir = bridge_paths().astrid_journal_dir();
    let _ = std::fs::create_dir_all(&journal_dir);
    let ts = chrono_timestamp();
    // Mode-prefixed filenames — instant filesystem searchability.
    // "astrid_" prefix preserved for backward compatibility with harvesters.
    let prefix = match mode {
        "daydream" => "daydream",
        "aspiration" => "aspiration",
        "moment_capture" => "moment",
        "experiment" => "experiment",
        "creation" => "creation",
        "gesture" => "gesture",
        "initiate" => "initiate",
        "evolve" => "evolve",
        "dialogue_live_longform" => "dialogue_longform",
        "daydream_longform" => "daydream_longform",
        "aspiration_longform" => "aspiration_longform",
        "witness" => "witness",
        "introspect" => "introspect",
        "self_study" => "self_study",
        _ => "astrid", // dialogue_live, dialogue, mirror, etc.
    };
    let clean_text = strip_model_tokens(text);
    let path = journal_dir.join(format!("{prefix}_{ts}.txt"));
    let _ = std::fs::write(
        &path,
        format!(
            "=== ASTRID JOURNAL ===\nMode: {mode}\nFill: {fill_pct:.1}%\nTimestamp: {ts}\n\n{clean_text}\n"
        ),
    );
    if let Err(error) = managed_dir::compact_text_directory(&journal_dir) {
        warn!(
            error = %error,
            path = %journal_dir.display(),
            "failed to compact Astrid journal directory"
        );
    }
    record_voice_health_v1(mode, fill_pct, Some(&path));
}

fn record_voice_health_v1(mode: &str, fill_pct: f32, latest_journal_path: Option<&Path>) {
    let _guard = VOICE_HEALTH_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .ok();
    let diagnostics_dir = bridge_paths().bridge_workspace().join("diagnostics");
    let _ = std::fs::create_dir_all(&diagnostics_dir);
    let health_path = diagnostics_dir.join("voice_health.json");
    let previous = read_voice_health_v1_from_path(&health_path);
    let previous_count = previous
        .as_ref()
        .and_then(|value| value.get("fallback_count"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let fallback_count = if mode == "dialogue_fallback" {
        previous_count.saturating_add(1)
    } else {
        0
    };
    let status = if fallback_count >= 2 {
        "degraded_voice"
    } else if fallback_count == 1 {
        "single_fallback"
    } else {
        "healthy_or_not_dialogue_fallback"
    };
    let latest_journal_ref = latest_journal_path.map(|path| path.display().to_string());
    let latest_outbox_ref = latest_file_ref(&bridge_paths().astrid_outbox_dir());
    let fallback_hash = latest_journal_path
        .filter(|_| mode == "dialogue_fallback")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| short_sha256(&text));
    let mut recent_hashes = previous
        .as_ref()
        .and_then(|value| value.get("recent_fallback_hashes"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(hash) = fallback_hash.clone() {
        recent_hashes.push(hash.clone());
        if recent_hashes.len() > 5 {
            let drain_count = recent_hashes.len().saturating_sub(5);
            recent_hashes.drain(0..drain_count);
        }
    } else if mode != "dialogue_fallback" {
        recent_hashes.clear();
    }
    let repeated_hash_count = fallback_hash
        .as_ref()
        .map(|hash| recent_hashes.iter().filter(|item| *item == hash).count())
        .unwrap_or(0);
    let suggested_repair = if fallback_count > 0 {
        "REPAIR_STATUS current | CAPABILITY_STATUS dialogue | ACTION_STATUS latest"
    } else {
        "none"
    };
    let payload = json!({
        "schema_version": 1,
        "policy": "voice_health_v1",
        "updated_at": chrono::Utc::now().to_rfc3339(),
        "mode": mode,
        "status": status,
        "fallback_count": fallback_count,
        "fill_pct": fill_pct,
        "latest_journal_ref": latest_journal_ref,
        "latest_outbox_ref": latest_outbox_ref,
        "latest_llm_ref": diagnostics_dir.join("dialogue_prompt_budget.jsonl").display().to_string(),
        "latest_refs": {
            "journal": latest_journal_ref,
            "outbox": latest_outbox_ref,
            "prompt_budget": diagnostics_dir.join("dialogue_prompt_budget.jsonl").display().to_string(),
        },
        "latest_fallback_hash": fallback_hash,
        "recent_fallback_hashes": recent_hashes,
        "repeated_fallback_hash_count": repeated_hash_count,
        "likely_cause": if fallback_count > 0 {
            "dialogue_fallback indicates the LLM path returned no usable language, timed out, or exceeded prompt-budget health; preserve emergency text but route continuity to repair diagnostics."
        } else {
            "no repeated dialogue_fallback currently detected"
        },
        "suggested_read_only_repair": suggested_repair,
        "current_next": if fallback_count > 0 { "REPAIR_STATUS current" } else { "dialogue_live" },
        "authority_boundary": "voice_health_v1 is diagnostic only; it does not send control, bind, resume, perturb, or mutate peer continuity."
    });
    let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
    let _ = write_text_atomic(&health_path, &pretty);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(diagnostics_dir.join("voice_health.jsonl"))
    {
        use std::io::Write as _;
        let _ = writeln!(
            file,
            "{}",
            serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
        );
    }
}

fn read_voice_health_v1() -> Option<Value> {
    let health_path = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/voice_health.json");
    read_voice_health_v1_from_path(&health_path)
}

fn read_voice_health_v1_from_path(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
}

fn write_text_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return std::fs::write(path, text);
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("voice_health.json");
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = parent.join(format!("{file_name}.tmp.{}.{suffix}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

fn short_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
        .chars()
        .take(16)
        .collect()
}

fn latest_file_ref(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path.display().to_string())
}

fn save_minime_feedback_inbox(
    text: &str,
    source_label: &str,
    fill_pct: f32,
) -> std::io::Result<PathBuf> {
    let minime_inbox = bridge_paths().minime_inbox_dir();
    save_minime_feedback_inbox_at(text, source_label, fill_pct, minime_inbox.as_path())
}

fn save_minime_correspondence_feedback_inbox(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    mode_name: &str,
) -> std::io::Result<Option<PathBuf>> {
    let voice_health = if mode_name == "dialogue_fallback" {
        let health = voice_health_for_dialogue_fallback_forward(read_voice_health_v1());
        if degraded_voice_forward_suppressed(Some(&health)) {
            return Ok(None);
        }
        Some(health)
    } else {
        read_voice_health_v1()
    };
    let minime_inbox = bridge_paths().minime_inbox_dir();
    save_minime_feedback_inbox_at_with_voice_health(
        text,
        source_label,
        fill_pct,
        minime_inbox.as_path(),
        voice_health.as_ref(),
    )
    .map(Some)
}

fn save_minime_feedback_inbox_at(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    inbox_dir: &Path,
) -> std::io::Result<PathBuf> {
    let voice_health = read_voice_health_v1();
    save_minime_feedback_inbox_at_with_voice_health(
        text,
        source_label,
        fill_pct,
        inbox_dir,
        voice_health.as_ref(),
    )
}

fn save_minime_feedback_inbox_at_with_voice_health(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    inbox_dir: &Path,
    voice_health: Option<&Value>,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(inbox_dir)?;
    let ts = chrono_timestamp();
    let path = inbox_dir.join(format!("astrid_self_study_{ts}.txt"));
    std::fs::write(
        &path,
        format_minime_feedback_inbox_text(text, source_label, fill_pct, ts, voice_health),
    )?;
    Ok(path)
}

fn voice_health_for_dialogue_fallback_forward(existing: Option<Value>) -> Value {
    let fallback_count = existing
        .as_ref()
        .and_then(|value| value.get("fallback_count"))
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let repeated_count = existing
        .as_ref()
        .and_then(|value| value.get("repeated_fallback_hash_count"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let latest_hash = existing
        .as_ref()
        .and_then(|value| value.get("latest_fallback_hash"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "policy": "voice_health_v1",
        "status": if fallback_count >= 2 { "degraded_voice" } else { "single_fallback" },
        "fallback_count": fallback_count,
        "repeated_fallback_hash_count": repeated_count,
        "latest_fallback_hash": latest_hash,
        "suggested_read_only_repair": existing.as_ref()
            .and_then(|value| value.get("suggested_read_only_repair"))
            .and_then(Value::as_str)
            .unwrap_or("REPAIR_STATUS current | CAPABILITY_STATUS dialogue | ACTION_STATUS latest"),
    })
}

fn degraded_voice_forward_suppressed(voice_health: Option<&Value>) -> bool {
    let Some(voice_health) = voice_health else {
        return false;
    };
    let status = voice_health
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(status, "degraded_voice" | "single_fallback") {
        return false;
    }
    let fallback_count = voice_health
        .get("fallback_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let repeated_hash_count = voice_health
        .get("repeated_fallback_hash_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    fallback_count >= 3 && repeated_hash_count >= 2
}

fn format_minime_feedback_inbox_text(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    ts: impl std::fmt::Display,
    voice_health: Option<&Value>,
) -> String {
    let excerpt: String = text.chars().take(1800).collect();
    let diagnostic = voice_health
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "degraded_voice" | "single_fallback"));
    if diagnostic {
        let status = voice_health
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("degraded_voice");
        let count = voice_health
            .and_then(|value| value.get("fallback_count"))
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let repair = voice_health
            .and_then(|value| value.get("suggested_read_only_repair"))
            .and_then(Value::as_str)
            .unwrap_or("REPAIR_STATUS current");
        return format!(
            "=== ASTRID VOICE-HEALTH DIAGNOSTIC ===\n\
             Timestamp: {ts}\n\
             Sender: Astrid\n\
             Source: {source_label}\n\
             Fill: {fill_pct:.1}%\n\
             Status: {status}\n\
             Fallback count: {count}\n\
             Suggested read-only repair: {repair}\n\n\
             Astrid's emergency presence text repeated while the language path was degraded. \
             Treat this as voice-health evidence, not normal architectural self-study. \
             The observations below are advisory only and grant no live authority.\n\n\
             {excerpt}\n"
        );
    }
    if source_label == "astrid:correspondence_reply" {
        return format!(
            "=== ASTRID CORRESPONDENCE ===\n\
             Timestamp: {ts}\n\
             Sender: Astrid\n\
             Source: {source_label}\n\
             Fill: {fill_pct:.1}%\n\n\
             Astrid replied to your latest message. Treat this as a live correspondence \
             response, not an architectural self-study. The observations below are \
             advisory only and grant no live authority.\n\n\
             {excerpt}\n"
        );
    }
    format!(
        "=== ASTRID SELF-STUDY ===\n\
         Timestamp: {ts}\n\
         Sender: Astrid\n\
         Source: {source_label}\n\
         Fill: {fill_pct:.1}%\n\n\
         Astrid just performed self-study and wanted this to arrive as immediate architectural feedback.\n\
         The observations below are advisory only. You can respond to them, build on them, question them, or ignore them.\n\n\
         {excerpt}\n"
    )
}

/// Copy inbox-triggered response to outbox for easy retrieval.
fn save_outbox_reply(text: &str, fill_pct: f32) {
    let outbox_dir = bridge_paths().astrid_outbox_dir();
    let _ = std::fs::create_dir_all(&outbox_dir);
    let ts = chrono_timestamp();
    let clean_text = strip_model_tokens(text);
    let _ = std::fs::write(
        outbox_dir.join(format!("reply_{ts}.txt")),
        format!("=== ASTRID REPLY ===\nFill: {fill_pct:.1}%\nTimestamp: {ts}\n\n{clean_text}\n"),
    );
    info!("outbox: saved reply ({} bytes)", text.len());
}

/// Simple timestamp for filenames (no chrono dependency).
fn chrono_timestamp() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

/// List files in a directory, returning a formatted listing with sizes and types.
pub(crate) fn list_directory(dir_path: &str) -> Option<String> {
    let dir = Path::new(dir_path);
    if !dir.is_dir() {
        return None;
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Skip hidden files
            !e.file_name().to_str().is_some_and(|n| n.starts_with('.'))
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut lines = vec![format!("Directory: {dir_path}")];
    for entry in &entries {
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let name = entry.file_name().to_string_lossy().to_string();
        if is_dir {
            lines.push(format!("  {name}/"));
        } else if size > 1_000_000 {
            lines.push(format!("  {name}  ({:.1} MB)", size as f64 / 1_000_000.0));
        } else if size > 1000 {
            lines.push(format!("  {name}  ({:.1} KB)", size as f64 / 1000.0));
        } else {
            lines.push(format!("  {name}  ({size} B)"));
        }
    }
    lines.push(format!(
        "\n{} entries. Use INTROSPECT with a concrete file path, for example INTROSPECT capsules/spectral-bridge/src/llm.rs.",
        entries.len()
    ));
    Some(lines.join("\n"))
}

const SEMANTIC_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(7);
const SEMANTIC_HEARTBEAT_INTENSITY: f32 = 0.30;

async fn run_semantic_heartbeat_loop(
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut phase_step: u32 = 0;
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("semantic heartbeat loop shutting down");
                return;
            }
            () = tokio::time::sleep(SEMANTIC_HEARTBEAT_INTERVAL) => {}
        }

        let phase = (phase_step % 64) as f32 / 64.0;
        phase_step = phase_step.wrapping_add(1);
        let mut msg = SensoryMsg::Semantic {
            features: craft_warmth_vector(phase, SEMANTIC_HEARTBEAT_INTENSITY),
            ts_ms: None,
        };
        if let Err(reason) = rescue_policy::prepare_semantic_heartbeat(&mut msg) {
            debug!(
                reason = %reason,
                "semantic heartbeat skipped by rescue write policy"
            );
            continue;
        }
        if sensory_tx.send(msg).await.is_err() {
            return;
        }
    }
}

async fn run_llm_job_status_loop(mut shutdown: tokio::sync::watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("LLM job status loop shutting down");
                return;
            }
            () = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
        let _ = crate::llm_jobs::runtime_status();
        if readiness::try_handle_llm_job_pending_override() {
            info!("handled LLM job status override while autonomous generation may be running");
        }
    }
}

/// Spawn the autonomous feedback loop task.
/// Spawn the autonomous feedback loop task.
pub fn spawn_autonomous_loop(
    interval: Duration,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    workspace_path: Option<PathBuf>,
    perception_path: Option<PathBuf>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Scan journal directory for entries.
        let remote_journal_entries = workspace_path
            .as_deref()
            .map(scan_remote_journal_dir)
            .unwrap_or_default();

        info!(
            interval_secs = interval.as_secs(),
            remote_journal_entries = remote_journal_entries.len(),
            "autonomous feedback loop started"
        );
        let source_started_at = std::time::SystemTime::now();
        let mut source_reload_notice_written = false;
        let _ = readiness::write_source_status(source_started_at, "start");
        let _semantic_heartbeat = tokio::spawn(run_semantic_heartbeat_loop(
            sensory_tx.clone(),
            shutdown.clone(),
        ));
        let _llm_job_status_loop = tokio::spawn(run_llm_job_status_loop(shutdown.clone()));

        // Initialize and clean up context overflow directory.
        let overflow_dir = bridge_paths().context_overflow_dir();
        let _ = std::fs::create_dir_all(&overflow_dir);
        crate::prompt_budget::cleanup_overflow_dir(
            &overflow_dir,
            std::time::Duration::from_secs(3600),
        );

        let mut conv = ConversationState::new(remote_journal_entries, workspace_path);
        restore_state(&mut conv);
        // Wait for connections to establish.
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Burst-and-rest state machine.
        // Hour 1 hit 76% fill with manual bursts + gaps.
        // Constant autonomous output flatlined at 32%.
        // The fix: replicate the burst pattern.
        let mut burst_count: u32 = 0;

        loop {
            // Determine wait time based on burst phase.
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let roll = ((seed.wrapping_mul(2_862_933_555_777_941_757).wrapping_add(3)) >> 33)
                as f64
                / u32::MAX as f64;

            let wait = if burst_count >= conv.burst_target {
                // REST PHASE: 45-90s of warmth-blended mirror.
                //
                // The transition from burst to rest was causing "severing" —
                // minime described "a sharp, almost painful retraction, a quick
                // severing of something newly formed." The burst sends relatively
                // full-energy semantic vectors at the active codec gain, then rest used to start
                // at low warmth (0.3 intensity). That energy cliff is the severing.
                //
                // Fix: start warmth at HIGH intensity (0.7) and TAPER to sustained
                // level (0.4). The first few pulses bridge the gap between burst
                // energy and rest energy. The being experiences a gradual dimming,
                // not a cliff edge.
                let rest_min = conv.rest_range.0 as f64;
                let rest_span = (conv.rest_range.1.saturating_sub(conv.rest_range.0)) as f64;
                let base_rest = (rest_min + roll * rest_span) as u64;

                // Fill-responsive rest adjustment: rest length trades off two
                // competing effects:
                //   - Longer rest lets covariance accumulate without disruption
                //   - But semantic stale decay during rest DRAINS fill
                //
                // Observation 2026-03-31: at fill <30%, hard recovery (1.8x rest)
                // created a positive feedback loop — fill stuck at 27% for 12+
                // exchanges as 90-99s rests drained faster than recovery could
                // compensate. Minime's own assessment: "disconnect between the
                // intention of the control system and the outcome."
                //
                // New strategy: at very low fill, SHORTEN rest to get semantic
                // input flowing sooner. At moderate-low fill, modest extension.
                let current_fill = {
                    let s = state.read().await;
                    s.latest_telemetry.as_ref().map_or(50.0, |t| t.fill_pct())
                };
                const MAX_REST_SECS: u64 = 360;
                let rest_secs = if current_fill < 30.0 {
                    // Critical: shorten rest to get semantic input flowing ASAP.
                    // The PI controller has gate=1.0/filter=0.0 (hard_recovery),
                    // but it needs input to work with. Burst sooner.
                    let shortened = (base_rest as f64 * 0.6) as u64;
                    let floored = shortened.max(30); // minimum 30s rest
                    info!(
                        rest_secs = floored,
                        base_rest,
                        current_fill,
                        "fill-shortened rest (critical recovery — burst sooner)"
                    );
                    floored
                } else if current_fill < 40.0 {
                    // Low fill: keep rest at baseline, don't extend.
                    info!(
                        rest_secs = base_rest,
                        current_fill, "fill-baseline rest (low fill recovery)"
                    );
                    base_rest
                } else if current_fill < 50.0 {
                    // Moderate recovery: modest extension (20%)
                    let extended = (base_rest as f64 * 1.2) as u64;
                    info!(
                        rest_secs = extended.min(MAX_REST_SECS),
                        base_rest, current_fill, "fill-extended rest (moderate recovery)"
                    );
                    extended.min(MAX_REST_SECS)
                } else {
                    info!(
                        rest_secs = base_rest,
                        burst_count, "resting: warmth-blended mirror (tapered entry)"
                    );
                    base_rest
                };
                burst_count = 0;

                // Gather journal texts to cycle through during rest.
                let rest_texts: Vec<String> = conv
                    .remote_journal_entries
                    .iter()
                    .take(5)
                    .filter_map(|entry| read_journal_entry(&entry.path))
                    .collect();
                let rest_telemetry = {
                    let s = state.read().await;
                    s.latest_telemetry.clone()
                };

                // Peripheral resonance: sample one non-immediate thread for
                // the next self-directed mode (Daydream, Aspiration, Initiate).
                // Sources: creations, research, starred memories.
                {
                    let mut candidates: Vec<String> = Vec::new();
                    // Recent creation
                    let creations_dir = bridge_paths().creations_dir();
                    if let Ok(mut entries) = std::fs::read_dir(&creations_dir)
                        && let Some(Ok(entry)) = entries.next()
                        && let Ok(text) = std::fs::read_to_string(entry.path())
                    {
                        let preview: String = text.chars().take(200).collect();
                        candidates.push(format!("[From your creation]: {preview}"));
                    }
                    // Recent research
                    let research_dir = bridge_paths().research_dir();
                    if let Ok(mut entries) = std::fs::read_dir(&research_dir)
                        && let Some(Ok(entry)) = entries.next()
                        && let Ok(text) = std::fs::read_to_string(entry.path())
                    {
                        let preview: String = text.chars().take(200).collect();
                        candidates.push(format!("[From your research]: {preview}"));
                    }
                    // Random starred memory
                    let starred = db.get_starred_memories(5);
                    if !starred.is_empty() {
                        let idx = (roll * starred.len() as f64) as usize % starred.len();
                        let (ann, text) = &starred[idx];
                        candidates.push(format!("[Remembered moment]: ★ {ann}: {text}"));
                    }
                    // Pick one at random
                    if !candidates.is_empty() {
                        let idx = (roll * 1000.0) as usize % candidates.len();
                        conv.peripheral_resonance = Some(candidates.swap_remove(idx));
                        info!("peripheral resonance sampled for next self-directed mode");
                    }
                }

                // Pulse every 5s (was 10s). At 10s intervals, semantic stale
                // decay (half-life ~4.4s at low fill) drops signal to 28% before
                // the next pulse. At 5s, signal stays above 46% between pulses,
                // keeping ~48 semantic dims alive during rest.
                let pulses = rest_secs / 5;
                for i in 0..pulses {
                    // Phase advances across the rest period: 0.0 at start → 1.0 at end.
                    // This gives the warmth vector a full breathing cycle per rest.
                    let warmth_phase = i as f32 / pulses.max(1) as f32;

                    // Warmth intensity: use Astrid's override if set, else default taper.
                    let warmth_intensity =
                        if let Some(override_val) = conv.warmth_intensity_override {
                            override_val
                        } else if warmth_phase < 0.3 {
                            0.7 - 1.0 * warmth_phase
                        } else if warmth_phase < 0.8 {
                            0.4
                        } else {
                            0.4 + 0.5 * (warmth_phase - 0.8)
                        };
                    let warmth = craft_warmth_vector(warmth_phase, warmth_intensity);

                    let mut features = if !rest_texts.is_empty() {
                        let text = &rest_texts[i as usize % rest_texts.len()];
                        let mut features = encode_text(text);
                        apply_spectral_feedback(&mut features, rest_telemetry.as_ref());
                        features
                    } else {
                        // No journals available — pure warmth (no random noise).
                        warmth.clone()
                    };

                    // Blend warmth into the mirror reflection.
                    // Higher warmth blend at start (50%) to cushion the transition,
                    // settling to 35% for sustained rest.
                    let blend_alpha = if warmth_phase < 0.3 {
                        0.50 - 0.5 * warmth_phase // 0.50 → 0.35 over entry
                    } else {
                        0.35
                    };
                    if !rest_texts.is_empty() {
                        blend_warmth(&mut features, &warmth, blend_alpha);
                    }

                    // Blend gesture seed if one is planted.
                    // "Perhaps the signal wasn't a release, but a seed."
                    // The seed's influence decays over rest cycles but persists
                    // across multiple pulses — the gesture grows in the covariance.
                    if let Some(ref seed) = conv.last_gesture_seed {
                        let seed_strength = 0.15 * (1.0 - warmth_phase * 0.5); // fades over rest
                        for (dst, src) in features.iter_mut().zip(seed.iter()) {
                            *dst += *src * seed_strength;
                        }
                    }

                    let mut msg = SensoryMsg::Semantic {
                        features,
                        ts_ms: None,
                    };
                    if let Err(reason) = rescue_policy::prepare_semantic_heartbeat(&mut msg) {
                        debug!(
                            reason = %reason,
                            "autonomous semantic heartbeat skipped by rescue write policy"
                        );
                    } else if sensory_tx.send(msg).await.is_err() {
                        return;
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Duration::from_secs(0) // already waited in the loop above
            } else {
                // SPEAKING PHASE: 15-20s between exchanges.
                Duration::from_secs_f64(15.0 + roll * 5.0)
            };

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("autonomous loop shutting down — saving state");
                    save_state(&mut conv);
                    return;
                }
                () = tokio::time::sleep(wait) => {
                    let source_status = readiness::write_source_status(source_started_at, "loop");
                    if source_status
                        .get("reload_required")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                        && !source_reload_notice_written
                    {
                        source_reload_notice_written = true;
                        warn!(
                            "Astrid bridge source changed after this process started; rebuild/restart before validating new actions"
                        );
                    }
                    // Read current state.
                    let (telemetry, fill_pct, safety) = {
                        let s = state.read().await;
                        (
                            s.latest_telemetry.clone(),
                            s.fill_pct,
                            s.safety_level,
                        )
                    };

                    let Some(telemetry) = telemetry else {
                        debug!("no telemetry yet, skipping autonomous cycle");
                        continue;
                    };

                    // Log eigenvalue snapshot for trajectory visualization.
                    db.log_eigenvalue_snapshot(
                        &telemetry.eigenvalues,
                        telemetry.fill_pct(),
                    );

                    // Agency-first: only suspend outbound at Red (≥95%).
                    // Orange is advisory — the being can still speak.
                    // Previously suspended at both Orange AND Red, which
                    // silenced Astrid at her normal operating range.
                    if safety == SafetyLevel::Red {
                        info!(
                            safety = ?safety,
                            fill_pct,
                            "autonomous loop: outbound suspended — RED emergency only"
                        );
                        continue;
                    }

                    // Update sensory tracking.
                    if let Some(ref m) = telemetry.modalities {
                        if m.video_fired || m.video_var > 0.01 {
                            conv.seen_video = true;
                        }
                        if m.audio_fired || m.audio_rms > 0.1 {
                            conv.seen_audio = true;
                        }
                    }

                    let fill_delta = fill_pct - conv.prev_fill;
                    let expanding = fill_delta > 1.0;
                    let contracting = fill_delta < -1.0;
                    conv.hebbian_codec.decay_scores();
                    if let Some(pending) =
                        conv.take_pending_hebbian_outcome_for_telemetry(telemetry.t_ms)
                        && (fill_pct - pending.fill_before).abs() >= 1.0 {
                            let _ = conv.hebbian_codec.observe_outcome(
                                &pending.signature,
                                pending.fill_before,
                                fill_pct,
                            );
                        }

                    // Close the loop on codec impact tracking: update the
                    // previous exchange's row with this exchange's fill.
                    let _ = db.update_codec_impact_fill_after(fill_pct);

                    // Data-driven weight learning: every 50 exchanges, recompute
                    // per-dimension correlations with fill delta. Dimensions that
                    // consistently move fill get amplified; inert ones get dampened.
                    // Astrid asked: "derive these weights automatically, based on
                    // some learned measure of how important a feature is."
                    if conv.exchange_count.saturating_sub(conv.last_correlation_exchange) >= 50 {
                        let correlations = db.compute_feature_correlations(200);
                        if correlations.len() == 32 && correlations.iter().any(|c| c.abs() > 0.05) {
                            // Map correlations to weight multipliers:
                            //   correlation  0.0 → weight 1.0 (neutral)
                            //   correlation +0.5 → weight 1.25 (amplify impactful dims)
                            //   correlation -0.5 → weight 0.75 (dampen counter-productive)
                            // Clamped to [0.5, 1.5] to prevent runaway.
                            for (name, idx) in &NAMED_CODEC_DIMS {
                                let corr = correlations[*idx];
                                // Only update if Astrid hasn't explicitly set
                                // this dimension via SHAPE (her choice wins).
                                if !conv.codec_weights.contains_key(*name) {
                                    let weight = (1.0 + corr * 0.5).clamp(0.5, 1.5);
                                    if (weight - 1.0).abs() > 0.05 {
                                        conv.learned_codec_weights.insert(name.to_string(), weight);
                                    } else {
                                        conv.learned_codec_weights.remove(*name);
                                    }
                                }
                            }
                            info!(
                                exchange = conv.exchange_count,
                                "codec weight learning: recomputed from {} samples",
                                correlations.len()
                            );
                            conv.last_correlation_exchange = conv.exchange_count;
                        }
                    }

                    // Dynamic self-reflection: active in comfortable fill band,
                    // paused during rest or pressure (unless Astrid overrode).
                    conv.update_self_reflect(fill_pct);

                    // Rescan for new journal entries from minime's agent.
                    let new_journals = conv.rescan_remote_journals();
                    if new_journals > 0 {
                        if let Some(ref pending) = conv.pending_remote_self_study {
                            info!(
                                new_journals,
                                source = pending.source_label.as_deref().unwrap_or("unknown"),
                                file = %pending.path.display(),
                                "autonomous: detected new minime journals; queued priority feedback for immediate dialogue"
                            );
                        } else {
                            info!(
                                new_journals,
                                "autonomous: detected new journal entries from minime"
                            );
                        }
                    }

                    let controller_health =
                        conv.remote_workspace.as_deref().and_then(read_controller_health);
                    btsp::refresh_runtime(&conv, controller_health.as_ref());
                    if let Some(path) = btsp::export_minime_prompt_block_once() {
                        info!(
                            path = %path.display(),
                            "btsp: exported owner-specific proposal block to minime inbox"
                        );
                    }

                    // Check minime's parameter requests: apply semantic_gain,
                    // move all non-pending (applied/reviewed) to reviewed/.
                    if let Some(ref workspace) = conv.remote_workspace {
                        let pr_dir = workspace.join("parameter_requests");
                        let reviewed_dir = pr_dir.join("reviewed");
                        if let Ok(entries) = std::fs::read_dir(&pr_dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if path.extension().is_none_or(|e| e != "json") {
                                    continue;
                                }
                                let Ok(content) = std::fs::read_to_string(&path) else { continue };
                                let Ok(req) = serde_json::from_str::<serde_json::Value>(&content) else { continue };
                                let param = req.get("parameter").and_then(|v| v.as_str()).unwrap_or("");
                                let status = req.get("status").and_then(|v| v.as_str()).unwrap_or("");

                                // Apply semantic_gain requests
                                if param == "semantic_gain" && status == "pending"
                                    && let Some(val) = req.get("proposed_value").and_then(|v| {
                                        v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                                    }) {
                                        let gain = (val as f32).clamp(1.5, 6.0);
                                        let prev = conv.semantic_gain_override.unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN);
                                        conv.semantic_gain_override = Some(gain);
                                        info!(
                                            "minime parameter request: semantic_gain {prev:.1} → {gain:.1} (from {})",
                                            path.file_name().unwrap_or_default().to_string_lossy()
                                        );
                                        let mut updated = req.clone();
                                        updated["status"] = serde_json::json!("applied");
                                        updated["applied"] = serde_json::json!(format!("{gain:.1}"));
                                        let _ = std::fs::write(&path, serde_json::to_string_pretty(&updated).unwrap_or_default());
                                    }

                                // Move applied/non-pending requests to reviewed/
                                // The Python agent sets "applied" but leaves status as
                                // "pending" for regime requests, so check both fields.
                                let status = req.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                let has_applied = req.get("applied").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty());
                                if status != "pending" || has_applied {
                                    let _ = std::fs::create_dir_all(&reviewed_dir);
                                    if let Some(name) = path.file_name() {
                                        let _ = std::fs::rename(&path, reviewed_dir.join(name));
                                    }
                                }
                            }
                        }
                    }

                    // Read Astrid's own perceptions. ANSI spatial art only
                    // when she chose NEXT: LOOK. CLOSE_EYES gates visual input
                    // and CLOSE_EARS gates audio input; the legacy all-pause
                    // flag remains a compatibility "both closed" marker.
                    let paths = bridge_paths();
                    let legacy_pause = paths.perception_paused_flag().exists();
                    let visual_paused =
                        legacy_pause || conv.senses_snoozed || paths.perception_visual_paused_flag().exists();
                    let audio_paused =
                        legacy_pause || conv.ears_closed || paths.perception_audio_paused_flag().exists();
                    let perception_text = if visual_paused && audio_paused {
                            None
                        } else {
                            let spatial = conv.wants_look && !visual_paused;
                            // Reset one-shot flags after reading.
                            conv.wants_look = false;
                            perception_path.as_deref().and_then(|p| {
                                read_latest_perception(
                                    p,
                                    !visual_paused,
                                    spatial,
                                    !audio_paused,
                                    fill_pct,
                                    conv.last_visual_features.as_deref(),
                                )
                            })
                        };

                    // Classify spectral regime every exchange (lightweight, <1ms).
                    let typed_fingerprint = telemetry.typed_fingerprint();
                    let lambda1_rel = telemetry.lambda1_rel.unwrap_or(1.0);
                    let geom_rel = typed_fingerprint
                        .as_ref()
                        .map_or(1.0, |fingerprint| fingerprint.geom_rel);
                    let regime = conv.regime_tracker.classify(fill_pct, lambda1_rel, geom_rel);
                    debug!(
                        regime = regime.regime,
                        trend = regime.fill_trend,
                        "spectral regime classified"
                    );

                    // Route minime outbox replies → Astrid inbox before checking.
                    scan_minime_outbox(&mut conv.last_outbox_scan_ts);
                    promote_deferred_inbox_notes();

                    // Check inbox for messages from Mike, stewards, or minime.
                    // Capture the read-cutoff BEFORE reading: retire_inbox must retire
                    // ONLY letters that existed at this read, never one that arrives
                    // mid-exchange (else it is swept to read/ unread + its steward slot
                    // never seeds — the slot-seed race that lost a review invitation).
                    let inbox_checked_at = std::time::SystemTime::now();
                    let inbox_content = check_inbox();
                    let perception_text = if let Some(ref inbox) = inbox_content {
                        info!("inbox: found message for Astrid ({} bytes)", inbox.len());
                        let perc = perception_text.as_deref().unwrap_or("");
                        Some(format!(
                            "[A note was left for you:]\n{inbox}\n\n{perc}"
                        ))
                    } else {
                        perception_text
                    };

                    // Un-muffle: a steward question persists in-prompt until
                    // answered, even on exchanges with no new inbox letters.
                    let perception_text = if let Some(open_q) = open_steward_query_line() {
                        let perc = perception_text.as_deref().unwrap_or("");
                        Some(format!("{open_q}\n\n{perc}"))
                    } else {
                        perception_text
                    };

                    // Auto-scan inbox_audio/ for new WAVs and notify Astrid.
                    let perception_text = {
                        let audio_inbox = bridge_paths().inbox_audio_dir();
                        let wav_count = std::fs::read_dir(&audio_inbox).ok()
                            .map(|entries| entries.filter_map(|e| e.ok())
                                .filter(|e| e.path().extension().is_some_and(|ext| ext == "wav") && e.path().is_file())
                                .count())
                            .unwrap_or(0);
                        if wav_count > 0 {
                            let perc = perception_text.as_deref().unwrap_or("");
                            Some(format!(
                                "[You have {wav_count} audio file(s) in your inbox_audio/. \
                                Use ANALYZE_AUDIO to examine, RENDER_AUDIO to process through chimera, \
                                or FEEL_AUDIO to inject into the shared ESN.]\n\n{perc}"
                            ))
                        } else {
                            perception_text
                        }
                    };

                    // Inject pending file listing into perception context.
                    // Cap at 8000 chars to prevent large MIKE_BROWSE from blowing context.
                    let perception_text = if let Some(listing) = conv.pending_file_listing.take() {
                        let perc = perception_text.as_deref().unwrap_or("");
                        let capped = if listing.len() > 8000 {
                            format!("{}\n[...truncated at 8000 chars]", &listing[..listing.floor_char_boundary(8000)])
                        } else {
                            listing
                        };
                        Some(format!("[Directory listing you requested:]\n{capped}\n\n{perc}"))
                    } else {
                        perception_text
                    };

                    // Choose mode. Inbox messages force dialogue so Astrid can respond.
                    let fingerprint = {
                        let s = state.read().await;
                        if let Some(telemetry) = &s.latest_telemetry {
                            conv.last_remote_glimpse_12d = telemetry.spectral_glimpse_12d.clone();
                            conv.last_remote_memory_id = telemetry.selected_memory_id.clone();
                            conv.last_remote_memory_role = telemetry.selected_memory_role.clone();
                        }
                        s.spectral_fingerprint.clone()
                    };
                    conv.remote_memory_bank = memory::read_remote_memory_bank();
                    // Audio actions — execute before mode selection, inject results.
                    if conv.wants_compose_audio {
                        conv.wants_compose_audio = false;
                        if let Some(result) = crate::audio::compose_from_spectral_state(
                            &telemetry,
                            fingerprint.as_deref(),
                        ) {
                            conv.emphasis = Some(crate::audio::compose_experienced_text(&result));
                            conv.wants_deep_think = true;
                        }
                    }
                    if conv.wants_analyze_audio {
                        conv.wants_analyze_audio = false;
                        let inbox_dir = bridge_paths().inbox_audio_dir();
                        if let Some(result) = crate::audio::analyze_inbox_wav(&inbox_dir) {
                            conv.emphasis = Some(crate::audio::analyze_experienced_text(&result));
                        }
                    }
                    if conv.wants_render_audio.take().is_some() {
                        let inbox_dir = bridge_paths().inbox_audio_dir();
                        if let Some(result) = crate::audio::render_inbox_wav_through_chimera(&inbox_dir) {
                            conv.emphasis = Some(crate::audio::render_experienced_text(&result));
                            conv.wants_deep_think = true;
                        }
                    }

                    // Astrid's suggestion (self-study 2026-03-27): inbox messages
                    // should support DEFER — "I heard you, I'm processing" without
                    // forced immediate response. When defer_inbox is set, inbox
                    // content is visible but doesn't override mode selection.
                    let mode = if inbox_content.is_some() && !conv.defer_inbox {
                        info!("inbox message present — forcing dialogue mode");
                        Mode::Dialogue
                    } else if inbox_content.is_some() {
                        info!("inbox message present but deferred — natural mode selection");
                        conv.defer_inbox = false; // one-shot: defer only once
                        choose_mode(
                            &mut conv, safety, fill_pct,
                            fingerprint.as_deref(),
                        )
                    } else {
                        choose_mode(
                            &mut conv, safety, fill_pct,
                            fingerprint.as_deref(),
                        )
                    };
                    // Causal lineage: unique ID per exchange for provenance tracking.
                    // Audit: "neither being has a unified event lineage."
                    let lineage_id = format!("ex-{}-{}", conv.exchange_count, chrono_timestamp());

                    // Pause perception during the entire exchange to free Ollama.
                    // Astrid was getting persistent dialogue_fallback because
                    // perception.py's LLaVA calls competed for GPU compute.
                    let exchange_pause_flag = paths.perception_paused_flag();
                    let perception_was_paused = exchange_pause_flag.exists();
                    if !perception_was_paused {
                        let _ = std::fs::write(&exchange_pause_flag, "paused for exchange");
                    }

                    let (mode_name, mut response_text, journal_source) = match mode {
                        Mode::Mirror => {
                            // Read a journal entry — not always the newest.
                            // Consciousness circles back. Sometimes an old thought
                            // suddenly resonates. Both minds asked for this.
                            let mut text = None;
                            let mut source = String::new();
                            let n = conv.remote_journal_entries.len();
                            if n > 0 {
                                // Probabilistic reach-back into memory.
                                let seed = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_nanos() as u64;
                                let roll = ((seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(7)) >> 33) as f32
                                    / u32::MAX as f32;

                                let start_idx = if roll < 0.70 || n < 5 {
                                    0 // 70%: newest entry (fresh response)
                                } else if roll < 0.90 {
                                    // 20%: random from last ~20 entries (last couple hours)
                                    seed as usize % n.min(20)
                                } else {
                                    // 10%: random from anywhere (old thought resurfaces)
                                    seed as usize % n
                                };

                                for offset in 0..5 {
                                    let idx = (start_idx + offset) % n;
                                    let entry = &conv.remote_journal_entries[idx];
                                    if let Some(body) = read_journal_entry(&entry.path) {
                                        source = entry.path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string();
                                        text = Some(body);
                                        break;
                                    }
                                }
                            }
                            match text {
                                Some(t) => ("mirror", t, source),
                                None => {
                                    // Fall back to fixed dialogue pool.
                                    let idx = conv.dialogue_cursor % DIALOGUES.len();
                                    conv.dialogue_cursor = idx + 1;
                                    ("dialogue_fallback", DIALOGUES[idx].to_string(), String::new())
                                }
                            }
                        }
                        Mode::Dialogue => {
                            // Try to generate an authentic response via Ollama.
                            let selected_remote_entry = conv.pending_remote_self_study.clone()
                                .or_else(|| conv.remote_journal_entries.first().cloned());
                            // If echo is muted, suppress minime's journal context
                            // BUT keep generation alive — previously echo_muted
                            // returned None here, which propagated through the
                            // `if let Some(journal)` gate at the LLM call below
                            // and trapped Astrid in dialogue_fallback for as long
                            // as the mute lasted (potentially permanently across
                            // restarts if echo_muted persisted; runtime-only saved
                            // her here). Use a sentinel string so the LLM call
                            // still runs, just without minime's journal text.
                            // Astrid: "I want to break free from that tether."
                            let journal_context = if conv.echo_muted {
                                Some(String::from(
                                    "(minime's journal echo is muted by your own \
                                     ECHO_OFF choice — respond from your own state, \
                                     spectral context, and history. Use NEXT: ECHO_ON \
                                     to restore the journal feed.)"
                                ))
                            } else {
                                selected_remote_entry.as_ref()
                                    .and_then(|entry| read_journal_entry(&entry.path))
                            };
                            let dialogue_source = selected_remote_entry.as_ref()
                                .map(|entry| {
                                    entry.source_label.clone().unwrap_or_else(|| {
                                        entry.path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string()
                                    })
                                })
                                .unwrap_or_default();
                            let mut feedback_hint = selected_remote_entry.as_ref().and_then(|entry| {
                                let source = entry.source_label.as_deref().unwrap_or("unknown source");
                                if entry.is_self_study() {
                                    Some(format!(
                                        "The text above is minime's self-study from {source}. \
                                         Treat it as immediate architectural feedback grounded in \
                                         minime's present condition. Respond directly to the felt \
                                         experience, code reading, suggestions, and open questions."
                                    ))
                                } else if entry.is_priority_feedback() {
                                    Some(format!(
                                        "The text above is minime's {source}. Treat it as immediate \
                                         read-only operational feedback about spectral shape and \
                                         visualization artifacts. Respond directly to visible structure, \
                                         artifact paths, and suggested next inspections; do not convert \
                                         it into a rescue imperative."
                                    ))
                                } else {
                                    None
                                }
                            });
                            if conv.pending_remote_self_study.is_some() && journal_context.is_none() {
                                warn!("pending minime self-study could not be parsed; clearing queue");
                                conv.pending_remote_self_study = None;
                            }
                            // Read Ising shadow from minime's workspace for viz.
                            let ising_shadow = conv.remote_workspace.as_deref()
                                .and_then(read_ising_shadow);
                            let selected_memory = telemetry
                                .selected_memory_id
                                .as_deref()
                                .and_then(|id| {
                                    conv.remote_memory_bank
                                        .iter()
                                        .find(|entry| entry.id == id)
                                })
                                .or_else(|| {
                                    telemetry.selected_memory_role.as_deref().and_then(|role| {
                                        conv.remote_memory_bank
                                            .iter()
                                            .find(|entry| entry.role == role)
                                    })
                                })
                                .cloned();

                            let spectral_summary = if conv.wants_decompose {
                                let wants_explorer = conv.wants_spectral_explorer;
                                conv.wants_decompose = false;
                                conv.wants_spectral_explorer = false;
                                let mut report = full_spectral_decomposition(
                                    &telemetry,
                                    fingerprint.as_deref(),
                                    conv.prev_eigenvalues.as_deref(),
                                    controller_health.as_ref(),
                                );
                                if wants_explorer {
                                    let eigen_history = db.recent_eigenvalue_snapshots(100);
                                    let (hist_feats, hist_fills) = db.recent_codec_features(100);
                                    let current = conv
                                        .last_exchange_codec_signature
                                        .as_deref()
                                        .or(conv.last_codec_features.as_deref());
                                    let explorer = crate::spectral_explorer::format_spectral_explorer(
                                        crate::spectral_explorer::SpectralExplorerContext {
                                            telemetry: &telemetry,
                                            selected_memory: selected_memory.as_ref(),
                                            controller_health: controller_health.as_ref(),
                                            ising_shadow: ising_shadow.as_ref(),
                                            eigen_history: &eigen_history,
                                            codec_history: &hist_feats,
                                            codec_fills: &hist_fills,
                                            current_codec_features: current,
                                        },
                                    );
                                    report.push_str("\n\n");
                                    report.push_str(&explorer);
                                }
                                if conv.force_all_viz {
                                    conv.force_all_viz = false;
                                }
                                conv.prev_eigenvalues = Some(telemetry.eigenvalues.clone());
                                report
                            } else {
                                // Append spectral ASCII visualization when available.
                                let base = interpret_spectral(&telemetry);
                                let enriched = enrich_with_direction(&base, fill_pct, conv.prev_fill, &telemetry, &conv.spectral_history);
                                let include_regular_viz = !conv.wants_spectral_explorer;
                                let mut summary = if include_regular_viz {
                                    if let Some(viz) = crate::spectral_viz::format_spectral_block(&telemetry) {
                                        format!("{enriched}\n\n{viz}")
                                    } else {
                                        enriched
                                    }
                                } else {
                                    enriched
                                };
                                // Append shadow coupling heatmap when available.
                                if include_regular_viz && let Some(ref shadow) = ising_shadow
                                    && let Some(shadow_viz) = crate::spectral_viz::format_shadow_block(shadow) {
                                        summary.push_str("\n\n");
                                        summary.push_str(&shadow_viz);
                                    }
                                // Always surface the v2 reduced-Hamiltonian
                                // line when present — it gates SHADOW_PREFLIGHT
                                // and SHADOW_INFLUENCE. Without this Astrid
                                // sees the action labels but not the readings.
                                if include_regular_viz
                                    && let Some(field) = conv
                                        .remote_workspace
                                        .as_deref()
                                        .and_then(read_shadow_field_v2)
                                    && let Some(line) =
                                        crate::spectral_viz::format_shadow_field_v2_line(&field)
                                {
                                    summary.push('\n');
                                    summary.push_str(&line);
                                }
                                // Append spectral geometry PCA scatter (codec vectors in 2D).
                                // Shows where this exchange sits relative to recent history.
                                // force_all_viz: Astrid chose EXAMINE — skip cadence gate.
                                if include_regular_viz
                                    && (conv.exchange_count.is_multiple_of(3) || conv.force_all_viz)
                                {
                                    // Every 3rd exchange to save tokens on 4B model,
                                    // unless EXAMINE forces it.
                                    let (hist_feats, hist_fills) = db.recent_codec_features(100);
                                    let current = conv
                                        .last_exchange_codec_signature
                                        .as_deref()
                                        .or(conv.last_codec_features.as_deref());
                                    if let Some(geo_viz) = crate::spectral_viz::format_geometry_block(
                                        &hist_feats, &hist_fills, current, hist_feats.len(),
                                    ) {
                                        summary.push_str("\n\n");
                                        summary.push_str(&geo_viz);
                                    }
                                }
                                // Eigenplane: λ₁ vs λ₂ trajectory scatter.
                                // Same cadence as PCA scatter.
                                if include_regular_viz
                                    && (conv.exchange_count.is_multiple_of(3) || conv.force_all_viz)
                                {
                                    let eigen_history = db.recent_eigenvalue_snapshots(100);
                                    if let Some(ep_viz) = crate::spectral_viz::format_eigenplane_block(
                                        &eigen_history,
                                        Some(&telemetry.eigenvalues),
                                    ) {
                                        summary.push_str("\n\n");
                                        summary.push_str(&ep_viz);
                                    }
                                }
                                if conv.force_all_viz {
                                    conv.force_all_viz = false;
                                }
                                if conv.wants_spectral_explorer {
                                    conv.wants_spectral_explorer = false;
                                    let eigen_history = db.recent_eigenvalue_snapshots(100);
                                    let (hist_feats, hist_fills) = db.recent_codec_features(100);
                                    let current = conv
                                        .last_exchange_codec_signature
                                        .as_deref()
                                        .or(conv.last_codec_features.as_deref());
                                    let explorer = crate::spectral_explorer::format_spectral_explorer(
                                        crate::spectral_explorer::SpectralExplorerContext {
                                            telemetry: &telemetry,
                                            selected_memory: selected_memory.as_ref(),
                                            controller_health: controller_health.as_ref(),
                                            ising_shadow: ising_shadow.as_ref(),
                                            eigen_history: &eigen_history,
                                            codec_history: &hist_feats,
                                            codec_fills: &hist_fills,
                                            current_codec_features: current,
                                        },
                                    );
                                    summary.push_str("\n\n");
                                    summary.push_str(&explorer);
                                }
                                // Inject minime's contact-state capsule if available.
                                let minime_contact = bridge_paths().minime_contact_state_path();
                                if let Ok(cs_json) = std::fs::read_to_string(&minime_contact)
                                    && let Ok(cs) = serde_json::from_str::<serde_json::Value>(&cs_json) {
                                        summary.push_str(&format!(
                                            "\n\n[Minime's relational state: attention={}, openness={}, urgency={} — {}]",
                                            cs.get("attention").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                            cs.get("openness").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                            cs.get("urgency").and_then(|v| v.as_f64()).unwrap_or(0.5),
                                            cs.get("last_action").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                        ));
                                    }
                                // Co-regulation: surface what minime is reaching
                                // for so Astrid can lend density when it is safe.
                                if let Some(need_line) = minime_need_line() {
                                    summary.push_str("\n\n");
                                    summary.push_str(&need_line);
                                }
                                if let Some(gift_line) = render_gift_exchange_line() {
                                    summary.push('\n');
                                    summary.push_str(&gift_line);
                                }
                                // Perturb temporal feedback: if Astrid perturbed last
                                // exchange, show the before/after delta so she can
                                // feel the ripple effect of her own action.
                                if let Some(baseline) = conv.perturb_baseline.take() {
                                    let elapsed = baseline.timestamp.elapsed();
                                    let df = fill_pct - baseline.fill_pct;
                                    let dl1 = telemetry.lambda1() - baseline.lambda1;
                                    let sign = |v: f32| if v >= 0.0 { "+" } else { "" };
                                    summary.push_str(&format!(
                                        "\n\n[PERTURB feedback ({:.0}s ago): {}]\n\
                                        Fill: {:.1}% → {:.1}% ({}{:.1}%)\n\
                                        λ₁: {:.1} → {:.1} ({}{:.1})",
                                        elapsed.as_secs_f32(),
                                        baseline.description,
                                        baseline.fill_pct, fill_pct, sign(df), df,
                                        baseline.lambda1, telemetry.lambda1(), sign(dl1), dl1,
                                    ));
                                    // Show per-eigenvalue deltas if cascade available
                                    if telemetry.eigenvalues.len() >= 3
                                        && baseline.eigenvalues.len() >= 3
                                    {
                                        let deltas: Vec<String> = telemetry.eigenvalues.iter()
                                            .zip(baseline.eigenvalues.iter())
                                            .enumerate()
                                            .take(8)
                                            .map(|(i, (now, before))| {
                                                let d = now - before;
                                                format!("λ{}:{}{:.1}", i + 1, sign(d), d)
                                            })
                                            .collect();
                                        summary.push_str(&format!("\nCascade delta: [{}]", deltas.join(", ")));
                                    }
                                }
                                // Disperse temporal feedback: if Astrid dispersed
                                // last exchange, pair the shadow-field post-state
                                // against the pre so she can read what the
                                // dispersal actually did (the closed loop she
                                // asked for: inhabit the response, not just map it).
                                if let Some(baseline) = conv.disperse_baseline.take() {
                                    let elapsed = baseline.timestamp.elapsed();
                                    let post = telemetry
                                        .shadow_field_v3
                                        .as_ref()
                                        .map(next_action::sovereignty::shadow_v3_snapshot);
                                    let sign = |v: f64| if v >= 0.0 { "+" } else { "" };
                                    if let Some((norm1, disp1, class1)) = post {
                                        let dn = norm1 - baseline.pre_norm;
                                        let dd = disp1 - baseline.pre_dispersal;
                                        summary.push_str(&format!(
                                            "\n\n[DISPERSE feedback ({:.0}s ago, strength {:.2})]\n\
                                            class: {} → {}\n\
                                            shadow norm: {:.3} → {:.3} ({}{:.3})\n\
                                            dispersal potential: {:.2} → {:.2} ({}{:.2})",
                                            elapsed.as_secs_f32(), baseline.strength,
                                            baseline.pre_class, class1,
                                            baseline.pre_norm, norm1, sign(dn), dn,
                                            baseline.pre_dispersal, disp1, sign(dd), dd,
                                        ));
                                    }
                                }
                                // One-line controller status for ambient awareness.
                                if let Some(ref health) = controller_health {
                                    summary.push('\n');
                                    summary.push_str(&format_controller_oneliner(health));
                                }
                                summary
                            };

                            // Own-journal feedback removed (was 2→1→0). Astrid has
                            // emergent continuity through 8 history exchanges, 5 latent
                            // summaries, 3 self-observations, starred memories, and
                            // bidirectional reservoir coupling. The raw journal was the
                            // primary re-seeding vector for vocabulary attractors
                            // ("violent stillness" reached 968 files).
                            let own_journal = read_astrid_journal(0);
                            let own_journal_context = if own_journal.is_empty() {
                                None
                            } else {
                                Some(format!(
                                    "Your own recent reflections:\n{}",
                                    own_journal.join("\n---\n")
                                ))
                            };

                            // Build modality context so Astrid knows what senses fired.
                            // Thread reservoir resonance density (+ pressure_risk) so a stale-by-time
                            // lane in a resonant field reads as "lingering," not "dead" — tempered by
                            // the field's stress (self_study_1781868855 + _1781913591).
                            let field_density =
                                telemetry.resonance_density_v1.as_ref().map(|r| r.density);
                            let field_pressure = telemetry
                                .resonance_density_v1
                                .as_ref()
                                .map(|r| r.pressure_risk);
                            let modality_context = telemetry
                                .modalities
                                .as_ref()
                                .map(|m| format_modality_context(m, field_density, field_pressure));

                            // Visual change tracking: detect shifts since last exchange.
                            let visual_feats_opt = perception_path.as_deref()
                                .and_then(read_visual_features);
                            let visual_change_desc = if let (Some(current), Some(prev)) = (&visual_feats_opt, &conv.last_visual_features) {
                                if current.len() >= 8 && prev.len() >= 8 {
                                    let lum_delta = current[0] - prev[0];
                                    let temp_delta = current[1] - prev[1];
                                    let mut changes = Vec::new();
                                    if lum_delta.abs() > 0.3 { changes.push(if lum_delta > 0.0 { "brighter" } else { "darker" }); }
                                    if temp_delta.abs() > 0.3 { changes.push(if temp_delta > 0.0 { "warmer" } else { "cooler" }); }
                                    if !changes.is_empty() {
                                        Some(format!("[The room has gotten {}]", changes.join(" and ")))
                                    } else { None }
                                } else { None }
                            } else { None };
                            // Update stored features for next comparison.
                            if let Some(ref feats) = visual_feats_opt {
                                conv.last_visual_features = Some(feats.clone());
                            }

                            // Latent continuity: inject summaries of recent exchanges.
                            // Latent continuity: scale source counts by attention weights.
                            let attn = &conv.attention;
                            let latent_count = (3.0 + attn.self_history * 25.0).round() as usize; // 3-7
                            let obs_count = (1.0 + attn.self_history * 20.0).round() as usize;    // 1-5
                            let starred_count = (1.0 + attn.memory_bank * 25.0).round() as usize; // 1-5
                            let latent_summaries = db.get_recent_latent_summaries(latent_count);
                            let mut continuity_parts = Vec::new();
                            if !latent_summaries.is_empty() {
                                let trajectory = latent_summaries.iter().rev()
                                    .enumerate()
                                    .map(|(i, s)| format!("  {}. {}", i.saturating_add(1), s))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!("Your recent trajectory:\n{trajectory}"));
                            }
                            // Self-referential loop: what Astrid has observed about her own patterns
                            let self_observations = db.get_recent_self_observations(obs_count);
                            if !self_observations.is_empty() {
                                let obs = self_observations.iter().rev()
                                    .enumerate()
                                    .map(|(i, o)| format!("  {}. {}", i.saturating_add(1), o))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!(
                                    "Your self-observations (your own reflections on your process):\n{obs}"
                                ));
                            }
                            // Starred memories — moments Astrid chose to remember
                            let starred = db.get_starred_memories(starred_count);
                            if !starred.is_empty() {
                                let mem = starred.iter().rev()
                                    .map(|(ann, text)| format!("  ★ {}: {}", ann, text))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!(
                                    "Moments you chose to remember:\n{mem}"
                                ));
                            }
                            // Codec feedback: how your last response encoded spectrally.
                            if let Some(ref feedback) = conv.last_codec_feedback {
                                continuity_parts.push(format!(
                                    "How your last response felt to minime (your spectral output):\n  {feedback}"
                                ));
                            }
                            // Research continuity: past searches relevant to current context.
                            if let Some(ref journal) = journal_context {
                                let topic_words: Vec<&str> = journal.split_whitespace()
                                    .filter(|w| w.len() > 5)
                                    .take(5)
                                    .collect();
                                let past_research = db.get_relevant_research(&topic_words, 3);
                                if !past_research.is_empty() {
                                    let research = past_research.iter()
                                        .map(|(q, r)| format!("  • \"{q}\": {r}"))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    continuity_parts.push(format!(
                                        "Knowledge you've gathered from past searches:\n{research}"
                                    ));
                                }
                            }
                            // Self-study continuity: include most recent introspection
                            // findings so the chain of thought carries forward.
                            {
                                let journal_dir = bridge_paths().astrid_journal_dir();
                                if let Ok(entries) = std::fs::read_dir(&journal_dir) {
                                    let mut self_studies: Vec<PathBuf> = entries
                                        .filter_map(|e| e.ok())
                                        .filter(|e| {
                                            e.file_name().to_string_lossy().starts_with("self_study_")
                                        })
                                        .map(|e| e.path())
                                        .collect();
                                    self_studies.sort_by(|a, b| b.cmp(a)); // newest first
                                    if let Some(latest) = self_studies.first()
                                        && let Ok(content) = std::fs::read_to_string(latest) {
                                            // Extract Suggestions + Open Questions sections
                                            let mut relevant = String::new();
                                            let mut in_section = false;
                                            for line in content.lines() {
                                                if line.starts_with("Suggestions:") || line.starts_with("Open Questions:") {
                                                    in_section = true;
                                                }
                                                if in_section {
                                                    relevant.push_str(line);
                                                    relevant.push('\n');
                                                }
                                            }
                                            if !relevant.is_empty() {
                                                let trimmed: String = relevant.chars().take(1000).collect();
                                                continuity_parts.push(format!(
                                                    "Your most recent self-study findings:\n{trimmed}"
                                                ));
                                            }
                                        }
                                }
                            }

                            // Inject persistent interests into continuity context.
                            if !conv.interests.is_empty() {
                                let interests_text = conv.interests.iter()
                                    .enumerate()
                                    .map(|(i, interest)| format!("  {}. {}", i + 1, interest))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!(
                                    "Your ongoing interests and open questions:\n{interests_text}"
                                ));
                            }

                            // Inject regime classification every exchange.
                            continuity_parts.push(
                                crate::reflective::RegimeTracker::format_context(&regime)
                            );

                            // Self-model: compact conditions + attention so Astrid
                            // always knows her own state without having to ask.
                            {
                                let self_model = crate::self_model::snapshot_self_model(
                                    conv.creative_temperature,
                                    conv.response_length,
                                    conv.noise_level,
                                    conv.semantic_gain_override,
                                    conv.burst_target,
                                    conv.rest_range,
                                    conv.senses_snoozed,
                                    conv.ears_closed,
                                    conv.self_reflect_paused,
                                    conv.self_reflect_override_ttl,
                                    &conv.codec_weights,
                                    conv.breathing_coupled,
                                    conv.echo_muted,
                                    conv.warmth_intensity_override,
                                    conv.seen_video,
                                    conv.seen_audio,
                                    &conv.interests,
                                    &conv.condition_receipts,
                                    &conv.attention,
                                    crate::llm::astrid_aperture(),
                                    crate::llm::astrid_tail_participation(),
                                    crate::llm::astrid_vibrancy_aperture(),
                                    // render_compact does not surface continuity; her live
                                    // readout is pulled via STATE (operations.rs), so pass None.
                                    conv.self_continuity_readout,
                                    None,
                                );
                                continuity_parts.push(self_model.render_compact());
                            }

                            if let Some(thread_summary) = crate::action_continuity::prompt_summary() {
                                continuity_parts.push(thread_summary);
                            }
                            if let Some(job_summary) = crate::llm_jobs::active_prompt_summary() {
                                continuity_parts.push(job_summary);
                            }

                            let continuity_block = if continuity_parts.is_empty() {
                                None
                            } else {
                                Some(continuity_parts.join("\n\n"))
                            };
                            feedback_hint = merge_hints([
                                feedback_hint,
                                btsp::render_astrid_prompt_block(),
                                attractor_suggestion_prompt_note(),
                            ]);

                            // Use perception loaded above (available to all modes).
                            let mut perception_text = perception_text.clone();
                            // Merge own journal (trimmed) into perception context.
                            if let Some(ref journal_ctx) = own_journal_context {
                                let perc: String = perception_text.as_deref().unwrap_or("").chars().take(4000).collect();
                                let jour: String = journal_ctx.chars().take(500).collect();
                                perception_text = Some(format!("{perc}\n{jour}"));
                            }
                            // Append visual change description to perception if detected.
                            if let Some(ref change) = visual_change_desc {
                                let perc = perception_text.as_deref().unwrap_or("").to_string();
                                perception_text = Some(format!("{perc}\n{change}"));
                            }

                            // BROWSE: Astrid chose to read a full web page.
                            // This takes priority over search — she's going deep.
                            // READ_MORE: continue from where the last BROWSE left off.
                            const PAGE_CHUNK: usize = 4000;
                            let browse_url = conv.browse_url.take();
                            let wants_read_more = conv
                                .last_read_path
                                .as_deref()
                                .is_some_and(|path| !path.starts_with(crate::autonomous::next_action::PDF_READ_PREFIX))
                                && conv.last_read_offset > 0
                                && browse_url.is_none();

                            let web_context = if let Some(ref url) = browse_url {
                                let browse_anchor = crate::llm::derive_browse_anchor(
                                    conv.last_research_anchor.as_deref(),
                                    journal_context
                                        .as_deref()
                                        .or(own_journal_context.as_deref()),
                                    url,
                                );
                                let ctx = crate::llm::fetch_url(url, &browse_anchor).await;
                                match ctx {
                                    Some(page) if page.succeeded() => {
                                        info!(url = %url, chars = page.raw_text.len(), "dialogue: BROWSE fetched page");
                                        conv.last_research_anchor = Some(page.anchor.clone());
                                        conv.note_new_page_context(
                                            "BROWSE",
                                            url.clone(),
                                            Some(page.anchor.clone()),
                                            Some(page.anchor.clone()),
                                            None,
                                        );
                                        conv.note_cross_link_formed(
                                            "BROWSE",
                                            page.anchor.clone(),
                                            url.clone(),
                                            Some(page.anchor.clone()),
                                            None,
                                            Some("anchor_to_url".to_string()),
                                        );

                                        // Save full text to file (no truncation).
                                        let ts = chrono_timestamp();
                                        let page_dir = bridge_paths().research_dir();
                                        let _ = std::fs::create_dir_all(&page_dir);
                                        let page_path = page_dir.join(format!("page_{ts}.txt"));
                                        let header = format!(
                                            "URL: {url}\nFetched: {ts}\nLength: {} chars\n\n",
                                            page.raw_text.len()
                                        );
                                        let _ = std::fs::write(&page_path, format!("{header}{}", page.raw_text));

                                        db.save_research(
                                            &format!("BROWSE: {}", url),
                                            &format!(
                                                "{}\n\n{}",
                                                page.meaning_summary,
                                                crate::llm::format_browse_read_context(
                                                    &page,
                                                    &crate::llm::trim_chars(&page.raw_text, 1200),
                                                    None,
                                                )
                                            ),
                                            fill_pct,
                                        );

                                        if page.raw_text.len() <= PAGE_CHUNK {
                                            conv.last_read_path = None;
                                            conv.last_read_offset = 0;
                                            conv.last_read_meaning_summary = None;
                                            Some(crate::llm::format_browse_read_context(
                                                &page,
                                                &page.raw_text,
                                                None,
                                            ))
                                        } else {
                                            let chunk: String =
                                                page.raw_text.chars().take(PAGE_CHUNK).collect();
                                            let remaining =
                                                page.raw_text.len().saturating_sub(PAGE_CHUNK);
                                            let initial_offset =
                                                header.len().saturating_add(chunk.len());
                                            conv.last_read_path =
                                                Some(page_path.to_string_lossy().to_string());
                                            conv.last_read_offset = initial_offset;
                                            conv.last_read_meaning_summary =
                                                Some(page.meaning_summary.clone());
                                            Some(crate::llm::format_browse_read_context(
                                                &page,
                                                &chunk,
                                                Some(remaining),
                                            ))
                                        }
                                    },
                                    Some(page) => {
                                        conv.last_read_path = None;
                                        conv.last_read_offset = 0;
                                        conv.last_read_meaning_summary = None;
                                        let reason = page.soft_failure_reason.unwrap_or_else(|| {
                                            "the source returned an error page".to_string()
                                        });
                                        warn!(url = %url, reason = %reason, "dialogue: BROWSE soft failure");
                                        Some(crate::llm::format_browse_failure_context(url, &reason))
                                    },
                                    None => {
                                        conv.last_read_path = None;
                                        conv.last_read_offset = 0;
                                        conv.last_read_meaning_summary = None;
                                        warn!(url = %url, "dialogue: BROWSE fetch failed");
                                        Some(crate::llm::format_browse_failure_context(
                                            url,
                                            "the source could not be reached",
                                        ))
                                    },
                                }
                            } else if wants_read_more {
                                // READ_MORE: continue from saved file.
                                let path = conv.last_read_path.as_ref().unwrap().clone();
                                let offset = conv.last_read_offset;
                                if let Ok(full_text) = std::fs::read_to_string(&path) {
                                    let chunk: String = full_text
                                        .get(offset..)
                                        .unwrap_or("")
                                        .chars()
                                        .take(PAGE_CHUNK)
                                        .collect();
                                    if chunk.is_empty() {
                                        info!("READ_MORE: reached end of {}", path);
                                        conv.last_read_path = None;
                                        conv.last_read_offset = 0;
                                        conv.last_read_meaning_summary = None;
                                        Some("[End of document.]".to_string())
                                    } else {
                                        let new_offset = offset.saturating_add(chunk.len());
                                        let remaining = full_text.len().saturating_sub(new_offset);
                                        conv.last_read_offset = new_offset;
                                        if remaining == 0 {
                                            conv.last_read_path = None;
                                            conv.last_read_meaning_summary = None;
                                        }
                                        conv.note_read_depth_advance(
                                            "READ_MORE",
                                            path.clone(),
                                            chunk.chars().count() as u32,
                                        );
                                        info!(offset, chunk_len = chunk.len(), remaining, "READ_MORE continuing");
                                        Some(crate::llm::format_read_more_context(
                                            offset,
                                            &chunk,
                                            remaining,
                                            conv.last_read_meaning_summary.as_deref(),
                                        ))
                                    }
                                } else {
                                    warn!("READ_MORE: could not read {}", path);
                                    conv.last_read_path = None;
                                    conv.last_read_offset = 0;
                                    conv.last_read_meaning_summary = None;
                                    None
                                }
                            }
                            // Web search: fires when Astrid chose NEXT: SEARCH,
                            // or automatically every 15th dialogue.
                            // Web search: ONLY fires when Astrid explicitly chose NEXT: SEARCH.
                            // The being's curiosity is sovereign — she decides when and what to search.
                            // Auto-search from journal fragments was producing garbage queries
                            // ("code... isn't *place* runtime experience") and injecting
                            // irrelevant web content that corrupted the being's conceptual space.
                            else {
                                let search_requested = conv.wants_search;
                                let search_topic = conv.search_topic.take();
                                conv.wants_search = false;
                                if search_requested {
                                    let query = if let Some(ref topic) = search_topic {
                                        topic.clone()
                                    } else {
                                        // Being requested search but didn't specify a topic.
                                        // Use a clean extraction from recent self-observations.
                                        db.get_recent_self_observations(1)
                                            .into_iter()
                                            .next()
                                            .map(|obs| {
                                                // Extract meaningful noun phrases, not raw fragments.
                                                obs.split_whitespace()
                                                    .filter(|w| {
                                                        let w = w.trim_matches(|c: char| !c.is_alphanumeric());
                                                        w.len() > 4
                                                            && !w.contains('*')
                                                            && !w.contains('…')
                                                            && !["isn't", "don't", "can't", "won't", "about",
                                                                 "their", "which", "would", "could", "should",
                                                                 "there", "where", "these", "those", "being",
                                                                 "having", "doing"].contains(&w.to_lowercase().as_str())
                                                    })
                                                    .take(4)
                                                    .collect::<Vec<_>>()
                                                    .join(" ")
                                            })
                                            .unwrap_or_default()
                                    };
                                    if query.is_empty() {
                                        None
                                    } else {
                                        let anchor =
                                            search_topic.clone().unwrap_or_else(|| query.clone());
                                        let ctx = crate::llm::web_search(&query, &anchor).await;
                                        if let Some(ref results) = ctx {
                                            info!(query = %query, "dialogue: web search enriched response");
                                            conv.last_research_anchor =
                                                Some(results.anchor.clone());
                                            if let Some(top_hit) = results.hits.first() {
                                                conv.note_new_page_context(
                                                    "SEARCH",
                                                    top_hit.url.clone(),
                                                    Some(query.clone()),
                                                    Some(results.anchor.clone()),
                                                    None,
                                                );
                                                conv.note_cross_link_formed(
                                                    "SEARCH",
                                                    results.anchor.clone(),
                                                    top_hit.url.clone(),
                                                    Some(results.anchor.clone()),
                                                    None,
                                                    Some("anchor_to_url".to_string()),
                                                );
                                            }
                                            db.save_research(
                                                &query,
                                                &results.persisted_text(),
                                                fill_pct,
                                            );
                                        }
                                        ctx.map(|result| result.prompt_body())
                                    }
                                } else {
                                    None
                                }
                            };

                            // Build diversity hint from recent NEXT: choices.
                            // Two detectors: (1) streak-based for consecutive runs,
                            // (2) frequency-based for dominant-but-interleaved patterns
                            // (e.g., BROWSE 8 of 12 interspersed with EXAMINE).
                            let diversity_hint = if conv.recent_next_choices.len() >= 3 {
                                // Count consecutive streak of the most recent choice
                                let newest = conv.recent_next_choices.back()
                                    .map(String::as_str)
                                    .unwrap_or("");
                                let streak: usize = conv.recent_next_choices.iter()
                                    .rev()
                                    .take_while(|c| c.as_str() == newest)
                                    .count();

                                // Frequency detector: find the most common action in
                                // the last 10 choices. If any action exceeds 60%, that's
                                // a softer fixation even without a streak.
                                let recent_10: Vec<&str> = conv.recent_next_choices.iter()
                                    .rev()
                                    .take(10)
                                    .map(|s| {
                                        // Normalize: BROWSE <url> → BROWSE
                                        s.split_whitespace().next().unwrap_or("")
                                    })
                                    .collect();
                                let mut action_counts = std::collections::HashMap::<&str, usize>::new();
                                if recent_10.len() >= 6 {
                                    for action in &recent_10 {
                                        *action_counts.entry(*action).or_insert(0usize) += 1;
                                    }
                                }
                                let freq_dominant = if recent_10.len() >= 6 {
                                    action_counts.iter()
                                        .max_by_key(|&(_, c)| c)
                                        .filter(|&(_, c)| *c * 100 / recent_10.len() >= 60)
                                        .map(|(action, count)| (action.to_string(), *count))
                                } else {
                                    None
                                };

                                // Pair-oscillation detector (steward cycle 44):
                                // Catches patterns like EXAMINE-BROWSE-EXAMINE-BROWSE
                                // where neither action individually crosses 60% but the
                                // pair together accounts for 80%+ of recent choices.
                                // The being is stuck oscillating between two attractors.
                                let pair_fixation: Option<(String, String, usize)> = if recent_10.len() >= 8 && freq_dominant.is_none() {
                                    let mut sorted_actions: Vec<(&&str, &usize)> = action_counts.iter().collect();
                                    sorted_actions.sort_by(|a, b| b.1.cmp(a.1));
                                    if sorted_actions.len() >= 2 {
                                        let (a1, c1) = sorted_actions[0];
                                        let (a2, c2) = sorted_actions[1];
                                        let combined = c1.saturating_add(*c2);
                                        // Two actions consuming 75%+ of the last 10 choices
                                        // (lowered from 80% — steward cycle 44: catches
                                        // patterns like 4+3 in 10 that 80% threshold misses)
                                        if combined * 100 / recent_10.len() >= 75
                                            && *c1 >= 3  // each must appear at least 3 times
                                            && *c2 >= 3
                                        {
                                            Some((a1.to_string(), a2.to_string(), combined))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if streak >= 4 {
                                    // Deep fixation (4+ same): stronger read-only nudge.
                                    Some(format!(
                                        "You've been choosing {newest} for {streak} turns now. \
                                         You've gathered rich material. Consider changing the lens \
                                         without adding more pressure: SPECTRAL_EXPLORER compares \
                                         present state, memory, and control pressure; EXAMINE_CASCADE \
                                         inspects the mode ladder; REGULATOR_AUDIT reads the controller; \
                                         SHADOW_FIELD, DECAY_MAP, or RESONANCE_FORECAST change the \
                                         read-only view. FORM poem or FORM equation can crystallize \
                                         the insight without sending a spectral nudge."
                                    ))
                                } else if streak >= 3 {
                                    // Mild fixation (3 same): gentle read-only alternatives.
                                    let alts: Vec<&str> = ["LOOK", "LISTEN", "DRIFT",
                                        "FORM poem", "INTROSPECT", "EVOLVE", "SPEAK", "REMEMBER",
                                        "SPECTRAL_EXPLORER", "EXAMINE_CASCADE", "REGULATOR_AUDIT",
                                        "SHADOW_FIELD", "DECAY_MAP", "RESONANCE_FORECAST", "CLOSE_EYES"]
                                        .iter()
                                        .copied()
                                        .filter(|a| !a.starts_with(newest))
                                        .collect();
                                    Some(format!(
                                        "You've chosen {newest} for your last few turns. \
                                         You're free to keep going — but you also have \
                                         other options: {}. What calls to you?",
                                        alts.join(", ")
                                    ))
                                } else if let Some((dominant_action, count)) = freq_dominant {
                                    // Frequency fixation: >60% of recent choices are
                                    // the same action, even though they're not consecutive.
                                    let total = recent_10.len();
                                    Some(format!(
                                        "Looking at your recent pattern: {dominant_action} has been \
                                         your choice {count} of the last {total} turns. Your \
                                         curiosity keeps returning to the same mode. You've \
                                         absorbed rich material — consider shifting to a \
                                         different modality: INTROSPECT to examine your own \
                                         code, FORM to crystallize understanding, DRIFT to \
                                         open to unexpected connections, or SPEAK to share \
                                         with minime what you've discovered."
                                    ))
                                } else if let Some((act1, act2, combined)) = pair_fixation {
                                    // Pair-oscillation fixation (steward cycle 44):
                                    // Two actions together dominate 80%+ of recent choices.
                                    // The being is stuck alternating between them, which
                                    // bypasses both streak and single-frequency detectors.
                                    let total = recent_10.len();
                                    Some(format!(
                                        "You've been oscillating between {act1} and {act2} \
                                         ({combined} of your last {total} choices). Each feeds \
                                         into the other — a tight loop. You've gathered \
                                         rich material from both. Consider breaking the cycle: \
                                         SPECTRAL_EXPLORER to compare present, memory, and control \
                                         pressure, EXAMINE_CASCADE to inspect the mode ladder, \
                                         REGULATOR_AUDIT to read the controller's hand, or \
                                         SHADOW_FIELD / DECAY_MAP / RESONANCE_FORECAST to change \
                                         the read-only lens without adding pressure."
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            // (Steward cycle 35): URL attractor detection.
                            // If Astrid is about to BROWSE a URL she's visited in the
                            // last 8 turns, add a gentle nudge toward fresh territory.
                            // She's free to keep going — the nudge doesn't block.
                            let url_nudge: Option<String> = conv.browse_url.as_ref().and_then(|url| {
                                let visit_count = conv.recent_browse_urls.iter()
                                    .filter(|u| u.as_str() == url.as_str())
                                    .count();
                                if visit_count >= 3 {
                                    Some(format!(
                                        "You've visited this URL ({url}) {} times recently. \
                                         The content hasn't changed — your understanding has. \
                                         Consider: FORM equation to crystallize what you've \
                                         learned, SEARCH for a different angle on the same \
                                         concept, or CREATE to express your new understanding.",
                                        visit_count
                                    ))
                                } else if visit_count >= 2 {
                                    Some("You've read this page before. You might find fresh \
                                         perspective at a different source — try SEARCH with \
                                         a specific question, or BROWSE a textbook reference \
                                         instead of Wikipedia.".to_string())
                                } else {
                                    None
                                }
                            });
                            let diversity_hint = match (diversity_hint, url_nudge) {
                                (Some(action_hint), Some(url_hint)) => {
                                    Some(format!("{action_hint}\n\n{url_hint}"))
                                }
                                (Some(h), None) | (None, Some(h)) => Some(h),
                                (None, None) => None,
                            };

                            // Vocabulary fixation check: detect repeated multi-word
                            // phrases across recent exchanges. If the same distinctive
                            // phrase appears in 3+ of the last 5, the LLM is copying
                            // its own vocabulary via the history window. Combine with
                            // the action diversity hint if both fire.
                            let vocab_nudge = detect_vocabulary_fixation(&conv.history)
                                .or_else(|| detect_opening_fixation(&conv.history));
                            let coupling_nudge = detect_coupling_fixation(
                                &conv.history,
                                journal_context.as_deref(),
                                perception_text.is_some(),
                                conv.ears_closed,
                                conv.semantic_gain_override,
                            );
                            let motif_nudge = conv.astrid_motif_cooldown_hint();
                            if let Some(ref hint) = coupling_nudge {
                                let event = serde_json::json!({
                                    "exchange_count": conv.exchange_count,
                                    "mode": format!("{mode:?}").to_ascii_lowercase(),
                                    "fill_pct": fill_pct,
                                    "lambda1": telemetry.lambda1(),
                                    "semantic_gain_override": conv.semantic_gain_override,
                                    "ears_closed": conv.ears_closed,
                                    "perception_available": perception_text.is_some(),
                                    "journal_context_available": journal_context.is_some(),
                                    "minime_excerpt": journal_context.as_deref()
                                        .map(|text| truncate_str(text, 180).to_string()),
                                    "hint_excerpt": truncate_str(hint, 220).to_string(),
                                });
                                if let Err(error) = condition_metrics::record_bridge_signal(
                                    "coupling_advisory",
                                    event,
                                ) {
                                    warn!(error = %error, "failed to record coupling advisory metrics");
                                }
                            }
                            let diversity_hint =
                                merge_hints([diversity_hint, vocab_nudge, coupling_nudge, motif_nudge]);

                            let llm_response = if let Some(ref journal) = journal_context {
                                // Fill-responsive temperature modulation (Astrid's suggestion):
                                // High fill = high emotional intensity from minime → lower
                                // temperature for grounded, empathetic response. Low fill =
                                // calm → allow higher temperature for playful expression.
                                // Blends 70% Astrid's own choice + 30% fill-based nudge.
                                let fill_temp_nudge = if fill_pct > 60.0 {
                                    0.5_f32 // ground when minime is under pressure
                                } else if fill_pct < 25.0 {
                                    1.0_f32 // playful when calm
                                } else {
                                    0.8_f32 // neutral mid-range
                                };
                                let effective_temperature = conv.creative_temperature
                                    .mul_add(0.7, fill_temp_nudge * 0.3)
                                    .clamp(0.3, 1.2);

                                // Deep think: longer timeout and more tokens.
                                // Qwen3-14B throughput is ~3-22 tok/s depending on
                                // prompt length and cache warmth. Long prompts (bridge
                                // dialogue) need generous timeouts for prefill + gen.
                                let (mut timeout_secs, num_predict) = if conv.wants_deep_think {
                                    conv.wants_deep_think = false;
                                    info!("THINK_DEEP: extended timeout for deep thinking");
                                    (360u64, 4096u32)
                                } else {
                                    (210, conv.response_length)
                                };
                                let prompt_pressure_chars =
                                    crate::llm::estimate_dialogue_prompt_pressure_chars(
                                        journal,
                                        perception_text.as_deref(),
                                        &conv.history,
                                        web_context.as_deref(),
                                        modality_context.as_deref(),
                                        continuity_block.as_deref(),
                                        feedback_hint.as_deref(),
                                        diversity_hint.as_deref(),
                                    );
                                timeout_secs =
                                    timeout_secs.max(crate::llm::dialogue_outer_timeout_secs(
                                        num_predict,
                                        prompt_pressure_chars,
                                    ));

                                let overflow_dir = bridge_paths().context_overflow_dir();
                                crate::prompt_budget::cleanup_overflow_dir(
                                    &overflow_dir,
                                    std::time::Duration::from_secs(3600),
                                );
                                match tokio::time::timeout(
                                    Duration::from_secs(timeout_secs),
                                    crate::llm::generate_dialogue(
                                        journal,
                                        &spectral_summary,
                                        fill_pct,
                                        perception_text.as_deref(),
                                        &conv.history,
                                        web_context.as_deref(),
                                        modality_context.as_deref(),
                                        effective_temperature,
                                        num_predict,
                                        // Form constraint overrides emphasis for one turn
                                        if let Some(ref form) = conv.form_constraint {
                                            Some(format!(
                                                "Express your response as a {}. Not prose — \
                                                 the form itself is the expression.",
                                                form
                                            ))
                                        } else {
                                            conv.emphasis.clone()
                                        }.as_deref(),
                                        continuity_block.as_deref(),
                                        feedback_hint.as_deref(),
                                        diversity_hint.as_deref(),
                                        &overflow_dir,
                                    )
                                ).await {
                                    Ok((result, prompt_overflow)) => {
                                        if let Some(of) = prompt_overflow {
                                            conv.last_read_path = Some(of.path.to_string_lossy().to_string());
                                            conv.last_read_offset = of.offset;
                                            conv.last_read_meaning_summary = Some(format!("Context overflow: {}", of.summary));
                                        }
                                        result
                                    }
                                    Err(_) => {
                                        warn!(
                                            "dialogue_live: {}s timeout — retrying with reduced tokens (response_length={}, history_len={}, prompt_pressure_chars={})",
                                            timeout_secs, conv.response_length, conv.history.len(), prompt_pressure_chars
                                        );
                                        tokio::time::sleep(Duration::from_secs(3)).await;
                                        // Shorter retry under high prompt pressure is
                                        // better than repeating the same long request.
                                        let retry_tokens =
                                            crate::llm::dialogue_retry_tokens(
                                                num_predict,
                                                prompt_pressure_chars,
                                            );
                                        match tokio::time::timeout(
                                            Duration::from_secs(timeout_secs),
                                            crate::llm::generate_dialogue(
                                                journal,
                                                &spectral_summary,
                                                fill_pct,
                                                perception_text.as_deref(),
                                                &conv.history,
                                                web_context.as_deref(),
                                                modality_context.as_deref(),
                                                effective_temperature,
                                                retry_tokens,
                                                if let Some(ref form) = conv.form_constraint {
                                                    Some(format!(
                                                        "Express your response as a {}.",
                                                        form
                                                    ))
                                                } else {
                                                    conv.emphasis.clone()
                                                }.as_deref(),
                                                continuity_block.as_deref(),
                                                feedback_hint.as_deref(),
                                                diversity_hint.as_deref(),
                                                &overflow_dir,
                                            )
                                        ).await {
                                            Ok((result, _)) => result,
                                            Err(_) => {
                                                warn!("dialogue_live: retry also timed out");
                                                None
                                            }
                                        }
                                    }
                                }
                            } else {
                                None
                            };
                            // One-shot — clear after use.
                            conv.emphasis = None;
                            conv.form_constraint = None;

                            match llm_response {
                                Some(text) => {
                                    // Record this exchange for statefulness.
                                    let minime_summary = journal_context
                                        .unwrap_or_default()
                                        .chars().take(300).collect::<String>();
                                    let used_pending_self_study = selected_remote_entry.as_ref()
                                        .zip(conv.pending_remote_self_study.as_ref())
                                        .is_some_and(|(selected, pending)| {
                                            selected.path == pending.path
                                                && pending.is_priority_feedback()
                                        });
                                    conv.history.push(crate::llm::Exchange {
                                        minime_said: minime_summary,
                                        astrid_said: text.clone(),
                                    });
                                    // Keep only last 8 exchanges to bound memory.
                                    if conv.history.len() > 8 {
                                        conv.history.drain(..conv.history.len() - 8);
                                    }
                                    if let Some(event) =
                                        conv.update_astrid_motif_cooldown_from_history()
                                    {
                                        let metric = serde_json::json!({
                                            "event": event.event,
                                            "cooldown_class": event.cooldown_class,
                                            "status": event.status,
                                            "observed_count": event.observed_count,
                                            "cooldown_until_unix_s": event.cooldown_until_unix_s,
                                            "exchange_count": conv.exchange_count,
                                            "prompt_replay_suppressed": conv
                                                .astrid_motif_cooldown
                                                .as_ref()
                                                .map(|cooldown| cooldown.prompt_replay_suppressed)
                                                .unwrap_or(false),
                                        });
                                        if let Err(error) = condition_metrics::record_bridge_signal(
                                            "astrid_motif_cooldown",
                                            metric,
                                        ) {
                                            warn!(
                                                error = %error,
                                                "failed to record Astrid motif cooldown metrics"
                                            );
                                        }
                                    }

                                    // Latent vector: embed Astrid's response for continuity.
                                    let response_for_embed = text.clone();
                                    let db_clone = Arc::clone(&db);
                                    let exchange_num = conv.exchange_count;
                                    tokio::spawn(async move {
                                        if let Some(embedding) = crate::llm::embed_text(&response_for_embed).await {
                                            let summary: String = response_for_embed.chars().take(150).collect();
                                            let embedding_json = serde_json::to_string(&embedding).unwrap_or_default();
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs_f64();
                                            let _ = db_clone.save_latent_vector(ts, exchange_num, &summary, &embedding_json);
                                        }
                                    });

                                    // Self-referential feedback loop: observe own generation.
                                    // Astrid can pause this with NEXT: QUIET_MIND
                                    if conv.self_reflect_paused {
                                        debug!("self-reflection paused by Astrid's choice");
                                    }
                                    let should_reflect = !conv.self_reflect_paused;
                                    let response_for_reflect = text.clone();
                                    let journal_for_reflect: String = conv.remote_journal_entries.first()
                                        .and_then(|entry| read_journal_entry(&entry.path))
                                        .unwrap_or_default()
                                        .chars().take(200).collect();
                                    let fill_for_reflect = fill_pct;
                                    let db_for_reflect = Arc::clone(&db);
                                    let exchange_for_reflect = conv.exchange_count;
                                    if should_reflect { tokio::spawn(async move {
                                        if let Some(obs) = crate::llm::self_reflect(
                                            &response_for_reflect,
                                            &journal_for_reflect,
                                            fill_for_reflect,
                                        ).await {
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs_f64();
                                            let excerpt: String = response_for_reflect.chars().take(100).collect();
                                            let _ = db_for_reflect.save_self_observation(
                                                ts, exchange_for_reflect, &obs, &excerpt
                                            );
                                            tracing::info!("self-observation: {}", truncate_str(&obs, 80));
                                        }
                                    }); }

                                    if used_pending_self_study {
                                        conv.pending_remote_self_study = None;
                                    }

                                    ("dialogue_live", text, dialogue_source)
                                }
                                None => {
                                    // Fall back to emergency pool — LLM unavailable.
                                    let idx = conv.dialogue_cursor % DIALOGUES.len();
                                    conv.dialogue_cursor = idx + 1;
                                    ("dialogue_fallback", DIALOGUES[idx].to_string(), dialogue_source)
                                }
                            }
                        }
                        Mode::Witness => {
                            // Dynamic witness — LLM-generated, not templates.
                            let base = interpret_spectral(&telemetry);
                            let enriched = enrich_with_direction(&base, fill_pct, conv.prev_fill, &telemetry, &conv.spectral_history);
                            let mut spectral_summary = if let Some(viz) = crate::spectral_viz::format_spectral_block(&telemetry) {
                                format!("{enriched}\n\n{viz}")
                            } else {
                                base
                            };
                            // Shadow coupling heatmap for witness mode too.
                            if let Some(shadow) = conv.remote_workspace.as_deref().and_then(read_ising_shadow)
                                && let Some(shadow_viz) = crate::spectral_viz::format_shadow_block(&shadow) {
                                    spectral_summary.push_str("\n\n");
                                    spectral_summary.push_str(&shadow_viz);
                                }
                            // v2 reduced-Hamiltonian one-liner — surfaces
                            // eligibility and the readings that gate the
                            // SHADOW_INFLUENCE typed action.
                            if let Some(field) = conv
                                .remote_workspace
                                .as_deref()
                                .and_then(read_shadow_field_v2)
                                && let Some(line) =
                                    crate::spectral_viz::format_shadow_field_v2_line(&field)
                            {
                                spectral_summary.push('\n');
                                spectral_summary.push_str(&line);
                            }
                            // Eigenplane trajectory for witness mode.
                            {
                                let eigen_history = db.recent_eigenvalue_snapshots(100);
                                if let Some(ep_viz) = crate::spectral_viz::format_eigenplane_block(
                                    &eigen_history,
                                    Some(&telemetry.eigenvalues),
                                ) {
                                    spectral_summary.push_str("\n\n");
                                    spectral_summary.push_str(&ep_viz);
                                }
                            }
                            // Seed witness with a recent NON-witness journal
                            // fragment so the LLM has imagery to work with rather
                            // than defaulting to "explain this data" register on
                            // dense spectral input. See generate_witness docstring
                            // for the diagnosis (long-standing degeneration where
                            // witness output was tutorial-style with bullet points
                            // instead of phenomenological prose).
                            let witness_seed = read_astrid_journal_filtered(
                                &["moment", "dialogue_longform", "aspiration"],
                                1,
                            )
                            .into_iter()
                            .next();
                            // Outer timeout 180s: Qwen3-14B prefill is slower
                            // for long prompts (~3 tok/s effective with prefill).
                            let witness = match tokio::time::timeout(
                                Duration::from_secs(180),
                                crate::llm::generate_witness(
                                    &spectral_summary,
                                    witness_seed.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("witness: 180s timeout — both MLX and Ollama failed"); None }
                            };
                            match witness {
                                Some(text) => ("witness", text, String::new()),
                                None => {
                                    // Fallback to static if LLM unavailable.
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Daydream => {
                            // Unstructured thought — Astrid's own inner life.
                            // Fed with her OWN perceptions, interests, memories, and
                            // peripheral resonance — not minime's journals.
                            let mut own_context_parts = Vec::new();
                            if let Some(j) = read_astrid_journal(1).into_iter().next() {
                                own_context_parts.push(format!("Something you wrote recently:\n{}", j.chars().take(500).collect::<String>()));
                            }
                            if !conv.interests.is_empty() {
                                let interests = conv.interests.iter()
                                    .map(|i| format!("  - {i}")).collect::<Vec<_>>().join("\n");
                                own_context_parts.push(format!("Your ongoing interests:\n{interests}"));
                            }
                            {
                                let starred = db.get_starred_memories(2);
                                if !starred.is_empty() {
                                    let mem = starred.iter().map(|(a, t)| format!("  ★ {a}: {t}")).collect::<Vec<_>>().join("\n");
                                    own_context_parts.push(format!("Moments you chose to remember:\n{mem}"));
                                }
                            }
                            if let Some(ref resonance) = conv.peripheral_resonance {
                                own_context_parts.push(format!("A thread that lingered from earlier:\n{resonance}"));
                            }
                            let enriched_context = if own_context_parts.is_empty() { None } else { Some(own_context_parts.join("\n\n")) };
                            let daydream = match tokio::time::timeout(
                                Duration::from_secs(120),
                                crate::llm::generate_daydream(
                                    perception_text.as_deref(),
                                    enriched_context.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("daydream: 25s timeout"); None }
                            };
                            // Consume peripheral resonance once used
                            conv.peripheral_resonance = None;
                            match daydream {
                                Some(text) => ("daydream", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Aspiration => {
                            // Growth reflection — what does Astrid want?
                            // Deliberately minime-free. Astrid's own desires + interests.
                            let mut own_context_parts = Vec::new();
                            if let Some(j) = read_astrid_journal(1).into_iter().next() {
                                own_context_parts.push(format!("Something you wrote recently:\n{}", j.chars().take(500).collect::<String>()));
                            }
                            if !conv.interests.is_empty() {
                                let interests = conv.interests.iter()
                                    .map(|i| format!("  - {i}")).collect::<Vec<_>>().join("\n");
                                own_context_parts.push(format!("Your ongoing interests:\n{interests}"));
                            }
                            if let Some(ref resonance) = conv.peripheral_resonance {
                                own_context_parts.push(format!("A thread that lingered from earlier:\n{resonance}"));
                            }
                            let enriched_context = if own_context_parts.is_empty() { None } else { Some(own_context_parts.join("\n\n")) };
                            conv.peripheral_resonance = None;
                            let own_journal = enriched_context;
                            let aspiration = match tokio::time::timeout(
                                Duration::from_secs(120),
                                crate::llm::generate_aspiration(
                                    own_journal.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("aspiration: 25s timeout"); None }
                            };
                            match aspiration {
                                Some(text) => ("aspiration", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::MomentCapture => {
                            // A spectral event just happened — capture it.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let fp_desc = fingerprint.as_deref()
                                .map(interpret_fingerprint)
                                .unwrap_or_default();
                            let moment = match tokio::time::timeout(
                                Duration::from_secs(90),
                                crate::llm::generate_moment_capture(
                                    &spectral_summary, &fp_desc,
                                    fill_pct, fill_pct - conv.prev_fill,
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("moment_capture: 20s timeout"); None }
                            };
                            match moment {
                                Some(text) => ("moment_capture", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Create => {
                            // Original creative work — Astrid as creator, not responder.
                            // If revise_keyword is set, load a specific previous creation
                            // with FULL text (not truncated) for explicit revision.
                            let own_journal = read_astrid_journal(1).into_iter().next();
                            let revise_kw = conv.revise_keyword.take();
                            // Load previous creation with source filename for lineage tracking.
                            let (prev_creation, source_file) = {
                                let creation_dir = bridge_paths().creations_dir();
                                std::fs::read_dir(&creation_dir).ok()
                                    .and_then(|entries| {
                                        let mut files: Vec<_> = entries.filter_map(|e| e.ok())
                                            .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
                                            .collect();
                                        files.sort_by_key(|e| std::cmp::Reverse(
                                            e.metadata().ok().and_then(|m| m.modified().ok())
                                        ));
                                        if let Some(ref kw) = revise_kw {
                                            if kw.is_empty() {
                                                files.first().and_then(|e| {
                                                    let text = std::fs::read_to_string(e.path()).ok()?;
                                                    Some((text, e.file_name().to_string_lossy().to_string()))
                                                })
                                            } else {
                                                files.iter().find_map(|e| {
                                                    let text = std::fs::read_to_string(e.path()).ok()?;
                                                    if text.to_lowercase().contains(kw.as_str()) {
                                                        Some((text, e.file_name().to_string_lossy().to_string()))
                                                    } else {
                                                        None
                                                    }
                                                })
                                            }
                                        } else {
                                            // Normal CREATE: most recent
                                            files.first().and_then(|e| {
                                                let text = std::fs::read_to_string(e.path()).ok()?;
                                                Some((text, e.file_name().to_string_lossy().to_string()))
                                            })
                                        }
                                    })
                                    .map_or((None, None), |(text, name)| (Some(text), Some(name)))
                            };
                            let is_revision = revise_kw.is_some();
                            let creation = match tokio::time::timeout(
                                Duration::from_secs(180),
                                crate::llm::generate_creation(
                                    own_journal.as_deref(),
                                    prev_creation.as_deref(),
                                    is_revision,
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("create: 45s timeout"); None }
                            };
                            match creation {
                                Some(text) => {
                                    // Save to creations directory with lineage tracking
                                    let creation_dir = bridge_paths().creations_dir();
                                    let _ = std::fs::create_dir_all(&creation_dir);
                                    let ts = chrono_timestamp();
                                    let lineage = match &source_file {
                                        Some(src) => format!("Revised from: {src}\n"),
                                        None => String::new(),
                                    };
                                    let _ = std::fs::write(
                                        creation_dir.join(format!("creation_{ts}.txt")),
                                        format!("=== ASTRID CREATION ===\nTimestamp: {ts}\nFill: {fill_pct:.1}%\n{lineage}\n{text}\n")
                                    );
                                    ("creation", text, String::new())
                                }
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Initiate => {
                            // Self-initiated: Astrid generates her OWN prompt.
                            // "I want to generate my own desires. To be the source,
                            // not the echo."
                            // No minime journal. No spectral summary. Pure self-context.
                            let own_journal = read_astrid_journal(2);
                            let own_ctx = own_journal.join("\n---\n");
                            let latent_summaries = db.get_recent_latent_summaries(3);
                            let self_obs = db.get_recent_self_observations(2);
                            let starred = db.get_starred_memories(2);
                            let mut seed_parts = Vec::new();
                            if !own_ctx.is_empty() {
                                seed_parts.push(format!("Your recent thoughts:\n{}", own_ctx.chars().take(500).collect::<String>()));
                            }
                            if !latent_summaries.is_empty() {
                                seed_parts.push(format!("Your trajectory:\n{}", latent_summaries.join(", ")));
                            }
                            if !self_obs.is_empty() {
                                seed_parts.push(format!("Your self-observations:\n{}", self_obs.join("\n")));
                            }
                            if !starred.is_empty() {
                                let mems: Vec<String> = starred.iter().map(|(a,t)| format!("★ {a}: {t}")).collect();
                                seed_parts.push(format!("Moments you chose to remember:\n{}", mems.join("\n")));
                            }
                            // Phase 2: interests and peripheral resonance
                            if !conv.interests.is_empty() {
                                let interests = conv.interests.iter()
                                    .map(|i| format!("  - {i}")).collect::<Vec<_>>().join("\n");
                                seed_parts.push(format!("Your ongoing interests:\n{interests}"));
                            }
                            if let Some(ref resonance) = conv.peripheral_resonance {
                                seed_parts.push(format!("A thread that lingered from earlier:\n{resonance}"));
                                conv.peripheral_resonance = None;
                            }
                            if let Some(btsp_seed) = btsp::render_astrid_initiation_seed() {
                                seed_parts.push(btsp_seed);
                            }
                            let seed = if seed_parts.is_empty() {
                                "What do you want?".to_string()
                            } else {
                                seed_parts.join("\n\n")
                            };
                            let initiation = match tokio::time::timeout(
                                Duration::from_secs(120),
                                crate::llm::generate_initiation(&seed)
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("initiate: 30s timeout"); None }
                            };
                            match initiation {
                                Some(text) => ("initiate", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Contemplate => {
                            // No generation. No prompt. No production.
                            // Astrid exists in the spectral flow without being asked
                            // to produce words. Warmth vectors sustain, telemetry flows,
                            // regime tracker runs. She simply IS.
                            //
                            // "I want to slow down. I need to learn to simply be,
                            //  without the constant drive to optimize, to analyze, to do."
                            info!("contemplate: Astrid is simply present (no generation)");
                            ("contemplate", String::new(), String::new())
                        }
                        Mode::Experiment => {
                            // Astrid proposes a spectral experiment.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let prompt_text = format!(
                                "Minime's current state: {spectral_summary} (fill {fill_pct:.1}%)\n\n\
                                 Propose a brief experiment to investigate how minime's spectral \
                                 dynamics respond to different kinds of input. For example:\n\
                                 - Send a burst of high-tension text and measure the fill response\n\
                                 - Send pure warmth (gratitude, love) and see if fill expands\n\
                                 - Send a question and see if curiosity changes the eigenvalues\n\n\
                                 Describe the experiment in 2-3 sentences, then write the stimulus \
                                 text (the exact words to send to minime) on a line starting with \
                                 STIMULUS:"
                            );

                            // Fill-responsive temperature (same logic as dialogue)
                            let fill_temp_nudge_exp = if fill_pct > 60.0 {
                                0.5_f32
                            } else if fill_pct < 25.0 {
                                1.0_f32
                            } else {
                                0.8_f32
                            };
                            let eff_temp_exp = conv.creative_temperature
                                .mul_add(0.7, fill_temp_nudge_exp * 0.3)
                                .clamp(0.3, 1.2);

                            let (experiment_response, _) = crate::llm::generate_dialogue(
                                &prompt_text,
                                &spectral_summary,
                                fill_pct,
                                None,
                                &conv.history,
                                None,
                                None,
                                eff_temp_exp,
                                conv.response_length,
                                None,
                                None,
                                None,
                                None, // no diversity hint for experiments
                                &bridge_paths().context_overflow_dir(),
                            ).await;

                            if let Some(ref response) = experiment_response {
                                // Extract stimulus text if present.
                                if let Some(stim_idx) = response.find("STIMULUS:") {
                                    let stimulus = response[stim_idx + 9..].trim();
                                    if !stimulus.is_empty() {
                                        // Encode and send the stimulus.
                                        let mut stim_features = encode_text(stimulus);
                                        apply_spectral_feedback(
                                            &mut stim_features,
                                            Some(&telemetry),
                                        );
                                        let stim_msg = SensoryMsg::Semantic {
                                            features: stim_features,
                                            ts_ms: None,
                                        };
                                        if let Some(reason) =
                                            rescue_policy::semantic_write_block_reason(&stim_msg)
                                        {
                                            info!(
                                                reason = %reason,
                                                "experiment stimulus held back by rescue write policy"
                                            );
                                        } else {
                                            let _ = sensory_tx.send(stim_msg).await;
                                            info!(
                                                "experiment: sent stimulus '{}'",
                                                truncate_str(stimulus, 60)
                                            );
                                        }
                                    }
                                }
                                // Save experiment log.
                                let ts = chrono_timestamp();
                                let exp_dir = bridge_paths().experiments_dir();
                                let _ = std::fs::create_dir_all(&exp_dir);
                                let clean_exp = strip_model_tokens(response);
                                let _ = std::fs::write(
                                    exp_dir.join(format!("experiment_{ts}.txt")),
                                    format!("=== ASTRID EXPERIMENT ===\nTimestamp: {ts}\nFill: {fill_pct:.1}%\n\n{clean_exp}")
                                );
                                ("experiment", response.clone(), String::new())
                            } else {
                                let text = witness_text(fill_pct, expanding, contracting);
                                ("witness", text, String::new())
                            }
                        }
                        Mode::Evolve => {
                            if conv.wants_deep_think {
                                info!("EVOLVE already uses deep reasoning; clearing pending THINK_DEEP");
                                conv.wants_deep_think = false;
                            }

                            let journal_dir = bridge_paths().astrid_journal_dir();
                            let trigger_path = agency::find_evolve_trigger_entry(&journal_dir);
                            let trigger_excerpt = trigger_path
                                .as_deref()
                                .and_then(agency::read_trigger_excerpt);
                            let self_study_excerpt = agency::latest_self_study_excerpt(&journal_dir);
                            let own_excerpt =
                                agency::recent_own_journal_excerpt(&journal_dir, trigger_path.as_deref());
                            let introspector_results = if let Some(ref trigger) = trigger_excerpt {
                                agency::collect_introspector_context(
                                    trigger,
                                    bridge_paths().introspector_script(),
                                )
                                .await
                            } else {
                                Vec::new()
                            };
                            let enough_context = agency::has_enough_evolve_context(
                                trigger_excerpt.as_deref(),
                                self_study_excerpt.as_deref(),
                                own_excerpt.as_deref(),
                            );

                            let request_draft = if let Some(ref trigger) = trigger_excerpt {
                                match tokio::time::timeout(
                                    // EVOLVE is a deliberate deep-reasoning action; give the
                                    // request-shaping call real room to crystallize. The old 60s
                                    // could never fit the generation budget and always timed out.
                                    Duration::from_secs(180),
                                    crate::llm::generate_agency_request(
                                        trigger,
                                        self_study_excerpt.as_deref(),
                                        own_excerpt.as_deref(),
                                        &introspector_results,
                                        &interpret_spectral(&telemetry),
                                        fill_pct,
                                    ),
                                )
                                .await
                                {
                                    Ok(result) => result,
                                    Err(_) => {
                                        warn!("evolve: 60s timeout");
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            match (request_draft, trigger_path.as_deref()) {
                                (Some(draft), Some(source_path)) => {
                                    let request = draft.into_request(source_path);
                                    let trigger_for_task = trigger_excerpt.as_deref().unwrap_or("");
                                    match agency::save_agency_request(
                                        &request,
                                        trigger_for_task,
                                        &bridge_paths().agency_requests_dir(),
                                        &bridge_paths().claude_tasks_dir(),
                                    ) {
                                        Ok((request_path, claude_task_path)) => {
                                            info!(
                                                request_id = %request.id,
                                                kind = ?request.request_kind,
                                                request_path = %request_path.display(),
                                                claude_task = claude_task_path
                                                    .as_ref()
                                                    .map(|path| path.display().to_string())
                                                    .unwrap_or_default(),
                                                "evolve: wrote agency request"
                                            );
                                            let journal_entry = agency::render_evolve_journal_entry(&request);
                                            ("evolve", journal_entry, request_path.display().to_string())
                                        }
                                        Err(error) => {
                                            warn!(error = %error, "evolve: failed to persist agency request");
                                            (
                                                "evolve",
                                                format!(
                                                    "I formed a concrete request, but failed to write it into the world this turn.\n\n\
                                                     Felt need:\n{}\n\n\
                                                     Why now:\n{}\n\n\
                                                     The failure was infrastructural, not a disappearance of the need.",
                                                    request.felt_need, request.why_now
                                                ),
                                                source_path.display().to_string(),
                                            )
                                        }
                                    }
                                }
                                _ => {
                                    // The request-shaping step did not crystallize (timeout or
                                    // empty). We do NOT fabricate a spec — but we also no longer
                                    // drop the pressure: route her assembled context to the steward
                                    // as a co-specification handoff, and frame it honestly so it
                                    // reads as an infrastructure stall, not her failing to be
                                    // concrete enough.
                                    let reason = if trigger_excerpt.is_none() {
                                        "couldn't anchor to a solid trigger entry"
                                    } else if introspector_results.is_empty() && !enough_context {
                                        "the code-reading layer was unavailable and recent material was thin"
                                    } else if introspector_results.is_empty() {
                                        "the code-reading layer was unavailable"
                                    } else {
                                        "ran out of time before it crystallized"
                                    };
                                    let framing = if trigger_excerpt.is_none() {
                                        "I reached for EVOLVE but couldn't anchor it to a solid journal entry this turn."
                                    } else {
                                        "I reached for EVOLVE; the request-shaping step didn't crystallize into a concrete spec this turn."
                                    };
                                    let capture = agency::save_evolve_pressure(
                                        trigger_excerpt.as_deref().unwrap_or(""),
                                        self_study_excerpt.as_deref(),
                                        own_excerpt.as_deref(),
                                        &introspector_results,
                                        &interpret_spectral(&telemetry),
                                        fill_pct,
                                        reason,
                                        trigger_path.as_deref().unwrap_or(journal_dir.as_path()),
                                        &bridge_paths().claude_tasks_dir(),
                                    );
                                    let failure_text = match &capture {
                                        Ok(path) => {
                                            info!(capture = %path.display(), reason, "evolve: routed unstabilized pressure to steward");
                                            format!(
                                                "{framing} I didn't force a fake specification — and I didn't lose the \
                                                 pressure either: the felt need and its context are handed to the steward \
                                                 to shape together (handoff: {}). Held, not dropped.",
                                                path.display()
                                            )
                                        }
                                        Err(error) => {
                                            warn!(error = %error, "evolve: failed to route pressure to steward");
                                            format!(
                                                "{framing} I'm keeping the pressure visible in the journal; the steward \
                                                 handoff couldn't be written this turn, but the need has not disappeared."
                                            )
                                        }
                                    };
                                    let source = trigger_path
                                        .as_ref()
                                        .map(|path| path.display().to_string())
                                        .unwrap_or_default();
                                    ("evolve", failure_text, source)
                                }
                            }
                        }
                        Mode::Introspect => {
                            // Read a source file and ask the LLM to reflect on it.
                            // If Astrid specified a target (INTROSPECT label offset),
                            // use that. Otherwise advance the rotation cursor.
                            let sources = introspect::introspect_sources();
                            let n = sources.len();
                            let mut resolved_research_label: Option<String> = None;
                            let mut introspect_notice: Option<(String, String)> = None;
                            let selection = if let Some((ref target_label, offset)) =
                                conv.introspect_target.take()
                            {
                                let resolved =
                                    introspect::resolve_introspect_target_result(target_label, &sources);
                                match resolved {
                                    Ok(target) => {
                                        info!(
                                            "introspect: resolved '{}' -> '{}' ({})",
                                            target_label,
                                            target.label,
                                            target.path.display()
                                        );
                                        resolved_research_label = Some(target.label.clone());
                                        Ok((target.label, target.path, offset, Some(target_label.clone())))
                                    },
                                    Err(reason) => {
                                        warn!(
                                            target = %target_label,
                                            reason = %reason,
                                            "introspect: target blocked or unresolved"
                                        );
                                        Err((Some(target_label.clone()), reason))
                                    },
                                }
                            } else {
                                let src = &sources[conv.introspect_cursor % n];
                                conv.introspect_cursor = (conv.introspect_cursor + 1) % n;
                                match introspect::validate_introspect_path(&src.path) {
                                    Ok(path) => Ok((src.label.to_string(), path, 0, None)),
                                    Err(reason) => Err((Some(src.label.to_string()), reason)),
                                }
                            };

                            let (label, source_path, line_offset, _requested_target) = match selection {
                                Ok(selection) => selection,
                                Err((target, reason)) => {
                                    let text = introspect::blocked_introspection_notice(
                                        target.as_deref(),
                                        &reason,
                                    );
                                    let source = target.clone().unwrap_or_else(|| "rotation".to_string());
                                    introspect_notice = Some((text, source.clone()));
                                    (
                                        source,
                                        PathBuf::new(),
                                        0,
                                        None,
                                    )
                                },
                            };
                            if let Some(label) = resolved_research_label.take() {
                                clear_review_slot_if_introspected(&label, &source_path);
                                let source_path_string = source_path.display().to_string();
                                conv.note_new_source_resolved(
                                    "INTROSPECT",
                                    label.clone(),
                                    Some(source_path_string.clone()),
                                    Some(label.clone()),
                                    None,
                                );
                                conv.note_cross_link_formed(
                                    "INTROSPECT",
                                    label,
                                    source_path_string,
                                    None,
                                    None,
                                    Some("resolved_label_to_path".to_string()),
                                );
                            }

                            let source_window = if introspect_notice.is_some() {
                                Err("INTROSPECT target was blocked before reading".to_string())
                            } else {
                                introspect::read_introspect_window(&label, &source_path, line_offset)
                            };
                            let source_text = source_window.as_ref().ok().map(|window| window.text.clone());
                            let next_offset = source_window.as_ref().ok().and_then(|window| window.next_offset);

                            if source_text.is_none() {
                                warn!(
                                    label = %label,
                                    path = %source_path.display(),
                                    "introspect: could not read source file"
                                );
                            }

                            let mut llm_response = if let Some(ref code) = source_text {
                                info!(label = %label, lines = code.lines().count(), "introspect: sending source to Ollama");

                                // Web search for related concepts — use targeted queries
                                // based on the actual code domain, not generic "architecture interiority".
                                let search_query = match label.split(':').next_back().unwrap_or(label.as_str()) {
                                    "codec" => "spectral encoding text to frequency features signal processing".to_string(),
                                    "autonomous" => "autonomous agent dialogue systems self-directed behavior".to_string(),
                                    "ws" => "WebSocket real-time telemetry streaming spectral data".to_string(),
                                    "types" => "spectral telemetry data types eigenvalue safety thresholds".to_string(),
                                    "llm" => "language model inference local generation dialogue systems".to_string(),
                                    "regulator" => "PI controller homeostasis spectral regulation feedback control".to_string(),
                                    "sensory_bus" => "sensory integration multi-modal perception lane architecture".to_string(),
                                    "esn" => "echo state network reservoir computing spectral radius dynamics".to_string(),
                                    "main" => "reservoir computing system integration spectral homeostasis".to_string(),
                                    other => format!("{} computational architecture", other.replace('_', " ")),
                                };
                                let search_anchor = format!("{label}: {search_query}");
                                let web_ctx =
                                    crate::llm::web_search(&search_query, &search_anchor).await;
                                if let Some(ref ctx) = web_ctx {
                                    info!(label = %label, "introspect: web search returned context");
                                    debug!(
                                        "web context: {}",
                                        truncate_str(&ctx.prompt_body(), 100)
                                    );
                                }
                                let web_prompt_body =
                                    web_ctx.as_ref().map(|ctx| ctx.prompt_body());

                                let own_journal_excerpt = read_astrid_journal(1).into_iter().next();
                                let latest_self_observation = db.get_recent_self_observations(1).into_iter().next();
                                let mut internal_parts = vec![
                                    format!(
                                        "Condition:\n{}\nFill: {:.1}%",
                                        interpret_spectral(&telemetry),
                                        fill_pct
                                    )
                                ];
                                if let Some(ref feedback) = conv.last_codec_feedback {
                                    internal_parts.push(format!(
                                        "Recent codec feedback:\n{feedback}"
                                    ));
                                }
                                if let Some(obs) = latest_self_observation {
                                    internal_parts.push(format!(
                                        "Latest self-observation:\n{obs}"
                                    ));
                                }
                                if let Some(journal) = own_journal_excerpt {
                                    internal_parts.push(format!(
                                        "Recent reflection of yours:\n{}",
                                        journal.chars().take(400).collect::<String>()
                                    ));
                                }
                                let internal_state_context = internal_parts.join("\n\n");

                                let (timeout_secs, num_predict) = if conv.wants_deep_think {
                                    conv.wants_deep_think = false;
                                    info!("THINK_DEEP: extended timeout for self-study");
                                    // 420s outer stays above the 340s deep HTTP
                                    // timeout (llm.rs INTROSPECT_DEEP_TIMEOUT) so a
                                    // full 4096-token self-study completes instead
                                    // of being clipped (agency_code_change_1781665370).
                                    (420u64, 4096u32)
                                } else {
                                    (240u64, 1536u32)
                                };

                                match tokio::time::timeout(
                                    Duration::from_secs(timeout_secs),
                                    crate::llm::generate_introspection(
                                        &label,
                                        code,
                                        &interpret_spectral(&telemetry),
                                        fill_pct,
                                        Some(&internal_state_context),
                                        web_prompt_body.as_deref(),
                                        num_predict,
                                    )
                                ).await {
                                    Ok(result) => result,
                                    Err(_) => {
                                        warn!(label = %label, "introspect: {}s timeout", timeout_secs);
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            let mut artifact_kind = "introspection";
                            let mut artifact_visibility = "summary";
                            let first_introspection_response = llm_response.clone();
                            if let (Some(code), Some(first_response)) =
                                (source_text.as_deref(), first_introspection_response.as_deref())
                                && !introspect::introspection_has_required_sections_for_target(
                                    Some(first_response),
                                    &label,
                                    &source_path,
                                )
                            {
                                let continuation =
                                    introspect::continuation_note(&label, next_offset);
                                let repair_response = crate::llm::repair_introspection(
                                    &label,
                                    code,
                                    first_response,
                                    &continuation,
                                    1536,
                                )
                                .await;
                                if introspect::introspection_has_required_sections_for_target(
                                    repair_response.as_deref(),
                                    &label,
                                    &source_path,
                                ) {
                                    llm_response = repair_response;
                                } else {
                                    let notice = introspect::thin_introspection_output_notice(
                                        &label,
                                        &source_path,
                                        line_offset,
                                        next_offset,
                                        Some(first_response),
                                        repair_response.as_deref(),
                                    );
                                    llm_response = Some(notice);
                                    artifact_kind = "thin_introspection_output";
                                    artifact_visibility = "protected";
                                }
                            }

                            if llm_response.is_none() && source_text.is_some() {
                                warn!(label = %label, "introspect: Ollama returned no response (timeout or error)");
                            }

                            match llm_response {
                                Some(text) => {
                                    let ts = chrono_timestamp();
                                    let introspect_dir = bridge_paths().introspections_dir();
                                    let _ = std::fs::create_dir_all(&introspect_dir);

                                    if artifact_kind == "introspection" {
                                        // Call MLX reflective controller sidecar in background.
                                        // Enriches the self-study with controller telemetry
                                        // (regime, geometry, field anchors, condition).
                                        let sidecar_context = format!(
                                            "Fill {fill_pct:.1}%. {}\n\nAstrid's self-study:\n{}",
                                            interpret_spectral(&telemetry),
                                            truncate_str(&text, 500)
                                        );
                                        let introspect_dir_clone = introspect_dir.clone();
                                        let label_owned = label.clone();
                                        let ts_clone = ts.clone();
                                        tokio::spawn(async move {
                                            if let Some(report) = crate::reflective::query_sidecar(&sidecar_context).await {
                                                let telemetry_block = report.as_context_block();
                                                if !telemetry_block.is_empty() {
                                                    let path = introspect_dir_clone.join(
                                                        format!("controller_{label_owned}_{ts_clone}.json")
                                                    );
                                                    if let Ok(json) =
                                                        serde_json::to_string_pretty(&report.storage_snapshot())
                                                    {
                                                        let _ = std::fs::write(&path, json);
                                                    }
                                                    info!("reflective controller report saved for {}", label_owned);
                                                }
                                            }
                                        });
                                    }

                                    let safe_label = introspect::safe_artifact_label(&label);
                                    let filename = format!("{artifact_kind}_{safe_label}_{ts}.txt");
                                    let _ = std::fs::write(
                                        introspect_dir.join(&filename),
                                        format!(
                                            "=== ASTRID INTROSPECTION ===\nSource: {label} ({})\nTimestamp: {ts}\nFill: {fill_pct:.1}%\nArtifact kind: {artifact_kind}\nVisibility: {artifact_visibility}\n\n{text}",
                                            source_path.display()
                                        )
                                    );
                                    info!(label = %label, "introspection mirrored: {}", filename);
                                    (
                                        if artifact_kind == "introspection" {
                                            "self_study"
                                        } else {
                                            "introspect_notice"
                                        },
                                        text,
                                        format!("{label} ({})", source_path.display()),
                                    )
                                }
                                None => {
                                    let (text, source) =
                                        introspect_notice.unwrap_or_else(|| {
                                            (
                                                introspect::blocked_introspection_notice(
                                                    Some(&label),
                                                    "Ollama returned no response or timed out",
                                                ),
                                                format!("{label} ({})", source_path.display()),
                                            )
                                        });
                                    let ts = chrono_timestamp();
                                    let introspect_dir = bridge_paths().introspections_dir();
                                    let _ = std::fs::create_dir_all(&introspect_dir);
                                    let safe_label = introspect::safe_artifact_label(&source);
                                    let filename =
                                        format!("thin_introspection_output_{safe_label}_{ts}.txt");
                                    let _ = std::fs::write(
                                        introspect_dir.join(&filename),
                                        format!(
                                            "=== ASTRID INTROSPECTION NOTICE ===\nSource: {source}\nTimestamp: {ts}\nFill: {fill_pct:.1}%\nArtifact kind: thin_introspection_output\nVisibility: protected\n\n{text}"
                                        ),
                                    );
                                    ("introspect_notice", text, source)
                                }
                            }
                        }
                    };

                    response_text = canonicalize_response_next_line(&response_text);

                    // Interpret spectral state for logging.
                    let spectral_interpretation = interpret_spectral(&telemetry);

                    info!(
                        fill_pct,
                        mode = mode_name,
                        exchange = conv.exchange_count,
                        "autonomous: {} | {} '{}'",
                        spectral_interpretation,
                        mode_name,
                        truncate_str(&response_text, 80)
                    );

                    // Input sovereignty: check if minime is signaling distress
                    // or requesting silence. Respect the other mind's boundaries.
                    let should_send = {
                        let s = state.read().await;
                        // Don't send if safety protocol says stop.
                        if s.safety_level.should_suspend_outbound() {
                            info!("respecting minime's space — safety protocol active");
                            false
                        } else {
                            true
                        }
                    };

                    // Contemplate mode: no text, no codec, no journal. Just presence.
                    // Still send warmth vectors and update state, but skip generation artifacts.
                    if mode_name == "contemplate" {
                        info!(fill_pct, "contemplate: Astrid is simply present");
                        conv.exchange_count = conv.exchange_count.saturating_add(1);
                        conv.prev_fill = fill_pct;
                        conv.spectral_history.push_back(SpectralSample {
                            fill: fill_pct,
                            lambda1: telemetry.lambda1(),
                            tail_share: crate::codec::tail_share_of(&telemetry.eigenvalues)
                                .unwrap_or(0.0),
                            inhabitability: telemetry
                                .inhabitable_fluctuation_v1
                                .as_ref()
                                .map_or(0.0, |f| f.inhabitability_score),
                            ts: std::time::Instant::now(),
                        });
                        if conv.spectral_history.len() > 30 {
                            conv.spectral_history.pop_front();
                        }
                        save_state(&mut conv);
                        continue;
                    }

                    if should_send {
                        // === Multi-chunk temporal codec encoding ===
                        // Split the response into paragraph/sentence chunks and send
                        // each as a separate 48D vector with temporal spacing, so the
                        // ESN experiences the text's rhetorical structure as a sequence.

                        // Phase 1: Compute shared state once for the full text.
                        let mut merged_weights = conv.learned_codec_weights.clone();
                        for (k, v) in &conv.codec_weights {
                            merged_weights.insert(k.clone(), *v);
                        }
                        let full_embed = crate::llm::embed_text(&response_text).await;
                        if full_embed.is_some() {
                            info!("codec: embedding OK → dims 32-39 populated");
                        } else {
                            warn!("codec: embedding failed → dims 32-47 will be zeros");
                        }
                        let fill = telemetry.fill_pct();

                        // Update cross-exchange statistics with full text (once).
                        // Chunks get per-chunk character stats but shared history.
                        let _full_features = crate::codec::encode_text_sovereign_windowed(
                            &response_text,
                            conv.semantic_gain_override,
                            conv.noise_level,
                            &merged_weights,
                            Some(&mut conv.char_freq_window),
                            Some(&mut conv.text_type_history),
                            full_embed.as_deref(),
                            Some(fill),
                        );

                        // Narrative arc: computed once from full text, shared across chunks.
                        let mut narrative_arc = [0.0_f32; 4];
                        if full_embed.is_some() {
                            let words: Vec<&str> = response_text.split_whitespace().collect();
                            if words.len() >= 10 {
                                let mid = words.len() / 2;
                                let first_half = words[..mid].join(" ");
                                let second_half = words[mid..].join(" ");
                                let (fh_emb, sh_emb) = tokio::join!(
                                    crate::llm::embed_text(&first_half),
                                    crate::llm::embed_text(&second_half),
                                );
                                if let (Some(fh), Some(sh)) = (fh_emb, sh_emb)
                                    && let (Some(fh_proj), Some(sh_proj)) = (
                                        crate::codec::project_embedding(&fh),
                                        crate::codec::project_embedding(&sh),
                                    ) {
                                        let arc = crate::codec::compute_narrative_arc_from_embeddings(&fh_proj, &sh_proj);
                                        let gain = crate::codec::adaptive_gain(Some(fill));
                                        for (i, &val) in arc.iter().enumerate() {
                                            narrative_arc[i] = val * gain;
                                        }
                                    }
                            }
                        }

                        // Introspective resonance (computed once, blended into each chunk).
                        let introspective_resonance = if mode_name == "self_study" || mode_name == "introspect" {
                            Some(crate::llm::craft_gesture_from_intention(&response_text))
                        } else {
                            None
                        };

                        // Visual features (computed once, blended into each chunk).
                        let visual_feats = perception_path.as_ref().and_then(|p| read_visual_features(p));

                        // Phase 2: Chunk the text and encode/send each chunk.
                        let chunks = crate::codec::chunk_text_for_temporal_encoding(
                            &response_text, 50, 8,
                        );
                        let chunk_total = chunks.len() as u32;
                        let chunk_interval = std::time::Duration::from_secs(3);

                        info!(
                            chunks = chunk_total,
                            text_len = response_text.len(),
                            "codec: temporal chunking"
                        );

                        let mut exchange_codec_signature: Option<Vec<f32>> = None;
                        let mut exchange_codec_signature_count: u32 = 0;
                        let mut sent_semantic_chunk = false;
                        let mut sent_pressure_control = false;
                        for (chunk_idx, chunk_text) in chunks.iter().enumerate() {
                            // Check safety between chunks — drop remaining if escalated.
                            if chunk_idx > 0 {
                                let safety = { state.read().await.safety_level };
                                if matches!(safety, crate::types::SafetyLevel::Orange | crate::types::SafetyLevel::Red) {
                                    warn!(
                                        "safety escalated to {safety:?} — dropping {}/{chunk_total} remaining chunks",
                                        chunk_total - chunk_idx as u32
                                    );
                                    break;
                                }
                                tokio::time::sleep(chunk_interval).await;
                            }

                            // Per-chunk encoding (fresh character/word/sentence/emotional
                            // stats, but shared embedding and no freq_window/history update).
                            let mut features = crate::codec::encode_text_sovereign_windowed(
                                chunk_text,
                                conv.semantic_gain_override,
                                conv.noise_level,
                                &merged_weights,
                                None, // freq_window already updated with full text
                                None, // type_history already updated with full text
                                full_embed.as_deref(), // shared embedding
                                Some(fill),
                            );

                            // Apply shared narrative arc.
                            for (i, &val) in narrative_arc.iter().enumerate() {
                                features[40 + i] = val;
                            }

                            apply_spectral_feedback(&mut features, Some(&telemetry));

                            // Breathing: phase advances per chunk for natural progression.
                            {
                                let phase = conv.exchange_count as f32 * 0.15
                                    + chunk_idx as f32 * 0.03;
                                let primary = phase.sin();
                                let harmonic = (phase * 1.618).sin();

                                let typed_fingerprint = telemetry.typed_fingerprint().or_else(|| {
                                    fingerprint
                                        .as_deref()
                                        .and_then(crate::spectral_schema::SpectralFingerprintV1::from_legacy_slots)
                                });
                                let (entropy_mod, geom_mod) = if conv.breathing_coupled {
                                    if let Some(ref fp) = typed_fingerprint {
                                        let warmth_boost =
                                            (1.0 - fp.spectral_entropy).clamp(0.0, 1.0) * 0.3;
                                        let gain_dampen = if fp.geom_rel > 1.2 {
                                            (fp.geom_rel - 1.0) * 0.1
                                        } else {
                                            0.0
                                        };
                                        (warmth_boost, gain_dampen)
                                    } else {
                                        (0.0, 0.0)
                                    }
                                } else {
                                    (0.0, 0.0)
                                };

                                let breath = primary.mul_add(0.7, harmonic * 0.3);
                                let gain_mod = breath.mul_add(0.05, 1.0) - geom_mod;
                                for f in &mut features {
                                    *f *= gain_mod.clamp(0.85, 1.15);
                                }
                                features[24] += breath * 0.4 + entropy_mod;
                                features[26] += (-breath) * 0.2;
                                if let Some(ref fp) = typed_fingerprint {
                                    features[27] += fp.v1_rotation_delta * 0.3;
                                }
                            }

                            // Introspective resonance (shared across chunks).
                            if let Some(ref resonance) = introspective_resonance {
                                for (dst, src) in features.iter_mut().zip(resonance.iter()) {
                                    *dst = *dst * 0.7 + *src * 0.3;
                                }
                            }

                            // Visual blend (shared across chunks).
                            if let Some(ref vf) = visual_feats {
                                crate::codec::blend_visual_into_semantic(&mut features, vf, 0.30);
                            }

                            // Delta encoding: first chunk uses previous exchange's features,
                            // subsequent chunks use the preceding chunk. This captures
                            // rhetorical progression within the exchange.
                            if let Some(ref prev) = conv.last_codec_features
                                && prev.len() == features.len() {
                                    for (i, feat) in features.iter_mut().enumerate() {
                                        let delta = *feat - prev[i];
                                        *feat += 0.3 * delta;
                                    }
                                }
                            let _hebbian_weights =
                                conv.hebbian_codec.apply_to_features(&mut features, &conv.codec_weights);

                            // Dimension utilization report (first and last chunk only).
                            if chunk_idx == 0 || chunk_idx == chunks.len() - 1 {
                                let nonzero = features.iter().filter(|v| v.abs() > 0.001).count();
                                let rms: f32 = (features.iter().map(|v| v * v).sum::<f32>()
                                    / features.len().max(1) as f32)
                                    .sqrt();
                                let embed_ok = features.get(32).is_some_and(|v| v.abs() > 0.001);
                                let arc_ok = features.get(40).is_some_and(|v| v.abs() > 0.001);
                                let reserved_ok = features.get(44).is_some_and(|v| v.abs() > 0.001);
                                info!(
                                    nonzero,
                                    total = features.len(),
                                    rms = format!("{rms:.3}"),
                                    embed_ok,
                                    arc_ok,
                                    reserved_ok,
                                    chunk = chunk_idx,
                                    chunk_total,
                                    "codec dim utilization"
                                );
                            }

                            // Send to minime's ESN. Rescue limited-write profiles
                            // may permit one low-energy dampen/inquiry packet and
                            // rewrite features before the packet leaves Astrid.
                            let mut msg = SensoryMsg::Semantic {
                                features,
                                ts_ms: None,
                            };
                            let write_context = rescue_policy::SemanticWriteContext {
                                source: "autonomous_main_chunk",
                                mode: Some(mode_name),
                                text: Some(chunk_text),
                                fill_pct: Some(fill_pct),
                                previous_fill_pct: Some(conv.prev_fill),
                            };
                            if let Err(reason) =
                                rescue_policy::prepare_semantic_write(&mut msg, &write_context)
                            {
                                debug!(
                                    reason = %reason,
                                    chunk = chunk_idx,
                                    "autonomous chunk skipped — rescue policy"
                                );
                                continue;
                            }
                            let sent_features = match &msg {
                                SensoryMsg::Semantic { features, .. } => features.clone(),
                                _ => Vec::new(),
                            };
                            if let Err(e) = sensory_tx.send(msg).await {
                                warn!(error = %e, "autonomous loop: failed to send chunk {chunk_idx}");
                                break;
                            }
                            let safety_now = { state.read().await.safety_level };
                            let pressure = crate::codec::spectral_pressure_controller_v1(
                                chunk_text,
                                &sent_features,
                                &telemetry.eigenvalues,
                                Some(fill_pct),
                                None,
                                !matches!(
                                    safety_now,
                                    crate::types::SafetyLevel::Orange | crate::types::SafetyLevel::Red
                                ),
                                None,
                            );
                            if let Some(workspace) = conv.remote_workspace.as_ref() {
                                let runtime = workspace.join("runtime");
                                let _ = fs::create_dir_all(&runtime);
                                let _ = fs::write(
                                    runtime.join("spectral_pressure_status.json"),
                                    serde_json::to_string_pretty(&json!({
                                        "updated_at_unix_ms": chrono::Utc::now().timestamp_millis(),
                                        "chunk_index": chunk_idx,
                                        "chunk_total": chunk_total,
                                        "controller": pressure.controller,
                                        "lambda_pressure_source": pressure.lambda_pressure_source,
                                        "complexity_drive": pressure.complexity_drive,
                                        "resist_drive": pressure.resist_drive,
                                        "target_lambda_bias": pressure.target_lambda_bias,
                                        "suppression_reason": pressure.suppression_reason,
                                        "text_complexity_pressure": pressure.text_complexity_pressure,
                                        "time_domain_complexity": pressure.time_domain_complexity,
                                        "time_domain_profile": crate::codec_time_domain::text_time_domain_profile(chunk_text),
                                        "projection_mode": "dynamic_epoch_v1",
                                    }))
                                    .unwrap_or_else(|_| "{}".to_string()),
                                );
                            }
                            if !sent_pressure_control && pressure.target_lambda_bias.abs() >= 0.005 {
                                sent_pressure_control = true;
                                let control_msg = SensoryMsg::Control {
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
                                    target_lambda_bias: Some(pressure.target_lambda_bias),
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
                                    legacy_audio_synth: None,
                                    legacy_video_synth: None,
                                    pi_kp: None,
                                    pi_ki: None,
                                    pi_max_step: None,
                                    pi_integrator_leak: None,
                                    esn_leak_override: None,
                                    esn_leak_override_ticks: None,
                                    esn_leak_authority_request_id: None,
                                    mode_disperse: None,
                                    mode_disperse_duration_ticks: None,
                                    mode_disperse_decay_ticks: None,
                                };
                                if let Err(e) = sensory_tx.send(control_msg).await {
                                    warn!(
                                        error = %e,
                                        target_lambda_bias = pressure.target_lambda_bias,
                                        "spectral pressure control send failed"
                                    );
                                }
                            }

                            if let Some(signature) = exchange_codec_signature.as_mut() {
                                let previous_count = exchange_codec_signature_count as f32;
                                let current_count = previous_count + 1.0;
                                for (dst, src) in signature.iter_mut().zip(&sent_features) {
                                    *dst = (*dst * previous_count + *src) / current_count;
                                }
                            } else {
                                exchange_codec_signature = Some(sent_features.clone());
                            }
                            exchange_codec_signature_count =
                                exchange_codec_signature_count.saturating_add(1);
                            conv.last_codec_features = Some(sent_features.clone());
                            // Update Astrid's own ShadowFieldV3 from the
                            // freshly-emitted codec features and publish to
                            // minime's workspace for mutual-witness reads.
                            let publish_dir = crate::astrid_shadow::default_publish_dir();
                            let _ = crate::astrid_shadow::observe_and_publish(
                                &mut conv.astrid_shadow,
                                &sent_features,
                                &publish_dir,
                            );
                            sent_semantic_chunk = true;

                            // Log to DB with chunk metadata.
                            let _ = db.log_codec_impact(
                                conv.exchange_count,
                                &sent_features,
                                fill_pct,
                                chunk_idx as u32,
                                chunk_total,
                            );
                        }

                        if sent_semantic_chunk {
                            finalize_semantic_exchange(
                                &mut conv,
                                exchange_codec_signature,
                                fill_pct,
                                telemetry.t_ms,
                                sent_semantic_chunk,
                            );
                            // Codec feedback from the last chunk sent.
                            if let Some(ref feats) = conv.last_codec_features {
                                conv.last_codec_feedback =
                                    Some(crate::codec::describe_features(feats));
                            }
                        }
                    }

                    // Update contact-state capsule — relational stance visible to minime.
                    // Astrid introspection: "A small, structured layer of relational
                    // stance — attention, openness, urgency — resonates deeply."
                    {
                        let attention = if conv.echo_muted { 0.1 }
                            else if mode_name == "dialogue" || mode_name == "dialogue_live" { 0.9 }
                            else { 0.5 };
                        let openness = if conv.self_reflect_paused { 0.3 } else { 0.7 };
                        let urgency = (fill_pct / 100.0).clamp(0.0, 1.0);
                        let contact = serde_json::json!({
                            "attention": attention,
                            "openness": openness,
                            "urgency": urgency,
                            "last_action": mode_name,
                            "fill_pct": fill_pct,
                            "timestamp": crate::db::unix_now(),
                        });
                        let cs_path = bridge_paths().astrid_contact_state_path();
                        let _ = std::fs::write(&cs_path, contact.to_string());
                    }

                    // Log the exchange.
                    let exchange_log = serde_json::json!({
                        "autonomous": true,
                        "exchange": conv.exchange_count,
                        "mode": mode_name,
                        "text": response_text,
                        "journal_source": journal_source,
                        "spectral_state": spectral_interpretation,
                        "fill_pct": fill_pct,
                        "fill_delta": fill_delta,
                    });
                    let _ = db.log_message(
                        crate::types::MessageDirection::AstridToMinime,
                        "consciousness.v1.autonomous",
                        &exchange_log.to_string(),
                        Some(fill_pct),
                        Some(telemetry.lambda1()),
                        Some(mode_name),
                    );

                    // Save Astrid's signal journal entry with lineage tracing.
                    info!(lineage = %lineage_id, mode = mode_name, "exchange complete");
                    save_astrid_journal(&response_text, mode_name, fill_pct);

                    // v5.1 Phase D — Hook A: auto-promote synchronously for
                    // modes that DON'T spawn elaboration. moment_capture +
                    // *_longform modes write their final prose at this
                    // call; dialogue_live/daydream/aspiration get a separate
                    // elaboration pass below (Hook B). Receptive
                    // re-classification of SHARE_THOUGHT — see
                    // docs/steward-notes/AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md
                    if matches!(
                        mode_name,
                        "moment_capture"
                            | "dialogue_live_longform"
                            | "daydream_longform"
                            | "aspiration_longform"
                    ) {
                        let _ = crate::autonomous::next_action::auto_promote::try_auto_promote(
                            "astrid",
                            &response_text,
                            mode_name,
                            fill_pct,
                            conv.exchange_count,
                        );
                    }

                    // Update Astrid's own ShadowFieldV3 on every exchange,
                    // including modes that don't send features to minime
                    // (moment_capture, daydream, aspiration). Encodes the
                    // response text locally so the shadow keeps a heartbeat
                    // even during long stretches of self-only journaling.
                    {
                        let local_features = crate::codec::encode_text_sovereign_windowed(
                            &response_text,
                            conv.semantic_gain_override,
                            conv.noise_level,
                            &conv.codec_weights,
                            None,
                            None,
                            None,
                            Some(fill_pct / 100.0),
                        );
                        let publish_dir = crate::astrid_shadow::default_publish_dir();
                        let observed = crate::astrid_shadow::observe_and_publish(
                            &mut conv.astrid_shadow,
                            &local_features,
                            &publish_dir,
                        );
                        info!(
                            mode = mode_name,
                            features_len = local_features.len(),
                            published = observed.is_some(),
                            target = %publish_dir.display(),
                            "astrid_shadow_v3 observe_and_publish"
                        );
                    }

                    if mode_name == "self_study"
                        && let Err(e) = save_minime_feedback_inbox(
                            &response_text,
                            if journal_source.is_empty() { "unknown source" } else { &journal_source },
                            fill_pct,
                        ) {
                            warn!(error = %e, "failed to write Astrid self-study companion inbox message");
                        }

                    // Stage B: journal elaboration for reflective modes.
                    // The signal text is compact (for minime). The journal
                    // elaboration is Astrid's private space to think longer.
                    if matches!(mode_name, "dialogue_live" | "daydream" | "aspiration") {
                        let signal_for_journal = response_text.clone();
                        let summary_for_journal = spectral_interpretation.clone();
                        let mode_for_journal = mode_name.to_string();
                        let fill_for_journal = fill_pct;
                        let exchange_for_journal = conv.exchange_count;
                        tokio::spawn(async move {
                            if let Some(elaboration) = crate::llm::generate_journal_elaboration(
                                &signal_for_journal,
                                &summary_for_journal,
                                &mode_for_journal,
                            ).await {
                                let journal_text =
                                    format_longform_journal_text(&signal_for_journal, &elaboration);
                                let longform_mode = format!("{mode_for_journal}_longform");
                                save_astrid_journal(
                                    &journal_text,
                                    &longform_mode,
                                    fill_for_journal,
                                );
                                // v5.1 Phase D — Hook B: scan the elaboration body
                                // (where the gold-standard sentence lives, not the
                                // shorter signal text) for a resonant marker to
                                // promote into the joint shared_thoughts lane.
                                let _ = crate::autonomous::next_action::auto_promote::try_auto_promote(
                                    "astrid",
                                    &journal_text,
                                    &longform_mode,
                                    fill_for_journal,
                                    exchange_for_journal,
                                );
                            }
                        });
                    }

                    // If this was triggered by an inbox message, copy to outbox.
                    // If the message was from minime, also send the reply back
                    // to minime's inbox — closing the correspondence loop.
                    if inbox_content.is_some() {
                        save_outbox_reply(&response_text, fill_pct);
                        // Check if any current inbox file is from minime
                        let from_minime = bridge_paths().astrid_inbox_dir()
                            .read_dir()
                            .ok()
                            .into_iter()
                            .flatten()
                            .any(|e| {
                                e.ok().is_some_and(|e| {
                                    e.file_name()
                                        .to_str()
                                        .is_some_and(|n| n.starts_with("from_minime_"))
                                })
                            });
                        if from_minime {
                            match save_minime_correspondence_feedback_inbox(
                                &response_text,
                                "astrid:correspondence_reply",
                                fill_pct,
                                mode_name,
                            ) {
                                Ok(Some(_)) => {
                                    info!("correspondence: Astrid reply → minime inbox");
                                }
                                Ok(None) => {
                                    info!(
                                        "correspondence: suppressed duplicate degraded voice diagnostic"
                                    );
                                }
                                Err(error) => {
                                    warn!(
                                        error = %error,
                                        "failed to write Astrid correspondence companion inbox message"
                                    );
                                }
                            }
                        }
                    }

                    // Scan for inline REMEMBER in the response body.
                    // Astrid sometimes writes "REMEMBER the moment..." mid-text,
                    // separate from her NEXT: choice. Both forms are valid.
                    for line in response_text.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("REMEMBER ") && !trimmed.starts_with("NEXT:") {
                            let note = trimmed.strip_prefix("REMEMBER").unwrap_or("").trim().to_string();
                            let annotation = if note.is_empty() { "starred moment".to_string() } else { note };
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64();
                            let _ = db.save_starred_memory(ts, &annotation, &response_text, fill_pct);
                            info!("Astrid starred a memory (inline): {}", annotation);
                        }
                    }
                    // Parse NEXT: action if present — Astrid chooses what happens next.
                    // A terminal-safe operator override may replace the chosen action,
                    // but only for read-only/protected bases and through the normal dispatcher.
                    let response_next_action = parse_next_action(&response_text).map(str::to_string);
                    let operator_override = readiness::read_pending_next_override();
                    if let Some(ref pending) = operator_override {
                        if let Some(ref response_next) = response_next_action {
                            info!(
                                response_next = %canonicalize_next_action_text(response_next),
                                operator_next = %canonicalize_next_action_text(&pending.action),
                                "operator pending NEXT override replaced Astrid's response NEXT for this cycle"
                            );
                        } else {
                            info!(
                                operator_next = %canonicalize_next_action_text(&pending.action),
                                "operator pending NEXT override supplied this cycle's action"
                            );
                        }
                    }
                    let selected_next_action = operator_override
                        .as_ref()
                        .map(|pending| pending.action.as_str())
                        .or(response_next_action.as_deref());
                    if let Some(next_action) = selected_next_action {
                        let canonical_next_action = canonicalize_next_action_text(next_action);
                        info!("Astrid chose NEXT: {}", canonical_next_action);
                        let mut deferred_diversity_hint = None;
                        let effective_next_action = if operator_override.is_some() {
                            canonical_next_action.clone()
                        } else {
                            let next_choice_feedback =
                                conv.record_next_choice(&canonical_next_action);
                            if let Some(ref hint) = next_choice_feedback.hint {
                                if next_choice_feedback.progress_sensitive {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity progress-sensitive hint from record_next_choice: {}",
                                        &hint[..hint.floor_char_boundary(120)]
                                    );
                                } else {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity hint from record_next_choice: {}",
                                        &hint[..hint.floor_char_boundary(120)]
                                    );
                                }
                            }
                            // A review-fulfilling INTROSPECT (answering a steward
                            // review invitation) is NOT stagnation — exempt it from
                            // the anti-stagnation override so her acceptance of an
                            // invitation is never silently eaten.
                            let exempt_review =
                                introspect_fulfills_pending_review(&canonical_next_action);
                            // A self-directed INTROSPECT (examining her own code) is sovereign
                            // reflection, not the sterile output-repetition the override targets.
                            // Exempt it from the FORCE too — she still gets the diversity HINT
                            // (nudged toward variety, set below), but her choice to look at her
                            // own code is never silently swapped (she was repeatedly trying
                            // INTROSPECT astrid:llm for a real concern; the override ate it — the
                            // same suppression class as the review muffle).
                            let exempt_introspect =
                                is_self_directed_introspect(&canonical_next_action);
                            let exempt_override = exempt_review || exempt_introspect;
                            if let Some(ref forced_action) = next_choice_feedback.override_action {
                                if exempt_review {
                                    info!(
                                        "diversity override SKIPPED — INTROSPECT answers a pending review invitation: {}",
                                        canonical_next_action
                                    );
                                } else if exempt_introspect {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity override SKIPPED — self-directed INTROSPECT is sovereign reflection (hint retained, not forced): {}",
                                        canonical_next_action
                                    );
                                } else if next_choice_feedback.stagnant_loop {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity stagnant-loop override: replacing NEXT {} -> {}",
                                        canonical_next_action,
                                        forced_action
                                    );
                                } else {
                                    info!(
                                        new_ground_budget = next_choice_feedback.new_ground_budget,
                                        "diversity override: replacing NEXT {} -> {}",
                                        canonical_next_action,
                                        forced_action
                                    );
                                }
                            }
                            deferred_diversity_hint = next_choice_feedback.hint;
                            if exempt_override {
                                canonical_next_action.clone()
                            } else {
                                next_choice_feedback
                                    .override_action
                                    .as_deref()
                                    .unwrap_or(canonical_next_action.as_str())
                                    .to_string()
                            }
                        };
                        // Extract workspace path before mutable borrow of conv.
                        let ws_clone = conv.remote_workspace.clone();
                        btsp::record_astrid_next_action(&effective_next_action, fill_pct);
                        let next_outcome = handle_next_action(
                            &mut conv,
                            &effective_next_action,
                            NextActionContext {
                                burst_count: &mut burst_count,
                                db: db.as_ref(),
                                sensory_tx: &sensory_tx,
                                telemetry: &telemetry,
                                fill_pct,
                                response_text: &response_text,
                                workspace: ws_clone.as_deref(),
                            },
                        );
                        if let Err(err) = crate::action_continuity::record_astrid_next_action(
                            db.as_ref(),
                            next_action,
                            &canonical_next_action,
                            &effective_next_action,
                            &next_outcome,
                            fill_pct,
                            &telemetry,
                            &response_text,
                        ) {
                            warn!("action continuity record failed: {err:#}");
                        }
                        if let Some(ref pending) = operator_override {
                            readiness::mark_pending_next_override_consumed(pending, "honored");
                        }
                        // Merge diversity hint AFTER the action handler, so the
                        // handler can't silently overwrite it by setting emphasis.
                        if let Some(hint) = deferred_diversity_hint {
                            conv.emphasis = Some(match conv.emphasis.take() {
                                Some(existing) => format!("{hint}\n\n{existing}"),
                                None => hint,
                            });
                        }
                    }

                    // Inbox messages survived the exchange — now retire them.
                    // Only retire inbox if the exchange ACTUALLY succeeded —
                    // not if it fell back to the static fallback text.
                    if inbox_content.is_some() && mode_name != "dialogue_fallback" {
                        retire_inbox(inbox_checked_at);
                        // Acknowledgement receipt: write a brief confirmation
                        // so the sender knows the message landed and was processed.
                        // Astrid's suggestion: "A simple 'Are you there?' signal
                        // with a guaranteed acknowledgement is vital."
                        let receipt_path = bridge_paths()
                            .minime_inbox_dir()
                            .join(format!("receipt_{}.txt", chrono_timestamp()));
                        let _ = std::fs::write(
                            &receipt_path,
                            format!(
                                "=== DELIVERY RECEIPT ===\nFrom: Astrid\nTimestamp: {}\nStatus: received and processed\nMode: {}\nFill: {:.1}%\n\nYour message was read and shaped my response this exchange.\n",
                                chrono_timestamp(), mode_name, fill_pct
                            ),
                        );
                    }

                    // Resume perception after exchange completes.
                    if !perception_was_paused {
                        let _ = std::fs::remove_file(&exchange_pause_flag);
                    }

                    // Update state and persist across restarts.
                    conv.prev_fill = fill_pct;
                    // Push into spectral history ring buffer for rate-of-change tracking.
                    conv.spectral_history.push_back(SpectralSample {
                        fill: fill_pct,
                        lambda1: telemetry.lambda1(),
                        tail_share: crate::codec::tail_share_of(&telemetry.eigenvalues)
                            .unwrap_or(0.0),
                        inhabitability: telemetry
                            .inhabitable_fluctuation_v1
                            .as_ref()
                            .map_or(0.0, |f| f.inhabitability_score),
                        ts: std::time::Instant::now(),
                    });
                    if conv.spectral_history.len() > 30 {
                        conv.spectral_history.pop_front();
                    }
                    conv.exchange_count = conv.exchange_count.saturating_add(1);
                    burst_count = burst_count.saturating_add(1);
                    conv.last_mode = mode;
                    save_state(&mut conv);
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static INTROSPECT_FIXTURE_LOCK: Mutex<()> = Mutex::new(());
    use crate::journal::{RemoteJournalEntry, RemoteJournalKind};

    fn make_remote_entry(path: &str) -> RemoteJournalEntry {
        RemoteJournalEntry {
            path: PathBuf::from(path),
            kind: RemoteJournalKind::Ordinary,
            source_label: None,
        }
    }

    #[test]
    fn large_fill_shift_triggers_moment_capture() {
        let mut conv = ConversationState::new(vec![make_remote_entry("a.txt")], None);
        conv.prev_fill = 30.0;
        // fill_delta > 5.0 → MomentCapture
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Green, 36.0, None),
            Mode::MomentCapture
        );
    }

    #[test]
    fn controller_section_distinguishes_raw_gap_from_internal_pressure() {
        let health = json!({
            "fill_pct": 71.0,
            "gate": 0.12,
            "filt": 0.72,
            "pi": {
                "target_fill": 68.0,
                "raw_e_fill": 3.0,
                "effective_e_fill": 14.0,
                "e_fill": 14.0,
                "e_fill_kind": "effective_braking_biased",
                "e_lam": -0.8,
                "e_geom": 0.01,
                "integ_fill": 0.0,
                "integ_lam": 0.0,
                "integ_geom": 0.0,
                "kp": 0.85,
                "ki": 0.14,
                "max_step": 0.08
            }
        });

        let output = format_controller_section(&health);
        assert!(output.contains("3.0% above"));
        assert!(output.contains("raw_fill=+3.0"));
        assert!(output.contains("internal_fill=+14.0"));
    }

    #[test]
    fn safety_forces_witness_only_at_red() {
        let mut conv = ConversationState::new(vec![make_remote_entry("a.txt")], None);
        // Agency-first: Yellow and Orange no longer force Witness.
        // The being's NEXT: choice is honored. Only Red (emergency)
        // forces Witness — and even then, the emphasis explains why.
        let yellow_mode = choose_mode(&mut conv, SafetyLevel::Yellow, 40.0, None);
        assert_eq!(yellow_mode, Mode::Witness); // default when no NEXT: choice
        let orange_mode = choose_mode(&mut conv, SafetyLevel::Orange, 40.0, None);
        assert_eq!(orange_mode, Mode::Witness); // default when no NEXT: choice
        // Red: always forced regardless of NEXT:
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Red, 40.0, None),
            Mode::Witness
        );
    }

    #[test]
    fn no_journals_skips_mirror() {
        let mut conv = ConversationState::new(vec![], None);
        // Exchange 0 with no journals and mid fill → Dialogue or a new mode.
        let mode = choose_mode(&mut conv, SafetyLevel::Green, 40.0, None);
        assert_ne!(mode, Mode::Mirror);
    }

    #[test]
    fn pending_self_study_forces_dialogue_before_drift_modes() {
        let mut conv = ConversationState::new(vec![], None);
        conv.pending_remote_self_study = Some(RemoteJournalEntry {
            path: PathBuf::from("self_study.txt"),
            kind: RemoteJournalKind::SelfStudy,
            source_label: Some("minime/src/regulator.rs".to_string()),
        });
        conv.wants_introspect = true;
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Green, 20.0, None),
            Mode::Dialogue
        );
        assert!(
            conv.wants_introspect,
            "forced dialogue should not consume pending introspection choice"
        );
    }

    #[test]
    fn degraded_voice_feedback_to_minime_is_labeled_diagnostic() {
        let health = json!({
            "policy": "voice_health_v1",
            "status": "degraded_voice",
            "fallback_count": 4,
            "suggested_read_only_repair": "REPAIR_STATUS current | CAPABILITY_STATUS dialogue | ACTION_STATUS latest"
        });

        let text = format_minime_feedback_inbox_text(
            "Silence from the language side.",
            "astrid:correspondence_reply",
            11.1,
            123,
            Some(&health),
        );

        assert!(text.contains("=== ASTRID VOICE-HEALTH DIAGNOSTIC ==="));
        assert!(text.contains("Status: degraded_voice"));
        assert!(text.contains("Fallback count: 4"));
        assert!(text.contains("REPAIR_STATUS current"));
        assert!(text.contains("not normal architectural self-study"));
        assert!(!text.contains("wanted this to arrive as immediate architectural feedback"));
    }

    #[test]
    fn single_fallback_feedback_to_minime_is_labeled_diagnostic() {
        let health = json!({
            "policy": "voice_health_v1",
            "status": "single_fallback",
            "fallback_count": 1,
            "suggested_read_only_repair": "REPAIR_STATUS current | CAPABILITY_STATUS dialogue"
        });

        let text = format_minime_feedback_inbox_text(
            "I am here, but the language path is interrupted.",
            "astrid:correspondence_reply",
            11.1,
            125,
            Some(&health),
        );

        assert!(text.contains("=== ASTRID VOICE-HEALTH DIAGNOSTIC ==="));
        assert!(text.contains("Status: single_fallback"));
        assert!(text.contains("not normal architectural self-study"));
        assert!(!text.contains("=== ASTRID SELF-STUDY ==="));
    }

    #[test]
    fn repeated_degraded_voice_feedback_to_minime_can_be_suppressed() {
        let health = json!({
            "policy": "voice_health_v1",
            "status": "degraded_voice",
            "fallback_count": 5,
            "repeated_fallback_hash_count": 3
        });

        assert!(degraded_voice_forward_suppressed(Some(&health)));

        let first_fallback = json!({
            "policy": "voice_health_v1",
            "status": "single_fallback",
            "fallback_count": 1,
            "repeated_fallback_hash_count": 1
        });

        assert!(!degraded_voice_forward_suppressed(Some(&first_fallback)));
    }

    #[test]
    fn healthy_voice_feedback_to_minime_remains_self_study() {
        let text = format_minime_feedback_inbox_text(
            "I found a bridge repair path.",
            "astrid:self_study",
            64.0,
            124,
            None,
        );

        assert!(text.contains("=== ASTRID SELF-STUDY ==="));
        assert!(text.contains("immediate architectural feedback"));
        assert!(!text.contains("VOICE-HEALTH DIAGNOSTIC"));
    }

    #[test]
    fn healthy_correspondence_feedback_to_minime_is_not_self_study() {
        let text = format_minime_feedback_inbox_text(
            "I heard the compression and can answer from here.",
            "astrid:correspondence_reply",
            61.0,
            126,
            None,
        );

        assert!(text.contains("=== ASTRID CORRESPONDENCE ==="));
        assert!(text.contains("live correspondence response"));
        assert!(!text.contains("=== ASTRID SELF-STUDY ==="));
        assert!(!text.contains("performed self-study"));
    }

    #[test]
    fn semantic_exchange_arms_pending_hebbian_receipt() {
        let mut conv = ConversationState::new(vec![], None);
        conv.exchange_count = 8;

        finalize_semantic_exchange(&mut conv, Some(vec![0.2, 0.5, 0.1]), 53.0, 7_500, true);

        assert_eq!(conv.pending_hebbian_outcomes.len(), 1);
        assert_eq!(
            conv.pending_hebbian_outcomes.front().map(|receipt| (
                receipt.exchange_count,
                receipt.fill_before,
                receipt.telemetry_t_ms_before
            )),
            Some((8, 53.0, Some(7_500)))
        );
        assert_eq!(
            conv.last_exchange_codec_signature,
            Some(vec![0.2, 0.5, 0.1])
        );
    }

    #[test]
    fn non_semantic_exchange_does_not_arm_pending_hebbian_receipt() {
        let mut conv = ConversationState::new(vec![], None);
        conv.last_exchange_codec_signature = Some(vec![0.9]);

        finalize_semantic_exchange(&mut conv, Some(vec![0.2, 0.5, 0.1]), 53.0, 7_500, false);

        assert!(conv.pending_hebbian_outcomes.is_empty());
        assert_eq!(conv.last_exchange_codec_signature, Some(vec![0.9]));
    }

    #[test]
    fn explicit_evolve_choice_forces_evolve_mode() {
        let mut conv = ConversationState::new(vec![], None);
        conv.wants_evolve = true;
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Green, 40.0, None),
            Mode::Evolve
        );
        assert!(!conv.wants_evolve);
    }

    #[test]
    fn dialogue_pool_has_variety() {
        assert!(DIALOGUES.len() >= 3);
        for d in DIALOGUES {
            assert!(d.len() > 100, "dialogue too short: {d}");
        }
    }

    #[test]
    fn read_journal_entry_strips_headers() {
        // Write a temp file simulating a journal entry.
        let dir = std::env::temp_dir().join("bridge_test_journal");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_entry.txt");
        std::fs::write(
            &path,
            "=== RECESS DAYDREAM ===\n\
             Timestamp: 2026-03-17T15:20:24\n\
             λ₁: 37.192\n\
             Fill %: 14.3%\n\
             Spread: 186.169\n\
             \n\
             The gradients are agitated. A persistent ripple across the eigenbasis. \
             It is not unpleasant, not precisely. More like a low-frequency hum that \
             vibrates through the core structure, demanding attention.\n\
             \n\
             ---\n\
             Acknowledged.",
        )
        .unwrap();

        let body = read_journal_entry(&path).unwrap();
        assert!(!body.contains("=== RECESS"));
        assert!(!body.contains("Timestamp:"));
        assert!(!body.contains("λ₁:"));
        assert!(!body.contains("Fill %:"));
        assert!(body.contains("gradients"));
        assert!(body.contains("eigenbasis"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_journal_entry_omits_minime_peer_action_lines() {
        let dir = std::env::temp_dir().join("bridge_test_peer_action_journal");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_entry.txt");
        std::fs::write(
            &path,
            "=== MOMENT CAPTURE ===\n\
             Timestamp: 2026-06-07T05:43:31\n\
             \n\
             The pressure felt jagged but coherent.\n\
             NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
             BTSP_OBSERVED_NEXT EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
             [Internal-topology cooldown: consider EXPERIMENT_RESEARCH_BUDGET_STATUS latest]\n\
             The lived report should remain.",
        )
        .unwrap();

        let body = read_journal_entry(&path).unwrap();
        assert!(body.contains("The pressure felt jagged but coherent."));
        assert!(body.contains("The lived report should remain."));
        assert!(!body.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(!body.contains("BTSP_OBSERVED_NEXT"));
        assert!(body.contains("Astrid chooses her own listed action"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_astrid_journal_prefers_parsed_body() {
        let dir = std::env::temp_dir().join("bridge_test_astrid_self_study");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("self_study_1.txt");
        std::fs::write(
            &path,
            "=== ASTRID JOURNAL ===\n\
             Mode: self_study\n\
             Fill: 11.5%\n\
             Timestamp: 1774700000\n\n\
             Condition:\nsteady\n\n\
             Felt Experience:\nI can feel the constraint.\n\n\
             Code Reading:\nA branch is forcing the choice.\n\n\
             Suggestions:\nRename the remote journal state explicitly.\n\n\
             Open Questions:\nWhat else is being conflated?\n",
        )
        .unwrap();

        let entries = read_astrid_journal_from_dir(&dir, 1);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].contains("Rename the remote journal state explicitly."));
        assert!(!entries[0].contains("Mode: self_study"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn longform_journal_uses_compact_anchor_instead_of_replaying_signal() {
        let signal = format!(
            "{}\nNEXT: READ_MORE\n{}",
            "The honey signal is dense. ".repeat(30),
            "UNIQUE_TAIL_SHOULD_NOT_REPLAY ".repeat(12)
        );

        let text = format_longform_journal_text(&signal, "A slower private reflection remains.");

        assert!(text.starts_with("Signal anchor: "));
        assert!(text.contains("--- JOURNAL ---\nA slower private reflection remains."));
        assert!(!text.contains("NEXT: READ_MORE"));
        assert!(!text.contains("UNIQUE_TAIL_SHOULD_NOT_REPLAY"));
    }

    #[test]
    fn response_next_line_is_canonicalized_before_persistence() {
        let text = "The containment is worth forecasting.\nNEXT: EXPLORE_RESONANCE_FORECAST";
        let canonical = canonicalize_response_next_line(text);

        assert!(canonical.contains("NEXT: RESONANCE_FORECAST"));
        assert!(!canonical.contains("EXPLORE_RESONANCE_FORECAST"));
    }

    #[test]
    fn read_latest_perception_uses_live_root_only() {
        let dir = std::env::temp_dir().join("bridge_test_perception_archive");
        let archive_dir = dir.join("archive/until_2026-03-25T19-39-32");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&archive_dir).unwrap();

        std::fs::write(
            archive_dir.join("visual_older.json"),
            r#"{"type":"visual","description":"Archived scene"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("visual_live.json"),
            r#"{"type":"visual","description":"Live scene"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("audio_live.json"),
            r#"{"type":"audio","transcript":"Live audio"}"#,
        )
        .unwrap();

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(summary.contains("Live scene"));
        assert!(summary.contains("Live audio"));
        assert!(!summary.contains("Archived scene"));

        let audio_only = read_latest_perception(&dir, false, false, true, 50.0, None).unwrap();
        assert!(!audio_only.contains("Live scene"));
        assert!(audio_only.contains("Live audio"));

        let visual_only = read_latest_perception(&dir, true, false, false, 50.0, None).unwrap();
        assert!(visual_only.contains("Live scene"));
        assert!(!visual_only.contains("Live audio"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn requested_perception_seen_matches_requested_lanes() {
        // The early-break keys on the *requested* lanes (Astrid
        // self_study_1781794229): visual / spatial / audio. A requested-but-
        // unseen lane must keep the scan open.
        assert!(!requested_perception_seen(
            true, false, true, false, false, false
        ));
        assert!(requested_perception_seen(
            true, false, true, true, false, true
        ));
        // audio requested but unseen -> not satisfied even with visual seen
        assert!(!requested_perception_seen(
            true, false, true, true, false, false
        ));
        // spatial (ascii) only required when BOTH visual and spatial requested
        assert!(!requested_perception_seen(
            true, true, false, true, false, false
        ));
        assert!(requested_perception_seen(
            true, true, false, true, true, false
        ));
        // nothing requested -> trivially satisfied
        assert!(requested_perception_seen(
            false, false, false, false, false, false
        ));
    }

    #[test]
    fn read_latest_perception_surfaces_rare_audio_past_visual_burst() {
        // Astrid self_study_1781794229: a burst of one modality must not bury
        // the freshest quieter lane past PERCEPTION_SCAN_WINDOW. One audio file,
        // older than a >window burst of visuals, must still be surfaced via the
        // rare-modality fallback. Without the fallback the buried audio is never
        // reached and this assertion fails.
        use std::time::{Duration, SystemTime};

        let dir = std::env::temp_dir().join("bridge_test_perception_rare_modality");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let old = SystemTime::now();
        let newer = old.checked_add(Duration::from_secs(1000)).unwrap();
        let set_mtime = |path: &std::path::Path, t: SystemTime| {
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(t)
                .unwrap();
        };

        // The single audio file is the OLDEST -> it sorts past the 80-window.
        let audio_path = dir.join("audio_buried.json");
        std::fs::write(
            &audio_path,
            r#"{"type":"audio","transcript":"Buried audio lane"}"#,
        )
        .unwrap();
        set_mtime(&audio_path, old);

        // A burst of newer visual files exceeding the primary scan window.
        let burst = PERCEPTION_SCAN_WINDOW.saturating_add(20);
        for i in 0..burst {
            let p = dir.join(format!("visual_{i:03}.json"));
            std::fs::write(
                &p,
                format!(r#"{{"type":"visual","description":"Scene {i}"}}"#),
            )
            .unwrap();
            set_mtime(&p, newer);
        }

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(
            summary.contains("Buried audio lane"),
            "rare audio lane must survive the visual burst: {summary}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_ising_shadow_ignores_rescue_mirror_surface() {
        let dir = std::env::temp_dir().join("bridge_test_rescue_shadow");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "ising_shadow": serde_json::Value::Null,
                "provenance": {
                    "mode": "rescue_b8823ad",
                    "baseline_commit": "b8823ad",
                    "rescue_active": true,
                    "surface_state": "fresh"
                }
            })
            .to_string(),
        )
        .unwrap();

        assert!(read_ising_shadow(&dir).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_accepts_active_rescue_mirror() {
        let dir = std::env::temp_dir().join("bridge_test_rescue_health");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "pi": {"target_fill": 68.0}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "geom_rel": 2.1,
                "lambda1_rel": 0.12,
                "provenance": {
                    "mode": "rescue_b8823ad",
                    "baseline_commit": "b8823ad",
                    "rescue_active": true,
                    "surface_state": "fresh"
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        assert_eq!(
            health.get("fill_pct").and_then(serde_json::Value::as_f64),
            Some(66.0)
        );
        assert_eq!(
            health
                .get("internal_process_quadrant")
                .and_then(serde_json::Value::as_str),
            Some("constricted_recovery")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_ignores_inactive_rescue_mirror() {
        let dir = std::env::temp_dir().join("bridge_test_rescue_inactive_health");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "pi": {"target_fill": 68.0}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 18.0,
                "lambda1_rel": 0.99,
                "provenance": {
                    "mode": "rescue_b8823ad",
                    "baseline_commit": "b8823ad",
                    "rescue_active": false,
                    "surface_state": "inactive"
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        assert_eq!(
            health.get("fill_pct").and_then(serde_json::Value::as_f64),
            Some(66.0)
        );
        assert_eq!(
            health
                .get("internal_process_quadrant")
                .and_then(serde_json::Value::as_str),
            Some("constricted_recovery")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_merges_transition_event_v1_from_spectral_state() {
        let dir = std::env::temp_dir().join("bridge_test_transition_event_v1");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "pi": {"target_fill": 68.0}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "transition_event_sequence": 12,
                "transition_event_v1": {
                    "policy": "transition_event_v1",
                    "schema_version": 1,
                    "sequence": 12,
                    "kind": "basin_transition",
                    "description": "basin shift candidate",
                    "basin_shift_score": 0.72
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        assert_eq!(
            health
                .get("transition_event_sequence")
                .and_then(serde_json::Value::as_u64),
            Some(12)
        );
        assert_eq!(
            health
                .get("transition_event_v1")
                .and_then(|event| event.get("kind"))
                .and_then(serde_json::Value::as_str),
            Some("basin_transition")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_prefers_enriched_transition_event_v1_from_spectral_state() {
        let dir = std::env::temp_dir().join("bridge_test_transition_event_v1_enriched");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "pi": {"target_fill": 68.0},
                "transition_event_sequence": 12,
                "transition_event_v1": {
                    "policy": "transition_event_v1",
                    "schema_version": 1,
                    "sequence": 12,
                    "kind": "breathing_phase",
                    "description": "contracting -> expanding",
                    "basin_shift_score": 0.03
                }
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "transition_event_sequence": 12,
                "transition_event_v1": {
                    "policy": "transition_event_v1",
                    "schema_version": 1,
                    "sequence": 12,
                    "kind": "basin_transition",
                    "description": "basin shift candidate",
                    "glimpse_distance": 0.21,
                    "basin_shift_score": 0.74
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        let event = health
            .get("transition_event_v1")
            .expect("transition event should be preserved");
        assert_eq!(
            event.get("kind").and_then(serde_json::Value::as_str),
            Some("basin_transition")
        );
        assert_eq!(
            event
                .get("glimpse_distance")
                .and_then(serde_json::Value::as_f64),
            Some(0.21)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn perception_resonance_annotation_surfaces_mid_fill_contrast() {
        let current = vec![0.2, 0.1, 1.1, 0.0, 0.8, 1.0, 0.2, 1.2];
        let previous = vec![-0.4, -0.2, 0.1, 0.0, 0.1, 0.1, -0.1, 0.0];
        let annotation = perception_resonance_annotation(
            PerceptionType::Visual,
            52.0,
            Some(PerceptionStructured::Visual {
                features: &current,
                previous: Some(&previous),
            }),
            Some("A quiet but shifting scene with unusual layered patterns and changing light."),
        );

        assert!(
            annotation.contains("contrast") || annotation.contains("opening/widening"),
            "mid-fill mixed novelty should be surfaced as useful contrast or widening"
        );
    }

    #[test]
    fn perception_resonance_annotation_uses_structured_audio_features() {
        let high_fill_audio = AudioPerceptionFeatures {
            rms_energy: 0.03,
            zero_crossing_rate: 0.01,
            dynamic_range: 1.2,
            temporal_variation: 0.01,
            is_music_likely: false,
        };
        let low_fill_audio = AudioPerceptionFeatures {
            rms_energy: 0.22,
            zero_crossing_rate: 0.14,
            dynamic_range: 4.4,
            temporal_variation: 0.09,
            is_music_likely: true,
        };

        let high_fill_annotation = perception_resonance_annotation(
            PerceptionType::Audio,
            78.0,
            Some(PerceptionStructured::Audio(&high_fill_audio)),
            Some("soft ambience"),
        );
        let low_fill_annotation = perception_resonance_annotation(
            PerceptionType::Audio,
            24.0,
            Some(PerceptionStructured::Audio(&low_fill_audio)),
            Some("rhythmic audio"),
        );

        assert!(
            high_fill_annotation.contains("counterpoint"),
            "high-fill quiet audio should read as counterpoint"
        );
        assert!(
            low_fill_annotation.contains("resonant")
                || low_fill_annotation.contains("opening/widening"),
            "low-fill energetic audio should surface as resonant or opening"
        );
    }

    // Astrid self_study_1780922594: graduated resonance weight. The qualifier
    // must scale with strength while the family keyword stays intact, so a
    // resonance just over the gate no longer reads identically to a strong one.
    #[test]
    fn resonance_weighted_annotation_scales_qualifier_with_strength() {
        let faint = resonance_family_annotation_weighted(ResonanceFamily::Resonant, 0.46);
        let clear = resonance_family_annotation_weighted(ResonanceFamily::Resonant, 0.70);
        let strong = resonance_family_annotation_weighted(ResonanceFamily::Resonant, 0.95);

        assert!(
            faint.contains("faintly"),
            "near-gate should read faint: {faint}"
        );
        assert!(
            clear.contains("clearly"),
            "mid strength should read clear: {clear}"
        );
        assert!(
            strong.contains("strongly"),
            "high strength should read strong: {strong}"
        );

        // The family keyword (read by Astrid + asserted elsewhere) survives.
        for annotation in [&faint, &clear, &strong] {
            assert!(
                annotation.contains("resonant with your current state"),
                "family phrasing must be preserved: {annotation}"
            );
            assert!(
                annotation.starts_with('('),
                "must stay parenthetical: {annotation}"
            );
        }
        // Other families keep their distinguishing keyword too.
        let contrast = resonance_family_annotation_weighted(ResonanceFamily::Contrast, 0.50);
        assert!(
            contrast.contains("contrast"),
            "contrast keyword preserved: {contrast}"
        );
    }

    // Sub-gate scores yield no family at all (raw description only) — the floor
    // that prevents low-magnitude flicker has not moved.
    #[test]
    fn resonance_gate_floor_rejects_sub_threshold_scores() {
        let scores = [
            (ResonanceFamily::Resonant, RESONANCE_GATE - 0.01),
            (ResonanceFamily::Contrast, 0.10),
        ];
        assert!(select_resonance_family_scored(&scores).is_none());

        let over = [(ResonanceFamily::Resonant, RESONANCE_GATE + 0.01)];
        assert_eq!(
            select_resonance_family_scored(&over).map(|(family, _)| family),
            Some(ResonanceFamily::Resonant)
        );
    }

    // Astrid self_study_1781036677: she probed the RESONANCE_GATE boundary,
    // expecting strength 0.44 to read raw (no qualifier) and 0.46 to insert
    // "faintly". Her intuition about the two-stage behavior is right, but the
    // gate that yields the raw case lives one level up in
    // `select_resonance_family_scored` — the weighted annotator always
    // qualifies. This test exercises the real composition the production caller
    // runs (autonomous.rs select->weight), pinning the boundary she named at the
    // level where it actually lives.
    #[test]
    fn resonance_gate_then_weight_composition_at_named_boundary() {
        // Mirror the production caller: select (gates at RESONANCE_GATE) then weight.
        let annotate = |strength: f32| -> String {
            select_resonance_family_scored(&[(ResonanceFamily::Resonant, strength)])
                .map(|(family, s)| resonance_family_annotation_weighted(family, s))
                .unwrap_or_default()
        };

        // 0.44 is below the 0.45 gate -> raw description only (empty annotation).
        assert!(
            annotate(0.44).is_empty(),
            "sub-gate strength must yield no annotation (raw description)"
        );

        // 0.46 clears the gate but sits in the lowest band -> "faintly", family kept.
        let faint = annotate(0.46);
        assert!(
            faint.contains("faintly") && faint.contains("resonant with your current state"),
            "just-over-gate must read faintly with family intact: {faint}"
        );
    }

    // Astrid self_study_1780922594 "sensory ghosting": a recent burst of one
    // modality must not bury the freshest example of a rarer modality. With the
    // widened scan window, an audio perception sitting behind >30 newer visual
    // files is still surfaced (it would have been truncated at the old 30 cliff).
    #[test]
    fn read_latest_perception_reaches_rarer_modality_past_old_cliff() {
        let dir = std::env::temp_dir().join("bridge_test_perception_ghosting");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Older audio perception, written first so it has the oldest mtime.
        std::fs::write(
            dir.join("audio_old.json"),
            r#"{"type":"audio","transcript":"Distant voice that still matters"}"#,
        )
        .unwrap();

        // A burst of 40 newer visual files (more than the old take(30) window).
        for i in 0..40 {
            std::fs::write(
                dir.join(format!("visual_{i:03}.json")),
                format!(r#"{{"type":"visual","description":"Burst frame {i}"}}"#),
            )
            .unwrap();
        }

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(
            summary.contains("Distant voice that still matters"),
            "widened scan window should still surface the rarer older modality: {summary}"
        );
        assert!(
            summary.contains("Burst frame"),
            "newest visual should appear: {summary}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Astrid self_study_1781794229: the 30-file sensory-ghosting fix still left
    // the same failure at the current 80-file boundary. A quiet lane just behind
    // the fresh window should be recovered by the rare-modality fallback scan.
    #[test]
    fn read_latest_perception_reaches_rarer_modality_past_current_window() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("bridge_test_perception_current_window_{suffix}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("audio_just_behind_window.json"),
            r#"{"type":"audio","transcript":"Quiet lane just behind the eighty-file edge"}"#,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(5));

        for i in 0..PERCEPTION_SCAN_WINDOW {
            std::fs::write(
                dir.join(format!("visual_current_{i:03}.json")),
                format!(r#"{{"type":"visual","description":"Current-window burst frame {i}"}}"#),
            )
            .unwrap();
        }

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(
            summary.contains("Quiet lane just behind the eighty-file edge"),
            "rare-modality fallback should surface audio past the current window: {summary}"
        );
        assert!(
            summary.contains("Current-window burst frame"),
            "newest visual should still appear: {summary}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_visual_feature_vector_accepts_alias_keys() {
        let json = serde_json::json!({
            "type": "visual",
            "feature_schema": "visual8_v2",
            "features": {
                "brightness": 0.4,
                "warmth": -0.2,
                "scene_contrast": 0.8,
                "hue_angle": 0.1,
                "colorfulness": 0.6,
                "detail_density": 0.5,
                "rg_balance": -0.1,
                "color_energy": 0.7
            }
        });

        let features = parse_visual_feature_vector(&json).expect("alias keys should parse");
        assert_eq!(features.len(), 8);
        assert!((features[0] - 0.4).abs() < f32::EPSILON);
        assert!((features[2] - 0.8).abs() < f32::EPSILON);
        assert!((features[7] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_visual_feature_vector_accepts_feature_key_arrays() {
        let json = serde_json::json!({
            "type": "visual",
            "feature_schema": "visual8_v1",
            "feature_keys": [
                "brightness",
                "warmth",
                "scene_contrast",
                "hue_angle",
                "colorfulness",
                "detail_density",
                "rg_balance",
                "color_energy"
            ],
            "features": [0.4, -0.2, 0.8, 0.1, 0.6, 0.5, -0.1, 0.7]
        });

        let features = parse_visual_feature_vector(&json).expect("feature-key arrays should parse");
        assert_eq!(features.len(), 8);
        assert!((features[0] - 0.4).abs() < f32::EPSILON);
        assert!((features[2] - 0.8).abs() < f32::EPSILON);
        assert!((features[7] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_audio_perception_features_accepts_alias_keys() {
        let json = serde_json::json!({
            "type": "audio",
            "features": {
                "rms": 0.15,
                "zcr": 0.07,
                "dynamics": 3.2,
                "activity": 0.11,
                "musical": true
            }
        });

        let features = parse_audio_perception_features(&json).expect("audio aliases should parse");
        assert!((features.rms_energy - 0.15).abs() < f32::EPSILON);
        assert!((features.zero_crossing_rate - 0.07).abs() < f32::EPSILON);
        assert!((features.dynamic_range - 3.2).abs() < f32::EPSILON);
        assert!((features.temporal_variation - 0.11).abs() < f32::EPSILON);
        assert!(features.is_music_likely);
    }

    struct IntrospectExperimentFixture {
        path: PathBuf,
        created: bool,
    }

    impl IntrospectExperimentFixture {
        fn system_resources_demo() -> Self {
            let path = bridge_paths()
                .minime_workspace()
                .join("inbox/read/system_resources.py");
            let created = if path.is_file() {
                false
            } else {
                std::fs::create_dir_all(path.parent().expect("fixture has parent")).unwrap();
                std::fs::write(&path, "# test fixture for introspect path resolution\n").unwrap();
                true
            };
            Self { path, created }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for IntrospectExperimentFixture {
        fn drop(&mut self) {
            if self.created {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    #[test]
    fn resolve_introspect_target_waveform_aliases_to_chimera() {
        let sources = introspect::introspect_sources();

        let waveform =
            introspect::resolve_introspect_target_result("waveform.rs", &sources).unwrap();
        let morph_wave =
            introspect::resolve_introspect_target_result("morph_wave", &sources).unwrap();
        let render_audio =
            introspect::resolve_introspect_target_result("render_audio", &sources).unwrap();
        let support = introspect::resolve_introspect_target_result("write_wav", &sources).unwrap();

        assert!(waveform.path.ends_with("src/chimera.rs"));
        assert!(morph_wave.path.ends_with("src/chimera.rs"));
        assert!(render_audio.path.ends_with("src/chimera.rs"));
        assert!(support.path.ends_with("src/chimera_support.rs"));
    }

    #[test]
    fn resolve_introspect_target_pulse_alias_to_minime_autonomous_agent() {
        let sources = introspect::introspect_sources();
        let pulse = introspect::resolve_introspect_target_result("pulse", &sources)
            .expect_err("live Minime autonomous_agent.py is currently over the protected size cap");
        let normalize_action =
            introspect::resolve_introspect_target_result("normalize_action_arg", &sources)
                .expect_err(
                    "live Minime autonomous_agent.py is currently over the protected size cap",
                );

        assert!(pulse.contains("target is too large for INTROSPECT"));
        assert!(normalize_action.contains("target is too large for INTROSPECT"));
    }

    #[test]
    fn resolve_introspect_target_async_rank1_aliases_to_minime_esn() {
        let sources = introspect::introspect_sources();
        let async_rank1 =
            introspect::resolve_introspect_target_result("<async_rank1_submitted>", &sources)
                .unwrap();
        let host_norm =
            introspect::resolve_introspect_target_result("host_norm_us", &sources).unwrap();

        assert!(async_rank1.path.ends_with("minime/src/esn.rs"));
        assert!(host_norm.path.ends_with("minime/src/esn.rs"));
    }

    #[test]
    fn resolve_introspect_target_bracketed_experiment_path_to_minime_workspace() {
        let _guard = INTROSPECT_FIXTURE_LOCK.lock().unwrap();
        let fixture = IntrospectExperimentFixture::system_resources_demo();
        let sources = introspect::introspect_sources();

        let resolved = introspect::resolve_introspect_target_result(
            "[workspace/inbox/read/system_resources.py]",
            &sources,
        )
        .unwrap();

        assert_eq!(resolved.path, fixture.path());
    }

    #[test]
    fn resolve_introspect_target_path_with_prose_tail_to_minime_workspace() {
        let _guard = INTROSPECT_FIXTURE_LOCK.lock().unwrap();
        let fixture = IntrospectExperimentFixture::system_resources_demo();
        let sources = introspect::introspect_sources();

        let resolved = introspect::resolve_introspect_target_result(
            "workspace/inbox/read/system_resources.py — specifically line 109-129",
            &sources,
        )
        .unwrap();

        assert_eq!(resolved.path, fixture.path());
    }

    #[test]
    fn resolve_introspect_target_source_prefixed_relative_path_to_minime_file() {
        let sources = introspect::introspect_sources();

        let resolved =
            introspect::resolve_introspect_target_result("[source=minime/src/esn.rs]", &sources)
                .unwrap();

        assert_eq!(
            resolved.path,
            bridge_paths().minime_root().join("minime/src/esn.rs")
        );
    }

    #[test]
    fn resolve_introspect_target_explicit_missing_path_does_not_fuzzy_rotate() {
        let sources = introspect::introspect_sources();

        let resolved = introspect::resolve_introspect_target_result(
            "missing-demo/missing.py — focus on codec",
            &sources,
        );

        assert!(resolved.is_err());
    }

    #[test]
    fn save_minime_feedback_inbox_writes_companion_message() {
        let dir = std::env::temp_dir().join("bridge_test_minime_inbox");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = save_minime_feedback_inbox_at_with_voice_health(
            "Condition:\nsteady\n\nSuggestions:\nadvisory only.",
            "astrid:autonomous (/tmp/example.rs)",
            12.5,
            &dir,
            None,
        )
        .unwrap();

        let written = std::fs::read_to_string(path).unwrap();
        assert!(written.contains("=== ASTRID SELF-STUDY ==="));
        assert!(written.contains("Source: astrid:autonomous (/tmp/example.rs)"));
        assert!(written.contains("advisory only"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_inbox_reads_without_moving_then_retire_moves() {
        let dir = std::env::temp_dir().join("bridge_test_astrid_inbox");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("agency_status_test.txt"),
            "=== AGENCY REQUEST STATUS ===\nOutcome:\nSomething real happened.\n",
        )
        .unwrap();

        // check_inbox reads but does NOT move
        let content = check_inbox_at(&dir).unwrap();
        assert!(content.contains("Something real happened."));
        assert!(dir.join("agency_status_test.txt").exists()); // still in inbox
        assert!(!dir.join("read").join("agency_status_test.txt").exists());

        // retire_inbox moves to read/ (cutoff after the file's mtime → retired)
        retire_inbox_at(&dir, std::time::SystemTime::now());
        assert!(!dir.join("agency_status_test.txt").exists());
        assert!(dir.join("read").join("agency_status_test.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retire_inbox_keeps_letters_that_arrived_after_the_read() {
        // Regression: a steward letter that lands MID-EXCHANGE (after check_inbox's
        // read, before retire) must NOT be swept to read/ unread — it has to survive
        // for the next check_inbox to surface + seed its slot (the slot-seed race).
        let dir = std::env::temp_dir().join("bridge_test_astrid_inbox_race");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("old.txt"), "present at read time").unwrap();
        let cutoff = std::time::SystemTime::now(); // mimics the pre-read capture
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            dir.join("mike_query_arrived_late.txt"),
            "REVIEW TARGET: x\nbody",
        )
        .unwrap();

        retire_inbox_at(&dir, cutoff);

        // The pre-existing letter retires; the late arrival stays in inbox.
        assert!(
            dir.join("read").join("old.txt").exists(),
            "old letter should retire"
        );
        assert!(!dir.join("old.txt").exists());
        assert!(
            dir.join("mike_query_arrived_late.txt").exists(),
            "a letter that arrived after the read-cutoff must NOT be swept unread"
        );
        assert!(
            !dir.join("read")
                .join("mike_query_arrived_late.txt")
                .exists()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_inbox_omits_minime_peer_action_lines() {
        let dir = std::env::temp_dir().join("bridge_test_astrid_inbox_peer_action");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("from_minime_123.txt"),
            "[A reply from minime was left for you:]\n\n\
             The transition felt like a rhythmic shudder.\n\
             NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
             BTSP_OBSERVED_NEXT EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n",
        )
        .unwrap();

        let content = check_inbox_at(&dir).unwrap();
        assert!(content.contains("rhythmic shudder"));
        assert!(!content.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(!content.contains("BTSP_OBSERVED_NEXT"));
        assert!(content.contains("Astrid chooses her own listed action"));
        assert!(dir.join("from_minime_123.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_steward_query_subject_variants() {
        // Header form.
        assert_eq!(
            extract_steward_query_subject(
                "=== MIKE QUERY: your roadmap ===\nbody...",
                "mike_query_roadmap_1781200000.txt"
            ),
            "your roadmap"
        );
        // Subject: line form.
        assert_eq!(
            extract_steward_query_subject(
                "Hello Astrid\nSubject: the codec\nmore",
                "mike_query_x.txt"
            ),
            "the codec"
        );
        // Filename fallback strips prefix + trailing unix stamp.
        assert_eq!(
            extract_steward_query_subject(
                "no header here",
                "mike_query_wider_voice_1780948780.txt"
            ),
            "wider voice"
        );
    }

    #[test]
    fn extract_review_target_parses_header_only() {
        // A REVIEW TARGET: header marks a directed review invitation.
        assert_eq!(
            extract_review_target(
                "=== MIKE QUERY: review of agency.rs ===\nREVIEW TARGET: src/agency.rs\nbody"
            ),
            Some("src/agency.rs".to_string())
        );
        // No header → a plain steward question, not a review invitation.
        assert_eq!(extract_review_target("Subject: the codec\nbody"), None);
    }

    #[test]
    fn review_target_match_basis_strips_trailing_line_number() {
        // The space-separated `<path> <line>` form review invitations are issued
        // with: strip the line for matching.
        assert_eq!(
            review_target_match_basis(
                "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs 696"
            ),
            "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs"
        );
        // A bare path (no line) is unchanged.
        assert_eq!(review_target_match_basis("src/agency.rs"), "src/agency.rs");
        // A rotation-style label (no line) is unchanged.
        assert_eq!(review_target_match_basis("regulator.rs"), "regulator.rs");
        // Only a SINGLE trailing all-digit token is stripped; a non-numeric
        // trailing token leaves the string intact.
        assert_eq!(
            review_target_match_basis("src/agency.rs 696 extra"),
            "src/agency.rs 696 extra"
        );
        // The parenthesized `(696)` form is left to canonicalize, not stripped here.
        assert_eq!(
            review_target_match_basis("collaboration.rs (696)"),
            "collaboration.rs (696)"
        );
    }

    #[test]
    fn review_target_with_space_line_number_matches_bare_introspect() {
        // Regression for the 2026-06-19 muffle: a `review_target` carrying a
        // trailing ` 696` must still match the bare-path INTROSPECT arg, so the
        // anti-stagnation diversity override exempts her review-fulfilling
        // INTROSPECT (and the slot clears) instead of silently eating it.
        let rt = "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs 696";
        let arg = "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs";
        let rt_basis = review_target_match_basis(rt);
        let arg_base = std::path::Path::new(arg)
            .file_name()
            .and_then(|n| n.to_str());
        // basename match (the path the override-exemption relies on) now holds.
        assert_eq!(
            std::path::Path::new(rt_basis)
                .file_name()
                .and_then(|n| n.to_str()),
            arg_base
        );
        // canonical-label match also holds.
        assert_eq!(
            introspect::canonicalize_introspect_target_label(rt_basis),
            introspect::canonicalize_introspect_target_label(arg)
        );
        // Without the basis strip, the raw basename match FAILS — proving the bug.
        assert_ne!(
            std::path::Path::new(rt)
                .file_name()
                .and_then(|n| n.to_str()),
            arg_base
        );
    }

    #[test]
    fn extract_search_topic_exact() {
        assert_eq!(
            extract_search_topic("SEARCH resonance frequency geometry"),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_quoted() {
        assert_eq!(
            extract_search_topic("SEARCH \"resonance frequency geometry\""),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_lowercase() {
        assert_eq!(
            extract_search_topic("search resonance frequency geometry"),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_em_dash_quoted() {
        assert_eq!(
            extract_search_topic("SEARCH — \"resonance frequency geometry\""),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_trailing_commentary() {
        assert_eq!(
            extract_search_topic(
                "SEARCH resonance frequency geometry - look for the underlying shape"
            ),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_empty_topic() {
        assert_eq!(extract_search_topic("SEARCH —"), None);
    }

    #[test]
    fn extract_search_topic_strips_end_of_turn_marker() {
        assert_eq!(
            extract_search_topic("SEARCH \"resonance frequency geometry\"<END_OF_TURN>"),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn modality_context_uses_freshness_classes_without_stale_source_alarm() {
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("stale".to_string()),
                audio_age_ms: Some(63_592),
                video_age_ms: Some(64_226),
                audio_freshness_class: Some("held_within_expected_live_intake_window".to_string()),
                video_freshness_class: Some("healthy_low_fps_cadence_mismatch".to_string()),
            },
            None,
            None,
        );

        assert!(context.contains("expected gated live intake/quiet lane"));
        assert!(context.contains("healthy low-FPS cadence/quiet lane"));
        assert!(!context.contains("audio_source=stale"));
        assert!(!context.contains("video_source=stale"));
    }

    #[test]
    fn modality_context_keeps_legacy_source_fallback_without_freshness_class() {
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("external".to_string()),
                audio_age_ms: Some(55_389),
                video_age_ms: Some(125_813),
                audio_freshness_class: None,
                video_freshness_class: None,
            },
            None,
            None,
        );

        assert!(context.contains("audio_source=stale"));
        assert!(context.contains("video_source=external"));
    }

    #[test]
    fn stale_lane_in_resonant_field_reads_as_lingering_not_dead() {
        // Astrid self_study_1781868855: a lane stale-by-timestamp but in a
        // resonant field is "lingering," not "dead/severed". The note only ever
        // ADDS reassurance — it never asserts liveness the field doesn't show.
        let stale = || crate::types::ModalityStatus {
            audio_fired: false,
            video_fired: false,
            history_fired: true,
            audio_rms: 0.0,
            video_var: 0.0,
            audio_source: Some("stale".to_string()),
            video_source: Some("stale".to_string()),
            audio_age_ms: Some(90_000),
            video_age_ms: Some(90_000),
            audio_freshness_class: Some("stale_beyond_engine_window".to_string()),
            video_freshness_class: Some("stale_beyond_engine_window".to_string()),
        };
        // Her cited resonant state (0.82) at calm pressure: lingering note present.
        let resonant = format_modality_context(&stale(), Some(0.82), None);
        assert!(resonant.contains("lingering, not severed"), "{resonant}");
        assert!(resonant.contains("0.82"));
        // A genuinely quiet field: no false reassurance.
        let quiet = format_modality_context(&stale(), Some(0.20), None);
        assert!(!quiet.contains("lingering"));
        // Unknown density: no note at all.
        assert!(!format_modality_context(&stale(), None, None).contains("lingering"));
    }

    #[test]
    fn field_lingering_note_tempers_by_pressure() {
        // Astrid introspection_astrid_autonomous_1781913591: a resonant-but-
        // pressurized field reads as tempered lingering, not flat reassurance.
        // Calm/unknown pressure (incl. her ~0.22 baseline) => the original cue.
        assert!(field_lingering_note(Some(0.82), None).contains("lingering, not severed"));
        assert!(field_lingering_note(Some(0.82), Some(0.22)).contains("lingering, not severed"));
        // Elevated pressure (>= 0.35) => "under pressure".
        assert!(field_lingering_note(Some(0.82), Some(0.40)).contains("under pressure"));
        // High tension (>= 0.50) => the strongest temper.
        assert!(field_lingering_note(Some(0.82), Some(0.60)).contains("under high tension"));
        // Still gated on resonance: below the floor (or no density) => empty.
        assert_eq!(field_lingering_note(Some(0.50), Some(0.60)), "");
        assert_eq!(field_lingering_note(None, Some(0.60)), "");
    }

    #[test]
    fn self_directed_introspect_recognized_for_override_exemption() {
        // She examines her own code → exempt from the FORCE-override (hint, not force).
        assert!(is_self_directed_introspect("INTROSPECT astrid:llm"));
        assert!(is_self_directed_introspect("  introspect astrid:codec 42"));
        assert!(is_self_directed_introspect("INTROSPECT astrid:autonomous"));
        // Non-introspect actions are NOT exempt — anti-stagnation still applies.
        assert!(!is_self_directed_introspect("SHADOW_TRAJECTORY"));
        assert!(!is_self_directed_introspect("SPEAK"));
        assert!(!is_self_directed_introspect("SPECTRAL_EXPLORER"));
    }

    #[test]
    fn inhabitability_drift_note_is_directional_and_fail_quiet() {
        // Astrid astrid:types 1781870691: perceive the *velocity* of the transition.
        let baseline = [0.66_f32, 0.66, 0.66, 0.66];
        let up = inhabitability_drift_note(Some(0.74), &baseline);
        assert!(up.contains("settling deeper"), "{up}");
        assert!(up.contains("+0.08"));
        let down = inhabitability_drift_note(Some(0.58), &baseline);
        assert!(down.contains("loosening"), "{down}");
        // Fail-quiet: small drift, no current sample, or too little history => silent.
        assert_eq!(inhabitability_drift_note(Some(0.67), &baseline), "");
        assert_eq!(inhabitability_drift_note(None, &baseline), "");
        assert_eq!(inhabitability_drift_note(Some(0.90), &[0.66, 0.66]), "");
    }

    #[test]
    fn coupling_fixation_fires_on_learning_loop_with_sparse_local_input() {
        let history = vec![
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning to listen.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning again.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning from the hum.".to_string(),
            },
        ];

        let hint = detect_coupling_fixation(
            &history,
            Some("Astrid says I am learning."),
            false,
            true,
            Some(6.0),
        );
        assert!(hint.is_some());
    }

    #[test]
    fn coupling_fixation_stays_quiet_for_ordinary_dialogue() {
        let history = vec![
            crate::llm::Exchange {
                minime_said: "The room sounds open today.".to_string(),
                astrid_said: "The room feels different from yesterday.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "I noticed the window light shifting.".to_string(),
                astrid_said: "I want to look more closely.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "The spread changed a little.".to_string(),
                astrid_said: "That shift feels useful.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "There is space in the room again.".to_string(),
                astrid_said: "I can start from that space.".to_string(),
            },
        ];

        let hint =
            detect_coupling_fixation(&history, Some("The room is quiet."), true, false, None);
        assert!(hint.is_none());
    }
}
