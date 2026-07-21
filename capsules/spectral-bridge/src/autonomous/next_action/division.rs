use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

use astrid_minime_protocol::{
    DivisionActionAvailabilityV1, DivisionActionV1, DivisionCommandV1, DivisionLifecycleV1,
    DivisionStatusV1,
};
use chrono::Utc;

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};
const ACTIONS: &[&str] = &[
    "DIVISION_PREPARE",
    "DIVISION_STATUS",
    "DIVISION_ASSENT",
    "DIVISION_COMMIT",
    "DIVISION_ABORT",
    "DIVISION_ROLLBACK",
];

const DIVISION_REHEARSAL_OPERATOR_ENV: &str = "ASTRID_DIVISION_REHEARSAL_ENABLED";

fn rehearsal_dispatch_enabled() -> bool {
    rehearsal_dispatch_enabled_from(
        cfg!(feature = "division-rehearsal"),
        std::env::var(DIVISION_REHEARSAL_OPERATOR_ENV)
            .ok()
            .as_deref(),
    )
}

fn rehearsal_dispatch_enabled_from(feature_compiled: bool, operator_value: Option<&str>) -> bool {
    feature_compiled && matches!(operator_value, Some("true"))
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> Option<Result<(), String>> {
    if !ACTIONS.contains(&base_action) {
        return None;
    }
    if base_action == "DIVISION_STATUS" {
        render_status(conv, ctx);
        return Some(Ok(()));
    }

    if !rehearsal_dispatch_enabled() {
        let message = format!(
            "{base_action} is source-prepared but feature-disabled. A reviewed build must include the `division-rehearsal` feature and the operator must set {DIVISION_REHEARSAL_OPERATOR_ENV}=true; neither condition grants commit authority."
        );
        conv.emphasis = Some(message.clone());
        return Some(Err(message));
    }

    let raw = strip_action(original, base_action);
    let result = load_command(&raw, ctx)
        .and_then(|command| validate_command(base_action, command))
        .and_then(block_without_live_authority_adapter);
    Some(match result {
        Ok(never) => match never {},
        Err(message) => {
            let rendered = format!("Division ACTION blocked: {message}");
            conv.emphasis = Some(rendered.clone());
            Err(rendered)
        },
    })
}

fn render_status(conv: &mut ConversationState, ctx: &NextActionContext<'_>) {
    let workspace = minime_workspace(ctx);
    match read_status_value(&workspace) {
        Some((path, mut value)) => {
            let availability = serde_json::from_value::<DivisionStatusV1>(value.clone())
                .ok()
                .map(|status| status.action_availability_for("astrid"));
            if let (Some(object), Some(availability)) =
                (value.as_object_mut(), availability.as_ref())
            {
                object.insert(
                    "action_availability_for_astrid".to_string(),
                    serde_json::to_value(availability).unwrap_or_default(),
                );
            }
            let rendered = serde_json::to_string_pretty(&value).unwrap_or_default();
            conv.pending_file_listing = Some(rendered.clone());
            conv.push_receipt(
                "DIVISION_STATUS",
                vec![
                    format!("read versioned division status from {}", path.display()),
                    "read-only; no assent, commit, abort, rollback, or Control message sent"
                        .to_string(),
                ],
            );
            conv.emphasis = Some(match availability {
                Some(availability) => format!(
                    "Division status is attached from {}. {} Readiness evidence is advisory unless the status policy says ready.",
                    path.display(),
                    render_action_summary(&availability)
                ),
                None => format!(
                    "Division status is attached from {}, but it does not satisfy division.status.v1; no mutating ACTION should be selected until native status is upgraded.",
                    path.display()
                ),
            });
        },
        None => {
            conv.emphasis = Some(format!(
                "No division status exists yet under {}. Use ACTION_PREFLIGHT DIVISION_PREPARE <command.json> before sending a prepared command artifact.",
                workspace.display()
            ));
        },
    }
}

fn read_status_value(workspace: &Path) -> Option<(PathBuf, serde_json::Value)> {
    let candidates = [
        workspace.join("division/status.json"),
        workspace.join("division_status.json"),
    ];
    candidates.iter().find_map(|path| {
        let text = fs::read_to_string(path).ok()?;
        let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
        Some((path.clone(), value))
    })
}

fn read_status(workspace: &Path) -> Option<DivisionStatusV1> {
    let (_, value) = read_status_value(workspace)?;
    serde_json::from_value(value).ok()
}

fn action_name(action: DivisionActionV1) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{action:?}"))
}

