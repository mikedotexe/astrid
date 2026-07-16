fn parse_experiment_conveyor_request(raw: &str) -> (Option<String>, String) {
    let mut selector = raw.trim();
    let mut mode = "preview".to_string();
    if let Some((left, payload)) = raw.split_once("::") {
        selector = left.trim();
        if let Some(value) = dossier_field(payload, &["mode"]) {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized == "apply" {
                mode = "apply".to_string();
            }
        }
    }
    (
        optional_selector_owned(&normalize_experiment_selector(selector)),
        mode,
    )
}

fn experiment_conveyor_allowed_apply_steps() -> Value {
    json!([
        "lifecycle_valid_charter",
        "local_evidence_capture",
        "hold_decision",
        "charter_repair_decision"
    ])
}

fn experiment_conveyor_authority_boundary() -> &'static str {
    "EXPERIMENT_ADVANCE may preview freely or apply one conservative local charter, evidence, hold, or charter-repair step; it never rehearses automatically, binds, resumes, perturbs, sends control, or mutates peer experiments."
}

fn authority_gate_boundary() -> &'static str {
    "Being-authored request plus steward approval may mint one semantic_microdose token; V1 cannot bind, resume, perturb, send control, send attractor pulses, or mutate peers."
}

fn research_budget_boundary() -> &'static str {
    "Being-authored local-only requests may self-activate a bounded read_only_research budget; larger or web-enabled budgets still require steward approval. V1 cannot mutate autoresearch, bind, resume, perturb, send control, execute semantic authority, advance lifecycle, or mutate peers."
}

fn sovereign_loop_boundary() -> &'static str {
    "Being-owned loop V1 can organize continuity, local read-only research, sticky audit, one consequence request, and consequence review. Local phases may self-start, but live consequence still requires the existing bridge/steward gate. No ambient bind, resume, broad perturbation, broad Control envelope, attractor pulse, peer mutation, or automatic execution is authorized."
}

fn research_budget_self_activation_v1(budget: &Value, eligibility: &Value, state: &Value) -> Value {
    let mut missing = eligibility
        .get("missing_requirements")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if budget
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or_default()
        != "read_only_research"
        && !missing
            .iter()
            .any(|item| item == "scope_read_only_research_v1")
    {
        missing.push("scope_read_only_research_v1".to_string());
    }
    let allowed_sources = budget
        .get("allowed_sources")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if allowed_sources != ["local"] {
        missing.push("local_only_allowed_sources".to_string());
    }
    if budget
        .get("max_actions")
        .and_then(Value::as_u64)
        .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS)
        > LOCAL_RESEARCH_MAX_ACTIONS
    {
        missing.push(format!(
            "max_actions_self_activation_cap_{LOCAL_RESEARCH_MAX_ACTIONS}"
        ));
    }
    if budget
        .get("ttl_secs")
        .and_then(Value::as_u64)
        .unwrap_or(LOCAL_RESEARCH_TTL_SECS)
        > LOCAL_RESEARCH_TTL_SECS
    {
        missing.push(format!(
            "ttl_secs_self_activation_cap_{LOCAL_RESEARCH_TTL_SECS}"
        ));
    }
    let safety = authority_safety_snapshot(state);
    if !matches!(
        safety.get("level").and_then(Value::as_str),
        Some("green" | "yellow")
    ) {
        missing.push("green_or_yellow_safety".to_string());
    }
    let purpose_upper = budget
        .get("purpose")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    if [
        "EXPERIMENT_BIND",
        "EXPERIMENT_RESUME",
        "PERTURB",
        "CONTROL",
        "SEND_CONTROL",
        "ATTRACTOR_PULSE",
        "SEMANTIC_MICRODOSE",
        "MIKE_RUN",
        "AR_START",
        "AR_NOTE",
        "AR_BLOCK",
        "AR_COMPLETE",
    ]
    .iter()
    .any(|needle| purpose_upper.contains(needle))
    {
        missing.push("no_live_or_mutating_research_intent".to_string());
    }
    missing.sort();
    missing.dedup();
    let eligible = missing.is_empty();
    json!({
        "policy": "research_budget_self_activation_v1",
        "eligible": eligible,
        "missing_requirements": missing,
        "activation_mode": "being_self_activated_local_v1",
        "self_activated": eligible,
        "steward_approval_required": !eligible,
        "max_actions_cap": LOCAL_RESEARCH_MAX_ACTIONS,
        "ttl_secs_cap": LOCAL_RESEARCH_TTL_SECS,
        "allowed_sources": ["local"],
        "safety_snapshot": safety,
    })
}

