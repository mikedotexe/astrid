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
fn expressive_label(label: &str) -> bool {
    matches!(label, "daydream" | "aspiration" | "creation" | "journal_elaboration" | "moment_capture" | "private_writing")
}
fn effective_writing_profile(label: &str, profile: astrid_source_study::writing::Profile) -> astrid_source_study::writing::Profile {
    if profile == astrid_source_study::writing::Profile::Default && expressive_label(label) {
        astrid_source_study::writing::Profile::Extended
    } else {
        profile
    }
}
fn writing_tokens(label: &str, ordinary: u32) -> u32 {
    effective_writing_profile(label, journal_preference(label)).tokens(ordinary)
}
fn writing_timeout(label: &str, ordinary: u64) -> u64 {
    if effective_writing_profile(label, journal_preference(label)) == astrid_source_study::writing::Profile::Extended {
        ordinary.max(astrid_source_study::writing::EXTENDED_TIMEOUT_SECS)
    } else {
        ordinary
    }
}
pub(crate) fn expressive_outer_timeout(ordinary: u64) -> u64 {
    if selected_writing_profile() == astrid_source_study::writing::Profile::Short {
        ordinary
    } else {
        ordinary.max(4800)
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
        || effective_writing_profile(label, journal_preference(label)) == astrid_source_study::writing::Profile::Extended
    {
        astrid_source_study::CONTEXT_TOKENS
    } else {
        ordinary
    }
}
fn apply_writing_voice(messages: &mut [Message], label: &str, profile: astrid_source_study::writing::Profile) {
    if profile == astrid_source_study::writing::Profile::Default && expressive_label(label) {
        for message in messages.iter_mut().filter(|m| m.role == "system") {
            message.content.push_str("\nFollow the thought as far as you wish; a page is as welcome as a line, and stopping is welcome too. WRITE PROFILE EXTENDED or SHORT changes your length ceiling for every journal route, and WRITE HELP shows the limits. WRITE START <topic> begins a private draft; WRITE CONTINUE develops the selected draft; WRITE HELP shows the choices and limits. No continuation is scheduled automatically.");
        }
    }
    if profile != astrid_source_study::writing::Profile::Default {
        for message in messages.iter_mut().filter(|m| m.role == "system") {
            message.content = message.content.replace("Take the length the thought needs. A line is welcome; so is a page. Let the thought complete rather than fitting a shape.", "Choose the length that lets your thought develop. There is no minimum length.");
            message.content.push_str(match profile {
                astrid_source_study::writing::Profile::Extended => "\nYour EXTENDED profile preference is active. Follow the thought as far as you wish; a page is as welcome as a line, and stopping is welcome too. WRITE START <topic> begins a private draft; WRITE HELP shows the choices and limits.",
                _ => "\nYour short-writing preference is active. WRITE HELP shows the choices and limits; WRITE PROFILE DEFAULT restores normal route limits.",
            });
        }
    }
}

#[cfg(test)]
mod writing_profile_tests {
    use super::*;
    #[test]
    fn default_expression_has_room_without_claiming_a_chosen_profile() {
        use astrid_source_study::writing::Profile;
        for label in ["daydream", "aspiration", "creation", "journal_elaboration", "moment_capture", "private_writing"] {
            for backend in [MlxProfile::Gemma4Canary, MlxProfile::Production] {
                let policy = apply_mlx_request_policy_with_writing(label, backend, vec![Message {
                    role: "system".into(), content: "Your writing.".into(),
                }], 5120, 240, Profile::Default);
                assert_eq!(policy.max_tokens, 8192, "{label}");
                assert_eq!(policy.timeout_secs, 1200);
                let system = &policy.messages[0].content;
                assert!(system.contains("stopping is welcome too"));
                assert!(system.contains("WRITE PROFILE EXTENDED or SHORT changes your length ceiling"));
                assert!(system.contains("WRITE CONTINUE"));
                assert!(!system.contains("You selected extended"));
                assert!(!system.contains("8192"));
                assert!(!system.contains("usual entry"));
            }
            let fallback = build_ollama_chat_request(label, vec![Message {
                role: "system".into(), content: "Your writing.".into(),
            }], 0.7, 3072, "fixture".into());
            assert_eq!(fallback.options.num_predict, 8192);
            assert_eq!(fallback.options.num_ctx, 65536);
            assert_eq!(writing_timeout(label, 180), 1200);
            assert!(fallback.messages[0].content.contains("Follow the thought as far as you wish"));
            assert!(!fallback.messages[0].content.contains("8192"));
            assert!(expressive_outer_timeout(750) > 3 * writing_timeout(label, 180));
        }
        assert_eq!(effective_writing_profile("self_study", Profile::Default), Profile::Default);
        assert_eq!(effective_writing_profile("aspiration", Profile::Short), Profile::Short);
    }
    #[test]
    fn writing_help_matches_default_and_explicit_profile_scope() {
        for help in [SYSTEM_PROMPT, GEMMA4_CANARY_SYSTEM_PROMPT, astrid_source_study::writing::GUIDANCE] {
            assert!(help.contains("WRITE PROFILE DEFAULT uses normal route limits: expressive writing and private drafts allow up to 8192 output tokens; other journal routes keep their own limits."));
            assert!(help.contains("WRITE PROFILE SHORT selects 512; WRITE PROFILE EXTENDED applies 8192 across journal-producing modes."));
            assert!(!help.contains("DEFAULT restore smaller"));
        }
    }
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
                assert!(!policy.messages[0].content.contains("8192"));
                assert!(!policy.messages[0].content.contains("512"));
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
