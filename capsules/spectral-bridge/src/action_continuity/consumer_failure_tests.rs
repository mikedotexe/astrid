//! Characterize file-first persistence and actual consumers with synthetic state.
use super::*;

fn fixture() -> (
    tempfile::TempDir,
    ActionContinuityStore,
    ResearchThread,
    ExperimentRecord,
) {
    let root = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(root.path().join("threads-store"));
    let thread = store.create_thread(None, "Synthetic review", None).unwrap();
    let experiment = store
        .start_experiment(None, "Synthetic claim", "Does a label establish truth?")
        .unwrap();
    let thread = store.read_thread(&thread.thread_id).unwrap();
    (root, store, thread, experiment)
}

#[test]
fn accepted_candidate_satisfies_advisory_classification_but_not_execution_eligibility() {
    let (_root, store, thread, mut experiment) = fixture();
    experiment.charter_v1 =
        Some(json!({"hypothesis":"A deliberately unsupported synthetic claim."}));
    experiment.evidence_v1 = Some(json!({"unverified":"A stored reference only."}));
    experiment.workbench_candidates_v1 = Some(json!({
        "charter":{"status":"candidate"}, "evidence":{"status":"candidate"}
    }));
    assert!(valid_experiment_charter(experiment.charter_v1.as_ref()));
    assert!(!lifecycle_valid_charter_value(
        experiment.charter_v1.as_ref()
    ));
    let refreshed = store
        .refresh_workbench_candidates(None, &thread, &experiment, None, None, "synthetic-test")
        .unwrap();
    let candidates = refreshed.workbench_candidates_v1.as_ref().unwrap();
    assert_eq!(candidates["charter"]["status"], "accepted");
    assert_eq!(candidates["evidence"]["status"], "accepted");
    assert_eq!(refreshed.charter_v1, experiment.charter_v1);
    assert_eq!(refreshed.evidence_v1, experiment.evidence_v1);
    // The advisory classifier uses the looser charter predicate, not execution eligibility.
    assert!(
        store
            .charter_required_guard_assessment("PERTURB SPREAD")
            .unwrap()
            .is_none()
    );
    let eligibility = store.authority_gate_eligibility(
        &thread,
        &refreshed,
        &json!({"scope":"semantic_microdose", "payload":"Synthetic proposal only."}),
        &json!({}),
    );
    assert_eq!(eligibility["eligible"], false);
    assert_eq!(eligibility["approval_required"], "being_plus_steward");
    let missing = eligibility["missing_requirements"].as_array().unwrap();
    for requirement in [
        "lifecycle_valid_charter",
        "meaningful_evidence",
        "read_only_rehearsal",
    ] {
        assert!(missing.contains(&json!(requirement)), "{requirement}");
    }
    assert!(!store.authority_gate_path(&thread.thread_id).exists());
}

#[test]
fn failed_history_append_does_not_advance_thread_or_mutate_the_input_thread() {
    let (_root, store, mut thread, mut experiment) = fixture();
    let log = store.experiments_path(&thread.thread_id);
    let retained = log.with_extension("retained");
    let before_log = fs::read(&log).unwrap();
    let before_thread = fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap();
    let before_value = serde_json::to_value(&thread).unwrap();
    fs::rename(&log, &retained).unwrap();
    fs::create_dir(&log).unwrap();
    experiment.status = "paused".into();
    let error = store
        .persist_experiment_update(None, &mut thread, &experiment, false)
        .unwrap_err();
    let receipt = save_receipts::receipt_from_error(&error);
    assert_eq!(receipt["history_append"], "failed_outcome_unknown");
    assert_eq!(receipt["thread"]["thread_json"], "not_attempted");
    assert_eq!(receipt["retry_authorized"], false);
    assert!(!error.to_string().contains("Does a label establish truth?"));
    assert_eq!(serde_json::to_value(thread).unwrap(), before_value);
    assert_eq!(fs::read(retained).unwrap(), before_log);
    assert_eq!(
        fs::read(store.thread_dir(&experiment.thread_id).join("thread.json")).unwrap(),
        before_thread
    );
}

