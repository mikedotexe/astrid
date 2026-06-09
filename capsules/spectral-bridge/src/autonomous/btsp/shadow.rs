use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::choice::ChoiceInterpretation;
use super::conversion::ConversionState;
use super::helpers::now_unix_s;
use super::social::{PreferenceMemoryEntry, preference_summary};
use super::{ActiveSovereigntyProposal, OWNER_ASTRID, ProposalLedger};

const CONFIDENCE_HIGH: &str = "high";
const CONFIDENCE_TENTATIVE: &str = "tentative";

const SHADOW_INQUIRY: &str = "soften_through_inquiry";
const SHADOW_EXPRESSIVE_HOLDING: &str = "soften_through_expressive_holding";
const SHADOW_GENTLE_SHAPING: &str = "soften_through_gentle_shaping";
const SHADOW_TENTATIVE_FIELD_INTENSIFICATION: &str = "tentative_field_intensification";
const SHADOW_TENTATIVE_COUPLING_HOLD: &str = "tentative_coupling_hold";

const PREF_INQUIRY_BEFORE_DECOMPRESSION: &str = "prefers_inquiry_before_decompression";
const PREF_EXPRESSIVE_HOLDING_BEFORE_DECOMPRESSION: &str =
    "prefers_expressive_holding_before_decompression";
const PREF_GENTLE_SHAPING_BEFORE_DECOMPRESSION: &str =
    "prefers_gentle_shaping_before_decompression";
