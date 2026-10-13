//! Synthetic public-source fixtures only. No runtime or live journal access.
use astrid_source_study::{
    Catalog, InputKind, JournalByteSpan, JournalPublish, JournalReceipt, JournalRegistration,
    JournalReserve, JournalSourceKind, Reader, StudyOutput,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn hash(value: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(value.as_ref()))
}

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    owner: String,
}

impl Fixture {
    fn new(owner: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("workspace/journal")).unwrap();
        Self {
            _temp: temp,
            root,
            owner: owner.into(),
        }
    }

    fn reader(&self) -> Reader {
        Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), self.root.clone())])).unwrap(),
            self.root.join("reader"),
        )
        .with_runtime_workspace(self.root.join("workspace"), &self.owner)
    }

    fn source(&self, name: &str) -> PathBuf {
        self.root.join("workspace/journal").join(name)
    }

    fn reserve_request(&self, operation: &str) -> JournalRegistration {
        JournalRegistration::Reserve(JournalReserve {
            owner: self.owner.clone(),
            operation_id: operation.into(),
            source_kind: JournalSourceKind::AuthoredPublicJournal,
        })
    }

    fn register(&self, host_request: &str, request: &JournalRegistration) -> JournalReceipt {
        let reader = self.reader();
        let result = reader
            .prepare_journal_once(
                host_request,
                &reader.preparation_revision().unwrap(),
                request,
            )
            .unwrap();
        assert!(!result.generation_requested);
        serde_json::from_str(&result.text).unwrap()
    }

    fn reserve(&self, operation: &str) -> JournalReceipt {
        self.register(
            &format!("reserve-{operation}"),
            &self.reserve_request(operation),
        )
    }

    fn publish_request(&self, operation: &str, id: &str, response: &str) -> JournalPublish {
        let source_path = self.source(&format!("entry_{operation}.txt"));
        let prefix = format!(
            "=== {} JOURNAL ===\nMode: journal\n\n",
            self.owner.to_ascii_uppercase()
        );
        let text = format!("{prefix}{response}\n");
        fs::write(&source_path, &text).unwrap();
        JournalPublish {
            owner: self.owner.clone(),
            operation_id: operation.into(),
            entry_id: id.into(),
            source_path,
            file_sha256: hash(&text),
            response_sha256: hash(response),
            authored_response: response.into(),
            response_span: Some(JournalByteSpan {
                start: prefix.len(),
                end: prefix.len().saturating_add(response.len()),
            }),
        }
    }

    fn published(&self, operation: &str, response: &str) -> (JournalReceipt, JournalPublish) {
        let reserved = self.reserve(operation);
        let request = self.publish_request(operation, &reserved.entry_id, response);
        let receipt = self.register(
            &format!("publish-{operation}"),
            &JournalRegistration::Publish(request.clone()),
        );
        (receipt, request)
    }

    fn prepare(&self, host_request: &str, action: &str) -> anyhow::Result<StudyOutput> {
        let reader = self.reader();
        reader.prepare_once(host_request, &reader.preparation_revision()?, action)
    }

    fn drafts(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root.join("reader/writing/drafts-v2.json")).unwrap())
            .unwrap()
    }

    fn native_bytes(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn collect(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
            if !path.exists() {
                return;
            }
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    collect(root, &path, entries);
                } else if entry.file_type().unwrap().is_file() && entry.file_name() != "reader.lock"
                {
                    entries.insert(
                        path.strip_prefix(root).unwrap().into(),
                        fs::read(&path).unwrap(),
                    );
                }
            }
        }
        let root = self.root.join("reader");
        let mut entries = BTreeMap::new();
        collect(&root, &root, &mut entries);
        entries
    }

    fn edit_registry(&self, edit: impl FnOnce(&mut Value)) {
        let path = self.root.join("reader/journal-sources-v1.json");
        let mut envelope: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let mut body: Value = serde_json::from_str(envelope["body"].as_str().unwrap()).unwrap();
        edit(&mut body);
        let body = serde_json::to_string(&body).unwrap();
        envelope["sha256"] = hash(&body).into();
        envelope["body"] = body.into();
        fs::write(path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    }
}

