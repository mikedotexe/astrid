use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";

fn reader(root: &Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.join("astrid"))])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

fn setup() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join(SOURCE);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(
        source,
        "pub struct RestartTracker;\n// fixture\n".repeat(1500),
    )
    .unwrap();
    temp
}

fn state(root: &Path, owner: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join("reader-v1.json")).unwrap()).unwrap()
}

fn prepare(reader: &Reader, id: &str, command: &str) -> StudyOutput {
    reader
        .prepare_once(id, &reader.preparation_revision().unwrap(), command)
        .unwrap()
}

fn deliver(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"system","content":output.system_prompt},
        {"role":"user","content":output.text}]})
    .to_string();
    let response = json!({"message":{"content":text},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

fn conflict(reader: &Reader) {
    let new = prepare(
        reader,
        "question",
        "SELF_STUDY QUESTION NEW Does RestartTracker stop retries?",
    );
    assert!(!new.text.contains("NEXT: SELF_STUDY CONTINUE uses"));
    deliver(
        reader,
        &new,
        "NEXT: SELF_STUDY OPEN RestartTracker::new [837]",
    );
    let invalid = prepare(
        reader,
        "invalid",
        "SELF_STUDY OPEN RestartTracker::new [837]",
    );
    assert_eq!(invalid.input_kind, InputKind::Recovery);
    deliver(reader, &invalid, "NEXT: SELF_STUDY FIND RestartTracker");
    let search = prepare(reader, "search", "SELF_STUDY FIND RestartTracker");
    assert!(search.text.contains(OPEN));
    deliver(
        reader,
        &search,
        &format!("A NAVIGATION_PROSE_MARKER, not a bookmark.\n{OPEN}\nNEXT: SELF_STUDY CONTINUE"),
    );
}

#[test]
fn observed_chain_preserves_choice_and_requires_explicit_source_selection() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let reader = reader(temp.path(), owner);
        conflict(&reader);
        let before = state(temp.path(), owner);
        for index in 0..5 {
            let id = format!("choice-{index}");
            let out = prepare(&reader, &id, "SELF_STUDY CONTINUE");
            assert_eq!(out.input_kind, InputKind::SourceSelectionNeeded);
            assert!(out.continuation_decision && out.is_continuation_decision());
            assert!(out.page.is_none() && out.session_pages.is_empty());
            assert_eq!(out.question_id.as_deref(), Some("q1"));
            assert!(out.text.contains("Does RestartTracker stop retries?"));
            assert!(out.text.contains("no saved") || out.text.contains("No saved"));
            assert!(out.text.contains("was not selected"));
            assert!(out.text.contains(&format!("NEXT: {OPEN}")));
            assert!(!out.text.contains("NAVIGATION_PROSE_MARKER"));
            assert!(!out.text.contains("NAVIGATION RECEIPT"));
            assert!(!out.text.contains("PREVIOUS RESPONSE CHOICE"));
            assert!(out.text.len() < 5000);
            assert!(out.text.matches("Previously offered location").count() <= 4);
            assert_eq!(prepare(&reader, &id, "SELF_STUDY CONTINUE"), out);
            assert!(
                reader
                    .prepare_once(&id, &reader.preparation_revision().unwrap(), OPEN)
                    .is_err()
            );
            deliver(
                &reader,
                &out,
                "Repeated NAVIGATION_PROSE_MARKER.\nNEXT: SELF_STUDY CONTINUE",
            );
            let after = state(temp.path(), owner);
            for key in ["current", "bookmarks", "progress"] {
                assert_eq!(before[key], after[key], "recovery changed {key}");
            }
            for key in ["question", "status", "sources", "finding"] {
                assert_eq!(
                    before["questions"]["entries"]["q1"][key],
                    after["questions"]["entries"]["q1"][key]
                );
            }
            assert!(
                after["questions"]["entries"]["q1"]["notebook"]["previous"]["text"]
                    .as_str()
                    .unwrap()
                    .contains("Repeated NAVIGATION_PROSE_MARKER")
            );
        }
        let out = prepare(&reader, "selected", OPEN);
        assert_eq!(out.input_kind, InputKind::SourcePage);
        assert_eq!(before["bookmarks"], state(temp.path(), owner)["bookmarks"]);
        let pending = prepare(&reader, "pending", "SELF_STUDY");
        assert_eq!(out, pending);
        deliver(
            &reader,
            &out,
            "Exact authored finding.\nNEXT: SELF_STUDY CONTINUE",
        );
        let next = prepare(&reader, "advance", "SELF_STUDY CONTINUE");
        assert_eq!(next.page.unwrap().start, out.page.unwrap().end);
    }
}