#[test]
fn appended_experiment_survives_thread_write_failure_and_read_reconciles_without_reappending() {
    let (_root, store, mut thread, mut experiment) = fixture();
    let log = store.experiments_path(&thread.thread_id);
    let before_log = fs::read(&log).unwrap();
    let path = store.thread_dir(&thread.thread_id).join("thread.json");
    let retained = path.with_extension("retained");
    let before_thread = fs::read(&path).unwrap();
    fs::rename(&path, &retained).unwrap();
    fs::create_dir(&path).unwrap();
    experiment.status = "paused".into();
    experiment.updated_at = "2026-01-02T00:00:00Z".into();
    let error = store
        .persist_experiment_update(None, &mut thread, &experiment, false)
        .unwrap_err();
    let receipt = save_receipts::receipt_from_error(&error);
    assert_eq!(receipt["history_append"], "returned_ok");
    assert_eq!(receipt["thread"]["projection"], "returned_ok");
    assert_eq!(receipt["thread"]["thread_json"], "failed_outcome_unknown");
    assert_eq!(receipt["thread"]["next_md"], "not_attempted");
    let outcome = NextActionOutcome::continuity_result(Err(error), "protected_summary");
    assert!(!outcome.handled);
    assert_eq!(outcome.status, "failed");
    assert_eq!(outcome.stage, "save_incomplete");
    assert!(outcome.suggested_next.is_none());
    assert!(
        outcome
            .outcome_summary
            .contains("continuity_save_receipt_v1")
    );
    let partial_log = fs::read(&log).unwrap();
    assert!(partial_log.starts_with(&before_log));
    assert!(partial_log.len() > before_log.len());
    assert_eq!(fs::read(&retained).unwrap(), before_thread);
    assert_eq!(thread.active_experiment_id, None); // The failed call also changed its in-memory argument.
    fs::remove_dir(&path).unwrap();
    fs::rename(&retained, &path).unwrap();
    let recovered = store.read_thread(&thread.thread_id).unwrap();
    assert!(recovered.active_experiment_id.is_none());
    assert_eq!(recovered.experiment_summary.unwrap()["status"], "paused");
    assert_eq!(fs::read(&log).unwrap(), partial_log);
}

