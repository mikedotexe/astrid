//! Reachability of a definition the walk has *already delivered*.
//!
//! In `introspection_astrid_crates_astrid-approval_src_manager.rs_1789423652`
//! Astrid reads the last page of `crates/astrid-approval/src/manager.rs`
//! (lines 874-911 of 911) and closes: "I still need to see the actual
//! implementation of `check_approval` (which I suspect is in a different file
//! or a section I haven't fully mapped yet)". She then chooses
//! `NEXT: SELF_STUDY MAP`.
//!
//! In the live file at the revision her report binds
//! (sha256:cf14a499399dec6143e4e1c74d17db14b22d37f25b33062b404ee0a6b35b4b27)
//! `pub async fn check_approval` is at line 207 — inside the *second* page of
//! her own eight-page walk (bytes 4274..8631 = lines 132..253, delivered as
//! `introspection_..._1789421939`). Neither of her two hypotheses holds: it is
//! not in a different file, and it is not in an unmapped section. It is behind
//! her cursor.
//!
//! This case is the mirror of `in_file_producer_walk_reach.rs`, where the
//! producer lay *ahead* and `SELF_STUDY CONTINUE` was sufficient. Here forward
//! motion is exhausted — her page is EOF — so continuing can never return the
//! answer, and only a non-walk affordance can. These tests pin which of those
//! affordances reaches backward.
//!
//! Read-only reachability pins. They change no live navigation, ranking, or
//! dispatch behaviour, and they assert nothing about what she recalls.
use astrid_source_study::{Catalog, Command, Reader};
use serde_json::json;
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/crates/demo-approval/src/manager.rs";
/// The definition she reports still needing, exactly as the live file spells it.
const DEFINITION: &str = "pub async fn check_approval(";
/// A line unique to the concluding test page she actually read.
const TAIL_MARKER: &str = "assert!(debug.contains(\"ApprovalManager\"));";

/// One file shaped like the live `manager.rs`: the public entry point early,
/// a long implementation middle, then a `#[cfg(test)] mod tests` whose final
/// test concludes the file — so the definition is several pages behind EOF.
fn entry_point_then_tests() -> (tempfile::TempDir, Reader) {
    let mut text = String::from(
        "impl ApprovalManager {\n    /// Check whether an action is approved.\n    \
         pub async fn check_approval(\n        &self,\n        action: &SensitiveAction,\n        \
         context: impl Into<String>,\n        workspace_root: Option<&Path>,\n    \
         ) -> ApprovalOutcome {\n        \
         self.check_approval_with_lifecycle(action, context, workspace_root, None, None)\n            \
         .await\n    }\n\n",
    );
    for block in 0..40 {
        write!(
            text,
            "    async fn defer_action_{block}(&self) -> ApprovalOutcome {{\n        \
             ApprovalOutcome::Denied {{ reason: String::new() }}\n    }}\n\n"
        )
        .unwrap();
        text.push_str(&"    // allowance bookkeeping filler line\n".repeat(10));
    }
    text.push_str("}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n");
    text.push_str(&"    // test-module filler line\n".repeat(30));
    write!(
        text,
        "    #[tokio::test]\n    async fn test_debug() {{\n        \
         let manager = make_manager();\n        let debug = format!(\"{{manager:?}}\");\n        \
         {TAIL_MARKER}\n    }}\n}}\n"
    )
    .unwrap();

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let path = root.join("crates/demo-approval/src/manager.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn wire(text: &str) -> String {
    json!({ "messages": [{ "role": "user", "content": text }] }).to_string()
}

fn response() -> String {
    json!({ "message": { "content": "NEXT: SELF_STUDY CONTINUE" }, "done": true }).to_string()
}

/// Walk the file forward from line 1, returning each delivered page in order.
fn walk(reader: &Reader) -> Vec<String> {
    let mut pages = Vec::new();
    let mut output = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 1,
        })
        .unwrap();
    loop {
        let page = output.page.as_ref().unwrap().clone();
        pages.push(output.text.clone());
        reader
            .delivered(&page.id, &wire(&output.text), &response())
            .unwrap();
        if page.eof {
            break;
        }
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }
    pages
}

/// The walk delivers the definition early and the concluding tests last, so by
/// the time the reader is on the final page the answer is strictly behind.
#[test]
fn the_walk_delivers_the_definition_on_a_page_before_the_concluding_tests() {
    let (_temp, reader) = entry_point_then_tests();
    let pages = walk(&reader);
    assert!(pages.len() > 2, "fixture must span several pages");

    let first = pages.first().unwrap();
    assert!(
        first.contains(DEFINITION),
        "the entry point must be delivered on an early page"
    );

    let tail = pages.last().unwrap();
    assert!(
        tail.contains(TAIL_MARKER),
        "the last page must deliver the concluding test she reports reading"
    );
    assert!(
        !tail.contains(DEFINITION),
        "the concluding-test page must not carry the definition"
    );
}

/// Forward motion cannot answer her. The page she reports on is EOF, so
/// `SELF_STUDY CONTINUE` — sufficient in the producer-ahead case — has no
/// remaining page of this file to deliver.
#[test]
fn continuing_from_the_final_page_cannot_return_the_walked_past_definition() {
    let (_temp, reader) = entry_point_then_tests();
    let mut output = reader
        .prepare(Command::Open {
            source: SOURCE.into(),
            line: 1,
        })
        .unwrap();
    loop {
        let page = output.page.as_ref().unwrap().clone();
        reader
            .delivered(&page.id, &wire(&output.text), &response())
            .unwrap();
        if page.eof {
            assert!(
                !output.text.contains(DEFINITION),
                "the EOF page must not carry the definition"
            );
            break;
        }
        output = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    }

    // Whatever CONTINUE does past EOF — another source, a notice, or an error —
    // it does not hand back this file's definition.
    if let Ok(after_eof) = reader.prepare_action("SELF_STUDY CONTINUE") {
        assert!(
            !after_eof.text.contains(DEFINITION),
            "continuing past EOF must not deliver the walked-past definition"
        );
    }
}

/// The affordance that does reach backward. From the same EOF position, an
/// identifier search names the definition's file and line, so the answer she
/// reports still needing is one non-walk Action away.
#[test]
fn identifier_search_reaches_the_definition_behind_the_cursor() {
    let (_temp, reader) = entry_point_then_tests();
    let _ = walk(&reader);

    let related = reader
        .prepare_action("SELF_STUDY RELATE check_approval")
        .unwrap();
    assert!(
        related.text.contains("manager.rs"),
        "RELATE must name the file that holds the definition"
    );

    let found = reader
        .prepare_action("SELF_STUDY FIND check_approval")
        .unwrap();
    assert!(
        found.text.contains("manager.rs"),
        "FIND must name the file that holds the definition"
    );
}
