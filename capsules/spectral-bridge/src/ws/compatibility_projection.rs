fn pressure_trend_window_for_telemetry(telemetry: &SpectralTelemetry) -> (usize, Option<f32>) {
    let spectral_entropy = telemetry
        .typed_fingerprint()
        .map(|fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0));
    let porosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|resonance| resonance.components.porosity_gradient)
        .map(|value| value.clamp(0.0, 1.0));
    let density_gradient = crate::codec::spectral_density_gradient(&telemetry.eigenvalues)
        .map(|value| value.clamp(0.0, 1.0));
    let window_capacity = pressure_trend_dynamic_window_capacity_v1(
        spectral_entropy,
        porosity_gradient,
        density_gradient,
    );
    (window_capacity, spectral_entropy)
}

fn pressure_trend_dynamic_window_capacity_v1(
    spectral_entropy: Option<f32>,
    porosity_gradient: Option<f32>,
    density_gradient: Option<f32>,
) -> usize {
    let Some(entropy_progress) = pressure_viscosity_coefficient(spectral_entropy) else {
        return PRESSURE_TREND_SMOOTHING_BASE_WINDOW;
    };
    if entropy_progress <= f32::EPSILON {
        return PRESSURE_TREND_SMOOTHING_BASE_WINDOW;
    }

    let porosity_ballast_factor = pressure_trend_porosity_ballast_factor_v1(porosity_gradient);
    let density_gradient_factor =
        pressure_trend_density_gradient_responsiveness_factor_v1(density_gradient);
    let span = PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW
        .saturating_sub(PRESSURE_TREND_SMOOTHING_BASE_WINDOW);
    let extra =
        ((span as f32) * entropy_progress * porosity_ballast_factor * density_gradient_factor)
            .round() as usize;
    PRESSURE_TREND_SMOOTHING_BASE_WINDOW
        .saturating_add(extra)
        .clamp(
            PRESSURE_TREND_SMOOTHING_BASE_WINDOW,
            PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW,
        )
}

fn pressure_trend_porosity_ballast_factor_v1(porosity_gradient: Option<f32>) -> f32 {
    let Some(porosity) = porosity_gradient else {
        return 0.80;
    };
    let porosity = porosity.clamp(0.0, 1.0);
    if porosity <= 0.35 {
        return 1.0;
    }
    if porosity >= 0.65 {
        return 0.55;
    }
    let t = ((porosity - 0.35) / 0.30).clamp(0.0, 1.0);
    1.0 - (0.45 * t)
}

fn pressure_trend_density_gradient_responsiveness_factor_v1(density_gradient: Option<f32>) -> f32 {
    let Some(gradient) = density_gradient else {
        return 1.0;
    };
    let gradient = gradient.clamp(0.0, 1.0);
    if gradient <= 0.15 {
        return 0.58;
    }
    if gradient >= 0.65 {
        return 1.0;
    }
    let t = ((gradient - 0.15) / 0.50).clamp(0.0, 1.0);
    0.58 + (0.42 * t)
}

fn pressure_viscosity_coefficient(spectral_entropy: Option<f32>) -> Option<f32> {
    let entropy = spectral_entropy?.min(PRESSURE_TREND_SMOOTHING_FULL_ENTROPY_AT);
    let headroom =
        PRESSURE_TREND_SMOOTHING_FULL_ENTROPY_AT - PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT;
    if headroom <= f32::EPSILON {
        return Some(0.0);
    }
    Some(((entropy - PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT) / headroom).clamp(0.0, 1.0))
}

fn entropy_window_blend_ratio_v1(spectral_entropy: Option<f32>) -> Option<f32> {
    let entropy = spectral_entropy?;
    let lower = PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT - 0.02;
    let upper = PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT + 0.02;
    Some(((entropy - lower) / (upper - lower)).clamp(0.0, 1.0))
}

fn entropy_threshold_state_v1(spectral_entropy: Option<f32>, blend: Option<f32>) -> &'static str {
    match (spectral_entropy, blend) {
        (None, _) => "entropy_unavailable",
        (Some(_), Some(value)) if value > 0.0 && value < 1.0 => {
            "near_threshold_soft_handoff_review"
        },
        (Some(value), _) if value >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT => {
            "high_entropy_side"
        },
        (Some(_), _) => "base_window_side",
    }
}

