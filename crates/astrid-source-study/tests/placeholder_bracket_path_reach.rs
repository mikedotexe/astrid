//! Safe path spelling recovery observed in Astrid's turns of 2026-09-13.
//!
//! Provenance: `introspection_source_catalog_1789339213` (plus the identical
//! `1789338946` / `1789338769` turns). Her reader's navigation records for
//! 1789338769..1789340218 alternate between exactly two recovery reasons —
//! `"Requested source \"astrid/crates/astrid_capsule/src/engine/mcp.rs\": source
//! unavailable locally"` (three turns, **with** a candidate block) and the same
//! logical target wrapped in the placeholder brackets our own recovery text
//! models, `"<astrid/crates/astrid_capsule/src/engine/mcp.rs>"` (two turns,
//! **no** candidate block) — while her chosen Action stayed
//! `NEXT: SELF_STUDY MAP action_continuity`.
//!
//! A malformed bracket-wrapped path offers no candidate or implicit open. The
//! historical companion probe about bare MAP names was superseded by safe
//! bare-name suggestions; its unchanged original is retained in
//! docs/steward-notes/archived-reader-probes-20260915/.

use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const MCP: &str = "astrid/crates/astrid-capsule/src/engine/mcp.rs";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for name in [
        "crates/astrid-capsule/src/engine/mcp.rs",
        "crates/astrid-mcp/src/mcp.rs",
        "crates/astrid-kernel/src/engine/mcp.rs",
        "capsules/spectral-bridge/src/action_continuity/runtime/guards.rs",
        "capsules/spectral-bridge/src/action_continuity/guards.rs",
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "line\n".repeat(200)).unwrap();
    }
    let minime = temp.path().join("minime/minime/src");
    fs::create_dir_all(&minime).unwrap();
    fs::write(minime.join("esn.rs"), "line\n".repeat(20)).unwrap();
    let catalog = Catalog::new(BTreeMap::from([
        ("astrid".into(), root),
        ("minime".into(), temp.path().join("minime")),
    ]))
    .unwrap();
    let reader = Reader::new(catalog, temp.path().join("reader"));
    (temp, reader)
}

fn candidate_block(text: &str) -> Option<&str> {
    text.split("Exact catalog spelling")
        .nth(1)?
        .split("You can choose")
        .next()
}

/// A misspelled rooted path offers its exact catalog spelling first after
/// underscore/hyphen normalization. Wrapping it in `<...>` makes the reference
/// malformed: resolution fails and the safe-reference gate rejects candidates.
/// Neither request opens a suggested source implicitly.
#[test]
fn bracket_wrapped_rooted_path_loses_the_candidate_block_its_bare_twin_receives() {
    let (_temp, reader) = setup();
    let bare = "astrid/crates/astrid_capsule/src/engine/mcp.rs";

    let helped = reader
        .prepare_action(&format!("SELF_STUDY OPEN {bare} 1"))
        .unwrap();
    assert_eq!(helped.input_kind, InputKind::Recovery);
    assert!(helped.page.is_none());
    assert!(helped.text.contains("source unavailable locally"));
    let candidates = candidate_block(&helped.text).expect("bare rooted path gets candidates");
    let first = candidates
        .split_once("not opened):\n")
        .expect("candidate list header")
        .1
        .lines()
        .next()
        .expect("at least one candidate");
    assert_eq!(
        first,
        format!("SELF_STUDY OPEN {MCP} 1"),
        "the exact spelling is offered first, not merely somewhere: {candidates}"
    );

    for wrapped in [
        format!("SELF_STUDY OPEN <{bare}> 1"),
        format!("SELF_STUDY RESUME <{bare}>"),
    ] {
        let blind = reader.prepare_action(&wrapped).unwrap();
        assert_eq!(blind.input_kind, InputKind::Recovery);
        assert!(blind.page.is_none());
        assert!(
            blind.text.contains("source not found"),
            "the bracketed form fails earlier, as an unrooted relative path"
        );
        assert!(
            candidate_block(&blind.text).is_none(),
            "{wrapped}: no candidate block is emitted for a bracket-wrapped path"
        );
        assert!(
            !blind.text.contains(MCP),
            "{wrapped}: the reachable spelling appears nowhere in the recovery"
        );
    }
}
