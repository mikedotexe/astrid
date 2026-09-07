#[cfg(test)]
mod activity_human_delivery_tests {
    use super::*;
    use std::path::PathBuf;

    const NAME: &str = "mike_query_holding_a_thought_123.txt";
    const SOURCE: &str = "=== MIKE QUERY: holding a thought ===\nDate: 2026-09-06\nFrom: Mike\n\nWill you hold this thought with me? Café λ.\n";

    struct Fixture {
        root: tempfile::TempDir,
        inbox: durable_inbox::DurableInbox,
        store: crate::action_continuity::ActionContinuityStore,
        letter: durable_inbox::InboxReservation,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let inbox_dir = root.path().join("inbox");
            std::fs::create_dir(&inbox_dir).unwrap();
            std::fs::write(inbox_dir.join(NAME), SOURCE).unwrap();
            let inbox = durable_inbox::DurableInbox::new(&inbox_dir, &root.path().join("queue"));
            let letter = match inbox
                .reserve(&durable_inbox::ReceiveWindow {
                    id: "chosen-human-window".into(),
                    now_unix_ms: 0,
                    eligible: true,
                    max_letter_bytes: durable_inbox::ORDINARY_LETTER_BYTES,
                })
                .unwrap()
            {
                durable_inbox::InboxAdmission::Reserved(letter) => letter,
                other => panic!("expected human letter, got {other:?}"),
            };
            let store =
                crate::action_continuity::ActionContinuityStore::new(root.path().join("threads"));
            Self {
                root,
                inbox,
                store,
                letter,
            }
        }

        fn raw_reply(&self) -> String {
            format!(
                "INBOX_REPLY {NAME}\nMike, I can hold the particular thought you brought and return to it in my own words.\nNEXT: REMEMBER this quoted action stays human language\nEND_INBOX_REPLY\nNEXT: LISTEN"
            )
        }

        fn completion(&self, raw: &str) -> crate::llm::DialogueCompletionV1 {
            crate::llm::test_completed_protected_dialogue_at(
                &protected_letter_input(&self.letter),
                &self.root.path().join("accepted"),
                8_000,
                raw,
            )
        }

        fn commit(
            &self,
            completion: &crate::llm::DialogueCompletionV1,
        ) -> anyhow::Result<ActivityDeliveryOutcome> {
            commit_activity_delivery(
                &self.store,
                &self.inbox,
                None,
                Some(&self.letter),
                completion.accepted_delivery.as_ref(),
                completion.text.as_deref(),
            )
        }

        fn recover(&self) -> anyhow::Result<ActivityRecoverySummary> {
            recover_activity_deliveries_with_lookup(&self.store, &self.inbox, None, |id, start| {
                crate::llm::recover_retained_delivery_at(
                    &self.root.path().join("accepted"),
                    id,
                    start,
                )
            })
        }

