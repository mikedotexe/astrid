use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::json;

use super::*;

fn temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astrid_lived_state_{label}_{}_{}",
        std::process::id(),
        rand::random::<u64>()
    ))
}

fn test_witness(artifact_bytes: &[u8]) -> TemporalLivedStateWitnessV1 {
    let authorship = begin_authorship_v1(None, &[], "introspection");
    let startup = identity::snapshot();
    let process_provenance = ProvenanceRefV1::new(
        ProvenanceOriginV1::BridgeDerived,
        "process:test".to_string(),
        startup.process.process_identity_sha256().to_string(),
        Vec::new(),
        authorship.authored_at.unix_ms,
        vec!["observed_process_v1".to_string()],
        vec![ProvenanceInfluenceTypeV1::Temporal],
    );
    TemporalLivedStateWitnessV1::new(
        authorship.witness_id,
        "introspection".to_string(),
        "introspection_test.txt".to_string(),
        sha256_bytes(artifact_bytes),
        authorship.authored_at.unix_ms,
        authorship.authored_at.monotonic_ns,
        None,
        startup.process,
        startup.build_candidate,
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        process_provenance,
    )
}

#[test]
fn startup_build_candidate_is_an_observation_not_deployment_proof() {
    let root = temp_root("candidate");
    fs::create_dir_all(&root).expect("temp root");
    let path = root.join("manifest.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "repository": {
                "dirty": true,
                "dirty_paths": ["relative/file.rs"],
                "source_identity_sha256": "a".repeat(64)
            },
            "artifacts": {"spectral-bridge": {"sha256": "b".repeat(64)}},
            "protocol": {"revision": "revision", "version": "1.1"}
        }))
        .expect("manifest JSON"),
    )
    .expect("manifest write");
    let candidate = identity::build_candidate_from_path_for_test(&path, 123).expect("candidate");
    let encoded = serde_json::to_value(&candidate).expect("serialize candidate");
    assert_eq!(encoded["deployment_established"], false);
    assert_eq!(
        encoded["relation_to_process"],
        "startup_observation_not_deployment_proof"
    );
    assert!(
        !encoded
            .to_string()
            .contains(root.to_string_lossy().as_ref())
    );
    fs::write(&path, br#"{"repository":{"dirty":false}}"#).expect("mutated manifest write");
    assert_eq!(
        serde_json::to_value(&candidate).expect("cached candidate serialization"),
        encoded,
        "a later build candidate must not rewrite startup process identity"
    );
    assert_ne!(
        serde_json::to_value(
            identity::build_candidate_from_path_for_test(&path, 456).expect("new candidate")
        )
        .expect("new candidate serialization"),
        encoded
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_snapshot_redacts_absolute_paths_and_hashes_exact_window() {
    let sample = clock_sample_v1();
    let path = bridge_paths().bridge_root().join("src/lib.rs");
    let snapshot = source_snapshot_v1(&path, "whole file", "viewed window", 2, 4, 9, sample);
    let encoded = serde_json::to_value(snapshot).expect("snapshot serialization");
    assert_eq!(encoded["window_sha256"], sha256_bytes(b"viewed window"));
    assert!(!encoded.to_string().contains("/Users/"));
    assert_eq!(encoded["private_path_included"], false);
}

#[test]
fn owner_only_sidecar_contains_no_prose() {
    let root = temp_root("owner_only");
    let source = source_snapshot_v1(
        &bridge_paths().bridge_root().join("src/lib.rs"),
        "private source prose",
        "private viewed prose",
        0,
        1,
        1,
        clock_sample_v1(),
    );
    let authorship = begin_authorship_v1(Some(&source), &[], "introspection");
    let startup = identity::snapshot();
    let process_provenance = ProvenanceRefV1::new(
        ProvenanceOriginV1::BridgeDerived,
        "process:test".to_string(),
        startup.process.process_identity_sha256().to_string(),
        Vec::new(),
        authorship.authored_at.unix_ms,
        vec!["observed_process_v1".to_string()],
        vec![ProvenanceInfluenceTypeV1::Temporal],
    );
    let witness = TemporalLivedStateWitnessV1::new(
        authorship.witness_id.clone(),
        "introspection".to_string(),
        "introspection_test.txt".to_string(),
        sha256_bytes(b"canonical report prose"),
        authorship.authored_at.unix_ms,
        authorship.authored_at.monotonic_ns,
        Some(source),
        startup.process,
        startup.build_candidate,
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        process_provenance,
    );
    writer::write_witness_for_test(&root, witness).expect("witness write");
    let path = root
        .join("witnesses")
        .join(format!("{}.json", authorship.witness_id));
    let raw = fs::read_to_string(&path).expect("sidecar read");
    assert!(!raw.contains("private source prose"));
    assert!(!raw.contains("private viewed prose"));
    assert!(!raw.contains("canonical report prose"));
    assert!(raw.contains("receipt_artifact_handling_only"));
    assert!(raw.contains("primary_actionable_evidence"));
    assert!(raw.contains("not_adjudicated_by_this_receipt"));
    assert!(raw.contains("reported_not_mechanistically_attributed"));
    assert!(raw.contains("preserved_in_canonical_report_no_scalar_substitution"));
    assert!(raw.contains("\"epistemic_posture\": \"non_adjudicating\""));
    assert!(raw.contains("\"live_control_effect\": false"));
    assert_eq!(
        fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(root.join("witnesses"))
            .expect("dir metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&root)
            .expect("root metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_sidecars_are_write_once_and_idempotent() {
    let root = temp_root("write_once");
    let path = root.join("witnesses/immutable.json");
    writer::atomic_owner_write(&path, b"same\n").expect("first publish");
    writer::atomic_owner_write(&path, b"same\n").expect("idempotent publish");
    let error = writer::atomic_owner_write(&path, b"changed\n")
        .expect_err("different bytes must not replace an immutable sidecar");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&path).expect("published bytes"), b"same\n");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_route_records_hashes_without_prompt_or_response() {
    let route = model_route_v1(
        Some("job_test".to_string()),
        Some("c".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        "production",
        10,
        25,
        None,
        "private response prose",
    );
    let encoded = serde_json::to_string(&route).expect("route JSON");
    assert!(!encoded.contains("private response prose"));
    assert!(encoded.contains("response_sha256"));
    assert!(encoded.contains("qos_request_identity_sha256"));
    assert!(encoded.contains("request_content_anchor_sha256"));
    assert!(encoded.contains("model_call_event_not_being_or_continuity_identity"));
    assert!(encoded.contains("output_integrity_not_being_or_continuity_identity"));
}

#[test]
fn model_route_identity_hashes_the_bounded_persisted_profile() {
    let route = model_route_v1(
        Some("job_test".to_string()),
        Some("c".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        &"profile".repeat(80),
        10,
        25,
        None,
        "private response prose",
    );
    let encoded = serde_json::to_value(route).expect("route JSON");
    let profile = encoded["model_profile"].as_str().expect("profile");
    assert!(profile.len() <= 160);
    let mut hasher = Sha256::new();
    hasher.update(b"astrid-lived-state-model-route-v1\0");
    hasher.update(b"job_test");
    hasher.update("c".repeat(64).as_bytes());
    hasher.update(b"mlx");
    hasher.update(profile.as_bytes());
    hasher.update(10_u64.to_le_bytes());
    hasher.update(
        encoded["response_sha256"]
            .as_str()
            .expect("response hash")
            .as_bytes(),
    );
    assert_eq!(
        encoded["call_id"],
        format!("lscall_{:x}", hasher.finalize())
    );
}

#[test]
fn repair_route_preserves_only_hashed_parent_ancestry() {
    let first = model_route_v1(
        Some("job_first".to_string()),
        Some("d".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        "production",
        10,
        20,
        None,
        "first private response",
    );
    let repair = model_route_v1(
        Some("job_repair".to_string()),
        Some("e".repeat(64)),
        Some("b".repeat(64)),
        "mlx",
        "production",
        21,
        30,
        Some(first.call_id().to_string()),
        "repair private response",
    );
    let encoded = serde_json::to_value([first, repair]).expect("routes JSON");
    assert_eq!(encoded[1]["repair_parent_call_id"], encoded[0]["call_id"]);
    assert!(!encoded.to_string().contains("private response"));
}

#[test]
fn response_changes_do_not_change_the_request_content_anchor_or_claim_continuity() {
    let first = model_route_v1(
        Some("job_first".to_string()),
        Some("d".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        "production",
        10,
        20,
        None,
        "same meaning.",
    );
    let punctuation_variant = model_route_v1(
        Some("job_second".to_string()),
        Some("e".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        "production",
        21,
        30,
        None,
        "same meaning!",
    );
    let encoded = serde_json::to_value([first, punctuation_variant]).expect("routes JSON");
    assert_ne!(encoded[0]["call_id"], encoded[1]["call_id"]);
    assert_eq!(
        encoded[0]["request_content_anchor_sha256"],
        encoded[1]["request_content_anchor_sha256"]
    );
    for route in encoded.as_array().expect("route array") {
        assert_eq!(route["being_identity_claimed"], false);
        assert_eq!(route["continuity_claimed"], false);
        assert_eq!(route["intent_equivalence_claimed"], false);
        assert_eq!(route["semantic_equivalence_claimed"], false);
    }
}

#[test]
fn stale_or_absent_peer_state_is_unknown_not_active_inference() {
    let value = json!({
        "fill_pct": 68.0,
        "spectral_entropy": 0.75,
        "structural_entropy": 0.5
    });
    let stale =
        peer_scalar_observations_from_snapshot(Some(&value), Some(MAX_PEER_STATE_AGE_MS + 1), 100);
    let stale_json = serde_json::to_value(stale).expect("stale observations");
    assert!(stale_json.as_array().expect("array").iter().all(|row| {
        row["observation_kind"] == "unknown" && row["value"].is_null() && row["fresh"] == false
    }));
    let absent = peer_scalar_observations_from_snapshot(None, None, 100);
    let absent_json = serde_json::to_value(absent).expect("absent observations");
    assert!(
        absent_json
            .as_array()
            .expect("array")
            .iter()
            .all(|row| { row["observation_kind"] == "unknown" && row["value"].is_null() })
    );
}

#[test]
fn bounded_writer_reports_saturation_without_waiting() {
    let root = temp_root("saturation");
    let results = writer::bounded_submit_probe_for_test(&root, test_witness(b"report"), 1, 3);
    assert_eq!(results[0].0, WitnessSubmitResultV1::Accepted);
    assert_eq!(results[1].0, WitnessSubmitResultV1::QueueFull);
    assert_eq!(results[2].0, WitnessSubmitResultV1::QueueFull);
}

#[test]
fn witness_enqueue_is_below_one_millisecond_p95() {
    let root = temp_root("latency");
    let results = writer::bounded_submit_probe_for_test(&root, test_witness(b"report"), 128, 100);
    assert!(
        results
            .iter()
            .all(|(result, _)| *result == WitnessSubmitResultV1::Accepted)
    );
    let mut durations: Vec<u128> = results
        .into_iter()
        .map(|(_, elapsed_ns)| elapsed_ns)
        .collect();
    durations.sort_unstable();
    let p95 = durations[94];
    assert!(p95 < 1_000_000, "enqueue p95 was {p95}ns");
}
