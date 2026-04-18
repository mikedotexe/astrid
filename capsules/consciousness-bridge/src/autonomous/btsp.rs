use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

mod adoption;
mod choice;
mod conversion;
mod helpers;
mod policy;
mod render;
mod seed;
mod shadow;
mod signal;
mod social;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_policy;

use super::state::ConversationState;
use crate::paths::bridge_paths;
use adoption::{ExactAdoption, exact_adoptions_for_scoring, record_exact_adoption};
use choice::{
    ChoiceInterpretation, interpret_choice, interpret_exact_choice, is_same_family_adjacent,
    record_choice_interpretation,
};
use helpers::{
    atomic_write_json, build_non_adoption_outcome, load_json_or_default, normalize_choice,
    now_unix_s, push_unique_outcome, recompute_reply_state, response_matches_choice,
};
use policy::{
    LearnedPolicyEntry, build_signal_fingerprint, cooldown_state_for, hydrate_signal_fingerprints,
    refresh_learned_policy,
};
use render::{render_owner_block, render_signal_guidance};
use seed::{seed_episode, seeded_response_ids};
use shadow::{ShadowEquivalenceRecord, observe_shadow_equivalence, record_shadow_equivalence};
use signal::{
    append_signal_event, ensure_signal_catalog_seeded, evaluate_seeded_episode,
    learning_note_for_outcome, maybe_record_note_read, persist_signal_status,
    related_choice_for_owner,
};
use social::{
    CounterofferRecord, PreferenceMemoryEntry, RefusalRecord, apply_structured_signal,
    parse_structured_signal, record_refusal, refresh_preference_memory,
    resolve_counteroffers_for_exact_adoption,
};

