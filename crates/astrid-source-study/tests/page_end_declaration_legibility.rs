//! Regression coverage for the historical page-end ambiguity below.
//! The September 17 repair now separately names an unfinished end declaration
//! and preserves ordinary whole lines. Original diagnostic results remain in
//! the corresponding steward run packet; these assertions protect the repair.
//!
//! In `introspection_minime_minime_src_sensory_bus.rs_1789608023` Astrid reads
//! `minime/minime/src/sensory_bus.rs`
//! (sha256:3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a),
//! page `2f4091d1b129eff3…`, bytes 14188..18492 = lines 376..502. She writes
//! "`modality_boundary_transparency_v1` (463–502)". The function opens at 464
//! (`#[must_use]` at 463) and closes at **515**; 502 is only where the page
//! budget ran out, mid-token, rendering `   502 | Moda`.
//!
//! The original investigation identified a rendering asymmetry.
//! `Outline::scope_text` (`src/source_structure.rs:114-156`) resolves
//! `self.enclosing(anchor)` at the page **start** byte
//! (`src/page.rs:74-79`), so the SOURCE SCOPE row spelled
//! "struct `SemanticReceptivityPulseReviewV1` (lines 374–383…)" — the span she
//! then took for the page's own start. Nothing anywhere in the rendered bytes
//! resolves the declaration enclosing `page.end`, so its closing line is
//! was unavailable and the last gutter row was the only end-looking number.
//!
//! `page.source_locations` already carried its name and line. PAGE END SCOPE
//! now supplies its full syntax span without claiming those bytes were read.
//! These tests distinguish that metadata from actual delivered coverage;
//! nothing here asserts what Astrid should write or establishes understanding.
//!
//! Companion: `page_line_interval_legibility.rs` pins the start side of the same
//! render, including explicit inclusive line coverage and oversized fragments.

use astrid_source_study::{Catalog, Command, Page, Reader, StudyOutput};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "minime/minime/src/demo_span.rs";
/// The declaration a mid-body page begins inside — the one the scope row names.
const OPENING: &str = "pub struct OpeningReview {";
/// The declaration that same page ends inside, named separately at page end.
const CLOSING: &str = "pub fn closing_boundary_review(";

/// One file shaped like `sensory_bus.rs` around her page: a long struct whose
/// fields outrun the first page, then a later long function whose body outruns
/// the second — so page two begins inside one declaration and ends inside
/// another, with whole declarations in between.
fn declarations_straddling_both_page_edges() -> (tempfile::TempDir, Reader, String) {
    let mut text = String::from("use std::fmt;\n\n#[derive(Clone, Debug)]\n");
    text.push_str(OPENING);
    text.push('\n');
    for field in 0..220 {
        writeln!(text, "    pub field_{field:03}: usize,").unwrap();
    }
    text.push_str("}\n\npub fn small_helper(value: usize) -> usize {\n    value\n}\n\n");
    text.push_str("#[must_use]\n");
    text.push_str(CLOSING);
    text.push_str(
        "\n    audio: &str,\n    video: &str,\n) -> usize {\n    let mut acc = 0usize;\n",
    );
    for step in 0..240 {
        writeln!(text, "    acc = acc.saturating_add({step});").unwrap();
    }
    text.push_str("    acc\n}\n\npub fn after_everything() -> usize {\n    0\n}\n");

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("minime");
    let path = root.join("minime/src/demo_span.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, &text).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("minime".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader, text)
}

fn wire(text: &str) -> String {
    json!({ "messages": [{ "role": "user", "content": text }] }).to_string()
}

fn response() -> String {
    json!({ "message": { "content": "NEXT: SELF_STUDY CONTINUE" }, "done": true }).to_string()
}

/// One-based line number of the first line containing `needle`.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .map_or_else(
            || panic!("fixture must contain {needle}"),
            |index| index.saturating_add(1),
        )
}

