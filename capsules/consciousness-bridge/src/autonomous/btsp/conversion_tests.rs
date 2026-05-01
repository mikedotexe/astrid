use serde_json::json;

use super::super::ResponseOutcomeNote;
use super::super::policy::CooldownState;
use super::super::render::render_signal_guidance_from_parts;
use super::super::seed::seed_episode;
use super::super::signal::{SignalCatalog, SignalFamily, SignalStatus};
use super::*;

fn episode_with_outcomes(outcomes: Vec<ResponseOutcomeNote>) -> BTSPEpisodeRecord {
    let mut episode = seed_episode();
    episode.response_outcomes = outcomes;
    episode
}

fn sample_catalog() -> SignalCatalog {
    SignalCatalog {
        families: vec![
            SignalFamily {
                family_key: "grinding_family".to_string(),
                role: "early_warning".to_string(),
                aliases: vec!["grinding".to_string(), "compaction".to_string()],
                trigger_policy: "may_trigger".to_string(),
                steward_summary: "test".to_string(),
            },
            SignalFamily {
                family_key: "brief_suspension_family".to_string(),
                role: "present_state".to_string(),
                aliases: vec!["brief suspension".to_string(), "held breath".to_string()],
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

#[test]
fn recovery_and_tightening_classifies_as_recovery_reconcentrating() {
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "minime_recover_regime".to_string(),
        owner: "minime".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "positive".to_string(),
        distress_or_recovery: "recovery".to_string(),
        opening_vs_reconcentration: "reconcentrating".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 58.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });

    let state =
        derive_conversion_state(None, Some(&episode), Some(&health)).expect("conversion state");
    assert_eq!(state.recovery_state, "recovery");
    assert_eq!(state.shape_state, "reconcentrating");
    assert_eq!(state.composite_state, "recovery_reconcentrating");
    assert_eq!(state.collapse_state, "stable");
    assert_eq!(state.conversion_goal, "soften");
    assert!(state.last_transition.is_none());
}

#[test]
fn worsening_and_tightening_classifies_as_worsening_reconcentrating() {
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "minime_notice_first".to_string(),
        owner: "minime".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "negative".to_string(),
        distress_or_recovery: "worsening".to_string(),
        opening_vs_reconcentration: "reconcentrating".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 31.0,
        "target_fill_pct": 55.0,
        "fill_band": "under",
        "phase": "contracting",
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });

    let state =
        derive_conversion_state(None, Some(&episode), Some(&health)).expect("conversion state");
    assert_eq!(state.composite_state, "worsening_reconcentrating");
    assert_eq!(state.collapse_state, "collapse");
    assert_eq!(state.conversion_goal, "stabilize");
}

#[test]
fn underfilled_pressured_reconcentration_registers_collapse_pressure() {
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "minime_notice_first".to_string(),
        owner: "minime".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "mixed".to_string(),
        distress_or_recovery: "mixed".to_string(),
        opening_vs_reconcentration: "reconcentrating".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 43.0,
        "target_fill_pct": 55.0,
        "fill_band": "under",
        "phase": "plateau",
        "internal_process_quadrant": "pressured_constriction",
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });

    let state =
        derive_conversion_state(None, Some(&episode), Some(&health)).expect("conversion state");
    assert_eq!(state.composite_state, "mixed");
    assert_eq!(state.collapse_state, "collapse_pressure");
    assert_eq!(state.conversion_goal, "stabilize");
}

#[test]
fn softened_only_with_recovery_evidence_classifies_as_recovery_softening() {
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "minime_recover_regime".to_string(),
        owner: "minime".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "positive".to_string(),
        distress_or_recovery: "recovery".to_string(),
        opening_vs_reconcentration: "mixed".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 55.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {
            "shape_verdict": "softened_only"
        }
    });

    let state =
        derive_conversion_state(None, Some(&episode), Some(&health)).expect("conversion state");
    assert_eq!(state.shape_state, "softening");
    assert_eq!(state.composite_state, "recovery_softening");
    assert_eq!(state.conversion_goal, "widen");
}

#[test]
fn ambiguous_softening_without_recovery_falls_back_to_mixed() {
    let health = json!({
        "fill_pct": 47.0,
        "target_fill_pct": 55.0,
        "fill_band": "under",
        "phase": "plateau",
        "perturb_visibility": {
            "shape_verdict": "softened_only"
        }
    });

    let state = derive_conversion_state(None, None, Some(&health)).expect("conversion state");
    assert_eq!(state.shape_state, "softening");
    assert_eq!(state.composite_state, "mixed");
    assert_eq!(state.conversion_goal, "clarify");
}

