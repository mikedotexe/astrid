use astrid_source_study::{Catalog, Command, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";

fn reader(root: &Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.join("astrid"))])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace").join(owner), owner)
}
fn fixture(owner: &str) -> (tempfile::TempDir, Reader) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SOURCE);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "pub fn example() {}\n".repeat(100)).unwrap();
    let reader = reader(root.path(), owner);
    (root, reader)
}
fn state(root: &Path, owner: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join("reader-v1.json")).unwrap()).unwrap()
}
fn accept(reader: &Reader, out: &StudyOutput, text: &str) {
    let wire = json!({"model":"synthetic","messages":[
        {"role":"system","content":out.system_prompt},{"role":"user","content":out.text}
    ]})
    .to_string();
    let response = json!({"message":{"content":text},"done":true,"done_reason":"stop"}).to_string();
    if let Some(page) = &out.page {
        reader.delivered(&page.id, &wire, &response).unwrap();
    } else {
        reader
            .navigation_delivered(out.navigation_id.as_ref().unwrap(), &wire, &response)
            .unwrap();
    }
}
fn view(out: &StudyOutput) -> Value {
    assert_eq!(out.input_kind, InputKind::InquiryReview);
    assert!(out.question_id.is_none());
    assert!(
        out.system_prompt
            .contains("notebook directives in this response are not saved")
    );
    serde_json::Deserializer::from_str(out.text.split_once("\n\n").unwrap().1)
        .into_iter::<Value>()
        .next()
        .unwrap()
        .unwrap()
}
fn unchanged(before: &Value, after: &Value) {
    for key in [
        "questions",
        "notebook",
        "bookmarks",
        "progress",
        "last_input",
        "cursor_owner",
    ] {
        assert!(before[key] == after[key], "changed {key}");
    }
    // Inspection has an ordinary navigation offer, never a new source position.
    let positions = |state: &Value| {
        let mut contexts = state["reading_contexts"].clone();
        for context in contexts.as_object_mut().unwrap().values_mut() {
            context
                .as_object_mut()
                .unwrap()
                .remove("pending_navigation");
        }
        contexts
    };
    assert!(
        positions(before) == positions(after),
        "changed source cursors"
    );
}

#[test]
fn both_owners_inspect_parked_accounts_without_reselection_revision_or_source_delivery() {
    for owner in ["astrid", "minime"] {
        let (root, reader) = fixture(owner);
        reader
            .prepare_action("SELF_STUDY QUESTION NEW An unresolved question?")
            .unwrap();
        let prose = format!(
            "Unresolved contrary observation.\n{}\n```text\nNEXT: REST\n```",
            "非線形な言葉 ".repeat(900)
        );
        let page = reader.prepare_action(OPEN).unwrap();
        let revision = page.page.as_ref().unwrap().revision.sha256.clone();
        accept(
            &reader,
            &page,
            &format!("{prose}\nNEXT: SELF_STUDY CONTINUE"),
        );
        reader
            .prepare_action("SELF_STUDY QUESTION PARK q1")
            .unwrap();
        reader
            .prepare_action("SELF_STUDY QUESTION NEW Unrelated question?")
            .unwrap();
        let pending = reader.prepare_action(OPEN).unwrap();
        let before = state(root.path(), owner);
        let listing = reader
            .prepare_action("SELF_STUDY QUESTION ACCOUNTS q1")
            .unwrap();
        let listed = view(&listing);
        assert_eq!(listed["newest_first"].as_array().unwrap().len(), 1);
        assert!(!listing.text.contains("Unresolved contrary observation."));
        let command = listed["newest_first"][0]["inspect_command"]
            .as_str()
            .unwrap();
        let first = reader.prepare_action(command).unwrap();
        let first_view = view(&first);
        assert!(first.page.is_none());
        assert!(
            first_view["historical_origin"]
                .as_str()
                .unwrap()
                .contains(&revision)
        );
        assert_eq!(first_view["complete_when_recorded"], true);
        let mut restored = String::new();
        for index in 1..=first_view["pages"].as_u64().unwrap() {
            let out = reader
                .prepare_action(&format!("{command} --page {index}"))
                .unwrap();
            let value = view(&out);
            let text = value["text"].as_str().unwrap();
            assert!(text.len() <= 4_000);
            assert_eq!(
                value["start_byte"].as_u64().unwrap(),
                u64::try_from(restored.len()).unwrap()
            );
            restored.push_str(text);
        }
        assert_eq!(restored, prose);
        let inspection = reader.prepare_action(command).unwrap();
        accept(
            &reader,
            &inspection,
            "STUDY_NOTE: Do not save this.\nSTUDY_QUESTION: Do not replace it.\nNEXT: REST",
        );
        unchanged(&before, &state(root.path(), owner));
        assert_eq!(
            reader.prepare_action("SELF_STUDY CONTINUE").unwrap().page,
            pending.page
        );
        reader
            .prepare_action("SELF_STUDY QUESTION PARK q2")
            .unwrap();
        for command in ["INTROSPECT", "SELF_STUDY MAP"] {
            let out = reader.prepare_action(command).unwrap();
            assert!(!out.text.contains("Unresolved contrary observation."));
        }
    }
}

