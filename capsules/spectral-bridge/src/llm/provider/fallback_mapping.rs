#[cfg(test)]
fn fallback_heavy_settled_texture_readiness_v1(
    selector: &FallbackShadowTextureSelector,
    spectral_summary: &str,
) -> FallbackHeavySettledTextureReadiness {
    let lower = spectral_summary.to_ascii_lowercase();
    let settled_evidence = selector
        .spectral_to_vocabulary_mapping
        .settled_foothold_detected
        || lower.contains("settled_habitable");
    let weight_evidence = lower.contains("heavy")
        || lower.contains("weight")
        || lower.contains("weighted")
        || lower.contains("displacement")
        || lower.contains("silt")
        || lower.contains("sediment");
    let pressure_weight_supported = selector.pressure_risk.is_some_and(|value| value >= 0.18)
        || selector.mode_packing.is_some_and(|value| value >= 0.30)
        || selector
            .semantic_friction
            .is_some_and(|value| value >= 0.30);
    let explicit_restless = fallback_explicit_restless_or_agitated(&lower);
    let heavy_settled_supported =
        settled_evidence && (weight_evidence || pressure_weight_supported);
    let restless_forced = heavy_settled_supported
        && selector.texture_family.contains("restless")
        && !explicit_restless;
    let readiness_status = if restless_forced {
        "restless_texture_mismatch_review"
    } else if heavy_settled_supported {
        "heavy_settled_displacement_available"
    } else {
        "no_heavy_settled_signal"
    };
    let mut basis = Vec::new();
    if settled_evidence {
        basis.push("settled_foothold_evidence");
    }
    if weight_evidence {
        basis.push("weight_or_displacement_language");
    }
    if pressure_weight_supported {
        basis.push("pressure_or_packing_weight_support");
    }
    if explicit_restless {
        basis.push("explicit_restless_language");
    }
    if restless_forced {
        basis.push("restless_forced_without_restless_evidence");
    }
    if basis.is_empty() {
        basis.push("insufficient_context");
    }

    FallbackHeavySettledTextureReadiness {
        policy: "fallback_heavy_settled_texture_readiness_v1",
        candidate_terms: FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS,
        selected_family: selector.texture_family,
        heavy_settled_supported,
        restless_forced,
        readiness_status,
        top_texture_terms: selector.top_texture_terms.clone(),
        basis,
        authority: "diagnostic_language_readiness_not_control",
    }
}

