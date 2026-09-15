use astrid_source_study::{Catalog, MAX_INPUT_BYTES, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

fn accept(reader: &Reader, output: &StudyOutput, content: &str) {
    let request = json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string();
    let response =
        json!({"message":{"content":content},"done":true,"done_reason":"stop"}).to_string();
    reader
        .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
        .unwrap();
}

#[test]
fn legal_full_notebook_and_named_sources_fit_without_shortening_saved_thoughts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("astrid");
    let directory = root.join("crates/demo/src");
    fs::create_dir_all(&directory).unwrap();
    for index in 0..100 {
        fs::write(
            directory.join(format!("file_{index:03}.rs")),
            "pub fn example() {}\n",
        )
        .unwrap();
    }
    let first = format!("component_{}.rs", "a".repeat(180));
    let second = format!("component_{}.rs", "b".repeat(180));
    for name in [&first, &second] {
        fs::write(directory.join(name), "pub fn example() {}\n").unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        tmp.path().join("reader"),
    );
    let mut output = reader
        .prepare_action(&format!("SELF_STUDY QUESTION NEW {}", "Q".repeat(340)))
        .unwrap();
    let note = "N".repeat(1600);
    let mut question = format!("Compare `{first}` and `{second}`. ");
    question.push_str(&"?".repeat(500_usize.saturating_sub(question.len())));
    for letter in ['A', 'B', 'C', 'D'] {
        let response = format!(
            "STUDY_QUESTION: {question}\nSTUDY_NOTE: {note}\n{}\nNEXT: SELF_STUDY OPEN astrid/crates/demo/src/{first} 1",
            letter.to_string().repeat(7200)
        );
        accept(&reader, &output, &response);
        output = reader
            .prepare_action("SELF_STUDY MAP astrid/crates/demo/src")
            .unwrap();
        assert!(output.text.len() + output.system_prompt.len() + 32 <= MAX_INPUT_BYTES);
    }
    assert!(output.page.is_none());
    let rendered: Value = serde_json::from_str(
        output
            .text
            .split("RECALLED ACCOUNT — your study notebook")
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
    assert_eq!(rendered["note"]["text"], note);
    assert_eq!(rendered["question"]["text"], question);
    assert_eq!(rendered["previous"]["text"], "D".repeat(7200));
    assert_eq!(rendered["previous"]["complete"], true);
    assert!(rendered["recent"].as_array().unwrap().len() < 3);
    let state: Value =
        serde_json::from_slice(&fs::read(tmp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(state["notebook"]["recent"].as_array().unwrap().len(), 3);
    assert_eq!(state["notebook"]["previous"]["text"], "D".repeat(7200));
    assert_eq!(state["notebook"]["note"]["text"], note);
    assert_eq!(state["notebook"]["question"]["text"], question);
    assert!(state["bookmarks"].as_object().unwrap().is_empty());
}
