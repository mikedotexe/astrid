fn activity_now_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn begin_activity_mailbox_window(
    conv: &mut ConversationState,
    inbox: &durable_inbox::DurableInbox,
) -> (Option<durable_inbox::InboxReservation>, String) {
    if let Err(error) = recover_activity_deliveries(
        &crate::action_continuity::ActionContinuityStore::for_astrid_workspace(),
        inbox,
        None,
    ) {
        warn!(%error, "mailbox recovery needs review; no new body admitted");
        return (
            None,
            "Mailbox recovery is unresolved; saved letters remain pending.".into(),
        );
    }
    let mut reservation = None;
    let mut detail = String::new();
    if let Some(window) = conv.activity.mailbox_window.clone() {
        let admission = inbox.reserve(&durable_inbox::ReceiveWindow {
            id: window.id,
            now_unix_ms: activity_now_ms(),
            eligible: conv.activity.foreground_reader.is_none(),
            max_letter_bytes: if window.large {
                crate::llm::MAX_EXPLICIT_LETTER_BYTES
            } else {
                durable_inbox::ORDINARY_LETTER_BYTES
            },
        });
        match admission {
            Ok(admission) => {
                let mut next = conv.activity.clone();
                next.mailbox_window = None;
                match activity_reading::persist_activity(
                    &crate::action_continuity::ActionContinuityStore::for_astrid_workspace(),
                    &next,
                ) {
                    Ok(()) => {
                        conv.activity = next;
                        match admission {
                            durable_inbox::InboxAdmission::Reserved(letter) => reservation = Some(letter),
                            durable_inbox::InboxAdmission::NeedsExplicitReadingWindow(letter) => {
                                detail = if letter.byte_len > crate::llm::MAX_EXPLICIT_LETTER_BYTES as u64 {
                                    format!("A {}-byte letter exceeds the current {}-byte intact window and remains waiting; it has not been acknowledged.", letter.byte_len, crate::llm::MAX_EXPLICIT_LETTER_BYTES)
                                } else {
                                    "The next letter needs CHECK_MAILBOX LARGE; it remains waiting intact.".into()
                                };
                            }
                            durable_inbox::InboxAdmission::Empty => detail = "No eligible letter in this window.".into(),
                            durable_inbox::InboxAdmission::AlreadyAttempted => detail = "This saved window was already attempted; CHECK_MAILBOX opens another.".into(),
                            durable_inbox::InboxAdmission::Ineligible => detail = "Foreground reading retains attention; park it to open a mailbox window.".into(),
                        }
                    },
                    Err(error) => {
                        warn!(%error, "mailbox selection could not be persisted; no body admitted")
                    },
                }
            },
            Err(error) => warn!(%error, "mailbox admission unavailable; letters remain pending"),
        }
    }
    let status = match inbox.scan() {
        Ok(summary) => format!(
            "Mailbox: {} waiting ({} need a larger window; {} unreadable sources). CHECK_MAILBOX chooses one letter. {}",
            summary.pending,
            summary.needs_explicit_reading_window,
            summary.unreadable_sources,
            detail,
        ),
        Err(error) => {
            warn!(%error, "mailbox metadata unavailable");
            "Mailbox status unavailable; no acknowledgement was inferred.".into()
        },
    };
    (reservation, status)
}

fn capture_reserved_letter(
    letter: &durable_inbox::InboxReservation,
) -> contact_capacity::InboxReadBatchV1 {
    let identity = correspondence_v1::contact_source_identity_for_inbox_file(
        &letter.letter.source_path,
        &letter.text,
    );
    let source = contact_capacity::InboxSourceMaterialV1::new(
        letter.letter.version_id.clone(),
        letter.text.as_bytes(),
        identity,
    );
    contact_capacity::InboxReadBatchV1::capture(
        letter.text.clone(),
        std::time::SystemTime::now(),
        vec![source],
        &bridge_paths()
            .bridge_workspace()
            .join("diagnostics/contact_capacity_trace_v1"),
    )
}

