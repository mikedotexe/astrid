use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::helpers::{now_unix_s, recompute_reply_state};
use super::shadow::derive_shadow_behavioral_preferences;
use super::signal::append_signal_event;
use super::{
    ActiveSovereigntyProposal, BTSPEpisodeRecord, OWNER_ASTRID, OWNER_MINIME, ProposalLedger,
};

const BEHAVIORAL_WINDOW: usize = 24;

const KIND_BEHAVIORAL: &str = "behavioral";
const KIND_DECLARED: &str = "declared";
const KIND_NEGOTIATED: &str = "negotiated";

const REASON_NOT_NOW: &str = "not_now";
const REASON_MISREAD: &str = "misread";
const REASON_TOO_FORCEFUL: &str = "too_forceful";
const REASON_STUDY_FIRST: &str = "study_first";
const REASON_STAY_WITH_ME: &str = "stay_with_me";
const REASON_GIVE_ME_SPACE: &str = "give_me_space";

const STANCE_SOFTER_CONTACT: &str = "softer_contact";
const STANCE_SLOWER_PACING: &str = "slower_pacing";
const STANCE_MORE_SPACE: &str = "more_space";
const STANCE_STAY_WITH_ME: &str = "stay_with_me";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct PreferenceMemoryEntry {
    pub owner: String,
    pub preference_key: String,
    pub kind: String,
    pub evidence_count: u32,
    pub last_observed_unix_s: u64,
    pub summary: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct PreferenceSummary {
    pub owner: String,
    pub preference_key: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct RefusalRecord {
    pub owner: String,
    pub reason: String,
    pub note: String,
    pub recorded_at_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct CounterofferRecord {
    pub counteroffer_id: String,
    pub owner: String,
    pub target_owner: String,
    pub kind: String,
    #[serde(default)]
    pub requested_response_id: Option<String>,
    #[serde(default)]
    pub requested_stance: Option<String>,
    pub state: String,
    pub note: String,
    pub recorded_at_unix_s: u64,
    #[serde(default)]
    pub resolved_at_unix_s: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct ActiveNegotiationView {
    #[serde(default)]
    pub items: Vec<ActiveNegotiationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct ActiveNegotiationItem {
    pub owner: String,
    pub target_owner: String,
    pub kind: String,
    #[serde(default)]
    pub requested_response_id: Option<String>,
    #[serde(default)]
    pub requested_stance: Option<String>,
    pub summary: String,
    pub response_hint: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::autonomous) enum StructuredSignal {
    Refusal { reason: String },
    Counter { payload: String },
    Accept,
    Decline,
}

pub(super) fn normalize_refusal_reason(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        REASON_NOT_NOW | REASON_MISREAD | REASON_TOO_FORCEFUL | REASON_STUDY_FIRST
        | REASON_STAY_WITH_ME | REASON_GIVE_ME_SPACE => Some(normalized),
        _ => None,
    }
}

pub(super) fn normalize_relation_stance(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        STANCE_SOFTER_CONTACT | STANCE_SLOWER_PACING | STANCE_MORE_SPACE | STANCE_STAY_WITH_ME => {
            Some(normalized)
        },
        _ => None,
    }
}

pub(super) fn parse_structured_signal(raw: &str) -> Option<StructuredSignal> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if upper == "BTSP_ACCEPT" {
            return Some(StructuredSignal::Accept);
        }
        if upper == "BTSP_DECLINE" {
            return Some(StructuredSignal::Decline);
        }
        if upper.starts_with("BTSP_REFUSAL ") {
            let reason = trimmed["BTSP_REFUSAL ".len()..].trim();
            let normalized = normalize_refusal_reason(reason)?;
            return Some(StructuredSignal::Refusal { reason: normalized });
        }
        if upper.starts_with("BTSP_COUNTER ") {
            let payload = trimmed["BTSP_COUNTER ".len()..].trim();
            if payload.is_empty() {
                return None;
            }
            return Some(StructuredSignal::Counter {
                payload: payload.to_string(),
            });
        }
    }
    None
}

pub(super) fn other_owner(owner: &str) -> &'static str {
    if owner == OWNER_MINIME {
        OWNER_ASTRID
    } else {
        OWNER_MINIME
    }
}

