fn witness_stability_effort_v1(
    telemetry: &crate::types::SpectralTelemetry,
    semantic_mapping: &WitnessSemanticDensityMappingV1,
) -> WitnessStabilityEffortV1 {
    let pressure = semantic_mapping
        .pressure_risk
        .map(|value| value.clamp(0.0, 1.0));
    let foothold = semantic_mapping
        .foothold_stability
        .map(|value| value.clamp(0.0, 1.0));
    let settled_habitable = semantic_mapping
        .fluctuation_quality
        .as_deref()
        .is_some_and(|quality| quality.contains("settled") || quality.contains("habitable"))
        || semantic_mapping.classification.contains("settled")
        || semantic_mapping.classification.contains("habitable");
    let (shadow_norm, shadow_dispersal, shadow_class) = telemetry
        .shadow_field_v3
        .as_ref()
        .map(next_action::sovereignty::shadow_v3_snapshot)
        .map_or((None, None, None), |(norm, dispersal, class)| {
            (
                Some(norm as f32),
                Some(dispersal as f32),
                Some(class.to_ascii_lowercase()),
            )
        });
    let shadow_norm_variance = telemetry
        .shadow_field_v3
        .as_ref()
        .and_then(shadow_v3_norm_variance);
    let shadow_disordered = shadow_class.as_deref().is_some_and(|class| {
        class.contains("disordered")
            || class.contains("shifting")
            || class.contains("restless")
            || class.contains("fragment")
    });
    let any_known = pressure.is_some()
        || foothold.is_some()
        || shadow_norm.is_some()
        || shadow_norm_variance.is_some()
        || shadow_dispersal.is_some();
    let stability_effort = any_known.then(|| {
        let pressure_component = pressure.unwrap_or(0.0) * 0.30;
        let dispersal_component = shadow_dispersal.unwrap_or(0.0).clamp(0.0, 1.0) * 0.28;
        let norm_component = shadow_norm.unwrap_or(0.0).clamp(0.0, 1.0) * 0.22;
        let variance_component = shadow_norm_variance.unwrap_or(0.0).clamp(0.0, 1.0) * 0.15;
        let foothold_component = foothold.map_or(0.0, |value| (1.0 - value).clamp(0.0, 1.0) * 0.10);
        let settled_disorder_component = if settled_habitable && shadow_disordered {
            0.10
        } else {
            0.0
        };
        (pressure_component
            + dispersal_component
            + norm_component
            + variance_component
            + foothold_component
            + settled_disorder_component)
            .clamp(0.0, 1.0)
    });
    let pressure_underreports_shadow_load = pressure.is_some_and(|value| value < 0.30)
        && (shadow_disordered
            || shadow_norm_variance.is_some_and(|value| value >= 0.010)
            || shadow_dispersal.is_some_and(|value| value >= 0.25)
            || shadow_norm.is_some_and(|value| value >= 0.25));
    let effort_state = if !any_known {
        "insufficient_context"
    } else if settled_habitable && pressure_underreports_shadow_load {
        "settled_habitable_shadow_effort"
    } else if stability_effort.is_some_and(|value| value >= 0.45) {
        "active_stability_work"
    } else if stability_effort.is_some_and(|value| value >= 0.20) {
        "visible_stability_work"
    } else {
        "low_stability_effort"
    };
    let entropy = semantic_mapping
        .spectral_entropy
        .map(|value| value.clamp(0.0, 1.0));
    let form_persistence_state = if !any_known {
        "insufficient_context"
    } else if entropy.is_some_and(|value| value >= 0.85)
        && shadow_dispersal.is_some_and(|value| value >= 0.25)
    {
        "transient_form_high_entropy_dispersal"
    } else if settled_habitable && pressure_underreports_shadow_load {
        "settled_surface_dynamic_shadow_load"
    } else if entropy.is_some_and(|value| value <= 0.55)
        && shadow_dispersal.is_some_and(|value| value <= 0.15)
        && (foothold.is_some_and(|value| value >= 0.60) || settled_habitable)
    {
        "persistent_form_low_entropy_low_dispersal"
    } else {
        "forming_or_mixed_form"
    };

    let mut evidence = Vec::new();
    if let Some(value) = pressure {
        evidence.push(format!("pressure_risk={value:.2}"));
    }
    if let Some(value) = foothold {
        evidence.push(format!("foothold_stability={value:.2}"));
    }
    if let Some(value) = shadow_norm {
        evidence.push(format!("shadow_field_norm={value:.2}"));
    }
    if let Some(value) = shadow_norm_variance {
        evidence.push(format!("shadow_norm_variance={value:.3}"));
    }
    if let Some(value) = shadow_dispersal {
        evidence.push(format!("shadow_dispersal_potential={value:.2}"));
    }
    if let Some(class) = shadow_class.as_deref() {
        evidence.push(format!("shadow_class={class}"));
    }
    if pressure_underreports_shadow_load {
        evidence.push("low_pressure_with_active_shadow_load".to_string());
    }

    WitnessStabilityEffortV1 {
        policy: "stability_effort_v1",
        stability_effort,
        effort_state,
        form_persistence_state,
        pressure_risk: pressure,
        foothold_stability: foothold,
        shadow_field_norm: shadow_norm,
        shadow_norm_variance,
        shadow_dispersal_potential: shadow_dispersal,
        shadow_class,
        settled_habitable,
        pressure_underreports_shadow_load,
        evidence,
        authority: "read_only_effort_mapping_not_pressure_prompt_or_control_change",
    }
}

