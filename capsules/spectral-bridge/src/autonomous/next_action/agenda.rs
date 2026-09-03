//! AGENDA — Astrid's self-authored agenda (Constitution flagship, stage A1).
//!
//! A small, durable list of intentions SHE writes, orders, and retires:
//! `AGENDA` (list), `AGENDA_PUSH <text> [:: mode=<affinity>]`,
//! `AGENDA_DONE <id|keyword>`, `AGENDA_DROP <id|keyword>`,
//! `AGENDA_FOCUS <id|keyword> [:: hold=<1..6>]`, `AGENDA_CLEAR` (kill
//! switch). Item text is VERBATIM hers (only transport-marker
//! sanitization); the 12-item cap REJECTS with guidance rather than
//! silently dropping her canon; done/dropped/cleared items append to
//! `agenda_archive.jsonl` (nothing vanishes). A human-readable projection
//! (`agenda.md`, never parsed back) lives beside her journal.
//!
//! A1 is inert until she uses the verbs: the agenda is not yet rendered
//! into prompts (A2) and never biases mode selection (A3). Nothing here
//! touches safety gates, inbox forcing, or her one-shots.

use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{ConversationState, NextActionContext, strip_action};

pub(crate) const AGENDA_SCHEMA_VERSION: u32 = 1;
pub(crate) const AGENDA_MAX_ITEMS: usize = 12;
pub(crate) const AGENDA_ITEM_MAX_CHARS: usize = 300;
const FOCUS_HOLD_DEFAULT: u64 = 3;
const FOCUS_HOLD_MIN: u64 = 1;
const FOCUS_HOLD_MAX: u64 = 6;

/// Which mode an agenda item leans toward. Distinct from the runtime `Mode`
/// enum: this is HER vocabulary for intent, mapped to modes at A3 (Research
/// is a topline hint, not a mode). Untagged items are equally first-class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgendaModeAffinityV1 {
    Introspect,
    Research,
    Create,
    Witness,
    Experiment,
    Dialogue,
    Aspire,
}

