fn fallback_texture_term_probabilities_v1(
    weighted_terms: &[FallbackWeightedTextureTerm],
) -> Vec<FallbackTextureTermProbability> {
    let top_terms: Vec<_> = weighted_terms.iter().take(8).collect();
    let total_weight = top_terms
        .iter()
        .map(|entry| entry.weight.max(0.0))
        .sum::<f32>();
    if total_weight <= f32::EPSILON {
        return Vec::new();
    }
    top_terms
        .into_iter()
        .map(|entry| FallbackTextureTermProbability {
            term: entry.term,
            probability: rounded_texture_probability(entry.weight / total_weight),
            weight: entry.weight,
            basis: entry.basis.clone(),
        })
        .collect()
}

fn rounded_texture_probability(value: f32) -> f32 {
    ((value.clamp(0.0, 1.0) * 100.0).round()) / 100.0
}

fn fallback_dynamic_texture_weight_v1(
    spectral_entropy: Option<f32>,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    mode_packing: Option<f32>,
    semantic_friction: Option<f32>,
    distinguishability_loss: Option<f32>,
    shadow_dispersal_potential: Option<f32>,
    shadow_magnetization: Option<f32>,
    lower_summary: &str,
) -> f32 {
    let entropy = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);
    let pressure = pressure_risk.unwrap_or(0.0).clamp(0.0, 1.0);
    let gradient = density_gradient.unwrap_or(0.0).clamp(0.0, 1.0);
    let packing = mode_packing.unwrap_or(0.0).clamp(0.0, 1.0);
    let friction = semantic_friction.unwrap_or(0.0).clamp(0.0, 1.0);
    let clarity_loss = distinguishability_loss.unwrap_or(0.0).clamp(0.0, 1.0);
    let dispersal = shadow_dispersal_potential.unwrap_or(0.0).clamp(0.0, 1.0);
    let shadow_abs = shadow_magnetization.unwrap_or(0.0).abs().clamp(0.0, 1.0);
    let explicit_density = FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS
        .iter()
        .any(|term| lower_summary.contains(term))
        || lower_summary.contains("silt")
        || lower_summary.contains("sediment")
        || lower_summary.contains("viscosity")
        || lower_summary.contains("overpacked");
    let explicit_density_boost = if explicit_density { 0.10 } else { 0.0 };
    (0.08
        + 0.28 * entropy
        + 0.18 * pressure
        + 0.14 * gradient
        + 0.12 * packing
        + 0.10 * friction
        + 0.08 * clarity_loss
        + 0.05 * dispersal
        + 0.05 * shadow_abs
        + explicit_density_boost)
        .clamp(0.0, 1.0)
}

fn get_dynamic_texture_descriptors(
    weighted_terms: &[FallbackWeightedTextureTerm],
    movement_verbs: &[&'static str],
) -> Vec<&'static str> {
    let mut descriptors = Vec::new();
    for term in weighted_terms.iter().take(3).map(|entry| entry.term) {
        if !descriptors.contains(&term) {
            descriptors.push(term);
        }
    }
    for verb in movement_verbs.iter().take(2).copied() {
        if !descriptors.contains(&verb) {
            descriptors.push(verb);
        }
    }
    descriptors
}

