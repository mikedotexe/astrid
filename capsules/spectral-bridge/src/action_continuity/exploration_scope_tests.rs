use super::*;

fn fixture() -> (
    tempfile::TempDir,
    ActionContinuityStore,
    ResearchThread,
    ExperimentRecord,
) {
    let root = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(root.path().join("threads"));
    let experiment = store
        .start_experiment(None, "Synthetic question", "What does this check consume?")
        .unwrap();
    let thread = store.current_thread().unwrap().unwrap();
    (root, store, thread, experiment)
}

#[test]
fn expressive_topics_and_notebook_questions_do_not_require_research_permission() {
    let (_root, store, thread, _) = fixture();
    let before = fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap();
    let telemetry: SpectralTelemetry = serde_json::from_value(json!({
        "t_ms": 0, "eigenvalues": [1.0], "fill_ratio": 0.68
    }))
    .unwrap();
    for command in [
        "CREATE a poem about control and the shape of a pulse",
        "INITIATE a thought about lambda and influence",
        "SELF a thought about lambda and influence",
        "REVISE then inject a different metaphor",
        "DAYDREAM then shift the direction of this story",
        "ASPIRATION then spread the idea",
        "EXPERIMENT_START Control :: Does amplitude alter the result?",
        "EXPERIMENT_BRANCH Pulse :: What evidence would distinguish it?",
        "SHADOW_DIALOGUE pulse response",
        "SHADOW_RESPONSE control",
    ] {
        assert!(ordinary_exploration(command), "{command}");
        assert!(
            store
                .charter_required_guard_assessment(command)
                .unwrap()
                .is_none(),
            "{command}"
        );
        assert!(
            store
                .research_budget_guard_assessment(command, 68.0, &telemetry)
                .unwrap()
                .is_none(),
            "{command}"
        );
    }
    for command in [
        "PERTURB SPREAD",
        "RUN_PYTHON code",
        "CONTROL_APPLY_LEASE change",
        "UNRECOGNIZED pulse",
    ] {
        assert!(!ordinary_exploration(command), "{command}");
    }
    assert!(
        store
            .charter_required_guard_assessment("PERTURB SPREAD")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        fs::read(store.thread_dir(&thread.thread_id).join("thread.json")).unwrap(),
        before
    );
    assert!(!store.authority_gate_path(&thread.thread_id).exists());
}

#[test]
fn research_eligibility_describes_budget_creation_not_writing_or_execution() {
    let (_root, store, thread, experiment) = fixture();
    for (request, eligible) in [
        (
            json!({"scope":"read_only_research", "purpose":"Synthetic inspection"}),
            true,
        ),
        (
            json!({"scope":"control_envelope", "purpose":"Synthetic inspection"}),
            false,
        ),
        (json!({"scope":"read_only_research", "purpose":" "}), false),
    ] {
        let result = store.research_budget_eligibility(&thread, &experiment, &request, &json!({}));
        assert_eq!(result["eligible"], eligible);
        assert_eq!(result["eligibility_scope"], "research_budget_creation");
        assert_eq!(result["disabled_actions_scope"], "this_budget_only");
        for field in [
            "action_admission_evaluated",
            "execution_authorized",
            "publication_evaluated",
        ] {
            assert_eq!(result[field], false, "{field}");
        }
    }
    let receipt = store
        .experiment_research_budget_request_command(
            None,
            "current :: purpose: Synthetic inspection; allowed_sources: local",
            json!({}),
        )
        .unwrap();
    assert!(receipt.contains("Budget creation only"));
    assert!(receipt.contains("ordinary exploration does not require this budget"));
}

#[test]
fn explicit_return_selects_paused_context_without_executing_or_rewriting_its_plan() {
    let (_root, store, mut thread, mut experiment) = fixture();
    experiment.status = "paused".into();
    experiment.planned_next = Some(format!(
        "EXPERIMENT_CHARTER {} :: saved author text",
        experiment.experiment_id
    ));
    store
        .persist_experiment_update(None, &mut thread, &experiment, false)
        .unwrap();
    thread.current_next = Some("WRITE STATUS".into());
    store.write_thread(&thread).unwrap();
    let history = fs::read(store.experiments_path(&thread.thread_id)).unwrap();
    let receipt = store
        .experiment_resume_command(None, Some(&experiment.experiment_id))
        .unwrap();
    assert!(
        receipt.starts_with("Resumed experiment context"),
        "{receipt}"
    );
    assert!(receipt.contains("not executed by resume"));
    let selected = store.read_thread(&thread.thread_id).unwrap();
    assert_eq!(
        selected.active_experiment_id.as_deref(),
        Some(experiment.experiment_id.as_str())
    );
    assert_eq!(selected.current_next.as_deref(), Some("WRITE STATUS"));
    let resumed_history = fs::read(store.experiments_path(&thread.thread_id)).unwrap();
    assert!(resumed_history.starts_with(&history));
    let resumed = store
        .resolve_experiment(&selected, Some(&experiment.experiment_id))
        .unwrap();
    assert_eq!(resumed.status, "active");
    assert_eq!(resumed.planned_next, experiment.planned_next);
    store
        .experiment_resume_command(None, Some(&experiment.experiment_id))
        .unwrap();
    assert_eq!(
        fs::read(store.experiments_path(&thread.thread_id)).unwrap(),
        resumed_history
    );
    assert_eq!(
        store.read_thread(&thread.thread_id).unwrap().current_next,
        selected.current_next
    );
    assert!(!store.authority_gate_path(&thread.thread_id).exists());
    assert!(
        store
            .recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 4)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .charter_required_guard_assessment("PERTURB SPREAD")
            .unwrap()
            .is_some()
    );
}