fn render_action_summary(availability: &DivisionActionAvailabilityV1) -> String {
    let available = availability
        .available_actions
        .iter()
        .map(|entry| action_name(entry.action))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Available now: {available}. Recommended: {}.",
        action_name(availability.recommended_action)
    )
}

pub(super) fn prompt_note(workspace: Option<&Path>) -> Option<String> {
    prompt_note_with_gate(workspace, rehearsal_dispatch_enabled())
}

fn prompt_note_with_gate(workspace: Option<&Path>, dispatch_enabled: bool) -> Option<String> {
    if !dispatch_enabled {
        return None;
    }
    let workspace = workspace.map_or_else(
        || bridge_paths().minime_workspace().to_path_buf(),
        Path::to_path_buf,
    );
    let status = read_status(&workspace)?;
    if status.lifecycle == DivisionLifecycleV1::Idle {
        return None;
    }
    let availability = status.action_availability_for("astrid");
    let commit_blockers = availability
        .blocked_actions
        .iter()
        .find(|entry| entry.action == DivisionActionV1::DivisionCommit)
        .map(|entry| entry.reasons.join(", "))
        .unwrap_or_else(|| "none".to_string());
    Some(format!(
        "DIVISION ACTION CARD (current native lifecycle; high priority): lifecycle={:?}; {} Commit blockers: {commit_blockers}. Mutations require ACTION_PREFLIGHT plus the exact command artifact; native safety and authority checks remain decisive.",
        status.lifecycle,
        render_action_summary(&availability)
    ))
}

fn load_command(raw: &str, ctx: &NextActionContext<'_>) -> Result<DivisionCommandV1, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(
            "supply a JSON command artifact path or an inline JSON object; required identity, generation, digest, and expiry are never inferred"
                .to_string(),
        );
    }
    let text = if raw.starts_with('{') {
        raw.to_string()
    } else {
        let path = resolve_command_path(raw, ctx);
        fs::read_to_string(&path)
            .map_err(|err| format!("cannot read command artifact {}: {err}", path.display()))?
    };
    serde_json::from_str(&text).map_err(|err| format!("invalid division command JSON: {err}"))
}

fn resolve_command_path(raw: &str, ctx: &NextActionContext<'_>) -> PathBuf {
    let path = PathBuf::from(raw.trim_matches(|c| matches!(c, '"' | '\'' | '`')));
    if path.is_absolute() {
        path
    } else {
        ctx.workspace
            .unwrap_or_else(|| bridge_paths().bridge_workspace())
            .join(path)
    }
}

fn validate_command(
    base_action: &str,
    command: DivisionCommandV1,
) -> Result<DivisionCommandV1, String> {
    let expected = match base_action {
        "DIVISION_PREPARE" => DivisionActionV1::DivisionPrepare,
        "DIVISION_ASSENT" => DivisionActionV1::DivisionAssent,
        "DIVISION_COMMIT" => DivisionActionV1::DivisionCommit,
        "DIVISION_ABORT" => DivisionActionV1::DivisionAbort,
        "DIVISION_ROLLBACK" => DivisionActionV1::DivisionRollback,
        _ => return Err("unsupported division action".to_string()),
    };
    if command.action != expected {
        return Err(format!(
            "artifact action {:?} does not match {base_action}",
            command.action
        ));
    }
    let now = Utc::now().timestamp_millis().max(0) as u64;
    if !command.is_well_formed(now) {
        return Err("schema, source identity, digest, or expiry validation failed".to_string());
    }
    if !command.authority_shape_is_valid(now) {
        return Err(
            "action authority is invalid; commit/rollback require an exact one-shot operator capability and other actions must not smuggle one"
                .to_string(),
        );
    }
    if matches!(
        command.action,
        DivisionActionV1::DivisionPrepare
            | DivisionActionV1::DivisionAssent
            | DivisionActionV1::DivisionAbort
    ) && command.source.being != "astrid"
    {
        return Err("Astrid may only send her own prepare, assent, or abort command".to_string());
    }
    Ok(command)
}

