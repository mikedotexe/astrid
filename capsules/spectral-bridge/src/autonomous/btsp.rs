use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

mod adoption;
mod agency;
mod causality;
mod choice;
mod conversion;
mod helpers;
mod policy;
mod proposal;
mod render;
mod roundtrip;
mod seed;
mod shadow;
mod signal;
mod social;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_agency;
#[cfg(test)]
mod tests_policy;
mod trace;

use super::state::ConversationState;
use crate::paths::bridge_paths;
use adoption::{ExactAdoption, record_exact_adoption};
use agency::{score_adopted_outcomes, score_final_non_adoption_outcomes};
use choice::{
    ChoiceInterpretation, has_choice_interpretation, interpret_choice, interpret_exact_choice,
    is_same_family_adjacent, record_choice_interpretation,
};
use helpers::{
    atomic_write_json, load_json_or_default, normalize_choice, now_unix_s, recompute_reply_state,
    response_matches_choice,
};
use policy::{
    LearnedPolicyEntry, build_signal_fingerprint, cooldown_state_for, hydrate_signal_fingerprints,
    refresh_learned_policy,
};
use render::{render_owner_block, render_signal_guidance};
pub(super) use roundtrip::{export_minime_prompt_block_once, record_minime_outbox_reply};
#[cfg(test)]
pub(super) use roundtrip::{record_minime_reply_into_runtime, render_minime_inbox_note};
use seed::seed_episode;
#[cfg(test)]
use seed::seeded_response_ids;
use shadow::{ShadowEquivalenceRecord, observe_shadow_equivalence, record_shadow_equivalence};
use signal::{
    append_signal_event, ensure_signal_catalog_seeded, evaluate_seeded_episode,
    maybe_record_note_read, persist_signal_status, related_choice_for_owner,
};
use social::{
    CounterofferRecord, PreferenceMemoryEntry, RefusalRecord, StudyFirstRecord,
    apply_structured_signal, has_study_first_record, link_study_first_resolution,
    parse_structured_signal, record_refusal, record_study_first, refresh_preference_memory,
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
    pub study_first_records: Vec<StudyFirstRecord>,
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
    changed |= trace::sync_live_trace_episodes(&mut bank, &ledger);

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

    if let Some(matched) = evaluation.matched {
        changed |= proposal::maybe_open_advisory_proposal(
            &bank,
            &mut ledger,
            conv,
            &matched,
            &signal_fingerprint,
            &cooldown_state,
        );
    }

    if changed {
        save_runtime(&mut bank, &mut ledger);
    }
}

#[allow(clippy::unnecessary_wraps)]
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

#[allow(clippy::unnecessary_wraps)]
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
    let responses = proposal::ordered_responses_for_proposal(episode, proposal, owner);
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

#[allow(clippy::too_many_lines)]
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
            if should_infer_study_first(proposal, owner, &interpretation) {
                let duplicate_study_first = has_study_first_record(
                    proposal,
                    owner,
                    "inquiry_before_intervention",
                    Some(&normalized),
                );
                if duplicate_study_first {
                    append_signal_event(
                        "study_first_duplicate_ignored",
                        json!({
                            "episode_id": proposal.episode_id.clone(),
                            "proposal_id": proposal.proposal_id.clone(),
                            "owner": owner,
                            "choice": normalized,
                            "category": interpretation.category.clone(),
                            "relation_to_proposal": interpretation.relation_to_proposal.clone(),
                            "study_first": true,
                            "signal_families": proposal.matched_signal_families.clone(),
                            "signal_roles": proposal.matched_signal_roles.clone(),
                            "detail": "Owner repeated the same study-first BTSP evidence for this proposal; the runtime kept the prior study-first record and did not rescore it."
                        }),
                    );
                    return false;
                }
                return record_study_first(
                    episode,
                    proposal,
                    owner,
                    "inquiry_before_intervention",
                    "inferred_epistemic_adjacent_after_prior_answer",
                    Some(&normalized),
                    true,
                );
            }
            let linked_study_first_resolution =
                if should_link_study_first_resolution(proposal, owner, &interpretation) {
                    link_study_first_resolution(
                        proposal,
                        owner,
                        &format!("evidence:{}", interpretation.normalized_choice),
                    )
                } else {
                    false
                };
            let duplicate_interpretation = has_choice_interpretation(proposal, &interpretation);
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
            let previous_owner_state = proposal
                .owner_reply_state
                .insert(owner.to_string(), "answered".to_string());
            proposal.reply_state = recompute_reply_state(proposal);
            if duplicate_interpretation {
                append_signal_event(
                    "choice_duplicate_ignored",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": owner,
                        "choice": normalized,
                        "category": interpretation.category.clone(),
                        "relation_to_proposal": interpretation.relation_to_proposal.clone(),
                        "signal_families": proposal.matched_signal_families.clone(),
                        "signal_roles": proposal.matched_signal_roles.clone(),
                        "detail": "Owner repeated the same adjacent BTSP choice for this proposal; the runtime kept the prior interpretation and did not rescore it."
                    }),
                );
                return shadow_changed
                    || linked_study_first_resolution
                    || previous_owner_state.as_deref() != Some("answered");
            }
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
                agency::append_adjacent_choice_agency_event(proposal, owner, &normalized);
                return interpretation_changed
                    || shadow_changed
                    || linked_study_first_resolution
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
            return interpretation_changed || shadow_changed || linked_study_first_resolution;
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
    let _ = link_study_first_resolution(
        proposal,
        owner,
        &format!("exact_accept:{}", response.response_id),
    );
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

fn should_infer_study_first(
    proposal: &ActiveSovereigntyProposal,
    owner: &str,
    interpretation: &ChoiceInterpretation,
) -> bool {
    interpretation.owner == owner
        && is_study_first_choice(interpretation)
        && proposal.choice_interpretations.iter().any(|existing| {
            existing.owner == owner && existing.relation_to_proposal != "exact_nominated"
        })
}

fn is_study_first_choice(interpretation: &ChoiceInterpretation) -> bool {
    if interpretation.category == "epistemic" {
        return true;
    }
    matches!(
        interpretation.normalized_choice.as_str(),
        "BROWSE"
            | "SEARCH"
            | "READ_MORE"
            | "DECOMPOSE"
            | "INTROSPECT"
            | "EXPERIMENT_REVIEW"
            | "EXPERIMENT_EVIDENCE"
    )
}

fn should_link_study_first_resolution(
    proposal: &ActiveSovereigntyProposal,
    owner: &str,
    interpretation: &ChoiceInterpretation,
) -> bool {
    interpretation.owner == owner
        && is_study_first_choice(interpretation)
        && proposal
            .study_first_records
            .iter()
            .rev()
            .any(|record| record.owner == owner && record.resolution_evidence.is_empty())
}

#[cfg(test)]
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
