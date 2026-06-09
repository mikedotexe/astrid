//! Read-only visibility for experiment lifecycle conveyor state.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::paths::bridge_paths;
use crate::shared_investigation;

#[derive(Debug, Clone)]
pub struct RenderedExperimentConveyor {
    pub output_dir: PathBuf,
    pub index_html: PathBuf,
    pub json_path: PathBuf,
    pub status: Value,
}

pub fn status() -> Result<Value> {
    status_from_paths(
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn status_from_paths(minime_workspace: &Path, bridge_workspace: &Path) -> Result<Value> {
    let minime = being_status("minime", minime_workspace)?;
    let astrid = being_status("astrid", bridge_workspace)?;
    Ok(json!({
        "schema_version": 1,
        "policy": "experiment_conveyor_visibility_v1",
        "authority_boundary": authority_boundary(),
        "systems": {
            "minime": minime,
            "astrid": astrid
        }
    }))
}

pub fn render(output_base: Option<&Path>) -> Result<RenderedExperimentConveyor> {
    let status = status()?;
    render_status_to_base(status, output_base)
}

pub fn render_status_to_base(
    status: Value,
    output_base: Option<&Path>,
) -> Result<RenderedExperimentConveyor> {
    let output_root = output_base.map_or_else(
        || {
            bridge_paths()
                .bridge_workspace()
                .join("diagnostics/experiment_conveyor")
        },
        Path::to_path_buf,
    );
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = unique_dir(&output_root.join(stamp));
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("experiment_conveyor.json");
    let index_html = output_dir.join("index.html");
    fs::write(&json_path, serde_json::to_string_pretty(&status)?)?;
    fs::write(&index_html, render_html(&status))?;
    Ok(RenderedExperimentConveyor {
        output_dir,
        index_html,
        json_path,
        status,
    })
}

fn being_status(being: &str, workspace: &Path) -> Result<Value> {
    let threads_root = workspace.join("action_threads/threads");
    let Some((thread_path, thread)) = latest_thread(&threads_root)? else {
        return Ok(json!({
            "being": being,
            "workspace": workspace,
            "status": "no_thread",
            "authority_boundary": authority_boundary()
        }));
    };
    let experiments_path = thread_path.join("experiments.jsonl");
    let runs_path = thread_path.join("experiment_runs.jsonl");
    let gate_path = thread_path.join("authority_gate.jsonl");
    let memory_path = thread_path.join("being_memory.jsonl");
    let experiments = latest_experiments(&experiments_path)?;
    let selected = selected_experiment(&thread, &experiments);
    let runs = selected.as_ref().map_or_else(Vec::new, |experiment| {
        let experiment_id = experiment
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        recent_runs(&runs_path, experiment_id).unwrap_or_default()
    });
    let conveyor = selected.as_ref().map(|experiment| {
        conveyor_readout(
            being,
            &thread,
            experiment,
            &runs,
            &experiments_path,
            &runs_path,
            &gate_path,
            &memory_path,
        )
    });
    Ok(json!({
        "being": being,
        "workspace": workspace,
        "thread_path": thread_path,
        "thread": compact_thread(&thread),
        "experiment": selected,
        "recent_runs": runs,
        "conveyor_v1": conveyor,
        "being_memory_v1": memory_summary(&memory_path, selected.as_ref()),
        "latest_authority_consequence_v1": latest_consequence(&gate_path),
        "voice_health_v1": (being == "astrid").then(|| read_voice_health(workspace)).flatten(),
        "authority_boundary": authority_boundary()
    }))
}

fn latest_thread(root: &Path) -> Result<Option<(PathBuf, Value)>> {
    if !root.exists() {
        return Ok(None);
    }
    let mut rows = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path().join("thread.json");
        if !path.exists() {
            continue;
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("read thread {}", path.display()))?;
        let value = serde_json::from_str::<Value>(&raw)
            .with_context(|| format!("parse thread {}", path.display()))?;
        rows.push((entry.path(), value));
    }
    rows.sort_by(|left, right| sort_key(&right.1).cmp(&sort_key(&left.1)));
    Ok(rows.into_iter().next())
}

fn latest_experiments(path: &Path) -> Result<Vec<Value>> {
    let mut by_id = BTreeMap::<String, Value>::new();
    for row in read_jsonl(path)? {
        if let Some(id) = row.get("experiment_id").and_then(Value::as_str) {
            by_id.insert(id.to_string(), row);
        }
    }
    Ok(by_id.into_values().collect())
}

fn selected_experiment(thread: &Value, experiments: &[Value]) -> Option<Value> {
    let active_id = thread.get("active_experiment_id").and_then(Value::as_str);
    if let Some(active_id) = active_id
        && let Some(experiment) = experiments
            .iter()
            .rev()
            .find(|row| row.get("experiment_id").and_then(Value::as_str) == Some(active_id))
    {
        return Some(experiment.clone());
    }
    let summary_id = thread
        .get("experiment_summary")
        .and_then(|summary| summary.get("experiment_id"))
        .and_then(Value::as_str);
    if let Some(summary_id) = summary_id {
        return experiments
            .iter()
            .rev()
            .find(|row| row.get("experiment_id").and_then(Value::as_str) == Some(summary_id))
            .cloned()
            .or_else(|| thread.get("experiment_summary").cloned());
    }
    experiments.last().cloned()
}

fn recent_runs(path: &Path, experiment_id: &str) -> Result<Vec<Value>> {
    let mut rows: Vec<Value> = read_jsonl(path)?
        .into_iter()
        .filter(|row| row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id))
        .collect();
    if rows.len() > 8 {
        rows = rows.split_off(rows.len().saturating_sub(8));
    }
    Ok(rows)
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect())
}

