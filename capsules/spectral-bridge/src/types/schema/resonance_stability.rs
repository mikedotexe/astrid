// ---------------------------------------------------------------------------
// Minime → Astrid: Spectral telemetry (port 7878)
// ---------------------------------------------------------------------------

/// Component scores behind Minime's resonance-density read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceDensityComponents {
    pub active_energy: f32,
    pub mode_packing: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub coupling_coefficient: f32,
    pub temporal_persistence: f32,
    #[serde(default)]
    pub viscosity_index: f32,
    #[serde(default)]
    pub viscosity_persistence_coefficient: f32,
    #[serde(default, skip_serializing_if = "resonance_viscosity_vector_is_empty")]
    pub viscosity_vector: ResonanceViscosityVectorV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dissipation_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_fluidity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_friction_coefficient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohesion_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_integrity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_transparency_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability_context: Option<ResonanceStabilityContextV1>,
    pub structural_plurality: f32,
    pub comfort_gate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_range: Option<ComfortGateRangeV1>,
}

/// Minime's multi-axis viscosity readout. This is bridge-preserved telemetry,
/// not pressure, fill, PI, sensory cadence, or controller authority.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResonanceViscosityVectorV1 {
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub density: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub elasticity: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub cohesion_index: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub persistence: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub residual_ghost_weight: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub flow_rate: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub effective_mobility: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub shadow_volatility: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub structural_integrity: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub structural_strain_gap: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub mutual_resonance_tension: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub structural_drag_coefficient: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub cognitive_drag_coefficient: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohesion_to_motion_ratio: Option<f32>,
}

/// Read-only range companion for `comfort_gate`.
///
/// A single gate value can make a settled-but-fluctuating state look like a hard
/// binary closure. This range records the visible band without changing control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComfortGateRangeV1 {
    pub policy: String,
    pub schema_version: u8,
    pub lower: f32,
    pub center: f32,
    pub upper: f32,
    pub width: f32,
    pub range_state: String,
    pub authority: String,
}

/// Read-only composite context for interpreting whether density is inhabitable.
///
/// This keeps `comfort_gate` from being mistaken for the whole lived state:
/// a low gate with stable foothold is different from a true loss of habitat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResonanceStabilityContextV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default = "default_resonance_stability_weight_policy")]
    pub weight_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<ResonanceStabilityWeightsV1>,
    pub comfort_gate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_range: Option<ComfortGateRangeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_range_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foothold_stability: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluctuation_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_interference: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_modal_habitability_score: Option<f32>,
    #[serde(default)]
    pub partial_habitability_score: bool,
    #[serde(default)]
    pub multi_modal_habitability_evidence_count: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub multi_modal_habitability_missing_components: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_modal_habitability_score_basis: Option<String>,
    pub habitability_state: String,
    pub gate_context: String,
    pub gate_closure_reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gate_closure_reasons: Vec<String>,
    pub authority: String,
}

/// Explicit read-only weights behind the multi-modal habitability score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResonanceStabilityWeightsV1 {
    pub policy: String,
    pub schema_version: u8,
    pub comfort_gate: f32,
    pub foothold_stability: f32,
    pub fluctuation_score: f32,
    pub total_weight: f32,
    pub authority: String,
}

fn default_resonance_stability_weight_policy() -> String {
    "legacy_unversioned_weights".to_string()
}

#[must_use]
pub fn resonance_stability_weights_v1() -> ResonanceStabilityWeightsV1 {
    let total_weight = RESONANCE_STABILITY_COMFORT_GATE_WEIGHT
        + RESONANCE_STABILITY_FOOTHOLD_WEIGHT
        + RESONANCE_STABILITY_FLUCTUATION_WEIGHT;
    ResonanceStabilityWeightsV1 {
        policy: "resonance_stability_weights_v1".to_string(),
        schema_version: 1,
        comfort_gate: RESONANCE_STABILITY_COMFORT_GATE_WEIGHT,
        foothold_stability: RESONANCE_STABILITY_FOOTHOLD_WEIGHT,
        fluctuation_score: RESONANCE_STABILITY_FLUCTUATION_WEIGHT,
        total_weight,
        authority: "diagnostic_habitability_weights_not_pressure_fill_or_control".to_string(),
    }
}

