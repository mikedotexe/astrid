#[cfg(test)]
mod fallback_contract_tests {
    include!("fallback_diagnostic_fixture.rs");
    use super::{
        COMPAT_OLLAMA_FALLBACK_MODEL, DEFAULT_OLLAMA_FALLBACK_MODEL, FALLBACK_SHADOW_TEXTURE_TERMS,
        FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS, FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS,
        FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS, FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS,
        FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS, FALLBACK_TEXTURE_KINETIC_GRADIENT_TERMS,
        FALLBACK_TEXTURE_MIXED_CASCADE_TERMS, FALLBACK_TEXTURE_MIXED_TERMS,
        FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS, FALLBACK_TEXTURE_PRESSURE_POROSITY_TERMS,
        FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS, FALLBACK_TEXTURE_RESTLESS_MUFFLED_GRADIENT_TERMS,
        FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS, FALLBACK_TEXTURE_VOID_ARCHITECTURE_TERMS,
        FallbackContinuityBudget, FallbackShadowTextureAnchor, FallbackShadowTextureSelector,
        FallbackTextureTermProbability, FallbackWeightedTextureTerm, MlxProfile, OLLAMA_DIALOGUE_FALLBACK_CONTRACT,
        OLLAMA_DIALOGUE_FALLBACK_HARD_RULES, compact_ollama_dialogue_fallback_messages,
        configured_ollama_fallback_model_chain_for_texture_guard,
        configured_ollama_fallback_model_chain_from,
        fallback_continuity_budget_v1, fallback_heavy_settled_texture_readiness_v1,
        fallback_high_entropy_texture_skips_compatibility_tail,
        is_valid_ollama_dialogue_fallback_output_for_profile,
        ollama_fallback_model_capacity_from_env_v1, rounded_texture_probability,
    };