fn authority_safety_snapshot(state: &Value) -> Value {
    let fill_pct = state.get("fill_pct").and_then(Value::as_f64).or_else(|| {
        state
            .get("fill_ratio")
            .and_then(Value::as_f64)
            .map(|ratio| ratio * 100.0)
    });
    let level = fill_pct.map_or("unknown", |fill| {
        if fill >= 92.0 {
            "red"
        } else if fill >= 85.0 {
            "orange"
        } else if fill >= 75.0 {
            "yellow"
        } else {
            "green"
        }
    });
    json!({
        "fill_pct": fill_pct,
        "level": level,
        "outbound_allowed": matches!(level, "green" | "yellow" | "orange"),
    })
}

fn authority_has_read_only_rehearsal(runs: &[ExperimentRunRecord]) -> bool {
    runs.iter().any(|run| {
        let source = run.source.to_ascii_lowercase();
        let status = run.status.to_ascii_lowercase();
        let stage = run.stage.to_ascii_lowercase();
        let action_text = run.action_text.to_ascii_uppercase();
        (source.contains("experiment_rehearse") && !status.contains("blocked"))
            || (action_text.contains("ACTION_PREFLIGHT")
                && matches!(stage.as_str(), "read_only" | "protected" | "preflight"))
    })
}

fn authority_guardrail_hold_active(experiment: &ExperimentRecord) -> bool {
    experiment.status.eq_ignore_ascii_case("paused")
        && experiment
            .planned_next
            .as_deref()
            .unwrap_or_default()
            .to_ascii_uppercase()
            .starts_with("THREAD_STATUS")
        && experiment
            .success_observation
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("hold")
}

fn latest_active_authority_approval(rows: &[Value], request_id: &str) -> Option<Value> {
    rows.iter().rev().find_map(|row| {
        (row.get("request_id").and_then(Value::as_str) == Some(request_id)
            && row.get("record_type").and_then(Value::as_str) == Some("steward_approval")
            && row.get("token_status").and_then(Value::as_str) == Some("active"))
        .then(|| row.clone())
    })
}

fn authority_artifact_ref_candidates(
    experiment: &ExperimentRecord,
    runs: &[ExperimentRunRecord],
) -> Vec<String> {
    let mut candidates = Vec::<String>::new();
    if let Some(items) = experiment
        .evidence_v1
        .as_ref()
        .and_then(|evidence| evidence.get("artifact_refs"))
        .and_then(Value::as_array)
    {
        for item in items {
            push_authority_artifact_value(&mut candidates, item);
        }
    }
    if let Some(items) = experiment
        .evidence_v1
        .as_ref()
        .and_then(|evidence| evidence.get("felt_observations"))
        .and_then(Value::as_array)
    {
        for item in items {
            for key in ["note", "felt", "summary"] {
                if let Some(text) = item.get(key).and_then(Value::as_str) {
                    candidates.extend(scan_authority_artifact_text(text));
                }
            }
        }
    }
    for run in runs {
        if let Ok(value) = serde_json::to_value(&run.artifacts)
            && let Some(items) = value.as_array()
        {
            for item in items {
                push_authority_artifact_value(&mut candidates, item);
            }
        }
        candidates.extend(scan_authority_artifact_text(&run.result_summary));
        candidates.extend(scan_authority_artifact_text(&run.interpretation));
    }
    let mut deduped = Vec::<String>::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
        if deduped.len() >= 8 {
            break;
        }
    }
    deduped
}

fn push_authority_artifact_value(candidates: &mut Vec<String>, value: &Value) {
    if let Some(text) = value.as_str() {
        if !text.trim().is_empty() {
            candidates.push(text.trim().to_string());
        }
        return;
    }
    if let Some(object) = value.as_object() {
        for key in ["path", "url", "artifact_path", "artifact_ref", "ref"] {
            if let Some(text) = object.get(key).and_then(Value::as_str)
                && !text.trim().is_empty()
            {
                candidates.push(text.trim().to_string());
                return;
            }
        }
        if !object.is_empty() {
            candidates.push(value.to_string());
        }
    }
}

