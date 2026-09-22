//! Regression coverage for the historical delivered-line ambiguity below.
//! The September 17 interface repair now labels inclusive delivered lines,
//! separates declaration metadata, and explicitly marks oversized fragments.
//! These tests supersede the original negative diagnostic assertions; the
//! corresponding steward run packet retains their original evidence.
//!
//! In `introspection_minime_minime_src_sensory_bus.rs_1789608500` Astrid opens
//! with "The current page (lines 464–620)". The page she was handed
//! (`page efabf5fb3465d74a0620642dd51fba75d8bedbfa70435cbe5a096eaffb2d6042`,
//! `minime/minime/src/sensory_bus.rs`
//! sha256:3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a)
//! delivered bytes 18492..22865, which is lines 502..620 — the witness
//! `lsw_618ae0646e…` records `window_start_line: 502`. Her 620 is exact. Her
//! 464 is the start line of the *enclosing declaration*, printed by the page's
//! own SOURCE SCOPE row: "function `modality_boundary_transparency_v1` (lines
//! 464–515; no test marker found)."
//!
//! At the time, this was the only line range the header stated in words.
//! `Page::read` then wrote the delivered interval as
//! "Exact source bytes {start}..{end}" and never as lines, while
//! `Outline::scope_text` (`src/source_structure.rs:147-156`) writes
//! "(lines `{start_line}`–`{end_line}`)" for the declaration the page begins
//! inside. A reader who wants the page's own line bounds must read them off the
//! numbered gutter — whose first row, here, was the fragment
//! "   502 | lityBoundaryTransparencyV1 {", because the previous page's budget
//! ran out mid-line 502.
//!
//! The exclusive end cursor can point to the next line after the last delivered
//! row. Regression expectations derive the inclusive end from actual source
//! bytes, not from declaration spans or a fixed page size. Nothing here asserts
//! what Astrid should write or establishes improved understanding.
use astrid_source_study::{Catalog, Command, Page, Reader, StudyOutput};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "minime/minime/src/demo_boundary.rs";
/// The declaration a mid-body page begins inside, spelled as the fixture spells it.
const ENCLOSING: &str = "pub fn boundary_spanning_review(";
/// Longer than one whole page budget, so a page boundary must fall inside it.
const UNSPLITTABLE_LINE_CHARS: usize = 9_000;

/// One file shaped like the live `sensory_bus.rs` at her page boundary: a long
/// public function whose body outruns a single page, containing one line too
/// long to fit any page, and more file after the function closes.
fn long_declaration_crossing_page_boundaries() -> (tempfile::TempDir, Reader, String) {
    let mut text = String::from(
        "use parking_lot::Mutex;\n\npub struct Review {\n    pub acc: usize,\n}\n\n#[must_use]\n",
    );
    text.push_str(ENCLOSING);
    text.push_str(
        "\n    audio: &str,\n    video: &str,\n) -> Review {\n    let mut acc = 0usize;\n",
    );
    for step in 0..12 {
        writeln!(text, "    acc = acc.saturating_add({step});").unwrap();
    }
    // One comment line no page can hold whole, so the next page starts mid-line.
    text.push_str("    // ");
    text.push_str(&"boundary".repeat(UNSPLITTABLE_LINE_CHARS / 8));
    text.push('\n');
    for step in 0..120 {
        writeln!(text, "    acc = acc.saturating_add({step} * 2);").unwrap();
    }
    text.push_str("    Review { acc }\n}\n\npub fn after_the_declaration() -> usize {\n    0\n}\n");

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("minime");
    let path = root.join("minime/src/demo_boundary.rs");
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

/// Walk forward from line 1 until a page begins strictly inside the enclosing
/// declaration. Returns that page and the pages delivered before it.
fn walk_into_the_declaration_body(reader: &Reader, text: &str) -> (StudyOutput, usize) {
    let enclosing_line = line_of(text, ENCLOSING);
    let mut output = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 1,
        })
        .unwrap();
    for _ in 0..12 {
        let page = output.page.as_ref().unwrap().clone();
        if page.start.line > enclosing_line {
            return (output, enclosing_line);
        }
        assert!(!page.eof, "the walk reached EOF before a mid-body page");
        reader
            .delivered(&page.id, &wire(&output.text), &response())
            .unwrap();
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }
    panic!("the fixture must yield a page starting inside the declaration");
}

/// Byte offset at which one-based `line` begins.
fn byte_of_line(text: &str, line: usize) -> usize {
    text.split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum()
}

/// Inclusive line coverage and exclusive byte cursors are both explicit.
#[test]
fn the_header_states_the_delivered_interval_in_bytes_and_inclusive_lines() {
    let (_temp, reader, text) = long_declaration_crossing_page_boundaries();
    let (output, _) = walk_into_the_declaration_body(&reader, &text);
    let page: Page = output.page.clone().unwrap();

    assert!(
        output.text.contains(&format!(
            "Exact source bytes {}..{}",
            page.start.byte, page.end.byte
        )),
        "the header must state the delivered byte interval"
    );
    let last_delivered_line = text[..page.end.byte].lines().count();
    assert!(output.text.contains(&format!(
        "Delivered source lines {}\u{2013}{last_delivered_line} (inclusive; partial lines explicitly marked)",
        page.start.line
    )));
    assert!(output.text.contains("(end exclusive)"));
    assert!(
        output
            .text
            .contains("Declaration spans below are separate metadata")
    );
}

