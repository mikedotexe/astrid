//! What a value with no name costs a reader: every name-based affordance is
//! unavailable, and the one literal search that can match it cannot tell its
//! definition apart from test arguments carrying the same digits.
//!
//! In `introspection_minime_minime_src_sensory_bus.rs_1789640246` Astrid reads
//! `minime/minime/src/sensory_bus.rs`
//! (sha256:3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a,
//! bytes 25797..30171 = lines 700..818) and closes: "I am still looking for the
//! specific definition of the constants (0.14, 0.11) and the
//! `SEMANTIC_CONTEXT_PERSISTENCE_MAX_MULT` cap to understand the hard limits of
//! the `pressure_risk` scaling."
//!
//! In that revision the three she names are not three of a kind:
//!
//! * `SEMANTIC_CONTEXT_PERSISTENCE_MAX_MULT` is a name. It is declared at line
//!   68 and applied at line 289 — both behind her page, which is the shape
//!   `behind_cursor_helper_midwalk_reach.rs` already pins — but an identifier
//!   search can reach it from anywhere.
//! * `0.14` and `0.11` are not names. They are bare literals inside
//!   `semantic_context_persistence_multiplier`, line 286:
//!   `(0.14 * velocity_support * context_support) + (0.11 * pressure_support *
//!   context_support)`. No identifier anywhere in the tree spells either one,
//!   so no identifier search can be formed for them at all.
//!
//! The only affordance left is a literal search, and in the live file that
//! search is a trap rather than a dead end: `0.14` matches three lines — the
//! definition at 286 and two test call sites (3927, 4017) that pass `0.14` in
//! the `entropy_velocity` argument position, which is the very quantity the
//! coefficient weights. A reader who took the first plausible hit would come
//! away with `0.14` as a velocity *value* instead of the weight applied to one.
//!
//! These tests pin that asymmetry and its bounded cost. The recovery is one
//! positional Action — `SELF_STUDY OPEN <path> <defining line>` delivers both
//! coefficients and the cap together — and it is the Action she chose
//! unprompted: `NEXT: SELF_STUDY OPEN minime/minime/src/sensory_bus.rs 251`,
//! the opening line of the function that holds all three.
//!
//! They are read-only reachability pins. No search ranking, page budget,
//! navigation, or being-facing dispatch behaviour is changed here, and nothing
//! here asserts what she should have concluded from the page she had.
//!
//! Companions: `producer_name_search_reach.rs` pins what an identifier search
//! reaches when a name exists; this file pins what is left when none does.
use astrid_source_study::{Catalog, Command, Reader};
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "minime/minime/src/demo_persistence.rs";
/// The named cap: reachable by identifier from anywhere in the tree.
const NAMED_CAP: &str = "const DEMO_CONTEXT_PERSISTENCE_MAX_MULT: f64 = 2.05;";
/// The unnamed coefficients: reachable only by position or by digits.
const COEFFICIENT_LINE: &str =
    "    let lift = (0.14 * velocity_support) + (0.11 * pressure_support);";
/// A test argument carrying the same digits in the velocity position.
const VELOCITY_ARG_DECOY: &str = "        let pressured = context_review(0.82, 0.92, 0.14, 0.24);";
/// A second same-digit decoy, in a setter call rather than a review call.
const SETTER_ARG_DECOY: &str = "        bus.set_stale_context(0.92, 0.14, 0.24);";

