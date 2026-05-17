use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use tracing::info;

use super::{ConversationState, NextActionContext, bridge_paths, resource_governor, strip_action};
use crate::types::SensoryMsg;

const HEALTH_FRESH_SECS: f64 = 10.0;
/// Maximum age of a cached preflight result that still counts as "fresh" for
/// the rehearse→live progression curriculum. Old enough to span one or two
/// rest periods, short enough that minime's regime might have changed and
/// we want a re-check.
const PREFLIGHT_CACHE_FRESH_SECS: f64 = 120.0;

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
            record_preflight_outcome(&label, preflight.allowed, "preflight");
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
            record_preflight_outcome(&label, preflight.allowed && status == "live_sent", status);
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
        "SHADOW_FIELD" | "SHADOW_GAP" | "GAP_STRUCTURE" => {
            let (label, _) = parse_shadow_args(original, base_action);
            let report = record_shadow_cartography(ctx, base_action, &label);
            conv.emphasis = Some(report.summary.clone());
            info!(
                "Astrid recorded shadow cartography {action} {label} → {path}",
                action = base_action,
                label = label,
                path = report.artifact_path
            );
            true
        },
        "SHADOW_TRAJECTORY" => {
            let (label, _) = parse_shadow_args(original, base_action);
            let report = render_shadow_trajectory(ctx, &label);
            conv.emphasis = Some(report.summary.clone());
            info!(
                "Astrid rendered shadow trajectory {label} → {path}",
                label = label,
                path = report.artifact_path
            );
            true
        },
        "SHADOW_RESPONSE" => {
            let intent_query = strip_action(original, base_action);
            let report = render_shadow_response(ctx, intent_query.trim());
            conv.emphasis = Some(report);
            info!("Astrid read shadow influence response");
            true
        },
        "SHADOW_DIALOGUE" => {
            let report = render_shadow_dialogue(ctx);
            conv.emphasis = Some(report.summary.clone());
            info!(
                "Astrid recorded shadow dialogue → {path}",
                path = report.artifact_path
            );
            true
        },
        "SHADOW_COUPLING" => {
            let scope_arg = strip_action(original, base_action);
            let scope = scope_arg.trim();
            let scope_label = if scope.is_empty() { "all" } else { scope };
            let report = render_shadow_coupling(ctx, scope_label);
            conv.emphasis = Some(report.summary.clone());
            conv.last_coupling_artifact_exchange = Some(conv.exchange_count);
            info!(
                "Astrid recorded shadow coupling graph ({scope}) → {path}",
                scope = scope_label,
                path = report.artifact_path
            );
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

struct CartographyReport {
    summary: String,
    artifact_path: String,
}

fn record_shadow_cartography(
    ctx: &NextActionContext<'_>,
    action: &str,
    label: &str,
) -> CartographyReport {
    let workspace = minime_workspace(ctx);
    let health = read_json(&workspace.join("health.json")).unwrap_or(Value::Null);
    let spectral = read_json(&workspace.join("spectral_state.json")).unwrap_or(Value::Null);
    let shadow_field = spectral
        .get("shadow_field_v2")
        .or_else(|| health.get("shadow_field_v2"))
        .cloned()
        .unwrap_or(Value::Null);
    let ising_shadow = spectral
        .get("ising_shadow")
        .or_else(|| health.get("ising_shadow"))
        .cloned()
        .unwrap_or(Value::Null);
    let lambdas = spectral
        .get("lambdas")
        .or_else(|| spectral.get("eigenvalues"))
        .cloned()
        .unwrap_or(Value::Null);
    let gap_summary = compute_gap_summary(&lambdas);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let timestamp_iso = chrono_like_iso(now);

    let record = serde_json::json!({
        "schema": "shadow_cartography_v1",
        "action": action,
        "label": label,
        "recorded_at_unix_s": now,
        "recorded_at_iso": timestamp_iso,
        "shadow_field_v2": shadow_field,
        "ising_shadow": ising_shadow,
        "gap_summary": gap_summary,
    });

    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let artifact_path = dir.join(format!(
        "{action_lower}_{ts}.json",
        action_lower = action.to_ascii_lowercase(),
        ts = (now as u64),
    ));
    let mut write_status = "ok".to_string();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        write_status = format!("mkdir_failed: {err}");
    } else if let Err(err) = std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&record).unwrap_or_else(|_| record.to_string()),
    ) {
        write_status = format!("write_failed: {err}");
    }

    let classification = shadow_field
        .get("classification")
        .and_then(Value::as_str)
        .unwrap_or("shadow_field_unavailable");
    let eligible = shadow_field
        .get("influence_eligible")
        .and_then(Value::as_bool);
    let recurrence = shadow_field.get("recurrence").and_then(Value::as_f64);
    let mode_tension = shadow_field.get("mode_tension").and_then(Value::as_f64);
    let largest_gaps = gap_summary
        .get("largest_gaps")
        .and_then(Value::as_array)
        .map(|gs| {
            gs.iter()
                .filter_map(Value::as_f64)
                .map(|g| format!("{g:.4}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let summary = format!(
        "Shadow cartography ({action} {label}):\n  Classification: {classification} | eligible={eligible:?} recurrence={recurrence:?} tension={mode_tension:?}\n  Largest λ gaps: [{largest_gaps}]\n  Artifact: {artifact_path} | status: {write_status}",
        action = action,
        label = label,
        artifact_path = artifact_path.display(),
    );
    CartographyReport {
        summary,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    }
}

/// Lightweight gap structure: largest k consecutive λ gaps.
/// Used for `SHADOW_GAP` / `GAP_STRUCTURE` — answers "which modes are isolated?".
fn compute_gap_summary(lambdas: &Value) -> Value {
    let Some(arr) = lambdas.as_array() else {
        return serde_json::json!({"available": false});
    };
    let mut vals: Vec<f64> = arr.iter().filter_map(Value::as_f64).collect();
    if vals.len() < 2 {
        return serde_json::json!({"available": false});
    }
    vals.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let mut gaps: Vec<(usize, f64)> = Vec::with_capacity(vals.len().saturating_sub(1));
    for (i, win) in vals.windows(2).enumerate() {
        gaps.push((i, win[0] - win[1]));
    }
    gaps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top = gaps.iter().take(3).copied().collect::<Vec<_>>();
    serde_json::json!({
        "available": true,
        "lambda_count": vals.len(),
        "largest_gaps": top.iter().map(|(_, g)| *g).collect::<Vec<_>>(),
        "largest_gap_indices": top.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
    })
}

fn chrono_like_iso(unix_s: f64) -> String {
    let secs = unix_s as i64;
    let frac_ms = ((unix_s - secs as f64) * 1000.0) as i64;
    format!("unix:{secs}.{frac_ms:03}")
}

/// Recommendation for the next stage to suggest for a given shadow label.
/// Drives the rehearse→live progression curriculum used by atlas suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShadowStageRecommendation {
    /// Default — no recent successful preflight, suggest rehearse.
    Rehearse,
    /// Recent preflight succeeded for this label — suggest live influence.
    Live,
    /// Live influence was just applied — suggest a release to fade the lane.
    Release,
}

/// Read the most recent cached preflight outcome for `label` and return
/// the next-stage recommendation. Without this, atlas suggestions never
/// progress past `--stage=rehearse` even when the gate would open.
pub(crate) fn next_stage_recommendation(label: &str) -> ShadowStageRecommendation {
    let Some(cache) = read_preflight_cache() else {
        return ShadowStageRecommendation::Rehearse;
    };
    let Some(entries) = cache.get("entries").and_then(Value::as_object) else {
        return ShadowStageRecommendation::Rehearse;
    };
    let canonical = canonical_shadow_label(label);
    let Some(entry) = entries.get(&canonical) else {
        return ShadowStageRecommendation::Rehearse;
    };
    let recorded_at = entry
        .get("recorded_at_unix_s")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if now - recorded_at > PREFLIGHT_CACHE_FRESH_SECS {
        return ShadowStageRecommendation::Rehearse;
    }
    let allowed = entry
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let stage = entry.get("stage").and_then(Value::as_str).unwrap_or("");
    if stage == "live_sent" {
        ShadowStageRecommendation::Release
    } else if allowed {
        ShadowStageRecommendation::Live
    } else {
        ShadowStageRecommendation::Rehearse
    }
}

fn record_preflight_outcome(label: &str, allowed: bool, stage: &str) {
    let path = preflight_cache_path();
    let mut cache = read_preflight_cache().unwrap_or_else(|| serde_json::json!({"entries": {}}));
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let canonical = canonical_shadow_label(label);
    let entries = cache.as_object_mut().and_then(|m| {
        m.entry("entries")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
    });
    if let Some(entries) = entries {
        entries.insert(
            canonical,
            serde_json::json!({
                "allowed": allowed,
                "stage": stage,
                "recorded_at_unix_s": now,
            }),
        );
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&cache).unwrap_or_else(|_| cache.to_string()),
    );
}