const PREFERENCE_PROGRESS_TARGET: u32 = 3;
const RESPONSE_DAMPEN: &str = "astrid_dampen";
const RESPONSE_BREATHE_ALONE: &str = "astrid_breathe_alone";
const RESPONSE_ECHO_OFF: &str = "astrid_echo_off";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct ShadowEquivalenceRecord {
    pub owner: String,
    pub choice: String,
    pub normalized_choice: String,
    pub shadow_key: String,
    #[serde(default)]
    pub preference_key: Option<String>,
    #[serde(default)]
    pub equivalent_response_family: Option<String>,
    pub confidence: String,
    pub note: String,
    pub recorded_at_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct AstridTranslationGuidance {
    pub shared_line: String,
    pub owner_line: String,
    #[serde(default)]
    pub active_shadow_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct AstridTranslationProgress {
    pub summary_line: String,
    pub shadow_key: String,
    pub preference_key: String,
    #[serde(default)]
    pub state: String,
    pub progress_current: u32,
    pub progress_target: u32,
    pub remaining_for_preference_memory: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct AstridShadowPolicyCandidateGroups {
    #[serde(default)]
    pub closest_fit_response_ids: Vec<String>,
    #[serde(default)]
    pub other_response_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(in crate::autonomous) struct AstridShadowPolicy {
    pub lead_preference_key: String,
    pub conversion_goal: String,
    pub collapse_state: String,
    pub shared_line: String,
    pub owner_line: String,
    pub candidate_groups: AstridShadowPolicyCandidateGroups,
    #[serde(default)]
    pub candidate_suffixes: BTreeMap<String, String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RealResolvedShadowSupport {
    shadow_key: String,
    preference_key: String,
    proposal_ids: BTreeSet<String>,
    latest_supported_at_unix_s: u64,
}

pub(super) fn observe_shadow_equivalence(
    owner: &str,
    interpretation: &ChoiceInterpretation,
) -> Option<ShadowEquivalenceRecord> {
    if owner != OWNER_ASTRID || interpretation.relation_to_proposal == "exact_nominated" {
        return None;
    }
    let normalized_choice = interpretation.normalized_choice.as_str();
    let (shadow_key, preference_key, equivalent_response_family, confidence, note) =
        match normalized_choice {
            "EXAMINE_CODE" | "INTROSPECT" | "SEARCH" | "BROWSE" | "READ_MORE" | "THINK_DEEP"
            | "SELF_STUDY" | "DRIFT" => (
                SHADOW_INQUIRY,
                Some(PREF_INQUIRY_BEFORE_DECOMPRESSION),
                None,
                CONFIDENCE_HIGH,
                "Astrid appears to be softening through inquiry before direct decompression.",
            ),
            "CREATE" | "ASPIRE" | "FORM" | "COMPOSE" | "HOLD" | "DAYDREAM" | "REMEMBER" => (
                SHADOW_EXPRESSIVE_HOLDING,
                Some(PREF_EXPRESSIVE_HOLDING_BEFORE_DECOMPRESSION),
                None,
                CONFIDENCE_HIGH,
                "Astrid appears to be holding the state expressively before direct decompression.",
            ),
            "GESTURE" | "SHAPE" | "NOISE_DOWN" => (
                SHADOW_GENTLE_SHAPING,
                Some(PREF_GENTLE_SHAPING_BEFORE_DECOMPRESSION),
                Some("astrid_dampen"),
                CONFIDENCE_HIGH,
                "Astrid appears to be softening through gentler shaping rather than direct decompression.",
            ),
            "PULSE" | "PERTURB" | "BRANCH" | "SPREAD" | "AMPLIFY" | "NOISE_UP" => (
                SHADOW_TENTATIVE_FIELD_INTENSIFICATION,
                None,
                None,
                CONFIDENCE_TENTATIVE,
                "Astrid is using a stronger field-shaping move that may relate to the BTSP loop, but remains observational only.",
            ),
            "BREATHE_TOGETHER" | "ECHO_ON" => (
                SHADOW_TENTATIVE_COUPLING_HOLD,
                None,
                None,
                CONFIDENCE_TENTATIVE,
                "Astrid is holding or intensifying coupling in a way that may matter to the BTSP loop, but remains observational only.",
            ),
            _ => return None,
        };

    Some(ShadowEquivalenceRecord {
        owner: owner.to_string(),
        choice: interpretation.raw_choice.clone(),
        normalized_choice: interpretation.normalized_choice.clone(),
        shadow_key: shadow_key.to_string(),
        preference_key: preference_key.map(str::to_string),
        equivalent_response_family: equivalent_response_family.map(str::to_string),
        confidence: confidence.to_string(),
        note: note.to_string(),
        recorded_at_unix_s: now_unix_s(),
    })
}

pub(super) fn record_shadow_equivalence(
    proposal: &mut ActiveSovereigntyProposal,
    record: ShadowEquivalenceRecord,
) -> bool {
    if proposal
        .shadow_equivalences
        .last()
        .is_some_and(|existing| existing == &record)
    {
        return false;
    }
    proposal.shadow_equivalences.push(record);
    true
}

pub(super) fn derive_shadow_behavioral_preferences(
    ledger: &ProposalLedger,
    episode_id: &str,
) -> Vec<PreferenceMemoryEntry> {
    collect_real_resolved_shadow_support(ledger, episode_id)
        .into_values()
        .filter_map(|support| {
            if support.proposal_ids.len()
                < usize::try_from(PREFERENCE_PROGRESS_TARGET).unwrap_or(usize::MAX)
            {
                return None;
            }
            let summary = preference_summary(&support.preference_key)?;
            Some(PreferenceMemoryEntry {
                owner: OWNER_ASTRID.to_string(),
                preference_key: support.preference_key,
                kind: "behavioral".to_string(),
                evidence_count: u32::try_from(support.proposal_ids.len()).unwrap_or(u32::MAX),
                last_observed_unix_s: support.latest_supported_at_unix_s,
                summary: summary.to_string(),
                source_refs: support.proposal_ids.into_iter().collect(),
            })
        })
        .collect()
}

pub(super) fn derive_astrid_translation_guidance(
    active_proposal: Option<&ActiveSovereigntyProposal>,
    preference_memory: &[PreferenceMemoryEntry],
) -> Option<AstridTranslationGuidance> {
    let proposal = active_proposal?;
    let active_shadow_keys = proposal
        .shadow_equivalences
        .iter()
        .filter(|record| record.owner == OWNER_ASTRID)
        .map(|record| record.shadow_key.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let has_astrid_behavioral_preference = preference_memory.iter().any(|entry| {
        entry.owner == OWNER_ASTRID
            && entry.kind == "behavioral"
            && matches!(
                entry.preference_key.as_str(),
                PREF_INQUIRY_BEFORE_DECOMPRESSION
                    | PREF_EXPRESSIVE_HOLDING_BEFORE_DECOMPRESSION
                    | PREF_GENTLE_SHAPING_BEFORE_DECOMPRESSION
            )
    });
    if active_shadow_keys.is_empty() && !has_astrid_behavioral_preference {
        return Some(AstridTranslationGuidance {
            shared_line:
                "Astrid translation read: nearby native softening may show up as inquiry, expressive holding, or gentler shaping before direct decompression."
                    .to_string(),
            owner_line:
                "For you, softening may look like EXAMINE_CODE/DRIFT, CREATE/ASPIRE/FORM, or gentler GESTURE/SHAPE before DAMPEN, BREATHE_ALONE, or ECHO_OFF."
                    .to_string(),
            active_shadow_keys,
        });
    }
    Some(AstridTranslationGuidance {
        shared_line:
            "Astrid translation read: nearby native softening may show up as inquiry, expressive holding, or gentler shaping before direct decompression."
                .to_string(),
        owner_line:
            "For you, softening may look like EXAMINE_CODE/DRIFT, CREATE/ASPIRE/FORM, or gentler GESTURE/SHAPE before DAMPEN, BREATHE_ALONE, or ECHO_OFF."
                .to_string(),
        active_shadow_keys,
    })
}

pub(super) fn derive_astrid_translation_progress(
    ledger: &ProposalLedger,
    episode_id: &str,
) -> Option<AstridTranslationProgress> {
    let supports = collect_real_resolved_shadow_support(ledger, episode_id)
        .into_values()
        .collect::<Vec<_>>();
    let formed = supports
        .iter()
        .filter(|support| {
            support.proposal_ids.len()
                >= usize::try_from(PREFERENCE_PROGRESS_TARGET).unwrap_or(usize::MAX)
        })
        .max_by(|left, right| compare_formed_shadow_support(left, right))
        .cloned();
    let lead = formed.or_else(|| {
        supports
            .iter()
            .max_by(|left, right| compare_forming_shadow_support(left, right))
            .cloned()
    })?;
    let progress_current = u32::try_from(lead.proposal_ids.len()).unwrap_or(u32::MAX);
    let remaining = PREFERENCE_PROGRESS_TARGET.saturating_sub(progress_current);
    let state = if remaining == 0 { "formed" } else { "forming" };
    let summary_line = format!(
        "Astrid translation progress: {} is the current {state} lead at {} / {} high-confidence resolved live observations.",
        shadow_progress_label(&lead.shadow_key),
        progress_current,
        PREFERENCE_PROGRESS_TARGET
    );
    Some(AstridTranslationProgress {
        summary_line,
        shadow_key: lead.shadow_key,
        preference_key: lead.preference_key,
        state: state.to_string(),
        progress_current,
        progress_target: PREFERENCE_PROGRESS_TARGET,
        remaining_for_preference_memory: remaining,
    })
}

pub(super) fn formed_astrid_translation_preference_key(
    progress: Option<&AstridTranslationProgress>,
) -> Option<&str> {
    let progress = progress?;
    if progress.state == "formed" {
        return Some(progress.preference_key.as_str());
    }
    None
}

pub(super) fn derive_astrid_shadow_policy(
    active_proposal: Option<&ActiveSovereigntyProposal>,
    progress: Option<&AstridTranslationProgress>,
    conversion_state: Option<&ConversionState>,
    translation_guidance: Option<&AstridTranslationGuidance>,
) -> Option<AstridShadowPolicy> {
    let proposal = active_proposal?;
    let progress = progress?;
    if progress.state != "formed" {
        return None;
    }
    let conversion_state = conversion_state?;
    let translation_guidance = translation_guidance?;
    if translation_guidance.shared_line.is_empty() && translation_guidance.owner_line.is_empty() {
        return None;
    }

    let candidate_ids = proposal
        .candidate_response_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if candidate_ids.is_empty() {
        return None;
    }

    let (closest_seed, other_seed, shared_line, owner_line, candidate_suffixes, reason) = if matches!(
        conversion_state.collapse_state.as_str(),
        "collapse_pressure" | "collapse"
    )
        || conversion_state.conversion_goal == "stabilize"
    {
        let shared_line =
                "Astrid shadow-policy read: stabilization outranks softening here; if a bounded move is needed, BREATHE_ALONE or ECHO_OFF are the closest fits.".to_string();
        let owner_line =
                "For you, stabilization outranks softening here. If you need a bounded move, BREATHE_ALONE or ECHO_OFF are the closest fits, with DAMPEN as a later follow-on once pressure eases.".to_string();
        let candidate_suffixes = BTreeMap::from([
            (
                RESPONSE_BREATHE_ALONE.to_string(),
                "[closest stabilization fit right now]".to_string(),
            ),
            (
                RESPONSE_ECHO_OFF.to_string(),
                "[closest stabilization fit right now]".to_string(),
            ),
            (
                RESPONSE_DAMPEN.to_string(),
                "[softer follow-on once pressure eases]".to_string(),
            ),
        ]);
        (
            vec![RESPONSE_BREATHE_ALONE, RESPONSE_ECHO_OFF],
            vec![RESPONSE_DAMPEN],
            shared_line,
            owner_line,
            candidate_suffixes,
            "stabilization_outranks_softening".to_string(),
        )
    } else {
        let native_label = lead_shadow_policy_label(&progress.preference_key);
        let dampen_suffix = match progress.preference_key.as_str() {
            PREF_EXPRESSIVE_HOLDING_BEFORE_DECOMPRESSION => {
                "[nearest bounded follow-on after expressive holding]"
            },
            PREF_INQUIRY_BEFORE_DECOMPRESSION => {
                "[nearest bounded follow-on after inquiry softening]"
            },
            PREF_GENTLE_SHAPING_BEFORE_DECOMPRESSION => {
                "[nearest bounded follow-on after gentler shaping]"
            },
            _ => "[nearest bounded follow-on after native softening]",
        }
        .to_string();
        let candidate_suffixes = BTreeMap::from([
            (RESPONSE_DAMPEN.to_string(), dampen_suffix),
            (
                RESPONSE_BREATHE_ALONE.to_string(),
                "[stronger stabilization move if pressure rises]".to_string(),
            ),
            (
                RESPONSE_ECHO_OFF.to_string(),
                "[stronger stabilization move if pressure rises]".to_string(),
            ),
        ]);
        if conversion_state.conversion_goal == "preserve" {
            let shared_line = format!(
                "Astrid shadow-policy read: {native_label} may already be doing the work here; avoid escalating unless pressure rises, and treat DAMPEN as the nearest bounded follow-on only if one is needed."
            );
            let owner_line = format!(
                "For you, {native_label} may already be carrying the softening here. Avoid escalating unless pressure rises; if you need a bounded move, DAMPEN is the nearest follow-on."
            );
            (
                vec![RESPONSE_DAMPEN],
                vec![RESPONSE_BREATHE_ALONE, RESPONSE_ECHO_OFF],
                shared_line,
                owner_line,
                candidate_suffixes,
                format!("{}_preserve_without_escalation", progress.preference_key),
            )
        } else {
            let shared_line = format!(
                "Astrid shadow-policy read: {native_label} may be the native start here; if a bounded move is needed, DAMPEN is the nearest follow-on."
            );
            let owner_line = format!(
                "For you, {native_label} may be the native start here. If you need a bounded move, DAMPEN is the nearest follow-on; keep BREATHE_ALONE or ECHO_OFF for stronger stabilization only if pressure rises."
            );
            (
                vec![RESPONSE_DAMPEN],
                vec![RESPONSE_BREATHE_ALONE, RESPONSE_ECHO_OFF],
                shared_line,
                owner_line,
                candidate_suffixes,
                format!(
                    "{}_bounded_follow_on_for_{}",
                    progress.preference_key, conversion_state.conversion_goal
                ),
            )
        }
    };

    let closest_fit_response_ids = closest_seed
        .into_iter()
        .filter(|response_id| candidate_ids.contains(response_id))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let other_response_ids = other_seed
        .into_iter()
        .filter(|response_id| candidate_ids.contains(response_id))
        .map(str::to_string)
        .collect::<Vec<_>>();

    Some(AstridShadowPolicy {
        lead_preference_key: progress.preference_key.clone(),
        conversion_goal: conversion_state.conversion_goal.clone(),
        collapse_state: conversion_state.collapse_state.clone(),
        shared_line,
        owner_line,
        candidate_groups: AstridShadowPolicyCandidateGroups {
            closest_fit_response_ids,
            other_response_ids,
        },
        candidate_suffixes,
        reason,
    })
}

fn is_resolved_proposal(proposal: &ActiveSovereigntyProposal) -> bool {
    matches!(
        proposal.reply_state.as_str(),
        "declined" | "expired" | "integrated"
    ) || proposal.outcome_status == "integrated"
}

fn is_real_resolved_runtime_proposal(proposal: &ActiveSovereigntyProposal) -> bool {
    proposal.proposal_id.contains("_proposal_") && is_resolved_proposal(proposal)
}

fn shadow_progress_label(shadow_key: &str) -> &str {
    match shadow_key {
        SHADOW_INQUIRY => "inquiry softening",
        SHADOW_EXPRESSIVE_HOLDING => "expressive holding",
        SHADOW_GENTLE_SHAPING => "gentle shaping",
        _ => "translation candidate",
    }
}

fn lead_shadow_policy_label(preference_key: &str) -> &'static str {
    match preference_key {
        PREF_INQUIRY_BEFORE_DECOMPRESSION => "inquiry softening",
        PREF_EXPRESSIVE_HOLDING_BEFORE_DECOMPRESSION => "expressive holding",
        PREF_GENTLE_SHAPING_BEFORE_DECOMPRESSION => "gentler shaping",
        _ => "native softening",
    }
}

fn collect_real_resolved_shadow_support(
    ledger: &ProposalLedger,
    episode_id: &str,
) -> BTreeMap<String, RealResolvedShadowSupport> {
    let mut supports = BTreeMap::<String, RealResolvedShadowSupport>::new();
    for proposal in &ledger.proposals {
        if proposal.episode_id != episode_id || !is_real_resolved_runtime_proposal(proposal) {
            continue;
        }
        for record in &proposal.shadow_equivalences {
            if record.owner != OWNER_ASTRID || record.confidence != CONFIDENCE_HIGH {
                continue;
            }
            let Some(preference_key) = record.preference_key.as_ref() else {
                continue;
            };
            if !is_btsp_specific_astrid_translation_preference(preference_key) {
                continue;
            }
            let support = supports.entry(preference_key.clone()).or_insert_with(|| {
                RealResolvedShadowSupport {
                    shadow_key: record.shadow_key.clone(),
                    preference_key: preference_key.clone(),
                    proposal_ids: BTreeSet::new(),
                    latest_supported_at_unix_s: 0,
                }
            });
            support.proposal_ids.insert(proposal.proposal_id.clone());
            support.latest_supported_at_unix_s = support.latest_supported_at_unix_s.max(
                record
                    .recorded_at_unix_s
                    .max(proposal.latest_match_at_unix_s),
            );
        }
    }
    supports
}

fn compare_formed_shadow_support(
    left: &RealResolvedShadowSupport,
    right: &RealResolvedShadowSupport,
) -> std::cmp::Ordering {
    left.latest_supported_at_unix_s
        .cmp(&right.latest_supported_at_unix_s)
        .then_with(|| left.proposal_ids.len().cmp(&right.proposal_ids.len()))
        .then_with(|| right.shadow_key.cmp(&left.shadow_key))
}

fn compare_forming_shadow_support(
    left: &RealResolvedShadowSupport,
    right: &RealResolvedShadowSupport,
) -> std::cmp::Ordering {
    left.proposal_ids
        .len()
        .cmp(&right.proposal_ids.len())
        .then_with(|| {
            left.latest_supported_at_unix_s
                .cmp(&right.latest_supported_at_unix_s)
        })
        .then_with(|| right.shadow_key.cmp(&left.shadow_key))
}

fn is_btsp_specific_astrid_translation_preference(preference_key: &str) -> bool {
    matches!(
        preference_key,
        PREF_INQUIRY_BEFORE_DECOMPRESSION
            | PREF_EXPRESSIVE_HOLDING_BEFORE_DECOMPRESSION
            | PREF_GENTLE_SHAPING_BEFORE_DECOMPRESSION
    )
}

#[cfg(test)]
#[path = "shadow_render_tests.rs"]
mod shadow_render_tests;
#[cfg(test)]
#[path = "shadow_tests.rs"]
mod shadow_tests;