pub(super) fn build_counteroffer(
    owner: &str,
    payload: &str,
    episode: &BTSPEpisodeRecord,
    proposal: &ActiveSovereigntyProposal,
) -> Option<CounterofferRecord> {
    let target_owner = other_owner(owner).to_string();
    let normalized_payload = payload.trim();
    if let Some(stance) = normalize_relation_stance(normalized_payload) {
        return Some(CounterofferRecord {
            counteroffer_id: format!(
                "{}_counter_{}_{}",
                proposal.proposal_id,
                owner,
                now_unix_s()
            ),
            owner: owner.to_string(),
            target_owner,
            kind: "relation_stance".to_string(),
            requested_response_id: None,
            requested_stance: Some(stance.clone()),
            state: "open".to_string(),
            note: format!(
                "{} is asking for {}.",
                owner_label(owner),
                stance_label(&stance)
            ),
            recorded_at_unix_s: now_unix_s(),
            resolved_at_unix_s: None,
        });
    }

    let response_id = normalized_payload.to_ascii_lowercase();
    let response = episode
        .nominated_responses
        .iter()
        .find(|response| response.response_id == response_id)?;
    if response.owner == owner {
        return None;
    }
    Some(CounterofferRecord {
        counteroffer_id: format!(
            "{}_counter_{}_{}",
            proposal.proposal_id,
            owner,
            now_unix_s()
        ),
        owner: owner.to_string(),
        target_owner,
        kind: "response_swap".to_string(),
        requested_response_id: Some(response_id.clone()),
        requested_stance: None,
        state: "open".to_string(),
        note: format!(
            "{} is asking for {}.",
            owner_label(owner),
            requested_response_label(&response_id)
        ),
        recorded_at_unix_s: now_unix_s(),
        resolved_at_unix_s: None,
    })
}

pub(super) fn open_counteroffer(
    proposal: &mut ActiveSovereigntyProposal,
    counteroffer: CounterofferRecord,
) -> bool {
    for existing in &mut proposal.counteroffers {
        if existing.owner == counteroffer.owner && existing.state == "open" {
            existing.state = "expired".to_string();
            existing.resolved_at_unix_s = Some(now_unix_s());
        }
    }
    proposal.last_negotiation_event_at_unix_s = now_unix_s();
    proposal.counteroffers.push(counteroffer);
    true
}

pub(super) fn resolve_incoming_counteroffer(
    proposal: &mut ActiveSovereigntyProposal,
    target_owner: &str,
    accepted: bool,
) -> Option<CounterofferRecord> {
    let now = now_unix_s();
    let counteroffer = proposal
        .counteroffers
        .iter_mut()
        .rev()
        .find(|counteroffer| {
            counteroffer.target_owner == target_owner && counteroffer.state == "open"
        })?;
    counteroffer.state = if accepted {
        "accepted".to_string()
    } else {
        "declined".to_string()
    };
    counteroffer.resolved_at_unix_s = Some(now);
    proposal.last_negotiation_event_at_unix_s = now;
    Some(counteroffer.clone())
}

pub(super) fn resolve_counteroffers_for_exact_adoption(
    proposal: &mut ActiveSovereigntyProposal,
    target_owner: &str,
    response_id: &str,
) -> Vec<CounterofferRecord> {
    let now = now_unix_s();
    let mut resolved = Vec::new();
    for counteroffer in &mut proposal.counteroffers {
        if counteroffer.target_owner != target_owner
            || counteroffer.kind != "response_swap"
            || counteroffer.state != "open"
        {
            continue;
        }
        if counteroffer.requested_response_id.as_deref() != Some(response_id) {
            continue;
        }
        counteroffer.state = "accepted".to_string();
        counteroffer.resolved_at_unix_s = Some(now);
        resolved.push(counteroffer.clone());
    }
    if !resolved.is_empty() {
        proposal.last_negotiation_event_at_unix_s = now;
    }
    resolved
}

