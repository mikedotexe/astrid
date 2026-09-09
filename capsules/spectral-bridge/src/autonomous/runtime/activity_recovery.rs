/// Receipt reconciliation and idempotent local human-reply publication only.
/// Retained completion prose never enters NEXT, peer routing, reflection hooks,
/// or a fresh model generation. Publication must succeed before acknowledgement.
#[derive(Debug, Default, PartialEq, Eq)]
struct ActivityRecoverySummary {
    letters_reconciled: usize,
    reading_bytes_reconciled: u64,
}

fn recover_activity_deliveries(
    store: &crate::action_continuity::ActionContinuityStore,
    inbox: &durable_inbox::DurableInbox,
    reading: Option<&activity_reading::ActivityReadingOfferV1>,
) -> anyhow::Result<ActivityRecoverySummary> {
    recover_activity_deliveries_with_lookup(
        store,
        inbox,
        reading,
        crate::llm::recover_retained_delivery,
    )
}

fn recover_activity_deliveries_with_lookup(
    store: &crate::action_continuity::ActionContinuityStore,
    inbox: &durable_inbox::DurableInbox,
    reading: Option<&activity_reading::ActivityReadingOfferV1>,
    mut lookup: impl FnMut(
        &str,
        usize,
    )
        -> std::io::Result<Option<(crate::llm::PromptDeliveryReceiptV1, String)>>,
) -> anyhow::Result<ActivityRecoverySummary> {
    let mut summary = ActivityRecoverySummary::default();
    let mut reconciled_versions = std::collections::BTreeSet::new();
    for pending in inbox.pending_reservations()? {
        if reconciled_versions.contains(&pending.letter.version_id) {
            continue;
        }
        let Some((receipt, text)) = lookup(&pending.reservation_id, 0)? else {
            // Submission may or may not have happened. Its durable attempt stays
            // pending; elapsed time does not create another receive window.
            continue;
        };
        let reservation = inbox
            .recover_reservation(&pending.reservation_id)?
            .ok_or_else(|| anyhow::anyhow!("pending inbox attempt disappeared during recovery"))?;
        if reservation.window_id != pending.window_id || reservation.letter != pending.letter {
            return Err(anyhow::anyhow!(
                "inbox attempt changed during receipt recovery"
            ));
        }
        match commit_activity_delivery(
            store,
            inbox,
            None,
            Some(&reservation),
            Some(&receipt),
            Some(&text),
        )? {
            ActivityDeliveryOutcome::Letter(_) => {
                reconciled_versions.insert(pending.letter.version_id);
                summary.letters_reconciled = summary.letters_reconciled.saturating_add(1);
            },
            _ => {
                return Err(anyhow::anyhow!(
                    "retained inbox completion did not acknowledge its reserved letter"
                ));
            },
        }
    }
    if let Some(reading) = reading
        && let Some((receipt, text)) = lookup(
            &reading.passage.offer_id,
            usize::try_from(reading.passage.start_byte)?,
        )?
    {
        match commit_activity_delivery(
            store,
            inbox,
            Some(reading),
            None,
            Some(&receipt),
            Some(&text),
        )? {
            ActivityDeliveryOutcome::Reading { admitted_bytes } => {
                summary.reading_bytes_reconciled = admitted_bytes
            },
            _ => {
                return Err(anyhow::anyhow!(
                    "retained reading completion did not commit its offered prefix"
                ));
            },
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod activity_recovery_tests {
    use super::*;

    const COMPLETION: &str = "I have followed this complete source and can decide what to do from the place it leaves me. Its particular details remain available for my next choice.\n\nNEXT: REMEMBER do not replay this action during recovery";

    fn queue_fixture(temp: &std::path::Path) -> durable_inbox::DurableInbox {
        let directory = temp.join("inbox");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(
            directory.join("steward_note.txt"),
            "An intact synthetic letter.",
        )
        .unwrap();
        durable_inbox::DurableInbox::new(&directory, &temp.join("queue"))
    }

    fn reserve(
        inbox: &durable_inbox::DurableInbox,
        id: &str,
        now: u64,
    ) -> durable_inbox::InboxReservation {
        match inbox
            .reserve(&durable_inbox::ReceiveWindow {
                id: id.into(),
                now_unix_ms: now,
                eligible: true,
                max_letter_bytes: 6_000,
            })
            .unwrap()
        {
            durable_inbox::InboxAdmission::Reserved(letter) => letter,
            other => panic!("unexpected reservation result: {other:?}"),
        }
    }

    #[test]
    fn recovers_completed_letter_after_window_pointer_is_lost_without_next_replay() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = queue_fixture(temp.path());
        let letter = reserve(&inbox, "window-before-crash", 0);
        let artifacts = temp.path().join("accepted");
        let completion = crate::llm::test_completed_protected_dialogue_at(
            &protected_letter_input(&letter),
            &artifacts,
            8_000,
            COMPLETION,
        );
        assert!(completion.accepted_delivery.is_some());
        drop(completion);
        drop(letter);
        let store =
            crate::action_continuity::ActionContinuityStore::new(temp.path().join("threads"));
        let recovered =
            recover_activity_deliveries_with_lookup(&store, &inbox, None, |id, start| {
                crate::llm::recover_retained_delivery_at(&artifacts, id, start)
            })
            .unwrap();
        assert_eq!(recovered.letters_reconciled, 1);
        assert_eq!(inbox.scan().unwrap().pending, 0);
        assert!(inbox.pending_reservations().unwrap().is_empty());
        assert!(!temp.path().join("threads").exists());
        assert!(!temp.path().join("open_steward_query.json").exists());
        let repeated =
            recover_activity_deliveries_with_lookup(&store, &inbox, None, |id, start| {
                crate::llm::recover_retained_delivery_at(&artifacts, id, start)
            })
            .unwrap();
        assert_eq!(repeated, ActivityRecoverySummary::default());
    }

    #[test]
    fn unknown_submission_outcome_stays_pending_without_new_attempt() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = queue_fixture(temp.path());
        let letter = reserve(&inbox, "unknown-outcome", 0);
        let store =
            crate::action_continuity::ActionContinuityStore::new(temp.path().join("threads"));
        let before = std::fs::read(temp.path().join("queue/state.json")).unwrap();
        let recovered =
            recover_activity_deliveries_with_lookup(&store, &inbox, None, |_, _| Ok(None)).unwrap();
        assert_eq!(recovered, ActivityRecoverySummary::default());
        assert_eq!(
            std::fs::read(temp.path().join("queue/state.json")).unwrap(),
            before
        );
        assert_eq!(inbox.pending_reservations().unwrap().len(), 1);
        assert_eq!(
            inbox
                .recover_reservation(&letter.reservation_id)
                .unwrap()
                .unwrap(),
            letter
        );
    }

    #[test]
    fn a_completed_earlier_attempt_is_reconciled_even_after_another_retry() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = queue_fixture(temp.path());
        let earlier = reserve(&inbox, "earlier", 0);
        let artifacts = temp.path().join("accepted");
        crate::llm::test_completed_protected_dialogue_at(
            &protected_letter_input(&earlier),
            &artifacts,
            8_000,
            COMPLETION,
        );
        let later = reserve(&inbox, "later", 30_000);
        assert_ne!(earlier.reservation_id, later.reservation_id);
        let store =
            crate::action_continuity::ActionContinuityStore::new(temp.path().join("threads"));
        let recovered =
            recover_activity_deliveries_with_lookup(&store, &inbox, None, |id, start| {
                crate::llm::recover_retained_delivery_at(&artifacts, id, start)
            })
            .unwrap();
        assert_eq!(recovered.letters_reconciled, 1);
        assert!(inbox.pending_reservations().unwrap().is_empty());
    }

    #[test]
    fn changed_retained_completion_cannot_retire_a_pending_letter() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = queue_fixture(temp.path());
        let letter = reserve(&inbox, "window", 0);
        let artifacts = temp.path().join("accepted");
        let completion = crate::llm::test_completed_protected_dialogue_at(
            &protected_letter_input(&letter),
            &artifacts,
            8_000,
            COMPLETION,
        );
        let receipt = completion.accepted_delivery.unwrap();
        std::fs::write(&receipt.retained_artifact_path, b"corrupt").unwrap();
        let store =
            crate::action_continuity::ActionContinuityStore::new(temp.path().join("threads"));
        assert!(
            recover_activity_deliveries_with_lookup(&store, &inbox, None, |id, start| {
                crate::llm::recover_retained_delivery_at(&artifacts, id, start)
            })
            .is_err()
        );
        assert_eq!(inbox.scan().unwrap().pending, 1);
        assert!(!temp.path().join("inbox/read").exists());
    }

    #[test]
    fn completed_reading_artifact_recovers_byte_progress_without_generation() {
        let temp = tempfile::tempdir().unwrap();
        let store =
            crate::action_continuity::ActionContinuityStore::new(temp.path().join("threads"));
        let inbox = queue_fixture(temp.path());
        let source = temp.path().join("reading.txt");
        std::fs::write(
            &source,
            "A synthetic reading with UTF-8 λ and an exact return.",
        )
        .unwrap();
        let thread = store.create_thread(None, "Reading", None).unwrap();
        let response = store
            .continuity_session_start_command("current :: title: Reading; focus: Continue")
            .unwrap();
        let session_id = response.split('`').nth(1).unwrap().to_string();
        let initial = store
            .reader_bookmark_preview(&thread.thread_id, &session_id)
            .unwrap()
            .unwrap();
        store
            .reader_bookmark_create(
                &thread.thread_id,
                &session_id,
                &initial.session_record_id,
                &source,
                "create",
            )
            .unwrap();
        let initial = store
            .reader_bookmark_preview(&thread.thread_id, &session_id)
            .unwrap()
            .unwrap();
        store
            .reader_bookmark_offer(
                &thread.thread_id,
                &session_id,
                &initial.version().unwrap(),
                4_000,
                "offer",
            )
            .unwrap();
        let offered = store
            .reader_bookmark_preview(&thread.thread_id, &session_id)
            .unwrap()
            .unwrap();
        let reading = activity_reading::ActivityReadingOfferV1 {
            source: offered.bookmark.as_ref().unwrap().source.clone(),
            reader: activity_reading::ReaderActivityRefV1 {
                thread_id: thread.thread_id.clone(),
                session_id: session_id.clone(),
                agenda_item_id: None,
            },
            version: offered.version().unwrap(),
            passage: offered
                .bookmark
                .as_ref()
                .unwrap()
                .offered_passage
                .clone()
                .unwrap(),
            text: offered.offered_text.unwrap(),
        };
        let artifacts = temp.path().join("accepted");
        let completion = crate::llm::test_completed_protected_dialogue_at(
            &protected_reading_input(&reading).unwrap(),
            &artifacts,
            8_000,
            COMPLETION,
        );
        assert!(completion.accepted_delivery.is_some());
        drop(completion);
        let recovered =
            recover_activity_deliveries_with_lookup(&store, &inbox, Some(&reading), |id, start| {
                crate::llm::recover_retained_delivery_at(&artifacts, id, start)
            })
            .unwrap();
        assert_eq!(
            recovered.reading_bytes_reconciled,
            reading.text.len() as u64
        );
        let final_state = store
            .reader_bookmark_preview(&thread.thread_id, &session_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            final_state.bookmark.as_ref().unwrap().cursor.next_byte,
            reading.text.len() as u64
        );
        assert!(
            final_state
                .bookmark
                .as_ref()
                .unwrap()
                .offered_passage
                .is_none()
        );
    }
}
