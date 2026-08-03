#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    reason = "bounded IEEE-754 summaries use checked/capped collection indices"
)]

use serde::{Deserialize, Serialize};

use crate::{SpectralMetrics, SpectrumBasis};

pub const MAX_CROSS_CORRELATION_LAG: usize = 1_440;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimedScalar {
    pub t_ms: u64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarSummary {
    pub count: usize,
    pub min: f64,
    pub mean: f64,
    pub max: f64,
    pub standard_deviation: f64,
    pub first: f64,
    pub last: f64,
    pub change: f64,
    pub slope_per_sample: Option<f64>,
    pub slope_per_second: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrelationSummary {
    pub paired_count: usize,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossCorrelationPoint {
    /// Positive lag compares `left[t]` with later `right[t + lag]`.
    pub lag_samples: i32,
    pub paired_count: usize,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedSpectralObservation {
    pub t_ms: u64,
    pub metrics: SpectralMetrics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_turnover: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_identity_stable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollingSpectralSummary {
    pub sample_count: usize,
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    pub full_spectrum_sample_count: usize,
    pub partial_spectrum_sample_count: usize,
    pub unknown_coverage_sample_count: usize,
    pub inconsistent_coverage_sample_count: usize,
    pub incomplete_sanitized_sample_count: usize,
    pub identity_unstable_sample_count: usize,
    pub normalized_entropy: ScalarSummary,
    pub effective_modes: ScalarSummary,
    pub lambda1_share: ScalarSummary,
    pub head_share: ScalarSummary,
    pub shoulder_share: ScalarSummary,
    pub tail_share: ScalarSummary,
    pub density_gradient: Option<ScalarSummary>,
    pub mode_turnover: Option<ScalarSummary>,
}

/// Summarizes finite values in caller-provided order.
#[must_use]
pub fn summarize_scalars(values: &[f64]) -> Option<ScalarSummary> {
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    summarize_ordered(&finite, None)
}

/// Summarizes finite values in timestamp order and adds a least-squares slope.
#[must_use]
pub fn summarize_timed_scalars(values: &[TimedScalar]) -> Option<ScalarSummary> {
    let mut finite = values
        .iter()
        .copied()
        .filter(|sample| sample.value.is_finite())
        .collect::<Vec<_>>();
    finite.sort_by_key(|sample| sample.t_ms);
    let ordered = finite.iter().map(|sample| sample.value).collect::<Vec<_>>();
    summarize_ordered(&ordered, Some(&finite))
}

fn summarize_ordered(values: &[f64], timed: Option<&[TimedScalar]>) -> Option<ScalarSummary> {
    let first = *values.first()?;
    let last = *values.last()?;
    let count = values.len();
    let sum = values.iter().sum::<f64>();
    let mean = sum / count as f64;
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f64>()
        / count as f64;
    let min = values.iter().copied().min_by(f64::total_cmp)?;
    let max = values.iter().copied().max_by(f64::total_cmp)?;

    Some(ScalarSummary {
        count,
        min,
        mean,
        max,
        standard_deviation: variance.max(0.0).sqrt(),
        first,
        last,
        change: last - first,
        slope_per_sample: linear_slope(values),
        slope_per_second: timed.and_then(timed_linear_slope_per_second),
    })
}

fn linear_slope(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean_x = (values.len().saturating_sub(1)) as f64 / 2.0;
    let mean_y = values.iter().sum::<f64>() / values.len() as f64;
    let mut numerator = 0.0_f64;
    let mut denominator = 0.0_f64;
    for (index, value) in values.iter().enumerate() {
        let centered_x = index as f64 - mean_x;
        numerator += centered_x * (*value - mean_y);
        denominator += centered_x * centered_x;
    }
    (denominator > f64::EPSILON).then(|| numerator / denominator)
}

fn timed_linear_slope_per_second(values: &[TimedScalar]) -> Option<f64> {
    if values.len() < 2 || values.first()?.t_ms == values.last()?.t_ms {
        return None;
    }
    let origin = values.first()?.t_ms;
    let xs = values
        .iter()
        .map(|sample| sample.t_ms.saturating_sub(origin) as f64 / 1_000.0)
        .collect::<Vec<_>>();
    let mean_x = xs.iter().sum::<f64>() / xs.len() as f64;
    let mean_y = values.iter().map(|sample| sample.value).sum::<f64>() / values.len() as f64;
    let mut numerator = 0.0_f64;
    let mut denominator = 0.0_f64;
    for (x, sample) in xs.iter().zip(values) {
        let centered_x = *x - mean_x;
        numerator += centered_x * (sample.value - mean_y);
        denominator += centered_x * centered_x;
    }
    (denominator > f64::EPSILON).then(|| numerator / denominator)
}

/// Pearson correlation over same-index finite pairs. A constant series has no
/// defined coefficient and returns `None` rather than manufacturing zero.
#[must_use]
pub fn pearson_correlation(left: &[f64], right: &[f64]) -> Option<CorrelationSummary> {
    let pairs = left
        .iter()
        .zip(right)
        .filter_map(|(left, right)| {
            (left.is_finite() && right.is_finite()).then_some((*left, *right))
        })
        .collect::<Vec<_>>();
    if pairs.len() < 2 {
        return None;
    }
    let count = pairs.len();
    let left_mean = pairs.iter().map(|pair| pair.0).sum::<f64>() / count as f64;
    let right_mean = pairs.iter().map(|pair| pair.1).sum::<f64>() / count as f64;
    let mut covariance = 0.0_f64;
    let mut left_variance = 0.0_f64;
    let mut right_variance = 0.0_f64;
    for (left, right) in pairs {
        let left_delta = left - left_mean;
        let right_delta = right - right_mean;
        covariance += left_delta * right_delta;
        left_variance += left_delta * left_delta;
        right_variance += right_delta * right_delta;
    }
    let denominator = (left_variance * right_variance).sqrt();
    if denominator <= f64::EPSILON {
        return None;
    }
    Some(CorrelationSummary {
        paired_count: count,
        coefficient: (covariance / denominator).clamp(-1.0, 1.0),
    })
}

/// Computes bounded sample-lag correlations. No cadence or causal meaning is
/// inferred from the caller's sample spacing.
#[must_use]
pub fn cross_correlation(
    left: &[f64],
    right: &[f64],
    requested_max_lag: usize,
) -> Vec<CrossCorrelationPoint> {
    let common_len = left.len().min(right.len());
    if common_len < 2 {
        return Vec::new();
    }
    let max_lag = requested_max_lag
        .min(MAX_CROSS_CORRELATION_LAG)
        .min(common_len.saturating_sub(2));
    let signed_max = i32::try_from(max_lag).unwrap_or(i32::MAX);
    let mut points = Vec::with_capacity(max_lag.saturating_mul(2).saturating_add(1));
    for lag in -signed_max..=signed_max {
        let offset = lag.unsigned_abs() as usize;
        let length = common_len.saturating_sub(offset);
        let (left_slice, right_slice) = if lag >= 0 {
            (
                &left[..length],
                &right[offset..offset.saturating_add(length)],
            )
        } else {
            (
                &left[offset..offset.saturating_add(length)],
                &right[..length],
            )
        };
        if let Some(correlation) = pearson_correlation(left_slice, right_slice) {
            points.push(CrossCorrelationPoint {
                lag_samples: lag,
                paired_count: correlation.paired_count,
                coefficient: correlation.coefficient,
            });
        }
    }
    points
}

/// Produces a bounded metric-by-metric summary without interpreting it.
#[must_use]
pub fn rolling_spectral_summary(
    samples: &[TimedSpectralObservation],
) -> Option<RollingSpectralSummary> {
    let mut ordered = samples.to_vec();
    ordered.sort_by_key(|sample| sample.t_ms);
    let first = ordered.first()?;
    let last = ordered.last()?;
    let summarize = |extract: fn(&TimedSpectralObservation) -> f64| {
        summarize_timed_scalars(
            &ordered
                .iter()
                .map(|sample| TimedScalar {
                    t_ms: sample.t_ms,
                    value: extract(sample),
                })
                .collect::<Vec<_>>(),
        )
    };
    let density_gradient = summarize_optional(&ordered, |sample| sample.metrics.density_gradient);
    let mode_turnover = summarize_optional(&ordered, |sample| sample.mode_turnover);

    Some(RollingSpectralSummary {
        sample_count: ordered.len(),
        window_start_ms: first.t_ms,
        window_end_ms: last.t_ms,
        full_spectrum_sample_count: basis_count(&ordered, SpectrumBasis::FullSpectrum),
        partial_spectrum_sample_count: basis_count(&ordered, SpectrumBasis::ExportedSpectrumPrefix),
        unknown_coverage_sample_count: basis_count(
            &ordered,
            SpectrumBasis::ExportedSpectrumUnknownCoverage,
        ),
        inconsistent_coverage_sample_count: basis_count(
            &ordered,
            SpectrumBasis::ExportedSpectrumWithInconsistentCoverage,
        ),
        incomplete_sanitized_sample_count: basis_count(
            &ordered,
            SpectrumBasis::IncompleteSanitizedSpectrum,
        ),
        identity_unstable_sample_count: ordered
            .iter()
            .filter(|sample| sample.mode_identity_stable == Some(false))
            .count(),
        normalized_entropy: summarize(|sample| sample.metrics.normalized_entropy)?,
        effective_modes: summarize(|sample| sample.metrics.effective_modes)?,
        lambda1_share: summarize(|sample| sample.metrics.lambda1_share)?,
        head_share: summarize(|sample| sample.metrics.energy_shares.head)?,
        shoulder_share: summarize(|sample| sample.metrics.energy_shares.shoulder)?,
        tail_share: summarize(|sample| sample.metrics.energy_shares.tail)?,
        density_gradient,
        mode_turnover,
    })
}

fn summarize_optional(
    samples: &[TimedSpectralObservation],
    extract: impl Fn(&TimedSpectralObservation) -> Option<f64>,
) -> Option<ScalarSummary> {
    summarize_timed_scalars(
        &samples
            .iter()
            .filter_map(|sample| {
                extract(sample).map(|value| TimedScalar {
                    t_ms: sample.t_ms,
                    value,
                })
            })
            .collect::<Vec<_>>(),
    )
}

fn basis_count(samples: &[TimedSpectralObservation], basis: SpectrumBasis) -> usize {
    samples
        .iter()
        .filter(|sample| sample.metrics.basis == basis)
        .count()
}
