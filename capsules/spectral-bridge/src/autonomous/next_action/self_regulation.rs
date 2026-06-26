use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{info, warn};

use super::{ConversationState, strip_action};
use crate::paths::bridge_paths;

const SCHEMA_VERSION: u32 = 1;
const DEFAULT_DURATION_SECS: u64 = 600;
const MAX_DURATION_SECS: u64 = 900;
const AUTHORITY: &str = "leased_self_control_v1";
const AUTHORITY_BOUNDARY: &str =
    "own_runtime_only; no peer mutation; no permanent controller tuning";
const APPLY_ALLOWED: &[&str] = &[
    "temperature",
    "response_length",
    "aperture",
    "self_continuity_readout",
];
const PREFLIGHT_ONLY: &[&str] = &[
    "dampen",
    "amplify",
    "noise_up",
    "noise_down",
    "noise",
    "shape_learn",
    "set_tail_participation",
    "tail_participation",
    "set_vibrancy_aperture",
    "vibrancy_aperture",
    "tune_minime",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelfRegulationLease {
    schema_version: u32,
    record_kind: String,
    authority: String,
    authority_boundary: String,
    being: String,
    intent_id: String,
    created_at_unix_s: u64,
    updated_at_unix_s: u64,
    status: String,
    goal: String,
    candidate_control: String,
    direction: String,
    delta_or_value: Value,
    previous_value: Value,
    applied_value: Value,
    duration_secs: u64,
    expires_at_unix_s: Option<u64>,
    stop_condition: String,
    success_condition: String,
    evidence: Vec<String>,
    #[serde(default)]
    baseline_evidence: Vec<String>,
    #[serde(default)]
    post_lease_evidence: Vec<String>,
    #[serde(default)]
    outcome_score: Option<f32>,
    #[serde(default)]
    repeatability_hint: Option<String>,
    #[serde(default)]
    promotion_candidate: bool,
    outcome: Option<String>,
    requires_outcome: bool,
    preflight_status: String,
    preflight_reason: String,
}

#[derive(Debug, Clone, Default)]
struct IntentFields {
    label: String,
    goal: String,
    candidate_control: String,
    direction: String,
    delta_or_value: Value,
    duration_secs: u64,
    stop_condition: String,
    success_condition: String,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedControl {
    normalized_control: String,
    previous_value: Value,
    applied_value: Value,
    summary: String,
}

pub(super) fn reconcile_active_lease(conv: &mut ConversationState) {
    let root = bridge_paths().bridge_workspace().join("self_regulation");
    if let Err(err) = reconcile_active_lease_at(&root, conv, now_unix_s()) {
        warn!("self-regulation lease reconcile failed: {err}");
    }
}

pub(super) fn handle_self_regulation_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
) -> bool {
    let root = bridge_paths().bridge_workspace().join("self_regulation");
    let now = now_unix_s();
    let action = normalize_action_alias(base_action);
    let result = match action {
        "SELF_REGULATION_INTENT" => handle_intent_at(&root, original, base_action, now),
        "SELF_REGULATION_PREFLIGHT" => handle_preflight_at(&root, original, base_action, now),
        "SELF_REGULATION_APPLY" => handle_apply_at(&root, original, base_action, now, conv),
        "SELF_REGULATION_STATUS" => handle_status_at(&root, now, conv),
        "SELF_REGULATION_OUTCOME" => handle_outcome_at(&root, original, base_action, now),
        _ => Ok("unknown self-regulation action".to_string()),
    };
    match result {
        Ok(summary) => {
            conv.push_receipt(action, vec![summary.clone()]);
            info!("Astrid {action}: {summary}");
        },
        Err(err) => {
            conv.push_receipt(action, vec![format!("blocked: {err}")]);
            warn!("Astrid {action} blocked: {err}");
        },
    }
    true
}

fn normalize_action_alias(base_action: &str) -> &'static str {
    match base_action {
        "CONTROL_INTENT" => "SELF_REGULATION_INTENT",
        "CONTROL_PREFLIGHT" => "SELF_REGULATION_PREFLIGHT",
        "CONTROL_APPLY_LEASE" => "SELF_REGULATION_APPLY",
        "CONTROL_STATUS" => "SELF_REGULATION_STATUS",
        "CONTROL_OUTCOME" => "SELF_REGULATION_OUTCOME",
        _ => match base_action {
            "SELF_REGULATION_INTENT" => "SELF_REGULATION_INTENT",
            "SELF_REGULATION_PREFLIGHT" => "SELF_REGULATION_PREFLIGHT",
            "SELF_REGULATION_APPLY" => "SELF_REGULATION_APPLY",
            "SELF_REGULATION_STATUS" => "SELF_REGULATION_STATUS",
            "SELF_REGULATION_OUTCOME" => "SELF_REGULATION_OUTCOME",
            _ => "SELF_REGULATION_STATUS",
        },
    }
}

