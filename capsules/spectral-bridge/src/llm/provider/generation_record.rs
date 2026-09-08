// Steward-only record of every `dialogue_live` generation.
//
// Tranche 1 (2026-09-06, Mike & Claude). One JSON file per backend attempt
// under `workspace/generations/<UTC day>/` (0600 files in 0700 directories);
// the system prompt is deduplicated by sha256 into
// `workspace/generations/system_prompts/<sha>.txt`.
//
// Why: the live dialogue lane persisted no prompt, and the Ollama fallback's
// served model was discarded, so nothing could say which model wrote a given
// journal entry or what it saw. These records are never read back into a
// prompt; they exist for the stewards' analysis only (reservoir-llm-research).
// Every failure is logged and swallowed.

const GENERATION_RECORD_SCHEMA_VERSION: u32 = 1;
const GENERATION_RECORD_ENABLED_ENV: &str = "ASTRID_GENERATION_RECORD";
const GENERATION_RECORD_DIR_ENV: &str = "ASTRID_GENERATION_RECORD_DIR";
const GENERATION_RECORD_SUBDIR: &str = "generations";
const GENERATION_RECORD_SYSTEM_PROMPTS_SUBDIR: &str = "system_prompts";
const GENERATION_RECORD_BEING: &str = "astrid";
const GENERATION_RECORD_LANE: &str = "dialogue_live";
const DIALOGUE_CONTRACT_V3: &str = "dialogue_prompt_v3";
const DIALOGUE_CONTRACT_V4_OWN_BODY: &str = "dialogue_prompt_v4_own_body";
const GENERATION_BACKEND_PRIMARY: &str = "mlx_coupled";
const GENERATION_BACKEND_FALLBACK: &str = "ollama_fallback";
const GENERATION_STATUS_OK: &str = "ok";
const GENERATION_STATUS_REJECTED: &str = "rejected_quality_gate";
const GENERATION_STATUS_UNAVAILABLE: &str = "unavailable_or_timeout";
/// Primary (coupled MLX) plus the Ollama fallback.
const GENERATION_ATTEMPTS_PLANNED: u32 = 2;
/// The Ollama fallback's request timeout in `generate_dialogue` (was a bare literal).
const DIALOGUE_OLLAMA_FALLBACK_TIMEOUT_SECS: u64 = 75;
static GENERATION_RECORD_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
struct GenerationRecordMessageV1 {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_sha256: Option<String>,
    chars: usize,
}