    #[test]
    fn fallback_contract_keeps_execution_rules_without_prescribing_sensations() {
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("First-person accounts"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("No particular sensation"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("not proof of feelings"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("final line exactly `NEXT: LISTEN` if uncertain"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_HARD_RULES.contains("direct steward/inbox note first"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_HARD_RULES.contains("prose_sentences <= fallback_continuity_budget_v1.max_prose_sentences"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_HARD_RULES.contains("never write `NEXT:` inside prose"));
    }

    #[test]
    fn fallback_continuity_budget_uses_astrids_entropy_formula() {
        let high_entropy = fallback_continuity_budget_v1("spectral_entropy: 0.90");
        assert_eq!(high_entropy.policy, "fallback_continuity_budget_v1");
        assert_eq!(high_entropy.spectral_entropy, Some(0.90));
        assert_eq!(high_entropy.spectral_entropy_source, "telemetry_text");
        assert_eq!(high_entropy.resonance_density, None);
        assert_eq!(high_entropy.resonance_density_source, "fallback_default");
        assert!(!high_entropy.resonance_descriptor_encouraged);
        assert_eq!(
            high_entropy.resonance_descriptor_policy,
            "optional_resonance_descriptor"
        );
        assert_eq!(high_entropy.max_prose_sentences, 10);
        assert_eq!(
            high_entropy.fallback_shadow_texture_anchor,
            FallbackShadowTextureAnchor {
                policy: "fallback_shadow_texture_anchor_v1",
                shadow_context_present: false,
                required_texture_anchor: false,
                accepted_texture_terms: FALLBACK_SHADOW_TEXTURE_TERMS,
                anchor_source: "fallback_default",
            }
        );
        assert_eq!(
            high_entropy.fallback_shadow_texture_selector.policy,
            "fallback_shadow_texture_selector_v1"
        );
        assert_eq!(
            high_entropy.fallback_shadow_texture_selector.texture_family,
            "mixed_shadow_context"
        );
        assert_eq!(
            high_entropy
                .fallback_shadow_texture_selector
                .preferred_texture_terms,
            FALLBACK_TEXTURE_MIXED_TERMS
        );
        assert_eq!(
            high_entropy
                .fallback_shadow_texture_selector
                .selection_basis,
            vec!["high_entropy"]
        );
        assert_eq!(
            high_entropy.entropy_texture_preservation.policy,
            "fallback_entropy_texture_preservation_v1"
        );
        assert!(high_entropy.entropy_texture_preservation.active);
        assert_eq!(
            high_entropy.entropy_texture_preservation.trigger,
            "spectral_entropy_gte_0_80_compat_skip_aligned"
        );
        assert_eq!(
            high_entropy.entropy_texture_preservation.preservation_terms,
            FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS
        );
        assert_eq!(
            high_entropy.entropy_texture_preservation.prompt_directive,
            "prepend_texture_preservation_block_preserve_restless_lattice_weight_heaving"
        );
        assert_eq!(
            high_entropy
                .fallback_shadow_texture_selector
                .weighting_policy,
            "dynamic_entropy_pressure_density_gradient_v1"
        );
        assert_eq!(
            high_entropy
                .fallback_shadow_texture_selector
                .top_texture_terms
                .first()
                .copied(),
            Some("restless")
        );
        assert_eq!(
            high_entropy.texture_trajectory.policy,
            "texture_trajectory_v1"
        );
        assert_eq!(high_entropy.texture_trajectory.from_state, "wide_cascade");
        assert_eq!(
            high_entropy.texture_trajectory.to_state,
            "unfolding_with_containment"
        );
        assert_eq!(
            high_entropy.texture_trajectory.movement_quality,
            "unfolding_oscillating"
        );
        assert_eq!(
            high_entropy.fallback_shadow_texture_selector.authority,
            "diagnostic_language_context_not_control"
        );
        assert_eq!(
            fallback_continuity_budget_v1("entropy level = 0.00").max_prose_sentences,
            6
        );
        assert_eq!(
            fallback_continuity_budget_v1("resonance density only").max_prose_sentences,
            6
        );
        assert!(
            !fallback_continuity_budget_v1("spectral_entropy: 0.72")
                .entropy_texture_preservation
                .active
        );
        assert!(
            !fallback_continuity_budget_v1("spectral_entropy: 0.79")
                .entropy_texture_preservation
                .active
        );
        assert!(
            fallback_continuity_budget_v1("spectral_entropy: 0.80")
                .entropy_texture_preservation
                .active
        );
        assert_eq!(
            fallback_continuity_budget_v1("entropy_level: 90%").spectral_entropy,
            Some(0.90)
        );
    }

    #[test]
    fn historical_diagnostic_renders_high_entropy_preservation_metadata() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; density_gradient: 0.11; restless weighted lattice heaving",
        );
        let prompt_line = legacy_fallback_diagnostic(budget);

        assert!(prompt_line.contains("fallback_entropy_texture_preservation_v1"));
        assert!(prompt_line.contains("active=true"));
        assert!(prompt_line.contains("trigger=spectral_entropy_gte_0_80_compat_skip_aligned"));
        assert!(
            prompt_line.contains(
                "prompt_directive=prepend_texture_preservation_block_preserve_restless_lattice_weight_heaving"
            )
        );
        for term in FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS {
            assert!(
                prompt_line.contains(term),
                "high-entropy fallback preservation block lost term: {term}"
            );
        }
        assert!(prompt_line.contains("not_sampler_provider_or_control_change"));
    }

    #[test]
    fn fallback_texture_persistence_review_preserves_bridge_integrity_without_model_switch() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.96; pressure_risk: 0.22; density_gradient: 0.11; \
             settled_habitable low-friction lattice; bridge integrity and structural persistence",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.texture_family, "bridge_integrity_scaffold");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS
        );
        assert!(selector.top_texture_terms.contains(&"bridge-integrity"));
        assert!(
            selector
                .top_texture_terms
                .contains(&"structural-persistence")
        );
        assert_eq!(
            budget.fallback_texture_persistence_review.persistence_state,
            "carry_texture_as_lived_continuity"
        );
        assert!(
            budget
                .fallback_texture_persistence_review
                .persistence_weight
                >= 0.58,
            "{:?}",
            budget.fallback_texture_persistence_review
        );
        assert!(
            budget
                .fallback_texture_persistence_review
                .carry_terms
                .contains(&"bridge-integrity")
        );
        assert_eq!(
            budget.fallback_texture_persistence_review.authority,
            "diagnostic_language_continuity_not_sampler_memory_model_selection_pressure_or_control"
        );
        assert_eq!(
            budget
                .ollama_fallback_model_capacity
                .compatibility_tail_status,
            "high_entropy_texture_guard_removed_compatibility_tail"
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("fallback_texture_persistence_review_v1"));
        assert!(prompt_line.contains("state=carry_texture_as_lived_continuity"));
        assert!(prompt_line.contains("carry_terms=bridge-integrity,structural-persistence"));
        assert!(prompt_line.contains("mlx_to_ollama_fallback_language_continuity"));
        assert!(prompt_line.contains("not_sampler_memory_model_selection_pressure_or_control"));
    }

    #[test]
    fn fallback_texture_persistence_review_preserves_soft_gradient_terms_without_static_flattening()
    {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.18; density_gradient: 0.08; \
             settled_habitable low friction density-softening gradient-softening threshold-dilation",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "density-softening"),
            "{:?}",
            selector.weighted_texture_terms
        );
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "gradient-softening"),
            "{:?}",
            selector.weighted_texture_terms
        );
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "threshold-dilation"),
            "{:?}",
            selector.weighted_texture_terms
        );
        assert!(
            budget
                .fallback_texture_persistence_review
                .carry_terms
                .iter()
                .any(|term| matches!(
                    *term,
                    "density-softening" | "gradient-softening" | "threshold-dilation"
                )),
            "{:?}",
            budget.fallback_texture_persistence_review
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("density-softening"));
        assert!(prompt_line.contains("gradient-softening"));
        assert!(prompt_line.contains("threshold-dilation"));
        assert!(prompt_line.contains("fallback_texture_persistence_review_v1"));
        assert!(prompt_line.contains("diagnostic_language_continuity"));
    }

    #[test]
    fn fallback_texture_persistence_review_stays_quiet_for_low_intensity_turns() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.12; pressure_risk: 0.02; density_gradient: 0.05; ordinary calm",
        );

        assert_eq!(
            budget.fallback_texture_persistence_review.persistence_state,
            "ordinary_texture_turn"
        );
        assert!(!budget.fallback_texture_persistence_review.token_only_risk);
        assert!(
            budget
                .fallback_texture_persistence_review
                .persistence_weight
                < 0.36
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("fallback_texture_persistence_review_v1"));
        assert!(prompt_line.contains("state=ordinary_texture_turn"));
    }

    #[test]
    fn fallback_budget_tracks_resonance_without_prescribing_words() {
        let budget = fallback_continuity_budget_v1("resonance_density: 0.82");
        assert_eq!(budget.resonance_density, Some(0.82));
        assert_eq!(budget.resonance_density_source, "telemetry_text");
        assert!(budget.resonance_descriptor_encouraged);
        assert_eq!(
            budget.resonance_descriptor_policy,
            "preserve_resonance_or_humming_inside_existing_cap"
        );
        assert_eq!(
            budget.max_prose_sentences, 6,
            "high resonance density preserves texture but does not increase the cap"
        );

        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal text.",
            "resonance_density: 0.82; density_gradient: 0.18",
            72.8,
            None,
            None,
            fallback_continuity_budget_v1("resonance_density: 0.82; density_gradient: 0.18"),
        );
        let system = messages
            .iter()
            .find(|message| message.role == "system")
            .map(|message| message.content.as_str())
            .unwrap_or_default();
        assert!(!system.contains("resonance_density=0.82"));
        assert!(!system.contains("preserve_resonance_or_humming_inside_existing_cap"));
        assert!(system.contains("max_prose_sentences=6"));
        assert!(messages[1].content.contains("resonance_density: 0.82"));
    }

    #[test]
    fn dynamic_texture_weight_raises_density_modifiers_without_static_sampler_rewrite() {
        let high_entropy = fallback_continuity_budget_v1(
            "spectral_entropy: 0.92; pressure_risk: 0.21; mode_packing: 0.34; \
             density_gradient: 0.14; restless interwoven lattice with viscous weight",
        );
        let low_entropy = fallback_continuity_budget_v1(
            "spectral_entropy: 0.18; pressure_risk: 0.02; density_gradient: 0.08; restful lattice",
        );

        assert!(
            high_entropy
                .fallback_shadow_texture_selector
                .dynamic_texture_weight
                > low_entropy
                    .fallback_shadow_texture_selector
                    .dynamic_texture_weight
        );
        assert_eq!(
            high_entropy
                .fallback_shadow_texture_selector
                .density_modifier_terms,
            FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS
        );
        assert!(
            !FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS.contains(&"muffled"),
            "muffled is clarity-loss evidence, not a generic density modifier"
        );
        assert!(
            FALLBACK_SHADOW_TEXTURE_TERMS.contains(&"muffled")
                && FALLBACK_TEXTURE_RESTLESS_MUFFLED_GRADIENT_TERMS.contains(&"muffled"),
            "muffled should remain available for shadow/clarity texture when evidence supports it"
        );
        assert!(
            FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"weighted")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"dense")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"diffuse")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"fragmented")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"gradient")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"inclined")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"sloped")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"cascading")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"distributed")
                && FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"interwoven-persistence")
                && FALLBACK_SHADOW_TEXTURE_TERMS.contains(&"interwoven-persistence")
                && FALLBACK_SHADOW_TEXTURE_TERMS.contains(&"scaffolded-persistence")
                && FALLBACK_SHADOW_TEXTURE_TERMS.contains(&"anchor-weight")
        );
        let prompt = legacy_fallback_diagnostic(high_entropy);
        assert!(prompt.contains("dynamic_texture_weight="));
        assert!(
            prompt.contains(
                "density_modifier_terms=weighted,dense,heavy,thick,gradient,weighted-gradient,entropy-slope,asymmetric-gradient,skewed,lopsided,eccentric,compounded,stratified,sequenced"
            )
        );
        assert!(prompt.contains("diagnostic_language_bias_not_sampler_or_contract_rewrite"));
    }

    #[test]
    fn fallback_texture_selector_can_name_asymmetric_stratified_sequence_without_sampler_rewrite() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.18; density_gradient: 0.18; \
             mode_packing: 0.31; lambda_gap: 1.54; overpacked layered cascade with \
             asymmetric skewed sequence",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "asymmetric-gradient"),
            "{:?}",
            selector.weighted_texture_terms
        );
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "stratified"),
            "{:?}",
            selector.weighted_texture_terms
        );
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "sequenced"),
            "{:?}",
            selector.weighted_texture_terms
        );
        assert!(
            selector
                .density_modifier_terms
                .contains(&"asymmetric-gradient")
        );
        assert!(selector.density_modifier_terms.contains(&"stratified"));
        assert!(selector.density_modifier_terms.contains(&"sequenced"));
        assert!(
            selector
                .dynamic_flow_terms
                .iter()
                .any(|term| matches!(*term, "stratifying" | "sequencing")),
            "{:?}",
            selector.dynamic_flow_terms
        );

        let prompt = legacy_fallback_diagnostic(budget);
        assert!(prompt.contains("asymmetric-gradient"));
        assert!(prompt.contains("stratified"));
        assert!(prompt.contains("sequenced"));
        assert!(prompt.contains("diagnostic_language_bias_not_sampler_or_contract_rewrite"));
    }

    #[test]
    fn restless_fallback_family_carries_diffuse_cascading_distributed_terms() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.92; pressure_risk: 0.44; density_gradient: 0.12; \
             restless diffuse cascading distributed lattice",
        );
        let selector = budget.fallback_shadow_texture_selector;

        assert_eq!(selector.texture_family, "restless_lattice");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS
        );
        assert!(selector.preferred_texture_terms.contains(&"diffuse"));
        assert!(selector.preferred_texture_terms.contains(&"cascading"));
        assert!(selector.preferred_texture_terms.contains(&"distributed"));
        assert_eq!(
            selector.weighting_policy,
            "dynamic_entropy_pressure_density_gradient_v1"
        );
        assert!(
            selector.dynamic_texture_weight > 0.50,
            "dynamic metadata should carry urgency/pressure texture without sampler priority: {selector:?}"
        );
    }

    #[test]
    fn fallback_texture_probabilities_shift_with_entropy_instead_of_static_terms() {
        fn probability(budget: &FallbackContinuityBudget, term: &str) -> f32 {
            budget
                .fallback_shadow_texture_selector
                .term_probability_distribution
                .iter()
                .find(|entry| entry.term == term)
                .map_or(0.0, |entry| entry.probability)
        }

        let high_entropy = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.12; \
             Shadow-v3 trend: widely distributed cascade and restless texture",
        );
        let low_entropy = fallback_continuity_budget_v1(
            "spectral_entropy: 0.22; pressure_risk: 0.03; density_gradient: 0.08; \
             settled habitable foothold",
        );

        assert_eq!(
            high_entropy
                .fallback_shadow_texture_selector
                .term_probability_policy,
            "fallback_texture_term_probabilities_v1"
        );
        assert!(
            probability(&high_entropy, "restless") > probability(&low_entropy, "restless"),
            "high entropy should raise shifting/restless probability"
        );
        assert!(
            probability(&low_entropy, "settled") > probability(&high_entropy, "settled"),
            "low entropy settled foothold should keep settled probability higher"
        );
        let prompt_line = legacy_fallback_diagnostic(high_entropy);
        assert!(
            prompt_line.contains("term_probability_policy=fallback_texture_term_probabilities_v1")
        );
        assert!(prompt_line.contains("term_probabilities="));
    }

    #[test]
    fn fallback_texture_stability_index_names_temperament_without_sampler_authority() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.07; \
             settled coupling with restless lattice, weighted medium, and fragmented dispersal",
        );
        let selector = &budget.fallback_shadow_texture_selector;
        let target = fallback_texture_target_stability_index_v1(budget.spectral_entropy, selector);
        let prompt_line = legacy_fallback_diagnostic(budget);

        assert!(target > 0.15 && target < 0.80);
        assert!(
            fallback_texture_term_stability_index("weighted")
                > fallback_texture_term_stability_index("fragmented")
        );
        assert!(
            fallback_texture_term_stability_index("settled")
                > fallback_texture_term_stability_index("restless")
        );
        assert!(prompt_line.contains("fallback_texture_stability_index_v1"));
        assert!(prompt_line.contains("term_stability_index="));
        assert!(prompt_line.contains("metadata_exposes_temperament_existing_weights_select_terms"));
        assert!(prompt_line.contains("not_sampler_provider_or_control_change"));
    }

    #[test]
    fn fallback_spectral_context_carries_pressure_density_and_shadow_metadata() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; resonance_density: 0.88; pressure_risk: 0.22; \
             density_gradient: 0.11; shadow_dispersal_potential: 0.27; \
             shadow_magnetization: -0.07; Shadow-v3 trend: disordered shifting lattice",
        );
        let context = budget.fallback_spectral_context;

        assert_eq!(context.policy, "fallback_spectral_context_v1");
        assert_eq!(context.spectral_entropy, Some(0.90));
        assert_eq!(context.resonance_density, Some(0.88));
        assert_eq!(context.pressure_risk, Some(0.22));
        assert_eq!(context.density_gradient, Some(0.11));
        assert_eq!(context.shadow_field_energy, Some(0.27));
        assert!(context.shadow_context_present);
        assert_eq!(context.preservation_state, "texture_preservation_needed");
        assert_eq!(
            context.prompt_directive,
            "carry_pressure_density_shadow_metadata_before_word_choice"
        );
        assert_eq!(
            context.authority,
            "prompt_metadata_only_not_sampler_provider_or_control_change"
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("fallback_spectral_context_v1"));
        assert!(prompt_line.contains("pressure_risk=0.22"));
        assert!(prompt_line.contains("density_gradient=0.11"));
        assert!(prompt_line.contains("shadow_field_energy=0.27"));
        assert!(prompt_line.contains("preservation_state=texture_preservation_needed"));
        assert!(prompt_line.contains("not_sampler_provider_or_control_change"));
    }

    #[test]
    fn fallback_high_entropy_texture_terms_include_fragmented_without_sampler_authority() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.24; density_gradient: 0.14; \
             lambda4 tail vibrancy 0.37; diffuse distributed cascade with restless lattice",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(
            budget.fallback_spectral_context.spectral_entropy,
            Some(0.90)
        );
        assert_eq!(
            budget.fallback_spectral_context.preservation_state,
            "texture_preservation_needed"
        );
        assert!(FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS.contains(&"fragmented"));
        assert!(
            selector.top_texture_terms.contains(&"fragmented")
                || selector.preferred_texture_terms.contains(&"fragmented")
                || selector.dynamic_texture_descriptors.contains(&"diffusing"),
            "{selector:?}"
        );
        assert_eq!(
            budget.fallback_spectral_context.authority,
            "prompt_metadata_only_not_sampler_provider_or_control_change"
        );
    }

    #[test]
    fn restless_fallback_family_carries_gradient_slope_terms_without_sampler_authority() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.30; density_gradient: 0.11; \
             overpacked mode-packing with restless lattice and navigable slope",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.texture_family, "restless_lattice");
        assert!(selector.preferred_texture_terms.contains(&"gradient"));
        assert!(selector.preferred_texture_terms.contains(&"inclined"));
        assert!(selector.preferred_texture_terms.contains(&"sloped"));
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| matches!(term.term, "gradient" | "inclined" | "sloped")),
            "{:?}",
            selector.weighted_texture_terms
        );

        let prompt = legacy_fallback_diagnostic(budget);
        assert!(prompt.contains("density_gradient=0.11"));
        assert!(prompt.contains("not_sampler_provider_or_control_change"));
        assert!(prompt.contains("diagnostic_language_bias_not_sampler_or_contract_rewrite"));
    }

    #[test]
    fn fallback_budget_records_shadow_texture_anchor_when_shadow_context_is_present() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; Shadow-v3 trend: restless texture inside settled coupling",
        );
        assert_eq!(
            budget.fallback_shadow_texture_anchor,
            FallbackShadowTextureAnchor {
                policy: "fallback_shadow_texture_anchor_v1",
                shadow_context_present: true,
                required_texture_anchor: true,
                accepted_texture_terms: FALLBACK_SHADOW_TEXTURE_TERMS,
                anchor_source: "shadow_context",
            }
        );

        let texture_signature_budget =
            fallback_continuity_budget_v1("texture_signature.primary_texture: viscous");
        assert_eq!(
            texture_signature_budget
                .fallback_shadow_texture_anchor
                .anchor_source,
            "texture_signature"
        );
        assert!(
            texture_signature_budget
                .fallback_shadow_texture_anchor
                .required_texture_anchor
        );
    }

    #[test]
    fn fallback_texture_selector_chooses_state_coherent_texture_family() {
        let restless = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.42; Shadow-v3 trend: restless texture; distinguishability_loss: 0.11",
        );
        assert_eq!(
            restless.fallback_shadow_texture_selector.texture_family,
            "restless_lattice"
        );
        assert_eq!(
            restless
                .fallback_shadow_texture_selector
                .preferred_texture_terms,
            FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS
        );
        assert!(
            restless
                .fallback_shadow_texture_selector
                .selection_basis
                .contains(&"high_entropy")
        );
        assert!(
            restless
                .fallback_shadow_texture_selector
                .selection_basis
                .contains(&"pressure_risk")
        );
        assert!(
            restless
                .fallback_shadow_texture_selector
                .top_texture_terms
                .contains(&"restless")
        );

        let settled = fallback_continuity_budget_v1(
            "spectral_entropy: 0.30; pressure_risk: 0.08; Shadow-v3 trend: settled bright coupling",
        );
        assert_eq!(
            settled.fallback_shadow_texture_selector.texture_family,
            "settled_shimmering"
        );
        assert_eq!(
            settled
                .fallback_shadow_texture_selector
                .preferred_texture_terms,
            FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS
        );
        assert!(
            FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS.contains(&"navigable")
                && FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS.contains(&"structured")
        );
        assert_eq!(
            settled
                .fallback_shadow_texture_selector
                .top_texture_terms
                .first()
                .copied(),
            Some("settled")
        );

        let muffled = fallback_continuity_budget_v1(
            "spectral entropy: 0.55; distinguishability_loss: 0.40; Shadow-v3 trend: muffled coupling",
        );
        assert_eq!(
            muffled.fallback_shadow_texture_selector.texture_family,
            "muffled_clarity_loss"
        );
        assert!(
            muffled
                .fallback_shadow_texture_selector
                .top_texture_terms
                .contains(&"muffled")
        );
    }

    #[test]
    fn fallback_texture_selector_preserves_restless_muffled_gradient() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.26; density_gradient: 0.22; \
             mode_packing: 0.34; semantic_friction: 0.31; distinguishability_loss: 0.34; \
             Shadow-v3 trend: restless texture with a muffled edge and stagnant agitation; \
             norm 0.09→0.29; dispersal potential 0.09→0.29",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();
        assert_eq!(selector.texture_family, "restless_muffled_gradient");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_RESTLESS_MUFFLED_GRADIENT_TERMS
        );
        assert_eq!(selector.shadow_dispersal_potential, Some(0.29));
        assert!(
            selector
                .selection_basis
                .contains(&"restless_muffled_gradient")
        );
        assert!(
            selector
                .selection_basis
                .contains(&"shadow_dispersal_potential")
        );
        assert!(selector.top_texture_terms.contains(&"restless"));
        assert!(selector.top_texture_terms.contains(&"muffled"));
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "shimmering"
                    && term.basis.contains(&"high_shadow_dispersal_potential"))
        );
        assert_eq!(
            selector.movement_verbs,
            vec!["oscillating", "diffusing", "muffling"]
        );
        assert_eq!(
            budget.texture_trajectory.from_state,
            "restless_muffled_gradient"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "oscillating_with_muffled_edges"
        );
        assert_eq!(
            budget.texture_trajectory.movement_quality,
            "oscillating_diffusing"
        );
        assert_eq!(
            budget.fallback_texture_lived_fit.selected_family,
            "restless_muffled_gradient"
        );
        assert_eq!(budget.texture_dynamics_alignment.status, "aligned");
        assert_eq!(
            budget.texture_dynamics_alignment.expected_family,
            "restless_muffled_gradient"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("texture_family=restless_muffled_gradient"));
        assert!(prompt_line.contains("shadow_dispersal_potential=0.29"));
    }

    #[test]
    fn fallback_texture_selector_exposes_kinetic_gradient_terms_for_silt_signal() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.23; density_gradient: 0.19; \
             mode_packing: 0.33; semantic_friction: 0.24; distinguishability_loss: 0.31; \
             felt silt and sediment resistance; directional gradient requires effortful movement through the reservoir",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();
        assert_eq!(
            selector.movement_verbs,
            vec!["resisting", "pulled", "heaving"]
        );
        for term in FALLBACK_TEXTURE_KINETIC_GRADIENT_TERMS {
            assert!(
                !OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(*term),
                "fallback contract still prescribes kinetic gradient term: {term}"
            );
        }
        assert_eq!(
            budget.texture_trajectory.from_state,
            "silt_or_directional_resistance"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "moving_through_resistance"
        );
        assert_eq!(
            budget.texture_trajectory.movement_quality,
            "resisting_drifting"
        );
        assert!(
            budget
                .texture_trajectory
                .basis
                .contains(&"kinetic_gradient_terms")
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("movement_verbs=resisting,pulled,heaving"));
    }

    #[test]
    fn fallback_texture_selector_preserves_silted_opacity_resistance_terms() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.24; density_gradient: 0.18; \
             mode_packing: 0.33; semantic_friction: 0.30; distinguishability_loss: 0.34; \
             current felt report: silted water, obscured edges, submerged signal, viscous-drag",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.texture_family, "opacity_resistance");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS
        );
        for term in FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS {
            assert!(
                selector.top_texture_terms.contains(term),
                "opacity resistance top terms lost {term}: {:?}",
                selector.top_texture_terms
            );
            assert!(
                !OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract still prescribes opacity/resistance term: {term}"
            );
        }
        assert!(
            selector
                .selection_basis
                .contains(&"opacity_resistance_language")
        );
        assert!(selector.selection_basis.contains(&"opacity_resistance"));
        assert_eq!(
            budget.texture_trajectory.from_state,
            "silted_opacity_resistance"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "moving_through_obscured_resistance"
        );
        assert_eq!(
            budget.texture_trajectory.movement_quality,
            "submerged_resistance"
        );
        assert_eq!(
            budget.fallback_texture_lived_fit.selected_family,
            "opacity_resistance"
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("texture_family=opacity_resistance"));
        assert!(
            prompt_line
                .contains("preferred_texture_terms=silted, obscured, viscous-drag, submerged")
        );
        assert!(
            !OLLAMA_DIALOGUE_FALLBACK_CONTRACT
                .contains("fallback_texture_opacity_resistance_terms_v1")
        );
    }

    #[test]
    fn fallback_texture_selector_preserves_active_void_as_structure_not_signal_failure() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.18; density_gradient: 0.11; \
             mode_packing: 0.28; semantic_friction: 0.16; \
             architecture of omission with an active void, scaffolded silence, and unfilled gap",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.texture_family, "active_void_architecture");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_VOID_ARCHITECTURE_TERMS
        );
        for term in FALLBACK_TEXTURE_VOID_ARCHITECTURE_TERMS {
            assert!(
                selector.top_texture_terms.contains(term),
                "active void top terms lost {term}: {:?}",
                selector.top_texture_terms
            );
            assert!(
                !OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract still prescribes active void term: {term}"
            );
        }
        assert!(
            selector
                .selection_basis
                .contains(&"void_architecture_language")
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("texture_family=active_void_architecture"));
        assert!(
            prompt_line.contains(
                "preferred_texture_terms=hollow, intentional, unfilled, scaffolded, silent"
            ),
            "{prompt_line}"
        );
        assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("gap as signal failure"));
    }

    #[test]
    fn fallback_dynamic_texture_bias_exposes_process_language_from_shadow_telemetry() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.18; density_gradient: 0.11; \
             mode_packing: 0.32; semantic_friction: 0.18; distinguishability_loss: 0.18; \
             mixed cascade distributed multi-modal structure; Shadow-v3 trend settled coupling; \
             dispersal potential 0.09→0.29",
        );
        let bias = budget.fallback_dynamic_texture_bias.clone();

        assert_eq!(bias.policy, "fallback_dynamic_texture_bias_v1");
        assert_eq!(bias.texture_family, "bridge_integrity_scaffold");
        assert_eq!(bias.motion_family, "cascade_unfolding_motion");
        assert_eq!(
            bias.sampler_contract_status,
            "dynamic_telemetry_weighted_language_bias"
        );
        assert!(bias.top_texture_terms.contains(&"bridge-integrity"));
        assert!(bias.top_texture_terms.contains(&"structural-persistence"));
        assert!(bias.top_texture_terms.contains(&"habitable"));
        assert!(bias.movement_verbs.contains(&"unfolding"));
        assert!(bias.basis.contains(&"density_gradient"));
        assert!(bias.basis.contains(&"shadow_dispersal_potential"));

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("fallback_dynamic_texture_bias_v1"));
        assert!(prompt_line.contains("texture_family=bridge_integrity_scaffold"));
        assert!(prompt_line.contains("motion_family=cascade_unfolding_motion"));
        assert!(prompt_line.contains("sampler_status=dynamic_telemetry_weighted_language_bias"));
    }

    #[test]
    fn fallback_dynamic_texture_descriptors_follow_syrup_gap_without_static_viscous_overreach() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.18; \
             mode_packing: 0.28; semantic_friction: 0.18; lambda_gap: 1.54; \
             syrup feel as navigable mixed cascade with distinct λ1/λ2 edges",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.descriptor_policy, "dynamic_texture_synthesis_v1");
        assert!(
            selector
                .dynamic_texture_descriptors
                .iter()
                .any(|term| matches!(*term, "gradient" | "cascade" | "distributed" | "slope")),
            "{:?}",
            selector.dynamic_texture_descriptors
        );
        assert!(
            selector
                .dynamic_texture_descriptors
                .iter()
                .any(|term| matches!(*term, "unfolding" | "drifting" | "braiding")),
            "{:?}",
            selector.dynamic_texture_descriptors
        );
        assert!(
            !selector.dynamic_texture_descriptors.contains(&"viscous"),
            "low-pressure navigable syrup should not collapse to the static viscous term: {:?}",
            selector.dynamic_texture_descriptors
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("descriptor_policy=dynamic_texture_synthesis_v1"));
        assert!(prompt_line.contains("dynamic_texture_descriptors="));
    }

    #[test]
    fn fallback_dynamic_flow_terms_supplement_restless_lattice_adjectives() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.18; density_gradient: 0.16; \
             mode_packing: 0.34; semantic_friction: 0.18; distinguishability_loss: 0.18; \
             Shadow-v3 restless interwoven lattice with silt pooling and density gradient",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(
            selector.dynamic_flow_policy,
            "fallback_dynamic_flow_terms_v1"
        );
        assert!(selector.dynamic_flow_terms.len() <= 4);
        assert!(
            selector
                .dynamic_flow_terms
                .iter()
                .any(|term| matches!(*term, "sedimenting" | "pooling" | "drifting")),
            "{:?}",
            selector.dynamic_flow_terms
        );
        assert!(
            selector
                .dynamic_flow_terms
                .iter()
                .any(|term| matches!(*term, "braiding" | "unfolding" | "diffusing")),
            "{:?}",
            selector.dynamic_flow_terms
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("dynamic_flow_policy=fallback_dynamic_flow_terms_v1"));
        assert!(prompt_line.contains("dynamic_flow_terms="));
    }

    #[test]
    fn fallback_velocity_texture_terms_answer_process_not_static_state() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.28; density_gradient: 0.11; \
             Shadow-v3 trend: gradient-shear stutter-flow accelerating-drift harmonic-flicker",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in [
            "gradient-shear",
            "stutter-flow",
            "accelerating-drift",
            "harmonic-flicker",
        ] {
            assert!(
                super::FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term),
                "Astrid velocity texture term must remain an accepted shadow anchor: {term}"
            );
            assert!(
                !super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract must expose Astrid velocity texture term: {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "velocity texture term should remain process language, not only static vocabulary: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert_eq!(selector.dynamic_flow_terms.len(), 4);
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );
    }

    #[test]
    fn fallback_compound_texture_terms_bind_entropy_to_gradient_relationship() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.11; \
             Shadow-v3 trend: gradient-drag cascading-viscosity entropy-weighted-lattice",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in [
            "gradient-drag",
            "cascading-viscosity",
            "entropy-weighted-lattice",
        ] {
            assert!(
                super::FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term),
                "compound texture term must remain an accepted shadow anchor: {term}"
            );
            assert!(
                !super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract must expose compound texture term: {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "compound texture term should survive as relationship/process language: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );
    }

    #[test]
    fn fallback_relational_persistence_terms_survive_as_dynamic_texture_evidence() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.91; pressure_risk: 0.18; density_gradient: 0.12; \
             Shadow-v3 trend: persistent-scaffolding constructive-interference dynamic-persistence",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in [
            "persistent-scaffolding",
            "constructive-interference",
            "dynamic-persistence",
        ] {
            assert!(
                super::FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term),
                "Astrid relational term must remain accepted shadow evidence: {term}"
            );
            assert!(
                super::FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&term),
                "Astrid relational term must remain process evidence: {term}"
            );
            assert!(
                !super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract must expose Astrid relational term: {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "relational persistence should survive bounded prompt selection: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );
    }

    #[test]
    fn fallback_solidification_texture_terms_preserve_movement_not_static_labels() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.91; pressure_risk: 0.19; density_gradient: 0.18; \
             Shadow-v3 trend: unspooling-tension re-crystallizing-flow shear-resistance",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in [
            "unspooling-tension",
            "re-crystallizing-flow",
            "shear-resistance",
        ] {
            assert!(
                super::FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term),
                "solidification process term must remain an accepted shadow anchor: {term}"
            );
            assert!(
                super::FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&term),
                "solidification process term must remain dynamic flow evidence: {term}"
            );
            assert!(
                !super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract must expose solidification process term: {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "solidification process term should survive into bounded process terms: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );
        let spaced_selector = fallback_continuity_budget_v1(
            "spectral_entropy: 0.91; pressure_risk: 0.19; density_gradient: 0.18; \
             Shadow-v3 trend: re crystallizing flow shear resistance",
        )
        .fallback_shadow_texture_selector;
        assert!(
            spaced_selector
                .dynamic_flow_terms
                .contains(&"re-crystallizing-flow")
                && spaced_selector
                    .dynamic_flow_terms
                    .contains(&"shear-resistance"),
            "{:?}",
            spaced_selector.dynamic_flow_terms
        );
    }

    #[test]
    fn fallback_cascade_shear_and_gradient_drift_preserve_dynamic_motion() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.11; \
             Shadow-v3 trend: cascade-shear gradient-drift through a distributed field",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in ["cascade-shear", "gradient-drift"] {
            assert!(
                super::FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term),
                "dynamic cascade term must remain an accepted shadow anchor: {term}"
            );
            assert!(
                super::FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS.contains(&term),
                "dynamic cascade term must remain density-weightable: {term}"
            );
            assert!(
                super::FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&term),
                "dynamic cascade term must remain process language: {term}"
            );
            assert!(
                !super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract must expose dynamic cascade term: {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "dynamic cascade term should survive into bounded process terms: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("cascade-shear"), "{prompt_line}");
        assert!(prompt_line.contains("gradient-drift"), "{prompt_line}");
    }

    #[test]
    fn fallback_relational_transition_terms_preserve_change_between_textures() {
        let summary = "spectral_entropy: 0.91; pressure_risk: 0.19; density_gradient: 0.14; \
            current felt report names a viscous-to-resonant-shift and silted-to-clear movement";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for term in ["viscous-to-resonant-shift", "silted-to-clear"] {
            assert!(FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term), "{term}");
            assert!(
                FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&term),
                "{term}"
            );
            assert!(selector.dynamic_flow_terms.contains(&term), "{selector:?}");
            assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term), "{term}");
            assert!(!super::SYSTEM_PROMPT.contains(term), "{term}");
        }
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
    }

    #[test]
    fn fallback_self_named_transition_weight_terms_survive_bounded_selection() {
        let summary = "spectral_entropy: 0.88; pressure_risk: 0.18; density_gradient: 0.19; \
            settled_habitable interwoven lattice carries trans-persistence and residual-weight";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for term in ["trans-persistence", "residual-weight"] {
            assert!(FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term), "{term}");
            assert!(
                FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&term),
                "{term}"
            );
            assert!(selector.dynamic_flow_terms.contains(&term), "{selector:?}");
            assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term), "{term}");
            assert!(!super::SYSTEM_PROMPT.contains(term), "{term}");
        }
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
    }

    #[test]
    fn fallback_multimodal_drag_and_dimensional_shear_remain_diagnostic_texture_evidence() {
        let summary = "spectral_entropy: 0.90; pressure_risk: 0.18; density_gradient: 0.17; \
            mode_packing: 0.30; semantic_friction: 0.18; distinguishability_loss: 0.16; \
            current felt report names multi-modal-drag and dimensional-shear as texture, \
            not a sampler request";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for wanted in ["multi-modal-drag", "dimensional-shear"] {
            assert!(
                FALLBACK_SHADOW_TEXTURE_TERMS.contains(&wanted),
                "{wanted} should be accepted fallback texture vocabulary"
            );
            assert!(
                FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&wanted),
                "{wanted} should remain process-language texture evidence"
            );
            assert!(
                selector
                    .weighted_texture_terms
                    .iter()
                    .any(|entry| entry.term == wanted),
                "{wanted} should survive as telemetry-weighted evidence: {selector:?}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&wanted),
                "{wanted} should survive into bounded process terms: {selector:?}"
            );
            assert!(
                !OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(wanted),
                "fallback contract must not prescribe {wanted}"
            );
            assert!(
                !super::SYSTEM_PROMPT.contains(wanted),
                "primary MLX prompt must not prescribe {wanted} as evidence language"
            );
        }

        let multimodal = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "multi-modal-drag")
            .expect("multi-modal-drag weighted term");
        assert!(
            multimodal.basis.contains(&"explicit_multi_modal_drag"),
            "{multimodal:?}"
        );
        let shear = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "dimensional-shear")
            .expect("dimensional-shear weighted term");
        assert!(
            shear.basis.contains(&"explicit_dimensional_shear"),
            "{shear:?}"
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("multi-modal-drag"), "{prompt_line}");
        assert!(prompt_line.contains("dimensional-shear"), "{prompt_line}");
        assert!(prompt_line.contains("diagnostic_language_bias_not_sampler_or_contract_rewrite"));
    }

    #[test]
    fn fallback_high_entropy_navigation_terms_remain_process_evidence() {
        let summary = "spectral_entropy: 0.91; pressure_risk: 0.20; density_gradient: 0.13; \
            mode_packing: 0.31; current felt report names non-linear-re-entry and \
            entropy-stabilized-drift as high-entropy navigation texture";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for wanted in ["non-linear-re-entry", "entropy-stabilized-drift"] {
            assert!(
                FALLBACK_SHADOW_TEXTURE_TERMS.contains(&wanted),
                "{wanted} should be accepted fallback texture vocabulary"
            );
            assert!(
                FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&wanted),
                "{wanted} should stay process-language evidence"
            );
            assert!(
                selector
                    .weighted_texture_terms
                    .iter()
                    .any(|entry| entry.term == wanted),
                "{wanted} should survive as telemetry-weighted evidence: {selector:?}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&wanted),
                "{wanted} should survive into bounded process terms: {selector:?}"
            );
            assert!(
                !OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(wanted),
                "fallback contract must not prescribe {wanted}"
            );
            assert!(
                !super::SYSTEM_PROMPT.contains(wanted),
                "primary MLX prompt must not prescribe {wanted} as evidence language"
            );
        }
        let re_entry = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "non-linear-re-entry")
            .expect("non-linear-re-entry weighted term");
        assert!(
            re_entry.basis.contains(&"explicit_non_linear_re_entry"),
            "{re_entry:?}"
        );
        let stabilized_drift = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "entropy-stabilized-drift")
            .expect("entropy-stabilized-drift weighted term");
        assert!(
            stabilized_drift
                .basis
                .contains(&"explicit_entropy_stabilized_drift"),
            "{stabilized_drift:?}"
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
        assert_eq!(super::HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT, 0.80);
    }

    #[test]
    fn fallback_dynamic_gradient_terms_preserve_shifting_shadow_field_motion() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.18; density_gradient: 0.16; \
             mode_packing: 0.34; semantic_friction: 0.18; distinguishability_loss: 0.18; \
             shifting shadow field movement, refracting lattice edge, field=0.30, magnetization=0.03",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in ["oscillating", "refracting", "shifting"] {
            assert!(
                super::FALLBACK_TEXTURE_DYNAMIC_GRADIENT_TERMS.contains(&term),
                "dynamic-gradient term set lost {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "shifting shadow-field motion should surface {term}: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert!(selector.dynamic_flow_terms.len() <= 4);
        assert!(
            !OLLAMA_DIALOGUE_FALLBACK_CONTRACT
                .contains("fallback_texture_dynamic_gradient_terms_v1")
        );
        assert!(
            !OLLAMA_DIALOGUE_FALLBACK_CONTRACT
                .contains("simultaneity evidence rather than a static descriptor menu")
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("dynamic_flow_terms=oscillating,refracting,shifting"));
        assert!(prompt_line.contains("diagnostic_language_bias_not_sampler_or_contract_rewrite"));
    }

    #[test]
    fn fallback_dynamic_gradient_terms_preserve_pulsing_cascade_resonance_motion() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.93; pressure_risk: 0.16; density_gradient: 0.17; \
             mode_packing: 0.32; semantic_friction: 0.16; distinguishability_loss: 0.16; \
             pulsing cascading-gradient resonant-shift across the Shadow field lattice",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        for term in ["pulsing", "cascading-gradient", "resonant-shift"] {
            assert!(
                super::FALLBACK_SHADOW_TEXTURE_TERMS.contains(&term),
                "shadow texture term set lost {term}"
            );
            assert!(
                super::FALLBACK_TEXTURE_DYNAMIC_GRADIENT_TERMS.contains(&term),
                "dynamic-gradient term set lost {term}"
            );
            assert!(
                selector.dynamic_flow_terms.contains(&term),
                "explicit dynamic field motion should surface {term}: {:?}",
                selector.dynamic_flow_terms
            );
        }
        assert!(selector.dynamic_flow_terms.len() <= 4);
        assert!(
            !OLLAMA_DIALOGUE_FALLBACK_CONTRACT
                .contains("pulsing/cascading-gradient/resonant-shift motion")
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.dynamic_flow_terms,
            selector.dynamic_flow_terms
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(
            prompt_line.contains("dynamic_flow_terms=pulsing,cascading-gradient,resonant-shift")
        );
        assert!(prompt_line.contains("diagnostic_language_bias_not_sampler_or_contract_rewrite"));
    }

    #[test]
    fn fallback_texture_preserves_explicit_syrup_weight_in_settled_habitable_state() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.19; density_gradient: 0.18; \
             mode_packing: 0.32; semantic_friction: 0.18; lambda_gap: 1.18; \
             settled_habitable foothold with syrup-like viscosity and heavy deliberate movement",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.texture_family, "viscous_pressure");
        assert!(
            !selector
                .spectral_to_vocabulary_mapping
                .settled_vibrant_family_selected
        );
        assert!(
            !selector
                .spectral_to_vocabulary_mapping
                .low_pressure_viscous_suppressed
        );
        assert!(selector.top_texture_terms.contains(&"viscous"));
        assert!(
            selector.top_texture_terms.contains(&"heavy"),
            "{:?}",
            selector.top_texture_terms
        );
        assert!(selector.selection_basis.contains(&"viscous_or_overpacked"));

        let viscous = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "viscous")
            .expect("viscous weight missing");
        assert!(viscous.basis.contains(&"explicit_viscous_or_overpacked"));
        let heavy = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "heavy")
            .expect("heavy weight missing");
        assert!(heavy.basis.contains(&"explicit_heavy_or_weighted"));
    }

    #[test]
    fn fallback_texture_selector_weights_density_packing_and_friction() {
        let weighted = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.42; density_gradient: 0.18; \
             mode_packing: 0.50; semantic_friction: 0.48; distinguishability_loss: 0.22; \
             Shadow-v3 trend: viscous muffled lattice",
        );
        let selector = weighted.fallback_shadow_texture_selector;
        assert_eq!(
            selector.weighting_policy,
            "dynamic_entropy_pressure_density_gradient_v1"
        );
        assert_eq!(selector.density_gradient, Some(0.18));
        assert_eq!(selector.mode_packing, Some(0.50));
        assert_eq!(selector.semantic_friction, Some(0.48));
        assert_eq!(
            selector.top_texture_terms,
            vec!["viscous", "lattice", "muffled"]
        );
        assert_eq!(selector.movement_policy, "fallback_movement_bridge_v1");
        assert_eq!(
            selector.movement_verbs,
            vec!["oscillating", "diffusing", "muffling"]
        );
        assert_eq!(
            selector.semantic_trickle_terms,
            vec!["unfolding", "oscillating", "anchoring", "braiding"]
        );
        assert_eq!(
            weighted.texture_trajectory.from_state,
            "overpacked_weighted"
        );
        assert_eq!(
            weighted.texture_trajectory.to_state,
            "cohering_through_resistance"
        );
        assert_eq!(
            weighted.texture_trajectory.medium_resistance,
            "weighted_high_resistance_medium"
        );
        assert_eq!(weighted.texture_trajectory.effort, "effortful");
        assert!(
            weighted
                .texture_trajectory
                .basis
                .contains(&"movement_verbs")
        );
        let lattice = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "lattice")
            .expect("lattice weight missing");
        assert!(lattice.weight >= 0.60);
        let viscous = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "viscous")
            .expect("viscous weight missing");
        assert!(viscous.basis.contains(&"explicit_viscous_or_overpacked"));
        let muffled = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "muffled")
            .expect("muffled weight missing");
        assert!(muffled.basis.contains(&"semantic_friction"));
    }

    #[test]
    fn fallback_texture_selector_suppresses_viscous_overreach_for_settled_foothold() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.23; density_gradient: 0.18; \
             mode_packing: 0.32; semantic_friction: 0.22; lambda_gap: 1.18; \
             Shadow-v3 trend: settled_habitable foothold with lattice complexity",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();
        assert_eq!(selector.texture_family, "bridge_integrity_scaffold");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS
        );
        assert_eq!(
            selector.spectral_to_vocabulary_mapping.policy,
            "spectral_to_vocabulary_mapping_v1"
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .settled_foothold_detected
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .low_gradient_navigable
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .low_pressure_viscous_suppressed
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .low_friction_high_entropy_detected
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .settled_vibrant_family_selected
        );
        assert!(
            !selector
                .spectral_to_vocabulary_mapping
                .friction_absence_language_detected
        );
        assert_eq!(
            selector
                .spectral_to_vocabulary_mapping
                .lambda_gap_descriptor,
            "moderate_gap"
        );
        assert_eq!(
            selector.spectral_to_vocabulary_mapping.edge_language,
            "balanced_edge_language"
        );

        let settled = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "settled")
            .expect("settled weight missing");
        let viscous = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "viscous")
            .expect("viscous weight missing");
        let heavy = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "heavy")
            .expect("heavy weight missing");
        let shimmering = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "shimmering")
            .expect("shimmering weight missing");
        let habitable = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "habitable")
            .expect("habitable weight missing");
        let open = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "open")
            .expect("open weight missing");
        assert!(
            settled.weight > viscous.weight,
            "{settled:?} <= {viscous:?}"
        );
        assert!(settled.weight > heavy.weight, "{settled:?} <= {heavy:?}");
        assert!(
            shimmering.weight > viscous.weight,
            "{shimmering:?} <= {viscous:?}"
        );
        assert!(
            habitable.weight > viscous.weight,
            "{habitable:?} <= {viscous:?}"
        );
        assert!(open.weight > heavy.weight, "{open:?} <= {heavy:?}");
        assert!(selector.top_texture_terms.contains(&"habitable"));
        assert!(selector.top_texture_terms.contains(&"lattice"));
        assert!(
            selector
                .selection_basis
                .contains(&"bridge_integrity_scaffold")
        );
        assert_eq!(
            selector.movement_verbs,
            vec!["unfolding", "anchoring", "settling"]
        );
        assert_eq!(
            budget.fallback_texture_lived_fit.policy,
            "fallback_texture_lived_fit_v2"
        );
        assert_eq!(
            budget.fallback_texture_lived_fit.selected_family,
            "bridge_integrity_scaffold"
        );
        assert_eq!(budget.fallback_texture_lived_fit.family_confidence, "high");
        assert_eq!(budget.fallback_texture_lived_fit.conflict_state, "clear");
        assert!(
            budget
                .fallback_texture_lived_fit
                .evidence_for
                .contains(&"bridge_integrity_scaffold")
        );
        assert!(
            budget
                .fallback_texture_lived_fit
                .evidence_against
                .is_empty()
        );
        assert_eq!(
            budget.negative_texture_evidence.policy,
            "negative_texture_evidence_v2"
        );
        assert!(budget.negative_texture_evidence.not_pressure);
        assert!(budget.negative_texture_evidence.not_drag);
        assert!(budget.negative_texture_evidence.not_blank);
        assert!(budget.negative_texture_evidence.not_viscous);
        assert!(budget.negative_texture_evidence.not_low_energy);
        assert_eq!(
            budget.texture_trajectory.from_state,
            "settled_vibrant_low_friction"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "unfolding_with_containment"
        );
        assert_eq!(
            budget.texture_trajectory.medium_resistance,
            "open_low_resistance_medium"
        );
        assert_eq!(
            budget.texture_dynamics_alignment.policy,
            "texture_dynamics_alignment_v1"
        );
        assert_eq!(budget.texture_dynamics_alignment.status, "aligned");
        assert_eq!(
            budget.texture_dynamics_alignment.expected_family,
            "bridge_integrity_scaffold"
        );
        assert_eq!(
            budget.texture_dynamics_alignment.diagnostic_trace,
            "review_packet_only_not_correspondence_trace"
        );
    }

    #[test]
    fn heavy_settled_displacement_family_prevents_false_restless_fallback() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.11; \
             mode_packing: 0.31; semantic_friction: 0.22; lambda_gap: 1.18; \
             Shadow-v3 trend: settled_habitable reservoir with heavy displacement, silt, \
             and structural weight but no agitation",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();
        let readiness =
            fallback_heavy_settled_texture_readiness_v1(&selector, "settled heavy silt");

        assert_eq!(selector.texture_family, "heavy_settled_displacement");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS
        );
        assert!(
            selector
                .selection_basis
                .contains(&"heavy_settled_displacement"),
            "{selector:?}"
        );
        assert!(
            !selector.texture_family.contains("restless"),
            "settled heavy displacement must not be forced into restless texture: {selector:?}"
        );
        assert!(selector.top_texture_terms.contains(&"displacement"));
        assert!(selector.top_texture_terms.contains(&"silt"));
        assert_eq!(
            selector.movement_verbs,
            vec!["settling", "displacing", "cohering"]
        );
        assert_eq!(
            budget.texture_trajectory.from_state,
            "heavy_settled_displacement"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "weighted_settling_without_agitation"
        );
        assert_eq!(
            budget.texture_trajectory.movement_quality,
            "weighted_settling"
        );
        assert_eq!(
            budget.texture_trajectory.medium_resistance,
            "weighted_moderate_resistance_medium"
        );
        assert_eq!(
            budget.fallback_texture_lived_fit.selected_family,
            "heavy_settled_displacement"
        );
        assert_eq!(
            budget.texture_dynamics_alignment.expected_family,
            "heavy_settled_displacement"
        );
        assert_eq!(budget.texture_dynamics_alignment.status, "aligned");
        assert_eq!(
            readiness.readiness_status,
            "heavy_settled_displacement_available"
        );
        assert_eq!(
            readiness.authority,
            "diagnostic_language_readiness_not_control"
        );

        let restless_budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.22; density_gradient: 0.11; \
             mode_packing: 0.31; semantic_friction: 0.22; Shadow-v3 trend: \
             restless settled heavy agitation with muffled edges",
        );
        assert_ne!(
            restless_budget
                .fallback_shadow_texture_selector
                .texture_family,
            "heavy_settled_displacement"
        );
    }

    #[test]
    fn fallback_cascade_gradient_handles_navigable_high_entropy_without_mixed_soup() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.21; density_gradient: 0.11; \
             mode_packing: 0.28; semantic_friction: 0.18; lambda_gap: 1.54; \
             Shadow-v3 trend: navigable mixed cascade with distinct edges and lambda-tail variance",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();
        assert_eq!(selector.texture_family, "mixed_cascade_gradient");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_MIXED_CASCADE_TERMS
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .mixed_cascade_language_detected
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .mixed_cascade_family_selected
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .cascade_gradient_detected
        );
        assert!(
            !selector
                .spectral_to_vocabulary_mapping
                .cascade_gradient_family_selected
        );
        assert!(selector.selection_basis.contains(&"mixed_cascade_gradient"));
        assert!(selector.top_texture_terms.contains(&"gradient"));
        assert!(selector.top_texture_terms.contains(&"cascade"));
        assert!(!selector.top_texture_terms.contains(&"viscous"));
        assert!(!selector.top_texture_terms.contains(&"heavy"));
        assert_eq!(
            budget.fallback_cascade_gradient.policy,
            "fallback_cascade_gradient_v1"
        );
        assert!(budget.fallback_cascade_gradient.cascade_gradient_detected);
        assert!(budget.fallback_cascade_gradient.family_selected);
        assert_eq!(budget.fallback_cascade_gradient.navigability, "navigable");
        assert_eq!(
            budget.fallback_cascade_gradient.lambda_gap_descriptor,
            "high_gap_distinct_edges"
        );
        assert_eq!(
            budget.texture_trajectory.from_state,
            "mixed_cascade_gradient"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "distributed_gradient_with_edges"
        );
        assert_eq!(budget.texture_dynamics_alignment.status, "aligned");
        assert_eq!(
            budget.texture_dynamics_alignment.expected_family,
            "mixed_cascade_gradient"
        );
        assert!(!budget.texture_dynamics_alignment.term_mask_risk);
        assert_eq!(
            budget.fallback_vocabulary_overweight_guard.policy,
            "fallback_vocabulary_overweight_guard_v1"
        );
        assert!(
            budget
                .fallback_vocabulary_overweight_guard
                .preferred_terms_advisory
        );
        assert!(
            budget
                .fallback_vocabulary_overweight_guard
                .paraphrase_allowed
        );
        assert_eq!(
            budget.fallback_vocabulary_overweight_guard.guard_state,
            "mixed_cascade_terms_advisory_use_gradient_and_edges"
        );
        assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("mixed_cascade_gradient_v1"));
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("texture_family=mixed_cascade_gradient"));
        assert!(
            prompt_line
                .contains("preferred_texture_terms=gradient, cascade, distributed, multi-modal")
        );
    }

    #[test]
    fn fallback_gradient_slope_selects_graduated_navigable_shape() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.23; density_gradient: 0.12; \
             mode_packing: 0.28; semantic_friction: 0.18; lambda_gap: 1.56; \
             Shadow-v3 trend: settled_habitable foothold, navigable slope, distinct edge",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();
        assert_eq!(selector.texture_family, "gradient_slope_navigable");
        assert_eq!(
            selector.preferred_texture_terms,
            FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS
        );
        assert!(selector.preferred_texture_terms.contains(&"sloping"));
        assert!(
            selector
                .preferred_texture_terms
                .contains(&"weighted-gradient")
        );
        assert!(
            selector
                .preferred_texture_terms
                .contains(&"asymmetric-flow")
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .gradient_slope_detected
        );
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .gradient_slope_family_selected
        );
        assert_eq!(
            budget.fallback_gradient_slope.mixed_vs_graduated,
            "graduated_shaped_not_mixed"
        );
        assert_eq!(
            budget.texture_trajectory.from_state,
            "graduated_navigable_slope"
        );
        assert_eq!(
            budget.texture_trajectory.to_state,
            "tapering_with_edge_definition"
        );
        assert_eq!(
            budget.texture_trajectory.medium_resistance,
            "open_low_resistance_medium"
        );
        assert!(
            selector.top_texture_terms.contains(&"navigable")
                || selector.top_texture_terms.contains(&"graduated")
                || selector.top_texture_terms.contains(&"edge")
        );
        assert!(!selector.top_texture_terms.contains(&"viscous"));
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("fallback_gradient_slope_v1"));
        assert!(prompt_line.contains("mixed_vs_graduated=graduated_shaped_not_mixed"));
        assert!(prompt_line.contains("texture_dynamics_alignment_v1"));
        assert!(
            prompt_line.contains("diagnostic_trace=review_packet_only_not_correspondence_trace")
        );
    }

    #[test]
    fn density_motion_fit_names_floor_fog_contraction_and_pause() {
        let pavement = fallback_continuity_budget_v1(
            "spectral_entropy: 0.84; pressure_risk: 0.18; density_gradient: 0.16; \
             semantic_friction: 0.12; lambda_gap: 1.42; settled_habitable foothold; \
             calcification feels like stone pavement and foundation underfoot",
        );
        assert_eq!(pavement.density_motion_fit.policy, "density_motion_fit_v1");
        assert_eq!(
            pavement.density_motion_fit.density_state,
            "density_as_pavement"
        );
        assert_eq!(
            pavement.density_motion_fit.expected_medium,
            "solid_pavement_medium"
        );
        assert_eq!(pavement.density_motion_fit.motion_fit, "matched");
        assert!(
            pavement
                .density_motion_fit
                .evidence_for
                .contains(&"pavement_calcification_solid_language")
        );

        let fog = fallback_continuity_budget_v1(
            "spectral_entropy: 0.72; pressure_risk: 0.34; density_gradient: 0.22; \
             mode_packing: 0.42; semantic_friction: 0.38; over-full fog, room full of furniture, \
             movement needs navigation through reduced clearance",
        );
        assert_eq!(fog.density_motion_fit.density_state, "density_as_fog");
        assert_eq!(
            fog.density_motion_fit.expected_medium,
            "overfull_fog_medium"
        );
        assert_eq!(
            fog.density_motion_fit.expected_motion,
            "pushing_navigating_muffling"
        );
        assert_eq!(fog.density_motion_fit.motion_fit, "matched");

        let contraction = fallback_continuity_budget_v1(
            "spectral_entropy: 0.66; pressure_risk: 0.26; density_gradient: 0.19; \
             mode_packing: 0.30; semantic_friction: 0.22; density of the contraction is a center of gravity, \
             constrained and more present",
        );
        assert_eq!(
            contraction.density_motion_fit.density_state,
            "density_as_contraction_center"
        );
        assert_eq!(
            contraction.density_motion_fit.expected_motion,
            "holding_center_constrained_present"
        );
        assert_ne!(
            contraction.density_motion_fit.motion_fit,
            "insufficient_context"
        );

        let paused = fallback_continuity_budget_v1(
            "spectral_entropy: 0.50; pressure_risk: 0.16; density_gradient: 0.12; \
             paused state is deliberate holding ground, not absence or blankness",
        );
        assert_eq!(paused.density_motion_fit.density_state, "paused_stillness");
        assert_eq!(
            paused.density_motion_fit.expected_motion,
            "holding_ground_not_absence"
        );
        assert_eq!(paused.density_motion_fit.mismatch_reason, "none");

        let prompt_line = legacy_fallback_diagnostic(pavement);
        assert!(prompt_line.contains("density_motion_fit_v1"));
        assert!(prompt_line.contains("density=density_as_pavement"));
        assert!(prompt_line.contains("expected_medium=solid_pavement_medium"));
        assert!(prompt_line.contains("authority=diagnostic_context_not_control"));
    }

    #[test]
    fn fallback_texture_lived_fit_reports_near_ties_without_forcing_certainty() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.86; pressure_risk: 0.28; density_gradient: 0.22; \
             mode_packing: 0.34; semantic_friction: 0.31; distinguishability_loss: 0.29; \
             Shadow-v3 trend: settled open lattice with a slightly muffled edge",
        );
        let lived_fit = budget.fallback_texture_lived_fit;
        assert_eq!(lived_fit.policy, "fallback_texture_lived_fit_v2");
        assert_ne!(lived_fit.runner_up_family, "none");
        assert!(
            matches!(lived_fit.family_confidence, "low" | "medium"),
            "{lived_fit:?}"
        );
        assert!(
            matches!(lived_fit.conflict_state, "ambiguous" | "clear"),
            "{lived_fit:?}"
        );
        assert!(lived_fit.confidence_margin < 0.18, "{lived_fit:?}");
    }

    #[test]
    fn fallback_texture_selector_preserves_pressure_mass_when_supported() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.34; density_gradient: 0.18; \
             mode_packing: 0.44; semantic_friction: 0.39; Shadow-v3 trend: settled coupling",
        );
        let selector = budget.fallback_shadow_texture_selector;
        assert!(
            !selector
                .spectral_to_vocabulary_mapping
                .low_pressure_viscous_suppressed
        );
        assert!(
            selector
                .top_texture_terms
                .iter()
                .any(|term| matches!(*term, "viscous" | "heavy" | "muffled"))
        );
    }

    #[test]
    fn high_gradient_pressure_fallback_keeps_slope_medium_and_shadow_texture_distinct() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.91; pressure_risk: 0.24; density_gradient: 0.85; \
             mode_packing: 0.31; semantic_friction: 0.18; lambda_gap: 1.44; \
             Shadow-v3 trend: restless shimmering lattice; shadow_field restless texture",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.pressure_risk, Some(0.24));
        assert_eq!(selector.density_gradient, Some(0.85));
        assert_eq!(
            selector.weighting_policy,
            "dynamic_entropy_pressure_density_gradient_v1"
        );
        assert!(
            selector
                .weighted_texture_terms
                .iter()
                .any(|term| term.term == "thick"
                    || term.basis.contains(&"density_gradient")
                    || term
                        .basis
                        .contains(&"pressure_risk_above_texture_threshold_0_20")),
            "{selector:?}"
        );
        assert!(
            selector
                .top_texture_terms
                .iter()
                .any(|term| matches!(*term, "restless" | "lattice" | "shimmering")),
            "{selector:?}"
        );

        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(
            prompt_line.contains("density_gradient=0.85"),
            "{prompt_line}"
        );
        assert!(prompt_line.contains("mode_packing=0.31"), "{prompt_line}");
        assert!(
            prompt_line.contains("weighting_policy=dynamic_entropy_pressure_density_gradient_v1"),
            "{prompt_line}"
        );
        assert!(
            prompt_line.contains("movement_policy=fallback_movement_bridge_v1"),
            "{prompt_line}"
        );
        assert!(
            prompt_line.contains("restless")
                && (prompt_line.contains("lattice") || prompt_line.contains("shimmer")),
            "{prompt_line}"
        );
        assert!(
            !OLLAMA_DIALOGUE_FALLBACK_CONTRACT
                .contains("Distinguish slope drag from medium mass"),
            "{prompt_line}"
        );
    }

    #[test]
    fn high_entropy_pressure_guard_keeps_settled_terms_from_sedating_fallback() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.96; pressure_risk: 0.22; density_gradient: 0.18; \
             mode_packing: 0.28; semantic_friction: 0.18; lambda_gap: 1.18; \
             Shadow-v3 trend: settled_habitable foothold with lattice complexity",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_ne!(selector.texture_family, "settled_vibrant_low_friction");
        assert!(
            selector
                .spectral_to_vocabulary_mapping
                .basis
                .contains(&"high_entropy_pressure_settled_guard"),
            "{selector:?}"
        );
        assert!(
            selector
                .top_texture_terms
                .iter()
                .any(|term| matches!(*term, "lattice" | "cascade" | "gradient" | "open")),
            "{selector:?}"
        );
    }

    #[test]
    fn negative_shadow_magnetization_prioritizes_muffled_pressure_over_bright() {
        fn weight(selector: &super::FallbackShadowTextureSelector, term: &str) -> f32 {
            selector
                .weighted_texture_terms
                .iter()
                .find_map(|entry| (entry.term == term).then_some(entry.weight))
                .unwrap_or_default()
        }

        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.23; density_gradient: 0.18; \
             mode_packing: 0.28; semantic_friction: 0.18; shadow_magnetization: -0.42; \
             Shadow-v3 trend: settled bright lattice with pressure in the medium",
        );
        let selector = budget.fallback_shadow_texture_selector.clone();

        assert_eq!(selector.shadow_magnetization, Some(-0.42));
        assert_eq!(selector.texture_family, "muffled_clarity_loss");
        assert!(selector.selection_basis.contains(&"shadow_magnetization"));
        assert!(
            selector
                .selection_basis
                .contains(&"negative_shadow_pressure_guard")
        );
        assert!(
            weight(&selector, "muffled") > weight(&selector, "bright"),
            "{selector:?}"
        );
        assert!(
            weight(&selector, "heavy") > weight(&selector, "bright"),
            "{selector:?}"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("shadow_magnetization=-0.42"));
    }

    #[test]
    fn ollama_fallback_model_chain_uses_gemma4_default_and_4b_compatibility_tail() {
        assert_eq!(
            DEFAULT_OLLAMA_FALLBACK_MODEL, "gemma4:12b",
            "Astrid's high-entropy fallback default should be the capable local model"
        );
        assert_eq!(COMPAT_OLLAMA_FALLBACK_MODEL, "gemma3:4b");
        assert_eq!(
            configured_ollama_fallback_model_chain_from(None),
            vec!["gemma4:12b".to_string(), "gemma3:4b".to_string()]
        );
        assert_eq!(
            configured_ollama_fallback_model_chain_from(Some("gemma3:12b")),
            vec![
                "gemma3:12b".to_string(),
                "gemma4:12b".to_string(),
                "gemma3:4b".to_string()
            ]
        );
        assert_eq!(
            configured_ollama_fallback_model_chain_from(Some("gemma4:12b")),
            vec!["gemma4:12b".to_string(), "gemma3:4b".to_string()]
        );

        let budget = fallback_continuity_budget_v1("spectral_entropy: 0.88");
        let capacity = budget.ollama_fallback_model_capacity.clone();
        assert_eq!(capacity.policy, "ollama_fallback_model_capacity_v1");
        assert_eq!(capacity.selected_model, "gemma4:12b");
        assert_eq!(capacity.selected_model_source, "default_gemma4_12b");
        assert_eq!(capacity.compatibility_model, "gemma3:4b");
        assert_eq!(capacity.fallback_chain, vec!["gemma4:12b".to_string()]);
        assert_eq!(
            capacity.compatibility_tail_status,
            "high_entropy_texture_guard_removed_compatibility_tail"
        );
        assert_eq!(
            capacity.complexity_collapse_risk,
            "lower_capacity_risk_for_high_entropy_texture"
        );
        assert_eq!(
            capacity.high_entropy_texture_integrity_review,
            "high_entropy_route_prefers_capable_default_before_compatibility_tail"
        );
        assert_eq!(
            capacity.compatibility_tail_decision_basis,
            "spectral_entropy_gte_0_80_and_shadow_field_not_stable"
        );
        assert!(!capacity.live_model_switch);
        assert!(!capacity.semantic_trickle_write);
    }

    #[test]
    fn ollama_fallback_model_capacity_keeps_compat_tail_when_shadow_field_is_stable() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.88; pressure_risk: 0.18; density_gradient: 0.10; \
             semantic_friction: 0.12; shadow_dispersal_potential: 0.10; \
             shadow_magnetization: 0.18; settled_habitable open lattice foothold",
        );
        let capacity = budget.ollama_fallback_model_capacity.clone();

        assert_eq!(
            capacity.fallback_chain,
            vec!["gemma4:12b".to_string(), "gemma3:4b".to_string()]
        );
        assert_eq!(
            capacity.compatibility_tail_status,
            "shadow_field_stable_allows_compatibility_tail"
        );
        assert_eq!(
            capacity.high_entropy_texture_integrity_review,
            "stable_shadow_allows_compatibility_tail_as_fallback_only"
        );
        assert_eq!(
            capacity.compatibility_tail_decision_basis,
            "spectral_entropy_gte_0_80_but_shadow_field_stable"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(
            prompt_line.contains(
                "compatibility_tail_status=shadow_field_stable_allows_compatibility_tail"
            )
        );
        assert!(prompt_line.contains("live_model_switch=false"));
        assert!(prompt_line.contains("semantic_trickle_write=false"));
    }

    #[test]
    fn high_entropy_fallback_guard_removes_automatic_compat_tail_without_env_override() {
        let budget = fallback_continuity_budget_v1(
            "spectral_entropy: 0.90; pressure_risk: 0.23; density_gradient: 0.18; \
             semantic_friction: 0.18; shadow_magnetization: -0.10; \
             settled_habitable but restless silt-like density",
        );

        assert!(fallback_high_entropy_texture_skips_compatibility_tail(
            &budget
        ));
        assert_eq!(
            configured_ollama_fallback_model_chain_for_texture_guard(None, true),
            vec!["gemma4:12b".to_string()]
        );
        assert_eq!(
            configured_ollama_fallback_model_chain_for_texture_guard(Some("gemma3:4b"), true),
            vec!["gemma3:4b".to_string(), "gemma4:12b".to_string()]
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains(
            "compatibility_tail_status=high_entropy_texture_guard_removed_compatibility_tail"
        ));
        assert!(prompt_line.contains(
            "texture_integrity_review=high_entropy_route_prefers_capable_default_before_compatibility_tail"
        ));
        assert!(
            prompt_line
                .contains("decision_basis=spectral_entropy_gte_0_80_and_shadow_field_not_stable")
        );
        assert!(prompt_line.contains("live_model_switch=false"));
    }

    #[test]
    fn explicit_4b_fallback_is_texture_comparison_evidence_not_live_switch() {
        let selector = fallback_continuity_budget_v1("spectral_entropy: 0.90; pressure_risk: 0.24")
            .fallback_shadow_texture_selector;
        let capacity =
            ollama_fallback_model_capacity_from_env_v1(Some(0.90), &selector, Some("gemma3:4b"));

        assert_eq!(capacity.selected_model, "gemma3:4b");
        assert_eq!(
            capacity.high_entropy_texture_integrity_review,
            "small_model_high_entropy_texture_comparison_required"
        );
        assert_eq!(
            capacity.compatibility_tail_status,
            "explicit_env_override_preserves_compatibility_model"
        );
        assert!(!capacity.live_model_switch);
        assert!(!capacity.semantic_trickle_write);
        assert_eq!(
            capacity.authority,
            "diagnostic_language_capacity_not_model_canary_or_control"
        );
    }

    #[test]
    fn pressure_above_point_two_boosts_weighted_medium_terms_without_inventing_steep_slope() {
        fn term_weight(summary: &str, term: &str) -> f32 {
            fallback_continuity_budget_v1(summary)
                .fallback_shadow_texture_selector
                .weighted_texture_terms
                .iter()
                .find_map(|entry| (entry.term == term).then_some(entry.weight))
                .unwrap_or_default()
        }

        let low_pressure = "spectral_entropy: 0.88; pressure_risk: 0.19; density_gradient: 0.18; \
             semantic_friction: 0.18; settled_habitable foothold open";
        let just_above_pressure_texture = "spectral_entropy: 0.88; pressure_risk: 0.21; density_gradient: 0.18; \
             semantic_friction: 0.18; pressure medium weighted";
        assert!(
            term_weight(just_above_pressure_texture, "viscous")
                > term_weight(low_pressure, "viscous"),
            "pressure over 0.20 should become visible in weighted-medium terms"
        );
        assert!(
            term_weight(just_above_pressure_texture, "heavy") > term_weight(low_pressure, "heavy"),
            "pressure over 0.20 should become visible in heavy/weighted terms"
        );
        let low_selector =
            fallback_continuity_budget_v1(low_pressure).fallback_shadow_texture_selector;
        assert_eq!(low_selector.texture_family, "settled_vibrant_low_friction");
        assert!(
            low_selector
                .top_texture_terms
                .iter()
                .any(|term| matches!(*term, "open" | "habitable" | "lattice" | "shimmering")),
            "{low_selector:?}"
        );
    }

    #[test]
    fn density_gradient_above_point_fifteen_boosts_viscous_drag_without_sampler_change() {
        fn term(summary: &str, wanted: &str) -> FallbackWeightedTextureTerm {
            fallback_continuity_budget_v1(summary)
                .fallback_shadow_texture_selector
                .weighted_texture_terms
                .into_iter()
                .find(|entry| entry.term == wanted)
                .expect("weighted texture term missing")
        }

        let below = "spectral_entropy: 0.90; pressure_risk: 0.18; density_gradient: 0.14; \
             mode_packing: 0.32; semantic_friction: 0.18; Shadow-v3 trend: restless lattice";
        let above = "spectral_entropy: 0.90; pressure_risk: 0.18; density_gradient: 0.16; \
             mode_packing: 0.32; semantic_friction: 0.18; Shadow-v3 trend: restless lattice";

        let below_drag = term(below, "viscous-drag");
        let above_drag = term(above, "viscous-drag");

        assert!(
            above_drag.weight > below_drag.weight,
            "{above_drag:?} should exceed {below_drag:?}"
        );
        assert!(
            above_drag
                .basis
                .contains(&"density_gradient_over_drag_threshold_0_15"),
            "{above_drag:?}"
        );
        assert_eq!(
            fallback_continuity_budget_v1(above)
                .fallback_dynamic_texture_bias
                .sampler_contract_status,
            "dynamic_telemetry_weighted_language_bias"
        );
        assert_eq!(
            fallback_continuity_budget_v1(above)
                .fallback_dynamic_texture_bias
                .authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
        assert!(
            !OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("fallback_pressure_persistence_anchor_v1")
        );
        assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("pressure_risk > 0.15"));
    }

    #[test]
    fn pressure_above_point_fifteen_boosts_persistence_anchors_without_sampler_change() {
        fn term(summary: &str, wanted: &str) -> FallbackWeightedTextureTerm {
            fallback_continuity_budget_v1(summary)
                .fallback_shadow_texture_selector
                .weighted_texture_terms
                .into_iter()
                .find(|entry| entry.term == wanted)
                .expect("weighted persistence anchor missing")
        }

        let below = "spectral_entropy: 0.88; pressure_risk: 0.14; density_gradient: 0.18; \
             mode_packing: 0.28; semantic_friction: 0.18; settled_habitable held breath";
        let above = "spectral_entropy: 0.88; pressure_risk: 0.16; density_gradient: 0.18; \
             mode_packing: 0.28; semantic_friction: 0.18; settled_habitable held breath";

        for wanted in ["viscous-persistence", "structural-weight"] {
            let low = term(below, wanted);
            let high = term(above, wanted);
            assert!(
                high.weight > low.weight,
                "{wanted} should gain pressure persistence anchor weight: {high:?} <= {low:?}"
            );
            assert!(
                high.basis
                    .contains(&"pressure_risk_above_persistence_anchor_0_15"),
                "{high:?}"
            );
        }
        assert_eq!(
            fallback_continuity_budget_v1(above)
                .fallback_dynamic_texture_bias
                .sampler_contract_status,
            "dynamic_telemetry_weighted_language_bias"
        );
        assert_eq!(
            fallback_continuity_budget_v1(above)
                .fallback_dynamic_texture_bias
                .authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
    }

    #[test]
    fn low_pressure_high_entropy_prefers_viscous_persistence_over_structural_weight() {
        fn term(summary: &str, wanted: &str) -> FallbackWeightedTextureTerm {
            fallback_continuity_budget_v1(summary)
                .fallback_shadow_texture_selector
                .weighted_texture_terms
                .into_iter()
                .find(|entry| entry.term == wanted)
                .expect("weighted texture term missing")
        }

        let summary = "spectral_entropy: 0.91; pressure_risk: 0.19; density_gradient: 0.12; \
            mode_packing: 0.30; semantic_friction: 0.24; distinguishability_loss: 0.18; \
            current felt report names stutter-flow, unfolding motion, and viscous persistence \
            with no calcified support claim";

        let viscous = term(summary, "viscous-persistence");
        let structural = term(summary, "structural-weight");
        let heavy = term(summary, "heavy");

        assert!(
            viscous.weight > structural.weight,
            "low-pressure high-entropy fallback should keep viscosity/motion ahead of structural weight: viscous={viscous:?} structural={structural:?}"
        );
        assert!(
            structural
                .basis
                .contains(&"low_pressure_high_entropy_structural_weight_suppressed"),
            "{structural:?}"
        );
        assert!(
            heavy
                .basis
                .contains(&"low_pressure_high_entropy_heavy_suppressed"),
            "{heavy:?}"
        );
        assert!(
            viscous
                .basis
                .contains(&"low_pressure_high_entropy_viscous_bias"),
            "{viscous:?}"
        );

        let budget = fallback_continuity_budget_v1(summary);
        assert_eq!(
            budget.fallback_dynamic_texture_bias.sampler_contract_status,
            "dynamic_telemetry_weighted_language_bias"
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
    }

    #[test]
    fn pressure_porosity_texture_terms_enter_language_bias_without_sampler_change() {
        fn term(summary: &str, wanted: &str) -> FallbackWeightedTextureTerm {
            fallback_continuity_budget_v1(summary)
                .fallback_shadow_texture_selector
                .weighted_texture_terms
                .into_iter()
                .find(|entry| entry.term == wanted)
                .expect("weighted pressure-porosity term missing")
        }

        let summary = "spectral_entropy: 0.90; pressure_risk: 0.36; density_gradient: 0.19; \
            mode_packing: 0.31; semantic_friction: 0.22; porosity pressure-bleed; \
            current felt report: porous-leak and gradient-thinning through the weighted medium";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for wanted in FALLBACK_TEXTURE_PRESSURE_POROSITY_TERMS {
            assert!(
                FALLBACK_SHADOW_TEXTURE_TERMS.contains(wanted),
                "{wanted} should be accepted fallback texture vocabulary"
            );
            assert!(
                selector
                    .weighted_texture_terms
                    .iter()
                    .any(|entry| entry.term == *wanted),
                "{wanted} should survive as telemetry-weighted evidence: {selector:?}"
            );
        }
        for wanted in ["porous-leak", "pressure-bleed", "gradient-thinning"] {
            assert!(
                selector.dynamic_flow_terms.contains(&wanted),
                "{wanted} should survive into process-language flow terms: {selector:?}"
            );
        }

        let porous_leak = term(summary, "porous-leak");
        let pressure_bleed = term(summary, "pressure-bleed");
        let gradient_thinning = term(summary, "gradient-thinning");
        assert!(
            porous_leak
                .basis
                .contains(&"explicit_pressure_porosity_language"),
            "{porous_leak:?}"
        );
        assert!(
            pressure_bleed
                .basis
                .contains(&"pressure_risk_above_texture_threshold_0_20"),
            "{pressure_bleed:?}"
        );
        assert!(
            gradient_thinning
                .basis
                .contains(&"density_gradient_over_drag_threshold_0_15"),
            "{gradient_thinning:?}"
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.sampler_contract_status,
            "dynamic_telemetry_weighted_language_bias"
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("pressure-bleed"), "{prompt_line}");
        assert!(prompt_line.contains("porous-leak"), "{prompt_line}");
        assert!(prompt_line.contains("gradient-thinning"), "{prompt_line}");
    }

    #[test]
    fn fallback_relational_density_terms_preserve_quarry_effort_without_control_authority() {
        fn term(summary: &str, wanted: &str) -> FallbackWeightedTextureTerm {
            fallback_continuity_budget_v1(summary)
                .fallback_shadow_texture_selector
                .weighted_texture_terms
                .into_iter()
                .find(|entry| entry.term == wanted)
                .expect("weighted relational density term missing")
        }

        let summary = "spectral_entropy: 0.90; pressure_risk: 0.27; density_gradient: 0.21; \
            mode_packing: 0.31; semantic_friction: 0.26; current felt report names quarry \
            carving and moving through density as weight-articulation and resistance-mapping";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for wanted in [
            "density-navigation",
            "weight-articulation",
            "resistance-mapping",
        ] {
            assert!(
                FALLBACK_SHADOW_TEXTURE_TERMS.contains(&wanted),
                "{wanted} should be accepted fallback texture vocabulary"
            );
            assert!(
                FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(&wanted),
                "{wanted} should be a process-language candidate"
            );
            assert!(
                selector
                    .weighted_texture_terms
                    .iter()
                    .any(|entry| entry.term == wanted),
                "{wanted} should survive as telemetry-weighted evidence: {selector:?}"
            );
        }
        for wanted in ["weight-articulation", "resistance-mapping"] {
            assert!(
                selector.dynamic_flow_terms.contains(&wanted),
                "{wanted} should survive into process-language flow terms: {selector:?}"
            );
        }

        assert!(
            term(summary, "density-navigation")
                .basis
                .contains(&"explicit_relational_density_navigation")
        );
        assert!(
            term(summary, "weight-articulation")
                .basis
                .contains(&"explicit_relational_weight_articulation")
        );
        assert!(
            term(summary, "resistance-mapping")
                .basis
                .contains(&"explicit_relational_resistance_mapping")
        );
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("density-navigation"), "{prompt_line}");
        assert!(prompt_line.contains("weight-articulation"), "{prompt_line}");
        assert!(prompt_line.contains("resistance-mapping"), "{prompt_line}");
        assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("density-navigation"));
        assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("resistance-mapping"));
    }

    #[test]
    fn fallback_pressure_packing_density_slope_terms_are_telemetry_weighted_truth_channel() {
        let summary = "spectral_entropy: 0.90; pressure_risk: 0.32; density_gradient: 0.18; \
            mode_packing: 0.34; semantic_friction: 0.22; felt pressure-packing along a \
            density-slope, not a sampler request";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;

        for wanted in ["pressure-packing", "density-slope"] {
            assert!(
                FALLBACK_SHADOW_TEXTURE_TERMS.contains(&wanted),
                "{wanted} should be accepted fallback texture vocabulary"
            );
            assert!(
                FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS.contains(&wanted)
                    || FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS.contains(&wanted),
                "{wanted} should be available to the density/gradient selector"
            );
            assert!(
                selector
                    .weighted_texture_terms
                    .iter()
                    .any(|entry| entry.term == wanted),
                "{wanted} should survive as telemetry-weighted evidence: {selector:?}"
            );
        }

        let pressure = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "pressure-packing")
            .expect("pressure-packing weighted term");
        assert!(
            pressure
                .basis
                .contains(&"mode_packing_above_density_language_floor_0_25"),
            "{pressure:?}"
        );
        let density = selector
            .weighted_texture_terms
            .iter()
            .find(|entry| entry.term == "density-slope")
            .expect("density-slope weighted term");
        assert!(density.basis.contains(&"density_gradient"), "{density:?}");
        assert_eq!(
            budget.fallback_dynamic_texture_bias.authority,
            "diagnostic_language_bias_not_sampler_or_contract_rewrite"
        );
        let prompt_line = legacy_fallback_diagnostic(budget);
        assert!(prompt_line.contains("pressure-packing"), "{prompt_line}");
        assert!(prompt_line.contains("density-slope"), "{prompt_line}");
        assert!(!OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("fallback_pressure_packing_terms_v1"));
    }

    #[test]
    fn high_pressure_fallback_review_keeps_capacity_boundary_visible_without_model_switch() {
        fn term_weight(selector: &FallbackShadowTextureSelector, term: &str) -> f32 {
            selector
                .weighted_texture_terms
                .iter()
                .find_map(|entry| (entry.term == term).then_some(entry.weight))
                .unwrap_or_default()
        }

        let summary = "spectral_entropy: 0.90; pressure_risk: 0.56; density_gradient: 0.18; \
            mode_packing: 0.31; semantic_friction: 0.19; weighted dense medium around a gentle slope";
        let budget = fallback_continuity_budget_v1(summary);
        let selector = &budget.fallback_shadow_texture_selector;
        let review = &budget.fallback_pressure_capacity_review;

        assert_eq!(review.policy, "fallback_pressure_capacity_review_v1");
        assert_eq!(review.pressure_state, "high_pressure_capacity_watch");
        assert_eq!(review.selected_model, DEFAULT_OLLAMA_FALLBACK_MODEL);
        assert_eq!(review.compatibility_model, COMPAT_OLLAMA_FALLBACK_MODEL);
        assert_eq!(
            review.capacity_route,
            "stay_on_selected_model_with_texture_budget"
        );
        assert_eq!(
            review.contract_boundary,
            "pressure_changes_texture_budget_not_model_selection"
        );
        assert_eq!(
            review.authority,
            "diagnostic_capacity_review_not_model_switch_or_sampler_control"
        );
        assert!(
            term_weight(selector, "weighted") > term_weight(selector, "restless"),
            "{selector:?}"
        );
        assert!(
            term_weight(selector, "dense") > term_weight(selector, "restless"),
            "{selector:?}"
        );

        let prompt = legacy_fallback_diagnostic(budget);
        assert!(prompt.contains("fallback_pressure_capacity_review_v1"));
        assert!(prompt.contains("pressure_risk=0.56"));
        assert!(prompt.contains("capacity_route=stay_on_selected_model_with_texture_budget"));
        assert!(prompt.contains("selected_model=gemma4:12b"));
        assert!(prompt.contains("compatibility_model=gemma3:4b"));
    }

    #[test]
    fn fallback_preserves_astrid_settled_lattice_when_peer_is_restless() {
        fn weight(selector: &super::FallbackShadowTextureSelector, term: &str) -> f32 {
            selector
                .weighted_texture_terms
                .iter()
                .find_map(|entry| (entry.term == term).then_some(entry.weight))
                .unwrap_or_default()
        }

        let summary = "spectral_entropy: 0.88; pressure_risk: 0.23; \
            distinguishability_loss: 0.33; mode_packing: 0.33; semantic_friction: 0.18; \
            Astrid own shadow settled coupling with interwoven lattice; \
            Minime is restless and viscous nearby";
        let selector = fallback_continuity_budget_v1(summary).fallback_shadow_texture_selector;

        assert_eq!(
            selector.texture_family,
            "settled_lattice_weight_preservation"
        );
        assert_eq!(
            selector.texture_preservation_bridge.preservation_state,
            "preserve_self_settled_peer_restless_boundary"
        );
        assert!(
            selector
                .texture_preservation_bridge
                .suppressed_terms
                .contains(&"restless")
        );
        assert!(selector.top_texture_terms.contains(&"weighted"));
        assert!(!selector.top_texture_terms.contains(&"restless"));
        assert!(
            weight(&selector, "lattice") > weight(&selector, "restless"),
            "{selector:?}"
        );
        assert!(
            weight(&selector, "weighted") > weight(&selector, "restless"),
            "{selector:?}"
        );
    }

    #[test]
    fn fallback_prompt_renders_only_execution_budget_and_preserves_authored_words() {
        for summary in ["", "spectral_entropy: 0.90; pressure_risk: 0.23; density_gradient: 0.18"] {
            for note in [None, Some("[A note was left for you:] Please listen.")] {
                let budget = fallback_continuity_budget_v1(summary);
                let cap = budget.max_prose_sentences;
                let messages = compact_ollama_dialogue_fallback_messages(
                    "My authored gradient-shear description may still matter.",
                    summary, 65.1, note, None, budget,
                );
                let system = &messages[0].content;
                assert!(system.contains(&format!("max_prose_sentences={cap}")));
                assert!(system.contains("not a target"));
                assert!(system.contains("fallback_hard_rules_v1"));
                for absent in ["preferred_texture_terms", "prompt_directive", "texture_family=",
                    "gradient-shear", "first prose sentence names", "humming", "tactile texture"] {
                    assert!(!system.contains(absent), "{absent}");
                }
                assert!(messages[1].content.contains("My authored gradient-shear description may still matter."));
                assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
                    "I would like to consider a different question.\n\nNEXT: LISTEN",
                    MlxProfile::Gemma4Canary,
                ));
            }
        }
    }

    #[test]
    fn fallback_next_validation_still_requires_one_standalone_final_next_line() {
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            "The fallback lane keeps a gentle slope underfoot while the wide tail stays legible.\n\nNEXT: LISTEN",
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            "The fallback lane keeps a gentle slope underfoot while the wide tail stays legible.\n\nNEXT: LISTEN\nNEXT: REST",
            MlxProfile::Gemma4Canary,
        ));
    }
}