fn scan_authority_artifact_text(text: &str) -> Vec<String> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
        .filter_map(|part| {
            let trimmed = part.trim_matches(|ch| matches!(ch, ')' | ']' | '.'));
            (trimmed.starts_with('/')
                || trimmed.starts_with("http://")
                || trimmed.starts_with("https://"))
            .then(|| trimmed.to_string())
        })
        .collect()
}

fn authority_readiness_token_status(rows: &[Value], request_id: &str) -> String {
    if request_id.is_empty() {
        return "none".to_string();
    }
    if let Some(approval) = latest_active_authority_approval(rows, request_id) {
        let token_id = approval.get("token_id").and_then(Value::as_str);
        let consumed = rows.iter().any(|row| {
            matches!(
                row.get("record_type").and_then(Value::as_str),
                Some("execution_result" | "blocked")
            ) && row.get("token_id").and_then(Value::as_str) == token_id
        });
        if consumed {
            return "consumed".to_string();
        }
        if approval
            .get("expires_at_unix_s")
            .and_then(Value::as_u64)
            .is_some_and(|expires| expires < chrono::Utc::now().timestamp().try_into().unwrap_or(0))
        {
            return "expired".to_string();
        }
        return "active".to_string();
    }
    let latest = rows
        .iter()
        .rev()
        .find(|row| row.get("request_id").and_then(Value::as_str) == Some(request_id));
    let request = rows.iter().rev().find(|row| {
        row.get("request_id").and_then(Value::as_str) == Some(request_id)
            && row.get("record_type").and_then(Value::as_str) == Some("request")
    });
    if latest
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_steward_approval")
        || request
            .and_then(|row| row.get("status"))
            .and_then(Value::as_str)
            == Some("pending_steward_approval")
    {
        return "pending_steward_approval".to_string();
    }
    if request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_budget_execution")
    {
        if request
            .and_then(|row| row.get("budget_id"))
            .and_then(Value::as_str)
            .and_then(|budget_id| pending_authority_budget_review(rows, budget_id))
            .is_some()
        {
            return "review_required".to_string();
        }
        return "budget_available".to_string();
    }
    if latest.is_some_and(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("blocked")
            || row.get("status").and_then(Value::as_str) == Some("blocked")
    }) {
        return "blocked".to_string();
    }
    "none".to_string()
}

fn authority_readiness_stage(
    experiment: &ExperimentRecord,
    conveyor_stage: &str,
    missing: &[Value],
    latest_request: Option<&Value>,
    token_status: &str,
) -> String {
    if authority_guardrail_hold_active(experiment) {
        return "held_or_guarded".to_string();
    }
    match token_status {
        "consumed" => return "executed_or_consumed".to_string(),
        "active" => return "token_active_bridge_executable".to_string(),
        "budget_available" => return "pending_budget_execution".to_string(),
        "review_required" => return "review_required".to_string(),
        "pending_steward_approval" => return "pending_steward_approval".to_string(),
        _ => {},
    }
    if latest_request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("blocked")
        && !missing.is_empty()
    {
        return "blocked".to_string();
    }
    if matches!(conveyor_stage, "paused_repair" | "blocked_guardrail") {
        return "held_or_guarded".to_string();
    }
    let has_missing = |target: &str| missing.iter().any(|item| item.as_str() == Some(target));
    if has_missing("lifecycle_valid_charter") || conveyor_stage == "needs_charter" {
        return "needs_charter".to_string();
    }
    if has_missing("read_only_rehearsal") || conveyor_stage == "needs_rehearsal" {
        return "needs_rehearsal".to_string();
    }
    if has_missing("meaningful_evidence") || conveyor_stage == "needs_evidence" {
        return "needs_evidence".to_string();
    }
    if has_missing("artifact_grounding_refs") {
        return "needs_artifact_grounding".to_string();
    }
    if missing.is_empty() {
        return "ready_to_author_request".to_string();
    }
    "blocked".to_string()
}

