use astrid_source_study::{Catalog, Command, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const BRIDGE: &str = "astrid/capsules/spectral-bridge";
const SPINE: &str = "astrid/crates/demo/src/signal_spine.rs";
const EVENT: &str = "astrid.v1.capsules_loaded";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for path in [
        "capsules/spectral-bridge/src/lib.rs",
        "crates/demo/src/signal_spine.rs",
        "crates/demo/src/event_consumer.rs",
        "crates/alpha/src/common.rs",
        "crates/beta/src/common.rs",
        "crates/gamma/src/common.rs",
        "crates/delta/src/common.rs",
        "workspace/journal/signal_spine.rs",
        "crates/demo/src/secrets.json",
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "pub fn example() {}\n".repeat(500)).unwrap();
    }
    fs::write(
        root.join("crates/demo/src/event_consumer.rs"),
        format!("if topic == \"{EVENT}\" {{ refresh(); }}\n"),
    )
    .unwrap();
    let minime = temp.path().join("minime");
    fs::create_dir_all(minime.join("minime_autonomy")).unwrap();
    fs::write(minime.join("minime_autonomy/common.rs"), "example\n").unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([
            ("astrid".into(), root),
            ("minime".into(), minime),
        ]))
        .unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn accept(reader: &Reader, output: &StudyOutput, content: &str) {
    let request = json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string();
    let response =
        json!({"message":{"content":content},"done":true,"done_reason":"stop"}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

fn choices(output: &StudyOutput) -> &str {
    output
        .text
        .split("Exact catalog spelling and nearby paths (candidates, not opened):\n")
        .nth(1)
        .unwrap()
        .split("\nYou can choose")
        .next()
        .unwrap()
}

#[test]
fn bare_component_typo_offers_an_exact_map_without_opening_or_advancing_source() {
    let (temp, reader) = setup();
    let first = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SPINE} 1"))
        .unwrap();
    accept(&reader, &first, "I can inspect this source.");
    let before: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    for topic in ["spectral_bridge", "spectral-bridge", "spectral_bridge/"] {
        let output = reader
            .prepare_action(&format!("SELF_STUDY MAP {topic}"))
            .unwrap();
        assert_eq!(output.input_kind, InputKind::Recovery);
        assert!(output.page.is_none());
        assert_eq!(choices(&output), format!("SELF_STUDY MAP {BRIDGE}"));
    }
    let after: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(before["bookmarks"], after["bookmarks"]);
    let chosen = reader
        .prepare_action(&format!("SELF_STUDY MAP {BRIDGE}"))
        .unwrap();
    assert_eq!(chosen.input_kind, InputKind::Map);
}

#[test]
fn bare_file_stem_and_ambiguous_basename_offer_bounded_stable_choices() {
    let (_temp, reader) = setup();
    for spelling in ["signal_spine", "signal-spine", "signal-spine.rs"] {
        let output = reader
            .prepare_action(&format!("SELF_STUDY OPEN {spelling} 1"))
            .unwrap();
        assert_eq!(output.input_kind, InputKind::Recovery);
        assert_eq!(choices(&output), format!("SELF_STUDY OPEN {SPINE} 1"));
    }
    let first = reader
        .prepare_action("SELF_STUDY OPEN common.rs 1")
        .unwrap();
    let repeat = reader
        .prepare_action("SELF_STUDY OPEN common.rs 1")
        .unwrap();
    assert_eq!(choices(&first), choices(&repeat));
    assert_eq!(choices(&first).lines().count(), 3);
    for command in choices(&first).lines() {
        assert!(matches!(
            Command::parse(command).unwrap(),
            Command::Open { .. }
        ));
    }
    let qualified = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/absent/common.rs 1")
        .unwrap();
    assert!(!choices(&qualified).contains("minime/"));
}

#[test]
fn malformed_private_and_unknown_repository_requests_do_not_expand_into_candidates() {
    let (_temp, reader) = setup();
    for reference in [
        "astrid/../signal_spine.rs",
        "unknown/src/signal_spine.rs",
        "astrid//signal_spine.rs",
        "spectral_bridge//",
        "<spectral_bridge>",
        "spectral bridge",
        "astrid/workspace/journal/signal_spine.rs",
        "workspace/signal_spine.rs",
        "private/signal_spine.rs",
        "astrid/crates/demo/src/secrets.json",
    ] {
        let output = reader
            .prepare_action(&format!("SELF_STUDY MAP {reference}"))
            .unwrap();
        assert!(output.page.is_none());
        assert!(
            !output.text.contains("candidates, not opened"),
            "{reference}"
        );
    }
}

#[test]
fn quoted_events_and_identifiers_offer_distinct_safe_lookups_with_actual_literal_hits() {
    for quote in ['`', '\'', '"'] {
        let (_temp, reader) = setup();
        let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
        accept(
            &reader,
            &map,
            &format!(
                "STUDY_QUESTION: Who handles {quote}{EVENT}{quote} through `EventSubscriber`?"
            ),
        );
        let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
        assert!(next.text.contains(&format!("SELF_STUDY FIND {EVENT}")));
        assert!(next.text.contains("SELF_STUDY RELATE EventSubscriber"));
        assert!(!next.text.contains(&format!("SELF_STUDY RELATE {EVENT}")));
        assert!(next.text.contains("existence and meaning unverified"));
        assert_eq!(
            Command::parse(&format!("SELF_STUDY FIND {EVENT}")).unwrap(),
            Command::Find {
                query: EVENT.into(),
                page: 1
            }
        );
        let result = reader
            .prepare_action(&format!("SELF_STUDY FIND {EVENT}"))
            .unwrap();
        assert!(
            result
                .text
                .contains("Implementation text: 1 matching lines")
        );
        assert!(
            result
                .text
                .contains("SELF_STUDY OPEN astrid/crates/demo/src/event_consumer.rs 1")
        );
        assert!(
            result
                .text
                .contains("This turn already supplies lexical results")
        );
    }
}

#[test]
fn an_unquoted_event_lookup_reaches_source_without_guessing_an_unanchored_filename() {
    let (_temp, reader) = setup();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        &format!("STUDY_QUESTION: Who receives {EVENT} in `event_consumer.rs`?"),
    );
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(next.text.contains(&format!("SELF_STUDY FIND {EVENT}")));
    assert!(
        next.text
            .contains("Source reference `event_consumer.rs` has no verified contextual match")
    );
    assert!(next.page.is_none());
    let found = reader
        .prepare_action(&format!("SELF_STUDY FIND {EVENT}"))
        .unwrap();
    assert!(
        found
            .text
            .contains("SELF_STUDY OPEN astrid/crates/demo/src/event_consumer.rs 1")
    );
}

