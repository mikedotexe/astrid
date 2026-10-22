use super::*;
use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};

#[path = "study_legacy_writing_tests.rs"]
mod legacy_writing;

struct Fixture {
    temp: tempfile::TempDir,
    store: ActionContinuityStore,
    reader: Reader,
    conv: ConversationState,
    experiment: String,
    authority: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("build.rs"), "pub fn synthetic() {}\n").unwrap();
        let store = ActionContinuityStore::new(root.join("action_threads"));
        let experiment = store
            .start_experiment(None, "Synthetic source", "Inspect source")
            .unwrap();
        let thread = store.current_thread().unwrap().unwrap().thread_id;
        let authority = store
            .root()
            .join("threads")
            .join(&thread)
            .join("authority_gate.jsonl");
        action_continuity::append_native_authority_record(&authority, &json!({
            "record_schema":"research_budget_v1", "record_type":"research_budget_approval",
            "budget_id":"source-fixture", "being":"astrid", "thread_id":thread,
            "experiment_id":experiment.experiment_id, "scope":"read_only_research", "status":"active",
            "max_actions":2, "allowed_sources":["local"], "expires_at_unix_s":4_102_444_800u64
        })).unwrap();
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), root.clone())])).unwrap(),
            root.join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(root, "astrid");
        Self {
            temp,
            store,
            reader,
            conv: ConversationState::new(Vec::new(), None),
            experiment: experiment.experiment_id,
            authority,
        }
    }
    fn ticket(&self, operation: &str, action: &str) -> ResearchReservation {
        self.store
            .reserve_research(&self.experiment, action, operation, &json!({}))
            .unwrap()
    }
    fn queue(&mut self, operation: &str) -> (String, ResearchReservation) {
        let ticket = self.ticket(operation, "SELF_STUDY OPEN astrid/build.rs 1");
        let store = &self.store;
        let outcome = dispatch_in(store, &mut self.conv, ticket.clone(), |conv| {
            let mut target = Some(IntrospectTargetV2::auto(
                "SELF_STUDY OPEN astrid/build.rs 1".into(),
            ));
            queue(store, conv, &mut target, operation, false).unwrap();
            conv.introspect_target = target;
            conv.wants_introspect = true;
            NextActionOutcome::handled("source_study", "queued")
        })
        .unwrap();
        assert_eq!(outcome.status, "queued");
        assert_eq!(
            outcome.research_budget_v1.unwrap()["execution_scope"],
            "source_study_generation_worker"
        );
        (
            self.conv
                .introspect_target
                .as_ref()
                .unwrap()
                .operation_id
                .clone()
                .unwrap(),
            ticket,
        )
    }
    fn prepare(&mut self, id: &str) -> StudyOutput {
        prepare(&self.store, &mut self.conv, id, |target| {
            crate::autonomous::runtime::prepare_shared_study_target(&self.reader, Some(target))
        })
        .unwrap()
    }
    fn rows(&self) -> Vec<Value> {
        action_continuity::read_native_authority_records(&self.authority).unwrap()
    }
    fn debits(&self) -> usize {
        self.rows()
            .iter()
            .filter(|row| row["record_type"] == "research_budget_debit")
            .count()
    }
    fn recover(&mut self) -> Result<()> {
        self.conv = ConversationState::new(Vec::new(), None);
        reconcile(
            &self.store,
            &mut self.conv,
            &self.reader,
            &self.temp.path().join("deliveries"),
        )
    }
    fn receipt(&self, output: &StudyOutput, id: &str) -> PromptDeliveryReceiptV1 {
        crate::llm::retain_source_study_fixture(
            output,
            id,
            &self.temp.path().join("deliveries"),
            "Synthetic exact response\nNEXT: SELF_STUDY CONTINUE",
        )
    }
}

