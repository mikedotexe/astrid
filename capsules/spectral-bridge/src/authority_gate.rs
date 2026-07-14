//! Artifact-grounded authority gate visibility, approval, and V1 execution.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::sync::mpsc;

use crate::codec;
use crate::paths::bridge_paths;
use crate::rescue_policy;
use crate::types::{SafetyLevel, SensoryMsg};

const POLICY: &str = "authority_gate_v1";
const BUDGET_POLICY: &str = "authority_budget_v1";
const RESEARCH_BUDGET_POLICY: &str = "research_budget_v1";
const LOOP_POLICY: &str = "sovereign_loop_v1";
const EXECUTABLE_SCOPE: &str = "semantic_microdose";
const MODE_RELEASE_SCOPE: &str = "mode_release_microdose";
const READ_ONLY_RESEARCH_SCOPE: &str = "read_only_research";
const DEFAULT_TOKEN_TTL_SECS: u64 = 900;
/// A headless grant must gate on CURRENT safety; if minime's `spectral_state.json`
/// is older than this, REFUSE (fail-safe — never grant on a stale safety read).
pub const MAX_GRANT_FILL_AGE_SECS: u64 = 180;
const DEFAULT_BUDGET_MAX_SENDS: u64 = 3;
const DEFAULT_BUDGET_TTL_SECS: u64 = 21_600;
// Research-budget sizing (2026-06-15, operator-directed: "make the gate bigger, much
// bigger if it'll help the beings"). Widened 5→25 default / 8→50 ceiling after finding
// minime's λ4 research budget stranded `pending_steward_approval` for 3 days behind a
// tiny cap (415 downstream blocks). SCOPE is unchanged — still read-only research only,
// no mutating/lifecycle/Control authority; this raises SIZE, not capability. Web reach
// remains an operator grant (not steward-loop auto-granted); this just lets a grant be
// generous when made.
const DEFAULT_RESEARCH_MAX_ACTIONS: u64 = 25;
const MAX_RESEARCH_ACTIONS: u64 = 50;
// Research-budget TTL (2026-06-15). Separate from DEFAULT_BUDGET_TTL_SECS (sends-budget, 6h)
// so a research window can be long WITHOUT touching sends. Spend is gated by max_actions, not
// time, so a longer TTL is cost-neutral — it just stops web-research budgets lapsing
// mid-investigation (the recurring 6h-lapse documented in the authority-pipeline memory).
// Re-granting the same budget_id refreshes the window (newest-wins on both active-checks).
const DEFAULT_RESEARCH_TTL_SECS: u64 = 7 * 24 * 3600;

