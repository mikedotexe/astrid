//! Synthetic presentation measurements, not a test of belief or model behavior.
use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt::Write as _, fs};

const QUESTION: &str = "SYNTHETIC_QUESTION: Does the named diagnostic enforce a gate?";
const NOTE: &str = "SYNTHETIC_NOTE: This interpretation remains unverified.";
const SOURCE: &str = "astrid/crates/example/src/lib.rs";

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn accept(reader: &Reader, output: &StudyOutput, prose: &str) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":prose},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

fn measurement(owner: &str, route: &str, output: &StudyOutput) -> Value {
    let recalled_bytes = output
        .text
        .split_once("RECALLED ACCOUNT")
        .and_then(|(_, tail)| tail.split_once("End of study notebook."))
        .map_or(0, |(body, _)| body.len());
    json!({
        "owner":owner,
        "route":route,
        "input_kind":output.input_kind,
        "system_bytes":output.system_prompt.len(),
        "user_bytes":output.text.len(),
        "recalled_section_body_bytes":recalled_bytes,
        "current_source_rendered_bytes":output.page.as_ref().map_or(0, |p| p.text.len()),
        "prior_account_markers":(0..4).filter(|index| output.text.contains(&format!("SYNTHETIC_ACCOUNT_{index}"))).count(),
        "question_occurrences":output.text.matches(QUESTION).count(),
        "saved_note_quoted":output.text.contains(NOTE),
        "generation_requested":output.generation_requested,
    })
}

#[test]
fn source_recall_and_quiet_routes_remain_distinct_for_both_owners() {
    let mut measurements = Vec::new();
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join(SOURCE);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        let mut source_text = String::new();
        for index in 0..240 {
            writeln!(source_text, "pub fn synthetic_{index}() {{}}").unwrap();
        }
        fs::write(&source, &source_text).unwrap();
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([(
                "astrid".into(),
                temp.path().join("astrid"),
            )]))
            .unwrap(),
            temp.path().join("state"),
        )
        .with_runtime_workspace(temp.path().join("workspace"), owner);
        reader
            .prepare_action(&format!("SELF_STUDY QUESTION NEW {QUESTION}"))
            .unwrap();
        for index in 0..4 {
            let page = reader
                .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} {}", index + 1))
                .unwrap();
            let prose = format!(
                "SYNTHETIC_ACCOUNT_{index}\n{}\nSTUDY_QUESTION: {QUESTION}\nSTUDY_NOTE: {NOTE}\nNEXT: REST",
                "A synthetic authored interpretation is not an observed enforcement result. "
                    .repeat(20)
            );
            accept(&reader, &page, &prose);
        }
        let page = reader
            .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 5"))
            .unwrap();
        assert_eq!(page.input_kind, InputKind::SourcePage);
        assert!(!page.text.contains(NOTE));
        assert!(page.text.contains("SYNTHETIC_ACCOUNT_3"));
        measurements.push(measurement(owner, "active-source-page", &page));
        accept(&reader, &page, "NEXT: REST");

        let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
        assert!(map.page.is_none());
        measurements.push(measurement(owner, "explicit-map-active-inquiry", &map));
        let reflection = reader.prepare_action("INTROSPECT").unwrap();
        assert_eq!(reflection.input_kind, InputKind::Reflection);
        assert!(!reflection.text.contains(QUESTION));
        assert!(!reflection.text.contains("SYNTHETIC_ACCOUNT_"));
        measurements.push(measurement(owner, "fresh-reflection", &reflection));

        let state_path = temp.path().join("state/reader-v1.json");
        let before: Value = serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
        let parked = reader
            .prepare_action("SELF_STUDY QUESTION PARK q1")
            .unwrap();
        assert!(!parked.text.contains("SYNTHETIC_ACCOUNT_"));
        measurements.push(measurement(owner, "park-receipt", &parked));
        let quiet = reader.prepare_action("SELF_STUDY MAP").unwrap();
        assert!(!quiet.text.contains("SYNTHETIC_ACCOUNT_"));
        assert!(!quiet.text.contains(QUESTION));
        measurements.push(measurement(owner, "map-after-parking", &quiet));
        let review = reader
            .prepare_action("SELF_STUDY QUESTION REVIEW q1")
            .unwrap();
        assert!(review.text.contains(NOTE));
        measurements.push(measurement(
            owner,
            "explicit-parked-inquiry-review",
            &review,
        ));
        let after: Value = serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
        assert!(after["questions"]["active"].is_null());
        assert_eq!(
            before["questions"]["entries"]["q1"]["notebook"],
            after["questions"]["entries"]["q1"]["notebook"]
        );
        assert!(!before["bookmarks"][SOURCE].is_null());
        assert_eq!(
            before["bookmarks"],
            after["reading_contexts"]["q1"]["bookmarks"]
        );
        assert_eq!(
            before["pending_navigation"],
            after["reading_contexts"]["q1"]["pending_navigation"]
        );
        assert_eq!(fs::read_to_string(source).unwrap(), source_text);
    }
    let report = json!({
        "schema":"synthetic-history-context-matrix-v1",
        "scope":"production shared reader preparation; synthetic authored history; no inference or live input claim",
        "reader_schema":astrid_source_study::SCHEMA_VERSION,
        "source_sha256":{
            "notebook.rs":digest(include_str!("../src/notebook.rs")),
            "store_navigation.rs":digest(include_str!("../src/store_navigation.rs")),
            "prompt.txt":digest(astrid_source_study::STUDY_PROMPT),
        },
        "measurements":measurements
    });
    if let Some(path) = std::env::var_os("ASTRID_HISTORY_CONTEXT_MEASUREMENTS") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
