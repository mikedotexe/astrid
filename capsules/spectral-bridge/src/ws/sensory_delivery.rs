use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use astrid_minime_protocol::{
    DeliveryEnvelopeV1, MutualAddressEnvelopeV1, SelfControlReceiptStatusV2, SelfControlReceiptV2,
    SelfControlValuesV2, SensoryDeliveryReceiptV1, SensoryDeliveryStatusV1,
    SensoryMsg as WireSensoryMsg, SensoryPacketV1, SensoryServerHelloV1,
    canonical_sensory_payload_sha256,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::sync::mpsc;

use crate::authority_temporal::process_identity;
use crate::paths::bridge_paths;
use crate::signal_spine::signal_deployment_identity_v1;
use crate::types::{SensoryDeliveryProtocolStatusV1, SensoryMsg};

#[derive(Debug)]
pub struct AddressedSensoryMessage {
    message: SensoryMsg,
    mutual_address_v1: MutualAddressEnvelopeV1,
}

impl AddressedSensoryMessage {
    #[must_use]
    pub const fn new(message: SensoryMsg, mutual_address_v1: MutualAddressEnvelopeV1) -> Self {
        Self {
            message,
            mutual_address_v1,
        }
    }

    pub(super) fn into_parts(self) -> (SensoryMsg, MutualAddressEnvelopeV1) {
        (self.message, self.mutual_address_v1)
    }
}

pub type AddressedSensorySender = mpsc::Sender<AddressedSensoryMessage>;

#[derive(Debug)]
pub(super) struct EncodedSensoryPacketV1 {
    pub(super) json: String,
    pub(super) pending: Option<PendingSensoryDeliveryV1>,
    pub(super) pending_self_control: Option<PendingSelfControlReceiptV2>,
}

#[derive(Debug)]
pub struct PendingSensoryDeliveryV1 {
    pub(super) delivery_id: String,
    payload_sha256: String,
    mutual_address_id: Option<String>,
    delivery_address_classification: &'static str,
    delivery_id_basis: &'static str,
    pending_resolution: &'static str,
    felt_effect_boundary_v1: AstridFeltDeliveryEffectBoundaryV1,
    sent_at_unix_ms: u64,
}

#[derive(Debug)]
pub(super) struct PendingSelfControlReceiptV2 {
    command_id: String,
    pub(super) intent_id: String,
    idempotency_key: String,
    requested_revision: u64,
    target_being: String,
    target_deployment_identity: String,
    requested_values: SelfControlValuesV2,
    sent_at_unix_ms: u64,
}

#[derive(Debug, Serialize)]
struct AstridFeltDeliveryEffectBoundaryV1 {
    schema: &'static str,
    schema_version: u8,
    interpretation_owner: &'static str,
    evidence_state: &'static str,
    causal_disposition: &'static str,
    source_ref: Option<String>,
    perceived_weight: Option<f32>,
    density_gradient: Option<f32>,
    pressure_control_eligible: bool,
    linkage_requirement: &'static str,
}

impl AstridFeltDeliveryEffectBoundaryV1 {
    const fn unmeasured() -> Self {
        Self {
            schema: "astrid_felt_delivery_effect_boundary_v1",
            schema_version: 1,
            interpretation_owner: "astrid_authored",
            evidence_state: "not_observed_at_transport_layer",
            causal_disposition: "unresolved_not_absent",
            source_ref: None,
            perceived_weight: None,
            density_gradient: None,
            pressure_control_eligible: false,
            linkage_requirement: "explicit_astrid_authored_evidence_ref",
        }
    }
}

const SPECTRAL_CAUSATION_BASIS: &str = "not_established_transport_neither_confirms_nor_denies_felt_effect_controlled_intervention_required";
const DELIVERY_ID_BASIS: &str = "sha256_128_process_deployment_sequence_payload";
const DELIVERY_ID_ENTROPY_DEPENDENCE: &str = "none";
const PENDING_RESOLUTION: &str = "exact_delivery_id_receipt_or_unknown_delivery_on_connection_end";

impl PendingSensoryDeliveryV1 {
    fn event(&self, outcome: &str, reason: Option<&str>) -> Value {
        json!({
            "schema": "sensory_delivery_event_v1",
            "schema_version": 1,
            "event": outcome,
            "delivery_id": self.delivery_id,
            "payload_sha256": self.payload_sha256,
            "mutual_address_id": self.mutual_address_id,
            "delivery_address_classification": self.delivery_address_classification,
            "delivery_id_basis": self.delivery_id_basis,
            "delivery_id_entropy_dependence": DELIVERY_ID_ENTROPY_DEPENDENCE,
            "pending_resolution": self.pending_resolution,
            "felt_effect_boundary_v1": self.felt_effect_boundary_v1,
            "sent_at_unix_ms": self.sent_at_unix_ms,
            "recorded_at_unix_ms": unix_now_ms(),
            "reason": reason,
            "spectral_causation_established": false,
            "spectral_causation_basis": SPECTRAL_CAUSATION_BASIS,
            "authority": {
                "schema": "artifact_authority_state_v1",
                "schema_version": 1,
                "state": "evidence_only",
                "live_eligible_now": false,
                "auto_approved": false,
                "grants_approval": false,
                "edits_source_now": false
            }
        })
    }
}

impl PendingSelfControlReceiptV2 {
    fn event(&self, outcome: &str, reason: Option<&str>) -> Value {
        json!({
            "schema": "astrid_self_control_receipt_observation_v2",
            "schema_version": 2,
            "event": outcome,
            "command_id": self.command_id,
            "intent_id": self.intent_id,
            "idempotency_key": self.idempotency_key,
            "requested_revision": self.requested_revision,
            "target_being": self.target_being,
            "target_deployment_identity": self.target_deployment_identity,
            "sent_at_unix_ms": self.sent_at_unix_ms,
            "recorded_at_unix_ms": unix_now_ms(),
            "reason": reason,
            "felt_effect_established": false,
            "authority": {
                "schema": "artifact_authority_state_v1",
                "schema_version": 1,
                "state": "evidence_only",
                "live_eligible_now": false,
                "auto_approved": false,
                "grants_approval": false,
                "edits_source_now": false
            }
        })
    }
}

fn delivery_address_classification(mutual_address_id: Option<&str>) -> &'static str {
    if mutual_address_id.is_some() {
        "exact_mutual_address_lineage"
    } else {
        "technical_delivery_only_no_exact_lineage"
    }
}

