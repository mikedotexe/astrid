use std::collections::HashMap;

use serde_json::json;

use super::super::state::ConversationState;
use super::adoption::ExactAdoption;
use super::policy::CooldownState;
use super::*;

fn active_test_proposal(proposal_id: &str) -> ActiveSovereigntyProposal {
    ActiveSovereigntyProposal {
        proposal_id: proposal_id.to_string(),
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
        study_first_records: Vec::new(),
        last_negotiation_event_at_unix_s: 0,
        shadow_equivalences: Vec::new(),
    }
}

#[test]
fn exact_accept_scores_as_agency_recovery() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut proposal = active_test_proposal("agency_exact");
    proposal.reply_state = "adopted".to_string();
    proposal.owner_reply_state = HashMap::from([(OWNER_MINIME.to_string(), "adopted".to_string())]);
    proposal.exact_adoptions = vec![ExactAdoption::new(
        OWNER_MINIME,
        "minime_notice_first",
        "NOTICE",
        "NOTICE",
        Some(json!({"fill_pct": 44.0})),
    )];
    let mut ledger = ProposalLedger {
        proposals: vec![proposal],
        last_updated_unix_s: 0,
    };
    let health = json!({
        "fill_pct": 50.8,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "expanding",
        "perturb_visibility": {"shape_verdict": "softened_only"}
    });

    assert!(agency::score_adopted_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));

    let outcome = ledger.proposals[0]
        .outcomes
        .iter()
        .find(|outcome| outcome.response_id == "minime_notice_first")
        .expect("expected exact adoption outcome");
    assert!(outcome.note.contains("Agency outcome exact_accept"));
    assert!(outcome.note.contains("reason_codes=exact_accept"));
    let telemetry = outcome
        .outcome_telemetry_v2
        .as_ref()
        .expect("structured telemetry");
    assert_eq!(telemetry.phase, "expanding");
    assert_eq!(telemetry.fill_band, "near");
    assert_eq!(telemetry.shape_verdict, "softened_only");
}

#[test]
fn adjacent_observed_next_records_weak_agency_outcome_once() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![active_test_proposal("agency_adjacent")],
        last_updated_unix_s: 0,
    };
    let reply = "\
BTSP_PROPOSAL_ID agency_adjacent
BTSP_OBSERVED_NEXT DECOMPOSE
";

    assert!(record_minime_reply_into_runtime(
        &mut bank,
        &mut ledger,
        None,
        reply,
        "hash"
    ));
    let health = json!({
        "fill_pct": 51.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {"shape_verdict": "softened_only"}
    });
    assert!(agency::score_final_non_adoption_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));
    assert!(!agency::score_final_non_adoption_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));

    let proposal = &ledger.proposals[0];
    assert_eq!(proposal.reply_state, "answered");
    assert_eq!(
        proposal
            .outcomes
            .iter()
            .filter(|outcome| outcome.response_id == "adjacent_uptake")
            .count(),
        1
    );
    let telemetry = proposal.outcomes[0]
        .outcome_telemetry_v2
        .as_ref()
        .expect("structured telemetry");
    assert_eq!(telemetry.phase, "plateau");
    assert_eq!(telemetry.shape_verdict, "softened_only");
}

#[test]
fn study_first_records_distinct_agency_outcome() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![active_test_proposal("agency_study_first")],
        last_updated_unix_s: 0,
    };

    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_MINIME,
        "NEXT: BTSP_STUDY_FIRST need evidence first",
        None,
    ));
    let health = json!({
        "fill_pct": 51.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {"shape_verdict": "softened_only"}
    });
    assert!(agency::score_final_non_adoption_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));

    let proposal = &ledger.proposals[0];
    assert_eq!(
        proposal
            .outcomes
            .iter()
            .filter(|outcome| outcome.response_id == "study_first")
            .count(),
        1
    );
    let telemetry = proposal.outcomes[0]
        .outcome_telemetry_v2
        .as_ref()
        .expect("structured telemetry");
    assert_eq!(telemetry.fill_pct, Some(51.0));
    assert_eq!(telemetry.target_fill_pct, Some(55.0));
    assert!(
        !proposal
            .outcomes
            .iter()
            .any(|outcome| outcome.response_id == "adjacent_uptake")
    );
}

