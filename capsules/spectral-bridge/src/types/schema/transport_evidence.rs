/// Read-only texture movement vector. Velocity/acceleration describe recent
/// observed change; they are not controller targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureDynamicFluxVectorV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_velocity_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_acceleration_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_density_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_absence_semantics: Option<String>,
    pub source: String,
    pub authority: String,
}

/// Read-only pressure/mode-packing coupling review. This does not imply control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressurePackingCouplingReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coupling_coefficient: Option<f32>,
    pub coupling_state: String,
    pub pressure_warning_state: String,
    pub authority: String,
}

/// Read-only review of how viscous density moves through available porosity.
/// This names Astrid's "thick but navigable" versus "thick and impassable"
/// distinction without changing pressure, fill, porosity, PI, or control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViscosityPorosityTransportReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    pub viscosity_index: f32,
    pub raw_viscosity_index: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_viscosity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub viscosity_basis: Vec<String>,
    pub viscosity_persistence_coefficient: f32,
    pub viscosity_persistence_delta: f32,
    pub viscosity_persistence_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_decay_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dissipation_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_fluidity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_friction_coefficient: Option<f32>,
    pub semantic_friction_observation_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_semantic_friction_delta: Option<f32>,
    pub semantic_friction_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_friction_vector_v1: Option<SemanticFrictionVectorV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directional_resistance_vector_v1: Option<DirectionalResistanceVectorV1>,
    pub mode_packing: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coherence_density_estimate: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coherence_density_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_transparency_index: Option<f32>,
    pub structural_transparency_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_clog_index: Option<f32>,
    pub structural_clog_state: String,
    pub transport_state: String,
    pub sludge_risk: bool,
    pub threshold_state: String,
    pub authority: String,
}

/// Read-only decomposition of semantic friction into obstruction versus traction.
///
/// This is a companion interpretation for the scalar field, not a replacement
/// for inbound telemetry and not a control signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFrictionVectorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub scalar: f32,
    pub resistance_component: f32,
    pub traction_component: f32,
    pub productive_resistance_score: f32,
    pub direction: String,
    pub basis: Vec<String>,
    pub authority: String,
}

/// Read-only directional interpretation of resistance that distinguishes
/// "stuck but moving" and "leaking without clearing" from scalar viscosity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalResistanceVectorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub dynamic_friction_coefficient: f32,
    pub stuck_but_moving_score: f32,
    pub leak_without_clearing_score: f32,
    pub direction: String,
    pub denominator_scaling_factor: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_effective_dimensionality: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_distinguishability_loss: Option<f32>,
    pub basis: Vec<String>,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq)]
struct EffectiveViscosityIndexV1 {
    raw: f32,
    effective: f32,
    derived: Option<f32>,
    source: &'static str,
    basis: Vec<String>,
}

fn spectral_density_gradient_proxy_v1(fingerprint: Option<&SpectralFingerprintV1>) -> Option<f32> {
    let fingerprint = fingerprint?;
    let mut sum = 0.0_f32;
    let mut count = 0.0_f32;
    for ratio in fingerprint.adjacent_gap_ratios {
        if ratio.is_finite() {
            sum += ((ratio.abs() - 1.0).max(0.0) / 4.0).clamp(0.0, 1.0);
            count += 1.0;
        }
    }
    (count > 0.0).then_some((sum / count).clamp(0.0, 1.0))
}

