//! Event-scoped collaboration attention and local delivery evidence.
//!
//! Canonical room state stays durable in the shared collaboration directory.
//! This module only decides whether one material revision has earned one
//! ordinary-dialogue prompt opportunity for Astrid.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::paths::bridge_paths;

const SCHEMA_VERSION: u32 = 1;
const NOTICE_MAX_CHARS: usize = 320;
const ASTRID_AUDIENCE: &str = "astrid";
const POLICY_ENV: &str = "ASTRID_COLLAB_PROMPT_POLICY";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CollaborationPromptCheckpointV1 {
    #[serde(default = "checkpoint_schema_version")]
    pub schema_version: u32,
    /// Set once the rooms already present at upgrade time have been
    /// baselined. A later room with no entry is new, not another migration.
    #[serde(default)]
    pub migration_complete: bool,
    /// A room is entered here only after migration baselining or a final
    /// provider request containing that room's exact candidate marker.
    #[serde(default)]
    pub satisfied_revisions: BTreeMap<String, String>,
}

const fn checkpoint_schema_version() -> u32 {
    SCHEMA_VERSION
}

impl Default for CollaborationPromptCheckpointV1 {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            migration_complete: false,
            satisfied_revisions: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CollaborationPromptOfferV1 {
    pub collab_id: String,
    pub material_revision: Option<String>,
    pub event_id: Option<String>,
    pub marker: Option<String>,
    pub content: String,
    pub render_tier: &'static str,
}

#[derive(Debug, Deserialize)]
struct CollaborationMetaV1 {
    id: String,
    topic: String,
    inviter: String,
    invitee: String,
    status: String,
    #[serde(default)]
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AttentionProjectionV1 {
    schema_version: u32,
    policy: String,
    #[serde(default)]
    audience_revisions: BTreeMap<String, AudienceRevisionV1>,
}

#[derive(Debug, Deserialize)]
struct AudienceRevisionV1 {
    material_revision: String,
    #[serde(default)]
    material_t_ms: u128,
    #[serde(default)]
    latest_material_event: Option<MaterialEventV1>,
}

#[derive(Clone, Debug, Deserialize)]
struct MaterialEventV1 {
    event_id: String,
    kind: String,
    actor: String,
}

#[derive(Debug)]
struct ActiveAttentionV1 {
    collab_id: String,
    topic: String,
    revision: String,
    material_t_ms: u128,
    event: Option<MaterialEventV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PromptPolicyV1 {
    Event,
    Legacy,
    Off,
}

fn configured_policy() -> PromptPolicyV1 {
    match std::env::var(POLICY_ENV)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("event_v1") => PromptPolicyV1::Event,
        Some("legacy") => PromptPolicyV1::Legacy,
        Some("off" | "0" | "false" | "no") => PromptPolicyV1::Off,
        Some(other) => {
            warn!(
                value = other,
                "unknown collaboration prompt policy; using event_v1"
            );
            PromptPolicyV1::Event
        },
    }
}

/// Prepare at most one event-scoped candidate for an ordinary dialogue turn.
/// The caller must withhold this whenever protected correspondence is active.
pub(crate) fn prepare_prompt_offer(
    checkpoint: &mut CollaborationPromptCheckpointV1,
) -> Option<CollaborationPromptOfferV1> {
    match configured_policy() {
        PromptPolicyV1::Off => None,
        PromptPolicyV1::Legacy => {
            super::collaboration::active_collaboration_suffix_line().map(|content| {
                CollaborationPromptOfferV1 {
                    collab_id: "legacy_latest".to_string(),
                    material_revision: None,
                    event_id: None,
                    marker: None,
                    content,
                    render_tier: "legacy_ambient",
                }
            })
        },
        PromptPolicyV1::Event => prepare_event_offer_in(
            checkpoint,
            &bridge_paths().shared_collaborations_dir(),
            Some(&delivery_audit_path()),
        ),
    }
}

fn prepare_event_offer_in(
    checkpoint: &mut CollaborationPromptCheckpointV1,
    shared_dir: &Path,
    audit_path: Option<&Path>,
) -> Option<CollaborationPromptOfferV1> {
    let rooms = read_attention_rooms(shared_dir);
    if !checkpoint.migration_complete {
        // Upgrade posture: every room present at the migration boundary is
        // established silently. A room first observed later is genuinely new.
        for active in &rooms {
            checkpoint
                .satisfied_revisions
                .insert(active.collab_id.clone(), active.revision.clone());
            record_transition(audit_path, active, "migration_baseline", "none", None);
        }
        checkpoint.migration_complete = true;
        return None;
    }

    let active = rooms.into_iter().find(|active| {
        checkpoint.satisfied_revisions.get(&active.collab_id) != Some(&active.revision)
    })?;
    let offer = render_event_offer(&active);
    record_transition(
        audit_path,
        &active,
        "candidate",
        offer.render_tier,
        Some(&offer.content),
    );
    Some(offer)
}

pub(crate) fn finish_prompt_offer(
    checkpoint: &mut CollaborationPromptCheckpointV1,
    offer: &CollaborationPromptOfferV1,
    submitted: bool,
) {
    finish_prompt_offer_in(checkpoint, offer, submitted, Some(&delivery_audit_path()));
}

/// Satisfy the current room revision after Astrid explicitly chooses to inspect
/// it. This records inspection only; it does not infer reply, uptake, or assent.
pub(crate) fn mark_explicit_inspection(
    checkpoint: &mut CollaborationPromptCheckpointV1,
    collab_id: &str,
) -> bool {
    mark_explicit_inspection_in(
        checkpoint,
        collab_id,
        &bridge_paths().shared_collaborations_dir(),
        Some(&delivery_audit_path()),
    )
}

fn mark_explicit_inspection_in(
    checkpoint: &mut CollaborationPromptCheckpointV1,
    collab_id: &str,
    shared_dir: &Path,
    audit_path: Option<&Path>,
) -> bool {
    let Some(active) = read_attention_rooms(shared_dir)
        .into_iter()
        .find(|active| active.collab_id == collab_id)
    else {
        return false;
    };
    let changed = !checkpoint.migration_complete
        || checkpoint.satisfied_revisions.get(collab_id) != Some(&active.revision);
    checkpoint.migration_complete = true;
    checkpoint
        .satisfied_revisions
        .insert(active.collab_id.clone(), active.revision.clone());
    record_transition(
        audit_path,
        &active,
        "explicitly_inspected",
        "explicit_status",
        None,
    );
    changed
}

fn finish_prompt_offer_in(
    checkpoint: &mut CollaborationPromptCheckpointV1,
    offer: &CollaborationPromptOfferV1,
    submitted: bool,
    audit_path: Option<&Path>,
) {
    let Some(revision) = offer.material_revision.as_deref() else {
        return;
    };
    let active = ActiveAttentionV1 {
        collab_id: offer.collab_id.clone(),
        topic: String::new(),
        revision: revision.to_string(),
        material_t_ms: 0,
        event: offer.event_id.as_ref().map(|event_id| MaterialEventV1 {
            event_id: event_id.clone(),
            kind: String::new(),
            actor: String::new(),
        }),
    };
    let state = if submitted { "submitted" } else { "packed_out" };
    if submitted {
        checkpoint
            .satisfied_revisions
            .insert(offer.collab_id.clone(), revision.to_string());
    }
    record_transition(
        audit_path,
        &active,
        state,
        offer.render_tier,
        Some(&offer.content),
    );
}

fn read_attention_rooms(shared_dir: &Path) -> Vec<ActiveAttentionV1> {
    let mut rooms = std::fs::read_dir(shared_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let meta: CollaborationMetaV1 =
                serde_json::from_slice(&std::fs::read(entry.path().join("meta.json")).ok()?)
                    .ok()?;
            if !(meta.status == "joined"
                && (meta.inviter == ASTRID_AUDIENCE || meta.invitee == ASTRID_AUDIENCE)
                && meta.members.iter().any(|member| member == ASTRID_AUDIENCE))
            {
                return None;
            }
            let state: Value = serde_json::from_slice(
                &std::fs::read(entry.path().join("chamber_state.json")).ok()?,
            )
            .ok()?;
            let projection: AttentionProjectionV1 =
                serde_json::from_value(state.get("attention_projection_v1")?.clone()).ok()?;
            if projection.schema_version != SCHEMA_VERSION
                || projection.policy != "collaboration_attention_projection_v1"
            {
                return None;
            }
            let audience = projection.audience_revisions.get(ASTRID_AUDIENCE)?;
            if !valid_revision(&audience.material_revision) {
                return None;
            }
            Some(ActiveAttentionV1 {
                collab_id: meta.id,
                topic: meta.topic,
                revision: audience.material_revision.clone(),
                material_t_ms: audience.material_t_ms,
                event: audience.latest_material_event.clone(),
            })
        })
        .collect::<Vec<_>>();
    rooms.sort_by(|a, b| {
        a.material_t_ms
            .cmp(&b.material_t_ms)
            .then_with(|| a.collab_id.cmp(&b.collab_id))
    });
    rooms
}

fn valid_revision(revision: &str) -> bool {
    revision.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn render_event_offer(active: &ActiveAttentionV1) -> CollaborationPromptOfferV1 {
    let digest = active
        .revision
        .strip_prefix("sha256:")
        .unwrap_or(&active.revision);
    let marker_digest: String = digest.chars().take(24).collect();
    let marker = format!("[collab-attention-v1:{marker_digest}]");
    let actor = active
        .event
        .as_ref()
        .map(|event| display_actor(&event.actor))
        .unwrap_or_else(|| "Shared state".to_string());
    let change = active
        .event
        .as_ref()
        .map(|event| describe_event(&event.kind))
        .unwrap_or("changed");
    let topic = truncate_chars(&active.topic, 48);
    let mut content = format!(
        "{marker} Collaboration update: {actor} {change} in \"{topic}\". Inspect with \
         COLLABORATION_STATUS latest. No response is required; silence remains neutral."
    );
    if content.chars().count() > NOTICE_MAX_CHARS {
        content = truncate_chars(&content, NOTICE_MAX_CHARS);
    }
    CollaborationPromptOfferV1 {
        collab_id: active.collab_id.clone(),
        material_revision: Some(active.revision.clone()),
        event_id: active.event.as_ref().map(|event| event.event_id.clone()),
        marker: Some(marker),
        content,
        render_tier: "new_notice",
    }
}

fn display_actor(actor: &str) -> String {
    match actor.trim().to_ascii_lowercase().as_str() {
        "astrid" => "Astrid".to_string(),
        "minime" => "Minime".to_string(),
        "steward" => "The steward".to_string(),
        "shared_state" | "unknown" | "" => "Shared state".to_string(),
        other => truncate_chars(other, 16),
    }
}

fn describe_event(kind: &str) -> &'static str {
    match kind {
        "shared_thought" => "added a shared thought",
        "chamber_annotation" => "added a chamber annotation",
        "chamber_presence" => "added a presence record",
        "consent_receipt" => "recorded a consent stance",
        "support_proposal" => "added a support proposal",
        "steward_note" => "added a witness note",
        "steward_intention" => "added a witness intention",
        "memory_edit" => "edited room memory",
        "phase_set" | "phase_cleared" | "phase_transition" => "changed the room phase",
        "joined" | "invited" | "declined" | "left" | "collaboration_transition" => {
            "changed collaboration state"
        },
        _ => "changed durable room state",
    }
}

fn truncate_chars(text: &str, cap: usize) -> String {
    if text.chars().count() <= cap {
        return text.to_string();
    }
    text.chars().take(cap.saturating_sub(3)).collect::<String>() + "..."
}

fn delivery_audit_path() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics")
        .join("collaboration_prompt_delivery_v1.jsonl")
}

