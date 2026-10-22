use astrid_source_study::{Catalog, Reader, source_action_route};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    process::{Command, Stdio},
};

#[test]
fn routing_is_stateless_and_does_not_need_a_catalog() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_astrid-source-study"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"operation":"source_action_route","action":"OPEN astrid/Cargo.toml 1"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["route"]["canonical_action"],
        "SELF_STUDY OPEN astrid/Cargo.toml 1"
    );
}

#[test]
fn normalized_open_uses_catalog_and_verified_delivery_with_exact_retry_identity() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let path = root.join("capsules/spectral-bridge/src/action_continuity/runtime.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "pub fn entry() {}\n".repeat(1200)).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("state"),
    )
    .with_runtime_workspace(temp.path().join("workspace"), "astrid");
    let action = source_action_route(
        "OPEN astrid/capsules/spectral_bridge/src/action_continuity/runtime.rs 1",
    )
    .unwrap()
    .canonical_action;
    let revision = reader.preparation_revision().unwrap();
    let first = reader
        .prepare_once("chosen-open", &revision, &action)
        .unwrap();
    let retry = reader
        .prepare_once(
            "chosen-open",
            &reader.preparation_revision().unwrap(),
            &action,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&retry).unwrap()
    );
    let page = first.page.as_ref().unwrap();
    assert!(page.source.contains("spectral-bridge"));
    assert!(first.text.contains("spectral_bridge"));
    assert_eq!(
        reader
            .prepare_action("SELF_STUDY CONTINUE")
            .unwrap()
            .page
            .unwrap()
            .id,
        page.id
    );
    let request = json!({"messages":[{"role":"system","content":first.system_prompt},{"role":"user","content":first.text}]}).to_string();
    let response =
        json!({"message":{"content":"A source account.\nNEXT: SELF_STUDY CONTINUE"},"done":true})
            .to_string();
    reader.delivered(&page.id, &request, &response).unwrap();
    let next = reader
        .prepare_action("SELF_STUDY CONTINUE")
        .unwrap()
        .page
        .unwrap();
    assert_eq!(next.start, page.end);
}

#[test]
fn invalid_open_never_supplies_another_file_or_authorizes_an_action() {
    for action in [
        "OPEN",
        "OPEN astrid/src/lib.rs 0",
        "OPEN ../private.txt 1",
        "OPEN https://example.test/code.rs",
        "OPEN astrid/missing.rs 1 | REST",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("astrid");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn candidate() {}\n").unwrap();
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
            temp.path().join("state"),
        );
        let route = source_action_route(action).unwrap();
        let result = reader.prepare_action(&route.canonical_action).unwrap();
        assert!(result.page.is_none(), "{action}");
        assert!(
            result
                .text
                .contains("No requested source bytes were delivered"),
            "{}",
            result.text
        );
    }
}
