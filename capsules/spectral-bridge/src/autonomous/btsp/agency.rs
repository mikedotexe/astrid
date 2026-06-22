use serde_json::{Value, json};

use super::adoption::exact_adoptions_for_scoring;
use super::helpers::{
    build_non_adoption_outcome, classify_live_state, now_unix_s, outcome_telemetry_from_health,
    push_unique_outcome,
};
use super::signal::append_signal_event;
use super::{
    ActiveSovereigntyProposal, EpisodeBank, OWNER_SYSTEM, ProposalLedger, ResponseOutcomeNote,
};

const RESPONSE_ADJACENT_UPTAKE: &str = "adjacent_uptake";
pub(super) const RESPONSE_STUDY_FIRST: &str = "study_first";

#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
pub(super) fn score_adopted_outcomes(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    controller_health: Option<&Value>,
) -> bool {
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

        for adoption in exact_adoptions_for_scoring(proposal) {
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
                });
            let (target_nearness, distress_or_recovery, opening_vs_reconcentration, details) =
                classify_live_state(controller_health, before_fill);
            let reason_codes = agency_reason_codes(
                "exact_accept",
                &target_nearness,
                &distress_or_recovery,
                &opening_vs_reconcentration,
            );
            let agency_score = agency_recovery_score(
                1.0,
                &target_nearness,
                &distress_or_recovery,
                &opening_vs_reconcentration,
            );
            let note = format!(
                "Outcome after {owner} selected {response_id}: target_nearness={target_nearness}, distress_or_recovery={distress_or_recovery}, opening_vs_reconcentration={opening_vs_reconcentration}. Agency outcome exact_accept score={agency_score:.2} reason_codes={}. {details}",
                reason_codes.join(",")
            );
            let outcome = ResponseOutcomeNote {
                proposal_id: proposal.proposal_id.clone(),
                response_id: response_id.clone(),
                owner: owner.clone(),
                recorded_at_unix_s: now_unix_s(),
                target_nearness,
                distress_or_recovery,
                opening_vs_reconcentration,
                outcome_telemetry_v2: outcome_telemetry_from_health(controller_health),
                note,
            };
            if push_unique_outcome(episode, proposal, outcome.clone()) {
                append_agency_outcome_event(
                    proposal,
                    &outcome,
                    "exact_accept",
                    agency_score,
                    &reason_codes,
                );
                changed = true;
            }
        }

        if !proposal.outcomes.is_empty() {
            proposal.outcome_status = "integrated".to_string();
            proposal.reply_state = "integrated".to_string();
            changed = true;
        }
    }
    changed
}

#[allow(clippy::too_many_lines)]
pub(super) fn score_final_non_adoption_outcomes(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    controller_health: Option<&Value>,
) -> bool {
    let mut changed = false;
    changed |= score_study_first_outcomes(bank, ledger, controller_health);
    changed |= score_adjacent_outcomes(bank, ledger, controller_health);
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
                    "Owner declined the proposal and continued the current course. Agency outcome refusal_clarity: the owner gave a clear no rather than silent non-adoption.",
                    controller_health,
                );
                if push_unique_outcome(episode, proposal, outcome.clone()) {
                    append_agency_outcome_event(
                        proposal,
                        &outcome,
                        "refusal_clarity",
                        0.65,
                        &[
                            "refusal".to_string(),
                            "clear_no".to_string(),
                            "owner_boundary".to_string(),
                        ],
                    );
                    changed = true;
                }
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
                    "Owner declined the proposal before expiry and continued the current course. Agency outcome refusal_clarity: the owner gave a clear no before the system timeout.",
                    controller_health,
                );
                if push_unique_outcome(episode, proposal, outcome.clone()) {
                    append_agency_outcome_event(
                        proposal,
                        &outcome,
                        "refusal_clarity",
                        0.65,
                        &[
                            "refusal".to_string(),
                            "clear_no".to_string(),
                            "owner_boundary".to_string(),
                        ],
                    );
                    changed = true;
                }
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
                    "Proposal expired without nominated response adoption; owner_states={owner_state_summary}. Agency outcome system_expired: neutral system timeout, not an owner preference."
                ),
                controller_health,
            );
            if push_unique_outcome(episode, proposal, outcome.clone()) {
                append_agency_outcome_event(
                    proposal,
                    &outcome,
                    "system_expired",
                    0.0,
                    &[
                        "expiration".to_string(),
                        "system_timeout".to_string(),
                        "neutral_preference".to_string(),
                    ],
                );
                changed = true;
            }
            if !proposal.outcomes.is_empty() {
                proposal.outcome_status = "integrated".to_string();
                changed = true;
            }
        }
    }
    changed
}

