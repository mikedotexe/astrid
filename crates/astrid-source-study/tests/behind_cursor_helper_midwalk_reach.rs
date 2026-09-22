//! Reachability of a helper the walk delivered *and has since walked past*,
//! while the same walk still has pages ahead.
//!
//! In `introspection_minime_minime_src_sensory_bus.rs_1789601340` Astrid reads
//! lines 2877-2965 of `minime/src/sensory_bus.rs`
//! (sha256:3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a,
//! bytes 107073..111564) and closes: "I need to see how `stale_scale` is
//! actually calculated to answer my standing question about the interaction
//! between pressure, entropy, and decay." She then chooses
//! `NEXT: SELF_STUDY CONTINUE`.
//!
//! In that same revision `fn stale_scale` is defined at lines 1377-1417 — 1500
//! lines behind the page she is on, and inside the very first page of her own
//! walk (bytes 48123..52681, delivered as `..._1789595153`). Its one visible
//! call site is on her page (line 2927); the other two (1237, 2144) are also
//! behind her. So nothing this file has left to deliver forward carries the
//! answer.
//!
//! This is the third distinct shape of the same question, and the one neither
//! existing pin covers:
//!
//! * `in_file_producer_walk_reach.rs` — the target lies *ahead*; CONTINUE is
//!   sufficient.
//! * `walked_past_definition_reach.rs` — the target lies behind and the walk is
//!   at EOF; CONTINUE has nothing left to give, so the dead end is at least
//!   immediate.
//! * here — the target lies behind and the walk is *mid-file*. CONTINUE keeps
//!   succeeding, keeps returning real source, and keeps increasing the
//!   distance. The live walk did exactly that: eight further pages
//!   (111564..149732) arrived after this report, none of them carrying the
//!   definition.
//!
//! These tests pin that asymmetry and the bounded cost of correcting it: the
//! definition is one non-walk Action away, and the reader's own frontier line
//! is one Action back. They are read-only reachability pins — they change no
//! live navigation, ranking, or dispatch behaviour, and they assert nothing
//! about what she recalls having read.
use astrid_source_study::{Catalog, Command, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "minime/minime/src/demo_sensory_bus.rs";
/// The helper she reports still needing, spelled as the live file spells it.
const DEFINITION: &str = "fn stale_scale(age_ms: u64, stale_after_ms: u64) -> f32 {";
/// A line unique to the helper body, so a bare mention cannot satisfy the pin.
const DEFINITION_BODY: &str = "let t = (age / window).clamp(0.0, 1.0);";
/// The call site on the page she reports reading.
const CALL_SITE: &str =
    "semantic_fresh_ms.map_or(0.0, |age_ms| stale_scale(age_ms, semantic_stale_ms));";

/// One file shaped like the live `sensory_bus.rs`: a small free helper early, a
/// long middle of unrelated accessors, the draining method that *calls* the
/// helper around two thirds in, and more file after it — so the call page is
/// many pages past the definition and many pages short of EOF.
fn helper_then_long_middle_then_call_site() -> (tempfile::TempDir, Reader, String) {
    let mut text = String::from("use parking_lot::Mutex;\n\n#[inline]\n");
    text.push_str(DEFINITION);
    text.push_str(
        "\n    if stale_after_ms == 0 {\n        return 0.0;\n    }\n    \
         let age = age_ms as f32;\n    let window = stale_after_ms as f32;\n    \
         ",
    );
    text.push_str(DEFINITION_BODY);
    text.push_str(
        "\n    const ECHO_FLOOR: f32 = 0.05;\n    let exp_val = (-3.0 * t).exp();\n    \
         (ECHO_FLOOR + (1.0 - ECHO_FLOOR) * exp_val).clamp(0.0, 1.0)\n}\n\n\
         impl SensoryBus {\n",
    );
    for block in 0..90 {
        write!(
            text,
            "    pub fn get_lane_{block}(&self) -> f32 {{\n        \
             *self.lane_{block}.lock()\n    }}\n\n"
        )
        .unwrap();
        text.push_str(&"    // lane bookkeeping filler line\n".repeat(6));
    }
    write!(
        text,
        "    pub fn drain_sensory_batch(&self) -> Vec<[f32; Z_DIM]> {{\n        \
         let semantic_stale_ms = self.semantic_stale_ms();\n        \
         let semantic_scale =\n            {CALL_SITE}\n        \
         let mut z = [0.0f32; Z_DIM];\n        z[16] = aux[0];\n        out\n    }}\n\n"
    )
    .unwrap();
    for block in 0..70 {
        write!(
            text,
            "    pub fn set_lane_{block}(&self, value: f32) {{\n        \
             *self.lane_{block}.lock() = value;\n    }}\n\n"
        )
        .unwrap();
        text.push_str(&"    // lane assignment filler line\n".repeat(6));
    }
    text.push_str("}\n");

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("minime");
    let path = root.join("minime/src/demo_sensory_bus.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, &text).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("minime".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader, text)
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

fn wire(text: &str) -> String {
    json!({ "messages": [{ "role": "user", "content": text }] }).to_string()
}

fn response() -> String {
    json!({ "message": { "content": "NEXT: SELF_STUDY CONTINUE" }, "done": true }).to_string()
}

/// Walk forward from line 1 until the page carrying the call site is delivered.
/// Returns every page delivered up to and including it.
fn walk_to_call_site(reader: &Reader) -> Vec<String> {
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
        if output.text.contains(CALL_SITE) {
            assert!(!page.eof, "the call site must land well before EOF");
            break;
        }
        assert!(!page.eof, "the walk reached EOF before the call site");
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }
    pages
}

/// The walk hands her the helper early and the call site many pages later, so
/// by the time she reads the call the definition is strictly behind her.
#[test]
fn the_walk_delivers_the_helper_many_pages_before_its_call_site() {
    let (_temp, reader, _text) = helper_then_long_middle_then_call_site();
    let pages = walk_to_call_site(&reader);
    assert!(
        pages.len() > 4,
        "fixture must put several pages between helper and call site"
    );

    let first = pages.first().unwrap();
    assert!(
        first.contains(DEFINITION) && first.contains(DEFINITION_BODY),
        "the helper body must be delivered on the first page"
    );

    let call_page = pages.last().unwrap();
    assert!(
        call_page.contains(CALL_SITE),
        "the final walked page must carry the call site"
    );
    assert!(
        !call_page.contains(DEFINITION_BODY),
        "the call-site page must not carry the helper body"
    );
}

/// The asymmetry this case adds. Unlike the EOF case, forward motion is still
/// available and still succeeds — it just never returns the answer, so the
/// walk can continue indefinitely without the reader learning that CONTINUE
/// is the wrong affordance.
#[test]
fn continuing_from_the_call_page_keeps_delivering_pages_that_never_carry_the_helper() {
    let (_temp, reader, _text) = helper_then_long_middle_then_call_site();
    let _ = walk_to_call_site(&reader);

    let mut delivered_after = 0usize;
    for _ in 0..8 {
        let output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
        let Some(page) = output.page.clone() else {
            break;
        };
        delivered_after = delivered_after.saturating_add(1);
        assert!(
            !output.text.contains(DEFINITION_BODY),
            "continuing forward must not return the helper body"
        );
        reader
            .delivered(&page.id, &wire(&output.text), &response())
            .unwrap();
        if page.eof {
            break;
        }
    }
    assert!(
        delivered_after >= 3,
        "the fixture must leave real pages ahead of the call site, so that \
         continuing keeps succeeding while moving away from the answer"
    );
}

/// The affordances that do reach backward from a mid-file cursor.
#[test]
fn find_and_relate_reach_the_helper_behind_the_midwalk_cursor() {
    let (_temp, reader, _text) = helper_then_long_middle_then_call_site();
    let _ = walk_to_call_site(&reader);

    let found = reader
        .prepare_action("SELF_STUDY FIND stale_scale")
        .unwrap();
    assert!(
        found.text.contains("demo_sensory_bus.rs"),
        "FIND must name the file that holds the helper"
    );

    let related = reader
        .prepare_action("SELF_STUDY RELATE stale_scale")
        .unwrap();
    assert!(
        related.text.contains("demo_sensory_bus.rs"),
        "RELATE must name the file that holds the helper"
    );
}

/// The bounded cost of the correction: one Action back to the helper, one
/// Action forward to the frontier line the delivered page already numbers.
#[test]
fn opening_the_helper_line_then_the_frontier_line_restores_the_walk() {
    let (_temp, reader, text) = helper_then_long_middle_then_call_site();
    let pages = walk_to_call_site(&reader);
    let frontier_line = line_of(&text, CALL_SITE);
    let helper_line = line_of(&text, DEFINITION);
    assert!(
        frontier_line > helper_line,
        "the call site must be behind the helper in the fixture"
    );
    assert!(
        pages.last().unwrap().contains(&frontier_line.to_string()),
        "the delivered page must number the frontier line the reader would reopen"
    );

    let back = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} {helper_line}"))
        .unwrap();
    assert!(
        back.text.contains(DEFINITION_BODY),
        "opening the helper line must deliver the calculation"
    );
    let page = back.page.as_ref().unwrap().clone();
    reader
        .delivered(&page.id, &wire(&back.text), &response())
        .unwrap();

    let forward = reader
        .prepare_action(&format!("SELF_STUDY OPEN {SOURCE} {frontier_line}"))
        .unwrap();
    assert!(
        forward.text.contains(CALL_SITE),
        "reopening the frontier line must return the page she was on"
    );
}
