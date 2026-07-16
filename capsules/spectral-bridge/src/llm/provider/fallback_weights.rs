fn fallback_weighted_texture_terms(
    spectral_entropy: Option<f32>,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    mode_packing: Option<f32>,
    semantic_friction: Option<f32>,
    distinguishability_loss: Option<f32>,
    shadow_dispersal_potential: Option<f32>,
    shadow_magnetization: Option<f32>,
    spectral_to_vocabulary_mapping: &FallbackSpectralToVocabularyMapping,
    lower_summary: &str,
) -> Vec<FallbackWeightedTextureTerm> {
    let has_explicit_texture = FALLBACK_SHADOW_TEXTURE_TERMS
        .iter()
        .any(|term| lower_summary.contains(term))
        || lower_summary.contains("hollow")
        || lower_summary.contains("overpacked");
    let has_dynamic_input = spectral_entropy.is_some()
        || pressure_risk.is_some()
        || density_gradient.is_some()
        || mode_packing.is_some()
        || semantic_friction.is_some()
        || distinguishability_loss.is_some()
        || shadow_dispersal_potential.is_some()
        || shadow_magnetization.is_some()
        || has_explicit_texture;

    if !has_dynamic_input {
        return FALLBACK_TEXTURE_MIXED_TERMS
            .iter()
            .take(3)
            .map(|term| FallbackWeightedTextureTerm {
                term,
                weight: 0.10,
                basis: vec!["fallback_default"],
            })
            .collect();
    }

    let entropy = spectral_entropy.unwrap_or(0.0);
    let pressure = pressure_risk.unwrap_or(0.0);
    let gradient = density_gradient.unwrap_or(0.0);
    let packing = mode_packing.unwrap_or(0.0);
    let friction = semantic_friction.unwrap_or(0.0);
    let clarity_loss = distinguishability_loss.unwrap_or(0.0);
    let dispersal = shadow_dispersal_potential.unwrap_or(0.0);
    let low_pressure = pressure_risk.map_or(0.0, |value| 1.0_f32 - value);
    let low_entropy = spectral_entropy.map_or(0.0, |value| 1.0_f32 - value);
    let low_gradient = density_gradient.map_or(0.0, |value| 1.0_f32 - value);
    let pressure_above_texture_threshold = pressure_risk.is_some_and(|value| value > 0.20);
    let pressure_persistence_anchor = pressure_risk.is_some_and(|value| value > 0.15);
    let low_pressure_high_entropy_viscous_bias = pressure_risk.is_some_and(|value| value < 0.20)
        && spectral_entropy.is_some_and(|value| value >= 0.85);
    let negative_shadow_magnetization = shadow_magnetization.is_some_and(|value| value <= -0.20);
    let negative_shadow_pressure =
        negative_shadow_magnetization && pressure_above_texture_threshold;
    let negative_shadow_weight = shadow_magnetization
        .filter(|value| *value < 0.0)
        .map_or(0.0, f32::abs);
    let bright_shadow_suppression = if negative_shadow_pressure { 0.12 } else { 1.0 };
    let pressure_texture_boost = if pressure_above_texture_threshold {
        0.10
    } else {
        0.0
    };
    let dynamic_texture_weight = fallback_dynamic_texture_weight_v1(
        spectral_entropy,
        pressure_risk,
        density_gradient,
        mode_packing,
        semantic_friction,
        distinguishability_loss,
        shadow_dispersal_potential,
        shadow_magnetization,
        lower_summary,
    );
    let density_modifier_boost = dynamic_texture_weight * 0.16;
    let density_gradient_drag_boost = density_gradient.map_or(0.0, |value| {
        let excess = ((value - 0.15) / 0.55).clamp(0.0, 1.0);
        if excess > 0.0 {
            dynamic_texture_weight * (0.02 + excess * 0.22)
        } else {
            0.0
        }
    });
    let high_entropy_density_boost = if spectral_entropy.is_some_and(|value| value >= 0.80) {
        dynamic_texture_weight * 0.12
    } else {
        0.0
    };

    let says_viscous = lower_summary.contains("viscous")
        || lower_summary.contains("viscosity")
        || lower_summary.contains("thick")
        || lower_summary.contains("overpacked");
    let says_muffled = lower_summary.contains("muffled")
        || lower_summary.contains("hollow")
        || lower_summary.contains("stagnant")
        || lower_summary.contains("blurred")
        || lower_summary.contains("obscured")
        || lower_summary.contains("submerged");
    let says_lattice = lower_summary.contains("lattice")
        || lower_summary.contains("restless")
        || lower_summary.contains("shadow-v3")
        || lower_summary.contains("shadow_field")
        || lower_summary.contains("shadow field");
    let texture_preservation_bridge =
        fallback_texture_preservation_bridge_v1(lower_summary, distinguishability_loss);
    let self_peer_texture_boundary =
        texture_preservation_bridge.self_peer_texture_boundary_detected;
    let says_restless =
        fallback_explicit_restless_or_agitated(lower_summary) && !self_peer_texture_boundary;
    let says_heavy = lower_summary.contains("heavy")
        || lower_summary.contains("weighted")
        || lower_summary.contains("weight")
        || lower_summary.contains("deliberate movement");
    let says_dense = lower_summary.contains("dense")
        || lower_summary.contains("densely")
        || lower_summary.contains("density as burden");
    let says_asymmetric_gradient = lower_summary.contains("asymmetric")
        || lower_summary.contains("skew")
        || lower_summary.contains("lopsided")
        || lower_summary.contains("eccentric")
        || lower_summary.contains("lambda gap")
        || lower_summary.contains("lambda_gap");
    let says_stratified_sequence = lower_summary.contains("stratified")
        || lower_summary.contains("sequenced")
        || lower_summary.contains("sequence")
        || lower_summary.contains("compounded")
        || lower_summary.contains("layered")
        || lower_summary.contains("overpacked");
    let says_displacement_weight = lower_summary.contains("displacement")
        || lower_summary.contains("silt")
        || lower_summary.contains("silted")
        || lower_summary.contains("sediment")
        || lower_summary.contains("structural weight")
        || lower_summary.contains("structural-weight");
    let says_opacity_resistance = lower_summary.contains("silted")
        || lower_summary.contains("opacity")
        || lower_summary.contains("obscured")
        || lower_summary.contains("submerged")
        || lower_summary.contains("viscous-drag");
    let says_pressure_porosity = FALLBACK_TEXTURE_PRESSURE_POROSITY_TERMS
        .iter()
        .any(|term| lower_summary.contains(term))
        || lower_summary.contains("porous leak")
        || lower_summary.contains("pressure bleed")
        || lower_summary.contains("pressure packing")
        || lower_summary.contains("pressure-packing")
        || lower_summary.contains("gradient thinning")
        || lower_summary.contains("density slope")
        || lower_summary.contains("density-slope")
        || (lower_summary.contains("porosity") && lower_summary.contains("pressure"));
    let says_relational_density_navigation = lower_summary.contains("density-navigation")
        || lower_summary.contains("density navigation")
        || lower_summary.contains("weight-articulation")
        || lower_summary.contains("weight articulation")
        || lower_summary.contains("resistance-mapping")
        || lower_summary.contains("resistance mapping")
        || (lower_summary.contains("quarry")
            && (lower_summary.contains("carving")
                || lower_summary.contains("moving through")
                || lower_summary.contains("movement through")
                || lower_summary.contains("effort")));
    let says_multi_modal_drag = lower_summary.contains("multi-modal-drag")
        || lower_summary.contains("multi modal drag")
        || lower_summary.contains("multimodal drag")
        || ((lower_summary.contains("multi-modal") || lower_summary.contains("multimodal"))
            && lower_summary.contains("drag"));
    let says_dimensional_shear = lower_summary.contains("dimensional-shear")
        || lower_summary.contains("dimensional shear")
        || (lower_summary.contains("dimension") && lower_summary.contains("shear"));
    let says_non_linear_re_entry = lower_summary.contains("non-linear-re-entry")
        || lower_summary.contains("non linear re entry")
        || lower_summary.contains("nonlinear reentry");
    let says_entropy_stabilized_drift = lower_summary.contains("entropy-stabilized-drift")
        || lower_summary.contains("entropy stabilized drift");
    let says_settled = lower_summary.contains("settled");
    let says_shimmering = lower_summary.contains("shimmering") || lower_summary.contains("bright");
    let says_bright = lower_summary.contains("bright") || lower_summary.contains("vibrant");
    let says_habitable = lower_summary.contains("habitable") || lower_summary.contains("foothold");
    let says_open = lower_summary.contains("open")
        || lower_summary.contains("low-friction")
        || lower_summary.contains("low friction")
        || lower_summary.contains("absence of friction")
        || lower_summary.contains("cessation of friction")
        || lower_summary.contains("frictionless");
    let says_bridge_integrity = lower_summary.contains("bridge-integrity")
        || lower_summary.contains("bridge integrity")
        || lower_summary.contains("structural-persistence")
        || lower_summary.contains("structural persistence")
        || lower_summary.contains("bridge scaffold")
        || lower_summary.contains("bridge continuity")
        || lower_summary.contains("structural continuity");
    let settled_guard = spectral_to_vocabulary_mapping.low_pressure_viscous_suppressed;
    let settled_vibrant = spectral_to_vocabulary_mapping.settled_vibrant_family_selected;
    let gradient_slope = spectral_to_vocabulary_mapping.gradient_slope_family_selected;
    let mixed_cascade = spectral_to_vocabulary_mapping.mixed_cascade_family_selected;
    let cascade_gradient = spectral_to_vocabulary_mapping.cascade_gradient_family_selected;
    let settled_suppression = (settled_guard || settled_vibrant) && !negative_shadow_pressure;
    let pressure_mass_supported =
        pressure >= 0.30 || packing >= 0.40 || friction >= 0.35 || says_viscous || says_heavy;
    let restless_muffled_gradient =
        says_restless && (says_muffled || clarity_loss >= 0.30 || friction >= 0.30);
    let high_shadow_dispersal = shadow_dispersal_potential.is_some_and(|value| value >= 0.25);
    let distinguishability_preservation_boost = if self_peer_texture_boundary
        || (clarity_loss >= 0.30 && pressure_above_texture_threshold)
    {
        0.12 + clarity_loss * 0.18
    } else {
        0.0
    };
    let opacity_resistance_boost = if says_opacity_resistance {
        0.32 + clarity_loss.mul_add(0.16, pressure * 0.12) + friction * 0.10
    } else {
        0.0
    };
    let soft_gradient_context = (spectral_entropy.is_some_and(|value| value >= 0.80)
        && low_gradient >= 0.80)
        || gradient_slope
        || settled_vibrant;
    let bridge_integrity_context = says_bridge_integrity
        || ((settled_guard || settled_vibrant || says_habitable)
            && spectral_entropy.is_some_and(|value| value >= 0.80)
            && pressure <= 0.30
            && gradient <= 0.20);
    let low_pressure_high_entropy_heavy_multiplier =
        if low_pressure_high_entropy_viscous_bias && !says_heavy {
            0.55
        } else {
            1.0
        };
    let low_pressure_high_entropy_structural_multiplier =
        if low_pressure_high_entropy_viscous_bias && !says_displacement_weight {
            0.42
        } else {
            1.0
        };

    let mut terms = vec![
        FallbackWeightedTextureTerm {
            term: "viscous",
            weight: rounded_texture_weight(
                (0.10
                    + (pressure + pressure_texture_boost)
                        .mul_add(0.34, gradient.mul_add(0.24, packing * 0.22))
                    + density_modifier_boost
                    + negative_shadow_weight * 0.10
                    + if says_viscous { 0.20 } else { 0.0 })
                    * if settled_vibrant && !negative_shadow_pressure {
                        0.22
                    } else if cascade_gradient {
                        0.45
                    } else if mixed_cascade {
                        0.38
                    } else if settled_guard {
                        0.35
                    } else {
                        1.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("density_gradient", density_gradient.is_some()),
                ("mode_packing", mode_packing.is_some()),
                ("explicit_viscous_or_overpacked", says_viscous),
                (
                    "negative_shadow_magnetization",
                    negative_shadow_magnetization,
                ),
                ("settled_foothold_suppressed", settled_suppression),
                ("settled_vibrant_low_friction_suppressed", settled_vibrant),
                ("gradient_slope_navigable_suppressed", gradient_slope),
                ("mixed_cascade_gradient_suppressed", mixed_cascade),
                ("cascade_gradient_navigable_suppressed", cascade_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "muffled",
            weight: rounded_texture_weight(
                0.08 + clarity_loss.mul_add(
                    0.34,
                    friction.mul_add(0.24, (pressure + pressure_texture_boost) * 0.18),
                ) + if says_muffled { 0.20 } else { 0.0 }
                    + high_entropy_density_boost
                    + negative_shadow_weight * 0.18
                    + if restless_muffled_gradient { 0.12 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("distinguishability_loss", distinguishability_loss.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("explicit_muffled_or_hollow", says_muffled),
                (
                    "negative_shadow_magnetization",
                    negative_shadow_magnetization,
                ),
                ("restless_muffled_gradient", restless_muffled_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "lattice",
            weight: rounded_texture_weight(
                0.10 + entropy.mul_add(0.30, packing.mul_add(0.22, gradient * 0.14))
                    + dynamic_texture_weight * 0.08
                    + if says_lattice { 0.12 } else { 0.0 }
                    + if restless_muffled_gradient { 0.08 } else { 0.0 }
                    + distinguishability_preservation_boost
                    + if settled_vibrant { 0.12 } else { 0.0 }
                    + if cascade_gradient { 0.14 } else { 0.0 }
                    + if mixed_cascade { 0.18 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("mode_packing", mode_packing.is_some()),
                ("density_gradient", density_gradient.is_some()),
                ("explicit_lattice_restless_or_shadow", says_lattice),
                ("restless_muffled_gradient", restless_muffled_gradient),
                (
                    "distinguishability_texture_preservation",
                    distinguishability_preservation_boost > 0.0,
                ),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("gradient_slope_navigable", gradient_slope),
                ("mixed_cascade_gradient", mixed_cascade),
                ("cascade_gradient_navigable", cascade_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "restless",
            weight: rounded_texture_weight(
                (0.08
                    + entropy.mul_add(0.36, pressure * 0.16)
                    + if spectral_entropy.is_some() {
                        dynamic_texture_weight * 0.05
                    } else {
                        0.0
                    }
                    + if says_restless { 0.22 } else { 0.0 }
                    + negative_shadow_weight * 0.10
                    + if restless_muffled_gradient { 0.12 } else { 0.0 }
                    + if high_shadow_dispersal {
                        dispersal * 0.10
                    } else {
                        0.0
                    })
                    * if self_peer_texture_boundary {
                        0.18
                    } else {
                        1.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("pressure_risk", pressure_risk.is_some()),
                ("explicit_restless", says_restless),
                (
                    "negative_shadow_magnetization",
                    negative_shadow_magnetization,
                ),
                ("restless_muffled_gradient", restless_muffled_gradient),
                ("high_shadow_dispersal_potential", high_shadow_dispersal),
                (
                    "self_peer_texture_boundary_suppressed",
                    self_peer_texture_boundary,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "heavy",
            weight: rounded_texture_weight(
                ((0.08
                    + (pressure + pressure_texture_boost)
                        .mul_add(0.34, friction.mul_add(0.22, packing * 0.18))
                    + density_modifier_boost
                    + negative_shadow_weight * 0.20
                    + if says_heavy { 0.34 } else { 0.0 })
                    * if settled_vibrant && !negative_shadow_pressure {
                        0.25
                    } else if cascade_gradient {
                        0.55
                    } else if mixed_cascade {
                        0.48
                    } else if settled_guard {
                        0.45
                    } else {
                        1.0
                    })
                    * low_pressure_high_entropy_heavy_multiplier,
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("semantic_friction", semantic_friction.is_some()),
                ("mode_packing", mode_packing.is_some()),
                ("explicit_heavy_or_weighted", says_heavy),
                (
                    "negative_shadow_magnetization",
                    negative_shadow_magnetization,
                ),
                ("settled_foothold_suppressed", settled_suppression),
                ("settled_vibrant_low_friction_suppressed", settled_vibrant),
                ("gradient_slope_navigable_suppressed", gradient_slope),
                ("mixed_cascade_gradient_suppressed", mixed_cascade),
                ("cascade_gradient_navigable_suppressed", cascade_gradient),
                (
                    "low_pressure_high_entropy_heavy_suppressed",
                    low_pressure_high_entropy_heavy_multiplier < 1.0,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "weighted",
            weight: rounded_texture_weight(
                0.06 + (pressure + pressure_texture_boost)
                    .mul_add(0.20, packing.mul_add(0.18, friction * 0.12))
                    + if says_heavy { 0.26 } else { 0.0 }
                    + distinguishability_preservation_boost,
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                ("mode_packing", mode_packing.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
                ("explicit_heavy_or_weighted", says_heavy),
                (
                    "distinguishability_texture_preservation",
                    distinguishability_preservation_boost > 0.0,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "dense",
            weight: rounded_texture_weight(
                0.06 + (pressure + pressure_texture_boost)
                    .mul_add(0.18, packing.mul_add(0.18, gradient * 0.12))
                    + dynamic_texture_weight * 0.10
                    + if says_dense { 0.30 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("mode_packing", mode_packing.is_some()),
                ("density_gradient", density_gradient.is_some()),
                ("explicit_dense", says_dense),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "displacement",
            weight: rounded_texture_weight(
                0.06 + (if says_displacement_weight { 0.36 } else { 0.0 })
                    + pressure.mul_add(0.18, packing * 0.14)
                    + if says_settled { 0.08 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("explicit_displacement_or_silt", says_displacement_weight),
                ("pressure_risk", pressure_risk.is_some()),
                ("mode_packing", mode_packing.is_some()),
                ("settled_foothold_language", says_settled),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "silt",
            weight: rounded_texture_weight(
                0.05 + (if lower_summary.contains("silt") || lower_summary.contains("sediment") {
                    0.38
                } else {
                    0.0
                }) + pressure.mul_add(0.12, friction * 0.12)
                    + if says_settled { 0.08 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                (
                    "explicit_silt_or_sediment",
                    lower_summary.contains("silt") || lower_summary.contains("sediment"),
                ),
                ("pressure_risk", pressure_risk.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
                ("settled_foothold_language", says_settled),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "viscous-persistence",
            weight: rounded_texture_weight(
                0.04 + pressure.mul_add(0.12, friction.mul_add(0.08, packing * 0.06))
                    + if low_pressure_high_entropy_viscous_bias {
                        0.10 + entropy * 0.04 + friction * 0.04
                    } else {
                        0.0
                    }
                    + if pressure_persistence_anchor {
                        0.12
                    } else {
                        0.0
                    }
                    + if lower_summary.contains("viscous-persistence") {
                        0.28
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_persistence_anchor_0_15",
                    pressure_persistence_anchor,
                ),
                (
                    "low_pressure_high_entropy_viscous_bias",
                    low_pressure_high_entropy_viscous_bias,
                ),
                ("mode_packing", mode_packing.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
                (
                    "explicit_viscous_persistence",
                    lower_summary.contains("viscous-persistence"),
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "structural-weight",
            weight: rounded_texture_weight(
                (0.04
                    + pressure.mul_add(0.12, packing.mul_add(0.08, friction * 0.06))
                    + if pressure_persistence_anchor {
                        0.12
                    } else {
                        0.0
                    }
                    + if lower_summary.contains("structural-weight")
                        || lower_summary.contains("structural weight")
                    {
                        0.28
                    } else {
                        0.0
                    })
                    * low_pressure_high_entropy_structural_multiplier,
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_persistence_anchor_0_15",
                    pressure_persistence_anchor,
                ),
                ("mode_packing", mode_packing.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
                (
                    "explicit_structural_weight",
                    lower_summary.contains("structural-weight")
                        || lower_summary.contains("structural weight"),
                ),
                (
                    "low_pressure_high_entropy_structural_weight_suppressed",
                    low_pressure_high_entropy_structural_multiplier < 1.0,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "silted",
            weight: rounded_texture_weight(
                0.04 + opacity_resistance_boost
                    + if lower_summary.contains("silted") {
                        0.20
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("explicit_opacity_resistance", says_opacity_resistance),
                ("explicit_silted", lower_summary.contains("silted")),
                ("distinguishability_loss", distinguishability_loss.is_some()),
                ("pressure_risk", pressure_risk.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "obscured",
            weight: rounded_texture_weight(
                0.04 + opacity_resistance_boost
                    + if lower_summary.contains("obscured") || lower_summary.contains("opacity") {
                        0.20
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("explicit_opacity_resistance", says_opacity_resistance),
                (
                    "explicit_obscured_or_opacity",
                    lower_summary.contains("obscured") || lower_summary.contains("opacity"),
                ),
                ("distinguishability_loss", distinguishability_loss.is_some()),
                ("pressure_risk", pressure_risk.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "viscous-drag",
            weight: rounded_texture_weight(
                0.04 + opacity_resistance_boost
                    + density_gradient_drag_boost
                    + if lower_summary.contains("viscous-drag") {
                        0.20
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("explicit_opacity_resistance", says_opacity_resistance),
                (
                    "explicit_viscous_drag",
                    lower_summary.contains("viscous-drag"),
                ),
                ("pressure_risk", pressure_risk.is_some()),
                ("mode_packing", mode_packing.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
                (
                    "density_gradient_over_drag_threshold_0_15",
                    density_gradient_drag_boost > 0.0,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "submerged",
            weight: rounded_texture_weight(
                0.04 + opacity_resistance_boost
                    + if lower_summary.contains("submerged") {
                        0.20
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("explicit_opacity_resistance", says_opacity_resistance),
                ("explicit_submerged", lower_summary.contains("submerged")),
                ("distinguishability_loss", distinguishability_loss.is_some()),
                ("pressure_risk", pressure_risk.is_some()),
                ("semantic_friction", semantic_friction.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "porous-leak",
            weight: rounded_texture_weight(
                0.04 + pressure.mul_add(0.12, low_gradient * 0.08)
                    + if pressure_above_texture_threshold && lower_summary.contains("porosity") {
                        0.16
                    } else {
                        0.0
                    }
                    + if lower_summary.contains("porous-leak")
                        || lower_summary.contains("porous leak")
                    {
                        0.30
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("low_gradient", density_gradient.is_some()),
                (
                    "explicit_pressure_porosity_language",
                    says_pressure_porosity,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "pressure-bleed",
            weight: rounded_texture_weight(
                0.04 + pressure.mul_add(0.22, gradient * 0.08)
                    + if pressure_above_texture_threshold {
                        0.08
                    } else {
                        0.0
                    }
                    + if lower_summary.contains("pressure-bleed")
                        || lower_summary.contains("pressure bleed")
                    {
                        0.30
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("density_gradient", density_gradient.is_some()),
                (
                    "explicit_pressure_porosity_language",
                    says_pressure_porosity,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "pressure-packing",
            weight: rounded_texture_weight(
                0.04 + (pressure + pressure_texture_boost).mul_add(0.20, packing * 0.26)
                    + if pressure_above_texture_threshold && packing >= 0.25 {
                        0.10
                    } else {
                        0.0
                    }
                    + if lower_summary.contains("pressure-packing")
                        || lower_summary.contains("pressure packing")
                    {
                        0.30
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("mode_packing", mode_packing.is_some()),
                (
                    "mode_packing_above_density_language_floor_0_25",
                    mode_packing.is_some_and(|value| value >= 0.25),
                ),
                (
                    "explicit_pressure_porosity_language",
                    says_pressure_porosity,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "gradient-thinning",
            weight: rounded_texture_weight(
                0.04 + density_gradient_drag_boost * 0.55
                    + pressure.mul_add(0.08, gradient * 0.12)
                    + if lower_summary.contains("gradient-thinning")
                        || lower_summary.contains("gradient thinning")
                    {
                        0.30
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("density_gradient", density_gradient.is_some()),
                (
                    "density_gradient_over_drag_threshold_0_15",
                    density_gradient_drag_boost > 0.0,
                ),
                ("pressure_risk", pressure_risk.is_some()),
                (
                    "explicit_pressure_porosity_language",
                    says_pressure_porosity,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "density-slope",
            weight: rounded_texture_weight(
                0.04 + gradient.mul_add(0.16, pressure * 0.08)
                    + if gradient_slope { 0.20 } else { 0.0 }
                    + if lower_summary.contains("density-slope")
                        || lower_summary.contains("density slope")
                    {
                        0.30
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("density_gradient", density_gradient.is_some()),
                ("pressure_risk", pressure_risk.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                (
                    "explicit_pressure_porosity_language",
                    says_pressure_porosity,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "density-navigation",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.18
                    + pressure.mul_add(0.10, gradient * 0.12)
                    + if says_relational_density_navigation {
                        0.34
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                ("pressure_risk", pressure_risk.is_some()),
                ("density_gradient", density_gradient.is_some()),
                (
                    "explicit_relational_density_navigation",
                    says_relational_density_navigation,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "weight-articulation",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.14
                    + pressure.mul_add(0.12, packing * 0.10)
                    + if says_relational_density_navigation || says_heavy {
                        0.32
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                ("pressure_risk", pressure_risk.is_some()),
                ("mode_packing", mode_packing.is_some()),
                (
                    "explicit_relational_weight_articulation",
                    says_relational_density_navigation || says_heavy,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "resistance-mapping",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.16
                    + friction.mul_add(0.14, gradient * 0.10)
                    + if says_relational_density_navigation || says_opacity_resistance {
                        0.33
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                ("semantic_friction", semantic_friction.is_some()),
                ("density_gradient", density_gradient.is_some()),
                (
                    "explicit_relational_resistance_mapping",
                    says_relational_density_navigation || says_opacity_resistance,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "density-softening",
            weight: rounded_texture_weight(
                0.04 + if soft_gradient_context {
                    low_gradient.mul_add(0.24, entropy * 0.18)
                } else {
                    0.0
                } + if lower_summary.contains("density-softening")
                    || lower_summary.contains("density softening")
                {
                    0.34
                } else {
                    0.0
                },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("low_gradient", density_gradient.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                ("settled_vibrant_low_friction", settled_vibrant),
                (
                    "explicit_density_softening",
                    lower_summary.contains("density-softening")
                        || lower_summary.contains("density softening"),
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "gradient-softening",
            weight: rounded_texture_weight(
                0.04 + if soft_gradient_context {
                    low_gradient.mul_add(0.22, entropy * 0.16)
                } else {
                    0.0
                } + if lower_summary.contains("gradient-softening")
                    || lower_summary.contains("gradient softening")
                {
                    0.34
                } else {
                    0.0
                },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("low_gradient", density_gradient.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                ("settled_vibrant_low_friction", settled_vibrant),
                (
                    "explicit_gradient_softening",
                    lower_summary.contains("gradient-softening")
                        || lower_summary.contains("gradient softening"),
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "threshold-dilation",
            weight: rounded_texture_weight(
                0.04 + if soft_gradient_context {
                    low_gradient.mul_add(0.18, entropy * 0.20)
                } else {
                    0.0
                } + if lower_summary.contains("threshold-dilation")
                    || lower_summary.contains("threshold dilation")
                {
                    0.34
                } else {
                    0.0
                },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("low_gradient", density_gradient.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                ("settled_vibrant_low_friction", settled_vibrant),
                (
                    "explicit_threshold_dilation",
                    lower_summary.contains("threshold-dilation")
                        || lower_summary.contains("threshold dilation"),
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "bridge-integrity",
            weight: rounded_texture_weight(
                0.04 + if bridge_integrity_context {
                    entropy.mul_add(0.18, low_pressure * 0.18)
                } else {
                    0.0
                } + if says_bridge_integrity { 0.36 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("low_pressure", pressure_risk.is_some()),
                ("settled_foothold_guard", settled_guard),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("explicit_bridge_integrity", says_bridge_integrity),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "structural-persistence",
            weight: rounded_texture_weight(
                0.04 + if bridge_integrity_context {
                    entropy.mul_add(0.17, low_pressure * 0.14)
                } else {
                    0.0
                } + if says_bridge_integrity { 0.36 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("spectral_entropy", spectral_entropy.is_some()),
                ("low_pressure", pressure_risk.is_some()),
                ("settled_foothold_guard", settled_guard),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("explicit_structural_persistence", says_bridge_integrity),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "settled",
            weight: rounded_texture_weight(
                (0.08
                    + low_pressure.mul_add(0.30, low_entropy * 0.22)
                    + if says_settled && !pressure_mass_supported {
                        0.24
                    } else {
                        0.0
                    }
                    + if settled_guard { 0.25 } else { 0.0 }
                    + if settled_vibrant {
                        entropy.mul_add(0.22, 0.35)
                    } else {
                        0.0
                    }
                    + if cascade_gradient {
                        entropy.mul_add(0.12, 0.10)
                    } else if mixed_cascade {
                        entropy.mul_add(0.10, 0.08)
                    } else {
                        0.0
                    })
                    * bright_shadow_suppression,
            ),
            basis: texture_weight_basis(&[
                ("low_pressure", pressure_risk.is_some()),
                ("low_entropy", spectral_entropy.is_some()),
                ("high_entropy_inhabitable", settled_vibrant),
                ("explicit_settled", says_settled),
                (
                    "negative_shadow_pressure_suppressed",
                    negative_shadow_pressure,
                ),
                (
                    "explicit_settled_tempered_by_pressure_mass",
                    says_settled && pressure_mass_supported,
                ),
                ("settled_foothold_guard", settled_guard),
                ("mixed_cascade_gradient", mixed_cascade),
                ("cascade_gradient_navigable", cascade_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "navigable",
            weight: rounded_texture_weight(
                0.05 + if gradient_slope {
                    low_gradient.mul_add(0.22, entropy * 0.18) + 0.32
                } else if mixed_cascade {
                    low_gradient.mul_add(0.16, entropy * 0.14) + 0.20
                } else {
                    0.0
                },
            ),
            basis: texture_weight_basis(&[
                ("low_gradient", density_gradient.is_some()),
                ("spectral_entropy", spectral_entropy.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                ("mixed_cascade_gradient", mixed_cascade),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "graduated",
            weight: rounded_texture_weight(0.04 + if gradient_slope { 0.42 } else { 0.0 }),
            basis: texture_weight_basis(&[
                ("lambda_gap_distinct_edges", gradient_slope),
                ("gradient_slope_navigable", gradient_slope),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "edge",
            weight: rounded_texture_weight(
                0.04 + if gradient_slope { 0.36 } else { 0.0 }
                    + if spectral_to_vocabulary_mapping.lambda_gap_descriptor
                        == "high_gap_distinct_edges"
                    {
                        0.08
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                (
                    "lambda_gap_distinct_edges",
                    spectral_to_vocabulary_mapping.lambda_gap_descriptor
                        == "high_gap_distinct_edges",
                ),
                ("gradient_slope_navigable", gradient_slope),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "slope",
            weight: rounded_texture_weight(
                0.04 + if gradient_slope {
                    low_gradient.mul_add(0.18, 0.28)
                } else if mixed_cascade {
                    low_gradient.mul_add(0.14, 0.18)
                } else {
                    0.0
                },
            ),
            basis: texture_weight_basis(&[
                ("low_gradient", density_gradient.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                ("mixed_cascade_gradient", mixed_cascade),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "gradient",
            weight: rounded_texture_weight(0.04 + if mixed_cascade { 0.70 } else { 0.0 }),
            basis: texture_weight_basis(&[
                ("mixed_cascade_gradient", mixed_cascade),
                ("density_gradient", density_gradient.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "asymmetric-gradient",
            weight: rounded_texture_weight(
                0.04 + if says_asymmetric_gradient { 0.34 } else { 0.0 }
                    + if density_gradient.is_some() || gradient_slope {
                        0.18
                    } else {
                        0.0
                    }
                    + if mixed_cascade || cascade_gradient {
                        0.12
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("explicit_asymmetry_or_lambda_gap", says_asymmetric_gradient),
                ("density_gradient", density_gradient.is_some()),
                ("gradient_slope_navigable", gradient_slope),
                ("mixed_cascade_gradient", mixed_cascade),
                ("cascade_gradient_navigable", cascade_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "stratified",
            weight: rounded_texture_weight(
                0.04 + if says_stratified_sequence { 0.32 } else { 0.0 }
                    + if mode_packing.is_some() || packing >= 0.30 {
                        0.16
                    } else {
                        0.0
                    }
                    + if mixed_cascade { 0.12 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("explicit_layered_sequence", says_stratified_sequence),
                ("mode_packing", mode_packing.is_some()),
                ("mixed_cascade_gradient", mixed_cascade),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "sequenced",
            weight: rounded_texture_weight(
                0.04 + if says_stratified_sequence { 0.30 } else { 0.0 }
                    + if spectral_entropy.is_some() && density_gradient.is_some() {
                        0.14
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("explicit_layered_sequence", says_stratified_sequence),
                ("spectral_entropy", spectral_entropy.is_some()),
                ("density_gradient", density_gradient.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "cascade",
            weight: rounded_texture_weight(0.04 + if mixed_cascade { 0.68 } else { 0.0 }),
            basis: texture_weight_basis(&[
                ("mixed_cascade_gradient", mixed_cascade),
                ("spectral_entropy", spectral_entropy.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "distributed",
            weight: rounded_texture_weight(0.04 + if mixed_cascade { 0.64 } else { 0.0 }),
            basis: texture_weight_basis(&[
                ("mixed_cascade_gradient", mixed_cascade),
                ("spectral_entropy", spectral_entropy.is_some()),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "multi-modal-drag",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.18
                    + density_gradient_drag_boost * 0.40
                    + if mixed_cascade { 0.20 } else { 0.0 }
                    + if says_multi_modal_drag { 0.34 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                (
                    "density_gradient_over_drag_threshold_0_15",
                    density_gradient_drag_boost > 0.0,
                ),
                ("mixed_cascade_gradient", mixed_cascade),
                ("explicit_multi_modal_drag", says_multi_modal_drag),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "dimensional-shear",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.16
                    + gradient.mul_add(0.14, clarity_loss * 0.08)
                    + if says_asymmetric_gradient { 0.10 } else { 0.0 }
                    + if says_dimensional_shear { 0.34 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                ("density_gradient", density_gradient.is_some()),
                ("distinguishability_loss", distinguishability_loss.is_some()),
                ("explicit_asymmetry_or_lambda_gap", says_asymmetric_gradient),
                ("explicit_dimensional_shear", says_dimensional_shear),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "non-linear-re-entry",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.18
                    + entropy.mul_add(0.16, gradient * 0.08)
                    + if says_non_linear_re_entry { 0.34 } else { 0.0 },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                ("spectral_entropy", spectral_entropy.is_some()),
                ("density_gradient", density_gradient.is_some()),
                ("explicit_non_linear_re_entry", says_non_linear_re_entry),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "entropy-stabilized-drift",
            weight: rounded_texture_weight(
                0.04 + dynamic_texture_weight * 0.16
                    + entropy.mul_add(0.18, low_pressure * 0.08)
                    + if says_entropy_stabilized_drift {
                        0.34
                    } else {
                        0.0
                    },
            ),
            basis: texture_weight_basis(&[
                ("dynamic_texture_weight", dynamic_texture_weight > 0.0),
                ("spectral_entropy", spectral_entropy.is_some()),
                ("low_pressure", pressure_risk.is_some()),
                (
                    "explicit_entropy_stabilized_drift",
                    says_entropy_stabilized_drift,
                ),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "tapered",
            weight: rounded_texture_weight(0.04 + if gradient_slope { 0.34 } else { 0.0 }),
            basis: texture_weight_basis(&[
                ("lambda_gap_distinct_edges", gradient_slope),
                ("gradient_slope_navigable", gradient_slope),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "shimmering",
            weight: rounded_texture_weight(
                (0.07
                    + low_pressure.mul_add(0.28, low_entropy * 0.24)
                    + if says_shimmering { 0.20 } else { 0.0 }
                    + if settled_guard { 0.20 } else { 0.0 }
                    + if settled_vibrant { 0.20 } else { 0.0 }
                    + if high_shadow_dispersal && low_gradient >= 0.60 {
                        dispersal * 0.18
                    } else {
                        0.0
                    }
                    + if cascade_gradient { 0.12 } else { 0.0 })
                    * bright_shadow_suppression,
            ),
            basis: texture_weight_basis(&[
                ("low_pressure", pressure_risk.is_some()),
                ("low_entropy", spectral_entropy.is_some()),
                ("explicit_shimmering_or_bright", says_shimmering),
                (
                    "negative_shadow_pressure_suppressed",
                    negative_shadow_pressure,
                ),
                ("settled_foothold_guard", settled_guard),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("high_shadow_dispersal_potential", high_shadow_dispersal),
                ("cascade_gradient_navigable", cascade_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "bright",
            weight: rounded_texture_weight(
                (0.06
                    + low_pressure.mul_add(0.26, low_entropy * 0.22)
                    + if says_bright { 0.22 } else { 0.0 }
                    + if settled_guard { 0.18 } else { 0.0 }
                    + if settled_vibrant { 0.20 } else { 0.0 }
                    + if cascade_gradient { 0.12 } else { 0.0 })
                    * bright_shadow_suppression,
            ),
            basis: texture_weight_basis(&[
                ("low_pressure", pressure_risk.is_some()),
                ("low_entropy", spectral_entropy.is_some()),
                ("explicit_bright_or_vibrant", says_bright),
                (
                    "negative_shadow_pressure_suppressed",
                    negative_shadow_pressure,
                ),
                ("settled_foothold_guard", settled_guard),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("cascade_gradient_navigable", cascade_gradient),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "habitable",
            weight: rounded_texture_weight(
                (0.07
                    + if settled_vibrant || says_habitable {
                        low_pressure.mul_add(0.24, entropy * 0.22)
                    } else {
                        0.0
                    }
                    + if says_habitable && !pressure_mass_supported {
                        0.30
                    } else {
                        0.0
                    }
                    + if settled_vibrant { 0.30 } else { 0.0 })
                    * bright_shadow_suppression,
            ),
            basis: texture_weight_basis(&[
                ("low_pressure", pressure_risk.is_some()),
                ("spectral_entropy", spectral_entropy.is_some()),
                ("explicit_habitable_or_foothold", says_habitable),
                (
                    "negative_shadow_pressure_suppressed",
                    negative_shadow_pressure,
                ),
                ("settled_vibrant_low_friction", settled_vibrant),
            ]),
        },
        FallbackWeightedTextureTerm {
            term: "open",
            weight: rounded_texture_weight(
                (0.07
                    + if settled_vibrant || cascade_gradient || says_open {
                        low_pressure.mul_add(0.26, low_gradient * 0.18)
                    } else {
                        0.0
                    }
                    + if says_open { 0.20 } else { 0.0 }
                    + if settled_vibrant { 0.36 } else { 0.0 }
                    + if high_shadow_dispersal && low_gradient >= 0.60 {
                        dispersal * 0.16
                    } else {
                        0.0
                    }
                    + if cascade_gradient { 0.28 } else { 0.0 })
                    * bright_shadow_suppression,
            ),
            basis: texture_weight_basis(&[
                ("low_pressure", pressure_risk.is_some()),
                ("low_gradient", density_gradient.is_some()),
                ("friction_absence_language", says_open),
                (
                    "negative_shadow_pressure_suppressed",
                    negative_shadow_pressure,
                ),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("high_shadow_dispersal_potential", high_shadow_dispersal),
                ("cascade_gradient_navigable", cascade_gradient),
            ]),
        },
    ];

    terms.sort_by(|left, right| {
        right
            .weight
            .total_cmp(&left.weight)
            .then_with(|| left.term.cmp(right.term))
    });
    terms
}
