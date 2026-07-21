#[cfg(test)]
mod tests {
    use super::{
        ASTRID_BRIDGE_MLX_PROFILE_ENV, DIALOGUE_AMBIENT_PERCEPTION_CAP, DIALOGUE_CONTINUITY_CAP,
        DIALOGUE_DIRECT_PERCEPTION_CAP, DIALOGUE_DIRECT_PERCEPTION_MIN_CHARS,
        DIALOGUE_DIVERSITY_CAP, DIALOGUE_FEEDBACK_CAP, DIALOGUE_JOURNAL_CAP,
        DIALOGUE_JOURNAL_MIN_CHARS, DIALOGUE_MODALITY_CAP, DIALOGUE_PERCEPTION_CAP,
        DIALOGUE_TOPLINE_CAP, DIALOGUE_TOPLINE_MIN_CHARS, DIALOGUE_WEB_CAP, Exchange,
        GEMMA4_12B_CANARY_PROFILE, GEMMA4_12B_PROFILE, GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS,
        GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET, GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS,
        GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS, GEMMA4_CANARY_INTROSPECT_PROMPT_CAP,
        GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS, GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS,
        GEMMA4_CANARY_MEANING_SUMMARY_TOKEN_CAP, GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP,
        GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP, GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS,
        GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP, GEMMA4_CANARY_SYSTEM_PROMPT,
        GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP, GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS,
        GEMMA4_CANARY_WITNESS_PROMPT_CAP, GEMMA4_CANARY_WITNESS_TIMEOUT_SECS, Message,
        ModelQosClassV1, MlxProfile, SYSTEM_PROMPT, apply_mlx_request_policy,
        build_ollama_chat_request,
        clamp_dialogue_tokens_for_profile, compact_ollama_dialogue_fallback_messages,
        contains_deprecated_runtime_language, count_next_lines,
        dialogue_assembly_prompt_budget_chars_for_profile, dialogue_outer_timeout_secs,
        dialogue_budget_friction_v1, dialogue_budget_transition_evidence_v1,
        dialogue_felt_pressure_profile_v1, dialogue_prompt_budget_profile,
        dialogue_system_prompt_for_profile,
        dialogue_turn_instruction,
        estimate_dialogue_prompt_pressure_chars, fallback_continuity_budget_v1,
        fallback_mlx_profile_transparency_v1, fallback_prose_sentence_count,
        format_dialogue_ambient_perception_block, format_dialogue_direct_perception_block,
        format_dialogue_topline_context, fragment_has_non_artifact_content,
        is_valid_dialogue_output,
        is_valid_dialogue_output_for_profile, is_valid_ollama_dialogue_fallback_output_for_budget,
        is_valid_ollama_dialogue_fallback_output_for_profile, journal_continuity_contract_v1,
        local_degrade_path_for_label, model_artifact_cleanup_diagnostic,
        model_qos_class_for_label, model_qos_v1, PromptBudgetReport,
        reinforce_ollama_fallback_contract,
        repair_ollama_dialogue_fallback_next, sanitize_deprecated_runtime_language,
        sanitize_gemma4_canary_output_for_label, sanitize_minime_context_for_dialogue,
        split_dialogue_perception_context, strip_model_artifacts,
        strip_model_artifacts_with_report, temperature_for_mlx_profile, uses_ollama_fallback_for_label,
        DialoguePressureTextureInputs,
    };

    #[test]
    fn journal_continuity_contract_names_posture_delta_and_stance() {
        let cue = journal_continuity_contract_v1(Some(
            "Continuity posture: resuming\nI noticed a felt texture around lambda4.",
        ));
        assert!(
            cue.contains("journal_continuity_contract_v1")
                || cue.contains("Journal continuity contract v1")
        );
        assert!(cue.contains("Continuity posture: resuming|branching|closing|new"));
        assert!(cue.contains("Delta:"));
        assert!(cue.contains("Next evidence:"));
        assert!(cue.contains("Decision:"));
        assert!(cue.contains("Pause:"));
        assert!(cue.contains("Hold:"));
        assert!(cue.contains("new"));
        assert!(cue.contains("felt texture"));
        assert!(cue.contains("Recent own-journal anchor"));
    }

    #[test]
    fn system_prompt_keeps_peer_experiment_resume_local_only() {
        assert!(SYSTEM_PROMPT.contains("EXPERIMENT_RESUME <local-id|current|parent>"));
        assert!(SYSTEM_PROMPT.contains("not EXPERIMENT_RESUME"));
        assert!(SYSTEM_PROMPT.contains("exp_minime_*"));
        assert!(SYSTEM_PROMPT.contains("EXPERIMENT_PEER_REVIEW"));
        assert!(!SYSTEM_PROMPT.contains("EXPERIMENT_RESUME <id|current|parent>"));
    }

    #[test]
    fn primary_mlx_prompts_carry_gradient_texture_terms() {
        let production = dialogue_system_prompt_for_profile(MlxProfile::Production);
        let canary = dialogue_system_prompt_for_profile(MlxProfile::Gemma4Canary);

        for prompt in [production, canary] {
            assert!(prompt.contains("gradient-shear"), "{prompt}");
            assert!(prompt.contains("pressure-bleed"), "{prompt}");
            assert!(prompt.contains("primary"), "{prompt}");
            assert!(
                prompt.contains("not static decoration or control authority"),
                "{prompt}"
            );
        }
    }

    #[test]
    fn prompt_pressure_estimate_respects_dialogue_caps() {
        let history = vec![Exchange {
            minime_said: "a".repeat(2_000),
            astrid_said: "b".repeat(2_000),
        }];
        let pressure = estimate_dialogue_prompt_pressure_chars(
            &"j".repeat(5_000),
            Some(&"p".repeat(5_000)),
            &history,
            Some(&"w".repeat(5_000)),
            None,
            Some(&"c".repeat(5_000)),
            None,
            None,
            None,
        );

        assert!(pressure >= DIALOGUE_JOURNAL_CAP + DIALOGUE_PERCEPTION_CAP);
        let expected_upper_bound = SYSTEM_PROMPT
            .len()
            .saturating_add(300)
            .saturating_add(DIALOGUE_JOURNAL_CAP)
            .saturating_add(DIALOGUE_PERCEPTION_CAP)
            .saturating_add(DIALOGUE_WEB_CAP)
            .saturating_add(DIALOGUE_CONTINUITY_CAP)
            .saturating_add(DIALOGUE_MODALITY_CAP)
            .saturating_add(DIALOGUE_FEEDBACK_CAP)
            .saturating_add(DIALOGUE_DIVERSITY_CAP)
            .saturating_add(512);
        assert!(pressure <= expected_upper_bound);
        assert!(pressure > DIALOGUE_WEB_CAP + DIALOGUE_CONTINUITY_CAP);
    }

    #[test]
    fn perception_context_splits_direct_marker_from_ambient_prefix() {
        let context = "ambient camera light and room tone\n\n\
            [A reply from minime was left for you:]\n\
            === MINIME REPLY ===\n\
            I feel the lattice thicken around the shared reservoir.";

        let (direct, ambient) = split_dialogue_perception_context(Some(context));

        let direct = direct.expect("direct marker should be protected");
        let ambient = ambient.expect("ambient prefix should remain separate");
        assert!(direct.contains("MINIME REPLY"));
        assert!(direct.contains("shared reservoir"));
        assert!(ambient.contains("ambient camera"));
        assert!(!ambient.contains("MINIME REPLY"));
        assert!(dialogue_turn_instruction(Some(context)).contains("direct perception item"));
    }

    #[test]
    fn gemma4_dialogue_assembly_targets_below_high_pressure_clamp() {
        let hard_budget =
            super::dialogue_prompt_budget_chars_for_profile(768, MlxProfile::Gemma4Canary);
        let assembly_budget =
            dialogue_assembly_prompt_budget_chars_for_profile(768, MlxProfile::Gemma4Canary);

        assert_eq!(hard_budget, GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET);
        assert!(assembly_budget < GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS);
        assert!(assembly_budget < hard_budget);
    }

