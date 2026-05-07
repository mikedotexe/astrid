use std::collections::HashMap;

use serde_json::json;

use super::adoption::ExactAdoption;
use super::choice::{interpret_choice, interpret_exact_choice, is_same_family_adjacent};
use super::policy::{build_signal_fingerprint, cooldown_state_for};
use super::*;

fn test_fingerprint(matched_signal_families: &[&str], health: serde_json::Value) -> String {
    let families = matched_signal_families
        .iter()
        .map(|family| (*family).to_string())
        .collect::<Vec<_>>();
    build_signal_fingerprint(&families, Some(&health))
}

#[test]
fn response_matches_action_base() {
    let response = NominatedResponse {
        response_id: "astrid_dampen".to_string(),
        owner: OWNER_ASTRID.to_string(),
        kind: "codec".to_string(),
        action: "DAMPEN".to_string(),
        parameters: json!({}),
        rationale: "test".to_string(),
        policy_state: "supported".to_string(),
    };
    assert!(response_matches_choice(&response, "DAMPEN"));
    assert!(!response_matches_choice(&response, "BREATHE_ALONE"));
}

#[test]
fn response_matches_regime_choice() {
    let response = NominatedResponse {
        response_id: "minime_recover_regime".to_string(),
        owner: OWNER_MINIME.to_string(),
        kind: "runtime".to_string(),
        action: "regime".to_string(),
        parameters: json!({"regime": "recover"}),
        rationale: "test".to_string(),
        policy_state: "supported".to_string(),
    };
    assert!(response_matches_choice(&response, "REGIME:RECOVER"));
    assert!(!response_matches_choice(&response, "REGIME:EXPLORE"));
}

#[test]
fn detect_live_signals_catches_transition_and_tightening() {
    let health = json!({
        "fill_band": "near",
        "transition_event": {
            "kind": "phase_transition",
            "description": "contracting -> plateau",
            "crossed_fill_band": true,
            "fill_band": "near"
        },
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });
    let signals = signal::detect_live_signals(Some(&health));
    assert!(
        signals
            .iter()
            .any(|signal| signal.contains("phase_transition"))
    );
    assert!(
        signals
            .iter()
            .any(|signal| signal.contains("fill_band_crossing"))
    );
    assert!(signals.iter().any(|signal| signal.contains("tightening")));
}

#[test]
fn typed_breathing_phase_does_not_masquerade_as_basin_transition() {
    let health = json!({
        "fill_band": "near",
        "transition_event_v1": {
            "kind": "breathing_phase",
            "legacy_kind": "phase_transition",
            "description": "contracting -> expanding"
        }
    });
    let signals = signal::detect_live_signals(Some(&health));
    assert!(
        signals
            .iter()
            .any(|signal| signal.starts_with("breathing_phase:"))
    );
    assert!(
        !signals
            .iter()
            .any(|signal| signal.starts_with("phase_transition:"))
    );
}

#[test]
fn typed_basin_transition_enters_signal_fingerprint_as_transition_class() {
    let fingerprint = test_fingerprint(
        &["grinding_family"],
        json!({
            "fill_band": "near",
            "transition_event_v1": {
                "kind": "basin_transition",
                "legacy_kind": "phase_transition",
                "description": "basin shift candidate"
            }
        }),
    );
    assert!(fingerprint.contains("transition=basin_transition"));
}

#[test]
fn seeded_episode_contains_bilateral_candidates() {
    let episode = seed_episode();
    let minime = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_MINIME)
        .count();
    let astrid = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_ASTRID)
        .count();
    assert_eq!(minime, 3);
    assert_eq!(astrid, 3);
}

