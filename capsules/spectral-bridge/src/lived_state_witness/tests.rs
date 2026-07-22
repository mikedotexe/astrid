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
        authorship.authored_process_sequence,
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
        encoded["candidate_scope"],
        "artifact_context_observation_not_evaluation_of_astrid"
    );
    assert_eq!(
        encoded["integrity_scope"],
        "byte_repository_protocol_and_artifact_integrity_only"
    );
    assert_eq!(
        encoded["semantic_integrity_relation"],
        "not_measured_not_validated_and_not_inferred_from_spectral_state"
    );
    assert_eq!(
        encoded["inhabitability_relation"],
        "not_adjudicated_by_build_candidate"
    );
    assert_eq!(
        encoded["source_identity_scope"],
        "repository_source_snapshot_not_being_identity_or_continuity"
    );
    assert_eq!(
        encoded["dirty_state_scope"],
        "process_start_repository_observation_not_live_workspace_or_being_state"
    );
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
fn bounded_protocol_text_cannot_collapse_build_candidate_identity() {
    let root = temp_root("protocol_collision");
    fs::create_dir_all(&root).expect("temp root");
    let first_path = root.join("first.json");
    let second_path = root.join("second.json");
    let shared_prefix = "r".repeat(80);
    let manifest = |suffix: &str| {
        json!({
            "repository": {"dirty": false},
            "protocol": {"revision": format!("{shared_prefix}{suffix}"), "version": "1.1"}
        })
    };
    fs::write(
        &first_path,
        serde_json::to_vec(&manifest("first")).expect("first manifest"),
    )
    .expect("first write");
    fs::write(
        &second_path,
        serde_json::to_vec(&manifest("second")).expect("second manifest"),
    )
    .expect("second write");

    let first = serde_json::to_value(
        identity::build_candidate_from_path_for_test(&first_path, 123).expect("first candidate"),
    )
    .expect("first JSON");
    let second = serde_json::to_value(
        identity::build_candidate_from_path_for_test(&second_path, 123).expect("second candidate"),
    )
    .expect("second JSON");
    assert_eq!(first["protocol_revision"], second["protocol_revision"]);
    assert_eq!(first["protocol_revision_complete"], false);
    assert_eq!(second["protocol_revision_complete"], false);
    assert_eq!(first["protocol_version_complete"], true);
    assert_eq!(second["protocol_version_complete"], true);
    assert_ne!(first["manifest_sha256"], second["manifest_sha256"]);
    assert_ne!(first, second);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_build_manifest_degrades_to_unknown_candidate() {
    let root = temp_root("missing_build_manifest");
    let candidate = identity::build_candidate_from_path_for_test(&root.join("missing.json"), 123);
    assert!(candidate.is_none());
    let process = identity::snapshot().process;
    assert!(!process.runtime_instance_id().is_empty());
}

#[test]
fn dirty_path_change_updates_candidate_and_dirty_state_hashes() {
    let root = temp_root("dirty_path_drift");
    fs::create_dir_all(&root).expect("temp root");
    let path = root.join("manifest.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "repository": {"dirty": false, "dirty_paths": []}
        }))
        .expect("first manifest"),
    )
    .expect("first manifest write");
    let first = serde_json::to_value(
        identity::build_candidate_from_path_for_test(&path, 123).expect("first candidate"),
    )
    .expect("first candidate JSON");

    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "repository": {"dirty": true, "dirty_paths": ["docs/note.md"]}
        }))
        .expect("second manifest"),
    )
    .expect("second manifest write");
    let second = serde_json::to_value(
        identity::build_candidate_from_path_for_test(&path, 123).expect("second candidate"),
    )
    .expect("second candidate JSON");

    assert_ne!(first["manifest_sha256"], second["manifest_sha256"]);
    assert_ne!(first["dirty_state_sha256"], second["dirty_state_sha256"]);
    assert_eq!(
        first["relation_to_process"],
        "startup_observation_not_deployment_proof"
    );
    assert_eq!(
        second["relation_to_process"],
        "startup_observation_not_deployment_proof"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_instance_identity_is_stable_for_process_lifetime() {
    let first = identity::initialize();
    let second = identity::initialize();
    assert!(std::ptr::eq(first, second));
    assert_eq!(
        first.process.runtime_instance_id(),
        second.process.runtime_instance_id()
    );
    let process = serde_json::to_value(&first.process).expect("process identity JSON");
    assert_eq!(
        process["technical_identity_scope"],
        "runtime_instance_discriminator_not_being_identity_continuity_or_selfhood"
    );
    assert_eq!(
        process["restart_relation"],
        "new_technical_instance_does_not_establish_new_or_same_being"
    );
}

#[test]
fn source_snapshot_redacts_absolute_paths_and_hashes_exact_window() {
    let sample = clock_sample_v1();
    let path = bridge_paths().bridge_root().join("src/lib.rs");
    let snapshot = source_snapshot_v1(&path, "whole file", "viewed window", 2, 4, 9, sample);
    let encoded = serde_json::to_value(snapshot).expect("snapshot serialization");
    assert_eq!(encoded["window_sha256"], sha256_bytes(b"viewed window"));
    assert_eq!(
        encoded["source_ownership_scope"],
        "names_byte_ownership_not_interpretation_authorship_or_experiential_identity"
    );
    assert_eq!(
        encoded["interpretation_relation"],
        "source_window_may_support_astrid_authored_distinct_or_mixed_interpretation"
    );
    assert_eq!(
        encoded["provenance_role_scope"],
        "evidence_graph_roles_only_no_runtime_weight_ranking_spectral_or_control_effect"
    );
    assert_eq!(
        encoded["provenance_ref_v1"]["context_anchor_v1"]["influence_types"],
        json!(["temporal", "interpretive"])
    );
    assert!(!encoded.to_string().contains("/Users/"));
    assert_eq!(encoded["private_path_included"], false);
}

#[test]
fn source_ownership_distinguishes_sibling_repository_roots() {
    let paths = bridge_paths();
    let (astrid_owner, astrid_relative) =
        source_owner_and_relative_path(&paths.astrid_root().join("crates/astrid-types/src/lib.rs"));
    assert_eq!(astrid_owner, "astrid");
    assert_eq!(astrid_relative, "crates/astrid-types/src/lib.rs");

    let (minime_owner, minime_relative) =
        source_owner_and_relative_path(&paths.minime_root().join("minime/src/lib.rs"));
    assert_eq!(minime_owner, "minime");
    assert_eq!(minime_relative, "minime/src/lib.rs");

    let (astrid_workspace_owner, astrid_workspace_relative) =
        source_owner_and_relative_path(&paths.bridge_workspace().join("diagnostics/status.json"));
    assert_eq!(astrid_workspace_owner, "astrid_workspace");
    assert_eq!(
        astrid_workspace_relative,
        "workspace/diagnostics/status.json"
    );

    let (minime_workspace_owner, minime_workspace_relative) =
        source_owner_and_relative_path(&paths.minime_workspace().join("spectral_state.json"));
    assert_eq!(minime_workspace_owner, "minime_workspace");
    assert_eq!(minime_workspace_relative, "workspace/spectral_state.json");
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
        authorship.authored_process_sequence,
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
    assert!(raw.contains("reported_persistence_preserved_mechanism_open"));
    assert!(raw.contains("reported_influence_not_denied_or_adjudicated_by_receipt"));
    assert!(raw.contains("preserved_in_canonical_report_no_scalar_substitution"));
    assert!(raw.contains("report_may_inform_claims_evidence_implementation_and_review"));
    assert!(raw.contains(
        "engineering_and_review_influence_allowed_direct_runtime_control_forbidden"
    ));
    assert!(raw.contains("separate_verified_authority_required_for_live_control"));
    assert!(raw.contains("interpretation_provenance_ref_v1"));
    assert!(raw.contains(
        "astrid_authored_artifact_with_exact_source_and_model_call_parents"
    ));
    assert!(raw.contains(
        "unmeasured_no_scalar_inferred_from_parent_membership_or_spectral_proximity"
    ));
    assert!(raw.contains("\"origin\": \"astrid_interpretation\""));
    assert!(raw.contains("\"influence_types\": [\n        \"interpretive\",\n        \"authorship\""));
    assert!(raw.contains("\"epistemic_posture\": \"non_adjudicating\""));
    assert!(raw.contains("\"artifact_live_control_effect\": false"));
    assert!(!raw.contains("\"live_control_effect\": false"));
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
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("change metadata");
    writer::atomic_owner_write(&path, b"same\n").expect("idempotent publish");
    assert_eq!(
        fs::metadata(&path)
            .expect("sidecar metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let error = writer::atomic_owner_write(&path, b"changed\n")
        .expect_err("different bytes must not replace an immutable sidecar");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&path).expect("published bytes"), b"same\n");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporal_progression_creates_a_new_immutable_sidecar() {
    let root = temp_root("temporal_progression");
    let first = test_witness(b"same report bytes");
    let second = test_witness(b"same report bytes");
    assert_ne!(first.witness_id(), second.witness_id());

    writer::write_witness_for_test(&root, first.clone()).expect("first temporal witness");
    writer::write_witness_for_test(&root, second.clone()).expect("second temporal witness");

    let first_bytes = fs::read(
        root.join("witnesses")
            .join(format!("{}.json", first.witness_id())),
    )
    .expect("first sidecar");
    let second_bytes = fs::read(
        root.join("witnesses")
            .join(format!("{}.json", second.witness_id())),
    )
    .expect("second sidecar");
    assert_ne!(first_bytes, second_bytes);
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
        Some(4),
        Some(11),
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
    assert!(encoded.contains("not_inspected_or_adjudicated_by_this_receipt"));
    assert!(encoded.contains("provider_split_observed"));
    assert!(
        encoded
            .contains("technical_metadata_availability_not_experiential_wholeness_or_continuity")
    );
    assert!(encoded.contains("request_enqueue_to_worker_selection_not_experiential_wait"));
    assert!(
        encoded
            .contains("worker_selection_to_response_after_reservoir_checkin_not_cognitive_effort")
    );
    assert!(encoded.contains("technical_delivery_path_not_experiential_center"));
    assert!(encoded.contains(
        "end_to_end_request_wall_time_with_optional_provider_phase_split_not_experiential_continuity"
    ));
    assert!(encoded.contains("post_call_authorship_observations_temporal_only"));
    assert!(
        encoded.contains("canonical_felt_report_primary_not_duplicated_or_scalarized_by_route")
    );
    assert!(!encoded.contains("being_identity_claimed"));
    assert!(!encoded.contains("continuity_claimed"));
}

#[test]
fn model_route_preserves_partial_provider_timing_without_scoring_wholeness() {
    let queue_only = model_route_v1(
        Some("job_queue".to_string()),
        Some("c".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        "production",
        10,
        25,
        Some(4),
        None,
        None,
        "private response prose",
    );
    let active_only = model_route_v1(
        Some("job_active".to_string()),
        Some("d".repeat(64)),
        Some("b".repeat(64)),
        "mlx",
        "production",
        30,
        50,
        None,
        Some(19),
        None,
        "other private response prose",
    );
    let encoded = serde_json::to_value([queue_only, active_only]).expect("route JSON");
    assert_eq!(encoded[0]["queue_wait_ms"], 4);
    assert!(encoded[0]["active_generation_and_reservoir_ms"].is_null());
    assert_eq!(encoded[0]["timing_completeness"], "queue_wait_only");
    assert!(encoded[1]["queue_wait_ms"].is_null());
    assert_eq!(encoded[1]["active_generation_and_reservoir_ms"], 19);
    assert_eq!(encoded[1]["timing_completeness"], "active_work_only");
    assert!(
        encoded
            .to_string()
            .contains("not_experiential_wholeness_or_continuity")
    );
    assert!(!encoded.to_string().contains("private response prose"));
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
        None,
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
    hasher.update(sha256_bytes(b"mlx").as_bytes());
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
    assert_eq!(encoded["provider_route"], "mlx");
    assert_eq!(encoded["provider_route_complete"], true);
    assert_eq!(encoded["provider_route_sha256"], sha256_bytes(b"mlx"));
    assert_eq!(
        encoded["provider_route_hash_scope"],
        "full_technical_route_integrity_not_experiential_identity"
    );
}

#[test]
fn model_route_identity_hashes_the_complete_route_before_bounding_display() {
    let prefix = "route/".repeat(8);
    let first_route = format!("{prefix}first");
    let second_route = format!("{prefix}second");
    assert_eq!(
        first_route.chars().take(40).collect::<String>(),
        second_route.chars().take(40).collect::<String>()
    );
    let first = model_route_v1(
        Some("job_test".to_string()),
        Some("c".repeat(64)),
        Some("a".repeat(64)),
        &first_route,
        "production",
        10,
        25,
        None,
        None,
        None,
        "same private response",
    );
    let second = model_route_v1(
        Some("job_test".to_string()),
        Some("c".repeat(64)),
        Some("a".repeat(64)),
        &second_route,
        "production",
        10,
        25,
        None,
        None,
        None,
        "same private response",
    );
    let encoded = serde_json::to_value([first, second]).expect("route JSON");
    assert_eq!(encoded[0]["provider_route"], encoded[1]["provider_route"]);
    assert_eq!(encoded[0]["provider_route_complete"], false);
    assert_eq!(encoded[1]["provider_route_complete"], false);
    assert_ne!(
        encoded[0]["provider_route_sha256"],
        encoded[1]["provider_route_sha256"]
    );
    assert_ne!(encoded[0]["call_id"], encoded[1]["call_id"]);
}

#[test]
fn model_route_call_identity_distinguishes_request_start_time() {
    let first = model_route_v1(
        None,
        None,
        None,
        "mlx",
        "production",
        10,
        25,
        None,
        None,
        None,
        "same private response",
    );
    let second = model_route_v1(
        None,
        None,
        None,
        "mlx",
        "production",
        11,
        25,
        None,
        None,
        None,
        "same private response",
    );
    let encoded = serde_json::to_value([first, second]).expect("route JSON");
    assert_ne!(encoded[0]["call_id"], encoded[1]["call_id"]);
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
        None,
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
        None,
        None,
        Some(first.call_id().to_string()),
        "repair private response",
    );
    let encoded = serde_json::to_value([first, repair]).expect("routes JSON");
    assert_eq!(encoded[1]["repair_parent_call_id"], encoded[0]["call_id"]);
    assert!(!encoded.to_string().contains("private response"));
}

#[test]
fn response_changes_do_not_change_the_request_anchor_or_classify_response_claims() {
    let first = model_route_v1(
        Some("job_first".to_string()),
        Some("d".repeat(64)),
        Some("a".repeat(64)),
        "mlx",
        "production",
        10,
        20,
        None,
        None,
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
        None,
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
        assert_eq!(
            route["response_claim_content_relation"],
            "not_inspected_or_adjudicated_by_this_receipt"
        );
        assert!(route.get("being_identity_claimed").is_none());
        assert!(route.get("continuity_claimed").is_none());
        assert!(route.get("intent_equivalence_claimed").is_none());
        assert!(route.get("semantic_equivalence_claimed").is_none());
    }
}

#[test]
fn stale_or_absent_peer_state_is_unknown_not_active_inference() {
    let value = json!({
        "fill_pct": 68.0,
        "spectral_entropy": 0.75,
        "structural_entropy": 0.5
    });
    let stale = peer_scalar_observations_from_snapshot(
        Some(&value),
        Some(MAX_PEER_STATE_AGE_MS + 1),
        100,
        PeerEvidenceSourceStatusV1::Observed,
    );
    let stale_json = serde_json::to_value(stale).expect("stale observations");
    assert!(stale_json.as_array().expect("array").iter().all(|row| {
        row["observation_kind"] == "unknown"
            && row["value"].is_null()
            && row["fresh"] == false
            && row["value_relation"] == "peer_scalar_withheld_as_stale_temporal_context_only"
    }));
    let fresh = peer_scalar_observations_from_snapshot(
        Some(&value),
        Some(MAX_PEER_STATE_AGE_MS),
        101,
        PeerEvidenceSourceStatusV1::Observed,
    );
    let fresh_json = serde_json::to_value(fresh).expect("fresh observations");
    assert!(fresh_json.as_array().expect("array").iter().all(|row| {
        row["observation_kind"] == "peer_observed"
            && !row["value"].is_null()
            && row["fresh"] == true
            && row["value_relation"] == "fresh_peer_scalar_observed"
    }));
    let absent = peer_scalar_observations_from_snapshot(
        None,
        None,
        100,
        PeerEvidenceSourceStatusV1::FileMissing,
    );
    let absent_json = serde_json::to_value(absent).expect("absent observations");
    assert!(
        absent_json
            .as_array()
            .expect("array")
            .iter()
            .all(|row| { row["observation_kind"] == "unknown" && row["value"].is_null() })
    );
    assert!(
        absent_json
            .as_array()
            .expect("array")
            .iter()
            .all(|row| row["value_relation"] == "source_file_missing")
    );
}

#[test]
fn peer_snapshot_distinguishes_missing_unreadable_and_malformed_sources() {
    let root = temp_root("peer_snapshot_status");
    fs::create_dir_all(&root).expect("temp root");
    let missing = peer_evidence_cache::load_for_test(&root.join("missing.json"));
    assert_eq!(missing.status, PeerEvidenceSourceStatusV1::FileMissing);

    let unreadable = peer_evidence_cache::load_for_test(&root);
    assert_eq!(
        unreadable.status,
        PeerEvidenceSourceStatusV1::FileUnreadable
    );

    let malformed_path = root.join("malformed.json");
    fs::write(&malformed_path, "{").expect("malformed fixture");
    let malformed = peer_evidence_cache::load_for_test(&malformed_path);
    assert_eq!(malformed.status, PeerEvidenceSourceStatusV1::JsonMalformed);
    assert!(malformed.value.is_none());

    let empty_path = root.join("empty.json");
    fs::write(&empty_path, "").expect("empty fixture");
    let empty = peer_evidence_cache::load_for_test(&empty_path);
    assert_eq!(empty.status, PeerEvidenceSourceStatusV1::JsonMalformed);
    assert!(empty.value.is_none());

    let observed_path = root.join("observed.json");
    fs::write(
        &observed_path,
        r#"{"fill_pct":68.0,"spectral_entropy":0.8,"structural_entropy":0.7}"#,
    )
    .expect("observed fixture");
    let observed = peer_evidence_cache::load_for_test(&observed_path);
    assert_eq!(observed.status, PeerEvidenceSourceStatusV1::Observed);
    assert_eq!(
        observed
            .value
            .as_ref()
            .and_then(|value| value.get("fill_pct"))
            .and_then(serde_json::Value::as_f64),
        Some(68.0)
    );
    fs::write(&observed_path, "{").expect("degraded fixture");
    let degraded = peer_evidence_cache::load_for_test(&observed_path);
    assert_eq!(degraded.status, PeerEvidenceSourceStatusV1::JsonMalformed);
    assert!(
        degraded.value.is_none(),
        "evidence errors must not retain last-known scalars as current presence"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cached_peer_snapshot_read_is_below_one_millisecond_p95() {
    let mut elapsed = (0..1_000)
        .map(|_| {
            let started = std::time::Instant::now();
            let _ = peer_evidence_cache::snapshot();
            started.elapsed().as_nanos()
        })
        .collect::<Vec<_>>();
    elapsed.sort_unstable();
    let p95 = elapsed[949];
    assert!(p95 < 1_000_000, "cached snapshot p95 was {p95}ns");
}

#[test]
fn capture_sequence_and_identity_scopes_are_explicit() {
    let first = begin_authorship_v1(None, &[], "introspection");
    let second = begin_authorship_v1(None, &[], "introspection");
    assert!(second.authored_process_sequence > first.authored_process_sequence);

    let encoded = serde_json::to_value(test_witness(b"scope")).expect("witness JSON");
    assert!(encoded["authored_process_sequence"].as_u64().is_some());
    assert_eq!(
        encoded["authored_process_sequence_scope"],
        "per_runtime_instance_capture_order_not_experiential_time_or_global_order"
    );
    assert_eq!(
        encoded["authorship_clock_scope"],
        "wall_clock_and_process_monotonic_observations_not_experiential_time_or_internal_sequence"
    );
    assert_eq!(
        encoded["peer_identity_scope"],
        "witnessed_protocol_advertisement_not_being_identity_or_peer_self_authority"
    );
    assert_eq!(
        encoded["privacy_hash_scope"],
        "absolute_path_redaction_not_being_or_continuity_identity"
    );
    assert_eq!(
        encoded["process_provenance_scope"],
        "bridge_evidence_derivation_not_being_origin_identity_or_continuity"
    );
}

#[test]
fn concurrent_capture_sequences_are_unique() {
    const THREADS: usize = 16;
    const CAPTURES_PER_THREAD: usize = 64;

    let mut handles = Vec::with_capacity(THREADS);
    for _ in 0..THREADS {
        handles.push(std::thread::spawn(|| {
            (0..CAPTURES_PER_THREAD)
                .map(|_| begin_authorship_v1(None, &[], "introspection").authored_process_sequence)
                .collect::<Vec<_>>()
        }));
    }

    let mut sequences = handles
        .into_iter()
        .flat_map(|handle| handle.join().expect("capture thread"))
        .collect::<Vec<_>>();
    assert_eq!(sequences.len(), THREADS * CAPTURES_PER_THREAD);
    sequences.sort_unstable();
    sequences.dedup();
    assert_eq!(sequences.len(), THREADS * CAPTURES_PER_THREAD);
}

#[test]
fn bounded_identity_is_deterministic_and_context_independent() {
    let absolute = "/private/runtime/example/process.json";
    let first_absolute = bounded_identity(Some(absolute)).expect("absolute identity");
    let second_absolute = bounded_identity(Some(absolute)).expect("absolute identity");
    assert_eq!(first_absolute, second_absolute);
    assert!(first_absolute.starts_with("sha256:"));
    assert!(!first_absolute.contains(absolute));

    let relative = "peer/runtime-instance-v1";
    assert_eq!(bounded_identity(Some(relative)).as_deref(), Some(relative));
    assert_eq!(bounded_identity(Some(relative)).as_deref(), Some(relative));

    let shared_prefix = "peer/".to_string() + &"r".repeat(180);
    let first_long = format!("{shared_prefix}/first");
    let second_long = format!("{shared_prefix}/second");
    let first_bounded = bounded_identity(Some(&first_long)).expect("first long identity");
    let second_bounded = bounded_identity(Some(&second_long)).expect("second long identity");
    assert!(first_bounded.starts_with("sha256:"));
    assert!(second_bounded.starts_with("sha256:"));
    assert_ne!(first_bounded, second_bounded);
    assert!(!first_bounded.contains(&shared_prefix));
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