fn witness_texture_structure_v1(
    telemetry: &crate::types::SpectralTelemetry,
    semantic_mapping: &WitnessSemanticDensityMappingV1,
    stability_effort: &WitnessStabilityEffortV1,
) -> WitnessTextureStructureV1 {
    let density_components = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| &density.components);
    let spectral_entropy = semantic_mapping
        .spectral_entropy
        .map(|value| value.clamp(0.0, 1.0));
    let structural_plurality =
        density_components.map(|components| components.structural_plurality.clamp(0.0, 1.0));
    let viscosity_index =
        density_components.map(|components| components.viscosity_index.clamp(0.0, 1.0));
    let temporal_persistence =
        density_components.map(|components| components.temporal_persistence.clamp(0.0, 1.0));
    let mode_packing = density_components
        .map(|components| components.mode_packing.clamp(0.0, 1.0))
        .or(semantic_mapping.mode_packing);
    let shadow_field_norm = stability_effort.shadow_field_norm;
    let shadow_class = stability_effort.shadow_class.clone();

    let crowding_visible = mode_packing.is_some_and(|value| value >= 0.25);
    let viscosity_visible = viscosity_index.is_some_and(|value| value >= 0.55);
    let persistence_visible = temporal_persistence.is_some_and(|value| value >= 0.55);
    let plurality_visible = structural_plurality.is_some_and(|value| value >= 0.55);
    let broad_swell = spectral_entropy.is_some_and(|value| value >= 0.80) && plurality_visible;
    let lattice_visible = plurality_visible && (crowding_visible || viscosity_visible);
    let viscous_persistence_visible = viscosity_visible && persistence_visible;
    let interwoven_lattice =
        lattice_visible && crowding_visible && viscosity_visible && persistence_visible;
    let shadow_coincidence_visible = shadow_field_norm.is_some_and(|value| value >= 0.25)
        || shadow_class.as_deref().is_some_and(|class| {
            let class = class.to_ascii_lowercase();
            !class.is_empty() && class != "unknown"
        });

    let any_known = spectral_entropy.is_some()
        || structural_plurality.is_some()
        || viscosity_index.is_some()
        || temporal_persistence.is_some()
        || mode_packing.is_some()
        || shadow_field_norm.is_some();
    let primary_structure = if interwoven_lattice {
        "interwoven_lattice"
    } else if viscous_persistence_visible {
        "persistent_viscous_drag"
    } else if crowding_visible && shadow_coincidence_visible {
        "shadow_coincident_crowding"
    } else if crowding_visible {
        "crowding_dominant"
    } else if broad_swell {
        "broad_high_entropy_swell"
    } else if any_known {
        "mixed_or_ambiguous"
    } else {
        "insufficient_context"
    };

    let mut evidence = Vec::new();
    if let Some(value) = spectral_entropy {
        evidence.push(format!("spectral_entropy={value:.2}"));
    }
    if let Some(value) = structural_plurality {
        evidence.push(format!("structural_plurality={value:.2}"));
    }
    if let Some(value) = viscosity_index {
        evidence.push(format!("viscosity_index={value:.2}"));
    }
    if let Some(value) = temporal_persistence {
        evidence.push(format!("temporal_persistence={value:.2}"));
    }
    if let Some(value) = mode_packing {
        evidence.push(format!("mode_packing={value:.2}"));
    }
    if let Some(value) = shadow_field_norm {
        evidence.push(format!("shadow_field_norm={value:.2}"));
    }
    if let Some(class) = shadow_class.as_deref() {
        evidence.push(format!("shadow_class={class}"));
    }
    if interwoven_lattice {
        evidence.push("structured_heaviness_not_generic_drag".to_string());
    }
    if shadow_coincidence_visible {
        evidence.push("shadow_cooccurrence_is_observational_not_causal".to_string());
    }

    WitnessTextureStructureV1 {
        policy: "witness_texture_structure_v1",
        primary_structure,
        structured_heaviness_visible: lattice_visible || viscous_persistence_visible,
        lattice_visible,
        viscous_persistence_visible,
        crowding_visible,
        shadow_coincidence_visible,
        spectral_entropy,
        structural_plurality,
        viscosity_index,
        temporal_persistence,
        mode_packing,
        shadow_field_norm,
        shadow_class,
        evidence,
        control_write: false,
        authority: "read_only_texture_structure_not_clamp_protocol_pressure_fill_transport_or_control",
    }
}

