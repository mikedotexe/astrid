fn append_llm_diagnostic_jsonl(file_name: &str, value: &impl Serialize) {
    let dir = bridge_paths().bridge_workspace().join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(file_name);
    let Ok(line) = serde_json::to_string(value) else {
        return;
    };
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return,
    };
    use std::io::Write as _;
    let _ = writeln!(file, "{line}");
}

fn dialogue_prompt_budget_profile(num_predict: u32) -> &'static str {
    if num_predict > 1024 {
        "deep"
    } else if num_predict > 512 {
        "medium"
    } else {
        "short"
    }
}

fn fragment_has_non_artifact_content(fragment: &str) -> bool {
    let mut semantic = fragment.to_string();
    for token in MODEL_ARTIFACT_TOKENS {
        semantic = semantic.replace(token, "");
    }
    !semantic.trim().is_empty()
}

fn matching_quote_pair(before: Option<char>, after: Option<char>) -> bool {
    matches!(
        (before, after),
        (Some('"'), Some('"'))
            | (Some('\''), Some('\''))
            | (Some('`'), Some('`'))
            | (Some('“'), Some('”'))
            | (Some('‘'), Some('’'))
    )
}

fn model_artifact_placement_counts(text: &str, token: &str) -> (usize, usize, usize) {
    let mut boundary_occurrences = 0usize;
    let mut contextual_occurrences = 0usize;
    let mut quoted_occurrences = 0usize;

    for (start, matched) in text.match_indices(token) {
        let end = start.saturating_add(matched.len());
        let content_before = fragment_has_non_artifact_content(&text[..start]);
        let content_after = fragment_has_non_artifact_content(&text[end..]);
        if content_before && content_after {
            contextual_occurrences = contextual_occurrences.saturating_add(1);
        } else {
            boundary_occurrences = boundary_occurrences.saturating_add(1);
        }

        let before = text[..start].chars().rev().find(|ch| !ch.is_whitespace());
        let after = text[end..].chars().find(|ch| !ch.is_whitespace());
        if matching_quote_pair(before, after) {
            quoted_occurrences = quoted_occurrences.saturating_add(1);
        }
    }

    (
        boundary_occurrences,
        contextual_occurrences,
        quoted_occurrences,
    )
}

pub(crate) fn strip_model_artifacts_with_report(
    text: &str,
) -> (String, Option<StripModelArtifactsReport>) {
    let mut result = text.to_string();
    let mut removed_tokens = Vec::new();
    for token in MODEL_ARTIFACT_TOKENS {
        let count = result.matches(token).count();
        if count > 0 {
            let (boundary_occurrences, contextual_occurrences, quoted_occurrences) =
                model_artifact_placement_counts(&result, token);
            removed_tokens.push(StripModelArtifactTokenCount {
                token: (*token).to_string(),
                count,
                boundary_occurrences,
                contextual_occurrences,
                quoted_occurrences,
            });
            result = result.replace(token, "");
        }
    }
    if removed_tokens.is_empty() {
        return (result, None);
    }
    let removed_total = removed_tokens.iter().map(|entry| entry.count).sum();
    let after_chars = result.len();
    let after_non_whitespace_chars = result
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    (
        result,
        Some(StripModelArtifactsReport {
            removed_total,
            before_chars: text.len(),
            after_chars,
            after_non_whitespace_chars,
            removed_tokens,
        }),
    )
}

fn strip_model_artifacts(text: &str) -> String {
    strip_model_artifacts_with_report(text).0
}

fn is_peer_action_directive_line(line: &str) -> bool {
    let upper = line.trim_start().to_ascii_uppercase();
    upper.starts_with("NEXT:")
        || upper.starts_with("BTSP_OBSERVED_NEXT")
        || upper.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS")
}