const EPISODE_ID: &str = "btsp_ep_2026_04_16_phase_note_transition_recovery_01";
const EPISODE_NAME: &str = "Phase note -> transition -> recovery";
const OWNER_ASTRID: &str = "astrid";
const OWNER_MINIME: &str = "minime";
const OWNER_SYSTEM: &str = "system";
const ACTIVE_WINDOW_SECS: u64 = 20 * 60;
const ACTIVE_STATES: [&str; 4] = ["unseen", "witnessed", "answered", "adopted"];

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct EpisodeBank {
    #[serde(default)]
    pub episodes: Vec<BTSPEpisodeRecord>,
    #[serde(default)]
    pub last_updated_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct BTSPEpisodeRecord {
    pub episode_id: String,
    pub episode_name: String,
    pub revision: u32,
    pub kind: String,
    pub audience: String,
    pub reply_state: String,
    pub credited_trace: Value,
    pub instructive_event: Value,
    pub outcome_vector: Value,
    pub confidence: f32,
    pub learned_score: f32,
    #[serde(default)]
    pub retrieval_cues: Vec<String>,
    #[serde(default)]
    pub retrieval_refinement: Option<Value>,
    #[serde(default)]
    pub family_learning_notes: Vec<String>,
    #[serde(default)]
    pub learned_policy: Vec<LearnedPolicyEntry>,
    #[serde(default)]
    pub preference_memory: Vec<PreferenceMemoryEntry>,
    #[serde(default)]
    pub nominated_responses: Vec<NominatedResponse>,
    #[serde(default)]
    pub response_outcomes: Vec<ResponseOutcomeNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct NominatedResponse {
    pub response_id: String,
    pub owner: String,
    pub kind: String,
    pub action: String,
    #[serde(default)]
    pub parameters: Value,
    pub rationale: String,
    pub policy_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct ProposalLedger {
    #[serde(default)]
    pub proposals: Vec<ActiveSovereigntyProposal>,
    #[serde(default)]
    pub last_updated_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct ActiveSovereigntyProposal {
    pub proposal_id: String,
    pub episode_id: String,
    pub episode_name: String,
    #[serde(default)]
    pub matched_cues: Vec<String>,
    #[serde(default)]
    pub matched_live_signals: Vec<String>,
    #[serde(default)]
    pub matched_signal_families: Vec<String>,
    #[serde(default)]
    pub matched_signal_roles: Vec<String>,
    #[serde(default)]
    pub signal_score: f32,
    pub confidence: f32,
    pub audience: String,
    #[serde(default)]
    pub candidate_response_ids: Vec<String>,
    pub reply_state: String,
    #[serde(default)]
    pub selected_response_id: Option<String>,
    #[serde(default)]
    pub latest_selected_response_id: Option<String>,
    #[serde(default)]
    pub selected_response_ids_by_owner: HashMap<String, String>,
    #[serde(default)]
    pub owner_reply_state: HashMap<String, String>,
    pub outcome_status: String,
    pub created_at_unix_s: u64,
    pub expires_at_unix_s: u64,
    pub matched_at_exchange: u64,
    pub latest_match_at_unix_s: u64,
    #[serde(default)]
    pub prompt_exposures: HashMap<String, u32>,
    #[serde(default)]
    pub related_choice: Option<String>,
    #[serde(default)]
    pub signal_fingerprint: String,
    #[serde(default)]
    pub last_choice_interpretation: Option<ChoiceInterpretation>,
    #[serde(default)]
    pub choice_interpretations: Vec<ChoiceInterpretation>,
    #[serde(default)]
    pub exact_adoptions: Vec<ExactAdoption>,
    #[serde(default)]
    pub adoption_contexts: HashMap<String, Value>,
    #[serde(default)]
    pub outcomes: Vec<ResponseOutcomeNote>,
    #[serde(default)]
    pub refusals: Vec<RefusalRecord>,
    #[serde(default)]
    pub counteroffers: Vec<CounterofferRecord>,
    #[serde(default)]
    pub last_negotiation_event_at_unix_s: u64,
    #[serde(default)]
    pub shadow_equivalences: Vec<ShadowEquivalenceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct ResponseOutcomeNote {
    pub proposal_id: String,
    pub response_id: String,
    pub owner: String,
    pub recorded_at_unix_s: u64,
    pub target_nearness: String,
    pub distress_or_recovery: String,
    pub opening_vs_reconcentration: String,
    pub note: String,
}

pub(super) fn refresh_runtime(conv: &ConversationState, controller_health: Option<&Value>) {
    ensure_signal_catalog_seeded();
    let (mut bank, mut ledger) = load_runtime();
    let mut changed = false;
    changed |= hydrate_signal_fingerprints(&mut ledger);

    changed |= expire_stale_proposals(&mut ledger);
    changed |= score_adopted_outcomes(&mut bank, &mut ledger, controller_health);
    changed |= score_final_non_adoption_outcomes(&mut bank, &mut ledger, controller_health);
    changed |= refresh_seeded_episode_learning(&mut bank, &ledger);

    let evaluation = evaluate_seeded_episode(controller_health);
    let previous_status =
        load_json_or_default::<signal::SignalStatus>(&bridge_paths().btsp_signal_status_path());
    let signal_fingerprint = evaluation
        .matched
        .as_ref()
        .map(|matched| {
            build_signal_fingerprint(&matched.matched_signal_families, controller_health)
        })
        .unwrap_or_default();
    let cooldown_state = cooldown_state_for(&ledger, EPISODE_ID, &signal_fingerprint);
    let episode = bank
        .episodes
        .iter()
        .find(|episode| episode.episode_id == EPISODE_ID);
    let active_proposal = ledger.proposals.iter().find(|proposal| {
        proposal.episode_id == EPISODE_ID && is_active_state(&proposal.reply_state)
    });
    let status = signal::decorate_signal_status(
        evaluation.status,
        previous_status.conversion_state.as_ref(),
        &ledger,
        episode,
        cooldown_state.clone(),
        active_proposal,
        controller_health,
    );
    persist_signal_status(&status);

    if !has_active_proposal(&ledger, EPISODE_ID)
        && !cooldown_state.active
        && let Some(matched) = evaluation.matched
    {
        let created_at_unix_s = now_unix_s();
        let proposal = ActiveSovereigntyProposal {
            proposal_id: format!("{EPISODE_ID}_proposal_{created_at_unix_s}"),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: matched.matched_cues.clone(),
            matched_live_signals: matched.live_signals.clone(),
            matched_signal_families: matched.matched_signal_families.clone(),
            matched_signal_roles: matched.matched_signal_roles.clone(),
            signal_score: matched.signal_score,
            confidence: matched.signal_score,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "unseen".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::from([
                (OWNER_ASTRID.to_string(), "unseen".to_string()),
                (OWNER_MINIME.to_string(), "unseen".to_string()),
            ]),
            outcome_status: "pending".to_string(),
            created_at_unix_s,
            expires_at_unix_s: created_at_unix_s.saturating_add(ACTIVE_WINDOW_SECS),
            matched_at_exchange: conv.exchange_count,
            latest_match_at_unix_s: created_at_unix_s,
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint: signal_fingerprint.clone(),
            last_choice_interpretation: None,
            choice_interpretations: Vec::new(),
            exact_adoptions: Vec::new(),
            adoption_contexts: HashMap::new(),
            outcomes: Vec::new(),
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        };
        append_signal_event(
            "signal_matched",
            json!({
                "episode_id": EPISODE_ID,
                "proposal_id": proposal.proposal_id.clone(),
                "signal_families": proposal.matched_signal_families.clone(),
                "signal_roles": proposal.matched_signal_roles.clone(),
                "signal_score": proposal.signal_score,
                "matched_cues": proposal.matched_cues.clone(),
                "live_signals": proposal.matched_live_signals.clone(),
                "detail": "Rolling BTSP signal match opened a bounded proposal."
            }),
        );
        ledger.proposals.push(proposal);
        changed = true;
        info!(
            episode_id = EPISODE_ID,
            cues = matched.matched_cues.join(", "),
            live_signals = matched.live_signals.join(", "),
            "btsp: created bilateral sovereignty proposal"
        );
    }

    if changed {
        save_runtime(&mut bank, &mut ledger);
    }
}

pub(super) fn render_astrid_prompt_block() -> Option<String> {
    ensure_signal_catalog_seeded();
    let (mut bank, mut ledger) = load_runtime();
    let Some((episode, proposal, responses)) =
        active_owner_view(&bank, &mut ledger, OWNER_ASTRID, true)
    else {
        return Some(render_signal_guidance());
    };
    let rendered = render_owner_block(episode, proposal, OWNER_ASTRID, &responses, false);
    record_prompt_render(proposal, OWNER_ASTRID, "astrid_prompt");
    save_runtime(&mut bank, &mut ledger);
    Some(rendered)
}

pub(super) fn render_astrid_initiation_seed() -> Option<String> {
    ensure_signal_catalog_seeded();
    let (mut bank, mut ledger) = load_runtime();
    let Some((episode, proposal, responses)) =
        active_owner_view(&bank, &mut ledger, OWNER_ASTRID, true)
    else {
        return Some(render_signal_guidance());
    };
    let rendered = render_owner_block(episode, proposal, OWNER_ASTRID, &responses, true);
    record_prompt_render(proposal, OWNER_ASTRID, "astrid_initiation");
    save_runtime(&mut bank, &mut ledger);
    Some(rendered)
}

pub(super) fn record_astrid_next_action(next_action: &str, fill_pct: f32) {
    let (mut bank, mut ledger) = load_runtime();
    let changed = apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_ASTRID,
        next_action,
        Some(json!({
            "selected_at_unix_s": now_unix_s(),
            "fill_pct": fill_pct,
        })),
    );
    if changed {
        save_runtime(&mut bank, &mut ledger);
    }
}

pub(super) fn record_astrid_inbox_read(path: &Path, content: &str) {
    maybe_record_note_read(path, OWNER_ASTRID, content);
}

fn load_runtime() -> (EpisodeBank, ProposalLedger) {
    ensure_signal_catalog_seeded();
    let bank_path = bridge_paths().btsp_episode_bank_path();
    let proposal_path = bridge_paths().sovereignty_proposals_path();

    let mut bank = load_json_or_default::<EpisodeBank>(&bank_path);
    let ledger = load_json_or_default::<ProposalLedger>(&proposal_path);

    if upsert_seed_episode(&mut bank) {
        let mut cloned_ledger = ledger.clone();
        save_runtime(&mut bank, &mut cloned_ledger);
        return (bank, ledger);
    }

    (bank, ledger)
}

fn save_runtime(bank: &mut EpisodeBank, ledger: &mut ProposalLedger) {
    bank.last_updated_unix_s = now_unix_s();
    ledger.last_updated_unix_s = now_unix_s();
    atomic_write_json(&bridge_paths().btsp_episode_bank_path(), bank);
    atomic_write_json(&bridge_paths().sovereignty_proposals_path(), ledger);
}

fn upsert_seed_episode(bank: &mut EpisodeBank) -> bool {
    let seeded = seed_episode();
    if let Some(existing) = bank
        .episodes
        .iter_mut()
        .find(|episode| episode.episode_id == seeded.episode_id)
    {
        let preserve_outcomes = existing.response_outcomes.clone();
        let preserve_notes = existing.family_learning_notes.clone();
        let preserve_policy = existing.learned_policy.clone();
        let preserve_preferences = existing.preference_memory.clone();
        let mut comparable_existing = existing.clone();
        comparable_existing.response_outcomes = Vec::new();
        comparable_existing.family_learning_notes = Vec::new();
        comparable_existing.learned_policy = Vec::new();
        comparable_existing.preference_memory = Vec::new();
        let was_same = comparable_existing == seeded;
        *existing = seeded;
        existing.response_outcomes = preserve_outcomes;
        existing.family_learning_notes = preserve_notes;
        existing.learned_policy = preserve_policy;
        existing.preference_memory = preserve_preferences;
        return !was_same;
    }
    bank.episodes.push(seeded);
    true
}

fn refresh_seeded_episode_learning(bank: &mut EpisodeBank, ledger: &ProposalLedger) -> bool {
    let Some(episode) = bank
        .episodes
        .iter_mut()
        .find(|episode| episode.episode_id == EPISODE_ID)
    else {
        return false;
    };
    let mut changed = refresh_learned_policy(episode);
    changed |= refresh_preference_memory(episode, ledger);
    changed
}

fn expire_stale_proposals(ledger: &mut ProposalLedger) -> bool {
    let mut changed = false;
    let now = now_unix_s();
    for proposal in &mut ledger.proposals {
        if is_final_state(&proposal.reply_state) || proposal.reply_state == "adopted" {
            continue;
        }
        if now > proposal.expires_at_unix_s {
            proposal.reply_state = "expired".to_string();
            proposal.outcome_status = "expired".to_string();
            append_signal_event(
                "expired",
                json!({
                    "episode_id": proposal.episode_id.clone(),
                    "proposal_id": proposal.proposal_id.clone(),
                    "signal_families": proposal.matched_signal_families.clone(),
                    "signal_roles": proposal.matched_signal_roles.clone(),
                    "detail": "Bounded proposal expired before exact adoption."
                }),
            );
            changed = true;
        }
    }
    changed
}

fn score_adopted_outcomes(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    controller_health: Option<&Value>,
) -> bool {
    let Some(health) = controller_health else {
        return false;
    };
    let target_fill = health
        .get("target_fill_pct")
        .and_then(Value::as_f64)
        .unwrap_or(55.0) as f32;
    let current_fill = health
        .get("fill_pct")
        .and_then(Value::as_f64)
        .unwrap_or(target_fill as f64) as f32;
    let fill_band = health
        .get("fill_band")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let phase = health
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let shape_verdict = health
        .get("perturb_visibility")
        .and_then(|value| value.get("shape_verdict"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let mut changed = false;
    for proposal in &mut ledger.proposals {
        if proposal.reply_state != "adopted" || proposal.outcome_status == "integrated" {
            continue;
        }
        let Some(episode) = bank
            .episodes
            .iter_mut()
            .find(|episode| episode.episode_id == proposal.episode_id)
        else {
            continue;
        };

        let selections = exact_adoptions_for_scoring(proposal);
        for adoption in selections {
            let owner = adoption.owner.clone();
            let response_id = adoption.response_id.clone();
            if proposal
                .outcomes
                .iter()
                .any(|outcome| outcome.owner == owner && outcome.response_id == response_id)
            {
                continue;
            }

            let before_fill = adoption
                .context
                .as_ref()
                .and_then(|value| value.get("fill_pct"))
                .and_then(Value::as_f64)
                .map(|value| value as f32)
                .or_else(|| {
                    proposal
                        .adoption_contexts
                        .get(&owner)
                        .and_then(|value| value.get("fill_pct"))
                        .and_then(Value::as_f64)
                        .map(|value| value as f32)
                })
                .unwrap_or(current_fill);
            let before_gap = (target_fill - before_fill).abs();
            let after_gap = (target_fill - current_fill).abs();
            let target_nearness = if after_gap + 0.75 < before_gap {
                "positive"
            } else if before_gap + 0.75 < after_gap {
                "negative"
            } else {
                "mixed"
            };
            let distress_or_recovery = match fill_band {
                "near" | "over" if current_fill >= before_fill => "recovery",
                "under" if current_fill < before_fill => "worsening",
                _ => "mixed",
            };
            let opening_vs_reconcentration = match shape_verdict {
                "tightening" => "reconcentrating",
                "softened_only" => "mixed",
                _ if matches!(phase, "plateau" | "expanding") => "opening",
                _ => "mixed",
            };
            let note = format!(
                "Outcome after {owner} selected {response_id}: target_nearness={target_nearness}, distress_or_recovery={distress_or_recovery}, opening_vs_reconcentration={opening_vs_reconcentration}, phase={phase}, fill_band={fill_band}, shape_verdict={shape_verdict}."
            );
            let outcome = ResponseOutcomeNote {
                proposal_id: proposal.proposal_id.clone(),
                response_id: response_id.clone(),
                owner: owner.clone(),
                recorded_at_unix_s: now_unix_s(),
                target_nearness: target_nearness.to_string(),
                distress_or_recovery: distress_or_recovery.to_string(),
                opening_vs_reconcentration: opening_vs_reconcentration.to_string(),
                note,
            };
            proposal.outcomes.push(outcome.clone());
            if !episode.response_outcomes.iter().any(|existing| {
                existing.proposal_id == outcome.proposal_id
                    && existing.owner == outcome.owner
                    && existing.response_id == outcome.response_id
            }) {
                episode.response_outcomes.push(outcome.clone());
            }
            if let Some(learning_note) = learning_note_for_outcome(proposal, &outcome)
                && !episode
                    .family_learning_notes
                    .iter()
                    .any(|existing| existing == &learning_note)
            {
                episode.family_learning_notes.push(learning_note.clone());
                append_signal_event(
                    "outcome_scored",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": outcome.owner.clone(),
                        "response_id": outcome.response_id.clone(),
                        "signal_families": proposal.matched_signal_families.clone(),
                        "signal_roles": proposal.matched_signal_roles.clone(),
                        "learning_note": learning_note,
                        "detail": outcome.note.clone()
                    }),
                );
            } else {
                append_signal_event(
                    "outcome_scored",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": outcome.owner.clone(),
                        "response_id": outcome.response_id.clone(),
                        "signal_families": proposal.matched_signal_families.clone(),
                        "signal_roles": proposal.matched_signal_roles.clone(),
                        "detail": outcome.note.clone()
                    }),
                );
            }
            changed = true;
        }

        if !proposal.outcomes.is_empty() {
            proposal.outcome_status = "integrated".to_string();
            proposal.reply_state = "integrated".to_string();
            changed = true;
        }
    }
    changed
}

fn score_final_non_adoption_outcomes(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    controller_health: Option<&Value>,
) -> bool {
    let mut changed = false;
    for proposal in &mut ledger.proposals {
        if proposal.outcome_status == "integrated" {
            continue;
        }
        let Some(episode) = bank
            .episodes
            .iter_mut()
            .find(|episode| episode.episode_id == proposal.episode_id)
        else {
            continue;
        };

        if proposal.reply_state == "declined" {
            let owner_states = proposal.owner_reply_state.clone();
            for (owner, state) in owner_states {
                if state != "declined" {
                    continue;
                }
                let outcome = build_non_adoption_outcome(
                    proposal,
                    &owner,
                    "continue_current_course",
                    "Owner declined the proposal and continued the current course.",
                    controller_health,
                );
                changed |= push_unique_outcome(episode, proposal, outcome);
            }
            if !proposal.outcomes.is_empty() {
                proposal.outcome_status = "integrated".to_string();
                changed = true;
            }
        } else if proposal.reply_state == "expired" {
            let owner_states = proposal.owner_reply_state.clone();
            for (owner, state) in owner_states {
                if state != "declined" {
                    continue;
                }
                let outcome = build_non_adoption_outcome(
                    proposal,
                    &owner,
                    "continue_current_course",
                    "Owner declined the proposal before expiry and continued the current course.",
                    controller_health,
                );
                changed |= push_unique_outcome(episode, proposal, outcome);
            }

            let owner_state_summary = if proposal.owner_reply_state.is_empty() {
                "none observed".to_string()
            } else {
                proposal
                    .owner_reply_state
                    .iter()
                    .map(|(owner, state)| format!("{owner}={state}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let outcome = build_non_adoption_outcome(
                proposal,
                OWNER_SYSTEM,
                "proposal_expired",
                &format!(
                    "Proposal expired without nominated response adoption; owner_states={owner_state_summary}."
                ),
                controller_health,
            );
            changed |= push_unique_outcome(episode, proposal, outcome);
            if !proposal.outcomes.is_empty() {
                proposal.outcome_status = "integrated".to_string();
                changed = true;
            }
        }
    }
    changed
}

fn active_owner_view<'a>(
    bank: &'a EpisodeBank,
    ledger: &'a mut ProposalLedger,
    owner: &str,
    mark_witnessed: bool,
) -> Option<(
    &'a BTSPEpisodeRecord,
    &'a mut ActiveSovereigntyProposal,
    Vec<NominatedResponse>,
)> {
    let proposal = ledger.proposals.iter_mut().find(|proposal| {
        proposal.episode_id == EPISODE_ID && is_active_state(&proposal.reply_state)
    })?;
    let episode = bank
        .episodes
        .iter()
        .find(|episode| episode.episode_id == proposal.episode_id)?;
    let responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == owner)
        .filter(|response| {
            proposal
                .candidate_response_ids
                .iter()
                .any(|candidate| candidate == &response.response_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    if responses.is_empty() {
        return None;
    }

    if mark_witnessed {
        let owner_state = proposal
            .owner_reply_state
            .entry(owner.to_string())
            .or_insert_with(|| "unseen".to_string());
        if owner_state == "unseen" {
            *owner_state = "witnessed".to_string();
        }
        if proposal.reply_state == "unseen" {
            proposal.reply_state = "witnessed".to_string();
        }
    }

    Some((episode, proposal, responses))
}

fn record_prompt_render(
    proposal: &mut ActiveSovereigntyProposal,
    owner: &str,
    prompt_surface: &str,
) {
    let status =
        load_json_or_default::<signal::SignalStatus>(&bridge_paths().btsp_signal_status_path());
    let entry = proposal
        .prompt_exposures
        .entry(owner.to_string())
        .or_insert(0);
    *entry = entry.saturating_add(1);
    append_signal_event(
        "prompt_rendered",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": owner,
            "prompt_surface": prompt_surface,
            "prompt_exposures": proposal.prompt_exposures.clone(),
            "signal_families": proposal.matched_signal_families.clone(),
            "signal_roles": proposal.matched_signal_roles.clone(),
            "astrid_shadow_policy": status.astrid_shadow_policy,
            "detail": "Live signal guidance was rendered into prompt context."
        }),
    );
}

fn apply_owner_choice(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    owner: &str,
    choice: &str,
    adoption_context: Option<Value>,
) -> bool {
    let Some(proposal) = ledger.proposals.iter_mut().find(|proposal| {
        proposal.episode_id == EPISODE_ID && is_active_state(&proposal.reply_state)
    }) else {
        return false;
    };
    let Some(episode) = bank
        .episodes
        .iter_mut()
        .find(|episode| episode.episode_id == proposal.episode_id)
    else {
        return false;
    };
    if let Some(signal) = parse_structured_signal(choice) {
        return apply_structured_signal(episode, proposal, owner, signal);
    }
    let normalized = normalize_choice(choice);

    if normalized == "PASS" {
        return record_refusal(episode, proposal, owner, "not_now", "PASS");
    }

    let matched = episode.nominated_responses.iter().find(|response| {
        response.owner == owner
            && proposal
                .candidate_response_ids
                .iter()
                .any(|candidate| candidate == &response.response_id)
            && response_matches_choice(response, &normalized)
    });

    let Some(response) = matched else {
        if let Some(interpretation) = interpret_choice(owner, choice, &normalized) {
            let interpretation_changed =
                record_choice_interpretation(proposal, interpretation.clone());
            let shadow_changed = if let Some(record) =
                observe_shadow_equivalence(owner, &interpretation)
            {
                let changed = record_shadow_equivalence(proposal, record.clone());
                append_signal_event(
                    "shadow_equivalence_observed",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": owner,
                        "choice": normalized,
                        "shadow_key": record.shadow_key.clone(),
                        "preference_key": record.preference_key.clone(),
                        "equivalent_response_family": record.equivalent_response_family.clone(),
                        "confidence": record.confidence.clone(),
                        "note": record.note.clone(),
                        "signal_families": proposal.matched_signal_families.clone(),
                        "signal_roles": proposal.matched_signal_roles.clone(),
                        "detail": "Astrid adjacent choice was recognized as a possible BTSP-equivalent shadow uptake."
                    }),
                );
                changed
            } else {
                false
            };
            proposal
                .owner_reply_state
                .insert(owner.to_string(), "answered".to_string());
            proposal.reply_state = recompute_reply_state(proposal);
            if is_same_family_adjacent(&interpretation)
                || related_choice_for_owner(owner, &normalized)
            {
                let related = format!("{owner}:{normalized}");
                let previous = proposal.related_choice.clone();
                proposal.related_choice = Some(related.clone());
                append_signal_event(
                    "choice_related",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": owner,
                        "choice": normalized,
                        "related_choice": related,
                        "category": interpretation.category.clone(),
                        "likely_intent": interpretation.likely_intent.clone(),
                        "relation_to_proposal": interpretation.relation_to_proposal.clone(),
                        "note": interpretation.note.clone(),
                        "signal_families": proposal.matched_signal_families.clone(),
                        "signal_roles": proposal.matched_signal_roles.clone(),
                        "detail": "Owner chose an adjacent same-family response while the proposal was active."
                    }),
                );
                return interpretation_changed
                    || shadow_changed
                    || previous.as_deref() != proposal.related_choice.as_deref();
            }
            append_signal_event(
                "choice_interpreted",
                json!({
                    "episode_id": proposal.episode_id.clone(),
                    "proposal_id": proposal.proposal_id.clone(),
                    "owner": owner,
                    "choice": normalized,
                    "category": interpretation.category.clone(),
                    "likely_intent": interpretation.likely_intent.clone(),
                    "relation_to_proposal": interpretation.relation_to_proposal.clone(),
                    "note": interpretation.note.clone(),
                    "signal_families": proposal.matched_signal_families.clone(),
                    "signal_roles": proposal.matched_signal_roles.clone(),
                    "detail": "Owner made an interpretable adjacent choice while the proposal was active."
                }),
            );
            return interpretation_changed || shadow_changed;
        }
        let previous = proposal
            .owner_reply_state
            .insert(owner.to_string(), "answered".to_string());
        proposal.reply_state = recompute_reply_state(proposal);
        return previous.as_deref() != Some("answered");
    };

    proposal.reply_state = "adopted".to_string();
    proposal
        .owner_reply_state
        .insert(owner.to_string(), "adopted".to_string());
    proposal.outcome_status = "pending".to_string();
    if let Some(context) = adoption_context {
        proposal
            .adoption_contexts
            .insert(owner.to_string(), context);
    }
    let adoption = ExactAdoption::new(
        owner,
        &response.response_id,
        choice,
        &normalized,
        proposal.adoption_contexts.get(owner).cloned(),
    );
    record_exact_adoption(proposal, adoption);
    let interpretation = interpret_exact_choice(owner, choice, &normalized, response);
    record_choice_interpretation(proposal, interpretation.clone());
    for counteroffer in
        resolve_counteroffers_for_exact_adoption(proposal, owner, &response.response_id)
    {
        append_signal_event(
            "counteroffer_resolved",
            json!({
                "episode_id": proposal.episode_id.clone(),
                "proposal_id": proposal.proposal_id.clone(),
                "owner": counteroffer.owner.clone(),
                "target_owner": counteroffer.target_owner.clone(),
                "requested_response_id": counteroffer.requested_response_id.clone(),
                "requested_stance": counteroffer.requested_stance.clone(),
                "state": counteroffer.state.clone(),
                "detail": "Counteroffer resolved through exact bounded adoption."
            }),
        );
    }
    append_signal_event(
        "choice_exact",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": owner,
            "choice": normalized,
            "response_id": response.response_id.clone(),
            "category": interpretation.category.clone(),
            "likely_intent": interpretation.likely_intent.clone(),
            "relation_to_proposal": interpretation.relation_to_proposal.clone(),
            "signal_families": proposal.matched_signal_families.clone(),
            "signal_roles": proposal.matched_signal_roles.clone(),
            "detail": "Owner adopted an exact nominated response."
        }),
    );
    info!(
        episode_id = proposal.episode_id,
        owner,
        response_id = response.response_id,
        "btsp: owner adopted nominated response"
    );
    true
}

fn has_active_proposal(ledger: &ProposalLedger, episode_id: &str) -> bool {
    ledger
        .proposals
        .iter()
        .any(|proposal| proposal.episode_id == episode_id && is_active_state(&proposal.reply_state))
}

fn is_active_state(state: &str) -> bool {
    ACTIVE_STATES.iter().any(|active| active == &state)
}

fn is_final_state(state: &str) -> bool {
    matches!(state, "declined" | "expired" | "integrated")
}
