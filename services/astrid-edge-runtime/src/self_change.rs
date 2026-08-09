//! Pure, bounded domain model for proposed CPU-edge self changes.
//!
//! This module deliberately has no filesystem, process, network, service-control, or deployment
//! implementation. It validates proposals and records deterministic lifecycle transitions so a
//! later, separately-authorized broker can integrate without turning model-facing tools into an
//! ambient authority surface.

// The domain is intentionally landed before its authority-bearing integration points.
#![allow(dead_code, unused_imports)]

mod receipt;
mod schema;
mod state;
mod validation;

pub use receipt::{
    ReceiptAppendV1, ReceiptChainHeadV1, SelfChangeReceiptContentV1, SelfChangeReceiptV1,
    append_receipt, verify_receipt_chain,
};
pub use schema::{
    BuildEvidenceV1, BuildStateV1, CandidateFileChangeV1, CandidateLifecycleV1, CandidatePatchV1,
    CandidatePhaseV1, ChangeOperationV1, ExactModelAttestationV1, ExactModelProvenanceV1,
    ImmutablePathClassV1, ProbationEvidenceV1, ProbationStateV1, RollbackEvidenceV1,
    RollbackStateV1, ScheduledIntrospectionKindV1, ScheduledIntrospectionV1,
    SelfChangeCommandKindV1, SelfChangeCommandV1, SelfChangeDomainStateV1,
    SelfChangeReceiptDraftV1, TransitionActorV1, TransitionRequestV1,
};
pub use state::{apply_transition, derive_candidate_id, state_sha256};
pub use validation::{
    MAX_ATTESTATION_AGE_MS, MAX_CHANGED_FILES, MAX_CHANGED_LINES, classify_immutable_path,
    sha256_hex, validate_candidate_patch, validate_candidate_source_path, validate_source_id,
};

/// Stable validation/transition failure classes suitable for receipts and operator reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelfChangeError {
    InvalidSchema(&'static str),
    InvalidIdentifier(&'static str),
    InvalidHash(&'static str),
    InvalidAttestation(&'static str),
    InvalidSchedule(&'static str),
    InvalidPatch(&'static str),
    ImmutablePath(ImmutablePathClassV1),
    UnsupportedMutableSurface,
    LimitExceeded(&'static str),
    ActiveTransactionExists,
    NoActiveTransaction,
    CandidateMismatch,
    InvalidTransition,
    InvalidAuthority,
    ReplayCommand,
    ReplayAttestation,
    StaleStateHash,
    ReceiptChainMismatch,
    ArithmeticOverflow,
    Serialization(String),
}

impl std::fmt::Display for SelfChangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SelfChangeError {}

impl From<serde_json::Error> for SelfChangeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error.to_string())
    }
}

pub type SelfChangeResult<T> = Result<T, SelfChangeError>;

#[cfg(test)]
mod tests;
