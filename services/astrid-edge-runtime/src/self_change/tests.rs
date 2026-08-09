use uuid::Uuid;

use super::SelfChangeError;
use super::receipt::{ReceiptChainHeadV1, append_receipt, verify_receipt_chain};
use super::schema::{
    BuildEvidenceV1, CANDIDATE_PATCH_SCHEMA_V1, CandidateFileChangeV1, CandidatePatchV1,
    CandidatePhaseV1, ChangeOperationV1, DOMAIN_STATE_SCHEMA_V1, EXACT_MODEL_ATTESTATION_SCHEMA_V1,
    ExactModelAttestationV1, ExactModelProvenanceV1, ImmutablePathClassV1, ProbationEvidenceV1,
    RECEIPT_SCHEMA_V1, SCHEDULED_INTROSPECTION_SCHEMA_V1, ScheduledIntrospectionAuthorityV1,
    ScheduledIntrospectionKindV1, ScheduledIntrospectionV1, SelfChangeCommandV1,
    SelfChangeDomainStateV1, TRANSITION_REQUEST_SCHEMA_V1, TransitionActorV1, TransitionRequestV1,
};
use super::state::{apply_transition, derive_candidate_id, state_sha256};
use super::validation::{
    MAX_CHANGED_FILES, MAX_CHANGED_LINES, classify_immutable_path, sha256_hex,
    validate_candidate_patch, validate_candidate_source_path, validate_source_id,
};

const INSTANCE: &str = "avado-edge-01";
const BASE_TIME: i64 = 2_000_000_000_000;

fn hash(label: &str) -> String {
    sha256_hex(label)
}

fn object_id(prefix: &str, label: &str) -> String {
    format!("{prefix}-{}", &hash(label)[..24])
}

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn exact_attestation(label: &str, at_ms: i64) -> ExactModelAttestationV1 {
    let offset_byte = label.bytes().fold(0_u8, u8::wrapping_add);
    let offset = u128::from(offset_byte);
    ExactModelAttestationV1 {
        schema: EXACT_MODEL_ATTESTATION_SCHEMA_V1.to_string(),
        provenance: ExactModelProvenanceV1::ExactModel,
        instance_id: INSTANCE.to_string(),
        producer_kind: "wasm_capsule".to_string(),
        producer_capsule_id: "astrid-capsule-react".to_string(),
        kernel_sequence: u64::from(offset_byte).saturating_add(1),
        trace_id: uuid(100 + offset),
        span_id: uuid(200 + offset),
        session_id: uuid(300 + offset),
        session_generation: 1,
        chain_id: None,
        chain_step: None,
        turn_id: uuid(400 + offset),
        response_sha256: hash(&format!("response:{label}")),
        terminal_declaration_sha256: hash(&format!("declaration:{label}")),
        model_id: "qwen3.5:4b".to_string(),
        authored_at_unix_ms: at_ms,
    }
}

fn source_id() -> String {
    format!("cpu-edge:{}", hash("source-revision"))
}

fn file_change(path: &str, added: u32, removed: u32) -> CandidateFileChangeV1 {
    CandidateFileChangeV1 {
        path: path.to_string(),
        operation: ChangeOperationV1::Modify,
        old_sha256: Some(hash(&format!("old:{path}"))),
        new_sha256: Some(hash(&format!("new:{path}"))),
        added_lines: added,
        removed_lines: removed,
    }
}

fn patch(attestation: &ExactModelAttestationV1) -> CandidatePatchV1 {
    let patch_sha256 = hash("candidate-patch");
    CandidatePatchV1 {
        schema: CANDIDATE_PATCH_SCHEMA_V1.to_string(),
        candidate_id: derive_candidate_id(
            INSTANCE,
            &source_id(),
            &patch_sha256,
            &attestation.response_sha256,
        ),
        source_id: source_id(),
        source_manifest_sha256: hash("source-tree"),
        proposal_sha256: hash("proposal"),
        patch_sha256,
        files: vec![file_change(
            "services/astrid-edge-runtime/src/host.rs",
            12,
            7,
        )],
    }
}