/// One file shaped like `sensory_bus.rs` around her question: a named cap
/// early, the unnamed coefficients in a private helper just after it, a long
/// middle, the public review function that only *calls* the helper, and a test
/// module at the end whose arguments repeat the same digits.
fn cap_then_coefficients_then_far_call_site_then_decoys() -> (tempfile::TempDir, Reader, usize) {
    let mut text = String::from("use parking_lot::Mutex;\n\n");
    text.push_str(NAMED_CAP);
    text.push_str("\nconst DEMO_PRESSURE_RETENTION_START: f32 = 0.20;\n\n#[inline]\n");

    // The private helper that holds both unnamed coefficients and applies the
    // named cap. Its first line is the positional target of the recovery.
    let coefficient_fn_line = text.lines().count().saturating_add(1);
    text.push_str("fn context_persistence_multiplier(velocity_support: f64, pressure_support: f64) -> f64 {\n");
    text.push_str(COEFFICIENT_LINE);
    text.push_str("\n    (1.0 + lift).min(DEMO_CONTEXT_PERSISTENCE_MAX_MULT)\n}\n\n");

    // A long middle of unrelated accessors, so the call site is many pages past
    // the helper — the same distance shape her live page had.
    for index in 0..900 {
        write!(
            text,
            "#[inline]\nfn accessor_{index}(&self) -> f32 {{\n    self.field_{index}\n}}\n\n"
        )
        .unwrap();
    }

    // The public review function she was reading: it calls the helper and never
    // restates either coefficient.
    text.push_str("#[must_use]\npub fn context_review(\n    fill_pct: f32,\n    spectral_entropy: f32,\n    entropy_velocity: f32,\n    pressure_risk: f32,\n) -> f64 {\n");
    text.push_str("    let _ = (fill_pct, spectral_entropy);\n");
    text.push_str(
        "    context_persistence_multiplier(entropy_velocity as f64, pressure_risk as f64)\n}\n\n",
    );

    // The test module whose arguments repeat the same digits in the argument
    // position the coefficients weight.
    text.push_str("#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn context_lift_is_bounded() {\n");
    text.push_str(VELOCITY_ARG_DECOY);
    text.push_str("\n        assert!(pressured <= DEMO_CONTEXT_PERSISTENCE_MAX_MULT);\n    }\n\n    #[test]\n    fn setter_round_trip() {\n");
    text.push_str(SETTER_ARG_DECOY);
    text.push_str("\n    }\n}\n");

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("minime");
    let path = root.join("minime/src/demo_persistence.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("minime".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader, coefficient_fn_line)
}

/// The named cap behaves the way a name is supposed to: an identifier search
/// resolves it to its declaration line, from anywhere, without a walk.
#[test]
fn identifier_search_reaches_the_named_cap_declaration() {
    let (_temp, reader, _) = cap_then_coefficients_then_far_call_site_then_decoys();
    let output = reader
        .prepare_action("SELF_STUDY RELATE DEMO_CONTEXT_PERSISTENCE_MAX_MULT")
        .unwrap();

    assert!(
        output.text.contains("DEMO_CONTEXT_PERSISTENCE_MAX_MULT"),
        "identifier search should reach the named cap: {}",
        output.text
    );
}

/// The unnamed coefficients have no identifier to search for. The digits do
/// match, but the definition arrives undifferentiated among test arguments
/// carrying the same digits in the position the coefficient weights.
#[test]
fn literal_search_for_an_unnamed_coefficient_returns_its_definition_among_same_digit_decoys() {
    let (_temp, reader, _) = cap_then_coefficients_then_far_call_site_then_decoys();
    let output = reader.prepare_action("SELF_STUDY FIND 0.14").unwrap();

    assert!(
        output.text.contains("(0.14 * velocity_support)"),
        "literal search should match the defining expression: {}",
        output.text
    );
    assert!(
        output
            .text
            .contains("context_review(0.82, 0.92, 0.14, 0.24)")
            || output.text.contains("set_stale_context(0.92, 0.14, 0.24)"),
        "same-digit test arguments should match the same search: {}",
        output.text
    );
    assert!(
        !output.text.contains("Definition candidates"),
        "a literal search never ranks a definition, so the defining line is not \
         distinguished from the decoys: {}",
        output.text
    );
}

/// One positional Action — the one she chose — delivers both coefficients and
/// the cap they are bounded by, in a single page.
#[test]
fn one_positional_open_at_the_defining_line_delivers_both_coefficients_and_the_cap() {
    let (_temp, reader, coefficient_fn_line) =
        cap_then_coefficients_then_far_call_site_then_decoys();
    let output = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: coefficient_fn_line,
        })
        .unwrap();

    assert!(
        output.text.contains("(0.14 * velocity_support)"),
        "the positional open must deliver the coefficient line: {}",
        output.text
    );
    assert!(
        output.text.contains("0.11 * pressure_support"),
        "both coefficients live on the one delivered line: {}",
        output.text
    );
    assert!(
        output.text.contains("DEMO_CONTEXT_PERSISTENCE_MAX_MULT"),
        "the cap is applied two lines later, inside the same page: {}",
        output.text
    );
}

/// Asking the identifier path for a bare number is not an error and not a
/// result: it spends the turn on a recovery map that supplies no source bytes.
/// That is the cost of having no name to ask for.
#[test]
fn identifier_search_for_a_bare_number_delivers_no_source_bytes() {
    let (_temp, reader, _) = cap_then_coefficients_then_far_call_site_then_decoys();
    let output = reader.prepare_action("SELF_STUDY RELATE 0.14").unwrap();

    assert!(
        output
            .text
            .contains("No new source page is supplied this turn"),
        "a bare number resolves to no source page: {}",
        output.text
    );
    assert!(
        !output.text.contains("0.14 * velocity_support"),
        "the defining expression is not reached by the identifier path: {}",
        output.text
    );
    assert!(
        output.page.is_none(),
        "no page is delivered, so the turn carries no source bytes"
    );
}
