use astrid_source_study::{Catalog, Reader};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write as _,
    process::{Command, Stdio},
};

fn reader(root: &std::path::Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.into())])).unwrap(),
        root.join("reader"),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

#[test]
fn lost_ack_replays_exact_question_or_draft_without_deduplicating_new_choices() {
    for owner in ["astrid", "minime"] {
        for action in [
            "SELF_STUDY QUESTION NEW Synthetic question?",
            "WRITE START exact private title",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let r = reader(temp.path(), owner);
            let revision = r.preparation_revision().unwrap();
            let first = r.prepare_once("event-1", &revision, action).unwrap();
            let reopened = reader(temp.path(), owner);
            let second = reopened
                .prepare_once("event-1", "old-revision-is-not-a-new-admission", action)
                .unwrap();
            assert_eq!(
                serde_json::to_value(&first).unwrap(),
                serde_json::to_value(second).unwrap()
            );
            assert!(
                reopened
                    .prepare_once("event-1", &revision, "SELF_STUDY MAP")
                    .is_err()
            );
            assert!(
                reader(
                    temp.path(),
                    if owner == "astrid" {
                        "minime"
                    } else {
                        "astrid"
                    }
                )
                .prepare_once("event-1", &revision, action)
                .is_err()
            );
            assert!(reopened.prepare_once("event-2", &revision, action).is_err());
            let new = reopened
                .prepare_once("event-2", &reopened.preparation_revision().unwrap(), action)
                .unwrap();
            assert_ne!(
                serde_json::to_value(first).unwrap(),
                serde_json::to_value(new).unwrap()
            );
        }
    }
}

#[test]
fn failed_private_preparation_does_not_commit_partial_migration_or_request() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "minime");
    let revision = r.preparation_revision().unwrap();
    assert!(
        r.prepare_once("bad", &revision, "WRITE RESUME d999")
            .is_err()
    );
    assert_eq!(revision, r.preparation_revision().unwrap());
    assert!(!temp.path().join("reader/preparation-operations").exists());
}

#[test]
fn concurrent_helper_processes_commit_one_exact_preparation() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "astrid");
    let request = json!({"astrid_root":temp.path(), "minime_root":temp.path(),
        "runtime_workspace":temp.path().join("workspace"), "being":"astrid",
        "state_directory":temp.path().join("reader"), "operation":"prepare_once",
        "request_id":"durable-action-event", "expected_revision":r.preparation_revision().unwrap(),
        "action":"SELF_STUDY QUESTION NEW Concurrent synthetic question?"});
    let children: Vec<_> = (0..4)
        .map(|_| {
            let mut child = Command::new(env!("CARGO_BIN_EXE_astrid-source-study"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(request.to_string().as_bytes())
                .unwrap();
            child
        })
        .collect();
    let values: Vec<Value> = children
        .into_iter()
        .map(|c| {
            let result = c.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            serde_json::from_slice(&result.stdout).unwrap()
        })
        .collect();
    assert!(values.iter().all(|v| v == &values[0]));
    let saved: Value =
        serde_json::from_slice(&std::fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(saved["questions"]["entries"].as_object().unwrap().len(), 1);
}