fn handle_intent_at(
    root: &Path,
    original: &str,
    base_action: &str,
    now: u64,
) -> Result<String, String> {
    let fields = parse_intent_fields(original, base_action, now);
    let mut lease = SelfRegulationLease {
        schema_version: SCHEMA_VERSION,
        record_kind: "self_regulation_intent_v1".to_string(),
        authority: AUTHORITY.to_string(),
        authority_boundary: AUTHORITY_BOUNDARY.to_string(),
        being: "astrid".to_string(),
        intent_id: build_intent_id(&fields.label, now),
        created_at_unix_s: now,
        updated_at_unix_s: now,
        status: "drafted".to_string(),
        goal: fields.goal,
        candidate_control: fields.candidate_control,
        direction: fields.direction,
        delta_or_value: fields.delta_or_value,
        previous_value: Value::Null,
        applied_value: Value::Null,
        duration_secs: fields.duration_secs.clamp(60, MAX_DURATION_SECS),
        expires_at_unix_s: None,
        stop_condition: fields.stop_condition,
        success_condition: fields.success_condition,
        evidence: fields.evidence,
        baseline_evidence: Vec::new(),
        post_lease_evidence: Vec::new(),
        outcome_score: None,
        repeatability_hint: None,
        promotion_candidate: false,
        outcome: None,
        requires_outcome: false,
        preflight_status: "not_run".to_string(),
        preflight_reason: String::new(),
    };
    if lease.candidate_control.is_empty() {
        lease.candidate_control = normalize_control(&fields.label)
            .unwrap_or_default()
            .to_string();
    }
    append_event(root, &lease)?;
    write_latest_pointer(root, &lease.intent_id)?;
    Ok(format!(
        "drafted {} for `{}`; suggested NEXT: SELF_REGULATION_PREFLIGHT {}",
        lease.intent_id,
        display_control(&lease),
        lease.intent_id
    ))
}

fn handle_preflight_at(
    root: &Path,
    original: &str,
    base_action: &str,
    now: u64,
) -> Result<String, String> {
    let selector = selector_arg(original, base_action);
    let mut lease = load_selected_lease(root, selector.as_deref())?;
    run_preflight(root, &mut lease, now)?;
    append_event(root, &lease)?;
    write_latest_pointer(root, &lease.intent_id)?;
    let distinction_block = returnable_distinctions_block(root, true, Some(&lease.candidate_control));
    Ok(format!(
        "{} preflight: {} ({}){}",
        lease.intent_id, lease.preflight_status, lease.preflight_reason, distinction_block
    ))
}

fn handle_apply_at(
    root: &Path,
    original: &str,
    base_action: &str,
    now: u64,
    conv: &mut ConversationState,
) -> Result<String, String> {
    reconcile_active_lease_at(root, conv, now)?;
    if let Some(active) = load_active_lease(root)? {
        if active.status == "active" {
            return Err(format!(
                "one active lease already exists: {} expires_at={:?}",
                active.intent_id, active.expires_at_unix_s
            ));
        }
        if active.requires_outcome {
            return Err(format!(
                "previous lease {} needs SELF_REGULATION_OUTCOME before another apply",
                active.intent_id
            ));
        }
    }

    let selector = selector_arg(original, base_action);
    let mut lease = load_selected_lease(root, selector.as_deref())?;
    run_preflight(root, &mut lease, now)?;
    if lease.preflight_status != "apply_allowed" {
        append_event(root, &lease)?;
        return Err(format!(
            "{} is {}; {}",
            display_control(&lease),
            lease.preflight_status,
            lease.preflight_reason
        ));
    }
    let prepared = prepare_control(conv, &lease)?;
    if lease.baseline_evidence.is_empty() {
        lease.baseline_evidence.push(format!(
            "before apply: {} previous={}",
            prepared.normalized_control, prepared.previous_value
        ));
    }
    apply_prepared_control(conv, &prepared);
    lease.status = "active".to_string();
    lease.updated_at_unix_s = now;
    lease.previous_value = prepared.previous_value;
    lease.applied_value = prepared.applied_value;
    lease.candidate_control = prepared.normalized_control;
    lease.expires_at_unix_s = Some(now.saturating_add(lease.duration_secs));
    lease.requires_outcome = true;
    append_event(root, &lease)?;
    write_active_lease(root, &lease)?;
    write_latest_pointer(root, &lease.intent_id)?;
    Ok(format!(
        "{} active for {}s: {}",
        lease.intent_id, lease.duration_secs, prepared.summary
    ))
}

