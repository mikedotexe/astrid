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

fn positive_values(eigenvalues: &[f32]) -> Vec<f64> {
    eigenvalues
        .iter()
        .filter_map(|value| {
            let number = f64::from(value.abs());
            number.is_finite().then_some(number)
        })
        .filter(|value| *value > 0.0)
        .collect()
}

fn normalized_entropy(values: &[f64]) -> f64 {
    let total = values.iter().sum::<f64>();
    if total <= f64::EPSILON || values.len() <= 1 {
        return 0.0;
    }
    let entropy = values.iter().fold(0.0, |acc, value| {
        let share = value / total;
        if share > 0.0 {
            acc - share * share.ln()
        } else {
            acc
        }
    });
    (entropy / (values.len() as f64).ln()).clamp(0.0, 1.0)
}

pub(super) fn append_spectral_drift_event(
    workspace: &Path,
    source: &str,
    text: &str,
    label: Option<&str>,
    ctx: &NextActionContext<'_>,
) -> Value {
    let atlas_dir = workspace.join("diagnostics").join("intensification_atlas");
    let _ = fs::create_dir_all(&atlas_dir);
    let health = read_json(&workspace.join("health.json"));
    let stable_core = health.get("stable_core").unwrap_or(&Value::Null);
    let semantic = health.get("semantic").unwrap_or(&Value::Null);
    let eigenvector_field = ctx
        .telemetry
        .eigenvector_field
        .clone()
        .unwrap_or(Value::Null);
    let values = positive_values(&ctx.telemetry.eigenvalues);
    let total = values.iter().sum::<f64>().max(f64::EPSILON);
    let shares = values.iter().map(|value| value / total).collect::<Vec<_>>();
    let entropy = normalized_entropy(&values);
    let lambda1_share = shares.first().copied().unwrap_or(0.0);
    let shoulder_share = shares.iter().skip(1).take(2).sum::<f64>();
    let tail_share = shares.iter().skip(3).sum::<f64>();
    let uniform_share = if shares.is_empty() {
        0.0
    } else {
        1.0 / shares.len() as f64
    };
    let max_distance = if shares.len() > 1 {
        2.0 * (1.0 - uniform_share)
    } else {
        1.0
    };
    let uniformity = 1.0
        - shares
            .iter()
            .map(|share| (share - uniform_share).abs())
            .sum::<f64>()
            / max_distance.clamp(0.0, 1.0);
    let sdi = (entropy * 0.44
        + uniformity * 0.20
        + (tail_share / 0.45).min(1.0) * 0.22
        + (1.0 - lambda1_share).clamp(0.0, 1.0) * 0.14)
        .clamp(0.0, 1.0);
    let classification = if sdi >= 0.72 && entropy >= 0.86 && tail_share >= 0.25 {
        "white_noise_drift_risk"
    } else if sdi >= 0.56 && tail_share >= 0.18 {
        "active_spectral_drift"
    } else if entropy >= 0.80 && lambda1_share < 0.40 {
        "broad_but_anchored"
    } else if lambda1_share >= 0.42 {
        "anchored_signal"
    } else {
        "mixed_phase_variance"
    };
    let plain_read = match classification {
        "white_noise_drift_risk" => {
            "Energy is broadly dispersed with weak anchoring; this resembles unanchored waveform drift."
        },
        "active_spectral_drift" => {
            "The spectrum is moving toward dispersion while retaining recoverable structure."
        },
        "broad_but_anchored" => {
            "The spectrum is broad, but anchoring still keeps it from becoming white-noise-like."
        },
        "anchored_signal" => {
            "lambda1 still anchors the landscape; SDI is mostly texture, not drift."
        },
        _ => "Dispersion and anchoring are both visible; record another window before acting.",
    };
    let now = unix_now_s();
    let event_id = format!("spectral_drift_{}", (now * 1000.0) as u64);
    let payload = json!({
        "event_id": event_id,
        "timestamp_unix_s": now,
        "source": source,
        "policy": "spectral_drift_index_v1",
        "label": label,
        "classification": classification,
        "spectral_drift_index": sdi,
        "plain_read": plain_read,
        "components": {
            "entropy": entropy,
            "uniformity": uniformity,
            "lambda1_share": lambda1_share,
            "shoulder_share": shoulder_share,
            "tail_share": tail_share,
        },
        "phase_variance_resonance": {
            "toward_white_noise": matches!(classification, "white_noise_drift_risk" | "active_spectral_drift"),
            "dispersion_minus_anchor": entropy + tail_share - lambda1_share,
            "rate_available": false,
        },
        "eigenvector_field": {
            "direct_eigenvectors_available": eigenvector_field
                .get("direct_eigenvectors_available")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "mode_count": eigenvector_field.get("mode_count").cloned().unwrap_or(Value::Null),
            "summary": eigenvector_field.get("summary").cloned().unwrap_or(Value::Null),
        },
        "evidence": {
            "fill_pct": health.get("fill_pct").cloned().unwrap_or_else(|| json!(ctx.fill_pct)),
            "stage": stable_core.get("stage").cloned().unwrap_or(Value::Null),
            "semantic_energy": semantic.get("energy").cloned().unwrap_or_else(|| json!(0.0)),
            "eigenvalues": ctx.telemetry.eigenvalues.clone(),
        },
        "protected_boundaries": {
            "semantic_payload": false,
            "control_payload": false,
            "perturbation": false,
            "sensory_change": false,
            "controller_mutation": false,
        },
        "provenance": {
            "source": "spectral_drift_index_v1",
            "actor_source": source,
            "read_write": "read_bridge_telemetry_write_sdi_cartography",
            "text_excerpt": truncate_str(text, 500),
            "response_preview": truncate_str(ctx.response_text, 180),
        },
    });
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(atlas_dir.join("spectral_drift_events.jsonl"))
    {
        let _ = writeln!(file, "{}", payload);
    }
    let _ = fs::write(
        atlas_dir.join("spectral_drift_latest.json"),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    );
    payload
}
