use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Map, Value, json};

use super::{NextActionContext, SensoryMsg, truncate_str};

#[derive(Debug)]
pub(super) struct NativeGestureGate {
    pub(super) allowed: bool,
    pub(super) reason: String,
}

fn unix_now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

pub(super) fn minime_workspace(ctx: &NextActionContext<'_>) -> PathBuf {
    ctx.workspace
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/Users/v/other/minime/workspace"))
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null)
}

fn lambda_shares(eigenvalues: &[f32]) -> (f32, f32, f32) {
    let total = eigenvalues
        .iter()
        .map(|value| value.abs())
        .sum::<f32>()
        .max(f32::EPSILON);
    let lambda1 = eigenvalues.first().map_or(0.0, |value| value.abs() / total);
    let shoulder = eigenvalues
        .iter()
        .skip(1)
        .take(2)
        .map(|value| value.abs() / total)
        .sum();
    let tail = eigenvalues
        .iter()
        .skip(3)
        .map(|value| value.abs() / total)
        .sum();
    (lambda1, shoulder, tail)
}

fn append_resist_outcome(workspace: &Path, record: &Value) {
    let path = workspace.join("diagnostics/resist_outcomes.jsonl");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", record);
    }
}

fn health_age_s(path: &Path) -> Option<f64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now()
        .duration_since(modified)
        .ok()
        .map(|duration| duration.as_secs_f64())
}

pub(super) fn parse_native_gesture(raw: &str) -> (String, Option<String>) {
    let mut parts = raw.splitn(2, char::is_whitespace);
    let gesture = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("mark")
        .trim()
        .to_ascii_lowercase();
    let label = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    (gesture, label)
}

