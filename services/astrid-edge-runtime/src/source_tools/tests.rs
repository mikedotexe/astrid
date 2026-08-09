use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::{
        fs::{OpenOptionsExt as _, PermissionsExt as _, symlink},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;
use super::{digest::sha256_hex, types::MAX_SOURCE_FILE_BYTES};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    root: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "astrid-source-tools-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create isolated test directory");
        Self { root }
    }

    fn source(&self) -> PathBuf {
        self.root.join("source")
    }

    fn candidates(&self) -> PathBuf {
        self.root.join("candidates")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove isolated test directory");
    }
}

struct Fixture {
    _temporary: TestDirectory,
    source: PathBuf,
    candidates: PathBuf,
    attestation: SignedSourceRootV1,
}

impl Fixture {
    fn new() -> Self {
        let temporary = TestDirectory::new("fixture");
        let source = temporary.source();
        let candidates = temporary.candidates();
        fs::create_dir(&source).expect("create source");
        fs::create_dir(source.join("nested")).expect("create source child");
        fs::write(
            source.join("engine.rs"),
            "pub fn shelf() -> f64 {\n    0.68\n}\n",
        )
        .expect("write source");
        fs::write(
            source.join("nested").join("NOTES.md"),
            "# Reservoir Notes\nThe stable shelf is sixty-eight percent.\n",
        )
        .expect("write notes");
        let manifest = compute_source_manifest_sha256(&source, "edge-source-1")
            .expect("compute fixture manifest");
        let payload = signed_source_payload_sha256("edge-source-1", &manifest)
            .expect("compute binding payload");
        let attestation = SignedSourceRootV1 {
            root: source.clone(),
            expected_source_id: "edge-source-1".to_string(),
            expected_manifest_sha256: manifest,
            binding: SignedSourceBindingV1 {
                schema: SOURCE_BINDING_SCHEMA_V1.to_string(),
                signer_key_id: "operator-key-1".to_string(),
                signature_hex: "ab".repeat(64),
                signed_payload_sha256: payload,
            },
        };
        Self {
            _temporary: temporary,
            source,
            candidates,
            attestation,
        }
    }

    fn broker(&self) -> SourceCandidateBroker {
        SourceCandidateBroker::open(&self.attestation, self.candidates.clone())
            .expect("open fixture broker")
    }
}

fn find_entry(broker: &SourceCandidateBroker, basename: &str) -> SourceEntry {
    broker
        .list_source(&ListSourceRequest {
            cursor: 0,
            limit: 50,
        })
        .expect("list source")
        .entries
        .into_iter()
        .find(|entry| entry.basename == basename)
        .expect("fixture source entry")
}

fn begin(broker: &SourceCandidateBroker, candidate_id: &str) -> CandidateInspection {
    broker
        .begin_candidate(&BeginCandidateRequest {
            candidate_id: candidate_id.to_string(),
            base_generation: "generation-7".to_string(),
            proposal_sha256: sha256_hex(b"bounded proposal"),
        })
        .expect("begin candidate")
}

#[test]
fn source_queries_are_bounded_and_path_free() {
    let fixture = Fixture::new();
    let broker = fixture.broker();
    let first_page = broker
        .list_source(&ListSourceRequest {
            cursor: 0,
            limit: 1,
        })
        .expect("list first page");
    assert_eq!(first_page.entries.len(), 1);
    assert_eq!(first_page.next_cursor, Some(1));
    assert!(!first_page.entries[0].source_file_id.contains('/'));

    let search = broker
        .search_source(&SearchSourceRequest {
            query: "STABLE SHELF".to_string(),
            cursor: 0,
            max_files: 8,
            max_matches: 4,
        })
        .expect("search source");
    assert_eq!(search.matches.len(), 1);
    assert!(search.matches[0].excerpt.contains("stable shelf"));
    assert!(search.matches[0].excerpt.chars().count() <= MAX_EXCERPT_CHARS);

    let engine = find_entry(&broker, "engine.rs");
    let chunk = broker
        .read_source_chunk(&ReadSourceChunkRequest {
            source_file_id: engine.source_file_id,
            start_line: 2,
            max_lines: 2,
        })
        .expect("read source chunk");
    assert_eq!(chunk.lines[0].line_number, 2);
    assert_eq!(chunk.lines[0].text.trim(), "0.68");
    assert_eq!(chunk.next_line, None);

    assert!(matches!(
        broker.search_source(&SearchSourceRequest {
            query: "x".repeat(MAX_SEARCH_QUERY_CHARS.saturating_add(1)),
            cursor: 0,
            max_files: 1,
            max_matches: 1,
        }),
        Err(BrokerError::LimitExceeded(_))
    ));
    assert!(matches!(
        broker.read_source_chunk(&ReadSourceChunkRequest {
            source_file_id: "../engine.rs".to_string(),
            start_line: 1,
            max_lines: 1,
        }),
        Err(BrokerError::InvalidValue(_))
    ));
}

