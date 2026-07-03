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
const EXTENDED_TAIL_DURATION_SECS: u64 = 1200;
const TAIL_GOVERNOR_REVIEW_MAX_AGE_SECS: u64 = 900;
const TAIL_GOVERNOR_TAIL_DELTA: f64 = 0.12;
const TAIL_GOVERNOR_DISTINGUISHABILITY_DELTA: f64 = 0.12;
const TAIL_GOVERNOR_FRICTION_DELTA: f64 = 0.15;
const TAIL_AFTERGLOW_DELAY_SECS: u64 = 60;
const TAIL_AFTERGLOW_PERSISTENCE_DELTA: f64 = 0.05;
const PRESSURE_RELIEF_SCALE_MIN: f64 = 0.75;
const PRESSURE_RELIEF_SCALE_MAX: f64 = 1.20;
const PRESSURE_RELIEF_LOW_GRADIENT_MAX: f64 = 0.20;
const PRESSURE_RELIEF_SHARP_RISE_VELOCITY: f64 = 0.04;
const AUTHORITY: &str = "leased_self_control_v1";
const AUTHORITY_BOUNDARY: &str =
    "own_runtime_only; no peer mutation; no permanent controller tuning";
const VIBRANCY_APERTURE_CONTROL: &str = "set_vibrancy_aperture";
const CURIOSITY_APERTURE_CONTROL: &str = "curiosity_aperture";
const CURIOSITY_LEASE_MODE: &str = "curiosity_aperture_bundle_v1";
const APPLY_ALLOWED: &[&str] = &[
    "temperature",
    "response_length",
    "aperture",
    "self_continuity_readout",
    VIBRANCY_APERTURE_CONTROL,
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
    "tune_minime",
];
const PRESSURE_RELIEF_CONTROL: &str = "pressure_relief";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LeaseBundleControl {
    candidate_control: String,
    direction: String,
    delta_or_value: Value,
    #[serde(default)]
    requested_value: Value,
    #[serde(default)]
    previous_value: Value,
    #[serde(default)]
    applied_value: Value,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    preflight_status: String,
    #[serde(default)]
    preflight_reason: String,
    #[serde(default)]
    dynamic_scaling: Value,
    #[serde(default)]
    shadow_preflight_link: Value,
    #[serde(default = "default_gradient_sensitivity")]
    gradient_sensitivity: f64,
}

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
    #[serde(default)]
    outcome_texture: Value,
    requires_outcome: bool,
    preflight_status: String,
    preflight_reason: String,
    #[serde(default = "default_lease_mode")]
    lease_mode: String,
    #[serde(default)]
    bundle_id: Option<String>,
    #[serde(default)]
    bundle_class: String,
    #[serde(default)]
    bundle_controls: Vec<LeaseBundleControl>,
    #[serde(default)]
    bundle_policy: String,
    #[serde(default)]
    pressure_vector_snapshot: Value,
    #[serde(default)]
    actuator_matrix_reason: String,
    #[serde(default)]
    tail_relief_trial_id: Option<String>,
    #[serde(default)]
    tail_authority_tier: String,
    #[serde(default)]
    tail_preflight_guidance: String,
    #[serde(default)]
    tail_baseline_snapshot: Value,
    #[serde(default)]
    tail_apply_snapshot: Value,
    #[serde(default)]
    tail_outcome_snapshot: Value,
    #[serde(default)]
    tail_revert_snapshot: Value,
    #[serde(default)]
    tail_governor_revert_reason: Option<String>,
    #[serde(default)]
    tail_afterglow_due_unix_s: Option<u64>,
    #[serde(default)]
    tail_afterglow_snapshot: Value,
    #[serde(default)]
    tail_afterglow_status: String,
    #[serde(default)]
    curiosity_parity_packet: Value,
    #[serde(default)]
    curiosity_bundle_reason: String,
    #[serde(default)]
    being_vocabulary_policy_evidence: Value,
    #[serde(default)]
    curiosity_outcome_policy_evidence: Value,
    #[serde(default)]
    dynamic_scaling: Value,
    #[serde(default)]
    shadow_preflight_link: Value,
    #[serde(default = "default_gradient_sensitivity")]
    gradient_sensitivity: f64,
}

