use std::fs;

use clap::Parser as _;

use super::{
    KNOWN_CADENCE_MINUTES, StudyManager, autocorrelation, ensure_metric_available, metric_value,
    parse_study, stats::correlation, valid_study_id,
};
use crate::{
    config::Config, inquiry::PendingActivity, reservoir::ReservoirSnapshot,
    trace::IpcTraceContextV1,
};
use uuid::Uuid;

fn fresh_snapshot(recorded_at_unix_ms: u64, sequence: u64) -> ReservoirSnapshot {
    ReservoirSnapshot {
        generation_id: "study-test-generation".to_string(),
        sequence,
        recorded_at_unix_ms,
        ..ReservoirSnapshot::default()
    }
}

#[test]
fn parses_bounded_study_grammar() {
    let study = parse_study(
        "audio_rms WITH artifact_rate OVER 6h :: Does acoustic energy precede artifacts?",
    )
    .unwrap();
    assert_eq!(study.primary_metric, "audio_rms");
    assert_eq!(study.secondary_metric.as_deref(), Some("artifact_rate"));
    assert_eq!(study.duration_hours, 6);
    assert!(parse_study("fill OVER 2h :: invalid duration").is_none());
    assert!(parse_study("unknown OVER 1h :: invalid metric").is_none());
    assert!(parse_study("fill WITH fill OVER 1h :: duplicate").is_none());
}

#[test]
fn spectral_metrics_are_canonical_and_explicitly_unavailable() {
    let study = parse_study(
        "spectrum_entropy WITH spectral_mode_turnover OVER 6h :: Does the spectrum reorganize?",
    )
    .unwrap();
    assert_eq!(study.primary_metric, "spectral_entropy");
    assert_eq!(study.secondary_metric.as_deref(), Some("mode_turnover"));

    let unavailable = ReservoirSnapshot::default();
    assert!(ensure_metric_available(&unavailable, "spectral_entropy").is_err());
    assert!(ensure_metric_available(&unavailable, "lambda1_share").is_err());

    let available = ReservoirSnapshot {
        spectral_entropy: Some(0.71),
        lambda1_share: Some(0.19),
        tail_share: Some(0.23),
        density_gradient: Some(-0.04),
        mode_turnover: Some(0.12),
        ..ReservoirSnapshot::default()
    };
    let config = Config::try_parse_from(["edge"]).unwrap();
    let pending = PendingActivity::default();
    for (metric, expected) in [
        ("spectral_entropy", 0.71),
        ("lambda1_share", 0.19),
        ("tail_share", 0.23),
        ("density_gradient", -0.04),
        ("mode_turnover", 0.12),
    ] {
        ensure_metric_available(&available, metric).unwrap();
        assert_eq!(
            metric_value(&config, &available, &pending, metric, 0),
            Some(expected)
        );
    }
}

#[test]
fn validates_study_identifiers() {
    assert!(valid_study_id("study_1234_deadbeef").is_some());
    assert!(valid_study_id("../../study_1234").is_none());
    assert!(valid_study_id("study_1234-deadbeef").is_none());
}

#[test]
fn deterministic_statistics_handle_constant_and_periodic_values() {
    assert_eq!(correlation(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]), None);
    let values = (0..120)
        .map(|index| if index % 5 == 0 { 1.0 } else { 0.0 })
        .collect::<Vec<_>>();
    assert!(autocorrelation(&values, 5).unwrap() > 0.99);
    assert_eq!(KNOWN_CADENCE_MINUTES, [3, 5, 10, 15, 60]);
}

