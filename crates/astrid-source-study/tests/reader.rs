use astrid_source_study::{Catalog, Command, MAX_PAGE_BYTES, Reader};
use serde_json::json;
use std::fmt::Write as _;
use std::{collections::BTreeMap, fs, path::Path};
fn setup(text: &str) -> (tempfile::TempDir, Catalog, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src/state")).unwrap();
    fs::write(root.join("crates/example/src/state/credentials.rs"), text).unwrap();
    let catalog = Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap();
    let reader = Reader::new(catalog.clone(), temp.path().join("reader"));
    (temp, catalog, reader)
}
const SOURCE: &str = "astrid/crates/example/src/state/credentials.rs";
fn open() -> Command {
    Command::Open {
        source: SOURCE.into(),
        line: 1,
    }
}
fn wire(text: &str) -> String {
    json!({"messages":[{"role":"user","content":text}]}).to_string()
}
fn response() -> String {
    json!({"message":{"content":"NEXT: SELF_STUDY CONTINUE"},"done":true}).to_string()
}
#[test]
fn prepare_and_failed_delivery_do_not_advance() {
    let (_temp, _, reader) = setup(&"hello\n".repeat(3000));
    let first = reader.prepare(open()).unwrap().page.unwrap();
    assert_eq!(
        reader.prepare(Command::Continue).unwrap().page.unwrap(),
        first
    );
    assert!(
        reader
            .delivered(&first.id, &wire("trimmed source"), &response())
            .is_err()
    );
    assert!(
        reader
            .delivered(
                &first.id,
                &wire(&first.text),
                r#"{"message":{"content":"partial"},"done":false}"#
            )
            .is_err()
    );
    assert_eq!(
        reader.prepare(Command::Continue).unwrap().page.unwrap(),
        first
    );
}
#[test]
fn every_byte_of_long_utf8_file_is_reachable_and_eof_stops() {
    let text = format!(
        "{}\n{}",
        "🦀".repeat(10000),
        (0..1200).fold(String::new(), |mut text, n| {
            writeln!(text, "row {n}").unwrap();
            text
        })
    );
    let (_temp, _, reader) = setup(&text);
    let mut page = reader.prepare(open()).unwrap().page.unwrap();
    let mut end = 0;
    loop {
        assert_eq!(page.start.byte, end);
        assert!(page.text.len() <= MAX_PAGE_BYTES);
        assert!(text.is_char_boundary(page.end.byte));
        end = page.end.byte;
        let receipt = reader
            .delivered(&page.id, &wire(&page.text), &response())
            .unwrap();
        assert!(receipt.artifact_path.is_file());
        let next = reader.prepare(Command::Continue).unwrap();
        if page.eof {
            assert!(next.page.is_none());
            break;
        }
        page = next.page.unwrap();
    }
    assert_eq!(end, text.len());
}
#[test]
fn source_changes_are_explicit_and_restart_keeps_pending() {
    let (temp, catalog, reader) = setup(&"before\n".repeat(3000));
    let first = reader.prepare(open()).unwrap().page.unwrap();
    let restarted = Reader::new(catalog.clone(), temp.path().join("reader"));
    assert_eq!(
        restarted.prepare(Command::Continue).unwrap().page.unwrap(),
        first
    );
    reader
        .delivered(&first.id, &wire(&first.text), &response())
        .unwrap();
    fs::write(
        catalog.resolve(SOURCE).unwrap().path,
        "changed\n".repeat(3000),
    )
    .unwrap();
    assert!(
        restarted
            .prepare(Command::Continue)
            .unwrap_err()
            .to_string()
            .contains("source changed")
    );
    assert_ne!(
        restarted.prepare(open()).unwrap().page.unwrap().revision,
        first.revision
    );
}
#[test]
fn catalog_opens_mechanism_code_but_blocks_private_and_symlink_escape() {
    let (temp, catalog, _) = setup("fn secret_key() {}\n");
    assert!(catalog.resolve(SOURCE).is_ok());
    assert!(catalog.resolve("credentials.rs").is_err());
    assert!(catalog.resolve("astrid/../secret.rs").is_err());
    assert!(
        catalog
            .resolve("astrid/capsules/example/workspace/private.txt")
            .is_err()
    );
    #[cfg(unix)]
    {
        let outside = temp.path().join("outside.rs");
        fs::write(&outside, "private").unwrap();
        let link = catalog
            .resolve(SOURCE)
            .unwrap()
            .path
            .with_file_name("escape.rs");
        std::os::unix::fs::symlink(outside, link).unwrap();
        assert!(
            catalog
                .resolve("astrid/crates/example/src/state/escape.rs")
                .is_err()
        );
    }
    let credentials = catalog
        .resolve(SOURCE)
        .unwrap()
        .path
        .with_file_name("credentials.toml");
    fs::write(&credentials, "token = 'fixture-private'\n").unwrap();
    assert!(
        catalog
            .resolve("astrid/crates/example/src/state/credentials.toml")
            .is_err()
    );
    assert_eq!(catalog.sources().unwrap().len(), 1);
}
#[test]
fn search_and_map_share_the_open_catalog_without_marking_pages() {
    let (_temp, _, reader) = setup("fn special_needle() {}\n");
    let result = reader
        .prepare(Command::Find {
            query: "special_needle".into(),
            page: 1,
        })
        .unwrap();
    assert!(result.text.contains(&format!("OPEN {SOURCE} 1")));
    assert!(result.page.is_none());
    let map = reader
        .prepare(Command::Map {
            topic: "astrid/crates/example".into(),
            page: 1,
        })
        .unwrap();
    assert!(map.text.contains(SOURCE));
    assert!(
        reader
            .prepare(Command::Continue)
            .unwrap()
            .text
            .contains("Shared system map")
    );
}
#[test]
fn cli_and_library_have_identical_catalog_and_page() {
    use std::io::Write as _;
    use std::process::{Command as Process, Stdio};
    let (temp, _, reader) = setup("fn parity() {}\n");
    let expected = reader.prepare(open()).unwrap();
    let mut child = Process::new(env!("CARGO_BIN_EXE_astrid-source-study"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"roots":{"astrid":temp.path().join("astrid")},"state_directory":temp.path().join("cli"),"operation":"prepare","action":format!("SELF_STUDY OPEN {SOURCE}")});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, serde_json::to_value(expected).unwrap());
    assert!(Path::new(&temp.path().join("cli/reader-v1.json")).is_file());
}