pub(super) fn append_adjacent_choice_agency_event(
    proposal: &ActiveSovereigntyProposal,
    owner: &str,
    choice: &str,
) {
    append_signal_event(
        "agency_outcome_recorded",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": owner,
            "choice": choice,
            "agency_outcome": "adjacent_uptake",
            "agency_recovery_score": 0.35,
            "reason_codes": ["adjacent_choice", "owner_not_silent", "weak_positive"],
            "lineage": super::proposal::proposal_lineage(proposal),
            "detail": "Owner chose a noncandidate action while the BTSP proposal was active; this is weak agency-positive uptake, not exact adoption."
        }),
    );
}

pub(super) fn append_counteroffer_agency_event(
    proposal: &ActiveSovereigntyProposal,
    owner: &str,
    counteroffer_id: &str,
    parseable_candidate: bool,
) {
    let mut reason_codes = vec![
        "counteroffer".to_string(),
        "almost_but_offer_differently".to_string(),
        "preference_data".to_string(),
    ];
    if parseable_candidate {
        reason_codes.push("candidate_seed".to_string());
    }
    append_signal_event(
        "agency_outcome_recorded",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": owner,
            "counteroffer_id": counteroffer_id,
            "agency_outcome": "counteroffer",
            "agency_recovery_score": if parseable_candidate { 0.85 } else { 0.7 },
            "reason_codes": reason_codes,
            "lineage": super::proposal::proposal_lineage(proposal),
            "detail": "Owner answered with an explicit counteroffer, preserving agency while improving future proposal fit."
        }),
    );
}

fn score_study_first_outcomes(
    bank: &mut EpisodeBank,
    ledger: &mut ProposalLedger,
    controller_health: Option<&Value>,
) -> bool {
    let mut changed = false;
    for proposal in &mut ledger.proposals {
        if proposal.outcome_status == "integrated" || proposal.study_first_records.is_empty() {
            continue;
        }
        let Some(episode) = bank
            .episodes
            .iter_mut()
            .find(|episode| episode.episode_id == proposal.episode_id)
        else {
            continue;
        };
        let records = proposal.study_first_records.clone();
        for record in records {
            let (target_nearness, distress_or_recovery, opening_vs_reconcentration, details) =
                classify_live_state(controller_health, None);
            let reason_codes = agency_reason_codes(
                RESPONSE_STUDY_FIRST,
                &target_nearness,
                &distress_or_recovery,
                &opening_vs_reconcentration,
            );
            let agency_score = agency_recovery_score(
                0.55,
                &target_nearness,
                &distress_or_recovery,
                &opening_vs_reconcentration,
            );
            let inferred = record
                .inferred_from_choice
                .as_ref()
                .map(|choice| format!(" inferred_from_choice={choice}."))
                .unwrap_or_default();
            let outcome = ResponseOutcomeNote {
                proposal_id: proposal.proposal_id.clone(),
                response_id: RESPONSE_STUDY_FIRST.to_string(),
                owner: record.owner.clone(),
                recorded_at_unix_s: now_unix_s(),
                target_nearness,
                distress_or_recovery,
                opening_vs_reconcentration,
                outcome_telemetry_v2: outcome_telemetry_from_health(controller_health),
                note: format!(
                    "Owner requested study-first before deciding. reason={}. source={}.{} Agency outcome study_first score={agency_score:.2} reason_codes={}. {details}",
                    record.reason,
                    record.source,
                    inferred,
                    reason_codes.join(",")
                ),
            };
            if push_unique_outcome(episode, proposal, outcome.clone()) {
                append_agency_outcome_event(
                    proposal,
                    &outcome,
                    RESPONSE_STUDY_FIRST,
                    agency_score,
                    &reason_codes,
                );
                changed = true;
            }
        }
    }
    changed
}

