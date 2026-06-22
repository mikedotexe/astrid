use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};
use tracing::info;

use super::helpers::now_unix_s;
use super::lab::record_suppression_hold_v3;
use super::policy::CooldownState;
use super::seed::seeded_response_ids;
use super::signal::{ProposalSignalMatch, append_signal_event};
use super::trace::BTSPAntiLoopState;
use super::{
    ACTIVE_WINDOW_SECS, ActiveSovereigntyProposal, BTSPEpisodeRecord, EPISODE_ID, EPISODE_NAME,
    EpisodeBank, OWNER_ASTRID, OWNER_MINIME, ProposalLedger,
};
use crate::autonomous::state::ConversationState;

const RANK_WINDOW: usize = 36;

pub(super) fn maybe_open_advisory_proposal(
    bank: &EpisodeBank,
    ledger: &mut ProposalLedger,
    conv: &ConversationState,
    matched: &ProposalSignalMatch,
    signal_fingerprint: &str,
    cooldown_state: &CooldownState,
    anti_loop_state: Option<&BTSPAntiLoopState>,
) -> bool {
    if has_active_for_any_owner(ledger) || cooldown_state.active {
        return false;
    }
    if let Some(anti_loop_state) = anti_loop_state
        && anti_loop_state.active
    {
        if record_suppression_hold_v3(signal_fingerprint, anti_loop_state) {
            append_signal_event(
                "proposal_suppressed_anti_loop",
                json!({
                    "episode_id": EPISODE_ID,
                    "signal_fingerprint": signal_fingerprint,
                    "reason": anti_loop_state.reason.clone(),
                    "scope": anti_loop_state.scope.clone(),
                    "same_fingerprint_count": anti_loop_state.same_fingerprint_count,
                    "similar_fingerprint_count": anti_loop_state.similar_fingerprint_count,
                    "reconcentrating_count": anti_loop_state.reconcentrating_count,
                    "widening_count": anti_loop_state.widening_count,
                    "mean_similarity_score": anti_loop_state.mean_similarity_score,
                    "nearest_similarity_score": anti_loop_state.nearest_similarity_score,
                    "suggested_routes": anti_loop_state.suggested_routes.clone(),
                    "counter_prompt": anti_loop_state.counter_prompt.clone(),
                    "recommendation": anti_loop_state.recommendation.clone(),
                    "detail": "BTSP V3.1 replay and causal-lab policy withheld a duplicate advisory proposal; study, refusal, counter, or new evidence routes remain visible instead."
                }),
            );
        }
        return false;
    }
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
        candidate_response_ids: ranked_candidate_response_ids(bank, ledger),
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
        signal_fingerprint: signal_fingerprint.to_string(),
        last_choice_interpretation: None,
        choice_interpretations: Vec::new(),
        exact_adoptions: Vec::new(),
        adoption_contexts: HashMap::new(),
        outcomes: Vec::new(),
        refusals: Vec::new(),
        counteroffers: Vec::new(),
        study_first_records: Vec::new(),
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
            "agency_hypothesis": proposal_agency_hypothesis(&proposal),
            "reason_codes": proposal_reason_codes(&proposal),
            "lineage": proposal_lineage(&proposal),
            "evidence_window": proposal_evidence_window(&proposal),
            "detail": "Rolling BTSP signal match opened a bounded agency-recovery proposal."
        }),
    );
    info!(
        episode_id = proposal.episode_id,
        cues = proposal.matched_cues.join(", "),
        live_signals = proposal.matched_live_signals.join(", "),
        "btsp: created bilateral agency-recovery proposal"
    );
    ledger.proposals.push(proposal);
    true
}

pub(super) fn proposal_agency_hypothesis(proposal: &ActiveSovereigntyProposal) -> String {
    let families = if proposal.matched_signal_families.is_empty() {
        "the current BTSP signal".to_string()
    } else {
        proposal.matched_signal_families.join(", ")
    };
    format!(
        "When {families} returns, offer bounded choices that make yes, no, adjacent uptake, or counteroffer easier to express without forcing adoption."
    )
}

pub(super) fn proposal_reason_codes(proposal: &ActiveSovereigntyProposal) -> Vec<String> {
    let mut codes = vec!["auto_advisory".to_string(), "agency_recovery".to_string()];
    for role in &proposal.matched_signal_roles {
        codes.push(format!("role:{role}"));
    }
    if proposal
        .matched_live_signals
        .iter()
        .any(|signal| signal.contains("tightening"))
    {
        codes.push("live:tightening".to_string());
    }
    if proposal
        .matched_live_signals
        .iter()
        .any(|signal| signal.contains("fill_band_crossing"))
    {
        codes.push("live:fill_band_crossing".to_string());
    }
    if !proposal.counteroffers.is_empty() {
        codes.push("prior:counteroffer".to_string());
    }
    if !proposal.study_first_records.is_empty() {
        codes.push("prior:study_first".to_string());
    }
    codes.sort();
    codes.dedup();
    codes
}

