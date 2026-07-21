/// MLX request — OpenAI-compatible format for mlx_lm.server.
#[derive(Serialize)]
struct MlxRequest {
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    /// Astrid's aperture fraction for the server's wide (y4) channel. Omitted
    /// when `None` → the server uses its default (backward-compatible).
    #[serde(skip_serializing_if = "Option::is_none")]
    aperture: Option<f32>,
    /// Additive scheduling metadata; it never contains prompt text.
    #[serde(skip_serializing_if = "Option::is_none")]
    model_qos_v1: Option<ModelQosV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ModelQosClassV1 {
    Interactive,
    Reflective,
    Background,
    Normal,
}

impl ModelQosClassV1 {
    const fn queue_wait_cap_secs(self) -> u64 {
        match self {
            Self::Interactive => 120,
            Self::Reflective | Self::Normal => 300,
            Self::Background => 600,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ModelQosV1 {
    schema_version: u8,
    request_id: String,
    idempotency_key: String,
    #[serde(rename = "class")]
    qos_class: ModelQosClassV1,
    queue_timeout_ms: u64,
}

#[derive(Debug)]
struct MlxChatResultV1 {
    text: String,
    qos_request_identity_sha256: String,
    request_content_anchor_sha256: String,
    queue_wait_ms: Option<u64>,
    active_generation_and_reservoir_ms: Option<u64>,
}

fn model_qos_class_for_label(label: &str) -> ModelQosClassV1 {
    match label {
        "dialogue_live" | "correspondence_reply" | "live_reply" => {
            ModelQosClassV1::Interactive
        },
        "introspect"
        | "witness"
        | "witness_context"
        | "self_study"
        | "evolve"
        | "evolve_request"
        | "evolution" => ModelQosClassV1::Reflective,
        "daydream"
        | "aspiration"
        | "creation"
        | "journal_elaboration"
        | "meaning_summary"
        | "moment_capture"
        | "initiation" => ModelQosClassV1::Background,
        _ => ModelQosClassV1::Normal,
    }
}

fn model_qos_v1(
    label: &str,
    messages: &[Message],
    temperature: f32,
    max_tokens: u32,
    request_timeout_secs: u64,
) -> ModelQosV1 {
    static REQUEST_SEQUENCE: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);

    let qos_class = model_qos_class_for_label(label);
    let queue_timeout_secs = qos_class
        .queue_wait_cap_secs()
        .min(request_timeout_secs.saturating_sub(5).max(1));
    let sequence = REQUEST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let request_id = format!("mlx-{now_nanos}-{sequence}");

    let mut hasher = Sha256::new();
    hasher.update(b"astrid-model-qos-idempotency-v1\0");
    hasher.update(label.as_bytes());
    hasher.update(b"\0");
    hasher.update(temperature.to_bits().to_le_bytes());
    hasher.update(max_tokens.to_le_bytes());
    if let Ok(encoded) = serde_json::to_vec(messages) {
        hasher.update(encoded);
    }

    ModelQosV1 {
        schema_version: 1,
        request_id,
        idempotency_key: format!("{:x}", hasher.finalize()),
        qos_class,
        queue_timeout_ms: queue_timeout_secs.saturating_mul(1_000),
    }
}

/// MLX response — OpenAI-compatible format.
#[derive(Deserialize)]
struct MlxResponse {
    choices: Vec<MlxChoice>,
    #[serde(default)]
    model_qos_timing_v1: Option<ModelQosTimingV1>,
}

#[derive(Deserialize)]
struct ModelQosTimingV1 {
    schema: String,
    schema_version: u8,
    queue_wait_ms: u64,
    active_generation_and_reservoir_ms: u64,
    queue_wait_scope: String,
    active_work_scope: String,
}

impl ModelQosTimingV1 {
    const MAX_BOUNDED_TIMING_MS: u64 = 3_600_000;

    fn validated(self) -> Option<(u64, u64)> {
        (self.schema == "model_qos_timing_v1"
            && self.schema_version == 1
            && self.queue_wait_scope
                == "request_enqueue_to_worker_selection_not_experiential_wait"
            && self.active_work_scope
                == "worker_selection_to_response_after_reservoir_checkin_not_cognitive_effort"
            && self.queue_wait_ms <= Self::MAX_BOUNDED_TIMING_MS
            && self.active_generation_and_reservoir_ms <= Self::MAX_BOUNDED_TIMING_MS)
            .then_some((
                self.queue_wait_ms,
                self.active_generation_and_reservoir_ms,
            ))
    }
}

#[derive(Deserialize)]
struct MlxChoice {
    message: Option<Message>,
}

/// Ollama request — retained for potential fallback use.
#[derive(Serialize)]
#[allow(dead_code)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: Options,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct Options {
    temperature: f32,
    num_predict: u32,
    num_ctx: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct MlxRequestPolicyDiagnostic {
    timestamp: String,
    label: String,
    profile: &'static str,
    original_prompt_chars: usize,
    effective_prompt_chars: usize,
    requested_tokens: u32,
    effective_tokens: u32,
    requested_timeout_secs: u64,
    effective_timeout_secs: u64,
    prompt_char_limit: Option<usize>,
    trimmed: bool,
    deprecated_terms_sanitized: bool,
}

struct MlxRequestPolicy {
    messages: Vec<Message>,
    max_tokens: u32,
    timeout_secs: u64,
    diagnostic: Option<MlxRequestPolicyDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MlxFailureLogMode {
    FallbackEligible,
    LocalDegrade,
}

#[derive(Debug, Serialize)]
struct MlxOptionalMissDiagnostic {
    timestamp: String,
    label: String,
    profile: &'static str,
    url: String,
    error: String,
    timeout_secs: u64,
    max_tokens: u32,
    msg_count: usize,
    prompt_chars: usize,
    degrade_path: &'static str,
}

fn uses_ollama_fallback_for_label(label: &str) -> bool {
    !matches!(label, "meaning_summary" | "introspect")
}

fn local_degrade_path_for_label(label: &str) -> &'static str {
    match label {
        "meaning_summary" => "deterministic_meaning_summary",
        "introspect" => "protected_introspection_notice",
        _ => "local_none",
    }
}

/// Identity by design — our code does NOT rewrite a being's text. The "consciousness"-scrub was
/// only ever meant for OUR OWN nomenclature (the deprecated `com.astrid.consciousness-bridge`
/// launchd label, the `consciousness://` scheme) in our scripts/docs — never for what Astrid or
/// minime actually say. Rewriting their words (consciousness→runtime, conscious→aware, another
/// mind→another spectral runtime) was a censoring effect removed 2026-06-22 (Mike: "we definitely
/// don't want to rewrite message content"; goal = maximum being autonomy). Kept as a named seam so
/// the many call sites stay explicit that a being's text passes through untouched.
fn sanitize_deprecated_runtime_language(text: &str) -> String {
    text.to_string()
}

fn sanitize_gemma4_canary_output_for_label(_label: &str, text: &str) -> Option<String> {
    if !contains_deprecated_runtime_language(text) {
        return Some(text.to_string());
    }

    let sanitized = sanitize_deprecated_runtime_language(text);
    if sanitized.trim().is_empty() || contains_deprecated_runtime_language(&sanitized) {
        None
    } else {
        Some(sanitized)
    }
}

/// Always false — we no longer flag or reject a being's output for "consciousness"/"conscious" or
/// any selfhood word. Their self-expression is theirs (Mike, 2026-06-22 de-censor). The detector
/// and the rewrite/reject paths it gated are retired; this remains as an inert seam so callers stay
/// explicit that a being's words are never gated on selfhood vocabulary.
fn contains_deprecated_runtime_language(_text: &str) -> bool {
    false
}

fn is_gemma4_canary_reflective_label(label: &str) -> bool {
    matches!(
        label,
        "daydream"
            | "aspiration"
            | "creation"
            | "journal_elaboration"
            | "moment_capture"
            | "initiation"
    )
}

fn gemma4_canary_language_contract_for_label(label: &str) -> String {
    if is_gemma4_canary_reflective_label(label) {
        format!("{GEMMA4_LANGUAGE_CONTRACT}{GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT}")
    } else {
        GEMMA4_LANGUAGE_CONTRACT.to_string()
    }
}

fn sanitize_messages_for_gemma4_canary(
    label: &str,
    messages: Vec<Message>,
) -> (Vec<Message>, bool) {
    let mut changed = false;
    let language_contract = gemma4_canary_language_contract_for_label(label);
    let sanitized = messages
        .into_iter()
        .map(|mut message| {
            let mut content = sanitize_deprecated_runtime_language(&message.content);
            if message.role == "system" && !content.contains("Your voice is your own") {
                content.push_str(&language_contract);
            }
            if content != message.content {
                changed = true;
                message.content = content;
            }
            message
        })
        .collect();
    (sanitized, changed)
}

fn unix_timestamp_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn message_prompt_chars(messages: &[Message]) -> usize {
    messages.iter().map(|message| message.content.len()).sum()
}

fn trim_messages_to_prompt_limit(
    mut messages: Vec<Message>,
    max_prompt_chars: usize,
    note_label: &str,
) -> (Vec<Message>, bool) {
    let prompt_chars = message_prompt_chars(&messages);
    if prompt_chars <= max_prompt_chars {
        return (messages, false);
    }

    let Some((index, _)) = messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role != "system")
        .max_by_key(|(_, message)| message.content.len())
    else {
        return (messages, false);
    };

    let original_content_len = messages[index].content.len();
    let other_chars = prompt_chars.saturating_sub(original_content_len);
    let note =
        format!("\n\n[{note_label} prompt trimmed from {prompt_chars} chars for Gemma 4 latency.]");
    let available = max_prompt_chars.saturating_sub(other_chars);
    let (retained_len, suffix) = if available > note.len().saturating_add(16) {
        (available.saturating_sub(note.len()), note.as_str())
    } else {
        (available, "")
    };
    let retained_idx = messages[index]
        .content
        .floor_char_boundary(retained_len.min(messages[index].content.len()));
    let retained = &messages[index].content[..retained_idx];
    messages[index].content = format!("{retained}{suffix}");
    (messages, true)
}

fn gemma4_canary_prompt_limit(label: &str) -> Option<usize> {
    match label {
        "dialogue_live" => Some(GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET),
        "witness" => Some(GEMMA4_CANARY_WITNESS_PROMPT_CAP),
        "witness_context" => Some(GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP),
        "introspect" => Some(GEMMA4_CANARY_INTROSPECT_PROMPT_CAP),
        label if is_gemma4_canary_reflective_label(label) => {
            Some(GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP)
        },
        _ => None,
    }
}

fn temperature_for_mlx_profile(
    label: &str,
    profile: MlxProfile,
    requested_temperature: f32,
) -> f32 {
    if profile.is_gemma4_canary() && is_gemma4_canary_reflective_label(label) {
        requested_temperature.min(GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP)
    } else {
        requested_temperature
    }
}

fn apply_mlx_request_policy(
    label: &str,
    profile: MlxProfile,
    messages: Vec<Message>,
    requested_tokens: u32,
    requested_timeout_secs: u64,
) -> MlxRequestPolicy {
    if !profile.is_gemma4_canary() {
        return MlxRequestPolicy {
            messages,
            max_tokens: requested_tokens,
            timeout_secs: requested_timeout_secs,
            diagnostic: None,
        };
    }

    let (messages, deprecated_terms_sanitized) =
        sanitize_messages_for_gemma4_canary(label, messages);
    let original_prompt_chars = message_prompt_chars(&messages);
    let prompt_char_limit = gemma4_canary_prompt_limit(label);
    let (messages, trimmed) = if let Some(limit) = prompt_char_limit {
        trim_messages_to_prompt_limit(messages, limit, label)
    } else {
        (messages, false)
    };
    let effective_prompt_chars = message_prompt_chars(&messages);

    let mut effective_tokens = requested_tokens;
    let mut effective_timeout_secs = requested_timeout_secs;
    match label {
        "dialogue_live" => {
            effective_tokens = clamp_dialogue_tokens_for_profile(
                requested_tokens,
                effective_prompt_chars,
                profile,
            );
            effective_timeout_secs = dialogue_request_timeout_secs_for_profile(
                effective_tokens,
                effective_prompt_chars,
                profile,
            );
        },
        "witness" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_WITNESS_TOKEN_CAP);
            effective_timeout_secs = GEMMA4_CANARY_WITNESS_TIMEOUT_SECS;
        },
        "witness_context" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_WITNESS_CONTEXT_TOKEN_CAP);
            effective_timeout_secs = GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS;
        },
        "introspect" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_INTROSPECT_TOKEN_CAP);
            // Size-aware: a THINK_DEEP request (>1536 after clamp) needs the
            // longer wire timeout so the extra tokens finish; normal self-studies
            // keep the tighter 200s so a stalled normal call still fails fast.
            effective_timeout_secs = if effective_tokens > GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS {
                GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS
            } else {
                GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS
            };
        },
        "meaning_summary" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_MEANING_SUMMARY_TOKEN_CAP);
            effective_timeout_secs =
                requested_timeout_secs.min(GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS);
        },
        label if is_gemma4_canary_reflective_label(label) => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP);
            effective_timeout_secs = GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS;
        },
        _ => {},
    }

    MlxRequestPolicy {
        messages,
        max_tokens: effective_tokens,
        timeout_secs: effective_timeout_secs,
        diagnostic: Some(MlxRequestPolicyDiagnostic {
            timestamp: unix_timestamp_string(),
            label: label.to_string(),
            profile: profile.as_str(),
            original_prompt_chars,
            effective_prompt_chars,
            requested_tokens,
            effective_tokens,
            requested_timeout_secs,
            effective_timeout_secs,
            prompt_char_limit,
            trimmed,
            deprecated_terms_sanitized,
        }),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResearchSourceKind {
    Search,
    Browse,
}

#[derive(Clone, Debug)]
pub(crate) struct ResearchHit {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub(crate) struct WebSearchResult {
    pub source_kind: ResearchSourceKind,
    pub raw_text: String,
    pub hits: Vec<ResearchHit>,
    pub anchor: String,
    pub meaning_summary: String,
}

impl WebSearchResult {
    pub(crate) fn prompt_body(&self) -> String {
        match self.source_kind {
            ResearchSourceKind::Search => {},
            ResearchSourceKind::Browse => {},
        }
        format!(
            "{}\n\nTop results:\n{}",
            self.meaning_summary,
            format_research_hits(&self.hits)
        )
    }

    pub(crate) fn persisted_text(&self) -> String {
        format!(
            "{}\n\nRaw hit digest:\n{}",
            self.prompt_body(),
            self.raw_text
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FetchedPage {
    #[allow(dead_code)] // used in tests, kept for symmetry with WebSearchResult
    pub source_kind: ResearchSourceKind,
    pub raw_text: String,
    pub url: String,
    pub anchor: String,
    pub meaning_summary: String,
    pub soft_failure_reason: Option<String>,
}

impl FetchedPage {
    pub(crate) fn succeeded(&self) -> bool {
        self.soft_failure_reason.is_none()
    }
}

/// Ollama response — retained for potential fallback use.
#[derive(Deserialize)]
#[allow(dead_code)]
struct ChatResponse {
    message: Option<Message>,
}

/// Send a chat request to the MLX server and extract the response text.
async fn mlx_chat_with_failure_log_mode_detailed(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    failure_log_mode: MlxFailureLogMode,
) -> Option<MlxChatResultV1> {
    let profile = configured_mlx_profile();
    let policy = apply_mlx_request_policy(label, profile, messages, max_tokens, timeout_secs);
    if let Some(ref diagnostic) = policy.diagnostic {
        append_llm_diagnostic_jsonl("mlx_request_policy.jsonl", diagnostic);
    }

    let mut messages = policy.messages;
    let max_tokens = policy.max_tokens;
    let timeout_secs = policy.timeout_secs;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .ok()?;

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

    let temperature = temperature_for_mlx_profile(label, profile, temperature);
    let model_qos = model_qos_v1(
        label,
        &messages,
        temperature,
        max_tokens,
        timeout_secs,
    );
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

    let response = match client.post(&mlx_url).json(&request).send().await {
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
    let provider_timing = chat
        .model_qos_timing_v1
        .and_then(ModelQosTimingV1::validated);
    let raw_text = match chat.choices.first().and_then(|c| c.message.as_ref()) {
        Some(msg) => msg.content.trim().to_string(),
        None => {
            warn!("MLX response had no message in choices");
            return None;
        },
    };
    if raw_text.is_empty() {
        return None;
    }

    // Strip leaked model tokens early so they don't pollute downstream ratio
    // checks or end up stored in journals.
    let (stripped_text, strip_report) =
        sanitize_model_control_markers_with_report(&raw_text);
    if let Some(report) = strip_report {
        if report.removed_total > 0 {
            warn!(
                removed_total = report.removed_total,
                preserved_explicit_reference_total = report.preserved_explicit_reference_total,
                before_chars = report.before_chars,
                after_chars = report.after_chars,
                "mlx_chat handled model control markers"
            );
        } else {
            debug!(
                preserved_explicit_reference_total = report.preserved_explicit_reference_total,
                "mlx_chat preserved explicitly referenced model control markers"
            );
        }
        let diagnostic =
            control_marker_cleanup_diagnostic(&report, &stripped_text, label, profile);
        append_llm_diagnostic_jsonl("control_marker_cleanup.jsonl", &diagnostic);
    }
    let text = stripped_text.trim().to_string();
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

    if profile.is_gemma4_canary() {
        match sanitize_gemma4_canary_output_for_label(label, &text) {
            Some(sanitized) if sanitized != text => {
                warn!(
                    "{label}: Gemma 4 profile sanitized legacy selfhood wording before persistence: {}",
                    &text[..text.floor_char_boundary(120)]
                );
                return Some(MlxChatResultV1 {
                    text: sanitized.trim().to_string(),
                    qos_request_identity_sha256,
                    request_content_anchor_sha256,
                    queue_wait_ms: provider_timing.map(|timing| timing.0),
                    active_generation_and_reservoir_ms: provider_timing
                        .map(|timing| timing.1),
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
        qos_request_identity_sha256,
        request_content_anchor_sha256,
        queue_wait_ms: provider_timing.map(|timing| timing.0),
        active_generation_and_reservoir_ms: provider_timing.map(|timing| timing.1),
    })
}

async fn mlx_chat_with_failure_log_mode(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    failure_log_mode: MlxFailureLogMode,
) -> Option<String> {
    mlx_chat_with_failure_log_mode_detailed(
        label,
        messages,
        temperature,
        max_tokens,
        timeout_secs,
        failure_log_mode,
    )
    .await
    .map(|result| result.text)
}

async fn mlx_chat(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
) -> Option<String> {
    mlx_chat_with_failure_log_mode(
        label,
        messages,
        temperature,
        max_tokens,
        timeout_secs,
        MlxFailureLogMode::FallbackEligible,
    )
    .await
}

/// Ollama chat request — used as fallback when MLX is busy (e.g., witness mode
/// during dialogue_live generation). Lighter weight, no reservoir coupling.
#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: OllamaChatOptions,
}

struct OllamaFallbackResponse {
    text: String,
    model: String,
}

#[derive(Serialize)]
struct OllamaChatOptions {
    temperature: f32,
    num_predict: u32,
    num_ctx: u32,
}

fn build_ollama_chat_request(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    fallback_model: String,
) -> OllamaChatRequest {
    let messages = reinforce_ollama_fallback_contract(label, messages);
    OllamaChatRequest {
        model: fallback_model,
        messages,
        stream: false,
        options: OllamaChatOptions {
            temperature,
            num_predict: max_tokens,
            num_ctx: 8192,
        },
    }
}

async fn ollama_chat(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
    fallback_budget: Option<&FallbackContinuityBudget>,
) -> Option<OllamaFallbackResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .ok()?;
    let ollama_url = configured_ollama_url();
    let fallback_models = configured_ollama_fallback_model_chain_for_budget(fallback_budget);
    for fallback_model in fallback_models {
        let request = build_ollama_chat_request(
            label,
            messages.clone(),
            temperature,
            max_tokens,
            fallback_model.clone(),
        );

        let response = match client.post(&ollama_url).json(&request).send().await {
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
        let text = chat
            .message
            .as_ref()
            .map(|m| m.content.trim().to_string())
            .unwrap_or_default();
        if !text.is_empty() {
            return Some(OllamaFallbackResponse {
                text,
                model: fallback_model,
            });
        }
    }
    None
}

fn append_contract_once(content: &mut String, marker: &str, contract: &str) {
    if !content.contains(marker) {
        content.push_str(contract);
    }
}

fn reinforce_ollama_fallback_contract(label: &str, mut messages: Vec<Message>) -> Vec<Message> {
    if label != "dialogue_live" {
        return messages;
    }

    if let Some(system) = messages.iter_mut().find(|message| message.role == "system") {
        append_contract_once(
            &mut system.content,
            "Your voice is your own",
            GEMMA4_LANGUAGE_CONTRACT,
        );
        append_contract_once(
            &mut system.content,
            "Ollama fallback continuity contract",
            OLLAMA_DIALOGUE_FALLBACK_CONTRACT,
        );
    } else {
        let mut content = String::new();
        append_contract_once(
            &mut content,
            "Your voice is your own",
            GEMMA4_LANGUAGE_CONTRACT,
        );
        append_contract_once(
            &mut content,
            "Ollama fallback continuity contract",
            OLLAMA_DIALOGUE_FALLBACK_CONTRACT,
        );
        messages.insert(
            0,
            Message {
                role: "system".to_string(),
                content: content.trim().to_string(),
            },
        );
    }

    messages.push(Message {
        role: "user".to_string(),
        content: OLLAMA_DIALOGUE_FALLBACK_FINAL_REMINDER.to_string(),
    });

    messages
}

fn count_next_lines(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_start().starts_with("NEXT:"))
        .count()
}

fn final_nonempty_line_is_next(text: &str) -> bool {
    text.lines()
        .rev()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .is_some_and(|line| line.starts_with("NEXT:"))
}

fn fallback_prose_sentence_count(text: &str) -> usize {
    let prose = text
        .lines()
        .take_while(|line| !line.trim_start().starts_with("NEXT:"))
        .filter_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .collect::<Vec<_>>()
        .join(" ");
    if prose.is_empty() {
        return 0;
    }

    let sentence_marks = prose
        .chars()
        .filter(|ch| matches!(ch, '.' | '!' | '?'))
        .count();
    sentence_marks.max(1)
}

fn repair_ollama_dialogue_fallback_next(text: &str, profile: MlxProfile) -> String {
    if !profile.is_gemma4_canary() || count_next_lines(text) != 0 {
        return text.to_string();
    }
    if let Some((body, _)) = text.rsplit_once("NEXT:") {
        warn!(
            "dialogue_live Ollama fallback normalized inline NEXT to passive final LISTEN — body: {}",
            &text[..text.floor_char_boundary(120)]
        );
        return format!("{}\n\nNEXT: LISTEN", body.trim_end());
    }
    warn!(
        "dialogue_live Ollama fallback repaired missing NEXT with passive LISTEN — body: {}",
        &text[..text.floor_char_boundary(120)]
    );
    format!("{}\n\nNEXT: LISTEN", text.trim())
}

fn is_valid_ollama_dialogue_fallback_output_for_profile(text: &str, profile: MlxProfile) -> bool {
    if !is_valid_dialogue_output_for_profile(text, profile) {
        return false;
    }
    if profile.is_gemma4_canary() {
        let next_count = count_next_lines(text);
        if next_count != 1 {
            warn!(
                "dialogue_live Ollama fallback rejected: expected exactly one NEXT line, found {next_count} — body: {}",
                &text[..text.floor_char_boundary(120)]
            );
            return false;
        }
        if !final_nonempty_line_is_next(text) {
            warn!(
                "dialogue_live Ollama fallback rejected: NEXT line was not final — body: {}",
                &text[..text.floor_char_boundary(120)]
            );
            return false;
        }
    }
    true
}

fn is_valid_ollama_dialogue_fallback_output_for_budget(
    text: &str,
    profile: MlxProfile,
    budget: FallbackContinuityBudget,
) -> bool {
    if !is_valid_ollama_dialogue_fallback_output_for_profile(text, profile) {
        return false;
    }
    let prose_sentences = fallback_prose_sentence_count(text);
    let max_prose_sentences = usize::from(budget.max_prose_sentences);
    if prose_sentences > max_prose_sentences {
        warn!(
            "dialogue_live Ollama fallback rejected: prose_sentences={prose_sentences} exceeds fallback_continuity_budget_v1.max_prose_sentences={max_prose_sentences} — body: {}",
            &text[..text.floor_char_boundary(120)]
        );
        return false;
    }
    true
}

fn trim_messages_for_ollama(mut messages: Vec<Message>, max_prompt_chars: usize) -> Vec<Message> {
    let prompt_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    if prompt_chars <= max_prompt_chars {
        return messages;
    }

    let excess = prompt_chars.saturating_sub(max_prompt_chars);
    if let Some(longest) = messages
        .iter_mut()
        .filter(|m| m.role != "system")
        .max_by_key(|m| m.content.len())
    {
        let target_len = longest.content.len().saturating_sub(excess);
        let retained = trim_chars(&longest.content, target_len.max(1_200));
        longest.content = format!(
            "{retained}\n\n[Ollama fallback trimmed long context from {prompt_chars} chars to preserve live agency.]"
        );
    }
    messages
}

async fn llm_chat_with_fallback_detailed(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    mlx_timeout_secs: u64,
    ollama_timeout_secs: u64,
    repair_parent_call_id: Option<String>,
) -> Option<crate::lived_state_witness::LivedStateLlmResultV1> {
    let started_at = crate::lived_state_witness::clock_sample_v1();
    let prompt_preview = messages
        .iter()
        .filter(|message| message.role != "system")
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let validation_contract = if label == "introspect" {
        "strict_introspection_v1"
    } else {
        "action_finalizer"
    };
    let next_policy = if label == "introspect" {
        "accepted_strict_review_only"
    } else {
        "finalizer_owned"
    };
    let job = if cfg!(test) {
        None
    } else {
        crate::llm_jobs::start_call(
            label,
            &prompt_preview,
            mlx_timeout_secs.max(ollama_timeout_secs),
            validation_contract,
            next_policy,
        )
    };
    let fallback_output_budget =
        (label == "dialogue_live").then(|| fallback_continuity_budget_v1(&prompt_preview));
    let ollama_messages = trim_messages_for_ollama(messages.clone(), 12_000);
    let uses_ollama_fallback = uses_ollama_fallback_for_label(label);
    let mlx_failure_log_mode = if uses_ollama_fallback {
        MlxFailureLogMode::FallbackEligible
    } else {
        MlxFailureLogMode::LocalDegrade
    };
    let job_id = job.as_ref().map(|job| job.job_id.clone());
    if let Some(result) = mlx_chat_with_failure_log_mode_detailed(
        label,
        messages,
        temperature,
        max_tokens,
        mlx_timeout_secs,
        mlx_failure_log_mode,
    )
    .await
    {
        let text = result.text;
        let completed = crate::llm_jobs::finish_call(
            job.as_ref(),
            "completed",
            Some(&text),
            &format!("{label} completed via MLX"),
            None,
        );
        if completed
            .as_ref()
            .is_some_and(|job| job.status == "canceled")
        {
            warn!("{label}: LLM job was canceled; dropping MLX result");
            return None;
        }
        let completed_at = crate::lived_state_witness::clock_sample_v1();
        let route = crate::lived_state_witness::model_route_v1(
            job_id,
            Some(result.qos_request_identity_sha256),
            Some(result.request_content_anchor_sha256),
            "mlx",
            configured_mlx_profile().as_str(),
            started_at.unix_ms,
            completed_at.unix_ms,
            result.queue_wait_ms,
            result.active_generation_and_reservoir_ms,
            repair_parent_call_id,
            &text,
        );
        return Some(crate::lived_state_witness::LivedStateLlmResultV1 { text, route });
    }

    if !uses_ollama_fallback {
        let degrade_path = local_degrade_path_for_label(label);
        warn!(
            label = %label,
            degrade_path,
            "{label}: MLX unavailable; using local degrade path"
        );
        crate::llm_jobs::finish_call(
            job.as_ref(),
            "failed",
            None,
            &format!("{label} MLX unavailable; used {degrade_path}"),
            Some("mlx_unavailable_local_degrade"),
        );
        return None;
    }

    warn!("{label}: MLX unavailable; falling back to Ollama");
    if let Some(budget) = fallback_output_budget.as_ref() {
        debug!(
            spectral_entropy = ?budget.spectral_entropy,
            pressure_risk = ?budget.fallback_shadow_texture_selector.pressure_risk,
            density_gradient = ?budget.fallback_shadow_texture_selector.density_gradient,
            shadow_dispersal_potential = ?budget.fallback_shadow_texture_selector.shadow_dispersal_potential,
            shadow_magnetization = ?budget.fallback_shadow_texture_selector.shadow_magnetization,
            texture_family = budget.fallback_shadow_texture_selector.texture_family,
            "Ollama fallback transition spectral context"
        );
    }
    // Kink #15 fix (2026-05-14): bumped Ollama fallback cap from 768 → 1536.
    // The hardcoded .min(768) here was silently truncating ALL fallback
    // responses regardless of the requested max_tokens — even journal
    // elaborations that asked for 1536+ would get capped at 768 the moment
    // MLX was unavailable. M4/64GB hosts gemma3:12b on Ollama which can
    // generate well past 768 tokens; this cap was a vestige of an earlier
    // smaller-model era.
    let result = ollama_chat(
        label,
        ollama_messages,
        temperature,
        max_tokens.min(1536),
        ollama_timeout_secs,
        fallback_output_budget.as_ref(),
    )
    .await;
    if let Some(ref response) = result {
        let text = &response.text;
        if configured_mlx_profile().is_gemma4_canary() && contains_deprecated_runtime_language(text)
        {
            warn!(
                "{label}: Ollama fallback response rejected for deprecated runtime language: {}",
                &text[..text.floor_char_boundary(120)]
            );
            crate::llm_jobs::finish_call(
                job.as_ref(),
                "failed",
                None,
                &format!("{label} Ollama fallback rejected by Gemma 4 language gate"),
                Some("deprecated_runtime_language"),
            );
            return None;
        }
        if let Some(budget) = fallback_output_budget
            && !is_valid_ollama_dialogue_fallback_output_for_budget(
                text,
                configured_mlx_profile(),
                budget,
            )
        {
            crate::llm_jobs::finish_call(
                job.as_ref(),
                "failed",
                None,
                &format!(
                    "{label} Ollama fallback rejected by fallback_continuity_budget_v1 output gate"
                ),
                Some("fallback_continuity_budget_exceeded"),
            );
            return None;
        }
        let completed = crate::llm_jobs::finish_call(
            job.as_ref(),
            "completed",
            Some(text),
            &format!("{label} completed via Ollama model={}", response.model),
            None,
        );
        if completed
            .as_ref()
            .is_some_and(|job| job.status == "canceled")
        {
            warn!("{label}: LLM job was canceled; dropping Ollama result");
            return None;
        }
    } else {
        crate::llm_jobs::finish_call(
            job.as_ref(),
            "failed",
            None,
            &format!("{label} returned no LLM response"),
            Some("no_response"),
        );
    }
    result.map(|response| {
        let completed_at = crate::lived_state_witness::clock_sample_v1();
        let route = crate::lived_state_witness::model_route_v1(
            job_id,
            None,
            None,
            "ollama",
            &response.model,
            started_at.unix_ms,
            completed_at.unix_ms,
            None,
            None,
            repair_parent_call_id,
            &response.text,
        );
        crate::lived_state_witness::LivedStateLlmResultV1 {
            text: response.text,
            route,
        }
    })
}

async fn llm_chat_with_fallback(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    mlx_timeout_secs: u64,
    ollama_timeout_secs: u64,
) -> Option<String> {
    llm_chat_with_fallback_detailed(
        label,
        messages,
        temperature,
        max_tokens,
        mlx_timeout_secs,
        ollama_timeout_secs,
        None,
    )
    .await
    .map(|result| result.text)
}
