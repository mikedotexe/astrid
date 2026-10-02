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

/// 2026-10-02: a reflection turn names the same exits the study prompt names,
/// another reflection last, and offers the being's previous PUBLISHED reflection
/// as optional material. Reader-retained deliveries are still never injected.
#[test]
fn reflection_turn_names_the_exits_and_offers_the_previous_published_reflection() {
    for (owner, relative) in [
        (
            "minime",
            "journal/introspect_2026-10-02T10-00-00.000001.txt",
        ),
        (
            "astrid",
            "introspections/introspection_open_reflection_1790900000.txt",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let r = reader(temp.path(), owner);
        let first = r.prepare_action("INTROSPECT").unwrap();
        assert_eq!(first.input_kind, InputKind::Reflection);
        assert!(
            !first.text.contains("previous open reflection"),
            "{owner}: nothing published yet"
        );
        let exits = first
            .text
            .find("NEXT choices remain yours: DAYDREAM or ASPIRE")
            .expect("exits line present");
        assert!(first.text[exits..].contains("REST, or INTROSPECT for another reflection"));
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
        // A reader-retained delivery is not a published reflection: still never injected.
        r.navigation_delivered(
            first.navigation_id.as_deref().unwrap(),
            &json!({"messages":[{"role":"user","content":first.text}]}).to_string(),
            &json!({"message":{"content":"Retained-only prose.\nNEXT: REST"},"done":true})
                .to_string(),
        )
        .unwrap();
        let second = r.prepare_action("INTROSPECT").unwrap();
        assert!(!second.text.contains("Retained-only prose"));
        // Her published entry is offered back, without its NEXT line, marked optional.
        published(
            temp.path(),
            owner,
            relative,
            "The concept of presence is a strange architecture to inhabit.\n\nA second paragraph.",
        );
        let third = r.prepare_action("INTROSPECT").unwrap();
        let block = third
            .text
            .find("Your previous open reflection, retained publicly")
            .expect("previous reflection offered");
        let body = &third.text[block..];
        assert!(body.contains("optional material: continue it, answer it, or leave it aside"));
        assert!(body.contains(
            "The concept of presence is a strange architecture to inhabit.\n\nA second paragraph."
        ));
        assert!(body.contains("[end of your previous reflection]"));
        assert!(
            !body[..body.find("[end of your previous reflection]").unwrap()]
                .contains("NEXT: INTROSPECT")
        );
        assert!(
            block < third.text.find("NEXT choices remain yours").unwrap(),
            "material precedes the exits"
        );
        // A very long previous reflection is bounded with an explicit omission marker.
        published(temp.path(), owner, relative, &"λ thought. ".repeat(1_000));
        let fourth = r.prepare_action("INTROSPECT").unwrap();
        assert!(
            fourth
                .text
                .contains("of your previous reflection omitted here")
        );
        assert!(fourth.text.len() < 12_000);
        // Private drafts are never read as material.
        let private = temp.path().join(format!(
            "{owner}-workspace/private_writing/journal/private_writing_x.txt"
        ));
        fs::create_dir_all(private.parent().unwrap()).unwrap();
        fs::write(&private, "=== PRIVATE WRITING ===\n\nSecret draft text.\n").unwrap();
        assert!(
            !r.prepare_action("INTROSPECT")
                .unwrap()
                .text
                .contains("Secret draft text")
        );
    }
}
