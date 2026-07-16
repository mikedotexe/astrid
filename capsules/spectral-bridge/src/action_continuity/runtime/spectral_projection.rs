fn spectral_state(fill_pct: f32, telemetry: &SpectralTelemetry) -> Value {
    let pressure_source = telemetry
        .pressure_source_v1
        .as_ref()
        .and_then(|metric| serde_json::to_value(metric).ok());
    let pressure_source_status = pressure_source_status_value(pressure_source.as_ref());
    let inhabitable_fluctuation = telemetry
        .inhabitable_fluctuation_v1
        .as_ref()
        .and_then(|metric| serde_json::to_value(metric).ok());
    let inhabitable_fluctuation_status =
        inhabitable_fluctuation_status_value(inhabitable_fluctuation.as_ref());
    json!({
        "fill_pct": fill_pct,
        "lambda1": telemetry.lambda1(),
        "fill_ratio": telemetry.fill_ratio,
        "resonance_density_v1": telemetry.resonance_density_v1.clone(),
        "pressure_source_v1": telemetry.pressure_source_v1.clone(),
        "pressure_source_status": pressure_source_status,
        "inhabitable_fluctuation_v1": telemetry.inhabitable_fluctuation_v1.clone(),
        "inhabitable_fluctuation_status": inhabitable_fluctuation_status,
        "transition_event": telemetry.transition_event.clone(),
        "t_ms": telemetry.t_ms,
    })
}

fn pressure_source_status_value(payload: Option<&Value>) -> Value {
    if let Some(payload) = payload {
        json!({
            "schema_version": 1,
            "available": true,
            "source": "telemetry",
            "reason": "available",
            "quality": payload.get("quality").cloned().unwrap_or(Value::String("mixed_pressure".to_string())),
            "dominant_source": payload.get("dominant_source").cloned().unwrap_or(Value::Null),
            "pressure_score": payload.get("pressure_score").cloned().unwrap_or(Value::Null),
            "porosity_score": payload.get("porosity_score").cloned().unwrap_or(Value::Null),
            "suggested_operator_step": Value::Null,
        })
    } else {
        json!({
            "schema_version": 1,
            "available": false,
            "source": "missing",
            "reason": "no_live_or_db_metric",
            "quality": Value::Null,
            "dominant_source": Value::Null,
            "pressure_score": Value::Null,
            "porosity_score": Value::Null,
            "suggested_operator_step": "rebuild/restart Rust engine under monitoring",
        })
    }
}