#[test]
fn expired_non_adoption_records_structured_telemetry() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut proposal = active_test_proposal("agency_expired");
    proposal.reply_state = "expired".to_string();
    proposal.outcome_status = "pending".to_string();
    proposal.owner_reply_state =
        HashMap::from([(OWNER_MINIME.to_string(), "witnessed".to_string())]);
    let mut ledger = ProposalLedger {
        proposals: vec![proposal],
        last_updated_unix_s: 0,
    };
    let health = json!({
        "fill_pct": 71.5,
        "target_fill_pct": 68.0,
        "fill_band": "over",
        "phase": "contracting",
        "perturb_visibility": {"shape_verdict": "tightening"},
        "pressure_source_status": {"dominant_source": "semantic"},
        "active_mode_count": 5,
        "spectral_denominator_v1": {
            "effective_dimensionality": 2.5,
            "distinguishability_loss": 0.3
        },
        "inhabitable_fluctuation_v1": {"inhabitability_score": 0.7}
    });

    assert!(agency::score_final_non_adoption_outcomes(
        &mut bank,
        &mut ledger,
        Some(&health)
    ));

    let outcome = ledger.proposals[0]
        .outcomes
        .iter()
        .find(|outcome| outcome.response_id == "proposal_expired")
        .expect("system expiry outcome");
    let telemetry = outcome
        .outcome_telemetry_v2
        .as_ref()
        .expect("structured telemetry");
    assert_eq!(telemetry.fill_band, "over");
    assert_eq!(telemetry.pressure_source, "semantic");
    assert_eq!(telemetry.active_mode_count, Some(5));
    assert_eq!(telemetry.effective_dimensionality, Some(2.5));
}

#[test]
fn parseable_counteroffer_seeds_future_candidate_ranking() {
    let mut bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: vec![active_test_proposal("agency_counter")],
        last_updated_unix_s: 0,
    };

    assert!(apply_owner_choice(
        &mut bank,
        &mut ledger,
        OWNER_MINIME,
        "BTSP_COUNTER NEXT: REGIME recover",
        None,
    ));
    assert_eq!(
        ledger.proposals[0].counteroffers[0]
            .requested_response_id
            .as_deref(),
        Some("minime_recover_regime")
    );

    ledger.proposals[0].reply_state = "integrated".to_string();
    ledger.proposals[0].outcome_status = "integrated".to_string();
    let matched = signal::ProposalSignalMatch {
        matched_cues: vec!["grinding".to_string()],
        live_signals: vec!["perturb_visibility:tightening".to_string()],
        matched_signal_families: vec!["grinding_family".to_string()],
        matched_signal_roles: vec!["early_warning".to_string()],
        signal_score: 0.81,
    };
    let conv = ConversationState::new(Vec::new(), None);

    assert!(proposal::maybe_open_advisory_proposal(
        &bank,
        &mut ledger,
        &conv,
        &matched,
        "families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=unknown",
        &CooldownState::default(),
        None,
    ));

    let opened = ledger.proposals.last().expect("expected opened proposal");
    assert_eq!(
        opened.candidate_response_ids.first().map(String::as_str),
        Some("minime_recover_regime")
    );
}

#[test]
fn anti_loop_state_suppresses_duplicate_advisory_proposal() {
    let bank = EpisodeBank {
        episodes: vec![seed_episode()],
        last_updated_unix_s: 0,
    };
    let mut ledger = ProposalLedger {
        proposals: Vec::new(),
        last_updated_unix_s: 0,
    };
    let matched = signal::ProposalSignalMatch {
        matched_cues: vec!["grinding".to_string()],
        live_signals: vec!["perturb_visibility:tightening".to_string()],
        matched_signal_families: vec!["grinding_family".to_string()],
        matched_signal_roles: vec!["early_warning".to_string()],
        signal_score: 0.81,
    };
    let anti_loop = trace::BTSPAntiLoopState {
        active: true,
        reason: "same_fingerprint_overwhelmingly_reconcentrating".to_string(),
        scope: "exact".to_string(),
        fingerprint:
            "families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=unknown"
                .to_string(),
        same_fingerprint_count: 12,
        similar_fingerprint_count: 0,
        reconcentrating_count: 12,
        widening_count: 0,
        mean_similarity_score: 100.0,
        nearest_similarity_score: 100,
        suggested_routes: vec![
            "BTSP_STUDY_FIRST".to_string(),
            "BTSP_REFUSAL".to_string(),
            "BTSP_COUNTER".to_string(),
            "new_evidence".to_string(),
        ],
        counter_prompt: "This exact BTSP signal mostly recovered by reconcentrating.".to_string(),
        recommendation: "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence"
            .to_string(),
    };
    let conv = ConversationState::new(Vec::new(), None);

    assert!(!proposal::maybe_open_advisory_proposal(
        &bank,
        &mut ledger,
        &conv,
        &matched,
        "families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=unknown",
        &CooldownState::default(),
        Some(&anti_loop),
    ));
    assert!(ledger.proposals.is_empty());
}

#[test]
fn minime_envelope_contains_agency_lineage_fields() {
    let proposal = active_test_proposal("agency_envelope");
    let responses = seed_episode()
        .nominated_responses
        .into_iter()
        .filter(|response| response.owner == OWNER_MINIME)
        .collect::<Vec<_>>();

    let note = render_minime_inbox_note(&proposal, &responses, "Candidate responses for you:");

    assert!(note.contains("\"agency_hypothesis\""));
    assert!(note.contains("\"reason_codes\""));
    assert!(note.contains("\"lineage\""));
    assert!(note.contains("\"evidence_window\""));
}
