#[cfg(test)]
mod runtime_feedback_tests {
    use super::*;
    use crate::runtime_action_feedback::consume_runtime_feedback_ids;

    const AUTHORED: &str = "You are Astrid.\n[For this exchange, you chose to emphasize: attend to the melody. This is your own direction.]\nExpress your response as a sonnet.";
    const COMPLETION: &str = "I can distinguish the runtime's report from my own preference. The requested continuation is blocked for now, while the original passage remains available for a later chosen step.\n\nNEXT: LISTEN";

    fn feedback() -> RuntimeActionFeedbackV1 {
        RuntimeActionFeedbackV1::from_guard_inputs(
            Some("act_fixture_read_more"),
            "READ_MORE",
            "no_active_read_only_research_budget",
            "No active read-only research budget can spend this action.",
            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"),
        )
    }

    fn source(kind: ProtectedDialogueKindV1, text: &str) -> ProtectedDialogueInputV1 {
        ProtectedDialogueInputV1 {
            reading_source: None,
            content_id: "independent-foreground".into(),
            kind,
            source_text: text.into(),
            source_start_byte: 0,
            reply_message_id: None,
        }
    }

    fn messages() -> Vec<Message> {
        vec![
            Message {
                role: "system".into(),
                content: AUTHORED.into(),
            },
            Message {
                role: "user".into(),
                content: "Ambient source λ🌊. ".repeat(2_000),
            },
        ]
    }

    /// Exercise production request adaptation and final admission with synthetic
    /// provider bytes, so these tests perform no HTTP or live workspace access.
    fn attempt(
        fallback: bool,
        protected: Option<&ProtectedDialogueInputV1>,
    ) -> (
        SubmittedRuntimeFeedbackAttemptV1,
        Option<SubmittedDeliveryAttemptV1>,
    ) {
        let (request_bytes, protected_admission, runtime_admission) = if fallback {
            let mut request = build_ollama_protected_chat_request(
                "dialogue_live",
                messages(),
                0.7,
                512,
                "mock-fallback".into(),
                protected.is_some(),
            );
            let (protected_admission, runtime_admission) =
                admit_runtime_feedback_and_protected_content(
                    &mut request.messages,
                    protected,
                    &[feedback()],
                    16_000,
                )
                .unwrap();
            assert!(message_prompt_chars(&request.messages) <= 16_000);
            (
                serde_json::to_vec(&request).unwrap(),
                protected_admission,
                runtime_admission,
            )
        } else {
            let policy = apply_mlx_request_policy(
                "dialogue_live",
                MlxProfile::Gemma4Canary,
                messages(),
                512,
                120,
            );
            let mut messages = policy.messages;
            let (protected_admission, runtime_admission) =
                admit_runtime_feedback_and_protected_content(
                    &mut messages,
                    protected,
                    &[feedback()],
                    4_000,
                )
                .unwrap();
            assert!(message_prompt_chars(&messages) <= 4_000);
            let request = MlxRequest {
                messages,
                max_tokens: 512,
                temperature: 0.7,
                stream: false,
                aperture: None,
                model_qos_v1: None,
            };
            (
                serde_json::to_vec(&request).unwrap(),
                protected_admission,
                runtime_admission,
            )
        };
        let response = if fallback {
            serde_json::json!({"message":{"content":COMPLETION},"done":true})
        } else {
            serde_json::json!({"choices":[{"message":{"content":COMPLETION}}]})
        }
        .to_string();
        let runtime = capture_runtime_feedback_attempt(
            "mock://provider",
            "mock-model",
            &request_bytes,
            &response,
            runtime_admission,
        )
        .unwrap();
        let protected = capture_submitted_delivery(
            "mock://provider",
            "mock-model",
            &request_bytes,
            &response,
            protected_admission,
        );
        (runtime, protected)
    }

