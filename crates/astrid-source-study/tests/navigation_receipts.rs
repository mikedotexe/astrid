use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

fn setup() -> (tempfile::TempDir, Catalog, Reader) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("astrid");
    fs::create_dir_all(root.join("crates/events/src")).unwrap();
    fs::write(
        root.join("crates/events/src/publisher.rs"),
        "pub fn publish() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/events/src/consumer.rs"),
        "pub fn receive() {}\n",
    )
    .unwrap();
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap();
    let reader = Reader::new(catalog.clone(), tmp.path().join("reader"));
    (tmp, catalog, reader)
}

fn wire(output: &StudyOutput, text: &str, reason: &str) -> (String, String) {
    (json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string(),
     json!({"message":{"content":text},"done":true,"done_reason":reason}).to_string())
}

fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let (req, resp) = wire(output, text, "stop");
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &req, &resp).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &req, &resp)
            .unwrap();
    }
}

const MAP: &str = "SELF_STUDY MAP astrid/crates/events/src";
const NAMED: &str = "I could read `publisher.rs` and `consumer.rs` to check my explanation.\nNEXT: SELF_STUDY MAP astrid/crates/events/src";

#[test]
fn verified_navigation_offers_named_sources_and_optional_comparison_without_redirecting() {
    let (_tmp, _, reader) = setup();
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        assert!(!output.text.contains("NAVIGATION RECEIPT —"));
        accept(&reader, &output, NAMED);
    }
    let output = reader.prepare_action(MAP).unwrap();
    assert!(output.page.is_none());
    assert!(output.text.contains("3 consecutive verified study inputs"));
    assert!(
        output
            .text
            .contains("Previously offered and named in your response")
    );
    let comparison = output
        .text
        .lines()
        .find_map(|line| {
            line.split_once("SELF_STUDY SESSION ")
                .map(|(_, targets)| format!("SELF_STUDY SESSION {targets}"))
        })
        .unwrap();
    let session = reader.prepare_action(&comparison).unwrap();
    assert_eq!(session.session_pages.len(), 2);
    accept(
        &reader,
        &session,
        "These are small definitions, not proof of a complete execution path.",
    );
    assert!(
        !reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("NAVIGATION RECEIPT —")
    );
}

