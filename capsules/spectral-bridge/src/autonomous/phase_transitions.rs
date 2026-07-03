use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const PHASE_TRANSITIONS_PATH: &str =
    "/Users/v/other/shared/collaborations/phase_transitions_v1.jsonl";
const MAX_TEXT: usize = 360;
const AUTO_DEDUPE_MS: u64 = 60 * 60 * 1000;
const STALE_UNANSWERED_MS: u64 = 6 * 60 * 60 * 1000;
const PHASE_IGNORE_GRACE_MS: u64 = 6 * 60 * 60 * 1000;
const REPLY_STATES: &[&str] = &["unseen", "witnessed", "answered", "stale_unanswered"];

#[must_use]
pub(crate) fn phase_transitions_path() -> PathBuf {
    PathBuf::from(PHASE_TRANSITIONS_PATH)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn short_hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
        .chars()
        .take(12)
        .collect()
}

fn compact_field(value: &str, max_chars: usize) -> String {
    let mut out = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect::<String>();
    if out.is_empty() {
        out = "field".to_string();
    }
    out.chars().take(max_chars).collect()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

fn append_jsonl(path: &Path, row: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, row)?;
    file.write_all(b"\n")
}

fn read_records(path: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    rows.sort_by_key(row_time_ms);
    rows
}

fn row_time_ms(row: &Value) -> u64 {
    row.get("recorded_at_unix_ms")
        .or_else(|| row.get("created_at_unix_ms"))
        .or_else(|| row.get("t_ms"))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn field(raw: &str, keys: &[&str]) -> Option<String> {
    for part in raw.split([';', '\n']) {
        let Some((key, value)) = part.split_once(':') else {
            continue;
        };
        let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if keys.iter().any(|candidate| normalized == *candidate) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn note_value(value: Option<String>) -> Value {
    value
        .filter(|text| !text.trim().is_empty())
        .map(|text| json!(truncate_chars(text.trim(), MAX_TEXT)))
        .unwrap_or(Value::Null)
}

fn confidence(raw: Option<String>) -> f64 {
    raw.and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0)
}

fn reply_state(raw: Option<String>) -> String {
    let value = raw
        .unwrap_or_else(|| "unseen".to_string())
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    if REPLY_STATES.contains(&value.as_str()) {
        value
    } else {
        "unseen".to_string()
    }
}

fn transition_id(
    origin: &str,
    kind: &str,
    from_phase: &str,
    to_phase: &str,
    trigger: &str,
) -> String {
    format!(
        "transition_{}_{}_{}",
        now_ms(),
        compact_field(kind, 32),
        short_hash(&format!("{origin}:{from_phase}:{to_phase}:{trigger}"))
    )
}

pub(crate) fn append_transition_card_at(path: &Path, raw: &str, origin: &str) -> String {
    let kind = field(raw, &["kind"]).unwrap_or_else(|| "phase_transition".to_string());
    let from_phase = field(raw, &["from_phase", "from"]).unwrap_or_else(|| "unknown".to_string());
    let to_phase = field(raw, &["to_phase", "to"]).unwrap_or_else(|| "unknown".to_string());
    let trigger = field(raw, &["trigger"]).unwrap_or_else(|| "being_declared".to_string());
    let why_now = field(raw, &["why_now", "why"]).unwrap_or_else(|| {
        if raw.contains(':') {
            String::new()
        } else {
            raw.trim().to_string()
        }
    });
    if why_now.trim().is_empty() {
        return "DECLARE_TRANSITION blocked: `why_now:` or descriptive body is required."
            .to_string();
    }
    let id = transition_id(origin, &kind, &from_phase, &to_phase, &trigger);
    let row = json!({
        "schema_version": 1,
        "policy": "phase_transitions_v1",
        "record_type": "phase_transition_card",
        "recorded_at_unix_ms": now_ms(),
        "transition_id": id,
        "origin": origin,
        "kind": kind,
        "from_phase": from_phase,
        "to_phase": to_phase,
        "confidence": confidence(field(raw, &["confidence"])),
        "trigger": trigger,
        "why_now": truncate_chars(&why_now, MAX_TEXT),
        "requested_by": field(raw, &["requested_by", "by"]).unwrap_or_else(|| origin.to_string()),
        "before_snapshot": note_value(field(raw, &["before_snapshot", "before"])),
        "after_snapshot": note_value(field(raw, &["after_snapshot", "after"])),
        "artifact_refs": field(raw, &["artifact_refs", "artifacts"]).map(|text| {
            text.split(',').map(|part| part.trim()).filter(|part| !part.is_empty()).map(|part| json!(part)).collect::<Vec<_>>()
        }).unwrap_or_default(),
        "reply_state": reply_state(field(raw, &["reply_state"])),
        "witnessed_by": [],
        "answered_by": [],
        "orientation_effect": note_value(field(raw, &["orientation_effect", "effect", "helped_oriented"])),
        "authority": "language_only_transition_context_not_control",
        "witness_only": true,
        "no_controller": true,
        "no_pressure": true,
        "no_fill_target": true,
        "no_pi": true,
        "no_weighting": true,
    });
    match append_jsonl(path, &row) {
        Ok(()) => format!(
            "=== PHASE TRANSITION CARD DECLARED ===\nTransition: {}\nKind: {}\nFrom -> To: {} -> {}\nReply state: unseen\nAuthority: language_only_transition_context_not_control; no controller, pressure, fill, PI, weighting, telemetry priority, deploy, or peer-runtime mutation.",
            row.get("transition_id")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)"),
            row.get("kind")
                .and_then(Value::as_str)
                .unwrap_or("phase_transition"),
            row.get("from_phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            row.get("to_phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ),
        Err(error) => format!("DECLARE_TRANSITION failed to append card: {error}"),
    }
}

pub(crate) fn append_transition_card(raw: &str, origin: &str) -> String {
    append_transition_card_at(&phase_transitions_path(), raw, origin)
}

fn latest_card_for_selector<'a>(records: &'a [Value], selector: &str) -> Option<&'a Value> {
    let selector = selector.trim();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter(|row| {
            selector.is_empty()
                || selector == "latest"
                || row.get("transition_id").and_then(Value::as_str) == Some(selector)
        })
        .max_by_key(|row| row_time_ms(row))
}