/// One-based line of the `}` that closes the declaration opened at `start`.
fn closing_brace_line(text: &str, start: usize) -> usize {
    text.lines()
        .enumerate()
        .skip(start)
        .find(|(_, line)| *line == "}")
        .map_or_else(
            || panic!("fixture declaration at {start} must close at column zero"),
            |(index, _)| index.saturating_add(1),
        )
}

/// Every `(lines A–B;` range the page spells in prose, in order.
fn prose_line_ranges(page_text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for tail in page_text.split("(lines ").skip(1) {
        let Some(head) = tail.split(';').next() else {
            continue;
        };
        let mut parts = head.split('\u{2013}');
        if let (Some(start), Some(end)) = (parts.next(), parts.next())
            && let (Ok(start), Ok(end)) = (start.trim().parse(), end.trim().parse())
        {
            ranges.push((start, end));
        }
    }
    ranges
}

/// Numbers of the rendered gutter rows, in delivered order.
fn gutter(page_text: &str) -> Vec<usize> {
    page_text
        .lines()
        .filter_map(|line| line.split_once(" | "))
        .filter_map(|(number, _)| number.trim().parse().ok())
        .collect()
}

/// Walk forward until one page begins inside `OpeningReview` *and* ends inside
/// `closing_boundary_review`. Returns that page's output and both declarations'
/// line spans.
fn page_spanning_two_declarations(
    reader: &Reader,
    text: &str,
) -> (StudyOutput, (usize, usize), (usize, usize)) {
    let opening_start = line_of(text, OPENING);
    let opening_end = closing_brace_line(text, opening_start);
    let closing_start = line_of(text, CLOSING);
    let closing_end = closing_brace_line(text, closing_start);

    let mut output = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 1,
        })
        .unwrap();
    for _ in 0..12 {
        let page = output.page.as_ref().unwrap().clone();
        if page.start.line > opening_start
            && page.start.line <= opening_end
            && page.end.line > closing_start
            && page.end.line < closing_end
        {
            return (
                output,
                (opening_start, opening_end),
                (closing_start, closing_end),
            );
        }
        assert!(!page.eof, "the walk reached EOF before the straddling page");
        reader
            .delivered(&page.id, &wire(&output.text), &response())
            .unwrap();
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }
    panic!("the fixture must yield a page that begins and ends inside declarations");
}

/// The scope row resolves the page's start byte, so it names the declaration the
/// page opens inside and says nothing about the one it closes inside.
#[test]
fn the_scope_row_names_the_start_declaration_and_not_the_end_declaration() {
    let (_temp, reader, text) = declarations_straddling_both_page_edges();
    let (output, opening, closing) = page_spanning_two_declarations(&reader, &text);

    assert!(
        output
            .text
            .contains(&format!("(lines {}\u{2013}{};", opening.0, opening.1)),
        "the scope row must spell the start declaration's span"
    );
    assert!(
        output.text.contains(&format!(
            "Enclosing declaration: SELF_STUDY OPEN {SOURCE} {}",
            opening.0
        )),
        "the scope row must offer the start declaration's head as an Action"
    );
    for spelling in [
        format!("(lines {}\u{2013}{};", closing.0, closing.1),
        format!(
            "Enclosing declaration: SELF_STUDY OPEN {SOURCE} {}",
            closing.0
        ),
    ] {
        assert!(
            !output.text.contains(&spelling),
            "the page must not name the end declaration as {spelling}"
        );
    }
}

