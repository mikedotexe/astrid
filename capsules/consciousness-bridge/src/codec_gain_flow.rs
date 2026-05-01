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

use crate::codec::{DEFAULT_SEMANTIC_GAIN, adaptive_gain};

const FLOW_STEP_PCT: usize = 5;
const SLOPE_WINDOW_PCT: f32 = 1.0;
const NORMALIZED_GAIN_KNEE_SLOPE: f32 = 0.009;

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
    pub points: Vec<AdaptiveGainFlowPoint>,
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
    Ok(())
}

fn gain_slope_at(fill_pct: f32) -> f32 {
    let low = (fill_pct - SLOPE_WINDOW_PCT).clamp(0.0, 100.0);
    let high = (fill_pct + SLOPE_WINDOW_PCT).clamp(0.0, 100.0);
    let span = (high - low).max(f32::EPSILON);
    (adaptive_gain(Some(high)) - adaptive_gain(Some(low))) / span
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
        "gain knee: small fill changes produce the strongest gain movement."
    } else if fill_pct < 45.0 {
        "low ramp: gain is beginning to rise, but expression remains restrained."
    } else if fill_pct < 70.0 {
        "operational ramp: expression opens while still below the ceiling."
    } else {
        "ceiling shelf: expression is capped to avoid louder semantic pressure."
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
        let _ = fs::remove_dir_all(output_dir);
    }
}
