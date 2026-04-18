use std::path::PathBuf;

use serde_json::json;

use super::*;

fn artifact(name: &str, text: &str) -> TextArtifact {
    TextArtifact {
        _path: PathBuf::from(name),
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