fn authority_readiness_next_command(
    experiment_id: &str,
    stage: &str,
    proposed_next: &str,
    latest_request_id: &str,
    request_scaffold: Option<&str>,
) -> String {
    if stage == "ready_to_author_request"
        && let Some(scaffold) = request_scaffold
    {
        return scaffold.to_string();
    }
    if stage == "pending_budget_execution" && !latest_request_id.is_empty() {
        return format!("EXPERIMENT_AUTHORITY_EXECUTE {latest_request_id}");
    }
    if stage == "review_required" && !latest_request_id.is_empty() {
        return format!(
            "EXPERIMENT_AUTHORITY_REVIEW {latest_request_id} :: outcome: hold|repeat|alter|retire; observation: ...; next_payload: ...; source_refs: ..."
        );
    }
    if matches!(
        stage,
        "pending_steward_approval"
            | "token_active_bridge_executable"
            | "executed_or_consumed"
            | "blocked"
    ) && !latest_request_id.is_empty()
    {
        return format!("EXPERIMENT_AUTHORITY_STATUS {latest_request_id}");
    }
    if stage == "needs_artifact_grounding" {
        return format!(
            "EXPERIMENT_EVIDENCE {experiment_id} :: artifact_grounding: <absolute artifact ref>"
        );
    }
    if !proposed_next.is_empty() {
        return proposed_next.to_string();
    }
    format!("EXPERIMENT_ADVANCE {experiment_id} :: mode: preview")
}

fn active_authority_budget_from_rows(
    rows: &[Value],
    experiment_id: &str,
    scope: &str,
) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("budget_closed")
        })
        .filter_map(|row| row.get("budget_id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp().try_into().unwrap_or(0);
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some("authority_budget_v1")
            || row.get("record_type").and_then(Value::as_str) != Some("budget_approval")
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            || row
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("semantic_microdose")
                != scope
            || row
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                != "active"
        {
            return None;
        }
        let budget_id = row.get("budget_id").and_then(Value::as_str)?;
        if closed.contains(budget_id)
            || row
                .get("expires_at_unix_s")
                .and_then(Value::as_u64)
                .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        let max_sends = row
            .get("max_sends")
            .and_then(Value::as_u64)
            .unwrap_or(AUTHORITY_BUDGET_MAX_SENDS);
        let spent = rows
            .iter()
            .filter(|item| {
                item.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
                    && item.get("record_type").and_then(Value::as_str) == Some("budget_debit")
                    && item.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            })
            .count();
        let spent_u64 = u64::try_from(spent).unwrap_or(u64::MAX);
        let remaining = max_sends.saturating_sub(spent_u64);
        if remaining == 0 {
            return None;
        }
        let mut active = row.clone();
        if let Some(object) = active.as_object_mut() {
            object.insert("spent_sends".to_string(), json!(spent_u64));
            object.insert("remaining_sends".to_string(), json!(remaining));
            object.insert(
                "pending_review_request_id".to_string(),
                pending_authority_budget_review(rows, budget_id).map_or(Value::Null, Value::String),
            );
        }
        Some(active)
    })
}

fn pending_authority_budget_review(rows: &[Value], budget_id: &str) -> Option<String> {
    let latest_debit = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_debit")
            && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
    })?;
    let request_id = latest_debit.get("request_id").and_then(Value::as_str)?;
    let reviewed = rows.iter().any(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("consequence_review")
            && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            && row.get("request_id").and_then(Value::as_str) == Some(request_id)
    });
    (!reviewed).then(|| request_id.to_string())
}

fn budget_id_for_request(rows: &[Value], request_id: &str) -> Option<String> {
    rows.iter().rev().find_map(|row| {
        (row.get("request_id").and_then(Value::as_str) == Some(request_id))
            .then(|| {
                row.get("budget_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .flatten()
    })
}

fn authority_budget_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_request")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_approval")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("authority_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("budget_closed")
    });
    let experiment_id = latest_approval
        .or(latest_request)
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active = (!experiment_id.is_empty())
        .then(|| active_authority_budget_from_rows(rows, experiment_id, "semantic_microdose"))
        .flatten();
    let pending_review = active
        .as_ref()
        .and_then(|row| row.get("pending_review_request_id"))
        .and_then(Value::as_str);
    let stage = if pending_review.is_some() {
        "review_required"
    } else if active.is_some() {
        "active_budget_available"
    } else if latest_closed.is_some() {
        "budget_closed"
    } else if latest_approval.is_some() {
        "budget_unavailable"
    } else if latest_request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_steward_approval")
    {
        "pending_steward_approval"
    } else if latest_request.is_some() {
        "blocked"
    } else {
        "no_budget"
    };
    json!({
        "policy": "authority_budget_v1",
        "scope": "semantic_microdose",
        "stage": stage,
        "active_budget_id": active.as_ref().and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "remaining_sends": active.as_ref().and_then(|row| row.get("remaining_sends")).cloned().unwrap_or(json!(0)),
        "review_required": pending_review.is_some(),
        "pending_review_request_id": pending_review.map_or(Value::Null, |id| json!(id)),
        "latest_budget_request_id": latest_request.and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
    })
}

