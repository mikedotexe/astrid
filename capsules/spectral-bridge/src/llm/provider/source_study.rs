/// Freeform source study keeps the same immutable page on each provider lane.
pub(crate) async fn generate_source_study(
    output: &astrid_source_study::StudyOutput,
) -> DialogueCompletionV1 {
    let private = output.input_kind == astrid_source_study::InputKind::PrivateWriting;
    let label = if private { "private_writing" } else { "self_study" };
    let input = ProtectedDialogueInputV1 {
        reading_source: None,
        content_id: output.page.as_ref().map_or_else(
            || format!("source-navigation:{}", protected_digest(&output.text)),
            |page| format!("source-study:{}", page.id),
        ),
        kind: if private { ProtectedDialogueKindV1::PrivateWriting } else { ProtectedDialogueKindV1::SourceStudy },
        source_text: output.text.clone(),
        source_start_byte: 0,
        reply_message_id: None,
    };
    let root = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/accepted_deliveries");
    // OPEN receives a fresh reader sequence. Recovery is only for an interrupted
    // delivery, and must still match the complete current input, including notes.
    if output.page.is_some()
        && let Ok(Some((receipt, text))) = recover_retained_delivery(&input.content_id, 0)
        && source_study_recovery_matches(output, &receipt)
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
        label,
        messages.clone(),
        0.7,
        4096,
        480,
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
            label,
            messages,
            0.7,
            4096,
            480,
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

fn source_study_recovery_matches(
    output: &astrid_source_study::StudyOutput,
    receipt: &PromptDeliveryReceiptV1,
) -> bool {
    let Ok(raw) = std::fs::read(&receipt.retained_artifact_path) else { return false; };
    let Ok(artifact) = serde_json::from_slice::<serde_json::Value>(&raw) else { return false; };
    let Some(request) = artifact["attempt"]["request_json"].as_str() else { return false; };
    let Some(response) = artifact["attempt"]["response_json"].as_str() else { return false; };
    verify_delivery_receipt(receipt).is_ok() && output.verify_delivery(request, response).is_ok()
}

fn source_study_attempt_complete(
    output: &astrid_source_study::StudyOutput,
    attempt: Option<&SubmittedDeliveryAttemptV1>,
) -> bool {
    attempt.is_some_and(|attempt| {
        output
            .verify_delivery(&attempt.request_json, &attempt.response_json)
            .is_ok()
    })
}

#[cfg(test)]
mod source_study_delivery_tests {
    use super::*;
    #[test]
    fn recent_study_context_fits_primary_and_fallback_without_trimming() {
        let text = format!("Source and complete earlier conclusion: {} END_OF_ANSWER", "evidence ".repeat(2000));
        let input = ProtectedDialogueInputV1 {
            reading_source: None, content_id: "study-context-fixture".into(),
            kind: ProtectedDialogueKindV1::SourceStudy, source_text: text.clone(),
            source_start_byte: 0, reply_message_id: None,
        };
        let messages = vec![Message { role: "system".into(), content: astrid_source_study::STUDY_PROMPT.into() }];
        let mut primary = messages.clone();
        assert!(admit_protected_dialogue_content(&mut primary.clone(), &input, 16_000).is_none());
        let accepted = admit_protected_dialogue_content(&mut primary, &input, astrid_source_study::MAX_INPUT_BYTES).unwrap();
        assert_eq!(accepted.admitted_end_byte, text.len());
        let mut fallback = build_ollama_protected_chat_request("self_study", messages, 0.7, 4096, "fixture".into(), true);
        assert_eq!(fallback.options.num_ctx, astrid_source_study::CONTEXT_TOKENS);
        let accepted = admit_protected_dialogue_content(&mut fallback.messages, &input, astrid_source_study::MAX_INPUT_BYTES).unwrap();
        assert_eq!(accepted.admitted_end_byte, text.len());
        assert_eq!(fallback.options.num_predict, 4096);
        assert!(fallback.messages.iter().any(|m| m.content.contains("END_OF_ANSWER")));
        let ordinary = build_ollama_protected_chat_request("dialogue_live", Vec::new(), 0.7, 4096, "fixture".into(), true);
        assert_eq!(ordinary.options.num_ctx, 10240);
    }

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
            reading_source: None,
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
        let complete_response = std::mem::replace(&mut attempt.response_json,
            serde_json::json!({"choices":[{"message":{"content":"partial"},"finish_reason":"length"}]}).to_string());
        assert!(!source_study_attempt_complete(&output, Some(&attempt)));
        attempt.response_json = complete_response;
        let receipt = retain_accepted_delivery_at(
            &temp.path().join("retained"), attempt, "NEXT: SELF_STUDY CONTINUE",
        ).unwrap();
        assert!(source_study_recovery_matches(&output, &receipt));
        let mut changed = output.clone();
        changed.text.push_str("\nSTUDY_QUESTION: What changed in this request?");
        assert!(!source_study_recovery_matches(&changed, &receipt));
        let reread = reader.prepare(astrid_source_study::Command::Open {
            source: "astrid/Cargo.toml".into(), line: 1,
        }).unwrap();
        assert_ne!(reread.page.as_ref().unwrap().id, output.page.as_ref().unwrap().id);
        assert!(!source_study_recovery_matches(&reread, &receipt));
    }
}