#[derive(Debug, Clone, Serialize)]
struct GenerationRecordOwnBodyV1 {
    present: bool,
    status: &'static str,
    chars: usize,
    trimmed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct GenerationRecordPromptV1 {
    fill_pct: f32,
    requested_tokens: u32,
    effective_tokens: u32,
    final_prompt_chars: usize,
    user_content_budget: usize,
    mlx_profile: String,
}

#[derive(Debug, Clone, Serialize)]
struct GenerationRecordV1 {
    schema_version: u32,
    generation_id: String,
    being: &'static str,
    lane: &'static str,
    contract_version: &'static str,
    model: String,
    backend: &'static str,
    attempt_index: u32,
    attempts_total: u32,
    fallback_used: bool,
    timeout_s: u64,
    elapsed_s: f64,
    status: &'static str,
    messages: Vec<GenerationRecordMessageV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_sha256: Option<String>,
    response_chars: usize,
    own_body: GenerationRecordOwnBodyV1,
    prompt: GenerationRecordPromptV1,
    linked_artifacts: Vec<serde_json::Value>,
    pid: u32,
    created_at_unix_ms: u128,
}

/// Facts shared by every attempt of one dialogue generation, captured before
/// the message vectors are moved into the transport calls.
struct DialogueGenerationRecordContext {
    generation_id: String,
    contract_version: &'static str,
    primary_messages: Vec<GenerationRecordMessageV1>,
    fallback_messages: Vec<GenerationRecordMessageV1>,
    system_prompts: Vec<(String, String)>,
    own_body: GenerationRecordOwnBodyV1,
    prompt: GenerationRecordPromptV1,
    linked_artifacts: Vec<serde_json::Value>,
}

struct DialogueGenerationPromptFacts {
    fill_pct: f32,
    requested_tokens: u32,
    effective_tokens: u32,
    final_prompt_chars: usize,
    user_content_budget: usize,
    mlx_profile: &'static str,
}

struct DialogueGenerationAttempt {
    backend: &'static str,
    model: String,
    attempt_index: u32,
    timeout_s: u64,
    elapsed_s: f64,
    status: &'static str,
    response_text: Option<String>,
}

impl DialogueGenerationRecordContext {
    fn capture(
        primary: &[Message],
        fallback: &[Message],
        own_body: &OwnBodyFetch,
        facts: DialogueGenerationPromptFacts,
        budget_report: Option<&PromptBudgetReport>,
        overflow_path: Option<String>,
    ) -> Self {
        let (primary_messages, mut system_prompts) = snapshot_generation_messages(primary);
        let (fallback_messages, fallback_prompts) = snapshot_generation_messages(fallback);
        for (sha, text) in fallback_prompts {
            if !system_prompts.iter().any(|(known, _)| known == &sha) {
                system_prompts.push((sha, text));
            }
        }
        let own_body_trimmed = budget_report
            .map(|report| {
                report.trimmed_blocks.iter().any(|block| {
                    block.label == OWN_BODY_BLOCK_LABEL && block.removed_chars > 0
                })
            })
            .unwrap_or(false);
        let own_body_chars = own_body
            .line
            .as_deref()
            .map(|line| line.chars().count())
            .unwrap_or(0);
        Self {
            generation_id: generation_record_id(),
            contract_version: if own_body.line.is_some() {
                DIALOGUE_CONTRACT_V4_OWN_BODY
            } else {
                DIALOGUE_CONTRACT_V3
            },
            primary_messages,
            fallback_messages,
            system_prompts,
            own_body: GenerationRecordOwnBodyV1 {
                present: own_body.line.is_some(),
                status: own_body.status,
                chars: own_body_chars,
                trimmed: own_body_trimmed,
            },
            prompt: GenerationRecordPromptV1 {
                fill_pct: facts.fill_pct,
                requested_tokens: facts.requested_tokens,
                effective_tokens: facts.effective_tokens,
                final_prompt_chars: facts.final_prompt_chars,
                user_content_budget: facts.user_content_budget,
                mlx_profile: facts.mlx_profile.to_string(),
            },
            linked_artifacts: overflow_path
                .into_iter()
                .map(|path| serde_json::json!({ "kind": "context_overflow", "path": path }))
                .collect(),
        }
    }

    fn replace_fallback_messages(&mut self, fallback: &[Message]) {
        let (messages, prompts) = snapshot_generation_messages(fallback);
        self.fallback_messages = messages;
        for (sha, text) in prompts {
            if !self.system_prompts.iter().any(|(known, _)| known == &sha) {
                self.system_prompts.push((sha, text));
            }
        }
    }
}

fn generation_record_id() -> String {
    let sequence =
        GENERATION_RECORD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{}-{sequence}", generation_unix_ms(), std::process::id())
}

fn generation_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn generation_sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// System-role content becomes a sha256 reference; other roles are kept verbatim.
fn snapshot_generation_messages(
    messages: &[Message],
) -> (Vec<GenerationRecordMessageV1>, Vec<(String, String)>) {
    let mut system_prompts: Vec<(String, String)> = Vec::new();
    let snapshot = messages
        .iter()
        .map(|message| {
            let chars = message.content.chars().count();
            if message.role == "system" {
                let sha = generation_sha256_hex(&message.content);
                if !system_prompts.iter().any(|(known, _)| known == &sha) {
                    system_prompts.push((sha.clone(), message.content.clone()));
                }
                GenerationRecordMessageV1 {
                    role: message.role.clone(),
                    content: None,
                    content_sha256: Some(sha),
                    chars,
                }
            } else {
                GenerationRecordMessageV1 {
                    role: message.role.clone(),
                    content: Some(message.content.clone()),
                    content_sha256: None,
                    chars,
                }
            }
        })
        .collect();
    (snapshot, system_prompts)
}

fn generation_attempt_status(raw: Option<&str>, accepted: Option<&str>) -> &'static str {
    match (raw, accepted) {
        (None, _) => GENERATION_STATUS_UNAVAILABLE,
        (Some(_), None) => GENERATION_STATUS_REJECTED,
        (Some(_), Some(_)) => GENERATION_STATUS_OK,
    }
}

