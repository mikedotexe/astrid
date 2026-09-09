#[cfg(test)]
mod dialogue_feedback_tests {
    use super::*;

    const BODY: &str = "I can stay with this passage and its careful account of returning to a chosen task. The words remain clear enough for me to follow their next step.";

    /// Synthetic provider bytes exercise admission, quality acceptance and the
    /// production retention seam without HTTP, environment changes or live data.
    fn attempts(
        fallback: bool,
        event: &str,
        text: &str,
    ) -> (
        SubmittedRuntimeFeedbackAttemptV1,
        SubmittedDeliveryAttemptV1,
    ) {
        let feedback = RuntimeActionFeedbackV1::from_guard_inputs(
            Some(event),
            "READ_MORE",
            "no_active_read_only_research_budget",
            "The runtime blocked this continuation.",
            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"),
        );
        let source = ProtectedDialogueInputV1 {
            reading_source: None,
            content_id: format!("source-{event}"),
            kind: ProtectedDialogueKindV1::Reading,
            source_text: "Exact chosen source: café λ🌊.\n".into(),
            source_start_byte: 0,
            reply_message_id: None,
        };
        let messages = vec![Message {
            role: "system".into(),
            content: "You are Astrid. Preserve your chosen form.".into(),
        }];
        let model = if fallback {
            "fixture-fallback-model"
        } else {
            "fixture-primary-model"
        };
        let route = if fallback {
            "mock://fallback/chat"
        } else {
            "mock://primary/completions"
        };
        let (request, protected, runtime) = if fallback {
            let mut request = build_ollama_protected_chat_request(
                "dialogue_live",
                messages,
                0.7,
                512,
                model.into(),
                true,
            );
            let (protected, runtime) = admit_runtime_feedback_and_protected_content(
                &mut request.messages,
                Some(&source),
                &[feedback],
                16_000,
            )
            .unwrap();
            (serde_json::to_vec(&request).unwrap(), protected, runtime)
        } else {
            let policy = apply_mlx_request_policy(
                "dialogue_live",
                MlxProfile::Gemma4Canary,
                messages,
                512,
                120,
            );
            let mut messages = policy.messages;
            let (protected, runtime) = admit_runtime_feedback_and_protected_content(
                &mut messages,
                Some(&source),
                &[feedback],
                4_000,
            )
            .unwrap();
            let request = MlxRequest {
                messages,
                max_tokens: 512,
                temperature: 0.7,
                stream: false,
                aperture: None,
                model_qos_v1: None,
            };
            (serde_json::to_vec(&request).unwrap(), protected, runtime)
        };
        let response = if fallback {
            serde_json::json!({"message":{"content":text},"done":true})
        } else {
            serde_json::json!({"choices":[{"message":{"content":text}}]})
        }
        .to_string();
        (
            capture_runtime_feedback_attempt(route, model, &request, &response, runtime).unwrap(),
            capture_submitted_delivery(route, model, &request, &response, protected).unwrap(),
        )
    }

    fn primary(
        text: &str,
        runtime: SubmittedRuntimeFeedbackAttemptV1,
        protected: SubmittedDeliveryAttemptV1,
    ) -> MlxChatResultV1 {
        MlxChatResultV1 {
            text: text.into(),
            runtime_feedback_attempt: Some(runtime),
            delivery_attempt: Some(protected),
            qos_request_identity_sha256: "fixture-qos".into(),
            request_content_anchor_sha256: "fixture-anchor".into(),
            queue_wait_ms: None,
            active_generation_and_reservoir_ms: None,
        }
    }

    fn fallback(
        text: &str,
        runtime: SubmittedRuntimeFeedbackAttemptV1,
        protected: SubmittedDeliveryAttemptV1,
    ) -> OllamaFallbackResponse {
        OllamaFallbackResponse {
            text: text.into(),
            model: "fixture-fallback-model".into(),
            runtime_feedback_attempt: Some(runtime),
            delivery_attempt: Some(protected),
        }
    }

