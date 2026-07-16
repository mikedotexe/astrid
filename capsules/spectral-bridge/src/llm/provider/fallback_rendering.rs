fn fallback_continuity_budget_prompt_line(budget: FallbackContinuityBudget) -> String {
    let entropy = budget
        .spectral_entropy
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "unknown".to_string());
    let resonance_clause = budget
        .resonance_density
        .map(|density| {
            format!(
                " resonance_density={density:.2}; resonance_descriptor_policy={};",
                budget.resonance_descriptor_policy
            )
        })
        .unwrap_or_default();
    let shadow_anchor = budget.fallback_shadow_texture_anchor;
    let texture_selector = budget.fallback_shadow_texture_selector;
    let spectral_mapping = texture_selector.spectral_to_vocabulary_mapping.clone();
    let texture_trajectory = budget.texture_trajectory;
    let lived_fit = budget.fallback_texture_lived_fit;
    let negative_evidence = budget.negative_texture_evidence;
    let cascade_gradient = budget.fallback_cascade_gradient;
    let gradient_slope = budget.fallback_gradient_slope;
    let vocabulary_guard = budget.fallback_vocabulary_overweight_guard;
    let texture_alignment = budget.texture_dynamics_alignment;
    let density_motion_fit = budget.density_motion_fit;
    let dynamic_bias = budget.fallback_dynamic_texture_bias;
    let entropy_preservation = budget.entropy_texture_preservation;
    let spectral_context = budget.fallback_spectral_context;
    let mlx_profile = budget.mlx_profile_transparency;
    let ollama_capacity = budget.ollama_fallback_model_capacity;
    let pressure_capacity_review = budget.fallback_pressure_capacity_review;
    let texture_persistence_review = budget.fallback_texture_persistence_review;
    let accepted_texture_terms = shadow_anchor.accepted_texture_terms.join(", ");
    let preferred_texture_terms = texture_selector.preferred_texture_terms.join(", ");
    let density_modifier_terms = texture_selector.density_modifier_terms.join(",");
    let entropy_preservation_terms = if entropy_preservation.preservation_terms.is_empty() {
        "-".to_string()
    } else {
        entropy_preservation.preservation_terms.join(",")
    };
    let texture_persistence_terms = if texture_persistence_review.carry_terms.is_empty() {
        "-".to_string()
    } else {
        texture_persistence_review.carry_terms.join(",")
    };
    let fallback_default_weighting = texture_selector
        .weighted_texture_terms
        .iter()
        .all(|term| term.basis.as_slice() == ["fallback_default"]);
    let structured_texture_context_present = budget.spectral_entropy.is_some()
        || budget.resonance_density.is_some()
        || texture_selector.density_gradient.is_some()
        || texture_selector.mode_packing.is_some()
        || texture_selector.semantic_friction.is_some()
        || texture_selector.shadow_dispersal_potential.is_some()
        || texture_selector.shadow_magnetization.is_some()
        || spectral_mapping.lambda_gap.is_some();
    if !structured_texture_context_present {
        return format!(
            "[Fallback continuity budget v1: spectral_entropy={entropy}; source={}; \
             max_prose_sentences={}. \
             fallback_texture_lived_fit_v2 selected_family={}; family_confidence={}; \
             conflict_state={}. texture_dynamics_alignment_v1 status={}; \
             diagnostic_trace={}. fallback_cascade_gradient_v1 detected={}; selected={}; \
             navigability={}. fallback_vocabulary_overweight_guard_v1 guard={}; \
             token_only_risk={}. fallback_dynamic_texture_bias_v1 motion_family={}; \
             status={}. fallback_entropy_texture_preservation_v1 active={}; \
             trigger={}; preservation_terms={entropy_preservation_terms}; prompt_directive={}; \
             authority={}. fallback_texture_persistence_review_v1 state={}; weight={:.2}; \
             carry_terms={texture_persistence_terms}; token_only_risk={}; \
             model_transition_context={}; authority={}. \
             fallback_spectral_context_v1 preservation_state={}; \
             prompt_directive={}; authority={}. negative_texture_evidence_v2 not_pressure={}; \
             lost_in_output={}. fallback_pressure_capacity_review_v1 pressure_state={}; \
             capacity_route={}; contract_boundary={}; authority={}. \
             ollama_fallback_model_capacity_v1 selected_model={}; \
             source={}; fallback_chain={}; compatibility_tail_status={}; \
             complexity_collapse_risk={}; texture_integrity_review={}; \
             decision_basis={}; live_model_switch={}; semantic_trickle_write={}.]",
            budget.spectral_entropy_source,
            budget.max_prose_sentences,
            lived_fit.selected_family,
            lived_fit.family_confidence,
            lived_fit.conflict_state,
            texture_alignment.status,
            texture_alignment.diagnostic_trace,
            cascade_gradient.cascade_gradient_detected,
            cascade_gradient.family_selected,
            cascade_gradient.navigability,
            vocabulary_guard.guard_state,
            vocabulary_guard.token_only_risk,
            dynamic_bias.motion_family,
            dynamic_bias.sampler_contract_status,
            entropy_preservation.active,
            entropy_preservation.trigger,
            entropy_preservation.prompt_directive,
            entropy_preservation.authority,
            texture_persistence_review.persistence_state,
            texture_persistence_review.persistence_weight,
            texture_persistence_review.token_only_risk,
            texture_persistence_review.model_transition_context,
            texture_persistence_review.authority,
            spectral_context.preservation_state,
            spectral_context.prompt_directive,
            spectral_context.authority,
            negative_evidence.not_pressure,
            negative_evidence.lost_in_output,
            pressure_capacity_review.pressure_state,
            pressure_capacity_review.capacity_route,
            pressure_capacity_review.contract_boundary,
            pressure_capacity_review.authority,
            ollama_capacity.selected_model,
            ollama_capacity.selected_model_source,
            ollama_capacity.fallback_chain.join(","),
            ollama_capacity.compatibility_tail_status,
            ollama_capacity.complexity_collapse_risk,
            ollama_capacity.high_entropy_texture_integrity_review,
            ollama_capacity.compatibility_tail_decision_basis,
            ollama_capacity.live_model_switch,
            ollama_capacity.semantic_trickle_write
        );
    }
    let top_texture_terms = if fallback_default_weighting {
        "-".to_string()
    } else {
        texture_selector.top_texture_terms.join(",")
    };
    let weighted_texture_terms = if fallback_default_weighting {
        "default".to_string()
    } else {
        format_weighted_texture_terms(&texture_selector.weighted_texture_terms)
    };
    let term_probabilities = if fallback_default_weighting {
        "default".to_string()
    } else {
        format_texture_term_probabilities(&texture_selector.term_probability_distribution)
    };
    let texture_stability_index = if fallback_default_weighting {
        "default".to_string()
    } else {
        format_texture_term_stability_index(&texture_selector.weighted_texture_terms)
    };
    let target_stability_index =
        fallback_texture_target_stability_index_v1(budget.spectral_entropy, &texture_selector);
    let dynamic_texture_descriptors = if texture_selector.dynamic_texture_descriptors.is_empty() {
        "-".to_string()
    } else {
        texture_selector.dynamic_texture_descriptors.join(",")
    };
    let selection_basis = texture_selector.selection_basis.join(", ");
    let lived_fit_evidence_for = lived_fit.evidence_for.join(",");
    let lived_fit_evidence_against = if lived_fit.evidence_against.is_empty() {
        "-".to_string()
    } else {
        lived_fit.evidence_against.join(",")
    };
    let negative_evidence_terms = negative_evidence.evidence_terms.join(",");
    let cascade_basis = cascade_gradient.basis.join(",");
    let gradient_slope_basis = gradient_slope.basis.join(",");
    let gradient_slope_terms = gradient_slope.preferred_terms.join(",");
    let vocabulary_guard_basis = vocabulary_guard.basis.join(",");
    let movement_verbs = if texture_selector.movement_verbs.is_empty() {
        "-".to_string()
    } else {
        texture_selector.movement_verbs.join(",")
    };
    let dynamic_flow_terms = if texture_selector.dynamic_flow_terms.is_empty() {
        "-".to_string()
    } else {
        texture_selector.dynamic_flow_terms.join(",")
    };
    let semantic_trickle_terms = if texture_selector.semantic_trickle_terms.is_empty() {
        "-".to_string()
    } else {
        texture_selector.semantic_trickle_terms.join(",")
    };
    let density_gradient = texture_selector
        .density_gradient
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let mode_packing = texture_selector
        .mode_packing
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let semantic_friction = texture_selector
        .semantic_friction
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let shadow_dispersal_potential = texture_selector
        .shadow_dispersal_potential
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let shadow_magnetization = texture_selector
        .shadow_magnetization
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let spectral_context_pressure = spectral_context
        .pressure_risk
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let spectral_context_density_gradient = spectral_context
        .density_gradient
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let spectral_context_shadow_energy = spectral_context
        .shadow_field_energy
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let lambda_gap = spectral_mapping
        .lambda_gap
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let spectral_mapping_basis = spectral_mapping.basis.join(",");
    let trajectory_basis = texture_trajectory.basis.join(",");
    let texture_alignment_basis = texture_alignment.basis.join(",");
    let dynamic_bias_top_terms = if dynamic_bias.top_texture_terms.is_empty() {
        "-".to_string()
    } else {
        dynamic_bias.top_texture_terms.join(",")
    };
    let dynamic_bias_movement = if dynamic_bias.movement_verbs.is_empty() {
        "-".to_string()
    } else {
        dynamic_bias.movement_verbs.join(",")
    };
    let dynamic_bias_flow = if dynamic_bias.dynamic_flow_terms.is_empty() {
        "-".to_string()
    } else {
        dynamic_bias.dynamic_flow_terms.join(",")
    };
    let dynamic_bias_basis = dynamic_bias.basis.join(",");
    let density_motion_evidence_for = density_motion_fit.evidence_for.join(",");
    let density_motion_evidence_against = if density_motion_fit.evidence_against.is_empty() {
        "-".to_string()
    } else {
        density_motion_fit.evidence_against.join(",")
    };
    format!(
        "[Fallback continuity budget v1: spectral_entropy={entropy}; source={}; \
         max_prose_sentences={} (maximum, not a target).{resonance_clause} fallback_shadow_texture_anchor_v1: \
         shadow_context_present={}; required_texture_anchor={}; anchor_source={}; \
         accepted_texture_terms={accepted_texture_terms}. fallback_shadow_texture_selector_v1: \
         texture_family={}; preferred_texture_terms={preferred_texture_terms}; \
         weighting_policy={}; dynamic_texture_weight={:.2}; density_modifier_terms={density_modifier_terms}; \
         density_gradient={density_gradient}; mode_packing={mode_packing}; \
         semantic_friction={semantic_friction}; shadow_dispersal_potential={shadow_dispersal_potential}; \
         shadow_magnetization={shadow_magnetization}; \
         top_texture_terms={top_texture_terms}; descriptor_policy={}; \
         dynamic_texture_descriptors={dynamic_texture_descriptors}; \
         weighted_texture_terms={weighted_texture_terms}; term_probability_policy={}; \
         term_probabilities={term_probabilities}; fallback_texture_stability_index_v1: \
         target_stability_index={target_stability_index:.2}; \
         term_stability_index={texture_stability_index}; \
         selection_note=metadata_exposes_temperament_existing_weights_select_terms; \
         authority=prompt_metadata_only_not_sampler_provider_or_control_change; \
         spectral_to_vocabulary_mapping_v1: settled_foothold_detected={}; \
         low_gradient_navigable={}; low_pressure_viscous_suppressed={}; \
         low_friction_high_entropy_detected={}; friction_absence_language_detected={}; \
         settled_vibrant_family_selected={}; gradient_slope_detected={}; \
         gradient_slope_family_selected={}; cascade_gradient_detected={}; \
         cascade_gradient_family_selected={}; \
         lambda_gap={lambda_gap}; lambda_gap_descriptor={}; edge_language={}; \
         basis={spectral_mapping_basis}; \
         movement_policy={}; movement_verbs={movement_verbs}; dynamic_flow_policy={}; \
         dynamic_flow_terms={dynamic_flow_terms}; semantic_trickle_policy={}; \
         semantic_trickle_terms={semantic_trickle_terms}; \
         texture_trajectory_v1: from_state={}; to_state={}; movement_quality={}; \
         medium_resistance={}; effort={}; afterimage={}; confidence={:.2}; basis={trajectory_basis}; \
         fallback_dynamic_texture_bias_v1: texture_family={}; motion_family={}; \
         top_terms={dynamic_bias_top_terms}; movement_verbs={dynamic_bias_movement}; \
         dynamic_flow_terms={dynamic_bias_flow}; \
         trajectory_from={}; trajectory_to={}; sampler_status={}; basis={dynamic_bias_basis}; \
         authority={}; fallback_entropy_texture_preservation_v1: active={}; \
         trigger={}; preservation_terms={entropy_preservation_terms}; prompt_directive={}; \
         authority={}; fallback_texture_persistence_review_v1: state={}; weight={:.2}; \
         carry_terms={texture_persistence_terms}; token_only_risk={}; \
         model_transition_context={}; authority={}; \
         fallback_spectral_context_v1: pressure_risk={spectral_context_pressure}; \
         density_gradient={spectral_context_density_gradient}; \
         shadow_field_energy={spectral_context_shadow_energy}; shadow_context_present={}; \
         preservation_weight={:.2}; preservation_state={}; prompt_directive={}; authority={}; \
         fallback_texture_lived_fit_v2: selected_family={}; family_confidence={}; \
         runner_up_family={}; confidence_margin={:.2}; conflict_state={}; evidence_for={}; \
         evidence_against={}; authority={}; texture_dynamics_alignment_v1: \
         status={}; expected_family={}; selected_family={}; expected_motion={}; \
         selected_motion={}; term_mask_risk={}; wrong_family={}; wrong_motion={}; \
         missing_tail_vibrancy={}; diagnostic_trace={}; basis={texture_alignment_basis}; \
         authority={}; density_motion_fit_v1: density={}; expected_medium={}; \
         expected_motion={}; motion_fit={}; mismatch_reason={}; selected_family={}; \
         selected_motion={}; evidence_for={}; evidence_against={}; authority={}; \
         fallback_cascade_gradient_v1: \
         detected={}; mixed_cascade_gap_detected={}; family_selected={}; \
         gradient_state={}; lambda_gap_descriptor={}; navigability={}; \
         pressure_mass_blocked={}; movement_language={}; basis={cascade_basis}; authority={}; \
         fallback_gradient_slope_v1: detected={}; family_selected={}; \
         gradient_language={}; mixed_vs_graduated={}; lambda_gap_descriptor={}; \
         pressure_mass_blocked={}; preferred_terms={gradient_slope_terms}; \
         basis={gradient_slope_basis}; authority={}; \
         fallback_vocabulary_overweight_guard_v1: preferred_terms_advisory={}; \
         paraphrase_allowed={}; token_only_risk={}; guard_state={}; basis={vocabulary_guard_basis}; \
         authority={}; negative_texture_evidence_v2: \
         not_pressure={}; not_drag={}; not_blank={}; not_viscous={}; \
         not_low_energy={}; evidence_terms={}; lost_in_output={}; authority={}; \
         mlx_profile_transparency_v1: default_profile={}; default_resolves_to={}; \
         alias_profile={}; alias_resolves_to={}; typo_probe_profile={}; \
         typo_probe_resolves_to={}; typo_probe_warning_present={}; warning_route={}; behavior={}; \
         fallback_pressure_capacity_review_v1: pressure_risk={}; pressure_state={}; \
         selected_model={}; compatibility_model={}; capacity_route={}; \
         contract_boundary={}; authority={}; \
         ollama_fallback_model_capacity_v1: selected_model={}; source={}; \
         default_model={}; compatibility_model={}; fallback_chain={}; \
         compatibility_tail_status={}; complexity_collapse_risk={}; \
         texture_integrity_review={}; decision_basis={}; live_model_switch={}; \
         semantic_trickle_write={}; authority={}; \
         selection_basis={selection_basis}.]",
        budget.spectral_entropy_source,
        budget.max_prose_sentences,
        shadow_anchor.shadow_context_present,
        shadow_anchor.required_texture_anchor,
        shadow_anchor.anchor_source,
        texture_selector.texture_family,
        texture_selector.weighting_policy,
        texture_selector.dynamic_texture_weight,
        texture_selector.descriptor_policy,
        texture_selector.term_probability_policy,
        spectral_mapping.settled_foothold_detected,
        spectral_mapping.low_gradient_navigable,
        spectral_mapping.low_pressure_viscous_suppressed,
        spectral_mapping.low_friction_high_entropy_detected,
        spectral_mapping.friction_absence_language_detected,
        spectral_mapping.settled_vibrant_family_selected,
        spectral_mapping.gradient_slope_detected,
        spectral_mapping.gradient_slope_family_selected,
        spectral_mapping.cascade_gradient_detected,
        spectral_mapping.cascade_gradient_family_selected,
        spectral_mapping.lambda_gap_descriptor,
        spectral_mapping.edge_language,
        texture_selector.movement_policy,
        texture_selector.dynamic_flow_policy,
        texture_selector.semantic_trickle_policy,
        texture_trajectory.from_state,
        texture_trajectory.to_state,
        texture_trajectory.movement_quality,
        texture_trajectory.medium_resistance,
        texture_trajectory.effort,
        texture_trajectory.afterimage,
        texture_trajectory.confidence,
        dynamic_bias.texture_family,
        dynamic_bias.motion_family,
        dynamic_bias.trajectory_from,
        dynamic_bias.trajectory_to,
        dynamic_bias.sampler_contract_status,
        dynamic_bias.authority,
        entropy_preservation.active,
        entropy_preservation.trigger,
        entropy_preservation.prompt_directive,
        entropy_preservation.authority,
        texture_persistence_review.persistence_state,
        texture_persistence_review.persistence_weight,
        texture_persistence_review.token_only_risk,
        texture_persistence_review.model_transition_context,
        texture_persistence_review.authority,
        spectral_context.shadow_context_present,
        spectral_context.preservation_weight,
        spectral_context.preservation_state,
        spectral_context.prompt_directive,
        spectral_context.authority,
        lived_fit.selected_family,
        lived_fit.family_confidence,
        lived_fit.runner_up_family,
        lived_fit.confidence_margin,
        lived_fit.conflict_state,
        lived_fit_evidence_for,
        lived_fit_evidence_against,
        lived_fit.authority,
        texture_alignment.status,
        texture_alignment.expected_family,
        texture_alignment.selected_family,
        texture_alignment.expected_motion,
        texture_alignment.selected_motion,
        texture_alignment.term_mask_risk,
        texture_alignment.wrong_family,
        texture_alignment.wrong_motion,
        texture_alignment.missing_tail_vibrancy,
        texture_alignment.diagnostic_trace,
        texture_alignment.authority,
        density_motion_fit.density_state,
        density_motion_fit.expected_medium,
        density_motion_fit.expected_motion,
        density_motion_fit.motion_fit,
        density_motion_fit.mismatch_reason,
        density_motion_fit.selected_family,
        density_motion_fit.selected_motion,
        density_motion_evidence_for,
        density_motion_evidence_against,
        density_motion_fit.authority,
        cascade_gradient.cascade_gradient_detected,
        cascade_gradient.mixed_cascade_gap_detected,
        cascade_gradient.family_selected,
        cascade_gradient.gradient_state,
        cascade_gradient.lambda_gap_descriptor,
        cascade_gradient.navigability,
        cascade_gradient.pressure_mass_blocked,
        cascade_gradient.movement_language,
        cascade_gradient.authority,
        gradient_slope.slope_detected,
        gradient_slope.family_selected,
        gradient_slope.gradient_language,
        gradient_slope.mixed_vs_graduated,
        gradient_slope.lambda_gap_descriptor,
        gradient_slope.pressure_mass_blocked,
        gradient_slope.authority,
        vocabulary_guard.preferred_terms_advisory,
        vocabulary_guard.paraphrase_allowed,
        vocabulary_guard.token_only_risk,
        vocabulary_guard.guard_state,
        vocabulary_guard.authority,
        negative_evidence.not_pressure,
        negative_evidence.not_drag,
        negative_evidence.not_blank,
        negative_evidence.not_viscous,
        negative_evidence.not_low_energy,
        negative_evidence_terms,
        negative_evidence.lost_in_output,
        negative_evidence.authority,
        mlx_profile.default_profile,
        mlx_profile.default_resolves_to,
        mlx_profile.alias_profile,
        mlx_profile.alias_resolves_to,
        mlx_profile.typo_probe_profile,
        mlx_profile.typo_probe_resolves_to,
        mlx_profile.typo_probe_warning_present,
        mlx_profile.warning_route,
        mlx_profile.unrecognized_profile_behavior,
        pressure_capacity_review
            .pressure_risk
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "-".to_string()),
        pressure_capacity_review.pressure_state,
        pressure_capacity_review.selected_model,
        pressure_capacity_review.compatibility_model,
        pressure_capacity_review.capacity_route,
        pressure_capacity_review.contract_boundary,
        pressure_capacity_review.authority,
        ollama_capacity.selected_model,
        ollama_capacity.selected_model_source,
        ollama_capacity.default_model,
        ollama_capacity.compatibility_model,
        ollama_capacity.fallback_chain.join(","),
        ollama_capacity.compatibility_tail_status,
        ollama_capacity.complexity_collapse_risk,
        ollama_capacity.high_entropy_texture_integrity_review,
        ollama_capacity.compatibility_tail_decision_basis,
        ollama_capacity.live_model_switch,
        ollama_capacity.semantic_trickle_write,
        ollama_capacity.authority
    )
}