fn exact_actor(attestation: ExactModelAttestationV1) -> TransitionActorV1 {
    TransitionActorV1::ExactModel {
        attestation: Box::new(attestation),
    }
}

fn broker_actor(label: &str) -> TransitionActorV1 {
    TransitionActorV1::CandidateBroker {
        broker_id: "edge-candidate-broker".to_string(),
        signed_receipt_sha256: hash(&format!("broker:{label}")),
    }
}

fn operator_actor(label: &str) -> TransitionActorV1 {
    TransitionActorV1::Operator {
        authorization_id: object_id("auth", label),
        signed_receipt_sha256: hash(&format!("operator:{label}")),
    }
}

fn request(
    state: &SelfChangeDomainStateV1,
    candidate_id: &str,
    label: &str,
    at_ms: i64,
    actor: TransitionActorV1,
    command: SelfChangeCommandV1,
) -> TransitionRequestV1 {
    TransitionRequestV1 {
        schema: TRANSITION_REQUEST_SCHEMA_V1.to_string(),
        command_id: object_id("cmd", label),
        candidate_id: candidate_id.to_string(),
        expected_state_sha256: state_sha256(state).unwrap(),
        occurred_at_unix_ms: at_ms,
        actor,
        command,
    }
}

fn proposed_state() -> (SelfChangeDomainStateV1, ExactModelAttestationV1, String) {
    let attestation = exact_attestation("proposal", BASE_TIME);
    let patch = patch(&attestation);
    let candidate_id = patch.candidate_id.clone();
    let mut state = SelfChangeDomainStateV1::new(INSTANCE);
    let proposal = request(
        &state,
        &candidate_id,
        "propose",
        BASE_TIME,
        exact_actor(attestation.clone()),
        SelfChangeCommandV1::Propose { patch },
    );
    apply_transition(&mut state, &proposal).unwrap();
    (state, attestation, candidate_id)
}

fn advance_to_build_passed() -> (SelfChangeDomainStateV1, ExactModelAttestationV1, String) {
    let (mut state, proposal_attestation, candidate_id) = proposed_state();
    let validate = request(
        &state,
        &candidate_id,
        "validate",
        BASE_TIME + 1_000,
        broker_actor("validate"),
        SelfChangeCommandV1::ValidateSource {
            source_tree_sha256: hash("source-tree"),
        },
    );
    apply_transition(&mut state, &validate).unwrap();
    let build_id = object_id("build", "one");
    let start = request(
        &state,
        &candidate_id,
        "build-start",
        BASE_TIME + 2_000,
        broker_actor("build-start"),
        SelfChangeCommandV1::StartBuild {
            build_id: build_id.clone(),
        },
    );
    apply_transition(&mut state, &start).unwrap();
    let finish = request(
        &state,
        &candidate_id,
        "build-finish",
        BASE_TIME + 3_000,
        broker_actor("build-finish"),
        SelfChangeCommandV1::CompleteBuild {
            evidence: BuildEvidenceV1 {
                build_id,
                source_tree_sha256: hash("source-tree"),
                test_manifest_sha256: hash("tests"),
                artifact_sha256: Some(hash("artifact")),
                evidence_sha256: hash("build-evidence"),
                completed_at_unix_ms: BASE_TIME + 3_000,
                passed: true,
            },
        },
    );
    apply_transition(&mut state, &finish).unwrap();
    (state, proposal_attestation, candidate_id)
}

fn probation_schedule(
    state: &SelfChangeDomainStateV1,
    proposal: &ExactModelAttestationV1,
    candidate_id: &str,
) -> ScheduledIntrospectionV1 {
    ScheduledIntrospectionV1 {
        schema: SCHEDULED_INTROSPECTION_SCHEMA_V1.to_string(),
        schedule_id: object_id("si", "probation"),
        instance_id: INSTANCE.to_string(),
        candidate_id: candidate_id.to_string(),
        kind: ScheduledIntrospectionKindV1::ProbationCheckpoint,
        authority: ScheduledIntrospectionAuthorityV1::ObservationOnly,
        not_before_unix_ms: BASE_TIME + 4_000,
        expires_at_unix_ms: BASE_TIME + 60_000,
        question: "Did the isolated candidate remain healthy without changing production?"
            .to_string(),
        expected_candidate_state_sha256: state_sha256(state).unwrap(),
        originating_trace_id: proposal.trace_id,
        originating_turn_id: proposal.turn_id,
        originating_response_sha256: proposal.response_sha256.clone(),
    }
}

