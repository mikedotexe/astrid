use serde_json::json;
use sha2::{Digest, Sha256};

use super::{BTSPEpisodeRecord, EpisodeBank, ProposalLedger};

const LIVE_TRACE_PREFIX: &str = "btsp_live_trace_";
const MAX_LIVE_TRACE_EPISODES: usize = 48;

pub(super) fn sync_live_trace_episodes(bank: &mut EpisodeBank, ledger: &ProposalLedger) -> bool {
    let live_trace_count = bank
        .episodes
        .iter()
        .filter(|episode| episode.episode_id.starts_with(LIVE_TRACE_PREFIX))
        .count();
    if live_trace_count >= MAX_LIVE_TRACE_EPISODES {
        return false;
    }

    let Some(seed) = bank
        .episodes
        .iter()
        .find(|episode| episode.episode_id == super::EPISODE_ID)
        .cloned()
    else {
        return false;
    };

    let mut changed = false;
    for proposal in &ledger.proposals {
        if proposal.exact_adoptions.is_empty() {
            continue;
        }
        for adoption in &proposal.exact_adoptions {
            if bank
                .episodes
                .iter()
                .filter(|episode| episode.episode_id.starts_with(LIVE_TRACE_PREFIX))
                .count()
                >= MAX_LIVE_TRACE_EPISODES
            {
                return changed;
            }
            let episode_id =
                live_trace_episode_id(proposal, &adoption.owner, &adoption.response_id);
            if bank
                .episodes
                .iter()
                .any(|episode| episode.episode_id == episode_id)
            {
                continue;
            }

            let nominated_responses = seed
                .nominated_responses
                .iter()
                .filter(|response| response.response_id == adoption.response_id)
                .cloned()
                .collect::<Vec<_>>();
            let response_outcomes = proposal
                .outcomes
                .iter()
                .filter(|outcome| {
                    outcome.owner == adoption.owner && outcome.response_id == adoption.response_id
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut retrieval_cues = proposal.matched_cues.clone();
            retrieval_cues.extend(proposal.matched_signal_families.clone());
            retrieval_cues.extend(proposal.matched_live_signals.clone());
            retrieval_cues.sort();
            retrieval_cues.dedup();

            bank.episodes.push(BTSPEpisodeRecord {
                episode_id,
                episode_name: format!(
                    "Live BTSP trace: {} -> {}",
                    adoption.owner, adoption.response_id
                ),
                revision: 1,
                kind: "BTSP live eligibility trace".to_string(),
                audience: proposal.audience.clone(),
                reply_state: proposal.reply_state.clone(),
                credited_trace: json!({
                    "kind": "live_round_trip_choice",
                    "proposal_id": proposal.proposal_id,
                    "owner": adoption.owner,
                    "response_id": adoption.response_id,
                    "raw_choice": adoption.raw_choice,
                    "normalized_choice": adoption.normalized_choice,
                    "adopted_at_unix_s": adoption.adopted_at_unix_s,
                    "context": adoption.context,
                }),
                instructive_event: json!({
                    "kind": "btsp_signal_window",
                    "matched_cues": proposal.matched_cues,
                    "matched_live_signals": proposal.matched_live_signals,
                    "matched_signal_families": proposal.matched_signal_families,
                    "matched_signal_roles": proposal.matched_signal_roles,
                    "signal_score": proposal.signal_score,
                    "signal_fingerprint": proposal.signal_fingerprint,
                }),
                outcome_vector: json!({
                    "status": proposal.outcome_status,
                    "reply_state": proposal.reply_state,
                    "related_choice": proposal.related_choice,
                    "scored_outcomes": response_outcomes.len(),
                }),
                confidence: proposal.confidence,
                learned_score: 0.0,
                retrieval_cues,
                retrieval_refinement: Some(json!({
                    "source_episode_id": seed.episode_id,
                    "source_proposal_id": proposal.proposal_id,
                    "trace_policy": "bounded bridge-owned dynamic trace; no direct reservoir mutation",
                })),
                family_learning_notes: Vec::new(),
                learned_policy: Vec::new(),
                preference_memory: Vec::new(),
                nominated_responses,
                response_outcomes,
            });
            changed = true;
        }
    }
    changed
}

fn live_trace_episode_id(
    proposal: &super::ActiveSovereigntyProposal,
    owner: &str,
    response_id: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(proposal.proposal_id.as_bytes());
    hasher.update(b":");
    hasher.update(proposal.signal_fingerprint.as_bytes());
    hasher.update(b":");
    hasher.update(owner.as_bytes());
    hasher.update(b":");
    hasher.update(response_id.as_bytes());
    let digest = hasher.finalize();
    let short = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{LIVE_TRACE_PREFIX}{short}")
}
