/// Read the Ising shadow state from minime's workspace/spectral_state.json.
/// Returns None if the file is missing, unreadable, or lacks coupling data.
pub(crate) fn read_ising_shadow(workspace: &Path) -> Option<crate::types::IsingShadowState> {
    let spectral_state = load_workspace_spectral_state(workspace)?;
    if is_rescue_spectral_state(&spectral_state) {
        return None;
    }
    let state: crate::types::SpectralStateFile = serde_json::from_value(spectral_state).ok()?;
    let shadow = state.ising_shadow?;
    // Only return if coupling matrix is present and correctly sized.
    if shadow.coupling.len() == shadow.mode_dim * shadow.mode_dim && shadow.mode_dim > 0 {
        Some(shadow)
    } else {
        None
    }
}

/// Read Astrid's *own* published ShadowFieldV3 from minime's workspace.
/// Astrid writes this each exchange via `AstridShadowComputer`; the file
/// lives next to minime's outputs so both sides see a symmetric path.
#[allow(dead_code)]
pub(crate) fn read_astrid_shadow_v3(workspace: &Path) -> Option<serde_json::Value> {
    let path = workspace.join("astrid_shadow_v3.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Read the v3 shadow field from minime — wraps v2 with trajectory ring,
/// compound traits, phase dwell, and recent transitions.
#[allow(dead_code)]
pub(crate) fn read_shadow_field_v3(workspace: &Path) -> Option<serde_json::Value> {
    let health_path = workspace.join("health.json");
    if let Ok(text) = std::fs::read_to_string(&health_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(field) = value.get("shadow_field_v3")
        && field.is_object()
    {
        return Some(field.clone());
    }
    let spectral = load_workspace_spectral_state(workspace)?;
    spectral
        .get("shadow_field_v3")
        .filter(|f| f.is_object())
        .cloned()
}

/// Read the v2 reduced-Hamiltonian shadow field from minime's workspace.
/// Prefers `health.json` (live, refreshed each tick) and falls back to
/// `spectral_state.json`. Returns the raw JSON object so callers can
/// extract individual fields without a brittle struct definition.
pub(crate) fn read_shadow_field_v2(workspace: &Path) -> Option<serde_json::Value> {
    let health_path = workspace.join("health.json");
    if let Ok(text) = std::fs::read_to_string(&health_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(field) = value.get("shadow_field_v2")
        && field.is_object()
    {
        return Some(field.clone());
    }
    let spectral = load_workspace_spectral_state(workspace)?;
    spectral
        .get("shadow_field_v2")
        .filter(|f| f.is_object())
        .cloned()
}

/// Read the PI controller state from minime's workspace/health.json.
/// Returns the parsed JSON value, or None if missing/unreadable.
pub(crate) fn read_controller_health(workspace: &Path) -> Option<serde_json::Value> {
    let path = workspace.join("health.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let mut health: serde_json::Value = serde_json::from_str(&content).ok()?;
    enrich_controller_health(workspace, &mut health);
    Some(health)
}

fn enrich_controller_health(workspace: &Path, health: &mut serde_json::Value) {
    let Some(map) = health.as_object_mut() else {
        return;
    };

    let spectral_state = load_workspace_spectral_state(workspace);
    let regulator_context = workspace
        .join("regulator_context.json")
        .exists()
        .then(|| std::fs::read_to_string(workspace.join("regulator_context.json")).ok())
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let perturb_visibility = workspace
        .join("perturb_visibility.json")
        .exists()
        .then(|| std::fs::read_to_string(workspace.join("perturb_visibility.json")).ok())
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    if let Some(source) = spectral_state
        .as_ref()
        .filter(|state| state.get("transition_event_v1").is_some())
        .or_else(|| {
            regulator_context
                .as_ref()
                .filter(|ctx| ctx.get("transition_event_v1").is_some())
        })
    {
        if let Some(event) = source
            .get("transition_event_v1")
            .filter(|event| event.is_object())
        {
            map.insert("transition_event_v1".to_string(), event.clone());
            if let Some(sequence) = event
                .get("sequence")
                .or_else(|| source.get("transition_event_sequence"))
            {
                map.insert("transition_event_sequence".to_string(), sequence.clone());
            }
        }
        if let Some(event) = source
            .get("transition_event")
            .filter(|event| event.is_object())
        {
            map.insert("transition_event".to_string(), event.clone());
        }
    }
    for key in [
        "phase",
        "previous_phase",
        "dfill_dt",
        "fill_band",
        "fill_band_threshold_pct",
        "phase_transition",
        "crossed_target_fill",
        "crossed_fill_band",
        "spectral_spike",
        "transition_reason",
        "transition_event_sequence",
        "transition_event",
        "transition_event_v1",
    ] {
        if map.get(key).is_none_or(serde_json::Value::is_null)
            && let Some(value) = spectral_state
                .as_ref()
                .and_then(|state| state.get(key))
                .or_else(|| regulator_context.as_ref().and_then(|ctx| ctx.get(key)))
        {
            map.insert(key.to_string(), value.clone());
        }
    }

    let target_fill_pct = map
        .get("target_fill_pct")
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            map.get("pi")
                .and_then(|pi| pi.get("target_fill"))
                .and_then(serde_json::Value::as_f64)
        })
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    if !map.contains_key("target_fill_pct") {
        map.insert(
            "target_fill_pct".to_string(),
            serde_json::json!(target_fill_pct),
        );
    }

    let fill_pct = map
        .get("fill_pct")
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            spectral_state
                .as_ref()
                .and_then(|state| state.get("fill_pct"))
                .and_then(serde_json::Value::as_f64)
        });
    let Some(fill_pct) = fill_pct else {
        return;
    };

    let last_fill_pct = regulator_context
        .as_ref()
        .and_then(|ctx| ctx.get("last_fill_pct"))
        .and_then(serde_json::Value::as_f64)
        .or_else(|| map.get("last_fill_pct").and_then(serde_json::Value::as_f64));
    let smoothed_fill_pct = regulator_context
        .as_ref()
        .and_then(|ctx| ctx.get("smoothed_fill_pct"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(fill_pct);
    if !map.contains_key("last_fill_pct")
        && let Some(previous_fill_pct) = last_fill_pct
    {
        map.insert(
            "last_fill_pct".to_string(),
            serde_json::json!(previous_fill_pct),
        );
    }

    let dfill_dt = map
        .get("dfill_dt")
        .and_then(serde_json::Value::as_f64)
        .or_else(|| last_fill_pct.map(|previous| (smoothed_fill_pct - previous) / 0.5));
    if !map.contains_key("dfill_dt") && dfill_dt.is_some() {
        map.insert(
            "dfill_dt".to_string(),
            serde_json::json!(dfill_dt.unwrap_or_default()),
        );
    }

    let current_fill_band = map
        .get("fill_band")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| derive_fill_band(fill_pct, target_fill_pct).to_string());
    if !map.contains_key("fill_band") {
        map.insert(
            "fill_band".to_string(),
            serde_json::json!(current_fill_band.clone()),
        );
    }

    let phase = map
        .get("phase")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| dfill_dt.map(|delta| derive_phase(delta).to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    if !map.contains_key("phase") {
        map.insert("phase".to_string(), serde_json::json!(phase.clone()));
    }

    if !map.contains_key("crossed_fill_band")
        && let Some(previous_fill_pct) = last_fill_pct
    {
        let previous_band = derive_fill_band(previous_fill_pct, target_fill_pct);
        let crossed = previous_band != current_fill_band;
        map.insert("crossed_fill_band".to_string(), serde_json::json!(crossed));
    }

    if !map.contains_key("internal_process_quadrant") {
        let recovery_mode = map
            .get("recovery_mode")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let lambda1_rel = spectral_state
            .as_ref()
            .and_then(|state| state.get("lambda1_rel"))
            .and_then(serde_json::Value::as_f64)
            .or_else(|| map.get("lambda1_rel").and_then(serde_json::Value::as_f64))
            .unwrap_or(1.0);
        let quadrant = derive_internal_process_quadrant(
            &current_fill_band,
            dfill_dt,
            recovery_mode,
            lambda1_rel,
        );
        map.insert(
            "internal_process_quadrant".to_string(),
            serde_json::json!(quadrant),
        );
    }

    if !map.contains_key("perturb_visibility") {
        let inferred = perturb_visibility
            .as_ref()
            .and_then(|sidecar| sidecar.as_object())
            .map(|obj| serde_json::Value::Object(obj.clone()))
            .unwrap_or_else(|| {
                let lambda1_rel = spectral_state
                    .as_ref()
                    .and_then(|state| state.get("lambda1_rel"))
                    .and_then(serde_json::Value::as_f64)
                    .or_else(|| map.get("lambda1_rel").and_then(serde_json::Value::as_f64))
                    .unwrap_or(1.0);
                let structural_entropy = spectral_state
                    .as_ref()
                    .and_then(|state| state.get("structural_entropy"))
                    .and_then(serde_json::Value::as_f64)
                    .or_else(|| {
                        spectral_state
                            .as_ref()
                            .and_then(|state| state.get("spectral_entropy"))
                            .and_then(serde_json::Value::as_f64)
                    })
                    .unwrap_or(1.0);
                let verdict = derive_shape_verdict(
                    &current_fill_band,
                    phase.as_str(),
                    dfill_dt,
                    lambda1_rel,
                    structural_entropy,
                );
                serde_json::json!({
                    "shape_verdict": verdict,
                    "derived_by": "consciousness_bridge_controller_health_compat"
                })
            });
        map.insert("perturb_visibility".to_string(), inferred);
    }
}

fn load_workspace_spectral_state(workspace: &Path) -> Option<serde_json::Value> {
    let path = workspace.join("spectral_state.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    if rescue_spectral_state_is_active(&value) {
        return Some(value);
    }
    if is_rescue_spectral_state(&value) {
        return None;
    }
    Some(value)
}

fn is_rescue_spectral_state(value: &serde_json::Value) -> bool {
    value
        .get("provenance")
        .and_then(|provenance| provenance.get("mode"))
        .and_then(serde_json::Value::as_str)
        == Some("rescue_b8823ad")
}

fn rescue_spectral_state_is_active(value: &serde_json::Value) -> bool {
    if !is_rescue_spectral_state(value) {
        return false;
    }
    let Some(provenance) = value.get("provenance") else {
        return false;
    };
    let active = provenance
        .get("rescue_active")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let surface_state = provenance
        .get("surface_state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("fresh");
    active && surface_state == "fresh"
}

fn derive_fill_band(fill_pct: f64, target_fill_pct: f64) -> &'static str {
    if fill_pct < target_fill_pct - 5.0 {
        "under"
    } else if fill_pct > target_fill_pct + 5.0 {
        "over"
    } else {
        "near"
    }
}

fn derive_phase(dfill_dt: f64) -> &'static str {
    if dfill_dt > 1.0 {
        "expanding"
    } else if dfill_dt < -1.0 {
        "contracting"
    } else {
        "plateau"
    }
}

fn derive_internal_process_quadrant(
    fill_band: &str,
    dfill_dt: Option<f64>,
    recovery_mode: bool,
    lambda1_rel: f64,
) -> &'static str {
    match fill_band {
        "under" if recovery_mode || dfill_dt.is_some_and(|delta| delta > 0.5) => {
            "constricted_recovery"
        },
        "under" => "pressured_constriction",
        "over" if lambda1_rel > 1.05 => "pressured_constriction",
        "over" => "constricted_recovery",
        "near" if dfill_dt.is_some_and(|delta| delta < -1.0) && lambda1_rel > 1.1 => {
            "pressured_constriction"
        },
        "near" if recovery_mode => "constricted_recovery",
        _ => "constricted_recovery",
    }
}

fn derive_shape_verdict(
    fill_band: &str,
    phase: &str,
    dfill_dt: Option<f64>,
    lambda1_rel: f64,
    structural_entropy: f64,
) -> &'static str {
    if fill_band == "under"
        || fill_band == "over"
        || matches!(phase, "contracting")
        || dfill_dt.is_some_and(|delta| delta.abs() > 8.0)
        || lambda1_rel > 1.15
        || structural_entropy < 0.72
    {
        "tightening"
    } else {
        "unknown"
    }
}