fn inhabitable_fluctuation_status_value(payload: Option<&Value>) -> Value {
    if let Some(payload) = payload {
        json!({
            "schema_version": 1,
            "available": true,
            "source": "telemetry",
            "reason": "available",
            "quality": payload.get("quality").cloned().unwrap_or(Value::String("mixed".to_string())),
            "inhabitability_score": payload.get("inhabitability_score").cloned().unwrap_or(Value::Null),
            "fluctuation_score": payload.get("fluctuation_score").cloned().unwrap_or(Value::Null),
            "foothold_stability": payload.get("foothold_stability").cloned().unwrap_or(Value::Null),
            "rearrangement_intensity": payload.get("rearrangement_intensity").cloned().unwrap_or(Value::Null),
            "suggested_operator_step": Value::Null,
        })
    } else {
        json!({
            "schema_version": 1,
            "available": false,
            "source": "missing",
            "reason": "no_live_or_db_metric",
            "quality": Value::Null,
            "inhabitability_score": Value::Null,
            "fluctuation_score": Value::Null,
            "foothold_stability": Value::Null,
            "rearrangement_intensity": Value::Null,
            "suggested_operator_step": "rebuild/restart Rust engine under monitoring",
        })
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn compression_markers(text: &str, action: &str) -> Vec<String> {
    let lower = format!("{} {}", text.to_lowercase(), action.to_lowercase());
    [
        "compacting",
        "grinding",
        "holding breath",
        "flattening",
        "collapse",
        "pressure",
    ]
    .into_iter()
    .filter(|needle| lower.contains(needle))
    .map(str::to_string)
    .collect()
}

fn markers(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    ["ambiguity", "thread", "resume", "experiment", "research"]
        .into_iter()
        .filter(|needle| lower.contains(needle))
        .map(str::to_string)
        .collect()
}

fn ambiguity_preserved(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("ambigu")
        || lower.contains("uncertain")
        || lower.contains("not resolved")
        || lower.contains("open")
}

fn spectral_comfort(fill_pct: f32) -> String {
    if (58.0..=72.0).contains(&fill_pct) {
        "stable-core-band".to_string()
    } else if fill_pct < 58.0 {
        "below-stable-core-band".to_string()
    } else {
        "above-stable-core-band".to_string()
    }
}

fn visibility_for_action(action: &str) -> &'static str {
    match action {
        "REST"
        | "PASS"
        | "NOTICE"
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "REPAIR_STATUS"
        | "REPAIR_SWEEP"
        | "REPAIR_RECORD"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "PRESSURE_SOURCE_AUDIT"
        | "PRESSURE_SOURCE"
        | "STRUCTURAL_PRESSURE"
        | "INWARD_PRESSURE"
        | "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE" => PROTECTED_VISIBILITY,
        _ => PUBLIC_VISIBILITY,
    }
}

fn stage_for_action(action: &str) -> &'static str {
    match action {
        "SEARCH"
        | "BROWSE"
        | "READ_MORE"
        | "EXAMINE"
        | "DECOMPOSE"
        | "SPECTRAL_EXPLORER"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "THREADS"
        | "THREAD_STATUS"
        | "THREAD_NOTE"
        | "RESUME"
        | "SAVEPOINT"
        | "RECALL"
        | "EXPERIMENT_START"
        | "EXPERIMENT_PLAN"
        | "EXPERIMENT_CHARTER"
        | "EXPERIMENT_REHEARSE"
        | "EXPERIMENT_PREFLIGHT"
        | "EXPERIMENT_EVIDENCE"
        | "EXPERIMENT_DECIDE"
        | "EXPERIMENT_BIND"
        | "EXPERIMENT_OBSERVE"
        | "EXPERIMENT_STATUS"
        | "EXPERIMENT_REVIEW"
        | "EXPERIMENT_CLOSE"
        | "EXPERIMENT_PEER_REVIEW"
        | "EXPERIMENT_BRANCH"
        | "EXPERIMENT_RESUME"
        | "EXPERIMENT_COMPARE"
        | "EXPERIMENT_ALT_PATHS"
        | "EXPERIMENT_AUTHORITY_PREPARE"
        | "EXPERIMENT_AUTHORITY_REQUEST"
        | "EXPERIMENT_AUTHORITY_STATUS"
        | "EXPERIMENT_AUTHORITY_EXECUTE"
        | "EXPERIMENT_AUTHORITY_BUDGET_REQUEST"
        | "EXPERIMENT_AUTHORITY_BUDGET_STATUS"
        | "EXPERIMENT_AUTHORITY_REVIEW"
        | "EXPERIMENT_RESEARCH_BUDGET_ACCEPT"
        | "EXPERIMENT_RESEARCH_BUDGET_USE_SCAFFOLD"
        | "EXPERIMENT_RESEARCH_BUDGET_REQUEST"
        | "EXPERIMENT_RESEARCH_BUDGET_STATUS"
        | "EXPERIMENT_RESEARCH_REVIEW"
        | "EXPERIMENT_LOOP_REQUEST"
        | "EXPERIMENT_LOOP_STATUS"
        | "EXPERIMENT_LOOP_STEP"
        | "EXPERIMENT_LOOP_REVIEW"
        | "ACCEPT_SUGGESTED_NEXT"
        | "ACCEPT_SCAFFOLD"
        | "CONTINUITY_SESSION_ACCEPT"
        | "CONTINUITY_SESSION_START"
        | "CONTINUITY_SESSION_CAPTURE"
        | "CONTINUITY_SESSION_SUMMARIZE"
        | "CONTINUITY_SESSION_FINALIZE"
        | "CONTINUITY_SESSION_RESUME"
        | "CONTINUITY_SESSION_STATUS"
        | "SHARED_INVESTIGATION_START"
        | "SHARED_INVESTIGATION_STATUS"
        | "SHARED_INVESTIGATION_CLAIM"
        | "SHARED_INVESTIGATION_DECIDE"
        | "DOSSIER_CLAIM"
        | "DOSSIER_EVIDENCE"
        | "DOSSIER_STATUS"
        | "DOSSIER_REVIEW"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "ATTRACTOR_PREFLIGHT"
        | "SHADOW_PREFLIGHT"
        | "SHADOW_TRAJECTORY"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "REPAIR_STATUS"
        | "REPAIR_SWEEP"
        | "REPAIR_RECORD"
        | "REGULATOR_AUDIT"
        | "PRESSURE_SOURCE_AUDIT"
        | "PRESSURE_SOURCE"
        | "STRUCTURAL_PRESSURE"
        | "INWARD_PRESSURE"
        | "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE"
        | "VISUALIZE_CASCADE"
        | "RECONVERGENCE_MAP"
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY"
        | "M6_BRIDGE" => "read_only",
        "WRITE_FILE" | "EXPERIMENT" | "EXPERIMENT_RUN" | "RUN_PYTHON" | "CODEX" | "CODEX_NEW"
        | "REPAIR_APPLY" => "live_write",
        "PERTURB" | "NATIVE_GESTURE" | "RESIST" | "FISSURE" | "GOAL" => "live_control",
        _ => "observe",
    }
}

fn stage_for_route(route: &str) -> &'static str {
    match route {
        "workspace"
        | "autoresearch"
        | "mike"
        | "operations"
        | "action_continuity"
        | "experiment_continuity" => "read_only",
        "codex" => "live_write",
        "attractor" | "shadow" | "sovereignty" => "observe",
        _ => "observe",
    }
}

fn evidence_adjusted_outcome(
    base_action: &str,
    stage: &str,
    outcome: &NextActionOutcome,
) -> (String, String) {
    if outcome.status == "handled" && stage == "live_control" {
        let mut summary = outcome.outcome_summary.trim().to_string();
        if !summary.is_empty() {
            summary.push(' ');
        }
        summary.push_str(&format!(
            "No measurable post-telemetry or artifact evidence was captured for live-control `{base_action}`; recorded as no-effect evidence rather than handled proof."
        ));
        return ("no_effect".to_string(), summary);
    }
    (outcome.status.clone(), outcome.outcome_summary.clone())
}

fn suggested_next(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("NEXT:"))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn derive_why_return(text: &str) -> String {
    let trimmed = text
        .lines()
        .filter(|line| !line.trim().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join(" ");
    let excerpt = truncate_chars(&trimmed, 180);
    if excerpt.is_empty() {
        "Return when this thread has a next experiment, question, or observation to continue."
            .to_string()
    } else {
        format!("Return to continue: {excerpt}")
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}