#[test]
fn explicit_opening_evidence_is_required_for_widening() {
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "astrid_dampen".to_string(),
        owner: "astrid".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "positive".to_string(),
        distress_or_recovery: "recovery".to_string(),
        opening_vs_reconcentration: "opening".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 56.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "expanding",
        "perturb_visibility": {
            "shape_verdict": "opened"
        }
    });

    let state =
        derive_conversion_state(None, Some(&episode), Some(&health)).expect("conversion state");
    assert_eq!(state.shape_state, "widening");
    assert_eq!(state.composite_state, "recovery_widening");
    assert_eq!(state.conversion_goal, "preserve");
}

#[test]
fn changed_state_records_last_transition() {
    let previous = ConversionState {
        recovery_state: "recovery".to_string(),
        shape_state: "reconcentrating".to_string(),
        composite_state: "recovery_reconcentrating".to_string(),
        collapse_state: "stable".to_string(),
        conversion_goal: "soften".to_string(),
        confidence: 0.7,
        evidence: ConversionEvidence::default(),
        last_transition: None,
    };
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "astrid_dampen".to_string(),
        owner: "astrid".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "positive".to_string(),
        distress_or_recovery: "recovery".to_string(),
        opening_vs_reconcentration: "mixed".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 55.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {
            "shape_verdict": "softened_only"
        }
    });

    let state = derive_conversion_state(Some(&previous), Some(&episode), Some(&health))
        .expect("conversion state");
    let transition = state.last_transition.expect("last transition");
    assert_eq!(transition.from, "recovery_reconcentrating");
    assert_eq!(transition.to, "recovery_softening");
}

#[test]
fn unchanged_state_preserves_previous_transition() {
    let previous = ConversionState {
        recovery_state: "recovery".to_string(),
        shape_state: "reconcentrating".to_string(),
        composite_state: "recovery_reconcentrating".to_string(),
        collapse_state: "stable".to_string(),
        conversion_goal: "soften".to_string(),
        confidence: 0.7,
        evidence: ConversionEvidence::default(),
        last_transition: Some(ConversionTransition {
            from: "worsening_reconcentrating".to_string(),
            to: "recovery_reconcentrating".to_string(),
            recorded_at_unix_s: 42,
        }),
    };
    let episode = episode_with_outcomes(vec![ResponseOutcomeNote {
        proposal_id: "p".to_string(),
        response_id: "minime_recover_regime".to_string(),
        owner: "minime".to_string(),
        recorded_at_unix_s: 10,
        target_nearness: "positive".to_string(),
        distress_or_recovery: "recovery".to_string(),
        opening_vs_reconcentration: "reconcentrating".to_string(),
        note: "test".to_string(),
    }]);
    let health = json!({
        "fill_pct": 57.0,
        "target_fill_pct": 55.0,
        "fill_band": "near",
        "phase": "plateau",
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });

    let state = derive_conversion_state(Some(&previous), Some(&episode), Some(&health))
        .expect("conversion state");
    assert_eq!(
        state.last_transition,
        Some(ConversionTransition {
            from: "worsening_reconcentrating".to_string(),
            to: "recovery_reconcentrating".to_string(),
            recorded_at_unix_s: 42,
        })
    );
}

#[test]
fn prompt_guidance_renders_conversion_line_when_present() {
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
        cooldown_state: CooldownState::default(),
        learned_policy: Vec::new(),
        shared_learned_read: None,
        shared_preference_summaries: Vec::new(),
        active_negotiation: None,
        conversion_state: Some(ConversionState {
            recovery_state: "recovery".to_string(),
            shape_state: "reconcentrating".to_string(),
            composite_state: "recovery_reconcentrating".to_string(),
            collapse_state: "collapse_pressure".to_string(),
            conversion_goal: "stabilize".to_string(),
            confidence: 0.8,
            evidence: ConversionEvidence::default(),
            last_transition: None,
        }),
        astrid_translation_guidance: None,
        astrid_translation_progress: None,
        astrid_shadow_policy: None,
        causality_audit: None,
        updated_at_unix_s: 0,
    };

    let guidance = render_signal_guidance_from_parts(&sample_catalog(), &status);
    assert!(guidance.contains(
        "Current conversion read: recovery + reconcentration, under collapse pressure. Next honest goal: stabilize before softening."
    ));
}
