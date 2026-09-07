use super::*;
use std::os::unix::fs::PermissionsExt;

struct ReaderFixture {
    directory: tempfile::TempDir,
    store: ActionContinuityStore,
    thread: String,
    session: String,
    source: PathBuf,
}

impl ReaderFixture {
    fn new(text: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(directory.path().join("action_threads"));
        let thread = store.create_thread(None, "Synthetic reader", None).unwrap();
        store
            .continuity_session_start_command(
                "current :: title: Read a saved source; focus: compare examples; next: REMEMBER synthetic-return-command",
            )
            .unwrap();
        let record = store
            .resolve_continuity_session(&thread, Some("latest"))
            .unwrap()
            .unwrap();
        let session = record["session_id"].as_str().unwrap().to_string();
        let source = directory.path().join("source.txt");
        fs::write(&source, text).unwrap();
        Self {
            directory,
            store,
            thread: thread.thread_id,
            session,
            source,
        }
    }

    fn preview(&self) -> ReaderBookmarkPreview {
        self.store
            .reader_bookmark_preview(&self.thread, &self.session)
            .unwrap()
            .unwrap()
    }

    fn create(&self) -> ReaderBookmarkReceipt {
        self.store
            .reader_bookmark_create(
                &self.thread,
                &self.session,
                &self.preview().session_record_id,
                &self.source,
                "create",
            )
            .unwrap()
    }

    fn offer(
        &self,
        version: &ReaderBookmarkVersion,
        maximum: usize,
        operation: &str,
    ) -> ReaderBookmarkReceipt {
        self.store
            .reader_bookmark_offer(&self.thread, &self.session, version, maximum, operation)
            .unwrap()
    }

    fn evidence(offer: &ReaderPassage, supplied: &str) -> ReaderDeliveryEvidence {
        ReaderDeliveryEvidence {
            offer_id: offer.offer_id.clone(),
            final_request_sha256: reader_bookmark_io::digest(b"synthetic final adapted request"),
            supplied_start_byte: offer.start_byte,
            supplied_end_byte: offer
                .start_byte
                .checked_add(u64::try_from(supplied.len()).unwrap())
                .unwrap(),
            supplied_sha256: reader_bookmark_io::digest(supplied.as_bytes()),
            retained_output_ref: "synthetic/output-1.txt".to_string(),
            retained_output_sha256: reader_bookmark_io::digest(b"synthetic completed output"),
        }
    }

    fn log_bytes(&self) -> Vec<u8> {
        fs::read(self.store.continuity_sessions_path(&self.thread)).unwrap()
    }
}

#[test]
fn reader_bookmark_missing_preview_is_pure() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("absent");
    let store = ActionContinuityStore::new(&root);
    assert!(
        store
            .reader_bookmark_preview("thread_1", "session_1")
            .unwrap()
            .is_none()
    );
    assert!(!root.exists());
}

#[test]
fn reader_bookmark_creation_is_mechanical_and_durable_without_memory_card() {
    let fixture = ReaderFixture::new("Read this later.");
    let memory_path = fixture.store.being_memory_path(&fixture.thread);
    let memory_before = fs::read(&memory_path).unwrap();
    let receipt = fixture.create();
    assert!(receipt.primary_record_durable);
    assert!(!receipt.derived_projection_required);
    assert_eq!(receipt.bookmark.cursor.next_byte, 0);
    assert_eq!(
        receipt.bookmark.authored_focus.as_deref(),
        Some("compare examples")
    );
    assert!(receipt.bookmark.stopping_note.is_none());
    assert_eq!(fs::read(memory_path).unwrap(), memory_before);
    let retained = fixture
        .store
        .root
        .join(&receipt.bookmark.source.retained_artifact);
    assert_eq!(fs::read_to_string(&retained).unwrap(), "Read this later.");
    assert_eq!(
        fs::metadata(retained).unwrap().permissions().mode() & 0o777,
        0o400
    );
    let restarted = ActionContinuityStore::new(fixture.store.root());
    let preview = restarted
        .reader_bookmark_preview(&fixture.thread, &fixture.session)
        .unwrap()
        .unwrap();
    assert_eq!(preview.bookmark.unwrap(), receipt.bookmark);
}

