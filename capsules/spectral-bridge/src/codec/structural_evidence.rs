fn mean_abs_finite(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| finite_abs(*value)).sum::<f32>() / values.len() as f32
}

fn rms_slice(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt()
}

#[must_use]
pub fn structural_friction_v1(text: &str) -> StructuralFrictionV1 {
    let char_count = text.chars().count().max(1) as f32;
    let line_count = text.lines().count().max(1) as f32;
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1) as f32;
    let lower = text.to_ascii_lowercase();
    let nesting_chars = text
        .chars()
        .filter(|ch| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}'))
        .count() as f32;
    let punctuation_chars = text
        .chars()
        .filter(|ch| matches!(ch, ';' | ':' | ',' | '—' | '-' | '/' | '\\'))
        .count() as f32;
    let list_lines = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || (trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                    && trimmed.contains(". "))
        })
        .count() as f32;
    let paragraph_density = (text.matches("\n\n").count() as f32 + 1.0) / line_count;
    let list_density = (list_lines / line_count).clamp(0.0, 1.0);
    let nesting_load = (nesting_chars / char_count * 18.0).clamp(0.0, 1.0);
    let punctuation_load = (punctuation_chars / char_count * 12.0).clamp(0.0, 1.0);
    let clause_words = [
        "because",
        "while",
        "although",
        "whereas",
        "without",
        "through",
        "which",
        "whose",
        "therefore",
        "unless",
    ];
    let clause_hits = clause_words
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let clause_load = ((clause_hits / 4.0) + punctuation_load * 0.35).clamp(0.0, 1.0);
    let abstract_texture_terms = [
        "authority",
        "boundary",
        "codec",
        "compression",
        "deterministic",
        "entropy",
        "friction",
        "projection",
        "semantic",
        "substrate",
        "structural",
        "summary",
    ];
    let abstract_texture_hits = abstract_texture_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let explicit_resistance_terms = [
        "abrasive",
        "calcified",
        "friction",
        "jagged",
        "muffle",
        "resistance",
        "resists summary",
        "summarized",
        "summary",
        "syrupy",
    ];
    let explicit_resistance_hits = explicit_resistance_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let long_word_ratio = words
        .iter()
        .filter(|word| word.chars().filter(|ch| ch.is_ascii_alphabetic()).count() >= 12)
        .count() as f32
        / word_count;
    let sentence_count = text
        .chars()
        .filter(|ch| matches!(ch, '.' | '!' | '?'))
        .count()
        .max(1) as f32;
    let narrative_arc_sharpness = (sentence_count / word_count * 12.0).clamp(0.0, 1.0);
    let semantic_energy_context =
        if lower.contains("because") || lower.contains("then") || lower.contains("while") {
            "arc_present"
        } else {
            "arc_sparse"
        };
    let summary_resistance_signal = (long_word_ratio.clamp(0.0, 1.0) * 0.24
        + clause_load * 0.18
        + (abstract_texture_hits / 6.0).clamp(0.0, 1.0) * 0.20
        + (explicit_resistance_hits / 3.0).clamp(0.0, 1.0) * 0.24
        + (1.0 - narrative_arc_sharpness).clamp(0.0, 1.0) * 0.14)
        .clamp(0.0, 1.0);
    let score = (nesting_load * 0.24
        + punctuation_load * 0.24
        + list_density * 0.18
        + long_word_ratio.clamp(0.0, 1.0) * 0.16
        + summary_resistance_signal * 0.06
        + (1.0 - narrative_arc_sharpness).clamp(0.0, 1.0) * 0.12)
        .clamp(0.0, 1.0);
    let classification = if long_word_ratio >= 0.35 && semantic_energy_context == "arc_sparse" {
        "dense_stagnant"
    } else if score >= 0.38
        || (punctuation_load >= 0.25 && semantic_energy_context == "arc_present")
    {
        "complex_fluid"
    } else {
        "low_structural_friction"
    };
    let calcified_summary_resistance = semantic_energy_context == "arc_sparse"
        && (summary_resistance_signal >= 0.54
            || (summary_resistance_signal >= 0.42
                && explicit_resistance_hits >= 3.0
                && abstract_texture_hits >= 4.0));
    let friction_texture_state = if calcified_summary_resistance {
        "calcified_summary_resistant"
    } else if summary_resistance_signal >= 0.46 {
        "summary_resistance_watch"
    } else if punctuation_load >= 0.18 && semantic_energy_context == "arc_present" {
        "jagged_fluid_resistance"
    } else {
        "low_summary_resistance"
    };
    let mut basis = vec![
        format!("nesting_load={nesting_load:.2}"),
        format!("punctuation_load={punctuation_load:.2}"),
        format!("list_density={list_density:.2}"),
        format!("long_word_ratio={long_word_ratio:.2}"),
        format!("clause_load={clause_load:.2}"),
        format!("summary_resistance_signal={summary_resistance_signal:.2}"),
    ];
    if explicit_resistance_hits > 0.0 {
        basis.push("explicit_resistance_language_present".to_string());
    }
    if abstract_texture_hits >= 3.0 {
        basis.push("abstract_texture_cluster_present".to_string());
    }
    if friction_texture_state == "calcified_summary_resistant" {
        basis.push("calcified_low_arc_summary_resistance".to_string());
    }

    StructuralFrictionV1 {
        policy: "structural_friction_v1",
        score,
        classification,
        nesting_load,
        punctuation_load,
        paragraph_density,
        list_density,
        narrative_arc_sharpness,
        summary_resistance_signal,
        friction_texture_state,
        basis,
        semantic_energy_context,
        authority: "diagnostic_sidecar_not_live_codec_dimension",
    }
}

