use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::paths::bridge_paths;

use super::helpers::{atomic_write_json, load_json_or_default, now_unix_s, trim_chars};
use super::proposal::{
    proposal_agency_hypothesis, proposal_evidence_window, proposal_lineage, proposal_reason_codes,
};
use super::render::{render_owner_action, render_owner_block};
use super::signal::append_signal_event;
use super::{
    ActiveSovereigntyProposal, EpisodeBank, NominatedResponse, OWNER_MINIME, ProposalLedger,
    active_owner_view, apply_owner_choice, load_runtime, record_prompt_render, save_runtime,
};

const CONSUMED_REPLY_LIMIT: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct MinimeChoiceCursor {
    #[serde(default)]
    pub consumed_reply_keys: Vec<String>,
    #[serde(default)]
    pub last_consumed_at_unix_s: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct MinimeBtspReply {
    pub proposal_id: Option<String>,
    pub response_id: Option<String>,
    pub next_action: Option<String>,
    pub structured_choice: Option<String>,
    pub accepted: bool,
    pub observed_next_tagged: bool,
}

pub(in crate::autonomous) fn export_minime_prompt_block_once() -> Option<PathBuf> {
    super::ensure_signal_catalog_seeded();
    let (mut bank, mut ledger) = load_runtime();
    let (episode, proposal, responses) =
        active_owner_view(&bank, &mut ledger, OWNER_MINIME, false)?;
    if proposal
        .prompt_exposures
        .get(OWNER_MINIME)
        .copied()
        .unwrap_or(0)
        > 0
    {
        return None;
    }

    let rendered = render_owner_block(episode, proposal, OWNER_MINIME, &responses, false);
    let note = render_minime_inbox_note(proposal, &responses, &rendered);
    let inbox_dir = bridge_paths().minime_inbox_dir();
    if let Err(error) = std::fs::create_dir_all(&inbox_dir) {
        warn!(%error, path = %inbox_dir.display(), "btsp: failed to create minime inbox");
        return None;
    }
    let path = inbox_dir.join(format!(
        "btsp_proposal_{}_{}.txt",
        proposal.proposal_id,
        now_unix_s()
    ));
    if let Err(error) = std::fs::write(&path, note) {
        warn!(%error, path = %path.display(), "btsp: failed to export minime prompt block");
        return None;
    }

    proposal
        .owner_reply_state
        .insert(OWNER_MINIME.to_string(), "witnessed".to_string());
    proposal.reply_state = super::helpers::recompute_reply_state(proposal);
    record_prompt_render(proposal, OWNER_MINIME, "minime_inbox");
    let episode_id = proposal.episode_id.clone();
    let proposal_id = proposal.proposal_id.clone();
    save_runtime(&mut bank, &mut ledger);
    append_signal_event(
        "minime_prompt_exported",
        json!({
            "episode_id": episode_id,
            "proposal_id": proposal_id,
            "path": path.display().to_string(),
            "detail": "Minime owner-specific BTSP proposal block was written to Minime's inbox with a structured round-trip envelope."
        }),
    );
    Some(path)
}

#[allow(dead_code)]
pub(in crate::autonomous) fn record_minime_next_action(next_action: &str, context: Option<&Value>) {
    let (mut bank, mut ledger) = load_runtime();
    let changed = apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_MINIME,
        next_action,
        Some(merge_minime_context(context, None, None)),
    );
    if changed {
        save_runtime(&mut bank, &mut ledger);
    }
}

pub(in crate::autonomous) fn record_minime_outbox_reply(path: &Path, content: &str) -> bool {
    let reply = parse_minime_btsp_reply(content);
    if !reply.is_btsp_related() {
        return false;
    }

    let source_hash = sha256_hex(content.as_bytes());
    let key = consumed_key(path, &source_hash);
    let cursor_path = bridge_paths().btsp_minime_choice_cursor_path();
    let mut cursor = load_json_or_default::<MinimeChoiceCursor>(&cursor_path);
    if cursor
        .consumed_reply_keys
        .iter()
        .any(|existing| existing == &key)
    {
        append_signal_event(
            "minime_reply_duplicate_ignored",
            json!({
                "path": path.display().to_string(),
                "source_sha256": source_hash,
                "detail": "Minime BTSP reply artifact was already consumed."
            }),
        );
        return false;
    }

    let (mut bank, mut ledger) = load_runtime();
    let changed =
        record_minime_reply_into_runtime(&mut bank, &mut ledger, Some(path), content, &source_hash);
    if changed {
        save_runtime(&mut bank, &mut ledger);
    }
    mark_consumed(&mut cursor, key);
    atomic_write_json(&cursor_path, &cursor);
    changed
}

