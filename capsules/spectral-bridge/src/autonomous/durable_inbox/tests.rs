use std::fs;
use std::sync::{Arc, Barrier};

use tempfile::TempDir;

use super::*;

struct Fixture {
    temp: TempDir,
    queue: DurableInbox,
}
impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let inbox = temp.path().join("inbox");
        fs::create_dir(&inbox).unwrap();
        let queue = DurableInbox::new(&inbox, &temp.path().join("queue"));
        Self { temp, queue }
    }
    fn put(&self, name: &str, text: &str) {
        fs::write(self.queue.inbox_dir.join(name), text).unwrap();
    }
    fn reserve(&self, id: &str, now: u64) -> InboxAdmission {
        self.queue.reserve(&window(id, now)).unwrap()
    }
}
fn window(id: &str, now: u64) -> ReceiveWindow {
    ReceiveWindow {
        id: id.into(),
        now_unix_ms: now,
        eligible: true,
        max_letter_bytes: ORDINARY_LETTER_BYTES,
    }
}
fn reserved(admission: InboxAdmission) -> InboxReservation {
    match admission {
        InboxAdmission::Reserved(reservation) => reservation,
        other => panic!("expected reservation, received {other:?}"),
    }
}
fn evidence(reservation: &InboxReservation) -> InboxDeliveryEvidence {
    InboxDeliveryEvidence {
        accepted_attempt_id: "accepted-provider-request".into(),
        submitted_content_sha256: reservation.content_sha256.clone(),
        retained_completion_sha256: storage::hash(b"retained synthetic completion"),
    }
}
fn envelope(message: &str, thread: &str, text: &str) -> String {
    format!(
        "=== CORRESPONDENCE V1 ===\nMessage-Id: {message}\nThread-Id: {thread}\nFrom: minime\nTo: astrid\n\n{text}"
    )
}

#[test]
fn arrival_scan_has_only_counts_and_does_not_admit_or_open_question() {
    let fixture = Fixture::new();
    fixture.put(
        "mike_query_attention.txt",
        "A private question that must wait.",
    );
    assert_eq!(
        fixture.queue.scan().unwrap(),
        InboxScanSummary {
            pending: 1,
            needs_explicit_reading_window: 0,
            unreadable_sources: 0
        }
    );
    assert!(!fixture.queue.inbox_dir.join("read").exists());
    assert!(!fixture.temp.path().join("open_steward_query.json").exists());
    let mut closed = window("closed", 0);
    closed.eligible = false;
    assert_eq!(
        fixture.queue.reserve(&closed).unwrap(),
        InboxAdmission::Ineligible
    );
    assert_eq!(fixture.queue.scan().unwrap().pending, 1);
    assert_eq!(
        fs::read_to_string(fixture.queue.inbox_dir.join("mike_query_attention.txt")).unwrap(),
        "A private question that must wait."
    );
}

#[test]
fn exact_acknowledgement_retires_one_version_and_preserves_other_arrivals() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "first intact letter");
    let letter = reserved(fixture.reserve("first", 0));
    fixture.put("b.txt", "arrived during generation");
    let receipt = fixture
        .queue
        .acknowledge(&letter, &evidence(&letter))
        .unwrap();
    assert_eq!(
        fs::read_to_string(receipt.archived_path).unwrap(),
        letter.text
    );
    assert!(receipt.source_path_retained);
    assert_eq!(fixture.queue.scan().unwrap().pending, 1);
    assert_eq!(
        reserved(fixture.reserve("next", 1)).text,
        "arrived during generation"
    );
}

#[test]
fn restart_retries_same_immutable_letter_after_backoff_not_within_window() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "original letter");
    let first = reserved(fixture.reserve("window-a", 10));
    let restarted = DurableInbox::new(&fixture.queue.inbox_dir, &fixture.queue.queue_root);
    assert_eq!(
        restarted.reserve(&window("window-a", 90_000)).unwrap(),
        InboxAdmission::AlreadyAttempted
    );
    assert_eq!(
        restarted.reserve(&window("too-soon", 11)).unwrap(),
        InboxAdmission::Empty
    );
    fs::remove_file(fixture.queue.inbox_dir.join("a.txt")).unwrap();
    let retry = reserved(restarted.reserve(&window("window-b", 30_010)).unwrap());
    assert_eq!(retry.text, first.text);
    assert_eq!(retry.letter.version_id, first.letter.version_id);
    assert_ne!(retry.reservation_id, first.reservation_id);
}