#[test]
fn exact_id_failure_and_unthreaded_inspection_never_borrow_another_inquiry() {
    let (root, reader) = fixture("minime");
    let empty = view(
        &reader
            .prepare_action("SELF_STUDY QUESTION ACCOUNTS NOTEBOOK")
            .unwrap(),
    );
    assert!(empty["newest_first"].as_array().unwrap().is_empty());
    let page = reader.prepare_action(OPEN).unwrap();
    accept(&reader, &page, "Unthreaded account.\nNEXT: REST");
    reader
        .prepare_action("SELF_STUDY QUESTION NEW Numbered question?")
        .unwrap();
    let page = reader.prepare_action(OPEN).unwrap();
    accept(&reader, &page, "Numbered account.\nNEXT: REST");
    let before = state(root.path(), "minime");
    let list = view(
        &reader
            .prepare_action("SELF_STUDY QUESTION ACCOUNTS NOTEBOOK")
            .unwrap(),
    );
    let command = list["newest_first"][0]["inspect_command"].as_str().unwrap();
    let account = reader.prepare_action(command).unwrap();
    assert!(account.text.contains("Unthreaded account."));
    assert!(!account.text.contains("Numbered account."));
    let hash = list["newest_first"][0]["account_sha256"].as_str().unwrap();
    for command in [
        format!("SELF_STUDY QUESTION ACCOUNT q1 {hash}"),
        "SELF_STUDY QUESTION ACCOUNTS q999".into(),
        format!("{command} --page 999"),
    ] {
        assert!(reader.prepare_action(&command).is_err(), "{command}");
    }
    unchanged(&before, &state(root.path(), "minime"));
    let other = super_reader(root.path());
    assert!(
        other
            .prepare_action("SELF_STUDY QUESTION ACCOUNTS q1")
            .is_err()
    );
}
fn super_reader(root: &Path) -> Reader {
    reader(root, "astrid")
}

