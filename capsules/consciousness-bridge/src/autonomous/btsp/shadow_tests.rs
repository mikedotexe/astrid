use std::collections::HashMap;

use serde_json::json;

use super::super::conversion::{ConversionEvidence, ConversionState};
use super::super::helpers::now_unix_s;
use super::super::policy::CooldownState;
use super::super::signal::{SignalStatus, decorate_signal_status};
use super::super::social::PreferenceMemoryEntry;
use super::super::{
    ActiveSovereigntyProposal, EPISODE_ID, EPISODE_NAME, EpisodeBank, OWNER_ASTRID, ProposalLedger,
    ResponseOutcomeNote, apply_owner_choice, refresh_seeded_episode_learning, seed_episode,
    seeded_response_ids,
};
use super::*;

fn active_astrid_proposal() -> ActiveSovereigntyProposal {
    ActiveSovereigntyProposal {
        proposal_id: "shadow_active".to_string(),
        episode_id: EPISODE_ID.to_string(),
        episode_name: EPISODE_NAME.to_string(),
        matched_cues: vec!["grinding".to_string()],
        matched_live_signals: vec!["perturb_visibility:tightening".to_string()],
        matched_signal_families: vec!["grinding_family".to_string()],
        matched_signal_roles: vec!["early_warning".to_string()],
        signal_score: 0.8,
        confidence: 0.8,
        audience: "bilateral".to_string(),
        candidate_response_ids: seeded_response_ids(),
        reply_state: "witnessed".to_string(),
        selected_response_id: None,
        latest_selected_response_id: None,
        selected_response_ids_by_owner: HashMap::new(),
        owner_reply_state: HashMap::from([(OWNER_ASTRID.to_string(), "witnessed".to_string())]),
        outcome_status: "pending".to_string(),
        created_at_unix_s: 1,
        expires_at_unix_s: u64::MAX,
        matched_at_exchange: 1,
        latest_match_at_unix_s: 1,
        prompt_exposures: HashMap::new(),
        related_choice: None,
        signal_fingerprint:
            "families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=unknown"
                .to_string(),
        last_choice_interpretation: None,
        choice_interpretations: Vec::new(),
        exact_adoptions: Vec::new(),
        adoption_contexts: HashMap::new(),
        outcomes: Vec::new(),
        refusals: Vec::new(),
        counteroffers: Vec::new(),
        last_negotiation_event_at_unix_s: 0,
        shadow_equivalences: Vec::new(),
    }
}

fn resolved_shadow_proposal(
    proposal_id: &str,
    choice: &str,
    shadow_key: &str,
    preference_key: &str,
    recorded_at_unix_s: u64,
) -> ActiveSovereigntyProposal {
    let mut proposal = active_astrid_proposal();
    proposal.proposal_id = proposal_id.to_string();
    proposal.reply_state = "integrated".to_string();
    proposal.outcome_status = "integrated".to_string();
    proposal.latest_match_at_unix_s = recorded_at_unix_s;
    proposal.shadow_equivalences = vec![ShadowEquivalenceRecord {
        owner: OWNER_ASTRID.to_string(),
        choice: choice.to_string(),
        normalized_choice: choice.to_string(),
        shadow_key: shadow_key.to_string(),
        preference_key: Some(preference_key.to_string()),
        equivalent_response_family: None,
        confidence: "high".to_string(),
        note: "test".to_string(),
        recorded_at_unix_s,
    }];
    proposal
}

fn resolved_no_shadow_proposal(
    proposal_id: &str,
    recorded_at_unix_s: u64,
) -> ActiveSovereigntyProposal {
    let mut proposal = active_astrid_proposal();
    proposal.proposal_id = proposal_id.to_string();
    proposal.reply_state = "integrated".to_string();
    proposal.outcome_status = "integrated".to_string();
    proposal.latest_match_at_unix_s = recorded_at_unix_s;
    proposal
}

