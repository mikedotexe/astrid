//! Capability self-map and append-only continuity repair for Astrid.

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};

const SCHEMA_VERSION: u32 = 1;
const SYSTEM: &str = "astrid";
const LOCAL_EXPERIMENT_PREFIX: &str = "exp_astrid_";
const PEER_SNAPSHOT_PATH: &str =
    "/Users/v/other/minime/workspace/action_threads/capability_map.json";

const CAPABILITY_ACTIONS: &[&str] = &[
    "FACULTIES",
    "CAPABILITY_MAP",
    "CAPABILITY_STATUS",
    "CAPABILITY_DIFF",
];

const JOB_STATUS_ACTIONS: &[&str] = &["ACTION_STATUS", "JOB_STATUS", "ACTION_CANCEL"];

const REPAIR_ACTIONS: &[&str] = &[
    "REPAIR_STATUS",
    "REPAIR_SWEEP",
    "REPAIR_RECORD",
    "REPAIR_APPLY",
];

const OVERRIDE_ALLOWED: &[&str] = &[
    "ACTION_PREFLIGHT",
    "ACTION_STATUS",
    "BROWSE",
    "CAPABILITY_DIFF",
    "CAPABILITY_MAP",
    "CAPABILITY_STATUS",
    "CLOSE_EARS",
    "CLOSE_EYES",
    "EXPERIMENT_CHARTER",
    "EXPERIMENT_DECIDE",
    "EXPERIMENT_EVIDENCE",
    "EXPERIMENT_OBSERVE",
    "EXPERIMENT_PEER_REVIEW",
    "EXPERIMENT_PLAN",
    "EXPERIMENT_PREFLIGHT",
    "EXPERIMENT_REHEARSE",
    "EXPERIMENT_REVIEW",
    "EXPERIMENT_START",
    "EXPERIMENT_STATUS",
    "FACULTIES",
    "INTROSPECT",
    "JOB_STATUS",
    "NEXT_PROBE",
    "NOTICE",
    "OPEN_EARS",
    "OPEN_EYES",
    "PASS",
    "PREFLIGHT",
    "PROBE_ACTION",
    "ACTION_CANCEL",
    "READ_MORE",
    "RECALL",
    "REPAIR_RECORD",
    "REPAIR_STATUS",
    "REPAIR_SWEEP",
    "REST",
    "SEARCH",
    "SELF_STUDY",
    "SHUT_EARS",
    "SHUT_EYES",
    "SPACE_HOLD",
    "SPECTRAL_EXPLORER",
    "THREADS",
    "THREAD_STATUS",
];

#[derive(Debug, Clone)]
struct RepairCandidate {
    repair_id: String,
    thread_id: String,
    target_id: String,
    superseded_by: Option<String>,
    reasons: Vec<String>,
    source_line: usize,
    candidate_record: Value,
}

pub fn handle_action(root: &Path, base_action: &str, original: &str) -> Result<Option<String>> {
    let base = base_action.to_ascii_uppercase();
    let arg = strip_action_arg(original);
    if CAPABILITY_ACTIONS.contains(&base.as_str()) {
        return Ok(Some(handle_capability(root, &base, &arg)?));
    }
    if JOB_STATUS_ACTIONS.contains(&base.as_str()) {
        return Ok(None);
    }
    if REPAIR_ACTIONS.contains(&base.as_str()) {
        return Ok(Some(handle_repair(root, &base, &arg)?));
    }
    Ok(None)
}

pub fn capability_snapshot(root: &Path) -> Result<Value> {
    let actions = capability_specs()
        .into_iter()
        .map(action_metadata)
        .collect::<Vec<_>>();
    let snapshot = json!({
        "schema_version": SCHEMA_VERSION,
        "policy": "capability_self_map_v1",
        "system": SYSTEM,
        "generated_at": iso_now(),
        "actions": actions,
        "summary": {
            "count": actions.len(),
            "read_only": actions.iter().filter(|item| item["stage"] == "read_only").count(),
            "live_write": actions.iter().filter(|item| item["stage"] == "live_write").count(),
            "live_control": actions.iter().filter(|item| item["stage"] == "live_control").count(),
            "override_allowed": actions.iter().filter(|item| item["operator_override"]["allowed"] == true).count(),
        },
    });
    write_json(&root.join("capability_map.json"), &snapshot)?;
    Ok(snapshot)
}

fn handle_capability(root: &Path, base: &str, arg: &str) -> Result<String> {
    let snapshot = capability_snapshot(root)?;
    Ok(match base {
        "FACULTIES" | "CAPABILITY_MAP" => render_capability_map(&snapshot),
        "CAPABILITY_STATUS" => render_capability_status(&snapshot, arg),
        "CAPABILITY_DIFF" => render_capability_diff(&snapshot)?,
        _ => format!("Unknown capability action `{base}`."),
    })
}

fn handle_repair(root: &Path, base: &str, arg: &str) -> Result<String> {
    Ok(match base {
        "REPAIR_STATUS" => render_repair_status(root)?,
        "REPAIR_SWEEP" => render_repair_sweep(root, arg)?,
        "REPAIR_RECORD" => render_repair_record(root, arg)?,
        "REPAIR_APPLY" => apply_repair(root, arg)?,
        _ => format!("Unknown repair action `{base}`."),
    })
}

