use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";
const QUESTION: &str = "Does the gate actually return early?";

fn setup() -> (tempfile::TempDir, Reader) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("astrid/crates/example/src/lib.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(source, "pub fn example() {}\n").unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([(
            "astrid".into(),
            root.path().join("astrid"),
        )]))
        .unwrap(),
        root.path().join("state"),
    )
    .with_runtime_workspace(root.path().join("workspace"), "minime");
    (root, reader)
}

fn state(root: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(root.path().join("state/reader-v1.json")).unwrap()).unwrap()
}

fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":text},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

#[test]
fn leave_inspect_return_and_reread_preserve_words_and_positions() {
    let (root, reader) = setup();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(
        &reader,
        &page,
        &format!(
            "UNVERIFIED_OLD_ACCOUNT\nSTUDY_QUESTION: {QUESTION}\nSTUDY_NOTE: Exact retained words.\nNEXT: REST"
        ),
    );
    let before = state(&root);
    assert!(
        reader
            .prepare_action("SELF_STUDY MAP")
            .unwrap()
            .text
            .contains(QUESTION)
    );
    let parked = reader
        .prepare_action("SELF_STUDY QUESTION PARK NOTEBOOK")
        .unwrap();
    assert!(parked.is_continuation_decision());
    assert!(!parked.text.contains(QUESTION));
    accept(
        &reader,
        &parked,
        &format!("STUDY_QUESTION: {QUESTION}\nNEXT: REST"),
    );
    assert_eq!(state(&root)["questions"]["unthreaded_quiet"], true);
    let quiet = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!quiet.text.contains(QUESTION));
    assert!(!quiet.text.contains("UNVERIFIED_OLD_ACCOUNT"));
    let inspected = reader
        .prepare_action("SELF_STUDY QUESTION NOTEBOOK")
        .unwrap();
    assert_eq!(inspected.input_kind, InputKind::InquiryReview);
    assert!(inspected.text.contains(QUESTION));
    accept(
        &reader,
        &inspected,
        "STUDY_QUESTION: Inspection is not an edit.\nNEXT: REST",
    );
    for key in [
        "question",
        "note",
        "note_history",
        "previous",
        "recent",
        "source_findings",
    ] {
        assert_eq!(state(&root)["notebook"][key], before["notebook"][key]);
    }
    assert_eq!(state(&root)["bookmarks"], before["bookmarks"]);
    assert!(
        state(&root)["questions"]["entries"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    let returned = reader
        .prepare_action("SELF_STUDY QUESTION RETURN NOTEBOOK")
        .unwrap();
    assert!(returned.text.contains(QUESTION));
    assert!(returned.is_continuation_decision());
    assert_eq!(state(&root)["bookmarks"], before["bookmarks"]);
    let eof = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(eof.input_kind, InputKind::EndOfFile);
    accept(&reader, &eof, "NEXT: REST");
    let reread = reader.prepare_action(OPEN).unwrap();
    assert!(!reread.is_continuation_decision());
    assert_eq!(reread.page.unwrap().start.byte, 0);
}

#[test]
fn new_and_focus_do_not_replay_other_findings_and_home_does_not_resume_legacy_question() {
    let (root, reader) = setup();
    accept(
        &reader,
        &reader.prepare_action(OPEN).unwrap(),
        &format!("STUDY_QUESTION: {QUESTION}"),
    );
    let original = state(&root)["notebook"].clone();
    for id in ["q1", "q2"] {
        let new = reader
            .prepare_action("SELF_STUDY QUESTION NEW A voluntary duplicate?")
            .unwrap();
        assert!(new.is_continuation_decision());
        assert!(!new.text.contains("OLD_SAVED_FINDING"));
        let resolved = reader
            .prepare_action(&format!(
                "SELF_STUDY QUESTION RESOLVE {id} OLD_SAVED_FINDING"
            ))
            .unwrap();
        assert!(resolved.is_continuation_decision());
        assert!(!resolved.text.contains(QUESTION));
        assert_eq!(state(&root)["notebook"], original);
    }
    let list = reader.prepare_action("SELF_STUDY QUESTION").unwrap();
    assert!(!list.text.contains("OLD_SAVED_FINDING"));
    assert!(
        reader
            .prepare_action("SELF_STUDY QUESTION REVIEW q1")
            .unwrap()
            .text
            .contains("OLD_SAVED_FINDING")
    );
    let focused = reader.prepare_action("SELF_STUDY QUESTION q2").unwrap();
    assert!(!focused.is_continuation_decision());
    let home = reader.prepare_action("SELF_STUDY QUESTION HOME").unwrap();
    assert!(!home.text.contains(QUESTION));
    assert_eq!(state(&root)["questions"]["entries"]["q2"]["status"], "open");
}

#[test]
fn migration_is_quiet_preserves_pending_wire_and_rejects_newer_state() {
    let (root, reader) = setup();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(&reader, &page, &format!("STUDY_QUESTION: {QUESTION}"));
    let pending = reader.prepare_action(OPEN).unwrap();
    let path = root.path().join("state/reader-v1.json");
    let mut old = state(&root);
    old["version"] = 10.into();
    old["questions"]
        .as_object_mut()
        .unwrap()
        .remove("unthreaded_quiet");
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    // An already offered input is immutable, even across presentation migration.
    assert_eq!(
        reader.prepare_action("SELF_STUDY CONTINUE").unwrap(),
        pending
    );
    accept(&reader, &pending, "NEXT: REST");
    let fresh = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!fresh.text.contains(QUESTION));
    assert_eq!(
        state(&root)["notebook"]["question"],
        old["notebook"]["question"]
    );
    assert_eq!(state(&root)["version"], 11);
    let mut unsupported = state(&root);
    unsupported["version"] = 12.into();
    let bytes = serde_json::to_vec(&unsupported).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(
        reader
            .prepare_action("SELF_STUDY QUESTION RETURN NOTEBOOK")
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn parking_is_idempotent_conflicting_retries_fail_and_changed_source_requires_reselection() {
    let (root, reader) = setup();
    accept(
        &reader,
        &reader.prepare_action(OPEN).unwrap(),
        &format!("STUDY_QUESTION: {QUESTION}"),
    );
    let revision = reader.preparation_revision().unwrap();
    let parked = reader
        .prepare_once("park", &revision, "SELF_STUDY QUESTION PARK NOTEBOOK")
        .unwrap();
    assert_eq!(
        reader
            .prepare_once("park", &revision, "SELF_STUDY QUESTION PARK NOTEBOOK")
            .unwrap(),
        parked
    );
    assert!(
        reader
            .prepare_once("park", &revision, "SELF_STUDY QUESTION RETURN NOTEBOOK")
            .is_err()
    );
    fs::write(
        root.path().join("astrid/crates/example/src/lib.rs"),
        "pub fn changed() {}\n",
    )
    .unwrap();
    reader
        .prepare_action("SELF_STUDY QUESTION RETURN NOTEBOOK")
        .unwrap();
    let recovery = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(recovery.input_kind, InputKind::RevisionRecovery);
    assert_eq!(state(&root)["notebook"]["question"]["text"], QUESTION);
    assert_eq!(
        reader
            .prepare_action(OPEN)
            .unwrap()
            .page
            .unwrap()
            .start
            .byte,
        0
    );
}

#[test]
fn absent_decision_field_keeps_old_serialized_offers_exact() {
    let (_root, reader) = setup();
    let old = serde_json::to_value(reader.prepare_action(OPEN).unwrap()).unwrap();
    assert!(old.get("continuation_decision").is_none());
    let decoded: StudyOutput = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), old);
}
