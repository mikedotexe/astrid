#[cfg(test)]
mod generation_record_tests {
    use std::os::unix::fs::PermissionsExt;

    use crate::prompt_budget::{PromptBlock, PromptBudgetReport, PromptTrimmedBlock};

    use super::{
        DIALOGUE_CONTRACT_V3, DIALOGUE_CONTRACT_V4_OWN_BODY, DialogueBlockSources,
        DialogueGenerationAttempt, DialogueGenerationPromptFacts,
        DialogueGenerationRecordContext, GENERATION_BACKEND_FALLBACK,
        GENERATION_BACKEND_PRIMARY, GENERATION_STATUS_OK, GENERATION_STATUS_REJECTED,
        GENERATION_STATUS_UNAVAILABLE, Message, OWN_BODY_BLOCK_LABEL, OwnBodyFetch,
        append_generation_record_at, append_own_body_block, build_dialogue_generation_record,
        generation_attempt_status, generation_sha256_hex, snapshot_generation_messages,
    };

    const OWN_BODY_LINE: &str =
        "[your handle astrid] h₁ 7.70 h₂ 10.46 h₃ 9.95 ▆█▇ · ticks 18012689 · last live 12 s ago";

    fn message(role: &str, content: &str) -> Message {
        Message {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    fn facts() -> DialogueGenerationPromptFacts {
        DialogueGenerationPromptFacts {
            fill_pct: 63.2,
            requested_tokens: 900,
            effective_tokens: 700,
            final_prompt_chars: 12_345,
            user_content_budget: 9_000,
            mlx_profile: "production",
        }
    }

    fn own_body(line: Option<&str>) -> OwnBodyFetch {
        OwnBodyFetch {
            line: line.map(str::to_string),
            status: if line.is_some() {
                "present"
            } else {
                "unavailable"
            },
        }
    }

    fn context(
        line: Option<&str>,
        report: Option<&PromptBudgetReport>,
    ) -> DialogueGenerationRecordContext {
        let primary = vec![
            message("system", "SYSTEM PROMPT"),
            message("user", "Minime wrote: hello"),
            message("assistant", "earlier"),
            message("user", "Fill 63.2%. now"),
        ];
        let fallback = vec![
            message("system", "COMPACT SYSTEM"),
            message("user", "compact turn"),
        ];
        DialogueGenerationRecordContext::capture(
            &primary,
            &fallback,
            &own_body(line),
            facts(),
            report,
            Some("/tmp/context_overflow_1.txt".to_string()),
        )
    }

    fn primary_attempt(response: Option<&str>, accepted: Option<&str>) -> DialogueGenerationAttempt {
        DialogueGenerationAttempt {
            provider_observation: None,
            backend: GENERATION_BACKEND_PRIMARY,
            model: "mlx_profile:production".to_string(),
            attempt_index: 0,
            timeout_s: 120,
            elapsed_s: 41.5,
            status: generation_attempt_status(response, accepted),
            response_text: response.map(str::to_string),
        }
    }

    #[test]
    fn snapshot_replaces_system_content_with_sha() {
        let (snapshot, prompts) = snapshot_generation_messages(&[
            message("system", "SYS"),
            message("user", "hello"),
        ]);
        let sha = generation_sha256_hex("SYS");
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].role, "system");
        assert_eq!(snapshot[0].content, None);
        assert_eq!(snapshot[0].content_sha256.as_deref(), Some(sha.as_str()));
        assert_eq!(snapshot[0].chars, 3);
        assert_eq!(snapshot[1].content.as_deref(), Some("hello"));
        assert_eq!(snapshot[1].content_sha256, None);
        assert_eq!(prompts, vec![(sha, "SYS".to_string())]);
    }