pub(super) fn apply_structured_signal(
    episode: &mut BTSPEpisodeRecord,
    proposal: &mut ActiveSovereigntyProposal,
    owner: &str,
    signal: StructuredSignal,
) -> bool {
    match signal {
        StructuredSignal::Refusal { reason } => record_refusal(
            episode,
            proposal,
            owner,
            &reason,
            &format!("BTSP_REFUSAL {reason}"),
        ),
        StructuredSignal::Counter { payload } => {
            let Some(counteroffer) = build_counteroffer(owner, &payload, episode, proposal) else {
                return false;
            };
            let opened = open_counteroffer(proposal, counteroffer.clone());
            proposal
                .owner_reply_state
                .insert(owner.to_string(), "answered".to_string());
            proposal.reply_state = recompute_reply_state(proposal);
            if let Some(preference_key) = preference_for_counteroffer(&counteroffer) {
                let source_ref = format!("{}:counteroffer", proposal.proposal_id);
                let _ = merge_preference_memory(
                    &mut episode.preference_memory,
                    owner,
                    preference_key,
                    KIND_DECLARED,
                    &source_ref,
                );
                append_signal_event(
                    "preference_observed",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": owner,
                        "preference_key": preference_key,
                        "kind": KIND_DECLARED,
                        "detail": "Structured counteroffer exposed a preference signal."
                    }),
                );
            }
            append_signal_event(
                "counteroffer_opened",
                json!({
                    "episode_id": proposal.episode_id.clone(),
                    "proposal_id": proposal.proposal_id.clone(),
                    "owner": counteroffer.owner.clone(),
                    "target_owner": counteroffer.target_owner.clone(),
                    "requested_response_id": counteroffer.requested_response_id.clone(),
                    "requested_stance": counteroffer.requested_stance.clone(),
                    "kind": counteroffer.kind.clone(),
                    "detail": counteroffer.note.clone()
                }),
            );
            opened
        },
        StructuredSignal::Accept => {
            let Some(counteroffer) = resolve_incoming_counteroffer(proposal, owner, true) else {
                return false;
            };
            proposal
                .owner_reply_state
                .insert(owner.to_string(), "answered".to_string());
            proposal.reply_state = recompute_reply_state(proposal);
            if let Some(preference_key) = preference_for_counteroffer(&counteroffer) {
                let source_ref = format!("{}:negotiated_accept", proposal.proposal_id);
                let _ = merge_preference_memory(
                    &mut episode.preference_memory,
                    &counteroffer.owner,
                    preference_key,
                    KIND_NEGOTIATED,
                    &source_ref,
                );
                append_signal_event(
                    "preference_observed",
                    json!({
                        "episode_id": proposal.episode_id.clone(),
                        "proposal_id": proposal.proposal_id.clone(),
                        "owner": counteroffer.owner.clone(),
                        "preference_key": preference_key,
                        "kind": KIND_NEGOTIATED,
                        "detail": "Accepted counteroffer reinforced a negotiated preference."
                    }),
                );
            }
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
                    "detail": "Incoming counteroffer was accepted explicitly."
                }),
            );
            true
        },
        StructuredSignal::Decline => {
            let Some(counteroffer) = resolve_incoming_counteroffer(proposal, owner, false) else {
                return false;
            };
            proposal
                .owner_reply_state
                .insert(owner.to_string(), "answered".to_string());
            proposal.reply_state = recompute_reply_state(proposal);
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
                    "detail": "Incoming counteroffer was declined explicitly."
                }),
            );
            true
        },
    }
}

