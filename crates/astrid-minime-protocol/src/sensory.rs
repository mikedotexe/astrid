use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::{
    CompatibilityStatus, DivisionCommandV1, PROTOCOL_MAJOR, ProtocolHeaderV1, classify_protocol,
    current_protocol, telemetry_protocol,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensoryPacketV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ProtocolHeaderV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_v1: Option<DeliveryEnvelopeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutual_address_v1: Option<MutualAddressEnvelopeV1>,
    #[serde(flatten)]
    pub message: SensoryMsg,
}

impl SensoryPacketV1 {
    #[must_use]
    pub fn versioned(message: SensoryMsg) -> Self {
        Self {
            protocol: Some(current_protocol()),
            delivery_v1: None,
            mutual_address_v1: None,
            message,
        }
    }

    #[must_use]
    pub fn versioned_1_0(message: SensoryMsg) -> Self {
        Self {
            protocol: Some(telemetry_protocol()),
            delivery_v1: None,
            mutual_address_v1: None,
            message,
        }
    }

    #[must_use]
    pub const fn legacy(message: SensoryMsg) -> Self {
        Self {
            protocol: None,
            delivery_v1: None,
            mutual_address_v1: None,
            message,
        }
    }

    #[must_use]
    pub fn with_envelopes(
        message: SensoryMsg,
        delivery_v1: DeliveryEnvelopeV1,
        mutual_address_v1: Option<MutualAddressEnvelopeV1>,
    ) -> Self {
        Self {
            protocol: Some(current_protocol()),
            delivery_v1: Some(delivery_v1),
            mutual_address_v1,
            message,
        }
    }

