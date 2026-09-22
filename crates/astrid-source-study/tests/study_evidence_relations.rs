use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const SOURCE_TEXT: &str = "pub enum Outcome { Allowed, Denied }\n\
/// Source-study scope: review-only.\n\
pub fn review() -> f32 { 0.5 }\n\
pub fn enforce(outcome: Outcome) {\n\
    match outcome { Outcome::Allowed => act(), Outcome::Denied => () }\n\
}\n\
pub fn act() {}\n\
// Outcome::Allowed is a historical comment, not a match site.\n\
const EXAMPLE: &str = \"match Outcome::Allowed\";\n\
#[cfg(test)]\n\
mod tests {\n\
    #[test]\n\
    fn example() { let _ = review(); enforce(Outcome::Allowed); }\n\
}\n";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(root.join("crates/example/src/lib.rs"), SOURCE_TEXT).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let wire = json!({"model":"fixture","messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":text},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &wire, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &wire, &response)
            .unwrap();
    }
}

fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
}

fn sidecar(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/source-findings-v1.json")).unwrap())
        .unwrap()
}

#[test]
fn relations_raise_a_sticky_compatibility_floor_without_resetting_existing_findings() {
    let (temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        &format!("STUDY_FINDING: {SOURCE}:1 | Existing ordinary finding."),
    );
    assert_eq!(sidecar(&temp)["schema"], "source_findings_sidecar_v1");
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &output,
        &format!("STUDY_RELATION: hypothesis | {SOURCE}:4 | {SOURCE}:7 | An optional connection."),
    );
    assert_eq!(sidecar(&temp)["schema"], "source_findings_sidecar_v2");
    let saved = state(&temp);
    let findings = saved["notebook"]["source_findings"]["authored"]
        .as_array()
        .unwrap();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["words"], "Existing ordinary finding.");
    let relation_id = findings[1]["id"].as_str().unwrap();
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &output,
        &format!("STUDY_FINDING_DROP: {relation_id}"),
    );
    reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert_eq!(sidecar(&temp)["schema"], "source_findings_sidecar_v2");
    assert_eq!(
        state(&temp)["notebook"]["source_findings"]["authored"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn consumer_search_ranks_parsed_match_before_definition_and_ignores_comment_string_decoys() {
    let (_temp, reader) = setup();
    let output = reader.prepare_action("SELF_STUDY RELATE Outcome").unwrap();
    let uses = output.text.find("Call / match sites").unwrap();
    let definitions = output.text.find("Definition candidates").unwrap();
    assert!(uses < definitions);
    let candidates = &output.text[uses..definitions];
    assert!(candidates.contains(&format!("OPEN {SOURCE} 5")));
    for line in [1, 8, 9, 13] {
        assert!(!candidates.contains(&format!("OPEN {SOURCE} {line} —")));
    }
    assert!(
        output
            .text
            .contains("name binding and execution unverified")
    );
}

#[test]
fn unquoted_consumer_question_offers_case_preserved_symbol_lookup() {
    let (_temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        "STUDY_QUESTION: Where is ApprovalOutcome consumed?",
    );
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(next.text.contains("SELF_STUDY RELATE ApprovalOutcome"));
    assert!(next.text.contains("asks about use or enforcement"));
}

#[test]
fn scope_distinguishes_declared_review_test_and_call_evidence_without_runtime_claims() {
    let (_temp, reader) = setup();
    let review = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 3"))
        .unwrap();
    assert!(review.text.contains("review: declared review-only"));
    assert!(review.text.contains("0 non-test and 1 test call sites"));
    assert!(review.text.contains("does not establish test-only use"));
    let function = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 7"))
        .unwrap();
    assert!(function.text.contains("act: non-test call candidates"));
    let test = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 13"))
        .unwrap();
    assert!(test.text.contains("example: test context"));
    assert!(!test.text.contains("live-called"));
}

#[test]
fn typed_relations_keep_two_delivery_anchors_and_remain_authored_across_reload() {
    let (temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        &format!(
            "STUDY_RELATION: flow | {SOURCE}:5 | {SOURCE}:7 | The allowed arm calls act.\nSTUDY_RELATION: neighborhood | {SOURCE}:1 | {SOURCE}:3 | The enum and review share this file.\nSTUDY_RELATION: hypothesis | {SOURCE}:4 | {SOURCE}:13 | This test may cover the full guard."
        ),
    );
    let saved = state(&temp);
    let findings = saved["notebook"]["source_findings"]["authored"]
        .as_array()
        .unwrap();
    assert_eq!(findings.len(), 3);
    for finding in findings {
        assert_eq!(
            finding["anchor"]["revision_sha256"],
            finding["relation"]["other"]["revision_sha256"]
        );
        assert_eq!(
            finding["anchor"]["page_id"],
            output.page.as_ref().unwrap().id
        );
    }
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(next.text.contains("Authored call/data-flow claim"));
    assert!(next.text.contains("relation unverified"));
    assert!(next.text.contains("\"hypothesis\""));
    assert!(next.text.len() + next.system_prompt.len() < astrid_source_study::MAX_INPUT_BYTES);
    // Ordinary finding replacement intentionally removes the relation at this anchor.
    accept(
        &reader,
        &next,
        &format!("STUDY_FINDING: {SOURCE}:5 | I am revising my earlier conclusion."),
    );
    let saved = state(&temp);
    assert!(
        saved["notebook"]["source_findings"]["authored"][0]
            .get("relation")
            .is_none()
    );
}

#[test]
fn relation_rejects_unsupplied_and_cross_revision_neighborhood_without_changing_findings() {
    let (temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        &format!("STUDY_RELATION: hypothesis | {SOURCE}:1 | {SOURCE}:999 | Missing source."),
    );
    assert_eq!(
        state(&temp)["notebook"]["source_findings"]["authored"],
        json!([])
    );
    // Retain an old anchor, then offer only the other line from a newer revision.
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        &format!("STUDY_FINDING: {SOURCE}:1 | The enum is present."),
    );
    fs::write(
        temp.path().join("astrid/crates/example/src/lib.rs"),
        format!("{SOURCE_TEXT}// changed revision\n"),
    )
    .unwrap();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 7"))
        .unwrap();
    accept(
        &reader,
        &output,
        &format!("STUDY_RELATION: neighborhood | {SOURCE}:1 | {SOURCE}:7 | Same revision?"),
    );
    let saved = state(&temp);
    assert_eq!(
        saved["notebook"]["source_findings"]["authored"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        saved["notebook"]["source_findings"]["updates"]
            .to_string()
            .contains("same file revision")
    );
}

#[test]
fn python_calls_exclude_arguments_literals_and_test_contexts() {
    let (temp, reader) = setup();
    let source = "astrid/crates/example/src/consumer.py";
    fs::write(
        temp.path().join("astrid/crates/example/src/consumer.py"),
        "def gate():\n    pass\ndef consume():\n    gate()\n    sink(gate)\n    text = 'gate()'\ndef test_gate():\n    gate()\n",
    )
    .unwrap();
    let output = reader.prepare_action("SELF_STUDY RELATE gate").unwrap();
    let candidates = output
        .text
        .split("Call / match sites")
        .nth(1)
        .unwrap()
        .split("Definition candidates")
        .next()
        .unwrap();
    assert!(candidates.contains(&format!("OPEN {source} 4")));
    for line in [1, 5, 6, 8] {
        assert!(!candidates.contains(&format!("OPEN {source} {line} —")));
    }
}

#[test]
fn full_relations_fit_and_explicit_revisions_keep_both_anchors() {
    let (temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let words = "w".repeat(600);
    let directives = (1..=6)
        .map(|line| format!("STUDY_RELATION: hypothesis | {SOURCE}:{line} | {SOURCE}:7 | {words}"))
        .collect::<Vec<_>>()
        .join("\n");
    accept(&reader, &output, &directives);
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(output.text.len() + output.system_prompt.len() < astrid_source_study::MAX_INPUT_BYTES);
    assert!(output.text.contains(&format!(
        "STUDY_RELATION: hypothesis | {SOURCE}:1 | {SOURCE}:7 | your revised words"
    )));
    accept(
        &reader,
        &output,
        &format!("STUDY_RELATION: hypothesis | {SOURCE}:1 | {SOURCE}:7 | Revised words."),
    );
    let saved = state(&temp);
    let findings = saved["notebook"]["source_findings"]["authored"]
        .as_array()
        .unwrap();
    assert_eq!(findings.len(), 6);
    assert_eq!(findings[0]["words"], "Revised words.");
    assert_eq!(findings[0]["relation"]["other"]["line"], 7);
    let id = findings[0]["id"].as_str().unwrap();
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(&reader, &output, &format!("STUDY_FINDING_DROP: {id}"));
    assert_eq!(
        state(&temp)["notebook"]["source_findings"]["authored"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
}

#[test]
fn escaped_maximum_relations_preserve_words_and_both_anchors_under_input_pressure() {
    let (temp, reader) = setup();
    let relative = format!(
        "crates/{}/src/{}/quoted.rs",
        "a".repeat(240),
        "b".repeat(30)
    );
    let source = format!("astrid/{relative}");
    let path = temp.path().join("astrid").join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut code = String::new();
    for line in 1..=7 {
        writeln!(
            code,
            "pub fn quoted_{line}() {{ let text = \"{}\"; }}",
            "\\\"".repeat(90)
        )
        .unwrap();
    }
    code.push_str(&"pub fn padding() {}\n".repeat(1000));
    fs::write(path, code).unwrap();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {source} 1"))
        .unwrap();
    let note = "\\\"".repeat(800);
    let question = "\\\"".repeat(250);
    let words = "\\\"".repeat(300);
    let mut directives = format!("STUDY_NOTE: {note}\nSTUDY_QUESTION: {question}\n");
    for line in 1..=6 {
        writeln!(
            directives,
            "STUDY_RELATION: hypothesis | {source}:{line} | {source}:7 | {words}"
        )
        .unwrap();
    }
    accept(&reader, &output, &directives);
    let findings = state(&temp)["notebook"]["source_findings"]["authored"].clone();
    assert_eq!(findings.as_array().unwrap().len(), 6);
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(&reader, &output, &"p".repeat(7200));
    for action in [
        format!("SELF_STUDY OPEN {source} 1"),
        format!("SELF_STUDY SESSION OPEN {source} 1 | OPEN {source} 100"),
    ] {
        let output = reader.prepare_action(&action).unwrap();
        assert!(
            output.text.len() + output.system_prompt.len() + 32
                <= astrid_source_study::MAX_INPUT_BYTES
        );
        let rendered: Value = serde_json::from_str(
            output
                .text
                .split_once("RECALLED ACCOUNT — your study notebook")
                .unwrap()
                .1
                .split_once('\n')
                .unwrap()
                .1
                .split("\nEnd of study notebook.")
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rendered["note"]["text"], note);
        assert_eq!(rendered["question"]["text"], question);
        assert_eq!(rendered["source_findings"]["authored"], findings);
        assert_eq!(
            state(&temp)["notebook"]["source_findings"]["authored"],
            findings
        );
    }
}
