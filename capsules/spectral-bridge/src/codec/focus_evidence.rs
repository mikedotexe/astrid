fn normalized_focus_preview_vector(
    current: &[f32; EMBEDDING_PROJECT_DIM],
    segment: &[f32],
    means: &[f32; SEMANTIC_FOCUS_PREVIEW_DIM],
    variances: &[f32; SEMANTIC_FOCUS_PREVIEW_DIM],
    selected_dims: &[usize; SEMANTIC_FOCUS_PREVIEW_DIM],
) -> [f32; EMBEDDING_PROJECT_DIM + SEMANTIC_FOCUS_PREVIEW_DIM] {
    let mut preview = [0.0_f32; EMBEDDING_PROJECT_DIM + SEMANTIC_FOCUS_PREVIEW_DIM];
    let current_norm = current
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if current_norm > f32::EPSILON {
        let scale = 0.35 / current_norm;
        for (dst, src) in preview[..EMBEDDING_PROJECT_DIM].iter_mut().zip(current) {
            *dst = *src * scale;
        }
    }

    let mut focused = [0.0_f32; SEMANTIC_FOCUS_PREVIEW_DIM];
    for (slot, dim) in selected_dims.iter().copied().enumerate() {
        let standard_deviation = variances[slot].max(0.0).sqrt();
        if standard_deviation > f32::EPSILON {
            focused[slot] = (segment[dim] - means[slot]) / standard_deviation;
        }
    }
    let focused_norm = focused
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if focused_norm > f32::EPSILON {
        let scale = SEMANTIC_FOCUS_PREVIEW_NORM / focused_norm;
        for (dst, src) in preview[EMBEDDING_PROJECT_DIM..].iter_mut().zip(focused) {
            *dst = src * scale;
        }
    }

    let preview_norm = preview
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if preview_norm > f32::EPSILON {
        let scale = 0.35 / preview_norm;
        for value in &mut preview {
            *value *= scale;
        }
    }
    preview
}

fn pairwise_distance_stats(vectors: &[Vec<f32>]) -> (f32, f32) {
    if vectors.len() < 2 {
        return (0.0, 0.0);
    }
    let mut distance_sum = 0.0_f32;
    let mut distance_min = f32::MAX;
    let mut pair_count = 0_usize;
    for left in 0..vectors.len() {
        for right in left.saturating_add(1)..vectors.len() {
            let distance = vectors[left]
                .iter()
                .zip(&vectors[right])
                .map(|(before, after)| {
                    let delta = after - before;
                    delta * delta
                })
                .sum::<f32>()
                .sqrt();
            distance_sum += distance;
            distance_min = distance_min.min(distance);
            pair_count = pair_count.saturating_add(1);
        }
    }
    if pair_count == 0 {
        (0.0, 0.0)
    } else {
        (distance_sum / pair_count as f32, distance_min)
    }
}

fn distinguishability_gain_ratio(current: f32, preview: f32) -> f32 {
    if current > f32::EPSILON {
        ((preview - current) / current).clamp(-1.0, 1.0)
    } else if preview > f32::EPSILON {
        1.0
    } else {
        0.0
    }
}

