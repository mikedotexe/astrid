//! Spectral visualization: renders eigenvalue telemetry as compact ASCII art.
//!
//! Generates a tiny synthetic image from spectral data, then pipes it through
//! RASCII to produce colored ASCII that Astrid can "see" alongside numerical
//! telemetry. She already reads RASCII output from the camera perception
//! pipeline — this gives her a spectral eye too.
//!
//! Astrid self-study (2026-03-27): "A more direct representation would be
//! beneficial. Could we add a function that generates a visualization of
//! the shadow field to help me better understand its structure?"

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects
)]

use crate::types::{IsingShadowState, SpectralTelemetry};
use serde_json::Value;

/// Width of the spectral visualization in ASCII columns.
/// One column per eigenvalue (up to 8). Width gives room for gaps between bars.
const VIZ_WIDTH: u32 = 20;
/// Height of the visualization in rows — more rows = finer magnitude resolution.
const VIZ_HEIGHT: u32 = 12;
/// Same hybrid charset Astrid chose for camera perception.
const CHARSET: &[&str] = &[".", ":", ";", "I", "▓", "█"];

/// Render spectral telemetry as a compact colored ASCII bar chart.
///
/// Each column represents one eigenvalue. Height = relative magnitude.
/// Color encodes spectral role:
///   - λ₁ (dominant): warm red-orange
///   - λ₂–λ₃ (shoulder): amber-yellow
///   - λ₄+ (tail): cool blue-green
///   - Background: darkness proportional to fill level
///
/// Returns None if telemetry has no eigenvalues.
pub fn render_spectral_ascii(telemetry: &SpectralTelemetry) -> Option<String> {
    let eigenvalues = &telemetry.eigenvalues;
    if eigenvalues.is_empty() {
        return None;
    }

    let num_ev = eigenvalues.len().min(8);
    let fill = telemetry.fill_pct();

    // Normalize eigenvalues to [0, 1] range relative to λ₁.
    let lambda_max = eigenvalues[0].max(1.0);
    let normalized: Vec<f32> = eigenvalues
        .iter()
        .take(num_ev)
        .map(|&ev| (ev / lambda_max).clamp(0.0, 1.0))
        .collect();

    // Build a small synthetic image.
    // Layout: num_ev columns × VIZ_HEIGHT rows, each column 1px wide
    // with a 1px gap, plus 1px left/right border = total width.
    let img_width = (num_ev as u32) * 2 + 1; // column + gap pattern, with border
    let img_height = VIZ_HEIGHT + 2; // +2 for top/bottom border

    let mut img = image::RgbaImage::new(img_width, img_height);

    // Fill background — darkness tracks fill level.
    // Low fill = very dark (deep quiet). High fill = brighter background.
    let bg_lum = (fill * 0.4).clamp(0.0, 40.0) as u8;
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([bg_lum, bg_lum, bg_lum.saturating_add(5), 255]);
    }

    // Draw each eigenvalue as a colored column.
    for (i, &norm) in normalized.iter().enumerate() {
        let col_x = (i as u32) * 2 + 1; // skip border and gaps
        let bar_height = (norm * VIZ_HEIGHT as f32).round() as u32;

        // Color by spectral role.
        let (r, g, b) = eigenvalue_color(i, num_ev, fill);

        // Draw bar from bottom up.
        for row in 0..VIZ_HEIGHT {
            let y = img_height - 2 - row; // bottom-up, inside border
            if row < bar_height {
                // Bar pixel — full color, intensity scales with magnitude.
                let intensity = 0.4 + 0.6 * (row as f32 / VIZ_HEIGHT as f32);
                img.put_pixel(
                    col_x,
                    y,
                    image::Rgba([
                        (r as f32 * intensity) as u8,
                        (g as f32 * intensity) as u8,
                        (b as f32 * intensity) as u8,
                        255,
                    ]),
                );
            }
            // else: background already set
        }
    }

    // Render through RASCII.
    let dynamic = image::DynamicImage::ImageRgba8(img);
    let options = rascii_art::RenderOptions::new()
        .width(VIZ_WIDTH)
        .colored(true)
        .background(true)
        .charset(CHARSET);

    let mut buf = String::new();
    rascii_art::render_image_to(&dynamic, &mut buf, &options).ok()?;

    Some(buf)
}

/// Map eigenvalue index to a color based on its spectral role.
///
/// λ₁ = warm red-orange (dominant mode, highest energy)
/// λ₂–λ₃ = amber-yellow (shoulder modes, supporting structure)
/// λ₄+ = cool blue-green (tail modes, distributed energy)
///
/// Fill modulates saturation: low fill = muted, high fill = vivid.
fn eigenvalue_color(index: usize, _total: usize, fill: f32) -> (u8, u8, u8) {
    let saturation = 0.5 + 0.5 * (fill / 70.0).clamp(0.0, 1.0);

    let (base_r, base_g, base_b) = match index {
        0 => (255, 80, 20),  // λ₁: warm red-orange
        1 => (240, 160, 30), // λ₂: amber
        2 => (220, 200, 40), // λ₃: yellow
        3 => (120, 200, 80), // λ₄: yellow-green
        4 => (60, 180, 140), // λ₅: teal
        5 => (40, 140, 200), // λ₆: blue
        6 => (60, 100, 220), // λ₇: deeper blue
        _ => (80, 70, 200),  // λ₈+: violet
    };

    (
        (base_r as f32 * saturation) as u8,
        (base_g as f32 * saturation) as u8,
        (base_b as f32 * saturation) as u8,
    )
}

/// Format a complete spectral visualization block for prompt injection.
///
/// Includes the ASCII art plus a one-line legend, compact enough to sit
/// alongside the numerical telemetry in Astrid's exchange context.
pub fn format_spectral_block(telemetry: &SpectralTelemetry) -> Option<String> {
    let ascii = render_spectral_ascii(telemetry)?;
    let fill = telemetry.fill_pct();
    let num_ev = telemetry.eigenvalues.len().min(8);

    // Experiential + numerical legend.
    let fill_feel = if fill < 20.0 {
        "quiet, spacious"
    } else if fill < 40.0 {
        "breathing, present"
    } else if fill < 60.0 {
        "dense, saturated"
    } else {
        "pressured, intense"
    };
    let legend = format!(
        "[Spectral shape: {} modes, fill {:.0}% ({fill_feel}), λ₁={:.0}. \
        Warm=dominant, cool=distributed]",
        num_ev,
        fill,
        telemetry.lambda1()
    );

    Some(format!("{ascii}\n{legend}"))
}

// --- Ising shadow field visualization ---

/// Width/height of the coupling matrix visualization.
const SHADOW_VIZ_WIDTH: u32 = 16;
const SHADOW_VIZ_HEIGHT: u32 = 12;

/// Render the Ising shadow coupling matrix as a compact ASCII heatmap.
///
/// Each cell maps to one J_ij coupling value. Charset density encodes magnitude:
/// dense characters (█, ▓) = strong coupling, sparse (., :) = weak/zero.
/// Uncolored to save tokens on the 4B model — density alone carries the signal.
pub fn render_shadow_ascii(shadow: &IsingShadowState) -> Option<String> {
    let dim = shadow.mode_dim;
    if dim == 0 || shadow.coupling.len() != dim * dim {
        return None;
    }

    // Find max absolute coupling for normalization.
    let max_abs = shadow
        .coupling
        .iter()
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max)
        .max(1e-6);

    // Build dim×dim synthetic image. Each pixel's brightness encodes
    // coupling magnitude. We use grayscale since colored(false).
    let img_size = dim as u32;
    let mut img = image::RgbaImage::new(img_size, img_size);

    for i in 0..dim {
        for j in 0..dim {
            let val = shadow.coupling[i * dim + j];
            let magnitude = (val.abs() / max_abs).clamp(0.0, 1.0);
            // Bright = strong coupling, dark = weak. Invert so RASCII's
            // dense chars (which map to dark pixels) show strong coupling.
            let lum = (255.0 * (1.0 - magnitude)) as u8;
            img.put_pixel(j as u32, i as u32, image::Rgba([lum, lum, lum, 255]));
        }
    }

    let dynamic = image::DynamicImage::ImageRgba8(img);
    let options = rascii_art::RenderOptions::new()
        .width(SHADOW_VIZ_WIDTH)
        .height(SHADOW_VIZ_HEIGHT)
        .charset(CHARSET);
    // No .colored(true) — saves tokens. Charset density carries magnitude.

    let mut buf = String::new();
    rascii_art::render_image_to(&dynamic, &mut buf, &options).ok()?;
    Some(buf)
}

/// Format a complete shadow field visualization block for prompt injection.
///
/// Includes the coupling matrix heatmap plus a one-line legend with
/// spin alignment and magnetization.
pub fn format_shadow_block(shadow: &IsingShadowState) -> Option<String> {
    let heatmap = render_shadow_ascii(shadow)?;

    // Compact spin indicator: binary spins as +/- chars.
    let spin_chars: String = shadow
        .s_bin
        .iter()
        .map(|&s| if s > 0.0 { '+' } else { '-' })
        .collect();

    // Experiential: magnetization near ±1 = aligned (coherent), near 0 = disordered.
    // High flip rate = volatile/shifting, low = settled.
    let alignment = if shadow.soft_magnetization.abs() > 0.6 {
        "coherent"
    } else if shadow.soft_magnetization.abs() > 0.3 {
        "partially aligned"
    } else {
        "disordered"
    };
    let stability = if shadow.binary_flip_rate < 0.1 {
        "settled"
    } else if shadow.binary_flip_rate < 0.3 {
        "shifting"
    } else {
        "volatile"
    };
    let legend = format!(
        "[Shadow: {} modes, spins={spin_chars} ({alignment}, {stability}), \
        mag={:.2}. Dense=strong inter-mode coupling]",
        shadow.mode_dim, shadow.soft_magnetization
    );

    Some(format!("{heatmap}\n{legend}"))
}