    #[test]
    fn dialogue_feedback_rejected_primary_retains_only_accepted_fallback_request() {
        let (runtime, protected) = attempts(false, "rejected-primary", "...");
        let rejected_request = protected_digest(&runtime.request_json);
        let rejected_id = runtime.admission.feedback[0].id.clone();
        let primary = accept_primary_dialogue_with_feedback(
            primary("...", runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        assert!(primary.is_none());

        // Missing NEXT is repaired by the real fallback acceptance helper. The
        // receipt must retain the original provider response and repaired text.
        let (runtime, protected) = attempts(true, "accepted-fallback", BODY);
        let exact_request = runtime.request_json.clone();
        let exact_response = runtime.response_json.clone();
        let feedback_id = runtime.admission.feedback[0].id.clone();
        let fallback = accept_ollama_dialogue_with_feedback(
            fallback(BODY, runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        assert!(fallback.is_some());
        let root = tempfile::tempdir().unwrap();
        let completion = finish_accepted_dialogue_attempts_at(primary, fallback, None, root.path());
        let repaired = format!("{BODY}\n\nNEXT: LISTEN");
        assert_eq!(completion.text.as_deref(), Some(repaired.as_str()));
        let receipt = completion.accepted_runtime_feedback.unwrap();
        verify_runtime_feedback_receipt(&receipt).unwrap();
        assert_eq!(receipt.provider_model, "fixture-fallback-model");
        assert_eq!(receipt.provider_route, "mock://fallback/chat");
        assert_eq!(receipt.request_sha256, protected_digest(&exact_request));
        assert_ne!(receipt.request_sha256, rejected_request);
        assert_eq!(receipt.feedback_ids, vec![feedback_id]);
        assert!(!receipt.feedback_ids.contains(&rejected_id));
        let artifact: RetainedRuntimeFeedbackV1 =
            serde_json::from_slice(&std::fs::read(&receipt.retained_artifact_path).unwrap())
                .unwrap();
        assert_eq!(artifact.attempt.request_json, exact_request);
        assert_eq!(artifact.attempt.response_json, exact_response);
        assert_eq!(artifact.accepted_completion, repaired);
        let protected = completion.accepted_delivery.unwrap();
        verify_delivery_receipt(&protected).unwrap();
        assert_eq!(protected.content_id, "source-accepted-fallback");
        assert_eq!(protected.request_sha256, receipt.request_sha256);
        assert_eq!(
            std::fs::read_dir(root.path().join("runtime_feedback"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn dialogue_feedback_two_rejections_create_no_artifact_or_acknowledgement() {
        let (runtime, protected) = attempts(false, "rejected-primary", "...");
        let primary = accept_primary_dialogue_with_feedback(
            primary("...", runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        let (runtime, protected) = attempts(true, "rejected-fallback", "...");
        let fallback = accept_ollama_dialogue_with_feedback(
            fallback("...", runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        assert!(primary.is_none());
        assert!(fallback.is_none());
        let root = tempfile::tempdir().unwrap();
        let completion = finish_accepted_dialogue_attempts_at(primary, fallback, None, root.path());
        assert!(completion.text.is_none());
        assert!(completion.accepted_delivery.is_none());
        assert!(completion.accepted_runtime_feedback.is_none());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn dialogue_feedback_retention_failure_keeps_text_but_supplies_no_feedback_ack() {
        let text = format!("{BODY}\n\nNEXT: LISTEN");
        let (runtime, protected) = attempts(false, "accepted-primary", &text);
        let primary = accept_primary_dialogue_with_feedback(
            primary(&text, runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        assert!(primary.is_some());
        let root = tempfile::tempdir().unwrap();
        let occupied = root.path().join("runtime_feedback");
        std::fs::write(&occupied, b"unrelated existing file").unwrap();
        let completion = finish_accepted_dialogue_attempts_at(primary, None, None, root.path());
        assert_eq!(completion.text.as_deref(), Some(text.as_str()));
        assert!(completion.accepted_runtime_feedback.is_none());
        assert_eq!(
            std::fs::read(&occupied).unwrap(),
            b"unrelated existing file"
        );
        // The independently retained foreground delivery remains verifiable.
        let protected = completion.accepted_delivery.unwrap();
        verify_delivery_receipt(&protected).unwrap();
        assert_eq!(protected.content_id, "source-accepted-primary");
    }

    #[test]
    fn dialogue_feedback_accepted_primary_cannot_be_replaced_by_unused_fallback() {
        let text = format!("{BODY}\n\nNEXT: LISTEN");
        let (runtime, protected) = attempts(false, "accepted-primary", &text);
        let expected_id = runtime.admission.feedback[0].id.clone();
        let expected_request = protected_digest(&runtime.request_json);
        let primary = accept_primary_dialogue_with_feedback(
            primary(&text, runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        let (runtime, protected) = attempts(true, "unused-fallback", BODY);
        let fallback = accept_ollama_dialogue_with_feedback(
            fallback(BODY, runtime, protected),
            MlxProfile::Gemma4Canary,
        );
        let root = tempfile::tempdir().unwrap();
        let completion = finish_accepted_dialogue_attempts_at(primary, fallback, None, root.path());
        let receipt = completion.accepted_runtime_feedback.unwrap();
        verify_runtime_feedback_receipt(&receipt).unwrap();
        assert_eq!(receipt.provider_model, "fixture-primary-model");
        assert_eq!(receipt.request_sha256, expected_request);
        assert_eq!(receipt.feedback_ids, vec![expected_id]);
        assert_eq!(
            completion.accepted_delivery.unwrap().content_id,
            "source-accepted-primary"
        );
        assert_eq!(
            std::fs::read_dir(root.path().join("runtime_feedback"))
                .unwrap()
                .count(),
            1
        );
    }
}
