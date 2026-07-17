use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::witness::ProvenanceRefV1;

fn write_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => output.push_str(&Value::String(value.clone()).to_string()),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_json(value, output);
            }
            output.push(']');
        },
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            output.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&Value::String(key.clone()).to_string());
                output.push(':');
                write_canonical_json(&values[key], output);
            }
            output.push('}');
        },
    }
}

fn canonical_sha256(value: &Value) -> String {
    let mut encoded = String::new();
    write_canonical_json(value, &mut encoded);
    format!("{:x}", Sha256::digest(encoded.as_bytes()))
}

fn normalize_measurement_numbers(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                normalize_measurement_numbers(value);
            }
        },
        Value::Object(values) => {
            for value in values.values_mut() {
                normalize_measurement_numbers(value);
            }
        },
        Value::Number(number) if !number.is_i64() && !number.is_u64() => {
            *value = Value::String(number.to_string());
        },
        _ => {},
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalStageKindV1 {
    Authored,
    Chunked,
    Encoded,
    Narrative,
    Feedback,
    Breathing,
    Resonance,
    Visual,
    Delta,
    Hebbian,
    FrictionReview,
    SafetyReview,
    Blocked,
    Dispatched,
    DeliveryEvidence,
    MinimeTelemetryWindow,
    CaptureGap,
}

impl SignalStageKindV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authored => "authored",
            Self::Chunked => "chunked",
            Self::Encoded => "encoded",
            Self::Narrative => "narrative",
            Self::Feedback => "feedback",
            Self::Breathing => "breathing",
            Self::Resonance => "resonance",
            Self::Visual => "visual",
            Self::Delta => "delta",
            Self::Hebbian => "hebbian",
            Self::FrictionReview => "friction_review",
            Self::SafetyReview => "safety_review",
            Self::Blocked => "blocked",
            Self::Dispatched => "dispatched",
            Self::DeliveryEvidence => "delivery_evidence",
            Self::MinimeTelemetryWindow => "minime_telemetry_window",
            Self::CaptureGap => "capture_gap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalOwnershipDomainV1 {
    AstridAuthored,
    BridgeCodec,
    BridgeEvidence,
    BridgeSafety,
    BridgeDispatch,
    MinimeObserved,
}

impl SignalOwnershipDomainV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AstridAuthored => "astrid_authored",
            Self::BridgeCodec => "bridge_codec",
            Self::BridgeEvidence => "bridge_evidence",
            Self::BridgeSafety => "bridge_safety",
            Self::BridgeDispatch => "bridge_dispatch",
            Self::MinimeObserved => "minime_observed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalRelationV1 {
    Root,
    ExactTransformation,
    ExactReview,
    SafetyDecision,
    DispatchOutcome,
    DeliveryEvidence,
    TemporalAssociation,
}

impl SignalRelationV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::ExactTransformation => "exact_transformation",
            Self::ExactReview => "exact_review",
            Self::SafetyDecision => "safety_decision",
            Self::DispatchOutcome => "dispatch_outcome",
            Self::DeliveryEvidence => "delivery_evidence",
            Self::TemporalAssociation => "temporal_association_not_direct_causation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalEffectV1 {
    Produced,
    Applied,
    NotApplied,
    Reviewed,
    Allowed,
    Blocked,
    Dispatched,
    DispatchFailed,
    EvidenceRecorded,
    TemporallyAssociated,
    CaptureGap,
}

impl SignalEffectV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Produced => "produced",
            Self::Applied => "applied",
            Self::NotApplied => "not_applied",
            Self::Reviewed => "reviewed",
            Self::Allowed => "allowed",
            Self::Blocked => "blocked",
            Self::Dispatched => "dispatched",
            Self::DispatchFailed => "dispatch_failed",
            Self::EvidenceRecorded => "evidence_recorded",
            Self::TemporallyAssociated => "temporally_associated",
            Self::CaptureGap => "capture_gap",
        }
    }
}

/// Unified wall-clock, source-clock, and process-monotonic timing.
///
/// Construction remains private to the spine so persisted values cannot be
/// read back and mistaken for trusted runtime time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignalTemporalEnvelopeV1 {
    source_time_ms: Option<u64>,
    arrival_time_unix_ms: u64,
    stage_time_unix_ms: u64,
    monotonic_time_ns: u64,
    connection_id: String,
    connection_sequence: u64,
    capture_window_ref: Option<String>,
}

impl SignalTemporalEnvelopeV1 {
    pub(super) fn new(
        source_time_ms: Option<u64>,
        arrival_time_unix_ms: u64,
        stage_time_unix_ms: u64,
        monotonic_time_ns: u64,
        connection_id: String,
        connection_sequence: u64,
        capture_window_ref: Option<String>,
    ) -> Self {
        Self {
            source_time_ms,
            arrival_time_unix_ms,
            stage_time_unix_ms,
            monotonic_time_ns,
            connection_id,
            connection_sequence,
            capture_window_ref,
        }
    }

