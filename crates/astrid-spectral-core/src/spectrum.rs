#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    reason = "bounded IEEE-754 spectral calculations cannot overflow integer state"
)]

use std::cmp::Ordering;

use astrid_minime_protocol::{
    SpectralFillSemanticsV1, SpectralFillSmoothingV1, SpectralSubstrateV1,
};
use serde::{Deserialize, Serialize};

const ENERGY_EPSILON: f64 = 1.0e-12;
const ACTIVE_SHARE_EPSILON: f64 = 1.0e-10;
pub const MAX_TRACKED_MODES: usize = 4;

/// Honest accounting for the serialized spectrum versus its declared source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectrumCoverage {
    pub declared_full_mode_count: Option<usize>,
    pub exported_mode_count: usize,
    pub usable_mode_count: usize,
    pub declared_count_is_consistent: bool,
    pub exported_mode_fraction: Option<f64>,
    pub full_spectrum_exported: Option<bool>,
}

/// A finite, non-negative, descending spectrum plus sanitation provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SanitizedSpectrum {
    pub eigenvalues: Vec<f64>,
    pub input_mode_count: usize,
    pub discarded_non_finite_count: usize,
    pub clamped_negative_count: usize,
    pub coverage: SpectrumCoverage,
}

/// Which spectrum supported a derived metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumBasis {
    FullSpectrum,
    ExportedSpectrumPrefix,
    ExportedSpectrumUnknownCoverage,
    ExportedSpectrumWithInconsistentCoverage,
    /// One or more declared spectrum values were unusable and discarded.
    IncompleteSanitizedSpectrum,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpectralEnergyShares {
    /// Energy in the first mode.
    pub head: f64,
    /// Energy in modes two and three.
    pub shoulder: f64,
    /// Energy in mode four and beyond.
    pub tail: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralGaps {
    pub lambda1_lambda2_absolute: Option<f64>,
    pub lambda1_lambda2_relative: Option<f64>,
    /// Relative adjacent drops `(left - right) / left`, bounded to `[0, 1]`.
    pub adjacent_relative_drops: Vec<f64>,
    pub largest_relative_drop: Option<f64>,
    pub largest_relative_drop_after_mode: Option<usize>,
}

/// Metrics derived only from the sanitized values named by `basis`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralMetrics {
    pub basis: SpectrumBasis,
    pub mode_count: usize,
    pub total_energy: f64,
    pub entropy_nats: f64,
    pub normalized_entropy: f64,
    pub effective_modes: f64,
    pub lambda1_share: f64,
    pub energy_shares: SpectralEnergyShares,
    /// Mean adjacent normalized drop; zero is flat and one approaches a cliff.
    pub density_gradient: Option<f64>,
    pub gaps: SpectralGaps,
    pub coverage: SpectrumCoverage,
}

