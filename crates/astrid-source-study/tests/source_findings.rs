use astrid_source_study::{Catalog, MAX_INPUT_BYTES, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const SECOND: &str = "astrid/crates/example/src/second.rs";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(
        root.join("crates/example/src/lib.rs"),
        (1..=10).fold(String::new(), |mut source, n| {
            writeln!(source, "pub fn supplied_{n}() {{}}").unwrap();
            source
        }),
    )
    .unwrap();
    fs::write(
        root.join("crates/example/src/second.rs"),
        "pub fn other() {}\n",
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn wire(output: &StudyOutput, content: &str, finish: &str) -> (String, String) {
    (
        json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string(),
        json!({"choices":[{"message":{"content":content},"finish_reason":finish}]}).to_string(),
    )
}

fn accept(reader: &Reader, output: &StudyOutput, content: &str) {
    let (request, response) = wire(output, content, "stop");
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

fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
}

fn findings(temp: &tempfile::TempDir) -> Value {
    state(temp)["notebook"]["source_findings"].clone()
}

fn open(reader: &Reader) -> StudyOutput {
    reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap()
}

#[test]
fn authored_words_are_independent_from_the_note_and_exact_supplied_fragment() {
    let (temp, reader) = setup();
    let output = open(&reader);
    let page = output.page.as_ref().unwrap();
    let words = "I think this proves immutable authorization, pending a caller check.";
    accept(
        &reader,
        &output,
        &format!(
            "STUDY_NOTE: A separate general note.\nSTUDY_QUESTION: Who calls it?\nSTUDY_FINDING: {SOURCE}:1 | {words}"
        ),
    );
    let saved = findings(&temp);
    assert_eq!(saved["authored"].as_array().unwrap().len(), 1);
    let finding = &saved["authored"][0];
    assert_eq!(finding["words"], words);
    assert_eq!(
        finding["anchor"]["delivered_line_fragment"],
        "pub fn supplied_1() {}"
    );
    assert_eq!(finding["anchor"]["revision_sha256"], page.revision.sha256);
    assert_eq!(finding["anchor"]["page_id"], page.id);
    assert!(finding.get("verified").is_none());
    assert_eq!(
        state(&temp)["notebook"]["note"]["text"],
        "A separate general note."
    );
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(map.text.contains("correctness is not verified"));
    assert!(map.text.contains("your unverified conclusions"));
    assert!(map.text.contains(&format!("SELF_STUDY OPEN {SOURCE} 1")));
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:100 | Unseen claim."),
    );
    assert_eq!(findings(&temp)["authored"], saved["authored"]);
    assert!(
        findings(&temp)["updates"][0]
            .as_str()
            .unwrap()
            .contains("not in this input")
    );
}

#[test]
fn quotations_examples_and_internal_blocks_do_not_mutate_findings() {
    let (temp, reader) = setup();
    let output = open(&reader);
    let directive = format!("STUDY_FINDING: {SOURCE}:1 | This is only an example.");
    let content = format!(
        "> {directive}\n\"{directive}\"\n    {directive}\n```text\n{directive}\n```\n<analysis>\n{directive}\n</analysis>\n<think>{directive}</think>\nThe code is visible; I am not saving a finding."
    );
    accept(&reader, &output, &content);
    assert_eq!(findings(&temp)["authored"], json!([]));
    assert_eq!(findings(&temp)["updates"], json!([]));
    assert!(
        state(&temp)["notebook"]["previous"]["text"]
            .as_str()
            .unwrap()
            .contains("This is only an example.")
    );
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:10 | A chosen hypothesis."),
    );
    let id = findings(&temp)["authored"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &next,
        &format!(
            "~~~\nSTUDY_FINDING_DROP: {id}\n~~~\n<analysis>\nSTUDY_FINDING_DROP: {id}\n</analysis>"
        ),
    );
    assert_eq!(findings(&temp)["authored"].as_array().unwrap().len(), 1);
}