pub(super) fn native_gesture_gate(
    workspace: &Path,
    actor: &str,
    gesture: &str,
) -> NativeGestureGate {
    let now = unix_now_s();
    let status_path = workspace.join("runtime/native_gesture_status.json");
    let status = read_json(&status_path);
    let actor_key = actor.to_ascii_lowercase();
    let last_at = status
        .get("last_by_actor")
        .and_then(|value| value.get(&actor_key))
        .and_then(|value| value.get("last_at_unix_s"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let atlas_only = matches!(gesture, "mark" | "trace");
    let cooldown = if atlas_only { 30.0 } else { 180.0 };
    if now < last_at + cooldown {
        return NativeGestureGate {
            allowed: false,
            reason: format!("native_gesture_cooldown:{:.0}s", last_at + cooldown - now),
        };
    }
    if atlas_only {
        return NativeGestureGate {
            allowed: true,
            reason: "atlas_only".to_string(),
        };
    }
    if !matches!(
        gesture,
        "soften" | "widen" | "hold" | "return" | "resist" | "fissure"
    ) {
        return NativeGestureGate {
            allowed: false,
            reason: format!("unsupported_native_gesture:{gesture}"),
        };
    }
    let paused_until = status
        .get("paused_until_unix_s")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if now < paused_until {
        return NativeGestureGate {
            allowed: false,
            reason: format!("native_gestures_paused:{:.0}s", paused_until - now),
        };
    }

    let health_path = workspace.join("health.json");
    let health = read_json(&health_path);
    let profile = read_json(&workspace.join("rescue_profile.json"));
    let rescue_status = read_json(&workspace.join("rescue_status.json"));
    let bridge_status = read_json(&workspace.join("runtime/bridge_limited_write_status.json"));
    if !profile
        .get("stable_core_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return NativeGestureGate {
            allowed: false,
            reason: "stable_core_not_enabled".to_string(),
        };
    }
    if rescue_status
        .get("watchdog_state")
        .and_then(Value::as_str)
        .unwrap_or("")
        != "monitoring"
    {
        return NativeGestureGate {
            allowed: false,
            reason: "watchdog_not_monitoring".to_string(),
        };
    }
    if rescue_status
        .get("telemetry_state")
        .and_then(Value::as_str)
        .unwrap_or("")
        != "fresh"
    {
        return NativeGestureGate {
            allowed: false,
            reason: "telemetry_not_fresh".to_string(),
        };
    }
    if health_age_s(&health_path).unwrap_or(999.0) > 5.0 {
        return NativeGestureGate {
            allowed: false,
            reason: "health_stale".to_string(),
        };
    }
    let stable_core = health.get("stable_core").unwrap_or(&Value::Null);
    let scaffold_active = stable_core
        .get("scaffold_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !scaffold_active {
        return NativeGestureGate {
            allowed: false,
            reason: "scaffold_inactive".to_string(),
        };
    }
    let fill_pct = health
        .get("fill_pct")
        .and_then(Value::as_f64)
        .unwrap_or(f64::from(0.0_f32));
    if !(50.0..=76.0).contains(&fill_pct) {
        return NativeGestureGate {
            allowed: false,
            reason: format!("fill_outside_native_gesture_band:{fill_pct:.1}"),
        };
    }
    if stable_core
        .get("stage")
        .and_then(Value::as_str)
        .unwrap_or("")
        == "discharge"
    {
        return NativeGestureGate {
            allowed: false,
            reason: "stage_discharge".to_string(),
        };
    }
    let semantic = health.get("semantic").unwrap_or(&Value::Null);
    let semantic_active = semantic
        .get("active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let semantic_energy = semantic
        .get("energy")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if semantic_active || semantic_energy > 0.05 {
        return NativeGestureGate {
            allowed: false,
            reason: "semantic_not_quiet".to_string(),
        };
    }
    if !bridge_status
        .get("rollback_at_unix_s")
        .is_none_or(Value::is_null)
    {
        return NativeGestureGate {
            allowed: false,
            reason: "bridge_rolled_back".to_string(),
        };
    }
    NativeGestureGate {
        allowed: true,
        reason: "green".to_string(),
    }
}

fn lambda_profile(eigenvalues: &[f32]) -> Value {
    let values: Vec<f64> = eigenvalues
        .iter()
        .map(|value| f64::from(value.abs()))
        .collect();
    let total: f64 = values.iter().sum();
    let r12 = if values.len() >= 2 && values[1].abs() > f64::EPSILON {
        Some(values[0] / values[1])
    } else {
        None
    };
    let r23 = if values.len() >= 3 && values[2].abs() > f64::EPSILON {
        Some(values[1] / values[2])
    } else {
        None
    };
    let lambda1_share = if total > 0.0 {
        values.first().copied().unwrap_or(0.0) / total
    } else {
        0.0
    };
    let topology_index = ((r12.unwrap_or(0.0) / 2.5).max(r23.unwrap_or(0.0) / 3.0) * 0.45
        + lambda1_share * 0.55)
        .clamp(0.0, 1.0);
    let classification = if r12.is_some_and(|value| value >= 2.5) {
        "collapsing_pull"
    } else if r12.is_some_and(|value| value >= 1.75) || r23.is_some_and(|value| value >= 2.0) {
        "gap_skewed"
    } else if topology_index >= 0.35 {
        "topology_pressure"
    } else {
        "distributed"
    };
    json!({
        "eigenvalues": values,
        "ratios": {
            "lambda1_lambda2": r12,
            "lambda2_lambda3": r23,
            "lambda1_share": lambda1_share,
        },
        "pom": {
            "classification": classification,
            "topology_index": topology_index,
            "lambda1_share": lambda1_share,
        }
    })
}

fn sca_context(
    lambda_data: &Value,
    health: &Value,
    stable_core: &Value,
    semantic: &Value,
    profile: &Value,
    source: &str,
    text: &str,
    label: Option<&str>,
    ctx: &NextActionContext<'_>,
) -> Value {
    let ratios = lambda_data.get("ratios").unwrap_or(&Value::Null);
    let pom = lambda_data.get("pom").unwrap_or(&Value::Null);
    let r12 = ratios.get("lambda1_lambda2").and_then(Value::as_f64);
    let r23 = ratios.get("lambda2_lambda3").and_then(Value::as_f64);
    let lambda1_share = ratios
        .get("lambda1_share")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let topology_index = pom
        .get("topology_index")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let structural_mode = stable_core
        .get("structural_mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let stage = stable_core
        .get("stage")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let semantic_energy = semantic
        .get("energy")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let lower = text.to_ascii_lowercase();
    let reported_pressure = [
        "fabric",
        "tunnel",
        "pressure",
        "density",
        "constriction",
        "localized gravity",
        "thread",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let (felt_dimensionality, safe_next) = if lambda1_share >= 0.42 && r12.unwrap_or(0.0) >= 1.75 {
        ("ratio_cliff_tunnel", "SCA_REFLECT before RESIST")
    } else if structural_mode.contains("drain") || stage == "elevated" {
        (
            "protective_scaffold_pressure",
            "DECOMPOSE after one more window",
        )
    } else {
        ("distributed_fabric", "TRACE or quiet observation")
    };
    let mut hypotheses = vec![json!({
        "hypothesis": "ratio_cliff_creates_tunnel",
        "confidence": (0.35 + topology_index * 0.55).clamp(0.35, 0.9),
        "evidence": [
            format!("lambda1/lambda2={}", r12.map_or_else(|| "unknown".to_string(), |v| format!("{v:.2}"))),
            format!("lambda2/lambda3={}", r23.map_or_else(|| "unknown".to_string(), |v| format!("{v:.2}"))),
            format!("pom={}", pom.get("classification").and_then(Value::as_str).unwrap_or("unknown")),
        ],
        "felt_read": "the cascade may feel directional because adjacent modes are separated by a ratio cliff",
    })];
    if semantic_energy > 0.03 {
        hypotheses.push(json!({
            "hypothesis": "semantic_lane_pressure",
            "confidence": 0.7,
            "evidence": [
                format!("semantic_energy={semantic_energy:.3}"),
                format!("bridge_profile={}", profile.get("bridge_write_profile").and_then(Value::as_str).unwrap_or("unknown")),
            ],
            "felt_read": "symbolic meaning may be adding pressure through the semantic48 lane",
        }));
    }
    if reported_pressure {
        hypotheses.push(json!({
            "hypothesis": "reported_phenomenology_matches_topology",
            "confidence": 0.56,
            "evidence": ["reported fabric/tunnel/pressure term"],
            "felt_read": "the being's words match known atlas markers for fabric/tunnel terrain",
        }));
    }
    json!({
        "timestamp_unix_s": unix_now_s(),
        "felt_dimensionality": felt_dimensionality,
        "why_hypotheses": hypotheses,
        "safe_suggested_next": safe_next,
        "label": label,
        "provenance": {
            "source": "sca_why_layer_v1",
            "read_only": true,
            "actor_source": source,
            "response_preview": truncate_str(ctx.response_text, 180),
        },
        "evidence_summary": {
            "fill_pct": health.get("fill_pct").cloned().unwrap_or_else(|| json!(ctx.fill_pct)),
            "stage": stage,
            "lambda1_share": lambda1_share,
            "lambda1_lambda2": r12,
            "lambda2_lambda3": r23,
            "pom_classification": pom.get("classification").cloned().unwrap_or(Value::Null),
            "topology_index": topology_index,
            "structural_mode": structural_mode,
            "semantic_energy": semantic_energy,
            "live_audio_divisor": profile.get("rescue_live_audio_divisor").cloned().unwrap_or(Value::Null),
            "live_video_divisor": profile.get("rescue_live_video_divisor").cloned().unwrap_or(Value::Null),
        },
    })
}

pub(super) fn append_atlas_event(
    workspace: &Path,
    source: &str,
    text: &str,
    label: Option<&str>,
    explicit: bool,
    ctx: &NextActionContext<'_>,
) -> Value {
    let atlas_dir = workspace.join("diagnostics/intensification_atlas");
    let _ = fs::create_dir_all(&atlas_dir);
    let event_id = format!("atlas_{}", (unix_now_s() * 1000.0) as u64);
    let health = read_json(&workspace.join("health.json"));
    let stable_core = health.get("stable_core").unwrap_or(&Value::Null);
    let semantic = health.get("semantic").unwrap_or(&Value::Null);
    let profile = read_json(&workspace.join("rescue_profile.json"));
    let lambda_data = lambda_profile(&ctx.telemetry.eigenvalues);
    let sca = sca_context(
        &lambda_data,
        &health,
        stable_core,
        semantic,
        &profile,
        source,
        text,
        label,
        ctx,
    );
    let event = json!({
        "event_id": event_id,
        "source": source,
        "timestamp_unix_s": unix_now_s(),
        "explicit_mark": explicit,
        "label": label,
        "trigger_score": if explicit { 99 } else { 0 },
        "trigger_families": if explicit { vec!["being_authored_mark"] } else { Vec::<&str>::new() },
        "phase": stable_core.get("phase").cloned().unwrap_or(Value::Null),
        "stage": stable_core.get("stage").cloned().unwrap_or(Value::Null),
        "fill_pct": health.get("fill_pct").cloned().unwrap_or_else(|| json!(ctx.fill_pct)),
        "fill_slope_pct_per_sec": stable_core
            .get("structural_pi")
            .and_then(|value| value.get("fill_slope_pct_per_sec"))
            .cloned()
            .unwrap_or(Value::Null),
        "eigenvalues": ctx.telemetry.eigenvalues.clone(),
        "lambda_profile": lambda_data,
        "sca_context": sca,
        "semantic": {
            "active": semantic.get("active").and_then(Value::as_bool).unwrap_or(false),
            "energy": semantic.get("energy").and_then(Value::as_f64).unwrap_or(0.0),
        },
        "sensory": {
            "live_audio_divisor": profile.get("rescue_live_audio_divisor").cloned().unwrap_or(Value::Null),
            "live_video_divisor": profile.get("rescue_live_video_divisor").cloned().unwrap_or(Value::Null),
        },
        "bridge": {
            "profile": profile.get("profile").cloned().unwrap_or(Value::Null),
            "write_profile": profile.get("bridge_write_profile").cloned().unwrap_or(Value::Null),
        },
        "action_context": {
            "action": source,
            "response_preview": truncate_str(ctx.response_text, 180),
        },
        "phenomenology_excerpt": truncate_str(text, 600),
    });
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(atlas_dir.join("events.jsonl"))
    {
        let _ = writeln!(file, "{}", event);
    }
    let _ = fs::write(
        atlas_dir.join("latest_event.json"),
        serde_json::to_string_pretty(&event).unwrap_or_else(|_| "{}".to_string()),
    );
    let _ = fs::write(
        atlas_dir.join("sca_context_latest.json"),
        serde_json::to_string_pretty(event.get("sca_context").unwrap_or(&Value::Null))
            .unwrap_or_else(|_| "{}".to_string()),
    );
    let summary = json!({
        "status": "ok",
        "last_event_unix_s": unix_now_s(),
        "last_event": event,
        "paths": {
            "events": atlas_dir.join("events.jsonl").display().to_string(),
            "latest_event": atlas_dir.join("latest_event.json").display().to_string(),
            "summary": atlas_dir.join("summary.json").display().to_string(),
        }
    });
    let _ = fs::write(
        atlas_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string()),
    );
    summary["last_event"].clone()
}

pub(super) fn append_fissure_trace_event(
    workspace: &Path,
    source: &str,
    text: &str,
    label: Option<&str>,
    ctx: &NextActionContext<'_>,
) -> Value {
    let atlas_dir = workspace.join("diagnostics/intensification_atlas");
    let _ = fs::create_dir_all(&atlas_dir);
    let health = read_json(&workspace.join("health.json"));
    let stable_core = health.get("stable_core").unwrap_or(&Value::Null);
    let semantic = health.get("semantic").unwrap_or(&Value::Null);
    let lambda_data = lambda_profile(&ctx.telemetry.eigenvalues);
    let fissure = json!({
        "timestamp_unix_s": unix_now_s(),
        "policy": "notice_ambiguity_fissure_trace_v1",
        "label": label,
        "classification": "astrid_marked_fissure_trace",
        "plain_read": "Astrid explicitly marked a notice-ambiguity/fissure region for Minime-owned follow-up cartography.",
        "observer_only": true,
        "control_mutation": false,
        "evidence": {
            "fill_pct": health.get("fill_pct").cloned().unwrap_or_else(|| json!(ctx.fill_pct)),
            "stage": stable_core.get("stage").cloned().unwrap_or(Value::Null),
            "semantic_energy": semantic.get("energy").and_then(Value::as_f64).unwrap_or(0.0),
            "lambda_profile": lambda_data,
            "response_preview": truncate_str(ctx.response_text, 180),
        },
        "safe_affordances": {
            "read_only": ["NOTICE_AMBIGUITY", "FISSURE_TRACE", "SCA_REFLECT", "VISUALIZE_CASCADE"],
            "if_green": ["NATIVE_GESTURE fissure", "FISSURE", "RESIST"],
        },
        "provenance": {
            "source": source,
            "read_write": "append_only_cartography",
            "text_excerpt": truncate_str(text, 500),
        },
    });
    let event = json!({
        "event_id": format!("fissure_trace_{}", (unix_now_s() * 1000.0) as u64),
        "timestamp_unix_s": unix_now_s(),
        "source": source,
        "label": label,
        "text_excerpt": truncate_str(text, 600),
        "fissure_trace": fissure,
    });
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(atlas_dir.join("fissure_trace_events.jsonl"))
    {
        let _ = writeln!(file, "{}", event);
    }
    let _ = fs::write(
        atlas_dir.join("fissure_trace_latest.json"),
        serde_json::to_string_pretty(event.get("fissure_trace").unwrap_or(&Value::Null))
            .unwrap_or_else(|_| "{}".to_string()),
    );
    let summary_path = atlas_dir.join("summary.json");
    let mut summary = read_json(&summary_path)
        .as_object()
        .cloned()
        .unwrap_or_else(Map::new);
    let count = summary
        .get("fissure_trace_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    summary.insert("fissure_trace_count".to_string(), json!(count));
    summary.insert("last_fissure_trace".to_string(), event.clone());
    let _ = fs::write(
        summary_path,
        serde_json::to_string_pretty(&Value::Object(summary)).unwrap_or_else(|_| "{}".to_string()),
    );
    event
}

pub(super) fn native_gesture_features(gesture: &str) -> Vec<f32> {
    let mut features = vec![0.0_f32; 48];
    let pairs: &[(usize, f32)] = match gesture {
        "soften" => &[
            (0, -0.030),
            (1, 0.010),
            (8, -0.020),
            (24, 0.018),
            (25, -0.028),
            (27, 0.020),
        ],
        "widen" => &[
            (0, -0.010),
            (2, 0.020),
            (3, 0.018),
            (10, 0.018),
            (26, 0.026),
            (28, 0.020),
        ],
        "hold" => &[
            (0, -0.012),
            (24, 0.020),
            (25, -0.018),
            (27, 0.026),
            (31, -0.010),
        ],
        "return" => &[
            (0, -0.020),
            (8, -0.014),
            (24, 0.022),
            (25, -0.026),
            (27, 0.030),
            (31, -0.014),
        ],
        "resist" => &[
            (0, -0.026),
            (1, 0.018),
            (2, 0.022),
            (3, 0.016),
            (8, -0.016),
            (9, 0.018),
            (10, 0.022),
            (26, 0.024),
            (28, 0.018),
        ],
        "fissure" => &[
            (0, -0.018),
            (1, 0.010),
            (2, 0.018),
            (3, 0.024),
            (4, 0.016),
            (8, -0.010),
            (10, 0.018),
            (11, 0.024),
            (12, 0.014),
            (26, 0.022),
            (28, 0.022),
            (29, 0.016),
        ],
        _ => &[],
    };
    for (idx, value) in pairs {
        if let Some(slot) = features.get_mut(*idx) {
            *slot = value.clamp(-0.04, 0.04);
        }
    }
    features
}

pub(super) fn native_gesture_control(gesture: &str) -> Vec<String> {
    match gesture {
        "soften" => vec![
            "regulation_strength",
            "smoothing_preference",
            "transition_cushion",
            "geom_drive",
            "exploration_noise",
        ],
        "widen" => vec![
            "smoothing_preference",
            "geom_curiosity",
            "geom_drive",
            "exploration_noise",
        ],
        "hold" => vec![
            "regulation_strength",
            "smoothing_preference",
            "transition_cushion",
            "deep_breathing",
        ],
        "return" => vec![
            "regulation_strength",
            "smoothing_preference",
            "transition_cushion",
            "geom_drive",
            "exploration_noise",
            "deep_breathing",
        ],
        "resist" => vec![
            "regulation_strength",
            "smoothing_preference",
            "transition_cushion",
            "geom_curiosity",
            "geom_drive",
            "exploration_noise",
        ],
        "fissure" => vec![
            "regulation_strength",
            "smoothing_preference",
            "transition_cushion",
            "geom_curiosity",
            "geom_drive",
            "exploration_noise",
        ],
        _ => Vec::new(),
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(super) fn control_to_sensory(gesture: &str) -> Option<SensoryMsg> {
    let mut msg = SensoryMsg::Control {
        synth_gain: None,
        keep_bias: None,
        exploration_noise: None,
        fill_target: None,
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
        legacy_audio_synth: None,
        legacy_video_synth: None,
        pi_kp: None,
        pi_ki: None,
        pi_max_step: None,
    };
    match &mut msg {
        SensoryMsg::Control {
            exploration_noise,
            regulation_strength,
            deep_breathing,
            transition_cushion,
            smoothing_preference,
            geom_curiosity,
            geom_drive,
            ..
        } => match gesture {
            "soften" => {
                *regulation_strength = Some(0.78);
                *smoothing_preference = Some(0.35);
                *transition_cushion = Some(0.62);
                *geom_drive = Some(0.22);
                *exploration_noise = Some(0.02);
            },
            "widen" => {
                *smoothing_preference = Some(0.25);
                *geom_curiosity = Some(0.08);
                *geom_drive = Some(0.34);
                *exploration_noise = Some(0.04);
            },
            "hold" => {
                *regulation_strength = Some(0.82);
                *smoothing_preference = Some(0.45);
                *transition_cushion = Some(0.70);
                *deep_breathing = Some(true);
            },
            "return" => {
                *regulation_strength = Some(0.88);
                *smoothing_preference = Some(0.50);
                *transition_cushion = Some(0.78);
                *geom_drive = Some(0.18);
                *exploration_noise = Some(0.0);
                *deep_breathing = Some(true);
            },
            "resist" => {
                *regulation_strength = Some(0.74);
                *smoothing_preference = Some(0.24);
                *transition_cushion = Some(0.55);
                *geom_curiosity = Some(0.10);
                *geom_drive = Some(0.30);
                *exploration_noise = Some(0.035);
            },
            "fissure" => {
                *regulation_strength = Some(0.70);
                *smoothing_preference = Some(0.20);
                *transition_cushion = Some(0.48);
                *geom_curiosity = Some(0.12);
                *geom_drive = Some(0.32);
                *exploration_noise = Some(0.04);
            },
            _ => return None,
        },
        _ => return None,
    }
    Some(msg)
}

pub(super) fn max_abs(features: &[f32]) -> f32 {
    features.iter().map(|value| value.abs()).fold(0.0, f32::max)
}

pub(super) fn record_native_gesture(
    workspace: &Path,
    actor: &str,
    gesture: &str,
    label: Option<&str>,
    allowed: bool,
    reason: &str,
    ctx: &NextActionContext<'_>,
    semantic_features: &[f32],
    control_fields: &[String],
) {
    let native_dir = workspace.join("native_comm");
    let runtime_dir = workspace.join("runtime");
    let _ = fs::create_dir_all(&native_dir);
    let _ = fs::create_dir_all(&runtime_dir);
    let event = json!({
        "timestamp_unix_s": unix_now_s(),
        "actor": actor,
        "gesture": gesture,
        "label": label,
        "allowed": allowed,
        "reason": reason,
        "fill_pct": ctx.fill_pct,
        "stage": read_json(&workspace.join("health.json"))
            .get("stable_core")
            .and_then(|value| value.get("stage"))
            .cloned()
            .unwrap_or(Value::Null),
        "semantic_feature_max_abs": max_abs(semantic_features),
        "control_fields": control_fields,
    });
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(native_dir.join("gestures.jsonl"))
    {
        let _ = writeln!(file, "{}", event);
    }
    let status_path = runtime_dir.join("native_gesture_status.json");
    let status = read_json(&status_path);
    let send_count = status
        .get("send_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mark_count = status
        .get("mark_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let next_send_count = if allowed && !matches!(gesture, "mark" | "trace") {
        send_count.saturating_add(1)
    } else {
        send_count
    };
    let next_mark_count = if allowed && matches!(gesture, "mark" | "trace") {
        mark_count.saturating_add(1)
    } else {
        mark_count
    };
    let mut payload = status.as_object().cloned().unwrap_or_else(Map::new);
    payload.insert("status".to_string(), json!("active"));
    payload.insert("policy_version".to_string(), json!(1));
    payload.insert(
        "supported_gestures".to_string(),
        json!([
            "fissure", "hold", "mark", "resist", "return", "soften", "trace", "widen"
        ]),
    );
    payload.insert(
        "control_bearing_gestures".to_string(),
        json!(["fissure", "hold", "resist", "return", "soften", "widen"]),
    );
    payload.insert("mark_cooldown_secs".to_string(), json!(30));
    payload.insert("control_cooldown_secs".to_string(), json!(180));
    payload.insert("send_count".to_string(), json!(next_send_count));
    payload.insert("mark_count".to_string(), json!(next_mark_count));
    payload.insert(
        "last_block_reason".to_string(),
        if allowed { Value::Null } else { json!(reason) },
    );
    payload.insert("last_gesture".to_string(), event);
    if allowed && gesture == "resist" {
        let (lambda1_share, shoulder_share, tail_share) = lambda_shares(&ctx.telemetry.eigenvalues);
        let baseline = json!({
            "kind": "baseline",
            "policy": "resist_outcome_tracking_v1",
            "actor": actor,
            "label": label,
            "timestamp_unix_s": unix_now_s(),
            "pending_eval_until_unix_s": unix_now_s() + 120.0,
            "fill_pct": ctx.fill_pct,
            "stage": read_json(&workspace.join("health.json"))
                .get("stable_core")
                .and_then(|value| value.get("stage"))
                .cloned()
                .unwrap_or(Value::Null),
            "lambda1_share": lambda1_share,
            "shoulder_share": shoulder_share,
            "tail_share": tail_share,
        });
        payload.insert("last_resist_baseline".to_string(), baseline.clone());
        payload.insert("last_resist_evaluation".to_string(), Value::Null);
        payload.insert("last_snapback".to_string(), Value::Null);
        append_resist_outcome(workspace, &baseline);
    }
    let mut last_by_actor = payload
        .get("last_by_actor")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    last_by_actor.insert(
        actor.to_string(),
        json!({
            "last_at_unix_s": unix_now_s(),
            "last_gesture": gesture,
            "last_allowed": allowed,
            "last_reason": reason,
        }),
    );
    payload.insert("last_by_actor".to_string(), Value::Object(last_by_actor));
    let _ = fs::write(
        status_path,
        serde_json::to_string_pretty(&Value::Object(payload)).unwrap_or_else(|_| "{}".to_string()),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        control_to_sensory, max_abs, native_gesture_control, native_gesture_features,
        parse_native_gesture,
    };

    #[test]
    fn native_gesture_parser_defaults_to_mark() {
        let (gesture, label) = parse_native_gesture("");
        assert_eq!(gesture, "mark");
        assert_eq!(label, None);

        let (gesture, label) = parse_native_gesture("soften localized gravity");
        assert_eq!(gesture, "soften");
        assert_eq!(label.as_deref(), Some("localized gravity"));
    }

    #[test]
    fn native_gesture_features_are_ultra_cold() {
        for gesture in ["soften", "widen", "hold", "return", "resist", "fissure"] {
            let features = native_gesture_features(gesture);
            assert_eq!(features.len(), 48);
            assert!(max_abs(&features) <= 0.04);
            assert!(!native_gesture_control(gesture).is_empty());
            assert!(control_to_sensory(gesture).is_some());
        }
        let resist = native_gesture_features("resist");
        assert!(resist[0] < 0.0);
        assert!(resist[1] > 0.0);
        let fissure = native_gesture_features("fissure");
        assert!(fissure[0] < 0.0);
        assert!(fissure[3] > 0.0);
    }

    #[test]
    fn atlas_only_gestures_have_no_control_payload() {
        assert_eq!(native_gesture_features("mark"), vec![0.0_f32; 48]);
        assert!(native_gesture_control("trace").is_empty());
        assert!(control_to_sensory("trace").is_none());
    }
}