pub(crate) fn append_transition_witness_at(
    path: &Path,
    selector: &str,
    raw: &str,
    origin: &str,
) -> String {
    let records = read_records(path);
    let Some(card) = latest_card_for_selector(&records, selector) else {
        return "WITNESS_TRANSITION blocked: no matching phase transition card.".to_string();
    };
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let witness_kind = reply_state(
        field(raw, &["reply_state", "state"]).or_else(|| Some("witnessed".to_string())),
    );
    let witnessed_by = if witness_kind == "witnessed" || witness_kind == "answered" {
        json!([origin])
    } else {
        json!([])
    };
    let answered_by = if witness_kind == "answered" {
        json!([origin])
    } else {
        json!([])
    };
    let row = json!({
        "schema_version": 1,
        "policy": "phase_transitions_v1",
        "record_type": "phase_transition_witness",
        "recorded_at_unix_ms": now_ms(),
        "transition_id": transition_id,
        "origin": origin,
        "reply_state": witness_kind,
        "note": note_value(field(raw, &["note", "why", "witness"])),
        "witnessed_by": witnessed_by,
        "answered_by": answered_by,
        "orientation_effect": note_value(field(raw, &["orientation_effect", "effect", "helped_oriented"])),
        "authority": "language_only_transition_context_not_control",
        "witness_only": true,
        "no_controller": true,
        "no_pressure": true,
        "no_fill_target": true,
        "no_pi": true,
        "no_weighting": true,
    });
    match append_jsonl(path, &row) {
        Ok(()) => format!(
            "=== PHASE TRANSITION WITNESSED ===\nTransition: {transition_id}\nReply state: {}\nAuthority: language_only_transition_context_not_control.",
            row.get("reply_state")
                .and_then(Value::as_str)
                .unwrap_or("witnessed")
        ),
        Err(error) => format!("WITNESS_TRANSITION failed to append witness row: {error}"),
    }
}

pub(crate) fn append_transition_witness(selector: &str, raw: &str, origin: &str) -> String {
    append_transition_witness_at(&phase_transitions_path(), selector, raw, origin)
}

