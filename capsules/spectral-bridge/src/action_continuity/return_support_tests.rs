use super::*;

#[test]
fn retained_support_is_exact_and_corruption_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path());
    let thread = store.create_thread(None, "Synthetic", None).unwrap();
    let experiment = store.start_experiment(None, "Saved", "Question?").unwrap();
    let path = store.experiments_path(&thread.thread_id);
    let original = fs::read(&path).unwrap();
    let support = store.saved_return_support(&thread.thread_id, Some(&experiment.experiment_id));
    assert_eq!(support["record_status"], "verified_readable");
    assert_eq!(support["execution_eligibility"], "not_evaluated");
    assert_eq!(fs::read(&path).unwrap(), original);
    let absent = store.saved_return_support(&thread.thread_id, Some("exp_astrid_missing"));
    assert_eq!(absent["record_status"], "unavailable");
    assert_eq!(absent["saved_next_action"], "unknown");
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{broken tail")
        .unwrap();
    let broken = fs::read(&path).unwrap();
    assert_eq!(
        store.saved_return_support(&thread.thread_id, Some(&experiment.experiment_id))["record_status"],
        "unavailable"
    );
    assert_eq!(fs::read(&path).unwrap(), broken);
}

#[test]
fn comparison_is_read_only_and_preserves_parked_experiments() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path());
    let thread = store.create_thread(None, "Synthetic", None).unwrap();
    let mut parent = store
        .start_experiment(None, "Decay", "Does this refer to the same mechanism?")
        .unwrap();
    parent.status = "paused".into();
    store
        .append_jsonl(&store.experiments_path(&thread.thread_id), &parent)
        .unwrap();
    let child = store
        .start_experiment(None, "Decay too", "Different mechanism?")
        .unwrap();
    let before_thread = fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap();
    let before_experiments = fs::read(store.experiments_path(&thread.thread_id)).unwrap();
    let before_index = fs::read(store.index_path()).unwrap();
    let before_dossier = fs::read(store.dossier_path(&thread.thread_id)).ok();
    for _ in 0..2 {
        let text = store
            .experiment_compare_command(Some(&format!(
                "{} WITH {}",
                parent.experiment_id, child.experiment_id
            )))
            .unwrap();
        assert!(text.contains("record_sha256"));
        assert!(text.contains("Juxtaposition does not establish"));
        assert!(!text.contains("Suggested next:"));
        assert!(!text.contains("returnability="));
        assert!(text.contains("paused"));
    }
    assert_eq!(
        fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap(),
        before_thread
    );
    assert_eq!(
        fs::read(store.experiments_path(&thread.thread_id)).unwrap(),
        before_experiments
    );
    assert_eq!(fs::read(store.index_path()).unwrap(), before_index);
    assert_eq!(
        fs::read(store.dossier_path(&thread.thread_id)).ok(),
        before_dossier
    );
    assert!(
        store
            .experiment_compare_command(Some("exp_astrid_missing WITH current"))
            .unwrap()
            .contains("unavailable")
    );
}

#[test]
fn comparison_does_not_initialize_an_empty_store() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("not-created");
    let store = ActionContinuityStore::new(&root);
    assert!(
        store
            .experiment_compare_command(None)
            .unwrap()
            .contains("unavailable")
    );
    assert!(!root.exists());
}

#[test]
fn comparison_refuses_corrupt_tail_and_snapshot_detects_change() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path());
    let thread = store.create_thread(None, "Synthetic", None).unwrap();
    store.start_experiment(None, "Record", "Question?").unwrap();
    let path = store.experiments_path(&thread.thread_id);
    let mut snapshot = retained_inspection::Snapshot::default();
    assert!(!snapshot.records::<Value>(&path).unwrap().is_empty());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{broken tail")
        .unwrap();
    assert!(snapshot.validate().is_err());
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .experiment_compare_command(None)
            .unwrap()
            .contains("unavailable")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn legacy_native_return_cues_are_not_replayed() {
    assert!(native_return_cue_line(&json!({"native_return_cue":"name felt texture"})).is_empty());
}

#[test]
fn comparison_bounds_runs_and_keeps_interpretation_separate() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path());
    let thread = store.create_thread(None, "Synthetic", None).unwrap();
    let record = store
        .start_experiment(None, "Decay", "Metaphor or mechanism?")
        .unwrap();
    for number in 0..6 {
        store.append_jsonl(&store.experiment_runs_path(&thread.thread_id), &json!({
            "schema_version": 1, "run_id": format!("run_{number}"), "experiment_id": record.experiment_id,
            "action_text": "REST", "gate_decision": {}, "pre_state": {}, "post_state": {},
            "updated_at": "synthetic", "suggested_next": null,
            "status": "recorded", "stage": "observation", "created_at": "synthetic",
            "result_summary": "This is a stored receipt", "interpretation": "The resemblance may fail",
            "artifacts": [{"schema_version":1, "artifact_id":"unverified", "action_id":"synthetic", "visibility":"private", "kind":"synthetic", "path_or_uri":"synthetic://not-read", "summary":"not inspected"}]
        })).unwrap();
    }
    let comparison = store.experiment_compare_command(None).unwrap();
    assert!(!comparison.contains("run_0"));
    assert!(!comparison.contains("run_1"));
    assert!(comparison.contains("run_5"));
    assert!(comparison.contains("authored_interpretation"));
    assert!(comparison.contains("not_verified"));
    assert!(comparison.contains("The resemblance may fail"));
}

#[test]
fn no_selected_experiment_does_not_borrow_historical_return_support() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path());
    let mut thread = store.create_thread(None, "Synthetic", None).unwrap();
    store
        .start_experiment(None, "Historical", "Retained?")
        .unwrap();
    thread.active_experiment_id = None;
    thread.current_next = None;
    store.write_thread(&thread).unwrap();
    let support = store.saved_return_support(&thread.thread_id, None);
    assert_eq!(support["record_status"], "absent");
    assert_eq!(support["saved_next_action"], "absent");
    assert!(
        store
            .experiment_compare_command(None)
            .unwrap()
            .contains("no selected experiment")
    );
}

#[test]
fn unreadable_or_foreign_thread_is_unknown_not_absent() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path());
    let mut thread = store.create_thread(None, "Synthetic", None).unwrap();
    thread.system_origin = "minime".into();
    store.write_thread(&thread).unwrap();
    let path = store.thread_dir(&thread.thread_id).join("thread.json");
    let original = fs::read(&path).unwrap();
    let support = store.saved_return_support(&thread.thread_id, None);
    assert_eq!(support["record_status"], "unavailable");
    assert_eq!(support["saved_next_action"], "unknown");
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(
        store.saved_return_support("missing", None)["record_status"],
        "unavailable"
    );
}

#[test]
fn experiment_inspection_never_initializes_or_refreshes_authored_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("absent");
    let store = ActionContinuityStore::new(&root);
    assert!(
        store
            .experiment_status(None)
            .unwrap()
            .contains("No selected")
    );
    assert!(
        store
            .experiment_review(None)
            .unwrap()
            .contains("No selected")
    );
    assert!(!root.exists());
    let thread = store.create_thread(None, "Synthetic", None).unwrap();
    store.start_experiment(None, "Saved", "Question?").unwrap();
    let path = store.thread_dir(&thread.thread_id).join("thread.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["projection_freshness_v1"] = Value::Null;
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    store.experiment_status(None).unwrap();
    store.experiment_review(None).unwrap();
    store.experiment_status(Some("exp_minime_missing")).unwrap();
    store.experiment_review(Some("exp_minime_missing")).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
}