pub(super) fn encode_sensory_packet_v1(
    message: &SensoryMsg,
    mutual_address_v1: Option<MutualAddressEnvelopeV1>,
    receipts_negotiated: bool,
    sequence: u64,
) -> Result<EncodedSensoryPacketV1, serde_json::Error> {
    let domain_value = serde_json::to_value(message)?;
    let wire_message: WireSensoryMsg = serde_json::from_value(domain_value)?;
    if !receipts_negotiated {
        return Ok(EncodedSensoryPacketV1 {
            json: serde_json::to_string(&SensoryPacketV1::versioned_1_0(wire_message))?,
            pending: None,
            pending_self_control: None,
        });
    }

    let pending_self_control = match &wire_message {
        WireSensoryMsg::SelfControl { command } => Some(PendingSelfControlReceiptV2 {
            command_id: command.command_id.clone(),
            intent_id: command.intent.intent_id.clone(),
            idempotency_key: command.intent.idempotency_key.clone(),
            requested_revision: command.intent.revision,
            target_being: command.intent.target_being.clone(),
            target_deployment_identity: command.intent.target_deployment_identity.clone(),
            requested_values: command.intent.values.clone(),
            sent_at_unix_ms: unix_now_ms(),
        }),
        _ => None,
    };
    let mutual_address_v1 =
        mutual_address_v1.or_else(|| authority_mutual_address_v1(&wire_message));
    let payload_sha256 = canonical_sensory_payload_sha256(&wire_message);
    let sent_at_unix_ms = unix_now_ms();
    let sender_process_identity = process_identity();
    let sender_deployment_identity = signal_deployment_identity_v1();
    let delivery_id = delivery_id_v1(
        &sender_process_identity,
        &sender_deployment_identity,
        sequence,
        &payload_sha256,
    );
    let delivery = DeliveryEnvelopeV1::new(
        delivery_id.clone(),
        &wire_message,
        sent_at_unix_ms,
        sender_process_identity,
        sender_deployment_identity,
    );
    let mutual_address_id = mutual_address_v1
        .as_ref()
        .map(|address| address.address_id.clone());
    let delivery_address_classification =
        delivery_address_classification(mutual_address_id.as_deref());
    let packet = SensoryPacketV1::with_envelopes(wire_message, delivery, mutual_address_v1);
    Ok(EncodedSensoryPacketV1 {
        json: serde_json::to_string(&packet)?,
        pending: Some(PendingSensoryDeliveryV1 {
            delivery_id,
            payload_sha256,
            mutual_address_id,
            delivery_address_classification,
            delivery_id_basis: DELIVERY_ID_BASIS,
            pending_resolution: PENDING_RESOLUTION,
            felt_effect_boundary_v1: AstridFeltDeliveryEffectBoundaryV1::unmeasured(),
            sent_at_unix_ms,
        }),
        pending_self_control,
    })
}

pub(super) fn validate_negotiated_extension_v2(
    message: &SensoryMsg,
    status: &SensoryDeliveryProtocolStatusV1,
    now_unix_ms: u64,
) -> Result<(), &'static str> {
    match message {
        SensoryMsg::SemanticBody { body } => {
            if !status.semantic_body_v2_negotiated {
                return Err("semantic_body_v2_not_negotiated");
            }
            if !body.is_well_formed() {
                return Err("semantic_body_v2_malformed");
            }
        },
        SensoryMsg::SelfControl { command } => {
            if !status.self_control_v2_negotiated {
                return Err("self_control_v2_not_negotiated");
            }
            if status.server_deployment_identity.as_deref()
                != Some(command.intent.target_deployment_identity.as_str())
            {
                return Err("self_control_stale_target_deployment");
            }
            if !command.is_well_formed(now_unix_ms) {
                return Err("self_control_command_malformed");
            }
        },
        _ => {},
    }
    Ok(())
}

