//! Why a `Recovery map:` turn can never be the answer to `SELF_STUDY CONTINUE`.
//!
//! In `introspection_source_catalog_1789453053` Astrid writes:
//!
//!     "I am currently navigating the `capsule_runtime_health.rs` source. While
//!     the actual code for `accepted_legacy` was not delivered in this specific
//!     turn, my study notebook maintains a consistent record of its logic
//!     (lines 202-210)."
//!
//! Her three consecutive study turns in that window
//! (`..._capsule_runtime_health.rs_1789452200`, `..._1789452645`,
//! `..._1789453053`) each close on `NEXT: SELF_STUDY CONTINUE`, and the two that
//! follow each carry `Input evidence: Recovery map: the requested source was not
//! supplied.` Read from the artifacts alone that looks like `CONTINUE` failing
//! three times, and `source_study_recovery_loop_watch.py` scores exactly that
//! shape as one loop keyed on `SELF_STUDY CONTINUE`.
//!
//! It is not what happened. `Command::parse` maps both `CONTINUE` and the empty
//! argument onto `Command::Continue` (`src/command.rs:30`), and
//! `Store::prepare_parsed` routes that variant straight into `prepare_continue`
//! (`src/store.rs:522`), which is the one command arm with no recovery branch:
//! it returns a pending page, a bookmark-advanced page, an end-of-file notice,
//! or the root map (`src/store.rs:547-591`). Only the arms that resolve a
//! *named* target reach `recovery_map` / `source_recovery` / the
//! `no catalog entries` branch (`src/store.rs:439-474`, `527-541`).
//!
//! The retained navigation artifacts for those two turns agree: both carry
//! `Reason: "no catalog entries for spectral_bridge; use SELF_STUDY MAP"`, and
//! `bridge.db action_events` shows four `SELF_STUDY MAP spectral_bridge`
//! dispatches interleaved between each pair of study turns. Her recorded
//! `CONTINUE` was never the request that failed; it was replaced before the
//! study turn ran.
//!
//! These tests pin the source fact the steward attribution rests on, across
//! every state `CONTINUE` can meet: no bookmark, an undelivered pending page, a
//! delivered page mid-file, and end of file. They assert reachability and input
//! kind only. No recovery text, navigation behaviour, target-selection rule or
//! being-facing prompt is changed here, and nothing here asserts what she should
//! choose.
use astrid_source_study::{Catalog, Command, InputKind, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/state/credentials.rs";
const RECOVERY_HEADER: &str = "Recovery map:";

/// One catalogued repository holding a multi-page source and the hyphenated
/// capsule directory whose underscore spelling she typed.
fn setup(text: &str) -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src/state")).unwrap();
    fs::write(root.join("crates/example/src/state/credentials.rs"), text).unwrap();
    fs::create_dir_all(root.join("capsules/spectral-bridge/src")).unwrap();
    fs::write(
        root.join("capsules/spectral-bridge/src/lifecycle.rs"),
        "pub fn placeholder() {}\n".repeat(20),
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn wire(text: &str) -> String {
    json!({"messages":[{"role":"user","content":text}]}).to_string()
}

fn response() -> String {
    json!({"message":{"content":"NEXT: SELF_STUDY CONTINUE"},"done":true}).to_string()
}

fn open() -> Command {
    Command::Open {
        source: SOURCE.into(),
        line: 1,
    }
}

#[test]
fn continue_never_yields_a_recovery_input_in_any_reader_state() {
    // Long enough that the first page cannot reach the end of the file.
    let (_temp, reader) = setup(&"row of ordinary source\n".repeat(4000));

    // 1. No bookmark at all: the root map, not a recovery.
    let fresh = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(fresh.input_kind, InputKind::Map);
    assert!(
        !fresh.evidence_scope.starts_with(RECOVERY_HEADER),
        "a first CONTINUE is a map, never a recovery: {}",
        fresh.evidence_scope
    );

    // 2. A prepared but undelivered page: the same page is re-offered.
    let pending = reader.prepare(open()).unwrap().page.unwrap();
    let repeat = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(repeat.input_kind, InputKind::SourcePage);
    assert_eq!(repeat.page.as_ref().unwrap(), &pending);

    // 3. Delivered mid-file: CONTINUE advances to the next byte interval.
    reader
        .delivered(&pending.id, &wire(&pending.text), &response())
        .unwrap();
    let advanced = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(advanced.input_kind, InputKind::SourcePage);
    let advanced_page = advanced.page.clone().unwrap();
    assert_eq!(
        advanced_page.start.byte, pending.end.byte,
        "CONTINUE resumes at the exact next byte"
    );

    // 4. Walk to end of file; CONTINUE then reports end of file, not a recovery.
    let mut page = advanced_page;
    for _ in 0..64 {
        reader
            .delivered(&page.id, &wire(&page.text), &response())
            .unwrap();
        let next = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
        assert_ne!(
            next.input_kind,
            InputKind::Recovery,
            "no CONTINUE anywhere in the walk recovers: {}",
            next.text
        );
        if next.input_kind == InputKind::EndOfFile {
            assert!(
                !next.evidence_scope.starts_with(RECOVERY_HEADER),
                "end of file is its own input kind: {}",
                next.evidence_scope
            );
            return;
        }
        page = next.page.unwrap();
    }
    panic!("the walk did not reach end of file within the bounded loop");
}

#[test]
fn the_recovery_answer_belongs_to_a_named_target_not_to_continue() {
    let (_temp, reader) = setup(&"row of ordinary source\n".repeat(4000));
    let page = reader.prepare(open()).unwrap().page.unwrap();
    reader
        .delivered(&page.id, &wire(&page.text), &response())
        .unwrap();

    // With a live bookmark mid-file, CONTINUE still delivers source.
    let resumed = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(resumed.input_kind, InputKind::SourcePage);

    // The bare topic from her window is what produces the recovery, and it names
    // itself in the reason line. The reader state is identical to the line above.
    let missed = reader
        .prepare_action("SELF_STUDY MAP spectral_bridge")
        .unwrap();
    assert_eq!(missed.input_kind, InputKind::Recovery);
    assert!(
        missed.evidence_scope.starts_with(RECOVERY_HEADER),
        "the recovery kind carries the header the artifact records: {}",
        missed.evidence_scope
    );
    assert!(
        missed
            .text
            .contains("no catalog entries for spectral_bridge"),
        "the reason names the topic that actually failed: {}",
        missed.text
    );

    // And the miss does not disturb the bookmark: the next CONTINUE still reads
    // source, so an interleaved failed target costs the turn, not the position.
    let after = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(after.input_kind, InputKind::SourcePage);
}