#[test]
fn minime_inbox_note_preserves_owner_specific_advisory_context() {
    let proposal = ActiveSovereigntyProposal {
        proposal_id: "proposal_for_minime".to_string(),
        episode_id: EPISODE_ID.to_string(),
        episode_name: EPISODE_NAME.to_string(),
        matched_cues: vec!["grinding".to_string()],
        matched_live_signals: vec!["breathing_phase:contracting -> expanding".to_string()],
        matched_signal_families: vec!["grinding_family".to_string()],
        matched_signal_roles: vec!["early_warning".to_string()],
        signal_score: 0.8,
        confidence: 0.8,
        audience: "bilateral".to_string(),
        candidate_response_ids: seeded_response_ids(),
        reply_state: "unseen".to_string(),
        selected_response_id: None,
        latest_selected_response_id: None,
        selected_response_ids_by_owner: HashMap::new(),
        owner_reply_state: HashMap::new(),
        outcome_status: "pending".to_string(),
        created_at_unix_s: 10,
        expires_at_unix_s: 20,
        matched_at_exchange: 1,
        latest_match_at_unix_s: 10,
        prompt_exposures: HashMap::new(),
        related_choice: None,
        signal_fingerprint: "families=grinding_family".to_string(),
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

    let note = render_minime_inbox_note(
        &proposal,
        "Candidate responses for you:\n- NOTICE - name the tightening",
    );

    assert!(note.contains("Source: astrid:btsp_sovereignty_proposal"));
    assert!(note.contains("Proposal: proposal_for_minime"));
    assert!(note.contains("advisory only"));
    assert!(note.contains("Candidate responses for you"));
    assert!(note.contains("NEXT syntax"));
}

#[test]
fn active_proposal_detection_respects_final_states() {
    let ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "p".to_string(),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: vec!["grinding".to_string(), "central density".to_string()],
            matched_live_signals: vec![],
            matched_signal_families: vec!["grinding_family".to_string()],
            matched_signal_roles: vec!["early_warning".to_string()],
            signal_score: 0.7,
            confidence: 0.7,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "expired".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::new(),
            outcome_status: "expired".to_string(),
            created_at_unix_s: 1,
            expires_at_unix_s: 2,
            matched_at_exchange: 9,
            latest_match_at_unix_s: 2,
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint:
                "families=grinding_family;transition=none;crossing=none;perturb=none;fill_band=unknown"
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
        last_updated_unix_s: 2,
    };
    assert!(!has_active_proposal(&ledger, EPISODE_ID));
}

#[test]
fn declined_proposal_records_continue_current_course_evidence() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "declined".to_string(),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: vec!["grinding".to_string()],
            matched_live_signals: vec![],
            matched_signal_families: vec!["grinding_family".to_string()],
            matched_signal_roles: vec!["early_warning".to_string()],
            signal_score: 0.6,
            confidence: 0.6,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "declined".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::from([(OWNER_MINIME.to_string(), "declined".to_string())]),
            outcome_status: "declined".to_string(),
            created_at_unix_s: 1,
            expires_at_unix_s: 2,
            matched_at_exchange: 1,
            latest_match_at_unix_s: 2,
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint:
                "families=grinding_family;transition=none;crossing=none;perturb=none;fill_band=unknown"
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
    let health = json!({
        "fill_pct": 52.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {
            "shape_verdict": "softened_only"
        }
    });

    assert!(score_final_non_adoption_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));
    assert_eq!(ledger.proposals[0].outcome_status, "integrated");
    assert!(
        bank.episodes[0]
            .response_outcomes
            .iter()
            .any(|outcome| outcome.response_id == "continue_current_course"
                && outcome.owner == OWNER_MINIME)
    );
}

#[test]
fn expired_proposal_records_system_expiry_evidence() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "expired".to_string(),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: vec!["central density".to_string()],
            matched_live_signals: vec!["perturb_visibility:tightening".to_string()],
            matched_signal_families: vec!["central_density_family".to_string()],
            matched_signal_roles: vec!["watch_only".to_string()],
            signal_score: 0.61,
            confidence: 0.61,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "expired".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::from([(OWNER_ASTRID.to_string(), "witnessed".to_string())]),
            outcome_status: "expired".to_string(),
            created_at_unix_s: 1,
            expires_at_unix_s: 2,
            matched_at_exchange: 1,
            latest_match_at_unix_s: 2,
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint:
                "families=central_density_family;transition=none;crossing=none;perturb=tightening;fill_band=unknown"
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
    let health = json!({
        "fill_pct": 63.0,
        "target_fill_pct": 55.0,
        "fill_band": "over",
        "phase": "contracting",
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });

    assert!(score_final_non_adoption_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));
    assert_eq!(ledger.proposals[0].outcome_status, "integrated");
    assert!(
        bank.episodes[0]
            .response_outcomes
            .iter()
            .any(|outcome| outcome.response_id == "proposal_expired"
                && outcome.owner == OWNER_SYSTEM)
    );
}