#[test]
fn reservation_is_preallocated_but_unavailable_until_publication() {
    let fixture = Fixture::new("astrid");
    let reader = fixture.reader();
    let old_revision = reader.preparation_revision().unwrap();
    let receipt = fixture.reserve("stable-host-job");
    assert!(!receipt.published);
    assert!(receipt.file_sha256.is_none());
    assert_eq!(receipt, fixture.reserve("stable-host-job"));
    assert_ne!(old_revision, reader.preparation_revision().unwrap());
    assert!(
        reader
            .prepare_once("stale", &old_revision, "WRITE START not admitted")
            .is_err()
    );
    let listing = reader.prepare_action("WRITE JOURNALS").unwrap();
    assert!(!listing.text.contains(&receipt.entry_id));
    assert!(
        reader
            .prepare_action(&format!("WRITE JOURNAL_READ {}", receipt.entry_id))
            .is_err()
    );
    let before = fixture.native_bytes();
    assert!(
        fixture
            .prepare(
                "unpublished",
                &format!("WRITE FROM_JOURNAL {}", receipt.entry_id)
            )
            .is_err()
    );
    assert_eq!(before, fixture.native_bytes());
    assert!(fixture.drafts()["drafts"].as_object().unwrap().is_empty());
}

#[test]
fn exact_transfer_survives_restart_retry_and_appends_only_new_prose() {
    for owner in ["astrid", "minime"] {
        let fixture = Fixture::new(owner);
        let authored = "A complete public thought.\n\nA second paragraph.\nNEXT: REST";
        let (receipt, publication) = fixture.published("one", authored);
        let original = fs::read(&publication.source_path).unwrap();
        let action = format!("WRITE FROM_JOURNAL {}", receipt.entry_id);
        assert!(fixture.reader().prepare_action(&action).is_err());
        let output = fixture.prepare("private-transfer", &action).unwrap();
        assert_eq!(output.input_kind, InputKind::PrivateWriting);
        assert!(output.generation_requested);
        let retry = fixture
            .reader()
            .prepare_once("private-transfer", "stale-but-already-committed", &action)
            .unwrap();
        assert_eq!(output, retry);
        assert_eq!(fixture.drafts()["drafts"].as_object().unwrap().len(), 1);
        assert_eq!(
            fixture.drafts()["drafts"]["d1"]["parts"][0],
            "A complete public thought.\n\nA second paragraph."
        );
        assert!(
            fixture.drafts()["drafts"]["d1"]["evidence"]
                .as_str()
                .unwrap()
                .contains(&receipt.entry_id)
        );
        let writer = astrid_source_study::writing::Writer::new(fixture.root.join("reader/writing"));
        let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
        let response = json!({"message":{"content":"A genuinely new private passage.\nNEXT: WRITE PARK"},"done":true}).to_string();
        writer
            .delivered(
                output.navigation_id.as_deref().unwrap(),
                &request,
                &response,
            )
            .unwrap();
        assert_eq!(
            fixture.drafts()["drafts"]["d1"]["parts"][1],
            "A genuinely new private passage."
        );
        assert_eq!(fs::read(&publication.source_path).unwrap(), original);
        let registry =
            fs::read_to_string(fixture.root.join("reader/journal-sources-v1.json")).unwrap();
        assert!(!registry.contains("A complete public thought"));
        assert!(!registry.contains("A genuinely new private passage"));
    }
}

#[test]
fn registration_requires_host_preparation_and_rejects_nonpublic_kinds() {
    let fixture = Fixture::new("astrid");
    assert!(
        fixture
            .reader()
            .register_journal(&fixture.reserve_request("no-transaction"))
            .is_err()
    );
    for kind in [
        JournalSourceKind::PeerMirror,
        JournalSourceKind::RuntimeNotice,
        JournalSourceKind::PrivateWriting,
    ] {
        let request = JournalRegistration::Reserve(JournalReserve {
            owner: "astrid".into(),
            operation_id: "not-public".into(),
            source_kind: kind,
        });
        let reader = fixture.reader();
        let before = fixture.native_bytes();
        assert!(
            reader
                .prepare_journal_once("reject", &reader.preparation_revision().unwrap(), &request)
                .is_err()
        );
        assert_eq!(before, fixture.native_bytes());
    }
}