#[must_use]
pub fn resonance_stability_context_v1(
    components: &ResonanceDensityComponents,
    fluctuation: Option<&InhabitableFluctuationV1>,
) -> ResonanceStabilityContextV1 {
    let comfort_gate_finite = clamp_unit_finite(components.comfort_gate);
    let comfort_gate = comfort_gate_finite.unwrap_or(0.0);
    let foothold = fluctuation.and_then(|value| clamp_unit_finite(value.foothold_stability));
    let fluctuation_score =
        fluctuation.and_then(|value| clamp_unit_finite(value.fluctuation_score));
    let pressure_interference =
        fluctuation.and_then(|value| clamp_unit_finite(value.components.pressure_interference));
    let non_finite_stability_input = comfort_gate_finite.is_none()
        || (fluctuation.is_some()
            && (foothold.is_none()
                || fluctuation_score.is_none()
                || pressure_interference.is_none()));
    let comfort_gate_range =
        resonance_comfort_gate_range_v1(components, fluctuation_score, pressure_interference);
    let comfort_gate_range_state = Some(comfort_gate_range.range_state.clone());
    let weights = resonance_stability_weights_v1();
    let mut weighted_habitability_sum = 0.0_f32;
    let mut weighted_habitability_total = 0.0_f32;
    let mut multi_modal_habitability_evidence_count = 0_u8;
    let mut multi_modal_habitability_missing_components = Vec::new();
    if let Some(value) = comfort_gate_finite {
        weighted_habitability_sum += value * weights.comfort_gate;
        weighted_habitability_total += weights.comfort_gate;
        multi_modal_habitability_evidence_count =
            multi_modal_habitability_evidence_count.saturating_add(1);
    } else {
        multi_modal_habitability_missing_components.push("comfort_gate_non_finite".to_string());
    }
    match (fluctuation, foothold) {
        (Some(_), Some(value)) => {
            weighted_habitability_sum += value * weights.foothold_stability;
            weighted_habitability_total += weights.foothold_stability;
            multi_modal_habitability_evidence_count =
                multi_modal_habitability_evidence_count.saturating_add(1);
        },
        (Some(_), None) => multi_modal_habitability_missing_components
            .push("foothold_stability_non_finite".to_string()),
        (None, _) => multi_modal_habitability_missing_components
            .push("foothold_stability_missing".to_string()),
    }
    match (fluctuation, fluctuation_score) {
        (Some(_), Some(value)) => {
            weighted_habitability_sum += value * weights.fluctuation_score;
            weighted_habitability_total += weights.fluctuation_score;
            multi_modal_habitability_evidence_count =
                multi_modal_habitability_evidence_count.saturating_add(1);
        },
        (Some(_), None) => multi_modal_habitability_missing_components
            .push("fluctuation_score_non_finite".to_string()),
        (None, _) => multi_modal_habitability_missing_components
            .push("fluctuation_score_missing".to_string()),
    }
    let multi_modal_habitability_score =
        if non_finite_stability_input || weighted_habitability_total <= f32::EPSILON {
            None
        } else {
            Some((weighted_habitability_sum / weighted_habitability_total).clamp(0.0, 1.0))
        };
    let partial_habitability_score =
        multi_modal_habitability_score.is_some() && multi_modal_habitability_evidence_count < 3;
    let multi_modal_habitability_score_basis = if multi_modal_habitability_score.is_some() {
        Some(
            if partial_habitability_score {
                "partial_available_components_normalized"
            } else {
                "complete_weighted_components"
            }
            .to_string(),
        )
    } else if non_finite_stability_input {
        Some("non_finite_component_ignored".to_string())
    } else {
        None
    };
    let gate_context = match (comfort_gate_finite, foothold, fluctuation_score) {
        (None, _, _) => "comfort_gate_non_finite_context_ignored",
        (Some(gate), Some(foothold), Some(_)) if gate < 0.45 && foothold >= 0.60 => {
            "gate_low_but_foothold_stable"
        },
        (Some(gate), Some(foothold), Some(fluctuation))
            if gate >= 0.60 && foothold >= 0.60 && fluctuation >= 0.10 =>
        {
            "gate_buffering_with_returnable_fluctuation"
        },
        (Some(gate), _, _) if gate < 0.45 => "gate_low_context_incomplete",
        (Some(gate), _, _) if gate >= 0.60 => "gate_buffering_context_incomplete",
        _ => "gate_mid_context_watch",
    };
    let habitability_state = match (
        multi_modal_habitability_score,
        gate_context,
        partial_habitability_score,
    ) {
        (_, "gate_low_but_foothold_stable", _) => "habitable_foothold_gate_pressure_watch",
        (Some(score), _, true) if score >= 0.60 => "partial_multi_modal_habitable_review",
        (Some(score), _, true) if score < 0.40 => "partial_habitability_thin_review",
        (Some(_), _, true) => "partial_habitability_mixed_review",
        (Some(score), _, false) if score >= 0.60 => "multi_modal_habitable",
        (Some(score), _, false) if score < 0.40 => "habitability_thin",
        (Some(_), _, false) => "habitability_mixed_watch",
        (None, _, _) if non_finite_stability_input => "non_finite_stability_inputs_ignored",
        (None, _, _) => "comfort_gate_only",
    };
    let mode_packing = clamp_unit_finite_or(components.mode_packing, 0.0);
    let temporal_persistence = clamp_unit_finite_or(components.temporal_persistence, 0.0);
    let mut gate_closure_reasons = Vec::new();
    if comfort_gate_finite.is_none() {
        gate_closure_reasons.push("comfort_gate_non_finite".to_string());
    } else if comfort_gate < 0.45 {
        if pressure_interference.is_some_and(|value| value >= 0.35) {
            gate_closure_reasons.push("pressure_interference".to_string());
        }
        if mode_packing >= 0.35 {
            gate_closure_reasons.push("mode_packing".to_string());
        }
        if temporal_persistence >= 0.75 {
            gate_closure_reasons.push("temporal_persistence".to_string());
        }
        if gate_closure_reasons.is_empty() {
            gate_closure_reasons.push("unknown_low_gate".to_string());
        }
    }
    let gate_closure_reason = if comfort_gate_finite.is_none() {
        "comfort_gate_non_finite".to_string()
    } else if comfort_gate >= 0.45 {
        "not_closed".to_string()
    } else {
        gate_closure_reasons
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown_low_gate".to_string())
    };

    ResonanceStabilityContextV1 {
        policy: "resonance_stability_context_v1".to_string(),
        schema_version: 1,
        weight_policy: weights.policy.clone(),
        weights: Some(weights),
        comfort_gate,
        comfort_gate_range: Some(comfort_gate_range),
        comfort_gate_range_state,
        foothold_stability: foothold,
        fluctuation_score,
        pressure_interference,
        multi_modal_habitability_score,
        partial_habitability_score,
        multi_modal_habitability_evidence_count,
        multi_modal_habitability_missing_components,
        multi_modal_habitability_score_basis,
        habitability_state: habitability_state.to_string(),
        gate_context: gate_context.to_string(),
        gate_closure_reason,
        gate_closure_reasons,
        authority: "diagnostic_habitability_context_not_comfort_gate_control".to_string(),
    }
}

