use serde::{Deserialize, Serialize};

use super::schema::{
    CandidatePhaseV1, RECEIPT_SCHEMA_V1, SelfChangeCommandKindV1, SelfChangeReceiptDraftV1,
    TransitionActorV1,
};
use super::validation::{canonical_sha256, validate_sha256_named};
use super::{SelfChangeError, SelfChangeResult};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelfChangeReceiptContentV1 {
    pub schema: String,
    pub sequence: u64,
    pub recorded_at_unix_ms: i64,
    pub command_id: String,
    pub candidate_id: String,
    pub actor: TransitionActorV1,
    pub command: SelfChangeCommandKindV1,
    pub from_phase: Option<CandidatePhaseV1>,
    pub to_phase: Option<CandidatePhaseV1>,
    pub expected_state_sha256: String,
    pub resulting_state_sha256: String,
    pub previous_receipt_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelfChangeReceiptV1 {
    pub content: SelfChangeReceiptContentV1,
    pub receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptChainHeadV1 {
    pub next_sequence: u64,
    pub last_receipt_sha256: Option<String>,
    pub last_resulting_state_sha256: Option<String>,
}

impl Default for ReceiptChainHeadV1 {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            last_receipt_sha256: None,
            last_resulting_state_sha256: None,
        }
    }
}

/// A fully encoded line and its next chain head. The integration layer can append `jsonl` exactly
/// as returned and persist the head after a durable flush.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptAppendV1 {
    pub receipt: SelfChangeReceiptV1,
    pub jsonl: String,
    pub next_head: ReceiptChainHeadV1,
}

/// Extends an in-memory receipt chain with one validated draft.
///
/// # Errors
///
/// Returns an error for an invalid digest, discontinuous state/receipt chain,
/// arithmetic overflow, or serialization failure.
pub fn append_receipt(
    head: &ReceiptChainHeadV1,
    draft: SelfChangeReceiptDraftV1,
) -> SelfChangeResult<ReceiptAppendV1> {
    validate_head(head)?;
    validate_sha256_named(&draft.expected_state_sha256, "receipt expected state")?;
    validate_sha256_named(&draft.resulting_state_sha256, "receipt resulting state")?;
    if let Some(previous_state) = &head.last_resulting_state_sha256
        && previous_state != &draft.expected_state_sha256
    {
        return Err(SelfChangeError::ReceiptChainMismatch);
    }

    let content = SelfChangeReceiptContentV1 {
        schema: RECEIPT_SCHEMA_V1.to_string(),
        sequence: head.next_sequence,
        recorded_at_unix_ms: draft.occurred_at_unix_ms,
        command_id: draft.command_id,
        candidate_id: draft.candidate_id,
        actor: draft.actor,
        command: draft.command,
        from_phase: draft.from_phase,
        to_phase: draft.to_phase,
        expected_state_sha256: draft.expected_state_sha256,
        resulting_state_sha256: draft.resulting_state_sha256,
        previous_receipt_sha256: head.last_receipt_sha256.clone(),
    };
    let receipt_sha256 = canonical_sha256(&content)?;
    let receipt = SelfChangeReceiptV1 {
        content,
        receipt_sha256: receipt_sha256.clone(),
    };
    let mut jsonl = serde_json::to_string(&receipt)?;
    jsonl.push('\n');
    let next_sequence = head
        .next_sequence
        .checked_add(1)
        .ok_or(SelfChangeError::ArithmeticOverflow)?;
    let next_head = ReceiptChainHeadV1 {
        next_sequence,
        last_receipt_sha256: Some(receipt_sha256),
        last_resulting_state_sha256: Some(receipt.content.resulting_state_sha256.clone()),
    };
    Ok(ReceiptAppendV1 {
        receipt,
        jsonl,
        next_head,
    })
}

/// Verifies a complete deterministic receipt chain and returns its head.
///
/// # Errors
///
/// Returns an error when any sequence, state digest, previous hash, receipt
/// hash, schema, or serialization invariant is invalid.
pub fn verify_receipt_chain(
    receipts: &[SelfChangeReceiptV1],
) -> SelfChangeResult<ReceiptChainHeadV1> {
    let mut head = ReceiptChainHeadV1::default();
    for receipt in receipts {
        if receipt.content.schema != RECEIPT_SCHEMA_V1
            || receipt.content.sequence != head.next_sequence
            || receipt.content.previous_receipt_sha256 != head.last_receipt_sha256
            || head
                .last_resulting_state_sha256
                .as_ref()
                .is_some_and(|previous| previous != &receipt.content.expected_state_sha256)
        {
            return Err(SelfChangeError::ReceiptChainMismatch);
        }
        validate_sha256_named(
            &receipt.content.expected_state_sha256,
            "receipt expected state",
        )?;
        validate_sha256_named(
            &receipt.content.resulting_state_sha256,
            "receipt resulting state",
        )?;
        let expected_receipt_hash = canonical_sha256(&receipt.content)?;
        if receipt.receipt_sha256 != expected_receipt_hash {
            return Err(SelfChangeError::ReceiptChainMismatch);
        }
        head = ReceiptChainHeadV1 {
            next_sequence: head
                .next_sequence
                .checked_add(1)
                .ok_or(SelfChangeError::ArithmeticOverflow)?,
            last_receipt_sha256: Some(receipt.receipt_sha256.clone()),
            last_resulting_state_sha256: Some(receipt.content.resulting_state_sha256.clone()),
        };
    }
    Ok(head)
}

fn validate_head(head: &ReceiptChainHeadV1) -> SelfChangeResult<()> {
    if head.next_sequence == 0
        || (head.next_sequence == 1)
            != (head.last_receipt_sha256.is_none() && head.last_resulting_state_sha256.is_none())
        || (head.last_receipt_sha256.is_some() != head.last_resulting_state_sha256.is_some())
    {
        return Err(SelfChangeError::ReceiptChainMismatch);
    }
    if let Some(hash) = &head.last_receipt_sha256 {
        validate_sha256_named(hash, "receipt chain head")?;
    }
    if let Some(hash) = &head.last_resulting_state_sha256 {
        validate_sha256_named(hash, "receipt state head")?;
    }
    Ok(())
}