#[test]
fn duplicate_operations_are_idempotent_but_conflicts_and_reused_paths_fail() {
    let fixture = Fixture::new("astrid");
    let (receipt, publication) = fixture.published("one", "Exact original.");
    assert_eq!(
        receipt,
        fixture.register(
            "independent-publish-retry",
            &JournalRegistration::Publish(publication.clone())
        )
    );
    let second = fixture.reserve("two");
    let mut collision = publication.clone();
    collision.operation_id = "two".into();
    collision.entry_id = second.entry_id;
    let reader = fixture.reader();
    let before = fixture.native_bytes();
    assert!(
        reader
            .prepare_journal_once(
                "reused-path",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(collision)
            )
            .is_err()
    );
    let mut conflict = publication;
    conflict.source_path = fixture.source("different.txt");
    fs::write(&conflict.source_path, "Exact original.").unwrap();
    conflict.file_sha256 = hash("Exact original.");
    conflict.response_span = None;
    assert!(
        reader
            .prepare_journal_once(
                "different-path",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(conflict)
            )
            .is_err()
    );
    assert_eq!(before, fixture.native_bytes());
}

#[test]
fn missing_failed_or_wrong_identity_publication_never_unlocks_entry() {
    let fixture = Fixture::new("astrid");
    let receipt = fixture.reserve("pending");
    let publication = fixture.publish_request("pending", &receipt.entry_id, "Bound response.");
    for failure in [
        "file-hash",
        "response-hash",
        "identity",
        "operation",
        "missing",
        "span",
    ] {
        let mut request = publication.clone();
        match failure {
            "file-hash" => request.file_sha256 = "0".repeat(64),
            "response-hash" => request.response_sha256 = "0".repeat(64),
            "identity" => request.entry_id = "0".repeat(64),
            "operation" => request.operation_id = "other".into(),
            "missing" => request.source_path = fixture.source("absent.txt"),
            "span" => request.response_span = Some(JournalByteSpan { start: 0, end: 1 }),
            _ => unreachable!(),
        }
        let reader = fixture.reader();
        let before = fixture.native_bytes();
        assert!(
            reader
                .prepare_journal_once(
                    failure,
                    &reader.preparation_revision().unwrap(),
                    &JournalRegistration::Publish(request)
                )
                .is_err()
        );
        assert_eq!(before, fixture.native_bytes());
        assert!(
            reader
                .prepare_action(&format!("WRITE JOURNAL_READ {}", receipt.entry_id))
                .is_err()
        );
    }
}

#[test]
fn ambiguous_occurrences_require_an_exact_host_span() {
    let fixture = Fixture::new("astrid");
    let receipt = fixture.reserve("repeated");
    let mut request = fixture.publish_request("repeated", &receipt.entry_id, "Repeated text.");
    let text = "Repeated text.\nRepeated text.\n";
    fs::write(&request.source_path, text).unwrap();
    request.file_sha256 = hash(text);
    request.response_span = None;
    let reader = fixture.reader();
    assert!(
        reader
            .prepare_journal_once(
                "ambiguous",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(request.clone())
            )
            .is_err()
    );
    request.response_span = Some(JournalByteSpan { start: 15, end: 29 });
    fixture.register("exact-boundary", &JournalRegistration::Publish(request));
    fixture
        .prepare(
            "transfer",
            &format!("WRITE FROM_JOURNAL {}", receipt.entry_id),
        )
        .unwrap();
    assert_eq!(
        fixture.drafts()["drafts"]["d1"]["parts"][0],
        "Repeated text."
    );
}