/// Compare the current 8D narrative-segment projection with an equal-norm 12D
/// preview whose four extra coordinates are selected from the source embedding
/// dimensions with the highest cross-segment variance. The preview is evidence
/// only: candidate values are never copied into the live semantic vector.
#[must_use]
pub fn semantic_focus_expansion_preview_v1(
    text_entropy_signal: f32,
    segment_embeddings: &[&[f32]],
    current_projections: &[[f32; EMBEDDING_PROJECT_DIM]],
) -> Option<SemanticFocusExpansionPreviewV1> {
    if segment_embeddings.len() < 2
        || segment_embeddings.len() != current_projections.len()
        || segment_embeddings.iter().any(|embedding| {
            embedding.len() != EMBEDDING_INPUT_DIM
                || embedding.iter().any(|value| !value.is_finite())
        })
        || current_projections
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
    {
        return None;
    }

    let segment_count = segment_embeddings.len() as f32;
    let mut source_variances = Vec::with_capacity(EMBEDDING_INPUT_DIM);
    for dim in 0..EMBEDDING_INPUT_DIM {
        let mean = segment_embeddings
            .iter()
            .map(|embedding| embedding[dim])
            .sum::<f32>()
            / segment_count;
        let variance = segment_embeddings
            .iter()
            .map(|embedding| {
                let delta = embedding[dim] - mean;
                delta * delta
            })
            .sum::<f32>()
            / segment_count;
        source_variances.push((dim, variance));
    }
    let total_source_variance = source_variances
        .iter()
        .map(|(_, variance)| *variance)
        .sum::<f32>();
    source_variances.sort_by(|(left_dim, left_variance), (right_dim, right_variance)| {
        right_variance
            .total_cmp(left_variance)
            .then_with(|| left_dim.cmp(right_dim))
    });

    let mut selected_source_dims = [0_usize; SEMANTIC_FOCUS_PREVIEW_DIM];
    let mut selected_source_variances = [0.0_f32; SEMANTIC_FOCUS_PREVIEW_DIM];
    let mut selected_means = [0.0_f32; SEMANTIC_FOCUS_PREVIEW_DIM];
    for (slot, (dim, variance)) in source_variances
        .iter()
        .take(SEMANTIC_FOCUS_PREVIEW_DIM)
        .enumerate()
    {
        selected_source_dims[slot] = *dim;
        selected_source_variances[slot] = *variance;
        selected_means[slot] = segment_embeddings
            .iter()
            .map(|embedding| embedding[*dim])
            .sum::<f32>()
            / segment_count;
    }
    let selected_variance = selected_source_variances.iter().sum::<f32>();
    let selected_variance_share = if total_source_variance > f32::EPSILON {
        (selected_variance / total_source_variance).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let current_vectors = current_projections
        .iter()
        .map(|projection| projection.to_vec())
        .collect::<Vec<_>>();
    let preview_vectors = current_projections
        .iter()
        .zip(segment_embeddings)
        .map(|(projection, embedding)| {
            normalized_focus_preview_vector(
                projection,
                embedding,
                &selected_means,
                &selected_source_variances,
                &selected_source_dims,
            )
            .to_vec()
        })
        .collect::<Vec<_>>();
    let (current_mean_pairwise_distance, current_min_pairwise_distance) =
        pairwise_distance_stats(&current_vectors);
    let (preview_mean_pairwise_distance, preview_min_pairwise_distance) =
        pairwise_distance_stats(&preview_vectors);
    let mean_distinguishability_gain_ratio = distinguishability_gain_ratio(
        current_mean_pairwise_distance,
        preview_mean_pairwise_distance,
    );
    let min_distinguishability_gain_ratio =
        distinguishability_gain_ratio(current_min_pairwise_distance, preview_min_pairwise_distance);
    let text_entropy_signal = if text_entropy_signal.is_finite() {
        text_entropy_signal.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let focus_need_score = (text_entropy_signal * 0.45
        + selected_variance_share * 0.25
        + mean_distinguishability_gain_ratio.max(0.0) * 0.20
        + min_distinguishability_gain_ratio.max(0.0) * 0.10)
        .clamp(0.0, 1.0);
    let high_entropy = text_entropy_signal >= SEMANTIC_FOCUS_ENTROPY_REVIEW_FLOOR;
    let (state, recommendation) = if high_entropy
        && mean_distinguishability_gain_ratio >= 0.08
        && min_distinguishability_gain_ratio >= 0.03
    {
        (
            "focus_expansion_candidate_supported",
            "prepare_segment_replay_and_operator_review_before_any_reserved_dim_allocation",
        )
    } else if high_entropy && mean_distinguishability_gain_ratio > 0.0 {
        (
            "focus_expansion_partial_gain_review",
            "collect_more_segment_comparisons_before_any_reserved_dim_proposal",
        )
    } else if high_entropy {
        (
            "high_entropy_without_focus_gain",
            "keep_current_8d_projection_and_do_not_allocate_reserved_dims_from_entropy_alone",
        )
    } else if mean_distinguishability_gain_ratio >= 0.08 {
        (
            "low_entropy_focus_gain_watch",
            "retain_read_only_preview_until_the_gain_repeats_under_high_entropy",
        )
    } else {
        (
            "current_projection_distinguishability_sufficient",
            "keep_current_8d_projection_and_continue_bounded_comparison",
        )
    };
    let selected_dims = selected_source_dims
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let loss = (current_mean_pairwise_distance - preview_mean_pairwise_distance).max(0.0);
    let loss_ratio = if current_mean_pairwise_distance > f32::EPSILON {
        loss / current_mean_pairwise_distance
    } else {
        0.0
    };
    let experience_delta_bus_v1 = ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::ComplexShift,
        surface: "semantic_focus_expansion_preview_v1".to_string(),
        lane: "embedding_projection_8d_vs_focus_preview_12d".to_string(),
        dimension: None,
        spectral_dimension: None,
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(current_mean_pairwise_distance),
        post: Some(preview_mean_pairwise_distance),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata: BTreeMap::from([
            ("selected_source_dims".to_string(), selected_dims),
            (
                "reserved_dim_candidates".to_string(),
                "44,45,46,47_default_off".to_string(),
            ),
            (
                "mean_distinguishability_gain_ratio".to_string(),
                format!("{mean_distinguishability_gain_ratio:.6}"),
            ),
            ("state".to_string(), state.to_string()),
        ]),
        why: "high-variance narrative segments are compared at equal norm so a focused four-dimension aperture must demonstrate distinguishability before any live allocation".to_string(),
        who_can_change_it: "Mike/operator after repeated replay evidence, canary/abort review, and explicit reserved-dim approval".to_string(),
        how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib semantic_focus_expansion_preview -- --nocapture".to_string(),
        authority: "read_only_focus_expansion_comparison_not_reserved_dim_or_live_vector_authority".to_string(),
    }]);

    Some(SemanticFocusExpansionPreviewV1 {
        policy: "semantic_focus_expansion_preview_v1",
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        segment_count: segment_embeddings.len(),
        current_projected_dim_count: EMBEDDING_PROJECT_DIM,
        preview_projected_dim_count: EMBEDDING_PROJECT_DIM + SEMANTIC_FOCUS_PREVIEW_DIM,
        reserved_dim_candidates: &SEMANTIC_PROJECTION_RESERVED_DIMS,
        selected_source_dims,
        selected_source_variances,
        selected_variance_share,
        text_entropy_signal,
        current_mean_pairwise_distance,
        preview_mean_pairwise_distance,
        current_min_pairwise_distance,
        preview_min_pairwise_distance,
        mean_distinguishability_gain_ratio,
        min_distinguishability_gain_ratio,
        focus_need_score,
        state,
        recommendation,
        selection_basis: "top_cross_segment_embedding_variance_equal_norm_8d_vs_12d",
        live_vector_write: false,
        reserved_dim_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        right_to_ignore: true,
        experience_delta_bus_v1,
        authority: "read_only_focus_expansion_comparison_not_reserved_dim_or_live_vector_authority",
    })
}