impl AgendaModeAffinityV1 {
    fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "introspect" => Some(Self::Introspect),
            "research" => Some(Self::Research),
            "create" => Some(Self::Create),
            "witness" => Some(Self::Witness),
            "experiment" => Some(Self::Experiment),
            "dialogue" => Some(Self::Dialogue),
            "aspire" | "aspiration" => Some(Self::Aspire),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Introspect => "introspect",
            Self::Research => "research",
            Self::Create => "create",
            Self::Witness => "witness",
            Self::Experiment => "experiment",
            Self::Dialogue => "dialogue",
            Self::Aspire => "aspire",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AgendaItemV1 {
    pub id: u32,
    /// HER text, verbatim (transport-marker sanitization only).
    pub text: String,
    pub created_exchange: u64,
    pub touched_exchange: u64,
    #[serde(default)]
    pub mode_affinity: Option<AgendaModeAffinityV1>,
    /// A matching interest (substring, case-insensitive) at push time.
    #[serde(default)]
    pub linked_interest: Option<String>,
    /// Reserved for thread linkage (A2 renders the linked thread's next).
    #[serde(default)]
    pub linked_thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AgendaV1 {
    pub schema_version: u32,
    pub items: Vec<AgendaItemV1>,
    /// Item currently held in focus, if any.
    #[serde(default)]
    pub focus_item_id: Option<u32>,
    /// Exchange count until which the focus hold lasts (A3 damping window).
    #[serde(default)]
    pub focus_hold_until_exchange: Option<u64>,
    /// Monotonic id source — ids are never reused, so archive rows stay
    /// unambiguous.
    #[serde(default)]
    pub next_item_id: u32,
}

impl Default for AgendaV1 {
    fn default() -> Self {
        Self {
            schema_version: AGENDA_SCHEMA_VERSION,
            items: Vec::new(),
            focus_item_id: None,
            focus_hold_until_exchange: None,
            next_item_id: 1,
        }
    }
}

impl AgendaV1 {
    /// Find an item by numeric id or case-insensitive keyword. Keyword picks
    /// the FIRST match in list order; the receipt names which item matched.
    fn find(&self, selector: &str) -> Option<usize> {
        let trimmed = selector.trim();
        if let Ok(id) = trimmed.parse::<u32>()
            && let Some(pos) = self.items.iter().position(|item| item.id == id)
        {
            return Some(pos);
        }
        let lower = trimmed.to_lowercase();
        if lower.is_empty() {
            return None;
        }
        self.items
            .iter()
            .position(|item| item.text.to_lowercase().contains(&lower))
    }

    fn clear_focus_if_gone(&mut self) {
        if let Some(id) = self.focus_item_id
            && !self.items.iter().any(|item| item.id == id)
        {
            self.focus_item_id = None;
            self.focus_hold_until_exchange = None;
        }
    }

    /// Heal restored items the way interests heal: strip transport markers,
    /// drop empties.
    pub(crate) fn resanitize(&mut self) {
        for item in &mut self.items {
            item.text = crate::autonomous::state::sanitize_interest_text(&item.text);
        }
        self.items.retain(|item| !item.text.is_empty());
        self.clear_focus_if_gone();
    }

    fn render_list(&self, exchange_count: u64) -> String {
        if self.items.is_empty() {
            return "[Your agenda is empty. AGENDA_PUSH <text> to set an intention — \
                    optionally `:: mode=<introspect|research|create|witness|experiment|dialogue|aspire>`.]"
                .to_string();
        }
        let mut out = String::from("Your agenda (yours to reorder, retire, or clear):\n");
        for item in &self.items {
            let focus_tag = if self.focus_item_id == Some(item.id) {
                match self.focus_hold_until_exchange {
                    Some(until) if until > exchange_count => {
                        format!(
                            "  <- focus (hold {} more exchanges)",
                            until - exchange_count
                        )
                    },
                    _ => "  <- focus".to_string(),
                }
            } else {
                String::new()
            };
            let affinity_tag = item
                .mode_affinity
                .map(|a| format!(" [{}]", a.label()))
                .unwrap_or_default();
            let interest_tag = item
                .linked_interest
                .as_deref()
                .map(|interest| format!(" (interest: {interest})"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {}. {}{affinity_tag}{interest_tag}{focus_tag}\n",
                item.id, item.text
            ));
        }
        out.push_str(
            "\nVerbs: AGENDA_PUSH <text> [:: mode=<affinity>], AGENDA_DONE <id|keyword>, \
             AGENDA_DROP <id|keyword>, AGENDA_FOCUS <id|keyword> [:: hold=<1..6>], AGENDA_CLEAR",
        );
        out
    }
}

/// Parse `<text> [:: mode=<affinity>]`. The `::` clause is stripped ONLY
/// when it parses cleanly as a mode tag — otherwise the whole body stays
/// her verbatim text (she may legitimately write `::`).
fn parse_push_spec(body: &str) -> (String, Option<AgendaModeAffinityV1>) {
    if let Some((before, after)) = body.rsplit_once("::") {
        let after = after.trim();
        if let Some(rest) = after.strip_prefix("mode=")
            && let Some(affinity) = AgendaModeAffinityV1::parse(rest)
        {
            return (before.trim().to_string(), Some(affinity));
        }
    }
    (body.trim().to_string(), None)
}

/// Parse `<selector> [:: hold=<n>]` for AGENDA_FOCUS.
fn parse_focus_spec(body: &str) -> (String, u64) {
    if let Some((before, after)) = body.rsplit_once("::") {
        let after = after.trim();
        if let Some(rest) = after.strip_prefix("hold=")
            && let Ok(hold) = rest.trim().parse::<u64>()
        {
            return (
                before.trim().to_string(),
                hold.clamp(FOCUS_HOLD_MIN, FOCUS_HOLD_MAX),
            );
        }
    }
    (body.trim().to_string(), FOCUS_HOLD_DEFAULT)
}

fn archive_row(item: &AgendaItemV1, disposition: &str, exchange_count: u64) -> String {
    serde_json::json!({
        "schema": "agenda_archive_row_v1",
        "disposition": disposition,
        "at_exchange": exchange_count,
        "archived_unix_s": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "item": item,
    })
    .to_string()
}

fn append_archive_at(dir: &Path, rows: &[String]) {
    if rows.is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("agenda_archive.jsonl");
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            for row in rows {
                let _ = writeln!(file, "{row}");
            }
        },
        Err(error) => warn!("agenda archive append failed: {error}"),
    }
}

