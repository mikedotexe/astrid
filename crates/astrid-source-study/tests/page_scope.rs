use astrid_source_study::{Catalog, Command, Page, Reader, StudyOutput};
use serde_json::json;
use std::{collections::BTreeMap, fs};

const ID: &str = "astrid/crates/example/src/sample.rs";

fn setup(path: &str, text: &str) -> (tempfile::TempDir, Reader, String) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader, format!("astrid/{path}"))
}

fn open(reader: &Reader, id: &str, line: usize) -> StudyOutput {
    reader
        .prepare(Command::Open {
            source: id.into(),
            line,
        })
        .unwrap()
}

fn accept(reader: &Reader, out: &StudyOutput) {
    let wire = json!({"messages":[{"role":"system","content":out.system_prompt},{"role":"user","content":out.text}]}).to_string();
    let response =
        json!({"message":{"content":"NEXT: SELF_STUDY CONTINUE"},"done":true,"done_reason":"stop"})
            .to_string();
    if let Some(page) = &out.page {
        reader.delivered(&page.id, &wire, &response).unwrap();
    } else {
        reader
            .navigation_delivered(out.navigation_id.as_ref().unwrap(), &wire, &response)
            .unwrap();
    }
}

fn numbered_bytes(page: &Page, original: &str) -> String {
    let mut remaining = page.end.byte.saturating_sub(page.start.byte);
    let mut bytes = String::new();
    for line in page.text.lines() {
        let Some((prefix, fragment)) = line.split_once(" | ") else {
            continue;
        };
        if prefix.trim().parse::<usize>().is_err() || remaining == 0 {
            continue;
        }
        let take = fragment.len().min(remaining);
        bytes.push_str(&fragment[..take]);
        remaining = remaining.saturating_sub(take);
        // Page rendering adds a newline to fragments too. Only recover a source
        // newline when the exact next byte in the frozen source was a newline.
        if remaining > 0
            && original.as_bytes()[page.start.byte.saturating_add(bytes.len())] == b'\n'
        {
            bytes.push('\n');
            remaining = remaining.saturating_sub(1);
        }
    }
    assert_eq!(remaining, 0);
    bytes
}

#[test]
fn middle_of_rust_test_reports_enclosing_function_and_module() {
    let source = "#[cfg(test)]\nmod checks {\n    #[test]\n    fn ack_serializes() {\n        let ack = Ack {\n            active: 0,\n        };\n        assert_eq!(ack.active, 0);\n    }\n}\n";
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", source);
    let out = open(&reader, &id, 6);
    let page = out.page.unwrap();
    assert!(
        page.text.contains("function checks::ack_serializes"),
        "{}",
        page.text
    );
    assert!(page.text.contains("test syntax: #[test] at line 3"));
    assert!(page.text.contains("module checks"));
    assert!(page.text.contains("test syntax: #[cfg(test)] at line 1"));
    assert!(
        page.text
            .contains(&format!("Enclosing declaration: SELF_STUDY OPEN {ID} 4"))
    );
    assert!(page.text.contains("origin: same SOURCE and sha256 above"));
    assert!(page.text.contains("runtime behavior unverified"));
    assert_eq!(
        numbered_bytes(&page, source),
        source[page.start.byte..page.end.byte]
    );
    assert!(
        !page
            .source_locations
            .iter()
            .any(|location| location.name == "ack_serializes")
    );
}

#[test]
fn implementation_method_is_distinguished_from_test_fixture_and_nested_module() {
    let source = "mod lease {\n    struct Ack { active: usize }\n    impl Ack {\n        fn validate(&self) -> bool {\n            self.active == 0\n        }\n    }\n}\n";
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", source);
    let page = open(&reader, &id, 5).page.unwrap();
    assert!(
        page.text.contains("function lease::impl Ack::validate"),
        "{}",
        page.text
    );
    assert!(page.text.contains("no test marker found"));
    assert!(page.text.contains(&format!("SELF_STUDY OPEN {ID} 4")));
    assert!(page.text.contains(&format!("SELF_STUDY OPEN {ID} 3")));
    assert!(!page.text.contains("test syntax:"));
}

#[test]
fn rust_literal_comment_and_attribute_text_cannot_fabricate_declarations() {
    let source = "fn owner() {\n    let source = r###\"\n#[test]\nfn invented() {\n}\n\"###;\n    /*\n    fn commented() {\n    }\n    */\n    let ordinary = \"#[cfg(test)]\";\n}\n";
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", source);
    let out = open(&reader, &id, 4);
    let page = out.page.as_ref().unwrap();
    let metadata = page.text.split("     4 | ").next().unwrap();
    assert!(metadata.contains("function owner"), "{metadata}");
    assert!(metadata.contains("Page begins inside a string/character literal"));
    assert!(!metadata.contains("function invented"));
    assert!(!metadata.contains("test syntax:"));
    assert!(page.source_locations.is_empty());
    accept(&reader, &out);
    let page = open(&reader, &id, 8).page.unwrap();
    assert!(page.text.contains("Page begins inside a comment"));
    assert!(
        !page
            .source_locations
            .iter()
            .any(|location| location.name == "commented")
    );
}