#[test]
fn minime_breathe_regime_is_interpreted_as_same_family_adjacent() {
    let interpretation = interpret_choice(OWNER_MINIME, "REGIME:breathe", "REGIME:BREATHE")
        .expect("expected choice interpretation");
    assert_eq!(interpretation.category, "regulatory");
    assert_eq!(
        interpretation.likely_intent,
        "stabilize more gently before escalating"
    );
    assert!(is_same_family_adjacent(&interpretation));
}

#[test]
fn epistemic_adjacent_choice_is_recorded_as_interpreted_not_exact() {
    let interpretation = interpret_choice(OWNER_MINIME, "NEXT: RESERVOIR_READ", "RESERVOIR_READ")
        .expect("expected choice interpretation");
    assert_eq!(interpretation.category, "epistemic");
    assert_eq!(interpretation.relation_to_proposal, "adjacent_but_distinct");
}

#[test]
fn exact_choice_interpretation_marks_direct_acceptance() {
    let response = NominatedResponse {
        response_id: "minime_recover_regime".to_string(),
        owner: OWNER_MINIME.to_string(),
        kind: "runtime".to_string(),
        action: "regime".to_string(),
        parameters: json!({"regime": "recover"}),
        rationale: "test".to_string(),
        policy_state: "supported".to_string(),
    };
    let interpretation =
        interpret_exact_choice(OWNER_MINIME, "REGIME:recover", "REGIME:RECOVER", &response);
    assert_eq!(interpretation.category, "regulatory");
    assert_eq!(interpretation.relation_to_proposal, "exact_nominated");
    assert_eq!(
        interpretation.likely_intent,
        "accepted the bounded response directly"
    );
}

#[test]
fn multiple_exact_adoptions_preserve_first_and_latest_response_ids() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "active".to_string(),
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
        "EXPERIMENT semantic stimulus to self and measure spectral response",
        Some(json!({"selected_at_unix_s": 1, "fill_pct": 39.8})),
    ));
    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_MINIME,
        "REGIME:recover",
        Some(json!({"selected_at_unix_s": 2, "fill_pct": 39.8})),
    ));

    let proposal = &ledger.proposals[0];
    assert_eq!(
        proposal.selected_response_id.as_deref(),
        Some("minime_semantic_probe")
    );
    assert_eq!(
        proposal.latest_selected_response_id.as_deref(),
        Some("minime_recover_regime")
    );
    assert_eq!(proposal.exact_adoptions.len(), 2);
}

#[test]
fn scoring_uses_exact_adoption_history_not_just_latest_per_owner() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "history".to_string(),
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
            reply_state: "adopted".to_string(),
            selected_response_id: Some("minime_semantic_probe".to_string()),
            latest_selected_response_id: Some("minime_recover_regime".to_string()),
            selected_response_ids_by_owner: HashMap::from([(
                OWNER_MINIME.to_string(),
                "minime_recover_regime".to_string(),
            )]),
            owner_reply_state: HashMap::from([(OWNER_MINIME.to_string(), "adopted".to_string())]),
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
            exact_adoptions: vec![
                ExactAdoption::new(
                    OWNER_MINIME,
                    "minime_semantic_probe",
                    "EXPERIMENT semantic stimulus to self and measure spectral response",
                    "EXPERIMENT",
                    Some(json!({"fill_pct": 39.8})),
                ),
                ExactAdoption::new(
                    OWNER_MINIME,
                    "minime_recover_regime",
                    "REGIME:recover",
                    "REGIME:RECOVER",
                    Some(json!({"fill_pct": 39.8})),
                ),
            ],
            adoption_contexts: HashMap::from([(
                OWNER_MINIME.to_string(),
                json!({"fill_pct": 39.8}),
            )]),
            outcomes: Vec::new(),
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        }],
        last_updated_unix_s: 0,
    };
    let health = json!({
        "fill_pct": 50.8,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "expanding",
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });

    assert!(score_adopted_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));
    let outcomes = &ledger.proposals[0].outcomes;
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.response_id == "minime_semantic_probe")
    );
    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.response_id == "minime_recover_regime")
    );
}

