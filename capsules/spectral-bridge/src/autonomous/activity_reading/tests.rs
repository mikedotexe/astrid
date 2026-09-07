use super::*;
use crate::action_continuity::ReaderDeliveryEvidence;
use sha2::{Digest, Sha256};
use std::fs;

struct Fixture {
    directory: tempfile::TempDir,
    store: ActionContinuityStore,
    conv: ConversationState,
    source: std::path::PathBuf,
}

impl Fixture {
    fn new(text: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(directory.path().join("action_threads"));
        let source = directory.path().join("source.txt");
        fs::write(&source, text).unwrap();
        let conv = ConversationState::new(Vec::new(), None);
        Self {
            directory,
            store,
            conv,
            source,
        }
    }

    fn choose(&mut self) {
        choose_saved_text_in(
            &self.store,
            &mut self.conv,
            &self.source,
            "Synthetic source",
        )
        .unwrap();
    }

    fn action(&mut self, base: &str, raw: &str) -> Result<String> {
        handle_action_in(&self.store, &mut self.conv, base, &format!("{base} {raw}")).unwrap()
    }

    fn offer(&mut self) -> ActivityReadingOfferV1 {
        offer_requested_reading_in(&self.store, &mut self.conv)
            .unwrap()
            .unwrap()
    }

    fn log(&self, reader: &ReaderActivityRefV1) -> Vec<u8> {
        // Discover only within this fixture; no canonical stores are consulted.
        let directory = self.store.root().join("threads").join(&reader.thread_id);
        let path = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name == "continuity_sessions.jsonl")
            })
            .unwrap();
        fs::read(path).unwrap()
    }
}

#[test]
fn selected_source_persists_immediately_and_ignores_legacy_offset() {
    let mut fixture = Fixture::new("αβγ source");
    fixture.conv.last_read_path = Some("old source".into());
    fixture.conv.last_read_offset = 999_999;
    fixture.conv.browse_url = Some("https://example.test/older-choice".into());
    fixture.conv.wants_search = true;
    fixture.conv.search_topic = Some("older search".into());
    fixture.choose();
    assert!(fixture.conv.last_read_path.is_none());
    assert_eq!(fixture.conv.last_read_offset, 0);
    assert!(fixture.conv.browse_url.is_none());
    assert!(!fixture.conv.wants_search);
    assert!(fixture.conv.search_topic.is_none());
    assert_eq!(
        load_activity(&fixture.store).unwrap(),
        fixture.conv.activity
    );
    let offer = fixture.offer();
    assert_eq!(offer.text, "αβγ source");
    assert_eq!(offer.passage.start_byte, 0);
    let preview = preview_reader(&fixture.store, &offer.reader).unwrap();
    assert_eq!(preview.bookmark.unwrap().cursor.next_byte, 0);
}

#[test]
fn failed_offer_retry_and_same_source_reselection_preserve_identity() {
    let mut fixture = Fixture::new(&"pending α bytes ".repeat(500));
    fixture.choose();
    let first = fixture.offer();
    let before = fixture.log(&first.reader);
    fixture.choose();
    let retry = fixture.offer();
    assert_eq!(first.reader, retry.reader);
    assert_eq!(first.version, retry.version);
    assert_eq!(first.passage, retry.passage);
    assert_eq!(first.text, retry.text);
    assert_eq!(before, fixture.log(&first.reader));
}

#[test]
fn mailbox_detour_survives_restart_and_returns_exact_failed_offer() {
    let mut fixture = Fixture::new(&"αβγ delta ".repeat(700));
    fixture.choose();
    let offered = fixture.offer();
    fixture.action("CHECK_MAILBOX", "LARGE").unwrap();
    let window = fixture.conv.activity.mailbox_window.clone().unwrap();
    assert!(window.large);
    fixture.action("CHECK_MAILBOX", "").unwrap();
    assert_eq!(fixture.conv.activity.mailbox_window.as_ref(), Some(&window));
    assert!(fixture.conv.activity.foreground_reader.is_none());
    assert_eq!(
        fixture.conv.activity.return_reader.as_ref(),
        Some(&offered.reader)
    );
    fs::write(
        &fixture.source,
        "The original file changed during the detour.",
    )
    .unwrap();

    let mut restarted = ConversationState::new(Vec::new(), None);
    restarted.activity = load_activity(&fixture.store).unwrap();
    fixture.conv = restarted;
    assert!(
        offer_requested_reading_in(&fixture.store, &mut fixture.conv)
            .unwrap()
            .is_none()
    );
    let before = fixture.log(&offered.reader);
    let status = fixture.action("ACTIVITY_STATUS", "").unwrap();
    assert!(status.contains("Changed"));
    fixture.action("RETURN_ACTIVITY", "").unwrap();
    assert_eq!(before, fixture.log(&offered.reader));
    assert!(fixture.conv.activity.foreground_reader.is_none());
    assert!(fixture.action("RETURN_ACTIVITY", "stale-record").is_err());
    let record = preview_reader(&fixture.store, &offered.reader)
        .unwrap()
        .session_record_id;
    fixture.action("RETURN_ACTIVITY", &record).unwrap();
    assert!(fixture.conv.activity.mailbox_window.is_none());
    let returned = fixture.offer();
    assert_eq!(returned.reader, offered.reader);
    assert_eq!(returned.passage, offered.passage);
    assert_eq!(returned.text, offered.text);
}