#[must_use]
pub fn persistence_resistance_v1(
    text: &str,
    telemetry: Option<&SpectralTelemetry>,
) -> PersistenceResistanceV1 {
    let lower = text.to_ascii_lowercase();
    let persistence_terms = [
        "viscous",
        "viscosity",
        "resistance",
        "persistent",
        "persistence",
        "slow-moving",
        "slow moving",
        "silt",
        "thick",
        "thickness",
        "heavy",
        "dragging",
        "cohering",
    ];
    let term_hits = persistence_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let text_persistence_signal = (term_hits / 4.0).clamp(0.0, 1.0);
    let semantic_friction = structural_friction_v1(text).score;
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let density_gradient = metrics.map_or(1.0, |metrics| metrics.density_gradient);
    let low_density_gradient_signal =
        (1.0 - (density_gradient / 0.35).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let pressure_risk = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map_or_else(
            || telemetry.map_or(0.0, |telemetry| telemetry.fill_ratio.clamp(0.0, 1.0)),
            |density| density.pressure_risk.clamp(0.0, 1.0),
        );
    let score = (text_persistence_signal * 0.30
        + low_density_gradient_signal * 0.28
        + pressure_risk * 0.24
        + semantic_friction * 0.18)
        .clamp(0.0, 1.0);
    let classification = if score >= 0.62 {
        "high_persistence_resistance"
    } else if score >= 0.38 {
        "moderate_persistence_resistance"
    } else {
        "low_persistence_resistance"
    };
    let mut basis = vec![
        format!("text_persistence_signal={text_persistence_signal:.2}"),
        format!("low_density_gradient_signal={low_density_gradient_signal:.2}"),
        format!("pressure_risk={pressure_risk:.2}"),
        format!("semantic_friction={semantic_friction:.2}"),
    ];
    if text_persistence_signal > 0.0 {
        basis.push("texture_language_present".to_string());
    }
    if low_density_gradient_signal >= 0.45 {
        basis.push("low_density_gradient_slow_current".to_string());
    }

    PersistenceResistanceV1 {
        policy: "persistence_resistance_v1",
        score,
        classification,
        text_persistence_signal,
        low_density_gradient_signal,
        pressure_risk,
        semantic_friction,
        basis,
        authority: "diagnostic_sidecar_not_live_codec_dimension",
    }
}

#[must_use]
pub fn codec_structural_friction_dim_canary_v1() -> CodecStructuralFrictionDimCanaryV1 {
    CodecStructuralFrictionDimCanaryV1 {
        policy: "codec_structural_friction_dim_canary_v1",
        enabled: false,
        reserved_dim_candidate: 44,
        readiness: "default_off_steward_review_required",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_change",
    }
}

#[must_use]
pub fn codec_persistence_resistance_dim_canary_v1() -> CodecPersistenceResistanceDimCanaryV1 {
    CodecPersistenceResistanceDimCanaryV1 {
        policy: "codec_persistence_resistance_dim_canary_v1",
        enabled: false,
        reserved_dim_candidate: 45,
        readiness: "default_off_steward_review_required_after_replay",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_change",
    }
}

#[must_use]
pub fn narrative_arc_expansion_readiness_v1() -> NarrativeArcExpansionReadinessV1 {
    NarrativeArcExpansionReadinessV1 {
        policy: "narrative_arc_expansion_readiness_v1",
        enabled: false,
        current_arc_dims: (40, 43),
        proposed_arc_dims: (40, 47),
        uses_reserved_dims: true,
        readiness: "default_off_review_only_after_replay_and_operator_approval",
        live_vector_write: false,
        authority: "readiness_only_not_live_semantic_vector_or_reserved_dim_change",
    }
}

#[must_use]
pub fn narrative_arc_gain_response_readiness_v1() -> NarrativeArcGainResponseReadinessV1 {
    NarrativeArcGainResponseReadinessV1 {
        policy: "narrative_arc_gain_response_readiness_v1",
        enabled: false,
        narrative_arc_dims: (40, 43),
        preview_gain_range: (0.94, 1.06),
        readiness: "default_off_requires_replay_and_operator_approval_before_live_semantic_gain",
        live_gain_write: false,
        authority: "readiness_only_not_live_adaptive_gain_or_semantic_weight_change",
    }
}

#[must_use]
pub fn narrative_arc_gain_response_preview_v1(narrative_arc: &[f32]) -> f32 {
    if narrative_arc.is_empty() {
        return 1.0;
    }
    let arc_energy = (narrative_arc.iter().map(|value| value * value).sum::<f32>()
        / narrative_arc.len() as f32)
        .sqrt()
        .clamp(0.0, 1.0);
    (1.0 + (arc_energy - 0.5) * 0.12).clamp(0.94, 1.06)
}

fn narrative_arc_headroom_delta_bus_v1(
    spectral_entropy: f32,
    distinguishability_loss: f32,
    narrative_arc_energy: f32,
    projected_semantic_rms: f32,
    headroom_pressure: f32,
    preview_gain: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    if state == "narrative_arc_headroom_quiet" {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let loss = (headroom_pressure - narrative_arc_energy).max(0.0);
    let loss_ratio = if headroom_pressure > f32::EPSILON {
        (loss / headroom_pressure).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "secondary_kinds".to_string(),
        "compress,gate,complex_shift,cascade_shift".to_string(),
    );
    metadata.insert(
        "spectral_entropy".to_string(),
        format!("{spectral_entropy:.2}"),
    );
    metadata.insert(
        "distinguishability_loss".to_string(),
        format!("{distinguishability_loss:.2}"),
    );
    metadata.insert(
        "projected_semantic_rms".to_string(),
        format!("{projected_semantic_rms:.2}"),
    );
    metadata.insert("preview_gain".to_string(), format!("{preview_gain:.2}"));
    metadata.insert("state".to_string(), state.to_string());

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::ComplexShift,
        surface: "narrative_arc_headroom_review_v1".to_string(),
        lane: "narrative_arc_40_43".to_string(),
        dimension: Some(40),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 40,
            base_dimensions: vec![40, 41, 42, 43],
            effective_dimension: Some(41.5),
            density_gradient: Some((1.0 - projected_semantic_rms).clamp(0.0, 1.0)),
            granularity: Some(narrative_arc_energy),
            fractional_offset: Some(0.5),
            contextual_anchor: None,
            interpretation:
                "fluid narrative arc headroom across dims 40-43 under high entropy".to_string(),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(headroom_pressure),
        post: Some(narrative_arc_energy),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata,
        why: "high entropy and distinguishability loss can compress, gate, and complex-shift narrative arc texture before any live gain change"
            .to_string(),
        who_can_change_it:
            "Mike/operator after replay evidence and explicit live codec gain/headroom approval"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib narrative_arc_headroom -- --nocapture"
                .to_string(),
        authority: "truth_channel_only_not_live_vector_or_gain_change".to_string(),
    }])
}