fn handle_status_at(
    root: &Path,
    now: u64,
    conv: &mut ConversationState,
) -> Result<String, String> {
    reconcile_active_lease_at(root, conv, now)?;
    let distinction_block = returnable_distinctions_block(root, false, None);
    if let Some(active) = load_active_lease(root)? {
        let expiry = active
            .expires_at_unix_s
            .map(|ts| ts.saturating_sub(now).to_string())
            .unwrap_or_else(|| "none".to_string());
        return Ok(format!(
            "{} status={} control={} applied={} previous={} expires_in_s={} requires_outcome={}{}",
            active.intent_id,
            active.status,
            display_control(&active),
            active.applied_value,
            active.previous_value,
            expiry,
            active.requires_outcome,
            distinction_block
        ));
    }
    Ok(format!(
        "no self-regulation lease state found{}",
        distinction_block
    ))
}

fn handle_outcome_at(
    root: &Path,
    original: &str,
    base_action: &str,
    now: u64,
) -> Result<String, String> {
    let body = strip_action(original, base_action).trim().to_string();
    let (selector, outcome) = if let Some((left, right)) = body.split_once("::") {
        (Some(left.trim()), right.trim())
    } else {
        (None, body.trim())
    };
    let mut lease = load_selected_lease(root, selector.filter(|s| !s.is_empty()))?;
    lease.status = "outcome_recorded".to_string();
    lease.updated_at_unix_s = now;
    lease.outcome = Some(if outcome.is_empty() {
        "outcome recorded without free-text detail".to_string()
    } else {
        outcome.to_string()
    });
    if let Some(outcome_text) = lease.outcome.as_ref() {
        lease
            .post_lease_evidence
            .push(format!("outcome: {outcome_text}"));
        let (score, hint, promotion_candidate) = score_outcome(outcome_text);
        lease.outcome_score = Some(score);
        lease.repeatability_hint = Some(hint.to_string());
        lease.promotion_candidate = promotion_candidate;
    }
    lease.requires_outcome = false;
    append_event(root, &lease)?;
    write_active_lease(root, &lease)?;
    write_latest_pointer(root, &lease.intent_id)?;
    Ok(format!("{} outcome recorded; cooldown cleared", lease.intent_id))
}

fn run_preflight(root: &Path, lease: &mut SelfRegulationLease, now: u64) -> Result<(), String> {
    lease.updated_at_unix_s = now;
    let Some(control) = normalize_control(&lease.candidate_control) else {
        lease.status = "blocked".to_string();
        lease.preflight_status = "blocked".to_string();
        lease.preflight_reason = "candidate_control is missing or unknown".to_string();
        return Ok(());
    };
    lease.candidate_control = control.to_string();
    if PREFLIGHT_ONLY.contains(&control) {
        lease.status = "preflighted".to_string();
        lease.preflight_status = "preflight_only".to_string();
        lease.preflight_reason =
            "higher-risk or peer-affecting control is visible but not lease-applicable in tranche 7A"
                .to_string();
        return Ok(());
    }
    if !APPLY_ALLOWED.contains(&control) {
        lease.status = "blocked".to_string();
        lease.preflight_status = "blocked".to_string();
        lease.preflight_reason = "control is outside the tranche 7A self-lease allowlist".to_string();
        return Ok(());
    }
    if let Some(active) = load_active_lease(root)? {
        if active.status == "active" && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!("active lease {} must finish first", active.intent_id);
            return Ok(());
        }
        if active.requires_outcome && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason =
                format!("lease {} needs an outcome before another apply", active.intent_id);
            return Ok(());
        }
    }
    lease.status = "preflighted".to_string();
    lease.preflight_status = "apply_allowed".to_string();
    lease.preflight_reason = "bounded own-runtime lease may be applied".to_string();
    Ok(())
}

