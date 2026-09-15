use astrid_source_study::{Catalog, Command, InputKind, MAX_PAGE_BYTES, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn put(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    put(
        &root,
        "crates/example/src/lib.rs",
        &"pub fn event() {}\n".repeat(1500),
    );
    put(
        &root,
        "crates/example/src/runtime/handlers.rs",
        "pub fn dispatch() {}\n",
    );
    put(
        &root,
        "crates/example/Cargo.toml",
        "[package]\nname = 'example'\n",
    );
    put(
        &root,
        "crates/example/tests/handler.rs",
        "fn test_handler() {}\n",
    );
    put(&root, "Cargo.toml", "[workspace]\n");
    put(&root, "README.md", "System overview\n");
    for index in 0..160 {
        put(
            &root,
            &format!("docs/steward-notes/history_{index:03}.md"),
            "Historical account\n",
        );
    }
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap();
    let reader = Reader::new(catalog, temp.path().join("reader"));
    (temp, reader)
}

fn prepare(reader: &Reader, command: &str) -> StudyOutput {
    reader.prepare(Command::parse(command).unwrap()).unwrap()
}

fn map_body(output: &StudyOutput) -> &str {
    let start = output
        .text
        .find("Shared system map —")
        .or_else(|| output.text.find("Recursive source list —"))
        .unwrap();
    let end = output.text[start..]
        .find("No source bookmark advanced.")
        .unwrap()
        .saturating_add(start)
        .saturating_add("No source bookmark advanced.".len());
    &output.text[start..end]
}

fn accept(reader: &Reader, output: &StudyOutput) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response =
        json!({"message":{"content":"I can choose a source directly."},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response)
            .unwrap();
    }
}

#[test]
fn repository_map_is_shallow_and_keeps_history_reachable() {
    let (_temp, reader) = setup();
    let output = prepare(&reader, "SELF_STUDY MAP astrid");
    let body = map_body(&output);
    assert_eq!(output.input_kind, InputKind::Map);
    assert!(body.contains("SELF_STUDY MAP astrid/crates"));
    assert!(body.contains("SELF_STUDY MAP astrid/docs"));
    assert!(body.contains("Historical commentary: 160"));
    assert!(
        body.contains("[Configuration / data / interfaces] SELF_STUDY OPEN astrid/Cargo.toml 1")
    );
    assert!(!body.contains("history_000.md"));
    assert!(!body.contains("OPEN astrid/crates/example/src/lib.rs"));
    assert!(body.find("MAP astrid/crates").unwrap() < body.find("MAP astrid/docs").unwrap());
    assert!(body.contains("End of map; no further map page."));
    assert!(body.contains("Recursive scope: SELF_STUDY LIST astrid"));
    let history = prepare(&reader, "SELF_STUDY MAP astrid/docs/steward-notes");
    assert!(map_body(&history).contains("OPEN astrid/docs/steward-notes/history_000.md 1"));
    assert!(map_body(&history).contains("[Historical commentary]"));
    assert!(
        prepare(
            &reader,
            "SELF_STUDY OPEN astrid/docs/steward-notes/history_159.md 1"
        )
        .page
        .is_some()
    );
}

#[test]
fn directories_offer_direct_files_and_use_exact_child_scope() {
    let (_temp, reader) = setup();
    let output = prepare(&reader, "SELF_STUDY MAP astrid/crates/example/src/");
    let body = map_body(&output);
    assert!(body.contains("SELF_STUDY MAP astrid/crates/example/src/runtime"));
    assert!(
        body.contains("[Implementation text] SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1")
    );
    assert!(!body.contains("handlers.rs"));
    assert!(body.contains("Parent/system map: SELF_STUDY MAP astrid/crates/example"));
    let nested = prepare(&reader, "SELF_STUDY MAP astrid/crates/example/src/runtime");
    assert!(
        map_body(&nested)
            .contains("SELF_STUDY OPEN astrid/crates/example/src/runtime/handlers.rs 1")
    );
}

#[test]
fn recursive_list_delivers_every_catalog_identity_without_truncated_commands() {
    let (_temp, reader) = setup();
    let mut seen = std::collections::BTreeSet::new();
    for page in 1_usize..30 {
        let output = prepare(&reader, &format!("SELF_STUDY LIST astrid --page {page}"));
        let body = map_body(&output);
        assert!(body.len() <= MAX_PAGE_BYTES, "{} bytes", body.len());
        for line in body.lines().filter(|line| line.starts_with('[')) {
            let source = line
                .split_once("SELF_STUDY OPEN ")
                .unwrap()
                .1
                .split_whitespace()
                .next()
                .unwrap();
            assert!(seen.insert(source.to_string()), "duplicate {source}");
            assert!(line.ends_with("1 [Not delivered]"));
        }
        assert!(
            body.contains(
                "CONTINUE resumes the saved source bookmark, not the next map/list page."
            )
        );
        if body.contains("End of list;") {
            assert!(page > 1);
            break;
        }
        assert!(body.contains(&format!(
            "Next list page: SELF_STUDY LIST astrid --page {}",
            page.saturating_add(1)
        )));
    }
    assert_eq!(seen.len(), 166);
    assert!(seen.contains("astrid/docs/steward-notes/history_159.md"));
    assert!(seen.contains("astrid/crates/example/src/runtime/handlers.rs"));
}