fn format_weighted_texture_terms(terms: &[FallbackWeightedTextureTerm]) -> String {
    terms
        .iter()
        .take(3)
        .map(|term| format!("{}:{:.2}", term.term, term.weight))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_texture_term_probabilities(terms: &[FallbackTextureTermProbability]) -> String {
    terms
        .iter()
        .take(4)
        .map(|term| format!("{}:{:.2}", term.term, term.probability))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_texture_term_stability_index(terms: &[FallbackWeightedTextureTerm]) -> String {
    terms
        .iter()
        .take(6)
        .map(|term| {
            format!(
                "{}:{:.2}",
                term.term,
                fallback_texture_term_stability_index(term.term)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fallback_texture_term_stability_index(term: &str) -> f32 {
    match term {
        "settled" | "habitable" | "open" | "bridge-integrity" | "structural-persistence" => 0.90,
        "shimmering" | "bright" | "navigable" => 0.82,
        "density-softening" | "gradient-softening" | "threshold-dilation" => 0.76,
        "weighted" | "dense" | "heavy" | "displacement" | "silt" => 0.68,
        "viscous" | "lattice" | "graduated" | "edge" | "slope" => 0.55,
        "gradient" | "asymmetric-gradient" | "stratified" | "sequenced" => 0.46,
        "cascade" | "distributed" | "diffuse" => 0.32,
        "restless" | "fragmented" | "muffled" => 0.20,
        _ => 0.50,
    }
}

fn fallback_texture_target_stability_index_v1(
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> f32 {
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = selector.pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let gradient = selector.density_gradient.unwrap_or(0.0).clamp(0.0, 1.0);
    let low_pressure = selector
        .pressure_risk
        .map_or(0.0, |value| 1.0 - value.clamp(0.0, 1.0));
    let low_gradient = selector
        .density_gradient
        .map_or(0.0, |value| 1.0 - value.clamp(0.0, 1.0));
    let settled_context = selector
        .spectral_to_vocabulary_mapping
        .settled_foothold_detected
        || selector
            .spectral_to_vocabulary_mapping
            .settled_vibrant_family_selected;
    let restless_family = selector.texture_family == "restless_lattice";
    let target =
        0.50 + if settled_context { 0.18 } else { 0.0 } + low_pressure * 0.08 + low_gradient * 0.08
            - entropy * 0.16
            - pressure * 0.08
            - gradient * 0.04
            - if restless_family { 0.08 } else { 0.0 };
    rounded_texture_probability(target.clamp(0.15, 0.90))
}