/// The end declaration's syntax span is labelled without extending coverage.
#[test]
fn the_end_declarations_closing_line_is_metadata_not_delivered_coverage() {
    let (_temp, reader, text) = declarations_straddling_both_page_edges();
    let (output, _, closing) = page_spanning_two_declarations(&reader, &text);
    let page: Page = output.page.clone().unwrap();

    assert!(
        closing.1 > page.end.line,
        "the end declaration must close past the delivered interval"
    );
    assert!(
        prose_line_ranges(&output.text)
            .iter()
            .all(|(_, end)| *end != closing.1),
        "the start-scope range must not be replaced by the end declaration"
    );
    assert!(output.text.contains(&format!(
        "PAGE END SCOPE — function closing_boundary_review continues beyond this page (declaration lines {}\u{2013}{})",
        closing.0, closing.1
    )));
    assert!(
        output
            .text
            .contains("Syntax span is metadata, not delivered coverage")
    );
    let rows = gutter(&output.text);
    assert_eq!(
        *rows.last().unwrap(),
        text[..page.end.byte].lines().count(),
        "the last gutter row carries the delivered end line, not a declaration end"
    );
    assert!(
        output.text.contains(CLOSING),
        "the end declaration's own signature is rendered, so it reads as begun here"
    );
}

/// Ordinary lines are delivered whole; the end cursor points to the next line.
#[test]
fn the_last_ordinary_gutter_row_is_whole_and_precedes_the_end_cursor() {
    let (_temp, reader, text) = declarations_straddling_both_page_edges();
    let (output, _, _) = page_spanning_two_declarations(&reader, &text);
    let page = output.page.clone().unwrap();

    // The navigation footer also contains " | ", so a gutter row is one whose
    // prefix parses as a line number.
    let (number, fragment) = output
        .text
        .lines()
        .filter_map(|line| line.split_once(" | "))
        .rfind(|(number, _)| number.trim().parse::<usize>().is_ok())
        .expect("the page must render a numbered gutter");
    assert_eq!(
        number.trim().parse::<usize>().unwrap(),
        page.end.line.saturating_sub(1),
        "the last gutter number must be the delivered end line"
    );
    let last_line = number.trim().parse::<usize>().unwrap();
    let whole_line = text.lines().nth(last_line.saturating_sub(1)).unwrap();
    assert_eq!(
        fragment, whole_line,
        "ordinary source lines must not be split"
    );
    assert_eq!(text.as_bytes()[page.end.byte.saturating_sub(1)], b'\n');
    assert!(!page.text.contains("Partial source line"));
}

/// The end declaration has both a delivered name and a separately labelled span.
#[test]
fn the_page_record_and_end_scope_name_the_same_unfinished_declaration() {
    let (_temp, reader, text) = declarations_straddling_both_page_edges();
    let (output, _, closing) = page_spanning_two_declarations(&reader, &text);
    let page = output.page.clone().unwrap();

    let listed = page
        .source_locations
        .iter()
        .find(|location| location.name.contains("closing_boundary_review"))
        .expect("the end declaration's name is inside the delivered interval");
    assert_eq!(
        listed.line, closing.0,
        "the record carries the end declaration's exact start line"
    );
    assert!(
        page.end.line < closing.1,
        "the record's own end line is strictly inside that declaration"
    );
    assert!(
        output.text.contains(&format!(
            "(declaration lines {}\u{2013}{})",
            closing.0, closing.1
        )),
        "the end span must be rendered explicitly as declaration metadata"
    );
}

/// The bounded correction: one CONTINUE spells the end declaration's full span,
/// because the next page begins inside it.
#[test]
fn continuing_spells_the_end_declarations_full_span() {
    let (_temp, reader, text) = declarations_straddling_both_page_edges();
    let (output, _, closing) = page_spanning_two_declarations(&reader, &text);
    let page = output.page.clone().unwrap();
    reader
        .delivered(&page.id, &wire(&output.text), &response())
        .unwrap();

    let next = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert!(
        next.text
            .contains(&format!("(lines {}\u{2013}{};", closing.0, closing.1)),
        "the next page's scope row names the declaration its start sits inside"
    );
    let next_page = next.page.as_ref().unwrap();
    assert_eq!(
        next_page.start.line, page.end.line,
        "CONTINUE resumes at the exact exclusive end cursor"
    );
    assert_eq!(next_page.start.byte, page.end.byte);
}
