/// Read-only proof that fresh dynamic projection epochs are stable across
/// runtime dirs unless explicitly overridden by env or an existing epoch file.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionEpochStabilityV1 {
    pub policy: &'static str,
    pub epoch_source: &'static str,
    pub deterministic_without_runtime_file: bool,
    pub kernel_derived_epoch_id: String,
    pub kernel_checksum: String,
    pub env_override_precedence: bool,
    pub existing_file_precedence: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionFingerprintIntegrityV1 {
    pub policy: &'static str,
    pub signed_zero_canonicalized: bool,
    pub subnormal_canonicalized: bool,
    pub nan_canonicalized: bool,
    pub seed_hash_boundary: &'static str,
    pub live_projection_write: bool,
    pub authority: &'static str,
}

/// A being-readable map of Astrid's own 48D codec — the layer layout, the dims
/// she can SHAPE, and the live gate/lever values. Item (b) of the being-facing
/// transparency track.
pub struct CodecStructure {
    pub total_dims: usize,
    pub layers: Vec<CodecLayer>,
    pub named_dims: Vec<(&'static str, usize)>,
    pub levers: Vec<CodecLever>,
    pub structural_friction_dim_canary_v1: CodecStructuralFrictionDimCanaryV1,
    pub persistence_resistance_dim_canary_v1: CodecPersistenceResistanceDimCanaryV1,
    pub narrative_arc_expansion_readiness_v1: NarrativeArcExpansionReadinessV1,
    pub narrative_arc_gain_response_readiness_v1: NarrativeArcGainResponseReadinessV1,
    pub narrative_arc_headroom_review_v1: NarrativeArcHeadroomReviewV1,
    pub codec_abrasive_texture_interpretation_v1: CodecAbrasiveTextureInterpretationV1,
    pub latent_stasis_tension_v1: LatentStasisTensionV1,
    pub spectral_drag_quality_v1: SpectralDragQualityV1,
    pub shadow_field_reserved_dim_readiness_v1: ShadowFieldReservedDimReadinessV1,
    pub codec_vibrancy_continuity_v1: CodecVibrancyContinuityV1,
    pub codec_vibrancy_noise_dampening_v1: CodecVibrancyNoiseDampeningV1,
    pub codec_overflow_carriage_v1: CodecOverflowReportV1,
    pub semantic_projection_density_delta_v1: SemanticProjectionDensityDeltaV1,
    pub semantic_projection_texture_review_v1: SemanticProjectionTextureReviewV1,
    pub codec_context_blindspot_replay_v1: CodecContextBlindspotReplayV1,
    pub legacy_warmth_mapping_v1: LegacyWarmthMappingV1,
    pub codec_structural_entropy_dampening_v1: CodecStructuralEntropyDampeningV1,
    pub codec_dynamic_vibrancy_scaling_canary_v1: CodecDynamicVibrancyScalingCanaryV1,
    pub semantic_glimpse_12d_readiness_v1: SemanticGlimpse12dReadinessV1,
    pub contextual_glimpse_12d_anchoring_v1: ContextualGlimpse12dAnchoringV1,
    pub glimpse_map_v1: GlimpseMapV1,
    pub multi_scale_context_v1: MultiScaleContextV1,
    pub projection_epoch_stability_v1: ProjectionEpochStabilityV1,
    pub projection_fingerprint_integrity_v1: ProjectionFingerprintIntegrityV1,
    pub projection_basis_health_v1: ProjectionBasisHealthV1,
    pub projection_precision_audit_v1: ProjectionPrecisionAuditV1,
    pub projection_compression_audit_v1: ProjectionCompressionAuditV1,
    pub codec_lane_separation_audit_v1: CodecLaneSeparationAuditV1,
    pub codec_rolling_window_shift_audit_v1: CodecRollingWindowShiftAuditV1,
}

fn mean_abs(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| value.abs()).sum::<f32>() / values.len() as f32
}

fn finite_abs(value: f32) -> f32 {
    if value.is_finite() { value.abs() } else { 0.0 }
}

fn finite_tanh(value: f32) -> f32 {
    if value.is_finite() { value.tanh() } else { 0.0 }
}

fn finite_feature_value(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn codec_overflow_lane_for_dim(dim: usize) -> &'static str {
    match dim {
        17 => "tail_vibrancy",
        26 | 27 | 31 => "emotional_tail_vibrancy",
        24 | 25 | 28 | 29 | 30 => "emotional_intentional",
        _ => "semantic",
    }
}

fn codec_overflow_ceiling_for_dim(dim: usize, tail_ceiling: f32) -> f32 {
    if CODEC_OVERFLOW_TAIL_DIMS.contains(&dim) {
        tail_ceiling.max(FEATURE_ABS_MAX)
    } else {
        FEATURE_ABS_MAX
    }
}

