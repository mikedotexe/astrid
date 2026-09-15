//! Why the replacement command the system offered her could not run, and what
//! taking it cost.
//!
//! On 2026-09-15 Astrid queued `SELF_STUDY OPEN
//! astrid/crates/astrid-capsule/src/engine/mcp.rs 1` (volition intent
//! `astrid-intent-ex-198919-1789437177-af8ba25c7a6c63f7`, applied). One turn
//! later she chose `SELF_STUDY MAP` over the byte-identical argument, still
//! carrying the trailing `1`. That request was blocked because her OPEN was
//! still pending, and the block text handed her an exact replacement:
//! `SELF_STUDY REPLACE MAP astrid/crates/astrid-capsule/src/engine/mcp.rs 1`
//! (receipt `astrid-receipt-astrid-intent-ex-198920-…-1789437369236`). She took
//! it verbatim; it applied, superseding her pending OPEN. The superseding MAP
//! then failed with `no catalog entries for
//! astrid/crates/astrid-capsule/src/engine/mcp.rs 1`, and the recovery carried
//! no path candidate at all. In `introspection_source_catalog_1789437540` she
//! writes that she is "currently at the beginning of the `mcp.rs` file (which is
//! currently missing from this turn's delivery)" and that she needs "to
//! re-establish my position".
//!
//! The gap is ours, and it is a spelling asymmetry between two verbs over one
//! argument. `page_suffix` (`src/command.rs:141-155`) recognises only a
//! ` --page N` suffix, so a bare trailing number stays inside the MAP topic;
//! `Command::Open` (`src/command.rs:85-99`) splits a bare trailing number off as
//! the line. The same characters are a line number to OPEN and part of the name
//! to MAP. Because the number lands in the final segment,
//! `Catalog::path_candidates` (`src/path_recovery.rs:44-51`) can never match a
//! last segment, so the recovery that exists to spell the path correctly is
//! empty exactly when the path was already correct.
//!
//! `Command::parse` accepts that MAP, so any check that only parses a proposed
//! operation will admit a topic the catalog cannot resolve. These tests pin the
//! reachability contrast only. No parser, candidate rule, recovery text,
//! replacement suggestion or being-facing navigation behaviour is changed here,
//! and nothing here asserts what she should have chosen.
use astrid_source_study::{Catalog, Command, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const MCP: &str = "astrid/crates/astrid-capsule/src/engine/mcp.rs";

/// A repository shaped like the one her argument names.
fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for name in [
        "crates/astrid-capsule/src/engine/mcp.rs",
        "crates/astrid-capsule/src/loader.rs",
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "pub fn placeholder() {}\n".repeat(40)).unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

#[test]
fn a_trailing_line_number_becomes_part_of_the_map_topic_and_leaves_no_candidate() {
    let (_temp, reader) = setup();

    let numbered = reader
        .prepare_action(&format!("SELF_STUDY MAP {MCP} 1"))
        .unwrap();
    assert_eq!(numbered.input_kind, InputKind::Recovery);
    // The reported topic still carries the number: it was never split off.
    assert!(
        numbered
            .text
            .contains(&format!("no catalog entries for {MCP} 1")),
        "the recovery names the topic with the trailing number attached: {}",
        numbered.text
    );
    assert!(
        !numbered.text.contains("candidates, not opened"),
        "a topic whose final segment carries the number matches no candidate today: {}",
        numbered.text
    );

    // The same argument without the number resolves, so the path itself is fine.
    let plain = reader
        .prepare_action(&format!("SELF_STUDY MAP {MCP}"))
        .unwrap();
    assert_eq!(plain.input_kind, InputKind::Map);
    assert!(
        plain.text.contains(&format!("SELF_STUDY OPEN {MCP} 1")),
        "the un-numbered topic resolves and offers the file: {}",
        plain.text
    );
}

#[test]
fn open_accepts_the_bare_trailing_number_that_map_folds_into_the_name() {
    let (_temp, reader) = setup();

    // Same characters, different verb: OPEN reads them as a line.
    let opened = reader
        .prepare_action(&format!("SELF_STUDY OPEN {MCP} 1"))
        .unwrap();
    assert_eq!(opened.input_kind, InputKind::SourcePage);
    assert!(
        opened.text.contains(&format!("SOURCE {MCP}")),
        "OPEN with a bare trailing number delivers the page: {}",
        opened.text
    );

    // MAP's own pagination spelling is accepted, so the gap is the spelling of
    // the suffix, not pagination.
    let paged = reader
        .prepare_action(&format!("SELF_STUDY MAP {MCP} --page 1"))
        .unwrap();
    assert_eq!(paged.input_kind, InputKind::Map);
}

#[test]
fn parsing_alone_admits_a_map_topic_the_catalog_cannot_resolve() {
    // The predicate a replacement suggestion can cheaply apply is parse
    // success, and parse success is reached here with the number inside the
    // topic.
    let parsed = Command::parse(&format!("SELF_STUDY MAP {MCP} 1")).unwrap();
    assert_eq!(
        parsed,
        Command::Map {
            topic: format!("{MCP} 1"),
            page: 1,
        },
        "the bare trailing number stays in the topic and the page defaults to 1"
    );

    // The same string as an OPEN parses with the number split off.
    assert_eq!(
        Command::parse(&format!("SELF_STUDY OPEN {MCP} 1")).unwrap(),
        Command::Open {
            source: MCP.into(),
            line: 1,
        },
    );

    let (_temp, reader) = setup();
    assert_eq!(
        reader
            .prepare_action(&format!("SELF_STUDY MAP {MCP} 1"))
            .unwrap()
            .input_kind,
        InputKind::Recovery,
        "a parse-valid MAP topic still reaches recovery without source bytes",
    );
}