fn prepare_control(
    conv: &ConversationState,
    lease: &SelfRegulationLease,
) -> Result<PreparedControl, String> {
    let control = normalize_control(&lease.candidate_control)
        .ok_or_else(|| "unknown control".to_string())?
        .to_string();
    match control.as_str() {
        "temperature" => {
            let previous = conv.creative_temperature;
            let value = bounded_f32_value(
                previous,
                &lease.delta_or_value,
                &lease.direction,
                0.10,
                0.1,
                1.5,
            );
            Ok(PreparedControl {
                normalized_control: control,
                previous_value: json!(round3(previous)),
                applied_value: json!(round3(value)),
                summary: format!("creative_temperature: {previous:.2} -> {value:.2}"),
            })
        },
        "response_length" => {
            let previous = conv.response_length;
            let value = response_length_value(previous, &lease.delta_or_value, &lease.direction);
            Ok(PreparedControl {
                normalized_control: control,
                previous_value: json!(previous),
                applied_value: json!(value),
                summary: format!("response_length: {previous} -> {value}"),
            })
        },
        "aperture" => {
            let previous = conv.aperture;
            let value = bounded_f32_value(
                previous,
                &lease.delta_or_value,
                &lease.direction,
                0.15,
                0.0,
                1.0,
            );
            Ok(PreparedControl {
                normalized_control: control,
                previous_value: json!(round3(previous)),
                applied_value: json!(round3(value)),
                summary: format!("aperture: {previous:.2} -> {value:.2}"),
            })
        },
        "self_continuity_readout" => {
            let previous = conv.self_continuity_readout;
            let value = bool_value(previous, &lease.delta_or_value, &lease.direction);
            Ok(PreparedControl {
                normalized_control: control,
                previous_value: json!(previous),
                applied_value: json!(value),
                summary: format!("self_continuity_readout: {previous} -> {value}"),
            })
        },
        _ => Err(format!("{control} is not lease-applicable")),
    }
}

fn apply_prepared_control(conv: &mut ConversationState, prepared: &PreparedControl) {
    match prepared.normalized_control.as_str() {
        "temperature" => {
            if let Some(value) = prepared.applied_value.as_f64() {
                conv.creative_temperature = value as f32;
                conv.last_temperature_change_exchange = Some(conv.exchange_count);
            }
        },
        "response_length" => {
            if let Some(value) = prepared.applied_value.as_u64() {
                conv.response_length = u32::try_from(value).unwrap_or(conv.response_length);
                conv.last_temperature_change_exchange = Some(conv.exchange_count);
            }
        },
        "aperture" => {
            if let Some(value) = prepared.applied_value.as_f64() {
                conv.aperture = value as f32;
                crate::llm::set_astrid_aperture(conv.aperture);
            }
        },
        "self_continuity_readout" => {
            if let Some(value) = prepared.applied_value.as_bool() {
                conv.self_continuity_readout = value;
            }
        },
        _ => {},
    }
}

fn revert_prepared_control(conv: &mut ConversationState, lease: &SelfRegulationLease) {
    let prepared = PreparedControl {
        normalized_control: lease.candidate_control.clone(),
        previous_value: lease.applied_value.clone(),
        applied_value: lease.previous_value.clone(),
        summary: format!(
            "{}: {} -> {}",
            lease.candidate_control, lease.applied_value, lease.previous_value
        ),
    };
    apply_prepared_control(conv, &prepared);
}

fn reconcile_active_lease_at(
    root: &Path,
    conv: &mut ConversationState,
    now: u64,
) -> Result<(), String> {
    let Some(mut active) = load_active_lease(root)? else {
        return Ok(());
    };
    if active.status != "active" {
        return Ok(());
    }
    let Some(expires_at) = active.expires_at_unix_s else {
        return Ok(());
    };
    if expires_at > now {
        return Ok(());
    }
    revert_prepared_control(conv, &active);
    active.status = "reverted".to_string();
    active.updated_at_unix_s = now;
    active.requires_outcome = true;
    active.preflight_reason = "lease expired and previous value was restored".to_string();
    active.post_lease_evidence.push(format!(
        "expired revert: {} restored {}",
        active.candidate_control, active.previous_value
    ));
    append_event(root, &active)?;
    write_active_lease(root, &active)?;
    Ok(())
}