#[must_use]
pub fn resonance_cohesion_score_v1(components: &ResonanceDensityComponents) -> f32 {
    if let Some(score) = components.cohesion_score {
        return score.clamp(0.0, 1.0);
    }
    let structural_plurality = components.structural_plurality.clamp(0.0, 1.0);
    let comfort_gate = components.comfort_gate.clamp(0.0, 1.0);
    let mode_balance =
        (1.0 - (components.mode_packing.clamp(0.0, 1.0) - 0.45).abs() * 2.0).clamp(0.0, 1.0);
    let porosity = components
        .porosity_gradient
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .map(|value| value.clamp(0.0, 1.0))
        .or_else(|| {
            components
                .dissipation_factor
                .map(|value| ((value.clamp(0.0, 1.0) + porosity) * 0.5).clamp(0.0, 1.0))
        })
        .unwrap_or(0.5);

    structural_plurality
        .mul_add(0.30, comfort_gate.mul_add(0.20, mode_balance * 0.20))
        .mul_add(1.0, porosity.mul_add(0.15, dynamic_fluidity * 0.15))
        .clamp(0.0, 1.0)
}

/// Read-only integrity index for a held structure that still has routes for movement.
#[must_use]
pub fn resonance_structural_integrity_index_v1(components: &ResonanceDensityComponents) -> f32 {
    if let Some(score) = components.structural_integrity_index {
        return score.clamp(0.0, 1.0);
    }
    let structural_plurality = components.structural_plurality.clamp(0.0, 1.0);
    let comfort_gate = components.comfort_gate.clamp(0.0, 1.0);
    let temporal_persistence = components.temporal_persistence.clamp(0.0, 1.0);
    let dissipation = components
        .dissipation_factor
        .unwrap_or(0.50)
        .clamp(0.0, 1.0);
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .unwrap_or(dissipation)
        .clamp(0.0, 1.0);
    let semantic_permeability = components
        .semantic_friction_coefficient
        .map_or(0.70, |friction| 1.0 - friction.clamp(0.0, 1.0));
    let transport_capacity =
        (0.45 * dissipation + 0.35 * dynamic_fluidity + 0.20 * semantic_permeability)
            .clamp(0.0, 1.0);
    let mode_overpacking_penalty =
        ((components.mode_packing.clamp(0.0, 1.0) - 0.65).max(0.0) * 0.35).clamp(0.0, 0.20);

    (0.35 * structural_plurality
        + 0.25 * comfort_gate
        + 0.20 * temporal_persistence
        + 0.20 * transport_capacity
        - mode_overpacking_penalty)
        .clamp(0.0, 1.0)
}