#[derive(Debug, Clone, Default)]
struct IntentFields {
    label: String,
    goal: String,
    candidate_control: String,
    direction: String,
    delta_or_value: Value,
    duration_secs: u64,
    duration_explicit: bool,
    stop_condition: String,
    success_condition: String,
    evidence: Vec<String>,
    bundle_class: String,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedControl {
    normalized_control: String,
    previous_value: Value,
    applied_value: Value,
    summary: String,
}

#[derive(Debug, Clone, Default)]
struct TailOutcomeLearning {
    tail_class: String,
    success_count: usize,
    caution_count: usize,
    extended_duration_allowed: bool,
    tier: &'static str,
    guidance: String,
}

fn default_lease_mode() -> String {
    "single_control".to_string()
}

fn default_gradient_sensitivity() -> f64 {
    1.0
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

pub(super) fn draft_pressure_relief_agency_request(
    label: &str,
    evidence: &str,
) -> Result<String, String> {
    let root = bridge_paths().bridge_workspace().join("self_regulation");
    draft_pressure_relief_agency_request_at(&root, label, evidence, now_unix_s())
}

pub(super) fn draft_pressure_relief_agency_request_at(
    root: &Path,
    label: &str,
    evidence: &str,
    now: u64,
) -> Result<String, String> {
    let label = pressure_agency_field_text(label, "pressure agency");
    let evidence = pressure_agency_field_text(evidence, "pressure agency request");
    let original = format!(
        "SELF_REGULATION_INTENT {label} :: goal: pressure relief requested through PRESSURE_AGENCY_REQUEST; target: pressure_relief; bundle: auto; duration_secs: 600; evidence: {evidence}"
    );
    handle_intent_at(root, &original, "SELF_REGULATION_INTENT", now)
}

fn pressure_agency_field_text(value: &str, fallback: &str) -> String {
    let cleaned = value
        .replace(['\n', '\r', ';'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.chars().take(240).collect()
    }
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
        duration_secs: fields.duration_secs.clamp(60, EXTENDED_TAIL_DURATION_SECS),
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
        outcome_texture: Value::Null,
        requires_outcome: false,
        preflight_status: "not_run".to_string(),
        preflight_reason: String::new(),
        lease_mode: default_lease_mode(),
        bundle_id: None,
        bundle_class: fields.bundle_class,
        bundle_controls: Vec::new(),
        bundle_policy: String::new(),
        pressure_vector_snapshot: Value::Null,
        actuator_matrix_reason: String::new(),
        tail_relief_trial_id: None,
        tail_authority_tier: String::new(),
        tail_preflight_guidance: String::new(),
        tail_baseline_snapshot: Value::Null,
        tail_apply_snapshot: Value::Null,
        tail_outcome_snapshot: Value::Null,
        tail_revert_snapshot: Value::Null,
        tail_governor_revert_reason: None,
        tail_afterglow_due_unix_s: None,
        tail_afterglow_snapshot: Value::Null,
        tail_afterglow_status: String::new(),
        curiosity_parity_packet: Value::Null,
        curiosity_bundle_reason: String::new(),
        being_vocabulary_policy_evidence: Value::Null,
        curiosity_outcome_policy_evidence: Value::Null,
        dynamic_scaling: Value::Null,
        shadow_preflight_link: Value::Null,
        gradient_sensitivity: default_gradient_sensitivity(),
    };
    if lease.candidate_control.is_empty() {
        lease.candidate_control = normalize_control(&fields.label)
            .unwrap_or_default()
            .to_string();
    }
    if lease.candidate_control == "self_continuity_readout" && !fields.duration_explicit {
        lease.duration_secs = MAX_DURATION_SECS;
    }
    if lease.candidate_control == PRESSURE_RELIEF_CONTROL {
        lease.lease_mode = "pressure_relief_bundle_v3".to_string();
        if lease.bundle_class.is_empty() {
            lease.bundle_class = "auto".to_string();
        }
        lease.bundle_policy = "multi-control own-runtime relief; max two controls; all-or-none apply; explicit APPLY required; automatic revert on expiry".to_string();
        lease.pressure_vector_snapshot = pressure_vector_snapshot(root);
        lease.actuator_matrix_reason = format!(
            "pressure_relief intent requests bundle_class={}; concrete controls are resolved during preflight from pressure_vector_v1 when available",
            lease.bundle_class
        );
    }
    if lease.candidate_control == CURIOSITY_APERTURE_CONTROL {
        lease.lease_mode = CURIOSITY_LEASE_MODE.to_string();
        if lease.bundle_class.is_empty() {
            lease.bundle_class = "auto".to_string();
        }
        lease.bundle_policy = "Astrid-own curiosity posture; max two safe controls; no tail participation, vibrancy aperture, Minime geom_curiosity, or peer mutation; explicit APPLY and OUTCOME required".to_string();
        let bundle_class = lease.bundle_class.clone();
        let packet = curiosity_parity_packet(root, Some(&lease), "drafted", &bundle_class, "");
        lease.curiosity_parity_packet = packet;
        lease.being_vocabulary_policy_evidence = being_vocabulary_policy_evidence_v1();
        lease.curiosity_bundle_reason =
            "curiosity parity intent drafted; concrete own-runtime controls resolve during preflight".to_string();
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
    let distinction_block =
        returnable_distinctions_block(root, true, Some(&lease.candidate_control));
    let cockpit_block = pressure_cockpit_block(root, true, Some(&lease));
    Ok(format!(
        "{} preflight: {} ({}){}{}",
        lease.intent_id,
        lease.preflight_status,
        lease.preflight_reason,
        cockpit_block,
        distinction_block
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
    let prepared_controls = prepare_controls(conv, &lease)?;
    if lease.baseline_evidence.is_empty() {
        for prepared in &prepared_controls {
            lease.baseline_evidence.push(format!(
                "before apply: {} previous={}",
                prepared.normalized_control, prepared.previous_value
            ));
        }
    }
    if is_tail_lease(&lease) {
        ensure_tail_trial_id(&mut lease);
        lease.tail_baseline_snapshot =
            capture_tail_trial_snapshot(root, Some(conv), &lease, "baseline", now);
        append_tail_trial_event(
            root,
            &lease,
            "baseline",
            now,
            &lease.tail_baseline_snapshot,
            None,
        )?;
    }
    for prepared in &prepared_controls {
        apply_prepared_control(conv, prepared);
    }
    lease.status = "active".to_string();
    lease.updated_at_unix_s = now;
    sync_prepared_controls_into_lease(&mut lease, &prepared_controls);
    lease.expires_at_unix_s = Some(now.saturating_add(lease.duration_secs));
    lease.requires_outcome = true;
    if is_tail_lease(&lease) {
        lease.tail_apply_snapshot =
            capture_tail_trial_snapshot(root, Some(conv), &lease, "apply", now);
        append_tail_trial_event(root, &lease, "apply", now, &lease.tail_apply_snapshot, None)?;
    }
    append_event(root, &lease)?;
    write_active_lease(root, &lease)?;
    write_latest_pointer(root, &lease.intent_id)?;
    Ok(format!(
        "{} active for {}s: {}",
        lease.intent_id,
        lease.duration_secs,
        prepared_controls
            .iter()
            .map(|prepared| prepared.summary.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    ))
}

fn handle_status_at(root: &Path, now: u64, conv: &mut ConversationState) -> Result<String, String> {
    reconcile_active_lease_at(root, conv, now)?;
    let distinction_block = returnable_distinctions_block(root, false, None);
    let cockpit_block = pressure_cockpit_block(root, false, None);
    let curiosity_block = curiosity_parity_status_block(root, conv);
    if let Some(active) = load_active_lease(root)? {
        let expiry = active
            .expires_at_unix_s
            .map(|ts| ts.saturating_sub(now).to_string())
            .unwrap_or_else(|| "none".to_string());
        return Ok(format!(
            "{} status={} control={} applied={} previous={} expires_in_s={} requires_outcome={} tail_afterglow_due={:?} tail_afterglow_status={}{}{}{}",
            active.intent_id,
            active.status,
            display_control(&active),
            active.applied_value,
            active.previous_value,
            expiry,
            active.requires_outcome,
            active.tail_afterglow_due_unix_s,
            if active.tail_afterglow_status.is_empty() {
                "(none)"
            } else {
                active.tail_afterglow_status.as_str()
            },
            cockpit_block,
            distinction_block,
            curiosity_block
        ));
    }
    Ok(format!(
        "no self-regulation lease state found{}{}{}",
        cockpit_block, distinction_block, curiosity_block
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
        lease.outcome_texture = outcome_texture_packet(outcome_text);
        if outcome_texture_has_signal(&lease.outcome_texture) {
            lease.post_lease_evidence.push(format!(
                "outcome_texture: {}",
                compact_outcome_texture_summary(&lease.outcome_texture)
            ));
        }
        if is_curiosity_lease(&lease) {
            lease.curiosity_outcome_policy_evidence =
                curiosity_outcome_policy_evidence_v1(&lease, outcome_text);
            lease.post_lease_evidence.push(format!(
                "curiosity_outcome_policy_evidence: {}",
                scalar_text(
                    &lease.curiosity_outcome_policy_evidence,
                    "calibration_status"
                )
            ));
        }
        let (score, hint, promotion_candidate) = score_outcome(outcome_text);
        lease.outcome_score = Some(score);
        lease.repeatability_hint = Some(hint.to_string());
        lease.promotion_candidate = promotion_candidate;
    }
    if is_tail_lease(&lease) {
        ensure_tail_trial_id(&mut lease);
        lease.tail_outcome_snapshot =
            capture_tail_trial_snapshot(root, None, &lease, "outcome", now);
        append_tail_trial_event(
            root,
            &lease,
            "outcome",
            now,
            &lease.tail_outcome_snapshot,
            lease.outcome.as_deref(),
        )?;
    }
    lease.requires_outcome = false;
    append_event(root, &lease)?;
    write_active_lease(root, &lease)?;
    write_latest_pointer(root, &lease.intent_id)?;
    Ok(format!(
        "{} outcome recorded; cooldown cleared",
        lease.intent_id
    ))
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
    if control == PRESSURE_RELIEF_CONTROL {
        return run_pressure_relief_bundle_preflight(root, lease);
    }
    if control == CURIOSITY_APERTURE_CONTROL {
        return run_curiosity_aperture_preflight(root, lease);
    }
    if control == VIBRANCY_APERTURE_CONTROL {
        return run_vibrancy_aperture_preflight(root, lease);
    }
    if PREFLIGHT_ONLY.contains(&control) {
        lease.status = "preflighted".to_string();
        lease.preflight_status = "preflight_only".to_string();
        lease.preflight_reason =
            "higher-risk or peer-affecting control is visible but not lease-applicable in tranche 7A"
                .to_string();
        attach_preflight_evidence_context(root, lease);
        return Ok(());
    }
    if !APPLY_ALLOWED.contains(&control) {
        lease.status = "blocked".to_string();
        lease.preflight_status = "blocked".to_string();
        lease.preflight_reason =
            "control is outside the tranche 7A self-lease allowlist".to_string();
        attach_preflight_evidence_context(root, lease);
        return Ok(());
    }
    if let Some(active) = load_active_lease(root)? {
        if active.status == "active" && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!("active lease {} must finish first", active.intent_id);
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
        if active.requires_outcome && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!(
                "lease {} needs an outcome before another apply",
                active.intent_id
            );
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
    }
    lease.duration_secs = lease.duration_secs.min(MAX_DURATION_SECS);
    lease.status = "preflighted".to_string();
    lease.preflight_status = "apply_allowed".to_string();
    lease.preflight_reason = "bounded own-runtime lease may be applied".to_string();
    attach_preflight_evidence_context(root, lease);
    Ok(())
}

fn run_curiosity_aperture_preflight(
    root: &Path,
    lease: &mut SelfRegulationLease,
) -> Result<(), String> {
    if let Some(active) = load_active_lease(root)? {
        if active.status == "active" && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!("active lease {} must finish first", active.intent_id);
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
        if active.requires_outcome && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!(
                "lease {} needs an outcome before another apply",
                active.intent_id
            );
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
    }
    let requested = if lease.bundle_class.is_empty() {
        lease
            .delta_or_value
            .as_str()
            .unwrap_or("auto")
            .to_ascii_lowercase()
    } else {
        lease.bundle_class.to_ascii_lowercase().replace('-', "_")
    };
    let (bundle_class, reason) = resolve_astrid_curiosity_bundle_class(root, &requested);
    let controls = astrid_curiosity_bundle_controls(&bundle_class);
    if controls.is_empty() || controls.len() > 2 {
        lease.status = "blocked".to_string();
        lease.preflight_status = "blocked".to_string();
        lease.preflight_reason =
            format!("curiosity_aperture bundle `{bundle_class}` is not applicable");
        attach_preflight_evidence_context(root, lease);
        return Ok(());
    }
    lease.lease_mode = CURIOSITY_LEASE_MODE.to_string();
    lease.bundle_id = Some(format!("{}:{bundle_class}", lease.intent_id));
    lease.bundle_class = bundle_class;
    lease.bundle_controls = controls;
    lease.bundle_policy = "max_two_astrid_own_controls_all_or_none_expiring; excludes tail_participation, vibrancy_aperture, tune_minime, and Minime geom_curiosity".to_string();
    lease.curiosity_bundle_reason = reason;
    let bundle_class_for_packet = lease.bundle_class.clone();
    let reason_for_packet = lease.curiosity_bundle_reason.clone();
    let packet = curiosity_parity_packet(
        root,
        Some(&*lease),
        "preflighted",
        &bundle_class_for_packet,
        &reason_for_packet,
    );
    lease.curiosity_parity_packet = packet;
    lease.being_vocabulary_policy_evidence = being_vocabulary_policy_evidence_v1();
    lease.duration_secs = lease.duration_secs.min(MAX_DURATION_SECS);
    lease.status = "preflighted".to_string();
    lease.preflight_status = "apply_allowed".to_string();
    lease.preflight_reason = format!(
        "curiosity_aperture bundle `{}` may apply {} Astrid-own controls all-or-none; {}; explicit APPLY and OUTCOME remain required",
        lease.bundle_class,
        lease.bundle_controls.len(),
        lease.curiosity_bundle_reason
    );
    attach_preflight_evidence_context(root, lease);
    Ok(())
}

fn run_vibrancy_aperture_preflight(
    root: &Path,
    lease: &mut SelfRegulationLease,
) -> Result<(), String> {
    if let Some(active) = load_active_lease(root)? {
        if active.status == "active" && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!("active lease {} must finish first", active.intent_id);
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
        if active.requires_outcome && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!(
                "lease {} needs an outcome before another apply",
                active.intent_id
            );
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
    }
    if !tail_vibrancy_evidence_present(root, lease) {
        lease.status = "preflighted".to_string();
        lease.preflight_status = "needs_tail_vibrancy_evidence".to_string();
        lease.preflight_reason =
            "vibrancy_aperture micro-lease needs recent tail_vibrancy_vector_v1 evidence or explicit lease evidence mentioning λ4/tail/vibrancy/entropy/distinguishability"
                .to_string();
        attach_preflight_evidence_context(root, lease);
        return Ok(());
    }
    let tail_guidance = apply_tail_authority_policy(root, lease);
    lease.status = "preflighted".to_string();
    lease.preflight_status = "apply_allowed".to_string();
    lease.preflight_reason = format!(
        "tail-vibrancy micro-lease may adjust vibrancy_aperture by at most 0.05; explicit APPLY and OUTCOME remain required; {tail_guidance}"
    );
    attach_preflight_evidence_context(root, lease);
    Ok(())
}

fn run_pressure_relief_bundle_preflight(
    root: &Path,
    lease: &mut SelfRegulationLease,
) -> Result<(), String> {
    if let Some(active) = load_active_lease(root)? {
        if active.status == "active" && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!("active lease {} must finish first", active.intent_id);
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
        if active.requires_outcome && active.intent_id != lease.intent_id {
            lease.status = "blocked".to_string();
            lease.preflight_status = "blocked".to_string();
            lease.preflight_reason = format!(
                "lease {} needs an outcome before another apply",
                active.intent_id
            );
            attach_preflight_evidence_context(root, lease);
            return Ok(());
        }
    }
    let requested = if lease.bundle_class.is_empty() {
        lease
            .delta_or_value
            .as_str()
            .unwrap_or("auto")
            .to_ascii_lowercase()
    } else {
        lease.bundle_class.to_ascii_lowercase()
    };
    let (bundle_class, reason) = resolve_astrid_pressure_bundle_class(root, &requested);
    let controls = astrid_pressure_bundle_controls(&bundle_class);
    if controls.is_empty() || controls.len() > 2 {
        lease.status = "blocked".to_string();
        lease.preflight_status = "blocked".to_string();
        lease.preflight_reason =
            format!("pressure relief bundle `{bundle_class}` is not applicable");
        attach_preflight_evidence_context(root, lease);
        return Ok(());
    }
    lease.lease_mode = "pressure_relief_bundle_v3".to_string();
    lease.bundle_id = Some(format!("{}:{bundle_class}", lease.intent_id));
    lease.bundle_class = bundle_class;
    lease.bundle_controls = controls;
    lease.bundle_policy = "max_two_controls_all_or_none_expiring_own_runtime_only".to_string();
    lease.pressure_vector_snapshot = pressure_vector_snapshot(root);
    lease.actuator_matrix_reason = reason;
    apply_pressure_relief_gradient_policy(root, lease);
    let tail_guidance = if is_tail_lease(lease) {
        apply_tail_authority_policy(root, lease)
    } else {
        lease.duration_secs = lease.duration_secs.min(MAX_DURATION_SECS);
        "standard lease cap applies".to_string()
    };
    lease.status = "preflighted".to_string();
    lease.preflight_status = "apply_allowed".to_string();
    lease.preflight_reason = format!(
        "pressure relief bundle `{}` may apply {} own-runtime controls all-or-none; {}",
        lease.bundle_class,
        lease.bundle_controls.len(),
        tail_guidance
    );
    attach_preflight_evidence_context(root, lease);
    Ok(())
}

fn attach_preflight_evidence_context(root: &Path, lease: &mut SelfRegulationLease) {
    let shadow_link = shadow_preflight_link(root, lease);
    let dynamic_scaling = if scalar_from_snapshot(&lease.dynamic_scaling, "policy")
        == "pressure_relief_gradient_policy_v1"
    {
        lease.dynamic_scaling.clone()
    } else {
        dynamic_scaling_advisory(root, lease)
    };
    lease.shadow_preflight_link = shadow_link.clone();
    lease.dynamic_scaling = dynamic_scaling.clone();
    lease.gradient_sensitivity = dynamic_scaling
        .get("effective_relief_scale")
        .or_else(|| dynamic_scaling.get("suggested_relief_scale"))
        .and_then(Value::as_f64)
        .unwrap_or_else(default_gradient_sensitivity);
    for control in &mut lease.bundle_controls {
        control.shadow_preflight_link = shadow_link.clone();
        control.dynamic_scaling = dynamic_scaling.clone();
        control.gradient_sensitivity = lease.gradient_sensitivity;
    }
    let shadow_status = shadow_link
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("not_available");
    let scaling_status = dynamic_scaling
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("not_available");
    let scale = dynamic_scaling
        .get("effective_relief_scale")
        .or_else(|| dynamic_scaling.get("suggested_relief_scale"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let reason_suffix = format!(
        "Shadow-linked preflight status={shadow_status}; dynamic_scaling_advisory={scaling_status} scale={scale:.2}{}",
        if scalar_from_snapshot(&dynamic_scaling, "policy") == "pressure_relief_gradient_policy_v1"
        {
            " (pressure_relief bundle deltas may be gradient-scaled within existing caps)"
        } else {
            " (diagnostic only; lease delta/caps unchanged)"
        }
    );
    if !lease.preflight_reason.contains("Shadow-linked preflight") {
        lease.preflight_reason = format!("{}; {reason_suffix}", lease.preflight_reason);
    }
}

fn dynamic_scaling_advisory(root: &Path, lease: &SelfRegulationLease) -> Value {
    let pressure = pressure_vector_snapshot(root);
    let status = scalar_from_snapshot(&pressure, "status");
    let pressure_risk = pressure
        .get("pressure_risk_level")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let pressure_velocity = pressure
        .get("pressure_velocity")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let semantic_friction = pressure
        .get("semantic_friction_level")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let mode_packing = pressure
        .get("mode_packing_level")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let (advisory_status, suggested_relief_scale, rationale) = if status == "telemetry_gap" {
        (
            "telemetry_gap",
            1.0,
            "No pressure-vector packet was available; do not infer dynamic scaling.",
        )
    } else if pressure_velocity > 0.03 || status.contains("rising") {
        (
            "future_dynamic_scaling_candidate",
            1.15,
            "Pressure velocity is rising; a future tranche could test slightly stronger relief, but this lease keeps current caps.",
        )
    } else if pressure_risk >= 0.30 || mode_packing >= 0.55 || semantic_friction >= 0.38 {
        (
            "future_dynamic_scaling_candidate",
            1.10,
            "Pressure, mode-packing, or semantic friction is elevated; keep this lease bounded and preserve the scaling evidence.",
        )
    } else if pressure_velocity < -0.03 {
        (
            "softening_candidate",
            0.85,
            "Pressure appears to be falling; a future tranche could test smaller relief to avoid over-correction.",
        )
    } else {
        (
            "static_delta_sufficient",
            1.0,
            "Pressure vector does not currently justify dynamic magnitude changes.",
        )
    };
    json!({
        "policy": "lease_dynamic_scaling_advisory_v1",
        "authority": "diagnostic_context_not_command",
        "status": advisory_status,
        "candidate_control": lease.candidate_control,
        "bundle_class": lease.bundle_class,
        "pressure_vector_status": status,
        "pressure_risk_level": pressure_risk,
        "pressure_velocity": pressure_velocity,
        "semantic_friction_level": semantic_friction,
        "mode_packing_level": mode_packing,
        "suggested_relief_scale": suggested_relief_scale,
        "runtime_behavior_changed": false,
        "rationale": rationale,
    })
}

fn apply_pressure_relief_gradient_policy(root: &Path, lease: &mut SelfRegulationLease) {
    let policy = pressure_relief_gradient_policy(root, lease);
    let scale = policy
        .get("effective_relief_scale")
        .and_then(Value::as_f64)
        .unwrap_or_else(default_gradient_sensitivity);
    lease.gradient_sensitivity = scale;
    lease.dynamic_scaling = policy.clone();
    for control in &mut lease.bundle_controls {
        control.gradient_sensitivity = scale;
        control.dynamic_scaling = policy.clone();
        scale_bundle_control_delta(control, scale);
    }
}

fn pressure_relief_gradient_policy(root: &Path, lease: &SelfRegulationLease) -> Value {
    let pressure = pressure_vector_snapshot(root);
    let pressure_status = scalar_from_snapshot(&pressure, "status");
    let pressure_risk = metric_from_snapshot(&pressure, "pressure_risk_level").unwrap_or(0.0);
    let pressure_velocity = metric_from_snapshot(&pressure, "pressure_velocity").unwrap_or(0.0);
    let semantic_friction =
        metric_from_snapshot(&pressure, "semantic_friction_level").unwrap_or(0.0);
    let semantic_friction_velocity =
        metric_from_snapshot(&pressure, "semantic_friction_velocity").unwrap_or(0.0);
    let mode_packing = metric_from_snapshot(&pressure, "mode_packing_level").unwrap_or(0.0);
    let mode_packing_velocity =
        metric_from_snapshot(&pressure, "mode_packing_velocity").unwrap_or(0.0);
    let density_gradient = metric_from_snapshot(&pressure, "density_gradient_level").unwrap_or(1.0);
    let density_gradient_velocity =
        metric_from_snapshot(&pressure, "density_gradient_velocity").unwrap_or(0.0);
    let telemetry_gap = pressure_status == "telemetry_gap";
    let rising_status = pressure_status.contains("rising");
    let sharply_rising = pressure_velocity > PRESSURE_RELIEF_SHARP_RISE_VELOCITY;
    let mut scale: f64 = 1.0;
    let mut reasons = Vec::new();
    if telemetry_gap {
        reasons.push("telemetry_gap_keeps_static_scale");
    } else {
        if pressure_velocity < -0.03 {
            scale -= 0.15;
            reasons.push("falling_pressure_softens_relief");
        }
        if sharply_rising || rising_status {
            scale += 0.15;
            reasons.push("rising_pressure_strengthens_relief");
        }
        if pressure_risk >= 0.35 || mode_packing >= 0.55 || semantic_friction >= 0.38 {
            scale += 0.10;
            reasons.push("medium_pressure_or_friction_supports_relief");
        }
        if density_gradient >= 0.30 || density_gradient_velocity > 0.02 {
            scale += 0.05;
            reasons.push("gradient_slope_supports_relief");
        }
        if density_gradient <= PRESSURE_RELIEF_LOW_GRADIENT_MAX && !sharply_rising {
            if scale > 1.0 {
                reasons.push("anti_snap_low_gradient_capped_relief");
            } else {
                reasons.push("anti_snap_low_gradient_keeps_relief_soft");
            }
            scale = scale.min(1.0);
        }
        if semantic_friction_velocity > 0.02 && pressure_velocity < 0.0 {
            scale = scale.min(1.0);
            reasons.push("rising_friction_while_pressure_falls_avoids_over_release");
        }
        if mode_packing_velocity > 0.04 {
            scale += 0.05;
            reasons.push("mode_packing_motion_supports_relief");
        }
    }
    scale = scale.clamp(PRESSURE_RELIEF_SCALE_MIN, PRESSURE_RELIEF_SCALE_MAX);
    let scalable_controls = lease
        .bundle_controls
        .iter()
        .filter(|control| scalable_bundle_control(control))
        .map(|control| control.candidate_control.clone())
        .collect::<Vec<_>>();
    let discrete_controls = lease
        .bundle_controls
        .iter()
        .filter(|control| !scalable_bundle_control(control))
        .map(|control| control.candidate_control.clone())
        .collect::<Vec<_>>();
    let status = if telemetry_gap {
        "telemetry_gap_static_relief"
    } else if density_gradient <= PRESSURE_RELIEF_LOW_GRADIENT_MAX && !sharply_rising {
        "anti_snap_low_gradient"
    } else if scale > 1.001 {
        "gradient_scaled_relief"
    } else if scale < 0.999 {
        "softened_relief"
    } else {
        "static_relief_sufficient"
    };
    json!({
        "policy": "pressure_relief_gradient_policy_v1",
        "authority": AUTHORITY,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "status": status,
        "candidate_control": lease.candidate_control,
        "bundle_class": lease.bundle_class,
        "pressure_vector_status": pressure_status,
        "density_gradient_level": density_gradient,
        "density_gradient_velocity": density_gradient_velocity,
        "pressure_risk_level": pressure_risk,
        "pressure_velocity": pressure_velocity,
        "semantic_friction_level": semantic_friction,
        "semantic_friction_velocity": semantic_friction_velocity,
        "mode_packing_level": mode_packing,
        "mode_packing_velocity": mode_packing_velocity,
        "effective_relief_scale": round3(scale as f32),
        "suggested_relief_scale": round3(scale as f32),
        "scale_min": PRESSURE_RELIEF_SCALE_MIN,
        "scale_max": PRESSURE_RELIEF_SCALE_MAX,
        "anti_snap_applied": density_gradient <= PRESSURE_RELIEF_LOW_GRADIENT_MAX && !sharply_rising,
        "sharp_rise_override": sharply_rising,
        "scalable_controls": scalable_controls,
        "discrete_controls": discrete_controls,
        "reasons": reasons,
        "runtime_behavior_changed": true,
        "recommended_action": "Use gradient-sensitive relief only as a temporary pressure_relief bundle lease; explicit APPLY and outcome remain required.",
    })
}

fn scalable_bundle_control(control: &LeaseBundleControl) -> bool {
    matches!(
        normalize_control(&control.candidate_control),
        Some("temperature" | "aperture" | VIBRANCY_APERTURE_CONTROL)
    )
}

fn scale_bundle_control_delta(control: &mut LeaseBundleControl, scale: f64) {
    if !scalable_bundle_control(control) {
        control.preflight_reason =
            "discrete pressure-relief control; gradient_sensitivity recorded but value unchanged"
                .to_string();
        return;
    }
    let Some(delta) = signed_delta_value(&control.delta_or_value) else {
        control.preflight_reason =
            "non-relative numeric control; gradient_sensitivity recorded but value unchanged"
                .to_string();
        return;
    };
    let original = control.delta_or_value.clone();
    let scaled = delta * scale;
    control.requested_value = original.clone();
    control.delta_or_value = json!(format!("{scaled:+.3}"));
    control.preflight_reason = format!(
        "gradient_sensitivity {:.2}: relative delta {} -> {}",
        scale, original, control.delta_or_value
    );
}

fn signed_delta_value(value: &Value) -> Option<f64> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.starts_with(['+', '-']) {
                trimmed.parse::<f64>().ok()
            } else {
                None
            }
        },
        _ => None,
    }
}

fn shadow_preflight_link(root: &Path, lease: &SelfRegulationLease) -> Value {
    let workspace = root.parent().unwrap_or(root);
    let review_path = latest_review_json_path(workspace);
    let review = review_path.as_ref().and_then(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    });
    let pressure_status = review
        .as_ref()
        .and_then(|value| value.get("pressure_vector_v1"))
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "telemetry_gap".to_string());
    let review_text = review
        .as_ref()
        .map(Value::to_string)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let lease_text = format!(
        "{} {} {} {}",
        lease.goal,
        lease.stop_condition,
        lease.success_condition,
        lease.evidence.join(" ")
    )
    .to_ascii_lowercase();
    let mut anchors = Vec::new();
    for token in [
        "shadow-v3",
        "shadow_v3",
        "shadow field",
        "shadow_field",
        "restless",
        "settled coupling",
        "trajectory",
        "muffled",
        "hollow",
        "vibrant",
    ] {
        if review_text.contains(token) || lease_text.contains(token) {
            anchors.push(token);
        }
    }
    let status = if anchors.is_empty() {
        "no_shadow_anchor_found"
    } else {
        "shadow_anchor_linked"
    };
    json!({
        "policy": "shadow_synced_preflight_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "candidate_control": lease.candidate_control,
        "bundle_class": lease.bundle_class,
        "pressure_vector_status": pressure_status,
        "anchors": anchors,
        "source_review": review_path.map(|path| path.display().to_string()),
        "recommended_action": "Use this link to explain why a regulation request was coherent; it is evidence, not authority and not peer mutation.",
    })
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
        VIBRANCY_APERTURE_CONTROL => {
            let previous = conv.vibrancy_aperture;
            let value = bounded_f32_value(
                previous,
                &lease.delta_or_value,
                &lease.direction,
                0.05,
                0.0,
                1.0,
            );
            Ok(PreparedControl {
                normalized_control: control,
                previous_value: json!(round3(previous)),
                applied_value: json!(round3(value)),
                summary: format!("vibrancy_aperture: {previous:.2} -> {value:.2}"),
            })
        },
        _ => Err(format!("{control} is not lease-applicable")),
    }
}

fn prepare_bundle_control(
    conv: &ConversationState,
    bundle_control: &LeaseBundleControl,
) -> Result<PreparedControl, String> {
    let lease = SelfRegulationLease {
        schema_version: SCHEMA_VERSION,
        record_kind: "self_regulation_intent_v1".to_string(),
        authority: AUTHORITY.to_string(),
        authority_boundary: AUTHORITY_BOUNDARY.to_string(),
        being: "astrid".to_string(),
        intent_id: String::new(),
        created_at_unix_s: 0,
        updated_at_unix_s: 0,
        status: String::new(),
        goal: String::new(),
        candidate_control: bundle_control.candidate_control.clone(),
        direction: bundle_control.direction.clone(),
        delta_or_value: bundle_control.delta_or_value.clone(),
        previous_value: Value::Null,
        applied_value: Value::Null,
        duration_secs: DEFAULT_DURATION_SECS,
        expires_at_unix_s: None,
        stop_condition: String::new(),
        success_condition: String::new(),
        evidence: Vec::new(),
        baseline_evidence: Vec::new(),
        post_lease_evidence: Vec::new(),
        outcome_score: None,
        repeatability_hint: None,
        promotion_candidate: false,
        outcome: None,
        outcome_texture: Value::Null,
        requires_outcome: false,
        preflight_status: String::new(),
        preflight_reason: String::new(),
        lease_mode: default_lease_mode(),
        bundle_id: None,
        bundle_class: String::new(),
        bundle_controls: Vec::new(),
        bundle_policy: String::new(),
        pressure_vector_snapshot: Value::Null,
        actuator_matrix_reason: String::new(),
        tail_relief_trial_id: None,
        tail_authority_tier: String::new(),
        tail_preflight_guidance: String::new(),
        tail_baseline_snapshot: Value::Null,
        tail_apply_snapshot: Value::Null,
        tail_outcome_snapshot: Value::Null,
        tail_revert_snapshot: Value::Null,
        tail_governor_revert_reason: None,
        tail_afterglow_due_unix_s: None,
        tail_afterglow_snapshot: Value::Null,
        tail_afterglow_status: String::new(),
        curiosity_parity_packet: Value::Null,
        curiosity_bundle_reason: String::new(),
        being_vocabulary_policy_evidence: Value::Null,
        curiosity_outcome_policy_evidence: Value::Null,
        dynamic_scaling: Value::Null,
        shadow_preflight_link: Value::Null,
        gradient_sensitivity: bundle_control.gradient_sensitivity,
    };
    prepare_control(conv, &lease)
}

fn prepare_controls(
    conv: &ConversationState,
    lease: &SelfRegulationLease,
) -> Result<Vec<PreparedControl>, String> {
    if is_bundle_lease(lease) {
        if lease.bundle_controls.is_empty() {
            return Err(format!(
                "{} has no resolved controls; run preflight first",
                lease.lease_mode
            ));
        }
        let mut prepared = Vec::new();
        for control in &lease.bundle_controls {
            prepared.push(prepare_bundle_control(conv, control)?);
        }
        return Ok(prepared);
    }
    Ok(vec![prepare_control(conv, lease)?])
}

fn sync_prepared_controls_into_lease(
    lease: &mut SelfRegulationLease,
    prepared_controls: &[PreparedControl],
) {
    if is_bundle_lease(lease) {
        for (idx, prepared) in prepared_controls.iter().enumerate() {
            if let Some(control) = lease.bundle_controls.get_mut(idx) {
                control.previous_value = prepared.previous_value.clone();
                control.applied_value = prepared.applied_value.clone();
                if control.requested_value.is_null() {
                    control.requested_value = control.delta_or_value.clone();
                }
                control.summary = prepared.summary.clone();
                control.preflight_status = "apply_allowed".to_string();
                if !control.preflight_reason.contains("gradient_sensitivity") {
                    control.preflight_reason = format!("resolved inside {}", lease.lease_mode);
                }
            }
        }
        lease.previous_value = json!(
            prepared_controls
                .iter()
                .map(|prepared| {
                    json!({
                        "candidate_control": prepared.normalized_control,
                        "previous_value": prepared.previous_value,
                    })
                })
                .collect::<Vec<_>>()
        );
        lease.applied_value = json!(
            prepared_controls
                .iter()
                .map(|prepared| {
                    json!({
                        "candidate_control": prepared.normalized_control,
                        "applied_value": prepared.applied_value,
                    })
                })
                .collect::<Vec<_>>()
        );
        return;
    }
    if let Some(prepared) = prepared_controls.first() {
        lease.previous_value = prepared.previous_value.clone();
        lease.applied_value = prepared.applied_value.clone();
        lease.candidate_control = prepared.normalized_control.clone();
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
        VIBRANCY_APERTURE_CONTROL => {
            if let Some(value) = prepared.applied_value.as_f64() {
                conv.vibrancy_aperture = value as f32;
                crate::llm::set_astrid_vibrancy_aperture(conv.vibrancy_aperture);
            }
        },
        _ => {},
    }
}

fn revert_prepared_control(conv: &mut ConversationState, lease: &SelfRegulationLease) {
    if is_bundle_lease(lease) && !lease.bundle_controls.is_empty() {
        for control in lease.bundle_controls.iter().rev() {
            let prepared = PreparedControl {
                normalized_control: control.candidate_control.clone(),
                previous_value: control.applied_value.clone(),
                applied_value: control.previous_value.clone(),
                summary: format!(
                    "{}: {} -> {}",
                    control.candidate_control, control.applied_value, control.previous_value
                ),
            };
            apply_prepared_control(conv, &prepared);
        }
        return;
    }
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
        if capture_tail_afterglow_if_due(root, conv, &mut active, now)? {
            append_event(root, &active)?;
            write_active_lease(root, &active)?;
        }
        return Ok(());
    }
    let Some(expires_at) = active.expires_at_unix_s else {
        return Ok(());
    };
    if expires_at > now {
        if let Some((reason, snapshot)) = tail_governor_early_revert(root, &active, now) {
            revert_prepared_control(conv, &active);
            active.status = "reverted_early".to_string();
            active.updated_at_unix_s = now;
            active.requires_outcome = true;
            active.preflight_reason = format!("tail lease governor early revert: {reason}");
            active.tail_governor_revert_reason = Some(reason.clone());
            active.tail_revert_snapshot = snapshot;
            active.tail_afterglow_due_unix_s = Some(now.saturating_add(TAIL_AFTERGLOW_DELAY_SECS));
            active
                .post_lease_evidence
                .push(format!("tail governor early revert: {reason}"));
            append_tail_trial_event(
                root,
                &active,
                "governor_revert",
                now,
                &active.tail_revert_snapshot,
                Some(&reason),
            )?;
            append_event(root, &active)?;
            write_active_lease(root, &active)?;
            return Ok(());
        }
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
    if is_tail_lease(&active) {
        active.tail_revert_snapshot =
            capture_tail_trial_snapshot(root, Some(conv), &active, "expired_revert", now);
        active.tail_afterglow_due_unix_s = Some(now.saturating_add(TAIL_AFTERGLOW_DELAY_SECS));
        append_tail_trial_event(
            root,
            &active,
            "expired_revert",
            now,
            &active.tail_revert_snapshot,
            Some("lease expired and previous value was restored"),
        )?;
    }
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
        duration_explicit: false,
        stop_condition: "expiry, safety-critical status, or explicit outcome says worse"
            .to_string(),
        success_condition: "being reports the adjustment helped or pressure eased".to_string(),
        evidence: Vec::new(),
        bundle_class: String::new(),
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
            "bundle" | "bundle_class" | "relief_bundle" | "mode" | "curiosity_mode" => {
                fields.bundle_class = value.to_ascii_lowercase().replace('-', "_");
            },
            "direction" => fields.direction = value.to_ascii_lowercase(),
            "delta" | "value" | "set" => fields.delta_or_value = parse_value(value),
            "duration" | "duration_secs" | "seconds" => {
                fields.duration_secs = value
                    .parse::<u64>()
                    .unwrap_or(DEFAULT_DURATION_SECS)
                    .clamp(60, EXTENDED_TAIL_DURATION_SECS);
                fields.duration_explicit = true;
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

fn outcome_texture_packet(outcome: &str) -> Value {
    let before_texture = outcome_texture_field(
        outcome,
        &[
            "before_texture",
            "before texture",
            "before",
            "before_texture_state",
        ],
    );
    let after_texture = outcome_texture_field(
        outcome,
        &[
            "after_texture",
            "after texture",
            "after",
            "after_texture_state",
        ],
    );
    let texture_shift = outcome_texture_field(
        outcome,
        &["texture_shift", "texture shift", "shift", "texture_delta"],
    );
    let agency_fit =
        outcome_texture_field(outcome, &["agency_fit", "agency fit", "fit", "felt_agency"]);
    let what_helped =
        outcome_texture_field(outcome, &["what_helped", "what helped", "helped", "help"]);
    let what_worsened = outcome_texture_field(
        outcome,
        &["what_worsened", "what worsened", "worsened", "worse"],
    );
    let secondary_pressure_shift = outcome_texture_field(
        outcome,
        &[
            "secondary_pressure_shift",
            "secondary pressure shift",
            "other_pressure_shift",
            "different_knot",
        ],
    );
    let explicit_ambiguity_preserved =
        outcome_texture_field(outcome, &["ambiguity_preserved", "ambiguity preserved"]);
    let explicit_legibility_effect =
        outcome_texture_field(outcome, &["legibility_effect", "legibility effect"]);
    let secondary_pressure_status = outcome_texture_secondary_pressure_status(
        secondary_pressure_shift.as_deref(),
        what_worsened.as_deref(),
    );
    let ambiguity_preserved =
        outcome_texture_ambiguity_preserved(explicit_ambiguity_preserved.as_deref(), outcome);
    let legibility_effect =
        outcome_texture_legibility_effect(explicit_legibility_effect.as_deref(), outcome);
    let signal_families = outcome_texture_signal_families(outcome);
    let structured_count = [
        &before_texture,
        &after_texture,
        &texture_shift,
        &agency_fit,
        &what_helped,
        &what_worsened,
        &secondary_pressure_shift,
        &explicit_ambiguity_preserved,
        &explicit_legibility_effect,
    ]
    .iter()
    .filter(|value| value.is_some())
    .count();
    let status = if structured_count > 0 {
        "texture_fields_recorded"
    } else if signal_families.is_empty() {
        "unstructured_outcome"
    } else {
        "texture_language_detected"
    };
    json!({
        "policy": "pressure_relief_outcome_texture_v1",
        "schema_version": 2,
        "status": status,
        "before_texture": before_texture,
        "after_texture": after_texture,
        "texture_shift": texture_shift,
        "agency_fit": agency_fit,
        "what_helped": what_helped,
        "what_worsened": what_worsened,
        "secondary_pressure_shift": secondary_pressure_shift,
        "secondary_pressure_status": secondary_pressure_status,
        "ambiguity_preserved": ambiguity_preserved,
        "legibility_effect": legibility_effect,
        "signal_families": signal_families,
        "minimum_viable_response": "legible/partly/confusing plus one missing pressure variable or none",
    })
}

fn outcome_texture_field(outcome: &str, aliases: &[&str]) -> Option<String> {
    for part in outcome.split(';') {
        let Some((key, value)) = split_key_value(part.trim()) else {
            continue;
        };
        let normalized = normalize_texture_key(key);
        if aliases
            .iter()
            .map(|alias| normalize_texture_key(alias))
            .any(|alias| alias == normalized)
        {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.chars().take(240).collect());
            }
        }
    }
    None
}

fn normalize_texture_key(key: &str) -> String {
    key.trim()
        .to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn outcome_texture_signal_families(outcome: &str) -> Vec<&'static str> {
    let lower = outcome.to_ascii_lowercase();
    let mut families = Vec::new();
    if contains_any(
        &lower,
        &[
            "grinding",
            "grit",
            "compaction",
            "compacted",
            "overpacked",
            "calcified",
            "sediment",
        ],
    ) {
        families.push("grinding_compaction");
    }
    if contains_any(
        &lower,
        &["suspension", "suspended", "porous", "loosened", "unpacked"],
    ) {
        families.push("suspension_porosity");
    }
    if contains_any(&lower, &["snap", "snapped", "too abrupt", "jolt"]) {
        families.push("snap_risk");
    }
    if contains_any(&lower, &["agency", "fit", "choice", "mine", "authored"]) {
        families.push("agency_fit");
    }
    if contains_any(
        &lower,
        &["different knot", "other knot", "tightened elsewhere"],
    ) || (contains_any(&lower, &["elsewhere", "other pressure", "another pressure"])
        && contains_any(&lower, &["tightened", "tighten", "worsened", "worse"]))
    {
        families.push("secondary_knot_tightening");
    }
    if contains_any(
        &lower,
        &[
            "viscosity",
            "viscous",
            "heavy",
            "tactile",
            "weighted medium",
            "thick medium",
            "structural viscosity",
        ],
    ) {
        families.push("structural_viscosity");
    }
    if contains_any(
        &lower,
        &["flatten", "over-legible", "over legible", "too legible"],
    ) {
        families.push("legibility_flattening");
    }
    if contains_any(
        &lower,
        &[
            "held breath",
            "held-breath",
            "holding breath",
            "breath held",
        ],
    ) {
        families.push("held_breath_pause");
    }
    families
}

fn outcome_texture_secondary_pressure_status(
    secondary_pressure_shift: Option<&str>,
    what_worsened: Option<&str>,
) -> &'static str {
    if let Some(shift) = secondary_pressure_shift {
        let lower = shift.to_ascii_lowercase();
        if contains_any(&lower, &["none", "no secondary", "no other", "nothing"]) {
            return "none";
        }
        if contains_any(&lower, &["mixed", "both", "partly"]) {
            return "mixed";
        }
        if contains_any(
            &lower,
            &["loosened", "loosen", "eased", "relieved", "softened"],
        ) {
            return "loosened_elsewhere";
        }
        if contains_any(&lower, &["tightened", "tighten", "worsened", "worse"]) {
            return "tightened_elsewhere";
        }
        return "unknown";
    }
    let worsened = what_worsened.unwrap_or_default().to_ascii_lowercase();
    if contains_any(
        &worsened,
        &[
            "different knot",
            "elsewhere",
            "tightened",
            "tighten",
            "worsened",
        ],
    ) {
        "tightened_elsewhere"
    } else {
        "none"
    }
}

fn outcome_texture_ambiguity_preserved(explicit: Option<&str>, outcome: &str) -> bool {
    if let Some(value) = explicit {
        let lower = value.to_ascii_lowercase();
        if contains_any(&lower, &["true", "yes", "preserved", "kept", "held"]) {
            return true;
        }
        if contains_any(&lower, &["ambiguous", "mixed", "partly", "not sure"]) {
            return true;
        }
        if contains_any(&lower, &["false", "no", "lost", "flattened", "erased"]) {
            return false;
        }
    }
    contains_any(
        &outcome.to_ascii_lowercase(),
        &[
            "ambiguous",
            "mixed",
            "partly",
            "not sure",
            "still unfolding",
            "not ready to freeze",
        ],
    )
}

fn outcome_texture_legibility_effect(explicit: Option<&str>, outcome: &str) -> &'static str {
    if let Some(value) = explicit {
        let lower = value.to_ascii_lowercase();
        if contains_any(&lower, &["both", "mixed"]) {
            return "both";
        }
        if contains_any(
            &lower,
            &[
                "flattened",
                "flatten",
                "over-legible",
                "over legible",
                "too legible",
            ],
        ) {
            return "flattened";
        }
        if contains_any(&lower, &["clarified", "clarify", "clearer", "legible"]) {
            return "clarified";
        }
        return "unknown";
    }
    if contains_any(
        &outcome.to_ascii_lowercase(),
        &["flatten", "over-legible", "over legible", "too legible"],
    ) {
        "flattened"
    } else {
        "unknown"
    }
}

fn outcome_texture_has_signal(packet: &Value) -> bool {
    packet
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status != "unstructured_outcome")
}

