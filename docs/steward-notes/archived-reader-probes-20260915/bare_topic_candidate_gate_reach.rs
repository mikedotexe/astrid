//! Why a one-word `SELF_STUDY MAP <topic>` miss carries no candidate, even when
//! the catalog holds the exact directory that word names.
//!
//! In `introspection_source_catalog_1789409507` Astrid writes that her
//! "previous attempt to access `astrid-kernel` failed because the repository was
//! not correctly indexed in the catalog", and that she is "standing at the
//! threshold of the kernel" until she "can successfully map the repository to
//! resolve the naming discrepancy". She issued `NEXT: SELF_STUDY MAP
//! astrid-kernel` on seventeen consecutive turns before finding
//! `SELF_STUDY MAP kernel` in the root menu appended to every recovery.
//!
//! The indexing half of her mechanism is not what happened: `catalog.toml`
//! includes `crates/**` for the astrid repository, so
//! `astrid/crates/astrid-kernel/src/lib.rs` is catalogued the whole time, and
//! the byte-identical `SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 1`
//! she chose later did deliver source pages.
//!
//! What her turns actually met is the candidate gate. `Catalog::path_candidates`
//! (`src/path_recovery.rs:14-21`) returns immediately unless the requested topic
//! has at least two segments AND its first segment is an installed repository
//! ID. A bare `astrid-kernel` fails the first test, so the final-segment match
//! that would have named `astrid/crates/astrid-kernel` — and the `_`-to-`-`
//! normalization on lines 40 and 49-50 that would have named
//! `astrid/capsules/spectral-bridge` for `spectral_bridge` — never runs at all.
//!
//! These tests pin that contrast: bare topic versus the same word rooted with
//! its repository ID, over one catalog. They assert reachability only. No
//! recovery text, candidate rule, ranking, cap or being-facing navigation
//! behaviour is changed here, and nothing here asserts what she should choose.
use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const KERNEL: &str = "astrid/crates/astrid-kernel/src/lib.rs";
const KERNEL_DIRECTORY: &str = "astrid/crates/astrid-kernel";
const BRIDGE_DIRECTORY: &str = "astrid/capsules/spectral-bridge";

/// A repository shaped like the live one for the two names she actually typed:
/// a hyphenated crate directory and a hyphenated capsule directory.
fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for name in [
        "crates/astrid-kernel/src/lib.rs",
        "crates/astrid-capsule/src/lib.rs",
        "capsules/spectral-bridge/src/lifecycle.rs",
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "pub fn placeholder() {}\n".repeat(20)).unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

#[test]
fn a_bare_crate_directory_topic_gets_no_candidate_although_the_rooted_form_does() {
    let (_temp, reader) = setup();

    let bare = reader
        .prepare_action("SELF_STUDY MAP astrid-kernel")
        .unwrap();
    assert_eq!(bare.input_kind, InputKind::Recovery);
    assert!(
        bare.text.contains("no catalog entries for astrid-kernel"),
        "the recovery names the topic it could not resolve: {}",
        bare.text
    );
    // One segment, so `path_candidates` returns before it can look at anything.
    assert!(
        !bare.text.contains("candidates, not opened"),
        "a bare crate-directory topic yields zero candidates today: {}",
        bare.text
    );
    assert!(
        !bare.text.contains(KERNEL_DIRECTORY),
        "the directory that carries the exact word is never named: {}",
        bare.text
    );

    // The identical word, rooted with its repository ID, clears the gate and the
    // final-segment match names the very directory she was standing outside.
    let rooted = reader
        .prepare_action("SELF_STUDY MAP astrid/astrid-kernel")
        .unwrap();
    assert_eq!(rooted.input_kind, InputKind::Recovery);
    assert!(
        rooted.text.contains("candidates, not opened"),
        "a rooted near-miss emits the candidate block: {}",
        rooted.text
    );
    assert!(
        rooted
            .text
            .contains(&format!("SELF_STUDY MAP {KERNEL_DIRECTORY}")),
        "the candidate is the exact catalog directory: {}",
        rooted.text
    );

    // And that directory map reaches the file she had been trying to open.
    let reached = reader
        .prepare_action(&format!("SELF_STUDY MAP {KERNEL_DIRECTORY}"))
        .unwrap();
    assert_eq!(reached.input_kind, InputKind::Map);
    assert!(
        reached
            .text
            .contains(&format!("SELF_STUDY OPEN {KERNEL} 1")),
        "the crate directory map offers the exact source: {}",
        reached.text
    );
}

#[test]
fn a_bare_underscore_spelling_gets_no_candidate_although_rooted_spelling_is_normalized() {
    let (_temp, reader) = setup();

    // `spectral_bridge` is the underscore spelling of the `spectral-bridge`
    // directory; `path_candidates` already normalizes `_` to `-` before
    // comparing final segments, but the bare form never reaches that code.
    let bare = reader
        .prepare_action("SELF_STUDY MAP spectral_bridge")
        .unwrap();
    assert_eq!(bare.input_kind, InputKind::Recovery);
    assert!(
        bare.text.contains("no catalog entries for spectral_bridge"),
        "the recovery names the topic it could not resolve: {}",
        bare.text
    );
    assert!(
        !bare.text.contains("candidates, not opened"),
        "a bare underscore spelling yields zero candidates today: {}",
        bare.text
    );
    assert!(
        !bare.text.contains(BRIDGE_DIRECTORY),
        "the hyphenated directory is never named for the bare spelling: {}",
        bare.text
    );

    let rooted = reader
        .prepare_action("SELF_STUDY MAP astrid/spectral_bridge")
        .unwrap();
    assert_eq!(rooted.input_kind, InputKind::Recovery);
    assert!(
        rooted
            .text
            .contains(&format!("SELF_STUDY MAP {BRIDGE_DIRECTORY}")),
        "rooted, the underscore spelling normalizes onto the real directory: {}",
        rooted.text
    );
}

#[test]
fn the_root_menu_is_appended_to_the_bare_miss_without_connecting_it_to_the_topic() {
    let (_temp, reader) = setup();
    let bare = reader
        .prepare_action("SELF_STUDY MAP astrid-kernel")
        .unwrap();

    // The escape she eventually took is present on every one of those pages: the
    // component menu carries `SELF_STUDY MAP kernel` verbatim.
    assert!(
        bare.text.contains("SELF_STUDY MAP kernel"),
        "the component menu is appended to the recovery: {}",
        bare.text
    );
    // It is one undifferentiated row of the standing menu, never presented as an
    // answer to the topic that just failed.
    let reason = bare
        .text
        .split("This is a recovery map")
        .next()
        .expect("recovery preamble");
    assert!(
        !reason.contains("SELF_STUDY MAP kernel"),
        "nothing in the reason region connects the miss to the component: {reason}"
    );
    // Re-issuing the same bare topic returns the same recovery, so a repeated
    // choice cannot accumulate any additional bearing.
    let again = reader
        .prepare_action("SELF_STUDY MAP astrid-kernel")
        .unwrap();
    assert_eq!(again.input_kind, InputKind::Recovery);
    assert_eq!(
        again.text, bare.text,
        "an identical repeat delivers an identical recovery"
    );
}