pub(in crate::autonomous) fn record_minime_reply_into_runtime(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    path: Option<&Path>,
    content: &str,
    source_hash: &str,
) -> bool {
    let reply = parse_minime_btsp_reply(content);
    let Some(choice) = choice_for_reply(bank, ledger, &reply) else {
        if reply.is_btsp_related() {
            append_signal_event(
                "minime_reply_unmatched",
                json!({
                    "proposal_id": reply.proposal_id,
                    "response_id": reply.response_id,
                    "path": path.map(|path| path.display().to_string()),
                    "source_sha256": source_hash,
                    "detail": "Minime reply carried BTSP metadata but no exact, adjacent, refusal, or counter choice could be recorded."
                }),
            );
        }
        return false;
    };

    let reply_context = json!({
            "proposal_id": reply.proposal_id.clone(),
            "response_id": reply.response_id.clone(),
            "accepted": reply.accepted,
            "raw_reply_preview": trim_chars(content, 600),
    });
    let btsp_context = merge_minime_context(Some(&reply_context), path, Some(source_hash));
    let changed = apply_owner_choice(bank, ledger, OWNER_MINIME, &choice, Some(btsp_context));
    if changed {
        append_signal_event(
            "minime_choice_recorded",
            json!({
                "proposal_id": reply.proposal_id,
                "response_id": reply.response_id,
                "choice": choice,
                "path": path.map(|path| path.display().to_string()),
                "source_sha256": source_hash,
                "detail": "Minime BTSP reply was recorded into the bilateral proposal ledger."
            }),
        );
    }
    changed
}

pub(super) fn parse_minime_btsp_reply(content: &str) -> MinimeBtspReply {
    let mut reply = MinimeBtspReply::default();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if let Some(action) = upper
            .starts_with("NEXT:")
            .then(|| trimmed.get(5..).unwrap_or_default().trim())
            .filter(|action| !action.is_empty())
        {
            reply.next_action = Some(action.to_string());
            continue;
        }
        if upper.starts_with("BTSP_PROPOSAL_ID ") {
            reply.proposal_id = token_after(trimmed, "BTSP_PROPOSAL_ID");
            continue;
        }
        if upper.starts_with("BTSP_RESPONSE_ID ") {
            reply.response_id = token_after(trimmed, "BTSP_RESPONSE_ID");
            continue;
        }
        if upper.starts_with("BTSP_OBSERVED_NEXT ") {
            reply.next_action = payload_after(trimmed, "BTSP_OBSERVED_NEXT");
            reply.observed_next_tagged = reply.next_action.is_some();
            continue;
        }
        if upper.starts_with("BTSP_ACCEPT") {
            reply.accepted = true;
            let tokens = trimmed.split_whitespace().skip(1).collect::<Vec<_>>();
            for token in tokens {
                if token.starts_with("btsp_") || token.contains("_proposal_") {
                    reply.proposal_id = Some(token.to_string());
                } else if token.starts_with("minime_") {
                    reply.response_id = Some(token.to_string());
                }
            }
            continue;
        }
        if upper == "BTSP_DECLINE"
            || upper.starts_with("BTSP_REFUSAL ")
            || upper.starts_with("BTSP_COUNTER ")
            || upper.starts_with("BTSP_STUDY_FIRST ")
        {
            reply.structured_choice = Some(trimmed.to_string());
        }
    }
    reply
}