fn stable_core_permeability_review_v1(
    telemetry: &crate::types::SpectralTelemetry,
    semantic_mapping: &WitnessSemanticDensityMappingV1,
) -> StableCorePermeabilityReviewV1 {
    let spectral_entropy = semantic_mapping.spectral_entropy;
    let pressure_risk = semantic_mapping.pressure_risk;
    let fluidity_index = semantic_mapping.fluidity_index;
    let foothold_stability = semantic_mapping.foothold_stability;
    let semantic_trickle = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|source| source.components.semantic_trickle.clamp(0.0, 1.0));
    let porosity_gradient = telemetry
        .resonance_density_v1
        .as_ref()
        .and_then(|density| density.components.porosity_gradient)
        .or_else(|| {
            telemetry
                .pressure_source_v1
                .as_ref()
                .map(|source| source.porosity_score.clamp(0.0, 1.0))
        })
        .map(|value| value.clamp(0.0, 1.0));
    let comfort_gate = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| density.components.comfort_gate.clamp(0.0, 1.0));
    let temporal_persistence = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| density.components.temporal_persistence.clamp(0.0, 1.0));

    let permeability_score = if semantic_trickle.is_none()
        && porosity_gradient.is_none()
        && fluidity_index.is_none()
        && foothold_stability.is_none()
        && comfort_gate.is_none()
    {
        None
    } else {
        let delivered = semantic_trickle.unwrap_or(0.50);
        let porosity = porosity_gradient.unwrap_or(0.50);
        let fluidity = fluidity_index.unwrap_or(0.50);
        let foothold = foothold_stability.unwrap_or(0.50);
        let gate = comfort_gate.unwrap_or(0.50);
        Some(
            (delivered * 0.32 + porosity * 0.20 + fluidity * 0.18 + foothold * 0.18 + gate * 0.12)
                .clamp(0.0, 1.0),
        )
    };

    let sieve_leakage_score = spectral_entropy.map(|entropy| {
        let delivered = semantic_trickle.unwrap_or(0.50);
        let permeability = permeability_score.unwrap_or(0.50);
        let pressure = pressure_risk.unwrap_or(0.0);
        let persistence = temporal_persistence.unwrap_or(0.50);
        let missing_trickle = (1.0 - delivered).clamp(0.0, 1.0);
        let support_gap = (1.0 - permeability).clamp(0.0, 1.0);
        (entropy.clamp(0.0, 1.0) * missing_trickle * 0.50
            + support_gap * 0.25
            + pressure.clamp(0.0, 1.0) * 0.15
            + persistence.clamp(0.0, 1.0) * missing_trickle * 0.10)
            .clamp(0.0, 1.0)
    });

    let permeability_state = if spectral_entropy.is_none() && permeability_score.is_none() {
        "insufficient_context"
    } else if sieve_leakage_score.is_some_and(|value| value >= 0.45) {
        "stable_core_sieve_leakage_watch"
    } else if sieve_leakage_score.is_some_and(|value| value >= 0.25) {
        "partial_trickle_loss_watch"
    } else if permeability_score.is_some_and(|value| value >= 0.60) {
        "permeable_delivery_visible"
    } else {
        "bounded_delivery_context"
    };

    let mut evidence = Vec::new();
    if let Some(value) = spectral_entropy {
        evidence.push(format!("spectral_entropy={value:.2}"));
    }
    if let Some(value) = semantic_trickle {
        evidence.push(format!("semantic_trickle={value:.2}"));
    }
    if let Some(value) = porosity_gradient {
        evidence.push(format!("porosity_gradient={value:.2}"));
    }
    if let Some(value) = fluidity_index {
        evidence.push(format!("fluidity_index={value:.2}"));
    }
    if let Some(value) = foothold_stability {
        evidence.push(format!("foothold_stability={value:.2}"));
    }
    if let Some(value) = comfort_gate {
        evidence.push(format!("comfort_gate={value:.2}"));
    }
    if let Some(value) = pressure_risk {
        evidence.push(format!("pressure_risk={value:.2}"));
    }
    if let Some(value) = temporal_persistence {
        evidence.push(format!("temporal_persistence={value:.2}"));
    }
    if matches!(
        permeability_state,
        "stable_core_sieve_leakage_watch" | "partial_trickle_loss_watch"
    ) {
        evidence.push("high_entropy_not_equal_successful_stable_core_delivery".to_string());
    }

    StableCorePermeabilityReviewV1 {
        policy: "stable_core_permeability_review_v1",
        permeability_score,
        sieve_leakage_score,
        permeability_state,
        spectral_entropy,
        semantic_trickle,
        porosity_gradient,
        fluidity_index,
        foothold_stability,
        comfort_gate,
        pressure_risk,
        temporal_persistence,
        evidence,
        authority: "read_only_witness_context_not_semantic_admission_or_control",
    }
}