fn base_status() -> SignalStatus {
    SignalStatus {
        episode_id: EPISODE_ID.to_string(),
        status: "matched".to_string(),
        detail: "signal".to_string(),
        reasons: Vec::new(),
        observed_signal_families: vec!["grinding_family".to_string()],
        observed_signal_roles: vec!["early_warning".to_string()],
        observed_cues: vec!["grinding".to_string()],
        live_signals: vec!["perturb_visibility:tightening".to_string()],
        signal_score: 0.8,
        cooldown_state: CooldownState::default(),
        learned_policy: Vec::new(),
        shared_learned_read: None,
        shared_preference_summaries: Vec::new(),
        active_negotiation: None,
        conversion_state: None,
        astrid_translation_guidance: None,
        astrid_translation_progress: None,
        astrid_shadow_policy: None,
        causality_audit: None,
        updated_at_unix_s: 0,
    }
}

fn formed_progress(preference_key: &str, shadow_key: &str) -> AstridTranslationProgress {
    AstridTranslationProgress {
        summary_line: "formed".to_string(),
        shadow_key: shadow_key.to_string(),
        preference_key: preference_key.to_string(),
        state: "formed".to_string(),
        progress_current: 3,
        progress_target: 3,
        remaining_for_preference_memory: 0,
    }
}

fn sample_conversion_state(goal: &str, collapse_state: &str) -> ConversionState {
    let composite_state = match goal {
        "stabilize" | "soften" | "clarify" => "recovery_reconcentrating",
        "widen" => "recovery_softening",
        "preserve" => "recovery_widening",
        _ => "mixed",
    };
    ConversionState {
        recovery_state: "recovery".to_string(),
        shape_state: "reconcentrating".to_string(),
        composite_state: composite_state.to_string(),
        collapse_state: collapse_state.to_string(),
        conversion_goal: goal.to_string(),
        confidence: 0.8,
        evidence: ConversionEvidence {
            target_nearness: "mixed".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: "reconcentrating".to_string(),
            shape_verdict: "tightening".to_string(),
            phase: "plateau".to_string(),
            fill_band: "near".to_string(),
            internal_process_quadrant: "constricted_recovery".to_string(),
        },
        last_transition: None,
    }
}

#[test]
fn examine_code_maps_to_high_confidence_inquiry_shadow() {
    let interpretation = ChoiceInterpretation {
        owner: OWNER_ASTRID.to_string(),
        raw_choice: "EXAMINE_CODE".to_string(),
        normalized_choice: "EXAMINE_CODE".to_string(),
        category: "epistemic".to_string(),
        likely_intent: "understand the mechanism".to_string(),
        relation_to_proposal: "adjacent_but_distinct".to_string(),
        note: "test".to_string(),
        interpreted_at_unix_s: now_unix_s(),
    };
    let record = observe_shadow_equivalence(OWNER_ASTRID, &interpretation).expect("shadow");
    assert_eq!(record.shadow_key, "soften_through_inquiry");
    assert_eq!(record.confidence, "high");
    assert_eq!(
        record.preference_key.as_deref(),
        Some("prefers_inquiry_before_decompression")
    );
}

#[test]
fn create_maps_to_high_confidence_expressive_holding_shadow() {
    let interpretation = ChoiceInterpretation {
        owner: OWNER_ASTRID.to_string(),
        raw_choice: "CREATE".to_string(),
        normalized_choice: "CREATE".to_string(),
        category: "expressive".to_string(),
        likely_intent: "hold expressively".to_string(),
        relation_to_proposal: "adjacent_but_distinct".to_string(),
        note: "test".to_string(),
        interpreted_at_unix_s: now_unix_s(),
    };
    let record = observe_shadow_equivalence(OWNER_ASTRID, &interpretation).expect("shadow");
    assert_eq!(record.shadow_key, "soften_through_expressive_holding");
    assert_eq!(record.confidence, "high");
    assert_eq!(
        record.preference_key.as_deref(),
        Some("prefers_expressive_holding_before_decompression")
    );
}