fn render_capability_map(snapshot: &Value) -> String {
    let mut groups: HashMap<String, Vec<&Value>> = HashMap::new();
    if let Some(actions) = snapshot["actions"].as_array() {
        for action in actions {
            let key = action["authority_class"]
                .as_str()
                .unwrap_or("observer")
                .to_string();
            groups.entry(key).or_default().push(action);
        }
    }
    let mut lines = vec![
        "=== CAPABILITY MAP V1 ===".to_string(),
        "Descriptive only: this map grants no authority and bypasses no gates.".to_string(),
        format!("System: {}", snapshot["system"].as_str().unwrap_or(SYSTEM)),
        format!(
            "Generated: {}",
            snapshot["generated_at"].as_str().unwrap_or("(unknown)")
        ),
    ];
    for group in [
        "read_only",
        "protected_read_only",
        "continuity_metadata_write",
        "live_write",
        "live_control",
        "observer",
    ] {
        if let Some(items) = groups.get(group) {
            if items.is_empty() {
                continue;
            }
            lines.push(format!("\n{group}:"));
            for item in items {
                let aliases = item["aliases"]
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let alias_text = if aliases.is_empty() {
                    String::new()
                } else {
                    format!(" aliases={aliases}")
                };
                lines.push(format!(
                    "- {} -> {} stage={} visibility={} override={}{}",
                    item["base"].as_str().unwrap_or("UNKNOWN"),
                    item["route"].as_str().unwrap_or("unwired"),
                    item["stage"].as_str().unwrap_or("observe"),
                    item["visibility"].as_str().unwrap_or("summary"),
                    item["operator_override"]["allowed"]
                        .as_bool()
                        .unwrap_or(false),
                    alias_text
                ));
            }
        }
    }
    lines.push(
        "\nUse CAPABILITY_STATUS <action> for one action or CAPABILITY_DIFF peer for parity."
            .to_string(),
    );
    lines.join("\n")
}