#[test]
fn fail_is_idempotent_and_cannot_retry_twice_in_same_window() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "letter");
    let letter = reserved(fixture.reserve("window-a", 100));
    fixture
        .queue
        .fail(&letter, 200, "transport failure")
        .unwrap();
    fixture
        .queue
        .fail(&letter, 999_999, "repeat reconciliation")
        .unwrap();
    assert_eq!(
        fixture.reserve("window-a", 100_000),
        InboxAdmission::AlreadyAttempted
    );
    assert_eq!(fixture.reserve("early", 30_199), InboxAdmission::Empty);
    assert!(matches!(
        fixture.reserve("retry", 30_200),
        InboxAdmission::Reserved(_)
    ));
}

#[test]
fn acknowledgement_does_not_delete_or_acknowledge_replaced_path() {
    let fixture = Fixture::new();
    fixture.put("steward_note.txt", "old letter");
    let first = reserved(fixture.reserve("old", 0));
    fixture.put("steward_note.txt", "new content at the same pathname");
    fixture
        .queue
        .acknowledge(&first, &evidence(&first))
        .unwrap();
    assert_eq!(
        fs::read_to_string(&first.letter.source_path).unwrap(),
        "new content at the same pathname"
    );
    assert_eq!(fixture.queue.scan().unwrap().pending, 1);
    let second = reserved(fixture.reserve("new", 1));
    assert_ne!(second.letter.version_id, first.letter.version_id);
    assert_eq!(second.text, "new content at the same pathname");
}

#[test]
fn truncated_or_changed_submission_cannot_acknowledge_intact_letter() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "complete long letter");
    let letter = reserved(fixture.reserve("first", 0));
    let mut wrong = evidence(&letter);
    wrong.submitted_content_sha256 = storage::hash(b"complete");
    assert!(fixture.queue.acknowledge(&letter, &wrong).is_err());
    let mut changed = letter.clone();
    changed.text.push('!');
    assert!(
        fixture
            .queue
            .acknowledge(&changed, &evidence(&changed))
            .is_err()
    );
    assert_eq!(fixture.queue.scan().unwrap().pending, 1);
}

#[test]
fn unknown_or_other_reservation_identity_cannot_acknowledge() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "one");
    fixture.put("b.txt", "two");
    let first = reserved(fixture.reserve("first", 0));
    let second = reserved(fixture.reserve("second", 0));
    let mut forged = first;
    forged.reservation_id = second.reservation_id;
    assert!(
        fixture
            .queue
            .acknowledge(&forged, &evidence(&forged))
            .is_err()
    );
    assert_eq!(fixture.queue.scan().unwrap().pending, 2);
}

#[test]
fn late_first_attempt_receipt_is_recoverable_after_retry_and_restart() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "same letter");
    let first = reserved(fixture.reserve("first", 0));
    let second = reserved(fixture.reserve("second", 30_000));
    let receipt = fixture
        .queue
        .acknowledge(&first, &evidence(&first))
        .unwrap();
    let restarted = DurableInbox::new(&fixture.queue.inbox_dir, &fixture.queue.queue_root);
    assert_eq!(
        restarted.acknowledge(&second, &evidence(&second)).unwrap(),
        receipt
    );
    assert_eq!(restarted.scan().unwrap().pending, 0);
}

#[test]
fn oversized_letter_waits_whole_until_explicit_larger_window() {
    let fixture = Fixture::new();
    let text = "λ".repeat(3_001);
    fixture.put("large.txt", &text);
    assert_eq!(
        fixture.queue.scan().unwrap().needs_explicit_reading_window,
        1
    );
    let pending = match fixture.reserve("ordinary", 0) {
        InboxAdmission::NeedsExplicitReadingWindow(letter) => letter,
        other => panic!("wrong admission {other:?}"),
    };
    assert_eq!(pending.byte_len, 6_002);
    let mut explicit = window("explicit-reading", 1);
    explicit.max_letter_bytes = 6_002;
    let reservation = reserved(fixture.queue.reserve(&explicit).unwrap());
    assert_eq!(reservation.text, text);
    fixture
        .queue
        .acknowledge(&reservation, &evidence(&reservation))
        .unwrap();
    assert_eq!(fixture.queue.scan().unwrap().pending, 0);
}

