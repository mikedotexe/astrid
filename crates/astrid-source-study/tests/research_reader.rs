use astrid_source_study::research::{MAX_CHECKPOINT_BYTES, ResearchReader, Section};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const GUARD: &[u8] = b"{\"requires_private_writing_schema\":2,\"checkpoint\":\"drafts-v2.json\"}\n";
const PASSAGE: &str = "  Exact authored line.\r\nSecond line with caf\u{e9}.\n";

fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    state: Value,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        // macOS's conventional temporary path itself contains a symlink.
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("writing")).unwrap();
        fs::write(root.join("reader.lock"), b"").unwrap();
        fs::write(root.join("writing/drafts-v1.json"), GUARD).unwrap();
        let body = serde_json::to_string("astrid").unwrap();
        fs::write(
            root.join("preparation-owner-v1.json"),
            json!({"sha256": hash(&body), "body": body}).to_string(),
        )
        .unwrap();
        let state = json!({
            "schema_version": 4, "sequence": 7, "active": "d1", "pending": null,
            "receipts": {}, "last_choice": null, "retained_pending": {}, "preview_deliveries": {},
            "drafts": {"d1": {
                "topic": "PRIVATE_TITLE_SENTINEL", "question": "PRIVATE_QUESTION_SENTINEL",
                "evidence": "PRIVATE_REFERENCE_SENTINEL", "parent": null, "finished": false,
                "revision": 2, "parts": [PASSAGE, "Second complete part."],
                "stopping_point": "PRIVATE_STOP_SENTINEL", "observations": {"owner": null,"records": []}
            }}
        });
        let fixture = Self {
            _temp: temp,
            root,
            state,
        };
        fixture.save();
        fixture
    }

    fn save(&self) {
        fs::write(self.checkpoint(), serde_json::to_vec(&self.state).unwrap()).unwrap();
    }

    fn checkpoint(&self) -> PathBuf {
        self.root.join("writing/drafts-v2.json")
    }
    fn reader(&self) -> ResearchReader {
        ResearchReader::new(self.root.clone(), "astrid".into())
    }
    fn token(&self) -> String {
        self.reader().list(1, 20).unwrap().snapshot_sha256
    }

    fn cli(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_writing-research"))
            .arg("--state-directory")
            .arg(&self.root)
            .args(["--owner", "astrid"])
            .args(arguments)
            .output()
            .unwrap()
    }

    fn add_annotations(&mut self) {
        let capture = record(
            "empty",
            "capture",
            &json!({"kind": "capture", "attachment_revision": "a".repeat(64),
            "snapshot": {
                "source_sha256": "b".repeat(64), "captured_at_unix_ms": 1000, "requested_seconds": 1,
                "source": "minime/workspace/runtime/esn_activation_trace_v1.json",
                "scope": "native_esn_128_activations", "identity": "boot_and_node_layout_unverified",
                "frames": [{"t_ms": 1000, "wall_clock_unix_ms": 1000, "activations": vec![0.0; 128]}],
                "quality": {"recorder_policy": "esn_activation_trace_v1", "source_frame_count": 1,
                    "nominal_sample_interval_ms": 1000, "source_retained_frame_limit": 180, "sanitized_frame_count": 0}
            }}),
        );
        let note = record(
            capture["id"].as_str().unwrap(),
            "note",
            &json!({
                "kind": "annotation", "target": capture["id"], "text": "PRIVATE_ANNOTATION_SENTINEL"
            }),
        );
        self.state["drafts"]["d1"]["observations"] =
            json!({"owner": "astrid", "records": [capture, note]});
        self.save();
    }
}

fn record(previous: &str, request_id: &str, body: &Value) -> Value {
    let body_json = body.to_string();
    let request_sha256 = hash(request_id);
    let id = hash(format!(
        "{previous}\n{}\n{request_sha256}\n{}",
        hash(request_id),
        hash(&body_json)
    ));
    json!({"id": id, "previous": previous, "request_id": request_id,
        "request_sha256": request_sha256, "body_json": body_json})
}

