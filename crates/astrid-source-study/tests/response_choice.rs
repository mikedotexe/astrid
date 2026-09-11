use astrid_source_study::{
    Catalog, Reader, StudyOutput,
    response_choice::{ChoiceReceipt, eligible_choice_line_indices, inspect_response},
    writing::Writer,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    process::{Command, Stdio},
};

fn wire(output: &StudyOutput, text: &str) -> (String, String) {
    (
        json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
        json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string(),
    )
}

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("state"),
    );
    (temp, reader)
}

fn receipt_in(text: &str) -> ChoiceReceipt {
    let line = text
        .lines()
        .find(|line| line.starts_with("{\"input_id\":"))
        .unwrap();
    serde_json::from_str(line).unwrap()
}

#[test]
fn shared_fixture_scanner_and_inspector_contract() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/response_choice_cases.json")).unwrap();
    for case in cases {
        let text = case["text"].as_str().unwrap();
        assert_eq!(
            json!(eligible_choice_line_indices(text)),
            case["eligible_indices"],
            "{}",
            case["name"]
        );
        let feedback = serde_json::to_value(inspect_response(
            text,
            case["private_writing"].as_bool().unwrap(),
        ))
        .unwrap();
        for field in [
            "selected_next",
            "selection_kind",
            "earlier_source_command",
            "recovery_commands",
        ] {
            assert_eq!(feedback[field], case[field], "{} {field}", case["name"]);
        }
    }
}

#[test]
fn cli_inspects_without_roots_or_state_and_matches_library() {
    let text = "SELF_STUDY OPEN astrid/Cargo.toml 1\nNEXT: SELF_STUDY CONTINUE";
    let mut child = Command::new(env!("CARGO_BIN_EXE_astrid-source-study"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            json!({"operation":"analyze_response","text":text})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        actual,
        serde_json::to_value(inspect_response(text, false)).unwrap()
    );
}

#[test]
fn verified_response_choice_carries_identity_without_replacing_action() {
    let (_temp, reader) = setup();
    let first = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let (request, response) = wire(
        &first,
        "SELF_STUDY OPEN astrid/Cargo.toml 1\nNEXT: SELF_STUDY CONTINUE",
    );
    let delivered = reader
        .navigation_delivered(first.navigation_id.as_deref().unwrap(), &request, &response)
        .unwrap();
    assert_eq!(
        delivered
            .choice_feedback
            .as_ref()
            .unwrap()
            .selected_next
            .as_deref(),
        Some("SELF_STUDY CONTINUE")
    );
    let next = reader
        .prepare_action("SELF_STUDY OPEN astrid/Cargo.toml 1")
        .unwrap();
    let choice = receipt_in(&next.text);
    assert_eq!(choice.input_id, delivered.page_id);
    assert_eq!(choice.request_sha256, delivered.request_sha256);
    assert_eq!(choice.response_sha256, delivered.response_sha256);
    assert_eq!(choice.feedback, delivered.choice_feedback.unwrap());
    assert!(
        choice
            .feedback
            .explanation
            .unwrap()
            .contains("earlier command was not substituted")
    );
    assert_eq!(next.page.unwrap().source, "astrid/Cargo.toml");
}

#[test]
fn malformed_find_is_only_recovery_after_verified_delivery() {
    let (_temp, reader) = setup();
    let first = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let (request, response) = wire(&first, "FIND dispatcher.rs");
    assert!(
        reader
            .navigation_delivered(
                first.navigation_id.as_deref().unwrap(),
                &request,
                &response.replace("stop", "length")
            )
            .is_err()
    );
    let still_pending = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!still_pending.text.contains("PREVIOUS RESPONSE CHOICE"));
    let (request, response) = wire(&still_pending, "FIND dispatcher.rs");
    let delivered = reader
        .navigation_delivered(
            still_pending.navigation_id.as_deref().unwrap(),
            &request,
            &response,
        )
        .unwrap();
    let feedback = delivered.choice_feedback.unwrap();
    assert_eq!(feedback.selected_next, None);
    assert_eq!(
        feedback.recovery_commands,
        ["SELF_STUDY FIND dispatcher.rs"]
    );
    assert!(
        feedback
            .explanation
            .as_deref()
            .unwrap()
            .contains("not an executed or queued search")
    );
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert_eq!(receipt_in(&next.text).feedback, feedback);
}

