use astrid_minime_protocol::{
    EigenPacketV1, SpectralFillSemanticsV1, SpectralFillSmoothingV1, SpectralSubstrateKindV1,
    SpectralSubstrateV1, SpectrumCoverageV1,
};

#[test]
fn legacy_packet_decodes_without_claiming_fill_semantics() {
    let packet: EigenPacketV1 = serde_json::from_value(serde_json::json!({
        "t_ms": 42,
        "eigenvalues": [3.0, 2.0, 1.0],
        "fill_ratio": 0.68
    }))
    .expect("legacy packet remains decodable");

    assert!(packet.spectral_substrate_v1.is_none());
    let encoded = serde_json::to_value(packet).expect("legacy packet remains encodable");
    assert!(encoded.get("spectral_substrate_v1").is_none());
}

#[test]
fn cpu_edge_substrate_round_trips_with_explicit_fill_meaning() {
    let substrate = SpectralSubstrateV1::cpu_edge_covariance_effective_rank(128, 256, 180_000);
    assert!(substrate.is_well_formed());
    assert_eq!(
        substrate.substrate_kind,
        SpectralSubstrateKindV1::CpuEdgeCovarianceEffectiveRank
    );
    assert_eq!(
        substrate.fill_semantics,
        SpectralFillSemanticsV1::NormalizedCovarianceEffectiveRank
    );
    assert_eq!(
        substrate.fill_smoothing,
        SpectralFillSmoothingV1::ExponentialMovingAverage
    );
    assert_eq!(substrate.fill_smoothing_alpha_ppm, Some(180_000));

    let encoded = serde_json::to_value(&substrate).expect("substrate encodes");
    assert_eq!(
        encoded["substrate_kind"],
        "cpu_edge_covariance_effective_rank"
    );
    assert_eq!(
        encoded["fill_semantics"],
        "normalized_covariance_effective_rank"
    );
    assert_eq!(encoded["fill_smoothing"], "exponential_moving_average");
    assert_eq!(
        serde_json::from_value::<SpectralSubstrateV1>(encoded).expect("substrate decodes"),
        substrate
    );
}

#[test]
fn pre_smoothing_cpu_edge_substrate_remains_readable_but_explicitly_legacy() {
    let mut encoded = serde_json::to_value(
        SpectralSubstrateV1::cpu_edge_covariance_effective_rank(128, 256, 180_000),
    )
    .unwrap();
    let object = encoded.as_object_mut().unwrap();
    object.remove("fill_smoothing");
    object.remove("fill_smoothing_alpha_ppm");
    let decoded = serde_json::from_value::<SpectralSubstrateV1>(encoded).unwrap();
    assert!(decoded.is_well_formed());
    assert_eq!(
        decoded.fill_smoothing,
        SpectralFillSmoothingV1::UnspecifiedLegacy
    );
    assert_eq!(decoded.fill_smoothing_alpha_ppm, None);
}

#[test]
fn inconsistent_substrate_pair_is_visible_as_malformed() {
    let mut substrate = SpectralSubstrateV1::minime_thresholded_eigenfill(Some(128));
    substrate.fill_semantics = SpectralFillSemanticsV1::NormalizedCovarianceEffectiveRank;
    assert!(!substrate.is_well_formed());
}

#[test]
fn spectrum_coverage_distinguishes_exported_prefix_from_full_denominator() {
    let coverage = SpectrumCoverageV1 {
        full_spectrum_mode_count: 128,
        exported_spectrum_mode_count: 16,
        usable_spectrum_mode_count: Some(128),
        discarded_non_finite_mode_count: 0,
        clamped_negative_mode_count: 0,
        exported_spectrum_energy_ratio: Some(0.93),
        denominator_uses_full_spectrum: true,
    };

    assert!(coverage.is_well_formed());
    assert!(!coverage.exports_full_spectrum());

    let dishonest_full_denominator = SpectrumCoverageV1 {
        discarded_non_finite_mode_count: 1,
        ..coverage
    };
    assert!(!dishonest_full_denominator.is_well_formed());
}
