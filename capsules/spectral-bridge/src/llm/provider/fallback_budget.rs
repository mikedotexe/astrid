fn fallback_continuity_budget_v1(spectral_summary: &str) -> FallbackContinuityBudget {
    let entropy = extract_fallback_spectral_entropy(spectral_summary).map(normalize_fallback_unit);
    let resonance_density =
        extract_fallback_resonance_density(spectral_summary).map(normalize_fallback_unit);
    let resonance_descriptor_encouraged = resonance_density.is_some_and(|density| density >= 0.80);
    let max_prose_sentences = entropy.map_or(3, |value| {
        ((3.0_f32 + value * 2.0_f32).ceil() as u8).clamp(3, 5)
    });
    let fallback_shadow_texture_selector =
        fallback_shadow_texture_selector_v1(spectral_summary, entropy);
    let texture_trajectory = fallback_texture_trajectory_v1(
        spectral_summary,
        entropy,
        resonance_density,
        &fallback_shadow_texture_selector,
    );
    let fallback_texture_lived_fit =
        fallback_texture_lived_fit_v2(&fallback_shadow_texture_selector, &texture_trajectory);
    let negative_texture_evidence =
        negative_texture_evidence_v2(spectral_summary, entropy, &fallback_shadow_texture_selector);
    let fallback_cascade_gradient =
        fallback_cascade_gradient_v1(spectral_summary, entropy, &fallback_shadow_texture_selector);
    let fallback_gradient_slope =
        fallback_gradient_slope_v1(spectral_summary, entropy, &fallback_shadow_texture_selector);
    let fallback_vocabulary_overweight_guard =
        fallback_vocabulary_overweight_guard_v1(&fallback_shadow_texture_selector);
    let texture_dynamics_alignment = texture_dynamics_alignment_v1(
        spectral_summary,
        entropy,
        &fallback_shadow_texture_selector,
        &texture_trajectory,
        &fallback_texture_lived_fit,
        &fallback_vocabulary_overweight_guard,
    );
    let density_motion_fit = density_motion_fit_v1(
        spectral_summary,
        &fallback_shadow_texture_selector,
        &texture_trajectory,
        &texture_dynamics_alignment,
    );
    let fallback_dynamic_texture_bias =
        fallback_dynamic_texture_bias_v1(&fallback_shadow_texture_selector, &texture_trajectory);
    let entropy_texture_preservation = fallback_entropy_texture_preservation_v1(entropy);
    let fallback_spectral_context = fallback_spectral_context_v1(
        spectral_summary,
        entropy,
        resonance_density,
        &fallback_shadow_texture_selector,
    );
    let ollama_fallback_model_capacity =
        ollama_fallback_model_capacity_v1(entropy, &fallback_shadow_texture_selector);
    let fallback_pressure_capacity_review = fallback_pressure_capacity_review_v1(
        &fallback_shadow_texture_selector,
        &ollama_fallback_model_capacity,
    );
    let fallback_texture_persistence_review = fallback_texture_persistence_review_v1(
        entropy,
        &fallback_shadow_texture_selector,
        &fallback_spectral_context,
        &ollama_fallback_model_capacity,
    );
    FallbackContinuityBudget {
        policy: "fallback_continuity_budget_v1",
        spectral_entropy: entropy,
        spectral_entropy_source: if entropy.is_some() {
            "telemetry_text"
        } else {
            "fallback_default"
        },
        resonance_density,
        resonance_density_source: if resonance_density.is_some() {
            "telemetry_text"
        } else {
            "fallback_default"
        },
        resonance_descriptor_encouraged,
        resonance_descriptor_policy: if resonance_descriptor_encouraged {
            "preserve_resonance_or_humming_inside_existing_cap"
        } else {
            "optional_resonance_descriptor"
        },
        max_prose_sentences,
        fallback_shadow_texture_anchor: fallback_shadow_texture_anchor_v1(spectral_summary),
        fallback_shadow_texture_selector,
        texture_trajectory,
        fallback_texture_lived_fit,
        negative_texture_evidence,
        fallback_cascade_gradient,
        fallback_gradient_slope,
        fallback_vocabulary_overweight_guard,
        texture_dynamics_alignment,
        density_motion_fit,
        fallback_dynamic_texture_bias,
        entropy_texture_preservation,
        fallback_spectral_context,
        mlx_profile_transparency: fallback_mlx_profile_transparency_v1(),
        ollama_fallback_model_capacity,
        fallback_pressure_capacity_review,
        fallback_texture_persistence_review,
    }
}