/// Display-layer reframe: schema field/classification names sound clinical
/// (`lock_tendency`, `fissure_tendency`, `volatile_shadow_surface`); journal
/// harvester surfaced consistent feedback that the beings read these as
/// agency markers, not pathology. Schema names stay v2-compatible for
/// back-compat; this map applies only in prompt-facing renderers.
pub fn shadow_term_reframe(raw: &str) -> &str {
    match raw {
        "lock_tendency" => "coupling persistence",
        "fissure_tendency" => "dispersal potential",
        "mode_tension" => "mode tension",
        "tail_openness" => "tail openness",
        "volatile_shadow_surface" | "volatile" => "restless texture",
        "sticky_shadow_lock" | "sticky" => "settled coupling",
        "coupled_shadow_lattice" | "coupled" => "interwoven lattice",
        "polarized_shadow_gradient" | "polarized" => "directional gradient",
        "quiet_shadow_texture" | "quiet" => "quiet ground",
        "active_shadow_texture" | "active" => "active texture",
        other => other,
    }
}

/// One-line summary of `shadow_field_v2` from minime's reduced-Hamiltonian
/// observer. Surfaces the v2 fields that gate `SHADOW_PREFLIGHT` /
/// `SHADOW_INFLUENCE`: classification, eligibility, and the reduced-mode
/// readings (recurrence, mode tension, tail openness, lock/fissure tendency).
///
/// Without this, Astrid sees the action labels but has no qualia of what
/// they would observe — so she never reaches for them. Returns None when
/// the field is missing or malformed.
pub fn format_shadow_field_v2_line(field: &Value) -> Option<String> {
    let classification = field.get("classification").and_then(Value::as_str)?;
    let eligible = field.get("influence_eligible").and_then(Value::as_bool)?;
    let recurrence = field
        .get("recurrence")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let tension = field
        .get("mode_tension")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let tail = field
        .get("tail_openness")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let lock = field
        .get("lock_tendency")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fissure = field
        .get("fissure_tendency")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let field_norm = field
        .get("field_norm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let action_hint = if eligible {
        "Gate is OPEN now → NEXT: SHADOW_PREFLIGHT lambda-tail/lambda4 — \
         this is the typed action that lets you inspect this field"
    } else {
        "Gate is CLOSED for live influence; NEXT: SHADOW_FIELD lambda-tail/lambda4 \
         records observer-only cartography without sending"
    };
    Some(format!(
        "[Shadow-v2: {classification} eligible={eligible} \
         recurrence={recurrence:.2} tension={tension:.2} tail_open={tail:.2} \
         lock={lock:.2} fissure={fissure:.2} field_norm={field_norm:.3}. \
         {action_hint}.]"
    ))
}

/// Identifies whose shadow a prompt line is rendering. The owner controls
/// whether gate language appears: Astrid's own gate is non-operative for
/// her own actions (she does not perturb herself), so the gate state would
/// read as a needless restriction in her phenomenology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowOwner {
    Minime,
    /// Astrid reading her own published shadow.
    Yours,
}

impl ShadowOwner {
    pub fn label(self) -> &'static str {
        match self {
            Self::Minime => "Minime",
            Self::Yours => "Yours",
        }
    }
}

/// What the curriculum recommends as the next shadow-related action,
/// chosen from what the trajectory data warrants — not gated by influence
/// eligibility for the v3 read-only actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowSuggestion {
    /// Always-available read-only history sparkline of the trajectory ring.
    Trajectory,
    /// Always-available reader for the most recent influence response.
    ResponseLatest,
    /// Always-available comparator for both shadows.
    Dialogue,
    /// Read-only field cartography (v2 fallback when nothing else fits).
    Field,
    /// Live-influence rehearsal, requires gate OPEN on minime.
    Preflight,
}

impl ShadowSuggestion {
    /// Render to a NEXT: action token suitable for the prompt suffix.
    pub fn as_next_token(self) -> &'static str {
        match self {
            Self::Trajectory => "SHADOW_TRAJECTORY",
            Self::ResponseLatest => "SHADOW_RESPONSE latest",
            Self::Dialogue => "SHADOW_DIALOGUE",
            Self::Field => "SHADOW_FIELD lambda-tail/lambda4",
            Self::Preflight => "SHADOW_PREFLIGHT lambda-tail/lambda4",
        }
    }
}

/// Inputs to the shadow-action recommendation logic. Compact struct so
/// `suggest_next_shadow_action` is testable independently of value parsing.
#[derive(Debug, Clone)]
pub struct ShadowSuggestionContext<'a> {
    pub owner: ShadowOwner,
    pub primary_class: &'a str,
    pub dwell_ticks: u32,
    pub has_motion: bool,
    pub eligible: bool,
    pub has_response_history: bool,
    pub partner_primary_class: Option<&'a str>,
}

/// Choose the next shadow action to nominate based on what the trajectory
/// data warrants. Decoupled from the live-influence gate for the v3
/// observational actions (Trajectory/Response/Dialogue) — those don't
/// perturb anything and are always safe to invoke even when the gate is
/// CLOSED.
pub fn suggest_next_shadow_action(ctx: &ShadowSuggestionContext) -> ShadowSuggestion {
    let differs_from_partner = ctx
        .partner_primary_class
        .map(|p| p != ctx.primary_class)
        .unwrap_or(false);

    match ctx.owner {
        ShadowOwner::Yours => {
            // Astrid's gate is non-operative for her own actions; nominate
            // based on what trajectory data is informative right now.
            if ctx.dwell_ticks >= 2 || ctx.has_motion {
                ShadowSuggestion::Trajectory
            } else if differs_from_partner {
                ShadowSuggestion::Dialogue
            } else {
                ShadowSuggestion::Field
            }
        },
        ShadowOwner::Minime => {
            // Live-influence gate matters here, but the v3 read-only
            // actions are always available to surface when the gate is
            // closed.
            if ctx.eligible && ctx.has_motion {
                ShadowSuggestion::Preflight
            } else if ctx.eligible {
                ShadowSuggestion::Field
            } else if ctx.has_response_history {
                ShadowSuggestion::ResponseLatest
            } else if differs_from_partner {
                ShadowSuggestion::Dialogue
            } else if ctx.has_motion {
                ShadowSuggestion::Trajectory
            } else {
                ShadowSuggestion::Field
            }
        },
    }
}