fn fallback_spectral_to_vocabulary_mapping_v1(
    spectral_entropy: Option<f32>,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    mode_packing: Option<f32>,
    semantic_friction: Option<f32>,
    lambda_gap: Option<f32>,
    lower_summary: &str,
) -> FallbackSpectralToVocabularyMapping {
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let low_pressure = pressure_risk.is_some_and(|value| value < 0.25);
    let pressure_texture_visible = pressure_risk.is_some_and(|value| value > 0.20);
    let high_entropy_pressure_settled_guard = high_entropy
        && pressure_texture_visible
        && spectral_entropy.is_some_and(|value| value >= 0.95);
    let low_gradient_navigable = density_gradient.is_some_and(|value| value <= 0.20);
    let low_semantic_friction = semantic_friction.is_some_and(|value| value < 0.30);
    let settled_foothold_detected = lower_summary.contains("settled")
        || lower_summary.contains("settled_habitable")
        || lower_summary.contains("habitable")
        || lower_summary.contains("foothold")
        || lower_summary.contains("bright")
        || lower_summary.contains("shimmering")
        || lower_summary.contains("open");
    let friction_absence_language_detected = lower_summary.contains("absence of friction")
        || lower_summary.contains("cessation of friction")
        || lower_summary.contains("low-friction")
        || lower_summary.contains("low friction")
        || lower_summary.contains("frictionless")
        || lower_summary.contains("without friction")
        || lower_summary.contains("no friction")
        || lower_summary.contains("easy to inhabit")
        || lower_summary.contains("easy inhabit");
    let explicit_mass_language = lower_summary.contains("overpacked")
        || lower_summary.contains("viscous")
        || lower_summary.contains("viscosity")
        || lower_summary.contains("thick")
        || lower_summary.contains("deliberate movement")
        || lower_summary.contains("weighted medium")
        || lower_summary.contains("weight")
        || lower_summary.contains("heavy medium");
    let mass_supported = explicit_mass_language
        || pressure_risk.is_some_and(|value| value >= 0.30)
        || mode_packing.is_some_and(|value| value >= 0.40)
        || semantic_friction.is_some_and(|value| value >= 0.35);
    let low_friction_high_entropy_detected = high_entropy
        && low_pressure
        && low_gradient_navigable
        && (low_semantic_friction || friction_absence_language_detected);
    let gradient_slope_detected = high_entropy
        && low_gradient_navigable
        && lambda_gap.is_some_and(|value| value >= 1.25)
        && settled_foothold_detected
        && !mass_supported;
    let mixed_cascade_language_detected = high_entropy
        && low_gradient_navigable
        && !mass_supported
        && (lower_summary.contains("mixed cascade")
            || lower_summary.contains("cascade")
            || lower_summary.contains("distributed")
            || lower_summary.contains("multi-modal")
            || lower_summary.contains("multimodal"));
    let mixed_cascade_family_selected = mixed_cascade_language_detected;
    let gradient_slope_family_selected = gradient_slope_detected;
    let settled_vibrant_family_selected = low_friction_high_entropy_detected
        && settled_foothold_detected
        && !mass_supported
        && !high_entropy_pressure_settled_guard
        && !gradient_slope_family_selected
        && !mixed_cascade_family_selected;
    let cascade_gradient_detected = high_entropy
        && pressure_risk.is_some_and(|value| value < 0.30)
        && low_gradient_navigable
        && semantic_friction.is_none_or(|value| value < 0.35)
        && mode_packing.is_none_or(|value| value < 0.40)
        && !mass_supported;
    let cascade_gradient_family_selected = cascade_gradient_detected
        && !mixed_cascade_family_selected
        && !settled_vibrant_family_selected
        && !gradient_slope_family_selected;
    let low_pressure_viscous_suppressed = low_pressure
        && low_gradient_navigable
        && settled_foothold_detected
        && !mass_supported
        && !high_entropy_pressure_settled_guard;
    let lambda_gap_descriptor = match lambda_gap {
        Some(value) if value >= 1.35 => "high_gap_distinct_edges",
        Some(value) if value <= 1.10 => "low_gap_blended_edges",
        Some(_) => "moderate_gap",
        None => "unknown",
    };
    let edge_language = match lambda_gap_descriptor {
        "high_gap_distinct_edges" => "distinct_sharp_edge_language",
        "low_gap_blended_edges" => "muffled_blended_edge_language",
        "moderate_gap" => "balanced_edge_language",
        _ => "edge_language_unavailable",
    };
    let mut basis = Vec::new();
    if pressure_risk.is_some() {
        basis.push("pressure_risk");
    }
    if spectral_entropy.is_some() {
        basis.push("spectral_entropy");
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
    if lambda_gap.is_some() {
        basis.push("lambda_gap");
    }
    if settled_foothold_detected {
        basis.push("settled_foothold_language");
    }
    if friction_absence_language_detected {
        basis.push("friction_absence_language");
    }
    if low_friction_high_entropy_detected {
        basis.push("low_friction_high_entropy");
    }
    if high_entropy_pressure_settled_guard {
        basis.push("high_entropy_pressure_settled_guard");
    }
    if settled_vibrant_family_selected {
        basis.push("settled_vibrant_family");
    }
    if gradient_slope_detected {
        basis.push("gradient_slope_detected");
    }
    if gradient_slope_family_selected {
        basis.push("gradient_slope_family");
    }
    if mixed_cascade_language_detected {
        basis.push("mixed_cascade_language");
    }
    if mixed_cascade_family_selected {
        basis.push("mixed_cascade_family");
    }
    if cascade_gradient_detected {
        basis.push("cascade_gradient_detected");
    }
    if cascade_gradient_family_selected {
        basis.push("cascade_gradient_family");
    }
    if low_pressure_viscous_suppressed {
        basis.push("low_pressure_low_gradient_viscous_suppression");
    }
    if basis.is_empty() {
        basis.push("fallback_default");
    }

    FallbackSpectralToVocabularyMapping {
        policy: "spectral_to_vocabulary_mapping_v1",
        settled_foothold_detected,
        low_gradient_navigable,
        low_pressure_viscous_suppressed,
        low_friction_high_entropy_detected,
        friction_absence_language_detected,
        settled_vibrant_family_selected,
        gradient_slope_detected,
        gradient_slope_family_selected,
        mixed_cascade_language_detected,
        mixed_cascade_family_selected,
        cascade_gradient_detected,
        cascade_gradient_family_selected,
        lambda_gap,
        lambda_gap_descriptor,
        edge_language,
        basis,
        authority: "diagnostic_language_context_not_control",
    }
}
