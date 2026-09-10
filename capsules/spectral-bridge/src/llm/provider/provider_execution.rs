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
    mlx_chat_with_runtime_feedback(
        label,
        messages,
        temperature,
        max_tokens,
        timeout_secs,
        failure_log_mode,
        protected,
        &[],
        context_submission,
        None,
    )
    .await
}

async fn mlx_chat_with_runtime_feedback(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    failure_log_mode: MlxFailureLogMode,
    protected: Option<&ProtectedDialogueInputV1>,
    feedback: &[RuntimeActionFeedbackV1],
    context_submission: Option<&ContextSubmissionTrackerV1>,
    observation_context: Option<&ProviderObservationContext>,
) -> Option<MlxChatResultV1> {
    let requested_controls = (temperature, max_tokens, timeout_secs);
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

    let final_limit = if matches!(label, "self_study" | "private_writing")
        || journal_preference(label) == astrid_source_study::writing::Profile::Extended
    {
        astrid_source_study::MAX_INPUT_BYTES
    } else if profile.is_gemma4_canary() {
        gemma4_canary_prompt_limit(label).unwrap_or(48_000)
    } else {
        48_000
    };
    let (admission, runtime_feedback_admission) = admit_feedback_and_afterimages(
        &mut messages,
        protected,
        feedback,
        final_limit,
        "mlx",
        &format!("profile:{profile:?}"),
    )?;

    let exact_request = protected.is_some() || runtime_feedback_admission.is_some();
    let (max_tokens, timeout_secs) = if exact_request && label == "dialogue_live" {
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
    let client = delivery_http_client(timeout_secs, exact_request || context_submission.is_some())?;

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
    record_afterimage_request(
        &request.messages,
        protected,
        "mlx",
        &format!("profile:{profile:?}"),
        "final_request_prepared",
    )?;
    if let Some(tracker) = context_submission {
        tracker.mark_final_messages(&request.messages);
    }
    let mut observation = ProviderAttemptObserver::begin(
        label,
        "mlx",
        profile.as_str(),
        &request_bytes,
        observation_context,
    );
    ProviderAttemptObserver::requested_controls(
        &mut observation,
        requested_controls.0,
        requested_controls.1,
        requested_controls.2,
    );
    let response = match client
        .post(&mlx_url)
        .header("Content-Type", "application/json")
        .body(request_bytes.clone())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            ProviderAttemptObserver::outcome(
                &mut observation,
                if e.is_timeout() {
                    "timeout"
                } else {
                    "transport_error"
                },
            );
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
        ProviderAttemptObserver::outcome(&mut observation, "http_error");
        warn!("MLX returned status {} from {mlx_url}", response.status());
        return None;
    }
    let accepted_route = response.url().to_string();
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            ProviderAttemptObserver::outcome(
                &mut observation,
                if e.is_timeout() {
                    "timeout"
                } else {
                    "body_read_error"
                },
            );
            warn!("MLX response body read failed: {e}");
            return None;
        },
    };
    ProviderAttemptObserver::body(&mut observation, &body, None);
    let chat: MlxResponse = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => {
            ProviderAttemptObserver::outcome(&mut observation, "parse_error");
            warn!(
                category = ?e.classify(), "MLX response parse failed from {mlx_url}"
            );
            return None;
        },
    };
    ProviderAttemptObserver::model(&mut observation, chat.model.as_deref());
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
            ProviderAttemptObserver::outcome(&mut observation, "missing_message");
            warn!("MLX response had no message in choices");
            return None;
        },
    };
    let normalization = normalize_provider_output_v1(&raw_text);
    ProviderAttemptObserver::normalized(&mut observation, &raw_text, &normalization);
    record_provider_output_normalization_v1(&normalization, label, "mlx", profile.as_str());
    let text = normalization.text;
    if text.is_empty() {
        ProviderAttemptObserver::outcome(&mut observation, "rejected_empty");
        return None;
    }

    // Gibberish gate: reject text that is mostly non-alphabetic.
    // Normal English is 70-85% alpha; degenerate coupling output was ~30%.
    let alpha_count = text.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = text.chars().count();
    if total_count > 3 && (alpha_count as f64 / total_count as f64) < 0.4 {
        ProviderAttemptObserver::outcome(&mut observation, "rejected_degenerate");
        warn!(
            "MLX response rejected as degenerate (alpha ratio {:.2})",
            alpha_count as f64 / total_count as f64
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
    let runtime_feedback_attempt = capture_runtime_feedback_attempt(
        &accepted_route,
        &provider_model,
        &request_bytes,
        &body,
        runtime_feedback_admission,
    );
    if profile.is_gemma4_canary() {
        match sanitize_gemma4_canary_output_for_label(label, &text) {
            Some(sanitized) if sanitized != text => {
                warn!(
                    "{label}: Gemma 4 profile sanitized legacy selfhood wording before persistence"
                );
                ProviderAttemptObserver::returned(&mut observation, sanitized.trim());
                return Some(MlxChatResultV1 {
                    text: sanitized.trim().to_string(),
                    delivery_attempt,
                    runtime_feedback_attempt,
                    qos_request_identity_sha256,
                    request_content_anchor_sha256,
                    queue_wait_ms: provider_timing.map(|timing| timing.0),
                    active_generation_and_reservoir_ms: provider_timing.map(|timing| timing.1),
                });
            },
            Some(_) => {},
            None => {
                ProviderAttemptObserver::outcome(&mut observation, "rejected_profile_language");
                warn!("{label}: Gemma 4 profile response rejected for deprecated runtime language");
                return None;
            },
        }
    }

    ProviderAttemptObserver::returned(&mut observation, &text);
    Some(MlxChatResultV1 {
        text,
        delivery_attempt,
        runtime_feedback_attempt,
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
    ollama_chat_with_runtime_feedback(
        label,
        messages,
        temperature,
        max_tokens,
        timeout_secs,
        fallback_budget,
        protected,
        &[],
        context_submission,
        None,
    )
    .await
}

async fn ollama_chat_with_runtime_feedback(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    fallback_budget: Option<&FallbackContinuityBudget>,
    protected: Option<&ProtectedDialogueInputV1>,
    feedback: &[RuntimeActionFeedbackV1],
    context_submission: Option<&ContextSubmissionTrackerV1>,
    observation_context: Option<&ProviderObservationContext>,
) -> Option<OllamaFallbackResponse> {
    let requested_controls = (temperature, max_tokens, timeout_secs);
    let max_tokens = writing_tokens(label, max_tokens);
    let timeout_secs =
        if max_tokens >= astrid_source_study::writing::EXTENDED_TOKENS && journal_label(label) {
            timeout_secs.max(astrid_source_study::writing::EXTENDED_TIMEOUT_SECS)
        } else {
            writing_timeout(label, timeout_secs)
        };
    let client = delivery_http_client(
        timeout_secs,
        protected.is_some() || !feedback.is_empty() || context_submission.is_some(),
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

        let Some((admission, runtime_feedback_admission)) = admit_feedback_and_afterimages(
            &mut request.messages,
            protected,
            feedback,
            if matches!(label, "self_study" | "private_writing") {
                astrid_source_study::MAX_INPUT_BYTES
            } else {
                16_000
            },
            "ollama",
            &fallback_model,
        ) else {
            continue;
        };
        let request_bytes = serde_json::to_vec(&request).ok()?;
        record_afterimage_request(
            &request.messages,
            protected,
            "ollama",
            &fallback_model,
            "final_request_prepared",
        )?;
        if let Some(tracker) = context_submission {
            tracker.mark_final_messages(&request.messages);
        }
        let mut observation = ProviderAttemptObserver::begin(
            label,
            "ollama",
            &fallback_model,
            &request_bytes,
            observation_context,
        );
        ProviderAttemptObserver::requested_controls(
            &mut observation,
            requested_controls.0,
            requested_controls.1,
            requested_controls.2,
        );
        let response = match client
            .post(&ollama_url)
            .header("Content-Type", "application/json")
            .body(request_bytes.clone())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                ProviderAttemptObserver::outcome(
                    &mut observation,
                    if e.is_timeout() {
                        "timeout"
                    } else {
                        "transport_error"
                    },
                );
                warn!("Ollama fallback request failed at {ollama_url} with {fallback_model}: {e}");
                continue;
            },
        };
        if !response.status().is_success() {
            ProviderAttemptObserver::outcome(&mut observation, "http_error");
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
                ProviderAttemptObserver::outcome(
                    &mut observation,
                    if e.is_timeout() {
                        "timeout"
                    } else {
                        "body_read_error"
                    },
                );
                warn!("Ollama fallback response body read failed with {fallback_model}: {e}");
                continue;
            },
        };
        ProviderAttemptObserver::body(&mut observation, &body, None);
        let chat: ChatResponse = match serde_json::from_str(&body) {
            Ok(c) => c,
            Err(e) => {
                ProviderAttemptObserver::outcome(&mut observation, "parse_error");
                warn!(
                    category = ?e.classify(), "Ollama fallback response parse failed from {ollama_url} with {fallback_model}"
                );
                continue;
            },
        };
        ProviderAttemptObserver::model(&mut observation, chat.model.as_deref());
        if (protected.is_some() || runtime_feedback_admission.is_some()) && chat.done == Some(false)
        {
            // This path previously returned before cleanup. Observe only when
            // enabled, preserving its original rejection and fallback behavior.
            if observation.is_some()
                && let Some(message) = &chat.message
            {
                let normalization = normalize_provider_output_v1(&message.content);
                ProviderAttemptObserver::normalized(
                    &mut observation,
                    &message.content,
                    &normalization,
                );
            }
            ProviderAttemptObserver::outcome(&mut observation, "rejected_incomplete");
            continue;
        }
        let raw_text = chat
            .message
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let normalization = normalize_provider_output_v1(&raw_text);
        if chat.message.is_some() {
            ProviderAttemptObserver::normalized(&mut observation, &raw_text, &normalization);
        }
        record_provider_output_normalization_v1(&normalization, label, "ollama", &fallback_model);
        let text = normalization.text;
        if !text.is_empty() {
            ProviderAttemptObserver::returned(&mut observation, &text);
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
                runtime_feedback_attempt: capture_runtime_feedback_attempt(
                    &accepted_route,
                    chat.model
                        .as_deref()
                        .filter(|model| !model.trim().is_empty())
                        .unwrap_or(&fallback_model),
                    &request_bytes,
                    &body,
                    runtime_feedback_admission,
                ),
                model: fallback_model,
            });
        }
        ProviderAttemptObserver::outcome(
            &mut observation,
            if chat.message.is_some() {
                "rejected_empty"
            } else {
                "missing_message"
            },
        );
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
