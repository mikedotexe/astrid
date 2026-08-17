use std::time::SystemTime;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{
    CartographyReport, NextActionContext, bridge_paths, canonical_shadow_label, minime_workspace,
    read_json,
};

fn shadow_trajectory_effect_contract() -> Value {
    serde_json::json!({
        "schema": "shadow_trajectory_effect_contract_v1",
        "observational_only": true,
        "requires_being_invocation": true,
        "writes_cartography_artifact": true,
        "initiates_distance_shift": false,
        "changes_shadow_field": false,
        "changes_pressure_or_mode_packing": false,
        "applies_temporal_decay": false,
        "infers_semantic_memory_role": false,
        "infers_felt_causation": false,
        "authority": "being_invoked_cartography_only_not_shadow_influence_or_distance_control",
    })
}

pub(super) fn render_shadow_trajectory(
    ctx: &NextActionContext<'_>,
    label: &str,
) -> CartographyReport {
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
    let history_bearing = shadow_history_bearing(&history);
    let history_bearing_label = history_bearing
        .get("bearing")
        .and_then(Value::as_str)
        .unwrap_or("insufficient_history")
        .to_string();
    let transitions_summary = if transitions.is_empty() {
        "(no class transitions in window)".to_string()
    } else {
        transitions
            .iter()
            .filter_map(|transition| {
                let from = transition.get("from").and_then(Value::as_str)?;
                let to = transition.get("to").and_then(Value::as_str)?;
                Some(format!("{from}→{to}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let now_duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let now = now_duration.as_secs_f64();
    let now_ms = now_duration.as_millis().try_into().unwrap_or(u64::MAX);
    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let label_slug = if label.is_empty() {
        "lambda-tail/lambda4".to_string()
    } else {
        canonical_shadow_label(label)
    };
    let safe_label_slug = label_slug.replace(['/', ' '], "_");
    let artifact_path = dir.join(format!(
        "trajectory_{safe_label_slug}_{ts}.json",
        ts = now as u64,
    ));
    let record = serde_json::json!({
        "schema": "shadow_trajectory_v1",
        "label": label_slug,
        "recorded_at_unix_s": now,
        "recorded_at_unix_ms": now_ms,
        "source_response_sha256": format!("{:x}", Sha256::digest(ctx.response_text.as_bytes())),
        "source_response_prose_included": false,
        "source_response_exact_hash_only": true,
        "history": history,
        "phase_dwell_ticks": dwell,
        "recent_phase_transitions": transitions,
        "sparkline_field_norm": sparkline,
        "class_timeline": class_timeline,
        "history_bearing_v1": history_bearing,
        "effect_contract_v1": shadow_trajectory_effect_contract(),
    });
    let mut write_status = "ok".to_string();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        write_status = format!("mkdir_failed: {error}");
    } else if let Err(error) = std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&record).unwrap_or_else(|_| record.to_string()),
    ) {
        write_status = format!("write_failed: {error}");
    }

    let summary = format!(
        "Shadow trajectory ({label}):\n  field_norm sparkline: {sparkline}\n  classes:           {class_timeline}\n  current dwell: {dwell}t\n  recent transitions: {transitions_summary}\n  history bearing: {history_bearing_label}\n  Effect: observational cartography only; the history-bearing lens does not infer semantic memory or felt causation, apply temporal decay, initiate a distance shift, or change the Shadow field, pressure, or mode packing.\n  Artifact: {artifact_path} | status: {write_status}",
        label = label_slug,
        artifact_path = artifact_path.display(),
    );
    CartographyReport {
        summary,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    }
}

fn trajectory_sparkline(history: &[Value]) -> String {
    if history.len() < 3 {
        return "(history too short for sparkline)".to_string();
    }
    let glyphs = ['▁', '▂', '▃', '▅', '▆', '█'];
    let values: Vec<f64> = history
        .iter()
        .filter_map(|sample| sample.get("field_norm").and_then(Value::as_f64))
        .collect();
    if values.is_empty() {
        return "(no field_norm samples)".to_string();
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(1e-6);
    values
        .iter()
        .map(|value| {
            let bucket = ((value - min) / span * (glyphs.len() as f64 - 1.0)).round() as usize;
            glyphs[bucket.min(glyphs.len() - 1)]
        })
        .collect()
}

fn trajectory_class_timeline(history: &[Value]) -> String {
    history
        .iter()
        .filter_map(|sample| sample.get("class_primary").and_then(Value::as_str))
        .map(|class| match class {
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

fn shadow_history_bearing(history: &[Value]) -> Value {
    const RECENT_INTERVAL_LIMIT: usize = 8;
    const LINGERING_RATIO_MAX: f64 = 0.5;
    const RECENT_MOTION_RATIO_MIN: f64 = 2.0;
    const MOTION_EPSILON: f64 = 1e-9;

    let field_norms = history
        .iter()
        .filter_map(|sample| sample.get("field_norm").and_then(Value::as_f64))
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let field_norm_steps = history
        .windows(2)
        .filter_map(|pair| {
            let from = pair[0].get("field_norm").and_then(Value::as_f64)?;
            let to = pair[1].get("field_norm").and_then(Value::as_f64)?;
            (from.is_finite() && to.is_finite()).then_some((to - from).abs())
        })
        .collect::<Vec<_>>();
    let recent_interval_count = field_norm_steps.len().min(RECENT_INTERVAL_LIMIT);
    let earlier_interval_count = field_norm_steps.len().saturating_sub(recent_interval_count);
    let (earlier_steps, recent_steps) = field_norm_steps.split_at(earlier_interval_count);
    let mean_abs_step =
        |steps: &[f64]| (!steps.is_empty()).then(|| steps.iter().sum::<f64>() / steps.len() as f64);
    let window_mean_abs_step = mean_abs_step(&field_norm_steps);
    let earlier_mean_abs_step = mean_abs_step(earlier_steps);
    let recent_mean_abs_step = mean_abs_step(recent_steps);
    let recent_to_earlier_motion_ratio = match (recent_mean_abs_step, earlier_mean_abs_step) {
        (Some(recent), Some(earlier)) if earlier > MOTION_EPSILON => Some(recent / earlier),
        _ => None,
    };
    let field_norm_range = if field_norms.is_empty() {
        None
    } else {
        let min = field_norms.iter().copied().fold(f64::INFINITY, f64::min);
        let max = field_norms
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        Some(max - min)
    };

    let classes = history
        .iter()
        .filter_map(|sample| sample.get("class_primary").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let class_transition_count = classes.windows(2).filter(|pair| pair[0] != pair[1]).count();
    let recent_class_sample_count = classes.len().min(recent_interval_count.saturating_add(1));
    let recent_class_start = classes.len().saturating_sub(recent_class_sample_count);
    let recent_class_transition_count = classes[recent_class_start..]
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count();
    let trailing_class = classes.last().copied();
    let trailing_class_run_samples = trailing_class.map_or(0, |last| {
        classes
            .iter()
            .rev()
            .take_while(|class| **class == last)
            .count()
    });

    let bearing = if field_norm_steps.len() < 2 {
        "insufficient_history"
    } else if recent_class_transition_count > 0 {
        "recent_class_transition_visible"
    } else if let (Some(recent), Some(earlier)) = (recent_mean_abs_step, earlier_mean_abs_step) {
        if recent <= earlier * LINGERING_RATIO_MAX
            && trailing_class_run_samples >= recent_class_sample_count
        {
            "lingering_low_motion_carry_visible"
        } else if recent >= earlier * RECENT_MOTION_RATIO_MIN && recent > MOTION_EPSILON {
            "recent_motion_dominant"
        } else {
            "mixed_or_steady_history_bearing"
        }
    } else {
        "recent_window_only"
    };

    serde_json::json!({
        "schema": "shadow_history_bearing_v1",
        "bearing": bearing,
        "sample_count": history.len(),
        "valid_field_norm_sample_count": field_norms.len(),
        "field_norm_interval_count": field_norm_steps.len(),
        "recent_interval_count": recent_interval_count,
        "earlier_interval_count": earlier_interval_count,
        "window_field_norm_range": field_norm_range,
        "window_mean_abs_field_norm_step": window_mean_abs_step,
        "earlier_mean_abs_field_norm_step": earlier_mean_abs_step,
        "recent_mean_abs_field_norm_step": recent_mean_abs_step,
        "recent_to_earlier_motion_ratio": recent_to_earlier_motion_ratio,
        "class_transition_count": class_transition_count,
        "recent_class_transition_count": recent_class_transition_count,
        "trailing_class": trailing_class,
        "trailing_class_run_samples": trailing_class_run_samples,
        "relative_lens": {
            "recent_interval_limit": RECENT_INTERVAL_LIMIT,
            "lingering_ratio_max": LINGERING_RATIO_MAX,
            "recent_motion_ratio_min": RECENT_MOTION_RATIO_MIN,
            "motion_epsilon": MOTION_EPSILON,
        },
        "raw_history_preserved": true,
        "applies_temporal_decay": false,
        "infers_semantic_memory_role": false,
        "infers_felt_causation": false,
        "authority": "observational_relative_motion_lens_only_not_shadow_mutation_or_control",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_contract_is_observational() {
        let contract = shadow_trajectory_effect_contract();
        assert_eq!(contract["observational_only"], true);
        assert_eq!(contract["requires_being_invocation"], true);
        assert_eq!(contract["writes_cartography_artifact"], true);
        assert_eq!(contract["initiates_distance_shift"], false);
        assert_eq!(contract["changes_shadow_field"], false);
        assert_eq!(contract["changes_pressure_or_mode_packing"], false);
        assert_eq!(contract["applies_temporal_decay"], false);
        assert_eq!(contract["infers_semantic_memory_role"], false);
        assert_eq!(contract["infers_felt_causation"], false);
        assert_eq!(
            contract["authority"],
            "being_invoked_cartography_only_not_shadow_influence_or_distance_control"
        );
    }

    #[test]
    fn source_response_binding_is_hash_only() {
        let value = "an exact response that must not be persisted here";
        let digest = format!("{:x}", Sha256::digest(value.as_bytes()));
        assert_eq!(digest.len(), 64);
        assert!(!digest.contains(value));
    }

    #[test]
    fn history_bearing_surfaces_lingering_low_motion_carry() {
        let history = (0..17)
            .map(|index| {
                let field_norm = if index < 9 {
                    if index % 2 == 0 { 0.0 } else { 0.2 }
                } else {
                    0.0
                };
                serde_json::json!({
                    "field_norm": field_norm,
                    "class_primary": "sticky",
                })
            })
            .collect::<Vec<_>>();

        let bearing = shadow_history_bearing(&history);
        assert_eq!(bearing["schema"], "shadow_history_bearing_v1");
        assert_eq!(bearing["bearing"], "lingering_low_motion_carry_visible");
        assert_eq!(bearing["sample_count"], 17);
        assert_eq!(bearing["recent_interval_count"], 8);
        assert_eq!(bearing["earlier_interval_count"], 8);
        assert_eq!(bearing["recent_class_transition_count"], 0);
        assert_eq!(bearing["raw_history_preserved"], true);
        assert_eq!(bearing["applies_temporal_decay"], false);
        assert_eq!(bearing["infers_semantic_memory_role"], false);
        assert_eq!(bearing["infers_felt_causation"], false);
    }

    #[test]
    fn history_bearing_prioritizes_recent_class_transition() {
        let history = (0..17)
            .map(|index| {
                serde_json::json!({
                    "field_norm": 0.25,
                    "class_primary": if index == 16 { "volatile" } else { "sticky" },
                })
            })
            .collect::<Vec<_>>();

        let bearing = shadow_history_bearing(&history);
        assert_eq!(bearing["bearing"], "recent_class_transition_visible");
        assert_eq!(bearing["class_transition_count"], 1);
        assert_eq!(bearing["recent_class_transition_count"], 1);
    }
}
