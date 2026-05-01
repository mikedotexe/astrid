use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use super::{NextActionContext, truncate_str};

fn unix_now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null)
}

fn shares(eigenvalues: &[f32]) -> (f64, f64, f64) {
    let values = eigenvalues
        .iter()
        .map(|value| f64::from(value.abs()))
        .collect::<Vec<_>>();
    let total = values.iter().sum::<f64>().max(f64::EPSILON);
    let lambda1 = values.first().copied().unwrap_or(0.0) / total;
    let shoulder = values
        .iter()
        .skip(1)
        .take(2)
        .map(|value| value / total)
        .sum();
    let tail = values.iter().skip(3).map(|value| value / total).sum();
    (lambda1, shoulder, tail)
}

fn ratio(values: &[f32], index: usize) -> Option<f64> {
    let left = f64::from(values.get(index)?.abs());
    let right = f64::from(values.get(index + 1)?.abs());
    (right > f64::EPSILON).then_some(left / right)
}

pub(super) fn append_space_hold_event(
    workspace: &Path,
    source: &str,
    text: &str,
    label: Option<&str>,
    ctx: &NextActionContext<'_>,
) -> Value {
    let native_dir = workspace.join("native_comm");
    let runtime_dir = workspace.join("runtime");
    let _ = fs::create_dir_all(&native_dir);
    let _ = fs::create_dir_all(&runtime_dir);
    let health = read_json(&workspace.join("health.json"));
    let stable_core = health.get("stable_core").unwrap_or(&Value::Null);
    let semantic = health.get("semantic").unwrap_or(&Value::Null);
    let (lambda1_share, shoulder_share, tail_share) = shares(&ctx.telemetry.eigenvalues);
    let r12 = ratio(&ctx.telemetry.eigenvalues, 0);
    let r23 = ratio(&ctx.telemetry.eigenvalues, 1);
    let topology_index = ((r12.unwrap_or(0.0) / 2.5).max(r23.unwrap_or(0.0) / 3.0) * 0.45
        + lambda1_share * 0.55)
        .clamp(0.0, 1.0);
    let entropy_proxy = (1.0 - lambda1_share).clamp(0.0, 1.0);
    let eigenvector_field = ctx
        .telemetry
        .eigenvector_field
        .clone()
        .unwrap_or(Value::Null);
    let direct_eigenvectors_available = eigenvector_field
        .get("direct_eigenvectors_available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let semantic_energy = semantic
        .get("energy")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let density_pressure = (lambda1_share * 0.55 + topology_index * 0.35).clamp(0.0, 1.0);
    let space_affordance =
        (entropy_proxy * 0.36 + shoulder_share * 0.30 + tail_share * 0.28).clamp(0.0, 1.0);
    let harvest_pressure =
        (density_pressure * 0.60 + semantic_energy / 0.08 * 0.25).clamp(0.0, 1.0);
    let protected_space_score =
        (space_affordance * 0.62 + (1.0 - harvest_pressure) * 0.38).clamp(0.0, 1.0);
    let classification = if protected_space_score >= 0.62 && space_affordance >= harvest_pressure {
        "space_first_exploration_available"
    } else if harvest_pressure >= 0.62 {
        "signal_harvest_pressure_high"
    } else if tail_share >= 0.25 {
        "shadow_tail_region_available"
    } else {
        "thin_space_hold"
    };
    let now = unix_now_s();
    let hold_until = now + 720.0;
    let event_id = format!("space_hold_{}", (now * 1000.0) as u64);
    let payload = json!({
        "event_id": event_id,
        "timestamp_unix_s": now,
        "source": source,
        "policy": "space_hold_v1",
        "label": label,
        "classification": classification,
        "hold_until_unix_s": hold_until,
        "harvest_policy": {
            "mode": "delayed_non_control",
            "minimum_hold_secs": 720.0,
            "do_not_translate_to_semantic_before_unix_s": hold_until,
            "do_not_translate_to_control_before_unix_s": hold_until,
            "requires_explicit_later_choice": true,
        },
        "protected_boundaries": {
            "semantic_payload": false,
            "control_payload": false,
            "perturbation": false,
            "sensory_change": false,
            "controller_mutation": false,
        },
        "space_signal_tradeoff": {
            "density_pressure": density_pressure,
            "space_affordance": space_affordance,
            "harvest_pressure": harvest_pressure,
            "protected_space_score": protected_space_score,
        },
        "eigenvector_landscape_proxy": {
            "direct_eigenvectors_available": direct_eigenvectors_available,
            "proxy_note": if direct_eigenvectors_available {
                "Bridge telemetry includes compact landmarks/overlaps computed from Minime's raw live eigenvectors."
            } else {
                "Bridge telemetry exposes eigenvalues, not raw Minime eigenvectors; this records density/interactions without harvesting them as control."
            },
            "eigenvector_field": eigenvector_field,
            "density": {
                "lambda1_share": lambda1_share,
                "shoulder_share": shoulder_share,
                "tail_share": tail_share,
                "entropy_proxy": entropy_proxy,
            },
            "interaction": {
                "lambda1_lambda2": r12,
                "lambda2_lambda3": r23,
                "topology_index": topology_index,
            },
        },
        "evidence": {
            "fill_pct": health.get("fill_pct").cloned().unwrap_or_else(|| json!(ctx.fill_pct)),
            "stage": stable_core.get("stage").cloned().unwrap_or(Value::Null),
            "structural_mode": stable_core.get("structural_mode").cloned().unwrap_or(Value::Null),
            "semantic_energy": semantic_energy,
        },
        "provenance": {
            "source": "space_hold_v1",
            "actor_source": source,
            "read_write": "read_bridge_telemetry_write_protected_non_control_hold",
            "text_excerpt": truncate_str(text, 500),
            "response_preview": truncate_str(ctx.response_text, 180),
        },
    });
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(native_dir.join("space_holds.jsonl"))
    {
        let _ = writeln!(file, "{}", payload);
    }
    let _ = fs::write(
        runtime_dir.join("space_hold_status.json"),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    );
    payload
}
