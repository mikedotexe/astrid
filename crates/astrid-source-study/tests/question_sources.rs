use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const KERNEL: &str = "astrid/crates/astrid-kernel/src/lib.rs";
const ROUTER: &str = "astrid/crates/astrid-kernel/src/kernel_router.rs";
const QUESTION: &str = "Does the `Kernel` struct in `lib.rs` implement the blocked check?";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    for source in [
        KERNEL,
        ROUTER,
        "astrid/capsules/spectral-bridge/src/lib.rs",
        "minime/minime_autonomy/source.py",
        "minime/minime_autonomy/src/lib.rs",
        "prime-esn/src/lib.rs",
        "rascii/src/lib.rs",
    ] {
        let path = temp.path().join(source);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "pub fn unrelated_fixture() {}\n".repeat(10)).unwrap();
    }
    let catalog = Catalog::new(
        ["astrid", "minime", "prime-esn", "rascii"]
            .into_iter()
            .map(|repo| (repo.to_owned(), temp.path().join(repo)))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap();
    let reader = Reader::new(catalog, temp.path().join("reader"));
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
            .navigation_delivered(
                output.navigation_id.as_deref().unwrap(),
                &request,
                &response,
            )
            .unwrap();
    }
}

fn open(reader: &Reader, source: &str) -> StudyOutput {
    reader
        .prepare_action(&format!("SELF_STUDY OPEN {source} 1"))
        .unwrap()
}

fn choices(output: &StudyOutput) -> Vec<&str> {
    output
        .text
        .lines()
        .filter(|line| line.contains("catalog path verified; relevance unverified"))
        .collect()
}

fn saved_question(temp: &tempfile::TempDir) -> Value {
    let state: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    state["notebook"]["question"].clone()
}

#[test]
fn repeated_question_keeps_its_kernel_origin_across_unrelated_lib_files() {
    let (temp, reader) = setup();
    let first = open(&reader, ROUTER);
    accept(&reader, &first, &format!("STUDY_QUESTION: {QUESTION}"));
    let question = saved_question(&temp);
    for detour in ["prime-esn/src/lib.rs", "rascii/src/lib.rs"] {
        let output = open(&reader, detour);
        let hints = choices(&output);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].ends_with(&format!("SELF_STUDY OPEN {KERNEL} 1")));
        assert!(
            hints[0]
                .starts_with("Filename candidate beside the source where your question was saved")
        );
        accept(
            &reader,
            &output,
            &format!("This is another repository.\nSTUDY_QUESTION: {QUESTION}"),
        );
        assert_eq!(saved_question(&temp), question);
    }
    let after = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert_eq!(choices(&after).len(), 1);
    assert!(choices(&after)[0].ends_with(&format!("SELF_STUDY OPEN {KERNEL} 1")));
}

#[test]
fn explicit_paths_can_deliberately_cross_repositories() {
    let (_temp, reader) = setup();
    let first = open(&reader, ROUTER);
    accept(
        &reader,
        &first,
        "STUDY_QUESTION: Compare Astrid with `prime-esn/src/lib.rs` and `rascii/src/lib.rs`.",
    );
    let output = open(&reader, KERNEL);
    let hints = choices(&output);
    assert_eq!(hints.len(), 2);
    assert!(hints[0].starts_with("Source path written in your question"));
    assert!(hints[0].ends_with("SELF_STUDY OPEN prime-esn/src/lib.rs 1"));
    assert!(hints[1].ends_with("SELF_STUDY OPEN rascii/src/lib.rs 1"));
}

#[test]
fn unanchored_basename_does_not_pick_short_paths_from_other_repositories() {
    let (_temp, reader) = setup();
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(&reader, &map, &format!("STUDY_QUESTION: {QUESTION}"));
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(choices(&output).is_empty());
    assert!(
        output
            .text
            .contains("Source reference `lib.rs` has no verified contextual match")
    );
    assert!(output.text.contains("SELF_STUDY RELATE Kernel"));
    // Explicit access to those repositories still works; only the inference is removed.
    assert_eq!(
        open(&reader, "prime-esn/src/lib.rs").page.unwrap().source,
        "prime-esn/src/lib.rs"
    );
}

#[test]
fn symbol_path_hint_ranks_known_contexts_without_claiming_a_definition() {
    let (_temp, reader) = setup();
    for source in [ROUTER, "prime-esn/src/lib.rs", "rascii/src/lib.rs"] {
        let output = open(&reader, source);
        accept(&reader, &output, "A completed page with no saved question.");
    }
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    accept(&reader, &map, &format!("STUDY_QUESTION: {QUESTION}"));
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let hints = choices(&output);
    assert_eq!(hints.len(), 1);
    assert!(
        hints[0]
            .starts_with("Filename candidate from recent source context (lexical path hints only)")
    );
    assert!(hints[0].ends_with(&format!("SELF_STUDY OPEN {KERNEL} 1")));
}

#[test]
fn an_explicit_repository_in_question_does_not_turn_a_detour_into_evidence() {
    let (_temp, reader) = setup();
    let detour = open(&reader, "prime-esn/src/lib.rs");
    accept(
        &reader,
        &detour,
        "STUDY_QUESTION: Does Astrid define `Kernel` in `lib.rs`?",
    );
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(choices(&output).is_empty());
    assert!(output.text.contains("no verified contextual match"));
}

#[test]
fn deliberate_question_revision_gets_new_provenance_and_clear_still_works() {
    let (temp, reader) = setup();
    let first = open(&reader, ROUTER);
    accept(&reader, &first, &format!("STUDY_QUESTION: {QUESTION}"));
    let detour = open(&reader, "prime-esn/src/lib.rs");
    accept(
        &reader,
        &detour,
        "STUDY_QUESTION: How does this reservoir's `lib.rs` initialize its state?",
    );
    assert_eq!(
        saved_question(&temp)["reopen"],
        "SELF_STUDY OPEN prime-esn/src/lib.rs 1"
    );
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(choices(&output)[0].ends_with("SELF_STUDY OPEN prime-esn/src/lib.rs 1"));
    accept(&reader, &output, "STUDY_QUESTION: -");
    assert_eq!(saved_question(&temp), Value::Null);
}

#[test]
fn missing_explicit_path_is_not_replaced_with_a_same_named_file() {
    let (_temp, reader) = setup();
    let first = open(&reader, ROUTER);
    accept(
        &reader,
        &first,
        "STUDY_QUESTION: What does `astrid/crates/missing/src/lib.rs` do?",
    );
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(choices(&output).is_empty());
    assert!(output.text.contains("no verified contextual match"));
}

#[test]
fn relative_slash_path_cannot_escape_minime_question_context_via_catalog_aliases() {
    let (temp, reader) = setup();
    let first = open(&reader, "minime/minime_autonomy/source.py");
    accept(
        &reader,
        &first,
        "STUDY_QUESTION: What does Minime's `src/lib.rs` do?",
    );
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let hints = choices(&output);
    assert_eq!(hints.len(), 1);
    assert!(
        hints[0].starts_with("Filename candidate beside the source where your question was saved")
    );
    assert!(hints[0].ends_with("SELF_STUDY OPEN minime/minime_autonomy/src/lib.rs 1"));
    // With the contextual candidate unavailable, Astrid's globally resolvable
    // compatibility path must not become a replacement.
    fs::remove_file(temp.path().join("minime/minime_autonomy/src/lib.rs")).unwrap();
    let output = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(choices(&output).is_empty());
    assert!(output.text.contains("no verified contextual match"));
}