#[test]
fn negative_cfg_is_not_misclassified_as_test_only() {
    let source = "#[cfg(not(test))]\nfn run() {\n    let test = \"#[test]\";\n}\n";
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", source);
    let page = open(&reader, &id, 3).page.unwrap();
    assert!(page.text.contains("no test marker found"));
    assert!(!page.text.contains("test syntax:"));
}

#[test]
fn python_nested_functions_classes_and_triple_strings_have_real_scope() {
    let source = "class Dispatcher:\n    def dispatch(self):\n        def local():\n            return 1\n        return local()\n\ndef test_dispatch():\n    fixture = '''\ndef imagined():\n    pass\n'''\n    assert Dispatcher().dispatch() == 1\n";
    let (_temp, reader, id) = setup("scripts/sample.py", source);
    let out = open(&reader, &id, 4);
    assert!(out.text.contains("tree-sitter-python"));
    assert!(
        out.text.contains("function Dispatcher::dispatch::local"),
        "{}",
        out.text
    );
    assert!(out.text.contains(&format!("SELF_STUDY OPEN {id} 3")));
    accept(&reader, &out);
    let page = open(&reader, &id, 9).page.unwrap();
    assert!(page.text.contains("function test_dispatch"));
    assert!(
        page.text
            .contains("test naming convention; collection/execution unverified")
    );
    assert!(
        page.text
            .contains("Page begins inside a string/character literal")
    );
    assert!(
        !page
            .source_locations
            .iter()
            .any(|location| location.name == "imagined")
    );
}

#[test]
fn malformed_or_unsupported_source_stays_readable_without_guessed_scope() {
    for (path, source, reason) in [
        (
            "crates/example/src/sample.rs",
            "fn broken( {\n    fn fake() {}\n",
            "syntax parse contains errors",
        ),
        (
            "scripts/sample.py",
            "def broken(:\n    pass\n",
            "syntax parse contains errors",
        ),
        (
            "docs/example.md",
            "fn looks_like_rust() {\nnot source\n",
            "unsupported source language",
        ),
    ] {
        let (_temp, reader, id) = setup(path, source);
        let page = open(&reader, &id, 2).page.unwrap();
        assert!(page.text.contains(reason), "{}", page.text);
        assert!(page.text.contains("no declaration guessed"));
        assert!(page.source_locations.is_empty());
        assert_eq!(
            numbered_bytes(&page, source),
            source[page.start.byte..page.end.byte]
        );
    }
}

#[test]
fn oversized_source_skips_parsing_but_preserves_pagination() {
    let source = format!("// {}\n", "x".repeat(2 * 1024 * 1024));
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", &source);
    let out = open(&reader, &id, 1);
    let page = out.page.as_ref().unwrap();
    assert!(page.text.contains("2 MiB syntax inspection limit"));
    assert!(!page.eof);
    assert!(page.text.len() <= astrid_source_study::MAX_PAGE_BYTES);
    assert_eq!(
        numbered_bytes(page, &source),
        source[page.start.byte..page.end.byte]
    );
    assert_eq!(out, reader.prepare_action("SELF_STUDY CONTINUE").unwrap());
    accept(&reader, &out);
    let next = reader
        .prepare_action("SELF_STUDY CONTINUE")
        .unwrap()
        .page
        .unwrap();
    assert_eq!(next.start, page.end);
}

#[test]
fn continued_unicode_fragments_keep_byte_identity_and_declaration_recall_is_delivered_only() {
    let source = format!(
        "fn café() {{\n    let s = r#\"{}\"#;\n}}\nfn next() {{}}\n",
        "λ".repeat(5_000)
    );
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", &source);
    let mut out = open(&reader, &id, 1);
    let first = out.page.as_ref().unwrap();
    assert!(
        first
            .source_locations
            .iter()
            .any(|location| location.name == "café" && location.line == 1)
    );
    assert!(
        !first
            .source_locations
            .iter()
            .any(|location| location.name == "next")
    );
    let mut restored = String::new();
    let mut saw_literal_scope = false;
    loop {
        let page = out.page.as_ref().unwrap();
        assert!(source.is_char_boundary(page.start.byte) && source.is_char_boundary(page.end.byte));
        restored.push_str(&numbered_bytes(page, &source));
        saw_literal_scope |= page
            .text
            .contains("Page begins inside a string/character literal");
        if page.eof {
            break;
        }
        accept(&reader, &out);
        out = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }
    assert_eq!(restored, source);
    assert!(saw_literal_scope);
    assert!(
        out.page
            .unwrap()
            .source_locations
            .iter()
            .any(|location| location.name == "next")
    );
}