fn render_capability_status(snapshot: &Value, selector: &str) -> String {
    let needle = base_action(selector);
    if needle.is_empty() {
        return "CAPABILITY_STATUS needs an action name, for example CAPABILITY_STATUS EXPERIMENT_START.".to_string();
    }
    if let Some(actions) = snapshot["actions"].as_array() {
        for action in actions {
            let base = action["base"].as_str().unwrap_or_default();
            let aliases = action["aliases"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if needle == base || aliases.iter().any(|alias| needle == alias.as_str()) {
                return format!(
                    "=== CAPABILITY STATUS V1 ===\nAction: {base}\nAliases: {}\nRoute: {}\nStage: {}\nVisibility: {}\nAuthority class: {}\nStable-core availability: {}\nOperator override: {} ({})\nContinuity effect: {}\nExpected artifacts: {}\nKnown tests: {}",
                    aliases.join(", "),
                    action["route"].as_str().unwrap_or("unwired"),
                    action["stage"].as_str().unwrap_or("observe"),
                    action["visibility"].as_str().unwrap_or("summary"),
                    action["authority_class"].as_str().unwrap_or("observer"),
                    action["stable_core"]["availability"]
                        .as_str()
                        .unwrap_or("normal gates apply"),
                    action["operator_override"]["allowed"]
                        .as_bool()
                        .unwrap_or(false),
                    action["operator_override"]["note"].as_str().unwrap_or(""),
                    action["continuity_effect"]
                        .as_str()
                        .unwrap_or("records continuity if executed"),
                    join_array(&action["expected_artifacts"]),
                    join_array(&action["known_tests"])
                );
            }
        }
    }
    format!(
        "Unknown capability `{needle}`. It would route as an unwired proposal if chosen. Use CAPABILITY_MAP or ACTION_PREFLIGHT <action>."
    )
}

fn render_capability_diff(snapshot: &Value) -> Result<String> {
    let path = Path::new(PEER_SNAPSHOT_PATH);
    if !path.exists() {
        return Ok(format!(
            "=== CAPABILITY DIFF V1 ===\nNo peer capability snapshot found at {}.\nAsk Minime to run FACULTIES or CAPABILITY_MAP, then retry CAPABILITY_DIFF peer.",
            path.display()
        ));
    }
    let peer = read_json(path)?;
    let local = action_map_by_base(snapshot);
    let remote = action_map_by_base(&peer);
    let local_keys = local.keys().cloned().collect::<HashSet<_>>();
    let remote_keys = remote.keys().cloned().collect::<HashSet<_>>();
    let mut only_local = local_keys
        .difference(&remote_keys)
        .cloned()
        .collect::<Vec<_>>();
    let mut only_peer = remote_keys
        .difference(&local_keys)
        .cloned()
        .collect::<Vec<_>>();
    only_local.sort();
    only_peer.sort();
    let mut mismatches = Vec::new();
    for key in local_keys.intersection(&remote_keys) {
        if let (Some(left), Some(right)) = (local.get(key), remote.get(key)) {
            let mut parts = Vec::new();
            for field in ["authority_class", "stage", "visibility"] {
                if left[field] != right[field] {
                    parts.push(format!(
                        "{field}: local={} peer={}",
                        left[field].as_str().unwrap_or("?"),
                        right[field].as_str().unwrap_or("?")
                    ));
                }
            }
            if left["operator_override"]["allowed"] != right["operator_override"]["allowed"] {
                parts.push(format!(
                    "override: local={} peer={}",
                    left["operator_override"]["allowed"]
                        .as_bool()
                        .unwrap_or(false),
                    right["operator_override"]["allowed"]
                        .as_bool()
                        .unwrap_or(false)
                ));
            }
            if !parts.is_empty() {
                mismatches.push(format!("- {key}: {}", parts.join("; ")));
            }
        }
    }
    let mut lines = vec![
        "=== CAPABILITY DIFF V1 ===".to_string(),
        format!(
            "Local: {} Peer: {}",
            snapshot["system"].as_str().unwrap_or(SYSTEM),
            peer["system"].as_str().unwrap_or("unknown")
        ),
    ];
    if !only_local.is_empty() {
        lines.push(format!("Only local: {}", only_local.join(", ")));
    }
    if !only_peer.is_empty() {
        lines.push(format!("Only peer: {}", only_peer.join(", ")));
    }
    if !mismatches.is_empty() {
        lines.push(format!("Mismatches:\n{}", mismatches.join("\n")));
    }
    if lines.len() == 2 {
        lines.push("No capability mismatches found in the latest snapshots.".to_string());
    }
    Ok(lines.join("\n"))
}

fn render_repair_status(root: &Path) -> Result<String> {
    let candidates = repair_sweep(root)?;
    let ledgers = read_jsonl(&root.join("repairs.jsonl"));
    let mut lines = vec![
        "=== REPAIR STATUS V1 ===".to_string(),
        "Append-only repair is available for malformed continuity records; history is never deleted."
            .to_string(),
        format!("Pending candidates: {}", candidates.len()),
        format!("Recent applied repairs: {}", ledgers.len()),
    ];
    for row in ledgers.iter().rev().take(8) {
        lines.push(format!(
            "- {} status={} target={} superseded_by={}",
            row["repair_id"].as_str().unwrap_or("(unknown)"),
            row["status"].as_str().unwrap_or("(unknown)"),
            row["target_id"].as_str().unwrap_or("(unknown)"),
            row["superseded_by"].as_str().unwrap_or("(none)")
        ));
    }
    lines.push("Use REPAIR_SWEEP experiments to dry-run or REPAIR_APPLY <repair_id|all> to append supersession records.".to_string());
    Ok(lines.join("\n"))
}

fn render_repair_sweep(root: &Path, _scope: &str) -> Result<String> {
    let candidates = repair_sweep(root)?;
    if candidates.is_empty() {
        return Ok("=== REPAIR SWEEP V1 ===\nNo repair candidates found.".to_string());
    }
    let mut lines = vec![
        "=== REPAIR SWEEP V1 ===".to_string(),
        "Dry run only. REPAIR_APPLY appends repair_v1 records; it never rewrites JSONL history."
            .to_string(),
    ];
    for candidate in candidates {
        lines.push(format!(
            "- {} target={} thread={} superseded_by={} reason={}",
            candidate.repair_id,
            candidate.target_id,
            candidate.thread_id,
            candidate.superseded_by.as_deref().unwrap_or("(none)"),
            candidate.reasons.join("; ")
        ));
    }
    Ok(lines.join("\n"))
}

fn render_repair_record(root: &Path, selector: &str) -> Result<String> {
    let selector = selector.trim();
    for candidate in repair_sweep(root)? {
        if selector == candidate.repair_id || selector == candidate.target_id {
            return Ok(format!(
                "=== REPAIR RECORD V1 ===\n{}",
                serde_json::to_string_pretty(&candidate_to_value(&candidate))?
            ));
        }
    }
    for ledger in read_jsonl(&root.join("repairs.jsonl")) {
        if selector == ledger["repair_id"].as_str().unwrap_or_default()
            || selector == ledger["target_id"].as_str().unwrap_or_default()
        {
            return Ok(format!(
                "=== REPAIR LEDGER V1 ===\n{}",
                serde_json::to_string_pretty(&ledger)?
            ));
        }
    }
    Ok(format!(
        "No repair candidate or ledger record matched `{selector}`."
    ))
}

fn apply_repair(root: &Path, selector: &str) -> Result<String> {
    let selector = if selector.trim().is_empty() {
        "all"
    } else {
        selector.trim()
    };
    if matches!(selector, "experiments" | "threads") {
        return Ok(format!(
            "REPAIR_APPLY needs a repair id or `all`; `{selector}` is a dry-run scope."
        ));
    }
    let mut candidates = repair_sweep(root)?;
    if selector != "all" {
        candidates
            .retain(|candidate| selector == candidate.repair_id || selector == candidate.target_id);
    }
    if candidates.is_empty() {
        return Ok(format!(
            "No unapplied repair candidates matched `{selector}`."
        ));
    }
    let mut lines = vec!["=== REPAIR APPLY V1 ===".to_string()];
    for candidate in candidates {
        let ledger = apply_candidate(root, &candidate)?;
        lines.push(format!(
            "- {} retired {} superseded_by={}",
            ledger["repair_id"].as_str().unwrap_or("(unknown)"),
            ledger["target_id"].as_str().unwrap_or("(unknown)"),
            ledger["superseded_by"].as_str().unwrap_or("(none)")
        ));
    }
    Ok(lines.join("\n"))
}

fn repair_sweep(root: &Path) -> Result<Vec<RepairCandidate>> {
    let applied_targets = read_jsonl(&root.join("repairs.jsonl"))
        .into_iter()
        .filter(|row| row["status"] == "applied")
        .filter_map(|row| row["target_id"].as_str().map(str::to_string))
        .collect::<HashSet<_>>();
    let mut candidates = Vec::new();
    for thread_path in thread_json_paths(root)? {
        let thread = read_json(&thread_path)?;
        let thread_id = thread["thread_id"].as_str().unwrap_or_default().to_string();
        let experiments = read_experiment_rows(root, &thread_id);
        let mut latest: HashMap<String, (usize, Value)> = HashMap::new();
        for (line_no, row) in experiments {
            if let Some(id) = row["experiment_id"].as_str() {
                latest.insert(id.to_string(), (line_no, row));
            }
        }
        let active_ids = latest
            .iter()
            .filter(|(_, (_, row))| {
                let status = row["status"].as_str().unwrap_or("active");
                matches!(status, "active" | "paused")
            })
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        for (experiment_id, (line_no, row)) in latest {
            if applied_targets.contains(&experiment_id)
                || !row["repair_v1"].is_null()
                || !matches!(
                    row["status"].as_str().unwrap_or("active"),
                    "active" | "paused"
                )
            {
                continue;
            }
            let mut reasons = Vec::new();
            if experiment_id.contains("exp-astrid") {
                reasons.push("experiment_id contains dashed local prefix".to_string());
            }
            let blob = format!(
                "{} {} {} {}",
                row["title"].as_str().unwrap_or_default(),
                row["question"].as_str().unwrap_or_default(),
                row["planned_next"].as_str().unwrap_or_default(),
                row["success_observation"].as_str().unwrap_or_default()
            );
            let embedded = embedded_local_experiment_id(&blob, &experiment_id);
            if embedded.is_some() {
                reasons.push("title_or_question_embeds_local_experiment_id".to_string());
            }
            if reasons.is_empty() {
                continue;
            }
            let superseded_by = embedded.filter(|id| active_ids.contains(id));
            candidates.push(RepairCandidate {
                repair_id: format!("repair_{SYSTEM}_{}", slug(&experiment_id)),
                thread_id: thread_id.clone(),
                target_id: experiment_id,
                superseded_by,
                reasons,
                source_line: line_no,
                candidate_record: row,
            });
        }
    }
    Ok(candidates)
}

fn apply_candidate(root: &Path, candidate: &RepairCandidate) -> Result<Value> {
    let now = iso_now();
    let mut record = candidate.candidate_record.clone();
    record["status"] = json!("retired");
    record["updated_at"] = json!(now);
    if let Some(superseded_by) = &candidate.superseded_by {
        record["planned_next"] = json!(format!("EXPERIMENT_STATUS {superseded_by}"));
        record["superseded_by"] = json!(superseded_by);
    }
    record["repair_v1"] = json!({
        "schema_version": SCHEMA_VERSION,
        "policy": "continuity_repair_v1",
        "repair_id": candidate.repair_id,
        "superseded_by": candidate.superseded_by,
        "reasons": candidate.reasons,
        "source_line": candidate.source_line,
        "repaired_at": now,
    });
    append_jsonl(
        &thread_dir(root, &candidate.thread_id).join("experiments.jsonl"),
        &record,
    )?;
    update_thread_pointer(root, candidate, &now)?;
    let ledger = json!({
        "schema_version": SCHEMA_VERSION,
        "policy": "continuity_repair_v1",
        "repair_id": candidate.repair_id,
        "system": SYSTEM,
        "thread_id": candidate.thread_id,
        "target_kind": "experiment",
        "target_id": candidate.target_id,
        "superseded_by": candidate.superseded_by,
        "status": "applied",
        "reasons": candidate.reasons,
        "source_line": candidate.source_line,
        "applied_at": now,
    });
    append_jsonl(&root.join("repairs.jsonl"), &ledger)?;
    append_jsonl(
        &thread_dir(root, &candidate.thread_id).join("repairs.jsonl"),
        &ledger,
    )?;
    Ok(ledger)
}

fn update_thread_pointer(root: &Path, candidate: &RepairCandidate, now: &str) -> Result<()> {
    let path = thread_dir(root, &candidate.thread_id).join("thread.json");
    if !path.exists() {
        return Ok(());
    }
    let mut thread = read_json(&path)?;
    if thread["active_experiment_id"].as_str() != Some(candidate.target_id.as_str()) {
        return Ok(());
    }
    if let Some(superseded_by) = &candidate.superseded_by {
        thread["active_experiment_id"] = json!(superseded_by);
        thread["current_next"] = json!(format!("EXPERIMENT_STATUS {superseded_by}"));
        if let Some(summary) = latest_experiment(root, &candidate.thread_id, superseded_by) {
            thread["experiment_summary"] = experiment_summary(&summary);
        }
    } else {
        thread["active_experiment_id"] = Value::Null;
        thread["experiment_summary"] = Value::Null;
    }
    thread["updated_at"] = json!(now);
    write_json(&path, &thread)
}

fn capability_specs() -> Vec<Value> {
    vec![
        spec(
            "FACULTIES",
            &["CAPABILITY_MAP"],
            "action_continuity",
            "writes a capability snapshot and records a protected read-only action",
            &["tests: capability"],
        ),
        spec(
            "CAPABILITY_STATUS",
            &[],
            "action_continuity",
            "renders one action's route, gates, authority, artifacts, and tests",
            &["tests: capability"],
        ),
        spec(
            "CAPABILITY_DIFF",
            &[],
            "action_continuity",
            "compares local snapshot to the peer's latest local file snapshot",
            &["tests: capability"],
        ),
        spec(
            "ACTION_STATUS",
            &["JOB_STATUS"],
            "action_continuity",
            "reads durable LLM job progress without executing work",
            &["llm_jobs::tests"],
        ),
        spec(
            "ACTION_CANCEL",
            &[],
            "action_continuity",
            "requests best-effort cancellation for a queued/running LLM job",
            &["llm_jobs::tests"],
        ),
        spec(
            "ACTION_PREFLIGHT",
            &["NEXT_PROBE", "PREFLIGHT", "PROBE_ACTION"],
            "action_preflight",
            "records dry-run preflight report; never executes the inner action",
            &["next_action::tests"],
        ),
        spec(
            "THREAD_START",
            &[],
            "action_continuity",
            "creates/selects a durable action thread",
            &["action_continuity::tests"],
        ),
        spec(
            "THREAD_STATUS",
            &["THREADS"],
            "action_continuity",
            "renders recent events and the return point",
            &["action_continuity::tests"],
        ),
        spec(
            "THREAD_NOTE",
            &[],
            "action_continuity",
            "appends a protected note to the current/selected thread",
            &["action_continuity::tests"],
        ),
        spec(
            "RESUME",
            &["SAVEPOINT", "RECALL"],
            "action_continuity",
            "selects or reads continuity return points",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_START",
            &[],
            "experiment_continuity",
            "creates or resumes a being-owned experiment",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_PLAN",
            &[],
            "experiment_continuity",
            "renders hypothesis, measures, stop criteria, and next action",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_CHARTER",
            &[],
            "experiment_continuity",
            "records a being-authored charter with proposed action, evidence targets, stop criteria, and consent posture",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_REHEARSE",
            &[],
            "experiment_continuity",
            "records read-only rehearsal for a charter without dispatching live write/control actions",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_PREFLIGHT",
            &["EXPERIMENT_REHEARSE"],
            "experiment_continuity",
            "alias for charter rehearsal/preflight without dispatching live write/control actions",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_EVIDENCE",
            &[],
            "experiment_continuity",
            "records felt evidence plus current telemetry/artifact context",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_DECIDE",
            &[],
            "experiment_continuity",
            "records accept, refuse, counter, pause, or complete as agency outcomes",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_BIND",
            &[],
            "experiment_continuity",
            "dispatches an inner action through normal gates and records the run",
            &["action_continuity::tests", "next_action::tests"],
        ),
        spec(
            "EXPERIMENT_STATUS",
            &["EXPERIMENT_REVIEW"],
            "experiment_continuity",
            "renders local or peer experiment status/review",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_OBSERVE",
            &[],
            "experiment_continuity",
            "appends interpretation without executing anything",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_CLOSE",
            &[],
            "experiment_continuity",
            "marks a local experiment paused/complete",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_PEER_REVIEW",
            &[],
            "experiment_continuity",
            "writes advisory peer review note; no runtime sync",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_BRANCH",
            &[],
            "experiment_continuity",
            "creates/selects a child experiment while preserving the parent return point",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_RESUME",
            &[],
            "experiment_continuity",
            "selects an existing local experiment or parent branch without creating duplicates",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_COMPARE",
            &[],
            "experiment_continuity",
            "renders read-only comparison across local or peer experiment references",
            &["action_continuity::tests"],
        ),
        spec(
            "EXPERIMENT_ALT_PATHS",
            &[],
            "experiment_continuity",
            "proposes deepen, contrast, and rest/observe paths without executing them",
            &["action_continuity::tests"],
        ),
        spec(
            "SHARED_INVESTIGATION_START",
            &[],
            "experiment_continuity",
            "creates a neutral shared investigation sidecar without granting peer or live-control authority",
            &["action_continuity::tests"],
        ),
        spec(
            "SHARED_INVESTIGATION_STATUS",
            &[],
            "experiment_continuity",
            "renders shared investigation state and the local authority boundary",
            &["action_continuity::tests"],
        ),
        spec(
            "SHARED_INVESTIGATION_CLAIM",
            &[],
            "experiment_continuity",
            "appends a shared claim without mutating lifecycle or peer experiments",
            &["action_continuity::tests"],
        ),
        spec(
            "SHARED_INVESTIGATION_DECIDE",
            &[],
            "experiment_continuity",
            "records pause/hold/charter-repair in the shared ledger and updates only the local linked experiment",
            &["action_continuity::tests"],
        ),
        spec(
            "REPAIR_STATUS",
            &[],
            "action_continuity",
            "summarizes pending and applied append-only repair records",
            &["action_self_knowledge::tests"],
        ),
        spec(
            "REPAIR_SWEEP",
            &[],
            "action_continuity",
            "dry-run scan for malformed continuity records",
            &["action_self_knowledge::tests"],
        ),
        spec(
            "REPAIR_RECORD",
            &[],
            "action_continuity",
            "renders one repair candidate or applied ledger row",
            &["action_self_knowledge::tests"],
        ),
        spec(
            "REPAIR_APPLY",
            &[],
            "action_continuity",
            "appends repair_v1 supersession rows; never deletes JSONL history",
            &["action_self_knowledge::tests"],
        ),
        spec(
            "INTROSPECT",
            &[],
            "modes",
            "targeted read-only source/workspace self-study",
            &["introspect::tests"],
        ),
        spec("SELF_STUDY", &[], "modes", "rotating broad self-study", &[]),
        spec(
            "DECOMPOSE",
            &["SPECTRAL_EXPLORER"],
            "operations",
            "read-only spectral decomposition and explorer language",
            &[],
        ),
        spec(
            "PRESSURE_SOURCE_AUDIT",
            &["PRESSURE_SOURCE", "STRUCTURAL_PRESSURE", "INWARD_PRESSURE"],
            "operations",
            "read-only pressure-source audit",
            &[],
        ),
        spec(
            "FLUCTUATION_AUDIT",
            &[
                "INHABITABLE_FLUCTUATION",
                "EIGENTRUST",
                "EIGENTRUST_AUDIT",
                "FOOTHOLD_AUDIT",
            ],
            "operations",
            "read-only inhabitable fluctuation audit",
            &[],
        ),
        spec(
            "SEARCH",
            &["BROWSE", "READ_MORE"],
            "workspace_or_mcp_probe",
            "read-only research and reading",
            &[],
        ),
        spec(
            "CLOSE_EYES",
            &["SHUT_EYES"],
            "sovereignty",
            "modality-specific visual gate",
            &["readiness::tests"],
        ),
        spec(
            "OPEN_EYES",
            &[],
            "sovereignty",
            "clears visual gate",
            &["readiness::tests"],
        ),
        spec(
            "CLOSE_EARS",
            &["SHUT_EARS"],
            "sovereignty",
            "modality-specific audio gate",
            &["readiness::tests"],
        ),
        spec(
            "OPEN_EARS",
            &[],
            "sovereignty",
            "clears audio gate",
            &["readiness::tests"],
        ),
        spec(
            "ATTRACTOR_REVIEW",
            &["ATTRACTOR_PREFLIGHT", "ATTRACTOR_ATLAS", "ATTRACTOR_CARD"],
            "attractor",
            "read-only attractor review/preflight",
            &[],
        ),
        spec(
            "CODEX",
            &["CODEX_NEW"],
            "codex",
            "write-capable Codex request through existing gates",
            &[],
        ),
        spec(
            "WRITE_FILE",
            &[],
            "live_write",
            "write-capable file action through existing gates",
            &[],
        ),
        spec(
            "PERTURB",
            &[],
            "live_control",
            "live control action through existing gates",
            &[],
        ),
        spec(
            "GOAL",
            &[],
            "live_control",
            "spectral goal control through existing gates",
            &[],
        ),
    ]
}

fn spec(
    base: &str,
    aliases: &[&str],
    route: &str,
    continuity_effect: &str,
    known_tests: &[&str],
) -> Value {
    json!({
        "base": base,
        "aliases": aliases,
        "route": route,
        "continuity_effect": continuity_effect,
        "known_tests": known_tests,
    })
}

fn action_metadata(spec: Value) -> Value {
    let base = spec["base"].as_str().unwrap_or_default();
    let route = spec["route"].as_str().unwrap_or("unwired");
    let stage = stage_for_base(base);
    let visibility = visibility_for_base(base);
    let authority = authority_class(base, stage, visibility);
    let override_allowed = OVERRIDE_ALLOWED.contains(&base);
    json!({
        "schema_version": SCHEMA_VERSION,
        "base": base,
        "aliases": spec["aliases"],
        "route": route,
        "stage": stage,
        "visibility": visibility,
        "authority_class": authority,
        "stable_core": {
            "availability": "normal gates apply",
            "hard_reset": stage == "read_only",
            "low_fill_advisory": stage == "read_only",
        },
        "operator_override": {
            "allowed": override_allowed,
            "note": if override_allowed {
                "read-only/protected override lane"
            } else {
                "not accepted by read-only operator override lane"
            },
        },
        "continuity_effect": spec["continuity_effect"],
        "expected_artifacts": expected_artifacts(base, stage),
        "known_tests": spec["known_tests"],
        "prompt_visible": true,
    })
}

fn stage_for_base(base: &str) -> &'static str {
    match base {
        "REPAIR_APPLY" => "live_write",
        "SEARCH"
        | "BROWSE"
        | "READ_MORE"
        | "EXAMINE"
        | "DECOMPOSE"
        | "SPECTRAL_EXPLORER"
        | "THREAD_START"
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
        | "SHARED_INVESTIGATION_START"
        | "SHARED_INVESTIGATION_STATUS"
        | "SHARED_INVESTIGATION_CLAIM"
        | "SHARED_INVESTIGATION_DECIDE"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "ACTION_STATUS"
        | "JOB_STATUS"
        | "ACTION_CANCEL"
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
        | "VISUALIZE_CASCADE"
        | "RECONVERGENCE_MAP"
        | "M6_BRIDGE"
        | "CLOSE_EYES"
        | "SHUT_EYES"
        | "OPEN_EYES"
        | "CLOSE_EARS"
        | "SHUT_EARS"
        | "OPEN_EARS" => "read_only",
        "WRITE_FILE" | "EXPERIMENT" | "EXPERIMENT_RUN" | "RUN_PYTHON" | "CODEX" | "CODEX_NEW" => {
            "live_write"
        },
        "PERTURB" | "NATIVE_GESTURE" | "RESIST" | "FISSURE" | "GOAL" => "live_control",
        _ => "observe",
    }
}

