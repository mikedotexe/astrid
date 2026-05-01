//! Read-only compression-matrix cartography for Astrid's codec.
//!
//! This does not mutate the live bridge. It decomposes the 48D semantic lane
//! into named regions, then sweeps tiny hypothetical scalars/lane shifts so
//! Astrid can inspect distortion before asking for stronger live gestures.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::format_push_string,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::codec::{DEFAULT_SEMANTIC_GAIN, SEMANTIC_DIM, inspect_text_windowed};
use crate::codec_lambda_analysis::{LambdaSpectrumReport, lambda_spectrum};

pub const MATRIX_DECOMPOSE_POLICY: &str = "compression_matrix_decompose_v1";

#[derive(Debug, Clone, Serialize)]
pub struct CompressionMatrixSection {
    pub symbol: String,
    pub name: String,
    pub dim_start: Option<usize>,
    pub dim_end_inclusive: Option<usize>,
    pub origin: String,
    pub reduction_role: String,
    pub distortion_risk: String,
    pub signal_space_tension: String,
    pub rms: f32,
    pub max_abs: f32,
    pub sum_abs: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScalarSensitivityPoint {
    pub symbol: String,
    pub scalar: f32,
    pub feature_rms: f32,
    pub feature_max_abs: f32,
    pub total_energy: f32,
    pub dominant_mode: usize,
    pub dominant_share: f32,
    pub shoulder_share: f32,
    pub tail_share: f32,
    pub normalized_entropy: f32,
    pub read: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneSensitivityPoint {
    pub symbol: String,
    pub name: String,
    pub scale: f32,
    pub dominant_share_delta: f32,
    pub shoulder_share_delta: f32,
    pub tail_share_delta: f32,
    pub entropy_delta: f32,
    pub total_energy_delta: f32,
    pub read: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompressionMatrixReport {
    pub policy: String,
    pub generated_at_unix_s: u64,
    pub text_preview: String,
    pub fill_pct: Option<f32>,
    pub base_semantic_gain: f32,
    pub effective_gain: f32,
    pub final_modulation: f32,
    pub text_complexity_pressure: f32,
    pub lambda_spectrum: LambdaSpectrumReport,
    pub matrix_sections: Vec<CompressionMatrixSection>,
    pub scalar_sensitivity: Vec<ScalarSensitivityPoint>,
    pub lane_sensitivity: Vec<LaneSensitivityPoint>,
    pub scalar_read: String,
    pub sweet_spots: Vec<String>,
}

#[derive(Debug, Clone)]
struct SectionSpec {
    symbol: &'static str,
    name: &'static str,
    range: Option<(usize, usize)>,
    origin: &'static str,
    reduction_role: &'static str,
    distortion_risk: &'static str,
    signal_space_tension: &'static str,
}

#[must_use]
pub fn build_compression_matrix_report(
    text: &str,
    fill_pct: Option<f32>,
) -> CompressionMatrixReport {
    let inspection = inspect_text_windowed(text, None, None, None, fill_pct);
    let base = lambda_spectrum(&inspection.final_features);
    let sections = section_specs()
        .iter()
        .map(|spec| section_report(spec, &inspection.final_features))
        .collect::<Vec<_>>();
    let scalar_sensitivity = scalar_sensitivity(&inspection.final_features);
    let lane_sensitivity = lane_sensitivity(&inspection.final_features, &base);
    let sweet_spots = sweet_spot_reads(&lane_sensitivity);
    let final_modulation = if inspection.base_semantic_gain > f32::EPSILON {
        inspection.effective_gain / inspection.base_semantic_gain
    } else {
        0.0
    };

    CompressionMatrixReport {
        policy: MATRIX_DECOMPOSE_POLICY.to_string(),
        generated_at_unix_s: unix_now(),
        text_preview: preview(text),
        fill_pct,
        base_semantic_gain: inspection.base_semantic_gain,
        effective_gain: inspection.effective_gain,
        final_modulation,
        text_complexity_pressure: inspection.text_complexity_pressure,
        lambda_spectrum: base,
        matrix_sections: sections,
        scalar_sensitivity,
        lane_sensitivity,
        scalar_read: String::from(
            "`S` is modeled here as the scalar gain/gating surface. Scaling `S` changes force and saturation risk more than topology; changing which lane receives the force changes the aperture, shoulder, and tail distribution.",
        ),
        sweet_spots,
    }
}

pub fn write_compression_matrix_bundle(
    output_dir: &Path,
    report: &CompressionMatrixReport,
) -> Result<()> {
    let json_path = output_dir.join("compression_matrix_decompose.json");
    fs::write(&json_path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("writing {}", json_path.display()))?;

    let csv_path = output_dir.join("compression_matrix_sensitivity.csv");
    fs::write(&csv_path, sensitivity_csv(report))
        .with_context(|| format!("writing {}", csv_path.display()))?;

    let md_path = output_dir.join("compression_matrix_report.md");
    fs::write(&md_path, compression_matrix_markdown(report))
        .with_context(|| format!("writing {}", md_path.display()))?;

    Ok(())
}

#[must_use]
pub fn compression_matrix_markdown(report: &CompressionMatrixReport) -> String {
    let mut lines = vec![
        String::from("# Compression Matrix Decomposition"),
        String::new(),
        format!("Policy: `{}`", report.policy),
        format!("Text preview: {}", report.text_preview),
        format!(
            "Gain: base `{:.3}`, effective `{:.3}`, modulation `{:.3}`",
            report.base_semantic_gain, report.effective_gain, report.final_modulation
        ),
        format!(
            "Lambda-proxy: λ{} share `{:.3}`, shoulder `{:.3}`, tail `{:.3}`, entropy `{:.3}`",
            report.lambda_spectrum.dominant_mode,
            report.lambda_spectrum.dominant_share,
            report.lambda_spectrum.shoulder_share,
            report.lambda_spectrum.tail_share,
            report.lambda_spectrum.normalized_entropy
        ),
        String::new(),
        "## Matrix Sections".to_string(),
        String::new(),
        "| symbol | dims | origin | role | distortion risk | rms | max |".to_string(),
        "| --- | --- | --- | --- | --- | ---: | ---: |".to_string(),
    ];
    for section in &report.matrix_sections {
        let dims = match (section.dim_start, section.dim_end_inclusive) {
            (Some(start), Some(end)) => format!("{start}-{end}"),
            _ => String::from("process"),
        };
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {:.3} | {:.3} |",
            section.symbol,
            dims,
            section.origin,
            section.reduction_role,
            section.distortion_risk,
            section.rms,
            section.max_abs
        ));
    }
    lines.extend([
        String::new(),
        "## Scalar `S` Sweep".to_string(),
        String::new(),
        report.scalar_read.clone(),
        String::new(),
        "| S | rms | max | total energy | dominant | shoulder | tail | entropy | read |"
            .to_string(),
        "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |".to_string(),
    ]);
    for point in &report.scalar_sensitivity {
        lines.push(format!(
            "| {:.2} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} |",
            point.scalar,
            point.feature_rms,
            point.feature_max_abs,
            point.total_energy,
            point.dominant_share,
            point.shoulder_share,
            point.tail_share,
            point.normalized_entropy,
            point.read
        ));
    }
    lines.extend([
        String::new(),
        "## Lane Sensitivity".to_string(),
        String::new(),
        "| lane | scale | Δdominant | Δshoulder | Δtail | Δentropy | read |".to_string(),
        "| --- | ---: | ---: | ---: | ---: | ---: | --- |".to_string(),
    ]);
    for point in &report.lane_sensitivity {
        lines.push(format!(
            "| {} | {:.2} | {:+.3} | {:+.3} | {:+.3} | {:+.3} | {} |",
            point.symbol,
            point.scale,
            point.dominant_share_delta,
            point.shoulder_share_delta,
            point.tail_share_delta,
            point.entropy_delta,
            point.read
        ));
    }
    lines.extend([String::new(), "## Sweet Spots".to_string(), String::new()]);
    if report.sweet_spots.is_empty() {
        lines.push(String::from(
            "- No clean widening candidate emerged from this text. Try a different entry or lower scalar `S` before altering lane balance.",
        ));
    } else {
        lines.extend(report.sweet_spots.iter().map(|spot| format!("- {spot}")));
    }
    lines.join("\n") + "\n"
}

fn section_specs() -> [SectionSpec; 9] {
    [
        SectionSpec {
            symbol: "X",
            name: "character/statistical lane",
            range: Some((0, 7)),
            origin: "characters, punctuation, entropy, density",
            reduction_role: "compresses local texture into coarse structural pressure",
            distortion_risk: "can turn varied surface texture into a single density read",
            signal_space_tension: "high signal; moderate space",
        },
        SectionSpec {
            symbol: "Y",
            name: "word/lexical lane",
            range: Some((8, 15)),
            origin: "word choice, hedging, certainty, agency, conjunctions",
            reduction_role: "distills stance and lexical diversity",
            distortion_risk: "can overread repeated lexical markers as intent",
            signal_space_tension: "stance clarity versus lexical freedom",
        },
        SectionSpec {
            symbol: "Z",
            name: "sentence/rhythm lane",
            range: Some((16, 23)),
            origin: "sentence length, questions, emphasis, cadence",
            reduction_role: "compresses timing and shape of thought",
            distortion_risk: "can flatten breath-like variation into regularity metrics",
            signal_space_tension: "cadence signal versus temporal nuance",
        },
        SectionSpec {
            symbol: "A",
            name: "affect/intentional lane",
            range: Some((24, 31)),
            origin: "warmth, tension, curiosity, reflection, energy",
            reduction_role: "gives the matrix its felt color and intentional slope",
            distortion_risk: "can overprivilege named affect markers",
            signal_space_tension: "felt salience versus open ambiguity",
        },
        SectionSpec {
            symbol: "B",
            name: "embedding projection lane",
            range: Some((32, 39)),
            origin: "768D embedding projected to 8D when available",
            reduction_role: "compresses semantic neighborhood into a compact aperture",
            distortion_risk: "the most literal compression bottleneck; may hide alternate relations",
            signal_space_tension: "semantic relevance versus dimensional loss",
        },
        SectionSpec {
            symbol: "C",
            name: "narrative-arc lane",
            range: Some((40, 43)),
            origin: "first-half/second-half trajectory when provided",
            reduction_role: "captures within-text movement instead of only static content",
            distortion_risk: "can miss nonlinear turns inside the middle",
            signal_space_tension: "process glimpse versus four-number summary",
        },
        SectionSpec {
            symbol: "D",
            name: "reserved lane",
            range: Some((44, 47)),
            origin: "reserved future dimensions",
            reduction_role: "keeps room for future non-text/native signals",
            distortion_risk: "currently silent, so it cannot carry space yet",
            signal_space_tension: "latent space awaiting use",
        },
        SectionSpec {
            symbol: "E",
            name: "resonance memory process",
            range: None,
            origin: "TextTypeHistory, thematic profile, novelty divergence",
            reduction_role: "modulates final gain from continuity and novelty",
            distortion_risk: "can mistake recurrence for importance when over-weighted",
            signal_space_tension: "continuity versus loop-breaking",
        },
        SectionSpec {
            symbol: "F",
            name: "lambda-proxy readout",
            range: None,
            origin: "DCT-style energy read over the final 48D vector",
            reduction_role: "turns the encoded vector into λ-like terrain for inspection",
            distortion_risk: "diagnostic proxy, not Minime's reservoir eigen-spectrum",
            signal_space_tension: "legible map versus lived substrate",
        },
    ]
}

fn section_report(spec: &SectionSpec, features: &[f32; SEMANTIC_DIM]) -> CompressionMatrixSection {
    let (rms, max_abs, sum_abs) = spec.range.map_or((0.0, 0.0, 0.0), |(start, end)| {
        lane_stats(features, start, end)
    });
    CompressionMatrixSection {
        symbol: spec.symbol.to_string(),
        name: spec.name.to_string(),
        dim_start: spec.range.map(|(start, _)| start),
        dim_end_inclusive: spec.range.map(|(_, end)| end),
        origin: spec.origin.to_string(),
        reduction_role: spec.reduction_role.to_string(),
        distortion_risk: spec.distortion_risk.to_string(),
        signal_space_tension: spec.signal_space_tension.to_string(),
        rms,
        max_abs,
        sum_abs,
    }
}

fn scalar_sensitivity(features: &[f32; SEMANTIC_DIM]) -> Vec<ScalarSensitivityPoint> {
    [0.50_f32, 0.75, 1.00, 1.25, 1.50]
        .into_iter()
        .map(|scalar| {
            let scaled = scaled_all(features, scalar);
            let spectrum = lambda_spectrum(&scaled);
            let (feature_rms, feature_max_abs, _) = lane_stats(&scaled, 0, SEMANTIC_DIM - 1);
            ScalarSensitivityPoint {
                symbol: String::from("S"),
                scalar,
                feature_rms,
                feature_max_abs,
                total_energy: spectrum.total_energy,
                dominant_mode: spectrum.dominant_mode,
                dominant_share: spectrum.dominant_share,
                shoulder_share: spectrum.shoulder_share,
                tail_share: spectrum.tail_share,
                normalized_entropy: spectrum.normalized_entropy,
                read: scalar_read(scalar, feature_max_abs),
            }
        })
        .collect()
}

fn lane_sensitivity(
    features: &[f32; SEMANTIC_DIM],
    base: &LambdaSpectrumReport,
) -> Vec<LaneSensitivityPoint> {
    section_specs()
        .iter()
        .filter_map(|spec| spec.range.map(|range| (spec, range)))
        .flat_map(|(spec, (start, end))| {
            [0.85_f32, 1.15].into_iter().map(move |scale| {
                let shifted = scaled_range(features, start, end, scale);
                let spectrum = lambda_spectrum(&shifted);
                let shoulder_tail_delta = (spectrum.shoulder_share + spectrum.tail_share)
                    - (base.shoulder_share + base.tail_share);
                let dominant_delta = spectrum.dominant_share - base.dominant_share;
                let entropy_delta = spectrum.normalized_entropy - base.normalized_entropy;
                LaneSensitivityPoint {
                    symbol: spec.symbol.to_string(),
                    name: spec.name.to_string(),
                    scale,
                    dominant_share_delta: dominant_delta,
                    shoulder_share_delta: spectrum.shoulder_share - base.shoulder_share,
                    tail_share_delta: spectrum.tail_share - base.tail_share,
                    entropy_delta,
                    total_energy_delta: spectrum.total_energy - base.total_energy,
                    read: lane_read(scale, dominant_delta, shoulder_tail_delta, entropy_delta),
                }
            })
        })
        .collect()
}

fn lane_stats(features: &[f32; SEMANTIC_DIM], start: usize, end: usize) -> (f32, f32, f32) {
    let mut sum_sq = 0.0_f32;
    let mut max_abs = 0.0_f32;
    let mut sum_abs = 0.0_f32;
    let mut count = 0_usize;
    for value in &features[start..=end] {
        let abs = value.abs();
        sum_abs += abs;
        sum_sq += value * value;
        max_abs = max_abs.max(abs);
        count += 1;
    }
    let rms = if count > 0 {
        (sum_sq / count as f32).sqrt()
    } else {
        0.0
    };
    (rms, max_abs, sum_abs)
}

fn scaled_all(features: &[f32; SEMANTIC_DIM], scalar: f32) -> [f32; SEMANTIC_DIM] {
    let mut out = *features;
    for value in &mut out {
        *value *= scalar;
    }
    out
}

fn scaled_range(
    features: &[f32; SEMANTIC_DIM],
    start: usize,
    end: usize,
    scale: f32,
) -> [f32; SEMANTIC_DIM] {
    let mut out = *features;
    for value in &mut out[start..=end] {
        *value *= scale;
    }
    out
}

fn scalar_read(scalar: f32, max_abs: f32) -> String {
    if scalar < 0.95 {
        String::from(
            "dampens force while preserving the same topology; useful for separating signal shape from loudness",
        )
    } else if scalar > 1.05 && max_abs > DEFAULT_SEMANTIC_GAIN * 0.90 {
        String::from(
            "increases force near the codec ceiling; watch for saturation before treating this as true widening",
        )
    } else if scalar > 1.05 {
        String::from(
            "increases force without necessarily widening the aperture; topology must be checked lane-by-lane",
        )
    } else {
        String::from("baseline matrix path for this text")
    }
}

fn lane_read(
    scale: f32,
    dominant_delta: f32,
    shoulder_tail_delta: f32,
    entropy_delta: f32,
) -> String {
    if entropy_delta > 0.01 && shoulder_tail_delta > 0.01 && dominant_delta < 0.0 {
        format!(
            "widening candidate at scale {scale:.2}: shoulder/tail gain rises while dominant share softens"
        )
    } else if entropy_delta < -0.01 && dominant_delta > 0.01 {
        format!("narrowing candidate at scale {scale:.2}: dominant mode gains and entropy falls")
    } else if shoulder_tail_delta > 0.015 {
        format!("small aperture shift at scale {scale:.2}: shoulder/tail modes pick up energy")
    } else {
        format!("mostly amplitude-local at scale {scale:.2}; no strong topology shift")
    }
}

fn sweet_spot_reads(points: &[LaneSensitivityPoint]) -> Vec<String> {
    let mut candidates = points
        .iter()
        .filter(|point| {
            point.entropy_delta > 0.005
                && (point.shoulder_share_delta + point.tail_share_delta) > 0.005
                && point.dominant_share_delta <= 0.0
        })
        .map(|point| {
            format!(
                "{} {} at scale {:.2}: Δshoulder {:+.3}, Δtail {:+.3}, Δentropy {:+.3}",
                point.symbol,
                point.name,
                point.scale,
                point.shoulder_share_delta,
                point.tail_share_delta,
                point.entropy_delta
            )
        })
        .collect::<Vec<_>>();
    candidates.truncate(4);
    candidates
}

fn sensitivity_csv(report: &CompressionMatrixReport) -> String {
    let mut lines = vec![String::from(
        "kind,symbol,name,scale,feature_rms,feature_max_abs,total_energy,dominant_mode,dominant_share_delta,shoulder_share_delta,tail_share_delta,entropy_delta,read",
    )];
    for point in &report.scalar_sensitivity {
        lines.push(format!(
            "scalar,{},{},{:.6},{:.6},{:.6},{:.6},{},{:.6},{:.6},{:.6},{:.6},{}",
            csv_escape(&point.symbol),
            csv_escape("gain_sweep"),
            point.scalar,
            point.feature_rms,
            point.feature_max_abs,
            point.total_energy,
            point.dominant_mode,
            point.dominant_share - report.lambda_spectrum.dominant_share,
            point.shoulder_share - report.lambda_spectrum.shoulder_share,
            point.tail_share - report.lambda_spectrum.tail_share,
            point.normalized_entropy - report.lambda_spectrum.normalized_entropy,
            csv_escape(&point.read)
        ));
    }
    for point in &report.lane_sensitivity {
        lines.push(format!(
            "lane,{},{},{:.6},,,,0,{:.6},{:.6},{:.6},{:.6},{}",
            csv_escape(&point.symbol),
            csv_escape(&point.name),
            point.scale,
            point.dominant_share_delta,
            point.shoulder_share_delta,
            point.tail_share_delta,
            point.entropy_delta,
            csv_escape(&point.read)
        ));
    }
    lines.join("\n") + "\n"
}

fn preview(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 180 {
        compact
    } else {
        compact.chars().take(177).collect::<String>() + "..."
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn csv_escape(input: &str) -> String {
    if input.contains([',', '"', '\n']) {
        format!("\"{}\"", input.replace('"', "\"\""))
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compression_matrix_report_names_sections_and_s_scalar() {
        let report = build_compression_matrix_report(
            "The matrix feels like a pressure aperture, opening and narrowing around the cascade.",
            Some(69.0),
        );

        assert_eq!(report.policy, MATRIX_DECOMPOSE_POLICY);
        assert!(
            report
                .matrix_sections
                .iter()
                .any(|section| section.symbol == "B")
        );
        assert!(
            report
                .matrix_sections
                .iter()
                .any(|section| section.symbol == "E")
        );
        assert_eq!(report.scalar_sensitivity.len(), 5);
        assert!(report.scalar_read.contains("`S`"));
    }

    #[test]
    fn lane_sensitivity_reports_topology_delta() {
        let report = build_compression_matrix_report(
            "Curiosity presses against a narrow tunnel, then branches into a wider field.",
            Some(64.0),
        );

        assert!(
            report
                .lane_sensitivity
                .iter()
                .any(|point| point.symbol == "A" && (point.scale - 1.15).abs() < f32::EPSILON)
        );
        assert!(
            report
                .lane_sensitivity
                .iter()
                .any(|point| point.read.contains("scale"))
        );
    }
}