#[test]
fn complete_candidate_lifecycle_remains_intent_only() {
    let fixture = Fixture::new();
    let broker = fixture.broker();
    let engine = find_entry(&broker, "engine.rs");
    let initial = begin(&broker, "candidate-1");
    assert_eq!(initial.status, "draft");
    assert!(initial.authority.contains("no_build"));

    let patched = broker
        .apply_candidate_patch(&CandidatePatchRequest {
            candidate_id: "candidate-1".to_string(),
            source_file_id: engine.source_file_id.clone(),
            expected_old_sha256: engine.sha256.clone(),
            replacement: "pub fn shelf() -> f64 {\r\n    0.69\r\n}".to_string(),
        })
        .expect("apply exact replacement");
    assert_eq!(patched.changes.len(), 1);
    assert!(!patched.changes[0].formatted);

    let formatted = broker
        .format_candidate(&FormatCandidateRequest {
            candidate_id: "candidate-1".to_string(),
        })
        .expect("format candidate");
    assert_eq!(formatted.status, "formatted");
    assert!(formatted.changes[0].formatted);

    let diff = broker
        .read_generation_diff(&GenerationDiffRequest {
            candidate_id: "candidate-1".to_string(),
            source_file_id: engine.source_file_id,
            start_line: 1,
            max_lines: 20,
        })
        .expect("read candidate diff");
    assert!(diff.before.iter().any(|line| line.text.contains("0.68")));
    assert!(diff.after.iter().any(|line| line.text.contains("0.69")));
    assert!(diff.authority.contains("no_build"));

    let receipt = broker
        .submit_candidate(&SubmitCandidateRequest {
            candidate_id: "candidate-1".to_string(),
            expected_candidate_digest: formatted.candidate_digest.clone(),
            attestation: exact_attestation("candidate-1", &formatted.candidate_digest),
        })
        .expect("submit intent");
    assert!(receipt.authority.contains("no_build"));
    assert_eq!(receipt.candidate_digest, formatted.candidate_digest);
    let submitted = broker
        .inspect_candidate(&InspectCandidateRequest {
            candidate_id: "candidate-1".to_string(),
        })
        .expect("inspect submitted candidate");
    assert_eq!(submitted.status, "submitted");

    for path in [
        fixture.candidates.join("active.state"),
        fixture.candidates.join("drafts/candidate-1/state.v1"),
        fixture
            .candidates
            .join("submissions")
            .join(receipt.submission_artifact),
    ] {
        let mode = fs::metadata(path)
            .expect("private artifact metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
    }
}

#[test]
fn exact_hashes_and_exact_model_provenance_are_required() {
    let fixture = Fixture::new();
    let broker = fixture.broker();
    let engine = find_entry(&broker, "engine.rs");
    begin(&broker, "candidate-2");
    assert!(matches!(
        broker.apply_candidate_patch(&CandidatePatchRequest {
            candidate_id: "candidate-2".to_string(),
            source_file_id: engine.source_file_id.clone(),
            expected_old_sha256: sha256_hex(b"wrong"),
            replacement: "different\n".to_string(),
        }),
        Err(BrokerError::Stale(_))
    ));
    broker
        .apply_candidate_patch(&CandidatePatchRequest {
            candidate_id: "candidate-2".to_string(),
            source_file_id: engine.source_file_id,
            expected_old_sha256: engine.sha256,
            replacement: "pub fn shelf() -> f64 { 0.67 }\n".to_string(),
        })
        .expect("apply replacement");
    let formatted = broker
        .format_candidate(&FormatCandidateRequest {
            candidate_id: "candidate-2".to_string(),
        })
        .expect("format replacement");
    let mut fallback = exact_attestation("candidate-2", &formatted.candidate_digest);
    fallback.provenance = "transport_fallback".to_string();
    assert!(matches!(
        broker.submit_candidate(&SubmitCandidateRequest {
            candidate_id: "candidate-2".to_string(),
            expected_candidate_digest: formatted.candidate_digest,
            attestation: fallback,
        }),
        Err(BrokerError::SecurityViolation(_))
    ));
}

#[test]
fn one_active_draft_and_changed_line_cap_are_enforced() {
    let fixture = Fixture::new();
    let broker = fixture.broker();
    let engine = find_entry(&broker, "engine.rs");
    let active = begin(&broker, "candidate-active");
    assert!(matches!(
        broker.begin_candidate(&BeginCandidateRequest {
            candidate_id: "candidate-other".to_string(),
            base_generation: "generation-7".to_string(),
            proposal_sha256: sha256_hex(b"other"),
        }),
        Err(BrokerError::Conflict(_))
    ));
    let oversized_change = "changed\n".repeat(MAX_CHANGED_LINES.saturating_add(1));
    assert!(matches!(
        broker.apply_candidate_patch(&CandidatePatchRequest {
            candidate_id: "candidate-active".to_string(),
            source_file_id: engine.source_file_id,
            expected_old_sha256: engine.sha256,
            replacement: oversized_change,
        }),
        Err(BrokerError::LimitExceeded(_))
    ));
    let abandoned = broker
        .abandon_candidate(&AbandonCandidateRequest {
            candidate_id: "candidate-active".to_string(),
            expected_candidate_digest: active.candidate_digest,
        })
        .expect("abandon exact draft");
    assert_eq!(abandoned.status, "abandoned");
    begin(&broker, "candidate-other");
}

#[test]
fn stale_source_and_candidate_tampering_fail_closed() {
    let fixture = Fixture::new();
    let broker = fixture.broker();
    let engine = find_entry(&broker, "engine.rs");
    begin(&broker, "candidate-tamper");
    broker
        .apply_candidate_patch(&CandidatePatchRequest {
            candidate_id: "candidate-tamper".to_string(),
            source_file_id: engine.source_file_id.clone(),
            expected_old_sha256: engine.sha256,
            replacement: "changed\n".to_string(),
        })
        .expect("apply replacement");
    let replacement = fixture
        .candidates
        .join("drafts/candidate-tamper/replacements")
        .join(format!("{}.replacement", engine.source_file_id));
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o644))
        .expect("widen replacement permissions");
    assert!(matches!(
        broker.inspect_candidate(&InspectCandidateRequest {
            candidate_id: "candidate-tamper".to_string(),
        }),
        Err(BrokerError::SecurityViolation(_))
    ));

    let second = Fixture::new();
    let second_broker = second.broker();
    fs::write(second.source.join("engine.rs"), "mutated\n").expect("mutate source");
    assert!(matches!(
        second_broker.list_source(&ListSourceRequest {
            cursor: 0,
            limit: 1,
        }),
        Err(BrokerError::Stale(_))
    ));
}