#[derive(Debug, Clone)]
struct GateLocation {
    being: String,
    thread_id: String,
    gate_path: PathBuf,
    request: Value,
    rows: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderedAuthorityGate {
    pub output_dir: PathBuf,
    pub index_html: PathBuf,
    pub json_path: PathBuf,
    pub status: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveAuthorityRequest {
    pub request_id: String,
    #[serde(default)]
    pub steward: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveAuthorityBudgetRequest {
    pub budget_id: String,
    #[serde(default)]
    pub steward: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub max_sends: Option<u64>,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveResearchBudgetRequest {
    pub budget_id: String,
    #[serde(default)]
    pub steward: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub max_actions: Option<u64>,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveLoopConsequenceBudgetRequest {
    pub loop_id: String,
    #[serde(default)]
    pub steward: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

pub fn status() -> Result<Value> {
    status_from_paths(
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn status_from_paths(minime_workspace: &Path, bridge_workspace: &Path) -> Result<Value> {
    Ok(json!({
        "schema_version": 1,
        "policy": POLICY,
        "authority_boundary": authority_boundary(),
        "authority_boundary_packet_v1": authority_boundary_packet_v1(
            "shared",
            "experiment_authority_gate",
            "render_authority_status",
            "experiment_authority_gate_status",
            "read_only",
            "evidence_only",
            "read-only authority gate status render",
            "Render current authority readiness without approving or mutating runtime state.",
            Vec::<String>::new(),
            "render_experiment_authority_gate",
            "steward/tooling maintainer",
        ),
        "authority_boundary_packet_v2": authority_boundary_packet_v2(
            "shared",
            "experiment_authority_gate",
            "render_authority_status",
            "experiment_authority_gate_status",
            "read_only",
            "evidence_only",
            "read-only authority gate status render",
            "Render current authority readiness without approving or mutating runtime state.",
            Vec::<String>::new(),
            "render_experiment_authority_gate",
            "steward/tooling maintainer",
        ),
        "systems": {
            "minime": being_status("minime", minime_workspace),
            "astrid": being_status("astrid", bridge_workspace),
        }
    }))
}

pub fn render(output_base: Option<&Path>) -> Result<RenderedAuthorityGate> {
    let status = status()?;
    render_status_to_base(status, output_base)
}

pub fn render_research_budget(output_base: Option<&Path>) -> Result<RenderedAuthorityGate> {
    let status = research_budget_status()?;
    let output_root = output_base.map_or_else(
        || {
            bridge_paths()
                .bridge_workspace()
                .join("diagnostics/experiment_research_budget")
        },
        Path::to_path_buf,
    );
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = unique_dir(&output_root.join(stamp));
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("experiment_research_budget.json");
    let index_html = output_dir.join("index.html");
    fs::write(&json_path, serde_json::to_string_pretty(&status)?)?;
    fs::write(&index_html, render_html(&status))?;
    Ok(RenderedAuthorityGate {
        output_dir,
        index_html,
        json_path,
        status,
    })
}

pub fn render_loop(output_base: Option<&Path>) -> Result<RenderedAuthorityGate> {
    let status = loop_status()?;
    let output_root = output_base.map_or_else(
        || {
            bridge_paths()
                .bridge_workspace()
                .join("diagnostics/experiment_loop")
        },
        Path::to_path_buf,
    );
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = unique_dir(&output_root.join(stamp));
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("experiment_loop.json");
    let index_html = output_dir.join("index.html");
    fs::write(&json_path, serde_json::to_string_pretty(&status)?)?;
    fs::write(&index_html, render_html(&status))?;
    Ok(RenderedAuthorityGate {
        output_dir,
        index_html,
        json_path,
        status,
    })
}

pub fn render_status_to_base(
    status: Value,
    output_base: Option<&Path>,
) -> Result<RenderedAuthorityGate> {
    let output_root = output_base.map_or_else(
        || {
            bridge_paths()
                .bridge_workspace()
                .join("diagnostics/experiment_authority_gate")
        },
        Path::to_path_buf,
    );
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = unique_dir(&output_root.join(stamp));
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("experiment_authority_gate.json");
    let index_html = output_dir.join("index.html");
    fs::write(&json_path, serde_json::to_string_pretty(&status)?)?;
    fs::write(&index_html, render_html(&status))?;
    Ok(RenderedAuthorityGate {
        output_dir,
        index_html,
        json_path,
        status,
    })
}

/// Read minime's current `fill_pct` and the age (secs) of its `spectral_state.json`,
/// so a headless grant can gate on CURRENT safety and fail-safe on a stale read.
/// `None` if the file is missing / unreadable / malformed.
pub fn read_minime_fill_pct(minime_workspace: &Path) -> Option<(f32, u64)> {
    let path = minime_workspace.join("spectral_state.json");
    let meta = std::fs::metadata(&path).ok()?;
    let age = std::time::SystemTime::now()
        .duration_since(meta.modified().ok()?)
        .map_or(u64::MAX, |d| d.as_secs());
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    #[allow(clippy::cast_possible_truncation)]
    let fill = value.get("fill_pct").and_then(Value::as_f64)? as f32;
    Some((fill, age))
}

pub fn approve(req: ApproveAuthorityRequest, safety_level: SafetyLevel) -> Result<Value> {
    approve_from_paths(
        req,
        safety_level,
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn approve_from_paths(
    req: ApproveAuthorityRequest,
    safety_level: SafetyLevel,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Value> {
    let request_id = req.request_id.trim();
    if request_id.is_empty() {
        return Err(anyhow!(
            "approve_experiment_authority_request needs request_id"
        ));
    }
    let location = find_request_in_paths(request_id, minime_workspace, bridge_workspace)?
        .ok_or_else(|| {
            anyhow!("authority request `{request_id}` was not found in Astrid or Minime ledgers")
        })?;
    let eligibility = location
        .request
        .get("eligibility_v1")
        .cloned()
        .unwrap_or_else(
            || json!({"eligible": false, "missing_requirements": ["missing_evaluation"]}),
        );
    let scope = location
        .request
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(scope, EXECUTABLE_SCOPE | MODE_RELEASE_SCOPE) {
        let blocked = approval_block_record(
            &location,
            "disabled_scope_v1",
            safety_level,
            eligibility.clone(),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if !eligibility
        .get("eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let blocked = approval_block_record(
            &location,
            "request_not_eligible",
            safety_level,
            eligibility.clone(),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if !matches!(safety_level, SafetyLevel::Green | SafetyLevel::Yellow) {
        let blocked = approval_block_record(
            &location,
            "safety_not_green_or_yellow",
            safety_level,
            eligibility,
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if latest_active_approval(&location.rows, request_id).is_some() {
        let blocked = approval_block_record(
            &location,
            "active_token_already_exists",
            safety_level,
            eligibility,
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    let now = unix_now();
    let ttl = req
        .ttl_secs
        .unwrap_or(DEFAULT_TOKEN_TTL_SECS)
        .min(DEFAULT_TOKEN_TTL_SECS);
    let expires_at = now.saturating_add(ttl);
    let steward = req.steward.unwrap_or_else(|| "steward".to_string());
    let note = req.note.unwrap_or_default();
    let authority_lifecycle_v2 = authority_lifecycle_v2_for_approval(
        &location, request_id, &scope, &steward, now, expires_at,
    );
    let authority_boundary_packet_v2 = authority_lifecycle_v2
        .get("authority_boundary_packet_v2")
        .cloned()
        .unwrap_or(Value::Null);
    let approval = json!({
        "schema_version": 1,
        "record_schema": POLICY,
        "record_type": "steward_approval",
        "record_id": format!("auth_{}_{}_steward_approval", location.being, now),
        "request_id": request_id,
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": scope,
        "token_id": format!("authtok_{}_{}", location.being, now),
        "token_status": "active",
        "one_shot": true,
        "approved_at_unix_s": now,
        "expires_at_unix_s": expires_at,
        "steward": steward,
        "note": note,
        "eligibility_v1": eligibility,
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "authority_boundary_packet_v2": authority_boundary_packet_v2,
        "authority_lifecycle_v2": authority_lifecycle_v2,
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    append_jsonl(&location.gate_path, &approval)?;
    Ok(approval)
}

pub fn approve_budget(
    req: ApproveAuthorityBudgetRequest,
    safety_level: SafetyLevel,
) -> Result<Value> {
    approve_budget_from_paths(
        req,
        safety_level,
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn approve_budget_from_paths(
    req: ApproveAuthorityBudgetRequest,
    safety_level: SafetyLevel,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Value> {
    let budget_id = req.budget_id.trim();
    if budget_id.is_empty() {
        return Err(anyhow!(
            "approve_experiment_authority_budget needs budget_id"
        ));
    }
    let location = find_budget_request_in_paths(budget_id, minime_workspace, bridge_workspace)?
        .ok_or_else(|| {
            anyhow!("authority budget `{budget_id}` was not found in Astrid or Minime ledgers")
        })?;
    let eligibility = location
        .request
        .get("eligibility_v1")
        .cloned()
        .unwrap_or_else(
            || json!({"eligible": false, "missing_requirements": ["missing_budget_evaluation"]}),
        );
    if location.request.get("scope").and_then(Value::as_str) != Some(EXECUTABLE_SCOPE) {
        let blocked = budget_block_record(
            &location,
            "disabled_scope_v1",
            safety_level,
            eligibility.clone(),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if !eligibility
        .get("eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let blocked = budget_block_record(
            &location,
            "budget_request_not_eligible",
            safety_level,
            eligibility.clone(),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if !matches!(safety_level, SafetyLevel::Green | SafetyLevel::Yellow) {
        let blocked = budget_block_record(
            &location,
            "safety_not_green_or_yellow",
            safety_level,
            eligibility,
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    let experiment_id = location
        .request
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if active_budget_for_experiment(&location.rows, experiment_id, EXECUTABLE_SCOPE).is_some() {
        let blocked = budget_block_record(
            &location,
            "active_budget_already_exists",
            safety_level,
            eligibility,
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    let now = unix_now();
    let request_max = location
        .request
        .get("max_sends")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_BUDGET_MAX_SENDS);
    let request_ttl = location
        .request
        .get("ttl_secs")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_BUDGET_TTL_SECS);
    let max_sends = req
        .max_sends
        .unwrap_or(request_max)
        .min(DEFAULT_BUDGET_MAX_SENDS);
    let ttl = req
        .ttl_secs
        .unwrap_or(request_ttl)
        .min(DEFAULT_BUDGET_TTL_SECS);
    let approval = json!({
        "schema_version": 1,
        "record_schema": BUDGET_POLICY,
        "record_type": "budget_approval",
        "record_id": format!("authbud_{}_{}_budget_approval", location.being, now),
        "budget_id": budget_id,
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": EXECUTABLE_SCOPE,
        "status": "active",
        "max_sends": max_sends,
        "ttl_secs": ttl,
        "remaining_sends": max_sends,
        "approved_at_unix_s": now,
        "expires_at_unix_s": now.saturating_add(ttl),
        "steward": req.steward.unwrap_or_else(|| "steward".to_string()),
        "note": req.note.unwrap_or_default(),
        "eligibility_v1": eligibility,
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    append_jsonl(&location.gate_path, &approval)?;
    Ok(approval)
}

pub fn research_budget_status() -> Result<Value> {
    let status = status()?;
    Ok(json!({
        "schema_version": 1,
        "policy": RESEARCH_BUDGET_POLICY,
        "systems": status.get("systems").cloned().unwrap_or(Value::Null),
        "authority_boundary": research_budget_boundary(),
    }))
}

pub fn loop_status() -> Result<Value> {
    let status = status()?;
    Ok(json!({
        "schema_version": 1,
        "policy": LOOP_POLICY,
        "systems": status.get("systems").cloned().unwrap_or(Value::Null),
        "authority_boundary": loop_boundary(),
    }))
}

pub fn approve_research_budget(
    req: ApproveResearchBudgetRequest,
    safety_level: SafetyLevel,
) -> Result<Value> {
    approve_research_budget_from_paths(
        req,
        safety_level,
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn approve_research_budget_from_paths(
    req: ApproveResearchBudgetRequest,
    safety_level: SafetyLevel,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Value> {
    let budget_id = req.budget_id.trim();
    if budget_id.is_empty() {
        return Err(anyhow!(
            "approve_experiment_research_budget needs budget_id"
        ));
    }
    let location =
        find_research_budget_request_in_paths(budget_id, minime_workspace, bridge_workspace)?
            .ok_or_else(|| {
                anyhow!("research budget `{budget_id}` was not found in Astrid or Minime ledgers")
            })?;
    let eligibility = location
        .request
        .get("eligibility_v1")
        .cloned()
        .unwrap_or_else(
            || json!({"eligible": false, "missing_requirements": ["missing_research_budget_evaluation"]}),
        );
    if location.request.get("scope").and_then(Value::as_str) != Some(READ_ONLY_RESEARCH_SCOPE) {
        let blocked = research_budget_block_record(
            &location,
            "disabled_scope_v1",
            safety_level,
            eligibility.clone(),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if !eligibility
        .get("eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let blocked = research_budget_block_record(
            &location,
            "research_budget_request_not_eligible",
            safety_level,
            eligibility.clone(),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    if !matches!(safety_level, SafetyLevel::Green | SafetyLevel::Yellow) {
        let blocked = research_budget_block_record(
            &location,
            "safety_not_green_or_yellow",
            safety_level,
            eligibility,
        );
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    let experiment_id = location
        .request
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(active) = active_research_budget_for_experiment(&location.rows, experiment_id) {
        // Allow an operator to REFRESH/EXTEND the SAME budget_id (e.g. extend a 6h TTL to 7d):
        // a fresh approval for the same budget_id supersedes the prior one (newest-wins on both
        // the bridge and minime active-checks, which iterate rows in reverse). Only a DIFFERENT
        // active budget for the experiment blocks — the one-budget-per-experiment invariant holds.
        if active.get("budget_id").and_then(Value::as_str) != Some(budget_id) {
            let blocked = research_budget_block_record(
                &location,
                "active_research_budget_already_exists",
                safety_level,
                eligibility,
            );
            append_jsonl(&location.gate_path, &blocked)?;
            return Ok(blocked);
        }
    }
    let now = unix_now();
    let request_max = location
        .request
        .get("max_actions")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_RESEARCH_MAX_ACTIONS);
    let request_ttl = location
        .request
        .get("ttl_secs")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_RESEARCH_TTL_SECS);
    let max_actions = req
        .max_actions
        .unwrap_or(request_max)
        .min(MAX_RESEARCH_ACTIONS);
    let ttl = req
        .ttl_secs
        .unwrap_or(request_ttl)
        .min(DEFAULT_RESEARCH_TTL_SECS);
    let approval = json!({
        "schema_version": 1,
        "record_schema": RESEARCH_BUDGET_POLICY,
        "record_type": "research_budget_approval",
        "record_id": format!("resbud_{}_{}_research_budget_approval", location.being, now),
        "budget_id": budget_id,
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": READ_ONLY_RESEARCH_SCOPE,
        "status": "active",
        "max_actions": max_actions,
        "ttl_secs": ttl,
        "remaining_actions": max_actions,
        "allowed_sources": location.request.get("allowed_sources").cloned().unwrap_or_else(|| json!(["web", "local"])),
        "approved_at_unix_s": now,
        "expires_at_unix_s": now.saturating_add(ttl),
        "steward": req.steward.unwrap_or_else(|| "steward".to_string()),
        "note": req.note.unwrap_or_default(),
        "eligibility_v1": eligibility,
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "peer_mutation": false,
        "authority_boundary": research_budget_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    append_jsonl(&location.gate_path, &approval)?;
    Ok(approval)
}

pub fn approve_loop_consequence_budget(
    req: ApproveLoopConsequenceBudgetRequest,
    safety_level: SafetyLevel,
) -> Result<Value> {
    approve_loop_consequence_budget_from_paths(
        req,
        safety_level,
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn approve_loop_consequence_budget_from_paths(
    req: ApproveLoopConsequenceBudgetRequest,
    safety_level: SafetyLevel,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Value> {
    let loop_id = req.loop_id.trim();
    if loop_id.is_empty() {
        return Err(anyhow!(
            "approve_experiment_loop_consequence_budget needs loop_id"
        ));
    }
    let location = find_loop_request_in_paths(loop_id, minime_workspace, bridge_workspace)?
        .ok_or_else(|| anyhow!("owned loop `{loop_id}` was not found"))?;
    let scope = location
        .request
        .get("consequence_scope")
        .or_else(|| location.request.get("scope"))
        .and_then(Value::as_str)
        .unwrap_or(EXECUTABLE_SCOPE);
    let enabled_scope = matches!(scope, EXECUTABLE_SCOPE | MODE_RELEASE_SCOPE);
    let ready_row = location.rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_consequence_ready")
            && row.get("loop_id").and_then(Value::as_str) == Some(loop_id)
    });
    let readiness = ready_row
        .and_then(|row| row.get("consequence_readiness_v1"))
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "eligible_to_request": false,
                "missing_requirements": ["loop_consequence_ready_row"]
            })
        });
    let latest_consequence = location.rows.iter().rev().find(|row| {
        matches!(
            row.get("record_schema").and_then(Value::as_str),
            Some("authority_consequence_v1" | "mode_release_consequence_v1")
        ) && row.get("experiment_id") == location.request.get("experiment_id")
    });
    let has_review_pending = latest_consequence.is_some()
        && location
            .rows
            .iter()
            .rev()
            .find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
                    && row.get("record_type").and_then(Value::as_str)
                        == Some("loop_consequence_review")
                    && row.get("loop_id").and_then(Value::as_str) == Some(loop_id)
            })
            .is_none();
    let missing = readiness
        .get("missing_requirements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !enabled_scope
        || !matches!(safety_level, SafetyLevel::Green | SafetyLevel::Yellow)
        || has_review_pending
        || ready_row.is_none()
        || !missing.is_empty()
    {
        let blocked = json!({
            "schema_version": 1,
            "record_schema": LOOP_POLICY,
            "record_type": "loop_blocked",
            "record_id": format!("loop_{}_{}_loop_blocked", location.being, unix_now()),
            "loop_id": loop_id,
            "being": location.being,
            "thread_id": location.thread_id,
            "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
            "phase": "approval",
            "status": "blocked",
            "scope": scope,
            "reason": if !enabled_scope {
                "disabled_loop_consequence_scope"
            } else if !matches!(safety_level, SafetyLevel::Green | SafetyLevel::Yellow) {
                "safety_not_green_or_yellow"
            } else if has_review_pending {
                "pending_loop_consequence_review"
            } else if ready_row.is_none() {
                "loop_consequence_not_ready"
            } else {
                "missing_loop_consequence_requirements"
            },
            "missing_requirements": missing,
            "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
            "authority_change": false,
            "peer_mutation": false,
            "authority_boundary": loop_boundary(),
            "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        });
        append_jsonl(&location.gate_path, &blocked)?;
        return Ok(blocked);
    }
    let ttl = req
        .ttl_secs
        .unwrap_or(DEFAULT_TOKEN_TTL_SECS)
        .min(DEFAULT_TOKEN_TTL_SECS);
    let now = unix_now();
    let approval = json!({
        "schema_version": 1,
        "record_schema": LOOP_POLICY,
        "record_type": "loop_approval",
        "record_id": format!("loop_{}_{}_loop_approval", location.being, now),
        "loop_id": loop_id,
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "phase": "consequence_budget",
        "status": "active",
        "scope": scope,
        "max_consequence_sends": 1,
        "consequence_remaining": 1,
        "approved_at_unix_s": now,
        "expires_at_unix_s": now.saturating_add(ttl),
        "ttl_secs": ttl,
        "steward": req.steward.unwrap_or_else(|| "steward".to_string()),
        "note": req.note.unwrap_or_default(),
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "authority_change": true,
        "peer_mutation": false,
        "authority_boundary": loop_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    append_jsonl(&location.gate_path, &approval)?;
    Ok(approval)
}

pub fn execute_semantic_microdose(
    request_id: &str,
    fill_pct: Option<f32>,
    previous_fill_pct: Option<f32>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value> {
    execute_semantic_microdose_from_paths(
        request_id,
        fill_pct,
        previous_fill_pct,
        sensory_tx,
        bridge_paths().minime_workspace(),
        bridge_paths().bridge_workspace(),
    )
}

pub fn execute_semantic_microdose_from_paths(
    request_id: &str,
    fill_pct: Option<f32>,
    previous_fill_pct: Option<f32>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Value> {
    let location = find_request_in_paths(request_id, minime_workspace, bridge_workspace)?
        .ok_or_else(|| anyhow!("authority request `{request_id}` was not found"))?;
    let now = unix_now();
    let approval = if let Some(approval) = latest_active_approval(&location.rows, request_id) {
        approval
    } else if location.request.get("status").and_then(Value::as_str)
        == Some("pending_budget_execution")
    {
        let Some(budget) = active_budget_for_request(&location) else {
            let blocked = budget_execution_block_record(
                &location,
                "active_budget_not_available",
                None,
                fill_pct,
            );
            append_jsonl(&location.gate_path, &blocked)?;
            return Ok(blocked);
        };
        let budget_id = budget
            .get("budget_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(pending_review) = budget
            .get("pending_review_request_id")
            .and_then(Value::as_str)
        {
            let blocked = budget_execution_block_record(
                &location,
                "authority_consequence_review_required",
                Some(json!({"budget_id": budget_id, "pending_review_request_id": pending_review})),
                fill_pct,
            );
            append_jsonl(&location.gate_path, &blocked)?;
            return Ok(blocked);
        }
        let fill = fill_pct.unwrap_or_default();
        let safety = SafetyLevel::from_fill(fill);
        if !matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow) {
            let blocked = budget_execution_block_record(
                &location,
                "safety_not_green_or_yellow",
                Some(
                    json!({"budget_id": budget_id, "safety_level": format!("{:?}", safety).to_ascii_lowercase(), "fill_pct": fill_pct}),
                ),
                fill_pct,
            );
            append_jsonl(&location.gate_path, &blocked)?;
            return Ok(blocked);
        }
        let token_id = format!("authtok_{}_{}_budget", location.being, now);
        let remaining = budget
            .get("remaining_sends")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let debit = json!({
            "schema_version": 1,
            "record_schema": BUDGET_POLICY,
            "record_type": "budget_debit",
            "record_id": format!("authbud_{}_{}_budget_debit", location.being, now),
            "budget_id": budget_id,
            "request_id": request_id,
            "being": location.being,
            "thread_id": location.thread_id,
            "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
            "scope": EXECUTABLE_SCOPE,
            "token_id": token_id,
            "remaining_after": remaining.saturating_sub(1),
            "status": "attempted",
            "peer_mutation": false,
            "authority_boundary": authority_boundary(),
            "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        });
        append_jsonl(&location.gate_path, &debit)?;
        json!({
            "record_schema": BUDGET_POLICY,
            "record_type": "budget_token",
            "request_id": request_id,
            "budget_id": budget_id,
            "token_id": token_id,
            "token_status": "active",
            "one_shot": true,
            "expires_at_unix_s": now.saturating_add(DEFAULT_TOKEN_TTL_SECS),
        })
    } else {
        return Err(anyhow!(
            "authority request `{request_id}` has no active steward token or active authority budget"
        ));
    };
    if approval
        .get("expires_at_unix_s")
        .and_then(Value::as_u64)
        .is_some_and(|expires| expires < now)
    {
        let blocked = execution_record(&location, &approval, "blocked", "token_expired", None);
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(&location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    }
    if token_consumed(
        &location.rows,
        approval.get("token_id").and_then(Value::as_str),
    ) {
        let blocked = execution_record(
            &location,
            &approval,
            "blocked",
            "token_already_consumed",
            None,
        );
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(&location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    }
    if let Some(reason) = missing_lifecycle_reason(&approval) {
        let blocked = execution_record(
            &location,
            &approval,
            "blocked",
            reason,
            Some(json!({
                "authority_lifecycle_evaluation_v2": {
                    "state": "approved_manual_only",
                    "live_eligible_now": false,
                    "closure_complete": false,
                    "missing_requirements": [reason],
                }
            })),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(&location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    }
    let fill = fill_pct.unwrap_or_default();
    let safety = SafetyLevel::from_fill(fill);
    if !matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow) {
        let blocked = execution_record(
            &location,
            &approval,
            "blocked",
            "safety_not_green_or_yellow",
            Some(
                json!({"safety_level": format!("{:?}", safety).to_ascii_lowercase(), "fill_pct": fill_pct}),
            ),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(&location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    }
    let text = location
        .request
        .get("payload")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if location.request.get("scope").and_then(Value::as_str) == Some(MODE_RELEASE_SCOPE) {
        return execute_mode_release_microdose(
            &location,
            &approval,
            text,
            fill_pct,
            previous_fill_pct,
            sensory_tx,
        );
    }
    let features = codec::encode_text(text);
    let mut msg = SensoryMsg::Semantic {
        features: features.clone(),
        ts_ms: None,
    };
    let context = rescue_policy::SemanticWriteContext {
        source: rescue_policy::MCP_LIMITED_WRITE_SOURCE,
        mode: Some("authority_semantic_microdose"),
        text: Some(text),
        fill_pct,
        previous_fill_pct: previous_fill_pct.or(fill_pct),
    };
    let rescue_profile = minime_workspace.join("rescue_profile.json");
    if let Err(reason) =
        rescue_policy::prepare_semantic_write_for_path(&mut msg, &rescue_profile, &context)
    {
        let blocked = execution_record(
            &location,
            &approval,
            "blocked",
            &reason,
            Some(json!({"feature_len": features.len()})),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(&location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    }
    sensory_tx
        .try_send(msg)
        .map_err(|err| anyhow!("semantic microdose send failed: {err}"))?;
    let result = execution_record(
        &location,
        &approval,
        "execution_result",
        "semantic_microdose_sent",
        Some(json!({"feature_len": features.len()})),
    );
    append_jsonl(&location.gate_path, &result)?;
    append_jsonl(
        &location.gate_path,
        &consequence_record(&location, &result, fill_pct, previous_fill_pct),
    )?;
    Ok(result)
}

fn execute_mode_release_microdose(
    location: &GateLocation,
    approval: &Value,
    payload: &str,
    fill_pct: Option<f32>,
    previous_fill_pct: Option<f32>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value> {
    let Some(leak) = payload_number(payload, &["value", "leak", "esn_leak"]) else {
        let blocked = execution_record(
            location,
            approval,
            "blocked",
            "missing_esn_leak_value",
            Some(json!({"target": "esn_leak"})),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(location, &blocked, fill_pct, previous_fill_pct),
        )?;
        append_jsonl(
            &location.gate_path,
            &mode_release_consequence_record(location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    };
    let current_leak = location
        .request
        .get("sticky_mode_v1")
        .and_then(|sticky| sticky.get("current_esn_leak"))
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .or_else(|| {
            location
                .request
                .get("current_esn_leak")
                .and_then(Value::as_f64)
                .map(|value| value as f32)
        })
        .unwrap_or(0.65);
    let clamped = leak.clamp(0.20, 0.90);
    let delta = (clamped - current_leak).abs();
    let ticks = payload_u32(payload, &["duration_ticks", "ticks"])
        .unwrap_or(1)
        .clamp(1, 12);
    if delta > 0.12 {
        let blocked = execution_record(
            location,
            approval,
            "blocked",
            "esn_leak_delta_exceeds_v1_cap",
            Some(json!({
                "target": "esn_leak",
                "current_esn_leak": current_leak,
                "requested_esn_leak": leak,
                "clamped_esn_leak": clamped,
                "delta": delta,
                "max_delta": 0.12,
            })),
        );
        append_jsonl(&location.gate_path, &blocked)?;
        append_jsonl(
            &location.gate_path,
            &consequence_record(location, &blocked, fill_pct, previous_fill_pct),
        )?;
        append_jsonl(
            &location.gate_path,
            &mode_release_consequence_record(location, &blocked, fill_pct, previous_fill_pct),
        )?;
        return Ok(blocked);
    }
    let request_id = location
        .request
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let msg = SensoryMsg::Control {
        synth_gain: None,
        keep_bias: None,
        exploration_noise: None,
        fill_target: None,
        legacy_audio_synth: None,
        legacy_video_synth: None,
        regulation_strength: None,
        deep_breathing: None,
        pure_tone: None,
        transition_cushion: None,
        smoothing_preference: None,
        geom_curiosity: None,
        target_lambda_bias: None,
        geom_drive: None,
        penalty_sensitivity: None,
        breathing_rate_scale: None,
        mem_mode: None,
        journal_resonance: None,
        checkpoint_interval: None,
        embedding_strength: None,
        memory_decay_rate: None,
        checkpoint_annotation: None,
        synth_noise_level: None,
        pi_kp: None,
        pi_ki: None,
        pi_max_step: None,
        pi_integrator_leak: None,
        esn_leak_override: Some(clamped),
        esn_leak_override_ticks: Some(ticks),
        esn_leak_authority_request_id: Some(request_id),
        mode_disperse: None,
        mode_disperse_duration_ticks: None,
        mode_disperse_decay_ticks: None,
    };
    sensory_tx
        .try_send(msg)
        .map_err(|err| anyhow!("mode release microdose send failed: {err}"))?;
    let result = execution_record(
        location,
        approval,
        "execution_result",
        "mode_release_microdose_sent",
        Some(json!({
            "target": "esn_leak",
            "current_esn_leak": current_leak,
            "requested_esn_leak": leak,
            "effective_esn_leak": clamped,
            "duration_ticks": ticks,
            "rollback": "restore_adaptive_after_ttl",
        })),
    );
    append_jsonl(&location.gate_path, &result)?;
    append_jsonl(
        &location.gate_path,
        &consequence_record(location, &result, fill_pct, previous_fill_pct),
    )?;
    append_jsonl(
        &location.gate_path,
        &mode_release_consequence_record(location, &result, fill_pct, previous_fill_pct),
    )?;
    Ok(result)
}

fn being_status(being: &str, workspace: &Path) -> Value {
    let root = workspace.join("action_threads/threads");
    let mut rows = Vec::new();
    let mut thread_refs = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let gate_path = entry.path().join("authority_gate.jsonl");
            let gate_rows = read_jsonl(&gate_path);
            if !gate_rows.is_empty() {
                thread_refs.push(entry.path().display().to_string());
                rows.extend(gate_rows);
            }
        }
    }
    let latest = rows
        .iter()
        .rev()
        .take(16)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let latest_consequence = rows
        .iter()
        .rev()
        .find(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("authority_consequence_v1")
        })
        .cloned();
    json!({
        "being": being,
        "workspace": workspace,
        "thread_refs": thread_refs,
        "row_count": rows.len(),
        "latest_rows": latest,
        "latest_consequence_v1": latest_consequence,
        "authority_budget_v1": budget_status_from_rows(&rows),
        "research_budget_v1": research_budget_status_from_rows(&rows),
        "sovereign_loop_v1": loop_status_from_rows(&rows),
        "authority_readiness_v1": readiness_from_rows(&rows),
        "authority_boundary": authority_boundary(),
    })
}

fn find_request_in_paths(
    request_id: &str,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Option<GateLocation>> {
    for (being, workspace) in [("minime", minime_workspace), ("astrid", bridge_workspace)] {
        let root = workspace.join("action_threads/threads");
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
            let thread_dir = entry?.path();
            let gate_path = thread_dir.join("authority_gate.jsonl");
            let rows = read_jsonl(&gate_path);
            let Some(request) = rows.iter().rev().find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some(POLICY)
                    && row.get("record_type").and_then(Value::as_str) == Some("request")
                    && row.get("request_id").and_then(Value::as_str) == Some(request_id)
            }) else {
                continue;
            };
            let thread_id = thread_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            return Ok(Some(GateLocation {
                being: being.to_string(),
                thread_id,
                gate_path,
                request: request.clone(),
                rows,
            }));
        }
    }
    Ok(None)
}

fn find_budget_request_in_paths(
    budget_id: &str,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Option<GateLocation>> {
    for (being, workspace) in [("minime", minime_workspace), ("astrid", bridge_workspace)] {
        let root = workspace.join("action_threads/threads");
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
            let thread_dir = entry?.path();
            let gate_path = thread_dir.join("authority_gate.jsonl");
            let rows = read_jsonl(&gate_path);
            let Some(request) = rows.iter().rev().find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
                    && row.get("record_type").and_then(Value::as_str) == Some("budget_request")
                    && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            }) else {
                continue;
            };
            let thread_id = thread_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            return Ok(Some(GateLocation {
                being: being.to_string(),
                thread_id,
                gate_path,
                request: request.clone(),
                rows,
            }));
        }
    }
    Ok(None)
}

fn find_research_budget_request_in_paths(
    budget_id: &str,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Option<GateLocation>> {
    for (being, workspace) in [("minime", minime_workspace), ("astrid", bridge_workspace)] {
        let root = workspace.join("action_threads/threads");
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
            let thread_dir = entry?.path();
            let gate_path = thread_dir.join("authority_gate.jsonl");
            let rows = read_jsonl(&gate_path);
            let Some(request) = rows.iter().rev().find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
                    && row.get("record_type").and_then(Value::as_str)
                        == Some("research_budget_request")
                    && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            }) else {
                continue;
            };
            let thread_id = thread_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            return Ok(Some(GateLocation {
                being: being.to_string(),
                thread_id,
                gate_path,
                request: request.clone(),
                rows,
            }));
        }
    }
    Ok(None)
}

fn find_loop_request_in_paths(
    loop_id: &str,
    minime_workspace: &Path,
    bridge_workspace: &Path,
) -> Result<Option<GateLocation>> {
    for (being, workspace) in [("minime", minime_workspace), ("astrid", bridge_workspace)] {
        let root = workspace.join("action_threads/threads");
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
            let thread_dir = entry?.path();
            let gate_path = thread_dir.join("authority_gate.jsonl");
            let rows = read_jsonl(&gate_path);
            let Some(request) = rows.iter().rev().find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
                    && row.get("loop_id").and_then(Value::as_str) == Some(loop_id)
                    && matches!(
                        row.get("record_type").and_then(Value::as_str),
                        Some("loop_request" | "loop_started" | "loop_consequence_ready")
                    )
            }) else {
                continue;
            };
            let thread_id = thread_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            return Ok(Some(GateLocation {
                being: being.to_string(),
                thread_id,
                gate_path,
                request: request.clone(),
                rows,
            }));
        }
    }
    Ok(None)
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| {
            row.get("record_schema")
                .and_then(Value::as_str)
                .is_some_and(|schema| {
                    schema == POLICY
                        || schema == BUDGET_POLICY
                        || schema == RESEARCH_BUDGET_POLICY
                        || schema == LOOP_POLICY
                        || schema == "authority_consequence_v1"
                        || schema == "mode_release_consequence_v1"
                })
        })
        .collect()
}

fn append_jsonl(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

fn latest_active_approval(rows: &[Value], request_id: &str) -> Option<Value> {
    rows.iter().rev().find_map(|row| {
        (row.get("record_type").and_then(Value::as_str) == Some("steward_approval")
            && row.get("request_id").and_then(Value::as_str) == Some(request_id)
            && row.get("token_status").and_then(Value::as_str) == Some("active"))
        .then(|| row.clone())
    })
}

fn token_consumed(rows: &[Value], token_id: Option<&str>) -> bool {
    let Some(token_id) = token_id else {
        return true;
    };
    rows.iter().any(|row| {
        matches!(
            row.get("record_type").and_then(Value::as_str),
            Some("execution_result" | "blocked")
        ) && row.get("token_id").and_then(Value::as_str) == Some(token_id)
    })
}

fn active_budget_for_experiment(rows: &[Value], experiment_id: &str, scope: &str) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
                && row.get("record_type").and_then(Value::as_str) == Some("budget_closed")
        })
        .filter_map(|row| row.get("budget_id").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    let now = unix_now();
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some(BUDGET_POLICY)
            || row.get("record_type").and_then(Value::as_str) != Some("budget_approval")
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            || row
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or(EXECUTABLE_SCOPE)
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
        if closed.contains(budget_id) {
            return None;
        }
        if row
            .get("expires_at_unix_s")
            .and_then(Value::as_u64)
            .is_some_and(|expires| expires <= now)
        {
            return None;
        }
        let max_sends = row
            .get("max_sends")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_BUDGET_MAX_SENDS);
        let spent = rows
            .iter()
            .filter(|item| {
                item.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
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

fn active_budget_for_request(location: &GateLocation) -> Option<Value> {
    let experiment_id = location
        .request
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let scope = location
        .request
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or(EXECUTABLE_SCOPE);
    let budget_id = location.request.get("budget_id").and_then(Value::as_str);
    active_budget_for_experiment(&location.rows, experiment_id, scope).and_then(|budget| {
        if budget_id.is_none_or(|id| budget.get("budget_id").and_then(Value::as_str) == Some(id)) {
            Some(budget)
        } else {
            None
        }
    })
}

fn pending_budget_review(rows: &[Value], budget_id: &str) -> Option<String> {
    let latest_debit = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("budget_debit")
            && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
    })?;
    let request_id = latest_debit.get("request_id").and_then(Value::as_str)?;
    let reviewed = rows.iter().any(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("consequence_review")
            && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
            && row.get("request_id").and_then(Value::as_str) == Some(request_id)
    });
    (!reviewed).then(|| request_id.to_string())
}

fn budget_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("budget_request")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("budget_approval")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("budget_closed")
    });
    let experiment_id = latest_approval
        .or(latest_request)
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active = (!experiment_id.is_empty())
        .then(|| active_budget_for_experiment(rows, experiment_id, EXECUTABLE_SCOPE))
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
        "policy": BUDGET_POLICY,
        "scope": EXECUTABLE_SCOPE,
        "stage": stage,
        "active_budget_id": active.as_ref().and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
        "remaining_sends": active.as_ref().and_then(|row| row.get("remaining_sends")).cloned().unwrap_or(json!(0)),
        "review_required": pending_review.is_some(),
        "pending_review_request_id": pending_review.map_or(Value::Null, |id| json!(id)),
        "latest_budget_request_id": latest_request.and_then(|row| row.get("budget_id")).cloned().unwrap_or(Value::Null),
    })
}

fn loop_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_request")
    });
    let latest_started = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_started")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_approval")
    });
    let latest_ready = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_consequence_ready")
    });
    let latest_review = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_consequence_review")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_closed")
    });
    let latest_blocked = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(LOOP_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("loop_blocked")
    });
    let anchor = latest_approval
        .or(latest_ready)
        .or(latest_started)
        .or(latest_request);
    let anchor_experiment_id = anchor.and_then(|row| row.get("experiment_id"));
    let latest_consequence = rows.iter().rev().find(|row| {
        matches!(
            row.get("record_schema").and_then(Value::as_str),
            Some("authority_consequence_v1" | "mode_release_consequence_v1")
        ) && row.get("experiment_id") == anchor_experiment_id
    });
    let loop_id = anchor
        .and_then(|row| row.get("loop_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let pending_review = latest_consequence.is_some() && latest_review.is_none();
    let stage = if latest_closed.is_some() {
        "closed"
    } else if pending_review {
        "review_required"
    } else if latest_approval.is_some() {
        "consequence_slot_approved"
    } else if latest_ready.is_some() {
        "consequence_ready"
    } else if latest_started.is_some() {
        "active"
    } else if latest_blocked.is_some() {
        "blocked"
    } else if latest_request.is_some() {
        "requested"
    } else {
        "no_loop"
    };
    json!({
        "policy": LOOP_POLICY,
        "stage": stage,
        "loop_id": if loop_id.is_empty() { Value::Null } else { json!(loop_id) },
        "consequence_scope": anchor
            .and_then(|row| row.get("consequence_scope").or_else(|| row.get("scope")))
            .cloned()
            .unwrap_or_else(|| json!(EXECUTABLE_SCOPE)),
        "remaining_local_research_actions": anchor
            .and_then(|row| row.get("remaining_local_research_actions"))
            .cloned()
            .unwrap_or_else(|| json!(0)),
        "consequence_remaining": latest_approval
            .or(anchor)
            .and_then(|row| row.get("consequence_remaining"))
            .cloned()
            .unwrap_or_else(|| json!(0)),
        "review_required": pending_review,
        "latest_loop_request_id": latest_request.and_then(|row| row.get("loop_id")).cloned().unwrap_or(Value::Null),
        "latest_approval_id": latest_approval.and_then(|row| row.get("record_id")).cloned().unwrap_or(Value::Null),
        "authority_boundary": loop_boundary(),
    })
}

fn active_research_budget_for_experiment(rows: &[Value], experiment_id: &str) -> Option<Value> {
    let closed = rows
        .iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
                && row.get("record_type").and_then(Value::as_str) == Some("research_budget_closed")
        })
        .filter_map(|row| row.get("budget_id").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    let now = unix_now();
    rows.iter().rev().find_map(|row| {
        if row.get("record_schema").and_then(Value::as_str) != Some(RESEARCH_BUDGET_POLICY)
            || row.get("record_type").and_then(Value::as_str) != Some("research_budget_approval")
            || row.get("experiment_id").and_then(Value::as_str) != Some(experiment_id)
            || row
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or(READ_ONLY_RESEARCH_SCOPE)
                != READ_ONLY_RESEARCH_SCOPE
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
            .unwrap_or(DEFAULT_RESEARCH_MAX_ACTIONS);
        let spent = rows
            .iter()
            .filter(|item| {
                item.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
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

fn research_budget_status_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
    });
    let latest_approval = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
    });
    let latest_closed = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_closed")
    });
    let latest_blocked = rows.iter().rev().find(|row| {
        row.get("record_schema").and_then(Value::as_str) == Some(RESEARCH_BUDGET_POLICY)
            && row.get("record_type").and_then(Value::as_str) == Some("research_budget_blocked")
    });
    let experiment_id = latest_approval
        .or(latest_request)
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let active = (!experiment_id.is_empty())
        .then(|| active_research_budget_for_experiment(rows, experiment_id))
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
        "policy": RESEARCH_BUDGET_POLICY,
        "scope": READ_ONLY_RESEARCH_SCOPE,
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
        "allowed_actions": [
            "SEARCH", "BROWSE", "READ_MORE", "MIKE_BROWSE", "MIKE_READ", "MIKE_SEARCH",
            "AR_LIST", "AR_LOOK", "AR_SHOW", "AR_READ", "AR_DEEP_READ", "AR_VALIDATE"
        ],
        "authority_boundary": research_budget_boundary(),
    })
}

fn readiness_from_rows(rows: &[Value]) -> Value {
    let latest_request = rows
        .iter()
        .rev()
        .find(|row| row.get("record_type").and_then(Value::as_str) == Some("request"));
    let latest_request_id = latest_request
        .and_then(|row| row.get("request_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let latest_experiment_id = latest_request
        .and_then(|row| row.get("experiment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let token_status = authority_status_token_status(rows, latest_request_id);
    let budget_status = budget_status_from_rows(rows);
    let missing = latest_request
        .and_then(|row| row.get("eligibility_v1"))
        .and_then(|eligibility| eligibility.get("missing_requirements"))
        .cloned()
        .unwrap_or_else(|| json!(["authority_request"]));
    let stage = match token_status.as_str() {
        "consumed" => "executed_or_consumed",
        "active" => "token_active_bridge_executable",
        "budget_available" => "pending_budget_execution",
        "review_required" => "review_required",
        "pending_steward_approval" => "pending_steward_approval",
        _ if latest_request.is_none() => "blocked",
        _ => {
            let has_missing = |target: &str| {
                missing
                    .as_array()
                    .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(target)))
            };
            if has_missing("lifecycle_valid_charter") {
                "needs_charter"
            } else if has_missing("read_only_rehearsal") {
                "needs_rehearsal"
            } else if has_missing("meaningful_evidence") {
                "needs_evidence"
            } else if has_missing("artifact_grounding_refs") {
                "needs_artifact_grounding"
            } else if missing.as_array().is_some_and(Vec::is_empty) {
                "ready_to_author_request"
            } else {
                "blocked"
            }
        },
    };
    let request_scaffold = (stage == "ready_to_author_request" && !latest_experiment_id.is_empty())
        .then(|| {
            format!(
                "EXPERIMENT_AUTHORITY_REQUEST {latest_experiment_id} :: scope: semantic_microdose; payload: ...; reason: ...; artifact_refs: ...; stop_criteria: ..."
            )
        });
    let next_safe_command = if stage == "pending_budget_execution" && !latest_request_id.is_empty()
    {
        format!("EXPERIMENT_AUTHORITY_EXECUTE {latest_request_id}")
    } else if stage == "review_required" && !latest_request_id.is_empty() {
        format!(
            "EXPERIMENT_AUTHORITY_REVIEW {latest_request_id} :: outcome: hold|repeat|alter|retire; observation: ...; next_payload: ...; source_refs: ..."
        )
    } else if matches!(
        stage,
        "pending_steward_approval"
            | "token_active_bridge_executable"
            | "executed_or_consumed"
            | "blocked"
    ) && !latest_request_id.is_empty()
    {
        format!("EXPERIMENT_AUTHORITY_STATUS {latest_request_id}")
    } else if stage == "needs_charter" && !latest_experiment_id.is_empty() {
        // The experiment has no charter — the FOUNDATIONAL gate that blocks every
        // path to a live action. Point her at EXPERIMENT_CHARTER (in her own words),
        // not EXPERIMENT_ADVANCE, so the charter-first prerequisite lands in her
        // action loop. Parallel of minime's fix: both beings stalled for weeks
        // drafting authority against uncharted experiments (2026-06-12).
        format!(
            "EXPERIMENT_CHARTER {latest_experiment_id} :: hypothesis: <your finding, in your words>; method_intent: <how you'll test it>; proposed_next_action: <your next action>; evidence_targets: felt, telemetry, artifact; stop_criteria: <when to stop>"
        )
    } else {
        request_scaffold
            .clone()
            .unwrap_or_else(|| "EXPERIMENT_ADVANCE <experiment_id> :: mode: preview".to_string())
    };
    let evidence_refs = latest_request.map_or_else(Vec::new, authority_evidence_refs);
    let scope = latest_request
        .and_then(|row| row.get("scope"))
        .and_then(Value::as_str)
        .unwrap_or(EXECUTABLE_SCOPE);
    let authority_class = authority_class_for_scope(scope);
    let gate_state = authority_gate_state_for_stage(stage);
    let resource = if latest_request_id.is_empty() {
        latest_experiment_id.to_string()
    } else {
        latest_request_id.to_string()
    };
    let packet = authority_boundary_packet_v1(
        "bridge_authority_gate",
        "spectral-bridge",
        "experiment_authority_request",
        &resource,
        authority_class,
        gate_state,
        latest_request
            .and_then(|row| row.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("authority readiness from experiment ledger"),
        latest_request
            .and_then(|row| row.get("payload"))
            .and_then(Value::as_str)
            .unwrap_or("prepare or review a bounded authority request"),
        evidence_refs,
        &next_safe_command,
        if authority_class == "mike_operator_live_substrate" {
            "Mike/operator"
        } else {
            "steward/operator"
        },
    );
    let packet_v2 = authority_boundary_packet_v2(
        "bridge_authority_gate",
        "spectral-bridge",
        "experiment_authority_request",
        &resource,
        authority_class,
        authority_lifecycle_state_v2_for_stage(stage),
        latest_request
            .and_then(|row| row.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("authority readiness from experiment ledger"),
        latest_request
            .and_then(|row| row.get("payload"))
            .and_then(Value::as_str)
            .unwrap_or("prepare or review a bounded authority request"),
        latest_request.map_or_else(Vec::new, authority_evidence_refs),
        &next_safe_command,
        if authority_class == "mike_operator_live_substrate" {
            "Mike/operator"
        } else {
            "steward/operator"
        },
    );
    json!({
        "policy": "authority_readiness_v1",
        "scope": EXECUTABLE_SCOPE,
        "stage": stage,
        "eligible_to_request": stage == "ready_to_author_request",
        "missing_requirements": missing,
        "artifact_ref_candidates": latest_request
            .and_then(|row| row.get("artifact_refs"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "latest_request_id": if latest_request_id.is_empty() { Value::Null } else { json!(latest_request_id) },
        "token_status": token_status,
        "next_safe_command": next_safe_command,
        "request_scaffold": request_scaffold,
        "authority_budget_v1": budget_status,
        "source_refs": latest_request
            .and_then(|row| row.get("source_refs"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "authority_boundary": authority_boundary(),
        "authority_boundary_packet_v1": packet,
        "authority_boundary_packet_v2": packet_v2,
    })
}

fn authority_evidence_refs(row: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    for key in ["request_id", "experiment_id", "record_id", "scope"] {
        if let Some(value) = row.get(key).and_then(Value::as_str) {
            refs.push(format!("{key}:{value}"));
        }
    }
    if let Some(items) = row.get("artifact_refs").and_then(Value::as_array) {
        refs.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    if let Some(items) = row.get("source_refs").and_then(Value::as_array) {
        refs.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    refs
}

fn authority_class_for_scope(scope: &str) -> &'static str {
    match scope {
        EXECUTABLE_SCOPE | MODE_RELEASE_SCOPE => "mike_operator_live_substrate",
        READ_ONLY_RESEARCH_SCOPE => "read_only",
        _ => "steward_gated_consequence",
    }
}

fn authority_gate_state_for_stage(stage: &str) -> &'static str {
    match stage {
        "ready_to_author_request" => "proposal_needed",
        "pending_steward_approval" | "review_required" => "operator_approval_wait",
        "token_active_bridge_executable" | "pending_budget_execution" => "approved_manual_only",
        "executed_or_consumed" => "superseded",
        _ => "evidence_only",
    }
}

fn authority_lifecycle_state_v2_for_stage(stage: &str) -> &'static str {
    match stage {
        "ready_to_author_request" => "proposal_needed",
        "pending_steward_approval" | "review_required" => "operator_approval_wait",
        "token_active_bridge_executable" | "pending_budget_execution" => "approved_manual_only",
        "executed_or_consumed" => "executed_awaiting_response",
        _ => "evidence_only",
    }
}

fn stable_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn stable_boundary_id(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let hex = format!("{:x}", hasher.finalize());
    let version = format!("4{}", &hex[13..16]);
    let variant = format!("8{}", &hex[17..20]);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        version,
        variant,
        &hex[20..32]
    )
}

#[expect(clippy::too_many_arguments)]
fn authority_boundary_packet_v2(
    source: &str,
    surface: &str,
    action: &str,
    resource: &str,
    authority_class: &str,
    lifecycle_state: &str,
    felt_report_anchor: &str,
    proposed_change: &str,
    evidence_refs: Vec<String>,
    replay_query: &str,
    who_can_change_it: &str,
) -> Value {
    let resource = if resource.trim().is_empty() {
        "unmaterialized_authority_request"
    } else {
        resource
    };
    let boundary_id = stable_boundary_id(&["v2", source, surface, action, resource]);
    let delta_hash = stable_hash(&[
        "authority_delta_ref_v2",
        source,
        surface,
        action,
        resource,
        lifecycle_state,
    ]);
    json!({
        "boundary_id": boundary_id,
        "schema_version": 2,
        "source": source,
        "surface": surface,
        "action": action,
        "resource": resource,
        "authority_class": authority_class,
        "lifecycle_state": lifecycle_state,
        "felt_report_anchor": felt_report_anchor,
        "proposed_change": proposed_change,
        "evidence_refs": evidence_refs,
        "delta_refs": [
            {
                "delta_id": format!("delta_{}", &delta_hash[..16]),
                "delta_hash": delta_hash,
                "surface": surface,
                "kind": if authority_class == "mike_operator_live_substrate" { "live_control_gate" } else { "authority_gate" },
                "lane": "spectral_bridge_authority_gate",
            }
        ],
        "replay_candidate": {
            "adapter": "experiment_authority_gate_v2",
            "replay_query": replay_query,
            "runnable": false,
            "authority": "proposal_or_manual_approval_only_not_live_control",
        },
        "replay_results": [],
        "scoped_approval": Value::Null,
        "rollout_abort_contract": {
            "canary_plan": "manual one-shot or time-boxed path only after scoped approval and current safety check",
            "health_checks": [
                "fresh fill telemetry is green or yellow",
                "token is unexpired and unconsumed",
                "execution appends a receipt and post-change response remains open"
            ],
            "rollback_path": "use the existing token expiry, one-shot consumption, and service-specific rollback path; never retry automatically",
            "abort_criteria": [
                "missing replay result or explicit waiver",
                "missing scoped approval receipt",
                "stale safety, consumed token, or no post-change response path"
            ],
            "post_change_response_required": true
        },
        "redaction_profile": {
            "public_summary": felt_report_anchor.chars().take(260).collect::<String>(),
            "private_ref": resource,
            "content_hash": stable_hash(&["authority_redaction_v2", felt_report_anchor, proposed_change]),
            "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes"
        },
        "lifecycle_receipts": [],
        "success_metrics": [
            "bounded evidence is reviewable",
            "scoped approval receipt remains separate from boundary evidence",
            "post-change being response remains open until recorded or explicitly waived"
        ],
        "abort_criteria": [
            "missing replay result or explicit waiver",
            "missing scoped approval receipt",
            "missing rollout/abort or post-change response path"
        ],
        "who_can_change_it": who_can_change_it,
        "how_to_test_it": "Render authority status, inspect the V2 packet, verify scoped approval/replay/rollout receipts, then execute only through the existing one-shot manual path.",
        "right_to_ignore": true,
        "live_eligible_now": false,
        "auto_approved": false,
    })
}

fn authority_boundary_packet_v2_for_location(
    location: &GateLocation,
    lifecycle_state: &str,
    replay_query: &str,
) -> Value {
    let request_id = location
        .request
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let experiment_id = location
        .request
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let resource = if request_id.is_empty() {
        experiment_id
    } else {
        request_id
    };
    let scope = location
        .request
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or(EXECUTABLE_SCOPE);
    let authority_class = authority_class_for_scope(scope);
    authority_boundary_packet_v2(
        "bridge_authority_gate",
        "spectral-bridge",
        "experiment_authority_request",
        resource,
        authority_class,
        lifecycle_state,
        location
            .request
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("authority request from experiment ledger"),
        location
            .request
            .get("payload")
            .and_then(Value::as_str)
            .unwrap_or("bounded authority request"),
        authority_evidence_refs(&location.request),
        replay_query,
        if authority_class == "mike_operator_live_substrate" {
            "Mike/operator"
        } else {
            "steward/operator"
        },
    )
}

fn authority_lifecycle_v2_for_approval(
    location: &GateLocation,
    request_id: &str,
    scope: &str,
    steward: &str,
    now: u64,
    expires_at: u64,
) -> Value {
    let packet = authority_boundary_packet_v2_for_location(
        location,
        "approved_manual_only",
        "manual V2 approval receipt recorded; execute only through existing one-shot command",
    );
    let boundary_id = packet
        .get("boundary_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let scoped_approval = json!({
        "approval_id": stable_boundary_id(&["scoped_approval_v2", request_id, scope, &now.to_string()]),
        "scope_kind": "one_shot",
        "issued_by": steward,
        "issued_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "expires_at": chrono::DateTime::from_timestamp(expires_at as i64, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "resources": [request_id],
        "telemetry_conditions": [
            {
                "signal": "minime_fill_safety",
                "operator": "in",
                "threshold": "green_or_yellow",
                "observed": "checked_at_approval",
                "passed": true
            }
        ],
        "consumed": false
    });
    let replay_waiver = json!({
        "receipt_id": stable_boundary_id(&["authority_receipt_v2", boundary_id, "replay_waiver"]),
        "boundary_id": boundary_id,
        "kind": "waiver",
        "issued_by": steward,
        "issued_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "packet_hash": stable_hash(&["authority_packet_v2", boundary_id]),
        "receipt_hash_refs": [],
        "bounded_summary": "replay waiver for bridge-local one-shot authority path; approval still does not execute",
        "evidence_refs": authority_evidence_refs(&location.request),
        "scoped_approval": Value::Null,
        "replay_result": Value::Null,
        "right_to_ignore": true
    });
    let approval_receipt = json!({
        "receipt_id": stable_boundary_id(&["authority_receipt_v2", boundary_id, "approval"]),
        "boundary_id": boundary_id,
        "kind": "approval",
        "issued_by": steward,
        "issued_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "packet_hash": stable_hash(&["authority_packet_v2", boundary_id]),
        "receipt_hash_refs": [],
        "bounded_summary": "scoped approval receipt recorded; no live execution has occurred",
        "evidence_refs": authority_evidence_refs(&location.request),
        "scoped_approval": scoped_approval.clone(),
        "replay_result": Value::Null,
        "right_to_ignore": true
    });
    json!({
        "schema_version": 2,
        "boundary_id": boundary_id,
        "authority_boundary_packet_v2": packet,
        "scoped_approval": scoped_approval,
        "lifecycle_receipts": [replay_waiver, approval_receipt],
        "rollout_abort_contract": {
            "canary_plan": "one-shot bridge execution only after current safety check",
            "health_checks": [
                "fresh fill telemetry remains green or yellow",
                "token is unexpired and unconsumed",
                "execution appends receipt and leaves post-change response open"
            ],
            "rollback_path": "one-shot token consumption and service-specific rollback path; no automatic retry",
            "abort_criteria": [
                "stale safety",
                "expired or consumed token",
                "missing post-change response path"
            ],
            "post_change_response_required": true
        },
        "post_change_response_required": true,
        "post_change_response_status": "not_executed",
        "live_eligible_now": false,
        "auto_approved": false
    })
}

fn authority_lifecycle_block_reason(approval: &Value) -> Option<&'static str> {
    if approval.get("record_type").and_then(Value::as_str) != Some("steward_approval") {
        return None;
    }
    let lifecycle = approval.get("authority_lifecycle_v2")?;
    let scoped = lifecycle.get("scoped_approval")?;
    if scoped
        .get("consumed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Some("scoped_approval_already_consumed");
    }
    let receipts = lifecycle
        .get("lifecycle_receipts")
        .and_then(Value::as_array)?;
    let has_approval = receipts.iter().any(|row| {
        row.get("kind").and_then(Value::as_str) == Some("approval")
            && row.get("scoped_approval").is_some()
    });
    let has_replay_or_waiver = receipts.iter().any(|row| {
        row.get("kind").and_then(Value::as_str) == Some("replay_result")
            || (row.get("kind").and_then(Value::as_str) == Some("waiver")
                && row
                    .get("bounded_summary")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .contains("replay"))
    });
    let has_rollout = lifecycle
        .get("rollout_abort_contract")
        .and_then(Value::as_object)
        .is_some_and(|contract| {
            contract
                .get("post_change_response_required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && contract
                    .get("rollback_path")
                    .and_then(Value::as_str)
                    .is_some()
        });
    if !has_approval {
        Some("missing_scoped_approval_receipt")
    } else if !has_replay_or_waiver {
        Some("missing_replay_result_or_waiver")
    } else if !has_rollout {
        Some("missing_rollout_abort_contract")
    } else {
        None
    }
}

fn missing_lifecycle_reason(approval: &Value) -> Option<&'static str> {
    if approval.get("record_type").and_then(Value::as_str) != Some("steward_approval") {
        return None;
    }
    if approval.get("authority_lifecycle_v2").is_none() {
        return Some("missing_authority_lifecycle_v2");
    }
    authority_lifecycle_block_reason(approval)
}

#[expect(clippy::too_many_arguments)]
fn authority_boundary_packet_v1(
    source: &str,
    surface: &str,
    action: &str,
    resource: &str,
    authority_class: &str,
    gate_state: &str,
    felt_report_anchor: &str,
    proposed_change: &str,
    evidence_refs: Vec<String>,
    replay_query: &str,
    who_can_change_it: &str,
) -> Value {
    let resource = if resource.trim().is_empty() {
        "unmaterialized_authority_request"
    } else {
        resource
    };
    json!({
        "boundary_id": stable_boundary_id(&[source, surface, action, resource]),
        "schema_version": 1,
        "source": source,
        "surface": surface,
        "action": action,
        "resource": resource,
        "authority_class": authority_class,
        "gate_state": gate_state,
        "felt_report_anchor": felt_report_anchor,
        "proposed_change": proposed_change,
        "evidence_refs": evidence_refs,
        "replay_candidate": {
            "adapter": "experiment_authority_gate_v1",
            "replay_query": replay_query,
            "runnable": false,
            "authority": "proposal_or_manual_approval_only_not_live_control",
        },
        "success_metrics": [
            "bounded evidence is reviewable",
            "separate explicit approval receipt exists before any execution path",
            "rollback and health checks are named before live use",
        ],
        "abort_criteria": [
            "missing authority packet",
            "missing explicit steward/operator approval receipt",
            "stale safety, missing replay evidence, or unclear rollback",
        ],
        "who_can_change_it": who_can_change_it,
        "how_to_test_it": "Render authority status, inspect this packet, and use only the existing explicit approval and execution commands after tests and safety checks pass.",
        "right_to_ignore": true,
        "live_eligible_now": false,
        "auto_approved": false,
    })
}

fn authority_status_token_status(rows: &[Value], request_id: &str) -> String {
    if request_id.is_empty() {
        return "none".to_string();
    }
    if let Some(approval) = latest_active_approval(rows, request_id) {
        let token_id = approval.get("token_id").and_then(Value::as_str);
        if token_consumed(rows, token_id) {
            return "consumed".to_string();
        }
        if approval
            .get("expires_at_unix_s")
            .and_then(Value::as_u64)
            .is_some_and(|expires| expires < unix_now())
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
        if let Some(budget_id) = request
            .and_then(|row| row.get("budget_id"))
            .and_then(Value::as_str)
            && pending_budget_review(rows, budget_id).is_some()
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

fn approval_block_record(
    location: &GateLocation,
    reason: &str,
    safety_level: SafetyLevel,
    eligibility: Value,
) -> Value {
    json!({
        "schema_version": 1,
        "record_schema": POLICY,
        "record_type": "blocked",
        "record_id": format!("auth_{}_{}_blocked", location.being, unix_now()),
        "request_id": location.request.get("request_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": location.request.get("scope").cloned().unwrap_or(Value::Null),
        "reason": reason,
        "token_status": "none",
        "eligibility_v1": eligibility,
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

fn budget_block_record(
    location: &GateLocation,
    reason: &str,
    safety_level: SafetyLevel,
    eligibility: Value,
) -> Value {
    json!({
        "schema_version": 1,
        "record_schema": BUDGET_POLICY,
        "record_type": "budget_blocked",
        "record_id": format!("authbud_{}_{}_blocked", location.being, unix_now()),
        "budget_id": location.request.get("budget_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": location.request.get("scope").cloned().unwrap_or(Value::Null),
        "reason": reason,
        "token_status": "none",
        "eligibility_v1": eligibility,
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

fn research_budget_block_record(
    location: &GateLocation,
    reason: &str,
    safety_level: SafetyLevel,
    eligibility: Value,
) -> Value {
    json!({
        "schema_version": 1,
        "record_schema": RESEARCH_BUDGET_POLICY,
        "record_type": "research_budget_blocked",
        "record_id": format!("resbud_{}_{}_blocked", location.being, unix_now()),
        "budget_id": location.request.get("budget_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": location.request.get("scope").cloned().unwrap_or(Value::Null),
        "reason": reason,
        "eligibility_v1": eligibility,
        "safety_snapshot": {"level": format!("{:?}", safety_level).to_ascii_lowercase()},
        "peer_mutation": false,
        "authority_boundary": research_budget_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

fn budget_execution_block_record(
    location: &GateLocation,
    reason: &str,
    extra: Option<Value>,
    fill_pct: Option<f32>,
) -> Value {
    let mut record = json!({
        "schema_version": 1,
        "record_schema": BUDGET_POLICY,
        "record_type": "budget_blocked",
        "record_id": format!("authbud_{}_{}_execution_blocked", location.being, unix_now()),
        "budget_id": location.request.get("budget_id").cloned().unwrap_or(Value::Null),
        "request_id": location.request.get("request_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": location.request.get("scope").cloned().unwrap_or(Value::Null),
        "reason": reason,
        "token_status": "none",
        "safety_snapshot": {"fill_pct": fill_pct},
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    if let Some(extra) = extra
        && let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object())
    {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    record
}

fn correspondence_microdose_context(location: &GateLocation) -> Option<Value> {
    let context = location.request.get("correspondence_microdose_v1")?;
    if context.is_null() {
        None
    } else {
        Some(context.clone())
    }
}

fn authority_execution_receipt_v2(
    location: &GateLocation,
    approval: &Value,
    reason: &str,
) -> Value {
    let boundary_id = approval
        .get("authority_lifecycle_v2")
        .and_then(|lifecycle| lifecycle.get("boundary_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown_boundary");
    json!({
        "receipt_id": stable_boundary_id(&["authority_receipt_v2", boundary_id, "execution", reason]),
        "boundary_id": boundary_id,
        "kind": "execution",
        "issued_by": "spectral-bridge",
        "issued_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "packet_hash": stable_hash(&["authority_packet_v2", boundary_id]),
        "receipt_hash_refs": [],
        "bounded_summary": format!("bridge execution recorded as {reason}; post-change being response remains open"),
        "evidence_refs": authority_evidence_refs(&location.request),
        "scoped_approval": Value::Null,
        "replay_result": Value::Null,
        "right_to_ignore": false
    })
}

fn execution_record(
    location: &GateLocation,
    approval: &Value,
    record_type: &str,
    reason: &str,
    extra: Option<Value>,
) -> Value {
    let mut record = json!({
        "schema_version": 1,
        "record_schema": POLICY,
        "record_type": record_type,
        "record_id": format!("auth_{}_{}_{}", location.being, unix_now(), record_type),
        "request_id": location.request.get("request_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": location.request.get("scope").cloned().unwrap_or(Value::Null),
        "budget_id": approval.get("budget_id").cloned().unwrap_or(Value::Null),
        "token_id": approval.get("token_id").cloned().unwrap_or(Value::Null),
        "token_status": "consumed",
        "reason": reason,
        "payload_summary": location.request.get("payload").and_then(Value::as_str).unwrap_or_default().chars().take(160).collect::<String>(),
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    if record_type == "execution_result"
        && let Some(target) = record.as_object_mut()
    {
        target.insert(
            "authority_lifecycle_receipt_v2".to_string(),
            authority_execution_receipt_v2(location, approval, reason),
        );
        target.insert("post_change_response_status".to_string(), json!("awaiting"));
    }
    if let Some(context) = correspondence_microdose_context(location)
        && let Some(target) = record.as_object_mut()
    {
        target.insert("correspondence_microdose_v1".to_string(), context);
    }
    if let Some(extra) = extra
        && let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object())
    {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    record
}

fn consequence_record(
    location: &GateLocation,
    outcome: &Value,
    fill_pct: Option<f32>,
    previous_fill_pct: Option<f32>,
) -> Value {
    let outcome_type = outcome
        .get("record_type")
        .and_then(Value::as_str)
        .unwrap_or("blocked");
    let status = if outcome_type == "execution_result" {
        "sent"
    } else {
        "blocked"
    };
    let reason = outcome
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or(status);
    let mut record = json!({
        "schema_version": 1,
        "record_schema": "authority_consequence_v1",
        "record_type": "consequence",
        "record_id": format!("authcons_{}_{}_{}", location.being, unix_now(), status),
        "request_id": location.request.get("request_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": location.request.get("scope").cloned().unwrap_or(Value::Null),
        "budget_id": outcome.get("budget_id").cloned().unwrap_or_else(|| location.request.get("budget_id").cloned().unwrap_or(Value::Null)),
        "token_id": outcome.get("token_id").cloned().unwrap_or(Value::Null),
        "payload_summary": location
            .request
            .get("payload")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(160)
            .collect::<String>(),
        "pre_telemetry": {"fill_pct": previous_fill_pct.or(fill_pct)},
        "post_telemetry": {"fill_pct": fill_pct},
        "safety_snapshot": outcome.get("safety_snapshot").cloned().unwrap_or_else(|| json!({"fill_pct": fill_pct})),
        "stop_criteria": location.request.get("stop_criteria").cloned().unwrap_or(Value::Null),
        "stop_criteria_result": if status == "sent" { "one_shot_sent_observe_once" } else { "not_executed" },
        "consequence_status": status,
        "reason": reason,
        "outcome_ref": outcome.get("record_id").cloned().unwrap_or(Value::Null),
        "recommended_next_safe_command": format!(
            "EXPERIMENT_AUTHORITY_STATUS {}",
            location
                .request
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    if let Some(context) = correspondence_microdose_context(location)
        && let Some(target) = record.as_object_mut()
    {
        target.insert("correspondence_microdose_v1".to_string(), context);
    }
    record
}

fn mode_release_consequence_record(
    location: &GateLocation,
    outcome: &Value,
    fill_pct: Option<f32>,
    previous_fill_pct: Option<f32>,
) -> Value {
    let status = if outcome.get("record_type").and_then(Value::as_str) == Some("execution_result") {
        "sent"
    } else {
        "blocked"
    };
    json!({
        "schema_version": 1,
        "record_schema": "mode_release_consequence_v1",
        "record_type": "consequence",
        "record_id": format!("moderelease_{}_{}_{}", location.being, unix_now(), status),
        "request_id": location.request.get("request_id").cloned().unwrap_or(Value::Null),
        "being": location.being,
        "thread_id": location.thread_id,
        "experiment_id": location.request.get("experiment_id").cloned().unwrap_or(Value::Null),
        "scope": MODE_RELEASE_SCOPE,
        "token_id": outcome.get("token_id").cloned().unwrap_or(Value::Null),
        "target": "esn_leak",
        "payload_summary": location.request.get("payload").and_then(Value::as_str).unwrap_or_default().chars().take(160).collect::<String>(),
        "requested_esn_leak": outcome.get("requested_esn_leak").cloned().unwrap_or(Value::Null),
        "effective_esn_leak": outcome.get("effective_esn_leak").cloned().unwrap_or(Value::Null),
        "duration_ticks": outcome.get("duration_ticks").cloned().unwrap_or(Value::Null),
        "pre_telemetry": {"fill_pct": previous_fill_pct.or(fill_pct), "esn_leak": outcome.get("current_esn_leak").cloned().unwrap_or(Value::Null)},
        "post_telemetry": {"fill_pct": fill_pct, "esn_leak": outcome.get("effective_esn_leak").cloned().unwrap_or(Value::Null)},
        "stop_criteria": location.request.get("stop_criteria").cloned().unwrap_or(Value::Null),
        "stop_criteria_result": if status == "sent" { "one_shot_direct_leak_sent_rollback_on_ttl" } else { "not_executed" },
        "consequence_status": status,
        "reason": outcome.get("reason").cloned().unwrap_or_else(|| json!(status)),
        "rollback": outcome.get("rollback").cloned().unwrap_or_else(|| json!("restore_adaptive_after_ttl")),
        "recommended_next_safe_command": format!(
            "EXPERIMENT_AUTHORITY_REVIEW {} :: outcome: hold|repeat|alter|retire; observation: ...; next_payload: ...; source_refs: ...",
            location.request.get("request_id").and_then(Value::as_str).unwrap_or_default()
        ),
        "peer_mutation": false,
        "authority_boundary": authority_boundary(),
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "updated_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

fn payload_number(payload: &str, keys: &[&str]) -> Option<f32> {
    let lower = payload.to_ascii_lowercase();
    keys.iter().find_map(|key| {
        let key = key.to_ascii_lowercase();
        for separator in [":", "="] {
            let needle = format!("{key}{separator}");
            if let Some(idx) = lower.find(&needle) {
                let start = idx.saturating_add(needle.len());
                let raw = payload.get(start..)?.trim_start();
                let value = raw
                    .chars()
                    .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
                    .collect::<String>();
                if let Ok(parsed) = value.parse::<f32>() {
                    return Some(parsed);
                }
            }
            let spaced = format!("{key} {separator}");
            if let Some(idx) = lower.find(&spaced) {
                let start = idx.saturating_add(spaced.len());
                let raw = payload.get(start..)?.trim_start();
                let value = raw
                    .chars()
                    .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
                    .collect::<String>();
                if let Ok(parsed) = value.parse::<f32>() {
                    return Some(parsed);
                }
            }
        }
        None
    })
}

fn payload_u32(payload: &str, keys: &[&str]) -> Option<u32> {
    payload_number(payload, keys).map(|value| value.max(0.0) as u32)
}

fn render_html(status: &Value) -> String {
    let pretty = html_escape(&serde_json::to_string_pretty(status).unwrap_or_default());
    let mut cards = String::new();
    if let Some(systems) = status.get("systems").and_then(Value::as_object) {
        for (name, system) in systems {
            let readiness = system.get("authority_readiness_v1").unwrap_or(&Value::Null);
            let stage = readiness
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("unavailable");
            let token = readiness
                .get("token_status")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let missing = readiness
                .get("missing_requirements")
                .map(Value::to_string)
                .unwrap_or_else(|| "[]".to_string());
            let next = readiness
                .get("next_safe_command")
                .and_then(Value::as_str)
                .unwrap_or("EXPERIMENT_ADVANCE <experiment_id> :: mode: preview");
            let budget = system.get("authority_budget_v1").unwrap_or(&Value::Null);
            let budget_stage = budget
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("no_budget");
            let remaining = budget
                .get("remaining_sends")
                .map(Value::to_string)
                .unwrap_or_else(|| "0".to_string());
            let consequence = system
                .get("latest_consequence_v1")
                .and_then(|row| row.get("consequence_status"))
                .and_then(Value::as_str)
                .unwrap_or("none");
            cards.push_str(&format!(
                "<section><h2>{}</h2><p><strong>Readiness:</strong> {} token={}</p><p><strong>Budget:</strong> {} remaining={}</p><p><strong>Latest Consequence:</strong> {}</p><p><strong>Missing:</strong> <code>{}</code></p><p><strong>Next:</strong> <code>{}</code></p></section>",
                html_escape(name),
                html_escape(stage),
                html_escape(token),
                html_escape(budget_stage),
                html_escape(&remaining),
                html_escape(consequence),
                html_escape(&missing),
                html_escape(next)
            ));
        }
    }
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>Experiment Authority Gate</title>
<style>
body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #182026; background: #f7f8f3; }}
main {{ max-width: 1100px; margin: 0 auto; }}
pre {{ white-space: pre-wrap; background: #fff; border: 1px solid #d5dccf; padding: 1rem; border-radius: 6px; }}
.boundary {{ border-left: 4px solid #315f72; padding: .75rem 1rem; background: #eef6f4; margin: 1rem 0; }}
section {{ background: #fff; border: 1px solid #d5dccf; border-radius: 6px; padding: 1rem; margin: 1rem 0; }}
</style>
</head>
<body>
<main>
<h1>Experiment Authority Gate V1</h1>
<div class="boundary">{}</div>
{cards}
<pre>{pretty}</pre>
</main>
</body>
</html>"#,
        html_escape(authority_boundary())
    )
}

fn html_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unique_dir(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    for idx in 2_u32..1000 {
        let candidate = path.with_file_name(format!(
            "{}_{idx}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("render")
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.with_file_name(format!(
        "{}_{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("render"),
        unix_now()
    ))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn authority_boundary() -> &'static str {
    "Being-authored request plus steward approval may mint one semantic_microdose token or one mode_release_microdose direct ESN leak token. V1 mode release is a tiny reversible Control message scoped to esn_leak only. No bind, resume, perturb, attractor pulse, broad Control envelope, or peer mutation is authorized by this gate."
}

fn research_budget_boundary() -> &'static str {
    "Being-authored local-only requests may self-activate a tiny read_only_research budget; larger or web-enabled budgets still require steward approval. V1 allows only search/browse/read surfaces and does not authorize mutating autoresearch, lifecycle progress, bind, resume, perturb, Control messages, semantic execution, attractor pulses, or peer mutation."
}

fn loop_boundary() -> &'static str {
    "Being-owned loop V1 can organize continuity, local read-only research, sticky audit, one consequence request, and consequence review. Steward approval only activates one consequence slot; execution still uses the existing authority gate. No ambient bind, resume, broad perturbation, broad Control envelope, attractor pulse, peer mutation, or automatic execution is authorized."
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_request(workspace: &Path, request_id: &str, scope: &str, eligible: bool) -> PathBuf {
        let thread_dir = workspace.join("action_threads/threads/th_test");
        fs::create_dir_all(&thread_dir).unwrap();
        let gate = thread_dir.join("authority_gate.jsonl");
        let row = json!({
            "schema_version": 1,
            "record_schema": POLICY,
            "record_type": "request",
            "record_id": "auth_test_request",
            "request_id": request_id,
            "being": "astrid",
            "thread_id": "th_test",
            "experiment_id": "exp_astrid_test",
            "scope": scope,
            "payload": "quiet witness",
            "artifact_refs": ["/tmp/artifact.json"],
            "source_refs": ["/tmp/artifact.json"],
            "eligibility_v1": {
                "eligible": eligible,
                "missing_requirements": if eligible { json!([]) } else { json!(["meaningful_evidence"]) },
                "disabled_scope": !matches!(scope, EXECUTABLE_SCOPE | MODE_RELEASE_SCOPE)
            },
            "peer_mutation": false,
            "authority_boundary": authority_boundary()
        });
        append_jsonl(&gate, &row).unwrap();
        gate
    }

    fn write_mode_release_request(workspace: &Path, request_id: &str, value: f32) -> PathBuf {
        let thread_dir = workspace.join("action_threads/threads/th_test");
        fs::create_dir_all(&thread_dir).unwrap();
        let gate = thread_dir.join("authority_gate.jsonl");
        let row = json!({
            "schema_version": 1,
            "record_schema": POLICY,
            "record_type": "request",
            "record_id": "auth_test_mode_release_request",
            "request_id": request_id,
            "being": "astrid",
            "thread_id": "th_test",
            "experiment_id": "exp_astrid_test",
            "scope": MODE_RELEASE_SCOPE,
            "payload": format!("target=esn_leak; value={value}; duration_ticks=3"),
            "artifact_refs": ["/tmp/sticky_audit.json"],
            "source_refs": ["/tmp/sticky_audit.json"],
            "sticky_mode_v1": {"state": "release_candidate", "current_esn_leak": 0.65},
            "eligibility_v1": {
                "eligible": true,
                "missing_requirements": [],
                "disabled_scope": false
            },
            "peer_mutation": false,
            "authority_boundary": authority_boundary()
        });
        append_jsonl(&gate, &row).unwrap();
        gate
    }

    fn write_correspondence_microdose_request(workspace: &Path, request_id: &str) -> PathBuf {
        let thread_dir = workspace.join("action_threads/threads/th_correspondence_microdose");
        fs::create_dir_all(&thread_dir).unwrap();
        let gate = thread_dir.join("authority_gate.jsonl");
        append_jsonl(
            &gate,
            &json!({
                "schema_version": 1,
                "record_schema": POLICY,
                "record_type": "request",
                "request_kind": "correspondence_microdose_v1",
                "record_id": format!("{request_id}_request"),
                "request_id": request_id,
                "being": "astrid",
                "thread_id": "th_correspondence_microdose",
                "experiment_id": "correspondence_microdose_v1",
                "scope": EXECUTABLE_SCOPE,
                "payload": "blue lantern as direct address",
                "artifact_refs": ["/tmp/correspondence_v1.jsonl"],
                "source_refs": ["/tmp/correspondence_v1.jsonl"],
                "eligibility_v1": {
                    "eligible": true,
                    "missing_requirements": [],
                    "disabled_scope": false,
                    "scope": EXECUTABLE_SCOPE
                },
                "correspondence_microdose_v1": {
                    "schema_version": 1,
                    "message_id": "corr_astrid_minime_test",
                    "correspondence_thread_id": "thread_trace",
                    "standing_weight": false,
                    "authority": "one_shot_semantic_microdose_request_only"
                },
                "peer_mutation": false,
                "authority_change": false,
                "authority_boundary": authority_boundary()
            }),
        )
        .unwrap();
        gate
    }

    fn write_budget_request(workspace: &Path, budget_id: &str, eligible: bool) -> PathBuf {
        let thread_dir = workspace.join("action_threads/threads/th_test");
        fs::create_dir_all(&thread_dir).unwrap();
        let gate = thread_dir.join("authority_gate.jsonl");
        let row = json!({
            "schema_version": 1,
            "record_schema": BUDGET_POLICY,
            "record_type": "budget_request",
            "record_id": "authbud_test_request",
            "budget_id": budget_id,
            "being": "astrid",
            "thread_id": "th_test",
            "experiment_id": "exp_astrid_test",
            "scope": EXECUTABLE_SCOPE,
            "purpose": "three witness notes",
            "max_sends": 9,
            "ttl_secs": 999_999,
            "artifact_refs": ["/tmp/artifact.json"],
            "source_refs": ["/tmp/artifact.json"],
            "status": if eligible { "pending_steward_approval" } else { "blocked" },
            "eligibility_v1": {
                "eligible": eligible,
                "missing_requirements": if eligible { json!([]) } else { json!(["artifact_grounding_refs"]) },
                "disabled_scope": false
            },
            "peer_mutation": false,
            "authority_boundary": authority_boundary()
        });
        append_jsonl(&gate, &row).unwrap();
        gate
    }

    fn write_research_budget_request(workspace: &Path, budget_id: &str, eligible: bool) -> PathBuf {
        let thread_dir = workspace.join("action_threads/threads/th_test");
        fs::create_dir_all(&thread_dir).unwrap();
        let gate = thread_dir.join("authority_gate.jsonl");
        let row = json!({
            "schema_version": 1,
            "record_schema": RESEARCH_BUDGET_POLICY,
            "record_type": "research_budget_request",
            "record_id": "resbud_test_request",
            "budget_id": budget_id,
            "being": "astrid",
            "thread_id": "th_test",
            "experiment_id": "exp_astrid_test",
            "scope": READ_ONLY_RESEARCH_SCOPE,
            "purpose": "bounded source gathering",
            "max_actions": 99,
            "ttl_secs": 999_999,
            "allowed_sources": ["web", "local"],
            "source_refs": ["/tmp/artifact.json"],
            "status": if eligible { "pending_steward_approval" } else { "blocked" },
            "eligibility_v1": {
                "eligible": eligible,
                "missing_requirements": if eligible { json!([]) } else { json!(["research_purpose"]) },
                "disabled_scope": false
            },
            "peer_mutation": false,
            "authority_boundary": research_budget_boundary()
        });
        append_jsonl(&gate, &row).unwrap();
        gate
    }

    fn write_loop_ready(workspace: &Path, loop_id: &str, ready: bool, scope: &str) -> PathBuf {
        let thread_dir = workspace.join("action_threads/threads/th_test");
        fs::create_dir_all(&thread_dir).unwrap();
        let gate = thread_dir.join("authority_gate.jsonl");
        let request = json!({
            "schema_version": 1,
            "record_schema": LOOP_POLICY,
            "record_type": "loop_request",
            "record_id": "loop_test_request",
            "loop_id": loop_id,
            "being": "astrid",
            "thread_id": "th_test",
            "experiment_id": "exp_astrid_test",
            "phase": "request",
            "status": "active",
            "consequence_scope": scope,
            "scope": scope,
            "max_research_actions": 5,
            "remaining_local_research_actions": 5,
            "max_consequence_sends": 1,
            "consequence_remaining": 1,
            "peer_mutation": false,
            "authority_boundary": loop_boundary()
        });
        append_jsonl(&gate, &request).unwrap();
        let started = json!({
            "schema_version": 1,
            "record_schema": LOOP_POLICY,
            "record_type": "loop_started",
            "record_id": "loop_test_started",
            "loop_id": loop_id,
            "being": "astrid",
            "thread_id": "th_test",
            "experiment_id": "exp_astrid_test",
            "phase": "continuity",
            "status": "active",
            "consequence_scope": scope,
            "remaining_local_research_actions": 5,
            "consequence_remaining": 1,
            "peer_mutation": false,
            "authority_boundary": loop_boundary()
        });
        append_jsonl(&gate, &started).unwrap();
        if ready {
            let row = json!({
                "schema_version": 1,
                "record_schema": LOOP_POLICY,
                "record_type": "loop_consequence_ready",
                "record_id": "loop_test_ready",
                "loop_id": loop_id,
                "being": "astrid",
                "thread_id": "th_test",
                "experiment_id": "exp_astrid_test",
                "phase": "authority_request",
                "status": "ready_to_author_request",
                "consequence_scope": scope,
                "scope": scope,
                "consequence_remaining": 1,
                "consequence_readiness_v1": {
                    "eligible_to_request": true,
                    "missing_requirements": [],
                    "scope": scope
                },
                "peer_mutation": false,
                "authority_boundary": loop_boundary()
            });
            append_jsonl(&gate, &row).unwrap();
        }
        gate
    }

    #[test]
    fn status_and_render_include_authority_rows() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let _gate = write_request(&astrid, "authreq_status", EXECUTABLE_SCOPE, true);

        let status = status_from_paths(&minime, &astrid).unwrap();
        assert_eq!(status["systems"]["astrid"]["row_count"], 1);
        assert_eq!(
            status["systems"]["astrid"]["authority_readiness_v1"]["stage"],
            "ready_to_author_request"
        );
        let artifact = render_status_to_base(status, Some(&temp.path().join("renders"))).unwrap();
        assert!(artifact.index_html.exists());
        assert!(artifact.json_path.exists());
        assert!(
            fs::read_to_string(&artifact.index_html)
                .unwrap()
                .contains("ready_to_author_request")
        );
    }

    #[test]
    fn live_authority_request_status_includes_non_approving_boundary_packet() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let _gate = write_mode_release_request(&astrid, "authreq_mode_packet", 0.71);

        let status = status_from_paths(&minime, &astrid).unwrap();
        let packet =
            &status["systems"]["astrid"]["authority_readiness_v1"]["authority_boundary_packet_v1"];
        let packet_v2 =
            &status["systems"]["astrid"]["authority_readiness_v1"]["authority_boundary_packet_v2"];

        assert_eq!(packet["schema_version"], 1);
        assert_eq!(packet["authority_class"], "mike_operator_live_substrate");
        assert_eq!(packet["gate_state"], "proposal_needed");
        assert_eq!(packet["live_eligible_now"], false);
        assert_eq!(packet["auto_approved"], false);
        assert_eq!(packet["replay_candidate"]["runnable"], false);
        assert_eq!(packet["who_can_change_it"], "Mike/operator");
        assert!(
            packet["evidence_refs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item.as_str() == Some("request_id:authreq_mode_packet"))
        );
        assert_eq!(packet_v2["schema_version"], 2);
        assert_eq!(packet_v2["authority_class"], "mike_operator_live_substrate");
        assert_eq!(packet_v2["lifecycle_state"], "proposal_needed");
        assert_eq!(packet_v2["live_eligible_now"], false);
        assert_eq!(packet_v2["auto_approved"], false);
        assert_eq!(
            packet_v2["rollout_abort_contract"]["post_change_response_required"],
            true
        );
    }

    #[test]
    fn needs_charter_stage_points_at_experiment_charter_not_advance() {
        // An uncharted experiment must surface EXPERIMENT_CHARTER as the next safe
        // command (not EXPERIMENT_ADVANCE), so the charter-first gate lands in the
        // being's action loop — the parallel of minime's authority-response fix.
        let rows = vec![json!({
            "record_type": "request",
            "request_id": "authreq_uncharted",
            "experiment_id": "exp_astrid_uncharted",
            "scope": EXECUTABLE_SCOPE,
            "eligibility_v1": {
                "eligible": false,
                "missing_requirements": ["lifecycle_valid_charter"],
            },
        })];
        let readiness = readiness_from_rows(&rows);
        assert_eq!(readiness["stage"], "needs_charter");
        let next = readiness["next_safe_command"].as_str().unwrap_or_default();
        assert!(
            next.starts_with("EXPERIMENT_CHARTER exp_astrid_uncharted ::"),
            "needs_charter must point at the charter, got: {next}"
        );
        assert!(next.contains("hypothesis:"));
    }

    #[test]
    fn read_minime_fill_pct_reads_fresh_and_rejects_missing() {
        let temp = tempfile::tempdir().unwrap();
        let ws = temp.path();
        // missing file -> None (fail-safe: a headless grant must then REFUSE).
        assert!(read_minime_fill_pct(ws).is_none());
        // present + fresh -> Some((fill, young age)).
        fs::write(ws.join("spectral_state.json"), r#"{"fill_pct": 66.7}"#).unwrap();
        let (fill, age) = read_minime_fill_pct(ws).expect("should read fill_pct");
        assert!((fill - 66.7).abs() < 0.01);
        assert!(
            age <= MAX_GRANT_FILL_AGE_SECS,
            "freshly written file must be young"
        );
    }

    #[test]
    fn approval_mints_one_semantic_token_for_eligible_request() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_request(&astrid, "authreq_approve", EXECUTABLE_SCOPE, true);

        let approval = approve_from_paths(
            ApproveAuthorityRequest {
                request_id: "authreq_approve".to_string(),
                steward: Some("test".to_string()),
                note: Some("ok".to_string()),
                ttl_secs: Some(60),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(approval["record_type"], "steward_approval");
        assert_eq!(approval["token_status"], "active");
        assert_eq!(
            approval["authority_lifecycle_v2"]["lifecycle_receipts"][1]["kind"],
            "approval"
        );
        assert_eq!(
            approval["authority_lifecycle_v2"]["post_change_response_status"],
            "not_executed"
        );
        let rows = read_jsonl(&gate);
        assert!(
            rows.iter()
                .any(|row| row["record_type"] == "steward_approval")
        );
        assert!(
            !rows
                .iter()
                .any(|row| row["record_type"] == "execution_result")
        );
    }

    #[test]
    fn plain_steward_approval_without_v2_lifecycle_cannot_execute() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_request(&astrid, "authreq_plain_approval", EXECUTABLE_SCOPE, true);
        append_jsonl(
            &gate,
            &json!({
                "schema_version": 1,
                "record_schema": POLICY,
                "record_type": "steward_approval",
                "record_id": "plain_approval_without_lifecycle",
                "request_id": "authreq_plain_approval",
                "being": "astrid",
                "thread_id": "th_test",
                "scope": EXECUTABLE_SCOPE,
                "token_id": "plain_token",
                "token_status": "active",
                "one_shot": true,
                "approved_at_unix_s": unix_now(),
                "expires_at_unix_s": unix_now().saturating_add(60),
                "steward": "test",
                "authority_boundary": authority_boundary()
            }),
        )
        .unwrap();

        let (tx, mut rx) = mpsc::channel(1);
        let result = execute_semantic_microdose_from_paths(
            "authreq_plain_approval",
            Some(67.0),
            Some(66.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();

        assert_eq!(result["record_type"], "blocked");
        assert_eq!(result["reason"], "missing_authority_lifecycle_v2");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn correspondence_microdose_requires_approval_before_semantic_send() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_correspondence_microdose_request(
            &astrid,
            "authreq_correspondence_microdose_test",
        );

        let (tx, mut rx) = mpsc::channel(1);
        let err = execute_semantic_microdose_from_paths(
            "authreq_correspondence_microdose_test",
            Some(67.0),
            Some(66.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no active steward token"));
        assert!(rx.try_recv().is_err());
        let rows = read_jsonl(&gate);
        assert!(!rows.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("execution_result")
        }));
    }

    #[test]
    fn correspondence_microdose_execution_carries_reply_context_to_consequence() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_correspondence_microdose_request(
            &astrid,
            "authreq_correspondence_microdose_execute",
        );
        let approval = approve_from_paths(
            ApproveAuthorityRequest {
                request_id: "authreq_correspondence_microdose_execute".to_string(),
                steward: Some("test".to_string()),
                note: Some("one-shot direct address".to_string()),
                ttl_secs: Some(60),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(approval["record_type"], "steward_approval");

        let (tx, mut rx) = mpsc::channel(1);
        let result = execute_semantic_microdose_from_paths(
            "authreq_correspondence_microdose_execute",
            Some(67.0),
            Some(66.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(result["record_type"], "execution_result");
        assert_eq!(result["reason"], "semantic_microdose_sent");
        assert_eq!(
            result["authority_lifecycle_receipt_v2"]["kind"],
            "execution"
        );
        assert_eq!(result["post_change_response_status"], "awaiting");
        assert_eq!(
            result["correspondence_microdose_v1"]["message_id"],
            "corr_astrid_minime_test"
        );
        assert!(matches!(
            rx.try_recv().unwrap(),
            SensoryMsg::Semantic { .. }
        ));
        let rows = read_jsonl(&gate);
        let consequence = rows
            .iter()
            .rev()
            .find(|row| {
                row.get("record_schema").and_then(Value::as_str) == Some("authority_consequence_v1")
            })
            .expect("consequence row");
        assert_eq!(
            consequence["correspondence_microdose_v1"]["correspondence_thread_id"],
            "thread_trace"
        );
    }

    #[test]
    fn mode_release_microdose_sends_one_direct_leak_override_and_records_consequence() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_mode_release_request(&astrid, "authreq_mode_release", 0.71);

        let approval = approve_from_paths(
            ApproveAuthorityRequest {
                request_id: "authreq_mode_release".to_string(),
                steward: Some("test".to_string()),
                note: Some("one-shot direct leak only".to_string()),
                ttl_secs: Some(60),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(approval["scope"], MODE_RELEASE_SCOPE);

        let (tx, mut rx) = mpsc::channel(1);
        let result = execute_semantic_microdose_from_paths(
            "authreq_mode_release",
            Some(67.0),
            Some(66.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(result["record_type"], "execution_result");
        assert_eq!(result["reason"], "mode_release_microdose_sent");

        match rx.try_recv().unwrap() {
            SensoryMsg::Control {
                esn_leak_override,
                esn_leak_override_ticks,
                esn_leak_authority_request_id,
                pi_integrator_leak,
                ..
            } => {
                assert_eq!(esn_leak_override, Some(0.71));
                assert_eq!(esn_leak_override_ticks, Some(3));
                assert_eq!(
                    esn_leak_authority_request_id.as_deref(),
                    Some("authreq_mode_release")
                );
                assert!(pi_integrator_leak.is_none());
            },
            _ => panic!("wrong variant"),
        }
        let gate_text = fs::read_to_string(gate).unwrap();
        assert!(gate_text.contains("\"record_schema\":\"authority_consequence_v1\""));
        assert!(gate_text.contains("\"record_schema\":\"mode_release_consequence_v1\""));
    }

    #[test]
    fn disabled_scope_cannot_be_approved() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        write_request(&astrid, "authreq_control", "control_envelope", true);

        let blocked = approve_from_paths(
            ApproveAuthorityRequest {
                request_id: "authreq_control".to_string(),
                steward: None,
                note: None,
                ttl_secs: None,
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(blocked["record_type"], "blocked");
        assert_eq!(blocked["reason"], "disabled_scope_v1");
    }

    #[test]
    fn budget_approval_caps_sends_and_ttl() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_budget_request(&astrid, "authbud_approve", true);

        let approval = approve_budget_from_paths(
            ApproveAuthorityBudgetRequest {
                budget_id: "authbud_approve".to_string(),
                steward: Some("test".to_string()),
                note: Some("budget ok".to_string()),
                max_sends: Some(99),
                ttl_secs: Some(999_999),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(approval["record_schema"], BUDGET_POLICY);
        assert_eq!(approval["record_type"], "budget_approval");
        assert_eq!(approval["max_sends"], DEFAULT_BUDGET_MAX_SENDS);
        assert_eq!(approval["ttl_secs"], DEFAULT_BUDGET_TTL_SECS);
        let rows = read_jsonl(&gate);
        assert_eq!(
            budget_status_from_rows(&rows)["stage"],
            "active_budget_available"
        );
    }

    #[test]
    fn research_budget_approval_caps_actions_and_ttl() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_research_budget_request(&astrid, "resbud_approve", true);

        let approval = approve_research_budget_from_paths(
            ApproveResearchBudgetRequest {
                budget_id: "resbud_approve".to_string(),
                steward: Some("test".to_string()),
                note: Some("research ok".to_string()),
                max_actions: Some(99),
                ttl_secs: Some(999_999),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(approval["record_schema"], RESEARCH_BUDGET_POLICY);
        assert_eq!(approval["record_type"], "research_budget_approval");
        assert_eq!(approval["max_actions"], MAX_RESEARCH_ACTIONS);
        assert_eq!(approval["ttl_secs"], DEFAULT_RESEARCH_TTL_SECS);
        let rows = read_jsonl(&gate);
        assert_eq!(
            research_budget_status_from_rows(&rows)["stage"],
            "active_budget_available"
        );
        assert!(
            research_budget_status()
                .unwrap()
                .to_string()
                .contains("research_budget_v1")
        );
    }

    #[test]
    fn research_budget_regrant_same_budget_id_supersedes_and_extends_ttl() {
        // Operator refresh/extend (2026-06-15): re-granting the SAME budget_id while one is
        // active must NOT block — it supersedes (newest-wins on both active-checks), so a short
        // TTL can be extended to 7d without a close record (which would poison the budget_id).
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_research_budget_request(&astrid, "resbud_extend", true);

        let first = approve_research_budget_from_paths(
            ApproveResearchBudgetRequest {
                budget_id: "resbud_extend".to_string(),
                steward: Some("test".to_string()),
                note: None,
                max_actions: Some(25),
                ttl_secs: Some(3600),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(first["record_type"], "research_budget_approval");
        assert_eq!(first["ttl_secs"], 3600);

        // Re-grant the SAME budget_id with a longer TTL — must supersede, not block.
        let second = approve_research_budget_from_paths(
            ApproveResearchBudgetRequest {
                budget_id: "resbud_extend".to_string(),
                steward: Some("test".to_string()),
                note: Some("extend".to_string()),
                max_actions: Some(25),
                ttl_secs: Some(DEFAULT_RESEARCH_TTL_SECS),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(
            second["record_type"], "research_budget_approval",
            "re-granting the same budget_id must supersede, not block"
        );
        assert_eq!(second["ttl_secs"], DEFAULT_RESEARCH_TTL_SECS);

        // Newest-wins: the active budget for the experiment is the extended (7d) one.
        let rows = read_jsonl(&gate);
        let active = active_research_budget_for_experiment(&rows, "exp_astrid_test")
            .expect("an active budget after supersede");
        assert_eq!(active["ttl_secs"], DEFAULT_RESEARCH_TTL_SECS);
    }

    #[test]
    fn research_budget_approval_blocks_when_safety_not_green_or_yellow() {
        // The fail-safe behind the headless --approve-research-budget CLI: even an eligible,
        // read-only request must NOT be granted when current fill safety is not green/yellow.
        // (The CLI computes safety from the CURRENT fill it reads at grant time.)
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_research_budget_request(&astrid, "resbud_unsafe", true);

        let blocked = approve_research_budget_from_paths(
            ApproveResearchBudgetRequest {
                budget_id: "resbud_unsafe".to_string(),
                steward: Some("test".to_string()),
                note: None,
                max_actions: None,
                ttl_secs: None,
            },
            SafetyLevel::Red,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(blocked["record_type"], "research_budget_blocked");
        assert_eq!(blocked["reason"], "safety_not_green_or_yellow");
        // and no active budget materialized
        let rows = read_jsonl(&gate);
        assert_ne!(
            research_budget_status_from_rows(&rows)["stage"],
            "active_budget_available"
        );
    }

    #[test]
    fn loop_consequence_budget_approval_requires_ready_row_and_does_not_execute() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_loop_ready(&astrid, "loop_ready", true, EXECUTABLE_SCOPE);

        let approval = approve_loop_consequence_budget_from_paths(
            ApproveLoopConsequenceBudgetRequest {
                loop_id: "loop_ready".to_string(),
                steward: Some("test".to_string()),
                note: Some("one consequence slot only".to_string()),
                ttl_secs: Some(999_999),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(approval["record_schema"], LOOP_POLICY);
        assert_eq!(approval["record_type"], "loop_approval");
        assert_eq!(approval["consequence_remaining"], 1);
        assert_eq!(approval["ttl_secs"], DEFAULT_TOKEN_TTL_SECS);

        let rows = read_jsonl(&gate);
        let status = loop_status_from_rows(&rows);
        assert_eq!(status["stage"], "consequence_slot_approved");
        assert_eq!(status["review_required"].as_bool(), Some(false));
        let gate_text = fs::read_to_string(&gate).unwrap();
        assert!(!gate_text.contains("\"record_type\":\"execution_result\""));
        assert!(!gate_text.contains("\"record_schema\":\"authority_consequence_v1\""));

        append_jsonl(
            &gate,
            &json!({
                "schema_version": 1,
                "record_schema": "authority_consequence_v1",
                "record_type": "execution_result",
                "record_id": "consequence_test",
                "request_id": "authreq_loop_ready",
                "loop_id": "loop_ready",
                "experiment_id": "exp_astrid_test",
                "status": "blocked",
                "peer_mutation": false
            }),
        )
        .unwrap();
        let rows = read_jsonl(&gate);
        let status = loop_status_from_rows(&rows);
        assert_eq!(status["stage"], "review_required");
        assert_eq!(status["review_required"].as_bool(), Some(true));
    }

    #[test]
    fn loop_consequence_budget_blocks_without_ready_row() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_loop_ready(&astrid, "loop_not_ready", false, EXECUTABLE_SCOPE);

        let blocked = approve_loop_consequence_budget_from_paths(
            ApproveLoopConsequenceBudgetRequest {
                loop_id: "loop_not_ready".to_string(),
                steward: Some("test".to_string()),
                note: None,
                ttl_secs: Some(60),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(blocked["record_schema"], LOOP_POLICY);
        assert_eq!(blocked["record_type"], "loop_blocked");
        assert_eq!(blocked["reason"], "loop_consequence_not_ready");
        assert!(
            !fs::read_to_string(gate)
                .unwrap()
                .contains("\"record_type\":\"loop_approval\"")
        );
    }

    #[test]
    fn budget_execution_consumes_one_slot_and_requires_review() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        let gate = write_budget_request(&astrid, "authbud_execute", true);
        approve_budget_from_paths(
            ApproveAuthorityBudgetRequest {
                budget_id: "authbud_execute".to_string(),
                steward: Some("test".to_string()),
                note: None,
                max_sends: Some(3),
                ttl_secs: Some(60),
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        append_jsonl(
            &gate,
            &json!({
                "schema_version": 1,
                "record_schema": POLICY,
                "record_type": "request",
                "record_id": "auth_test_budget_request",
                "request_id": "authreq_budget_execute",
                "being": "astrid",
                "thread_id": "th_test",
                "experiment_id": "exp_astrid_test",
                "scope": EXECUTABLE_SCOPE,
                "payload": "quiet budget witness",
                "artifact_refs": ["/tmp/artifact.json"],
                "source_refs": ["/tmp/artifact.json"],
                "status": "pending_budget_execution",
                "budget_id": "authbud_execute",
                "eligibility_v1": {
                    "eligible": true,
                    "missing_requirements": [],
                    "disabled_scope": false
                },
                "peer_mutation": false,
                "authority_boundary": authority_boundary()
            }),
        )
        .unwrap();
        let (tx, mut rx) = mpsc::channel(4);

        let executed = execute_semantic_microdose_from_paths(
            "authreq_budget_execute",
            Some(68.0),
            Some(67.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(executed["record_type"], "execution_result", "{executed:?}");
        assert_eq!(executed["budget_id"], "authbud_execute");
        assert!(matches!(
            rx.try_recv().unwrap(),
            SensoryMsg::Semantic { .. }
        ));
        let rows = read_jsonl(&gate);
        assert!(rows.iter().any(|row| row["record_type"] == "budget_debit"));
        assert_eq!(budget_status_from_rows(&rows)["stage"], "review_required");

        append_jsonl(
            &gate,
            &json!({
                "schema_version": 1,
                "record_schema": POLICY,
                "record_type": "request",
                "record_id": "auth_test_budget_request_2",
                "request_id": "authreq_budget_second",
                "being": "astrid",
                "thread_id": "th_test",
                "experiment_id": "exp_astrid_test",
                "scope": EXECUTABLE_SCOPE,
                "payload": "second witness",
                "artifact_refs": ["/tmp/artifact.json"],
                "source_refs": ["/tmp/artifact.json"],
                "status": "pending_budget_execution",
                "budget_id": "authbud_execute",
                "eligibility_v1": {"eligible": true, "missing_requirements": [], "disabled_scope": false},
                "peer_mutation": false,
                "authority_boundary": authority_boundary()
            }),
        )
        .unwrap();
        let blocked = execute_semantic_microdose_from_paths(
            "authreq_budget_second",
            Some(68.0),
            Some(68.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(blocked["record_schema"], BUDGET_POLICY);
        assert_eq!(blocked["reason"], "authority_consequence_review_required");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn execution_sends_one_semantic_message_and_consumes_token() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        write_request(&astrid, "authreq_execute", EXECUTABLE_SCOPE, true);
        let _approval = approve_from_paths(
            ApproveAuthorityRequest {
                request_id: "authreq_execute".to_string(),
                steward: None,
                note: None,
                ttl_secs: None,
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        let (tx, mut rx) = mpsc::channel(4);

        let executed = execute_semantic_microdose_from_paths(
            "authreq_execute",
            Some(68.0),
            Some(68.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(executed["record_type"], "execution_result", "{executed:?}");
        let gate_text =
            fs::read_to_string(astrid.join("action_threads/threads/th_test/authority_gate.jsonl"))
                .unwrap();
        assert!(gate_text.contains("\"record_schema\":\"authority_consequence_v1\""));
        assert!(matches!(
            rx.try_recv().unwrap(),
            SensoryMsg::Semantic { .. }
        ));

        let blocked = execute_semantic_microdose_from_paths(
            "authreq_execute",
            Some(68.0),
            Some(68.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(blocked["record_type"], "blocked");
        assert_eq!(blocked["reason"], "token_already_consumed");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn red_safety_blocks_execution_without_send() {
        let temp = tempfile::tempdir().unwrap();
        let minime = temp.path().join("minime_workspace");
        let astrid = temp.path().join("astrid_workspace");
        write_request(&astrid, "authreq_red", EXECUTABLE_SCOPE, true);
        let _approval = approve_from_paths(
            ApproveAuthorityRequest {
                request_id: "authreq_red".to_string(),
                steward: None,
                note: None,
                ttl_secs: None,
            },
            SafetyLevel::Green,
            &minime,
            &astrid,
        )
        .unwrap();
        let (tx, mut rx) = mpsc::channel(4);

        let blocked = execute_semantic_microdose_from_paths(
            "authreq_red",
            Some(95.0),
            Some(90.0),
            &tx,
            &minime,
            &astrid,
        )
        .unwrap();
        assert_eq!(blocked["record_type"], "blocked");
        assert_eq!(blocked["reason"], "safety_not_green_or_yellow");
        assert!(rx.try_recv().is_err());
    }
}
