fn fallback_cascade_gradient_v1(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> FallbackCascadeGradient {
    let lower = spectral_summary.to_ascii_lowercase();
    let mapping = &selector.spectral_to_vocabulary_mapping;
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let density_gradient = selector.density_gradient.unwrap_or(1.0);
    let pressure = selector.pressure_risk.unwrap_or(0.0);
    let friction = selector.semantic_friction.unwrap_or(0.0);
    let packing = selector.mode_packing.unwrap_or(0.0);
    let navigable_gradient = selector.density_gradient.is_some_and(|value| value <= 0.25);
    let pressure_mass_blocked = pressure >= 0.30
        || friction >= 0.35
        || packing >= 0.40
        || lower.contains("overpacked")
        || lower.contains("viscous");
    let mixed_cascade_gap_detected = high_entropy
        && navigable_gradient
        && !pressure_mass_blocked
        && !mapping.settled_vibrant_family_selected;
    let cascade_gradient_detected = mapping.cascade_gradient_detected || mixed_cascade_gap_detected;
    let family_selected = selector.texture_family == "cascade_gradient_navigable"
        || selector.texture_family == "mixed_cascade_gradient";
    let gradient_state = if density_gradient <= 0.15 {
        "smooth_open_slope"
    } else if density_gradient <= 0.25 {
        "navigable_textured_slope"
    } else if density_gradient <= 0.40 {
        "moderate_slope"
    } else {
        "steep_or_resistant_slope"
    };
    let navigability = if cascade_gradient_detected && !pressure_mass_blocked {
        "navigable"
    } else if pressure_mass_blocked {
        "blocked_by_pressure_or_mass"
    } else {
        "not_enough_context"
    };
    let movement_language = if family_selected {
        "movement_and_edge_language_preferred_over_static_adjectives"
    } else if mapping.settled_vibrant_family_selected {
        "settled_vibrant_family_handles_habitable_cascade"
    } else {
        "fallback_family_handles_current_state"
    };
    let mut basis = Vec::new();
    if high_entropy {
        basis.push("high_entropy");
    }
    if selector.density_gradient.is_some() {
        basis.push("density_gradient");
    }
    if mapping.lambda_gap.is_some() {
        basis.push("lambda_gap");
    }
    if mapping.settled_foothold_detected {
        basis.push("settled_foothold");
    }
    if !pressure_mass_blocked {
        basis.push("pressure_mass_absent");
    }
    if family_selected {
        basis.push("cascade_gradient_family_selected");
    }
    if selector.texture_family == "mixed_cascade_gradient" {
        basis.push("mixed_cascade_gradient_family_selected");
    }
    if basis.is_empty() {
        basis.push("insufficient_context");
    }

    FallbackCascadeGradient {
        policy: "fallback_cascade_gradient_v1",
        cascade_gradient_detected,
        mixed_cascade_gap_detected,
        family_selected,
        gradient_state,
        lambda_gap_descriptor: mapping.lambda_gap_descriptor,
        navigability,
        pressure_mass_blocked,
        movement_language,
        basis,
        authority: "diagnostic_language_context_not_control",
    }
}

fn fallback_gradient_slope_v1(
    spectral_summary: &str,
    spectral_entropy: Option<f32>,
    selector: &FallbackShadowTextureSelector,
) -> FallbackGradientSlope {
    let lower = spectral_summary.to_ascii_lowercase();
    let mapping = &selector.spectral_to_vocabulary_mapping;
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let density_gradient = selector.density_gradient.unwrap_or(1.0);
    let low_gradient = selector.density_gradient.is_some_and(|value| value <= 0.20);
    let lambda_gap_shaped = mapping.lambda_gap.is_some_and(|value| value >= 1.25);
    let pressure_mass_blocked = selector.pressure_risk.unwrap_or(0.0) >= 0.30
        || selector.mode_packing.unwrap_or(0.0) >= 0.40
        || selector.semantic_friction.unwrap_or(0.0) >= 0.35
        || lower.contains("overpacked")
        || lower.contains("viscous");
    let slope_detected =
        mapping.gradient_slope_detected || (high_entropy && low_gradient && lambda_gap_shaped);
    let family_selected = selector.texture_family == "gradient_slope_navigable";
    let mixed_vs_graduated = if family_selected {
        "graduated_shaped_not_mixed"
    } else if slope_detected && pressure_mass_blocked {
        "shape_present_but_mass_overrides"
    } else if slope_detected {
        "graduated_shape_detected"
    } else {
        "not_enough_slope_context"
    };
    let gradient_language = if density_gradient <= 0.12 {
        "smooth_navigable_slope"
    } else if density_gradient <= 0.20 {
        "tapered_graduated_slope"
    } else {
        "slope_not_low_gradient"
    };
    let mut basis = Vec::new();
    if high_entropy {
        basis.push("high_entropy");
    }
    if selector.density_gradient.is_some() {
        basis.push("density_gradient");
    }
    if mapping.lambda_gap.is_some() {
        basis.push("lambda_gap");
    }
    if mapping.settled_foothold_detected {
        basis.push("settled_habitable_foothold");
    }
    if family_selected {
        basis.push("gradient_slope_family_selected");
    }
    if pressure_mass_blocked {
        basis.push("pressure_mass_override");
    }
    if basis.is_empty() {
        basis.push("insufficient_context");
    }

    FallbackGradientSlope {
        policy: "fallback_gradient_slope_v1",
        slope_detected,
        family_selected,
        gradient_language,
        mixed_vs_graduated,
        lambda_gap_descriptor: mapping.lambda_gap_descriptor,
        pressure_mass_blocked,
        preferred_terms: FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS,
        basis,
        authority: "diagnostic_language_context_not_control",
    }
}

fn fallback_vocabulary_overweight_guard_v1(
    selector: &FallbackShadowTextureSelector,
) -> FallbackVocabularyOverweightGuard {
    let specific_family = selector.texture_family != "mixed_shadow_context"
        && selector.texture_family != "fallback_default";
    let token_only_risk = specific_family && selector.preferred_texture_terms.len() >= 3;
    let guard_state = if selector
        .spectral_to_vocabulary_mapping
        .mixed_cascade_family_selected
    {
        "mixed_cascade_terms_advisory_use_gradient_and_edges"
    } else if selector
        .spectral_to_vocabulary_mapping
        .cascade_gradient_family_selected
    {
        "cascade_terms_advisory_use_movement_and_edges"
    } else if selector.texture_family == "restless_muffled_gradient" {
        "restless_muffled_terms_advisory_use_motion_and_edges"
    } else if selector
        .spectral_to_vocabulary_mapping
        .settled_vibrant_family_selected
    {
        "settled_vibrant_terms_advisory_paraphrase_allowed"
    } else if token_only_risk {
        "preferred_terms_advisory_not_required_vocabulary"
    } else {
        "low_overweight_risk"
    };
    let mut basis = vec![selector.texture_family];
    if token_only_risk {
        basis.push("token_only_risk");
    }
    if selector
        .spectral_to_vocabulary_mapping
        .cascade_gradient_detected
    {
        basis.push("cascade_gradient_detected");
    }
    if selector
        .spectral_to_vocabulary_mapping
        .mixed_cascade_language_detected
    {
        basis.push("mixed_cascade_language_detected");
    }

    FallbackVocabularyOverweightGuard {
        policy: "fallback_vocabulary_overweight_guard_v1",
        preferred_terms_advisory: true,
        paraphrase_allowed: true,
        token_only_risk,
        guard_state,
        basis,
        authority: "diagnostic_language_context_not_control",
    }
}

fn rounded_texture_weight(value: f32) -> f32 {
    ((value.clamp(0.0, 1.0) * 100.0).round()) / 100.0
}

fn fallback_shadow_texture_anchor_v1(spectral_summary: &str) -> FallbackShadowTextureAnchor {
    let lower = spectral_summary.to_ascii_lowercase();
    let shadow_context_present = lower.contains("shadow-v3")
        || lower.contains("shadow_field")
        || lower.contains("shadow field");
    let texture_signature_present = lower.contains("texture_signature");
    let anchor_source = if shadow_context_present {
        "shadow_context"
    } else if texture_signature_present {
        "texture_signature"
    } else {
        "fallback_default"
    };
    FallbackShadowTextureAnchor {
        policy: "fallback_shadow_texture_anchor_v1",
        shadow_context_present,
        required_texture_anchor: shadow_context_present || texture_signature_present,
        accepted_texture_terms: FALLBACK_SHADOW_TEXTURE_TERMS,
        anchor_source,
    }
}

fn normalize_fallback_unit(value: f32) -> f32 {
    if value > 1.0 && value <= 100.0 {
        (value / 100.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn normalize_fallback_signed_unit(value: f32) -> f32 {
    if value.abs() > 1.0 && value.abs() <= 100.0 {
        (value / 100.0).clamp(-1.0, 1.0)
    } else {
        value.clamp(-1.0, 1.0)
    }
}

fn extract_fallback_spectral_entropy(spectral_summary: &str) -> Option<f32> {
    [
        "spectral_entropy",
        "spectral entropy",
        "entropy_level",
        "entropy level",
    ]
    .iter()
    .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_resonance_density(spectral_summary: &str) -> Option<f32> {
    ["resonance_density", "resonance density"]
        .iter()
        .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_pressure_risk(spectral_summary: &str) -> Option<f32> {
    ["pressure_risk", "pressure risk"]
        .iter()
        .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_distinguishability_loss(spectral_summary: &str) -> Option<f32> {
    ["distinguishability_loss", "distinguishability loss"]
        .iter()
        .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_density_gradient(spectral_summary: &str) -> Option<f32> {
    ["density_gradient", "density gradient"]
        .iter()
        .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_mode_packing(spectral_summary: &str) -> Option<f32> {
    ["mode_packing", "mode packing"]
        .iter()
        .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_semantic_friction(spectral_summary: &str) -> Option<f32> {
    ["semantic_friction", "semantic friction"]
        .iter()
        .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_lambda_gap(spectral_summary: &str) -> Option<f32> {
    [
        "lambda_gap",
        "lambda gap",
        "lambda1/lambda2 gap",
        "lambda1 lambda2 gap",
        "λ1/λ2 gap",
    ]
    .iter()
    .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_fallback_shadow_dispersal_potential(spectral_summary: &str) -> Option<f32> {
    [
        "shadow_dispersal_potential",
        "shadow dispersal potential",
        "dispersal_potential",
        "dispersal potential",
    ]
    .iter()
    .find_map(|label| {
        extract_max_number_after_label_clause(spectral_summary, label)
            .or_else(|| extract_number_after_label(spectral_summary, label))
    })
}

fn extract_fallback_shadow_magnetization(spectral_summary: &str) -> Option<f32> {
    [
        "shadow_magnetization",
        "shadow magnetization",
        "magnetization",
    ]
    .iter()
    .find_map(|label| extract_number_after_label(spectral_summary, label))
}

fn extract_number_after_label(text: &str, label: &str) -> Option<f32> {
    let haystack = text.to_ascii_lowercase();
    let label = label.to_ascii_lowercase();
    let mut offset = 0usize;
    while let Some(pos) = haystack.get(offset..)?.find(&label) {
        let after_label = offset.saturating_add(pos).saturating_add(label.len());
        if let Some(value) = first_f32_in_prefix(haystack.get(after_label..)?, 48) {
            return Some(value);
        }
        offset = after_label;
    }
    None
}

fn extract_max_number_after_label_clause(text: &str, label: &str) -> Option<f32> {
    let haystack = text.to_ascii_lowercase();
    let label = label.to_ascii_lowercase();
    let mut offset = 0usize;
    while let Some(pos) = haystack.get(offset..)?.find(&label) {
        let after_label = offset.saturating_add(pos).saturating_add(label.len());
        let clause = haystack
            .get(after_label..)?
            .split(['\n', ',', ';'])
            .next()
            .unwrap_or_default();
        if let Some(value) = max_f32_in_prefix(clause, 64) {
            return Some(value);
        }
        offset = after_label;
    }
    None
}

fn first_f32_in_prefix(text: &str, max_chars: usize) -> Option<f32> {
    let mut start = None;
    let mut seen_chars = 0usize;
    let mut previous = '\0';
    let mut chars = text.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if seen_chars >= max_chars {
            break;
        }
        seen_chars = seen_chars.saturating_add(1);
        let next_is_digit = chars
            .peek()
            .map(|(_, next)| next.is_ascii_digit())
            .unwrap_or(false);
        if ch.is_ascii_digit()
            || ((ch == '-' || ch == '+') && next_is_digit)
            || (ch == '.' && next_is_digit)
        {
            start = Some(idx);
            break;
        }
        if ch == '\n' || (ch == ',' && previous != ':') || ch == ';' {
            break;
        }
        previous = ch;
    }
    let start = start?;
    let mut end = start;
    for (idx, ch) in text.get(start..)?.char_indices() {
        if !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+')) {
            break;
        }
        end = start.saturating_add(idx).saturating_add(ch.len_utf8());
    }
    text.get(start..end)?
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

fn max_f32_in_prefix(text: &str, max_chars: usize) -> Option<f32> {
    let prefix = text.chars().take(max_chars).collect::<String>();
    prefix
        .split(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+')))
        .filter_map(|candidate| candidate.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .max_by(f32::total_cmp)
}
