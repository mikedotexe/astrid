#[cfg(test)]
mod activity_delivery_tests {
    use super::*;
    use crate::action_continuity::{ActionContinuityStore, ReaderDisposition};
    use crate::llm::test_completed_protected_dialogue_at;

    const RESPONSE: &str = "I can stay with the passage and keep its careful thought in view. This gives me a clear place to return when I finish reading the letter.\n\nNEXT: LISTEN";

    struct Fixture {
        root: tempfile::TempDir,
        store: ActionContinuityStore,
        inbox: durable_inbox::DurableInbox,
        conv: state::ConversationState,
        reading: activity_reading::ActivityReadingOfferV1,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let store = ActionContinuityStore::new(root.path().join("action_threads"));
            let source = root.path().join("source.txt");
            std::fs::write(&source, "λ🙂 a saved source passage\n".repeat(300)).unwrap();
            let mut conv = state::ConversationState::new(Vec::new(), None);
            activity_reading::choose_saved_text_in(&store, &mut conv, &source, "Synthetic reading")
                .unwrap();
            let reading = activity_reading::offer_requested_reading_in(&store, &mut conv)
                .unwrap()
                .unwrap();
            let inbox_dir = root.path().join("inbox");
            std::fs::create_dir_all(&inbox_dir).unwrap();
            let inbox = durable_inbox::DurableInbox::new(&inbox_dir, &root.path().join("queue"));
            Self {
                root,
                store,
                inbox,
                conv,
                reading,
            }
        }

        fn cursor(&self) -> u64 {
            self.store
                .reader_bookmark_preview(
                    &self.reading.reader.thread_id,
                    &self.reading.reader.session_id,
                )
                .unwrap()
                .unwrap()
                .bookmark
                .unwrap()
                .cursor
                .next_byte
        }

        fn reading_completion(&self, budget: usize) -> crate::llm::DialogueCompletionV1 {
            test_completed_protected_dialogue_at(
                &protected_reading_input(&self.reading).unwrap(),
                &self.root.path().join("provider_artifacts"),
                budget,
                RESPONSE,
            )
        }

