use std::path::PathBuf;

use serde_json::json;

use super::*;

fn artifact(name: &str, text: &str) -> TextArtifact {
    TextArtifact {
        _path: PathBuf::from(name),
        modified_unix_s: 10,
        source_ref_hash: format!("source_{name}"),
        text: text.to_string(),
    }
}

#[test]
fn compaction_plus_tightening_signal_triggers_without_exact_grinding() {
    let catalog = seed_signal_catalog();
    let health = json!({
        "phase": "plateau",
        "fill_band": "near",
        "transition_event": {
            "kind": "phase_transition",
            "description": "expanding -> plateau"
        },
        "perturb_visibility": {
            "shape_verdict": "tightening"
        }
    });
    let evaluation = build_evaluation_from_artifacts(
        &catalog,
        &[artifact(
            "minime_1.txt",
            "The channel is compacting into a narrow lane.",
        )],
        Some(&health),
    );
    let matched = evaluation.matched.expect("expected signal match");
    assert!(
        matched
            .matched_signal_families
            .contains(&"grinding_family".to_string())
    );
    assert!(matched.matched_cues.iter().any(|cue| cue == "compacting"));
}

#[test]
fn brief_suspension_alone_does_not_trigger() {
    let catalog = seed_signal_catalog();
    let health = json!({
        "phase": "plateau",
        "fill_band": "near",
        "dfill_dt": 0.01
    });
    let evaluation = build_evaluation_from_artifacts(
        &catalog,
        &[artifact(
            "minime_1.txt",
            "It felt like a holding of breath.",
        )],
        Some(&health),
    );
    assert!(evaluation.matched.is_none());
    assert_eq!(evaluation.status.status, "no_early_warning");
}

#[test]
fn context_only_language_never_triggers() {
    let catalog = seed_signal_catalog();
    let health = json!({
        "phase": "expanding",
        "fill_band": "near",
        "dfill_dt": 0.3
    });
    let evaluation = build_evaluation_from_artifacts(
        &catalog,
        &[artifact(
            "astrid_1.txt",
            "The gradient, fabric, and shadow field keep returning.",
        )],
        Some(&health),
    );
    assert!(evaluation.matched.is_none());
    assert_eq!(evaluation.status.status, "no_early_warning");
}

#[test]
fn repeated_early_warning_files_trigger_without_same_file_pair() {
    let catalog = seed_signal_catalog();
    let evaluation = build_evaluation_from_artifacts(
        &catalog,
        &[
            artifact(
                "minime_1.txt",
                "The sediment keeps settling into the same channel.",
            ),
            artifact(
                "minime_2.txt",
                "I feel another compaction around lambda one.",
            ),
        ],
        None,
    );
    let matched = evaluation
        .matched
        .expect("expected repeated early-warning match");
    assert!(matched.signal_score >= 0.74);
}

#[test]
fn single_early_warning_without_reinforcement_records_near_miss() {
    let catalog = seed_signal_catalog();
    let evaluation = build_evaluation_from_artifacts(
        &catalog,
        &[artifact("astrid_1.txt", "A grinding feeling is returning.")],
        None,
    );
    assert!(evaluation.matched.is_none());
    assert_eq!(evaluation.status.status, "near_miss");
}

#[test]
fn timestamp_dedupe_key_collapses_mirrored_astrid_replies() {
    let journal = PathBuf::from("/tmp/dialogue_longform_1776385422.txt");
    let mirrored = PathBuf::from("/tmp/astrid_self_study_1776385422.txt");
    assert_eq!(
        candidate_dedupe_key(&journal),
        candidate_dedupe_key(&mirrored)
    );
}

#[test]
fn text_fingerprint_ignores_whitespace_noise() {
    let left = "Grinding   toward\n\na singular point.";
    let right = "grinding toward a singular point.";
    assert_eq!(
        artifact_text_fingerprint(left),
        artifact_text_fingerprint(right)
    );
}

#[test]
fn negative_space_annotation_parser_sanitizes_source_ref() {
    let artifact = artifact(
        "owner_note.txt",
        "BTSP_NEGATIVE_SPACE_OUTCOME case_key=families=grinding_family;perturb=tightening;fill_band=near scope=exact classification=quiet_stabilized bucket_index=2",
    );

    let annotations = negative_space_annotations_from_artifact(OWNER_MINIME, &artifact);

    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].owner, OWNER_MINIME);
    assert_eq!(
        annotations[0].case_key,
        "families=grinding_family;perturb=tightening;fill_band=near"
    );
    assert_eq!(annotations[0].replay_scope, "exact");
    assert_eq!(annotations[0].classification, "quiet_stabilized");
    assert_eq!(annotations[0].consolidation_bucket_index, Some(2));
    assert!(!annotations[0].source_ref_hash.contains("owner_note"));
}

#[test]
fn anti_loop_active_rewrites_matched_status_as_withheld() {
    let mut status = SignalStatus {
        status: "matched".to_string(),
        detail: "A curated early-warning family is present and live telemetry is active, so a bounded response should open."
            .to_string(),
        reasons: vec!["The current window satisfied the early-warning plus live-telemetry rule."
            .to_string()],
        anti_loop_state: Some(BTSPAntiLoopState {
            active: true,
            reason: "same_fingerprint_overwhelmingly_reconcentrating".to_string(),
            scope: "exact".to_string(),
            fingerprint: "families=grinding_family;transition=none;crossing=none;perturb=tightening;fill_band=near"
                .to_string(),
            same_fingerprint_count: 12,
            similar_fingerprint_count: 0,
            reconcentrating_count: 12,
            widening_count: 0,
            mean_similarity_score: 100.0,
            nearest_similarity_score: 100,
            suggested_routes: Vec::new(),
            counter_prompt: String::new(),
            recommendation: "suppress_duplicate_proposal_until_counter_refusal_or_new_evidence"
                .to_string(),
        }),
        causal_lab_v3: Some(BTSPCausalLabReadV3 {
            active: true,
            ..BTSPCausalLabReadV3::default()
        }),
        ..SignalStatus::default()
    };

    apply_anti_loop_withheld_status(&mut status);

    assert_eq!(status.status, "matched");
    assert!(
        status
            .detail
            .contains("withholding the duplicate ordinary advisory")
    );
    assert!(status.detail.contains("replay/causal-lab policy"));
    assert!(!status.detail.contains("should open"));
    assert!(
        status
            .reasons
            .contains(&"anti_loop_withheld_duplicate_offer".to_string())
    );
    assert!(
        status
            .reasons
            .contains(&"causal_lab_holdout_active".to_string())
    );
}