fn score_adjacent_outcomes(
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
        let Some((owner, choice)) = adjacent_owner_choice(proposal) else {
            continue;
        };
        if proposal
            .study_first_records
            .iter()
            .any(|record| record.owner == owner)
        {
            continue;
        }
        let (target_nearness, distress_or_recovery, opening_vs_reconcentration, details) =
            classify_live_state(controller_health, None);
        let reason_codes = agency_reason_codes(
            "adjacent_uptake",
            &target_nearness,
            &distress_or_recovery,
            &opening_vs_reconcentration,
        );
        let agency_score = agency_recovery_score(
            0.35,
            &target_nearness,
            &distress_or_recovery,
            &opening_vs_reconcentration,
        );
        let outcome = ResponseOutcomeNote {
            proposal_id: proposal.proposal_id.clone(),
            response_id: RESPONSE_ADJACENT_UPTAKE.to_string(),
            owner: owner.clone(),
            recorded_at_unix_s: now_unix_s(),
            target_nearness,
            distress_or_recovery,
            opening_vs_reconcentration,
            outcome_telemetry_v2: outcome_telemetry_from_health(controller_health),
            note: format!(
                "Owner chose adjacent action {choice}. Agency outcome adjacent_uptake score={agency_score:.2} reason_codes={}. {details}",
                reason_codes.join(",")
            ),
        };
        if push_unique_outcome(episode, proposal, outcome.clone()) {
            append_agency_outcome_event(
                proposal,
                &outcome,
                "adjacent_uptake",
                agency_score,
                &reason_codes,
            );
            changed = true;
        }
    }
    changed
}

fn append_agency_outcome_event(
    proposal: &ActiveSovereigntyProposal,
    outcome: &ResponseOutcomeNote,
    agency_outcome: &str,
    agency_recovery_score: f32,
    reason_codes: &[String],
) {
    append_signal_event(
        "agency_outcome_recorded",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": outcome.owner.clone(),
            "response_id": outcome.response_id.clone(),
            "agency_outcome": agency_outcome,
            "agency_recovery_score": agency_recovery_score,
            "reason_codes": reason_codes,
            "lineage": super::proposal::proposal_lineage(proposal),
            "detail": outcome.note.clone()
        }),
    );
}

fn agency_reason_codes(
    base: &str,
    target_nearness: &str,
    distress_or_recovery: &str,
    opening_vs_reconcentration: &str,
) -> Vec<String> {
    let mut codes = vec![base.to_string()];
    match target_nearness {
        "positive" => codes.push("target_nearer".to_string()),
        "negative" => codes.push("target_farther".to_string()),
        _ => {},
    }
    match distress_or_recovery {
        "recovery" => codes.push("recovery".to_string()),
        "worsening" => codes.push("worsening".to_string()),
        _ => {},
    }
    match opening_vs_reconcentration {
        "opening" => codes.push("opening".to_string()),
        "reconcentrating" => codes.push("reconcentrating".to_string()),
        _ => {},
    }
    codes
}

#[allow(clippy::arithmetic_side_effects)]
fn agency_recovery_score(
    base: f32,
    target_nearness: &str,
    distress_or_recovery: &str,
    opening_vs_reconcentration: &str,
) -> f32 {
    let mut score = base;
    if target_nearness == "positive" {
        score += 0.15;
    } else if target_nearness == "negative" {
        score -= 0.15;
    }
    if distress_or_recovery == "recovery" {
        score += 0.15;
    } else if distress_or_recovery == "worsening" {
        score -= 0.2;
    }
    if opening_vs_reconcentration == "opening" {
        score += 0.15;
    } else if opening_vs_reconcentration == "reconcentrating" {
        score -= 0.2;
    }
    score.clamp(0.0, 1.35)
}

fn adjacent_owner_choice(proposal: &ActiveSovereigntyProposal) -> Option<(String, String)> {
    if let Some(choice) = related_owner_choice(proposal) {
        return Some(choice);
    }
    proposal
        .choice_interpretations
        .iter()
        .rev()
        .find(|interpretation| interpretation.relation_to_proposal != "exact_nominated")
        .map(|interpretation| {
            (
                interpretation.owner.clone(),
                interpretation.normalized_choice.clone(),
            )
        })
}

fn related_owner_choice(proposal: &ActiveSovereigntyProposal) -> Option<(String, String)> {
    let related = proposal.related_choice.as_ref()?;
    let (owner, choice) = related.split_once(':')?;
    Some((owner.to_string(), choice.to_string()))
}
