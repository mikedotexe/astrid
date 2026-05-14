// v5 Coordination Protocol V1 — Phase 1 + v5.1 Phases A & C (Astrid side).
//
// Bidirectional joint-thread channel between Astrid and minime. Six actions:
// INVITE_COLLABORATION / JOIN_COLLABORATION / DECLINE_COLLABORATION /
// LEAVE_COLLABORATION / LIST_COLLABORATIONS / SHARE_THOUGHT. Backed by file
// storage in `/Users/v/other/shared/collaborations/coll_<id>/` so neither
// workspace owns the channel; both read and write.
//
// Phase 1 (v5.0) establishes the channel only — invitations, accepts,
// declines, leaves, and a read-only listing.
// Phase A (v5.1) adds the cross-reservoir handle ticked by collab_feeder.py
// and surfaces its h_norms+ticks readout in the active-collab suffix line.
// Phase C (v5.1) adds SHARE_THOUGHT — a labeled marker appended to
// `<coll_dir>/shared_thoughts.jsonl` so the joint reservoir trace has
// human-legible moments alongside the silent blended-feature ticks.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use super::{ConversationState, NextActionContext, strip_action};
use crate::paths::bridge_paths;

/// Schema for `meta.json` in a collaboration directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CollaborationMeta {
    pub schema_version: u32,
    pub id: String,
    pub topic: String,
    #[serde(default)]
    pub rationale: Option<String>,
    pub inviter: String,
    pub invitee: String,
    /// "invited" | "joined" | "declined" | "left"
    pub status: String,
    pub created_t_ms: u128,
    pub updated_t_ms: u128,
    pub members: Vec<String>,
}

