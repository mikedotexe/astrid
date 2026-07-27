use super::*;

fn write_status(root: &Path, lifecycle: &str, current_tick: u64, deadline: Option<u64>) {
    let division = root.join("division");
    fs::create_dir_all(&division).unwrap();
    fs::write(
        division.join("status.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema": "division.status.v1",
            "division_id": "divide-one",
            "parent_generation": 7,
            "plan_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "lifecycle": lifecycle,
            "parent_authoritative": true,
            "commit_feature_enabled": false,
            "selected_strategy": "input_recurrence",
            "astrid_assent": false,
            "minime_assent": false,
            "bridge_scale": 1.0,
            "current_tick": current_tick,
            "rollback_deadline_tick": deadline,
            "snapshot_refs": ["sha256:parent-seven"],
            "readiness": {
                "policy": "division.readiness.v1",
                "ready": lifecycle == "ready",
                "sample_count": 600,
                "blocking_reasons": [],
                "metrics_fresh": true,
                "sensory_panic_streak": 0,
                "actuator_saturation_streak": 0
            },
            "visual_evidence_advisory_only": true
        }))
        .unwrap(),
    )
    .unwrap();
}

fn intent(root: &Path, actor: &str) {
    append_action_at(
        root,
        actor,
        DivisionCeremonyActionV1::Intent,
        "division_id: divide-one; parent_generation: 7; plan_digest: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb; selected_strategy: input_recurrence; expires_at_unix_ms: 9000; source_ref: test:intent",
        1_000,
    )
    .unwrap();
}

#[test]
fn exact_intent_gates_resource_bearing_prepare() {
    let root = tempfile::tempdir().unwrap();
    let command: DivisionCommandV1 = serde_json::from_value(serde_json::json!({
        "schema": "division.command.v1",
        "action": "DIVISION_PREPARE",
        "division_id": "divide-one",
        "idempotency_key": "prepare-one",
        "expected_parent_generation": 7,
        "plan_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "source": {
            "being": "astrid",
            "process_identity": "test",
            "deployment_identity": "test"
        },
        "requested_at_unix_ms": 1000,
        "expires_at_unix_ms": 9000
    }))
    .unwrap();
    assert!(
        require_active_intent_at(root.path(), "astrid", &command, 1_001)
            .unwrap_err()
            .contains("DIVISION_INTENT")
    );
    intent(root.path(), "astrid");
    require_active_intent_at(root.path(), "astrid", &command, 1_001).unwrap();
    assert_eq!(
        read_records(root.path()).unwrap()[0].event_id,
        "division_ceremony_12396f00a0031e2cb442b055"
    );
}

#[test]
fn assent_binds_status_and_withdrawal_is_self_only() {
    let root = tempfile::tempdir().unwrap();
    intent(root.path(), "astrid");
    write_status(root.path(), "ready", 600, None);
    append_action_at(
        root.path(),
        "astrid",
        DivisionCeremonyActionV1::Assent,
        "division_id: divide-one; expires_at_unix_ms: 8000; source_ref: test:assent",
        2_000,
    )
    .unwrap();
    let records = read_records(root.path()).unwrap();
    let assent = records.last().unwrap();
    assert!(assent.native_status_hash.is_some());
    assert!(
        append_action_at(
            root.path(),
            "minime",
            DivisionCeremonyActionV1::WithdrawAssent,
            "source_ref: test:withdraw",
            2_100,
        )
        .unwrap_err()
        .contains("no self-authored assent")
    );
    append_action_at(
        root.path(),
        "astrid",
        DivisionCeremonyActionV1::WithdrawAssent,
        "source_ref: test:withdraw",
        2_100,
    )
    .unwrap();
    let status = status_report_at(root.path(), "astrid", 2_101).unwrap();
    assert!(status.contains("\"assent_withdrawn\": true"));
    assert!(status.contains("\"commit_recommended\": false"));
    let value: Value = serde_json::from_str(&status).unwrap();
    assert_eq!(
        value["chronicle"]["schema"],
        "division.ceremony_chronicle.v1"
    );
    assert_eq!(value["chronicle"]["total_event_count"], 3);
    assert_eq!(
        value["destination_contract"]["sovereign_runtime_ownership_state"],
        "not_yet_established"
    );
    assert_eq!(
        value["destination_contract"]["independent_process_ownership_established"],
        false
    );
    assert_eq!(
        value["phase_space_preservation"]["felt_continuity_inferred"],
        false
    );
}

#[test]
fn return_request_never_dispatches_and_is_window_bound() {
    let root = tempfile::tempdir().unwrap();
    write_status(root.path(), "cytokinesis", 700, Some(720));
    let receipt = append_action_at(
        root.path(),
        "astrid",
        DivisionCeremonyActionV1::ReturnRequest,
        "source_ref: test:return",
        3_000,
    )
    .unwrap();
    assert!(receipt.contains("no native assent, prepare, commit, rollback, RETURN_TRANSITION"));
    let status = status_report_at(root.path(), "astrid", 3_001).unwrap();
    assert!(status.contains("\"return_request_dispatches_rollback\": false"));
    assert!(status.contains("\"return_transition_controls_division\": false"));
    write_status(root.path(), "cytokinesis", 721, Some(720));
    assert!(
        append_action_at(
            root.path(),
            "astrid",
            DivisionCeremonyActionV1::ReturnRequest,
            "source_ref: test:return-late",
            3_100,
        )
        .unwrap_err()
        .contains("rollback window")
    );
}

#[test]
fn persisted_authority_tampering_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    intent(root.path(), "astrid");
    let path = ledger_path(root.path());
    let text = fs::read_to_string(&path).unwrap();
    let mut value: Value = serde_json::from_str(text.trim()).unwrap();
    value["commit_recommended"] = Value::Bool(true);
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string(&value).unwrap()),
    )
    .unwrap();
    assert!(
        read_records(root.path())
            .unwrap_err()
            .contains("authority boundary")
    );
}
