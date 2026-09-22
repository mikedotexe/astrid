use super::*;
use astrid_source_study::{Catalog, Reader, StudyOutput};

struct Fixture {
    temp: tempfile::TempDir,
    store: ActionContinuityStore,
    reader: Reader,
    conv: ConversationState,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("build.rs"), "pub fn synthetic() {}\n").unwrap();
        let store =
            ActionContinuityStore::new(temp.path().canonicalize().unwrap().join("action_threads"));
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            temp.path()
                .join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(temp.path().into(), "astrid");
        Self {
            temp,
            store,
            reader,
            conv: ConversationState::new(Vec::new(), None),
        }
    }
    fn queue(&mut self, operation: &str, action: &str, replace: bool) -> Result<String> {
        let mut target = Some(IntrospectTargetV2::auto(action.into()));
        ensure!(
            queue(&self.store, &mut self.conv, &mut target, operation, replace)?,
            "not newly queued"
        );
        let id = target.as_ref().unwrap().operation_id.clone().unwrap();
        self.conv.introspect_target = target;
        self.conv.wants_introspect = true;
        Ok(id)
    }
    fn prepare(&mut self, id: &str) -> StudyOutput {
        let target = self.conv.introspect_target.as_ref().unwrap().clone();
        prepare(&self.store, &mut self.conv, id, || {
            super::super::prepare_shared_study_target(&self.reader, Some(target))
        })
        .unwrap()
    }
    fn restart(&mut self, target: Option<IntrospectTargetV2>) {
        self.conv = ConversationState::new(Vec::new(), None);
        self.conv.introspect_target = target;
    }
    fn recover(&mut self) -> Result<()> {
        reconcile(
            &self.store,
            &mut self.conv,
            &self.reader,
            &self.temp.path().join("deliveries"),
        )
    }
    fn receipt(&self, output: &StudyOutput, id: &str, text: &str) -> PromptDeliveryReceiptV1 {
        crate::llm::retain_source_study_fixture(
            output,
            id,
            &self.temp.path().join("deliveries"),
            text,
        )
    }
}

#[test]
fn accepted_choice_survives_before_conversation_checkpoint_and_preparation_is_exact() {
    let mut f = Fixture::new();
    let id = f
        .queue(
            "authored-1",
            "SELF_STUDY QUESTION NEW Synthetic question?",
            false,
        )
        .unwrap();
    f.restart(None);
    f.recover().unwrap();
    assert_eq!(
        f.conv
            .introspect_target
            .as_ref()
            .unwrap()
            .operation_id
            .as_deref(),
        Some(id.as_str())
    );
    let first = f.prepare(&id);
    f.restart(None);
    f.recover().unwrap();
    let retry = f.prepare(&id);
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(retry).unwrap()
    );
    let questions = f.reader.prepare_action("SELF_STUDY QUESTION LIST").unwrap();
    assert!(!questions.text.contains("q2"));
}

#[test]
fn provider_claim_is_single_use_and_no_receipt_leaves_visible_uncertainty() {
    let mut f = Fixture::new();
    let id = f.queue("authored-1", "SELF_STUDY MAP", false).unwrap();
    f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    assert!(claim(&f.store, &mut f.conv, &id).is_err());
    assert!(failed(&f.store, &mut f.conv, &id, false).is_err());
    f.restart(None);
    assert!(f.recover().unwrap_err().to_string().contains("uncertain"));
    assert!(f.queue("new", "SELF_STUDY MAP", true).is_err());
    let state = activity_reading::load_activity(&f.store).unwrap();
    assert_eq!(state.study_handoff.jobs[&id].phase, Phase::Claimed);
}

