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

/// A bare file stem is the most natural way to name a file you have been reading, and it
/// is exactly the form `path_candidates` refuses to help with: `src/path_recovery.rs:14-21`
/// returns empty unless the request already has two or more segments whose first segment
/// is an installed repository ID. `MAP <stem>` therefore recovers with a reason line and
/// the root component menu, and no candidate at all — even when a catalog file of that
/// exact stem exists and a rooted form would have resolved in one move.
#[test]
fn bare_file_stem_map_topic_recovers_with_a_reason_but_no_candidate() {
    let (_temp, reader) = setup();
    let directory = "astrid/crates/demo/src/action_continuity/runtime";
    for topic in ["core", "core.rs", "action_continuity"] {
        let recovery = reader
            .prepare_action(&format!("SELF_STUDY MAP {topic}"))
            .unwrap();
        assert_eq!(recovery.input_kind, InputKind::Recovery);
        assert!(recovery.page.is_none());
        // The being is told *that* the topic missed, and with which word.
        assert!(
            recovery
                .text
                .contains(&format!("no catalog entries for {topic}")),
            "recovery for {topic:?} should name the topic it could not resolve"
        );
        // She is not told *which* exact entry she meant: no candidate block is emitted.
        assert!(
            !recovery.text.contains("candidates, not opened"),
            "bare stem {topic:?} currently yields zero path candidates"
        );
        assert!(
            !recovery.text.contains(directory),
            "the reachable rooted form is not offered anywhere in the {topic:?} recovery"
        );
    }
    // The target was reachable the whole time through its rooted directory form.
    let reached = reader
        .prepare_action(&format!("SELF_STUDY MAP {directory}"))
        .unwrap();
    assert_eq!(reached.input_kind, InputKind::Map);
    assert!(
        reached
            .text
            .contains(&format!("SELF_STUDY OPEN {TARGET} 1"))
    );
}

/// `SELF_STUDY MAP <topic>` accepts three different namespaces — component IDs, repository
/// IDs, and rooted directory prefixes — and the recovery reason cannot tell them apart. A
/// rooted path whose final segment is a *component ID* is the worst case: `Catalog::map`
/// only matches a component when the whole topic equals its ID (`src/navigation.rs:47`), and
/// `path_candidates` with `directory: true` only ever compares final segments of catalog
/// *directories* (`src/path_recovery.rs:26-51`), so the component namespace is never
/// searched. Being rooted does not help. The working command is present in the appended root
/// menu, but nothing connects it to the topic that just failed.
#[test]
fn rooted_map_topic_ending_in_a_component_id_gets_no_candidate_naming_that_component() {
    let (_temp, reader) = setup();
    let topic = "astrid/crates/demo/src/action_continuity/runtime/verification";
    let recovery = reader
        .prepare_action(&format!("SELF_STUDY MAP {topic}"))
        .unwrap();
    assert_eq!(recovery.input_kind, InputKind::Recovery);
    assert!(recovery.page.is_none());
    assert!(
        recovery
            .text
            .contains(&format!("no catalog entries for {topic}")),
        "the recovery names the topic it could not resolve"
    );
    // Rooted, well-formed, two-or-more segments led by an installed repository ID — and
    // still no candidate block, because no catalog directory is named `verification`.
    assert!(
        !recovery.text.contains("candidates, not opened"),
        "a rooted topic whose leaf names a component yields zero path candidates"
    );
    // The working command is in the appended root menu as one undifferentiated row among
    // the components, never as an answer to the failed topic.
    assert!(
        recovery.text.contains("SELF_STUDY MAP verification"),
        "the root component menu is appended to every recovery"
    );
    let reason = recovery
        .text
        .split("This is a recovery map")
        .next()
        .expect("recovery preamble");
    assert!(
        !reason.contains("SELF_STUDY MAP verification"),
        "nothing in the reason or candidate region names the component the leaf matched"
    );
    // It resolves in one move when the topic is the bare component ID by itself.
    let chosen = reader
        .prepare_action("SELF_STUDY MAP verification")
        .unwrap();
    assert_eq!(chosen.input_kind, InputKind::Map);
    assert!(
        chosen
            .text
            .contains("Tests, configuration and deployed-source evidence"),
        "the component branch supplies its title and entry points"
    );
}