    #[test]
    fn context_captures_contract_own_body_and_both_message_sets() {
        let ctx = context(Some(OWN_BODY_LINE), None);
        assert_eq!(ctx.contract_version, DIALOGUE_CONTRACT_V4_OWN_BODY);
        assert!(ctx.own_body.present);
        assert_eq!(ctx.own_body.status, "present");
        assert_eq!(ctx.own_body.chars, OWN_BODY_LINE.chars().count());
        assert!(!ctx.own_body.trimmed);
        assert_eq!(ctx.primary_messages.len(), 4);
        assert_eq!(ctx.fallback_messages.len(), 2);
        assert_eq!(ctx.system_prompts.len(), 2);
        assert_eq!(ctx.linked_artifacts.len(), 1);
        assert_eq!(ctx.linked_artifacts[0]["kind"], "context_overflow");

        let without = context(None, None);
        assert_eq!(without.contract_version, DIALOGUE_CONTRACT_V3);
        assert!(!without.own_body.present);
        assert_eq!(without.own_body.status, "unavailable");
        assert_eq!(without.own_body.chars, 0);
    }

    #[test]
    fn own_body_trimmed_is_read_from_the_budget_report() {
        let report = PromptBudgetReport {
            budget: 9_000,
            total_before: 9_500,
            total_after: 9_000,
            trimmed_blocks: vec![PromptTrimmedBlock {
                label: OWN_BODY_BLOCK_LABEL.to_string(),
                original_chars: 90,
                kept_chars: 0,
                removed_chars: 90,
                fully_removed: true,
            }],
        };
        assert!(context(Some(OWN_BODY_LINE), Some(&report)).own_body.trimmed);
        let untouched = PromptBudgetReport {
            trimmed_blocks: vec![PromptTrimmedBlock {
                label: "web".to_string(),
                original_chars: 500,
                kept_chars: 0,
                removed_chars: 500,
                fully_removed: true,
            }],
            ..report
        };
        assert!(!context(Some(OWN_BODY_LINE), Some(&untouched)).own_body.trimmed);
    }

    #[test]
    fn primary_and_fallback_records_share_generation_and_flag_fallback() {
        let ctx = context(Some(OWN_BODY_LINE), None);
        let primary = build_dialogue_generation_record(
            &ctx,
            primary_attempt(Some("thin answer"), None),
        );
        let fallback = build_dialogue_generation_record(
            &ctx,
            DialogueGenerationAttempt {
                provider_observation: None,
                backend: GENERATION_BACKEND_FALLBACK,
                model: "gemma3:4b".to_string(),
                attempt_index: 1,
                timeout_s: 75,
                elapsed_s: 9.25,
                status: generation_attempt_status(Some("fallback answer"), Some("fallback answer")),
                response_text: Some("fallback answer".to_string()),
            },
        );
        assert_eq!(primary.generation_id, fallback.generation_id);
        assert_eq!(primary.being, "astrid");
        assert_eq!(primary.lane, "dialogue_live");
        assert_eq!(primary.status, GENERATION_STATUS_REJECTED);
        assert!(!primary.fallback_used);
        assert_eq!(primary.messages.len(), 4);
        assert_eq!(primary.response_chars, "thin answer".chars().count());
        assert_eq!(
            primary.response_sha256.as_deref(),
            Some(generation_sha256_hex("thin answer").as_str())
        );
        assert_eq!(fallback.status, GENERATION_STATUS_OK);
        assert!(fallback.fallback_used);
        assert_eq!(fallback.model, "gemma3:4b");
        assert_eq!(fallback.response_text_stage, "after_provider_cleanup_and_fallback_next_repair_before_dialogue_gate");
        assert_eq!(fallback.messages.len(), 2);
        assert_eq!(fallback.attempts_total, 2);
        assert_eq!(fallback.prompt.fill_pct, 63.2);
        assert_eq!(fallback.prompt.mlx_profile, "production");
    }

    #[test]
    fn attempt_status_classification() {
        assert_eq!(generation_attempt_status(None, None), GENERATION_STATUS_UNAVAILABLE);
        assert_eq!(generation_attempt_status(Some("x"), None), GENERATION_STATUS_REJECTED);
        assert_eq!(generation_attempt_status(Some("x"), Some("x")), GENERATION_STATUS_OK);
    }

