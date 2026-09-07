/// One protected foreground source may be acknowledged by one accepted turn.
#[derive(Debug)]
enum ActivityDeliveryOutcome {
    None,
    Reading { admitted_bytes: u64 },
    Letter(Box<durable_inbox::InboxDeliveryReceipt>),
}

fn protected_reading_input(
    reading: &activity_reading::ActivityReadingOfferV1,
) -> anyhow::Result<crate::llm::ProtectedDialogueInputV1> {
    Ok(crate::llm::ProtectedDialogueInputV1 {
        content_id: reading.passage.offer_id.clone(),
        kind: crate::llm::ProtectedDialogueKindV1::Reading,
        source_text: reading.text.clone(),
        source_start_byte: usize::try_from(reading.passage.start_byte)?,
    })
}

fn protected_letter_input(
    letter: &durable_inbox::InboxReservation,
) -> crate::llm::ProtectedDialogueInputV1 {
    crate::llm::ProtectedDialogueInputV1 {
        content_id: letter.reservation_id.clone(),
        kind: crate::llm::ProtectedDialogueKindV1::Letter,
        source_text: letter.text.clone(),
        source_start_byte: 0,
    }
}

/// Commit source progress before interpreting NEXT. Neither a failed generation
/// nor an unretained response can advance a cursor or retire a durable letter.
fn commit_activity_delivery(
    store: &crate::action_continuity::ActionContinuityStore,
    inbox: &durable_inbox::DurableInbox,
    reading: Option<&activity_reading::ActivityReadingOfferV1>,
    letter: Option<&durable_inbox::InboxReservation>,
    receipt: Option<&crate::llm::PromptDeliveryReceiptV1>,
    completion_text: Option<&str>,
) -> anyhow::Result<ActivityDeliveryOutcome> {
    use sha2::{Digest as _, Sha256};
    if reading.is_some() && letter.is_some() {
        return Err(anyhow::anyhow!(
            "one accepted turn cannot deliver two foreground sources"
        ));
    }
    let (Some(receipt), Some(text)) = (receipt, completion_text) else {
        return Ok(ActivityDeliveryOutcome::None);
    };
    if text.trim().is_empty()
        || format!("{:x}", Sha256::digest(text.as_bytes())) != receipt.retained_completion_sha256
    {
        return Err(anyhow::anyhow!(
            "completion text does not match its retained delivery receipt"
        ));
    }
    crate::llm::verify_delivery_receipt(receipt)?;
    if let Some(reading) = reading {
        let start = u64::try_from(receipt.source_start_byte)?;
        let end = u64::try_from(receipt.admitted_end_byte)?;
        let admitted_bytes = end
            .checked_sub(start)
            .ok_or_else(|| anyhow::anyhow!("delivery byte range is reversed"))?;
        let admitted = reading
            .text
            .get(..usize::try_from(admitted_bytes)?)
            .ok_or_else(|| {
                anyhow::anyhow!("delivery range is not a UTF-8 prefix of the reading")
            })?;
        if receipt.kind != crate::llm::ProtectedDialogueKindV1::Reading
            || receipt.content_id != reading.passage.offer_id
            || start != reading.passage.start_byte
            || end > reading.passage.end_byte
            || admitted.is_empty()
            || format!("{:x}", Sha256::digest(admitted.as_bytes())) != receipt.admitted_text_sha256
        {
            return Err(anyhow::anyhow!(
                "delivery receipt does not identify the offered reading prefix"
            ));
        }
        let evidence = crate::action_continuity::ReaderDeliveryEvidence {
            offer_id: receipt.content_id.clone(),
            final_request_sha256: receipt.request_sha256.clone(),
            supplied_start_byte: start,
            supplied_end_byte: end,
            supplied_sha256: receipt.admitted_text_sha256.clone(),
            retained_output_ref: receipt.retained_artifact_path.clone(),
            // This reference names the complete execution artifact; its hash
            // covers request, response and the accepted completion together.
            retained_output_sha256: receipt.retained_artifact_sha256.clone(),
        };
        store.reader_bookmark_commit(
            &reading.reader.thread_id,
            &reading.reader.session_id,
            &reading.version,
            &evidence,
            &format!("delivered_{}", receipt.retained_artifact_sha256),
        )?;
        return Ok(ActivityDeliveryOutcome::Reading { admitted_bytes });
    }
    if let Some(letter) = letter {
        if receipt.kind != crate::llm::ProtectedDialogueKindV1::Letter
            || receipt.content_id != letter.reservation_id
            || receipt.source_start_byte != 0
            || receipt.admitted_end_byte != letter.text.len()
            || receipt.admitted_text_sha256 != letter.content_sha256
            || format!("{:x}", Sha256::digest(letter.text.as_bytes())) != letter.content_sha256
        {
            return Err(anyhow::anyhow!(
                "delivery receipt does not identify the intact reserved letter"
            ));
        }
        let receipt = inbox.acknowledge(
            letter,
            &durable_inbox::InboxDeliveryEvidence {
                accepted_attempt_id: receipt.request_sha256.clone(),
                submitted_content_sha256: receipt.admitted_text_sha256.clone(),
                retained_completion_sha256: receipt.retained_completion_sha256.clone(),
            },
        )?;
        return Ok(ActivityDeliveryOutcome::Letter(Box::new(receipt)));
    }
    Err(anyhow::anyhow!(
        "delivery receipt has no corresponding foreground offer"
    ))
}

include!("activity_delivery_tests.rs");