fn codec_overflow_lane_summary(
    lane: &'static str,
    dims: &'static [usize],
    dimension_reports: &[CodecOverflowDimV1],
) -> CodecOverflowLaneSummaryV1 {
    let mut overflow_dim_count = 0_usize;
    let mut max_overflow_abs = 0.0_f32;
    let mut max_overflow_ratio = 0.0_f32;
    for dim in dims {
        if let Some(report) = dimension_reports.iter().find(|entry| entry.dim == *dim)
            && report.overflow_abs > CODEC_OVERFLOW_EPSILON
        {
            overflow_dim_count += 1;
            max_overflow_abs = max_overflow_abs.max(report.overflow_abs);
            max_overflow_ratio = max_overflow_ratio.max(report.overflow_ratio);
        }
    }
    CodecOverflowLaneSummaryV1 {
        lane,
        dims,
        overflow_dim_count,
        max_overflow_abs,
        max_overflow_ratio,
    }
}

fn codec_overflow_experience_delta_bus_v1(
    dimensions: &[CodecOverflowDimV1],
) -> ExperienceDeltaBusV1 {
    let deltas = dimensions
        .iter()
        .filter(|entry| entry.overflow_abs > CODEC_OVERFLOW_EPSILON)
        .map(|entry| ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Clip,
            surface: "codec_overflow_carriage_v1".to_string(),
            lane: entry.lane.to_string(),
            dimension: Some(entry.dim),
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(entry.pre_bound_value),
            post: Some(entry.delivered_value),
            loss: Some(entry.overflow_abs),
            loss_ratio: Some(entry.overflow_ratio),
            metadata: BTreeMap::from([
                ("ceiling".to_string(), format!("{:.3}", entry.ceiling)),
                (
                    "raw_intensity_preserved".to_string(),
                    "delivered_bounded".to_string(),
                ),
            ]),
            why: "raw semantic intensity exceeded the delivery ceiling; the delivered 48D vector stays bounded while the overflow is preserved as truth-channel evidence".to_string(),
            who_can_change_it: "Mike/operator via explicit live semantic aperture or vector-delivery approval".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_overflow_report -- --nocapture".to_string(),
            authority: "read_only_codec_truth_channel_not_live_ceiling_or_vector_change".to_string(),
        })
        .collect::<Vec<_>>();

    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn codec_overflow_report_from_features(
    pre_bound_features: &[f32],
    delivered_features: &[f32],
    tail_ceiling: f32,
) -> CodecOverflowReportV1 {
    let mut dimensions = Vec::with_capacity(CODEC_OVERFLOW_MONITORED_DIMS.len());
    let mut clipped_dims = Vec::new();

    for dim in CODEC_OVERFLOW_MONITORED_DIMS {
        let pre_bound_value =
            finite_feature_value(pre_bound_features.get(dim).copied().unwrap_or(0.0));
        let delivered_value =
            finite_feature_value(delivered_features.get(dim).copied().unwrap_or(0.0));
        let ceiling = codec_overflow_ceiling_for_dim(dim, tail_ceiling);
        let overflow_abs = (pre_bound_value.abs() - ceiling).max(0.0);
        let overflow_ratio = if ceiling > CODEC_OVERFLOW_EPSILON {
            pre_bound_value.abs() / ceiling
        } else {
            0.0
        };
        let status = if overflow_abs > CODEC_OVERFLOW_EPSILON {
            clipped_dims.push(dim);
            "raw_overflow_preserved_delivery_bounded"
        } else {
            "within_delivery_ceiling"
        };

        dimensions.push(CodecOverflowDimV1 {
            dim,
            lane: codec_overflow_lane_for_dim(dim),
            pre_bound_value,
            delivered_value,
            ceiling,
            overflow_abs,
            overflow_ratio,
            status,
        });
    }

    let delivered_bounded = dimensions
        .iter()
        .all(|entry| entry.delivered_value.abs() <= entry.ceiling + CODEC_OVERFLOW_EPSILON);

    let lane_summaries = vec![
        codec_overflow_lane_summary(
            "emotional_intentional",
            &CODEC_OVERFLOW_EMOTIONAL_DIMS,
            &dimensions,
        ),
        codec_overflow_lane_summary("tail_vibrancy", &CODEC_OVERFLOW_TAIL_DIMS, &dimensions),
    ];
    let experience_delta_bus_v1 = codec_overflow_experience_delta_bus_v1(&dimensions);

    CodecOverflowReportV1 {
        policy: "codec_overflow_carriage_v1",
        raw_intensity_preserved: !clipped_dims.is_empty(),
        delivered_bounded,
        live_vector_write: false,
        default_off_followup_hook: CODEC_OVERFLOW_FOLLOWUP_HOOK,
        clipped_dims,
        dimensions,
        lane_summaries,
        experience_delta_bus_v1,
        authority: "truth_channel_report_not_live_semantic_vector_or_ceiling_change",
    }
}