#[test]
fn preparation_failure_and_retry_do_not_inflate_navigation_count() {
    let (tmp, _, reader) = setup();
    let output = reader.prepare_action(MAP).unwrap();
    let (req, truncated) = wire(&output, NAMED, "length");
    assert!(
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &req, &truncated)
            .is_err()
    );
    let state: Value =
        serde_json::from_slice(&fs::read(tmp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(state["navigation_history"]["without_source"], 0);
    accept(&reader, &output, NAMED);
    accept(&reader, &output, NAMED);
    let state: Value =
        serde_json::from_slice(&fs::read(tmp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(state["navigation_history"]["without_source"], 1);
    for _ in 0..3 {
        reader.prepare_action(MAP).unwrap();
    }
    let state: Value =
        serde_json::from_slice(&fs::read(tmp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(state["navigation_history"]["without_source"], 1);
}

#[test]
fn source_delivery_and_changed_question_reset_the_scoped_streak() {
    let (_tmp, _, reader) = setup();
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        accept(&reader, &output, NAMED);
    }
    let source = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/events/src/publisher.rs 1")
        .unwrap();
    accept(&reader, &source, "I can see this definition.");
    assert!(
        !reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("NAVIGATION RECEIPT —")
    );
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        accept(&reader, &output, NAMED);
    }
    let output = reader.prepare_action(MAP).unwrap();
    accept(
        &reader,
        &output,
        "STUDY_QUESTION: A different question\nNEXT: SELF_STUDY MAP",
    );
    assert!(
        !reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("NAVIGATION RECEIPT —")
    );
}

#[test]
fn old_helper_omission_and_pending_input_survive_without_inventing_history() {
    let (tmp, catalog, reader) = setup();
    let output = reader.prepare_action(MAP).unwrap();
    let path = tmp.path().join("reader/reader-v1.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state.as_object_mut().unwrap().remove("navigation_history");
    state.as_object_mut().unwrap().remove("navigation_offers");
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    let migrated = Reader::new(catalog, tmp.path().join("reader"));
    accept(&migrated, &output, NAMED);
    let next = migrated.prepare_action(MAP).unwrap();
    assert!(!next.text.contains("NAVIGATION RECEIPT —"));
    assert!(next.text.contains("PREVIOUS RESPONSE CHOICE"));
}

#[test]
fn absent_or_private_filename_is_not_promoted_to_a_candidate() {
    let (_tmp, _, reader) = setup();
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        accept(
            &reader,
            &output,
            "I imagine `made_up.rs` and `workspace/private.txt`.\nNEXT: SELF_STUDY MAP",
        );
    }
    let output = reader.prepare_action(MAP).unwrap();
    assert!(output.text.contains("3 consecutive verified study inputs"));
    assert!(!output.text.contains("Previously offered and named"));
}

#[test]
fn bare_list_is_a_source_choice_and_continue_still_resumes_source() {
    use astrid_source_study::response_choice::inspect_response;
    assert_eq!(
        inspect_response("SELF_STUDY LIST astrid", false)
            .selected_next
            .as_deref(),
        Some("SELF_STUDY LIST astrid")
    );
    let (_tmp, _, reader) = setup();
    let source = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/events/src/publisher.rs 1")
        .unwrap();
    accept(&reader, &source, "NEXT: SELF_STUDY LIST astrid");
    let list = reader.prepare_action("SELF_STUDY LIST astrid").unwrap();
    accept(&reader, &list, "NEXT: SELF_STUDY CONTINUE");
    assert!(
        reader
            .prepare_action("SELF_STUDY CONTINUE")
            .unwrap()
            .text
            .contains("End of astrid/crates/events/src/publisher.rs")
    );
}

#[test]
fn late_source_for_same_inquiry_resets_count_without_replacing_newer_choice() {
    let (_tmp, _, reader) = setup();
    let source = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/events/src/publisher.rs 1")
        .unwrap();
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        accept(&reader, &output, NAMED);
    }
    assert!(
        reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("3 consecutive verified study inputs")
    );
    accept(
        &reader,
        &source,
        "The code arrived later.\nNEXT: SELF_STUDY FIND older_choice",
    );
    let output = reader.prepare_action(MAP).unwrap();
    assert!(!output.text.contains("NAVIGATION RECEIPT —"));
    assert!(
        !output
            .text
            .split("PREVIOUS RESPONSE CHOICE")
            .last()
            .unwrap()
            .contains("older_choice")
    );
}

#[test]
fn late_source_for_another_inquiry_does_not_reset_current_inquiry() {
    let (_tmp, _, reader) = setup();
    let source = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/events/src/publisher.rs 1")
        .unwrap();
    let question = reader
        .prepare_action("SELF_STUDY QUESTION NEW Different inquiry")
        .unwrap();
    accept(&reader, &question, "I will explore a different inquiry.");
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        accept(&reader, &output, NAMED);
    }
    accept(&reader, &source, "Older inquiry source is now delivered.");
    assert!(
        reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("3 consecutive verified study inputs")
    );
}

#[test]
fn actual_search_excerpts_retain_their_named_opening_commands() {
    let (_tmp, _, reader) = setup();
    for _ in 0..3 {
        let output = reader.prepare_action("SELF_STUDY FIND pub fn").unwrap();
        assert!(output.text.contains(" — line 1:"));
        accept(&reader, &output, NAMED);
    }
    let next = reader.prepare_action(MAP).unwrap();
    let receipt = next
        .text
        .split("NAVIGATION RECEIPT —")
        .nth(1)
        .unwrap()
        .split("RECALLED ACCOUNT")
        .next()
        .unwrap();
    assert!(receipt.contains("SELF_STUDY OPEN astrid/crates/events/src/publisher.rs 1"));
    assert!(receipt.contains("SELF_STUDY OPEN astrid/crates/events/src/consumer.rs 1"));
    assert!(receipt.contains("SELF_STUDY SESSION"));
}

#[test]
fn ambiguous_basename_does_not_claim_a_precise_previously_named_source() {
    let (_tmp, catalog, reader) = setup();
    let source = catalog
        .resolve("astrid/crates/events/src/publisher.rs")
        .unwrap()
        .path;
    let other = source.parent().unwrap().join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("publisher.rs"), "fn other() {}\n").unwrap();
    for _ in 0..3 {
        let output = reader.prepare_action("SELF_STUDY LIST astrid").unwrap();
        accept(
            &reader,
            &output,
            "I might read `publisher.rs`.\nNEXT: SELF_STUDY LIST astrid",
        );
    }
    let output = reader.prepare_action("SELF_STUDY LIST astrid").unwrap();
    assert!(!output.text.contains("Previously offered and named"));
    accept(
        &reader,
        &output,
        "I mean `astrid/crates/events/src/publisher.rs`.\nNEXT: SELF_STUDY MAP",
    );
    assert!(
        reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("Previously offered and named")
    );
}

#[test]
fn empty_source_session_does_not_claim_that_code_was_supplied() {
    let (_tmp, catalog, reader) = setup();
    for name in ["publisher.rs", "consumer.rs"] {
        let path = catalog
            .resolve(&format!("astrid/crates/events/src/{name}"))
            .unwrap()
            .path;
        fs::write(path, "").unwrap();
    }
    for _ in 0..3 {
        let output = reader.prepare_action(MAP).unwrap();
        accept(&reader, &output, NAMED);
    }
    let session = reader.prepare_action("SELF_STUDY SESSION OPEN astrid/crates/events/src/publisher.rs 1 | OPEN astrid/crates/events/src/consumer.rs 1").unwrap();
    assert_eq!(
        session.input_kind,
        astrid_source_study::InputKind::EndOfFile
    );
    accept(&reader, &session, "Both files are empty.");
    assert!(
        reader
            .prepare_action(MAP)
            .unwrap()
            .text
            .contains("4 consecutive verified study inputs")
    );
}
