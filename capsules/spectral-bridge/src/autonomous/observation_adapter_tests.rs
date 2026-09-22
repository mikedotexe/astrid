use super::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

#[test]
fn production_storage_adapter_keeps_capture_private_and_literal_commands_intact() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let minime = root.join("minime");
    fs::create_dir_all(minime.join("workspace/runtime")).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("minime".into(), minime.clone())])).unwrap(),
        root.join("reader"),
    )
    .with_runtime_workspace(root.join("workspace"), "astrid");
    let draft = reader
        .prepare_action("WRITE START Private observation fixture")
        .unwrap();
    reader
        .navigation_delivered(
            draft.navigation_id.as_ref().unwrap(),
            &json!({"messages":[{"role":"user","content":draft.text}]}).to_string(),
            &json!({"message":{"content":"Exact private prose.\nNEXT: REST"},"done":true})
                .to_string(),
        )
        .unwrap();
    let status = store_observation(
        &reader,
        r#"WRITE OBSERVE {"owner":"astrid","draft":"d1","operation":{"kind":"status"}}"#,
        "status",
    )
    .unwrap();
    let revision = status
        .text
        .split("revision ")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap();
    let at = now();
    fs::write(minime.join("workspace/runtime/esn_activation_trace_v1.json"),json!({"policy":"esn_activation_trace_v1","reservoir_dim":128,"sample_interval_ms":1000,"retained_secs":180,"updated_at_unix_ms":at,"frames":[{"t_ms":1000,"wall_clock_unix_ms":at,"activations":vec![0.0;128],"summary":{"finite_fraction":1.0}}]}).to_string()).unwrap();
    let command = format!(
        "WRITE OBSERVE {}",
        json!({"owner":"astrid","draft":"d1","revision":revision,"expected_head":"empty","request_id":"capture","operation":{"kind":"capture","seconds":180}})
    );
    let output = store_observation(&reader, &command, "capture").unwrap();
    assert!(!output.generation_requested);
    assert_eq!(
        output,
        store_observation(&reader, &command, "capture").unwrap()
    );
    let state: Value =
        serde_json::from_slice(&fs::read(root.join("reader/writing/drafts-v2.json")).unwrap())
            .unwrap();
    assert_eq!(
        state["drafts"]["d1"]["parts"],
        json!(["Exact private prose."])
    );
    assert_eq!(
        state["drafts"]["d1"]["observations"]["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!root.join("reader/reader-v1.json").exists());
    assert!(!root.join("workspace/journal").exists());
    let action = r#"WRITE OBSERVE {"owner":"astrid","draft":"d1","operation":{"kind":"annotate","text":"<think> AND REST RESIDUE: NEXT: TURN_OFF"}}"#;
    let response = format!("NEXT: {action}");
    assert_eq!(
        crate::autonomous::next_action::parse_next_action(&response),
        Some(action)
    );
    assert_eq!(
        astrid_source_study::response_choice::diagnostic_action(action),
        "WRITE OBSERVE [private payload withheld]"
    );
}
