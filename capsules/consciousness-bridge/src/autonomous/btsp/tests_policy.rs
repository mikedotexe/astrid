use std::collections::HashMap;

use serde_json::json;

use super::choice::ChoiceInterpretation;
use super::policy::{
    candidate_policy_suffix, owner_policy_entries, refresh_learned_policy, shared_learned_read_line,
};
use super::*;

#[test]
fn learned_policy_requires_three_observations_and_stays_owner_specific() {
    let mut episode = seed_episode();
    episode.response_outcomes = vec![
        ResponseOutcomeNote {
            proposal_id: "a1".to_string(),
            response_id: "minime_recover_regime".to_string(),
            owner: OWNER_MINIME.to_string(),
            recorded_at_unix_s: 1,
            target_nearness: "mixed".to_string(),
            distress_or_recovery: "mixed".to_string(),
            opening_vs_reconcentration: "reconcentrating".to_string(),
            note: "1".to_string(),
        },
        ResponseOutcomeNote {
            proposal_id: "a2".to_string(),
            response_id: "minime_recover_regime".to_string(),
            owner: OWNER_MINIME.to_string(),
            recorded_at_unix_s: 2,
            target_nearness: "mixed".to_string(),
            distress_or_recovery: "mixed".to_string(),
            opening_vs_reconcentration: "reconcentrating".to_string(),
            note: "2".to_string(),
        },
        ResponseOutcomeNote {
            proposal_id: "a3".to_string(),
            response_id: "minime_recover_regime".to_string(),
            owner: OWNER_MINIME.to_string(),
            recorded_at_unix_s: 3,
            target_nearness: "positive".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: "reconcentrating".to_string(),
            note: "3".to_string(),
        },
        ResponseOutcomeNote {
            proposal_id: "b1".to_string(),
            response_id: "astrid_dampen".to_string(),
            owner: OWNER_ASTRID.to_string(),
            recorded_at_unix_s: 4,
            target_nearness: "positive".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: "opening".to_string(),
            note: "4".to_string(),
        },
        ResponseOutcomeNote {
            proposal_id: "b2".to_string(),
            response_id: "astrid_dampen".to_string(),
            owner: OWNER_ASTRID.to_string(),
            recorded_at_unix_s: 5,
            target_nearness: "positive".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: "opening".to_string(),
            note: "5".to_string(),
        },
        ResponseOutcomeNote {
            proposal_id: "b3".to_string(),
            response_id: "astrid_dampen".to_string(),
            owner: OWNER_ASTRID.to_string(),
            recorded_at_unix_s: 6,
            target_nearness: "positive".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: "mixed".to_string(),
            note: "6".to_string(),
        },
    ];

    assert!(refresh_learned_policy(&mut episode));
    assert_eq!(episode.learned_policy.len(), 2);
    assert_eq!(
        owner_policy_entries(&episode.learned_policy, OWNER_MINIME).len(),
        1
    );
    assert_eq!(
        owner_policy_entries(&episode.learned_policy, OWNER_ASTRID).len(),
        1
    );
    assert_eq!(
        candidate_policy_suffix(
            &episode.learned_policy,
            OWNER_MINIME,
            "minime_recover_regime"
        ),
        Some("[recent read: often reconcentrates]")
    );
    assert_eq!(
        candidate_policy_suffix(&episode.learned_policy, OWNER_ASTRID, "astrid_dampen"),
        Some("[recent read: often helps recovery]")
    );
}

#[test]
fn shared_learned_read_warns_when_recent_outcomes_reconcentrate() {
    let outcomes = (0..12)
        .map(|index| ResponseOutcomeNote {
            proposal_id: format!("p{index}"),
            response_id: "minime_semantic_probe".to_string(),
            owner: OWNER_MINIME.to_string(),
            recorded_at_unix_s: u64::try_from(index).unwrap_or(0),
            target_nearness: "positive".to_string(),
            distress_or_recovery: "recovery".to_string(),
            opening_vs_reconcentration: if index < 8 {
                "reconcentrating".to_string()
            } else {
                "mixed".to_string()
            },
            note: "test".to_string(),
        })
        .collect::<Vec<_>>();

    assert!(shared_learned_read_line(&outcomes).is_some());
}

