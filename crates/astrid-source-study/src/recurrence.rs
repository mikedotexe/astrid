//! Bounded descriptive analysis of frozen ESN activations, never engine covariance.
#![allow(clippy::arithmetic_side_effects, clippy::cast_precision_loss)] // At most 180 finite 128D vectors in [-1,1]; all clocks validated before subtraction.
use crate::geometry::{Frame, Snapshot};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
#[cfg(test)]
#[path = "recurrence_tests.rs"]
mod tests;

pub(crate) const LIMITS: &str = "Sampled ESN activation coordinates only, not the engine covariance matrix. No interpolation, significance, attractor, causal or experiential claim. Covariance similarity does not establish repeated temporal order. Separate captures are exploratory: boot/node identity is unavailable.";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Window {
    pub capture: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "recipe", deny_unknown_fields)]
pub(crate) enum Analysis {
    #[serde(rename = "state-return-rms-v1")]
    StateReturn {
        window: Window,
        threshold: f64,
        #[serde(default = "exclusion")]
        temporal_exclusion_ms: u64,
    },
    #[serde(rename = "activation-covariance-shape-v1")]
    CovarianceShape { first: Window, second: Window },
}
fn exclusion() -> u64 {
    5000
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Coverage {
    pub sample_count: usize,
    pub rank_upper_bound: usize,
    pub engine_start_ms: u64,
    pub engine_end_ms: u64,
    pub wall_start_ms: u64,
    pub wall_end_ms: u64,
    pub gaps: Vec<[u64; 2]>,
    pub source_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Diagonal {
    pub samples: usize,
    pub first_engine_ms: [u64; 2],
    pub second_engine_ms: [u64; 2],
    pub first_wall_ms: [u64; 2],
    pub second_wall_ms: [u64; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "recipe")]
pub(crate) enum ResultRecord {
    #[serde(rename = "state-return-rms-v1")]
    StateReturn {
        coverage: Coverage,
        eligible_pairs: usize,
        near_pairs: usize,
        near_state_fraction: Option<f64>,
        mean_per_node_variance: f64,
        degenerate: bool,
        diagonals: Vec<Diagonal>,
        limits: String,
    },
    #[serde(rename = "activation-covariance-shape-v1")]
    CovarianceShape {
        first: Coverage,
        second: Coverage,
        mean_state_rms_distance: f64,
        first_mean_per_node_variance: f64,
        second_mean_per_node_variance: f64,
        first_covariance_frobenius: f64,
        second_covariance_frobenius: f64,
        normalized_frobenius_similarity: Option<f64>,
        insufficient_variance: bool,
        separate_captures_exploratory: bool,
        limits: String,
    },
}

fn select<'a>(snapshot: &'a Snapshot, window: &Window, minimum: usize) -> Result<Vec<&'a Frame>> {
    snapshot.validate_observation()?;
    ensure!(window.start_ms <= window.end_ms, "reversed window");
    ensure!(
        window.start_ms >= snapshot.frames[0].t_ms
            && window.end_ms <= snapshot.frames.last().context("empty capture")?.t_ms,
        "window is outside frozen evidence"
    );
    let frames: Vec<_> = snapshot
        .frames
        .iter()
        .filter(|f| (window.start_ms..=window.end_ms).contains(&f.t_ms))
        .collect();
    ensure!(
        frames.len() >= minimum,
        "insufficient frames: need {minimum}"
    );
    Ok(frames)
}

fn coverage(snapshot: &Snapshot, frames: &[&Frame]) -> Coverage {
    let first = frames[0];
    let last = frames[frames.len() - 1];
    Coverage {
        sample_count: frames.len(),
        rank_upper_bound: frames.len().saturating_sub(1).min(128),
        engine_start_ms: first.t_ms,
        engine_end_ms: last.t_ms,
        wall_start_ms: first.wall_clock_unix_ms,
        wall_end_ms: last.wall_clock_unix_ms,
        gaps: frames
            .windows(2)
            .filter(|p| !consecutive(p[0], p[1]))
            .map(|p| [p[0].t_ms, p[1].t_ms])
            .collect(),
        source_sha256: snapshot.source_sha256.clone(),
    }
}
fn consecutive(a: &Frame, b: &Frame) -> bool {
    b.t_ms - a.t_ms <= 1500 && b.wall_clock_unix_ms - a.wall_clock_unix_ms <= 1500
}
fn rms(a: &[f64], b: &[f64]) -> f64 {
    (a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f64>() / 128.0).sqrt()
}
fn moments(frames: &[&Frame]) -> (Vec<f64>, Vec<f64>, f64) {
    let n = frames.len() as f64;
    let mean: Vec<_> = (0..128)
        .map(|i| frames.iter().map(|f| f.activations[i]).sum::<f64>() / n)
        .collect();
    let mut covariance = vec![0.0; 128 * 128];
    for frame in frames {
        for i in 0..128 {
            for j in 0..128 {
                covariance[i * 128 + j] +=
                    (frame.activations[i] - mean[i]) * (frame.activations[j] - mean[j]) / (n - 1.0);
            }
        }
    }
    let variance = (0..128).map(|i| covariance[i * 128 + i]).sum::<f64>() / 128.0;
    (mean, covariance, variance)
}