#[test]
fn real_bound_alias_dispatch_queues_exact_ticket_without_host_completion() {
    let mut f = Fixture::new();
    let _scope = action_continuity::scoped_test_action_continuity_root(f.store.root());
    let db = crate::db::BridgeDb::open(":memory:").unwrap();
    let (sensory_tx, _) = tokio::sync::mpsc::channel(1);
    let telemetry =
        serde_json::from_value(json!({"t_ms":0,"eigenvalues":[1.0],"fill_ratio":0.68})).unwrap();
    let mut burst = 0;
    let outcome = crate::autonomous::next_action::handle_next_action(
        &mut f.conv,
        &format!("EXPERIMENT_BIND {} :: OPEN astrid/build.rs 1", f.experiment),
        crate::autonomous::next_action::NextActionContext {
            operation_id: Some("real-alias"),
            burst_count: &mut burst,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: None,
        },
    );
    assert_eq!(outcome.status, "queued", "{}", outcome.outcome_summary);
    assert_eq!(f.debits(), 0);
    let id = f
        .conv
        .introspect_target
        .as_ref()
        .unwrap()
        .operation_id
        .clone()
        .unwrap();
    let job = &f.conv.activity.study_handoff.jobs[&id];
    let ticket = reservation(&f.store, &id, job).unwrap().unwrap();
    assert_eq!(ticket.metadata()["operation_id"], "real-alias");
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "queued");
    let output = f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    assert!(claim(&f.store, &mut f.conv, &id).is_err());
    let receipt = f.receipt(&output, &id);
    delivered(&f.store, &mut f.conv, &id, &f.reader, &receipt).unwrap();
    assert_eq!(f.debits(), 1);
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "completed");
    for row in f
        .rows()
        .iter()
        .filter(|row| row["operation_id"] == "real-alias" && row["lifecycle"] != "admitted")
    {
        assert_eq!(row["execution_scope"], "source_study_generation_worker");
        assert_eq!(row["input_delivered"], "unknown");
    }
    let before = f.rows();
    f.recover().unwrap();
    assert_eq!(f.rows(), before);
    assert!(!f.conv.wants_introspect);
}

#[test]
fn queued_research_keeps_capacity_reserved_and_recovers_exact_input() {
    let mut f = Fixture::new();
    let (id, ticket) = f.queue("reserved");
    let output = f.prepare(&id);
    let other = f.ticket("second", "SELF_STUDY MAP");
    assert!(
        f.store
            .reserve_research(&f.experiment, "SELF_STUDY MAP", "third", &json!({}))
            .is_err()
    );
    assert_eq!(f.debits(), 0);
    f.recover().unwrap();
    let again = f.prepare(&id);
    assert_eq!(
        serde_json::to_value(output).unwrap(),
        serde_json::to_value(again).unwrap()
    );
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "queued");
    f.store
        .transition_research(&other, "released_unstarted")
        .unwrap();
    assert_eq!(f.debits(), 0);
}

#[test]
fn hold_expiry_scope_and_experiment_pause_rechecked_before_worker_claim() {
    for change in ["hold", "expired", "scope", "experiment"] {
        let mut f = Fixture::new();
        let (id, ticket) = f.queue(change);
        f.prepare(&id);
        match change {
            "hold" => action_continuity::append_native_authority_record(
                &f.authority,
                &json!({
                    "record_schema":"research_budget_v1", "record_type":"research_budget_review",
                    "budget_id":"source-fixture", "outcome":"hold"
                }),
            )
            .unwrap(),
            "experiment" => {
                f.store
                    .close_experiment(None, Some(&f.experiment), "paused synthetic experiment")
                    .unwrap();
            },
            _ => {
                let mut approval = f.rows()[0].clone();
                if change == "expired" {
                    approval["expires_at_unix_s"] = json!(1);
                } else {
                    approval["allowed_sources"] = json!(["web"]);
                }
                action_continuity::append_native_authority_record(&f.authority, &approval).unwrap();
            },
        }
        assert!(claim(&f.store, &mut f.conv, &id).is_err(), "{change}");
        assert_eq!(f.debits(), 0, "{change}");
        assert_eq!(
            lifecycle(&f.store, &ticket).unwrap(),
            "released_unstarted",
            "{change}"
        );
        f.recover().unwrap();
        assert!(!f.conv.wants_introspect);
        assert!(!f.conv.activity.study_handoff.pending());
    }
}