#[test]
fn incomplete_error_and_missing_input_cannot_record_and_retry_is_idempotent() {
    let (temp, reader) = setup();
    let output = open(&reader);
    let page = output.page.as_ref().unwrap();
    let content = format!("STUDY_FINDING: {SOURCE}:1 | A tentative answer.");
    let before = state(&temp);
    for finish in [
        "length",
        "content_filter",
        "error",
        "failed",
        "cancelled",
        "canceled",
        "timeout",
    ] {
        let (request, response) = wire(&output, &content, finish);
        let ollama =
            json!({"message":{"content":content},"done":true,"done_reason":finish}).to_string();
        for failed in [&response, &ollama] {
            assert!(
                reader.delivered(&page.id, &request, failed).is_err(),
                "explicit failed termination {finish} must not save findings"
            );
            assert_eq!(state(&temp), before);
        }
    }
    let (request, _) = wire(&output, &content, "stop");
    let error =
        json!({"error":"fixture provider failure","message":{"content":content},"done":true})
            .to_string();
    assert!(reader.delivered(&page.id, &request, &error).is_err());
    assert_eq!(state(&temp), before);
    let (_, response) = wire(&output, &content, "stop");
    assert!(
        reader
            .delivered(&page.id, "{\"messages\":[]}", &response)
            .is_err()
    );
    assert_eq!(state(&temp), before);
    accept(&reader, &output, &content);
    let accepted = state(&temp);
    accept(&reader, &output, &content);
    assert_eq!(state(&temp), accepted);
}

#[test]
fn every_session_page_can_anchor_a_finding_and_late_delivery_keeps_inquiry_ownership() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW First inquiry?")
        .unwrap();
    let session = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SECOND} 1"
        ))
        .unwrap();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Second inquiry?")
        .unwrap();
    accept(
        &reader,
        &session,
        &format!(
            "STUDY_FINDING: {SOURCE}:1 | First source remains uncertain.\nSTUDY_FINDING: {SECOND}:1 | The other function is named here."
        ),
    );
    let saved = state(&temp);
    assert_eq!(
        saved["questions"]["entries"]["q1"]["notebook"]["source_findings"]["authored"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(saved["notebook"]["source_findings"].is_null());
    let second = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &second,
        &format!("STUDY_FINDING: {SOURCE}:1 | This inquiry has not received it."),
    );
    assert_eq!(findings(&temp)["authored"], json!([]));
    let first = reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    assert!(first.text.contains("First source remains uncertain."));
    assert_eq!(findings(&temp)["authored"].as_array().unwrap().len(), 2);
}

#[test]
fn capacity_never_evicts_authored_findings_and_explicit_updates_and_removals_work() {
    let (temp, reader) = setup();
    let output = open(&reader);
    let content = (1..=6).fold(String::new(), |mut content, line| {
        writeln!(
            content,
            "STUDY_FINDING: {SOURCE}:{line} | My finding {line}."
        )
        .unwrap();
        content
    });
    accept(&reader, &output, &content);
    let original = findings(&temp)["authored"].clone();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:7 | Seventh finding."),
    );
    assert_eq!(findings(&temp)["authored"], original);
    assert!(
        findings(&temp)["updates"][0]
            .as_str()
            .unwrap()
            .contains("Nothing was evicted")
    );
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:1 | I now disagree with my earlier claim."),
    );
    assert_eq!(findings(&temp)["authored"][0]["id"], original[0]["id"]);
    assert_eq!(
        findings(&temp)["authored"][0]["words"],
        "I now disagree with my earlier claim."
    );
    let id = original[1]["id"].as_str().unwrap();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING_DROP: {id}\nSTUDY_FINDING: {SOURCE}:7 | Seventh finding."),
    );
    let saved = findings(&temp);
    assert_eq!(saved["authored"].as_array().unwrap().len(), 6);
    assert!(
        !saved["authored"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["id"] == id)
    );
    assert!(
        saved["authored"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["words"] == "Seventh finding.")
    );
}

#[test]
fn source_changes_do_not_rewrite_a_saved_revision_or_authored_opinion() {
    let (temp, reader) = setup();
    let output = open(&reader);
    accept(
        &reader,
        &output,
        &format!("STUDY_FINDING: {SOURCE}:1 | My old interpretation."),
    );
    let original = findings(&temp)["authored"][0].clone();
    fs::write(
        temp.path().join("astrid/crates/example/src/lib.rs"),
        "pub fn changed() {}\n",
    )
    .unwrap();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(
        map.text
            .contains("current checkout, which may differ from the saved revision")
    );
    assert_eq!(findings(&temp)["authored"][0], original);
    let changed = open(&reader);
    accept(
        &reader,
        &changed,
        "This new reading does not compel me to change my saved opinion.",
    );
    assert_eq!(findings(&temp)["authored"][0], original);
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:1 | I choose to revise against the new line."),
    );
    assert_eq!(findings(&temp)["authored"][0]["id"], original["id"]);
    assert_ne!(
        findings(&temp)["authored"][0]["anchor"]["revision_sha256"],
        original["anchor"]["revision_sha256"]
    );
}

