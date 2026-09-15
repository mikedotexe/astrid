//! What a dense early-sorting data file does to every later file in the catalog.
//!
//! Astrid spent many navigation-only turns hunting "the specific arithmetic
//! formula ... that transforms raw reservoir telemetry into the `fill_pct`
//! value" (`introspection_source_catalog_1789282513` and predecessors). She read
//! the instrument correctly — "my search is hitting a scan limit and returning
//! hundreds of matches in a `.json` file" — and drew the only inference the
//! output offered: that the producer must be hidden somewhere less indexable.
//!
//! It is not. `Catalog::sources` returns ids in sort order and
//! `SearchReport::collect_lines` breaks the *whole* scan at `MAX_HITS`, so one
//! dense fixture that sorts early spends the entire hit budget and every file
//! after it is never read. The header reports `scan limit reached: true` and the
//! number of files read, but neither the count nor any page says which file ate
//! the budget or that a named declaration was never looked at.
//!
//! These tests pin that boundary for both search shapes, and pin the separate
//! grammar fact that an unknown `--option` is absorbed into the literal query.
//! They assert reachability only; no ranking, cap, ordering or being-facing
//! navigation text is changed here.
use astrid_source_study::{Catalog, Reader};
use std::{collections::BTreeMap, fs};

/// One page past the last is an error, so walking stops at the reported count.
fn walk_all_pages(reader: &Reader, command: &str) -> String {
    let mut text = reader.prepare_action(command).unwrap().text;
    let total = text
        .rsplit_once("Navigation page 1/")
        .and_then(|(_, tail)| tail.split('.').next())
        .and_then(|count| count.trim().parse::<usize>().ok())
        .expect("a paginated navigation result");
    for page in 2..=total {
        text.push_str(
            &reader
                .prepare_action(&format!("{command} --page {page}"))
                .unwrap()
                .text,
        );
    }
    text
}

fn reader(files: &[(&str, String)]) -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

/// `autonomous/` sorts before `types/`, exactly as in the live tree, and the
/// fixture alone carries more occurrences than the whole scan is allowed.
fn dense_fixture_then_declaration() -> (tempfile::TempDir, Reader) {
    reader(&[
        (
            "capsules/demo/src/autonomous/fixtures/ledger.json",
            format!("[\n{}]\n", "  {\"fill_pct\": 71.0},\n".repeat(1600)),
        ),
        (
            "capsules/demo/src/types/telemetry.rs",
            "impl Telemetry {\n    pub fn fill_pct(&self) -> f32 {\n        self.fill_ratio * 100.0\n    }\n}\n\nfn resolve_fill_pct(telemetry: &Telemetry) -> f32 {\n    (telemetry.fill_ratio * 100.0).clamp(0.0, 100.0)\n}\n"
                .into(),
        ),
    ])
}

#[test]
fn a_dense_early_fixture_hides_a_later_declaration_from_every_literal_page() {
    let (_temp, reader) = dense_fixture_then_declaration();
    let walked = walk_all_pages(&reader, "SELF_STUDY FIND fill_pct");

    // The reader is told the scan was bounded, which is honest as far as it goes.
    assert!(
        walked.contains("scan limit reached: true"),
        "the dense fixture should bound the scan: {walked}"
    );
    // But the declaration that answers the question is never scanned at all, so
    // no amount of paging can surface it.
    assert!(
        !walked.contains("pub fn fill_pct"),
        "the later declaration must be absent from every page: {walked}"
    );
    assert!(
        !walked.contains("telemetry.rs"),
        "the later file is never read, so it cannot appear even as a path row: {walked}"
    );
    // Nothing in the output names the file that consumed the budget, or says a
    // remainder of the catalog went unread beyond a bare file count.
    assert!(
        !walked.contains("ledger.json consumed"),
        "no cost attribution is offered today: {walked}"
    );
}

#[test]
fn exact_relate_hits_the_same_wall_and_offers_no_definition_candidate() {
    let (_temp, reader) = dense_fixture_then_declaration();
    let walked = walk_all_pages(&reader, "SELF_STUDY RELATE fill_pct");

    // RELATE is the lookup the study prompt offers for a name in her question,
    // and on a saturated query it reaches no further than the literal search.
    assert!(
        walked.contains("scan limit reached: true"),
        "exact search shares the one hit budget: {walked}"
    );
    assert!(
        !walked.contains("Definition candidates"),
        "the declaration is unscanned, so no definition ranking is possible: {walked}"
    );
    assert!(
        !walked.contains("pub fn fill_pct"),
        "the twin declaration stays out of reach: {walked}"
    );
}

#[test]
fn a_query_the_fixture_does_not_carry_scans_the_catalog_and_reaches_the_producer() {
    let (_temp, reader) = dense_fixture_then_declaration();
    let output = reader
        .prepare_action("SELF_STUDY RELATE resolve_fill_pct")
        .unwrap();

    // Same catalog, same ordering, same caps: only the hit density differs.
    assert!(
        output.text.contains("scan limit reached: false"),
        "an unsaturated query reads the whole catalog: {}",
        output.text
    );
    assert!(
        output.text.contains("Definition candidates"),
        "the producer declaration ranks once it is actually scanned: {}",
        output.text
    );
    // The declaration row is the reachable unit; the arithmetic sits on the next
    // line, so the result offers the exact OPEN that would show it.
    assert!(
        output.text.contains("fn resolve_fill_pct"),
        "the producer declaration line is reached: {}",
        output.text
    );
    assert!(
        output.text.contains("telemetry.rs 7"),
        "an exact OPEN into the producer is offered: {}",
        output.text
    );
}

#[test]
fn an_unknown_double_dash_option_is_absorbed_into_the_literal_query() {
    let (_temp, reader) = dense_fixture_then_declaration();
    // Astrid issued `FIND "check_phase_timeout" --path astrid/crates/...` and
    // got one empty page. `--page` is the only recognised suffix; everything
    // else stays in the query text.
    let output = reader
        .prepare_action("SELF_STUDY FIND fill_pct --path capsules/demo/src/types/")
        .unwrap();

    assert!(
        output
            .text
            .contains("No matches for the exact literal query"),
        "the whole string is searched verbatim: {}",
        output.text
    );
    assert!(
        output
            .text
            .contains("fill_pct --path capsules/demo/src/types/"),
        "the unknown option is echoed back inside the query: {}",
        output.text
    );
    // The zero-hit hint names punctuation. It does not name the unknown option,
    // and it does not say that literal search has no path-scoping form.
    assert!(
        output.text.contains("Punctuation is part of the query"),
        "the existing hint is the punctuation one: {}",
        output.text
    );
    assert!(
        !output.text.contains("is not a recognised option"),
        "no unknown-option correction exists today: {}",
        output.text
    );
}