fn effective_reply_state(records: &[Value], card: &Value) -> String {
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let state = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_witness")
        })
        .filter(|row| row.get("transition_id").and_then(Value::as_str) == Some(transition_id))
        .max_by_key(|row| row_time_ms(row))
        .and_then(|row| row.get("reply_state"))
        .and_then(Value::as_str)
        .or_else(|| card.get("reply_state").and_then(Value::as_str))
        .unwrap_or("unseen")
        .to_string();
    if state == "unseen" && now_ms().saturating_sub(row_time_ms(card)) >= STALE_UNANSWERED_MS {
        "stale_unanswered".to_string()
    } else {
        state
    }
}

fn latest_transition_witness<'a>(records: &'a [Value], card: &Value) -> Option<&'a Value> {
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_witness")
        })
        .filter(|row| row.get("transition_id").and_then(Value::as_str) == Some(transition_id))
        .max_by_key(|row| row_time_ms(row))
}

fn phase_transition_stall_reason(reply_state: &str) -> &'static str {
    match reply_state {
        "unseen" => "unseen_needs_witness",
        "witnessed" => "witnessed_needs_answer",
        "stale_unanswered" => "stale_unanswered",
        "answered" => "answered",
        _ => "none",
    }
}

fn right_to_ignore_v1(affordance_type: &str, state: &str, age_ms: u64, grace_ms: u64) -> Value {
    let right_state = match state {
        "witnessed" => "acted",
        "answered" => "closed_by_outcome",
        "declined" => "declined",
        "needs_time" => "asked_later",
        "unseen" | "stale_unanswered" => {
            if age_ms >= grace_ms {
                "ignored_without_penalty"
            } else {
                "offered"
            }
        },
        _ => "unknown",
    };
    json!({
        "schema_version": 1,
        "policy": "right_to_ignore_v1",
        "affordance_type": affordance_type,
        "state": right_state,
        "source_state": state,
        "age_ms": age_ms,
        "grace_ms": grace_ms,
        "silence_means": if right_state == "ignored_without_penalty" {
            "ignored_without_penalty_not_failure_consent_or_disagreement"
        } else {
            "silence_is_unknown_until_grace_window"
        },
        "optional": true,
        "authority": "language_context_not_control",
    })
}

