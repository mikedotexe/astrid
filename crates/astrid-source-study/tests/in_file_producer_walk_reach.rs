//! Reachability of an *in-file* producer, and the premise a walk can refute.
//!
//! In `introspection_astrid_services_astrid-edge-runtime_src_reservoir.rs_1789309733`
//! Astrid reports of lines 372-480 of `services/astrid-edge-runtime/src/reservoir.rs`
//! that "the specific arithmetic for `fill_pct` is still elusive on this page",
//! suspects it lives "in a separate `update` or `compute_metrics` method ... or
//! ... further down the file", and chooses `NEXT: SELF_STUDY CONTINUE`.
//!
//! Two facts about the live file make this case differ from the already-pinned
//! cross-file one in `page_walk_producer_reach.rs`:
//!
//! 1. The producer *is* in this file. `instantaneous_fill` is computed at line
//!    709 and `"fill_pct": snapshot.fill_ratio * 100.0` renders at line 1145 —
//!    three and seven pages past where she stopped. Her chosen `CONTINUE` is
//!    sufficient here, where in `core.rs` an exhaustive walk could never reach
//!    the answer. Nothing on either page tells the reader which case she is in.
//!
//! 2. Her stated target is a data-flow edge the file does not contain. She
//!    looks for "where the `input` buffer ... is actually transformed into the
//!    `fill_ratio`". In the live source `self.input` reaches fill only through
//!    state, covariance and the eigen-spectrum: fill is
//!    `spectral_metrics.effective_modes / RESERVOIR_DIM_F32`. No arithmetic
//!    anywhere takes the input buffer to a fill value. So the walk that does
//!    deliver the producer refutes the premise rather than confirming it — the
//!    page she is owed is a correction, not a match.
//!
//! `update` and `compute_metrics` are both phantom names: neither exists in the
//! live file. That is her hypothesis, correctly labelled as one, not a citation.
//!
//! These are read-only reachability pins. They change no live navigation,
//! ranking, or dispatch behaviour.
use astrid_source_study::{Catalog, Command, InputKind, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/services/demo-edge-runtime/src/reservoir.rs";
/// The arithmetic she is hunting, exactly as the live file spells it.
const FILL_ARITHMETIC: &str = "(effective_dimensionality / RESERVOIR_DIM_F32).clamp(0.0, 1.0)";
/// The percentage render, seven pages past her stopping point in the live file.
const PCT_ARITHMETIC: &str = "snapshot.fill_ratio * 100.0";

/// One file shaped like the live `reservoir.rs`: an ingest region that scales
/// the `input` buffer and resets age ticks (the page she read), a long middle,
/// then the fill derivation and the percentage render — all in the same file.
fn ingest_then_producer() -> (tempfile::TempDir, Reader) {
    let mut text = String::new();
    text.push_str(
        "    fn ingest(&mut self, ingress: SensoryIngress) {\n        match ingress {\n            \
         SensoryIngress::Aux { features, source, availability } => {\n                \
         assign_lane(&mut self.input, AUX_OFFSET, AUX_DIM, &features);\n                \
         for value in &mut self.input[AUX_OFFSET..AUX_OFFSET + AUX_DIM] {\n                    \
         *value *= AUX_INPUT_SCALE;\n                }\n                \
         self.aux_age_ticks = 0;\n            }\n            \
         SensoryIngress::Semantic(features) => {\n                \
         assign_lane(&mut self.input, SEMANTIC_OFFSET, SEMANTIC_DIM, &features);\n                \
         for value in &mut self.input[SEMANTIC_OFFSET..SEMANTIC_OFFSET + SEMANTIC_DIM] {\n                    \
         *value *= SEMANTIC_INPUT_SCALE;\n                }\n                \
         self.semantic_age_ticks = 0;\n            }\n        }\n    }\n\n",
    );
    // A long middle, as in the live file, so the producer is several pages away.
    for block in 0..40 {
        write!(
            text,
            "    fn update_mode_continuity_{block}(&mut self) -> f32 {{\n        \
             let _ = self.semantic_age_ticks;\n        0.0\n    }}\n\n"
        )
        .unwrap();
        text.push_str(&"    // mode bookkeeping filler line\n".repeat(10));
    }
    write!(
        text,
        "    fn sample(&mut self) -> (EigenPacketV1, ReservoirSnapshot) {{\n        \
         let spectral_metrics = sanitized.metrics();\n        \
         let effective_dimensionality = spectral_metrics\n            \
         .map_or(0.0, |metrics| metrics.effective_modes as f32)\n            \
         .clamp(0.0, RESERVOIR_DIM_F32);\n        \
         let instantaneous_fill = {FILL_ARITHMETIC};\n        \
         self.fill_ema = FILL_EMA_ALPHA * instantaneous_fill\n            \
         + (1.0 - FILL_EMA_ALPHA) * self.fill_ema;\n        \
         ReservoirSnapshot {{ fill_ratio: self.fill_ema }}\n    }}\n\n"
    )
    .unwrap();
    text.push_str(&"    // export bookkeeping filler line\n".repeat(30));
    write!(
        text,
        "\nfn render(snapshot: &ReservoirSnapshot) -> Value {{\n    json!({{\n        \
         \"fill_ratio\": snapshot.fill_ratio,\n        \
         \"fill_pct\": {PCT_ARITHMETIC},\n    }})\n}}\n"
    )
    .unwrap();

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let path = root.join("services/demo-edge-runtime/src/reservoir.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn wire(text: &str) -> String {
    json!({ "messages": [{ "role": "user", "content": text }] }).to_string()
}

fn response() -> String {
    json!({ "message": { "content": "NEXT: SELF_STUDY CONTINUE" }, "done": true }).to_string()
}

/// Walk the file forward from line 1, returning each delivered page in order.
fn walk(reader: &Reader) -> Vec<String> {
    let mut pages = Vec::new();
    let mut output = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 1,
        })
        .unwrap();
    loop {
        let page = output.page.as_ref().unwrap().clone();
        pages.push(output.text.clone());
        reader
            .delivered(&page.id, &wire(&output.text), &response())
            .unwrap();
        if page.eof {
            break;
        }
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }
    pages
}