    #[must_use]
    pub fn compatibility(&self) -> CompatibilityStatus {
        classify_protocol(self.protocol.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryEnvelopeV1 {
    pub schema_version: u8,
    pub delivery_id: String,
    pub payload_sha256: String,
    pub sent_at_unix_ms: u64,
    pub sender_process_identity: String,
    pub sender_deployment_identity: String,
}

impl DeliveryEnvelopeV1 {
    #[must_use]
    pub fn new(
        delivery_id: String,
        message: &SensoryMsg,
        sent_at_unix_ms: u64,
        sender_process_identity: String,
        sender_deployment_identity: String,
    ) -> Self {
        Self {
            schema_version: 1,
            delivery_id,
            payload_sha256: canonical_sensory_payload_sha256(message),
            sent_at_unix_ms,
            sender_process_identity,
            sender_deployment_identity,
        }
    }

    #[must_use]
    pub fn payload_matches(&self, message: &SensoryMsg) -> bool {
        self.schema_version == 1
            && valid_identifier(&self.delivery_id)
            && valid_sha256(&self.payload_sha256)
            && self.payload_sha256 == canonical_sensory_payload_sha256(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutualAddressEnvelopeV1 {
    pub schema_version: u8,
    pub address_id: String,
    pub from_being: String,
    pub to_being: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correspondence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_lineage_id: Option<String>,
    pub created_at_unix_ms: u64,
    pub body_sha256: String,
    pub raw_body_included: bool,
}

impl MutualAddressEnvelopeV1 {
    #[must_use]
    pub fn is_exact_lineage(&self) -> bool {
        self.schema_version == 1
            && valid_identifier(&self.address_id)
            && valid_identifier(&self.from_being)
            && valid_identifier(&self.to_being)
            && valid_sha256(&self.body_sha256)
            && !self.raw_body_included
            && (self
                .correspondence_id
                .as_deref()
                .is_some_and(valid_identifier)
                || self
                    .authority_lineage_id
                    .as_deref()
                    .is_some_and(valid_identifier))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensoryServerHelloV1 {
    pub kind: String,
    pub schema_version: u8,
    pub protocol: ProtocolHeaderV1,
    pub capabilities: Vec<String>,
    pub server_process_identity: String,
    pub server_deployment_identity: String,
    pub spectral_causation_established: bool,
}

impl SensoryServerHelloV1 {
    #[must_use]
    pub fn new(server_process_identity: String, server_deployment_identity: String) -> Self {
        Self {
            kind: "sensory_server_hello".to_string(),
            schema_version: 1,
            protocol: current_protocol(),
            capabilities: vec![
                "delivery_v1".to_string(),
                "mutual_address_v1".to_string(),
                "sensory_delivery_receipt_v1".to_string(),
                "division_command_v1".to_string(),
                "division_receipt_v1".to_string(),
            ],
            server_process_identity,
            server_deployment_identity,
            spectral_causation_established: false,
        }
    }

    #[must_use]
    pub fn supports_receipts(&self) -> bool {
        self.schema_version == 1
            && self.protocol.major == PROTOCOL_MAJOR
            && self.protocol.minor >= 1
            && self
                .capabilities
                .iter()
                .any(|capability| capability == "sensory_delivery_receipt_v1")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensoryDeliveryStatusV1 {
    Accepted,
    Duplicate,
    Rejected,
    PolicyBlocked,
    PartiallyApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensoryDeliveryReceiptV1 {
    pub kind: String,
    pub schema_version: u8,
    pub receipt_id: String,
    pub delivery_id: String,
    pub payload_sha256: String,
    pub status: SensoryDeliveryStatusV1,
    pub received_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routed_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutual_address_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub server_process_identity: String,
    pub server_deployment_identity: String,
    pub spectral_causation_established: bool,
}

impl SensoryDeliveryReceiptV1 {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        receipt_id: String,
        delivery_id: String,
        payload_sha256: String,
        status: SensoryDeliveryStatusV1,
        received_at_unix_ms: u64,
        routed_at_unix_ms: Option<u64>,
        mutual_address_id: Option<String>,
        reason: Option<String>,
        server_process_identity: String,
        server_deployment_identity: String,
    ) -> Self {
        Self {
            kind: "sensory_delivery_receipt".to_string(),
            schema_version: 1,
            receipt_id,
            delivery_id,
            payload_sha256,
            status,
            received_at_unix_ms,
            routed_at_unix_ms,
            mutual_address_id,
            reason,
            server_process_identity,
            server_deployment_identity,
            spectral_causation_established: false,
        }
    }
}

#[must_use]
pub fn canonical_sensory_payload_sha256(message: &SensoryMsg) -> String {
    let mut value = serde_json::to_value(message).unwrap_or(Value::Null);
    canonicalize_json(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn canonicalize_json(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            let mut ordered = fields
                .iter_mut()
                .map(|(key, child)| {
                    canonicalize_json(child);
                    (key.clone(), child.take())
                })
                .collect::<Vec<_>>();
            ordered.sort_by(|left, right| left.0.cmp(&right.0));
            fields.clear();
            fields.extend(ordered);
        },
        Value::Array(values) => {
            for child in values {
                canonicalize_json(child);
            }
        },
        _ => {},
    }
}

fn valid_identifier(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value.len() <= 256
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SensoryMsg {
    /// Dedicated ACTION-controlled reservoir-division command lane.
    Division { command: DivisionCommandV1 },
    Video {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    Audio {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    Aux {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    Semantic {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    #[serde(rename = "attractor_pulse")]
    AttractorPulse {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
    },
    #[serde(rename = "shadow_influence")]
    ShadowInfluence {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        basis: Option<String>,
    },
    Control {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        synth_gain: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_bias: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exploration_noise: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill_target: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        regulation_strength: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smoothing_preference: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_curiosity: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_lambda_bias: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_drive: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalty_sensitivity: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breathing_rate_scale: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mem_mode: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        journal_resonance: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_interval: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        embedding_strength: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        memory_decay_rate: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_cushion: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_annotation: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deep_breathing: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        synth_noise_level: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pure_tone: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_audio_synth: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_video_synth: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        live_audio_enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        live_video_enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_kp: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_ki: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_max_step: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_geom_weight: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_integrator_leak: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_authority_request_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_decay_ticks: Option<u32>,
    },
}