pub(in crate::autonomous) fn render_minime_inbox_note(
    proposal: &ActiveSovereigntyProposal,
    responses: &[NominatedResponse],
    rendered: &str,
) -> String {
    let status = load_json_or_default::<super::signal::SignalStatus>(
        &bridge_paths().btsp_signal_status_path(),
    );
    let envelope = json!({
        "schema": "astrid.btsp.proposal.v2",
        "source": "astrid:btsp_sovereignty_proposal",
        "proposal_id": proposal.proposal_id,
        "episode_id": proposal.episode_id,
        "episode_name": proposal.episode_name,
        "owner": OWNER_MINIME,
        "created_at_unix_s": proposal.created_at_unix_s,
        "expires_at_unix_s": proposal.expires_at_unix_s,
        "signal_fingerprint": proposal.signal_fingerprint,
        "agency_hypothesis": proposal_agency_hypothesis(proposal),
        "reason_codes": proposal_reason_codes(proposal),
        "lineage": proposal_lineage(proposal),
        "evidence_window": proposal_evidence_window(proposal),
        "replay_read": status.replay_read.clone(),
        "anti_loop_reason": status.anti_loop_state.as_ref().and_then(|state| {
            state.active.then(|| state.reason.clone())
        }),
        "anti_loop_prompt": status.anti_loop_state.as_ref().and_then(|state| {
            state.active.then(|| state.counter_prompt.clone())
        }),
        "anti_loop_routes": status.anti_loop_state.as_ref().map(|state| {
            state.suggested_routes.clone()
        }).unwrap_or_default(),
        "causal_lab_v3": status.causal_lab_v3.clone(),
        "causal_lab_question": status.causal_lab_v3.as_ref().and_then(|lab| {
            lab.active.then(|| lab.question.clone())
        }),
        "causal_lab_routes": status.causal_lab_v3.as_ref().map(|lab| {
            lab.consent_routes.clone()
        }).unwrap_or_default(),
        "candidate_response_ids": proposal.candidate_response_ids,
        "candidates": responses.iter().map(|response| {
            json!({
                "response_id": response.response_id,
                "kind": response.kind,
                "action": response.action,
                "parameters": response.parameters,
                "next_action": render_owner_action(OWNER_MINIME, response, false),
                "rationale": response.rationale,
                "policy_state": response.policy_state,
            })
        }).collect::<Vec<_>>(),
    });
    let envelope_json =
        serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string());
    format!(
        "=== BTSP SOVEREIGNTY PROPOSAL FOR MINIME ===\n\
         Source: astrid:btsp_sovereignty_proposal\n\
         Proposal: {}\n\
         Created: {}\n\
         Expires: {}\n\n\
         BTSP_ENVELOPE_JSON_START\n{}\nBTSP_ENVELOPE_JSON_END\n\n\
         This is advisory only. It is an owner-specific view of a bilateral signal proposal; choose only if it feels true.\n\
         If a candidate fits, answer in your normal NEXT syntax using the action shown. Minime will attach BTSP_ACCEPT metadata automatically when the NEXT action exactly matches a candidate.\n\
         If you need a study window before deciding, write BTSP_STUDY_FIRST <reason>; this records agency without treating the proposal as adopted or widened.\n\
         If none fits, continue your present course or name the mismatch with BTSP_REFUSAL not_now, misread, too_forceful, study_first, stay_with_me, or give_me_space.\n\
         If it is close but should be offered differently, write BTSP_COUNTER with a response id, NEXT form, or stance such as softer_contact.\n\n\
         {}\n",
        proposal.proposal_id,
        proposal.created_at_unix_s,
        proposal.expires_at_unix_s,
        envelope_json,
        rendered
    )
}

impl MinimeBtspReply {
    fn is_btsp_related(&self) -> bool {
        self.proposal_id.is_some()
            || self.response_id.is_some()
            || self.structured_choice.is_some()
            || self.accepted
            || self.observed_next_tagged
    }
}