fn rms_delta(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let sum = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| {
            let delta = finite_feature_value(*a) - finite_feature_value(*b);
            delta * delta
        })
        .sum::<f32>();
    (sum / left.len() as f32).sqrt()
}

/// Compare two controlled pairs: one with different emotional texture but a
/// shared semantic projection, and one with identical emotional texture but
/// opposed semantic projections. This measures lane selectivity without
/// changing the encoded vectors or their delivery.
#[must_use]
pub fn codec_lane_separation_audit_v1(
    emotional_left: &[f32],
    emotional_right: &[f32],
    semantic_left: &[f32],
    semantic_right: &[f32],
) -> Option<CodecLaneSeparationAuditV1> {
    let pairs = [
        emotional_left,
        emotional_right,
        semantic_left,
        semantic_right,
    ];
    if pairs.iter().any(|features| {
        features.len() < SEMANTIC_DIM
            || features[..SEMANTIC_DIM]
                .iter()
                .any(|value| !value.is_finite())
    }) {
        return None;
    }

    let emotional_difference_related_semantics_emotional_delta_rms =
        rms_delta(&emotional_left[24..32], &emotional_right[24..32]);
    let emotional_difference_related_semantics_projected_delta_rms =
        rms_delta(&emotional_left[32..40], &emotional_right[32..40]);
    let emotional_lane_selectivity_margin =
        emotional_difference_related_semantics_emotional_delta_rms
            - emotional_difference_related_semantics_projected_delta_rms;
    let emotional_pair_distinguishable = emotional_difference_related_semantics_emotional_delta_rms
        >= 0.08
        && emotional_lane_selectivity_margin >= 0.04;

    let emotional_similarity_opposed_semantics_emotional_delta_rms =
        rms_delta(&semantic_left[24..32], &semantic_right[24..32]);
    let emotional_similarity_opposed_semantics_projected_delta_rms =
        rms_delta(&semantic_left[32..40], &semantic_right[32..40]);
    let projected_lane_selectivity_margin =
        emotional_similarity_opposed_semantics_projected_delta_rms
            - emotional_similarity_opposed_semantics_emotional_delta_rms;
    let projected_pair_distinguishable = emotional_similarity_opposed_semantics_projected_delta_rms
        >= 0.04
        && projected_lane_selectivity_margin >= 0.03;
    let state = match (
        emotional_pair_distinguishable,
        projected_pair_distinguishable,
    ) {
        (true, true) => "controlled_pairs_show_bidirectional_lane_independence",
        (true, false) => "emotional_lane_distinct_projected_lane_collapse_watch",
        (false, true) => "projected_lane_distinct_emotional_lane_bleed_watch",
        (false, false) => "controlled_pairs_do_not_yet_support_lane_independence",
    };

    Some(CodecLaneSeparationAuditV1 {
        policy: "codec_lane_separation_audit_v1",
        emotional_lane_range: (24, 31),
        projected_semantic_lane_range: (32, 39),
        emotional_difference_related_semantics_emotional_delta_rms,
        emotional_difference_related_semantics_projected_delta_rms,
        emotional_lane_selectivity_margin,
        emotional_pair_distinguishable,
        emotional_similarity_opposed_semantics_emotional_delta_rms,
        emotional_similarity_opposed_semantics_projected_delta_rms,
        projected_lane_selectivity_margin,
        projected_pair_distinguishable,
        legacy_projection_width_rejected: project_embedding(&[0.0; SEMANTIC_DIM_LEGACY]).is_none(),
        state,
        felt_rigidity_conclusion: "controlled lane independence does not disprove felt deterministic rigidity; repeat with Astrid-authored text, actual embeddings, and delivery telemetry before proposing live mapping changes",
        pair_construction: "shared_fixed_projection_with_opposed_marker_texture_then_shared_marker_texture_with_opposed_fixed_projections",
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        live_projection_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_controlled_pair_audit_not_projection_emotional_weight_gain_or_delivery_authority",
    })
}