#[test]
fn unsafe_source_entries_are_rejected() {
    assert_unsafe_source("hidden", |root| {
        fs::write(root.join(".secret"), "secret\n").expect("write hidden source");
    });
    assert_unsafe_source("binary", |root| {
        fs::write(root.join("binary.rs"), b"text\0binary").expect("write binary source");
    });
    assert_unsafe_source("symlink", |root| {
        fs::write(root.join("target.rs"), "safe\n").expect("write target");
        symlink(root.join("target.rs"), root.join("link.rs")).expect("create source symlink");
    });
    assert_unsafe_source("hardlink", |root| {
        fs::write(root.join("first.rs"), "safe\n").expect("write hard-link source");
        fs::hard_link(root.join("first.rs"), root.join("second.rs"))
            .expect("create source hard link");
    });
    assert_unsafe_source("socket", |root| {
        let listener = UnixListener::bind(root.join("source.sock")).expect("bind local socket");
        drop(listener);
    });
    assert_unsafe_source("oversize", |root| {
        let file = File::create(root.join("large.rs")).expect("create sparse source");
        file.set_len(MAX_SOURCE_FILE_BYTES.saturating_add(1))
            .expect("size sparse source");
    });
}

#[test]
fn source_binding_and_root_separation_fail_closed() {
    let fixture = Fixture::new();
    let mut malformed = fixture.attestation.clone();
    malformed.binding.signed_payload_sha256 = sha256_hex(b"rebound");
    assert!(matches!(
        SourceCandidateBroker::open(&malformed, fixture.candidates.clone()),
        Err(BrokerError::Integrity(_))
    ));
    assert!(matches!(
        SourceCandidateBroker::open(&fixture.attestation, fixture.source.join("candidates")),
        Err(BrokerError::SecurityViolation(_))
    ));
    assert!(!fixture.source.join("candidates").exists());
}

