use super::*;
use crate::action_continuity::ReaderDeliveryEvidence;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

struct Fixture {
    root: tempfile::TempDir,
    store: ActionContinuityStore,
    conv: ConversationState,
    experiment: String,
    thread: String,
    authority: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(root.path().join("action_threads"));
        let experiment = store
            .start_experiment(
                None,
                "Synthetic saved reading",
                "Inspect a retained passage",
            )
            .unwrap();
        let thread = store.current_thread().unwrap().unwrap().thread_id;
        let authority = store
            .root()
            .join("threads")
            .join(&thread)
            .join("authority_gate.jsonl");
        action_continuity::append_native_authority_record(&authority, &json!({
            "record_schema":"research_budget_v1", "record_type":"research_budget_approval", "budget_id":"reading-fixture",
            "being":"astrid", "thread_id":thread, "experiment_id":experiment.experiment_id,
            "scope":"read_only_research", "status":"active", "max_actions":4,
            "allowed_sources":["local","web"], "expires_at_unix_s":4_102_444_800u64
        })).unwrap();
        Self {
            root,
            store,
            conv: ConversationState::new(Vec::new(), None),
            experiment: experiment.experiment_id,
            thread,
            authority,
        }
    }
    fn select(&mut self, filename: &str) {
        let source = self.root.path().join(filename);
        std::fs::write(&source, "synthetic source passage ".repeat(500)).unwrap();
        activity_reading::choose_saved_text_in(
            &self.store,
            &mut self.conv,
            &source,
            "Synthetic source",
        )
        .unwrap();
    }
    fn dispatch(&mut self, action: &str, id: &str) -> NextActionOutcome {
        let db = BridgeDb::open(":memory:").unwrap();
        let (sensory_tx, _) = mpsc::channel(1);
        let telemetry =
            serde_json::from_value(json!({"t_ms":0,"eigenvalues":[1.0],"fill_ratio":0.68}))
                .unwrap();
        let mut burst = 0;
        super::super::handle_next_action(
            &mut self.conv,
            action,
            NextActionContext {
                operation_id: Some(id),
                burst_count: &mut burst,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            },
        )
    }
    fn queue(&mut self) {
        self.select("source.txt");
        self.store.resume_thread(&self.thread).unwrap();
        let command = format!("EXPERIMENT_BIND {} :: READ_MORE", self.experiment);
        let outcome = self.dispatch(&command, "reading-operation");
        assert_eq!(outcome.status, "queued", "{}", outcome.outcome_summary);
        assert_eq!(
            outcome.research_budget_v1.unwrap()["execution_scope"],
            "saved_text_generation_worker"
        );
    }
    fn rows(&self) -> Vec<Value> {
        action_continuity::read_native_authority_records(&self.authority).unwrap()
    }
    fn offer(&mut self) -> ActivityReadingOfferV1 {
        activity_reading::offer_requested_reading(&mut self.conv)
            .unwrap()
            .unwrap()
    }
    fn commit(&self, offer: &ActivityReadingOfferV1) {
        self.store
            .reader_bookmark_commit(
                &offer.reader.thread_id,
                &offer.reader.session_id,
                &offer.version,
                &ReaderDeliveryEvidence {
                    offer_id: offer.passage.offer_id.clone(),
                    final_request_sha256: format!(
                        "{:x}",
                        Sha256::digest(b"synthetic final request")
                    ),
                    supplied_start_byte: offer.passage.start_byte,
                    supplied_end_byte: offer.passage.end_byte,
                    supplied_sha256: offer.passage.sha256.clone(),
                    retained_output_ref: "synthetic/output.txt".into(),
                    retained_output_sha256: format!(
                        "{:x}",
                        Sha256::digest(b"synthetic completed output")
                    ),
                },
                "synthetic-reader-commit",
            )
            .unwrap();
    }
}

#[test]
fn saved_reading_dispatch_debits_only_actual_worker_and_links_native_commit() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    f.queue();
    assert!(
        !f.rows()
            .iter()
            .any(|row| row["record_type"] == "research_budget_debit")
    );
    // Selection can move to a reader thread; it cannot replace the exact grant.
    let reader_thread = f
        .conv
        .activity
        .foreground_reader
        .as_ref()
        .unwrap()
        .thread_id
        .clone();
    f.store.resume_thread(&reader_thread).unwrap();
    let offer = f.offer();
    let job = begin(&mut f.conv, Some(&offer)).unwrap();
    assert!(begin(&mut f.conv, Some(&offer)).is_err());
    f.commit(&offer);
    finish(&mut f.conv, job).unwrap();
    let rows = f.rows();
    assert_eq!(
        rows.iter()
            .filter(|row| row["record_type"] == "research_budget_debit")
            .count(),
        1
    );
    assert_eq!(rows.last().unwrap()["lifecycle"], "completed");
    assert_eq!(
        rows.last().unwrap()["execution_scope"],
        "saved_text_generation_worker"
    );
    assert!(f.conv.activity.foreground_reader.is_none());
    let terminal = latest(&f.store).unwrap().unwrap();
    assert_eq!(
        terminal.delivery.unwrap()["offer_id"],
        offer.passage.offer_id
    );
    let raw = std::fs::read_to_string(path(&f.store)).unwrap();
    assert!(!raw.contains("synthetic source passage"));
    assert!(!raw.contains("synthetic/output.txt"));
}

#[test]
fn saved_reading_no_receipt_is_failure_not_a_delivered_entry() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    f.queue();
    let offer = f.offer();
    let job = begin(&mut f.conv, Some(&offer)).unwrap();
    finish(&mut f.conv, job).unwrap();
    assert_eq!(f.rows().last().unwrap()["lifecycle"], "failed");
    assert!(latest(&f.store).unwrap().unwrap().delivery.is_none());
    assert!(f.conv.activity.foreground_reader.is_none());
}