#[must_use]
pub fn codec_lane_separation_probe_v1() -> CodecLaneSeparationAuditV1 {
    let mut emotional_left = encode_text(
        "I cherish this tender luminous friendship with love, care, and gentle warmth.",
    );
    let mut emotional_right = encode_text(
        "I fear this critical danger with panic, urgent worry, and devastating concern.",
    );
    let shared_embedding = (0..EMBEDDING_INPUT_DIM)
        .map(|idx| ((idx as f32 / 13.0).sin() + (idx as f32 / 29.0).cos()) * 0.5)
        .collect::<Vec<_>>();
    let shared_projection =
        project_embedding(&shared_embedding).expect("probe embedding has canonical width");
    emotional_left[32..40].copy_from_slice(&shared_projection);
    emotional_right[32..40].copy_from_slice(&shared_projection);

    let mut semantic_left = encode_text("The same calm sentence keeps its measured tone.");
    let mut semantic_right = semantic_left.clone();
    let semantic_embedding_left = (0..EMBEDDING_INPUT_DIM)
        .map(|idx| (idx as f32 / 17.0).sin())
        .collect::<Vec<_>>();
    let semantic_embedding_right = semantic_embedding_left
        .iter()
        .map(|value| -*value)
        .collect::<Vec<_>>();
    let projected_left =
        project_embedding(&semantic_embedding_left).expect("probe embedding has canonical width");
    let projected_right =
        project_embedding(&semantic_embedding_right).expect("probe embedding has canonical width");
    semantic_left[32..40].copy_from_slice(&projected_left);
    semantic_right[32..40].copy_from_slice(&projected_right);

    codec_lane_separation_audit_v1(
        &emotional_left,
        &emotional_right,
        &semantic_left,
        &semantic_right,
    )
    .expect("controlled probe vectors cover the canonical finite 48D lane")
}