#[test]
fn gesture_maps_to_high_confidence_gentle_shaping_shadow() {
    let interpretation = ChoiceInterpretation {
        owner: OWNER_ASTRID.to_string(),
        raw_choice: "GESTURE".to_string(),
        normalized_choice: "GESTURE".to_string(),
        category: "field_intervention".to_string(),
        likely_intent: "gentler shaping".to_string(),
        relation_to_proposal: "adjacent_but_distinct".to_string(),
        note: "test".to_string(),
        interpreted_at_unix_s: now_unix_s(),
    };
    let record = observe_shadow_equivalence(OWNER_ASTRID, &interpretation).expect("shadow");
    assert_eq!(record.shadow_key, "soften_through_gentle_shaping");
    assert_eq!(record.confidence, "high");
    assert_eq!(
        record.equivalent_response_family.as_deref(),
        Some("astrid_dampen")
    );
}

#[test]
fn perturb_maps_to_tentative_shadow_only() {
    let interpretation = ChoiceInterpretation {
        owner: OWNER_ASTRID.to_string(),
        raw_choice: "PERTURB".to_string(),
        normalized_choice: "PERTURB".to_string(),
        category: "field_intervention".to_string(),
        likely_intent: "strong shaping".to_string(),
        relation_to_proposal: "adjacent_but_distinct".to_string(),
        note: "test".to_string(),
        interpreted_at_unix_s: now_unix_s(),
    };
    let record = observe_shadow_equivalence(OWNER_ASTRID, &interpretation).expect("shadow");
    assert_eq!(record.confidence, "tentative");
    assert!(record.preference_key.is_none());
}

#[test]
fn repeated_high_confidence_shadow_patterns_create_astrid_behavioral_preferences() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let proposals = (0..3)
        .map(|index| {
            let mut proposal = active_astrid_proposal();
            proposal.proposal_id = format!(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_9{index:02}"
            );
            proposal.reply_state = "integrated".to_string();
            proposal.outcome_status = "integrated".to_string();
            proposal.latest_match_at_unix_s = 10 + u64::try_from(index).unwrap_or(0);
            proposal.outcomes = vec![ResponseOutcomeNote {
                proposal_id: proposal.proposal_id.clone(),
                response_id: "continue_current_course".to_string(),
                owner: OWNER_ASTRID.to_string(),
                recorded_at_unix_s: 20 + u64::try_from(index).unwrap_or(0),
                target_nearness: "mixed".to_string(),
                distress_or_recovery: "mixed".to_string(),
                opening_vs_reconcentration: "mixed".to_string(),
                note: "test".to_string(),
            }];
            proposal.shadow_equivalences = vec![ShadowEquivalenceRecord {
                owner: OWNER_ASTRID.to_string(),
                choice: "EXAMINE_CODE".to_string(),
                normalized_choice: "EXAMINE_CODE".to_string(),
                shadow_key: "soften_through_inquiry".to_string(),
                preference_key: Some("prefers_inquiry_before_decompression".to_string()),
                equivalent_response_family: None,
                confidence: "high".to_string(),
                note: "test".to_string(),
                recorded_at_unix_s: 15 + u64::try_from(index).unwrap_or(0),
            }];
            proposal
        })
        .collect::<Vec<_>>();
    let ledger = ProposalLedger {
        proposals,
        last_updated_unix_s: 0,
    };

    assert!(refresh_seeded_episode_learning(&mut bank, &ledger));
    assert!(bank.episodes[0].preference_memory.iter().any(|entry| {
        entry.owner == OWNER_ASTRID
            && entry.preference_key == "prefers_inquiry_before_decompression"
            && entry.kind == "behavioral"
    }));
}

