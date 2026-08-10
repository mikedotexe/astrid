//! Typed, authority-free request and result contracts for the local source broker.

use std::{fmt, io, path::PathBuf};

pub const SOURCE_BINDING_SCHEMA_V1: &str = "astrid.edge.signed_source_binding.v1";
pub const BUILD_EVIDENCE_SCHEMA_V1: &str = "astrid.edge.local_build_evidence.v1";
pub const SUBMISSION_ATTESTATION_SCHEMA_V1: &str =
    "astrid.edge.exact_candidate_submission_attestation.v1";
pub const CANDIDATE_STATE_SCHEMA_V1: &str = "astrid.edge.local_candidate_state.v1";
pub const CANDIDATE_SUBMISSION_SCHEMA_V1: &str = "astrid.edge.local_candidate_submission.v1";

pub const MAX_SOURCE_FILES: usize = 4_096;
pub const MAX_SOURCE_FILE_BYTES: u64 = 1_048_576;
pub const MAX_SOURCE_TOTAL_BYTES: u64 = 67_108_864;
pub const MAX_LIST_RESULTS: usize = 50;
pub const MAX_SEARCH_FILES: usize = 128;
pub const MAX_SEARCH_MATCHES: usize = 20;
pub const MAX_SEARCH_QUERY_CHARS: usize = 120;
pub const MAX_EXCERPT_CHARS: usize = 240;
pub const MAX_CHUNK_LINES: usize = 200;
pub const MAX_CHUNK_CHARS: usize = 32_000;
pub const MAX_PATCH_BYTES: usize = 1_048_576;
pub const MAX_CANDIDATE_FILES: usize = 25;
pub const MAX_CHANGED_LINES: usize = 4_000;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BrokerError {
    InvalidInput(&'static str),
    InvalidValue(String),
    NotFound(&'static str),
    SecurityViolation(&'static str),
    LimitExceeded(&'static str),
    Stale(&'static str),
    Conflict(&'static str),
    Integrity(&'static str),
    Io(io::ErrorKind),
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::NotFound(message)
            | Self::SecurityViolation(message)
            | Self::LimitExceeded(message)
            | Self::Stale(message)
            | Self::Conflict(message)
            | Self::Integrity(message) => formatter.write_str(message),
            Self::InvalidValue(message) => formatter.write_str(message),
            Self::Io(kind) => write!(formatter, "local I/O failed: {kind:?}"),
        }
    }
}

impl std::error::Error for BrokerError {}

impl From<io::Error> for BrokerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

pub type BrokerResult<T> = Result<T, BrokerError>;

/// An upstream-verified signature binding. The broker checks the canonical payload digest and
/// syntax, but deliberately owns no verification key or trust policy.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedSourceBindingV1 {
    pub schema: String,
    pub signer_key_id: String,
    pub signature_hex: String,
    pub signed_payload_sha256: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedSourceRootV1 {
    pub root: PathBuf,
    pub expected_source_id: String,
    pub expected_manifest_sha256: String,
    pub binding: SignedSourceBindingV1,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ListSourceRequest {
    pub cursor: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceEntry {
    pub source_file_id: String,
    pub basename: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub line_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ListSourceResult {
    pub source_id: String,
    pub manifest_sha256: String,
    pub entries: Vec<SourceEntry>,
    pub next_cursor: Option<usize>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchSourceRequest {
    pub query: String,
    pub cursor: usize,
    pub max_files: usize,
    pub max_matches: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchMatch {
    pub source_file_id: String,
    pub basename: String,
    pub line_number: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchSourceResult {
    pub source_id: String,
    pub manifest_sha256: String,
    pub matches: Vec<SearchMatch>,
    pub next_cursor: Option<usize>,
    pub files_considered: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadSourceChunkRequest {
    pub source_file_id: String,
    pub start_line: usize,
    pub max_lines: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NumberedLine {
    pub line_number: usize,
    pub text: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadSourceChunkResult {
    pub source_file_id: String,
    pub basename: String,
    pub sha256: String,
    pub lines: Vec<NumberedLine>,
    pub next_line: Option<usize>,
    pub truncated_by_character_limit: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BeginCandidateRequest {
    pub candidate_id: String,
    pub base_generation: String,
    pub proposal_sha256: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CandidatePatchRequest {
    pub candidate_id: String,
    pub source_file_id: String,
    pub expected_old_sha256: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FormatCandidateRequest {
    pub candidate_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InspectCandidateRequest {
    pub candidate_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AbandonCandidateRequest {
    pub candidate_id: String,
    pub expected_candidate_digest: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CandidateChange {
    pub source_file_id: String,
    pub basename: String,
    pub old_sha256: String,
    pub new_sha256: String,
    pub changed_lines: usize,
    pub formatted: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CandidateInspection {
    pub schema: String,
    pub candidate_id: String,
    pub source_id: String,
    pub source_manifest_sha256: String,
    pub base_generation: String,
    pub proposal_sha256: String,
    pub revision: u64,
    pub status: String,
    pub candidate_digest: String,
    pub changed_lines: usize,
    pub changes: Vec<CandidateChange>,
    pub authority: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GenerationDiffRequest {
    pub candidate_id: String,
    pub source_file_id: String,
    pub start_line: usize,
    pub max_lines: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GenerationDiffResult {
    pub candidate_id: String,
    pub candidate_digest: String,
    pub source_file_id: String,
    pub basename: String,
    pub old_sha256: String,
    pub new_sha256: String,
    pub before: Vec<NumberedLine>,
    pub after: Vec<NumberedLine>,
    pub next_line: Option<usize>,
    pub authority: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BuildEvidenceRequest {
    pub build_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BuildEvidence {
    pub schema: String,
    pub build_id: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub source_manifest_sha256: String,
    pub test_manifest_sha256: String,
    pub artifact_sha256: Option<String>,
    pub status: String,
    pub recorded_at_unix_ms: u64,
    pub evidence_sha256: String,
    pub authority: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExactSubmissionAttestationV1 {
    pub schema: String,
    pub provenance: String,
    pub instance_id: String,
    pub trace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub response_sha256: String,
    pub terminal_declaration_sha256: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub model_id: String,
    pub authored_at_unix_ms: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubmitCandidateRequest {
    pub candidate_id: String,
    pub expected_candidate_digest: String,
    pub attestation: ExactSubmissionAttestationV1,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CandidateSubmissionReceipt {
    pub schema: String,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub submission_artifact: String,
    pub submission_sha256: String,
    pub attestation_sha256: String,
    pub authority: String,
}