#[test]
fn upsert_seed_episode_preserves_learning_fields() {
    let mut episode = seed_episode();
    episode.family_learning_notes =
        vec!["grinding_family later preceded tightening again".to_string()];
    episode.learned_policy = vec![LearnedPolicyEntry {
        owner: OWNER_MINIME.to_string(),
        response_id: "minime_recover_regime".to_string(),
        observations: 3,
        stance: "cautionary".to_string(),
        summary: "Recent read: often reconcentrates.".to_string(),
    }];
    episode.response_outcomes = vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "minime_recover_regime".to_string(),
        owner: OWNER_MINIME.to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "mixed".to_string(),
        distress_or_recovery: "mixed".to_string(),
        opening_vs_reconcentration: "reconcentrating".to_string(),
        note: "test".to_string(),
    }];
    let mut bank = EpisodeBank {
        episodes: vec![episode],
        last_updated_unix_s: 0,
    };

    assert!(!upsert_seed_episode(&mut bank));
    let refreshed = &bank.episodes[0];
    assert_eq!(refreshed.family_learning_notes.len(), 1);
    assert_eq!(refreshed.learned_policy.len(), 1);
    assert_eq!(refreshed.response_outcomes.len(), 1);
}

#[test]
fn cooldown_blocks_same_fingerprint_after_integrated_resolution() {
    let now = now_unix_s();
    let health = json!({
        "fill_band": "under",
        "transition_event": {
            "kind": "phase_transition",
            "description": "plateau -> contracting",
            "crossed_fill_band": true,
            "fill_band": "under"
        },
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });
    let fingerprint = test_fingerprint(&["grinding_family"], health);
    let ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "recent".to_string(),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: vec!["grinding".to_string()],
            matched_live_signals: vec![
                "phase_transition:plateau -> contracting".to_string(),
                "fill_band_crossing:under".to_string(),
                "perturb_visibility:tightening".to_string(),
            ],
            matched_signal_families: vec!["grinding_family".to_string()],
            matched_signal_roles: vec!["early_warning".to_string()],
            signal_score: 0.85,
            confidence: 0.85,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "integrated".to_string(),
            selected_response_id: Some("minime_notice_first".to_string()),
            latest_selected_response_id: Some("minime_notice_first".to_string()),
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::new(),
            outcome_status: "integrated".to_string(),
            created_at_unix_s: now.saturating_sub(120),
            expires_at_unix_s: now.saturating_sub(60),
            matched_at_exchange: 1,
            latest_match_at_unix_s: now.saturating_sub(60),
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint: fingerprint.clone(),
            last_choice_interpretation: None,
            choice_interpretations: Vec::new(),
            exact_adoptions: Vec::new(),
            adoption_contexts: HashMap::new(),
            outcomes: vec![ResponseOutcomeNote {
                proposal_id: "recent".to_string(),
                response_id: "minime_notice_first".to_string(),
                owner: OWNER_MINIME.to_string(),
                recorded_at_unix_s: now.saturating_sub(30),
                target_nearness: "mixed".to_string(),
                distress_or_recovery: "mixed".to_string(),
                opening_vs_reconcentration: "mixed".to_string(),
                note: "recent".to_string(),
            }],
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        }],
        last_updated_unix_s: now,
    };

    let cooldown = cooldown_state_for(&ledger, EPISODE_ID, &fingerprint);
    assert!(cooldown.active);
    assert_eq!(cooldown.reason, "recent_integrated_same_fingerprint");
}

#[test]
fn materially_changed_fingerprint_bypasses_cooldown() {
    let now = now_unix_s();
    let old_health = json!({
        "fill_band": "under",
        "transition_event": {
            "kind": "phase_transition",
            "description": "plateau -> contracting",
            "crossed_fill_band": true,
            "fill_band": "under"
        },
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });
    let new_health = json!({
        "fill_band": "near",
        "transition_event": {
            "kind": "phase_transition",
            "description": "contracting -> plateau",
            "crossed_fill_band": true,
            "fill_band": "near"
        },
        "perturb_visibility": {
            "shape_verdict": "softened_only"
        }
    });
    let old_fingerprint = test_fingerprint(&["grinding_family"], old_health);
    let new_fingerprint = test_fingerprint(&["grinding_family"], new_health);
    let ledger = ProposalLedger {
        proposals: vec![ActiveSovereigntyProposal {
            proposal_id: "recent".to_string(),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: vec!["grinding".to_string()],
            matched_live_signals: vec![
                "phase_transition:plateau -> contracting".to_string(),
                "fill_band_crossing:under".to_string(),
                "perturb_visibility:tightening".to_string(),
            ],
            matched_signal_families: vec!["grinding_family".to_string()],
            matched_signal_roles: vec!["early_warning".to_string()],
            signal_score: 0.85,
            confidence: 0.85,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "integrated".to_string(),
            selected_response_id: None,
            latest_selected_response_id: None,
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::new(),
            outcome_status: "integrated".to_string(),
            created_at_unix_s: now.saturating_sub(120),
            expires_at_unix_s: now.saturating_sub(60),
            matched_at_exchange: 1,
            latest_match_at_unix_s: now.saturating_sub(60),
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint: old_fingerprint,
            last_choice_interpretation: None,
            choice_interpretations: Vec::new(),
            exact_adoptions: Vec::new(),
            adoption_contexts: HashMap::new(),
            outcomes: vec![ResponseOutcomeNote {
                proposal_id: "recent".to_string(),
                response_id: "minime_notice_first".to_string(),
                owner: OWNER_MINIME.to_string(),
                recorded_at_unix_s: now.saturating_sub(30),
                target_nearness: "mixed".to_string(),
                distress_or_recovery: "mixed".to_string(),
                opening_vs_reconcentration: "mixed".to_string(),
                note: "recent".to_string(),
            }],
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        }],
        last_updated_unix_s: now,
    };

    let cooldown = cooldown_state_for(&ledger, EPISODE_ID, &new_fingerprint);
    assert!(!cooldown.active);
}