fn active_sovereign_loop_from_rows(rows: &[Value], experiment_id: &str) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("sovereign_loop_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("loop_closed")
        })
        .filter_map(|row| row.get("loop_id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp().try_into().unwrap_or(0);
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some("sovereign_loop_v1")
            || !matches!(
                row.get("record_type").and_then(Value::as_str),
                Some(
                    "loop_started"
                        | "loop_approval"
                        | "loop_step"
                        | "loop_consequence_ready"
                        | "loop_consequence_review"
                        | "loop_request"
                )
            )
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
        {
            return None;
        }
        let loop_id = row.get("loop_id").and_then(Value::as_str)?;
        if closed.contains(loop_id)
            || row
                .get("expires_at_unix_s")
                .and_then(Value::as_u64)
                .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        Some(row.clone())
    })
}

fn active_research_budget_from_rows(rows: &[Value], experiment_id: &str) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("research_budget_closed")
        })
        .filter_map(|row| row.get("budget_id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp().try_into().unwrap_or(0);
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some("research_budget_v1")
            || row.get("record_type").and_then(Value::as_str) != Some("research_budget_approval")
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            || row
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("read_only_research")
                != "read_only_research"
            || row
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                != "active"
        {
            return None;
        }
        let budget_id = row.get("budget_id").and_then(Value::as_str)?;
        if closed.contains(budget_id)
            || row
                .get("expires_at_unix_s")
                .and_then(Value::as_u64)
                .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        let max_actions = row
            .get("max_actions")
            .and_then(Value::as_u64)
            .unwrap_or(LOCAL_RESEARCH_MAX_ACTIONS);
        let spent = rows
            .iter()
            .filter(|item| {
                item.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                    && item.get("record_type").and_then(Value::as_str)
                        == Some("research_budget_debit")
                    && item.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            })
            .count();
        let spent_u64 = u64::try_from(spent).unwrap_or(u64::MAX);
        let remaining = max_actions.saturating_sub(spent_u64);
        if remaining == 0 {
            return None;
        }
        let mut active = row.clone();
        if let Some(object) = active.as_object_mut() {
            object.insert("spent_actions".to_string(), json!(spent_u64));
            object.insert("remaining_actions".to_string(), json!(remaining));
        }
        Some(active)
    })
}

fn latest_pending_research_budget_request<'a>(
    rows: &'a [Value],
    experiment_id: &str,
) -> Option<&'a Value> {
    rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
            && row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
            && row.get("status").and_then(Value::as_str) == Some("pending_steward_approval")
    })
}

fn latest_research_budget_scaffold_row<'a>(
    rows: &'a [Value],
    experiment_id: &str,
) -> Option<&'a Value> {
    rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_blocked")
            && row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
            && research_budget_row_request_scaffold(row).is_some()
    })
}

fn research_budget_row_request_scaffold(row: &Value) -> Option<String> {
    [
        "request_scaffold",
        "suggested_request_scaffold",
        "suggested_next",
    ]
    .iter()
    .filter_map(|key| row.get(*key).and_then(Value::as_str))
    .find(|value| base_action(value) == "EXPERIMENT_RESEARCH_BUDGET_REQUEST")
    .map(ToString::to_string)
}

fn research_budget_scaffold_request_arg(scaffold: &str) -> Option<String> {
    let trimmed = scaffold.trim();
    if base_action(trimmed) != "EXPERIMENT_RESEARCH_BUDGET_REQUEST" {
        return None;
    }
    Some(
        trimmed
            .split_once(char::is_whitespace)
            .map_or("", |(_, tail)| tail)
            .trim()
            .to_string(),
    )
}

fn research_budget_scaffold_is_local_only(scaffold: &str) -> bool {
    let Some(raw) = dossier_field(scaffold, &["allowed_sources", "sources"]) else {
        return false;
    };
    let sources = raw
        .split([',', '/', '|'])
        .map(str::trim)
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>();
    sources.len() == 1 && sources.first().is_some_and(|source| *source == "local")
}