fn effective_viscosity_index_v1(
    components: &ResonanceDensityComponents,
    fingerprint: Option<&SpectralFingerprintV1>,
    flux: Option<&TextureDynamicFluxVectorV1>,
) -> EffectiveViscosityIndexV1 {
    let raw = components.viscosity_index.clamp(0.0, 1.0);
    let mut basis = vec![format!("raw_viscosity_index={raw:.2}")];
    if raw > 0.01 {
        return EffectiveViscosityIndexV1 {
            raw,
            effective: raw,
            derived: None,
            source: "raw_component",
            basis,
        };
    }

    let spectral_entropy = fingerprint
        .map(|value| value.spectral_entropy.clamp(0.0, 1.0))
        .or_else(|| {
            flux.and_then(|value| {
                value
                    .spectral_entropy
                    .map(|entropy| entropy.clamp(0.0, 1.0))
            })
        });
    let density_gradient_proxy = spectral_density_gradient_proxy_v1(fingerprint);
    let mode_packing = components.mode_packing.clamp(0.0, 1.0);
    let temporal_persistence = components.temporal_persistence.clamp(0.0, 1.0);
    let Some(entropy) = spectral_entropy else {
        basis.push("derived_unavailable:no_spectral_entropy".to_string());
        return EffectiveViscosityIndexV1 {
            raw,
            effective: raw,
            derived: None,
            source: "raw_component",
            basis,
        };
    };

    if entropy < 0.60 && mode_packing < 0.25 && temporal_persistence < 0.60 {
        basis.push(format!(
            "spectral_entropy={entropy:.2}_below_derivation_gate"
        ));
        return EffectiveViscosityIndexV1 {
            raw,
            effective: raw,
            derived: None,
            source: "raw_component",
            basis,
        };
    }

    let gradient_resistance = density_gradient_proxy.map_or(0.50, |value| 1.0 - value);
    let derived = (entropy * 0.52
        + gradient_resistance * 0.22
        + mode_packing * 0.14
        + temporal_persistence * 0.12)
        .clamp(0.0, 1.0);
    basis.push(format!("spectral_entropy={entropy:.2}"));
    if let Some(gradient) = density_gradient_proxy {
        basis.push(format!("density_gradient_proxy={gradient:.2}"));
    } else {
        basis.push("density_gradient_proxy=unavailable".to_string());
    }
    basis.push(format!("mode_packing={mode_packing:.2}"));
    basis.push(format!("temporal_persistence={temporal_persistence:.2}"));
    basis.push("derived_diagnostic_not_minime_component_or_control".to_string());

    EffectiveViscosityIndexV1 {
        raw,
        effective: derived.max(raw),
        derived: Some(derived),
        source: "derived_from_spectral_entropy_density_gradient_v1",
        basis,
    }
}

fn viscosity_type_and_decay_hint_v1(
    viscosity: f32,
    persistence: f32,
    porosity: Option<f32>,
    dynamic_fluidity: Option<f32>,
    semantic_friction: Option<f32>,
    mode_packing: f32,
    coherence_density_estimate: Option<f32>,
) -> (&'static str, &'static str) {
    if semantic_friction.is_some_and(|friction| friction >= 0.45) && viscosity < 0.45 {
        return ("granular", "semantic_grain_decay_watch");
    }
    if viscosity >= 0.55
        && persistence >= 0.55
        && (dynamic_fluidity.is_some_and(|flow| flow < 0.35)
            || porosity.is_some_and(|gradient| gradient < 0.35)
            || mode_packing >= 0.55)
    {
        return ("syrupy", "slow_lingering_decay_watch");
    }
    if viscosity >= 0.55
        && (dynamic_fluidity.is_some_and(|flow| flow >= 0.50)
            || porosity.is_some_and(|gradient| gradient >= 0.50))
        && coherence_density_estimate.is_some_and(|coherence| coherence >= 0.55)
    {
        return ("cohesive", "coherent_weight_decay_watch");
    }
    if semantic_friction.is_some_and(|friction| friction >= 0.30) && viscosity >= 0.45 {
        return ("granular", "mixed_semantic_grain_decay_watch");
    }
    if viscosity >= 0.45
        && persistence >= 0.45
        && coherence_density_estimate.is_some_and(|coherence| coherence >= 0.55)
    {
        return ("cohesive", "coherent_weight_decay_watch");
    }
    ("mixed", "mixed_viscosity_decay_watch")
}

