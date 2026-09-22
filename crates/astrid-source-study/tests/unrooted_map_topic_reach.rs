use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const ROOTED_DIRECTORY: &str = "astrid/capsules/astralis/astrid-capsule-agents/src";
const UNROOTED_DIRECTORY: &str = "capsules/astralis/astrid-capsule-agents/src";
const UNROOTED_FILE: &str = "capsules/astralis/astrid-capsule-agents/src/lib.rs";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for name in [
        "capsules/astralis/README.md",
        "capsules/astralis/astrid-capsule-agents/Capsule.toml",
        "capsules/astralis/astrid-capsule-agents/src/lib.rs",
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "pub fn example() {}\n".repeat(200)).unwrap();
    }
    let minime = temp.path().join("minime");
    fs::create_dir_all(minime.join("minime_autonomy")).unwrap();
    fs::write(minime.join("minime_autonomy/runtime.py"), "value = 1\n").unwrap();
    let catalog = Catalog::new(BTreeMap::from([
        ("astrid".into(), root),
        ("minime".into(), minime),
    ]))
    .unwrap();
    let reader = Reader::new(catalog, temp.path().join("reader"));
    (temp, reader)
}

/// `OPEN` and `MAP` do not share a path namespace. `Catalog::resolve` accepts a
/// repository-relative path by trying each installed root in turn
/// (`src/catalog.rs:106-118`), so `OPEN capsules/.../lib.rs 1` delivers source. `Catalog::map`
/// matches its topic against repository-prefixed catalog IDs only
/// (`src/navigation.rs:84-97`), so the same path one directory up matches nothing — and
/// `path_candidates` returns early for any multi-segment request whose first segment is not an
/// installed repository ID (`src/path_recovery.rs:37-39`), so the recovery names no candidate
/// either. Adding the repository prefix that `OPEN` did not require is the whole fix, and it is
/// the one move the recovery does not offer.
#[test]
fn unrooted_map_topic_that_open_would_accept_gets_no_candidate_naming_its_rooted_form() {
    let (_temp, reader) = setup();

    // OPEN accepts the un-rooted repository-relative path and delivers the page.
    let opened = reader
        .prepare_action(&format!("SELF_STUDY OPEN {UNROOTED_FILE} 1"))
        .unwrap();
    assert_eq!(
        opened.page.as_ref().map(|page| page.source.as_str()),
        Some("astrid/capsules/astralis/astrid-capsule-agents/src/lib.rs"),
        "OPEN resolves a repository-relative path without a repository prefix"
    );

    // MAP of that same path's directory does not, and offers no path candidate.
    let recovery = reader
        .prepare_action(&format!("SELF_STUDY MAP {UNROOTED_DIRECTORY}"))
        .unwrap();
    assert_eq!(recovery.input_kind, InputKind::Recovery);
    assert!(recovery.page.is_none());
    assert!(
        recovery
            .text
            .contains(&format!("no catalog entries for {UNROOTED_DIRECTORY}")),
        "the recovery names the topic it could not resolve"
    );
    assert!(
        !recovery.text.contains("candidates, not opened"),
        "a multi-segment topic without an installed repository ID yields zero path candidates"
    );
    assert!(
        !recovery.text.contains(ROOTED_DIRECTORY),
        "nothing in the recovery names the rooted spelling of the failed topic"
    );

    // The rooted spelling resolves in one move and lists the file OPEN already reached.
    let chosen = reader
        .prepare_action(&format!("SELF_STUDY MAP {ROOTED_DIRECTORY}"))
        .unwrap();
    assert_eq!(chosen.input_kind, InputKind::Map);
    assert!(
        chosen
            .text
            .contains(&format!("SELF_STUDY OPEN {ROOTED_DIRECTORY}/lib.rs 1")),
        "the rooted map offers the exact file entry point"
    );
}

/// A single-segment topic recovers across repositories (`src/path_recovery.rs:36-39` allows
/// bare names), so the gap above is specific to multi-segment un-rooted topics rather than a
/// general refusal to suggest directories.
#[test]
fn bare_directory_name_still_recovers_to_its_rooted_spelling() {
    let (_temp, reader) = setup();
    let recovery = reader
        .prepare_action("SELF_STUDY MAP astrid-capsule-agents")
        .unwrap();
    assert_eq!(recovery.input_kind, InputKind::Recovery);
    assert!(
        recovery
            .text
            .contains("SELF_STUDY MAP astrid/capsules/astralis/astrid-capsule-agents"),
        "a bare directory name is offered its exact rooted map command"
    );
}