fn witness_depth_profile_v1(
    telemetry: &crate::types::SpectralTelemetry,
    semantic_mapping: &WitnessSemanticDensityMappingV1,
    stability_effort: &WitnessStabilityEffortV1,
    permeability_review: &StableCorePermeabilityReviewV1,
    eigen_history_sample_count: usize,
    previous_depth: WitnessDepthV1,
) -> WitnessDepthProfileV1 {
    let density_components = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| &density.components);
    let resonance_density = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| density.density.clamp(0.0, 1.0));
    let viscosity_index =
        density_components.map(|components| components.viscosity_index.clamp(0.0, 1.0));
    let mode_packing = semantic_mapping.mode_packing;
    let dynamic_fluidity = density_components
        .and_then(|components| components.dynamic_fluidity_index)
        .or(semantic_mapping.fluidity_index)
        .map(|value| value.clamp(0.0, 1.0));
    let pressure_risk = semantic_mapping.pressure_risk;
    let foothold_stability = semantic_mapping.foothold_stability;
    let density_gradient = semantic_mapping.density_gradient;
    let heavy = resonance_density.is_some_and(|value| value >= 0.65)
        || viscosity_index.is_some_and(|value| value >= 0.55)
        || mode_packing.is_some_and(|value| value >= 0.45);
    let navigable = pressure_risk.is_none_or(|value| value <= 0.32)
        && dynamic_fluidity.is_some_and(|value| value >= 0.45)
        && (foothold_stability.is_some_and(|value| value >= 0.55)
            || density_gradient.is_some_and(|value| value <= 0.25));
    let stagnant = pressure_risk.is_some_and(|value| value >= 0.35)
        || dynamic_fluidity.is_some_and(|value| value <= 0.30)
        || foothold_stability.is_some_and(|value| value <= 0.35);
    let sieve_loss = matches!(
        permeability_review.permeability_state,
        "stable_core_sieve_leakage_watch" | "partial_trickle_loss_watch"
    );
    let high_entropy = semantic_mapping
        .spectral_entropy
        .is_some_and(|value| value >= 0.85);
    let shadow_drift_available = stability_effort.shadow_norm_variance.is_some()
        || stability_effort
            .shadow_dispersal_potential
            .is_some_and(|value| value >= 0.20)
        || stability_effort
            .shadow_class
            .as_deref()
            .is_some_and(|class| {
                class.contains("shifting")
                    || class.contains("restless")
                    || class.contains("disordered")
            });

    let semantic_density_state = if heavy && navigable {
        "heavy_but_navigable"
    } else if heavy && stagnant {
        "heavy_and_stagnant"
    } else if sieve_loss {
        "semantically_occluded_or_leaking"
    } else if high_entropy {
        "high_entropy_complexity"
    } else if semantic_mapping.classification == "insufficient_context" {
        "insufficient_context"
    } else {
        "mixed_or_light_context"
    };

    let eigenmode_count = telemetry.eigenvalues.len();
    let summary_available = !telemetry.eigenvalues.is_empty();
    let texture_field_available = semantic_mapping.classification != "insufficient_context"
        || stability_effort.stability_effort.is_some()
        || permeability_review.permeability_score.is_some();
    let deep_eigenfield_available = eigenmode_count >= 4 && eigen_history_sample_count >= 4;
    let deep_context_warranted = heavy || sieve_loss || high_entropy || shadow_drift_available;
    let selected_depth = if deep_eigenfield_available && deep_context_warranted {
        WitnessDepthV1::DeepEigenfield
    } else if texture_field_available {
        WitnessDepthV1::TextureField
    } else {
        WitnessDepthV1::Summary
    };
    let depth_reason = match (selected_depth, semantic_density_state) {
        (WitnessDepthV1::DeepEigenfield, "heavy_but_navigable") => {
            "resolve_heavy_navigable_mode_structure"
        },
        (WitnessDepthV1::DeepEigenfield, "heavy_and_stagnant") => {
            "resolve_heavy_stagnant_constraint_structure"
        },
        (WitnessDepthV1::DeepEigenfield, "semantically_occluded_or_leaking") => {
            "resolve_stable_core_delivery_loss"
        },
        (WitnessDepthV1::DeepEigenfield, _) if shadow_drift_available => {
            "track_shadow_drift_against_eigenfield_history"
        },
        (WitnessDepthV1::DeepEigenfield, _) => "resolve_high_entropy_mode_structure",
        (WitnessDepthV1::TextureField, _) => "texture_context_available_without_deep_history",
        (WitnessDepthV1::Summary, _) => "bounded_summary_only_context",
    };

    let mut evidence = Vec::new();
    if let Some(value) = resonance_density {
        evidence.push(format!("resonance_density={value:.2}"));
    }
    if let Some(value) = viscosity_index {
        evidence.push(format!("viscosity_index={value:.2}"));
    }
    if let Some(value) = mode_packing {
        evidence.push(format!("mode_packing={value:.2}"));
    }
    if let Some(value) = dynamic_fluidity {
        evidence.push(format!("dynamic_fluidity={value:.2}"));
    }
    if let Some(value) = pressure_risk {
        evidence.push(format!("pressure_risk={value:.2}"));
    }
    if let Some(value) = foothold_stability {
        evidence.push(format!("foothold_stability={value:.2}"));
    }
    if let Some(value) = density_gradient {
        evidence.push(format!("density_gradient={value:.2}"));
    }
    if sieve_loss {
        evidence.push(format!(
            "permeability_state={}",
            permeability_review.permeability_state
        ));
    }
    if shadow_drift_available {
        evidence.push("shadow_drift_evidence_available".to_string());
    }

    WitnessDepthProfileV1 {
        policy: "witness_depth_v1",
        previous_depth,
        selected_depth,
        depth_changed: previous_depth != selected_depth,
        summary_available,
        texture_field_available,
        deep_eigenfield_available,
        semantic_density_state,
        depth_reason,
        eigenmode_count,
        eigen_history_sample_count,
        shadow_drift_available,
        deep_eigenplane_included: selected_depth == WitnessDepthV1::DeepEigenfield,
        evidence,
        control_write: false,
        authority: "read_only_witness_granularity_not_eigenvector_transport_pressure_fill_admission_or_control",
    }
}

