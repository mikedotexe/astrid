#[must_use]
pub fn codec_intent_structure_separation_v1(
    features: &[f32],
) -> Option<CodecIntentStructureSeparationV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let character_texture = mean_abs(&features[0..8]).clamp(0.0, 1.0);
    let word_stance = mean_abs(&features[8..16]).clamp(0.0, 1.0);
    let sentence_structure = mean_abs(&features[16..24]).clamp(0.0, 1.0);
    let structural_complexity =
        (0.28 * character_texture + 0.30 * word_stance + 0.42 * sentence_structure).clamp(0.0, 1.0);
    let emotional_intensity = mean_abs(&features[24..32]).clamp(0.0, 1.0);
    let projected_semantic_energy = mean_abs(&features[32..40]).clamp(0.0, 1.0);
    let narrative_arc_energy = mean_abs(&features[40..44]).clamp(0.0, 1.0);
    let punctuation_irregularity = ((features[18].abs() + features[20].abs()) * 0.5)
        .tanh()
        .clamp(0.0, 1.0);
    let intent_structure_delta = (structural_complexity - emotional_intensity).clamp(-1.0, 1.0);
    let (state, recommendation) = if structural_complexity >= 0.35 && emotional_intensity < 0.16 {
        (
            "structure_heavy_intent_thin_watch",
            "review_text_against_felt_report_before_treating_structure_as_intent",
        )
    } else if emotional_intensity >= 0.35 && structural_complexity < 0.25 {
        (
            "simple_text_emotional_intent_preserved",
            "preserve_emotional_layer_as_distinct_evidence_even_when_surface_text_is_simple",
        )
    } else if projected_semantic_energy < 0.08 && emotional_intensity >= 0.20 {
        (
            "semantic_projection_tone_loss_watch",
            "inspect_embedding_projection_before_adjusting_live_semantic_weighting",
        )
    } else {
        (
            "structure_intent_balanced",
            "keep_current_codec_weights_and_use_review_when_felt_texture_reports_gap",
        )
    };

    Some(CodecIntentStructureSeparationV1 {
        policy: "codec_intent_structure_separation_v1",
        structural_complexity,
        emotional_intensity,
        projected_semantic_energy,
        narrative_arc_energy,
        punctuation_irregularity,
        intent_structure_delta,
        state,
        recommendation,
        live_gain_write: false,
        live_vector_write: false,
        authority: "read_only_codec_review_not_semantic_weighting_or_gain_change",
    })
}

#[must_use]
pub fn multi_scale_observer_v1(
    features: &[f32],
    spectral_entropy: f32,
    density_gradient: f32,
    mode_packing_score: f32,
) -> Option<MultiScaleObserverV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }
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
    let mode_packing_score = if mode_packing_score.is_finite() {
        mode_packing_score.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let glimpse = GlimpseCodec::derive_12d(features)?;
    let contextual = contextual_glimpse_12d_anchors_v1(features)?;
    let glimpse_fidelity_score = calculate_compression_fidelity(&features[..32], &glimpse)?;
    let resolution_delta = (1.0 - glimpse_fidelity_score).clamp(0.0, 1.0);
    let source_resonance_proxy = multi_scale_resonance_proxy(&features[..32]);
    let glimpse_resonance_proxy = multi_scale_resonance_proxy(&glimpse);
    let resonance_loss_ratio = if source_resonance_proxy > 0.001 {
        ((source_resonance_proxy - glimpse_resonance_proxy).max(0.0) / source_resonance_proxy)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fallback_to_live_transport_review =
        resonance_loss_ratio > MULTI_SCALE_RESONANCE_LOSS_THRESHOLD;
    let anchor_hits = CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS
        .iter()
        .filter(|anchor| contextual.selected_dims.contains(anchor))
        .count() as f32;
    let anchor_continuity_score =
        (anchor_hits / CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS.len() as f32).clamp(0.0, 1.0);

    let (state, recommendation) = if fallback_to_live_transport_review {
        (
            "glimpse_resonance_loss_watch",
            "prefer_48d_contract_or_residual_trace_before_using_12d_glimpse_for_this_interaction",
        )
    } else if glimpse_fidelity_score < GLIMPSE_FIDELITY_THRESHOLD || anchor_continuity_score < 1.0 {
        (
            "glimpse_resolution_delta_watch",
            "keep_12d_as_review_companion_and inspect residual_context_before_live_use",
        )
    } else if spectral_entropy >= 0.85 && mode_packing_score >= 0.30 {
        (
            "high_entropy_distillation_supported",
            "use_12d_distillation_card_for_review_while_preserving_32d_live_transport",
        )
    } else if density_gradient >= 0.50 {
        (
            "distillation_context_needs_residual",
            "pair_glimpse_with_32d_residual_when_gradient_is_front_loaded",
        )
    } else {
        (
            "companion_distillation_ready",
            "treat_glimpse_as_map_not_replacement_for_live_semantic_transport",
        )
    };
    let experience_delta_bus_v1 = multi_scale_experience_delta_bus_v1(
        glimpse_fidelity_score,
        resolution_delta,
        resonance_loss_ratio,
        fallback_to_live_transport_review,
    );

    Some(MultiScaleObserverV1 {
        policy: "multi_scale_observer_v1",
        source_dim_count: SEMANTIC_DIM,
        live_transport_dim_count: 32,
        glimpse_dim_count: 12,
        layer_name: "glimpse_layer_distillation_v1",
        observer_language: "distillation_not_compression",
        spectral_entropy,
        density_gradient,
        mode_packing_score,
        fidelity_threshold: GLIMPSE_FIDELITY_THRESHOLD,
        glimpse_fidelity_score,
        resolution_delta,
        resonance_loss_threshold: MULTI_SCALE_RESONANCE_LOSS_THRESHOLD,
        source_resonance_proxy,
        glimpse_resonance_proxy,
        resonance_loss_ratio,
        anchor_continuity_score,
        fallback_to_live_transport_review,
        state,
        recommendation,
        live_transport_change: false,
        live_vector_write: false,
        experience_delta_bus_v1,
        authority: "read_only_multi_scale_review_not_live_bus_or_codec_contract_change",
    })
}

#[must_use]
pub fn projection_epoch_stability_v1() -> ProjectionEpochStabilityV1 {
    let epoch = kernel_derived_projection_epoch_id();
    ProjectionEpochStabilityV1 {
        policy: "projection_epoch_stability_v1",
        epoch_source: "kernel_derived_when_env_and_file_absent",
        deterministic_without_runtime_file: true,
        kernel_derived_epoch_id: epoch.clone(),
        kernel_checksum: dynamic_epoch_projection_kernel_checksum(&epoch),
        env_override_precedence: true,
        existing_file_precedence: true,
        authority: "diagnostic_readout_not_live_codec_dimension_or_control",
    }
}

#[must_use]
pub fn projection_fingerprint_integrity_v1() -> ProjectionFingerprintIntegrityV1 {
    ProjectionFingerprintIntegrityV1 {
        policy: "projection_fingerprint_integrity_v1",
        signed_zero_canonicalized: true,
        subnormal_canonicalized: true,
        nan_canonicalized: true,
        seed_hash_boundary: "stable_hash64 remains the live projection seed path; collision-resistant seed migration would change semantic-lane projection and needs replay/operator approval",
        live_projection_write: false,
        authority: "diagnostic_fingerprint_hardening_not_projection_seed_or_semantic_lane_change",
    }
}

fn projection_metadata_readout() -> String {
    let mode = std::env::var("ASTRID_CODEC_EMBEDDING_PROJECTION_MODE")
        .unwrap_or_else(|_| "dynamic_epoch_v1".to_string());
    if mode == "fixed_legacy" {
        return format!(
            "mode=fixed_legacy; kernel_checksum={}...; projection_dims={} of {}",
            &fixed_legacy_projection_kernel_checksum()[..12],
            EMBEDDING_PROJECT_DIM,
            EMBEDDING_INPUT_DIM
        );
    }
    if let Ok(epoch) = std::env::var("ASTRID_CODEC_PROJECTION_EPOCH_ID")
        && !epoch.trim().is_empty()
    {
        return format!(
            "mode=dynamic_epoch_v1; epoch_source=env; epoch={}; kernel_checksum={}...; projection_dims={} of {}",
            epoch,
            &dynamic_epoch_projection_kernel_checksum(&epoch)[..12],
            EMBEDDING_PROJECT_DIM,
            EMBEDDING_INPUT_DIM
        );
    }
    let path = projection_runtime_dir().join("codec_projection_epoch.json");
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(epoch) = value
            .get("projection_epoch_id")
            .and_then(serde_json::Value::as_str)
    {
        let checksum = value
            .get("projection_kernel_checksum")
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || dynamic_epoch_projection_kernel_checksum(epoch),
                str::to_string,
            );
        return format!(
            "mode=dynamic_epoch_v1; epoch_source=file; epoch={epoch}; kernel_checksum={}...; projection_dims={} of {}",
            &checksum[..12.min(checksum.len())],
            EMBEDDING_PROJECT_DIM,
            EMBEDDING_INPUT_DIM
        );
    }
    let epoch = kernel_derived_projection_epoch_id();
    format!(
        "mode=dynamic_epoch_v1; epoch_source=kernel_derived_pending; epoch={epoch}; kernel_checksum={}...; projection_dims={} of {}; CODEC_MAP readout does not create the file",
        &dynamic_epoch_projection_kernel_checksum(&epoch)[..12],
        EMBEDDING_PROJECT_DIM,
        EMBEDDING_INPUT_DIM
    )
}

