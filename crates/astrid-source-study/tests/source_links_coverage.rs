use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";

fn setup(text: &str) -> (tempfile::TempDir, Catalog, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(root.join("crates/example/src/lib.rs"), text).unwrap();
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap();
    let reader = Reader::new(catalog.clone(), temp.path().join("reader"));
    (temp, catalog, reader)
}

fn accept(reader: &Reader, output: &StudyOutput, finish: &str) -> anyhow::Result<()> {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response =
        json!({"choices":[{"message":{"content":"I may revisit this."},"finish_reason":finish}]})
            .to_string();
    reader.delivered(&output.page.as_ref().unwrap().id, &request, &response)?;
    Ok(())
}

#[test]
fn eof_shows_undelivered_prefix_without_giving_preparation_or_failure_credit() {
    let text = format!(
        "pub fn implementation() {{}}\n{}\n#[test]\nfn example() {{\n    implementation();\n}}\n",
        "// retained opening\n".repeat(300)
    );
    let (temp, _, reader) = setup(&text);
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 304"))
        .unwrap();
    let page = output.page.as_ref().unwrap();
    assert!(page.eof && page.start.byte > 0);
    assert!(output.text.contains("End of file is a reading position"));
    assert!(output.text.contains(&format!(
        "bytes 0..{}: SELF_STUDY OPEN {SOURCE} 1",
        page.start.byte
    )));
    assert!(output.text.contains("If this offered page is verified"));
    assert!(accept(&reader, &output, "length").is_err());
    let state: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert!(state["progress"][SOURCE].is_null());
    accept(&reader, &output, "stop").unwrap();
    for action in [
        "SELF_STUDY CONTINUE".to_owned(),
        format!("SELF_STUDY RESUME {SOURCE}"),
    ] {
        let eof = reader.prepare_action(&action).unwrap();
        assert!(eof.page.is_none());
        assert!(eof.text.contains("At the current verified coverage"));
        assert!(eof.text.contains(&format!("SELF_STUDY OPEN {SOURCE} 1")));
    }
}

#[test]
fn separate_byte_ranges_merge_only_after_verified_delivery() {
    let text = "pub fn one() {}\npub fn two() {}\npub fn three() {}\n";
    let (_temp, _, reader) = setup(text);
    let tail = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 3"))
        .unwrap();
    accept(&reader, &tail, "stop").unwrap();
    let whole = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    assert!(
        whole
            .text
            .contains("If this offered page is verified, all bytes")
    );
    accept(&reader, &whole, "stop").unwrap();
    let eof = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert!(
        eof.text
            .contains("At the current verified coverage, all bytes")
    );
    assert!(!eof.text.contains("Optional missing-region read"));
}

#[test]
fn session_coverage_projects_all_offered_pages_atomically() {
    let (_temp, _, reader) = setup("pub fn one() {}\npub fn two() {}\npub fn three() {}\n");
    let session = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {SOURCE} 3"
        ))
        .unwrap();
    assert_eq!(
        session
            .text
            .matches("If this whole offered session is verified, all bytes")
            .count(),
        2
    );
    assert!(!session.text.contains("these source bytes remain"));
}

#[test]
fn tests_offer_implementation_and_definitions_offer_references_as_candidates() {
    let text = format!(
        "pub struct Ack;\npub fn validate(_: &Ack) -> bool {{ true }}\npub fn caller(a: &Ack) -> bool {{ validate(a) }}\n{}#[test]\nfn serializes() {{ let ack = Ack; let _ok = validate(&ack); }}\n",
        "// padding\n".repeat(800)
    );
    let (_temp, _, reader) = setup(&text);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 805"))
        .unwrap()
        .page
        .unwrap();
    assert!(page.text.contains("test syntax: #[test]"));
    assert!(page.text.contains("definition candidate Ack"));
    assert!(page.text.contains(&format!("SELF_STUDY OPEN {SOURCE} 1")));
    assert!(page.text.contains(&format!("SELF_STUDY OPEN {SOURCE} 2")));
    assert!(page.text.contains("not resolved calls or runtime proof"));
    // Bound the page so caller stays outside the delivered interval.
    let text = format!(
        "pub fn check() {{}}\n{}pub fn caller() {{ check(); }}\n",
        "// padding\n".repeat(800)
    );
    let (_temp, _, reader) = setup(&text);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap()
        .page
        .unwrap();
    assert!(page.text.contains("reference candidate in caller"));
    assert!(page.text.contains(&format!("SELF_STUDY OPEN {SOURCE} 802")));
}

#[test]
fn old_pending_input_is_replayed_exactly_across_new_reader() {
    let (temp, _, reader) = setup("pub fn sample() {}\n");
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let path = temp.path().join("reader/reader-v1.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let old_text = "An older complete prepared framing with unchanged source identity.";
    state["pending_page_output"]["text"] = old_text.into();
    state["pending_page_output"]["system_prompt"] = "old prompt".into();
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    for action in [
        "SELF_STUDY CONTINUE".to_owned(),
        format!("SELF_STUDY RESUME {SOURCE}"),
    ] {
        let resumed = reader.prepare_action(&action).unwrap();
        assert_eq!(resumed.text, old_text);
        assert_eq!(resumed.system_prompt, "old prompt");
        assert_eq!(resumed.page, output.page);
    }
}

#[test]
fn older_writer_cannot_hide_an_intervening_question_update() {
    let (temp, _, reader) = setup("pub fn sample() {}\n");
    let original = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let request = json!({"messages":[{"role":"user","content":map.text}]}).to_string();
    let response =
        json!({"message":{"content":"STUDY_QUESTION: What consumes this value?"},"done":true})
            .to_string();
    reader
        .navigation_delivered(map.navigation_id.as_deref().unwrap(), &request, &response)
        .unwrap();
    let path = temp.path().join("reader/reader-v1.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state
        .as_object_mut()
        .unwrap()
        .remove("pending_page_context_stale");
    fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    let resumed = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(resumed.page, original.page);
    assert!(resumed.text.contains("What consumes this value?"));
}
