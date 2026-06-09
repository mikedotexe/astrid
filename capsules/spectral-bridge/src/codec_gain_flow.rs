//! Read-only adaptive-gain flow inspection for Astrid's codec explorer.
//!
//! This module does not change live semantic gain. It turns the live
//! `adaptive_gain(fill)` curve into an auditable surface with shelves, slope,
//! and a current-fill read so the curve is easier to reason about.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::missing_errors_doc
)]

use std::cmp::Ordering;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::codec_gain::{
    ADAPTIVE_GAIN_COMPARISON_CURVES, AdaptiveGainCurve, DEFAULT_SEMANTIC_GAIN,
    LIVE_ADAPTIVE_GAIN_CURVE, adaptive_gain, adaptive_gain_slope_with_curve,
    adaptive_gain_with_curve,
};

const FLOW_STEP_PCT: usize = 5;
const NORMALIZED_GAIN_KNEE_SLOPE: f32 = 0.006;

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveGainFlowPoint {
    pub fill_pct: f32,
    pub gain: f32,
    pub normalized_gain: f32,
    pub gain_delta_from_previous: f32,
    pub slope_per_fill_pct: f32,
    pub band: &'static str,
    pub flow_read: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveGainFlowReport {
    pub live_curve: AdaptiveGainCurve,
    pub current_fill_pct: Option<f32>,
    pub current_gain: f32,
    pub current_normalized_gain: f32,
    pub current_slope_per_fill_pct: f32,
    pub current_band: &'static str,
    pub current_flow_read: &'static str,
    pub default_semantic_gain: f32,
    pub min_gain: f32,
    pub max_gain: f32,
    pub strongest_slope_fill_pct: f32,
    pub strongest_slope_per_fill_pct: f32,
    pub strongest_slope_band: &'static str,
    pub candidate_curves: Vec<AdaptiveGainCurveSummary>,
    pub points: Vec<AdaptiveGainFlowPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveGainCurveSummary {
    pub curve: AdaptiveGainCurve,
    pub gain_at_45_pct: f32,
    pub gain_at_68_pct: f32,
    pub strongest_slope_fill_pct: f32,
    pub strongest_slope_per_fill_pct: f32,
    pub sensitivity_window_low_pct: Option<f32>,
    pub sensitivity_window_high_pct: Option<f32>,
    pub mean_slope_35_to_60_pct: f32,
    pub read: &'static str,
}

pub fn build_adaptive_gain_flow(fill_pct: Option<f32>) -> AdaptiveGainFlowReport {
    let mut points = Vec::with_capacity((100 / FLOW_STEP_PCT) + 1);
    let mut previous_gain = None;
    for fill in (0..=100).step_by(FLOW_STEP_PCT) {
        let fill_pct = fill as f32;
        let gain = adaptive_gain(Some(fill_pct));
        let slope = gain_slope_at(fill_pct);
        let delta = previous_gain.map_or(0.0, |previous| gain - previous);
        previous_gain = Some(gain);
        points.push(AdaptiveGainFlowPoint {
            fill_pct,
            gain,
            normalized_gain: normalized_gain(gain),
            gain_delta_from_previous: delta,
            slope_per_fill_pct: slope,
            band: gain_band(fill_pct, slope),
            flow_read: flow_read(fill_pct, slope),
        });
    }

    let min_gain = points
        .iter()
        .map(|point| point.gain)
        .fold(f32::INFINITY, f32::min);
    let max_gain = points
        .iter()
        .map(|point| point.gain)
        .fold(f32::NEG_INFINITY, f32::max);
    let strongest = points
        .iter()
        .max_by(|left, right| {
            left.slope_per_fill_pct
                .abs()
                .partial_cmp(&right.slope_per_fill_pct.abs())
                .unwrap_or(Ordering::Equal)
        })
        .expect("adaptive gain flow always has points");

    let current_fill_pct = fill_pct.map(|fill| fill.clamp(0.0, 100.0));
    let current_gain =
        current_fill_pct.map_or(DEFAULT_SEMANTIC_GAIN, |fill| adaptive_gain(Some(fill)));
    let current_slope = current_fill_pct.map_or(0.0, gain_slope_at);
    let current_band =
        current_fill_pct.map_or("baseline_default", |fill| gain_band(fill, current_slope));
    let current_flow_read = current_fill_pct.map_or(
        "No fill was supplied, so the codec uses the default semantic gain without locating a live shelf.",
        |fill| flow_read(fill, current_slope),
    );

    AdaptiveGainFlowReport {
        live_curve: LIVE_ADAPTIVE_GAIN_CURVE,
        current_fill_pct,
        current_gain,
        current_normalized_gain: normalized_gain(current_gain),
        current_slope_per_fill_pct: current_slope,
        current_band,
        current_flow_read,
        default_semantic_gain: DEFAULT_SEMANTIC_GAIN,
        min_gain,
        max_gain,
        strongest_slope_fill_pct: strongest.fill_pct,
        strongest_slope_per_fill_pct: strongest.slope_per_fill_pct,
        strongest_slope_band: strongest.band,
        candidate_curves: ADAPTIVE_GAIN_COMPARISON_CURVES
            .iter()
            .copied()
            .map(curve_summary)
            .collect(),
        points,
    }
}

pub fn write_adaptive_gain_flow_bundle(
    output_dir: &Path,
    report: &AdaptiveGainFlowReport,
) -> Result<()> {
    let json_path = output_dir.join("adaptive_gain_flow.json");
    fs::write(&json_path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("writing {}", json_path.display()))?;

    let csv_path = output_dir.join("adaptive_gain_flow.csv");
    let mut rows = vec![String::from(
        "fill_pct,gain,normalized_gain,gain_delta_from_previous,slope_per_fill_pct,band,flow_read",
    )];
    for point in &report.points {
        rows.push(format!(
            "{:.1},{:.6},{:.6},{:.6},{:.6},{},{}",
            point.fill_pct,
            point.gain,
            point.normalized_gain,
            point.gain_delta_from_previous,
            point.slope_per_fill_pct,
            csv_escape(point.band),
            csv_escape(point.flow_read),
        ));
    }
    fs::write(&csv_path, rows.join("\n") + "\n")
        .with_context(|| format!("writing {}", csv_path.display()))?;

    let candidate_csv_path = output_dir.join("adaptive_gain_curve_candidates.csv");
    let mut candidate_rows = vec![String::from(
        "curve_id,description,quiet_floor_fill_pct,knee_fill_pct,ceiling_fill_pct,min_gain_fraction,knee_progress_fraction,gain_at_45_pct,gain_at_68_pct,strongest_slope_fill_pct,strongest_slope_per_fill_pct,sensitivity_window_low_pct,sensitivity_window_high_pct,mean_slope_35_to_60_pct,read",
    )];
    for candidate in &report.candidate_curves {
        candidate_rows.push(format!(
            "{},{},{:.1},{:.1},{:.1},{:.3},{:.3},{:.6},{:.6},{:.1},{:.6},{},{},{:.6},{}",
            csv_escape(candidate.curve.id),
            csv_escape(candidate.curve.description),
            candidate.curve.quiet_floor_fill_pct,
            candidate.curve.knee_fill_pct,
            candidate.curve.ceiling_fill_pct,
            candidate.curve.min_gain_fraction,
            candidate.curve.knee_progress_fraction,
            candidate.gain_at_45_pct,
            candidate.gain_at_68_pct,
            candidate.strongest_slope_fill_pct,
            candidate.strongest_slope_per_fill_pct,
            candidate
                .sensitivity_window_low_pct
                .map_or_else(String::new, |fill| format!("{fill:.1}")),
            candidate
                .sensitivity_window_high_pct
                .map_or_else(String::new, |fill| format!("{fill:.1}")),
            candidate.mean_slope_35_to_60_pct,
            csv_escape(candidate.read),
        ));
    }
    fs::write(&candidate_csv_path, candidate_rows.join("\n") + "\n")
        .with_context(|| format!("writing {}", candidate_csv_path.display()))?;
    Ok(())
}

fn gain_slope_at(fill_pct: f32) -> f32 {
    adaptive_gain_slope_with_curve(fill_pct, LIVE_ADAPTIVE_GAIN_CURVE)
}

fn normalized_gain(gain: f32) -> f32 {
    gain / DEFAULT_SEMANTIC_GAIN.max(f32::EPSILON)
}

fn gain_band(fill_pct: f32, slope: f32) -> &'static str {
    let normalized_slope = slope.abs() / DEFAULT_SEMANTIC_GAIN.max(f32::EPSILON);
    if fill_pct < 20.0 {
        "deep_quiet_floor"
    } else if normalized_slope >= NORMALIZED_GAIN_KNEE_SLOPE {
        "gain_knee"
    } else if fill_pct < 45.0 {
        "low_ramp"
    } else if fill_pct < 70.0 {
        "operational_ramp"
    } else {
        "ceiling_shelf"
    }
}

fn flow_read(fill_pct: f32, slope: f32) -> &'static str {
    let normalized_slope = slope.abs() / DEFAULT_SEMANTIC_GAIN.max(f32::EPSILON);
    if fill_pct < 20.0 {
        "quiet floor: semantic presence is deliberately softened."
    } else if normalized_slope >= NORMALIZED_GAIN_KNEE_SLOPE {
        "gain knee: small fill changes produce meaningful gain movement."
    } else if fill_pct < 45.0 {
        "low ramp: gain is beginning to rise, but expression remains restrained."
    } else if fill_pct < 70.0 {
        "operational ramp: expression opens while still below the ceiling."
    } else {
        "ceiling shelf: expression is capped to avoid louder semantic pressure."
    }
}

