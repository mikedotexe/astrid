use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

fn fixture() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for file in [
        "crates/astrid-kernel/src/keep_name/lib.rs",
        "crates/dual_name/src/exact.rs",
        "crates/dual-name/src/other.rs",
        "workspace/private-name/lib.rs",
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "pub fn example() {}\n".repeat(1000)).unwrap();
    }
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap();
    let reader = Reader::new(catalog, temp.path().join("reader"));
    (temp, reader)
}

#[test]
fn observed_underscore_map_lists_the_exact_directory_without_moving_the_bookmark() {
    let (_temp, reader) = fixture();
    let page = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/astrid-kernel/src/keep_name/lib.rs 1")
        .unwrap();
    for verb in ["MAP", "LIST"] {
        let action = format!("SELF_STUDY {verb} astrid/crates/astrid_kernel/src/keep_name/");
        let output = reader.prepare_action(&action).unwrap();
        assert_eq!(output.input_kind, InputKind::Map);
        assert!(output.page.is_none());
        assert!(output.text.contains("Directory spelling"));
        assert!(
            output
                .text
                .contains("astrid/crates/astrid_kernel/src/keep_name")
        );
        assert!(
            output
                .text
                .contains("SELF_STUDY OPEN astrid/crates/astrid-kernel/src/keep_name/lib.rs 1")
        );
        assert_eq!(
            reader.prepare_action("SELF_STUDY CONTINUE").unwrap().page,
            page.page
        );
    }
}

#[test]
fn exact_directory_wins_and_invalid_or_private_requests_remain_recovery() {
    let (_temp, reader) = fixture();
    let exact = reader
        .prepare_action("SELF_STUDY MAP astrid/crates/dual_name/src")
        .unwrap();
    assert_eq!(exact.input_kind, InputKind::Map);
    assert!(exact.text.contains("exact.rs"));
    assert!(!exact.text.contains("Directory spelling"));
    assert!(!exact.text.contains("other.rs"));
    for target in [
        "astrid/../astrid/crates/astrid_kernel/src",
        "astrid/workspace/private_name",
        "astrid/crates/astrid_kernel/missing",
        "unknown/crates/astrid_kernel/src",
    ] {
        let output = reader
            .prepare_action(&format!("SELF_STUDY MAP {target}"))
            .unwrap();
        assert_eq!(output.input_kind, InputKind::Recovery);
        assert!(output.page.is_none());
        assert!(!output.text.contains("Directory spelling"));
    }
}