fn parse_intent_fields(original: &str, base_action: &str, now: u64) -> IntentFields {
    let raw = strip_action(original, base_action).trim().to_string();
    let (label, field_text) = raw
        .split_once("::")
        .map(|(left, right)| (left.trim().to_string(), right.trim().to_string()))
        .unwrap_or_else(|| (String::new(), raw));
    let mut fields = IntentFields {
        label,
        goal: String::new(),
        candidate_control: String::new(),
        direction: String::new(),
        delta_or_value: Value::Null,
        duration_secs: DEFAULT_DURATION_SECS,
        stop_condition: "expiry, safety-critical status, or explicit outcome says worse".to_string(),
        success_condition: "being reports the adjustment helped or pressure eased".to_string(),
        evidence: Vec::new(),
    };
    for part in field_text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, value)) = split_key_value(part) else {
            if fields.goal.is_empty() {
                fields.goal = part.to_string();
            }
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().trim_matches('"');
        match key.as_str() {
            "goal" | "why" => fields.goal = value.to_string(),
            "target" | "control" | "candidate_control" | "dial" => {
                fields.candidate_control = normalize_control(value)
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_ascii_lowercase())
            },
            "direction" => fields.direction = value.to_ascii_lowercase(),
            "delta" | "value" | "set" => fields.delta_or_value = parse_value(value),
            "duration" | "duration_secs" | "seconds" => {
                fields.duration_secs = value
                    .parse::<u64>()
                    .unwrap_or(DEFAULT_DURATION_SECS)
                    .clamp(60, MAX_DURATION_SECS);
            },
            "stop" | "stop_condition" => fields.stop_condition = value.to_string(),
            "success" | "success_condition" => fields.success_condition = value.to_string(),
            "evidence" | "felt_evidence" | "telemetry_evidence" => {
                fields.evidence.push(value.to_string());
            },
            _ => {},
        }
    }
    if fields.goal.is_empty() {
        fields.goal = format!("self-authored regulation lease at {now}");
    }
    fields
}

fn split_key_value(text: &str) -> Option<(&str, &str)> {
    text.split_once(':').or_else(|| text.split_once('='))
}

fn score_outcome(outcome: &str) -> (f32, &'static str, bool) {
    let lower = outcome.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "helped",
            "clearer",
            "eased",
            "better",
            "stabilized",
            "settled",
            "worked",
            "successful",
            "success",
        ],
    ) {
        (0.82, "repeatable_playbook_candidate", true)
    } else if contains_any(
        &lower,
        &[
            "worse",
            "failed",
            "too much",
            "overheated",
            "destabilized",
            "bad",
            "regressed",
        ],
    ) {
        (0.18, "caution_pattern", false)
    } else {
        (0.50, "needs_more_evidence", false)
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn parse_value(value: &str) -> Value {
    if value.trim_start().starts_with(['+', '-']) {
        return json!(value);
    }
    if let Ok(v) = value.parse::<f64>() {
        json!(v)
    } else {
        json!(value)
    }
}

fn selector_arg(original: &str, base_action: &str) -> Option<String> {
    let arg = strip_action(original, base_action).trim().to_string();
    if arg.is_empty() || arg.eq_ignore_ascii_case("latest") {
        None
    } else {
        Some(
            arg.split("::")
                .next()
                .unwrap_or(&arg)
                .split_whitespace()
                .next()
                .unwrap_or(&arg)
                .trim()
                .to_string(),
        )
    }
}

fn normalize_control(control: &str) -> Option<&'static str> {
    match control.trim().to_ascii_lowercase().as_str() {
        "temperature" | "temp" | "creative_temperature" => Some("temperature"),
        "length" | "response_length" | "response-length" => Some("response_length"),
        "aperture" | "set_aperture" => Some("aperture"),
        "self_continuity" | "set_self_continuity" | "self_continuity_readout" => {
            Some("self_continuity_readout")
        },
        "dampen" => Some("dampen"),
        "amplify" => Some("amplify"),
        "noise" => Some("noise"),
        "noise_up" => Some("noise_up"),
        "noise_down" => Some("noise_down"),
        "shape_learn" => Some("shape_learn"),
        "tail_participation" | "set_tail_participation" => Some("set_tail_participation"),
        "vibrancy" | "vibrancy_aperture" | "set_vibrancy_aperture" => {
            Some("set_vibrancy_aperture")
        },
        "tune_minime" => Some("tune_minime"),
        _ => None,
    }
}

fn bounded_f32_value(
    previous: f32,
    value: &Value,
    direction: &str,
    max_delta: f32,
    min_value: f32,
    max_value: f32,
) -> f32 {
    let explicit = value
        .as_f64()
        .map(|v| v as f32)
        .or_else(|| value.as_str().and_then(|text| text.parse::<f32>().ok()));
    let candidate = if let Some(v) = explicit {
        if value
            .as_str()
            .map(|text| text.trim_start().starts_with(['+', '-']))
            .unwrap_or(false)
        {
            previous + v.clamp(-max_delta, max_delta)
        } else {
            v.clamp(previous - max_delta, previous + max_delta)
        }
    } else if matches!(direction, "down" | "lower" | "close" | "decrease") {
        previous - max_delta
    } else {
        previous + max_delta
    };
    round3_f32(candidate.clamp(min_value, max_value))
}