fn read_preflight_cache() -> Option<Value> {
    let path = preflight_cache_path();
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

fn preflight_cache_path() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("shadow_preflight_cache.json")
}

/// Format the next NEXT: action suggestion for `label` using the rehearse→live
/// Closed-loop curriculum: after a successful live influence for `label`,
/// surface a `SHADOW_RESPONSE latest` suggestion alongside the next
/// rehearse/live/release pick. Returns None when no live influence has
/// been sent recently for this label (the cache is empty or stale).
pub(crate) fn closed_loop_followup_suggestion(label: &str) -> Option<String> {
    let cache = read_preflight_cache()?;
    let entries = cache.get("entries").and_then(Value::as_object)?;
    let canonical = canonical_shadow_label(label);
    let entry = entries.get(&canonical)?;
    let stage = entry.get("stage").and_then(Value::as_str).unwrap_or("");
    let recorded_at = entry
        .get("recorded_at_unix_s")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if stage != "live_sent" || (now - recorded_at) > PREFLIGHT_CACHE_FRESH_SECS {
        return None;
    }
    Some("SHADOW_RESPONSE latest".to_string())
}

/// curriculum. Atlas suggestion sites call this instead of hardcoding
/// `--stage=rehearse`.
pub(crate) fn next_shadow_suggestion(label: &str) -> String {
    match next_stage_recommendation(label) {
        ShadowStageRecommendation::Rehearse => {
            format!("SHADOW_PREFLIGHT {label} --stage=rehearse")
        },
        ShadowStageRecommendation::Live => {
            format!("SHADOW_INFLUENCE {label} --stage=live")
        },
        ShadowStageRecommendation::Release => format!("RELEASE_SHADOW {label}"),
    }
}