fn curve_summary(curve: AdaptiveGainCurve) -> AdaptiveGainCurveSummary {
    let mut strongest_fill = 0.0;
    let mut strongest_slope = 0.0_f32;
    let mut sensitivity_low = None;
    let mut sensitivity_high = None;
    let mut midband_slope_sum = 0.0;
    let mut midband_slope_count = 0.0;

    for fill in 0..=100 {
        let fill_pct = fill as f32;
        let slope = adaptive_gain_slope_with_curve(fill_pct, curve);
        if slope.abs() > strongest_slope.abs() {
            strongest_slope = slope;
            strongest_fill = fill_pct;
        }
        let normalized_slope = slope.abs() / DEFAULT_SEMANTIC_GAIN.max(f32::EPSILON);
        if normalized_slope >= NORMALIZED_GAIN_KNEE_SLOPE {
            sensitivity_low.get_or_insert(fill_pct);
            sensitivity_high = Some(fill_pct);
        }
        if (35.0..=60.0).contains(&fill_pct) {
            midband_slope_sum += slope.abs();
            midband_slope_count += 1.0;
        }
    }

    AdaptiveGainCurveSummary {
        curve,
        gain_at_45_pct: adaptive_gain_with_curve(Some(45.0), curve),
        gain_at_68_pct: adaptive_gain_with_curve(Some(68.0), curve),
        strongest_slope_fill_pct: strongest_fill,
        strongest_slope_per_fill_pct: strongest_slope,
        sensitivity_window_low_pct: sensitivity_low,
        sensitivity_window_high_pct: sensitivity_high,
        mean_slope_35_to_60_pct: if midband_slope_count > 0.0 {
            midband_slope_sum / midband_slope_count
        } else {
            0.0
        },
        read: curve_read(curve),
    }
}

