//! Reachability boundary for a value's producer when the producing function's
//! name only *contains* the queried field name as a suffix.
//!
//! Astrid spent repeated navigation-only turns
//! (`introspection_source_catalog_1789259010` and predecessors) searching for
//! "the specific arithmetic used by the `research_budget_guard` to calculate
//! `fill_pct`", expecting to find it under identifiers like "authority",
//! "budget", "guard" or "policy". In the live tree the producer is
//! `resolve_fill_pct` in `capsules/spectral-bridge/src/ws/telemetry_port.rs`,
//! and a near-twin `SpectralTelemetry::fill_pct()` lives in
//! `types/schema/telemetry.rs`. These tests pin what each search shape can and
//! cannot reach, so the gap is a recorded boundary rather than a surprise.
use astrid_source_study::{Catalog, Reader};
use std::{collections::BTreeMap, fs};

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

/// Two producers of one percentage, shaped like the live tree: a twin method
/// named exactly for the field, and the actual upstream producer whose name
/// carries the field name as a suffix.
fn twin_and_producer() -> (tempfile::TempDir, Reader) {
    reader(&[
        (
            "capsules/demo/src/telemetry.rs",
            "impl Telemetry {\n    pub fn fill_pct(&self) -> f32 {\n        self.fill_ratio * 100.0\n    }\n}\n"
                .into(),
        ),
        (
            "capsules/demo/src/telemetry_port.rs",
            "fn resolve_fill_pct(telemetry: &Telemetry) -> (f32, bool) {\n    if telemetry.fill_ratio.is_finite() {\n        ((telemetry.fill_ratio * 100.0).clamp(0.0, 100.0), false)\n    } else {\n        (estimate(telemetry.lambda1()), true)\n    }\n}\n\nfn accept(state: &mut State, telemetry: &Telemetry) {\n    let (resolved, _) = resolve_fill_pct(telemetry);\n    state.fill_pct = resolved;\n}\n"
                .into(),
        ),
        (
            "capsules/demo/src/dispatch.rs",
            "    budget_guard_for_next(&original, ctx.fill_pct, ctx.telemetry);\n".repeat(9),
        ),
    ])
}

#[test]
fn exact_identifier_search_reaches_the_twin_definition_but_not_the_suffix_named_producer() {
    let (_temp, reader) = twin_and_producer();
    let output = reader.prepare_action("SELF_STUDY RELATE fill_pct").unwrap();

    // The twin method is named exactly for the field, so it ranks as the
    // definition the reader is offered first.
    assert!(
        output.text.contains("Definition candidates"),
        "exact search should offer a definition candidate: {}",
        output.text
    );
    assert!(
        output.text.contains("telemetry.rs 2"),
        "the twin declaration line should be the definition row: {}",
        output.text
    );

    // The producing function's declaration line never matches at all: word
    // splitting yields `resolve_fill_pct`, which is not the queried word.
    assert!(
        !output.text.contains("fn resolve_fill_pct"),
        "suffix-named producer declaration must not appear in exact results: {}",
        output.text
    );

    // Its file is still present, but only through a pass-through assignment,
    // which carries no declaration-like ranking.
    assert!(
        output.text.contains("state.fill_pct = resolved;"),
        "the producer file should appear via its assignment line: {}",
        output.text
    );

    // Spelling hints cannot bridge the gap either: they are suppressed while
    // implementation matches exist, and the length difference exceeds the bound.
    assert!(
        !output.text.contains("Candidate identifier text"),
        "nearby spellings stay suppressed when implementation hits exist: {}",
        output.text
    );
}

#[test]
fn literal_search_finds_the_producer_declaration_but_never_ranks_it_as_a_definition() {
    let (_temp, reader) = twin_and_producer();
    let output = reader.prepare_action("SELF_STUDY FIND fill_pct").unwrap();

    assert!(
        output.text.contains("fn resolve_fill_pct"),
        "literal search should match the producer declaration: {}",
        output.text
    );
    assert!(
        !output.text.contains("Definition candidates"),
        "literal search never emits definition ranking: {}",
        output.text
    );
    assert!(
        output.text.contains("Other references"),
        "the producer declaration lands among undifferentiated references: {}",
        output.text
    );
}

#[test]
fn exact_search_on_the_full_producer_name_is_the_one_move_recovery() {
    let (_temp, reader) = twin_and_producer();
    let output = reader
        .prepare_action("SELF_STUDY RELATE resolve_fill_pct")
        .unwrap();

    assert!(
        output.text.contains("Definition candidates"),
        "the full producer name ranks as a definition: {}",
        output.text
    );
    assert!(
        output.text.contains("telemetry_port.rs 1"),
        "the definition row should be the producer declaration line: {}",
        output.text
    );
    assert!(
        output.text.contains("clamp") || output.text.contains("resolve_fill_pct(telemetry)"),
        "the recovery should expose the producing arithmetic or its call: {}",
        output.text
    );
}