        fn letter(&self) -> durable_inbox::InboxReservation {
            std::fs::write(
                self.root.path().join("inbox/letter.txt"),
                "An intact letter for Astrid: café λ.",
            )
            .unwrap();
            match self
                .inbox
                .reserve(&durable_inbox::ReceiveWindow {
                    id: "explicit-mailbox-window".into(),
                    now_unix_ms: 0,
                    eligible: true,
                    max_letter_bytes: durable_inbox::ORDINARY_LETTER_BYTES,
                })
                .unwrap()
            {
                durable_inbox::InboxAdmission::Reserved(letter) => letter,
                other => panic!("expected letter, got {other:?}"),
            }
        }
    }

    #[test]
    fn reading_identity_comes_from_the_bound_snapshot_and_rejects_substitution() {
        let mut fixture = Fixture::new();
        let input = protected_reading_input(&fixture.reading).unwrap();
        assert_eq!(input.reading_source.as_ref(), Some(&fixture.reading.source));
        fixture.reading.source.sha256 = "0".repeat(64);
        assert!(protected_reading_input(&fixture.reading).is_err());
        assert_eq!(fixture.cursor(), 0);
    }

    #[test]
    fn accepted_reading_delivery_commits_exact_prefix_and_retries_idempotently_after_restart() {
        let fixture = Fixture::new();
        let result = fixture.reading_completion(1_200);
        let receipt = result.accepted_delivery.as_ref().unwrap();
        let expected = u64::try_from(receipt.admitted_end_byte).unwrap();
        assert!(expected > 0 && expected < fixture.reading.passage.end_byte);
        let outcome = commit_activity_delivery(
            &fixture.store,
            &fixture.inbox,
            Some(&fixture.reading),
            None,
            Some(receipt),
            result.text.as_deref(),
        )
        .unwrap();
        assert!(
            matches!(outcome, ActivityDeliveryOutcome::Reading { admitted_bytes } if admitted_bytes == expected)
        );
        assert_eq!(fixture.cursor(), expected);
        let reopened = ActionContinuityStore::new(fixture.store.root().to_path_buf());
        commit_activity_delivery(
            &reopened,
            &fixture.inbox,
            Some(&fixture.reading),
            None,
            Some(receipt),
            result.text.as_deref(),
        )
        .unwrap();
        assert_eq!(fixture.cursor(), expected);
    }

    #[test]
    fn no_delivery_receipt_or_substituted_response_never_advances_reading() {
        let fixture = Fixture::new();
        assert!(matches!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                Some(&fixture.reading),
                None,
                None,
                Some(RESPONSE)
            )
            .unwrap(),
            ActivityDeliveryOutcome::None
        ));
        let result = fixture.reading_completion(8_000);
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                Some(&fixture.reading),
                None,
                result.accepted_delivery.as_ref(),
                Some("A different text.\n\nNEXT: LISTEN")
            )
            .is_err()
        );
        assert_eq!(fixture.cursor(), 0);
    }

    #[test]
    fn stale_bookmark_revision_cannot_be_committed_by_late_completion() {
        let fixture = Fixture::new();
        let result = fixture.reading_completion(8_000);
        fixture
            .store
            .reader_bookmark_transition(
                &fixture.reading.reader.thread_id,
                &fixture.reading.reader.session_id,
                &fixture.reading.version,
                ReaderDisposition::Parked,
                "park-before-completion",
            )
            .unwrap();
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                Some(&fixture.reading),
                None,
                result.accepted_delivery.as_ref(),
                result.text.as_deref()
            )
            .is_err()
        );
        assert_eq!(fixture.cursor(), 0);
    }

    #[test]
    fn intact_letter_receipt_retires_only_its_version_and_never_changes_reading() {
        let fixture = Fixture::new();
        let letter = fixture.letter();
        let result = test_completed_protected_dialogue_at(
            &protected_letter_input(&letter),
            &fixture.root.path().join("provider_artifacts"),
            8_000,
            RESPONSE,
        );
        std::fs::write(
            &letter.letter.source_path,
            "A newer letter at the same pathname.",
        )
        .unwrap();
        let outcome = commit_activity_delivery(
            &fixture.store,
            &fixture.inbox,
            None,
            Some(&letter),
            result.accepted_delivery.as_ref(),
            result.text.as_deref(),
        )
        .unwrap();
        assert!(
            matches!(outcome, ActivityDeliveryOutcome::Letter(ref acknowledged) if acknowledged.version_id == letter.letter.version_id)
        );
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
        assert_eq!(fixture.cursor(), 0);
        assert_eq!(
            std::fs::read_to_string(&letter.letter.source_path).unwrap(),
            "A newer letter at the same pathname."
        );
    }

    #[test]
    fn mutated_receipt_or_wrong_target_cannot_retire_letter() {
        let fixture = Fixture::new();
        let letter = fixture.letter();
        let result = test_completed_protected_dialogue_at(
            &protected_letter_input(&letter),
            &fixture.root.path().join("provider_artifacts"),
            8_000,
            RESPONSE,
        );
        let mut receipt = result.accepted_delivery.unwrap();
        receipt.content_id = "other-letter".into();
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                None,
                Some(&letter),
                Some(&receipt),
                result.text.as_deref()
            )
            .is_err()
        );
        let reading_result = fixture.reading_completion(8_000);
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                None,
                Some(&letter),
                reading_result.accepted_delivery.as_ref(),
                reading_result.text.as_deref()
            )
            .is_err()
        );
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
    }

    #[test]
    fn ambiguous_two_target_turn_is_rejected_before_either_store_changes() {
        let fixture = Fixture::new();
        let letter = fixture.letter();
        let result = fixture.reading_completion(8_000);
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                Some(&fixture.reading),
                Some(&letter),
                result.accepted_delivery.as_ref(),
                result.text.as_deref()
            )
            .is_err()
        );
        assert_eq!(fixture.cursor(), 0);
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
    }

    #[test]
    fn artifact_loss_keeps_reading_offer_recoverable() {
        let mut fixture = Fixture::new();
        let result = fixture.reading_completion(8_000);
        let receipt = result.accepted_delivery.as_ref().unwrap();
        std::fs::remove_file(&receipt.retained_artifact_path).unwrap();
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                Some(&fixture.reading),
                None,
                Some(receipt),
                result.text.as_deref()
            )
            .is_err()
        );
        let retry = activity_reading::offer_requested_reading_in(&fixture.store, &mut fixture.conv)
            .unwrap()
            .unwrap();
        assert_eq!(retry.passage, fixture.reading.passage);
        assert_eq!(fixture.cursor(), 0);
    }
    #[test]
    fn reading_letter_detour_restart_and_explicit_return_preserve_exact_saved_place() {
        let mut fixture = Fixture::new();
        let original = std::fs::read_to_string(fixture.root.path().join("source.txt")).unwrap();
        let failed = test_completed_protected_dialogue_at(
            &protected_reading_input(&fixture.reading).unwrap(),
            &fixture.root.path().join("provider_artifacts"),
            1_200,
            "...",
        );
        assert!(matches!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                Some(&fixture.reading),
                None,
                failed.accepted_delivery.as_ref(),
                failed.text.as_deref()
            )
            .unwrap(),
            ActivityDeliveryOutcome::None
        ));
        assert_eq!(fixture.cursor(), 0);
        let accepted = fixture.reading_completion(1_200);
        commit_activity_delivery(
            &fixture.store,
            &fixture.inbox,
            Some(&fixture.reading),
            None,
            accepted.accepted_delivery.as_ref(),
            accepted.text.as_deref(),
        )
        .unwrap();
        let saved_cursor = fixture.cursor();
        activity_reading::handle_action_in(
            &fixture.store,
            &mut fixture.conv,
            "CHECK_MAILBOX",
            "CHECK_MAILBOX",
        )
        .unwrap()
        .unwrap();
        assert!(fixture.conv.activity.foreground_reader.is_none());
        assert!(fixture.conv.activity.return_reader.is_some());
        let letter_path = fixture.root.path().join("inbox/steward_letter.txt");
        std::fs::write(
            &letter_path,
            "An intact steward letter while reading is parked.",
        )
        .unwrap();
        let letter = match fixture
            .inbox
            .reserve(&durable_inbox::ReceiveWindow {
                id: fixture
                    .conv
                    .activity
                    .mailbox_window
                    .as_ref()
                    .unwrap()
                    .id
                    .clone(),
                now_unix_ms: 0,
                eligible: true,
                max_letter_bytes: durable_inbox::ORDINARY_LETTER_BYTES,
            })
            .unwrap()
        {
            durable_inbox::InboxAdmission::Reserved(letter) => letter,
            other => panic!("expected letter, got {other:?}"),
        };
        let accepted_letter = test_completed_protected_dialogue_at(
            &protected_letter_input(&letter),
            &fixture.root.path().join("provider_artifacts"),
            8_000,
            RESPONSE,
        );
        std::fs::write(
            &letter_path,
            "A second letter replaced the first path before acknowledgement.",
        )
        .unwrap();
        assert!(matches!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                None,
                Some(&letter),
                accepted_letter.accepted_delivery.as_ref(),
                accepted_letter.text.as_deref()
            )
            .unwrap(),
            ActivityDeliveryOutcome::Letter(_)
        ));
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
        std::fs::write(
            fixture.root.path().join("source.txt"),
            "The original pathname now has different bytes.",
        )
        .unwrap();
        let restarted_store = ActionContinuityStore::new(fixture.store.root().to_path_buf());
        let mut restarted = state::ConversationState::new(Vec::new(), None);
        restarted.activity = activity_reading::load_activity(&restarted_store).unwrap();
        assert!(restarted.activity.foreground_reader.is_none());
        let reader = restarted.activity.return_reader.clone().unwrap();
        let preview = restarted_store
            .reader_bookmark_preview(&reader.thread_id, &reader.session_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            preview.bookmark.as_ref().unwrap().cursor.next_byte,
            saved_cursor
        );
        activity_reading::handle_action_in(
            &restarted_store,
            &mut restarted,
            "RETURN_ACTIVITY",
            &format!("RETURN_ACTIVITY revision: {}", preview.session_record_id),
        )
        .unwrap()
        .unwrap();
        let resumed =
            activity_reading::offer_requested_reading_in(&restarted_store, &mut restarted)
                .unwrap()
                .unwrap();
        assert_eq!(resumed.passage.start_byte, saved_cursor);
        let end = usize::try_from(resumed.passage.end_byte).unwrap();
        assert_eq!(
            resumed.text,
            original[usize::try_from(saved_cursor).unwrap()..end]
        );
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
    }
}