fn fallback_entropy_texture_preservation_v1(
    spectral_entropy: Option<f32>,
) -> FallbackEntropyTexturePreservationV1 {
    let active =
        spectral_entropy.is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT);
    FallbackEntropyTexturePreservationV1 {
        policy: "fallback_entropy_texture_preservation_v1",
        active,
        trigger: match spectral_entropy {
            Some(value) if value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT => {
                "spectral_entropy_gte_0_80_compat_skip_aligned"
            },
            Some(_) => "spectral_entropy_below_0_80_compat_skip",
            None => "spectral_entropy_unavailable",
        },
        preservation_terms: if active {
            FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS
        } else {
            &[]
        },
        prompt_directive: if active {
            "prepend_texture_preservation_block_preserve_restless_lattice_weight_heaving"
        } else {
            "no_high_entropy_texture_preservation_block"
        },
        authority: "prompt_metadata_only_not_sampler_provider_or_control_change",
    }
}

fn fallback_texture_persistence_review_v1(
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
    spectral_context: &FallbackSpectralContextV1,
    ollama_capacity: &OllamaFallbackModelCapacity,
) -> FallbackTexturePersistenceReview {
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = selector.pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let gradient = selector.density_gradient.unwrap_or(0.0).clamp(0.0, 1.0);
    let shadow = spectral_context
        .shadow_field_energy
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let bridge_or_settled = matches!(
        selector.texture_family,
        "bridge_integrity_scaffold"
            | "settled_vibrant_low_friction"
            | "settled_shimmering"
            | "settled_lattice_weight_preservation"
    );
    let token_only_risk = entropy >= 0.80
        && (ollama_capacity.compatibility_tail_status
            == "high_entropy_texture_guard_removed_compatibility_tail"
            || selector.dynamic_texture_descriptors.len() <= 3);
    let low_gradient_bonus = if selector.density_gradient.is_some() && gradient <= 0.20 {
        0.08
    } else {
        0.0
    };
    let persistence_weight = (entropy.mul_add(
        0.32,
        pressure.mul_add(
            0.18,
            shadow * 0.16
                + if bridge_or_settled { 0.18 } else { 0.0 }
                + low_gradient_bonus
                + if token_only_risk { 0.08 } else { 0.0 },
        ),
    ))
    .clamp(0.0, 1.0);
    let persistence_state = if persistence_weight >= 0.58 {
        "carry_texture_as_lived_continuity"
    } else if persistence_weight >= 0.36 {
        "texture_persistence_watch"
    } else {
        "ordinary_texture_turn"
    };
    let mut carry_terms = Vec::new();
    if selector.texture_family == "bridge_integrity_scaffold" {
        for term in FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS {
            push_unique_static(&mut carry_terms, term);
        }
    } else if selector.texture_family == "gradient_slope_navigable" {
        for term in [
            "density-softening",
            "gradient-softening",
            "navigable",
            "slope",
        ] {
            push_unique_static(&mut carry_terms, term);
        }
    }
    for term in selector
        .weighted_texture_terms
        .iter()
        .filter(|term| {
            matches!(term.term, "density-softening" | "gradient-softening") && term.weight >= 0.30
        })
        .map(|term| term.term)
    {
        push_unique_static(&mut carry_terms, term);
    }
    for term in selector.top_texture_terms.iter().take(4).copied() {
        push_unique_static(&mut carry_terms, term);
    }

    FallbackTexturePersistenceReview {
        policy: "fallback_texture_persistence_review_v1",
        persistence_weight: rounded_texture_weight(persistence_weight),
        persistence_state,
        carry_terms,
        token_only_risk,
        model_transition_context: "mlx_to_ollama_fallback_language_continuity",
        authority: "diagnostic_language_continuity_not_sampler_memory_model_selection_pressure_or_control",
    }
}