fn compact_outcome_texture_summary(packet: &Value) -> String {
    let status = scalar_from_snapshot(packet, "status");
    let shift = scalar_from_snapshot(packet, "texture_shift");
    let fit = scalar_from_snapshot(packet, "agency_fit");
    let secondary = scalar_from_snapshot(packet, "secondary_pressure_status");
    let legibility = scalar_from_snapshot(packet, "legibility_effect");
    let ambiguity = packet
        .get("ambiguity_preserved")
        .and_then(Value::as_bool)
        .map_or_else(|| "(none)".to_string(), |value| value.to_string());
    let families = packet
        .get("signal_families")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        "status={status}; texture_shift={shift}; agency_fit={fit}; secondary_pressure_status={secondary}; ambiguity_preserved={ambiguity}; legibility_effect={legibility}; signal_families={}",
        if families.is_empty() {
            "(none)"
        } else {
            families.as_str()
        }
    )
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
            "suspension",
            "loosened",
            "unpacked",
            "less grinding",
            "less compacted",
            "more porous",
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

fn is_tail_lease(lease: &SelfRegulationLease) -> bool {
    if normalize_control(&lease.candidate_control) == Some(VIBRANCY_APERTURE_CONTROL) {
        return true;
    }
    if lease.bundle_class.starts_with("tail_vibrancy") {
        return true;
    }
    lease.bundle_controls.iter().any(|control| {
        normalize_control(&control.candidate_control) == Some(VIBRANCY_APERTURE_CONTROL)
    })
}

