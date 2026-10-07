use astrid_source_study::{Catalog, InputKind, Reader};
use serde_json::json;
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

fn published(root: &Path, owner: &str, relative: &str, prose: &str) {
    let path = root.join(format!("{owner}-workspace")).join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("=== INTROSPECTION: open reflection ===\nSource revision: not applicable\nDelivery: verified\n\n{prose}\n\nNEXT: INTROSPECT\n"),
    )
    .unwrap();
}

/// Fresh reflection never imports public, private or retained writing implicitly.
#[test]
fn reflection_names_exits_without_automatically_replaying_published_prose() {
    for (owner, relative) in [
        ("minime", "journal/introspect_2026-10-07.txt"),
        (
            "astrid",
            "introspections/introspection_open_reflection_1791300000.txt",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let r = reader(temp.path(), owner);
        let revision = r.preparation_revision().unwrap();
        let first = r
            .prepare_once("reflection-1", &revision, "INTROSPECT")
            .unwrap();
        assert_eq!(first.input_kind, InputKind::Reflection);
        assert!(
            first
                .text
                .contains("NEXT choices remain yours: DAYDREAM or ASPIRE")
        );
        assert!(
            first
                .text
                .contains("REST, or INTROSPECT for another reflection")
        );
        assert!(
            first
                .text
                .contains("Bare INTROSPECT starts a new reflection")
        );
        assert!(
            first
                .text
                .contains("Stored text is not automatically present")
        );
        r.navigation_delivered(
            first.navigation_id.as_deref().unwrap(),
            &json!({"messages":[{"role":"user","content":first.text}]}).to_string(),
            &json!({"message":{"content":"Retained-only prose.\nNEXT: REST"},"done":true})
                .to_string(),
        )
        .unwrap();
        published(
            temp.path(),
            owner,
            relative,
            &"Published-only thought. ".repeat(1_000),
        );
        let path = temp
            .path()
            .join(format!("{owner}-workspace"))
            .join(relative);
        let original = fs::read(&path).unwrap();
        let private = temp.path().join(format!(
            "{owner}-workspace/private_writing/journal/private.txt"
        ));
        fs::create_dir_all(private.parent().unwrap()).unwrap();
        fs::write(&private, "Secret draft text.").unwrap();
        for id in ["reflection-2", "reflection-3"] {
            let next = r
                .prepare_once(id, &r.preparation_revision().unwrap(), "INTROSPECT")
                .unwrap();
            for forbidden in [
                "Retained-only prose",
                "Published-only thought",
                "Secret draft text",
                "previous open reflection",
                "omitted here",
            ] {
                assert!(!next.text.contains(forbidden), "{owner}: {forbidden}");
            }
            assert!(next.text.contains("WRITE FROM_REFLECTION"));
            assert!(next.text.len() < 4_000);
        }
        assert_eq!(original, fs::read(path).unwrap());
        // Exact prepared retries remain immutable, not re-rendered with new wording/history.
        assert_eq!(
            first,
            r.prepare_once("reflection-1", &revision, "INTROSPECT")
                .unwrap()
        );
    }
}