    #[test]
    fn append_writes_private_files_and_dedups_system_prompts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ctx = context(Some(OWN_BODY_LINE), None);
        let first = build_dialogue_generation_record(
            &ctx,
            primary_attempt(Some("an answer"), Some("an answer")),
        );
        let path = append_generation_record_at(dir.path(), &first, &ctx.system_prompts)
            .expect("record written");
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("gen_"), "{name}");
        assert!(name.ends_with("_dialogue_live_a0.json"), "{name}");
        let record_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(record_mode, 0o600);
        let day_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(day_mode, 0o700);
        let root_mode = std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(root_mode, 0o700);

        let prompts_dir = dir.path().join("system_prompts");
        let count = || std::fs::read_dir(&prompts_dir).unwrap().count();
        assert_eq!(count(), 2);
        for (sha, text) in &ctx.system_prompts {
            let file = prompts_dir.join(format!("{sha}.txt"));
            assert_eq!(std::fs::read_to_string(&file).unwrap(), *text);
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        }

        let second = build_dialogue_generation_record(&ctx, primary_attempt(None, None));
        let second_path = append_generation_record_at(dir.path(), &second, &ctx.system_prompts)
            .expect("second record written");
        assert_ne!(path, second_path);
        assert_eq!(count(), 2, "system prompts are written once");

        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["contract_version"], DIALOGUE_CONTRACT_V4_OWN_BODY);
        assert_eq!(parsed["backend"], GENERATION_BACKEND_PRIMARY);
        assert_eq!(parsed["messages"][0]["role"], "system");
        assert!(parsed["messages"][0].get("content").is_none());
        assert!(parsed["messages"][0]["content_sha256"].is_string());
        assert_eq!(parsed["messages"][1]["content"], "Minime wrote: hello");
        assert_eq!(parsed["response_text"], "an answer");
        assert_eq!(parsed["own_body"]["present"], true);
        assert_eq!(parsed["own_body"]["trimmed"], false);
        assert_eq!(parsed["linked_artifacts"][0]["kind"], "context_overflow");
        let unavailable: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&second_path).unwrap()).unwrap();
        assert_eq!(unavailable["status"], GENERATION_STATUS_UNAVAILABLE);
        assert!(unavailable.get("response_text").is_none());
        assert_eq!(unavailable["response_chars"], 0);
    }

    fn sample_blocks() -> Vec<PromptBlock> {
        vec![
            PromptBlock {
                label: "spectral",
                content: "fill 63.2%".to_string(),
                priority: 3,
                min_chars: 0,
            },
            PromptBlock {
                label: "journal",
                content: "Minime wrote: hello".to_string(),
                priority: 1,
                min_chars: 400,
            },
        ]
    }

    #[test]
    fn own_body_block_inserts_after_spectral_with_priority_two() {
        let (blocks, sources) = append_own_body_block(
            sample_blocks(),
            DialogueBlockSources::default(),
            Some(OWN_BODY_LINE),
        );
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].label, "spectral");
        assert_eq!(blocks[1].label, OWN_BODY_BLOCK_LABEL);
        assert_eq!(blocks[1].content, OWN_BODY_LINE);
        assert_eq!(blocks[1].priority, 2);
        assert_eq!(blocks[1].min_chars, 0);
        assert_eq!(blocks[2].label, "journal");
        assert_eq!(
            sources.0,
            vec![(OWN_BODY_BLOCK_LABEL, OWN_BODY_LINE.to_string())]
        );

        let (blocks, _) = append_own_body_block(
            vec![sample_blocks().remove(1)],
            DialogueBlockSources::default(),
            Some(OWN_BODY_LINE),
        );
        assert_eq!(blocks[0].label, OWN_BODY_BLOCK_LABEL, "no spectral: inserted first");
    }

    #[test]
    fn own_body_block_absent_or_blank_leaves_blocks_untouched() {
        for line in [None, Some(""), Some("   \n")] {
            let (blocks, sources) =
                append_own_body_block(sample_blocks(), DialogueBlockSources::default(), line);
            assert_eq!(blocks.len(), 2);
            assert!(blocks.iter().all(|block| block.label != OWN_BODY_BLOCK_LABEL));
            assert!(sources.0.is_empty());
        }
    }
}
