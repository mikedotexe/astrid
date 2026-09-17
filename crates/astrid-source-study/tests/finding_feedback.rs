use astrid_source_study::{Catalog, Reader, StudyOutput};
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

fn state_bytes(temp: &tempfile::TempDir) -> Vec<u8> {
    fs::read(temp.path().join("reader/reader-v1.json")).unwrap()
}

fn findings(temp: &tempfile::TempDir) -> Value {
    let state: Value = serde_json::from_slice(&state_bytes(temp)).unwrap();
    state["notebook"]["source_findings"].clone()
}

fn deliver(reader: &Reader, output: &StudyOutput, content: &str, finish: &str) -> bool {
    let request = json!({"messages":[
        {"role":"system","content":output.system_prompt},
        {"role":"user","content":output.text}
    ]})
    .to_string();
    let response =
        json!({"choices":[{"message":{"content":content},"finish_reason":finish}]}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).is_ok()
    } else {
        reader
            .navigation_delivered(
                output.navigation_id.as_deref().unwrap(),
                &request,
                &response,
            )
            .is_ok()
    }
}

fn accept(reader: &Reader, output: &StudyOutput, content: &str) {
    assert!(deliver(reader, output, content, "stop"));
}

fn check_in(output: &StudyOutput) -> &str {
    output
        .text
        .split_once("OPTIONAL STUDY CHECK-IN")
        .unwrap()
        .1
        .split_once("Your findings remain yours to retain, qualify or revise.\n")
        .unwrap()
        .0
}

#[test]
fn rejection_is_visible_without_a_note_or_finding_and_persists_until_an_explicit_update() {
    let (temp, reader) = setup();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SECOND}:1 | An unseen claim."),
    );
    let rejected = findings(&temp);
    assert_eq!(rejected["authored"], json!([]));
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let visible = check_in(&next);
    assert!(visible.contains("0/6 retained; 6 available"));
    assert!(visible.contains("Finding not saved"));
    assert!(visible.contains(&format!("SELF_STUDY OPEN {SECOND} 1")));
    assert!(visible.contains("most recent explicit update"));
    assert!(
        visible.find("LATEST FINDING SAVE/REMOVE RESULTS").unwrap()
            < visible.find("No source-linked finding").unwrap()
    );

    let before = state_bytes(&temp);
    for finish in ["length", "error", "timeout"] {
        assert!(!deliver(
            &reader,
            &next,
            "STUDY_FINDING_DROP: ignored",
            finish
        ));
        assert_eq!(state_bytes(&temp), before);
    }
    accept(
        &reader,
        &next,
        "I can choose to keep exploring. No finding update is chosen.",
    );
    assert_eq!(findings(&temp)["updates"], rejected["updates"]);
    let ordinary = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(check_in(&ordinary).contains("Finding not saved"));
    accept(
        &reader,
        &ordinary,
        "```\nSTUDY_FINDING_DROP: example\n```\nA quoted command is not a choice.",
    );
    assert_eq!(findings(&temp)["updates"], rejected["updates"]);

    // The supplied recovery command opens actual source and enables an explicit
    // retry, without treating the failed save as evidence of source delivery.
    let recovered = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SECOND} 1"))
        .unwrap();
    accept(
        &reader,
        &recovered,
        &format!("STUDY_FINDING: {SECOND}:1 | A function is named other."),
    );
    let saved = findings(&temp);
    assert_eq!(saved["authored"].as_array().unwrap().len(), 1);
    assert_eq!(saved["updates"].as_array().unwrap().len(), 1);
    assert!(saved["updates"][0].as_str().unwrap().starts_with("Saved "));
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(check_in(&next).contains("1/6 retained; 5 available"));
    assert!(!check_in(&next).contains("Finding not saved"));
}