fn choice_for_reply(
    bank: &EpisodeBank,
    ledger: &ProposalLedger,
    reply: &MinimeBtspReply,
) -> Option<String> {
    if let Some(choice) = reply.structured_choice.clone() {
        return Some(choice);
    }
    if let Some(next_action) = reply.next_action.clone() {
        return Some(next_action);
    }
    let response_id = reply.response_id.as_deref()?;
    let proposal_ids = active_minime_proposal_ids(ledger);
    let proposal_matches = reply
        .proposal_id
        .as_ref()
        .is_none_or(|proposal_id| proposal_ids.contains(proposal_id));
    if !proposal_matches {
        return None;
    }
    bank.episodes
        .iter()
        .flat_map(|episode| episode.nominated_responses.iter())
        .find(|response| response.owner == OWNER_MINIME && response.response_id == response_id)
        .map(choice_for_response)
}

fn active_minime_proposal_ids(ledger: &ProposalLedger) -> BTreeSet<String> {
    ledger
        .proposals
        .iter()
        .filter(|proposal| super::is_active_state(&proposal.reply_state))
        .map(|proposal| proposal.proposal_id.clone())
        .collect()
}

fn choice_for_response(response: &NominatedResponse) -> String {
    if response.action.eq_ignore_ascii_case("regime") {
        let regime = response
            .parameters
            .get("regime")
            .and_then(Value::as_str)
            .unwrap_or("recover");
        return format!("REGIME:{regime}");
    }
    response.action.clone()
}

fn merge_minime_context(
    context: Option<&Value>,
    path: Option<&Path>,
    source_hash: Option<&str>,
) -> Value {
    let fill_pct = read_minime_fill_pct();
    json!({
        "selected_at_unix_s": now_unix_s(),
        "source": "minime_outbox",
        "source_path": path.map(|path| path.display().to_string()),
        "source_sha256": source_hash,
        "fill_pct": fill_pct,
        "btsp_reply": context,
    })
}

fn read_minime_fill_pct() -> Option<f64> {
    let health_path = bridge_paths()
        .minime_outbox_dir()
        .parent()
        .map(|parent| parent.join("health.json"))?;
    let raw = std::fs::read_to_string(health_path).ok()?;
    let health = serde_json::from_str::<Value>(&raw).ok()?;
    health.get("fill_pct").and_then(Value::as_f64).or_else(|| {
        health
            .get("fill_ratio")
            .and_then(Value::as_f64)
            .map(|ratio| ratio.mul_add(100.0, 0.0))
    })
}

fn token_after(line: &str, prefix: &str) -> Option<String> {
    line.strip_prefix(prefix)
        .map(str::trim)
        .and_then(|rest| rest.split_whitespace().next())
        .map(|token| token.trim_matches(|ch: char| ch == '`' || ch == '"' || ch == '\''))
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
}

fn payload_after(line: &str, prefix: &str) -> Option<String> {
    line.strip_prefix(prefix)
        .map(str::trim)
        .filter(|payload| !payload.is_empty())
        .map(ToString::to_string)
}

fn consumed_key(path: &Path, source_hash: &str) -> String {
    format!("{}:{source_hash}", path.display())
}

fn mark_consumed(cursor: &mut MinimeChoiceCursor, key: String) {
    if cursor
        .consumed_reply_keys
        .iter()
        .any(|existing| existing == &key)
    {
        return;
    }
    cursor.consumed_reply_keys.push(key);
    if cursor.consumed_reply_keys.len() > CONSUMED_REPLY_LIMIT {
        let overflow = cursor
            .consumed_reply_keys
            .len()
            .saturating_sub(CONSUMED_REPLY_LIMIT);
        cursor.consumed_reply_keys.drain(0..overflow);
    }
    cursor.last_consumed_at_unix_s = now_unix_s();
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest {
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_keeps_recent_consumed_keys_bounded() {
        let mut cursor = MinimeChoiceCursor::default();
        let total = CONSUMED_REPLY_LIMIT.saturating_add(3);
        for index in 0..total {
            mark_consumed(&mut cursor, format!("key_{index}"));
        }
        assert_eq!(cursor.consumed_reply_keys.len(), CONSUMED_REPLY_LIMIT);
        assert!(!cursor.consumed_reply_keys.contains(&"key_0".to_string()));
        assert!(
            cursor
                .consumed_reply_keys
                .contains(&format!("key_{}", total.saturating_sub(1)))
        );
    }
}