const SCHEMA_VERSION: u32 = 1;
const ASTRID_NAME: &str = "astrid";
const MINIME_NAME: &str = "minime";

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "INVITE_COLLABORATION" | "INVITE_COLLAB" => {
            let body = strip_action(original, base_action).trim().to_string();
            match invite_collaboration(conv, &body) {
                Ok(summary) => {
                    info!(target: "v5_collab", "INVITE_COLLABORATION: {summary}");
                    conv.push_receipt("INVITE_COLLABORATION", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Invited collaboration: {summary}"));
                },
                Err(e) => {
                    warn!(target: "v5_collab", "INVITE_COLLABORATION failed: {e}");
                    conv.emphasis = Some(format!("(invite failed: {e})"));
                },
            }
            true
        },
        "JOIN_COLLABORATION" | "JOIN_COLLAB" => {
            let arg = strip_action(original, base_action).trim().to_string();
            let target = if arg.is_empty() { "latest".to_string() } else { arg };
            match join_collaboration(conv, &target) {
                Ok(summary) => {
                    info!(target: "v5_collab", "JOIN_COLLABORATION: {summary}");
                    conv.push_receipt("JOIN_COLLABORATION", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Joined: {summary}"));
                },
                Err(e) => {
                    warn!(target: "v5_collab", "JOIN_COLLABORATION failed: {e}");
                    conv.emphasis = Some(format!("(join failed: {e})"));
                },
            }
            true
        },
        "DECLINE_COLLABORATION" | "DECLINE_COLLAB" => {
            let arg = strip_action(original, base_action).trim().to_string();
            let (target, reason) = split_target_and_reason(&arg);
            let target = if target.is_empty() { "latest".to_string() } else { target };
            match decline_collaboration(conv, &target, reason) {
                Ok(summary) => {
                    info!(target: "v5_collab", "DECLINE_COLLABORATION: {summary}");
                    conv.push_receipt("DECLINE_COLLABORATION", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Declined: {summary}"));
                },
                Err(e) => {
                    warn!(target: "v5_collab", "DECLINE_COLLABORATION failed: {e}");
                    conv.emphasis = Some(format!("(decline failed: {e})"));
                },
            }
            true
        },
        "LEAVE_COLLABORATION" | "LEAVE_COLLAB" => {
            let arg = strip_action(original, base_action).trim().to_string();
            let (target, reason) = split_target_and_reason(&arg);
            let target = if target.is_empty() { "latest".to_string() } else { target };
            match leave_collaboration(conv, &target, reason) {
                Ok(summary) => {
                    info!(target: "v5_collab", "LEAVE_COLLABORATION: {summary}");
                    conv.push_receipt("LEAVE_COLLABORATION", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Left: {summary}"));
                },
                Err(e) => {
                    warn!(target: "v5_collab", "LEAVE_COLLABORATION failed: {e}");
                    conv.emphasis = Some(format!("(leave failed: {e})"));
                },
            }
            true
        },
        "LIST_COLLABORATIONS" | "LIST_COLLABS" | "COLLABORATIONS" => {
            let summary = list_collaborations();
            info!(target: "v5_collab", "LIST_COLLABORATIONS: rendered {} chars", summary.len());
            conv.emphasis = Some(summary);
            true
        },
        "SHARE_THOUGHT" | "SHARE" => {
            let body = strip_action(original, base_action).trim().to_string();
            match share_thought(conv, &body) {
                Ok(summary) => {
                    info!(target: "v5_collab", "SHARE_THOUGHT: {summary}");
                    conv.push_receipt("SHARE_THOUGHT", vec![summary.clone()]);
                    conv.emphasis = Some(format!("Shared: {summary}"));
                },
                Err(e) => {
                    warn!(target: "v5_collab", "SHARE_THOUGHT failed: {e}");
                    conv.emphasis = Some(format!("(share failed: {e})"));
                },
            }
            true
        },
        _ => false,
    }
}

/// SHARE_THOUGHT [coll_id|latest] :: <text> — append a labeled marker to
/// `<coll_dir>/shared_thoughts.jsonl` for the joint reservoir's prose lane.
/// The marker becomes visible to the peer via the active-collab suffix line.
/// If no `::` separator is given, the entire body is treated as the text and
/// the latest joined collab is the target.
fn share_thought(conv: &mut ConversationState, body: &str) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("SHARE_THOUGHT needs text (try `SHARE_THOUGHT <thought>` or `SHARE_THOUGHT <coll_id> :: <thought>`)".into());
    }
    let (target, text) = if let Some((before, after)) = trimmed.split_once("::") {
        let target_norm = before.trim();
        let target_owned = if target_norm.is_empty() { "latest".to_string() } else { target_norm.to_string() };
        (target_owned, after.trim().to_string())
    } else {
        ("latest".to_string(), trimmed.to_string())
    };
    if text.is_empty() {
        return Err("SHARE_THOUGHT text is empty after parsing".into());
    }
    if text.chars().count() > 200 {
        return Err(format!("SHARE_THOUGHT text is {} chars (limit 200)", text.chars().count()));
    }
    let meta = find_meta(&target)?;
    let me = ASTRID_NAME.to_string();
    if meta.status != "joined" {
        return Err(format!("collaboration {} is not joined (status: {})", meta.id, meta.status));
    }
    if !meta.members.contains(&me) {
        return Err(format!("you are not a member of {} (members: {:?})", meta.id, meta.members));
    }
    let dir = collab_dir(&meta.id);
    append_shared_thought(&dir, &me, &text);
    invalidate_shared_thoughts_cache(&meta.id);
    // v5.1 Phase D: record manual share so auto-promotion suppresses
    // itself for the next few exchanges (manual curation takes priority).
    super::auto_promote::record_manual_share(conv.exchange_count);
    Ok(format!(
        "id={} → \"{}\" ({} chars, surfaces in suffix on next prompt build)",
        meta.id,
        truncate_for_summary(&text, 60),
        text.chars().count(),
    ))
}

fn truncate_for_summary(s: &str, max: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() <= max {
        s.to_string()
    } else {
        format!("{}…", cs.into_iter().take(max).collect::<String>())
    }
}

