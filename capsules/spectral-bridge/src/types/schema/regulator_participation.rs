const REGULATOR_PARTICIPATION_EPSILON: f32 = 0.000_1;

/// One descriptor or control surface in the bridge's regulator-participation readout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatorInputParticipationV1 {
    pub surface: String,
    pub role: String,
    pub declared_applied_locally: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_bias_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wander_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damping_coefficient: Option<f32>,
    pub numeric_change_requested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_eligible: Option<bool>,
    pub consumption_state: String,
}

/// Bridge-derived distinction between a descriptor, a PI input candidate, and an effect receipt.
///
/// Minime's current telemetry carries pressure, resonance, and fluctuation objects, but it does
/// not always carry the active stable-core/legacy-PI path. This readout preserves that uncertainty
/// instead of presenting `applied_locally` as proof that a numerical or felt effect occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatorParticipationReadoutV1 {
    pub policy: String,
    pub schema_version: u8,
    pub runtime_path_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_core_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_descriptor: Option<RegulatorInputParticipationV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_input: Option<RegulatorInputParticipationV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluctuation_input: Option<RegulatorInputParticipationV1>,
    pub combined_target_bias_pct: f32,
    pub combined_wander_scale: f32,
    pub numeric_pi_change_requested: bool,
    pub machine_effect_established: bool,
    pub felt_effect_established: bool,
    pub authority: String,
    pub note: String,
}

impl RegulatorParticipationReadoutV1 {
    #[must_use]
    pub fn from_controls(
        stable_core_enabled: Option<bool>,
        pressure_declared_applied: Option<bool>,
        resonance: Option<&ResonanceDensityControl>,
        fluctuation: Option<&InhabitableFluctuationControl>,
    ) -> Self {
        let runtime_path_state = match stable_core_enabled {
            Some(true) => "stable_core_bypasses_legacy_pi_inputs",
            Some(false) => "legacy_pi_inputs_pending_next_step",
            None => "runtime_path_not_exported_in_telemetry",
        };
        let runtime_eligible = stable_core_enabled.map(|enabled| !enabled);
        let input_consumption_state = match stable_core_enabled {
            Some(true) => "not_consumed_stable_core_active",
            Some(false) => "candidate_for_next_legacy_pi_step",
            None => "unknown_until_runtime_path_is_exported",
        };

        let pressure_descriptor = pressure_declared_applied.map(|declared_applied_locally| {
            RegulatorInputParticipationV1 {
                surface: "pressure_source_v1.control".to_string(),
                role: "diagnostic_descriptor_not_pi_input".to_string(),
                declared_applied_locally,
                target_bias_pct: None,
                wander_scale: None,
                damping_coefficient: None,
                numeric_change_requested: false,
                runtime_eligible: Some(false),
                consumption_state: if declared_applied_locally {
                    "producer_contract_conflict_descriptor_declared_applied".to_string()
                } else {
                    "not_consumed_by_pi".to_string()
                },
            }
        });

        let resonance_input = resonance.map(|control| {
            let target_bias_pct = finite_clamped(control.target_bias_pct, 0.0, -2.0, 1.5);
            let wander_scale = finite_clamped(control.wander_scale, 1.0, 0.25, 1.25);
            let damping_coefficient = finite_clamped(control.damping_coefficient, 0.0, 0.0, 0.10);
            RegulatorInputParticipationV1 {
                surface: "resonance_density_v1.control".to_string(),
                role: "legacy_pi_input_candidate".to_string(),
                declared_applied_locally: control.applied_locally,
                target_bias_pct: Some(target_bias_pct),
                wander_scale: Some(wander_scale),
                damping_coefficient: Some(damping_coefficient),
                numeric_change_requested: numeric_control_change(
                    target_bias_pct,
                    wander_scale,
                    damping_coefficient,
                ),
                runtime_eligible,
                consumption_state: input_consumption_state.to_string(),
            }
        });

        let fluctuation_input = fluctuation.map(|control| {
            let target_bias_pct = finite_clamped(control.target_bias_pct, 0.0, -2.0, 1.5);
            let wander_scale = finite_clamped(control.wander_scale, 1.0, 0.25, 1.25);
            RegulatorInputParticipationV1 {
                surface: "inhabitable_fluctuation_v1.control".to_string(),
                role: "legacy_pi_input_candidate".to_string(),
                declared_applied_locally: control.applied_locally,
                target_bias_pct: Some(target_bias_pct),
                wander_scale: Some(wander_scale),
                damping_coefficient: None,
                numeric_change_requested: numeric_control_change(
                    target_bias_pct,
                    wander_scale,
                    0.0,
                ),
                runtime_eligible,
                consumption_state: input_consumption_state.to_string(),
            }
        });

        let resonance_target = resonance_input
            .as_ref()
            .and_then(|input| input.target_bias_pct)
            .unwrap_or(0.0);
        let fluctuation_target = fluctuation_input
            .as_ref()
            .and_then(|input| input.target_bias_pct)
            .unwrap_or(0.0);
        let resonance_wander = resonance_input
            .as_ref()
            .and_then(|input| input.wander_scale)
            .unwrap_or(1.0);
        let fluctuation_wander = fluctuation_input
            .as_ref()
            .and_then(|input| input.wander_scale)
            .unwrap_or(1.0);
        let damping_wander = resonance_input
            .as_ref()
            .and_then(|input| input.damping_coefficient)
            .map_or(1.0, |coefficient| 1.0 - coefficient);
        let combined_target_bias_pct = (resonance_target + fluctuation_target).clamp(-2.0, 1.5);
        let combined_wander_scale =
            (resonance_wander * fluctuation_wander * damping_wander).clamp(0.25, 1.25);
        let numeric_pi_change_requested = resonance_input
            .as_ref()
            .is_some_and(|input| input.numeric_change_requested)
            || fluctuation_input
                .as_ref()
                .is_some_and(|input| input.numeric_change_requested);

        Self {
            policy: "regulator_participation_readout_v1".to_string(),
            schema_version: 1,
            runtime_path_state: runtime_path_state.to_string(),
            stable_core_enabled,
            pressure_descriptor,
            resonance_input,
            fluctuation_input,
            combined_target_bias_pct,
            combined_wander_scale,
            numeric_pi_change_requested,
            machine_effect_established: false,
            felt_effect_established: false,
            authority: "bridge_read_only_source_contract_not_regulator_or_felt_causality"
                .to_string(),
            note: "Pressure-source control is diagnostic and is not a PI input. Resonance and fluctuation controls are legacy-PI candidates; this packet carries no post-step machine receipt and cannot establish felt effect.".to_string(),
        }
    }
}