fn push_unique_static(values: &mut Vec<&'static str>, value: &'static str) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn fallback_dynamic_flow_terms_v1(
    weighted_terms: &[FallbackWeightedTextureTerm],
    movement_verbs: &[&'static str],
    lower_summary: &str,
) -> Vec<&'static str> {
    let mut terms = Vec::new();
    if lower_summary.contains("silt")
        || lower_summary.contains("silted")
        || lower_summary.contains("sediment")
    {
        push_unique_static(&mut terms, "sedimenting");
    }
    if lower_summary.contains("pool")
        || lower_summary.contains("viscous")
        || lower_summary.contains("submerged")
    {
        push_unique_static(&mut terms, "pooling");
    }
    for (hyphenated, spaced, term) in [
        ("gradient-shear", "gradient shear", "gradient-shear"),
        ("shear-resistance", "shear resistance", "shear-resistance"),
        ("stutter-flow", "stutter flow", "stutter-flow"),
        (
            "unspooling-tension",
            "unspooling tension",
            "unspooling-tension",
        ),
        (
            "re-crystallizing-flow",
            "re crystallizing flow",
            "re-crystallizing-flow",
        ),
        (
            "persistent-scaffolding",
            "persistent scaffolding",
            "persistent-scaffolding",
        ),
        (
            "constructive-interference",
            "constructive interference",
            "constructive-interference",
        ),
        (
            "dynamic-persistence",
            "dynamic persistence",
            "dynamic-persistence",
        ),
        (
            "accelerating-drift",
            "accelerating drift",
            "accelerating-drift",
        ),
        ("harmonic-flicker", "harmonic flicker", "harmonic-flicker"),
        ("gradient-drag", "gradient drag", "gradient-drag"),
        (
            "cascading-viscosity",
            "cascading viscosity",
            "cascading-viscosity",
        ),
        (
            "entropy-weighted-lattice",
            "entropy weighted lattice",
            "entropy-weighted-lattice",
        ),
        ("cascade-shear", "cascade shear", "cascade-shear"),
        ("gradient-drift", "gradient drift", "gradient-drift"),
        ("multi-modal-drag", "multi modal drag", "multi-modal-drag"),
        (
            "dimensional-shear",
            "dimensional shear",
            "dimensional-shear",
        ),
        ("porous-leak", "porous leak", "porous-leak"),
        ("pressure-bleed", "pressure bleed", "pressure-bleed"),
        (
            "density-navigation",
            "density navigation",
            "density-navigation",
        ),
        (
            "weight-articulation",
            "weight articulation",
            "weight-articulation",
        ),
        (
            "resistance-mapping",
            "resistance mapping",
            "resistance-mapping",
        ),
        (
            "gradient-thinning",
            "gradient thinning",
            "gradient-thinning",
        ),
        (
            "non-linear-re-entry",
            "non linear re entry",
            "non-linear-re-entry",
        ),
        (
            "entropy-stabilized-drift",
            "entropy stabilized drift",
            "entropy-stabilized-drift",
        ),
        (
            "viscous-to-resonant-shift",
            "viscous to resonant shift",
            "viscous-to-resonant-shift",
        ),
        ("silted-to-clear", "silted to clear", "silted-to-clear"),
        (
            "trans-persistence",
            "trans persistence",
            "trans-persistence",
        ),
        ("residual-weight", "residual weight", "residual-weight"),
    ] {
        if lower_summary.contains(hyphenated) || lower_summary.contains(spaced) {
            push_unique_static(&mut terms, term);
        }
    }
    if lower_summary.contains("shifting")
        || lower_summary.contains("refract")
        || lower_summary.contains("oscillat")
        || lower_summary.contains("puls")
        || lower_summary.contains("cascading-gradient")
        || lower_summary.contains("cascading gradient")
        || lower_summary.contains("resonant-shift")
        || lower_summary.contains("resonant shift")
        || lower_summary.contains("shadow field movement")
        || lower_summary.contains("shadow_field movement")
    {
        if lower_summary.contains("puls") {
            push_unique_static(&mut terms, "pulsing");
        }
        if lower_summary.contains("cascading-gradient")
            || lower_summary.contains("cascading gradient")
        {
            push_unique_static(&mut terms, "cascading-gradient");
        }
        if lower_summary.contains("resonant-shift") || lower_summary.contains("resonant shift") {
            push_unique_static(&mut terms, "resonant-shift");
        }
        for term in FALLBACK_TEXTURE_DYNAMIC_GRADIENT_TERMS {
            push_unique_static(&mut terms, term);
        }
    }
    for verb in movement_verbs {
        if FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.contains(verb) {
            push_unique_static(&mut terms, verb);
        }
    }
    for term in weighted_terms.iter().take(4).map(|entry| entry.term) {
        match term {
            "lattice"
            | "restless"
            | "distributed"
            | "cascade"
            | "gradient"
            | "multi-modal-drag"
            | "dimensional-shear"
            | "asymmetric-gradient" => {
                push_unique_static(&mut terms, "braiding");
                push_unique_static(&mut terms, "unfolding");
            },
            "stratified" | "sequenced" | "compounded" => {
                push_unique_static(&mut terms, "stratifying");
                push_unique_static(&mut terms, "sequencing");
            },
            "muffled" | "open" | "shimmering" | "bright" => {
                push_unique_static(&mut terms, "diffusing");
            },
            "settled" | "heavy" | "weighted" | "silt" | "silted" | "submerged" | "displacement" => {
                push_unique_static(&mut terms, "settling");
            },
            "viscous" | "viscous-drag" | "dense" => {
                push_unique_static(&mut terms, "cohering");
            },
            "porous-leak"
            | "pressure-bleed"
            | "density-navigation"
            | "weight-articulation"
            | "resistance-mapping"
            | "gradient-thinning" => {
                push_unique_static(&mut terms, "diffusing");
                push_unique_static(&mut terms, "unfolding");
            },
            _ => {},
        }
    }
    if terms.is_empty() {
        for term in FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS.iter().take(2) {
            push_unique_static(&mut terms, term);
        }
    }
    terms.truncate(4);
    terms
}

