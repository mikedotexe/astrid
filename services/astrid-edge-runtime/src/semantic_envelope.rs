//! Private, typed semantic admission for immutable-steward inquiry steps.
//!
//! Ordinary sensory IPC deliberately cannot construct this envelope.  It is
//! created only after the runtime has verified the immutable steward's signed
//! inquiry projection, then acknowledged by the reservoir after the exact
//! finite 48-dimensional vector has been applied.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use crate::codec::SEMANTIC_DIM;

pub(crate) const ENVELOPE_SCHEMA: &str = "astrid.edge.semantic_envelope.v1";
pub(crate) const ACK_SCHEMA: &str = "astrid.edge.semantic_admission_ack.v1";
pub(crate) const CODEC_VERSION: &str = "astrid.edge.feature_hash_48d.v1";
const ADMISSION_DOMAIN: &[u8] = b"astrid.edge.inquiry.admission.v1\0";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SemanticSourceClassV1 {
    ScheduledInquiry,
    EvidenceIntegration,
}

impl SemanticSourceClassV1 {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ScheduledInquiry => "scheduled_inquiry",
            Self::EvidenceIntegration => "evidence_integration",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticTraceV1 {
    pub(crate) schema_version: u8,
    pub(crate) trace_id: String,
    pub(crate) turn_id: String,
    pub(crate) span_id: String,
    pub(crate) session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) chain_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticEnvelopeV1 {
    pub(crate) schema: String,
    pub(crate) source_class: SemanticSourceClassV1,
    /// Immutable steward ledger identity covered by its Ed25519 signature.
    pub(crate) signed_entry_id: String,
    /// Domain-separated identity covered by the signed inquiry projection.
    pub(crate) admission_id: String,
    pub(crate) summary_sha256: String,
    pub(crate) codec_version: String,
    pub(crate) trace: SemanticTraceV1,
    pub(crate) vector: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticAdmissionAckV1 {
    pub(crate) schema: String,
    pub(crate) admission_id: String,
    pub(crate) signed_entry_id: String,
    pub(crate) source_class: SemanticSourceClassV1,
    pub(crate) reservoir_generation: String,
    /// Reservoir sample sequence current at the exact application boundary.
    pub(crate) reservoir_sequence: u64,
    pub(crate) vector_sha256: String,
    pub(crate) accepted_at_unix_ms: u64,
    pub(crate) status: String,
}

impl SemanticEnvelopeV1 {
    pub(crate) fn validate(&self, appliance_id: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema == ENVELOPE_SCHEMA,
            "unsupported semantic envelope"
        );
        validate_identifier(&self.signed_entry_id, 96, "signed entry id")?;
        validate_prefixed_sha256(&self.admission_id, "inquiry-admission-", "admission id")?;
        validate_sha256(&self.summary_sha256, "summary hash")?;
        anyhow::ensure!(
            self.admission_id == derive_admission_id(appliance_id, &self.signed_entry_id),
            "semantic admission identity is not bound to the signed inquiry entry"
        );
        anyhow::ensure!(
            self.codec_version == CODEC_VERSION,
            "unsupported semantic codec version"
        );
        self.trace.validate()?;
        anyhow::ensure!(
            self.vector.len() == SEMANTIC_DIM && self.vector.iter().all(|value| value.is_finite()),
            "semantic envelope is not an exact finite 48D vector"
        );
        Ok(())
    }

    pub(crate) fn vector_sha256(&self) -> String {
        vector_sha256(&self.vector)
    }
}

impl SemanticTraceV1 {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == 1,
            "unsupported semantic trace schema"
        );
        validate_uuid(&self.trace_id, "trace id")?;
        validate_uuid(&self.turn_id, "turn id")?;
        validate_uuid(&self.span_id, "span id")?;
        validate_identifier(&self.session_id, 96, "session id")?;
        if let Some(chain_id) = &self.chain_id {
            validate_identifier(chain_id, 96, "chain id")?;
        }
        Ok(())
    }
}

pub(crate) fn derive_admission_id(appliance_id: &str, signed_entry_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ADMISSION_DOMAIN);
    hasher.update(appliance_id.as_bytes());
    hasher.update([0]);
    hasher.update(signed_entry_id.as_bytes());
    format!("inquiry-admission-{:x}", hasher.finalize())
}

pub(crate) fn vector_sha256(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"astrid.edge.semantic.vector.f32le.v1\0");
    hasher.update(
        u64::try_from(vector.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn validate_identifier(value: &str, maximum: usize, label: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value.len() <= maximum
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':')
            }),
        "{label} is invalid"
    );
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "{label} is not a canonical SHA-256 digest"
    );
    Ok(())
}

fn validate_prefixed_sha256(value: &str, prefix: &str, label: &str) -> anyhow::Result<()> {
    let digest = value
        .strip_prefix(prefix)
        .ok_or_else(|| anyhow::anyhow!("{label} has the wrong domain prefix"))?;
    validate_sha256(digest, label)
}

fn validate_uuid(value: &str, label: &str) -> anyhow::Result<()> {
    let parsed = Uuid::parse_str(value).map_err(|error| anyhow::anyhow!("{label}: {error}"))?;
    anyhow::ensure!(!parsed.is_nil(), "{label} must not be nil");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> SemanticEnvelopeV1 {
        let appliance = "avado-astrid";
        let entry = "inq-0000000000000001";
        SemanticEnvelopeV1 {
            schema: ENVELOPE_SCHEMA.to_owned(),
            source_class: SemanticSourceClassV1::ScheduledInquiry,
            signed_entry_id: entry.to_owned(),
            admission_id: derive_admission_id(appliance, entry),
            summary_sha256: format!("{:x}", Sha256::digest(b"summary")),
            codec_version: CODEC_VERSION.to_owned(),
            trace: SemanticTraceV1 {
                schema_version: 1,
                trace_id: Uuid::from_u128(1).to_string(),
                turn_id: Uuid::from_u128(2).to_string(),
                span_id: Uuid::from_u128(3).to_string(),
                session_id: "scheduled-session-1".to_owned(),
                chain_id: None,
            },
            vector: vec![0.1; SEMANTIC_DIM],
        }
    }

    #[test]
    fn exact_envelope_is_finite_bound_and_identity_bound() {
        let value = envelope();
        value.validate("avado-astrid").expect("valid envelope");
        assert_eq!(value.vector_sha256().len(), 64);
    }

    #[test]
    fn replay_under_another_appliance_and_non_finite_vector_fail_closed() {
        let mut value = envelope();
        assert!(value.validate("icp-astrid").is_err());
        value.vector[0] = f32::NAN;
        assert!(value.validate("avado-astrid").is_err());
    }

    #[test]
    fn vector_hash_binds_float_bits_and_order() {
        let mut first = vec![0.0; SEMANTIC_DIM];
        let mut second = first.clone();
        second[1] = -0.0;
        assert_ne!(vector_sha256(&first), vector_sha256(&second));
        first[0] = 1.0;
        second[0] = 1.0;
        assert_ne!(vector_sha256(&first), vector_sha256(&second));
    }
}
