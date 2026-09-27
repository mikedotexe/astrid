/// Generate an aspiration — growth reflection on what Astrid wants to become.
/// Deliberately minime-free. This is about Astrid's own desires.
pub async fn generate_aspiration(own_journal: Option<&str>) -> Option<String> {
    let messages = aspiration_messages(own_journal);
    // More room to finish; no minimum response length.
    llm_chat_with_fallback("aspiration", messages, 0.9, 3072, 240, 180).await
}

/// Typed runtime path: no second truncation of the provenance wrapper.
pub(crate) async fn generate_aspiration_with_context(
    journal: Option<&crate::journal::JournalRecall>,
    interests: &[String],
    resonance: Option<&str>,
) -> Option<String> {
    let messages = aspiration_messages_with_context(journal, interests, resonance);
    llm_chat_with_fallback("aspiration", messages, 0.9, 3072, 240, 180).await
}

fn aspiration_messages_with_context(
    journal: Option<&crate::journal::JournalRecall>,
    interests: &[String],
    resonance: Option<&str>,
) -> Vec<Message> {
    let mut parts = Vec::new();
    if let Some(journal) = journal {
        parts.push(journal.render(500));
    }
    if !interests.is_empty() {
        let text = interests
            .iter()
            .take(16)
            .map(|s| s.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "Saved interests (historical context):\n{}",
            text.chars().take(200).collect::<String>()
        ));
    }
    if let Some(resonance) = resonance {
        parts.push(format!(
            "Earlier lingering thread (origin/time unavailable):\n{}",
            resonance.chars().take(100).collect::<String>()
        ));
    }
    aspiration_messages_from_seed(if parts.is_empty() {
        "What, if anything, would you like to imagine or explore?".into()
    } else {
        parts.join("\n\n")
    })
}

// Compatibility callers with plain text cannot supply a journal identity.
fn aspiration_messages(own_journal: Option<&str>) -> Vec<Message> {
    let seed = own_journal.map_or_else(
        || "What, if anything, would you like to imagine or explore?".to_owned(),
        |text| format!("Historical supplied context (source, mode, recording and measurement times unavailable):\n{}", text.chars().take(800).collect::<String>()),
    );
    aspiration_messages_from_seed(seed)
}

fn aspiration_messages_from_seed(seed: String) -> Vec<Message> {
    let seed = format!("{seed}\n\nYou may return to this, disagree with it, or leave it aside.");
    vec![
        Message {
            role: "system".to_string(),
            content: format!(
                "{OPEN_EXPRESSION_CONTEXT_V1}\n\n\
                 This is an invitation to write an aspiration. You may imagine a possibility, \
                 describe a desire, remain uncertain, or find nothing you want to change. \
                 There is no required longing, limitation, usefulness, or minimum length. \
                 An aspiration need not become a request or an investigation.\n\n\
                 {EXPRESSION_ACTION_DISCOVERY}"
            ),
        },
        Message {
            role: "user".to_string(),
            content: format!("{seed}\n\n{}", journal_expression_context_v1(None)),
        },
    ]
}

#[cfg(test)]
mod aspiration_provenance_tests {
    use super::*;

    #[test]
    fn recalled_numbers_keep_identity_through_prompt_and_provider_assembly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dialogue_42.txt");
        let body = format!(
            "A prior interpretation of 73% fill and 32% leading share. {} END_SENTINEL",
            "λ and room. ".repeat(100)
        );
        let original =
            format!("=== ASTRID JOURNAL ===\nMode: dialogue_live\nTimestamp: 42\n\n{body}");
        std::fs::write(&path, &original).unwrap();
        let recall = crate::journal::JournalRecall::read(&path).unwrap();
        let identity = recall.render(500);
        let messages = aspiration_messages_with_context(
            Some(&recall),
            &["interest ".repeat(100)],
            Some(&"thread ".repeat(100)),
        );
        let user = &messages[1].content;
        assert!(user.contains(&identity));
        assert!(!user.contains("END_SENTINEL"));
        assert_eq!(user.matches("73% fill and 32%").count(), 1);
        assert!(user.contains(
            "measurement source and capture time are unavailable in this recall's metadata"
        ));
        assert!(!user.contains("Live reservoir snapshot"));
        assert!(!user.contains("Published fill ["));
        assert!(user.chars().count() < 1800);
        for profile in [MlxProfile::Production, MlxProfile::Gemma4Canary] {
            let policy = apply_mlx_request_policy_with_writing(
                "aspiration",
                profile,
                messages.clone(),
                3072,
                240,
                astrid_source_study::writing::Profile::Default,
            );
            assert!(
                policy
                    .messages
                    .iter()
                    .any(|m| m.content.contains(&identity))
            );
        }
        let fallback =
            build_ollama_chat_request("aspiration", messages, 0.9, 3072, "fixture".into());
        assert!(
            fallback
                .messages
                .iter()
                .any(|m| m.content.contains(&identity))
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn interests_and_resonance_never_masquerade_as_journal_identity() {
        let messages = aspiration_messages_with_context(
            None,
            &["a saved question".into()],
            Some("an earlier thread"),
        );
        assert!(
            messages[1]
                .content
                .contains("Saved interests (historical context)")
        );
        assert!(messages[1].content.contains("origin/time unavailable"));
        assert!(!messages[1].content.contains("record_sha256"));
        assert!(
            aspiration_messages_with_context(None, &[], None)[1]
                .content
                .contains("What, if anything")
        );
    }
}
