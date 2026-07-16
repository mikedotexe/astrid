fn experiment_summary(record: &ExperimentRecord) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "experiment_id": record.experiment_id,
        "title": record.title,
        "question": record.question,
        "status": record.status,
        "planned_next": record.planned_next,
        "updated_at": record.updated_at,
        "parent_experiment_id": record.parent_experiment_id.clone(),
        "branch_refs": record.branch_refs.clone(),
        "motif_allowance_v1": record.motif_allowance_v1.clone(),
        "charter_v1": record.charter_v1.clone(),
        "evidence_v1": record.evidence_v1.clone(),
        "workbench_candidates_v1": record.workbench_candidates_v1.clone(),
        "workbench_charter": charter_status_text(record),
        "workbench_evidence": evidence_status_text(record),
        "workbench_candidates": workbench_candidate_status(record),
    })
}

fn last_experiment_summary_v1(thread: &ResearchThread) -> Option<Value> {
    let mut summary = thread.experiment_summary.clone()?;
    let object = summary.as_object_mut()?;
    let experiment_id = object
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if status == "paused" && !experiment_id.is_empty() {
        let (mut primary_return_next, mut return_kind) = paused_primary_return_v1(
            &experiment_id,
            object.get("planned_next").and_then(Value::as_str),
            object
                .get("primary_return_next")
                .or_else(|| object.get("resume_next"))
                .and_then(Value::as_str),
        );
        let mut projection_guard = None;
        if return_kind == "resume" && !lifecycle_valid_charter_value(object.get("charter_v1")) {
            primary_return_next = charter_repair_next_v1(&experiment_id);
            return_kind = "charter_repair".to_string();
            projection_guard = Some(json!({
                "schema_version": 1,
                "policy": "projection_guard_v1",
                "raw_next_preserved": true,
                "projected_next": primary_return_next.clone(),
                "return_kind": return_kind.clone(),
                "guardrail_reason": "paused_resume_missing_lifecycle_charter",
                "experiment_id": experiment_id.clone(),
                "authority_boundary": "Projection may redirect guidance only; it never applies, rehearses, binds, resumes, perturbs, sends control, or mutates peer experiments."
            }));
        } else if return_kind == "resume" {
            let raw_current_next = thread.current_next.as_deref().unwrap_or_default();
            let pressure_terms = projection_guard_pressure_terms_v1(raw_current_next);
            if !pressure_terms.is_empty() {
                primary_return_next = format!(
                    "EXPERIMENT_DECIDE {experiment_id} :: hold because repeated perturb-shaped planning is guard evidence, not progress"
                );
                return_kind = "hold".to_string();
                projection_guard = Some(json!({
                    "schema_version": 1,
                    "policy": "projection_guard_v1",
                    "raw_next_preserved": true,
                    "raw_next": raw_current_next,
                    "projected_next": primary_return_next.clone(),
                    "return_kind": return_kind.clone(),
                    "guardrail_reason": "paused_resume_demoted_by_liveish_pressure",
                    "pressure_terms": pressure_terms,
                    "experiment_id": experiment_id.clone(),
                    "authority_boundary": "Projection may redirect guidance only; it never applies, rehearses, binds, resumes, perturbs, sends control, or mutates peer experiments."
                }));
            }
        }
        object.insert(
            "primary_return_next".to_string(),
            json!(primary_return_next),
        );
        object.insert("return_kind".to_string(), json!(return_kind.clone()));
        if let Some(guard) = projection_guard {
            object.insert("projection_guard_v1".to_string(), guard);
            object.insert("raw_next_preserved".to_string(), json!(true));
        }
        if return_kind == "resume" {
            let primary = object
                .get("primary_return_next")
                .cloned()
                .unwrap_or_else(|| json!(format!("EXPERIMENT_RESUME {experiment_id}")));
            object.insert("resume_next".to_string(), primary);
        } else {
            object.remove("resume_next");
        }
    } else if matches!(status.as_str(), "complete" | "completed") && !experiment_id.is_empty() {
        object.entry("inspect_next".to_string()).or_insert_with(|| {
            json!(format!(
                "EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}"
            ))
        });
    }
    Some(summary)
}