    #[test]
    fn primary_and_fallback_keep_runtime_origin_separate_from_authored_form_and_preference() {
        for fallback in [false, true] {
            let (attempt, protected) = attempt(fallback, None);
            assert!(protected.is_none());
            validate_runtime_feedback_attempt(&attempt).unwrap();
            let request: serde_json::Value = serde_json::from_str(&attempt.request_json).unwrap();
            let rendered = request["messages"][attempt.admission.message_index]["content"]
                .as_str()
                .unwrap();
            assert!(rendered.contains("\"origin\":\"runtime\""));
            assert!(rendered.contains("no_active_read_only_research_budget"));
            assert!(!rendered.contains("you chose to emphasize"));
            assert!(
                request["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|message| {
                        message["content"].as_str().is_some_and(|text| {
                            text.contains("attend to the melody") && text.contains("sonnet")
                        })
                    })
            );
            let dir = tempfile::tempdir().unwrap();
            let receipt = retain_runtime_feedback_at(dir.path(), attempt, COMPLETION).unwrap();
            verify_runtime_feedback_receipt(&receipt).unwrap();
            assert_eq!(receipt.feedback_ids, vec![feedback().id]);
        }
    }

    #[test]
    fn protected_reading_and_letter_keep_independent_identity_and_source_receipts() {
        for kind in [
            ProtectedDialogueKindV1::Reading,
            ProtectedDialogueKindV1::Letter,
        ] {
            for fallback in [false, true] {
                let source = source(kind, "Exact foreground café λ🌊. Preserve my words.");
                let (runtime, protected) = attempt(fallback, Some(&source));
                let protected = protected.unwrap();
                validate_submitted_admission(&protected).unwrap();
                assert_eq!(protected.admission.content_id, source.content_id);
                assert_eq!(
                    protected.admission.admitted_text_sha256,
                    protected_digest(&source.source_text)
                );
                let dir = tempfile::tempdir().unwrap();
                let receipt =
                    retain_runtime_feedback_at(&dir.path().join("runtime"), runtime, COMPLETION)
                        .unwrap();
                let reading_receipt =
                    retain_accepted_delivery_at(&dir.path().join("reading"), protected, COMPLETION)
                        .unwrap();
                assert_eq!(receipt.feedback_ids, vec![feedback().id]);
                assert_eq!(reading_receipt.content_id, source.content_id);
                assert_eq!(reading_receipt.admitted_end_byte, source.source_text.len());
                assert_ne!(
                    receipt.retained_artifact_path,
                    reading_receipt.retained_artifact_path
                );
            }
        }
    }

    #[test]
    fn foreground_that_only_fits_without_feedback_leaves_feedback_unadmitted() {
        let source = source(ProtectedDialogueKindV1::Letter, &"x".repeat(600));
        let mut messages = vec![Message {
            role: "system".into(),
            content: "Preserve policy.".into(),
        }];
        let (protected, runtime) = admit_runtime_feedback_and_protected_content(
            &mut messages,
            Some(&source),
            &[feedback()],
            1_200,
        )
        .unwrap();
        assert!(runtime.is_none());
        assert_eq!(protected.unwrap().admitted_end_byte, 600);
        assert!(
            !messages
                .iter()
                .any(|message| message.content.contains("runtime_action_feedback_v1"))
        );
        let mut messages = self::messages();
        assert!(
            admit_runtime_feedback_and_protected_content(
                &mut messages,
                Some(&source),
                &[feedback()],
                100
            )
            .is_none()
        );
    }

    #[test]
    fn same_words_elsewhere_or_changed_feedback_ids_cannot_prove_admission() {
        let (mut attempt, _) = attempt(false, None);
        let mut request: serde_json::Value = serde_json::from_str(&attempt.request_json).unwrap();
        let exact = request["messages"][0]["content"].clone();
        request["messages"][0]["content"] =
            serde_json::json!("Feedback removed during final adaptation.");
        request["messages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"role":"user","content":exact}));
        attempt.request_json = serde_json::to_string(&request).unwrap();
        assert!(validate_runtime_feedback_attempt(&attempt).is_err());
        assert!(
            capture_runtime_feedback_attempt(
                "mock://provider",
                "model",
                attempt.request_json.as_bytes(),
                &attempt.response_json,
                Some(attempt.admission.clone())
            )
            .is_none()
        );
        let (mut attempt, _) = self::attempt(false, None);
        attempt.admission.feedback[0].id = "different-event".into();
        assert!(validate_runtime_feedback_attempt(&attempt).is_err());
    }

    #[test]
    fn retention_failure_unfinished_or_unrelated_completion_cannot_consume_pending_feedback() {
        let (attempt, _) = attempt(false, None);
        let mut pending = vec![feedback()];
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("not-a-directory");
        std::fs::write(&blocked, b"occupied").unwrap();
        assert!(retain_runtime_feedback_at(&blocked, attempt.clone(), COMPLETION).is_err());
        assert!(
            retain_runtime_feedback_at(dir.path(), attempt.clone(), "unrelated accepted words")
                .is_err()
        );
        let mut unfinished = attempt.clone();
        unfinished.response_json =
            serde_json::json!({"message":{"content":COMPLETION},"done":false}).to_string();
        assert!(retain_runtime_feedback_at(dir.path(), unfinished, COMPLETION).is_err());
        assert_eq!(pending, vec![feedback()]);
        let receipt = retain_runtime_feedback_at(dir.path(), attempt, COMPLETION).unwrap();
        let newer = RuntimeActionFeedbackV1::from_guard_inputs(
            Some("act_fixture_read_more"),
            "READ_MORE",
            "new_state",
            "New feedback",
            None,
        );
        pending.push(newer.clone());
        verify_runtime_feedback_receipt(&receipt).unwrap();
        consume_runtime_feedback_ids(&mut pending, &receipt.feedback_ids);
        assert_eq!(pending, vec![newer]);
    }

    #[test]
    fn durable_receipts_are_private_idempotent_and_reject_tampering() {
        use std::os::unix::fs::PermissionsExt as _;
        let (attempt, _) = attempt(true, None);
        let dir = tempfile::tempdir().unwrap();
        let receipt = retain_runtime_feedback_at(dir.path(), attempt.clone(), COMPLETION).unwrap();
        assert_eq!(
            retain_runtime_feedback_at(dir.path(), attempt, COMPLETION).unwrap(),
            receipt
        );
        assert_eq!(
            std::fs::metadata(&receipt.retained_artifact_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let mut changed = receipt.clone();
        changed.feedback_ids = vec!["another-event".into()];
        assert!(verify_runtime_feedback_receipt(&changed).is_err());
        std::fs::write(&receipt.retained_artifact_path, b"changed artifact").unwrap();
        assert!(verify_runtime_feedback_receipt(&receipt).is_err());
    }

    #[test]
    fn small_fallback_budget_admits_a_complete_prefix_and_advances_only_that_prefix() {
        let mut pending: Vec<_> = (0..12)
            .map(|number| {
                RuntimeActionFeedbackV1::from_guard_inputs(
                    Some(&format!("act_{number}")),
                    "READ_MORE",
                    "blocked",
                    &"Detailed runtime result. ".repeat(20),
                    None,
                )
            })
            .collect();
        let mut request = build_ollama_protected_chat_request(
            "dialogue_live",
            vec![Message {
                role: "system".into(),
                content: "Preserve policy and form.".into(),
            }],
            0.7,
            512,
            "mock-fallback".into(),
            false,
        );
        let system_bytes: usize = request
            .messages
            .iter()
            .filter(|message| message.role == "system")
            .map(|message| message.content.len())
            .sum();
        let limit = system_bytes + render_runtime_action_feedback(&pending[..2]).unwrap().len();
        let (_, admitted) = admit_runtime_feedback_and_protected_content(
            &mut request.messages,
            None,
            &pending,
            limit,
        )
        .unwrap();
        let admitted = admitted.unwrap();
        assert_eq!(admitted.feedback.len(), 2);
        let response =
            serde_json::json!({"message":{"content":COMPLETION},"done":true}).to_string();
        let attempt = capture_runtime_feedback_attempt(
            "mock://fallback",
            "mock-model",
            &serde_json::to_vec(&request).unwrap(),
            &response,
            Some(admitted),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let receipt = retain_runtime_feedback_at(dir.path(), attempt, COMPLETION).unwrap();
        verify_runtime_feedback_receipt(&receipt).unwrap();
        let expected_next = pending[2].id.clone();
        consume_runtime_feedback_ids(&mut pending, &receipt.feedback_ids);
        assert_eq!(pending.len(), 10);
        assert_eq!(pending[0].id, expected_next);
    }

    #[test]
    fn protected_foreground_can_share_a_smaller_feedback_prefix() {
        let feedback: Vec<_> = (0..8)
            .map(|number| {
                RuntimeActionFeedbackV1::from_guard_inputs(
                    Some(&format!("act_{number}")),
                    "READ_MORE",
                    "blocked",
                    &"runtime result ".repeat(30),
                    None,
                )
            })
            .collect();
        let source = source(
            ProtectedDialogueKindV1::Letter,
            "Complete chosen source λ🌊.",
        );
        let mut messages = vec![Message {
            role: "system".into(),
            content: "Policy.".into(),
        }];
        let mut foreground_only = messages.clone();
        admit_protected_dialogue_content(&mut foreground_only, &source, 4_000).unwrap();
        let limit = message_prompt_chars(&foreground_only)
            + render_runtime_action_feedback(&feedback[..1])
                .unwrap()
                .len();
        let (protected, runtime) = admit_runtime_feedback_and_protected_content(
            &mut messages,
            Some(&source),
            &feedback,
            limit,
        )
        .unwrap();
        assert_eq!(runtime.unwrap().feedback.len(), 1);
        assert_eq!(
            protected.unwrap().admitted_end_byte,
            source.source_text.len()
        );
        assert!(message_prompt_chars(&messages) <= limit);
    }

    #[test]
    fn runtime_feedback_cannot_squeeze_a_reading_below_its_baseline_prefix() {
        let feedback: Vec<_> = (0..8)
            .map(|number| {
                RuntimeActionFeedbackV1::from_guard_inputs(
                    Some(&format!("act_{number}")),
                    "READ_MORE",
                    "blocked",
                    &"runtime result ".repeat(10),
                    None,
                )
            })
            .collect();
        let source = source(
            ProtectedDialogueKindV1::Reading,
            &"Exact source λ🌊. ".repeat(2_000),
        );
        let original = vec![Message {
            role: "system".into(),
            content: "Keep policy.".into(),
        }];
        let mut baseline_messages = original.clone();
        let baseline =
            admit_protected_dialogue_content(&mut baseline_messages, &source, 9_000).unwrap();
        let mut squeezed = original.clone();
        let candidate_feedback = admit_runtime_feedback(&mut squeezed, &feedback, 9_000).unwrap();
        assert_eq!(candidate_feedback.feedback.len(), 8);
        let candidate = admit_protected_dialogue_content(&mut squeezed, &source, 9_000).unwrap();
        assert!(candidate.admitted_end_byte < baseline.admitted_end_byte);

        let mut messages = original;
        let (admitted, runtime) = admit_runtime_feedback_and_protected_content(
            &mut messages,
            Some(&source),
            &feedback,
            9_000,
        )
        .unwrap();
        let admitted = admitted.unwrap();
        assert!(runtime.is_none());
        assert_eq!(admitted.source_start_byte, baseline.source_start_byte);
        assert_eq!(admitted.admitted_end_byte, baseline.admitted_end_byte);
        assert_eq!(admitted.admitted_text_sha256, baseline.admitted_text_sha256);
    }

    #[test]
    fn rejected_primary_cannot_reach_the_accepted_feedback_retention_branch() {
        let (attempt, _) = attempt(false, None);
        let response = MlxChatResultV1 {
            text: "...".into(),
            delivery_attempt: None,
            runtime_feedback_attempt: Some(attempt),
            qos_request_identity_sha256: "mock-qos".into(),
            request_content_anchor_sha256: "mock-anchor".into(),
            queue_wait_ms: None,
            active_generation_and_reservoir_ms: None,
        };
        let feedback_attempt = response.runtime_feedback_attempt.clone();
        let accepted = accept_primary_dialogue_attempt(response, MlxProfile::Gemma4Canary);
        let dir = tempfile::tempdir().unwrap();
        let receipt = accepted.and_then(|(text, _)| {
            feedback_attempt
                .and_then(|attempt| retain_runtime_feedback_at(dir.path(), attempt, &text).ok())
        });
        assert!(receipt.is_none());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