fn build_dialogue_generation_record(
    ctx: &DialogueGenerationRecordContext,
    attempt: DialogueGenerationAttempt,
) -> GenerationRecordV1 {
    let messages = if attempt.attempt_index == 0 {
        ctx.primary_messages.clone()
    } else {
        ctx.fallback_messages.clone()
    };
    let response_sha256 = attempt.response_text.as_deref().map(generation_sha256_hex);
    let response_chars = attempt
        .response_text
        .as_deref()
        .map(|text| text.chars().count())
        .unwrap_or(0);
    GenerationRecordV1 {
        schema_version: GENERATION_RECORD_SCHEMA_VERSION,
        generation_id: ctx.generation_id.clone(),
        being: GENERATION_RECORD_BEING,
        lane: GENERATION_RECORD_LANE,
        contract_version: ctx.contract_version,
        model: attempt.model,
        backend: attempt.backend,
        attempt_index: attempt.attempt_index,
        attempts_total: GENERATION_ATTEMPTS_PLANNED,
        fallback_used: attempt.attempt_index > 0,
        timeout_s: attempt.timeout_s,
        elapsed_s: attempt.elapsed_s,
        status: attempt.status,
        messages,
        response_text: attempt.response_text,
        response_sha256,
        response_chars,
        own_body: ctx.own_body.clone(),
        prompt: ctx.prompt.clone(),
        linked_artifacts: ctx.linked_artifacts.clone(),
        pid: std::process::id(),
        created_at_unix_ms: generation_unix_ms(),
    }
}

fn record_dialogue_attempt(
    ctx: &DialogueGenerationRecordContext,
    attempt: DialogueGenerationAttempt,
) {
    let record = build_dialogue_generation_record(ctx, attempt);
    append_generation_record(&record, &ctx.system_prompts);
}

fn generation_record_enabled() -> bool {
    !matches!(
        std::env::var(GENERATION_RECORD_ENABLED_ENV)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Some("off" | "0" | "false" | "no")
    )
}

fn generation_record_dir() -> std::path::PathBuf {
    match std::env::var(GENERATION_RECORD_DIR_ENV) {
        Ok(value) if !value.trim().is_empty() => std::path::PathBuf::from(value.trim()),
        _ => bridge_paths()
            .bridge_workspace()
            .join(GENERATION_RECORD_SUBDIR),
    }
}

fn generation_record_day() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Create the directory if needed and keep it steward-only (0700), tightening
/// a pre-existing looser mode rather than trusting it.
fn generation_record_ensure_private_dir(path: &std::path::Path) -> std::io::Result<()> {
    if !path.is_dir() {
        std::fs::create_dir_all(path)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o700 {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Create-new 0600 write; `Ok(false)` when the file already exists.
fn generation_record_write_once(path: &std::path::Path, content: &str) -> std::io::Result<bool> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(content.as_bytes())?;
            Ok(true)
        },
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

fn append_generation_record_at(
    dir: &std::path::Path,
    record: &GenerationRecordV1,
    system_prompts: &[(String, String)],
) -> std::io::Result<std::path::PathBuf> {
    generation_record_ensure_private_dir(dir)?;
    let day_dir = dir.join(generation_record_day());
    generation_record_ensure_private_dir(&day_dir)?;
    if !system_prompts.is_empty() {
        let prompts_dir = dir.join(GENERATION_RECORD_SYSTEM_PROMPTS_SUBDIR);
        generation_record_ensure_private_dir(&prompts_dir)?;
        for (sha, text) in system_prompts {
            generation_record_write_once(&prompts_dir.join(format!("{sha}.txt")), text)?;
        }
    }
    let payload = serde_json::to_string(record)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let base = format!(
        "gen_{}_{}_a{}",
        record.created_at_unix_ms, record.lane, record.attempt_index
    );
    for suffix in ["", "_1", "_2", "_3", "_4"] {
        let path = day_dir.join(format!("{base}{suffix}.json"));
        if generation_record_write_once(&path, &payload)? {
            return Ok(path);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "generation record name collision",
    ))
}

fn append_generation_record(record: &GenerationRecordV1, system_prompts: &[(String, String)]) {
    if cfg!(test) || !generation_record_enabled() {
        return;
    }
    let dir = generation_record_dir();
    match append_generation_record_at(&dir, record, system_prompts) {
        Ok(path) => debug!(
            generation_id = %record.generation_id,
            backend = record.backend,
            status = record.status,
            path = %path.display(),
            "persisted dialogue generation record"
        ),
        Err(error) => warn!(
            generation_id = %record.generation_id,
            backend = record.backend,
            error = %error,
            "failed to persist dialogue generation record; nothing was logged in its place"
        ),
    }
}