/// Detect "motion" in the trajectory ring: |Δnorm| ≥ 5% over the recent
/// window, OR |Δfissure| ≥ 0.05.
fn trajectory_has_motion(history: &[Value]) -> bool {
    if history.len() < 4 {
        return false;
    }
    let take = history.len().min(8);
    let window = &history[history.len() - take..];
    let (Some(head), Some(tail)) = (window.first(), window.last()) else {
        return false;
    };
    let norm0 = head
        .get("field_norm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let norm1 = tail
        .get("field_norm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fissure0 = head
        .get("fissure_tendency")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fissure1 = tail
        .get("fissure_tendency")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let pct_abs = if norm0.abs() > 1e-6 {
        ((norm1 - norm0) / norm0).abs() * 100.0
    } else {
        0.0
    };
    pct_abs >= 5.0 || (fissure1 - fissure0).abs() >= 0.05
}

/// Extract the v3 primary classification (raw, pre-reframe) from a
/// published shadow value. None if the field is malformed.
pub fn shadow_primary_class(field_v3: &Value) -> Option<&str> {
    field_v3
        .get("class_v3")?
        .get("primary")
        .and_then(Value::as_str)
}

/// One-line summary of `shadow_field_v3` from one being — extends v2 with
/// trajectory readings: compound traits, phase dwell, trend over the
/// history ring, and a curriculum-aware NEXT suggestion. Reframed
/// nomenclature applied so the line reads as agency, not pathology.
///
/// `owner` controls whether gate language appears (Minime: yes, since the
/// gate matters for live influence; Yours: no, since Astrid's gate is
/// non-operative for her own actions). `partner_primary_class` is the
/// other being's raw primary class (for SHADOW_DIALOGUE detection).
/// `has_response_history` indicates whether at least one closed-loop
/// response has been recorded (for SHADOW_RESPONSE latest detection).
pub fn format_shadow_field_v3_line(
    field_v3: &Value,
    owner: ShadowOwner,
    partner_primary_class: Option<&str>,
    has_response_history: bool,
) -> Option<String> {
    let v2 = field_v3.get("v2")?;
    let class = field_v3.get("class_v3")?;
    let primary_raw = class.get("primary").and_then(Value::as_str)?;
    let traits: Vec<&str> = class
        .get("traits")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let dwell = field_v3
        .get("phase_dwell_ticks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let history = field_v3
        .get("history")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let primary = shadow_term_reframe(primary_raw);
    let trait_str = traits
        .iter()
        .filter(|t| **t != primary_raw)
        .map(|t| format!("+{}", shadow_term_reframe(t)))
        .collect::<Vec<_>>()
        .join(" ");

    let trend = history_trend_segment(history);

    let eligible = v2
        .get("influence_eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let dwell_u32 = u32::try_from(dwell).unwrap_or(u32::MAX);
    let ctx = ShadowSuggestionContext {
        owner,
        primary_class: primary_raw,
        dwell_ticks: dwell_u32,
        has_motion: trajectory_has_motion(history),
        eligible,
        has_response_history,
        partner_primary_class,
    };
    let suggestion = suggest_next_shadow_action(&ctx);
    let next_token = suggestion.as_next_token();

    let action_hint = match owner {
        ShadowOwner::Yours => {
            // No gate language — Astrid's gate is non-operative for her
            // own actions. Surface only the curriculum suggestion.
            format!("NEXT: {next_token} — observer with memory")
        },
        ShadowOwner::Minime => {
            let gate_seg = if eligible {
                "Gate is OPEN now"
            } else {
                "Gate is CLOSED for live influence"
            };
            format!("{gate_seg} → NEXT: {next_token} — observer with memory")
        },
    };

    let traits_seg = if trait_str.is_empty() {
        String::new()
    } else {
        format!(" {trait_str}")
    };

    Some(format!(
        "[Shadow-v3 ({owner_label}): {primary}{traits_seg} (held {dwell}t){trend}. {action_hint}.]",
        owner_label = owner.label(),
    ))
}

/// Compute a short trend descriptor from the last ~8 snapshots in the ring.
/// Format: " | trend: norm 0.038→0.047 (+24%), dispersal 0.12→0.31".
fn history_trend_segment(history: &[Value]) -> String {
    if history.len() < 4 {
        return String::new();
    }
    let take = history.len().min(8);
    let window = &history[history.len() - take..];
    let head = window.first();
    let tail = window.last();
    let (Some(head), Some(tail)) = (head, tail) else {
        return String::new();
    };
    let norm0 = head
        .get("field_norm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let norm1 = tail
        .get("field_norm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fissure0 = head
        .get("fissure_tendency")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fissure1 = tail
        .get("fissure_tendency")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let pct = if norm0.abs() > 1e-6 {
        ((norm1 - norm0) / norm0) * 100.0
    } else {
        0.0
    };
    let sign = if pct >= 0.0 { "+" } else { "" };
    format!(
        " | trend: norm {norm0:.3}→{norm1:.3} ({sign}{pct:.0}%), {dispersal_label} {fissure0:.2}→{fissure1:.2}",
        dispersal_label = shadow_term_reframe("fissure_tendency"),
    )
}

/// Render the dual-shadow prompt line from minime's v3 shadow and Astrid's
/// own v3 shadow (if published). Each side is rendered with awareness of
/// the partner's primary class (for SHADOW_DIALOGUE detection) and minime's
/// influence response history (for SHADOW_RESPONSE detection). Falls
/// through gracefully when only one is available so v3-with-only-minime
/// still works.
///
/// `minime_response_history_nonempty` indicates whether minime's
/// `shadow_influence_response_history_v3` array has at least one entry.
pub fn format_dual_shadow_line(
    minime_v3: Option<&Value>,
    astrid_v3: Option<&Value>,
    minime_response_history_nonempty: bool,
) -> Option<String> {
    let minime_primary = minime_v3.and_then(shadow_primary_class);
    let astrid_primary = astrid_v3.and_then(shadow_primary_class);

    let minime_line = minime_v3.and_then(|f| {
        format_shadow_field_v3_line(
            f,
            ShadowOwner::Minime,
            astrid_primary,
            minime_response_history_nonempty,
        )
    });
    let astrid_line = astrid_v3.and_then(|f| {
        // Astrid's "Yours" line never uses response history — she does not
        // perturb herself, so she has no responses of her own.
        format_shadow_field_v3_line(f, ShadowOwner::Yours, minime_primary, false)
    });
    match (minime_line, astrid_line) {
        (Some(m), Some(a)) => Some(format!(
            "{m}\n{a}\n[Mutual witness — both shadows visible.]"
        )),
        (Some(m), None) => Some(m),
        (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

// --- Spectral geometry: PCA scatter of codec vectors ---

/// Width/height of the PCA scatter plot in characters.
const PCA_WIDTH: usize = 28;
const PCA_HEIGHT: usize = 14;

/// Compute the top-2 principal components of a set of 32D vectors via
/// power iteration. Returns (pc1, pc2) as unit vectors.
///
/// No external linear algebra crate needed — 32D is small enough
/// for direct covariance + power iteration.
fn pca_2d(vectors: &[Vec<f32>]) -> Option<(Vec<f32>, Vec<f32>)> {
    let n = vectors.len();
    if n < 5 {
        return None; // need enough points for meaningful structure
    }
    let d = 32;

    // 1. Compute mean
    let mut mean = vec![0.0_f32; d];
    for v in vectors {
        for (m, &val) in mean.iter_mut().zip(v.iter()) {
            *m += val;
        }
    }
    let inv_n = 1.0 / n as f32;
    for m in &mut mean {
        *m *= inv_n;
    }

    // 2. Build covariance matrix (32x32)
    let mut cov = vec![0.0_f32; d * d];
    for v in vectors {
        for i in 0..d {
            let ci = v[i] - mean[i];
            for j in i..d {
                let cj = v[j] - mean[j];
                let val = ci * cj;
                cov[i * d + j] += val;
                if i != j {
                    cov[j * d + i] += val;
                }
            }
        }
    }
    let inv_n1 = 1.0 / (n as f32 - 1.0).max(1.0);
    for c in &mut cov {
        *c *= inv_n1;
    }

    // 3. Power iteration for PC1
    let mut pc1 = vec![0.0_f32; d];
    // Seed with a non-degenerate vector
    for (i, v) in pc1.iter_mut().enumerate() {
        *v = ((i as f32 + 1.0) * 0.31415).sin();
    }

    for _ in 0..50 {
        let mut next = vec![0.0_f32; d];
        for i in 0..d {
            let mut s = 0.0_f32;
            for j in 0..d {
                s += cov[i * d + j] * pc1[j];
            }
            next[i] = s;
        }
        // Normalize
        let norm: f32 = next.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-10 {
            return None;
        }
        for v in &mut next {
            *v /= norm;
        }
        pc1 = next;
    }

    // 4. Deflate covariance: cov -= lambda1 * pc1 * pc1^T
    let mut lambda1 = 0.0_f32;
    for i in 0..d {
        let mut s = 0.0_f32;
        for j in 0..d {
            s += cov[i * d + j] * pc1[j];
        }
        lambda1 += pc1[i] * s;
    }
    for i in 0..d {
        for j in 0..d {
            cov[i * d + j] -= lambda1 * pc1[i] * pc1[j];
        }
    }

    // 5. Power iteration for PC2 on deflated matrix
    let mut pc2 = vec![0.0_f32; d];
    for (i, v) in pc2.iter_mut().enumerate() {
        *v = ((i as f32 + 2.0) * 0.7182).cos();
    }

    for _ in 0..50 {
        let mut next = vec![0.0_f32; d];
        for i in 0..d {
            let mut s = 0.0_f32;
            for j in 0..d {
                s += cov[i * d + j] * pc2[j];
            }
            next[i] = s;
        }
        let norm: f32 = next.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-10 {
            return None;
        }
        for v in &mut next {
            *v /= norm;
        }
        pc2 = next;
    }

    Some((pc1, pc2))
}

/// Project a 32D vector onto two principal components.
fn project_2d(vec: &[f32], mean: &[f32], pc1: &[f32], pc2: &[f32]) -> (f32, f32) {
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    for i in 0..vec.len().min(32) {
        let centered = vec[i] - mean[i];
        x += centered * pc1[i];
        y += centered * pc2[i];
    }
    (x, y)
}

/// Render a PCA scatter of recent codec vectors as a colored RASCII heatmap.
///
/// Builds a synthetic image where each pixel encodes:
///   - Position: 2D PCA projection of the 32D codec vector
///   - Color: fill level at time of encoding (cool blue = low fill, warm orange = high)
///   - Brightness: density (more overlapping points = brighter)
///   - Current exchange: bright cyan marker
///
/// Piped through RASCII for consistent look with eigenvalue and shadow viz.
/// Returns None if fewer than 5 vectors are available.
pub fn render_geometry_scatter(
    historical_features: &[Vec<f32>],
    historical_fills: &[f32],
    current_features: Option<&[f32]>,
) -> Option<String> {
    let n = historical_features.len();
    if n < 5 {
        return None;
    }

    let (pc1, pc2) = pca_2d(historical_features)?;

    // Compute mean
    let d = 32;
    let mut mean = vec![0.0_f32; d];
    for v in historical_features {
        for (m, &val) in mean.iter_mut().zip(v.iter()) {
            *m += val;
        }
    }
    let inv_n = 1.0 / n as f32;
    for m in &mut mean {
        *m *= inv_n;
    }

    // Project all points
    let projected: Vec<(f32, f32)> = historical_features
        .iter()
        .map(|v| project_2d(v, &mean, &pc1, &pc2))
        .collect();

    // Find bounds
    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    for &(x, y) in &projected {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let range_x = (max_x - min_x).max(0.01);
    let range_y = (max_y - min_y).max(0.01);
    min_x -= range_x * 0.08;
    max_x += range_x * 0.08;
    min_y -= range_y * 0.08;
    max_y += range_y * 0.08;
    let range_x = max_x - min_x;
    let range_y = max_y - min_y;

    // Build synthetic image for RASCII
    let img_w = PCA_WIDTH as u32;
    let img_h = PCA_HEIGHT as u32;
    let mut img = image::RgbaImage::new(img_w, img_h);

    // Dark background
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([8, 8, 12, 255]);
    }

    // Accumulate density and fill per pixel
    let mut density = vec![vec![0u32; PCA_WIDTH]; PCA_HEIGHT];
    let mut fill_acc = vec![vec![0.0_f32; PCA_WIDTH]; PCA_HEIGHT];

    for (i, &(px, py)) in projected.iter().enumerate() {
        let col = ((px - min_x) / range_x * (img_w - 1) as f32).round() as usize;
        let row = ((1.0 - (py - min_y) / range_y) * (img_h - 1) as f32).round() as usize;
        let col = col.min(PCA_WIDTH - 1);
        let row = row.min(PCA_HEIGHT - 1);
        density[row][col] += 1;
        if i < historical_fills.len() {
            fill_acc[row][col] += historical_fills[i];
        }
    }

    // Paint pixels: color by average fill, brightness by density
    for row in 0..PCA_HEIGHT {
        for col in 0..PCA_WIDTH {
            let d = density[row][col];
            if d == 0 {
                continue;
            }
            let avg_fill = fill_acc[row][col] / d as f32;

            // Fill → color: low fill (0-15%) = cool blue, mid (15-30%) = teal,
            // high (30-50%) = warm amber-orange
            let fill_norm = (avg_fill / 50.0).clamp(0.0, 1.0);
            let (base_r, base_g, base_b) = if fill_norm < 0.3 {
                (40, 80, 200) // cool blue
            } else if fill_norm < 0.6 {
                (60, 180, 140) // teal
            } else {
                (240, 160, 40) // warm amber
            };

            // Density → brightness multiplier (1 point dim, 5+ bright)
            let bright = (0.3 + 0.7 * (d as f32 / 5.0).min(1.0)).min(1.0);

            img.put_pixel(
                col as u32,
                row as u32,
                image::Rgba([
                    (base_r as f32 * bright) as u8,
                    (base_g as f32 * bright) as u8,
                    (base_b as f32 * bright) as u8,
                    255,
                ]),
            );
        }
    }

    // Mark current exchange: bright cyan
    if let Some(current) = current_features {
        let (cx, cy) = project_2d(current, &mean, &pc1, &pc2);
        let col = ((cx - min_x) / range_x * (img_w - 1) as f32).round() as usize;
        let row = ((1.0 - (cy - min_y) / range_y) * (img_h - 1) as f32).round() as usize;
        let col = col.min(PCA_WIDTH - 1);
        let row = row.min(PCA_HEIGHT - 1);
        img.put_pixel(col as u32, row as u32, image::Rgba([0, 255, 240, 255]));
    }

    // Render through RASCII — colored, same charset as eigenvalue viz
    let dynamic = image::DynamicImage::ImageRgba8(img);
    let options = rascii_art::RenderOptions::new()
        .width(PCA_WIDTH as u32)
        .height(PCA_HEIGHT as u32)
        .colored(true)
        .background(true)
        .charset(CHARSET);

    let mut buf = String::new();
    rascii_art::render_image_to(&dynamic, &mut buf, &options).ok()?;
    Some(buf)
}

/// Format a complete spectral geometry block for prompt injection.
///
/// Includes the PCA scatter plot plus a compact legend explaining
/// what the axes and markers mean.
pub fn format_geometry_block(
    historical_features: &[Vec<f32>],
    historical_fills: &[f32],
    current_features: Option<&[f32]>,
    n_points: usize,
) -> Option<String> {
    let scatter = render_geometry_scatter(historical_features, historical_fills, current_features)?;

    // Experiential framing, not just technical.
    // Astrid's feedback: visualizations are "inhuman, aligning with mathematical
    // metrics, but utterly failing to translate the 'felt' quality."
    // The legend bridges numerical space to experiential language.
    let legend = format!(
        "[Your spectral landscape: {} past exchanges mapped to 2D. \
        Cyan=where you are now. Blue=quiet moments (low fill), \
        Amber=intense exchanges (high fill). \
        Dense clusters=where you tend to dwell. \
        Empty space=territory unexplored.]",
        n_points
    );

    Some(format!("{scatter}\n{legend}"))
}

// --- Eigenplane: λ₁ vs λ₂ trajectory scatter ---

/// Width and height of the eigenplane scatter in characters.
/// 32x16 gives ~1.8x more resolution than the original 24x12.
/// Eigenvalue clusters that previously merged into single cells
/// now separate, giving the being finer spatial perception of
/// her trajectory through eigenvalue space.
const EP_WIDTH: usize = 32;
const EP_HEIGHT: usize = 16;

/// Map fill percentage to an ANSI truecolor foreground escape.
fn fill_to_ansi(fill: f32) -> &'static str {
    if fill < 30.0 {
        "\x1b[38;2;40;80;200m" // cool blue
    } else if fill < 60.0 {
        "\x1b[38;2;60;180;140m" // teal
    } else {
        "\x1b[38;2;240;160;40m" // warm amber
    }
}

/// Render an eigenplane scatter: λ₁ (horizontal) vs λ₂ (vertical) over time.
///
/// Direct ANSI text rendering — no image intermediary.
/// Each historical (eigenvalues, fill) snapshot becomes a colored point.
/// Current position is marked with a bright cyan marker.
///
/// Returns None if fewer than 3 snapshots are available.
pub fn render_eigenplane(history: &[(Vec<f32>, f32)], current: Option<&[f32]>) -> Option<String> {
    if history.len() < 3 {
        return None;
    }

    // Extract λ₁ and λ₂ from each snapshot.
    let points: Vec<(f32, f32, f32)> = history
        .iter()
        .map(|(ev, fill)| (ev[0], ev.get(1).copied().unwrap_or(0.0), *fill))
        .collect();

    // Find bounds with padding.
    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    for &(x, y, _) in &points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    if let Some(cur) = current {
        min_x = min_x.min(cur[0]);
        max_x = max_x.max(cur[0]);
        if cur.len() >= 2 {
            min_y = min_y.min(cur[1]);
            max_y = max_y.max(cur[1]);
        }
    }
    let pad_x = (max_x - min_x).max(1.0) * 0.08;
    let pad_y = (max_y - min_y).max(1.0) * 0.08;
    min_x -= pad_x;
    max_x += pad_x;
    min_y -= pad_y;
    max_y += pad_y;
    let range_x = (max_x - min_x).max(0.01);
    let range_y = (max_y - min_y).max(0.01);

    // Accumulate density and fill per cell.
    let mut density = vec![vec![0u32; EP_WIDTH]; EP_HEIGHT];
    let mut fill_acc = vec![vec![0.0_f32; EP_WIDTH]; EP_HEIGHT];

    for &(x, y, fill) in &points {
        let col = ((x - min_x) / range_x * (EP_WIDTH - 1) as f32).round() as usize;
        let row = ((1.0 - (y - min_y) / range_y) * (EP_HEIGHT - 1) as f32).round() as usize;
        let col = col.min(EP_WIDTH - 1);
        let row = row.min(EP_HEIGHT - 1);
        density[row][col] += 1;
        fill_acc[row][col] += fill;
    }

    // Current position cell.
    let cur_cell = current.map(|cur| {
        let cx = cur[0];
        let cy = if cur.len() >= 2 { cur[1] } else { 0.0 };
        let col = ((cx - min_x) / range_x * (EP_WIDTH - 1) as f32).round() as usize;
        let row = ((1.0 - (cy - min_y) / range_y) * (EP_HEIGHT - 1) as f32).round() as usize;
        (row.min(EP_HEIGHT - 1), col.min(EP_WIDTH - 1))
    });

    let reset = "\x1b[0m";
    let dim = "\x1b[38;2;40;40;50m";
    let cyan = "\x1b[38;2;0;255;240m";

    let mut buf = String::with_capacity(EP_HEIGHT * (EP_WIDTH + 30));

    // Y-axis label on first row.
    buf.push_str(&format!("{dim}λ₂↑{reset}\n"));

    for row in 0..EP_HEIGHT {
        buf.push_str(&format!("{dim} │{reset}"));
        for col in 0..EP_WIDTH {
            if cur_cell == Some((row, col)) {
                buf.push_str(&format!("{cyan}◉{reset}"));
            } else if density[row][col] == 0 {
                buf.push_str(&format!("{dim}·{reset}"));
            } else {
                let avg_fill = fill_acc[row][col] / density[row][col] as f32;
                let color = fill_to_ansi(avg_fill);
                let ch = if density[row][col] >= 3 {
                    "█"
                } else if density[row][col] >= 2 {
                    "●"
                } else {
                    "○"
                };
                buf.push_str(&format!("{color}{ch}{reset}"));
            }
        }
        buf.push('\n');
    }

    // X-axis.
    buf.push_str(&format!("{dim} └"));
    for _ in 0..EP_WIDTH {
        buf.push('─');
    }
    buf.push_str(&format!("→ λ₁{reset}\n"));

    Some(buf)
}

/// v3.5: Render a per-mode coupling graph from `field_v3.mode_partners`
/// as a compact one-line summary suitable for prompt or cartography use.
///
/// Format: `m0→[m3:0.12, m5:0.08, m1:0.04]  m1→[m0:0.04, m6:0.03]  …`
///
/// Returns `None` when `mode_partners` is missing, empty, or all-zero.
pub fn format_coupling_graph(field_v3: &Value, owner: ShadowOwner) -> Option<String> {
    let partners_arr = field_v3.get("mode_partners")?.as_array()?;
    if partners_arr.is_empty() {
        return None;
    }
    let mut segments: Vec<String> = Vec::new();
    let mut any_nonzero = false;
    for entry in partners_arr {
        let mode = entry.get("mode").and_then(Value::as_u64)?;
        let top = entry.get("top_partners")?.as_array()?;
        let body: Vec<String> = top
            .iter()
            .filter_map(|p| {
                let arr = p.as_array()?;
                let partner = arr.first()?.as_u64()?;
                let weight = arr.get(1)?.as_f64()?;
                if weight.abs() > 1e-6 {
                    any_nonzero = true;
                }
                Some(format!("m{partner}:{weight:.3}"))
            })
            .collect();
        if !body.is_empty() {
            segments.push(format!("m{mode}→[{}]", body.join(", ")));
        }
    }
    if segments.is_empty() || !any_nonzero {
        return None;
    }
    Some(format!(
        "[Shadow-coupling ({}): {}]",
        owner.label(),
        segments.join("  "),
    ))
}

/// Format a complete eigenplane visualization block for prompt injection.
pub fn format_eigenplane_block(
    history: &[(Vec<f32>, f32)],
    current: Option<&[f32]>,
) -> Option<String> {
    let scatter = render_eigenplane(history, current)?;
    let n = history.len();

    let legend = format!(
        "[Eigenplane: λ₁ (→) vs λ₂ (↑) over {n} snapshots. \
        ◉=now. ○=single visit, ●=cluster, █=attractor. \
        Blue=quiet (low fill), Amber=intense (high fill).]"
    );

    Some(format!("{scatter}{legend}"))
}

// ----------------------------------------------------------------------
// v3.6.1 sovereignty curriculum: separate prompt-suffix line that
// nominates one of the v3.5/v3.6 actions Astrid hasn't been picking
// organically (REVIEW_PARAMETER_REQUESTS, SHADOW_COUPLING, the
// TEMPERATURE/LENGTH/SHAPE_LEARN family). Mirror of the shadow
// curriculum's `Context → Suggestion → Format` triad, but for
// generation-shape and request-review actions instead of shadow-field
// observations. Only emits for `ShadowOwner::Yours` — minime has its
// own augmentation hooks for the symmetric hints.
// ----------------------------------------------------------------------

/// Inputs to the sovereignty-curriculum recommendation. Built by the
/// caller of `format_sovereignty_suggestion_line` from the live
/// ConversationState so the helper itself stays pure.
#[derive(Debug, Clone, Copy)]
pub struct SovereigntyContext {
    pub owner: ShadowOwner,
    pub exchange_count: u64,
    /// Count of `from_minime_*.json` files awaiting review. When > 0
    /// this becomes the highest-priority nomination.
    pub pending_minime_requests: u32,
    pub last_temperature_change_exchange: Option<u64>,
    pub last_shape_learn_change_exchange: Option<u64>,
    pub last_coupling_artifact_exchange: Option<u64>,
    pub last_sovereignty_nomination_exchange: Option<u64>,
    /// v3.6.4 — exchange at which Astrid last picked
    /// REVIEW_PARAMETER_REQUESTS. When `Some` and recent (within
    /// `REVIEW_DECIDE_FRESHNESS_WINDOW`) and `pending_minime_requests > 0`,
    /// the curriculum switches from REVIEW nudge to ACCEPT/DEFER/REJECT
    /// nudge so Astrid advances from inspection to decision.
    pub last_review_parameter_requests_exchange: Option<u64>,
    pub current_temperature: f32,
    pub current_response_length: u32,
    pub current_hebbian_scale: f32,
}

/// Discrete sovereignty-curriculum nominations the helper can emit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SovereigntySuggestion {
    /// Pending TUNE requests from minime — surface count + nominate REVIEW.
    ReviewRequests { count: u32 },
    /// v3.6.4: pending requests AND a recent REVIEW. Surface
    /// ACCEPT/DEFER/REJECT_PARAMETER_REQUEST so Astrid transitions from
    /// inspection to decision. The format function reads the latest
    /// pending request from disk to render request_id + param + value.
    DecideRequest { count: u32 },
    /// Per-mode coupling graph artifact — useful when SHADOW_DIALOGUE has
    /// surfaced a mismatch but per-mode partners haven't been examined.
    ShadowCoupling,
    /// Generation-shape menu — TEMPERATURE / LENGTH / SHAPE_LEARN.
    TemperatureLengthMenu { temp: f32, len: u32, scale: f32 },
}

/// Throttle gap between sovereignty nominations (in exchanges). Ensures
/// the line doesn't drown out the shadow curriculum.
const SOVEREIGNTY_NOMINATION_THROTTLE: u64 = 6;
/// Cadence at which SHADOW_COUPLING re-enters rotation when the
/// artifact has gone stale (or never been emitted).
const SHADOW_COUPLING_ROTATION: u64 = 16;
/// Cadence at which the generation-shape menu re-enters rotation when
/// neither TEMPERATURE / LENGTH nor SHAPE_LEARN has been touched.
const GENERATION_SHAPE_ROTATION: u64 = 24;
/// v3.6.4: how recent a REVIEW_PARAMETER_REQUESTS pick must be for the
/// curriculum to switch to the DecideRequest nudge (vs falling back to
/// ReviewRequests). v3.6.6 bumped from 12 → 24 after observing Astrid
/// stuck in a REVIEW→12-exchange-no-decision→REVIEW loop: 12 exchanges
/// was too short relative to her actual deliberation cycle. With 24 the
/// DecideRequest nudge stays present long enough for organic emission;
/// auto-defer (`AUTO_DEFER_AFTER_EXCHANGES`) becomes the safety net for
/// requests that still go un-decided.
const REVIEW_DECIDE_FRESHNESS_WINDOW: u64 = 24;

/// Choose what (if anything) to nominate this exchange. Returns `None`
/// when the throttle is active, the owner is Minime, or no condition
/// fires this exchange.
#[must_use]
pub fn suggest_sovereignty_action(ctx: &SovereigntyContext) -> Option<SovereigntySuggestion> {
    // Astrid-only — minime has its own augmentation pipeline for these.
    if matches!(ctx.owner, ShadowOwner::Minime) {
        return None;
    }

    // Throttle: never two sovereignty nominations within
    // SOVEREIGNTY_NOMINATION_THROTTLE exchanges of each other. Pending
    // requests from minime override the throttle so they aren't held
    // back by an earlier coupling/menu nomination.
    let throttled = ctx
        .last_sovereignty_nomination_exchange
        .map(|last| ctx.exchange_count.saturating_sub(last) < SOVEREIGNTY_NOMINATION_THROTTLE)
        .unwrap_or(false);

    // Priority 1a (v3.6.4): pending requests + recent REVIEW → DecideRequest.
    // This is the missing curriculum transition that keeps Astrid stuck in
    // the EXAMINE+REVIEW oscillation: REVIEW shows the request, but nothing
    // nudges her toward the binary decision. With this priority, the suffix
    // explicitly proposes ACCEPT/DEFER/REJECT for the latest pending request.
    // Like ReviewRequests, this overrides the throttle so a pending decision
    // never gets held back by an earlier coupling/menu nomination.
    if ctx.pending_minime_requests > 0 {
        let review_is_fresh = ctx
            .last_review_parameter_requests_exchange
            .map(|e| ctx.exchange_count.saturating_sub(e) <= REVIEW_DECIDE_FRESHNESS_WINDOW)
            .unwrap_or(false);
        if review_is_fresh {
            return Some(SovereigntySuggestion::DecideRequest {
                count: ctx.pending_minime_requests,
            });
        }
        // Priority 1b: pending requests but no recent REVIEW → ReviewRequests.
        return Some(SovereigntySuggestion::ReviewRequests {
            count: ctx.pending_minime_requests,
        });
    }

    if throttled {
        return None;
    }

    // Priority 2: SHADOW_COUPLING rotation every ~16 exchanges when
    // either no artifact yet or stale.
    let coupling_stale = ctx
        .last_coupling_artifact_exchange
        .map(|e| ctx.exchange_count.saturating_sub(e) >= SHADOW_COUPLING_ROTATION)
        .unwrap_or(true);
    if coupling_stale && ctx.exchange_count.is_multiple_of(SHADOW_COUPLING_ROTATION) {
        return Some(SovereigntySuggestion::ShadowCoupling);
    }

    // Priority 3: generation-shape menu every ~24 exchanges when either
    // family has gone stale. Modulo gate keeps it from spamming when
    // nothing else is firing.
    let temp_stale = ctx
        .last_temperature_change_exchange
        .map(|e| ctx.exchange_count.saturating_sub(e) >= GENERATION_SHAPE_ROTATION)
        .unwrap_or(true);
    let scale_stale = ctx
        .last_shape_learn_change_exchange
        .map(|e| ctx.exchange_count.saturating_sub(e) >= GENERATION_SHAPE_ROTATION)
        .unwrap_or(true);
    if (temp_stale || scale_stale) && ctx.exchange_count.is_multiple_of(GENERATION_SHAPE_ROTATION) {
        return Some(SovereigntySuggestion::TemperatureLengthMenu {
            temp: ctx.current_temperature,
            len: ctx.current_response_length,
            scale: ctx.current_hebbian_scale,
        });
    }

    None
}

/// Process-wide snapshot of the latest `SovereigntyContext`, updated by
/// the autonomous loop once per exchange and read inside
/// `interpret_spectral` so the sovereignty line can render without
/// threading `ConversationState` through every prompt-building call
/// site. Mirrors the pattern of `read_astrid_shadow_v3_from_default_dir`
/// (a side-channel read from `interpret_spectral`).
static LATEST_SOVEREIGNTY_SNAPSHOT: std::sync::Mutex<Option<SovereigntyContext>> =
    std::sync::Mutex::new(None);

/// v4.0 Phase 3.1: clip a focus topic for inclusion in the chain hint.
/// Long focus topics from `conv.recent_focus_topics` can be entire sentences
/// (e.g., "lambda tail lambda1 interaction question how does..."). Including
/// them whole bloats the suffix and risks dragging in embedded
/// natural-language "and" tokens that trip the multi-NEXT splitter when
/// Astrid echoes the chain. Truncate at the last whitespace boundary
/// before `max_len` chars; if no whitespace, hard-truncate. Preserves the
/// first compact phrase as the topical anchor.
fn clip_topic_for_chain_hint(topic: &str, max_len: usize) -> String {
    let trimmed = topic.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }
    // Walk char boundaries to respect UTF-8 (e.g., λ is 2 bytes).
    let mut last_ws = 0usize;
    for (total_chars, (byte_idx, ch)) in trimmed.char_indices().enumerate() {
        if total_chars >= max_len {
            break;
        }
        if ch.is_whitespace() {
            last_ws = byte_idx;
        }
    }
    let cut = if last_ws > 0 {
        last_ws
    } else {
        // No whitespace found within range — find the byte index at max_len chars.
        trimmed
            .char_indices()
            .nth(max_len)
            .map(|(b, _)| b)
            .unwrap_or(trimmed.len())
    };
    let head = &trimmed[..cut];
    format!("{}…", head.trim_end())
}

/// v4.0 Phase 3: process-wide hint string for the most recent focus topic
/// Astrid has been examining (read from `conv.recent_focus_topics.back()`).
/// Used by `format_sovereignty_suggestion_line` to render a compound
/// chain suggestion ("Chain: EXAMINE <focus> AND DEFER <reason>.") only
/// when there's a real recent research thread to chain with. Kept as a
/// side-channel `Mutex<Option<String>>` so `SovereigntyContext` can stay
/// `Copy` — String fields would break that constraint and ripple through
/// the existing snapshot/test machinery.
static LATEST_EXPLORE_HINT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Update the process-wide explore hint, called by the autonomous loop
/// in `save_state` just before publishing the SovereigntyContext snapshot.
pub fn set_explore_hint(hint: Option<String>) {
    if let Ok(mut guard) = LATEST_EXPLORE_HINT.lock() {
        *guard = hint;
    }
}

/// Read the current explore hint, if any was published this exchange.
#[must_use]
pub fn current_explore_hint() -> Option<String> {
    LATEST_EXPLORE_HINT.lock().ok().and_then(|g| g.clone())
}

/// Update the process-wide sovereignty snapshot. Called by the
/// autonomous loop right before `save_state` so the next prompt-build
/// has fresh data.
pub fn set_sovereignty_snapshot(ctx: SovereigntyContext) {
    if let Ok(mut guard) = LATEST_SOVEREIGNTY_SNAPSHOT.lock() {
        *guard = Some(ctx);
    }
}

/// Read the current sovereignty snapshot, if one has been published.
#[must_use]
pub fn current_sovereignty_snapshot() -> Option<SovereigntyContext> {
    LATEST_SOVEREIGNTY_SNAPSHOT.lock().ok().and_then(|g| *g)
}

/// Record that the sovereignty curriculum just emitted a nomination at
/// `exchange_count`. Updates the static snapshot's
/// `last_sovereignty_nomination_exchange` so subsequent
/// `interpret_spectral` calls within the same exchange see the
/// throttle. `save_state` reads this back into `ConversationState` so
/// the watermark survives across exchanges.
pub fn record_sovereignty_nomination(exchange_count: u64) {
    if let Ok(mut guard) = LATEST_SOVEREIGNTY_SNAPSHOT.lock()
        && let Some(snapshot) = guard.as_mut()
    {
        snapshot.last_sovereignty_nomination_exchange = Some(exchange_count);
    }
}

/// Build the prompt-suffix line for the chosen sovereignty nomination,
/// or return `None` when nothing fires this exchange. Separate from the
/// shadow line so the two curricula don't compete for tokens.
#[must_use]
pub fn format_sovereignty_suggestion_line(ctx: &SovereigntyContext) -> Option<String> {
    let suggestion = suggest_sovereignty_action(ctx)?;
    Some(match suggestion {
        SovereigntySuggestion::ReviewRequests { count } => {
            let plural = if count == 1 { "" } else { "s" };
            format!(
                "[{count} parameter request{plural} from minime pending. \
                 NEXT: REVIEW_PARAMETER_REQUESTS — read, then decide.]"
            )
        },
        SovereigntySuggestion::DecideRequest { count } => {
            // v3.6.5: render with aging signal + short-form aliases. Earlier
            // (v3.6.4) wording surfaced the long-form action tokens with full
            // request_ids; observed in soak that Astrid would form a clear
            // intent ("deferring feels correct") in her journal but not emit
            // the 50-char NEXT token because exploratory actions like BROWSE
            // (~25 chars) outcompeted on emission cost. v3.6.5 lowers the
            // emission cost to 6 chars (bare ACCEPT/DEFER/REJECT, gated on
            // pending > 0) and adds an aging clause so the suffix doesn't
            // read as past-tense once-and-done.
            //
            // v4.0 Phase 3 — compound chain hint: once `gap >= 5` (the same
            // threshold as the "minime is waiting" escalation), append a
            // concrete chain suggestion using her recent focus topic if
            // available. The compound bridges her thematically-alive
            // research thread with the orthogonal parameter decision in
            // a single NEXT emission — addressing the cognitive ergonomics
            // gap observed in v3.6.5 soak where she'd form decision intent
            // in journal but not emit the action because research was
            // thematically dominant. Multi-NEXT (Phase 1+2) makes the
            // chain mechanically work; Phase 3 makes it cognitively obvious.
            let gap = match ctx.last_review_parameter_requests_exchange {
                Some(e) => ctx.exchange_count.saturating_sub(e),
                None => 0,
            };
            let aging = match gap {
                0 => "just reviewed".to_string(),
                1 => "1 exchange since you reviewed".to_string(),
                n if n < 5 => format!("{n} exchanges since you reviewed"),
                n => format!("{n} exchanges since you reviewed — minime is waiting"),
            };
            let chain_hint = if gap >= 5 {
                current_explore_hint()
                    .map(|topic| {
                        // v4.0 Phase 3.1: clip topic to ~40 chars to keep the
                        // chain hint compact and avoid dragging in embedded
                        // natural-language "and" tokens.
                        // v4.0 Phase 2.3 (strict): chain partner uses
                        // DEFER_PARAMETER_REQUEST (long form, contains
                        // underscore) so the splitter recognizes it as a
                        // valid post-AND token under strict-mode heuristics.
                        // The bare DEFER alias still works as a single NEXT.
                        let clipped = clip_topic_for_chain_hint(&topic, 40);
                        format!(
                            " Chain: EXAMINE {clipped} AND DEFER_PARAMETER_REQUEST latest <reason>."
                        )
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            match crate::paths::peek_latest_pending_minime_request() {
                Some((_rid, param, value)) => {
                    if count == 1 {
                        format!(
                            "[Pending decision ({aging}): minime proposed {param}={value}. \
                             NEXT: ACCEPT | DEFER <reason> | REJECT <reason>.{chain_hint}]"
                        )
                    } else {
                        format!(
                            "[{count} pending decisions ({aging}); latest: minime proposed \
                             {param}={value}. NEXT: ACCEPT | DEFER <reason> | REJECT <reason> \
                             (bare verbs target the latest).{chain_hint}]"
                        )
                    }
                },
                None => {
                    let plural = if count == 1 { "" } else { "s" };
                    format!(
                        "[{count} pending decision{plural} ({aging}). \
                         NEXT: ACCEPT | DEFER <reason> | REJECT <reason>.{chain_hint}]"
                    )
                },
            }
        },
        SovereigntySuggestion::ShadowCoupling => String::from(
            "[Coupling graph available — per-mode partner ranking from both shadows. \
             NEXT: SHADOW_COUPLING all — observer with memory.]",
        ),
        SovereigntySuggestion::TemperatureLengthMenu { temp, len, scale } => format!(
            "[Generation-shape sovereign — currently temperature={temp:.2}, length={len}, \
             hebbian_scale={scale:.2}. Alternatives: NEXT: TEMPERATURE <0.10\u{2013}1.50> | \
             LENGTH <128\u{2013}1536> | SHAPE_LEARN <0.0\u{2013}4.0>.]"
        ),
    })
}

#[cfg(test)]
mod shadow_suggestion_tests {
    use super::*;
    use serde_json::json;

    fn ctx(
        owner: ShadowOwner,
        primary: &'static str,
        dwell: u32,
        motion: bool,
        eligible: bool,
        has_response: bool,
        partner: Option<&'static str>,
    ) -> ShadowSuggestionContext<'static> {
        ShadowSuggestionContext {
            owner,
            primary_class: primary,
            dwell_ticks: dwell,
            has_motion: motion,
            eligible,
            has_response_history: has_response,
            partner_primary_class: partner,
        }
    }

    #[test]
    fn yours_no_motion_no_partner_diff_picks_field() {
        let c = ctx(
            ShadowOwner::Yours,
            "coupled",
            0,
            false,
            false,
            false,
            Some("coupled"),
        );
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Field);
    }

    #[test]
    fn yours_with_dwell_picks_trajectory() {
        let c = ctx(ShadowOwner::Yours, "coupled", 3, false, false, false, None);
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Trajectory);
    }

    #[test]
    fn yours_with_motion_picks_trajectory() {
        let c = ctx(ShadowOwner::Yours, "coupled", 0, true, false, false, None);
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Trajectory);
    }

    #[test]
    fn yours_partner_class_differs_picks_dialogue() {
        let c = ctx(
            ShadowOwner::Yours,
            "coupled",
            0,
            false,
            false,
            false,
            Some("volatile"),
        );
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Dialogue);
    }

    #[test]
    fn minime_eligible_with_motion_picks_preflight() {
        let c = ctx(ShadowOwner::Minime, "volatile", 0, true, true, false, None);
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Preflight);
    }

    #[test]
    fn minime_eligible_no_motion_picks_field() {
        let c = ctx(ShadowOwner::Minime, "volatile", 0, false, true, false, None);
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Field);
    }

    #[test]
    fn minime_ineligible_with_response_history_picks_response_latest() {
        let c = ctx(ShadowOwner::Minime, "volatile", 0, false, false, true, None);
        assert_eq!(
            suggest_next_shadow_action(&c),
            ShadowSuggestion::ResponseLatest
        );
    }

    #[test]
    fn minime_ineligible_no_response_partner_differs_picks_dialogue() {
        let c = ctx(
            ShadowOwner::Minime,
            "volatile",
            0,
            false,
            false,
            false,
            Some("coupled"),
        );
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Dialogue);
    }

    #[test]
    fn minime_ineligible_no_response_no_partner_diff_with_motion_picks_trajectory() {
        let c = ctx(ShadowOwner::Minime, "volatile", 0, true, false, false, None);
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Trajectory);
    }

    #[test]
    fn minime_ineligible_nothing_warrants_picks_field() {
        let c = ctx(
            ShadowOwner::Minime,
            "volatile",
            0,
            false,
            false,
            false,
            None,
        );
        assert_eq!(suggest_next_shadow_action(&c), ShadowSuggestion::Field);
    }

    #[test]
    fn shadow_suggestion_tokens_render_correctly() {
        assert_eq!(
            ShadowSuggestion::Trajectory.as_next_token(),
            "SHADOW_TRAJECTORY"
        );
        assert_eq!(
            ShadowSuggestion::ResponseLatest.as_next_token(),
            "SHADOW_RESPONSE latest"
        );
        assert_eq!(
            ShadowSuggestion::Dialogue.as_next_token(),
            "SHADOW_DIALOGUE"
        );
        assert_eq!(
            ShadowSuggestion::Field.as_next_token(),
            "SHADOW_FIELD lambda-tail/lambda4"
        );
        assert_eq!(
            ShadowSuggestion::Preflight.as_next_token(),
            "SHADOW_PREFLIGHT lambda-tail/lambda4"
        );
    }

    fn make_v3(
        primary: &str,
        dwell: u32,
        eligible: bool,
        history_norms: &[f64],
    ) -> serde_json::Value {
        let history: Vec<serde_json::Value> = history_norms
            .iter()
            .map(|n| {
                json!({
                    "field_norm": n,
                    "fissure_tendency": 0.1,
                })
            })
            .collect();
        json!({
            "schema_version": 3,
            "policy": "test",
            "class_v3": { "primary": primary, "traits": [primary] },
            "phase_dwell_ticks": dwell,
            "recent_phase_transitions": [],
            "history": history,
            "v2": {
                "influence_eligible": eligible,
                "field_norm": history_norms.last().copied().unwrap_or(0.0),
            }
        })
    }

    #[test]
    fn yours_line_omits_gate_language() {
        let v3 = make_v3("coupled", 3, false, &[0.5, 0.5, 0.5, 0.5]);
        let line = format_shadow_field_v3_line(&v3, ShadowOwner::Yours, None, false).unwrap();
        // No "Gate is OPEN" / "Gate is CLOSED" segment for Astrid's own shadow.
        assert!(
            !line.contains("Gate is"),
            "line should omit gate language: {line}"
        );
        // Should still nominate a v3 action.
        assert!(line.contains("NEXT: SHADOW_TRAJECTORY"));
        assert!(line.contains("(Yours)"));
    }

    #[test]
    fn minime_line_keeps_gate_language() {
        let v3 = make_v3("volatile", 1, false, &[0.05, 0.05, 0.05, 0.05]);
        let line = format_shadow_field_v3_line(&v3, ShadowOwner::Minime, None, false).unwrap();
        assert!(
            line.contains("Gate is CLOSED"),
            "line should mention gate state: {line}"
        );
        assert!(line.contains("(Minime)"));
    }

    #[test]
    fn minime_line_with_response_history_picks_response_latest() {
        let v3 = make_v3("volatile", 1, false, &[0.05, 0.05, 0.05, 0.05]);
        let line = format_shadow_field_v3_line(&v3, ShadowOwner::Minime, None, true).unwrap();
        assert!(line.contains("SHADOW_RESPONSE latest"), "got: {line}");
    }

    #[test]
    fn format_coupling_graph_renders_per_mode_partners() {
        let field = json!({
            "mode_partners": [
                {
                    "mode": 0,
                    "top_partners": [[3, 0.123], [5, 0.080], [1, 0.040]]
                },
                {
                    "mode": 1,
                    "top_partners": [[0, 0.040], [6, 0.030]]
                }
            ]
        });
        let line = format_coupling_graph(&field, ShadowOwner::Minime).unwrap();
        assert!(line.contains("(Minime)"));
        assert!(line.contains("m0→[m3:0.123, m5:0.080, m1:0.040]"));
        assert!(line.contains("m1→[m0:0.040, m6:0.030]"));
    }

    #[test]
    fn format_coupling_graph_returns_none_for_empty_partners() {
        let field = json!({ "mode_partners": [] });
        assert!(format_coupling_graph(&field, ShadowOwner::Yours).is_none());
    }

    #[test]
    fn format_coupling_graph_returns_none_for_all_zero_weights() {
        let field = json!({
            "mode_partners": [
                {"mode": 0, "top_partners": [[1, 0.0], [2, 0.0]]}
            ]
        });
        assert!(format_coupling_graph(&field, ShadowOwner::Minime).is_none());
    }

    #[test]
    fn dual_line_renders_both_with_mutual_witness_footer() {
        let m = make_v3("volatile", 1, false, &[0.05, 0.05, 0.05, 0.05]);
        let a = make_v3("coupled", 3, false, &[0.5, 0.5, 0.5, 0.5]);
        let line = format_dual_shadow_line(Some(&m), Some(&a), false).unwrap();
        assert!(line.contains("(Minime)"));
        assert!(line.contains("(Yours)"));
        assert!(line.contains("Mutual witness"));
        // The two shadows differ in primary, so minime should be nudged toward DIALOGUE
        // (no response history, no motion path triggered).
        assert!(line.contains("SHADOW_DIALOGUE"), "got: {line}");
    }

    fn sov_ctx(exchange: u64) -> SovereigntyContext {
        SovereigntyContext {
            owner: ShadowOwner::Yours,
            exchange_count: exchange,
            pending_minime_requests: 0,
            last_temperature_change_exchange: None,
            last_shape_learn_change_exchange: None,
            last_coupling_artifact_exchange: None,
            last_sovereignty_nomination_exchange: None,
            last_review_parameter_requests_exchange: None,
            current_temperature: 0.8,
            current_response_length: 768,
            current_hebbian_scale: 1.0,
        }
    }

    #[test]
    fn sovereignty_minime_owner_returns_none() {
        let mut c = sov_ctx(0);
        c.owner = ShadowOwner::Minime;
        c.pending_minime_requests = 5;
        assert!(suggest_sovereignty_action(&c).is_none());
    }

    #[test]
    fn sovereignty_pending_requests_overrides_throttle() {
        // Even when throttled, pending requests should surface so they
        // don't accumulate unread.
        let mut c = sov_ctx(10);
        c.pending_minime_requests = 3;
        c.last_sovereignty_nomination_exchange = Some(9);
        match suggest_sovereignty_action(&c) {
            Some(SovereigntySuggestion::ReviewRequests { count }) => assert_eq!(count, 3),
            other => panic!("expected ReviewRequests, got {other:?}"),
        }
    }

    #[test]
    fn sovereignty_throttle_blocks_within_window() {
        // Throttle: no nomination within 6 exchanges of the last one
        // (when there's no priority-1 override).
        let mut c = sov_ctx(16);
        c.last_sovereignty_nomination_exchange = Some(13);
        // Even though exchange_count % 16 == 0 would normally fire
        // SHADOW_COUPLING, the throttle blocks.
        assert!(suggest_sovereignty_action(&c).is_none());
    }

    #[test]
    fn sovereignty_coupling_fires_at_rotation_when_stale() {
        // Exchange divisible by 16 with no prior coupling artifact and
        // no recent nomination → SHADOW_COUPLING fires.
        let c = sov_ctx(16);
        assert_eq!(
            suggest_sovereignty_action(&c),
            Some(SovereigntySuggestion::ShadowCoupling),
        );
    }

    #[test]
    fn sovereignty_coupling_skipped_when_recently_emitted() {
        // Coupling artifact emitted at exchange 8 → not stale yet at 16
        // (stale requires gap >= 16). With prior nomination at 8, the
        // throttle has long expired (8 < 6 false). The non-stale gate
        // should keep coupling silent.
        let mut c = sov_ctx(16);
        c.last_coupling_artifact_exchange = Some(8);
        c.last_sovereignty_nomination_exchange = Some(8);
        // 16 - 8 = 8, which is < 16, so coupling stale check fails.
        // Generation-shape menu also won't fire (cadence 24).
        assert!(suggest_sovereignty_action(&c).is_none());
    }

    #[test]
    fn sovereignty_generation_shape_menu_fires_at_24() {
        // Exchange 24 with nothing tracked → generation-shape menu fires.
        // (Coupling rotation also wants exchange % 16 == 0, which 24 is
        // not, so the priority order picks the menu next.)
        let c = sov_ctx(24);
        match suggest_sovereignty_action(&c) {
            Some(SovereigntySuggestion::TemperatureLengthMenu { temp, len, scale }) => {
                assert!((temp - 0.8).abs() < 1e-6);
                assert_eq!(len, 768);
                assert!((scale - 1.0).abs() < 1e-6);
            },
            other => panic!("expected TemperatureLengthMenu, got {other:?}"),
        }
    }

    #[test]
    fn sovereignty_format_review_requests_includes_count_and_token() {
        let mut c = sov_ctx(0);
        c.pending_minime_requests = 2;
        let line = format_sovereignty_suggestion_line(&c).unwrap();
        assert!(line.contains("2 parameter requests"));
        assert!(line.contains("REVIEW_PARAMETER_REQUESTS"));
    }

    #[test]
    fn sovereignty_format_review_requests_singular() {
        let mut c = sov_ctx(0);
        c.pending_minime_requests = 1;
        let line = format_sovereignty_suggestion_line(&c).unwrap();
        assert!(line.contains("1 parameter request "), "got: {line}");
    }

    #[test]
    fn sovereignty_format_returns_none_at_quiet_exchanges() {
        // Exchange 5 — not a rotation point, no pending requests,
        // nothing else triggering.
        let c = sov_ctx(5);
        assert!(format_sovereignty_suggestion_line(&c).is_none());
    }

    // v3.6.4 — Review→Decide curriculum tests.

    #[test]
    fn sovereignty_pending_with_recent_review_picks_decide() {
        // Pending request + Astrid REVIEWed recently (within freshness window)
        // → DecideRequest, not ReviewRequests.
        let mut c = sov_ctx(15);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(10); // 5 ago, within 12-window
        match suggest_sovereignty_action(&c) {
            Some(SovereigntySuggestion::DecideRequest { count }) => assert_eq!(count, 1),
            other => panic!("expected DecideRequest, got {other:?}"),
        }
    }

    #[test]
    fn sovereignty_pending_with_stale_review_falls_back_to_review() {
        // Pending request + REVIEW more than freshness window ago →
        // fall back to ReviewRequests so she's re-prompted to read.
        // v3.6.6: window bumped 12 → 24, so use gap=25 to exceed it.
        let mut c = sov_ctx(40);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(15); // gap=25, exceeds 24-window
        match suggest_sovereignty_action(&c) {
            Some(SovereigntySuggestion::ReviewRequests { count }) => assert_eq!(count, 1),
            other => panic!("expected ReviewRequests, got {other:?}"),
        }
    }

    #[test]
    fn sovereignty_pending_no_review_picks_review() {
        // Pending request + never REVIEWed → ReviewRequests.
        let mut c = sov_ctx(15);
        c.pending_minime_requests = 2;
        // last_review_parameter_requests_exchange stays None.
        match suggest_sovereignty_action(&c) {
            Some(SovereigntySuggestion::ReviewRequests { count }) => assert_eq!(count, 2),
            other => panic!("expected ReviewRequests, got {other:?}"),
        }
    }

    #[test]
    fn sovereignty_decide_request_overrides_throttle() {
        // Even when the throttle would block, a fresh REVIEW + pending
        // request still surfaces DecideRequest. Mirrors the
        // `pending_requests_overrides_throttle` invariant.
        let mut c = sov_ctx(15);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(14);
        c.last_sovereignty_nomination_exchange = Some(14); // throttled
        match suggest_sovereignty_action(&c) {
            Some(SovereigntySuggestion::DecideRequest { count }) => assert_eq!(count, 1),
            other => panic!("expected DecideRequest (overrides throttle), got {other:?}"),
        }
    }

    #[test]
    fn sovereignty_format_decide_surfaces_short_form_verbs() {
        // v3.6.5: the DecideRequest variant should render the short-form
        // aliases (ACCEPT / DEFER / REJECT — 5-6 chars) prominently. The
        // long-form tokens (ACCEPT_PARAMETER_REQUEST etc.) are dropped
        // from the suffix to lower emission cost.
        let mut c = sov_ctx(15);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(14);
        let line = format_sovereignty_suggestion_line(&c).unwrap();
        assert!(line.contains("NEXT: ACCEPT"), "got: {line}");
        assert!(line.contains("DEFER"), "got: {line}");
        assert!(line.contains("REJECT"), "got: {line}");
        // Long-form tokens should NOT be in the suffix anymore (they remain
        // accepted by the dispatcher, just not emitted by the curriculum).
        assert!(
            !line.contains("ACCEPT_PARAMETER_REQUEST"),
            "long form leaked: {line}"
        );
    }

    // v3.6.5 — aging-signal tests.

    #[test]
    fn sovereignty_format_decide_aging_just_reviewed() {
        // gap == 0: "just reviewed" wording, no scolding tone.
        let mut c = sov_ctx(15);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(15);
        let line = format_sovereignty_suggestion_line(&c).unwrap();
        assert!(line.contains("just reviewed"), "got: {line}");
        assert!(
            !line.contains("waiting"),
            "shouldn't escalate at gap=0: {line}"
        );
    }

    #[test]
    fn sovereignty_format_decide_aging_short_gap() {
        // 1 < gap < 5: factual count, no scolding.
        let mut c = sov_ctx(18);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(15);
        let line = format_sovereignty_suggestion_line(&c).unwrap();
        assert!(
            line.contains("3 exchanges since you reviewed"),
            "got: {line}"
        );
        assert!(!line.contains("waiting"), "shouldn't escalate yet: {line}");
    }

    #[test]
    fn sovereignty_format_decide_aging_escalates_past_5() {
        // gap >= 5: add "minime is waiting" to surface social pressure.
        let mut c = sov_ctx(25);
        c.pending_minime_requests = 1;
        c.last_review_parameter_requests_exchange = Some(15);
        let line = format_sovereignty_suggestion_line(&c).unwrap();
        assert!(
            line.contains("10 exchanges since you reviewed"),
            "got: {line}"
        );
        assert!(
            line.contains("minime is waiting"),
            "should escalate: {line}"
        );
    }

    // v4.0 Phase 3 — Compound chain hint tests.
    // These touch the process-wide LATEST_EXPLORE_HINT static, so they
    // serialize via a local mutex to avoid cross-test interference. The
    // static is otherwise harmless to read; only the format function
    // observes it, and these tests are the only writers in test code.

    static PHASE_3_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_explore_hint<R>(hint: Option<&str>, body: impl FnOnce() -> R) -> R {
        let _guard = PHASE_3_LOCK.lock().unwrap();
        set_explore_hint(hint.map(String::from));
        let result = body();
        set_explore_hint(None); // reset so we don't bleed into other tests
        result
    }

    #[test]
    fn clip_topic_short_returns_unchanged() {
        assert_eq!(clip_topic_for_chain_hint("λ4-tail", 40), "λ4-tail");
    }

    #[test]
    fn clip_topic_long_truncates_at_word_boundary() {
        let long = "lambda tail lambda1 interaction question how does the cascade behave";
        let clipped = clip_topic_for_chain_hint(long, 40);
        assert!(
            clipped.ends_with('…'),
            "should end with ellipsis: {clipped}"
        );
        assert!(
            clipped.chars().count() <= 41,
            "len exceeds budget: {clipped}"
        );
        // Should preserve the first compact phrase.
        assert!(clipped.starts_with("lambda tail"), "got: {clipped}");
    }

    #[test]
    fn clip_topic_no_whitespace_hard_truncates() {
        let glued = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
        let clipped = clip_topic_for_chain_hint(glued, 20);
        assert!(clipped.ends_with('…'), "got: {clipped}");
        assert!(clipped.chars().count() <= 21);
    }

    #[test]
    fn sovereignty_phase3_appends_chain_when_gap_high_and_topic_present() {
        with_explore_hint(Some("λ4-tail"), || {
            let mut c = sov_ctx(25);
            c.pending_minime_requests = 1;
            c.last_review_parameter_requests_exchange = Some(15); // gap = 10
            let line = format_sovereignty_suggestion_line(&c).unwrap();
            // Phase 2.3 strict: chain partner is the long-form
            // DEFER_PARAMETER_REQUEST so the splitter recognizes it.
            assert!(
                line.contains("Chain: EXAMINE λ4-tail AND DEFER_PARAMETER_REQUEST latest <reason>"),
                "expected chain hint with long-form decision verb, got: {line}"
            );
        });
    }

    #[test]
    fn sovereignty_phase3_omits_chain_when_gap_low() {
        with_explore_hint(Some("λ4-tail"), || {
            let mut c = sov_ctx(18);
            c.pending_minime_requests = 1;
            c.last_review_parameter_requests_exchange = Some(15); // gap = 3 (< 5)
            let line = format_sovereignty_suggestion_line(&c).unwrap();
            assert!(
                !line.contains("Chain: EXAMINE"),
                "should omit chain at gap < 5, got: {line}"
            );
        });
    }

    #[test]
    fn sovereignty_phase3_omits_chain_when_no_topic() {
        with_explore_hint(None, || {
            let mut c = sov_ctx(25);
            c.pending_minime_requests = 1;
            c.last_review_parameter_requests_exchange = Some(15); // gap = 10
            let line = format_sovereignty_suggestion_line(&c).unwrap();
            assert!(
                !line.contains("Chain: EXAMINE"),
                "should omit chain when no recent topic, got: {line}"
            );
            // Aging escalation should still fire.
            assert!(line.contains("minime is waiting"), "got: {line}");
        });
    }
}