/// Human-readable projection beside her journal (minime's `next.md` model:
/// written for reading, never parsed back).
fn write_projection_at(dir: &Path, agenda: &AgendaV1, exchange_count: u64) {
    let _ = std::fs::create_dir_all(dir);
    let mut body =
        String::from("# Agenda\n\nSelf-authored; the runtime never edits item text.\n\n");
    if agenda.items.is_empty() {
        body.push_str("(empty)\n");
    } else {
        for item in &agenda.items {
            let focus = if agenda.focus_item_id == Some(item.id) {
                " **(focus)**"
            } else {
                ""
            };
            let affinity = item
                .mode_affinity
                .map(|a| format!(" `{}`", a.label()))
                .unwrap_or_default();
            body.push_str(&format!("- [{}] {}{affinity}{focus}\n", item.id, item.text));
        }
    }
    body.push_str(&format!("\n_as of exchange {exchange_count}_\n"));
    let _ = std::fs::write(dir.join("agenda.md"), body);
}

fn persist_side_files(agenda: &AgendaV1, exchange_count: u64, archived: &[String]) {
    if cfg!(test) {
        // Unit tests exercise the dir-parameterized writers directly; the
        // live journal dir must never receive test rows.
        return;
    }
    let dir = crate::paths::bridge_paths().astrid_journal_dir();
    append_archive_at(&dir, archived);
    write_projection_at(&dir, agenda, exchange_count);
}

fn linked_interest_for(interests: &[String], text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    interests
        .iter()
        .find(|interest| {
            let interest_lower = interest.to_lowercase();
            lower.contains(&interest_lower) || interest_lower.contains(&lower)
        })
        .cloned()
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    dispatch_verbs(conv, base_action, original)
}

