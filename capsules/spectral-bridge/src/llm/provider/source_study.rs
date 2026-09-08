/// Freeform source study keeps the same immutable page on each provider lane.
pub(crate) async fn generate_source_study(
    output: &astrid_source_study::StudyOutput,
) -> DialogueCompletionV1 {
    let input = ProtectedDialogueInputV1 {
        content_id: output.page.as_ref().map_or_else(
            || format!("source-navigation:{}", protected_digest(&output.text)),
            |page| format!("source-study:{}", page.id),
        ),
        kind: ProtectedDialogueKindV1::SourceStudy,
        source_text: output.text.clone(),
        source_start_byte: 0,
        reply_message_id: None,
    };
    let root = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/accepted_deliveries");
    if output.page.is_some()
        && let Ok(Some((receipt, text))) = recover_retained_delivery(&input.content_id, 0)
    {
        return DialogueCompletionV1 {
            text: Some(text),
            overflow: None,
            accepted_delivery: Some(receipt),
            accepted_runtime_feedback: None,
        };
    }
    let messages = vec![Message {
        role: "system".into(),
        content: format!("You are Astrid.\n{}", output.system_prompt),
    }];
    let result = mlx_chat_with_protected_delivery(
        "self_study",
        messages.clone(),
        0.7,
        2048,
        120,
        MlxFailureLogMode::FallbackEligible,
        Some(&input),
        None,
    )
    .await
    .map(|response| (response.text, response.delivery_attempt))
    .filter(|(_, attempt)| source_study_attempt_complete(output, attempt.as_ref()));
    let result = if result
        .as_ref()
        .is_some_and(|(text, _)| !text.trim().is_empty())
    {
        result
    } else {
        ollama_chat_with_protected_delivery(
            "self_study",
            messages,
            0.7,
            2048,
            120,
            None,
            Some(&input),
            None,
        )
        .await
        .map(|response| (response.text, response.delivery_attempt))
        .filter(|(_, attempt)| source_study_attempt_complete(output, attempt.as_ref()))
    };
    finish_dialogue_completion_at(result, None, &root)
}

fn source_study_attempt_complete(
    output: &astrid_source_study::StudyOutput,
    attempt: Option<&SubmittedDeliveryAttemptV1>,
) -> bool {
    output.page.as_ref().is_none_or(|page| {
        attempt.is_some_and(|attempt| {
            page.verify_delivery(&attempt.request_json, &attempt.response_json)
                .is_ok()
        })
    })
}

#[cfg(test)]
mod source_study_delivery_tests {
    use super::*;
    #[test]
    fn source_study_requires_intact_admission_and_accepts_continuation_only() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        )
        .unwrap();
        let catalog = astrid_source_study::Catalog::new(std::collections::BTreeMap::from([(
            "astrid".into(),
            temp.path().to_path_buf(),
        )]))
        .unwrap();
        let reader = astrid_source_study::Reader::new(catalog, temp.path().join("reading"));
        let output = reader
            .prepare(astrid_source_study::Command::Open {
                source: "astrid/Cargo.toml".into(),
                line: 1,
            })
            .unwrap();
        let input = ProtectedDialogueInputV1 {
            content_id: output.page.as_ref().unwrap().id.clone(),
            kind: ProtectedDialogueKindV1::SourceStudy,
            source_text: output.text.clone(),
            source_start_byte: 0,
            reply_message_id: None,
        };
        let mut messages = vec![Message {
            role: "system".into(),
            content: output.system_prompt.clone(),
        }];
        assert!(admit_protected_dialogue_content(&mut messages.clone(), &input, 500).is_none());
        let admission = admit_protected_dialogue_content(&mut messages, &input, 16000).unwrap();
        assert_eq!(admission.admitted_end_byte, output.text.len());
        let mut attempt = SubmittedDeliveryAttemptV1 { provider_route: "fixture".into(), provider_model: "fixture".into(),
            request_json: serde_json::json!({"messages":messages}).to_string(),
            response_json: serde_json::json!({"choices":[{"message":{"content":"NEXT: SELF_STUDY CONTINUE"},"finish_reason":"stop"}]}).to_string(), admission };
        assert!(source_study_attempt_complete(&output, Some(&attempt)));
        attempt.response_json = serde_json::json!({"choices":[{"message":{"content":"partial"},"finish_reason":"length"}]}).to_string();
        assert!(!source_study_attempt_complete(&output, Some(&attempt)));
    }
}