        fn replies(&self) -> Vec<PathBuf> {
            let directory = self.root.path().join("outbox/human/mike");
            if !directory.is_dir() {
                return Vec::new();
            }
            std::fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
                .collect()
        }
    }

    #[test]
    fn retained_human_completion_publishes_exact_identity_once_before_acknowledgement() {
        let fixture = Fixture::new();
        let completion = fixture.completion(&fixture.raw_reply());
        assert!(matches!(
            fixture.commit(&completion).unwrap(),
            ActivityDeliveryOutcome::Letter(_)
        ));
        let paths = fixture.replies();
        assert_eq!(paths.len(), 1);
        let bytes = std::fs::read(&paths[0]).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains(&format!("Reply-To: {NAME}\n")));
        assert!(text.contains("To: mike\nThread-Id:") || text.contains("To: mike\nReply-To:"));
        assert!(text.contains(&format!("Thread-Id: {}\n", fixture.letter.letter.thread_id)));
        assert!(text.contains(&format!(
            "Source-SHA256: {}\n",
            fixture.letter.content_sha256
        )));
        assert!(text.contains(&format!(
            "Source-Version: {}\n",
            fixture.letter.letter.version_id
        )));
        assert!(
            text.contains(&format!(
                "Completion-SHA256: {}\n",
                completion
                    .accepted_delivery
                    .as_ref()
                    .unwrap()
                    .retained_completion_sha256
            ))
        );
        assert!(text.contains("NEXT: REMEMBER this quoted action stays human language"));
        assert!(!text.contains("NEXT: LISTEN"));
        fixture.commit(&completion).unwrap();
        assert_eq!(fixture.replies(), paths);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), bytes);
        assert_eq!(fixture.inbox.scan().unwrap().pending, 0);
        assert_eq!(
            fixture.recover().unwrap(),
            ActivityRecoverySummary::default()
        );
        assert!(!fixture.root.path().join("threads").exists());
    }

    #[test]
    fn publication_failure_keeps_letter_pending_and_recovery_needs_no_fresh_generation() {
        let fixture = Fixture::new();
        let completion = fixture.completion(&fixture.raw_reply());
        let blocker = fixture.root.path().join("outbox");
        std::fs::write(&blocker, "publication unavailable").unwrap();
        assert!(fixture.commit(&completion).is_err());
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
        assert!(!fixture.root.path().join("inbox/read").exists());
        std::fs::remove_file(blocker).unwrap();
        assert_eq!(fixture.recover().unwrap().letters_reconciled, 1);
        assert_eq!(fixture.replies().len(), 1);
        assert_eq!(
            fixture.recover().unwrap(),
            ActivityRecoverySummary::default()
        );
        assert!(!fixture.root.path().join("threads").exists());
    }

    #[test]
    fn recovery_reuses_published_reply_after_acknowledgement_failed() {
        let fixture = Fixture::new();
        let completion = fixture.completion(&fixture.raw_reply());
        let blocker = fixture.root.path().join("inbox/read");
        std::fs::write(&blocker, "archive unavailable").unwrap();
        assert!(fixture.commit(&completion).is_err());
        assert_eq!(fixture.replies().len(), 1);
        let path = fixture.replies().remove(0);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
        std::fs::remove_file(blocker).unwrap();
        assert_eq!(fixture.recover().unwrap().letters_reconciled, 1);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(fixture.replies(), vec![path]);
        assert_eq!(fixture.inbox.scan().unwrap().pending, 0);
    }

    #[test]
    fn conflicting_published_artifact_stops_recovery_without_acknowledging() {
        let fixture = Fixture::new();
        let completion = fixture.completion(&fixture.raw_reply());
        let blocker = fixture.root.path().join("inbox/read");
        std::fs::write(&blocker, "archive unavailable").unwrap();
        assert!(fixture.commit(&completion).is_err());
        let path = fixture.replies().remove(0);
        std::fs::write(&path, "conflicting existing reply").unwrap();
        std::fs::remove_file(blocker).unwrap();
        assert!(fixture.recover().is_err());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "conflicting existing reply"
        );
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
        assert!(!fixture.root.path().join("threads").exists());
    }

    #[test]
    fn replaced_filename_cannot_rebind_an_older_reply_to_new_content() {
        let fixture = Fixture::new();
        let completion = fixture.completion(&fixture.raw_reply());
        std::fs::write(
            &fixture.letter.letter.source_path,
            SOURCE.replace("Café λ", "Different content"),
        )
        .unwrap();
        fixture.commit(&completion).unwrap();
        let artifact = std::fs::read_to_string(fixture.replies().remove(0)).unwrap();
        assert!(artifact.contains(&format!(
            "Source-SHA256: {}\n",
            fixture.letter.content_sha256
        )));
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
        let next = match fixture
            .inbox
            .reserve(&durable_inbox::ReceiveWindow {
                id: "new-version-window".into(),
                now_unix_ms: 30_000,
                eligible: true,
                max_letter_bytes: 6_000,
            })
            .unwrap()
        {
            durable_inbox::InboxAdmission::Reserved(letter) => letter,
            other => panic!("expected replacement letter, got {other:?}"),
        };
        assert_eq!(next.letter.message_id, fixture.letter.letter.message_id);
        assert_ne!(next.letter.version_id, fixture.letter.letter.version_id);
        assert_ne!(next.content_sha256, fixture.letter.content_sha256);
    }

    #[test]
    fn tampered_reservation_and_unretained_completions_cannot_publish() {
        let fixture = Fixture::new();
        let completion = fixture.completion(&fixture.raw_reply());
        let mut tampered = fixture.letter.clone();
        tampered.letter.thread_id = "invented_thread".into();
        assert!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                None,
                Some(&tampered),
                completion.accepted_delivery.as_ref(),
                completion.text.as_deref()
            )
            .is_err()
        );
        assert!(fixture.replies().is_empty());
        assert!(matches!(
            commit_activity_delivery(
                &fixture.store,
                &fixture.inbox,
                None,
                Some(&fixture.letter),
                None,
                completion.text.as_deref()
            )
            .unwrap(),
            ActivityDeliveryOutcome::None
        ));
        assert!(fixture.replies().is_empty());
        let mut partial = completion.accepted_delivery.as_ref().unwrap().clone();
        partial.admitted_end_byte = partial.admitted_end_byte.saturating_sub(1);
        assert!(commit_activity_delivery(&fixture.store, &fixture.inbox, None,
            Some(&fixture.letter), Some(&partial), completion.text.as_deref()).is_err());
        assert!(fixture.replies().is_empty());
        std::fs::remove_file(
            &completion
                .accepted_delivery
                .as_ref()
                .unwrap()
                .retained_artifact_path,
        )
        .unwrap();
        assert!(fixture.commit(&completion).is_err());
        assert!(fixture.replies().is_empty());
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
    }

    #[test]
    fn unaddressed_and_wrong_address_completions_acknowledge_without_a_human_reply() {
        for raw in [
            "An ordinary peer reflection, chosen freely after reading this complete letter. Its ideas can remain with me without claiming that this answers Mike.\nNEXT: LISTEN".to_string(),
            "INBOX_REPLY different.txt\nThis is an explicit address-shaped body for the wrong letter, retained as evidence without an inferred recipient.\nEND_INBOX_REPLY\nNEXT: LISTEN".to_string(),
        ] {
            let fixture = Fixture::new();
            let completion = fixture.completion(&raw);
            assert!(matches!(fixture.commit(&completion).unwrap(), ActivityDeliveryOutcome::Letter(_)));
            assert!(fixture.replies().is_empty());
            assert_eq!(fixture.inbox.scan().unwrap().pending, 0);
        }
    }

    #[test]
    fn a_different_completion_cannot_publish_after_the_source_is_already_acknowledged() {
        let fixture = Fixture::new();
        let first = fixture.completion("This is an ordinary completed reflection after reading the whole letter. I choose to keep it as shared prose without declaring a human recipient.\nNEXT: LISTEN");
        fixture.commit(&first).unwrap();
        assert!(fixture.replies().is_empty());
        let before = std::fs::read(fixture.root.path().join("queue/state.json")).unwrap();
        let different = fixture.completion(&fixture.raw_reply());
        assert!(fixture.commit(&different).is_err());
        assert!(fixture.replies().is_empty());
        assert_eq!(std::fs::read(fixture.root.path().join("queue/state.json")).unwrap(), before);
    }

    #[test]
    fn unterminated_human_reply_cannot_invent_an_independent_next_or_acknowledge() {
        let fixture = Fixture::new();
        let raw = format!("INBOX_REPLY {NAME}\nThis human address is unfinished and therefore cannot become a confirmed addressed reply, despite its otherwise meaningful content.\nNEXT: LISTEN");
        let completion = fixture.completion(&raw);
        assert!(completion.accepted_delivery.is_none());
        assert!(matches!(fixture.commit(&completion).unwrap(), ActivityDeliveryOutcome::None));
        assert!(fixture.replies().is_empty());
        assert_eq!(fixture.inbox.scan().unwrap().pending, 1);
    }
}