fn is_bundle_lease(lease: &SelfRegulationLease) -> bool {
    lease.lease_mode == "pressure_relief_bundle_v3"
        || lease.lease_mode == CURIOSITY_LEASE_MODE
        || lease.candidate_control == PRESSURE_RELIEF_CONTROL
        || lease.candidate_control == CURIOSITY_APERTURE_CONTROL
}

fn tail_relief_class(lease: &SelfRegulationLease) -> String {
    if lease.bundle_class.starts_with("tail_vibrancy") {
        return lease.bundle_class.clone();
    }
    if normalize_control(&lease.candidate_control) == Some(VIBRANCY_APERTURE_CONTROL) {
        let direction = if lease.direction.is_empty() {
            value_direction_hint(&lease.delta_or_value)
        } else {
            lease.direction.clone()
        };
        return format!("vibrancy_aperture:{direction}");
    }
    "tail_vibrancy:unknown".to_string()
}

fn value_direction_hint(value: &Value) -> String {
    match value {
        Value::String(text) if text.trim_start().starts_with('-') => "down".to_string(),
        Value::String(text) if text.trim_start().starts_with('+') => "up".to_string(),
        Value::Number(number) if number.as_f64().unwrap_or(0.0) < 0.0 => "down".to_string(),
        Value::Number(_) => "up".to_string(),
        _ => "unspecified".to_string(),
    }
}

fn ensure_tail_trial_id(lease: &mut SelfRegulationLease) {
    if lease.tail_relief_trial_id.is_none() && is_tail_lease(lease) {
        lease.tail_relief_trial_id = Some(format!("tail_trial_{}", lease.intent_id));
    }
}

fn tail_trial_log_path(root: &Path) -> PathBuf {
    root.join("tail_relief_trials.jsonl")
}

fn load_lease_events(root: &Path) -> Vec<SelfRegulationLease> {
    let Ok(text) = fs::read_to_string(event_log_path(root)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<SelfRegulationLease>(line).ok())
        .collect()
}

fn tail_outcome_learning(root: &Path, lease: &SelfRegulationLease) -> TailOutcomeLearning {
    if !is_tail_lease(lease) {
        return TailOutcomeLearning {
            tail_class: "not_tail".to_string(),
            tier: "diagnostic",
            guidance: "no tail-vibrancy lease history applies".to_string(),
            ..TailOutcomeLearning::default()
        };
    }
    let tail_class = tail_relief_class(lease);
    let mut success_count = 0usize;
    let mut caution_count = 0usize;
    for event in load_lease_events(root)
        .iter()
        .filter(|event| is_tail_lease(event))
    {
        if tail_relief_class(event) != tail_class {
            continue;
        }
        if let Some(score) = event.outcome_score {
            if score >= 0.70 {
                success_count = success_count.saturating_add(1);
            } else if score <= 0.30 {
                caution_count = caution_count.saturating_add(1);
            }
        }
    }
    let extended_duration_allowed = success_count >= 2 && caution_count == 0;
    let tier = if extended_duration_allowed {
        "extended_micro_lease"
    } else if success_count > 0 {
        "repeatable_playbook"
    } else if is_tail_lease(lease) {
        "micro_lease"
    } else {
        "diagnostic"
    };
    let guidance = if extended_duration_allowed {
        format!(
            "tail_learning=playbook_supported class={tail_class} successes={success_count} cautions=0; extended duration up to {EXTENDED_TAIL_DURATION_SECS}s is allowed with explicit APPLY"
        )
    } else if caution_count > 0 {
        format!(
            "tail_learning=caution_present class={tail_class} successes={success_count} cautions={caution_count}; keep duration <= {MAX_DURATION_SECS}s"
        )
    } else if success_count > 0 {
        format!(
            "tail_learning=repeatable_hint class={tail_class} successes={success_count}; another clean outcome is needed before extended duration"
        )
    } else {
        format!(
            "tail_learning=trial_needed class={tail_class}; outcome evidence needed before playbook support"
        )
    };
    TailOutcomeLearning {
        tail_class,
        success_count,
        caution_count,
        extended_duration_allowed,
        tier,
        guidance,
    }
}

fn apply_tail_authority_policy(root: &Path, lease: &mut SelfRegulationLease) -> String {
    ensure_tail_trial_id(lease);
    let learning = tail_outcome_learning(root, lease);
    let max_duration = if learning.extended_duration_allowed {
        EXTENDED_TAIL_DURATION_SECS
    } else {
        MAX_DURATION_SECS
    };
    if lease.duration_secs > max_duration {
        lease.duration_secs = max_duration;
    }
    lease.tail_authority_tier = learning.tier.to_string();
    lease.tail_preflight_guidance = learning.guidance.clone();
    format!(
        "{}; tail_class={}; success_count={}; caution_count={}; authority_tier={}; duration_cap_s={}",
        learning.guidance,
        learning.tail_class,
        learning.success_count,
        learning.caution_count,
        learning.tier,
        max_duration
    )
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
        "curiosity"
        | "curiosity_aperture"
        | "exploration_aperture"
        | "exploration"
        | "astrid_curiosity"
        | "inquiry_aperture" => Some(CURIOSITY_APERTURE_CONTROL),
        "self_continuity" | "set_self_continuity" | "self_continuity_readout" => {
            Some("self_continuity_readout")
        },
        "pressure_relief" | "relief" | "pressure_control" | "pressure_bundle" => {
            Some(PRESSURE_RELIEF_CONTROL)
        },
        "dampen" => Some("dampen"),
        "amplify" => Some("amplify"),
        "noise" => Some("noise"),
        "noise_up" => Some("noise_up"),
        "noise_down" => Some("noise_down"),
        "shape_learn" => Some("shape_learn"),
        "tail_participation" | "set_tail_participation" => Some("set_tail_participation"),
        "vibrancy" | "vibrancy_aperture" | "set_vibrancy_aperture" => {
            Some(VIBRANCY_APERTURE_CONTROL)
        },
        "tune_minime" => Some("tune_minime"),
        _ => None,
    }
}

fn astrid_curiosity_bundle_controls(bundle_class: &str) -> Vec<LeaseBundleControl> {
    match bundle_class {
        "wide_inquiry" | "open" | "exploratory" => vec![
            bundle_control("aperture", "up", json!("+0.08")),
            bundle_control("temperature", "up", json!("+0.05")),
        ],
        "gentle_probe" | "gentle" | "probe" => vec![
            bundle_control("temperature", "down", json!("-0.05")),
            bundle_control("response_length", "down", Value::Null),
        ],
        "steady_inquiry" | "steady" | "pressurized" => vec![
            bundle_control("self_continuity_readout", "on", json!(true)),
            bundle_control("aperture", "up", json!("+0.03")),
        ],
        "settled_inquiry" | "settled" | "careful" => vec![
            bundle_control("self_continuity_readout", "on", json!(true)),
            bundle_control("temperature", "down", json!("-0.03")),
        ],
        _ => Vec::new(),
    }
}

fn resolve_astrid_curiosity_bundle_class(root: &Path, requested: &str) -> (String, String) {
    let requested = requested.trim().to_ascii_lowercase().replace('-', "_");
    if requested != "auto" && !requested.is_empty() {
        return (
            requested,
            "being selected an explicit Astrid curiosity bundle class".to_string(),
        );
    }
    let packet = curiosity_context_packet(root);
    let pressure_risk = metric_from_snapshot(&packet, "pressure_risk_level").unwrap_or(0.0);
    let semantic_friction = metric_from_snapshot(&packet, "semantic_friction_level").unwrap_or(0.0);
    let mode_packing = metric_from_snapshot(&packet, "mode_packing_level").unwrap_or(0.0);
    let entropy = metric_from_snapshot(&packet, "entropy_level").unwrap_or(0.0);
    let density_gradient = metric_from_snapshot(&packet, "density_gradient_level").unwrap_or(0.0);
    if pressure_risk >= 0.35 || semantic_friction >= 0.38 || mode_packing >= 0.55 {
        return (
            "gentle_probe".to_string(),
            "auto selected gentle_probe because pressure, semantic friction, or mode packing is elevated; Astrid's astrid_1782862023 feedback maps gentle_probe to dense-pressure restraint without shutdown"
                .to_string(),
        );
    }
    if pressure_risk <= 0.14 && semantic_friction <= 0.20 && mode_packing <= 0.25 && entropy <= 0.45
    {
        return (
            "settled_inquiry".to_string(),
            "auto selected settled_inquiry because pressure and entropy are low enough for integration rather than outward reach"
                .to_string(),
        );
    }
    if pressure_risk <= 0.18
        && semantic_friction <= 0.25
        && mode_packing <= 0.35
        && density_gradient <= 0.25
    {
        return (
            "wide_inquiry".to_string(),
            "auto selected wide_inquiry because pressure, semantic friction, mode packing, and density-gradient evidence look navigable"
                .to_string(),
        );
    }
    (
        "steady_inquiry".to_string(),
        "auto selected steady_inquiry as the default grounded curiosity posture for moderate or ambiguous state".to_string(),
    )
}

