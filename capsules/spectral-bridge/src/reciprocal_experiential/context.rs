//! Technical reciprocal context that must not be promoted into uptake.

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
pub struct ReciprocalContextReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    context_kind: ReciprocalContextKindV1,
    actor: String,
    peer: String,
    thread_id: String,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    corrects_inferred_uptake_receipt_id: Option<String>,
    presence_inferred: bool,
    acknowledgement_inferred: bool,
    uptake_inferred: bool,
    reply_intention_inferred: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ReciprocalContextReceiptV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn new(
        receipt_id: String,
        context_kind: ReciprocalContextKindV1,
        actor: String,
        peer: String,
        thread_id: String,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
        corrects_inferred_uptake_receipt_id: Option<String>,
    ) -> Self {
        Self {
            schema: "reciprocal_context_receipt_v1",
            schema_version: 1,
            receipt_id,
            context_kind,
            actor,
            peer,
            thread_id,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            corrects_inferred_uptake_receipt_id,
            presence_inferred: false,
            acknowledgement_inferred: false,
            uptake_inferred: false,
            reply_intention_inferred: false,
            raw_prose_included: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}
