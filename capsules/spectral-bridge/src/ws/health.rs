#[derive(Clone, Copy, Debug)]
enum WsLane {
    Telemetry,
    Sensory,
}

impl WsLane {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Telemetry => "telemetry",
            Self::Sensory => "sensory",
        }
    }
}

fn unix_now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn bridge_age_and_future_skew_ms(now_s: f64, observed_at_s: f64) -> (Option<f64>, Option<f64>) {
    let delta_ms = (now_s - observed_at_s) * 1000.0;
    if delta_ms < 0.0 {
        (Some(0.0), Some((-delta_ms).round()))
    } else {
        (Some(delta_ms.round()), None)
    }
}

fn bridge_clock_skew_state(
    telemetry_future_skew_ms: Option<f64>,
    sensory_future_skew_ms: Option<f64>,
) -> &'static str {
    match (
        telemetry_future_skew_ms.is_some(),
        sensory_future_skew_ms.is_some(),
    ) {
        (true, true) => "both_lanes_future_timestamp_visible",
        (true, false) => "telemetry_future_timestamp_visible",
        (false, true) => "sensory_future_timestamp_visible",
        (false, false) => "none",
    }
}

fn bridge_age_is_recent(age_ms: Option<f64>) -> bool {
    age_ms.is_some_and(|age| age <= BRIDGE_RECIPROCITY_RECENT_WINDOW_MS)
}

fn bridge_age_is_stale(age_ms: Option<f64>, stale_window_ms: f64) -> bool {
    age_ms.is_some_and(|age| age > stale_window_ms)
}

fn bridge_dynamic_stale_window_ms(telemetry: Option<&SpectralTelemetry>) -> (f64, &'static str) {
    let Some(telemetry) = telemetry else {
        return (
            BRIDGE_RECIPROCITY_STALE_WINDOW_MS,
            "fixed_default_no_telemetry_context",
        );
    };
    let pressure_risk = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.pressure_risk.clamp(0.0, 1.0));
    let porosity = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let semantic_viscosity = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| resonance.components.viscosity_index.clamp(0.0, 1.0));
    let entropy = telemetry
        .typed_fingerprint()
        .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));

    if pressure_risk.is_some_and(|pressure| pressure >= 0.20)
        && porosity.is_some_and(|value| value <= 0.35)
    {
        (
            BRIDGE_RECIPROCITY_PRESSURE_POROSITY_STALE_WINDOW_MS,
            "pressure_high_porosity_low_reflective_silence",
        )
    } else if pressure_risk.is_some_and(|pressure| pressure >= 0.20)
        && semantic_viscosity.is_some_and(|value| value >= 0.60)
    {
        (
            BRIDGE_RECIPROCITY_VISCOSITY_REFLECTIVE_STALE_WINDOW_MS,
            "pressure_high_semantic_viscosity_reflective_silence",
        )
    } else if pressure_risk.is_some_and(|pressure| pressure >= 0.20)
        && entropy.is_some_and(|value| value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT)
    {
        (
            BRIDGE_RECIPROCITY_ENTROPY_REFLECTIVE_STALE_WINDOW_MS,
            "pressure_high_entropy_reflective_silence",
        )
    } else {
        (
            BRIDGE_RECIPROCITY_STALE_WINDOW_MS,
            "fixed_default_context_does_not_extend_stale_window",
        )
    }
}

fn bridge_entropy_contract_preview_window_ms(
    spectral_entropy: Option<f32>,
    resonance_cohesion_score: Option<f32>,
) -> Option<f64> {
    let spectral_entropy = spectral_entropy?;
    if spectral_entropy < 0.85 {
        return None;
    }

    let cohesion = resonance_cohesion_score.unwrap_or(0.50);
    if spectral_entropy >= 0.90 && cohesion <= 0.55 {
        Some(BRIDGE_RECIPROCITY_ENTROPY_CONTRACT_PREVIEW_WINDOW_MS)
    } else if cohesion <= 0.45 {
        Some(BRIDGE_RECIPROCITY_STALE_WINDOW_MS)
    } else {
        None
    }
}

fn pressure_gradient_delta_from_trend(trend: &PressureTrendV1) -> Option<(f32, &'static str)> {
    match (trend.pressure_delta, trend.mode_packing_delta) {
        (Some(pressure_delta), Some(mode_packing_delta))
            if mode_packing_delta.abs() > pressure_delta.abs() =>
        {
            Some((
                mode_packing_delta,
                "bridge_pressure_trend_v1.mode_packing_delta",
            ))
        },
        (Some(pressure_delta), _) => {
            Some((pressure_delta, "bridge_pressure_trend_v1.pressure_delta"))
        },
        (None, Some(mode_packing_delta)) => Some((
            mode_packing_delta,
            "bridge_pressure_trend_v1.mode_packing_delta",
        )),
        (None, None) => None,
    }
}

