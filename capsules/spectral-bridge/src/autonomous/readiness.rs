use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Map, Value, json};

use crate::paths::bridge_paths;

const SOURCE_STATUS_VERSION: u32 = 1;
const PENDING_NEXT_OVERRIDE_VERSION: u32 = 1;
const PENDING_NEXT_MAX_AGE: Duration = Duration::from_secs(12 * 60 * 60);
const ALLOWED_PENDING_NEXT_BASES: &[&str] = &[
    "ACTION_PREFLIGHT",
    "ACTION_STATUS",
    "BROWSE",
    "CAPABILITY_DIFF",
    "CAPABILITY_MAP",
    "CAPABILITY_STATUS",
    "CLOSE_EARS",
    "CLOSE_EYES",
    "EXPERIMENT_OBSERVE",
    "EXPERIMENT_ALT_PATHS",
    "EXPERIMENT_BRANCH",
    "EXPERIMENT_COMPARE",
    "EXPERIMENT_PEER_REVIEW",
    "EXPERIMENT_PLAN",
    "EXPERIMENT_RESUME",
    "EXPERIMENT_REVIEW",
    "EXPERIMENT_START",
    "EXPERIMENT_STATUS",
    "SHARED_INVESTIGATION_CLAIM",
    "SHARED_INVESTIGATION_DECIDE",
    "SHARED_INVESTIGATION_START",
    "SHARED_INVESTIGATION_STATUS",
    "FACULTIES",
    "INTROSPECT",
    "JOURNAL",
    "JOB_STATUS",
    "NEXT_PROBE",
    "NOTICE",
    "OPEN_EARS",
    "OPEN_EYES",
    "PASS",
    "PREFLIGHT",
    "PROBE_ACTION",
    "ACTION_CANCEL",
    "READ_MORE",
    "RECALL",
    "REPAIR_RECORD",
    "REPAIR_STATUS",
    "REPAIR_SWEEP",
    "REST",
    "SEARCH",
    "SELF_STUDY",
    "SHUT_EARS",
    "SHUT_EYES",
    "FOLD_HOLD",
    "LATENT_STASIS",
    "BRACE_AUDIT",
    "SPACE_HOLD",
    "SPECTRAL_EXPLORER",
    "THREADS",
    "THREAD_STATUS",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingNextOverride {
    pub action: String,
    token: String,
}

#[derive(Debug, Clone)]
struct SourceStatusInput {
    source_root: PathBuf,
    executable_path: PathBuf,
    started_at: SystemTime,
    checked_at: SystemTime,
    reason: String,
}

fn runtime_dir() -> PathBuf {
    bridge_paths().bridge_workspace().join("runtime")
}

fn source_status_path() -> PathBuf {
    runtime_dir().join("astrid_autonomous_source_status.json")
}

fn pending_next_override_path() -> PathBuf {
    runtime_dir().join("pending_next_override.json")
}

fn unix_secs(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or_default()
}

fn iso_now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn newest_source_file(root: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut stack = vec![root.to_path_buf()];
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    while let Some(path) = stack.pop() {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if let Ok(modified) = path.metadata().and_then(|metadata| metadata.modified()) {
                let update = newest
                    .as_ref()
                    .is_none_or(|(_, current)| modified > *current);
                if update {
                    newest = Some((path, modified));
                }
            }
            continue;
        }
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || matches!(
                    name.as_str(),
                    "target" | "workspace" | "node_modules" | "__pycache__" | ".venv" | "venv"
                )
            {
                continue;
            }
            stack.push(child);
        }
    }
    newest
}

