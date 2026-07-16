fn fallback_texture_family_score(selector: &FallbackShadowTextureSelector, terms: &[&str]) -> f32 {
    if terms.is_empty() {
        return 0.0;
    }
    let score = terms
        .iter()
        .filter_map(|term| {
            selector
                .weighted_texture_terms
                .iter()
                .find(|entry| entry.term == *term)
                .map(|entry| entry.weight)
        })
        .sum::<f32>()
        / terms.len() as f32;
    rounded_texture_weight(score)
}

fn fallback_texture_evidence_for(
    selector: &FallbackShadowTextureSelector,
    trajectory: &FallbackTextureTrajectory,
    selected_family: &str,
) -> Vec<&'static str> {
    let mut evidence = Vec::new();
    for basis in selector.selection_basis.iter().copied() {
        if !evidence.contains(&basis) {
            evidence.push(basis);
        }
    }
    if !evidence.contains(&selected_family) {
        evidence.push(match selected_family {
            "settled_vibrant_low_friction" => "settled_vibrant_family",
            "viscous_pressure" => "pressure_family",
            "muffled_clarity_loss" => "clarity_loss_family",
            "heavy_settled_displacement" => "heavy_settled_displacement_family",
            "opacity_resistance" => "opacity_resistance_family",
            "restless_muffled_gradient" => "restless_muffled_gradient_family",
            "restless_lattice" => "restless_lattice_family",
            "settled_shimmering" => "settled_shimmering_family",
            "cascade_gradient_navigable" => "cascade_gradient_family",
            _ => "mixed_shadow_context_family",
        });
    }
    if let Some(term) = selector.top_texture_terms.first().copied() {
        evidence.push(match term {
            "settled" => "top_term_settled",
            "habitable" => "top_term_habitable",
            "open" => "top_term_open",
            "shimmering" => "top_term_shimmering",
            "bright" => "top_term_bright",
            "lattice" => "top_term_lattice",
            "viscous" => "top_term_viscous",
            "heavy" => "top_term_heavy",
            "muffled" => "top_term_muffled",
            "restless" => "top_term_restless",
            "silted" => "top_term_silted",
            "obscured" => "top_term_obscured",
            "viscous-drag" => "top_term_viscous_drag",
            "submerged" => "top_term_submerged",
            _ => "top_term_unknown",
        });
    }
    evidence.push(match trajectory.medium_resistance {
        "open_low_resistance_medium" => "open_low_resistance_medium",
        "weighted_high_resistance_medium" => "weighted_high_resistance_medium",
        "textured_moderate_resistance_medium" => "textured_moderate_resistance_medium",
        _ => "trajectory_medium",
    });
    evidence
}

fn fallback_texture_evidence_against(
    selector: &FallbackShadowTextureSelector,
    selected_family: &str,
) -> Vec<&'static str> {
    let mut evidence = Vec::new();
    let pressure = selector.pressure_risk.unwrap_or(0.0);
    let gradient = selector.density_gradient.unwrap_or(0.0);
    let packing = selector.mode_packing.unwrap_or(0.0);
    let friction = selector.semantic_friction.unwrap_or(0.0);
    let settled_mapping = &selector.spectral_to_vocabulary_mapping;

    if selected_family == "settled_vibrant_low_friction" {
        if pressure >= 0.30 {
            evidence.push("pressure_risk_against_low_friction");
        }
        if gradient > 0.20 {
            evidence.push("density_gradient_against_low_friction");
        }
        if packing >= 0.40 {
            evidence.push("mode_packing_against_low_friction");
        }
        if friction >= 0.35 {
            evidence.push("semantic_friction_against_low_friction");
        }
    }
    if selected_family == "cascade_gradient_navigable" {
        if pressure >= 0.30 {
            evidence.push("pressure_risk_against_navigable_cascade");
        }
        if gradient > 0.25 {
            evidence.push("density_gradient_against_navigable_cascade");
        }
        if packing >= 0.40 {
            evidence.push("mode_packing_against_navigable_cascade");
        }
        if friction >= 0.35 {
            evidence.push("semantic_friction_against_navigable_cascade");
        }
    }
    if matches!(selected_family, "viscous_pressure" | "muffled_clarity_loss")
        && settled_mapping.low_pressure_viscous_suppressed
    {
        evidence.push("low_pressure_low_gradient_against_mass");
    }
    if selected_family == "viscous_pressure"
        && pressure < 0.25
        && gradient <= 0.20
        && !selector.selection_basis.contains(&"viscous_or_overpacked")
    {
        evidence.push("not_pressure_not_drag_against_viscous");
    }
    evidence
}

