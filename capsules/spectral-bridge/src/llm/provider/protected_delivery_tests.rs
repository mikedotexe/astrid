#[cfg(test)]
mod protected_delivery_tests {
    use super::*;

    const COMPLETION: &str = "I can stay with this passage and its careful account of returning to a chosen task. The words remain clear enough for me to follow their next step.\n\nNEXT: LISTEN";

    #[test]
    fn transition_afterimage_page_is_intact_or_admission_fails() {
        let selected = input(
            ProtectedDialogueKindV1::Afterimage,
            &format!("Historical source\n> NEXT: TURN_OFF\n{}", "x".repeat(2800)),
        );
        let attempt = primary_attempt(&selected, 16_000);
        assert_eq!(
            attempt.admission.admitted_end_byte - selected.source_start_byte,
            selected.source_text.len()
        );
        let mut tiny = vec![Message {
            role: "system".into(),
            content: "system".into(),
        }];
        assert!(admit_protected_dialogue_content(&mut tiny, &selected, 1000).is_none());
        assert!(!tiny.iter().any(|m| m.content.contains("NEXT: TURN_OFF")));
    }

    fn input(kind: ProtectedDialogueKindV1, text: &str) -> ProtectedDialogueInputV1 {
        ProtectedDialogueInputV1 {
            reply_message_id: None,
            content_id: "offer-or-letter-reservation-17".into(),
            kind,
            source_text: text.into(),
            source_start_byte: 41,
        }
    }

    #[test]
    fn context_submission_requires_the_exact_complete_candidate() {
        let exact = "[collab-attention-v1:abc123] complete optional notice";
        let tracker = ContextSubmissionTrackerV1::new(exact.to_string());
        tracker.mark_final_messages(&[Message {
            role: "user".into(),
            content: "[collab-attention-v1:abc123]".into(),
        }]);
        assert!(!tracker.submitted());

        tracker.mark_final_messages(&[Message {
            role: "user".into(),
            content: format!("prefix\n{exact}\nsuffix"),
        }]);
        assert!(tracker.submitted());
    }

