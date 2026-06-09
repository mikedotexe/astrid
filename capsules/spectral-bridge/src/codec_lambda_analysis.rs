#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::format_push_string
)]

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::codec::SEMANTIC_DIM;
use crate::codec_explorer::CodecExplorerSummary;
use crate::codec_time_domain::{TextTimeDomainProfile, text_time_domain_profile};

pub const LAMBDA_PROXY_BINS: usize = 8;

#[derive(Debug, Clone, Serialize)]
pub struct LambdaSpectrumReport {
    pub basis: String,
    pub bins: [f32; LAMBDA_PROXY_BINS],
    pub dominant_mode: usize,
    pub dominant_share: f32,
    pub shoulder_share: f32,
    pub tail_share: f32,
    pub normalized_entropy: f32,
    pub total_energy: f32,
}

pub type TimeDomainFeatureReport = TextTimeDomainProfile;

#[derive(Debug, Clone, Serialize)]
struct LambdaGradientRow {
    index: usize,
    from_label: String,
    to_label: String,
    dominant_delta: f32,
    shoulder_delta: f32,
    tail_delta: f32,
    entropy_delta: f32,
    gradient_magnitude: f32,
    constriction_index: f32,
    expansion_index: f32,
    movement: String,
}

#[must_use]
pub fn lambda_spectrum(features: &[f32; SEMANTIC_DIM]) -> LambdaSpectrumReport {
    let len = features.len().max(1) as f32;
    let mut energy = [0.0_f32; LAMBDA_PROXY_BINS];
    for (mode, slot) in energy.iter_mut().enumerate() {
        let mode_f = mode as f32;
        let mut coeff = 0.0_f32;
        for (index, value) in features.iter().enumerate() {
            let phase = std::f32::consts::PI * (index as f32 + 0.5) * mode_f / len;
            coeff += *value * phase.cos();
        }
        *slot = coeff.mul_add(coeff, 0.0) / len.max(f32::EPSILON);
    }

    let total_energy = energy.iter().copied().sum::<f32>();
    let mut bins = [0.0_f32; LAMBDA_PROXY_BINS];
    if total_energy > f32::EPSILON {
        for (index, value) in energy.iter().enumerate() {
            bins[index] = (*value / total_energy).clamp(0.0, 1.0);
        }
    }
    let dominant_index = bins
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(0, |(index, _)| index);
    let shoulder_share = bins
        .iter()
        .enumerate()
        .filter(|(index, _)| (1..=2).contains(index))
        .map(|(_, value)| *value)
        .sum::<f32>();
    let tail_share = bins
        .iter()
        .enumerate()
        .filter(|(index, _)| *index >= 3)
        .map(|(_, value)| *value)
        .sum::<f32>();
    let entropy = bins
        .iter()
        .filter(|value| **value > f32::EPSILON)
        .map(|value| -value * value.ln())
        .sum::<f32>();
    let normalized_entropy = (entropy / (LAMBDA_PROXY_BINS as f32).ln()).clamp(0.0, 1.0);

    LambdaSpectrumReport {
        basis: String::from("dct_energy_over_48d_semantic_features"),
        bins,
        dominant_mode: dominant_index + 1,
        dominant_share: bins[dominant_index],
        shoulder_share,
        tail_share,
        normalized_entropy,
        total_energy,
    }
}

#[must_use]
pub fn time_domain_features(text: &str) -> TimeDomainFeatureReport {
    text_time_domain_profile(text)
}

pub fn write_lambda_analysis_bundle(summary: &CodecExplorerSummary) -> Result<()> {
    write_lambda_spectrum_csv(summary)?;
    write_lambda_spectrum_svg(&summary.output_dir, summary)?;
    write_lambda_gradient_csv(summary)?;
    write_lambda_gradient_svg(&summary.output_dir, summary)?;
    write_time_domain_csv(summary)?;
    Ok(())
}