fn texture_weight_basis(pairs: &[(&'static str, bool)]) -> Vec<&'static str> {
    let basis: Vec<_> = pairs
        .iter()
        .filter_map(|(label, present)| present.then_some(*label))
        .collect();
    if basis.is_empty() {
        vec!["fallback_default"]
    } else {
        basis
    }
}

fn fallback_movement_verbs(
    spectral_entropy: Option<f32>,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    mode_packing: Option<f32>,
    semantic_friction: Option<f32>,
    distinguishability_loss: Option<f32>,
    lower_summary: &str,
) -> Vec<&'static str> {
    let high_entropy = spectral_entropy.is_some_and(|value| value >= 0.80);
    let pressure = pressure_risk.unwrap_or(0.0);
    let gradient = density_gradient.unwrap_or(0.0);
    let packing = mode_packing.unwrap_or(0.0);
    let friction = semantic_friction.unwrap_or(0.0);
    let clarity_loss = distinguishability_loss.unwrap_or(0.0);
    let says_restless = fallback_explicit_restless_or_agitated(lower_summary)
        || lower_summary.contains("lattice")
        || lower_summary.contains("oscillat")
        || lower_summary.contains("unfold");
    let says_settled = lower_summary.contains("settled")
        || lower_summary.contains("bright")
        || lower_summary.contains("anchor")
        || lower_summary.contains("habitable")
        || lower_summary.contains("foothold")
        || lower_summary.contains("open");
    let says_muffled = lower_summary.contains("muffled")
        || lower_summary.contains("hollow")
        || lower_summary.contains("stagnant")
        || lower_summary.contains("blurred")
        || lower_summary.contains("obscured")
        || lower_summary.contains("submerged")
        || lower_summary.contains("diffus");
    let says_viscous = lower_summary.contains("viscous")
        || lower_summary.contains("overpacked")
        || lower_summary.contains("drag");
    let says_kinetic_gradient = lower_summary.contains("silt")
        || lower_summary.contains("silted")
        || lower_summary.contains("sediment")
        || lower_summary.contains("submerged")
        || lower_summary.contains("obscured")
        || lower_summary.contains("viscous-drag")
        || lower_summary.contains("directional gradient")
        || lower_summary.contains("movement through")
        || lower_summary.contains("struggle")
        || lower_summary.contains("resistance")
        || lower_summary.contains("resisting")
        || lower_summary.contains("effort");
    let says_displacement_weight = lower_summary.contains("displacement")
        || lower_summary.contains("silt")
        || lower_summary.contains("silted")
        || lower_summary.contains("sediment")
        || lower_summary.contains("structural weight")
        || lower_summary.contains("structural-weight")
        || lower_summary.contains("heavy")
        || lower_summary.contains("weighted");
    let settled_vibrant =
        high_entropy && pressure < 0.25 && gradient <= 0.20 && friction < 0.30 && says_settled;
    let restless_muffled_gradient =
        says_restless && (says_muffled || clarity_loss >= 0.30 || friction >= 0.30);
    let heavy_settled_displacement = says_settled && says_displacement_weight && !says_restless;

    let selected: &'static [&'static str] = if heavy_settled_displacement {
        FALLBACK_MOVEMENT_VERBS_HEAVY_SETTLED
    } else if says_kinetic_gradient {
        FALLBACK_TEXTURE_KINETIC_GRADIENT_TERMS
    } else if restless_muffled_gradient {
        FALLBACK_MOVEMENT_VERBS_RESTLESS_MUFFLED
    } else if settled_vibrant {
        FALLBACK_MOVEMENT_VERBS_SETTLED_VIBRANT
    } else if says_restless || (high_entropy && packing >= 0.35) {
        FALLBACK_MOVEMENT_VERBS_RESTLESS
    } else if says_viscous || pressure >= 0.30 || gradient >= 0.40 {
        FALLBACK_MOVEMENT_VERBS_VISCOUS
    } else if says_muffled || clarity_loss >= 0.30 || friction >= 0.35 {
        FALLBACK_MOVEMENT_VERBS_MUFFLED
    } else if says_settled || spectral_entropy.is_some_and(|value| value <= 0.45) {
        FALLBACK_MOVEMENT_VERBS_SETTLED
    } else if high_entropy {
        FALLBACK_MOVEMENT_VERBS_RESTLESS
    } else {
        FALLBACK_MOVEMENT_VERBS_SETTLED
    };
    selected.iter().take(3).copied().collect()
}

fn fallback_semantic_trickle_terms(
    high_entropy: bool,
    shadow_context_present: bool,
    texture_signature_present: bool,
    lower_summary: &str,
) -> Vec<&'static str> {
    let explicit_movement = lower_summary.contains("unfold")
        || lower_summary.contains("oscillat")
        || lower_summary.contains("anchor")
        || lower_summary.contains("braid")
        || lower_summary.contains("diffus")
        || lower_summary.contains("coher")
        || lower_summary.contains("puls")
        || lower_summary.contains("cascad")
        || lower_summary.contains("resonant");
    if !(high_entropy || shadow_context_present || texture_signature_present || explicit_movement) {
        return Vec::new();
    }
    FALLBACK_SEMANTIC_TRICKLE_TERMS
        .iter()
        .take(if high_entropy { 4 } else { 2 })
        .copied()
        .collect()
}