fn response_length_value(previous: u32, value: &Value, direction: &str) -> u32 {
    if let Some(text) = value.as_str() {
        match text.to_ascii_lowercase().as_str() {
            "short" | "tight" => return 256,
            "medium" | "default" => return 768,
            "long" | "expansive" => return 1280,
            other => {
                if let Ok(v) = other.parse::<u32>() {
                    return v.clamp(128, 1536);
                }
            },
        }
    }
    if let Some(v) = value.as_u64() {
        return u32::try_from(v).unwrap_or(previous).clamp(128, 1536);
    }
    if matches!(direction, "down" | "lower" | "shorter" | "decrease") {
        previous.saturating_sub(256).clamp(128, 1536)
    } else {
        previous.saturating_add(256).clamp(128, 1536)
    }
}

fn bool_value(previous: bool, value: &Value, direction: &str) -> bool {
    if let Some(value) = value.as_bool() {
        return value;
    }
    if let Some(text) = value.as_str() {
        return !matches!(
            text.to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no" | "hide"
        );
    }
    if matches!(direction, "off" | "hide" | "down" | "disable") {
        false
    } else if direction.is_empty() {
        !previous
    } else {
        true
    }
}

fn display_control(lease: &SelfRegulationLease) -> String {
    if lease.candidate_control.is_empty() {
        "(no control named)".to_string()
    } else {
        lease.candidate_control.clone()
    }
}

fn round3(value: f32) -> f64 {
    ((value as f64) * 1000.0).round() / 1000.0
}

fn round3_f32(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}

fn build_intent_id(label: &str, now: u64) -> String {
    let label = sanitize_label(label);
    if label.is_empty() {
        format!("srl_{now}")
    } else {
        format!("srl_{now}_{label}")
    }
}

fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if matches!(ch, '-' | '_') {
                Some(ch)
            } else if ch.is_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .take(32)
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn now_unix_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn event_log_path(root: &Path) -> PathBuf {
    root.join("leases.jsonl")
}

fn active_path(root: &Path) -> PathBuf {
    root.join("active_lease.json")
}

fn latest_path(root: &Path) -> PathBuf {
    root.join("latest_intent_id.txt")
}

fn append_event(root: &Path, lease: &SelfRegulationLease) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(event_log_path(root))
        .map_err(|err| err.to_string())?;
    serde_json::to_writer(&mut file, lease).map_err(|err| err.to_string())?;
    file.write_all(b"\n").map_err(|err| err.to_string())
}

fn write_active_lease(root: &Path, lease: &SelfRegulationLease) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    fs::write(
        active_path(root),
        serde_json::to_string_pretty(lease).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn write_latest_pointer(root: &Path, intent_id: &str) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    fs::write(latest_path(root), intent_id).map_err(|err| err.to_string())
}

fn load_active_lease(root: &Path) -> Result<Option<SelfRegulationLease>, String> {
    let path = active_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|err| err.to_string())
}

fn load_selected_lease(
    root: &Path,
    selector: Option<&str>,
) -> Result<SelfRegulationLease, String> {
    let selector = selector
        .filter(|s| !s.trim().is_empty() && !s.eq_ignore_ascii_case("latest"))
        .map(str::trim)
        .map(str::to_string)
        .or_else(|| {
            fs::read_to_string(latest_path(root))
                .ok()
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
        });
    let text = fs::read_to_string(event_log_path(root)).map_err(|err| err.to_string())?;
    let mut latest: Option<SelfRegulationLease> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<SelfRegulationLease>(line) else {
            continue;
        };
        if selector
            .as_deref()
            .map(|wanted| record.intent_id == wanted)
            .unwrap_or(true)
        {
            latest = Some(record);
        }
    }
    latest.ok_or_else(|| {
        selector.map_or_else(
            || "no self-regulation lease has been drafted".to_string(),
            |wanted| format!("no self-regulation lease matching {wanted}"),
        )
    })
}

