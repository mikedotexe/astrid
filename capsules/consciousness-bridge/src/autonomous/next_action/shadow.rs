use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use tracing::info;

use super::{ConversationState, NextActionContext, bridge_paths, resource_governor, strip_action};
use crate::types::SensoryMsg;

const HEALTH_FRESH_SECS: f64 = 10.0;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "RESOURCE_AUDIT" | "MAC_RESOURCE_STATUS" => {
            conv.emphasis = Some(resource_audit_text(ctx));
            info!("Astrid requested M4 resource audit");
            true
        },
        "SHADOW_PREFLIGHT" => {
            let (label, stage) = parse_shadow_args(original, base_action);
            let preflight = shadow_preflight(ctx, &label, stage.as_deref().unwrap_or("live"));
            conv.emphasis = Some(format_shadow_preflight(&preflight));
            info!("Astrid requested shadow preflight for {label}");
            true
        },
        "SHADOW_INFLUENCE" => {
            let (label, stage) = parse_shadow_args(original, base_action);
            let stage = stage.unwrap_or_else(|| "rehearse".to_string());
            let preflight = shadow_preflight(ctx, &label, &stage);
            let mut sent = false;
            let status = if stage == "live" && preflight.allowed {
                let msg = shadow_influence_msg(&label, "apply", "live", &preflight.shadow_field);
                sent = ctx.sensory_tx.try_send(msg).is_ok();
                if sent {
                    "live_sent"
                } else {
                    "live_send_failed"
                }
            } else if stage == "live" {
                "downgraded_to_rehearse"
            } else {
                "rehearsed"
            };
            conv.emphasis = Some(format!(
                "{}\n\nShadow influence status: {status}. WebSocket sent: {sent}.",
                format_shadow_preflight(&preflight)
            ));
            info!("Astrid shadow influence {status} for {label}");
            true
        },
        "RELEASE_SHADOW" => {
            let (label, _) = parse_shadow_args(original, base_action);
            let preflight = shadow_preflight(ctx, &label, "live");
            let sent = ctx
                .sensory_tx
                .try_send(shadow_influence_msg(
                    &label,
                    "release",
                    "live",
                    &preflight.shadow_field,
                ))
                .is_ok();
            conv.emphasis = Some(format!(
                "{}\n\nShadow release requested. WebSocket sent: {sent}. Release is allowed because it fades the separate shadow influence lane toward zero.",
                format_shadow_preflight(&preflight)
            ));
            info!("Astrid requested shadow release for {label}");
            true
        },
        _ => false,
    }
}

#[derive(Debug, Clone)]
struct ShadowPreflight {
    label: String,
    requested_stage: String,
    expected_stage: String,
    allowed: bool,
    block_reason: Option<String>,
    health_age_s: Option<f64>,
    fill_pct: Option<f64>,
    safety_level: String,
    shadow_field: Value,
    shadow_influence_active: bool,
    attractor_pulse_active: bool,
    resource_governor: resource_governor::ResourceGovernorStatus,
}

fn parse_shadow_args(original: &str, base_action: &str) -> (String, Option<String>) {
    let raw = strip_action(original, base_action);
    let mut label_parts = Vec::new();
    let mut stage = None;
    for part in raw.split_whitespace() {
        if let Some(value) = part.strip_prefix("--stage=") {
            stage = Some(value.trim().to_ascii_lowercase());
        } else {
            label_parts.push(part);
        }
    }
    let label = label_parts.join(" ");
    let label = if label.trim().is_empty() {
        "lambda-tail/lambda4".to_string()
    } else {
        canonical_shadow_label(label.trim())
    };
    (label, stage)
}

fn canonical_shadow_label(label: &str) -> String {
    let lower = label
        .replace('λ', "lambda")
        .replace('_', "-")
        .to_ascii_lowercase();
    if lower.contains("lambda4") || lower.contains("lambda-4") || lower.contains("lambda 4") {
        "lambda-tail/lambda4".to_string()
    } else if lower.contains("lambda") && lower.contains("tail") {
        "lambda-tail".to_string()
    } else {
        lower
    }
}