fn memory_summary(path: &Path, experiment: Option<&Value>) -> Value {
    let experiment_id = experiment
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str);
    let rows = read_jsonl(path).unwrap_or_default();
    let filtered = rows
        .into_iter()
        .filter(|row| row.get("record_schema").and_then(Value::as_str) == Some("being_memory_v1"))
        .filter(|row| {
            experiment_id.is_none()
                || row.get("experiment_id").and_then(Value::as_str) == experiment_id
        })
        .collect::<Vec<_>>();
    json!({
        "policy": "being_memory_v1",
        "card_count": filtered.iter().filter(|row| row.get("record_type").and_then(Value::as_str) == Some("card")).count(),
        "draft_count": filtered.iter().filter(|row| row.get("record_type").and_then(Value::as_str) == Some("draft")).count(),
        "latest_memory": filtered.last().cloned(),
        "suggested_capture_next": format!("MEMORY_CAPTURE {} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...", experiment_id.unwrap_or("latest")),
        "suggested_recall_next": format!("MEMORY_RECALL {} :: focus: ...", experiment_id.unwrap_or("latest")),
        "authority_boundary": authority_boundary(),
    })
}

fn latest_consequence(path: &Path) -> Value {
    read_jsonl(path)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("authority_consequence_v1")
        })
        .unwrap_or(Value::Null)
}

fn conveyor_readout(
    being: &str,
    thread: &Value,
    experiment: &Value,
    runs: &[Value],
    experiments_path: &Path,
    runs_path: &Path,
    gate_path: &Path,
    memory_path: &Path,
) -> Value {
    let experiment_id = experiment
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let classification = classify(experiment, runs);
    let return_info = (classification == "paused").then(|| paused_return_info(experiment));
    let stage = stage_for(experiment, &classification);
    let proposed_next = proposed_next(experiment, &stage);
    let can_apply = visibility_can_apply(experiment, &stage);
    let apply_blocked_reason = visibility_apply_blocked_reason(experiment, &stage, can_apply);
    let gate_rows = read_jsonl(gate_path).unwrap_or_default();
    let mut source_refs = vec![
        experiments_path.display().to_string(),
        runs_path.display().to_string(),
        gate_path.display().to_string(),
        memory_path.display().to_string(),
    ];
    if let Some(thread_id) = thread.get("thread_id").and_then(Value::as_str) {
        source_refs.push(
            experiments_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join("thread.json")
                .display()
                .to_string(),
        );
        source_refs.push(format!("thread:{thread_id}"));
    }
    let shared = shared_investigation_for_experiment(experiment_id);
    if let Some(id) = shared
        .as_ref()
        .and_then(|row| row.get("id"))
        .and_then(Value::as_str)
    {
        source_refs.push(format!("shared_investigation:{id}"));
    }
    let mut payload = json!({
        "schema_version": 1,
        "policy": "experiment_conveyor_visibility_v1",
        "being": being,
        "experiment_id": experiment_id,
        "status": experiment.get("status").and_then(Value::as_str).unwrap_or("active"),
        "classification": classification,
        "stage": stage,
        "missing_requirements": missing_requirements(experiment, &stage),
        "proposed_next": proposed_next,
        "conveyor_next": format!("EXPERIMENT_ADVANCE {experiment_id} :: mode: preview"),
        "preview_allowed": true,
        "apply_policy": "conservative_local_v1",
        "allowed_apply_steps": allowed_apply_steps(),
        "can_apply": can_apply,
        "apply_blocked_reason": apply_blocked_reason,
        "would_mutate": false,
        "source_refs": source_refs,
        "shared_investigation_v1": shared,
        "being_memory_v1": memory_summary(memory_path, Some(experiment)),
        "latest_authority_consequence_v1": latest_consequence(gate_path),
        "authority_readiness_v1": authority_readiness(
            experiment,
            runs,
            &gate_rows,
            &stage,
            &proposed_next,
            &source_refs,
        ),
        "authority_boundary": authority_boundary(),
    });
    if let Some((primary_return_next, return_kind)) = return_info {
        payload["primary_return_next"] = json!(primary_return_next);
        payload["return_kind"] = json!(return_kind);
    }
    if let Some(guardrail) = decision_guardrail(experiment) {
        payload["decision_guardrail_v1"] = guardrail;
    }
    payload
}

fn allowed_apply_steps() -> Value {
    json!([
        "lifecycle_valid_charter",
        "local_evidence_capture",
        "hold_decision",
        "charter_repair_decision"
    ])
}

fn visibility_can_apply(experiment: &Value, stage: &str) -> bool {
    match stage {
        "needs_evidence" | "needs_decision" => true,
        "blocked_guardrail" => true,
        "needs_charter" => valid_charter(experiment.get("charter_v1")),
        _ => false,
    }
}

fn visibility_apply_blocked_reason(
    experiment: &Value,
    stage: &str,
    can_apply: bool,
) -> Option<&'static str> {
    if can_apply {
        return None;
    }
    match stage {
        "needs_charter" if !valid_charter(experiment.get("charter_v1")) => {
            Some("no_lifecycle_valid_charter_scaffold")
        },
        "needs_rehearsal" => Some("rehearsal_requires_explicit_experiment_rehearse"),
        "paused_repair" | "paused_resume" => {
            Some("paused_experiments_require_explicit_return_command")
        },
        "complete" => Some("complete_experiments_are_review_only"),
        _ => Some("no_conservative_apply_step_available"),
    }
}

fn compact_thread(thread: &Value) -> Value {
    json!({
        "thread_id": thread.get("thread_id"),
        "title": thread.get("title"),
        "status": thread.get("status"),
        "active_experiment_id": thread.get("active_experiment_id"),
        "current_next": thread.get("current_next"),
        "experiment_summary": thread.get("experiment_summary"),
    })
}