// Compare recursive bytes and write-sensitive metadata. Access times are omitted:
// reading may update those under the filesystem's access-time policy.
#[derive(Debug, PartialEq, Eq)]
struct InventoryEntry {
    bytes: Vec<u8>,
    len: u64,
    modified: Option<std::time::SystemTime>,
    readonly: bool,
    #[cfg(unix)]
    unix: (u64, u64, u32, u32, u32, u64, i64, i64),
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, InventoryEntry> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, InventoryEntry>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(path).unwrap()
        } else if metadata.file_type().is_symlink() {
            fs::read_link(path)
                .unwrap()
                .to_string_lossy()
                .as_bytes()
                .to_vec()
        } else {
            Vec::new()
        };
        #[cfg(unix)]
        let unix = {
            use std::os::unix::fs::MetadataExt as _;
            (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.uid(),
                metadata.gid(),
                metadata.nlink(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        };
        result.insert(
            path.strip_prefix(root).unwrap().to_path_buf(),
            InventoryEntry {
                bytes,
                len: metadata.len(),
                modified: metadata.modified().ok(),
                readonly: metadata.permissions().readonly(),
                #[cfg(unix)]
                unix,
            },
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), result);
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn unchanged<T>(fixture: &Fixture, action: impl FnOnce() -> T) -> T {
    let before = inventory(&fixture.root);
    let result = action();
    assert_eq!(before, inventory(&fixture.root));
    result
}

#[test]
fn metadata_only_listing_and_exact_section_selection_leave_recursive_inventory_unchanged() {
    let mut fixture = Fixture::new();
    fixture.add_annotations();
    fs::write(fixture.root.join("writing/raw-delivery.json"),
        b"{\"request_json\":\"DO_NOT_READ_REQUEST\",\"response_json\":\"<think>DO_NOT_READ_RESPONSE</think>\"}").unwrap();
    unchanged(&fixture, || {
        let page = fixture.reader().list(1, 20).unwrap();
        let listing = serde_json::to_value(&page).unwrap();
        assert!(!listing.to_string().contains("PRIVATE_"));
        assert!(!listing.to_string().contains("Exact authored"));
        assert_eq!(
            listing["drafts"][0]["part_bytes"],
            json!([PASSAGE.len(), "Second complete part.".len()])
        );
        assert_eq!(listing["drafts"][0]["revision"], 2);
        assert_eq!(listing["drafts"][0]["status"], "active");
        for (section, key, expected) in [
            (
                Section::Passages,
                "parts",
                json!([PASSAGE, "Second complete part."]),
            ),
            (
                Section::Questions,
                "text",
                json!("PRIVATE_QUESTION_SENTINEL"),
            ),
            (
                Section::References,
                "text",
                json!("PRIVATE_REFERENCE_SENTINEL"),
            ),
            (
                Section::StoppingPoint,
                "text",
                json!("PRIVATE_STOP_SENTINEL"),
            ),
        ] {
            let output = serde_json::to_value(
                fixture
                    .reader()
                    .show("d1", &page.snapshot_sha256, section)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(output[key], expected);
            assert!(output.get("annotations").is_none());
            assert!(!output.to_string().contains("PRIVATE_TITLE_SENTINEL"));
            assert!(!output.to_string().contains("DO_NOT_READ"));
        }
        let output = serde_json::to_value(
            fixture
                .reader()
                .show("d1", &page.snapshot_sha256, Section::Annotations)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            output["annotations"][0]["text"],
            "PRIVATE_ANNOTATION_SENTINEL"
        );
        assert_eq!(output["annotations"].as_array().unwrap().len(), 1);
        for key in ["parts", "text", "body_json", "snapshot", "topic"] {
            assert!(output.get(key).is_none());
        }
    });
}

#[test]
fn schemas_four_and_five_are_read_without_migration_and_newer_or_older_are_refused() {
    for schema in [0, 3, 4, 5, 6, u32::MAX] {
        let mut fixture = Fixture::new();
        fixture.state["schema_version"] = schema.into();
        if schema == 5 {
            fixture.state["operations"] = json!({"operation-id": {"operation": "PRIVATE_OPERATION_SENTINEL",
                "draft": "d1", "revision": 2, "generation_requested": false}});
        }
        fixture.save();
        unchanged(&fixture, || {
            let result = fixture.reader().list(1, 20);
            assert_eq!(result.is_ok(), matches!(schema, 4 | 5));
            if let Ok(page) = result {
                assert!(
                    !serde_json::to_string(&page)
                        .unwrap()
                        .contains("PRIVATE_OPERATION_SENTINEL")
                );
            }
        });
    }
}

#[test]
fn minime_owner_binding_uses_the_same_read_only_contract() {
    let fixture = Fixture::new();
    let body = serde_json::to_string("minime").unwrap();
    fs::write(
        fixture.root.join("preparation-owner-v1.json"),
        json!({"sha256": hash(&body), "body": body}).to_string(),
    )
    .unwrap();
    unchanged(&fixture, || {
        let reader = ResearchReader::new(fixture.root.clone(), "minime".into());
        let page = reader.list(1, 20).unwrap();
        assert_eq!(page.owner, "minime");
        assert!(
            reader
                .show("d1", &page.snapshot_sha256, Section::Passages)
                .is_ok()
        );
        assert!(fixture.reader().list(1, 20).is_err());
    });
}

#[cfg(unix)]
#[test]
fn read_only_permissions_are_preserved_without_native_lock_side_effects() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    for name in [
        "reader.lock",
        "preparation-owner-v1.json",
        "writing/drafts-v1.json",
        "writing/drafts-v2.json",
    ] {
        fs::set_permissions(fixture.root.join(name), fs::Permissions::from_mode(0o400)).unwrap();
    }
    for directory in [&fixture.root, &fixture.root.join("writing")] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o500)).unwrap();
    }
    unchanged(&fixture, || {
        let token = fixture.token();
        assert!(
            fixture
                .reader()
                .show("d1", &token, Section::Passages)
                .is_ok()
        );
    });
    // Fixture cleanup only, outside the inventory-protected reader invocation.
    for directory in [&fixture.root, &fixture.root.join("writing")] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