fn advance_to_probation_passed() -> (SelfChangeDomainStateV1, String) {
    let (mut state, proposal, candidate_id) = advance_to_build_passed();
    let probation_id = object_id("prob", "one");
    let schedule = probation_schedule(&state, &proposal, &candidate_id);
    let schedule_request = request(
        &state,
        &candidate_id,
        "probation-schedule",
        BASE_TIME + 4_000,
        broker_actor("probation-schedule"),
        SelfChangeCommandV1::ScheduleProbation {
            probation_id: probation_id.clone(),
            introspection: schedule,
        },
    );
    apply_transition(&mut state, &schedule_request).unwrap();
    let start = request(
        &state,
        &candidate_id,
        "probation-start",
        BASE_TIME + 5_000,
        broker_actor("probation-start"),
        SelfChangeCommandV1::StartProbation {
            started_at_unix_ms: BASE_TIME + 5_000,
        },
    );
    apply_transition(&mut state, &start).unwrap();
    let finish = request(
        &state,
        &candidate_id,
        "probation-finish",
        BASE_TIME + 6_000,
        broker_actor("probation-finish"),
        SelfChangeCommandV1::CompleteProbation {
            evidence: ProbationEvidenceV1 {
                probation_id,
                artifact_sha256: hash("artifact"),
                health_manifest_sha256: hash("health"),
                expected_samples: 100,
                observed_samples: 95,
                started_at_unix_ms: BASE_TIME + 5_000,
                completed_at_unix_ms: BASE_TIME + 6_000,
                passed: true,
            },
        },
    );
    apply_transition(&mut state, &finish).unwrap();
    (state, candidate_id)
}

#[test]
fn exact_candidate_id_is_deterministic_and_bound_to_authored_response() {
    let attestation = exact_attestation("proposal", BASE_TIME);
    let first = patch(&attestation).candidate_id;
    let second = patch(&attestation).candidate_id;
    assert_eq!(first, second);
    let different = patch(&exact_attestation("different", BASE_TIME)).candidate_id;
    assert_ne!(first, different);
}

#[test]
fn fallback_or_repair_provenance_cannot_deserialize_as_exact_model() {
    let attestation = exact_attestation("proposal", BASE_TIME);
    let mut encoded = serde_json::to_value(attestation).unwrap();
    encoded["provenance"] = serde_json::json!("with_local_safe_fallback");
    assert!(serde_json::from_value::<ExactModelAttestationV1>(encoded).is_err());

    let mut repaired = serde_json::to_value(exact_attestation("repair", BASE_TIME)).unwrap();
    repaired["provenance"] = serde_json::json!("with_local_format_repair");
    assert!(serde_json::from_value::<ExactModelAttestationV1>(repaired).is_err());
}

#[test]
fn exact_attestation_requires_kernel_react_producer_and_freshness() {
    let attestation = exact_attestation("proposal", BASE_TIME);
    let candidate = patch(&attestation);
    let candidate_id = candidate.candidate_id.clone();
    let mut state = SelfChangeDomainStateV1::new(INSTANCE);
    let mut wrong_producer = attestation.clone();
    wrong_producer.producer_capsule_id = "operator-harness".to_string();
    let proposal = request(
        &state,
        &candidate_id,
        "wrong-producer",
        BASE_TIME,
        exact_actor(wrong_producer),
        SelfChangeCommandV1::Propose { patch: candidate },
    );
    assert_eq!(
        apply_transition(&mut state, &proposal),
        Err(SelfChangeError::InvalidAttestation(
            "missing canonical kernel provenance"
        ))
    );

    let stale_attestation = exact_attestation("stale", BASE_TIME);
    let stale_patch = patch(&stale_attestation);
    let stale_id = stale_patch.candidate_id.clone();
    let stale_request = request(
        &state,
        &stale_id,
        "stale",
        BASE_TIME + 600_001,
        exact_actor(stale_attestation),
        SelfChangeCommandV1::Propose { patch: stale_patch },
    );
    assert!(matches!(
        apply_transition(&mut state, &stale_request),
        Err(SelfChangeError::InvalidAttestation(_))
    ));
}