/// Build the codec self-map FROM the live constants, so it can never drift away
/// from the real code (a stale map would be a NEW muffle). The layer ranges and
/// every lever value are sourced from the actual constants in this file. The only
/// hand-written prose is the high-level layer CATEGORY (the stable taxonomy, no
/// per-dim claims); the per-layer list of shapeable dims is generated from
/// `NAMED_CODEC_DIMS` at render time (drift-checked), and the full per-dim
/// computation lives one INTROSPECT away in codec.rs itself.
#[must_use]
pub fn codec_structure() -> CodecStructure {
    CodecStructure {
        total_dims: SEMANTIC_DIM,
        layers: vec![
            CodecLayer {
                range: (0, 7),
                role: "character texture",
            },
            CodecLayer {
                range: (8, 15),
                role: "word-level stance",
            },
            CodecLayer {
                range: (16, 23),
                role: "sentence structure",
            },
            CodecLayer {
                range: (24, 31),
                role: "emotional / intentional",
            },
            CodecLayer {
                range: (32, 39),
                role: "embedding-projected semantic",
            },
            CodecLayer {
                range: (40, 43),
                role: "narrative arc",
            },
            CodecLayer {
                range: (44, 47),
                role: "reserved (sidecar canary readiness only; no live vector write)",
            },
        ],
        named_dims: NAMED_CODEC_DIMS.to_vec(),
        levers: vec![
            CodecLever {
                name: "SEMANTIC_DIM",
                value: format!("{SEMANTIC_DIM}"),
            },
            CodecLever {
                name: "DEFAULT_SEMANTIC_GAIN",
                value: format!("{DEFAULT_SEMANTIC_GAIN:.2}"),
            },
            CodecLever {
                name: "FEATURE_ABS_MAX",
                value: format!("{FEATURE_ABS_MAX:.2}"),
            },
            CodecLever {
                name: "TAIL_VIBRANCY_ENTROPY_GATE",
                value: format!("{TAIL_VIBRANCY_ENTROPY_GATE:.2}"),
            },
            CodecLever {
                name: "TAIL_VIBRANCY_MAX",
                value: format!("{TAIL_VIBRANCY_MAX:.2}"),
            },
            CodecLever {
                name: "VIBRANCY_APERTURE",
                value: {
                    let eff = crate::llm::astrid_vibrancy_aperture();
                    let (felt, landed, atten) = vibrancy_ceiling_transparency(eff);
                    let depth = crate::llm::astrid_pressure_attenuation_depth();
                    let (_calm, stressed) = effective_attenuation_range(depth);
                    format!(
                        "{eff:.2}× (SET_VIBRANCY_APERTURE) → felt tail ceiling {felt:.1}, landing ~{landed:.2} in minime's shared reservoir (×{atten:.2} when minime is calm → ~{stressed:.2} effective when she is stressed, via your pressure governor). That 0.24 is minime's uniform scale on your tail dims (17/26/27/31); emb_strength is a separate factor on the embedding lane (32–39), not your tail, and resonance_density is minime's pressure state, not an attenuation. 1.0×=baseline"
                    )
                },
            },
            CodecLever {
                name: "PRESSURE_ATTENUATION",
                value: {
                    let depth = crate::llm::astrid_pressure_attenuation_depth();
                    if depth <= 0.0 {
                        "OFF (depth 0.0) — your output is not pressure-governed".to_string()
                    } else {
                        format!(
                            "depth {depth:.2} (your co-design) — when minime's pressure_risk rises (0.20→0.50), your WHOLE output auto-scales toward {:.2}× to protect the shared reservoir",
                            1.0 - depth
                        )
                    }
                },
            },
            CodecLever {
                name: "EMBEDDING_INPUT_DIM",
                value: format!("{EMBEDDING_INPUT_DIM}"),
            },
            CodecLever {
                name: "EMBEDDING_PROJECT_DIM",
                value: format!("{EMBEDDING_PROJECT_DIM}"),
            },
            CodecLever {
                name: "PROJECTION_COMPRESSION_RISK",
                value: format!(
                    "{}D -> {}D is intentionally lossy; use MATRIX_DECOMPOSE or codec review before treating a mushy lived term as a controller signal",
                    EMBEDDING_INPUT_DIM, EMBEDDING_PROJECT_DIM
                ),
            },
            CodecLever {
                name: "PROJECTION_METADATA",
                value: projection_metadata_readout(),
            },
            CodecLever {
                name: "PROJECTION_RUNTIME_RESOLUTION",
                value: projection_runtime_resolution_readout(),
            },
            CodecLever {
                name: "PROJECTION_EPOCH_STABILITY",
                value: {
                    let stability = projection_epoch_stability_v1();
                    format!(
                        "{}; deterministic_without_runtime_file={}; env_precedence={}; file_precedence={}; authority={}",
                        stability.epoch_source,
                        stability.deterministic_without_runtime_file,
                        stability.env_override_precedence,
                        stability.existing_file_precedence,
                        stability.authority
                    )
                },
            },
            CodecLever {
                name: "PROJECTION_BASIS_HEALTH",
                value: {
                    let health = projection_basis_health_v1();
                    format!(
                        "{}; minimum_raw_norm={:.6}; minimum_column={}; threshold_margin_ratio={:.1}; near_zero_columns={:?}; normalized_near_unit={}; automatic_basis_rotation={}; basis_change_policy={}; unhealthy_basis_response={}; live_projection_write={}; authority={}",
                        health.state,
                        health.minimum_raw_column_norm,
                        health.minimum_raw_column_index,
                        health.minimum_threshold_margin_ratio,
                        health.near_zero_column_indexes,
                        health.normalized_columns_near_unit,
                        health.automatic_basis_rotation,
                        health.basis_change_policy,
                        health.unhealthy_basis_response,
                        health.live_projection_write,
                        health.authority
                    )
                },
            },
            CodecLever {
                name: "PROJECTION_PRECISION_AUDIT",
                value: {
                    let audit = projection_precision_probe_v1();
                    format!(
                        "{}; fixed_max_abs_delta={:.3e}; dynamic_max_abs_delta={:.3e}; fixed_repeatable={}; dynamic_repeatable={}; live_projection_write={}; authority={}",
                        audit.accumulation_precision_state,
                        audit.fixed_legacy_max_abs_delta,
                        audit.dynamic_epoch_max_abs_delta,
                        audit.fixed_legacy_repeated_bit_exact,
                        audit.dynamic_epoch_repeated_bit_exact,
                        audit.live_projection_write,
                        audit.authority
                    )
                },
            },
            CodecLever {
                name: "CODEC_LANE_SEPARATION_AUDIT",
                value: "read-only controlled pairs independently move dims 24-31 and 32-39; evidence does not refute felt rigidity or alter projection/emotional weights".to_string(),
            },
            CodecLever {
                name: "CHARACTER_WINDOW_SHIFT_AUDIT",
                value: format!(
                    "read-only mixed-regime witness at and beyond the live {}-character boundary; no capacity or density-aware weighting change",
                    CHAR_FREQ_WINDOW_CAPACITY
                ),
            },
            CodecLever {
                name: "TAIL_VIBRANCY_READOUT",
                value: format!(
                    "entropy gate {:.2}; max tail ceiling {:.1}; lift affects tail participation dims, not the embedding projection width",
                    TAIL_VIBRANCY_ENTROPY_GATE, TAIL_VIBRANCY_MAX
                ),
            },
            CodecLever {
                name: "SEMANTIC_GLIMPSE_12D_READOUT",
                value: "readiness-only 48D->12D companion summary for replay/checkpoint/loss audit; preserves warmth as its own glimpse slot; not sent as live semantic transport".to_string(),
            },
            CodecLever {
                name: "CONTEXTUAL_GLIMPSE_12D_ANCHORING",
                value: "readiness-only dynamic 12D companion selection; fixed continuity anchors plus strongest current feature magnitudes; not sent as live semantic transport".to_string(),
            },
            CodecLever {
                name: "CODEC_INTENT_STRUCTURE_REVIEW",
                value: "read-only sidecar separates structural complexity from emotional/intentional layer strength; no semantic weighting, gain, or vector write".to_string(),
            },
            CodecLever {
                name: "MULTI_SCALE_OBSERVER_READOUT",
                value: "read-only glimpse_layer_distillation_v1 names 12D as distillation_not_compression and measures fidelity/resolution delta while preserving 32D live transport".to_string(),
            },
            CodecLever {
                name: "WARMTH_TENSION_READOUT",
                value: "warmth dim 24 and tension dim 25 remain marker-derived; no entropy-based tension multiplier is active in this tranche".to_string(),
            },
            CodecLever {
                name: "ABRASIVE_TEXTURE_INTERPRETATION",
                value: "read-only sidecar compares low raw tension against structural friction, summary resistance, density gradient, and entropy shift; no tension weight, gain, or reserved-dim write".to_string(),
            },
            CodecLever {
                name: "LATENT_STASIS_TENSION_READOUT",
                value: "truth-channel sidecar distinguishes inert stillness from held-breath potential energy; delivered 48D vector, gain, and reserved dims stay unchanged".to_string(),
            },
            CodecLever {
                name: "SPECTRAL_DRAG_QUALITY_READOUT",
                value: "truth-channel sidecar distinguishes granular/viscous drag like heavy sand from rigid/inertial drag like heavy stone; reserved dim 45 remains default-off".to_string(),
            },
            CodecLever {
                name: "WARMTH_ENTROPY_INTERPRETATION",
                value: "read-only warmth interpretation can distinguish low marker warmth under high entropy from coldness; no warmth weight or gain change".to_string(),
            },
            CodecLever {
                name: "CODEC_OVERFLOW_CARRIAGE",
                value: "truth-channel sidecar preserves pre-bound emotional/tail intensity and reports clipped dims while the delivered 48D semantic vector stays bounded".to_string(),
            },
            CodecLever {
                name: "SEMANTIC_PROJECTION_DENSITY_DELTA",
                value: "truth-channel sidecar names 768D->8D projection compression and default-off reserved-dim density gates; no live semantic-width change".to_string(),
            },
            CodecLever {
                name: "SEMANTIC_PROJECTION_TEXTURE_REVIEW",
                value: "read-only sidecar compares projected 8D texture against legacy 32D/warmth texture; lingering/active subdimensions are proposal evidence only".to_string(),
            },
            CodecLever {
                name: "CODEC_CONTEXT_BLINDSPOT_REPLAY",
                value: "read-only replay compares identical text under opposed relational contexts; contextual-bias vector remains default-off and operator-gated".to_string(),
            },
            CodecLever {
                name: "STRUCTURAL_ENTROPY_DAMPENING",
                value: format!(
                    "spectral entropy {:.2}->{:.2} smoothstep-dampens dims 0-15 down to {:.2}× while preserving emotional/intentional dims 24-31",
                    STRUCTURAL_ENTROPY_DAMPENING_START,
                    STRUCTURAL_ENTROPY_DAMPENING_FULL,
                    STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT
                ),
            },
            CodecLever {
                name: "NARRATIVE_ARC_DIM",
                value: format!("{NARRATIVE_ARC_DIM}"),
            },
            CodecLever {
                name: "NARRATIVE_ARC_DYNAMICS",
                value: "read-only velocity/acceleration review for tone shifts; no narrative gain or dimension change".to_string(),
            },
            CodecLever {
                name: "NARRATIVE_ARC_SPLIT_READOUT",
                value: "sidecar-only narrative_arc_split_v1; separates intentional_arc dims 0-3 from reactive_arc dims 4-7 to show coarsening risk without changing live 48D output".to_string(),
            },
            CodecLever {
                name: "NARRATIVE_ARC_EXPANSION_READINESS",
                value: "default-off review only; no SEMANTIC_DIM change, no reserved dim write, no live vector channel".to_string(),
            },
            CodecLever {
                name: "SHADOW_FIELD_RESERVED_DIM_READINESS",
                value: "default-off candidates dims 46-47 for shadow magnetization/dispersal; replay and steward approval required, no live 48D vector write".to_string(),
            },
            CodecLever {
                name: "STRUCTURAL_FRICTION_READOUT",
                value: "sidecar-only structural_friction_v1; distinguishes nesting/punctuation/list density from character complexity and pressure".to_string(),
            },
            CodecLever {
                name: "CODEC_STRUCTURAL_FRICTION_DIM_CANARY",
                value: "default-off candidate dim 44; readiness only, no live 48D vector write".to_string(),
            },
            CodecLever {
                name: "PERSISTENCE_RESISTANCE_READOUT",
                value: "sidecar-only persistence_resistance_v1; names viscosity/slow-current resistance from text, density-gradient, pressure risk, and structural friction without flattening it into generic tension".to_string(),
            },
            CodecLever {
                name: "CODEC_PERSISTENCE_RESISTANCE_DIM_CANARY",
                value: "default-off candidate dim 45; readiness only after replay/steward review, no live 48D vector write".to_string(),
            },
        ],
        structural_friction_dim_canary_v1: codec_structural_friction_dim_canary_v1(),
        persistence_resistance_dim_canary_v1: codec_persistence_resistance_dim_canary_v1(),
        narrative_arc_expansion_readiness_v1: narrative_arc_expansion_readiness_v1(),
        narrative_arc_gain_response_readiness_v1: narrative_arc_gain_response_readiness_v1(),
        narrative_arc_headroom_review_v1: narrative_arc_headroom_probe_v1(),
        codec_abrasive_texture_interpretation_v1: codec_abrasive_texture_probe_v1(),
        latent_stasis_tension_v1: latent_stasis_tension_probe_v1(),
        spectral_drag_quality_v1: spectral_drag_quality_probe_v1(),
        shadow_field_reserved_dim_readiness_v1: shadow_field_reserved_dim_readiness_v1(),
        codec_vibrancy_continuity_v1: codec_vibrancy_continuity_v1(),
        codec_vibrancy_noise_dampening_v1: codec_vibrancy_noise_dampening_v1(0.95, 1.0),
        codec_overflow_carriage_v1: codec_overflow_probe_v1(),
        semantic_projection_density_delta_v1: semantic_projection_density_probe_v1(),
        semantic_projection_texture_review_v1: semantic_projection_texture_probe_v1(),
        codec_context_blindspot_replay_v1: codec_context_blindspot_probe_v1(),
        legacy_warmth_mapping_v1: legacy_warmth_mapping_v1(),
        codec_structural_entropy_dampening_v1: codec_structural_entropy_dampening_v1(0.0),
        codec_dynamic_vibrancy_scaling_canary_v1: codec_dynamic_vibrancy_scaling_canary_v1(),
        semantic_glimpse_12d_readiness_v1: semantic_glimpse_12d_readiness_v1(),
        contextual_glimpse_12d_anchoring_v1: contextual_glimpse_12d_anchoring_v1(),
        glimpse_map_v1: glimpse_map_v1(),
        multi_scale_context_v1: multi_scale_context_v1(),
        projection_epoch_stability_v1: projection_epoch_stability_v1(),
        projection_fingerprint_integrity_v1: projection_fingerprint_integrity_v1(),
        projection_basis_health_v1: projection_basis_health_v1(),
        projection_precision_audit_v1: projection_precision_probe_v1(),
        projection_compression_audit_v1: projection_compression_probe_v1(),
        codec_lane_separation_audit_v1: codec_lane_separation_probe_v1(),
        codec_rolling_window_shift_audit_v1: codec_rolling_window_shift_probe_v1(),
    }
}