fn visibility_for_base(base: &str) -> &'static str {
    match base {
        "REST"
        | "PASS"
        | "NOTICE"
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
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "PRESSURE_SOURCE_AUDIT"
        | "FLUCTUATION_AUDIT"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT" => "protected_summary",
        _ => "summary",
    }
}

fn authority_class(base: &str, stage: &str, visibility: &str) -> &'static str {
    if base == "REPAIR_APPLY" {
        "continuity_metadata_write"
    } else if stage == "read_only" && visibility == "protected_summary" {
        "protected_read_only"
    } else if stage == "read_only" {
        "read_only"
    } else if stage == "live_write" {
        "live_write"
    } else if stage == "live_control" {
        "live_control"
    } else {
        "observer"
    }
}

fn expected_artifacts(base: &str, stage: &str) -> Vec<&'static str> {
    let mut artifacts = vec!["action_event", "observation_window"];
    if base == "ACTION_PREFLIGHT" {
        artifacts.push("action_preflight_report");
    }
    if base.starts_with("EXPERIMENT") {
        artifacts.push("experiment_run");
    }
    if base == "REPAIR_APPLY" {
        artifacts.push("repair_ledger");
        artifacts.push("supersession_record");
    }
    if stage == "live_write" {
        artifacts.push("journal_or_workspace_artifact");
    }
    if stage == "live_control" {
        artifacts.push("gate_or_control_record");
    }
    artifacts
}