fn block_without_live_authority_adapter(command: DivisionCommandV1) -> Result<Infallible, String> {
    Err(format!(
        "validated {} for division_id={}, but this source-prepared protocol has no live authority adapter; validation evidence cannot construct or dispatch authority",
        action_name(command.action),
        command.division_id
    ))
}

fn minime_workspace(ctx: &NextActionContext<'_>) -> PathBuf {
    ctx.workspace.map_or_else(
        || bridge_paths().minime_workspace().to_path_buf(),
        Path::to_path_buf,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn division_rehearsal_requires_both_compile_feature_and_operator_ack() {
        assert!(!rehearsal_dispatch_enabled_from(false, None));
        assert!(!rehearsal_dispatch_enabled_from(false, Some("true")));
        assert!(!rehearsal_dispatch_enabled_from(true, None));
        assert!(!rehearsal_dispatch_enabled_from(true, Some("1")));
        assert!(rehearsal_dispatch_enabled_from(true, Some("true")));
    }

    #[test]
    fn validated_division_evidence_cannot_dispatch_without_authority_adapter() {
        let command: DivisionCommandV1 = serde_json::from_value(serde_json::json!({
            "schema": "division.command.v1",
            "action": "DIVISION_PREPARE",
            "division_id": "division-source-prep-test",
            "idempotency_key": "prepare-source-prep-test",
            "expected_parent_generation": 7,
            "plan_digest": "bbbbbbbbbbbbbbbb",
            "source": {
                "being": "astrid",
                "process_identity": "spectral-bridge:test",
                "deployment_identity": "test-deployment"
            },
            "requested_at_unix_ms": 1,
            "expires_at_unix_ms": 2
        }))
        .expect("well-shaped source-prep command");

        let error = block_without_live_authority_adapter(command)
            .expect_err("evidence-only source prep must not dispatch");
        assert!(error.contains("no live authority adapter"));
        assert!(error.contains("cannot construct or dispatch authority"));
    }

    #[test]
    fn active_transaction_prompt_names_current_astrid_actions() {
        let root = tempfile::tempdir().unwrap();
        let division_dir = root.path().join("division");
        fs::create_dir_all(&division_dir).unwrap();
        fs::write(
            division_dir.join("status.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "division.status.v1",
                "division_id": "division-prompt-test",
                "parent_generation": 7,
                "plan_digest": "bbbbbbbbbbbbbbbb",
                "lifecycle": "shadowing",
                "parent_authoritative": true,
                "commit_feature_enabled": false,
                "selected_strategy": "input_recurrence",
                "astrid_assent": false,
                "minime_assent": true,
                "bridge_scale": 1.0,
                "current_tick": 600,
                "rollback_deadline_tick": null,
                "snapshot_refs": ["sha256:parent"],
                "readiness": {
                    "policy": "division.readiness.v1",
                    "ready": false,
                    "sample_count": 600,
                    "blocking_reasons": ["shadow_window_incomplete"],
                    "metrics_fresh": true,
                    "sensory_panic_streak": 0,
                    "actuator_saturation_streak": 0
                },
                "visual_evidence_advisory_only": true
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(prompt_note_with_gate(Some(root.path()), false).is_none());
        let note =
            prompt_note_with_gate(Some(root.path()), true).expect("active division prompt note");
        assert!(note.contains("DIVISION_ASSENT"));
        assert!(note.contains("DIVISION_ABORT"));
        assert!(note.contains("commit_feature_disabled"));
        assert!(note.contains("ACTION_PREFLIGHT"));
    }
}