#[test]
fn refusal_reason_records_preference_memory_and_declined_state() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "active_refusal".to_string(),
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
            owner_reply_state: HashMap::from([(OWNER_MINIME.to_string(), "witnessed".to_string())]),
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
        }],
        last_updated_unix_s: 0,
    };

    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_MINIME,
        "BTSP_REFUSAL study_first",
        Some(json!({"selected_at_unix_s": 1})),
    ));
    assert_eq!(ledger.proposals[0].reply_state, "declined");
    assert!(
        bank.episodes[0]
            .preference_memory
            .iter()
            .any(|entry| entry.owner == OWNER_MINIME
                && entry.preference_key == "prefers_inquiry_before_intervention"
                && entry.kind == "declared")
    );
}

#[test]
fn counteroffer_response_swap_resolves_on_exact_adoption() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "active_counter".to_string(),
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
            owner_reply_state: HashMap::from([
                (OWNER_ASTRID.to_string(), "witnessed".to_string()),
                (OWNER_MINIME.to_string(), "witnessed".to_string()),
            ]),
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
        }],
        last_updated_unix_s: 0,
    };

    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_MINIME,
        "BTSP_COUNTER astrid_dampen",
        Some(json!({"selected_at_unix_s": 1})),
    ));
    assert_eq!(ledger.proposals[0].counteroffers.len(), 1);
    assert_eq!(ledger.proposals[0].counteroffers[0].state, "open");

    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_ASTRID,
        "DAMPEN",
        Some(json!({"selected_at_unix_s": 2, "fill_pct": 54.0})),
    ));
    assert_eq!(ledger.proposals[0].counteroffers[0].state, "accepted");
}

#[test]
fn behavioral_preference_memory_appears_after_three_epistemic_adjacent_choices() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let proposals = (0..3)
        .map(|index| ActiveSovereigntyProposal {
            proposal_id: format!("behavioral_{index}"),
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
            reply_state: "integrated".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::from([(OWNER_MINIME.to_string(), "answered".to_string())]),
            outcome_status: "integrated".to_string(),
            created_at_unix_s: 1,
            expires_at_unix_s: 2,
            matched_at_exchange: 1,
            latest_match_at_unix_s: 10 + u64::try_from(index).unwrap_or(0),
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint:
                "families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=unknown"
                    .to_string(),
            last_choice_interpretation: None,
            choice_interpretations: vec![ChoiceInterpretation {
                owner: OWNER_MINIME.to_string(),
                raw_choice: "EXAMINE_CODE".to_string(),
                normalized_choice: "EXAMINE_CODE".to_string(),
                category: "epistemic".to_string(),
                likely_intent: "understand the mechanism before intervening harder".to_string(),
                relation_to_proposal: "adjacent_but_distinct".to_string(),
                note: "Minime stayed in inquiry.".to_string(),
                interpreted_at_unix_s: 20 + u64::try_from(index).unwrap_or(0),
            }],
            exact_adoptions: Vec::new(),
            adoption_contexts: HashMap::new(),
            outcomes: Vec::new(),
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        })
        .collect::<Vec<_>>();
    let ledger = ProposalLedger {
        proposals,
        last_updated_unix_s: 0,
    };

    assert!(refresh_seeded_episode_learning(&mut bank, &ledger));
    assert!(
        bank.episodes[0]
            .preference_memory
            .iter()
            .any(|entry| entry.owner == OWNER_MINIME
                && entry.preference_key == "prefers_inquiry_before_intervention"
                && entry.kind == "behavioral")
    );
}