#[allow(clippy::too_many_lines)] // Two bounded recipes share validation and a single typed result boundary; no IO or inference.
pub(crate) fn run(
    analysis: &Analysis,
    get: impl Fn(&str) -> Result<Snapshot>,
) -> Result<ResultRecord> {
    match analysis {
        Analysis::StateReturn {
            window,
            threshold,
            temporal_exclusion_ms,
        } => {
            ensure!(
                threshold.is_finite() && *threshold > 0.0 && *threshold <= 2.0,
                "explicit RMS threshold must be in (0,2]"
            );
            ensure!(
                *temporal_exclusion_ms <= 180_000,
                "temporal exclusion exceeds recording bound"
            );
            let snapshot = get(&window.capture)?;
            let frames = select(&snapshot, window, 32)?;
            let n = frames.len();
            let mut near = vec![false; n * n];
            let mut eligible_pairs = 0;
            let mut near_pairs = 0;
            for i in 0..n {
                for j in i + 1..n {
                    if frames[j].t_ms - frames[i].t_ms <= *temporal_exclusion_ms {
                        continue;
                    }
                    eligible_pairs += 1;
                    if rms(&frames[i].activations, &frames[j].activations) <= *threshold {
                        near[i * n + j] = true;
                        near_pairs += 1;
                    }
                }
            }
            let (_, _, variance) = moments(&frames);
            let degenerate = variance <= 1e-12;
            let mut diagonals = Vec::new();
            if !degenerate {
                for i in 0..n {
                    for j in i + 1..n {
                        if !near[i * n + j]
                            || (i > 0
                                && near[(i - 1) * n + j - 1]
                                && consecutive(frames[i - 1], frames[i])
                                && consecutive(frames[j - 1], frames[j]))
                        {
                            continue;
                        }
                        let mut length = 1;
                        while j + length < n
                            && near[(i + length) * n + j + length]
                            && consecutive(frames[i + length - 1], frames[i + length])
                            && consecutive(frames[j + length - 1], frames[j + length])
                        {
                            length += 1;
                        }
                        if length >= 2 {
                            diagonals.push(Diagonal {
                                samples: length,
                                first_engine_ms: [frames[i].t_ms, frames[i + length - 1].t_ms],
                                second_engine_ms: [frames[j].t_ms, frames[j + length - 1].t_ms],
                                first_wall_ms: [
                                    frames[i].wall_clock_unix_ms,
                                    frames[i + length - 1].wall_clock_unix_ms,
                                ],
                                second_wall_ms: [
                                    frames[j].wall_clock_unix_ms,
                                    frames[j + length - 1].wall_clock_unix_ms,
                                ],
                            });
                        }
                    }
                }
            }
            Ok(ResultRecord::StateReturn {
                coverage: coverage(&snapshot, &frames),
                eligible_pairs,
                near_pairs,
                near_state_fraction: (eligible_pairs > 0)
                    .then(|| near_pairs as f64 / eligible_pairs as f64),
                mean_per_node_variance: variance,
                degenerate,
                diagonals,
                limits: LIMITS.into(),
            })
        },
        Analysis::CovarianceShape { first, second } => {
            let a = get(&first.capture)?;
            let b = get(&second.capture)?;
            let fa = select(&a, first, 30)?;
            let fb = select(&b, second, 30)?;
            ensure!(
                first.capture != second.capture
                    || first.end_ms < second.start_ms
                    || second.end_ms < first.start_ms,
                "covariance windows must not overlap"
            );
            ensure!(
                fa.last().context("empty window")?.wall_clock_unix_ms < fb[0].wall_clock_unix_ms
                    || fb.last().context("empty window")?.wall_clock_unix_ms
                        < fa[0].wall_clock_unix_ms,
                "covariance windows overlap in wall time"
            );
            let (ma, ca, va) = moments(&fa);
            let (mb, cb, vb) = moments(&fb);
            let na = ca.iter().map(|x| x * x).sum::<f64>().sqrt();
            let nb = cb.iter().map(|x| x * x).sum::<f64>().sqrt();
            let insufficient = va <= 1e-12 || vb <= 1e-12;
            Ok(ResultRecord::CovarianceShape {
                first: coverage(&a, &fa),
                second: coverage(&b, &fb),
                mean_state_rms_distance: rms(&ma, &mb),
                first_mean_per_node_variance: va,
                second_mean_per_node_variance: vb,
                first_covariance_frobenius: na,
                second_covariance_frobenius: nb,
                normalized_frobenius_similarity: (!insufficient).then(|| {
                    (ca.iter().zip(&cb).map(|(x, y)| x * y).sum::<f64>() / (na * nb))
                        .clamp(-1.0, 1.0)
                }),
                insufficient_variance: insufficient,
                separate_captures_exploratory: first.capture != second.capture,
                limits: LIMITS.into(),
            })
        },
    }
}