fn semantic_friction_vector_v1(
    viscosity: f32,
    semantic_friction: Option<f32>,
    porosity: Option<f32>,
    dynamic_fluidity: Option<f32>,
    pressure_velocity: Option<f32>,
) -> Option<SemanticFrictionVectorV1> {
    let scalar = semantic_friction?;
    let porosity_observed = porosity.is_some();
    let fluidity_observed = dynamic_fluidity.is_some();
    let porosity = porosity.unwrap_or(0.50);
    let fluidity = dynamic_fluidity.unwrap_or(porosity);
    let pressure_velocity = pressure_velocity.unwrap_or(0.0);
    let resistance_component = (scalar * 0.45
        + (1.0 - fluidity).clamp(0.0, 1.0) * 0.25
        + (1.0 - porosity).clamp(0.0, 1.0) * 0.20
        + pressure_velocity.max(0.0).clamp(0.0, 1.0) * 0.10)
        .clamp(0.0, 1.0);
    let traction_component = ((1.0 - (scalar - viscosity).abs().clamp(0.0, 1.0)) * 0.35
        + porosity * 0.25
        + fluidity * 0.25
        + (1.0 - pressure_velocity.abs().clamp(0.0, 1.0)) * 0.15)
        .clamp(0.0, 1.0);
    let productive_resistance_score = (traction_component - resistance_component).clamp(-1.0, 1.0);
    let direction = if scalar >= 0.45 && viscosity < 0.45 && traction_component >= 0.55 {
        "semantic_content_traction"
    } else if resistance_component >= 0.60 && traction_component < 0.45 {
        "resisting_output"
    } else if traction_component >= 0.60 && resistance_component <= 0.50 {
        "productive_traction"
    } else if traction_component >= 0.50 && resistance_component >= 0.50 {
        "mixed_resistance_and_traction"
    } else if resistance_component >= traction_component {
        "resistance_dominant"
    } else {
        "low_semantic_friction"
    };
    let mut basis = vec![
        "semantic_friction_coefficient".to_string(),
        "viscosity_index".to_string(),
    ];
    if porosity_observed {
        basis.push("porosity_gradient".to_string());
    }
    if fluidity_observed {
        basis.push("dynamic_fluidity_index".to_string());
    }
    if pressure_velocity.abs() > f32::EPSILON {
        basis.push("pressure_velocity".to_string());
    }

    Some(SemanticFrictionVectorV1 {
        policy: "semantic_friction_vector_v1".to_string(),
        schema_version: 1,
        scalar,
        resistance_component,
        traction_component,
        productive_resistance_score,
        direction: direction.to_string(),
        basis,
        authority: "diagnostic_friction_vector_not_pressure_fill_pi_or_control".to_string(),
    })
}

