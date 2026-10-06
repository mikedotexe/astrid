use astrid_source_study::{Catalog, Command, InputKind, QuestionCommand, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";

fn reader(root: &Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.join("astrid"))])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

fn setup() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("astrid/crates/example/src/lib.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "pub struct RestartTracker;\n".repeat(600)).unwrap();
    root
}

fn state(root: &Path, owner: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join("reader-v1.json")).unwrap()).unwrap()
}

fn deliver(reader: &Reader, output: &StudyOutput, text: &str) {
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
fn copied_template_is_rejected_before_closure_through_text_and_typed_commands() {
    for owner in ["astrid", "minime"] {
        let root = setup();
        let reader = reader(root.path(), owner);
        reader
            .prepare_action("SELF_STUDY QUESTION NEW Does the retry stop?")
            .unwrap();
        let page = reader.prepare_action(OPEN).unwrap();
        deliver(
            &reader,
            &page,
            "Exact authored uncertainty.\nSTUDY_NOTE: Keep this note.\nNEXT: SELF_STUDY QUESTION RESOLVE q1 [finding]",
        );
        let before = state(root.path(), owner);
        for finding in [
            "[finding]",
            "[FINDING]",
            "[finding, up to 700 bytes]",
            "<finding>",
            "[finding].",
        ] {
            for typed in [false, true] {
                let output = if typed {
                    reader
                        .prepare(Command::Question(QuestionCommand::Resolve {
                            id: "q1".into(),
                            finding: finding.into(),
                        }))
                        .unwrap()
                } else {
                    reader
                        .prepare_action(&format!("SELF_STUDY QUESTION RESOLVE q1 {finding}"))
                        .unwrap()
                };
                assert_eq!(output.input_kind, InputKind::Recovery);
                assert!(output.continuation_decision && output.page.is_none());
                assert!(output.text.contains("template marker"));
                assert!(output.text.contains("no question was changed"));
                assert!(output.text.contains("without a finding"));
                let after = state(root.path(), owner);
                for field in ["questions", "notebook", "bookmarks", "current", "progress"] {
                    assert_eq!(before[field], after[field], "{owner}: {field}");
                }
            }
        }
        let next = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
        assert_eq!(next.page.unwrap().start, page.page.unwrap().end);
    }
}

#[test]
fn valid_closure_can_be_brief_uncertain_bracketed_or_without_a_finding() {
    for finding in [
        "",
        "I do not know.",
        "[Uncertain]",
        "The literal [finding] was a template.",
        "有待確認。",
    ] {
        let root = setup();
        let reader = reader(root.path(), "minime");
        reader
            .prepare_action("SELF_STUDY QUESTION NEW Does the retry stop?")
            .unwrap();
        let output = reader
            .prepare_action(&format!("SELF_STUDY QUESTION RESOLVE q1 {finding}"))
            .unwrap();
        assert_eq!(output.input_kind, InputKind::Questions);
        if finding.is_empty() {
            assert!(output.text.contains("without a finding"));
        }
        let saved = state(root.path(), "minime");
        assert_eq!(
            saved["questions"]["entries"]["q1"]["status"],
            "resolved by you"
        );
        assert_eq!(saved["questions"]["entries"]["q1"]["finding"], finding);
        assert!(saved["questions"]["active"].is_null());
    }
}

#[test]
fn pending_offer_from_old_guidance_is_delivered_exactly_but_cannot_author_a_finding() {
    let root = setup();
    let initial = reader(root.path(), "minime");
    initial
        .prepare_action("SELF_STUDY QUESTION NEW First question?")
        .unwrap();
    let pending = initial.prepare_action(OPEN).unwrap();
    initial
        .prepare_action("SELF_STUDY QUESTION NEW Second question?")
        .unwrap();
    let before = state(root.path(), "minime");
    let restarted = reader(root.path(), "minime");
    deliver(
        &restarted,
        &pending,
        "Authored account is not a replacement finding.\nNEXT: SELF_STUDY QUESTION RESOLVE q1 [finding]",
    );
    let output = restarted
        .prepare_action("SELF_STUDY QUESTION RESOLVE q1 [finding]")
        .unwrap();
    assert_eq!(output.input_kind, InputKind::Recovery);
    let after = state(root.path(), "minime");
    assert_eq!(after["questions"]["active"], "q2");
    assert_eq!(
        after["questions"]["entries"]["q2"],
        before["questions"]["entries"]["q2"]
    );
    assert_eq!(after["questions"]["entries"]["q1"]["status"], "open");
    assert_eq!(after["questions"]["entries"]["q1"]["finding"], "");
    assert_eq!(
        after["questions"]["entries"]["q1"]["notebook"]["previous"]["text"],
        "Authored account is not a replacement finding."
    );
}

#[test]
fn typed_closure_cannot_bypass_finding_byte_limit() {
    let root = setup();
    let reader = reader(root.path(), "astrid");
    reader
        .prepare_action("SELF_STUDY QUESTION NEW First question?")
        .unwrap();
    let before = state(root.path(), "astrid");
    let output = reader
        .prepare(Command::Question(QuestionCommand::Resolve {
            id: "q1".into(),
            finding: "x".repeat(701),
        }))
        .unwrap();
    assert_eq!(output.input_kind, InputKind::Recovery);
    assert_eq!(
        before["questions"],
        state(root.path(), "astrid")["questions"]
    );
    let finding = "x".repeat(700);
    reader
        .prepare_action(&format!("SELF_STUDY QUESTION RESOLVE q1 {finding}"))
        .unwrap();
    assert_eq!(
        state(root.path(), "astrid")["questions"]["entries"]["q1"]["finding"],
        finding
    );
}

#[test]
fn rejected_closure_is_idempotent_after_restart_and_does_not_close_another_inquiry() {
    let root = setup();
    let initial = reader(root.path(), "astrid");
    initial
        .prepare_action("SELF_STUDY QUESTION NEW First question?")
        .unwrap();
    initial
        .prepare_action("SELF_STUDY QUESTION NEW Second question?")
        .unwrap();
    let before = state(root.path(), "astrid");
    let command = "SELF_STUDY QUESTION RESOLVE q1 [finding]";
    let revision = initial.preparation_revision().unwrap();
    let output = initial
        .prepare_once("rejected-closure", &revision, command)
        .unwrap();
    assert_eq!(output.input_kind, InputKind::Recovery);
    let restarted = reader(root.path(), "astrid");
    assert_eq!(
        output,
        restarted
            .prepare_once("rejected-closure", &revision, command)
            .unwrap()
    );
    assert!(
        restarted
            .prepare_once(
                "rejected-closure",
                &revision,
                "SELF_STUDY QUESTION RESOLVE q1 Authored conclusion."
            )
            .is_err()
    );
    assert_eq!(
        before["questions"],
        state(root.path(), "astrid")["questions"]
    );
    assert_eq!(state(root.path(), "astrid")["questions"]["active"], "q2");
    deliver(&restarted, &output, "I retain uncertainty.\nNEXT: REST");
    assert_eq!(
        state(root.path(), "astrid")["questions"]["entries"]["q1"],
        before["questions"]["entries"]["q1"]
    );
}

#[test]
fn guidance_does_not_offer_a_literal_finding_and_historical_values_are_not_rewritten() {
    let root = setup();
    let reader = reader(root.path(), "minime");
    for action in [
        "SELF_STUDY QUESTION NEW Does the retry stop?",
        OPEN,
        "SELF_STUDY QUESTION",
        "SELF_STUDY HELP notebook",
        "QUESTION RESOLVE q1",
    ] {
        let output = reader.prepare_action(action).unwrap();
        assert!(!output.text.contains("[finding"), "{action}");
    }
    let path = root.path().join("minime/reader-v1.json");
    let mut legacy = state(root.path(), "minime");
    legacy["questions"]["entries"]["q1"]["status"] = "resolved by you".into();
    legacy["questions"]["entries"]["q1"]["finding"] = "[finding]".into();
    fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let review = reader
        .prepare_action("SELF_STUDY QUESTION REVIEW q1")
        .unwrap();
    assert!(review.text.contains("template marker"));
    assert!(review.text.contains("[finding]"));
    assert_eq!(
        legacy["questions"],
        state(root.path(), "minime")["questions"]
    );
    let list = reader.prepare_action("SELF_STUDY QUESTION").unwrap();
    assert!(list.text.contains("template marker"));
    assert_eq!(
        legacy["questions"],
        state(root.path(), "minime")["questions"]
    );
}
