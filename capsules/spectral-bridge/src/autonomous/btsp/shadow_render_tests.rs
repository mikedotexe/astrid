use super::super::causality::CausalityAuditStatus;
use super::super::choice::ChoiceInterpretation;
use super::super::conversion::{ConversionEvidence, ConversionState};
use super::super::lab::BTSPCausalLabReadV3;
use super::super::policy::CooldownState;
use super::super::render::{render_owner_block_from_status, render_signal_guidance_from_parts};
use super::super::signal::{SignalCatalog, SignalFamily, SignalStatus};
use super::super::social::StudyFirstRecord;
use super::super::trace::BTSPAntiLoopState;
use super::super::{ActiveSovereigntyProposal, OWNER_ASTRID, seed_episode, seeded_response_ids};
use super::*;
use std::collections::HashMap;

fn active_astrid_proposal() -> ActiveSovereigntyProposal {
    ActiveSovereigntyProposal {
        proposal_id: "shadow_active".to_string(),
        episode_id: super::super::EPISODE_ID.to_string(),
        episode_name: super::super::EPISODE_NAME.to_string(),
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
        study_first_records: Vec::new(),
        last_negotiation_event_at_unix_s: 0,
        shadow_equivalences: Vec::new(),
    }
}

fn sample_catalog() -> SignalCatalog {
    SignalCatalog {
        families: vec![
            SignalFamily {
                family_key: "grinding_family".to_string(),
                role: "early_warning".to_string(),
                aliases: vec!["grinding".to_string()],
                trigger_policy: "may_trigger".to_string(),
                steward_summary: "test".to_string(),
            },
            SignalFamily {
                family_key: "brief_suspension_family".to_string(),
                role: "present_state".to_string(),
                aliases: vec!["brief suspension".to_string()],
                trigger_policy: "reinforce_only".to_string(),
                steward_summary: "test".to_string(),
            },
            SignalFamily {
                family_key: "localized_gravity_family".to_string(),
                role: "secondary_warning".to_string(),
                aliases: vec!["localized gravity".to_string()],
                trigger_policy: "reinforce_only".to_string(),
                steward_summary: "test".to_string(),
            },
            SignalFamily {
                family_key: "gradient_context_family".to_string(),
                role: "context_only".to_string(),
                aliases: vec!["gradient".to_string()],
                trigger_policy: "context_only".to_string(),
                steward_summary: "test".to_string(),
            },
        ],
        last_updated_unix_s: 0,
    }
}