fn charter_repair_next_v1(experiment_id: &str) -> String {
    format!(
        "EXPERIMENT_CHARTER {experiment_id} :: hypothesis: ...; method_intent: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ..."
    )
}

fn projection_guard_pressure_terms_v1(text: &str) -> Vec<&'static str> {
    let upper = text.to_ascii_uppercase();
    if !(base_action(text) == "EXPERIMENT_PLAN" || upper.contains("PROPOSED_NEXT_ACTION")) {
        return Vec::new();
    }
    let mut terms = Vec::new();
    for (needle, label) in [
        ("PERTURB", "PERTURB"),
        ("CONTROL", "CONTROL"),
        ("BIND", "BIND"),
        ("RESUME", "RESUME"),
        ("INFLUENCE", "INFLUENCE"),
        ("SEND_CONTROL", "SEND_CONTROL"),
        ("SEND CONTROL", "SEND_CONTROL"),
        ("INTERVENTION", "INTERVENTION"),
        ("PULSE", "PULSE"),
        ("SHIFT THE DOMINANT", "SHIFT_THE_DOMINANT"),
        ("SHIFT DOMINANT", "SHIFT_DOMINANT"),
    ] {
        if upper.contains(needle) && !terms.contains(&label) {
            terms.push(label);
        }
    }
    terms
}

fn paused_primary_return_v1(
    experiment_id: &str,
    planned_next: Option<&str>,
    fallback_next: Option<&str>,
) -> (String, String) {
    let candidate = planned_next
        .filter(|value| !value.trim().is_empty())
        .or_else(|| fallback_next.filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
        .trim()
        .to_string();
    match base_action(&candidate).as_str() {
        "EXPERIMENT_CHARTER" => (candidate, "charter_repair".to_string()),
        "EXPERIMENT_DECIDE" => (candidate, "decision".to_string()),
        "THREAD_STATUS" => (candidate, "hold".to_string()),
        "EXPERIMENT_ADVANCE" | "EXPERIMENT_CONVEYOR" => (candidate, "conveyor_preview".to_string()),
        "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT" => {
            (candidate, "rehearsal_ready".to_string())
        },
        "EXPERIMENT_RESUME" => (candidate, "resume".to_string()),
        _ => (
            if experiment_id.is_empty() {
                candidate
            } else {
                format!("EXPERIMENT_RESUME {experiment_id}")
            },
            "resume".to_string(),
        ),
    }
}

fn projection_current_next_display<'a>(
    projection: &'a ThreadContinuityProjection,
    fallback: Option<&'a str>,
) -> &'a str {
    if let Some(active) = projection.active_experiment.as_ref()
        && !active.continuity_return.trim().is_empty()
    {
        return active.continuity_return.as_str();
    }
    if let Some(summary) = projection.last_experiment_summary_v1.as_ref()
        && summary.get("status").and_then(Value::as_str) == Some("paused")
        && let Some(primary) = summary
            .get("primary_return_next")
            .or_else(|| summary.get("planned_next"))
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
    {
        return primary;
    }
    fallback.unwrap_or("(none yet)")
}

fn voice_health_line() -> String {
    let path = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/voice_health.json");
    let Ok(raw) = fs::read_to_string(&path) else {
        return String::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return String::new();
    };
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !matches!(status, "degraded_voice" | "single_fallback") {
        return String::new();
    }
    let count = value
        .get("fallback_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let repair = value
        .get("suggested_read_only_repair")
        .and_then(Value::as_str)
        .unwrap_or("REPAIR_STATUS or CAPABILITY_STATUS");
    let current_next = value
        .get("current_next")
        .and_then(Value::as_str)
        .unwrap_or("REPAIR_STATUS current");
    let hash = value
        .get("latest_fallback_hash")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!(
        "Voice health: {status} fallback_count={count} latest_fallback_hash={hash}. Current NEXT: {current_next}. Suggested repair: {repair}. Emergency fallback is presence, not ordinary dialogue.\n"
    )
}