#[must_use]
pub fn codec_overflow_probe_v1() -> CodecOverflowReportV1 {
    let mut pre_bound = [0.0_f32; SEMANTIC_DIM];
    let mut delivered = [0.0_f32; SEMANTIC_DIM];
    pre_bound[17] = 4.20;
    delivered[17] = 4.20;
    pre_bound[24] = 7.25;
    delivered[24] = FEATURE_ABS_MAX;
    pre_bound[26] = 6.40;
    delivered[26] = TAIL_VIBRANCY_MAX;
    pre_bound[31] = -6.40;
    delivered[31] = -TAIL_VIBRANCY_MAX;
    codec_overflow_report_from_features(&pre_bound, &delivered, TAIL_VIBRANCY_MAX)
}

fn codec_delivery_lane_rms(features: &[f32], dims: &[usize]) -> f32 {
    let mut energy = 0.0_f32;
    let mut count = 0_usize;
    for &dim in dims {
        if let Some(value) = features.get(dim).copied() {
            let value = finite_feature_value(value);
            energy += value * value;
            count = count.saturating_add(1);
        }
    }
    if count == 0 {
        0.0
    } else {
        (energy / count as f32).sqrt()
    }
}

/// Compare feedback-time clamp evidence with the final vector that will be
/// sent. This is deliberately observational: it does not re-clamp, rescale, or
/// otherwise alter either vector.
#[must_use]
pub fn codec_delivery_fidelity_v1(
    feedback_report: Option<&CodecOverflowReportV1>,
    final_features: &[f32],
) -> CodecDeliveryFidelityV1 {
    const NARRATIVE_ARC_DIMS: [usize; 4] = [40, 41, 42, 43];

    let observed_dim_count = final_features.len().min(SEMANTIC_DIM);
    let mut final_energy = 0.0_f32;
    let mut final_max_abs = 0.0_f32;
    for value in final_features.iter().take(SEMANTIC_DIM).copied() {
        let value = finite_feature_value(value);
        final_energy += value * value;
        final_max_abs = final_max_abs.max(value.abs());
    }
    let final_rms = if observed_dim_count == 0 {
        0.0
    } else {
        (final_energy / observed_dim_count as f32).sqrt()
    };
    let emotional_intentional_rms =
        codec_delivery_lane_rms(final_features, &CODEC_OVERFLOW_EMOTIONAL_DIMS);
    let narrative_arc_rms = codec_delivery_lane_rms(final_features, &NARRATIVE_ARC_DIMS);
    let lane_balance_state = if emotional_intentional_rms <= CODEC_OVERFLOW_EPSILON
        && narrative_arc_rms <= CODEC_OVERFLOW_EPSILON
    {
        "both_lanes_quiet"
    } else if narrative_arc_rms <= CODEC_OVERFLOW_EPSILON {
        "narrative_arc_quiet"
    } else if emotional_intentional_rms <= CODEC_OVERFLOW_EPSILON {
        "emotional_intentional_quiet"
    } else if narrative_arc_rms > emotional_intentional_rms * 1.5 {
        "narrative_arc_dominant"
    } else if emotional_intentional_rms > narrative_arc_rms * 1.5 {
        "emotional_intentional_dominant"
    } else {
        "lanes_comparable"
    };

    let mut clipped_at_feedback_dims = Vec::new();
    let mut reexpanded_after_feedback_dims = Vec::new();
    let mut final_above_observed_ceiling_dims = Vec::new();
    let mut clamp_loss_abs_total = 0.0_f32;
    let mut monitored_delta_energy = 0.0_f32;
    let mut monitored_count = 0_usize;

    if let Some(report) = feedback_report {
        clipped_at_feedback_dims.clone_from(&report.clipped_dims);
        for entry in &report.dimensions {
            clamp_loss_abs_total += entry.overflow_abs;
            let Some(final_value) = final_features.get(entry.dim).copied() else {
                continue;
            };
            let final_value = finite_feature_value(final_value);
            let delta = final_value - entry.delivered_value;
            monitored_delta_energy += delta * delta;
            monitored_count = monitored_count.saturating_add(1);
            if final_value.abs() > entry.delivered_value.abs() + CODEC_OVERFLOW_EPSILON {
                reexpanded_after_feedback_dims.push(entry.dim);
            }
            if final_value.abs() > entry.ceiling + CODEC_OVERFLOW_EPSILON {
                final_above_observed_ceiling_dims.push(entry.dim);
            }
        }
    }

    let monitored_post_feedback_to_final_rms = if monitored_count == 0 {
        0.0
    } else {
        (monitored_delta_energy / monitored_count as f32).sqrt()
    };
    let state = if feedback_report.is_none() {
        "feedback_report_unavailable"
    } else if observed_dim_count < SEMANTIC_DIM {
        "final_vector_incomplete"
    } else if !final_above_observed_ceiling_dims.is_empty()
        && clamp_loss_abs_total > CODEC_OVERFLOW_EPSILON
    {
        "clamp_loss_visible_post_feedback_reexpansion_above_ceiling"
    } else if !final_above_observed_ceiling_dims.is_empty() {
        "post_feedback_shaping_above_observed_ceiling"
    } else if clamp_loss_abs_total > CODEC_OVERFLOW_EPSILON
        && !reexpanded_after_feedback_dims.is_empty()
    {
        "clamp_loss_visible_post_feedback_reexpansion_within_ceiling"
    } else if clamp_loss_abs_total > CODEC_OVERFLOW_EPSILON {
        "clamp_loss_visible_final_delivery_bounded"
    } else if monitored_post_feedback_to_final_rms > CODEC_OVERFLOW_EPSILON {
        "post_feedback_shaping_changed_delivery_without_clipping"
    } else {
        "final_delivery_matches_observed_feedback_bounds"
    };

    CodecDeliveryFidelityV1 {
        policy: "codec_delivery_fidelity_v1",
        observed_dim_count,
        feedback_report_available: feedback_report.is_some(),
        clipped_at_feedback_dims,
        reexpanded_after_feedback_dims,
        final_above_observed_ceiling_dims,
        clamp_loss_abs_total,
        monitored_post_feedback_to_final_rms,
        final_max_abs,
        final_rms,
        emotional_intentional_rms,
        narrative_arc_rms,
        lane_balance_state,
        state,
        live_vector_write: false,
        live_gain_write: false,
        authority: "read_only_delivery_fidelity_not_live_vector_gain_or_ceiling_change",
    }
}