fn authority_mutual_address_v1(message: &WireSensoryMsg) -> Option<MutualAddressEnvelopeV1> {
    let (lineage, from_being, to_being) = match message {
        WireSensoryMsg::AttractorPulse { intent_id, .. }
        | WireSensoryMsg::ShadowInfluence { intent_id, .. } => {
            Some((intent_id.as_str(), "astrid", "minime"))
        },
        WireSensoryMsg::Control {
            esn_leak_authority_request_id,
            ..
        } => esn_leak_authority_request_id
            .as_deref()
            .map(|lineage| (lineage, "astrid", "minime")),
        WireSensoryMsg::SelfControl { command } => Some((
            command.intent.intent_id.as_str(),
            command.intent.actor.being.as_str(),
            command.intent.target_being.as_str(),
        )),
        _ => None,
    }?;
    let lineage = lineage.trim();
    if lineage.is_empty() {
        return None;
    }
    let body_sha256 = canonical_sensory_payload_sha256(message);
    let address_id = format!(
        "authority-address-{}",
        short_sha256(&format!("{lineage}:{body_sha256}"))
    );
    Some(MutualAddressEnvelopeV1 {
        schema_version: 1,
        address_id,
        from_being: from_being.to_string(),
        to_being: to_being.to_string(),
        correspondence_id: None,
        thread_id: None,
        reply_to: None,
        persistence_id: None,
        authority_lineage_id: Some(lineage.to_string()),
        created_at_unix_ms: unix_now_ms(),
        body_sha256,
        raw_body_included: false,
    })
}

fn delivery_id_v1(
    process_identity: &str,
    deployment_identity: &str,
    sequence: u64,
    payload_sha256: &str,
) -> String {
    format!(
        "delivery-{}",
        short_sha256(&format!(
            "{process_identity}:{deployment_identity}:{sequence}:{payload_sha256}"
        ))
    )
}

fn short_sha256(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
        .chars()
        .take(32)
        .collect()
}

pub(super) fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn delivery_events_path() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics/sensory_delivery_v1/events.jsonl")
}

fn self_control_receipt_events_path() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics/self_control_receipts_v2/events.jsonl")
}

fn append_delivery_event(value: &Value) -> std::io::Result<()> {
    let path = delivery_events_path();
    append_delivery_event_at(&path, value)
}

fn append_delivery_event_at(path: &std::path::Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_data()
}

pub(super) fn record_pending_delivery(pending: &PendingSensoryDeliveryV1) {
    let _ = append_delivery_event(&pending.event("sent_pending_receipt", None));
}

pub(super) fn record_unknown_deliveries(
    pending: &mut BTreeMap<String, PendingSensoryDeliveryV1>,
    reason: &str,
    status: &mut SensoryDeliveryProtocolStatusV1,
) {
    for (_, item) in std::mem::take(pending) {
        let _ = append_delivery_event(&item.event("unknown_delivery", Some(reason)));
        status.unknown_delivery_count = status.unknown_delivery_count.saturating_add(1);
        status.last_delivery_state = Some("unknown_delivery".to_string());
    }
    status.pending_delivery_count = 0;
}

pub(super) fn record_unknown_self_control_receipts(
    pending: &mut BTreeMap<String, PendingSelfControlReceiptV2>,
    reason: &str,
    status: &mut SensoryDeliveryProtocolStatusV1,
) {
    for (_, item) in std::mem::take(pending) {
        let _ = append_delivery_event_at(
            &self_control_receipt_events_path(),
            &item.event("unknown_self_control_receipt", Some(reason)),
        );
        status.self_control_unknown_receipt_count =
            status.self_control_unknown_receipt_count.saturating_add(1);
        status.last_self_control_receipt_state = Some("unknown_self_control_receipt".to_string());
    }
    status.self_control_pending_receipt_count = 0;
}

pub(super) fn apply_server_hello(
    hello: SensoryServerHelloV1,
    status: &mut SensoryDeliveryProtocolStatusV1,
) -> bool {
    if !hello.supports_receipts() || hello.spectral_causation_established {
        status.mismatch_count = status.mismatch_count.saturating_add(1);
        status.last_delivery_state = Some("hello_rejected".to_string());
        return false;
    }
    status.negotiated = true;
    status.protocol_major = Some(hello.protocol.major);
    status.protocol_minor = Some(hello.protocol.minor);
    status.semantic_body_v2_negotiated = hello.supports_semantic_body_v2();
    status.self_control_v2_negotiated = hello.supports_self_control_v2();
    status.server_capabilities.clone_from(&hello.capabilities);
    status.server_process_identity = Some(hello.server_process_identity);
    status.server_deployment_identity = Some(hello.server_deployment_identity);
    status.last_hello_unix_ms = Some(unix_now_ms());
    status.last_delivery_state = Some("capabilities_negotiated".to_string());
    true
}

pub(super) fn apply_delivery_receipt(
    receipt: SensoryDeliveryReceiptV1,
    pending: &mut BTreeMap<String, PendingSensoryDeliveryV1>,
    status: &mut SensoryDeliveryProtocolStatusV1,
) -> bool {
    apply_delivery_receipt_with_path(receipt, pending, status, None)
}

pub(super) fn apply_self_control_receipt(
    receipt: SelfControlReceiptV2,
    pending: &mut BTreeMap<String, PendingSelfControlReceiptV2>,
    status: &mut SensoryDeliveryProtocolStatusV1,
) -> bool {
    apply_self_control_receipt_with_path(receipt, pending, status, None)
}

