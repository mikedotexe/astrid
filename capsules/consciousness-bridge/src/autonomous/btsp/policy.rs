use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::now_unix_s;
use super::{ActiveSovereigntyProposal, BTSPEpisodeRecord, ProposalLedger, ResponseOutcomeNote};

const COOLDOWN_INTEGRATED_SECS: u64 = 5 * 60;
const COOLDOWN_EXPIRED_SECS: u64 = 5 * 60;
const COOLDOWN_DECLINED_SECS: u64 = 3 * 60;
const COOLDOWN_MISREAD_DECLINED_SECS: u64 = 10 * 60;
const COOLDOWN_ESCALATED_SECS: u64 = 15 * 60;
const COOLDOWN_ESCALATION_WINDOW_SECS: u64 = 30 * 60;
const LEARNED_POLICY_WINDOW: usize = 24;
const LEARNED_POLICY_SHARED_WINDOW: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct CooldownState {
    pub active: bool,
    #[serde(default)]
    pub until_unix_s: u64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct LearnedPolicyEntry {
    pub owner: String,
    pub response_id: String,
    pub observations: u32,
    pub stance: String,
    pub summary: String,
}

pub(super) fn build_signal_fingerprint(
    matched_signal_families: &[String],
    controller_health: Option<&Value>,
) -> String {
    let families = sorted_unique(matched_signal_families);
    let transition = normalize_component(live_transition_descriptor(controller_health));
    let crossing = normalize_component(fill_band_crossing(controller_health));
    let perturb = normalize_component(perturb_verdict(controller_health));
    let fill_band = normalize_component(current_fill_band(controller_health));
    format!(
        "families={};transition={transition};crossing={crossing};perturb={perturb};fill_band={fill_band}",
        families.join("+")
    )
}

pub(super) fn hydrate_signal_fingerprints(ledger: &mut ProposalLedger) -> bool {
    let mut changed = false;
    for proposal in &mut ledger.proposals {
        if proposal.signal_fingerprint.is_empty() {
            let fingerprint = proposal_signal_fingerprint(proposal);
            if !fingerprint.is_empty() {
                proposal.signal_fingerprint = fingerprint;
                changed = true;
            }
        }
    }
    changed
}

pub(super) fn cooldown_state_for(
    ledger: &ProposalLedger,
    episode_id: &str,
    fingerprint: &str,
) -> CooldownState {
    if fingerprint.is_empty() {
        return CooldownState::default();
    }
    let now = now_unix_s();
    let mut latest_same = ledger
        .proposals
        .iter()
        .filter(|proposal| proposal.episode_id == episode_id)
        .filter(|proposal| proposal_signal_fingerprint(proposal) == fingerprint)
        .filter(|proposal| is_resolved_state(&proposal.reply_state))
        .collect::<Vec<_>>();
    latest_same.sort_by_key(|proposal| proposal_resolved_at(proposal));
    let Some(latest) = latest_same.last().copied() else {
        return CooldownState {
            active: false,
            until_unix_s: 0,
            reason: String::new(),
            fingerprint: fingerprint.to_string(),
        };
    };

    let escalated = should_escalate_cooldown(ledger, episode_id, fingerprint, now);
    let misread_decline = latest
        .refusals
        .iter()
        .rev()
        .any(|refusal| refusal.reason == "misread");
    let base_duration = match latest.reply_state.as_str() {
        "declined" if misread_decline => COOLDOWN_MISREAD_DECLINED_SECS,
        "declined" => COOLDOWN_DECLINED_SECS,
        "expired" => COOLDOWN_EXPIRED_SECS,
        _ => COOLDOWN_INTEGRATED_SECS,
    };
    let duration = if escalated {
        COOLDOWN_ESCALATED_SECS
    } else {
        base_duration
    };
    let until_unix_s = proposal_resolved_at(latest).saturating_add(duration);

    CooldownState {
        active: now < until_unix_s,
        until_unix_s,
        reason: if escalated {
            "repeated_reconcentrating_same_fingerprint".to_string()
        } else {
            match latest.reply_state.as_str() {
                "declined" if misread_decline => "recent_misread_same_fingerprint".to_string(),
                "declined" => "recent_declined_same_fingerprint".to_string(),
                "expired" => "recent_expired_same_fingerprint".to_string(),
                _ => "recent_integrated_same_fingerprint".to_string(),
            }
        },
        fingerprint: fingerprint.to_string(),
    }
}

pub(super) fn refresh_learned_policy(episode: &mut BTSPEpisodeRecord) -> bool {
    let next = derive_learned_policy(&episode.response_outcomes);
    if episode.learned_policy == next {
        return false;
    }
    episode.learned_policy = next;
    true
}

pub(super) fn owner_policy_entries(
    entries: &[LearnedPolicyEntry],
    owner: &str,
) -> Vec<LearnedPolicyEntry> {
    let mut filtered = entries
        .iter()
        .filter(|entry| entry.owner == owner)
        .cloned()
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| {
        right
            .observations
            .cmp(&left.observations)
            .then_with(|| left.response_id.cmp(&right.response_id))
    });
    filtered.truncate(2);
    filtered
}