#[test]
fn thread_fifo_and_fairness_survive_failure_and_restart() {
    let fixture = Fixture::new();
    fixture.put("a1.txt", &envelope("a1", "thread-a", "first a"));
    fixture.queue.scan().unwrap();
    fixture.put("a2.txt", &envelope("a2", "thread-a", "second a"));
    fixture.queue.scan().unwrap();
    fixture.put("b1.txt", &envelope("b1", "thread-b", "first b"));
    fixture.queue.scan().unwrap();
    let a1 = reserved(fixture.reserve("first", 0));
    assert_eq!(a1.letter.message_id, "a1");
    fixture.queue.fail(&a1, 0, "failed").unwrap();
    let b1 = reserved(fixture.reserve("second", 1));
    assert_eq!(b1.letter.message_id, "b1");
    fixture.queue.acknowledge(&b1, &evidence(&b1)).unwrap();
    assert_eq!(fixture.reserve("still-blocked", 2), InboxAdmission::Empty);
    let a1_retry = reserved(fixture.reserve("retry", 30_000));
    assert_eq!(a1_retry.letter.message_id, "a1");
    fixture
        .queue
        .acknowledge(&a1_retry, &evidence(&a1_retry))
        .unwrap();
    assert_eq!(
        reserved(fixture.reserve("last", 30_001)).letter.message_id,
        "a2"
    );
}

#[test]
fn oversized_thread_head_does_not_starve_other_threads() {
    let fixture = Fixture::new();
    fixture.put("large.txt", &"x".repeat(6_001));
    fixture.queue.scan().unwrap();
    fixture.put("small.txt", "small");
    assert!(matches!(
        fixture.reserve("first", 0),
        InboxAdmission::NeedsExplicitReadingWindow(_)
    ));
    assert_eq!(reserved(fixture.reserve("second", 0)).text, "small");
}

#[test]
fn concurrent_reservers_get_only_one_attempt_for_same_window() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "one");
    fixture.put("b.txt", "two");
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let queue = fixture.queue.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                queue.reserve(&window("shared", 0)).unwrap()
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, InboxAdmission::Reserved(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, InboxAdmission::AlreadyAttempted))
            .count(),
        1
    );
}

#[test]
fn corrupted_state_is_preserved_and_fails_closed() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "one");
    fixture.queue.scan().unwrap();
    let path = fixture.queue.queue_root.join("state.json");
    fs::write(&path, b"{truncated").unwrap();
    assert!(fixture.queue.scan().is_err());
    assert!(fixture.queue.reserve(&window("first", 0)).is_err());
    assert_eq!(fs::read(path).unwrap(), b"{truncated");
}

#[test]
fn changed_retained_artifact_cannot_be_delivered_or_acknowledged() {
    let fixture = Fixture::new();
    fixture.put("a.txt", "one");
    let first = reserved(fixture.reserve("first", 0));
    fs::write(
        fixture
            .queue
            .queue_root
            .join("sources")
            .join(format!("{}.txt", first.content_sha256)),
        "tampered",
    )
    .unwrap();
    assert!(
        fixture
            .queue
            .acknowledge(&first, &evidence(&first))
            .is_err()
    );
    assert!(fixture.queue.reserve(&window("retry", 30_000)).is_err());
}

#[test]
fn symlink_sources_are_not_followed_or_admitted() {
    let fixture = Fixture::new();
    let outside = fixture.temp.path().join("outside.txt");
    fs::write(&outside, "outside").unwrap();
    std::os::unix::fs::symlink(outside, fixture.queue.inbox_dir.join("link.txt")).unwrap();
    assert_eq!(fixture.queue.scan().unwrap().pending, 0);
    assert_eq!(fixture.reserve("first", 0), InboxAdmission::Empty);
}

#[test]
fn source_above_retention_bound_is_reported_without_reading_or_truncation() {
    let fixture = Fixture::new();
    let path = fixture.queue.inbox_dir.join("too-large.txt");
    fs::File::create(&path)
        .unwrap()
        .set_len((MAX_RETAINED_LETTER_BYTES as u64).saturating_add(1))
        .unwrap();
    let summary = fixture.queue.scan().unwrap();
    assert_eq!(summary.pending, 1);
    assert_eq!(summary.needs_explicit_reading_window, 1);
    match fixture.reserve("first", 0) {
        InboxAdmission::NeedsExplicitReadingWindow(letter) => {
            assert_eq!(letter.content_sha256, None);
            assert_eq!(letter.source_path, path);
        },
        other => panic!("oversized source was incorrectly admitted: {other:?}"),
    }
    assert_eq!(
        fs::read_dir(fixture.queue.queue_root.join("sources"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn non_utf8_source_is_reported_and_left_for_explicit_attention() {
    let fixture = Fixture::new();
    let path = fixture.queue.inbox_dir.join("invalid.txt");
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert_eq!(fixture.queue.scan().unwrap().unreadable_sources, 1);
    assert_eq!(fixture.reserve("first", 0), InboxAdmission::Empty);
    assert_eq!(fs::read(path).unwrap(), [0xff, 0xfe]);
}
