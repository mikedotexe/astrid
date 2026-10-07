use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const FIRST: &str = "astrid/crates/example/src/lib.rs";
const SECOND: &str = "astrid/crates/example/src/other.rs";

fn reader(root: &Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.join("astrid"))])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

fn state(root: &Path, owner: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join("reader-v1.json")).unwrap()).unwrap()
}

fn deliver(reader: &Reader, out: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"user","content":out.text}]}).to_string();
    let response = json!({"message":{"content":text},"done":true}).to_string();
    if let Some(page) = &out.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(out.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

fn assert_quiet(out: &StudyOutput) {
    for absent in [
        "NAVIGATION_PROSE",
        "NAVIGATION RECEIPT",
        "RECALLED ACCOUNT",
        "NEXT: SELF_STUDY CONTINUE uses",
    ] {
        assert!(!out.text.contains(absent), "{absent}: {}", out.text);
    }
}

#[test]
fn eof_map_continue_loop_is_quiet_and_requires_explicit_selection_for_both_owners() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join(FIRST).parent().unwrap()).unwrap();
        fs::write(temp.path().join(FIRST), "pub fn first() {}\n").unwrap();
        fs::write(temp.path().join(SECOND), "pub fn other() {}\n").unwrap();
        let r = reader(temp.path(), owner);
        r.prepare_action("SELF_STUDY QUESTION NEW Where is the consumer?")
            .unwrap();
        let first = r
            .prepare_action(&format!("SELF_STUDY OPEN {FIRST} 1"))
            .unwrap();
        assert!(first.page.as_ref().unwrap().eof);
        assert!(
            first
                .text
                .contains("EOF after verified delivery: CONTINUE has no next bytes")
        );
        deliver(
            &r,
            &first,
            "STUDY_NOTE: Exact retained note.\nNEXT: SELF_STUDY CONTINUE",
        );
        let before = state(temp.path(), owner);
        let search = r.prepare_action("SELF_STUDY FIND other").unwrap();
        deliver(
            &r,
            &search,
            &format!("NAVIGATION_PROSE\nSELF_STUDY OPEN {SECOND} 1\nNEXT: SELF_STUDY CONTINUE"),
        );
        for (index, command) in [
            "SELF_STUDY",
            "SELF_STUDY MAP",
            "SELF_STUDY CONTINUE",
            "SELF_STUDY LIST astrid",
        ]
        .iter()
        .enumerate()
        {
            let restarted = reader(temp.path(), owner);
            let id = format!("recovery-{index}");
            let out = restarted
                .prepare_once(&id, &restarted.preparation_revision().unwrap(), command)
                .unwrap();
            assert_eq!(
                out,
                restarted
                    .prepare_once(&id, &restarted.preparation_revision().unwrap(), command)
                    .unwrap()
            );
            assert!(out.page.is_none());
            assert!(out.text.contains("Where is the consumer?"));
            assert!(
                out.text
                    .contains(&format!("NEXT: SELF_STUDY OPEN {SECOND} 1"))
            );
            assert!(out.text.contains("was not selected"));
            assert_quiet(&out);
            if command.contains("MAP") || command.contains("LIST") {
                assert_eq!(out.input_kind, InputKind::Map);
                assert!(!out.is_continuation_decision());
                assert!(out.text.contains("Saved source position: end of"));
            } else {
                assert_eq!(out.input_kind, InputKind::EndOfFile);
                assert!(out.is_continuation_decision());
                assert!(
                    out.evidence_scope
                        .contains("MAP browses without moving the saved position")
                );
                assert!(!out.text.contains("MAP chooses another source"));
            }
            deliver(
                &restarted,
                &out,
                if index == 0 {
                    "No action selected."
                } else {
                    "NEXT: REST"
                },
            );
            let after = state(temp.path(), owner);
            for key in ["current", "bookmarks", "progress"] {
                assert_eq!(before[key], after[key]);
            }
        }
        let note = r.prepare_action("SELF_STUDY NOTE").unwrap();
        assert!(note.text.contains("Exact retained note."));
        let second = r
            .prepare_action(&format!("SELF_STUDY OPEN {SECOND} 1"))
            .unwrap();
        assert_eq!(state(temp.path(), owner)["bookmarks"], before["bookmarks"]);
        assert_eq!(r.prepare_action("SELF_STUDY CONTINUE").unwrap(), second);
        deliver(&r, &second, "Exact new account.\nNEXT: REST");
        assert!(state(temp.path(), owner)["bookmarks"].get(SECOND).is_some());
    }
}

#[test]
fn older_retained_offers_keep_text_and_default_host_feedback() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join(FIRST).parent().unwrap()).unwrap();
    fs::write(temp.path().join(FIRST), "pub fn first() {}\n").unwrap();
    let r = reader(temp.path(), "minime");
    let page = r
        .prepare_action(&format!("SELF_STUDY OPEN {FIRST} 1"))
        .unwrap();
    let encoded = serde_json::to_value(&page).unwrap();
    assert!(encoded.get("host_feedback_policy").is_none());
    let old: StudyOutput = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(serde_json::to_value(&old).unwrap(), encoded);
    assert_eq!(old, page);
    deliver(&r, &page, "NEXT: SELF_STUDY CONTINUE");
    let eof = r.prepare_action("SELF_STUDY CONTINUE").unwrap();
    let encoded = serde_json::to_value(&eof).unwrap();
    assert_eq!(encoded["host_feedback_policy"], "reader_only");
    assert_eq!(serde_json::from_value::<StudyOutput>(encoded).unwrap(), eof);
}

#[test]
fn pending_source_and_revision_change_are_not_mislabelled_as_current_eof() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join(FIRST).parent().unwrap()).unwrap();
    fs::write(temp.path().join(FIRST), "pub fn first() {}\n").unwrap();
    let r = reader(temp.path(), "astrid");
    let out = r
        .prepare_action(&format!("SELF_STUDY OPEN {FIRST} 1"))
        .unwrap();
    assert!(
        !r.prepare_action("SELF_STUDY MAP")
            .unwrap()
            .text
            .contains("Saved source position: end of")
    );
    deliver(&r, &out, "NEXT: REST");
    fs::write(temp.path().join(FIRST), "pub fn changed() {}\n").unwrap();
    let map = r
        .prepare_action("SELF_STUDY MAP astrid/crates/example/src")
        .unwrap();
    assert!(map.text.contains("Source changed or unreadable"));
    assert!(map.text.contains(&out.page.unwrap().revision.sha256));
    assert_eq!(
        r.prepare_action("SELF_STUDY CONTINUE").unwrap().input_kind,
        InputKind::RevisionRecovery
    );
    r.prepare_action("SELF_STUDY QUESTION NEW Unrelated question?")
        .unwrap();
    assert!(
        !r.prepare_action("SELF_STUDY MAP")
            .unwrap()
            .text
            .contains("Saved source position: end of")
    );
    assert_eq!(
        r.prepare_action("SELF_STUDY CONTINUE").unwrap().input_kind,
        InputKind::SourceSelectionNeeded
    );
}
