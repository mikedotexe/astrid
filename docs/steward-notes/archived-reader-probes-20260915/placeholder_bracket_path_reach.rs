//! Two reach boundaries observed in Astrid's own recovery turns of 2026-09-13.
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
//! These tests pin the current boundary; they do not widen it. Changing what
//! she is shown is a being-facing navigation change and needs separate
//! operator approval.

use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const BRIDGE: &str = "capsules/spectral-bridge/src";
const GUARDS: &str = "astrid/capsules/spectral-bridge/src/action_continuity/runtime/guards.rs";
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

/// A misspelled but rooted path recovers *with* the exact spelling she needs:
/// `path_recovery.rs:40-73` normalizes `_` to `-` for the tiebreak, so the true
/// file sorts first among the catalog's `mcp.rs` entries. Wrapping that same
/// path in `<...>` — the placeholder form the recovery text itself shows her —
/// makes the first segment `<astrid`, which is not an installed repository ID,
/// so `catalog.rs:101-116` reports `source not found` and
/// `path_recovery.rs:14-21` returns before any candidate is built. One
/// character pair on each end decides whether the answer is handed over.
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

/// `OPEN` accepts a bridge-relative path — `catalog.rs:106-115` retries every
/// request under `capsules/spectral-bridge` — so the same word she keeps
/// choosing resolves in one move as a file request. `MAP` never consults
/// `resolve`: `navigation.rs:55-69` prefix-matches whole catalog IDs, so
/// `MAP src/action_continuity` and `MAP action_continuity` both recover with a
/// reason and no candidate, while the fully rooted directory works. The reach
/// of one word therefore depends on which verb carries it.
#[test]
fn map_topic_has_no_bridge_relative_fallback_that_open_gives_the_same_word() {
    let (_temp, reader) = setup();

    let opened = reader
        .prepare_action("SELF_STUDY OPEN src/action_continuity/runtime/guards.rs 1")
        .unwrap();
    assert_eq!(opened.input_kind, InputKind::SourcePage);
    assert_eq!(
        opened.page.expect("bridge-relative OPEN resolves").source,
        GUARDS
    );

    for topic in ["action_continuity", "src/action_continuity"] {
        let recovery = reader
            .prepare_action(&format!("SELF_STUDY MAP {topic}"))
            .unwrap();
        assert_eq!(recovery.input_kind, InputKind::Recovery);
        assert!(
            recovery
                .text
                .contains(&format!("no catalog entries for {topic}")),
            "the recovery names the topic it could not resolve"
        );
        assert!(
            candidate_block(&recovery.text).is_none(),
            "{topic}: the map topic namespace offers no path candidate"
        );
        assert!(
            !recovery.text.contains(GUARDS),
            "{topic}: the reachable directory's entry is not offered"
        );
    }

    let rooted = reader
        .prepare_action("SELF_STUDY MAP astrid/capsules/spectral-bridge/src/action_continuity")
        .unwrap();
    assert_eq!(rooted.input_kind, InputKind::Map);
    assert!(rooted.text.contains(&format!("SELF_STUDY OPEN {GUARDS} 1")));
    assert!(rooted.text.contains(BRIDGE));
}