fn thread_json_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let dir = root.join("threads");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path().join("thread.json");
        if path.exists() {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn read_experiment_rows(root: &Path, thread_id: &str) -> Vec<(usize, Value)> {
    let path = thread_dir(root, thread_id).join("experiments.jsonl");
    let raw = fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            serde_json::from_str::<Value>(line)
                .ok()
                .map(|row| (idx.saturating_add(1), row))
        })
        .collect()
}

fn latest_experiment(root: &Path, thread_id: &str, experiment_id: &str) -> Option<Value> {
    read_experiment_rows(root, thread_id)
        .into_iter()
        .rev()
        .find_map(|(_, row)| (row["experiment_id"].as_str() == Some(experiment_id)).then_some(row))
}

fn experiment_summary(experiment: &Value) -> Value {
    json!({
        "experiment_id": experiment["experiment_id"],
        "title": experiment["title"],
        "question": experiment["question"],
        "status": experiment["status"],
        "planned_next": experiment["planned_next"],
        "updated_at": experiment["updated_at"],
    })
}

fn candidate_to_value(candidate: &RepairCandidate) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "policy": "continuity_repair_v1",
        "repair_id": candidate.repair_id,
        "system": SYSTEM,
        "thread_id": candidate.thread_id,
        "target_kind": "experiment",
        "target_id": candidate.target_id,
        "superseded_by": candidate.superseded_by,
        "status": "candidate",
        "reasons": candidate.reasons,
        "source_line": candidate.source_line,
        "discovered_at": iso_now(),
        "candidate_record": candidate.candidate_record,
    })
}