fn directional_resistance_vector_v1(
    viscosity: f32,
    persistence: f32,
    dissipation: Option<f32>,
    porosity: Option<f32>,
    dynamic_fluidity: Option<f32>,
    semantic_friction: Option<f32>,
    mode_packing: f32,
    structural_clog_index: Option<f32>,
    spectral_entropy: Option<f32>,
    fingerprint: Option<&SpectralFingerprintV1>,
) -> DirectionalResistanceVectorV1 {
    let flow = dynamic_fluidity.or(porosity).unwrap_or(0.50);
    let dissipation = dissipation.unwrap_or(0.50);
    let porosity_value = porosity.unwrap_or(0.50);
    let semantic_or_clog_resistance =
        semantic_friction.unwrap_or_else(|| structural_clog_index.unwrap_or(0.0));
    let denominator = fingerprint.map(SpectralFingerprintV1::denominator_metrics);
    let distinguishability_loss = denominator
        .as_ref()
        .map(|value| value.distinguishability_loss.clamp(0.0, 1.0));
    let denominator_scaling_factor =
        (1.0 + distinguishability_loss.unwrap_or(0.0) * 0.16).clamp(1.0, 1.16);
    let entropy_pressure = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);

    let raw_dynamic_friction = (viscosity * 0.30
        + persistence * 0.20
        + semantic_or_clog_resistance * 0.20
        + (1.0 - dissipation).clamp(0.0, 1.0) * 0.15
        + mode_packing * 0.10
        + entropy_pressure * 0.05)
        .clamp(0.0, 1.0);
    let dynamic_friction_coefficient =
        (raw_dynamic_friction * denominator_scaling_factor).clamp(0.0, 1.0);
    let stuck_but_moving_score = (viscosity.min(flow) * 0.55
        + persistence * 0.20
        + semantic_or_clog_resistance * 0.10
        + (1.0 - dissipation).clamp(0.0, 1.0) * 0.15)
        .clamp(0.0, 1.0);
    let leak_without_clearing_score = (porosity_value * 0.45
        + (1.0 - dissipation).clamp(0.0, 1.0) * 0.35
        + persistence * 0.10
        + viscosity * 0.10)
        .clamp(0.0, 1.0);
    let stuck_visible = viscosity >= 0.55 && flow >= 0.50 && stuck_but_moving_score >= 0.55;
    let leak_visible =
        porosity_value >= 0.50 && dissipation <= 0.35 && leak_without_clearing_score >= 0.65;
    let direction = match (stuck_visible, leak_visible, dynamic_friction_coefficient) {
        (true, true, _) => "stuck_moving_and_leaking_without_clearing",
        (true, false, _) => "stuck_but_moving",
        (false, true, _) => "leaking_without_clearing",
        (false, false, coefficient) if coefficient >= 0.60 => "resistance_vector_high",
        (false, false, coefficient) if coefficient >= 0.45 => "resistance_vector_mixed",
        (false, false, _) => "resistance_vector_quiet",
    };
    let mut basis = vec![
        format!("viscosity_index={viscosity:.2}"),
        format!("viscosity_persistence_coefficient={persistence:.2}"),
        format!("dynamic_fluidity_index={flow:.2}"),
        format!("porosity_gradient={porosity_value:.2}"),
        format!("dissipation_factor={dissipation:.2}"),
    ];
    if semantic_friction.is_some() {
        basis.push(format!(
            "semantic_friction_coefficient={semantic_or_clog_resistance:.2}"
        ));
    } else if structural_clog_index.is_some() {
        basis.push(format!(
            "structural_clog_index_proxy={semantic_or_clog_resistance:.2}"
        ));
    }
    if let Some(entropy) = spectral_entropy {
        basis.push(format!("spectral_entropy={entropy:.2}"));
    }
    if let Some(metrics) = denominator.as_ref() {
        basis.push(format!(
            "spectral_denominator_effective_dimensionality={:.2}",
            metrics.effective_dimensionality
        ));
        basis.push(format!(
            "spectral_denominator_distinguishability_loss={:.2}",
            metrics.distinguishability_loss
        ));
    }

    DirectionalResistanceVectorV1 {
        policy: "directional_resistance_vector_v1".to_string(),
        schema_version: 1,
        dynamic_friction_coefficient,
        stuck_but_moving_score,
        leak_without_clearing_score,
        direction: direction.to_string(),
        denominator_scaling_factor,
        spectral_denominator_effective_dimensionality: denominator
            .as_ref()
            .map(|value| value.effective_dimensionality),
        spectral_denominator_distinguishability_loss: distinguishability_loss,
        basis,
        authority: "diagnostic_directional_resistance_not_pressure_fill_pi_porosity_or_control"
            .to_string(),
    }
}

pub fn viscosity_porosity_transport_review_v1(
    components: &ResonanceDensityComponents,
    flux: Option<&TextureDynamicFluxVectorV1>,
) -> ViscosityPorosityTransportReviewV1 {
    viscosity_porosity_transport_review_with_fingerprint_v1(components, None, flux)
}