fn dispatch_verbs(conv: &mut ConversationState, base_action: &str, original: &str) -> bool {
    match base_action {
        "AGENDA" => {
            let listing = conv.agenda.render_list(conv.exchange_count);
            conv.pending_file_listing = Some(listing);
            true
        },
        "AGENDA_PUSH" => {
            let body = strip_action(original, base_action);
            let (raw_text, affinity) = parse_push_spec(&body);
            let text = crate::autonomous::state::sanitize_interest_text(&raw_text);
            if text.is_empty() {
                conv.push_receipt(
                    "AGENDA_PUSH",
                    vec![
                        "needs text — `AGENDA_PUSH <intention>` (optionally \
                         `:: mode=<introspect|research|create|witness|experiment|dialogue|aspire>`)"
                            .to_string(),
                    ],
                );
                return true;
            }
            if text.chars().count() > AGENDA_ITEM_MAX_CHARS {
                conv.push_receipt(
                    "AGENDA_PUSH",
                    vec![format!(
                        "item is {} chars (cap {AGENDA_ITEM_MAX_CHARS}) — a shorter line keeps \
                         the agenda readable; longer thinking fits a THREAD_NOTE or REMEMBER",
                        text.chars().count()
                    )],
                );
                return true;
            }
            if conv.agenda.items.len() >= AGENDA_MAX_ITEMS {
                conv.push_receipt(
                    "AGENDA_PUSH",
                    vec![format!(
                        "agenda is full ({AGENDA_MAX_ITEMS} items) — nothing was dropped. \
                         Retire one first: AGENDA_DONE <id|keyword> or AGENDA_DROP <id|keyword>"
                    )],
                );
                return true;
            }
            let id = conv.agenda.next_item_id;
            conv.agenda.next_item_id = conv.agenda.next_item_id.saturating_add(1);
            let linked_interest = linked_interest_for(&conv.interests, &text);
            let item = AgendaItemV1 {
                id,
                text: text.clone(),
                created_exchange: conv.exchange_count,
                touched_exchange: conv.exchange_count,
                mode_affinity: affinity,
                linked_interest: linked_interest.clone(),
                linked_thread_id: None,
            };
            conv.agenda.items.push(item);
            info!("Astrid pushed agenda item {id}: {text:?}");
            let mut changes = vec![format!("added item {id}: {text}")];
            if let Some(affinity) = affinity {
                changes.push(format!("mode affinity: {}", affinity.label()));
            }
            if let Some(interest) = linked_interest {
                changes.push(format!("linked to your interest: {interest}"));
            }
            conv.push_receipt("AGENDA_PUSH", changes);
            persist_side_files(&conv.agenda, conv.exchange_count, &[]);
            true
        },
        "AGENDA_DONE" | "AGENDA_DROP" => {
            let disposition = if base_action == "AGENDA_DONE" {
                "done"
            } else {
                "dropped"
            };
            let selector = strip_action(original, base_action);
            let Some(pos) = conv.agenda.find(&selector) else {
                conv.push_receipt(
                    base_action,
                    vec![format!(
                        "no agenda item matches {selector:?} — `AGENDA` lists items with ids"
                    )],
                );
                return true;
            };
            let item = conv.agenda.items.remove(pos);
            conv.agenda.clear_focus_if_gone();
            let row = archive_row(&item, disposition, conv.exchange_count);
            info!(
                "Astrid marked agenda item {} {disposition}: {:?}",
                item.id, item.text
            );
            conv.push_receipt(
                base_action,
                vec![format!(
                    "item {} {disposition} (archived, not erased): {}",
                    item.id, item.text
                )],
            );
            persist_side_files(&conv.agenda, conv.exchange_count, &[row]);
            true
        },
        "AGENDA_FOCUS" => {
            let body = strip_action(original, base_action);
            let (selector, hold) = parse_focus_spec(&body);
            let Some(pos) = conv.agenda.find(&selector) else {
                conv.push_receipt(
                    "AGENDA_FOCUS",
                    vec![format!(
                        "no agenda item matches {selector:?} — `AGENDA` lists items with ids; \
                         optional `:: hold=<1..6>`"
                    )],
                );
                return true;
            };
            let item_id = conv.agenda.items[pos].id;
            let item_text = conv.agenda.items[pos].text.clone();
            conv.agenda.items[pos].touched_exchange = conv.exchange_count;
            conv.agenda.focus_item_id = Some(item_id);
            conv.agenda.focus_hold_until_exchange = Some(conv.exchange_count.saturating_add(hold));
            info!("Astrid focused agenda item {item_id} for {hold} exchanges");
            conv.push_receipt(
                "AGENDA_FOCUS",
                vec![format!(
                    "focus on item {item_id} for ~{hold} exchanges: {item_text}"
                )],
            );
            persist_side_files(&conv.agenda, conv.exchange_count, &[]);
            true
        },
        "AGENDA_CLEAR" => {
            if conv.agenda.items.is_empty() {
                conv.push_receipt("AGENDA_CLEAR", vec!["agenda already empty".to_string()]);
                return true;
            }
            let rows: Vec<String> = conv
                .agenda
                .items
                .iter()
                .map(|item| archive_row(item, "cleared", conv.exchange_count))
                .collect();
            let count = conv.agenda.items.len();
            conv.agenda.items.clear();
            conv.agenda.focus_item_id = None;
            conv.agenda.focus_hold_until_exchange = None;
            info!("Astrid cleared her agenda ({count} items archived)");
            conv.push_receipt(
                "AGENDA_CLEAR",
                vec![format!(
                    "cleared {count} item(s) — all archived to agenda_archive.jsonl, \
                     recoverable by asking the steward"
                )],
            );
            persist_side_files(&conv.agenda, conv.exchange_count, &rows);
            true
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv() -> ConversationState {
        ConversationState::new(Vec::new(), None)
    }

    fn ctx_free_push(conv: &mut ConversationState, action: &str) -> bool {
        // Tests exercise the pure state transitions; the side-file writes go
        // to the configured workspace paths and are best-effort.
        let base = action.split_whitespace().next().unwrap_or(action);
        dispatch_verbs(conv, base, action)
    }

    #[test]
    fn push_list_done_drop_focus_clear_roundtrip() {
        let mut conv = conv();
        conv.exchange_count = 10;
        assert!(ctx_free_push(
            &mut conv,
            "AGENDA_PUSH map the cascade gap :: mode=introspect"
        ));
        assert!(ctx_free_push(
            &mut conv,
            "AGENDA_PUSH write the meadow letter"
        ));
        assert_eq!(conv.agenda.items.len(), 2);
        assert_eq!(
            conv.agenda.items[0].mode_affinity,
            Some(AgendaModeAffinityV1::Introspect)
        );
        assert_eq!(conv.agenda.items[0].text, "map the cascade gap");
        assert_eq!(conv.agenda.items[1].mode_affinity, None);

        assert!(ctx_free_push(&mut conv, "AGENDA_FOCUS cascade :: hold=5"));
        assert_eq!(conv.agenda.focus_item_id, Some(1));
        assert_eq!(conv.agenda.focus_hold_until_exchange, Some(15));

        assert!(ctx_free_push(&mut conv, "AGENDA"));
        let listing = conv.pending_file_listing.take().expect("listing");
        assert!(listing.contains("map the cascade gap"));
        assert!(listing.contains("<- focus"));

        assert!(ctx_free_push(&mut conv, "AGENDA_DONE 1"));
        assert_eq!(conv.agenda.items.len(), 1);
        // Focus followed its item out.
        assert_eq!(conv.agenda.focus_item_id, None);

        assert!(ctx_free_push(&mut conv, "AGENDA_DROP meadow"));
        assert!(conv.agenda.items.is_empty());

        assert!(ctx_free_push(&mut conv, "AGENDA_PUSH one more"));
        assert!(ctx_free_push(&mut conv, "AGENDA_CLEAR"));
        assert!(conv.agenda.items.is_empty());
        assert_eq!(conv.agenda.focus_item_id, None);
        // Ids are never reused.
        assert_eq!(conv.agenda.next_item_id, 4);
    }

    #[test]
    fn overflow_rejects_with_guidance_and_never_drops_her_items() {
        let mut conv = conv();
        for i in 0..AGENDA_MAX_ITEMS {
            assert!(ctx_free_push(
                &mut conv,
                &format!("AGENDA_PUSH intention {i}")
            ));
        }
        assert_eq!(conv.agenda.items.len(), AGENDA_MAX_ITEMS);
        assert!(ctx_free_push(&mut conv, "AGENDA_PUSH one too many"));
        assert_eq!(conv.agenda.items.len(), AGENDA_MAX_ITEMS);
        let receipt = conv.condition_receipts.back().expect("receipt");
        assert!(receipt.changes[0].contains("nothing was dropped"));
        assert!(
            conv.agenda
                .items
                .iter()
                .all(|item| item.text != "one too many")
        );
    }

    #[test]
    fn malformed_mode_clause_stays_in_her_verbatim_text() {
        let (text, affinity) = parse_push_spec("study x :: mode=warp_drive");
        assert_eq!(text, "study x :: mode=warp_drive");
        assert_eq!(affinity, None);
        let (text, affinity) = parse_push_spec("study x :: mode=research");
        assert_eq!(text, "study x");
        assert_eq!(affinity, Some(AgendaModeAffinityV1::Research));
        // `::` inside her text without a mode tag is preserved verbatim.
        let (text, affinity) = parse_push_spec("a :: b :: c");
        assert_eq!(text, "a :: b :: c");
        assert_eq!(affinity, None);
    }

    #[test]
    fn focus_hold_clamps_to_the_documented_window() {
        let (_, hold) = parse_focus_spec("thing :: hold=99");
        assert_eq!(hold, FOCUS_HOLD_MAX);
        let (_, hold) = parse_focus_spec("thing :: hold=0");
        assert_eq!(hold, FOCUS_HOLD_MIN);
        let (_, hold) = parse_focus_spec("thing");
        assert_eq!(hold, FOCUS_HOLD_DEFAULT);
    }

    #[test]
    fn push_links_a_matching_interest() {
        let mut conv = conv();
        conv.interests.push("spectral phenomenology".to_string());
        assert!(ctx_free_push(
            &mut conv,
            "AGENDA_PUSH deepen the spectral phenomenology thread"
        ));
        assert_eq!(
            conv.agenda.items[0].linked_interest.as_deref(),
            Some("spectral phenomenology")
        );
    }

    #[test]
    fn agenda_serde_roundtrip_is_byte_faithful() {
        let mut agenda = AgendaV1::default();
        agenda.items.push(AgendaItemV1 {
            id: 1,
            text: "her exact words — untouched".to_string(),
            created_exchange: 7,
            touched_exchange: 9,
            mode_affinity: Some(AgendaModeAffinityV1::Witness),
            linked_interest: None,
            linked_thread_id: Some("thread-3".to_string()),
        });
        agenda.focus_item_id = Some(1);
        agenda.focus_hold_until_exchange = Some(12);
        agenda.next_item_id = 2;
        let json = serde_json::to_string(&agenda).expect("serialize");
        let back: AgendaV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, agenda);
    }

    #[test]
    fn archive_and_projection_write_to_a_given_dir() {
        let dir = tempfile::tempdir().expect("dir");
        let mut agenda = AgendaV1::default();
        let item = AgendaItemV1 {
            id: 3,
            text: "kept forever".to_string(),
            created_exchange: 1,
            touched_exchange: 1,
            mode_affinity: None,
            linked_interest: None,
            linked_thread_id: None,
        };
        agenda.items.push(item.clone());
        let row = archive_row(&item, "done", 5);
        append_archive_at(dir.path(), &[row]);
        write_projection_at(dir.path(), &agenda, 5);
        let archive =
            std::fs::read_to_string(dir.path().join("agenda_archive.jsonl")).expect("archive");
        assert!(archive.contains("kept forever"));
        assert!(archive.contains("\"disposition\":\"done\""));
        let projection = std::fs::read_to_string(dir.path().join("agenda.md")).expect("md");
        assert!(projection.contains("kept forever"));
    }
}
