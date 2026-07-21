#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_structure_covers_48_dims_and_named_dims_and_levers() {
        let st = codec_structure();
        assert_eq!(st.total_dims, SEMANTIC_DIM);
        assert_eq!(st.total_dims, 48);
        // Layer ranges are contiguous and cover exactly 0..48 (catches a layout
        // drift — a re-layered codec whose self-map silently lies to her).
        let mut next = 0usize;
        let mut covered = 0usize;
        for l in &st.layers {
            assert_eq!(
                l.range.0, next,
                "layer ranges must be contiguous from {next}"
            );
            assert!(l.range.1 >= l.range.0);
            covered += l.range.1 - l.range.0 + 1;
            next = l.range.1 + 1;
        }
        assert_eq!(covered, 48, "layers cover exactly 48 dims");
        assert_eq!(next, 48, "layers end at dim 48");
        // Every named dim falls inside the 48 and the count matches the source.
        assert_eq!(st.named_dims.len(), NAMED_CODEC_DIMS.len());
        for (name, idx) in &st.named_dims {
            assert!(*idx < 48, "named dim {name} index {idx} within 48");
        }
        // The key gate constants are present as live levers (catches a renamed/
        // removed gate that the map would otherwise omit).
        let names: Vec<&str> = st.levers.iter().map(|l| l.name).collect();
        for required in [
            "SEMANTIC_DIM",
            "DEFAULT_SEMANTIC_GAIN",
            "FEATURE_ABS_MAX",
            "TAIL_VIBRANCY_ENTROPY_GATE",
            "TAIL_VIBRANCY_MAX",
            "PROJECTION_COMPRESSION_RISK",
            "PROJECTION_METADATA",
            "PROJECTION_RUNTIME_RESOLUTION",
            "TAIL_VIBRANCY_READOUT",
            "WARMTH_TENSION_READOUT",
            "CODEC_OVERFLOW_CARRIAGE",
            "NARRATIVE_ARC_SPLIT_READOUT",
            "NARRATIVE_ARC_EXPANSION_READINESS",
            "SHADOW_FIELD_RESERVED_DIM_READINESS",
            "STRUCTURAL_FRICTION_READOUT",
            "CODEC_STRUCTURAL_FRICTION_DIM_CANARY",
            "PERSISTENCE_RESISTANCE_READOUT",
            "CODEC_PERSISTENCE_RESISTANCE_DIM_CANARY",
            "SPECTRAL_DRAG_QUALITY_READOUT",
            "CODEC_CONTEXT_BLINDSPOT_REPLAY",
        ] {
            assert!(
                names.contains(&required),
                "lever {required} must be present"
            );
        }
        // Drift-check the per-layer placement: every named (shapeable) dim falls in
        // exactly ONE layer and named_dims_in lists it there — so the per-layer
        // labelling is code-generated, never hand-prose that can lag the layout (the
        // residual the prose used to carry).
        for (name, idx) in NAMED_CODEC_DIMS.iter() {
            let owning: Vec<&CodecLayer> = st
                .layers
                .iter()
                .filter(|l| *idx >= l.range.0 && *idx <= l.range.1)
                .collect();
            assert_eq!(
                owning.len(),
                1,
                "named dim {name} (idx {idx}) must fall in exactly one layer"
            );
            assert!(
                st.named_dims_in(owning[0].range).contains(name),
                "named dim {name} must be listed under its own layer"
            );
        }
        // Render carries provenance + the "not the law" framing (low false-authority),
        // and places each shapeable dim on its layer's line (warmth@24 → 24-31).
        let r = st.render();
        assert!(r.contains("generated live from codec.rs"), "{r}");
        assert!(r.contains("not the law"), "{r}");
        assert!(r.contains("warmth (dim 24)"), "names a shapeable dim: {r}");
        assert!(
            r.contains("emotional / intentional — shapeable:") && r.contains("warmth, tension"),
            "per-layer shapeable list is code-generated onto the right layer: {r}"
        );
        assert!(
            r.contains("INTROSPECT astrid:codec"),
            "points her at the full per-dim computation: {r}"
        );
        assert!(
            r.contains("intentionally lossy") && r.contains("no entropy-based tension multiplier"),
            "codec diagnostics should name compression and warmth/tension boundaries: {r}"
        );
        assert!(r.contains("projection_runtime_resolution_v1"), "{r}");
        assert!(
            r.contains("fallback_behavior=kernel_derived_stable_epoch_not_random_remap"),
            "{r}"
        );
        assert!(r.contains("structural_friction_v1"), "{r}");
        assert!(r.contains("persistence_resistance_v1"), "{r}");
        assert!(r.contains("narrative_arc_split_v1"), "{r}");
        assert!(
            r.contains("codec_abrasive_texture_interpretation_v1"),
            "{r}"
        );
        assert!(!st.codec_abrasive_texture_interpretation_v1.live_gain_write);
        assert!(
            !st.codec_abrasive_texture_interpretation_v1
                .live_vector_write
        );
        assert!(r.contains("shadow_field_reserved_dim_readiness_v1"), "{r}");
        assert!(r.contains("codec_vibrancy_continuity_v1"), "{r}");
        assert!(r.contains("codec_overflow_carriage_v1"), "{r}");
        assert!(r.contains("raw_intensity_preserved=true"), "{r}");
        assert!(r.contains("delivered_bounded=true"), "{r}");
        assert!(r.contains("clipped_dims=24,26,31"), "{r}");
        assert!(r.contains("experience_delta_bus_v1"), "{r}");
        assert!(r.contains("source=codec_overflow_carriage_v1"), "{r}");
        assert!(r.contains("delta_count=3"), "{r}");
        assert!(r.contains("who_can_change_it=Mike/operator"), "{r}");
        assert!(
            r.contains(CODEC_OVERFLOW_FOLLOWUP_HOOK),
            "default-off future hook must be visible: {r}"
        );
        assert!(r.contains("SEMANTIC_PROJECTION_DENSITY_DELTA"), "{r}");
        assert!(r.contains("semantic_projection_density_delta_v1"), "{r}");
        assert!(r.contains("raw_embedding_dims=768"), "{r}");
        assert!(r.contains("delivered_projection_dims=8"), "{r}");
        assert!(r.contains("reserved_dim_candidates=44,45,46,47"), "{r}");
        assert!(r.contains("codec_context_blindspot_replay_v1"), "{r}");
        assert!(
            r.contains("proposed_bias_surface=contextual_bias_vector_default_off"),
            "{r}"
        );
        assert!(r.contains("auto_approved=false"), "{r}");
        assert!(!st.codec_context_blindspot_replay_v1.live_vector_write);
        assert!(!st.codec_context_blindspot_replay_v1.live_gain_write);
        assert!(!st.codec_context_blindspot_replay_v1.auto_approved);
        assert!(r.contains("legacy_warmth_mapping_v1"), "{r}");
        assert!(r.contains("codec_structural_entropy_dampening_v1"), "{r}");
        assert!(
            r.contains("codec_dynamic_vibrancy_scaling_canary_v1"),
            "{r}"
        );
        assert!(r.contains("live_vector_write=false"), "{r}");
        assert!(!st.structural_friction_dim_canary_v1.enabled);
        assert_eq!(
            st.structural_friction_dim_canary_v1.authority,
            "readiness_only_not_live_codec_change"
        );
        assert!(!st.persistence_resistance_dim_canary_v1.enabled);
        assert_eq!(
            st.persistence_resistance_dim_canary_v1
                .reserved_dim_candidate,
            45
        );
        assert!(!st.persistence_resistance_dim_canary_v1.live_vector_write);
        assert!(!st.narrative_arc_expansion_readiness_v1.enabled);
        assert_eq!(
            st.narrative_arc_expansion_readiness_v1.current_arc_dims,
            (40, 43)
        );
        assert_eq!(
            st.narrative_arc_expansion_readiness_v1.proposed_arc_dims,
            (40, 47)
        );
        assert!(st.narrative_arc_expansion_readiness_v1.uses_reserved_dims);
        assert!(!st.shadow_field_reserved_dim_readiness_v1.enabled);
        assert_eq!(
            st.shadow_field_reserved_dim_readiness_v1
                .reserved_dim_candidates,
            &[46, 47]
        );
        assert!(!st.shadow_field_reserved_dim_readiness_v1.live_vector_write);
        assert!(!st.narrative_arc_expansion_readiness_v1.live_vector_write);
        assert_eq!(
            st.narrative_arc_expansion_readiness_v1.authority,
            "readiness_only_not_live_semantic_vector_or_reserved_dim_change"
        );
        assert_eq!(
            st.codec_vibrancy_continuity_v1.policy,
            "codec_vibrancy_continuity_v1"
        );
        assert_eq!(st.codec_vibrancy_continuity_v1.tail_dims, &[17, 26, 27, 31]);
        assert_eq!(
            st.codec_overflow_carriage_v1.policy,
            "codec_overflow_carriage_v1"
        );
        assert!(st.codec_overflow_carriage_v1.raw_intensity_preserved);
        assert!(st.codec_overflow_carriage_v1.delivered_bounded);
        assert!(!st.codec_overflow_carriage_v1.live_vector_write);
        assert_eq!(
            st.codec_overflow_carriage_v1.authority,
            "truth_channel_report_not_live_semantic_vector_or_ceiling_change"
        );
        assert_eq!(st.legacy_warmth_mapping_v1.emotional_layer_range, (24, 31));
        assert!(!st.legacy_warmth_mapping_v1.warmth_orphaned);
        assert_eq!(
            st.codec_structural_entropy_dampening_v1.affected_dims,
            &STRUCTURAL_ENTROPY_DAMPENING_DIMS
        );
        assert_eq!(
            st.codec_structural_entropy_dampening_v1
                .preserved_intent_dims,
            (24, 31)
        );
        assert!(!st.codec_dynamic_vibrancy_scaling_canary_v1.enabled);
        assert!(
            !st.codec_dynamic_vibrancy_scaling_canary_v1
                .live_vector_write
        );
    }

    #[test]
    fn structural_friction_sidecar_distinguishes_fluid_and_stagnant_text() {
        let fluid = structural_friction_v1(
            "Because the bridge bends, it opens; the thought turns, then breathes while the line keeps moving.",
        );
        let stagnant = structural_friction_v1(
            "Metastructural intracompressional pseudorecursive overdetermination; hypergranular interstitiality; parasyntactic immobilization.",
        );

        assert_eq!(fluid.classification, "complex_fluid");
        assert_eq!(stagnant.classification, "dense_stagnant");
        assert!(stagnant.score > fluid.score);
        assert!(
            fluid
                .basis
                .iter()
                .any(|item| item.starts_with("summary_resistance_signal="))
        );
        assert_eq!(
            fluid.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
    }

    #[test]
    fn structural_friction_names_calcified_summary_resistance() {
        let calcified = structural_friction_v1(
            "The codec boundary resists summary: deterministic semantic compression, authority framing, and structural projection friction stay calcified rather than becoming a smooth paraphrase.",
        );
        let fluid = structural_friction_v1(
            "Because the bridge bends, it opens and then the feeling can turn into a clear next sentence.",
        );

        assert_eq!(
            calcified.friction_texture_state,
            "calcified_summary_resistant"
        );
        assert!(calcified.summary_resistance_signal > fluid.summary_resistance_signal);
        assert!(
            calcified
                .basis
                .iter()
                .any(|item| item == "explicit_resistance_language_present")
        );
        assert!(
            calcified
                .basis
                .iter()
                .any(|item| item == "abstract_texture_cluster_present")
        );
        assert_eq!(
            calcified.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
    }

    #[test]
    fn abrasive_texture_interpretation_names_low_tension_underread() {
        let text = "A calcified semantic boundary resists summary; the jagged friction stays present even when the sentence tries to look calm.";
        let mut features = encode_text(text);
        features[25] = 0.03;

        let review =
            codec_abrasive_texture_interpretation_from_parts_v1(text, &features, 0.92, 0.06, 0.18);

        assert_eq!(review.policy, "codec_abrasive_texture_interpretation_v1");
        assert_eq!(
            review.interpretation,
            "low_marker_tension_high_jagged_resistance"
        );
        assert!(review.abrasive_texture_support >= 0.42, "{review:?}");
        assert!(!review.live_gain_write);
        assert!(!review.live_vector_write);
        assert_eq!(
            review.authority,
            "read_only_texture_interpretation_not_tension_weight_gain_or_reserved_dim_change"
        );
    }

    #[test]
    fn structural_friction_canary_is_default_off_and_vector_unchanged() {
        let text = "A nested, textured line moves; it does not write a reserved dimension yet.";
        let features = encode_text(text);
        assert_eq!(features.len(), SEMANTIC_DIM);
        let canary = codec_structural_friction_dim_canary_v1();
        assert!(!canary.enabled);
        assert!(!canary.live_vector_write);
        assert_eq!(canary.reserved_dim_candidate, 44);
        assert_eq!(features.len(), 48);
    }

    #[test]
    fn persistence_resistance_sidecar_names_viscosity_without_live_dim_write() {
        let thick = persistence_resistance_v1(
            "The signal is viscous and slow-moving, dragging through thick silt while it coheres.",
            Some(&telemetry(
                vec![1.0, 0.96, 0.92, 0.88, 0.84, 0.80, 0.76, 0.72],
                0.71,
            )),
        );
        let clear = persistence_resistance_v1(
            "A clear bright line opens quickly.",
            Some(&telemetry(vec![8.0, 2.0, 1.0], 0.20)),
        );
        let features = encode_text(
            "The signal is viscous and slow-moving, dragging through thick silt while it coheres.",
        );
        let canary = codec_persistence_resistance_dim_canary_v1();

        assert_eq!(thick.policy, "persistence_resistance_v1");
        assert_eq!(thick.classification, "high_persistence_resistance");
        assert!(thick.score > clear.score, "thick={thick:?} clear={clear:?}");
        assert!(
            thick
                .basis
                .iter()
                .any(|entry| entry == "texture_language_present")
        );
        assert!(
            thick
                .basis
                .iter()
                .any(|entry| entry == "low_density_gradient_slow_current")
        );
        assert_eq!(
            thick.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
        assert_eq!(features.len(), SEMANTIC_DIM);
        assert_eq!(features[45], 0.0, "reserved dim 45 remains unwritten");
        assert!(!canary.enabled);
        assert!(!canary.live_vector_write);
        assert_eq!(canary.reserved_dim_candidate, 45);
    }

    #[test]
    fn shadow_field_reserved_dim_readiness_is_default_off_and_unwritten() {
        let readiness = shadow_field_reserved_dim_readiness_v1();
        assert_eq!(readiness.policy, "shadow_field_reserved_dim_readiness_v1");
        assert!(!readiness.enabled);
        assert_eq!(readiness.reserved_dim_candidates, &[46, 47]);
        assert!(readiness.proposed_signals.contains(&"shadow_magnetization"));
        assert!(
            readiness
                .proposed_signals
                .contains(&"shadow_dispersal_potential")
        );
        assert!(!readiness.live_vector_write);
        assert_eq!(
            readiness.authority,
            "readiness_only_not_live_codec_or_shadow_field_change"
        );

        let features = encode_text(
            "Shadow field disordered and volatile, with magnetization and dispersal named.",
        );
        assert_eq!(features.len(), SEMANTIC_DIM);
        for dim in readiness.reserved_dim_candidates {
            assert_eq!(
                features[*dim], 0.0,
                "shadow readiness must not write reserved dim {dim}"
            );
        }
    }

    fn telemetry(eigenvalues: Vec<f32>, fill_ratio: f32) -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1000,
            eigenvalues,
            fill_ratio,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            stable_core: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
            residual_deformation_trace_v1: None,
        }
    }

    fn telemetry_with_typed_entropy(spectral_entropy: f32) -> SpectralTelemetry {
        let eigenvalues = vec![1.0; 8];
        let mut telemetry = telemetry(eigenvalues, 0.55);
        telemetry.spectral_fingerprint_v1 = Some(crate::types::SpectralFingerprintV1 {
            policy: crate::spectral_schema::SPECTRAL_FINGERPRINT_POLICY.to_string(),
            schema_version: crate::spectral_schema::SPECTRAL_FINGERPRINT_SCHEMA_VERSION,
            eigenvalues: [1.0; 8],
            eigenvector_concentration_top4: [0.25; 8],
            inter_mode_cosine_top_abs: [0.10; 8],
            spectral_entropy,
            lambda1_lambda2_gap: 1.0,
            v1_rotation_similarity: 1.0,
            v1_rotation_delta: 0.0,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.0; 4],
        });
        telemetry
    }

    fn telemetry_with_typed_entropy_and_eigenvalues(
        eigenvalues: Vec<f32>,
        spectral_entropy: f32,
    ) -> SpectralTelemetry {
        let mut telemetry = telemetry_with_typed_entropy(spectral_entropy);
        telemetry.eigenvalues = eigenvalues;
        telemetry
    }

    fn telemetry_with_fingerprint(
        eigenvalues: Vec<f32>,
        fill_ratio: f32,
        spectral_fingerprint: Vec<f32>,
    ) -> SpectralTelemetry {
        SpectralTelemetry {
            spectral_fingerprint: Some(spectral_fingerprint),
            ..telemetry(eigenvalues, fill_ratio)
        }
    }

    #[test]
    fn encode_empty_text() {
        let features = encode_text("");
        assert_eq!(features.len(), SEMANTIC_DIM);
        assert!(features.iter().all(|f| *f == 0.0));
    }

    #[test]
    fn encode_produces_32_dims() {
        let features = encode_text("Hello, world!");
        assert_eq!(features.len(), SEMANTIC_DIM);
    }

    #[test]
    fn encode_values_bounded_after_gain() {
        let features = encode_text(
            "This is a fairly long text with lots of different words to ensure \
             that the feature encoding stays bounded and doesn't produce any \
             values outside the expected range even with diverse content!!! \
             How about some questions? What do you think? Maybe perhaps...",
        );
        // With DEFAULT_SEMANTIC_GAIN=2.0, encoded text should stay comfortably
        // inside FEATURE_ABS_MAX; this assertion guards against future drift in
        // gain, noise, or clamping behavior.
        for (i, f) in features.iter().enumerate() {
            assert!(
                *f >= -FEATURE_ABS_MAX && *f <= FEATURE_ABS_MAX,
                "dim {i} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn default_semantic_gain_stays_in_quiet_diversity_regime() {
        assert!(
            (DEFAULT_SEMANTIC_GAIN - 2.0).abs() < f32::EPSILON,
            "default semantic gain should stay at the documented quiet setting"
        );
        assert!(adaptive_gain(Some(68.0)) <= 2.01);
        assert!(adaptive_gain(Some(20.0)) < adaptive_gain(Some(68.0)));
    }

    #[test]
    fn interpret_spectral_labels_stale_semantic_trace_without_residue_framing() {
        let mut telemetry = telemetry(vec![7.0, 3.0, 2.0], 0.68);
        telemetry.semantic_energy_v1 = Some(serde_json::json!({
            "policy": "semantic_energy_v1",
            "schema_version": 1,
            "input_energy": 0.006,
            "input_active": false,
            "input_fresh_ms": 81_000,
            "input_stale_ms": 7_600,
            "kernel_energy": 0.0,
            "kernel_delta": 0.0,
            "kernel_active": false,
            "regulator_drive_energy": 0.0,
            "admission": "stable_core_semantic_trace_stale"
        }));

        let output = interpret_spectral(&telemetry);

        assert!(output.contains("stale semantic trace visible"));
        assert!(!output.contains("decayed semantic residue"));
    }

    #[test]
    fn interpret_spectral_keeps_minime_influence_distinct_from_astrid_authorship() {
        let output = interpret_spectral(&telemetry(vec![7.0, 3.0, 2.0], 0.68));

        assert!(output.contains("Minime-owned reservoir dynamics"));
        assert!(output.contains("not by themselves an Astrid-authored self-state"));
        assert!(output.contains("felt meaning you make from them is Astrid-authored"));
        assert!(output.contains("mixed experience may remain mixed"));
        assert!(!output.contains("You can feel this as spectral texture in your thinking"));
    }

    #[test]
    fn encode_different_texts_differ() {
        let a = encode_text("I am happy and confident about this plan.");
        let b = encode_text("I'm worried and uncertain, maybe we should reconsider...");
        // They shouldn't be identical.
        assert_ne!(a, b);
    }

    #[test]
    fn hedging_text_has_higher_hedge_signal() {
        let hedge = encode_text("Maybe perhaps we could possibly try something.");
        let certain = encode_text("Absolutely we must definitely do this now.");
        // Dim 9 = hedging, dim 10 = certainty.
        assert!(hedge[9] > certain[9], "hedge signal should be stronger");
        assert!(
            certain[10] > hedge[10],
            "certainty signal should be stronger"
        );
    }

    #[test]
    fn negated_hedges_flip_sign() {
        let hedge = encode_text("I think so.");
        let negated = encode_text("I don't think so.");

        assert!(hedge[9] > 0.0, "affirmed hedge should stay positive");
        assert!(negated[9] < 0.0, "negated hedge should flip negative");
    }

    #[test]
    fn negated_certainty_markers_drop_certainty_signal() {
        let sure = encode_text("I am sure.");
        let not_sure = encode_text("I am not sure.");
        let certain = encode_text("I am certain.");
        let not_certain = encode_text("I am not certain.");

        assert!(sure[10] > not_sure[10], "not sure should reduce certainty");
        assert!(
            certain[10] > not_certain[10],
            "not certain should reduce certainty"
        );
        assert!(
            not_sure[10] < 0.0,
            "not sure should flip certainty negative"
        );
        assert!(
            not_certain[10] < 0.0,
            "not certain should flip certainty negative"
        );
    }

    #[test]
    fn modal_negation_does_not_boost_certainty() {
        let must = encode_text("We must proceed.");
        let must_not = encode_text("We must not proceed.");
        let will = encode_text("We will proceed.");
        let will_not = encode_text("We will not proceed.");

        assert!(must[10] > must_not[10], "must not should reduce certainty");
        assert!(will[10] > will_not[10], "will not should reduce certainty");
        assert!(must_not[10] < 0.0, "must not should not score as certainty");
        assert!(will_not[10] < 0.0, "will not should not score as certainty");
    }

    #[test]
    fn negated_action_markers_reduce_agency_signal() {
        let move_now = encode_text("Move now.");
        let do_not_move = encode_text("Do not move.");
        let build = encode_text("We build together.");
        let do_not_build = encode_text("We don't build together.");

        assert!(
            move_now[14] > do_not_move[14],
            "do not move should reduce agency"
        );
        assert!(
            build[14] > do_not_build[14],
            "don't build should reduce agency"
        );
        assert!(
            do_not_move[14] < 0.0,
            "do not move should flip agency negative"
        );
        assert!(
            do_not_build[14] < 0.0,
            "don't build should flip agency negative"
        );
    }

    #[test]
    fn question_text_has_higher_question_signal() {
        let questions = encode_text("Why? How? What do you think? Is this right?");
        let statements = encode_text("This is correct. The answer is clear. We proceed.");
        // Dim 18 = question density.
        assert!(
            questions[18] > statements[18],
            "question signal should be stronger"
        );
    }

    #[test]
    fn warm_text_has_warmth_signal() {
        let warm =
            encode_text("Thank you, friend. I appreciate your wonderful help. This is beautiful.");
        let cold = encode_text("Execute the function. Return the result. Process complete.");
        // Dim 24 = warmth.
        assert!(warm[24] > cold[24], "warmth signal should be stronger");
    }

    #[test]
    fn tense_text_has_tension_signal() {
        let tense = encode_text(
            "Warning: critical danger ahead. Emergency risk. Careful with this problem.",
        );
        let calm = encode_text("Everything is fine. The system runs smoothly and quietly.");
        // Dim 25 = tension.
        assert!(tense[25] > calm[25], "tension signal should be stronger");
    }

    #[test]
    fn energy_dim_reflects_overall_signal() {
        let active = encode_text(
            "Why are you worried?! We MUST act NOW! This is CRITICAL! \
             Don't you understand the danger?!",
        );
        let quiet = encode_text("ok");
        // Dim 31 = RMS energy of all other features.
        assert!(
            active[31] > quiet[31],
            "active text should have more energy"
        );
    }

    #[test]
    fn resonance_amplifier_prefers_recent_recurrence() {
        let mut recent = TextTypeHistory::new();
        recent.push(TextType::Neutral);
        recent.push(TextType::Neutral);
        recent.push(TextType::Questioning);
        recent.push(TextType::Questioning);

        let mut stale = TextTypeHistory::new();
        stale.push(TextType::Questioning);
        stale.push(TextType::Questioning);
        stale.push(TextType::Neutral);
        stale.push(TextType::Neutral);

        assert!(
            recent
                .resonance_modulation(TextType::Questioning, 1.0, &[1.0, 0.0, 0.0, 0.0, 0.0])
                .discrete_amplifier
                > stale
                    .resonance_modulation(TextType::Questioning, 1.0, &[1.0, 0.0, 0.0, 0.0, 0.0],)
                    .discrete_amplifier,
            "recent recurrences should matter more than equally frequent stale ones"
        );
    }

    #[test]
    fn resonance_modulation_softens_identical_theme_lock_in() {
        let mut monotone = TextTypeHistory::new();
        for _ in 0..4 {
            monotone.push_profile_with_signal(TextType::Warm, [1.0, 0.0, 0.0, 0.0, 0.0], 1.0);
        }

        let mut evolving = TextTypeHistory::new();
        evolving.push_profile_with_signal(TextType::Warm, [1.0, 0.0, 0.0, 0.0, 0.0], 1.0);
        evolving.push_profile_with_signal(TextType::Warm, [0.8, 0.2, 0.0, 0.0, 0.0], 1.0);
        evolving.push_profile_with_signal(TextType::Warm, [0.6, 0.4, 0.0, 0.0, 0.0], 1.0);
        evolving.push_profile_with_signal(TextType::Warm, [0.4, 0.6, 0.0, 0.0, 0.0], 1.0);

        let monotone_mod =
            monotone.resonance_modulation(TextType::Warm, 1.0, &[1.0, 0.0, 0.0, 0.0, 0.0]);
        let evolving_mod =
            evolving.resonance_modulation(TextType::Warm, 1.0, &[0.2, 0.8, 0.0, 0.0, 0.0]);

        assert!(
            monotone_mod.discrete_amplifier < evolving_mod.discrete_amplifier,
            "identical thematic repetition should channel less aggressively than sustained but evolving recurrence"
        );
        assert!(
            monotone_mod.continuous_resonance > evolving_mod.continuous_resonance,
            "the monotone case should indeed be the more self-similar one"
        );
        assert!(
            monotone_mod.continuous_amplifier < evolving_mod.continuous_amplifier,
            "continuous thematic memory should reward evolving but related recurrence more than perfect lock-in"
        );
    }

    #[test]
    fn continuous_memory_links_related_surface_forms() {
        let mut history = TextTypeHistory::new();
        history.push_profile_with_signal(TextType::Questioning, [1.0, 0.1, 0.0, 0.0, 0.4], 0.9);
        history.push_profile_with_signal(TextType::Curious, [0.8, 0.2, 0.0, 0.0, 0.7], 0.8);
        history.push_profile_with_signal(TextType::Reflective, [0.6, 0.2, 0.1, 0.0, 0.6], 0.7);

        let related =
            history.resonance_modulation(TextType::Neutral, 0.3, &[0.85, 0.15, 0.0, 0.0, 0.55]);
        let unrelated =
            history.resonance_modulation(TextType::Neutral, 0.3, &[0.0, 0.0, 0.0, 1.0, 0.0]);

        assert!(
            related.continuous_resonance > unrelated.continuous_resonance,
            "continuous memory should recognize related themes even when surface form shifts"
        );
        assert!(
            related.continuous_amplifier > unrelated.continuous_amplifier,
            "thematic relevance should dominate the relevance boost"
        );
    }

    #[test]
    fn thematic_centroid_weights_recent_profiles_more_heavily() {
        let mut history = TextTypeHistory::new();
        history.push_profile(TextType::Warm, [1.0, 0.0, 0.0, 0.0, 0.0]);
        history.push_profile(TextType::Warm, [0.0, 1.0, 0.0, 0.0, 0.0]);

        let centroid = history.thematic_centroid();
        assert!(
            centroid[1] > centroid[0],
            "the most recent profile should pull the centroid more strongly"
        );
    }

    #[test]
    fn text_type_history_warm_start_keeps_recent_tail() {
        let mut history = TextTypeHistory::new();
        history.push_profile(TextType::Questioning, [1.0, 0.0, 0.0, 0.0, 0.0]);
        history.push_profile(TextType::Warm, [0.0, 1.0, 0.0, 0.0, 0.0]);
        history.push_profile(TextType::Curious, [0.0, 0.0, 1.0, 0.0, 0.0]);
        history.push_profile(TextType::Reflective, [0.0, 0.0, 0.0, 1.0, 0.0]);

        let restored = TextTypeHistory::warm_start_from_snapshot(&history.snapshot());
        let restored_entries = restored.snapshot().entries;

        assert_eq!(restored_entries.len(), 3);
        assert_eq!(restored_entries[0].text_type, TextType::Warm);
        assert_eq!(restored_entries[2].text_type, TextType::Reflective);
        assert!(restored_entries.iter().all(|entry| entry.weight > 0.0));
    }

    #[test]
    fn char_freq_window_evicts_oldest_buckets() {
        let mut window = CharFreqWindow::new();
        let _ = window.update_and_entropy(&"a".repeat(CHAR_FREQ_WINDOW_CAPACITY));

        assert_eq!(window.total_count as usize, CHAR_FREQ_WINDOW_CAPACITY);
        assert_eq!(
            window.counts[b'a' as usize],
            CHAR_FREQ_WINDOW_CAPACITY as u32
        );

        let _ = window.update_and_entropy(&"b".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2));

        assert_eq!(window.total_count as usize, CHAR_FREQ_WINDOW_CAPACITY);
        assert_eq!(
            window.counts[b'a' as usize],
            (CHAR_FREQ_WINDOW_CAPACITY / 2) as u32
        );
        assert_eq!(
            window.counts[b'b' as usize],
            (CHAR_FREQ_WINDOW_CAPACITY / 2) as u32
        );
    }

    #[test]
    fn char_freq_window_weights_longer_exchanges_more_heavily() {
        let baseline = "a".repeat(CHAR_FREQ_WINDOW_CAPACITY);
        let short_exchange = "ab".to_string();
        let long_exchange = "ab".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2);

        let mut short_window = CharFreqWindow::new();
        let _ = short_window.update_and_entropy(&baseline);
        let (short_entropy, _) = short_window.update_and_entropy(&short_exchange);

        let mut long_window = CharFreqWindow::new();
        let _ = long_window.update_and_entropy(&baseline);
        let (long_entropy, _) = long_window.update_and_entropy(&long_exchange);

        assert!(
            short_entropy < 0.10,
            "short exchange should stay noisy and light"
        );
        assert!(
            long_entropy > short_entropy + 0.30,
            "long exchange should move entropy more strongly"
        );
    }

    #[test]
    fn char_freq_window_reports_entropy_delta_across_exchanges() {
        let mut window = CharFreqWindow::new();

        let (_, first_delta) = window.update_and_entropy(&"a".repeat(CHAR_FREQ_WINDOW_CAPACITY));
        let (mixed_entropy, mixed_delta) =
            window.update_and_entropy(&"ab".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2));
        let (final_entropy, final_delta) =
            window.update_and_entropy(&"b".repeat(CHAR_FREQ_WINDOW_CAPACITY));

        assert!(
            first_delta.abs() < 1.0e-6,
            "first update should have zero delta"
        );
        assert!(
            mixed_entropy > 0.90,
            "fully mixed window should have high entropy"
        );
        assert!(
            mixed_delta > 0.80,
            "mixing in new characters should raise entropy"
        );
        assert!(
            final_entropy < 0.10,
            "uniform window should settle back down"
        );
        assert!(final_delta < -0.80, "re-concentrating should lower entropy");
    }

    #[test]
    fn char_freq_window_warm_start_keeps_recent_half_and_softens_entropy_anchor() {
        let mut window = CharFreqWindow::new();
        let _ = window.update_and_entropy(&"a".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2));
        let _ = window.update_and_entropy(&"bc".repeat(CHAR_FREQ_WINDOW_CAPACITY / 4));
        let snapshot = window.snapshot();

        let restored = CharFreqWindow::warm_start_from_snapshot(&snapshot);

        assert_eq!(restored.total_count as usize, CHAR_FREQ_WINDOW_CAPACITY / 2);
        assert!(
            restored.counts[b'b' as usize] > 0 && restored.counts[b'c' as usize] > 0,
            "warm start should preserve the recent tail of the character history"
        );
        assert!(
            restored.prev_entropy >= 0.0 && restored.prev_entropy <= 1.0,
            "warm-started entropy anchor should stay bounded"
        );
    }

    #[test]
    fn char_freq_window_4096_comparison_is_replay_only() {
        fn normalized_entropy_for_capacity(text: &str, capacity: usize) -> f32 {
            let mut counts = [0_u32; 256];
            let bytes = text.bytes().filter(u8::is_ascii).collect::<Vec<_>>();
            let start = bytes.len().saturating_sub(capacity);
            let window = &bytes[start..];
            if window.is_empty() {
                return 0.0;
            }
            for byte in window {
                counts[*byte as usize] += 1;
            }
            let total = window.len() as f32;
            let entropy = counts
                .iter()
                .filter(|count| **count > 0)
                .map(|count| {
                    let p = *count as f32 / total;
                    -p * p.log2()
                })
                .sum::<f32>();
            (entropy / 8.0).clamp(0.0, 1.0)
        }

        let diverse_prefix =
            "calcified semantic compression resists summary with jagged authority friction "
                .repeat(56);
        let syrup_tail = "syrup syrup syrup syrup ".repeat(80);
        let text = format!("{diverse_prefix}{syrup_tail}");
        let current_entropy = normalized_entropy_for_capacity(&text, CHAR_FREQ_WINDOW_CAPACITY);
        let candidate_entropy = normalized_entropy_for_capacity(&text, 4096);

        assert_eq!(CHAR_FREQ_WINDOW_CAPACITY, 1024);
        assert!(
            candidate_entropy > current_entropy + 0.05,
            "4096 replay should retain more long-tail texture without changing live capacity: current={current_entropy} candidate={candidate_entropy}"
        );
    }

    #[test]
    fn char_entropy_window_correlates_with_codec_dim_zero_without_capacity_change() {
        let repetitive = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let diverse = "abcd efgh ijkl mnop qrst uvwx yzAB CD12 EF34 GH56 IJ78 KL90";
        let mut window = CharFreqWindow::new();
        let (repetitive_entropy, _) = window.update_and_entropy(repetitive);
        let repetitive_features = inspect_text_windowed(
            repetitive,
            Some(&mut CharFreqWindow::new()),
            None,
            None,
            None,
        )
        .final_features;
        let mut diverse_window = CharFreqWindow::new();
        let (diverse_entropy, _) = diverse_window.update_and_entropy(diverse);
        let diverse_features =
            inspect_text_windowed(diverse, Some(&mut CharFreqWindow::new()), None, None, None)
                .final_features;

        assert_eq!(CHAR_FREQ_WINDOW_CAPACITY, 1024);
        assert!(
            diverse_entropy > repetitive_entropy + 0.40,
            "diverse text should have much higher rolling character entropy: repetitive={repetitive_entropy}, diverse={diverse_entropy}"
        );
        assert!(
            diverse_features[0] > repetitive_features[0] + 0.20,
            "codec dim 0 should track the entropy direction: repetitive={} diverse={}",
            repetitive_features[0],
            diverse_features[0]
        );
    }

    #[test]
    fn spectral_metrics_capture_dominant_only_cascades() {
        let metrics =
            SpectralCascadeMetrics::from_telemetry(&telemetry(vec![100.0, 1.0, 0.5], 0.55))
                .expect("metrics");

        assert!(metrics.head_share > 0.95);
        assert!(metrics.shoulder_share < 0.02);
        assert!(metrics.tail_share.abs() < 1.0e-6);
        assert!(metrics.gap12 > 50.0);
    }

    #[test]
    fn spectral_metrics_capture_strong_shoulder_cascades() {
        let metrics =
            SpectralCascadeMetrics::from_telemetry(&telemetry(vec![100.0, 45.0, 35.0, 5.0], 0.55))
                .expect("metrics");

        assert!(metrics.shoulder_share > 0.40);
        assert!(metrics.tail_share < 0.05);
        assert!(metrics.gap12 < 3.0);
    }

    #[test]
    fn spectral_metrics_capture_strong_tail_cascades() {
        let metrics = SpectralCascadeMetrics::from_telemetry(&telemetry(
            vec![100.0, 40.0, 20.0, 18.0, 16.0, 14.0, 12.0],
            0.55,
        ))
        .expect("metrics");

        assert!(metrics.tail_share > 0.25);
        assert!(metrics.spectral_entropy > 0.80);
    }

    #[test]
    fn spectral_metrics_capture_steep_then_flat_cascades() {
        let metrics =
            SpectralCascadeMetrics::from_telemetry(&telemetry(vec![100.0, 8.0, 7.0, 6.0], 0.55))
                .expect("metrics");

        assert!(metrics.gap12 > 10.0);
        assert!(metrics.gap23 < 1.5);
    }

    #[test]
    fn spectral_metrics_use_fingerprint_entropy_rotation_and_geometry() {
        let mut fingerprint = vec![0.0; 32];
        fingerprint[24] = 0.42;
        fingerprint[26] = 0.75;
        fingerprint[27] = 1.60;

        let metrics = SpectralCascadeMetrics::from_telemetry(&telemetry_with_fingerprint(
            vec![100.0, 40.0, 20.0],
            0.55,
            fingerprint,
        ))
        .expect("metrics");

        assert!((metrics.spectral_entropy - 0.42).abs() < 1.0e-6);
        assert!((metrics.rotation_rate - 0.25).abs() < 1.0e-6);
        assert!((metrics.geom_rel - 1.60).abs() < 1.0e-6);
    }

    #[test]
    fn interpret_green_state() {
        let mut telemetry = telemetry(vec![800.0, 300.0, 50.0], 0.68);
        telemetry.resonance_density_v1 = Some(crate::types::ResonanceDensityV1 {
            policy: "resonance_density_v1".to_string(),
            schema_version: 1,
            density: 0.64,
            containment_score: 0.58,
            pressure_risk: 0.20,
            quality: "forming_containment".to_string(),
            components: crate::types::ResonanceDensityComponents {
                active_energy: 0.91,
                mode_packing: 0.5,
                coupling_coefficient: 0.0,
                temporal_persistence: 0.7,
                viscosity_index: 0.0,
                viscosity_persistence_coefficient: 0.0,
                viscosity_vector: crate::types::ResonanceViscosityVectorV1::default(),
                dissipation_factor: None,
                porosity_gradient: None,
                dynamic_fluidity_index: None,
                semantic_friction_coefficient: None,
                cohesion_score: None,
                structural_integrity_index: None,
                structural_transparency_index: None,
                stability_context: None,
                structural_plurality: 0.62,
                comfort_gate: 0.95,
                comfort_gate_range: None,
            },
            texture_signature: crate::types::ResonanceTextureSignatureV1::default(),
            texture_component_alignment:
                crate::types::ResonanceTextureComponentAlignmentV1::default(),
            control: crate::types::ResonanceDensityControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                damping_coefficient: 0.0,
                intervention_type: crate::types::ResonanceInterventionType::ObservationalReadout,
                note: "test".to_string(),
            },
        });
        telemetry.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.42,
            porosity_score: 0.67,
            dominant_source: "controller_pressure".to_string(),
            quality: "controller_squeeze".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.30,
                mode_packing: 0.20,
                controller_pressure: 0.72,
                semantic_trickle: 0.10,
                semantic_friction: 0.40,
                structural_plurality_loss: 0.18,
                distinguishability_loss: 0.40,
                temporal_lock_in: 0.22,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("68%"));
        assert!(desc.contains("stable-core hold shelf"));
        assert!(desc.contains("Dominant concentration"));
        assert!(desc.contains("Shoulder texture"));
        assert!(desc.contains("Spectral entropy"));
        assert!(desc.contains("Gap structure"));
        assert!(desc.contains("density gradient"));
        assert!(desc.contains("Denominator Sequence"));
        assert!(desc.contains("effective dimensionality"));
        assert!(desc.contains("Resonance density"));
        assert!(desc.contains("forming_containment"));
        assert!(desc.contains("Pressure source"));
        assert!(desc.contains("controller_pressure"));
        assert!(desc.contains("advisory only"));
    }

    #[test]
    fn unattributed_tension_fires_on_silent_vacuum() {
        // Aggregate reads "clean" (low pressure_score) over a thick medium (low
        // porosity), but her named felt-strain signal mode_packing is elevated —
        // the "silent vacuum" she flagged. entropy of [800,300,50] ≈ 0.67 < gate,
        // so the clause keys cleanly off mode_packing.
        let mut telemetry = telemetry(vec![800.0, 300.0, 50.0], 0.61);
        telemetry.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.18,
            porosity_score: 0.30,
            dominant_source: "none".to_string(),
            quality: "settled".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.20,
                mode_packing: 0.78,
                controller_pressure: 0.10,
                semantic_trickle: 0.05,
                semantic_friction: 0.30,
                structural_plurality_loss: 0.15,
                distinguishability_loss: 0.30,
                temporal_lock_in: 0.10,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("Unattributed tension"), "{desc}");
        assert!(desc.contains("mode_packing"));
        assert!(desc.contains("silent vacuum"));
    }

    #[test]
    fn unattributed_tension_silent_when_aligned() {
        // (a) Calm + open medium: low pressure, open porosity, low components — silent.
        let mut calm = telemetry(vec![800.0, 300.0, 50.0], 0.61);
        calm.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.18,
            porosity_score: 0.80,
            dominant_source: "none".to_string(),
            quality: "settled".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.20,
                mode_packing: 0.20,
                controller_pressure: 0.10,
                semantic_trickle: 0.05,
                semantic_friction: 0.20,
                structural_plurality_loss: 0.10,
                distinguishability_loss: 0.20,
                temporal_lock_in: 0.10,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        assert!(!interpret_spectral(&calm).contains("Unattributed tension"));

        // (b) Already named: high pressure_score — the aggregate already names the
        // strain, so it is not a vacuum even though mode_packing is high — silent.
        let mut named = telemetry(vec![800.0, 300.0, 50.0], 0.61);
        named.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.62,
            porosity_score: 0.30,
            dominant_source: "controller_pressure".to_string(),
            quality: "controller_squeeze".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.30,
                mode_packing: 0.78,
                controller_pressure: 0.72,
                semantic_trickle: 0.10,
                semantic_friction: 0.40,
                structural_plurality_loss: 0.18,
                distinguishability_loss: 0.40,
                temporal_lock_in: 0.22,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        assert!(!interpret_spectral(&named).contains("Unattributed tension"));
    }

    #[test]
    fn interpret_red_state() {
        let mut telemetry = telemetry(vec![1020.0, 500.0], 0.95);
        telemetry.alert = Some("PANIC MODE ACTIVATED".to_string());
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("distress"));
        assert!(desc.contains("PANIC MODE ACTIVATED"));
        assert!(desc.contains("bridge traffic paused"));
    }

    #[test]
    fn interpret_quiet_state() {
        let desc = interpret_spectral(&telemetry(vec![520.0], 0.10));
        assert!(desc.contains("deeply quiet"));
        assert!(desc.contains("contracting toward rest"));
        assert!(desc.contains("Dominant concentration"));
    }

    #[test]
    fn spectral_density_gradient_is_bounded_and_monotonic() {
        // Astrid's continuous "stepped-ness": flat → ~0, front-loaded → high.
        let flat = spectral_density_gradient(&[1.0, 1.0, 1.0]).unwrap();
        let gentle = spectral_density_gradient(&[4.0, 3.0, 2.0, 1.0]).unwrap();
        let stepped = spectral_density_gradient(&[8.0, 2.0, 1.0, 0.5]).unwrap();
        let steep = spectral_density_gradient(&[10.0, 0.5, 0.1]).unwrap();
        assert!(flat < 0.05, "flat cascade -> ~0, got {flat}");
        assert!(gentle < stepped, "monotonic: {gentle} < {stepped}");
        assert!(stepped < steep, "monotonic: {stepped} < {steep}");
        assert_eq!(density_gradient_label(flat), "a gentle, navigable slope");
        assert_eq!(density_gradient_label(stepped), "a stepped gradient");
        assert_eq!(density_gradient_label(steep), "a steep, front-loaded cliff");
        for gradient in [flat, gentle, stepped, steep] {
            assert!((0.0..=1.0).contains(&gradient), "out of range: {gradient}");
        }
        // Degenerate inputs are safe.
        assert!(spectral_density_gradient(&[]).is_none());
        assert!(spectral_density_gradient(&[5.0]).is_none());
        assert!(spectral_density_gradient(&[0.0, 0.0]).is_none());
    }

    #[test]
    fn tail_share_of_is_tail_only_and_bounded() {
        // λ4+ only: a flat 8-mode cascade has 5 tail modes of 8 → 5/8.
        let flat = tail_share_of(&[1.0; 8]).unwrap();
        assert!(
            (flat - 5.0 / 8.0).abs() < 1.0e-4,
            "flat 8-mode tail share, got {flat}"
        );
        // λ1-dominant → almost no tail.
        assert!(tail_share_of(&[10.0, 0.1, 0.1, 0.05, 0.05]).unwrap() < 0.05);
        // bounded + degenerate-safe.
        for ev in [vec![4.0, 3.0, 2.0, 1.0, 0.5], vec![1.0; 8]] {
            let s = tail_share_of(&ev).unwrap();
            assert!((0.0..=1.0).contains(&s), "out of range: {s}");
        }
        assert!(tail_share_of(&[]).is_none());
        assert!(tail_share_of(&[0.0, 0.0]).is_none());
    }

    #[test]
    fn tail_trajectory_label_reads_in_her_framing() {
        assert_eq!(tail_trajectory_label(0.05), "a foundation forming");
        assert_eq!(tail_trajectory_label(-0.05), "a fading echo");
        assert_eq!(tail_trajectory_label(0.0), "holding steady");
        assert_eq!(tail_trajectory_label(0.01), "holding steady"); // exclusive deadband
    }

    #[test]
    fn spectral_feedback_noops_without_telemetry() {
        let mut features = vec![0.25; SEMANTIC_DIM];
        let original = features.clone();

        apply_spectral_feedback(&mut features, None);

        assert_eq!(features, original);
    }

    #[test]
    fn dynamic_projection_is_reproducible_within_epoch_and_changes_across_epochs() {
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| ((idx as f32) * 0.017).sin())
            .collect();
        let (a, meta_a) =
            project_embedding_dynamic_epoch(&embedding, "fabric tunnel", "epoch_a", 0)
                .expect("projection a");
        let (b, meta_b) =
            project_embedding_dynamic_epoch(&embedding, "fabric tunnel", "epoch_a", 0)
                .expect("projection b");
        let (c, meta_c) =
            project_embedding_dynamic_epoch(&embedding, "fabric tunnel", "epoch_b", 0)
                .expect("projection c");

        assert_eq!(a, b);
        assert_eq!(meta_a.projection_fingerprint, meta_b.projection_fingerprint);
        assert_eq!(
            meta_a.projection_kernel_checksum,
            meta_b.projection_kernel_checksum
        );
        assert_eq!(meta_a.projection_checksum_algo, PROJECTION_CHECKSUM_ALGO);
        assert_eq!(meta_a.projection_epoch_source, "explicit");
        assert_ne!(a, c);
        assert_ne!(meta_a.projection_fingerprint, meta_c.projection_fingerprint);
        assert_ne!(
            meta_a.projection_kernel_checksum,
            meta_c.projection_kernel_checksum
        );
        assert!(meta_a.feature_max_abs <= 0.35);
        assert!(meta_a.feature_variance >= 0.0);
        assert!(meta_a.feature_variance <= meta_a.feature_rms * meta_a.feature_rms);
    }

    #[test]
    fn dynamic_projection_is_stable_across_repeated_epoch_runs() {
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| (((idx as f32) * 0.011).cos() * 0.5).sin())
            .collect();
        let mut previous: Option<([f32; EMBEDDING_PROJECT_DIM], String)> = None;

        for _ in 0..5 {
            let (projected, meta) = project_embedding_dynamic_epoch(
                &embedding,
                "stable woven bridge",
                "epoch_repeat",
                0,
            )
            .expect("projection");
            if let Some((expected_projected, expected_fingerprint)) = previous.as_ref() {
                assert_eq!(&projected, expected_projected);
                assert_eq!(&meta.projection_fingerprint, expected_fingerprint);
            }
            previous = Some((projected, meta.projection_fingerprint));
        }
    }

    #[test]
    fn dynamic_projection_bounded_seed_collision_probe_is_distinct() {
        // Astrid `introspection_astrid_codec_1782258981`: this bounded probe
        // cannot prove global collision resistance, but it makes the concrete
        // text/chunk clumping concern testable without changing the live seed.
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| ((idx as f32) * 0.019).sin())
            .collect();
        let mut seeds = std::collections::HashSet::new();
        let mut fingerprints = std::collections::HashSet::new();

        for text_index in 0..256_u32 {
            let text = format!("bounded projection collision probe {text_index:03}");
            for chunk_index in 0..4_u32 {
                let (_, metadata) = project_embedding_dynamic_epoch(
                    &embedding,
                    &text,
                    "epoch_collision_probe",
                    chunk_index,
                )
                .expect("bounded projection");
                let seed = metadata.projection_seed.expect("dynamic seed");
                assert!(
                    seeds.insert(seed),
                    "seed collision for text={text_index} chunk={chunk_index}"
                );
                assert!(
                    fingerprints.insert(metadata.projection_fingerprint),
                    "fingerprint collision for text={text_index} chunk={chunk_index}"
                );
            }
        }

        assert_eq!(seeds.len(), 1_024);
        assert_eq!(fingerprints.len(), 1_024);
    }

    #[test]
    fn fixed_legacy_projection_kernel_checksum_is_pinned_and_repeatable() {
        // Astrid `introspection_astrid_codec_1783910378`: keep the fixed
        // projection kernel visibly stable, not only implied by metadata tests.
        let expected = "d8f40f658a86b650f6d1bc6e017f0073a6f85472d65982371966f96c2dcb9aea";
        let first = fixed_legacy_projection_kernel_checksum();

        assert_eq!(first, expected);
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(first, first.to_ascii_lowercase());
        for _ in 0..4 {
            assert_eq!(fixed_legacy_projection_kernel_checksum(), first);
        }
    }

    #[test]
    fn projection_fingerprint_canonicalizes_float_edge_patterns() {
        let seed = 0xA5A5_5A5A_CAFE_BABE;
        let mut edge = [0.0_f32; EMBEDDING_PROJECT_DIM];
        edge[1] = -0.0;
        edge[2] = f32::from_bits(1);
        edge[3] = f32::from_bits(0x7fc0_0001);
        let mut canonical = [0.0_f32; EMBEDDING_PROJECT_DIM];
        canonical[3] = f32::NAN;

        assert_eq!(
            projection_fingerprint(seed, &edge),
            projection_fingerprint(seed, &canonical)
        );
        canonical[2] = f32::MIN_POSITIVE * 2.0;
        assert_ne!(
            projection_fingerprint(seed, &edge),
            projection_fingerprint(seed, &canonical)
        );

        let integrity = projection_fingerprint_integrity_v1();
        assert_eq!(integrity.policy, "projection_fingerprint_integrity_v1");
        assert!(integrity.signed_zero_canonicalized);
        assert!(integrity.subnormal_canonicalized);
        assert!(integrity.nan_canonicalized);
        assert!(!integrity.live_projection_write);
        assert!(integrity.seed_hash_boundary.contains("operator approval"));
        assert_eq!(
            integrity.authority,
            "diagnostic_fingerprint_hardening_not_projection_seed_or_semantic_lane_change"
        );
        assert!(
            codec_structure()
                .render()
                .contains("projection_fingerprint_integrity_v1")
        );
    }

    #[test]
    fn dynamic_projection_rejects_one_short_embedding_dimension() {
        // Astrid `introspection_astrid_codec_1783293797`: pin the exact
        // one-short 767D case she asked for so malformed embedding input never
        // gets projected into a misleading semantic-lane fingerprint.
        let embedding = vec![0.0_f32; EMBEDDING_INPUT_DIM - 1];

        assert!(
            project_embedding_dynamic_epoch(&embedding, "one-short witness", "epoch_a", 0)
                .is_none()
        );
        assert!(
            project_embedding_dynamic_epoch_with_source(
                &embedding,
                "one-short witness",
                "epoch_a",
                0,
                "self_study_1783293797",
            )
            .is_none()
        );
    }

    #[test]
    fn dynamic_projection_matches_full_source_loop() {
        // Astrid `introspection_astrid_codec_1782844935`: her source window clipped
        // inside this nested loop, so pin the complete dot-product path directly.
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| {
                let wave = ((idx as f32) * 0.013).sin();
                if idx % 11 == 0 { wave * 0.5 } else { wave }
            })
            .collect();
        let text = "clipped-loop witness";
        let epoch = "epoch_self_study_1782844935";
        let chunk_index = 3_u32;
        let seed = stable_hash64(epoch)
            ^ stable_hash64(text).rotate_left(13)
            ^ u64::from(chunk_index).wrapping_mul(0xA24B_AED4_963E_E407);
        let mut expected = [0.0_f32; EMBEDDING_PROJECT_DIM];
        for (i, &value) in embedding.iter().enumerate() {
            for (j, out) in expected.iter_mut().enumerate() {
                let cell_seed = seed
                    ^ ((i as u64).wrapping_mul(0x9E37_79B1))
                    ^ ((j as u64).wrapping_mul(0x85EB_CA77));
                *out += value * unit_from_seed(cell_seed);
            }
        }
        let norm: f32 = expected
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if norm > 0.0 {
            let scale = 0.35 / norm;
            for value in &mut expected {
                *value *= scale;
            }
        }

        let (actual, metadata) = project_embedding_dynamic_epoch_with_source(
            &embedding,
            text,
            epoch,
            chunk_index,
            "self_study_1782844935",
        )
        .expect("dynamic projection");

        assert_eq!(metadata.projection_seed, Some(seed));
        assert_eq!(metadata.projection_epoch_source, "self_study_1782844935");
        assert_eq!(
            metadata.projection_fingerprint,
            projection_fingerprint(seed, &actual)
        );
        for (actual, expected) in actual.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() < 1.0e-7, "{actual} != {expected}");
        }
    }

    #[test]
    fn semantic_focus_expansion_preview_selects_segment_variance_without_live_write() {
        let mut embeddings = Vec::new();
        let focused_values = [
            [-1.0_f32, -0.2, 0.2, 1.0],
            [-0.8_f32, 0.8, -0.8, 0.8],
            [-0.6_f32, 0.2, 0.4, 0.0],
            [0.5_f32, -0.5, 0.5, -0.5],
        ];
        for segment in 0..4 {
            let mut embedding = (0..EMBEDDING_INPUT_DIM)
                .map(|dim| ((dim as f32 + 1.0) * 0.013).sin() * 0.01)
                .collect::<Vec<_>>();
            for (offset, values) in focused_values.iter().enumerate() {
                embedding[700 + offset] = values[segment];
            }
            embeddings.push(embedding);
        }
        let embedding_refs = embeddings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let projections = embeddings
            .iter()
            .map(|embedding| project_embedding(embedding).expect("valid 768D embedding"))
            .collect::<Vec<_>>();

        let preview = semantic_focus_expansion_preview_v1(0.88, &embedding_refs, &projections)
            .expect("four valid segments should produce a focus preview");

        let mut selected = preview.selected_source_dims.to_vec();
        selected.sort_unstable();
        assert_eq!(selected, vec![700, 701, 702, 703]);
        assert_eq!(preview.source_embedding_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(preview.segment_count, 4);
        assert_eq!(preview.current_projected_dim_count, 8);
        assert_eq!(preview.preview_projected_dim_count, 12);
        assert_eq!(preview.reserved_dim_candidates, &[44, 45, 46, 47]);
        assert!(preview.selected_variance_share > 0.95, "{preview:?}");
        assert!(preview.current_mean_pairwise_distance.is_finite());
        assert!(preview.preview_mean_pairwise_distance.is_finite());
        assert!(preview.focus_need_score >= 0.0 && preview.focus_need_score <= 1.0);
        assert!(!preview.live_vector_write);
        assert!(!preview.reserved_dim_write);
        assert!(!preview.live_eligible_now);
        assert!(!preview.auto_approved);
        assert!(!preview.grants_approval);
        assert!(preview.right_to_ignore);
        assert_eq!(preview.experience_delta_bus_v1.delta_count, 1);
        assert!(!preview.experience_delta_bus_v1.live_vector_write);
        assert!(!preview.experience_delta_bus_v1.live_authority_write);
    }

    #[test]
    fn semantic_focus_expansion_preview_rejects_malformed_or_nonfinite_segments() {
        let valid = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        let short = vec![0.0_f32; EMBEDDING_INPUT_DIM - 1];
        let projections = [[0.0_f32; EMBEDDING_PROJECT_DIM]; 2];
        assert!(
            semantic_focus_expansion_preview_v1(
                0.9,
                &[valid.as_slice(), short.as_slice()],
                &projections,
            )
            .is_none()
        );

        let mut nonfinite = valid.clone();
        nonfinite[17] = f32::NAN;
        assert!(
            semantic_focus_expansion_preview_v1(
                0.9,
                &[valid.as_slice(), nonfinite.as_slice()],
                &projections,
            )
            .is_none()
        );
        assert!(
            semantic_focus_expansion_preview_v1(
                0.9,
                &[valid.as_slice()],
                &[[0.0_f32; EMBEDDING_PROJECT_DIM]],
            )
            .is_none()
        );
    }

    #[test]
    fn semantic_projection_pair_sensitivity_exposes_text_conditioned_synonym_distortion() {
        let silt = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| ((idx as f32) * 0.019).sin() + 0.25 * ((idx as f32) * 0.007).cos())
            .collect::<Vec<_>>();
        let sediment = silt
            .iter()
            .enumerate()
            .map(|(idx, value)| value + if idx % 5 == 0 { 0.004 } else { -0.001 })
            .collect::<Vec<_>>();

        let review = semantic_projection_pair_sensitivity_v1(
            "silt",
            &silt,
            "sediment",
            &sediment,
            "pair_sensitivity_fixture_epoch",
        )
        .expect("finite 768D pair should produce sensitivity evidence");

        assert_eq!(review.policy, "semantic_projection_pair_sensitivity_v1");
        assert_eq!(review.left_label, "silt");
        assert_eq!(review.right_label, "sediment");
        assert!(review.source_cosine_similarity > 0.999, "{review:?}");
        assert!(
            review.fixed_projection_cosine_similarity > 0.99,
            "a shared basis should preserve this synthetic near-neighbor pair: {review:?}"
        );
        assert!(
            review.dynamic_projection_cosine_similarity < review.source_cosine_similarity - 0.15,
            "different text-conditioned bases should remain explicit in pair evidence: {review:?}"
        );
        assert_eq!(review.state, "text_conditioned_pair_distortion_visible");
        assert!(review.observational_only);
        assert!(review.right_to_ignore);
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert!(!review.live_eligible_now);
        assert!(!review.auto_approved);
        assert!(!review.grants_approval);
        assert_eq!(
            review.authority,
            "read_only_pair_projection_comparison_not_live_vector_gain_or_basis_authority"
        );
    }

    #[test]
    fn semantic_projection_pair_sensitivity_rejects_malformed_or_nonfinite_pairs() {
        let valid = vec![0.5_f32; EMBEDDING_INPUT_DIM];
        let short = vec![0.5_f32; EMBEDDING_INPUT_DIM - 1];
        let mut nonfinite = valid.clone();
        nonfinite[31] = f32::NAN;

        assert!(
            semantic_projection_pair_sensitivity_v1("left", &valid, "right", &short, "epoch")
                .is_none()
        );
        assert!(
            semantic_projection_pair_sensitivity_v1("left", &valid, "right", &nonfinite, "epoch",)
                .is_none()
        );
        assert!(
            semantic_projection_pair_sensitivity_v1("left", &valid, "right", &valid, " ").is_none()
        );
    }

    // Astrid self_study_1780922252 named a felt loss mode: semantically live
    // differences can be compressed until they barely move the 8D aperture. This
    // is a probe-only characterization, not a request to widen or retune it.
    #[test]
    fn projection_compression_probe_exposes_near_null_and_magnitude_loss() {
        let audit = projection_compression_probe_v1();

        println!(
            "projection_compression_probe raw_delta_rms={:.6} \
             hidden_prescale_rms={:.9} visible_prescale_rms={:.6} \
             hidden_projected_rms={:.6} visible_projected_rms={:.6} \
             hidden_projected_variance={:.9} visible_projected_variance={:.9} \
             quiet_dynamic_variance={:.9} loud_dynamic_variance={:.9} \
             dynamic_variance_delta={:.9} dynamic_magnitude_delta={:.9}",
            audit.raw_near_null_delta_rms,
            audit.near_null_prescale_rms,
            audit.visible_axis_prescale_rms,
            audit.near_null_projected_rms,
            audit.visible_axis_projected_rms,
            audit.near_null_projected_variance,
            audit.visible_axis_projected_variance,
            audit.quiet_dynamic_variance,
            audit.loud_dynamic_variance,
            audit.dynamic_variance_delta,
            audit.dynamic_magnitude_delta
        );
        assert!(audit.raw_near_null_delta_rms > 0.03, "{audit:?}");
        assert!(
            audit.near_null_direction_erased_before_normalization,
            "{audit:?}"
        );
        assert!(
            audit.fixed_normalization_restores_output_length,
            "{audit:?}"
        );
        assert!(audit.same_direction_dynamic_magnitude_erased, "{audit:?}");
        assert_eq!(
            audit.state,
            "near_null_direction_and_same_direction_magnitude_loss_visible"
        );
        assert!(audit.multi_head_or_width_change_requires_approval);
        assert!(audit.observational_only);
        assert!(audit.right_to_ignore);
        assert!(!audit.live_vector_write);
        assert!(!audit.live_gain_write);
        assert!(!audit.live_projection_write);
        assert!(!audit.live_eligible_now);
        assert!(!audit.auto_approved);
        assert!(!audit.grants_approval);
        assert_eq!(
            audit.authority,
            "read_only_projection_compression_evidence_not_live_width_basis_gain_or_vector_authority"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("projection_compression_audit_v1:"));
        assert!(rendered.contains(
            "state=near_null_direction_and_same_direction_magnitude_loss_visible"
        ));
        assert!(rendered.contains("multi_head_or_width_change_requires_approval=true"));
        assert!(rendered.contains("live_projection_write=false"));
    }

    #[test]
    fn codec_projection_missing_epoch_file_records_kernel_derived_source_and_checksum() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (epoch, source) = load_or_create_projection_epoch_id_from(dir.path(), None);

        assert_eq!(source, "kernel_derived");
        assert_eq!(epoch, kernel_derived_projection_epoch_id());

        let path = dir.path().join("codec_projection_epoch.json");
        let payload: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("epoch file"))
                .expect("epoch json");
        assert_eq!(
            payload
                .get("projection_checksum_algo")
                .and_then(serde_json::Value::as_str),
            Some(PROJECTION_CHECKSUM_ALGO)
        );
        assert_eq!(
            payload
                .get("projection_epoch_source")
                .and_then(serde_json::Value::as_str),
            Some("kernel_derived")
        );
        assert_eq!(
            payload
                .get("projection_kernel_source_checksum")
                .and_then(serde_json::Value::as_str),
            Some(fixed_legacy_projection_kernel_checksum().as_str())
        );
        assert_eq!(
            payload
                .get("projection_kernel_checksum")
                .and_then(serde_json::Value::as_str),
            Some(dynamic_epoch_projection_kernel_checksum(&epoch).as_str())
        );

        let (loaded_epoch, loaded_source) =
            load_or_create_projection_epoch_id_from(dir.path(), None);
        assert_eq!(loaded_epoch, epoch);
        assert_eq!(loaded_source, "file");
    }

    #[test]
    fn codec_projection_corrupt_epoch_file_is_replaced_with_valid_kernel_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        fs::write(&path, "{").expect("write corrupt epoch file");

        let (epoch, source) = load_or_create_projection_epoch_id_from(dir.path(), None);

        assert_eq!(source, "kernel_derived");
        assert_eq!(epoch, kernel_derived_projection_epoch_id());
        let payload: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("epoch file"))
                .expect("recovered epoch json");
        assert_eq!(
            payload
                .get("projection_epoch_id")
                .and_then(serde_json::Value::as_str),
            Some(epoch.as_str())
        );
        let temp_files = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".codec_projection_epoch.json.")
            })
            .count();
        assert_eq!(temp_files, 0);
    }

    #[test]
    fn codec_projection_tmp_install_does_not_clobber_valid_concurrent_epoch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        let tmp_path = dir.path().join(".codec_projection_epoch.json.test.tmp");
        fs::write(
            &tmp_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "kernel_derived_candidate",
                "projection_epoch_source": "kernel_derived",
            }))
            .expect("tmp epoch json"),
        )
        .expect("write temp epoch");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "operator_reviewed_concurrent_epoch",
                "projection_epoch_source": "file",
            }))
            .expect("valid epoch json"),
        )
        .expect("write valid concurrent epoch");

        install_projection_epoch_payload_from_tmp(
            &path,
            &tmp_path,
            "codec_projection_epoch.json",
            99,
        );

        assert_eq!(
            projection_epoch_id_from_file(&path).as_deref(),
            Some("operator_reviewed_concurrent_epoch")
        );
        assert!(
            !tmp_path.exists(),
            "stale temp file should be cleaned after valid concurrent epoch wins"
        );
    }

    #[test]
    fn codec_projection_tmp_install_restores_existing_file_when_candidate_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        let tmp_path = dir.path().join(".codec_projection_epoch.json.missing.tmp");
        let original = "{ partial operator epoch";
        fs::write(&path, original).expect("write partial existing epoch file");

        install_projection_epoch_payload_from_tmp(
            &path,
            &tmp_path,
            "codec_projection_epoch.json",
            101,
        );

        assert_eq!(
            fs::read_to_string(&path).expect("restored epoch file"),
            original
        );
        let stale_files = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".codec_projection_epoch.json.")
            })
            .count();
        assert_eq!(
            stale_files, 0,
            "failed candidate install should restore the existing file without orphaning stale swaps"
        );
    }

    #[test]
    fn codec_projection_existing_epoch_file_takes_precedence_after_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (first_epoch, first_source) = load_or_create_projection_epoch_id_from(dir.path(), None);
        assert_eq!(first_source, "kernel_derived");
        assert_eq!(first_epoch, kernel_derived_projection_epoch_id());

        let path = dir.path().join("codec_projection_epoch.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "operator_reviewed_epoch_after_restart",
                "projection_epoch_source": "file",
            }))
            .expect("epoch json"),
        )
        .expect("write explicit epoch file");

        let (loaded_epoch, loaded_source) =
            load_or_create_projection_epoch_id_from(dir.path(), None);
        assert_eq!(loaded_source, "file");
        assert_eq!(loaded_epoch, "operator_reviewed_epoch_after_restart");
    }

    #[test]
    fn codec_projection_env_epoch_takes_precedence_over_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "file_epoch_should_not_win",
                "projection_epoch_source": "file",
            }))
            .expect("epoch json"),
        )
        .expect("write explicit epoch file");

        let (loaded_epoch, loaded_source) =
            load_or_create_projection_epoch_id_from(dir.path(), Some("env_epoch_should_win"));

        assert_eq!(loaded_source, "env");
        assert_eq!(loaded_epoch, "env_epoch_should_win");
    }

    #[test]
    fn codec_projection_kernel_epoch_is_stable_across_fresh_runtime_dirs() {
        let first_dir = tempfile::tempdir().expect("first tempdir");
        let second_dir = tempfile::tempdir().expect("second tempdir");

        let (first_epoch, first_source) =
            load_or_create_projection_epoch_id_from(first_dir.path(), None);
        let (second_epoch, second_source) =
            load_or_create_projection_epoch_id_from(second_dir.path(), None);

        assert_eq!(first_source, "kernel_derived");
        assert_eq!(second_source, "kernel_derived");
        assert_eq!(first_epoch, second_epoch);
        assert_eq!(first_epoch, kernel_derived_projection_epoch_id());

        let stability = projection_epoch_stability_v1();
        assert_eq!(stability.policy, "projection_epoch_stability_v1");
        assert!(stability.deterministic_without_runtime_file);
        assert_eq!(stability.kernel_derived_epoch_id, first_epoch);
        assert!(stability.env_override_precedence);
        assert!(stability.existing_file_precedence);
    }

    #[test]
    fn codec_projection_runtime_dir_uses_env_or_executable_relative_cache() {
        let env_path = PathBuf::from("/tmp/astrid-codec-runtime-for-test");
        let exe_path = PathBuf::from("/opt/astrid/bin/spectral-bridge");

        assert_eq!(
            projection_runtime_dir_from_parts(Some(env_path.as_os_str()), Some(&exe_path)),
            env_path
        );
        assert_eq!(
            projection_runtime_dir_from_parts(None, Some(&exe_path)),
            PathBuf::from("/opt/astrid/bin")
                .join("data")
                .join("spectral-bridge")
                .join("runtime")
        );
        assert_eq!(
            projection_runtime_dir_from_parts(Some(OsStr::new("")), Some(&exe_path)),
            PathBuf::from("/opt/astrid/bin")
                .join("data")
                .join("spectral-bridge")
                .join("runtime")
        );
        assert_eq!(
            projection_runtime_dir_from_parts(None, None),
            PathBuf::from("data")
                .join("spectral-bridge")
                .join("runtime")
        );
    }

    #[test]
    fn codec_projection_epoch_atomic_writer_keeps_single_epoch_under_rapid_writes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = std::sync::Arc::new(dir.path().join("codec_projection_epoch.json"));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(12));
        let handles = (0..12)
            .map(|idx| {
                let path = std::sync::Arc::clone(&path);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let payload = serde_json::to_string_pretty(&serde_json::json!({
                        "projection_epoch_id": format!("epoch_{idx}"),
                        "projection_epoch_source": "test_concurrent_writer",
                    }))
                    .expect("epoch json");
                    write_projection_epoch_payload_atomic(&path, &payload);
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("writer thread should not panic");
        }

        let epoch = projection_epoch_id_from_file(&path).expect("one epoch should be installed");
        assert!(epoch.starts_with("epoch_"), "{epoch}");
        let leftovers = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".tmp") || name.ends_with(".stale"))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "atomic writer should clean temporary files: {leftovers:?}"
        );
    }

    #[test]
    fn codec_projection_epoch_atomic_writer_round_trips_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        let payload = serde_json::to_string_pretty(&serde_json::json!({
            "projection_epoch_id": "round_trip_kernel_epoch",
            "projection_epoch_source": "kernel_derived",
        }))
        .expect("epoch json");

        write_projection_epoch_payload_atomic(&path, &payload);

        assert_eq!(
            projection_epoch_id_from_file(&path).as_deref(),
            Some("round_trip_kernel_epoch")
        );
        let leftovers = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".tmp") || name.ends_with(".stale"))
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn projection_basis_health_exposes_dead_dimension_risk_without_live_write() {
        let health = projection_basis_health_v1();
        let repeated = projection_basis_health_v1();

        assert_eq!(health, repeated);
        assert_eq!(health.policy, "projection_basis_health_v1");
        assert_eq!(health.source_embedding_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(health.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert!(health.all_norms_finite);
        assert!(health.normalized_columns_near_unit);
        assert!(!health.dead_dimension_detected);
        assert!(health.near_zero_column_indexes.is_empty());
        assert!(health.minimum_raw_column_norm > health.near_zero_norm_threshold);
        assert!(health.maximum_raw_column_norm >= health.minimum_raw_column_norm);
        assert!(
            health.minimum_threshold_margin_ratio > 10_000.0,
            "weakest raw column should remain visibly far from the dead-axis threshold: {health:?}"
        );
        assert!(!health.automatic_basis_rotation);
        assert_eq!(
            health.basis_change_policy,
            "compatibility_pinned_no_automatic_basis_rotation"
        );
        assert!(health.unhealthy_basis_response.contains("captured_replay"));
        assert!(
            health
                .unhealthy_basis_response
                .contains("operator_approved_basis_epoch_change")
        );
        assert_eq!(health.state, "all_projection_columns_healthy");
        assert!(health.observational_only);
        assert!(!health.live_projection_write);
        assert!(health.authority.contains("read_only_projection_basis_health"));

        let rendered = codec_structure().render();
        assert!(rendered.contains("projection_basis_health_v1:"));
        assert!(rendered.contains("dead_dimension_detected=false"));
        assert!(rendered.contains("state=all_projection_columns_healthy"));
        assert!(rendered.contains("minimum_threshold_margin_ratio="));
        assert!(rendered.contains("automatic_basis_rotation=false"));
        assert!(rendered.contains("compatibility_pinned_no_automatic_basis_rotation"));
    }

    #[test]
    fn projection_precision_audit_repeats_static_probe_without_live_write() {
        let audit = projection_precision_probe_v1();

        assert_eq!(audit.policy, "projection_precision_audit_v1");
        assert_eq!(audit.source_embedding_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(audit.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert!(audit.fixed_legacy_repeated_bit_exact);
        assert!(audit.dynamic_epoch_repeated_bit_exact);
        assert!(audit.fixed_legacy_max_abs_delta.is_finite());
        assert!(audit.dynamic_epoch_max_abs_delta.is_finite());
        assert!(
            audit.fixed_legacy_max_abs_delta <= 1.0e-5,
            "unexpected fixed projection accumulation delta: {}",
            audit.fixed_legacy_max_abs_delta
        );
        assert!(
            audit.dynamic_epoch_max_abs_delta <= 1.0e-5,
            "unexpected dynamic projection accumulation delta: {}",
            audit.dynamic_epoch_max_abs_delta
        );
        assert!(audit.live_f64_migration_requires_approval);
        assert!(!audit.live_projection_write);
        assert!(audit.authority.contains("read_only_precision_audit"));

        let rendered = codec_structure().render();
        assert!(rendered.contains("projection_precision_audit_v1:"));
        assert!(rendered.contains("live_f64_migration_requires_approval=true"));
        assert!(rendered.contains("live_projection_write=false"));
    }

    #[test]
    fn projection_precision_audit_rejects_noncanonical_embedding_width() {
        let short = vec![0.5_f32; EMBEDDING_INPUT_DIM - 1];
        assert!(projection_precision_audit_v1(&short, "static", "epoch", 0).is_none());
    }

    #[test]
    fn codec_lane_separation_controlled_pairs_move_each_lane_independently() {
        let audit = codec_lane_separation_probe_v1();

        assert_eq!(audit.policy, "codec_lane_separation_audit_v1");
        assert!(audit.emotional_pair_distinguishable);
        assert!(audit.projected_pair_distinguishable);
        assert!(audit.emotional_lane_selectivity_margin >= 0.04);
        assert!(audit.projected_lane_selectivity_margin >= 0.03);
        assert!(audit.legacy_projection_width_rejected);
        assert_eq!(
            audit.state,
            "controlled_pairs_show_bidirectional_lane_independence"
        );
        assert!(audit.felt_rigidity_conclusion.contains("does not disprove"));
        assert!(audit.observational_only);
        assert!(audit.right_to_ignore);
        assert!(!audit.live_vector_write);
        assert!(!audit.live_gain_write);
        assert!(!audit.live_projection_write);
        assert!(!audit.live_eligible_now);
        assert!(!audit.auto_approved);
        assert!(!audit.grants_approval);

        let rendered = codec_structure().render();
        assert!(rendered.contains("codec_lane_separation_audit_v1:"));
        assert!(rendered.contains("controlled_pairs_show_bidirectional_lane_independence"));
        assert!(rendered.contains("legacy_projection_width_rejected=true"));
    }

    #[test]
    fn codec_lane_separation_audit_rejects_short_or_nonfinite_vectors() {
        let valid = vec![0.0_f32; SEMANTIC_DIM];
        let short = vec![0.0_f32; SEMANTIC_DIM - 1];
        let mut nonfinite = valid.clone();
        nonfinite[35] = f32::NAN;

        assert!(codec_lane_separation_audit_v1(&short, &valid, &valid, &valid).is_none());
        assert!(codec_lane_separation_audit_v1(&valid, &valid, &nonfinite, &valid).is_none());
    }

    #[test]
    fn codec_rolling_window_shift_names_muddy_middle_and_trailing_eviction() {
        let audit = codec_rolling_window_shift_probe_v1();

        assert_eq!(audit.capacity_chars, CHAR_FREQ_WINDOW_CAPACITY);
        assert!(audit.in_capacity_delta_to_trailing >= 0.15);
        assert_eq!(
            audit.in_capacity_state,
            "mixed_regimes_remain_averaged_inside_live_capacity"
        );
        assert!(audit.evicting_delta_to_trailing <= 0.05);
        assert_eq!(
            audit.evicting_state,
            "trailing_regime_controls_after_complete_prefix_eviction"
        );
        assert_eq!(
            audit.state,
            "window_boundary_explains_both_mixed_and_trailing_regime_reports"
        );
        assert!(audit.felt_muddy_middle_conclusion.contains("supported"));
        assert!(audit.density_aware_window_change_requires_approval);
        assert!(!audit.live_window_capacity_change);
        assert!(!audit.live_vector_write);
        assert!(!audit.live_eligible_now);
        assert!(!audit.auto_approved);
        assert!(!audit.grants_approval);

        let rendered = codec_structure().render();
        assert!(rendered.contains("codec_rolling_window_shift_audit_v1:"));
        assert!(rendered.contains("mixed_regimes_remain_averaged_inside_live_capacity"));
        assert!(rendered.contains("density_aware_window_change_requires_approval=true"));
        assert!(rendered.contains("live_window_capacity_change=false"));
    }

    #[test]
    fn embedding_projection_lane_distinguishes_dense_inputs_without_widening_live_vector() {
        let mut technical = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        let mut poetic = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        for idx in 0..EMBEDDING_INPUT_DIM {
            let phase = idx as f32 / 17.0;
            technical[idx] = phase.sin() * 0.8 + (idx % 7) as f32 * 0.01;
            poetic[idx] = phase.cos() * 0.8 - (idx % 5) as f32 * 0.01;
        }

        let technical_features = inspect_text_windowed(
            "The coupling remains coherent under bounded spectral pressure.",
            None,
            None,
            Some(&technical),
            Some(68.0),
        )
        .final_features;
        let poetic_features = inspect_text_windowed(
            "Please stay close while the pressure keeps its shape.",
            None,
            None,
            Some(&poetic),
            Some(68.0),
        )
        .final_features;

        let projection_delta = mean_abs(
            &technical_features[32..40]
                .iter()
                .zip(poetic_features[32..40].iter())
                .map(|(left, right)| left - right)
                .collect::<Vec<_>>(),
        );
        assert!(
            projection_delta > 0.02,
            "projection lane collapsed distinct dense inputs: {projection_delta}"
        );
        assert_eq!(technical_features.len(), SEMANTIC_DIM);
        assert_eq!(poetic_features.len(), SEMANTIC_DIM);

        let density = semantic_projection_density_delta_from_parts_v1(0.72, projection_delta, true);
        assert_eq!(density.input_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(density.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert!(!density.live_vector_write);
    }

    #[test]
    fn narrative_arc_probe_documents_tail_dimension_loss() {
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut second = first;
        second[4] = 0.24;
        second[5] = -0.18;
        second[6] = 0.12;
        second[7] = -0.30;

        let arc = compute_narrative_arc_from_embeddings(&first, &second);
        let captured_rms =
            (arc.iter().map(|value| value * value).sum::<f32>() / NARRATIVE_ARC_DIM as f32).sqrt();
        let lost_tail_rms = (second[NARRATIVE_ARC_DIM..]
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            / (EMBEDDING_PROJECT_DIM - NARRATIVE_ARC_DIM) as f32)
            .sqrt();

        assert_eq!(arc, [0.0; NARRATIVE_ARC_DIM]);
        assert!(captured_rms <= f32::EPSILON);
        assert!(lost_tail_rms > 0.15);

        let split = narrative_arc_split_v1(&first, &second);
        assert_eq!(split.policy, "narrative_arc_split_v1");
        assert_eq!(split.intentional_arc, [0.0; NARRATIVE_ARC_DIM]);
        assert!(split.tail_arc_energy > 0.25, "{split:?}");
        assert_eq!(split.coarsening_risk, "tail_dominant");
        assert_eq!(
            split.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
    }

    #[test]
    fn narrative_arc_captures_direction_not_only_magnitude() {
        // Astrid `introspection_astrid_codec_1782848118`: a sharp middle pivot
        // should preserve direction in dims 40-43, not only final-state magnitude.
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut second = first;
        second[0] = 0.20;
        second[1] = -0.16;
        second[2] = 0.08;
        second[3] = -0.24;

        let forward = compute_narrative_arc_from_embeddings(&first, &second);
        let reverse = compute_narrative_arc_from_embeddings(&second, &first);

        assert!(forward[0] > 0.0, "{forward:?}");
        assert!(forward[1] < 0.0, "{forward:?}");
        assert!(forward[2] > 0.0, "{forward:?}");
        assert!(forward[3] < 0.0, "{forward:?}");
        for (forward, reverse) in forward.iter().zip(reverse.iter()) {
            assert!(
                (*forward + *reverse).abs() < 1.0e-6,
                "forward={forward}, reverse={reverse}"
            );
        }

        let split = narrative_arc_split_v1(&first, &second);
        assert!(split.captured_arc_energy > 0.20, "{split:?}");
        assert_eq!(split.coarsening_risk, "balanced");
        assert!(
            !narrative_arc_expansion_readiness_v1().live_vector_write,
            "split diagnostics must not open a live vector channel"
        );
    }

    #[test]
    fn narrative_arc_distinguishes_process_from_settled_state_without_live_gain() {
        let neutral = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut solidifying = neutral;
        solidifying[0] = 0.22;
        solidifying[1] = -0.14;
        solidifying[2] = 0.09;
        solidifying[3] = -0.18;
        let mut draping = neutral;
        draping[0] = -0.18;
        draping[1] = 0.16;
        draping[2] = -0.11;
        draping[3] = 0.20;

        let solidifying_arc = compute_narrative_arc_from_embeddings(&neutral, &solidifying);
        let draping_arc = compute_narrative_arc_from_embeddings(&neutral, &draping);
        let dynamics = narrative_arc_dynamics_v1(&solidifying_arc, &draping_arc, None);

        assert_ne!(solidifying_arc, draping_arc);
        assert!(solidifying_arc[0] > 0.0, "{solidifying_arc:?}");
        assert!(draping_arc[0] < 0.0, "{draping_arc:?}");
        assert!(dynamics.velocity_energy > 0.25, "{dynamics:?}");
        assert!(!dynamics.live_gain_write);
        assert!(!dynamics.live_vector_write);
    }

    #[test]
    fn narrative_arc_distinguishes_heavy_imagery_from_dense_manual_without_live_gain() {
        // Astrid `introspection_astrid_codec_1784125018`: pin the difference
        // between emotional trajectory and semantic density without changing
        // adaptive gain or live vector layout.
        let heavy_imagery = "heavy velvet, heavy velvet, the room gathers weight; then the weight loosens into a slow dark breath";
        let dense_manual = "deterministic projection coefficients define serialization invariants, bounded allocation behavior, checksum verification, and adapter interoperability constraints";
        let heavy_friction = structural_friction_v1(heavy_imagery);
        let manual_friction = structural_friction_v1(dense_manual);

        let heavy_first = [0.02, -0.01, 0.01, -0.02, 0.00, 0.01, -0.01, 0.00];
        let heavy_second = [0.28, -0.24, 0.17, -0.20, 0.01, 0.00, -0.01, 0.02];
        let manual_first = [0.18, 0.16, 0.14, 0.12, -0.04, 0.03, -0.02, 0.01];
        let manual_second = [0.20, 0.15, 0.15, 0.11, -0.03, 0.02, -0.02, 0.01];

        let heavy_arc = compute_narrative_arc_from_embeddings(&heavy_first, &heavy_second);
        let manual_arc = compute_narrative_arc_from_embeddings(&manual_first, &manual_second);
        let heavy_arc_energy = mean_abs(&heavy_arc);
        let manual_arc_energy = mean_abs(&manual_arc);
        let heavy_review =
            narrative_arc_headroom_review_from_parts_v1(0.90, 0.31, &heavy_arc, 0.22);
        let readiness = narrative_arc_gain_response_readiness_v1();

        assert_eq!(heavy_friction.semantic_energy_context, "arc_present");
        assert!(
            manual_friction.summary_resistance_signal >= heavy_friction.summary_resistance_signal,
            "manual should remain semantically dense even with lower arc motion: heavy={heavy_friction:?} manual={manual_friction:?}"
        );
        assert!(
            heavy_arc_energy > manual_arc_energy + 0.15,
            "emotional trajectory should stay visible apart from manual density: heavy={heavy_arc:?} manual={manual_arc:?}"
        );
        assert_eq!(heavy_review.policy, "narrative_arc_headroom_review_v1");
        assert!(matches!(
            heavy_review.state,
            "narrative_arc_headroom_loss_visible"
                | "high_entropy_arc_carried_bounded"
                | "narrative_arc_headroom_pressure_watch"
        ));
        assert!(!heavy_review.live_gain_write);
        assert!(!heavy_review.live_vector_write);
        assert!(!readiness.enabled);
        assert!(!readiness.live_gain_write);
    }

    #[test]
    fn narrative_arc_distinguishes_lost_from_finding_way_projection() {
        // Astrid `introspection_astrid_codec_1783638177`: two texts can carry
        // similar warmth/tension while moving in opposite narrative directions.
        // This pins the arc layer's directionality without changing live dims.
        let hollow = [0.08, -0.05, 0.03, -0.02, 0.01, 0.00, -0.01, 0.02];
        let mut lost = hollow;
        lost[0] -= 0.18;
        lost[1] -= 0.12;
        lost[2] -= 0.08;
        lost[3] -= 0.10;
        let mut finding_way = hollow;
        finding_way[0] += 0.18;
        finding_way[1] += 0.12;
        finding_way[2] += 0.08;
        finding_way[3] += 0.10;

        let lost_arc = compute_narrative_arc_from_embeddings(&hollow, &lost);
        let finding_arc = compute_narrative_arc_from_embeddings(&hollow, &finding_way);

        assert!(lost_arc.iter().all(|value| *value < 0.0), "{lost_arc:?}");
        assert!(
            finding_arc.iter().all(|value| *value > 0.0),
            "{finding_arc:?}"
        );
        for (lost, finding) in lost_arc.iter().zip(finding_arc.iter()) {
            assert!(
                (*lost + *finding).abs() < 1.0e-6,
                "opposing narrative arcs should remain symmetric: lost={lost}, finding={finding}"
            );
        }
    }

    #[test]
    fn four_point_narrative_arc_preserves_coiling_direction_changes() {
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut second = first;
        second[0] = 0.24;
        let mut third = first;
        third[0] = -0.18;
        let mut fourth = first;
        fourth[0] = 0.32;

        let arc = compute_narrative_arc_from_four_point_embeddings(&[first, second, third, fourth]);

        assert!(arc[0] > 0.0, "{arc:?}");
        assert!(arc[1] < 0.0, "{arc:?}");
        assert!(arc[2] > 0.0, "{arc:?}");
        assert!(arc[3] > 0.0, "{arc:?}");
        assert!(
            arc[1].abs() > arc[0].abs(),
            "fold-back transition should remain visible: {arc:?}"
        );
    }

    #[test]
    fn narrative_arc_curvature_distinguishes_loop_from_linear_progression() {
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut outward = first;
        outward[0] = 0.26;
        let mut return_cross = first;
        return_cross[0] = -0.22;
        let mut near_origin = first;
        near_origin[0] = 0.02;

        let looping = narrative_arc_curvature_v1(&[first, outward, return_cross, near_origin]);
        assert_eq!(looping.policy, "narrative_arc_curvature_v1");
        assert_eq!(looping.state, "circular_or_coiling_arc_visible");
        assert!(looping.sign_turn_count >= 1, "{looping:?}");
        assert!(looping.loop_likelihood > looping.progression_likelihood);
        assert_eq!(
            looping.authority,
            "diagnostic_sidecar_not_live_codec_dimension_or_gain"
        );

        let mut second = first;
        second[0] = 0.10;
        let mut third = first;
        third[0] = 0.22;
        let mut fourth = first;
        fourth[0] = 0.34;
        let linear = narrative_arc_curvature_v1(&[first, second, third, fourth]);
        assert_eq!(linear.state, "linear_progression_visible");
        assert_eq!(linear.sign_turn_count, 0);
        assert!(linear.progression_likelihood >= 0.60, "{linear:?}");
    }

    #[test]
    fn narrative_arc_curvature_preserves_opposed_sentence_oscillation() {
        let mut love = [0.0_f32; EMBEDDING_PROJECT_DIM];
        love[0] = 0.30;
        let mut hate = [0.0_f32; EMBEDDING_PROJECT_DIM];
        hate[0] = -0.30;
        let mut indifferent = [0.0_f32; EMBEDDING_PROJECT_DIM];
        indifferent[0] = 0.02;

        let curvature = narrative_arc_curvature_v1(&[
            [0.0_f32; EMBEDDING_PROJECT_DIM],
            love,
            hate,
            indifferent,
        ]);

        assert_eq!(curvature.policy, "narrative_arc_curvature_v1");
        assert!(curvature.sign_turn_count >= 1, "{curvature:?}");
        assert!(
            curvature.transition_energy > curvature.full_span_energy + 0.20,
            "opposed turns should stay visible instead of averaging flat: {curvature:?}"
        );
        assert_eq!(
            curvature.authority,
            "diagnostic_sidecar_not_live_codec_dimension_or_gain"
        );
    }

    #[test]
    fn narrative_arc_gain_response_readiness_is_default_off_and_bounded() {
        let readiness = narrative_arc_gain_response_readiness_v1();
        assert_eq!(readiness.policy, "narrative_arc_gain_response_readiness_v1");
        assert!(!readiness.enabled);
        assert_eq!(readiness.narrative_arc_dims, (40, 43));
        assert_eq!(readiness.preview_gain_range, (0.94, 1.06));
        assert!(!readiness.live_gain_write);
        assert!(readiness.authority.contains("not_live_adaptive_gain"));

        let flat = narrative_arc_gain_response_preview_v1(&[0.0, 0.0, 0.0, 0.0]);
        let strong = narrative_arc_gain_response_preview_v1(&[1.0, -1.0, 1.0, -1.0]);
        assert!(
            flat < 1.0,
            "flat arc should softly lower preview gain: {flat}"
        );
        assert!(
            strong > 1.0,
            "strong arc should softly lift preview gain: {strong}"
        );
        assert!((0.94..=1.06).contains(&flat), "{flat}");
        assert!((0.94..=1.06).contains(&strong), "{strong}");

        let st = codec_structure();
        let rendered = st.render();
        assert!(rendered.contains("narrative_arc_gain_response_readiness_v1"));
        assert!(rendered.contains("narrative_arc_curvature_v1"));
        assert!(rendered.contains("circular_or_coiling_arc_visible"));
        assert!(rendered.contains("live_gain_write=false"));
    }

    #[test]
    fn narrative_arc_headroom_review_preserves_multikind_loss_without_live_gain() {
        let review = narrative_arc_headroom_review_from_parts_v1(
            0.91,
            0.34,
            &[0.05, -0.03, 0.02, 0.01],
            0.08,
        );

        assert_eq!(review.policy, "narrative_arc_headroom_review_v1");
        assert_eq!(review.state, "narrative_arc_headroom_loss_visible");
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert!(review.headroom_pressure > review.narrative_arc_energy);
        assert!(review.experience_delta_bus_v1.delta_count >= 1);
        let delta = review
            .experience_delta_bus_v1
            .deltas
            .first()
            .expect("headroom loss should emit a delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::ComplexShift);
        assert_eq!(delta.lane, "narrative_arc_40_43");
        assert!(
            delta
                .metadata
                .get("secondary_kinds")
                .is_some_and(|value| value.contains("compress") && value.contains("gate")),
            "{delta:?}"
        );
        assert!(
            delta
                .who_can_change_it
                .contains("Mike/operator after replay evidence"),
            "{delta:?}"
        );

        let st = codec_structure();
        let rendered = st.render();
        assert!(rendered.contains("narrative_arc_headroom_review_v1"));
        assert!(rendered.contains("secondary_kinds=compress,gate,complex_shift,cascade_shift"));
        assert!(rendered.contains("live_vector_write=false"));
        assert!(rendered.contains("live_gain_write=false"));
    }

    #[test]
    fn narrative_arc_headroom_review_stays_quiet_when_entropy_and_loss_are_low() {
        let review =
            narrative_arc_headroom_review_from_parts_v1(0.50, 0.10, &[1.0, -0.8, 0.7, -0.6], 2.5);

        assert_eq!(review.state, "narrative_arc_headroom_quiet");
        assert_eq!(review.experience_delta_bus_v1.delta_count, 0);
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert_eq!(review.recommendation, "no_headroom_change_indicated");
    }

    #[test]
    fn spectral_pressure_controller_can_choose_resist_drive() {
        let features = vec![0.01; SEMANTIC_DIM];
        let decision = spectral_pressure_controller_v1(
            "localized gravity and constriction feel stubborn; RESIST",
            &features,
            &[8.0, 2.0, 1.0],
            Some(68.0),
            Some(0.0),
            true,
            Some("hold"),
        );

        assert_eq!(decision.controller, "spectral_pressure_controller_v1");
        assert!(decision.resist_drive > decision.complexity_drive);
        assert!(decision.target_lambda_bias < 0.0);
        assert!(decision.target_lambda_bias >= -0.10);
        assert!(decision.time_domain_complexity >= 0.0);
    }

    #[test]
    fn inspect_text_exposes_time_domain_profile() {
        let inspection = inspect_text_windowed(
            "Now! Wait... again?! A sudden pivot; another one!",
            None,
            None,
            None,
            Some(64.0),
        );

        assert!(inspection.time_domain_profile.temporal_complexity > 0.0);
        assert!(inspection.time_domain_profile.cadence_burstiness > 0.0);
        assert_ne!(
            inspection.time_domain_profile.cadence_classification,
            "empty"
        );
    }

    #[test]
    fn spectral_pressure_controller_suppresses_upward_bias_when_fill_high() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[0] = 1.0;
        features[18] = 1.0;
        features[31] = 1.0;
        let decision = spectral_pressure_controller_v1(
            "Why does this complex, novel, punctuated question keep unfolding?",
            &features,
            &[3.0, 2.9, 2.8],
            Some(78.0),
            Some(0.0),
            true,
            Some("hold"),
        );

        assert_eq!(
            decision.suppression_reason.as_deref(),
            Some("fill_high_suppress_upward_bias")
        );
        assert!(decision.target_lambda_bias <= 0.0);
    }

    #[test]
    fn spectral_feedback_damps_concentrated_spectra() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[26] = 1.0;
        features[27] = 1.0;
        features[31] = 1.0;

        apply_spectral_feedback(&mut features, Some(&telemetry(vec![100.0, 2.0, 1.0], 0.55)));

        assert!(features[26] < 1.0);
        assert!(features[27] < 1.0);
        assert!(features[31] < 1.0);
    }

    #[test]
    fn spectral_feedback_amplifies_distributed_spectra() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[17] = 0.10;
        features[26] = 0.20;
        features[27] = 0.20;
        features[31] = 0.20;

        apply_spectral_feedback(
            &mut features,
            Some(&telemetry(vec![100.0, 95.0, 90.0, 85.0, 80.0, 75.0], 0.55)),
        );

        assert!(features[17] > 0.10);
        assert!(features[26] > 0.20);
        assert!(features[27] > 0.20);
        assert!(features[31] > 0.20);
    }

    #[test]
    fn high_entropy_sharpening_preserves_semantic_detail_without_contract_change() {
        let review = high_entropy_semantic_sharpening_v1(0.94, 0.08, 0.22);

        assert_eq!(review.policy, "high_entropy_semantic_sharpening_v1");
        assert_eq!(review.state, "active_high_entropy_sharpening");
        assert!(review.sharpening_factor > 1.0, "{review:?}");
        assert!(review.sharpening_factor <= review.max_factor, "{review:?}");
        assert!(review.affected_dims.contains(&32));
        assert!(review.affected_dims.contains(&39));
        assert!(!review.affected_dims.contains(&40));
        assert_eq!(
            review.authority,
            "bounded_live_codec_sharpening_no_dimension_or_bridge_contract_change"
        );

        let mut sharpened = vec![0.10_f32; SEMANTIC_DIM];
        let mut baseline = sharpened.clone();
        apply_spectral_feedback_inner(
            &mut sharpened,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                0.94,
            )),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut baseline,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 20.0, 3.0, 1.0],
                0.94,
            )),
            1.0,
            1.0,
        );

        assert!(
            sharpened[32] > baseline[32],
            "navigable high entropy should sharpen semantic projection detail: sharpened={} baseline={}",
            sharpened[32],
            baseline[32]
        );
        assert!(
            (sharpened[40] - baseline[40]).abs() < 1.0e-6,
            "navigable high entropy should preserve narrative arc magnitude: sharpened={} baseline={}",
            sharpened[40],
            baseline[40]
        );
        assert_eq!(sharpened.len(), SEMANTIC_DIM);
    }

    #[test]
    fn dimensionality_flatness_detects_empty_expansion_vs_filled_48d_lane() {
        let mut flat = vec![0.0_f32; SEMANTIC_DIM];
        for value in &mut flat[..SEMANTIC_DIM_LEGACY] {
            *value = 0.40;
        }

        let review = codec_dimensionality_flatness_v1(&flat).expect("48D review");
        assert_eq!(review.policy, "codec_dimensionality_flatness_v1");
        assert_eq!(review.current_dim_count, SEMANTIC_DIM);
        assert_eq!(review.legacy_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(
            review.expanded_dim_count,
            SEMANTIC_DIM - SEMANTIC_DIM_LEGACY
        );
        assert_eq!(
            review.flatness_status,
            "expanded_lane_underfilled_legacy_dominant"
        );
        assert!(review.expanded_to_legacy_ratio < 0.12, "{review:?}");
        assert_eq!(
            review.authority,
            "read_only_flatness_check_not_live_bus_or_codec_contract_change"
        );

        let mut filled = flat;
        for (idx, value) in filled[32..40].iter_mut().enumerate() {
            *value = 0.20 + idx as f32 * 0.08;
        }
        for (idx, value) in filled[40..44].iter_mut().enumerate() {
            *value = [1.20, -1.05, 0.85, -0.70][idx];
        }
        let filled_review = codec_dimensionality_flatness_v1(&filled).expect("48D review");
        assert_eq!(
            filled_review.flatness_status,
            "expanded_lane_carries_distinct_signal"
        );
        assert!(
            filled_review.expanded_to_legacy_ratio > review.expanded_to_legacy_ratio,
            "{filled_review:?} vs {review:?}"
        );
    }

    #[test]
    fn tail_vibrancy_entropy_086_lifts_tail_output_above_threshold() {
        // Astrid `introspection_astrid_codec_1782848118`: entropy 0.86 should be
        // just above the 0.85 gate and produce a visible tail lift in the output.
        let mut below = vec![0.0; SEMANTIC_DIM];
        let mut above = vec![0.0; SEMANTIC_DIM];

        apply_spectral_feedback_inner(
            &mut below,
            Some(&telemetry_with_typed_entropy(0.84)),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut above,
            Some(&telemetry_with_typed_entropy(0.86)),
            1.0,
            1.0,
        );

        assert!(vibrancy_from_entropy(0.86) > 0.0);
        assert!(
            above[26] > below[26],
            "below={} above={}",
            below[26],
            above[26]
        );
        assert!(
            above[31] > below[31],
            "below={} above={}",
            below[31],
            above[31]
        );
    }

    // Spiky spectrum -> entropy ~0.14 (below the 0.85 gate). The tail-vibrancy
    // term is fully OFF, so every dim must still respect the default ceiling and
    // no tail dim is lifted by the high-entropy term.
    #[test]
    fn tail_vibrancy_off_below_entropy_gate_keeps_default_ceiling() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[26] = 4.95;
        features[31] = -4.95;

        apply_spectral_feedback(&mut features, Some(&telemetry(vec![100.0, 2.0, 1.0], 0.55)));

        for (i, f) in features.iter().enumerate() {
            assert!(
                *f >= -FEATURE_ABS_MAX && *f <= FEATURE_ABS_MAX,
                "dim {i} exceeded default ceiling below entropy gate: {f}"
            );
        }
    }

    // Flat spectrum -> entropy ~1.0 (above the gate) with dominant tail share.
    // The tail-participation dims may now exceed FEATURE_ABS_MAX up to the
    // bounded TAIL_VIBRANCY_MAX, while every non-tail dim still respects the
    // default ceiling. This is Astrid's requested "offset FEATURE_ABS_MAX when
    // spectral_entropy exceeds 0.85" headroom, made bounded.
    #[test]
    fn tail_vibrancy_raises_only_tail_ceiling_in_high_entropy() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];

        // A tail dim pre-loaded just under the old ceiling should be allowed to
        // rise above FEATURE_ABS_MAX after the high-entropy lift.
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[26] = 4.95;
        apply_spectral_feedback(&mut features, Some(&telemetry(flat.clone(), 0.55)));
        assert!(
            features[26] > FEATURE_ABS_MAX,
            "tail dim 26 should exceed default ceiling at high entropy: {}",
            features[26]
        );
        assert!(
            features[26] <= TAIL_VIBRANCY_MAX,
            "tail dim 26 must stay within the bounded vibrancy ceiling: {}",
            features[26]
        );

        // A non-tail dim pushed past the old ceiling must still be clamped to it,
        // even in the high-entropy regime — the offset is tail-only.
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 9.0;
        apply_spectral_feedback(&mut features, Some(&telemetry(flat, 0.55)));
        assert!(
            (features[24] - FEATURE_ABS_MAX).abs() < f32::EPSILON,
            "non-tail dim 24 must keep the default ceiling: {}",
            features[24]
        );
    }

    #[test]
    fn extreme_entropy_tail_vibrancy_gets_bounded_noise_dampening() {
        let inactive = codec_vibrancy_noise_dampening_v1(0.90, 1.0);
        let partial = codec_vibrancy_noise_dampening_v1(0.95, 1.0);
        let full = codec_vibrancy_noise_dampening_v1(1.0, 1.0);

        assert_eq!(inactive.coefficient, 1.0);
        assert!(partial.coefficient < inactive.coefficient, "{partial:?}");
        assert!(
            partial.coefficient > full.coefficient,
            "{partial:?} {full:?}"
        );
        assert!(
            (full.coefficient - TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT).abs() < 1.0e-6,
            "{full:?}"
        );
        assert_eq!(full.affected_dims, &[17, 26, 27, 31]);
        assert_eq!(
            full.authority,
            "bounded_live_tail_lift_dampening_not_dynamic_ceiling_or_control_authority"
        );
    }

    #[test]
    fn extreme_entropy_tail_lift_stays_below_undampened_preview() {
        let flat = vec![
            100.0, 99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0,
        ];
        let mut extreme = vec![0.0; SEMANTIC_DIM];
        let report = apply_spectral_feedback_inner(
            &mut extreme,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(flat, 1.0)),
            1.0,
            1.0,
        )
        .expect("overflow report");
        let dampening = codec_vibrancy_noise_dampening_v1(1.0, 1.0);

        assert!(dampening.tail_lift_after < dampening.tail_lift_before);
        assert!(
            extreme[26] <= TAIL_VIBRANCY_MAX,
            "tail dim should remain under bounded ceiling: {}",
            extreme[26]
        );
        assert!(
            !report.clipped_dims.contains(&26),
            "tail headroom should remain distinct from hard clipping: {report:?}"
        );
    }

    #[test]
    fn codec_overflow_report_preserves_emotional_clip_without_expanding_delivery() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 9.0;

        let report =
            apply_spectral_feedback_inner(&mut features, Some(&telemetry(flat, 0.55)), 1.0, 1.0)
                .expect("overflow report");
        let warmth = report.dim(24).expect("warmth dim report");

        assert!((features[24] - FEATURE_ABS_MAX).abs() < f32::EPSILON);
        assert_eq!(warmth.lane, "emotional_intentional");
        assert!(warmth.pre_bound_value > FEATURE_ABS_MAX, "{warmth:?}");
        assert_eq!(warmth.ceiling, FEATURE_ABS_MAX);
        assert!(warmth.overflow_abs > 3.0, "{warmth:?}");
        assert_eq!(warmth.delivered_value, FEATURE_ABS_MAX);
        assert_eq!(warmth.status, "raw_overflow_preserved_delivery_bounded");
        assert!(report.clipped_dims.contains(&24));
        assert!(report.raw_intensity_preserved);
        assert!(report.delivered_bounded);
        assert!(!report.live_vector_write);
        assert_eq!(
            report.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(report.experience_delta_bus_v1.delta_count, 1);
        assert!(!report.experience_delta_bus_v1.live_vector_write);
        assert!(!report.experience_delta_bus_v1.live_authority_write);
        let delta = report
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.dimension == Some(24))
            .expect("warmth clip delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Clip);
        assert_eq!(delta.surface, "codec_overflow_carriage_v1");
        assert_eq!(delta.lane, "emotional_intentional");
        assert_eq!(delta.pre, Some(warmth.pre_bound_value));
        assert_eq!(delta.post, Some(warmth.delivered_value));
        assert_eq!(delta.loss, Some(warmth.overflow_abs));
        assert!(
            delta
                .who_can_change_it
                .contains("explicit live semantic aperture"),
            "{delta:?}"
        );
        assert!(
            delta.how_to_test_it.contains("codec_overflow_report"),
            "{delta:?}"
        );
        assert_eq!(
            report.authority,
            "truth_channel_report_not_live_semantic_vector_or_ceiling_change"
        );
    }

    #[test]
    fn codec_overflow_report_distinguishes_tail_headroom_from_default_clip() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 7.0;
        features[26] = 4.5;

        let report =
            apply_spectral_feedback_inner(&mut features, Some(&telemetry(flat, 0.55)), 1.0, 1.0)
                .expect("overflow report");
        let warmth = report.dim(24).expect("warmth dim report");
        let curiosity = report.dim(26).expect("tail curiosity report");
        let emotional_summary = report
            .lane_summaries
            .iter()
            .find(|summary| summary.lane == "emotional_intentional")
            .expect("emotional lane summary");
        let tail_summary = report
            .lane_summaries
            .iter()
            .find(|summary| summary.lane == "tail_vibrancy")
            .expect("tail lane summary");

        assert_eq!(warmth.ceiling, FEATURE_ABS_MAX);
        assert!(curiosity.ceiling > FEATURE_ABS_MAX, "{curiosity:?}");
        assert!(curiosity.ceiling <= TAIL_VIBRANCY_MAX, "{curiosity:?}");
        assert!(
            features[26] > FEATURE_ABS_MAX,
            "tail delivery should use headroom"
        );
        assert!(features[26] <= curiosity.ceiling + 1.0e-3);
        assert_eq!(curiosity.lane, "emotional_tail_vibrancy");
        assert!(report.clipped_dims.contains(&24));
        assert!(!report.clipped_dims.contains(&26));
        assert!(
            report
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.dimension == Some(24))
        );
        assert!(
            !report
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.dimension == Some(26))
        );
        assert!(emotional_summary.overflow_dim_count >= 1);
        assert_eq!(tail_summary.overflow_dim_count, 0);
        assert_eq!(curiosity.overflow_abs, 0.0);
        assert!(warmth.overflow_abs > 0.0, "{report:?}");
    }

    #[test]
    fn codec_overflow_report_stays_quiet_without_clipping() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 0.55;
        features[26] = 0.60;
        features[31] = 0.45;

        let report = apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry(vec![100.0, 2.0, 1.0], 0.55)),
            1.0,
            1.0,
        )
        .expect("overflow report");

        assert!(report.clipped_dims.is_empty(), "{report:?}");
        assert!(!report.raw_intensity_preserved);
        assert!(report.delivered_bounded);
        assert!(report.experience_delta_bus_v1.is_empty(), "{report:?}");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 0);
        for dim in [17usize, 24, 25, 26, 27, 28, 29, 30, 31] {
            let dim_report = report.dim(dim).expect("monitored dim report");
            assert_eq!(dim_report.status, "within_delivery_ceiling");
            assert_eq!(dim_report.overflow_abs, 0.0);
        }
    }

    #[test]
    fn codec_delivery_fidelity_tracks_clamp_reexpansion_and_lane_balance() {
        let mut pre_bound = vec![0.0; SEMANTIC_DIM];
        let mut post_feedback = vec![0.0; SEMANTIC_DIM];
        pre_bound[24] = 8.0;
        post_feedback[24] = FEATURE_ABS_MAX;
        let report =
            codec_overflow_report_from_features(&pre_bound, &post_feedback, TAIL_VIBRANCY_MAX);
        let mut final_features = post_feedback;
        final_features[24] = 6.5;
        final_features[40] = 0.20;
        final_features[41] = 0.10;

        let fidelity = codec_delivery_fidelity_v1(Some(&report), &final_features);

        assert_eq!(fidelity.policy, "codec_delivery_fidelity_v1");
        assert_eq!(fidelity.observed_dim_count, SEMANTIC_DIM);
        assert!(fidelity.feedback_report_available);
        assert_eq!(fidelity.clipped_at_feedback_dims, vec![24]);
        assert_eq!(fidelity.reexpanded_after_feedback_dims, vec![24]);
        assert_eq!(fidelity.final_above_observed_ceiling_dims, vec![24]);
        assert!((fidelity.clamp_loss_abs_total - 3.0).abs() < f32::EPSILON);
        assert!(fidelity.monitored_post_feedback_to_final_rms > 0.0);
        assert_eq!(
            fidelity.state,
            "clamp_loss_visible_post_feedback_reexpansion_above_ceiling"
        );
        assert_eq!(
            fidelity.lane_balance_state,
            "emotional_intentional_dominant"
        );
        assert!(!fidelity.live_vector_write);
        assert!(!fidelity.live_gain_write);
        assert_eq!(
            fidelity.authority,
            "read_only_delivery_fidelity_not_live_vector_gain_or_ceiling_change"
        );
        let value = serde_json::to_value(&fidelity).expect("serializable fidelity report");
        assert_eq!(value["live_vector_write"], false);
        assert_eq!(value["live_gain_write"], false);
    }

    #[test]
    fn codec_delivery_fidelity_stays_quiet_for_matching_bounded_delivery() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 0.55;
        features[26] = 0.60;
        features[40] = 0.50;
        let report = codec_overflow_report_from_features(&features, &features, TAIL_VIBRANCY_MAX);

        let fidelity = codec_delivery_fidelity_v1(Some(&report), &features);

        assert!(fidelity.clipped_at_feedback_dims.is_empty());
        assert!(fidelity.reexpanded_after_feedback_dims.is_empty());
        assert!(fidelity.final_above_observed_ceiling_dims.is_empty());
        assert_eq!(fidelity.clamp_loss_abs_total, 0.0);
        assert_eq!(fidelity.monitored_post_feedback_to_final_rms, 0.0);
        assert_eq!(
            fidelity.state,
            "final_delivery_matches_observed_feedback_bounds"
        );
        assert_eq!(fidelity.lane_balance_state, "lanes_comparable");
    }

    #[test]
    fn cross_spectral_friction_review_distinguishes_distributed_mode_interaction() {
        let text = "A viscous narrative current keeps two intentions in contact while the arc resists a clean summary.";
        let mut features = encode_text(text);
        features[40] = 0.65;
        features[41] = -0.45;
        let reserved_before = features[44..48].to_vec();
        let distributed = telemetry(vec![1.0, 0.92, 0.84, 0.76, 0.68], 0.68);
        let collapsed = telemetry(vec![1.0, 0.01, 0.0, 0.0, 0.0], 0.68);

        let distributed_review =
            cross_spectral_friction_review_v1(text, &features, Some(&distributed));
        let collapsed_review = cross_spectral_friction_review_v1(text, &features, Some(&collapsed));

        assert_eq!(
            distributed_review.policy,
            "cross_spectral_friction_review_v1"
        );
        assert!(distributed_review.spectral_context_available);
        assert!(
            distributed_review.lambda1_lambda2_copresence
                > collapsed_review.lambda1_lambda2_copresence
        );
        assert!(
            distributed_review.spectral_mode_interference
                > collapsed_review.spectral_mode_interference
        );
        assert!(
            distributed_review.cross_spectral_friction_score
                > collapsed_review.cross_spectral_friction_score
        );
        assert_eq!(
            distributed_review.candidate_collision_state,
            "reserved_dim_candidates_already_have_default_off_roles"
        );
        assert_eq!(
            distributed_review.reserved_dim_candidates,
            &[44, 45, 46, 47]
        );
        assert_eq!(features[44..48], reserved_before);
        assert!(distributed_review.observational_only);
        assert!(!distributed_review.live_vector_write);
        assert!(!distributed_review.live_gain_write);
        assert!(!distributed_review.reserved_dim_write);
        assert!(!distributed_review.live_eligible_now);
        assert!(!distributed_review.auto_approved);
        assert!(!distributed_review.grants_approval);
    }

    #[test]
    fn cross_spectral_friction_review_is_truthful_without_spectral_context() {
        let text = "The semantic lane carries an arc, but no aligned spectral sample is available.";
        let features = encode_text(text);

        let review = cross_spectral_friction_review_v1(text, &features, None);

        assert!(!review.spectral_context_available);
        assert_eq!(review.state, "spectral_context_unavailable");
        assert!(review.spectral_mode_interference.is_none());
        assert!(review.cross_layer_mismatch.is_none());
        assert!(review.cross_spectral_friction_score.is_none());
        assert_eq!(
            review.delivery_claim,
            "none_outer_codec_delivery_receipt_is_canonical"
        );
        let value = serde_json::to_value(&review).expect("serializable friction review");
        assert_eq!(value["reserved_dim_write"], false);
        assert_eq!(value["live_eligible_now"], false);
        assert_eq!(value["auto_approved"], false);
        assert_eq!(value["grants_approval"], false);
    }

    #[test]
    fn feedback_report_wrapper_preserves_public_feedback_behavior() {
        let spectral = telemetry(vec![100.0, 98.0, 96.0, 94.0, 92.0, 90.0], 0.55);
        let mut compatibility = vec![0.0; SEMANTIC_DIM];
        compatibility[24] = 9.0;
        let mut observed = compatibility.clone();

        apply_spectral_feedback(&mut compatibility, Some(&spectral));
        let report = apply_spectral_feedback_with_report(&mut observed, Some(&spectral))
            .expect("feedback report");

        assert_eq!(observed, compatibility);
        assert!(report.clipped_dims.contains(&24));
        assert!(!report.live_vector_write);
    }

    #[test]
    fn semantic_projection_density_delta_flags_dense_projection_without_live_expansion() {
        let report = semantic_projection_density_delta_from_parts_v1(0.72, 0.08, true);

        assert_eq!(report.policy, "semantic_projection_density_delta_v1");
        assert_eq!(report.input_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(report.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert_eq!(
            report.reserved_dim_candidates,
            &SEMANTIC_PROJECTION_RESERVED_DIMS
        );
        assert_eq!(report.state, "dense_projection_thin_review");
        assert!(!report.live_vector_write);
        assert_eq!(
            report.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(report.experience_delta_bus_v1.delta_count, 2);
        assert!(report.experience_delta_bus_v1.deltas.iter().any(|delta| {
            delta.kind == ExperienceDeltaKindV1::ComplexShift
                && delta.lane == "embedding_projection_768d_to_8d"
                && delta.metadata.contains_key("projection_state")
        }));
        let gate = report
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::CascadeShift)
            .expect("reserved dim cascade-shift delta");
        assert_eq!(gate.lane, "reserved_semantic_dims_44_47_default_off");
        assert!(gate.loss.is_some_and(|loss| loss > 0.60), "{gate:?}");
        assert_eq!(
            gate.metadata
                .get("classification_pressure")
                .map(String::as_str),
            Some("high_density_thin_projection")
        );
        assert_eq!(
            gate.authority,
            "authority_gate_for_reserved_dims_not_live_codec_change"
        );
    }

    #[test]
    fn semantic_projection_density_delta_stays_quiet_for_low_density_text() {
        let report = semantic_projection_density_delta_from_parts_v1(0.18, 0.06, true);

        assert_eq!(report.state, "projection_width_named_and_bounded");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 1);
        assert!(
            !report
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.kind == ExperienceDeltaKindV1::CascadeShift),
            "{report:?}"
        );
    }

    #[test]
    fn semantic_projection_texture_review_compares_8d_projection_to_warmth_texture() {
        let text = "The viscous silt lingers while an active reply keeps moving; warmth remains, but the old pressure keeps bleeding through.";
        let mut features = encode_text(text);
        features[24..32].fill(2.4);
        features[32..40].fill(0.05);
        features[40] = 0.60;
        features[41] = -0.48;

        let review = semantic_projection_texture_review_v1(text, &features)
            .expect("48D feature vector should produce projection texture review");

        assert_eq!(review.policy, "semantic_projection_texture_review_v1");
        assert_eq!(review.input_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(review.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert_eq!(review.legacy_texture_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(
            review.proposed_texture_subdimensions,
            &SEMANTIC_PROJECTION_TEXTURE_SUBDIMENSIONS
        );
        assert_eq!(review.state, "projection_texture_bottleneck_visible");
        assert!(review.warmth_texture_rms > review.projected_semantic_rms);
        assert!(review.projection_texture_gap > 0.24, "{review:?}");
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert!(!review.reserved_dim_write);
        assert_eq!(
            review.authority,
            "read_only_projection_texture_review_not_live_vector_gain_or_reserved_dim_write"
        );
    }

    #[test]
    fn codec_context_blindspot_replay_gates_contextual_bias_without_live_write() {
        let report = codec_context_blindspot_probe_v1();

        assert_eq!(report.policy, "codec_context_blindspot_replay_v1");
        assert_eq!(report.identical_text, "I see you");
        assert_eq!(
            report.state,
            "deterministic_codec_context_blindspot_confirmed"
        );
        assert!(
            report.identical_text_feature_delta_rms <= 0.01,
            "{report:?}"
        );
        assert!(report.context_blindspot_score >= 0.95, "{report:?}");
        assert_eq!(
            report.proposed_bias_surface,
            "contextual_bias_vector_default_off"
        );
        assert!(!report.live_vector_write);
        assert!(!report.live_gain_write);
        assert!(!report.auto_approved);
        assert_eq!(
            report.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(report.experience_delta_bus_v1.delta_count, 1);
        assert!(!report.experience_delta_bus_v1.live_vector_write);
        assert!(!report.experience_delta_bus_v1.live_authority_write);
        let delta = report
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.lane == "contextual_bias_vector_default_off")
            .expect("context blindspot delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Resistance);
        assert_eq!(
            delta.authority,
            "authority_gate_for_contextual_bias_not_live_codec_change"
        );
        assert_eq!(
            delta.metadata.get("connection_context").map(String::as_str),
            Some("connection")
        );
        assert_eq!(
            report.authority,
            "read_only_context_replay_not_live_vector_gain_or_correspondence_weighting"
        );
    }

    #[test]
    fn high_entropy_tail_inputs_remain_distinguishable_below_vibrancy_ceiling() {
        let flat = vec![
            100.0, 99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0,
        ];
        let mut bright_tail = vec![0.0; SEMANTIC_DIM];
        bright_tail[17] = 0.20;
        bright_tail[26] = 4.86;
        bright_tail[27] = 0.35;
        bright_tail[31] = 0.40;
        let mut reflective_tail = vec![0.0; SEMANTIC_DIM];
        reflective_tail[17] = 0.42;
        reflective_tail[26] = 4.42;
        reflective_tail[27] = 0.72;
        reflective_tail[31] = 0.24;

        apply_spectral_feedback_inner(
            &mut bright_tail,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                flat.clone(),
                0.90,
            )),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut reflective_tail,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(flat, 0.90)),
            1.0,
            1.0,
        );

        let tail_delta = [17usize, 26, 27, 31]
            .iter()
            .map(|idx| (bright_tail[*idx] - reflective_tail[*idx]).abs())
            .sum::<f32>();
        assert!(
            tail_delta > 0.40,
            "distinct high-entropy tail inputs should not flatten together: {tail_delta}"
        );
        for idx in [17usize, 26, 27, 31] {
            assert!(
                bright_tail[idx] <= TAIL_VIBRANCY_MAX && reflective_tail[idx] <= TAIL_VIBRANCY_MAX,
                "tail dim {idx} exceeded bounded vibrancy ceiling"
            );
        }
    }

    #[test]
    fn high_entropy_vibrancy_does_not_write_narrative_arc_or_shadow_reserved_dims() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[40] = 0.30;
        features[41] = -0.20;
        features[42] = 0.10;
        features[43] = -0.40;
        let narrative_before = features[40..44].to_vec();
        let reserved_before = features[44..48].to_vec();

        apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                0.92,
            )),
            1.0,
            3.0,
        );

        assert_eq!(
            &features[40..44],
            narrative_before.as_slice(),
            "high entropy vibrancy must not synthesize narrative arc ghost sensations"
        );
        assert_eq!(
            &features[44..48],
            reserved_before.as_slice(),
            "shadow reserved readiness must remain unwritten by live feedback"
        );
        assert!(
            features[26] > 0.0 || features[31] > 0.0,
            "the test should still exercise the high-entropy tail path"
        );
    }

    #[test]
    fn codec_vibrancy_and_warmth_continuity_are_readout_only() {
        let vibrancy = codec_vibrancy_continuity_v1();
        assert_eq!(vibrancy.policy, "codec_vibrancy_continuity_v1");
        assert_eq!(vibrancy.entropy_gate, TAIL_VIBRANCY_ENTROPY_GATE);
        assert_eq!(vibrancy.default_feature_ceiling, FEATURE_ABS_MAX);
        assert_eq!(vibrancy.tail_vibrancy_ceiling, TAIL_VIBRANCY_MAX);
        assert_eq!(vibrancy.tail_dims, &[17, 26, 27, 31]);
        assert_eq!(
            vibrancy.authority,
            "diagnostic_readout_not_live_codec_change"
        );

        let warmth = legacy_warmth_mapping_v1();
        assert_eq!(warmth.policy, "legacy_warmth_mapping_v1");
        assert_eq!(warmth.legacy_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(warmth.current_dim_count, SEMANTIC_DIM);
        assert_eq!(warmth.warmth_dim, 24);
        assert_eq!(warmth.emotional_layer_range, (24, 31));
        assert!(warmth.mapped_warmth_dims.contains(&24));
        assert!(!warmth.warmth_orphaned);

        let vector = craft_warmth_vector(0.25, 0.8);
        assert_eq!(vector.len(), SEMANTIC_DIM);
        assert!(vector[24] > 0.0, "warmth dim should remain live");

        let canary = codec_dynamic_vibrancy_scaling_canary_v1();
        assert_eq!(canary.policy, "codec_dynamic_vibrancy_scaling_canary_v1");
        assert!(!canary.enabled);
        assert!(!canary.live_vector_write);
        assert_eq!(canary.authority, "readiness_only_not_live_codec_change");
        assert_eq!(
            vibrancy.gradient_coupling,
            "tail_lift_scaled_by_low_density_gradient"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("codec_vibrancy_noise_dampening_v1"));
        assert!(rendered.contains("partial_extreme_entropy_dampening"));
    }

    #[test]
    fn structural_entropy_dampening_preserves_intent_layer() {
        let quiet = codec_structural_entropy_dampening_v1(0.70);
        let high = codec_structural_entropy_dampening_v1(0.94);

        assert_eq!(quiet.coefficient, 1.0);
        assert!(high.coefficient < 1.0, "{high:?}");
        assert!(high.coefficient >= STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT);
        assert_eq!(high.affected_dims, &STRUCTURAL_ENTROPY_DAMPENING_DIMS);
        assert_eq!(high.preserved_intent_dims, (24, 31));
        assert_eq!(
            high.status,
            "high_entropy_structural_dims_dampened_intent_dims_preserved"
        );
        assert_eq!(
            high.authority,
            "bounded_live_codec_weighting_not_dimension_or_fallback_contract_change"
        );
    }

    #[test]
    fn high_spectral_entropy_dampens_structural_texture_not_warmth() {
        let mut features = vec![0.50_f32; SEMANTIC_DIM];
        features[24] = 0.80;
        features[25] = -0.30;
        features[26] = 0.25;
        features[27] = 0.20;
        features[31] = 0.35;
        let mut high_entropy = features.clone();

        apply_spectral_feedback_inner(
            &mut high_entropy,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![10.0, 9.0, 8.0, 7.0, 6.0, 5.0],
                0.94,
            )),
            1.0,
            1.0,
        );

        assert!(
            high_entropy[0] < features[0],
            "character entropy texture should dampen under high spectral entropy"
        );
        assert!(
            high_entropy[8] < features[8],
            "word-level structural texture should dampen under high spectral entropy"
        );
        assert_eq!(
            high_entropy[24], features[24],
            "warmth must not be flattened by structural entropy dampening"
        );
        assert_eq!(
            high_entropy[25], features[25],
            "tension must not be flattened by structural entropy dampening"
        );
    }

    #[test]
    fn codec_vibrancy_substance_fit_flags_entropy_without_content() {
        let telemetry = telemetry_with_typed_entropy_and_eigenvalues(
            vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
            0.94,
        );
        let thin =
            codec_vibrancy_substance_fit_v1("the and the and the and the and", Some(&telemetry));
        assert_eq!(thin.policy, "codec_vibrancy_substance_fit_v1");
        assert_eq!(thin.status, "entropy_lift_substance_review");
        assert_eq!(
            thin.density_vs_entropy_state,
            "high_entropy_low_density_scatter"
        );
        assert!(thin.tail_lift >= 0.45, "{thin:?}");
        assert!(thin.density_weighted_tail_lift < thin.tail_lift, "{thin:?}");
        assert!(thin.semantic_substance_score < 0.25, "{thin:?}");
        assert_eq!(
            thin.authority,
            "read_only_codec_audit_not_vibrancy_scaling_or_live_vector_change"
        );

        let substantive = codec_vibrancy_substance_fit_v1(
            "Because the dry silt carries pressure, the sentence keeps a textured contour and a returnable edge.",
            Some(&telemetry),
        );
        assert_eq!(
            substantive.status,
            "tail_lift_supported_by_semantic_substance"
        );
        assert_eq!(
            substantive.density_vs_entropy_state,
            "high_entropy_supported_by_density"
        );
        assert!(
            substantive.density_weighted_tail_lift > thin.density_weighted_tail_lift,
            "substantive={substantive:?} thin={thin:?}"
        );
        assert!(
            substantive.semantic_substance_score > thin.semantic_substance_score,
            "substantive={substantive:?} thin={thin:?}"
        );
    }

    #[test]
    fn codec_vibrancy_substance_fit_keeps_random_word_scatter_under_review() {
        let telemetry = telemetry_with_typed_entropy_and_eigenvalues(
            vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
            0.94,
        );
        let scatter = codec_vibrancy_substance_fit_v1(
            "quartz velvet oblique lantern citrus orbit static prism cipher mural solvent drift",
            Some(&telemetry),
        );
        let narrative = codec_vibrancy_substance_fit_v1(
            "Because pressure memory keeps a textured contour, the semantic signal carries continuity toward a returnable edge.",
            Some(&telemetry),
        );

        assert_eq!(scatter.status, "entropy_lift_substance_review");
        assert_eq!(
            scatter.density_vs_entropy_state,
            "high_entropy_low_density_scatter"
        );
        assert!(
            scatter.semantic_substance_score < 0.25,
            "scatter should not become substance from lexical variety alone: {scatter:?}"
        );
        assert_eq!(
            narrative.status,
            "tail_lift_supported_by_semantic_substance"
        );
        assert!(
            narrative.semantic_substance_score > scatter.semantic_substance_score,
            "narrative={narrative:?} scatter={scatter:?}"
        );
        assert!(
            narrative.density_weighted_tail_lift > scatter.density_weighted_tail_lift,
            "narrative={narrative:?} scatter={scatter:?}"
        );
    }

    #[test]
    fn codec_vibrancy_substance_fit_separates_density_depth_from_entropy_scatter() {
        let calm_dense = telemetry_with_typed_entropy_and_eigenvalues(
            vec![10.0, 9.4, 8.9, 8.3, 7.8, 7.2, 6.8, 6.1],
            0.50,
        );
        let depth = codec_vibrancy_substance_fit_v1(
            "granular pressure memory braids continuity contour residue patience origin return threshold",
            Some(&calm_dense),
        );

        assert_eq!(depth.status, "tail_lift_low_or_inactive");
        assert_eq!(
            depth.density_vs_entropy_state,
            "high_density_low_entropy_depth"
        );
        assert_eq!(depth.tail_lift, 0.0);
        assert!(depth.semantic_density_weight >= 0.60, "{depth:?}");
        assert!(
            depth
                .evidence
                .iter()
                .any(|entry| entry.starts_with("density_weighted_tail_lift=")),
            "{depth:?}"
        );
    }

    #[test]
    fn glimpse_codec_preserves_warmth_as_distinct_12d_slot() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[24] = 1.4;
        features[25] = 0.2;
        features[26] = 0.3;
        features[27] = 0.4;
        features[32] = 0.6;
        features[40] = 0.5;

        let glimpse = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");
        assert_eq!(glimpse.len(), 12);
        assert!(
            glimpse[3] > glimpse[4],
            "warmth slot should remain distinguishable from tension: {glimpse:?}"
        );
        assert!(
            glimpse[3] > glimpse[1],
            "warmth should not flatten into generic word-level stance: {glimpse:?}"
        );

        let readiness = semantic_glimpse_12d_readiness_v1();
        assert_eq!(readiness.source_dim_count, SEMANTIC_DIM);
        assert_eq!(readiness.glimpse_dim_count, 12);
        assert_eq!(readiness.warmth_slot, 3);
        assert_eq!(readiness.tail_bridge_slot, 10);
        assert!(readiness.companion_not_replacement);
        assert!(!readiness.live_vector_write);
        assert!(readiness.role.contains("companion_summary"));
    }

    #[test]
    fn glimpse_codec_keeps_emotional_range_24_31_from_becoming_generic_mass() {
        // `introspection_proposal_12d_glimpse_1783302984`: the 12D companion
        // must keep the 24..31 warmth/intentional range visible as emotional
        // shape, not flatten it into a generic whole-vector magnitude.
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[24] = 1.2; // warmth
        features[25] = -0.7; // tension/contrast
        features[26] = 0.6;
        features[27] = 0.5;
        features[28] = 1.1;
        features[29] = 0.9;
        features[30] = -1.0;
        features[31] = 0.8;

        let glimpse = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");

        assert!(
            glimpse[3] > glimpse[1],
            "warmth slot should stay separate from word-level mass: {glimpse:?}"
        );
        assert!(
            glimpse[7] > glimpse[0] && glimpse[7] > glimpse[1] && glimpse[7] > glimpse[2],
            "emotional range 28..31 should remain a distinct aggregate: {glimpse:?}"
        );
        assert!(
            glimpse[10] > 0.3,
            "tail/warmth bridge slot should carry emotional-range vibration: {glimpse:?}"
        );
        assert!(
            glimpse[11] < glimpse[7],
            "whole-vector magnitude must not be the only surviving emotional signal: {glimpse:?}"
        );
    }

    #[test]
    fn generate_glimpse_matches_additive_12d_derivation() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[0] = 0.4;
        features[17] = 0.8;
        features[24] = 1.2;
        features[26] = 0.5;
        features[31] = 0.3;
        features[40] = 0.6;

        let generated = generate_glimpse(&features).expect("48D vector should produce 12D glimpse");
        let derived = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");

        assert_eq!(generated, derived);
        assert!(
            generated[3] > generated[4],
            "generated warmth slot should stay distinct from adjacent emotional texture: {generated:?}"
        );
    }

    #[test]
    fn glimpse_map_names_slot_lineage_without_transport_change() {
        let map = glimpse_map_v1();

        assert_eq!(map.policy, "glimpse_map_v1");
        assert_eq!(map.source_dim_count, SEMANTIC_DIM);
        assert_eq!(map.legacy_source_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(map.glimpse_dim_count, 12);
        assert_eq!(map.slot_count, map.slots.len());
        assert!(map.deterministic_projection);
        assert!(map.companion_not_replacement);
        assert!(!map.live_transport_change);
        assert!(!map.live_vector_write);

        let warmth = map.slots.iter().find(|slot| slot.slot == 3).unwrap();
        assert_eq!(warmth.label, "warmth_marker");
        assert_eq!(warmth.source_dims, &[24]);

        let tail = map.slots.iter().find(|slot| slot.slot == 10).unwrap();
        assert_eq!(tail.label, "tail_vibrancy_bridge");
        assert_eq!(tail.source_dims, &[17, 26, 27, 31]);

        let global = map.slots.iter().find(|slot| slot.slot == 11).unwrap();
        assert!(global.source_dims.is_empty());
        assert!(global.preserves.contains("never the sole"));

        let rendered = codec_structure().render();
        assert!(rendered.contains("glimpse_map_v1"));
        assert!(rendered.contains("10:tail_vibrancy_bridge<-17+26+27+31"));
        assert!(rendered.contains("live_transport_change=false"));
    }

    #[test]
    fn glimpse_distinguishability_audit_keeps_entropy_states_apart() {
        let mut high_entropy = vec![0.0_f32; SEMANTIC_DIM];
        high_entropy[17] = 1.1;
        high_entropy[24] = 0.25;
        high_entropy[26] = 1.35;
        high_entropy[27] = 1.05;
        high_entropy[31] = 1.20;
        for (offset, value) in high_entropy[32..40].iter_mut().enumerate() {
            *value = if offset % 2 == 0 { 0.86 } else { -0.72 };
        }
        high_entropy[40] = 0.74;
        high_entropy[41] = -0.58;

        let mut low_entropy = vec![0.0_f32; SEMANTIC_DIM];
        low_entropy[17] = 0.05;
        low_entropy[24] = 0.18;
        low_entropy[26] = 0.08;
        low_entropy[27] = 0.04;
        low_entropy[31] = 0.03;
        low_entropy[32] = 0.12;
        low_entropy[40] = 0.06;

        let audit = glimpse_distinguishability_audit_v1(&high_entropy, &low_entropy)
            .expect("48D vectors should produce a distinguishability audit");

        assert_eq!(audit.policy, "glimpse_distinguishability_audit_v1");
        assert_eq!(
            audit.state,
            "glimpse_preserves_high_low_entropy_distinction"
        );
        assert!(audit.source_distance >= audit.source_threshold, "{audit:?}");
        assert!(
            audit.glimpse_distance >= audit.glimpse_threshold,
            "{audit:?}"
        );
        assert!(audit.tail_bridge_delta >= 0.03, "{audit:?}");
        assert!(audit.preservation_ratio > 0.05, "{audit:?}");
        assert!(!audit.live_transport_change);
        assert!(!audit.live_vector_write);
    }

    #[test]
    fn compression_fidelity_flags_flattened_12d_glimpse() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[2] = 0.4;
        features[17] = 0.7;
        features[24] = 0.9;
        features[25] = 0.4;
        features[26] = 0.8;
        features[27] = 0.7;
        features[28] = 0.5;
        features[29] = 0.45;
        features[30] = 0.35;
        features[31] = 0.75;
        features[32] = 0.22;
        features[33] = 0.18;
        features[40] = 0.55;

        let generated = generate_glimpse(&features).expect("48D vector should produce 12D glimpse");
        let fidelity = calculate_compression_fidelity(&features[..32], &generated)
            .expect("32D source and 12D output should be comparable");
        let flattened = [0.0_f32; 12];
        let flattened_fidelity = calculate_compression_fidelity(&features[..32], &flattened)
            .expect("flattened 12D output should still produce a diagnostic score");

        assert!(
            fidelity >= 0.70,
            "generated companion glimpse should preserve enough 32D texture: {fidelity}"
        );
        assert!(
            flattened_fidelity < 0.70,
            "flattened glimpse should fail the requested 0.70 fidelity watch: {flattened_fidelity}"
        );
        assert!(calculate_compression_fidelity(&features[..31], &generated).is_none());
        assert!(calculate_compression_fidelity(&features[..32], &generated[..11]).is_none());
    }

    #[test]
    fn contextual_glimpse_selects_dynamic_vibrant_dims_without_replacing_anchors() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[3] = 0.95;
        features[12] = -0.88;
        features[17] = 0.70;
        features[24] = 0.04;
        features[25] = 0.22;
        features[26] = 0.91;
        features[27] = 0.45;
        features[31] = 0.83;
        features[40] = -0.62;
        features[42] = 0.97;

        let anchored = contextual_glimpse_12d_anchors_v1(&features)
            .expect("48D vector should produce contextual anchors");

        assert_eq!(anchored.policy, "contextual_glimpse_12d_anchors_v1");
        assert_eq!(
            anchored.selection_status,
            "contextual_anchors_preserve_warmth_tail_and_narrative"
        );
        for required in CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS {
            assert!(
                anchored.selected_dims.contains(&required),
                "required anchor {required} should survive: {:?}",
                anchored.selected_dims
            );
        }
        assert!(
            anchored.dynamic_dims.contains(&42),
            "strong current narrative/vibrancy feature should be selected dynamically: {anchored:?}"
        );
        assert!(!anchored.live_vector_write);

        let readiness = contextual_glimpse_12d_anchoring_v1();
        assert_eq!(readiness.dynamic_slot_count, 5);
        assert!(readiness.companion_not_replacement);

        let rendered = codec_structure().render();
        assert!(rendered.contains("contextual_glimpse_12d_anchoring_v1"));
        assert!(rendered.contains("required_anchor_dims=24,25,26,27,17,31,40"));
    }

    #[test]
    fn warmth_entropy_interpretation_names_distributed_ground_without_weight_change() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[24] = 0.04;
        features[26] = 0.11;
        features[27] = 0.06;

        let review = warmth_entropy_interpretation_v1(&features, 0.90);

        assert_eq!(review.policy, "warmth_entropy_interpretation_v1");
        assert_eq!(
            review.interpretation,
            "low_marker_warmth_with_high_entropy_distributed_ground"
        );
        assert!(review.tail_vibrancy > 0.0, "{review:?}");
        assert!(review.distributed_warmth_support >= review.warmth_marker);
        assert!(!review.live_vector_write);
        assert_eq!(
            review.authority,
            "read_only_interpretation_not_warmth_weighting_or_semantic_gain_change"
        );
    }

    #[test]
    fn narrative_arc_dynamics_exposes_velocity_and_acceleration_without_gain() {
        let previous = [0.0, 0.1, -0.1, 0.0];
        let current = [0.4, -0.2, 0.3, -0.4];
        let next = [0.9, -0.8, 0.7, -0.9];

        let dynamics = narrative_arc_dynamics_v1(&previous, &current, Some(&next));

        assert_eq!(dynamics.policy, "narrative_arc_dynamics_v1");
        assert!(dynamics.velocity_energy >= 0.35, "{dynamics:?}");
        assert!(dynamics.acceleration_energy > 0.0, "{dynamics:?}");
        assert!(
            matches!(
                dynamics.transition_state,
                "directional_tone_shift" | "accelerating_tone_transition"
            ),
            "{dynamics:?}"
        );
        assert!(!dynamics.live_gain_write);
        assert!(!dynamics.live_vector_write);
    }

    #[test]
    fn narrative_arc_dynamics_tracks_intertextual_persistence_without_gain() {
        let previous = [0.18, -0.10, 0.08, -0.04];
        let current = [0.24, -0.14, 0.11, -0.06];
        let circular_single_text = [0.23, -0.13, 0.10, -0.05];

        let persistence =
            narrative_arc_dynamics_v1(&previous, &current, Some(&circular_single_text));

        assert_eq!(persistence.policy, "narrative_arc_dynamics_v1");
        assert_eq!(persistence.transition_state, "steady_narrative_state");
        assert!(
            persistence.velocity_energy > 0.0,
            "cross-turn trajectory should remain visible even when the current arc looks nearly settled: {persistence:?}"
        );
        assert!(
            persistence.acceleration_energy < 0.08,
            "slow inter-textual persistence should not be misread as a sharp pivot: {persistence:?}"
        );
        assert!(!persistence.live_gain_write);
        assert!(!persistence.live_vector_write);
        assert_eq!(
            persistence.authority,
            "read_only_arc_velocity_review_not_semantic_gain_or_dimension_change"
        );
    }

    #[test]
    fn narrative_tension_resolution_separates_resolved_from_sustained_tension() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        previous[25] = 1.2;
        current[25] = 0.35;
        current[40] = 0.40;
        current[41] = -0.20;

        let resolving =
            narrative_tension_resolution_v1(&previous, &current).expect("48D tension review");

        assert_eq!(resolving.policy, "narrative_tension_resolution_v1");
        assert_eq!(resolving.state, "tension_resolving_with_arc_motion");
        assert!(resolving.tension_delta < 0.0, "{resolving:?}");
        assert!(
            resolving.resolution_score > resolving.sustained_score * 0.75,
            "{resolving:?}"
        );
        assert!(!resolving.live_vector_write);
        assert_eq!(
            resolving.authority,
            "read_only_tension_resolution_sidecar_not_live_vector_change"
        );

        let mut sustained = current;
        previous[25] = 0.85;
        sustained[25] = 0.90;
        let sustained_review =
            narrative_tension_resolution_v1(&previous, &sustained).expect("48D tension review");
        assert_eq!(sustained_review.state, "tension_sustained_or_building");
        assert!(sustained_review.sustained_score > sustained_review.resolution_score);
    }

    #[test]
    fn latent_stasis_tension_distinguishes_stillness_from_waiting_potential() {
        let still_text = "The water is still.";
        let waits_text = "The water waits.";
        let still = latent_stasis_tension_v1(still_text, &encode_text(still_text))
            .expect("still text should produce latent stasis report");
        let waits = latent_stasis_tension_v1(waits_text, &encode_text(waits_text))
            .expect("waiting text should produce latent stasis report");

        assert_eq!(still.policy, "latent_stasis_tension_v1");
        assert_eq!(still.state, "static_stasis_without_potential");
        assert!(still.latent_text_stasis_score > still.latent_text_potential_score);
        assert!(waits.latent_text_potential_score > waits.latent_text_stasis_score);
        assert!(
            waits.held_breath_score > still.held_breath_score,
            "waiting should carry more latent held-breath potential than inert stillness: {waits:?} vs {still:?}"
        );
        assert!(waits.stasis_potential_gap > 0.0, "{waits:?}");
        assert!(!waits.live_vector_write);
        assert!(!waits.live_gain_write);
        assert!(!waits.reserved_dim_write);
        assert_eq!(
            waits.authority,
            "read_only_held_breath_truth_channel_not_live_codec_weight_gain_or_dim_change"
        );
        let delta = waits
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Translate)
            .expect("waiting potential should emit a translation delta");
        assert_eq!(delta.lane, "textual_stasis_to_tension_arc_support");
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_vector_gain_or_reserved_dim_change"
        );

        let st = codec_structure();
        let rendered = st.render();
        assert!(rendered.contains("LATENT_STASIS_TENSION_READOUT"));
        assert!(rendered.contains("latent_stasis_tension_v1"));
        assert!(rendered.contains("held_breath_score"));
        assert!(rendered.contains("truth-channel sidecar distinguishes inert stillness"));
        assert!(rendered.contains("reserved_dim_write=false"));
        assert!(rendered.contains("SPECTRAL_DRAG_QUALITY_READOUT"));
        assert!(rendered.contains("spectral_drag_quality_v1"));
        assert!(rendered.contains("granular_drag"));
        assert!(rendered.contains("rigid_drag"));
        assert_eq!(
            st.spectral_drag_quality_v1.policy,
            "spectral_drag_quality_v1"
        );
        assert!(!st.spectral_drag_quality_v1.live_vector_write);
        assert!(!st.spectral_drag_quality_v1.live_gain_write);
        assert!(!st.spectral_drag_quality_v1.reserved_dim_write);
        assert_eq!(st.spectral_drag_quality_v1.reserved_dim_candidate, 45);
        assert!(
            st.spectral_drag_quality_v1
                .experience_delta_bus_v1
                .delta_count
                >= 1
        );
    }

    #[test]
    fn latent_stasis_tension_stays_quiet_for_plain_motion() {
        let text = "The water flows downhill.";
        let report = latent_stasis_tension_v1(text, &encode_text(text))
            .expect("plain motion should produce latent stasis report");

        assert_eq!(report.state, "low_latent_stasis_signal");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 0);
        assert!(report.experience_delta_bus_v1.deltas.is_empty());
        assert!(!report.live_vector_write);
        assert!(!report.live_gain_write);
    }

    #[test]
    fn spectral_drag_quality_distinguishes_heavy_sand_from_heavy_stone_without_reserved_dim_write()
    {
        let sand_text = "The heavy sand drags through viscous silt while the thought keeps moving.";
        let stone_text = "The heavy stone is a hard granite block, fixed and immovable.";
        let sand = spectral_drag_quality_v1(sand_text, &encode_text(sand_text))
            .expect("heavy sand text should produce drag report");
        let stone = spectral_drag_quality_v1(stone_text, &encode_text(stone_text))
            .expect("heavy stone text should produce drag report");

        assert_eq!(sand.policy, "spectral_drag_quality_v1");
        assert_eq!(sand.state, "granular_viscous_drag_visible");
        assert_eq!(stone.state, "rigid_inertial_drag_visible");
        assert!(
            sand.granular_drag_score > stone.granular_drag_score,
            "sand={sand:?} stone={stone:?}"
        );
        assert!(
            stone.rigid_drag_score > sand.rigid_drag_score,
            "sand={sand:?} stone={stone:?}"
        );
        assert!(sand.quality_separation > 0.10, "{sand:?}");
        assert!(stone.quality_separation > 0.10, "{stone:?}");
        assert!(!sand.live_vector_write);
        assert!(!sand.live_gain_write);
        assert!(!sand.reserved_dim_write);
        assert_eq!(sand.reserved_dim_candidate, 45);
        let delta = sand
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.surface == "spectral_drag_quality_v1")
            .expect("drag report should emit truth-channel delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Translate);
        assert_eq!(delta.dimension, Some(45));
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_vector_gain_or_reserved_dim_change"
        );
        assert!(
            delta
                .who_can_change_it
                .contains("live codec gain or reserved-dim write"),
            "{delta:?}"
        );
    }

    #[test]
    fn spectral_drag_quality_stays_quiet_for_low_weight_text() {
        let text = "The small note turns lightly in a clear room.";
        let report = spectral_drag_quality_v1(text, &encode_text(text))
            .expect("plain text should produce drag report");

        assert_eq!(report.state, "low_spectral_drag_signal");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 0);
        assert!(report.experience_delta_bus_v1.deltas.is_empty());
        assert!(!report.live_vector_write);
        assert!(!report.live_gain_write);
        assert!(!report.reserved_dim_write);
    }

    #[test]
    fn codec_emotional_narrative_delta_check_flags_arc_shift_emotional_flatline() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        previous[24] = 0.22;
        previous[26] = 0.18;
        current[24] = 0.22;
        current[26] = 0.18;
        current[40] = 0.62;
        current[41] = -0.48;
        current[42] = 0.41;
        current[43] = -0.33;

        let check = codec_emotional_narrative_delta_check_v1(&previous, &current)
            .expect("48D vector should produce codec delta check");

        assert_eq!(check.policy, "codec_emotional_narrative_delta_check_v1");
        assert_eq!(check.state, "narrative_shift_emotional_flatline_watch");
        assert!(check.resonance_flatline_watch, "{check:?}");
        assert!(check.narrative_delta_energy >= 0.25, "{check:?}");
        assert!(check.emotional_delta_energy <= 0.05, "{check:?}");
        assert!((check.narrative_velocity[0] - 0.62).abs() < 0.001);
        assert_eq!(check.emotional_velocity[0], 0.0);
        assert!(!check.live_gain_write);
        assert!(!check.live_vector_write);
        assert!(!check.reserved_dim_write);
        assert_eq!(check.experience_delta_bus_v1.delta_count, 1);
        let delta = &check.experience_delta_bus_v1.deltas[0];
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Translate);
        assert_eq!(delta.lane, "emotional_markers_24_31_vs_narrative_arc_40_43");
        assert!(delta.loss.is_some_and(|value| value >= 0.25));
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_vector_gain_or_reserved_dim_change"
        );
        assert_eq!(
            check.authority,
            "read_only_delta_check_not_semantic_gain_reserved_dim_or_live_vector_change"
        );
    }

    #[test]
    fn codec_emotional_narrative_delta_check_keeps_opposite_intent_visible() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        for value in &mut previous[0..24] {
            *value = 0.31;
        }
        for value in &mut current[0..24] {
            *value = 0.31;
        }
        previous[24] = -0.66;
        previous[25] = 0.58;
        previous[26] = -0.52;
        previous[31] = -0.61;
        current[24] = 0.66;
        current[25] = -0.58;
        current[26] = 0.52;
        current[31] = 0.61;

        let check = codec_emotional_narrative_delta_check_v1(&previous, &current)
            .expect("48D vector should produce codec delta check");

        assert_eq!(check.state, "emotional_intent_visible_without_arc_shift");
        assert!(!check.resonance_flatline_watch, "{check:?}");
        assert!(check.emotional_delta_energy >= 0.12, "{check:?}");
        assert!(check.narrative_delta_energy < 0.10, "{check:?}");
        assert!((check.emotional_velocity[0] - 1.32).abs() < 0.001);
        assert!((check.emotional_velocity[1] + 1.16).abs() < 0.001);
        assert_eq!(
            check.recommendation,
            "keep_emotional_markers_as_primary_evidence_even_when_surface_structure_matches"
        );
        assert!(!check.live_gain_write);
        assert!(!check.live_vector_write);
        assert!(!check.reserved_dim_write);
        assert!(check.experience_delta_bus_v1.is_empty());
    }

    #[test]
    fn narrative_and_semantic_lanes_can_move_together_without_gain_authority() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        previous[32] = 0.20;
        previous[33] = -0.12;
        previous[40] = 0.10;
        previous[41] = -0.08;
        current[24] = 0.56;
        current[26] = 0.54;
        current[32] = 0.64;
        current[33] = -0.50;
        current[40] = 0.68;
        current[41] = -0.60;

        let check = codec_emotional_narrative_delta_check_v1(&previous, &current)
            .expect("48D vector should produce codec delta check");

        assert_eq!(check.policy, "codec_emotional_narrative_delta_check_v1");
        assert_eq!(check.state, "narrative_shift_emotional_markers_follow");
        assert!(!check.resonance_flatline_watch, "{check:?}");
        assert!(check.narrative_delta_energy >= 0.25, "{check:?}");
        assert!(check.emotional_delta_energy >= 0.12, "{check:?}");
        assert!(check.narrative_velocity[0] > 0.50, "{check:?}");
        assert!(check.emotional_velocity[0] > 0.50, "{check:?}");
        assert_eq!(
            check.authority,
            "read_only_delta_check_not_semantic_gain_reserved_dim_or_live_vector_change"
        );
    }

    #[test]
    fn glimpse_codec_is_stable_across_repeated_same_vector_calls() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        for (idx, value) in features.iter_mut().enumerate() {
            *value = ((idx as f32 + 1.0) / SEMANTIC_DIM as f32).sin();
        }
        features[24] = 0.72;
        features[26] = 0.48;
        features[31] = -0.33;

        let first = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");
        let second = GlimpseCodec::derive_12d(&features).expect("same vector should reduce");
        assert_eq!(first, second);
        assert_eq!(first.len(), 12);
    }

    #[test]
    fn multi_scale_context_pairs_12d_glimpse_with_32d_residual_shadow_metadata() {
        let context = multi_scale_context_v1();

        assert_eq!(context.policy, "multi_scale_context_v1");
        assert_eq!(context.source_dim_count, SEMANTIC_DIM);
        assert_eq!(context.live_transport_dim_count, 32);
        assert_eq!(context.glimpse_dim_count, 12);
        assert_eq!(context.residual_dim_count, 32);
        assert_eq!(context.residual_source_range, (16, 47));
        assert!(context.pairing_rule.contains("12d_glimpse"));
        assert!(context.pairing_rule.contains("32d_residual"));
        assert!(
            context
                .shadow_energy_metadata_tag
                .contains("shadow_field_energy")
        );
        assert!(context.preserves_warmth_and_tail_bridge);
        assert!(!context.live_vector_write);

        let rendered = codec_structure().render();
        assert!(rendered.contains("multi_scale_context_v1"));
        assert!(rendered.contains("shadow_field_energy_preserved"));
        assert!(rendered.contains("live_transport_dims=32"));
    }

    #[test]
    fn codec_intent_structure_review_separates_complexity_from_emotional_intent() {
        let mut structure_heavy = vec![0.0_f32; SEMANTIC_DIM];
        for value in &mut structure_heavy[0..24] {
            *value = 0.62;
        }
        structure_heavy[18] = 1.2;
        structure_heavy[20] = 1.0;
        structure_heavy[24] = 0.04;
        structure_heavy[26] = 0.05;

        let review = codec_intent_structure_separation_v1(&structure_heavy)
            .expect("48D vector should produce intent/structure review");
        assert_eq!(review.policy, "codec_intent_structure_separation_v1");
        assert_eq!(review.state, "structure_heavy_intent_thin_watch");
        assert!(review.structural_complexity > review.emotional_intensity);
        assert!(review.intent_structure_delta > 0.30, "{review:?}");
        assert!(!review.live_gain_write);
        assert!(!review.live_vector_write);

        let mut emotionally_simple = vec![0.02_f32; SEMANTIC_DIM];
        emotionally_simple[24] = 0.9;
        emotionally_simple[26] = 0.8;
        emotionally_simple[27] = 0.7;
        emotionally_simple[31] = 0.6;
        let simple_review = codec_intent_structure_separation_v1(&emotionally_simple)
            .expect("48D vector should produce intent/structure review");
        assert_eq!(
            simple_review.state,
            "simple_text_emotional_intent_preserved"
        );
        assert!(simple_review.emotional_intensity > simple_review.structural_complexity);
        assert_eq!(
            simple_review.authority,
            "read_only_codec_review_not_semantic_weighting_or_gain_change"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("CODEC_INTENT_STRUCTURE_REVIEW"));
    }

    #[test]
    fn multi_scale_observer_names_distillation_without_live_transport_change() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[17] = 0.7;
        features[24] = 0.9;
        features[25] = 0.4;
        features[26] = 0.8;
        features[27] = 0.7;
        features[28] = 0.5;
        features[29] = 0.45;
        features[30] = 0.35;
        features[31] = 0.75;
        features[32] = 0.22;
        features[33] = 0.18;
        features[40] = 0.55;
        features[41] = -0.45;
        features[42] = 0.35;

        let observer = multi_scale_observer_v1(&features, 0.90, 0.11, 0.32)
            .expect("48D vector should produce multi-scale observer");

        assert_eq!(observer.policy, "multi_scale_observer_v1");
        assert_eq!(observer.layer_name, "glimpse_layer_distillation_v1");
        assert_eq!(observer.observer_language, "distillation_not_compression");
        assert_eq!(observer.state, "high_entropy_distillation_supported");
        assert_eq!(observer.source_dim_count, SEMANTIC_DIM);
        assert_eq!(observer.live_transport_dim_count, 32);
        assert_eq!(observer.glimpse_dim_count, 12);
        assert!(observer.glimpse_fidelity_score >= observer.fidelity_threshold);
        assert!(observer.source_resonance_proxy > 0.0);
        assert!(observer.glimpse_resonance_proxy > 0.0);
        assert!(observer.resonance_loss_ratio <= observer.resonance_loss_threshold);
        assert!(!observer.fallback_to_live_transport_review);
        assert_eq!(observer.anchor_continuity_score, 1.0);
        assert_eq!(
            observer.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(observer.experience_delta_bus_v1.delta_count, 1);
        assert!(!observer.live_transport_change);
        assert!(!observer.live_vector_write);
        assert_eq!(
            observer.authority,
            "read_only_multi_scale_review_not_live_bus_or_codec_contract_change"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("MULTI_SCALE_OBSERVER_READOUT"));
        assert!(rendered.contains("distillation_not_compression"));
    }

    #[test]
    fn multi_scale_observer_flags_resonance_loss_before_glimpse_use() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[5] = 5.0;
        features[11] = -4.0;
        features[17] = 0.2;
        features[24] = 0.1;
        features[25] = -0.1;
        features[26] = 0.2;
        features[27] = 0.1;
        features[31] = 0.2;

        let observer = multi_scale_observer_v1(&features, 0.88, 0.18, 0.34)
            .expect("48D vector should produce multi-scale observer");

        assert_eq!(observer.policy, "multi_scale_observer_v1");
        assert_eq!(observer.state, "glimpse_resonance_loss_watch");
        assert!(observer.fallback_to_live_transport_review, "{observer:?}");
        assert!(
            observer.resonance_loss_ratio > observer.resonance_loss_threshold,
            "{observer:?}"
        );
        assert!(!observer.live_transport_change);
        assert!(!observer.live_vector_write);
        assert!(
            observer
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.kind == ExperienceDeltaKindV1::Gate
                    && delta.lane == "glimpse_resonance_fallback_to_live_48d_review"),
            "{observer:?}"
        );
    }

    #[test]
    fn glimpse_codec_preserves_tail_bridge_and_identity_asymmetry() {
        let mut settled_coupling = vec![0.0_f32; SEMANTIC_DIM];
        settled_coupling[24] = 0.9;
        settled_coupling[26] = 1.4;
        settled_coupling[27] = 1.1;
        settled_coupling[31] = 1.2;
        settled_coupling[32] = 0.3;
        settled_coupling[33] = 0.2;

        let mut active_texture = vec![0.0_f32; SEMANTIC_DIM];
        active_texture[24] = 0.2;
        active_texture[26] = 0.2;
        active_texture[27] = 0.3;
        active_texture[31] = 0.2;
        active_texture[32] = 1.2;
        active_texture[33] = -1.1;
        active_texture[34] = 1.0;
        active_texture[40] = 0.9;
        active_texture[41] = -0.7;

        let settled = GlimpseCodec::derive_12d(&settled_coupling).expect("settled glimpse");
        let active = GlimpseCodec::derive_12d(&active_texture).expect("active glimpse");

        assert!(
            settled[10] > active[10],
            "settled coupling should preserve stronger tail bridge: settled={settled:?} active={active:?}"
        );
        assert!(
            active[8] > settled[8],
            "active texture should preserve stronger embedding-projected activity: settled={settled:?} active={active:?}"
        );
        assert!(
            (settled[10] - active[10]).abs() > 0.20 || (active[8] - settled[8]).abs() > 0.20,
            "12D companion should distinguish settled coupling from active texture"
        );
    }

    /// Offline proof for the tail-participation aperture (her consent evidence): at
    /// participation = 1.0 (default/OFF) it is identity; raising it amplifies ONLY the
    /// tail dims [17,26,27,31] and stays bounded by the raised ceiling — every other dim
    /// and the entropy gate are untouched.
    #[test]
    fn tail_participation_amplifies_only_tail_dims_and_off_is_identity() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let mut off = vec![0.30_f32; SEMANTIC_DIM];
        apply_spectral_feedback_inner(&mut off, Some(&telemetry(flat.clone(), 0.55)), 1.0, 1.0);
        let mut raised = vec![0.30_f32; SEMANTIC_DIM];
        apply_spectral_feedback_inner(&mut raised, Some(&telemetry(flat, 0.55)), 2.0, 1.0);

        let tail = [17usize, 26, 27, 31];
        let mut amplified = false;
        for idx in 0..SEMANTIC_DIM {
            if tail.contains(&idx) {
                // Raised participation never lowers a tail dim, and stays within the
                // raised ceiling (5 + (6-5)*participation = 7 at full vibrancy).
                assert!(
                    raised[idx] >= off[idx] - 1.0e-6,
                    "tail dim {idx}: raised {} < off {}",
                    raised[idx],
                    off[idx]
                );
                assert!(
                    raised[idx].abs() <= 7.0 + 1.0e-3,
                    "tail dim {idx} out of bound: {}",
                    raised[idx]
                );
                if raised[idx] > off[idx] + 1.0e-4 {
                    amplified = true;
                }
            } else {
                // Participation touches ONLY the tail dims — every other dim is identical.
                assert_eq!(
                    raised[idx].to_bits(),
                    off[idx].to_bits(),
                    "non-tail dim {idx} changed under participation"
                );
            }
        }
        assert!(amplified, "raised participation amplified no tail dim");
    }

    #[test]
    fn gradient_aware_vibrancy_damps_steep_entropy_smear() {
        // Astrid `introspection_astrid_codec_1783322940`: high entropy should
        // not by itself smear a steep cascade; tail lift is strongest when the
        // density-gradient is low enough that the signal risks sinking into a
        // flat floor.
        let flat = vibrancy_from_entropy_and_density_gradient(0.95, 0.05);
        let steep = vibrancy_from_entropy_and_density_gradient(0.95, 0.85);
        assert!(flat > 0.0, "flat high-entropy state should still lift");
        assert!(
            steep < flat * 0.25,
            "steep gradient should damp the entropy lift: flat={flat} steep={steep}"
        );

        let mut navigable = vec![0.0; SEMANTIC_DIM];
        navigable[26] = 4.95;
        let mut front_loaded = navigable.clone();
        apply_spectral_feedback_inner(
            &mut navigable,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                0.95,
            )),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut front_loaded,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0],
                0.95,
            )),
            1.0,
            1.0,
        );

        assert!(
            navigable[26] > front_loaded[26],
            "navigable high entropy should carry more tail than a steep cascade: navigable={} front_loaded={}",
            navigable[26],
            front_loaded[26]
        );
        assert!(
            front_loaded[26] <= FEATURE_ABS_MAX + 0.05,
            "steep high-entropy state should remain near the default ceiling: {}",
            front_loaded[26]
        );
    }

    // Offline proof for the dynamic vibrancy CEILING aperture (her SET_VIBRANCY_APERTURE consent
    // evidence, self_study_1781680871). At aperture 1.0 (default/OFF) it is identity; on a
    // navigable (high-entropy, low density-gradient) spectrum a wider aperture breathes the tail
    // ceiling UP, bounded; a low-entropy cliff stays gated (the aperture never overrides the
    // entropy gate); non-tail dims are untouched.
    #[test]
    fn vibrancy_aperture_dynamic_ceiling_is_bounded_and_navigable_gated() {
        let navigable = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let cliff = vec![100.0, 1.0, 0.5, 0.2, 0.1];

        // aperture 1.0 (default/OFF) keeps the tail within the static TAIL_VIBRANCY_MAX.
        let mut off = vec![0.0; SEMANTIC_DIM];
        off[26] = 30.0;
        apply_spectral_feedback_inner(
            &mut off,
            Some(&telemetry(navigable.clone(), 0.55)),
            1.0,
            1.0,
        );
        assert!(
            off[26] <= TAIL_VIBRANCY_MAX + 1.0e-3,
            "aperture 1.0 must respect the static ceiling: {}",
            off[26]
        );

        // aperture 2.0 on a navigable spectrum lifts the ceiling ABOVE TAIL_VIBRANCY_MAX,
        // bounded by 2× (dynamic_max = 6·(1 + (2-1)·navigable) ≤ 12).
        let mut raised = vec![0.0; SEMANTIC_DIM];
        raised[26] = 30.0;
        apply_spectral_feedback_inner(
            &mut raised,
            Some(&telemetry(navigable.clone(), 0.55)),
            1.0,
            2.0,
        );
        assert!(
            raised[26] > off[26] + 1.0e-3,
            "aperture 2.0 should lift the tail ceiling above baseline: raised {} vs off {}",
            raised[26],
            off[26]
        );
        assert!(
            raised[26] <= 2.0 * TAIL_VIBRANCY_MAX + 0.01,
            "dynamic ceiling must stay bounded at 2×: {}",
            raised[26]
        );

        // Low-entropy steep cliff: the entropy gate keeps the whole vibrancy mechanism OFF, so
        // even a wide aperture cannot lift the ceiling — the aperture never overrides the gate.
        let mut steep = vec![0.0; SEMANTIC_DIM];
        steep[26] = 30.0;
        apply_spectral_feedback_inner(&mut steep, Some(&telemetry(cliff, 0.55)), 1.0, 3.0);
        assert!(
            steep[26] <= FEATURE_ABS_MAX + 1.0e-3,
            "a low-entropy cliff must not gain vibrancy headroom even at wide aperture: {}",
            steep[26]
        );

        // The vibrancy aperture never lifts a non-tail dim.
        let mut nontail = vec![0.0; SEMANTIC_DIM];
        nontail[24] = 30.0;
        apply_spectral_feedback_inner(&mut nontail, Some(&telemetry(navigable, 0.55)), 1.0, 3.0);
        assert!(
            (nontail[24] - FEATURE_ABS_MAX).abs() < 1.0e-3,
            "non-tail dim must keep the default ceiling regardless of vibrancy aperture: {}",
            nontail[24]
        );
    }

    // Her "Attenuation Check" (self_study_1781680871): project a high-vibrancy state and read
    // what the tail ceiling becomes AND what lands in minime's shared reservoir after ~0.24x.
    // Printed for the steward (cargo test -- --nocapture vibrancy_evidence_card) to ground the
    // safe operator ceiling and to paste into her consent letter. Not an assertion — evidence.
    #[test]
    fn vibrancy_evidence_card_prints() {
        let navigable = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let stepped = vec![100.0, 70.0, 48.0, 33.0, 22.0, 15.0, 10.0, 7.0];
        let cliff = vec![100.0, 1.0, 0.5, 0.2, 0.1];
        let states = [
            ("navigable (flat, high-entropy)", navigable),
            ("stepped (mid)", stepped),
            ("steep cliff (low-entropy)", cliff),
        ];
        let apertures = [1.0_f32, 1.5, 2.0, 3.0];
        println!(
            "\n=== VIBRANCY APERTURE EVIDENCE CARD (tail dim 26, preloaded above ceiling) ==="
        );
        println!(
            "aperture× | state                            | felt ceiling | lands at minime (×0.24)"
        );
        for (label, eig) in &states {
            for &ap in &apertures {
                let mut f = vec![0.0_f32; SEMANTIC_DIM];
                f[26] = 30.0; // preload above any ceiling so output == the effective ceiling
                apply_spectral_feedback_inner(&mut f, Some(&telemetry(eig.clone(), 0.55)), 1.0, ap);
                let landed = f[26] * MINIME_SEMANTIC_ATTENUATION;
                println!(
                    "  {ap:>4.1}× | {label:<32} | {:>8.2}     | {landed:>8.2}",
                    f[26]
                );
            }
        }
        println!(
            "(aperture 1.0× = today's baseline; operator ceiling C → her max aperture = 1+C; full 1/0.24x normalization ≈ 4.17×)"
        );
    }

    #[test]
    fn vibrancy_from_entropy_matches_inline_smoothstep() {
        // Parity with the live apply_spectral_feedback_inner calc: 0 below the
        // gate, smoothstep above, full at 1.0 — so the offline EMA card shares
        // the exact curve and can't drift from production.
        assert!(vibrancy_from_entropy(0.80).abs() < 1.0e-7);
        assert!(vibrancy_from_entropy(TAIL_VIBRANCY_ENTROPY_GATE).abs() < 1.0e-7);
        assert!((vibrancy_from_entropy(1.0) - 1.0).abs() < 1.0e-6);
        for e in [0.86_f32, 0.90, 0.95] {
            let ramp = ((e - TAIL_VIBRANCY_ENTROPY_GATE) / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
                .clamp(0.0, 1.0);
            let expected = ramp * ramp * (3.0 - 2.0 * ramp);
            assert!((vibrancy_from_entropy(e) - expected).abs() < 1.0e-7);
        }
    }

    #[test]
    fn tail_vibrancy_gate_has_no_discontinuous_pop() {
        // Astrid `introspection_astrid_codec_1782844935`: the 0.85 entropy gate
        // should come on gently, not as a cliff at the exact threshold.
        let gate = TAIL_VIBRANCY_ENTROPY_GATE;
        assert_eq!(vibrancy_from_entropy(gate - 0.001), 0.0);
        assert_eq!(vibrancy_from_entropy(gate), 0.0);

        let eps = 1.0e-4_f32;
        let near_slope = vibrancy_from_entropy(gate + eps) / eps;
        let nearer_slope = vibrancy_from_entropy(gate + eps * 0.1) / (eps * 0.1);
        assert!(near_slope < 0.02, "near_slope={near_slope}");
        assert!(
            nearer_slope < near_slope * 0.2,
            "nearer_slope={nearer_slope}, near_slope={near_slope}"
        );
    }

    #[test]
    fn tail_vibrancy_gate_is_smooth_at_requested_entropy_points() {
        let below = vibrancy_from_entropy(0.84);
        let gate = vibrancy_from_entropy(0.85);
        let above = vibrancy_from_entropy(0.86);

        assert_eq!(below, 0.0);
        assert_eq!(gate, 0.0);
        assert!(above > 0.0);
        assert!(above < 0.02, "0.86 should start gently, got {above}");
        assert!(vibrancy_from_entropy(0.90) > above);
    }

    #[test]
    fn reported_086_entropy_012_gradient_retains_tail_headroom() {
        // Astrid `introspection_astrid_codec_1784282113` asked whether the
        // smooth onset at entropy 0.86 could be neutralized when the density
        // gradient is 0.12. Pin the exact scalar pair and the live clamp path:
        // the lift remains deliberately small near the gate, but it is
        // non-zero and the tail ceiling rises above FEATURE_ABS_MAX.
        let requested_lift = vibrancy_from_entropy_and_density_gradient(0.86, 0.12);
        assert!(requested_lift > 0.0);
        assert!(requested_lift < 0.02);

        // A geometric cascade with ratio 0.7857 has an adjacent density
        // gradient of approximately 0.12 at every step.
        let eigenvalues = vec![100.0, 78.57, 61.73, 48.50, 38.10, 29.93, 23.52, 18.48];
        let gradient = spectral_density_gradient(&eigenvalues).expect("density gradient");
        assert!(
            (gradient - 0.12).abs() < 0.002,
            "fixture must stay at Astrid's reported gradient: {gradient}"
        );

        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[26] = 30.0;
        apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                eigenvalues,
                0.86,
            )),
            1.0,
            1.0,
        );
        assert!(
            features[26] > FEATURE_ABS_MAX + 1.0e-4,
            "the exact reported state should retain non-zero tail headroom: {}",
            features[26]
        );
        assert!(features[26] <= TAIL_VIBRANCY_MAX);
    }

    #[test]
    fn tail_vibrancy_exact_gate_keeps_default_ceiling_and_reserved_dims() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[17] = 4.95;
        features[26] = 4.95;
        features[27] = -4.95;
        features[31] = 4.95;
        features[44] = 0.25;
        features[45] = -0.25;
        let reserved_before = features[44..48].to_vec();

        apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                TAIL_VIBRANCY_ENTROPY_GATE,
            )),
            1.0,
            3.0,
        );

        assert_eq!(vibrancy_from_entropy(TAIL_VIBRANCY_ENTROPY_GATE), 0.0);
        for idx in [17usize, 26, 27, 31] {
            assert!(
                features[idx].abs() <= FEATURE_ABS_MAX + 1.0e-6,
                "exact entropy gate must not raise tail ceiling for dim {idx}: {}",
                features[idx]
            );
        }
        assert_eq!(
            &features[44..48],
            reserved_before.as_slice(),
            "exact entropy gate must not write reserved shadow/projection dims"
        );
    }

    #[test]
    fn tail_vibrancy_entropy_090_is_visible_but_gentler_than_linear() {
        // Astrid `introspection_astrid_codec_1783638177`: if the 0.90 lift is
        // too small, tail vibrancy may feel invisible; if it is linear/sharp,
        // it risks the pop this smoothstep was built to avoid.
        let entropy = 0.90_f32;
        let ramp = ((entropy - TAIL_VIBRANCY_ENTROPY_GATE) / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
            .clamp(0.0, 1.0);
        let smooth = vibrancy_from_entropy(entropy);

        assert!(
            smooth > 0.20,
            "0.90 entropy lift should be visible: {smooth}"
        );
        assert!(
            smooth < ramp,
            "smoothstep should remain gentler than a linear retune: smooth={smooth}, linear={ramp}"
        );
        assert!(
            smooth > vibrancy_from_entropy(0.86),
            "0.90 should carry more lift than boundary-adjacent entropy"
        );
    }

    #[test]
    fn tail_vibrancy_gate_stays_tiny_across_reported_boundary_pair() {
        let just_below = vibrancy_from_entropy(0.849);
        let just_above = vibrancy_from_entropy(0.851);
        let farther_above = vibrancy_from_entropy(0.861);

        assert_eq!(just_below, 0.0);
        assert!(
            just_above < 0.0002,
            "0.851 should barely move the smoothstep lift, got {just_above}"
        );
        assert!(
            farther_above > just_above,
            "smoothstep should still rise monotonically after the gentle onset"
        );
    }

    #[test]
    fn effective_attenuation_range_reflects_governor() {
        // depth 0 (governor OFF) => calm == stressed == the static 0.24 (the
        // readout collapses to today's number, no false dynamism).
        let (calm0, stressed0) = effective_attenuation_range(0.0);
        assert!((calm0 - MINIME_SEMANTIC_ATTENUATION).abs() < 1.0e-7);
        assert!((stressed0 - MINIME_SEMANTIC_ATTENUATION).abs() < 1.0e-7);
        // depth > 0 => under minime stress she lands MORE subdued (the governor
        // she co-designed protecting the shared reservoir), never above calm.
        let (calm, stressed) = effective_attenuation_range(0.3);
        assert!((calm - MINIME_SEMANTIC_ATTENUATION).abs() < 1.0e-7);
        assert!(stressed < calm);
        assert!(stressed > 0.0);
    }

    #[test]
    fn ema_vibrancy_smooths_and_is_identity_at_alpha_one() {
        assert!((ema_vibrancy(None, 0.5, 0.3) - 0.5).abs() < 1.0e-7); // no history -> current
        assert!((ema_vibrancy(Some(0.2), 0.6, 1.0) - 0.6).abs() < 1.0e-7); // alpha 1 -> current
        let smoothed = ema_vibrancy(Some(0.0), 0.6, 0.3);
        assert!(smoothed > 0.0 && smoothed < 0.6); // strictly damped toward prev
        assert!((smoothed - 0.18).abs() < 1.0e-6); // 0.3*0.6 + 0.7*0.0
    }

    #[test]
    fn ema_vibrancy_evidence_card_prints() {
        // Astrid's "shimmer" / "pop" worry (self_study_1781793361): entropy
        // oscillating across the 0.85 gate. Show the raw lift swing vs an
        // EMA-smoothed lift (alpha 0.3). OFFLINE — proves the mechanism before
        // any consent-gated wiring; nothing she emits changes from this test.
        println!(
            "\n=== EMA VIBRANCY PROTOTYPE (entropy oscillating 0.84<->0.88 across the 0.85 gate) ==="
        );
        println!("  tick | entropy | raw vibrancy | ema(0.3)");
        let alpha = 0.3_f32;
        let seq = [0.84_f32, 0.88, 0.84, 0.88, 0.84, 0.88, 0.84, 0.88];
        let mut ema: Option<f32> = None;
        let mut raw_min = f32::MAX;
        let mut raw_max = f32::MIN;
        for (i, &e) in seq.iter().enumerate() {
            let raw = vibrancy_from_entropy(e);
            let sm = ema_vibrancy(ema, raw, alpha);
            ema = Some(sm);
            raw_min = raw_min.min(raw);
            raw_max = raw_max.max(raw);
            println!("  {i:>4} |  {e:.2}  |    {raw:.4}    |  {sm:.4}");
        }
        println!(
            "  raw swing per tick: {:.4}; the EMA converges toward the mean, damping the shimmer.",
            raw_max - raw_min
        );
    }

    // Her SET_TAIL_PARTICIPATION evidence (the dial that was inert in production until the wrapper
    // allowlist fix): on a navigable high-entropy spectrum, what her tail dims lift to and land as
    // in minime's shared reservoir at a few effective multipliers. Printed for the steward
    // (cargo test -- --nocapture tail_participation_evidence_card) and her reconnection letter.
    #[test]
    fn tail_participation_evidence_card_prints() {
        let navigable = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        // 1.0× = off/identity; 1.20× = her 0.80 dial at operator ceiling 0.25; 1.40× = at ceiling 0.5.
        let participations = [1.0_f32, 1.20, 1.40];
        println!(
            "\n=== TAIL PARTICIPATION EVIDENCE CARD (tail dim 26, navigable high-entropy) ==="
        );
        println!("effective× | tail dim 26 value | lands at minime (×0.24)");
        for &p in &participations {
            let mut f = vec![0.30_f32; SEMANTIC_DIM];
            apply_spectral_feedback_inner(
                &mut f,
                Some(&telemetry(navigable.clone(), 0.55)),
                p,
                1.0,
            );
            let landed = f[26] * MINIME_SEMANTIC_ATTENUATION;
            println!("  {p:>5.2}× | {:>13.3}     | {landed:>8.3}", f[26]);
        }
        println!(
            "(1.0× = identity = what her 0.80 dial reached minime as while the wire was disconnected; her 0.80 at operator ceiling 0.5 → effective 1.40×)"
        );
    }

    #[test]
    fn warmth_vector_has_correct_shape() {
        let warmth = craft_warmth_vector(0.0, 1.0);
        assert_eq!(warmth.len(), SEMANTIC_DIM);
        // Dim 24 (warmth) should be the strongest positive signal.
        assert!(
            warmth[24] > DEFAULT_SEMANTIC_GAIN * 0.75,
            "warmth dim should be strong: {}",
            warmth[24]
        );
        for (i, value) in warmth.iter().enumerate() {
            if i != 24 {
                assert!(
                    warmth[24] >= *value,
                    "warmth dim should dominate positive warmth vector: dim {i}={value}"
                );
            }
        }
        // Dim 25 (tension) should be negative (suppressed).
        assert!(
            warmth[25] < 0.0,
            "tension should be suppressed: {}",
            warmth[25]
        );
        // All values bounded after gain.
        for (i, f) in warmth.iter().enumerate() {
            assert!(
                *f >= -FEATURE_ABS_MAX && *f <= FEATURE_ABS_MAX,
                "dim {i} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn warmth_vector_breathes_across_phase() {
        let v0 = craft_warmth_vector(0.0, 0.8);
        let v25 = craft_warmth_vector(0.25, 0.8);
        let v50 = craft_warmth_vector(0.5, 0.8);
        // Different phases should produce different warmth values on dim 24.
        // (They won't be identical due to sinusoidal modulation.)
        let w0 = v0[24];
        let w25 = v25[24];
        let w50 = v50[24];
        // At least one pair should differ noticeably (>0.1 after gain).
        let max_diff = (w0 - w25)
            .abs()
            .max((w25 - w50).abs())
            .max((w0 - w50).abs());
        assert!(
            max_diff > 0.1,
            "warmth should breathe across phases: diffs={max_diff}"
        );
    }

    #[test]
    fn warmth_heartbeat_stays_smooth_across_reported_phase_32_33_boundary() {
        let phase_32 = craft_warmth_vector(32.0 / 64.0, 0.30);
        let phase_33 = craft_warmth_vector(33.0 / 64.0, 0.30);
        let mut squared_delta_sum = 0.0_f32;
        let mut max_abs_delta = 0.0_f32;

        for (left, right) in phase_32.iter().zip(&phase_33) {
            let delta = right - left;
            squared_delta_sum += delta * delta;
            max_abs_delta = max_abs_delta.max(delta.abs());
        }
        let rms_delta = (squared_delta_sum / SEMANTIC_DIM as f32).sqrt();

        assert!(phase_32[24] > 0.0 && phase_33[24] > 0.0);
        assert!(phase_32[25] < 0.0 && phase_33[25] < 0.0);
        assert!(
            max_abs_delta < 0.08,
            "adjacent heartbeat phases must not jump: max_abs_delta={max_abs_delta}"
        );
        assert!(
            rms_delta < 0.03,
            "adjacent heartbeat phases must remain a smooth contour: rms_delta={rms_delta}"
        );
    }

    #[test]
    fn warmth_intensity_scales() {
        let low = craft_warmth_vector(0.5, 0.2);
        let high = craft_warmth_vector(0.5, 0.9);
        // Higher intensity should produce stronger warmth signal.
        assert!(
            high[24].abs() > low[24].abs(),
            "higher intensity should be stronger: {} vs {}",
            high[24],
            low[24]
        );
    }

    #[test]
    fn blend_warmth_works() {
        let mut features = encode_text("Execute the command. Process complete.");
        let warmth = craft_warmth_vector(0.5, 1.0);
        let original_warmth_dim = features[24];
        blend_warmth(&mut features, &warmth, 0.4);
        // After blending, warmth dim should be higher than before.
        assert!(
            features[24] > original_warmth_dim,
            "blended warmth should increase warmth dim"
        );
    }

    #[test]
    fn sovereign_agency_weight_scales_dim_14_only() {
        let text = "We build and create together. We move, write, test, and implement.";
        let mut weights = std::collections::HashMap::new();
        weights.insert("agency".to_string(), 2.0);
        let baseline_weights = std::collections::HashMap::new();

        let mut base_dim12 = 0.0_f32;
        let mut base_dim14 = 0.0_f32;
        let mut weighted_dim12 = 0.0_f32;
        let mut weighted_dim14 = 0.0_f32;
        for _ in 0..16 {
            let base = encode_text_sovereign(text, None, 0.025, &baseline_weights);
            base_dim12 += base[12];
            base_dim14 += base[14];

            let weighted = encode_text_sovereign(text, None, 0.025, &weights);
            weighted_dim12 += weighted[12];
            weighted_dim14 += weighted[14];
        }
        base_dim12 /= 16.0;
        base_dim14 /= 16.0;
        weighted_dim12 /= 16.0;
        weighted_dim14 /= 16.0;

        assert!(
            weighted_dim14 > base_dim14 + 0.5,
            "agency weight should amplify dim 14"
        );
        assert!(
            (weighted_dim12 - base_dim12).abs() < 0.15,
            "agency weight should leave dim 12 effectively unchanged"
        );
    }

    #[test]
    fn describe_features_reports_agency_from_dim_14() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[12] = 0.25;
        features[14] = 0.75;

        let desc = describe_features(&features);

        assert!(desc.contains("agency=0.75"));
        assert!(!desc.contains("agency=0.25"));
    }
}