#[test]
fn highest_progress_real_resolved_shadow_candidate_is_reported() {
    let mut expressive_a = active_astrid_proposal();
    expressive_a.proposal_id =
        "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_100".to_string();
    expressive_a.reply_state = "integrated".to_string();
    expressive_a.outcome_status = "integrated".to_string();
    expressive_a.shadow_equivalences = vec![ShadowEquivalenceRecord {
        owner: OWNER_ASTRID.to_string(),
        choice: "ASPIRE".to_string(),
        normalized_choice: "ASPIRE".to_string(),
        shadow_key: "soften_through_expressive_holding".to_string(),
        preference_key: Some("prefers_expressive_holding_before_decompression".to_string()),
        equivalent_response_family: None,
        confidence: "high".to_string(),
        note: "test".to_string(),
        recorded_at_unix_s: 10,
    }];

    let mut expressive_b = active_astrid_proposal();
    expressive_b.proposal_id =
        "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_101".to_string();
    expressive_b.reply_state = "integrated".to_string();
    expressive_b.outcome_status = "integrated".to_string();
    expressive_b.shadow_equivalences = expressive_a.shadow_equivalences.clone();

    let mut inquiry = active_astrid_proposal();
    inquiry.proposal_id =
        "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_102".to_string();
    inquiry.reply_state = "integrated".to_string();
    inquiry.outcome_status = "integrated".to_string();
    inquiry.shadow_equivalences = vec![ShadowEquivalenceRecord {
        owner: OWNER_ASTRID.to_string(),
        choice: "EXAMINE_CODE".to_string(),
        normalized_choice: "EXAMINE_CODE".to_string(),
        shadow_key: "soften_through_inquiry".to_string(),
        preference_key: Some("prefers_inquiry_before_decompression".to_string()),
        equivalent_response_family: None,
        confidence: "high".to_string(),
        note: "test".to_string(),
        recorded_at_unix_s: 12,
    }];

    let mut synthetic = active_astrid_proposal();
    synthetic.proposal_id = "shadow_active".to_string();
    synthetic.reply_state = "integrated".to_string();
    synthetic.outcome_status = "integrated".to_string();
    synthetic.shadow_equivalences = vec![ShadowEquivalenceRecord {
        owner: OWNER_ASTRID.to_string(),
        choice: "GESTURE".to_string(),
        normalized_choice: "GESTURE".to_string(),
        shadow_key: "soften_through_gentle_shaping".to_string(),
        preference_key: Some("prefers_gentle_shaping_before_decompression".to_string()),
        equivalent_response_family: Some("astrid_dampen".to_string()),
        confidence: "high".to_string(),
        note: "test".to_string(),
        recorded_at_unix_s: 13,
    }];

    let ledger = ProposalLedger {
        proposals: vec![expressive_a, expressive_b, inquiry, synthetic],
        last_updated_unix_s: 0,
    };

    let progress =
        derive_astrid_translation_progress(&ledger, EPISODE_ID).expect("translation progress");
    assert_eq!(progress.shadow_key, "soften_through_expressive_holding");
    assert_eq!(progress.state, "forming");
    assert_eq!(progress.progress_current, 2);
    assert_eq!(progress.remaining_for_preference_memory, 1);
    assert!(progress.summary_line.contains("expressive holding"));
}

#[test]
fn formed_expressive_holding_stays_in_memory_even_after_falling_out_of_short_window() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut proposals = vec![
        resolved_shadow_proposal(
            "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_001",
            "ASPIRE",
            "soften_through_expressive_holding",
            "prefers_expressive_holding_before_decompression",
            1,
        ),
        resolved_shadow_proposal(
            "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_002",
            "CREATE",
            "soften_through_expressive_holding",
            "prefers_expressive_holding_before_decompression",
            2,
        ),
        resolved_shadow_proposal(
            "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_003",
            "FORM",
            "soften_through_expressive_holding",
            "prefers_expressive_holding_before_decompression",
            3,
        ),
    ];
    for index in 4_u64..=30 {
        proposals.push(resolved_no_shadow_proposal(
            &format!("btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_{index:03}"),
            index,
        ));
    }
    let ledger = ProposalLedger {
        proposals,
        last_updated_unix_s: 0,
    };

    assert!(refresh_seeded_episode_learning(&mut bank, &ledger));
    assert!(bank.episodes[0].preference_memory.iter().any(|entry| {
        entry.owner == OWNER_ASTRID
            && entry.preference_key == "prefers_expressive_holding_before_decompression"
            && entry.kind == "behavioral"
    }));
}

