//! Synthetic interruptions through the production scheduler adapter and native stores.
use super::*;
use std::collections::BTreeMap;

struct Fixture {
    _root: tempfile::TempDir,
    store: ActionContinuityStore,
    reader: Reader,
    conv: ConversationState,
}

impl Fixture {
    fn new(turns: u8) -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(root.path().join("action_threads"));
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), root.path().to_owned())])).unwrap(),
            root.path()
                .join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(root.path().to_owned(), "astrid");
        reader
            .prepare_action("WRITE START synthetic private title")
            .unwrap();
        let mut conv = ConversationState::new(Vec::new(), None);
        start_selection(
            &store,
            &reader,
            &mut conv,
            &format!("ACTIVITY_FOCUS WRITE d1 turns {turns}"),
            "chosen-focus",
        )
        .unwrap();
        Self {
            _root: root,
            store,
            reader,
            conv,
        }
    }

    fn prepare(&mut self) -> bool {
        prepare_exchange_in(&self.store, &self.reader, &mut self.conv).unwrap()
    }

    fn begin(&mut self) -> (astrid_source_study::StudyOutput, Admission) {
        assert!(self.prepare());
        let action = self.conv.introspect_target.as_ref().unwrap().label.clone();
        let output = self.reader.prepare_action(&action).unwrap();
        let job = admit(&self.reader, &output, &action).unwrap().unwrap();
        (output, job)
    }

    fn deliver(&self, output: &astrid_source_study::StudyOutput, job: &Admission, text: &str) {
        self.reader.navigation_delivered(&job.input,
            &serde_json::json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
            &serde_json::json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string(),
        ).unwrap();
    }

    fn restart(&mut self) {
        let target = self.conv.introspect_target.clone();
        self.conv = ConversationState::new(Vec::new(), None);
        self.conv.activity = activity_reading::load_activity(&self.store).unwrap();
        self.conv.introspect_target = target;
        self.conv.wants_introspect = self.conv.introspect_target.is_some();
    }
}

#[test]
fn delivery_before_complete_restores_the_next_slot_not_the_stale_target() {
    let mut f = Fixture::new(4);
    let (output, job) = f.begin();
    f.deliver(&output, &job, "Exact private result.\nNEXT: WRITE CONTINUE");
    f.restart();
    assert!(f.prepare());
    let target = f.conv.introspect_target.as_ref().unwrap();
    assert_eq!(target.label, "WRITE CONTINUE");
    assert!(target.operation_id.as_ref().unwrap().ends_with("-1"));
    assert_eq!(view(&f.reader).unwrap().admitted, 1);
    assert!(view(&f.reader).unwrap().pending_job.is_none());
}

#[test]
fn complete_before_host_checkpoint_cannot_replay_a_spent_slot() {
    for ending in ["budget", "missing_next", "rest", "deadline", "failed"] {
        let mut f = Fixture::new(if ending == "budget" { 1 } else { 4 });
        let (output, job) = f.begin();
        let text = match ending {
            "missing_next" => "Exact private result without a choice.",
            "rest" => "Exact private result.\nNEXT: REST",
            _ => "Exact private result.\nNEXT: WRITE CONTINUE",
        };
        if ending != "failed" {
            f.deliver(&output, &job, text);
        }
        finish(&f.reader, Some(job), ending != "failed").unwrap();
        if ending == "deadline" {
            let deadline = view(&f.reader).unwrap().deadline_ms.unwrap();
            f.reader
                .activity("expiry", None, deadline, Request::Status)
                .unwrap();
        }
        f.restart();
        assert!(!f.prepare(), "{ending}");
        assert!(
            f.conv.introspect_target.is_none(),
            "spent target survived: {ending}"
        );
        assert!(!f.conv.wants_introspect, "{ending}");
        let status = view(&f.reader).unwrap();
        assert_eq!(status.admitted, 1);
        assert!(
            !serde_json::to_string(&status)
                .unwrap()
                .contains("private result")
        );
    }
}

#[test]
fn uncertain_provider_claim_blocks_competing_work_without_budget_refill() {
    let mut f = Fixture::new(4);
    let (_, job) = f.begin();
    f.restart();
    assert!(prepare_exchange_in(&f.store, &f.reader, &mut f.conv).is_err());
    let status = view(&f.reader).unwrap();
    assert_eq!(status.pending_job.as_deref(), Some(job.job.as_str()));
    assert_eq!(status.admitted, 1);
    let retry = change_once(
        &f.reader,
        Request::Claim {
            job_id: job.job.clone(),
        },
        "different-claim",
    )
    .unwrap();
    assert!(!retry.invocation_granted);
    assert_eq!(retry.admitted, 1);
}

#[test]
fn native_selection_without_host_commit_releases_priority_without_generation() {
    let mut f = Fixture::new(4);
    // Simulate the native commit surviving without the host pointer update.
    let mut old = f.conv.activity.clone();
    old.native_focus_window = None;
    f.conv.activity = activity_reading::persist_activity(&f.store, &old).unwrap();
    f.restart();
    assert!(prepare_exchange_in(&f.store, &f.reader, &mut f.conv).is_err());
    assert!(!view(&f.reader).unwrap().protected);
    assert_eq!(view(&f.reader).unwrap().admitted, 0);
    assert!(f.conv.introspect_target.is_none());
    assert!(!f.prepare());
    assert!(
        f.reader
            .prepare_action("WRITE RESUME d1")
            .unwrap()
            .text
            .contains("synthetic private title")
    );
}

#[test]
fn recovery_does_not_erase_a_different_authored_choice() {
    let mut f = Fixture::new(1);
    let (output, job) = f.begin();
    f.deliver(&output, &job, "Exact result.\nNEXT: WRITE CONTINUE");
    finish(&f.reader, Some(job), true).unwrap();
    let mut selected = super::super::state::IntrospectTargetV2::auto("SELF_STUDY MAP".into());
    selected.operation_id = Some("separate-authored-event".into());
    f.conv.introspect_target = Some(selected.clone());
    f.restart();
    assert!(!f.prepare());
    assert_eq!(f.conv.introspect_target, Some(selected));
    assert!(f.conv.wants_introspect);
}
