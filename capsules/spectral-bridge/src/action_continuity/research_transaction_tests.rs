use super::*;

#[test]
fn diagnostic_labels_and_aliases_are_not_control_requests_or_inherited_debits() {
    let (store, _, path) = fixture("diagnostic-labels");
    let before = fs::read(&path).unwrap();
    let telemetry: SpectralTelemetry = serde_json::from_value(json!({
        "t_ms": 0, "eigenvalues": [1.0], "fill_ratio": 0.68
    }))
    .unwrap();
    for base in [
        "SPECTRAL_EXPLORER",
        "DECOMPOSE",
        "VISUALIZE_CASCADE",
        "CASCADE",
        "EXAMINE_CASCADE",
        "INVESTIGATE_CASCADE",
        "EXAMINE_AUDIO",
        "PRESSURE_SOURCE_AUDIT",
        "PRESSURE_SOURCE",
        "STRUCTURAL_PRESSURE",
        "INWARD_PRESSURE",
        "FLUCTUATION_AUDIT",
        "INHABITABLE_FLUCTUATION",
        "EIGENTRUST",
        "EIGENTRUST_AUDIT",
        "FOOTHOLD_AUDIT",
        "BRACE_AUDIT",
        "AFTERSHOCK_TRACE",
        "TREMOR_RESIDUE",
        "CASCADE_RESIDUE",
        "RESONANCE_FORECAST",
        "FORECAST",
        "PROBABILITIES",
    ] {
        for label in [
            "baseline",
            "pulse response",
            "CONTROL amplitude inject Vec<T>",
        ] {
            let command = format!("{base} {label}");
            assert!(ordinary_exploration(&command), "{command}");
            assert!(
                store
                    .research_budget_guard_assessment(&command, 68.0, &telemetry)
                    .unwrap()
                    .is_none(),
                "{command}"
            );
            assert!(
                store
                    .charter_required_guard_assessment(&command)
                    .unwrap()
                    .is_none(),
                "{command}"
            );
        }
    }
    for command in [
        "RUN_PYTHON code",
        "CONTROL_APPLY_LEASE change",
        "UNRECOGNIZED pulse",
    ] {
        assert!(!ordinary_exploration(command), "{command}");
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn ordinary_exploration_is_not_bound_by_history_or_query_vocabulary() {
    let (store, experiment, path) = fixture("ordinary");
    let original = fs::read(&path).unwrap();
    for tail in [
        "",
        "{\"record_schema\":\"research_budget_v1\",\"record_type\":\"research_budget_review\",\"budget_id\":\"budget_fixture\",\"outcome\":\"hold\"}\n",
        "{\"record_schema\":\"research_budget_v1\",\"record_type\":\"research_budget_review\",\"budget_id\":\"budget_fixture\",\"outcome\":\"close\"}\n",
    ] {
        let mut bytes = original.clone();
        bytes.extend_from_slice(tail.as_bytes());
        fs::write(&path, &bytes).unwrap();
        for command in [
            "SEARCH CONTROL Vec<T> amplitude AND AR_START",
            "RESEARCH control",
            "SELF_STUDY FIND Vec<T>",
            "WRITE STATUS",
            "EXAMINE amplitude",
        ] {
            assert!(ordinary_exploration(command), "{command}");
            assert!(read_only_control_intent_matches(command).is_empty());
            assert!(
                store
                    .charter_required_guard_assessment(command)
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::write(&path, &original).unwrap();
    let ticket = store
        .reserve_research(
            &experiment.experiment_id,
            "SEARCH CONTROL Vec<T> amplitude AND AR_START",
            "literal",
            &json!({}),
        )
        .unwrap();
    store.claim_research_start(&ticket).unwrap();
    assert!(!ordinary_exploration("CONTROL_APPLY_LEASE change"));
    assert!(research_binding_requires_budget("CONTROL_APPLY_LEASE change").is_err());
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn duplicate_json_keys_are_integrity_errors_not_last_wins_authority() {
    let (store, experiment, path) = fixture("duplicate_keys");
    let original = fs::read(&path).unwrap();
    for line in [
        "{\"status\":\"held\",\"status\":\"active\"}\n",
        "{\"nested\":{\"max_actions\":1,\"max_actions\":4}}\n",
    ] {
        let mut bytes = original.clone();
        bytes.extend_from_slice(line.as_bytes());
        fs::write(&path, &bytes).unwrap();
        assert!(
            store
                .reserve_research(
                    &experiment.experiment_id,
                    "SEARCH fixture",
                    "duplicate-json",
                    &json!({})
                )
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_dir_all(store.root()).unwrap();
}

fn fixture(name: &str) -> (ActionContinuityStore, ExperimentRecord, PathBuf) {
    let root =
        std::env::temp_dir().join(format!("astrid_research_tx_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let store = ActionContinuityStore::new(root);
    let experiment = store
        .start_experiment(
            None,
            "Admission fixture",
            "Exact bounded synthetic operations",
        )
        .unwrap();
    let thread = store.current_thread().unwrap().unwrap();
    let path = store.authority_gate_path(&thread.thread_id);
    let approval = store.research_budget_record("research_budget_approval", "budget_fixture", &thread, &experiment, &json!({}),
        json!({"scope": "read_only_research", "status": "active", "max_actions": 4, "allowed_sources": ["web", "local"], "expires_at_unix_s": 4_102_444_800u64}));
    store.append_jsonl(&path, &approval).unwrap();
    (store, experiment, path)
}

fn run(
    store: &ActionContinuityStore,
    experiment: &ExperimentRecord,
    op: &str,
) -> ResearchReservation {
    let ticket = store
        .reserve_research(&experiment.experiment_id, "SEARCH fixture", op, &json!({}))
        .unwrap();
    store
        .transition_research(&ticket, "execution_started")
        .unwrap();
    store.transition_research(&ticket, "failed").unwrap();
    ticket
}

#[test]
fn bound_source_alias_and_canonical_navigation_use_the_same_allowance() {
    let (store, experiment, _) = fixture("source_alias");
    for (index, action) in [
        "OPEN astrid/Cargo.toml 1",
        "SELF_STUDY OPEN astrid/Cargo.toml 1",
        "SELF_STUDY FIND CONTROL Vec<T>",
    ]
    .iter()
    .enumerate()
    {
        assert!(research_binding_requires_budget(action).unwrap());
        let ticket = store
            .reserve_research(
                &experiment.experiment_id,
                action,
                &format!("source-{index}"),
                &json!({}),
            )
            .unwrap();
        store.claim_research_start(&ticket).unwrap();
        store.transition_research(&ticket, "completed").unwrap();
    }
    for action in [
        "SELF_STUDY WRITE START private",
        "SELF_STUDY EXPERIMENT RUN x",
        "OPEN",
    ] {
        assert!(research_binding_requires_budget(action).is_err());
    }
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_latest_grant_and_committed_close_are_terminal() {
    let (store, experiment, path) = fixture("latest");
    let approval = research_transactions::strict_rows(&path)
        .unwrap()
        .pop()
        .unwrap();
    for (key, value) in [
        ("max_actions", json!(0)),
        ("expires_at_unix_s", json!(1)),
        ("status", json!("refused")),
        ("experiment_id", json!("other")),
    ] {
        let mut changed = approval.clone();
        changed[key] = value;
        assert!(
            active_research_budget_from_rows(
                &[approval.clone(), changed],
                &experiment.experiment_id
            )
            .is_none()
        );
    }
    let close = json!({"record_schema":"research_budget_v1", "record_type":"research_budget_review",
        "budget_id":"budget_fixture", "outcome":"close"});
    let mut continued = close.clone();
    continued["outcome"] = json!("continue");
    assert!(
        active_research_budget_from_rows(&[approval, close, continued], &experiment.experiment_id)
            .is_none()
    );
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_deferred_worker_rechecks_and_never_replays() {
    use crate::autonomous::next_action::research_jobs::BoundResearchJob;
    let (store, experiment, path) = fixture("deferred");
    let approval = research_transactions::strict_rows(&path)
        .unwrap()
        .pop()
        .unwrap();
    let ticket = store
        .reserve_research(
            &experiment.experiment_id,
            "EXAMINE https://example.org/paper",
            "web-worker",
            &json!({}),
        )
        .unwrap();
    let queued = BoundResearchJob::prepare(&store, ticket.clone()).unwrap();
    let saved = serde_json::to_string(&queued).unwrap();
    let restored: BoundResearchJob = serde_json::from_str(&saved).unwrap();
    let rows = research_transactions::strict_rows(&path).unwrap();
    assert_eq!(rows.last().unwrap()["lifecycle"], "queued");
    assert!(
        !rows
            .iter()
            .any(|row| row["record_type"] == "research_budget_debit")
    );
    let mut changed = approval.clone();
    changed["allowed_sources"] = json!(["local"]);
    store.append_jsonl(&path, &changed).unwrap();
    assert!(restored.start(&store).is_err());
    changed["allowed_sources"] = json!(["web"]);
    changed["expires_at_unix_s"] = json!(1);
    store.append_jsonl(&path, &changed).unwrap();
    assert!(restored.start(&store).is_err());
    store.append_jsonl(&path, &approval).unwrap();
    restored.start(&store).unwrap();
    assert!(restored.start(&store).is_err());
    assert!(
        store
            .transition_research(&ticket, "released_unstarted")
            .is_err()
    );
    restored.finish(&store, false).unwrap();
    assert!(restored.start(&store).is_err());
    let rows = research_transactions::strict_rows(&path).unwrap();
    assert_eq!(
        rows.iter()
            .filter(|row| row["record_type"] == "research_budget_debit")
            .count(),
        1
    );
    assert_eq!(rows.last().unwrap()["lifecycle"], "failed");
    assert_eq!(rows.last().unwrap()["input_delivered"], "unknown");
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn queued_research_rejects_corrupt_or_foreign_experiment_history_before_debit() {
    use crate::autonomous::next_action::research_jobs::BoundResearchJob;
    let (store, experiment, authority) = fixture("queued_corrupt_experiment");
    let ticket = store
        .reserve_research(
            &experiment.experiment_id,
            "SEARCH fixture",
            "queued",
            &json!({}),
        )
        .unwrap();
    let job = BoundResearchJob::prepare(&store, ticket).unwrap();
    let path = store.experiments_path(&experiment.thread_id);
    let original = fs::read(&path).unwrap();
    let authority_before = fs::read(&authority).unwrap();
    let mut tails = vec![
        b"{\"status\":\"held\"".to_vec(),
        b"{}\n".to_vec(),
        b"\n".to_vec(),
    ];
    for (key, value) in [
        ("schema_version", json!(999)),
        ("thread_id", json!("foreign_thread")),
        ("experiment_id", json!("exp_minime_foreign")),
        ("status", json!("held")),
    ] {
        let mut record = serde_json::to_value(&experiment).unwrap();
        record[key] = value;
        let mut bytes = serde_json::to_vec(&record).unwrap();
        bytes.push(b'\n');
        tails.push(bytes);
    }
    for tail in tails {
        let mut damaged = original.clone();
        damaged.extend(tail);
        fs::write(&path, &damaged).unwrap();
        assert!(job.start(&store).is_err());
        assert_eq!(fs::read(&path).unwrap(), damaged);
        assert_eq!(fs::read(&authority).unwrap(), authority_before);
    }
    fs::remove_file(&path).unwrap();
    assert!(job.start(&store).is_err());
    assert!(!path.exists());
    assert_eq!(fs::read(&authority).unwrap(), authority_before);
    fs::write(&path, &original).unwrap();
    // The queued owner is authoritative even after UI selection becomes unreadable.
    fs::write(store.index_path(), b"{unrelated-selection").unwrap();
    job.start(&store).unwrap();
    assert!(job.start(&store).is_err());
    assert_eq!(
        research_transactions::strict_rows(&authority)
            .unwrap()
            .iter()
            .filter(|row| row["record_type"] == "research_budget_debit")
            .count(),
        1
    );
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn queued_research_revalidates_original_thread_owner_and_history_file_type() {
    use crate::autonomous::next_action::research_jobs::BoundResearchJob;
    let (store, experiment, authority) = fixture("queued_owner_history");
    let ticket = store
        .reserve_research(
            &experiment.experiment_id,
            "SEARCH fixture",
            "queued",
            &json!({}),
        )
        .unwrap();
    let job = BoundResearchJob::prepare(&store, ticket).unwrap();
    let authority_before = fs::read(&authority).unwrap();
    let thread_path = store.thread_dir(&experiment.thread_id).join("thread.json");
    let thread_before = fs::read(&thread_path).unwrap();
    let thread: Value = serde_json::from_slice(&thread_before).unwrap();
    for (key, value) in [
        ("schema_version", json!(999)),
        ("system_origin", json!("minime")),
        ("thread_id", json!("foreign_thread")),
    ] {
        let mut invalid = thread.clone();
        invalid[key] = value;
        let bytes = serde_json::to_vec(&invalid).unwrap();
        fs::write(&thread_path, &bytes).unwrap();
        assert!(job.start(&store).is_err());
        assert_eq!(fs::read(&thread_path).unwrap(), bytes);
        assert_eq!(fs::read(&authority).unwrap(), authority_before);
    }
    fs::write(&thread_path, &thread_before).unwrap();
    let history = store.experiments_path(&experiment.thread_id);
    let history_before = fs::read(&history).unwrap();
    fs::remove_file(&history).unwrap();
    let target = store.root().join("missing-experiment-target");
    std::os::unix::fs::symlink(&target, &history).unwrap();
    assert!(job.start(&store).is_err());
    assert!(history.is_symlink());
    assert!(!target.exists());
    fs::write(&target, &history_before).unwrap();
    assert!(job.start(&store).is_err());
    assert_eq!(fs::read(&target).unwrap(), history_before);
    fs::remove_file(&history).unwrap();
    fs::create_dir(&history).unwrap();
    assert!(job.start(&store).is_err());
    fs::remove_dir(&history).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&history)
            .status()
            .unwrap()
            .success()
    );
    assert!(job.start(&store).is_err());
    assert_eq!(fs::read(&authority).unwrap(), authority_before);
    fs::remove_file(&history).unwrap();
    fs::write(&history, &history_before).unwrap();
    let directory = store.thread_dir(&experiment.thread_id);
    let moved = store.root().join("moved-owner");
    fs::rename(&directory, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &directory).unwrap();
    assert!(job.start(&store).is_err());
    assert_eq!(fs::read(&authority).unwrap(), authority_before);
    fs::remove_file(&directory).unwrap();
    fs::rename(&moved, &directory).unwrap();
    job.start(&store).unwrap();
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn native_history_append_rejects_dangling_links_and_special_files_without_creation() {
    use std::os::unix::fs::FileTypeExt;
    let (store, experiment, authority) = fixture("native_history_types");
    let experiment_path = store.experiments_path(&experiment.thread_id);
    for path in [&authority, &experiment_path] {
        fs::remove_file(path).unwrap();
        assert!(research_transactions::strict_rows(path).unwrap().is_empty());
        let target = store.root().join("missing-native-history-target");
        std::os::unix::fs::symlink(&target, path).unwrap();
        let refuse = || {
            assert!(research_transactions::strict_rows(path).is_err());
            assert!(
                store
                    .append_jsonl(path, &json!({"record_type":"test"}))
                    .is_err()
            );
            if path == &authority {
                assert!(
                    append_native_authority_record(path, &json!({"record_type":"test"})).is_err()
                );
            }
        };
        refuse();
        assert!(path.is_symlink());
        assert!(!target.exists());
        fs::write(&target, b"{}\n").unwrap();
        refuse();
        assert_eq!(fs::read(&target).unwrap(), b"{}\n");
        fs::remove_file(path).unwrap();
        fs::remove_file(&target).unwrap();
        fs::create_dir(path).unwrap();
        refuse();
        assert_eq!(fs::read_dir(path).unwrap().count(), 0);
        fs::remove_dir(path).unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
        refuse();
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_fifo());
        fs::remove_file(path).unwrap();
        store
            .append_jsonl(path, &json!({"record_type":"test"}))
            .unwrap();
        assert_eq!(research_transactions::strict_rows(path).unwrap().len(), 1);
    }
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_noncurrent_failure_counts_without_replay() {
    let (store, experiment, path) = fixture("noncurrent");
    store
        .start_experiment(None, "Other experiment", "Different question")
        .unwrap();
    let ticket = run(&store, &experiment, "job-1");
    let rows = research_transactions::strict_rows(&path).unwrap();
    assert_eq!(
        active_research_budget_from_rows(&rows, &experiment.experiment_id).unwrap()["spent_actions"],
        1
    );
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH fixture",
                "job-1",
                &json!({})
            )
            .is_err()
    );
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH different",
                "job-1",
                &json!({})
            )
            .is_err()
    );
    assert!(
        store
            .transition_research(&ticket, "released_unstarted")
            .is_err()
    );
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_repeated_targets_spend_the_existing_allowance() {
    let (store, experiment, path) = fixture("retry");
    run(&store, &experiment, "job-a");
    run(&store, &experiment, "job-b");
    store
        .experiment_research_review_command(
            None,
            "budget_fixture :: outcome: continue; observation: old review",
            json!({}),
        )
        .unwrap();
    assert!(
        store
            .research_preflight(&experiment.experiment_id, "SEARCH fixture")
            .unwrap()["would_admit"]
            .as_bool()
            .unwrap()
    );
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "BROWSE fixture",
                "substituted",
                &json!({})
            )
            .is_err()
    );
    run(&store, &experiment, "job-c");
    run(&store, &experiment, "job-d");
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH fixture",
                "job-e",
                &json!({})
            )
            .is_err()
    );
    assert_eq!(
        research_transactions::strict_rows(&path)
            .unwrap()
            .iter()
            .filter(|row| row["record_type"] == "research_budget_debit")
            .count(),
        4
    );
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_shape_scope_pending_duplicates_and_expiry() {
    let (store, experiment, path) = fixture("scope");
    for action in [
        "READ_MORE fixture AND AR_START change",
        "SEARCH fixture\nWRITE_FILE x",
        "UNKNOWN read",
    ] {
        assert!(research_binding_requires_budget(action).is_err());
    }
    let first = store
        .reserve_research(
            &experiment.experiment_id,
            "SEARCH fixture",
            "first",
            &json!({}),
        )
        .unwrap();
    store
        .reserve_research(
            &experiment.experiment_id,
            "SEARCH fixture",
            "second",
            &json!({}),
        )
        .unwrap();
    assert_eq!(
        store
            .research_preflight(&experiment.experiment_id, "SEARCH fixture")
            .unwrap()["would_admit"],
        true
    );
    let mut rows = research_transactions::strict_rows(&path).unwrap();
    rows[0]["expires_at_unix_s"] = json!(1);
    fs::write(
        &path,
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
    assert!(
        store
            .transition_research(&first, "execution_started")
            .is_err()
    );
    rows[0]["expires_at_unix_s"] = json!(4_102_444_800u64);
    rows[0]["allowed_sources"] = json!(["local"]);
    fs::write(
        &path,
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH changed",
                "outside",
                &json!({})
            )
            .is_err()
    );
    rows[0].as_object_mut().unwrap().remove("expires_at_unix_s");
    assert!(active_research_budget_from_rows(&rows, &experiment.experiment_id).is_none());
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_concurrent_reservations_and_unstarted_release() {
    let (store, experiment, path) = fixture("concurrency");
    let mut workers = Vec::new();
    for index in 0..8 {
        let root = store.root().to_path_buf();
        let id = experiment.experiment_id.clone();
        workers.push(std::thread::spawn(move || {
            ActionContinuityStore::new(root).reserve_research(
                &id,
                &format!("SEARCH fixture-{index}"),
                &format!("job-{index}"),
                &json!({}),
            )
        }));
    }
    let tickets: Vec<_> = workers
        .into_iter()
        .filter_map(|worker| worker.join().unwrap().ok())
        .collect();
    assert_eq!(tickets.len(), 4);
    assert!(
        active_research_budget_from_rows(
            &research_transactions::strict_rows(&path).unwrap(),
            &experiment.experiment_id
        )
        .is_none()
    );
    store
        .transition_research(&tickets[0], "released_unstarted")
        .unwrap();
    store
        .transition_research(&tickets[1], "execution_started")
        .unwrap();
    assert!(
        store
            .transition_research(&tickets[1], "released_unstarted")
            .is_err()
    );
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn explicit_research_foreign_owner_corrupt_tail_and_held_review_fail_closed() {
    let (store, experiment, path) = fixture("failure");
    assert!(
        store
            .reserve_research(
                "exp_minime_foreign",
                "SEARCH fixture",
                "foreign",
                &json!({})
            )
            .is_err()
    );
    let ticket = store
        .reserve_research(
            &experiment.experiment_id,
            "SEARCH fixture",
            "hold",
            &json!({}),
        )
        .unwrap();
    store
        .experiment_research_review_command(
            None,
            "budget_fixture :: outcome: hold; observation: stop",
            json!({}),
        )
        .unwrap();
    assert!(
        store
            .transition_research(&ticket, "execution_started")
            .is_err()
    );
    let mut damaged = fs::read(&path).unwrap();
    damaged.extend_from_slice(b"{\"partial\":");
    fs::write(&path, &damaged).unwrap();
    assert!(
        store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH fixture",
                "corrupt",
                &json!({})
            )
            .is_err()
    );
    assert!(
        store
            .append_jsonl(&path, &json!({"record_type": "test"}))
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), damaged);
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn research_capability_floor_rejects_invalid_state_without_admission() {
    let (store, experiment, authority) = fixture("invalid_floor");
    let floor = store.root().join("research-transactions-required.json");
    let before = fs::read(&authority).unwrap();
    for raw in [
        b"{broken".to_vec(),
        b"{}".to_vec(),
        vec![b' '; 4097],
        vec![0xff],
        br#"{"schema_version":1,"owner":"minime","required_research_transaction_version":1}"#.to_vec(),
        br#"{"schema_version":2,"owner":"astrid","required_research_transaction_version":1}"#.to_vec(),
        br#"{"schema_version":1,"owner":"astrid","required_research_transaction_version":2}"#.to_vec(),
        br#"{"schema_version":1,"owner":"astrid","required_research_transaction_version":true}"#.to_vec(),
        br#"{"schema_version":1,"owner":"astrid","required_research_transaction_version":2,"required_research_transaction_version":1}"#.to_vec(),
        br#"{"schema_version":1,"owner":"astrid","owner":"astrid","required_research_transaction_version":1}"#.to_vec(),
        br#"{"schema_version":1,"owner":"astrid","required_research_transaction_version":1,"extra":true}"#.to_vec(),
    ] {
        fs::write(&floor, &raw).unwrap();
        assert!(store.reserve_research(&experiment.experiment_id, "SEARCH fixture", "invalid-floor", &json!({})).is_err());
        assert_eq!(fs::read(&floor).unwrap(), raw);
        assert_eq!(fs::read(&authority).unwrap(), before);
    }
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn research_capability_floor_rejects_links_and_special_files_before_opening() {
    let (store, experiment, authority) = fixture("linked_floor");
    let floor = store.root().join("research-transactions-required.json");
    let target = store.root().join("missing-floor-target");
    let before = fs::read(&authority).unwrap();
    let refuse = || {
        assert!(
            store
                .reserve_research(
                    &experiment.experiment_id,
                    "SEARCH fixture",
                    "linked-floor",
                    &json!({})
                )
                .is_err()
        );
        assert_eq!(fs::read(&authority).unwrap(), before);
    };
    std::os::unix::fs::symlink(&target, &floor).unwrap();
    refuse();
    assert!(floor.is_symlink());
    assert!(!target.exists());
    fs::write(&target, b"unchanged").unwrap();
    refuse();
    assert_eq!(fs::read(&target).unwrap(), b"unchanged");
    fs::remove_file(&floor).unwrap();
    fs::create_dir(&floor).unwrap();
    refuse();
    assert!(floor.is_dir());
    fs::remove_dir(&floor).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&floor)
            .status()
            .unwrap()
            .success()
    );
    refuse();
    use std::os::unix::fs::FileTypeExt;
    assert!(fs::symlink_metadata(&floor).unwrap().file_type().is_fifo());
    fs::remove_file(&floor).unwrap();
    let alias = store.root().join("owner-alias");
    std::os::unix::fs::symlink(store.root(), &alias).unwrap();
    assert!(research_transactions::require_capability(&alias).is_err());
    assert!(!floor.exists());
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn research_capability_floor_retries_every_uncertain_sync_without_rewriting() {
    for failure in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let floor = root.path().join("research-transactions-required.json");
        let expected = vec![
            floor.clone(),
            root.path().to_path_buf(),
            root.path().parent().unwrap().to_path_buf(),
        ];
        let mut attempted = Vec::new();
        let result =
            research_transactions::require_capability_with_sync(root.path(), |path, file| {
                attempted.push(path.to_path_buf());
                anyhow::ensure!(
                    attempted.len().checked_sub(1) != Some(failure),
                    "injected sync failure"
                );
                Ok(file.sync_all()?)
            });
        assert!(result.is_err());
        assert_eq!(attempted, expected[..=failure]);
        let bytes = fs::read(&floor).unwrap();
        let modified = fs::metadata(&floor).unwrap().modified().unwrap();
        let mut retried = Vec::new();
        research_transactions::require_capability_with_sync(root.path(), |path, file| {
            retried.push(path.to_path_buf());
            Ok(file.sync_all()?)
        })
        .unwrap();
        assert_eq!(retried, expected);
        assert_eq!(fs::read(&floor).unwrap(), bytes);
        assert_eq!(fs::metadata(&floor).unwrap().modified().unwrap(), modified);
    }
}
#[test]
fn compatibility_inspection_is_read_only_and_refuses_newer_or_corrupt_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("absent");
    assert_eq!(
        research_transaction_compatibility(&root).unwrap()["required_version"],
        0
    );
    assert!(!root.exists());
    std::fs::create_dir(&root).unwrap();
    let path = root.join("research-transactions-required.json");
    for raw in [
        r#"{"schema_version":1,"owner":"astrid","required_research_transaction_version":2}"#,
        r#"{"schema_version":1,"owner":"minime","required_research_transaction_version":1}"#,
        r#"{"schema_version":1,"owner":"astrid","owner":"astrid","required_research_transaction_version":1}"#,
        r#"{"schema_version":1"#,
    ] {
        std::fs::write(&path, raw).unwrap();
        assert!(research_transaction_compatibility(&root).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
    }
    let raw = r#"{"schema_version":1,"owner":"astrid","required_research_transaction_version":1}"#;
    std::fs::write(&path, raw).unwrap();
    assert_eq!(
        research_transaction_compatibility(&root).unwrap()["required_version"],
        1
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
}
