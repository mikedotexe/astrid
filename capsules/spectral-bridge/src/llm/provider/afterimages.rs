/// Foreground source wins over feedback; optional cues use only remaining space.
fn admit_feedback_and_afterimages(
    messages: &mut Vec<Message>,
    protected: Option<&ProtectedDialogueInputV1>,
    feedback: &[RuntimeActionFeedbackV1],
    limit: usize,
    backend: &str,
    model: &str,
) -> Option<(
    Option<ProtectedAdmissionV1>,
    Option<RuntimeFeedbackAdmissionV1>,
)> {
    let admission =
        admit_runtime_feedback_and_protected_content(messages, protected, feedback, limit);
    if admission.is_none() {
        let _ = record_afterimage_request(messages, protected, backend, model, "admission_failed");
        return None;
    }
    prepend_afterimage_cue(messages, limit, protected.is_some());
    admission
}

fn prepend_afterimage_cue(messages: &mut [Message], limit: usize, protected: bool) {
    if protected {
        return;
    }
    if let Some(context) = crate::transition_afterimages::active_cue()
        && let Some(text) = context.selection["text"].as_str()
    {
        let prefix = format!(
            "Optional historical context (reference only, not a new request). \
             Revisiting it is optional.\n\
             {text}\nEnd optional memory.\n\n"
        );
        if message_prompt_chars(messages).saturating_add(prefix.len()) <= limit
            && let Some(foreground) = messages
                .iter_mut()
                .rev()
                .find(|message| message.role == "user")
            && !foreground.content.trim().is_empty()
        {
            foreground.content.insert_str(0, &prefix);
        }
    }
}

fn record_afterimage_request(
    messages: &[Message],
    protected: Option<&ProtectedDialogueInputV1>,
    backend: &str,
    model: &str,
    outcome: &str,
) -> Option<()> {
    let (client, selection) = if let Some(input) =
        protected.filter(|input| input.kind == ProtectedDialogueKindV1::Afterimage)
    {
        let client = crate::transition_afterimages::ReaderClient::configured();
        let selection = client
            .pending()
            .ok()
            .flatten()
            .filter(|selection| selection["content_id"] == input.content_id)?;
        (client, selection)
    } else {
        let Some(context) = crate::transition_afterimages::active_cue() else {
            return Some(());
        };
        (context.client, context.selection)
    };
    crate::transition_afterimages::record_exposure(
        &client,
        selection,
        serde_json::to_value(messages).unwrap_or_default(),
        backend,
        model,
        outcome,
    )
    .map_err(|error| warn!(%error, "afterimage receipt unavailable; provider attempt withheld"))
    .ok()?;
    Some(())
}

#[cfg(test)]
mod afterimage_provider_tests {
    use super::*;

    #[tokio::test]
    async fn optional_cue_preserves_foreground_roles_and_whole_block_budget() {
        use crate::transition_afterimages::{CueContext, ReaderClient, with_test_cue};
        let root = tempfile::tempdir().unwrap();
        let context = CueContext {
            client: ReaderClient {
                python: "python3".into(),
                script: "unused".into(),
                workspace: root.path().join("astrid"),
                archive_workspace: root.path().join("minime"),
            },
            selection: serde_json::json!({"text":"Past sample | recorded history"}),
        };
        with_test_cue(context, async {
            let original = vec![
                Message {
                    role: "system".into(),
                    content: "Chosen form: dialogue".into(),
                },
                Message {
                    role: "user".into(),
                    content: "Earlier question".into(),
                },
                Message {
                    role: "assistant".into(),
                    content: "Earlier response".into(),
                },
                Message {
                    role: "user".into(),
                    content: "Develop my chosen passage.\nNEXT remains voluntary.".into(),
                },
            ];
            let mut admitted = original.clone();
            prepend_afterimage_cue(&mut admitted, 1000, false);
            assert_eq!(admitted.len(), original.len());
            for (before, after) in original.iter().zip(&admitted).take(3) {
                assert_eq!(before.role, after.role);
                assert_eq!(before.content, after.content);
            }
            let final_message = admitted.last().unwrap();
            assert!(
                final_message
                    .content
                    .starts_with("Optional historical context")
            );
            assert!(
                final_message
                    .content
                    .ends_with(&original.last().unwrap().content)
            );
            let exact_limit = message_prompt_chars(&admitted);
            let mut exact = original.clone();
            prepend_afterimage_cue(&mut exact, exact_limit, false);
            assert_eq!(message_prompt_chars(&exact), exact_limit);
            for (limit, protected) in [(exact_limit.saturating_sub(1), false), (1000, true)] {
                let mut omitted = original.clone();
                prepend_afterimage_cue(&mut omitted, limit, protected);
                assert_eq!(
                    serde_json::to_value(omitted).unwrap(),
                    serde_json::to_value(&original).unwrap()
                );
            }
            for role in ["system", "assistant", "user"] {
                let mut empty = vec![Message {
                    role: role.into(),
                    content: String::new(),
                }];
                prepend_afterimage_cue(&mut empty, 1000, false);
                assert!(empty[0].content.is_empty());
            }
        })
        .await;
    }

    #[tokio::test]
    async fn transition_afterimage_cue_fallback_is_whole_and_each_attempt_receipted() {
        use crate::transition_afterimages::{CueContext, ReaderClient, with_test_cue};
        let root = tempfile::tempdir().unwrap();
        let client = ReaderClient {
            python: "python3".into(),
            script: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../minime/minime_autonomy/afterimages.py"),
            workspace: root.path().join("astrid"),
            archive_workspace: root.path().join("minime"),
        };
        let text = "Past ai_2026-09-07_test | 2026-09-07 | quiet_sample";
        let context = CueContext {
            client: client.clone(),
            selection: serde_json::json!({"id":"ai_2026-09-07_test", "text":text, "opportunity_id":"native_retry_fixture"}),
        };
        with_test_cue(context, async {
            let mut primary = vec![Message {
                role: "user".into(),
                content: "ambient".into(),
            }];
            prepend_afterimage_cue(&mut primary, 400, false);
            assert!(primary.last().unwrap().content.contains(text));
            assert!(primary.last().unwrap().content.ends_with("ambient"));
            assert_eq!(primary.len(), 1);
            assert!(
                record_afterimage_request(
                    &primary,
                    None,
                    "mlx",
                    "fixture",
                    "final_request_prepared"
                )
                .is_some()
            );
            let mut fallback = vec![Message {
                role: "user".into(),
                content: "ambient".into(),
            }];
            prepend_afterimage_cue(&mut fallback, 8, false);
            assert_eq!(fallback.len(), 1);
            assert!(
                record_afterimage_request(
                    &fallback,
                    None,
                    "ollama",
                    "fixture",
                    "final_request_prepared"
                )
                .is_some()
            );
        })
        .await;
        let paths = std::fs::read_dir(
            client
                .workspace
                .join("transition_afterimage_memory/exposures"),
        )
        .unwrap();
        let mut records = Vec::new();
        for entry in paths {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                for line in std::fs::read_to_string(path).unwrap().lines() {
                    records.push(serde_json::from_str::<serde_json::Value>(line).unwrap());
                }
            }
        }
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["included"], true);
        assert_eq!(records[1]["included"], false);
        assert_eq!(records[0]["opportunity_id"], records[1]["opportunity_id"]);
    }
}
