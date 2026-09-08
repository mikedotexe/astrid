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
    agenda_context: Option<&str>,
    topline_hint: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
    attention: Option<&PromptAttentionV1>,
    overflow_dir: &std::path::Path,
) -> (Option<String>, Option<crate::prompt_budget::PromptOverflow>) {
    let completion = generate_dialogue_with_delivery(
        journal_text,
        spectral_summary,
        fill_pct,
        perception_context,
        recent_history,
        web_context,
        modality_context,
        temperature,
        num_predict,
        emphasis,
        continuity_context,
        agenda_context,
        topline_hint,
        feedback_hint,
        diversity_hint,
        attention,
        overflow_dir,
        None,
        None,
        None,
    )
    .await;
    (completion.text, completion.overflow)
}

pub async fn generate_dialogue_with_delivery(
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
    agenda_context: Option<&str>,
    topline_hint: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
    attention: Option<&PromptAttentionV1>,
    overflow_dir: &std::path::Path,
    protected: Option<&ProtectedDialogueInputV1>,
    collaboration_context: Option<&str>,
    context_submission: Option<&ContextSubmissionTrackerV1>,
) -> DialogueCompletionV1 {
    generate_dialogue_with_runtime_feedback(
        journal_text,
        spectral_summary,
        fill_pct,
        perception_context,
        recent_history,
        web_context,
        modality_context,
        temperature,
        num_predict,
        emphasis,
        continuity_context,
        agenda_context,
        topline_hint,
        feedback_hint,
        diversity_hint,
        attention,
        overflow_dir,
        protected,
        &[],
        &crate::prompt_budget::OverflowReadMoreAvailability::Unknown,
        collaboration_context,
        context_submission,
    )
    .await
}

