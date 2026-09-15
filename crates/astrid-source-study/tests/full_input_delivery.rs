use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";

fn setup() -> (tempfile::TempDir, Reader, StudyOutput) {
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
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    reader
        .navigation_delivered(
            map.navigation_id.as_deref().unwrap(),
            &wire(&map.text),
            &response("STUDY_NOTE: Keep this notebook beside the source."),
        )
        .unwrap();
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    assert!(output.require_complete_input);
    assert!(output.question_id.is_none());
    assert!(output.text.contains("READING COVERAGE"));
    assert!(
        output
            .text
            .contains("Keep this notebook beside the source.")
    );
    (temp, reader, output)
}

fn wire(text: &str) -> String {
    json!({"messages":[{"role":"user","content":text}]}).to_string()
}

fn response(content: &str) -> String {
    json!({"choices":[{"message":{"content":content},"finish_reason":"stop"}]}).to_string()
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

fn unchanged_files(temp: &tempfile::TempDir) -> (Vec<u8>, Vec<u8>) {
    (
        fs::read(temp.path().join("reader/reader-v1.json")).unwrap(),
        fs::read(temp.path().join("reader/source-findings-v1.json")).unwrap(),
    )
}

fn retain_artifact(
    temp: &tempfile::TempDir,
    output: &StudyOutput,
    request: &str,
    response: &str,
) -> Value {
    let page = output.page.as_ref().unwrap();
    let raw = serde_json::to_vec_pretty(&json!({"schema":"source_study_delivery_v1", "page":page,
        "output":output, "request_json":request,"response_json":response}))
    .unwrap();
    let hash = digest(&raw);
    let directory = temp.path().join("reader/deliveries").join(&page.id);
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{hash}.json"));
    fs::write(&path, raw).unwrap();
    json!({"page_id":page.id,"request_sha256":digest(request),"response_sha256":digest(response),
        "artifact_path":path,"artifact_sha256":hash})
}

#[test]
fn new_unthreaded_page_requires_full_coverage_and_notebook_input_before_any_credit() {
    let (temp, reader, output) = setup();
    let page = output.page.as_ref().unwrap();
    let before = unchanged_files(&temp);
    let content = response(&format!(
        "STUDY_FINDING: {SOURCE}:1 | An offered conclusion."
    ));
    let coverage_line = output
        .text
        .lines()
        .find(|line| line.contains("READING COVERAGE"))
        .unwrap();
    for trimmed in [
        page.text.clone(),
        output.text.replace(coverage_line, ""),
        output
            .text
            .replace("Keep this notebook beside the source.", ""),
    ] {
        assert!(
            reader
                .delivered(&page.id, &wire(&trimmed), &content)
                .is_err()
        );
        assert_eq!(unchanged_files(&temp), before);
    }
    let receipt = reader
        .delivered(&page.id, &wire(&output.text), &content)
        .unwrap();
    assert_eq!(
        reader
            .delivered(&page.id, &wire(&output.text), &content)
            .unwrap(),
        receipt
    );
    let saved: Value = serde_json::from_slice(&unchanged_files(&temp).0).unwrap();
    assert!(saved["pending"].is_null());
    assert_eq!(saved["bookmarks"][SOURCE]["id"], page.id);
    assert_eq!(
        saved["notebook"]["source_findings"]["authored"][0]["words"],
        "An offered conclusion."
    );
}

#[test]
fn crash_recovery_refuses_a_trimmed_artifact_and_preserves_the_pending_offer() {
    let (temp, reader, output) = setup();
    let page = output.page.as_ref().unwrap();
    let before = unchanged_files(&temp);
    retain_artifact(
        &temp,
        &output,
        &wire(&page.text),
        &response("Partial input was sent."),
    );
    assert!(reader.prepare_action("SELF_STUDY CONTINUE").is_err());
    assert_eq!(unchanged_files(&temp), before);
}

#[test]
fn receipt_retry_cannot_bypass_full_input_via_pending_or_retained_offer() {
    for keep_pending in [true, false] {
        let (temp, reader, output) = setup();
        let page = output.page.as_ref().unwrap();
        let request = wire(&page.text);
        let response = response("Only page text was sent.");
        let receipt = retain_artifact(&temp, &output, &request, &response);
        let path = temp.path().join("reader/reader-v1.json");
        let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        saved["receipts"][&page.id] = receipt;
        if !keep_pending {
            saved["pending"] = Value::Null;
            saved["pending_page_output"] = Value::Null;
        }
        fs::write(path, serde_json::to_vec(&saved).unwrap()).unwrap();
        let before = unchanged_files(&temp);
        assert!(reader.delivered(&page.id, &request, &response).is_err());
        assert_eq!(unchanged_files(&temp), before);
    }
}

#[test]
fn older_unthreaded_pending_page_contract_remains_accepted() {
    for keep_output in [true, false] {
        let (temp, reader, output) = setup();
        let page = output.page.as_ref().unwrap();
        let path = temp.path().join("reader/reader-v1.json");
        let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if keep_output {
            saved["pending_page_output"]
                .as_object_mut()
                .unwrap()
                .remove("require_complete_input");
            saved["pending_page_output"]["system_prompt"] = "Prior release study prompt.".into();
            let legacy: StudyOutput =
                serde_json::from_value(saved["pending_page_output"].clone()).unwrap();
            assert!(!legacy.require_complete_input);
        } else {
            saved.as_object_mut().unwrap().remove("pending_page_output");
        }
        fs::write(path, serde_json::to_vec(&saved).unwrap()).unwrap();
        reader
            .delivered(
                &page.id,
                &wire(&page.text),
                &response("A legacy pending page completes."),
            )
            .unwrap();
        let saved: Value = serde_json::from_slice(&unchanged_files(&temp).0).unwrap();
        assert_eq!(saved["bookmarks"][SOURCE]["id"], page.id);
    }
}

#[test]
fn omitted_new_flag_still_requires_complete_input_for_the_retained_new_prompt() {
    let (temp, reader, output) = setup();
    let page = output.page.as_ref().unwrap();
    let path = temp.path().join("reader/reader-v1.json");
    let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    saved["pending_page_output"]
        .as_object_mut()
        .unwrap()
        .remove("require_complete_input");
    fs::write(path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let before = unchanged_files(&temp);
    assert!(
        reader
            .delivered(
                &page.id,
                &wire(&page.text),
                &response("Framing was omitted.")
            )
            .is_err()
    );
    assert_eq!(unchanged_files(&temp), before);
    reader
        .delivered(
            &page.id,
            &wire(&output.text),
            &response("Complete input reached this request."),
        )
        .unwrap();
}

#[test]
fn recovery_keeps_exact_source_while_restoring_additive_metadata_stripped_by_old_writer() {
    let (temp, reader, output) = setup();
    let page = output.page.as_ref().unwrap();
    assert!(!page.source_locations.is_empty());
    retain_artifact(
        &temp,
        &output,
        &wire(&output.text),
        &response("A full retained source input."),
    );
    let path = temp.path().join("reader/reader-v1.json");
    let mut old: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    old["pending"]
        .as_object_mut()
        .unwrap()
        .remove("source_locations");
    old["pending_page_output"]["page"]
        .as_object_mut()
        .unwrap()
        .remove("source_locations");
    old["pending_page_output"]
        .as_object_mut()
        .unwrap()
        .remove("require_complete_input");
    fs::write(path, serde_json::to_vec(&old).unwrap()).unwrap();
    reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    let saved: Value = serde_json::from_slice(&unchanged_files(&temp).0).unwrap();
    assert!(saved["pending"].is_null());
    assert_eq!(
        saved["bookmarks"][SOURCE]["source_locations"],
        serde_json::to_value(&page.source_locations).unwrap()
    );
    assert_eq!(saved["bookmarks"][SOURCE]["text"], page.text);
}

#[test]
fn recovery_rejects_differing_nonempty_metadata_or_differing_stable_source() {
    for change_metadata in [true, false] {
        let (temp, reader, output) = setup();
        let mut altered = output.clone();
        let page = altered.page.as_mut().unwrap();
        if change_metadata {
            page.source_locations[0].name = "A different lexical claim".into();
        } else {
            page.text.push_str("\nDifferent source bytes");
        }
        retain_artifact(
            &temp,
            &altered,
            &wire(&output.text),
            &response("A retained source input."),
        );
        let before = unchanged_files(&temp);
        assert!(reader.prepare_action("SELF_STUDY CONTINUE").is_err());
        assert_eq!(unchanged_files(&temp), before);
    }
}