#[test]
fn durable_receipt_recovers_checkpoint_crash_and_explicit_reread_is_fresh() {
    let (temp, catalog, reader) = setup(&"row\n".repeat(3000));
    let first = reader.prepare(open()).unwrap().page.unwrap();
    let checkpoint = temp.path().join("reader/reader-v1.json");
    let before = fs::read(&checkpoint).unwrap();
    reader
        .delivered(&first.id, &wire(&first.text), &response())
        .unwrap();
    // Simulate the receipt reaching disk immediately before a checkpoint crash.
    fs::write(&checkpoint, before).unwrap();
    let restarted = Reader::new(catalog, temp.path().join("reader"));
    let next = restarted.prepare(Command::Continue).unwrap().page.unwrap();
    assert_eq!(next.start, first.end);
    let reread = restarted.prepare(open()).unwrap().page.unwrap();
    assert_eq!(reread.start, first.start);
    assert_ne!(reread.id, first.id);
}

#[test]
fn catalog_covers_kernel_interfaces_shaders_build_files_and_sibling_implementations() {
    let temp = tempfile::tempdir().unwrap();
    let ids = [
        "astrid",
        "minime",
        "reservoir",
        "prime-esn",
        "rascii",
        "channel",
    ];
    let roots = ids
        .into_iter()
        .map(|id| (id.into(), temp.path().join(id)))
        .collect::<BTreeMap<_, _>>();
    let examples = [
        "astrid/crates/astrid-kernel/src/lib.rs",
        "astrid/capsules/spectral-bridge/tests/contracts.rs",
        "astrid/wit/astrid-capsule.wit",
        "astrid/Cargo.toml",
        "astrid/CLAUDE.md",
        "astrid/.cargo/config.toml",
        "astrid/.github/workflows/ci.yml",
        "astrid/docs/architecture/runtime.md",
        "astrid/packages/client/src/index.ts",
        "minime/minime/src/runtime.rs",
        "minime/minime/shaders/esn.metal",
        "minime/minime/build.rs",
        "minime/minime_autonomy/runtime.py",
        "minime/md-chapters/architecture.md",
        "reservoir/mlx_reservoir.py",
        "reservoir/persistence.py",
        "reservoir/launchd/runtime.plist",
        "prime-esn/src/lib.rs",
        "rascii/src/lib.rs",
        "channel/channel.py",
    ];
    for id in examples {
        let (repo, path) = id.split_once('/').unwrap();
        let path = roots[repo].join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "fixture\n").unwrap();
    }
    let catalog = Catalog::new(roots).unwrap();
    assert_eq!(catalog.sources().unwrap().len(), examples.len());
    for id in examples {
        assert_eq!(catalog.resolve(id).unwrap().id, id);
    }
}