#[test]
fn build_evidence_is_digest_bound_and_owner_only() {
    let fixture = Fixture::new();
    let broker = fixture.broker();
    let body = format!(
        "schema={BUILD_EVIDENCE_SCHEMA_V1}\nbuild_id=build-1\ncandidate_id=candidate-1\ncandidate_digest={}\nsource_manifest_sha256={}\ntest_manifest_sha256={}\nartifact_sha256=none\nstatus=passed\nrecorded_at_unix_ms=42\nauthority=external_build_evidence_no_activation_authority\n",
        sha256_hex(b"candidate"),
        fixture.attestation.expected_manifest_sha256,
        sha256_hex(b"tests")
    );
    let record = format!("{body}evidence_sha256={}\n", sha256_hex(body.as_bytes()));
    let evidence_path = fixture
        .candidates
        .join("build-evidence/build_build-1.evidence");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&evidence_path)
        .expect("create private evidence");
    file.write_all(record.as_bytes()).expect("write evidence");
    file.sync_all().expect("sync evidence");
    let evidence = broker
        .read_build_evidence(&BuildEvidenceRequest {
            build_id: "build-1".to_string(),
        })
        .expect("read build evidence");
    assert_eq!(evidence.status, "passed");
    assert!(evidence.artifact_sha256.is_none());

    fs::set_permissions(&evidence_path, fs::Permissions::from_mode(0o644)).expect("widen evidence");
    assert!(matches!(
        broker.read_build_evidence(&BuildEvidenceRequest {
            build_id: "build-1".to_string(),
        }),
        Err(BrokerError::SecurityViolation(_))
    ));
}

fn exact_attestation(candidate_id: &str, candidate_digest: &str) -> ExactSubmissionAttestationV1 {
    ExactSubmissionAttestationV1 {
        schema: SUBMISSION_ATTESTATION_SCHEMA_V1.to_string(),
        provenance: "exact_model".to_string(),
        instance_id: "avado-astrid".to_string(),
        trace_id: "trace-1".to_string(),
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        response_sha256: sha256_hex(b"response"),
        terminal_declaration_sha256: sha256_hex(b"terminal declaration"),
        candidate_id: candidate_id.to_string(),
        candidate_digest: candidate_digest.to_string(),
        model_id: "qwen3.5:4b".to_string(),
        authored_at_unix_ms: 1,
    }
}

fn assert_unsafe_source(label: &str, populate: impl FnOnce(&Path)) {
    let temporary = TestDirectory::new(label);
    let source = temporary.source();
    fs::create_dir(&source).expect("create unsafe source fixture");
    populate(&source);
    assert!(compute_source_manifest_sha256(&source, "source-unsafe").is_err());
}