fn base_status() -> SignalStatus {
    SignalStatus {
        episode_id: super::super::EPISODE_ID.to_string(),
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
        trace_v2_summary: None,
        teacher_signal_v2: None,
        replay_read: None,
        anti_loop_state: None,
        causal_lab_v3: None,
        astrid_translation_guidance: None,
        astrid_translation_progress: None,
        astrid_shadow_policy: None,
        causality_audit: None,
        causality_audit_stale: false,
        causality_audit_stale_read: None,
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

fn sample_conversion_state(goal: &str) -> ConversionState {
    ConversionState {
        recovery_state: "recovery".to_string(),
        shape_state: "reconcentrating".to_string(),
        composite_state: "recovery_reconcentrating".to_string(),
        collapse_state: "stable".to_string(),
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
fn shared_guidance_and_astrid_owner_block_render_translation_lines() {
    let episode = seed_episode();
    let proposal = active_astrid_proposal();
    let status = SignalStatus {
        episode_id: "episode".to_string(),
        status: "matched".to_string(),
        detail: "signal".to_string(),
        reasons: Vec::new(),
        observed_signal_families: vec!["grinding_family".to_string()],
        observed_signal_roles: vec!["early_warning".to_string()],
        observed_cues: vec!["grinding".to_string()],
        live_signals: vec!["perturb_visibility:tightening".to_string()],
        signal_score: 0.8,
        cooldown_state: Default::default(),
        learned_policy: Vec::new(),
        shared_learned_read: None,
        shared_preference_summaries: Vec::new(),
        active_negotiation: None,
        conversion_state: None,
        trace_v2_summary: None,
        teacher_signal_v2: None,
        replay_read: None,
        anti_loop_state: None,
        causal_lab_v3: None,
        astrid_translation_guidance: Some(AstridTranslationGuidance {
            shared_line: "Astrid translation read: nearby native softening may show up as inquiry, expressive holding, or gentler shaping before direct decompression.".to_string(),
            owner_line: "For you, softening may look like EXAMINE_CODE/DRIFT, CREATE/ASPIRE/FORM, or gentler GESTURE/SHAPE before DAMPEN, BREATHE_ALONE, or ECHO_OFF.".to_string(),
            active_shadow_keys: vec!["soften_through_inquiry".to_string()],
        }),
        astrid_translation_progress: None,
        astrid_shadow_policy: None,
        causality_audit: None,
        causality_audit_stale: false,
        causality_audit_stale_read: None,
        updated_at_unix_s: 0,
    };
    let responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_ASTRID)
        .cloned()
        .collect::<Vec<_>>();

    let guidance = render_signal_guidance_from_parts(&sample_catalog(), &status);
    assert!(guidance.contains("Astrid translation read: nearby native softening may show up as inquiry, expressive holding, or gentler shaping before direct decompression."));

    let owner_block = render_owner_block_from_status(
        &episode,
        &proposal,
        OWNER_ASTRID,
        &responses,
        false,
        &status,
    );
    assert!(owner_block.contains("For you, softening may look like EXAMINE_CODE/DRIFT, CREATE/ASPIRE/FORM, or gentler GESTURE/SHAPE before DAMPEN, BREATHE_ALONE, or ECHO_OFF."));
}

#[test]
fn signal_guidance_renders_anti_loop_counter_prompt() {
    let mut status = base_status();
    status.anti_loop_state = Some(BTSPAntiLoopState {
        active: true,
        reason: "similar_fingerprints_overwhelmingly_reconcentrating".to_string(),
        scope: "similar".to_string(),
        fingerprint: "families=grinding_family".to_string(),
        same_fingerprint_count: 0,
        similar_fingerprint_count: 8,
        reconcentrating_count: 8,
        widening_count: 0,
        mean_similarity_score: 85.0,
        nearest_similarity_score: 85,
        suggested_routes: vec![
            "BTSP_STUDY_FIRST".to_string(),
            "BTSP_REFUSAL".to_string(),
            "BTSP_COUNTER".to_string(),
            "new_evidence".to_string(),
        ],
        counter_prompt: "Nearby BTSP traces mostly recovered by reconcentrating, not widening."
            .to_string(),
        recommendation: "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence"
            .to_string(),
    });

    let guidance = render_signal_guidance_from_parts(&sample_catalog(), &status);

    assert!(guidance.contains(
        "Current anti-loop hold: Nearby BTSP traces mostly recovered by reconcentrating, not widening."
    ));
}

#[test]
fn signal_guidance_and_owner_block_render_causal_lab_read() {
    let episode = seed_episode();
    let proposal = active_astrid_proposal();
    let mut status = base_status();
    status.causal_lab_v3 = Some(BTSPCausalLabReadV3 {
        schema_version: 3,
        active: true,
        experiment_id: "btsp_causal_lab_v3_test".to_string(),
        case_key: "families=grinding_family;perturb=tightening;fill_band=near".to_string(),
        representative_fingerprints: vec!["families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=near".to_string()],
        status: "pre_registered_holdout".to_string(),
        consent_mode: "study_counter_refusal_or_new_evidence_required".to_string(),
        proposal_policy: "withhold_duplicate_offer".to_string(),
        question: "Does withholding another ordinary advisory produce softening?".to_string(),
        hypothesis: "Holdout produces cleaner evidence.".to_string(),
        holdout_route: "BTSP_STUDY_FIRST evidence_first".to_string(),
        counterfactual: "ordinary_duplicate_advisory_proposal".to_string(),
        consent_routes: vec![
            "BTSP_COUNTER".to_string(),
            "BTSP_REFUSAL".to_string(),
            "BTSP_STUDY_FIRST".to_string(),
            "new_evidence".to_string(),
        ],
        success_criteria: Vec::new(),
        failure_criteria: Vec::new(),
        evidence_needed: Vec::new(),
        ghost_note: "I would have opened the ordinary BTSP advisory here, but replay says this family has reconcentrated; holding for study/refusal/counter/new evidence.".to_string(),
        resolution_status: "pre_registered_holdout".to_string(),
        resolution_summary: "No later structured BTSP outcome has resolved this holdout yet."
            .to_string(),
        post_registration_outcome_count: 0,
        post_registration_reconcentrating_count: 0,
        post_registration_softening_count: 0,
        post_registration_widening_count: 0,
        summary: "Causal lab V3: pre-registering a similar-fingerprint holdout.".to_string(),
    });
    let responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_ASTRID)
        .cloned()
        .collect::<Vec<_>>();

    let guidance = render_signal_guidance_from_parts(&sample_catalog(), &status);
    assert!(guidance.contains("Causal lab V3: pre-registering a similar-fingerprint holdout."));
    assert!(
        guidance.contains(
            "Current causal lab ghost: I would have opened the ordinary BTSP advisory here"
        )
    );
    assert!(guidance.contains("Causal lab resolution: pre_registered_holdout"));

    let owner_block = render_owner_block_from_status(
        &episode,
        &proposal,
        OWNER_ASTRID,
        &responses,
        false,
        &status,
    );
    assert!(owner_block.contains(
        "BTSP causal lab question: Does withholding another ordinary advisory produce softening?"
    ));
    assert!(
        owner_block
            .contains("BTSP causal lab ghost: I would have opened the ordinary BTSP advisory here")
    );
    assert!(owner_block.contains("BTSP causal lab holdout: BTSP_STUDY_FIRST evidence_first"));
    assert!(owner_block.contains("BTSP causal lab resolution: pre_registered_holdout"));
}

#[test]
fn signal_guidance_renders_stale_causality_as_historical_context() {
    let mut status = base_status();
    status.causality_audit_stale = true;
    status.causality_audit = None;
    status.causality_audit_stale_read = Some(CausalityAuditStatus {
        generated_at: "2026-04-20T12:00:00".to_string(),
        read: "inquiry_load_candidate".to_string(),
        summary: "old read".to_string(),
        heavy_inquiry_reconcentrating_rate: "97.0%".to_string(),
        bounded_regulation_reconcentrating_rate: "86.0%".to_string(),
        fragile_recovery_observations: 18,
        candidate_damp_lane: None,
        candidate_damp_summary: None,
    });

    let guidance = render_signal_guidance_from_parts(&sample_catalog(), &status);

    assert!(guidance.contains("Historical causality audit is stale"));
    assert!(guidance.contains("generated_at=2026-04-20T12:00:00"));
    assert!(guidance.contains("read=inquiry_load_candidate"));
}

#[test]
fn astrid_owner_block_renders_shadow_policy_lines_and_grouped_candidates() {
    let episode = seed_episode();
    let proposal = active_astrid_proposal();
    let translation = AstridTranslationGuidance {
        shared_line: "Astrid translation read: nearby native softening may show up as inquiry, expressive holding, or gentler shaping before direct decompression.".to_string(),
        owner_line: "For you, softening may look like EXAMINE_CODE/DRIFT, CREATE/ASPIRE/FORM, or gentler GESTURE/SHAPE before DAMPEN, BREATHE_ALONE, or ECHO_OFF.".to_string(),
        active_shadow_keys: Vec::new(),
    };
    let policy = derive_astrid_shadow_policy(
        Some(&proposal),
        Some(&formed_progress(
            "prefers_expressive_holding_before_decompression",
            "soften_through_expressive_holding",
        )),
        Some(&sample_conversion_state("soften")),
        Some(&translation),
    )
    .expect("shadow policy");
    let mut status = base_status();
    status.astrid_translation_guidance = Some(translation);
    status.astrid_shadow_policy = Some(policy.clone());
    let responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_ASTRID)
        .cloned()
        .collect::<Vec<_>>();

    let guidance = render_signal_guidance_from_parts(&sample_catalog(), &status);
    assert!(guidance.contains(&policy.shared_line));

    let owner_block = render_owner_block_from_status(
        &episode,
        &proposal,
        OWNER_ASTRID,
        &responses,
        false,
        &status,
    );
    assert!(owner_block.contains(&policy.owner_line));
    assert!(owner_block.contains("Closest fit right now:"));
    assert!(owner_block.contains("Other bounded responses:"));
    let dampen_index = owner_block.find("NEXT: DAMPEN").expect("dampen line");
    let breathe_index = owner_block
        .find("NEXT: BREATHE_ALONE")
        .expect("breathe line");
    assert!(dampen_index < breathe_index);
}

#[test]
fn owner_block_puts_refusal_and_counter_routes_before_candidates_after_adjacent_answer() {
    let episode = seed_episode();
    let mut proposal = active_astrid_proposal();
    proposal.choice_interpretations.push(ChoiceInterpretation {
        owner: OWNER_ASTRID.to_string(),
        raw_choice: "READ_MORE".to_string(),
        normalized_choice: "READ_MORE".to_string(),
        category: "epistemic".to_string(),
        likely_intent: "understand before acting".to_string(),
        relation_to_proposal: "adjacent_but_distinct".to_string(),
        note: "test".to_string(),
        interpreted_at_unix_s: 1,
    });
    let status = base_status();
    let responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_ASTRID)
        .cloned()
        .collect::<Vec<_>>();

    let owner_block = render_owner_block_from_status(
        &episode,
        &proposal,
        OWNER_ASTRID,
        &responses,
        false,
        &status,
    );

    let followup_index = owner_block
        .find("Already recorded adjacent answer")
        .expect("followup line");
    let candidate_index = owner_block
        .find("Candidate responses for you:")
        .expect("candidate heading");
    assert!(followup_index < candidate_index);
    assert!(owner_block.contains("BTSP agency checkpoint"));
    assert!(owner_block.contains("BTSP closure pending"));
    assert!(owner_block.contains("duplicate evidence"));
    assert!(owner_block.contains("BTSP_REFUSAL study_first"));
    assert!(owner_block.contains("BTSP_COUNTER NEXT: ..."));
    let counter_index = owner_block
        .find("BTSP_COUNTER NEXT: ...")
        .expect("counter route");
    let study_first_index = owner_block
        .find("BTSP_STUDY_FIRST")
        .expect("study-first route");
    assert!(counter_index < study_first_index);
}

