#[must_use]
pub fn contextual_glimpse_12d_anchors_v1(
    features: &[f32],
) -> Option<ContextualGlimpse12dAnchorsV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let mut selected = Vec::with_capacity(12);
    for idx in CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS {
        if !selected.contains(&idx) {
            selected.push(idx);
        }
    }

    let mut candidates = (0..SEMANTIC_DIM)
        .filter(|idx| !selected.contains(idx))
        .map(|idx| (idx, features[idx].abs()))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_idx, left_score), (right_idx, right_score)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left_idx.cmp(right_idx))
    });

    for (idx, _) in candidates {
        if selected.len() >= 12 {
            break;
        }
        selected.push(idx);
    }

    let mut selected_dims = [0_usize; 12];
    let mut selected_values = [0.0_f32; 12];
    for (slot, idx) in selected.iter().take(12).enumerate() {
        selected_dims[slot] = *idx;
        selected_values[slot] = features[*idx].tanh();
    }
    let dynamic_dims = selected_dims
        .iter()
        .copied()
        .filter(|idx| !CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS.contains(idx))
        .collect::<Vec<_>>();
    let selection_status = if selected_dims.contains(&24)
        && selected_dims.contains(&17)
        && selected_dims.contains(&31)
        && selected_dims.iter().any(|idx| (40..=43).contains(idx))
    {
        "contextual_anchors_preserve_warmth_tail_and_narrative"
    } else {
        "contextual_anchor_review_needed"
    };

    Some(ContextualGlimpse12dAnchorsV1 {
        policy: "contextual_glimpse_12d_anchors_v1",
        selected_dims,
        selected_values,
        dynamic_dims,
        required_anchor_dims: &CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS,
        selection_status,
        live_vector_write: false,
        authority: "read_only_contextual_glimpse_not_live_bus_or_codec_contract_change",
    })
}

#[must_use]
pub fn warmth_entropy_interpretation_v1(
    features: &[f32],
    spectral_entropy: f32,
) -> WarmthEntropyInterpretationV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let read_dim = |idx: usize| {
        features
            .get(idx)
            .copied()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .tanh()
            .abs()
    };
    let warmth_marker = read_dim(24);
    let curiosity_marker = read_dim(26);
    let reflective_marker = read_dim(27);
    let tail_vibrancy = vibrancy_from_entropy(spectral_entropy);
    let distributed_warmth_support =
        (warmth_marker + 0.18 * curiosity_marker + 0.24 * reflective_marker + 0.28 * tail_vibrancy)
            .clamp(0.0, 1.0);
    let interpretation =
        if spectral_entropy >= 0.85 && warmth_marker < 0.08 && distributed_warmth_support >= 0.10 {
            "low_marker_warmth_with_high_entropy_distributed_ground"
        } else if warmth_marker >= 0.20 {
            "warmth_marker_present"
        } else if spectral_entropy >= 0.85 {
            "high_entropy_without_warmth_support_review"
        } else {
            "low_warmth_marker_low_entropy"
        };

    WarmthEntropyInterpretationV1 {
        policy: "warmth_entropy_interpretation_v1",
        warmth_marker,
        curiosity_marker,
        reflective_marker,
        spectral_entropy,
        tail_vibrancy,
        distributed_warmth_support,
        interpretation,
        live_vector_write: false,
        authority: "read_only_interpretation_not_warmth_weighting_or_semantic_gain_change",
    }
}