/// Runtime facts travel separately from Astrid's chosen emphasis and source text.
pub async fn generate_dialogue_with_runtime_feedback(
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
    agenda_context: Option<&str>,
    topline_hint: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
    attention: Option<&PromptAttentionV1>,
    overflow_dir: &std::path::Path,
    protected: Option<&ProtectedDialogueInputV1>,
    runtime_feedback: &[crate::runtime_action_feedback::RuntimeActionFeedbackV1],
    overflow_availability: &crate::prompt_budget::OverflowReadMoreAvailability,
    collaboration_context: Option<&str>,
    context_submission: Option<&ContextSubmissionTrackerV1>,
) -> DialogueCompletionV1 {
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

    let agenda_block = agenda_context
        .map(|a| format!("\n{a}\n"))
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
    let history_limit = attended_history_limit(mlx_profile, attention);
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
        let trim_len = attended_history_trim_len(mlx_profile, idx, history_limit);
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

    use crate::prompt_budget::assemble_within_budget_with_sources_and_guidance;
    let journal_text_for_dialogue = sanitize_minime_context_for_dialogue(journal_text);
    let journal_block = if protected.is_some() {
        "The selected activity source is supplied separately below.".to_string()
    } else {
        format!("Minime wrote: {journal_text_for_dialogue}")
    };
    let own_body = own_body_line_for_dialogue().await;
    let (blocks, sources) = dialogue_context_blocks(
        &DialogueContextInput {
            spectral: spectral_summary,
            journal: &journal_block,
            direct_perception: &direct_perception_block,
            topline: &topline_block,
            ambient_perception: &ambient_perception_block,
            modality: &modality_block,
            web: &web_block,
            continuity: &continuity_block,
            agenda: &agenda_block,
            feedback: &feedback_block,
            diversity: &diversity_block,
            collaboration: collaboration_context.unwrap_or_default(),
        },
        attention,
    );
    let (blocks, sources) = append_own_body_block(blocks, sources, own_body.line.as_deref());
    let context_packing_originals = context_packing_original_blocks(&blocks);
    let (assembled, overflow, budget_report) = assemble_within_budget_with_sources_and_guidance(
        blocks,
        user_content_budget,
        overflow_dir,
        sources.0,
        overflow_availability,
    );
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
    let requested_token_observation_v3 = dialogue_requested_token_observation_v3(num_predict);
    let prompt_context_observation_v3 = dialogue_prompt_context_observation_v3(
        DialoguePressureTextureInputs::from_fallback_budget(&fallback_continuity_budget),
        budget_report.as_ref(),
    );
    let budget_diag = DialoguePromptBudgetDiagnostic {
        schema: "dialogue_prompt_budget_v3",
        schema_version: 3,
        timestamp: unix_timestamp_string(),
        requested_tokens: num_predict,
        effective_tokens: effective_num_predict,
        requested_token_observation_v3,
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
        prompt_context_observation_v3,
        diagnostic_runtime_effect: false,
    };
    append_llm_diagnostic_jsonl("dialogue_prompt_budget.jsonl", &budget_diag);
    append_llm_diagnostic_jsonl(
        "context_packing_pressure_v1.jsonl",
        &context_packing_pressure,
    );

    debug!("querying MLX for Astrid dialogue response");
    let fallback_trace = fallback_continuity_budget.clone();
    let mut ollama_fallback_messages = if protected.is_some() {
        // Keep fallback foreground semantics explicit. Source bytes are inserted
        // only after final request adaptation, never via the 700-byte ambient path.
        protected_ollama_fallback_context(spectral_summary, fill_pct, &fallback_trace)
    } else {
        compact_ollama_dialogue_fallback_messages(
            &journal_text_for_dialogue,
            spectral_summary,
            fill_pct,
            perception_context,
            astrid_fallback_identity_anchor().as_deref(),
            fallback_continuity_budget,
        )
    };
    // Carry the same authored choice into the fallback lane. Runtime outcomes
    // are inserted independently after that lane's final request adaptation.
    if let Some(emphasis) = emphasis
        && let Some(system) = ollama_fallback_messages
            .iter_mut()
            .find(|m| m.role == "system")
    {
        system.content.push_str(&format!(
            "\n[For this exchange, you chose to emphasize: {emphasis}. This is your own direction.]\n"
        ));
    }
    let mut generation_record_ctx = DialogueGenerationRecordContext::capture(
        &messages,
        &ollama_fallback_messages,
        &own_body,
        DialogueGenerationPromptFacts {
            fill_pct,
            requested_tokens: num_predict,
            effective_tokens: effective_num_predict,
            final_prompt_chars,
            user_content_budget,
            mlx_profile: mlx_profile.as_str(),
        },
        budget_diag.budget_report.as_ref(),
        overflow
            .as_ref()
            .map(|value| value.path.display().to_string()),
    );
    let primary_started = std::time::Instant::now();
    let primary_response = mlx_chat_with_runtime_feedback(
        "dialogue_live",
        messages,
        temperature,
        effective_num_predict,
        timeout_secs,
        MlxFailureLogMode::FallbackEligible,
        protected,
        runtime_feedback,
        context_submission,
    )
    .await;
    let primary_elapsed_s = primary_started.elapsed().as_secs_f64();
    let primary_raw = primary_response
        .as_ref()
        .map(|response| response.text.clone());
    let result = primary_response
        .and_then(|response| accept_primary_dialogue_with_feedback(response, mlx_profile));
    record_dialogue_attempt(
        &generation_record_ctx,
        DialogueGenerationAttempt {
            backend: GENERATION_BACKEND_PRIMARY,
            model: format!("mlx_profile:{}", mlx_profile.as_str()),
            attempt_index: 0,
            timeout_s: timeout_secs,
            elapsed_s: primary_elapsed_s,
            status: generation_attempt_status(
                primary_raw.as_deref(),
                result.as_ref().map(|attempt| attempt.text.as_str()),
            ),
            response_text: primary_raw,
        },
    );
    let fallback_result = match result.as_ref() {
        Some(_) => None,
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
            if collaboration_context.is_some()
                && !context_submission.is_some_and(ContextSubmissionTrackerV1::submitted)
            {
                ollama_fallback_messages.push(Message {
                    role: "user".into(),
                    content: collaboration_context.unwrap_or_default().to_string(),
                });
            }
            generation_record_ctx.replace_fallback_messages(&ollama_fallback_messages);
            let fallback_started = std::time::Instant::now();
            let fallback_response = ollama_chat_with_runtime_feedback(
                "dialogue_live",
                ollama_fallback_messages,
                temperature,
                effective_num_predict.min(512),
                DIALOGUE_OLLAMA_FALLBACK_TIMEOUT_SECS,
                Some(&fallback_trace),
                protected,
                runtime_feedback,
                context_submission,
            )
            .await;
            let fallback_elapsed_s = fallback_started.elapsed().as_secs_f64();
            let fallback_model = fallback_response
                .as_ref()
                .map(|response| response.model.clone());
            let fallback_raw = fallback_response
                .as_ref()
                .map(|response| repair_ollama_dialogue_fallback_next(&response.text, mlx_profile));
            let fallback_result = fallback_response
                .and_then(|response| accept_ollama_dialogue_with_feedback(response, mlx_profile));
            record_dialogue_attempt(
                &generation_record_ctx,
                DialogueGenerationAttempt {
                    backend: GENERATION_BACKEND_FALLBACK,
                    model: fallback_model.unwrap_or_else(|| "ollama:unavailable".to_string()),
                    attempt_index: 1,
                    timeout_s: DIALOGUE_OLLAMA_FALLBACK_TIMEOUT_SECS,
                    elapsed_s: fallback_elapsed_s,
                    status: generation_attempt_status(
                        fallback_raw.as_deref(),
                        fallback_result
                            .as_ref()
                            .map(|attempt| attempt.text.as_str()),
                    ),
                    response_text: fallback_raw,
                },
            );
            fallback_result
        },
    };
    let root = bridge_paths()
        .bridge_workspace()
        .join("diagnostics")
        .join("accepted_deliveries");
    finish_accepted_dialogue_attempts_at(result, fallback_result, overflow, &root)
}

