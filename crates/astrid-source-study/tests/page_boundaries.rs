//! Delivered line intervals, bounded fragments and exact continuation are
//! separate from syntax spans and verified coverage.
use astrid_source_study::{Catalog, MAX_PAGE_BYTES, Reader, StudyOutput};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/crates/example/src/boundaries.rs";

fn setup(source: &str) -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let path = root.join("crates/example/src/boundaries.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn accept(reader: &Reader, output: &StudyOutput) {
    let request = json!({"messages": [{"role": "user", "content": output.text}]}).to_string();
    let response = json!({"message": {"content": "NEXT: SELF_STUDY CONTINUE"}, "done": true, "done_reason": "stop"}).to_string();
    reader
        .delivered(&output.page.as_ref().unwrap().id, &request, &response)
        .unwrap();
}

#[test]
fn ordinary_source_walk_keeps_identifiers_and_inclusive_line_labels_intact() {
    let mut source = "fn update(pressure: f64) {\n".to_owned();
    for _ in 0..180 {
        source.push_str(
            "    let entropy_mult = semantic_context_persistence_multiplier(pressure);\n",
        );
    }
    source.push_str(
        "}\nfn semantic_context_persistence_multiplier(pressure: f64) -> f64 { pressure }\n",
    );
    let (_temp, reader) = setup(&source);
    let mut output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let mut next_byte = 0;
    let mut delivered_lines = 0usize;
    loop {
        let page = output.page.as_ref().unwrap();
        assert_eq!(page.start.byte, next_byte);
        assert!(page.text.len() <= MAX_PAGE_BYTES);
        assert!(page.end.byte > page.start.byte);
        assert!(
            page.start.byte == 0 || source.as_bytes()[page.start.byte.saturating_sub(1)] == b'\n'
        );
        assert_eq!(source.as_bytes()[page.end.byte.saturating_sub(1)], b'\n');
        assert!(!page.text.contains("Partial source line"));
        let last_line = page.end.line.saturating_sub(1);
        assert!(page.text.contains(&format!(
            "Delivered source lines {}–{last_line} (inclusive; complete lines)",
            page.start.line
        )));
        for row in page.text.lines() {
            let Some((prefix, code)) = row.split_once(" | ") else {
                continue;
            };
            let Ok(line) = prefix.trim().parse::<usize>() else {
                continue;
            };
            assert_eq!(code, source.lines().nth(line.saturating_sub(1)).unwrap());
            delivered_lines = delivered_lines.saturating_add(1);
        }
        next_byte = page.end.byte;
        let eof = page.eof;
        accept(&reader, &output);
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
        if eof {
            assert!(output.page.is_none());
            break;
        }
    }
    assert_eq!(next_byte, source.len());
    assert_eq!(delivered_lines, source.lines().count());
}

#[test]
fn final_page_inside_a_long_line_does_not_claim_the_whole_line_or_file_was_delivered() {
    let source = format!("// {}\nfn after() {{}}\n", "λ".repeat(8_000));
    let (_temp, reader) = setup(&source);
    let mut output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let first = output.page.as_ref().unwrap().clone();
    assert!(
        first
            .text
            .contains("Partial source line 1; begins here; continues on next page")
    );
    assert!(
        first
            .text
            .contains("lines 1–1 (inclusive; partial lines explicitly marked)")
    );
    // A failed completion leaves both the pending interval and coverage intact.
    let request = json!({"messages": [{"role": "user", "content": output.text}]}).to_string();
    let failed =
        json!({"message": {"content": "unfinished"}, "done": true, "done_reason": "length"})
            .to_string();
    assert!(reader.delivered(&first.id, &request, &failed).is_err());
    assert_eq!(
        reader.prepare_action("SELF_STUDY CONTINUE").unwrap(),
        output
    );
    let mut final_page = None;
    for _ in 0..20 {
        let previous = output.page.as_ref().unwrap().clone();
        accept(&reader, &output);
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
        let page = output.page.as_ref().unwrap();
        assert_eq!(page.start, previous.end);
        assert!(page.text.len() <= MAX_PAGE_BYTES);
        assert!(source.is_char_boundary(page.start.byte) && source.is_char_boundary(page.end.byte));
        if page.eof {
            final_page = Some(page.clone());
            break;
        }
    }
    let page = final_page.expect("bounded fixture must reach EOF");
    assert_eq!(page.end.byte, source.len());
    assert!(
        page.text
            .contains("Partial source line 1; began before this page; ends here")
    );
    assert!(
        output
            .text
            .contains("If this offered page is verified, all bytes")
    );
    assert!(
        output
            .text
            .contains("End of file is a reading position, not a claim")
    );
}

#[test]
fn page_end_names_the_unfinished_declaration_separately_from_the_delivered_lines() {
    let mut source = "struct Opening {\n".to_owned();
    for field in 0..100 {
        writeln!(source, "    field_{field}: usize,").unwrap();
    }
    source.push_str("}\nfn finishing_later() {\n");
    for _ in 0..200 {
        source.push_str("    let value = 1;\n");
    }
    source.push_str("}\n");
    let closing_line = source.lines().count();
    let definition_line = source
        .lines()
        .position(|line| line == "fn finishing_later() {")
        .unwrap()
        .saturating_add(1);
    let (_temp, reader) = setup(&source);
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 2"))
        .unwrap();
    let page = output.page.unwrap();
    assert!(page.start.line < definition_line && page.end.line > definition_line);
    assert!(page.end.line < closing_line);
    assert!(page.text.contains("SOURCE SCOPE at page start"));
    assert!(page.text.contains("struct Opening (lines 1–102"));
    assert!(page.text.contains(&format!(
        "PAGE END SCOPE — function finishing_later continues beyond this page (declaration lines {definition_line}–{closing_line})"
    )));
    assert!(
        page.text
            .contains("Syntax span is metadata, not delivered coverage")
    );
}

#[test]
fn empty_source_has_no_fictitious_delivered_line_range() {
    let (_temp, reader) = setup("");
    let output = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} 1"))
        .unwrap();
    let page = output.page.unwrap();
    assert_eq!(page.start.byte, page.end.byte);
    assert!(page.eof);
    assert!(
        page.text
            .contains("No source lines delivered (empty byte interval)")
    );
    assert!(!page.text.contains("Delivered source lines 1–1"));
}