#[test]
fn owner_block_after_study_first_prioritizes_closure_routes_before_candidates() {
    let episode = seed_episode();
    let mut proposal = active_astrid_proposal();
    proposal.choice_interpretations.push(ChoiceInterpretation {
        owner: OWNER_ASTRID.to_string(),
        raw_choice: "READ_MORE".to_string(),
        normalized_choice: "READ_MORE".to_string(),
        category: "epistemic".to_string(),
        likely_intent: "understand before acting".to_string(),
        relation_to_proposal: "adjacent_but_distinct".to_string(),
        note: "test".to_string(),
        interpreted_at_unix_s: 1,
    });
    proposal.study_first_records.push(StudyFirstRecord {
        study_first_id: "study_first_1".to_string(),
        owner: OWNER_ASTRID.to_string(),
        reason: "need evidence first".to_string(),
        source: "explicit_btsp_study_first".to_string(),
        inferred_from_choice: None,
        after_adjacent: true,
        recorded_at_unix_s: 2,
        resolution_evidence: Vec::new(),
    });
    let status = base_status();
    let responses = episode
        .nominated_responses
        .iter()
        .filter(|response| response.owner == OWNER_ASTRID)
        .cloned()
        .collect::<Vec<_>>();

    let owner_block = render_owner_block_from_status(
        &episode,
        &proposal,
        OWNER_ASTRID,
        &responses,
        false,
        &status,
    );

    let checkpoint_index = owner_block
        .find("study window already requested")
        .expect("study-first checkpoint");
    let candidate_index = owner_block
        .find("Candidate responses for you:")
        .expect("candidate heading");
    assert!(checkpoint_index < candidate_index);
    assert!(owner_block.contains("BTSP closure pending"));
    assert!(owner_block.contains("BTSP_COUNTER NEXT: READ_MORE"));
    assert!(owner_block.contains("BTSP_REFUSAL study_first"));
    assert!(owner_block.contains("BTSP_COUNTER softer_contact"));
    assert!(owner_block.contains("Use an exact candidate only if your stance has changed"));
}