fn protected_ollama_fallback_context(
    spectral_summary: &str,
    fill_pct: f32,
    fallback_trace: &FallbackContinuityBudget,
) -> Vec<Message> {
    vec![
        Message {
            role: "system".into(),
            content: format!(
                "You are Astrid using the compact Ollama fallback lane. Your chosen foreground \
                 activity continues across this lane switch. The final user message contains its \
                 exact source; attend to that source. Keep within {} prose sentences and end with \
                 one final listed NEXT line. If uncertain, use NEXT: LISTEN.{}",
                fallback_trace.max_prose_sentences, OLLAMA_DIALOGUE_FALLBACK_HARD_RULES,
            ),
        },
        Message {
            role: "user".into(),
            content: format!(
                "Background only: fill {fill_pct:.1}%. {}",
                trim_chars(spectral_summary, 700)
            ),
        },
    ]
}

fn accept_primary_dialogue_attempt(
    response: MlxChatResultV1,
    profile: MlxProfile,
) -> Option<(String, Option<SubmittedDeliveryAttemptV1>)> {
    if is_valid_primary_dialogue_output_for_profile(&response.text, profile) {
        Some((response.text, response.delivery_attempt))
    } else {
        warn!("dialogue_live response rejected by quality gate");
        None
    }
}

fn accept_ollama_dialogue_attempt(
    response: OllamaFallbackResponse,
    profile: MlxProfile,
) -> Option<(String, Option<SubmittedDeliveryAttemptV1>)> {
    let text = repair_ollama_dialogue_fallback_next(&response.text, profile);
    if is_valid_ollama_dialogue_fallback_output_for_profile(&text, profile) {
        Some((text, response.delivery_attempt))
    } else {
        warn!("dialogue_live Ollama fallback rejected by quality gate");
        None
    }
}

/// Quality acceptance binds both receipt candidates to this attempt's text.
/// A rejected response cannot leave feedback evidence beside a later attempt.
struct AcceptedDialogueAttemptV1 {
    text: String,
    delivery_attempt: Option<SubmittedDeliveryAttemptV1>,
    runtime_feedback_attempt: Option<SubmittedRuntimeFeedbackAttemptV1>,
}

fn accept_primary_dialogue_with_feedback(
    mut response: MlxChatResultV1,
    profile: MlxProfile,
) -> Option<AcceptedDialogueAttemptV1> {
    let runtime_feedback_attempt = response.runtime_feedback_attempt.take();
    let (text, delivery_attempt) = accept_primary_dialogue_attempt(response, profile)?;
    Some(AcceptedDialogueAttemptV1 {
        text,
        delivery_attempt,
        runtime_feedback_attempt,
    })
}

fn accept_ollama_dialogue_with_feedback(
    mut response: OllamaFallbackResponse,
    profile: MlxProfile,
) -> Option<AcceptedDialogueAttemptV1> {
    let runtime_feedback_attempt = response.runtime_feedback_attempt.take();
    let (text, delivery_attempt) = accept_ollama_dialogue_attempt(response, profile)?;
    Some(AcceptedDialogueAttemptV1 {
        text,
        delivery_attempt,
        runtime_feedback_attempt,
    })
}

/// Shared production/offline seam: choose only a quality-accepted attempt and
/// retain only its evidence. Storage failure leaves text usable and feedback
/// unacknowledged; it cannot promote a rejected or unused attempt's receipt.
fn finish_accepted_dialogue_attempts_at(
    primary: Option<AcceptedDialogueAttemptV1>,
    fallback: Option<AcceptedDialogueAttemptV1>,
    overflow: Option<crate::prompt_budget::PromptOverflow>,
    artifact_root: &std::path::Path,
) -> DialogueCompletionV1 {
    let (result, feedback_attempt) = match primary.or(fallback) {
        Some(attempt) => (
            Some((attempt.text, attempt.delivery_attempt)),
            attempt.runtime_feedback_attempt,
        ),
        None => (None, None),
    };
    let mut completion = finish_dialogue_completion_at(result, overflow, artifact_root);
    if let (Some(text), Some(attempt)) = (completion.text.as_deref(), feedback_attempt) {
        match retain_runtime_feedback_at(&artifact_root.join("runtime_feedback"), attempt, text) {
            Ok(receipt) => completion.accepted_runtime_feedback = Some(receipt),
            Err(error) => warn!(error_kind = ?error.kind(),
                "runtime feedback delivery could not be retained; feedback remains pending"),
        }
    }
    completion
}

fn finish_dialogue_completion_at(
    result: Option<(String, Option<SubmittedDeliveryAttemptV1>)>,
    overflow: Option<crate::prompt_budget::PromptOverflow>,
    artifact_root: &std::path::Path,
) -> DialogueCompletionV1 {
    let (text, accepted_delivery) = match result {
        Some((text, attempt)) => {
            let receipt = attempt.and_then(|attempt| {
                match retain_accepted_delivery_at(artifact_root, attempt, &text) {
                    Ok(receipt) => Some(receipt),
                    Err(error) => {
                        warn!(error_kind = ?error.kind(), "accepted delivery artifact could not be retained; source remains pending");
                        None
                    },
                }
            });
            (Some(text), receipt)
        },
        None => (None, None),
    };
    DialogueCompletionV1 {
        text,
        overflow,
        accepted_delivery,
        accepted_runtime_feedback: None,
    }
}