#[test]
fn reader_bookmark_utf8_partial_delivery_retains_undelivered_suffix_across_restart() {
    let fixture = ReaderFixture::new("αβγ delta");
    let initial = fixture.create();
    assert!(
        fixture
            .store
            .reader_bookmark_offer(
                &fixture.thread,
                &fixture.session,
                &initial.version,
                1,
                "too-small"
            )
            .is_err()
    );
    let offer = fixture.offer(&initial.version, 5, "offer-1");
    assert_eq!(offer.bookmark.cursor.next_byte, 0);
    let passage = offer.bookmark.offered_passage.as_ref().unwrap();
    assert_eq!(passage.end_byte, 4);
    assert_eq!(fixture.preview().offered_text.as_deref(), Some("αβ"));
    let evidence = ReaderFixture::evidence(passage, "α");
    let committed = fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            &fixture.session,
            &offer.version,
            &evidence,
            "commit-prefix",
        )
        .unwrap();
    assert_eq!(committed.bookmark.cursor.next_byte, 2);
    let suffix = committed.bookmark.offered_passage.as_ref().unwrap();
    assert_eq!((suffix.start_byte, suffix.end_byte), (2, 4));
    assert_eq!(suffix.offer_id, passage.offer_id);
    let restarted = ActionContinuityStore::new(fixture.store.root());
    assert_eq!(
        restarted
            .reader_bookmark_preview(&fixture.thread, &fixture.session)
            .unwrap()
            .unwrap()
            .offered_text
            .as_deref(),
        Some("β")
    );
    let retry_offer = fixture.offer(&committed.version, 64, "offer-retry");
    assert_eq!(retry_offer.bookmark.offered_passage.as_ref(), Some(suffix));
    assert_eq!(retry_offer.bookmark.cursor.next_byte, 2);
}

#[test]
fn reader_bookmark_invalid_delivery_never_advances_or_appends() {
    let fixture = ReaderFixture::new("αβgamma");
    let initial = fixture.create();
    let offer = fixture.offer(&initial.version, 8, "offer");
    let evidence = ReaderFixture::evidence(offer.bookmark.offered_passage.as_ref().unwrap(), "αβ");
    let before = fixture.log_bytes();
    for case in 0..6 {
        let mut invalid = evidence.clone();
        match case {
            0 => invalid.offer_id = "different-offer".to_string(),
            1 => invalid.final_request_sha256.clear(),
            2 => invalid.retained_output_ref.clear(),
            3 => invalid.supplied_start_byte = 2,
            4 => invalid.supplied_end_byte = 1,
            _ => invalid.supplied_sha256 = reader_bookmark_io::digest(b"other"),
        }
        assert!(
            fixture
                .store
                .reader_bookmark_commit(
                    &fixture.thread,
                    &fixture.session,
                    &offer.version,
                    &invalid,
                    &format!("bad-{case}")
                )
                .is_err()
        );
        assert_eq!(fixture.log_bytes(), before);
    }
}