fn write_lambda_spectrum_csv(summary: &CodecExplorerSummary) -> Result<()> {
    let mut header = vec![
        String::from("label"),
        String::from("path"),
        String::from("dominant_mode"),
        String::from("dominant_share"),
        String::from("shoulder_share"),
        String::from("tail_share"),
        String::from("normalized_entropy"),
        String::from("total_energy"),
    ];
    for mode in 1..=LAMBDA_PROXY_BINS {
        header.push(format!("lambda_proxy_{mode}"));
    }
    let mut lines = vec![header.join(",")];
    for entry in &summary.entries {
        let mut row = vec![
            csv_escape(entry.label.clone()),
            csv_escape(
                entry
                    .path
                    .as_ref()
                    .map_or(String::new(), |path| path.display().to_string()),
            ),
            entry.lambda_spectrum.dominant_mode.to_string(),
            format!("{:.6}", entry.lambda_spectrum.dominant_share),
            format!("{:.6}", entry.lambda_spectrum.shoulder_share),
            format!("{:.6}", entry.lambda_spectrum.tail_share),
            format!("{:.6}", entry.lambda_spectrum.normalized_entropy),
            format!("{:.6}", entry.lambda_spectrum.total_energy),
        ];
        for value in entry.lambda_spectrum.bins {
            row.push(format!("{value:.6}"));
        }
        lines.push(row.join(","));
    }
    let path = summary.output_dir.join("lambda_spectrum.csv");
    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn write_time_domain_csv(summary: &CodecExplorerSummary) -> Result<()> {
    let mut lines = vec![String::from(
        "label,path,char_count,word_count,sentence_count,avg_word_len,punctuation_rate,question_rate,exclamation_rate,uppercase_rate,digit_rate,line_break_rate,rhythm_alternation_rate,repetition_rate,sentence_length_cv,cadence_burstiness,regularity_score,temporal_complexity,cadence_classification",
    )];
    for entry in &summary.entries {
        let time = &entry.time_domain;
        lines.push(format!(
            "{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}",
            csv_escape(entry.label.clone()),
            csv_escape(
                entry
                    .path
                    .as_ref()
                    .map_or(String::new(), |path| path.display().to_string()),
            ),
            time.char_count,
            time.word_count,
            time.sentence_count,
            time.avg_word_len,
            time.punctuation_rate,
            time.question_rate,
            time.exclamation_rate,
            time.uppercase_rate,
            time.digit_rate,
            time.line_break_rate,
            time.rhythm_alternation_rate,
            time.repetition_rate,
            time.sentence_length_cv,
            time.cadence_burstiness,
            time.regularity_score,
            time.temporal_complexity,
            csv_escape(time.cadence_classification.clone()),
        ));
    }
    let path = summary.output_dir.join("time_domain_features.csv");
    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn write_lambda_gradient_csv(summary: &CodecExplorerSummary) -> Result<()> {
    let rows = lambda_gradient_rows(summary);
    let mut lines = vec![String::from(
        "index,from_label,to_label,dominant_delta,shoulder_delta,tail_delta,entropy_delta,gradient_magnitude,constriction_index,expansion_index,movement",
    )];
    for row in rows {
        lines.push(format!(
            "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}",
            row.index,
            csv_escape(row.from_label),
            csv_escape(row.to_label),
            row.dominant_delta,
            row.shoulder_delta,
            row.tail_delta,
            row.entropy_delta,
            row.gradient_magnitude,
            row.constriction_index,
            row.expansion_index,
            csv_escape(row.movement),
        ));
    }
    let path = summary.output_dir.join("lambda_gradient.csv");
    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn write_lambda_spectrum_svg(output_dir: &Path, summary: &CodecExplorerSummary) -> Result<()> {
    let path = output_dir.join("lambda_spectrum.svg");
    let width = 960.0_f32;
    let height = (170.0 + summary.entries.len() as f32 * 34.0).max(300.0);
    let left = 190.0_f32;
    let top = 72.0_f32;
    let cell_w = 62.0_f32;
    let cell_h = 24.0_f32;
    let side_x = left + cell_w * LAMBDA_PROXY_BINS as f32 + 26.0;
    let mut svg = String::new();
    svg.push_str(&svg_header(width, height));
    svg.push_str(
        r#"<text x="24" y="28" font-size="20" font-family="monospace">Astrid Codec Explorer: lambda-proxy energy spectrum</text>"#,
    );
    svg.push_str(
        r##"<text x="24" y="50" font-size="11" font-family="monospace" fill="#4b5563">DCT energy over the live 48D semantic vector. This is an offline codec proxy, not Minime's reservoir eigenvalue spectrum.</text>"##,
    );

    for mode in 0..LAMBDA_PROXY_BINS {
        let x = left + mode as f32 * cell_w;
        svg.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" font-size="11" font-family="monospace">λ{}</text>"#,
            x + 18.0,
            top - 12.0,
            mode + 1
        ));
    }
    svg.push_str(&format!(
        r#"<text x="{side_x:.1}" y="{:.1}" font-size="11" font-family="monospace">dominant / entropy</text>"#,
        top - 12.0
    ));

    for (row, entry) in summary.entries.iter().enumerate() {
        let y = top + row as f32 * cell_h;
        svg.push_str(&format!(
            r#"<text x="18" y="{:.1}" font-size="10" font-family="monospace">{}</text>"#,
            y + 16.0,
            xml_escape(&entry.label)
        ));
        for (mode, value) in entry.lambda_spectrum.bins.iter().enumerate() {
            let x = left + mode as f32 * cell_w;
            svg.push_str(&format!(
                r##"<rect x="{x:.1}" y="{y:.1}" width="{:.1}" height="{:.1}" fill="{}" stroke="#111827" stroke-opacity="0.08"/>"##,
                cell_w - 2.0,
                cell_h - 2.0,
                sequential_color(*value),
            ));
            svg.push_str(&format!(
                r##"<text x="{:.1}" y="{:.1}" font-size="9" font-family="monospace" fill="#111827">{:.2}</text>"##,
                x + 14.0,
                y + 15.0,
                value
            ));
        }
        svg.push_str(&format!(
            r#"<text x="{side_x:.1}" y="{:.1}" font-size="10" font-family="monospace">λ{} {:.2} | H {:.2}</text>"#,
            y + 15.0,
            entry.lambda_spectrum.dominant_mode,
            entry.lambda_spectrum.dominant_share,
            entry.lambda_spectrum.normalized_entropy
        ));
    }
    let legend_y = height - 36.0;
    for step in 0..=8 {
        let value = step as f32 / 8.0;
        let x = left + step as f32 * 38.0;
        svg.push_str(&format!(
            r#"<rect x="{x:.1}" y="{legend_y:.1}" width="34" height="12" fill="{}"/>"#,
            sequential_color(value)
        ));
    }
    svg.push_str(&format!(
        r##"<text x="{:.1}" y="{:.1}" font-size="10" font-family="monospace" fill="#4b5563">low share → high share</text>"##,
        left + 330.0,
        legend_y + 11.0
    ));
    svg.push_str("</svg>\n");
    fs::write(&path, svg).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn write_lambda_gradient_svg(output_dir: &Path, summary: &CodecExplorerSummary) -> Result<()> {
    let path = output_dir.join("lambda_gradient.svg");
    let width = 960.0_f32;
    let height = 360.0_f32;
    let left = 82.0_f32;
    let top = 50.0_f32;
    let plot_w = width - left - 44.0;
    let plot_h = 230.0_f32;
    let bottom_y = top + plot_h;
    let entries = &summary.entries;
    let step = if entries.len() > 1 {
        plot_w / entries.len().saturating_sub(1).max(1) as f32
    } else {
        plot_w
    };
    let series = [
        ("dominant λ share", "#dc2626"),
        ("entropy", "#2563eb"),
        ("constriction", "#7c3aed"),
    ];
    let mut svg = String::new();
    svg.push_str(&svg_header(width, height));
    svg.push_str(
        r#"<text x="24" y="28" font-size="20" font-family="monospace">Astrid Codec Explorer: lambda gradient and fabric boundary</text>"#,
    );
    svg.push_str(
        r##"<text x="24" y="48" font-size="11" font-family="monospace" fill="#4b5563">Consecutive-entry movement: does the codec concentrate, widen, or hold the semantic fabric?</text>"##,
    );
    for tick in 0..=4 {
        let frac = tick as f32 / 4.0;
        let y = bottom_y - plot_h * frac;
        svg.push_str(&format!(
            r##"<line x1="{left}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="#e5e7eb" stroke-width="1"/>"##,
            left + plot_w
        ));
        svg.push_str(&format!(
            r#"<text x="24" y="{:.1}" font-size="10" font-family="monospace">{frac:.2}</text>"#,
            y + 3.0
        ));
    }
    svg.push_str(&format!(
        r##"<rect x="{left}" y="{top}" width="{plot_w:.1}" height="{plot_h:.1}" fill="none" stroke="#d1d5db"/>"##
    ));

    for (label, color) in series {
        let points = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let value = match label {
                    "dominant λ share" => entry.lambda_spectrum.dominant_share,
                    "entropy" => entry.lambda_spectrum.normalized_entropy,
                    _ => constriction_index(&entry.lambda_spectrum),
                }
                .clamp(0.0, 1.0);
                let x = left + step * index as f32;
                let y = bottom_y - plot_h * value;
                format!("{x:.1},{y:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        svg.push_str(&format!(
            r#"<polyline fill="none" stroke="{color}" stroke-width="3" points="{points}"/>"#
        ));
    }

    for (index, entry) in entries.iter().enumerate() {
        let x = left + step * index as f32;
        let y = bottom_y - plot_h * entry.lambda_spectrum.dominant_share.clamp(0.0, 1.0);
        svg.push_str(&format!(
            r##"<circle cx="{x:.1}" cy="{y:.1}" r="3.5" fill="#111827"/>"##
        ));
        svg.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" font-size="9" transform="rotate(20 {:.1} {:.1})" font-family="monospace">{}</text>"#,
            x - 8.0,
            height - 28.0,
            x - 8.0,
            height - 28.0,
            xml_escape(&entry.label)
        ));
    }

    for (index, (label, color)) in series.iter().enumerate() {
        let x = left + index as f32 * 190.0;
        svg.push_str(&format!(
            r#"<rect x="{x:.1}" y="{:.1}" width="14" height="14" fill="{color}"/><text x="{:.1}" y="{:.1}" font-size="11" font-family="monospace">{label}</text>"#,
            height - 62.0,
            x + 20.0,
            height - 50.0,
        ));
    }
    svg.push_str("</svg>\n");
    fs::write(&path, svg).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn lambda_gradient_rows(summary: &CodecExplorerSummary) -> Vec<LambdaGradientRow> {
    summary
        .entries
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            let previous = &pair[0];
            let current = &pair[1];
            let dominant_delta =
                current.lambda_spectrum.dominant_share - previous.lambda_spectrum.dominant_share;
            let shoulder_delta =
                current.lambda_spectrum.shoulder_share - previous.lambda_spectrum.shoulder_share;
            let tail_delta =
                current.lambda_spectrum.tail_share - previous.lambda_spectrum.tail_share;
            let entropy_delta = current.lambda_spectrum.normalized_entropy
                - previous.lambda_spectrum.normalized_entropy;
            let gradient_magnitude = current
                .lambda_spectrum
                .bins
                .iter()
                .zip(previous.lambda_spectrum.bins.iter())
                .map(|(right, left)| (right - left).abs())
                .sum::<f32>();
            LambdaGradientRow {
                index,
                from_label: previous.label.clone(),
                to_label: current.label.clone(),
                dominant_delta,
                shoulder_delta,
                tail_delta,
                entropy_delta,
                gradient_magnitude,
                constriction_index: constriction_index(&current.lambda_spectrum),
                expansion_index: expansion_index(&current.lambda_spectrum),
                movement: classify_gradient(dominant_delta, entropy_delta).to_string(),
            }
        })
        .collect()
}

