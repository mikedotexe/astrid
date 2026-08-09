//! Pure local broker for a signed source snapshot and one bounded private candidate draft.
//!
//! The caller supplies and verifies the source-root signature. This module checks that the
//! canonical signed payload, source identity, and full source manifest remain exact. It has no
//! process, shell, network, build, install, activation, or execution capability. Submission
//! creates an owner-only intent artifact; a separate authority must decide whether to act on it.

#[path = "source_tools/candidate.rs"]
mod candidate;
#[path = "source_tools/digest.rs"]
mod digest;
#[path = "source_tools/fs.rs"]
mod fs;
#[path = "source_tools/source.rs"]
mod source;

pub use types::*;
#[path = "source_tools/types.rs"]
mod types;

use std::path::{Path, PathBuf};

use fs::{SourceIndex, ensure_private_directory};

/// A source-bound, authority-free broker. File-system paths never cross its typed API.
#[derive(Debug)]
pub struct SourceCandidateBroker {
    source: SourceIndex,
    candidate_root: PathBuf,
}

impl SourceCandidateBroker {
    /// Opens a full attested snapshot and a disjoint private candidate root.
    ///
    /// Signature verification belongs to the caller. The broker validates the canonical payload
    /// digest and signature encoding so a verified binding cannot be accidentally rebound.
    ///
    /// # Errors
    ///
    /// Returns an error when either root is unsafe, overlapping, stale, incorrectly bound, or
    /// inaccessible.
    pub fn open(source: &SignedSourceRootV1, candidate_root: PathBuf) -> BrokerResult<Self> {
        validate_root_locations(&source.root, &candidate_root)?;
        let source = SourceIndex::load(source)?;
        candidate::initialize_candidate_root(&candidate_root)?;
        ensure_disjoint_canonical_roots(&source.root, &candidate_root)?;
        Ok(Self {
            source,
            candidate_root,
        })
    }

    /// Lists a bounded page of opaque source IDs and metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid bound/cursor or a source snapshot change.
    pub fn list_source(&self, request: &ListSourceRequest) -> BrokerResult<ListSourceResult> {
        let source = self.fresh_source()?;
        source::list_source(&source, request)
    }

    /// Searches bounded UTF-8 source text using a case-insensitive literal query.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds, unsafe source content, or a stale snapshot.
    pub fn search_source(&self, request: &SearchSourceRequest) -> BrokerResult<SearchSourceResult> {
        let source = self.fresh_source()?;
        source::search_source(&source, request)
    }

    /// Reads a bounded, numbered chunk by opaque source file ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID, invalid bounds, or a stale source snapshot.
    pub fn read_source_chunk(
        &self,
        request: &ReadSourceChunkRequest,
    ) -> BrokerResult<ReadSourceChunkResult> {
        let source = self.fresh_source()?;
        source::read_source_chunk(&source, request)
    }