#[test]
fn changed_revision_at_eof_is_not_silently_treated_as_finished() {
    let (_temp, catalog, reader) = setup("short\n");
    let first = reader.prepare(open()).unwrap().page.unwrap();
    assert!(first.eof);
    reader
        .delivered(&first.id, &wire(&first.text), &response())
        .unwrap();
    fs::write(
        catalog.resolve(SOURCE).unwrap().path,
        "short\nnew mechanism\n",
    )
    .unwrap();
    assert!(
        reader
            .prepare(Command::Continue)
            .unwrap_err()
            .to_string()
            .contains("source changed")
    );
    assert!(
        reader
            .prepare(Command::Resume {
                source: SOURCE.into()
            })
            .is_err()
    );
}

fn map_source(reader: &Reader) -> String {
    reader
        .prepare(Command::Map {
            topic: SOURCE.into(),
            page: 1,
        })
        .unwrap()
        .text
}

#[test]
fn maps_distinguish_pending_partial_complete_reread_and_changed_revision() {
    let (_temp, catalog, reader) = setup(&"row\n".repeat(3000));
    let mut page = reader.prepare(open()).unwrap().page.unwrap();
    assert!(map_source(&reader).contains("Not delivered"));
    reader
        .delivered(&page.id, &wire(&page.text), &response())
        .unwrap();
    let map = map_source(&reader);
    assert!(map.contains("Partial delivery"));
    assert!(map.contains(&format!("SELF_STUDY RESUME {SOURCE}")));
    while !page.eof {
        page = reader.prepare(Command::Continue).unwrap().page.unwrap();
        reader
            .delivered(&page.id, &wire(&page.text), &response())
            .unwrap();
    }
    assert!(map_source(&reader).contains("Deliberate reread — Complete delivery"));
    let reread = reader.prepare(open()).unwrap().page.unwrap();
    reader
        .delivered(&reread.id, &wire(&reread.text), &response())
        .unwrap();
    assert!(map_source(&reader).contains("Complete delivery"));
    fs::write(catalog.resolve(SOURCE).unwrap().path, "changed\n").unwrap();
    assert!(map_source(&reader).contains("Source changed or unreadable; previous revision"));
}

#[test]
fn opening_only_the_final_lines_is_not_complete_coverage() {
    let (_temp, _, reader) = setup(&"row\n".repeat(3000));
    let page = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 2999,
        })
        .unwrap()
        .page
        .unwrap();
    assert!(page.eof);
    reader
        .delivered(&page.id, &wire(&page.text), &response())
        .unwrap();
    let map = map_source(&reader);
    assert!(map.contains("Read missing earlier bytes — Partial delivery"));
    assert!(!map.contains("Complete delivery"));
}

