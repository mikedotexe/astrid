#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LlmDiagnosticPersistenceReceiptV1 {
    json_bytes: usize,
    elapsed_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LlmDiagnosticPersistenceErrorV1 {
    stage: &'static str,
    error_kind: &'static str,
    retryability: &'static str,
    automatic_retry_attempted: bool,
    json_bytes: Option<usize>,
    elapsed_us: u64,
}

fn llm_diagnostic_persistence_elapsed_us(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn llm_diagnostic_persistence_error(
    started: std::time::Instant,
    stage: &'static str,
    error_kind: &'static str,
    retryability: &'static str,
    json_bytes: Option<usize>,
) -> LlmDiagnosticPersistenceErrorV1 {
    LlmDiagnosticPersistenceErrorV1 {
        stage,
        error_kind,
        retryability,
        automatic_retry_attempted: false,
        json_bytes,
        elapsed_us: llm_diagnostic_persistence_elapsed_us(started),
    }
}

fn llm_diagnostic_io_error_kind(error: &std::io::Error) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::NotFound => "not_found",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::AlreadyExists => "already_exists",
        std::io::ErrorKind::InvalidInput => "invalid_input",
        std::io::ErrorKind::InvalidData => "invalid_data",
        std::io::ErrorKind::WriteZero => "write_zero",
        std::io::ErrorKind::StorageFull => "storage_full",
        std::io::ErrorKind::ReadOnlyFilesystem => "read_only_filesystem",
        _ => "other_io_error",
    }
}

fn llm_diagnostic_io_retryability(error: &std::io::Error) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::Interrupted
        | std::io::ErrorKind::WouldBlock
        | std::io::ErrorKind::TimedOut => "transient_candidate_no_automatic_retry",
        std::io::ErrorKind::NotFound
        | std::io::ErrorKind::PermissionDenied
        | std::io::ErrorKind::AlreadyExists
        | std::io::ErrorKind::InvalidInput
        | std::io::ErrorKind::InvalidData
        | std::io::ErrorKind::WriteZero
        | std::io::ErrorKind::StorageFull
        | std::io::ErrorKind::ReadOnlyFilesystem => "non_retryable_by_default",
        _ => "unknown_no_automatic_retry",
    }
}

fn append_llm_diagnostic_jsonl_at(
    dir: &std::path::Path,
    file_name: &str,
    value: &impl Serialize,
) -> Result<LlmDiagnosticPersistenceReceiptV1, LlmDiagnosticPersistenceErrorV1> {
    let started = std::time::Instant::now();
    std::fs::create_dir_all(dir).map_err(|error| {
        llm_diagnostic_persistence_error(
            started,
            "create_diagnostics_directory",
            llm_diagnostic_io_error_kind(&error),
            llm_diagnostic_io_retryability(&error),
            None,
        )
    })?;
    let path = dir.join(file_name);
    let line = serde_json::to_string(value).map_err(|_| {
        llm_diagnostic_persistence_error(
            started,
            "serialize_bounded_diagnostic",
            "serialization_error",
            "not_applicable_non_io_failure",
            None,
        )
    })?;
    let json_bytes = line.len();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            llm_diagnostic_persistence_error(
                started,
                "open_diagnostic_log",
                llm_diagnostic_io_error_kind(&error),
                llm_diagnostic_io_retryability(&error),
                Some(json_bytes),
            )
        })?;
    use std::io::Write as _;
    writeln!(file, "{line}").map_err(|error| {
        llm_diagnostic_persistence_error(
            started,
            "append_diagnostic_line",
            llm_diagnostic_io_error_kind(&error),
            llm_diagnostic_io_retryability(&error),
            Some(json_bytes),
        )
    })?;
    Ok(LlmDiagnosticPersistenceReceiptV1 {
        json_bytes,
        elapsed_us: llm_diagnostic_persistence_elapsed_us(started),
    })
}

fn append_llm_diagnostic_jsonl(file_name: &str, value: &impl Serialize) {
    let dir = bridge_paths().bridge_workspace().join("diagnostics");
    match append_llm_diagnostic_jsonl_at(&dir, file_name, value) {
        Ok(receipt) => {
            debug!(
                diagnostic_file = file_name,
                json_bytes = receipt.json_bytes,
                persistence_elapsed_us = receipt.elapsed_us,
                "persisted bounded LLM diagnostic"
            );
        },
        Err(error) => {
            warn!(
                diagnostic_file = file_name,
                failure_stage = error.stage,
                error_kind = error.error_kind,
                retryability = error.retryability,
                automatic_retry_attempted = error.automatic_retry_attempted,
                json_bytes = error.json_bytes,
                persistence_elapsed_us = error.elapsed_us,
                "failed to persist bounded LLM diagnostic; report content was not logged"
            );
        },
    }
}
