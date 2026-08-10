//! Pure, deterministic spectral derivations shared by CPU-edge Astrid components.
//!
//! This crate deliberately owns no filesystem, network, clock, model, control,
//! or linear-algebra runtime. Callers supply spectra, transient mode vectors,
//! timestamps, and provenance identifiers; the crate returns bounded summaries.

#![deny(unsafe_code)]

mod evidence;
mod spectrum;
mod temporal;

pub use evidence::{
    CorrelationAttributionV1, CorrelationEvidenceV1, EvidenceError,
    NON_CAUSAL_SPECTRAL_EVIDENCE_POLICY_V1, NonCausalSpectralEvidenceV1,
};
pub use spectrum::{
    MAX_TRACKED_MODES, ModeConcentration, ModeTurnoverSummary, SanitizedSpectrum,
    SpectralEnergyShares, SpectralGaps, SpectralMetrics, SpectralMode, SpectrumBasis,
    SpectrumCoverage, fill_values_are_comparable, mode_concentration, mode_turnover,
    mode_turnover_with_boundary, sanitize_spectrum,
};
pub use temporal::{
    CorrelationSummary, CrossCorrelationPoint, MAX_CROSS_CORRELATION_LAG, RollingSpectralSummary,
    ScalarSummary, TimedScalar, TimedSpectralObservation, cross_correlation, pearson_correlation,
    rolling_spectral_summary, summarize_scalars, summarize_timed_scalars,
};