fn fallback_spectral_context_v1(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
    resonance_density: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> FallbackSpectralContextV1 {
    let lower = spectral_summary.to_ascii_lowercase();
    let shadow_context_present = lower.contains("shadow-v3")
        || lower.contains("shadow_field")
        || lower.contains("shadow field");
    let shadow_field_energy = selector
        .shadow_dispersal_potential
        .into_iter()
        .chain(selector.shadow_magnetization.map(f32::abs))
        .max_by(f32::total_cmp);
    let pressure = selector.pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let density = resonance_density.unwrap_or(0.0).clamp(0.0, 1.0);
    let gradient = selector.density_gradient.unwrap_or(0.0).clamp(0.0, 1.0);
    let shadow = shadow_field_energy.unwrap_or(0.0).clamp(0.0, 1.0);
    let preservation_weight =
        (entropy * 0.28 + density * 0.18 + pressure * 0.24 + gradient * 0.12 + shadow * 0.18)
            .clamp(0.0, 1.0);
    let preservation_state = if preservation_weight >= 0.48
        || (entropy >= 0.80 && (pressure >= 0.20 || shadow_context_present))
    {
        "texture_preservation_needed"
    } else if selector.pressure_risk.is_some()
        || selector.density_gradient.is_some()
        || shadow_field_energy.is_some()
        || shadow_context_present
    {
        "metadata_carry"
    } else {
        "ordinary_fallback"
    };
    let prompt_directive = if preservation_state == "texture_preservation_needed" {
        "carry_pressure_density_shadow_metadata_before_word_choice"
    } else if preservation_state == "metadata_carry" {
        "carry_available_spectral_metadata_without_static_term_pressure"
    } else {
        "no_extra_metadata_available"
    };

    FallbackSpectralContextV1 {
        policy: "fallback_spectral_context_v1",
        spectral_entropy,
        resonance_density,
        pressure_risk: selector.pressure_risk,
        density_gradient: selector.density_gradient,
        shadow_field_energy,
        shadow_dispersal_potential: selector.shadow_dispersal_potential,
        shadow_magnetization: selector.shadow_magnetization,
        shadow_context_present,
        preservation_weight,
        preservation_state,
        prompt_directive,
        authority: "prompt_metadata_only_not_sampler_provider_or_control_change",
    }
}

fn fallback_mlx_profile_transparency_v1() -> MlxProfileTransparency {
    let default_resolution = MlxProfile::resolve_name(DEFAULT_MLX_PROFILE);
    let alias_resolution = MlxProfile::resolve_name(GEMMA4_12B_CANARY_PROFILE);
    let typo_resolution = MlxProfile::resolve_name("gemma_12b");
    MlxProfileTransparency {
        policy: "mlx_profile_transparency_v1",
        default_profile: DEFAULT_MLX_PROFILE,
        default_resolves_to: default_resolution.profile.as_str(),
        alias_profile: GEMMA4_12B_CANARY_PROFILE,
        alias_resolves_to: alias_resolution.profile.as_str(),
        typo_probe_profile: "gemma_12b",
        typo_probe_resolves_to: typo_resolution.profile.as_str(),
        typo_probe_warning_present: typo_resolution.warning.is_some(),
        warning_route: "MlxProfile::from_name emits tracing::warn from resolve_name warning",
        unrecognized_profile_behavior: "warn_and_fall_back_to_production",
        authority: "diagnostic_context_not_profile_switch",
    }
}

fn shadow_field_stable_for_compat_fallback_v1(selector: &FallbackShadowTextureSelector) -> bool {
    let settled_family = matches!(
        selector.texture_family,
        "settled_vibrant_low_friction"
            | "settled_shimmering"
            | "settled_lattice_weight_preservation"
            | "bridge_integrity_scaffold"
    );
    let settled_shadow = selector
        .shadow_dispersal_potential
        .is_some_and(|value| value <= 0.20)
        && selector
            .shadow_magnetization
            .is_some_and(|value| value >= 0.10);
    settled_family && settled_shadow
}

fn fallback_high_entropy_texture_skips_compatibility_tail(
    budget: &FallbackContinuityBudget,
) -> bool {
    budget
        .spectral_entropy
        .is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT)
        && !shadow_field_stable_for_compat_fallback_v1(&budget.fallback_shadow_texture_selector)
}