#[test]
fn retained_completion_recovers_native_delivery_without_replaying_generation_or_next() {
    for action in [
        "SELF_STUDY OPEN astrid/build.rs 1",
        "SELF_STUDY MAP",
        "WRITE START synthetic private title",
    ] {
        let mut f = Fixture::new();
        let id = f.queue("authored-1", action, false).unwrap();
        let old_target = f.conv.introspect_target.clone();
        let output = f.prepare(&id);
        if action.contains("OPEN") {
            assert!(output.page.is_some());
        }
        claim(&f.store, &mut f.conv, &id).unwrap();
        let exact = "Synthetic retained words.\nNEXT: SELF_STUDY MAP";
        let receipt = f.receipt(&output, &id, exact);
        f.restart(old_target.clone());
        f.recover().unwrap();
        assert!(!f.conv.wants_introspect);
        assert!(f.conv.introspect_target.is_none());
        assert!(f.conv.activity.study_handoff.active.is_none());
        assert_eq!(
            f.conv.activity.study_handoff.jobs[&id].phase,
            Phase::Delivered
        );
        let artifact = fs::read_to_string(&receipt.retained_artifact_path).unwrap();
        assert!(artifact.contains("Synthetic retained words."));
        assert!(
            !serde_json::to_string(&f.conv.activity)
                .unwrap()
                .contains("synthetic private")
        );
        assert!(!f.temp.path().join("journal").exists());
        f.restart(old_target);
        f.recover().unwrap();
        assert!(!f.conv.wants_introspect);
        let mut retry = Some(IntrospectTargetV2::auto(action.into()));
        assert!(!queue(&f.store, &mut f.conv, &mut retry, "authored-1", false).unwrap());
        assert!(f.conv.introspect_target.is_none());
    }
}

#[test]
fn native_delivery_before_host_commit_recovers_idempotently() {
    let mut f = Fixture::new();
    let id = f.queue("authored-1", "SELF_STUDY MAP", false).unwrap();
    let output = f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    let receipt = f.receipt(&output, &id, "Synthetic answer.\nNEXT: REST");
    f.reader
        .navigation_delivered_artifact(
            output.navigation_id.as_ref().unwrap(),
            Path::new(&receipt.retained_artifact_path),
        )
        .unwrap();
    f.restart(None);
    f.recover().unwrap();
    assert_eq!(
        f.conv.activity.study_handoff.jobs[&id].phase,
        Phase::Delivered
    );
}

#[test]
fn completed_old_checkpoint_cannot_displace_a_newer_accepted_choice() {
    let mut f = Fixture::new();
    let old = f.queue("old", "SELF_STUDY MAP", false).unwrap();
    let old_target = f.conv.introspect_target.clone();
    let output = f.prepare(&old);
    claim(&f.store, &mut f.conv, &old).unwrap();
    let receipt = f.receipt(&output, &old, "Synthetic result");
    delivered(&f.store, &mut f.conv, &old, &f.reader, &receipt).unwrap();
    let new = f.queue("new", "SELF_STUDY MAP", false).unwrap();
    f.restart(old_target);
    f.recover().unwrap();
    assert_eq!(
        f.conv
            .introspect_target
            .as_ref()
            .unwrap()
            .operation_id
            .as_deref(),
        Some(new.as_str())
    );
}

#[test]
fn conflicting_retries_replacement_and_failed_jobs_preserve_history() {
    let mut f = Fixture::new();
    let old = f.queue("old", "SELF_STUDY MAP", false).unwrap();
    assert!(
        f.queue("old", "SELF_STUDY OPEN astrid/source.rs 1", false)
            .is_err()
    );
    assert!(
        f.queue("new", "SELF_STUDY OPEN astrid/source.rs 1", false)
            .is_err()
    );
    let new = f
        .queue("new", "SELF_STUDY OPEN astrid/source.rs 1", true)
        .unwrap();
    assert_eq!(
        f.conv.activity.study_handoff.jobs[&old].phase,
        Phase::Superseded
    );
    failed(&f.store, &mut f.conv, &new, false).unwrap();
    f.restart(f.conv.introspect_target.clone());
    f.recover().unwrap();
    assert!(!f.conv.wants_introspect);
    assert_eq!(
        f.conv.activity.study_handoff.jobs[&new].phase,
        Phase::Failed
    );
}