#[test]
fn held_or_expired_ticket_cannot_enter_reader_preparation() {
    for expired in [false, true] {
        let mut f = Fixture::new();
        let (id, ticket) = f.queue("preparation-check");
        if expired {
            let mut approval = f.rows()[0].clone();
            approval["expires_at_unix_s"] = json!(1);
            action_continuity::append_native_authority_record(&f.authority, &approval).unwrap();
        } else {
            action_continuity::append_native_authority_record(
                &f.authority,
                &json!({
                    "record_schema":"research_budget_v1", "record_type":"research_budget_review",
                    "budget_id":"source-fixture", "outcome":"hold"
                }),
            )
            .unwrap();
        }
        let error = prepare(&f.store, &mut f.conv, &id, |_| {
            panic!("held reader preparation must not run")
        });
        assert!(error.is_err());
        assert!(
            !f.temp
                .path()
                .join("diagnostics/source_first_v3/shared_reader/reader-v1.json")
                .exists()
        );
        failed(&f.store, &mut f.conv, &id, false).unwrap();
        assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "released_unstarted");
        assert_eq!(f.debits(), 0);
    }
}

#[test]
fn note_is_notebook_inspection_and_cannot_write_through_command_operands() {
    let mut f = Fixture::new();
    f.reader.prepare_action("SELF_STUDY MAP").unwrap();
    let path = f
        .temp
        .path()
        .join("diagnostics/source_first_v3/shared_reader/reader-v1.json");
    let before: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for command in [
        "SELF_STUDY NOTE new authored words",
        "SELF_STUDY NOTE --page 1 STUDY_NOTE: replacement",
    ] {
        assert!(astrid_source_study::research_source_route(command).is_none());
        assert!(
            f.store
                .reserve_research(&f.experiment, command, "invalid-note", &json!({}))
                .is_err()
        );
    }
    let ticket = f.ticket("note-inspection", "SELF_STUDY NOTE");
    let store = &f.store;
    dispatch_in(store, &mut f.conv, ticket.clone(), |conv| {
        let mut target = Some(IntrospectTargetV2::auto("SELF_STUDY NOTE".into()));
        queue(store, conv, &mut target, "note-inspection", false).unwrap();
        conv.introspect_target = target;
        NextActionOutcome::handled("source_study", "queued")
    })
    .unwrap();
    let id = f
        .conv
        .introspect_target
        .as_ref()
        .unwrap()
        .operation_id
        .clone()
        .unwrap();
    let output = f.prepare(&id);
    assert_eq!(output.input_kind, astrid_source_study::InputKind::Notebook);
    assert!(output.generation_requested);
    let after: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(before["notebook"], after["notebook"]);
    assert_eq!(before["questions"], after["questions"]);
    assert_eq!(f.debits(), 0);
    claim(&f.store, &mut f.conv, &id).unwrap();
    let receipt = f.receipt(&output, &id);
    delivered(&f.store, &mut f.conv, &id, &f.reader, &receipt).unwrap();
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "completed");
    assert_eq!(f.debits(), 1);
}

#[test]
fn exact_ticket_and_target_tampering_cannot_claim() {
    for change in ["target", "ticket", "missing_ticket", "offset"] {
        let mut f = Fixture::new();
        let (id, _) = f.queue(change);
        f.prepare(&id);
        let mut selection = activity_reading::load_activity(&f.store).unwrap();
        let job = selected(&mut selection, &id).unwrap();
        let mut record: Intent = read(&f.store, &job.intent).unwrap();
        match change {
            "target" => record.target.label = "SELF_STUDY MAP".into(),
            "offset" => record.target.offset = IntrospectOffsetV2::Exact(99),
            "ticket" => {
                let mut value = serde_json::to_value(record.research.take().unwrap()).unwrap();
                value["record"]["budget_id"] = json!("forged");
                let ticket = serde_json::from_value(value).unwrap();
                job.research = Some(digest(serde_json::to_vec(&ticket).unwrap()));
                record.research = Some(ticket);
            },
            _ => record.research = None,
        }
        job.intent = retain(&f.store, &record).unwrap();
        activity_reading::persist_activity(&f.store, &selection).unwrap();
        assert!(claim(&f.store, &mut f.conv, &id).is_err(), "{change}");
        assert_eq!(f.debits(), 0, "{change}");
        assert!(f.recover().is_err(), "{change}");
    }
}