#[test]
fn verified_notes_survive_restart_navigation_and_continuation_only() {
    let (temp, catalog, reader) = setup(&"row\n".repeat(3000));
    let page = reader.prepare(open()).unwrap().page.unwrap();
    let reply = json!({"message":{"content":"The first page defines the bus.\nSTUDY_NOTE: Follow the bus across modules.\nSTUDY_QUESTION: Who subscribes?\nNEXT: SELF_STUDY CONTINUE"},"done":true}).to_string();
    assert!(
        reader
            .delivered(&page.id, &wire("shortened"), &reply)
            .is_err()
    );
    assert!(!map_source(&reader).contains("Who subscribes?"));
    reader
        .delivered(&page.id, &wire(&page.text), &reply)
        .unwrap();
    let restarted = Reader::new(catalog, temp.path().join("reader"));
    let next = restarted.prepare(Command::Continue).unwrap();
    assert!(next.text.contains("Who subscribes?"));
    assert!(next.text.contains(&page.revision.sha256));
    assert!(!next.system_prompt.contains("Who subscribes?"));
    let map = restarted
        .prepare(Command::Map {
            topic: SOURCE.into(),
            page: 1,
        })
        .unwrap();
    let map_reply = json!({"choices":[{"message":{"content":"STUDY_QUESTION: Where is the receiver?\nNEXT: SELF_STUDY CONTINUE"},"finish_reason":"stop"}]}).to_string();
    let id = map.navigation_id.as_ref().unwrap();
    assert!(
        restarted
            .navigation_delivered(id, &wire("trimmed"), &map_reply)
            .is_err()
    );
    let receipt = restarted
        .navigation_delivered(id, &wire(&map.text), &map_reply)
        .unwrap();
    assert!(receipt.artifact_path.is_file());
    let resumed = restarted.prepare(Command::Continue).unwrap();
    assert_eq!(resumed.page, next.page);
    assert!(resumed.text.contains("Where is the receiver?"));
    assert!(resumed.text.contains("Follow the bus across modules."));
    let page = resumed.page.unwrap();
    restarted
        .delivered(&page.id, &wire(&resumed.text), &response())
        .unwrap();
    let after = map_source(&restarted);
    assert!(after.contains("Where is the receiver?"));
    assert!(after.contains("The first page defines the bus."));
}

#[test]
fn notes_are_bounded_visible_optional_and_clearable() {
    let (_temp, _, reader) = setup("short\n");
    let page = reader.prepare(open()).unwrap().page.unwrap();
    let reply = json!({"message":{"content":format!("<think>hidden deliberation</think>\nSTUDY_NOTE: {}\nSTUDY_QUESTION: why?\nNEXT: SELF_STUDY MAP", "🦀".repeat(3000))},"done":true}).to_string();
    reader
        .delivered(&page.id, &wire(&page.text), &reply)
        .unwrap();
    let map = reader
        .prepare(Command::Map {
            topic: SOURCE.into(),
            page: 1,
        })
        .unwrap();
    assert!(!map.text.contains("hidden deliberation"));
    assert!(map.text.contains("excerpt truncated"));
    assert!(map.text.len() + map.system_prompt.len() < 16000);
    let clear = json!({"message":{"content":"STUDY_NOTE: -\nSTUDY_QUESTION: -\nNEXT: SELF_STUDY CONTINUE"},"done":true}).to_string();
    reader
        .navigation_delivered(
            map.navigation_id.as_ref().unwrap(),
            &wire(&map.text),
            &clear,
        )
        .unwrap();
    let next = map_source(&reader);
    assert!(!next.contains("🦀"));
    assert!(!next.contains("why?"));
}

#[test]
fn legacy_shared_receipts_migrate_coverage_and_last_words_without_resetting_pending() {
    let (temp, catalog, reader) = setup(&"row\n".repeat(3000));
    let first = reader.prepare(open()).unwrap().page.unwrap();
    let reply =
        json!({"message":{"content":"I want to trace the event bus."},"done":true}).to_string();
    reader
        .delivered(&first.id, &wire(&first.text), &reply)
        .unwrap();
    let pending = reader.prepare(Command::Continue).unwrap().page.unwrap();
    let checkpoint = temp.path().join("reader/reader-v1.json");
    let mut old: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    for field in [
        "progress",
        "notebook",
        "pending_navigation",
        "last_navigation",
    ] {
        old.as_object_mut().unwrap().remove(field);
    }
    fs::write(&checkpoint, serde_json::to_vec(&old).unwrap()).unwrap();
    let restarted = Reader::new(catalog, temp.path().join("reader"));
    let next = restarted.prepare(Command::Continue).unwrap();
    assert_eq!(next.page.unwrap(), pending);
    assert!(next.text.contains("I want to trace the event bus."));
    assert!(map_source(&restarted).contains("Partial delivery"));
}