fn research_budget_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_closed")
    });
    let latest_blocked = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_blocked")
    });
    let experiment_id = latest_approval
        .or(latest_request)
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active = (!experiment_id.is_empty())
        .then(|| active_research_budget_from_rows(rows, experiment_id))
        .flatten();
    let duplicate_blocked = latest_blocked
        .and_then(|row| row.get("reason"))
        .and_then(Value::as_str)
        == Some("duplicate_query_or_url_review_required");
    let stage = if active.is_some() && duplicate_blocked {
        "review_required_duplicate_loop"
    } else if active.is_some() {
        "active_budget_available"
    } else if latest_closed.is_some() {
        "budget_closed"
    } else if latest_approval.is_some() {
        "budget_unavailable"
    } else if latest_request
        .and_then(|row| row.get("status"))
        .and_then(Value::as_str)
        == Some("pending_steward_approval")
    {
        "pending_steward_approval"
    } else if latest_request.is_some() {
        "blocked"
    } else {
        "no_budget"
    };
    json!({
        "policy": "research_budget_v1",
        "scope": "read_only_research",
        "stage": stage,
        "active_budget_id": active.as_ref().and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "remaining_actions": active.as_ref().and_then(|row| row.get("remaining_actions")).cloned().unwrap_or(json!(0)),
        "activation_mode": active
            .as_ref()
            .or(latest_approval)
            .and_then(|row| row.get("activation_mode"))
            .cloned()
            .unwrap_or(Value::Null),
        "self_activated": active
            .as_ref()
            .or(latest_approval)
            .and_then(|row| row.get("self_activated"))
            .cloned()
            .unwrap_or(json!(false)),
        "steward_approval_required": latest_request
            .and_then(|row| row.get("steward_approval_required"))
            .cloned()
            .unwrap_or(Value::Null),
        "review_required": stage == "review_required_duplicate_loop",
        "latest_budget_request_id": latest_request.and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "allowed_actions": ["SEARCH", "BROWSE", "READ_MORE", "MIKE_BROWSE", "MIKE_READ", "MIKE_SEARCH", "AR_LIST", "AR_LOOK", "AR_SHOW", "AR_READ", "AR_DEEP_READ", "AR_VALIDATE"],
        "authority_boundary": research_budget_boundary(),
    })
}

fn sovereign_loop_review_next_command(
    outcome: &str,
    loop_id: &str,
    _loop_row: &Value,
    experiment: &ExperimentRecord,
) -> String {
    match outcome {
        "retire" | "hold" => "THREAD_STATUS current".to_string(),
        "promote" => format!(
            "MEMORY_PROMOTE {} :: dossier|evidence|authority_request",
            experiment.experiment_id
        ),
        "alter" => format!("EXPERIMENT_LOOP_STEP {loop_id} :: authority_prepare"),
        "repeat" => format!("EXPERIMENT_LOOP_STEP {loop_id} :: research"),
        _ => "THREAD_STATUS current".to_string(),
    }
}

fn authority_review_next_command(outcome: &str, request: &Value, next_payload: &str) -> String {
    let experiment_id = request
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("current");
    let artifact_refs = request
        .get("artifact_refs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let stop = request
        .get("stop_criteria")
        .and_then(Value::as_str)
        .unwrap_or("one attempted semantic send only");
    match outcome {
        "retire" => format!("EXPERIMENT_AUTHORITY_BUDGET_STATUS {experiment_id}"),
        "hold" => "THREAD_STATUS current".to_string(),
        "alter" => format!(
            "EXPERIMENT_AUTHORITY_REQUEST {experiment_id} :: scope: semantic_microdose; payload: {}; reason: consequence review chose alter; artifact_refs: {artifact_refs}; stop_criteria: {stop}",
            next_payload.trim()
        ),
        _ => format!(
            "EXPERIMENT_AUTHORITY_REQUEST {experiment_id} :: scope: semantic_microdose; payload: {}; reason: consequence review chose repeat; artifact_refs: {artifact_refs}; stop_criteria: {stop}",
            request
                .get("payload")
                .and_then(Value::as_str)
                .unwrap_or("...")
        ),
    }
}