pub(super) fn record_refusal(
    episode: &mut BTSPEpisodeRecord,
    proposal: &mut ActiveSovereigntyProposal,
    owner: &str,
    reason: &str,
    raw_choice: &str,
) -> bool {
    let normalized_reason =
        normalize_refusal_reason(reason).unwrap_or_else(|| REASON_NOT_NOW.to_string());
    proposal
        .owner_reply_state
        .insert(owner.to_string(), "declined".to_string());
    proposal.reply_state = recompute_reply_state(proposal);
    proposal.outcome_status = proposal.reply_state.clone();
    let refusal = RefusalRecord {
        owner: owner.to_string(),
        reason: normalized_reason.clone(),
        note: format!("{owner} declined with reason `{normalized_reason}`."),
        recorded_at_unix_s: now_unix_s(),
    };
    proposal.refusals.push(refusal.clone());
    proposal.last_negotiation_event_at_unix_s = refusal.recorded_at_unix_s;
    append_signal_event(
        "refusal_recorded",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": owner,
            "reason": normalized_reason.clone(),
            "choice": raw_choice,
            "signal_families": proposal.matched_signal_families.clone(),
            "signal_roles": proposal.matched_signal_roles.clone(),
            "detail": refusal.note.clone()
        }),
    );
    append_signal_event(
        "declined",
        json!({
            "episode_id": proposal.episode_id.clone(),
            "proposal_id": proposal.proposal_id.clone(),
            "owner": owner,
            "choice": raw_choice,
            "signal_families": proposal.matched_signal_families.clone(),
            "signal_roles": proposal.matched_signal_roles.clone(),
            "detail": "Owner declined the bounded proposal and continued the current course."
        }),
    );
    if let Some(preference_key) = preference_for_refusal(&normalized_reason) {
        let source_ref = format!("{}:refusal:{}", proposal.proposal_id, normalized_reason);
        let _ = merge_preference_memory(
            &mut episode.preference_memory,
            owner,
            preference_key,
            KIND_DECLARED,
            &source_ref,
        );
        append_signal_event(
            "preference_observed",
            json!({
                "episode_id": proposal.episode_id.clone(),
                "proposal_id": proposal.proposal_id.clone(),
                "owner": owner,
                "preference_key": preference_key,
                "kind": KIND_DECLARED,
                "detail": "Structured refusal exposed a preference signal."
            }),
        );
    }
    true
}

pub(super) fn refresh_preference_memory(
    episode: &mut BTSPEpisodeRecord,
    ledger: &ProposalLedger,
) -> bool {
    let mut next = episode
        .preference_memory
        .iter()
        .filter(|entry| entry.kind != KIND_BEHAVIORAL)
        .cloned()
        .collect::<Vec<_>>();

    let derived = derive_behavioral_preferences(ledger, &episode.episode_id);
    next.extend(derive_shadow_behavioral_preferences(
        ledger,
        &episode.episode_id,
    ));
    let before = episode.preference_memory.clone();
    next.extend(derived);
    next.sort_by(|left, right| {
        left.owner
            .cmp(&right.owner)
            .then_with(|| right.evidence_count.cmp(&left.evidence_count))
            .then_with(|| left.preference_key.cmp(&right.preference_key))
    });
    if before == next {
        return false;
    }
    episode.preference_memory = next;
    true
}