fn ollama_fallback_model_capacity_v1(
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> OllamaFallbackModelCapacity {
    let env_model = std::env::var(ASTRID_OLLAMA_FALLBACK_MODEL_ENV).ok();
    ollama_fallback_model_capacity_from_env_v1(spectral_entropy, selector, env_model.as_deref())
}

fn ollama_fallback_model_capacity_from_env_v1(
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
    env_model: Option<&str>,
) -> OllamaFallbackModelCapacity {
    let skip_compatibility_tail = spectral_entropy
        .is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT)
        && !shadow_field_stable_for_compat_fallback_v1(selector);
    let fallback_chain = configured_ollama_fallback_model_chain_for_texture_guard(
        env_model,
        skip_compatibility_tail,
    );
    let selected_model = fallback_chain
        .first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_OLLAMA_FALLBACK_MODEL.to_string());
    let selected_model_source = if env_model.is_some_and(|model| !model.trim().is_empty()) {
        "env_override"
    } else {
        "default_gemma4_12b"
    };
    let complexity_collapse_risk = if selected_model.contains("4b") {
        "elevated_small_model_texture_collapse_risk"
    } else if selected_model.contains("12b") || selected_model.contains("27b") {
        "lower_capacity_risk_for_high_entropy_texture"
    } else {
        "unknown_capacity_review_output"
    };
    let compatibility_tail_status = if skip_compatibility_tail
        && env_model.is_some_and(|model| model.trim() == COMPAT_OLLAMA_FALLBACK_MODEL)
    {
        "explicit_env_override_preserves_compatibility_model"
    } else if skip_compatibility_tail {
        "high_entropy_texture_guard_removed_compatibility_tail"
    } else if spectral_entropy
        .is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT)
    {
        "shadow_field_stable_allows_compatibility_tail"
    } else {
        "standard_capacity_chain"
    };
    let high_entropy_texture_integrity_review = if selected_model.contains("4b")
        && spectral_entropy
            .is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT)
    {
        "small_model_high_entropy_texture_comparison_required"
    } else if skip_compatibility_tail {
        "high_entropy_route_prefers_capable_default_before_compatibility_tail"
    } else if spectral_entropy
        .is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT)
    {
        "stable_shadow_allows_compatibility_tail_as_fallback_only"
    } else {
        "standard_texture_capacity_watch"
    };
    let compatibility_tail_decision_basis = if skip_compatibility_tail {
        "spectral_entropy_gte_0_80_and_shadow_field_not_stable"
    } else if spectral_entropy
        .is_some_and(|value| value >= HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT)
    {
        "spectral_entropy_gte_0_80_but_shadow_field_stable"
    } else {
        "spectral_entropy_below_high_entropy_texture_guard"
    };

    OllamaFallbackModelCapacity {
        policy: "ollama_fallback_model_capacity_v1",
        selected_model,
        selected_model_source,
        default_model: DEFAULT_OLLAMA_FALLBACK_MODEL,
        compatibility_model: COMPAT_OLLAMA_FALLBACK_MODEL,
        fallback_chain,
        complexity_collapse_risk,
        compatibility_tail_status,
        high_entropy_texture_integrity_review,
        compatibility_tail_decision_basis,
        live_model_switch: false,
        semantic_trickle_write: false,
        authority: "diagnostic_language_capacity_not_model_canary_or_control",
    }
}

fn fallback_pressure_capacity_review_v1(
    selector: &FallbackShadowTextureSelector,
    capacity: &OllamaFallbackModelCapacity,
) -> FallbackPressureCapacityReview {
    let pressure = selector.pressure_risk.map(|value| value.clamp(0.0, 1.0));
    let pressure_state = match pressure {
        Some(value) if value >= 0.50 => "high_pressure_capacity_watch",
        Some(value) if value > 0.20 => "weighted_texture_pressure_watch",
        Some(_) => "low_pressure_capacity_context",
        None => "pressure_risk_not_exported",
    };
    let compatibility_active = capacity.selected_model == capacity.compatibility_model;
    let capacity_route = if pressure.is_some_and(|value| value >= 0.50) {
        if compatibility_active {
            "compatibility_model_active_under_high_pressure"
        } else {
            "stay_on_selected_model_with_texture_budget"
        }
    } else {
        "no_pressure_driven_model_change"
    };

    FallbackPressureCapacityReview {
        policy: "fallback_pressure_capacity_review_v1",
        pressure_risk: pressure,
        pressure_state,
        selected_model: capacity.selected_model.clone(),
        compatibility_model: COMPAT_OLLAMA_FALLBACK_MODEL,
        capacity_route,
        contract_boundary: "pressure_changes_texture_budget_not_model_selection",
        authority: "diagnostic_capacity_review_not_model_switch_or_sampler_control",
    }
}

