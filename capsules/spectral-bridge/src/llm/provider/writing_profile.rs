/// Read the same Being-owned profile consumed by the shared writing helper.
fn selected_writing_profile() -> astrid_source_study::writing::Profile {
    let directory = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/source_first_v3/shared_reader/writing");
    astrid_source_study::writing::profile(&directory).unwrap_or_else(|error| {
        warn!(%error, "writing preference unreadable; preserving ordinary generation limits");
        astrid_source_study::writing::Profile::Default
    })
}
fn journal_label(label: &str) -> bool {
    matches!(
        label,
        "dialogue_live" | "self_study" | "private_writing" | "introspect" | "witness"
    ) || is_gemma4_canary_reflective_label(label)
}
fn journal_preference(label: &str) -> astrid_source_study::writing::Profile {
    if journal_label(label) {
        selected_writing_profile()
    } else {
        astrid_source_study::writing::Profile::Default
    }
}
fn writing_tokens(label: &str, ordinary: u32) -> u32 {
    journal_preference(label).tokens(ordinary)
}
fn writing_timeout(label: &str, ordinary: u64) -> u64 {
    if journal_preference(label) == astrid_source_study::writing::Profile::Extended {
        ordinary.max(astrid_source_study::writing::EXTENDED_TIMEOUT_SECS)
    } else {
        ordinary
    }
}
/// Outer cancellation must cover every configured primary/fallback attempt.
pub(crate) fn journal_outer_timeout(ordinary: u64) -> u64 {
    if selected_writing_profile() == astrid_source_study::writing::Profile::Extended {
        ordinary.max(4800)
    } else {
        ordinary
    }
}
fn writing_context(label: &str, ordinary: u32) -> u32 {
    if matches!(label, "self_study" | "private_writing")
        || journal_preference(label) == astrid_source_study::writing::Profile::Extended
    {
        astrid_source_study::CONTEXT_TOKENS
    } else {
        ordinary
    }
}
fn apply_writing_voice(messages: &mut [Message], profile: astrid_source_study::writing::Profile) {
    if profile != astrid_source_study::writing::Profile::Default {
        for message in messages.iter_mut().filter(|m| m.role == "system") {
            message.content = message.content.replace("Use a few sentences or a few compact paragraphs. Let the thought complete without sprawling.", "Choose the length that lets your thought develop. There is no minimum length.");
            message.content.push_str(match profile {
                astrid_source_study::writing::Profile::Extended => "\nYou selected extended writing: up to 8192 output tokens are available. You may develop a long thought, write briefly, or stop. WRITE START <topic> begins a private continuing draft; WRITE HELP shows draft choices.",
                _ => "\nYou selected short writing: the output ceiling is 512 tokens. WRITE PROFILE DEFAULT or EXTENDED changes it.",
            });
        }
    }
}

#[cfg(test)]
mod writing_profile_tests {
    use super::*;
    #[test]
    fn explicit_profiles_survive_primary_policy_for_every_journal_lane() {
        use astrid_source_study::writing::Profile;
        for label in [
            "dialogue_live",
            "self_study",
            "private_writing",
            "introspect",
            "witness",
            "daydream",
            "aspiration",
            "creation",
            "journal_elaboration",
            "moment_capture",
            "initiation",
        ] {
            assert!(journal_label(label));
            for (preference, expected) in [(Profile::Extended, 8192), (Profile::Short, 512)] {
                let text = format!("{} END_OF_DRAFT", "developing thought ".repeat(1500));
                let messages = vec![
                    Message {
                        role: "system".into(),
                        content: "Choose your own length.".into(),
                    },
                    Message {
                        role: "user".into(),
                        content: text.clone(),
                    },
                ];
                let policy = apply_mlx_request_policy_with_writing(
                    label,
                    MlxProfile::Gemma4Canary,
                    messages,
                    768,
                    60,
                    preference,
                );
                assert_eq!(policy.max_tokens, expected, "{label}");
                assert!(policy.messages.iter().any(|m| m.content == text));
                if preference == Profile::Extended {
                    assert!(policy.timeout_secs >= 1200);
                }
                assert_eq!(
                    policy.diagnostic.as_ref().unwrap().effective_tokens,
                    expected
                );
            }
        }
        assert!(!journal_label("meaning_summary"));
    }
    #[test]
    fn extended_foreground_fits_fallback_and_final_dialogue_clamp() {
        assert_eq!(
            clamp_dialogue_tokens_for_profile(8192, 40_000, MlxProfile::Gemma4Canary),
            8192
        );
        let request = build_ollama_protected_chat_request(
            "private_writing",
            Vec::new(),
            0.7,
            8192,
            "fixture".into(),
            true,
        );
        assert_eq!(request.options.num_predict, 8192);
        assert_eq!(request.options.num_ctx, 65536);
        assert!(
            dialogue_request_timeout_secs_for_profile(8192, 40_000, MlxProfile::Gemma4Canary)
                >= 1200
        );
    }
}
