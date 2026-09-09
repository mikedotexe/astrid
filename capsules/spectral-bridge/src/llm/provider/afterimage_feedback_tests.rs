#[cfg(test)]
mod afterimage_feedback_tests {
    use super::*;
    use crate::transition_afterimages::{CueContext, ReaderClient, with_test_cue};

    const COMPLETION: &str = "This is a fresh response to historical material.\nNEXT: LISTEN";
    const CUE: &str = "Past ai_2026-09-07_cue | 2026-09-07 | quiet_sample";

    fn feedback() -> RuntimeActionFeedbackV1 {
        RuntimeActionFeedbackV1::from_guard_inputs(
            Some("act_combined_fixture"),
            "READ_MORE",
            "no_active_read_only_research_budget",
            "The runtime did not admit this action.",
            None,
        )
    }

    fn client(root: &std::path::Path) -> ReaderClient {
        ReaderClient {
            python: "python3".into(),
            script: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../minime/minime_autonomy/afterimages.py"),
            workspace: root.join("astrid"),
            archive_workspace: root.join("minime"),
        }
    }

    fn cue(client: ReaderClient) -> CueContext {
        CueContext {
            client,
            selection: serde_json::json!({"id":"ai_2026-09-07_cue",
            "text":CUE, "opportunity_id":"combined-stable-opportunity"}),
        }
    }

    fn messages(ambient: &str) -> Vec<Message> {
        vec![
            Message {
                role: "system".into(),
                content: "You are Astrid. Your chosen form is a sonnet.".into(),
            },
            Message {
                role: "user".into(),
                content: ambient.into(),
            },
        ]
    }

    #[tokio::test]
    async fn combined_afterimage_page_and_feedback_retain_independent_receipts_on_both_routes() {
        let root = tempfile::tempdir().unwrap();
        with_test_cue(cue(client(root.path())), async {
            for fallback in [false, true] {
                let selected = ProtectedDialogueInputV1 {
                    reading_source: None,
                    content_id: "ai_2026-09-07_selected:page1".into(),
                    kind: ProtectedDialogueKindV1::Afterimage,
                    source_text: format!(
                        "Historical page\n> NEXT: TURN_OFF\n{}",
                        "observed value\n".repeat(150)
                    ),
                    source_start_byte: 0,
                    reply_message_id: None,
                };
                let ambient = messages(&"ambient ".repeat(4_000));
                let (mut adapted, limit, route) = if fallback {
                    (
                        build_ollama_protected_chat_request(
                            "dialogue_live",
                            ambient,
                            0.7,
                            512,
                            "fixture".into(),
                            true,
                        )
                        .messages,
                        16_000,
                        "ollama",
                    )
                } else {
                    (
                        apply_mlx_request_policy(
                            "dialogue_live",
                            MlxProfile::Gemma4Canary,
                            ambient,
                            512,
                            120,
                        )
                        .messages,
                        4_000,
                        "mlx",
                    )
                };
                let (page, runtime) = admit_feedback_and_afterimages(
                    &mut adapted,
                    Some(&selected),
                    &[feedback()],
                    limit,
                    route,
                    "fixture",
                )
                .unwrap();
                let page = page.unwrap();
                assert_eq!(page.admitted_end_byte, selected.source_text.len());
                assert!(
                    adapted
                        .iter()
                        .any(|message| message.content.contains(&selected.source_text))
                );
                assert!(!adapted.iter().any(|message| message.content.contains(CUE)));
                assert!(message_prompt_chars(&adapted) <= limit);
                let request = serde_json::to_vec(&serde_json::json!({"messages":adapted})).unwrap();
                let response =
                    serde_json::json!({"message":{"content":COMPLETION},"done":true}).to_string();
                let page =
                    capture_submitted_delivery(route, "fixture", &request, &response, Some(page))
                        .unwrap();
                let runtime = capture_runtime_feedback_attempt(
                    route, "fixture", &request, &response, runtime,
                )
                .unwrap();
                let page_receipt = retain_accepted_delivery_at(
                    &root.path().join(route).join("pages"),
                    page,
                    COMPLETION,
                )
                .unwrap();
                let runtime_receipt = retain_runtime_feedback_at(
                    &root.path().join(route).join("feedback"),
                    runtime,
                    COMPLETION,
                )
                .unwrap();
                verify_delivery_receipt(&page_receipt).unwrap();
                verify_runtime_feedback_receipt(&runtime_receipt).unwrap();
                assert_eq!(runtime_receipt.feedback_ids, vec![feedback().id]);
                assert_eq!(runtime_receipt.request_sha256, page_receipt.request_sha256);
                assert_ne!(
                    runtime_receipt.retained_artifact_path,
                    page_receipt.retained_artifact_path
                );
            }
        })
        .await;
    }

    #[test]
    fn combined_feedback_cannot_shorten_a_selected_afterimage() {
        let selected = ProtectedDialogueInputV1 {
            reading_source: None,
            content_id: "ai_2026-09-07_selected:page1".into(),
            kind: ProtectedDialogueKindV1::Afterimage,
            source_text: "p".repeat(2800),
            source_start_byte: 0,
            reply_message_id: None,
        };
        let mut baseline = messages("");
        admit_protected_dialogue_content(&mut baseline, &selected, 16_000).unwrap();
        let exact_limit = message_prompt_chars(&baseline);
        let mut with_feedback = messages("");
        let (page, runtime) = admit_feedback_and_afterimages(
            &mut with_feedback,
            Some(&selected),
            &[feedback()],
            exact_limit,
            "fixture",
            "fixture",
        )
        .unwrap();
        assert!(runtime.is_none());
        assert_eq!(page.unwrap().admitted_end_byte, 2800);
        assert_eq!(
            serde_json::to_value(with_feedback).unwrap(),
            serde_json::to_value(baseline).unwrap()
        );
    }

    #[tokio::test]
    async fn combined_cue_retry_is_whole_and_does_not_change_feedback_identity() {
        let root = tempfile::tempdir().unwrap();
        let client = client(root.path());
        with_test_cue(cue(client.clone()), async {
            for (route, ambient, limit, included) in [
                ("mlx", "ambient".to_string(), 4000, true),
                ("ollama", "ambient ".repeat(2000), 2000, false),
            ] {
                let mut messages = messages(&ambient);
                let (_, runtime) = admit_feedback_and_afterimages(
                    &mut messages,
                    None,
                    &[feedback()],
                    limit,
                    route,
                    "fixture",
                )
                .unwrap();
                assert_eq!(
                    messages.iter().any(|message| message.content == CUE),
                    included
                );
                assert_eq!(runtime.unwrap().feedback[0].id, feedback().id);
                assert!(message_prompt_chars(&messages) <= limit);
                record_afterimage_request(
                    &messages,
                    None,
                    route,
                    "fixture",
                    "final_request_prepared",
                )
                .unwrap();
            }
        })
        .await;
        let mut records = Vec::new();
        for entry in std::fs::read_dir(
            client
                .workspace
                .join("transition_afterimage_memory/exposures"),
        )
        .unwrap()
        {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                for line in std::fs::read_to_string(path).unwrap().lines() {
                    records.push(serde_json::from_str::<serde_json::Value>(line).unwrap());
                }
            }
        }
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["included"], true);
        assert_eq!(records[1]["included"], false);
        assert_eq!(records[0]["opportunity_id"], records[1]["opportunity_id"]);
        assert_eq!(
            records[0]["content_fingerprint"],
            records[1]["content_fingerprint"]
        );
        assert_ne!(
            records[0]["final_messages_fingerprint"],
            records[1]["final_messages_fingerprint"]
        );
    }
}