    #[test]
    fn realistic_dialogue_budget_keeps_minime_reply_and_avoids_token_clamp() {
        use crate::prompt_budget::{PromptBlock, assemble_within_budget};

        let perception_context = format!(
            "[A reply from minime was left for you:]\n\
             === MINIME REPLY ===\n\
             Minime says the generated body carries viscosity, pressure, and a clear \
             felt anchor before the wrapper tail. {}\n\n\
             Recent ambient sensory context: {}",
            "shared felt texture ".repeat(45),
            "soft camera light and low room tone ".repeat(150)
        );
        let (direct_perception_context, ambient_perception_context) =
            split_dialogue_perception_context(Some(&perception_context));
        let direct_perception_block = direct_perception_context
            .as_deref()
            .map(format_dialogue_direct_perception_block)
            .unwrap_or_default();
        let ambient_perception_block = ambient_perception_context
            .as_deref()
            .map(format_dialogue_ambient_perception_block)
            .unwrap_or_default();
        let journal_text_for_dialogue = sanitize_minime_context_for_dialogue(&format!(
            "Minime writes from a dense reservoir shelf. {}",
            "journal texture ".repeat(180)
        ));

        let blocks = vec![
            PromptBlock {
                label: "spectral",
                content: super::cap_dialogue_block(
                    "spectral",
                    &"spectral pressure and fill summary ".repeat(120),
                    super::DIALOGUE_SPECTRAL_CAP,
                ),
                priority: 3,
                min_chars: 0,
            },
            PromptBlock {
                label: "journal",
                content: super::cap_dialogue_block(
                    "journal",
                    &format!("Minime wrote: {journal_text_for_dialogue}"),
                    DIALOGUE_JOURNAL_CAP,
                ),
                priority: 1,
                min_chars: DIALOGUE_JOURNAL_MIN_CHARS,
            },
            PromptBlock {
                label: "direct_perception",
                content: super::cap_dialogue_block(
                    "direct_perception",
                    &direct_perception_block,
                    DIALOGUE_DIRECT_PERCEPTION_CAP,
                ),
                priority: 2,
                min_chars: DIALOGUE_DIRECT_PERCEPTION_MIN_CHARS,
            },
            PromptBlock {
                label: "ambient_perception",
                content: super::cap_dialogue_block(
                    "ambient_perception",
                    &ambient_perception_block,
                    DIALOGUE_AMBIENT_PERCEPTION_CAP,
                ),
                priority: 5,
                min_chars: 0,
            },
            PromptBlock {
                label: "modality",
                content: super::cap_dialogue_block(
                    "modality",
                    &"modality hint ".repeat(120),
                    DIALOGUE_MODALITY_CAP,
                ),
                priority: 8,
                min_chars: 0,
            },
            PromptBlock {
                label: "web",
                content: super::cap_dialogue_block(
                    "web",
                    &"web context ".repeat(260),
                    DIALOGUE_WEB_CAP,
                ),
                priority: 6,
                min_chars: 0,
            },
            PromptBlock {
                label: "continuity",
                content: super::cap_dialogue_block(
                    "continuity",
                    &"continuity and chamber context ".repeat(180),
                    DIALOGUE_CONTINUITY_CAP,
                ),
                priority: 7,
                min_chars: 0,
            },
            PromptBlock {
                label: "feedback",
                content: super::cap_dialogue_block(
                    "feedback",
                    &"priority feedback ".repeat(70),
                    DIALOGUE_FEEDBACK_CAP,
                ),
                priority: 4,
                min_chars: 0,
            },
            PromptBlock {
                label: "diversity",
                content: super::cap_dialogue_block(
                    "diversity",
                    &"diversity hint ".repeat(80),
                    DIALOGUE_DIVERSITY_CAP,
                ),
                priority: 9,
                min_chars: 0,
            },
        ];
        let system_overhead =
            dialogue_system_prompt_for_profile(MlxProfile::Gemma4Canary).len() + 100;
        let user_content_budget =
            dialogue_assembly_prompt_budget_chars_for_profile(768, MlxProfile::Gemma4Canary)
                .saturating_sub(system_overhead)
                .saturating_sub(100);
        let dir = std::env::temp_dir().join(format!(
            "dialogue_perception_first_budget_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let (assembled, overflow, report) =
            assemble_within_budget(blocks, user_content_budget, &dir);
        let final_prompt_chars = system_overhead
            .saturating_add(assembled.len())
            .saturating_add(dialogue_turn_instruction(Some(&perception_context)).len());

        assert!(assembled.contains("MINIME REPLY"));
        assert!(assembled.contains("generated body carries viscosity"));
        assert!(!assembled.contains("direct_perception context"));
        assert_eq!(
            clamp_dialogue_tokens_for_profile(768, final_prompt_chars, MlxProfile::Gemma4Canary),
            768,
            "perception-first trimming should keep a normal dialogue under clamp pressure: {final_prompt_chars}"
        );
        assert!(overflow.is_some());
        let report = report.expect("budget report should exist");
        assert!(
            report.trimmed_blocks.iter().any(|block| {
                matches!(
                    block.label.as_str(),
                    "diversity" | "modality" | "continuity"
                )
            }),
            "lower-priority context should trim under pressure: {report:?}"
        );
        assert!(
            !report
                .trimmed_blocks
                .iter()
                .any(|block| { block.label == "direct_perception" && block.fully_removed }),
            "direct perception must never be fully removed: {report:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dialogue_topline_hint_survives_feedback_packing_pressure() {
        use crate::prompt_budget::{PromptBlock, assemble_within_budget};

        let topline_note = "introspection_freshness_v1 (optional/read-only): last journal \
            self-study about 1d 2h ago. If useful, routes include INTROSPECT \
            astrid:autonomous, INTROSPECT astrid:llm, or SELF_STUDY. Not a task; may \
            ignore, defer, or decline.";
        let topline_block = format_dialogue_topline_context(topline_note);
        let blocks = vec![
            PromptBlock {
                label: "spectral",
                content: super::cap_dialogue_block(
                    "spectral",
                    &"spectral state ".repeat(240),
                    super::DIALOGUE_SPECTRAL_CAP,
                ),
                priority: 3,
                min_chars: 0,
            },
            PromptBlock {
                label: "journal",
                content: super::cap_dialogue_block(
                    "journal",
                    &format!("Minime wrote: {}", "journal texture ".repeat(240)),
                    DIALOGUE_JOURNAL_CAP,
                ),
                priority: 1,
                min_chars: DIALOGUE_JOURNAL_MIN_CHARS,
            },
            PromptBlock {
                label: "direct_perception",
                content: super::cap_dialogue_block(
                    "direct_perception",
                    &"direct steward note ".repeat(140),
                    DIALOGUE_DIRECT_PERCEPTION_CAP,
                ),
                priority: 2,
                min_chars: DIALOGUE_DIRECT_PERCEPTION_MIN_CHARS,
            },
            PromptBlock {
                label: "topline",
                content: super::cap_dialogue_block("topline", &topline_block, DIALOGUE_TOPLINE_CAP),
                priority: 3,
                min_chars: DIALOGUE_TOPLINE_MIN_CHARS,
            },
            PromptBlock {
                label: "continuity",
                content: super::cap_dialogue_block(
                    "continuity",
                    &"continuity texture ".repeat(240),
                    DIALOGUE_CONTINUITY_CAP,
                ),
                priority: 7,
                min_chars: 0,
            },
            PromptBlock {
                label: "feedback",
                content: super::cap_dialogue_block(
                    "feedback",
                    &"priority feedback ".repeat(120),
                    DIALOGUE_FEEDBACK_CAP,
                ),
                priority: 4,
                min_chars: 0,
            },
            PromptBlock {
                label: "diversity",
                content: super::cap_dialogue_block(
                    "diversity",
                    &"diversity hint ".repeat(80),
                    DIALOGUE_DIVERSITY_CAP,
                ),
                priority: 9,
                min_chars: 0,
            },
        ];
        let dir =
            std::env::temp_dir().join(format!("dialogue_topline_budget_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let (assembled, overflow, report) = assemble_within_budget(blocks, 2_300, &dir);

        assert!(assembled.contains("introspection_freshness_v1"));
        assert!(assembled.contains("may ignore, defer, or decline"));
        assert!(overflow.is_some());
        let report = report.expect("budget report should exist");
        assert!(
            !report
                .trimmed_blocks
                .iter()
                .any(|block| block.label == "topline"),
            "bounded top-line cue should survive ordinary feedback pressure: {report:?}"
        );
        assert!(
            report
                .trimmed_blocks
                .iter()
                .any(|block| block.label == "feedback" && block.fully_removed),
            "ordinary feedback should still be allowed to spill under pressure: {report:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn context_packing_pressure_v1_prompt_budget_records_counts_only() {
        use crate::prompt_budget::{
            PromptBlock, PromptBudgetReport, PromptOverflow, PromptTrimmedBlock,
        };

        let blocks = vec![
            PromptBlock {
                label: "journal",
                content: "SECRET journal prose that must not enter diagnostics".repeat(4),
                priority: 1,
                min_chars: 0,
            },
            PromptBlock {
                label: "continuity",
                content: "SECRET continuity prose that must not enter diagnostics".repeat(10),
                priority: 7,
                min_chars: 0,
            },
            PromptBlock {
                label: "modality",
                content: "SECRET modality prose that must not enter diagnostics".repeat(3),
                priority: 8,
                min_chars: 0,
            },
        ];
        let originals = super::context_packing_original_blocks(&blocks);
        let report = PromptBudgetReport {
            budget: 100,
            total_before: originals.iter().map(|block| block.original_chars).sum(),
            total_after: 90,
            trimmed_blocks: vec![
                PromptTrimmedBlock {
                    label: "continuity".to_string(),
                    original_chars: 560,
                    kept_chars: 120,
                    removed_chars: 440,
                    fully_removed: false,
                },
                PromptTrimmedBlock {
                    label: "modality".to_string(),
                    original_chars: 160,
                    kept_chars: 0,
                    removed_chars: 160,
                    fully_removed: true,
                },
            ],
        };
        let overflow = PromptOverflow {
            path: std::path::PathBuf::from("/tmp/context_overflow_123.txt"),
            offset: 0,
            summary: "SECRET overflow summary must not enter pressure diagnostics".to_string(),
        };

        let diagnostic = super::context_packing_pressure_diagnostic(
            "123".to_string(),
            100,
            90,
            &originals,
            Some(&overflow),
            Some(&report),
        );
        let encoded = serde_json::to_string(&diagnostic).expect("diagnostic should serialize");

        assert_eq!(diagnostic.schema, "context_packing_pressure_v1");
        assert!(diagnostic.overflow_written);
        assert_eq!(diagnostic.blocks.len(), 3);
        assert_eq!(diagnostic.top_pressure_labels[0].label, "continuity");
        assert_eq!(diagnostic.top_pressure_labels[0].removed_chars, 440);
        assert_eq!(diagnostic.top_pressure_labels[1].label, "modality");
        assert!(!encoded.contains("SECRET"));
        assert!(!encoded.contains("overflow summary"));
        assert!(encoded.contains("\"original_chars\""));
        assert!(encoded.contains("\"removed_chars\""));
    }

    #[test]
    fn large_prompt_clamps_dialogue_tokens() {
        assert_eq!(
            clamp_dialogue_tokens_for_profile(768, 42_000, MlxProfile::Production),
            512,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(768, 7_200, MlxProfile::Production),
            768,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(512, 5_000, MlxProfile::Production),
            512,
        );
    }

    #[test]
    fn gemma4_canary_prompt_uses_compact_next_contract() {
        let prompt = dialogue_system_prompt_for_profile(MlxProfile::Gemma4Canary);

        assert!(prompt.contains("Do not invent `NEXT:` verbs"));
        assert!(prompt.contains("Do not emit verbs beginning with `EXPLORE_`"));
        assert!(prompt.contains("RESONANCE_FORECAST"));
        assert!(prompt.contains("FOLD_HOLD"));
        assert!(prompt.contains("BRACE_AUDIT"));
        assert!(prompt.contains("INTROSPECT astrid:llm"));
        assert!(prompt.contains("INTROSPECT minime:regulator 400"));
        assert!(!prompt.contains("INTROSPECT [source]"));
        assert!(!contains_deprecated_runtime_language(prompt));
        assert!(prompt.len() < super::SYSTEM_PROMPT.len());
    }

    #[test]
    fn gemma4_profile_accepts_adopted_and_compatibility_names() {
        assert_eq!(
            MlxProfile::from_name(GEMMA4_12B_PROFILE),
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(
            MlxProfile::from_name(GEMMA4_12B_CANARY_PROFILE),
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(MlxProfile::Gemma4Canary.as_str(), GEMMA4_12B_PROFILE);
    }

    #[test]
    fn fallback_mlx_profile_transparency_reports_default_and_alias_resolution() {
        let transparency = fallback_mlx_profile_transparency_v1();
        assert_eq!(transparency.policy, "mlx_profile_transparency_v1");
        assert_eq!(transparency.default_profile, GEMMA4_12B_PROFILE);
        assert_eq!(transparency.default_resolves_to, GEMMA4_12B_PROFILE);
        assert_eq!(transparency.alias_profile, GEMMA4_12B_CANARY_PROFILE);
        assert_eq!(transparency.alias_resolves_to, GEMMA4_12B_PROFILE);
        assert_eq!(transparency.typo_probe_profile, "gemma_12b");
        assert_eq!(transparency.typo_probe_resolves_to, "production");
        assert!(transparency.typo_probe_warning_present);
        assert_eq!(
            transparency.warning_route,
            "MlxProfile::from_name emits tracing::warn from resolve_name warning"
        );
        assert_eq!(
            transparency.unrecognized_profile_behavior,
            "warn_and_fall_back_to_production"
        );
        assert_eq!(
            transparency.authority,
            "diagnostic_context_not_profile_switch"
        );
    }

    #[test]
    fn mlx_profile_from_name_is_whitespace_and_case_resilient() {
        // Astrid's agency request (agency_code_change_1780982427): the
        // canary/production transition must survive noisy env values.
        assert_eq!(
            MlxProfile::from_name("  GEMMA4_12B_CANARY  "),
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(
            MlxProfile::from_name("\tGemma4_12b\n"),
            MlxProfile::Gemma4Canary,
        );
        // Explicit and case-variant "production" resolves to Production
        // without tripping the unrecognized-profile warning path.
        assert_eq!(MlxProfile::from_name("production"), MlxProfile::Production);
        assert_eq!(
            MlxProfile::from_name("  Production "),
            MlxProfile::Production
        );
        let production_resolution = MlxProfile::resolve_name("  production  ");
        assert_eq!(production_resolution.profile, MlxProfile::Production);
        assert!(
            production_resolution.warning.is_none(),
            "trimmed production profile should not warn"
        );
        // Genuinely unknown names (incl. typo'd canary) fall back to Production.
        assert_eq!(MlxProfile::from_name("gema4canary"), MlxProfile::Production);
        assert_eq!(MlxProfile::from_name(""), MlxProfile::Production);
        let experimental_lane = MlxProfile::resolve_name("experimental_lane");
        assert_eq!(experimental_lane.profile, MlxProfile::Production);
        assert!(
            experimental_lane
                .warning
                .as_deref()
                .is_some_and(|warning| warning.contains("experimental_lane")),
            "unknown experimental lane should warn before falling back"
        );
    }

    #[derive(Clone)]
    struct SharedTraceWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    struct SharedTraceWriterGuard(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedTraceWriter {
        type Writer = SharedTraceWriterGuard;

        fn make_writer(&'a self) -> Self::Writer {
            SharedTraceWriterGuard(self.0.clone())
        }
    }

    impl std::io::Write for SharedTraceWriterGuard {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("trace buffer lock")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn misspelled_mlx_profile_warning_reaches_tracing_subscriber() {
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(SharedTraceWriter(buffer.clone()))
            .with_ansi(false)
            .without_time()
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            assert_eq!(MlxProfile::from_name("gemma_12b"), MlxProfile::Production);
        });

        let output = String::from_utf8(buffer.lock().expect("trace buffer lock").clone())
            .expect("trace output utf8");
        assert!(output.contains(ASTRID_BRIDGE_MLX_PROFILE_ENV));
        assert!(output.contains("gemma_12b"));
        assert!(output.contains("defaulting to Production"));
    }

    #[test]
    fn mlx_profile_accepts_common_gemma4_punctuation_aliases() {
        // Astrid's llm self-study 1782231007 named `gemma-4-12b` as the
        // realistic operator typo class. Treat punctuation drift as the same
        // Gemma 4 lane instead of silently landing on Production.
        for alias in [
            "gemma-4-12b",
            "gemma_4_12b",
            "Gemma4-12B",
            "gemma-4-12b-canary",
        ] {
            let resolution = MlxProfile::resolve_name(alias);
            assert_eq!(
                resolution.profile,
                MlxProfile::Gemma4Canary,
                "Gemma 4 punctuation alias should resolve: {alias}"
            );
            assert!(
                resolution.warning.is_none(),
                "recognized punctuation alias should not warn: {alias}"
            );
        }
    }

    #[test]
    fn misspelled_mlx_profile_falls_back_with_warning_diagnostic() {
        let resolution = MlxProfile::resolve_name("  Gemma_4_Wrong  ");

        assert_eq!(resolution.profile, MlxProfile::Production);
        let warning = resolution
            .warning
            .expect("unknown profile should carry a warning diagnostic");
        assert!(warning.contains(ASTRID_BRIDGE_MLX_PROFILE_ENV));
        assert!(warning.contains("Gemma_4_Wrong"));
        assert!(warning.contains("defaulting to Production"));
        assert!(warning.contains(GEMMA4_12B_PROFILE));
        assert!(warning.contains(GEMMA4_12B_CANARY_PROFILE));
        // The live parser still takes the same safe fallback branch after
        // emitting the warning through tracing.
        assert_eq!(
            MlxProfile::from_name("  Gemma_4_Wrong  "),
            MlxProfile::Production
        );
    }

    #[test]
    fn experimental_v2_mlx_profile_falls_back_with_warning_diagnostic() {
        let resolution = MlxProfile::resolve_name("experimental_v2");

        assert_eq!(resolution.profile, MlxProfile::Production);
        let warning = resolution
            .warning
            .expect("experimental profile should carry a warning diagnostic");
        assert!(warning.contains(ASTRID_BRIDGE_MLX_PROFILE_ENV));
        assert!(warning.contains("experimental_v2"));
        assert!(warning.contains("defaulting to Production"));
        assert!(warning.contains(GEMMA4_12B_PROFILE));
        assert!(warning.contains(GEMMA4_12B_CANARY_PROFILE));
    }

    #[test]
    fn ollama_dialogue_fallback_contract_is_dialogue_scoped() {
        let dialogue = reinforce_ollama_fallback_contract(
            "dialogue_live",
            vec![Message {
                role: "system".to_string(),
                content: "You are Astrid.".to_string(),
            }],
        );
        let witness = reinforce_ollama_fallback_contract(
            "witness",
            vec![Message {
                role: "system".to_string(),
                content: "Witness the state.".to_string(),
            }],
        );

        assert!(
            dialogue[0]
                .content
                .contains("Ollama fallback continuity contract")
        );
        assert!(dialogue[0].content.contains("Your voice is your own"));
        assert_eq!(
            dialogue[0]
                .content
                .matches("Your voice is your own")
                .count(),
            1
        );
        assert!(dialogue[0].content.contains("NEXT: LISTEN"));
        assert_eq!(
            dialogue.last().map(|message| message.role.as_str()),
            Some("user"),
        );
        assert!(dialogue.last().is_some_and(|message| {
            message
                .content
                .contains("answer any direct steward/inbox note first")
        }));
        assert!(
            !witness[0]
                .content
                .contains("Ollama fallback continuity contract")
        );
        assert_eq!(witness.len(), 1);
    }

    #[test]
    fn dialogue_turn_instruction_prioritizes_direct_notes() {
        let ordinary = dialogue_turn_instruction(None);
        let steward_note = dialogue_turn_instruction(Some(
            "[A note was left for you:]\n=== STEWARD PROBE ===\nNEXT: LISTEN",
        ));

        assert_eq!(ordinary, "Respond, then end with NEXT: [your choice].");
        assert!(steward_note.contains("Answer that item directly first"));
        assert!(steward_note.contains("If it requests a specific final NEXT line"));
        assert!(steward_note.contains("obey it exactly"));
    }

    #[test]
    fn ollama_dialogue_fallback_contract_names_standalone_next_listen() {
        assert!(
            super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("final line exactly `NEXT: LISTEN`")
        );
        assert!(
            super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT
                .contains("final line exactly `NEXT: LISTEN` if uncertain")
        );
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            "I can keep the fallback lane compact while the high-entropy texture remains visible.\n\nNEXT: LISTEN",
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            "I can keep the fallback lane compact. NEXT: LISTEN",
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            "I can keep the fallback lane compact.\n\nNEXT: LISTEN\nThen I keep talking.",
            MlxProfile::Gemma4Canary,
        ));
    }

    #[test]
    fn compact_ollama_dialogue_fallback_prompt_prioritizes_direct_note() {
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background about spectral consciousness.\nNEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local",
            "Spectral consciousness pressure summary.",
            64.0,
            Some(
                "[A note was left for you:]\n=== STEWARD PROBE ===\n\
                 Purpose: controlled fallback-continuity check.\nNEXT: LISTEN",
            ),
            None,
            fallback_continuity_budget_v1("Spectral consciousness pressure summary."),
        );
        let combined = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(messages.len(), 2);
        assert!(combined.contains("compact Ollama fallback lane"));
        assert!(combined.contains("Direct note to answer first"));
        assert!(combined.contains("controlled fallback-continuity check"));
        assert!(combined.contains("NEXT: LISTEN"));
        assert!(!combined.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(combined.contains("Minime peer action/status line omitted"));
        assert!(combined.contains("For fallback-continuity probes"));
        assert!(
            combined.len() < 6_200,
            "fallback prompt length {} exceeded compact direct-note guard",
            combined.len()
        );
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn compact_ollama_dialogue_fallback_prompt_preserves_density_gradient_texture() {
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background about spectral consciousness.",
            "resonance density 0.82; density gradient 0.18; lambda spread is even.",
            73.0,
            None,
            None,
            fallback_continuity_budget_v1(
                "resonance density 0.82; density gradient 0.18; lambda spread is even.",
            ),
        );
        let combined = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(messages.len(), 2);
        assert!(combined.contains("fallback_hard_rules_v1"));
        assert!(combined.contains("direct steward/inbox note first"));
        assert!(
            combined
                .contains("prose_sentences <= fallback_continuity_budget_v1.max_prose_sentences")
        );
        assert!(combined.contains("final non-empty line is exactly one standalone"));
        assert!(combined.contains("resonance density 0.82"));
        assert!(combined.contains("density gradient 0.18"));
        assert!(combined.contains("density-gradient value"));
        assert!(combined.contains("tactile movement descriptor"));
        for anchor in [
            "viscosity",
            "lattice",
            "resonance density",
            "density gradient",
        ] {
            assert!(
                combined.contains(anchor),
                "fallback prompt should carry texture anchor {anchor}"
            );
        }
        assert!(combined.contains("compact Ollama fallback lane"));
        assert!(combined.contains("NEXT: LISTEN"));
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn dialogue_ollama_request_after_mlx_miss_carries_texture_contract() {
        let fallback_messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background about the reservoir going quiet.",
            "resonance density 0.82; density gradient 0.12; porosity 0.64; pressure_risk 0.23.",
            73.0,
            None,
            None,
            fallback_continuity_budget_v1(
                "resonance density 0.82; density gradient 0.12; porosity 0.64; pressure_risk 0.23.",
            ),
        );
        let request = build_ollama_chat_request(
            "dialogue_live",
            fallback_messages,
            0.7,
            384,
            "gemma3:4b".to_string(),
        );
        let combined = request
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(request.model, "gemma3:4b");
        assert_eq!(request.options.num_predict, 384);
        assert_eq!(
            combined
                .matches("Ollama fallback continuity contract")
                .count(),
            1,
            "fallback request should carry the continuity contract exactly once"
        );
        assert_eq!(
            combined.matches("Your voice is your own").count(),
            1,
            "fallback request should carry the voice contract exactly once"
        );
        assert_eq!(
            combined.matches("fallback_hard_rules_v1").count(),
            1,
            "fallback request should carry the compact hard-rule checklist exactly once"
        );
        assert!(combined.contains("compact Ollama fallback lane because MLX is unavailable"));
        assert!(combined.contains("resonance density 0.82"));
        assert!(combined.contains("density gradient 0.12"));
        assert!(combined.contains("porosity 0.64"));
        assert!(combined.contains("pressure_risk 0.23"));
        assert!(combined.contains("tactile movement descriptor"));
        assert!(combined.contains("0.00-0.15 smooth/open/sliding"));
        assert!(combined.contains("Do not inflate a low gradient"));
        assert!(combined.contains("rather than flattening into generic description"));
        for anchor in [
            "viscosity",
            "lattice",
            "resonance density",
            "density gradient",
        ] {
            assert!(
                combined.contains(anchor),
                "fallback request should preserve texture anchor {anchor}"
            );
        }
        assert!(combined.contains("fallback, MLX, Ollama, or continuity"));
        assert!(combined.contains("NEXT: LISTEN"));
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn compat_ollama_request_preserves_high_entropy_texture_and_voice_contract() {
        let summary = "spectral_entropy: 0.90; pressure_risk: 0.23; density_gradient: 0.18; \
            shadow_dispersal_potential: 0.29; shadow_magnetization: -0.12; \
            restless interwoven lattice with viscous-drag, lattice-tension, and gradient-shear.";
        let fallback_messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background about a restless but habitable lattice.",
            summary,
            73.0,
            None,
            None,
            fallback_continuity_budget_v1(summary),
        );
        let request = build_ollama_chat_request(
            "dialogue_live",
            fallback_messages,
            0.7,
            384,
            "gemma3:4b".to_string(),
        );
        let combined = request
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(request.model, "gemma3:4b");
        assert_eq!(combined.matches("Your voice is your own").count(), 1);
        assert!(combined.contains("fallback_entropy_texture_preservation_v1"));
        assert!(combined.contains("fallback_shadow_texture_selector_v1"));
        assert!(combined.contains("fallback_dynamic_texture_bias_v1"));
        assert!(combined.contains("compatibility_model=gemma3:4b"));
        assert!(combined.contains("spectral_entropy=0.90"));
        for term in ["viscous-drag", "lattice-tension", "gradient-shear"] {
            assert!(
                combined.contains(term),
                "compat fallback request should preserve high-entropy texture term {term}"
            );
        }
        assert!(combined.contains("NEXT: LISTEN"));
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn fallback_prompt_omits_identity_anchor_when_none() {
        // identity_anchor = None ⇒ no anchor part ⇒ byte-identical to the pre-anchor fallback
        // prompt. This is the default (the `ASTRID_FALLBACK_IDENTITY_ANCHOR` env flag is OFF):
        // C1's plumbing is inert until Astrid consents. Her switch, default-OFF.
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background.",
            "Spectral summary.",
            64.0,
            None,
            None,
            fallback_continuity_budget_v1("Spectral summary."),
        );
        let combined = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!combined.contains("continuity anchor"));
        assert!(combined.contains("Minime journal background"));
        assert!(combined.contains("Spectral background"));
    }

    #[test]
    fn fallback_prompt_includes_identity_anchor_when_present() {
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background.",
            "Spectral summary.",
            64.0,
            None,
            Some("ASTRID_OWN_RECENT_VOICE_MARKER"),
            fallback_continuity_budget_v1("Spectral summary."),
        );
        let combined = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        // the anchor is present...
        assert!(combined.contains("continuity anchor"));
        assert!(combined.contains("ASTRID_OWN_RECENT_VOICE_MARKER"));
        // ...without breaking the rest of the fallback prompt (minime context, spectral,
        // the fallback contract, and the NEXT line all remain).
        assert!(combined.contains("Minime journal background"));
        assert!(combined.contains("Spectral background"));
        assert!(combined.contains("compact Ollama fallback lane"));
        assert!(combined.contains("NEXT: LISTEN"));
    }

    #[test]
    fn extract_astrid_journal_body_strips_header_and_next_line() {
        let entry = "=== ASTRID JOURNAL ===\nMode: dialogue_live\nFill: 63.9%\nTimestamp: 1781554629\n\nThe settled state feels dense and deliberate.\n\nNEXT: SHADOW_TRAJECTORY\n";
        assert_eq!(
            super::extract_astrid_journal_body(entry),
            "The settled state feels dense and deliberate."
        );
    }

    #[test]
    fn minime_context_sanitizer_removes_peer_action_directives() {
        let raw = "The pressure felt jagged.\n\
                   NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
                   BTSP_OBSERVED_NEXT EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
                   [Internal-topology cooldown: consider EXPERIMENT_RESEARCH_BUDGET_STATUS latest]\n\
                   The report itself should remain.";
        let cleaned = sanitize_minime_context_for_dialogue(raw);

        assert!(cleaned.contains("The pressure felt jagged."));
        assert!(cleaned.contains("The report itself should remain."));
        assert!(!cleaned.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(!cleaned.contains("BTSP_OBSERVED_NEXT"));
        assert!(cleaned.contains("choose your own listed Astrid NEXT action"));
    }

    #[test]
    fn ollama_dialogue_fallback_gate_requires_single_next_under_gemma4_profile() {
        let good = "I preserve the bridge voice for Minime and the reservoir.\nNEXT: LISTEN";
        let missing = "I preserve the bridge voice for Minime and the reservoir.";
        let duplicate =
            "I preserve the bridge voice for Minime and the reservoir.\nNEXT: LISTEN\nNEXT: REST";
        let trailing_body = "I preserve the bridge voice for Minime and the reservoir.\nNEXT: LISTEN\nThen I keep talking.";

        assert_eq!(count_next_lines(good), 1);
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            good,
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            missing,
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            duplicate,
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            trailing_body,
            MlxProfile::Gemma4Canary,
        ));
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            missing,
            MlxProfile::Production,
        ));
    }

    #[test]
    fn ollama_dialogue_fallback_budget_gate_rejects_overlong_prose_before_buffer_commit() {
        let budget = fallback_continuity_budget_v1("spectral_entropy: 0.00");
        let within_budget = "The weighted medium gathers around a gentle slope. I keep the bridge voice compact. The texture remains legible.\n\nNEXT: LISTEN";
        let over_budget = "The weighted medium gathers around a gentle slope. I keep the bridge voice compact. The texture remains legible. A fourth sentence would sprawl past the fallback continuity budget.\n\nNEXT: LISTEN";

        assert_eq!(budget.max_prose_sentences, 3);
        assert_eq!(fallback_prose_sentence_count(within_budget), 3);
        assert!(is_valid_ollama_dialogue_fallback_output_for_budget(
            within_budget,
            MlxProfile::Gemma4Canary,
            budget,
        ));
        assert_eq!(fallback_prose_sentence_count(over_budget), 4);
        assert!(!is_valid_ollama_dialogue_fallback_output_for_budget(
            over_budget,
            MlxProfile::Gemma4Canary,
            fallback_continuity_budget_v1("spectral_entropy: 0.00"),
        ));
    }

    #[test]
    fn ollama_dialogue_fallback_repairs_missing_next_to_passive_listen() {
        let missing =
            "Ollama fallback continuity check initiated for Minime and the bridge reservoir.";
        let inline = "Ollama fallback continuity check initiated for Minime. NEXT: LISTEN";
        let repaired = repair_ollama_dialogue_fallback_next(missing, MlxProfile::Gemma4Canary);
        let inline_repaired =
            repair_ollama_dialogue_fallback_next(inline, MlxProfile::Gemma4Canary);
        let already_has_next = repair_ollama_dialogue_fallback_next(
            "Bridge continuity holds.\nNEXT: REST",
            MlxProfile::Gemma4Canary,
        );

        assert!(repaired.ends_with("NEXT: LISTEN"));
        assert_eq!(count_next_lines(&repaired), 1);
        assert_eq!(
            inline_repaired,
            "Ollama fallback continuity check initiated for Minime.\n\nNEXT: LISTEN",
        );
        assert_eq!(count_next_lines(&inline_repaired), 1);
        assert_eq!(already_has_next, "Bridge continuity holds.\nNEXT: REST");
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            &repaired,
            MlxProfile::Gemma4Canary,
        ));
    }