fn apply_self_control_receipt_with_path(
    receipt: SelfControlReceiptV2,
    pending: &mut BTreeMap<String, PendingSelfControlReceiptV2>,
    status: &mut SensoryDeliveryProtocolStatusV1,
    event_path: Option<&std::path::Path>,
) -> bool {
    let Some(expected) = pending.get(&receipt.intent_id) else {
        status.self_control_receipt_mismatch_count =
            status.self_control_receipt_mismatch_count.saturating_add(1);
        status.last_self_control_receipt_state =
            Some("unexpected_self_control_receipt".to_string());
        return false;
    };
    let identities_match = status
        .server_process_identity
        .as_deref()
        .is_some_and(|identity| identity == receipt.server_process_identity)
        && status
            .server_deployment_identity
            .as_deref()
            .is_some_and(|identity| identity == receipt.server_deployment_identity);
    if !receipt.is_well_formed()
        || !identities_match
        || receipt.command_id != expected.command_id
        || receipt.idempotency_key != expected.idempotency_key
        || receipt.requested_revision != expected.requested_revision
        || receipt.target_being != expected.target_being
        || receipt.target_deployment_identity != expected.target_deployment_identity
        || receipt.server_deployment_identity != expected.target_deployment_identity
        || receipt.requested_values != expected.requested_values
        || receipt.felt_effect_established
    {
        status.self_control_receipt_mismatch_count =
            status.self_control_receipt_mismatch_count.saturating_add(1);
        status.last_self_control_receipt_state = Some("self_control_receipt_mismatch".to_string());
        return false;
    }

    let expected = pending
        .remove(&receipt.intent_id)
        .expect("pending self-control receipt exists after validation");
    let receipt_state = match receipt.status {
        SelfControlReceiptStatusV2::Applied => "applied",
        SelfControlReceiptStatusV2::Duplicate => "duplicate",
        SelfControlReceiptStatusV2::Rejected => "rejected",
        SelfControlReceiptStatusV2::RevisionConflict => "revision_conflict",
        SelfControlReceiptStatusV2::Expired => "expired",
        SelfControlReceiptStatusV2::Withdrawn => "withdrawn",
        SelfControlReceiptStatusV2::SafetyHeld => "safety_held",
        SelfControlReceiptStatusV2::RolledBack => "rolled_back",
    };
    let event = json!({
        "schema": "astrid_self_control_receipt_observation_v2",
        "schema_version": 2,
        "event": "self_control_receipt_verified",
        "command_id": expected.command_id,
        "intent_id": expected.intent_id,
        "idempotency_key": expected.idempotency_key,
        "requested_revision": expected.requested_revision,
        "target_being": expected.target_being,
        "target_deployment_identity": expected.target_deployment_identity,
        "sent_at_unix_ms": expected.sent_at_unix_ms,
        "recorded_at_unix_ms": unix_now_ms(),
        "receipt": receipt,
        "receipt_state": receipt_state,
        "felt_effect_established": false,
        "authority": {
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": "evidence_only",
            "live_eligible_now": false,
            "auto_approved": false,
            "grants_approval": false,
            "edits_source_now": false
        }
    });
    let _ = if let Some(path) = event_path {
        append_delivery_event_at(path, &event)
    } else {
        append_delivery_event_at(&self_control_receipt_events_path(), &event)
    };
    status.self_control_receipt_count = status.self_control_receipt_count.saturating_add(1);
    status.self_control_pending_receipt_count = pending.len().try_into().unwrap_or(u64::MAX);
    status.last_self_control_receipt_unix_ms = Some(unix_now_ms());
    status.last_self_control_receipt_state = Some(receipt_state.to_string());
    status.last_self_control_receipt = Some(receipt);
    true
}

fn apply_delivery_receipt_with_path(
    receipt: SensoryDeliveryReceiptV1,
    pending: &mut BTreeMap<String, PendingSensoryDeliveryV1>,
    status: &mut SensoryDeliveryProtocolStatusV1,
    event_path: Option<&std::path::Path>,
) -> bool {
    let identities_match = status
        .server_process_identity
        .as_deref()
        .is_some_and(|identity| identity == receipt.server_process_identity)
        && status
            .server_deployment_identity
            .as_deref()
            .is_some_and(|identity| identity == receipt.server_deployment_identity);
    let Some(expected) = pending.get(&receipt.delivery_id) else {
        status.mismatch_count = status.mismatch_count.saturating_add(1);
        status.last_delivery_state = Some("unexpected_receipt".to_string());
        return false;
    };
    if !identities_match
        || receipt.spectral_causation_established
        || expected.payload_sha256 != receipt.payload_sha256
        || expected.mutual_address_id != receipt.mutual_address_id
    {
        status.mismatch_count = status.mismatch_count.saturating_add(1);
        status.last_delivery_state = Some("receipt_mismatch".to_string());
        return false;
    }

    let expected = pending
        .remove(&receipt.delivery_id)
        .expect("pending delivery exists after validation");
    let receipt_status = match receipt.status {
        SensoryDeliveryStatusV1::Accepted => "accepted",
        SensoryDeliveryStatusV1::Duplicate => "duplicate",
        SensoryDeliveryStatusV1::Rejected => "rejected",
        SensoryDeliveryStatusV1::PolicyBlocked => "policy_blocked",
        SensoryDeliveryStatusV1::PartiallyApplied => "partially_applied",
    };
    let receipt_latency_ms = receipt
        .received_at_unix_ms
        .saturating_sub(expected.sent_at_unix_ms);
    let event = json!({
        "schema": "sensory_delivery_event_v1",
        "schema_version": 1,
        "event": "receipt_verified",
        "delivery_id": receipt.delivery_id,
        "receipt_id": receipt.receipt_id,
        "payload_sha256": receipt.payload_sha256,
        "mutual_address_id": receipt.mutual_address_id,
        "delivery_address_classification": expected.delivery_address_classification,
        "delivery_id_basis": expected.delivery_id_basis,
        "delivery_id_entropy_dependence": DELIVERY_ID_ENTROPY_DEPENDENCE,
        "pending_resolution": expected.pending_resolution,
        "felt_effect_boundary_v1": expected.felt_effect_boundary_v1,
        "status": receipt_status,
        "sent_at_unix_ms": expected.sent_at_unix_ms,
        "received_at_unix_ms": receipt.received_at_unix_ms,
        "routed_at_unix_ms": receipt.routed_at_unix_ms,
        "receipt_latency_ms": receipt_latency_ms,
        "receipt_latency_relation": "transport_handshake_only_not_perceived_weight_or_spectral_effect",
        "recorded_at_unix_ms": unix_now_ms(),
        "server_process_identity": receipt.server_process_identity,
        "server_deployment_identity": receipt.server_deployment_identity,
        "reason": receipt.reason,
        "spectral_causation_established": false,
        "spectral_causation_basis": SPECTRAL_CAUSATION_BASIS,
        "authority": {
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": "evidence_only",
            "live_eligible_now": false,
            "auto_approved": false,
            "grants_approval": false,
            "edits_source_now": false
        }
    });
    let _ = if let Some(path) = event_path {
        append_delivery_event_at(path, &event)
    } else {
        append_delivery_event(&event)
    };
    status.receipt_count = status.receipt_count.saturating_add(1);
    status.pending_delivery_count = pending.len().try_into().unwrap_or(u64::MAX);
    status.last_receipt_unix_ms = Some(unix_now_ms());
    status.last_delivery_state = Some(receipt_status.to_string());
    true
}