#[test]
fn debit_before_claim_publication_is_uncertain_and_never_replayed_or_refunded() {
    let mut f = Fixture::new();
    let (id, ticket) = f.queue("interrupted-claim");
    f.prepare(&id);
    f.store.claim_research_start(&ticket).unwrap();
    assert!(f.recover().is_err());
    assert!(claim(&f.store, &mut f.conv, &id).is_err());
    assert!(failed(&f.store, &mut f.conv, &id, false).is_err());
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "execution_started");
    assert_eq!(f.debits(), 1);
    assert_eq!(
        activity_reading::load_activity(&f.store)
            .unwrap()
            .study_handoff
            .jobs[&id]
            .phase,
        Phase::Prepared
    );
}

#[test]
fn verified_delivery_recovers_after_native_commit_or_accounting_commit() {
    for accounting_committed in [false, true] {
        let mut f = Fixture::new();
        let (id, ticket) = f.queue("delivered");
        let output = f.prepare(&id);
        claim(&f.store, &mut f.conv, &id).unwrap();
        let receipt = f.receipt(&output, &id);
        f.reader
            .delivered_artifact(
                &output.page.as_ref().unwrap().id,
                Path::new(&receipt.retained_artifact_path),
            )
            .unwrap();
        if accounting_committed {
            f.store.transition_research(&ticket, "completed").unwrap();
        }
        f.recover().unwrap();
        assert_eq!(
            f.conv.activity.study_handoff.jobs[&id].phase,
            Phase::Delivered
        );
        assert_eq!(f.debits(), 1);
        assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "completed");
        assert!(!f.conv.wants_introspect);
        let bytes = fs::read(&f.authority).unwrap();
        f.recover().unwrap();
        assert_eq!(fs::read(&f.authority).unwrap(), bytes);
    }
}

#[test]
fn known_failures_release_only_before_start_and_recover_interrupted_publication() {
    for (started, interrupted) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut f = Fixture::new();
        let (id, ticket) = f.queue("known-failure");
        f.prepare(&id);
        if started {
            claim(&f.store, &mut f.conv, &id).unwrap();
        }
        if interrupted {
            // Crash after the terminal authority append, before activity publication.
            f.store
                .transition_research(
                    &ticket,
                    if started {
                        "failed"
                    } else {
                        "released_unstarted"
                    },
                )
                .unwrap();
        } else {
            failed(&f.store, &mut f.conv, &id, started).unwrap();
        }
        assert_eq!(f.debits(), usize::from(started));
        assert_eq!(
            lifecycle(&f.store, &ticket).unwrap(),
            if started {
                "failed"
            } else {
                "released_unstarted"
            }
        );
        f.recover().unwrap();
        assert_eq!(f.conv.activity.study_handoff.jobs[&id].phase, Phase::Failed);
        assert!(!f.conv.wants_introspect);
        assert_eq!(f.debits(), usize::from(started));
    }
}

#[test]
fn claimed_without_receipt_keeps_debit_and_rejects_wrong_delivery() {
    let mut f = Fixture::new();
    let (id, ticket) = f.queue("uncertain-worker");
    let output = f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    assert!(f.recover().is_err());
    let wrong = f.receipt(&output, "wrong-operation");
    assert!(delivered(&f.store, &mut f.conv, &id, &f.reader, &wrong).is_err());
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "execution_started");
    assert_eq!(f.debits(), 1);
}