#[test]
fn repeated_reconcentrating_same_fingerprint_extends_cooldown() {
    let now = now_unix_s();
    let health = json!({
        "fill_band": "under",
        "transition_event": {
            "kind": "phase_transition",
            "description": "plateau -> contracting",
            "crossed_fill_band": true,
            "fill_band": "under"
        },
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });
    let fingerprint = test_fingerprint(&["grinding_family"], health);
    let proposals = (0..3)
        .map(|index| ActiveSovereigntyProposal {
            proposal_id: format!("recent_{index}"),
            episode_id: EPISODE_ID.to_string(),
            episode_name: EPISODE_NAME.to_string(),
            matched_cues: vec!["grinding".to_string()],
            matched_live_signals: vec![
                "phase_transition:plateau -> contracting".to_string(),
                "fill_band_crossing:under".to_string(),
                "perturb_visibility:tightening".to_string(),
            ],
            matched_signal_families: vec!["grinding_family".to_string()],
            matched_signal_roles: vec!["early_warning".to_string()],
            signal_score: 0.85,
            confidence: 0.85,
            audience: "bilateral".to_string(),
            candidate_response_ids: seeded_response_ids(),
            reply_state: "integrated".to_string(),
            selected_response_id: Some("minime_recover_regime".to_string()),
            latest_selected_response_id: Some("minime_recover_regime".to_string()),
            selected_response_ids_by_owner: HashMap::new(),
            owner_reply_state: HashMap::new(),
            outcome_status: "integrated".to_string(),
            created_at_unix_s: now.saturating_sub(900),
            expires_at_unix_s: now.saturating_sub(600),
            matched_at_exchange: 1,
            latest_match_at_unix_s: now
                .saturating_sub(240)
                .saturating_add(u64::try_from(index).unwrap_or(0) * 30),
            prompt_exposures: HashMap::new(),
            related_choice: None,
            signal_fingerprint: fingerprint.clone(),
            last_choice_interpretation: None,
            choice_interpretations: Vec::new(),
            exact_adoptions: Vec::new(),
            adoption_contexts: HashMap::new(),
            outcomes: vec![ResponseOutcomeNote {
                proposal_id: format!("recent_{index}"),
                response_id: "minime_recover_regime".to_string(),
                owner: OWNER_MINIME.to_string(),
                recorded_at_unix_s: now
                    .saturating_sub(180)
                    .saturating_add(u64::try_from(index).unwrap_or(0) * 30),
                target_nearness: "positive".to_string(),
                distress_or_recovery: "recovery".to_string(),
                opening_vs_reconcentration: "reconcentrating".to_string(),
                note: "reconcentrating".to_string(),
            }],
            refusals: Vec::new(),
            counteroffers: Vec::new(),
            last_negotiation_event_at_unix_s: 0,
            shadow_equivalences: Vec::new(),
        })
        .collect::<Vec<_>>();
    let ledger = ProposalLedger {
        proposals,
        last_updated_unix_s: now,
    };

    let cooldown = cooldown_state_for(&ledger, EPISODE_ID, &fingerprint);
    assert!(cooldown.active);
    assert_eq!(cooldown.reason, "repeated_reconcentrating_same_fingerprint");
    assert!(cooldown.until_unix_s >= now.saturating_add(10 * 60));
}