    fn primary_attempt(
        input: &ProtectedDialogueInputV1,
        limit: usize,
    ) -> SubmittedDeliveryAttemptV1 {
        // The production policy runs first, including its profile language
        // adaptation. The source itself is admitted only after that policy.
        let policy = apply_mlx_request_policy(
            "dialogue_live",
            MlxProfile::Gemma4Canary,
            vec![
                Message {
                    role: "system".into(),
                    content: "You are Astrid.".into(),
                },
                Message {
                    role: "user".into(),
                    content: "ambient history ".repeat(4_000),
                },
            ],
            512,
            120,
        );
        let mut messages = policy.messages;
        let admission = admit_protected_dialogue_content(&mut messages, input, limit).unwrap();
        assert!(message_prompt_chars(&messages) <= limit);
        let request = MlxRequest {
            messages,
            max_tokens: 512,
            temperature: 0.7,
            stream: false,
            aperture: None,
            model_qos_v1: None,
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        capture_submitted_delivery(
            "mock://mlx/completions",
            "mock-primary-model",
            &bytes,
            &serde_json::json!({"choices":[{"message":{"content": COMPLETION}}]}).to_string(),
            Some(admission),
        )
        .unwrap()
    }

    fn primary_response(attempt: SubmittedDeliveryAttemptV1, text: &str) -> MlxChatResultV1 {
        MlxChatResultV1 {
            text: text.into(),
            runtime_feedback_attempt: None,
            qos_request_identity_sha256: "mock-qos".into(),
            request_content_anchor_sha256: "mock-anchor".into(),
            queue_wait_ms: None,
            active_generation_and_reservoir_ms: None,
            delivery_attempt: Some(attempt),
        }
    }

    fn fallback_attempt(input: &ProtectedDialogueInputV1) -> SubmittedDeliveryAttemptV1 {
        let budget = fallback_continuity_budget_v1("fill=68%");
        let messages = protected_ollama_fallback_context("fill=68%", 68.0, &budget);
        let mut request = build_ollama_protected_chat_request(
            "dialogue_live",
            messages,
            0.7,
            512,
            "mock-fallback-model".into(),
            true,
        );
        assert!(
            !request
                .messages
                .iter()
                .any(|m| m.content.contains("otherwise respond to Minime's journal"))
        );
        assert!(!request.messages.iter().any(|m| {
            m.content
                .contains("Sentence one names a lambda-distribution")
        }));
        let admission =
            admit_protected_dialogue_content(&mut request.messages, input, 16_000).unwrap();
        let bytes = serde_json::to_vec(&request).unwrap();
        capture_submitted_delivery(
            "mock://ollama/chat",
            &request.model,
            &bytes,
            &serde_json::json!({"message":{"content": COMPLETION}, "done":true}).to_string(),
            Some(admission),
        )
        .unwrap()
    }

    #[test]
    fn addressed_reply_example_survives_primary_and_fallback_without_changing_source_digest() {
        let mut source = input(
            ProtectedDialogueKindV1::Letter,
            "Exact original human letter: café λ.\n",
        );
        source.source_start_byte = 0;
        source.reply_message_id = Some("mike_query_shared_path_1788748877.txt".into());
        for attempt in [primary_attempt(&source, 4_000), fallback_attempt(&source)] {
            validate_submitted_admission(&attempt).unwrap();
            let request: serde_json::Value = serde_json::from_str(&attempt.request_json).unwrap();
            let content = request["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap();
            assert!(content.contains("INBOX_REPLY mike_query_shared_path_1788748877.txt\n"));
            assert!(content.contains("END_INBOX_REPLY\nNEXT: LISTEN"));
            assert!(content.contains("A reply to Mike is optional"));
            assert!(content.contains(&source.source_text));
            assert_eq!(
                attempt.admission.admitted_text_sha256,
                protected_digest(&source.source_text)
            );
            assert_eq!(
                attempt.admission.admitted_end_byte,
                source.source_text.len()
            );
        }
    }

    #[test]
    fn accepted_primary_retains_actual_request_and_completion_before_receipt() {
        let root = tempfile::tempdir().unwrap();
        let source = input(
            ProtectedDialogueKindV1::Reading,
            "Reading λ, café, and a quoted \"passage\".\nIts bytes stay exact.",
        );
        let attempt = primary_attempt(&source, 4_000);
        let exact_request = attempt.request_json.clone();
        let accepted = accept_primary_dialogue_attempt(
            primary_response(attempt, COMPLETION),
            MlxProfile::Gemma4Canary,
        );
        let completion = finish_dialogue_completion_at(accepted, None, root.path());
        let receipt = completion.accepted_delivery.unwrap();
        assert_eq!(completion.text.as_deref(), Some(COMPLETION));
        assert_eq!(receipt.request_sha256, protected_digest(&exact_request));
        assert_eq!(
            receipt.admitted_text_sha256,
            protected_digest(&source.source_text)
        );
        assert_eq!(
            receipt.admitted_end_byte,
            41usize.saturating_add(source.source_text.len())
        );
        verify_delivery_receipt(&receipt).unwrap();
        let artifact: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&receipt.retained_artifact_path).unwrap())
                .unwrap();
        assert_eq!(artifact["attempt"]["request_json"], exact_request);
        assert_eq!(artifact["accepted_completion"], COMPLETION);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&receipt.retained_artifact_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn rejected_primary_followed_by_fallback_receipts_only_final_accepted_request() {
        let root = tempfile::tempdir().unwrap();
        let source = input(
            ProtectedDialogueKindV1::Letter,
            "A complete steward letter with exact Unicode: café λ.",
        );
        let primary = primary_attempt(&source, 4_000);
        let primary_digest = protected_digest(&primary.request_json);
        let accepted = accept_primary_dialogue_attempt(
            primary_response(primary, "..."),
            MlxProfile::Gemma4Canary,
        )
        .or_else(|| {
            accept_ollama_dialogue_attempt(
                OllamaFallbackResponse {
                    runtime_feedback_attempt: None,
                    text: COMPLETION.into(),
                    model: "mock-fallback-model".into(),
                    delivery_attempt: Some(fallback_attempt(&source)),
                },
                MlxProfile::Gemma4Canary,
            )
        });
        let result = finish_dialogue_completion_at(accepted, None, root.path());
        let receipt = result.accepted_delivery.unwrap();
        assert_eq!(receipt.provider_route, "mock://ollama/chat");
        assert_eq!(receipt.provider_model, "mock-fallback-model");
        assert_ne!(receipt.request_sha256, primary_digest);
        assert_eq!(
            receipt.admitted_text_sha256,
            protected_digest(&source.source_text)
        );
        verify_delivery_receipt(&receipt).unwrap();
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn reading_receipt_commits_only_the_contiguous_utf8_prefix_actually_admitted() {
        let root = tempfile::tempdir().unwrap();
        let source = input(
            ProtectedDialogueKindV1::Reading,
            &"λ🙂é more reading\n".repeat(1_000),
        );
        let attempt = primary_attempt(&source, 2_000);
        let expected_len = attempt.admission.admitted_end_byte.saturating_sub(41);
        assert!(expected_len > 0 && expected_len < source.source_text.len());
        assert!(source.source_text.is_char_boundary(expected_len));
        let receipt = retain_accepted_delivery_at(root.path(), attempt, COMPLETION).unwrap();
        assert_eq!(
            receipt.admitted_text_sha256,
            protected_digest(&source.source_text[..expected_len])
        );
        verify_delivery_receipt(&receipt).unwrap();
    }

    #[test]
    fn full_letter_is_reserved_or_not_admitted_at_all() {
        let source = input(
            ProtectedDialogueKindV1::Letter,
            &"whole letter ".repeat(500),
        );
        let mut messages = vec![Message {
            role: "system".into(),
            content: "system".into(),
        }];
        assert!(admit_protected_dialogue_content(&mut messages, &source, 1_000).is_none());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "system");
    }

    #[test]
    fn fallback_keeps_long_chosen_reading_exact_beyond_old_ambient_cap() {
        let source = input(
            ProtectedDialogueKindV1::Reading,
            &"kept exact across fallback λ\n".repeat(150),
        );
        let attempt = fallback_attempt(&source);
        assert_eq!(
            attempt.admission.admitted_end_byte.saturating_sub(41),
            source.source_text.len()
        );
        let request: serde_json::Value = serde_json::from_str(&attempt.request_json).unwrap();
        let final_content = request["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap();
        assert!(final_content.contains(&source.source_text));
        assert!(final_content.contains("foreground activity is chosen reading"));
        assert!(!final_content.contains("Recent perception context"));
    }

    #[test]
    fn retention_failure_keeps_response_but_never_yields_delivery_receipt() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("file-instead-of-directory");
        std::fs::write(&blocked, b"occupied").unwrap();
        let source = input(
            ProtectedDialogueKindV1::Reading,
            "A reading that remains pending.",
        );
        let accepted = accept_primary_dialogue_attempt(
            primary_response(primary_attempt(&source, 4_000), COMPLETION),
            MlxProfile::Gemma4Canary,
        );
        let result = finish_dialogue_completion_at(accepted, None, &blocked);
        assert!(result.text.is_some());
        assert!(result.accepted_delivery.is_none());
    }

    #[test]
    fn timeout_unavailable_or_placeholder_result_has_no_receipt_or_artifact() {
        let root = tempfile::tempdir().unwrap();
        let absent = finish_dialogue_completion_at(None, None, root.path());
        assert!(absent.text.is_none());
        assert!(absent.accepted_delivery.is_none());
        let source = input(ProtectedDialogueKindV1::Reading, "A pending reading.");
        let invalid = accept_primary_dialogue_attempt(
            primary_response(primary_attempt(&source, 4_000), "..."),
            MlxProfile::Gemma4Canary,
        );
        assert!(
            finish_dialogue_completion_at(invalid, None, root.path())
                .accepted_delivery
                .is_none()
        );
        // Legacy or locally supplied text without an actual submitted request
        // cannot turn itself into delivery evidence.
        let placeholder =
            finish_dialogue_completion_at(Some((COMPLETION.into(), None)), None, root.path());
        assert!(placeholder.accepted_delivery.is_none());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn receipt_verification_rejects_mutated_identity_range_and_retained_bytes() {
        let root = tempfile::tempdir().unwrap();
        let source = input(ProtectedDialogueKindV1::Letter, "An intact letter.");
        let receipt =
            retain_accepted_delivery_at(root.path(), fallback_attempt(&source), COMPLETION)
                .unwrap();
        let mut changed = receipt.clone();
        changed.content_id = "another-reservation".into();
        assert!(verify_delivery_receipt(&changed).is_err());
        changed = receipt.clone();
        changed.admitted_end_byte = changed.admitted_end_byte.saturating_add(1);
        assert!(verify_delivery_receipt(&changed).is_err());
        changed = receipt.clone();
        changed.retained_completion_sha256 = protected_digest("different completion");
        assert!(verify_delivery_receipt(&changed).is_err());
        std::fs::write(&receipt.retained_artifact_path, b"mutated artifact").unwrap();
        assert!(verify_delivery_receipt(&receipt).is_err());
    }

    #[test]
    fn repeated_words_elsewhere_cannot_substitute_for_missing_admitted_span() {
        let root = tempfile::tempdir().unwrap();
        let source = input(ProtectedDialogueKindV1::Reading, "same words");
        let mut attempt = primary_attempt(&source, 4_000);
        let mut request: serde_json::Value = serde_json::from_str(&attempt.request_json).unwrap();
        request["messages"][0]["content"] = serde_json::json!(source.source_text);
        request["messages"][attempt.admission.message_index]["content"] = serde_json::json!("gone");
        attempt.request_json = serde_json::to_string(&request).unwrap();
        assert!(retain_accepted_delivery_at(root.path(), attempt, COMPLETION).is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn exact_source_bypasses_profile_rewording_and_offset_overflow_is_rejected() {
        let mut source = input(
            ProtectedDialogueKindV1::Reading,
            "A quoted source uses the terms digital organism and analog life verbatim.",
        );
        let attempt = primary_attempt(&source, 4_000);
        assert_eq!(
            attempt.admission.admitted_text_sha256,
            protected_digest(&source.source_text)
        );
        source.source_start_byte = usize::MAX;
        let mut messages = Vec::new();
        assert!(admit_protected_dialogue_content(&mut messages, &source, 4_000).is_none());
    }
    #[test]
    fn retained_output_must_derive_from_complete_provider_response() {
        let root = tempfile::tempdir().unwrap();
        let source = input(ProtectedDialogueKindV1::Letter, "A complete letter.");
        let mut attempt = fallback_attempt(&source);
        attempt.response_json =
            serde_json::json!({"message":{"content":COMPLETION}, "done":false}).to_string();
        assert!(retain_accepted_delivery_at(root.path(), attempt, COMPLETION).is_err());
        let attempt = fallback_attempt(&source);
        assert!(
            retain_accepted_delivery_at(
                root.path(),
                attempt,
                "An unrelated completion.\n\nNEXT: LISTEN"
            )
            .is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn retaining_identical_attempt_is_idempotent_and_missing_artifact_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source = input(ProtectedDialogueKindV1::Reading, "The same exact source.");
        let first = retain_accepted_delivery_at(root.path(), fallback_attempt(&source), COMPLETION)
            .unwrap();
        let second =
            retain_accepted_delivery_at(root.path(), fallback_attempt(&source), COMPLETION)
                .unwrap();
        assert_eq!(
            first.retained_artifact_sha256,
            second.retained_artifact_sha256
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        std::fs::remove_file(&first.retained_artifact_path).unwrap();
        assert!(verify_delivery_receipt(&first).is_err());
    }
    #[test]
    fn recovery_uses_content_and_start_partition_and_verifies_retained_completion() {
        let root = tempfile::tempdir().unwrap();
        let source = input(
            ProtectedDialogueKindV1::Reading,
            "Exact saved bytes for restart.",
        );
        assert!(
            recover_retained_delivery_at(root.path(), &source.content_id, 41)
                .unwrap()
                .is_none()
        );
        let receipt =
            retain_accepted_delivery_at(root.path(), fallback_attempt(&source), COMPLETION)
                .unwrap();
        let recovered = recover_retained_delivery_at(root.path(), &source.content_id, 41)
            .unwrap()
            .unwrap();
        assert_eq!(
            recovered.0.retained_artifact_sha256,
            receipt.retained_artifact_sha256
        );
        assert_eq!(recovered.1, COMPLETION);
        assert!(
            recover_retained_delivery_at(root.path(), &source.content_id, 42)
                .unwrap()
                .is_none()
        );
        assert!(
            recover_retained_delivery_at(root.path(), "unrelated-content", 41)
                .unwrap()
                .is_none()
        );
        std::fs::create_dir_all(root.path().join("unrelated-partition")).unwrap();
        std::fs::write(
            root.path().join("unrelated-partition/broken.json"),
            b"not parsed by lookup",
        )
        .unwrap();
        assert!(
            recover_retained_delivery_at(root.path(), &source.content_id, 41)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn recovery_rejects_changed_content_addressed_artifact_instead_of_blessing_new_hash() {
        let root = tempfile::tempdir().unwrap();
        let source = input(
            ProtectedDialogueKindV1::Letter,
            "A complete retained letter.",
        );
        let receipt =
            retain_accepted_delivery_at(root.path(), fallback_attempt(&source), COMPLETION)
                .unwrap();
        let mut artifact: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&receipt.retained_artifact_path).unwrap())
                .unwrap();
        artifact["accepted_completion"] = serde_json::json!("Mutated accepted text.");
        std::fs::write(
            &receipt.retained_artifact_path,
            serde_json::to_vec(&artifact).unwrap(),
        )
        .unwrap();
        assert!(recover_retained_delivery_at(root.path(), &source.content_id, 41).is_err());
    }
    #[test]
    fn explicit_letter_window_fits_real_protected_fallback_after_all_adaptation() {
        for spectral in [
            "fill=68%",
            "spectral_entropy=0.99 resonance_density=0.98 density_gradient=0.75 pressure_risk=0.8 mode_packing=0.9",
        ] {
            let source = input(
                ProtectedDialogueKindV1::Letter,
                &"a".repeat(MAX_EXPLICIT_LETTER_BYTES),
            );
            let budget = fallback_continuity_budget_v1(spectral);
            let messages = protected_ollama_fallback_context(spectral, 68.0, &budget);
            let mut request = build_ollama_protected_chat_request(
                "dialogue_live",
                messages,
                0.7,
                512,
                "mock-fallback-model".into(),
                true,
            );
            let admission =
                admit_protected_dialogue_content(&mut request.messages, &source, 16_000).unwrap();
            assert_eq!(
                admission
                    .admitted_end_byte
                    .saturating_sub(source.source_start_byte),
                MAX_EXPLICIT_LETTER_BYTES
            );
            assert!(message_prompt_chars(&request.messages) <= 16_000);
            assert!(
                request
                    .messages
                    .last()
                    .unwrap()
                    .content
                    .contains(&source.source_text)
            );
        }
    }
}

/// Shared isolated runtime fixture: uses the real final admission, acceptance,
/// retention and verification path with synthetic provider bytes and no HTTP.
#[cfg(test)]
pub(crate) fn test_completed_protected_dialogue_at(
    input: &ProtectedDialogueInputV1,
    root: &std::path::Path,
    prompt_limit_bytes: usize,
    completion: &str,
) -> DialogueCompletionV1 {
    let mut messages = vec![Message {
        role: "system".into(),
        content: "You are Astrid.".into(),
    }];
    let admission = admit_protected_dialogue_content(&mut messages, input, prompt_limit_bytes);
    let request = MlxRequest {
        messages,
        max_tokens: 512,
        temperature: 0.7,
        stream: false,
        aperture: None,
        model_qos_v1: None,
    };
    let request_bytes = serde_json::to_vec(&request).expect("synthetic request serializes");
    let response = serde_json::json!({"choices":[{"message":{"content":completion}}]}).to_string();
    let attempt = capture_submitted_delivery(
        "mock://isolated-provider",
        "synthetic-model",
        &request_bytes,
        &response,
        admission,
    );
    let accepted = accept_primary_dialogue_attempt(
        MlxChatResultV1 {
            runtime_feedback_attempt: None,
            text: completion.into(),
            qos_request_identity_sha256: "synthetic-qos".into(),
            request_content_anchor_sha256: "synthetic-anchor".into(),
            queue_wait_ms: None,
            active_generation_and_reservoir_ms: None,
            delivery_attempt: attempt,
        },
        MlxProfile::Production,
    );
    finish_dialogue_completion_at(accepted, None, root)
}