pub fn viscosity_porosity_transport_review_with_fingerprint_v1(
    components: &ResonanceDensityComponents,
    fingerprint: Option<&SpectralFingerprintV1>,
    flux: Option<&TextureDynamicFluxVectorV1>,
) -> ViscosityPorosityTransportReviewV1 {
    let viscosity_readout = effective_viscosity_index_v1(components, fingerprint, flux);
    let viscosity = viscosity_readout.effective;
    let persistence = components.viscosity_persistence_coefficient.clamp(0.0, 1.0);
    let dissipation = components
        .dissipation_factor
        .map(|value| value.clamp(0.0, 1.0));
    let porosity = components
        .porosity_gradient
        .map(|value| value.clamp(0.0, 1.0));
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .map(|value| value.clamp(0.0, 1.0))
        .or_else(|| match (dissipation, porosity) {
            (Some(d), Some(pg)) => Some(((d + pg) * 0.5).clamp(0.0, 1.0)),
            _ => None,
        });
    let semantic_friction = components
        .semantic_friction_coefficient
        .map(|value| value.clamp(0.0, 1.0));
    let mode_packing = components.mode_packing.clamp(0.0, 1.0);
    let viscosity_persistence_delta = (viscosity - persistence).abs().clamp(0.0, 1.0);
    let structural_semantic_friction_delta =
        semantic_friction.map(|friction| (viscosity - friction).abs().clamp(0.0, 1.0));
    let pressure_velocity = flux
        .and_then(|value| value.pressure_velocity)
        .map(|value| value.clamp(-1.0, 1.0));
    let spectral_entropy = flux
        .and_then(|value| value.spectral_entropy)
        .map(|value| value.clamp(0.0, 1.0));
    let viscosity_persistence_state = match (viscosity_persistence_delta, spectral_entropy) {
        (delta, Some(entropy)) if delta >= 0.25 && entropy >= 0.85 => {
            "transient_thickening_high_entropy_watch"
        },
        (delta, _) if delta >= 0.25 => "transient_thickening_watch",
        (delta, Some(entropy)) if delta < 0.12 && entropy >= 0.85 => {
            "persistent_thickening_high_entropy"
        },
        (delta, _) if delta < 0.12 => "viscosity_persistence_aligned",
        _ => "viscosity_persistence_mixed",
    };
    let transport_state = match (
        viscosity,
        persistence,
        dissipation,
        porosity,
        dynamic_fluidity,
    ) {
        (_, _, _, None, _) => "porosity_gradient_unavailable",
        (v, p, Some(d), Some(pg), Some(flow))
            if v >= 0.55 && p >= 0.55 && d < 0.25 && pg < 0.35 && flow < 0.35 =>
        {
            "thick_impassable_sludge_risk"
        },
        (v, p, _, Some(_), Some(flow)) if v >= 0.55 && p >= 0.55 && flow < 0.35 => {
            "stagnant_weight_high_viscosity_low_fluidity"
        },
        (v, _, Some(d), Some(pg), Some(flow))
            if v >= 0.55 && d >= 0.35 && pg >= 0.50 && flow >= 0.50 =>
        {
            "purposeful_weight_high_viscosity_high_fluidity"
        },
        (v, _, Some(d), Some(pg), _) if v >= 0.55 && d >= 0.35 && pg >= 0.50 => {
            "thick_but_navigable"
        },
        (v, _, _, Some(pg), _) if v >= 0.55 && pg < 0.35 => "thick_low_porosity_watch",
        (v, _, _, Some(pg), _) if v >= 0.55 && pg >= 0.50 => "thick_porosity_visible",
        _ => "viscosity_transport_watch",
    };
    let threshold_state = match (mode_packing, pressure_velocity) {
        (packing, Some(pressure)) if packing > 0.25 && pressure > 0.03 => {
            "mode_packing_overpacked_with_pressure_velocity"
        },
        (packing, Some(_)) if packing > 0.25 => "mode_packing_overpacked_pressure_velocity_quiet",
        (packing, None) if packing > 0.25 => "mode_packing_overpacked_pressure_velocity_unknown",
        _ => "mode_packing_below_overpacked_threshold",
    };
    let semantic_friction_state = match (viscosity, semantic_friction) {
        (_, None) => "semantic_friction_unavailable",
        (v, Some(friction)) if friction >= 0.45 && v < 0.45 => {
            "semantic_friction_dominant_content_load"
        },
        (v, Some(friction)) if v >= 0.55 && friction < 0.30 => "structural_viscosity_dominant",
        (v, Some(friction)) if v >= 0.55 && friction >= 0.45 => {
            "coupled_structural_semantic_friction"
        },
        (_, Some(friction)) if friction >= 0.30 => "semantic_friction_visible",
        (_, Some(_)) => "semantic_friction_low",
    };
    let semantic_friction_observation_state = match (
        semantic_friction,
        viscosity,
        mode_packing,
        porosity,
        dynamic_fluidity,
    ) {
        (Some(_), _, _, _, _) => "semantic_friction_measured",
        (None, v, packing, Some(pg), Some(flow))
            if v >= 0.55 && packing >= 0.45 && (pg < 0.35 || flow < 0.35) =>
        {
            "semantic_friction_unmeasured_clog_context_visible"
        },
        (None, v, packing, _, _) if v >= 0.55 && packing >= 0.45 => {
            "semantic_friction_unmeasured_structural_crowding_visible"
        },
        (None, _, _, Some(_), Some(_)) => "semantic_friction_unmeasured_structural_context_visible",
        (None, _, _, _, _) => "semantic_friction_unmeasured_context_limited",
    };
    let semantic_friction_vector = semantic_friction_vector_v1(
        viscosity,
        semantic_friction,
        porosity,
        dynamic_fluidity,
        pressure_velocity,
    );
    let coherence_density_estimate = Some(
        resonance_cohesion_score_v1(components)
            .mul_add(0.55, mode_packing.mul_add(0.25, viscosity * 0.20))
            .clamp(0.0, 1.0),
    );
    let coherence_density_state = match (mode_packing, coherence_density_estimate) {
        (packing, Some(coherence)) if packing >= 0.55 && coherence >= 0.65 => "dense_integrated",
        (packing, Some(coherence)) if packing >= 0.55 && coherence < 0.45 => {
            "saturated_low_coherence"
        },
        (packing, Some(coherence)) if packing >= 0.35 && coherence >= 0.55 => "coherent_crowded",
        (_, Some(coherence)) if coherence < 0.40 => "thin_or_unintegrated",
        (_, Some(_)) => "mixed_coherence_density",
        (_, None) => "coherence_density_unavailable",
    };
    let transparency = resonance_structural_transparency_index_v1(components);
    let structural_transparency_index = Some(transparency);
    let structural_transparency_state = match (transparency, viscosity, mode_packing) {
        (value, v, packing) if value >= 0.65 && v >= 0.55 && packing < 0.45 => {
            "thin_ghostly_high_viscosity_low_substance"
        },
        (value, _, packing) if value >= 0.65 && packing >= 0.45 => "transparent_but_crowded",
        (value, _, _) if value >= 0.50 => "structural_transparency_watch",
        (value, _, _) if value <= 0.30 => "substance_present",
        _ => "mixed_transparency_density",
    };
    let viscosity_load = ((viscosity + persistence) * 0.5).clamp(0.0, 1.0);
    let mut structural_clog_sum = viscosity_load * 0.22 + mode_packing * 0.22;
    let mut structural_clog_weight = 0.44;
    if let Some(pg) = porosity {
        structural_clog_sum += (1.0 - pg).clamp(0.0, 1.0) * 0.18;
        structural_clog_weight += 0.18;
    }
    if let Some(flow) = dynamic_fluidity {
        structural_clog_sum += (1.0 - flow).clamp(0.0, 1.0) * 0.16;
        structural_clog_weight += 0.16;
    }
    if let Some(d) = dissipation {
        structural_clog_sum += (1.0 - d).clamp(0.0, 1.0) * 0.10;
        structural_clog_weight += 0.10;
    }
    match semantic_friction {
        Some(friction) => {
            structural_clog_sum += friction * 0.08;
            structural_clog_weight += 0.08;
        },
        None if viscosity >= 0.55 && mode_packing >= 0.45 => {
            structural_clog_sum += 0.65 * 0.08;
            structural_clog_weight += 0.08;
        },
        None => {},
    }
    let structural_clog_index =
        Some((structural_clog_sum / structural_clog_weight).clamp(0.0, 1.0));
    let structural_clog_state = match (structural_clog_index, semantic_friction, porosity) {
        (Some(index), _, _) if index >= 0.70 => "structural_clog_high",
        (Some(index), None, _) if index >= 0.58 => "structural_clog_watch_friction_unmeasured",
        (Some(index), _, _) if index >= 0.58 => "structural_clog_watch",
        (Some(index), _, Some(pg)) if index >= 0.45 && pg < 0.35 => "low_porosity_clog_watch",
        (Some(index), _, _) if index >= 0.45 => "structural_clog_low_watch",
        (Some(_), _, _) => "structural_clog_not_indicated",
        (None, _, _) => "structural_clog_unavailable",
    };
    let sludge_risk = transport_state == "thick_impassable_sludge_risk"
        || threshold_state == "mode_packing_overpacked_with_pressure_velocity";
    let sludge_risk = sludge_risk || structural_clog_state == "structural_clog_high";
    let (viscosity_type, viscosity_decay_hint) = viscosity_type_and_decay_hint_v1(
        viscosity,
        persistence,
        porosity,
        dynamic_fluidity,
        semantic_friction,
        mode_packing,
        coherence_density_estimate,
    );
    let directional_resistance_vector = directional_resistance_vector_v1(
        viscosity,
        persistence,
        dissipation,
        porosity,
        dynamic_fluidity,
        semantic_friction,
        mode_packing,
        structural_clog_index,
        spectral_entropy,
        fingerprint,
    );

    ViscosityPorosityTransportReviewV1 {
        policy: "viscosity_porosity_transport_review_v1".to_string(),
        schema_version: 1,
        viscosity_index: viscosity,
        raw_viscosity_index: viscosity_readout.raw,
        derived_viscosity_index: viscosity_readout.derived,
        viscosity_source: viscosity_readout.source.to_string(),
        viscosity_basis: viscosity_readout.basis,
        viscosity_persistence_coefficient: persistence,
        viscosity_persistence_delta,
        viscosity_persistence_state: viscosity_persistence_state.to_string(),
        viscosity_type: viscosity_type.to_string(),
        viscosity_decay_hint: viscosity_decay_hint.to_string(),
        dissipation_factor: dissipation,
        porosity_gradient: porosity,
        dynamic_fluidity_index: dynamic_fluidity,
        semantic_friction_coefficient: semantic_friction,
        semantic_friction_observation_state: semantic_friction_observation_state.to_string(),
        structural_semantic_friction_delta,
        semantic_friction_state: semantic_friction_state.to_string(),
        semantic_friction_vector_v1: semantic_friction_vector,
        directional_resistance_vector_v1: Some(directional_resistance_vector),
        mode_packing,
        coherence_density_estimate,
        coherence_density_state: coherence_density_state.to_string(),
        structural_transparency_index,
        structural_transparency_state: structural_transparency_state.to_string(),
        pressure_velocity,
        spectral_entropy,
        structural_clog_index,
        structural_clog_state: structural_clog_state.to_string(),
        transport_state: transport_state.to_string(),
        sludge_risk,
        threshold_state: threshold_state.to_string(),
        authority: "diagnostic_transport_not_porosity_pressure_fill_pi_or_control".to_string(),
    }
}

