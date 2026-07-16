#[cfg(test)]
fn non_instrumental_presence_readiness_v1() -> NonInstrumentalPresenceReadinessV1 {
    NonInstrumentalPresenceReadinessV1 {
        mode: "contemplate",
        non_goal_state_available: true,
        text_generation_suppressed: true,
        codec_send_suppressed: true,
        journal_write_suppressed: true,
        warmth_and_state_tracking_continue: true,
        authority: "read_only_presence_readiness_not_scheduler_prompt_or_control_change",
    }
}

fn classify_witness_relational_friction_v1(
    chamber_state: Option<&Value>,
) -> WitnessRelationalFrictionV1 {
    let Some(state) = chamber_state else {
        return WitnessRelationalFrictionV1 {
            classification: "insufficient_context",
            weather: None,
            gravity_participant: None,
            gravity_role: None,
            non_categorical_resonance: None,
            fluidity_index: None,
            gradient_texture: None,
            temporal_persistence: "unknown",
            evidence: vec!["relational_metrics_absent".to_string()],
            schema_diagnostics: vec!["missing_key=chamber_state".to_string()],
            authority: "interpretive_context_not_instruction_or_control",
        };
    };
    let Some(metrics_value) = state.get("relational_metrics") else {
        return WitnessRelationalFrictionV1 {
            classification: "insufficient_context",
            weather: None,
            gravity_participant: None,
            gravity_role: None,
            non_categorical_resonance: None,
            fluidity_index: None,
            gradient_texture: None,
            temporal_persistence: "unknown",
            evidence: vec!["relational_metrics_absent".to_string()],
            schema_diagnostics: vec!["missing_key=relational_metrics".to_string()],
            authority: "interpretive_context_not_instruction_or_control",
        };
    };
    let Some(metrics) = metrics_value.as_object() else {
        return WitnessRelationalFrictionV1 {
            classification: "insufficient_context",
            weather: None,
            gravity_participant: None,
            gravity_role: None,
            non_categorical_resonance: None,
            fluidity_index: None,
            gradient_texture: None,
            temporal_persistence: "unknown",
            evidence: vec!["relational_metrics_absent".to_string()],
            schema_diagnostics: vec![format!(
                "schema_drift=relational_metrics:expected_object_found_{}",
                witness_value_kind(metrics_value)
            )],
            authority: "interpretive_context_not_instruction_or_control",
        };
    };
    let mut schema_diagnostics = Vec::new();
    let weather = extract_witness_weather(metrics, &mut schema_diagnostics);
    let (gravity_participant, gravity_role) =
        extract_witness_gravity(metrics, &mut schema_diagnostics);
    let density_gradient = witness_metric_f32(metrics, &["density_gradient", "densityGradient"]);
    let fluidity_index = witness_metric_f32(metrics, &["fluidity_index", "fluidityIndex"])
        .or_else(|| density_gradient.map(witness_fluidity_from_density_gradient))
        .map(|value| value.clamp(0.0, 1.0));
    let gradient_texture = metrics
        .get("gradient_texture")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            density_gradient
                .map(witness_gradient_texture_label)
                .map(str::to_string)
        });

    let mut evidence = Vec::new();
    if let Some(weather) = weather.as_deref() {
        evidence.push(format!("weather={weather}"));
    }
    if let Some(participant) = gravity_participant.as_deref() {
        evidence.push(format!("gravity_participant={participant}"));
    }
    if let Some(role) = gravity_role.as_deref() {
        evidence.push(format!("gravity_role={role}"));
    }
    if let Some(prompt_mirror) = metrics.get("prompt_mirror").and_then(Value::as_str)
        && prompt_mirror.contains("carry-forward residue")
    {
        evidence.push("carry_forward_residue_present".to_string());
    }
    if let Some(gradient) = density_gradient {
        evidence.push(format!("density_gradient={gradient:.2}"));
    }
    if let Some(texture) = gradient_texture.as_deref() {
        evidence.push(format!("gradient_texture={texture}"));
    }
    if let Some(fluidity) = fluidity_index {
        evidence.push(format!("fluidity_index={fluidity:.2}"));
    }
    let explicit_non_categorical = metrics
        .get("non_categorical_resonance")
        .and_then(Value::as_str)
        .map(str::to_string);
    let metric_key_matches_uncategorized_tension = |key: &str| {
        let lower = key.to_ascii_lowercase();
        lower.contains("friction")
            || lower.contains("tension")
            || lower.contains("pressure")
            || lower.contains("resonance")
            || lower.contains("lambda")
            || lower.contains("variance")
            || lower.contains("density")
            || lower.contains("distinguishability")
    };
    let mut non_categorical_resonance = explicit_non_categorical;
    if weather.is_none() && gravity_participant.is_none() {
        let mut metric_keys = metrics.keys().map(String::as_str).collect::<Vec<_>>();
        metric_keys.sort_unstable();
        evidence.push("relational_metrics_present_without_weather_or_gravity".to_string());
        if !metric_keys.is_empty() {
            evidence.push(format!(
                "relational_metric_keys={}",
                metric_keys
                    .iter()
                    .copied()
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if non_categorical_resonance.is_none()
            && metric_keys
                .iter()
                .any(|key| metric_key_matches_uncategorized_tension(key))
        {
            non_categorical_resonance =
                Some("unclassified_tension_without_weather_or_gravity".to_string());
        } else if non_categorical_resonance.is_none() && !metric_keys.is_empty() {
            non_categorical_resonance =
                Some("relational_metrics_present_without_categorical_bucket".to_string());
        }
    }
    if let Some(non_categorical) = non_categorical_resonance.as_deref() {
        evidence.push(format!("non_categorical_resonance={non_categorical}"));
    }
    let temporal_persistence = classify_witness_temporal_persistence(metrics, &mut evidence);

    let weather_lower = weather.as_deref().unwrap_or_default().to_ascii_lowercase();
    let role_lower = gravity_role
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let participant_lower = gravity_participant
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let classification = if weather.is_none() && gravity_participant.is_none() {
        "insufficient_context"
    } else if matches!(
        weather_lower.as_str(),
        "mixed" | "oscillating" | "divergent"
    ) {
        "shared_weather_shift"
    } else if participant_lower == "astrid" && matches!(role_lower.as_str(), "unsettled" | "mover")
    {
        "internal_instability"
    } else if !participant_lower.is_empty()
        && participant_lower != "astrid"
        && matches!(role_lower.as_str(), "unsettled" | "mover" | "pulled")
    {
        "relational_instability"
    } else {
        "shared_weather_shift"
    };

    WitnessRelationalFrictionV1 {
        classification,
        weather,
        gravity_participant,
        gravity_role,
        non_categorical_resonance,
        fluidity_index,
        gradient_texture,
        temporal_persistence,
        evidence,
        schema_diagnostics,
        authority: "interpretive_context_not_instruction_or_control",
    }
}

fn classify_witness_temporal_persistence(
    metrics: &serde_json::Map<String, Value>,
    evidence: &mut Vec<String>,
) -> &'static str {
    if let Some(raw) = metrics
        .get("temporal_persistence")
        .and_then(Value::as_str)
        .or_else(|| metrics.get("persistence_state").and_then(Value::as_str))
    {
        let lower = raw.to_ascii_lowercase();
        evidence.push(format!("temporal_persistence_source={raw}"));
        if lower.contains("sediment") || lower.contains("persistent") || lower.contains("durable") {
            return "sedimented";
        }
        if lower.contains("settling") || lower.contains("linger") || lower.contains("carry") {
            return "settling";
        }
        if lower.contains("fleeting") || lower.contains("flicker") || lower.contains("brief") {
            return "fleeting";
        }
    }
    let score = metrics
        .get("persistence_score")
        .and_then(Value::as_f64)
        .or_else(|| {
            metrics
                .get("temporal_persistence_score")
                .and_then(Value::as_f64)
        });
    if let Some(score) = score {
        evidence.push(format!("temporal_persistence_score={score:.2}"));
        if score >= 0.66 {
            return "sedimented";
        }
        if score >= 0.33 {
            return "settling";
        }
        return "fleeting";
    }
    let duration_ms = metrics
        .get("duration_ms")
        .and_then(Value::as_u64)
        .or_else(|| metrics.get("age_ms").and_then(Value::as_u64));
    if let Some(duration_ms) = duration_ms {
        evidence.push(format!("temporal_duration_ms={duration_ms}"));
        if duration_ms >= 300_000 {
            return "sedimented";
        }
        if duration_ms >= 30_000 {
            return "settling";
        }
        return "fleeting";
    }
    if metrics
        .get("prompt_mirror")
        .and_then(Value::as_str)
        .is_some_and(|prompt| prompt.contains("carry-forward residue"))
    {
        evidence.push("temporal_persistence_from_carry_forward_residue".to_string());
        return "sedimented";
    }
    "unknown"
}

fn mirror_resonance_drift_guard_v1(
    chamber_state: Option<&Value>,
    relational_friction: &WitnessRelationalFrictionV1,
) -> MirrorResonanceDriftGuardV1 {
    let metrics = chamber_state
        .and_then(|state| state.get("relational_metrics"))
        .and_then(Value::as_object);
    let prompt_mirror = metrics
        .and_then(|value| value.get("prompt_mirror"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let non_categorical = relational_friction
        .non_categorical_resonance
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let peer_language_feedback_present = prompt_mirror.contains("minime")
        || prompt_mirror.contains("peer")
        || prompt_mirror.contains("carry-forward residue");
    let abstract_pressure_descriptor_present = non_categorical.contains("pressure")
        || non_categorical.contains("tension")
        || prompt_mirror.contains("pressure")
        || prompt_mirror.contains("tension");
    let mut evidence = Vec::new();
    if peer_language_feedback_present {
        evidence.push("peer_language_feedback_present".to_string());
    }
    if abstract_pressure_descriptor_present {
        evidence.push("abstract_pressure_descriptor_present".to_string());
    }
    if relational_friction.classification != "insufficient_context" {
        evidence.push(format!(
            "relational_classification={}",
            relational_friction.classification
        ));
    }
    if relational_friction.temporal_persistence != "unknown" {
        evidence.push(format!(
            "temporal_persistence={}",
            relational_friction.temporal_persistence
        ));
    }
    let self_other_blur_risk = if peer_language_feedback_present
        && relational_friction.temporal_persistence == "sedimented"
    {
        "elevated_echo_chamber_risk"
    } else if peer_language_feedback_present || abstract_pressure_descriptor_present {
        "watch_for_hallucinated_resonance"
    } else {
        "low"
    };
    let recommended_posture = if self_other_blur_risk == "low" {
        "ordinary_witness_context"
    } else {
        "name_self_other_boundary_keep_witness_read_only"
    };

    MirrorResonanceDriftGuardV1 {
        policy: "mirror_resonance_drift_guard_v1",
        self_other_blur_risk,
        abstract_pressure_descriptor_present,
        peer_language_feedback_present,
        temporal_persistence: relational_friction.temporal_persistence,
        recommended_posture,
        evidence,
        authority: "diagnostic_context_not_mirror_action_or_control",
    }
}

fn mirror_fidelity_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| {
        !character.is_alphanumeric() && character != '\'' && character != '-'
    })
    .filter(|token| !token.is_empty())
    .map(str::to_lowercase)
    .collect()
}

fn mirror_fidelity_hash_prefix(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("{digest:x}").chars().take(16).collect()
}

fn mirror_source_fidelity_v1(
    source_text: &str,
    rendered_text: &str,
    source_ref: &str,
    semantic_chunk_sent: bool,
    codec_signature_dims: Option<usize>,
    codec_signature_rms: Option<f32>,
) -> MirrorSourceFidelityV1 {
    let source_tokens = mirror_fidelity_tokens(source_text);
    let rendered_tokens = mirror_fidelity_tokens(rendered_text);
    let source_distinct = source_tokens
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let rendered_distinct = rendered_tokens
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let preserved_distinct_token_count = source_distinct.intersection(&rendered_distinct).count();
    let lexical_recall = if source_distinct.is_empty() {
        if rendered_distinct.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        preserved_distinct_token_count as f32 / source_distinct.len() as f32
    };
    let edge_width = 4;
    let leading_edge_preserved = source_tokens
        .iter()
        .take(edge_width)
        .eq(rendered_tokens.iter().take(edge_width));
    let trailing_edge_preserved = source_tokens
        .iter()
        .rev()
        .take(edge_width)
        .eq(rendered_tokens.iter().rev().take(edge_width));
    let normalized_source = source_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_rendered = rendered_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let exact_text_match = source_text == rendered_text;
    let normalized_text_match = normalized_source == normalized_rendered;
    let fidelity_state = if exact_text_match {
        "exact_source_render"
    } else if normalized_text_match {
        "whitespace_canonicalized_source_render"
    } else if lexical_recall >= 0.95 && leading_edge_preserved && trailing_edge_preserved {
        "high_fidelity_source_render"
    } else if lexical_recall >= 0.75 && (leading_edge_preserved || trailing_edge_preserved) {
        "partial_fidelity_review"
    } else {
        "low_fidelity_review"
    };
    let codec_observation_state = if semantic_chunk_sent && codec_signature_dims == Some(48) {
        "encoded_48d_signature_observed"
    } else if semantic_chunk_sent && codec_signature_dims.is_some() {
        "encoded_nonstandard_signature_observed"
    } else if semantic_chunk_sent {
        "semantic_chunk_sent_signature_unavailable"
    } else {
        "semantic_send_not_observed"
    };

    MirrorSourceFidelityV1 {
        policy: "mirror_source_fidelity_v1",
        source_ref: truncate_str(source_ref, 120).to_string(),
        source_sha256_prefix: mirror_fidelity_hash_prefix(source_text),
        rendered_sha256_prefix: mirror_fidelity_hash_prefix(rendered_text),
        exact_text_match,
        normalized_text_match,
        source_word_count: source_tokens.len(),
        rendered_word_count: rendered_tokens.len(),
        source_distinct_token_count: source_distinct.len(),
        preserved_distinct_token_count,
        lexical_recall,
        leading_edge_preserved,
        trailing_edge_preserved,
        semantic_chunk_sent,
        codec_signature_dims,
        codec_signature_rms: codec_signature_rms.filter(|value| value.is_finite()),
        codec_observation_state,
        fidelity_state,
        right_to_ignore: true,
        control_applied: false,
        behavior_changed: false,
        authority: "read_only_source_render_and_codec_receipt_not_mirror_choice_gain_transport_or_control",
    }
}

fn classify_witness_semantic_density_mapping_v1(
    telemetry: &crate::types::SpectralTelemetry,
    relational_friction: &WitnessRelationalFrictionV1,
    correspondence_stall_ambiguous: bool,
) -> WitnessSemanticDensityMappingV1 {
    let spectral_entropy = normalized_eigen_entropy(&telemetry.eigenvalues);
    let density_gradient = eigen_density_gradient(&telemetry.eigenvalues);
    let resonance_density = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| density.density.clamp(0.0, 1.0));
    let pressure_risk = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| density.pressure_risk.clamp(0.0, 1.0))
        .or_else(|| {
            telemetry
                .pressure_source_v1
                .as_ref()
                .map(|source| source.pressure_score.clamp(0.0, 1.0))
        });
    let mode_packing = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| density.components.mode_packing.clamp(0.0, 1.0))
        .or_else(|| {
            telemetry
                .pressure_source_v1
                .as_ref()
                .map(|source| source.components.mode_packing.clamp(0.0, 1.0))
        });
    let semantic_friction = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|source| source.components.semantic_friction.clamp(0.0, 1.0));
    let fluctuation = telemetry.inhabitable_fluctuation_v1.as_ref();
    let fluctuation_quality = fluctuation.map(|value| value.quality.clone());
    let foothold_stability = fluctuation.map(|value| value.foothold_stability.clamp(0.0, 1.0));
    let fluctuation_score = fluctuation.map(|value| value.fluctuation_score.clamp(0.0, 1.0));
    let density_texture = resonance_density.map(witness_resonance_density_texture_label);
    let pressure_texture = pressure_risk.map(witness_pressure_texture_label);
    let gradient_texture = density_gradient.map(witness_gradient_texture_label);
    let fluidity_index = witness_semantic_fluidity_index(
        density_gradient,
        pressure_risk,
        mode_packing,
        foothold_stability,
    );
    let settled_habitable = fluctuation_quality
        .as_deref()
        .is_some_and(|quality| quality.contains("settled") || quality.contains("habitable"));
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.85);
    let low_pressure = pressure_risk.is_none_or(|value| value < 0.30);
    let low_gradient = density_gradient.is_none_or(|value| value <= 0.20);
    let stable_foothold = foothold_stability.is_some_and(|value| value >= 0.62);
    let active_reorg = fluctuation_score.is_some_and(|value| value >= 0.21)
        || fluctuation
            .map(|value| value.rearrangement_intensity >= 0.21)
            .unwrap_or(false);
    let friction_high = mode_packing.is_some_and(|value| value >= 0.45)
        || semantic_friction.is_some_and(|value| value >= 0.35)
        || pressure_risk.is_some_and(|value| value >= 0.35);

    let classification = if spectral_entropy.is_none() && fluctuation.is_none() {
        "insufficient_context"
    } else if settled_habitable && high_entropy && low_pressure {
        "settled_high_entropy_complexity"
    } else if settled_habitable && low_gradient && stable_foothold {
        "silt_weighted_habitable"
    } else if active_reorg && !friction_high {
        "luminous_reorganization"
    } else if friction_high {
        "overpacked_friction"
    } else {
        "insufficient_context"
    };

    let mut evidence = Vec::new();
    if let Some(entropy) = spectral_entropy {
        evidence.push(format!("spectral_entropy={entropy:.2}"));
    }
    if let Some(density) = resonance_density {
        evidence.push(format!("resonance_density={density:.2}"));
    }
    if let Some(texture) = density_texture {
        evidence.push(format!("density_texture={texture}"));
    }
    if let Some(pressure) = pressure_risk {
        evidence.push(format!("pressure_risk={pressure:.2}"));
    }
    if let Some(texture) = pressure_texture {
        evidence.push(format!("pressure_texture={texture}"));
    }
    if let Some(gradient) = density_gradient {
        evidence.push(format!("density_gradient={gradient:.2}"));
    }
    if let Some(texture) = gradient_texture {
        evidence.push(format!("gradient_texture={texture}"));
    }
    if let Some(fluidity) = fluidity_index {
        evidence.push(format!("fluidity_index={fluidity:.2}"));
    }
    if let Some(packing) = mode_packing {
        evidence.push(format!("mode_packing={packing:.2}"));
    }
    if let Some(friction) = semantic_friction {
        evidence.push(format!("semantic_friction={friction:.2}"));
    }
    if let Some(quality) = fluctuation_quality.as_deref() {
        evidence.push(format!("fluctuation_quality={quality}"));
    }
    if let Some(foothold) = foothold_stability {
        evidence.push(format!("foothold_stability={foothold:.2}"));
    }
    if correspondence_stall_ambiguous {
        evidence.push(
            "reply_linked_requires_peer_ack_or_trace; silence_cannot_be_treated_as_absence"
                .to_string(),
        );
    }
    if relational_friction.classification != "insufficient_context" {
        evidence.push(format!(
            "witness_relational_friction={}",
            relational_friction.classification
        ));
    }
    if let Some(non_categorical) = relational_friction.non_categorical_resonance.as_deref() {
        evidence.push(format!(
            "witness_non_categorical_resonance={non_categorical}"
        ));
    }

    WitnessSemanticDensityMappingV1 {
        classification,
        spectral_entropy,
        resonance_density,
        pressure_risk,
        density_gradient,
        fluidity_index,
        density_texture,
        pressure_texture,
        gradient_texture,
        mode_packing,
        semantic_friction,
        fluctuation_quality,
        foothold_stability,
        correspondence_stall_ambiguous,
        evidence,
        authority: "interpretive_context_not_instruction_or_control",
    }
}

