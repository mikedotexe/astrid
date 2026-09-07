use std::time::SystemTime;

use serde_json::Value;

use super::{CartographyReport, NextActionContext, bridge_paths, minime_workspace, read_json};

pub(super) fn render_shadow_response(ctx: &NextActionContext<'_>, intent_query: &str) -> String {
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
            .find(|response| {
                response
                    .get("intent_id")
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
    let class_segment = if class_changed {
        format!(", classification {class_from}→{class_to}")
    } else {
        format!(", classification stayed {class_from}")
    };
    let direction = if delta >= 0.0 { "+" } else { "" };
    format!(
        "Shadow influence response (intent {intent_id}, label {label}, stage {stage}):\n  Pre→Post field_norm delta: {direction}{delta:.4}\n  Basin shift score: {basin:.3} (1.0 = field totally rearranged, 0.0 = unchanged){class_segment}\n  Applied: rms={applied_rms:.4}, max_abs={applied_max:.4} over {total_ticks} ticks.\n  The shadow remembers."
    )
}

pub(super) fn render_shadow_dialogue(ctx: &NextActionContext<'_>) -> CartographyReport {
    let workspace = minime_workspace(ctx);
    let minime_health = read_json(&workspace.join("health.json")).unwrap_or(Value::Null);
    let minime_shadow = minime_health
        .get("shadow_field_v3")
        .cloned()
        .unwrap_or(Value::Null);
    let astrid_shadow = read_json(&workspace.join("astrid_shadow_v3.json")).unwrap_or(Value::Null);

    let minime_class = minime_shadow
        .get("class_v3")
        .and_then(|class| class.get("primary"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let astrid_class = astrid_shadow
        .get("class_v3")
        .and_then(|class| class.get("primary"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let minime_norm = minime_shadow
        .get("v2")
        .and_then(|value| value.get("field_norm"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let astrid_norm = astrid_shadow
        .get("v2")
        .and_then(|value| value.get("field_norm"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let minime_eligible = minime_shadow
        .get("v2")
        .and_then(|value| value.get("influence_eligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let astrid_eligible = astrid_shadow
        .get("v2")
        .and_then(|value| value.get("influence_eligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let alignment = if minime_class == astrid_class {
        format!("Both shadows share the same primary class: {minime_class}.")
    } else {
        format!("Shadows diverge: minime={minime_class}, yours={astrid_class}.")
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64());
    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let artifact_path = dir.join(format!("dialogue_{ts}.json", ts = now as u64));
    let record = serde_json::json!({
        "schema": "shadow_dialogue_v1",
        "recorded_at_unix_s": now,
        "minime_shadow_v3": minime_shadow,
        "astrid_shadow_v3": astrid_shadow,
        "alignment_summary": alignment,
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

pub(super) fn render_shadow_coupling(
    ctx: &NextActionContext<'_>,
    scope: &str,
) -> CartographyReport {
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
        .map_or(0.0, |duration| duration.as_secs_f64());
    let dir = bridge_paths().bridge_workspace().join("shadow_cartography");
    let artifact_path = dir.join(format!("coupling_{scope}_{ts}.json", ts = now as u64,));
    let record = serde_json::json!({
        "schema": "shadow_coupling_v1",
        "scope": scope,
        "recorded_at_unix_s": now,
        "minime_mode_partners": minime_shadow.get("mode_partners").cloned().unwrap_or(Value::Null),
        "astrid_mode_partners": astrid_shadow.get("mode_partners").cloned().unwrap_or(Value::Null),
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

    let lines = match (minime_line, astrid_line) {
        (Some(minime), Some(astrid)) => format!("{minime}\n  {astrid}"),
        (Some(minime), None) => minime,
        (None, Some(astrid)) => astrid,
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