#[test]
fn no_match_search_names_literal_punctuation_and_navigation_does_not_advance() {
    let (_temp, _, reader) = setup("fn connection_closed() {}\n");
    let miss = reader
        .prepare(Command::Find {
            query: "connection_closed;".into(),
            page: 1,
        })
        .unwrap();
    assert!(
        miss.text
            .contains("No matches for the exact literal query \"connection_closed;\"")
    );
    assert!(miss.text.contains("Punctuation is part of the query"));
    let hit = reader
        .prepare(Command::Find {
            query: "connection_closed".into(),
            page: 1,
        })
        .unwrap();
    assert!(!hit.text.contains("No matches"));
    assert!(hit.text.contains(&format!("OPEN {SOURCE} 1")));
    assert!(
        reader
            .navigation_delivered(
                miss.navigation_id.as_ref().unwrap(),
                &wire(&miss.text),
                &response()
            )
            .is_err()
    );
    reader
        .navigation_delivered(
            hit.navigation_id.as_ref().unwrap(),
            &wire(&hit.text),
            &response(),
        )
        .unwrap();
    assert!(map_source(&reader).contains("Not delivered"));
}

#[test]
fn navigation_receipt_recovers_after_checkpoint_crash_and_rejects_changed_replay() {
    let (temp, catalog, reader) = setup("short\n");
    let offered = reader
        .prepare(Command::Map {
            topic: SOURCE.into(),
            page: 1,
        })
        .unwrap();
    let checkpoint = temp.path().join("reader/reader-v1.json");
    let before = fs::read(&checkpoint).unwrap();
    let reply = json!({"message":{"content":"STUDY_NOTE: Follow this module.\nNEXT: SELF_STUDY CONTINUE"},"done":true}).to_string();
    reader
        .navigation_delivered(
            offered.navigation_id.as_ref().unwrap(),
            &wire(&offered.text),
            &reply,
        )
        .unwrap();
    assert!(
        reader
            .navigation_delivered(
                offered.navigation_id.as_ref().unwrap(),
                &wire(&offered.text),
                &response()
            )
            .is_err()
    );
    fs::write(&checkpoint, before).unwrap();
    let restarted = Reader::new(catalog, temp.path().join("reader"));
    assert!(map_source(&restarted).contains("Follow this module."));
    assert!(map_source(&restarted).contains("Not delivered"));
}

#[test]
fn complete_offer_verification_includes_notebook_and_protects_navigation() {
    let (_temp, _, reader) = setup("short\n");
    let first = reader.prepare(open()).unwrap();
    let reply =
        json!({"message":{"content":"STUDY_QUESTION: Who calls this?"},"done":true}).to_string();
    reader
        .delivered(&first.page.unwrap().id, &wire(&first.text), &reply)
        .unwrap();
    let next = reader.prepare(open()).unwrap();
    assert!(next.verify_delivery(&wire(&next.text), &response()).is_ok());
    assert!(
        next.verify_delivery(&wire(&next.page.as_ref().unwrap().text), &response())
            .is_err()
    );
    let map = reader
        .prepare(Command::Map {
            topic: SOURCE.into(),
            page: 1,
        })
        .unwrap();
    assert!(map.verify_delivery(&wire(&map.text), &response()).is_ok());
    assert!(
        map.verify_delivery(&wire("map without notebook"), &response())
            .is_err()
    );
    assert!(
        map.verify_delivery(
            &wire(&map.text),
            r#"{"message":{"content":"partial"},"done":false}"#
        )
        .is_err()
    );
}