#[test]
fn reader_bookmark_duplicate_operation_recovers_receipt_without_rewriting_newer_state() {
    let fixture = ReaderFixture::new("abcdefghij");
    let before_create = fixture.preview().session_record_id;
    let initial = fixture.create();
    let offer = fixture.offer(&initial.version, 5, "offer");
    let evidence =
        ReaderFixture::evidence(offer.bookmark.offered_passage.as_ref().unwrap(), "abcde");
    let committed = fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            &fixture.session,
            &offer.version,
            &evidence,
            "commit",
        )
        .unwrap();
    let parked = fixture
        .store
        .reader_bookmark_transition(
            &fixture.thread,
            &fixture.session,
            &committed.version,
            ReaderDisposition::Parked,
            "park",
        )
        .unwrap();
    let before_retry = fixture.log_bytes();
    let retry = fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            &fixture.session,
            &offer.version,
            &evidence,
            "commit",
        )
        .unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.record_id, committed.record_id);
    assert_eq!(fixture.log_bytes(), before_retry);
    assert_eq!(fixture.preview().version().as_ref(), Some(&parked.version));
    fs::write(&fixture.source, "source subsequently changed").unwrap();
    let retry_create = fixture
        .store
        .reader_bookmark_create(
            &fixture.thread,
            &fixture.session,
            &before_create,
            &fixture.source,
            "create",
        )
        .unwrap();
    assert!(retry_create.duplicate);
    assert_eq!(retry_create.bookmark.source, initial.bookmark.source);
    assert!(
        fixture
            .store
            .reader_bookmark_offer(
                &fixture.thread,
                &fixture.session,
                &initial.version,
                6,
                "offer"
            )
            .is_err()
    );
}

#[test]
fn reader_bookmark_park_preview_changed_source_and_explicit_return() {
    let fixture = ReaderFixture::new("original bytes");
    let initial = fixture.create();
    let offer = fixture.offer(&initial.version, 8, "offer");
    let parked = fixture
        .store
        .reader_bookmark_transition(
            &fixture.thread,
            &fixture.session,
            &offer.version,
            ReaderDisposition::Parked,
            "park",
        )
        .unwrap();
    fs::write(&fixture.source, "new source version").unwrap();
    let before = fixture.log_bytes();
    let preview = fixture.preview();
    assert_eq!(preview.session_status, "parked");
    assert_eq!(
        preview.source_comparison.unwrap().current_status,
        ReaderCurrentSourceStatus::Changed
    );
    assert_eq!(preview.offered_text.as_deref(), Some("original"));
    assert_eq!(fixture.log_bytes(), before);
    assert!(
        fixture
            .store
            .reader_bookmark_offer(
                &fixture.thread,
                &fixture.session,
                &parked.version,
                8,
                "uninvited"
            )
            .is_err()
    );
    assert!(
        fixture
            .store
            .reader_bookmark_transition(
                &fixture.thread,
                &fixture.session,
                &offer.version,
                ReaderDisposition::Active,
                "stale-return"
            )
            .is_err()
    );
    let resumed = fixture
        .store
        .reader_bookmark_transition(
            &fixture.thread,
            &fixture.session,
            &parked.version,
            ReaderDisposition::Active,
            "explicit-return",
        )
        .unwrap();
    assert_eq!(resumed.bookmark.source, initial.bookmark.source);
    assert_eq!(resumed.bookmark.cursor.next_byte, 0);
    assert_eq!(fixture.preview().offered_text.as_deref(), Some("original"));
}

#[test]
fn reader_bookmark_ordinary_lifecycle_invalidates_expected_version() {
    let fixture = ReaderFixture::new("ordinary lifecycle");
    let initial = fixture.create();
    let offer = fixture.offer(&initial.version, 8, "offer");
    fixture
        .store
        .continuity_session_finalize_command(&format!("{} :: outcome: park", fixture.session))
        .unwrap();
    let preview = fixture.preview();
    assert_eq!(preview.session_status, "parked");
    assert_eq!(preview.bookmark.as_ref().unwrap().cursor.next_byte, 0);
    assert_eq!(preview.offered_text.as_deref(), Some("ordinary"));
    assert_ne!(preview.version().unwrap(), offer.version);
    let evidence =
        ReaderFixture::evidence(offer.bookmark.offered_passage.as_ref().unwrap(), "ordinary");
    assert!(
        fixture
            .store
            .reader_bookmark_commit(
                &fixture.thread,
                &fixture.session,
                &offer.version,
                &evidence,
                "late-stale"
            )
            .is_err()
    );
    let saved = fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            &fixture.session,
            &preview.version().unwrap(),
            &evidence,
            "late-reconciled",
        )
        .unwrap();
    assert_eq!(saved.bookmark.disposition, ReaderDisposition::Parked);
    assert_eq!(fixture.preview().session_status, "parked");
}