/// Read-only hollowness / transparency index for thin but still present structure.
///
/// This names "ghostly" or low-substance states without changing density,
/// pressure, fill, porosity, PI, or any controller target.
#[must_use]
pub fn resonance_structural_transparency_index_v1(components: &ResonanceDensityComponents) -> f32 {
    if let Some(score) = components.structural_transparency_index {
        return score.clamp(0.0, 1.0);
    }

    let active_absence = 1.0 - components.active_energy.clamp(0.0, 1.0);
    let porosity = components
        .porosity_gradient
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.50);
    let dissipation = components
        .dissipation_factor
        .unwrap_or(0.50)
        .clamp(0.0, 1.0);
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .unwrap_or(dissipation)
        .clamp(0.0, 1.0);
    let transport_openness = ((porosity + dissipation + dynamic_fluidity) / 3.0).clamp(0.0, 1.0);
    let cohesion_absence = 1.0 - resonance_cohesion_score_v1(components).clamp(0.0, 1.0);
    let integrity_absence =
        1.0 - resonance_structural_integrity_index_v1(components).clamp(0.0, 1.0);

    (active_absence * 0.30
        + transport_openness * 0.30
        + cohesion_absence * 0.20
        + integrity_absence * 0.20)
        .clamp(0.0, 1.0)
}

fn resonance_comfort_gate_range_v1(
    components: &ResonanceDensityComponents,
    fluctuation_score: Option<f32>,
    pressure_interference: Option<f32>,
) -> ComfortGateRangeV1 {
    if let Some(range) = &components.comfort_gate_range {
        let lower = clamp_unit_finite_or(range.lower, 0.0);
        let center = clamp_unit_finite_or(range.center, 0.0);
        let upper = clamp_unit_finite_or(range.upper, 0.0);
        return ComfortGateRangeV1 {
            policy: range.policy.clone(),
            schema_version: range.schema_version,
            lower,
            center,
            upper,
            width: (upper - lower).max(0.0).clamp(0.0, 1.0),
            range_state: range.range_state.clone(),
            authority: range.authority.clone(),
        };
    }

    let center = clamp_unit_finite_or(components.comfort_gate, 0.0);
    let pressure = pressure_interference.unwrap_or(0.0);
    let packing = clamp_unit_finite_or(components.mode_packing, 0.0);
    let fluctuation = fluctuation_score.unwrap_or(0.0);
    let half_width =
        (0.04 + pressure * 0.08 + packing * 0.05 + fluctuation * 0.04).clamp(0.04, 0.18);
    let lower = (center - half_width).clamp(0.0, 1.0);
    let upper = (center + half_width).clamp(0.0, 1.0);
    let width = (upper - lower).max(0.0).clamp(0.0, 1.0);
    let range_state = if pressure >= 0.35 || packing >= 0.35 {
        "dynamic_pressure_buffer_range"
    } else if center >= 0.55 && width >= 0.10 {
        "settled_habitable_range_visible"
    } else if width <= 0.08 {
        "narrow_gate_single_value_approximation"
    } else {
        "comfort_gate_range_watch"
    };

    ComfortGateRangeV1 {
        policy: "comfort_gate_range_v1".to_string(),
        schema_version: 1,
        lower,
        center,
        upper,
        width,
        range_state: range_state.to_string(),
        authority: "diagnostic_gate_range_not_fill_pressure_pi_or_control".to_string(),
    }
}