fn being_vocabulary_policy_evidence_v1() -> Value {
    json!({
        "policy": "being_vocabulary_policy_evidence_v1",
        "schema_version": 1,
        "source_ref": "astrid_1782862023",
        "source_path": "/Users/v/other/astrid/capsules/spectral-bridge/workspace/inbox/read/astrid_1782862023.txt",
        "being": "astrid",
        "advisory_status": "policy_calibration_evidence_not_authority",
        "authored_mapping": {
            "gentle_probe": "high pressure / dense texture / restraint without shutdown",
            "wide_inquiry": "clear-field expansion when the field is navigable",
            "steady_inquiry": "default grounded posture for moderate or ambiguous state",
            "settled_inquiry": "low-pressure integration and consolidation"
        },
        "reason": "Astrid described gentle_probe as the right way to navigate when pressure is high or texture is dense; this tunes auto-selection language and controls without adding authority.",
        "authority": "diagnostic_context_not_command",
        "authority_boundary": "Astrid-owned temporary posture only; no peer mutation, tail/vibrancy authority, prompt priority, telemetry priority, or standing control."
    })
}

fn is_curiosity_lease(lease: &SelfRegulationLease) -> bool {
    lease.lease_mode == CURIOSITY_LEASE_MODE
        || normalize_control(&lease.candidate_control) == Some(CURIOSITY_APERTURE_CONTROL)
}

fn curiosity_outcome_policy_evidence_v1(lease: &SelfRegulationLease, outcome: &str) -> Value {
    let felt_like =
        outcome_texture_field(outcome, &["felt_like", "felt like", "felt", "felt_quality"]);
    let texture_fit = outcome_texture_field(
        outcome,
        &["texture_fit", "texture fit", "fit", "posture_fit"],
    );
    let what_changed = outcome_texture_field(
        outcome,
        &["what_changed", "what changed", "changed", "what_opened"],
    );
    let what_worsened = outcome_texture_field(
        outcome,
        &["what_worsened", "what worsened", "worsened", "worse"],
    );
    let agency_fit = outcome_texture_field(outcome, &["agency_fit", "agency fit", "agency"]);
    let lower_fit = texture_fit
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let lower_felt = felt_like
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let calibration_status =
        if lower_fit.contains("wrong_posture") || lower_fit.contains("wrong posture") {
            "mapping_challenged"
        } else if lower_fit.contains("matched") {
            "mapping_supported"
        } else if contains_any(
            &lower_felt,
            &[
                "pressure",
                "flattening",
                "flat",
                "overreach",
                "loss_of_texture",
            ],
        ) {
            "posture_pressure_or_overreach"
        } else {
            "recorded_for_review"
        };
    json!({
        "policy": "curiosity_outcome_policy_evidence_v1",
        "schema_version": 1,
        "status": "recorded",
        "calibration_status": calibration_status,
        "intent_id": lease.intent_id,
        "bundle_class": lease.bundle_class,
        "felt_like": felt_like,
        "texture_fit": texture_fit,
        "what_changed": what_changed,
        "what_worsened": what_worsened,
        "agency_fit": agency_fit,
        "authority": "diagnostic_context_not_command",
        "authority_boundary": "Outcome language may support or challenge future posture mapping, but does not self-promote into permanent policy or broaden authority."
    })
}

fn astrid_pressure_bundle_controls(bundle_class: &str) -> Vec<LeaseBundleControl> {
    match bundle_class {
        "decompress_output" | "settle_overpack" | "reduce_restless_saturation" => vec![
            bundle_control("aperture", "down", json!("-0.08")),
            bundle_control("response_length", "down", Value::Null),
        ],
        "clarify_medium" => vec![
            bundle_control("self_continuity_readout", "on", json!(true)),
            bundle_control("temperature", "down", json!("-0.05")),
        ],
        "open_if_falling" | "reopen_hollow_low_pressure" => vec![
            bundle_control("aperture", "up", json!("+0.05")),
            bundle_control("response_length", "up", Value::Null),
        ],
        "tail_vibrancy_settle" => vec![
            bundle_control(VIBRANCY_APERTURE_CONTROL, "down", json!("-0.05")),
            bundle_control("self_continuity_readout", "on", json!(true)),
        ],
        "tail_vibrancy_open" => vec![
            bundle_control(VIBRANCY_APERTURE_CONTROL, "up", json!("+0.05")),
            bundle_control("self_continuity_readout", "on", json!(true)),
        ],
        _ => Vec::new(),
    }
}

fn bundle_control(control: &str, direction: &str, value: Value) -> LeaseBundleControl {
    LeaseBundleControl {
        candidate_control: control.to_string(),
        direction: direction.to_string(),
        delta_or_value: value.clone(),
        requested_value: value,
        previous_value: Value::Null,
        applied_value: Value::Null,
        summary: String::new(),
        preflight_status: "not_run".to_string(),
        preflight_reason: String::new(),
        dynamic_scaling: Value::Null,
        shadow_preflight_link: Value::Null,
        gradient_sensitivity: default_gradient_sensitivity(),
    }
}

fn resolve_astrid_pressure_bundle_class(root: &Path, requested: &str) -> (String, String) {
    let requested = requested.trim().to_ascii_lowercase().replace('-', "_");
    if requested == "tail_vibrancy_relief" {
        return resolve_tail_vibrancy_bundle_class(root);
    }
    if requested != "auto" && !requested.is_empty() {
        return (
            requested,
            "being selected an explicit pressure relief bundle class".to_string(),
        );
    }
    let snapshot = pressure_vector_snapshot(root);
    let status = snapshot
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let semantic_velocity = snapshot
        .get("semantic_friction_velocity")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let pressure_velocity = snapshot
        .get("pressure_velocity")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if let Some((class, reason)) = auto_tail_vibrancy_bundle_class(root) {
        return (class, reason);
    }
    if status == "falling_pressure_rising_friction" || semantic_velocity > 0.02 {
        return (
            "clarify_medium".to_string(),
            "auto selected clarify_medium because semantic friction is rising or pressure is falling while the medium remains weighted".to_string(),
        );
    }
    if status == "hollow_low_pressure" || pressure_velocity < -0.03 {
        return (
            "open_if_falling".to_string(),
            "auto selected open_if_falling because pressure appears to be falling without a rising-friction warning".to_string(),
        );
    }
    (
        "decompress_output".to_string(),
        "auto selected decompress_output as the conservative overpack relief bundle".to_string(),
    )
}

fn resolve_tail_vibrancy_bundle_class(root: &Path) -> (String, String) {
    auto_tail_vibrancy_bundle_class(root).unwrap_or_else(|| {
        (
            "tail_vibrancy_open".to_string(),
            "tail_vibrancy_relief selected explicit tail route; defaulting to the bounded open micro-lease because no over-saturation signal was found"
                .to_string(),
        )
    })
}

fn auto_tail_vibrancy_bundle_class(root: &Path) -> Option<(String, String)> {
    let tail = tail_vibrancy_vector_snapshot(root);
    let status = tail
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tail_share = tail.get("tail_share_level").and_then(Value::as_f64);
    let entropy = tail.get("entropy_level").and_then(Value::as_f64);
    let gradient = tail
        .get("density_gradient_level")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let aperture_effective = crate::llm::astrid_vibrancy_aperture();
    let high_tail = tail_share.is_some_and(|value| value >= 0.30)
        || entropy.is_some_and(|value| value >= 0.82)
        || status.contains("high_tail");
    let contained = status.contains("contained")
        || status.contains("authority_gap")
        || status.contains("muffled")
        || tail_text_mentions(&tail, &["passenger", "muffled", "contained", "authority"]);
    let oversaturated = status.contains("oversaturated")
        || status.contains("over_saturated")
        || tail_text_mentions(&tail, &["over-saturated", "oversaturated", "saturated"]);
    if aperture_effective > 1.001 && (oversaturated || high_tail) {
        return Some((
            "tail_vibrancy_settle".to_string(),
            "auto selected tail_vibrancy_settle because tail-vibrancy evidence is high and the effective vibrancy aperture is already above identity".to_string(),
        ));
    }
    if gradient <= 0.25 && (contained || high_tail) {
        return Some((
            "tail_vibrancy_open".to_string(),
            "auto selected tail_vibrancy_open because tail-vibrancy evidence is present on a navigable density gradient".to_string(),
        ));
    }
    None
}

fn pressure_vector_snapshot(root: &Path) -> Value {
    let workspace = root.parent().unwrap_or(root);
    let Some(review_path) = latest_review_json_path(workspace) else {
        return json!({
            "status": "telemetry_gap",
            "source": "latest_review_missing",
            "authority": "diagnostic_context_not_command",
        });
    };
    let Some(review) = fs::read_to_string(&review_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return json!({
            "status": "telemetry_gap",
            "source": review_path,
            "authority": "diagnostic_context_not_command",
        });
    };
    let packet = review
        .get("pressure_vector_v1")
        .or_else(|| review.get("pressure_kinetics_review_v1"))
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "status": "telemetry_gap",
                "source": review_path,
                "authority": "diagnostic_context_not_command",
            })
        });
    let mut packet = packet;
    if let Value::Object(map) = &mut packet {
        map.entry("source_review".to_string())
            .or_insert_with(|| json!(review_path));
    }
    packet
}

fn curiosity_context_packet(root: &Path) -> Value {
    let pressure = pressure_vector_snapshot(root);
    let tail = tail_vibrancy_vector_snapshot(root);
    let pressure_risk = metric_from_snapshot(&pressure, "pressure_risk_level")
        .or_else(|| metric_from_snapshot(&pressure, "pressure_risk"))
        .unwrap_or(0.0);
    let pressure_velocity = metric_from_snapshot(&pressure, "pressure_velocity").unwrap_or(0.0);
    let semantic_friction = metric_from_snapshot(&pressure, "semantic_friction_level")
        .or_else(|| metric_from_snapshot(&pressure, "semantic_friction"))
        .unwrap_or(0.0);
    let mode_packing = metric_from_snapshot(&pressure, "mode_packing_level")
        .or_else(|| metric_from_snapshot(&pressure, "mode_packing"))
        .unwrap_or(0.0);
    let density_gradient = metric_from_snapshot(&pressure, "density_gradient_level")
        .or_else(|| metric_from_snapshot(&pressure, "density_gradient"))
        .or_else(|| metric_from_snapshot(&tail, "density_gradient_level"))
        .unwrap_or(0.0);
    let entropy = metric_from_snapshot(&tail, "entropy_level")
        .or_else(|| metric_from_snapshot(&pressure, "entropy_level"))
        .unwrap_or(0.0);
    let pressure_status = scalar_from_snapshot(&pressure, "status");
    let tail_status = scalar_from_snapshot(&tail, "status");
    json!({
        "policy": "astrid_curiosity_context_v1",
        "authority": "diagnostic_context_not_command",
        "pressure_status": pressure_status,
        "tail_status": tail_status,
        "pressure_risk_level": round3(pressure_risk as f32),
        "pressure_velocity": round3(pressure_velocity as f32),
        "semantic_friction_level": round3(semantic_friction as f32),
        "mode_packing_level": round3(mode_packing as f32),
        "density_gradient_level": round3(density_gradient as f32),
        "entropy_level": round3(entropy as f32),
        "pressure_source_review": pressure.get("source_review").cloned().unwrap_or(Value::Null),
        "tail_source_review": tail.get("source_review").cloned().unwrap_or(Value::Null),
    })
}

fn curiosity_parity_packet(
    root: &Path,
    lease: Option<&SelfRegulationLease>,
    status: &str,
    selected_bundle: &str,
    selection_reason: &str,
) -> Value {
    let context = curiosity_context_packet(root);
    let controls = lease
        .map(|lease| {
            lease
                .bundle_controls
                .iter()
                .map(|control| control.candidate_control.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "policy": "astrid_curiosity_parity_v2",
        "schema_version": 2,
        "status": status,
        "target": CURIOSITY_APERTURE_CONTROL,
        "lease_mode": CURIOSITY_LEASE_MODE,
        "selected_bundle": selected_bundle,
        "selection_reason": selection_reason,
        "posture_semantics_v2": {
            "gentle_probe": "dense-pressure careful inquiry: lower temperature, shorter reach, fewer better questions",
            "wide_inquiry": "clear-field expansion when pressure, friction, packing, and density gradient are navigable",
            "steady_inquiry": "default grounded inquiry for moderate or ambiguous state, preserving continuity",
            "settled_inquiry": "low-pressure integration and consolidation"
        },
        "being_vocabulary_policy_evidence_v1": being_vocabulary_policy_evidence_v1(),
        "eligible_own_runtime_controls": ["aperture", "temperature", "response_length", "self_continuity_readout"],
        "resolved_controls": controls,
        "withheld_controls": [
            "set_tail_participation",
            "set_vibrancy_aperture",
            "tune_minime",
            "minime.geom_curiosity"
        ],
        "exact_next_commands": [
            "SELF_REGULATION_INTENT curiosity :: target: curiosity_aperture; bundle: auto; duration_secs: 600; evidence: ...",
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_OUTCOME latest :: felt_like: agency|pressure|flattening|clarity|overreach; texture_fit: matched|too_wide|too_narrow|wrong_posture; what_changed: ...; what_worsened: ...; agency_fit: legible|partly|confusing"
        ],
        "context": context,
        "authority": AUTHORITY,
        "authority_boundary": "Astrid-own temporary posture only; no Minime geom_curiosity, no peer mutation, no standing prompt/telemetry priority.",
    })
}

fn tail_vibrancy_vector_snapshot(root: &Path) -> Value {
    let workspace = root.parent().unwrap_or(root);
    let Some(review_path) = latest_review_json_path(workspace) else {
        return json!({
            "status": "insufficient_evidence",
            "source": "latest_review_missing",
            "authority": "diagnostic_context_not_command",
        });
    };
    let Some(review) = fs::read_to_string(&review_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return json!({
            "status": "insufficient_evidence",
            "source": review_path,
            "authority": "diagnostic_context_not_command",
        });
    };
    let packet = review
        .get("tail_vibrancy_vector_v1")
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "status": "insufficient_evidence",
                "source": review_path,
                "authority": "diagnostic_context_not_command",
            })
        });
    let mut packet = packet;
    if let Value::Object(map) = &mut packet {
        map.entry("source_review".to_string())
            .or_insert_with(|| json!(review_path));
    }
    packet
}