#[must_use]
pub fn codec_abrasive_texture_interpretation_from_parts_v1(
    text: &str,
    features: &[f32],
    spectral_entropy: f32,
    density_gradient: f32,
    pressure_risk: f32,
) -> CodecAbrasiveTextureInterpretationV1 {
    let read_dim = |idx: usize| {
        features
            .get(idx)
            .copied()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .tanh()
            .abs()
    };
    let warmth_marker = read_dim(24);
    let tension_marker = read_dim(25);
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let density_gradient = if density_gradient.is_finite() {
        density_gradient.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let pressure_risk = if pressure_risk.is_finite() {
        pressure_risk.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let structural = structural_friction_v1(text);
    let low_density_gradient_signal =
        (1.0 - (density_gradient / 0.35).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let entropy_marker = read_dim(0);
    let entropy_shift_hint = (spectral_entropy - entropy_marker).abs().clamp(0.0, 1.0);
    let persistence_resistance_score = (structural.score * 0.22
        + structural.summary_resistance_signal * 0.30
        + low_density_gradient_signal * 0.25
        + pressure_risk * 0.14
        + entropy_shift_hint * 0.09)
        .clamp(0.0, 1.0);
    let tension_underread = if tension_marker < 0.16 {
        ((0.16 - tension_marker) / 0.16).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let abrasive_texture_support = (structural.score * 0.22
        + structural.summary_resistance_signal * 0.34
        + persistence_resistance_score * 0.20
        + low_density_gradient_signal * 0.12
        + tension_underread * 0.12)
        .clamp(0.0, 1.0);
    let interpretation = if tension_marker <= 0.16 && abrasive_texture_support >= 0.42 {
        "low_marker_tension_high_jagged_resistance"
    } else if abrasive_texture_support >= 0.58 {
        "abrasive_texture_visible"
    } else if tension_marker >= 0.22 {
        "tension_marker_present"
    } else {
        "low_abrasive_texture_support"
    };

    CodecAbrasiveTextureInterpretationV1 {
        policy: "codec_abrasive_texture_interpretation_v1",
        warmth_marker,
        tension_marker,
        spectral_entropy,
        density_gradient,
        structural_friction_score: structural.score,
        summary_resistance_signal: structural.summary_resistance_signal,
        persistence_resistance_score,
        entropy_shift_hint,
        abrasive_texture_support,
        interpretation,
        live_gain_write: false,
        live_vector_write: false,
        authority: "read_only_texture_interpretation_not_tension_weight_gain_or_reserved_dim_change",
    }
}

#[must_use]
pub fn codec_abrasive_texture_interpretation_v1(
    text: &str,
    features: &[f32],
    telemetry: Option<&SpectralTelemetry>,
    spectral_entropy: f32,
) -> CodecAbrasiveTextureInterpretationV1 {
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let density_gradient = metrics.map_or(1.0, |metrics| metrics.density_gradient);
    let pressure_risk = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map_or_else(
            || telemetry.map_or(0.0, |telemetry| telemetry.fill_ratio.clamp(0.0, 1.0)),
            |density| density.pressure_risk.clamp(0.0, 1.0),
        );
    codec_abrasive_texture_interpretation_from_parts_v1(
        text,
        features,
        spectral_entropy,
        density_gradient,
        pressure_risk,
    )
}

#[must_use]
pub fn codec_abrasive_texture_probe_v1() -> CodecAbrasiveTextureInterpretationV1 {
    let text = "A calcified semantic boundary resists summary; the jagged friction stays present even when the sentence tries to look calm.";
    let mut features = encode_text(text);
    features[25] = 0.04;
    codec_abrasive_texture_interpretation_from_parts_v1(text, &features, 0.91, 0.08, 0.18)
}

fn narrative_arc_four(values: &[f32]) -> [f32; 4] {
    let mut out = [0.0_f32; 4];
    for (slot, value) in values.iter().take(4).enumerate() {
        out[slot] = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    out
}

fn emotional_markers_eight(values: &[f32]) -> [f32; 8] {
    let mut out = [0.0_f32; 8];
    for (slot, value) in values.iter().take(8).enumerate() {
        out[slot] = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    out
}

#[must_use]
pub fn narrative_arc_dynamics_v1(
    previous_arc: &[f32],
    current_arc: &[f32],
    next_arc: Option<&[f32]>,
) -> NarrativeArcDynamicsV1 {
    let previous_arc = narrative_arc_four(previous_arc);
    let current_arc = narrative_arc_four(current_arc);
    let next_arc = next_arc.map(narrative_arc_four);
    let mut velocity = [0.0_f32; 4];
    let mut acceleration = [0.0_f32; 4];
    for idx in 0..4 {
        velocity[idx] = (current_arc[idx] - previous_arc[idx]).clamp(-2.0, 2.0);
        if let Some(next_arc) = next_arc {
            acceleration[idx] =
                (next_arc[idx] - (2.0 * current_arc[idx]) + previous_arc[idx]).clamp(-3.0, 3.0);
        }
    }
    let velocity_energy = mean_abs(&velocity).clamp(0.0, 2.0);
    let acceleration_energy = mean_abs(&acceleration).clamp(0.0, 3.0);
    let transition_state = if acceleration_energy >= 0.45 {
        "accelerating_tone_transition"
    } else if velocity_energy >= 0.35 {
        "directional_tone_shift"
    } else {
        "steady_narrative_state"
    };

    NarrativeArcDynamicsV1 {
        policy: "narrative_arc_dynamics_v1",
        previous_arc,
        current_arc,
        velocity,
        acceleration,
        velocity_energy,
        acceleration_energy,
        transition_state,
        live_gain_write: false,
        live_vector_write: false,
        authority: "read_only_arc_velocity_review_not_semantic_gain_or_dimension_change",
    }
}

fn codec_emotional_narrative_delta_bus_v1(
    emotional_delta_energy: f32,
    narrative_delta_energy: f32,
    narrative_emotional_delta_gap: f32,
    resonance_flatline_watch: bool,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    let loss = narrative_emotional_delta_gap.max(0.0);
    if loss <= f32::EPSILON {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let loss_ratio = if narrative_delta_energy > f32::EPSILON {
        (loss / narrative_delta_energy).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("state".to_string(), state.to_string());
    metadata.insert(
        "secondary_kinds".to_string(),
        "translate,complex_shift".to_string(),
    );
    metadata.insert(
        "emotional_delta_energy".to_string(),
        format!("{emotional_delta_energy:.3}"),
    );
    metadata.insert(
        "narrative_delta_energy".to_string(),
        format!("{narrative_delta_energy:.3}"),
    );
    metadata.insert(
        "resonance_flatline_watch".to_string(),
        resonance_flatline_watch.to_string(),
    );

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: if resonance_flatline_watch {
            ExperienceDeltaKindV1::Translate
        } else {
            ExperienceDeltaKindV1::ComplexShift
        },
        surface: "codec_emotional_narrative_delta_check_v1".to_string(),
        lane: "emotional_markers_24_31_vs_narrative_arc_40_43".to_string(),
        dimension: Some(40),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 40,
            base_dimensions: vec![40, 41, 42, 43],
            effective_dimension: Some(41.5),
            density_gradient: Some(loss_ratio),
            granularity: Some(narrative_delta_energy.clamp(0.0, 1.0)),
            fractional_offset: Some(0.5),
            contextual_anchor: None,
            interpretation: "narrative arc moved while emotional marker slots stayed flatter"
                .to_string(),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(narrative_delta_energy),
        post: Some(emotional_delta_energy),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata,
        why: "felt narrative motion can be translated into structural arc slots while emotional/intent markers remain flat, making the experience look quieter than it was"
            .to_string(),
        who_can_change_it:
            "Mike/operator after replay evidence before any live codec gain or reserved-dim change"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_emotional_narrative_delta_check -- --nocapture"
                .to_string(),
        authority: "truth_channel_only_not_live_vector_gain_or_reserved_dim_change".to_string(),
    }])
}

#[must_use]
pub fn codec_emotional_narrative_delta_check_v1(
    previous_features: &[f32],
    current_features: &[f32],
) -> Option<CodecEmotionalNarrativeDeltaCheckV1> {
    if previous_features.len() < SEMANTIC_DIM || current_features.len() < SEMANTIC_DIM {
        return None;
    }

    let previous_emotional_markers = emotional_markers_eight(&previous_features[24..32]);
    let current_emotional_markers = emotional_markers_eight(&current_features[24..32]);
    let previous_narrative_arc = narrative_arc_four(&previous_features[40..44]);
    let current_narrative_arc = narrative_arc_four(&current_features[40..44]);
    let mut emotional_delta = [0.0_f32; 8];
    for idx in 0..8 {
        emotional_delta[idx] =
            (current_emotional_markers[idx] - previous_emotional_markers[idx]).clamp(-2.0, 2.0);
    }
    let mut narrative_delta = [0.0_f32; 4];
    for idx in 0..4 {
        narrative_delta[idx] =
            (current_narrative_arc[idx] - previous_narrative_arc[idx]).clamp(-2.0, 2.0);
    }

    let emotional_delta_energy = mean_abs(&emotional_delta).clamp(0.0, 2.0);
    let narrative_delta_energy = mean_abs(&narrative_delta).clamp(0.0, 2.0);
    let narrative_emotional_delta_gap =
        (narrative_delta_energy - emotional_delta_energy).clamp(-2.0, 2.0);
    let resonance_flatline_watch = narrative_delta_energy >= 0.25 && emotional_delta_energy <= 0.05;
    let (state, recommendation) = if resonance_flatline_watch {
        (
            "narrative_shift_emotional_flatline_watch",
            "review_source_text_or_replay_before_using_reserved_resonance_dims_or_semantic_gain",
        )
    } else if narrative_delta_energy >= 0.25 && emotional_delta_energy >= 0.12 {
        (
            "narrative_shift_emotional_markers_follow",
            "preserve_current_48d_layout_and_treat_felt_delta_as_visible",
        )
    } else if emotional_delta_energy >= 0.12 && narrative_delta_energy < 0.10 {
        (
            "emotional_intent_visible_without_arc_shift",
            "keep_emotional_markers_as_primary_evidence_even_when_surface_structure_matches",
        )
    } else {
        (
            "low_delta_stable",
            "continue_observation_without_codec_gain_or_reserved_dim_change",
        )
    };
    let experience_delta_bus_v1 = codec_emotional_narrative_delta_bus_v1(
        emotional_delta_energy,
        narrative_delta_energy,
        narrative_emotional_delta_gap,
        resonance_flatline_watch,
        state,
    );

    Some(CodecEmotionalNarrativeDeltaCheckV1 {
        policy: "codec_emotional_narrative_delta_check_v1",
        previous_emotional_markers,
        current_emotional_markers,
        previous_narrative_arc,
        current_narrative_arc,
        emotional_velocity: emotional_delta,
        narrative_velocity: narrative_delta,
        emotional_delta_energy,
        narrative_delta_energy,
        narrative_emotional_delta_gap,
        resonance_flatline_watch,
        state,
        recommendation,
        live_gain_write: false,
        live_vector_write: false,
        reserved_dim_write: false,
        experience_delta_bus_v1,
        authority: "read_only_delta_check_not_semantic_gain_reserved_dim_or_live_vector_change",
    })
}