#[test]
fn schema_five_operations_are_validated_but_never_returned() {
    let mut fixture = Fixture::new();
    fixture.state["operations"] = json!({"op": {"operation": "private", "draft": "d1", "revision": 2, "generation_requested": false}});
    fixture.save();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
    fixture.state["schema_version"] = 5.into();
    fixture.state["operations"]["op"]["generation_requested"] = "PRIVATE_INVALID_FLAG".into();
    fixture.save();
    unchanged(&fixture, || {
        let output = fixture.cli(&["list"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_INVALID_FLAG"));
    });
}

#[test]
fn wrong_missing_or_tampered_owner_envelope_is_refused_without_writes() {
    let fixture = Fixture::new();
    unchanged(&fixture, || {
        assert!(
            ResearchReader::new(fixture.root.clone(), "minime".into())
                .list(1, 20)
                .is_err()
        );
        assert!(
            ResearchReader::new(fixture.root.clone(), "../astrid".into())
                .list(1, 20)
                .is_err()
        );
    });
    fs::write(
        fixture.root.join("preparation-owner-v1.json"),
        json!({"body": "\"astrid\"", "sha256": "0".repeat(64)}).to_string(),
    )
    .unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
    fs::remove_file(fixture.root.join("preparation-owner-v1.json")).unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
}

#[test]
fn any_checkpoint_or_owner_envelope_change_invalidates_the_snapshot_not_just_prose_revision() {
    let mut fixture = Fixture::new();
    let token = fixture.token();
    fixture.state["drafts"]["d1"]["question"] = "Changed question".into();
    fixture.save();
    unchanged(&fixture, || {
        assert!(
            fixture
                .reader()
                .show("d1", &token, Section::Passages)
                .is_err()
        );
        assert!(fixture.reader().show("d1", "2", Section::Passages).is_err());
        assert!(
            fixture
                .reader()
                .show("d2", &fixture.token(), Section::Passages)
                .is_err()
        );
    });
    let token = fixture.token();
    let path = fixture.root.join("preparation-owner-v1.json");
    let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    unchanged(&fixture, || {
        assert!(
            fixture
                .reader()
                .show("d1", &token, Section::Passages)
                .is_err()
        );
    });
}

#[test]
fn corrupt_tail_duplicate_draft_and_unknown_fields_never_produce_partial_output() {
    let fixture = Fixture::new();
    for bytes in [
        format!("{} TRAILING_PRIVATE_SENTINEL", fixture.state),
        fixture.state.to_string().trim_end_matches('}').into(),
        fixture.state.to_string().replacen(
            "\"drafts\":{",
            &format!("\"drafts\":{{\"d1\":{},", fixture.state["drafts"]["d1"]),
            1,
        ),
        fixture.state.to_string().replacen(
            "\"schema_version\":4",
            "\"schema_version\":4,\"PRIVATE_UNKNOWN_FIELD\":true",
            1,
        ),
    ] {
        fs::write(fixture.checkpoint(), bytes).unwrap();
        unchanged(&fixture, || {
            let output = fixture.cli(&["list"]);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_"));
        });
    }
}

#[test]
fn downgrade_guard_and_legacy_only_stores_are_refused_without_migration() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("writing/drafts-v1.json"), b"{}").unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
    fs::remove_file(fixture.checkpoint()).unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
}