#[test]
fn missing_and_changed_candidates_are_checked_without_borrowing_other_inquiry_positions() {
    let temp = setup();
    let reader = reader(temp.path(), "minime");
    conflict(&reader);
    fs::write(
        temp.path().join(SOURCE),
        "// changed bytes, still a valid location\n",
    )
    .unwrap();
    let changed = prepare(&reader, "changed", "SELF_STUDY CONTINUE");
    assert!(changed.text.contains(OPEN));
    assert!(changed.text.contains("current-checkout navigation hint"));
    fs::remove_file(temp.path().join(SOURCE)).unwrap();
    let missing = prepare(&reader, "missing", "SELF_STUDY CONTINUE");
    assert!(!missing.text.contains(OPEN));
    assert!(missing.text.contains("NEXT: SELF_STUDY MAP"));
    fs::write(temp.path().join(SOURCE), "// replacement\n".repeat(1500)).unwrap();
    let out = prepare(&reader, "first", OPEN);
    deliver(&reader, &out, "NEXT: REST");
    let before = state(temp.path(), "minime");
    prepare(
        &reader,
        "new",
        "SELF_STUDY QUESTION NEW Unrelated question?",
    );
    let empty = prepare(&reader, "empty", "SELF_STUDY");
    assert_eq!(empty.input_kind, InputKind::SourceSelectionNeeded);
    assert!(!empty.text.contains(OPEN));
    assert!(!empty.text.contains("Does RestartTracker stop retries?"));
    deliver(&reader, &empty, "NEXT: REST");
    prepare(&reader, "return", "SELF_STUDY QUESTION q1");
    let next = prepare(&reader, "resume", "SELF_STUDY CONTINUE");
    assert_eq!(next.page.unwrap().start, out.page.unwrap().end);
    assert_eq!(before["progress"], state(temp.path(), "minime")["progress"]);
}

#[test]
fn no_candidates_missing_next_and_explicit_map_remain_distinct() {
    let temp = setup();
    let reader = reader(temp.path(), "astrid");
    let first = prepare(&reader, "first", "SELF_STUDY");
    assert_eq!(first.input_kind, InputKind::SourceSelectionNeeded);
    assert!(first.text.contains("unthreaded browsing"));
    assert!(!first.text.contains("Previously offered location"));
    deliver(&reader, &first, "No action chosen. Exact authored words.");
    let before = state(temp.path(), "astrid");
    assert!(before["last_choice"]["feedback"]["selected_next"].is_null());
    let map = prepare(&reader, "map", "SELF_STUDY MAP");
    assert_eq!(map.input_kind, InputKind::Map);
    assert!(!map.is_continuation_decision());
    assert!(
        state(temp.path(), "astrid")["bookmarks"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn v12_pending_input_is_preserved_and_v13_recovery_is_durable() {
    let temp = setup();
    let initial = reader(temp.path(), "minime");
    let first = prepare(&initial, "first", OPEN);
    let path = temp.path().join("minime/reader-v1.json");
    let mut old = state(temp.path(), "minime");
    old["version"] = 12.into();
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let restarted = reader(temp.path(), "minime");
    assert_eq!(first, prepare(&restarted, "pending", "SELF_STUDY CONTINUE"));
    prepare(
        &restarted,
        "new",
        "SELF_STUDY QUESTION NEW No previous position?",
    );
    let recovery = prepare(&restarted, "recover", "SELF_STUDY CONTINUE");
    assert_eq!(state(temp.path(), "minime")["version"], 13);
    assert_eq!(
        recovery,
        prepare(
            &reader(temp.path(), "minime"),
            "recover",
            "SELF_STUDY CONTINUE"
        )
    );
    let mut future = state(temp.path(), "minime");
    future["version"] = 14.into();
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(restarted.prepare_action("SELF_STUDY CONTINUE").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