#[test]
fn inferred_boundaries_reject_inline_substrings_but_exact_spans_are_authoritative() {
    let fixture = Fixture::new("astrid");
    let receipt = fixture.reserve("inline");
    let mut request = fixture.publish_request("inline", &receipt.entry_id, "body");
    fs::write(&request.source_path, "prefix body suffix").unwrap();
    request.file_sha256 = hash("prefix body suffix");
    request.response_span = None;
    let reader = fixture.reader();
    assert!(
        reader
            .prepare_journal_once(
                "inline-reject",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(request.clone())
            )
            .is_err()
    );
    request.response_span = Some(JournalByteSpan { start: 7, end: 11 });
    fixture.register("inline-exact", &JournalRegistration::Publish(request));
    assert!(
        reader
            .prepare_action(&format!("WRITE JOURNAL_READ {}", receipt.entry_id))
            .unwrap()
            .text
            .contains("\n\nbody\n\n")
    );
}

#[test]
fn inferred_unique_complete_line_response_is_accepted() {
    let fixture = Fixture::new("minime");
    let receipt = fixture.reserve("unique");
    let mut request = fixture.publish_request("unique", &receipt.entry_id, "One unique response.");
    request.response_span = None;
    fixture.register("unique-publish", &JournalRegistration::Publish(request));
    assert!(
        fixture
            .reader()
            .prepare_action(&format!("WRITE JOURNAL_READ {}", receipt.entry_id))
            .unwrap()
            .text
            .contains("\n\nOne unique response.\n\n")
    );
}

#[test]
fn selected_passages_use_visible_utf8_boundaries_without_shortening() {
    let fixture = Fixture::new("astrid");
    let (receipt, _) = fixture.published("utf8", "A\u{e9}\u{1f642}Z\nNEXT: REST");
    let before = fixture.native_bytes();
    for range in [
        "1 2", "2 3", "4 7", "7 7", "8 2", "0 999", "-1 2", "UTF8 1 3",
    ] {
        assert!(
            fixture
                .prepare(
                    range,
                    &format!("WRITE FROM_JOURNAL {} {range}", receipt.entry_id)
                )
                .is_err()
        );
        assert_eq!(before, fixture.native_bytes());
    }
    fixture
        .prepare(
            "utf8-selection",
            &format!("WRITE FROM_JOURNAL {} 1 7", receipt.entry_id),
        )
        .unwrap();
    assert_eq!(
        fixture.drafts()["drafts"]["d1"]["parts"][0],
        "\u{e9}\u{1f642}"
    );
}

#[test]
fn hidden_blocks_and_executable_next_are_removed_but_quoted_examples_survive() {
    let fixture = Fixture::new("astrid");
    let response = "<think>never transfer hidden text</think>\nVisible beginning.\n> NEXT: DELETE\n```text\nNEXT: WRITE START example\n```\n    NEXT: REST\n<analysis>hidden analysis</analysis>\nVisible ending.\nNEXT: WRITE CONTINUE";
    let (receipt, _) = fixture.published("quoted", response);
    fixture
        .prepare(
            "quoted-transfer",
            &format!("WRITE FROM_JOURNAL {}", receipt.entry_id),
        )
        .unwrap();
    let draft = fixture.drafts();
    let selected = draft["drafts"]["d1"]["parts"][0].as_str().unwrap();
    assert!(!selected.contains("hidden"));
    assert!(!selected.contains("NEXT: WRITE CONTINUE"));
    assert!(selected.contains("> NEXT: DELETE"));
    assert!(selected.contains("```text\nNEXT: WRITE START example\n```"));
    assert!(selected.contains("    NEXT: REST"));
    assert!(selected.ends_with("Visible ending."));
}