fn source_status_for_input(input: &SourceStatusInput) -> Value {
    let source = newest_source_file(&input.source_root);
    let (latest_source_path, source_mtime) = source
        .as_ref()
        .map(|(path, modified)| (Some(path.display().to_string()), Some(*modified)))
        .unwrap_or((None, None));
    let executable_mtime = input
        .executable_path
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok();

    let source_changed_since_start = source_mtime.is_some_and(|mtime| mtime > input.started_at);
    let source_newer_than_executable = match (source_mtime, executable_mtime) {
        (Some(source), Some(executable)) => source > executable,
        (Some(_), None) => true,
        _ => false,
    };
    let reload_required = source_changed_since_start || source_newer_than_executable;
    let reload_note = reload_required.then(|| {
        "The live Astrid bridge appears older than source on disk; rebuild/restart the bridge before validating newly added actions."
            .to_string()
    });

    json!({
        "schema_version": SOURCE_STATUS_VERSION,
        "system": "astrid",
        "component": "consciousness_bridge_autonomous",
        "pid": std::process::id(),
        "started_at_unix_s": unix_secs(input.started_at),
        "checked_at_unix_s": unix_secs(input.checked_at),
        "checked_at": iso_now(),
        "reason": input.reason,
        "source_root": input.source_root.display().to_string(),
        "source_path": latest_source_path,
        "source_mtime_current": source_mtime.map(unix_secs),
        "executable_path": input.executable_path.display().to_string(),
        "executable_mtime_current": executable_mtime.map(unix_secs),
        "source_changed_since_start": source_changed_since_start,
        "source_newer_than_executable": source_newer_than_executable,
        "reload_required": reload_required,
        "llm_jobs": crate::llm_jobs::runtime_status(),
        "reload_note": reload_note,
    })
}

pub(super) fn write_source_status(started_at: SystemTime, reason: &str) -> Value {
    let input = SourceStatusInput {
        source_root: bridge_paths().bridge_root().join("src"),
        executable_path: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("unknown")),
        started_at,
        checked_at: SystemTime::now(),
        reason: reason.to_string(),
    };
    let status = source_status_for_input(&input);
    let path = source_status_path();
    if let Err(error) = fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
        .and_then(|()| {
            fs::write(
                &path,
                serde_json::to_string_pretty(&status).unwrap_or_default(),
            )
        })
    {
        tracing::debug!(error = %error, "failed to write Astrid source status");
    }
    status
}