#[test]
fn missing_store_and_missing_existing_lock_are_never_created() {
    let fixture = Fixture::new();
    unchanged(&fixture, || {
        assert!(
            ResearchReader::new(fixture.root.join("missing"), "astrid".into())
                .list(1, 20)
                .is_err()
        );
    });
    fs::remove_file(fixture.root.join("reader.lock")).unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
}

#[test]
fn pending_preparation_is_never_recovered_even_when_the_journal_is_valid() {
    let fixture = Fixture::new();
    let body =
        json!({"schema": 1, "writes": {"would-be-written": {"before": null, "after": [65]}}})
            .to_string();
    fs::write(
        fixture.root.join("preparation-transaction-v1.json"),
        json!({"sha256": hash(&body), "body": body}).to_string(),
    )
    .unwrap();
    unchanged(&fixture, || {
        assert!(fixture.reader().list(1, 20).is_err());
        assert!(!fixture.root.join("would-be-written").exists());
    });
}

#[test]
fn writer_lock_contention_refuses_immediately_without_state_changes() {
    let fixture = Fixture::new();
    let lock = fs::File::open(fixture.root.join("reader.lock")).unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
}

#[cfg(unix)]
#[test]
fn symlinked_files_and_directories_are_never_followed() {
    use std::os::unix::fs::symlink;
    for name in [
        "reader.lock",
        "preparation-owner-v1.json",
        "writing/drafts-v1.json",
        "writing/drafts-v2.json",
        "writing",
    ] {
        let fixture = Fixture::new();
        let source = fixture.root.join(name);
        let destination = fixture.root.join("retained-original");
        fs::rename(&source, &destination).unwrap();
        symlink(&destination, &source).unwrap();
        unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
    }
    let fixture = Fixture::new();
    let alias = fixture.root.join("owner-alias");
    symlink(&fixture.root, &alias).unwrap();
    unchanged(&fixture, || {
        assert!(
            ResearchReader::new(alias, "astrid".into())
                .list(1, 20)
                .is_err()
        );
    });
}

#[test]
fn oversized_and_nonregular_files_are_refused() {
    let fixture = Fixture::new();
    fs::File::create(fixture.checkpoint())
        .unwrap()
        .set_len(MAX_CHECKPOINT_BYTES.saturating_add(1))
        .unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
    fs::remove_file(fixture.checkpoint()).unwrap();
    fs::create_dir(fixture.checkpoint()).unwrap();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
}

#[test]
fn listing_is_numeric_bounded_and_has_no_titles_or_prose() {
    let mut fixture = Fixture::new();
    for number in 2..=23 {
        fixture.state["drafts"][format!("d{number}")] = fixture.state["drafts"]["d1"].clone();
    }
    fixture.state["drafts"]["d2"]["finished"] = true.into();
    fixture.save();
    unchanged(&fixture, || {
        let first = serde_json::to_value(fixture.reader().list(1, 20).unwrap()).unwrap();
        assert_eq!(first["drafts"].as_array().unwrap().len(), 20);
        assert_eq!(first["drafts"][9]["id"], "d10");
        assert_eq!(first["drafts"][1]["status"], "finished");
        assert_eq!(first["drafts"][2]["status"], "parked");
        assert_eq!(first["next_page"], 2);
        let last = serde_json::to_value(fixture.reader().list(2, 20).unwrap()).unwrap();
        assert_eq!(last["drafts"].as_array().unwrap().len(), 3);
        assert!(last["next_page"].is_null());
        for (page, size) in [(0, 20), (1, 0), (1, 21), (3, 20), (usize::MAX, 20)] {
            assert!(fixture.reader().list(page, size).is_err());
        }
    });
}

