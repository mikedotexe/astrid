use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
fn setup() -> (tempfile::TempDir, Reader) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("astrid");
    fs::create_dir_all(repo.join("crates/example/src")).unwrap();
    fs::write(repo.join("crates/example/src/lib.rs"),
        "fn validate() { eprintln!(\"missing import\"); }\nfn caller() { validate(); proceed(); }\nfn proceed() {}\n").unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), repo)])).unwrap(),
        tmp.path().join("reader"),
    );
    (tmp, reader)
}
fn accept(reader: &Reader, output: &StudyOutput, content: &str) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":content},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(
                output.navigation_id.as_deref().unwrap(),
                &request,
                &response,
            )
            .unwrap();
    }
}
fn state(tmp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(tmp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
}
fn open(reader: &Reader) -> StudyOutput {
    reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap()
}

#[test]
fn note_stays_quiet_until_opened_and_revision_preserves_both_accounts_and_anchor() {
    let (tmp, reader) = setup();
    let first = open(&reader);
    accept(
        &reader,
        &first,
        "STUDY_NOTE: ORIGINAL_ASSUMPTION rejects loading.\nSTUDY_QUESTION: What happens on failure?",
    );
    let prior = state(&tmp)["notebook"]["note"].clone();
    let page = open(&reader);
    assert!(!page.text.contains("ORIGINAL_ASSUMPTION"));
    assert!(page.text.contains("SELF_STUDY NOTE"));
    accept(&reader, &page, "I will inspect the note explicitly.");
    let note = reader.prepare_action("SELF_STUDY NOTE").unwrap();
    assert!(note.text.contains("ORIGINAL_ASSUMPTION"));
    assert_eq!(note.input_kind, InputKind::Notebook);
    accept(&reader, &note, "NEXT: SELF_STUDY CONTINUE");
    let page = open(&reader);
    let revision = json!({"prior":prior["response_sha256"],"text":"Logs and continues, contrary to my earlier account.","source":SOURCE,"line":2});
    accept(&reader, &page, &format!("STUDY_REVISE: {revision}"));
    let saved = state(&tmp);
    assert_eq!(saved["notebook"]["note_history"][1]["previous"], prior);
    assert_eq!(
        saved["notebook"]["note_history"][1]["counterevidence"]["page_id"],
        page.page.as_ref().unwrap().id
    );
    assert!(
        saved["notebook"]["note"]["text"]
            .as_str()
            .unwrap()
            .contains("Logs and continues")
    );
    assert_eq!(
        saved["notebook"]["question"]["text"],
        "What happens on failure?"
    );
    let opened = reader.prepare_action("SELF_STUDY NOTE").unwrap();
    assert!(opened.text.contains("ORIGINAL_ASSUMPTION"));
    assert!(opened.text.contains("Logs and continues"));
}

#[test]
fn stale_or_unsupplied_revision_does_not_replace_the_note() {
    let (tmp, reader) = setup();
    let first = open(&reader);
    accept(&reader, &first, "STUDY_NOTE: Retain exactly.");
    let prior = state(&tmp)["notebook"]["note"].clone();
    for (hash, line) in [
        ("wrong".to_string(), 2),
        (prior["response_sha256"].as_str().unwrap().into(), 400),
    ] {
        let out = open(&reader);
        accept(
            &reader,
            &out,
            &format!(
                "STUDY_REVISE: {}",
                json!({"prior":hash,"source":SOURCE,"line":line,"text":"Do not save."})
            ),
        );
        assert_eq!(state(&tmp)["notebook"]["note"], prior);
        assert!(
            state(&tmp)["notebook"]["note_feedback"]
                .as_str()
                .unwrap()
                .contains("not saved")
        );
    }
}

#[test]
fn reflection_is_not_source_study_and_preserves_the_pending_page() {
    let (tmp, reader) = setup();
    let first = open(&reader);
    accept(
        &reader,
        &first,
        "STUDY_NOTE: TECHNICAL_NOTE_ONLY\nSTUDY_QUESTION: TECHNICAL_QUESTION_ONLY",
    );
    let pending = open(&reader);
    let before = state(&tmp);
    let reflection = reader.prepare_action("INTROSPECT").unwrap();
    assert_eq!(reflection.input_kind, InputKind::Reflection);
    assert!(!reflection.text.contains("TECHNICAL_"));
    assert!(!reflection.text.contains("RECALLED ACCOUNT"));
    assert!(!reflection.system_prompt.contains("studying your system"));
    accept(
        &reader,
        &reflection,
        "Something unfinished interests me.\nSTUDY_NOTE: Not a study update.\nNEXT: REST",
    );
    let after = state(&tmp);
    assert_eq!(before["notebook"], after["notebook"]);
    assert_eq!(before["bookmarks"], after["bookmarks"]);
    assert_eq!(before["progress"], after["progress"]);
    assert_eq!(before["questions"], after["questions"]);
    assert_eq!(after["last_choice"]["feedback"]["selected_next"], "REST");
    let continued = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(continued.page.unwrap(), pending.page.unwrap());
}

#[test]
fn relate_offers_real_caller_and_definition_without_asserting_a_gate() {
    let (_tmp, reader) = setup();
    let result = reader.prepare_action("SELF_STUDY RELATE validate").unwrap();
    assert!(
        result
            .text
            .contains(&format!("SESSION OPEN {SOURCE} 1 | OPEN {SOURCE} 2"))
    );
    assert!(result.text.contains("log-and-continue"));
    assert!(result.text.contains("binding unverified"));
}

#[test]
fn full_history_rejects_changes_without_eviction_and_migration_has_no_invented_past() {
    let (tmp, reader) = setup();
    let first = open(&reader);
    accept(&reader, &first, "STUDY_NOTE: Migrated exact account.");
    let path = tmp.path().join("reader/reader-v1.json");
    let mut old = state(&tmp);
    old["version"] = 7.into();
    old["notebook"]
        .as_object_mut()
        .unwrap()
        .remove("note_history");
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let view = reader.prepare_action("SELF_STUDY NOTE").unwrap();
    assert!(view.text.contains("Migrated exact account."));
    assert!(state(&tmp)["notebook"]["note_history"].is_null());
    for number in 0..65 {
        let page = open(&reader);
        accept(&reader, &page, &format!("STUDY_NOTE: revision {number}"));
    }
    let saved = state(&tmp);
    assert_eq!(
        saved["notebook"]["note_history"].as_array().unwrap().len(),
        64
    );
    assert_eq!(saved["notebook"]["note"]["text"], "revision 63");
    assert!(
        saved["notebook"]["note_feedback"]
            .as_str()
            .unwrap()
            .contains("Not saved")
    );
}

#[test]
fn revision_is_idempotent_question_scoped_and_corruption_is_preserved() {
    let (tmp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW First?")
        .unwrap();
    let first = open(&reader);
    accept(&reader, &first, "STUDY_NOTE: First account.");
    accept(&reader, &first, "STUDY_NOTE: First account.");
    let prior = state(&tmp)["notebook"]["note"]["response_sha256"].clone();
    assert_eq!(
        state(&tmp)["notebook"]["note_history"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Second?")
        .unwrap();
    let second = open(&reader);
    accept(&reader, &second, "STUDY_NOTE: Independent account.");
    let before = state(&tmp)["notebook"]["note"].clone();
    let second = open(&reader);
    accept(
        &reader,
        &second,
        &format!(
            "STUDY_REVISE: {}",
            json!({"prior":prior,"text":"Wrong inquiry.","source":SOURCE,"line":2})
        ),
    );
    assert_eq!(state(&tmp)["notebook"]["note"], before);
    reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    assert_eq!(state(&tmp)["notebook"]["note"]["text"], "First account.");
    let path = tmp.path().join("reader/reader-v1.json");
    let mut corrupt = state(&tmp);
    corrupt["notebook"]["note_history"][0]["replacement"]["text"] = "Tampered".into();
    let bytes = serde_json::to_vec(&corrupt).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(reader.prepare_action("SELF_STUDY NOTE").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    corrupt["version"] = 999.into();
    let bytes = serde_json::to_vec(&corrupt).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(reader.prepare_action("INTROSPECT").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn comparison_does_not_choose_among_ambiguous_definitions_or_parse_comment_decoys() {
    let (tmp, reader) = setup();
    let path = tmp.path().join("astrid/crates/example/src/other.rs");
    fs::write(
        &path,
        "// fn validate() {}\nconst TEXT: &str = \"validate();\";\n",
    )
    .unwrap();
    assert!(
        reader
            .prepare_action("SELF_STUDY RELATE validate")
            .unwrap()
            .text
            .contains("Optional caller/definition comparison")
    );
    fs::write(&path, "fn validate() {}\n").unwrap();
    assert!(
        !reader
            .prepare_action("SELF_STUDY RELATE validate")
            .unwrap()
            .text
            .contains("Optional caller/definition comparison")
    );
}