#[test]
fn transition_is_atomic_and_rejects_stale_hash_and_replay() {
    let attestation = exact_attestation("proposal", BASE_TIME);
    let patch = patch(&attestation);
    let candidate_id = patch.candidate_id.clone();
    let mut state = SelfChangeDomainStateV1::new(INSTANCE);
    let proposal = request(
        &state,
        &candidate_id,
        "propose",
        BASE_TIME,
        exact_actor(attestation),
        SelfChangeCommandV1::Propose { patch },
    );
    let original = state.clone();
    let mut stale_transition = proposal.clone();
    stale_transition.expected_state_sha256 = hash("not-the-state");
    assert_eq!(
        apply_transition(&mut state, &stale_transition),
        Err(SelfChangeError::StaleStateHash)
    );
    assert_eq!(state, original);

    apply_transition(&mut state, &proposal).unwrap();
    let after = state.clone();
    assert_eq!(
        apply_transition(&mut state, &proposal),
        Err(SelfChangeError::ReplayCommand)
    );
    assert_eq!(state, after);
}

#[test]
fn exact_attestation_cannot_authorize_a_second_command() {
    let (mut state, attestation, candidate_id) = proposed_state();
    let cancel = request(
        &state,
        &candidate_id,
        "cancel-with-replayed-turn",
        BASE_TIME + 1_000,
        exact_actor(attestation),
        SelfChangeCommandV1::Cancel {
            reason_sha256: hash("reason"),
        },
    );
    assert_eq!(
        apply_transition(&mut state, &cancel),
        Err(SelfChangeError::ReplayAttestation)
    );
}

#[test]
fn only_one_candidate_can_be_active() {
    let (mut state, _, _) = proposed_state();
    let second_attestation = exact_attestation("second", BASE_TIME + 1_000);
    let second_patch = patch(&second_attestation);
    let second_id = second_patch.candidate_id.clone();
    let second = request(
        &state,
        &second_id,
        "second-proposal",
        BASE_TIME + 1_000,
        exact_actor(second_attestation),
        SelfChangeCommandV1::Propose {
            patch: second_patch,
        },
    );
    assert_eq!(
        apply_transition(&mut state, &second),
        Err(SelfChangeError::ActiveTransactionExists)
    );
}

#[test]
fn proposal_requires_exact_model_and_machine_transitions_require_broker() {
    let attestation = exact_attestation("proposal", BASE_TIME);
    let patch = patch(&attestation);
    let candidate_id = patch.candidate_id.clone();
    let mut state = SelfChangeDomainStateV1::new(INSTANCE);
    let machine_proposal = request(
        &state,
        &candidate_id,
        "machine-proposal",
        BASE_TIME,
        broker_actor("machine-proposal"),
        SelfChangeCommandV1::Propose { patch },
    );
    assert_eq!(
        apply_transition(&mut state, &machine_proposal),
        Err(SelfChangeError::InvalidAuthority)
    );

    let (mut proposed, _, id) = proposed_state();
    let exact = exact_attestation("tries-build", BASE_TIME + 1_000);
    let validate = request(
        &proposed,
        &id,
        "exact-validates",
        BASE_TIME + 1_000,
        exact_actor(exact),
        SelfChangeCommandV1::ValidateSource {
            source_tree_sha256: hash("source-tree"),
        },
    );
    assert_eq!(
        apply_transition(&mut proposed, &validate),
        Err(SelfChangeError::InvalidAuthority)
    );
}