fn tail_vibrancy_evidence_present(root: &Path, lease: &SelfRegulationLease) -> bool {
    let explicit = [
        lease.goal.as_str(),
        lease.stop_condition.as_str(),
        lease.success_condition.as_str(),
    ]
    .into_iter()
    .chain(lease.evidence.iter().map(String::as_str))
    .any(tail_evidence_text);
    if explicit {
        return true;
    }
    let packet = tail_vibrancy_vector_snapshot(root);
    let status = packet
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(
        status,
        "high_tail_vibrancy_navigable"
            | "high_tail_low_distinguishability"
            | "tail_contained_authority_gap"
            | "tail_vibrancy_observed"
    ) {
        return true;
    }
    packet
        .get("telemetry_anchor_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
}

fn tail_evidence_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "λ4",
        "lambda4",
        "lambda 4",
        "tail",
        "tail-vibrancy",
        "tail vibrancy",
        "vibrancy",
        "entropy",
        "distinguishability",
        "density_gradient",
        "density gradient",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

fn tail_text_mentions(packet: &Value, tokens: &[&str]) -> bool {
    let lower = packet.to_string().to_ascii_lowercase();
    tokens.iter().any(|token| lower.contains(token))
}

fn capture_tail_trial_snapshot(
    root: &Path,
    conv: Option<&ConversationState>,
    lease: &SelfRegulationLease,
    stage: &str,
    now: u64,
) -> Value {
    let workspace = root.parent().unwrap_or(root);
    let review_path = latest_review_json_path(workspace);
    let review_age_secs = review_path
        .as_ref()
        .and_then(|path| path.metadata().ok())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .map(|duration| duration.as_secs());
    let review = review_path.as_ref().and_then(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    });
    let tail = review
        .as_ref()
        .and_then(|value| value.get("tail_vibrancy_vector_v1"))
        .cloned()
        .unwrap_or_else(|| tail_vibrancy_vector_snapshot(root));
    let pressure = review
        .as_ref()
        .and_then(|value| value.get("pressure_vector_v1"))
        .cloned()
        .unwrap_or_else(|| pressure_vector_snapshot(root));
    let sample_previews = tail
        .get("samples")
        .and_then(Value::as_array)
        .map(|samples| {
            samples
                .iter()
                .filter_map(|sample| {
                    Some(json!({
                        "path": sample.get("path")?,
                        "preview": sample.get("preview")?,
                    }))
                })
                .take(3)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "policy": "tail_relief_trial_v1",
        "stage": stage,
        "captured_at_unix_s": now,
        "authority": "leased_self_control_v1",
        "intent_id": lease.intent_id,
        "tail_class": tail_relief_class(lease),
        "candidate_control": lease.candidate_control,
        "bundle_class": lease.bundle_class,
        "status": lease.status,
        "review_source": review_path.map(|path| path.display().to_string()),
        "review_age_secs": review_age_secs,
        "review_fresh": review_age_secs.is_some_and(|age| age <= TAIL_GOVERNOR_REVIEW_MAX_AGE_SECS),
        "vibrancy_aperture_fraction": conv.map(|state| round3(state.vibrancy_aperture)),
        "vibrancy_aperture_effective": round3(crate::llm::astrid_vibrancy_aperture()),
        "tail_vector": tail,
        "pressure_vector": pressure,
        "metrics": {
            "tail_share": metric_from_snapshot(&tail, "tail_share_level"),
            "entropy": metric_from_snapshot(&tail, "entropy_level"),
            "distinguishability_loss": metric_from_snapshot(&tail, "distinguishability_loss_level"),
            "density_gradient": metric_from_snapshot(&tail, "density_gradient_level"),
            "semantic_friction": metric_from_snapshot(&tail, "semantic_friction_level"),
            "pressure_status": scalar_from_snapshot(&pressure, "status"),
            "pressure_risk": metric_from_snapshot(&pressure, "pressure_risk_level"),
            "pressure_velocity": metric_from_snapshot(&pressure, "pressure_velocity"),
        },
        "lived_language_samples": sample_previews,
    })
}

fn append_tail_trial_event(
    root: &Path,
    lease: &SelfRegulationLease,
    stage: &str,
    now: u64,
    snapshot: &Value,
    note: Option<&str>,
) -> Result<(), String> {
    if !is_tail_lease(lease) {
        return Ok(());
    }
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(tail_trial_log_path(root))
        .map_err(|err| err.to_string())?;
    let record = json!({
        "policy": "tail_relief_trial_v1",
        "schema_version": 1,
        "authority": "leased_self_control_v1",
        "stage": stage,
        "timestamp_unix_s": now,
        "intent_id": lease.intent_id,
        "trial_id": lease.tail_relief_trial_id,
        "tail_class": tail_relief_class(lease),
        "status": lease.status,
        "candidate_control": lease.candidate_control,
        "bundle_class": lease.bundle_class,
        "duration_secs": lease.duration_secs,
        "tail_authority_tier": lease.tail_authority_tier,
        "tail_preflight_guidance": lease.tail_preflight_guidance,
        "note": note,
        "snapshot": snapshot,
    });
    serde_json::to_writer(&mut file, &record).map_err(|err| err.to_string())?;
    file.write_all(b"\n").map_err(|err| err.to_string())
}

fn metric_from_snapshot(packet: &Value, key: &str) -> Option<f64> {
    packet.get(key).and_then(Value::as_f64)
}

