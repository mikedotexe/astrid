/// Runtime daydream path: keep historical identity outside the excerpt budget.
pub(crate) async fn generate_daydream_with_context(
    perception: Option<&str>,
    journal: Option<&crate::journal::JournalRecall>,
    interests: &[String],
    memories: &[(String, String)],
    resonance: Option<&str>,
) -> Option<String> {
    let messages = daydream_messages_with_context(perception, journal, interests, memories, resonance);
    llm_chat_with_fallback("daydream", messages, 1.0, 3072, 240, 180).await
}

fn daydream_messages_with_context(
    perception: Option<&str>,
    journal: Option<&crate::journal::JournalRecall>,
    interests: &[String],
    memories: &[(String, String)],
    resonance: Option<&str>,
) -> Vec<Message> {
    let mut historical = Vec::new();
    if let Some(perception) = perception {
        historical.push(format!("Supplied sensory context or labelled operational notice (optional):\n{}", perception.chars().take(800).collect::<String>()));
    }
    if let Some(journal) = journal {
        historical.push(journal.render(500));
    }
    if !interests.is_empty() {
        let text = interests.iter().take(8).map(|s| s.chars().take(150).collect::<String>())
            .collect::<Vec<_>>().join("\n");
        historical.push(format!("Saved interests (authored historical context, not obligations):\n{text}"));
    }
    for (source, text) in memories.iter().take(2) {
        historical.push(format!(
            "Explicitly starred memory (historical, not a current measurement; recording/capture times unavailable): {}\n{}",
            source.chars().take(200).collect::<String>(), text.chars().take(400).collect::<String>()
        ));
    }
    if let Some(resonance) = resonance {
        historical.push(format!("Earlier lingering thread (origin/time unavailable):\n{}", resonance.chars().take(200).collect::<String>()));
    }
    let context = if historical.is_empty() {
        "No perception or journal excerpt is supplied for this invitation.".into()
    } else {
        format!("Optional context; return to it or leave it aside. It does not require a numerical account.\n{}", historical.join("\n\n"))
    };
    daydream_messages_from_context(context)
}

#[cfg(test)]
mod daydream_context_tests {
    use super::*;

    #[test]
    fn chosen_history_keeps_provenance_numbers_and_full_wrapper() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daydream_42.txt");
        std::fs::write(&path, format!("=== ASTRID JOURNAL ===\nMode: daydream\nTimestamp: 42\n\n{}", "I chose to consider 71% and uncertainty. ".repeat(40))).unwrap();
        let journal = crate::journal::JournalRecall::read(&path).unwrap();
        let rendered = journal.render(500);
        let messages = daydream_messages_with_context(Some("Fresh selected view"), Some(&journal), &["A question".into()], &[("chosen source".into(), "Recorded 65% matters to me".into())], Some("Earlier thread"));
        let input = &messages[1].content;
        assert!(input.contains(&rendered));
        assert!(input.contains("Recorded 65% matters to me"));
        assert!(input.contains("Explicitly starred memory"));
        assert!(input.contains("origin/time unavailable"));
        assert!(!input.contains("Published fill ["));
        for profile in [MlxProfile::Production, MlxProfile::Gemma4Canary] {
            let policy = apply_mlx_request_policy_with_writing("daydream", profile, messages.clone(), 3072, 240, astrid_source_study::writing::Profile::Default);
            assert!(policy.messages.iter().any(|m| m.content.contains(&rendered)));
        }
        let fallback = build_ollama_chat_request("daydream", messages, 1.0, 3072, "fixture".into());
        assert!(fallback.messages.iter().any(|m| m.content.contains(&rendered)));
    }
}
