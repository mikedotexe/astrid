//! Canonical operating-stack metadata for continuity surfaces.

use serde_json::{Value, json};

pub const LOCAL_RESEARCH_MAX_ACTIONS: u64 = 5;
pub const LOCAL_RESEARCH_TTL_SECS: u64 = 21_600;
pub const LOOP_RESEARCH_MAX_ACTIONS: u64 = 5;
pub const LOOP_TTL_SECS: u64 = 21_600;
pub const LOOP_CONSEQUENCE_MAX_SENDS: u64 = 1;
pub const AUTHORITY_BUDGET_MAX_SENDS: u64 = 3;
pub const STEWARD_RESEARCH_MAX_ACTIONS: u64 = 8;
pub const READ_ONLY_RESEARCH_SCOPE: &str = "read_only_research";
pub const SEMANTIC_MICRODOSE_SCOPE: &str = "semantic_microdose";

pub fn local_research_budget_request_scaffold(
    selector: &str,
    purpose: &str,
    allowed_sources: &str,
    stop_criteria: &str,
) -> String {
    format!(
        "EXPERIMENT_RESEARCH_BUDGET_REQUEST {selector} :: scope: {READ_ONLY_RESEARCH_SCOPE}; purpose: {purpose}; max_actions: {LOCAL_RESEARCH_MAX_ACTIONS}; ttl_secs: {LOCAL_RESEARCH_TTL_SECS}; allowed_sources: {allowed_sources}; stop_criteria: {stop_criteria}"
    )
}

pub fn default_local_research_budget_request_scaffold(selector: &str) -> String {
    local_research_budget_request_scaffold(selector, "...", "local", "...")
}

pub fn research_budget_accept_guidance() -> String {
    format!(
        "No research-budget scaffold is available to accept. Wait for a guarded read-only research action, or author {}",
        default_local_research_budget_request_scaffold("<id>")
    )
}

pub fn owned_loop_request_scaffold(
    selector: &str,
    purpose: &str,
    consequence_scope: &str,
    stop_criteria: &str,
) -> String {
    format!(
        "EXPERIMENT_LOOP_REQUEST {selector} :: purpose: {purpose}; consequence_scope: {consequence_scope}; max_research_actions: {LOOP_RESEARCH_MAX_ACTIONS}; ttl_secs: {LOOP_TTL_SECS}; stop_criteria: {stop_criteria}"
    )
}

pub fn default_owned_loop_request_scaffold(selector: &str) -> String {
    owned_loop_request_scaffold(selector, "...", SEMANTIC_MICRODOSE_SCOPE, "...")
}

pub fn authority_budget_request_scaffold(
    selector: &str,
    purpose: &str,
    artifact_refs: &str,
    stop_criteria: &str,
) -> String {
    format!(
        "EXPERIMENT_AUTHORITY_BUDGET_REQUEST {selector} :: scope: {SEMANTIC_MICRODOSE_SCOPE}; purpose: {purpose}; max_sends: {AUTHORITY_BUDGET_MAX_SENDS}; ttl_secs: {LOCAL_RESEARCH_TTL_SECS}; artifact_refs: {artifact_refs}; stop_criteria: {stop_criteria}"
    )
}

pub fn default_authority_budget_request_scaffold(selector: &str) -> String {
    authority_budget_request_scaffold(selector, "...", "...", "...")
}

pub fn cap_label_text() -> String {
    format!(
        "local_research={LOCAL_RESEARCH_MAX_ACTIONS}/{LOCAL_RESEARCH_TTL_SECS}s; loop_research={LOOP_RESEARCH_MAX_ACTIONS}/{LOOP_TTL_SECS}s; consequence={LOOP_CONSEQUENCE_MAX_SENDS} gated slot"
    )
}

pub fn caps_v1() -> Value {
    json!({
        "local_research": {
            "scope": READ_ONLY_RESEARCH_SCOPE,
            "self_activated_max_actions": LOCAL_RESEARCH_MAX_ACTIONS,
            "self_activated_ttl_secs": LOCAL_RESEARCH_TTL_SECS,
            "steward_max_actions": STEWARD_RESEARCH_MAX_ACTIONS,
        },
        "owned_loop": {
            "max_research_actions": LOOP_RESEARCH_MAX_ACTIONS,
            "ttl_secs": LOOP_TTL_SECS,
            "max_consequence_sends": LOOP_CONSEQUENCE_MAX_SENDS,
        },
        "authority_budget": {
            "scope": SEMANTIC_MICRODOSE_SCOPE,
            "max_sends": AUTHORITY_BUDGET_MAX_SENDS,
            "ttl_secs": LOCAL_RESEARCH_TTL_SECS,
        },
    })
}

