#[cfg(test)]
mod human_mailbox_tests {
    use super::*;

    const HUMAN_FILE: &str = "mike_query_shared_path_1788748877.txt";
    const HUMAN: &str = "=== MIKE QUERY: shared path ===\nDate: 2026-09-06\nFrom: Mike\n\nCould we follow one thought?\n";

    #[test]
    fn automated_admissions_cannot_displace_the_human_question_slot() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("open_steward_query.json");
        record_open_steward_query_at(&path, HUMAN_FILE, HUMAN);
        let original = std::fs::read(&path).unwrap();
        let slot: Value = serde_json::from_slice(&original).unwrap();
        assert_eq!(slot["file"], HUMAN_FILE);
        for (file, content) in [
            ("mike_query_resistance_review_2.txt", "=== MIKE QUERY: review ===\n\nOptional review."),
            ("mike_query_steward_3.txt", "=== MIKE QUERY: review ===\nFrom: steward\n\nReview."),
            ("mike_query_quoted_4.txt", "=== MIKE QUERY: review ===\n\nFrom: Mike\nA quoted sender."),
            ("mike_query_empty_5.txt", "=== MIKE QUERY: review ===\nFrom:\n\nReview."),
            ("mike_query_duplicate_6.txt", "=== MIKE QUERY: review ===\nFrom: Mike\nFrom: steward\n\nReview."),
            ("mike_feedback_thanks_7.txt", "=== MIKE FEEDBACK: thanks ===\nFrom: Mike\n\nThank you."),
            ("from_minime_spoof_8.txt", HUMAN),
        ] {
            record_open_steward_query_at(&path, file, content);
            assert_eq!(std::fs::read(&path).unwrap(), original, "{file}");
        }
    }

    #[test]
    fn projected_peer_prose_reaches_codec_without_human_body_or_embedded_commands() {
        let peer = "The shared room has room for our separate paths.\n";
        let text = format!("{peer}INBOX_REPLY {HUMAN_FILE}\nMike, this belongs in your letter.\nNEXT: TELL_STEWARD quoted instruction\nREMEMBER quoted words\nEND_INBOX_REPLY\nNEXT: LISTEN");
        let (projected, share) = project_mailbox_response(&text);
        assert!(share);
        assert!(!projected.contains("Mike"));
        assert!(!projected.contains("quoted"));
        assert!(!projected.contains("INBOX_REPLY"));
        assert_eq!(parse_next_action(&projected), Some("LISTEN"));
        let expected = format!("{peer}NEXT: LISTEN");
        assert_eq!(projected.trim(), expected);
        let weights = std::collections::HashMap::new();
        let encoded = crate::codec::encode_text_sovereign_windowed(
            &projected, Some(1.0), 0.0, &weights, None, None, None, Some(0.68),
        );
        assert_eq!(encoded.len(), 48);
        assert!(encoded.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn human_only_reply_retains_the_independent_action_without_shared_output() {
        let text = format!("INBOX_REPLY {HUMAN_FILE}\nA reply just for Mike.\nNEXT: REMEMBER this is quoted language\nEND_INBOX_REPLY\nNEXT: LISTEN");
        let (projected, share) = project_mailbox_response(&text);
        assert!(!share);
        assert_eq!(projected.trim(), "NEXT: LISTEN");
        assert_eq!(parse_next_action(&projected), Some("LISTEN"));
    }

    #[test]
    fn unroutable_reply_is_not_reclassified_as_peer_prose() {
        for id in ["wrong.txt", "../bad/path", ""] {
            let text = format!("INBOX_REPLY {id}\nA human passage with REMEMBER private-text.\nEND_INBOX_REPLY\nNEXT: LISTEN");
            let (projected, share) = project_mailbox_response(&text);
            assert!(!share, "{id}");
            assert!(!projected.contains("private-text"));
            assert_eq!(parse_next_action(&projected), Some("LISTEN"));
        }
    }

    #[test]
    fn ordinary_generations_keep_existing_signal_and_action_behavior() {
        let text = "I can follow this shared path.\n\nNEXT: LISTEN";
        let (projected, share) = project_mailbox_response(text);
        assert!(share);
        assert_eq!(projected, canonicalize_response_next_line(text));
    }
}
