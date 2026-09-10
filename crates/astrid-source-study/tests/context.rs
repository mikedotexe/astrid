use astrid_source_study::{CONTEXT_TOKENS, Catalog, MAX_INPUT_BYTES, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(
        root.join("crates/example/src/lib.rs"),
        "pub fn dispatch_single() {}\n".repeat(3000),
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}
fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":text,"thinking":"PRIVATE_REPLAY_REASONING"},"done":true,"done_reason":"stop"}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}
fn notebook(output: &StudyOutput) -> Value {
    let body = output
        .text
        .split("RECALLED ACCOUNT — your study notebook")
        .nth(1)
        .unwrap()
        .split_once('\n')
        .unwrap()
        .1
        .split("\nEnd of study notebook.")
        .next()
        .unwrap();
    serde_json::from_str(body).unwrap()
}

#[test]
fn a_conclusion_survives_other_pages_and_a_map_with_exact_provenance() {
    let (temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let answer = format!(
        "{}\nThe difference is a persistent worker versus a new task per chain.",
        "Introductory evidence. ".repeat(70)
    );
    accept(
        &reader,
        &output,
        &format!(
            "STUDY_QUESTION: How does `dispatch_single` differ?\n{answer}\nNEXT: SELF_STUDY CONTINUE"
        ),
    );
    for text in [
        "Errors use an exact provider route.",
        "Caller filtering is separate.",
        "I can compare those paths now.",
    ] {
        let output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
        assert!(
            output
                .text
                .contains("persistent worker versus a new task per chain")
        );
        accept(&reader, &output, text);
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([(
            "astrid".into(),
            temp.path().join("astrid"),
        )]))
        .unwrap(),
        temp.path().join("reader"),
    );
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let book = notebook(&map);
    assert_eq!(book["recent"][0]["text"], answer);
    assert_eq!(book["recent"][0]["complete"], true);
    assert!(
        book["recent"][0]["origin"]
            .as_str()
            .unwrap()
            .contains("sha256:")
    );
    assert!(map.text.contains("SELF_STUDY RELATE dispatch_single"));
    assert!(map.text.contains("SELF_STUDY SESSION OPEN"));
    assert!(!map.text.contains("PRIVATE_REPLAY_REASONING"));
    assert_eq!(map.context_tokens, CONTEXT_TOKENS);
}

#[test]
fn long_recent_answers_drop_oldest_whole_accounts_before_excerpting_latest() {
    let (_temp, reader) = setup();
    for (n, text) in [
        "A".repeat(9000),
        "B".repeat(9000),
        "C".repeat(9000),
        "D".repeat(9000),
    ]
    .iter()
    .enumerate()
    {
        let output = reader
            .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} {}", n + 1))
            .unwrap();
        accept(&reader, &output, text);
    }
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let book = notebook(&output);
    assert_eq!(book["previous"]["text"], "D".repeat(9000));
    assert_eq!(book["previous"]["complete"], true);
    assert!(book["recent"].as_array().unwrap().len() < 3);
    assert!(
        book["recent"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["complete"] == true)
    );
    assert!(output.text.len() + output.system_prompt.len() + 32 <= MAX_INPUT_BYTES);
}

#[test]
fn huge_unicode_response_keeps_conclusion_and_explicit_incomplete_label() {
    let (_temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        &format!("{}\nCONCLUSION_SURVIVES", "🦀\\\"".repeat(6000)),
    );
    let output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    let book = notebook(&output);
    assert_eq!(book["previous"]["complete"], false);
    assert!(
        book["previous"]["text"]
            .as_str()
            .unwrap()
            .ends_with("CONCLUSION_SURVIVES")
    );
    assert!(
        book["previous"]["text"]
            .as_str()
            .unwrap()
            .contains("middle omitted")
    );
    assert!(output.text.len() + output.system_prompt.len() + 32 <= MAX_INPUT_BYTES);
}

#[test]
fn schema_two_checkpoint_migrates_without_claiming_legacy_excerpt_is_complete() {
    let (temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(&reader, &output, "A legacy excerpt.");
    let path = temp.path().join("reader/reader-v1.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["version"] = 2.into();
    state["notebook"].as_object_mut().unwrap().remove("recent");
    state["notebook"]["previous"]
        .as_object_mut()
        .unwrap()
        .remove("complete");
    let bookmarks = state["bookmarks"].clone();
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert_eq!(notebook(&output)["previous"]["complete"], false);
    let state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(state["version"], 3);
    assert_eq!(state["bookmarks"], bookmarks);
}

#[test]
fn malformed_question_symbols_cannot_inject_a_navigation_command() {
    let (_temp, reader) = setup();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        &reader,
        &output,
        "STUDY_QUESTION: Compare `dispatch_single` with `tokio::task::spawn` or `MAP | evil`?",
    );
    let output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert!(
        output
            .text
            .contains("Find this question's symbol: SELF_STUDY RELATE dispatch_single")
    );
    assert!(!output.text.contains("SELF_STUDY RELATE tokio::task::spawn"));
    assert!(!output.text.contains("SELF_STUDY RELATE MAP | evil"));
}