pub(super) fn shared_learned_read_line(outcomes: &[ResponseOutcomeNote]) -> Option<String> {
    let trailing = trailing_outcomes(outcomes, LEARNED_POLICY_SHARED_WINDOW);
    if trailing.is_empty() {
        return None;
    }
    let reconcentrating = trailing
        .iter()
        .filter(|outcome| outcome.opening_vs_reconcentration == "reconcentrating")
        .count();
    if !ratio_at_least(reconcentrating, trailing.len(), 6, 10) {
        return None;
    }
    Some(
        "Recent learned read: responses have often improved fill or legibility without producing real widening; treat opening claims cautiously."
            .to_string(),
    )
}

pub(super) fn candidate_policy_suffix(
    entries: &[LearnedPolicyEntry],
    owner: &str,
    response_id: &str,
) -> Option<&'static str> {
    let entry = entries
        .iter()
        .find(|entry| entry.owner == owner && entry.response_id == response_id)?;
    Some(match entry.stance.as_str() {
        "cautionary" => "[recent read: often reconcentrates]",
        "supportive" => "[recent read: often helps recovery]",
        _ => "[recent read: mixed]",
    })
}

pub(super) fn learned_policy_label(response_id: &str) -> &'static str {
    match response_id {
        "minime_notice_first" => "NOTICE",
        "minime_recover_regime" => "recover",
        "minime_semantic_probe" => "semantic probe",
        "astrid_dampen" => "DAMPEN",
        "astrid_breathe_alone" => "BREATHE_ALONE",
        "astrid_echo_off" => "ECHO_OFF",
        _ => "response",
    }
}

pub(super) fn proposal_signal_fingerprint(proposal: &ActiveSovereigntyProposal) -> String {
    if !proposal.signal_fingerprint.is_empty() {
        return proposal.signal_fingerprint.clone();
    }
    let transition = normalize_component(extract_transition_signal_value(
        &proposal.matched_live_signals,
    ));
    let crossing = normalize_component(extract_live_signal_value(
        &proposal.matched_live_signals,
        "fill_band_crossing:",
    ));
    let perturb = normalize_component(extract_live_signal_value(
        &proposal.matched_live_signals,
        "perturb_visibility:",
    ));
    let fill_band = if crossing != "none" {
        crossing.clone()
    } else {
        "unknown".to_string()
    };
    format!(
        "families={};transition={transition};crossing={crossing};perturb={perturb};fill_band={fill_band}",
        sorted_unique(&proposal.matched_signal_families).join("+")
    )
}

fn derive_learned_policy(outcomes: &[ResponseOutcomeNote]) -> Vec<LearnedPolicyEntry> {
    let trailing = trailing_outcomes(outcomes, LEARNED_POLICY_WINDOW);
    let mut grouped = BTreeMap::<(String, String), Vec<ResponseOutcomeNote>>::new();
    for outcome in trailing {
        grouped
            .entry((outcome.owner.clone(), outcome.response_id.clone()))
            .or_default()
            .push(outcome);
    }

    grouped
        .into_iter()
        .filter_map(|((owner, response_id), grouped_outcomes)| {
            if grouped_outcomes.len() < 3 {
                return None;
            }
            let observations = u32::try_from(grouped_outcomes.len()).unwrap_or(u32::MAX);
            let reconcentrating = grouped_outcomes
                .iter()
                .filter(|outcome| outcome.opening_vs_reconcentration == "reconcentrating")
                .count();
            let positive = grouped_outcomes
                .iter()
                .filter(|outcome| {
                    outcome.distress_or_recovery == "recovery"
                        || outcome.target_nearness == "positive"
                })
                .count();

            let (stance, summary) =
                if ratio_at_least(reconcentrating, grouped_outcomes.len(), 6, 10) {
                    (
                        "cautionary",
                        "Recent read: often reconcentrates.".to_string(),
                    )
                } else if ratio_at_least(positive, grouped_outcomes.len(), 6, 10)
                    && !ratio_at_least(reconcentrating, grouped_outcomes.len(), 4, 10)
                {
                    (
                        "supportive",
                        "Recent read: often helps recovery or target nearness.".to_string(),
                    )
                } else {
                    ("mixed", "Recent read: mixed results so far.".to_string())
                };

            Some(LearnedPolicyEntry {
                owner,
                response_id,
                observations,
                stance: stance.to_string(),
                summary,
            })
        })
        .collect()
}