#[test]
fn historical_hidden_markers_and_embedded_wires_are_rejected_not_sanitized() {
    let mut fixture = Fixture::new();
    for text in [
        "Visible <think>HIDDEN_SENTINEL</think> end",
        "<analysis>HIDDEN_SENTINEL",
        "</analysis>HIDDEN_SENTINEL",
        "<Thinking>HIDDEN_SENTINEL</Thinking>",
        "<THINKING role='internal'>HIDDEN_SENTINEL",
        "<writing_mode>HIDDEN_SENTINEL",
        "<denial_record>HIDDEN_SENTINEL",
        "<reasoning>HIDDEN_SENTINEL",
        "<|start|>assistant<|channel|>analysis<|message|>HIDDEN_SENTINEL",
        "{\"response_json\":\"HIDDEN_SENTINEL\"}",
        "```json\n{\"messages\": [{\"content\":\"HIDDEN_SENTINEL\"}]}\n```",
        "{\"message\":{\"thinking\":\"HIDDEN_SENTINEL\"}}",
    ] {
        fixture.state["drafts"]["d1"]["parts"] = json!([text]);
        fixture.save();
        unchanged(&fixture, || {
            let token = fixture.token();
            let output = fixture.cli(&["show", "--draft", "d1", "--revision", &token]);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&output.stderr).contains("HIDDEN_SENTINEL"));
            assert!(
                fixture
                    .reader()
                    .show("d1", &token, Section::Questions)
                    .is_ok()
            );
        });
    }
}

#[test]
fn all_explicit_text_sections_apply_the_hidden_content_guard() {
    for (field, section) in [
        ("question", Section::Questions),
        ("evidence", Section::References),
        ("stopping_point", Section::StoppingPoint),
    ] {
        let mut fixture = Fixture::new();
        fixture.state["drafts"]["d1"][field] = "<thinking>HIDDEN_SENTINEL".into();
        fixture.save();
        unchanged(&fixture, || {
            let token = fixture.token();
            assert!(fixture.reader().show("d1", &token, section).is_err());
            assert!(
                fixture
                    .reader()
                    .show("d1", &token, Section::Passages)
                    .is_ok()
            );
        });
    }
    let mut fixture = Fixture::new();
    fixture.add_annotations();
    let records = fixture.state["drafts"]["d1"]["observations"]["records"]
        .as_array_mut()
        .unwrap();
    records[1] = record(
        records[0]["id"].as_str().unwrap(),
        "note",
        &json!({
            "kind": "annotation", "target": records[0]["id"], "text": "<thinking>HIDDEN_SENTINEL"
        }),
    );
    fixture.save();
    unchanged(&fixture, || {
        assert!(
            fixture
                .reader()
                .show("d1", &fixture.token(), Section::Annotations)
                .is_err()
        );
    });
}

#[test]
fn corrupt_observation_tail_and_mismatching_owner_are_refused() {
    let mut fixture = Fixture::new();
    fixture.add_annotations();
    fixture.state["drafts"]["d1"]["observations"]["owner"] = "minime".into();
    fixture.save();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
    fixture.state["drafts"]["d1"]["observations"]["owner"] = "astrid".into();
    fixture.state["drafts"]["d1"]["observations"]["records"][1]["body_json"] = "{}".into();
    fixture.save();
    unchanged(&fixture, || assert!(fixture.reader().list(1, 20).is_err()));
}

#[test]
fn cli_requires_explicit_owner_draft_and_snapshot_and_rejects_export_or_mutation() {
    let fixture = Fixture::new();
    unchanged(&fixture, || {
        let output = fixture.cli(&["list"]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        let token = page["snapshot_sha256"].as_str().unwrap();
        let output = fixture.cli(&["show", "--draft", "d1", "--revision", token]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["parts"][0],
            PASSAGE
        );
        for args in [
            vec!["show"],
            vec!["show", "--draft", "d1"],
            vec!["export"],
            vec!["list", "--page-size", "21"],
            vec!["list", "--export", "output.json"],
            vec!["show", "--draft", "d1", "--revision", token, "all"],
            vec![
                "show",
                "--draft",
                "d1",
                "--revision",
                token,
                "references",
                "questions",
            ],
            vec!["WRITE", "CONTINUE"],
        ] {
            let output = fixture.cli(&args);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
        let output = Command::new(env!("CARGO_BIN_EXE_writing-research"))
            .arg("--state-directory")
            .arg(&fixture.root)
            .arg("list")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    });
}