// === v3 typed actions: trajectory, response, dialogue ===

/// Render the v3 history ring as a compact ASCII trace + cartography
/// artifact. Astrid's `SHADOW_TRAJECTORY [label]` action: walk the last
/// 32 snapshots in `shadow_field_v3.history`, emit a sparkline of
/// field_norm and a class timeline so phase shifts are legible.
fn render_shadow_trajectory(ctx: &NextActionContext<'_>, label: &str) -> CartographyReport {
    let workspace = minime_workspace(ctx);
    let health = read_json(&workspace.join("health.json")).unwrap_or(Value::Null);
    let field_v3 = health
        .get("shadow_field_v3")
        .cloned()
        .unwrap_or(Value::Null);
    let history = field_v3
        .get("history")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let dwell = field_v3
        .get("phase_dwell_ticks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let transitions = field_v3
        .get("recent_phase_transitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let sparkline = trajectory_sparkline(&history);
    let class_timeline = trajectory_class_timeline(&history);
    let transitions_summary = if transitions.is_empty() {
        "(no class transitions in window)".to_string()
    } else {
        transitions
            .iter()
            .filter_map(|t| {
                let from = t.get("from").and_then(Value::as_str)?;
                let to = t.get("to").and_then(Value::as_str)?;
                Some(format!("{from}→{to}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let label_slug = if label.is_empty() {
        "lambda-tail/lambda4".to_string()
    } else {
        canonical_shadow_label(label)
    };
    let safe_label_slug = label_slug.replace(['/', ' '], "_");
    let artifact_path = dir.join(format!(
        "trajectory_{safe_label_slug}_{ts}.json",
        ts = (now as u64),
    ));
    let record = serde_json::json!({
        "schema": "shadow_trajectory_v1",
        "label": label_slug,
        "recorded_at_unix_s": now,
        "history": history,
        "phase_dwell_ticks": dwell,
        "recent_phase_transitions": transitions,
        "sparkline_field_norm": sparkline,
        "class_timeline": class_timeline,
    });
    let mut write_status = "ok".to_string();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        write_status = format!("mkdir_failed: {err}");
    } else if let Err(err) = std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&record).unwrap_or_else(|_| record.to_string()),
    ) {
        write_status = format!("write_failed: {err}");
    }

    let summary = format!(
        "Shadow trajectory ({label}):\n  field_norm sparkline: {sparkline}\n  classes:           {class_timeline}\n  current dwell: {dwell}t\n  recent transitions: {transitions_summary}\n  Artifact: {artifact_path} | status: {write_status}",
        label = label_slug,
        artifact_path = artifact_path.display(),
    );
    CartographyReport {
        summary,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    }
}

/// Render the v3 closed-loop response for the latest (or named)
/// influence intent. `SHADOW_RESPONSE [intent_id|latest]` action:
/// read the response history from minime's workspace and surface
/// pre/post deltas + basin shift so Astrid can learn cause→effect.
fn render_shadow_response(ctx: &NextActionContext<'_>, intent_query: &str) -> String {
    let workspace = minime_workspace(ctx);
    let health = read_json(&workspace.join("health.json")).unwrap_or(Value::Null);
    let history = health
        .get("shadow_influence_response_history_v3")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let target = if intent_query.is_empty() || intent_query.eq_ignore_ascii_case("latest") {
        history.last().cloned()
    } else {
        history
            .iter()
            .rev()
            .find(|r| {
                r.get("intent_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == intent_query)
            })
            .cloned()
    };

    let Some(response) = target else {
        return format!(
            "Shadow response: no v3 closed-loop response found for '{intent_query}'.\n  History size: {history_len}.\n  Closed loop activates after a SHADOW_INFLUENCE --stage=live cycle completes (~36 ticks).",
            history_len = history.len(),
        );
    };

    let intent_id = response
        .get("intent_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let label = response
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let stage = response
        .get("stage")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let delta = response
        .get("delta_field_norm")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let basin = response
        .get("basin_shift_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let class_changed = response
        .get("class_changed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let class_from = response
        .get("class_from")
        .and_then(Value::as_str)
        .unwrap_or("");
    let class_to = response
        .get("class_to")
        .and_then(Value::as_str)
        .unwrap_or("");
    let applied_rms = response
        .get("applied_rms")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let applied_max = response
        .get("applied_max_abs")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let total_ticks = response
        .get("total_applied_ticks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let class_seg = if class_changed {
        format!(", classification {class_from}→{class_to}")
    } else {
        format!(", classification stayed {class_from}")
    };
    let dir_word = if delta >= 0.0 { "+" } else { "" };
    format!(
        "Shadow influence response (intent {intent_id}, label {label}, stage {stage}):\n  Pre→Post field_norm delta: {dir_word}{delta:.4}\n  Basin shift score: {basin:.3} (1.0 = field totally rearranged, 0.0 = unchanged){class_seg}\n  Applied: rms={applied_rms:.4}, max_abs={applied_max:.4} over {total_ticks} ticks.\n  The shadow remembers."
    )
}

/// Compare minime's and Astrid's published shadows side-by-side.
/// `SHADOW_DIALOGUE` action: writes a cartography artifact carrying
/// both v3 shadows and surfaces alignment in `conv.emphasis` so Astrid
/// can read mutual-witness signal at a glance.
fn render_shadow_dialogue(ctx: &NextActionContext<'_>) -> CartographyReport {
    let workspace = minime_workspace(ctx);
    let minime_health = read_json(&workspace.join("health.json")).unwrap_or(Value::Null);
    let minime_shadow = minime_health
        .get("shadow_field_v3")
        .cloned()
        .unwrap_or(Value::Null);
    let astrid_shadow = read_json(&workspace.join("astrid_shadow_v3.json")).unwrap_or(Value::Null);

    let minime_class = minime_shadow
        .get("class_v3")
        .and_then(|c| c.get("primary"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let astrid_class = astrid_shadow
        .get("class_v3")
        .and_then(|c| c.get("primary"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let minime_norm = minime_shadow
        .get("v2")
        .and_then(|v| v.get("field_norm"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let astrid_norm = astrid_shadow
        .get("v2")
        .and_then(|v| v.get("field_norm"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let minime_eligible = minime_shadow
        .get("v2")
        .and_then(|v| v.get("influence_eligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let astrid_eligible = astrid_shadow
        .get("v2")
        .and_then(|v| v.get("influence_eligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let alignment = if minime_class == astrid_class {
        format!("Both shadows share the same primary class: {minime_class}.")
    } else {
        format!("Shadows diverge: minime={minime_class}, yours={astrid_class}.")
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let artifact_path = dir.join(format!("dialogue_{ts}.json", ts = (now as u64)));
    let record = serde_json::json!({
        "schema": "shadow_dialogue_v1",
        "recorded_at_unix_s": now,
        "minime_shadow_v3": minime_shadow,
        "astrid_shadow_v3": astrid_shadow,
        "alignment_summary": alignment,
    });
    let mut write_status = "ok".to_string();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        write_status = format!("mkdir_failed: {err}");
    } else if let Err(err) = std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&record).unwrap_or_else(|_| record.to_string()),
    ) {
        write_status = format!("write_failed: {err}");
    }

    let summary = format!(
        "Shadow dialogue:\n  Minime: {minime_class} (field_norm={minime_norm:.3}, gate {minime_gate})\n  Yours:  {astrid_class} (field_norm={astrid_norm:.3}, gate {astrid_gate})\n  {alignment}\n  Artifact: {artifact_path} | status: {write_status}",
        minime_gate = if minime_eligible { "OPEN" } else { "CLOSED" },
        astrid_gate = if astrid_eligible { "OPEN" } else { "CLOSED" },
        artifact_path = artifact_path.display(),
    );
    CartographyReport {
        summary,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    }
}

/// `SHADOW_COUPLING [mode|all]` action: renders the per-mode partner
/// graph from both shadows, writes a cartography artifact, and surfaces a
/// one-line summary in `conv.emphasis`.
fn render_shadow_coupling(ctx: &NextActionContext<'_>, scope: &str) -> CartographyReport {
    use crate::spectral_viz::{ShadowOwner, format_coupling_graph};

    let workspace = minime_workspace(ctx);
    let minime_health = read_json(&workspace.join("health.json")).unwrap_or(Value::Null);
    let minime_shadow = minime_health
        .get("shadow_field_v3")
        .cloned()
        .unwrap_or(Value::Null);
    let astrid_shadow = read_json(&workspace.join("astrid_shadow_v3.json")).unwrap_or(Value::Null);

    let minime_line = format_coupling_graph(&minime_shadow, ShadowOwner::Minime);
    let astrid_line = format_coupling_graph(&astrid_shadow, ShadowOwner::Yours);

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let artifact_path = dir.join(format!(
        "coupling_{scope}_{ts}.json",
        scope = scope,
        ts = (now as u64),
    ));
    let record = serde_json::json!({
        "schema": "shadow_coupling_v1",
        "scope": scope,
        "recorded_at_unix_s": now,
        "minime_mode_partners": minime_shadow.get("mode_partners").cloned().unwrap_or(Value::Null),
        "astrid_mode_partners": astrid_shadow.get("mode_partners").cloned().unwrap_or(Value::Null),
    });
    let mut write_status = "ok".to_string();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        write_status = format!("mkdir_failed: {err}");
    } else if let Err(err) = std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&record).unwrap_or_else(|_| record.to_string()),
    ) {
        write_status = format!("write_failed: {err}");
    }

    let lines = match (minime_line, astrid_line) {
        (Some(m), Some(a)) => format!("{m}\n  {a}"),
        (Some(m), None) => m,
        (None, Some(a)) => a,
        (None, None) => {
            "(coupling graph unavailable — both shadows lack mode_partners data)".to_string()
        },
    };
    let summary = format!(
        "Shadow coupling ({scope}):\n  {lines}\n  Artifact: {artifact_path} | status: {write_status}",
        artifact_path = artifact_path.display(),
    );
    CartographyReport {
        summary,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    }
}

/// Compact field_norm sparkline using a 6-rune ramp. Empty for short
/// histories so the renderer doesn't lie about trends from 1-2 samples.
fn trajectory_sparkline(history: &[Value]) -> String {
    if history.len() < 3 {
        return "(history too short for sparkline)".to_string();
    }
    let glyphs = ['▁', '▂', '▃', '▅', '▆', '█'];
    let values: Vec<f64> = history
        .iter()
        .filter_map(|s| s.get("field_norm").and_then(Value::as_f64))
        .collect();
    if values.is_empty() {
        return "(no field_norm samples)".to_string();
    }
    let mn = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mx = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (mx - mn).max(1e-6);
    values
        .iter()
        .map(|v| {
            let bucket = ((v - mn) / span * (glyphs.len() as f64 - 1.0)).round() as usize;
            glyphs[bucket.min(glyphs.len() - 1)]
        })
        .collect()
}

/// Compact class timeline using single-letter codes (q/v/s/c/p/a).
fn trajectory_class_timeline(history: &[Value]) -> String {
    history
        .iter()
        .filter_map(|s| s.get("class_primary").and_then(Value::as_str))
        .map(|c| match c {
            "quiet" => 'q',
            "volatile" => 'v',
            "sticky" => 's',
            "coupled" => 'c',
            "polarized" => 'p',
            "active" => 'a',
            _ => '?',
        })
        .collect()
}