#[test]
fn missing_corrupt_cross_owner_and_tampered_receipts_fail_closed() {
    for failure in ["missing", "corrupt", "owner", "receipt"] {
        let mut f = Fixture::new();
        let id = f.queue("op", "SELF_STUDY MAP", false).unwrap();
        let output = f.prepare(&id);
        claim(&f.store, &mut f.conv, &id).unwrap();
        let receipt = f.receipt(&output, &id, "Synthetic result");
        let path = root(&f.store).unwrap().join(format!(
            "{}.json",
            f.conv.activity.study_handoff.jobs[&id].intent
        ));
        match failure {
            "missing" => fs::remove_file(&path).unwrap(),
            "corrupt" => fs::write(&path, "partial").unwrap(),
            "receipt" => fs::write(&receipt.retained_artifact_path, "partial").unwrap(),
            _ => {
                let other = f
                    .temp
                    .path()
                    .canonicalize()
                    .unwrap()
                    .join("other/action_threads");
                fs::create_dir_all(&other).unwrap();
                for file in [
                    "activity_runtime_v1.json",
                    "activity_runtime_v2.json",
                    "activity_runtime_v3.json",
                ] {
                    fs::copy(f.store.root().join(file), other.join(file)).unwrap();
                }
                let copied = ActionContinuityStore::new(other);
                let copied_root = root(&copied).unwrap();
                fs::create_dir_all(&copied_root).unwrap();
                for file in fs::read_dir(root(&f.store).unwrap()).unwrap() {
                    let file = file.unwrap();
                    fs::copy(file.path(), copied_root.join(file.file_name())).unwrap();
                }
                f.store = copied;
            },
        }
        let before = fs::read(f.store.root().join("activity_runtime_v3.json")).unwrap();
        f.restart(None);
        assert!(f.recover().is_err(), "{failure}");
        assert_eq!(
            fs::read(f.store.root().join("activity_runtime_v3.json")).unwrap(),
            before
        );
    }
}

#[test]
fn source_drift_and_receipt_from_another_operation_are_not_blessed() {
    let mut f = Fixture::new();
    let id = f
        .queue("op", "SELF_STUDY OPEN astrid/build.rs 1", false)
        .unwrap();
    let output = f.prepare(&id);
    assert!(
        output.page.is_some(),
        "fixture must exercise an actual source page"
    );
    claim(&f.store, &mut f.conv, &id).unwrap();
    let receipt = f.receipt(&output, "different-job", "Synthetic result");
    assert!(delivered(&f.store, &mut f.conv, &id, &f.reader, &receipt).is_err());
    fs::write(f.temp.path().join("build.rs"), "changed\n").unwrap();
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), f.temp.path().into())])).unwrap();
    assert!(validate_sources(&output, &catalog).is_err());
}

#[test]
fn recovery_preserves_a_conflicting_checkpoint_instead_of_erasing_its_choice() {
    let mut f = Fixture::new();
    let id = f.queue("op", "SELF_STUDY MAP", false).unwrap();
    let output = f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    f.receipt(&output, &id, "Synthetic result");
    let other = Some(IntrospectTargetV2::auto("WRITE CONTINUE".into()));
    f.restart(other.clone());
    assert!(f.recover().is_err());
    assert_eq!(f.conv.introspect_target, other);
    assert_eq!(
        activity_reading::load_activity(&f.store)
            .unwrap()
            .study_handoff
            .jobs[&id]
            .phase,
        Phase::Claimed
    );
}

#[test]
fn native_prepare_commit_before_handoff_pointer_is_reused_without_a_second_mutation() {
    let mut f = Fixture::new();
    let id = f
        .queue("op", "SELF_STUDY QUESTION NEW Synthetic question?", false)
        .unwrap();
    // Simulate native redo completion followed by process loss before host commit.
    let output =
        super::super::prepare_shared_study_target(&f.reader, f.conv.introspect_target.clone())
            .unwrap();
    f.restart(None);
    f.recover().unwrap();
    let retried = f.prepare(&id);
    assert_eq!(
        serde_json::to_value(output).unwrap(),
        serde_json::to_value(retried).unwrap()
    );
    assert!(
        !f.reader
            .prepare_action("SELF_STUDY QUESTION LIST")
            .unwrap()
            .text
            .contains("q2")
    );
}

#[test]
fn durable_dispatch_refuses_missing_identity_and_unwritable_state_before_acknowledging() {
    let mut f = Fixture::new();
    let _scope = crate::action_continuity::scoped_test_action_continuity_root(f.store.root());
    let dispatch = super::super::next_action::study_navigation::handle_durable_request;
    let no_id = dispatch(&mut f.conv, "SELF_STUDY", "SELF_STUDY MAP", None).unwrap();
    assert_eq!(no_id.status, "blocked");
    assert!(f.conv.introspect_target.is_none());
    fs::create_dir_all(f.store.root()).unwrap();
    fs::write(f.store.root().join("activity_runtime_v1.json"), "partial").unwrap();
    let corrupt = dispatch(&mut f.conv, "SELF_STUDY", "SELF_STUDY MAP", Some("op")).unwrap();
    assert_eq!(corrupt.status, "blocked");
    assert!(f.conv.introspect_target.is_none());
    assert!(!corrupt.outcome_summary.contains("Queued:"));
    assert_eq!(
        fs::read(f.store.root().join("activity_runtime_v1.json")).unwrap(),
        b"partial"
    );
}

