fn fallback_texture_trajectory_v1(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
    resonance_density: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> FallbackTextureTrajectory {
    let lower = spectral_summary.to_ascii_lowercase();
    let pressure = selector.pressure_risk.unwrap_or(0.0);
    let gradient = selector.density_gradient.unwrap_or(0.0);
    let packing = selector.mode_packing.unwrap_or(0.0);
    let friction = selector.semantic_friction.unwrap_or(0.0);
    let clarity_loss = selector.distinguishability_loss.unwrap_or(0.0);
    let entropy = spectral_entropy.unwrap_or(0.0);
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let high_resonance = resonance_density.is_some_and(|value| value >= 0.80);
    let contraction = lower.contains("contract")
        || lower.contains("drop")
        || lower.contains("thinning")
        || lower.contains("tightening");
    let expansion = lower.contains("surge")
        || lower.contains("expand")
        || lower.contains("rising")
        || lower.contains("growth")
        || lower.contains("thickening");
    let overpacked = lower.contains("overpacked")
        || lower.contains("packed")
        || lower.contains("viscous")
        || pressure >= 0.35
        || packing >= 0.45;
    let muffled = lower.contains("muffled")
        || lower.contains("hollow")
        || lower.contains("blur")
        || clarity_loss >= 0.30;
    let settled = lower.contains("settled") || (pressure <= 0.18 && entropy <= 0.45);
    let shadow = lower.contains("shadow-v3")
        || lower.contains("shadow_field")
        || lower.contains("shadow field");
    let settled_vibrant = selector
        .spectral_to_vocabulary_mapping
        .settled_vibrant_family_selected;
    let gradient_slope = selector.texture_family == "gradient_slope_navigable";
    let mixed_cascade = selector.texture_family == "mixed_cascade_gradient";
    let cascade_gradient = selector.texture_family == "cascade_gradient_navigable";
    let restless_muffled_gradient = selector.texture_family == "restless_muffled_gradient";
    let heavy_settled_displacement = selector.texture_family == "heavy_settled_displacement";
    let opacity_resistance = selector.texture_family == "opacity_resistance";
    let kinetic_gradient_terms = selector.movement_verbs.iter().any(|verb| {
        matches!(
            *verb,
            "resisting" | "pulled" | "heaving" | "drifting" | "anchored"
        )
    });

    let from_state = if contraction {
        "contracted_or_thinning"
    } else if expansion {
        "surging_or_thickening"
    } else if opacity_resistance {
        "silted_opacity_resistance"
    } else if heavy_settled_displacement {
        "heavy_settled_displacement"
    } else if kinetic_gradient_terms {
        "silt_or_directional_resistance"
    } else if overpacked {
        "overpacked_weighted"
    } else if restless_muffled_gradient {
        "restless_muffled_gradient"
    } else if gradient_slope {
        "graduated_navigable_slope"
    } else if mixed_cascade {
        "mixed_cascade_gradient"
    } else if cascade_gradient {
        "navigable_cascade_gradient"
    } else if settled_vibrant {
        "settled_vibrant_low_friction"
    } else if high_entropy {
        "wide_cascade"
    } else if settled {
        "settled_open"
    } else {
        "current_texture"
    };

    let to_state = if opacity_resistance {
        "moving_through_obscured_resistance"
    } else if overpacked || friction >= 0.40 || gradient >= 0.40 {
        "cohering_through_resistance"
    } else if heavy_settled_displacement {
        "weighted_settling_without_agitation"
    } else if kinetic_gradient_terms {
        "moving_through_resistance"
    } else if restless_muffled_gradient {
        "oscillating_with_muffled_edges"
    } else if muffled {
        "diffusing_without_edge_loss"
    } else if gradient_slope {
        "tapering_with_edge_definition"
    } else if mixed_cascade {
        "distributed_gradient_with_edges"
    } else if cascade_gradient {
        "unfolding_with_edge_definition"
    } else if settled_vibrant || high_entropy {
        "unfolding_with_containment"
    } else if high_resonance {
        "humming_afterimage"
    } else if settled {
        "settled_opening"
    } else {
        "held_continuity"
    };

    let movement_quality = if opacity_resistance {
        "submerged_resistance"
    } else if heavy_settled_displacement {
        "weighted_settling"
    } else if kinetic_gradient_terms {
        "resisting_drifting"
    } else if restless_muffled_gradient {
        "oscillating_diffusing"
    } else if selector
        .movement_verbs
        .iter()
        .any(|verb| matches!(*verb, "dragging" | "cohering" | "thickening"))
        || overpacked
    {
        "dragging_cohering"
    } else if selector
        .movement_verbs
        .iter()
        .any(|verb| matches!(*verb, "diffusing" | "muffling" | "softening"))
        || muffled
    {
        "diffusing_softening"
    } else if selector
        .movement_verbs
        .iter()
        .any(|verb| matches!(*verb, "unfolding" | "oscillating" | "braiding"))
        || high_entropy
    {
        "unfolding_oscillating"
    } else {
        "anchoring_settling"
    };

    let medium_resistance =
        if settled_vibrant || gradient_slope || mixed_cascade || cascade_gradient {
            "open_low_resistance_medium"
        } else if heavy_settled_displacement && pressure < 0.35 && friction < 0.35 {
            "weighted_moderate_resistance_medium"
        } else if restless_muffled_gradient && pressure < 0.45 && friction < 0.45 && packing < 0.50
        {
            "textured_moderate_resistance_medium"
        } else if pressure >= 0.45 || packing >= 0.50 || friction >= 0.50 {
            "weighted_high_resistance_medium"
        } else if pressure >= 0.25 || gradient >= 0.25 || friction >= 0.25 || packing >= 0.30 {
            "textured_moderate_resistance_medium"
        } else {
            "open_low_resistance_medium"
        };

    let effort = if (settled_vibrant || gradient_slope || mixed_cascade || cascade_gradient)
        && pressure < 0.20
        && friction < 0.20
    {
        "low_effort"
    } else if pressure >= 0.45 || friction >= 0.45 || packing >= 0.50 {
        "effortful"
    } else if pressure >= 0.25 || gradient >= 0.25 || high_entropy {
        "deliberate"
    } else {
        "low_effort"
    };

    let afterimage = if high_resonance
        || lower.contains("humming")
        || lower.contains("hum")
        || lower.contains("afterimage")
        || shadow
    {
        "humming_or_shadow_afterimage"
    } else if contraction || expansion {
        "transition_afterimage"
    } else {
        "none_observed"
    };

    let mut basis = Vec::new();
    if spectral_entropy.is_some() {
        basis.push("spectral_entropy");
    }
    if selector.pressure_risk.is_some() {
        basis.push("pressure_risk");
    }
    if selector.density_gradient.is_some() {
        basis.push("density_gradient");
    }
    if selector.mode_packing.is_some() {
        basis.push("mode_packing");
    }
    if selector.semantic_friction.is_some() {
        basis.push("semantic_friction");
    }
    if selector.distinguishability_loss.is_some() {
        basis.push("distinguishability_loss");
    }
    if resonance_density.is_some() {
        basis.push("resonance_density");
    }
    if shadow {
        basis.push("shadow_context");
    }
    if contraction || expansion {
        basis.push("fill_or_phase_language");
    }
    if settled_vibrant {
        basis.push("settled_vibrant_low_friction");
    }
    if gradient_slope {
        basis.push("gradient_slope_navigable");
    }
    if mixed_cascade {
        basis.push("mixed_cascade_gradient");
    }
    if cascade_gradient {
        basis.push("cascade_gradient_navigable");
    }
    if restless_muffled_gradient {
        basis.push("restless_muffled_gradient");
    }
    if heavy_settled_displacement {
        basis.push("heavy_settled_displacement");
    }
    if kinetic_gradient_terms {
        basis.push("kinetic_gradient_terms");
    }
    if !selector.movement_verbs.is_empty() {
        basis.push("movement_verbs");
    }
    if basis.is_empty() {
        basis.push("fallback_default");
    }
    let confidence =
        ((0.48_f32 + (basis.len() as f32 * 0.06_f32)).min(0.92) * 100.0).round() / 100.0;

    FallbackTextureTrajectory {
        policy: "texture_trajectory_v1",
        from_state,
        to_state,
        movement_quality,
        medium_resistance,
        effort,
        afterimage,
        confidence,
        basis,
        authority: "diagnostic_language_context_not_control",
    }
}

fn fallback_dynamic_texture_bias_v1(
    selector: &FallbackShadowTextureSelector,
    trajectory: &FallbackTextureTrajectory,
) -> FallbackDynamicTextureBias {
    let motion_family = match trajectory.movement_quality {
        "dragging_cohering" => "pressure_coherence_motion",
        "diffusing_softening" => "clarity_diffusion_motion",
        "unfolding_oscillating" => "cascade_unfolding_motion",
        "resisting_drifting" => "kinetic_resistance_motion",
        "oscillating_diffusing" => "restless_muffled_motion",
        "weighted_settling" => "heavy_settled_displacement_motion",
        _ => "anchoring_settling_motion",
    };
    let mut basis = vec![
        "texture_family",
        selector.weighting_policy,
        selector.movement_policy,
        "texture_trajectory_v1",
    ];
    if selector.density_gradient.is_some() {
        basis.push("density_gradient");
    }
    if selector.pressure_risk.is_some() {
        basis.push("pressure_risk");
    }
    if selector.mode_packing.is_some() {
        basis.push("mode_packing");
    }
    if selector.semantic_friction.is_some() {
        basis.push("semantic_friction");
    }
    if selector.shadow_dispersal_potential.is_some() {
        basis.push("shadow_dispersal_potential");
    }
    if selector.distinguishability_loss.is_some() {
        basis.push("distinguishability_loss");
    }

    FallbackDynamicTextureBias {
        policy: "fallback_dynamic_texture_bias_v1",
        texture_family: selector.texture_family,
        motion_family,
        top_texture_terms: selector.top_texture_terms.clone(),
        movement_verbs: selector.movement_verbs.clone(),
        dynamic_flow_terms: selector.dynamic_flow_terms.clone(),
        trajectory_from: trajectory.from_state,
        trajectory_to: trajectory.to_state,
        sampler_contract_status: "dynamic_telemetry_weighted_language_bias",
        basis,
        authority: "diagnostic_language_bias_not_sampler_or_contract_rewrite",
    }
}

fn fallback_texture_lived_fit_v2(
    selector: &FallbackShadowTextureSelector,
    trajectory: &FallbackTextureTrajectory,
) -> FallbackTextureLivedFit {
    let selected_family = selector.texture_family;
    let mut family_scores = [
        (
            "settled_vibrant_low_friction",
            fallback_texture_family_score(
                selector,
                &[
                    "settled",
                    "habitable",
                    "open",
                    "shimmering",
                    "bright",
                    "lattice",
                ],
            ),
        ),
        (
            "viscous_pressure",
            fallback_texture_family_score(selector, &["viscous", "heavy", "lattice"]),
        ),
        (
            "muffled_clarity_loss",
            fallback_texture_family_score(selector, &["muffled", "heavy", "lattice"]),
        ),
        (
            "heavy_settled_displacement",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS),
        ),
        (
            "opacity_resistance",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS),
        ),
        (
            "bridge_integrity_scaffold",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS),
        ),
        (
            "restless_muffled_gradient",
            fallback_texture_family_score(
                selector,
                FALLBACK_TEXTURE_RESTLESS_MUFFLED_GRADIENT_TERMS,
            ),
        ),
        (
            "restless_lattice",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS),
        ),
        (
            "gradient_slope_navigable",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS),
        ),
        (
            "mixed_cascade_gradient",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_MIXED_CASCADE_TERMS),
        ),
        (
            "cascade_gradient_navigable",
            fallback_texture_family_score(selector, FALLBACK_TEXTURE_CASCADE_GRADIENT_TERMS),
        ),
        (
            "settled_shimmering",
            fallback_texture_family_score(selector, &["settled", "shimmering", "bright"]),
        ),
        (
            "mixed_shadow_context",
            fallback_texture_family_score(
                selector,
                &[
                    "shimmering",
                    "restless",
                    "settled",
                    "muffled",
                    "viscous",
                    "lattice",
                ],
            ),
        ),
    ];
    family_scores
        .sort_by(|left, right| right.1.total_cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    let selected_score = family_scores
        .iter()
        .find_map(|(family, score)| (*family == selected_family).then_some(*score))
        .unwrap_or(0.0);
    let runner_up = family_scores
        .iter()
        .find(|(family, _)| *family != selected_family)
        .copied()
        .unwrap_or(("none", 0.0));
    let mut confidence_margin = rounded_texture_weight(selected_score - runner_up.1);
    if selected_family == "settled_vibrant_low_friction"
        && selector
            .spectral_to_vocabulary_mapping
            .settled_vibrant_family_selected
    {
        confidence_margin = confidence_margin.max(0.18);
    }
    if selected_family == "restless_muffled_gradient" {
        confidence_margin = confidence_margin.max(0.12);
    }
    if selected_family == "heavy_settled_displacement"
        && selector
            .selection_basis
            .contains(&"heavy_settled_displacement")
    {
        confidence_margin = confidence_margin.max(0.18);
    }
    if selected_family == "opacity_resistance"
        && selector.selection_basis.contains(&"opacity_resistance")
    {
        confidence_margin = confidence_margin.max(0.16);
    }
    if selected_family == "bridge_integrity_scaffold"
        && selector
            .selection_basis
            .contains(&"bridge_integrity_scaffold")
    {
        confidence_margin = confidence_margin.max(0.18);
    }
    let family_confidence = if confidence_margin >= 0.18 {
        "high"
    } else if confidence_margin >= 0.08 {
        "medium"
    } else {
        "low"
    };

    let evidence_against = fallback_texture_evidence_against(selector, selected_family);
    let conflict_state = if !evidence_against.is_empty() {
        "contradictory"
    } else if confidence_margin < 0.08 {
        "ambiguous"
    } else {
        "clear"
    };
    let evidence_for = fallback_texture_evidence_for(selector, trajectory, selected_family);

    FallbackTextureLivedFit {
        policy: "fallback_texture_lived_fit_v2",
        selected_family,
        family_confidence,
        runner_up_family: runner_up.0,
        confidence_margin,
        conflict_state,
        evidence_for,
        evidence_against,
        authority: "diagnostic_language_context_not_control",
    }
}