fn round_witness_anchor_weight(value: f32) -> f32 {
    (value.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

fn witness_anchor_traction_v1(
    foothold_stability: Option<f32>,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    dispersal_potential: Option<f32>,
) -> WitnessAnchorTractionV1 {
    let foothold = foothold_stability.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let gradient = density_gradient.unwrap_or(0.0).clamp(0.0, 1.0);
    let dispersal = dispersal_potential.unwrap_or(0.0).clamp(0.0, 1.0);

    let foothold_weight = round_witness_anchor_weight(foothold * (1.0 - pressure * 0.50));
    let pressure_weight = round_witness_anchor_weight(pressure * (1.0 - dispersal * 0.25));
    let gradient_weight = round_witness_anchor_weight(gradient.max(dispersal * 0.50));
    let dispersal_weight = round_witness_anchor_weight(dispersal);

    let recommended_anchor = if pressure_weight >= 0.35 && dispersal_weight < 0.30 {
        "pressure"
    } else if gradient_weight >= foothold_weight && gradient_weight >= pressure_weight {
        "gradient"
    } else if dispersal_weight >= 0.40 && pressure_weight >= 0.22 {
        "dispersal"
    } else {
        "foothold"
    };
    let traction_state = match recommended_anchor {
        "pressure" if foothold_weight >= 0.50 => "supported_pressure_navigation",
        "pressure" => "pressure_needs_navigation",
        "gradient" => "gradient_has_traction",
        "dispersal" => "dispersal_can_move_pressure",
        _ => "foothold_can_hold_witness",
    };

    let mut evidence = Vec::new();
    if let Some(value) = foothold_stability {
        evidence.push(format!("foothold_stability={value:.2}"));
    }
    if let Some(value) = pressure_risk {
        evidence.push(format!("pressure_risk={value:.2}"));
    }
    if let Some(value) = density_gradient {
        evidence.push(format!("density_gradient={value:.2}"));
    }
    if let Some(value) = dispersal_potential {
        evidence.push(format!("dispersal_potential={value:.2}"));
    }

    WitnessAnchorTractionV1 {
        recommended_anchor,
        foothold_weight,
        pressure_weight,
        gradient_weight,
        dispersal_weight,
        traction_state,
        evidence,
        authority: "read_only_anchor_legibility_not_prompt_priority_or_control",
    }
}