#[test]
fn reader_bookmark_historical_preview_and_retry_beyond_256_newer_records() {
    let fixture = ReaderFixture::new("old selected source");
    let initial = fixture.create();
    for index in 0..320 {
        fixture.store.append_jsonl(&fixture.store.continuity_sessions_path(&fixture.thread), &json!({
            "record_schema":"continuity_session_v1", "record_type":"session_start",
            "record_id":format!("other-record-{index}"), "session_id":format!("other-session-{index}"),
            "thread_id":fixture.thread, "status":"active",
        })).unwrap();
    }
    let before = fixture.log_bytes();
    let preview = fixture.preview();
    assert_eq!(preview.bookmark.as_ref().unwrap(), &initial.bookmark);
    assert_eq!(fixture.log_bytes(), before);
    assert!(
        fixture
            .store
            .reader_bookmark_preview(&fixture.thread, "does-not-exist")
            .unwrap()
            .is_none()
    );
    let offered = fixture.offer(&initial.version, 3, "historical-offer");
    assert_eq!(offered.bookmark.session_id, fixture.session);
}

#[test]
fn reader_bookmark_incomplete_log_is_explicit_and_inspection_never_repairs_it() {
    let fixture = ReaderFixture::new("durable text");
    let initial = fixture.create();
    let path = fixture.store.continuity_sessions_path(&fixture.thread);
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(b"{\"record_id\":\"interrupted").unwrap();
    let before = fixture.log_bytes();
    let error = fixture
        .store
        .reader_bookmark_preview(&fixture.thread, &fixture.session)
        .unwrap_err();
    assert!(error.to_string().contains("incomplete tail"));
    assert!(
        fixture
            .store
            .reader_bookmark_offer(
                &fixture.thread,
                &fixture.session,
                &initial.version,
                4,
                "after-partial"
            )
            .is_err()
    );
    assert_eq!(fixture.log_bytes(), before);
}

#[test]
fn reader_bookmark_concurrent_expected_revision_has_one_winner() {
    let fixture = ReaderFixture::new("one source, two simultaneous offers");
    let initial = fixture.create();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut workers = Vec::new();
    for index in 0..2 {
        let (store, thread, session, version, barrier) = (
            fixture.store.clone(),
            fixture.thread.clone(),
            fixture.session.clone(),
            initial.version.clone(),
            barrier.clone(),
        );
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.reader_bookmark_offer(
                &thread,
                &session,
                &version,
                4,
                &format!("concurrent-{index}"),
            )
        }));
    }
    let outcomes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(fixture.preview().bookmark.unwrap().revision, 2);
}

#[test]
fn reader_bookmark_unavailable_or_tampered_snapshot_is_not_silently_replaced() {
    let fixture = ReaderFixture::new("original saved text");
    let initial = fixture.create();
    let parked = fixture
        .store
        .reader_bookmark_transition(
            &fixture.thread,
            &fixture.session,
            &initial.version,
            ReaderDisposition::Parked,
            "park",
        )
        .unwrap();
    let path = fixture
        .store
        .root
        .join(&initial.bookmark.source.retained_artifact);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(path, "tampered").unwrap();
    let before = fixture.log_bytes();
    let comparison = fixture.preview().source_comparison.unwrap();
    assert!(!comparison.retained_source_available);
    assert!(comparison.retained_source_error.unwrap().contains("digest"));
    assert!(
        fixture
            .store
            .reader_bookmark_transition(
                &fixture.thread,
                &fixture.session,
                &parked.version,
                ReaderDisposition::Active,
                "resume"
            )
            .is_err()
    );
    assert_eq!(fixture.log_bytes(), before);
}