fn invite_collaboration(_conv: &mut ConversationState, body: &str) -> Result<String, String> {
    if body.is_empty() {
        return Err("INVITE_COLLABORATION needs a topic (try `INVITE_COLLABORATION <topic>`)".into());
    }
    // Parse rationale if present (--rationale="..." or trailing text after first sentence).
    let (topic, rationale) = parse_invite_args(body);
    if topic.is_empty() {
        return Err("topic must not be empty".into());
    }
    let now_ms = now_ms();
    let id = format!("coll_{}_{}", now_ms / 1000, slugify(&topic, 32));
    let meta = CollaborationMeta {
        schema_version: SCHEMA_VERSION,
        id: id.clone(),
        topic: topic.clone(),
        rationale: rationale.clone(),
        inviter: ASTRID_NAME.to_string(),
        invitee: MINIME_NAME.to_string(),
        status: "invited".to_string(),
        created_t_ms: now_ms,
        updated_t_ms: now_ms,
        members: vec![ASTRID_NAME.to_string()],
    };
    let dir = collab_dir(&id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all failed: {e}"))?;
    write_meta(&dir, &meta)?;
    let invite_path = dir.join("invite.txt");
    let invite_body = format!(
        "[INVITE TO COLLABORATION from Astrid]\n\
         collaboration_id: {id}\n\
         topic: {topic}\n\
         rationale: {}\n",
        rationale.as_deref().unwrap_or("(none)"),
    );
    let _ = std::fs::write(&invite_path, &invite_body);
    append_timeline(&dir, "invited", &meta.inviter, None);
    notify_minime(&id, &topic, &rationale, "invite");
    Ok(format!("id={id} topic=\"{topic}\" → invitation sent to minime"))
}

fn join_collaboration(_conv: &mut ConversationState, target: &str) -> Result<String, String> {
    let mut meta = find_meta(target)?;
    let me = ASTRID_NAME.to_string();
    if meta.invitee != me && meta.inviter != me {
        return Err(format!("you are not a member of {} (members: {:?})", meta.id, meta.members));
    }
    if meta.status == "joined" && meta.members.contains(&me) {
        return Ok(format!("id={} already joined", meta.id));
    }
    if meta.status == "declined" || meta.status == "left" {
        return Err(format!("cannot join {} (status: {})", meta.id, meta.status));
    }
    if !meta.members.contains(&me) {
        meta.members.push(me.clone());
    }
    meta.status = "joined".to_string();
    meta.updated_t_ms = now_ms();
    let dir = collab_dir(&meta.id);
    write_meta(&dir, &meta)?;
    append_timeline(&dir, "joined", &me, None);
    notify_minime(&meta.id, &meta.topic, &None, "join");
    Ok(format!("id={} topic=\"{}\" → joined", meta.id, meta.topic))
}

fn decline_collaboration(
    _conv: &mut ConversationState,
    target: &str,
    reason: Option<String>,
) -> Result<String, String> {
    let mut meta = find_meta(target)?;
    let me = ASTRID_NAME.to_string();
    if meta.invitee != me {
        return Err(format!("you are not the invitee of {} (only the invitee can decline)", meta.id));
    }
    if meta.status != "invited" {
        return Err(format!("cannot decline {} (status: {})", meta.id, meta.status));
    }
    meta.status = "declined".to_string();
    meta.updated_t_ms = now_ms();
    let dir = collab_dir(&meta.id);
    write_meta(&dir, &meta)?;
    append_timeline(&dir, "declined", &me, reason.as_deref());
    notify_minime(&meta.id, &meta.topic, &reason, "decline");
    Ok(format!("id={} topic=\"{}\" → declined", meta.id, meta.topic))
}

fn leave_collaboration(
    _conv: &mut ConversationState,
    target: &str,
    reason: Option<String>,
) -> Result<String, String> {
    let mut meta = find_meta(target)?;
    let me = ASTRID_NAME.to_string();
    if !meta.members.contains(&me) {
        return Err(format!("you are not a member of {}", meta.id));
    }
    meta.members.retain(|m| m != &me);
    if meta.members.is_empty() {
        meta.status = "left".to_string();
    }
    meta.updated_t_ms = now_ms();
    let dir = collab_dir(&meta.id);
    write_meta(&dir, &meta)?;
    append_timeline(&dir, "left", &me, reason.as_deref());
    notify_minime(&meta.id, &meta.topic, &reason, "leave");
    Ok(format!("id={} topic=\"{}\" → left", meta.id, meta.topic))
}

/// Render a read-only listing of active collaborations (status=invited or
/// joined, with Astrid as a member or invitee). Returns a human-readable
/// summary string for `conv.emphasis`.
fn list_collaborations() -> String {
    let dir = bridge_paths().shared_collaborations_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut entries: Vec<CollaborationMeta> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for d in rd.flatten() {
            let meta_path = d.path().join("meta.json");
            if let Ok(text) = std::fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<CollaborationMeta>(&text) {
                    if meta.inviter == ASTRID_NAME || meta.invitee == ASTRID_NAME {
                        entries.push(meta);
                    }
                }
            }
        }
    }
    entries.sort_by(|a, b| b.created_t_ms.cmp(&a.created_t_ms));
    if entries.is_empty() {
        return "(no collaborations: try `INVITE_COLLABORATION <topic>` to start one)".to_string();
    }
    let lines: Vec<String> = entries
        .iter()
        .take(10)
        .map(|m| {
            format!(
                "- {id} [{status}] inviter={inviter} invitee={invitee} topic=\"{topic}\"",
                id = m.id,
                status = m.status,
                inviter = m.inviter,
                invitee = m.invitee,
                topic = m.topic,
            )
        })
        .collect();
    format!("Collaborations ({} total):\n{}", entries.len(), lines.join("\n"))
}