#[test]
fn study_survives_reload_completes_without_inference_and_can_be_cancelled() {
    let workspace = std::env::temp_dir().join(format!(
        "astrid-edge-study-lifecycle-{}",
        super::unix_millis()
    ));
    let mut config = Config::try_parse_from(["edge"]).unwrap();
    config.workspace.clone_from(&workspace);
    config.prepare_workspace().unwrap();
    let spec = parse_study("fill OVER 1h :: Is the deterministic shelf stable?").unwrap();
    let started = 1_900_000_000_000_u64;
    let snapshot = fresh_snapshot(started, 1);
    let mut manager = StudyManager::load(&config);
    let definition = manager
        .start(
            &config,
            &snapshot,
            started,
            &spec,
            None,
            None,
            "operator_harness",
        )
        .unwrap();
    assert!(definition.starts_with("home://edge/studies/definitions/study_"));
    assert!(manager.tick(&config, &snapshot, started).unwrap().is_none());

    let mut reloaded = StudyManager::load(&config);
    let midpoint_snapshot = fresh_snapshot(started + 30 * 60_000, 2);
    assert!(
        reloaded
            .tick(&config, &midpoint_snapshot, started + 30 * 60_000)
            .unwrap()
            .is_none()
    );
    let completion_snapshot = fresh_snapshot(started + 60 * 60_000, 3);
    let completion = reloaded
        .tick(&config, &completion_snapshot, started + 60 * 60_000)
        .unwrap()
        .unwrap();
    let result = fs::read_to_string(
        workspace
            .join("studies/results")
            .join(completion.artifact_basename),
    )
    .unwrap();
    assert!(result.contains("deterministic descriptive measurements"));
    assert!(result.contains("do not establish causation"));

    let second = parse_study("fill OVER 3h :: Should this study be cancelled?").unwrap();
    let second_definition = reloaded
        .start(
            &config,
            &snapshot,
            started + 61 * 60_000,
            &second,
            None,
            None,
            "astrid_action",
        )
        .unwrap();
    let study_id = std::path::Path::new(&second_definition)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap();
    reloaded
        .cancel(&config, started + 62 * 60_000, study_id, None, None)
        .unwrap();
    assert!(reloaded.registry.active.is_none());
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn study_completion_preserves_only_exact_declaring_identity() {
    let workspace =
        std::env::temp_dir().join(format!("astrid-edge-study-trace-{}", super::unix_millis()));
    let mut config = Config::try_parse_from(["edge"]).unwrap();
    config.workspace.clone_from(&workspace);
    config.prepare_workspace().unwrap();
    let spec = parse_study("fill OVER 1h :: Is the shelf stable?").unwrap();
    let started = 1_900_100_000_000_u64;
    let snapshot = fresh_snapshot(started + 60 * 60_000, 1);
    let trace = IpcTraceContextV1::root(
        Uuid::new_v4(),
        "session-exact".to_string(),
        Some("chain-exact".to_string()),
    );
    let response_sha256 = "a".repeat(64);
    let mut manager = StudyManager::load(&config);
    manager
        .start(
            &config,
            &snapshot,
            started,
            &spec,
            Some(&trace),
            Some(&response_sha256),
            "astrid_action",
        )
        .unwrap();
    let completion = manager
        .tick(&config, &snapshot, started + 60 * 60_000)
        .unwrap()
        .unwrap();
    let completion_trace = completion.trace.unwrap();
    assert_eq!(completion_trace.trace_id, trace.trace_id);
    assert_eq!(completion_trace.session_id, trace.session_id);
    assert_eq!(completion_trace.chain_id, trace.chain_id);
    assert_eq!(
        completion.parent_response_sha256.as_deref(),
        Some(response_sha256.as_str())
    );
    fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn study_excludes_duplicate_and_stale_reservoir_snapshots() {
    let workspace = std::env::temp_dir().join(format!(
        "astrid-edge-study-freshness-{}",
        super::unix_millis()
    ));
    let mut config = Config::try_parse_from(["edge"]).unwrap();
    config.workspace.clone_from(&workspace);
    config.prepare_workspace().unwrap();
    let started = 1_900_200_000_000_u64;
    let spec = parse_study("fill OVER 1h :: Are samples physically distinct?").unwrap();
    let first = fresh_snapshot(started, 1);
    let mut manager = StudyManager::load(&config);
    manager
        .start(
            &config,
            &first,
            started,
            &spec,
            None,
            None,
            "operator_harness",
        )
        .unwrap();
    assert!(manager.tick(&config, &first, started).unwrap().is_none());

    assert!(
        manager
            .tick(&config, &first, started + 60_000)
            .unwrap()
            .is_none()
    );
    let active = manager.registry.active.as_ref().unwrap();
    assert_eq!(active.sample_count, 1);
    assert_eq!(active.stale_snapshot_tick_count, 1);

    let second = fresh_snapshot(started + 120_000, 2);
    assert!(
        manager
            .tick(&config, &second, started + 120_000)
            .unwrap()
            .is_none()
    );
    let active = manager.registry.active.as_ref().unwrap();
    assert_eq!(active.sample_count, 2);
    assert_eq!(active.stale_snapshot_tick_count, 1);
    fs::remove_dir_all(workspace).unwrap();
}