/// The start declaration's range is retained, separately from page coverage.
#[test]
fn the_start_scope_range_is_distinct_from_delivered_line_coverage() {
    let (_temp, reader, text) = long_declaration_crossing_page_boundaries();
    let (output, enclosing_line) = walk_into_the_declaration_body(&reader, &text);
    let page = output.page.clone().unwrap();

    assert!(
        output.text.contains("boundary_spanning_review (lines")
            && output.text.contains(&format!(
                "Enclosing declaration: SELF_STUDY OPEN {SOURCE} {enclosing_line}"
            )),
        "the scope row must name the enclosing declaration and its start line"
    );

    let ranges = prose_line_ranges(&output.text);
    assert_eq!(
        ranges.len(),
        1,
        "exactly one prose line range is expected on this page, found {ranges:?}"
    );
    let (range_start, range_end) = ranges[0];
    assert_eq!(
        range_start, enclosing_line,
        "the prose range must start at the declaration, not at the page"
    );
    assert!(
        range_start < page.start.line,
        "the declaration starts strictly before the first delivered line, so \
         borrowing its start understates the page start"
    );
    assert!(
        range_end != page.end.line || range_start != page.start.line,
        "the prose range must not coincide with the delivered interval"
    );
}

/// Oversized lines remain reachable, but a continuation fragment is labelled.
#[test]
fn the_first_gutter_row_marks_and_preserves_a_mid_line_fragment() {
    let (_temp, reader, text) = long_declaration_crossing_page_boundaries();
    let (previous, _) = walk_into_the_declaration_body(&reader, &text);
    let previous_page = previous.page.as_ref().unwrap();
    reader
        .delivered(&previous_page.id, &wire(&previous.text), &response())
        .unwrap();
    let output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    let page = output.page.clone().unwrap();
    assert_eq!(page.start, previous_page.end);

    assert!(
        output.text.contains(&format!(
            "Partial source line {}; began before this page",
            page.start.line
        )),
        "a resumed fragment must be labelled outside the source gutter"
    );
    let first_row = output
        .text
        .lines()
        .find(|line| {
            line.split_once(" | ")
                .is_some_and(|(number, _)| number.trim().parse::<usize>().is_ok())
        })
        .expect("the page must render a numbered gutter");
    let (number, fragment) = first_row.split_once(" | ").unwrap();
    assert_eq!(
        number.trim().parse::<usize>().unwrap(),
        page.start.line,
        "the first gutter number must be the delivered start line"
    );

    let whole_line = text.lines().nth(page.start.line.saturating_sub(1)).unwrap();
    assert!(
        page.start.byte > byte_of_line(&text, page.start.line),
        "this fixture page must begin partway into its start line"
    );
    assert!(
        !fragment.is_empty() && whole_line != fragment,
        "the first row must contain a strict fragment"
    );
    assert_eq!(
        fragment,
        text[page.start.byte..page.end.byte].lines().next().unwrap()
    );
}

/// Gutter bounds agree with exact source coverage even for a single long line.
#[test]
fn the_exact_delivered_interval_is_available_on_the_page_record() {
    let (_temp, reader, text) = long_declaration_crossing_page_boundaries();
    let (output, _) = walk_into_the_declaration_body(&reader, &text);
    let page = output.page.clone().unwrap();

    let gutter: Vec<usize> = output
        .text
        .lines()
        .filter_map(|line| line.split_once(" | "))
        .filter_map(|(number, _)| number.trim().parse().ok())
        .collect();
    assert!(!gutter.is_empty(), "the page must make nonempty progress");
    assert_eq!(
        *gutter.first().unwrap(),
        page.start.line,
        "start.line is the first rendered line"
    );
    assert_eq!(
        *gutter.last().unwrap(),
        text[..page.end.byte].lines().count()
    );
    assert!(page.end.byte > page.start.byte);
    assert_eq!(
        (page.revision.bytes, page.revision.lines),
        (text.len(), text.lines().count()),
        "the page must bind the revision extent it was read from"
    );
}

/// The bounded correction: the number the scope row offers is a real Action, and
/// taking it returns the declaration head rather than the page she was on — so
/// the two spans are distinguishable in one read.
#[test]
fn opening_the_scope_row_line_returns_the_declaration_head_not_the_page() {
    let (_temp, reader, text) = long_declaration_crossing_page_boundaries();
    let (output, enclosing_line) = walk_into_the_declaration_body(&reader, &text);
    let page = output.page.clone().unwrap();
    reader
        .delivered(&page.id, &wire(&output.text), &response())
        .unwrap();

    let back = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} {enclosing_line}"))
        .unwrap();
    assert!(
        back.text.contains(ENCLOSING),
        "opening the scope row's line must deliver the declaration head"
    );
    let head = back.page.as_ref().unwrap();
    assert_eq!(
        head.start.line, enclosing_line,
        "the reopened page must start exactly at the declaration"
    );
    assert!(
        head.start.line < page.start.line,
        "the declaration head is strictly behind the page she was reading"
    );
}