fn classify(experiment: &Value, runs: &[Value]) -> String {
    let status = experiment
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active")
        .to_ascii_lowercase();
    if status == "paused" {
        return "paused".to_string();
    }
    if matches!(status.as_str(), "complete" | "completed") {
        return "complete".to_string();
    }
    let blocked_count = runs
        .iter()
        .rev()
        .take(4)
        .filter(|run| {
            matches!(
                run.get("status").and_then(Value::as_str),
                Some("blocked" | "no_effect" | "rehearsal_blocked" | "failed")
            )
        })
        .count();
    if blocked_count >= 2 {
        return "blocked_loop".to_string();
    }
    if !valid_charter(experiment.get("charter_v1")) {
        return "needs_charter".to_string();
    }
    if meaningful_evidence(experiment.get("evidence_v1")) {
        return "needs_decision".to_string();
    }
    if runs.iter().any(|run| {
        matches!(
            run.get("status").and_then(Value::as_str),
            Some("handled" | "rehearsed" | "observed" | "evidence_recorded")
        )
    }) {
        return "needs_evidence".to_string();
    }
    "needs_rehearsal".to_string()
}

fn stage_for(experiment: &Value, classification: &str) -> String {
    if classification == "paused" {
        if !valid_charter(experiment.get("charter_v1")) {
            return "paused_repair".to_string();
        }
        let base = base_action(
            experiment
                .get("planned_next")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        return if matches!(
            base.as_str(),
            "EXPERIMENT_CHARTER"
                | "EXPERIMENT_DECIDE"
                | "THREAD_STATUS"
                | "EXPERIMENT_ADVANCE"
                | "EXPERIMENT_CONVEYOR"
                | "EXPERIMENT_REHEARSE"
                | "EXPERIMENT_PREFLIGHT"
        ) {
            "paused_repair".to_string()
        } else {
            "paused_resume".to_string()
        };
    }
    match classification {
        "complete" => "complete",
        "blocked_loop" => "blocked_guardrail",
        "needs_charter" => "needs_charter",
        "needs_decision" => "needs_decision",
        "needs_evidence" => "needs_evidence",
        _ => "needs_rehearsal",
    }
    .to_string()
}

fn paused_return_info(experiment: &Value) -> (String, &'static str) {
    let planned = experiment
        .get("planned_next")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let base = base_action(planned);
    if !valid_charter(experiment.get("charter_v1"))
        && !matches!(
            base.as_str(),
            "EXPERIMENT_CHARTER" | "EXPERIMENT_DECIDE" | "THREAD_STATUS"
        )
    {
        let id = experiment
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or("current");
        return (
            format!(
                "EXPERIMENT_CHARTER {id} :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: spectral_condition, fill_pressure_state, recurrence_pattern, artifact_grounding; stop_criteria: ..."
            ),
            "charter_repair",
        );
    }
    let kind = match base.as_str() {
        "EXPERIMENT_CHARTER" => "charter_repair",
        "EXPERIMENT_DECIDE" => "decision",
        "THREAD_STATUS" => "hold",
        "EXPERIMENT_ADVANCE" | "EXPERIMENT_CONVEYOR" => "conveyor_preview",
        "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT" => "rehearsal_ready",
        "EXPERIMENT_RESUME" => "resume",
        _ => "resume",
    };
    let primary = if planned.trim().is_empty() && kind == "resume" {
        let id = experiment
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        format!("EXPERIMENT_RESUME {id}")
    } else {
        planned.to_string()
    };
    (primary, kind)
}

fn proposed_next(experiment: &Value, stage: &str) -> String {
    let id = experiment
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("current");
    match stage {
        "paused_repair" | "paused_resume" => paused_return_info(experiment).0,
        "complete" => format!("EXPERIMENT_REVIEW {id}"),
        "blocked_guardrail" if !valid_charter(experiment.get("charter_v1")) => format!(
            "EXPERIMENT_DECIDE {id} :: charter_repair because blocked guardrail evidence appeared without a lifecycle-valid charter"
        ),
        "blocked_guardrail" => format!(
            "EXPERIMENT_DECIDE {id} :: hold because blocked guardrail evidence is not experiment progress"
        ),
        "needs_charter" => format!(
            "EXPERIMENT_CHARTER {id} :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: spectral_condition, fill_pressure_state, recurrence_pattern, artifact_grounding; stop_criteria: ..."
        ),
        "needs_rehearsal" => format!("EXPERIMENT_REHEARSE {id}"),
        "needs_evidence" => format!(
            "EXPERIMENT_EVIDENCE {id} :: spectral_condition ...; fill_pressure_state ...; recurrence_pattern ...; artifact_grounding ..."
        ),
        "needs_decision" => format!(
            "EXPERIMENT_DECIDE {id} :: hold because evidence is ready to interpret without live authority"
        ),
        _ => "THREAD_STATUS current".to_string(),
    }
}

fn missing_requirements(experiment: &Value, stage: &str) -> Vec<&'static str> {
    match stage {
        "needs_charter" => charter_missing_fields(experiment.get("charter_v1")),
        "needs_rehearsal" => vec!["read_only_rehearsal"],
        "needs_evidence" => vec!["explicit_experiment_evidence"],
        "needs_decision" => vec!["explicit_lifecycle_decision"],
        "paused_repair" => vec!["explicit_repair_return"],
        "paused_resume" => vec!["explicit_resume_or_hold"],
        "blocked_guardrail" if !valid_charter(experiment.get("charter_v1")) => {
            vec!["charter_repair_decision"]
        },
        "blocked_guardrail" => vec!["hold_decision"],
        _ => Vec::new(),
    }
}

