use astrid_minime_protocol::SpectralSubstrateV1;
use astrid_spectral_core::{
    CorrelationAttributionV1, CorrelationEvidenceV1, NonCausalSpectralEvidenceV1, SpectralMode,
    SpectrumBasis, TimedScalar, TimedSpectralObservation, cross_correlation,
    fill_values_are_comparable, mode_concentration, mode_turnover, pearson_correlation,
    rolling_spectral_summary, sanitize_spectrum, summarize_timed_scalars,
};

fn observation(t_ms: u64, values: &[f32], turnover: Option<f64>) -> TimedSpectralObservation {
    TimedSpectralObservation {
        t_ms,
        metrics: sanitize_spectrum(values, Some(values.len()))
            .metrics()
            .expect("non-zero spectrum"),
        mode_turnover: turnover,
        mode_identity_stable: Some(true),
    }
}

#[test]
fn sanitation_is_explicit_and_partial_coverage_is_not_upgraded() {
    let spectrum = sanitize_spectrum(&[1.0, f32::NAN, -0.01, 3.0], Some(128));

    assert_eq!(spectrum.eigenvalues, vec![3.0, 1.0, 0.0]);
    assert_eq!(spectrum.discarded_non_finite_count, 1);
    assert_eq!(spectrum.clamped_negative_count, 1);
    assert_eq!(spectrum.coverage.exported_mode_fraction, Some(4.0 / 128.0));
    assert_eq!(spectrum.basis(), SpectrumBasis::ExportedSpectrumPrefix);

    let inconsistent = sanitize_spectrum(&[3.0, 2.0, 1.0], Some(2));
    assert_eq!(
        inconsistent.basis(),
        SpectrumBasis::ExportedSpectrumWithInconsistentCoverage
    );
    assert!(inconsistent.coverage.exported_mode_fraction.is_none());
}

#[test]
fn entropy_shares_gaps_and_density_are_deterministic() {
    let flat = sanitize_spectrum(&[1.0, 1.0, 1.0, 1.0], Some(4))
        .metrics()
        .expect("flat metrics");
    assert!((flat.normalized_entropy - 1.0).abs() < 1.0e-12);
    assert!((flat.effective_modes - 4.0).abs() < 1.0e-12);
    assert!((flat.energy_shares.head - 0.25).abs() < 1.0e-12);
    assert!((flat.energy_shares.shoulder - 0.5).abs() < 1.0e-12);
    assert!((flat.energy_shares.tail - 0.25).abs() < 1.0e-12);
    assert_eq!(flat.density_gradient, Some(0.0));

    let steep = sanitize_spectrum(&[10.0, 0.5, 0.1, 0.05], Some(4))
        .metrics()
        .expect("steep metrics");
    assert!(steep.lambda1_share > 0.9);
    assert!(steep.density_gradient.unwrap() > flat.density_gradient.unwrap());
    assert!(steep.gaps.lambda1_lambda2_relative.unwrap() > 0.9);
}

#[test]
fn fill_comparability_requires_the_same_known_semantics() {
    let edge = SpectralSubstrateV1::cpu_edge_covariance_effective_rank(128, 256);
    let edge_peer = SpectralSubstrateV1::cpu_edge_covariance_effective_rank(128, 512);
    let minime = SpectralSubstrateV1::minime_thresholded_eigenfill(Some(128));

    assert!(fill_values_are_comparable(&edge, &edge_peer));
    assert!(!fill_values_are_comparable(&edge, &minime));
}

#[test]
fn mode_turnover_ignores_sign_and_marks_degenerate_identity() {
    let previous = vec![
        SpectralMode {
            eigenvalue: 3.0,
            components: vec![1.0, 0.0, 0.0],
        },
        SpectralMode {
            eigenvalue: 2.0,
            components: vec![0.0, 1.0, 0.0],
        },
    ];
    let sign_flipped = vec![
        SpectralMode {
            eigenvalue: 3.0,
            components: vec![-1.0, 0.0, 0.0],
        },
        SpectralMode {
            eigenvalue: 2.0,
            components: vec![0.0, -1.0, 0.0],
        },
    ];
    let turnover = mode_turnover(&previous, &sign_flipped, 0.01);
    assert_eq!(turnover.mean_sign_invariant_turnover, Some(0.0));
    assert!(turnover.identity_stable);

    let mut degenerate = sign_flipped;
    degenerate[1].eigenvalue = 2.99;
    let turnover = mode_turnover(&previous, &degenerate, 0.01);
    assert!(!turnover.identity_stable);
    assert_eq!(turnover.identity_unstable_modes, vec![0, 1]);
}