    #[must_use]
    pub const fn source_time_ms(&self) -> Option<u64> {
        self.source_time_ms
    }

    #[must_use]
    pub const fn arrival_time_unix_ms(&self) -> u64 {
        self.arrival_time_unix_ms
    }

    #[must_use]
    pub const fn stage_time_unix_ms(&self) -> u64 {
        self.stage_time_unix_ms
    }

    #[must_use]
    pub const fn monotonic_time_ns(&self) -> u64 {
        self.monotonic_time_ns
    }

    #[must_use]
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    #[must_use]
    pub const fn connection_sequence(&self) -> u64 {
        self.connection_sequence
    }

    #[must_use]
    pub fn capture_window_ref(&self) -> Option<&str> {
        self.capture_window_ref.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignalProcessIdentityV1 {
    pid: u32,
    executable: String,
    deployment_identity: String,
}

impl SignalProcessIdentityV1 {
    pub(super) fn current(deployment_identity: String) -> Self {
        Self {
            pid: std::process::id(),
            executable: super::recorder::executable_name().to_string(),
            deployment_identity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignalCaptureFixtureRefV1 {
    capture_window_id: String,
    fixture_sha256: String,
    relative_path: String,
    vector_dimensions: usize,
}

impl SignalCaptureFixtureRefV1 {
    pub(super) fn new(
        capture_window_id: String,
        fixture_sha256: String,
        relative_path: String,
        vector_dimensions: usize,
    ) -> Self {
        Self {
            capture_window_id,
            fixture_sha256,
            relative_path,
            vector_dimensions,
        }
    }
}

/// Bounded persisted metadata for one trusted stage.
///
/// Deliberately `Serialize`-only: disk receipts must pass through the
/// untrusted verifier before they can inform a new trusted journey.
#[derive(Debug, Clone, Serialize)]
pub struct SignalStageReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    journey_id: String,
    stage_id: String,
    stage_index: u32,
    stage_kind: SignalStageKindV1,
    relation: SignalRelationV1,
    effect: SignalEffectV1,
    ownership_domain: SignalOwnershipDomainV1,
    parent_stage_ids: Vec<String>,
    source_sha256: String,
    output_sha256: String,
    provenance_ref_v1: ProvenanceRefV1,
    process_identity_v1: SignalProcessIdentityV1,
    temporal_envelope_v1: SignalTemporalEnvelopeV1,
    measurements: BTreeMap<String, Value>,
    capture_fixture_ref_v1: Option<SignalCaptureFixtureRefV1>,
    raw_response_prose_included: bool,
    live_control_authority: bool,
    receipt_integrity_sha256: String,
}

impl SignalStageReceiptV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        journey_id: String,
        stage_id: String,
        stage_index: u32,
        stage_kind: SignalStageKindV1,
        relation: SignalRelationV1,
        effect: SignalEffectV1,
        ownership_domain: SignalOwnershipDomainV1,
        parent_stage_ids: Vec<String>,
        source_sha256: String,
        output_sha256: String,
        provenance_ref_v1: ProvenanceRefV1,
        process_identity_v1: SignalProcessIdentityV1,
        temporal_envelope_v1: SignalTemporalEnvelopeV1,
        mut measurements: BTreeMap<String, Value>,
        capture_fixture_ref_v1: Option<SignalCaptureFixtureRefV1>,
    ) -> Self {
        // JSON floating-point canonicalization differs subtly across runtimes.
        // Measurements are evidence metadata, so retain exact decimal text
        // while keeping the signed receipt language-independent.
        for value in measurements.values_mut() {
            normalize_measurement_numbers(value);
        }
        let mut receipt = Self {
            schema: "signal_stage_receipt_v1",
            schema_version: 1,
            journey_id,
            stage_id,
            stage_index,
            stage_kind,
            relation,
            effect,
            ownership_domain,
            parent_stage_ids,
            source_sha256,
            output_sha256,
            provenance_ref_v1,
            process_identity_v1,
            temporal_envelope_v1,
            measurements,
            capture_fixture_ref_v1,
            raw_response_prose_included: false,
            live_control_authority: false,
            receipt_integrity_sha256: String::new(),
        };
        receipt.receipt_integrity_sha256 = receipt.calculated_integrity_sha256();
        receipt
    }

    #[must_use]
    pub fn stage_id(&self) -> &str {
        &self.stage_id
    }

    #[must_use]
    pub const fn stage_kind(&self) -> SignalStageKindV1 {
        self.stage_kind
    }

    #[must_use]
    pub const fn relation(&self) -> SignalRelationV1 {
        self.relation
    }

    #[must_use]
    pub fn parent_stage_ids(&self) -> &[String] {
        &self.parent_stage_ids
    }

    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    #[must_use]
    pub fn output_sha256(&self) -> &str {
        &self.output_sha256
    }

    pub(super) fn set_capture_fixture_ref(&mut self, fixture_ref: SignalCaptureFixtureRefV1) {
        self.capture_fixture_ref_v1 = Some(fixture_ref);
        self.receipt_integrity_sha256 = self.calculated_integrity_sha256();
    }

    fn calculated_integrity_sha256(&self) -> String {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(fields) = &mut value {
            fields.remove("receipt_integrity_sha256");
        }
        canonical_sha256(&value)
    }

    #[cfg(test)]
    pub(super) fn integrity_valid(&self) -> bool {
        self.receipt_integrity_sha256 == self.calculated_integrity_sha256()
    }
}

#[cfg(test)]
mod canonical_tests {
    use serde_json::json;

    use super::canonical_sha256;

    #[test]
    fn canonical_hash_matches_python_projector_fixture() {
        let fixture = json!({
            "zeta": {"s": "line\n", "n": 7},
            "alpha": [1, true, null, "é"],
        });
        assert_eq!(
            canonical_sha256(&fixture),
            "118fe7607c342d93dbffa5bcd0d0410cec4c6e7e39088935c075919d96aae129"
        );
    }
}

/// A trusted in-memory stage. Its payload never enters the persisted receipt.
#[derive(Debug, Clone)]
pub struct CausalSignalStageV1<T> {
    receipt: SignalStageReceiptV1,
    value: T,
}

impl<T> CausalSignalStageV1<T> {
    pub(super) fn new(receipt: SignalStageReceiptV1, value: T) -> Self {
        Self { receipt, value }
    }

    #[must_use]
    pub const fn receipt(&self) -> &SignalStageReceiptV1 {
        &self.receipt
    }

    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}

/// Trusted in-memory journey. Only `receipts` are serialized by its bounded
/// projection; stage payloads remain transient.
#[derive(Debug)]
pub struct CausalSignalJourneyV1 {
    journey_id: String,
    receipts: Vec<SignalStageReceiptV1>,
    stage_ids: HashSet<String>,
}

impl CausalSignalJourneyV1 {
    pub(super) fn new(journey_id: String) -> Self {
        Self {
            journey_id,
            receipts: Vec::new(),
            stage_ids: HashSet::new(),
        }
    }

    pub(super) fn push(&mut self, receipt: SignalStageReceiptV1) -> Result<(), String> {
        if self.stage_ids.contains(receipt.stage_id()) {
            return Err(format!("duplicate stage id {}", receipt.stage_id()));
        }
        if receipt.relation() == SignalRelationV1::Root {
            if !receipt.parent_stage_ids().is_empty() {
                return Err("root stage must not have parents".to_string());
            }
        } else {
            if receipt.parent_stage_ids().is_empty() {
                return Err("non-root stage must have at least one parent".to_string());
            }
            for parent in receipt.parent_stage_ids() {
                if !self.stage_ids.contains(parent) {
                    return Err(format!("unknown or forward parent stage {parent}"));
                }
            }
        }
        self.stage_ids.insert(receipt.stage_id().to_string());
        self.receipts.push(receipt);
        Ok(())
    }

    #[must_use]
    pub fn journey_id(&self) -> &str {
        &self.journey_id
    }

    #[must_use]
    pub fn receipts(&self) -> &[SignalStageReceiptV1] {
        &self.receipts
    }

    pub(super) fn receipts_mut(&mut self) -> &mut [SignalStageReceiptV1] {
        &mut self.receipts
    }

    pub fn validate_parent_chain(&self) -> Result<(), String> {
        let mut seen: HashSet<String> = HashSet::new();
        for receipt in &self.receipts {
            if receipt.relation() == SignalRelationV1::Root {
                if !receipt.parent_stage_ids().is_empty() {
                    return Err(format!("root {} has parents", receipt.stage_id()));
                }
            } else if receipt.parent_stage_ids().is_empty() {
                return Err(format!("stage {} has no parent", receipt.stage_id()));
            }
            for parent in receipt.parent_stage_ids() {
                if !seen.contains(parent.as_str()) {
                    return Err(format!(
                        "stage {} has unknown or forward parent {parent}",
                        receipt.stage_id()
                    ));
                }
            }
            if !seen.insert(receipt.stage_id().to_string()) {
                return Err(format!("duplicate stage {}", receipt.stage_id()));
            }
        }
        Ok(())
    }
}