    /// Begins the sole active private candidate draft.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identity data, stale source, or an existing active draft.
    pub fn begin_candidate(
        &self,
        request: &BeginCandidateRequest,
    ) -> BrokerResult<CandidateInspection> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::begin_candidate(&source, &self.candidate_root, request)
    }

    /// Applies a full-content replacement bound to the exact old source digest.
    ///
    /// # Errors
    ///
    /// Returns an error for stale hashes, unsafe text, exceeded limits, or a non-active draft.
    pub fn apply_candidate_patch(
        &self,
        request: &CandidatePatchRequest,
    ) -> BrokerResult<CandidateInspection> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::apply_candidate_patch(&source, &self.candidate_root, request)
    }

    /// Applies deterministic newline normalization without running an external formatter.
    ///
    /// # Errors
    ///
    /// Returns an error when the draft or source changed or formatting exceeds candidate limits.
    pub fn format_candidate(
        &self,
        request: &FormatCandidateRequest,
    ) -> BrokerResult<CandidateInspection> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::format_candidate(&source, &self.candidate_root, request)
    }

    /// Verifies and reports an existing candidate without exposing paths or replacement bodies.
    ///
    /// # Errors
    ///
    /// Returns an error when the candidate, replacement, or source binding fails verification.
    pub fn inspect_candidate(
        &self,
        request: &InspectCandidateRequest,
    ) -> BrokerResult<CandidateInspection> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::inspect_candidate(&source, &self.candidate_root, request)
    }

    /// Marks an exact active draft abandoned while preserving its evidence on disk.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, digest, state, or source verification fails.
    pub fn abandon_candidate(
        &self,
        request: &AbandonCandidateRequest,
    ) -> BrokerResult<CandidateInspection> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::abandon_candidate(&source, &self.candidate_root, request)
    }

    /// Writes a submission-intent artifact bound to an exact model-authored attestation.
    ///
    /// # Errors
    ///
    /// Returns an error for unverifiable state, non-exact provenance, or a stale attestation.
    pub fn submit_candidate(
        &self,
        request: &SubmitCandidateRequest,
    ) -> BrokerResult<CandidateSubmissionReceipt> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::submit_candidate(&source, &self.candidate_root, request)
    }

    /// Reads a bounded before/after view of one candidate replacement.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds, unknown IDs, tampering, or stale source state.
    pub fn read_generation_diff(
        &self,
        request: &GenerationDiffRequest,
    ) -> BrokerResult<GenerationDiffResult> {
        let source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::read_generation_diff(&source, &self.candidate_root, request)
    }

    /// Reads deterministic, owner-only build evidence produced by a separate authority.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, widened, hard-linked, or digest-invalid evidence.
    pub fn read_build_evidence(
        &self,
        request: &BuildEvidenceRequest,
    ) -> BrokerResult<BuildEvidence> {
        let _source = self.fresh_source()?;
        self.verify_candidate_root()?;
        candidate::read_build_evidence(&self.candidate_root, request)
    }

    fn fresh_source(&self) -> BrokerResult<SourceIndex> {
        self.source.verify_current()
    }

    fn verify_candidate_root(&self) -> BrokerResult<()> {
        ensure_private_directory(&self.candidate_root)?;
        ensure_disjoint_canonical_roots(&self.source.root, &self.candidate_root)
    }
}

/// Computes the deterministic manifest expected by [`SignedSourceRootV1`].
///
/// # Errors
///
/// Returns an error when the source tree is unsafe, non-textual, oversized, or inaccessible.
pub fn compute_source_manifest_sha256(root: &Path, source_id: &str) -> BrokerResult<String> {
    fs::compute_source_manifest_sha256(root, source_id)
}

/// Computes the exact payload digest an upstream signature must cover.
///
/// # Errors
///
/// Returns an error when the source identity or manifest digest is malformed.
pub fn signed_source_payload_sha256(
    source_id: &str,
    manifest_sha256: &str,
) -> BrokerResult<String> {
    fs::signed_binding_payload_sha256(source_id, manifest_sha256)
}

fn validate_root_locations(source_root: &Path, candidate_root: &Path) -> BrokerResult<()> {
    if !source_root.is_absolute() || !candidate_root.is_absolute() {
        return Err(BrokerError::InvalidInput(
            "source and candidate roots must be absolute",
        ));
    }
    let candidate_name = candidate_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(BrokerError::SecurityViolation(
            "candidate root name is invalid",
        ))?;
    fs::validate_component(candidate_name)?;
    let candidate_parent = candidate_root
        .parent()
        .ok_or(BrokerError::SecurityViolation(
            "candidate root has no parent",
        ))?
        .canonicalize()?;
    let source = source_root.canonicalize()?;
    let candidate_target = candidate_parent.join(candidate_name);
    if candidate_target.starts_with(&source) || source.starts_with(&candidate_target) {
        return Err(BrokerError::SecurityViolation(
            "source and candidate roots must be disjoint",
        ));
    }
    Ok(())
}

fn ensure_disjoint_canonical_roots(source_root: &Path, candidate_root: &Path) -> BrokerResult<()> {
    let source = source_root.canonicalize()?;
    let candidate = candidate_root.canonicalize()?;
    if candidate.starts_with(&source) || source.starts_with(&candidate) {
        return Err(BrokerError::SecurityViolation(
            "source and candidate roots must be disjoint",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_tools/tests.rs"]
mod tests;