#[test]
fn source_locations_default_for_old_pages_and_complete_name_is_required() {
    let (_temp, reader, id) = setup(
        "crates/example/src/sample.rs",
        "pub struct Ack {\n    value: usize,\n}\n",
    );
    let page = open(&reader, &id, 1).page.unwrap();
    assert_eq!(page.source_locations.len(), 1);
    assert_eq!(page.source_locations[0].kind, "struct");
    let mut encoded = serde_json::to_value(&page).unwrap();
    encoded.as_object_mut().unwrap().remove("source_locations");
    let old: Page = serde_json::from_value(encoded).unwrap();
    assert!(old.source_locations.is_empty());
    assert_eq!(old.id, page.id);
    assert_eq!(old.text, page.text);
}

#[test]
fn three_page_sessions_retain_scope_and_useful_source_with_atomic_delivery() {
    let source = format!(
        "#[cfg(test)]\nmod checks {{\n    #[test]\n    fn read_ack() {{\n{}    }}\n}}\n",
        "        let active = 0;\n".repeat(300)
    );
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", &source);
    let out = reader
        .prepare_action(&format!(
            "SELF_STUDY SESSION OPEN {id} 7 | OPEN {id} 30 | OPEN {id} 70"
        ))
        .unwrap();
    assert_eq!(out.session_pages.len(), 3);
    for page in &out.session_pages {
        assert!(
            page.text.contains("function checks::read_ack"),
            "{}",
            page.text
        );
        assert!(page.text.contains(&format!("SELF_STUDY OPEN {id} 4")));
        assert!(page.end.byte.saturating_sub(page.start.byte) >= 400);
        assert!(page.text.len() <= 7_000 / 3);
        assert_eq!(
            numbered_bytes(page, &source),
            source[page.start.byte..page.end.byte]
        );
    }
    assert!(
        out.text
            .len()
            .saturating_add(out.system_prompt.len())
            .saturating_add(32)
            <= astrid_source_study::MAX_INPUT_BYTES
    );
    assert_eq!(out, reader.prepare_action("SELF_STUDY CONTINUE").unwrap());
    accept(&reader, &out);
    let next = reader
        .prepare_action("SELF_STUDY CONTINUE")
        .unwrap()
        .page
        .unwrap();
    assert_eq!(next.start, out.session_pages[2].end);
}

#[test]
fn commented_cfg_tokens_remain_test_context_but_macro_bodies_are_unexpanded() {
    let source = "#[cfg( /* test-only */ test )]\nmod cases {\n    fn exercise() {\n        quote! {\n            fn invented() {}\n        };\n    }\n}\n";
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", source);
    let page = open(&reader, &id, 5).page.unwrap();
    assert!(
        page.text.contains("function cases::exercise"),
        "{}",
        page.text
    );
    assert!(page.text.contains("test syntax: #[cfg(test)] at line 1"));
    assert!(
        page.text
            .contains("Page begins inside unexpanded macro/attribute syntax")
    );
    assert!(page.source_locations.is_empty());
}

#[test]
fn a_declaration_name_cut_by_the_page_boundary_is_not_saved_as_delivered() {
    let source = format!("fn {}() {{}}\n", "a".repeat(9_000));
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", &source);
    let out = open(&reader, &id, 1);
    let first = out.page.as_ref().unwrap();
    assert!(!first.eof);
    assert!(first.source_locations.is_empty());
    assert_eq!(
        numbered_bytes(first, &source),
        source[first.start.byte..first.end.byte]
    );
    accept(&reader, &out);
    let next = reader
        .prepare_action("SELF_STUDY CONTINUE")
        .unwrap()
        .page
        .unwrap();
    assert_eq!(next.start, first.end);
    assert!(next.source_locations.is_empty());
    assert!(next.text.contains(&format!("SELF_STUDY OPEN {id} 1")));
}

#[test]
fn delivered_test_location_keeps_its_test_marker_in_notebook_recall() {
    let source = "#[cfg(test)]\nmod checks {\n    #[test]\n    fn ack_serializes() {\n        assert_eq!(1, 1);\n    }\n}\n";
    let (_temp, reader, id) = setup("crates/example/src/sample.rs", source);
    let out = open(&reader, &id, 1);
    let location = out
        .page
        .as_ref()
        .unwrap()
        .source_locations
        .iter()
        .find(|location| location.name == "ack_serializes")
        .unwrap();
    assert_eq!(location.kind, "function; test syntax: #[test] at line 3");
    accept(&reader, &out);
    let recalled = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(
        recalled
            .text
            .contains("function; test syntax: #[test] at line 3 candidate: ack_serializes")
    );
}