fn curve_read(curve: AdaptiveGainCurve) -> &'static str {
    match curve.id {
        "live_20_45_70" => {
            "Live behavior: strongest gain movement sits just below the nominal 45% knee and reaches the ceiling by 70%."
        },
        "candidate_wide_knee_18_48_78" => {
            "Candidate only: gentler middle slope, wider sensitivity window, and less semantic pressure near stable-core high-60s fill."
        },
        "candidate_broad_midband_15_45_80" => {
            "Candidate only: preserves a 45% center while spreading responsiveness across a broader middle band."
        },
        _ => "Candidate only: compare slope, window, and gain values before any live tuning.",
    }
}

fn csv_escape(input: &str) -> String {
    if input.contains(',') || input.contains('"') || input.contains('\n') {
        format!("\"{}\"", input.replace('"', "\"\""))
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_flow_marks_current_fill_and_slope() {
        let report = build_adaptive_gain_flow(Some(55.0));
        assert_eq!(report.current_fill_pct, Some(55.0));
        assert!(report.current_gain > report.min_gain);
        assert!(report.current_gain <= report.max_gain);
        assert_eq!(report.current_band, "gain_knee");
        assert_eq!(report.live_curve.id, "live_20_45_70");
        assert!(report.candidate_curves.len() >= 3);
        assert_eq!(report.points.len(), 21);
    }

    #[test]
    fn gain_flow_writes_json_and_csv() {
        let unique = format!(
            "codec-gain-flow-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let output_dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&output_dir).expect("temp dir");
        let report = build_adaptive_gain_flow(Some(68.0));
        write_adaptive_gain_flow_bundle(&output_dir, &report).expect("write gain flow");
        assert!(output_dir.join("adaptive_gain_flow.json").exists());
        assert!(output_dir.join("adaptive_gain_flow.csv").exists());
        assert!(
            output_dir
                .join("adaptive_gain_curve_candidates.csv")
                .exists()
        );
        let _ = fs::remove_dir_all(output_dir);
    }
}