#[test]
fn retained_window_excerpt_flags_restart_and_idempotent_preparation_are_honest() {
    let (root, current) = fixture("astrid");
    current
        .prepare_action("SELF_STUDY QUESTION NEW Why?")
        .unwrap();
    let mut old_command = String::new();
    for index in 0..6 {
        let input = current.prepare_action(OPEN).unwrap();
        accept(
            &current,
            &input,
            &format!("ACCOUNT_{index}\n{}\nNEXT: REST", "x".repeat(65_000)),
        );
        if index == 0 {
            old_command = view(
                &current
                    .prepare_action("SELF_STUDY QUESTION ACCOUNTS q1")
                    .unwrap(),
            )["newest_first"][0]["inspect_command"]
                .as_str()
                .unwrap()
                .into();
        }
    }
    current
        .prepare_action("SELF_STUDY QUESTION PARK q1")
        .unwrap();
    let reopened = reader(root.path(), "astrid");
    let list = view(
        &reopened
            .prepare_action("SELF_STUDY QUESTION ACCOUNTS q1")
            .unwrap(),
    );
    assert_eq!(list["newest_first"].as_array().unwrap().len(), 4);
    assert_eq!(list["newest_first"][0]["complete_when_recorded"], false);
    assert!(reopened.prepare_action(&old_command).is_err());
    let command = list["newest_first"][0]["inspect_command"].as_str().unwrap();
    let before = state(root.path(), "astrid");
    let revision = reopened.preparation_revision().unwrap();
    let out = reopened
        .prepare_once("inspect-account", &revision, command)
        .unwrap();
    assert_eq!(
        reopened
            .prepare_once("inspect-account", &revision, command)
            .unwrap(),
        out
    );
    assert!(
        reopened
            .prepare_once("inspect-account", &revision, "SELF_STUDY QUESTION q1")
            .is_err()
    );
    unchanged(&before, &state(root.path(), "astrid"));
    let value = view(&out);
    assert_eq!(value["complete_when_recorded"], false);
    assert_eq!(value["retained_bytes"], 64_000);
}

#[test]
fn account_commands_are_bounded_and_roundtrip_without_an_owner_or_path_override() {
    let hash = "a".repeat(64);
    for command in [
        "SELF_STUDY QUESTION ACCOUNTS q1".into(),
        "SELF_STUDY QUESTION ACCOUNTS NOTEBOOK".into(),
        format!("SELF_STUDY QUESTION ACCOUNT q1 {hash} --page 2"),
    ] {
        let parsed = Command::parse(&command).unwrap();
        assert_eq!(
            serde_json::from_str::<Command>(&serde_json::to_string(&parsed).unwrap()).unwrap(),
            parsed
        );
    }
    for command in [
        "SELF_STUDY QUESTION ACCOUNTS".into(),
        "SELF_STUDY QUESTION ACCOUNTS minime/q1".into(),
        "SELF_STUDY QUESTION ACCOUNT q1 latest".into(),
        format!("SELF_STUDY QUESTION ACCOUNT q1 {hash} --page 0"),
        format!("SELF_STUDY QUESTION ACCOUNT q1 {hash} --page 1 REST"),
    ] {
        assert!(Command::parse(&command).is_err(), "{command}");
    }
}

#[test]
fn identical_responses_from_distinct_source_positions_have_distinct_account_ids() {
    for final_line in [1, 50] {
        let (_root, reader) = fixture("minime");
        reader
            .prepare_action("SELF_STUDY QUESTION NEW Still unresolved?")
            .unwrap();
        // Consecutive identical responses are already deduplicated by the writer.
        for (line, text) in [
            (1, "Still unresolved.\nNEXT: REST"),
            (30, "Another observation.\nNEXT: REST"),
            (final_line, "Still unresolved.\nNEXT: REST"),
        ] {
            let input = reader
                .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} {line}"))
                .unwrap();
            accept(&reader, &input, text);
        }
        let list = view(
            &reader
                .prepare_action("SELF_STUDY QUESTION ACCOUNTS q1")
                .unwrap(),
        );
        let accounts = list["newest_first"].as_array().unwrap();
        assert_eq!(accounts.len(), 3);
        assert_eq!(
            accounts[0]["response_sha256"],
            accounts[2]["response_sha256"]
        );
        assert_eq!(
            accounts[0]["account_sha256"] == accounts[2]["account_sha256"],
            final_line == 1
        );
        for index in [0, 2] {
            let command = accounts[index]["inspect_command"].as_str().unwrap();
            let inspected = view(&reader.prepare_action(command).unwrap());
            assert_eq!(inspected["text"], "Still unresolved.");
            assert_eq!(
                inspected["identical_retained_occurrences"],
                if final_line == 1 { 2 } else { 1 }
            );
        }
    }
}
