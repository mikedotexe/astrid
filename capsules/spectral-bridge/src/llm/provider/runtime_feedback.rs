use crate::runtime_action_feedback::{
    MAX_RUNTIME_FEEDBACK_PER_REQUEST, RuntimeActionFeedbackV1, RuntimeFeedbackReceiptV1,
    render_runtime_action_feedback,
};

const MAX_RUNTIME_FEEDBACK_ARTIFACT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeFeedbackAdmissionV1 {
    feedback: Vec<RuntimeActionFeedbackV1>,
    message_index: usize,
}

/// Exact provider bytes and the specific system block inserted by the runtime.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SubmittedRuntimeFeedbackAttemptV1 {
    provider_route: String,
    provider_model: String,
    request_json: String,
    response_json: String,
    admission: RuntimeFeedbackAdmissionV1,
}

fn runtime_feedback_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// Called after profile adaptation, before protected foreground admission. It
/// never edits existing system messages, including authored emphasis and form.
fn admit_runtime_feedback(
    messages: &mut Vec<Message>,
    feedback: &[RuntimeActionFeedbackV1],
    limit: usize,
) -> Option<RuntimeFeedbackAdmissionV1> {
    let system_bytes = messages
        .iter()
        .filter(|message| message.role == "system")
        .map(|message| message.content.len())
        .sum::<usize>();
    let (count, content) = (1..=feedback.len().min(MAX_RUNTIME_FEEDBACK_PER_REQUEST))
        .rev()
        .find_map(|count| {
            let content = render_runtime_action_feedback(&feedback[..count])?;
            (system_bytes.checked_add(content.len())? <= limit).then_some((count, content))
        })?;
    let mut excess = message_prompt_chars(messages)
        .saturating_add(content.len())
        .saturating_sub(limit);
    while excess > 0 {
        let longest = messages
            .iter_mut()
            .filter(|message| message.role != "system" && !message.content.is_empty())
            .max_by_key(|message| message.content.len())?;
        let old_len = longest.content.len();
        let keep = old_len.saturating_sub(excess);
        let keep = longest.content.floor_char_boundary(keep);
        longest.content.truncate(keep);
        excess = excess.saturating_sub(old_len.saturating_sub(keep));
    }
    // A leading system block keeps this exact index through protected admission,
    // which can discard ambient non-system messages but preserves system policy.
    messages.insert(
        0,
        Message {
            role: "system".into(),
            content,
        },
    );
    Some(RuntimeFeedbackAdmissionV1 {
        feedback: feedback[..count].to_vec(),
        message_index: 0,
    })
}

/// Keep foreground reading/letters intact if reserving feedback would prevent
/// their admission. Omitted feedback has no receipt and remains pending.
fn admit_runtime_feedback_and_protected_content(
    messages: &mut Vec<Message>,
    protected: Option<&ProtectedDialogueInputV1>,
    feedback: &[RuntimeActionFeedbackV1],
    limit: usize,
) -> Option<(
    Option<ProtectedAdmissionV1>,
    Option<RuntimeFeedbackAdmissionV1>,
)> {
    let original = messages.clone();
    let mut baseline_messages = original.clone();
    let baseline = match protected {
        Some(input) => Some(admit_protected_dialogue_content(
            &mut baseline_messages,
            input,
            limit,
        )?),
        None => None,
    };
    let mut offered = feedback.len().min(MAX_RUNTIME_FEEDBACK_PER_REQUEST);
    while offered > 0 {
        *messages = original.clone();
        let Some(runtime) = admit_runtime_feedback(messages, &feedback[..offered], limit) else {
            break;
        };
        let Some(input) = protected else {
            return Some((None, Some(runtime)));
        };
        if let Some(admission) = admit_protected_dialogue_content(messages, input, limit)
            && baseline.as_ref().is_some_and(|baseline| {
                admission.content_id == baseline.content_id
                    && admission.kind == baseline.kind
                    && admission.source_start_byte == baseline.source_start_byte
                    && admission.admitted_end_byte >= baseline.admitted_end_byte
            })
        {
            return Some((Some(admission), Some(runtime)));
        }
        // Try a smaller prefix before omitting all feedback. Only the actually
        // admitted IDs become eligible for a receipt and queue consumption.
        offered = runtime.feedback.len().saturating_sub(1);
    }
    *messages = baseline_messages;
    Some((baseline, None))
}

fn validate_runtime_feedback_attempt(
    attempt: &SubmittedRuntimeFeedbackAttemptV1,
) -> std::io::Result<()> {
    if attempt.provider_route.trim().is_empty() || attempt.provider_model.trim().is_empty() {
        return Err(runtime_feedback_error("missing provider identity"));
    }
    let request: serde_json::Value = serde_json::from_str(&attempt.request_json)?;
    let message = request
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .and_then(|messages| messages.get(attempt.admission.message_index))
        .ok_or_else(|| runtime_feedback_error("runtime feedback message missing"))?;
    let expected = render_runtime_action_feedback(&attempt.admission.feedback)
        .ok_or_else(|| runtime_feedback_error("invalid runtime feedback identity"))?;
    if message.get("role").and_then(serde_json::Value::as_str) != Some("system")
        || message.get("content").and_then(serde_json::Value::as_str) != Some(expected.as_str())
    {
        return Err(runtime_feedback_error(
            "inserted runtime feedback block did not survive",
        ));
    }
    Ok(())
}

