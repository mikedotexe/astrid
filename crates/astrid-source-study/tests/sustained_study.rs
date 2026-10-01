use astrid_source_study::{
    Catalog, Command, InputKind, Reader, StudyOutput, recover_local_navigation,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const SECOND: &str = "astrid/crates/example/src/second.rs";
const QUESTION: &str = "Does the `Kernel` struct in `lib.rs` (specifically in `astrid_kernel`) implement the `if !blocked` check within its methods or a wrapper?";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    for name in ["lib.rs", "second.rs"] {
        fs::write(
            root.join("crates/example/src").join(name),
            "pub fn admission(blocked: bool) -> bool { !blocked }\n".repeat(500),
        )
        .unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    )
    .with_runtime_workspace(temp.path().join("workspace"), "astrid");
    (temp, reader)
}
fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
}
fn accept(reader: &Reader, out: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"system","content":out.system_prompt},{"role":"user","content":out.text}]}).to_string();
    let response = json!({"message":{"content":text},"done":true,"done_reason":"stop"}).to_string();
    if let Some(page) = &out.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(out.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

#[test]
fn exact_new_failure_offers_valid_authored_command_without_creating_an_inquiry() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Existing question?")
        .unwrap();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &page,
        "STUDY_NOTE: Retain this account.\nNEXT: REST",
    );
    let before = state(&temp);
    let broken = format!("SELF_STUDY NEW {QUESTION}");
    assert!(Command::parse(&broken).is_err());
    let recovery = recover_local_navigation(&broken).unwrap();
    assert_eq!(
        recovery.commands,
        [format!("SELF_STUDY QUESTION NEW {QUESTION}")]
    );
    let out = reader.prepare_action(&broken).unwrap();
    assert!(out.is_continuation_decision());
    assert!(out.text.contains(&recovery.commands[0]));
    assert!(!out.text.contains("Requested source"));
    let after = state(&temp);
    for field in ["questions", "notebook", "bookmarks", "last_input"] {
        assert_eq!(before[field], after[field], "{field}");
    }
    assert!(matches!(
        Command::parse(&recovery.commands[0]).unwrap(),
        Command::Question(_)
    ));
    reader.prepare_action(&recovery.commands[0]).unwrap();
    assert_eq!(
        state(&temp)["questions"]["entries"]["q2"]["question"],
        QUESTION
    );
}

#[test]
fn recovery_is_bounded_and_does_not_split_command_like_question_text() {
    for question in [
        "Why X AND REST?",
        "Why café AND TURN_OFF?",
        "Why `NEXT: REST` here?",
    ] {
        let command = format!("SELF_STUDY NEW {question}");
        assert!(astrid_source_study::command_boundary::has_study_payload(
            &command
        ));
        let recovered = recover_local_navigation(&command).unwrap();
        assert_eq!(
            recovered.commands,
            [format!("SELF_STUDY QUESTION NEW {question}")]
        );
    }
    for bad in [
        "SELF_STUDY NEW Why?\nNEXT: TURN_OFF",
        "SELF_STUDY NEW a | b",
    ] {
        assert!(recover_local_navigation(bad).is_none());
        let (_temp, reader) = setup();
        let out = reader.prepare_action(bad).unwrap();
        assert!(out.text.contains("No question was created"));
        assert!(!out.text.contains("TURN_OFF"));
    }
    let long = format!("SELF_STUDY NEW {}", "é".repeat(176));
    assert_eq!(
        recover_local_navigation(&long).unwrap().commands,
        ["SELF_STUDY HELP notebook"]
    );
}