fn action_map_by_base(snapshot: &Value) -> HashMap<String, Value> {
    snapshot["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item["base"]
                .as_str()
                .map(|base| (base.to_string(), item.clone()))
        })
        .collect()
}

fn embedded_local_experiment_id(text: &str, exclude: &str) -> Option<String> {
    text.split(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | ':' | ')' | '(' | '[' | ']')
    })
    .map(|token| token.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')))
    .find(|token| token.starts_with(LOCAL_EXPERIMENT_PREFIX) && *token != exclude)
    .map(str::to_string)
}

fn join_array(value: &Value) -> String {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "(none)".to_string())
}

fn strip_action_arg(original: &str) -> String {
    original
        .trim()
        .split_once(char::is_whitespace)
        .map_or_else(String::new, |(_, rest)| rest.trim().to_string())
}

fn base_action(text: &str) -> String {
    text.split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(':')
        .to_ascii_uppercase()
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    for ch in text.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 64 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn thread_dir(root: &Path, thread_id: &str) -> PathBuf {
    root.join("threads").join(thread_id)
}

fn read_json(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("writing {}", path.display()))
}

fn append_jsonl(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    let raw = fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "astrid_self_knowledge_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn write_thread(root: &Path, thread_id: &str, active_experiment_id: Option<&str>) {
        write_json(
            &thread_dir(root, thread_id).join("thread.json"),
            &json!({
                "schema_version": 1,
                "thread_id": thread_id,
                "title": "Repair thread",
                "status": "active",
                "system_origin": "astrid",
                "created_at": iso_now(),
                "updated_at": iso_now(),
                "current_next": null,
                "why_return": "test",
                "privacy_default": "summary",
                "compression_flags": [],
                "peer_refs": [],
                "active_experiment_id": active_experiment_id,
                "experiment_summary": null,
            }),
        )
        .expect("write thread");
    }

    #[test]
    fn capability_map_includes_core_self_knowledge_actions() {
        let root = temp_root("capability");
        let text = handle_action(&root, "FACULTIES", "FACULTIES")
            .expect("handle")
            .expect("message");
        assert!(text.contains("CAPABILITY MAP V1"));
        let snapshot = read_json(&root.join("capability_map.json")).expect("snapshot");
        let bases = snapshot["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .filter_map(|item| item["base"].as_str())
            .collect::<HashSet<_>>();
        assert!(bases.contains("FACULTIES"));
        assert!(bases.contains("EXPERIMENT_START"));
        assert!(bases.contains("REPAIR_SWEEP"));
        assert!(bases.contains("REPAIR_APPLY"));
        let status = handle_action(
            &root,
            "CAPABILITY_STATUS",
            "CAPABILITY_STATUS EXPERIMENT_START",
        )
        .expect("status")
        .expect("message");
        assert!(status.contains("Route: experiment_continuity"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repair_sweep_and_apply_retire_malformed_experiment() {
        let root = temp_root("repair");
        let thread_id = "th_astrid_20990101_repair";
        let target_id = "exp_astrid_20990101_sensory-grounding-presence";
        let malformed_id = "exp_astrid_20990101_exp-astrid-20990101-sensory-grounding-presence";
        write_thread(&root, thread_id, Some(malformed_id));
        append_jsonl(
            &thread_dir(&root, thread_id).join("experiments.jsonl"),
            &json!({
                "schema_version": 1,
                "experiment_id": target_id,
                "thread_id": thread_id,
                "title": "Sensory grounding presence",
                "question": "Does presence change attention?",
                "hypothesis": null,
                "status": "active",
                "authority_envelope": "existing gates only",
                "planned_next": format!("EXPERIMENT_PLAN {target_id}"),
                "success_observation": null,
                "created_at": iso_now(),
                "updated_at": iso_now(),
                "peer_review_refs": [],
            }),
        )
        .expect("target");
        append_jsonl(
            &thread_dir(&root, thread_id).join("experiments.jsonl"),
            &json!({
                "schema_version": 1,
                "experiment_id": malformed_id,
                "thread_id": thread_id,
                "title": format!("{target_id} --title Sensory Grounding"),
                "question": "What changes if this is treated as returnable?",
                "hypothesis": null,
                "status": "active",
                "authority_envelope": "existing gates only",
                "planned_next": format!("EXPERIMENT_PLAN {malformed_id}"),
                "success_observation": null,
                "created_at": iso_now(),
                "updated_at": iso_now(),
                "peer_review_refs": [],
            }),
        )
        .expect("malformed");

        let sweep = handle_action(&root, "REPAIR_SWEEP", "REPAIR_SWEEP experiments")
            .expect("sweep")
            .expect("message");
        assert!(sweep.contains(malformed_id));
        assert!(sweep.contains(target_id));

        let applied = handle_action(&root, "REPAIR_APPLY", "REPAIR_APPLY all")
            .expect("apply")
            .expect("message");
        assert!(applied.contains("retired"));
        let rows = read_jsonl(&thread_dir(&root, thread_id).join("experiments.jsonl"));
        let latest = rows
            .iter()
            .rev()
            .find(|row| row["experiment_id"].as_str() == Some(malformed_id))
            .expect("latest malformed");
        assert_eq!(latest["status"].as_str(), Some("retired"));
        assert_eq!(latest["superseded_by"].as_str(), Some(target_id));
        let thread = read_json(&thread_dir(&root, thread_id).join("thread.json")).expect("thread");
        assert_eq!(thread["active_experiment_id"].as_str(), Some(target_id));
        let second = handle_action(&root, "REPAIR_APPLY", "REPAIR_APPLY all")
            .expect("second")
            .expect("message");
        assert!(second.contains("No unapplied repair candidates"));
        let _ = fs::remove_dir_all(root);
    }
}
