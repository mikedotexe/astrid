use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const KERNEL_DIRECTORY: &str = "astrid/crates/astrid-kernel/src";
const ENTRY_POINT: &str = "astrid/crates/astrid-kernel/src/lib.rs";
const SIBLING: &str = "astrid/crates/astrid-kernel/src/socket_bridge.rs";

/// The `kernel` component's declared entry points (`catalog.toml:63-66`) plus the sibling
/// file Astrid named as her next step.
fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for name in [
        "crates/astrid-kernel/src/lib.rs",
        "crates/astrid-kernel/src/socket.rs",
        "crates/astrid-kernel/src/socket_bridge.rs",
        "crates/astrid-capsule/src/lib.rs",
        "wit/astrid-capsule.wit",
        "capsules/spectral-bridge/src/lifecycle.rs",
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

/// A component map is a curated entry-point list, not a directory listing. `Catalog::map`
/// takes the component branch for a bare component ID (`src/navigation.rs:42-55`) and emits
/// only `component.sources`, so a sibling file in the same directory — here
/// `socket_bridge.rs`, the file Astrid named as her next step in
/// `introspection_source_catalog_1789589587` — is absent from `MAP kernel`. The branch does
/// derive each entry point's directory (`src/navigation.rs:48-55`), so the sibling is exactly
/// one further `MAP` away, and that hop is the only thing standing between the curated list
/// and the rest of the directory.
#[test]
fn component_map_omits_entry_point_sibling_but_offers_its_directory_in_one_hop() {
    let (_temp, reader) = setup();

    let component = reader.prepare_action("SELF_STUDY MAP kernel").unwrap();
    assert_eq!(component.input_kind, InputKind::Map);
    assert!(
        component
            .text
            .contains("Kernel, capabilities and lifecycle"),
        "the component map names the component it rendered"
    );
    assert!(
        component
            .text
            .contains(&format!("SELF_STUDY OPEN {ENTRY_POINT} 1")),
        "the component map offers its declared entry point"
    );
    assert!(
        !component.text.contains("socket_bridge.rs"),
        "a sibling of an entry point is not itself an entry point"
    );
    assert!(
        component
            .text
            .contains(&format!("SELF_STUDY MAP {KERNEL_DIRECTORY}")),
        "the component map offers the entry point's directory as the next hop"
    );

    let directory = reader
        .prepare_action(&format!("SELF_STUDY MAP {KERNEL_DIRECTORY}"))
        .unwrap();
    assert_eq!(directory.input_kind, InputKind::Map);
    assert!(
        directory
            .text
            .contains(&format!("SELF_STUDY OPEN {SIBLING} 1")),
        "one hop from the component map reaches the sibling by its exact OPEN command"
    );
}

/// `MAP` and `LIST` do not share a topic namespace either. `Catalog::map` resolves a component
/// ID through its own branch, but `Catalog::list` only prefix-matches catalog IDs
/// (`src/navigation.rs:70-80`), so the same word that maps cleanly lists nothing. The
/// component map correctly withholds a `LIST` offer for that reason — it passes
/// `scoped = false` (`src/navigation.rs:56-60`), which suppresses the recursive-scope line
/// that scoped directory maps print (`src/navigation.rs:176-181`).
#[test]
fn component_id_maps_but_does_not_list_and_the_component_map_offers_no_list() {
    let (_temp, reader) = setup();

    let component = reader.prepare_action("SELF_STUDY MAP kernel").unwrap();
    assert!(
        !component.text.contains("SELF_STUDY LIST kernel"),
        "the component map does not offer a LIST its topic cannot satisfy"
    );

    let listed = reader.prepare_action("SELF_STUDY LIST kernel").unwrap();
    assert_eq!(listed.input_kind, InputKind::Recovery);
    assert!(listed.page.is_none());
    assert!(
        listed.text.contains("no catalog entries for kernel"),
        "the recovery names the topic LIST could not resolve: {}",
        listed.text
    );

    let directory = reader
        .prepare_action(&format!("SELF_STUDY MAP {KERNEL_DIRECTORY}"))
        .unwrap();
    assert!(
        directory
            .text
            .contains(&format!("SELF_STUDY LIST {KERNEL_DIRECTORY}")),
        "a scoped directory map does offer its recursive LIST"
    );
}