#[test]
fn eligible_surface_includes_core_runtime_capsule_report_profile_and_service() {
    for path in [
        "crates/astrid-daemon/src/main.rs",
        "Cargo.toml",
        "Cargo.lock",
        "crates/astrid-daemon/Cargo.toml",
        "crates/astrid-kernel/src/lib.rs",
        "crates/astrid-types/src/ipc.rs",
        "services/astrid-edge-runtime/src/actions.rs",
        "services/astrid-edge-runtime/src/autonomy.rs",
        "services/astrid-edge-runtime/src/host.rs",
        "services/astrid-edge-runtime/src/self_change.rs",
        "services/astrid-edge-runtime/src/self_change/state.rs",
        "services/astrid-edge-runtime/Cargo.toml",
        "services/astrid-edge-runtime/Cargo.lock",
        "capsules/astralis/astrid-capsule-edge-context/src/lib.rs",
        "capsules/astralis/astrid-capsule-edge-context/Cargo.toml",
        "capsules/astralis/astrid-capsule-edge-context/Capsule.toml",
        "scripts/report_edge_appliance.py",
        "scripts/test_report_edge_activity.py",
        "packaging/appliances/avado-i3-16g.env",
        "packaging/appliances/icp-j3455-8g.edge-context.json",
        "packaging/systemd/astrid-edge-runtime.service",
        "packaging/systemd/icp/astrid.service",
    ] {
        assert!(
            validate_candidate_source_path(path).is_ok(),
            "rejected eligible CPU-edge surface {path}"
        );
    }
}

#[test]
fn immutable_rescue_mac_private_and_vcs_paths_are_denied_by_class() {
    assert_eq!(
        classify_immutable_path("scripts/edge_self_change_supervisor.py"),
        Some(ImmutablePathClassV1::ImmutableRescueRoot)
    );
    assert_eq!(
        classify_immutable_path("packaging/systemd/astrid-edge-self-change-supervisor.service"),
        Some(ImmutablePathClassV1::ImmutableRescueRoot)
    );
    for path in [
        "services/astrid-edge-steward-helper/src/main.rs",
        "services/astrid-edge-rescue-helper/src/main.rs",
        "services/astrid-edge-web-broker/src/main.rs",
        "services/astrid-edge-checkpoint/src/main.rs",
    ] {
        assert_eq!(
            classify_immutable_path(path),
            Some(ImmutablePathClassV1::ImmutableRescueRoot),
            "immutable native helper was not classified: {path}"
        );
        assert_eq!(
            validate_candidate_source_path(path),
            Err(SelfChangeError::ImmutablePath(
                ImmutablePathClassV1::ImmutableRescueRoot
            ))
        );
    }
    assert_eq!(
        classify_immutable_path("capsules/spectral-bridge/src/lib.rs"),
        Some(ImmutablePathClassV1::MacMinimeOrBridge)
    );
    assert_eq!(
        classify_immutable_path("crates/astrid-minime-protocol/src/lib.rs"),
        Some(ImmutablePathClassV1::MacMinimeOrBridge)
    );
    assert_eq!(
        classify_immutable_path("home/default/edge/actions/receipts.jsonl"),
        Some(ImmutablePathClassV1::PrivateStateOrSecrets)
    );
    assert_eq!(
        classify_immutable_path(".github/workflows/cpu-edge.yml"),
        Some(ImmutablePathClassV1::VcsOrCi)
    );
}

#[test]
fn paths_reject_traversal_absolute_hidden_and_non_source_surfaces() {
    for path in [
        "../minime/src/main.rs",
        "/home/avado/astrid/services/astrid-edge-runtime/src/host.rs",
        "services/astrid-edge-runtime/src/../config.rs",
        "services/astrid-edge-runtime/src/.hidden.rs",
        "services\\astrid-edge-runtime\\src\\host.rs",
        "services/astrid-edge-runtime/src/host.toml",
        "README.md",
    ] {
        assert!(
            validate_candidate_source_path(path).is_err(),
            "accepted {path}"
        );
    }
}

#[test]
fn source_id_requires_exact_lowercase_sha256() {
    assert!(validate_source_id(&source_id()).is_ok());
    assert!(validate_source_id(&format!("cpu-edge:{}", "A".repeat(64))).is_err());
    assert!(validate_source_id(&format!("mac:{}", hash("source"))).is_err());
    assert!(validate_source_id("cpu-edge:deadbeef").is_err());
}