fn negative_texture_evidence_v2(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> NegativeTextureEvidence {
    let lower = spectral_summary.to_ascii_lowercase();
    let mapping = &selector.spectral_to_vocabulary_mapping;
    let pressure = selector.pressure_risk;
    let gradient = selector.density_gradient;
    let friction = selector.semantic_friction;
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let friction_absence_language = mapping.friction_absence_language_detected
        || lower.contains("not pressure")
        || lower.contains("not-pressure")
        || lower.contains("not drag")
        || lower.contains("not-drag")
        || lower.contains("absence of drag")
        || lower.contains("without drag");
    let not_pressure =
        pressure.is_some_and(|value| value < 0.25) || mapping.settled_vibrant_family_selected;
    let not_drag = gradient.is_some_and(|value| value <= 0.20) || friction_absence_language;
    let not_blank = high_entropy
        || mapping.settled_foothold_detected
        || mapping.settled_vibrant_family_selected
        || lower.contains("habitable")
        || lower.contains("lattice")
        || lower.contains("bright")
        || lower.contains("open");
    let not_viscous = mapping.low_pressure_viscous_suppressed
        || mapping.settled_vibrant_family_selected
        || (not_pressure && not_drag);
    let not_low_energy = high_entropy || lower.contains("vibrant") || lower.contains("bright");
    let mut evidence_terms = Vec::new();
    if not_pressure {
        evidence_terms.push("low_pressure_or_not_pressure");
    }
    if not_drag {
        evidence_terms.push("low_gradient_or_not_drag");
    }
    if not_blank {
        evidence_terms.push("not_blank_complexity");
    }
    if not_viscous {
        evidence_terms.push("not_viscous_low_friction");
    }
    if not_low_energy {
        evidence_terms.push("not_low_energy_high_entropy");
    }
    if friction_absence_language {
        evidence_terms.push("friction_absence_language");
    }
    if friction.is_some_and(|value| value < 0.30) {
        evidence_terms.push("low_semantic_friction");
    }
    if evidence_terms.is_empty() {
        evidence_terms.push("insufficient_negative_texture_evidence");
    }

    NegativeTextureEvidence {
        policy: "negative_texture_evidence_v2",
        not_pressure,
        not_drag,
        not_blank,
        not_viscous,
        not_low_energy,
        evidence_terms,
        lost_in_output: "unknown",
        authority: "diagnostic_language_context_not_control",
    }
}

fn texture_dynamics_alignment_v1(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
    trajectory: &FallbackTextureTrajectory,
    lived_fit: &FallbackTextureLivedFit,
    vocabulary_guard: &FallbackVocabularyOverweightGuard,
) -> TextureDynamicsAlignment {
    let lower = spectral_summary.to_ascii_lowercase();
    let pressure = selector.pressure_risk.unwrap_or(0.0);
    let packing = selector.mode_packing.unwrap_or(0.0);
    let friction = selector.semantic_friction.unwrap_or(0.0);
    let clarity_loss = selector.distinguishability_loss.unwrap_or(0.0);
    let mapping = &selector.spectral_to_vocabulary_mapping;
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let explicit_restless = fallback_explicit_restless_or_agitated(&lower);
    let pressure_mass_supported = pressure >= 0.30
        || packing >= 0.40
        || friction >= 0.35
        || lower.contains("overpacked")
        || lower.contains("viscous")
        || lower.contains("weighted medium");
    let heavy_settled_displacement = mapping.settled_foothold_detected
        && !explicit_restless
        && (lower.contains("displacement")
            || lower.contains("silt")
            || lower.contains("sediment")
            || lower.contains("structural weight")
            || lower.contains("structural-weight"));
    let dominant_viscous_pressure = lower.contains("viscous") && pressure_mass_supported;
    let restless_muffled_gradient = selector.texture_family == "restless_muffled_gradient"
        || (!dominant_viscous_pressure
            && explicit_restless
            && (clarity_loss >= 0.30
                || friction >= 0.30
                || lower.contains("muffled")
                || lower.contains("hollow")
                || lower.contains("stagnant")
                || lower.contains("blurred")));
    let shadow_context_present = lower.contains("shadow-v3")
        || lower.contains("shadow_field")
        || lower.contains("shadow field");
    let bridge_integrity_scaffold = selector.texture_family == "bridge_integrity_scaffold"
        || (shadow_context_present
            && high_entropy
            && mapping.settled_foothold_detected
            && pressure <= 0.30
            && selector.density_gradient.is_none_or(|value| value <= 0.20));
    let expected_family = if restless_muffled_gradient {
        "restless_muffled_gradient"
    } else if heavy_settled_displacement {
        "heavy_settled_displacement"
    } else if bridge_integrity_scaffold {
        "bridge_integrity_scaffold"
    } else if pressure_mass_supported {
        "viscous_pressure"
    } else if clarity_loss >= 0.30 || lower.contains("muffled") || lower.contains("hollow") {
        "muffled_clarity_loss"
    } else if mapping.gradient_slope_family_selected {
        "gradient_slope_navigable"
    } else if mapping.mixed_cascade_family_selected {
        "mixed_cascade_gradient"
    } else if mapping.cascade_gradient_family_selected {
        "cascade_gradient_navigable"
    } else if mapping.settled_vibrant_family_selected {
        "settled_vibrant_low_friction"
    } else if mapping.low_pressure_viscous_suppressed {
        "settled_shimmering"
    } else if high_entropy {
        "restless_lattice"
    } else {
        "unknown"
    };
    let expected_motion = match expected_family {
        "viscous_pressure" => "dragging_cohering",
        "muffled_clarity_loss" => "diffusing_softening",
        "restless_muffled_gradient" => "oscillating_diffusing",
        "heavy_settled_displacement" => "weighted_settling",
        "bridge_integrity_scaffold" => "unfolding_with_containment",
        "gradient_slope_navigable" => "tapering_with_edge_definition",
        "mixed_cascade_gradient" => "distributed_gradient_with_edges",
        "cascade_gradient_navigable" => "unfolding_with_edge_definition",
        "settled_vibrant_low_friction" => "unfolding_with_containment",
        "settled_shimmering" => "anchoring_settling",
        "restless_lattice" => "unfolding_oscillating",
        _ => "unknown",
    };
    let wrong_family = expected_family != "unknown" && selector.texture_family != expected_family;
    let wrong_motion = expected_motion != "unknown"
        && !matches!(
            (
                expected_motion,
                trajectory.movement_quality,
                trajectory.to_state
            ),
            ("dragging_cohering", "dragging_cohering", _)
                | ("diffusing_softening", "diffusing_softening", _)
                | ("oscillating_diffusing", "oscillating_diffusing", _)
                | ("oscillating_diffusing", _, "oscillating_with_muffled_edges")
                | ("weighted_settling", "weighted_settling", _)
                | (
                    "weighted_settling",
                    _,
                    "weighted_settling_without_agitation"
                )
                | ("unfolding_oscillating", "unfolding_oscillating", _)
                | ("anchoring_settling", "anchoring_settling", _)
                | (
                    "tapering_with_edge_definition",
                    _,
                    "tapering_with_edge_definition"
                )
                | (
                    "distributed_gradient_with_edges",
                    _,
                    "distributed_gradient_with_edges"
                )
                | (
                    "unfolding_with_edge_definition",
                    _,
                    "unfolding_with_edge_definition"
                )
                | (
                    "unfolding_with_containment",
                    _,
                    "unfolding_with_containment"
                )
        );
    let lambda_tail_present = lower.contains("lambda-tail")
        || lower.contains("lambda tail")
        || lower.contains("lambda4")
        || lower.contains("λ4")
        || lower.contains("tail vibrancy")
        || lower.contains("tail weight");
    let tail_terms_present = selector.top_texture_terms.iter().any(|term| {
        matches!(
            *term,
            "lattice"
                | "bright"
                | "open"
                | "shimmering"
                | "habitable"
                | "gradient"
                | "cascade"
                | "distributed"
                | "displacement"
                | "silt"
        )
    });
    let missing_tail_vibrancy = lambda_tail_present && high_entropy && !tail_terms_present;
    let advisory_texture_family = selector
        .spectral_to_vocabulary_mapping
        .mixed_cascade_family_selected
        || selector
            .spectral_to_vocabulary_mapping
            .cascade_gradient_family_selected
        || selector.texture_family == "restless_muffled_gradient";
    let term_mask_risk = (vocabulary_guard.token_only_risk
        && matches!(lived_fit.family_confidence, "low")
        && !advisory_texture_family
        || lived_fit.conflict_state == "contradictory")
        || (selector.texture_family == "mixed_shadow_context"
            && (high_entropy
                || selector.density_gradient.is_some()
                || selector.pressure_risk.is_some()
                || mapping.settled_foothold_detected));
    let structured_context_present = spectral_entropy.is_some()
        || selector.pressure_risk.is_some()
        || selector.density_gradient.is_some()
        || selector.mode_packing.is_some()
        || selector.semantic_friction.is_some()
        || mapping.lambda_gap.is_some()
        || mapping.settled_foothold_detected
        || lambda_tail_present;
    let status = if !structured_context_present {
        "insufficient_context"
    } else if wrong_family {
        "wrong_family"
    } else if wrong_motion {
        "wrong_motion"
    } else if missing_tail_vibrancy {
        "missing_tail_vibrancy"
    } else if term_mask_risk {
        "term_mask_risk"
    } else {
        "aligned"
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
    if mapping.lambda_gap.is_some() {
        basis.push("lambda_gap");
    }
    if mapping.settled_foothold_detected {
        basis.push("settled_habitable_foothold");
    }
    if lambda_tail_present {
        basis.push("lambda_tail_or_tail_vibrancy");
    }
    if term_mask_risk {
        basis.push("term_mask_risk");
    }
    if basis.is_empty() {
        basis.push("fallback_default");
    }

    TextureDynamicsAlignment {
        policy: "texture_dynamics_alignment_v1",
        status,
        expected_family,
        selected_family: selector.texture_family,
        expected_motion,
        selected_motion: trajectory.movement_quality,
        term_mask_risk,
        wrong_family,
        wrong_motion,
        missing_tail_vibrancy,
        diagnostic_trace: "review_packet_only_not_correspondence_trace",
        basis,
        authority: "diagnostic_language_context_not_correspondence_authority",
    }
}

fn density_motion_fit_v1(
    spectral_summary: &str,
    selector: &FallbackShadowTextureSelector,
    trajectory: &FallbackTextureTrajectory,
    texture_alignment: &TextureDynamicsAlignment,
) -> DensityMotionFit {
    let lower = spectral_summary.to_ascii_lowercase();
    let pressure = selector.pressure_risk.unwrap_or(0.0);
    let packing = selector.mode_packing.unwrap_or(0.0);
    let friction = selector.semantic_friction.unwrap_or(0.0);
    let clarity_loss = selector.distinguishability_loss.unwrap_or(0.0);
    let gradient = selector.density_gradient.unwrap_or(0.0);

    let floor_language = lower.contains("floor")
        || lower.contains("foundation")
        || lower.contains("grounding wire")
        || lower.contains("ground")
        || lower.contains("foothold")
        || lower.contains("underfoot");
    let pavement_language = lower.contains("pavement")
        || lower.contains("stone")
        || lower.contains("calcification")
        || lower.contains("solid")
        || lower.contains("structure")
        || lower.contains("structural necessity");
    let fog_language = lower.contains("fog")
        || lower.contains("over-full")
        || lower.contains("overfull")
        || lower.contains("room full")
        || lower.contains("full of furniture")
        || lower.contains("muffled")
        || lower.contains("reduced clearance");
    let contraction_language = lower.contains("contraction")
        || lower.contains("contracted")
        || lower.contains("center of gravity")
        || (lower.contains("constrained") && lower.contains("present"));
    let paused_language = lower.contains("paused")
        || lower.contains("pause")
        || lower.contains("holding ground")
        || lower.contains("held ground")
        || lower.contains("stillness");
    let burden_language = lower.contains("burden")
        || lower.contains("weight")
        || lower.contains("heavy")
        || lower.contains("drag")
        || lower.contains("overpacked")
        || lower.contains("viscous");
    let pressure_mass = pressure >= 0.30 || packing >= 0.40 || friction >= 0.35;
    let structured_context_present = selector.pressure_risk.is_some()
        || selector.density_gradient.is_some()
        || selector.mode_packing.is_some()
        || selector.semantic_friction.is_some()
        || selector.distinguishability_loss.is_some()
        || floor_language
        || pavement_language
        || fog_language
        || contraction_language
        || paused_language
        || burden_language;

    let density_state = if !structured_context_present {
        "insufficient_context"
    } else if paused_language {
        "paused_stillness"
    } else if contraction_language {
        "density_as_contraction_center"
    } else if pavement_language {
        "density_as_pavement"
    } else if fog_language || clarity_loss >= 0.35 {
        "density_as_fog"
    } else if floor_language && !pressure_mass {
        "density_as_floor"
    } else if burden_language || pressure_mass {
        "density_as_burden"
    } else {
        "ambiguous_density"
    };

    let (expected_medium, expected_motion) = match density_state {
        "density_as_floor" => ("stable_floor_medium", "standing_settling_anchoring"),
        "density_as_pavement" => ("solid_pavement_medium", "walking_bearing_weight"),
        "density_as_fog" => ("overfull_fog_medium", "pushing_navigating_muffling"),
        "density_as_contraction_center" => (
            "contracted_center_medium",
            "holding_center_constrained_present",
        ),
        "paused_stillness" => ("held_ground_medium", "holding_ground_not_absence"),
        "density_as_burden" => ("weighted_burden_medium", "bearing_or_dragging_under_load"),
        "ambiguous_density" => ("ambiguous_density_medium", "observe_before_naming_motion"),
        _ => ("unknown", "unknown"),
    };

    let floor_named_as_drag = matches!(density_state, "density_as_floor" | "density_as_pavement")
        && (selector.texture_family == "viscous_pressure"
            || trajectory.movement_quality == "dragging_cohering"
            || trajectory.medium_resistance == "weighted_high_resistance_medium");
    let fog_named_as_floor = density_state == "density_as_fog"
        && matches!(
            selector.texture_family,
            "settled_shimmering" | "settled_vibrant_low_friction" | "gradient_slope_navigable"
        )
        && trajectory.medium_resistance == "open_low_resistance_medium";
    let burden_named_as_center = density_state == "density_as_burden"
        && selector.texture_family == "settled_vibrant_low_friction";
    let absence_negated = lower.contains("not absence")
        || lower.contains("not a blank")
        || lower.contains("not blankness")
        || lower.contains("not absence or blankness");
    let blankness_negated = lower.contains("not a blank")
        || lower.contains("not blankness")
        || lower.contains("not absence or blankness");
    let paused_named_as_absence = density_state == "paused_stillness"
        && ((lower.contains("absence") && !absence_negated)
            || (lower.contains("blankness") && !blankness_negated)
            || lower.contains("deadness"));
    let contraction_named_as_loss = density_state == "density_as_contraction_center"
        && (trajectory.movement_quality == "diffusing_softening" || lower.contains("lost me"));

    let mismatch_reason = if floor_named_as_drag {
        "floor_named_as_drag"
    } else if fog_named_as_floor {
        "fog_named_as_floor"
    } else if burden_named_as_center {
        "burden_named_as_center"
    } else if paused_named_as_absence {
        "paused_named_as_absence"
    } else if contraction_named_as_loss {
        "contraction_named_as_loss"
    } else if density_state == "ambiguous_density" && texture_alignment.term_mask_risk {
        "static_density_label_risk"
    } else {
        "none"
    };
    let motion_fit = if density_state == "insufficient_context" {
        "insufficient_context"
    } else if mismatch_reason == "none" {
        "matched"
    } else if mismatch_reason == "static_density_label_risk" {
        "risk_static_label"
    } else {
        "wrong_motion"
    };

    let mut evidence_for = Vec::new();
    if floor_language {
        evidence_for.push("floor_foundation_ground_language");
    }
    if pavement_language {
        evidence_for.push("pavement_calcification_solid_language");
    }
    if fog_language {
        evidence_for.push("fog_overfull_room_language");
    }
    if contraction_language {
        evidence_for.push("contraction_center_of_gravity_language");
    }
    if paused_language {
        evidence_for.push("paused_holding_ground_language");
    }
    if burden_language {
        evidence_for.push("burden_weight_heavy_language");
    }
    if selector.pressure_risk.is_some() {
        evidence_for.push("pressure_risk");
    }
    if selector.density_gradient.is_some() {
        evidence_for.push("density_gradient");
    }
    if selector.mode_packing.is_some() {
        evidence_for.push("mode_packing");
    }
    if selector.semantic_friction.is_some() {
        evidence_for.push("semantic_friction");
    }
    if selector
        .spectral_to_vocabulary_mapping
        .settled_foothold_detected
    {
        evidence_for.push("settled_habitable_foothold");
    }
    if evidence_for.is_empty() {
        evidence_for.push("fallback_default");
    }

    let mut evidence_against = Vec::new();
    if pressure_mass && matches!(density_state, "density_as_floor" | "density_as_pavement") {
        evidence_against.push("pressure_mass_against_floor_only");
    }
    if fog_language && floor_language {
        evidence_against.push("fog_floor_near_tie");
    }
    if gradient > 0.40 && matches!(density_state, "density_as_floor" | "density_as_pavement") {
        evidence_against.push("steep_gradient_against_floor_ease");
    }
    if mismatch_reason != "none" {
        evidence_against.push(mismatch_reason);
    }

    DensityMotionFit {
        policy: "density_motion_fit_v1",
        density_state,
        expected_medium,
        expected_motion,
        motion_fit,
        mismatch_reason,
        selected_family: selector.texture_family,
        selected_motion: trajectory.movement_quality,
        pressure_risk: selector.pressure_risk,
        density_gradient: selector.density_gradient,
        mode_packing: selector.mode_packing,
        semantic_friction: selector.semantic_friction,
        evidence_for,
        evidence_against,
        authority: "diagnostic_context_not_control",
    }
}