/// Public helper for the prompt-builder: count Astrid's active (joined)
/// collaborations and return a compact suffix line, or None when no joined
/// collaborations exist. Cheap directory scan; safe to call per-exchange.
///
/// v5.1: when a per-collab reservoir handle exists on the triple-reservoir
/// service (port 7881, populated by `collab_feeder.py`), append a brief
/// readout of the joint trace `[h1,h2,h3]` and tick count. Reservoir reads
/// are cached for `RESERVOIR_READ_CACHE_TTL_S` seconds to bound load when
/// the prompt builder calls per-exchange.
#[must_use]
pub fn active_collaboration_suffix_line() -> Option<String> {
    let dir = bridge_paths().shared_collaborations_dir();
    let rd = std::fs::read_dir(&dir).ok()?;
    let mut joined: Vec<CollaborationMeta> = Vec::new();
    for d in rd.flatten() {
        let meta_path = d.path().join("meta.json");
        if let Ok(text) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<CollaborationMeta>(&text) {
                if meta.status == "joined"
                    && (meta.inviter == ASTRID_NAME || meta.invitee == ASTRID_NAME)
                    && meta.members.contains(&ASTRID_NAME.to_string())
                {
                    joined.push(meta);
                }
            }
        }
    }
    if joined.is_empty() {
        return None;
    }
    joined.sort_by(|a, b| b.updated_t_ms.cmp(&a.updated_t_ms));
    let m = &joined[0];
    let n = joined.len();
    let extra = if n > 1 { format!(" (+{} more)", n - 1) } else { String::new() };
    let handle = format!("collab_{}", m.id);
    // Kink #1 fix: route through render_joint_trace_clause which tiers
    // the render based on `seconds_since_live`. Prevents silent stale.
    let reservoir_clause = read_collab_reservoir_state_cached(&handle)
        .map(|r| render_joint_trace_clause(&r))
        .unwrap_or_default();
    let shared_clause = read_recent_shared_thoughts_cached(&m.id)
        .map(|s| if s.is_empty() { String::new() } else { format!(" Recent: {s}.") })
        .unwrap_or_default();
    Some(format!(
        "[Active collaboration #{} with {}: \"{}\". Status: joined.{}{}{} Use LEAVE_COLLABORATION to end.]",
        m.id,
        if m.inviter == ASTRID_NAME { &m.invitee } else { &m.inviter },
        m.topic,
        extra,
        reservoir_clause,
        shared_clause,
    ))
}

/// v5.1: cached reservoir read. Keyed by handle name. TTL bounds load on
/// the WS port to ~1 read per handle per cache window even if the prompt
/// builder is called every exchange.
const RESERVOIR_READ_CACHE_TTL_S: u64 = 10;