#[test]
fn only_completed_evidence_advances_the_next_offered_span() {
    let mut fixture = Fixture::new(&"α".repeat(4_000));
    fixture.choose();
    let first = fixture.offer();
    let supplied = &first.text[..20];
    fixture
        .store
        .reader_bookmark_commit(
            &first.reader.thread_id,
            &first.reader.session_id,
            &first.version,
            &ReaderDeliveryEvidence {
                offer_id: first.passage.offer_id.clone(),
                final_request_sha256: format!("{:x}", Sha256::digest(b"synthetic final request")),
                supplied_start_byte: 0,
                supplied_end_byte: 20,
                supplied_sha256: format!("{:x}", Sha256::digest(supplied.as_bytes())),
                retained_output_ref: "synthetic/output.txt".into(),
                retained_output_sha256: format!(
                    "{:x}",
                    Sha256::digest(b"synthetic completed output")
                ),
            },
            "synthetic-completion",
        )
        .unwrap();
    let suffix = fixture.offer();
    assert_eq!(suffix.passage.start_byte, 20);
    assert_eq!(suffix.passage.end_byte, first.passage.end_byte);
    assert_eq!(suffix.text, first.text[20..]);
}

#[test]
fn choosing_second_source_does_not_inherit_first_cursor_or_erase_return() {
    let mut fixture = Fixture::new(&"first ".repeat(1_000));
    fixture.choose();
    let first = fixture.offer();
    let second_path = fixture.directory.path().join("second.txt");
    fs::write(&second_path, "Second selected text.").unwrap();
    choose_saved_text_in(
        &fixture.store,
        &mut fixture.conv,
        &second_path,
        "Second source",
    )
    .unwrap();
    let second = fixture.offer();
    assert_ne!(second.reader, first.reader);
    assert_eq!(second.passage.start_byte, 0);
    assert_eq!(second.text, "Second selected text.");
    assert_eq!(
        fixture.conv.activity.return_reader.as_ref(),
        Some(&first.reader)
    );
    assert_eq!(
        preview_reader(&fixture.store, &first.reader)
            .unwrap()
            .offered_text
            .as_deref(),
        Some(first.text.as_str())
    );
}

#[test]
fn authored_resume_for_known_return_restores_runtime_without_dispatch() {
    let mut fixture = Fixture::new("Saved source.");
    fixture.choose();
    let reader = fixture.conv.activity.foreground_reader.clone().unwrap();
    fixture.action("PARK_ACTIVITY", "").unwrap();
    let record = preview_reader(&fixture.store, &reader)
        .unwrap()
        .session_record_id;
    let command = format!("{} :: revision: {record}", reader.session_id);
    fixture
        .action("CONTINUITY_SESSION_RESUME", &command)
        .unwrap();
    assert_eq!(
        fixture.conv.activity.foreground_reader.as_ref(),
        Some(&reader)
    );
    assert!(fixture.conv.activity.return_reader.is_none());
    assert!(fixture.conv.pending_file_listing.is_none());
    assert!(
        handle_action_in(
            &fixture.store,
            &mut fixture.conv,
            "CONTINUITY_SESSION_RESUME",
            "CONTINUITY_SESSION_RESUME unrelated"
        )
        .is_none()
    );
}

#[test]
fn restart_obeys_quiet_log_after_interrupted_pointer_update() {
    let mut fixture = Fixture::new("Saved source.");
    fixture.choose();
    let reader = fixture.conv.activity.foreground_reader.clone().unwrap();
    let preview = preview_reader(&fixture.store, &reader).unwrap();
    fixture
        .store
        .reader_bookmark_transition(
            &reader.thread_id,
            &reader.session_id,
            &version(&preview).unwrap(),
            ReaderDisposition::Parked,
            "park-before-crash",
        )
        .unwrap();
    let loaded = load_activity(&fixture.store).unwrap();
    assert!(loaded.foreground_reader.is_none());
    assert_eq!(loaded.return_reader, Some(reader));
}