/// A transient eigenmode. Runtime callers should retain no more than four.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralMode {
    pub eigenvalue: f64,
    pub components: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModeConcentration {
    pub component_count: usize,
    /// Inverse-participation concentration in `[1/n, 1]`.
    pub concentration: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModeTurnoverSummary {
    pub compared_mode_count: usize,
    pub mean_sign_invariant_turnover: Option<f64>,
    pub maximum_sign_invariant_turnover: Option<f64>,
    pub per_mode_turnover: Vec<Option<f64>>,
    pub identity_stable: bool,
    /// Zero-based modes involved in a near-degenerate adjacent eigenvalue pair.
    pub identity_unstable_modes: Vec<usize>,
    pub degeneracy_relative_tolerance: f64,
}

/// Removes non-finite values, clamps negative covariance artifacts to zero, and
/// sorts descending. Coverage is never inferred when the declaration is absent
/// or inconsistent.
#[must_use]
pub fn sanitize_spectrum(
    eigenvalues: &[f32],
    declared_full_mode_count: Option<usize>,
) -> SanitizedSpectrum {
    let mut discarded_non_finite_count = 0_usize;
    let mut clamped_negative_count = 0_usize;
    let mut sanitized = Vec::with_capacity(eigenvalues.len());
    for value in eigenvalues {
        if !value.is_finite() {
            discarded_non_finite_count = discarded_non_finite_count.saturating_add(1);
            continue;
        }
        if *value < 0.0 {
            clamped_negative_count = clamped_negative_count.saturating_add(1);
            sanitized.push(0.0);
        } else {
            sanitized.push(f64::from(*value));
        }
    }
    sanitized.sort_by(|left, right| right.partial_cmp(left).unwrap_or(Ordering::Equal));

    let exported_mode_count = eigenvalues.len();
    let consistent = declared_full_mode_count
        .is_none_or(|full_count| full_count >= exported_mode_count && full_count > 0);
    let exported_mode_fraction = declared_full_mode_count.and_then(|full_count| {
        if consistent {
            Some(exported_mode_count as f64 / full_count as f64)
        } else {
            None
        }
    });
    let full_spectrum_exported = declared_full_mode_count
        .filter(|_| consistent)
        .map(|full_count| full_count == exported_mode_count);

    SanitizedSpectrum {
        coverage: SpectrumCoverage {
            declared_full_mode_count,
            exported_mode_count,
            usable_mode_count: sanitized.len(),
            declared_count_is_consistent: consistent,
            exported_mode_fraction,
            full_spectrum_exported,
        },
        eigenvalues: sanitized,
        input_mode_count: eigenvalues.len(),
        discarded_non_finite_count,
        clamped_negative_count,
    }
}

impl SanitizedSpectrum {
    #[must_use]
    pub fn basis(&self) -> SpectrumBasis {
        if self.discarded_non_finite_count > 0
            || self.coverage.usable_mode_count < self.coverage.exported_mode_count
        {
            return SpectrumBasis::IncompleteSanitizedSpectrum;
        }
        match (
            self.coverage.declared_count_is_consistent,
            self.coverage.full_spectrum_exported,
        ) {
            (false, _) => SpectrumBasis::ExportedSpectrumWithInconsistentCoverage,
            (true, Some(true)) => SpectrumBasis::FullSpectrum,
            (true, Some(false)) => SpectrumBasis::ExportedSpectrumPrefix,
            (true, None) => SpectrumBasis::ExportedSpectrumUnknownCoverage,
        }
    }

    /// Returns no metrics for an empty or zero-energy spectrum.
    #[must_use]
    pub fn metrics(&self) -> Option<SpectralMetrics> {
        let total_energy = self.eigenvalues.iter().sum::<f64>();
        if !total_energy.is_finite() || total_energy <= ENERGY_EPSILON {
            return None;
        }

        let shares = self
            .eigenvalues
            .iter()
            .map(|value| *value / total_energy)
            .collect::<Vec<_>>();
        let entropy_nats = shares
            .iter()
            .filter(|share| **share > ENERGY_EPSILON)
            .map(|share| -*share * share.ln())
            .sum::<f64>();
        let normalized_entropy = if shares.len() > 1 {
            (entropy_nats / (shares.len() as f64).ln()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let effective_modes = entropy_nats.exp().clamp(1.0, shares.len() as f64);
        let lambda1_share = shares.first().copied().unwrap_or_default();
        let shoulder = shares.iter().skip(1).take(2).sum::<f64>();
        let tail = shares.iter().skip(3).sum::<f64>();

        Some(SpectralMetrics {
            basis: self.basis(),
            mode_count: shares.len(),
            total_energy,
            entropy_nats,
            normalized_entropy,
            effective_modes,
            lambda1_share,
            energy_shares: SpectralEnergyShares {
                head: lambda1_share,
                shoulder,
                tail,
            },
            density_gradient: density_gradient(&shares),
            gaps: spectral_gaps(&self.eigenvalues),
            coverage: self.coverage.clone(),
        })
    }
}

fn density_gradient(shares: &[f64]) -> Option<f64> {
    let active = shares
        .iter()
        .copied()
        .filter(|share| *share > ACTIVE_SHARE_EPSILON)
        .collect::<Vec<_>>();
    if active.len() < 2 {
        return None;
    }
    let mut sum = 0.0_f64;
    let mut count = 0_usize;
    for pair in active.windows(2) {
        let denominator = pair[0] + pair[1];
        if denominator > ENERGY_EPSILON {
            sum += (pair[0] - pair[1]).abs() / denominator;
            count = count.saturating_add(1);
        }
    }
    (count > 0).then(|| (sum / count as f64).clamp(0.0, 1.0))
}

fn spectral_gaps(eigenvalues: &[f64]) -> SpectralGaps {
    let lambda1_lambda2_absolute = eigenvalues
        .first()
        .zip(eigenvalues.get(1))
        .map(|(left, right)| (*left - *right).max(0.0));
    let lambda1_lambda2_relative =
        eigenvalues
            .first()
            .zip(eigenvalues.get(1))
            .and_then(|(left, right)| {
                (*left > ENERGY_EPSILON).then(|| ((*left - *right) / *left).clamp(0.0, 1.0))
            });
    let all_relative_drops = eigenvalues
        .windows(2)
        .map(|pair| {
            if pair[0] > ENERGY_EPSILON {
                ((pair[0] - pair[1]) / pair[0]).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    let adjacent_relative_drops = all_relative_drops.iter().copied().take(8).collect();
    let largest = all_relative_drops
        .iter()
        .copied()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(&right.1));

    SpectralGaps {
        lambda1_lambda2_absolute,
        lambda1_lambda2_relative,
        adjacent_relative_drops,
        largest_relative_drop: largest.map(|(_, value)| value),
        largest_relative_drop_after_mode: largest.map(|(index, _)| index.saturating_add(1)),
    }
}

/// Returns whether fill values share a known semantic definition. Unknown legacy
/// values and differently defined substrates are deliberately non-comparable.
#[must_use]
pub fn fill_values_are_comparable(left: &SpectralSubstrateV1, right: &SpectralSubstrateV1) -> bool {
    left.is_well_formed()
        && right.is_well_formed()
        && left.substrate_kind == right.substrate_kind
        && left.fill_semantics == right.fill_semantics
        && left.fill_smoothing == right.fill_smoothing
        && left.fill_smoothing_alpha_ppm == right.fill_smoothing_alpha_ppm
        && left.reservoir_dimensions == right.reservoir_dimensions
        && left.covariance_window_samples == right.covariance_window_samples
        && left.fill_semantics != SpectralFillSemanticsV1::UnspecifiedLegacy
        && left.fill_smoothing != SpectralFillSmoothingV1::UnspecifiedLegacy
}

/// Computes inverse-participation concentration without retaining the vector.
#[must_use]
pub fn mode_concentration(components: &[f64]) -> Option<ModeConcentration> {
    if components.is_empty() || components.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let energy = components.iter().map(|value| value * value).sum::<f64>();
    if energy <= ENERGY_EPSILON {
        return None;
    }
    let fourth_moment = components
        .iter()
        .map(|value| {
            let squared = value * value;
            squared * squared
        })
        .sum::<f64>();
    Some(ModeConcentration {
        component_count: components.len(),
        concentration: (fourth_moment / (energy * energy)).clamp(0.0, 1.0),
    })
}

/// Compares at most four corresponding modes using absolute cosine similarity,
/// making the result invariant to arbitrary eigenvector sign flips.
#[must_use]
pub fn mode_turnover(
    previous: &[SpectralMode],
    current: &[SpectralMode],
    degeneracy_relative_tolerance: f64,
) -> ModeTurnoverSummary {
    mode_turnover_with_boundary(previous, current, None, None, degeneracy_relative_tolerance)
}

/// As [`mode_turnover`], while also checking whether the last retained mode is
/// near-degenerate with the first unretained mode. Only that fifth eigenvalue is
/// needed; its eigenvector is never retained.
#[must_use]
pub fn mode_turnover_with_boundary(
    previous: &[SpectralMode],
    current: &[SpectralMode],
    previous_next_eigenvalue: Option<f64>,
    current_next_eigenvalue: Option<f64>,
    degeneracy_relative_tolerance: f64,
) -> ModeTurnoverSummary {
    let compared_mode_count = previous.len().min(current.len()).min(MAX_TRACKED_MODES);
    let tolerance = if degeneracy_relative_tolerance.is_finite() {
        degeneracy_relative_tolerance.clamp(0.0, 1.0)
    } else {
        0.01
    };
    let mut unstable = near_degenerate_modes(previous, compared_mode_count, tolerance);
    unstable.extend(near_degenerate_modes(
        current,
        compared_mode_count,
        tolerance,
    ));
    mark_boundary_degeneracy(
        &mut unstable,
        previous,
        compared_mode_count,
        previous_next_eigenvalue,
        tolerance,
    );
    mark_boundary_degeneracy(
        &mut unstable,
        current,
        compared_mode_count,
        current_next_eigenvalue,
        tolerance,
    );
    unstable.sort_unstable();
    unstable.dedup();

    let per_mode_turnover = previous
        .iter()
        .zip(current.iter())
        .take(compared_mode_count)
        .enumerate()
        .map(|(index, (left, right))| {
            (!unstable.contains(&index))
                .then(|| sign_invariant_turnover(&left.components, &right.components))
                .flatten()
        })
        .collect::<Vec<_>>();
    unstable.extend(
        per_mode_turnover
            .iter()
            .enumerate()
            .filter_map(|(index, turnover)| turnover.is_none().then_some(index)),
    );
    unstable.sort_unstable();
    unstable.dedup();
    let usable = per_mode_turnover
        .iter()
        .filter_map(|value| *value)
        .collect::<Vec<_>>();
    let mean = (!usable.is_empty()).then(|| usable.iter().sum::<f64>() / usable.len() as f64);
    let maximum = usable.iter().copied().max_by(f64::total_cmp);

    ModeTurnoverSummary {
        compared_mode_count,
        mean_sign_invariant_turnover: mean,
        maximum_sign_invariant_turnover: maximum,
        per_mode_turnover,
        identity_stable: unstable.is_empty() && compared_mode_count > 0,
        identity_unstable_modes: unstable,
        degeneracy_relative_tolerance: tolerance,
    }
}

fn mark_boundary_degeneracy(
    unstable: &mut Vec<usize>,
    modes: &[SpectralMode],
    compared_mode_count: usize,
    next_eigenvalue: Option<f64>,
    tolerance: f64,
) {
    if compared_mode_count != MAX_TRACKED_MODES || modes.len() < compared_mode_count {
        return;
    }
    let Some(next) = next_eigenvalue else {
        return;
    };
    let retained = modes[compared_mode_count.saturating_sub(1)].eigenvalue;
    if !retained.is_finite() || !next.is_finite() {
        unstable.push(compared_mode_count.saturating_sub(1));
        return;
    }
    let scale = retained.abs().max(next.abs()).max(ENERGY_EPSILON);
    if (retained - next).abs() / scale <= tolerance {
        unstable.push(compared_mode_count.saturating_sub(1));
    }
}

fn near_degenerate_modes(
    modes: &[SpectralMode],
    compared_mode_count: usize,
    tolerance: f64,
) -> Vec<usize> {
    let mut unstable = Vec::new();
    for (index, pair) in modes[..compared_mode_count].windows(2).enumerate() {
        if !pair[0].eigenvalue.is_finite() || !pair[1].eigenvalue.is_finite() {
            unstable.push(index);
            unstable.push(index.saturating_add(1));
            continue;
        }
        let scale = pair[0]
            .eigenvalue
            .abs()
            .max(pair[1].eigenvalue.abs())
            .max(ENERGY_EPSILON);
        let relative_gap = (pair[0].eigenvalue - pair[1].eigenvalue).abs() / scale;
        if relative_gap <= tolerance {
            unstable.push(index);
            unstable.push(index.saturating_add(1));
        }
    }
    unstable
}

fn sign_invariant_turnover(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.is_empty()
        || left.len() != right.len()
        || left.iter().chain(right).any(|value| !value.is_finite())
    {
        return None;
    }
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm <= ENERGY_EPSILON || right_norm <= ENERGY_EPSILON {
        return None;
    }
    Some((1.0 - (dot / (left_norm * right_norm)).abs().clamp(0.0, 1.0)).clamp(0.0, 1.0))
}