#[test]
fn paged_compact_map_has_explicit_next_and_terminal_choices() {
    let (_temp, reader) = setup();
    let mut page = 1_usize;
    loop {
        let output = prepare(
            &reader,
            &format!("SELF_STUDY MAP astrid/docs/steward-notes --page {page}"),
        );
        let body = map_body(&output);
        assert!(body.len() <= MAX_PAGE_BYTES);
        assert!(body.contains("Parent/system map: SELF_STUDY MAP astrid/docs"));
        if body.contains("End of map;") {
            assert!(page > 1);
            assert!(!body.contains("Next map page:"));
            break;
        }
        assert!(body.contains(&format!(
            "Next map page: SELF_STUDY MAP astrid/docs/steward-notes --page {}",
            page.saturating_add(1)
        )));
        page = page.saturating_add(1);
        assert!(page < 30);
    }
}

#[test]
fn old_flat_map_page_gets_explicit_layout_migration_recovery() {
    let (_temp, reader) = setup();
    let output = prepare(&reader, "SELF_STUDY MAP astrid --page 126");
    assert_eq!(output.input_kind, InputKind::Recovery);
    assert!(
        output
            .text
            .contains("Requested map page 126 is past the end (1 pages)")
    );
    assert!(output.text.contains("SELF_STUDY LIST astrid --page 126"));
    assert!(output.text.contains("page boundaries may differ"));
    assert!(output.text.contains("Compact scope: SELF_STUDY MAP astrid"));
    assert!(output.text.contains("Shared system map"));
    assert!(output.page.is_none());
}

#[test]
fn catalog_choices_do_not_advance_a_source_bookmark_and_continue_keeps_source_meaning() {
    let (temp, reader) = setup();
    let first = prepare(
        &reader,
        "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1",
    );
    accept(&reader, &first);
    let before: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    let map = prepare(&reader, "SELF_STUDY MAP astrid/crates/example/src");
    assert!(map_body(&map).contains("SELF_STUDY RESUME astrid/crates/example/src/lib.rs"));
    accept(&reader, &map);
    let list = prepare(&reader, "SELF_STUDY LIST astrid");
    accept(&reader, &list);
    let after: Value =
        serde_json::from_slice(&fs::read(temp.path().join("reader/reader-v1.json")).unwrap())
            .unwrap();
    assert_eq!(before["current"], after["current"]);
    assert_eq!(before["bookmarks"], after["bookmarks"]);
    assert_eq!(before["progress"], after["progress"]);
    let next = prepare(&reader, "SELF_STUDY CONTINUE");
    assert_eq!(
        next.page.as_ref().unwrap().source,
        "astrid/crates/example/src/lib.rs"
    );
    assert_eq!(
        next.page.as_ref().unwrap().start,
        first.page.as_ref().unwrap().end
    );
}

#[test]
fn list_is_additive_typed_and_validated() {
    let list = Command::List {
        topic: "astrid/crates".into(),
        page: 3,
    };
    assert_eq!(
        Command::parse("SELF_STUDY LIST astrid/crates --page 3").unwrap(),
        list
    );
    assert_eq!(
        Command::parse("SELF_STUDY REPLACE LIST astrid/crates --page 3").unwrap(),
        list
    );
    assert_eq!(
        serde_json::from_value::<Command>(
            json!({"command":"list","topic":"astrid/crates","page":3})
        )
        .unwrap(),
        list
    );
    assert!(Command::parse("SELF_STUDY LIST").is_err());
    assert!(Command::parse("SELF_STUDY LIST astrid --page 0").is_err());
    assert!(Command::parse("SELF_STUDY LIST astrid --page nan").is_err());
    assert_eq!(
        Command::parse("SELF_STUDY MAP astrid --page 3").unwrap(),
        Command::Map {
            topic: "astrid".into(),
            page: 3
        }
    );
}

#[test]
fn long_utf8_identities_stay_complete_and_openable_across_map_pages() {
    let (temp, reader) = setup();
    let directory = format!(
        "crates/example/src/{}/{}/{}",
        "first".repeat(16),
        "second".repeat(13),
        "third".repeat(16)
    );
    let scope = format!("astrid/{directory}");
    for index in 0..20 {
        put(
            &temp.path().join("astrid"),
            &format!("{directory}/{}_{index:02}.rs", "文".repeat(50)),
            "pub fn long_path() {}\n",
        );
    }
    let mut found = std::collections::BTreeSet::new();
    for page in 1_usize..30 {
        let output = prepare(&reader, &format!("SELF_STUDY MAP {scope} --page {page}"));
        let body = map_body(&output);
        assert!(body.len() <= MAX_PAGE_BYTES);
        for line in body
            .lines()
            .filter(|line| line.starts_with("[Implementation text]"))
        {
            let command = line
                .split_once("] ")
                .unwrap()
                .1
                .strip_suffix(" [Not delivered]")
                .unwrap();
            let Command::Open { source, line } = Command::parse(command).unwrap() else {
                panic!("incomplete source command: {command}");
            };
            assert_eq!(line, 1);
            assert!(found.insert(source.clone()));
            assert!(temp.path().join(&source).is_file());
            assert!(
                reader
                    .prepare(Command::Open { source, line })
                    .unwrap()
                    .page
                    .is_some()
            );
        }
        if body.contains("End of map;") {
            break;
        }
    }
    assert_eq!(found.len(), 20);
}
