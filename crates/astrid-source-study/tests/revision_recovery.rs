use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";

fn reader(root: &Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.join("astrid"))])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

fn prepare(reader: &Reader, id: &str, command: &str) -> StudyOutput {
    reader
        .prepare_once(id, &reader.preparation_revision().unwrap(), command)
        .unwrap()
}

fn delivered(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"system","content":output.system_prompt},
        {"role":"user","content":output.text}]})
    .to_string();
    let response = json!({"message":{"content":text},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

fn state(root: &Path, owner: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join("reader-v1.json")).unwrap()).unwrap()
}

#[test]
fn deployed_change_recovers_by_explicit_selection_for_both_owners() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let source = root.join(SOURCE);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(
            &source,
            format!(
                "pub fn prior_mechanism() {{}}\n{}",
                "// old source\n".repeat(1500)
            ),
        )
        .unwrap();
        let reader = reader(root, owner);
        prepare(&reader, "question", "SELF_STUDY QUESTION NEW What changed?");
        let first = prepare(&reader, "first", OPEN);
        delivered(
            &reader,
            &first,
            "STUDY_NOTE: Original authored explanation.\nNEXT: SELF_STUDY CONTINUE",
        );
        let before = state(root, owner);
        let old = &first.page.as_ref().unwrap().revision.sha256;
        fs::write(&source, "pub fn replacement() {}\n// new, shorter source\n").unwrap();

        for (i, action) in [
            "SELF_STUDY",
            "SELF_STUDY CONTINUE",
            "SELF_STUDY RESUME astrid/crates/example/src/lib.rs",
        ]
        .iter()
        .enumerate()
        {
            let id = format!("recovery-{i}");
            let recovery = prepare(&reader, &id, action);
            assert_eq!(recovery.input_kind, InputKind::RevisionRecovery);
            assert!(recovery.page.is_none() && recovery.session_pages.is_empty());
            assert!(recovery.question_id.is_none());
            assert!(recovery.text.contains(old) && recovery.text.contains(OPEN));
            assert!(recovery.text.contains("OLD revision only"));
            assert!(!recovery.text.contains("Original authored explanation"));
            assert!(!recovery.text.contains("<line>"));
            assert!(
                recovery.text.len() + recovery.system_prompt.len() <= recovery.input_budget_bytes
            );
            assert_eq!(recovery, prepare(&reader, &id, action));
            assert!(
                reader
                    .prepare_once(&id, &reader.preparation_revision().unwrap(), OPEN)
                    .is_err()
            );
            delivered(
                &reader,
                &recovery,
                "STUDY_NOTE: Not authorized by a recovery response.\nSTUDY_QUESTION: Not a replacement question.\nNEXT: REST",
            );
            let after = state(root, owner);
            for key in [
                "bookmarks",
                "current",
                "progress",
                "notebook",
                "questions",
                "last_input",
            ] {
                assert_eq!(before[key], after[key], "recovery changed {key}");
            }
        }
        let selected = prepare(&reader, "explicit-open", OPEN);
        let page = selected.page.as_ref().unwrap();
        assert_ne!(&page.revision.sha256, old);
        assert_eq!(page.start.byte, 0);
        assert_eq!(before["bookmarks"], state(root, owner)["bookmarks"]);
        delivered(
            &reader,
            &selected,
            "Newly selected source.\nNEXT: SELF_STUDY CONTINUE",
        );
        assert_eq!(
            state(root, owner)["bookmarks"][SOURCE]["revision"]["sha256"],
            page.revision.sha256
        );
        assert_eq!(
            prepare(&reader, "new-continue", "SELF_STUDY CONTINUE").input_kind,
            InputKind::EndOfFile
        );
    }
}

#[test]
fn pending_bytes_and_old_state_migration_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = root.join(SOURCE);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "// old\n".repeat(1200)).unwrap();
    let initial = reader(root, "minime");
    let first = prepare(&initial, "old", OPEN);
    let path = root.join("minime/reader-v1.json");
    let mut old = state(root, "minime");
    old["version"] = 9.into();
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    fs::write(&source, "// new\n").unwrap();
    let reopened = reader(root, "minime");
    let pending = prepare(&reopened, "pending", "SELF_STUDY CONTINUE");
    assert_eq!(first, pending);
    delivered(
        &reopened,
        &pending,
        "Synthetic old delivery.\nNEXT: SELF_STUDY CONTINUE",
    );
    assert_eq!(
        prepare(&reopened, "changed", "SELF_STUDY CONTINUE").input_kind,
        InputKind::RevisionRecovery
    );
    assert_eq!(
        state(root, "minime")["version"],
        astrid_source_study::SCHEMA_VERSION
    );
    let mut future = state(root, "minime");
    future["version"] = (astrid_source_study::SCHEMA_VERSION + 1).into();
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(reopened.prepare_action("SELF_STUDY CONTINUE").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::write(&path, b"{broken").unwrap();
    assert!(reopened.prepare_action("SELF_STUDY CONTINUE").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{broken");
}

#[test]
fn missing_source_is_not_misreported_as_revision_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = root.join(SOURCE);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "pub fn example() {}\n").unwrap();
    let reader = reader(root, "astrid");
    let output = prepare(&reader, "first", OPEN);
    delivered(&reader, &output, "NEXT: SELF_STUDY CONTINUE");
    fs::remove_file(source).unwrap();
    assert!(reader.prepare_action("SELF_STUDY CONTINUE").is_err());
}
