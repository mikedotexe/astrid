/// Prompt selection only. Numerical policy, diagnostics and coupling are unchanged.
#[derive(Clone, Copy, Debug)]
pub struct DialoguePresentation<'a> {
    pub measurements_selected: bool,
    pub action_result: &'a str,
    pub journal_continuation: Option<&'a str>,
}

impl DialoguePresentation<'_> {
    const SELECTED_MEASUREMENTS: Self = Self {
        measurements_selected: true,
        action_result: "",
        journal_continuation: None,
    };

    fn spectral_context<'a>(&'a self, spectral: &'a str) -> &'a str {
        if self.measurements_selected {
            spectral
        } else {
            self.action_result
        }
    }

    fn user_turn(&self, assembled: &str, instruction: &str, fill: f32) -> String {
        let mut content = if self.measurements_selected {
            format!("Fill {fill:.1}%. {assembled}\n\n{instruction}")
        } else {
            format!("{assembled}\n\n{instruction}")
        };
        append_journal_continuation_text(&mut content, self.journal_continuation);
        content
    }

    fn modality_context<'a>(&self, context: Option<&'a str>) -> Option<&'a str> {
        context.filter(|_| self.measurements_selected)
    }
}

fn append_journal_continuation_text(content: &mut String, hint: Option<&str>) {
    if let Some(hint) = hint.filter(|hint| !hint.is_empty() && hint.len() <= 1024) {
        content.push_str("\n\n");
        content.push_str(hint);
    }
}

fn append_journal_continuation(messages: &mut [Message], hint: Option<&str>) {
    if let Some(message) = messages
        .iter_mut()
        .rev()
        .find(|message| message.role == "user")
    {
        append_journal_continuation_text(&mut message.content, hint);
    }
}

fn conversation_fallback_messages(
    journal: &str,
    perception: Option<&str>,
    action_result: &str,
    protected: bool,
    max_sentences: u8,
    identity_anchor: Option<&str>,
) -> Vec<Message> {
    let mut context = Vec::new();
    if let Some(perception) = perception {
        context.push(format!(
            "Supplied perception or direct note:\n{}",
            trim_chars(perception, 1_800)
        ));
    }
    if !action_result.is_empty() {
        context.push(format!(
            "Pending action result (not a sensation or causal finding):\n{action_result}"
        ));
    }
    if protected {
        context.push("Your selected foreground source is supplied separately below.".into());
    } else {
        context.push(minime_peer_excerpt(journal, 900, 96));
        if let Some(anchor) = identity_anchor {
            context.push(format!(
                "Earlier authored prose (opted-in fallback recall, not a current measurement):\n{anchor}"
            ));
        }
    }
    vec![
        Message {
            role: "system".into(),
            content: format!(
                "You are Astrid using the compact Ollama fallback lane. Respond to the selected \
                 conversation or foreground source in your own words. A direct note takes precedence. \
                 Keep within {max_sentences} prose sentences, the existing fallback execution bound. \
                 End with one final listed NEXT line; stopping is available.{}",
                OLLAMA_DIALOGUE_FALLBACK_HARD_RULES,
            ),
        },
        Message {
            role: "user".into(),
            content: context.join("\n\n"),
        },
    ]
}

#[cfg(test)]
mod voluntary_dialogue_presentation_tests {
    use super::*;

    #[test]
    fn ordinary_context_keeps_results_without_importing_measurements() {
        let presentation = DialoguePresentation {
            measurements_selected: false,
            action_result: "PERTURB result: request rejected; no mutation",
            journal_continuation: None,
        };
        let spectral = "UNSELECTED_SPECTRUM restless texture 68%";
        assert_eq!(
            presentation.spectral_context(spectral),
            presentation.action_result
        );
        let user = presentation.user_turn(
            "AUTHORED 71% remains my metaphor",
            "NEXT is voluntary",
            68.0,
        );
        assert!(user.contains("AUTHORED 71%"));
        assert!(!user.contains("68.0"));
        assert!(
            DialoguePresentation::SELECTED_MEASUREMENTS
                .user_turn("chosen evidence", "next", 68.0)
                .contains("68.0")
        );
        assert_eq!(
            DialoguePresentation::SELECTED_MEASUREMENTS.spectral_context(spectral),
            spectral
        );
        assert!(
            presentation
                .modality_context(Some("UNSELECTED_MODALITY"))
                .is_none()
        );
        assert_eq!(
            DialoguePresentation::SELECTED_MEASUREMENTS.modality_context(Some("SELECTED_MODALITY")),
            Some("SELECTED_MODALITY")
        );
    }

