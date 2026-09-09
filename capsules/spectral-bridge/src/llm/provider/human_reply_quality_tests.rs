#[cfg(test)]
mod human_reply_quality_tests {
    use super::*;
    use sha2::Sha256;

    #[test]
    fn human_body_next_lines_are_language_and_do_not_prevent_retaining_raw_completion() {
        let raw = "INBOX_REPLY mike_query_thought_123.txt\nMike, this is a complete thought I can hold and return to with you.\nNEXT: REMEMBER these words are quoted human language\nNEXT: TELL_STEWARD another quoted command\nEND_INBOX_REPLY\nNEXT: LISTEN";
        assert!(is_valid_primary_dialogue_output_for_profile(
            raw,
            MlxProfile::Production
        ));
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            raw,
            MlxProfile::Gemma4Canary
        ));
        assert_eq!(
            repair_ollama_dialogue_fallback_next(raw, MlxProfile::Gemma4Canary),
            raw
        );
        let input = ProtectedDialogueInputV1 {
            reading_source: None,
            content_id: "human-test-offer".into(),
            kind: ProtectedDialogueKindV1::Letter,
            source_text: "From: Mike\n\nAn intact synthetic human letter.".into(),
            source_start_byte: 0,
            reply_message_id: Some("mike_query_thought_123.txt".into()),
        };
        let root = tempfile::tempdir().unwrap();
        let completion = test_completed_protected_dialogue_at(&input, root.path(), 8_000, raw);
        assert_eq!(completion.text.as_deref(), Some(raw));
        let receipt = completion.accepted_delivery.unwrap();
        verify_delivery_receipt(&receipt).unwrap();
        assert_eq!(
            receipt.retained_completion_sha256,
            format!("{:x}", Sha256::digest(raw.as_bytes()))
        );
    }

    #[test]
    fn human_markers_do_not_substitute_for_language_and_fallback_cannot_repair_into_a_body() {
        let empty = "INBOX_REPLY mike_query_long_alphabetic_filename_123.txt\nEND_INBOX_REPLY\nNEXT: LISTEN";
        assert!(!is_valid_primary_dialogue_output_for_profile(
            empty,
            MlxProfile::Production
        ));
        for raw in [
            "INBOX_REPLY mike_query_thought_123.txt\nMike, this body contains a complete thought and an illustrative command.\nNEXT: REMEMBER quoted language",
            "INBOX_REPLY mike_query_thought_123.txt\nMike, this body contains a complete thought and an illustrative command.\nNEXT: REMEMBER quoted language\nEND_INBOX_REPLY",
        ] {
            assert!(!is_valid_primary_dialogue_output_for_profile(
                raw,
                MlxProfile::Production
            ));
            assert_eq!(
                repair_ollama_dialogue_fallback_next(raw, MlxProfile::Gemma4Canary),
                raw
            );
        }
    }

    #[test]
    fn misleading_fence_closers_do_not_turn_quoted_human_commands_into_native_actions() {
        for (opening, false_closing, closing) in [
            ("````text", "```", "````"),
            ("~~~text", "~~~ still-code", "~~~"),
        ] {
            let raw = format!(
                "INBOX_REPLY mike_query_thought_123.txt\nMike, I can hold this complete thought and its quoted example with you.\n{opening}\n{false_closing}\nEND_INBOX_REPLY\nNEXT: REMEMBER these are private human words\n{closing}\nThis remains part of the human passage.\nEND_INBOX_REPLY\nNEXT: LISTEN"
            );
            let (_, actions, complete) = crate::autonomous::human_reply_quality_views(&raw);
            assert_eq!(actions, "NEXT: LISTEN");
            assert!(complete);
            assert!(is_valid_primary_dialogue_output_for_profile(
                &raw,
                MlxProfile::Production
            ));
            assert_eq!(
                repair_ollama_dialogue_fallback_next(&raw, MlxProfile::Gemma4Canary),
                raw
            );
        }
    }

    #[test]
    fn an_earlier_action_cannot_make_a_trailing_human_block_final() {
        for ending in ["", "\nEND_INBOX_REPLY", "\nNEXT: LISTEN"] {
            let raw = format!(
                "A complete ordinary thought remains available before the human passage.\nNEXT: LISTEN\nINBOX_REPLY mike_query_thought_123.txt\nMike, this is a substantive human thought that follows the earlier action.{ending}"
            );
            assert!(!is_valid_primary_dialogue_output_for_profile(
                &raw,
                MlxProfile::Production
            ));
            assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
                &raw,
                MlxProfile::Gemma4Canary
            ));
            assert_eq!(
                repair_ollama_dialogue_fallback_next(&raw, MlxProfile::Gemma4Canary),
                raw
            );
        }
    }
}