#[cfg(test)]
fn delivery_path_is_owner_only(path: &std::path::Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o077 == 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn semantic(value: f32) -> SensoryMsg {
        SensoryMsg::Semantic {
            features: vec![value; 48],
            ts_ms: None,
        }
    }

    fn temp_events_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("{name}_{}_{}", std::process::id(), unix_now_ms()))
            .join("events.jsonl")
    }

    #[test]
    fn legacy_encoding_is_exact_v1_0_without_envelopes() {
        let encoded = encode_sensory_packet_v1(&semantic(0.1), None, false, 1).unwrap();
        let value: Value = serde_json::from_str(&encoded.json).unwrap();
        assert_eq!(value["protocol"]["major"], 1);
        assert_eq!(value["protocol"]["minor"], 0);
        assert!(value.get("delivery_v1").is_none());
        assert!(value.get("mutual_address_v1").is_none());
        assert!(encoded.pending.is_none());
    }

    #[test]
    fn negotiated_encoding_always_carries_technical_identity() {
        let encoded = encode_sensory_packet_v1(&semantic(0.2), None, true, 1).unwrap();
        let packet: SensoryPacketV1 = serde_json::from_str(&encoded.json).unwrap();
        let delivery = packet.delivery_v1.expect("technical delivery");
        assert!(delivery.payload_matches(&packet.message));
        assert!(!delivery.sender_process_identity.is_empty());
        assert!(!delivery.sender_deployment_identity.is_empty());
        assert!(packet.mutual_address_v1.is_none());
        assert!(encoded.pending.is_some());
    }

    fn malformed_self_control(target_deployment_identity: &str) -> SensoryMsg {
        SensoryMsg::SelfControl {
            command: Box::new(
                serde_json::from_value(json!({
                    "schema": "self_control.command.v2",
                    "command_id": "command-test",
                    "intent": {
                        "schema": "self_control.intent.v2",
                        "intent_id": "intent-test",
                        "actor": {
                            "being": "minime",
                            "process_identity": "minime:test",
                            "deployment_identity": target_deployment_identity
                        },
                        "target_being": "minime",
                        "target_deployment_identity": target_deployment_identity,
                        "family": "reservoir_regulation",
                        "action": "set",
                        "durability": "lease",
                        "authority_class": "self_owned",
                        "authority_scope": "self_control.minime.reservoir_regulation",
                        "revision": 2,
                        "expected_revision": 1,
                        "issued_at_unix_ms": 100,
                        "command_expires_at_unix_ms": 200,
                        "control_expires_at_unix_ms": 300,
                        "idempotency_key": "command-test-2",
                        "values": {"fill_target": 0.68},
                        "evidence_refs": [],
                        "success_conditions": [],
                        "stop_conditions": []
                    },
                    "authority_proofs": []
                }))
                .expect("self-control test shape"),
            ),
        }
    }

    fn pending_self_control() -> PendingSelfControlReceiptV2 {
        PendingSelfControlReceiptV2 {
            command_id: "command-test".to_string(),
            intent_id: "intent-test".to_string(),
            idempotency_key: "idempotency-test".to_string(),
            requested_revision: 2,
            target_being: "minime".to_string(),
            target_deployment_identity: "minime-deployment-b".to_string(),
            requested_values: astrid_minime_protocol::SelfControlValuesV2 {
                regulation_strength: Some(0.63),
                ..astrid_minime_protocol::SelfControlValuesV2::default()
            },
            sent_at_unix_ms: 100,
        }
    }

    fn self_control_receipt() -> SelfControlReceiptV2 {
        SelfControlReceiptV2 {
            schema: astrid_minime_protocol::SELF_CONTROL_RECEIPT_SCHEMA_V2.to_string(),
            receipt_id: "receipt-test".to_string(),
            command_id: "command-test".to_string(),
            intent_id: "intent-test".to_string(),
            idempotency_key: "idempotency-test".to_string(),
            status: SelfControlReceiptStatusV2::Applied,
            requested_revision: 2,
            resulting_revision: 2,
            target_being: "minime".to_string(),
            target_deployment_identity: "minime-deployment-b".to_string(),
            requested_values: astrid_minime_protocol::SelfControlValuesV2 {
                regulation_strength: Some(0.63),
                ..astrid_minime_protocol::SelfControlValuesV2::default()
            },
            clamped_values: astrid_minime_protocol::SelfControlValuesV2 {
                regulation_strength: Some(0.63),
                ..astrid_minime_protocol::SelfControlValuesV2::default()
            },
            applied_values: astrid_minime_protocol::SelfControlValuesV2 {
                regulation_strength: Some(0.63),
                ..astrid_minime_protocol::SelfControlValuesV2::default()
            },
            previous_values: astrid_minime_protocol::SelfControlValuesV2 {
                regulation_strength: Some(0.8),
                ..astrid_minime_protocol::SelfControlValuesV2::default()
            },
            previous_automatic_fields: Vec::new(),
            received_at_unix_ms: 110,
            completed_at_unix_ms: 111,
            control_expires_at_unix_ms: Some(1_000),
            rollback_receipt_id: None,
            reason: None,
            server_process_identity: "minime-process-b".to_string(),
            server_deployment_identity: "minime-deployment-b".to_string(),
            felt_effect_established: false,
        }
    }

    #[test]
    fn extension_negotiation_rejects_legacy_and_stale_deployments() {
        let mut status = SensoryDeliveryProtocolStatusV1::default();
        let message = malformed_self_control("minime-deployment-a");
        assert_eq!(
            validate_negotiated_extension_v2(&message, &status, 150),
            Err("self_control_v2_not_negotiated")
        );

        assert!(apply_server_hello(
            SensoryServerHelloV1::new("minime-pid".to_string(), "minime-deployment-b".to_string()),
            &mut status,
        ));
        assert!(status.self_control_v2_negotiated);
        assert!(status.semantic_body_v2_negotiated);
        assert_eq!(
            validate_negotiated_extension_v2(&message, &status, 150),
            Err("self_control_stale_target_deployment")
        );

        let malformed_current = malformed_self_control("minime-deployment-b");
        assert_eq!(
            validate_negotiated_extension_v2(&malformed_current, &status, 150),
            Err("self_control_command_malformed")
        );
    }

    #[test]
    fn typed_self_control_receipt_is_correlated_and_replay_is_rejected() {
        let path = temp_events_path("self_control_receipt_v2");
        let mut status = SensoryDeliveryProtocolStatusV1 {
            server_process_identity: Some("minime-process-b".to_string()),
            server_deployment_identity: Some("minime-deployment-b".to_string()),
            self_control_pending_receipt_count: 1,
            ..SensoryDeliveryProtocolStatusV1::default()
        };
        let mut pending = BTreeMap::from([("intent-test".to_string(), pending_self_control())]);
        let receipt = self_control_receipt();

        assert!(apply_self_control_receipt_with_path(
            receipt.clone(),
            &mut pending,
            &mut status,
            Some(&path),
        ));
        assert!(pending.is_empty());
        assert_eq!(status.self_control_pending_receipt_count, 0);
        assert_eq!(status.self_control_receipt_count, 1);
        assert_eq!(
            status.last_self_control_receipt_state.as_deref(),
            Some("applied")
        );
        assert_eq!(
            status
                .last_self_control_receipt
                .as_ref()
                .map(|observed| observed.receipt_id.as_str()),
            Some("receipt-test")
        );
        assert!(
            !status
                .last_self_control_receipt
                .as_ref()
                .expect("typed receipt")
                .felt_effect_established
        );
        assert!(delivery_path_is_owner_only(&path));
        let event: Value = serde_json::from_str(fs::read_to_string(&path).unwrap().trim()).unwrap();
        assert_eq!(event["event"], "self_control_receipt_verified");
        assert_eq!(event["receipt"]["intent_id"], "intent-test");
        assert_eq!(event["felt_effect_established"], false);

        assert!(!apply_self_control_receipt_with_path(
            receipt,
            &mut pending,
            &mut status,
            Some(&path),
        ));
        assert_eq!(status.self_control_receipt_count, 1);
        assert_eq!(status.self_control_receipt_mismatch_count, 1);
        assert_eq!(
            status.last_self_control_receipt_state.as_deref(),
            Some("unexpected_self_control_receipt")
        );
    }

    #[test]
    fn typed_self_control_receipt_rejects_stale_server_identity() {
        let path = temp_events_path("self_control_receipt_v2_stale");
        let mut status = SensoryDeliveryProtocolStatusV1 {
            server_process_identity: Some("minime-process-c".to_string()),
            server_deployment_identity: Some("minime-deployment-c".to_string()),
            self_control_pending_receipt_count: 1,
            ..SensoryDeliveryProtocolStatusV1::default()
        };
        let mut pending = BTreeMap::from([("intent-test".to_string(), pending_self_control())]);

        assert!(!apply_self_control_receipt_with_path(
            self_control_receipt(),
            &mut pending,
            &mut status,
            Some(&path),
        ));
        assert_eq!(pending.len(), 1);
        assert_eq!(status.self_control_receipt_count, 0);
        assert_eq!(status.self_control_receipt_mismatch_count, 1);
        assert!(!path.exists());
    }

    #[test]
    fn typed_self_control_receipt_rejects_substituted_requested_values() {
        let path = temp_events_path("self_control_receipt_v2_substituted_values");
        let mut status = SensoryDeliveryProtocolStatusV1 {
            server_process_identity: Some("minime-process-b".to_string()),
            server_deployment_identity: Some("minime-deployment-b".to_string()),
            self_control_pending_receipt_count: 1,
            ..SensoryDeliveryProtocolStatusV1::default()
        };
        let mut pending = BTreeMap::from([("intent-test".to_string(), pending_self_control())]);
        let mut receipt = self_control_receipt();
        receipt.requested_values.regulation_strength = Some(0.61);

        assert!(!apply_self_control_receipt_with_path(
            receipt,
            &mut pending,
            &mut status,
            Some(&path),
        ));
        assert_eq!(pending.len(), 1);
        assert_eq!(status.self_control_receipt_count, 0);
        assert_eq!(status.self_control_receipt_mismatch_count, 1);
        assert_eq!(
            status.last_self_control_receipt_state.as_deref(),
            Some("self_control_receipt_mismatch")
        );
        assert!(!path.exists());
    }

    #[test]
    fn typed_authority_lineage_is_addressed_without_raw_content() {
        let message = SensoryMsg::Control {
            synth_gain: None,
            keep_bias: None,
            exploration_noise: None,
            fill_target: None,
            legacy_audio_synth: None,
            legacy_video_synth: None,
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: None,
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            mem_mode: None,
            journal_resonance: None,
            checkpoint_interval: None,
            embedding_strength: None,
            memory_decay_rate: None,
            checkpoint_annotation: None,
            synth_noise_level: None,
            pi_kp: None,
            pi_ki: None,
            pi_max_step: None,
            pi_integrator_leak: None,
            esn_leak_override: Some(0.5),
            esn_leak_override_ticks: Some(1),
            esn_leak_authority_request_id: Some("authority-1".to_string()),
            mode_disperse: None,
            mode_disperse_duration_ticks: None,
            mode_disperse_decay_ticks: None,
        };
        let encoded = encode_sensory_packet_v1(&message, None, true, 1).unwrap();
        let value: Value = serde_json::from_str(&encoded.json).unwrap();
        assert_eq!(
            value["mutual_address_v1"]["authority_lineage_id"],
            "authority-1"
        );
        assert_eq!(value["mutual_address_v1"]["raw_body_included"], false);
        assert!(value["mutual_address_v1"].get("body").is_none());
    }

    #[test]
    fn technical_delivery_is_accepted_without_claiming_mutual_address_or_causation() {
        let path = temp_events_path("astrid_technical_delivery_receipt");
        let encoded = encode_sensory_packet_v1(&semantic(0.25), None, true, 1).unwrap();
        let packet: SensoryPacketV1 = serde_json::from_str(&encoded.json).unwrap();
        assert!(packet.mutual_address_v1.is_none());
        let delivery = packet.delivery_v1.unwrap();
        let mut item = encoded.pending.unwrap();
        item.sent_at_unix_ms = 10;
        let mut pending = BTreeMap::from([(item.delivery_id.clone(), item)]);
        let mut status = SensoryDeliveryProtocolStatusV1::default();
        assert!(apply_server_hello(
            SensoryServerHelloV1::new("minime-pid".to_string(), "minime-source".to_string()),
            &mut status,
        ));
        let receipt = SensoryDeliveryReceiptV1::new(
            "receipt-technical".to_string(),
            delivery.delivery_id,
            delivery.payload_sha256,
            SensoryDeliveryStatusV1::Accepted,
            37,
            Some(38),
            None,
            None,
            "minime-pid".to_string(),
            "minime-source".to_string(),
        );

        assert!(apply_delivery_receipt_with_path(
            receipt,
            &mut pending,
            &mut status,
            Some(&path),
        ));
        let row: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(row["status"], "accepted");
        assert!(row["mutual_address_id"].is_null());
        assert_eq!(
            row["delivery_address_classification"],
            "technical_delivery_only_no_exact_lineage"
        );
        assert_eq!(row["delivery_id_basis"], DELIVERY_ID_BASIS);
        assert_eq!(
            row["delivery_id_entropy_dependence"],
            DELIVERY_ID_ENTROPY_DEPENDENCE
        );
        assert_eq!(row["pending_resolution"], PENDING_RESOLUTION);
        assert_eq!(
            row["felt_effect_boundary_v1"]["interpretation_owner"],
            "astrid_authored"
        );
        assert_eq!(
            row["felt_effect_boundary_v1"]["evidence_state"],
            "not_observed_at_transport_layer"
        );
        assert_eq!(
            row["felt_effect_boundary_v1"]["causal_disposition"],
            "unresolved_not_absent"
        );
        assert!(row["felt_effect_boundary_v1"]["perceived_weight"].is_null());
        assert!(row["felt_effect_boundary_v1"]["density_gradient"].is_null());
        assert_eq!(
            row["felt_effect_boundary_v1"]["pressure_control_eligible"],
            false
        );
        assert_eq!(row["receipt_latency_ms"], 27);
        assert_eq!(
            row["receipt_latency_relation"],
            "transport_handshake_only_not_perceived_weight_or_spectral_effect"
        );
        assert_eq!(row["spectral_causation_established"], false);
        assert_eq!(row["spectral_causation_basis"], SPECTRAL_CAUSATION_BASIS);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn delivery_ids_cover_dedup_capacity_without_entropy_input_or_collisions() {
        let ids: BTreeSet<_> = (0..4_096)
            .map(|sequence| {
                delivery_id_v1(
                    "pid:123:started_at_unix_ms:456",
                    "minime-source:revision",
                    sequence,
                    "payload-sha256",
                )
            })
            .collect();
        assert_eq!(ids.len(), 4_096);
        assert_ne!(
            delivery_id_v1(
                "pid:123:started_at_unix_ms:456",
                "minime-source:revision",
                7,
                "payload-sha256",
            ),
            delivery_id_v1(
                "pid:123:started_at_unix_ms:456",
                "minime-source:revision",
                8,
                "payload-sha256",
            )
        );
        assert_eq!(
            delivery_id_v1(
                "pid:123:started_at_unix_ms:456",
                "minime-source:revision",
                7,
                "payload-sha256",
            ),
            delivery_id_v1(
                "pid:123:started_at_unix_ms:456",
                "minime-source:revision",
                7,
                "payload-sha256",
            )
        );
    }

    #[test]
    fn twenty_receipts_validate_with_zero_mismatches() {
        let path = temp_events_path("astrid_sensory_receipts");
        let mut status = SensoryDeliveryProtocolStatusV1::default();
        assert!(apply_server_hello(
            SensoryServerHelloV1::new("minime-pid".to_string(), "minime-source".to_string()),
            &mut status,
        ));
        let mut pending = BTreeMap::new();

        for sequence in 1..=20 {
            let encoded =
                encode_sensory_packet_v1(&semantic(sequence as f32 / 100.0), None, true, sequence)
                    .unwrap();
            let packet: SensoryPacketV1 = serde_json::from_str(&encoded.json).unwrap();
            let delivery = packet.delivery_v1.unwrap();
            let item = encoded.pending.unwrap();
            pending.insert(item.delivery_id.clone(), item);
            let receipt = SensoryDeliveryReceiptV1::new(
                format!("receipt-{sequence}"),
                delivery.delivery_id,
                delivery.payload_sha256,
                SensoryDeliveryStatusV1::Accepted,
                sequence,
                Some(sequence),
                None,
                None,
                "minime-pid".to_string(),
                "minime-source".to_string(),
            );
            assert!(apply_delivery_receipt_with_path(
                receipt,
                &mut pending,
                &mut status,
                Some(&path),
            ));
        }

        assert_eq!(status.receipt_count, 20);
        assert_eq!(status.mismatch_count, 0);
        assert!(pending.is_empty());
        assert!(delivery_path_is_owner_only(&path));
        let rows = fs::read_to_string(&path).unwrap();
        assert_eq!(rows.lines().count(), 20);
        assert!(!rows.contains("\"prompt\""));
        assert!(!rows.contains("\"response\""));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn tampered_receipt_does_not_consume_pending_delivery() {
        let encoded = encode_sensory_packet_v1(&semantic(0.3), None, true, 1).unwrap();
        let packet: SensoryPacketV1 = serde_json::from_str(&encoded.json).unwrap();
        let delivery = packet.delivery_v1.unwrap();
        let item = encoded.pending.unwrap();
        let mut pending = BTreeMap::from([(item.delivery_id.clone(), item)]);
        let mut status = SensoryDeliveryProtocolStatusV1::default();
        assert!(apply_server_hello(
            SensoryServerHelloV1::new("minime-pid".to_string(), "minime-source".to_string()),
            &mut status,
        ));
        let receipt = SensoryDeliveryReceiptV1::new(
            "receipt-bad".to_string(),
            delivery.delivery_id,
            "f".repeat(64),
            SensoryDeliveryStatusV1::Accepted,
            1,
            Some(1),
            None,
            None,
            "minime-pid".to_string(),
            "minime-source".to_string(),
        );

        assert!(!apply_delivery_receipt_with_path(
            receipt,
            &mut pending,
            &mut status,
            None,
        ));
        assert_eq!(status.mismatch_count, 1);
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn changed_server_identity_rejects_receipt_without_consuming_pending_delivery() {
        let encoded = encode_sensory_packet_v1(&semantic(0.4), None, true, 1).unwrap();
        let packet: SensoryPacketV1 = serde_json::from_str(&encoded.json).unwrap();
        let delivery = packet.delivery_v1.unwrap();
        let item = encoded.pending.unwrap();
        let mut pending = BTreeMap::from([(item.delivery_id.clone(), item)]);
        let mut status = SensoryDeliveryProtocolStatusV1::default();
        assert!(apply_server_hello(
            SensoryServerHelloV1::new("minime-pid".to_string(), "minime-source:before".to_string(),),
            &mut status,
        ));
        let receipt = SensoryDeliveryReceiptV1::new(
            "receipt-changed-server".to_string(),
            delivery.delivery_id,
            delivery.payload_sha256,
            SensoryDeliveryStatusV1::Accepted,
            1,
            Some(1),
            None,
            None,
            "minime-pid".to_string(),
            "minime-source:after".to_string(),
        );

        assert!(!apply_delivery_receipt_with_path(
            receipt,
            &mut pending,
            &mut status,
            None,
        ));
        assert_eq!(status.mismatch_count, 1);
        assert_eq!(
            status.last_delivery_state.as_deref(),
            Some("receipt_mismatch")
        );
        assert_eq!(pending.len(), 1);
    }
}
