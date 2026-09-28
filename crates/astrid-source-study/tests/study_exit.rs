use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput, response_choice};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";
const HASH: &str = "fde7a774aa642dda0bb51971f42d6331ca629a44088769785f244887f335267e";

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
    );
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
fn actual_hash_commands_receive_state_aware_rejection_without_mutation() {
    let (root, reader) = setup();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(
        &reader,
        &page,
        "STUDY_QUESTION: Where is the gate?\nNEXT: REST",
    );
    let before = state(&root);
    for action in [
        format!("QUESTION RESOLVE {HASH}"),
        format!("SELF_STUDY QUESTION RESOLVE {HASH}"),
        "SELF_STUDY QUESTION RESOLVE q1".into(),
    ] {
        let output = reader.prepare_action(&action).unwrap();
        assert_eq!(output.input_kind, InputKind::Recovery);
        assert!(output.text.contains("Study command rejected"));
        assert!(output.text.contains("No numbered inquiry is selected"));
        assert!(output.text.contains("STUDY_QUESTION: -"));
        assert!(
            output
                .text
                .contains("SELF_STUDY QUESTION lists existing inquiry IDs")
        );
        assert!(!output.text.contains("RECALLED ACCOUNT"));
        let after = state(&root);
        assert_eq!(before["questions"], after["questions"]);
        assert_eq!(before["bookmarks"], after["bookmarks"]);
        assert_eq!(before["notebook"], after["notebook"]);
    }
}