fn minime_workspace(ctx: &NextActionContext<'_>) -> PathBuf {
    ctx.workspace.map_or_else(
        || bridge_paths().minime_workspace().to_path_buf(),
        Path::to_path_buf,
    )
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn path_age_s(path: &Path) -> Option<f64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now()
        .duration_since(modified)
        .ok()
        .map(|d| d.as_secs_f64())
}

fn shadow_preflight(
    ctx: &NextActionContext<'_>,
    label: &str,
    requested_stage: &str,
) -> ShadowPreflight {
    let workspace = minime_workspace(ctx);
    let health_path = workspace.join("health.json");
    let spectral_path = workspace.join("spectral_state.json");
    let health = read_json(&health_path).unwrap_or(Value::Null);
    let spectral = read_json(&spectral_path).unwrap_or(Value::Null);
    let health_age_s = path_age_s(&health_path);
    let health_fresh = health_age_s.is_some_and(|age| age <= HEALTH_FRESH_SECS);
    let fill_pct = health
        .get("fill_pct")
        .and_then(Value::as_f64)
        .or(Some(ctx.fill_pct as f64));
    let stable_core = health.get("stable_core").unwrap_or(&Value::Null);
    let restart_gate = stable_core.get("restart_gate").unwrap_or(&Value::Null);
    let structural_pi = stable_core.get("structural_pi").unwrap_or(&Value::Null);
    let restart_applied = restart_gate
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || structural_pi
            .get("restart_gate_applied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let restart_active = restart_gate
        .get("active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let restart_settled = restart_gate
        .get("settled_at_unix_ms")
        .and_then(Value::as_i64)
        .is_some();
    let stage = stable_core
        .get("stage")
        .or_else(|| health.get("stage"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let recovery_impulse = structural_pi
        .get("recovery_impulse_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let shadow_field = spectral
        .get("shadow_field_v2")
        .or_else(|| health.get("shadow_field_v2"))
        .cloned()
        .unwrap_or(Value::Null);
    let shadow_influence_active = health
        .get("shadow_influence")
        .and_then(|v| v.get("active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let attractor_pulse_active = health
        .get("attractor_pulse")
        .and_then(|v| v.get("active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let field_eligible = shadow_field
        .get("influence_eligible")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let governor = resource_governor::status(ctx, true);
    let safety_level = match fill_pct {
        Some(fill) if fill < 58.0 => "low_fill_recovery",
        Some(fill) if fill >= 85.0 => "overbright_fill",
        Some(fill) if fill >= 75.0 => "yellow",
        Some(_) => "green",
        None => "stale_telemetry",
    }
    .to_string();
    let live_requested = requested_stage.eq_ignore_ascii_case("live");
    let block_reason = if !live_requested {
        None
    } else if !health_fresh {
        Some("stale_telemetry".to_string())
    } else if stage == "discharge" {
        Some("discharge".to_string())
    } else if restart_applied {
        Some("restart_gate_applied".to_string())
    } else if restart_active && !restart_settled {
        Some("restart_gate_awaiting_settle_proof".to_string())
    } else if recovery_impulse {
        Some("recovery_impulse_active".to_string())
    } else if attractor_pulse_active {
        Some("attractor_pulse_active".to_string())
    } else if shadow_influence_active {
        Some("shadow_influence_active".to_string())
    } else if !governor.allowed_live {
        Some(format!(
            "resource_governor:{}",
            governor
                .primary_block_reason
                .as_deref()
                .unwrap_or("blocked")
        ))
    } else if matches!(
        safety_level.as_str(),
        "low_fill_recovery" | "overbright_fill"
    ) {
        Some(safety_level.clone())
    } else if !field_eligible {
        Some("shadow_field_not_influence_eligible".to_string())
    } else {
        None
    };
    let allowed = live_requested && block_reason.is_none();
    ShadowPreflight {
        label: label.to_string(),
        requested_stage: requested_stage.to_string(),
        expected_stage: if allowed { "live" } else { "rehearse" }.to_string(),
        allowed,
        block_reason,
        health_age_s,
        fill_pct,
        safety_level,
        shadow_field,
        shadow_influence_active,
        attractor_pulse_active,
        resource_governor: governor,
    }
}

fn format_shadow_preflight(preflight: &ShadowPreflight) -> String {
    let field = &preflight.shadow_field;
    format!(
        "Shadow preflight:\n  Label: {}\n  Requested stage: {} | expected stage: {}\n  Allowed live: {} | block: {}\n  Health: age_s={:?} fill={:?} safety={}\n  Active conflicts: attractor_pulse={} shadow_influence={}\n  {}\n  Field: {} recurrence={:?} tension={:?} tail_open={:?} lock={:?} fissure={:?} eligible={:?}\n  Suggested next: {}",
        preflight.label,
        preflight.requested_stage,
        preflight.expected_stage,
        preflight.allowed,
        preflight.block_reason.as_deref().unwrap_or("none"),
        preflight.health_age_s,
        preflight.fill_pct,
        preflight.safety_level,
        preflight.attractor_pulse_active,
        preflight.shadow_influence_active,
        preflight.resource_governor.summary_line(),
        field
            .get("classification")
            .and_then(Value::as_str)
            .unwrap_or("shadow_field_unavailable"),
        field.get("recurrence").and_then(Value::as_f64),
        field.get("mode_tension").and_then(Value::as_f64),
        field.get("tail_openness").and_then(Value::as_f64),
        field.get("lock_tendency").and_then(Value::as_f64),
        field.get("fissure_tendency").and_then(Value::as_f64),
        field.get("influence_eligible").and_then(Value::as_bool),
        if preflight.allowed {
            format!("SHADOW_INFLUENCE {} --stage=live", preflight.label)
        } else {
            format!("SHADOW_INFLUENCE {} --stage=rehearse", preflight.label)
        }
    )
}

fn shadow_influence_msg(
    label: &str,
    command: &str,
    stage: &str,
    shadow_field: &Value,
) -> SensoryMsg {
    SensoryMsg::ShadowInfluence {
        intent_id: format!("astrid-shadow-{}", deterministic_hash(label) & 0xffff_ffff),
        label: label.to_string(),
        command: command.to_string(),
        stage: Some(stage.to_string()),
        features: if command == "release" {
            Vec::new()
        } else {
            shadow_features(label, shadow_field)
        },
        max_abs: Some(if command == "release" { 0.0 } else { 0.025 }),
        duration_ticks: Some(if command == "release" { 0 } else { 24 }),
        decay_ticks: Some(12),
        basis: Some(label.to_string()),
    }
}

fn shadow_features(label: &str, shadow_field: &Value) -> Vec<f32> {
    let mut features = vec![0.0; 66];
    let basis = format!(
        "astrid-shadow:{}:{}",
        label,
        shadow_field
            .get("classification")
            .and_then(Value::as_str)
            .unwrap_or("shadow")
    );
    let tail = shadow_field
        .get("tail_openness")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    let tension = shadow_field
        .get("mode_tension")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    let recurrence = shadow_field
        .get("recurrence")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    features[16] = ((tail - 0.5) * 0.018).clamp(-0.012, 0.012);
    features[17] = ((tension - recurrence) * 0.018).clamp(-0.012, 0.012);
    for idx in 0..48 {
        let byte = ((deterministic_hash(&format!("{basis}:{idx}")) >> (idx % 8)) & 0xff) as f32;
        features[18 + idx] = ((byte / 255.0) - 0.5) * 0.05;
    }
    features
}

fn deterministic_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn resource_audit_text(ctx: &NextActionContext<'_>) -> String {
    resource_governor::audit_text(ctx)
}