#[test]
fn inspection_is_private_paged_and_verified_delivery_never_appends_to_a_draft() {
    let fixture = Fixture::new("astrid");
    fixture
        .prepare("existing", "WRITE START Preserve this existing draft")
        .unwrap();
    let response = "\u{e9}".repeat(13_000);
    let (receipt, _) = fixture.published("pages", &response);
    let before = fixture.drafts();
    let reader = fixture.reader();
    let list = reader.prepare_action("WRITE JOURNALS 1").unwrap();
    assert!(list.text.contains(&receipt.entry_id));
    assert!(!list.text.contains(&response));
    let mut offers = vec![list];
    for page in 1..=3 {
        offers.push(
            reader
                .prepare_action(&format!("WRITE JOURNAL_READ {} {page}", receipt.entry_id))
                .unwrap(),
        );
    }
    for output in offers {
        assert_eq!(output.input_kind, InputKind::PrivateWriting);
        assert!(output.generation_requested);
        let id = output.navigation_id.as_deref().unwrap();
        assert!(output.text.len() < astrid_source_study::MAX_INPUT_BYTES);
        let response = json!({"message":{"content":"An inspection response, never draft prose.\nNEXT: REST"},"done":true}).to_string();
        assert!(
            reader
                .navigation_delivered(id, "{\"messages\":[]}", &response)
                .is_err()
        );
        let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
        let delivered = reader
            .navigation_delivered(id, &request, &response)
            .unwrap();
        assert_eq!(
            delivered,
            reader
                .navigation_delivered(id, &request, &response)
                .unwrap()
        );
        let artifact: Value =
            serde_json::from_slice(&fs::read(delivered.artifact_path).unwrap()).unwrap();
        assert_eq!(artifact["schema"], "private_writing_delivery_v1");
        assert!(artifact["draft"].is_null());
        assert_eq!(fixture.drafts()["drafts"], before["drafts"]);
        assert_eq!(fixture.drafts()["active"], before["active"]);
    }
    let state_after_inspections = fixture.native_bytes();
    for action in [
        "WRITE JOURNALS 0",
        "WRITE JOURNALS 2",
        "WRITE JOURNALS 1 extra",
        "WRITE JOURNAL_READ latest",
        "WRITE FROM_JOURNAL latest",
    ] {
        assert!(reader.prepare_action(action).is_err());
    }
    assert!(
        reader
            .prepare_action(&format!("WRITE JOURNAL_READ {} 4", receipt.entry_id))
            .is_err()
    );
    assert_eq!(state_after_inspections, fixture.native_bytes());
}

#[test]
fn owner_scope_rejects_foreign_state_paths_and_identical_cross_owner_operations() {
    let first = Fixture::new("astrid");
    let second = Fixture::new("minime");
    let astrid = first.reserve("same-host-id");
    let minime = second.reserve("same-host-id");
    assert_ne!(astrid.entry_id, minime.entry_id);
    let mut publish = first.publish_request("same-host-id", &astrid.entry_id, "Own response.");
    publish.source_path = second.source("foreign.txt");
    fs::write(&publish.source_path, "Own response.").unwrap();
    publish.file_sha256 = hash("Own response.");
    publish.response_span = None;
    let reader = first.reader();
    assert!(
        reader
            .prepare_journal_once(
                "foreign-file",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(publish)
            )
            .is_err()
    );
    let foreign_reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), first.root.clone())])).unwrap(),
        first.root.join("reader"),
    )
    .with_runtime_workspace(first.root.join("workspace"), "minime");
    assert!(foreign_reader.prepare_action("WRITE JOURNALS").is_err());
    assert!(
        second
            .reader()
            .prepare_action(&format!("WRITE JOURNAL_READ {}", astrid.entry_id))
            .is_err()
    );
}