fn trailing_outcomes(outcomes: &[ResponseOutcomeNote], limit: usize) -> Vec<ResponseOutcomeNote> {
    let mut trailing = outcomes.to_vec();
    trailing.sort_by_key(|outcome| outcome.recorded_at_unix_s);
    if trailing.len() <= limit {
        return trailing;
    }
    trailing.split_off(trailing.len().saturating_sub(limit))
}

fn should_escalate_cooldown(
    ledger: &ProposalLedger,
    episode_id: &str,
    fingerprint: &str,
    now: u64,
) -> bool {
    let recent = ledger
        .proposals
        .iter()
        .filter(|proposal| proposal.episode_id == episode_id)
        .filter(|proposal| proposal_signal_fingerprint(proposal) == fingerprint)
        .filter(|proposal| is_resolved_state(&proposal.reply_state))
        .filter(|proposal| {
            now.saturating_sub(proposal_resolved_at(proposal)) <= COOLDOWN_ESCALATION_WINDOW_SECS
        })
        .collect::<Vec<_>>();
    if recent.len() < 3 {
        return false;
    }

    let mut reconcentrating = 0usize;
    let mut total = 0usize;
    for proposal in recent {
        for outcome in &proposal.outcomes {
            total = total.saturating_add(1);
            if outcome.opening_vs_reconcentration == "reconcentrating" {
                reconcentrating = reconcentrating.saturating_add(1);
            }
        }
    }
    total != 0 && ratio_at_least(reconcentrating, total, 1, 2)
}

fn proposal_resolved_at(proposal: &ActiveSovereigntyProposal) -> u64 {
    proposal
        .outcomes
        .iter()
        .map(|outcome| outcome.recorded_at_unix_s)
        .max()
        .unwrap_or(0)
        .max(proposal.expires_at_unix_s)
        .max(proposal.latest_match_at_unix_s)
        .max(proposal.created_at_unix_s)
}

fn is_resolved_state(state: &str) -> bool {
    matches!(state, "declined" | "expired" | "integrated")
}

fn ratio_at_least(
    numerator_count: usize,
    denominator_count: usize,
    numerator: usize,
    denominator: usize,
) -> bool {
    denominator_count != 0
        && numerator_count.saturating_mul(denominator)
            >= denominator_count.saturating_mul(numerator)
}

fn live_transition_descriptor(controller_health: Option<&Value>) -> Option<&str> {
    let health = controller_health?;
    if let Some(typed) = health.get("transition_event_v1") {
        return typed
            .get("kind")
            .and_then(Value::as_str)
            .or_else(|| typed.get("description").and_then(Value::as_str));
    }
    health
        .get("transition_event")
        .and_then(|transition| transition.get("description"))
        .and_then(Value::as_str)
}

fn fill_band_crossing(controller_health: Option<&Value>) -> Option<&str> {
    let Some(health) = controller_health else {
        return None;
    };
    if health
        .get("transition_event_v1")
        .and_then(|transition| transition.get("crossed_fill_band"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return health
            .get("transition_event_v1")
            .and_then(|transition| transition.get("fill_band"))
            .and_then(Value::as_str);
    }
    if health
        .get("transition_event")
        .and_then(|transition| transition.get("crossed_fill_band"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return health
            .get("transition_event")
            .and_then(|transition| transition.get("fill_band"))
            .and_then(Value::as_str);
    }
    if health
        .get("crossed_fill_band")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return health.get("fill_band").and_then(Value::as_str);
    }
    None
}

fn perturb_verdict(controller_health: Option<&Value>) -> Option<&str> {
    controller_health
        .and_then(|health| health.get("perturb_visibility"))
        .and_then(|value| value.get("shape_verdict"))
        .and_then(Value::as_str)
}

fn current_fill_band(controller_health: Option<&Value>) -> Option<&str> {
    controller_health
        .and_then(|health| health.get("fill_band"))
        .and_then(Value::as_str)
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_component(value: Option<&str>) -> String {
    value
        .unwrap_or("none")
        .to_ascii_lowercase()
        .replace(" -> ", "->")
        .replace(' ', "_")
}

fn extract_live_signal_value<'a>(live_signals: &'a [String], prefix: &str) -> Option<&'a str> {
    live_signals
        .iter()
        .find_map(|signal| signal.strip_prefix(prefix))
}

fn extract_transition_signal_value(live_signals: &[String]) -> Option<&str> {
    ["basin_transition:", "phase_transition:", "breathing_phase:"]
        .iter()
        .find_map(|prefix| extract_live_signal_value(live_signals, prefix))
}