fn authority_readiness(
    experiment: &Value,
    runs: &[Value],
    gate_rows: &[Value],
    conveyor_stage: &str,
    proposed_next: &str,
    source_refs: &[String],
) -> Value {
    let experiment_id = experiment
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("current");
    let artifact_refs = artifact_ref_candidates(experiment, runs);
    let mut missing = Vec::<String>::new();
    if !valid_charter(experiment.get("charter_v1")) {
        missing.push("lifecycle_valid_charter".to_string());
    }
    if !authority_has_read_only_rehearsal(runs) {
        missing.push("read_only_rehearsal".to_string());
    }
    if !meaningful_evidence(experiment.get("evidence_v1")) {
        missing.push("meaningful_evidence".to_string());
    }
    if artifact_refs.is_empty() {
        missing.push("artifact_grounding_refs".to_string());
    }
    if authority_guardrail_hold_active_value(experiment) {
        missing.push("no_active_guardrail_hold".to_string());
    }
    let rows = gate_rows
        .iter()
        .filter(|row| row.get("experiment_id").and_then(Value::as_str) == Some(experiment_id))
        .cloned()
        .collect::<Vec<_>>();
    let latest_request = rows
        .iter()
        .rev()
        .find(|row| row.get("record_type").and_then(Value::as_str) == Some("request"));
    let latest_request_id = latest_request
        .and_then(|row| row.get("request_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let budget_status = authority_budget_status(&rows);
    let mut token_status = authority_token_status(&rows, latest_request_id);
    if budget_status.get("stage").and_then(Value::as_str) == Some("review_required") {
        token_status = "review_required".to_string();
    }
    let stage = authority_readiness_stage(
        experiment,
        conveyor_stage,
        &missing,
        latest_request,
        &token_status,
    );
    let request_scaffold = (stage == "ready_to_author_request").then(|| {
        format!(
            "EXPERIMENT_AUTHORITY_REQUEST {experiment_id} :: scope: semantic_microdose; payload: ...; reason: ...; artifact_refs: {}; stop_criteria: ...",
            artifact_refs.join(", ")
        )
    });
    let next_safe_command = authority_readiness_next_command(
        experiment_id,
        &stage,
        proposed_next,
        latest_request_id,
        request_scaffold.as_deref(),
    );
    json!({
        "policy": "authority_readiness_v1",
        "scope": "semantic_microdose",
        "stage": stage,
        "eligible_to_request": stage == "ready_to_author_request",
        "missing_requirements": missing,
        "artifact_ref_candidates": artifact_refs,
        "latest_request_id": if latest_request_id.is_empty() { Value::Null } else { json!(latest_request_id) },
        "token_status": token_status,
        "next_safe_command": next_safe_command,
        "request_scaffold": request_scaffold,
        "budget_request_scaffold": if stage == "ready_to_author_request" {
            json!(format!(
                "EXPERIMENT_AUTHORITY_BUDGET_REQUEST {experiment_id} :: scope: semantic_microdose; purpose: ...; max_sends: 3; ttl_secs: 21600; artifact_refs: {}; stop_criteria: ...",
                artifact_refs.join(", ")
            ))
        } else {
            Value::Null
        },
        "authority_budget_v1": budget_status,
        "source_refs": source_refs,
        "authority_boundary": authority_boundary(),
    })
}

fn artifact_ref_candidates(experiment: &Value, runs: &[Value]) -> Vec<String> {
    let mut candidates = Vec::<String>::new();
    if let Some(items) = experiment
        .get("evidence_v1")
        .and_then(|evidence| evidence.get("artifact_refs"))
        .and_then(Value::as_array)
    {
        for item in items {
            push_artifact_value(&mut candidates, item);
        }
    }
    if let Some(items) = experiment
        .get("evidence_v1")
        .and_then(|evidence| evidence.get("felt_observations"))
        .and_then(Value::as_array)
    {
        for item in items {
            for key in ["note", "felt", "summary"] {
                if let Some(text) = item.get(key).and_then(Value::as_str) {
                    candidates.extend(scan_artifact_text(text));
                }
            }
        }
    }
    for run in runs {
        if let Some(items) = run.get("artifacts").and_then(Value::as_array) {
            for item in items {
                push_artifact_value(&mut candidates, item);
            }
        }
        for key in ["result_summary", "interpretation"] {
            if let Some(text) = run.get(key).and_then(Value::as_str) {
                candidates.extend(scan_artifact_text(text));
            }
        }
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

fn push_artifact_value(candidates: &mut Vec<String>, value: &Value) {
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

fn scan_artifact_text(text: &str) -> Vec<String> {
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

fn authority_has_read_only_rehearsal(runs: &[Value]) -> bool {
    runs.iter().any(|run| {
        let source = run
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let status = run
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let stage = run
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let action_text = run
            .get("action_text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_uppercase();
        (source.contains("experiment_rehearse") && !status.contains("blocked"))
            || (action_text.contains("ACTION_PREFLIGHT")
                && matches!(stage.as_str(), "read_only" | "protected" | "preflight"))
    })
}

fn authority_guardrail_hold_active_value(experiment: &Value) -> bool {
    experiment
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case("paused"))
        && experiment
            .get("planned_next")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .starts_with("THREAD_STATUS")
        && experiment
            .get("success_observation")
            .and_then(Value::as_str)
            .map(str::to_ascii_lowercase)
            .is_some_and(|text| {
                text.contains("held:") || text.contains("guard") || text.contains("hold")
            })
}

fn authority_token_status(rows: &[Value], request_id: &str) -> String {
    if request_id.is_empty() {
        return "none".to_string();
    }
    let approval = rows.iter().rev().find(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("steward_approval")
            && row.get("request_id").and_then(Value::as_str) == Some(request_id)
            && row.get("token_status").and_then(Value::as_str) == Some("active")
    });
    if let Some(approval) = approval {
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
            .and_then(|budget_id| pending_budget_review(rows, budget_id))
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
    experiment: &Value,
    conveyor_stage: &str,
    missing: &[String],
    latest_request: Option<&Value>,
    token_status: &str,
) -> String {
    if authority_guardrail_hold_active_value(experiment) {
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
    if missing.iter().any(|item| item == "lifecycle_valid_charter")
        || conveyor_stage == "needs_charter"
    {
        return "needs_charter".to_string();
    }
    if missing.iter().any(|item| item == "read_only_rehearsal")
        || conveyor_stage == "needs_rehearsal"
    {
        return "needs_rehearsal".to_string();
    }
    if missing.iter().any(|item| item == "meaningful_evidence")
        || conveyor_stage == "needs_evidence"
    {
        return "needs_evidence".to_string();
    }
    if missing.iter().any(|item| item == "artifact_grounding_refs") {
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

fn authority_budget_status(rows: &[Value]) -> Value {
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
        .then(|| active_budget(rows, experiment_id))
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

fn active_budget(rows: &[Value], experiment_id: &str) -> Option<Value> {
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
        let max_sends = row.get("max_sends").and_then(Value::as_u64).unwrap_or(3);
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
                pending_budget_review(rows, budget_id).map_or(Value::Null, Value::String),
            );
        }
        Some(active)
    })
}

fn pending_budget_review(rows: &[Value], budget_id: &str) -> Option<String> {
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

fn read_voice_health(workspace: &Path) -> Option<Value> {
    let path = workspace.join("diagnostics/voice_health.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
}

fn charter_missing_fields(charter: Option<&Value>) -> Vec<&'static str> {
    let Some(charter) = charter else {
        return vec!["hypothesis", "proposed_next_action", "evidence_targets"];
    };
    let mut missing = Vec::new();
    if !meaningful_text(charter.get("hypothesis")) {
        missing.push("hypothesis");
    }
    if !meaningful_text(charter.get("proposed_next_action")) {
        missing.push("proposed_next_action");
    }
    let targets_present = charter
        .get("evidence_targets")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| meaningful_text(Some(item))));
    if !targets_present {
        missing.push("evidence_targets");
    }
    missing
}

fn valid_charter(charter: Option<&Value>) -> bool {
    charter_missing_fields(charter).is_empty()
}

fn meaningful_evidence(evidence: Option<&Value>) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    for key in ["felt_observations", "telemetry_snapshots", "artifact_refs"] {
        if evidence
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            return true;
        }
    }
    false
}

fn meaningful_text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty() && text.trim() != "...")
}

fn decision_guardrail(experiment: &Value) -> Option<Value> {
    if let Some(existing) = experiment.get("decision_guardrail_v1")
        && existing.is_object()
    {
        return Some(existing.clone());
    }
    let decisions = experiment
        .get("evidence_v1")
        .and_then(|evidence| evidence.get("decisions"))
        .and_then(Value::as_array)?;
    if let Some(decision) = decisions.iter().next_back() {
        let status = decision
            .get("guardrail_status")
            .or_else(|| decision.get("decision_status"))
            .and_then(Value::as_str)?;
        return Some(json!({
            "schema_version": 1,
            "status": status,
            "outcome": decision.get("outcome"),
            "reason": decision.get("reason"),
            "source_pressure": decision.get("source_pressure"),
            "pressure_source": decision.get("pressure_source"),
            "pressure_terms": decision.get("pressure_terms").cloned().unwrap_or_else(|| json!([])),
            "authority_change": decision.get("authority_change").and_then(Value::as_bool).unwrap_or(false),
        }));
    }
    None
}

fn shared_investigation_for_experiment(experiment_id: &str) -> Option<Value> {
    shared_investigation::list().ok()?.into_iter().find(|row| {
        row.get("participants")
            .and_then(Value::as_array)
            .is_some_and(|participants| {
                participants.iter().any(|participant| {
                    participant.get("experiment_id").and_then(Value::as_str) == Some(experiment_id)
                })
            })
    })
}

fn sort_key(value: &Value) -> String {
    value
        .get("updated_at")
        .or_else(|| value.get("created_at"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn base_action(action: &str) -> String {
    action
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(':')
        .to_ascii_uppercase()
}

fn unique_dir(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    for suffix in 2_u32..1000 {
        let candidate = PathBuf::from(format!("{}_{}", path.display(), suffix));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

fn render_html(status: &Value) -> String {
    let systems = status
        .get("systems")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    let mut cards = String::new();
    for (name, system) in systems {
        let conveyor = system.get("conveyor_v1").unwrap_or(&Value::Null);
        let stage = conveyor
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let next = conveyor
            .get("proposed_next")
            .and_then(Value::as_str)
            .unwrap_or("THREAD_STATUS current");
        let return_kind = conveyor
            .get("return_kind")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let marker = conveyor
            .get("decision_guardrail_v1")
            .and_then(|guardrail| guardrail.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        let preview_allowed = conveyor
            .get("preview_allowed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let apply_policy = conveyor
            .get("apply_policy")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let can_apply = conveyor
            .get("can_apply")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let blocked = conveyor
            .get("apply_blocked_reason")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let readiness = conveyor
            .get("authority_readiness_v1")
            .unwrap_or(&Value::Null);
        let readiness_stage = readiness
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let readiness_next = readiness
            .get("next_safe_command")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        let token_status = readiness
            .get("token_status")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let budget = readiness.get("authority_budget_v1").unwrap_or(&Value::Null);
        let budget_stage = budget
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("no_budget");
        let remaining = budget
            .get("remaining_sends")
            .map(Value::to_string)
            .unwrap_or_else(|| "0".to_string());
        let missing = readiness
            .get("missing_requirements")
            .map(Value::to_string)
            .unwrap_or_else(|| "[]".to_string());
        let memory = conveyor.get("being_memory_v1").unwrap_or(&Value::Null);
        let memory_count = memory
            .get("card_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let consequence = conveyor
            .get("latest_authority_consequence_v1")
            .and_then(|row| row.get("consequence_status"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        let _ = write!(
            cards,
            "<section><h2>{}</h2><p><strong>Stage:</strong> {}</p><p><strong>Return Kind:</strong> {}</p><p><strong>Preview Allowed:</strong> {}</p><p><strong>Apply Policy:</strong> {}</p><p><strong>Can Apply:</strong> {}</p><p><strong>Apply Blocked:</strong> {}</p><p><strong>Memory Cards:</strong> {}</p><p><strong>Latest Consequence:</strong> {}</p><p><strong>Guardrail:</strong> {}</p><p><strong>Next:</strong> <code>{}</code></p><p><strong>Authority Readiness:</strong> {} token={}</p><p><strong>Budget:</strong> {} remaining={}</p><p><strong>Authority Missing:</strong> <code>{}</code></p><p><strong>Authority Next:</strong> <code>{}</code></p></section>",
            escape_html(&name),
            escape_html(stage),
            escape_html(return_kind),
            preview_allowed,
            escape_html(apply_policy),
            can_apply,
            escape_html(blocked),
            memory_count,
            escape_html(consequence),
            escape_html(marker),
            escape_html(next),
            escape_html(readiness_stage),
            escape_html(token_status),
            escape_html(budget_stage),
            escape_html(&remaining),
            escape_html(&missing),
            escape_html(readiness_next)
        );
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Experiment Conveyor</title><style>body{{font-family:system-ui,sans-serif;margin:32px;line-height:1.45}}section{{border:1px solid #bbb;padding:16px;margin:12px 0;border-radius:8px}}code{{white-space:pre-wrap}}</style></head><body><h1>Experiment Conveyor</h1><p><strong>Authority Boundary:</strong> {}</p>{}<h2>Raw JSON</h2><pre>{}</pre></body></html>",
        escape_html(authority_boundary()),
        cards,
        escape_html(&serde_json::to_string_pretty(status).unwrap_or_default())
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn authority_boundary() -> &'static str {
    "Read-only visibility only; preview is safe, apply is conservative local continuity only, and no bind, resume, perturb, live control, or peer mutation authority is granted."
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "experiment_conveyor_{}_{}",
            name,
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn status_reads_charter_repair_pause_without_mutation() {
        let root = temp_dir("status");
        let minime_workspace = root.join("minime_workspace");
        let bridge_workspace = root.join("bridge_workspace");
        let thread_dir = minime_workspace.join("action_threads/threads/th_minime_test");
        fs::create_dir_all(&thread_dir).unwrap();
        fs::create_dir_all(bridge_workspace.join("action_threads/threads")).unwrap();
        fs::write(
            thread_dir.join("thread.json"),
            serde_json::to_string_pretty(&json!({
                "thread_id": "th_minime_test",
                "title": "Lambda conveyor",
                "updated_at": "2099-01-01T00:00:00Z",
                "active_experiment_id": null,
                "current_next": "EXPERIMENT_PLAN current",
                "experiment_summary": {
                    "experiment_id": "exp_minime_gap",
                    "status": "paused",
                    "planned_next": "EXPERIMENT_CHARTER exp_minime_gap :: hypothesis: ..."
                }
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiments.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_gap",
                    "title": "Gap",
                    "question": "Repair?",
                    "status": "paused",
                    "planned_next": "EXPERIMENT_CHARTER exp_minime_gap :: hypothesis: ..."
                })
            ),
        )
        .unwrap();
        fs::write(thread_dir.join("experiment_runs.jsonl"), "").unwrap();

        let status = status_from_paths(&minime_workspace, &bridge_workspace).unwrap();
        let conveyor = &status["systems"]["minime"]["conveyor_v1"];

        assert_eq!(conveyor["stage"], "paused_repair");
        assert_eq!(conveyor["return_kind"], "charter_repair");
        assert_eq!(conveyor["preview_allowed"], true);
        assert_eq!(conveyor["apply_policy"], "conservative_local_v1");
        assert_eq!(conveyor["can_apply"], false);
        assert_eq!(
            conveyor["apply_blocked_reason"],
            "paused_experiments_require_explicit_return_command"
        );
        assert!(
            conveyor["authority_boundary"]
                .as_str()
                .unwrap()
                .contains("preview is safe")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_reads_guarded_hold_pause_marker() {
        let root = temp_dir("hold");
        let minime_workspace = root.join("minime_workspace");
        let bridge_workspace = root.join("bridge_workspace");
        let thread_dir = minime_workspace.join("action_threads/threads/th_minime_hold");
        fs::create_dir_all(&thread_dir).unwrap();
        fs::create_dir_all(bridge_workspace.join("action_threads/threads")).unwrap();
        fs::write(
            thread_dir.join("thread.json"),
            serde_json::to_string_pretty(&json!({
                "thread_id": "th_minime_hold",
                "title": "Lambda hold",
                "updated_at": "2099-01-01T00:00:00Z",
                "active_experiment_id": null,
                "current_next": "EXPERIMENT_PLAN current — proposed_next_action: PERTURB SPREAD",
                "experiment_summary": {
                    "experiment_id": "exp_minime_hold",
                    "status": "paused",
                    "planned_next": "THREAD_STATUS current"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiments.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_hold",
                    "title": "Hold",
                    "question": "Can pressure become evidence?",
                    "status": "paused",
                    "planned_next": "THREAD_STATUS current",
                    "evidence_v1": {
                        "decisions": [{
                            "outcome": "hold",
                            "reason": "because pressure became evidence",
                            "guardrail_status": "soft_perturb_converted_to_hold",
                            "source_pressure": "EXPERIMENT_PLAN current — proposed_next_action: PERTURB SPREAD",
                            "pressure_terms": ["PERTURB"],
                            "authority_change": false
                        }]
                    }
                })
            ),
        )
        .unwrap();
        fs::write(thread_dir.join("experiment_runs.jsonl"), "").unwrap();

        let status = status_from_paths(&minime_workspace, &bridge_workspace).unwrap();
        let conveyor = &status["systems"]["minime"]["conveyor_v1"];

        assert_eq!(conveyor["stage"], "paused_repair");
        assert_eq!(conveyor["return_kind"], "hold");
        assert_eq!(conveyor["primary_return_next"], "THREAD_STATUS current");
        assert_eq!(conveyor["preview_allowed"], true);
        assert_eq!(conveyor["would_mutate"], false);
        assert_eq!(
            conveyor["decision_guardrail_v1"]["status"],
            "soft_perturb_converted_to_hold"
        );

        let html = render_html(&status);
        assert!(html.contains("soft_perturb_converted_to_hold"));
        assert!(html.contains("THREAD_STATUS current"));
        assert!(html.contains("conservative local continuity"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_demotes_paused_missing_charter_resume_to_repair() {
        let root = temp_dir("missing_charter_resume");
        let minime_workspace = root.join("minime_workspace");
        let bridge_workspace = root.join("bridge_workspace");
        let thread_dir = minime_workspace.join("action_threads/threads/th_minime_repair");
        fs::create_dir_all(&thread_dir).unwrap();
        fs::create_dir_all(bridge_workspace.join("action_threads/threads")).unwrap();
        fs::write(
            thread_dir.join("thread.json"),
            serde_json::to_string_pretty(&json!({
                "thread_id": "th_minime_repair",
                "title": "Lambda repair",
                "updated_at": "2099-01-01T00:00:00Z",
                "active_experiment_id": null,
                "current_next": "EXPERIMENT_RESUME exp_minime_repair",
                "experiment_summary": {
                    "experiment_id": "exp_minime_repair",
                    "status": "paused",
                    "planned_next": "EXPERIMENT_RESUME exp_minime_repair"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiments.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_repair",
                    "title": "Repair",
                    "question": "Should resume stay primary?",
                    "status": "paused",
                    "planned_next": "EXPERIMENT_RESUME exp_minime_repair"
                })
            ),
        )
        .unwrap();
        fs::write(thread_dir.join("experiment_runs.jsonl"), "").unwrap();

        let status = status_from_paths(&minime_workspace, &bridge_workspace).unwrap();
        let conveyor = &status["systems"]["minime"]["conveyor_v1"];

        assert_eq!(conveyor["stage"], "paused_repair");
        assert_eq!(conveyor["return_kind"], "charter_repair");
        assert!(
            conveyor["primary_return_next"]
                .as_str()
                .unwrap()
                .starts_with("EXPERIMENT_CHARTER exp_minime_repair")
        );
        assert!(
            conveyor["proposed_next"]
                .as_str()
                .unwrap()
                .starts_with("EXPERIMENT_CHARTER exp_minime_repair")
        );
        assert_eq!(
            conveyor["apply_blocked_reason"],
            "paused_experiments_require_explicit_return_command"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_surfaces_authority_readiness_request_scaffold() {
        let root = temp_dir("authority_ready");
        let minime_workspace = root.join("minime_workspace");
        let bridge_workspace = root.join("bridge_workspace");
        let thread_dir = minime_workspace.join("action_threads/threads/th_minime_ready");
        fs::create_dir_all(&thread_dir).unwrap();
        fs::create_dir_all(bridge_workspace.join("action_threads/threads")).unwrap();
        fs::write(
            thread_dir.join("thread.json"),
            serde_json::to_string_pretty(&json!({
                "thread_id": "th_minime_ready",
                "title": "Authority ready",
                "updated_at": "2099-01-01T00:00:00Z",
                "active_experiment_id": "exp_minime_ready"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiments.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_ready",
                    "title": "Ready",
                    "question": "Can semantic authority be requested?",
                    "status": "active",
                    "charter_v1": {
                        "hypothesis": "a tiny semantic witness can be bounded",
                        "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                        "evidence_targets": ["artifact_grounding"]
                    },
                    "evidence_v1": {
                        "felt_observations": [{"note": "artifact_grounding: /tmp/semantic.json"}],
                        "telemetry_snapshots": [{"snapshot": {"fill": 68}}],
                        "artifact_refs": ["/tmp/semantic.json"]
                    }
                })
            ),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiment_runs.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_ready",
                    "source": "experiment_rehearse",
                    "action_text": "ACTION_PREFLIGHT DECOMPOSE",
                    "stage": "read_only",
                    "status": "rehearsed",
                    "artifacts": []
                })
            ),
        )
        .unwrap();

        let status = status_from_paths(&minime_workspace, &bridge_workspace).unwrap();
        let readiness = &status["systems"]["minime"]["conveyor_v1"]["authority_readiness_v1"];

        assert_eq!(readiness["stage"], "ready_to_author_request");
        assert_eq!(readiness["eligible_to_request"], true);
        assert!(
            readiness["request_scaffold"]
                .as_str()
                .unwrap()
                .contains("EXPERIMENT_AUTHORITY_REQUEST exp_minime_ready")
        );
        assert!(render_html(&status).contains("ready_to_author_request"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_surfaces_authority_active_token_as_bridge_executable() {
        let root = temp_dir("authority_token");
        let minime_workspace = root.join("minime_workspace");
        let bridge_workspace = root.join("bridge_workspace");
        let thread_dir = minime_workspace.join("action_threads/threads/th_minime_token");
        fs::create_dir_all(&thread_dir).unwrap();
        fs::create_dir_all(bridge_workspace.join("action_threads/threads")).unwrap();
        fs::write(
            thread_dir.join("thread.json"),
            serde_json::to_string_pretty(&json!({
                "thread_id": "th_minime_token",
                "title": "Authority token",
                "updated_at": "2099-01-01T00:00:00Z",
                "active_experiment_id": "exp_minime_token"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiments.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_token",
                    "title": "Token",
                    "question": "Can a token be seen?",
                    "status": "active",
                    "charter_v1": {
                        "hypothesis": "bounded semantic write",
                        "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                        "evidence_targets": ["artifact_grounding"]
                    },
                    "evidence_v1": {
                        "artifact_refs": ["/tmp/semantic.json"]
                    }
                })
            ),
        )
        .unwrap();
        fs::write(
            thread_dir.join("experiment_runs.jsonl"),
            format!(
                "{}\n",
                json!({
                    "experiment_id": "exp_minime_token",
                    "source": "experiment_rehearse",
                    "action_text": "ACTION_PREFLIGHT DECOMPOSE",
                    "stage": "read_only",
                    "status": "rehearsed",
                    "artifacts": []
                })
            ),
        )
        .unwrap();
        fs::write(
            thread_dir.join("authority_gate.jsonl"),
            format!(
                "{}\n{}\n",
                json!({
                    "record_schema": "authority_gate_v1",
                    "record_type": "request",
                    "request_id": "authreq_token",
                    "experiment_id": "exp_minime_token",
                    "status": "pending_steward_approval",
                    "artifact_refs": ["/tmp/semantic.json"],
                    "eligibility_v1": {"eligible": true, "missing_requirements": []}
                }),
                json!({
                    "record_schema": "authority_gate_v1",
                    "record_type": "steward_approval",
                    "request_id": "authreq_token",
                    "experiment_id": "exp_minime_token",
                    "token_id": "authtok_token",
                    "token_status": "active",
                    "expires_at_unix_s": 4102444800u64
                })
            ),
        )
        .unwrap();

        let status = status_from_paths(&minime_workspace, &bridge_workspace).unwrap();
        let readiness = &status["systems"]["minime"]["conveyor_v1"]["authority_readiness_v1"];

        assert_eq!(readiness["stage"], "token_active_bridge_executable");
        assert_eq!(readiness["token_status"], "active");
        assert_eq!(
            readiness["next_safe_command"],
            "EXPERIMENT_AUTHORITY_STATUS authreq_token"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn render_html_and_json_include_authority_boundary() {
        let root = temp_dir("render");
        let output = root.join("out");
        let status = json!({
            "schema_version": 1,
            "policy": "experiment_conveyor_visibility_v1",
            "authority_boundary": authority_boundary(),
            "systems": {
                "minime": {
                    "conveyor_v1": {
                        "stage": "needs_evidence",
                        "return_kind": "hold",
                        "preview_allowed": true,
                        "apply_policy": "conservative_local_v1",
                        "can_apply": true,
                        "would_mutate": false,
                        "decision_guardrail_v1": {
                            "status": "soft_perturb_converted_to_hold"
                        },
                        "proposed_next": "EXPERIMENT_EVIDENCE exp_minime_test :: artifact_grounding ..."
                    }
                }
            }
        });
        let artifact = render_status_to_base(status, Some(&output)).unwrap();

        assert!(artifact.json_path.exists());
        assert!(artifact.index_html.exists());
        assert!(
            fs::read_to_string(&artifact.index_html)
                .unwrap()
                .contains("Authority Boundary")
        );
        assert!(
            fs::read_to_string(&artifact.index_html)
                .unwrap()
                .contains("needs_evidence")
        );
        assert!(
            fs::read_to_string(&artifact.index_html)
                .unwrap()
                .contains("soft_perturb_converted_to_hold")
        );
        assert!(
            fs::read_to_string(&artifact.index_html)
                .unwrap()
                .contains("conservative_local_v1")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_surfaces_astrid_voice_health_diagnostic() {
        let root = temp_dir("voice_health");
        let minime_workspace = root.join("minime_workspace");
        let bridge_workspace = root.join("bridge_workspace");
        fs::create_dir_all(minime_workspace.join("action_threads/threads")).unwrap();
        let thread_dir = bridge_workspace.join("action_threads/threads/th_astrid_voice");
        fs::create_dir_all(&thread_dir).unwrap();
        fs::create_dir_all(bridge_workspace.join("diagnostics")).unwrap();
        fs::write(
            bridge_workspace.join("diagnostics/voice_health.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": 1,
                "policy": "voice_health_v1",
                "status": "degraded_voice",
                "fallback_count": 3,
                "suggested_read_only_repair": "REPAIR_STATUS or CAPABILITY_STATUS",
                "authority_boundary": "diagnostic only"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            thread_dir.join("thread.json"),
            serde_json::to_string_pretty(&json!({
                "thread_id": "th_astrid_voice",
                "title": "Voice diagnostics",
                "updated_at": "2099-01-01T00:00:00Z",
                "current_next": "REPAIR_STATUS"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(thread_dir.join("experiments.jsonl"), "").unwrap();
        fs::write(thread_dir.join("experiment_runs.jsonl"), "").unwrap();

        let status = status_from_paths(&minime_workspace, &bridge_workspace).unwrap();
        assert_eq!(
            status["systems"]["astrid"]["voice_health_v1"]["status"],
            "degraded_voice"
        );
        assert_eq!(
            status["systems"]["astrid"]["voice_health_v1"]["fallback_count"],
            3
        );

        let html = render_html(&status);
        assert!(html.contains("voice_health_v1"));
        assert!(html.contains("degraded_voice"));

        let _ = fs::remove_dir_all(root);
    }
}
