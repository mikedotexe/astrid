use super::*;

fn client(root: &std::path::Path) -> ReaderClient {
    ReaderClient {
        python: std::env::var_os("ASTRID_AFTERIMAGE_PYTHON")
            .map_or_else(|| "python3".into(), PathBuf::from),
        script: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../minime/minime_autonomy/afterimages.py"),
        workspace: root.join("astrid"),
        archive_workspace: root.join("minime"),
    }
}

#[test]
fn transition_afterimage_shared_fixture_and_pending_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let reader = client(root.path());
    let source = std::fs::read(
        reader
            .script
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/transition_afterimage_v1.json"),
    )
    .unwrap();
    let fixture: Value = serde_json::from_slice(&source).unwrap();
    let archive = reader
        .archive_workspace
        .join("transition_afterimages/2026-09-07");
    std::fs::create_dir_all(&archive).unwrap();
    std::fs::write(
        archive.join(format!("{}.json", fixture["id"].as_str().unwrap())),
        source,
    )
    .unwrap();
    assert!(!reader.cues_enabled());
    assert!(!reader.workspace.exists());
    let opened = reader
        .invoke(json!({"action":format!("AFTERIMAGE_OPEN {}", fixture["id"].as_str().unwrap())}))
        .unwrap();
    let selected = reader
        .invoke(json!({"operation":"select", "artifact_id":fixture["id"]}))
        .unwrap();
    assert_eq!(opened["text"], selected["text"]);
    let input = reader.pending_input().unwrap().unwrap();
    assert_eq!(input.kind, crate::llm::ProtectedDialogueKindV1::Afterimage);
    assert_eq!(input.source_text, opened["text"].as_str().unwrap());
    assert_eq!(
        reader.pending().unwrap().unwrap()["page_fingerprint"],
        digest(&input.source_text)
    );
    let reloaded = client(root.path());
    assert_eq!(
        reloaded.pending_input().unwrap().unwrap().source_text,
        input.source_text
    );
    assert!(
        reader
            .invoke(json!({"operation":"ack", "content_id":"wrong", "receipt":{}}))
            .is_err()
    );
    assert!(reader.pending().unwrap().is_some());
}

#[test]
fn transition_afterimage_astrid_private_authorship_and_explicit_sharing() {
    let root = tempfile::tempdir().unwrap();
    let reader = client(root.path());
    let saved = reader
        .invoke(json!({"action":"AFTERIMAGE_KEEP ::  fragment AND REST\nNEXT: TURN_OFF  "}))
        .unwrap();
    assert_eq!(saved["status"], "incomplete");
    let note: Value = serde_json::from_slice(
        &std::fs::read(
            reader
                .private()
                .join("notes")
                .join(format!("{}.json", saved["note_id"].as_str().unwrap())),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(note["text"], " fragment AND REST\nNEXT: TURN_OFF  ");
    assert_eq!(note["author"], "astrid");
    assert!(
        !reader
            .archive_workspace
            .join("transition_afterimages/shared_notes")
            .exists()
    );
    reader
        .invoke(
            json!({"action":format!("AFTERIMAGE_SHARE {}", saved["note_id"].as_str().unwrap())}),
        )
        .unwrap();
    let shared: Value = serde_json::from_slice(
        &std::fs::read(
            reader
                .archive_workspace
                .join("transition_afterimages/shared_notes")
                .join(saved["id"].as_str().unwrap())
                .join(format!("{}.json", saved["note_id"].as_str().unwrap())),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(shared["text"], note["text"]);
    assert_eq!(shared["source"], note["source"]);
}