#[test]
fn patch_enforces_file_and_line_limits_at_boundaries() {
    let attestation = exact_attestation("limits", BASE_TIME);
    let mut bounded = patch(&attestation);
    bounded.files = (0..MAX_CHANGED_FILES)
        .map(|index| {
            file_change(
                &format!("capsules/astralis/astrid-capsule-edge-context/src/file{index:02}.rs"),
                MAX_CHANGED_LINES / u32::try_from(MAX_CHANGED_FILES).unwrap(),
                0,
            )
        })
        .collect();
    assert!(validate_candidate_patch(&bounded).is_ok());

    let mut too_many = bounded.clone();
    too_many.files.push(file_change(
        "capsules/astralis/astrid-capsule-edge-context/src/zz.rs",
        1,
        0,
    ));
    assert_eq!(
        validate_candidate_patch(&too_many),
        Err(SelfChangeError::LimitExceeded("changed file count"))
    );

    let mut too_large = patch(&attestation);
    too_large.files[0].added_lines = MAX_CHANGED_LINES;
    too_large.files[0].removed_lines = 1;
    assert_eq!(
        validate_candidate_patch(&too_large),
        Err(SelfChangeError::LimitExceeded("changed line count"))
    );
}

#[test]
fn patch_rejects_duplicates_unsorted_files_and_operation_hash_mismatches() {
    let attestation = exact_attestation("shape", BASE_TIME);
    let mut duplicate = patch(&attestation);
    duplicate.files.push(duplicate.files[0].clone());
    assert!(matches!(
        validate_candidate_patch(&duplicate),
        Err(SelfChangeError::InvalidPatch(_))
    ));

    let mut invalid_create = patch(&attestation);
    invalid_create.files[0].operation = ChangeOperationV1::Create;
    assert!(matches!(
        validate_candidate_patch(&invalid_create),
        Err(SelfChangeError::InvalidPatch(_))
    ));
}

#[test]
fn schedule_is_observational_and_exactly_bound_to_proposal() {
    let (mut state, proposal, candidate_id) = advance_to_build_passed();
    let mut schedule = probation_schedule(&state, &proposal, &candidate_id);
    schedule.originating_response_sha256 = hash("someone-else");
    let transition = request(
        &state,
        &candidate_id,
        "bad-schedule",
        BASE_TIME + 4_000,
        broker_actor("bad-schedule"),
        SelfChangeCommandV1::ScheduleProbation {
            probation_id: object_id("prob", "one"),
            introspection: schedule,
        },
    );
    let before = state.clone();
    assert!(matches!(
        apply_transition(&mut state, &transition),
        Err(SelfChangeError::InvalidSchedule(_))
    ));
    assert_eq!(state, before);
}