#[test]
fn mirror_write_failure_is_best_effort_not_an_atomic_file_database_commit() {
    let (root, store, mut thread, mut experiment) = fixture();
    let store = store.with_save_warnings();
    let db_path = root.path().join("synthetic.sqlite3");
    let db = BridgeDb::open(&db_path).unwrap();
    db.mirror_action_thread(&thread.thread_id, "{\"synthetic\":\"old mirror\"}")
        .unwrap();
    drop(db);
    let read_only = BridgeDb::open_read_only(&db_path).unwrap();
    assert!(
        read_only
            .mirror_action_thread(&thread.thread_id, "{}")
            .is_err()
    );
    experiment.status = "paused".into();
    store
        .persist_experiment_update(Some(&read_only), &mut thread, &experiment, false)
        .unwrap();
    let saved: ResearchThread = serde_json::from_slice(
        &fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved.experiment_summary.unwrap()["status"], "paused");
    let conn =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mirror: String = conn
        .query_row(
            "SELECT payload FROM action_threads WHERE thread_id = ?1",
            [&thread.thread_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(mirror, "{\"synthetic\":\"old mirror\"}");
    let message = store.finish_save_warnings(Ok("Saved.".into())).unwrap();
    assert!(message.contains("\"database_mirror\":\"failed_outcome_unknown\""));
    assert!(message.contains("\"thread_json\":\"returned_ok\""));
    assert!(message.contains("\"retry_authorized\":false"));
    assert!(!message.contains("Does a label establish truth?"));
    assert_eq!(
        store
            .finish_save_warnings(Ok("Next command.".into()))
            .unwrap(),
        "Next command."
    );
}

#[test]
fn failed_thread_write_retains_run_evidence_without_claiming_next_was_saved() {
    let (_root, store, thread, experiment) = fixture();
    let path = store.thread_dir(&thread.thread_id).join("thread.json");
    let retained = path.with_extension("retained");
    let before = fs::read(&path).unwrap();
    fs::rename(&path, &retained).unwrap();
    fs::create_dir(&path).unwrap();
    let result = store.append_experiment_run(
        None,
        &thread,
        &experiment,
        "SYNTHETIC_FAILED_ACTION",
        "execution",
        "failed",
        json!({"allowed":false}),
        json!({}),
        json!({}),
        Vec::new(),
        "Synthetic executed failure.",
        "Unverified interpretation.",
        Some("REST".into()),
        "synthetic-test",
    );
    let receipt = save_receipts::receipt_from_error(&result.unwrap_err());
    assert_eq!(receipt["record_kind"], "experiment_run");
    assert_eq!(receipt["history_append"], "returned_ok");
    assert_eq!(receipt["thread"]["thread_json"], "failed_outcome_unknown");
    let log = store.experiment_runs_path(&thread.thread_id);
    let bytes = fs::read(&log).unwrap();
    let records: Vec<ExperimentRunRecord> = String::from_utf8(bytes.clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, "failed");
    assert_eq!(records[0].suggested_next.as_deref(), Some("REST"));
    assert_eq!(fs::read(&retained).unwrap(), before);
    fs::remove_dir(&path).unwrap();
    fs::rename(&retained, &path).unwrap();
    let inspected = store.read_thread(&thread.thread_id).unwrap();
    assert_eq!(inspected.current_next, thread.current_next);
    assert_ne!(inspected.current_next.as_deref(), Some("REST"));
    assert_eq!(fs::read(log).unwrap(), bytes);
}

#[test]
fn later_projection_failures_identify_the_saved_thread_without_claiming_all_outputs_saved() {
    for failure in ["next.md", "index.json"] {
        let (_root, store, mut thread, mut experiment) = fixture();
        let path = if failure == "index.json" {
            store.index_path()
        } else {
            store.thread_dir(&thread.thread_id).join(failure)
        };
        fs::rename(&path, path.with_extension("retained")).unwrap();
        fs::create_dir(&path).unwrap();
        experiment.status = "paused".into();
        let error = store
            .persist_experiment_update(None, &mut thread, &experiment, false)
            .unwrap_err();
        let receipt = save_receipts::receipt_from_error(&error);
        assert_eq!(receipt["history_append"], "returned_ok");
        assert_eq!(receipt["thread"]["thread_json"], "returned_ok");
        if failure == "next.md" {
            assert_eq!(receipt["thread"]["next_md"], "failed_outcome_unknown");
            assert_eq!(receipt["thread"]["index_read"], "not_attempted");
        } else {
            assert_eq!(receipt["thread"]["next_md"], "returned_ok");
            assert_eq!(receipt["thread"]["index_read"], "failed_outcome_unknown");
        }
        assert_eq!(receipt["thread"]["index_write"], "not_attempted");
        assert_eq!(receipt["database_mirror"], "not_requested");
        let saved: Value = serde_json::from_slice(
            &fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["experiment_summary"]["status"], "paused");
    }
}

#[test]
fn mirror_warning_collection_is_operation_local_and_bounded() {
    let (root, store, mut thread, experiment) = fixture();
    let store = store.with_save_warnings();
    let path = root.path().join("mirror.sqlite3");
    drop(BridgeDb::open(&path).unwrap());
    let db = BridgeDb::open_read_only(&path).unwrap();
    for _ in 0..10 {
        store
            .persist_experiment_update(Some(&db), &mut thread, &experiment, true)
            .unwrap();
    }
    let message = store.finish_save_warnings(Ok("Saved.".into())).unwrap();
    assert_eq!(
        message
            .matches("\"schema\":\"continuity_save_receipt_v1\"")
            .count(),
        8
    );
    assert!(message.contains("\"omitted_warning_count\":2"));
    let other = ActionContinuityStore::new(store.root()).with_save_warnings();
    assert_eq!(
        other.finish_save_warnings(Ok("Unrelated.".into())).unwrap(),
        "Unrelated."
    );
}

#[test]
fn real_command_wrapper_surfaces_mirror_warning_once_without_repeating_the_action() {
    let (root, store, thread, experiment) = fixture();
    let _scope = scoped_test_action_continuity_root(store.root());
    let path = root.path().join("mirror.sqlite3");
    drop(BridgeDb::open(&path).unwrap());
    let db = BridgeDb::open_read_only(&path).unwrap();
    let telemetry = serde_json::from_value(json!({
        "t_ms":0,"eigenvalues":[4.0,2.0,1.0],"fill_ratio":0.68
    }))
    .unwrap();
    let command = format!(
        "EXPERIMENT_CLOSE {} :: Synthetic completed review.",
        experiment.experiment_id
    );
    let result =
        handle_thread_next_action(&db, "EXPERIMENT_CLOSE", &command, "", &telemetry, 68.0).unwrap();
    let outcome = NextActionOutcome::continuity_result(result, "protected_summary");
    assert!(outcome.handled);
    assert!(
        outcome
            .outcome_summary
            .contains("\"database_mirror\":\"failed_outcome_unknown\"")
    );
    assert!(
        outcome
            .outcome_summary
            .contains("\"retry_authorized\":false")
    );
    let history = fs::read(store.experiments_path(&thread.thread_id)).unwrap();
    let inspection = handle_thread_next_action(
        &db,
        "EXPERIMENT_STATUS",
        &format!("EXPERIMENT_STATUS {}", experiment.experiment_id),
        "",
        &telemetry,
        68.0,
    )
    .unwrap()
    .unwrap();
    assert!(!inspection.contains("Save warnings"));
    assert_eq!(
        fs::read(store.experiments_path(&thread.thread_id)).unwrap(),
        history
    );
}
