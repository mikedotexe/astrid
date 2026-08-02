use std::fmt;

use astrid_minime_protocol::SpectralSubstrateV1;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{CorrelationSummary, RollingSpectralSummary, ScalarSummary};

pub const NON_CAUSAL_SPECTRAL_EVIDENCE_POLICY_V1: &str = "non_causal_spectral_evidence_v1";
const NON_CAUSAL_AUTHORITY: &str = "deterministic_machine_evidence_non_causal_no_control_authority";
const MAX_QUESTION_CHARS: usize = 512;
const MAX_SERIES_LABEL_CHARS: usize = 64;
const MAX_PROVENANCE_HASHES: usize = 16;

/// States how a caller joined an activity series to spectral observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationAttributionV1 {
    /// Joined using an exact trace/session/chain/response identifier.
    ExactIdentifierJoin,
    /// Aggregate series with no claim that individual events share a cause.
    AggregateUnattributed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrelationEvidenceV1 {
    pub left_series: String,
    pub right_series: String,
    pub correlation: CorrelationSummary,
    pub attribution: CorrelationAttributionV1,
}

/// Hash-bound deterministic evidence. Its authority string is deliberately
/// machine/non-causal and grants no Action or reservoir-control capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonCausalSpectralEvidenceV1 {
    pub policy: String,
    pub schema_version: u8,
    pub authority: String,
    pub question: String,
    pub substrate: SpectralSubstrateV1,
    pub summary: RollingSpectralSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<CorrelationEvidenceV1>,
    pub provenance_sha256: Vec<String>,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceError {
    InvalidQuestion,
    InvalidSubstrate,
    InvalidSummary,
    InvalidCorrelation,
    MissingProvenance,
    TooManyProvenanceHashes,
    InvalidProvenanceHash,
    Serialization,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidQuestion => "question must be non-empty and at most 512 characters",
            Self::InvalidSubstrate => "spectral substrate metadata is not well formed",
            Self::InvalidSummary => "spectral summary is empty, inconsistent, or non-finite",
            Self::InvalidCorrelation => "correlation metadata is malformed or non-finite",
            Self::MissingProvenance => "at least one provenance SHA-256 is required",
            Self::TooManyProvenanceHashes => "at most 16 provenance hashes are accepted",
            Self::InvalidProvenanceHash => "provenance hashes must be 64 hexadecimal characters",
            Self::Serialization => "evidence commitment could not be serialized",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EvidenceError {}

#[derive(Serialize)]
struct EvidenceCommitment<'a> {
    policy: &'a str,
    schema_version: u8,
    authority: &'a str,
    question: &'a str,
    substrate: &'a SpectralSubstrateV1,
    summary: &'a RollingSpectralSummary,
    correlation: &'a Option<CorrelationEvidenceV1>,
    provenance_sha256: &'a [String],
}

impl NonCausalSpectralEvidenceV1 {
    /// Creates deterministic evidence after validating every bounded field.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError`] when metadata is malformed, non-finite,
    /// unbounded, lacks provenance, or cannot be serialized canonically.
    pub fn new(
        question: impl Into<String>,
        substrate: SpectralSubstrateV1,
        summary: RollingSpectralSummary,
        correlation: Option<CorrelationEvidenceV1>,
        mut provenance_sha256: Vec<String>,
    ) -> Result<Self, EvidenceError> {
        provenance_sha256.sort_unstable();
        provenance_sha256.dedup();
        let mut evidence = Self {
            policy: NON_CAUSAL_SPECTRAL_EVIDENCE_POLICY_V1.to_string(),
            schema_version: 1,
            authority: NON_CAUSAL_AUTHORITY.to_string(),
            question: question.into(),
            substrate,
            summary,
            correlation,
            provenance_sha256,
            evidence_sha256: String::new(),
        };
        evidence.validate_fields()?;
        evidence.evidence_sha256 = evidence.canonical_sha256()?;
        Ok(evidence)
    }