#[test]
fn refused_or_conflicting_dispatch_does_not_rebind_existing_job() {
    let mut f = Fixture::new();
    let (id, first) = f.queue("original");
    let before = f.conv.activity.study_handoff.clone();
    let ticket = f.ticket("later", "SELF_STUDY MAP");
    let outcome = dispatch_in(&f.store, &mut f.conv, ticket.clone(), |_| {
        panic!("must not dispatch")
    })
    .unwrap();
    assert_eq!(outcome.status, "blocked");
    assert_eq!(f.conv.activity.study_handoff, before);
    assert_eq!(lifecycle(&f.store, &first).unwrap(), "queued");
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "released_unstarted");
    assert_eq!(f.debits(), 0);
    failed(&f.store, &mut f.conv, &id, false).unwrap();
    f.conv.wants_introspect = false;
    let unused = f.ticket("not-queued", "SELF_STUDY MAP");
    let outcome = dispatch_in(&f.store, &mut f.conv, unused.clone(), |_| {
        NextActionOutcome::handled("stub", "no queue")
    })
    .unwrap();
    assert_eq!(outcome.status, "blocked");
    assert_eq!(lifecycle(&f.store, &unused).unwrap(), "released_unstarted");
}

#[test]
fn legacy_pending_state_cannot_silently_run_as_unbound() {
    let mut f = Fixture::new();
    let (id, ticket) = f.queue("pre-upgrade");
    let mut selection = activity_reading::load_activity(&f.store).unwrap();
    let job = selected(&mut selection, &id).unwrap();
    let record: Intent = read(&f.store, &job.intent).unwrap();
    let old = json!({"owner_root":record.owner_root, "target":record.target});
    job.intent = retain(&f.store, &old).unwrap();
    job.research = None;
    activity_reading::persist_activity(&f.store, &selection).unwrap();
    let before = retained_files(f.store.root());
    assert!(inspect_deployment_compatibility(&f.store).is_err());
    assert_eq!(retained_files(f.store.root()), before);
    assert!(
        f.recover()
            .unwrap_err()
            .to_string()
            .contains("legacy pending")
    );
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "queued");
    assert_eq!(f.debits(), 0);
}