fn reserved_letter_peer_target(
    letter: &durable_inbox::InboxReservation,
) -> Option<correspondence_v1::InboxPeerMessage> {
    if let Some(envelope) = correspondence_v1::parse_envelope_text(&letter.text) {
        return (envelope.from_being == "minime" && envelope.to_being == "astrid").then_some(
            correspondence_v1::InboxPeerMessage {
                message_id: letter.letter.message_id.clone(),
                thread_id: letter.letter.thread_id.clone(),
                persistence_id: envelope.persistence_id,
                from_being: envelope.from_being,
                file_path: letter.letter.source_path.clone(),
            },
        );
    }
    letter
        .letter
        .source_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("from_minime_"))
        .then(|| correspondence_v1::InboxPeerMessage {
            message_id: letter.letter.message_id.clone(),
            thread_id: letter.letter.thread_id.clone(),
            persistence_id: None,
            from_being: "minime".into(),
            file_path: letter.letter.source_path.clone(),
        })
}

fn unpack_activity_completion(
    conv: &mut ConversationState,
    completion: crate::llm::DialogueCompletionV1,
    accepted: &mut Option<crate::llm::PromptDeliveryReceiptV1>,
) -> Option<String> {
    if let Some(overflow) = completion.overflow
        && conv.activity.foreground_reader.is_none()
        && conv.activity.return_reader.is_none()
        && should_arm_prompt_overflow_read_more(
            conv.last_read_path.as_deref(),
            conv.recent_next_choices.back().map(String::as_str),
        )
    {
        conv.last_read_path = Some(overflow.path.to_string_lossy().into_owned());
        conv.last_read_offset = overflow.offset;
        conv.last_read_meaning_summary = Some(format!("Context overflow: {}", overflow.summary));
    }
    *accepted = completion.accepted_delivery;
    completion.text
}

fn finish_activity_turn(
    conv: &mut ConversationState,
    inbox: &durable_inbox::DurableInbox,
    reading: Option<&activity_reading::ActivityReadingOfferV1>,
    letter: Option<&durable_inbox::InboxReservation>,
    receipt: Option<&crate::llm::PromptDeliveryReceiptV1>,
    completion: Option<&str>,
) -> bool {
    let outcome = commit_activity_delivery(
        &crate::action_continuity::ActionContinuityStore::for_astrid_workspace(),
        inbox,
        reading,
        letter,
        receipt,
        completion,
    );
    match outcome {
        Ok(ActivityDeliveryOutcome::Reading { admitted_bytes }) => {
            if let Some(offer) = reading {
                let admitted_chars = usize::try_from(admitted_bytes).ok()
                    .and_then(|end| offer.text.get(..end))
                    .map_or(0, |text| text.chars().count());
                conv.note_read_depth_advance(
                    "READ_MORE", offer.reader.session_id.clone(),
                    u32::try_from(admitted_chars).unwrap_or(u32::MAX),
                );
            }
            false
        },
        Ok(ActivityDeliveryOutcome::Letter(delivered)) => {
            if let Some(letter) = letter {
                btsp::record_astrid_inbox_read(&letter.letter.source_path, &letter.text);
            }
            if let Err(error) = correspondence_v1::append_read_receipt(
                "astrid",
                &delivered.message_id,
                &delivered.thread_id,
                &delivered.archived_path,
            ) {
                warn!(%error, "letter completed but correspondence ledger receipt unavailable");
            }
            if let Some(letter) = letter
                && let Some(name) = letter
                    .letter
                    .source_path
                    .file_name()
                    .and_then(|name| name.to_str())
                && name.starts_with("mike_query")
            {
                record_open_steward_query(name, &letter.text);
            }
            true
        },
        result => {
            let reason = match result {
                Err(error) => error.to_string(),
                _ => "no verified retained completion for selected content".into(),
            };
            if let Some(letter) = letter
                && let Err(error) = inbox.fail(letter, activity_now_ms(), &reason)
            {
                warn!(%error, "letter retry state unavailable; no acknowledgement recorded");
            }
            if reading.is_some() || letter.is_some() {
                warn!(%reason, "activity delivery remains pending");
            }
            false
        },
    }
}

fn prepare_activity_reading(
    conv: &mut ConversationState,
    inbox: &durable_inbox::DurableInbox,
) -> anyhow::Result<Option<activity_reading::ActivityReadingOfferV1>> {
    let offer = activity_reading::offer_requested_reading(conv)?;
    if offer.is_none() {
        return Ok(None);
    }
    let recovered = recover_activity_deliveries(
        &crate::action_continuity::ActionContinuityStore::for_astrid_workspace(),
        inbox,
        offer.as_ref(),
    )?;
    if recovered.reading_bytes_reconciled > 0 {
        activity_reading::offer_requested_reading(conv)
    } else {
        Ok(offer)
    }
}