#[test]
fn saved_reading_expired_or_held_allowance_never_starts() {
    for (key, value) in [
        ("expires_at_unix_s", json!(1)),
        ("status", json!("held")),
        ("status", json!("refused")),
    ] {
        let mut f = Fixture::new();
        let _scope =
            action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
        f.queue();
        let mut grant = f.rows()[0].clone();
        grant[key] = value;
        action_continuity::append_native_authority_record(&f.authority, &grant).unwrap();
        let offer = f.offer();
        assert!(begin(&mut f.conv, Some(&offer)).is_err());
        assert_eq!(f.rows().last().unwrap()["lifecycle"], "released_unstarted");
        assert!(
            !f.rows()
                .iter()
                .any(|row| row["record_type"] == "research_budget_debit")
        );
        assert!(f.conv.activity.foreground_reader.is_none());
    }
}

#[test]
fn saved_reading_superseded_selection_is_not_replaced_or_debited() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    f.queue();
    f.select("other.txt");
    let other = f.offer();
    assert!(begin(&mut f.conv, Some(&other)).is_err());
    assert_eq!(
        f.conv.activity.foreground_reader.as_ref(),
        Some(&other.reader)
    );
    assert_eq!(f.rows().last().unwrap()["lifecycle"], "released_unstarted");
}

#[test]
fn saved_reading_reselection_of_same_session_survives_old_job_cleanup() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    f.queue();
    let first = f.offer();
    let old_revision = f.conv.activity.selection_revision;
    activity_reading::choose_saved_text_in(
        &f.store,
        &mut f.conv,
        &f.root.path().join("source.txt"),
        "Reselected source",
    )
    .unwrap();
    let reselected = f.offer();
    assert_eq!(first.reader, reselected.reader);
    assert!(f.conv.activity.selection_revision > old_revision);
    assert!(begin(&mut f.conv, Some(&reselected)).is_err());
    assert_eq!(
        f.conv.activity.foreground_reader.as_ref(),
        Some(&reselected.reader)
    );
    assert!(!pending().unwrap());
}

#[cfg(unix)]
#[test]
fn saved_reading_symlink_and_invalid_phase_are_rejected_without_append() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    f.queue();
    let mut job = latest(&f.store).unwrap().unwrap();
    let before = std::fs::read(path(&f.store)).unwrap();
    job.phase = Phase::HostStarted;
    assert!(append(&f.store, &job).is_err());
    assert_eq!(std::fs::read(path(&f.store)).unwrap(), before);
    let original = f.root.path().join("retained-handoff.jsonl");
    std::fs::rename(path(&f.store), &original).unwrap();
    std::os::unix::fs::symlink(&original, path(&f.store)).unwrap();
    assert!(ready(&mut f.conv).is_err());
    assert!(append(&f.store, &job).is_err());
    assert_eq!(std::fs::read(&original).unwrap(), before);
}

#[test]
fn saved_reading_restart_keeps_uncertain_start_and_recovers_only_exact_commit() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    f.queue();
    let offer = f.offer();
    begin(&mut f.conv, Some(&offer)).unwrap();
    // No conversation pending pointer is needed to find this durable job.
    assert!(ready(&mut f.conv).unwrap());
    assert!(begin(&mut f.conv, Some(&offer)).is_err());
    f.commit(&offer);
    assert!(!ready(&mut f.conv).unwrap());
    assert_eq!(f.rows().last().unwrap()["lifecycle"], "completed");
    assert!(!pending().unwrap());
    assert!(f.conv.activity.foreground_reader.is_none());
}

#[test]
fn saved_reading_partial_preparation_and_corrupt_tail_block_without_overwrite() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    let reservation = f
        .store
        .reserve_research(&f.experiment, "READ_MORE", "preparation-crash", &json!({}))
        .unwrap();
    append(
        &f.store,
        &ReadingJob {
            schema_version: 1,
            reservation,
            phase: Phase::Preparing,
            selected_reader: None,
            selection_revision: None,
            selection: None,
            delivery: None,
        },
    )
    .unwrap();
    f.select("source.txt");
    assert!(!ready(&mut f.conv).unwrap());
    let mut bytes = std::fs::read(path(&f.store)).unwrap();
    bytes.extend_from_slice(b"{\"interrupted\":");
    std::fs::write(path(&f.store), &bytes).unwrap();
    assert!(ready(&mut f.conv).is_err());
    assert_eq!(std::fs::read(path(&f.store)).unwrap(), bytes);
}

#[test]
fn synchronous_page_dispatch_retains_host_scope_and_invalid_read_is_not_honored() {
    let mut f = Fixture::new();
    let _scope =
        action_continuity::scoped_test_action_continuity_root(f.store.root().to_path_buf());
    let page = f.root.path().join("codex_synthetic.txt");
    std::fs::write(&page, "Synthetic saved response.").unwrap();
    f.conv.last_read_path = Some(page.to_string_lossy().into_owned());
    let command = format!("EXPERIMENT_BIND {} :: READ_MORE", f.experiment);
    let outcome = f.dispatch(&command, "host-page");
    assert_eq!(outcome.status, "handled", "{}", outcome.outcome_summary);
    assert!(!pending().unwrap());
    assert_eq!(
        f.rows().last().unwrap()["execution_scope"],
        "host_action_dispatch"
    );
    let command = format!("EXPERIMENT_BIND {} :: MIKE_READ", f.experiment);
    let outcome = f.dispatch(&command, "invalid-read");
    assert_eq!(outcome.status, "blocked");
    assert_eq!(f.rows().last().unwrap()["lifecycle"], "released_unstarted");
}