#[test]
fn absent_and_corrupt_runtime_reads_never_create_or_repair_state() {
    let fixture = Fixture::new("Saved source.");
    assert_eq!(
        load_activity(&fixture.store).unwrap(),
        ActivityRuntimeV1::default()
    );
    assert!(!fixture.store.root().exists());
    fs::create_dir_all(fixture.store.root()).unwrap();
    let path = fixture.store.root().join("activity_runtime_v1.json");
    fs::write(&path, b"{partial").unwrap();
    assert!(load_activity(&fixture.store).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{partial");
}

#[test]
fn failed_new_source_does_not_quiet_current_reader() {
    let mut fixture = Fixture::new("Saved source.");
    fixture.choose();
    let before = fixture.conv.activity.clone();
    let missing = fixture.directory.path().join("missing.txt");
    assert!(choose_saved_text_in(&fixture.store, &mut fixture.conv, &missing, "Missing").is_err());
    assert_eq!(fixture.conv.activity, before);
    assert!(fixture.offer().text.contains("Saved source"));
}

#[test]
fn retained_source_loss_blocks_return_without_destroying_selection() {
    let mut fixture = Fixture::new("Saved source.");
    fixture.choose();
    let reader = fixture.conv.activity.foreground_reader.clone().unwrap();
    fixture.action("PARK_ACTIVITY", "").unwrap();
    let preview = preview_reader(&fixture.store, &reader).unwrap();
    let artifact = fixture
        .store
        .root()
        .join(preview.bookmark.unwrap().source.retained_artifact);
    fs::remove_file(artifact).unwrap();
    let before = fixture.conv.activity.clone();
    assert!(
        fixture
            .action("RETURN_ACTIVITY", &preview.session_record_id)
            .is_err()
    );
    assert_eq!(fixture.conv.activity, before);
}

#[test]
fn housekeeping_keeps_reading_and_explicit_contemplation_parks_it() {
    let mut fixture = Fixture::new("Saved source.");
    let _scope = crate::action_continuity::scoped_test_action_continuity_root(fixture.store.root());
    fixture.choose();
    let before = fixture.conv.activity.clone();
    for action in ["SPEAK", "REMEMBER", "ATTEND", "DEFER", "FOCUS", "FORM"] {
        observe_chosen_action(&mut fixture.conv, action).unwrap();
        assert_eq!(fixture.conv.activity, before);
    }
    observe_chosen_action(&mut fixture.conv, "CONTEMPLATE").unwrap();
    assert!(fixture.conv.activity.foreground_reader.is_none());
    assert_eq!(
        fixture.conv.activity.return_reader,
        before.foreground_reader
    );
}

#[test]
fn activity_preflight_names_the_wired_local_authority_boundary() {
    for action in [
        "ACTIVITY_STATUS",
        "MAILBOX_STATUS",
        "PARK_ACTIVITY",
        "RETURN_ACTIVITY",
        "CHECK_MAILBOX",
    ] {
        let report = super::super::next_action::action_preflight_report(action);
        assert_eq!(report.effective_route, "activity");
        assert_eq!(report.visibility, "protected_summary");
        assert_eq!(
            report.stage,
            if action.ends_with("STATUS") {
                "read_only"
            } else {
                "local_state"
            }
        );
    }
}

#[test]
fn explicit_reader_return_restores_authored_thread_without_executing_saved_next() {
    let mut fixture = Fixture::new("The original selected text.");
    fixture.choose();
    let original = fixture.conv.activity.foreground_reader.clone().unwrap();
    fixture
        .store
        .continuity_session_capture_command(&format!(
            "{} :: summary: Synthetic bookmark note; next: SEARCH must-not-dispatch",
            original.session_id,
        ))
        .unwrap();
    let detour_source = fixture.directory.path().join("detour.txt");
    fs::write(&detour_source, "A separately chosen detour.").unwrap();
    choose_saved_text_in(&fixture.store, &mut fixture.conv, &detour_source, "Detour").unwrap();
    let detour = fixture.conv.activity.foreground_reader.clone().unwrap();
    assert_ne!(original.thread_id, detour.thread_id);
    let record = preview_reader(&fixture.store, &original)
        .unwrap()
        .session_record_id;
    fixture.action("RETURN_ACTIVITY", &record).unwrap();
    let status = fixture.store.thread_status(None).unwrap();
    assert!(status.contains(&original.thread_id));
    assert_eq!(
        fixture.conv.activity.foreground_reader.as_ref(),
        Some(&original)
    );
    assert!(!fixture.conv.wants_search);
    assert!(fixture.conv.search_topic.is_none());
    assert!(fixture.conv.pending_file_listing.is_none());
}