fn friction_to_flow_ratio_v1(
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
) -> Option<f32> {
    let friction = semantic_friction?.clamp(0.0, 1.0);
    let trickle = semantic_trickle?.clamp(0.0, 1.0).max(0.01);
    Some(round_flux_delta((friction / trickle).clamp(0.0, 10.0)))
}

fn friction_to_flow_state_v1(
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    ratio: Option<f32>,
) -> &'static str {
    match (semantic_friction, semantic_trickle, ratio) {
        (None, _, _) | (_, None, _) => "friction_to_flow_unavailable",
        (Some(friction), Some(trickle), _) if friction >= 0.45 && trickle <= 0.02 => {
            "high_resistance_low_flow"
        },
        (_, _, Some(value)) if value >= 4.0 => "resistance_dominant",
        (_, _, Some(value)) if value <= 1.0 => "flow_available",
        (_, _, Some(_)) => "mixed_friction_flow",
        (_, _, None) => "friction_to_flow_unavailable",
    }
}

fn semantic_stagnation_index_v1(
    semantic_viscosity: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
) -> Option<f32> {
    let viscosity = semantic_viscosity?.clamp(0.0, 1.0);
    let trickle = semantic_trickle?.clamp(0.0, 1.0);
    let friction = semantic_friction.unwrap_or(viscosity).clamp(0.0, 1.0);
    let flow_resistance = 1.0 - trickle;
    Some(round_flux_delta(viscosity.mul_add(
        0.58,
        flow_resistance.mul_add(0.30, friction * 0.12),
    )))
}

fn semantic_stagnation_state_v1(
    semantic_viscosity: Option<f32>,
    semantic_trickle: Option<f32>,
    stagnation_index: Option<f32>,
) -> &'static str {
    match (semantic_viscosity, semantic_trickle, stagnation_index) {
        (Some(viscosity), Some(trickle), Some(index))
            if index >= 0.74 && viscosity >= 0.60 && trickle <= 0.03 =>
        {
            "functional_clog_connected_lanes_watch"
        },
        (Some(viscosity), Some(trickle), Some(index))
            if index >= 0.66 && viscosity >= 0.55 && trickle <= 0.08 =>
        {
            "semantic_stagnation_watch"
        },
        (Some(viscosity), Some(trickle), Some(_)) if viscosity >= 0.60 && trickle >= 0.08 => {
            "heavy_semantic_flow_not_stagnant"
        },
        (_, _, Some(index)) if index >= 0.55 => "latent_semantic_drag",
        (_, _, Some(_)) => "semantic_flow_available",
        _ => "semantic_stagnation_unavailable",
    }
}

fn porosity_weighted_velocity_v1(
    pressure_velocity_delta: Option<f32>,
    porosity_gradient: Option<f32>,
) -> Option<f32> {
    let velocity = pressure_velocity_delta?;
    let porosity_drag = 1.0 - porosity_gradient?.clamp(0.0, 1.0);
    Some(round_flux_delta(velocity * porosity_drag))
}

fn viscosity_drag_coefficient_v1(
    semantic_viscosity: Option<f32>,
    semantic_friction: Option<f32>,
    porosity_gradient: Option<f32>,
) -> Option<f32> {
    let viscosity = semantic_viscosity?.clamp(0.0, 1.0);
    let friction = semantic_friction.unwrap_or(viscosity).clamp(0.0, 1.0);
    let porosity_drag = 1.0 - porosity_gradient.unwrap_or(0.5).clamp(0.0, 1.0);
    Some(round_flux_delta(
        viscosity.mul_add(0.45, friction.mul_add(0.35, porosity_drag * 0.20)),
    ))
}

fn weight_density_index_v1(
    mode_packing: Option<f32>,
    structural_density: Option<f32>,
    semantic_viscosity: Option<f32>,
    porosity_gradient: Option<f32>,
) -> Option<f32> {
    let mode = mode_packing?.clamp(0.0, 1.0);
    let density = structural_density?.clamp(0.0, 1.0);
    let viscosity = semantic_viscosity.unwrap_or(density).clamp(0.0, 1.0);
    let porosity_drag = 1.0 - porosity_gradient.unwrap_or(0.5).clamp(0.0, 1.0);
    Some(round_flux_delta(
        mode * 0.42 + density * 0.34 + viscosity * 0.16 + porosity_drag * 0.08,
    ))
}