#[test]
fn paused_return_projection_does_not_invent_a_charter_or_hold_from_topic_words() {
    let (_root, _store, mut thread, mut experiment) = fixture();
    experiment.status = "paused".into();
    let resume = format!("EXPERIMENT_RESUME {}", experiment.experiment_id);
    experiment.planned_next = Some(resume.clone());
    thread.active_experiment_id = None;
    thread.experiment_summary = Some(experiment_summary(&experiment));
    thread.current_next =
        Some("EXPERIMENT_PLAN current :: compare CONTROL and pulse source definitions".into());
    let before = serde_json::to_value(&thread).unwrap();
    let projected = last_experiment_summary_v1(&thread).unwrap();
    assert_eq!(projected["primary_return_next"], resume);
    assert_eq!(projected["resume_next"], resume);
    assert_eq!(projected["return_kind"], "resume");
    assert!(projected.get("projection_guard_v1").is_none());
    assert_eq!(serde_json::to_value(thread).unwrap(), before);
}

#[test]
fn paused_history_never_replaces_a_newer_unrelated_choice_during_reconciliation() {
    let (_root, store, mut thread, mut experiment) = fixture();
    experiment.status = "paused".into();
    experiment.planned_next = Some(format!(
        "EXPERIMENT_CHARTER {} :: retained",
        experiment.experiment_id
    ));
    store
        .persist_experiment_update(None, &mut thread, &experiment, false)
        .unwrap();
    thread.current_next = Some("WRITE STATUS".into());
    store.write_thread(&thread).unwrap();
    let restored = store.read_thread(&thread.thread_id).unwrap();
    assert_eq!(restored.current_next.as_deref(), Some("WRITE STATUS"));
    assert!(restored.active_experiment_id.is_none());
    assert_eq!(restored.experiment_summary.unwrap()["status"], "paused");
    let rendered = fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md")).unwrap();
    assert!(
        rendered.contains("Current NEXT: WRITE STATUS\n"),
        "{rendered}"
    );
    thread.current_next = None;
    store.write_thread(&thread).unwrap();
    let rendered = fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md")).unwrap();
    assert!(
        rendered.contains("Current NEXT: (none yet)\n"),
        "{rendered}"
    );
}

#[test]
fn return_keeps_held_allowance_and_concurrent_retries_append_once() {
    let (_root, store, mut thread, mut experiment) = fixture();
    experiment.status = "paused".into();
    experiment.planned_next = Some("THREAD_STATUS current".into());
    store
        .persist_experiment_update(None, &mut thread, &experiment, false)
        .unwrap();
    let authority = store.authority_gate_path(&thread.thread_id);
    let approval = store.research_budget_record("research_budget_approval", "held_fixture", &thread, &experiment, &json!({}),
        json!({"scope":"read_only_research", "status":"active", "max_actions":4, "allowed_sources":["local"], "expires_at_unix_s":4_102_444_800u64}));
    let hold = store.research_budget_record(
        "research_budget_review",
        "held_fixture",
        &thread,
        &experiment,
        &json!({}),
        json!({"outcome":"hold"}),
    );
    store.append_jsonl(&authority, &approval).unwrap();
    store.append_jsonl(&authority, &hold).unwrap();
    let before = fs::read(&authority).unwrap();
    let rows = store.read_experiments(&thread.thread_id).unwrap().len();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let root = store.root().to_path_buf();
                let id = experiment.experiment_id.clone();
                scope.spawn(move || {
                    ActionContinuityStore::new(root).experiment_resume_command(None, Some(&id))
                })
            })
            .collect();
        for worker in workers {
            assert!(worker.join().unwrap().is_ok());
        }
    });
    assert_eq!(
        store.read_experiments(&thread.thread_id).unwrap().len(),
        rows.saturating_add(1)
    );
    assert_eq!(fs::read(&authority).unwrap(), before);
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "MIKE_READ fixture.txt",
                "held-return",
                &json!({})
            )
            .is_err()
    );
    assert_eq!(fs::read(&authority).unwrap(), before);
    assert!(
        store
            .recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 4)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn corrupt_experiment_history_refuses_return_without_overwrite() {
    let (_root, store, thread, experiment) = fixture();
    let path = store.experiments_path(&thread.thread_id);
    let mut bytes = fs::read(&path).unwrap();
    let thread_path = store.thread_dir(&thread.thread_id).join("thread.json");
    let thread_before = fs::read(&thread_path).unwrap();
    bytes.extend_from_slice(b"{\"status\":");
    fs::write(&path, &bytes).unwrap();
    assert!(
        store
            .experiment_resume_command(None, Some(&experiment.experiment_id))
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read(thread_path).unwrap(), thread_before);
}

#[test]
fn foreign_or_linked_history_cannot_become_selected_context() {
    for linked in [false, true] {
        let (_root, store, thread, mut experiment) = fixture();
        let local_id = experiment.experiment_id.clone();
        let path = store.experiments_path(&thread.thread_id);
        if linked {
            let target = path.with_file_name("retained-history.jsonl");
            fs::rename(&path, &target).unwrap();
            std::os::unix::fs::symlink(target, &path).unwrap();
        } else {
            experiment.experiment_id = "exp_minime_foreign".into();
            store.append_jsonl(&path, &experiment).unwrap();
        }
        let before = fs::read(&path).unwrap();
        let thread_path = store.thread_dir(&thread.thread_id).join("thread.json");
        let thread_before = fs::read(&thread_path).unwrap();
        assert!(
            store
                .experiment_resume_command(None, Some(&local_id))
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read(thread_path).unwrap(), thread_before);
        assert_eq!(path.is_symlink(), linked);
    }
}