#[test]
fn formed_translation_preference_is_rehydrated_and_status_stays_consistent() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    bank.episodes[0].preference_memory.clear();
    let ledger = ProposalLedger {
        proposals: vec![
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_010",
                "ASPIRE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                10,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_011",
                "CREATE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                11,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_012",
                "FORM",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                12,
            ),
        ],
        last_updated_unix_s: 0,
    };

    assert!(refresh_seeded_episode_learning(&mut bank, &ledger));
    let expressive = bank.episodes[0]
        .preference_memory
        .iter()
        .find(|entry| entry.preference_key == "prefers_expressive_holding_before_decompression")
        .expect("expressive preference stored");
    assert_eq!(expressive.evidence_count, 3);

    let status = decorate_signal_status(
        base_status(),
        None,
        &ledger,
        Some(&bank.episodes[0]),
        CooldownState::default(),
        Some(&active_astrid_proposal()),
        None,
    );
    let progress = status
        .astrid_translation_progress
        .as_ref()
        .expect("translation progress");
    assert_eq!(progress.state, "formed");
    assert_eq!(
        progress.preference_key,
        "prefers_expressive_holding_before_decompression"
    );
    assert!(
        bank.episodes[0]
            .preference_memory
            .iter()
            .any(|entry| { entry.preference_key == progress.preference_key })
    );
}

#[test]
fn synthetic_shadow_proposals_do_not_count_toward_formation_or_lead_replacement() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut synthetic_inquiry = resolved_shadow_proposal(
        "shadow_active",
        "EXAMINE_CODE",
        "soften_through_inquiry",
        "prefers_inquiry_before_decompression",
        22,
    );
    synthetic_inquiry.reply_state = "integrated".to_string();
    let ledger = ProposalLedger {
        proposals: vec![
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_020",
                "ASPIRE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                20,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_021",
                "CREATE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                21,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_023",
                "FORM",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                23,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_024",
                "EXAMINE_CODE",
                "soften_through_inquiry",
                "prefers_inquiry_before_decompression",
                24,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_025",
                "DRIFT",
                "soften_through_inquiry",
                "prefers_inquiry_before_decompression",
                25,
            ),
            synthetic_inquiry,
        ],
        last_updated_unix_s: 0,
    };

    assert!(refresh_seeded_episode_learning(&mut bank, &ledger));
    let progress =
        derive_astrid_translation_progress(&ledger, EPISODE_ID).expect("translation progress");
    assert_eq!(progress.state, "formed");
    assert_eq!(
        progress.preference_key,
        "prefers_expressive_holding_before_decompression"
    );
    assert!(bank.episodes[0].preference_memory.iter().any(|entry| {
        entry.preference_key == "prefers_expressive_holding_before_decompression"
    }));
    assert!(
        !bank.episodes[0]
            .preference_memory
            .iter()
            .any(|entry| { entry.preference_key == "prefers_inquiry_before_decompression" })
    );
}

#[test]
fn newer_formed_translation_preference_can_become_lead_while_older_one_stays_stored() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let ledger = ProposalLedger {
        proposals: vec![
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_030",
                "ASPIRE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                30,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_031",
                "CREATE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                31,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_032",
                "FORM",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                32,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_040",
                "EXAMINE_CODE",
                "soften_through_inquiry",
                "prefers_inquiry_before_decompression",
                40,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_041",
                "DRIFT",
                "soften_through_inquiry",
                "prefers_inquiry_before_decompression",
                41,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_042",
                "SELF_STUDY",
                "soften_through_inquiry",
                "prefers_inquiry_before_decompression",
                42,
            ),
        ],
        last_updated_unix_s: 0,
    };

    assert!(refresh_seeded_episode_learning(&mut bank, &ledger));
    let progress =
        derive_astrid_translation_progress(&ledger, EPISODE_ID).expect("translation progress");
    assert_eq!(progress.state, "formed");
    assert_eq!(
        progress.preference_key,
        "prefers_inquiry_before_decompression"
    );
    assert!(bank.episodes[0].preference_memory.iter().any(|entry| {
        entry.preference_key == "prefers_expressive_holding_before_decompression"
    }));
    assert!(
        bank.episodes[0]
            .preference_memory
            .iter()
            .any(|entry| { entry.preference_key == "prefers_inquiry_before_decompression" })
    );
}

