use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

fn accept(reader: &Reader, out: &StudyOutput, content: &str) {
    let request = json!({"messages":[{"role":"user","content":out.text}]}).to_string();
    let response = json!({"message":{"content":content},"done":true}).to_string();
    if let Some(page) = &out.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(out.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

#[test]
fn explicit_verification_compares_call_definition_and_consumer_then_retains_revision() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("astrid");
        fs::create_dir_all(root.join("crates/example/src")).unwrap();
        fs::write(root.join("crates/example/src/caller.rs"),
            "fn assemble() {\n    // Historical intention: recent journals provide continuity.\n    let own_journal = read_journal(0);\n    let _diagnostic = make_hint();\n    let hint: Option<String> = None;\n    send(own_journal, hint);\n}\n").unwrap();
        fs::write(root.join("crates/example/src/helper.rs"),
            "fn read_journal(limit: usize) -> Vec<String> {\n    entries().into_iter().take(limit).collect()\n}\n").unwrap();
        let directory = temp.path().join(owner);
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
            directory.clone(),
        )
        .with_runtime_workspace(temp.path().join("workspace"), owner);
        let saved = || -> Value {
            serde_json::from_slice(&fs::read(directory.join("reader-v1.json")).unwrap()).unwrap()
        };
        reader
            .prepare_action("SELF_STUDY QUESTION NEW Is the journal actually supplied?")
            .unwrap();
        let first = reader
            .prepare_action("SELF_STUDY OPEN astrid/crates/example/src/caller.rs 1")
            .unwrap();
        assert_eq!(first.input_kind, InputKind::SourcePage);
        accept(
            &reader,
            &first,
            "STUDY_NOTE: My earlier account said this input supplied recent journals.\nNEXT: SELF_STUDY HELP verification",
        );
        let before = saved();
        let help = reader
            .prepare_action("SELF_STUDY HELP verification")
            .unwrap();
        assert_eq!(help.input_kind, InputKind::Help);
        assert!(help.text.contains("overwritten before the consumer"));
        assert!(help.text.contains("Source study itself runs no code"));
        accept(&reader, &help, "NEXT: SELF_STUDY RELATE read_journal");
        let related = reader
            .prepare_action("SELF_STUDY RELATE read_journal")
            .unwrap();
        assert!(
            related
                .text
                .contains("SELF_STUDY OPEN astrid/crates/example/src/helper.rs 1"),
            "{}",
            related.text
        );
        assert!(
            related
                .text
                .contains("SELF_STUDY OPEN astrid/crates/example/src/caller.rs 3")
        );
        accept(&reader, &related, "NEXT: SELF_STUDY FIND let hint");
        let found = reader.prepare_action("SELF_STUDY FIND let hint").unwrap();
        assert!(
            found
                .text
                .contains("SELF_STUDY OPEN astrid/crates/example/src/caller.rs 5")
        );
        accept(&reader, &found, "NEXT: REST");
        assert_eq!(saved()["bookmarks"], before["bookmarks"]);
        let session = reader.prepare_action("SELF_STUDY SESSION OPEN astrid/crates/example/src/caller.rs 1 | OPEN astrid/crates/example/src/helper.rs 1").unwrap();
        assert_eq!(session.input_kind, InputKind::SourceSession);
        for fact in [
            "read_journal(0)",
            "take(limit)",
            "let hint: Option<String> = None",
            "send(own_journal, hint)",
        ] {
            assert!(session.text.contains(fact));
        }
        let prior = before["questions"]["entries"]["q1"]["notebook"]["note"].clone();
        let revision = json!({"prior":prior["response_sha256"],"text":"This call takes zero entries; my earlier explanation of this input was mistaken. Other paths remain unexamined.","source":"astrid/crates/example/src/caller.rs","line":3});
        accept(
            &reader,
            &session,
            &format!("STUDY_REVISE: {revision}\nNEXT: REST"),
        );
        let after = saved();
        let notebook = &after["questions"]["entries"]["q1"]["notebook"];
        assert_eq!(notebook["note_history"][1]["previous"], prior);
        assert!(
            notebook["note"]["text"]
                .as_str()
                .unwrap()
                .contains("Other paths remain unexamined")
        );
        assert_eq!(after["questions"]["entries"]["q1"]["status"], "open");
        assert_eq!(after["current"], "astrid/crates/example/src/helper.rs");
    }
}