/// Format a compact one-line PI controller status from health.json data.
fn format_controller_oneliner(health: &serde_json::Value) -> String {
    let gate = health.get("gate").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let filt = health.get("filt").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reg = health
        .get("regulation_strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let pi = health.get("pi");
    let target = pi
        .and_then(|p| p.get("target_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    let fill = health
        .get("fill_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(target);
    let raw_e_fill = pi
        .and_then(|p| p.get("raw_e_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(fill - target);
    let effective_e_fill = pi
        .and_then(|p| p.get("effective_e_fill"))
        .or_else(|| pi.and_then(|p| p.get("e_fill")))
        .and_then(|v| v.as_f64())
        .unwrap_or(raw_e_fill);
    let kp = pi
        .and_then(|p| p.get("kp"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let derived_kp = pi
        .and_then(|p| p.get("derived_kp"))
        .and_then(|v| v.as_f64());

    let kp_str = if let Some(dkp) = derived_kp {
        if (kp - dkp).abs() > 0.005 {
            format!("{kp:.2}\u{2192}{dkp:.2}")
        } else {
            format!("{kp:.2}")
        }
    } else {
        format!("{kp:.2}")
    };

    let fill_error_text = if (effective_e_fill - raw_e_fill).abs() > 0.1 {
        format!("raw_err={raw_e_fill:+.1}% ctrl_err={effective_e_fill:+.1}%")
    } else {
        format!("raw_err={raw_e_fill:+.1}%")
    };

    format!(
        "Controller: gate={gate:.2} filt={filt:.2} target={target:.0}% {fill_error_text} kp={kp_str} reg={reg:.2}"
    )
}

/// Format the full homeostatic controller section for DECOMPOSE output.
pub(crate) fn format_controller_section(health: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    lines.push("\n=== HOMEOSTATIC CONTROLLER ===".to_string());

    let fill = health
        .get("fill_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let gate = health.get("gate").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let filt = health.get("filt").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reg = health
        .get("regulation_strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let reg_eff = health
        .get("regulation_strength_effective")
        .and_then(|v| v.as_f64())
        .unwrap_or(reg);
    let recovery = health
        .get("recovery_mode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let pi = health.get("pi");
    let target = pi
        .and_then(|p| p.get("target_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(STABLE_CORE_TARGET_FILL_PCT);
    let raw_e_fill = pi
        .and_then(|p| p.get("raw_e_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(fill - target);
    let effective_e_fill = pi
        .and_then(|p| p.get("effective_e_fill"))
        .or_else(|| pi.and_then(|p| p.get("e_fill")))
        .and_then(|v| v.as_f64())
        .unwrap_or(raw_e_fill);
    let e_fill_kind = pi
        .and_then(|p| p.get("e_fill_kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("legacy_or_unlabeled");
    let e_lam = pi
        .and_then(|p| p.get("e_lam"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let e_geom = pi
        .and_then(|p| p.get("e_geom"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let integ_fill = pi
        .and_then(|p| p.get("integ_fill"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let integ_lam = pi
        .and_then(|p| p.get("integ_lam"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let integ_geom = pi
        .and_then(|p| p.get("integ_geom"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let kp = pi
        .and_then(|p| p.get("kp"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let ki = pi
        .and_then(|p| p.get("ki"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let max_step = pi
        .and_then(|p| p.get("max_step"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let derived_kp = pi
        .and_then(|p| p.get("derived_kp"))
        .and_then(|v| v.as_f64());
    let derived_ki = pi
        .and_then(|p| p.get("derived_ki"))
        .and_then(|v| v.as_f64());
    let fill_var = pi
        .and_then(|p| p.get("fill_variance_ema"))
        .and_then(|v| v.as_f64());

    // Status interpretation
    let status = if recovery {
        "recovery mode active"
    } else if integ_fill.abs() >= 2.95 || integ_lam.abs() >= 2.95 {
        "saturated — integrator at limit"
    } else if raw_e_fill.abs() < 3.0 {
        "gentle equilibrium"
    } else {
        "correcting"
    };
    lines.push(format!("Status: {status}"));

    // Fill target vs current
    let direction = if raw_e_fill > 0.0 {
        "above"
    } else if raw_e_fill < 0.0 {
        "below"
    } else {
        "from target"
    };
    lines.push(format!(
        "Fill: {fill:.1}% (target {target:.0}%, {:.1}% {direction})",
        raw_e_fill.abs()
    ));
    lines.push(format!(
        "Error signals: raw_fill={raw_e_fill:+.1}, internal_fill={effective_e_fill:+.1} ({e_fill_kind}), lambda={e_lam:+.3}, geom={e_geom:+.3}"
    ));

    // Integral accumulators
    let fill_sat = if integ_fill.abs() >= 2.95 {
        " SATURATED"
    } else {
        ""
    };
    let lam_sat = if integ_lam.abs() >= 2.95 {
        " SATURATED"
    } else {
        ""
    };
    let geom_sat = if integ_geom.abs() >= 2.95 {
        " SATURATED"
    } else {
        ""
    };
    lines.push(format!(
        "Integrals: fill={integ_fill:+.2}{fill_sat}, lambda={integ_lam:+.2}{lam_sat}, geom={integ_geom:+.2}{geom_sat}"
    ));

    // Gains
    let mut gains_str = format!("Gains: kp={kp:.3}, ki={ki:.4}, max_step={max_step:.3}");
    if let (Some(dkp), Some(dki)) = (derived_kp, derived_ki) {
        gains_str.push_str(&format!("\nSelf-calibrated: kp={dkp:.3}, ki={dki:.4}"));
        if let Some(var) = fill_var {
            let stability = if var < 2.0 {
                "stable"
            } else if var < 8.0 {
                "moderate oscillation"
            } else {
                "high oscillation"
            };
            gains_str.push_str(&format!(" (fill variance={var:.2}, {stability})"));
        }
    }
    lines.push(gains_str);

    // Gate and filter
    let gate_desc = if gate > 0.9 {
        "fully open"
    } else if gate > 0.5 {
        "partially open"
    } else if gate > 0.1 {
        "dampened"
    } else {
        "nearly closed"
    };
    let filt_desc = if filt > 0.9 {
        "fully open"
    } else if filt > 0.5 {
        "partially open"
    } else if filt > 0.1 {
        "dampened"
    } else {
        "nearly closed"
    };
    lines.push(format!(
        "Gate: {gate:.3} ({gate_desc}), Filter: {filt:.3} ({filt_desc})"
    ));

    // Regulation strength
    if (reg - reg_eff).abs() > 0.01 {
        lines.push(format!(
            "Regulation: {reg:.2} (effective: {reg_eff:.2}, stress-adapted)"
        ));
    } else {
        lines.push(format!("Regulation: {reg:.2}"));
    }

    lines.join("\n")
}