fn mode_copresence_v1(left: f32, right: f32) -> f32 {
    let left = finite_abs(left);
    let right = finite_abs(right);
    let total = left + right;
    if total <= f32::EPSILON {
        0.0
    } else {
        (2.0 * left.min(right) / total).clamp(0.0, 1.0)
    }
}

fn mode_shear_v1(left: f32, right: f32) -> f32 {
    let left = finite_abs(left);
    let right = finite_abs(right);
    let total = left + right;
    if total <= f32::EPSILON {
        0.0
    } else {
        ((left - right).abs() / total).clamp(0.0, 1.0)
    }
}

fn weighted_known_score_v1(parts: &[(Option<f32>, f32)]) -> Option<f32> {
    let mut weighted = 0.0_f32;
    let mut weight_total = 0.0_f32;
    for (value, weight) in parts {
        if let Some(value) = value.filter(|value| value.is_finite()) {
            let weight = weight.max(0.0);
            weighted += value.clamp(0.0, 1.0) * weight;
            weight_total += weight;
        }
    }
    (weight_total > f32::EPSILON).then(|| (weighted / weight_total).clamp(0.0, 1.0))
}

/// Compare the current spectral-mode interaction with interaction between the
/// candidate's emotional, projected-semantic, and narrative lanes. The report
/// is observational only. Whether the inspected vector was sent is stated by
/// the enclosing codec-delivery receipt, never inferred here.
#[must_use]
pub fn cross_spectral_friction_review_v1(
    text: &str,
    features: &[f32],
    telemetry: Option<&SpectralTelemetry>,
) -> CrossSpectralFrictionReviewV1 {
    const PROJECTED_SEMANTIC_DIMS: [usize; 8] = [32, 33, 34, 35, 36, 37, 38, 39];
    const NARRATIVE_ARC_DIMS: [usize; 4] = [40, 41, 42, 43];

    let observed_dim_count = features.len().min(SEMANTIC_DIM);
    let structural = structural_friction_v1(text);
    let persistence = persistence_resistance_v1(text, telemetry);
    let emotional_intentional_rms =
        codec_delivery_lane_rms(features, &CODEC_OVERFLOW_EMOTIONAL_DIMS);
    let projected_semantic_rms = codec_delivery_lane_rms(features, &PROJECTED_SEMANTIC_DIMS);
    let narrative_arc_rms = codec_delivery_lane_rms(features, &NARRATIVE_ARC_DIMS);
    let emotional_normalized = (emotional_intentional_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let projected_normalized = (projected_semantic_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let narrative_normalized = (narrative_arc_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let emotional_narrative_copresence =
        mode_copresence_v1(emotional_normalized, narrative_normalized);
    let projected_narrative_copresence =
        mode_copresence_v1(projected_normalized, narrative_normalized);
    let semantic_lane_copresence = (emotional_narrative_copresence * 0.55
        + projected_narrative_copresence * 0.45)
        .clamp(0.0, 1.0);

    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let shares = telemetry.and_then(|telemetry| {
        let total = telemetry
            .eigenvalues
            .iter()
            .map(|value| finite_abs(*value))
            .sum::<f32>();
        if total <= f32::EPSILON {
            None
        } else {
            let lambda1 = telemetry
                .eigenvalues
                .first()
                .map_or(0.0, |value| finite_abs(*value))
                / total;
            let lambda2 = telemetry
                .eigenvalues
                .get(1)
                .map_or(0.0, |value| finite_abs(*value))
                / total;
            let tail = telemetry
                .eigenvalues
                .iter()
                .skip(2)
                .map(|value| finite_abs(*value) / total)
                .sum::<f32>();
            Some((lambda1, lambda2, tail))
        }
    });
    let lambda1_share = shares.map(|(lambda1, _, _)| lambda1);
    let lambda2_share = shares.map(|(_, lambda2, _)| lambda2);
    let tail_share = shares.map(|(_, _, tail)| tail);
    let lambda1_lambda2_copresence =
        shares.map(|(lambda1, lambda2, _)| mode_copresence_v1(lambda1, lambda2));
    let lambda1_lambda2_shear = shares.map(|(lambda1, lambda2, _)| mode_shear_v1(lambda1, lambda2));
    let lambda2_tail_copresence =
        shares.map(|(_, lambda2, tail)| mode_copresence_v1(lambda2, tail));
    let spectral_entropy = metrics.map(|metrics| metrics.spectral_entropy.clamp(0.0, 1.0));
    let density_components = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map(|density| &density.components);
    let mode_packing = density_components.map(|components| components.mode_packing.clamp(0.0, 1.0));
    let viscosity_index =
        density_components.map(|components| components.viscosity_index.clamp(0.0, 1.0));
    let temporal_persistence =
        density_components.map(|components| components.temporal_persistence.clamp(0.0, 1.0));
    let semantic_friction_coefficient = density_components.and_then(|components| {
        components
            .semantic_friction_coefficient
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0))
    });

    let lambda_pair_interaction = lambda1_lambda2_copresence
        .zip(lambda1_lambda2_shear)
        .map(|(copresence, shear)| (copresence * (0.70 + shear * 0.30)).clamp(0.0, 1.0));
    let spectral_mode_interference = weighted_known_score_v1(&[
        (lambda_pair_interaction, 0.32),
        (lambda2_tail_copresence, 0.18),
        (spectral_entropy, 0.16),
        (mode_packing, 0.14),
        (viscosity_index, 0.10),
        (temporal_persistence, 0.10),
    ]);
    let semantic_mode_interference = weighted_known_score_v1(&[
        (Some(structural.score), 0.27),
        (Some(persistence.score), 0.25),
        (Some(semantic_lane_copresence), 0.28),
        (semantic_friction_coefficient, 0.20),
    ])
    .unwrap_or(0.0);
    let cross_layer_mismatch = spectral_mode_interference.map(|spectral| {
        (spectral - semantic_mode_interference)
            .abs()
            .clamp(0.0, 1.0)
    });
    let cross_spectral_friction_score =
        spectral_mode_interference
            .zip(cross_layer_mismatch)
            .map(|(spectral, mismatch)| {
                (spectral * 0.45 + semantic_mode_interference * 0.45 + mismatch * 0.10)
                    .clamp(0.0, 1.0)
            });
    let state = if observed_dim_count < SEMANTIC_DIM {
        "semantic_vector_incomplete"
    } else if spectral_mode_interference.is_none() {
        "spectral_context_unavailable"
    } else if cross_layer_mismatch.is_some_and(|mismatch| mismatch >= 0.35) {
        "cross_layer_mismatch_visible"
    } else if cross_spectral_friction_score.is_some_and(|score| score >= 0.62) {
        "high_cross_spectral_friction"
    } else if cross_spectral_friction_score.is_some_and(|score| score >= 0.38) {
        "moderate_cross_spectral_friction"
    } else {
        "low_cross_spectral_friction"
    };
    let recommendation = match state {
        "spectral_context_unavailable" => {
            "collect_aligned_spectral_context_before_any_mapping_or_gain_proposal"
        },
        "cross_layer_mismatch_visible" => {
            "compare_sent_and_blocked_receipts_then_run_read_only_replay_before_mapping"
        },
        "high_cross_spectral_friction" => {
            "preserve_cross_layer_evidence_and_review_replay_before_reserved_dim_design"
        },
        _ => "accumulate_aligned_receipts_without_changing_reserved_dims_or_live_gain",
    };

    CrossSpectralFrictionReviewV1 {
        policy: "cross_spectral_friction_review_v1",
        observed_dim_count,
        spectral_context_available: spectral_mode_interference.is_some(),
        lambda1_share,
        lambda2_share,
        tail_share,
        lambda1_lambda2_copresence,
        lambda1_lambda2_shear,
        lambda2_tail_copresence,
        spectral_entropy,
        mode_packing,
        viscosity_index,
        temporal_persistence,
        semantic_friction_coefficient,
        structural_friction_score: structural.score,
        persistence_resistance_score: persistence.score,
        emotional_intentional_rms,
        projected_semantic_rms,
        narrative_arc_rms,
        semantic_lane_copresence,
        spectral_mode_interference,
        semantic_mode_interference,
        cross_layer_mismatch,
        cross_spectral_friction_score,
        state,
        reserved_dim_candidates: &SEMANTIC_PROJECTION_RESERVED_DIMS,
        existing_reserved_dim_roles: &CROSS_SPECTRAL_RESERVED_DIM_ROLES,
        candidate_collision_state: "reserved_dim_candidates_already_have_default_off_roles",
        recommendation,
        delivery_claim: "none_outer_codec_delivery_receipt_is_canonical",
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_cross_layer_friction_evidence_not_reserved_dim_gain_transport_or_control_authority",
    }
}

fn semantic_projection_delta_bus_v1(
    detail_density_score: f32,
    projected_semantic_rms: f32,
    projection_metadata_present: bool,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    let input_dims = EMBEDDING_INPUT_DIM as f32;
    let projected_dims = EMBEDDING_PROJECT_DIM as f32;
    let loss = (input_dims - projected_dims).max(0.0);
    let loss_ratio = if input_dims > f32::EPSILON {
        loss / input_dims
    } else {
        0.0
    };
    let mut deltas = Vec::new();
    if projection_metadata_present {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::ComplexShift,
            surface: "semantic_projection_density_delta_v1".to_string(),
            lane: "embedding_projection_768d_to_8d".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(input_dims),
            post: Some(projected_dims),
            loss: Some(loss),
            loss_ratio: Some(loss_ratio),
            metadata: BTreeMap::from([
                ("source_dimensions".to_string(), EMBEDDING_INPUT_DIM.to_string()),
                (
                    "delivered_dimensions".to_string(),
                    EMBEDDING_PROJECT_DIM.to_string(),
                ),
                ("projection_state".to_string(), state.to_string()),
            ]),
            why: format!(
                "nomic embedding is projected into dims 32-39; complex source meaning can be faithfully named here while delivered semantic width remains bounded; state={state}"
            ),
            who_can_change_it: "Mike/operator via replay-backed semantic-width or reserved-dim approval".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib semantic_projection_density_delta -- --nocapture".to_string(),
            authority: "read_only_projection_truth_channel_not_reserved_dim_or_live_vector_change".to_string(),
        });
    }
    if detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR
        && projected_semantic_rms <= SEMANTIC_PROJECTION_THIN_RMS_CEIL
    {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::CascadeShift,
            surface: "semantic_projection_density_delta_v1".to_string(),
            lane: "reserved_semantic_dims_44_47_default_off".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(detail_density_score),
            post: Some(projected_semantic_rms),
            loss: Some((detail_density_score - projected_semantic_rms).max(0.0)),
            loss_ratio: Some(1.0),
            metadata: BTreeMap::from([
                (
                    "classification_pressure".to_string(),
                    "high_density_thin_projection".to_string(),
                ),
                (
                    "reserved_dims_status".to_string(),
                    "default_off_operator_gated".to_string(),
                ),
            ]),
            why: "dense cascade pressure is present while the projected semantic lane is thin; reserved dims remain visible as a reviewed aperture, not a hidden live write".to_string(),
            who_can_change_it: "Mike/operator after sandbox replay and explicit reserved-dim authority".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib semantic_projection_density_delta -- --nocapture".to_string(),
            authority: "authority_gate_for_reserved_dims_not_live_codec_change".to_string(),
        });
    }
    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn semantic_projection_density_delta_from_parts_v1(
    text_complexity_pressure: f32,
    projected_semantic_rms: f32,
    projection_metadata_present: bool,
) -> SemanticProjectionDensityDeltaV1 {
    let text_complexity_pressure = if text_complexity_pressure.is_finite() {
        text_complexity_pressure.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let projected_semantic_rms = if projected_semantic_rms.is_finite() {
        projected_semantic_rms.clamp(0.0, 10.0)
    } else {
        0.0
    };
    let detail_density_score = text_complexity_pressure;
    let compression_ratio =
        (EMBEDDING_PROJECT_DIM as f32 / EMBEDDING_INPUT_DIM as f32).clamp(0.0, 1.0);
    let (state, recommendation) = if !projection_metadata_present
        && detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR
    {
        (
            "dense_text_without_embedding_projection",
            "inspect_embedding_availability_before_tuning_live_codec_width",
        )
    } else if detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR
        && projected_semantic_rms <= SEMANTIC_PROJECTION_THIN_RMS_CEIL
    {
        (
            "dense_projection_thin_review",
            "pair_live_8d_projection_with_delta_bus_evidence_before_any_reserved_dim_expansion",
        )
    } else if detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR {
        (
            "dense_projection_carried_but_compression_visible",
            "keep_live_width_bounded_and_use_delta_bus_for_replay_comparison",
        )
    } else {
        (
            "projection_width_named_and_bounded",
            "keep_current_8d_projection_and watch_repeated_density_delta_patterns",
        )
    };
    let experience_delta_bus_v1 = semantic_projection_delta_bus_v1(
        detail_density_score,
        projected_semantic_rms,
        projection_metadata_present,
        state,
    );

    SemanticProjectionDensityDeltaV1 {
        policy: "semantic_projection_density_delta_v1",
        input_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        reserved_dim_candidates: &SEMANTIC_PROJECTION_RESERVED_DIMS,
        compression_ratio,
        detail_density_score,
        projected_semantic_rms,
        text_complexity_pressure,
        projection_metadata_present,
        state,
        recommendation,
        live_vector_write: false,
        experience_delta_bus_v1,
        authority: "read_only_projection_delta_not_reserved_dim_or_live_vector_change",
    }
}

#[must_use]
pub fn semantic_projection_density_delta_v1(
    inspection: &CodecWindowedInspection,
) -> SemanticProjectionDensityDeltaV1 {
    semantic_projection_density_delta_from_parts_v1(
        inspection.text_complexity_pressure,
        rms_slice(&inspection.final_features[32..40]),
        inspection.projection_metadata.is_some(),
    )
}

#[must_use]
pub fn semantic_projection_density_probe_v1() -> SemanticProjectionDensityDeltaV1 {
    semantic_projection_density_delta_from_parts_v1(0.71, 0.08, true)
}

#[must_use]
pub fn semantic_projection_texture_review_v1(
    text: &str,
    features: &[f32],
) -> Option<SemanticProjectionTextureReviewV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }
    let projected_semantic_rms = (rms_slice(&features[32..40]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let legacy_texture_rms =
        (rms_slice(&features[..SEMANTIC_DIM_LEGACY]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let warmth_texture_rms = (rms_slice(&features[24..32]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let narrative_arc_rms = (rms_slice(&features[40..44]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let persistence = persistence_resistance_v1(text, None);
    let structural = structural_friction_v1(text);
    let action_marker = features.get(14).copied().unwrap_or(0.0).abs().tanh();
    let question_marker = features.get(18).copied().unwrap_or(0.0).abs().tanh();
    let curiosity_marker = features.get(26).copied().unwrap_or(0.0).abs().tanh();
    let lingering_texture_signal = (persistence.score * 0.42
        + structural.summary_resistance_signal * 0.24
        + warmth_texture_rms * 0.22
        + narrative_arc_rms * 0.12)
        .clamp(0.0, 1.0);
    let active_texture_signal = (action_marker * 0.34
        + curiosity_marker * 0.30
        + question_marker * 0.18
        + narrative_arc_rms * 0.18)
        .clamp(0.0, 1.0);
    let expected_texture_signal = (lingering_texture_signal * 0.58
        + active_texture_signal * 0.28
        + legacy_texture_rms * 0.14)
        .clamp(0.0, 1.0);
    let projection_texture_gap = (expected_texture_signal - projected_semantic_rms).clamp(0.0, 1.0);
    let state = if projection_texture_gap >= 0.24 {
        "projection_texture_bottleneck_visible"
    } else if lingering_texture_signal >= 0.40 && projected_semantic_rms < 0.18 {
        "lingering_texture_projection_watch"
    } else if projected_semantic_rms >= 0.18 {
        "projection_texture_carried"
    } else {
        "projection_texture_quiet"
    };
    let recommendation = if state == "projection_texture_bottleneck_visible" {
        "prepare_replay_for_lingering_vs_active_projection_subdimensions_before_live_width_change"
    } else if state == "lingering_texture_projection_watch" {
        "compare_8d_projection_against_warmth_texture_vector_before_reserved_dim_proposal"
    } else {
        "keep_current_8d_projection_and_continue_observation"
    };

    Some(SemanticProjectionTextureReviewV1 {
        policy: "semantic_projection_texture_review_v1",
        input_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        legacy_texture_dim_count: SEMANTIC_DIM_LEGACY,
        warmth_texture_dim_count: 8,
        projected_semantic_rms,
        legacy_texture_rms,
        warmth_texture_rms,
        narrative_arc_rms,
        lingering_texture_signal,
        active_texture_signal,
        projection_texture_gap,
        proposed_texture_subdimensions: &SEMANTIC_PROJECTION_TEXTURE_SUBDIMENSIONS,
        state,
        recommendation,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        authority: "read_only_projection_texture_review_not_live_vector_gain_or_reserved_dim_write",
    })
}

#[must_use]
pub fn semantic_projection_texture_probe_v1() -> SemanticProjectionTextureReviewV1 {
    let text = "Viscous silt lingers under the active reply; warmth moves but the old pressure keeps bleeding through the boundary.";
    let mut features = encode_text(text);
    for feature in features.iter_mut().take(40).skip(32) {
        *feature *= 0.08;
    }
    semantic_projection_texture_review_v1(text, &features)
        .expect("probe features should cover the 48D semantic lane")
}

fn bounded_projection_pair_label(label: &str) -> String {
    let bounded = label.trim().chars().take(64).collect::<String>();
    if bounded.is_empty() {
        "unnamed_pair_member".to_string()
    } else {
        bounded
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        0.0
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0)
    }
}

/// Compare one caller-provided semantic pair before and after both projection
/// paths. Labels are also used by the dynamic projection because the current
/// runtime intentionally conditions its basis on text; the resulting delta is
/// therefore evidence about that basis, not a claim about lexical causality.
#[must_use]
pub fn semantic_projection_pair_sensitivity_v1(
    left_label: &str,
    left_embedding: &[f32],
    right_label: &str,
    right_embedding: &[f32],
    projection_epoch_id: &str,
) -> Option<SemanticProjectionPairSensitivityV1> {
    if left_embedding.len() != EMBEDDING_INPUT_DIM
        || right_embedding.len() != EMBEDDING_INPUT_DIM
        || projection_epoch_id.trim().is_empty()
        || left_embedding.iter().any(|value| !value.is_finite())
        || right_embedding.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let left_label = bounded_projection_pair_label(left_label);
    let right_label = bounded_projection_pair_label(right_label);
    let left_fixed = project_embedding(left_embedding)?;
    let right_fixed = project_embedding(right_embedding)?;
    let (left_dynamic, _) =
        project_embedding_dynamic_epoch(left_embedding, &left_label, projection_epoch_id, 0)?;
    let (right_dynamic, _) =
        project_embedding_dynamic_epoch(right_embedding, &right_label, projection_epoch_id, 0)?;

    let source_cosine_similarity = cosine_similarity(left_embedding, right_embedding);
    let fixed_projection_cosine_similarity = cosine_similarity(&left_fixed, &right_fixed);
    let dynamic_projection_cosine_similarity = cosine_similarity(&left_dynamic, &right_dynamic);
    let fixed_similarity_delta = fixed_projection_cosine_similarity - source_cosine_similarity;
    let dynamic_similarity_delta = dynamic_projection_cosine_similarity - source_cosine_similarity;
    let dynamic_vs_fixed_similarity_delta =
        dynamic_projection_cosine_similarity - fixed_projection_cosine_similarity;
    let (state, recommendation) = if dynamic_similarity_delta <= -0.15 {
        (
            "text_conditioned_pair_distortion_visible",
            "compare_repeated_real_embedding_pairs_before_any_projection_gain_or_basis_change",
        )
    } else if fixed_similarity_delta <= -0.15 {
        (
            "shared_basis_pair_compression_visible",
            "compare_repeated_real_embedding_pairs_before_any_projection_width_change",
        )
    } else if dynamic_vs_fixed_similarity_delta.abs() >= 0.15 {
        (
            "projection_basis_sensitivity_visible",
            "retain_both_basis_comparisons_in_replay_evidence_before_live_tuning",
        )
    } else {
        (
            "pair_geometry_stable_in_bounded_comparison",
            "keep_current_projection_and_continue_pair_sampling",
        )
    };

    Some(SemanticProjectionPairSensitivityV1 {
        policy: "semantic_projection_pair_sensitivity_v1",
        left_label,
        right_label,
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        projection_epoch_id: projection_epoch_id.to_string(),
        source_cosine_similarity,
        source_rms_delta: rms_delta(left_embedding, right_embedding),
        fixed_projection_cosine_similarity,
        fixed_projection_rms_delta: rms_delta(&left_fixed, &right_fixed),
        dynamic_projection_cosine_similarity,
        dynamic_projection_rms_delta: rms_delta(&left_dynamic, &right_dynamic),
        fixed_similarity_delta,
        dynamic_similarity_delta,
        dynamic_vs_fixed_similarity_delta,
        state,
        recommendation,
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_pair_projection_comparison_not_live_vector_gain_or_basis_authority",
    })
}