/// The contrast with the cross-file case: here `SELF_STUDY CONTINUE` is enough.
/// The page she stopped on does not carry the arithmetic, and a later page of
/// the same file does — so the walk she chose terminates in an answer.
#[test]
fn forward_walk_reaches_an_in_file_producer_the_ingest_page_withholds() {
    let (_temp, reader) = ingest_then_producer();
    let pages = walk(&reader);
    assert!(pages.len() > 2, "fixture must span several pages");

    let ingest_page = pages.first().unwrap();
    assert!(
        ingest_page.contains("*value *= AUX_INPUT_SCALE;"),
        "the first page must deliver the scaling she reports seeing"
    );
    assert!(
        !ingest_page.contains(FILL_ARITHMETIC) && !ingest_page.contains(PCT_ARITHMETIC),
        "the page she reports as elusive must carry no fill arithmetic"
    );

    let walked: String = pages.concat();
    assert!(
        walked.contains(FILL_ARITHMETIC),
        "continuing forward must reach the fill derivation in this same file"
    );
    assert!(
        walked.contains(PCT_ARITHMETIC),
        "continuing forward must reach the percentage render in this same file"
    );
}

/// The premise the walk refutes. Every `self.input` occurrence the walk
/// delivers is a lane write or a scale; the fill value is derived from the
/// spectral-metrics term instead. A reader tracing `input` toward `fill_ratio`
/// is following an edge this file does not contain.
#[test]
fn delivered_fill_arithmetic_reads_from_spectral_metrics_not_the_input_buffer() {
    let (_temp, reader) = ingest_then_producer();
    let walked: String = walk(&reader).concat();

    assert!(
        walked.contains("let effective_dimensionality = spectral_metrics"),
        "the delivered derivation must name the spectral-metrics source"
    );
    for input_free in [
        "self.input / ",
        "self.input.iter().sum",
        "fill_ratio = self.input",
        "instantaneous_fill = self.input",
    ] {
        assert!(
            !walked.contains(input_free),
            "no delivered page may carry input-buffer-to-fill arithmetic: {input_free}"
        );
    }

    // The input buffer appears only where she already saw it: lane assignment
    // and scaling inside ingest, never in the fill expression itself.
    let fill_line = walked
        .lines()
        .find(|line| line.contains(FILL_ARITHMETIC))
        .expect("the fill derivation must be delivered");
    assert!(
        !fill_line.contains("self.input"),
        "the fill expression must not read the input buffer"
    );
}

/// The affordance boundary. Her page is indistinguishable from the cross-file
/// case: nothing delivered at her stopping point states whether a producer lies
/// ahead in this file, so `CONTINUE` is a bet either way. Pinned as a recorded
/// gap, not repaired here.
#[test]
fn the_elusive_page_carries_no_indicator_that_a_producer_lies_ahead() {
    let (_temp, reader) = ingest_then_producer();
    let first = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 1,
        })
        .unwrap();
    assert_eq!(first.input_kind, InputKind::SourcePage);
    let page = first.page.as_ref().unwrap();
    assert!(!page.eof, "the ingest page must not be the last page");

    for promise in ["fill_pct", "instantaneous_fill", "fill_ratio"] {
        assert!(
            !first.text.contains(promise),
            "the ingest page must not name the value she is hunting: {promise}"
        );
    }
    // Both phantom method names she hypothesises are absent, exactly as in the
    // live file. Her suspicion is a hypothesis, and the page does not settle it.
    for phantom in ["fn update(", "fn compute_metrics("] {
        assert!(
            !first.text.contains(phantom),
            "phantom hypothesis name must not appear: {phantom}"
        );
    }
}