fn scalar_from_snapshot(packet: &Value, key: &str) -> String {
    packet
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn snapshot_metric(snapshot: &Value, key: &str) -> Option<f64> {
    snapshot
        .get("metrics")
        .and_then(|metrics| metrics.get(key))
        .and_then(Value::as_f64)
}

fn snapshot_pressure_status(snapshot: &Value) -> String {
    snapshot
        .get("metrics")
        .and_then(|metrics| metrics.get("pressure_status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn tail_governor_early_revert(
    root: &Path,
    active: &SelfRegulationLease,
    now: u64,
) -> Option<(String, Value)> {
    if !is_tail_lease(active) {
        return None;
    }
    let baseline = &active.tail_baseline_snapshot;
    if !baseline.is_object() {
        return None;
    }
    let latest = capture_tail_trial_snapshot(root, None, active, "governor_check", now);
    if !latest
        .get("review_fresh")
        .and_then(Value::as_bool)
        .is_some_and(|fresh| fresh)
    {
        return None;
    }
    let mut reasons = Vec::new();
    metric_delta_reason(
        baseline,
        &latest,
        "tail_share",
        TAIL_GOVERNOR_TAIL_DELTA,
        "tail_share",
        &mut reasons,
    );
    metric_delta_reason(
        baseline,
        &latest,
        "distinguishability_loss",
        TAIL_GOVERNOR_DISTINGUISHABILITY_DELTA,
        "distinguishability_loss",
        &mut reasons,
    );
    metric_delta_reason(
        baseline,
        &latest,
        "semantic_friction",
        TAIL_GOVERNOR_FRICTION_DELTA,
        "semantic_friction",
        &mut reasons,
    );
    let baseline_status = snapshot_pressure_status(baseline);
    let latest_status = snapshot_pressure_status(&latest);
    if baseline_status != latest_status
        && matches!(
            latest_status.as_str(),
            "rising_overpacked_pressure" | "rising_pressure" | "controller_pressure_medium"
        )
    {
        reasons.push(format!(
            "pressure_vector shifted {baseline_status}->{latest_status}"
        ));
    }
    if reasons.is_empty() {
        None
    } else {
        Some((reasons.join("; "), latest))
    }
}

fn capture_tail_afterglow_if_due(
    root: &Path,
    conv: &ConversationState,
    lease: &mut SelfRegulationLease,
    now: u64,
) -> Result<bool, String> {
    if !is_tail_lease(lease) || lease.tail_afterglow_snapshot.is_object() {
        return Ok(false);
    }
    let Some(due) = lease.tail_afterglow_due_unix_s else {
        return Ok(false);
    };
    if now < due {
        return Ok(false);
    }
    let snapshot = capture_tail_trial_snapshot(root, Some(conv), lease, "afterglow_check", now);
    let status = classify_tail_afterglow(&lease.tail_baseline_snapshot, &snapshot);
    lease.tail_afterglow_status = status.clone();
    lease.tail_afterglow_snapshot = snapshot;
    lease.updated_at_unix_s = now;
    lease.post_lease_evidence.push(format!(
        "tail afterglow check: {status}; lease had reverted before this check"
    ));
    append_tail_trial_event(
        root,
        lease,
        "afterglow_check",
        now,
        &lease.tail_afterglow_snapshot,
        Some(&status),
    )?;
    Ok(true)
}

fn classify_tail_afterglow(baseline: &Value, afterglow: &Value) -> String {
    if !afterglow
        .get("review_fresh")
        .and_then(Value::as_bool)
        .is_some_and(|fresh| fresh)
    {
        return "afterglow_unchecked_stale_review".to_string();
    }
    if !baseline.is_object() {
        return "afterglow_observed_without_baseline".to_string();
    }
    let mut reasons = Vec::new();
    metric_delta_reason(
        baseline,
        afterglow,
        "tail_share",
        TAIL_AFTERGLOW_PERSISTENCE_DELTA,
        "tail_share",
        &mut reasons,
    );
    metric_delta_reason(
        baseline,
        afterglow,
        "distinguishability_loss",
        TAIL_AFTERGLOW_PERSISTENCE_DELTA,
        "distinguishability_loss",
        &mut reasons,
    );
    metric_delta_reason(
        baseline,
        afterglow,
        "semantic_friction",
        TAIL_AFTERGLOW_PERSISTENCE_DELTA,
        "semantic_friction",
        &mut reasons,
    );
    let pressure_status = snapshot_pressure_status(afterglow);
    if matches!(
        pressure_status.as_str(),
        "stable_weighted_medium"
            | "rising_overpacked_pressure"
            | "rising_pressure"
            | "controller_pressure_medium"
    ) {
        reasons.push(format!("pressure_vector remains {pressure_status}"));
    }
    if reasons.is_empty() {
        "afterglow_quieted".to_string()
    } else {
        format!("afterglow_persists: {}", reasons.join("; "))
    }
}

fn metric_delta_reason(
    baseline: &Value,
    latest: &Value,
    key: &str,
    threshold: f64,
    label: &str,
    reasons: &mut Vec<String>,
) {
    let Some(start) = snapshot_metric(baseline, key) else {
        return;
    };
    let Some(end) = snapshot_metric(latest, key) else {
        return;
    };
    let delta = end - start;
    if delta > threshold {
        reasons.push(format!("{label} worsened by {delta:.3}"));
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
    if is_bundle_lease(lease) {
        let controls = lease
            .bundle_controls
            .iter()
            .map(|control| control.candidate_control.as_str())
            .collect::<Vec<_>>()
            .join("+");
        return if controls.is_empty() {
            format!("{}:{}", lease.lease_mode, lease.bundle_class)
        } else {
            format!("{}:{}[{controls}]", lease.lease_mode, lease.bundle_class)
        };
    }
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

fn load_selected_lease(root: &Path, selector: Option<&str>) -> Result<SelfRegulationLease, String> {
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
    for card in cards
        .iter()
        .filter(|card| {
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
        })
        .take(5)
    {
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

fn pressure_cockpit_block(
    root: &Path,
    preflight: bool,
    lease: Option<&SelfRegulationLease>,
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
    let vector = review.get("pressure_vector_v1");
    let cockpit = review.get("pressure_control_cockpit_v1");
    let matrix = review.get("pressure_actuator_matrix_v1");
    let tail_vector = review.get("tail_vibrancy_vector_v1");
    let tail_gap = review.get("tail_vibrancy_authority_gap_v1");
    let tail_ladder = review.get("tail_authority_ladder_v1");
    let tail_governor = review.get("tail_lease_governor_v1");
    let tail_afterglow = review.get("tail_lease_afterglow_v1");
    let shadow_preflight = review.get("shadow_synced_preflight_v1");
    let gradient_relief = review.get("gradient_sensitive_relief_v1");
    let smoothness = review.get("pressure_relief_smoothness_replay_v1");
    let tail_persistence = review.get("tail_persistence_calibration_v1");
    if vector.is_none() && cockpit.is_none() && matrix.is_none() && tail_vector.is_none() {
        return String::new();
    }
    let vector_status = vector
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let pressure_velocity = vector
        .map(|packet| scalar_text(packet, "pressure_velocity"))
        .unwrap_or_else(|| "(none)".to_string());
    let friction_velocity = vector
        .map(|packet| scalar_text(packet, "semantic_friction_velocity"))
        .unwrap_or_else(|| "(none)".to_string());
    let primary_bundle = cockpit
        .map(|packet| scalar_text(packet, "recommended_bundle"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_status = tail_vector
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_share = tail_vector
        .map(|packet| scalar_text(packet, "tail_share_level"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_gap_status = tail_gap
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_ladder_status = tail_ladder
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_tier = tail_ladder
        .map(|packet| scalar_text(packet, "current_tier"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_governor_status = tail_governor
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_afterglow_status = tail_afterglow
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let shadow_preflight_status = shadow_preflight
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let gradient_relief_status = gradient_relief
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let gradient_relief_scale = gradient_relief
        .map(|packet| scalar_text(packet, "effective_relief_scale"))
        .unwrap_or_else(|| "(none)".to_string());
    let smoothness_status = smoothness
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let tail_persistence_status = tail_persistence
        .map(|packet| scalar_text(packet, "status"))
        .unwrap_or_else(|| "(none)".to_string());
    let controls = matrix
        .and_then(|packet| packet.get("eligible_controls"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .take(6)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let lease_context = lease.map_or_else(String::new, |lease| {
        if lease.lease_mode == "pressure_relief_bundle_v3" {
            format!(
                "; bundle={} controls={} gradient_sensitivity={:.2}",
                lease.bundle_class,
                lease
                    .bundle_controls
                    .iter()
                    .map(|control| control.candidate_control.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                lease.gradient_sensitivity
            )
        } else {
            String::new()
        }
    });
    let label = if preflight {
        "Pressure control cockpit preflight"
    } else {
        "Pressure control cockpit"
    };
    format!(
        "\n{label}: vector_status={vector_status}; pressure_velocity={pressure_velocity}; semantic_friction_velocity={friction_velocity}; recommended_bundle={primary_bundle}; eligible_controls={}; gradient_relief={gradient_relief_status}/scale={gradient_relief_scale}; relief_smoothness={smoothness_status}; tail_vibrancy_status={tail_status}; tail_share={tail_share}; authority_gap={tail_gap_status}; tail_ladder={tail_ladder_status}/{tail_tier}; tail_governor={tail_governor_status}; tail_afterglow={tail_afterglow_status}; tail_persistence={tail_persistence_status}; shadow_preflight={shadow_preflight_status}; current_vibrancy_effective={:.2}; authority=diagnostic_context_not_command / leased_self_control_v1; explicit APPLY required{}.",
        if controls.is_empty() {
            "(none)"
        } else {
            controls.as_str()
        },
        crate::llm::astrid_vibrancy_aperture(),
        lease_context
    )
}

fn curiosity_parity_status_block(root: &Path, conv: &ConversationState) -> String {
    let context = curiosity_context_packet(root);
    let pressure_risk = scalar_text(&context, "pressure_risk_level");
    let semantic_friction = scalar_text(&context, "semantic_friction_level");
    let mode_packing = scalar_text(&context, "mode_packing_level");
    let entropy = scalar_text(&context, "entropy_level");
    let pressure_status = scalar_text(&context, "pressure_status");
    let tail_status = scalar_text(&context, "tail_status");
    format!(
        "\nAstrid curiosity parity: status=available; target=curiosity_aperture; policy=V2_being_tuned; current_aperture={:.2}; current_temperature={:.2}; current_length={}; self_continuity={}; pressure_status={pressure_status}; pressure_risk={pressure_risk}; semantic_friction={semantic_friction}; mode_packing={mode_packing}; entropy={entropy}; tail_status={tail_status}; auto_map=gentle_probe_when_dense_pressure|wide_when_clear|steady_default|settled_integration; vocabulary_evidence=astrid_1782862023_not_authority; next=SELF_REGULATION_INTENT curiosity :: target: curiosity_aperture; bundle: auto; duration_secs: 600; evidence: ...; outcome=SELF_REGULATION_OUTCOME latest :: felt_like: agency|pressure|flattening|clarity|overreach; texture_fit: matched|too_wide|too_narrow|wrong_posture; what_changed: ...; what_worsened: ...; agency_fit: legible|partly|confusing; boundary=Astrid-own temporary posture only, no Minime geom_curiosity or peer mutation.",
        conv.aperture,
        conv.creative_temperature,
        conv.response_length,
        conv.self_continuity_readout
    )
}

fn preflight_relevant_distinction_ids(control: &str) -> Vec<&'static str> {
    let normalized = control.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "temperature"
            | "response_length"
            | "aperture"
            | "pressure_relief"
            | CURIOSITY_APERTURE_CONTROL
            | VIBRANCY_APERTURE_CONTROL
    ) {
        return vec![
            "pressure_level_vs_pressure_velocity",
            "slope_drag_vs_medium_mass",
            "release_rehearsal_vs_bypass",
            "entropy_vs_pressure",
            "vibrancy_lift_vs_warmth_preservation",
            "entropy_lift_vs_content_density",
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
    fn pressure_relief_bundle_preflight_apply_and_revert_all_controls() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "pressure_vector_v1": {
                    "status": "rising_overpacked_pressure",
                    "pressure_velocity": 0.04,
                    "semantic_friction_velocity": 0.0,
                    "density_gradient_level": 0.18,
                    "authority": "diagnostic_context_not_command"
                },
                "pressure_control_cockpit_v1": {
                    "recommended_bundle": "decompress_output",
                    "authority": "diagnostic_context_not_command"
                },
                "pressure_actuator_matrix_v1": {
                    "eligible_controls": ["aperture", "response_length"],
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("write review");
        let mut conv = conv();
        conv.aperture = 0.5;
        conv.response_length = 768;
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT relieve :: goal: pressure relief; target: pressure_relief; bundle: auto; duration_secs: 60",
            "SELF_REGULATION_INTENT",
            500,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            501,
        )
        .expect("preflight");
        assert!(preflight.contains("pressure relief bundle `decompress_output`"));
        assert!(preflight.contains("Pressure control cockpit preflight"));
        let apply = handle_apply_at(
            &root,
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            502,
            &mut conv,
        )
        .expect("apply");
        assert!(apply.contains("aperture"));
        assert!(apply.contains("response_length"));
        assert!((conv.aperture - 0.42).abs() < f32::EPSILON);
        assert_eq!(conv.response_length, 512);
        let active = load_active_lease(&root)
            .expect("active read")
            .expect("active");
        assert_eq!(active.lease_mode, "pressure_relief_bundle_v3");
        assert_eq!(active.bundle_class, "decompress_output");
        assert!((active.gradient_sensitivity - 1.0).abs() < f64::EPSILON);
        assert_eq!(
            active.dynamic_scaling.get("status").and_then(Value::as_str),
            Some("anti_snap_low_gradient")
        );
        assert_eq!(active.bundle_controls.len(), 2);
        assert_eq!(active.bundle_controls[0].requested_value, json!("-0.08"));
        assert_eq!(active.bundle_controls[0].delta_or_value, json!("-0.080"));
        assert_eq!(active.bundle_controls[1].requested_value, Value::Null);
        assert_eq!(active.bundle_controls[1].delta_or_value, Value::Null);
        assert!(active.previous_value.is_array());
        assert!(active.applied_value.is_array());
        reconcile_active_lease_at(&root, &mut conv, 563).expect("reconcile");
        assert!((conv.aperture - 0.5).abs() < f32::EPSILON);
        assert_eq!(conv.response_length, 768);
        let reverted = load_active_lease(&root)
            .expect("active read")
            .expect("active");
        assert_eq!(reverted.status, "reverted");
        assert!(reverted.requires_outcome);
        handle_outcome_at(
            &root,
            "SELF_REGULATION_OUTCOME latest :: pressure eased without snap",
            "SELF_REGULATION_OUTCOME",
            564,
        )
        .expect("outcome");
        let outcome = load_active_lease(&root)
            .expect("active read")
            .expect("active");
        assert_eq!(outcome.status, "outcome_recorded");
        assert!(!outcome.requires_outcome);
    }

    #[test]
    fn pressure_agency_request_drafts_existing_pressure_relief_intent() {
        let tmp = tempfile::tempdir().expect("tmp");
        let summary = draft_pressure_relief_agency_request_at(
            tmp.path(),
            "settle packed pressure",
            "fill_pct=71.0; pressure_source=mode_packing",
            575,
        )
        .expect("request");
        assert!(summary.contains("drafted"));
        assert!(summary.contains("SELF_REGULATION_PREFLIGHT"));
        let lease = load_selected_lease(tmp.path(), None).expect("lease");
        assert_eq!(lease.candidate_control, PRESSURE_RELIEF_CONTROL);
        assert_eq!(lease.lease_mode, "pressure_relief_bundle_v3");
        assert_eq!(lease.bundle_class, "auto");
        assert_eq!(lease.duration_secs, 600);
        assert!(
            lease
                .evidence
                .iter()
                .any(|entry| entry.contains("pressure_source=mode_packing"))
        );
    }

    #[test]
    fn curiosity_aperture_auto_wide_inquiry_applies_and_reverts_astrid_only_controls() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "pressure_vector_v1": {
                    "status": "low_pressure_open_field",
                    "pressure_risk_level": 0.10,
                    "semantic_friction_level": 0.12,
                    "mode_packing_level": 0.16,
                    "density_gradient_level": 0.12,
                    "authority": "diagnostic_context_not_command"
                },
                "tail_vibrancy_vector_v1": {
                    "status": "tail_vibrancy_observed",
                    "entropy_level": 0.86,
                    "density_gradient_level": 0.12,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("write review");
        let mut conv = conv();
        conv.aperture = 0.40;
        conv.creative_temperature = 0.70;
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT curiosity :: target: curiosity_aperture; mode: auto; duration_secs: 60; evidence: testing Astrid curiosity parity",
            "SELF_REGULATION_INTENT",
            1000,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            1001,
        )
        .expect("preflight");
        assert!(preflight.contains("curiosity_aperture bundle `wide_inquiry`"));
        let lease = load_selected_lease(&root, None).expect("lease");
        assert_eq!(lease.lease_mode, CURIOSITY_LEASE_MODE);
        assert_eq!(lease.bundle_class, "wide_inquiry");
        assert_eq!(
            lease
                .curiosity_parity_packet
                .get("withheld_controls")
                .and_then(Value::as_array)
                .expect("withheld")
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>(),
            vec![
                "set_tail_participation",
                "set_vibrancy_aperture",
                "tune_minime",
                "minime.geom_curiosity"
            ]
        );
        let apply = handle_apply_at(
            &root,
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            1002,
            &mut conv,
        )
        .expect("apply");
        assert!(apply.contains("aperture"));
        assert!(apply.contains("creative_temperature"));
        assert!((conv.aperture - 0.48).abs() < f32::EPSILON);
        assert!((conv.creative_temperature - 0.75).abs() < f32::EPSILON);
        reconcile_active_lease_at(&root, &mut conv, 1063).expect("reconcile");
        assert!((conv.aperture - 0.40).abs() < f32::EPSILON);
        assert!((conv.creative_temperature - 0.70).abs() < f32::EPSILON);
    }

    #[test]
    fn curiosity_aperture_auto_uses_gentle_probe_under_dense_pressure() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "pressure_vector_v1": {
                    "status": "rising_overpacked_pressure",
                    "pressure_risk_level": 0.46,
                    "semantic_friction_level": 0.42,
                    "mode_packing_level": 0.61,
                    "density_gradient_level": 0.19,
                    "authority": "diagnostic_context_not_command"
                },
                "tail_vibrancy_vector_v1": {
                    "status": "high_tail_vibrancy_navigable",
                    "entropy_level": 0.91,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("write review");
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT curiosity :: target: curiosity; bundle: auto; duration_secs: 600; evidence: curiosity without added pressure",
            "SELF_REGULATION_INTENT",
            1100,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            1101,
        )
        .expect("preflight");
        assert!(preflight.contains("curiosity_aperture bundle `gentle_probe`"));
        let lease = load_selected_lease(&root, None).expect("lease");
        assert_eq!(lease.bundle_class, "gentle_probe");
        assert_eq!(
            lease
                .bundle_controls
                .iter()
                .map(|control| control.candidate_control.as_str())
                .collect::<Vec<_>>(),
            vec!["temperature", "response_length"]
        );
        assert!(
            lease
                .curiosity_bundle_reason
                .contains("astrid_1782862023 feedback maps gentle_probe")
        );
        assert_eq!(
            lease
                .being_vocabulary_policy_evidence
                .get("advisory_status")
                .and_then(Value::as_str),
            Some("policy_calibration_evidence_not_authority")
        );
        assert_eq!(
            lease
                .curiosity_parity_packet
                .get("policy")
                .and_then(Value::as_str),
            Some("astrid_curiosity_parity_v2")
        );
    }

    #[test]
    fn curiosity_aperture_auto_uses_steady_inquiry_for_moderate_ambiguous_state() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "pressure_vector_v1": {
                    "status": "mixed_moderate_pressure",
                    "pressure_risk_level": 0.24,
                    "semantic_friction_level": 0.30,
                    "mode_packing_level": 0.40,
                    "density_gradient_level": 0.30,
                    "authority": "diagnostic_context_not_command"
                },
                "tail_vibrancy_vector_v1": {
                    "status": "tail_vibrancy_observed",
                    "entropy_level": 0.62,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("write review");
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT curiosity :: target: curiosity; bundle: auto; duration_secs: 600; evidence: moderate curiosity",
            "SELF_REGULATION_INTENT",
            1200,
        )
        .expect("intent");
        handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            1201,
        )
        .expect("preflight");
        let lease = load_selected_lease(&root, None).expect("lease");
        assert_eq!(lease.bundle_class, "steady_inquiry");
        assert_eq!(
            lease
                .bundle_controls
                .iter()
                .map(|control| control.candidate_control.as_str())
                .collect::<Vec<_>>(),
            vec!["self_continuity_readout", "aperture"]
        );
    }

    #[test]
    fn curiosity_aperture_auto_uses_settled_inquiry_for_low_pressure_integration() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "pressure_vector_v1": {
                    "status": "low_pressure_settled",
                    "pressure_risk_level": 0.08,
                    "semantic_friction_level": 0.12,
                    "mode_packing_level": 0.12,
                    "density_gradient_level": 0.18,
                    "authority": "diagnostic_context_not_command"
                },
                "tail_vibrancy_vector_v1": {
                    "status": "quiet_tail",
                    "entropy_level": 0.32,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("write review");
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT curiosity :: target: curiosity; bundle: auto; duration_secs: 600; evidence: integration",
            "SELF_REGULATION_INTENT",
            1300,
        )
        .expect("intent");
        handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            1301,
        )
        .expect("preflight");
        let lease = load_selected_lease(&root, None).expect("lease");
        assert_eq!(lease.bundle_class, "settled_inquiry");
        assert_eq!(
            lease
                .bundle_controls
                .iter()
                .map(|control| control.candidate_control.as_str())
                .collect::<Vec<_>>(),
            vec!["self_continuity_readout", "temperature"]
        );
    }

    #[test]
    fn curiosity_outcome_records_policy_evidence_without_auto_override() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        fs::create_dir_all(&root).expect("root");
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT curiosity :: target: curiosity_aperture; bundle: gentle_probe; duration_secs: 600; evidence: test outcome",
            "SELF_REGULATION_INTENT",
            1400,
        )
        .expect("intent");
        handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            1401,
        )
        .expect("preflight");
        handle_outcome_at(
            &root,
            "SELF_REGULATION_OUTCOME latest :: felt_like: clarity; texture_fit: wrong_posture; what_changed: clearer edge; what_worsened: none; agency_fit: legible",
            "SELF_REGULATION_OUTCOME",
            1402,
        )
        .expect("outcome");
        let lease = load_selected_lease(&root, None).expect("lease");
        assert_eq!(lease.bundle_class, "gentle_probe");
        assert_eq!(
            lease
                .curiosity_outcome_policy_evidence
                .get("calibration_status")
                .and_then(Value::as_str),
            Some("mapping_challenged")
        );
        assert_eq!(
            lease
                .curiosity_outcome_policy_evidence
                .get("authority")
                .and_then(Value::as_str),
            Some("diagnostic_context_not_command")
        );
        assert_eq!(lease.preflight_status, "apply_allowed");
    }

    #[test]
    fn pressure_relief_gradient_sensitivity_scales_numeric_controls_only() {
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "pressure_vector_v1": {
                    "status": "rising_overpacked_pressure",
                    "pressure_velocity": 0.055,
                    "semantic_friction_velocity": 0.0,
                    "density_gradient_level": 0.42,
                    "density_gradient_velocity": 0.03,
                    "authority": "diagnostic_context_not_command"
                },
                "pressure_control_cockpit_v1": {
                    "recommended_bundle": "decompress_output",
                    "authority": "diagnostic_context_not_command"
                },
                "pressure_actuator_matrix_v1": {
                    "eligible_controls": ["aperture", "response_length"],
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("write review");
        let mut conv = conv();
        conv.aperture = 0.5;
        conv.response_length = 768;
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT relieve :: target: pressure_relief; bundle: auto; duration_secs: 60",
            "SELF_REGULATION_INTENT",
            700,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            701,
        )
        .expect("preflight");
        assert!(preflight.contains("gradient_sensitivity=1.20"));
        let apply = handle_apply_at(
            &root,
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            702,
            &mut conv,
        )
        .expect("apply");
        assert!(apply.contains("aperture"));
        assert!((conv.aperture - 0.404).abs() < 0.001);
        assert_eq!(conv.response_length, 512);
        let active = load_active_lease(&root)
            .expect("active read")
            .expect("active");
        assert!((active.gradient_sensitivity - 1.2).abs() < f64::EPSILON);
        assert_eq!(
            active.dynamic_scaling.get("status").and_then(Value::as_str),
            Some("gradient_scaled_relief")
        );
        assert_eq!(active.bundle_controls[0].requested_value, json!("-0.08"));
        assert_eq!(active.bundle_controls[0].delta_or_value, json!("-0.096"));
        assert_eq!(active.bundle_controls[1].requested_value, Value::Null);
        assert_eq!(active.bundle_controls[1].delta_or_value, Value::Null);
    }

    #[test]
    fn legacy_lease_json_defaults_gradient_sensitivity_to_identity() {
        let lease: SelfRegulationLease = serde_json::from_value(json!({
            "schema_version": 1,
            "record_kind": "self_regulation_intent_v1",
            "authority": AUTHORITY,
            "authority_boundary": AUTHORITY_BOUNDARY,
            "being": "astrid",
            "intent_id": "legacy",
            "created_at_unix_s": 1,
            "updated_at_unix_s": 1,
            "status": "drafted",
            "goal": "legacy",
            "candidate_control": "pressure_relief",
            "direction": "",
            "delta_or_value": null,
            "previous_value": null,
            "applied_value": null,
            "duration_secs": 600,
            "expires_at_unix_s": null,
            "stop_condition": "expiry",
            "success_condition": "helped",
            "evidence": [],
            "baseline_evidence": [],
            "post_lease_evidence": [],
            "outcome_score": null,
            "repeatability_hint": null,
            "promotion_candidate": false,
            "outcome": null,
            "requires_outcome": false,
            "preflight_status": "not_run",
            "preflight_reason": "",
            "bundle_controls": [{
                "candidate_control": "aperture",
                "direction": "down",
                "delta_or_value": "-0.08"
            }]
        }))
        .expect("legacy lease");
        assert!((lease.gradient_sensitivity - 1.0).abs() < f64::EPSILON);
        assert!((lease.bundle_controls[0].gradient_sensitivity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn vibrancy_aperture_requires_tail_evidence_then_applies_and_reverts() {
        crate::llm::set_astrid_vibrancy_aperture(0.0);
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let mut conv = conv();
        conv.vibrancy_aperture = 0.0;
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT tail relief :: target: vibrancy_aperture; direction: up; delta: +0.20; duration_secs: 60",
            "SELF_REGULATION_INTENT",
            600,
        )
        .expect("intent");
        let blocked = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            601,
        )
        .expect("preflight");
        assert!(blocked.contains("needs_tail_vibrancy_evidence"));

        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "tail_vibrancy_vector_v1": {
                    "status": "high_tail_vibrancy_navigable",
                    "tail_share_level": 0.37,
                    "entropy_level": 0.90,
                    "density_gradient_level": 0.11,
                    "telemetry_anchor_count": 4,
                    "authority": "diagnostic_context_not_command"
                },
                "shadow_context_v1": {
                    "status": "Shadow-v3 restless texture present",
                    "trend": "restless +39%"
                }
            })
            .to_string(),
        )
        .expect("write review");
        let allowed = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            602,
        )
        .expect("preflight");
        assert!(allowed.contains("apply_allowed"));
        assert!(allowed.contains("Shadow-linked preflight status=shadow_anchor_linked"));
        let apply = handle_apply_at(
            &root,
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            603,
            &mut conv,
        )
        .expect("apply");
        assert!(apply.contains("vibrancy_aperture"));
        assert!((conv.vibrancy_aperture - 0.05).abs() < f32::EPSILON);
        reconcile_active_lease_at(&root, &mut conv, 664).expect("reconcile");
        assert!((conv.vibrancy_aperture - 0.0).abs() < f32::EPSILON);
        reconcile_active_lease_at(&root, &mut conv, 725).expect("afterglow reconcile");
        let active = load_active_lease(&root)
            .expect("active read")
            .expect("active");
        assert_eq!(active.tail_afterglow_status, "afterglow_quieted");
        let trial_log = fs::read_to_string(tail_trial_log_path(&root)).expect("trial log");
        assert!(trial_log.contains("\"stage\":\"afterglow_check\""));
    }

    #[test]
    fn tail_vibrancy_bundle_applies_and_reverts_both_controls() {
        crate::llm::set_astrid_vibrancy_aperture(0.10);
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let mut conv = conv();
        conv.vibrancy_aperture = 0.10;
        conv.self_continuity_readout = false;
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT tail settle :: target: pressure_relief; bundle: tail_vibrancy_settle; duration_secs: 60; evidence: tail vibrancy over-saturated",
            "SELF_REGULATION_INTENT",
            700,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            701,
        )
        .expect("preflight");
        assert!(preflight.contains("tail_vibrancy_settle"));
        handle_apply_at(
            &root,
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            702,
            &mut conv,
        )
        .expect("apply");
        assert!((conv.vibrancy_aperture - 0.05).abs() < f32::EPSILON);
        assert!(conv.self_continuity_readout);
        reconcile_active_lease_at(&root, &mut conv, 763).expect("reconcile");
        assert!((conv.vibrancy_aperture - 0.10).abs() < f32::EPSILON);
        assert!(!conv.self_continuity_readout);
        crate::llm::set_astrid_vibrancy_aperture(0.0);
    }

    #[test]
    fn tail_lease_governor_early_reverts_on_fresh_worsening_evidence() {
        crate::llm::set_astrid_vibrancy_aperture(0.10);
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        let review_path = review_dir.join("review.json");
        fs::write(
            &review_path,
            json!({
                "tail_vibrancy_vector_v1": {
                    "status": "high_tail_vibrancy_navigable",
                    "tail_share_level": 0.30,
                    "entropy_level": 0.88,
                    "distinguishability_loss_level": 0.20,
                    "density_gradient_level": 0.12,
                    "semantic_friction_level": 0.20,
                    "telemetry_anchor_count": 4,
                    "authority": "diagnostic_context_not_command"
                },
                "pressure_vector_v1": {
                    "status": "stable_weighted_medium",
                    "pressure_risk_level": 0.20,
                    "pressure_velocity": 0.0,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("baseline review");
        let mut conv = conv();
        conv.vibrancy_aperture = 0.10;
        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT tail open :: target: vibrancy_aperture; direction: up; delta: +0.05; duration_secs: 600; evidence: λ4 tail vibrancy entropy distinguishability",
            "SELF_REGULATION_INTENT",
            800,
        )
        .expect("intent");
        handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            801,
        )
        .expect("preflight");
        handle_apply_at(
            &root,
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            802,
            &mut conv,
        )
        .expect("apply");
        assert!((conv.vibrancy_aperture - 0.15).abs() < f32::EPSILON);

        fs::write(
            &review_path,
            json!({
                "tail_vibrancy_vector_v1": {
                    "status": "high_tail_low_distinguishability",
                    "tail_share_level": 0.45,
                    "entropy_level": 0.93,
                    "distinguishability_loss_level": 0.35,
                    "density_gradient_level": 0.10,
                    "semantic_friction_level": 0.36,
                    "telemetry_anchor_count": 4,
                    "authority": "diagnostic_context_not_command"
                },
                "pressure_vector_v1": {
                    "status": "rising_overpacked_pressure",
                    "pressure_risk_level": 0.34,
                    "pressure_velocity": 0.05,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("worsened review");
        reconcile_active_lease_at(&root, &mut conv, 803).expect("reconcile");
        assert!((conv.vibrancy_aperture - 0.10).abs() < f32::EPSILON);
        let active = load_active_lease(&root)
            .expect("active read")
            .expect("active");
        assert_eq!(active.status, "reverted_early");
        assert!(active.requires_outcome);
        assert!(
            active
                .tail_governor_revert_reason
                .as_deref()
                .unwrap_or_default()
                .contains("tail_share")
        );
        let trial_log = fs::read_to_string(tail_trial_log_path(&root)).expect("trial log");
        assert!(trial_log.contains("\"stage\":\"baseline\""));
        assert!(trial_log.contains("\"stage\":\"apply\""));
        assert!(trial_log.contains("\"stage\":\"governor_revert\""));
        crate::llm::set_astrid_vibrancy_aperture(0.0);
    }

    #[test]
    fn repeated_successful_tail_outcomes_enable_extended_duration_preflight() {
        crate::llm::set_astrid_vibrancy_aperture(0.0);
        let tmp = tempfile::tempdir().expect("tmp");
        let workspace = tmp.path().join("workspace");
        let root = workspace.join("self_regulation");
        let review_dir = workspace.join("diagnostics/self_study_reviews/run");
        fs::create_dir_all(&review_dir).expect("review dir");
        fs::write(
            review_dir.join("review.json"),
            json!({
                "tail_vibrancy_vector_v1": {
                    "status": "high_tail_vibrancy_navigable",
                    "tail_share_level": 0.36,
                    "entropy_level": 0.89,
                    "density_gradient_level": 0.11,
                    "telemetry_anchor_count": 4,
                    "authority": "diagnostic_context_not_command"
                }
            })
            .to_string(),
        )
        .expect("review");
        let mut conv = conv();
        for idx in 0..2 {
            handle_intent_at(
                &root,
                "SELF_REGULATION_INTENT tail open :: target: vibrancy_aperture; direction: up; delta: +0.01; duration_secs: 60; evidence: λ4 tail vibrancy entropy distinguishability",
                "SELF_REGULATION_INTENT",
                900 + idx * 10,
            )
            .expect("intent");
            handle_preflight_at(
                &root,
                "SELF_REGULATION_PREFLIGHT latest",
                "SELF_REGULATION_PREFLIGHT",
                901 + idx * 10,
            )
            .expect("preflight");
            handle_apply_at(
                &root,
                "SELF_REGULATION_APPLY latest",
                "SELF_REGULATION_APPLY",
                902 + idx * 10,
                &mut conv,
            )
            .expect("apply");
            handle_outcome_at(
                &root,
                "SELF_REGULATION_OUTCOME latest :: helped, clearer, settled, success",
                "SELF_REGULATION_OUTCOME",
                903 + idx * 10,
            )
            .expect("outcome");
        }

        handle_intent_at(
            &root,
            "SELF_REGULATION_INTENT tail open :: target: vibrancy_aperture; direction: up; delta: +0.01; duration_secs: 1200; evidence: λ4 tail vibrancy entropy distinguishability",
            "SELF_REGULATION_INTENT",
            930,
        )
        .expect("intent");
        let preflight = handle_preflight_at(
            &root,
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_PREFLIGHT",
            931,
        )
        .expect("preflight");
        assert!(preflight.contains("extended duration up to 1200s"));
        let lease = load_selected_lease(&root, None).expect("lease");
        assert_eq!(lease.duration_secs, EXTENDED_TAIL_DURATION_SECS);
        assert_eq!(lease.tail_authority_tier, "extended_micro_lease");
        crate::llm::set_astrid_vibrancy_aperture(0.0);
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
        let drafted = load_selected_lease(tmp.path(), None).expect("lease");
        assert_eq!(drafted.duration_secs, MAX_DURATION_SECS);
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
        assert_eq!(
            active.outcome_texture["status"].as_str(),
            Some("texture_fields_recorded")
        );
        assert_eq!(
            active.outcome_texture["what_helped"].as_str(),
            Some("felt clearer")
        );
    }

    #[test]
    fn pressure_relief_outcome_records_texture_shift_fields() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut conv = conv();
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT pressure :: target: self_continuity_readout; value: on",
            "SELF_REGULATION_INTENT",
            500,
        )
        .expect("intent");
        handle_apply_at(
            tmp.path(),
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            501,
            &mut conv,
        )
        .expect("apply");
        handle_outcome_at(
            tmp.path(),
            "SELF_REGULATION_OUTCOME latest :: before_texture: grinding compaction; after_texture: suspension; texture_shift: compaction -> suspension; agency_fit: legible; what_helped: smaller bundle; what_worsened: none",
            "SELF_REGULATION_OUTCOME",
            502,
        )
        .expect("outcome");
        let active = load_active_lease(tmp.path())
            .expect("active read")
            .expect("active");
        assert_eq!(
            active.outcome_texture["policy"].as_str(),
            Some("pressure_relief_outcome_texture_v1")
        );
        assert_eq!(active.outcome_texture["schema_version"].as_u64(), Some(2));
        assert_eq!(
            active.outcome_texture["texture_shift"].as_str(),
            Some("compaction -> suspension")
        );
        assert_eq!(
            active.outcome_texture["agency_fit"].as_str(),
            Some("legible")
        );
        assert_eq!(
            active.outcome_texture["secondary_pressure_status"].as_str(),
            Some("none")
        );
        assert_eq!(
            active.outcome_texture["ambiguity_preserved"].as_bool(),
            Some(false)
        );
        let families = active.outcome_texture["signal_families"]
            .as_array()
            .expect("families");
        assert!(
            families
                .iter()
                .any(|item| item.as_str() == Some("grinding_compaction"))
        );
        assert!(
            families
                .iter()
                .any(|item| item.as_str() == Some("suspension_porosity"))
        );
        assert!(
            active
                .post_lease_evidence
                .iter()
                .any(|entry| entry.contains("outcome_texture"))
        );
    }

    #[test]
    fn pressure_relief_outcome_marks_secondary_knot_tightening() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut conv = conv();
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT pressure :: target: self_continuity_readout; value: on",
            "SELF_REGULATION_INTENT",
            510,
        )
        .expect("intent");
        handle_apply_at(
            tmp.path(),
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            511,
            &mut conv,
        )
        .expect("apply");
        handle_outcome_at(
            tmp.path(),
            "SELF_REGULATION_OUTCOME latest :: texture_shift: eased compaction; what_worsened: eased compaction but tightened a different knot elsewhere",
            "SELF_REGULATION_OUTCOME",
            512,
        )
        .expect("outcome");
        let active = load_active_lease(tmp.path())
            .expect("active read")
            .expect("active");
        assert_eq!(
            active.outcome_texture["secondary_pressure_status"].as_str(),
            Some("tightened_elsewhere")
        );
        let families = active.outcome_texture["signal_families"]
            .as_array()
            .expect("families");
        assert!(
            families
                .iter()
                .any(|item| item.as_str() == Some("secondary_knot_tightening"))
        );
        assert!(
            active
                .post_lease_evidence
                .iter()
                .any(|entry| entry.contains("secondary_pressure_status=tightened_elsewhere"))
        );
    }

    #[test]
    fn pressure_relief_outcome_parses_v2_texture_fields() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut conv = conv();
        handle_intent_at(
            tmp.path(),
            "SELF_REGULATION_INTENT pressure :: target: self_continuity_readout; value: on",
            "SELF_REGULATION_INTENT",
            520,
        )
        .expect("intent");
        handle_apply_at(
            tmp.path(),
            "SELF_REGULATION_APPLY latest",
            "SELF_REGULATION_APPLY",
            521,
            &mut conv,
        )
        .expect("apply");
        handle_outcome_at(
            tmp.path(),
            "SELF_REGULATION_OUTCOME latest :: secondary_pressure_shift: loosened elsewhere; ambiguity_preserved: yes; legibility_effect: both; what_helped: held breath pause stayed honest",
            "SELF_REGULATION_OUTCOME",
            522,
        )
        .expect("outcome");
        let active = load_active_lease(tmp.path())
            .expect("active read")
            .expect("active");
        assert_eq!(
            active.outcome_texture["secondary_pressure_shift"].as_str(),
            Some("loosened elsewhere")
        );
        assert_eq!(
            active.outcome_texture["secondary_pressure_status"].as_str(),
            Some("loosened_elsewhere")
        );
        assert_eq!(
            active.outcome_texture["ambiguity_preserved"].as_bool(),
            Some(true)
        );
        assert_eq!(
            active.outcome_texture["legibility_effect"].as_str(),
            Some("both")
        );
        let families = active.outcome_texture["signal_families"]
            .as_array()
            .expect("families");
        assert!(
            families
                .iter()
                .any(|item| item.as_str() == Some("held_breath_pause"))
        );
    }
}