pub(super) fn shared_preference_summaries(
    entries: &[PreferenceMemoryEntry],
    astrid_priority_preference: Option<&str>,
    prioritize_astrid_translation: bool,
) -> Vec<PreferenceSummary> {
    let mut by_owner = BTreeMap::<String, Vec<PreferenceMemoryEntry>>::new();
    for entry in entries {
        by_owner
            .entry(entry.owner.clone())
            .or_default()
            .push(entry.clone());
    }
    let mut summaries = Vec::new();
    for (owner, mut owner_entries) in by_owner {
        owner_entries.sort_by(|left, right| {
            right
                .evidence_count
                .cmp(&left.evidence_count)
                .then_with(|| right.last_observed_unix_s.cmp(&left.last_observed_unix_s))
                .then_with(|| left.preference_key.cmp(&right.preference_key))
        });
        if prioritize_astrid_translation
            && owner == OWNER_ASTRID
            && let Some(priority_preference) = astrid_priority_preference
            && let Some(index) = owner_entries
                .iter()
                .position(|entry| entry.preference_key == priority_preference)
        {
            let prioritized = owner_entries.remove(index);
            owner_entries.insert(0, prioritized);
        }
        owner_entries.truncate(2);
        summaries.extend(owner_entries.into_iter().map(|entry| PreferenceSummary {
            owner: entry.owner,
            preference_key: entry.preference_key,
            summary: entry.summary,
        }));
    }
    summaries
}

pub(super) fn owner_preference_summaries(
    entries: &[PreferenceSummary],
    owner: &str,
) -> Vec<PreferenceSummary> {
    entries
        .iter()
        .filter(|entry| entry.owner == owner)
        .take(2)
        .cloned()
        .collect()
}

pub(super) fn active_negotiation_view(
    proposal: Option<&ActiveSovereigntyProposal>,
) -> Option<ActiveNegotiationView> {
    let proposal = proposal?;
    let items = proposal
        .counteroffers
        .iter()
        .filter(|counteroffer| counteroffer.state == "open")
        .map(|counteroffer| ActiveNegotiationItem {
            owner: counteroffer.owner.clone(),
            target_owner: counteroffer.target_owner.clone(),
            kind: counteroffer.kind.clone(),
            requested_response_id: counteroffer.requested_response_id.clone(),
            requested_stance: counteroffer.requested_stance.clone(),
            summary: render_counteroffer_summary(counteroffer),
            response_hint: "Reply with BTSP_ACCEPT or BTSP_DECLINE.".to_string(),
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return None;
    }
    Some(ActiveNegotiationView { items })
}

pub(super) fn merge_preference_memory(
    entries: &mut Vec<PreferenceMemoryEntry>,
    owner: &str,
    preference_key: &str,
    kind: &str,
    source_ref: &str,
) -> bool {
    let Some(summary) = preference_summary(preference_key) else {
        return false;
    };
    let now = now_unix_s();
    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| entry.owner == owner && entry.preference_key == preference_key)
    {
        let mut changed = false;
        if !existing
            .source_refs
            .iter()
            .any(|existing_ref| existing_ref == source_ref)
        {
            existing.source_refs.push(source_ref.to_string());
            existing.evidence_count = existing.evidence_count.saturating_add(1);
            changed = true;
        }
        let prioritized = prefer_kind(&existing.kind, kind);
        if existing.kind != prioritized {
            existing.kind = prioritized.to_string();
            changed = true;
        }
        if existing.summary != summary {
            existing.summary = summary.to_string();
            changed = true;
        }
        if existing.last_observed_unix_s != now {
            existing.last_observed_unix_s = now;
            changed = true;
        }
        return changed;
    }
    entries.push(PreferenceMemoryEntry {
        owner: owner.to_string(),
        preference_key: preference_key.to_string(),
        kind: kind.to_string(),
        evidence_count: 1,
        last_observed_unix_s: now,
        summary: summary.to_string(),
        source_refs: vec![source_ref.to_string()],
    });
    true
}

pub(super) fn preference_for_refusal(reason: &str) -> Option<&'static str> {
    match reason {
        REASON_MISREAD => Some("prefers_current_course_when_misread"),
        REASON_TOO_FORCEFUL => Some("prefers_softer_contact"),
        REASON_STUDY_FIRST => Some("prefers_inquiry_before_intervention"),
        REASON_STAY_WITH_ME => Some("prefers_stay_with_me"),
        REASON_GIVE_ME_SPACE => Some("prefers_more_space"),
        _ => None,
    }
}