#[test]
fn child_writer_process() {
    let Ok(root) = std::env::var("ASTRID_STUDY_HANDOFF_TEST_ROOT") else {
        return;
    };
    let kind = std::env::var("ASTRID_STUDY_HANDOFF_TEST_KIND").unwrap();
    let store = ActionContinuityStore::new(root);
    let mut conv = ConversationState::new(Vec::new(), None);
    let success = if kind == "queue" {
        let mut target = Some(IntrospectTargetV2::auto("SELF_STUDY MAP".into()));
        queue(&store, &mut conv, &mut target, "concurrent", false).is_ok()
    } else {
        let id = format!("study-job-{}", digest("concurrent"));
        claim(&store, &mut conv, &id).is_ok()
    };
    println!("HANDOFF_CHILD: {success}");
}

#[test]
fn identical_dispatch_retries_require_the_durable_choice_not_only_a_cached_target() {
    let mut f = Fixture::new();
    let _scope = crate::action_continuity::scoped_test_action_continuity_root(f.store.root());
    let dispatch = super::super::next_action::study_navigation::handle_durable_request;
    assert!(
        dispatch(&mut f.conv, "SELF_STUDY", "SELF_STUDY MAP", Some("op"))
            .unwrap()
            .handled
    );
    let id = f
        .conv
        .introspect_target
        .as_ref()
        .unwrap()
        .operation_id
        .clone()
        .unwrap();
    assert!(
        dispatch(&mut f.conv, "SELF_STUDY", "SELF_STUDY MAP", Some("retry"))
            .unwrap()
            .handled
    );
    assert_eq!(
        activity_reading::load_activity(&f.store)
            .unwrap()
            .study_handoff
            .jobs
            .len(),
        1
    );
    f.prepare(&id);
    claim(&f.store, &mut f.conv, &id).unwrap();
    assert!(
        !dispatch(
            &mut f.conv,
            "SELF_STUDY",
            "SELF_STUDY MAP",
            Some("claimed-retry")
        )
        .unwrap()
        .handled
    );
    f.conv.introspect_target.as_mut().unwrap().operation_id = None;
    assert!(
        !dispatch(
            &mut f.conv,
            "SELF_STUDY",
            "SELF_STUDY MAP",
            Some("legacy-retry")
        )
        .unwrap()
        .handled
    );
    assert_eq!(
        activity_reading::load_activity(&f.store)
            .unwrap()
            .study_handoff
            .jobs[&id]
            .phase,
        Phase::Claimed
    );
}

#[test]
fn cross_process_retries_share_one_queue_and_exactly_one_provider_claim() {
    let mut f = Fixture::new();
    let launch = |kind: &str| {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "autonomous::runtime::study_handoff::tests::child_writer_process",
                "--nocapture",
            ])
            .env("ASTRID_STUDY_HANDOFF_TEST_ROOT", f.store.root())
            .env("ASTRID_STUDY_HANDOFF_TEST_KIND", kind)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let a = launch("queue");
    let b = launch("queue");
    for child in [a, b] {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("HANDOFF_CHILD: true")
        );
    }
    f.recover().unwrap();
    let id = f
        .conv
        .introspect_target
        .as_ref()
        .unwrap()
        .operation_id
        .clone()
        .unwrap();
    f.prepare(&id);
    let a = std::process::Command::new(std::env::current_exe().unwrap());
    // Spawn both claims before waiting; each child uses the actual owner flock.
    let spawn_claim = |mut command: std::process::Command| {
        command
            .args([
                "--exact",
                "autonomous::runtime::study_handoff::tests::child_writer_process",
                "--nocapture",
            ])
            .env("ASTRID_STUDY_HANDOFF_TEST_ROOT", f.store.root())
            .env("ASTRID_STUDY_HANDOFF_TEST_KIND", "claim")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let a = spawn_claim(a);
    let b = spawn_claim(std::process::Command::new(std::env::current_exe().unwrap()));
    let mut successes = 0;
    for child in [a, b] {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        successes += usize::from(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("HANDOFF_CHILD: true"),
        );
    }
    assert_eq!(successes, 1);
    let state = activity_reading::load_activity(&f.store).unwrap();
    assert_eq!(state.study_handoff.jobs.len(), 1);
    assert_eq!(state.study_handoff.jobs[&id].phase, Phase::Claimed);
}
