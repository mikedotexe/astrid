//! Technical reciprocal context that remains distinct from self-authored uptake.

use serde::Serialize;

use super::ExperientialEvidenceAuthorityV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReciprocalContextKindV1 {
    DeliveryReceipt,
    ReadReceipt,
    ReplyLink,
    PresenceHeartbeat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReciprocalContextReceiptV2 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    context_kind: ReciprocalContextKindV1,
    actor: String,
    peer: String,
    thread_id: String,
    message_id: Option<String>,
    source_event_id: String,
    source_event_sha256: String,
    body_sha256: Option<String>,
    recorded_at_unix_ms: u64,
    corrects_legacy_receipt_id: Option<String>,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ReciprocalContextReceiptV2 {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(super) fn new(
        receipt_id: String,
        context_kind: ReciprocalContextKindV1,
        actor: String,
        peer: String,
        thread_id: String,
        message_id: Option<String>,
        source_event_id: String,
        source_event_sha256: String,
        body_sha256: Option<String>,
        recorded_at_unix_ms: u64,
        corrects_legacy_receipt_id: Option<String>,
    ) -> Self {
        Self {
            schema: "reciprocal_context_receipt_v2",
            schema_version: 2,
            receipt_id,
            context_kind,
            actor,
            peer,
            thread_id,
            message_id,
            source_event_id,
            source_event_sha256,
            body_sha256,
            recorded_at_unix_ms,
            corrects_legacy_receipt_id,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}
