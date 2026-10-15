#[cfg(test)]
mod writing_admission_tests {
    use super::*;
    use astrid_source_study::writing::Profile;

    fn input(text: String) -> ProtectedDialogueInputV1 {
        ProtectedDialogueInputV1 { content_id: "synthetic-writing-boundary".into(),
            kind: ProtectedDialogueKindV1::PrivateWriting, source_text: text,
            source_start_byte: 0, reply_message_id: None, reading_source: None }
    }

    #[tokio::test]
    async fn final_primary_and_fallback_admission_counts_the_actual_request() {
        let base = vec![Message { role: "system".into(), content: "Synthetic complete-input fixture.".into() }];
        for profile in [Profile::Default, Profile::Short, Profile::Extended] {
            let primary = apply_mlx_request_policy_with_writing("private_writing", MlxProfile::Gemma4Canary, base.clone(), 4096, 480, profile);
            assert_eq!(primary.max_tokens, if profile == Profile::Short { 512 } else { 8192 });
            let fallback = build_ollama_protected_chat_request("private_writing", base.clone(), 0.7, 4096, "fixture".into(), true);
            for (route, adapted) in [("mlx", primary.messages), ("ollama", fallback.messages)] {
                let empty = input(String::new());
                let capacity = final_writing_capacity(&adapted, &empty, 48_000).unwrap();
                let available = 48_000usize.saturating_sub(capacity.system_bytes).saturating_sub(capacity.wrapper_bytes);
                for excess in [0, 1, 8] {
                    WRITING_ADMISSION.scope(std::cell::RefCell::default(), async {
                        // Multibyte source plus an exact ASCII tail exercises byte, not character, limits.
                        let text = format!("{}{}", "\u{03bb}".repeat(available / 2), "x".repeat(available % 2 + excess));
                        let input = input(text.clone());
                        let mut messages = adapted.clone();
                        let admitted = admit_feedback_and_afterimages(&mut messages, Some(&input), &[], 48_000, route, "fixture");
                        if excess == 0 {
                            assert!(admitted.is_some());
                            assert_eq!(message_prompt_chars(&messages), 48_000);
                            assert!(messages.last().unwrap().content.contains(&text));
                            assert!(writing_admission_failure().is_none());
                        } else {
                            assert!(admitted.is_none());
                            let failure = writing_admission_failure().unwrap();
                            assert_eq!(failure.complete_bytes(), Some(48_000 + excess));
                            assert_eq!(failure.prepared_input_bytes, text.len());
                            assert_eq!(serde_json::to_value(messages).unwrap(), serde_json::to_value(&adapted).unwrap());
                        }
                    }).await;
                }
            }
        }
    }

    #[tokio::test]
    async fn helper_fit_is_not_final_fit_and_an_admitted_fallback_is_not_capacity_failure() {
        WRITING_ADMISSION.scope(std::cell::RefCell::default(), async {
            let input = input("x".repeat(47_000));
            let base = vec![Message { role: "system".into(), content: "Synthetic instructions.".into() }];
            assert!(input.source_text.len() + base[0].content.len() + 32 < 48_000);
            let mut large = apply_mlx_request_policy_with_writing("private_writing", MlxProfile::Gemma4Canary, base.clone(), 4096, 480, Profile::Default).messages;
            // Additional real system messages must count too; no first-message approximation.
            large.push(Message { role: "system".into(), content: "s".repeat(500) });
            assert!(admit_feedback_and_afterimages(&mut large, Some(&input), &[], 48_000, "mlx", "fixture").is_none());
            assert!(writing_admission_failure().is_some());
            let mut small = base;
            assert!(admit_feedback_and_afterimages(&mut small, Some(&input), &[], 48_000, "ollama", "fixture").is_some());
            assert!(writing_admission_failure().is_none());
            // Even if a subsequent route is refused, the admitted attempt may have executed.
            record_writing_admission(final_writing_capacity(&large, &input, 48_000), false);
            assert!(writing_admission_failure().is_none());
        }).await;
        assert!(writing_admission_failure().is_none());
    }

    #[tokio::test]
    async fn admission_evidence_is_task_local_and_unknown_failure_stays_unknown() {
        let refused = async {
            WRITING_ADMISSION.scope(std::cell::RefCell::default(), async {
                let mut messages = vec![];
                assert!(admit_feedback_and_afterimages(&mut messages, Some(&input("x".repeat(48_000))), &[], 48_000, "mlx", "fixture").is_none());
                tokio::task::yield_now().await;
                assert!(writing_admission_failure().is_some());
                record_writing_admission(None, false);
                assert!(writing_admission_failure().is_none());
            }).await;
        };
        let independent = async {
            WRITING_ADMISSION.scope(std::cell::RefCell::default(), async {
                tokio::task::yield_now().await;
                assert!(writing_admission_failure().is_none());
            }).await;
        };
        tokio::join!(refused, independent);
    }

    #[test]
    fn private_wrapper_allows_stopping_and_ambient_labels_do_not_invent_sensory_provenance() {
        let ending = protected_source_ending(&input("Exact prose.".into()));
        assert!(ending.contains("NEXT is optional"));
        assert!(ending.contains("no NEXT or NEXT: REST stops"));
        assert!(!ending.contains("end with one final NEXT"));
        assert!(!ending.contains("Continue your chosen writing"));
        let text = "Mailbox: 21 waiting.\nHistorical passage: exact words.";
        let context = format_dialogue_ambient_perception_block(text);
        assert!(context.contains(text));
        assert!(context.contains("source labels and timestamps apply"));
        assert!(!context.contains("YOUR senses"));
        assert!(format_dialogue_direct_perception_block("Fresh objection.").contains("Fresh objection."));
    }
}
