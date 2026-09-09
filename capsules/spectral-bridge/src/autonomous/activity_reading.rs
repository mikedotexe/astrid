//! Chosen saved-text activity and explicit mailbox detours.
//!
//! Session records own reading progress. The small atomic runtime file owns only
//! foreground selection; a conversation checkpoint is a cache of that selection.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::state::ConversationState;
use crate::action_continuity::{
    ActionContinuityStore, ReaderBookmarkPreview, ReaderBookmarkVersion, ReaderDisposition,
    ReaderPassage,
};

#[path = "activity_reading/persistence.rs"]
mod persistence;
pub(crate) use persistence::{load_activity, persist_activity};

const PASSAGE_BYTES: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct ActivityRuntimeV1 {
    pub foreground_reader: Option<ReaderActivityRefV1>,
    pub return_reader: Option<ReaderActivityRefV1>,
    pub mailbox_window: Option<MailboxWindowV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReaderActivityRefV1 {
    pub thread_id: String,
    pub session_id: String,
    #[serde(default)]
    pub agenda_item_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MailboxWindowV1 {
    pub id: String,
    pub large: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ActivityReadingOfferV1 {
    pub reader: ReaderActivityRefV1,
    pub version: ReaderBookmarkVersion,
    pub passage: ReaderPassage,
    pub text: String,
    pub source: crate::action_continuity::ReaderSourceSnapshot,
}

fn operation_id(kind: &str) -> String {
    format!("activity_{kind}_{:032x}", rand::random::<u128>())
}

fn preview_reader(
    store: &ActionContinuityStore,
    reader: &ReaderActivityRefV1,
) -> Result<ReaderBookmarkPreview> {
    persistence::validate_reader_ref(reader)?;
    let preview = store
        .reader_bookmark_preview(&reader.thread_id, &reader.session_id)?
        .ok_or_else(|| {
            anyhow!("selected reader session is unavailable; inspect ACTIVITY_STATUS")
        })?;
    if preview.bookmark.is_none() {
        return Err(anyhow!("selected session has no saved-text bookmark"));
    }
    Ok(preview)
}

fn version(preview: &ReaderBookmarkPreview) -> Result<ReaderBookmarkVersion> {
    preview
        .version()
        .ok_or_else(|| anyhow!("selected session has no saved-text bookmark"))
}

fn save_selection(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    next: ActivityRuntimeV1,
) -> Result<()> {
    // Update memory only after the selection reaches its authoritative file.
    persist_activity(store, &next)?;
    conv.activity = next;
    Ok(())
}

fn park_selection(
    store: &ActionContinuityStore,
    activity: &ActivityRuntimeV1,
) -> Result<ActivityRuntimeV1> {
    let mut next = activity.clone();
    let Some(reader) = next.foreground_reader.take() else {
        return Ok(next);
    };
    let preview = preview_reader(store, &reader)?;
    if preview
        .bookmark
        .as_ref()
        .is_some_and(|bookmark| bookmark.disposition == ReaderDisposition::Active)
    {
        store.reader_bookmark_transition(
            &reader.thread_id,
            &reader.session_id,
            &version(&preview)?,
            ReaderDisposition::Parked,
            &operation_id("park"),
        )?;
    }
    // A detour may itself choose reading. Keep the original return destination;
    // the second parked session remains in the ordinary session history.
    if next.return_reader.is_none() {
        next.return_reader = Some(reader);
    }
    Ok(next)
}

/// Called only after the source-specific Action has checked its access boundary.
pub(super) fn choose_saved_text(
    conv: &mut ConversationState,
    source: &Path,
    label: &str,
) -> Result<String> {
    choose_saved_text_in(
        &ActionContinuityStore::for_astrid_workspace(),
        conv,
        source,
        label,
    )
}

pub(super) fn choose_saved_text_in(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    source: &Path,
    label: &str,
) -> Result<String> {
    let source = source
        .canonicalize()
        .context("resolving chosen saved text")?;
    if let Some(reader) = conv.activity.foreground_reader.as_ref() {
        let preview = preview_reader(store, reader)?;
        if preview.bookmark.as_ref().is_some_and(|bookmark| {
            bookmark.source.original_path == source
                && bookmark.disposition == ReaderDisposition::Active
        }) {
            let session_id = reader.session_id.clone();
            persist_activity(store, &conv.activity)?;
            clear_superseded_reading_intents(conv);
            return Ok(format!(
                "[Continuing saved reading session {} from its committed byte position. Pending bytes remain pending until a completed model turn.]",
                session_id
            ));
        }
    }

    // Read/retain the replacement before parking the old activity. A failed new
    // source must not displace a valid foreground reader.
    let title: String = label
        .chars()
        .take(160)
        .map(|ch| {
            if matches!(ch, ';' | '\n' | '\r') {
                ' '
            } else {
                ch
            }
        })
        .collect();
    let thread = store.create_thread(None, &format!("Reading: {title}"), None)?;
    let started = store.continuity_session_start_command(&format!(
        "current :: title: Reading {title}; focus: Continue the chosen saved text"
    ))?;
    let session_id = started
        .split('`')
        .nth(1)
        .filter(|value| value.starts_with("sess_"))
        .ok_or_else(|| anyhow!("created session did not return its identity"))?
        .to_string();
    let reader = ReaderActivityRefV1 {
        thread_id: thread.thread_id,
        session_id,
        agenda_item_id: conv.agenda.focus_item_id,
    };
    let initial = store
        .reader_bookmark_preview(&reader.thread_id, &reader.session_id)?
        .ok_or_else(|| anyhow!("created reading session is unavailable"))?;
    let created = store.reader_bookmark_create(
        &reader.thread_id,
        &reader.session_id,
        &initial.session_record_id,
        &source,
        &operation_id("choose"),
    )?;
    let mut next = park_selection(store, &conv.activity)?;
    next.foreground_reader = Some(reader.clone());
    next.mailbox_window = None;
    save_selection(store, conv, next)?;
    // A typed source never participates in either legacy eager-advance path.
    clear_superseded_reading_intents(conv);
    Ok(format!(
        "[Saved reading selected: {title}. Session {} retains {} UTF-8 bytes. Progress starts at byte 0 and commits only after a completed model turn. PARK_ACTIVITY saves a quiet return; CHECK_MAILBOX opens one chosen letter window.]",
        reader.session_id, created.bookmark.source.byte_count
    ))
}

fn clear_superseded_reading_intents(conv: &mut ConversationState) {
    conv.last_read_path = None;
    conv.last_read_offset = 0;
    conv.last_read_meaning_summary = None;
    conv.browse_url = None;
    conv.wants_search = false;
    conv.search_topic = None;
    if conv.next_mode_override == Some(super::state::Mode::Dialogue) {
        conv.next_mode_override = None;
    }
}

pub(super) fn offer_requested_reading(
    conv: &mut ConversationState,
) -> Result<Option<ActivityReadingOfferV1>> {
    offer_requested_reading_in(&ActionContinuityStore::for_astrid_workspace(), conv)
}

/// An explicit view switch starts a distinct byte stream and parks the readable
/// bookmark. It never translates offsets or silently replaces a pending offer.
pub(super) fn choose_raw_reading(conv: &mut ConversationState) -> Result<String> {
    choose_raw_reading_in(&ActionContinuityStore::for_astrid_workspace(), conv)
}

fn choose_raw_reading_in(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
) -> Result<String> {
    let reader = conv
        .activity
        .foreground_reader
        .as_ref()
        .context("no foreground reading; use ACTIVITY_STATUS")?;
    let preview = preview_reader(store, reader)?;
    let raw = preview
        .bookmark
        .as_ref()
        .and_then(|b| b.source.raw_source.as_ref())
        .context("this reading is already exact text; no separate raw view is bound")?;
    if !preview
        .source_comparison
        .as_ref()
        .is_some_and(|s| s.retained_source_available)
    {
        return Err(anyhow!("retained reading origin is unavailable"));
    }
    let raw_path = store.root().join(&raw.retained_artifact);
    choose_saved_text_in(
        store,
        conv,
        &raw_path,
        "Original terminal text of saved prompt overflow",
    )
}

pub(super) fn offer_requested_reading_in(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
) -> Result<Option<ActivityReadingOfferV1>> {
    let Some(reader) = conv.activity.foreground_reader.clone() else {
        return Ok(None);
    };
    if conv.activity.mailbox_window.is_some() {
        return Err(anyhow!(
            "a mailbox window and foreground reader cannot run together"
        ));
    }
    let preview = preview_reader(store, &reader)?;
    let bookmark = preview
        .bookmark
        .as_ref()
        .context("reader bookmark missing")?;
    if !matches!(preview.session_status.as_str(), "active" | "summarized")
        || bookmark.disposition != ReaderDisposition::Active
    {
        return Err(anyhow!(
            "reader is quiet; ACTIVITY_STATUS supplies its explicit return command"
        ));
    }
    if bookmark.cursor.next_byte == bookmark.source.byte_count && bookmark.offered_passage.is_none()
    {
        store.reader_bookmark_transition(
            &reader.thread_id,
            &reader.session_id,
            &version(&preview)?,
            ReaderDisposition::Complete,
            &operation_id("complete"),
        )?;
        let mut next = conv.activity.clone();
        next.foreground_reader = None;
        save_selection(store, conv, next)?;
        return Ok(None);
    }
    if bookmark.offered_passage.is_none() {
        store.reader_bookmark_offer(
            &reader.thread_id,
            &reader.session_id,
            &version(&preview)?,
            PASSAGE_BYTES,
            &operation_id("offer"),
        )?;
    }
    let preview = preview_reader(store, &reader)?;
    let version = version(&preview)?;
    let passage = preview
        .bookmark
        .as_ref()
        .and_then(|bookmark| bookmark.offered_passage.clone())
        .ok_or_else(|| anyhow!("reader did not retain its offered passage"))?;
    let source = preview
        .bookmark
        .as_ref()
        .context("source binding missing")?
        .source
        .clone();
    let text = preview
        .offered_text
        .context("retained passage bytes are unavailable")?;
    Ok(Some(ActivityReadingOfferV1 {
        source,
        reader,
        version,
        passage,
        text,
    }))
}

fn describe_reader(store: &ActionContinuityStore, reader: &ReaderActivityRefV1) -> Result<String> {
    let preview = preview_reader(store, reader)?;
    let bookmark = preview
        .bookmark
        .as_ref()
        .context("reader bookmark missing")?;
    let source = preview
        .source_comparison
        .as_ref()
        .context("reader source comparison missing")?;
    Ok(format!(
        "{} [{}]: committed byte {} / {}; pending {}; retained source {}; original {:?}. Return command: RETURN_ACTIVITY {}",
        reader.session_id,
        preview.session_status,
        bookmark.cursor.next_byte,
        bookmark.source.byte_count,
        bookmark.offered_passage.as_ref().map_or_else(
            || "none".into(),
            |passage| format!("{}..{}", passage.start_byte, passage.end_byte)
        ),
        if source.retained_source_available {
            "available"
        } else {
            "unavailable"
        },
        source.current_status,
        preview.session_record_id,
    ))
}

fn status_in(store: &ActionContinuityStore, activity: &ActivityRuntimeV1) -> Result<String> {
    let foreground = activity
        .foreground_reader
        .as_ref()
        .map(|reader| describe_reader(store, reader))
        .transpose()?
        .unwrap_or_else(|| "none".into());
    let parked = activity
        .return_reader
        .as_ref()
        .map(|reader| describe_reader(store, reader))
        .transpose()?
        .unwrap_or_else(|| "none".into());
    let mailbox = activity.mailbox_window.as_ref().map_or_else(
        || "closed".into(),
        |window| format!("{} (one letter; large={})", window.id, window.large),
    );
    Ok(format!(
        "Foreground: {foreground}\nSaved return: {parked}\nMailbox: {mailbox}\nStatus does not advance reading or dispatch a saved command."
    ))
}

fn return_to_reader(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    reader: ReaderActivityRefV1,
    expected_record: &str,
) -> Result<String> {
    let preview = preview_reader(store, &reader)?;
    if expected_record.is_empty() {
        return Ok(format!(
            "Inspect this saved return, then choose its current revision:\n{}",
            describe_reader(store, &reader)?
        ));
    }
    if preview.session_record_id != expected_record {
        return Err(anyhow!(
            "stale activity return revision; inspect ACTIVITY_STATUS before returning"
        ));
    }
    let available = preview
        .source_comparison
        .as_ref()
        .is_some_and(|source| source.retained_source_available);
    if !available {
        return Err(anyhow!(
            "retained source is unavailable; the saved return is unchanged"
        ));
    }
    // Validate the requested destination before quieting a competing foreground.
    let mut next = if conv
        .activity
        .foreground_reader
        .as_ref()
        .is_some_and(|current| current != &reader)
    {
        park_selection(store, &conv.activity)?
    } else {
        conv.activity.clone()
    };
    store.reader_bookmark_transition(
        &reader.thread_id,
        &reader.session_id,
        &version(&preview)?,
        ReaderDisposition::Active,
        &operation_id("return"),
    )?;
    next.foreground_reader = Some(reader.clone());
    if next.return_reader.as_ref() == Some(&reader) {
        next.return_reader = None;
    }
    next.mailbox_window = None;
    save_selection(store, conv, next)?;
    if let Some(item_id) = reader.agenda_item_id
        && conv.agenda.items.iter().any(|item| item.id == item_id)
    {
        conv.agenda.focus_item_id = Some(item_id);
    }
    // The reader selection is already durable. Align the ordinary authored-thread
    // index as a derived projection; this API only selects a thread and returns
    // its saved text, which we deliberately discard rather than dispatch.
    let projection_notice = store.resume_thread(&reader.thread_id).err().map_or_else(
        String::new,
        |error| format!(" The reader return is saved, but the authored-thread index could not be aligned: {error}."),
    );
    clear_superseded_reading_intents(conv);
    Ok(format!(
        "[Returned to saved reading {} at its committed byte position. Pending offered bytes are preserved. No saved NEXT command was dispatched.]{projection_notice}",
        reader.session_id,
    ))
}

fn expected_revision(raw: &str) -> &str {
    let payload = raw
        .split_once("::")
        .map_or(raw, |(_, payload)| payload)
        .trim();
    payload.strip_prefix("revision:").unwrap_or(payload).trim()
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base: &str,
    original: &str,
) -> Option<Result<String>> {
    handle_action_in(
        &ActionContinuityStore::for_astrid_workspace(),
        conv,
        base,
        original,
    )
}

pub(super) fn handle_action_in(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    base: &str,
    original: &str,
) -> Option<Result<String>> {
    let raw = super::next_action::strip_action(original, base);
    match base {
        "ACTIVITY_STATUS" | "MAILBOX_STATUS" => Some(status_in(store, &conv.activity)),
        "PARK_ACTIVITY" => Some((|| {
            let next = park_selection(store, &conv.activity)?;
            save_selection(store, conv, next)?;
            status_in(store, &conv.activity)
        })()),
        "CHECK_MAILBOX" => Some((|| {
            if conv.activity.mailbox_window.is_none() {
                let mut next = park_selection(store, &conv.activity)?;
                next.mailbox_window = Some(MailboxWindowV1 {
                    id: operation_id("mailbox"),
                    large: raw.trim().eq_ignore_ascii_case("LARGE"),
                });
                save_selection(store, conv, next)?;
            }
            Ok(format!(
                "[Mailbox window selected for one durable letter. The saved reading will wait for explicit return.]\n{}",
                status_in(store, &conv.activity)?
            ))
        })()),
        "RETURN_ACTIVITY" => Some((|| {
            let reader = conv.activity.return_reader.clone().or_else(|| conv.activity.foreground_reader.clone())
                .ok_or_else(|| anyhow!("there is no selected saved return; inspect CONTINUITY_SESSION_STATUS for historical sessions"))?;
            return_to_reader(store, conv, reader, expected_revision(&raw))
        })()),
        "CONTINUITY_SESSION_RESUME" => {
            let selector = raw.split("::").next().unwrap_or("").trim();
            let reader = [
                &conv.activity.return_reader,
                &conv.activity.foreground_reader,
            ]
            .into_iter()
            .flatten()
            .find(|reader| reader.session_id == selector)
            .cloned()?;
            Some(return_to_reader(
                store,
                conv,
                reader,
                raw.split_once("::")
                    .map_or("", |(_, payload)| expected_revision(payload)),
            ))
        },
        _ => None,
    }
}

/// An explicit competing choice may park reading. Call after authorization.
pub(super) fn observe_chosen_action(conv: &mut ConversationState, base: &str) -> Result<()> {
    if matches!(
        base,
        "INTROSPECT"
            | "SELF_STUDY"
            | "INVESTIGATE"
            | "EXAMINE_CODE"
            | "CONTEMPLATE"
            | "BE"
            | "STILL"
            | "NOTICE"
            | "OBSERVE"
            | "DAYDREAM"
            | "REST"
            | "RECESS"
            | "BROWSE"
            | "SEARCH"
            | "RESEARCH"
            | "AR_READ"
            | "CODEX"
            | "COMPOSE"
            | "VOICE"
            | "REVISE"
            | "CREATE"
            | "ASPIRE"
            | "ASPIRATION"
            | "EXPERIMENT"
    ) && conv.activity.foreground_reader.is_some()
    {
        let store = ActionContinuityStore::for_astrid_workspace();
        let next = park_selection(&store, &conv.activity)?;
        save_selection(&store, conv, next)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "activity_reading/tests.rs"]
mod tests;