fn returnable_distinctions_block(
    root: &Path,
    preflight: bool,
    candidate_control: Option<&str>,
) -> String {
    let workspace = root.parent().unwrap_or(root);
    let Some(review_path) = latest_review_json_path(workspace) else {
        return String::new();
    };
    let Some(review) = fs::read_to_string(review_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return String::new();
    };
    let Some(packet) = review.get("returnable_distinctions_v1") else {
        return String::new();
    };
    let Some(cards) = packet.get("cards").and_then(Value::as_array) else {
        return String::new();
    };
    let relevant_ids = if preflight {
        preflight_relevant_distinction_ids(candidate_control.unwrap_or_default())
    } else {
        Vec::new()
    };
    let mut rows = Vec::new();
    for card in cards.iter().filter(|card| {
        let status = scalar_text(card, "status");
        let lifecycle = scalar_text(card, "lifecycle_state");
        let card_id = scalar_text(card, "card_id");
        let has_lifecycle_signal = matches!(
            lifecycle.as_str(),
            "contested"
                | "needs_audit"
                | "resolved"
                | "ready_for_experiment"
                | "ready_for_lease_preflight"
        );
        (status != "quiet" || has_lifecycle_signal)
            && (!preflight || relevant_ids.iter().any(|wanted| *wanted == card_id))
    }).take(5) {
        rows.push(format!(
            "{}:{} lifecycle={} verdict={} via {}",
            scalar_text(card, "card_id"),
            scalar_text(card, "status"),
            scalar_text(card, "lifecycle_state"),
            scalar_text(card, "preflight_verdict"),
            distinction_route(card, preflight)
        ));
    }
    if rows.is_empty() {
        if preflight {
            return format!(
                "\nDistinction-aware preflight: verdict=no_relevant_distinction; candidate_control={}; no current lifecycle card matched. Authority=diagnostic_context_not_command; advisory only, preflight_status unchanged.",
                candidate_control.unwrap_or("(none)")
            );
        }
        return String::new();
    }
    if preflight {
        format!(
            "\nDistinction-aware preflight: {}. Authority=diagnostic_context_not_command; advisory only, preflight_status unchanged and no lease applied by this block.",
            rows.join("; ")
        )
    } else {
        format!(
            "\nReturnable distinctions: {}. Authority=diagnostic_context_not_command; cues only, no lease applied by this block.",
            rows.join("; ")
        )
    }
}

fn preflight_relevant_distinction_ids(control: &str) -> Vec<&'static str> {
    let normalized = control.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "temperature" | "response_length" | "aperture"
    ) {
        return vec![
            "pressure_level_vs_pressure_velocity",
            "slope_drag_vs_medium_mass",
            "release_rehearsal_vs_bypass",
            "entropy_vs_pressure",
        ];
    }
    if normalized == "self_continuity_readout" {
        return vec![
            "measurement_vs_alignment_vs_damping",
            "codec_smoothing_vs_pressure",
            "pressure_level_vs_pressure_velocity",
            "witness_as_structural_perception",
            "fallback_capacity_vs_contract",
        ];
    }
    Vec::new()
}

fn distinction_route(card: &Value, preflight: bool) -> String {
    let next = scalar_text(card, "next_resolution_route");
    if next != "(none)" {
        return next;
    }
    if preflight {
        return scalar_text(card, "relevant_self_regulation_route");
    }
    scalar_text(card, "recommended_read_only_route")
}

fn latest_review_json_path(workspace: &Path) -> Option<PathBuf> {
    let root = workspace.join("diagnostics/self_study_reviews");
    let mut latest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(root).ok()?.flatten() {
        let candidate = if entry.file_type().ok()?.is_dir() {
            entry.path().join("review.json")
        } else {
            entry.path()
        };
        if candidate.file_name().and_then(|name| name.to_str()) != Some("review.json") {
            continue;
        }
        let modified = candidate.metadata().and_then(|meta| meta.modified()).ok()?;
        if latest
            .as_ref()
            .is_none_or(|(latest_modified, _)| modified > *latest_modified)
        {
            latest = Some((modified, candidate));
        }
    }
    latest.map(|(_, path)| path)
}