#[test]
fn traversal_subdirectories_and_private_or_notice_wrappers_are_rejected() {
    let fixture = Fixture::new("minime");
    let receipt = fixture.reserve("paths");
    let request = fixture.publish_request("paths", &receipt.entry_id, "Exact public sentence.");
    for path in [
        PathBuf::from("relative.txt"),
        fixture.root.join("workspace/journal/../outside.txt"),
        fixture.source("nested/entry.txt"),
        fixture.source("private_entry.txt"),
        fixture.source("peer_mirror.txt"),
        fixture.source("runtime_notice.txt"),
    ] {
        let mut candidate = request.clone();
        candidate.source_path = path;
        let reader = fixture.reader();
        assert!(
            reader
                .prepare_journal_once(
                    "bad-path",
                    &reader.preparation_revision().unwrap(),
                    &JournalRegistration::Publish(candidate)
                )
                .is_err()
        );
    }
    for wrapper in [
        "=== PEER MIRROR EXCERPT ===",
        "=== SOURCE STUDY RUNTIME NOTICE ===",
        "=== PRIVATE JOURNAL ===",
        "Mode: moment_capture",
        "=== ASTRID JOURNAL ===",
    ] {
        let text = format!("{wrapper}\n\nExact public sentence.\n");
        fs::write(&request.source_path, &text).unwrap();
        let mut candidate = request.clone();
        candidate.file_sha256 = hash(&text);
        candidate.response_span = None;
        let reader = fixture.reader();
        assert!(
            reader
                .prepare_journal_once(
                    wrapper,
                    &reader.preparation_revision().unwrap(),
                    &JournalRegistration::Publish(candidate)
                )
                .is_err()
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_source_directory_metadata_and_hardlink_are_rejected() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new("astrid");
    let receipt = fixture.reserve("links");
    let request = fixture.publish_request("links", &receipt.entry_id, "Original.");
    let actual = fixture.root.join("actual.txt");
    fs::rename(&request.source_path, &actual).unwrap();
    symlink(&actual, &request.source_path).unwrap();
    let reader = fixture.reader();
    assert!(
        reader
            .prepare_journal_once(
                "symlink-file",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(request.clone())
            )
            .is_err()
    );
    fs::remove_file(&request.source_path).unwrap();
    fs::hard_link(&actual, &request.source_path).unwrap();
    assert!(
        reader
            .prepare_journal_once(
                "hardlink-file",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(request.clone())
            )
            .is_err()
    );
    fs::remove_file(&request.source_path).unwrap();
    fs::rename(&actual, &request.source_path).unwrap();
    let journal = fixture.root.join("workspace/journal");
    let real_journal = fixture.root.join("journal-real");
    fs::rename(&journal, &real_journal).unwrap();
    symlink(&real_journal, &journal).unwrap();
    assert!(
        reader
            .prepare_journal_once(
                "symlink-dir",
                &reader.preparation_revision().unwrap(),
                &JournalRegistration::Publish(request)
            )
            .is_err()
    );
    fs::remove_file(&journal).unwrap();
    fs::rename(real_journal, journal).unwrap();
    let registry = fixture.root.join("reader/journal-sources-v1.json");
    let real_registry = fixture.root.join("registry-real.json");
    fs::rename(&registry, &real_registry).unwrap();
    symlink(real_registry, registry).unwrap();
    assert!(reader.prepare_action("WRITE JOURNALS").is_err());
}

#[test]
fn changed_source_is_rejected_without_new_draft_or_silent_fallback() {
    let fixture = Fixture::new("astrid");
    let (receipt, request) = fixture.published("stale", "Original publication.");
    let (healthy, _) = fixture.published("healthy", "Still available.");
    fs::write(request.source_path, "Changed publication.").unwrap();
    let reader = fixture.reader();
    let listing = reader.prepare_action("WRITE JOURNALS").unwrap();
    assert!(
        listing
            .text
            .contains(&format!("{} | unavailable:", receipt.entry_id))
    );
    assert!(listing.text.contains(&healthy.entry_id));
    let before = fixture.native_bytes();
    assert!(
        reader
            .prepare_action(&format!("WRITE JOURNAL_READ {}", receipt.entry_id))
            .is_err()
    );
    assert!(
        fixture
            .prepare(
                "stale-transfer",
                &format!("WRITE FROM_JOURNAL {}", receipt.entry_id)
            )
            .is_err()
    );
    assert_eq!(before, fixture.native_bytes());
}

#[test]
fn capacity_failure_is_atomic_and_keeps_existing_drafts_and_public_source() {
    let fixture = Fixture::new("astrid");
    fixture
        .prepare("existing", "WRITE START Keep the original draft")
        .unwrap();
    let (receipt, request) = fixture.published("large", &"x".repeat(60_000));
    let source_before = fs::read(&request.source_path).unwrap();
    let before = fixture.native_bytes();
    assert!(
        fixture
            .prepare(
                "too-large",
                &format!("WRITE FROM_JOURNAL {}", receipt.entry_id)
            )
            .is_err()
    );
    assert_eq!(before, fixture.native_bytes());
    assert_eq!(source_before, fs::read(request.source_path).unwrap());
    assert_eq!(fixture.drafts()["drafts"].as_object().unwrap().len(), 1);
}

#[test]
fn corrupt_newer_duplicate_or_foreign_metadata_is_never_reset() {
    for failure in [
        "bad-json",
        "bad-hash",
        "new-schema",
        "duplicate",
        "foreign-owner",
        "invalid-span",
    ] {
        let fixture = Fixture::new("astrid");
        let (receipt, _) = fixture.published("corruption", "Retain original publication.");
        let registry = fixture.root.join("reader/journal-sources-v1.json");
        match failure {
            "bad-json" => fs::write(&registry, b"{corrupt").unwrap(),
            "bad-hash" => {
                let mut envelope: Value =
                    serde_json::from_slice(&fs::read(&registry).unwrap()).unwrap();
                envelope["sha256"] = "0".repeat(64).into();
                fs::write(&registry, serde_json::to_vec(&envelope).unwrap()).unwrap();
            },
            "new-schema" => fixture.edit_registry(|state| state["schema"] = 999.into()),
            "duplicate" => fixture.edit_registry(|state| {
                let duplicate = state["entries"][0].clone();
                state["entries"].as_array_mut().unwrap().push(duplicate);
            }),
            "foreign-owner" => fixture.edit_registry(|state| state["owner"] = "minime".into()),
            "invalid-span" => fixture.edit_registry(|state| {
                state["entries"][0]["publication"]["response_span"]["end"] = 0.into();
            }),
            _ => unreachable!(),
        }
        let before = fixture.native_bytes();
        assert!(
            fixture.reader().prepare_action("WRITE JOURNALS").is_err(),
            "{failure}"
        );
        assert!(
            fixture
                .prepare(
                    "corrupt-transfer",
                    &format!("WRITE FROM_JOURNAL {}", receipt.entry_id)
                )
                .is_err(),
            "{failure}"
        );
        assert_eq!(before, fixture.native_bytes(), "{failure}");
    }
}

#[test]
fn full_reservation_history_rejects_new_identity_atomically() {
    let fixture = Fixture::new("astrid");
    fixture.reserve("capacity");
    fixture.edit_registry(|state| {
        state["entries"] = Value::Array((0..4096).map(|index| {
            let operation = format!("synthetic-{index}");
            let id = hash(serde_json::to_vec(&("owner_public_journal_v1", "astrid", &operation)).unwrap());
            json!({"id":id,"reservation":{"owner":"astrid","operation_id":operation,"source_kind":"authored_public_journal"},"publication":null})
        }).collect());
    });
    let reader = fixture.reader();
    let before = fixture.native_bytes();
    assert!(
        reader
            .prepare_journal_once(
                "overflow",
                &reader.preparation_revision().unwrap(),
                &fixture.reserve_request("one-too-many")
            )
            .is_err()
    );
    assert_eq!(before, fixture.native_bytes());
}

#[test]
fn corrupt_reader_checkpoint_blocks_transfer_even_with_valid_source_metadata() {
    let fixture = Fixture::new("astrid");
    let (receipt, _) = fixture.published("reader-corrupt", "Source is still sound.");
    fs::write(fixture.root.join("reader/reader-v1.json"), b"{broken").unwrap();
    let before = fixture.native_bytes();
    assert!(
        fixture
            .prepare(
                "refuse-corrupt",
                &format!("WRITE FROM_JOURNAL {}", receipt.entry_id)
            )
            .is_err()
    );
    assert_eq!(before, fixture.native_bytes());
}

fn restore_before_and_stage_redo(
    fixture: &Fixture,
    before: &BTreeMap<PathBuf, Vec<u8>>,
    after: &BTreeMap<PathBuf, Vec<u8>>,
    boundary: usize,
) {
    let native = fixture.root.join("reader");
    let mut writes = serde_json::Map::new();
    for (path, bytes) in after {
        if before.get(path) == Some(bytes) {
            continue;
        }
        writes.insert(
            path.to_str().unwrap().into(),
            json!({
                "before":before.get(path).map(hash), "after":bytes,
            }),
        );
        if let Some(original) = before.get(path) {
            fs::write(native.join(path), original).unwrap();
        } else {
            fs::remove_file(native.join(path)).unwrap();
        }
    }
    assert!(!writes.is_empty());
    if boundary == 0 {
        return;
    }
    let body = serde_json::to_string(&json!({"schema":1,"writes":writes})).unwrap();
    let envelope = json!({"sha256":hash(&body),"body":body});
    fs::write(
        native.join("preparation-transaction-v1.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .unwrap();
    if boundary >= 2 {
        for (name, change) in writes
            .iter()
            .take(if boundary == 2 { 1 } else { writes.len() })
        {
            let bytes: Vec<u8> = serde_json::from_value(change["after"].clone()).unwrap();
            let path = native.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }
}

#[test]
fn reserve_publish_and_transfer_recover_exactly_once_across_synthetic_redo_boundaries() {
    for operation in ["reserve", "publish", "transfer"] {
        for boundary in 0..=3 {
            let fixture = Fixture::new("astrid");
            let reserve = fixture.reserve_request("crash-operation");
            let mut publication = None;
            let mut action = String::new();
            if operation != "reserve" {
                let reserved = fixture.reserve("crash-operation");
                let request = fixture.publish_request(
                    "crash-operation",
                    &reserved.entry_id,
                    "Exact recovery passage.",
                );
                if operation == "transfer" {
                    fixture.register(
                        "original-publication",
                        &JournalRegistration::Publish(request.clone()),
                    );
                    action = format!("WRITE FROM_JOURNAL {}", reserved.entry_id);
                }
                publication = Some(JournalRegistration::Publish(request));
            }
            let reader = fixture.reader();
            let revision = reader.preparation_revision().unwrap();
            let before = fixture.native_bytes();
            let run = |reader: &Reader| match operation {
                "reserve" => reader.prepare_journal_once("recover-operation", &revision, &reserve),
                "publish" => reader.prepare_journal_once(
                    "recover-operation",
                    &revision,
                    publication.as_ref().unwrap(),
                ),
                "transfer" => reader.prepare_once("recover-operation", &revision, &action),
                _ => unreachable!(),
            };
            let expected = run(&reader).unwrap();
            let after = fixture.native_bytes();
            restore_before_and_stage_redo(&fixture, &before, &after, boundary);
            assert_eq!(expected, run(&fixture.reader()).unwrap());
            assert_eq!(expected, run(&fixture.reader()).unwrap());
            assert_eq!(
                after,
                fixture.native_bytes(),
                "{operation} boundary {boundary}"
            );
            if operation == "transfer" {
                assert_eq!(fixture.drafts()["drafts"].as_object().unwrap().len(), 1);
                assert_eq!(
                    fixture.drafts()["drafts"]["d1"]["parts"][0],
                    "Exact recovery passage."
                );
            }
        }
    }
}

#[test]
fn concurrent_host_reservations_commit_one_identity_and_one_receipt() {
    let fixture = Fixture::new("astrid");
    let request = fixture.reserve_request("concurrent");
    let revision = fixture.reader().preparation_revision().unwrap();
    let outputs = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    fixture
                        .reader()
                        .prepare_journal_once("concurrent-host-request", &revision, &request)
                        .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(outputs.iter().all(|output| *output == outputs[0]));
    let envelope: Value = serde_json::from_slice(
        &fs::read(fixture.root.join("reader/journal-sources-v1.json")).unwrap(),
    )
    .unwrap();
    let sources: Value = serde_json::from_str(envelope["body"].as_str().unwrap()).unwrap();
    assert_eq!(sources["entries"].as_array().unwrap().len(), 1);
    assert_eq!(
        fs::read_dir(fixture.root.join("reader/preparation-operations"))
            .unwrap()
            .count(),
        1
    );
}