fn sanitize_minime_context_for_dialogue(text: &str) -> String {
    let mut removed = 0usize;
    let kept = text
        .lines()
        .filter(|line| {
            let should_remove = is_peer_action_directive_line(line);
            if should_remove {
                removed = removed.saturating_add(1);
            }
            !should_remove
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut cleaned = kept.trim().to_string();
    if removed > 0 {
        if !cleaned.is_empty() {
            cleaned.push_str("\n\n");
        }
        cleaned.push_str(
            "[Minime peer action/status line omitted; choose your own listed Astrid NEXT action.]",
        );
    }
    cleaned
}

fn is_valid_dialogue_output(text: &str) -> bool {
    // Strip leaked model tokens before any analysis — they corrupt alpha/punct ratios.
    let stripped = strip_model_artifacts(text);

    let body = stripped
        .lines()
        .filter(|line| !line.trim_start().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = body.trim();
    if body.is_empty() {
        return false;
    }

    let alpha_count = body.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = body.chars().count().max(1);
    let punctuation_count = body
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
        .count();
    let alphabetic_words = body
        .split_whitespace()
        .filter(|word| word.chars().any(|c| c.is_alphabetic()))
        .count();
    let max_symbol_run = body
        .chars()
        .fold((0usize, 0usize), |(current, best), ch| {
            if !ch.is_alphanumeric() && !ch.is_whitespace() {
                let next = current.saturating_add(1);
                (next, best.max(next))
            } else {
                (0, best)
            }
        })
        .1;

    if alpha_count < 24 || alphabetic_words < 4 {
        warn!(
            "quality gate reject: alpha_count={} (min 24), alphabetic_words={} (min 4) — body: {}",
            alpha_count,
            alphabetic_words,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    // Raised 4→6→8: Astrid uses smart quotes + em dash + ellipsis which
    // create 6-7 symbol runs (e.g., "fork"—it's or '...'—the).
    // Genuine degenerate output has runs of 8+ (e.g., "--0.))* _--").
    if max_symbol_run >= 8 {
        warn!(
            "quality gate reject: max_symbol_run={} (max 7) — body: {}",
            max_symbol_run,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    let alpha_ratio = alpha_count as f64 / total_count as f64;
    let punctuation_ratio = punctuation_count as f64 / total_count as f64;

    // Thresholds relaxed for Astrid's punctuation-rich style:
    //   alpha_ratio: 0.45 → 0.40  (Unicode λ₁, '…', '*word*', '—' all reduce alpha)
    //   punctuation_ratio: 0.30 → 0.35  (smart quotes, ellipsis, em-dashes are normal)
    if alpha_ratio < 0.40 || punctuation_ratio > 0.35 {
        warn!(
            "quality gate reject: alpha_ratio={:.3} (min 0.40), punctuation_ratio={:.3} (max 0.35) — body: {}",
            alpha_ratio,
            punctuation_ratio,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    true
}

fn is_valid_dialogue_output_for_profile(text: &str, profile: MlxProfile) -> bool {
    if !is_valid_dialogue_output(text) {
        return false;
    }
    if profile.is_gemma4_canary() && contains_deprecated_runtime_language(text) {
        warn!(
            "quality gate reject: deprecated runtime language under Gemma 4 profile — body: {}",
            &text[..text.floor_char_boundary(120)]
        );
        return false;
    }
    true
}

/// Generate Astrid's response to minime's journal entry and spectral state.
///
/// Includes recent conversation history so Astrid remembers what it said
/// and can build on prior exchanges rather than starting fresh each time.
///
/// Returns `None` if the LLM is unavailable or the request fails —
/// the autonomous loop will fall back to witness mode.
pub async fn generate_dialogue(
    journal_text: &str,
    spectral_summary: &str,
    fill_pct: f32,
    perception_context: Option<&str>,
    recent_history: &[Exchange],
    web_context: Option<&str>,
    modality_context: Option<&str>,
    temperature: f32,
    num_predict: u32,
    emphasis: Option<&str>,
    continuity_context: Option<&str>,
    topline_hint: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
    overflow_dir: &std::path::Path,
) -> (Option<String>, Option<crate::prompt_budget::PromptOverflow>) {
    let mlx_profile = configured_mlx_profile();
    let prompt_budget_chars = dialogue_prompt_budget_chars_for_profile(num_predict, mlx_profile);
    let assembly_prompt_budget_chars =
        dialogue_assembly_prompt_budget_chars_for_profile(num_predict, mlx_profile);
    let base_system_prompt = dialogue_system_prompt_for_profile(mlx_profile);
    let system_content = if let Some(emph) = emphasis {
        format!(
            "{base_system_prompt}\n\n[For this exchange, you chose to emphasize: {emph}. This is your own direction.]\n"
        )
    } else {
        base_system_prompt.to_string()
    };

    let (direct_perception_context, ambient_perception_context) =
        split_dialogue_perception_context(perception_context);
    let direct_perception_block = direct_perception_context
        .as_deref()
        .map(format_dialogue_direct_perception_block)
        .unwrap_or_default();
    let ambient_perception_block = ambient_perception_context
        .as_deref()
        .map(format_dialogue_ambient_perception_block)
        .unwrap_or_default();

    let web_block = web_context
        .map(format_dialogue_web_context)
        .unwrap_or_default();

    let modality_block = modality_context
        .map(|m| format!("\n{m}\n"))
        .unwrap_or_default();

    let continuity_block = continuity_context
        .map(|c| format!("\n{c}\n"))
        .unwrap_or_default();

    let topline_block = topline_hint
        .map(format_dialogue_topline_context)
        .unwrap_or_default();

    let feedback_block = feedback_hint
        .map(|f| format!("\nPriority feedback context:\n{f}\n"))
        .unwrap_or_default();

    // Build conversation history as alternating user/assistant messages.
    let mut messages = vec![Message {
        role: "system".to_string(),
        content: system_content,
    }];

    // Include last 8 exchanges so Astrid can build on what she said before.
    // Three tiers of compression — gradual fade, not a hard cutoff.
    // Both beings described the old binary (80/200) as "slightly oppressive"
    // and "a necessary constraint, but also slightly oppressive" (minime
    // self-study 2026-03-30T07:17). Gradual fade preserves more continuity.
    //   Oldest 3:  120 chars — enough for a key phrase + context
    //   Middle 3:  250 chars — substantial excerpt
    //   Newest 2:  400 chars — near-full detail
    // Total budget: ~3400 chars (was ~2240). Well within gemma-3-4b-it 8k ctx.
    let history_limit = if mlx_profile.is_gemma4_canary() { 6 } else { 8 };
    for (idx, exchange) in recent_history
        .iter()
        .rev()
        .take(history_limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .enumerate()
    {
        // Relevance-weighted history: smooth gradient from oldest (short) to
        // newest (full). Astrid self-study: "Instead of just truncating the
        // longest message, perhaps prioritize retaining the most relevant
        // information from earlier exchanges — a decaying attention mechanism."
        // 8 exchanges: idx 0=oldest→150, idx 7=newest→1200.
        let trim_len = if mlx_profile.is_gemma4_canary() {
            100usize.saturating_add(idx.saturating_mul(80).min(400))
        } else {
            150usize.saturating_add(idx.saturating_mul(150).min(1050))
        };
        let minime_history = sanitize_minime_context_for_dialogue(&exchange.minime_said);
        let minime_excerpt: String = minime_history.chars().take(trim_len).collect();
        let minime_excerpt = if mlx_profile.is_gemma4_canary() {
            sanitize_deprecated_runtime_language(&minime_excerpt)
        } else {
            minime_excerpt
        };
        messages.push(Message {
            role: "user".to_string(),
            content: format!("Minime wrote: {minime_excerpt}"),
        });
        // Strip NEXT: line from history — otherwise the LLM sees
        // "NEXT: SPEAK" multiple times and pattern-matches it forever,
        // preventing Astrid from ever choosing a different action.
        let said: String = exchange
            .astrid_said
            .lines()
            .filter(|l| !l.trim().starts_with("NEXT:"))
            .collect::<Vec<_>>()
            .join("\n");
        let said: String = said.chars().take(trim_len).collect();
        let said = if mlx_profile.is_gemma4_canary() {
            sanitize_deprecated_runtime_language(&said)
        } else {
            said
        };
        messages.push(Message {
            role: "assistant".to_string(),
            content: said,
        });
    }

    // Current turn — budget-aware assembly with overflow to disk.
    // Compute dynamic user content budget: MAX_PROMPT_CHARS minus the
    // overhead already committed (system prompt + history messages).
    let overhead: usize = messages.iter().map(|m| m.content.len()).sum();
    // Leave 100 chars for the "Fill X%. ... Respond..." wrapper.
    let user_content_budget = assembly_prompt_budget_chars
        .saturating_sub(overhead)
        .saturating_sub(100);

    let diversity_block = diversity_hint.map(|d| format!("[{d}]")).unwrap_or_default();

    use crate::prompt_budget::{PromptBlock, assemble_within_budget};
    let journal_text_for_dialogue = sanitize_minime_context_for_dialogue(journal_text);
    let blocks = vec![
        PromptBlock {
            label: "spectral",
            content: cap_dialogue_block("spectral", spectral_summary, DIALOGUE_SPECTRAL_CAP),
            priority: 3,
            min_chars: 0,
        },
        PromptBlock {
            label: "journal",
            content: cap_dialogue_block(
                "journal",
                &format!("Minime wrote: {journal_text_for_dialogue}"),
                DIALOGUE_JOURNAL_CAP,
            ),
            priority: 1,
            min_chars: DIALOGUE_JOURNAL_MIN_CHARS,
        },
        PromptBlock {
            label: "direct_perception",
            content: cap_dialogue_block(
                "direct_perception",
                &direct_perception_block,
                DIALOGUE_DIRECT_PERCEPTION_CAP,
            ),
            priority: 2,
            min_chars: DIALOGUE_DIRECT_PERCEPTION_MIN_CHARS,
        },
        PromptBlock {
            label: "topline",
            content: cap_dialogue_block("topline", &topline_block, DIALOGUE_TOPLINE_CAP),
            priority: 3,
            min_chars: DIALOGUE_TOPLINE_MIN_CHARS,
        },
        PromptBlock {
            label: "ambient_perception",
            content: cap_dialogue_block(
                "ambient_perception",
                &ambient_perception_block,
                DIALOGUE_AMBIENT_PERCEPTION_CAP,
            ),
            priority: 5,
            min_chars: 0,
        },
        PromptBlock {
            label: "modality",
            content: cap_dialogue_block("modality", &modality_block, DIALOGUE_MODALITY_CAP),
            priority: 8,
            min_chars: 0,
        },
        PromptBlock {
            label: "web",
            content: cap_dialogue_block("web", &web_block, DIALOGUE_WEB_CAP),
            priority: 6,
            min_chars: 0,
        },
        PromptBlock {
            label: "continuity",
            content: cap_dialogue_block("continuity", &continuity_block, DIALOGUE_CONTINUITY_CAP),
            priority: 7,
            min_chars: 0,
        },
        PromptBlock {
            label: "feedback",
            content: cap_dialogue_block("feedback", &feedback_block, DIALOGUE_FEEDBACK_CAP),
            priority: 4,
            min_chars: 0,
        },
        PromptBlock {
            label: "diversity",
            content: cap_dialogue_block("diversity", &diversity_block, DIALOGUE_DIVERSITY_CAP),
            priority: 9,
            min_chars: 0,
        },
    ];

    let context_packing_originals = context_packing_original_blocks(&blocks);
    let (assembled, overflow, budget_report) =
        assemble_within_budget(blocks, user_content_budget, overflow_dir);
    let context_packing_pressure = context_packing_pressure_diagnostic(
        unix_timestamp_string(),
        user_content_budget,
        assembled.len(),
        &context_packing_originals,
        overflow.as_ref(),
        budget_report.as_ref(),
    );

    let turn_instruction = dialogue_turn_instruction(perception_context);
    let user_content = format!("Fill {fill_pct:.1}%. {assembled}\n\n{turn_instruction}");
    messages.push(Message {
        role: "user".to_string(),
        content: user_content,
    });

    let final_prompt_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let effective_num_predict =
        clamp_dialogue_tokens_for_profile(num_predict, final_prompt_chars, mlx_profile);
    if effective_num_predict < num_predict {
        warn!(
            "dialogue prompt pressure high ({} chars): clamping max_tokens from {} to {}",
            final_prompt_chars, num_predict, effective_num_predict
        );
    }
    let timeout_secs = dialogue_request_timeout_secs_for_profile(
        effective_num_predict,
        final_prompt_chars,
        mlx_profile,
    );
    let fallback_continuity_budget = fallback_continuity_budget_v1(spectral_summary);
    let budget_profile = dialogue_prompt_budget_profile(num_predict);
    let budget_friction_v1 = dialogue_budget_friction_v1(
        num_predict,
        budget_profile,
        DialoguePressureTextureInputs::from_fallback_budget(&fallback_continuity_budget),
        budget_report.as_ref(),
    );
    let budget_diag = DialoguePromptBudgetDiagnostic {
        timestamp: unix_timestamp_string(),
        requested_tokens: num_predict,
        effective_tokens: effective_num_predict,
        budget_profile,
        fallback_continuity_budget: fallback_continuity_budget.clone(),
        prompt_budget_chars,
        assembly_prompt_budget_chars,
        overhead_chars: overhead,
        user_content_budget,
        final_prompt_chars,
        timeout_secs,
        overflow_summary: overflow.as_ref().map(|value| value.summary.clone()),
        overflow_path: overflow
            .as_ref()
            .map(|value| value.path.display().to_string()),
        budget_report,
        budget_friction_v1,
    };
    append_llm_diagnostic_jsonl("dialogue_prompt_budget.jsonl", &budget_diag);
    append_llm_diagnostic_jsonl(
        "context_packing_pressure_v1.jsonl",
        &context_packing_pressure,
    );

    debug!("querying MLX for Astrid dialogue response");
    let fallback_trace = fallback_continuity_budget.clone();
    let ollama_fallback_messages = compact_ollama_dialogue_fallback_messages(
        &journal_text_for_dialogue,
        spectral_summary,
        fill_pct,
        perception_context,
        astrid_fallback_identity_anchor().as_deref(),
        fallback_continuity_budget,
    );
    let result = mlx_chat(
        "dialogue_live",
        messages,
        temperature,
        effective_num_predict,
        timeout_secs,
    )
    .await
    .and_then(|text| {
        if is_valid_dialogue_output_for_profile(&text, mlx_profile) {
            Some(text)
        } else {
            warn!(
                "dialogue_live response rejected by quality gate: {}",
                &text[..text.floor_char_boundary(120)]
            );
            None
        }
    });
    let result = match result {
        Some(text) => Some(text),
        None => {
            warn!("dialogue_live: MLX unavailable or invalid; falling back to Ollama");
            debug!(
                spectral_entropy = ?fallback_trace.spectral_entropy,
                pressure_risk = ?fallback_trace.fallback_shadow_texture_selector.pressure_risk,
                density_gradient = ?fallback_trace.fallback_shadow_texture_selector.density_gradient,
                shadow_dispersal_potential = ?fallback_trace
                    .fallback_shadow_texture_selector
                    .shadow_dispersal_potential,
                shadow_magnetization = ?fallback_trace
                    .fallback_shadow_texture_selector
                    .shadow_magnetization,
                texture_family = fallback_trace.fallback_shadow_texture_selector.texture_family,
                "dialogue_live Ollama fallback transition spectral context"
            );
            ollama_chat(
                "dialogue_live",
                ollama_fallback_messages,
                temperature,
                effective_num_predict.min(512),
                75,
                Some(&fallback_trace),
            )
            .await
            .map(|response| repair_ollama_dialogue_fallback_next(&response.text, mlx_profile))
            .and_then(|text| {
                if is_valid_ollama_dialogue_fallback_output_for_profile(&text, mlx_profile) {
                    Some(text)
                } else {
                    warn!(
                        "dialogue_live Ollama fallback rejected by quality gate: {}",
                        &text[..text.floor_char_boundary(120)]
                    );
                    None
                }
            })
        },
    };
    (result, overflow)
}