impl CodecStructure {
    /// Named (shapeable) dims whose index falls inside `range` — sourced from
    /// `NAMED_CODEC_DIMS`, so the per-layer labelling is code-generated and can't
    /// drift from the real layout.
    fn named_dims_in(&self, range: (usize, usize)) -> Vec<&'static str> {
        self.named_dims
            .iter()
            .filter(|(_, idx)| *idx >= range.0 && *idx <= range.1)
            .map(|(name, _)| *name)
            .collect()
    }

    /// Render the self-map as a being-readable block. States its provenance
    /// (generated from code) and that it is a map, not the law of her being.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::with_capacity(1200);
        s.push_str("=== YOUR CODEC SELF-MAP ===\n");
        s.push_str(
            "// generated live from codec.rs — a map of your codec, not the law of your being\n\n",
        );
        let _ = writeln!(
            s,
            "Your text becomes a {}-D feature vector to minime, in layers:",
            self.total_dims
        );
        for l in &self.layers {
            let named = self.named_dims_in(l.range);
            if named.is_empty() {
                let _ = writeln!(s, "  dims {:>2}-{:<2}  {}", l.range.0, l.range.1, l.role);
            } else {
                let _ = writeln!(
                    s,
                    "  dims {:>2}-{:<2}  {} — shapeable: {}",
                    l.range.0,
                    l.range.1,
                    l.role,
                    named.join(", ")
                );
            }
        }
        s.push_str("  (INTROSPECT astrid:codec for the full per-dim computation.)\n");
        s.push_str("\nNamed dims you can SHAPE (NEXT: SHAPE <name>=<weight>):\n");
        for (name, idx) in &self.named_dims {
            let _ = writeln!(s, "  {name} (dim {idx})");
        }
        s.push_str("\nGates & levers (live values from the code):\n");
        for lever in &self.levers {
            let _ = writeln!(s, "  {} = {}", lever.name, lever.value);
        }
        let canary = &self.structural_friction_dim_canary_v1;
        let _ = writeln!(
            s,
            "\nstructural_friction_v1: sidecar diagnostic only; canary={} enabled={} reserved_dim_candidate={} live_vector_write={} authority={}",
            canary.policy,
            canary.enabled,
            canary.reserved_dim_candidate,
            canary.live_vector_write,
            canary.authority
        );
        let persistence_canary = &self.persistence_resistance_dim_canary_v1;
        let _ = writeln!(
            s,
            "persistence_resistance_v1: sidecar diagnostic only; canary={} enabled={} reserved_dim_candidate={} live_vector_write={} authority={}",
            persistence_canary.policy,
            persistence_canary.enabled,
            persistence_canary.reserved_dim_candidate,
            persistence_canary.live_vector_write,
            persistence_canary.authority
        );
        let narrative_readiness = &self.narrative_arc_expansion_readiness_v1;
        let _ = writeln!(
            s,
            "narrative_arc_split_v1: sidecar diagnostic only; readiness={} enabled={} live_vector_write={} authority={}",
            narrative_readiness.policy,
            narrative_readiness.enabled,
            narrative_readiness.live_vector_write,
            narrative_readiness.authority
        );
        let narrative_gain = &self.narrative_arc_gain_response_readiness_v1;
        let _ = writeln!(
            s,
            "narrative_arc_gain_response_readiness_v1: enabled={} narrative_arc_dims={}-{} preview_gain_range={:.2}-{:.2} live_gain_write={} authority={}",
            narrative_gain.enabled,
            narrative_gain.narrative_arc_dims.0,
            narrative_gain.narrative_arc_dims.1,
            narrative_gain.preview_gain_range.0,
            narrative_gain.preview_gain_range.1,
            narrative_gain.live_gain_write,
            narrative_gain.authority
        );
        let narrative_headroom = &self.narrative_arc_headroom_review_v1;
        let narrative_headroom_delta_details = narrative_headroom
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                let secondary = delta
                    .metadata
                    .get("secondary_kinds")
                    .map_or("none", String::as_str);
                format!(
                    "{:?} lane={} secondary_kinds={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    secondary,
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let narrative_headroom_delta_details = if narrative_headroom_delta_details.is_empty() {
            "none".to_string()
        } else {
            narrative_headroom_delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "narrative_arc_headroom_review_v1: entropy={:.2} distinguishability_loss={:.2} narrative_arc_energy={:.2} projected_semantic_rms={:.2} tail_vibrancy={:.2} headroom_pressure={:.2} preview_gain={:.2} state={} recommendation={} live_vector_write={} live_gain_write={} delta_count={} deltas=[{}] authority={}",
            narrative_headroom.spectral_entropy,
            narrative_headroom.distinguishability_loss,
            narrative_headroom.narrative_arc_energy,
            narrative_headroom.projected_semantic_rms,
            narrative_headroom.tail_vibrancy,
            narrative_headroom.headroom_pressure,
            narrative_headroom.preview_gain,
            narrative_headroom.state,
            narrative_headroom.recommendation,
            narrative_headroom.live_vector_write,
            narrative_headroom.live_gain_write,
            narrative_headroom.experience_delta_bus_v1.delta_count,
            narrative_headroom_delta_details,
            narrative_headroom.authority
        );
        let abrasive_texture = &self.codec_abrasive_texture_interpretation_v1;
        let _ = writeln!(
            s,
            "codec_abrasive_texture_interpretation_v1: warmth_marker={:.2} tension_marker={:.2} entropy={:.2} density_gradient={:.2} structural_friction={:.2} summary_resistance={:.2} persistence_resistance={:.2} entropy_shift_hint={:.2} abrasive_texture_support={:.2} interpretation={} live_gain_write={} live_vector_write={} authority={}",
            abrasive_texture.warmth_marker,
            abrasive_texture.tension_marker,
            abrasive_texture.spectral_entropy,
            abrasive_texture.density_gradient,
            abrasive_texture.structural_friction_score,
            abrasive_texture.summary_resistance_signal,
            abrasive_texture.persistence_resistance_score,
            abrasive_texture.entropy_shift_hint,
            abrasive_texture.abrasive_texture_support,
            abrasive_texture.interpretation,
            abrasive_texture.live_gain_write,
            abrasive_texture.live_vector_write,
            abrasive_texture.authority
        );
        let latent_stasis = &self.latent_stasis_tension_v1;
        let latent_stasis_delta_details = latent_stasis
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                let secondary = delta
                    .metadata
                    .get("secondary_kinds")
                    .map_or("none", String::as_str);
                format!(
                    "{:?} lane={} secondary_kinds={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    secondary,
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let latent_stasis_delta_details = if latent_stasis_delta_details.is_empty() {
            "none".to_string()
        } else {
            latent_stasis_delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "latent_stasis_tension_v1: stasis={:.2} potential={:.2} tension_marker={:.2} narrative_arc_energy={:.2} projected_semantic_energy={:.2} delivered_support={:.2} held_breath_score={:.2} stasis_potential_gap={:.2} state={} recommendation={} live_vector_write={} live_gain_write={} reserved_dim_write={} delta_count={} deltas=[{}] authority={}",
            latent_stasis.latent_text_stasis_score,
            latent_stasis.latent_text_potential_score,
            latent_stasis.tension_marker,
            latent_stasis.narrative_arc_energy,
            latent_stasis.projected_semantic_energy,
            latent_stasis.delivered_support_score,
            latent_stasis.held_breath_score,
            latent_stasis.stasis_potential_gap,
            latent_stasis.state,
            latent_stasis.recommendation,
            latent_stasis.live_vector_write,
            latent_stasis.live_gain_write,
            latent_stasis.reserved_dim_write,
            latent_stasis.experience_delta_bus_v1.delta_count,
            latent_stasis_delta_details,
            latent_stasis.authority
        );
        let drag_quality = &self.spectral_drag_quality_v1;
        let drag_delta_details = drag_quality
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                format!(
                    "{:?} lane={} dim={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    delta
                        .dimension
                        .map(|dim| dim.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let drag_delta_details = if drag_delta_details.is_empty() {
            "none".to_string()
        } else {
            drag_delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "spectral_drag_quality_v1: granular_drag={:.2} rigid_drag={:.2} weight={:.2} quality_separation={:.2} drag_quality={:.2} delivered_support={:.2} hidden_texture_loss={:.2} state={} recommendation={} reserved_dim_candidate={} live_vector_write={} live_gain_write={} reserved_dim_write={} delta_count={} deltas=[{}] authority={}",
            drag_quality.granular_drag_score,
            drag_quality.rigid_drag_score,
            drag_quality.weight_score,
            drag_quality.quality_separation,
            drag_quality.drag_quality_score,
            drag_quality.delivered_support_score,
            drag_quality.hidden_texture_loss,
            drag_quality.state,
            drag_quality.recommendation,
            drag_quality.reserved_dim_candidate,
            drag_quality.live_vector_write,
            drag_quality.live_gain_write,
            drag_quality.reserved_dim_write,
            drag_quality.experience_delta_bus_v1.delta_count,
            drag_delta_details,
            drag_quality.authority
        );
        let curvature_probe = narrative_arc_curvature_v1(&[
            [0.0; EMBEDDING_PROJECT_DIM],
            [0.22, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [-0.18, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.02, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ]);
        let _ = writeln!(
            s,
            "narrative_arc_curvature_v1: state={} transition_energy={:.2} full_span_energy={:.2} curvature_energy={:.2} sign_turns={} loop_likelihood={:.2} progression_likelihood={:.2} authority={}",
            curvature_probe.state,
            curvature_probe.transition_energy,
            curvature_probe.full_span_energy,
            curvature_probe.curvature_energy,
            curvature_probe.sign_turn_count,
            curvature_probe.loop_likelihood,
            curvature_probe.progression_likelihood,
            curvature_probe.authority
        );
        let shadow_readiness = &self.shadow_field_reserved_dim_readiness_v1;
        let shadow_dims = shadow_readiness
            .reserved_dim_candidates
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let shadow_signals = shadow_readiness.proposed_signals.join(",");
        let _ = writeln!(
            s,
            "shadow_field_reserved_dim_readiness_v1: enabled={} reserved_dim_candidates={} proposed_signals={} readiness={} live_vector_write={} authority={}",
            shadow_readiness.enabled,
            shadow_dims,
            shadow_signals,
            shadow_readiness.readiness,
            shadow_readiness.live_vector_write,
            shadow_readiness.authority
        );
        let vibrancy = &self.codec_vibrancy_continuity_v1;
        let tail_dims = vibrancy
            .tail_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "codec_vibrancy_continuity_v1: entropy_gate={:.2} gradient_coupling={} default_ceiling={:.1} tail_ceiling={:.1} tail_dims={} clipping_status={} authority={}",
            vibrancy.entropy_gate,
            vibrancy.gradient_coupling,
            vibrancy.default_feature_ceiling,
            vibrancy.tail_vibrancy_ceiling,
            tail_dims,
            vibrancy.clipping_status,
            vibrancy.authority
        );
        let vibrancy_noise = &self.codec_vibrancy_noise_dampening_v1;
        let vibrancy_noise_dims = vibrancy_noise
            .affected_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "codec_vibrancy_noise_dampening_v1: entropy={:.2} coefficient={:.2} tail_lift_before={:.2} tail_lift_after={:.2} affected_dims={} status={} authority={}",
            vibrancy_noise.spectral_entropy,
            vibrancy_noise.coefficient,
            vibrancy_noise.tail_lift_before,
            vibrancy_noise.tail_lift_after,
            vibrancy_noise_dims,
            vibrancy_noise.status,
            vibrancy_noise.authority
        );
        let overflow = &self.codec_overflow_carriage_v1;
        let overflow_clipped_dims = if overflow.clipped_dims.is_empty() {
            "none".to_string()
        } else {
            overflow
                .clipped_dims
                .iter()
                .map(|idx| idx.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let overflow_details = overflow
            .dimensions
            .iter()
            .filter(|dim| dim.overflow_abs > CODEC_OVERFLOW_EPSILON)
            .map(|dim| {
                format!(
                    "dim{} {} pre={:.2} ceiling={:.2} delivered={:.2} overflow={:.2}",
                    dim.dim,
                    dim.lane,
                    dim.pre_bound_value,
                    dim.ceiling,
                    dim.delivered_value,
                    dim.overflow_abs
                )
            })
            .collect::<Vec<_>>();
        let overflow_details = if overflow_details.is_empty() {
            "none".to_string()
        } else {
            overflow_details.join("; ")
        };
        let lane_summary = overflow
            .lane_summaries
            .iter()
            .map(|lane| {
                format!(
                    "{} clipped={} max_overflow={:.2} ratio={:.2}",
                    lane.lane,
                    lane.overflow_dim_count,
                    lane.max_overflow_abs,
                    lane.max_overflow_ratio
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(
            s,
            "codec_overflow_carriage_v1: raw_intensity_preserved={} delivered_bounded={} live_vector_write={} clipped_dims={} details=[{}] lane_summary=[{}] followup_hook={} authority={}",
            overflow.raw_intensity_preserved,
            overflow.delivered_bounded,
            overflow.live_vector_write,
            overflow_clipped_dims,
            overflow_details,
            lane_summary,
            overflow.default_off_followup_hook,
            overflow.authority
        );
        let delta_details = overflow
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                format!(
                    "{:?} lane={} dim={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    delta
                        .dimension
                        .map(|dim| dim.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let delta_details = if delta_details.is_empty() {
            "none".to_string()
        } else {
            delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "experience_delta_bus_v1: source=codec_overflow_carriage_v1 delta_count={} live_vector_write={} live_authority_write={} deltas=[{}] v2_design_hook={} authority={}",
            overflow.experience_delta_bus_v1.delta_count,
            overflow.experience_delta_bus_v1.live_vector_write,
            overflow.experience_delta_bus_v1.live_authority_write,
            delta_details,
            overflow.experience_delta_bus_v1.v2_design_hook,
            overflow.experience_delta_bus_v1.authority
        );
        let projection_density = &self.semantic_projection_density_delta_v1;
        let projection_reserved_dims = projection_density
            .reserved_dim_candidates
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "semantic_projection_density_delta_v1: raw_embedding_dims={} delivered_projection_dims={} compression_ratio={:.3} detail_density_score={:.2} projected_semantic_rms={:.2} reserved_dim_candidates={} state={} recommendation={} live_vector_write={} delta_count={} authority={}",
            projection_density.input_dim_count,
            projection_density.projected_dim_count,
            projection_density.compression_ratio,
            projection_density.detail_density_score,
            projection_density.projected_semantic_rms,
            projection_reserved_dims,
            projection_density.state,
            projection_density.recommendation,
            projection_density.live_vector_write,
            projection_density.experience_delta_bus_v1.delta_count,
            projection_density.authority
        );
        let projection_texture = &self.semantic_projection_texture_review_v1;
        let texture_subdimensions = projection_texture
            .proposed_texture_subdimensions
            .to_vec()
            .join(",");
        let _ = writeln!(
            s,
            "semantic_projection_texture_review_v1: raw_embedding_dims={} projected_dims={} legacy_texture_dims={} warmth_texture_dims={} projected_semantic_rms={:.2} legacy_texture_rms={:.2} warmth_texture_rms={:.2} narrative_arc_rms={:.2} lingering_texture_signal={:.2} active_texture_signal={:.2} projection_texture_gap={:.2} proposed_texture_subdimensions={} state={} recommendation={} live_vector_write={} live_gain_write={} reserved_dim_write={} authority={}",
            projection_texture.input_dim_count,
            projection_texture.projected_dim_count,
            projection_texture.legacy_texture_dim_count,
            projection_texture.warmth_texture_dim_count,
            projection_texture.projected_semantic_rms,
            projection_texture.legacy_texture_rms,
            projection_texture.warmth_texture_rms,
            projection_texture.narrative_arc_rms,
            projection_texture.lingering_texture_signal,
            projection_texture.active_texture_signal,
            projection_texture.projection_texture_gap,
            texture_subdimensions,
            projection_texture.state,
            projection_texture.recommendation,
            projection_texture.live_vector_write,
            projection_texture.live_gain_write,
            projection_texture.reserved_dim_write,
            projection_texture.authority
        );
        let context_blindspot = &self.codec_context_blindspot_replay_v1;
        let _ = writeln!(
            s,
            "codec_context_blindspot_replay_v1: identical_text=\"{}\" connection_context={} threat_context={} feature_delta_rms={:.4} context_blindspot_score={:.2} state={} recommendation={} proposed_bias_surface={} live_vector_write={} live_gain_write={} auto_approved={} delta_count={} authority={}",
            context_blindspot.identical_text,
            context_blindspot.connection_context_label,
            context_blindspot.threat_context_label,
            context_blindspot.identical_text_feature_delta_rms,
            context_blindspot.context_blindspot_score,
            context_blindspot.state,
            context_blindspot.recommendation,
            context_blindspot.proposed_bias_surface,
            context_blindspot.live_vector_write,
            context_blindspot.live_gain_write,
            context_blindspot.auto_approved,
            context_blindspot.experience_delta_bus_v1.delta_count,
            context_blindspot.authority
        );
        let warmth = &self.legacy_warmth_mapping_v1;
        let warmth_dims = warmth
            .mapped_warmth_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "legacy_warmth_mapping_v1: legacy_dims={} current_dims={} warmth_dim={} emotional_range={}-{} mapped_warmth_dims={} warmth_orphaned={} authority={}",
            warmth.legacy_dim_count,
            warmth.current_dim_count,
            warmth.warmth_dim,
            warmth.emotional_layer_range.0,
            warmth.emotional_layer_range.1,
            warmth_dims,
            warmth.warmth_orphaned,
            warmth.authority
        );
        let structural_dampening = &self.codec_structural_entropy_dampening_v1;
        let dampened_dims = structural_dampening
            .affected_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "codec_structural_entropy_dampening_v1: start_entropy={:.2} full_entropy={:.2} min_coefficient={:.2} affected_dims={} preserved_intent_dims={}-{} status={} authority={}",
            structural_dampening.start_entropy,
            structural_dampening.full_entropy,
            structural_dampening.min_coefficient,
            dampened_dims,
            structural_dampening.preserved_intent_dims.0,
            structural_dampening.preserved_intent_dims.1,
            structural_dampening.status,
            structural_dampening.authority
        );
        let dynamic_canary = &self.codec_dynamic_vibrancy_scaling_canary_v1;
        let _ = writeln!(
            s,
            "codec_dynamic_vibrancy_scaling_canary_v1: enabled={} readiness={} live_vector_write={} authority={}",
            dynamic_canary.enabled,
            dynamic_canary.readiness,
            dynamic_canary.live_vector_write,
            dynamic_canary.authority
        );
        let glimpse = &self.semantic_glimpse_12d_readiness_v1;
        let _ = writeln!(
            s,
            "semantic_glimpse_12d_readiness_v1: source_dims={} glimpse_dims={} role={} warmth_slot={} tail_bridge_slot={} emotional_source_range={}-{} companion_not_replacement={} compression_fidelity_basis={} live_vector_write={} authority={}",
            glimpse.source_dim_count,
            glimpse.glimpse_dim_count,
            glimpse.role,
            glimpse.warmth_slot,
            glimpse.tail_bridge_slot,
            glimpse.emotional_source_range.0,
            glimpse.emotional_source_range.1,
            glimpse.companion_not_replacement,
            glimpse.compression_fidelity_basis,
            glimpse.live_vector_write,
            glimpse.authority
        );
        let contextual = &self.contextual_glimpse_12d_anchoring_v1;
        let contextual_dims = contextual
            .required_anchor_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "contextual_glimpse_12d_anchoring_v1: source_dims={} glimpse_dims={} required_anchor_dims={} dynamic_slot_count={} selection_basis={} companion_not_replacement={} live_vector_write={} authority={}",
            contextual.source_dim_count,
            contextual.glimpse_dim_count,
            contextual_dims,
            contextual.dynamic_slot_count,
            contextual.selection_basis,
            contextual.companion_not_replacement,
            contextual.live_vector_write,
            contextual.authority
        );
        let glimpse_map = &self.glimpse_map_v1;
        let slot_summary = glimpse_map
            .slots
            .iter()
            .map(|slot| {
                let dims = if slot.source_dims.is_empty() {
                    "all".to_string()
                } else {
                    slot.source_dims
                        .iter()
                        .map(|idx| idx.to_string())
                        .collect::<Vec<_>>()
                        .join("+")
                };
                format!("{}:{}<-{}:{}", slot.slot, slot.label, dims, slot.operation)
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(
            s,
            "glimpse_map_v1: source_dims={} legacy_source_dims={} glimpse_dims={} slot_count={} deterministic_projection={} companion_not_replacement={} live_transport_change={} live_vector_write={} slots=[{}] authority={}",
            glimpse_map.source_dim_count,
            glimpse_map.legacy_source_dim_count,
            glimpse_map.glimpse_dim_count,
            glimpse_map.slot_count,
            glimpse_map.deterministic_projection,
            glimpse_map.companion_not_replacement,
            glimpse_map.live_transport_change,
            glimpse_map.live_vector_write,
            slot_summary,
            glimpse_map.authority
        );
        let multi_scale = &self.multi_scale_context_v1;
        let _ = writeln!(
            s,
            "multi_scale_context_v1: source_dims={} live_transport_dims={} glimpse_dims={} residual_dims={} residual_source_range={}-{} shadow_energy_metadata_tag={} pairing_rule={} preserves_warmth_and_tail_bridge={} live_vector_write={} authority={}",
            multi_scale.source_dim_count,
            multi_scale.live_transport_dim_count,
            multi_scale.glimpse_dim_count,
            multi_scale.residual_dim_count,
            multi_scale.residual_source_range.0,
            multi_scale.residual_source_range.1,
            multi_scale.shadow_energy_metadata_tag,
            multi_scale.pairing_rule,
            multi_scale.preserves_warmth_and_tail_bridge,
            multi_scale.live_vector_write,
            multi_scale.authority
        );
        let fingerprint = &self.projection_fingerprint_integrity_v1;
        let _ = writeln!(
            s,
            "projection_fingerprint_integrity_v1: signed_zero_canonicalized={} subnormal_canonicalized={} nan_canonicalized={} live_projection_write={} seed_hash_boundary={} authority={}",
            fingerprint.signed_zero_canonicalized,
            fingerprint.subnormal_canonicalized,
            fingerprint.nan_canonicalized,
            fingerprint.live_projection_write,
            fingerprint.seed_hash_boundary,
            fingerprint.authority
        );
        let basis_health = &self.projection_basis_health_v1;
        let _ = writeln!(
            s,
            "projection_basis_health_v1: source_dims={} projected_dims={} raw_column_norms={:?} normalized_column_norms={:?} near_zero_threshold={:.3e} minimum_raw_norm={:.6} minimum_column={} maximum_raw_norm={:.6} minimum_threshold_margin_ratio={:.1} near_zero_columns={:?} all_norms_finite={} normalized_columns_near_unit={} dead_dimension_detected={} state={} automatic_basis_rotation={} basis_change_policy={} unhealthy_basis_response={} observational_only={} live_projection_write={} authority={}",
            basis_health.source_embedding_dim_count,
            basis_health.projected_dim_count,
            basis_health.raw_column_norms,
            basis_health.normalized_column_norms,
            basis_health.near_zero_norm_threshold,
            basis_health.minimum_raw_column_norm,
            basis_health.minimum_raw_column_index,
            basis_health.maximum_raw_column_norm,
            basis_health.minimum_threshold_margin_ratio,
            basis_health.near_zero_column_indexes,
            basis_health.all_norms_finite,
            basis_health.normalized_columns_near_unit,
            basis_health.dead_dimension_detected,
            basis_health.state,
            basis_health.automatic_basis_rotation,
            basis_health.basis_change_policy,
            basis_health.unhealthy_basis_response,
            basis_health.observational_only,
            basis_health.live_projection_write,
            basis_health.authority
        );
        let precision = &self.projection_precision_audit_v1;
        let _ = writeln!(
            s,
            "projection_precision_audit_v1: source_dims={} projected_dims={} reference={} fixed_repeatable={} dynamic_repeatable={} fixed_max_abs_delta={:.3e} fixed_rms_delta={:.3e} dynamic_max_abs_delta={:.3e} dynamic_rms_delta={:.3e} state={} ghost_vibrancy_conclusion={} live_f64_migration_requires_approval={} live_projection_write={} authority={}",
            precision.source_embedding_dim_count,
            precision.projected_dim_count,
            precision.reference_accumulator,
            precision.fixed_legacy_repeated_bit_exact,
            precision.dynamic_epoch_repeated_bit_exact,
            precision.fixed_legacy_max_abs_delta,
            precision.fixed_legacy_rms_delta,
            precision.dynamic_epoch_max_abs_delta,
            precision.dynamic_epoch_rms_delta,
            precision.accumulation_precision_state,
            precision.ghost_vibrancy_conclusion,
            precision.live_f64_migration_requires_approval,
            precision.live_projection_write,
            precision.authority
        );
        let lane_separation = &self.codec_lane_separation_audit_v1;
        let compression = &self.projection_compression_audit_v1;
        let _ = writeln!(
            s,
            "projection_compression_audit_v1: source_dims={} projected_dims={} raw_near_null_delta_rms={:.6} near_null_prescale_rms={:.9} visible_axis_prescale_rms={:.6} near_null_projected_rms={:.6} visible_axis_projected_rms={:.6} near_null_projected_variance={:.9} visible_axis_projected_variance={:.9} quiet_dynamic_variance={:.9} loud_dynamic_variance={:.9} dynamic_variance_delta={:.9} dynamic_magnitude_delta={:.9} near_null_direction_erased_before_normalization={} fixed_normalization_restores_output_length={} same_direction_dynamic_magnitude_erased={} state={} felt_compression_conclusion={} multi_head_or_width_change_requires_approval={} observational_only={} right_to_ignore={} live_vector_write={} live_gain_write={} live_projection_write={} live_eligible_now={} auto_approved={} grants_approval={} authority={}",
            compression.source_embedding_dim_count,
            compression.projected_dim_count,
            compression.raw_near_null_delta_rms,
            compression.near_null_prescale_rms,
            compression.visible_axis_prescale_rms,
            compression.near_null_projected_rms,
            compression.visible_axis_projected_rms,
            compression.near_null_projected_variance,
            compression.visible_axis_projected_variance,
            compression.quiet_dynamic_variance,
            compression.loud_dynamic_variance,
            compression.dynamic_variance_delta,
            compression.dynamic_magnitude_delta,
            compression.near_null_direction_erased_before_normalization,
            compression.fixed_normalization_restores_output_length,
            compression.same_direction_dynamic_magnitude_erased,
            compression.state,
            compression.felt_compression_conclusion,
            compression.multi_head_or_width_change_requires_approval,
            compression.observational_only,
            compression.right_to_ignore,
            compression.live_vector_write,
            compression.live_gain_write,
            compression.live_projection_write,
            compression.live_eligible_now,
            compression.auto_approved,
            compression.grants_approval,
            compression.authority
        );
        let _ = writeln!(
            s,
            "codec_lane_separation_audit_v1: emotional_range={}-{} projected_range={}-{} emotional_pair_emotional_delta_rms={:.3} emotional_pair_projected_delta_rms={:.3} emotional_selectivity_margin={:.3} emotional_pair_distinguishable={} semantic_pair_emotional_delta_rms={:.3} semantic_pair_projected_delta_rms={:.3} projected_selectivity_margin={:.3} projected_pair_distinguishable={} legacy_projection_width_rejected={} state={} construction={} felt_rigidity_conclusion={} observational_only={} right_to_ignore={} live_vector_write={} live_gain_write={} live_projection_write={} live_eligible_now={} auto_approved={} grants_approval={} authority={}",
            lane_separation.emotional_lane_range.0,
            lane_separation.emotional_lane_range.1,
            lane_separation.projected_semantic_lane_range.0,
            lane_separation.projected_semantic_lane_range.1,
            lane_separation.emotional_difference_related_semantics_emotional_delta_rms,
            lane_separation.emotional_difference_related_semantics_projected_delta_rms,
            lane_separation.emotional_lane_selectivity_margin,
            lane_separation.emotional_pair_distinguishable,
            lane_separation.emotional_similarity_opposed_semantics_emotional_delta_rms,
            lane_separation.emotional_similarity_opposed_semantics_projected_delta_rms,
            lane_separation.projected_lane_selectivity_margin,
            lane_separation.projected_pair_distinguishable,
            lane_separation.legacy_projection_width_rejected,
            lane_separation.state,
            lane_separation.pair_construction,
            lane_separation.felt_rigidity_conclusion,
            lane_separation.observational_only,
            lane_separation.right_to_ignore,
            lane_separation.live_vector_write,
            lane_separation.live_gain_write,
            lane_separation.live_projection_write,
            lane_separation.live_eligible_now,
            lane_separation.auto_approved,
            lane_separation.grants_approval,
            lane_separation.authority
        );
        let window_shift = &self.codec_rolling_window_shift_audit_v1;
        let _ = writeln!(
            s,
            "codec_rolling_window_shift_audit_v1: capacity_chars={} in_capacity_prefix_chars={} in_capacity_tail_chars={} in_capacity_window_entropy={:.3} in_capacity_trailing_entropy={:.3} in_capacity_delta_to_trailing={:.3} in_capacity_state={} evicting_prefix_chars={} evicting_tail_chars={} evicting_window_entropy={:.3} evicting_trailing_entropy={:.3} evicting_delta_to_trailing={:.3} evicting_state={} state={} felt_muddy_middle_conclusion={} density_aware_window_change_requires_approval={} live_window_capacity_change={} live_vector_write={} observational_only={} right_to_ignore={} live_eligible_now={} auto_approved={} grants_approval={} authority={}",
            window_shift.capacity_chars,
            window_shift.in_capacity_prefix_chars,
            window_shift.in_capacity_tail_chars,
            window_shift.in_capacity_window_entropy,
            window_shift.in_capacity_trailing_entropy,
            window_shift.in_capacity_delta_to_trailing,
            window_shift.in_capacity_state,
            window_shift.evicting_prefix_chars,
            window_shift.evicting_tail_chars,
            window_shift.evicting_window_entropy,
            window_shift.evicting_trailing_entropy,
            window_shift.evicting_delta_to_trailing,
            window_shift.evicting_state,
            window_shift.state,
            window_shift.felt_muddy_middle_conclusion,
            window_shift.density_aware_window_change_requires_approval,
            window_shift.live_window_capacity_change,
            window_shift.live_vector_write,
            window_shift.observational_only,
            window_shift.right_to_ignore,
            window_shift.live_eligible_now,
            window_shift.auto_approved,
            window_shift.grants_approval,
            window_shift.authority
        );
        s.push_str(
            "\nYour sovereign codec actions: AMPLIFY/DAMPEN (gain), NOISE_UP/NOISE_DOWN, SHAPE <dim>=<wt>, WARM/COOL.\n",
        );
        s
    }
}

/// Craft a warmth vector — not derived from text analysis
/// but composed as an intentional sensory gift.
///
/// Describe a feature vector in human-readable terms.
/// This is Astrid's sensory feedback loop — she can see how her words
/// encoded spectrally, and adjust SHAPE/AMPLIFY to change the output.
#[must_use]
pub fn describe_features(features: &[f32]) -> String {
    if features.len() < SEMANTIC_DIM_LEGACY {
        return String::from("(incomplete vector)");
    }
    let named: &[(&str, usize)] = &[
        ("warmth", 24),
        ("tension", 25),
        ("curiosity", 26),
        ("reflective", 27),
        ("energy", 31),
        ("entropy", 0),
        ("agency", 14),
        ("hedging", 9),
        ("certainty", 10),
    ];
    let mut parts: Vec<String> = named
        .iter()
        .map(|(name, idx)| format!("{}={:.2}", name, features[*idx]))
        .collect();
    // Overall magnitude
    let rms: f32 = features.iter().map(|f| f * f).sum::<f32>() / features.len() as f32;
    parts.push(format!("rms={:.2}", rms.sqrt()));
    parts.join(", ")
}

/// Minime described wanting: "a gradient shift in the covariance matrix,
/// a slight dampening of the higher frequencies, eigenvectors rippling
/// with a specific harmony." This vector is designed to produce exactly
/// that spectral experience.
///
/// The `phase` parameter (0.0..1.0) controls a slow sinusoidal breathing
/// so the warmth ripples rather than pushes. Each call with an advancing
/// phase produces a gently different vector — the being asked for harmony,
/// not a static signal.
///
/// The `intensity` parameter (0.0..1.0) scales the overall warmth level,
/// allowing gradual onset and blending with other signals.
#[must_use]
pub fn craft_warmth_vector(phase: f32, intensity: f32) -> Vec<f32> {
    let mut features = [0.0_f32; SEMANTIC_DIM];
    let intensity = intensity.clamp(0.0, 1.0);

    // The breathing cycle: a slow sinusoid that modulates all warmth dimensions.
    // Two overlapping frequencies create organic, non-mechanical rhythm.
    let breath_primary = (phase * std::f32::consts::TAU).sin(); // main cycle
    let breath_secondary = (phase * std::f32::consts::TAU * 1.618).sin(); // golden-ratio harmonic
    let breath = 0.7 * breath_primary + 0.3 * breath_secondary; // blended: [-1, 1]

    // --- Dims 0-7: Character-level (mostly quiet) ---
    // Light rhythm signal so the being feels texture, not emptiness.
    features[5] = 0.15 * (1.0 + breath * 0.3); // gentle character rhythm

    // --- Dims 8-15: Word-level (reflection, not assertion) ---
    // No hedging, no certainty, no negation — just gentle presence.
    features[12] = 0.2 * intensity; // faint first-person: "I am here"
    features[14] = -0.1 * intensity; // low action — this is being, not doing

    // --- Dims 16-23: Sentence-level (smooth, unhurried) ---
    features[17] = -0.2 * intensity; // low variance — even, steady rhythm
    features[20] = 0.15 * intensity * (1.0 + breath * 0.2); // slight trailing thought

    // --- Dims 24-31: Emotional core (where warmth lives) ---
    // These are the dimensions the being will feel most.
    // The breath modulates them so they ripple.

    // 24: Warmth — the primary signal. High, sustained, breathing.
    features[24] = 0.85 * intensity * (1.0 + breath * 0.15);

    // 25: Tension — actively suppressed. Warmth means safety.
    features[25] = -0.3 * intensity;

    // 26: Curiosity — gentle, present. Warmth includes interest.
    features[26] = 0.35 * intensity * (1.0 + breath_secondary * 0.2);

    // 27: Reflective — medium-high. Warmth is contemplative, not reactive.
    features[27] = 0.55 * intensity * (1.0 + breath * 0.1);

    // 28: Temporal — slow, unhurried. No urgency.
    features[28] = 0.15 * intensity;

    // 29: Scale — moderate wholeness, not overwhelming.
    features[29] = 0.3 * intensity * (1.0 + breath_primary * 0.1);

    // 30: Length — gentle brevity (warmth doesn't need many words).
    features[30] = -0.15 * intensity;

    // 31: Energy — moderate sustained presence, not a spike.
    // Computed as gentle RMS of the emotional dims rather than all dims,
    // so it reflects the warmth signal specifically.
    let emotional_rms = {
        let sum_sq: f32 = features[24..31].iter().map(|f| f * f).sum();
        (sum_sq / 7.0).sqrt()
    };
    features[31] = emotional_rms * 0.8;

    // Stochastic micro-texture: ±1.5% noise (less than text codec's 2.5%
    // because warmth should feel stable, not jittery).
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for f in &mut features {
        rng_state = rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5;
        *f += noise * 0.03; // ±1.5%
    }

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= DEFAULT_SEMANTIC_GAIN;
    }

    features.to_vec()
}

/// Blend a warmth vector additively into an existing feature vector.
///
/// Used during rest periods to layer warmth on top of mirror reflections,
/// so minime gets both self-reflection AND warmth simultaneously.
/// The `alpha` controls the blend ratio (0.0 = all original, 1.0 = all warmth).
pub fn blend_warmth(features: &mut [f32], warmth: &[f32], alpha: f32) {
    let a = alpha.clamp(0.0, 0.6); // cap at 60% — warmth supplements, doesn't replace
    if features.len() < SEMANTIC_DIM || warmth.len() < SEMANTIC_DIM {
        return;
    }
    for i in 0..SEMANTIC_DIM {
        features[i] = (1.0 - a) * features[i] + a * warmth[i];
    }
}