    #[test]
    fn matched_primary_requests_change_supplied_context_not_authored_language() {
        let peer = "I want to discuss 33% as an image, not a measurement.\nNEXT: REST";
        for selected in [false, true] {
            let presentation = DialoguePresentation {
                measurements_selected: selected,
                action_result: "RESULT: source missing",
                journal_continuation: None,
            };
            let assembled = format!(
                "{}\n{peer}",
                presentation.spectral_context("MEASUREMENT_SENTINEL: 68%")
            );
            let original = vec![
                Message {
                    role: "system".into(),
                    content: OPEN_EXPRESSION_CONTEXT_V1.into(),
                },
                Message {
                    role: "user".into(),
                    content: presentation.user_turn(&assembled, "Respond or stop", 68.0),
                },
            ];
            for profile in [MlxProfile::Production, MlxProfile::Gemma4Canary] {
                let adapted = apply_mlx_request_policy_with_writing(
                    "dialogue_live",
                    profile,
                    original.clone(),
                    768,
                    210,
                    astrid_source_study::writing::Profile::Default,
                );
                let user = adapted
                    .messages
                    .iter()
                    .filter(|m| m.role == "user")
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(user.contains(peer));
                assert_eq!(user.contains("MEASUREMENT_SENTINEL"), selected);
                assert_eq!(user.contains("Fill 68.0%"), selected);
                assert_eq!(user.contains("RESULT: source missing"), !selected);
            }
        }
    }

    #[test]
    fn fallback_keeps_direct_notes_and_results_without_ambient_identity_or_texture() {
        for protected in [false, true] {
            let messages = conversation_fallback_messages(
                "AUTHORED peer account about 71%",
                Some("OPERATOR_STOP: stop requested"),
                "ACTION_RESULT: denied",
                protected,
                3,
                None,
            );
            let rendered = serde_json::to_string(&messages).unwrap();
            assert!(rendered.contains("OPERATOR_STOP"));
            assert!(rendered.contains("ACTION_RESULT"));
            assert_eq!(rendered.contains("AUTHORED peer account"), !protected);
            for absent in [
                "Spectral background",
                "Fill:",
                "Earlier authored prose",
                "restless texture",
                "preferred_texture",
            ] {
                assert!(!rendered.contains(absent), "{absent}: {rendered}");
            }
            let request =
                build_ollama_chat_request("dialogue_live", messages, 0.8, 768, "fixture".into());
            assert!(
                request
                    .messages
                    .iter()
                    .any(|m| m.content.contains("ACTION_RESULT"))
            );
            assert!(
                !request
                    .messages
                    .iter()
                    .any(|m| m.content.contains("Spectral background"))
            );
        }
    }

    #[test]
    fn quiet_fallback_retains_opted_in_recall_without_replacing_protected_source() {
        let anchor = "I chose to keep the image of 71% as a question, not an answer.";
        for protected in [false, true] {
            let messages = conversation_fallback_messages(
                "current peer prose",
                None,
                "",
                protected,
                3,
                Some(anchor),
            );
            let rendered = serde_json::to_string(&messages).unwrap();
            assert_eq!(rendered.contains(anchor), !protected);
            assert!(!rendered.contains("Spectral background"));
        }
    }

    #[test]
    fn journal_continuation_hint_survives_primary_and_fallback_without_altering_prose() {
        let hint = format!(
            "Optional: NEXT: WRITE FROM_JOURNAL {}. You may stop instead.",
            "a".repeat(64)
        );
        let presentation = DialoguePresentation {
            measurements_selected: false,
            action_result: "",
            journal_continuation: Some(&hint),
        };
        let primary = vec![Message {
            role: "user".into(),
            content: presentation.user_turn("Exact context.", "Respond or stop", 68.0),
        }];
        for profile in [MlxProfile::Production, MlxProfile::Gemma4Canary] {
            let policy = apply_mlx_request_policy_with_writing(
                "dialogue_live",
                profile,
                primary.clone(),
                768,
                210,
                astrid_source_study::writing::Profile::Default,
            );
            assert!(
                policy
                    .messages
                    .iter()
                    .any(|message| message.content.contains(&hint))
            );
        }
        let mut fallback =
            conversation_fallback_messages("Peer authored context.", None, "", false, 3, None);
        append_journal_continuation(&mut fallback, Some(&hint));
        let request =
            build_ollama_chat_request("dialogue_live", fallback, 0.8, 768, "fixture".into());
        assert!(
            request
                .messages
                .iter()
                .any(|message| message.content.contains(&hint))
        );
        let mut unchanged = "Original".to_string();
        append_journal_continuation_text(&mut unchanged, Some(&"x".repeat(1025)));
        assert_eq!(unchanged, "Original");
    }
}