#[test]
fn late_page_delivery_and_idempotent_retry_do_not_replace_newer_choice() {
    let (_temp, reader) = setup();
    let page = reader
        .prepare_action("SELF_STUDY OPEN astrid/Cargo.toml 1")
        .unwrap();
    let navigation = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let (nav_request, nav_response) = wire(&navigation, "NEXT: REST");
    let latest = reader
        .navigation_delivered(
            navigation.navigation_id.as_deref().unwrap(),
            &nav_request,
            &nav_response,
        )
        .unwrap();
    let (request, response) = wire(&page, "NEXT: SELF_STUDY CONTINUE");
    let page_id = &page.page.as_ref().unwrap().id;
    reader.delivered(page_id, &request, &response).unwrap();
    reader.delivered(page_id, &request, &response).unwrap();
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let choice = receipt_in(&next.text);
    assert_eq!(choice.input_id, latest.page_id);
    assert_eq!(choice.feedback.selected_next.as_deref(), Some("REST"));
}

#[test]
fn private_finish_recovery_is_specific_and_does_not_finish_draft() {
    for action in ["FINISH", "finish"] {
        let temp = tempfile::tempdir().unwrap();
        let writer = Writer::new(temp.path().into());
        let first = writer.prepare("WRITE START test recovery").unwrap();
        let (request, response) = wire(&first, &format!("A complete thought.\nNEXT: {action}"));
        assert!(
            writer
                .delivered(
                    first.navigation_id.as_deref().unwrap(),
                    &request,
                    &response.replace("stop", "length")
                )
                .is_err()
        );
        let receipt = writer
            .delivered(first.navigation_id.as_deref().unwrap(), &request, &response)
            .unwrap();
        let feedback = receipt.choice_feedback.unwrap();
        assert_eq!(feedback.selected_next.as_deref(), Some(action));
        assert_eq!(feedback.recovery_commands, ["WRITE FINISH"]);
        assert!(
            feedback
                .explanation
                .as_deref()
                .unwrap()
                .contains("does not mark the draft finished")
        );
        let next = writer.prepare("WRITE CONTINUE").unwrap();
        assert_eq!(receipt_in(&next.text).feedback, feedback);
        let state: Value =
            serde_json::from_slice(&fs::read(temp.path().join("drafts-v1.json")).unwrap()).unwrap();
        assert_eq!(state["drafts"]["d1"]["finished"], json!(false));
    }
}

#[test]
fn receipt_does_not_reseed_private_evidence_or_select_a_command() {
    let temp = tempfile::tempdir().unwrap();
    let writer = Writer::new(temp.path().into());
    let first = writer.prepare("WRITE START a topic").unwrap();
    let (request, response) = wire(
        &first,
        "A passage.\nNEXT: WRITE EVIDENCE SENSITIVE_CARRIED_NOTE",
    );
    writer
        .delivered(first.navigation_id.as_deref().unwrap(), &request, &response)
        .unwrap();
    let fresh = writer.prepare("WRITE START unrelated topic").unwrap();
    assert!(!fresh.text.contains("SENSITIVE_CARRIED_NOTE"));
    let choice = receipt_in(&fresh.text);
    assert!(
        choice
            .feedback
            .selected_next
            .as_deref()
            .unwrap()
            .contains("arguments withheld")
    );
    assert_eq!(
        inspect_response(&choice.render(true), true).selected_next,
        None
    );
    assert!(!choice.render(false).contains("SENSITIVE_CARRIED_NOTE"));
}

#[test]
fn malformed_private_arguments_and_oversized_choices_are_bounded() {
    for action in [
        "WRITE: EVIDENCE SECRET",
        "SELF_STUDY REPLACE WRITE EVIDENCE SECRET",
        "INTROSPECT [WRITE START SECRET]",
    ] {
        let feedback = inspect_response(&format!("NEXT: {action}"), true);
        assert!(!serde_json::to_string(&feedback).unwrap().contains("SECRET"));
    }
    let huge = format!("WRITE {} secret", "verb".repeat(5000));
    let feedback = inspect_response(&format!("NEXT: {huge}"), true);
    assert!(serde_json::to_string(&feedback).unwrap().len() < 512);
}