pub(super) fn preference_for_counteroffer(
    counteroffer: &CounterofferRecord,
) -> Option<&'static str> {
    if let Some(stance) = counteroffer.requested_stance.as_deref() {
        return match stance {
            STANCE_SOFTER_CONTACT => Some("prefers_softer_contact"),
            STANCE_MORE_SPACE => Some("prefers_more_space"),
            STANCE_STAY_WITH_ME => Some("prefers_stay_with_me"),
            _ => None,
        };
    }
    match counteroffer.requested_response_id.as_deref() {
        Some("astrid_dampen") => Some("prefers_softer_contact"),
        Some("astrid_breathe_alone") | Some("astrid_echo_off") => Some("prefers_more_space"),
        Some("minime_notice_first") => Some("prefers_witnessing_first"),
        _ => None,
    }
}

fn derive_behavioral_preferences(
    ledger: &ProposalLedger,
    episode_id: &str,
) -> Vec<PreferenceMemoryEntry> {
    let mut proposals = ledger
        .proposals
        .iter()
        .filter(|proposal| proposal.episode_id == episode_id)
        .filter(|proposal| is_resolved_proposal(proposal))
        .cloned()
        .collect::<Vec<_>>();
    proposals.sort_by_key(proposal_order_key);
    let trailing = proposals
        .into_iter()
        .rev()
        .take(BEHAVIORAL_WINDOW)
        .collect::<Vec<_>>();

    let mut counts = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for proposal in trailing {
        let proposal_ref = proposal.proposal_id.clone();
        for interpretation in &proposal.choice_interpretations {
            if interpretation.category == "epistemic"
                && interpretation.relation_to_proposal != "exact_nominated"
            {
                counts
                    .entry((
                        interpretation.owner.clone(),
                        "prefers_inquiry_before_intervention".to_string(),
                    ))
                    .or_default()
                    .insert(proposal_ref.clone());
            }
            if interpretation.normalized_choice == "NOTICE"
                || interpretation.relation_to_proposal == "same_family_adjacent"
                    && interpretation.normalized_choice == "NOTICE"
            {
                counts
                    .entry((
                        interpretation.owner.clone(),
                        "prefers_witnessing_first".to_string(),
                    ))
                    .or_default()
                    .insert(proposal_ref.clone());
            }
        }

        let exact_adoptions = if proposal.exact_adoptions.is_empty() {
            proposal
                .selected_response_ids_by_owner
                .iter()
                .map(|(owner, response_id)| (owner.clone(), response_id.clone()))
                .collect::<Vec<_>>()
        } else {
            proposal
                .exact_adoptions
                .iter()
                .map(|adoption| (adoption.owner.clone(), adoption.response_id.clone()))
                .collect::<Vec<_>>()
        };
        for (owner, response_id) in exact_adoptions {
            if let Some(preference_key) = preference_from_response_id(&response_id) {
                counts
                    .entry((owner, preference_key.to_string()))
                    .or_default()
                    .insert(proposal_ref.clone());
            }
        }
    }

    counts
        .into_iter()
        .filter_map(|((owner, preference_key), refs)| {
            if refs.len() < 3 {
                return None;
            }
            let summary = preference_summary(&preference_key)?;
            Some(PreferenceMemoryEntry {
                owner,
                preference_key,
                kind: KIND_BEHAVIORAL.to_string(),
                evidence_count: u32::try_from(refs.len()).unwrap_or(u32::MAX),
                last_observed_unix_s: now_unix_s(),
                summary: summary.to_string(),
                source_refs: refs.into_iter().collect(),
            })
        })
        .collect()
}

