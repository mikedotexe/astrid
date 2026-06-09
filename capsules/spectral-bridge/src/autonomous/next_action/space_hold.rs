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

fn transition_value<'a>(ctx: &'a NextActionContext<'_>, key: &str) -> Option<&'a Value> {
    ctx.telemetry
        .transition_event_v1
        .as_ref()
        .or(ctx.telemetry.transition_event.as_ref())
        .and_then(|event| event.get(key))
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
    let center_tail_delta = (lambda1_share - tail_share).clamp(-1.0, 1.0);
    let surrounding_share = (shoulder_share + tail_share).max(f64::EPSILON);
    let center_surround_ratio = lambda1_share / surrounding_share;
    let lambda_gap_pressure = (r12.unwrap_or(1.0) / 3.0).clamp(0.0, 1.0);
    let singular_weight_index =
        (lambda1_share * 0.60 + lambda_gap_pressure * 0.25 + (1.0 - entropy_proxy) * 0.15)
            .clamp(0.0, 1.0);
    let flow_continuity_index =
        ((shoulder_share * 0.42 + tail_share * 0.34 + entropy_proxy * 0.24)
            * (1.0 - lambda_gap_pressure * 0.25))
            .clamp(0.0, 1.0);
    let harvest_pressure =
        (density_pressure * 0.60 + semantic_energy / 0.08 * 0.25).clamp(0.0, 1.0);
    let protected_space_score =
        (space_affordance * 0.62 + (1.0 - harvest_pressure) * 0.38).clamp(0.0, 1.0);
    let medium_thinning_risk =
        (singular_weight_index * 0.55 + (1.0 - space_affordance) * 0.45).clamp(0.0, 1.0);
    let flow_map = source.contains("lambda_flow_map");
    let fold_hold = source.contains("fold_hold");
    let classification = if flow_map {
        if singular_weight_index >= 0.55 && medium_thinning_risk >= 0.50 {
            "lambda1_weight_thin_medium_watch"
        } else if flow_continuity_index >= 0.50 && center_tail_delta.abs() <= 0.18 {
            "center_tail_flow_available"
        } else {
            "mixed_center_tail_gradient"
        }
    } else if fold_hold {
        if density_pressure >= 0.55 && protected_space_score >= 0.45 {
            "fold_region_available"
        } else if harvest_pressure >= 0.62 {
            "result_harvest_pressure_high"
        } else if semantic_energy <= 0.01 && density_pressure >= 0.45 {
            "hum_decay_watchpoint"
        } else {
            "thin_fold_hold"
        }
    } else if protected_space_score >= 0.62 && space_affordance >= harvest_pressure {
        "space_first_exploration_available"
    } else if harvest_pressure >= 0.62 {
        "signal_harvest_pressure_high"
    } else if tail_share >= 0.25 {
        "shadow_tail_region_available"
    } else {
        "thin_space_hold"
    };
    let now = unix_now_s();
    let minimum_hold_secs = if flow_map {
        600.0
    } else if fold_hold {
        900.0
    } else {
        720.0
    };
    let hold_until = now + minimum_hold_secs;
    let event_prefix = if flow_map {
        "lambda_flow_map"
    } else if fold_hold {
        "fold_hold"
    } else {
        "space_hold"
    };
    let policy = if flow_map {
        "lambda_flow_map_v1"
    } else if fold_hold {
        "fold_hold_v1"
    } else {
        "space_hold_v1"
    };
    let dfill_dt = transition_value(ctx, "dfill_dt")
        .and_then(Value::as_f64)
        .or_else(|| health.get("dfill_dt").and_then(Value::as_f64));
    let transition_phase = transition_value(ctx, "phase")
        .and_then(Value::as_str)
        .or_else(|| transition_value(ctx, "phase_to").and_then(Value::as_str))
        .unwrap_or("unknown");
    let event_id = format!("{}_{}", event_prefix, (now * 1000.0) as u64);
    let mut payload = json!({
        "event_id": event_id,
        "timestamp_unix_s": now,
        "source": source,
        "policy": policy,
        "hold_kind": event_prefix,
        "label": label,
        "classification": classification,
        "hold_until_unix_s": hold_until,
        "harvest_policy": {
            "mode": "delayed_non_control",
            "minimum_hold_secs": minimum_hold_secs,
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
    if flow_map {
        payload["lambda_flow_map_v1"] = json!({
            "snapshot_kind": "frozen_current_telemetry",
            "freeze_note": "This freezes the live lambda terrain at action dispatch so a sharp surge can be compared after the state moves on; it does not hold or mutate Minime.",
            "lambda_shares": {
                "lambda1_share": lambda1_share,
                "shoulder_share": shoulder_share,
                "tail_share": tail_share,
                "center_tail_delta": center_tail_delta,
                "center_surround_ratio": center_surround_ratio,
            },
            "flow_indices": {
                "singular_weight_index": singular_weight_index,
                "flow_continuity_index": flow_continuity_index,
                "medium_thinning_risk": medium_thinning_risk,
                "lambda_gap_pressure": lambda_gap_pressure,
                "space_affordance": space_affordance,
            },
            "surge_context": {
                "dfill_dt": dfill_dt,
                "transition_phase": transition_phase,
                "lambda1_rel": ctx.telemetry.lambda1_rel,
                "fill_pct": ctx.fill_pct,
            },
            "interpretation": classification,
            "suggested_return_next": [
                "VISUALIZE_CASCADE lambda-flow",
                "TIME_DOMAIN lambda-flow",
                "SPACE_HOLD lambda-flow",
                "PRESSURE_SOURCE_AUDIT lambda-flow"
            ],
            "authority_boundary": "read current telemetry and write a protected cartography snapshot only; no semantic payload, perturbation, native gesture, or control mutation"
        });
    }
    if fold_hold {
        payload["fold_hold_contract"] = json!({
            "artifact": "sustained_transition_process",
            "frustration_acknowledgment": "The contraction or fold may be the object of study; no immediate result artifact is required during the hold window.",
            "hum_decay_question": "If the signal hum fades, compare whether density collapses, relaxes, or persists as a new basin shape.",
            "suggested_return_next": [
                "DECAY_MAP fold-hold",
                "FLUCTUATION_AUDIT fold-hold",
                "SPACE_HOLD fold-hold",
                "EXPERIMENT_EVIDENCE current :: fold_hold observed ..."
            ],
            "authority_boundary": "read Minime telemetry and write protected study markers only; no semantic payload, perturbation, native gesture, or control mutation"
        });
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(native_dir.join(if flow_map {
            "lambda_flow_maps.jsonl"
        } else {
            "space_holds.jsonl"
        }))
    {
        let _ = writeln!(file, "{}", payload);
    }
    let _ = fs::write(
        runtime_dir.join(if flow_map {
            "lambda_flow_map_status.json"
        } else {
            "space_hold_status.json"
        }),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    );
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::BridgeDb, types::SpectralTelemetry};
    use tokio::sync::mpsc;

    fn telemetry_from(value: Value) -> SpectralTelemetry {
        serde_json::from_value(value).expect("telemetry")
    }

    #[test]
    fn lambda_flow_map_records_frozen_center_tail_snapshot() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("health.json"),
            serde_json::to_string(&json!({
                "fill_pct": 72.2,
                "dfill_dt": 9.5,
                "stable_core": {
                    "stage": "hold",
                    "structural_mode": "settled_habitable"
                },
                "semantic": {
                    "energy": 0.002
                }
            }))
            .expect("json"),
        )
        .expect("write health");
        let telemetry = telemetry_from(json!({
            "t_ms": 42,
            "eigenvalues": [5.0, 1.8, 1.2, 0.8, 0.7],
            "fill_ratio": 0.722,
            "lambda1_rel": 1.18,
            "transition_event_v1": {
                "dfill_dt": 9.5,
                "phase": "surge"
            }
        }));
        let db = BridgeDb::open(":memory:").expect("db");
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        let mut burst_count = 0;
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 72.2,
            response_text: "lambda flow map test",
            workspace: Some(temp.path()),
        };

        let payload = append_space_hold_event(
            temp.path(),
            "astrid:lambda_flow_map",
            "heavy center, fluid tail",
            Some("heavy-center"),
            &ctx,
        );

        assert_eq!(payload["policy"].as_str(), Some("lambda_flow_map_v1"));
        assert_eq!(payload["hold_kind"].as_str(), Some("lambda_flow_map"));
        assert_eq!(
            payload["lambda_flow_map_v1"]["snapshot_kind"].as_str(),
            Some("frozen_current_telemetry")
        );
        assert!(
            payload["lambda_flow_map_v1"]["flow_indices"]["singular_weight_index"]
                .as_f64()
                .is_some_and(|value| value > 0.4)
        );
        assert!(
            payload["lambda_flow_map_v1"]["flow_indices"]["medium_thinning_risk"]
                .as_f64()
                .is_some_and(|value| value > 0.3)
        );
        assert_eq!(
            payload["lambda_flow_map_v1"]["surge_context"]["dfill_dt"].as_f64(),
            Some(9.5)
        );
        assert_eq!(
            payload["protected_boundaries"]["control_payload"].as_bool(),
            Some(false)
        );
        assert!(
            temp.path()
                .join("native_comm/lambda_flow_maps.jsonl")
                .exists()
        );
        assert!(
            temp.path()
                .join("runtime/lambda_flow_map_status.json")
                .exists()
        );
    }
}