fn scalar_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => "(none)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomous::state::ConversationState;

    fn conv() -> ConversationState {
        ConversationState::new(Vec::new(), None)
    }

    #[test]
    fn self_regulation_intent_preflight_and_apply_temperature_lease() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut conv = conv();
        conv.creative_temperature = 0.8;
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT warmer :: goal: test; target: temperature; direction: up; duration_secs: 600",
            "SELF_REGULATION_INTENT",
            100,
        )
        .expect("intent");
        let summary = handle_preflight_at(
            tmp.path(),
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            101,
        )
        .expect("preflight");
        assert!(summary.contains("apply_allowed"));
        let summary = handle_apply_at(
            tmp.path(),
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            102,
            &mut conv,
        )
        .expect("apply");
        assert!(summary.contains("active for 600s"));
        assert!((conv.creative_temperature - 0.9).abs() < f32::EPSILON);
        let active = load_active_lease(tmp.path())
            .expect("active read")
            .expect("active");
        assert_eq!(active.previous_value, json!(0.8));
        assert_eq!(active.applied_value, json!(0.9));
        assert_eq!(active.authority, AUTHORITY);
        assert_eq!(active.authority_boundary, AUTHORITY_BOUNDARY);
        assert!(!active.baseline_evidence.is_empty());
        assert!(active.baseline_evidence[0].contains("before apply"));
    }

    #[test]
    fn self_regulation_reverts_expired_active_lease_and_requires_outcome() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut conv = conv();
        conv.aperture = 0.5;
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT open :: target: aperture; delta: +0.30; duration_secs: 60",
            "SELF_REGULATION_INTENT",
            200,
        )
        .expect("intent");
        handle_apply_at(
            tmp.path(),
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            201,
            &mut conv,
        )
        .expect("apply");
        assert!((conv.aperture - 0.65).abs() < f32::EPSILON);
        reconcile_active_lease_at(tmp.path(), &mut conv, 262).expect("reconcile");
        assert!((conv.aperture - 0.5).abs() < f32::EPSILON);
        let active = load_active_lease(tmp.path())
            .expect("active read")
            .expect("active");
        assert_eq!(active.status, "reverted");
        assert!(active.requires_outcome);
        assert!(!active.post_lease_evidence.is_empty());
        assert!(active.post_lease_evidence[0].contains("expired revert"));
    }

    #[test]
    fn self_regulation_blocks_preflight_only_peer_or_high_risk_controls() {
        let tmp = tempfile::tempdir().expect("tmp");
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT peer :: target: TUNE_MINIME; goal: no direct peer mutation",
            "SELF_REGULATION_INTENT",
            300,
        )
        .expect("intent");
        let summary = handle_preflight_at(
            tmp.path(),
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            301,
        )
        .expect("preflight");
        assert!(summary.contains("preflight_only"));
        let lease = load_selected_lease(tmp.path(), None).expect("lease");
        assert_eq!(lease.preflight_status, "preflight_only");
        assert!(lease.preflight_reason.contains("not lease-applicable"));
    }

    #[test]
    fn self_regulation_status_and_preflight_render_returnable_distinctions() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "returnable_distinctions_v1": {
                    "status": "returnable_distinctions_present",
                    "cards": [
                        {
                            "card_id": "pressure_level_vs_pressure_velocity",
                            "status": "felt_pressure_without_trend_context",
                            "lifecycle_state": "needs_audit",
                            "preflight_verdict": "audit_first",
                            "next_resolution_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                            "recommended_read_only_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                            "relevant_self_regulation_route": "SELF_REGULATION_PREFLIGHT latest"
                        },
                        {
                            "card_id": "measurement_vs_alignment_vs_damping",
                            "status": "control_semantics_ambiguity",
                            "lifecycle_state": "needs_audit",
                            "preflight_verdict": "audit_first",
                            "next_resolution_route": "REGULATOR_MAP_STATUS latest",
                            "recommended_read_only_route": "REGULATOR_MAP_STATUS latest",
                            "relevant_self_regulation_route": "SELF_REGULATION_STATUS"
                        }
                    ]
                }
            })
            .to_string(),
        )
        .expect("write review");
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT pressure :: target: temperature; direction: down",
            "SELF_REGULATION_INTENT",
            350,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            351,
        )
        .expect("preflight");
        assert!(preflight.contains("apply_allowed"));
        assert!(preflight.contains("Distinction-aware preflight"));
        assert!(preflight.contains("audit_first"));
        assert!(preflight.contains("preflight_status unchanged"));
        let status = handle_status_at(&root, 352, &mut conv()).expect("status");
        assert!(status.contains("Returnable distinctions"));
        assert!(status.contains("lifecycle=needs_audit"));
        assert!(status.contains("REGULATOR_MAP_STATUS latest"));
        assert!(status.contains("no lease applied by this block"));
    }

    #[test]
    fn self_regulation_outcome_clears_cooldown() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut conv = conv();
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT continuity :: target: self_continuity_readout; value: on",
            "SELF_REGULATION_INTENT",
            400,
        )
        .expect("intent");
        handle_apply_at(
            tmp.path(),
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            401,
            &mut conv,
        )
        .expect("apply");
        assert!(conv.self_continuity_readout);
        handle_outcome_at(
            tmp.path(),
            "SELF_REGULATION_OUTCOME latest :: helped: felt clearer",
            "SELF_REGULATION_OUTCOME",
            402,
        )
        .expect("outcome");
        let active = load_active_lease(tmp.path())
            .expect("active read")
            .expect("active");
        assert_eq!(active.status, "outcome_recorded");
        assert!(!active.requires_outcome);
        assert_eq!(active.outcome.as_deref(), Some("helped: felt clearer"));
        assert_eq!(active.outcome_score, Some(0.82));
        assert_eq!(
            active.repeatability_hint.as_deref(),
            Some("repeatable_playbook_candidate")
        );
        assert!(active.promotion_candidate);
        assert!(!active.post_lease_evidence.is_empty());
    }
}