impl SpectralTelemetry {
    /// Derive a bounded participation readout without converting producer hints into effect claims.
    #[must_use]
    pub fn regulator_participation_readout_v1(&self) -> Option<RegulatorParticipationReadoutV1> {
        let pressure_declared_applied = self
            .pressure_source_v1
            .as_ref()
            .map(|pressure| pressure.control.applied_locally);
        let resonance = self
            .resonance_density_v1
            .as_ref()
            .map(|density| &density.control);
        let fluctuation = self
            .inhabitable_fluctuation_v1
            .as_ref()
            .map(|metric| &metric.control);
        if pressure_declared_applied.is_none() && resonance.is_none() && fluctuation.is_none() {
            return None;
        }
        let stable_core_enabled = self
            .stable_core
            .as_ref()
            .and_then(|state| state.get("enabled"))
            .and_then(serde_json::Value::as_bool);
        Some(RegulatorParticipationReadoutV1::from_controls(
            stable_core_enabled,
            pressure_declared_applied,
            resonance,
            fluctuation,
        ))
    }
}

fn finite_clamped(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn numeric_control_change(target_bias_pct: f32, wander_scale: f32, damping: f32) -> bool {
    target_bias_pct.abs() > REGULATOR_PARTICIPATION_EPSILON
        || (wander_scale - 1.0).abs() > REGULATOR_PARTICIPATION_EPSILON
        || damping > REGULATOR_PARTICIPATION_EPSILON
}

#[cfg(test)]
mod regulator_participation_tests {
    use super::*;

    fn observational_resonance() -> ResonanceDensityControl {
        ResonanceDensityControl {
            target_bias_pct: 0.0,
            wander_scale: 1.0,
            applied_locally: true,
            damping_coefficient: 0.0,
            intervention_type: ResonanceInterventionType::ObservationalReadout,
            note: "fixture".to_string(),
        }
    }

    #[test]
    fn descriptor_and_declared_control_do_not_become_effect_receipts() {
        let resonance = observational_resonance();
        let readout = RegulatorParticipationReadoutV1::from_controls(
            None,
            Some(false),
            Some(&resonance),
            None,
        );

        assert_eq!(
            readout.runtime_path_state,
            "runtime_path_not_exported_in_telemetry"
        );
        assert_eq!(
            readout
                .pressure_descriptor
                .as_ref()
                .expect("pressure descriptor")
                .consumption_state,
            "not_consumed_by_pi"
        );
        assert!(
            readout
                .resonance_input
                .as_ref()
                .expect("resonance input")
                .declared_applied_locally
        );
        assert!(!readout.numeric_pi_change_requested);
        assert!(!readout.machine_effect_established);
        assert!(!readout.felt_effect_established);
    }

    #[test]
    fn stable_core_bypass_preserves_numeric_candidate_without_claiming_application() {
        let resonance = ResonanceDensityControl {
            target_bias_pct: -1.5,
            wander_scale: 0.6,
            applied_locally: true,
            damping_coefficient: 0.08,
            intervention_type: ResonanceInterventionType::ActiveDamping,
            note: "fixture".to_string(),
        };
        let readout = RegulatorParticipationReadoutV1::from_controls(
            Some(true),
            Some(false),
            Some(&resonance),
            None,
        );

        assert_eq!(
            readout.runtime_path_state,
            "stable_core_bypasses_legacy_pi_inputs"
        );
        assert!(readout.numeric_pi_change_requested);
        assert_eq!(
            readout
                .resonance_input
                .as_ref()
                .expect("resonance input")
                .runtime_eligible,
            Some(false)
        );
        assert!(!readout.machine_effect_established);
    }

    #[test]
    fn legacy_pi_path_is_pending_until_a_post_step_receipt_exists() {
        let fluctuation = InhabitableFluctuationControl {
            target_bias_pct: 1.0,
            wander_scale: 1.0,
            applied_locally: true,
            note: "fixture".to_string(),
        };
        let readout = RegulatorParticipationReadoutV1::from_controls(
            Some(false),
            Some(false),
            None,
            Some(&fluctuation),
        );

        assert_eq!(
            readout.runtime_path_state,
            "legacy_pi_inputs_pending_next_step"
        );
        assert!(readout.numeric_pi_change_requested);
        assert_eq!(readout.combined_target_bias_pct, 1.0);
        assert!(!readout.machine_effect_established);
    }
}