fn round_flux_delta(value: f32) -> f32 {
    (value.clamp(-100.0, 100.0) * 10_000.0).round() / 10_000.0
}

fn silt_noise_separation_v1(
    high_entropy: &PressureTrendSampleV1,
    low_entropy: &PressureTrendSampleV1,
) -> Option<SiltNoiseSeparationV1> {
    let high_entropy_value = high_entropy.spectral_entropy?;
    let low_entropy_value = low_entropy.spectral_entropy?;
    let high_mode = high_entropy.mode_packing?;
    let low_mode = low_entropy.mode_packing?;
    let mode_packing_delta = round_flux_delta(high_mode - low_mode).abs();
    let high_density = high_entropy
        .structural_density
        .unwrap_or(high_mode)
        .clamp(0.0, 1.0);
    let high_friction = high_entropy
        .semantic_friction
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let semantic_trickle = high_entropy
        .semantic_trickle
        .or(low_entropy.semantic_trickle)
        .map(|value| value.clamp(0.0, 1.0));
    let low_porosity_weight = high_entropy
        .porosity_gradient
        .map_or(0.0, |porosity| (1.0 - porosity.clamp(0.0, 1.0)) * 0.16);
    let low_semantic_trickle = semantic_trickle.is_some_and(|value| value <= 0.02);
    let semantic_signal_present = semantic_trickle.is_some_and(|value| value >= 0.08);
    let low_semantic_weight = if low_semantic_trickle { 0.08 } else { 0.0 };
    let persistence_weight = (1.0 - (mode_packing_delta * 10.0).clamp(0.0, 1.0)) * 0.18;
    let contextual_resonance_score = round_flux_delta(
        (high_mode * 0.26)
            + (high_density * 0.22)
            + (high_friction * 0.18)
            + low_porosity_weight
            + low_semantic_weight
            + persistence_weight,
    )
    .clamp(0.0, 1.0);
    let dynamic_high_mode_threshold = round_flux_delta(
        (0.45
            - low_porosity_weight * 0.35
            - high_friction * 0.04
            - if low_semantic_trickle { 0.05 } else { 0.0 })
        .clamp(0.32, 0.45),
    );
    let heritage_preservation_state = if contextual_resonance_score >= 0.55
        && mode_packing_delta <= 0.04
    {
        "contextual_resonance_preserve_as_heritage"
    } else if high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT && high_mode < 0.35 {
        "high_entropy_noise_watch"
    } else {
        "contextual_resonance_insufficient_for_heritage"
    };
    let silt_signal_state = if semantic_signal_present {
        "semantic_signal_present_review"
    } else if low_semantic_trickle && high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
    {
        "low_semantic_trickle_noise_or_silt"
    } else if semantic_trickle.is_some() {
        "semantic_trickle_low_review"
    } else {
        "semantic_trickle_unknown"
    };
    let interpretation = if mode_packing_delta <= 0.03
        && high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
        && low_entropy_value <= 0.50
        && high_mode >= dynamic_high_mode_threshold
        && low_mode >= dynamic_high_mode_threshold
    {
        "mode_packing_silt_persists_across_entropy"
    } else if high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
        && low_entropy_value <= 0.50
        && high_mode < dynamic_high_mode_threshold
        && low_semantic_trickle
    {
        "high_entropy_low_semantic_trickle_noise"
    } else if high_entropy_value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT
        && low_entropy_value <= 0.50
        && high_mode < dynamic_high_mode_threshold
        && low_mode < dynamic_high_mode_threshold
    {
        "entropy_cascade_more_likely_than_overpacked_silt"
    } else {
        "insufficient_contrast_for_silt_noise_separation"
    };
    Some(SiltNoiseSeparationV1 {
        policy: "silt_noise_separation_v1",
        high_entropy: high_entropy_value,
        low_entropy: low_entropy_value,
        high_entropy_mode_packing: high_mode,
        low_entropy_mode_packing: low_mode,
        mode_packing_delta,
        semantic_trickle,
        dynamic_high_mode_threshold,
        contextual_resonance_score,
        contextual_resonance_basis: "mode_density_semantic_friction_porosity_semantic_trickle_persistence_v2",
        heritage_preservation_state,
        interpretation,
        silt_signal_state,
        porosity_change_authority: "diagnostic_only_porosity_change_requires_operator_approval",
    })
}