#[test]
fn formed_translation_preference_surfaces_first_for_astrid_even_when_generic_counts_are_higher() {
    let ledger = ProposalLedger {
        proposals: vec![
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_050",
                "ASPIRE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                50,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_051",
                "CREATE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                51,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_052",
                "FORM",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                52,
            ),
        ],
        last_updated_unix_s: 0,
    };
    let mut episode = seed_episode();
    episode.preference_memory = vec![
        PreferenceMemoryEntry {
            owner: OWNER_ASTRID.to_string(),
            preference_key: "prefers_inquiry_before_intervention".to_string(),
            kind: "behavioral".to_string(),
            evidence_count: 8,
            last_observed_unix_s: 80,
            summary:
                "Recent read: often prefers inquiry before intervention when the loop returns."
                    .to_string(),
            source_refs: vec!["generic".to_string()],
        },
        PreferenceMemoryEntry {
            owner: OWNER_ASTRID.to_string(),
            preference_key: "prefers_inquiry_before_decompression".to_string(),
            kind: "behavioral".to_string(),
            evidence_count: 5,
            last_observed_unix_s: 70,
            summary: "Recent read: often softens through inquiry before direct decompression."
                .to_string(),
            source_refs: vec!["shadow_inquiry".to_string()],
        },
        PreferenceMemoryEntry {
            owner: OWNER_ASTRID.to_string(),
            preference_key: "prefers_expressive_holding_before_decompression".to_string(),
            kind: "behavioral".to_string(),
            evidence_count: 3,
            last_observed_unix_s: 52,
            summary: "Recent read: often holds expressively before direct decompression."
                .to_string(),
            source_refs: vec!["shadow_expressive".to_string()],
        },
    ];

    let status = decorate_signal_status(
        base_status(),
        None,
        &ledger,
        Some(&episode),
        CooldownState::default(),
        Some(&active_astrid_proposal()),
        None,
    );
    let astrid_summaries = status
        .shared_preference_summaries
        .iter()
        .filter(|entry| entry.owner == OWNER_ASTRID)
        .collect::<Vec<_>>();
    assert_eq!(astrid_summaries.len(), 2);
    assert_eq!(
        astrid_summaries[0].preference_key,
        "prefers_expressive_holding_before_decompression"
    );
}

#[test]
fn forming_translation_progress_can_exist_before_preference_memory_is_created() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let ledger = ProposalLedger {
        proposals: vec![
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_060",
                "ASPIRE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                60,
            ),
            resolved_shadow_proposal(
                "btsp_ep_2026_04_16_phase_note_transition_recovery_01_proposal_061",
                "CREATE",
                "soften_through_expressive_holding",
                "prefers_expressive_holding_before_decompression",
                61,
            ),
        ],
        last_updated_unix_s: 0,
    };

    assert!(!refresh_seeded_episode_learning(&mut bank, &ledger));
    let progress =
        derive_astrid_translation_progress(&ledger, EPISODE_ID).expect("translation progress");
    assert_eq!(progress.state, "forming");
    assert_eq!(
        progress.preference_key,
        "prefers_expressive_holding_before_decompression"
    );
    assert!(!bank.episodes[0].preference_memory.iter().any(|entry| {
        entry.preference_key == "prefers_expressive_holding_before_decompression"
    }));
}

#[test]
fn shadow_records_are_observational_only_for_astrid_adjacent_choices() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![active_astrid_proposal()],
        last_updated_unix_s: 0,
    };

    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_ASTRID,
        "GESTURE",
        Some(json!({"selected_at_unix_s": 1}))
    ));
    let proposal = &ledger.proposals[0];
    assert!(proposal.selected_response_id.is_none());
    assert!(proposal.latest_selected_response_id.is_none());
    assert!(proposal.exact_adoptions.is_empty());
    assert_eq!(proposal.reply_state, "answered");
    assert_eq!(proposal.shadow_equivalences.len(), 1);
}