#[test]
fn concentration_is_bounded_and_rejects_invalid_vectors() {
    let concentrated = mode_concentration(&[1.0, 0.0, 0.0]).expect("concentration");
    let distributed = mode_concentration(&[1.0, 1.0, 1.0]).expect("concentration");
    assert!((concentrated.concentration - 1.0).abs() < 1.0e-12);
    assert!((distributed.concentration - 1.0 / 3.0).abs() < 1.0e-12);
    assert!(mode_concentration(&[f64::NAN]).is_none());
}

#[test]
fn rolling_summaries_and_correlations_preserve_direction() {
    let timed = [
        TimedScalar {
            t_ms: 0,
            value: 1.0,
        },
        TimedScalar {
            t_ms: 1_000,
            value: 2.0,
        },
        TimedScalar {
            t_ms: 2_000,
            value: 3.0,
        },
    ];
    let summary = summarize_timed_scalars(&timed).expect("summary");
    assert!((summary.slope_per_second.unwrap() - 1.0).abs() < 1.0e-12);

    let correlation = pearson_correlation(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]).expect("correlation");
    assert!((correlation.coefficient + 1.0).abs() < 1.0e-12);
    assert!(pearson_correlation(&[1.0, 1.0], &[2.0, 3.0]).is_none());

    let cross = cross_correlation(&[0.0, 1.0, 2.0, 3.0], &[9.0, 0.0, 1.0, 2.0], 1);
    assert!(cross.iter().any(|point| point.lag_samples == 1));

    let rolling = rolling_spectral_summary(&[
        observation(0, &[4.0, 2.0, 1.0, 0.5], Some(0.1)),
        observation(60_000, &[3.0, 2.0, 1.0, 1.0], Some(0.2)),
    ])
    .expect("rolling summary");
    assert_eq!(rolling.sample_count, 2);
    assert_eq!(rolling.full_spectrum_sample_count, 2);
    assert_eq!(rolling.mode_turnover.as_ref().unwrap().count, 2);
}

#[test]
fn evidence_is_non_causal_hash_bound_and_tamper_evident() {
    let summary = rolling_spectral_summary(&[
        observation(1_000, &[4.0, 2.0, 1.0, 0.5], Some(0.1)),
        observation(61_000, &[3.0, 2.0, 1.0, 1.0], Some(0.2)),
    ])
    .expect("rolling summary");
    let correlation =
        pearson_correlation(&[0.67, 0.68, 0.69], &[0.2, 0.3, 0.4]).expect("correlation");
    let substrate = SpectralSubstrateV1::cpu_edge_covariance_effective_rank(128, 256);
    let second_summary = summary.clone();
    let second_correlation = correlation.clone();
    let second_substrate = substrate.clone();
    let evidence = NonCausalSpectralEvidenceV1::new(
        "Does tail share co-vary with exact traced Action outcomes?",
        substrate,
        summary,
        Some(CorrelationEvidenceV1 {
            left_series: "tail_share".to_string(),
            right_series: "action_outcome_rate".to_string(),
            correlation,
            attribution: CorrelationAttributionV1::ExactIdentifierJoin,
        }),
        vec!["a".repeat(64), "b".repeat(64)],
    )
    .expect("valid evidence");

    assert!(evidence.is_well_formed());
    assert_eq!(evidence.evidence_sha256.len(), 64);
    assert!(evidence.authority.contains("non_causal"));

    let duplicate = NonCausalSpectralEvidenceV1::new(
        "Does tail share co-vary with exact traced Action outcomes?",
        second_substrate,
        second_summary,
        Some(CorrelationEvidenceV1 {
            left_series: "tail_share".to_string(),
            right_series: "action_outcome_rate".to_string(),
            correlation: second_correlation,
            attribution: CorrelationAttributionV1::ExactIdentifierJoin,
        }),
        vec!["b".repeat(64), "a".repeat(64)],
    )
    .expect("same evidence with reordered provenance");
    assert_eq!(duplicate.evidence_sha256, evidence.evidence_sha256);
    let mut tampered = evidence;
    tampered.question.push_str(" changed");
    assert!(!tampered.is_well_formed());
}