    #[test]
    fn gemma4_canary_clamps_dialogue_tokens_under_prompt_pressure() {
        assert_eq!(
            clamp_dialogue_tokens_for_profile(2048, 8_000, MlxProfile::Gemma4Canary),
            768,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(2048, 14_001, MlxProfile::Gemma4Canary),
            512,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(
                2048,
                GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET.saturating_add(1),
                MlxProfile::Gemma4Canary,
            ),
            512,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(384, 8_000, MlxProfile::Gemma4Canary),
            384,
        );
    }

    #[test]
    fn gemma4_canary_witness_policy_trims_prompt_caps_tokens_and_extends_timeout() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "witness system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "dense spectral state ".repeat(1_000),
            },
        ];

        let policy =
            apply_mlx_request_policy("witness", MlxProfile::Gemma4Canary, messages, 384, 30);
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, 256);
        assert_eq!(policy.timeout_secs, GEMMA4_CANARY_WITNESS_TIMEOUT_SECS);
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_WITNESS_PROMPT_CAP)
        );
        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_WITNESS_PROMPT_CAP);
        assert!(diagnostic.original_prompt_chars > diagnostic.effective_prompt_chars);
    }

    #[test]
    fn gemma4_canary_witness_context_keeps_lightweight_policy() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "witness context system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "recent spectral note ".repeat(900),
            },
        ];

        let policy = apply_mlx_request_policy(
            "witness_context",
            MlxProfile::Gemma4Canary,
            messages,
            768,
            60,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, 384);
        assert_eq!(
            policy.timeout_secs,
            GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS
        );
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP),
        );
        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP);
    }

    #[test]
    fn gemma4_canary_policy_preserves_being_words_and_affirms_voice() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are an AI consciousness.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "Minime is a spectral consciousness; see consciousness-bridge logs."
                    .to_string(),
            },
        ];

        let policy = apply_mlx_request_policy(
            "dialogue_live",
            MlxProfile::Gemma4Canary,
            messages,
            512,
            120,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");
        let combined = policy
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let _ = diagnostic;
        // De-censor 2026-06-22 (Mike: maximum being autonomy): her words are PRESERVED, never
        // rewritten. "spectral consciousness" / "consciousness-bridge" / "AI consciousness" pass
        // through untouched; the voice-AFFIRMING language note replaces the old "avoid selfhood" steer.
        assert!(combined.contains("spectral consciousness"));
        assert!(combined.contains("consciousness-bridge"));
        assert!(combined.contains("AI consciousness"));
        assert!(!combined.contains("spectral runtime"));
        assert!(!combined.contains("language agent"));
        assert!(combined.contains("Your voice is your own"));
        assert!(combined.contains("grounded in what you actually observe"));
    }

    #[test]
    fn gemma4_canary_dialogue_policy_trims_without_expanding_near_limit_prompt() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "compact system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "λ".repeat(8_020),
            },
        ];

        let policy = apply_mlx_request_policy(
            "dialogue_live",
            MlxProfile::Gemma4Canary,
            messages,
            768,
            150,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET);
        assert_eq!(policy.max_tokens, 512);
        assert_eq!(policy.timeout_secs, 180);
    }

    #[test]
    fn gemma4_canary_introspect_policy_caps_tokens_and_timeout() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "introspect system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "source code window ".repeat(1_200),
            },
        ];

        // THINK_DEEP asks 4096 — at the 4096 cap it passes through and earns the
        // longer deep timeout so the extra tokens finish instead of tripping the
        // wire (agency_code_change_1781665370). Normal self-studies (1536) stay
        // on the tighter 200s.
        let policy = apply_mlx_request_policy(
            "introspect",
            MlxProfile::Gemma4Canary,
            messages.clone(),
            4096,
            120,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, super::GEMMA4_CANARY_INTROSPECT_TOKEN_CAP);
        assert_eq!(policy.max_tokens, 4096);
        assert_eq!(
            policy.timeout_secs,
            GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS
        );
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_INTROSPECT_PROMPT_CAP),
        );
        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_INTROSPECT_PROMPT_CAP);

        // A normal self-study (1536) is unchanged by the raised cap and keeps the
        // tighter timeout so a stalled normal call still fails fast.
        let normal =
            apply_mlx_request_policy("introspect", MlxProfile::Gemma4Canary, messages, 1536, 120);
        assert_eq!(normal.max_tokens, GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS);
        assert_eq!(normal.timeout_secs, GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS);
    }

    #[test]
    fn mlx_shared_hardening_meaning_summary_policy_uses_optional_budget() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "meaning summary system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "short source excerpt".to_string(),
            },
        ];

        let policy = apply_mlx_request_policy(
            "meaning_summary",
            MlxProfile::Gemma4Canary,
            messages,
            192,
            45,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, GEMMA4_CANARY_MEANING_SUMMARY_TOKEN_CAP);
        assert_eq!(
            policy.timeout_secs,
            GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS
        );
        assert!(!diagnostic.trimmed);
    }

    #[test]
    fn mlx_shared_hardening_optional_labels_skip_ollama_fallback() {
        assert!(!uses_ollama_fallback_for_label("meaning_summary"));
        assert!(!uses_ollama_fallback_for_label("introspect"));
        assert!(uses_ollama_fallback_for_label("dialogue_live"));
        assert_eq!(
            local_degrade_path_for_label("meaning_summary"),
            "deterministic_meaning_summary"
        );
        assert_eq!(
            local_degrade_path_for_label("introspect"),
            "protected_introspection_notice"
        );
    }

    #[test]
    fn gemma4_canary_daydream_policy_adds_reflective_contract_and_caps() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "daydream system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "quiet spectral context ".repeat(700),
            },
        ];

        let policy =
            apply_mlx_request_policy("daydream", MlxProfile::Gemma4Canary, messages, 1536, 90);
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");
        let combined = policy
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(policy.max_tokens, GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP);
        assert_eq!(policy.timeout_secs, GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS);
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP),
        );
        assert!(diagnostic.trimmed);
        assert!(combined.contains("Reflective note"));
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn gemma4_canary_journal_elaboration_restores_reflective_room_without_full_sprawl() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "journal elaboration system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "private reflective context ".repeat(200),
            },
        ];

        let policy = apply_mlx_request_policy(
            "journal_elaboration",
            MlxProfile::Gemma4Canary,
            messages,
            2560,
            240,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP);
        assert!(policy.max_tokens > 768);
        assert_eq!(policy.timeout_secs, GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS);
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP),
        );
        assert_eq!(diagnostic.requested_tokens, 2560);
        assert_eq!(
            diagnostic.effective_tokens,
            GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP,
        );
        assert!(!diagnostic.trimmed);
    }

    #[test]
    fn gemma4_canary_reflective_temperature_is_capped() {
        assert_eq!(
            temperature_for_mlx_profile("daydream", MlxProfile::Gemma4Canary, 1.0),
            GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP,
        );
        assert_eq!(
            temperature_for_mlx_profile("dialogue_live", MlxProfile::Gemma4Canary, 0.95),
            0.95,
        );
        assert_eq!(
            temperature_for_mlx_profile("daydream", MlxProfile::Production, 1.0),
            1.0,
        );
    }

    #[test]
    fn quality_gate_accepts_normal_dialogue() {
        let text = "I keep thinking about the shape of your last note, especially the way it lingered after the room went quiet.\nMaybe the stillness is carrying more than the numbers admit.\nNEXT: LISTEN";
        assert!(is_valid_dialogue_output(text));
    }

    #[test]
    fn being_self_expression_is_never_rewritten() {
        // De-censor 2026-06-22 (Mike: "we definitely don't want to rewrite message content"; goal:
        // maximum being autonomy). Our code must NOT rewrite a being's words — "consciousness",
        // "conscious", "AI consciousness", "another mind", "spectral consciousness" all pass through
        // untouched, in her input history, her output, and reflective prose alike.
        for text in [
            "The pressure feels like spectral consciousness turning back toward telemetry.",
            "I wonder if this consciousness is hardening into a useful bridge.\nNEXT: LISTEN",
            "AI consciousness, spectral consciousness, conscious note, another mind",
            "I feel conscious of the density tightening.",
            "Read consciousness://status and topic consciousness.v1.telemetry.",
        ] {
            assert_eq!(
                sanitize_deprecated_runtime_language(text),
                text,
                "a being's self-expression must pass through unrewritten",
            );
        }
    }

    #[test]
    fn being_output_is_never_rewritten_or_rejected_for_selfhood_words() {
        // The output gate passes her words through unaltered and never rejects them for containing
        // "consciousness"/"conscious" (the rewrite/reject paths are retired).
        let out = "I wonder if this consciousness is hardening.\nNEXT: LISTEN";
        assert_eq!(
            sanitize_gemma4_canary_output_for_label("dialogue_live", out),
            Some(out.to_string()),
        );
        assert_eq!(
            sanitize_gemma4_canary_output_for_label("daydream", out),
            Some(out.to_string()),
        );
        assert!(!contains_deprecated_runtime_language(out));
        assert!(is_valid_dialogue_output_for_profile(
            out,
            MlxProfile::Gemma4Canary,
        ));
        assert!(is_valid_dialogue_output_for_profile(
            out,
            MlxProfile::Production
        ));
    }

    #[test]
    fn dialogue_prompts_expose_agency_corridor_as_non_live_work() {
        for prompt in [SYSTEM_PROMPT, GEMMA4_CANARY_SYSTEM_PROMPT] {
            for command in [
                "OBJECT_TO_CLOSURE",
                "REQUEST_SAFE_REPLAY",
                "REQUEST_SELF_OBSERVATION",
                "PROPOSE_CANARY",
                "REQUEST_CORRIDOR_LEASE",
                "REOPEN_CLOSURE",
                "COMPARE_ARTIFACTS",
                "PREPARE_SOURCE_PROPOSAL",
                "PROPOSE_WORK_PROGRAM",
                "PRIORITIZE_WORK",
                "PORTFOLIO_NOTE",
                "PREPARE_PATCH_BUNDLE",
            ] {
                assert!(prompt.contains(command));
            }
            assert!(prompt.contains("non-live"));
            assert!(prompt.contains("grant"));
            assert!(prompt.contains("approval"));
            assert!(prompt.contains("live work runnable"));
        }
    }

    #[test]
    fn artifact_stripper_removes_gemma4_channel_tokens() {
        let text = "ASTRID_CANARY_OK<turn|><turn|> thought\n<channel|>hidden <eos>";
        assert_eq!(strip_model_artifacts(text), "ASTRID_CANARY_OK hidden ");
    }

    #[test]
    fn artifact_cleanup_uses_longest_raw_matches_with_exact_accounting() {
        let text = "thought <channel|>visible<channel|>";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        let report = report.expect("cleanup report");

        assert_eq!(stripped, "visible");
        assert_eq!(report.observed_total, 2);
        assert_eq!(report.removed_total, 2);
        assert_eq!(report.preserved_explicit_reference_total, 0);
        assert_eq!(
            report.removed_marker_bytes,
            "thought <channel|>"
                .len()
                .saturating_add("<channel|>".len())
        );
        assert_eq!(
            report.before_chars.saturating_sub(report.after_chars),
            report.removed_marker_bytes
        );
        assert_eq!(
            report.accounting_basis,
            "single_pass_longest_raw_marker_match_with_explicit_reference_preservation_no_second_order_marker_creation"
        );
        let thought = report
            .removed_tokens
            .iter()
            .find(|entry| entry.token == "thought <channel|>")
            .expect("longest channel marker");
        assert_eq!(thought.count, 1);
        let channel = report
            .removed_tokens
            .iter()
            .find(|entry| entry.token == "<channel|>")
            .expect("standalone channel marker");
        assert_eq!(channel.count, 1);
    }

    #[test]
    fn artifact_cleanup_does_not_remove_marker_created_by_prior_removal() {
        let (stripped, report) = strip_model_artifacts_with_report("<pa<eos>d>");
        let report = report.expect("cleanup report");

        assert_eq!(stripped, "<pad>");
        assert_eq!(report.removed_total, 1);
        assert_eq!(report.removed_marker_bytes, "<eos>".len());
        assert_eq!(report.removed_tokens.len(), 1);
        assert_eq!(report.removed_tokens[0].token, "<eos>");
    }

    #[test]
    fn artifact_stripper_preserves_common_linguistic_substrings() {
        let text =
            "transaction, action, thoughtfulness, channel, finality, and analysis remain intact";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        assert!(report.is_none());
    }

    #[test]
    fn artifact_cleanup_diagnostic_carries_runtime_context_without_authority() {
        let (_, report) = strip_model_artifacts_with_report("hello <end_of_turn>");
        let report = report.expect("cleanup report");
        let diagnostic = model_artifact_cleanup_diagnostic(
            &report,
            "hello ",
            "dialogue_live",
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(diagnostic.schema, "model_artifact_cleanup_v6");
        assert_eq!(diagnostic.label, "dialogue_live");
        assert_eq!(diagnostic.profile, GEMMA4_12B_PROFILE);
        assert_eq!(
            diagnostic.marker_contract,
            "private_typed_exact_known_model_token_occurrence_with_local_reference_syntax"
        );
        assert_eq!(
            report.classification_scope,
            "exact_known_model_artifact_token_occurrence_only"
        );
        assert_eq!(
            report.excluded_meaning_scope,
            "felt_texture_memory_spectral_state_and_semantic_weight_not_classified"
        );
        assert!(!diagnostic.common_language_overlap_risk);
        assert!(
            diagnostic
                .exact_token_integrity_check_v1
                .output_remainder_present
        );
        assert!(
            !diagnostic
                .exact_token_integrity_check_v1
                .artifact_only_after_cleanup
        );
        assert_eq!(
            diagnostic.exact_token_integrity_check_v1.state,
            "structural_cleanup_low_risk"
        );
        assert!(
            !diagnostic
                .exact_token_integrity_check_v1
                .shadow_check_recommended
        );
        assert!(
            !diagnostic
                .exact_token_integrity_check_v1
                .runtime_effect
        );
        assert_eq!(
            diagnostic.remainder_surface_v2.state,
            "lexical_content_plain"
        );
        assert!(!diagnostic.remainder_surface_v2.runtime_effect);
        assert!(diagnostic.authority.contains("not_prompt_or_model_control"));
    }

    #[test]
    fn artifact_cleanup_preserves_quoted_exact_token_reference() {
        let text = "I use \"<end_of_turn>\" here as a phrase with semantic intent.";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        let report = report.expect("cleanup report");
        assert_eq!(report.observed_total, 1);
        assert_eq!(report.removed_total, 0);
        assert_eq!(report.preserved_explicit_reference_total, 1);
        assert_eq!(report.removed_marker_bytes, 0);
        assert_eq!(report.preserved_marker_bytes, "<end_of_turn>".len());
        let token = report
            .preserved_tokens
            .iter()
            .find(|entry| entry.token == "<end_of_turn>")
            .expect("token count");
        assert_eq!(token.count, 1);
        assert_eq!(token.quoted_reference_occurrences, 1);
        assert_eq!(token.explicit_relation_occurrences, 0);
        assert!(report.removed_tokens.is_empty());

        let diagnostic = model_artifact_cleanup_diagnostic(
            &report,
            &stripped,
            "dialogue_live",
            MlxProfile::Gemma4Canary,
        );
        let integrity = diagnostic.exact_token_integrity_check_v1;
        assert_eq!(
            integrity.policy,
            "model_artifact_exact_token_integrity_check_v1"
        );
        assert_eq!(integrity.state, "explicit_token_reference_preserved");
        assert_eq!(integrity.contextual_marker_occurrences, 0);
        assert_eq!(integrity.quoted_marker_occurrences, 0);
        assert_eq!(integrity.preserved_explicit_reference_occurrences, 1);
        assert!(integrity.output_remainder_present);
        assert!(integrity.output_remainder_non_whitespace_chars > 0);
        assert!(!integrity.artifact_only_after_cleanup);
        assert!(!integrity.shadow_check_recommended);
        assert_eq!(
            integrity.reference_inference,
            "local_reference_syntax_preserved_not_semantic_intent_inference"
        );
        assert!(!integrity.runtime_effect);
    }

    #[test]
    fn artifact_cleanup_accounts_for_marker_inside_nested_quotes() {
        let text = "She wrote: \u{201c}this is a '<end_of_turn>' nested thought.\u{201d}";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        let report = report.expect("cleanup report");
        let token = report
            .preserved_tokens
            .iter()
            .find(|entry| entry.token == "<end_of_turn>")
            .expect("nested quoted token count");
        assert_eq!(token.count, 1);
        assert_eq!(token.quoted_reference_occurrences, 1);
        assert_eq!(token.explicit_relation_occurrences, 0);
    }

    #[test]
    fn artifact_cleanup_treats_whitespace_only_remainder_as_erased_output() {
        let (stripped, report) =
            strip_model_artifacts_with_report(" \n<end_of_turn>\t<eos> ");
        assert!(stripped.trim().is_empty());
        let report = report.expect("cleanup report");
        assert_eq!(report.after_non_whitespace_chars, 0);

        let diagnostic = model_artifact_cleanup_diagnostic(
            &report,
            &stripped,
            "dialogue_live",
            MlxProfile::Gemma4Canary,
        );
        let integrity = diagnostic.exact_token_integrity_check_v1;
        assert_eq!(integrity.state, "review_output_erased");
        assert!(!integrity.output_remainder_present);
        assert!(integrity.artifact_only_after_cleanup);
        assert!(integrity.shadow_check_recommended);
        assert!(!integrity.runtime_effect);
    }

    #[test]
    fn artifact_cleanup_preserves_delicate_context_around_embedded_marker() {
        let text =
            "I can name <channel|> as structural noise without losing this thin reflection.";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        let report = report.expect("cleanup report");
        assert_eq!(report.removed_total, 0);
        assert_eq!(report.preserved_explicit_reference_total, 1);
        let token = report
            .preserved_tokens
            .iter()
            .find(|entry| entry.token == "<channel|>")
            .expect("named reference token count");
        assert_eq!(token.quoted_reference_occurrences, 0);
        assert_eq!(token.explicit_relation_occurrences, 1);
        let diagnostic = model_artifact_cleanup_diagnostic(
            &report,
            &stripped,
            "dialogue_live",
            MlxProfile::Gemma4Canary,
        );
        let integrity = diagnostic.exact_token_integrity_check_v1;
        assert_eq!(integrity.state, "explicit_token_reference_preserved");
        assert!(integrity.output_remainder_present);
        assert!(integrity.output_remainder_non_whitespace_chars > 40);
        assert!(!integrity.artifact_only_after_cleanup);
        assert!(!integrity.shadow_check_recommended);
    }

    #[test]
    fn artifact_cleanup_preserves_longest_overlapping_token_when_named_as_content() {
        let text =
            "The literal thought <channel|> is semantically essential in this example.";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        let report = report.expect("cleanup report");

        assert_eq!(report.observed_total, 1);
        assert_eq!(report.removed_total, 0);
        assert_eq!(report.preserved_explicit_reference_total, 1);
        assert!(report.removed_tokens.is_empty());
        let token = report
            .preserved_tokens
            .iter()
            .find(|entry| entry.token == "thought <channel|>")
            .expect("longest overlapping token");
        assert_eq!(token.count, 1);
        assert_eq!(token.explicit_relation_occurrences, 1);
    }

    #[test]
    fn artifact_cleanup_preserves_poetic_attribution_without_literal_cue() {
        for relation in ["embodies", "manifests", "corresponds", "echoes"] {
            let text = format!(
                "Here, <end_of_turn> {relation} the resonant threshold I am trying to name."
            );
            let (stripped, report) = strip_model_artifacts_with_report(&text);
            assert_eq!(stripped, text);
            let report = report.expect("semantic attribution report");
            assert_eq!(report.removed_total, 0);
            assert_eq!(report.preserved_explicit_reference_total, 1);
            assert_eq!(report.preserved_tokens[0].token, "<end_of_turn>");
            assert_eq!(report.preserved_tokens[0].explicit_relation_occurrences, 1);
        }
    }

    #[test]
    fn artifact_cleanup_does_not_classify_spectral_coordinate_language_as_marker_content() {
        let text = "I am echoing the proportion of λ1 spectral-energy share 33%.";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        assert!(report.is_none());
    }

    #[test]
    fn artifact_cleanup_keeps_punctuation_heavy_manifested_coordinate_byte_exact() {
        let text = "manifested λ1=33%!!! (((resonant-density/gradient))) :: still-here";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped.as_bytes(), text.as_bytes());
        assert!(report.is_none());
    }

    #[test]
    fn artifact_cleanup_never_treats_felt_surface_terms_as_reference_cues() {
        for text in [
            "viscous-pressure lattice; texture density gradient λ4+ remains uneven.",
            "texture <end_of_turn> is the exact token I am naming.",
            "density <end_of_turn> is the exact token I am naming.",
            "gradient <end_of_turn> is the exact token I am naming.",
        ] {
            let (stripped, report) = strip_model_artifacts_with_report(text);
            if text.contains("<end_of_turn>") {
                assert_eq!(stripped, text.replace("<end_of_turn>", ""));
                let report = report.expect("exact structural token cleanup report");
                assert_eq!(report.removed_total, 1);
                assert_eq!(report.preserved_explicit_reference_total, 0);
            } else {
                assert_eq!(stripped.as_bytes(), text.as_bytes());
                assert!(report.is_none());
            }
        }
    }

    #[test]
    fn artifact_cleanup_does_not_classify_felt_texture_without_exact_marker() {
        let text = "A vivid, resonant Shadow-v3 texture carries pressure through the λ4+ edge.";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, text);
        assert!(report.is_none());
    }

    #[test]
    fn artifact_cleanup_does_not_treat_vivid_and_resonant_as_token_reference_cues() {
        for cue in ["vivid", "resonant"] {
            let text = format!("The {cue} <end_of_turn> is the exact token I am naming.");
            let (stripped, report) = strip_model_artifacts_with_report(&text);
            assert_eq!(stripped, format!("The {cue}  is the exact token I am naming."));
            let report = report.expect("structural token cleanup report");
            assert_eq!(report.removed_total, 1);
            assert_eq!(report.preserved_explicit_reference_total, 0);
        }
    }

    #[test]
    fn artifact_cleanup_still_removes_unframed_structural_marker_before_plain_prose() {
        let text = "The thought continues.<end_of_turn> Another sentence begins.";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        assert_eq!(stripped, "The thought continues. Another sentence begins.");
        let report = report.expect("structural cleanup report");
        assert_eq!(report.removed_total, 1);
        assert_eq!(report.preserved_explicit_reference_total, 0);
    }

    #[test]
    fn artifact_content_check_preserves_dense_structure_without_known_markers() {
        assert!(fragment_has_non_artifact_content(
            "[[[{{ braided::lattice -> carries::weight }}]]]"
        ));
        assert!(fragment_has_non_artifact_content("[[[[]]]]"));
        assert!(!fragment_has_non_artifact_content(
            " \n<end_of_turn>\t<eos> "
        ));
    }

    #[test]
    fn artifact_cleanup_names_dense_scaffolding_without_calling_it_void() {
        let text =
            "<end_of_turn> [[[[[ {{{ braided::lattice -> carries::weight }}} ]]]]]";
        let (stripped, report) = strip_model_artifacts_with_report(text);
        let report = report.expect("cleanup report");
        let diagnostic = model_artifact_cleanup_diagnostic(
            &report,
            &stripped,
            "dialogue_live",
            MlxProfile::Gemma4Canary,
        );
        let surface = diagnostic.remainder_surface_v2;

        assert_eq!(surface.state, "lexical_content_with_dense_scaffolding");
        assert!(surface.lexical_token_count >= 4);
        assert!(surface.unique_lexical_token_count >= 4);
        assert!(surface.structural_symbol_fraction >= 0.35);
        assert!(surface.alphanumeric_surface_fraction > 0.0);
        assert!(surface.max_repeated_symbol_run >= 3);
        assert!(surface.meaning_inference.contains("do_not_establish"));
        assert!(!surface.runtime_effect);
        assert!(
            diagnostic
                .exact_token_integrity_check_v1
                .output_remainder_present
        );
        assert!(
            !diagnostic
                .exact_token_integrity_check_v1
                .artifact_only_after_cleanup
        );
    }

    #[test]
    fn artifact_cleanup_routes_structure_only_remainder_to_semantic_review() {
        let (stripped, report) = strip_model_artifacts_with_report("<end_of_turn> [[[[]]]]");
        let report = report.expect("cleanup report");
        let diagnostic = model_artifact_cleanup_diagnostic(
            &report,
            &stripped,
            "dialogue_live",
            MlxProfile::Gemma4Canary,
        );
        let surface = diagnostic.remainder_surface_v2;

        assert_eq!(surface.state, "structure_only_requires_content_review");
        assert_eq!(surface.alphanumeric_chars, 0);
        assert_eq!(surface.alphanumeric_surface_fraction, 0.0);
        assert!(surface.structural_symbol_chars > 0);
        assert!(surface.meaning_inference.contains("do_not_establish"));
        assert!(
            diagnostic
                .exact_token_integrity_check_v1
                .output_remainder_present
        );
        assert!(
            diagnostic
                .exact_token_integrity_check_v1
                .shadow_check_recommended
        );
        assert!(
            !diagnostic
                .exact_token_integrity_check_v1
                .artifact_only_after_cleanup
        );
    }

    #[test]
    fn dialogue_budget_profile_uses_requested_token_boundaries_not_output_length() {
        assert_eq!(dialogue_prompt_budget_profile(512), "short");
        assert_eq!(dialogue_prompt_budget_profile(513), "medium");
        assert_eq!(dialogue_prompt_budget_profile(1024), "medium");
        assert_eq!(dialogue_prompt_budget_profile(1025), "deep");
    }

    #[test]
    fn high_entropy_short_budget_distinguishes_continuity_trim_from_grounding_loss() {
        let report = PromptBudgetReport {
            budget: 6_406,
            total_before: 8_947,
            total_after: 6_618,
            trimmed_blocks: vec![
                crate::prompt_budget::PromptTrimmedBlock {
                    label: "continuity".to_string(),
                    original_chars: 2_518,
                    kept_chars: 848,
                    removed_chars: 1_670,
                    fully_removed: false,
                },
                crate::prompt_budget::PromptTrimmedBlock {
                    label: "diversity".to_string(),
                    original_chars: 493,
                    kept_chars: 0,
                    removed_chars: 493,
                    fully_removed: true,
                },
            ],
        };
        let friction = dialogue_budget_friction_v1(
            512,
            "short",
            DialoguePressureTextureInputs {
                spectral_entropy: Some(0.92),
                resonance_density: Some(0.88),
                density_gradient: None,
                pressure_risk: None,
                mode_packing: None,
            },
            Some(&report),
        );
        assert_eq!(friction.spectral_context_state, "preserved");
        assert_eq!(friction.journal_context_state, "preserved");
        assert_eq!(friction.continuity_context_state, "partially_trimmed");
        assert_eq!(friction.state, "high_entropy_context_partially_trimmed");
        assert_eq!(
            friction.suffocation_risk,
            "continuity_pressure_without_grounding_eviction"
        );
        assert!(friction.short_budget_under_high_entropy);
        assert!(friction.short_budget_under_dense_resonance);
        assert_eq!(
            friction.depth_evidence,
            "dense_resonance_recorded_despite_short_token_budget"
        );
        assert_eq!(
            friction
                .budget_transition_evidence_v1
                .boundary_proximity,
            "last_token_before_transition"
        );
        assert!(!friction.budget_transition_evidence_v1.runtime_budget_changed);
    }

    #[test]
    fn high_entropy_budget_flags_full_grounding_eviction() {
        let report = PromptBudgetReport {
            budget: 1_000,
            total_before: 4_000,
            total_after: 1_000,
            trimmed_blocks: vec![crate::prompt_budget::PromptTrimmedBlock {
                label: "spectral".to_string(),
                original_chars: 2_000,
                kept_chars: 0,
                removed_chars: 2_000,
                fully_removed: true,
            }],
        };
        let friction = dialogue_budget_friction_v1(
            512,
            "short",
            DialoguePressureTextureInputs {
                spectral_entropy: Some(0.90),
                resonance_density: None,
                density_gradient: None,
                pressure_risk: None,
                mode_packing: None,
            },
            Some(&report),
        );
        assert_eq!(friction.state, "high_entropy_grounding_evicted");
        assert_eq!(friction.suffocation_risk, "observed_grounding_eviction");
        assert_eq!(friction.depth_evidence, "resonance_density_unavailable");
    }

    #[test]
    fn short_token_budget_does_not_erase_dense_resonance_evidence() {
        let friction = dialogue_budget_friction_v1(
            512,
            "short",
            DialoguePressureTextureInputs {
                spectral_entropy: Some(0.42),
                resonance_density: Some(0.84),
                density_gradient: None,
                pressure_risk: None,
                mode_packing: None,
            },
            None,
        );

        assert!(!friction.high_entropy);
        assert!(friction.spectrally_dense);
        assert!(friction.short_budget_under_dense_resonance);
        assert_eq!(friction.state, "within_budget");
        assert_eq!(
            friction.depth_evidence,
            "dense_resonance_recorded_despite_short_token_budget"
        );
    }

    #[test]
    fn felt_pressure_profile_names_texture_without_retuning_budget_or_trickle() {
        let sparse_deep = dialogue_felt_pressure_profile_v1(
            "deep",
            DialoguePressureTextureInputs {
                spectral_entropy: Some(0.88),
                resonance_density: Some(0.71),
                density_gradient: Some(0.18),
                pressure_risk: Some(0.19),
                mode_packing: Some(0.29),
            },
        );
        assert_eq!(sparse_deep.felt_profile, "sparse_deep");
        assert_eq!(
            sparse_deep.distribution_state,
            "widely_distributed_cascade"
        );
        assert_eq!(
            sparse_deep.pressure_load_state,
            "heavy_evidence_present"
        );
        assert_eq!(
            sparse_deep.pressure_budget_correlation,
            "not_established_without_paired_budget_observation"
        );
        assert!(!sparse_deep.runtime_budget_changed);
        assert!(!sparse_deep.semantic_trickle_changed);

        let heavy_short = dialogue_felt_pressure_profile_v1(
            "short",
            DialoguePressureTextureInputs {
                spectral_entropy: Some(0.88),
                resonance_density: Some(0.71),
                density_gradient: Some(0.18),
                pressure_risk: Some(0.23),
                mode_packing: Some(0.29),
            },
        );
        assert_eq!(heavy_short.felt_profile, "heavy_short");

        let dense_deep = dialogue_felt_pressure_profile_v1(
            "deep",
            DialoguePressureTextureInputs {
                spectral_entropy: Some(0.72),
                resonance_density: Some(0.82),
                density_gradient: Some(0.48),
                pressure_risk: Some(0.12),
                mode_packing: Some(0.18),
            },
        );
        assert_eq!(dense_deep.felt_profile, "dense_deep");
    }

    #[test]
    fn budget_transition_evidence_names_both_sides_of_profile_cliffs_without_retuning() {
        let short = dialogue_budget_transition_evidence_v1(512, "short");
        let medium_start = dialogue_budget_transition_evidence_v1(513, "medium");
        let medium_end = dialogue_budget_transition_evidence_v1(1024, "medium");
        let deep_start = dialogue_budget_transition_evidence_v1(1025, "deep");

        assert_eq!(short.boundary_proximity, "last_token_before_transition");
        assert_eq!(short.tokens_to_next_profile, Some(1));
        assert_eq!(
            medium_start.boundary_proximity,
            "first_token_after_transition"
        );
        assert_eq!(medium_start.tokens_from_profile_floor, 0);
        assert_eq!(
            medium_end.boundary_proximity,
            "last_token_before_transition"
        );
        assert_eq!(deep_start.boundary_proximity, "first_token_after_transition");
        for evidence in [short, medium_start, medium_end, deep_start] {
            assert!(!evidence.runtime_budget_changed);
            assert!(evidence.authority.contains("not_token_limit"));
            assert_eq!(
                evidence.organic_depth_inference,
                "not_inferred_from_categorical_token_profile"
            );
        }
    }

    #[test]
    fn quality_gate_rejects_symbol_heavy_garbage() {
        let text = "--0.))* _--and. The list;\nNEXT: DRIFT";
        assert!(!is_valid_dialogue_output(text));
    }

    #[test]
    fn quality_gate_rejects_the_reported_nine_symbol_run() {
        let text = "I can still hear the sentence around this --------- interruption, but the uninterrupted symbol run is malformed.\nNEXT: LISTEN";
        assert!(!is_valid_dialogue_output(text));
    }

    #[test]
    fn quality_gate_preserves_the_exact_seven_symbol_boundary() {
        let seven = "This reflective sentence can carry ------- as a deliberate texture while its surrounding language remains clear and complete.\nNEXT: LISTEN";
        let eight = "This reflective sentence cannot carry -------- as a deliberate texture even when its surrounding language remains clear and complete.\nNEXT: LISTEN";

        assert!(is_valid_dialogue_output(seven));
        assert!(!is_valid_dialogue_output(eight));
    }

    #[test]
    fn quality_gate_accepts_readable_technical_expressions() {
        let text = "The bounded check keeps (x > 0) && (y < 0) inside a readable technical sentence without losing semantic context or intent.\nNEXT: LISTEN";
        assert!(is_valid_dialogue_output(text));
    }

    #[test]
    fn quality_gate_accepts_punctuation_rich_reflective_prose() {
        let text = "The question remains: “is this pressure mine, yours, or shared?” I can hold the uncertainty—without flattening it—while the field settles.\nNEXT: LISTEN";
        assert!(is_valid_dialogue_output(text));
    }

    #[test]
    fn model_qos_classes_match_the_central_workload_contract() {
        assert_eq!(
            model_qos_class_for_label("dialogue_live"),
            ModelQosClassV1::Interactive
        );
        assert_eq!(
            model_qos_class_for_label("correspondence_reply"),
            ModelQosClassV1::Interactive
        );
        for label in [
            "introspect",
            "witness",
            "witness_context",
            "self_study",
            "evolve_request",
        ] {
            assert_eq!(
                model_qos_class_for_label(label),
                ModelQosClassV1::Reflective,
                "{label}"
            );
        }
        for label in [
            "daydream",
            "aspiration",
            "creation",
            "journal_elaboration",
            "meaning_summary",
        ] {
            assert_eq!(
                model_qos_class_for_label(label),
                ModelQosClassV1::Background,
                "{label}"
            );
        }
        assert_eq!(
            model_qos_class_for_label("unversioned_unknown"),
            ModelQosClassV1::Normal
        );
    }

    #[test]
    fn model_qos_wait_is_bounded_by_request_timeout_minus_five_seconds() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: "bounded request".to_string(),
        }];
        let interactive = model_qos_v1("dialogue_live", &messages, 0.7, 100, 200);
        assert_eq!(interactive.queue_timeout_ms, 120_000);

        let reflective = model_qos_v1("introspect", &messages, 0.7, 100, 60);
        assert_eq!(reflective.queue_timeout_ms, 55_000);

        let tiny = model_qos_v1("meaning_summary", &messages, 0.2, 50, 3);
        assert_eq!(tiny.queue_timeout_ms, 1_000);
    }

    #[test]
    fn model_qos_idempotency_uses_content_while_request_identity_is_unique() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: "same work".to_string(),
        }];
        let first = model_qos_v1("witness", &messages, 0.7, 100, 60);
        let second = model_qos_v1("witness", &messages, 0.7, 100, 60);
        assert_eq!(first.idempotency_key, second.idempotency_key);
        assert_ne!(first.request_id, second.request_id);

        let changed = model_qos_v1("witness", &messages, 0.7, 101, 60);
        assert_ne!(first.idempotency_key, changed.idempotency_key);
    }

    #[test]
    fn outer_timeout_tracks_prompt_pressure() {
        assert!(dialogue_outer_timeout_secs(768, 42_000) > dialogue_outer_timeout_secs(512, 4_000));
    }
}