#[test]
fn full_capacity_displays_exact_optional_commands_without_eviction() {
    let (temp, reader) = setup();
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let directives = (1..=6).fold(String::new(), |mut text, line| {
        writeln!(
            text,
            "STUDY_FINDING: {SOURCE}:{line} | Tentative finding {line}."
        )
        .unwrap();
        text
    });
    accept(&reader, &page, &directives);
    let original = findings(&temp)["authored"].clone();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:7 | A seventh finding."),
    );
    assert_eq!(findings(&temp)["authored"], original);
    let receipt = findings(&temp)["updates"].clone();
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let visible = check_in(&next);
    assert!(visible.contains("6/6 retained; 0 available"));
    assert!(visible.contains("Finding not saved: six authored findings"));
    assert!(visible.contains("You may keep them all"));
    assert!(visible.contains("Nothing was evicted"));
    assert!(
        visible.find("LATEST FINDING SAVE/REMOVE RESULTS").unwrap()
            < visible.find("YOUR SOURCE-LINKED FINDINGS").unwrap()
    );
    for (index, finding) in original.as_array().unwrap().iter().enumerate() {
        assert!(visible.contains(&format!(
            "STUDY_FINDING: {SOURCE}:{} | your revised words",
            index.saturating_add(1)
        )));
        assert!(visible.contains(&format!(
            "STUDY_FINDING_DROP: {}",
            finding["id"].as_str().unwrap()
        )));
    }
    accept(
        &reader,
        &next,
        "I choose to keep all six and continue reading.",
    );
    assert_eq!(findings(&temp)["authored"], original);
    assert_eq!(findings(&temp)["updates"], receipt);

    // Use the exact displayed location to revise without requiring a free slot.
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_FINDING: {SOURCE}:1 | I withdraw the earlier certainty."),
    );
    let revised = findings(&temp);
    assert_eq!(revised["authored"].as_array().unwrap().len(), 6);
    assert_eq!(revised["authored"][0]["id"], original[0]["id"]);
    assert!(
        revised["updates"][0]
            .as_str()
            .unwrap()
            .starts_with("Updated ")
    );
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let id = original[1]["id"].as_str().unwrap();
    accept(&reader, &map, &format!("STUDY_FINDING_DROP: {id}"));
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(check_in(&next).contains("5/6 retained; 1 available"));
    assert!(check_in(&next).contains(&format!("Removed authored finding {id}")));
    assert_eq!(findings(&temp)["authored"].as_array().unwrap().len(), 5);
    accept(&reader, &next, &format!("STUDY_FINDING_DROP: {id}"));
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(check_in(&next).contains("No authored finding with that ID"));
    assert!(check_in(&next).contains("5/6 retained; 1 available"));
    assert!(!check_in(&next).contains(&format!("Removed authored finding {id}")));
}

#[test]
fn receipts_stay_with_their_inquiry_after_delayed_delivery_and_restore() {
    let (temp, reader) = setup();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW First inquiry?")
        .unwrap();
    let first = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Second inquiry?")
        .unwrap();
    accept(
        &reader,
        &first,
        &format!("STUDY_FINDING: {SECOND}:1 | First unseen finding."),
    );
    let second = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!second.text.contains("Finding not saved"));
    assert!(findings(&temp).is_null());
    let restored = reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    assert!(check_in(&restored).contains("Finding not saved"));
    assert!(check_in(&restored).contains(&format!("SELF_STUDY OPEN {SECOND} 1")));
    accept(&reader, &restored, "I choose to retain the inquiry.");
    let again = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(check_in(&again).contains("Finding not saved"));
}

#[test]
fn ambiguous_session_anchor_offers_an_exact_single_page_recovery() {
    let (temp, reader) = setup();
    let session = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SOURCE} 1"
        ))
        .unwrap();
    accept(
        &reader,
        &session,
        &format!("STUDY_FINDING: {SOURCE}:1 | An ambiguous fragment."),
    );
    assert_eq!(findings(&temp)["authored"], json!([]));
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(check_in(&map).contains("multiple supplied fragments"));
    let recovery = format!("SELF_STUDY OPEN {SOURCE} 1");
    assert!(check_in(&map).contains(&recovery));
    let source = reader.prepare_action(&recovery).unwrap();
    accept(
        &reader,
        &source,
        &format!("STUDY_FINDING: {SOURCE}:1 | One supplied source fragment."),
    );
    assert_eq!(findings(&temp)["authored"].as_array().unwrap().len(), 1);
    assert!(
        findings(&temp)["updates"][0]
            .as_str()
            .unwrap()
            .starts_with("Saved ")
    );
}