pub(crate) fn read_source_status() -> Option<Value> {
    let path = source_status_path();
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn pending_next_base(action: &str) -> String {
    let cleaned = action.trim();
    let cleaned = cleaned
        .strip_prefix("NEXT:")
        .map(str::trim)
        .unwrap_or(cleaned);
    cleaned
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .to_ascii_uppercase()
}

fn operator_pending_next_action(payload: &Map<String, Value>) -> String {
    payload
        .get("pending_next_action")
        .or_else(|| payload.get("action"))
        .or_else(|| payload.get("next"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_timestamp_secs(value: &Value) -> Option<f64> {
    if let Some(seconds) = value.as_f64() {
        return Some(seconds);
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(seconds) = text.parse::<f64>() {
        return Some(seconds);
    }
    if let Ok(datetime) = DateTime::parse_from_rfc3339(text) {
        return Some(datetime.timestamp_millis() as f64 / 1000.0);
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f") {
        let datetime = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
        return Some(datetime.timestamp_millis() as f64 / 1000.0);
    }
    None
}

fn timestamp_age(value: Option<&Value>, now: SystemTime) -> Option<Duration> {
    let timestamp = value.and_then(parse_timestamp_secs)?;
    let now_secs = unix_secs(now);
    if timestamp > now_secs {
        return Some(PENDING_NEXT_MAX_AGE + Duration::from_secs(1));
    }
    Some(Duration::from_secs_f64(now_secs - timestamp))
}

fn pending_next_status_payload(
    mut payload: Map<String, Value>,
    status: &str,
    reason: &str,
    action: Option<&str>,
) -> Map<String, Value> {
    let original_action = action
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| operator_pending_next_action(&payload));
    payload.insert(
        "schema_version".to_string(),
        json!(
            payload
                .get("schema_version")
                .and_then(Value::as_u64)
                .unwrap_or(u64::from(PENDING_NEXT_OVERRIDE_VERSION))
        ),
    );
    payload.insert("status".to_string(), json!(status));
    payload.insert("status_reason".to_string(), json!(reason));
    payload.insert(format!("{status}_at"), json!(iso_now()));

    if matches!(status, "blocked" | "consumed" | "expired") {
        payload.insert("active".to_string(), json!(false));
        payload.insert("terminal".to_string(), json!(true));
        payload.insert("pending_next_action".to_string(), Value::Null);
        payload.remove("action");
        payload.remove("next");
        if !original_action.is_empty() {
            payload.insert(
                "last_pending_next_action".to_string(),
                json!(original_action),
            );
        }
    } else {
        payload.insert("active".to_string(), json!(true));
        payload.insert("terminal".to_string(), json!(false));
        if !original_action.is_empty() {
            payload.insert("pending_next_action".to_string(), json!(original_action));
        }
    }
    payload
}

fn write_pending_next_status(
    payload: Map<String, Value>,
    status: &str,
    reason: &str,
    action: Option<&str>,
) {
    let payload = pending_next_status_payload(payload, status, reason, action);

    let path = pending_next_override_path();
    if let Err(error) = fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
        .and_then(|()| {
            fs::write(
                &path,
                serde_json::to_string_pretty(&Value::Object(payload)).unwrap_or_default(),
            )
        })
    {
        tracing::warn!(error = %error, "failed to write pending NEXT override status");
    }
}

pub(super) fn read_pending_next_override() -> Option<PendingNextOverride> {
    let path = pending_next_override_path();
    let content = fs::read_to_string(path).ok()?;
    let Value::Object(payload) = serde_json::from_str::<Value>(&content).ok()? else {
        return None;
    };

    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending")
        .to_ascii_lowercase();
    if status != "pending" {
        return None;
    }

    let action = operator_pending_next_action(&payload);
    if action.is_empty() {
        write_pending_next_status(payload, "blocked", "missing pending_next_action", None);
        return None;
    }

    let age = timestamp_age(
        payload
            .get("updated_at")
            .or_else(|| payload.get("created_at"))
            .or_else(|| payload.get("pending_next_action_updated_at")),
        SystemTime::now(),
    );
    if age.is_some_and(|age| age > PENDING_NEXT_MAX_AGE) {
        write_pending_next_status(
            payload,
            "expired",
            "pending NEXT override is stale",
            Some(&action),
        );
        return None;
    }

    let base = pending_next_base(&action);
    if !ALLOWED_PENDING_NEXT_BASES.contains(&base.as_str()) {
        write_pending_next_status(
            payload,
            "blocked",
            &format!(
                "operator override is limited to read-only/protected NEXT bases; `{}` is not allowed",
                if base.is_empty() {
                    "(none)"
                } else {
                    base.as_str()
                }
            ),
            Some(&action),
        );
        return None;
    }

    let token = payload
        .get("override_id")
        .or_else(|| payload.get("updated_at"))
        .or_else(|| payload.get("created_at"))
        .and_then(Value::as_str)
        .unwrap_or(action.as_str())
        .to_string();

    Some(PendingNextOverride { action, token })
}

pub(super) fn mark_pending_next_override_consumed(pending: &PendingNextOverride, reason: &str) {
    mark_pending_next_override_consumed_with_result(pending, reason, None);
}

fn mark_pending_next_override_consumed_with_result(
    pending: &PendingNextOverride,
    reason: &str,
    result: Option<&str>,
) {
    let path = pending_next_override_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    if payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending")
        != "pending"
    {
        return;
    }
    if operator_pending_next_action(&payload) != pending.action {
        return;
    }
    let token = payload
        .get("override_id")
        .or_else(|| payload.get("updated_at"))
        .or_else(|| payload.get("created_at"))
        .and_then(Value::as_str)
        .unwrap_or(pending.action.as_str());
    if token != pending.token {
        return;
    }
    let mut payload =
        pending_next_status_payload(payload, "consumed", reason, Some(&pending.action));
    if let Some(result) = result {
        payload.insert("result".to_string(), json!(result));
    }
    if let Err(error) = fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
        .and_then(|()| {
            fs::write(
                &path,
                serde_json::to_string_pretty(&Value::Object(payload)).unwrap_or_default(),
            )
        })
    {
        tracing::warn!(error = %error, "failed to write consumed pending NEXT override result");
    }
}

fn strip_action_arg(action: &str, base: &str) -> String {
    let cleaned = action.trim();
    let cleaned = cleaned
        .strip_prefix("NEXT:")
        .map(str::trim)
        .unwrap_or(cleaned);
    cleaned
        .strip_prefix(base)
        .or_else(|| cleaned.strip_prefix(&base.to_ascii_lowercase()))
        .unwrap_or(cleaned)
        .trim_start_matches([' ', ':', '-'])
        .trim()
        .to_string()
}

pub(super) fn try_handle_llm_job_pending_override() -> bool {
    let Some(pending) = read_pending_next_override() else {
        return false;
    };
    let base = pending_next_base(&pending.action);
    if !matches!(
        base.as_str(),
        "ACTION_STATUS" | "JOB_STATUS" | "ACTION_CANCEL"
    ) {
        return false;
    }
    let selector = strip_action_arg(&pending.action, &base);
    let selector = (!selector.is_empty()).then_some(selector.as_str());
    let result = if base == "ACTION_CANCEL" {
        crate::llm_jobs::cancel(selector)
    } else {
        crate::llm_jobs::status_text(selector)
    };
    let result = match result {
        Ok(text) => text,
        Err(error) => format!(
            "{base} could not resolve `{}`: {error}",
            selector.unwrap_or("latest")
        ),
    };
    mark_pending_next_override_consumed_with_result(
        &pending,
        "llm job status override handled by background status loop",
        Some(&result),
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("astrid_readiness_{name}_{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn source_status_flags_newer_source() {
        let dir = temp_dir("source_status");
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        let source = src.join("autonomous.rs");
        fs::write(&source, "fn main() {}\n").unwrap();
        let exe = dir.join("bridge-bin");
        fs::write(&exe, "bin\n").unwrap();
        let started_at = SystemTime::now() - Duration::from_secs(5);
        let status = source_status_for_input(&SourceStatusInput {
            source_root: src,
            executable_path: exe,
            started_at,
            checked_at: SystemTime::now(),
            reason: "test".to_string(),
        });
        assert_eq!(status["system"], "astrid");
        assert_eq!(status["component"], "consciousness_bridge_autonomous");
        assert_eq!(status["source_changed_since_start"], true);
        assert_eq!(status["reload_required"], true);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pending_next_base_allows_read_only_and_blocks_control() {
        assert!(
            ALLOWED_PENDING_NEXT_BASES
                .contains(&pending_next_base("INTROSPECT autonomous").as_str())
        );
        assert!(
            ALLOWED_PENDING_NEXT_BASES
                .contains(&pending_next_base("ACTION_PREFLIGHT PERTURB lambda2").as_str())
        );
        assert!(ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("FACULTIES").as_str()));
        assert!(
            ALLOWED_PENDING_NEXT_BASES
                .contains(&pending_next_base("CAPABILITY_STATUS EXPERIMENT_START").as_str())
        );
        assert!(
            ALLOWED_PENDING_NEXT_BASES
                .contains(&pending_next_base("REPAIR_SWEEP experiments").as_str())
        );
        assert!(
            !ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("REPAIR_APPLY all").as_str())
        );
        assert!(ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("CLOSE_EYES").as_str()));
        assert!(ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("SHUT_EARS").as_str()));
        assert!(ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("OPEN_EYES").as_str()));
        assert!(ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("OPEN_EARS").as_str()));
        assert!(
            ALLOWED_PENDING_NEXT_BASES.contains(
                &pending_next_base(
                    "EXPERIMENT_START Sensory grounding :: Does presence change returnability?"
                )
                .as_str()
            )
        );
        assert!(
            ALLOWED_PENDING_NEXT_BASES
                .contains(&pending_next_base("EXPERIMENT_PLAN current").as_str())
        );
        assert!(
            ALLOWED_PENDING_NEXT_BASES
                .contains(&pending_next_base("EXPERIMENT_OBSERVE current :: quiet note").as_str())
        );
        assert!(
            !ALLOWED_PENDING_NEXT_BASES.contains(
                &pending_next_base("EXPERIMENT_BIND current :: PERTURB lambda2").as_str()
            )
        );
        assert!(
            !ALLOWED_PENDING_NEXT_BASES.contains(&pending_next_base("PERTURB lambda2").as_str())
        );
    }

    #[test]
    fn terminal_status_preserves_last_action() {
        let mut payload = Map::new();
        payload.insert("status".to_string(), json!("pending"));
        payload.insert(
            "pending_next_action".to_string(),
            json!("INTROSPECT autonomous"),
        );
        let original = operator_pending_next_action(&payload);
        assert_eq!(original, "INTROSPECT autonomous");
        let status = Value::Object(pending_next_status_payload(
            payload,
            "consumed",
            "test",
            Some(&original),
        ));
        assert_eq!(status["active"], false);
        assert_eq!(status["terminal"], true);
        assert!(status["pending_next_action"].is_null());
        assert_eq!(status["last_pending_next_action"], "INTROSPECT autonomous");
    }
}
