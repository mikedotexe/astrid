use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(
        root.join("crates/example/src/lib.rs"),
        "pub fn entry() {}\n",
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
}

fn sidecar(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("reader/source-findings-v1.json")).unwrap())
        .unwrap()
}

fn write_state(temp: &tempfile::TempDir, state: &Value) {
    fs::write(
        temp.path().join("reader/reader-v1.json"),
        serde_json::to_vec(state).unwrap(),
    )
    .unwrap();
}

fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"system","content":output.system_prompt},
        {"role":"user","content":output.text}]})
    .to_string();
    let response =
        json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string();
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

fn finding(reader: &Reader, words: &str) {
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    accept(
        reader,
        &output,
        &format!("STUDY_NOTE: Independent authored note.\nSTUDY_FINDING: {SOURCE}:1 | {words}"),
    );
}

fn strip_new_fields(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("source_findings");
        for child in object.values_mut() {
            strip_new_fields(child);
        }
    } else if let Some(array) = value.as_array_mut() {
        for child in array {
            strip_new_fields(child);
        }
    }
}

#[test]
fn old_helper_rewrite_restores_findings_without_replacing_notes_or_pending_offer() {
    let (temp, reader) = setup();
    finding(&reader, "My tentative conclusion survives.");
    let pending = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let mut old = state(&temp);
    let expected = old["notebook"]["source_findings"].clone();
    strip_new_fields(&mut old);
    old["notebook"]["note"]["text"] = "A later authored note from the older helper.".into();
    write_state(&temp, &old);
    let retry = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(
        serde_json::to_value(&retry).unwrap(),
        serde_json::to_value(&pending).unwrap()
    );
    accept(
        &reader,
        &retry,
        "I can retain that conclusion while asking another question.",
    );
    let after = state(&temp);
    assert_eq!(
        after["notebook"]["source_findings"]["authored"],
        expected["authored"]
    );
    assert_eq!(
        after["notebook"]["note"]["text"],
        "A later authored note from the older helper."
    );
    assert!(
        after["notebook"]["previous"]["text"]
            .as_str()
            .unwrap()
            .contains("another question")
    );
}

#[test]
fn explicit_drop_does_not_resurrect_from_stale_inline_or_old_writer_omission() {
    let (temp, reader) = setup();
    finding(&reader, "A conclusion I may remove.");
    let stale = state(&temp)["notebook"]["source_findings"].clone();
    let id = stale["authored"][0]["id"].as_str().unwrap();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(&reader, &map, &format!("STUDY_FINDING_DROP: {id}"));
    assert_eq!(sidecar(&temp)["notebooks"]["home"]["authored"], json!([]));
    let mut old = state(&temp);
    old["notebook"]["source_findings"] = stale;
    write_state(&temp, &old);
    let restored = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!restored.text.contains("A conclusion I may remove."));
    assert_eq!(
        state(&temp)["notebook"]["source_findings"]["authored"],
        json!([])
    );
    let mut old = state(&temp);
    strip_new_fields(&mut old);
    write_state(&temp, &old);
    reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert_eq!(
        state(&temp)["notebook"]["source_findings"]["authored"],
        json!([])
    );
}

#[test]
fn home_and_stable_inquiry_ids_restore_without_leaking_through_the_global_mirror() {
    let (temp, reader) = setup();
    finding(&reader, "Home-only finding.");
    reader
        .prepare_action("SELF_STUDY QUESTION NEW First?")
        .unwrap();
    finding(&reader, "First inquiry only.");
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Second?")
        .unwrap();
    assert!(sidecar(&temp)["notebooks"]["q2"].is_null());
    let mut old = state(&temp);
    // A stale global mirror must never populate a fresh, explicitly empty owner.
    let stale = old["questions"]["entries"]["q1"]["notebook"]["source_findings"].clone();
    strip_new_fields(&mut old);
    old["notebook"]["source_findings"] = stale;
    write_state(&temp, &old);
    reader.prepare_action("SELF_STUDY QUESTION q2").unwrap();
    assert!(state(&temp)["notebook"].get("source_findings").is_none());
    finding(&reader, "Second inquiry only.");
    let mut old = state(&temp);
    strip_new_fields(&mut old);
    write_state(&temp, &old);
    for (command, expected) in [
        ("SELF_STUDY QUESTION q1", "First inquiry only."),
        ("SELF_STUDY QUESTION q2", "Second inquiry only."),
        ("SELF_STUDY QUESTION HOME", "Home-only finding."),
    ] {
        reader.prepare_action(command).unwrap();
        let current = state(&temp);
        let authored = current["notebook"]["source_findings"]["authored"]
            .as_array()
            .unwrap();
        assert_eq!(authored.len(), 1);
        assert_eq!(authored[0]["words"], expected);
    }
}

#[test]
fn corrupt_or_unsupported_sidecar_is_preserved_and_does_not_change_reader_state() {
    let (temp, reader) = setup();
    finding(&reader, "Preserve this evidence.");
    let path = temp.path().join("reader/source-findings-v1.json");
    let original = fs::read(temp.path().join("reader/reader-v1.json")).unwrap();
    for invalid in [
        b"{unfinished".as_slice(),
        br#"{"schema":"future_sidecar","notebooks":{"home":null}}"#.as_slice(),
        br#"{"schema":"source_findings_sidecar_v1","notebooks":{"home":{"authored":false}}}"#
            .as_slice(),
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(reader.prepare_action("SELF_STUDY MAP").is_err());
        assert_eq!(fs::read(&path).unwrap(), invalid);
        assert_eq!(
            fs::read(temp.path().join("reader/reader-v1.json")).unwrap(),
            original
        );
    }
}

#[test]
fn interrupted_new_inquiry_sidecar_does_not_block_or_create_an_inquiry() {
    let (temp, reader) = setup();
    finding(&reader, "Existing home finding.");
    let path = temp.path().join("reader/source-findings-v1.json");
    let mut interrupted = sidecar(&temp);
    // Simulate sidecar replacement succeeding for QUESTION NEW before reader
    // checkpoint replacement. The new question has no findings or source data.
    interrupted["notebooks"]["q1"] = Value::Null;
    fs::write(&path, serde_json::to_vec(&interrupted).unwrap()).unwrap();
    reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(state(&temp)["questions"]["entries"].get("q1").is_none());
    assert!(sidecar(&temp)["notebooks"].get("q1").is_none());
    assert_eq!(
        state(&temp)["notebook"]["source_findings"]["authored"][0]["words"],
        "Existing home finding."
    );
    // Actual authored findings with a missing owner are never silently pruned.
    let mut nonempty = sidecar(&temp);
    nonempty["notebooks"]["q1"] = nonempty["notebooks"]["home"].clone();
    let bytes = serde_json::to_vec(&nonempty).unwrap();
    fs::write(&path, &bytes).unwrap();
    let before = fs::read(temp.path().join("reader/reader-v1.json")).unwrap();
    assert!(reader.prepare_action("SELF_STUDY MAP").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        fs::read(temp.path().join("reader/reader-v1.json")).unwrap(),
        before
    );
}