#[test]
fn reader_bookmark_eof_and_abandonment_require_no_authored_report() {
    let fixture = ReaderFixture::new("end");
    let initial = fixture.create();
    let offer = fixture.offer(&initial.version, 3, "offer");
    let evidence = ReaderFixture::evidence(offer.bookmark.offered_passage.as_ref().unwrap(), "end");
    let committed = fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            &fixture.session,
            &offer.version,
            &evidence,
            "commit",
        )
        .unwrap();
    assert!(committed.bookmark.offered_passage.is_none());
    assert_eq!(committed.bookmark.disposition, ReaderDisposition::Active);
    assert!(
        fixture
            .store
            .reader_bookmark_offer(
                &fixture.thread,
                &fixture.session,
                &committed.version,
                3,
                "eof"
            )
            .is_err()
    );
    let abandoned = fixture
        .store
        .reader_bookmark_transition(
            &fixture.thread,
            &fixture.session,
            &committed.version,
            ReaderDisposition::Abandoned,
            "abandon",
        )
        .unwrap();
    assert_eq!(abandoned.bookmark.disposition, ReaderDisposition::Abandoned);
    assert_eq!(fixture.preview().session_status, "abandoned");
}

#[test]
fn reader_bookmark_invalid_utf8_never_attaches_source() {
    let fixture = ReaderFixture::new("valid initially");
    fs::write(&fixture.source, [0xff, 0xfe]).unwrap();
    let before = fixture.log_bytes();
    assert!(
        fixture
            .store
            .reader_bookmark_create(
                &fixture.thread,
                &fixture.session,
                &fixture.preview().session_record_id,
                &fixture.source,
                "invalid"
            )
            .is_err()
    );
    assert_eq!(fixture.log_bytes(), before);
    assert!(fixture.preview().bookmark.is_none());
    assert!(!fixture.store.root.join("reader_sources").exists());
}

#[test]
fn reader_bookmark_append_failure_reports_the_uncertain_stage() {
    let fixture = ReaderFixture::new("failure fixture");
    let path = fixture.store.continuity_sessions_path(&fixture.thread);
    let mut log = reader_bookmark_io::ReaderSessionLog {
        file: fs::File::open(&path).unwrap(),
        path,
        latest: None,
        prior_operation: None,
    };
    let error = log
        .append(&json!({"synthetic":true}), "operation", "record")
        .unwrap_err();
    let error = error.downcast_ref::<ReaderPersistenceError>().unwrap();
    assert_eq!(error.stage, ReaderAppendStage::PartialOrUnknown);
    assert_eq!(error.record_id, "record");
    assert_eq!(error.operation_id, "operation");
    assert!(fixture.directory.path().exists());
}

