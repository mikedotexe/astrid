// Compatibility entry points used by the non-activity provider lanes.
async fn mlx_chat_with_failure_log_mode_detailed(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    failure_log_mode: MlxFailureLogMode,
) -> Option<MlxChatResultV1> {
    mlx_chat_with_protected_delivery(
        label,
        messages,
        temperature,
        max_tokens,
        timeout_secs,
        failure_log_mode,
        None,
        None,
    )
    .await
}

async fn ollama_chat(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    fallback_budget: Option<&FallbackContinuityBudget>,
) -> Option<OllamaFallbackResponse> {
    ollama_chat_with_protected_delivery(
        label,
        messages,
        temperature,
        max_tokens,
        timeout_secs,
        fallback_budget,
        None,
        None,
    )
    .await
}

/// Send a chat request to the MLX server and extract the response text.
async fn mlx_chat_with_protected_delivery(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    failure_log_mode: MlxFailureLogMode,
    protected: Option<&ProtectedDialogueInputV1>,
    context_submission: Option<&ContextSubmissionTrackerV1>,
) -> Option<MlxChatResultV1> {
    let profile = configured_mlx_profile();
    let policy = apply_mlx_request_policy(label, profile, messages, max_tokens, timeout_secs);
    if let Some(ref diagnostic) = policy.diagnostic {
        append_llm_diagnostic_jsonl("mlx_request_policy.jsonl", diagnostic);
    }

    let mut messages = policy.messages;
    let max_tokens = policy.max_tokens;
    let timeout_secs = policy.timeout_secs;

    let msg_count = messages.len();
    let prompt_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let mlx_url = configured_mlx_url();

    // Safety net: if total prompt exceeds budget, truncate the longest
    // non-system message. Prevents prefill timeouts on any caller.
    // Legacy profile safety net. The adopted Gemma 4 profile applies tighter
    // per-label caps before this generic budget is reached.
    const MAX_PROMPT_CHARS: usize = 48_000;
    if !profile.is_gemma4_canary() && prompt_chars > MAX_PROMPT_CHARS {
        let excess = prompt_chars.saturating_sub(MAX_PROMPT_CHARS);
        warn!(
            "Prompt budget exceeded ({prompt_chars} > {MAX_PROMPT_CHARS}), trimming {excess} chars"
        );
        // Find the longest non-system message and truncate it.
        if let Some(longest) = messages
            .iter_mut()
            .filter(|m| m.role != "system")
            .max_by_key(|m| m.content.len())
        {
            let new_len = longest.content.len().saturating_sub(excess);
            longest.content = longest.content.chars().take(new_len).collect();
        }
    }

    let admission = if let Some(input) = protected {
        Some(admit_protected_dialogue_content(
            &mut messages,
            input,
            if profile.is_gemma4_canary() {
                gemma4_canary_prompt_limit(label).unwrap_or(48_000)
            } else {
                48_000
            },
        )?)
    } else {
        None
    };

    let (max_tokens, timeout_secs) = if protected.is_some() && label == "dialogue_live" {
        let final_bytes = message_prompt_chars(&messages);
        let tokens = clamp_dialogue_tokens_for_profile(max_tokens, final_bytes, profile);
        (
            tokens,
            timeout_secs.max(dialogue_request_timeout_secs_for_profile(
                tokens,
                final_bytes,
                profile,
            )),
        )
    } else {
        (max_tokens, timeout_secs)
    };
    let client = delivery_http_client(
        timeout_secs,
        protected.is_some() || context_submission.is_some(),
    )?;

    let temperature = temperature_for_mlx_profile(label, profile, temperature);
    let model_qos = model_qos_v1(label, &messages, temperature, max_tokens, timeout_secs);
    let qos_request_identity_sha256 = serde_json::to_vec(&model_qos)
        .ok()
        .map(|encoded| format!("{:x}", Sha256::digest(encoded)))?;
    let request_content_anchor_sha256 = model_qos.idempotency_key.clone();
    let request = MlxRequest {
        messages,
        max_tokens,
        temperature,
        stream: false,
        aperture: Some(astrid_aperture()),
        model_qos_v1: Some(model_qos),
    };

    let request_bytes = serde_json::to_vec(&request).ok()?;
    if let Some(tracker) = context_submission {
        tracker.mark_final_messages(&request.messages);
    }
    let response = match client
        .post(&mlx_url)
        .header("Content-Type", "application/json")
        .body(request_bytes.clone())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            match failure_log_mode {
                MlxFailureLogMode::FallbackEligible => {
                    warn!(
                        "MLX request failed at {mlx_url}: {e} (timeout={timeout_secs}s, max_tokens={max_tokens}, msg_count={msg_count}, prompt_chars={prompt_chars})",
                    );
                },
                MlxFailureLogMode::LocalDegrade => {
                    let diagnostic = MlxOptionalMissDiagnostic {
                        timestamp: unix_timestamp_string(),
                        label: label.to_string(),
                        profile: profile.as_str(),
                        url: mlx_url.clone(),
                        error: e.to_string(),
                        timeout_secs,
                        max_tokens,
                        msg_count,
                        prompt_chars,
                        degrade_path: local_degrade_path_for_label(label),
                    };
                    warn!(
                        label = %label,
                        timeout_secs,
                        max_tokens,
                        msg_count,
                        prompt_chars,
                        degrade_path = diagnostic.degrade_path,
                        "optional MLX lane unavailable; using local degrade path"
                    );
                    append_llm_diagnostic_jsonl("mlx_optional_miss.jsonl", &diagnostic);
                },
            }
            return None;
        },
    };
    if !response.status().is_success() {
        warn!("MLX returned status {} from {mlx_url}", response.status());
        return None;
    }
    let accepted_route = response.url().to_string();
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            warn!("MLX response body read failed: {e}");
            return None;
        },
    };
    let chat: MlxResponse = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "MLX response parse failed from {mlx_url}: {e} — body: {}",
                &body[..body.floor_char_boundary(200)]
            );
            return None;
        },
    };
    let provider_model = chat
        .model
        .clone()
        .unwrap_or_else(|| format!("unreported; configured_profile={}", profile.as_str()));
    let provider_timing = chat
        .model_qos_timing_v1
        .and_then(ModelQosTimingV1::validated);
    let raw_text = match chat.choices.first().and_then(|c| c.message.as_ref()) {
        Some(msg) => msg.content.clone(),
        None => {
            warn!("MLX response had no message in choices");
            return None;
        },
    };
    let normalization = normalize_provider_output_v1(&raw_text);
    record_provider_output_normalization_v1(&normalization, label, "mlx", profile.as_str());
    let text = normalization.text;
    if text.is_empty() {
        return None;
    }

    // Gibberish gate: reject text that is mostly non-alphabetic.
    // Normal English is 70-85% alpha; degenerate coupling output was ~30%.
    let alpha_count = text.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = text.chars().count();
    if total_count > 3 && (alpha_count as f64 / total_count as f64) < 0.4 {
        warn!(
            "MLX response rejected as degenerate (alpha ratio {:.2}): {}",
            alpha_count as f64 / total_count as f64,
            &text[..text.floor_char_boundary(120)]
        );
        return None;
    }

    let delivery_attempt = capture_submitted_delivery(
        &accepted_route,
        &provider_model,
        &request_bytes,
        &body,
        admission,
    );
    if profile.is_gemma4_canary() {
        match sanitize_gemma4_canary_output_for_label(label, &text) {
            Some(sanitized) if sanitized != text => {
                warn!(
                    "{label}: Gemma 4 profile sanitized legacy selfhood wording before persistence: {}",
                    &text[..text.floor_char_boundary(120)]
                );
                return Some(MlxChatResultV1 {
                    text: sanitized.trim().to_string(),
                    delivery_attempt,
                    qos_request_identity_sha256,
                    request_content_anchor_sha256,
                    queue_wait_ms: provider_timing.map(|timing| timing.0),
                    active_generation_and_reservoir_ms: provider_timing.map(|timing| timing.1),
                });
            },
            Some(_) => {},
            None => {
                warn!(
                    "{label}: Gemma 4 profile response rejected for deprecated runtime language: {}",
                    &text[..text.floor_char_boundary(120)]
                );
                return None;
            },
        }
    }

    Some(MlxChatResultV1 {
        text,
        delivery_attempt,
        qos_request_identity_sha256,
        request_content_anchor_sha256,
        queue_wait_ms: provider_timing.map(|timing| timing.0),
        active_generation_and_reservoir_ms: provider_timing.map(|timing| timing.1),
    })
}