    /// Verifies both schema invariants and the commitment hash.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.policy == NON_CAUSAL_SPECTRAL_EVIDENCE_POLICY_V1
            && self.schema_version == 1
            && self.authority == NON_CAUSAL_AUTHORITY
            && self.validate_fields().is_ok()
            && self
                .canonical_sha256()
                .is_ok_and(|hash| hash == self.evidence_sha256)
    }

    /// Hashes the evidence commitment, excluding its own hash field.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError::Serialization`] if a field cannot be encoded.
    pub fn canonical_sha256(&self) -> Result<String, EvidenceError> {
        let commitment = EvidenceCommitment {
            policy: &self.policy,
            schema_version: self.schema_version,
            authority: &self.authority,
            question: &self.question,
            substrate: &self.substrate,
            summary: &self.summary,
            correlation: &self.correlation,
            provenance_sha256: &self.provenance_sha256,
        };
        let bytes = serde_json::to_vec(&commitment).map_err(|_| EvidenceError::Serialization)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    fn validate_fields(&self) -> Result<(), EvidenceError> {
        let question = self.question.trim();
        if question.is_empty() || question.chars().count() > MAX_QUESTION_CHARS {
            return Err(EvidenceError::InvalidQuestion);
        }
        if !self.substrate.is_well_formed() {
            return Err(EvidenceError::InvalidSubstrate);
        }
        if !valid_summary(&self.summary) {
            return Err(EvidenceError::InvalidSummary);
        }
        if self
            .correlation
            .as_ref()
            .is_some_and(|correlation| !valid_correlation(correlation))
        {
            return Err(EvidenceError::InvalidCorrelation);
        }
        if self.provenance_sha256.is_empty() {
            return Err(EvidenceError::MissingProvenance);
        }
        if self.provenance_sha256.len() > MAX_PROVENANCE_HASHES {
            return Err(EvidenceError::TooManyProvenanceHashes);
        }
        if self
            .provenance_sha256
            .iter()
            .any(|hash| !valid_sha256(hash))
        {
            return Err(EvidenceError::InvalidProvenanceHash);
        }
        Ok(())
    }
}

fn valid_summary(summary: &RollingSpectralSummary) -> bool {
    let coverage_sample_count = [
        summary.full_spectrum_sample_count,
        summary.partial_spectrum_sample_count,
        summary.unknown_coverage_sample_count,
        summary.inconsistent_coverage_sample_count,
    ]
    .into_iter()
    .sum::<usize>();
    summary.sample_count > 0
        && summary.window_start_ms <= summary.window_end_ms
        && coverage_sample_count == summary.sample_count
        && [
            &summary.normalized_entropy,
            &summary.effective_modes,
            &summary.lambda1_share,
            &summary.head_share,
            &summary.shoulder_share,
            &summary.tail_share,
        ]
        .into_iter()
        .all(valid_scalar_summary)
        && summary
            .density_gradient
            .as_ref()
            .is_none_or(valid_scalar_summary)
        && summary
            .mode_turnover
            .as_ref()
            .is_none_or(valid_scalar_summary)
}

fn valid_scalar_summary(summary: &ScalarSummary) -> bool {
    summary.count > 0
        && [
            summary.min,
            summary.mean,
            summary.max,
            summary.standard_deviation,
            summary.first,
            summary.last,
            summary.change,
        ]
        .into_iter()
        .all(f64::is_finite)
        && summary.slope_per_sample.is_none_or(f64::is_finite)
        && summary.slope_per_second.is_none_or(f64::is_finite)
        && summary.min <= summary.mean
        && summary.mean <= summary.max
}

fn valid_correlation(correlation: &CorrelationEvidenceV1) -> bool {
    valid_label(&correlation.left_series)
        && valid_label(&correlation.right_series)
        && correlation.correlation.paired_count >= 2
        && correlation.correlation.coefficient.is_finite()
        && (-1.0..=1.0).contains(&correlation.correlation.coefficient)
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_SERIES_LABEL_CHARS
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_.:-".contains(character))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