#[test]
fn reader_bookmark_episode_failed_passage_synthetic_detour_restart_and_explicit_return() {
    let fixture = ReaderFixture::new("first second third");
    let initial = fixture.create();
    let first = fixture.offer(&initial.version, 6, "a-first-offer");
    let evidence =
        ReaderFixture::evidence(first.bookmark.offered_passage.as_ref().unwrap(), "first ");
    let first_done = fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            &fixture.session,
            &first.version,
            &evidence,
            "a-first-complete",
        )
        .unwrap();
    let pending = fixture.offer(&first_done.version, 7, "a-second-offer");
    // Simulated generation failure: no completed-delivery evidence is supplied.
    assert_eq!(pending.bookmark.cursor.next_byte, 6);
    fixture
        .store
        .continuity_session_finalize_command(&format!("{} :: outcome: park", fixture.session))
        .unwrap();
    assert!(
        fixture
            .store
            .continuity_session_capture_command(&format!(
                "{} :: summary: synthetic note cannot reopen parked reading",
                fixture.session
            ),)
            .is_err()
    );
    let raw = String::from_utf8(fixture.log_bytes()).unwrap();
    let mut bypass: Value = serde_json::from_str(raw.lines().last().unwrap()).unwrap();
    bypass["expected_session_record_id"] = bypass["record_id"].clone();
    bypass["record_id"] = json!("synthetic-capture-cannot-reopen");
    bypass["record_type"] = json!("session_capture");
    bypass["status"] = json!("active");
    let before_bypass = fixture.log_bytes();
    assert!(
        fixture
            .store
            .append_jsonl(
                &fixture.store.continuity_sessions_path(&fixture.thread),
                &bypass
            )
            .is_err()
    );
    assert_eq!(fixture.log_bytes(), before_bypass);

    // B is a synthetic reading detour, not an implemented inbox reservation.
    fixture
        .store
        .continuity_session_start_command(
            "current :: title: Synthetic detour B; focus: inspect another source",
        )
        .unwrap();
    let raw = String::from_utf8(fixture.log_bytes()).unwrap();
    let b_record: Value = serde_json::from_str(raw.lines().last().unwrap()).unwrap();
    let b_session = b_record["session_id"].as_str().unwrap();
    assert_ne!(b_session, fixture.session);
    let b_source = fixture.directory.path().join("synthetic-detour.txt");
    fs::write(&b_source, "detour only").unwrap();
    let b_created = fixture
        .store
        .reader_bookmark_create(
            &fixture.thread,
            b_session,
            b_record["record_id"].as_str().unwrap(),
            &b_source,
            "b-create",
        )
        .unwrap();
    let b_offer = fixture
        .store
        .reader_bookmark_offer(&fixture.thread, b_session, &b_created.version, 6, "b-offer")
        .unwrap();
    let b_evidence =
        ReaderFixture::evidence(b_offer.bookmark.offered_passage.as_ref().unwrap(), "detour");
    fixture
        .store
        .reader_bookmark_commit(
            &fixture.thread,
            b_session,
            &b_offer.version,
            &b_evidence,
            "b-complete",
        )
        .unwrap();
    fs::remove_file(&fixture.source).unwrap();

    let restarted = ActionContinuityStore::new(fixture.store.root());
    let before_status = fixture.log_bytes();
    let status = restarted
        .continuity_session_status_command(&fixture.session)
        .unwrap();
    let status: Value = serde_json::from_str(status.split_once('\n').unwrap().1).unwrap();
    assert_eq!(status["latest_session"]["session_id"], fixture.session);
    assert_eq!(status["reader_preview"]["offered_text"], "second ");
    assert_eq!(
        status["reader_preview"]["bookmark"]["cursor"]["next_byte"],
        6
    );
    assert_eq!(
        status["reader_preview"]["source_comparison"]["current_status"],
        "unavailable"
    );
    assert_eq!(fixture.log_bytes(), before_status);
    let record_id = status["reader_preview"]["session_record_id"]
        .as_str()
        .unwrap();
    assert!(
        restarted
            .continuity_session_resume_command(&fixture.session)
            .is_err()
    );
    let memory_before = fs::read(fixture.store.being_memory_path(&fixture.thread)).unwrap();
    let resumed = restarted
        .continuity_session_resume_command(&format!("{} :: revision: {record_id}", fixture.session))
        .unwrap();
    assert!(resumed.contains("Suggested NEXT (not dispatched): REMEMBER synthetic-return-command"));
    assert_eq!(
        fs::read(fixture.store.being_memory_path(&fixture.thread)).unwrap(),
        memory_before
    );
    let after = restarted
        .reader_bookmark_preview(&fixture.thread, &fixture.session)
        .unwrap()
        .unwrap();
    assert_eq!(after.session_status, "active");
    assert_eq!(after.offered_text.as_deref(), Some("second "));
    let bookmark = after.bookmark.unwrap();
    assert_eq!(bookmark.cursor.next_byte, 6);
    assert_eq!(bookmark.source, initial.bookmark.source);
    assert_eq!(bookmark.offered_passage, pending.bookmark.offered_passage);
}