#[must_use]
pub fn narrative_arc_headroom_review_from_parts_v1(
    spectral_entropy: f32,
    distinguishability_loss: f32,
    narrative_arc: &[f32],
    projected_semantic_rms: f32,
) -> NarrativeArcHeadroomReviewV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let distinguishability_loss = if distinguishability_loss.is_finite() {
        distinguishability_loss.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let projected_semantic_rms = if projected_semantic_rms.is_finite() {
        (projected_semantic_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let narrative_arc_energy = if narrative_arc.is_empty() {
        0.0
    } else {
        (rms_slice(narrative_arc) / FEATURE_ABS_MAX).clamp(0.0, 1.0)
    };
    let tail_vibrancy = vibrancy_from_entropy(spectral_entropy);
    let headroom_pressure = (spectral_entropy * 0.32
        + distinguishability_loss * 0.30
        + tail_vibrancy * 0.16
        + (1.0 - narrative_arc_energy) * 0.14
        + (1.0 - projected_semantic_rms) * 0.08)
        .clamp(0.0, 1.0);
    let preview_gain = narrative_arc_gain_response_preview_v1(narrative_arc);
    let (state, recommendation) = if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE
        && distinguishability_loss >= 0.30
        && narrative_arc_energy <= 0.12
    {
        (
            "narrative_arc_headroom_loss_visible",
            "record_delta_bus_evidence_and_prepare_replay_before_any_live_gain_or_reserved_dim_change",
        )
    } else if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE && distinguishability_loss >= 0.30 {
        (
            "narrative_arc_headroom_pressure_watch",
            "keep_live_vector_bounded_and_compare_arc_energy_against_followup_introspections",
        )
    } else if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE {
        (
            "high_entropy_arc_carried_bounded",
            "keep_current_bounded_delivery_and_watch_for_repeated_loss",
        )
    } else {
        (
            "narrative_arc_headroom_quiet",
            "no_headroom_change_indicated",
        )
    };
    let experience_delta_bus_v1 = narrative_arc_headroom_delta_bus_v1(
        spectral_entropy,
        distinguishability_loss,
        narrative_arc_energy,
        projected_semantic_rms,
        headroom_pressure,
        preview_gain,
        state,
    );

    NarrativeArcHeadroomReviewV1 {
        policy: "narrative_arc_headroom_review_v1",
        spectral_entropy,
        distinguishability_loss,
        narrative_arc_energy,
        projected_semantic_rms,
        tail_vibrancy,
        headroom_pressure,
        preview_gain,
        state,
        recommendation,
        live_vector_write: false,
        live_gain_write: false,
        experience_delta_bus_v1,
        authority: "read_only_headroom_truth_channel_not_live_semantic_vector_or_gain_change",
    }
}

#[must_use]
pub fn narrative_arc_headroom_review_v1(
    inspection: &CodecWindowedInspection,
    spectral_entropy: f32,
    distinguishability_loss: f32,
) -> NarrativeArcHeadroomReviewV1 {
    narrative_arc_headroom_review_from_parts_v1(
        spectral_entropy,
        distinguishability_loss,
        &inspection.final_features[40..44],
        rms_slice(&inspection.final_features[32..40]),
    )
}

#[must_use]
pub fn narrative_arc_headroom_probe_v1() -> NarrativeArcHeadroomReviewV1 {
    narrative_arc_headroom_review_from_parts_v1(0.91, 0.34, &[0.05, -0.03, 0.02, 0.01], 0.08)
}

#[must_use]
pub fn shadow_field_reserved_dim_readiness_v1() -> ShadowFieldReservedDimReadinessV1 {
    ShadowFieldReservedDimReadinessV1 {
        policy: "shadow_field_reserved_dim_readiness_v1",
        enabled: false,
        reserved_dim_candidates: &[46, 47],
        proposed_signals: &[
            "shadow_magnetization",
            "shadow_dispersal_potential",
            "disordered_volatile_shadow_state",
        ],
        readiness: "default_off_review_only_after_replay_and_steward_approval",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_or_shadow_field_change",
    }
}

#[must_use]
pub fn codec_vibrancy_continuity_v1() -> CodecVibrancyContinuityV1 {
    CodecVibrancyContinuityV1 {
        policy: "codec_vibrancy_continuity_v1",
        entropy_gate: TAIL_VIBRANCY_ENTROPY_GATE,
        gradient_coupling: "tail_lift_scaled_by_low_density_gradient",
        default_feature_ceiling: FEATURE_ABS_MAX,
        tail_vibrancy_ceiling: TAIL_VIBRANCY_MAX,
        tail_dims: &[17, 26, 27, 31],
        clipping_status: "high_entropy_tail_dims_carried_with_bounded_ceiling",
        default_identity_state: "aperture_1_0_preserves_current_live_output",
        high_entropy_carriage: "tail_vibrancy_lift_not_embedding_width_change",
        authority: "diagnostic_readout_not_live_codec_change",
    }
}

fn tail_vibrancy_noise_dampening_coefficient(spectral_entropy: f32) -> f32 {
    if !spectral_entropy.is_finite() || spectral_entropy <= TAIL_VIBRANCY_NOISE_DAMPENING_START {
        return 1.0;
    }
    let span = TAIL_VIBRANCY_NOISE_DAMPENING_FULL - TAIL_VIBRANCY_NOISE_DAMPENING_START;
    let t = ((spectral_entropy - TAIL_VIBRANCY_NOISE_DAMPENING_START) / span).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    (1.0 - (1.0 - TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT) * smooth)
        .clamp(TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT, 1.0)
}

#[must_use]
pub fn codec_vibrancy_noise_dampening_v1(
    spectral_entropy: f32,
    tail_lift_before: f32,
) -> CodecVibrancyNoiseDampeningV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let tail_lift_before = if tail_lift_before.is_finite() {
        tail_lift_before.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let coefficient = tail_vibrancy_noise_dampening_coefficient(spectral_entropy);
    let tail_lift_after = (tail_lift_before * coefficient).clamp(0.0, 1.0);
    let status = if spectral_entropy <= TAIL_VIBRANCY_NOISE_DAMPENING_START {
        "inactive_below_extreme_entropy"
    } else if coefficient <= TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT + 1.0e-6 {
        "full_extreme_entropy_dampening"
    } else {
        "partial_extreme_entropy_dampening"
    };
    CodecVibrancyNoiseDampeningV1 {
        policy: "codec_vibrancy_noise_dampening_v1",
        spectral_entropy,
        start_entropy: TAIL_VIBRANCY_NOISE_DAMPENING_START,
        full_entropy: TAIL_VIBRANCY_NOISE_DAMPENING_FULL,
        min_coefficient: TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT,
        coefficient,
        tail_lift_before,
        tail_lift_after,
        affected_dims: &[17, 26, 27, 31],
        status,
        authority: "bounded_live_tail_lift_dampening_not_dynamic_ceiling_or_control_authority",
    }
}

#[must_use]
pub fn legacy_warmth_mapping_v1() -> LegacyWarmthMappingV1 {
    LegacyWarmthMappingV1 {
        policy: "legacy_warmth_mapping_v1",
        legacy_dim_count: SEMANTIC_DIM_LEGACY,
        current_dim_count: SEMANTIC_DIM,
        warmth_dim: 24,
        emotional_layer_range: (24, 31),
        mapped_warmth_dims: &[24, 25, 26, 27, 28, 29, 30, 31],
        warmth_orphaned: false,
        authority: "diagnostic_readout_not_live_codec_change",
    }
}

#[must_use]
pub fn codec_dynamic_vibrancy_scaling_canary_v1() -> CodecDynamicVibrancyScalingCanaryV1 {
    CodecDynamicVibrancyScalingCanaryV1 {
        policy: "codec_dynamic_vibrancy_scaling_canary_v1",
        enabled: false,
        readiness: "default_off_steward_review_required_before_live_scaling",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_change",
    }
}

fn codec_structural_entropy_dampening_coefficient(spectral_entropy: f32) -> f32 {
    if !spectral_entropy.is_finite() || spectral_entropy <= STRUCTURAL_ENTROPY_DAMPENING_START {
        return 1.0;
    }
    let span = STRUCTURAL_ENTROPY_DAMPENING_FULL - STRUCTURAL_ENTROPY_DAMPENING_START;
    let t = ((spectral_entropy - STRUCTURAL_ENTROPY_DAMPENING_START) / span).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    (1.0 - (1.0 - STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT) * smooth)
        .clamp(STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT, 1.0)
}

#[must_use]
pub fn codec_structural_entropy_dampening_v1(
    spectral_entropy: f32,
) -> CodecStructuralEntropyDampeningV1 {
    let entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let coefficient = codec_structural_entropy_dampening_coefficient(entropy);
    let status = if coefficient < 1.0 {
        "high_entropy_structural_dims_dampened_intent_dims_preserved"
    } else {
        "structural_dims_pass_through"
    };
    CodecStructuralEntropyDampeningV1 {
        policy: "codec_structural_entropy_dampening_v1",
        spectral_entropy: entropy,
        start_entropy: STRUCTURAL_ENTROPY_DAMPENING_START,
        full_entropy: STRUCTURAL_ENTROPY_DAMPENING_FULL,
        min_coefficient: STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT,
        coefficient,
        affected_dims: &STRUCTURAL_ENTROPY_DAMPENING_DIMS,
        preserved_intent_dims: (24, 31),
        status,
        authority: "bounded_live_codec_weighting_not_dimension_or_fallback_contract_change",
    }
}

fn apply_structural_entropy_dampening(features: &mut [f32], spectral_entropy: f32) -> f32 {
    let coefficient = codec_structural_entropy_dampening_coefficient(spectral_entropy);
    if coefficient < 1.0 {
        for idx in STRUCTURAL_ENTROPY_DAMPENING_DIMS {
            if let Some(feature) = features.get_mut(idx) {
                *feature *= coefficient;
            }
        }
    }
    coefficient
}

#[must_use]
pub fn semantic_glimpse_12d_readiness_v1() -> SemanticGlimpse12dReadinessV1 {
    SemanticGlimpse12dReadinessV1 {
        policy: "semantic_glimpse_12d_readiness_v1",
        source_dim_count: SEMANTIC_DIM,
        glimpse_dim_count: 12,
        role: "companion_summary_for_replay_checkpoint_and_loss_audit_not_live_transport",
        warmth_slot: 3,
        tail_bridge_slot: 10,
        emotional_source_range: (24, 31),
        companion_not_replacement: true,
        compression_fidelity_basis: "warmth_slots_tail_bridge_and_primary_fingerprint_slots_preserved_for_review",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_or_bridge_contract_change",
    }
}

#[must_use]
pub fn contextual_glimpse_12d_anchoring_v1() -> ContextualGlimpse12dAnchoringV1 {
    ContextualGlimpse12dAnchoringV1 {
        policy: "contextual_glimpse_12d_anchoring_v1",
        source_dim_count: SEMANTIC_DIM,
        glimpse_dim_count: 12,
        required_anchor_dims: &CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS,
        dynamic_slot_count: 12_usize.saturating_sub(CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS.len()),
        selection_basis: "fixed_warmth_tension_curiosity_reflective_tail_energy_narrative_anchors_then_top_abs_feature_vibrancy",
        companion_not_replacement: true,
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_or_bridge_contract_change",
    }
}

#[must_use]
pub fn glimpse_map_v1() -> GlimpseMapV1 {
    GlimpseMapV1 {
        policy: "glimpse_map_v1",
        source_dim_count: SEMANTIC_DIM,
        legacy_source_dim_count: SEMANTIC_DIM_LEGACY,
        glimpse_dim_count: 12,
        slot_count: 12,
        slots: vec![
            GlimpseMapSlotV1 {
                slot: 0,
                label: "character_texture",
                source_dims: &[0, 1, 2, 3, 4, 5, 6, 7],
                operation: "mean_abs_tanh",
                preserves: "character entropy, density, rhythm",
            },
            GlimpseMapSlotV1 {
                slot: 1,
                label: "word_stance",
                source_dims: &[8, 9, 10, 11, 12, 13, 14, 15],
                operation: "mean_abs_tanh",
                preserves: "lexical diversity, hedging, certainty",
            },
            GlimpseMapSlotV1 {
                slot: 2,
                label: "sentence_structure",
                source_dims: &[16, 17, 18, 19, 20, 21, 22, 23],
                operation: "mean_abs_tanh",
                preserves: "sentence rhythm, punctuation, paragraph structure",
            },
            GlimpseMapSlotV1 {
                slot: 3,
                label: "warmth_marker",
                source_dims: &[24],
                operation: "direct_tanh",
                preserves: "warmth stays separate from generic emotional mass",
            },
            GlimpseMapSlotV1 {
                slot: 4,
                label: "tension_marker",
                source_dims: &[25],
                operation: "direct_tanh",
                preserves: "concern/tension marker as its own coordinate",
            },
            GlimpseMapSlotV1 {
                slot: 5,
                label: "curiosity_marker",
                source_dims: &[26],
                operation: "direct_tanh",
                preserves: "curiosity and tail participation bridge",
            },
            GlimpseMapSlotV1 {
                slot: 6,
                label: "reflective_marker",
                source_dims: &[27],
                operation: "direct_tanh",
                preserves: "reflective/introspective marker",
            },
            GlimpseMapSlotV1 {
                slot: 7,
                label: "emotional_tail_mass",
                source_dims: &[28, 29, 30, 31],
                operation: "mean_abs_tanh",
                preserves: "remaining emotional/intentional range",
            },
            GlimpseMapSlotV1 {
                slot: 8,
                label: "projected_semantic_texture",
                source_dims: &[32, 33, 34, 35, 36, 37, 38, 39],
                operation: "mean_abs_tanh",
                preserves: "embedding-projected semantic detail",
            },
            GlimpseMapSlotV1 {
                slot: 9,
                label: "narrative_arc",
                source_dims: &[40, 41, 42, 43],
                operation: "mean_abs_tanh",
                preserves: "trajectory within the current text",
            },
            GlimpseMapSlotV1 {
                slot: 10,
                label: "tail_vibrancy_bridge",
                source_dims: &[17, 26, 27, 31],
                operation: "mean_abs_tanh",
                preserves: "lambda-tail-facing vibrancy bridge",
            },
            GlimpseMapSlotV1 {
                slot: 11,
                label: "whole_vector_energy",
                source_dims: &[],
                operation: "mean_abs_all_48_tanh",
                preserves: "global energy only; never the sole continuity proof",
            },
        ],
        deterministic_projection: true,
        companion_not_replacement: true,
        live_transport_change: false,
        live_vector_write: false,
        authority: "read_only_glimpse_lineage_not_live_transport_or_codec_contract_change",
    }
}

#[must_use]
pub fn glimpse_distinguishability_audit_v1(
    high_entropy_features: &[f32],
    low_entropy_features: &[f32],
) -> Option<GlimpseDistinguishabilityAuditV1> {
    if high_entropy_features.len() < SEMANTIC_DIM || low_entropy_features.len() < SEMANTIC_DIM {
        return None;
    }
    let high_glimpse = GlimpseCodec::derive_12d(high_entropy_features)?;
    let low_glimpse = GlimpseCodec::derive_12d(low_entropy_features)?;
    let source_distance = rms_delta(
        &high_entropy_features[..SEMANTIC_DIM],
        &low_entropy_features[..SEMANTIC_DIM],
    );
    let glimpse_distance = rms_delta(&high_glimpse, &low_glimpse);
    let preservation_ratio = if source_distance <= 1.0e-6 {
        0.0
    } else {
        (glimpse_distance / source_distance).clamp(0.0, 1.0)
    };
    let tail_bridge_delta = finite_abs(high_glimpse[10] - low_glimpse[10]);
    let source_threshold = 0.18;
    let glimpse_threshold = 0.05;
    let state = if source_distance < source_threshold {
        "source_states_too_close_for_distinguishability_claim"
    } else if glimpse_distance >= glimpse_threshold && tail_bridge_delta >= 0.03 {
        "glimpse_preserves_high_low_entropy_distinction"
    } else if glimpse_distance >= glimpse_threshold {
        "glimpse_preserves_global_but_not_tail_distinction"
    } else {
        "glimpse_collapse_watch"
    };

    Some(GlimpseDistinguishabilityAuditV1 {
        policy: "glimpse_distinguishability_audit_v1",
        source_distance,
        glimpse_distance,
        preservation_ratio,
        tail_bridge_delta,
        source_threshold,
        glimpse_threshold,
        state,
        live_transport_change: false,
        live_vector_write: false,
        authority: "read_only_12d_distinguishability_audit_not_live_transport_or_shadow_change",
    })
}

#[must_use]
pub fn multi_scale_context_v1() -> MultiScaleContextV1 {
    MultiScaleContextV1 {
        policy: "multi_scale_context_v1",
        source_dim_count: SEMANTIC_DIM,
        live_transport_dim_count: 32,
        glimpse_dim_count: 12,
        residual_dim_count: 32,
        residual_source_range: (16, 47),
        shadow_energy_metadata_tag: "shadow_field_energy_preserved_when_12d_glimpse_is_active",
        pairing_rule: "12d_glimpse_must_travel_with_32d_residual_context_for_persistence_review",
        preserves_warmth_and_tail_bridge: true,
        live_vector_write: false,
        authority: "dimensionality_aware_persistence_readout_not_live_bus_or_codec_contract_change",
    }
}
