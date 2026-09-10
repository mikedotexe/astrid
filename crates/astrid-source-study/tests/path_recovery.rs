use astrid_source_study::{Catalog, InputKind, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fs};

const CURRENT: &str = "astrid/crates/demo/src/current.rs";
const TARGET: &str = "astrid/crates/demo/src/action_continuity/runtime/core.rs";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for name in [
        "crates/demo/src/current.rs",
        "crates/demo/src/action_continuity/runtime/core.rs",
        "crates/other/src/runtime/core.rs",
        "crates/third/src/runtime/core.rs",
        "crates/fourth/src/runtime/core.rs",
        "workspace/runtime/core.rs",
        "crates/demo/src/runtime/secrets.json",
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "line\n".repeat(3000)).unwrap();
    }
    let other = temp.path().join("minime/minime_autonomy/runtime/core.rs");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, "other").unwrap();
    let catalog = Catalog::new(BTreeMap::from([
        ("astrid".into(), root),
        ("minime".into(), temp.path().join("minime")),
    ]))
    .unwrap();
    let reader = Reader::new(catalog, temp.path().join("reader"));
    (temp, reader)
}

#[test]
fn wrong_turns_offer_choices_without_losing_pending_source_or_recording_delivery() {
    let (temp, reader) = setup();
    let first = reader
        .prepare_action(&format!("SELF_STUDY OPEN {CURRENT} 1"))
        .unwrap();
    let page = first.page.as_ref().unwrap();
    reader
        .delivered(
            &page.id,
            &json!({"messages":[{"role":"user","content":first.text}]}).to_string(),
            r#"{"message":{"content":"I can keep exploring."},"done":true}"#,
        )
        .unwrap();
    let pending = reader
        .prepare_action("SELF_STUDY CONTINUE")
        .unwrap()
        .page
        .unwrap();
    for action in [
        "SELF_STUDY OPEN astrid/crates/demo/src/runtime/core.rs 1",
        "SELF_STUDY RESUME astrid/crates/demo/src/runtime/core.rs",
        "SELF_STUDY SESSION OPEN astrid/crates/demo/src/runtime/core.rs 1 | OPEN astrid/crates/demo/src/current.rs 1",
    ] {
        let recovery = reader.prepare_action(action).unwrap();
        assert_eq!(recovery.input_kind, InputKind::Recovery);
        assert!(recovery.page.is_none());
        assert!(
            recovery
                .text
                .contains(&format!("SELF_STUDY OPEN {TARGET} 1"))
        );
        assert!(recovery.text.contains("candidates, not opened"));
        assert!(!recovery.text.contains("OPEN minime/"));
        assert!(!recovery.text.contains("OPEN astrid/workspace/"));
        let candidates = recovery
            .text
            .split("Exact catalog spelling")
            .nth(1)
            .unwrap()
            .split("You can choose")
            .next()
            .unwrap();
        assert_eq!(candidates.matches("SELF_STUDY OPEN").count(), 3);
        assert!(candidates.find(TARGET).unwrap() < candidates.find("crates/other").unwrap());
        assert_eq!(
            reader
                .prepare_action("SELF_STUDY CONTINUE")
                .unwrap()
                .page
                .unwrap(),
            pending
        );
    }
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(state["bookmarks"].as_object().unwrap().len(), 1);
    // Accepting a suggestion is optional; the Being can choose another exact file.
    let elsewhere = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/other/src/runtime/core.rs 1")
        .unwrap();
    assert_eq!(
        elsewhere.page.unwrap().source,
        "astrid/crates/other/src/runtime/core.rs"
    );
}

#[test]
fn empty_directory_map_offers_existing_parent_routes_only() {
    let (_temp, reader) = setup();
    let recovery = reader
        .prepare_action("SELF_STUDY MAP astrid/crates/demo/src/runtime/")
        .unwrap();
    assert_eq!(recovery.input_kind, InputKind::Recovery);
    let target = "astrid/crates/demo/src/action_continuity/runtime";
    assert!(recovery.text.contains(&format!("SELF_STUDY MAP {target}")));
    let chosen = reader
        .prepare_action(&format!("SELF_STUDY MAP {target}"))
        .unwrap();
    assert_eq!(chosen.input_kind, InputKind::Map);
    assert!(chosen.text.contains(&format!("SELF_STUDY OPEN {TARGET} 1")));
}

#[test]
fn unavailable_private_traversal_and_unrelated_targets_get_no_path_candidates() {
    let (_temp, reader) = setup();
    for action in [
        "SELF_STUDY OPEN astrid/crates/demo/src/runtime/secrets.json 1",
        "SELF_STUDY OPEN astrid/../runtime/core.rs 1",
        "SELF_STUDY OPEN astrid/crates/demo/src/runtime/absent.rs 1",
    ] {
        let output = reader.prepare_action(action).unwrap();
        assert!(output.page.is_none());
        assert!(!output.text.contains("candidates, not opened"));
    }
}