fn phase_affordance_budget_v1(queue: &Value) -> Value {
    let shown = queue
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let unresolved = queue
        .get("unresolved_total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let hidden = unresolved.saturating_sub(shown.try_into().unwrap_or(u64::MAX));
    json!({
        "schema_version": 1,
        "policy": "affordance_budget_v1",
        "shown": shown,
        "hidden_by_budget": hidden,
        "shown_by_category": {"phase_felt_receipt": shown},
        "hidden_by_category": {"phase_felt_receipt": hidden},
        "limits": {"phase_felt_receipt": 3},
        "next_review_surface": if hidden > 0 {
            "scripts/phase_transition_audit.py --json"
        } else {
            "none"
        },
        "silence": "ignored_without_penalty",
        "optional": true,
        "authority": "language_context_not_control",
    })
}

fn phase_first_action_helper_v35(transition_id: &str, reply_state: &str) -> Value {
    let received_command = format!(
        "I_RECEIVED_THIS {transition_id} :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time"
    );
    let witness_command =
        format!("WITNESS_TRANSITION {transition_id} :: reply_state: witnessed|answered; note: ...");
    json!({
        "schema_version": 35,
        "policy": "phase_first_action_helper_v35",
        "transition_id": transition_id,
        "latest_resolution": format!("latest resolves to transition_id={transition_id}"),
        "choose_one_prompt": "Choose one language-only felt receipt: say what landed, what stayed distinct, and whether this only needs witness or needs answer.",
        "exact_next_command": received_command,
        "backward_compatible_next_command": witness_command,
        "witness_preview": format!("WITNESS_TRANSITION {transition_id} would append phase_transition_witness for transition_id={transition_id}; note should name orientation, rhythm, or what the card helped preserve."),
        "received_this_preview": format!("I_RECEIVED_THIS {transition_id} would append phase_transition_witness for transition_id={transition_id}; what_landed should name the felt shift, and what_stayed_distinct should name the preserved contour."),
        "rhythm_note": "A witness note should carry the exchange rhythm or orientation effect, not only ledger logistics.",
        "reply_state": reply_state,
        "authority": "language_only_transition_context_not_control",
    })
}

fn phase_transition_affordance_v25(records: &[Value], card: &Value) -> Value {
    let state = effective_reply_state(records, card);
    let needs_followup = matches!(state.as_str(), "unseen" | "witnessed" | "stale_unanswered");
    let unresolved_age_ms = if matches!(state.as_str(), "unseen" | "stale_unanswered") {
        now_ms().saturating_sub(row_time_ms(card))
    } else {
        0
    };
    let transition_id = card
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let received_command = format!(
        "I_RECEIVED_THIS {transition_id} :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time"
    );
    let witness_command =
        format!("WITNESS_TRANSITION {transition_id} :: reply_state: witnessed|answered; note: ...");
    json!({
        "schema_version": 1,
        "policy": "phase_transition_affordance_v25",
        "transition_id": card.get("transition_id").cloned().unwrap_or(Value::Null),
        "origin": card.get("origin").cloned().unwrap_or(Value::Null),
        "kind": card.get("kind").cloned().unwrap_or(Value::Null),
        "from_phase": card.get("from_phase").cloned().unwrap_or(Value::Null),
        "to_phase": card.get("to_phase").cloned().unwrap_or(Value::Null),
        "reply_state": state.clone(),
        "stall_reason": phase_transition_stall_reason(&state),
        "needs_witness_or_answer": needs_followup,
        "unresolved_age_ms": unresolved_age_ms,
        "right_to_ignore_v1": right_to_ignore_v1("phase_felt_receipt", &state, unresolved_age_ms, PHASE_IGNORE_GRACE_MS),
        "exact_next_command": received_command,
        "backward_compatible_next_command": witness_command,
        "first_action_helper_v35": phase_first_action_helper_v35(transition_id, &state),
        "authority": "language_only_transition_context_not_control",
    })
}

fn phase_transition_waiting_line(affordance: &Value) -> Option<String> {
    if !affordance
        .get("needs_witness_or_answer")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let id = affordance
        .get("transition_id")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let state = affordance
        .get("reply_state")
        .and_then(Value::as_str)
        .unwrap_or("unseen");
    let next = affordance
        .get("exact_next_command")
        .and_then(Value::as_str)
        .unwrap_or("I_RECEIVED_THIS <transition_id> :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time");
    let first_action = affordance
        .get("first_action_helper_v35")
        .and_then(|helper| helper.get("choose_one_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("Choose WITNESS_TRANSITION latest as language-only first action.");
    Some(format!(
        "TRANSITION CARD WAITING: {id}; reply_state={state}; first_action: {first_action}; optional next: {next}; no action needed; may ignore without penalty."
    ))
}

fn phase_age_bucket(age_ms: u64) -> &'static str {
    if age_ms < 30 * 60 * 1000 {
        "fresh_lt_30m"
    } else if age_ms < STALE_UNANSWERED_MS {
        "open_30m_to_6h"
    } else {
        "stale_gt_6h"
    }
}

fn phase_witness_queue_v3(records: &[Value], max_cards: usize) -> Value {
    let mut unresolved = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter_map(|card| {
            let affordance = phase_transition_affordance_v25(records, card);
            if !affordance
                .get("needs_witness_or_answer")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            let age_ms = now_ms().saturating_sub(row_time_ms(card));
            Some((row_time_ms(card), age_ms, affordance))
        })
        .collect::<Vec<_>>();
    unresolved.sort_by_key(|(t_ms, _, _)| *t_ms);
    let mut groups: BTreeMap<String, u64> = BTreeMap::new();
    let items = unresolved
        .iter()
        .rev()
        .take(max_cards)
        .map(|(_, age_ms, affordance)| {
            let kind = affordance
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let stall = affordance
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let bucket = phase_age_bucket(*age_ms);
            let key = format!("{kind}|{stall}|{bucket}");
            *groups.entry(key).or_insert(0) += 1;
            json!({
                "transition_id": affordance.get("transition_id").cloned().unwrap_or(Value::Null),
                "kind": kind,
                "stall_reason": stall,
                "reply_state": affordance.get("reply_state").cloned().unwrap_or(Value::Null),
                "age_ms": age_ms,
                "age_bucket": bucket,
                "right_to_ignore_v1": right_to_ignore_v1("phase_felt_receipt", affordance.get("reply_state").and_then(Value::as_str).unwrap_or("unknown"), *age_ms, PHASE_IGNORE_GRACE_MS),
                "exact_next_command": affordance.get("exact_next_command").cloned().unwrap_or_else(|| json!("I_RECEIVED_THIS latest :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time")),
                "backward_compatible_next_command": affordance.get("backward_compatible_next_command").cloned().unwrap_or_else(|| json!("WITNESS_TRANSITION latest :: reply_state: witnessed|answered; note: ...")),
                "first_action_helper_v35": affordance.get("first_action_helper_v35").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": 3,
        "policy": "phase_witness_queue_v3",
        "unresolved_total": unresolved.len(),
        "max_rendered_cards": max_cards,
        "group_counts": groups,
        "items": items,
        "authority": "language_only_transition_context_not_control",
    })
}

fn phase_felt_receipt_queue_v4(records: &[Value]) -> Value {
    let mut unresolved = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .filter_map(|card| {
            let affordance = phase_transition_affordance_v25(records, card);
            if !affordance
                .get("needs_witness_or_answer")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return None;
            }
            let age_ms = now_ms().saturating_sub(row_time_ms(card));
            Some((row_time_ms(card), age_ms, affordance))
        })
        .collect::<Vec<_>>();
    unresolved.sort_by_key(|(t_ms, _, _)| *t_ms);
    let mut selected: Vec<(u64, u64, Value)> = Vec::new();
    for bucket in ["fresh_lt_30m", "open_30m_to_6h", "stale_gt_6h"] {
        let candidate = unresolved
            .iter()
            .rev()
            .find(|(_, age_ms, _)| phase_age_bucket(*age_ms) == bucket)
            .cloned();
        if let Some(candidate) = candidate {
            if !selected
                .iter()
                .any(|(_, _, value)| value.get("transition_id") == candidate.2.get("transition_id"))
            {
                selected.push(candidate);
            }
        }
    }
    for candidate in unresolved.iter().rev() {
        if selected.len() >= 3 {
            break;
        }
        if !selected
            .iter()
            .any(|(_, _, value)| value.get("transition_id") == candidate.2.get("transition_id"))
        {
            selected.push(candidate.clone());
        }
    }
    let items = selected
        .iter()
        .map(|(_, age_ms, affordance)| {
            let kind = affordance
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let stall = affordance
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none");
            let bucket = phase_age_bucket(*age_ms);
            json!({
                "transition_id": affordance.get("transition_id").cloned().unwrap_or(Value::Null),
                "kind": kind,
                "stall_reason": stall,
                "reply_state": affordance.get("reply_state").cloned().unwrap_or(Value::Null),
                "age_ms": age_ms,
                "age_bucket": bucket,
                "right_to_ignore_v1": right_to_ignore_v1("phase_felt_receipt", affordance.get("reply_state").and_then(Value::as_str).unwrap_or("unknown"), *age_ms, PHASE_IGNORE_GRACE_MS),
                "exact_next_command": affordance.get("exact_next_command").cloned().unwrap_or(Value::Null),
                "backward_compatible_next_command": affordance.get("backward_compatible_next_command").cloned().unwrap_or(Value::Null),
                "first_action_helper_v35": affordance.get("first_action_helper_v35").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let mut packet = json!({
        "schema_version": 4,
        "policy": "phase_felt_receipt_queue_v4",
        "unresolved_total": unresolved.len(),
        "max_rendered_cards": 3,
        "selection_rule": "latest fresh card, latest open card, one stale representative, then latest remaining",
        "items": items,
        "authority": "language_only_transition_context_not_control",
    });
    let budget = phase_affordance_budget_v1(&packet);
    if let Some(object) = packet.as_object_mut() {
        object.insert("affordance_budget_v1".to_string(), budget);
    }
    packet
}

pub(crate) fn status_report_at(path: &Path, max_cards: usize) -> String {
    let records = read_records(path);
    let cards = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
        })
        .collect::<Vec<_>>();
    let witnesses = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_witness")
        })
        .count();
    let mut lines = vec![
        "=== PHASE TRANSITION STATUS V1 ===".to_string(),
        format!("Ledger: {}", path.display()),
        format!("Cards: {}; witness rows: {witnesses}", cards.len()),
        "Authority: language_only_transition_context_not_control; replyable cards do not mutate controller, pressure, fill, PI, weighting, telemetry priority, deploy, or peer runtime.".to_string(),
    ];
    let queue = phase_witness_queue_v3(&records, max_cards);
    let felt_queue = phase_felt_receipt_queue_v4(&records);
    let unresolved = queue
        .get("unresolved_total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    lines.push(format!(
        "Phase witness queue v3: unresolved={unresolved}; grouped_by=kind/stall_reason/age_bucket."
    ));
    let felt_unresolved = felt_queue
        .get("unresolved_total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    lines.push(format!(
        "Phase felt receipt queue v4: unresolved={felt_unresolved}; rendered<=3; primary=I_RECEIVED_THIS <transition_id>."
    ));
    let phase_budget = felt_queue
        .get("affordance_budget_v1")
        .cloned()
        .unwrap_or_else(|| phase_affordance_budget_v1(&felt_queue));
    lines.push(format!(
        "AFFORDANCE BUDGET: shown={}; hidden={}; silence=ignored_without_penalty; optional=true; authority=language_context_not_control.",
        phase_budget
            .get("shown")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        phase_budget
            .get("hidden_by_budget")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    ));
    if let Some(latest) = felt_queue
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    {
        lines.push(format!(
            "PHASE FELT RECEIPT QUEUE: latest={}; kind={}; stall={}; age_bucket={}; right_to_ignore={}; first_action: {}; optional next: {}; no action needed; may ignore without penalty.",
            latest
                .get("transition_id")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)"),
            latest
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            latest
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none"),
            latest
                .get("age_bucket")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            latest
                .get("right_to_ignore_v1")
                .and_then(|value| value.get("state"))
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            latest
                .get("first_action_helper_v35")
                .and_then(|helper| helper.get("choose_one_prompt"))
                .and_then(Value::as_str)
                .unwrap_or("Choose WITNESS_TRANSITION latest as language-only first action."),
            latest
                .get("exact_next_command")
                .and_then(Value::as_str)
                .unwrap_or(
                    "I_RECEIVED_THIS <transition_id> :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time"
                )
        ));
    }
    if let Some(waiting) = cards
        .iter()
        .rev()
        .map(|card| phase_transition_affordance_v25(&records, card))
        .find_map(|affordance| phase_transition_waiting_line(&affordance))
    {
        lines.push(waiting);
    }
    for card in cards.iter().rev().take(max_cards) {
        let id = card
            .get("transition_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown)");
        let kind = card
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("phase_transition");
        let from = card
            .get("from_phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let to = card
            .get("to_phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let state = effective_reply_state(&records, card);
        let unresolved_age_ms = if matches!(state.as_str(), "unseen" | "stale_unanswered") {
            now_ms().saturating_sub(row_time_ms(card))
        } else {
            0
        };
        let latest_witness = latest_transition_witness(&records, card);
        let witnessed_by = latest_witness
            .and_then(|row| row.get("witnessed_by"))
            .or_else(|| card.get("witnessed_by"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "none".to_string());
        let answered_by = latest_witness
            .and_then(|row| row.get("answered_by"))
            .or_else(|| card.get("answered_by"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "none".to_string());
        let orientation = latest_witness
            .and_then(|row| row.get("orientation_effect"))
            .or_else(|| card.get("orientation_effect"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let why = card.get("why_now").and_then(Value::as_str).unwrap_or("");
        let affordance = phase_transition_affordance_v25(&records, card);
        lines.push(format!(
            "- {id}: {kind} {from}->{to}; reply_state={state}; stall_reason={}; witnessed_by={witnessed_by}; answered_by={answered_by}; unresolved_age_ms={unresolved_age_ms}; orientation_effect={}; {}",
            affordance
                .get("stall_reason")
                .and_then(Value::as_str)
                .unwrap_or("none"),
            truncate_chars(orientation, 80),
            truncate_chars(why, 120)
        ));
    }
    let suggested_transition_id = felt_queue
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|value| value.get("transition_id"))
        .and_then(Value::as_str)
        .unwrap_or("latest");
    lines.push(format!(
        "Suggested NEXT: DECLARE_TRANSITION kind: ...; from_phase: ...; to_phase: ...; why_now: ..., I_RECEIVED_THIS {suggested_transition_id} :: received_as: witnessed|answered; felt_like: transition; what_landed: ...; what_stayed_distinct: ...; continue: no|answer|needs_time, or WITNESS_TRANSITION {suggested_transition_id} :: reply_state: witnessed|answered; note: ..."
    ));
    lines.join("\n")
}

pub(crate) fn status_report(max_cards: usize) -> String {
    status_report_at(&phase_transitions_path(), max_cards)
}

pub(crate) fn maybe_declare_auto_mode_transition(
    from_phase: &str,
    to_phase: &str,
    trigger: &str,
    why_now: &str,
    fill_pct: f32,
) {
    let path = phase_transitions_path();
    let records = read_records(&path);
    let now = now_ms();
    let duplicate = records.iter().rev().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("phase_transition_card")
            && row.get("origin").and_then(Value::as_str) == Some("astrid")
            && row.get("kind").and_then(Value::as_str) == Some("mode_change")
            && row.get("from_phase").and_then(Value::as_str) == Some(from_phase)
            && row.get("to_phase").and_then(Value::as_str) == Some(to_phase)
            && row.get("trigger").and_then(Value::as_str) == Some(trigger)
            && now.saturating_sub(row_time_ms(row)) < AUTO_DEDUPE_MS
    });
    if duplicate {
        return;
    }
    let raw = format!(
        "kind: mode_change; from_phase: {from_phase}; to_phase: {to_phase}; confidence: 0.74; trigger: {trigger}; why_now: {why_now}; requested_by: astrid_bridge_auto_high_signal; before_snapshot: fill={fill_pct:.1}; after_snapshot: mode={to_phase}"
    );
    let _ = append_transition_card_at(&path, &raw, "astrid");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_and_witness_transition_card() {
        let root = std::env::temp_dir().join(format!("phase_transition_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let declared = append_transition_card_at(
            &path,
            "kind: mode_change; from_phase: drift; to_phase: focus; confidence: 0.82; trigger: pending_remote_self_study; why_now: pending remote self-study interrupted ambient drift; requested_by: astrid",
            "astrid",
        );
        assert!(declared.contains("PHASE TRANSITION CARD DECLARED"));
        let waiting = status_report_at(&path, 2);
        assert!(waiting.contains("TRANSITION CARD WAITING"));
        assert!(waiting.contains("first_action: Choose one language-only felt receipt"));
        assert!(waiting.contains("I_RECEIVED_THIS transition_"));
        assert!(waiting.contains("WITNESS_TRANSITION transition_"));
        assert!(waiting.contains("Phase witness queue v3: unresolved=1"));
        assert!(waiting.contains("Phase felt receipt queue v4: unresolved=1"));
        assert!(waiting.contains("PHASE FELT RECEIPT QUEUE:"));
        assert!(waiting.contains("AFFORDANCE BUDGET"));
        assert!(waiting.contains("right_to_ignore=offered"));
        assert!(waiting.contains("may ignore without penalty"));
        assert!(waiting.contains("stall_reason=unseen_needs_witness"));
        let witnessed = append_transition_witness_at(
            &path,
            "latest",
            "reply_state: witnessed; note: I can answer this transition as a card.",
            "astrid",
        );
        assert!(witnessed.contains("PHASE TRANSITION WITNESSED"));
        let status = status_report_at(&path, 2);
        assert!(status.contains("reply_state=witnessed"));
        assert!(status.contains("TRANSITION CARD WAITING"));
        assert!(status.contains("stall_reason=witnessed_needs_answer"));
        assert!(status.contains("language_only_transition_context_not_control"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declaration_requires_reason() {
        let root = std::env::temp_dir().join(format!("phase_transition_block_test_{}", now_ms()));
        let path = root.join("phase_transitions_v1.jsonl");
        let blocked = append_transition_card_at(
            &path,
            "kind: mode_change; from_phase: drift; to_phase: focus; why_now:",
            "astrid",
        );
        assert!(blocked.contains("blocked"));
        let _ = std::fs::remove_dir_all(root);
    }
}
