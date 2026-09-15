//! Reachability boundary for a sequential in-file page walk.
//!
//! In `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_core.rs_1789270252`
//! Astrid reports, of a page of `action_continuity/runtime/core.rs`, that it
//! "still does not reveal the arithmetic formula for `fill_pct`", that the value
//! "is consistently appearing as a pre-calculated `f32`", and chooses
//! `NEXT: SELF_STUDY CONTINUE` to keep walking the same file forward.
//!
//! Her in-file reading is exactly right: across all 10187 lines of the live
//! `core.rs` every one of the fourteen `fill_pct` occurrences is a typed
//! parameter or a pass-through, and none is arithmetic. The producer
//! `resolve_fill_pct` lives in a different file,
//! `capsules/spectral-bridge/src/ws/telemetry_port.rs`.
//!
//! So the walk she chose is *complete and still insufficient*: finishing every
//! remaining page of the consumer file cannot deliver the answer, because the
//! answer is not in that file. These tests pin that boundary, and pin the
//! cross-file open that does reach it in one step, so the gap is a recorded
//! reachability fact rather than a surprise discovered page by page.
//!
//! This is a read-only reachability pin. It changes no live navigation,
//! ranking, or dispatch behaviour.
use astrid_source_study::{Catalog, Command, InputKind, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const CONSUMER: &str = "astrid/capsules/demo/src/action_continuity/runtime/core.rs";
const PRODUCER: &str = "astrid/capsules/demo/src/ws/telemetry_port.rs";
const ARITHMETIC: &str = "telemetry.fill_ratio * 100.0";

/// A consumer file shaped like the live `core.rs`: several pages long, with
/// `fill_pct` appearing only as a parameter or a pass-through, plus a sibling
/// file that holds the arithmetic. Deliberately spans more than one page so the
/// walk has somewhere to go.
fn consumer_and_producer() -> (tempfile::TempDir, Reader) {
    let mut consumer = String::new();
    for block in 0..24 {
        write!(consumer,
            "    pub fn record_next_event_{block}(\n        &self,\n        \
             fill_pct: f32,\n        telemetry: &SpectralTelemetry,\n    ) -> Result<()> {{\n        \
             let state = spectral_state(fill_pct, telemetry);\n        \
             self.append_proposal(&state, fill_pct)?;\n        \
             let _ = json!({{ \"fill_pct\": fill_pct }});\n        Ok(())\n    }}\n\n"
        ).unwrap();
        consumer.push_str(&"    // experiment bookkeeping filler line\n".repeat(12));
    }
    let producer = format!(
        "fn resolve_fill_pct(telemetry: &SpectralTelemetry) -> (f32, String, bool) {{\n    \
         if telemetry.fill_ratio.is_finite() {{\n        \
         (({ARITHMETIC}).clamp(0.0, 100.0), String::from(\"primary_fill_ratio\"), false)\n    \
         }} else {{\n        \
         (estimate_fill_pct(telemetry.lambda1()), String::from(\"lambda1_sigmoid_fallback\"), true)\n    \
         }}\n}}\n"
    );

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for (relative, text) in [
        (
            "capsules/demo/src/action_continuity/runtime/core.rs",
            consumer,
        ),
        ("capsules/demo/src/ws/telemetry_port.rs", producer),
    ] {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
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

/// Walk the consumer file from line 1 to end-of-file, returning every delivered
/// page's text in order.
fn walk_consumer(reader: &Reader) -> Vec<String> {
    let mut pages = Vec::new();
    let mut output = reader
        .prepare(Command::Open {
            source: CONSUMER.into(),
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

/// The walk is exhaustive over the file she is in — and that is precisely why
/// it cannot answer her question. Every occurrence of the field name is
/// delivered; the arithmetic that produces it never is.
#[test]
fn exhaustive_page_walk_of_the_consumer_never_delivers_the_producer_arithmetic() {
    let (_temp, reader) = consumer_and_producer();
    let pages = walk_consumer(&reader);

    assert!(pages.len() > 1, "fixture must span more than one page");

    let walked: String = pages.concat();
    assert!(
        walked.contains("fill_pct: f32"),
        "the walk must deliver the parameter she reports seeing"
    );
    assert!(
        walked.contains("spectral_state(fill_pct, telemetry)"),
        "the walk must deliver the pass-through she reports seeing"
    );
    assert!(
        !walked.contains(ARITHMETIC),
        "no page of the consumer file may carry the producing arithmetic"
    );
    assert!(
        !walked.contains("fn resolve_fill_pct"),
        "no page of the consumer file may carry the producer definition"
    );

    // Continuing past the end yields a navigation notice, not more source: the
    // walk is finished and the question is still open.
    let past_end = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(past_end.input_kind, InputKind::EndOfFile);
    assert!(past_end.page.is_none());
    assert!(!past_end.text.contains(ARITHMETIC));
}

/// The recovery that does reach it: naming the sibling file directly delivers
/// the arithmetic on the first page, without any further walking.
#[test]
fn opening_the_sibling_producer_file_delivers_the_arithmetic_in_one_step() {
    let (_temp, reader) = consumer_and_producer();
    walk_consumer(&reader);

    let opened = reader
        .prepare_action(&format!("SELF_STUDY OPEN {PRODUCER} 1"))
        .unwrap();
    assert_eq!(opened.input_kind, InputKind::SourcePage);
    let page = opened.page.as_ref().unwrap();
    assert_eq!(page.start.line, 1);
    assert!(
        opened.text.contains("fn resolve_fill_pct"),
        "the producer definition must arrive on the first page"
    );
    assert!(
        opened.text.contains(ARITHMETIC),
        "the arithmetic she is looking for must arrive on the first page"
    );
    assert!(
        opened.text.contains("lambda1_sigmoid_fallback"),
        "the fallback branch is part of the same answer"
    );
}

/// A cross-file jump is a distinct page identity, not a continuation of the
/// consumer walk: the reader does not silently fold the two files into one
/// forward cursor.
#[test]
fn cross_file_open_is_a_new_page_not_a_continuation_of_the_consumer_walk() {
    let (_temp, reader) = consumer_and_producer();
    let consumer_pages = walk_consumer(&reader);
    let last_consumer = consumer_pages.last().unwrap().clone();

    let opened = reader
        .prepare_action(&format!("SELF_STUDY OPEN {PRODUCER} 1"))
        .unwrap();
    let page = opened.page.as_ref().unwrap().clone();
    assert_ne!(opened.text, last_consumer);
    assert_eq!(page.start.byte, 0);

    reader
        .delivered(&page.id, &wire(&opened.text), &response())
        .unwrap();
    let after = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert!(
        !after.text.contains("record_next_event_0"),
        "continuing after the jump must not snap back into the consumer file"
    );
}