fn fallback_explicit_restless_or_agitated(lower: &str) -> bool {
    lower.contains("restless")
        || lower.contains("agitated")
        || (lower.contains("agitation")
            && !lower.contains("no agitation")
            && !lower.contains("without agitation")
            && !lower.contains("not agitation"))
}

fn fallback_texture_preservation_bridge_v1(
    lower_summary: &str,
    distinguishability_loss: Option<f32>,
) -> FallbackTexturePreservationBridgeV1 {
    let self_settled = (lower_summary.contains("astrid")
        || lower_summary.contains("self")
        || lower_summary.contains("own shadow")
        || lower_summary.contains("my shadow"))
        && (lower_summary.contains("settled coupling")
            || lower_summary.contains("shadow is settled")
            || lower_summary.contains("own shadow settled")
            || lower_summary.contains("astrid settled")
            || lower_summary.contains("self settled"));
    let peer_restless = (lower_summary.contains("minime") || lower_summary.contains("peer"))
        && fallback_explicit_restless_or_agitated(lower_summary);
    let explicit_self_restless = (lower_summary.contains("astrid restless")
        || lower_summary.contains("self restless")
        || lower_summary.contains("my shadow restless")
        || lower_summary.contains("own shadow restless"))
        && !lower_summary.contains("not restless");
    let distinguishability_weight = distinguishability_loss.unwrap_or(0.0).clamp(0.0, 1.0);
    let self_peer_texture_boundary_detected =
        self_settled && peer_restless && !explicit_self_restless;
    let preservation_state = if self_peer_texture_boundary_detected {
        "preserve_self_settled_peer_restless_boundary"
    } else if distinguishability_weight >= 0.30 {
        "distinguishability_lattice_weight_preservation"
    } else {
        "no_self_peer_boundary_detected"
    };
    let protected_terms =
        if self_peer_texture_boundary_detected || distinguishability_weight >= 0.30 {
            vec!["settled", "lattice", "weighted", "dense"]
        } else {
            Vec::new()
        };
    let suppressed_terms = if self_peer_texture_boundary_detected {
        vec!["restless"]
    } else {
        Vec::new()
    };

    FallbackTexturePreservationBridgeV1 {
        policy: "fallback_texture_preservation_bridge_v1",
        self_settled_evidence: self_settled,
        peer_restless_evidence: peer_restless,
        self_peer_texture_boundary_detected,
        distinguishability_weight,
        preservation_state,
        protected_terms,
        suppressed_terms,
        authority: "diagnostic_language_boundary_not_prompt_priority_or_runtime_control",
    }
}