fn constriction_index(spectrum: &LambdaSpectrumReport) -> f32 {
    (spectrum.dominant_share * (1.0 - spectrum.normalized_entropy)).clamp(0.0, 1.0)
}

fn expansion_index(spectrum: &LambdaSpectrumReport) -> f32 {
    (spectrum.tail_share * spectrum.normalized_entropy).clamp(0.0, 1.0)
}

fn classify_gradient(dominant_delta: f32, entropy_delta: f32) -> &'static str {
    if dominant_delta > 0.035 && entropy_delta < -0.025 {
        "concentrating"
    } else if dominant_delta < -0.035 && entropy_delta > 0.025 {
        "widening"
    } else if dominant_delta.abs() > 0.05 || entropy_delta.abs() > 0.04 {
        "shifting"
    } else {
        "holding"
    }
}

fn sequential_color(value: f32) -> String {
    let v = value.clamp(0.0, 1.0);
    let red = lerp_u8(239, 30, v);
    let green = lerp_u8(246, 64, v);
    let blue = lerp_u8(255, 175, v);
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn lerp_u8(start: u8, end: u8, frac: f32) -> u8 {
    let start_f = f32::from(start);
    let end_f = f32::from(end);
    (start_f + (end_f - start_f) * frac)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn svg_header(width: f32, height: f32) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\">"
    )
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn csv_escape(input: String) -> String {
    if input.contains(',') || input.contains('"') || input.contains('\n') {
        format!("\"{}\"", input.replace('"', "\"\""))
    } else {
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lambda_spectrum_reports_normalized_energy() {
        let mut features = [0.0_f32; SEMANTIC_DIM];
        features[0] = 1.0;
        features[1] = -0.5;
        features[20] = 0.25;

        let report = lambda_spectrum(&features);
        let share_sum = report.bins.iter().sum::<f32>();

        assert!((share_sum - 1.0).abs() < 0.001);
        assert!((1..=LAMBDA_PROXY_BINS).contains(&report.dominant_mode));
        assert!(report.total_energy > 0.0);
    }

    #[test]
    fn time_domain_features_capture_rhythm_and_rates() {
        let report = time_domain_features("Hello??\nAardvark 123!!");

        assert_eq!(report.word_count, 3);
        assert!(report.question_rate > 0.0);
        assert!(report.exclamation_rate > 0.0);
        assert!(report.digit_rate > 0.0);
        assert!(report.rhythm_alternation_rate > 0.0);
    }

    #[test]
    fn gradient_classifier_distinguishes_concentration_and_widening() {
        assert_eq!(classify_gradient(0.05, -0.03), "concentrating");
        assert_eq!(classify_gradient(-0.05, 0.03), "widening");
        assert_eq!(classify_gradient(0.01, 0.0), "holding");
    }
}