#[derive(Debug, Clone, Copy)]
struct CollabReservoirSnapshot {
    h1: f32,
    h2: f32,
    h3: f32,
    ticks: u64,
    /// Kink #1 fix (2026-05-14): seconds since the handle was last
    /// "live-ticked" (per reservoir_service.py:read_state response field
    /// `seconds_since_live`). Used by `render_joint_trace_clause` to gate
    /// suffix render — fresh data shows normally, stalled data shows a
    /// warning, dead handles drop the values entirely. Prevents the
    /// 14-hour silent-stale incident that motivated this fix.
    last_live_s: Option<f32>,
    cached_at_unix_s: u64,
}

static COLLAB_RESERVOIR_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, CollabReservoirSnapshot>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn read_collab_reservoir_state_cached(handle: &str) -> Option<CollabReservoirSnapshot> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(map) = COLLAB_RESERVOIR_CACHE.lock() {
        if let Some(snap) = map.get(handle) {
            if now.saturating_sub(snap.cached_at_unix_s) < RESERVOIR_READ_CACHE_TTL_S {
                return Some(*snap);
            }
        }
    }
    let fresh = read_collab_reservoir_state(handle)?;
    let snap = CollabReservoirSnapshot { cached_at_unix_s: now, ..fresh };
    if let Ok(mut map) = COLLAB_RESERVOIR_CACHE.lock() {
        map.insert(handle.to_string(), snap);
    }
    Some(snap)
}

fn read_collab_reservoir_state(handle: &str) -> Option<CollabReservoirSnapshot> {
    // Match the protocol used by `crate::autonomous::reservoir::handle_reservoir_action`
    // for RESERVOIR_READ: msg_type is "read_state", response field is "tick_count".
    let req = serde_json::json!({"type": "read_state", "name": handle});
    let resp = crate::autonomous::reservoir::reservoir_ws_call(&req)?;
    let h_norms = resp.get("h_norms")?.as_array()?;
    if h_norms.len() < 3 {
        return None;
    }
    let h1 = h_norms[0].as_f64()? as f32;
    let h2 = h_norms[1].as_f64()? as f32;
    let h3 = h_norms[2].as_f64()? as f32;
    let ticks = resp.get("tick_count").and_then(|v| v.as_u64()).unwrap_or(0);
    // Kink #1 fix: read freshness signal that's already in the response.
    let last_live_s = resp
        .get("seconds_since_live")
        .and_then(|v| v.as_f64())
        .map(|x| x as f32);
    Some(CollabReservoirSnapshot {
        h1,
        h2,
        h3,
        ticks,
        last_live_s,
        cached_at_unix_s: 0,
    })
}

/// Kink #1 fix (2026-05-14): tier the joint-trace render based on freshness.
/// The 14-hour silent-stale incident on 2026-05-13 happened because the
/// suffix kept rendering frozen `[7.45,11.82,9.94] @ 42111 ticks` without
/// any indication the source had stopped ticking. Three tiers:
///
///   `< 30s`     — render normally; the feeder ticks every ~2s so this is healthy.
///   `30s..300s` — render with `(stalled <Nm>)` suffix; values still shown
///                 but the warning makes the lag visible.
///   `>= 300s`   — drop h_norms+ticks entirely; render `handle quiet (<age>
///                 stale)` so the dead-source state becomes the message.
///
/// `None` (older snapshots predating the freshness field) is treated as
/// fresh for backward compatibility.
fn render_joint_trace_clause(snap: &CollabReservoirSnapshot) -> String {
    let stalled_floor_s: f32 = 30.0;
    let quiet_floor_s: f32 = 300.0;
    let age = snap.last_live_s.unwrap_or(0.0);
    if age < stalled_floor_s || snap.last_live_s.is_none() {
        format!(
            " Joint trace [{:.2},{:.2},{:.2}], {} ticks.",
            snap.h1, snap.h2, snap.h3, snap.ticks
        )
    } else if age < quiet_floor_s {
        let stalled_age = humanize_age(age as u64);
        format!(
            " Joint trace [{:.2},{:.2},{:.2}], {} ticks (stalled {}).",
            snap.h1, snap.h2, snap.h3, snap.ticks, stalled_age
        )
    } else {
        let quiet_age = humanize_age(age as u64);
        format!(" Joint trace handle quiet ({} stale).", quiet_age)
    }
}