pub fn command_palette_v1() -> Value {
    json!([
        {
            "group": "Lifecycle",
            "mutability": "local_lifecycle_metadata",
            "commands": ["EXPERIMENT_ADVANCE", "EXPERIMENT_CHARTER", "EXPERIMENT_REHEARSE", "EXPERIMENT_EVIDENCE", "EXPERIMENT_DECIDE", "EXPERIMENT_STATUS"],
            "example": "EXPERIMENT_ADVANCE current :: mode: preview",
            "authority_boundary": "Local lifecycle metadata only; no bind/resume/perturb/control is implied."
        },
        {
            "group": "Owned Loop",
            "mutability": "local_loop_metadata",
            "commands": ["EXPERIMENT_LOOP_REQUEST", "EXPERIMENT_LOOP_STATUS", "EXPERIMENT_LOOP_STEP", "EXPERIMENT_LOOP_REVIEW"],
            "example": "EXPERIMENT_LOOP_STATUS latest",
            "authority_boundary": "Loops orchestrate continuity, local research, sticky audit, and one gated consequence slot."
        },
        {
            "group": "Local Research",
            "mutability": "self_activated_read_only_budget",
            "commands": ["EXPERIMENT_RESEARCH_BUDGET_ACCEPT", "EXPERIMENT_RESEARCH_BUDGET_REQUEST", "EXPERIMENT_RESEARCH_BUDGET_STATUS", "EXPERIMENT_RESEARCH_REVIEW"],
            "example": default_local_research_budget_request_scaffold("current"),
            "generic_accept": "ACCEPT_SUGGESTED_NEXT latest",
            "authority_boundary": "Being-owned local-only read-only research; web/larger/mutating budgets still need steward approval."
        },
        {
            "group": "Continuity Session",
            "mutability": "local_memory_draft_or_session",
            "commands": ["CONTINUITY_SESSION_ACCEPT", "CONTINUITY_SESSION_START", "CONTINUITY_SESSION_CAPTURE", "CONTINUITY_SESSION_SUMMARIZE", "CONTINUITY_SESSION_FINALIZE", "CONTINUITY_SESSION_RESUME", "CONTINUITY_SESSION_STATUS"],
            "example": "CONTINUITY_SESSION_CAPTURE latest :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...",
            "generic_accept": "ACCEPT_SUGGESTED_NEXT latest",
            "authority_boundary": "Captures thought continuity; does not spend research or change authority."
        },
        {
            "group": "Memory/Dossier",
            "mutability": "local_cite_backed_memory",
            "commands": ["MEMORY_STATUS", "MEMORY_RECALL", "MEMORY_CAPTURE", "MEMORY_PROMOTE", "DOSSIER_CLAIM", "DOSSIER_EVIDENCE", "DOSSIER_STATUS", "DOSSIER_REVIEW"],
            "example": "MEMORY_RECALL latest :: focus: ...",
            "authority_boundary": "Cite-backed memory and research claims; no lifecycle acceptance or peer authority by itself."
        },
        {
            "group": "Authority Readiness",
            "mutability": "request_or_steward_gated_consequence",
            "commands": ["EXPERIMENT_AUTHORITY_PREPARE", "EXPERIMENT_AUTHORITY_REQUEST", "EXPERIMENT_AUTHORITY_STATUS", "EXPERIMENT_AUTHORITY_BUDGET_REQUEST", "EXPERIMENT_AUTHORITY_BUDGET_STATUS", "EXPERIMENT_AUTHORITY_REVIEW", "EXPERIMENT_AUTHORITY_EXECUTE"],
            "example": "EXPERIMENT_AUTHORITY_STATUS current",
            "authority_boundary": "Execution is explicit and steward/bridge-gated; projection never executes authority."
        },
        {
            "group": "Sticky/Telemetry",
            "mutability": "read_only_diagnostic",
            "commands": ["STICKY_MODE_AUDIT", "EXPERIMENT_ADVANCE"],
            "example": "STICKY_MODE_AUDIT",
            "authority_boundary": "Audit/readiness only; mode release remains separately gated."
        }
    ])
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn route(group: &str, command: String, reason: &str, priority: u64, source: &str) -> Value {
    json!({
        "group": group,
        "command": command,
        "reason": reason,
        "priority": priority,
        "source": source,
    })
}

pub fn build_control_plane_v1(input: &Value) -> Value {
    let mut routes = Vec::<Value>::new();
    let lifecycle_stage = text(input.get("lifecycle_stage"));
    let lifecycle_next = text(input.get("lifecycle_next"));
    if !lifecycle_next.is_empty()
        && matches!(
            lifecycle_stage.as_str(),
            "needs_charter" | "blocked_loop" | "paused" | "hold"
        )
    {
        routes.push(route(
            "Lifecycle",
            lifecycle_next,
            &format!("safety lifecycle stage: {lifecycle_stage}"),
            5,
            "lifecycle",
        ));
    } else if !lifecycle_next.is_empty() {
        routes.push(route(
            "Lifecycle",
            lifecycle_next,
            "current lifecycle return",
            30,
            "lifecycle",
        ));
    }

    if let Some(loop_status) = input
        .get("sovereign_loop_v1")
        .filter(|value| value.is_object())
    {
        let stage = text(loop_status.get("stage"));
        let command = text(loop_status.get("next_safe_command"));
        if !command.is_empty() && !matches!(stage.as_str(), "" | "no_loop") {
            let priority = if matches!(stage.as_str(), "review_required" | "consequence_ready") {
                8
            } else {
                18
            };
            routes.push(route(
                "Owned Loop",
                command,
                &format!("owned loop stage: {stage}"),
                priority,
                "sovereign_loop_v1",
            ));
        }
    }

    if let Some(research) = input
        .get("research_budget_priority_route_v1")
        .filter(|value| value.is_object())
    {
        let command = text(research.get("next"));
        let stage = text(research.get("stage"));
        if !command.is_empty() {
            routes.push(route(
                "Local Research",
                command,
                &format!("research budget stage: {stage}"),
                12,
                "research_budget_priority_route_v1",
            ));
        }
    }

    if let Some(session) = input
        .get("continuity_session_v1")
        .filter(|value| value.is_object())
    {
        let command = text(session.get("suggested_next"));
        routes.push(route(
            "Continuity Session",
            if command.is_empty() {
                "CONTINUITY_SESSION_STATUS latest".to_string()
            } else {
                command
            },
            "latest continuity session",
            20,
            "continuity_session_v1",
        ));
    }

    if input.get("interpretation_risk_v1").is_some()
        || input.get("constraint_release_trajectory_v1").is_some()
    {
        routes.push(route(
            "Continuity Session",
            "CONTINUITY_SESSION_CAPTURE latest".to_string(),
            "interpretation or release cue needs capture",
            10,
            "self_study_cue",
        ));
    }

    routes.sort_by(|left, right| {
        let left_priority = left
            .get("priority")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        let right_priority = right
            .get("priority")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        left_priority.cmp(&right_priority)
    });
    routes.dedup_by(|left, right| {
        left.get("group") == right.get("group") && left.get("command") == right.get("command")
    });
    routes.truncate(7);
    let primary = routes.first().cloned().unwrap_or_else(|| {
        route(
            "Lifecycle",
            "THREAD_STATUS current".to_string(),
            "no higher-priority route",
            99,
            "fallback",
        )
    });
    let mut boundaries = serde_json::Map::new();
    if let Some(groups) = command_palette_v1().as_array() {
        for group in groups {
            if let (Some(name), Some(boundary)) = (
                group.get("group").and_then(Value::as_str),
                group.get("authority_boundary").and_then(Value::as_str),
            ) {
                boundaries.insert(name.to_string(), json!(boundary));
            }
        }
    }
    json!({
        "record_schema": "continuity_control_plane_v1",
        "schema_version": 1,
        "policy": "continuity_control_plane_v1",
        "primary_route": primary,
        "route_stack": routes,
        "command_palette": command_palette_v1(),
        "caps_v1": caps_v1(),
        "authority_boundaries": boundaries,
        "source_refs": input.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        "projection_freshness_v1": input.get("projection_freshness_v1").cloned().unwrap_or(Value::Null),
        "authority_change": false,
        "peer_mutation": false,
    })
}

pub fn command_palette_text() -> String {
    let groups = command_palette_v1();
    let Some(groups) = groups.as_array() else {
        return String::new();
    };
    let rendered = groups
        .iter()
        .filter_map(|group| {
            let name = group.get("group").and_then(Value::as_str)?;
            let commands = group
                .get("commands")
                .and_then(Value::as_array)?
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!("{name}: {commands}"))
        })
        .collect::<Vec<_>>()
        .join(" | ");
    format!("Command palette (generated): {rendered}")
}

pub fn control_plane_text(control: &Value) -> String {
    let Some(primary) = control
        .get("primary_route")
        .filter(|value| value.is_object())
    else {
        return String::new();
    };
    let primary_command = text(primary.get("command"));
    let stack = control
        .get("route_stack")
        .and_then(Value::as_array)
        .map(|routes| {
            routes
                .iter()
                .take(4)
                .filter_map(|route| {
                    let group = route.get("group").and_then(Value::as_str)?;
                    let command = route.get("command").and_then(Value::as_str)?;
                    Some(format!("{group}: {command}"))
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "Lifecycle: THREAD_STATUS current".to_string());
    format!(
        "continuity_control_plane_v1: primary={}\nOperating stack: {}\nCaps: {}\n",
        if primary_command.is_empty() {
            "THREAD_STATUS current"
        } else {
            primary_command.as_str()
        },
        stack,
        cap_label_text(),
    )
}