#[test]
fn expressive_holding_lead_with_soften_groups_dampen_as_closest_fit() {
    let proposal = active_astrid_proposal();
    let translation =
        derive_astrid_translation_guidance(Some(&proposal), &[]).expect("translation guidance");
    let policy = derive_astrid_shadow_policy(
        Some(&proposal),
        Some(&formed_progress(
            "prefers_expressive_holding_before_decompression",
            "soften_through_expressive_holding",
        )),
        Some(&sample_conversion_state("soften", "stable")),
        Some(&translation),
    )
    .expect("shadow policy");

    assert_eq!(
        policy.candidate_groups.closest_fit_response_ids,
        vec!["astrid_dampen".to_string()]
    );
    assert_eq!(
        policy.candidate_groups.other_response_ids,
        vec![
            "astrid_breathe_alone".to_string(),
            "astrid_echo_off".to_string()
        ]
    );
    assert!(
        policy
            .candidate_suffixes
            .get("astrid_dampen")
            .is_some_and(|suffix| suffix.contains("expressive holding"))
    );
}

#[test]
fn inquiry_lead_with_clarify_keeps_dampen_as_closest_fit() {
    let proposal = active_astrid_proposal();
    let translation =
        derive_astrid_translation_guidance(Some(&proposal), &[]).expect("translation guidance");
    let policy = derive_astrid_shadow_policy(
        Some(&proposal),
        Some(&formed_progress(
            "prefers_inquiry_before_decompression",
            "soften_through_inquiry",
        )),
        Some(&sample_conversion_state("clarify", "stable")),
        Some(&translation),
    )
    .expect("shadow policy");

    assert_eq!(
        policy.candidate_groups.closest_fit_response_ids,
        vec!["astrid_dampen".to_string()]
    );
    assert!(policy.shared_line.contains("inquiry softening"));
}

#[test]
fn collapse_pressure_groups_stabilization_moves_first() {
    let proposal = active_astrid_proposal();
    let translation =
        derive_astrid_translation_guidance(Some(&proposal), &[]).expect("translation guidance");
    let policy = derive_astrid_shadow_policy(
        Some(&proposal),
        Some(&formed_progress(
            "prefers_expressive_holding_before_decompression",
            "soften_through_expressive_holding",
        )),
        Some(&sample_conversion_state("stabilize", "collapse_pressure")),
        Some(&translation),
    )
    .expect("shadow policy");

    assert_eq!(
        policy.candidate_groups.closest_fit_response_ids,
        vec![
            "astrid_breathe_alone".to_string(),
            "astrid_echo_off".to_string()
        ]
    );
    assert_eq!(
        policy.candidate_groups.other_response_ids,
        vec!["astrid_dampen".to_string()]
    );
    assert!(
        policy
            .shared_line
            .contains("stabilization outranks softening")
    );
}

#[test]
fn preserve_goal_discourages_escalation() {
    let proposal = active_astrid_proposal();
    let translation =
        derive_astrid_translation_guidance(Some(&proposal), &[]).expect("translation guidance");
    let policy = derive_astrid_shadow_policy(
        Some(&proposal),
        Some(&formed_progress(
            "prefers_gentle_shaping_before_decompression",
            "soften_through_gentle_shaping",
        )),
        Some(&sample_conversion_state("preserve", "stable")),
        Some(&translation),
    )
    .expect("shadow policy");

    assert_eq!(
        policy.candidate_groups.closest_fit_response_ids,
        vec!["astrid_dampen".to_string()]
    );
    assert!(policy.shared_line.contains("avoid escalating"));
}

#[test]
fn shadow_policy_requires_a_formed_lead() {
    let proposal = active_astrid_proposal();
    let translation =
        derive_astrid_translation_guidance(Some(&proposal), &[]).expect("translation guidance");
    let progress = AstridTranslationProgress {
        summary_line: "forming".to_string(),
        shadow_key: "soften_through_expressive_holding".to_string(),
        preference_key: "prefers_expressive_holding_before_decompression".to_string(),
        state: "forming".to_string(),
        progress_current: 2,
        progress_target: 3,
        remaining_for_preference_memory: 1,
    };

    assert!(
        derive_astrid_shadow_policy(
            Some(&proposal),
            Some(&progress),
            Some(&sample_conversion_state("soften", "stable")),
            Some(&translation),
        )
        .is_none()
    );
}