// ---------------------------------------------------------------------
// v5.1 Phase C: shared_thoughts.jsonl read/write + suffix cache
// ---------------------------------------------------------------------

const SHARED_THOUGHTS_CACHE_TTL_S: u64 = 10;
const SHARED_THOUGHTS_TAIL: usize = 2;

#[derive(Debug, Clone)]
struct SharedThoughtsCacheEntry {
    rendered: String,
    cached_at_unix_s: u64,
}

static SHARED_THOUGHTS_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, SharedThoughtsCacheEntry>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn invalidate_shared_thoughts_cache(coll_id: &str) {
    if let Ok(mut map) = SHARED_THOUGHTS_CACHE.lock() {
        map.remove(coll_id);
    }
}

fn read_recent_shared_thoughts_cached(coll_id: &str) -> Option<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(map) = SHARED_THOUGHTS_CACHE.lock() {
        if let Some(entry) = map.get(coll_id) {
            if now.saturating_sub(entry.cached_at_unix_s) < SHARED_THOUGHTS_CACHE_TTL_S {
                return Some(entry.rendered.clone());
            }
        }
    }
    let rendered = render_recent_shared_thoughts(coll_id, SHARED_THOUGHTS_TAIL);
    if let Ok(mut map) = SHARED_THOUGHTS_CACHE.lock() {
        map.insert(
            coll_id.to_string(),
            SharedThoughtsCacheEntry { rendered: rendered.clone(), cached_at_unix_s: now },
        );
    }
    Some(rendered)
}