pub(super) fn proposal_lineage(proposal: &ActiveSovereigntyProposal) -> Vec<String> {
    let mut lineage = vec![
        format!("episode:{}", proposal.episode_id),
        "source:astrid:btsp_agency_recovery_v3".to_string(),
    ];
    if !proposal.signal_fingerprint.is_empty() {
        lineage.push(format!(
            "signal_fingerprint:{}",
            proposal.signal_fingerprint
        ));
    }
    if !proposal.matched_signal_families.is_empty() {
        lineage.push(format!(
            "families:{}",
            proposal.matched_signal_families.join("+")
        ));
    }
    lineage
}

pub(super) fn proposal_evidence_window(proposal: &ActiveSovereigntyProposal) -> Value {
    json!({
        "created_at_unix_s": proposal.created_at_unix_s,
        "expires_at_unix_s": proposal.expires_at_unix_s,
        "matched_at_exchange": proposal.matched_at_exchange,
        "matched_cues": proposal.matched_cues.clone(),
        "matched_live_signals": proposal.matched_live_signals.clone(),
        "matched_signal_families": proposal.matched_signal_families.clone(),
        "matched_signal_roles": proposal.matched_signal_roles.clone(),
        "signal_score": proposal.signal_score,
        "signal_fingerprint": proposal.signal_fingerprint.clone(),
    })
}

pub(super) fn ordered_responses_for_proposal(
    episode: &BTSPEpisodeRecord,
    proposal: &ActiveSovereigntyProposal,
    owner: &str,
) -> Vec<super::NominatedResponse> {
    let order = proposal
        .candidate_response_ids
        .iter()
        .enumerate()
        .map(|(index, response_id)| (response_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == owner)
        .filter(|response| order.contains_key(response.response_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    responses.sort_by_key(|response| {
        order
            .get(response.response_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    responses
}

#[allow(clippy::too_many_lines)]
fn ranked_candidate_response_ids(bank: &EpisodeBank, ledger: &ProposalLedger) -> Vec<String> {
    let seed_ids = seeded_response_ids();
    let response_owners = response_owner_index(bank);
    let seed_order = seed_ids
        .iter()
        .enumerate()
        .map(|(index, response_id)| (response_id.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut scores = seed_ids
        .iter()
        .map(|response_id| (response_id.clone(), 0_i32))
        .collect::<HashMap<_, _>>();
    let mut proposals = ledger.proposals.iter().collect::<Vec<_>>();
    proposals.sort_by_key(|proposal| proposal.latest_match_at_unix_s);
    for proposal in proposals.into_iter().rev().take(RANK_WINDOW) {
        for adoption in &proposal.exact_adoptions {
            add_score(scores.entry(adoption.response_id.clone()).or_insert(0), 6);
        }
        for outcome in &proposal.outcomes {
            let score = scores.entry(outcome.response_id.clone()).or_insert(0);
            if outcome.target_nearness == "positive" || outcome.distress_or_recovery == "recovery" {
                add_score(score, 2);
            }
            if outcome.opening_vs_reconcentration == "opening" {
                add_score(score, 2);
            }
            if outcome.opening_vs_reconcentration == "reconcentrating"
                || outcome.distress_or_recovery == "worsening"
            {
                subtract_score(score, 3);
            }
        }
        for refusal in &proposal.refusals {
            for response_id in &proposal.candidate_response_ids {
                if response_owners.get(response_id).map(String::as_str)
                    == Some(refusal.owner.as_str())
                {
                    subtract_score(scores.entry(response_id.clone()).or_insert(0), 3);
                }
            }
        }
        for counteroffer in &proposal.counteroffers {
            if let Some(response_id) = counteroffer.requested_response_id.as_ref() {
                let score = scores.entry(response_id.clone()).or_insert(0);
                if counteroffer.state == "accepted" {
                    add_score(score, 8);
                } else if counteroffer.state == "open" || counteroffer.state == "noted" {
                    add_score(score, 5);
                }
            }
        }
    }
    let mut ranked = seed_ids;
    ranked.sort_by(|left, right| {
        let left_score = scores.get(left).copied().unwrap_or(0);
        let right_score = scores.get(right).copied().unwrap_or(0);
        right_score.cmp(&left_score).then_with(|| {
            seed_order
                .get(left)
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(&seed_order.get(right).copied().unwrap_or(usize::MAX))
        })
    });
    ranked
}

fn add_score(score: &mut i32, amount: i32) {
    *score = score.saturating_add(amount);
}

fn subtract_score(score: &mut i32, amount: i32) {
    *score = score.saturating_sub(amount);
}

fn response_owner_index(bank: &EpisodeBank) -> HashMap<String, String> {
    bank.episodes
        .iter()
        .find(|episode| episode.episode_id == EPISODE_ID)
        .map(|episode| {
            episode
                .nominated_responses
                .iter()
                .map(|response| (response.response_id.clone(), response.owner.clone()))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

fn has_active_for_any_owner(ledger: &ProposalLedger) -> bool {
    ledger
        .proposals
        .iter()
        .any(|proposal| super::is_active_state(&proposal.reply_state))
}