fn preference_from_response_id(response_id: &str) -> Option<&'static str> {
    match response_id {
        "minime_notice_first" => Some("prefers_witnessing_first"),
        "astrid_dampen" => Some("prefers_softer_contact"),
        "astrid_breathe_alone" | "astrid_echo_off" => Some("prefers_more_space"),
        _ => None,
    }
}

pub(super) fn preference_summary(preference_key: &str) -> Option<&'static str> {
    match preference_key {
        "prefers_inquiry_before_intervention" => {
            Some("Recent read: often prefers inquiry before intervention when the loop returns.")
        },
        "prefers_inquiry_before_decompression" => {
            Some("Recent read: often softens through inquiry before direct decompression.")
        },
        "prefers_witnessing_first" => {
            Some("Recent read: often prefers witnessing first before stronger intervention.")
        },
        "prefers_expressive_holding_before_decompression" => {
            Some("Recent read: often holds expressively before direct decompression.")
        },
        "prefers_gentle_shaping_before_decompression" => {
            Some("Recent read: often softens through gentler shaping before direct decompression.")
        },
        "prefers_softer_contact" => {
            Some("Recent read: often wants softer contact when the signal tightens.")
        },
        "prefers_more_space" => {
            Some("Recent read: often wants more space or looser coupling when the loop returns.")
        },
        "prefers_stay_with_me" => Some(
            "Recent read: often wants relation-held contact rather than immediate decompression.",
        ),
        "prefers_current_course_when_misread" => Some(
            "Recent read: when the signal feels misread, often prefers staying with the current course.",
        ),
        _ => None,
    }
}

fn render_counteroffer_summary(counteroffer: &CounterofferRecord) -> String {
    if let Some(response_id) = counteroffer.requested_response_id.as_deref() {
        return format!(
            "{} is asking for {}.",
            owner_label(&counteroffer.owner),
            requested_response_label(response_id)
        );
    }
    if let Some(stance) = counteroffer.requested_stance.as_deref() {
        return format!(
            "{} is asking for {}.",
            owner_label(&counteroffer.owner),
            stance_label(stance)
        );
    }
    counteroffer.note.clone()
}

pub(super) fn proposal_order_key(proposal: &ActiveSovereigntyProposal) -> u64 {
    proposal
        .outcomes
        .iter()
        .map(|outcome| outcome.recorded_at_unix_s)
        .max()
        .unwrap_or(proposal.latest_match_at_unix_s)
}

fn is_resolved_proposal(proposal: &ActiveSovereigntyProposal) -> bool {
    matches!(
        proposal.reply_state.as_str(),
        "declined" | "expired" | "integrated"
    ) || proposal.outcome_status == "integrated"
}

fn owner_label(owner: &str) -> &'static str {
    if owner == OWNER_MINIME {
        "Minime"
    } else {
        "Astrid"
    }
}

fn stance_label(stance: &str) -> &'static str {
    match stance {
        STANCE_SOFTER_CONTACT => "softer contact",
        STANCE_SLOWER_PACING => "slower pacing",
        STANCE_MORE_SPACE => "more space",
        STANCE_STAY_WITH_ME => "staying with the current relation",
        _ => "a different relation stance",
    }
}

fn requested_response_label(response_id: &str) -> &'static str {
    match response_id {
        "minime_notice_first" => "NOTICE",
        "minime_recover_regime" => "recover",
        "minime_semantic_probe" => "the semantic probe",
        "astrid_dampen" => "DAMPEN",
        "astrid_breathe_alone" => "BREATHE_ALONE",
        "astrid_echo_off" => "ECHO_OFF",
        _ => "that bounded response",
    }
}

fn prefer_kind<'a>(existing: &'a str, incoming: &'a str) -> &'a str {
    let rank = |kind: &str| match kind {
        KIND_NEGOTIATED => 3,
        KIND_DECLARED => 2,
        KIND_BEHAVIORAL => 1,
        _ => 0,
    };
    if rank(incoming) >= rank(existing) {
        incoming
    } else {
        existing
    }
}