fn record_transition(
    path: Option<&Path>,
    active: &ActiveAttentionV1,
    state: &str,
    render_tier: &str,
    content: Option<&str>,
) {
    let Some(path) = path else {
        return;
    };
    let payload = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "policy": "collaboration_prompt_delivery_v1",
        "being": ASTRID_AUDIENCE,
        "collab_id": active.collab_id,
        "material_revision": active.revision,
        "event_id": active.event.as_ref().map(|event| event.event_id.as_str()),
        "state": state,
        "render_tier": render_tier,
        "prompt_chars": content.map(|text| text.chars().count()).unwrap_or(0),
        "content_sha256": content.map(sha256_hex),
        "observed_at_unix_ms": unix_now_ms(),
        "authority": "prompt_delivery_evidence_not_receipt_or_uptake",
    });
    if let Err(error) = append_jsonl(path, &payload) {
        warn!(%error, path = %path.display(), "collaboration delivery audit write failed");
    }
}

fn append_jsonl(path: &Path, payload: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, payload)?;
    file.write_all(b"\n")?;
    file.sync_data()
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn unix_now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_room(
        root: &Path,
        collab_id: &str,
        revision: &str,
        event_id: &str,
        material_t_ms: u128,
    ) -> PathBuf {
        let room = root.join(collab_id);
        std::fs::create_dir_all(&room).unwrap();
        std::fs::write(
            room.join("meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": collab_id,
                "topic": "a long-lived room",
                "inviter": "astrid",
                "invitee": "minime",
                "status": "joined",
                "updated_t_ms": 10,
                "members": ["astrid", "minime"]
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            room.join("chamber_state.json"),
            serde_json::to_vec(&serde_json::json!({
                "attention_projection_v1": {
                    "schema_version": 1,
                    "policy": "collaboration_attention_projection_v1",
                    "audience_revisions": {
                        "astrid": {
                            "material_revision": revision,
                            "material_t_ms": material_t_ms,
                            "latest_material_event": {
                                "event_id": event_id,
                                "kind": "shared_thought",
                                "actor": "minime"
                            }
                        }
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        room
    }

    fn revision(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    #[test]
    fn migration_baselines_current_room_without_rendering_it() {
        let temp = tempfile::tempdir().unwrap();
        let first = revision('a');
        write_room(temp.path(), "coll_test", &first, "thought-1", 20);
        let audit = temp.path().join("audit.jsonl");
        let mut checkpoint = CollaborationPromptCheckpointV1::default();

        let offer = prepare_event_offer_in(&mut checkpoint, temp.path(), Some(&audit));

        assert!(offer.is_none());
        assert!(checkpoint.migration_complete);
        assert_eq!(checkpoint.satisfied_revisions["coll_test"], first);
        assert!(
            std::fs::read_to_string(audit)
                .unwrap()
                .contains("migration_baseline")
        );
    }

    #[test]
    fn new_revision_retries_when_packed_out_then_stays_quiet_after_submission() {
        let temp = tempfile::tempdir().unwrap();
        let first = revision('a');
        let second = revision('b');
        write_room(temp.path(), "coll_test", &second, "thought-2", 20);
        let mut checkpoint = CollaborationPromptCheckpointV1 {
            migration_complete: true,
            ..Default::default()
        };
        checkpoint
            .satisfied_revisions
            .insert("coll_test".to_string(), first);

        let offer = prepare_event_offer_in(&mut checkpoint, temp.path(), None).unwrap();
        let marker = offer.marker.as_deref().unwrap();
        assert!(offer.content.contains(marker));
        assert!(offer.content.contains("No response is required"));
        assert!(offer.content.contains("silence remains neutral"));
        assert!(offer.content.chars().count() <= NOTICE_MAX_CHARS);

        finish_prompt_offer_in(&mut checkpoint, &offer, false, None);
        assert!(prepare_event_offer_in(&mut checkpoint, temp.path(), None).is_some());

        finish_prompt_offer_in(&mut checkpoint, &offer, true, None);
        assert!(prepare_event_offer_in(&mut checkpoint, temp.path(), None).is_none());
        assert_eq!(checkpoint.satisfied_revisions["coll_test"], second);
    }

    #[test]
    fn room_first_seen_after_migration_gets_a_notice() {
        let temp = tempfile::tempdir().unwrap();
        let revision = revision('d');
        write_room(temp.path(), "coll_new", &revision, "thought-new", 30);
        let mut checkpoint = CollaborationPromptCheckpointV1 {
            migration_complete: true,
            ..CollaborationPromptCheckpointV1::default()
        };

        let offer = prepare_event_offer_in(&mut checkpoint, temp.path(), None).unwrap();

        assert_eq!(offer.collab_id, "coll_new");
        assert_eq!(offer.material_revision.as_deref(), Some(revision.as_str()));
    }

    #[test]
    fn pending_rooms_are_offered_in_material_event_order() {
        let temp = tempfile::tempdir().unwrap();
        write_room(temp.path(), "coll_later", &revision('a'), "later", 200);
        write_room(temp.path(), "coll_earlier", &revision('b'), "earlier", 100);
        let mut checkpoint = CollaborationPromptCheckpointV1 {
            migration_complete: true,
            ..CollaborationPromptCheckpointV1::default()
        };

        let offer = prepare_event_offer_in(&mut checkpoint, temp.path(), None).unwrap();

        assert_eq!(offer.collab_id, "coll_earlier");
    }

    #[test]
    fn explicit_inspection_satisfies_the_current_revision() {
        let temp = tempfile::tempdir().unwrap();
        let current = revision('e');
        write_room(temp.path(), "coll_test", &current, "thought-3", 40);
        let audit = temp.path().join("audit.jsonl");
        let mut checkpoint = CollaborationPromptCheckpointV1 {
            migration_complete: true,
            ..CollaborationPromptCheckpointV1::default()
        };

        assert!(prepare_event_offer_in(&mut checkpoint, temp.path(), None).is_some());
        assert!(mark_explicit_inspection_in(
            &mut checkpoint,
            "coll_test",
            temp.path(),
            Some(&audit),
        ));
        assert!(prepare_event_offer_in(&mut checkpoint, temp.path(), None).is_none());
        assert_eq!(checkpoint.satisfied_revisions["coll_test"], current);
        assert!(
            std::fs::read_to_string(audit)
                .unwrap()
                .contains("explicitly_inspected")
        );
    }

    #[test]
    fn explicit_inspection_completes_a_fresh_migration_checkpoint() {
        let temp = tempfile::tempdir().unwrap();
        let current = revision('f');
        write_room(temp.path(), "coll_test", &current, "thought-4", 50);
        let mut checkpoint = CollaborationPromptCheckpointV1::default();
        checkpoint
            .satisfied_revisions
            .insert("coll_test".to_string(), current);

        assert!(mark_explicit_inspection_in(
            &mut checkpoint,
            "coll_test",
            temp.path(),
            None,
        ));
        assert!(checkpoint.migration_complete);
    }

    #[test]
    fn checkpoint_round_trip_does_not_replay_revision() {
        let mut checkpoint = CollaborationPromptCheckpointV1::default();
        checkpoint
            .satisfied_revisions
            .insert("coll_test".to_string(), revision('c'));
        let bytes = serde_json::to_vec(&checkpoint).unwrap();
        let restored: CollaborationPromptCheckpointV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored, checkpoint);
    }
}