fn retained_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return files;
    }
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(retained_files(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}

#[test]
fn deployment_inspection_preserves_pending_input_and_creates_no_missing_store() {
    let mut f = Fixture::new();
    let absent = f.temp.path().join("missing-owner");
    let empty = ActionContinuityStore::new(absent.clone());
    let view = inspect_deployment_compatibility(&empty).unwrap();
    assert_eq!(view["pending"], false);
    assert!(!absent.exists());
    let (id, _) = f.queue("compatible-preflight");
    f.prepare(&id);
    let before = retained_files(f.store.root());
    let view = inspect_deployment_compatibility(&f.store).unwrap();
    assert_eq!(view["pending"], true);
    assert_eq!(view["worker_quiescence_verified"], false);
    assert_eq!(view["mutation_performed"], false);
    assert_eq!(retained_files(f.store.root()), before);
    let mut selection = activity_reading::load_activity(&f.store).unwrap();
    selection.study_handoff.jobs.get_mut(&id).unwrap().output = Some("a".repeat(64));
    activity_reading::persist_activity(&f.store, &selection).unwrap();
    let before = retained_files(f.store.root());
    assert!(inspect_deployment_compatibility(&f.store).is_err());
    assert_eq!(retained_files(f.store.root()), before);
}

#[test]
fn read_only_activity_inspection_accepts_parent_alias_but_rejects_linked_checkpoint() {
    let f = Fixture::new();
    activity_reading::persist_activity(&f.store, &activity_reading::ActivityRuntimeV1::default())
        .unwrap();
    let parent_alias = f.temp.path().join("parent-alias");
    std::os::unix::fs::symlink(f.store.root().parent().unwrap(), &parent_alias).unwrap();
    let aliased = ActionContinuityStore::new(parent_alias.join("action_threads"));
    let before = retained_files(f.store.root());
    activity_reading::inspect_activity(&aliased).unwrap();
    assert_eq!(retained_files(f.store.root()), before);
    let checkpoint = f.store.root().join("activity_runtime_v3.json");
    fs::remove_file(&checkpoint).unwrap();
    std::os::unix::fs::symlink(f.temp.path().join("missing-state"), &checkpoint).unwrap();
    assert!(activity_reading::inspect_activity(&f.store).is_err());
    assert!(checkpoint.is_symlink());
    assert!(!f.temp.path().join("missing-state").exists());
}

#[test]
fn legacy_claimed_completion_recovers_exact_delivery_without_new_authority() {
    let mut f = Fixture::new();
    let mut target = Some(IntrospectTargetV2::auto(
        "SELF_STUDY OPEN astrid/build.rs 1".into(),
    ));
    queue(
        &f.store,
        &mut f.conv,
        &mut target,
        "legacy-unbound-claim",
        false,
    )
    .unwrap();
    f.conv.introspect_target = target;
    let id = f
        .conv
        .introspect_target
        .as_ref()
        .unwrap()
        .operation_id
        .clone()
        .unwrap();
    let output = f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    let receipt = f.receipt(&output, &id);
    let mut selection = activity_reading::load_activity(&f.store).unwrap();
    let job = selected(&mut selection, &id).unwrap();
    let record: Intent = read(&f.store, &job.intent).unwrap();
    job.intent = retain(
        &f.store,
        &json!({"owner_root":record.owner_root,"target":record.target}),
    )
    .unwrap();
    activity_reading::persist_activity(&f.store, &selection).unwrap();
    let authority_before = fs::read(&f.authority).unwrap();
    let receipt_before = fs::read(&receipt.retained_artifact_path).unwrap();
    f.recover().unwrap();
    assert_eq!(
        f.conv.activity.study_handoff.jobs[&id].phase,
        Phase::Delivered
    );
    assert!(!f.conv.wants_introspect);
    assert_eq!(fs::read(&f.authority).unwrap(), authority_before);
    assert_eq!(
        fs::read(&receipt.retained_artifact_path).unwrap(),
        receipt_before
    );
}

#[test]
fn native_source_drift_fails_before_debit_without_replacing_pending_page() {
    let mut f = Fixture::new();
    let (id, ticket) = f.queue("source-drift");
    let output = f.prepare(&id);
    fs::write(f.temp.path().join("build.rs"), "pub fn changed() {}\n").unwrap();
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), f.temp.path().into())])).unwrap();
    assert!(validate_sources(&output, &catalog).is_err());
    failed(&f.store, &mut f.conv, &id, false).unwrap();
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "released_unstarted");
    assert_eq!(f.debits(), 0);
}

#[test]
fn cross_process_worker_claim_debits_exactly_once() {
    let mut f = Fixture::new();
    let (id, ticket) = f.queue("racing-workers");
    f.prepare(&id);
    let spawn = || {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "autonomous::runtime::study_handoff::research::tests::bound_claim_child",
                "--nocapture",
            ])
            .env("ASTRID_BOUND_STUDY_TEST_ROOT", f.store.root())
            .env("ASTRID_BOUND_STUDY_TEST_JOB", &id)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let a = spawn();
    let b = spawn();
    let results = [a, b].map(|child| {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("BOUND_STUDY_CLAIM: true")
    });
    assert_eq!(results.into_iter().filter(|success| *success).count(), 1);
    assert_eq!(f.debits(), 1);
    assert_eq!(lifecycle(&f.store, &ticket).unwrap(), "execution_started");
    assert!(f.recover().is_err());
}

#[test]
fn bound_claim_child() {
    let Ok(root) = std::env::var("ASTRID_BOUND_STUDY_TEST_ROOT") else {
        return;
    };
    let id = std::env::var("ASTRID_BOUND_STUDY_TEST_JOB").unwrap();
    let store = ActionContinuityStore::new(root);
    let mut conv = ConversationState::new(Vec::new(), None);
    println!(
        "BOUND_STUDY_CLAIM: {}",
        claim(&store, &mut conv, &id).is_ok()
    );
}