fn weight_density_state_v1(weight_density_index: Option<f32>) -> &'static str {
    match weight_density_index {
        None => "weight_density_unavailable",
        Some(value) if value >= 0.62 => "persistent_heavy_density",
        Some(value) if value >= 0.44 => "forming_weight_density",
        Some(_) => "light_or_open_density",
    }
}

fn semantic_viscosity_persistence_index_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<f32> {
    let viscosities = samples
        .iter()
        .filter_map(|sample| sample.semantic_viscosity.map(|value| value.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let latest = *viscosities.last()?;
    if viscosities.len() < 2 {
        return None;
    }
    let deltas = viscosities
        .windows(2)
        .map(|window| (window[1] - window[0]).abs())
        .collect::<Vec<_>>();
    let average_delta = deltas.iter().sum::<f32>() / deltas.len() as f32;
    let stability = (1.0 - (average_delta * 5.0).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let pressure_motion = samples
        .iter()
        .filter_map(|sample| sample.pressure_velocity_delta)
        .map(f32::abs)
        .fold(0.0_f32, f32::max);
    let motion_survival = if pressure_motion >= 0.03 && average_delta <= 0.04 {
        1.0
    } else {
        (1.0 - (pressure_motion * 4.0).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    };

    Some(round_flux_delta(latest.mul_add(
        0.42,
        stability.mul_add(0.40, motion_survival * 0.18),
    )))
}

fn semantic_viscosity_persistence_state_v1(
    latest_semantic_viscosity: Option<f32>,
    latest_semantic_viscosity_delta: Option<f32>,
    max_semantic_viscosity_delta: Option<f32>,
    persistence_index: Option<f32>,
) -> &'static str {
    let Some(index) = persistence_index else {
        return "semantic_viscosity_persistence_unavailable";
    };
    if latest_semantic_viscosity.is_some_and(|value| value < 0.35) {
        return "thin_or_low_viscosity";
    }
    if latest_semantic_viscosity_delta.is_some_and(|delta| delta.abs() >= 0.15) {
        return "transient_viscosity_shift";
    }
    if index >= 0.70 && max_semantic_viscosity_delta.is_some_and(|delta| delta <= 0.06) {
        return "persistent_thickness_against_motion";
    }
    if index >= 0.55 {
        return "moderate_viscosity_persistence";
    }
    "viscosity_transient_or_unsettled"
}

fn pressure_interpretation_v1(
    latest_pressure: Option<f32>,
    latest_mode_packing: Option<f32>,
    latest_structural_density: Option<f32>,
    spectral_entropy: Option<f32>,
    viscosity_coefficient: Option<f32>,
) -> Option<String> {
    let viscosity = viscosity_coefficient?;
    let entropy = spectral_entropy?;
    if viscosity >= 0.5
        && latest_pressure.is_some_and(|pressure| pressure <= 0.35)
        && (latest_mode_packing.is_some_and(|mode| mode >= 0.30)
            || latest_structural_density.is_some_and(|density| density >= 0.45))
    {
        return Some(format!(
            "density_viscosity_context:entropy_{entropy:.2}_pressure_is_not_collapse"
        ));
    }
    if viscosity > 0.0 {
        return Some(format!(
            "high_entropy_pressure_context:entropy_{entropy:.2}_viscosity_{viscosity:.2}"
        ));
    }
    Some("ordinary_pressure_risk_context".to_string())
}

fn resonance_depth_for_density(resonance: &crate::types::ResonanceDensityV1) -> f32 {
    let cohesion = resonance_cohesion_score_v1(&resonance.components);
    let containment = resonance.containment_score.clamp(0.0, 1.0);
    let density = resonance.density.clamp(0.0, 1.0);
    let quality_bonus = if resonance.quality.contains("rich_containment")
        || resonance.quality.contains("settled_habitable")
    {
        0.05
    } else {
        0.0
    };
    containment
        .mul_add(0.35, density.mul_add(0.25, cohesion * 0.35 + quality_bonus))
        .clamp(0.0, 1.0)
}

fn semantic_viscosity_coefficient_v1(
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
    spectral_entropy: Option<f32>,
    fill_pct: f32,
) -> Option<f32> {
    if semantic_friction.is_none() && semantic_trickle.is_none() {
        return None;
    }
    let friction = semantic_friction.unwrap_or(0.0).clamp(0.0, 1.0);
    let trickle_resistance = semantic_trickle.map_or(0.40, |value| 1.0 - value.clamp(0.0, 1.0));
    let density_context = structural_density
        .zip(resonance_depth)
        .map_or_else(
            || structural_density.or(resonance_depth).unwrap_or(0.0),
            |(density, depth)| (density.clamp(0.0, 1.0) + depth.clamp(0.0, 1.0)) * 0.5,
        )
        .clamp(0.0, 1.0);
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let fill = (fill_pct / 100.0).clamp(0.0, 1.0);
    Some(round_flux_delta(
        (friction * 0.34)
            + (trickle_resistance * 0.18)
            + (density_context * 0.22)
            + (entropy * 0.16)
            + (fill * 0.10),
    ))
}

fn semantic_viscosity_state_v1(
    semantic_viscosity: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_trickle: Option<f32>,
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
) -> Option<String> {
    let viscosity = semantic_viscosity?;
    let friction = semantic_friction.unwrap_or(0.0).clamp(0.0, 1.0);
    let trickle = semantic_trickle.unwrap_or(0.0).clamp(0.0, 1.0);
    let density_context = structural_density
        .or(resonance_depth)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let state = if viscosity < 0.35 {
        "semantic_viscosity_low"
    } else if trickle <= 0.02 && friction >= 0.45 {
        "semantic_bottleneck_watch"
    } else if viscosity >= 0.60 && trickle >= 0.03 && density_context >= 0.55 {
        "heavy_semantic_flow"
    } else if viscosity >= 0.60 {
        "semantic_viscosity_high_watch"
    } else {
        "semantic_viscosity_mixed"
    };
    Some(state.to_string())
}

fn complexity_density_v1(
    structural_density: Option<f32>,
    resonance_depth: Option<f32>,
    mode_packing: Option<f32>,
    semantic_viscosity: Option<f32>,
    spectral_entropy: Option<f32>,
    pressure_risk: Option<f32>,
) -> Option<f32> {
    let entropy = spectral_entropy?;
    let density_context = structural_density
        .zip(resonance_depth)
        .map_or_else(
            || structural_density.or(resonance_depth).unwrap_or(0.0),
            |(density, depth)| (density.clamp(0.0, 1.0) + depth.clamp(0.0, 1.0)) * 0.5,
        )
        .clamp(0.0, 1.0);
    let mode = mode_packing.unwrap_or(0.0).clamp(0.0, 1.0);
    let viscosity = semantic_viscosity.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    Some(round_flux_delta(
        ((entropy.clamp(0.0, 1.0) * 0.40)
            + (density_context * 0.30)
            + (viscosity * 0.15)
            + (mode * 0.10)
            + (pressure * 0.05))
            .clamp(0.0, 1.0),
    ))
}

fn complexity_density_state_v1(
    complexity_density: Option<f32>,
    mode_packing: Option<f32>,
    pressure_risk: Option<f32>,
    spectral_entropy: Option<f32>,
) -> Option<String> {
    let complexity = complexity_density?;
    let mode = mode_packing.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let state = if complexity >= 0.60
        && mode < PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT
        && pressure <= 0.35
    {
        "interwoven_complexity_without_volume_pressure"
    } else if complexity >= 0.55 && entropy >= PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_AT {
        "high_entropy_complexity_density"
    } else if complexity >= 0.40 {
        "moderate_complexity_density"
    } else {
        "low_complexity_density"
    };
    Some(state.to_string())
}

fn enrich_resonance_component_context_v1(telemetry: &mut SpectralTelemetry) {
    if let Some(resonance) = telemetry.resonance_density_v1.as_mut() {
        if resonance.components.cohesion_score.is_none() {
            resonance.components.cohesion_score =
                Some(resonance_cohesion_score_v1(&resonance.components));
        }
        if resonance.components.structural_integrity_index.is_none() {
            resonance.components.structural_integrity_index = Some(
                resonance_structural_integrity_index_v1(&resonance.components),
            );
        }
    }
}

fn optional_flux_delta(latest: Option<f32>, previous: Option<f32>) -> Option<f32> {
    latest
        .zip(previous)
        .map(|(latest, previous)| round_flux_delta(latest - previous))
}

fn semantic_coherence_proxy_v1(sample: &PressureTrendSampleV1) -> Option<f32> {
    let trickle = sample.semantic_trickle?.clamp(0.0, 1.0);
    let density_context = sample
        .structural_density
        .or(sample.resonance_depth)
        .or(sample.semantic_viscosity)
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let friction_relief = 1.0 - sample.semantic_friction.unwrap_or(0.5).clamp(0.0, 1.0);
    Some(round_flux_delta(
        (trickle.mul_add(0.45, density_context.mul_add(0.35, friction_relief * 0.20)))
            .clamp(0.0, 1.0),
    ))
}

fn semantic_coherence_delta_v1(
    latest: &PressureTrendSampleV1,
    previous: &PressureTrendSampleV1,
) -> Option<f32> {
    semantic_coherence_proxy_v1(latest)
        .zip(semantic_coherence_proxy_v1(previous))
        .map(|(latest, previous)| round_flux_delta(latest - previous))
}

fn semantic_fidelity_score_v1(
    spectral_entropy: Option<f32>,
    semantic_trickle: Option<f32>,
    semantic_coherence_delta: Option<f32>,
    semantic_friction: Option<f32>,
    semantic_stagnation_index: Option<f32>,
) -> Option<f32> {
    if semantic_trickle.is_none()
        && semantic_coherence_delta.is_none()
        && semantic_friction.is_none()
        && semantic_stagnation_index.is_none()
    {
        return None;
    }
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let trickle_score = (semantic_trickle.unwrap_or(0.0).clamp(0.0, 1.0) / 0.24).clamp(0.0, 1.0);
    let coherence_score = semantic_coherence_delta
        .map(|delta| (0.50 + delta.clamp(-0.25, 0.25) * 2.0).clamp(0.0, 1.0))
        .unwrap_or(0.50);
    let friction_relief = 1.0 - semantic_friction.unwrap_or(0.50).clamp(0.0, 1.0);
    let stagnation_relief = 1.0 - semantic_stagnation_index.unwrap_or(0.50).clamp(0.0, 1.0);
    let entropy_penalty = if entropy >= 0.85 {
        ((entropy - 0.85) / 0.15).clamp(0.0, 1.0) * 0.10
    } else {
        0.0
    };
    Some(round_flux_delta(
        (trickle_score * 0.40
            + coherence_score * 0.30
            + friction_relief * 0.15
            + stagnation_relief * 0.15
            - entropy_penalty)
            .clamp(0.0, 1.0),
    ))
}

fn semantic_fidelity_state_v1(
    spectral_entropy: Option<f32>,
    semantic_fidelity_score: Option<f32>,
) -> &'static str {
    let Some(score) = semantic_fidelity_score else {
        return "semantic_fidelity_unavailable";
    };
    if spectral_entropy.is_some_and(|entropy| entropy >= 0.85) {
        if score >= 0.58 {
            "high_entropy_semantic_trickle_preserved"
        } else if score >= 0.35 {
            "high_entropy_semantic_fidelity_watch"
        } else {
            "high_entropy_semantic_fidelity_thin"
        }
    } else if score >= 0.58 {
        "semantic_fidelity_preserved"
    } else if score >= 0.35 {
        "semantic_fidelity_watch"
    } else {
        "semantic_fidelity_thin"
    }
}

fn dominant_spectral_drift_velocity(
    mode_packing_delta: Option<f32>,
    structural_density_delta: Option<f32>,
    resonance_depth_delta: Option<f32>,
) -> Option<f32> {
    [
        mode_packing_delta,
        structural_density_delta,
        resonance_depth_delta,
    ]
    .into_iter()
    .flatten()
    .max_by(|left, right| left.abs().total_cmp(&right.abs()))
    .map(round_flux_delta)
}

fn optional_flux_acceleration(
    latest: Option<f32>,
    previous: Option<f32>,
    before_previous: Option<f32>,
) -> Option<f32> {
    latest
        .zip(previous)
        .zip(before_previous)
        .map(|((latest, previous), before_previous)| {
            round_flux_delta((latest - previous) - (previous - before_previous))
        })
}

fn flux_confidence_for_pairs(pairs: &[(Option<f32>, Option<f32>)]) -> f32 {
    if pairs.is_empty() {
        return 0.0;
    }
    let available = pairs
        .iter()
        .filter(|(latest, previous)| latest.is_some() && previous.is_some())
        .count() as f32;
    round_flux_delta(available / pairs.len() as f32)
}

fn flux_absence_semantics(
    pressure_velocity: Option<f32>,
    mode_packing_velocity: Option<f32>,
    structural_density_delta: Option<f32>,
) -> Option<String> {
    if pressure_velocity.is_some()
        && mode_packing_velocity.is_some()
        && structural_density_delta.is_some()
    {
        return None;
    }
    Some("absent_flux_component_means_unknown_not_zero".to_string())
}

fn build_texture_dynamic_flux_vector_v1(
    samples: &VecDeque<PressureTrendSampleV1>,
) -> Option<TextureDynamicFluxVectorV1> {
    let latest = samples.back()?;
    let previous = samples.iter().rev().nth(1)?;
    let before_previous = samples.iter().rev().nth(2);
    Some(TextureDynamicFluxVectorV1 {
        policy: "texture_dynamic_flux_vector_v1".to_string(),
        schema_version: 1,
        pressure_velocity: optional_flux_delta(latest.pressure_risk, previous.pressure_risk),
        pressure_acceleration: optional_flux_acceleration(
            latest.pressure_risk,
            previous.pressure_risk,
            before_previous.and_then(|sample| sample.pressure_risk),
        ),
        mode_packing_velocity: optional_flux_delta(latest.mode_packing, previous.mode_packing),
        mode_packing_acceleration: optional_flux_acceleration(
            latest.mode_packing,
            previous.mode_packing,
            before_previous.and_then(|sample| sample.mode_packing),
        ),
        fill_velocity_pct: Some(round_flux_delta(latest.fill_pct - previous.fill_pct)),
        fill_acceleration_pct: before_previous.map(|sample| {
            round_flux_delta(
                (latest.fill_pct - previous.fill_pct) - (previous.fill_pct - sample.fill_pct),
            )
        }),
        structural_density_delta: optional_flux_delta(
            latest.structural_density,
            previous.structural_density,
        ),
        semantic_viscosity_velocity: optional_flux_delta(
            latest.semantic_viscosity,
            previous.semantic_viscosity,
        ),
        semantic_viscosity_acceleration: optional_flux_acceleration(
            latest.semantic_viscosity,
            previous.semantic_viscosity,
            before_previous.and_then(|sample| sample.semantic_viscosity),
        ),
        porosity_velocity: optional_flux_delta(
            latest.porosity_gradient,
            previous.porosity_gradient,
        ),
        comfort_gate_velocity: optional_flux_delta(latest.comfort_gate, previous.comfort_gate),
        comfort_gate_acceleration: optional_flux_acceleration(
            latest.comfort_gate,
            previous.comfort_gate,
            before_previous.and_then(|sample| sample.comfort_gate),
        ),
        spectral_entropy: latest.spectral_entropy,
        flux_confidence: Some(flux_confidence_for_pairs(&[
            (latest.pressure_risk, previous.pressure_risk),
            (latest.mode_packing, previous.mode_packing),
            (latest.structural_density, previous.structural_density),
            (latest.semantic_viscosity, previous.semantic_viscosity),
            (latest.porosity_gradient, previous.porosity_gradient),
        ])),
        flux_absence_semantics: flux_absence_semantics(
            optional_flux_delta(latest.pressure_risk, previous.pressure_risk),
            optional_flux_delta(latest.mode_packing, previous.mode_packing),
            optional_flux_delta(latest.structural_density, previous.structural_density),
        ),
        source: "bridge_pressure_trend_samples_v1".to_string(),
        authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
    })
}
