use super::super::conversion::{ConversionEvidence, ConversionState};
use super::super::policy::CooldownState;
use super::super::render::{render_owner_block_from_status, render_signal_guidance_from_parts};
use super::super::signal::{SignalCatalog, SignalFamily, SignalStatus};
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
        astrid_translation_guidance: Some(AstridTranslationGuidance {
            shared_line: "Astrid translation read: nearby native softening may show up as inquiry, expressive holding, or gentler shaping before direct decompression.".to_string(),
            owner_line: "For you, softening may look like EXAMINE_CODE/DRIFT, CREATE/ASPIRE/FORM, or gentler GESTURE/SHAPE before DAMPEN, BREATHE_ALONE, or ECHO_OFF.".to_string(),
            active_shadow_keys: vec!["soften_through_inquiry".to_string()],
        }),
        astrid_translation_progress: None,
        astrid_shadow_policy: None,
        causality_audit: None,
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