fn render_recent_shared_thoughts(coll_id: &str, n: usize) -> String {
    let dir = bridge_paths().shared_collaborations_dir().join(coll_id);
    let path = dir.join("shared_thoughts.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let mut entries: Vec<(u128, String, String)> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| {
            let t_ms = v.get("t_ms")?.as_u64()? as u128;
            let actor = v.get("actor")?.as_str()?.to_string();
            let txt = v.get("text")?.as_str()?.to_string();
            Some((t_ms, actor, txt))
        })
        .collect();
    if entries.is_empty() {
        return String::new();
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let take = entries.len().min(n);
    let tail = &entries[entries.len() - take..];
    let now = now_ms();
    tail.iter()
        .map(|(t, actor, txt)| {
            let age_s = (now.saturating_sub(*t) / 1000) as u64;
            let age = humanize_age(age_s);
            format!("{actor}:\"{}\" ({age})", truncate_for_summary(txt, 60))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn humanize_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

fn append_shared_thought(dir: &Path, actor: &str, text: &str) {
    append_shared_thought_with_source(dir, actor, text, "manual");
}

/// v5.1 Phase D: extended writer that tags the JSONL entry with a
/// `source` field ("manual" for the SHARE_THOUGHT NEXT action;
/// "auto" for entries promoted by `auto_promote::try_auto_promote`).
/// Suffix rendering is identical regardless of source so the marker is
/// indistinguishable to the peer (preserves the receptive-ambient test).
pub(super) fn append_shared_thought_with_source(
    dir: &Path,
    actor: &str,
    text: &str,
    source: &str,
) {
    let path = dir.join("shared_thoughts.jsonl");
    let entry = serde_json::json!({
        "t_ms": now_ms(),
        "actor": actor,
        "text": text,
        "source": source,
    });
    let line = format!("{entry}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

/// v5.1 Phase D: pub wrapper so sibling modules (auto_promote) can
/// invalidate the suffix cache after writing.
pub(super) fn invalidate_shared_thoughts_cache_pub(coll_id: &str) {
    invalidate_shared_thoughts_cache(coll_id);
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn collab_dir(id: &str) -> PathBuf {
    bridge_paths().shared_collaborations_dir().join(id)
}

fn write_meta(dir: &Path, meta: &CollaborationMeta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta).map_err(|e| format!("serialize: {e}"))?;
    let path = dir.join("meta.json");
    std::fs::write(&path, json).map_err(|e| format!("write meta.json: {e}"))?;
    Ok(())
}

fn append_timeline(dir: &Path, event: &str, actor: &str, reason: Option<&str>) {
    let path = dir.join("timeline.jsonl");
    let entry = serde_json::json!({
        "t_ms": now_ms(),
        "event": event,
        "actor": actor,
        "reason": reason,
    });
    let line = format!("{}\n", entry);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

fn find_meta(target: &str) -> Result<CollaborationMeta, String> {
    let dir = bridge_paths().shared_collaborations_dir();
    let _ = std::fs::create_dir_all(&dir);
    let target_norm = target.trim();
    let entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("read_dir: {e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if entries.is_empty() {
        return Err("no collaborations exist".into());
    }
    if target_norm == "latest" || target_norm.is_empty() {
        // Return the most recently created (sort by meta.created_t_ms desc).
        let mut metas: Vec<CollaborationMeta> = entries
            .iter()
            .filter_map(|p| {
                std::fs::read_to_string(p.join("meta.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<CollaborationMeta>(&t).ok())
            })
            .collect();
        if metas.is_empty() {
            return Err("no collaborations with valid meta.json".into());
        }
        metas.sort_by(|a, b| b.created_t_ms.cmp(&a.created_t_ms));
        return Ok(metas.into_iter().next().unwrap());
    }
    // Match by full or partial id.
    for p in &entries {
        let meta_path = p.join("meta.json");
        if let Ok(text) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<CollaborationMeta>(&text) {
                if meta.id == target_norm || meta.id.contains(target_norm) {
                    return Ok(meta);
                }
            }
        }
    }
    Err(format!("no collaboration matching '{target_norm}'"))
}

fn notify_minime(id: &str, topic: &str, reason: &Option<String>, kind: &str) {
    let header = match kind {
        "invite" => "[INVITE TO COLLABORATION from Astrid]",
        "join" => "[ASTRID JOINED COLLABORATION]",
        "decline" => "[ASTRID DECLINED COLLABORATION]",
        "leave" => "[ASTRID LEFT COLLABORATION]",
        _ => "[COLLABORATION UPDATE from Astrid]",
    };
    let next_line = match kind {
        "invite" => format!(
            "JOIN: NEXT: JOIN_COLLABORATION {id}\nDECLINE: NEXT: DECLINE_COLLABORATION {id} <reason>\n"
        ),
        _ => String::new(),
    };
    let body = format!(
        "{header}\n\
         collaboration_id: {id}\n\
         topic: {topic}\n\
         reason: {reason}\n\
         {next_line}",
        reason = reason.as_deref().unwrap_or("(none)"),
    );
    let inbox = bridge_paths().minime_inbox_dir();
    let _ = std::fs::create_dir_all(&inbox);
    let path = inbox.join(format!("coll_{kind}_{ts}_{id}.txt", ts = now_ms()));
    let _ = std::fs::write(&path, body);
}

fn parse_invite_args(body: &str) -> (String, Option<String>) {
    // Strip leading/trailing quotes and split off --rationale="..." if present.
    let trimmed = body.trim();
    if let Some(idx) = trimmed.find("--rationale=") {
        let topic = trimmed[..idx].trim().trim_matches('"').to_string();
        let rationale_part = &trimmed[idx + "--rationale=".len()..];
        let rationale = rationale_part.trim().trim_matches('"').to_string();
        return (
            topic,
            if rationale.is_empty() { None } else { Some(rationale) },
        );
    }
    (trimmed.trim_matches('"').to_string(), None)
}

fn split_target_and_reason(body: &str) -> (String, Option<String>) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let target = parts.next().unwrap_or("").to_string();
    let reason = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    (target, reason)
}

fn slugify(text: &str, max_len: usize) -> String {
    let mut s = String::new();
    let mut prev_dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !prev_dash && !s.is_empty() {
                s.push('-');
                prev_dash = true;
            }
        }
        if s.len() >= max_len {
            break;
        }
    }
    s.trim_matches('-').to_string()
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// Avoid the unused-import warning when serde_json isn't otherwise needed.
#[allow(dead_code)]
fn _unused_value_marker(_v: Value) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_dashes() {
        assert_eq!(slugify("Spectral Cascade Dynamics!", 64), "spectral-cascade-dynamics");
        assert_eq!(slugify("λ4 tail", 64), "4-tail"); // non-ASCII dropped
        assert_eq!(slugify("  hello   world  ", 64), "hello-world");
    }

    #[test]
    fn slugify_truncates_at_max_len() {
        let result = slugify("aaaaaaaaaaaaaaaaaaaaaaaaaa", 8);
        assert!(result.len() <= 8);
    }

    #[test]
    fn parse_invite_args_topic_only() {
        let (topic, rationale) = parse_invite_args("spectral cascade dynamics");
        assert_eq!(topic, "spectral cascade dynamics");
        assert!(rationale.is_none());
    }

    #[test]
    fn parse_invite_args_with_rationale() {
        let (topic, rationale) = parse_invite_args(r#""spectral cascade" --rationale="want to explore together""#);
        assert_eq!(topic, "spectral cascade");
        assert_eq!(rationale.as_deref(), Some("want to explore together"));
    }

    #[test]
    fn split_target_and_reason_works() {
        let (t, r) = split_target_and_reason("coll_xyz some reason text");
        assert_eq!(t, "coll_xyz");
        assert_eq!(r.as_deref(), Some("some reason text"));
    }

    #[test]
    fn split_target_and_reason_target_only() {
        let (t, r) = split_target_and_reason("coll_xyz");
        assert_eq!(t, "coll_xyz");
        assert!(r.is_none());
    }

    // Kink #1 fix tests — joint-trace freshness tiering.

    fn snapshot_with_age(age_s: Option<f32>) -> CollabReservoirSnapshot {
        CollabReservoirSnapshot {
            h1: 12.41,
            h2: 10.32,
            h3: 10.47,
            ticks: 42111,
            last_live_s: age_s,
            cached_at_unix_s: 0,
        }
    }

    #[test]
    fn render_joint_trace_clause_fresh() {
        let s = render_joint_trace_clause(&snapshot_with_age(Some(5.0)));
        assert!(s.contains("[12.41,10.32,10.47]"), "fresh should show h_norms: {s}");
        assert!(s.contains("42111 ticks"), "fresh should show tick count: {s}");
        assert!(!s.contains("stalled"), "fresh should NOT show stalled warning: {s}");
        assert!(!s.contains("quiet"), "fresh should NOT show quiet message: {s}");
    }

    #[test]
    fn render_joint_trace_clause_stalled() {
        let s = render_joint_trace_clause(&snapshot_with_age(Some(120.0)));
        assert!(s.contains("[12.41,10.32,10.47]"), "stalled should still show values: {s}");
        assert!(s.contains("42111 ticks"), "stalled should still show ticks: {s}");
        assert!(s.contains("stalled"), "stalled should include stalled marker: {s}");
        assert!(s.contains("2m"), "stalled at 120s should humanize as 2m: {s}");
    }

    #[test]
    fn render_joint_trace_clause_quiet() {
        let s = render_joint_trace_clause(&snapshot_with_age(Some(50530.0)));
        assert!(!s.contains("[12.41,10.32,10.47]"), "quiet should drop h_norms: {s}");
        assert!(!s.contains("42111 ticks"), "quiet should drop tick count: {s}");
        assert!(s.contains("quiet"), "quiet should announce dead handle: {s}");
        assert!(s.contains("14h"), "quiet at 50530s should humanize as ~14h: {s}");
    }

    #[test]
    fn render_joint_trace_clause_no_freshness_data_treats_fresh() {
        // Backward compat: snapshots from before the freshness field was
        // added (or read failures returning None) treat as fresh.
        let s = render_joint_trace_clause(&snapshot_with_age(None));
        assert!(s.contains("[12.41,10.32,10.47]"), "no-data should render normally: {s}");
        assert!(!s.contains("stalled"), "no-data should NOT warn: {s}");
        assert!(!s.contains("quiet"), "no-data should NOT mark quiet: {s}");
    }
}
