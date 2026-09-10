use astrid_source_study::{Catalog, Command, InputKind, Reader, recover_local_navigation};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    process::{Command as Process, Stdio},
};

fn reader(files: &[(&str, String)]) -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

#[test]
fn evidence_roles_and_full_scan_counts_remain_visible_on_every_result_page() {
    let symbol = ["Parcel", "Relay"].concat();
    let (_temp, reader) = reader(&[
        (
            "crates/demo/src/lib.rs",
            format!(
                "pub struct {symbol};\n#[cfg(test)]\nmod checks {{ const INPUT: &str = \"{symbol}\"; }}\n"
            ),
        ),
        (
            "crates/demo/tests/navigation.rs",
            format!("let fixture = \"fn {symbol}() {{}}\";\n"),
        ),
        (
            "docs/steward-notes/review.md",
            format!("Earlier account says {symbol}.\n").repeat(80),
        ),
        (
            "docs/architecture.md",
            format!("This describes {symbol}.\n"),
        ),
        ("Cargo.toml", format!("# {symbol}\n")),
    ]);
    let first = reader
        .prepare_action(&format!("SELF_STUDY RELATE {symbol}"))
        .unwrap();
    let second = reader
        .prepare_action(&format!("SELF_STUDY RELATE {symbol} --page 2"))
        .unwrap();
    for output in [&first, &second] {
        assert_eq!(output.input_kind, InputKind::Relationships);
        assert!(
            output
                .text
                .contains("Implementation text: 1 matching lines (1 Rust), 0 path matches.")
        );
        assert!(
            output
                .text
                .contains("Test / fixture / example material: 2 matching lines (2 Rust)")
        );
        assert!(
            output
                .text
                .contains("Historical commentary: 80 matching lines (0 Rust)")
        );
        assert!(
            output
                .text
                .contains("Other documentation: 1 matching lines (0 Rust)")
        );
        assert!(
            output
                .text
                .contains("Configuration / data / interfaces: 1 matching lines (0 Rust)")
        );
        assert!(output.text.contains("include all result pages"));
        assert!(
            output
                .text
                .contains("Test data does not establish a production counterpart")
        );
    }
    assert!(
        first
            .text
            .contains("[Implementation text] SELF_STUDY OPEN astrid/crates/demo/src/lib.rs 1")
    );
    assert!(first.text.contains("[Test / fixture / example material] SELF_STUDY OPEN astrid/crates/demo/tests/navigation.rs 1"));
    assert!(
        second
            .text
            .contains("[Historical commentary] SELF_STUDY OPEN")
    );
}

#[test]
fn absent_implementation_does_not_turn_fixture_or_history_into_a_definition() {
    let query = ["parcel", "_tx"].concat();
    let candidate = ["parcels", "_tx"].concat();
    let (_temp, reader) = reader(&[
        (
            "crates/demo/src/lib.rs",
            format!(
                "pub struct Context {{ pub {candidate}: Sender }}\n// parcel and parcel_rx are different spellings.\n"
            ),
        ),
        (
            "tests/navigation.rs",
            format!("let input = \"fn {query}() {{}}\";\n"),
        ),
        (
            "docs/steward-notes/review.md",
            format!("fn {query} is an unverified recalled name.\n"),
        ),
        (
            "crates/demo/src/tests.rs",
            format!("let fixture = \"{query}\";\n"),
        ),
    ]);
    let result = reader
        .prepare_action(&format!("SELF_STUDY RELATE {query}"))
        .unwrap();
    assert!(
        result
            .text
            .contains("No implementation-text occurrence found in the scanned portion")
    );
    assert!(result.text.contains(&format!(
        "Candidate identifier text {candidate}: SELF_STUDY OPEN astrid/crates/demo/src/lib.rs 1"
    )));
    assert!(
        result
            .text
            .contains("spelling does not establish the role you intend")
    );
    assert!(!result.text.contains("Definition candidates"));
    assert!(!result.text.contains("Candidate identifier text parcel:"));
    assert!(!result.text.contains("Candidate identifier text parcel_rx:"));
    assert!(
        result
            .text
            .contains("Test / fixture / example material: 2 matching lines (2 Rust)")
    );
    let find = reader
        .prepare_action(&format!("SELF_STUDY FIND {query}"))
        .unwrap();
    assert!(
        find.text
            .contains("Implementation text: 0 matching lines (0 Rust)")
    );
    assert!(
        find.text
            .contains("Historical commentary: 1 matching lines")
    );
    assert!(!find.text.contains("Candidate identifier text"));
}

#[test]
fn literal_path_counts_and_unreadable_files_do_not_claim_complete_absence() {
    let symbol = ["quartz", "_route"].concat();
    let (temp, reader) = reader(&[("crates/demo/src/quartz.rs", "fn live() {}".into())]);
    fs::write(
        temp.path().join("astrid/crates/demo/src/broken.rs"),
        [0xff, 0xfe],
    )
    .unwrap();
    let result = reader.prepare_action("SELF_STUDY FIND quartz.rs").unwrap();
    assert!(
        result
            .text
            .contains("Implementation text: 0 matching lines (0 Rust), 1 path matches.")
    );
    assert!(result.text.contains("1 skipped; scan limit reached: false"));
    assert!(
        result
            .text
            .contains("Skipped files and any unscanned remainder are unknown")
    );
    let missing = reader
        .prepare_action(&format!("SELF_STUDY RELATE {symbol}"))
        .unwrap();
    assert!(missing.text.contains("No exact identifier matches"));
}

