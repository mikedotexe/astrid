//! Reachability boundary for the enclosing scope of a delivered source page.
//!
//! In `introspection_source_catalog_1789393728` Astrid reads
//! `capsules/spectral-bridge/src/autonomous/btsp/lab.rs` lines 1506-1634 and
//! writes that the page "serves as the operational 'logbook' and registration
//! layer for the causal lab", and that "the `entry_for` function is the primary
//! way the system initializes a `BTSPCausalExperimentV3`".
//!
//! Every mechanism she names on that page is exact. The attribution is not:
//! lines 1506-1634 sit entirely inside `#[cfg(test)] mod tests {`, opened at
//! lines 1433-1434 — seventy-three lines above where her page begins. The
//! production constructor is `causal_lab_entry_for` at line 291; `entry_for` at
//! line 1551 is a test fixture that delegates to it.
//!
//! The page as rendered by `Page::read_with_budget` carries the source id, the
//! revision, the page identity, the byte interval, numbered lines and a
//! navigation footer. It carries no enclosing scope. So a bare `fn` helper that
//! appears *above* the page's first `#[test]` attribute reaches her with
//! nothing in its delivered bytes marking it as test-only code.
//!
//! These tests pin that boundary, and pin the walk that does reach the
//! distinction, so the gap is a recorded reachability fact rather than a
//! reading error attributed to her.
//!
//! This is a read-only reachability pin. It changes no live navigation,
//! ranking, rendering, or dispatch behaviour.
use astrid_source_study::{Catalog, Command, InputKind, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/capsules/demo/src/autonomous/btsp/lab.rs";
const PRODUCTION_FN: &str = "fn causal_lab_entry_for(";
const HELPER_FN: &str = "fn entry_for(";
const CONSENT_MODE: &str = "study_counter_refusal_or_new_evidence_required";

/// A file shaped like the live `lab.rs`: a production constructor that requires
/// both an active anti-loop and a replay, a long production body, then a
/// `#[cfg(test)] mod tests` whose bare helpers precede its first `#[test]`.
/// The filler is sized so the `mod tests` opening and the helper land on
/// different pages, which is the condition her page met.
fn lab_like_source() -> (tempfile::TempDir, Reader) {
    let mut text = String::new();
    text.push_str(
        "pub(super) struct BTSPCausalExperimentV3 {\n    pub consent_mode: String,\n}\n\n",
    );
    let _ = write!(
        text,
        "{PRODUCTION_FN}\n    signal_fingerprint: &str,\n    \
         replay_read: Option<&BTSPReplaySummaryV2>,\n    \
         anti_loop_state: Option<&BTSPAntiLoopState>,\n) -> Option<BTSPCausalExperimentV3> {{\n    \
         let anti_loop = anti_loop_state?;\n    if !anti_loop.active {{\n        return None;\n    }}\n    \
         let replay = replay_read?;\n    let _ = (signal_fingerprint, replay);\n    \
         Some(BTSPCausalExperimentV3 {{\n        \
         consent_mode: \"{CONSENT_MODE}\".to_string(),\n    }})\n}}\n\n"
    );
    for block in 0..18 {
        let _ = write!(
            text,
            "fn production_helper_{block}(entry: &BTSPCausalExperimentV3) -> bool {{\n    \
             !entry.consent_mode.is_empty()\n}}\n\n"
        );
        text.push_str(&"// production bookkeeping filler line\n".repeat(10));
    }
    text.push_str("#[cfg(test)]\nmod tests {\n    use super::*;\n\n");
    // Filler inside the test module, so the module opening falls on an earlier
    // page than the helper — exactly the 1433 / 1551 separation she was given.
    for block in 0..12 {
        let _ = write!(
            text,
            "    fn fixture_filler_{block}() -> u64 {{\n        {block}\n    }}\n\n"
        );
        text.push_str(&"    // fixture bookkeeping filler line\n".repeat(10));
    }
    let _ = write!(
        text,
        "    {HELPER_FN}scope: &str, fingerprint: &str) -> BTSPCausalExperimentV3 {{\n        \
         causal_lab_entry_for(fingerprint, Some(&replay(scope)), Some(&anti_loop(scope)))\n            \
         .expect(\"causal lab entry\")\n    }}\n\n"
    );
    text.push_str(
        "    #[test]\n    fn causal_lab_entry_preregisters_similar_holdout() {\n        \
         let entry = entry_for(\"similar\", \"fp\");\n        \
         assert_eq!(entry.consent_mode, \"study_counter_refusal_or_new_evidence_required\");\n    }\n}\n",
    );

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let path = root.join("capsules/demo/src/autonomous/btsp/lab.rs");
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

/// Walk the file from line 1 to end of file, returning every delivered page in
/// order.
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

fn index_of(pages: &[String], needle: &str) -> usize {
    pages
        .iter()
        .position(|page| page.contains(needle))
        .unwrap_or_else(|| panic!("fixture must deliver {needle:?} on some page"))
}

/// The page that carries the test fixture carries no marker that it is inside a
/// test module: not the `#[cfg(test)]` attribute, not the `mod tests` opening,
/// and nothing in the page header or footer naming an enclosing scope.
#[test]
fn the_page_carrying_a_test_fixture_states_no_enclosing_scope() {
    let (_temp, reader) = lab_like_source();
    let pages = walk(&reader);

    let helper_page = index_of(&pages, HELPER_FN);
    let module_page = index_of(&pages, "mod tests {");
    assert!(
        module_page < helper_page,
        "fixture must separate the module opening from the helper, as lab.rs 1433 vs 1551 does"
    );

    let page = &pages[helper_page];
    assert!(
        !page.contains("#[cfg(test)]"),
        "the helper's page must not carry the cfg attribute"
    );
    assert!(
        !page.contains("mod tests {"),
        "the helper's page must not carry the module opening"
    );
    assert!(
        !page.to_lowercase().contains("enclosing"),
        "no rendered field names the enclosing scope"
    );
    // What the page *does* state about itself: identity, revision, interval.
    assert!(page.contains("SOURCE astrid/capsules/demo/src/autonomous/btsp/lab.rs"));
    assert!(page.contains("Exact source bytes"));
    assert!(page.contains("Navigation: SELF_STUDY CONTINUE"));
}

/// The sharper case: a bare `fn` helper delivered *above* the page's first
/// `#[test]` attribute has nothing at all distinguishing it from production
/// code — which is the exact shape of `entry_for` at lab.rs:1551 against the
/// first `#[test]` on her page at 1563.
#[test]
fn a_helper_above_the_pages_first_test_attribute_is_unmarked() {
    let (_temp, reader) = lab_like_source();
    let pages = walk(&reader);
    let page = pages[index_of(&pages, HELPER_FN)].clone();

    let helper_at = page.find(HELPER_FN).unwrap();
    let first_test_attr = page
        .find("#[test]")
        .expect("fixture page must also carry the first test attribute");
    assert!(
        helper_at < first_test_attr,
        "the helper must be delivered before any #[test] marker on its page"
    );

    let before_first_test = &page[..first_test_attr];
    assert!(
        !before_first_test.contains("#[test]"),
        "no earlier attribute may mark the helper region"
    );
    assert!(
        !before_first_test.contains("#[cfg(test)]"),
        "no cfg attribute may mark the helper region"
    );
}

/// The distinction is reachable, but only by holding two pages at once: the
/// production constructor is delivered on a strictly earlier page of the same
/// file, and a literal search reaches both definitions in one step.
#[test]
fn production_constructor_and_test_fixture_arrive_on_different_pages() {
    let (_temp, reader) = lab_like_source();
    let pages = walk(&reader);

    let production_page = index_of(&pages, PRODUCTION_FN);
    let helper_page = index_of(&pages, HELPER_FN);
    assert!(
        production_page < helper_page,
        "the production constructor must precede the fixture, as lab.rs 291 vs 1551 does"
    );
    assert!(
        !pages[helper_page].contains(PRODUCTION_FN),
        "the fixture's page must not also carry the production constructor"
    );
    assert!(
        !pages[production_page].contains(HELPER_FN),
        "the production constructor's page must not also carry the fixture"
    );

    // Her mechanism reading is on the production side and is exact there: an
    // entry requires an active anti-loop and a replay, and carries the consent
    // mode she quoted.
    let production = &pages[production_page];
    assert!(production.contains("let anti_loop = anti_loop_state?;"));
    assert!(production.contains("if !anti_loop.active {"));
    assert!(production.contains("let replay = replay_read?;"));
    assert!(production.contains(CONSENT_MODE));

    let found = reader.prepare_action("SELF_STUDY FIND entry_for").unwrap();
    assert_eq!(found.input_kind, InputKind::Search);
    assert!(found.page.is_none(), "search supplies no new source page");
    assert!(
        found.text.contains("causal_lab_entry_for"),
        "a literal search reaches the production name the page did not distinguish"
    );
}
