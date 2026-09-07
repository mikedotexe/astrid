//! Durable saved-text reader contracts. Byte positions always refer to retained UTF-8.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const READER_SOURCE_MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const READER_PASSAGE_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderBookmarkVersion {
    pub revision: u64,
    /// Also guards against an ordinary session lifecycle update.
    pub session_record_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderSourceSnapshot {
    pub original_path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
    /// Relative to the action-continuity store, never an expiring overflow file.
    pub retained_artifact: PathBuf,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderUtf8Cursor {
    pub next_byte: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderPassage {
    pub offer_id: String,
    pub source_sha256: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub sha256: String,
}

/// The caller must verify its final request and retain the completed output before
/// supplying this evidence. Public fields are not proof of submission: the store
/// validates identities, hash shapes and supplied bytes against the saved source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderDeliveryEvidence {
    pub offer_id: String,
    pub final_request_sha256: String,
    pub supplied_start_byte: u64,
    pub supplied_end_byte: u64,
    pub supplied_sha256: String,
    pub retained_output_ref: String,
    pub retained_output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderCommittedPassage {
    pub passage: ReaderPassage,
    pub delivery: ReaderDeliveryEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderDisposition {
    Active,
    Parked,
    Complete,
    Abandoned,
}

impl ReaderDisposition {
    pub(super) fn session_status(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Parked => "parked",
            Self::Complete => "complete",
            Self::Abandoned => "abandoned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderBookmark {
    pub schema_version: u32,
    pub activity_id: String,
    pub session_id: String,
    pub revision: u64,
    pub disposition: ReaderDisposition,
    pub source: ReaderSourceSnapshot,
    pub cursor: ReaderUtf8Cursor,
    pub last_committed_passage: Option<ReaderCommittedPassage>,
    pub offered_passage: Option<ReaderPassage>,
    /// Existing authored context may be carried; mechanical progress adds none.
    pub authored_focus: Option<String>,
    pub stopping_note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderCurrentSourceStatus {
    Unchanged,
    Changed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReaderSourceComparison {
    pub retained_source_available: bool,
    pub retained_source_error: Option<String>,
    pub current_status: ReaderCurrentSourceStatus,
    pub current_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderBookmarkPreview {
    pub session_id: String,
    pub session_record_id: String,
    pub session_status: String,
    pub bookmark: Option<ReaderBookmark>,
    pub source_comparison: Option<ReaderSourceComparison>,
    pub offered_text: Option<String>,
}

impl ReaderBookmarkPreview {
    #[must_use]
    pub fn version(&self) -> Option<ReaderBookmarkVersion> {
        self.bookmark
            .as_ref()
            .map(|bookmark| ReaderBookmarkVersion {
                revision: bookmark.revision,
                session_record_id: self.session_record_id.clone(),
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderBookmarkReceipt {
    pub operation_id: String,
    pub record_id: String,
    pub version: ReaderBookmarkVersion,
    pub bookmark: ReaderBookmark,
    pub duplicate: bool,
    pub primary_record_durable: bool,
    /// No memory-card or thread projection is needed to preserve mechanical progress.
    pub derived_projection_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderAppendStage {
    PartialOrUnknown,
    AppendedNotSynced,
}

#[derive(Debug)]
pub struct ReaderPersistenceError {
    pub operation_id: String,
    pub record_id: String,
    pub stage: ReaderAppendStage,
    pub detail: String,
}

impl std::fmt::Display for ReaderPersistenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "reader operation {} record {}: {:?}: {}",
            self.operation_id, self.record_id, self.stage, self.detail
        )
    }
}

impl std::error::Error for ReaderPersistenceError {}
