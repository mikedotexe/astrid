use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn reader(root: &Path, owner: &str) -> Reader {
    let source = root.join("sources");
    fs::create_dir_all(&source).unwrap();
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), source)])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join(format!("{owner}-workspace")), owner)
}
fn prepare(reader: &Reader, id: &str, action: &str) -> anyhow::Result<StudyOutput> {
    reader.prepare_once(id, &reader.preparation_revision()?, action)
}
fn deliver(
    reader: &Reader,
    output: &StudyOutput,
    prose: &str,
) -> astrid_source_study::DeliveryReceipt {
    reader
        .navigation_delivered(
            output.navigation_id.as_deref().unwrap(),
            &json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
            &json!({"message":{"content":prose},"done":true}).to_string(),
        )
        .unwrap()
}

#[test]
fn fresh_reflection_and_explicit_private_continuation_are_distinct_for_both_owners() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let r = reader(temp.path(), owner);
        prepare(
            &r,
            "question",
            "SELF_STUDY QUESTION NEW Original unresolved question?",
        )
        .unwrap();
        let old = prepare(&r, "old-draft", "WRITE START unrelated work").unwrap();
        deliver(&r, &old, "Unrelated private prose.\nNEXT: REST");
        let reflection = prepare(&r, "reflection", "INTROSPECT").unwrap();
        assert!(
            reflection
                .text
                .contains("Stored text is not automatically present")
        );
        assert!(!reflection.text.contains("Unrelated private prose"));
        assert!(!reflection.text.contains("Original unresolved question"));
        let id = reflection.navigation_id.as_ref().unwrap();
        assert!(
            reflection
                .text
                .contains(&format!("NEXT: WRITE FROM_REFLECTION {id}"))
        );
        let prose = "A precise λ passage.\n\n```text\nNEXT: REST\n```";
        let receipt = deliver(
            &r,
            &reflection,
            &format!("{prose}\nNEXT: WRITE FROM_REFLECTION {id}"),
        );
        let original = fs::read(&receipt.artifact_path).unwrap();
        let fresh = prepare(&r, "fresh", "INTROSPECT").unwrap();
        assert!(!fresh.text.contains(prose));
        let action = format!("WRITE FROM_REFLECTION {id}");
        let before = r.preparation_revision().unwrap();
        let private = r.prepare_once("carry", &before, &action).unwrap();
        assert_eq!(private.input_kind, InputKind::PrivateWriting);
        assert!(private.text.contains(prose));
        assert!(!private.text.contains("Unrelated private prose"));
        assert_eq!(private, r.prepare_once("carry", &before, &action).unwrap());
        assert!(
            r.prepare_once("carry", &before, "WRITE START different")
                .is_err()
        );
        deliver(&r, &private, "A new development.\nNEXT: WRITE PARK");
        let park = prepare(&r, "park", "WRITE PARK").unwrap();
        deliver(&r, &park, "NEXT: REST");
        prepare(&r, "unrelated", "INTROSPECT").unwrap();
        let returned = prepare(&r, "return", "WRITE RESUME d2").unwrap();
        assert!(returned.text.contains(prose));
        assert!(returned.text.contains("A new development."));
        let revised = prepare(&r, "revise", "WRITE REVISE qualify the explanation").unwrap();
        deliver(
            &r,
            &revised,
            "I retain the interest but qualify the explanation.\nNEXT: REST",
        );
        assert_eq!(original, fs::read(&receipt.artifact_path).unwrap());
        let state: Value = serde_json::from_slice(
            &fs::read(temp.path().join(owner).join("writing/drafts-v2.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            state["drafts"]["d1"]["parts"][0],
            "Unrelated private prose."
        );
        assert_eq!(state["drafts"].as_object().unwrap().len(), 2);
        let reader_state: Value = serde_json::from_slice(
            &fs::read(temp.path().join(owner).join("reader-v1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(reader_state["questions"]["active"], "q1");
    }
}

#[test]
fn exact_ranges_owner_scope_and_corrupt_deliveries_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "minime");
    let reflection = prepare(&r, "reflect", "INTROSPECT").unwrap();
    let id = reflection.navigation_id.as_ref().unwrap();
    let action = format!("WRITE FROM_REFLECTION {id}");
    assert!(prepare(&r, "undelivered", &action).is_err());
    let receipt = deliver(&r, &reflection, "λ chosen passage\nNEXT: REST");
    let before = r.preparation_revision().unwrap();
    for (n, bad) in [
        format!("{action} 1 5"),
        format!("{action} 0 0"),
        format!("{action} 0 999"),
        "WRITE FROM_REFLECTION ../escape".into(),
        format!("{action} extra"),
    ]
    .iter()
    .enumerate()
    {
        assert!(prepare(&r, &format!("bad-{n}"), bad).is_err());
        assert_eq!(before, r.preparation_revision().unwrap());
    }
    assert!(prepare(&reader(temp.path(), "astrid"), "foreign", &action).is_err());
    let selected = prepare(&r, "selected", &format!("{action} 0 2")).unwrap();
    assert!(selected.text.contains("\nλ\nEnd of draft."));
    assert!(!selected.text.contains("chosen passage"));
    let original = fs::read(&receipt.artifact_path).unwrap();
    fs::write(&receipt.artifact_path, b"corrupt").unwrap();
    let before = r.preparation_revision().unwrap();
    assert!(prepare(&r, "tampered", &action).is_err());
    assert_eq!(before, r.preparation_revision().unwrap());
    fs::write(&receipt.artifact_path, &original).unwrap();
    fs::write(
        receipt
            .artifact_path
            .parent()
            .unwrap()
            .join("conflict.json"),
        b"{}",
    )
    .unwrap();
    assert!(prepare(&r, "conflict", &action).is_err());
}

#[test]
fn source_study_cannot_be_mislabeled_as_reflection_and_overflow_never_truncates() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "astrid");
    let map = prepare(&r, "map", "SELF_STUDY MAP").unwrap();
    deliver(&r, &map, "Source interpretation.\nNEXT: REST");
    assert!(
        prepare(
            &r,
            "not-reflection",
            &format!("WRITE FROM_REFLECTION {}", map.navigation_id.unwrap())
        )
        .is_err()
    );
    let reflection = prepare(&r, "reflect", "INTROSPECT").unwrap();
    let receipt = deliver(&r, &reflection, &"Large preserved passage. ".repeat(3000));
    let before = r.preparation_revision().unwrap();
    assert!(
        prepare(
            &r,
            "too-large",
            &format!(
                "WRITE FROM_REFLECTION {}",
                reflection.navigation_id.unwrap()
            )
        )
        .is_err()
    );
    assert_eq!(before, r.preparation_revision().unwrap());
    assert!(receipt.artifact_path.exists());
    assert!(!temp.path().join("astrid/writing/drafts-v2.json").exists());
}

#[test]
fn concurrent_identical_imports_create_one_draft() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "minime");
    let reflection = prepare(&r, "reflect", "INTROSPECT").unwrap();
    deliver(&r, &reflection, "One exact thought.\nNEXT: REST");
    let action = format!(
        "WRITE FROM_REFLECTION {}",
        reflection.navigation_id.unwrap()
    );
    let revision = r.preparation_revision().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let root = temp.path().to_owned();
            let action = action.clone();
            let revision = revision.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let r = reader(&root, "minime");
                barrier.wait();
                r.prepare_once("same-operation", &revision, &action)
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let outputs = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outputs[0], outputs[1]);
    let state: Value = serde_json::from_slice(
        &fs::read(temp.path().join("minime/writing/drafts-v2.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(state["drafts"].as_object().unwrap().len(), 1);
}
