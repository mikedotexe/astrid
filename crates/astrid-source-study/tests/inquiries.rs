use astrid_source_study::{Catalog, Command, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::create_dir_all(root.join("crates/example/tests")).unwrap();
    fs::write(
        root.join("crates/example/src/lib.rs"),
        format!(
            "pub struct EventDispatcher {{}}\nimpl EventDispatcher {{}}\n{}",
            "pub fn call() { EventDispatcher::new(); }\n".repeat(900)
        ),
    )
    .unwrap();
    fs::write(
        root.join("crates/example/src/second.rs"),
        "pub fn second() {}\n".repeat(900),
    )
    .unwrap();
    fs::write(
        root.join("crates/example/tests/dispatch.rs"),
        "fn test_dispatch() { EventDispatcher::new(); }\n",
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}
const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const SECOND: &str = "astrid/crates/example/src/second.rs";
fn wire(out: &StudyOutput) -> String {
    json!({"model":"fixture","options":{"num_predict":4096},"messages":[{"role":"system","content":out.system_prompt},{"role":"user","content":out.text}]}).to_string()
}
fn response(text: &str) -> String {
    json!({"message":{"content":text},"done":true,"done_reason":"stop"}).to_string()
}
fn accept(reader: &Reader, out: &StudyOutput, text: &str) {
    if let Some(page) = &out.page {
        reader
            .delivered(&page.id, &wire(out), &response(text))
            .unwrap();
    } else {
        reader
            .navigation_delivered(
                out.navigation_id.as_ref().unwrap(),
                &wire(out),
                &response(text),
            )
            .unwrap();
    }
}
fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
}
#[test]
fn inquiries_restore_notes_park_resolve_and_retain_source_references() {
    let (temp, reader) = setup();
    let q = reader
        .prepare_action("SELF_STUDY QUESTION NEW Who calls the dispatcher?")
        .unwrap();
    assert_eq!(q.question_id.as_deref(), Some("q1"));
    accept(&reader, &q, "STUDY_NOTE: Start with the definition.");
    let a = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &a,
        "STUDY_NOTE: The caller is still unknown.\nNEXT: SELF_STUDY RELATE EventDispatcher",
    );
    let q = reader
        .prepare_action("SELF_STUDY QUESTION NEW How is the journal saved?")
        .unwrap();
    accept(&reader, &q, "STUDY_NOTE: Different question.");
    let restored = reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    assert!(restored.text.contains("The caller is still unknown."));
    assert!(!restored.text.contains("Different question."));
    assert!(restored.text.contains(&format!("OPEN {SOURCE} 1")));
    let parked = reader
        .prepare_action("SELF_STUDY QUESTION PARK q1")
        .unwrap();
    assert!(parked.question_id.is_none());
    assert!(parked.text.contains("parked"));
    let resolved = reader
        .prepare_action("SELF_STUDY QUESTION RESOLVE q2 A test covers the write.")
        .unwrap();
    assert!(resolved.text.contains("resolved by you"));
    assert_eq!(
        state(&temp)["questions"]["entries"]["q2"]["finding"],
        "A test covers the write."
    );
    assert_eq!(state(&temp)["version"], 3);
}
#[test]
fn delayed_source_completion_updates_its_original_question_only() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW First question?")
        .unwrap();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let second = reader
        .prepare_action("SELF_STUDY QUESTION NEW Second question?")
        .unwrap();
    let resumed = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(resumed.question_id, page.question_id);
    assert!(resumed.text.contains("ACTIVE STUDY QUESTION q1"));
    assert!(!resumed.text.contains("Second question?"));
    accept(
        &reader,
        &resumed,
        "STUDY_NOTE: Answer belonging to the first question.",
    );
    let s = state(&temp);
    assert_eq!(s["questions"]["active"], "q2");
    assert!(s["notebook"]["note"].is_null());
    accept(
        &reader,
        &second,
        "STUDY_NOTE: Second question still receives its response.",
    );
    assert_eq!(
        state(&temp)["notebook"]["note"]["text"],
        "Second question still receives its response."
    );
    let first = reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    assert!(
        first
            .text
            .contains("Answer belonging to the first question.")
    );
}
#[test]
fn inquiry_delivery_requires_the_complete_question_context() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Why does this run?")
        .unwrap();
    let out = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let page = out.page.as_ref().unwrap();
    let trimmed = json!({"messages":[{"role":"user","content":page.text}]}).to_string();
    assert!(
        reader
            .delivered(&page.id, &trimmed, &response("Answer"))
            .is_err()
    );
    assert_eq!(state(&temp)["bookmarks"], json!({}));
    accept(&reader, &out, "STUDY_NOTE: Complete input delivered.");
}
#[test]
fn sessions_are_atomic_retryable_and_continue_the_last_selected_source() {
    let (temp, reader) = setup();
    let out = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SECOND} 1"
        ))
        .unwrap();
    assert_eq!(out.input_kind, InputKind::SourceSession);
    assert_eq!(out.session_pages.len(), 2);
    assert_eq!(out, reader.prepare_action("SELF_STUDY CONTINUE").unwrap());
    let incomplete =
        json!({"messages":[{"role":"user","content":out.session_pages[0].text}]}).to_string();
    assert!(
        reader
            .navigation_delivered(
                out.navigation_id.as_ref().unwrap(),
                &incomplete,
                &response("partial evidence")
            )
            .is_err()
    );
    assert_eq!(state(&temp)["bookmarks"], json!({}));
    accept(
        &reader,
        &out,
        "The two pages have different responsibilities.\nNEXT: SELF_STUDY CONTINUE",
    );
    let s = state(&temp);
    assert_eq!(s["bookmarks"].as_object().unwrap().len(), 2);
    let next = reader
        .prepare_action("SELF_STUDY CONTINUE")
        .unwrap()
        .page
        .unwrap();
    assert_eq!(next.source, SECOND);
    assert_eq!(next.start, out.session_pages[1].end);
}
#[test]
fn session_checkpoint_crash_recovers_every_page_once() {
    let (temp, reader) = setup();
    let out = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SECOND} 1"
        ))
        .unwrap();
    let before = fs::read(temp.path().join("reader/reader-v1.json")).unwrap();
    accept(&reader, &out, "STUDY_NOTE: One session note.");
    let delivered = state(&temp);
    fs::write(temp.path().join("reader/reader-v1.json"), before).unwrap();
    reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    let recovered = state(&temp);
    for key in ["bookmarks", "receipts", "progress", "notebook"] {
        assert_eq!(delivered[key], recovered[key], "{key}");
    }
}
#[test]
fn bad_session_and_relationship_requests_preserve_pending_source() {
    let (temp, reader) = setup();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap()
        .page
        .unwrap();
    for action in [
        format!("SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN astrid/nope.rs 1"),
        "SELF_STUDY RELATE ../../secret".into(),
        "SELF_STUDY QUESTION missing".into(),
    ] {
        assert_eq!(
            reader.prepare_action(&action).unwrap().input_kind,
            InputKind::Recovery
        );
        assert_eq!(state(&temp)["pending"]["id"], page.id);
    }
}
#[test]
fn relationships_prioritize_definitions_and_show_tests_without_claiming_calls() {
    let (_temp, reader) = setup();
    let out = reader
        .prepare_action("SELF_STUDY RELATE EventDispatcher")
        .unwrap();
    assert_eq!(out.input_kind, InputKind::Relationships);
    assert!(out.text.contains("Definition candidates"));
    assert!(out.text.contains("Implementation blocks"));
    assert!(out.text.contains("Test occurrences"));
    assert!(out.text.contains("not compiler-resolved"));
    assert!(
        out.text.find("Definition candidates").unwrap()
            < out.text.find("Other references").unwrap()
    );
    assert!(
        reader
            .prepare_action("SELF_STUDY RELATE MissingIdentifier")
            .unwrap()
            .text
            .contains("No exact identifier matches")
    );
}
#[test]
fn trace_last_verifies_input_completion_and_does_not_claim_next_execution() {
    let (temp, reader) = setup();
    let out = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(&reader, &out, "NEXT: SELF_STUDY CONTINUE");
    let before = state(&temp);
    let trace = reader.prepare_action("SELF_STUDY TRACE LAST").unwrap();
    assert_eq!(trace.input_kind, InputKind::RuntimeTrace);
    assert!(trace.text.contains("4096"));
    assert!(trace.text.contains(SOURCE));
    assert!(trace.text.contains("does not establish"));
    accept(
        &reader,
        &trace,
        "I can inspect the actual next action separately.",
    );
    assert_eq!(before["last_input"], state(&temp)["last_input"]);
    assert_eq!(before["bookmarks"], state(&temp)["bookmarks"]);
    let receipt = before["last_input"]["artifact_path"].as_str().unwrap();
    fs::write(receipt, "{}").unwrap();
    assert!(
        reader
            .prepare_action("SELF_STUDY TRACE LAST")
            .unwrap()
            .text
            .contains("hash mismatch")
    );
}
#[test]
fn runtime_trace_restricts_owner_paths_and_preserves_unknown_fields() {
    let (temp, reader) = setup();
    let workspace = temp.path().join("workspace");
    let folder = workspace.join("llm_jobs/jobs/job_minime_fixture");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        workspace.join("llm_jobs/index.json"),
        json!({"recent_jobs":["job_minime_fixture"]}).to_string(),
    )
    .unwrap();
    fs::write(folder.join("job.json"),json!({"job_id":"job_minime_fixture","system":"minime","action_id":"act_minime_fixture","action_text":"SELF_STUDY CONTINUE","status":"completed","created_at":"2026-09-09T17:00:00Z"}).to_string()).unwrap();
    let reader = reader.with_runtime_workspace(workspace, "minime");
    let out = reader
        .prepare_action("SELF_STUDY TRACE act_minime_fixture")
        .unwrap();
    assert!(out.text.contains("job_minime_fixture"));
    assert!(out.text.contains("\"linked_action_manifest\": null"));
    for target in ["../../secret", "job_astrid_fixture", "act_astrid_fixture"] {
        assert!(
            reader
                .prepare_action(&format!("SELF_STUDY TRACE {target}"))
                .unwrap()
                .text
                .contains("never a path")
        );
    }
}
#[test]
fn full_notebook_and_three_page_session_fit_the_shared_wire_budget() {
    let (_temp, reader) = setup();
    let q = reader
        .prepare_action(&format!("SELF_STUDY QUESTION NEW {}", "Q".repeat(340)))
        .unwrap();
    accept(
        &reader,
        &q,
        &format!(
            "STUDY_NOTE: {}\nSTUDY_QUESTION: {}\n{}",
            "\"\\".repeat(340),
            "Q".repeat(340),
            "P".repeat(690)
        ),
    );
    let out = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SECOND} 1 | OPEN {SOURCE} 20"
        ))
        .unwrap();
    assert_eq!(out.session_pages.len(), 3);
    assert!(
        out.text.len() + out.system_prompt.len() + 32 <= astrid_source_study::MAX_INPUT_BYTES,
        "{}",
        out.text.len() + out.system_prompt.len()
    );
    let heading = "RECALLED ACCOUNT — your study notebook";
    let start = out.text.find(heading).unwrap();
    let body = &out.text[start..];
    let json = body
        .split_once('\n')
        .unwrap()
        .1
        .split("\nEnd of study notebook.")
        .next()
        .unwrap();
    let _: Value = serde_json::from_str(json).unwrap();
    accept(&reader, &out, "NEXT: SELF_STUDY CONTINUE");
}
#[test]
fn new_commands_parse_without_rewriting_case_sensitive_symbols() {
    assert_eq!(
        Command::parse("SELF_STUDY RELATE EventDispatcher --page 2").unwrap(),
        Command::Relate {
            symbol: "EventDispatcher".into(),
            page: 2
        }
    );
    for text in [
        "SELF_STUDY QUESTION NEW Why?",
        "SELF_STUDY QUESTION q1",
        "SELF_STUDY QUESTION PARK q1",
        "SELF_STUDY QUESTION RESOLVE q1 Found it.",
        "SELF_STUDY QUESTION --page 2",
    ] {
        let command = Command::parse(text).unwrap();
        let encoded = serde_json::to_string(&command).unwrap();
        let decoded: Command = serde_json::from_str(&encoded).unwrap();
        assert_eq!(command, decoded);
    }
}

#[test]
fn repeated_home_preserves_new_unthreaded_notes() {
    let (_temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW A question?")
        .unwrap();
    let home = reader.prepare_action("SELF_STUDY QUESTION HOME").unwrap();
    accept(&reader, &home, "STUDY_NOTE: Fresh unthreaded thought.");
    let home = reader.prepare_action("SELF_STUDY QUESTION HOME").unwrap();
    assert!(home.text.contains("Fresh unthreaded thought."));
    assert!(Command::parse("SELF_STUDY SESSION SESSION OPEN a 1 | OPEN b 1").is_err());
}