#[test]
fn help_is_detached_and_preserves_pending_source_and_authored_history() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Existing question?")
        .unwrap();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let before = state(&temp);
    for topic in ["", "navigation", "notebook", "attention", "unknown"] {
        let command = format!("SELF_STUDY HELP {topic}");
        let revision = reader.preparation_revision().unwrap();
        let output = reader.prepare_once(&command, &revision, &command).unwrap();
        assert_eq!(output.input_kind, InputKind::Help);
        assert!(output.is_continuation_decision() && output.question_id.is_none());
        assert!(output.page.is_none() && output.session_pages.is_empty());
        assert!(!output.text.contains("Existing question?"));
        assert!(output.text.contains("STUDY COMMAND REFERENCE"));
        assert!(output.text.len() + output.system_prompt.len() < 7_000);
        assert_eq!(
            output,
            reader.prepare_once(&command, &revision, &command).unwrap()
        );
        accept(
            &reader,
            &output,
            "STUDY_NOTE: Do not save help as an account.\nSTUDY_QUESTION: Not a new question.\nNEXT: REST",
        );
        let after = state(&temp);
        for field in [
            "questions",
            "notebook",
            "bookmarks",
            "last_input",
            "navigation_history",
        ] {
            assert_eq!(before[field], after[field], "{field}");
        }
    }
    assert_eq!(
        reader.prepare_action("SELF_STUDY CONTINUE").unwrap().page,
        page.page
    );
}

#[test]
fn session_supports_sustained_account_revision_quiet_parking_and_explicit_return() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Is the check advisory?")
        .unwrap();
    let action = format!("SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SECOND} 1");
    let session = reader.prepare_action(&action).unwrap();
    assert!(session.text.contains("sustained synthesis"));
    assert!(session.system_prompt.contains("800-1,500 words"));
    assert!(session.system_prompt.contains("Brief writing and stopping"));
    assert!(!session.system_prompt.contains("up to 600 bytes"));
    let prose = format!(
        "{}END_OF_SYNTHESIS\nSTUDY_NOTE: Earlier hypothesis: advisory.\nNEXT: REST",
        "Two interpretations remain; the source distinguishes them. ".repeat(130)
    );
    accept(&reader, &session, &prose);
    let note = state(&temp)["questions"]["entries"]["q1"]["notebook"]["note"].clone();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let revision = json!({"prior":note["response_sha256"],"text":"The supplied branch returns a condition; its consumer is unresolved.","source":SOURCE,"line":1});
    accept(
        &reader,
        &page,
        &format!("STUDY_REVISE: {revision}\nNEXT: REST"),
    );
    let before = state(&temp);
    reader
        .prepare_action("SELF_STUDY QUESTION PARK q1")
        .unwrap();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!map.text.contains("Is the check advisory?"));
    reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    let notes = reader.prepare_action("SELF_STUDY NOTE").unwrap();
    assert!(notes.text.contains("Earlier hypothesis: advisory."));
    assert!(notes.text.contains("its consumer is unresolved"));
    assert_eq!(before["bookmarks"], state(&temp)["bookmarks"]);
    assert_ne!(
        state(&temp)["questions"]["entries"]["q1"]["status"],
        "resolved"
    );
}

#[test]
fn v11_migration_preserves_exact_pending_input_and_downgrade_fails_closed() {
    let (temp, reader) = setup();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let path = temp.path().join("reader/reader-v1.json");
    let mut old = state(&temp);
    old["version"] = json!(11);
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(reader.prepare_action("SELF_STUDY CONTINUE").unwrap(), page);
    reader.prepare_action("SELF_STUDY HELP").unwrap();
    assert_eq!(state(&temp)["version"], json!(12));
    let mut future = state(&temp);
    future["version"] = json!(13);
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(reader.prepare_action("SELF_STUDY HELP").is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn study_prompt_names_the_exits() {
    // 2026-10-01: the study loop had captured minime's whole cycle because a study turn
    // showed only study verbs. The shared prompt now names the ways out on its own line;
    // nothing schedules or forces them.
    let prompt = astrid_source_study::STUDY_PROMPT;
    assert!(prompt.contains("Leaving the study is always available and is not a failure"));
    for exit in [
        "NEXT: DAYDREAM",
        "NEXT: ASPIRE",
        "NEXT: WRITE START <topic>",
        "NEXT: INTROSPECT",
        "NEXT: REST",
    ] {
        assert!(prompt.contains(exit), "{exit}");
    }
    assert!(prompt.contains("SELF_STUDY CONTINUE resumes the bookmark whenever you return"));
    assert!(
        prompt.contains("800-1,500 words"),
        "the study page invitation is unchanged this step"
    );
}