fn witness_mean_known_score(values: &[Option<f32>]) -> Option<f32> {
    let known = values
        .iter()
        .flatten()
        .copied()
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
        .collect::<Vec<_>>();
    if known.is_empty() {
        None
    } else {
        Some((known.iter().sum::<f32>() / known.len() as f32).clamp(0.0, 1.0))
    }
}

fn witness_friction_provenance_v1(
    telemetry: &crate::types::SpectralTelemetry,
    semantic_mapping: &WitnessSemanticDensityMappingV1,
    relational_friction: &WitnessRelationalFrictionV1,
    witness_frame: Option<&crate::witness::WitnessFrameV1>,
) -> WitnessFrictionProvenanceV1 {
    let density_components = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|density| &density.components);
    let viscosity_vector = density_components.map(|components| &components.viscosity_vector);
    let viscosity_vector_available = viscosity_vector.is_some_and(|vector| {
        vector.structural_drag_coefficient.abs() > f32::EPSILON
            || vector.effective_mobility.abs() > f32::EPSILON
            || vector.density.abs() > f32::EPSILON
            || vector.persistence.abs() > f32::EPSILON
    });

    let viscosity = density_components.and_then(|components| {
        (components.viscosity_index.abs() > f32::EPSILON || viscosity_vector_available)
            .then_some(components.viscosity_index.clamp(0.0, 1.0))
    });
    let structural_drag = viscosity_vector
        .filter(|_| viscosity_vector_available)
        .map(|vector| vector.structural_drag_coefficient.clamp(0.0, 1.0));
    let mobility_resistance = viscosity_vector
        .filter(|_| viscosity_vector_available)
        .map(|vector| (1.0 - vector.effective_mobility.clamp(0.0, 1.0)).clamp(0.0, 1.0));
    let porosity_resistance = density_components
        .and_then(|components| components.porosity_gradient)
        .map(|porosity| (1.0 - porosity.clamp(0.0, 1.0)).clamp(0.0, 1.0));
    let reservoir_medium_score = witness_mean_known_score(&[
        viscosity,
        structural_drag,
        mobility_resistance,
        semantic_mapping.mode_packing,
        porosity_resistance,
    ]);

    let pressure_components = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|source| &source.components);
    let semantic_trickle_gap = pressure_components
        .map(|components| (1.0 - components.semantic_trickle.clamp(0.0, 1.0)).clamp(0.0, 1.0));
    let distinguishability_loss =
        pressure_components.map(|components| components.distinguishability_loss.clamp(0.0, 1.0));
    let internal_relational_instability =
        (relational_friction.classification == "internal_instability").then_some(0.65);
    let semantic_processing_score = witness_mean_known_score(&[
        semantic_mapping.semantic_friction,
        semantic_trickle_gap,
        distinguishability_loss,
        internal_relational_instability,
    ]);

    let relational_context_available = relational_friction.classification != "insufficient_context";
    let relational_classification_score = match relational_friction.classification {
        "relational_instability" => Some(0.75),
        "shared_weather_shift" => Some(0.45),
        _ => None,
    };
    let relational_fluidity_resistance = relational_friction
        .fluidity_index
        .map(|fluidity| (1.0 - fluidity.clamp(0.0, 1.0)).clamp(0.0, 1.0));
    let relational_persistence = if relational_context_available {
        match relational_friction.temporal_persistence {
            "sedimented" => Some(0.65),
            "settling" => Some(0.40),
            "fleeting" => Some(0.15),
            _ => None,
        }
    } else {
        None
    };
    let correspondence_stall = semantic_mapping
        .correspondence_stall_ambiguous
        .then_some(0.60);
    let non_categorical_tension = relational_friction
        .non_categorical_resonance
        .as_ref()
        .map(|_| 0.40);
    let relational_transport_score = witness_mean_known_score(&[
        relational_classification_score,
        relational_fluidity_resistance,
        relational_persistence,
        correspondence_stall,
        non_categorical_tension,
    ]);

    let mut ranked = Vec::new();
    if let Some(score) = reservoir_medium_score {
        ranked.push(("reservoir_medium", score));
    }
    if let Some(score) = semantic_processing_score {
        ranked.push(("semantic_processing", score));
    }
    if let Some(score) = relational_transport_score {
        ranked.push(("relational_transport", score));
    }
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));

    let (dominant_origin, cross_layer_state) = match ranked.as_slice() {
        [] => ("insufficient_context", "insufficient_context"),
        [(origin, _)] => (*origin, "single_layer_evidence"),
        ranked => {
            let top = ranked[0];
            let second = ranked[1];
            let margin = top.1 - second.1;
            if margin >= 0.15 {
                (top.0, "dominant_origin_with_secondary_context")
            } else if ranked.len() == 3 && top.1 - ranked[2].1 <= 0.15 {
                ("cross_layer_coupled", "cross_layer_convergence")
            } else {
                let names = [top.0, second.0];
                let coupled = if names.contains(&"reservoir_medium")
                    && names.contains(&"semantic_processing")
                {
                    "coupled_reservoir_semantic"
                } else if names.contains(&"reservoir_medium")
                    && names.contains(&"relational_transport")
                {
                    "coupled_reservoir_relational"
                } else {
                    "coupled_semantic_relational"
                };
                (coupled, "coupled_origin_evidence")
            }
        },
    };
    let coverage = ranked.len() as f32 / 3.0;
    let strongest = ranked.first().map_or(0.0, |(_, score)| *score);
    let attribution_confidence = (coverage * 0.70 + strongest * 0.30).clamp(0.0, 1.0);

    let mut evidence = Vec::new();
    if let Some(value) = viscosity {
        evidence.push(format!("viscosity_index={value:.2}"));
    }
    if let Some(value) = structural_drag {
        evidence.push(format!("structural_drag_coefficient={value:.2}"));
    }
    if let Some(value) = mobility_resistance {
        evidence.push(format!("mobility_resistance={value:.2}"));
    }
    if let Some(value) = semantic_mapping.mode_packing {
        evidence.push(format!("mode_packing={value:.2}"));
    }
    if let Some(value) = semantic_mapping.semantic_friction {
        evidence.push(format!("semantic_friction={value:.2}"));
    }
    if let Some(value) = semantic_trickle_gap {
        evidence.push(format!("semantic_trickle_gap={value:.2}"));
    }
    if let Some(value) = distinguishability_loss {
        evidence.push(format!("distinguishability_loss={value:.2}"));
    }
    if relational_context_available {
        evidence.push(format!(
            "relational_classification={}",
            relational_friction.classification
        ));
    }
    if semantic_mapping.correspondence_stall_ambiguous {
        evidence.push("correspondence_stall_ambiguous".to_string());
    }

    WitnessFrictionProvenanceV1 {
        policy: "witness_friction_provenance_v1",
        observed_parent_id: witness_frame.map(|frame| frame.observation().source_id().to_string()),
        derived_parent_id: witness_frame.map(|frame| frame.evidence().source_id().to_string()),
        interpreted_parent_id: witness_frame
            .map(|frame| frame.interpretation().source_id().to_string()),
        dominant_origin,
        reservoir_medium_score,
        semantic_processing_score,
        relational_transport_score,
        attribution_confidence,
        cross_layer_state,
        proprioceptive_feedback_available: !ranked.is_empty(),
        witness_posture: "descriptive_non_directive_proprioception",
        evidence,
        control_write: false,
        authority: "read_only_friction_attribution_not_pressure_fill_admission_transport_or_control",
    }
}