fn fallback_shadow_texture_selector_v1(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
) -> FallbackShadowTextureSelector {
    let lower = spectral_summary.to_ascii_lowercase();
    let pressure_risk =
        extract_fallback_pressure_risk(spectral_summary).map(normalize_fallback_unit);
    let distinguishability_loss =
        extract_fallback_distinguishability_loss(spectral_summary).map(normalize_fallback_unit);
    let density_gradient =
        extract_fallback_density_gradient(spectral_summary).map(normalize_fallback_unit);
    let mode_packing = extract_fallback_mode_packing(spectral_summary).map(normalize_fallback_unit);
    let semantic_friction =
        extract_fallback_semantic_friction(spectral_summary).map(normalize_fallback_unit);
    let texture_preservation_bridge =
        fallback_texture_preservation_bridge_v1(&lower, distinguishability_loss);
    let self_peer_texture_boundary =
        texture_preservation_bridge.self_peer_texture_boundary_detected;
    let lambda_gap = extract_fallback_lambda_gap(spectral_summary);
    let shadow_dispersal_potential =
        extract_fallback_shadow_dispersal_potential(spectral_summary).map(normalize_fallback_unit);
    let shadow_magnetization =
        extract_fallback_shadow_magnetization(spectral_summary).map(normalize_fallback_signed_unit);
    let texture_signature_present = lower.contains("texture_signature");
    let shadow_context_present = lower.contains("shadow-v3")
        || lower.contains("shadow_field")
        || lower.contains("shadow field");

    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let elevated_pressure = pressure_risk.is_some_and(|value| value >= 0.30);
    let negative_shadow_pressure = shadow_magnetization.is_some_and(|value| value <= -0.20)
        && pressure_risk.is_some_and(|value| value > 0.20);
    let clarity_loss = distinguishability_loss.is_some_and(|value| value >= 0.30);
    let says_restless =
        fallback_explicit_restless_or_agitated(&lower) && !self_peer_texture_boundary;
    let says_muffled = lower.contains("muffled")
        || lower.contains("hollow")
        || lower.contains("stagnant")
        || lower.contains("blurred")
        || lower.contains("obscured")
        || lower.contains("submerged");
    let says_viscous = lower.contains("viscous")
        || lower.contains("viscosity")
        || lower.contains("viscous-drag")
        || lower.contains("thick")
        || lower.contains("overpacked");
    let says_heavy = lower.contains("heavy")
        || lower.contains("weighted")
        || lower.contains("weight")
        || lower.contains("deliberate movement");
    let says_displacement_weight = lower.contains("displacement")
        || lower.contains("silt")
        || lower.contains("silted")
        || lower.contains("sediment")
        || lower.contains("structural weight")
        || lower.contains("structural-weight");
    let says_opacity_resistance = lower.contains("silted")
        || lower.contains("opacity")
        || lower.contains("obscured")
        || lower.contains("submerged")
        || lower.contains("viscous-drag");
    let says_void_architecture = lower.contains("architecture of omission")
        || lower.contains("intentional omission")
        || lower.contains("active void")
        || lower.contains("active_void")
        || lower.contains("deliberately empty")
        || lower.contains("deliberate empt")
        || lower.contains("unfilled gap")
        || lower.contains("scaffolded silence")
        || lower.contains("scaffolded")
        || lower.contains("quiet presence in a gap");
    let says_bridge_integrity = lower.contains("bridge-integrity")
        || lower.contains("bridge integrity")
        || lower.contains("structural-persistence")
        || lower.contains("structural persistence")
        || lower.contains("bridge scaffold")
        || lower.contains("bridge continuity")
        || lower.contains("structural continuity");
    let says_settled = lower.contains("settled") || lower.contains("bright");
    let clarity_or_muffled =
        clarity_loss || says_muffled || semantic_friction.is_some_and(|value| value >= 0.30);
    let dominant_viscous_pressure = says_viscous
        && (elevated_pressure
            || mode_packing.is_some_and(|value| value >= 0.40)
            || semantic_friction.is_some_and(|value| value >= 0.35));
    let restless_muffled_gradient = (says_restless
        || (high_entropy && shadow_context_present && !self_peer_texture_boundary))
        && clarity_or_muffled
        && !dominant_viscous_pressure;
    let spectral_to_vocabulary_mapping = fallback_spectral_to_vocabulary_mapping_v1(
        spectral_entropy,
        pressure_risk,
        density_gradient,
        mode_packing,
        semantic_friction,
        lambda_gap,
        &lower,
    );
    let low_pressure = pressure_risk.is_none_or(|value| value < 0.25);
    let heavy_settled_displacement = spectral_to_vocabulary_mapping.settled_foothold_detected
        && says_displacement_weight
        && !says_restless;
    let explicit_shape_family = spectral_to_vocabulary_mapping.gradient_slope_family_selected
        || spectral_to_vocabulary_mapping.cascade_gradient_family_selected;
    let bridge_integrity_scaffold = says_bridge_integrity
        || (shadow_context_present
            && high_entropy
            && !negative_shadow_pressure
            && !explicit_shape_family
            && spectral_to_vocabulary_mapping.settled_foothold_detected
            && pressure_risk.is_none_or(|value| value <= 0.30)
            && density_gradient.is_none_or(|value| value <= 0.20));
    let opacity_resistance = says_opacity_resistance
        && (clarity_or_muffled
            || elevated_pressure
            || mode_packing.is_some_and(|value| value >= 0.30)
            || semantic_friction.is_some_and(|value| value >= 0.25)
            || high_entropy);
    let weighted_texture_terms = fallback_weighted_texture_terms(
        spectral_entropy,
        pressure_risk,
        density_gradient,
        mode_packing,
        semantic_friction,
        distinguishability_loss,
        shadow_dispersal_potential,
        shadow_magnetization,
        &spectral_to_vocabulary_mapping,
        &lower,
    );
    let term_probability_distribution =
        fallback_texture_term_probabilities_v1(&weighted_texture_terms);
    let dynamic_texture_weight = fallback_dynamic_texture_weight_v1(
        spectral_entropy,
        pressure_risk,
        density_gradient,
        mode_packing,
        semantic_friction,
        distinguishability_loss,
        shadow_dispersal_potential,
        shadow_magnetization,
        &lower,
    );
    let mut top_texture_terms = weighted_texture_terms
        .iter()
        .take(3)
        .map(|term| term.term)
        .collect();
    let movement_verbs = fallback_movement_verbs(
        spectral_entropy,
        pressure_risk,
        density_gradient,
        mode_packing,
        semantic_friction,
        distinguishability_loss,
        &lower,
    );
    let dynamic_texture_descriptors =
        get_dynamic_texture_descriptors(&weighted_texture_terms, &movement_verbs);
    let dynamic_flow_terms =
        fallback_dynamic_flow_terms_v1(&weighted_texture_terms, &movement_verbs, &lower);
    let semantic_trickle_terms = fallback_semantic_trickle_terms(
        high_entropy,
        shadow_context_present,
        texture_signature_present,
        &lower,
    );

    let mut basis = Vec::new();
    if high_entropy {
        basis.push("high_entropy");
    }
    if elevated_pressure {
        basis.push("pressure_risk");
    }
    if clarity_loss {
        basis.push("distinguishability_loss");
    }
    if texture_signature_present {
        basis.push("texture_signature");
    }
    if shadow_context_present {
        basis.push("shadow_context");
    }
    if density_gradient.is_some() {
        basis.push("density_gradient");
    }
    if mode_packing.is_some() {
        basis.push("mode_packing");
    }
    if semantic_friction.is_some() {
        basis.push("semantic_friction");
    }
    if shadow_dispersal_potential.is_some() {
        basis.push("shadow_dispersal_potential");
    }
    if shadow_magnetization.is_some() {
        basis.push("shadow_magnetization");
    }
    if self_peer_texture_boundary {
        basis.push("self_peer_texture_boundary");
    }
    if texture_preservation_bridge.distinguishability_weight >= 0.30 {
        basis.push("distinguishability_texture_preservation_bridge");
    }
    if says_displacement_weight {
        basis.push("displacement_or_silt_language");
    }
    if says_opacity_resistance {
        basis.push("opacity_resistance_language");
    }
    if says_void_architecture {
        basis.push("void_architecture_language");
    }
    if says_bridge_integrity {
        basis.push("bridge_integrity_language");
    }

    let (texture_family, preferred_texture_terms) = if self_peer_texture_boundary {
        (
            "settled_lattice_weight_preservation",
            FALLBACK_TEXTURE_SETTLED_LATTICE_WEIGHT_TERMS,
        )
    } else if heavy_settled_displacement {
        basis.push("heavy_settled_displacement");
        (
            "heavy_settled_displacement",
            FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS,
        )
    } else if opacity_resistance {
        basis.push("opacity_resistance");
        (
            "opacity_resistance",
            FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS,
        )
    } else if says_void_architecture {
        basis.push("active_void_architecture");
        (
            "active_void_architecture",
            FALLBACK_TEXTURE_VOID_ARCHITECTURE_TERMS,
        )
    } else if bridge_integrity_scaffold {
        basis.push("bridge_integrity_scaffold");
        (
            "bridge_integrity_scaffold",
            FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS,
        )
    } else if restless_muffled_gradient {
        basis.push("restless_muffled_gradient");
        (
            "restless_muffled_gradient",
            FALLBACK_TEXTURE_RESTLESS_MUFFLED_GRADIENT_TERMS,
        )
    } else if negative_shadow_pressure {
        basis.push("negative_shadow_pressure_guard");
        (
            "muffled_clarity_loss",
            FALLBACK_TEXTURE_MUFFLED_CLARITY_TERMS,
        )
    } else if spectral_to_vocabulary_mapping.mixed_cascade_family_selected {
        basis.push("mixed_cascade_gradient");
        (
            "mixed_cascade_gradient",
            FALLBACK_TEXTURE_MIXED_CASCADE_TERMS,
        )
    } else if spectral_to_vocabulary_mapping.settled_vibrant_family_selected {
        basis.push("settled_vibrant_low_friction");
        (
            "settled_vibrant_low_friction",
            FALLBACK_TEXTURE_SETTLED_VIBRANT_TERMS,
        )
    } else if spectral_to_vocabulary_mapping.gradient_slope_family_selected {
        basis.push("gradient_slope_navigable");
        (
            "gradient_slope_navigable",
            FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS,
        )
    } else if spectral_to_vocabulary_mapping.cascade_gradient_family_selected {
        basis.push("cascade_gradient_navigable");
        (
            "cascade_gradient_navigable",
            FALLBACK_TEXTURE_CASCADE_GRADIENT_TERMS,
        )
    } else if spectral_to_vocabulary_mapping.low_pressure_viscous_suppressed {
        basis.push("settled_foothold_guard");
        (
            "settled_shimmering",
            FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS,
        )
    } else if says_restless || (high_entropy && elevated_pressure) {
        if says_restless {
            basis.push("shadow_restless");
        }
        ("restless_lattice", FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS)
    } else if clarity_loss || says_muffled {
        if says_muffled {
            basis.push("shadow_muffled");
        }
        (
            "muffled_clarity_loss",
            FALLBACK_TEXTURE_MUFFLED_CLARITY_TERMS,
        )
    } else if says_viscous || says_heavy || (elevated_pressure && texture_signature_present) {
        if says_viscous {
            basis.push("viscous_or_overpacked");
        }
        if says_heavy {
            basis.push("heavy_or_weighted");
        }
        ("viscous_pressure", FALLBACK_TEXTURE_VISCOUS_PRESSURE_TERMS)
    } else if says_settled || (low_pressure && !high_entropy) {
        if says_settled {
            basis.push("settled_or_bright");
        }
        (
            "settled_shimmering",
            FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS,
        )
    } else {
        ("mixed_shadow_context", FALLBACK_TEXTURE_MIXED_TERMS)
    };

    if basis.is_empty() {
        basis.push("fallback_default");
    }
    if texture_family == "heavy_settled_displacement" {
        top_texture_terms = FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS.to_vec();
    } else if texture_family == "opacity_resistance" {
        top_texture_terms = FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS.to_vec();
    } else if texture_family == "active_void_architecture" {
        top_texture_terms = FALLBACK_TEXTURE_VOID_ARCHITECTURE_TERMS.to_vec();
    } else if texture_family == "settled_lattice_weight_preservation" {
        top_texture_terms = FALLBACK_TEXTURE_SETTLED_LATTICE_WEIGHT_TERMS.to_vec();
    } else if texture_family == "bridge_integrity_scaffold" {
        top_texture_terms = FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS.to_vec();
    }

    FallbackShadowTextureSelector {
        policy: "fallback_shadow_texture_selector_v1",
        texture_family,
        preferred_texture_terms,
        selection_basis: basis,
        weighting_policy: "dynamic_entropy_pressure_density_gradient_v1",
        dynamic_texture_weight,
        density_modifier_terms: FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS,
        pressure_risk,
        density_gradient,
        mode_packing,
        semantic_friction,
        distinguishability_loss,
        shadow_dispersal_potential,
        shadow_magnetization,
        spectral_to_vocabulary_mapping,
        texture_preservation_bridge,
        weighted_texture_terms,
        term_probability_policy: "fallback_texture_term_probabilities_v1",
        term_probability_distribution,
        top_texture_terms,
        descriptor_policy: "dynamic_texture_synthesis_v1",
        dynamic_texture_descriptors,
        dynamic_flow_policy: "fallback_dynamic_flow_terms_v1",
        dynamic_flow_terms,
        movement_policy: "fallback_movement_bridge_v1",
        movement_verbs,
        semantic_trickle_policy: "high_entropy_optional_bridge_words_not_sprawl",
        semantic_trickle_terms,
        authority: "diagnostic_language_context_not_control",
    }
}