#[test]
fn full_lifecycle_requires_distinct_nomination_and_records_rollback() {
    let (mut state, candidate_id) = advance_to_probation_passed();
    assert_eq!(
        state.active.as_ref().unwrap().phase,
        CandidatePhaseV1::ProbationPassed
    );

    let nomination_attestation = exact_attestation("nomination", BASE_TIME + 7_000);
    let nominate = request(
        &state,
        &candidate_id,
        "nominate",
        BASE_TIME + 7_000,
        exact_actor(nomination_attestation),
        SelfChangeCommandV1::Nominate {
            rationale_sha256: hash("rationale"),
        },
    );
    apply_transition(&mut state, &nominate).unwrap();

    let activate = request(
        &state,
        &candidate_id,
        "activate-record",
        BASE_TIME + 8_000,
        operator_actor("activate-record"),
        SelfChangeCommandV1::RecordActivation {
            deployment_receipt_sha256: hash("deployment"),
        },
    );
    apply_transition(&mut state, &activate).unwrap();
    assert_eq!(
        state.active.as_ref().unwrap().phase,
        CandidatePhaseV1::Active
    );

    let rollback_id = object_id("rollback", "one");
    let rollback = request(
        &state,
        &candidate_id,
        "rollback-request",
        BASE_TIME + 9_000,
        TransitionActorV1::SafetyMonitor {
            event_id: object_id("safe", "red-health"),
            evidence_sha256: hash("red-health"),
        },
        SelfChangeCommandV1::RequestRollback {
            rollback_id: rollback_id.clone(),
            reason_sha256: hash("unsafe"),
        },
    );
    apply_transition(&mut state, &rollback).unwrap();

    let complete = request(
        &state,
        &candidate_id,
        "rollback-complete",
        BASE_TIME + 10_000,
        broker_actor("rollback-complete"),
        SelfChangeCommandV1::CompleteRollback {
            evidence: super::schema::RollbackEvidenceV1 {
                rollback_id,
                restored_artifact_sha256: hash("prior-artifact"),
                health_manifest_sha256: hash("healthy-again"),
                completed_at_unix_ms: BASE_TIME + 10_000,
            },
        },
    );
    apply_transition(&mut state, &complete).unwrap();
    assert_eq!(
        state.active.as_ref().unwrap().phase,
        CandidatePhaseV1::RolledBack
    );

    let archive = request(
        &state,
        &candidate_id,
        "archive",
        BASE_TIME + 11_000,
        operator_actor("archive"),
        SelfChangeCommandV1::Archive,
    );
    apply_transition(&mut state, &archive).unwrap();
    assert!(state.active.is_none());
    assert!(state.completed_candidate_ids.contains(&candidate_id));
}

#[test]
fn probation_pass_requires_ninety_percent_coverage() {
    let (mut state, proposal, candidate_id) = advance_to_build_passed();
    let probation_id = object_id("prob", "coverage");
    let schedule = probation_schedule(&state, &proposal, &candidate_id);
    let schedule_request = request(
        &state,
        &candidate_id,
        "coverage-schedule",
        BASE_TIME + 4_000,
        broker_actor("coverage-schedule"),
        SelfChangeCommandV1::ScheduleProbation {
            probation_id: probation_id.clone(),
            introspection: schedule,
        },
    );
    apply_transition(&mut state, &schedule_request).unwrap();
    let start = request(
        &state,
        &candidate_id,
        "coverage-start",
        BASE_TIME + 5_000,
        broker_actor("coverage-start"),
        SelfChangeCommandV1::StartProbation {
            started_at_unix_ms: BASE_TIME + 5_000,
        },
    );
    apply_transition(&mut state, &start).unwrap();
    let insufficient = request(
        &state,
        &candidate_id,
        "coverage-finish",
        BASE_TIME + 6_000,
        broker_actor("coverage-finish"),
        SelfChangeCommandV1::CompleteProbation {
            evidence: ProbationEvidenceV1 {
                probation_id,
                artifact_sha256: hash("artifact"),
                health_manifest_sha256: hash("health"),
                expected_samples: 100,
                observed_samples: 89,
                started_at_unix_ms: BASE_TIME + 5_000,
                completed_at_unix_ms: BASE_TIME + 6_000,
                passed: true,
            },
        },
    );
    let before = state.clone();
    assert_eq!(
        apply_transition(&mut state, &insufficient),
        Err(SelfChangeError::LimitExceeded("probation sample coverage"))
    );
    assert_eq!(state, before);
}

#[test]
fn cross_instance_attestation_and_candidate_are_rejected() {
    let mut attestation = exact_attestation("foreign", BASE_TIME);
    attestation.instance_id = "icp-edge-01".to_string();
    let patch = patch(&attestation);
    let candidate_id = patch.candidate_id.clone();
    let mut state = SelfChangeDomainStateV1::new(INSTANCE);
    let foreign = request(
        &state,
        &candidate_id,
        "foreign",
        BASE_TIME,
        exact_actor(attestation),
        SelfChangeCommandV1::Propose { patch },
    );
    assert_eq!(
        apply_transition(&mut state, &foreign),
        Err(SelfChangeError::InvalidAttestation("instance mismatch"))
    );
}