#[test]
fn quoted_malformed_commands_never_become_lookup_commands() {
    let (_temp, reader) = setup();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        "STUDY_QUESTION: Compare `MAP | evil` and `--page 100` and `tokio::task::spawn` and `../private`?",
    );
    let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(!next.text.contains("Optional lexical lookup"));
    assert!(!next.text.contains("Open a source named in this question"));
}

#[test]
fn note_examples_share_next_eligibility_and_do_not_overwrite_the_chosen_question() {
    let (temp, reader) = setup();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        "STUDY_NOTE: Chosen note.\nSTUDY_QUESTION: Chosen question.",
    );
    for response in [
        "    STUDY_NOTE: Indented example.\n\tSTUDY_QUESTION: Tab example.",
        "> STUDY_NOTE: Quoted example.\n\"STUDY_QUESTION: Quoted example.\"",
        "````text\n```\nSTUDY_NOTE: Still fenced.\nSTUDY_QUESTION: Still fenced.\n````",
        "~~~text\n```\nSTUDY_NOTE: Still tilde fenced.\nSTUDY_QUESTION: Still tilde fenced.",
        "<thinking>\nSTUDY_NOTE: Internal block.\nSTUDY_QUESTION: Internal block.\n</thinking>",
        "<think>Hidden.</think>STUDY_NOTE: Inline metadata.\n<analysis>Hidden.</analysis>STUDY_QUESTION: Inline metadata.",
    ] {
        let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
        accept(&reader, &map, response);
        let state: Value =
            serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
                .unwrap();
        assert_eq!(state["notebook"]["note"]["text"], "Chosen note.");
        assert_eq!(state["notebook"]["question"]["text"], "Chosen question.");
    }
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(
        &reader,
        &map,
        "STUDY_NOTE: Revised by choice.\nSTUDY_QUESTION: -",
    );
    let state: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(state["notebook"]["note"]["text"], "Revised by choice.");
    assert!(state["notebook"]["question"].is_null());
}
