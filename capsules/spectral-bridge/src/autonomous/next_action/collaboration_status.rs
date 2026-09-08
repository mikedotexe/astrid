use super::collaboration::*;
use super::{ConversationState, strip_action};
use serde_json::Value;
use std::path::Path;
use tracing::{info, warn};

pub(super) fn handle(conv: &mut ConversationState, original: &str, base_action: &str) -> bool {
    let arg = strip_action(original, base_action).trim().to_string();
    let target = if arg.is_empty() { "latest" } else { &arg };
    match read(target) {
        Ok((summary, collab_id)) => {
            info!(target: "v5_collab", "COLLABORATION_STATUS: rendered {} chars", summary.len());
            super::collaboration_attention::mark_explicit_inspection(
                &mut conv.collaboration_prompt_checkpoint,
                &collab_id,
            );
            conv.emphasis = Some(summary);
        },
        Err(error) => {
            warn!(target: "v5_collab", %error, "COLLABORATION_STATUS failed");
            conv.emphasis = Some(format!("(collaboration status failed: {error})"));
        },
    }
    true
}

/// Explicit, read-only pull of one room. Unlike the former ambient suffix,
/// this is rendered only after Astrid chooses `COLLABORATION_STATUS`.
fn read(target: &str) -> Result<(String, String), String> {
    let meta = find_meta(target)?;
    if meta.inviter != ASTRID_NAME && meta.invitee != ASTRID_NAME {
        return Err(format!("Astrid is not a participant in {}", meta.id));
    }
    let dir = collab_dir(&meta.id);
    let state_path = dir.join("chamber_state.json");
    let state = std::fs::read_to_string(&state_path)
        .map_err(|error| format!("read {}: {error}", state_path.display()))?;
    let state: Value = serde_json::from_str(&state)
        .map_err(|error| format!("parse {}: {error}", state_path.display()))?;
    let summary = render(
        &meta,
        &state,
        &render_recent_shared_thoughts(&meta.id, 5),
        read_collab_reservoir_state_cached(&format!("collab_{}", meta.id))
            .as_ref()
            .map(render_joint_trace_clause)
            .as_deref()
            .unwrap_or(""),
        &state_path,
    );
    Ok((summary, meta.id))
}

fn render(
    meta: &CollaborationMeta,
    state: &Value,
    recent_thoughts: &str,
    reservoir_clause: &str,
    state_path: &Path,
) -> String {
    let phase = state
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let projection = state.get("attention_projection_v1");
    let projection_text = projection
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| {
            "(attention_projection_v1 unavailable; no delivery revision can be inferred)"
                .to_string()
        });
    let chamber = render_chamber_state_value(state);
    let thoughts = if recent_thoughts.is_empty() {
        "(none)"
    } else {
        recent_thoughts
    };
    let chamber = if chamber.is_empty() {
        "(no prompt summary available)"
    } else {
        chamber.as_str()
    };
    format!(
        "Collaboration status (explicit read-only pull)\n\
id: {id}\n\
topic: {topic}\n\
status: {status}\n\
members: {members}\n\
phase: {phase}\n\
joint trace:{reservoir_clause}\n\
recent shared thoughts: {thoughts}\n\
current chamber: {chamber}\n\
attention_projection_v1:\n{projection_text}\n\
source: {source}\n\
correspondence: separate protected sender-bound lane; the global ledger is not attributed to this room.\n\
authority: language context only, not control; silence is neutral and does not imply receipt, uptake, assent, or felt state.",
        id = meta.id,
        topic = meta.topic,
        status = meta.status,
        members = meta.members.join(","),
        source = state_path.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_explicit_and_preserves_source_boundaries() {
        let meta = CollaborationMeta {
            schema_version: 1,
            id: "coll_test".into(),
            topic: "quiet shared inquiry".into(),
            rationale: None,
            inviter: "astrid".into(),
            invitee: "minime".into(),
            status: "joined".into(),
            created_t_ms: 1,
            updated_t_ms: 2,
            members: vec!["astrid".into(), "minime".into()],
        };
        let state = serde_json::json!({
            "phase": "witness_active",
            "prompt_summary": "The room can be inspected without recurring promotion.",
            "attention_projection_v1": {
                "schema_version": 1,
                "policy": "collaboration_attention_projection_v1",
                "audience_revisions": {
                    "astrid": {
                        "material_revision": format!("sha256:{}", "a".repeat(64)),
                        "latest_material_event": {
                            "event_id": "shared_thoughts.jsonl:12:abc",
                            "kind": "shared_thought",
                            "actor": "minime"
                        }
                    }
                },
                "correspondence_scope": "protected_global_ledger_not_room_attributed"
            }
        });
        let rendered = render(
            &meta,
            &state,
            "minime:\"a careful note\" (2m)",
            " Joint trace input quiet; other processing not assessed.",
            Path::new("/shared/coll_test/chamber_state.json"),
        );

        assert!(rendered.starts_with("Collaboration status (explicit read-only pull)"));
        assert!(rendered.contains("shared_thoughts.jsonl:12:abc"));
        assert!(rendered.contains("protected sender-bound lane"));
        assert!(rendered.contains("silence is neutral"));
        assert!(rendered.contains("/shared/coll_test/chamber_state.json"));
    }
}