async fn ollama_chat_with_protected_delivery(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    fallback_budget: Option<&FallbackContinuityBudget>,
    protected: Option<&ProtectedDialogueInputV1>,
    context_submission: Option<&ContextSubmissionTrackerV1>,
) -> Option<OllamaFallbackResponse> {
    let client = delivery_http_client(
        timeout_secs,
        protected.is_some() || context_submission.is_some(),
    )?;
    let ollama_url = configured_ollama_url();
    let fallback_models = configured_ollama_fallback_model_chain_for_budget(fallback_budget);
    for fallback_model in fallback_models {
        let messages = if context_submission.is_some_and(ContextSubmissionTrackerV1::submitted) {
            messages
                .iter()
                .filter(|message| {
                    !context_submission.is_some_and(|tracker| {
                        message.content.contains(tracker.exact_content.as_ref())
                    })
                })
                .cloned()
                .collect()
        } else {
            messages.clone()
        };
        let mut request = build_ollama_protected_chat_request(
            label,
            messages,
            temperature,
            max_tokens,
            fallback_model.clone(),
            protected.is_some(),
        );

        let admission = if let Some(input) = protected {
            let Some(admission) =
                admit_protected_dialogue_content(&mut request.messages, input, 16_000)
            else {
                continue;
            };
            Some(admission)
        } else {
            None
        };
        let request_bytes = serde_json::to_vec(&request).ok()?;
        if let Some(tracker) = context_submission {
            tracker.mark_final_messages(&request.messages);
        }
        let response = match client
            .post(&ollama_url)
            .header("Content-Type", "application/json")
            .body(request_bytes.clone())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("Ollama fallback request failed at {ollama_url} with {fallback_model}: {e}");
                continue;
            },
        };
        if !response.status().is_success() {
            warn!(
                "Ollama fallback returned status {} from {ollama_url} with {fallback_model}",
                response.status()
            );
            continue;
        }
        let accepted_route = response.url().to_string();
        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => {
                warn!("Ollama fallback response body read failed with {fallback_model}: {e}");
                continue;
            },
        };
        let chat: ChatResponse = match serde_json::from_str(&body) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "Ollama fallback response parse failed from {ollama_url} with {fallback_model}: {e} — body: {}",
                    &body[..body.floor_char_boundary(200)]
                );
                continue;
            },
        };
        if protected.is_some() && chat.done == Some(false) {
            continue;
        }
        let raw_text = chat
            .message
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let normalization = normalize_provider_output_v1(&raw_text);
        record_provider_output_normalization_v1(&normalization, label, "ollama", &fallback_model);
        let text = normalization.text;
        if !text.is_empty() {
            return Some(OllamaFallbackResponse {
                text,
                delivery_attempt: capture_submitted_delivery(
                    &accepted_route,
                    chat.model
                        .as_deref()
                        .filter(|model| !model.trim().is_empty())
                        .unwrap_or(&fallback_model),
                    &request_bytes,
                    &body,
                    admission,
                ),
                model: fallback_model,
            });
        }
    }
    None
}

fn delivery_http_client(timeout_secs: u64, protected: bool) -> Option<reqwest::Client> {
    let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(timeout_secs));
    // A redirected POST can change method/body. A protected delivery receipt
    // must describe the request actually accepted at the recorded endpoint.
    let builder = if protected {
        builder.redirect(reqwest::redirect::Policy::none())
    } else {
        builder
    };
    builder.build().ok()
}