#[test]
fn malformed_private_unseen_and_oversized_citations_do_not_create_anchors() {
    let (temp, reader) = setup();
    let output = open(&reader);
    accept(
        &reader,
        &output,
        &format!("STUDY_FINDING: {SOURCE}:1 | Keep this."),
    );
    let saved = findings(&temp)["authored"].clone();
    for citation in [
        "astrid/../secret.rs:1",
        "astrid/workspace/journal/private.txt:1",
        "repository/path:<line>",
        "astrid/crates/example/src/lib.rs:0",
        "astrid/crates/example/src/lib.rs :1",
        "astrid/crates/example/src/lib.rs:99999999999999999999999999999",
    ] {
        let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
        accept(
            &reader,
            &map,
            &format!("STUDY_FINDING: {citation} | Not a supplied anchor."),
        );
        assert_eq!(findings(&temp)["authored"], saved);
        assert!(
            findings(&temp)["updates"][0]
                .as_str()
                .unwrap()
                .contains("not saved")
        );
    }
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:1 | {}", "🦀".repeat(151)),
    );
    assert_eq!(findings(&temp)["authored"], saved);
}

#[test]
fn full_findings_notes_and_question_fit_shared_budget_and_legacy_state_still_loads() {
    let (temp, reader) = setup();
    let output = open(&reader);
    let words = "X".repeat(600);
    let content = (1..=6).fold(String::new(), |mut content, line| {
        writeln!(content, "STUDY_FINDING: {SOURCE}:{line} | {words}").unwrap();
        content
    });
    accept(&reader, &output, &content);
    for index in 0..4 {
        let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
        accept(
            &reader,
            &map,
            &format!(
                "STUDY_NOTE: {}\nSTUDY_QUESTION: {}\n{index} {}",
                "N".repeat(1600),
                "Q".repeat(500),
                "P".repeat(7200)
            ),
        );
    }
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(map.text.len() + map.system_prompt.len() + 32 <= MAX_INPUT_BYTES);
    let rendered: Value = serde_json::from_str(
        map.text
            .split("RECALLED ACCOUNT — ")
            .nth(1)
            .unwrap()
            .split_once('\n')
            .unwrap()
            .1
            .split("\nEnd of study notebook.")
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        rendered["source_findings"]["authored"],
        findings(&temp)["authored"]
    );
    assert_eq!(rendered["note"]["text"], "N".repeat(1600));
    assert_eq!(rendered["question"]["text"], "Q".repeat(500));
    // A genuinely old checkpoint has no new sidecar to restore; do not model
    // legacy compatibility by deleting a field from an already-upgraded reader.
    let (legacy_temp, legacy_reader) = setup();
    legacy_reader.prepare_action("SELF_STUDY MAP").unwrap();
    let mut legacy = state(&legacy_temp);
    legacy["notebook"]
        .as_object_mut()
        .unwrap()
        .remove("source_findings");
    fs::write(
        legacy_temp.path().join("reader/reader-v1.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    assert!(legacy_reader.prepare_action("SELF_STUDY MAP").is_ok());
    assert!(findings(&legacy_temp).is_null());
}

#[test]
fn automatic_locations_do_not_promote_raw_string_examples_and_fragments_are_marked() {
    let (temp, reader) = setup();
    fs::write(
        temp.path().join("astrid/crates/example/src/lib.rs"),
        "const EXAMPLE: &str = r#\"\nfn invented_definition() {}\n\"#;\npub fn actual_definition() {}\n",
    ).unwrap();
    let output = open(&reader);
    accept(
        &reader,
        &output,
        "A literal string contains code-shaped example text.",
    );
    let saved = findings(&temp);
    let locations = saved["supplied_locations"].as_array().unwrap();
    assert!(locations.iter().any(|location| location["line"] == 4));
    assert!(!locations.iter().any(|location| location["line"] == 2));

    fs::write(
        temp.path().join("astrid/crates/example/src/lib.rs"),
        format!("// {}\npub fn after_comment() {{}}\n", "🦀".repeat(1700)),
    )
    .unwrap();
    let output = open(&reader);
    assert!(!output.page.as_ref().unwrap().eof);
    accept(
        &reader,
        &output,
        &format!("STUDY_FINDING: {SOURCE}:1 | This is only a fragment of a comment."),
    );
    let saved = findings(&temp);
    let anchor = &saved["authored"][0]["anchor"];
    assert_eq!(anchor["fragment_truncated"], true);
    assert!(anchor["delivered_line_fragment"].as_str().unwrap().len() <= 240);
    assert_eq!(
        anchor["page_end_byte"],
        output.page.as_ref().unwrap().end.byte
    );
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:2 | The next function was not supplied."),
    );
    assert_eq!(findings(&temp)["authored"], saved["authored"]);
}