#[test]
fn bounded_search_reports_partial_counts_without_blocking_direct_source_access() {
    let query = ["cobalt", "_message"].concat();
    let (_temp, reader) = reader(&[(
        "crates/demo/src/lib.rs",
        format!("let {query} = 1;\n").repeat(1501),
    )]);
    let search = reader
        .prepare_action(&format!("SELF_STUDY FIND {query}"))
        .unwrap();
    assert!(
        search
            .text
            .contains("Implementation text: 1500 matching lines (1500 Rust)")
    );
    assert!(search.text.contains("scan limit reached: true"));
    assert!(search.text.contains("any unscanned remainder are unknown"));
    let source = reader
        .prepare_action("SELF_STUDY OPEN astrid/crates/demo/src/lib.rs 1501")
        .unwrap();
    assert_eq!(source.input_kind, InputKind::SourcePage);
    assert_eq!(source.page.unwrap().start.line, 1501);
}

#[test]
fn a_repeated_notebook_question_is_not_an_instruction_to_repeat_the_same_lookup() {
    let query = ["cobalt", "_handle"].concat();
    let (_temp, reader) = reader(&[("crates/demo/src/lib.rs", "fn actual() {}".into())]);
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    let request = json!({"messages":[{"role":"system","content":map.system_prompt},{"role":"user","content":map.text}]}).to_string();
    let response = json!({"message":{"content":format!("STUDY_QUESTION: What uses `{query}`?\nThe role remains a hypothesis.")},"done":true,"done_reason":"stop"}).to_string();
    reader
        .navigation_delivered(map.navigation_id.as_ref().unwrap(), &request, &response)
        .unwrap();
    let result = reader
        .prepare_action(&format!("SELF_STUDY RELATE {query}"))
        .unwrap();
    assert!(
        result
            .text
            .contains("This turn already supplies lexical results")
    );
    assert!(!result.text.contains("Optional lexical lookup for a name"));
    assert!(result.text.contains("No exact identifier matches"));
    assert!(result.text.contains("The role remains a hypothesis"));
    let map = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert!(map.text.contains("existence and meaning unverified"));
}

#[test]
fn recovery_suggestions_preserve_intent_without_inventing_a_combined_query() {
    let simple = recover_local_navigation("RELATE EnvelopeRouter").unwrap();
    assert_eq!(simple.commands, ["SELF_STUDY RELATE EnvelopeRouter"]);
    let compound = recover_local_navigation("RELATE dispatch.rs \"Route\" \"Stage\"").unwrap();
    assert_eq!(
        compound.commands,
        [
            "SELF_STUDY FIND dispatch.rs",
            "SELF_STUDY RELATE Route",
            "SELF_STUDY RELATE Stage"
        ]
    );
    assert_eq!(
        recover_local_navigation("RELATE EnvelopeRouter --page 2")
            .unwrap()
            .commands,
        ["SELF_STUDY RELATE EnvelopeRouter --page 2"]
    );
    for action in ["RELATE", "RELATE ", "SELF_STUDY RELATE"] {
        assert_eq!(
            recover_local_navigation(action).unwrap().commands,
            ["SELF_STUDY MAP"]
        );
    }
    for action in ["SEARCH dispatch.rs", "RESEARCH dispatch.rs"] {
        let recovery = recover_local_navigation(action).unwrap();
        assert_eq!(recovery.commands, ["SELF_STUDY FIND dispatch.rs"]);
        assert!(
            recovery
                .text
                .contains("No substitute command was executed or queued")
        );
        for command in recovery.commands {
            assert!(Command::parse(&command).is_ok());
        }
    }
    for action in [
        "SEARCH bird migration",
        "SEARCH https://example.com/code.rs",
        "RELATE X | OPEN secrets",
        "SELF_STUDY RELATE EnvelopeRouter",
        "SELF_STUDY RELATE EnvelopeRouter --page 2",
        "WRITE CONTINUE",
    ] {
        assert!(recover_local_navigation(action).is_none(), "{action}");
    }
}

#[test]
fn recovery_cli_requires_no_reader_roots_or_mutable_state() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Process::new(env!("CARGO_BIN_EXE_astrid-source-study"))
        .current_dir(temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"operation":"recover_navigation","action":"RELATE EnvelopeRouter"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["commands"],
        json!(["SELF_STUDY RELATE EnvelopeRouter"])
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn explicit_replace_is_one_modifier_for_ordinary_study_operations() {
    assert_eq!(
        Command::parse("SELF_STUDY REPLACE RELATE EnvelopeRouter").unwrap(),
        Command::parse("SELF_STUDY RELATE EnvelopeRouter").unwrap()
    );
    assert_eq!(
        Command::parse("SELF_STUDY REPLACE OPEN astrid/crates/demo/src/lib.rs 19").unwrap(),
        Command::Open {
            source: "astrid/crates/demo/src/lib.rs".into(),
            line: 19
        }
    );
    assert_eq!(
        Command::parse("SELF_STUDY replace continue").unwrap(),
        Command::Continue
    );
    assert_eq!(
        Command::parse("SELF_STUDY REPLACE RESUME\tastrid/crates/demo/src/lib.rs").unwrap(),
        Command::Resume {
            source: "astrid/crates/demo/src/lib.rs".into()
        }
    );
    for action in [
        "SELF_STUDY REPLACE",
        "SELF_STUDY REPLACE REPLACE MAP",
        "SELF_STUDY REPLACE WRITE CONTINUE",
        "SELF_STUDY REPLACE SELF_STUDY MAP",
        "SELF_STUDY REPLACE invented_alias",
        "SELF_STUDY REPLACE RESUME",
        "SELF_STUDY replace resume ",
        "SELF_STUDY REPLACE CONTINUE extra",
        "SELF_STUDY replace continue extra",
    ] {
        assert!(Command::parse(action).is_err(), "{action}");
    }
}