#[test]
fn real_inquiry_can_resolve_but_bare_question_never_executes() {
    let (root, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Is the gate implemented here?")
        .unwrap();
    let before = state(&root)["questions"].clone();
    let rejected = reader.prepare_action("QUESTION RESOLVE q1").unwrap();
    assert_eq!(rejected.input_kind, InputKind::Recovery);
    assert!(rejected.text.contains("SELF_STUDY QUESTION RESOLVE q1"));
    assert_eq!(state(&root)["questions"], before);
    reader
        .prepare_action("SELF_STUDY QUESTION RESOLVE q1 I have not established its location.")
        .unwrap();
    let saved = state(&root);
    assert_eq!(saved["questions"]["active"], Value::Null);
    assert_eq!(
        saved["questions"]["entries"]["q1"]["status"],
        "resolved by you"
    );
    assert_eq!(
        saved["questions"]["entries"]["q1"]["finding"],
        "I have not established its location."
    );
}

#[test]
fn repeated_eof_offers_a_decision_preserves_uncertainty_and_allows_deliberate_rereading() {
    let (root, reader) = setup();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(
        &reader,
        &page,
        "UNSUPPORTED_ARCHITECTURE_CONFIRMED\nSTUDY_QUESTION: Where is the gate?\nNEXT: REST",
    );
    let bookmarks = state(&root)["bookmarks"].clone();
    for _ in 0..3 {
        let eof = reader.prepare_action("SELF_STUDY").unwrap();
        assert_eq!(eof.input_kind, InputKind::EndOfFile);
        assert!(eof.page.is_none());
        assert!(eof.system_prompt.contains("continuation decision"));
        assert!(eof.text.contains("Where is the gate?"));
        assert!(!eof.text.contains("UNSUPPORTED_ARCHITECTURE_CONFIRMED"));
        assert!(!eof.text.contains("RECALLED ACCOUNT"));
        accept(
            &reader,
            &eof,
            "I leave the question unanswered.\nNEXT: REST",
        );
        assert_eq!(state(&root)["bookmarks"], bookmarks);
        assert!(
            state(&root)["questions"]["entries"]
                .as_object()
                .unwrap()
                .is_empty()
        );
    }
    let eof = reader.prepare_action("SELF_STUDY").unwrap();
    accept(&reader, &eof, "STUDY_QUESTION: -\nNEXT: REST");
    assert_eq!(state(&root)["notebook"]["question"], Value::Null);
    assert_eq!(
        reader.prepare_action(OPEN).unwrap().input_kind,
        InputKind::SourcePage
    );
}

#[test]
fn numbered_inquiry_decisions_do_not_replay_a_saved_finding() {
    let (_root, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Where is the gate?")
        .unwrap();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(&reader, &page, "I remain uncertain.\nNEXT: REST");
    reader
        .prepare_action("SELF_STUDY QUESTION RESOLVE q1 UNSUPPORTED_SAVED_FINDING")
        .unwrap();
    reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    for action in ["SELF_STUDY", "QUESTION RESOLVE q1"] {
        let decision = reader.prepare_action(action).unwrap();
        assert!(decision.text.contains("Selected inquiry q1"));
        assert!(decision.text.contains("Where is the gate?"));
        assert!(decision.text.contains("SELF_STUDY QUESTION REVIEW q1"));
        assert!(!decision.text.contains("UNSUPPORTED_SAVED_FINDING"));
    }
    let review = reader
        .prepare_action("SELF_STUDY QUESTION REVIEW q1")
        .unwrap();
    assert!(review.text.contains("UNSUPPORTED_SAVED_FINDING"));
}

#[test]
fn invalid_selection_receipt_supplies_recovery_without_substituting_a_choice() {
    let command = format!("QUESTION RESOLVE {HASH}");
    let choice = response_choice::inspect_response(&format!("NEXT: {command}"), false);
    assert_eq!(choice.selected_next.as_deref(), Some(command.as_str()));
    assert_eq!(choice.recovery_commands, ["SELF_STUDY QUESTION"]);
    assert!(choice.explanation.unwrap().contains("response hashes"));
    assert!(
        response_choice::inspect_response(&command, false)
            .selected_next
            .is_none()
    );
    let rest = response_choice::inspect_response(&format!("{command}\nNEXT: REST"), false);
    assert_eq!(rest.selected_next.as_deref(), Some("REST"));
}

#[test]
fn bare_question_recovery_is_visible_but_never_executes_or_replaces_a_choice() {
    let (_root, reader) = setup();
    for command in [
        "SELF_STUDY QUESTION",
        "SELF_STUDY QUESTION NEW Where is the gate?",
    ] {
        let recovery = reader.prepare_action("QUESTION HOME").unwrap();
        assert!(recovery.system_prompt.contains("NEXT: SELF_STUDY QUESTION"));
        accept(&reader, &recovery, command);
        let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
        assert!(next.text.contains("unselected_study_command"));
        assert!(next.text.contains("no NEXT: prefix"));
        let feedback = response_choice::inspect_response(command, false);
        assert!(feedback.selected_next.is_none());
        assert_eq!(feedback.recovery_commands, [command]);
        for wrapped in [
            format!("> {command}"),
            format!("```text\n{command}\n```"),
            format!("<think>\n{command}\n</think>"),
        ] {
            let feedback = response_choice::inspect_response(&wrapped, false);
            assert!(feedback.selected_next.is_none());
            assert!(feedback.recovery_commands.is_empty());
        }
        assert!(
            response_choice::inspect_response(command, true)
                .recovery_commands
                .is_empty()
        );
        let explicit = response_choice::inspect_response(&format!("{command}\nNEXT: REST"), false);
        assert_eq!(explicit.selected_next.as_deref(), Some("REST"));
        assert!(explicit.recovery_commands.is_empty());
    }
}

#[test]
fn misplaced_directive_is_rejected_without_storage_and_valid_update_still_works() {
    let (root, reader) = setup();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(
        &reader,
        &page,
        "STUDY_QUESTION: A legacy question.\nNEXT: REST",
    );
    let before = state(&root);
    let directive = "STUDY_FINDING: astrid/crates/example/src/lib.rs:1 | A check and apply_identity_config (authorization).";
    let choice = response_choice::inspect_response(&format!("NEXT: {directive}"), false);
    assert_eq!(choice.selected_next.as_deref(), Some(directive));
    assert!(
        choice
            .explanation
            .unwrap()
            .contains("Notebook update not applied")
    );
    for command in [
        directive,
        "STUDY_QUESTION:-",
        "STUDY_NOTE: A note and TURN_OFF",
        "STUDY_REVISE: {}",
        "STUDY_FINDING_DROP: absent",
        "STUDY_RELATION: hypothesis | missing",
    ] {
        assert!(astrid_source_study::command_boundary::has_study_payload(
            command
        ));
        let rejected = reader.prepare_action(command).unwrap();
        assert_eq!(rejected.input_kind, InputKind::Recovery);
        assert!(rejected.text.contains("Notebook update not applied"));
        assert!(rejected.text.contains("without NEXT:"));
        let after = state(&root);
        for field in ["notebook", "questions", "bookmarks"] {
            assert_eq!(before[field], after[field]);
        }
    }
    let page = reader.prepare_action(OPEN).unwrap();
    accept(
        &reader,
        &page,
        &format!("{directive}\nSTUDY_QUESTION: -\nNEXT: REST"),
    );
    assert_eq!(state(&root)["notebook"]["question"], Value::Null);
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(map.text.contains("A check and apply_identity_config"));
    assert!(map.text.contains("1/6 retained"));
}