#[test]
fn failed_build_is_terminal_until_operator_archives_it() {
    let (mut state, _, candidate_id) = proposed_state();
    let validate = request(
        &state,
        &candidate_id,
        "failure-validate",
        BASE_TIME + 1_000,
        broker_actor("failure-validate"),
        SelfChangeCommandV1::ValidateSource {
            source_tree_sha256: hash("source-tree"),
        },
    );
    apply_transition(&mut state, &validate).unwrap();
    let build_id = object_id("build", "failure");
    let start = request(
        &state,
        &candidate_id,
        "failure-start",
        BASE_TIME + 2_000,
        broker_actor("failure-start"),
        SelfChangeCommandV1::StartBuild {
            build_id: build_id.clone(),
        },
    );
    apply_transition(&mut state, &start).unwrap();
    let fail = request(
        &state,
        &candidate_id,
        "failure-finish",
        BASE_TIME + 3_000,
        broker_actor("failure-finish"),
        SelfChangeCommandV1::CompleteBuild {
            evidence: BuildEvidenceV1 {
                build_id,
                source_tree_sha256: hash("source-tree"),
                test_manifest_sha256: hash("failed-tests"),
                artifact_sha256: None,
                evidence_sha256: hash("failure-evidence"),
                completed_at_unix_ms: BASE_TIME + 3_000,
                passed: false,
            },
        },
    );
    apply_transition(&mut state, &fail).unwrap();
    assert_eq!(
        state.active.as_ref().unwrap().phase,
        CandidatePhaseV1::BuildFailed
    );
}

#[test]
fn receipt_helpers_form_deterministic_append_only_chain_and_detect_tampering() {
    let attestation = exact_attestation("receipt", BASE_TIME);
    let patch = patch(&attestation);
    let candidate_id = patch.candidate_id.clone();
    let mut state = SelfChangeDomainStateV1::new(INSTANCE);
    let proposal = request(
        &state,
        &candidate_id,
        "receipt-proposal",
        BASE_TIME,
        exact_actor(attestation),
        SelfChangeCommandV1::Propose { patch },
    );
    let draft = apply_transition(&mut state, &proposal).unwrap();
    let append = append_receipt(&ReceiptChainHeadV1::default(), draft).unwrap();
    assert!(append.jsonl.ends_with('\n'));
    assert_eq!(append.receipt.content.schema, RECEIPT_SCHEMA_V1);
    assert_eq!(append.receipt.content.sequence, 1);
    assert_eq!(
        verify_receipt_chain(std::slice::from_ref(&append.receipt)).unwrap(),
        append.next_head
    );

    let mut tampered = append.receipt.clone();
    tampered.content.resulting_state_sha256 = hash("tampered");
    assert_eq!(
        verify_receipt_chain(&[tampered]),
        Err(SelfChangeError::ReceiptChainMismatch)
    );
}

#[test]
fn receipt_append_rejects_state_discontinuity() {
    let head = ReceiptChainHeadV1 {
        next_sequence: 2,
        last_receipt_sha256: Some(hash("receipt-one")),
        last_resulting_state_sha256: Some(hash("state-one")),
    };
    let draft = super::schema::SelfChangeReceiptDraftV1 {
        command_id: object_id("cmd", "two"),
        candidate_id: object_id("sc", "candidate"),
        occurred_at_unix_ms: BASE_TIME,
        actor: broker_actor("two"),
        command: super::schema::SelfChangeCommandKindV1::StartBuild,
        from_phase: Some(CandidatePhaseV1::SourceValidated),
        to_phase: Some(CandidatePhaseV1::Building),
        expected_state_sha256: hash("different-state"),
        resulting_state_sha256: hash("state-two"),
    };
    assert_eq!(
        append_receipt(&head, draft),
        Err(SelfChangeError::ReceiptChainMismatch)
    );
}

#[test]
fn schemas_deny_unknown_fields_and_state_schema_is_stable() {
    let state = SelfChangeDomainStateV1::new(INSTANCE);
    assert_eq!(state.schema, DOMAIN_STATE_SCHEMA_V1);
    let mut value = serde_json::to_value(exact_attestation("unknown", BASE_TIME)).unwrap();
    value["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ExactModelAttestationV1>(value).is_err());
}