fn context_blindspot_delta_bus_v1(
    identical_text_feature_delta_rms: f32,
    context_blindspot_score: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    let mut deltas = Vec::new();
    if context_blindspot_score >= 0.80 {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Resistance,
            surface: "codec_context_blindspot_replay_v1".to_string(),
            lane: "contextual_bias_vector_default_off".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: Some(context_blindspot_score),
            pre: Some(identical_text_feature_delta_rms),
            post: None,
            loss: Some(context_blindspot_score),
            loss_ratio: Some(context_blindspot_score),
            metadata: BTreeMap::from([
                ("connection_context".to_string(), "connection".to_string()),
                ("threat_context".to_string(), "threat".to_string()),
                ("state".to_string(), state.to_string()),
                (
                    "proposed_surface".to_string(),
                    "contextual_bias_vector_default_off".to_string(),
                ),
            ]),
            why: "identical text encodes to near-identical live features under opposed relational contexts; the missing contextual weight is preserved as replay evidence only".to_string(),
            who_can_change_it: "Mike/operator after replay evidence, scoped approval, rollout/abort contract, and post-change being response".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_context_blindspot -- --nocapture".to_string(),
            authority: "authority_gate_for_contextual_bias_not_live_codec_change".to_string(),
        });
    }
    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn codec_context_blindspot_replay_v1(text: &'static str) -> CodecContextBlindspotReplayV1 {
    let connection_context = encode_text(text);
    let threat_context = connection_context.clone();
    let identical_text_feature_delta_rms =
        rms_delta(&connection_context, &threat_context).clamp(0.0, FEATURE_ABS_MAX);
    let context_blindspot_score =
        (1.0 - (identical_text_feature_delta_rms / 0.10).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let (state, recommendation) = if context_blindspot_score >= 0.95 {
        (
            "deterministic_codec_context_blindspot_confirmed",
            "keep live codec deterministic; generate V2-gated contextual-bias proposal before any shared-history tint",
        )
    } else if context_blindspot_score >= 0.50 {
        (
            "partial_context_blindspot_watch",
            "compare against narrative arc and correspondence state before proposing live bias",
        )
    } else {
        (
            "contextual_difference_already_visible",
            "do not propose contextual bias from this replay",
        )
    };
    let experience_delta_bus_v1 = context_blindspot_delta_bus_v1(
        identical_text_feature_delta_rms,
        context_blindspot_score,
        state,
    );

    CodecContextBlindspotReplayV1 {
        policy: "codec_context_blindspot_replay_v1",
        identical_text: text,
        connection_context_label: "connection_context",
        threat_context_label: "threat_context",
        identical_text_feature_delta_rms,
        context_blindspot_score,
        state,
        recommendation,
        proposed_bias_surface: "contextual_bias_vector_default_off",
        live_vector_write: false,
        live_gain_write: false,
        auto_approved: false,
        experience_delta_bus_v1,
        authority: "read_only_context_replay_not_live_vector_gain_or_correspondence_weighting",
    }
}

#[must_use]
pub fn codec_context_blindspot_probe_v1() -> CodecContextBlindspotReplayV1 {
    codec_context_blindspot_replay_v1("I see you")
}