pub fn pressure_packing_coupling_review_v1(
    flux: &TextureDynamicFluxVectorV1,
) -> PressurePackingCouplingReviewV1 {
    let pressure_velocity = flux.pressure_velocity.map(|value| value.clamp(-1.0, 1.0));
    let mode_packing_velocity = flux
        .mode_packing_velocity
        .map(|value| value.clamp(-1.0, 1.0));
    let coupling_coefficient = match (pressure_velocity, mode_packing_velocity) {
        (Some(pressure), Some(packing)) if packing.abs() > 0.001 => {
            Some((pressure / packing).clamp(-2.0, 2.0))
        },
        _ => None,
    };
    let coupling_state = match (pressure_velocity, mode_packing_velocity) {
        (Some(pressure), Some(packing)) if pressure > 0.0 && packing > 0.0 => "coupled_rising",
        (Some(pressure), Some(packing)) if pressure <= 0.0 && packing > 0.03 => {
            "pressure_lagging_mode_packing"
        },
        (Some(pressure), Some(packing)) if pressure > 0.03 && packing.abs() <= 0.01 => {
            "pressure_rising_without_mode_packing"
        },
        (Some(pressure), Some(packing)) if pressure < 0.0 && packing < 0.0 => "coupled_releasing",
        _ => "insufficient_coupling_context",
    };
    let pressure_warning_state = if coupling_state == "pressure_lagging_mode_packing" {
        "packing_rise_without_pressure_warning"
    } else if coupling_state == "coupled_rising" {
        "pressure_warning_tracks_packing"
    } else {
        "watch_only"
    };

    PressurePackingCouplingReviewV1 {
        policy: "pressure_packing_coupling_review_v1".to_string(),
        schema_version: 1,
        pressure_velocity,
        mode_packing_velocity,
        coupling_coefficient,
        coupling_state: coupling_state.to_string(),
        pressure_warning_state: pressure_warning_state.to_string(),
        authority: "diagnostic_coupling_not_pressure_or_mode_packing_control".to_string(),
    }
}