fn capture_runtime_feedback_attempt(
    route: &str,
    model: &str,
    request_bytes: &[u8],
    response_body: &str,
    admission: Option<RuntimeFeedbackAdmissionV1>,
) -> Option<SubmittedRuntimeFeedbackAttemptV1> {
    if request_bytes.len().saturating_add(response_body.len())
        > MAX_RUNTIME_FEEDBACK_ARTIFACT_BYTES as usize
    {
        return None;
    }
    let attempt = SubmittedRuntimeFeedbackAttemptV1 {
        provider_route: route.to_owned(),
        provider_model: model.to_owned(),
        request_json: std::str::from_utf8(request_bytes).ok()?.to_owned(),
        response_json: response_body.to_owned(),
        admission: admission?,
    };
    validate_runtime_feedback_attempt(&attempt).ok()?;
    Some(attempt)
}

#[derive(Serialize, Deserialize)]
struct RetainedRuntimeFeedbackV1 {
    schema: String,
    attempt: SubmittedRuntimeFeedbackAttemptV1,
    accepted_completion: String,
}

fn runtime_feedback_receipt(
    artifact: &RetainedRuntimeFeedbackV1,
    path: &std::path::Path,
    encoded: &[u8],
) -> RuntimeFeedbackReceiptV1 {
    RuntimeFeedbackReceiptV1 {
        feedback_ids: artifact
            .attempt
            .admission
            .feedback
            .iter()
            .map(|item| item.id.clone())
            .collect(),
        provider_route: artifact.attempt.provider_route.clone(),
        provider_model: artifact.attempt.provider_model.clone(),
        request_sha256: protected_digest(&artifact.attempt.request_json),
        retained_completion_sha256: protected_digest(&artifact.accepted_completion),
        retained_artifact_path: path.display().to_string(),
        retained_artifact_sha256: protected_digest(encoded),
    }
}

fn reject_runtime_feedback_symlinks(path: &std::path::Path) -> std::io::Result<()> {
    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && metadata.file_type().is_symlink()
    {
        return Err(runtime_feedback_error("runtime feedback path is a symlink"));
    }
    Ok(())
}

fn verify_private_runtime_feedback_file(path: &std::path::Path) -> std::io::Result<()> {
    reject_runtime_feedback_symlinks(path)?;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_RUNTIME_FEEDBACK_ARTIFACT_BYTES {
        return Err(runtime_feedback_error(
            "invalid runtime feedback artifact file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(runtime_feedback_error(
                "runtime feedback artifact is not private",
            ));
        }
    }
    Ok(())
}

/// Called only for the quality gate's accepted attempt. Both the exact runtime
/// block and accepted provider-derived response are verified before persistence.
fn retain_runtime_feedback_at(
    root: &std::path::Path,
    attempt: SubmittedRuntimeFeedbackAttemptV1,
    accepted_text: &str,
) -> std::io::Result<RuntimeFeedbackReceiptV1> {
    use std::io::Write as _;
    validate_runtime_feedback_attempt(&attempt)?;
    validate_retained_completion_json(&attempt.response_json, accepted_text)?;
    let artifact = RetainedRuntimeFeedbackV1 {
        schema: "accepted_runtime_feedback_v1".into(),
        attempt,
        accepted_completion: accepted_text.into(),
    };
    let encoded = serde_json::to_vec(&artifact)?;
    if encoded.len() > MAX_RUNTIME_FEEDBACK_ARTIFACT_BYTES as usize {
        return Err(runtime_feedback_error(
            "runtime feedback artifact exceeds byte limit",
        ));
    }
    reject_runtime_feedback_symlinks(root)?;
    std::fs::create_dir_all(root)?;
    reject_runtime_feedback_symlinks(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = root.join(format!("{}.json", protected_digest(&encoded)));
    reject_runtime_feedback_symlinks(&path)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            if let Err(error) = file.write_all(&encoded).and_then(|()| file.sync_all()) {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if std::fs::read(&path)? != encoded {
                return Err(runtime_feedback_error("runtime feedback artifact changed"));
            }
        },
        Err(error) => return Err(error),
    }
    std::fs::File::open(root)?.sync_all()?;
    verify_private_runtime_feedback_file(&path)?;
    Ok(runtime_feedback_receipt(&artifact, &path, &encoded))
}

/// Validate the durable accepted-attempt proof before consuming any queue IDs.
pub(crate) fn verify_runtime_feedback_receipt(
    receipt: &RuntimeFeedbackReceiptV1,
) -> std::io::Result<()> {
    let path = std::path::Path::new(&receipt.retained_artifact_path);
    verify_private_runtime_feedback_file(path)?;
    let bytes = std::fs::read(path)?;
    if protected_digest(&bytes) != receipt.retained_artifact_sha256 {
        return Err(runtime_feedback_error(
            "runtime feedback artifact digest changed",
        ));
    }
    let artifact: RetainedRuntimeFeedbackV1 = serde_json::from_slice(&bytes)?;
    if artifact.schema != "accepted_runtime_feedback_v1" {
        return Err(runtime_feedback_error(
            "unsupported runtime feedback artifact schema",
        ));
    }
    validate_runtime_feedback_attempt(&artifact.attempt)?;
    validate_retained_completion_json(
        &artifact.attempt.response_json,
        &artifact.accepted_completion,
    )?;
    if &runtime_feedback_receipt(&artifact, path, &bytes) != receipt {
        return Err(runtime_feedback_error(
            "runtime feedback receipt does not match artifact",
        ));
    }
    Ok(())
}
