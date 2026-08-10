use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};

use serde_json::Value;

use crate::config::GateConfig;
use crate::util::read_stable_regular;
use crate::{Error, Result};

const MAX_STATE_BYTES: u64 = 256 * 1024;
const MAX_LEDGER_TAIL_BYTES: u64 = 128 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
pub struct GateStatus {
    pub ready: bool,
    pub reason: String,
    pub thermal_celsius: u16,
}

pub fn inspect(config: &GateConfig) -> Result<GateStatus> {
    let autonomy = read_json(&config.autonomy_state)?;
    let autonomy = exact_autonomy_gate(&autonomy)?;
    if autonomy.turn_active {
        return Ok(deferred("ordinary autonomy is active", 0));
    }
    if autonomy.recovery_active {
        return Ok(deferred("transport recovery is active", 0));
    }
    if autonomy.pending_runtime_work {
        return Ok(deferred("an Action or durable receipt is active", 0));
    }
    // This tail scan is corroborating evidence only. A request may legitimately
    // predate the bounded tail, so authoritative Action state above must never
    // be inferred from this ledger scan. Exact active conversation, session,
    // tool, and provider counts are drained and attested by the immutable
    // reflection-preparation ACK before this helper is allowed to call a model.
    if has_unmatched_action(&config.action_receipts)? {
        return Ok(deferred("an Action or tool request is active", 0));
    }
    let thermal = thermal_celsius(config)?;
    if thermal >= config.maximum_thermal_celsius {
        return Ok(deferred(
            "thermal pressure exceeds the immutable limit",
            thermal,
        ));
    }
    Ok(GateStatus {
        ready: true,
        reason: "ready".to_owned(),
        thermal_celsius: thermal,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AutonomyGate {
    turn_active: bool,
    recovery_active: bool,
    pending_runtime_work: bool,
}

fn exact_autonomy_gate(value: &Value) -> Result<AutonomyGate> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("autonomy gate state is not an object"))?;
    if object.get("schema").and_then(Value::as_str) != Some("astrid_edge_autonomy_state_v3") {
        return Err(Error::new(
            "autonomy gate state is not the exact supported v3 schema",
        ));
    }

    let last_status = optional_string(object.get("last_status"), "last_status")?;
    let consecutive_failures =
        required_u64(object.get("consecutive_failures"), "consecutive_failures")?;
    let run_receipt_pending =
        required_bool(object.get("run_receipt_pending"), "run_receipt_pending")?;
    let chain_receipt_pending =
        required_bool(object.get("chain_receipt_pending"), "chain_receipt_pending")?;
    let action_dispatch_pending = required_bool(
        object.get("action_dispatch_pending"),
        "action_dispatch_pending",
    )?;
    let thread_projection_pending = object
        .get("thread_projection_pending")
        .ok_or_else(|| Error::new("autonomy gate field thread_projection_pending is absent"))?
        .is_object();
    if object
        .get("thread_projection_pending")
        .is_some_and(|value| !value.is_null() && !value.is_object())
    {
        return Err(Error::new(
            "autonomy gate field thread_projection_pending has the wrong type",
        ));
    }

    let response = optional_string(
        object.get("pending_action_response_sha256"),
        "pending_action_response_sha256",
    )?;
    if response.is_some_and(|digest| !is_hex64(digest)) {
        return Err(Error::new(
            "pending Action response hash is not an exact SHA-256 digest",
        ));
    }
    let trace_pending =
        optional_object(object.get("pending_action_trace"), "pending_action_trace")?;
    let session = optional_string(
        object.get("pending_action_session_id"),
        "pending_action_session_id",
    )?;
    if session.is_some_and(|value| value.is_empty() || value.len() > 192) {
        return Err(Error::new("pending Action session identifier is invalid"));
    }
    let transcript = optional_string(
        object.get("pending_action_transcript_path"),
        "pending_action_transcript_path",
    )?;
    let provenance = optional_string(
        object.get("pending_action_response_provenance"),
        "pending_action_response_provenance",
    )?;
    if provenance.is_some_and(|value| {
        !matches!(
            value,
            "model_authored"
                | "model_authored_with_local_safe_fallback"
                | "model_authored_with_local_format_repair"
        )
    }) {
        return Err(Error::new(
            "pending Action provenance is not a recognized typed value",
        ));
    }
    let any_action_binding = response.is_some()
        || trace_pending
        || session.is_some()
        || transcript.is_some()
        || provenance.is_some();
    if action_dispatch_pending != any_action_binding {
        return Err(Error::new(
            "pending Action boolean and exact binding fields disagree",
        ));
    }
    if action_dispatch_pending
        && (response.is_none()
            || !trace_pending
            || session.is_none()
            || transcript.is_none()
            || provenance.is_none())
    {
        return Err(Error::new("pending Action binding is incomplete"));
    }

    Ok(AutonomyGate {
        turn_active: matches!(last_status, Some("running" | "starting")),
        recovery_active: consecutive_failures > 0
            || matches!(
                last_status,
                Some("transport_recovery" | "recovering" | "backoff")
            ),
        pending_runtime_work: run_receipt_pending
            || chain_receipt_pending
            || action_dispatch_pending
            || thread_projection_pending,
    })
}

fn required_bool(value: Option<&Value>, field: &str) -> Result<bool> {
    value.and_then(Value::as_bool).ok_or_else(|| {
        Error::new(format!(
            "autonomy gate field {field} is absent or not boolean"
        ))
    })
}

fn required_u64(value: Option<&Value>, field: &str) -> Result<u64> {
    value
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("autonomy gate field {field} is absent or not u64")))
}

fn optional_string<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a str>> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(Error::new(format!(
            "autonomy gate field {field} is absent or has the wrong type"
        ))),
    }
}

fn optional_object(value: Option<&Value>, field: &str) -> Result<bool> {
    match value {
        Some(Value::Null) => Ok(false),
        Some(Value::Object(_)) => Ok(true),
        _ => Err(Error::new(format!(
            "autonomy gate field {field} is absent or has the wrong type"
        ))),
    }
}

fn is_hex64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn deferred(reason: &str, thermal_celsius: u16) -> GateStatus {
    GateStatus {
        ready: false,
        reason: reason.to_owned(),
        thermal_celsius,
    }
}

fn read_json(path: &std::path::Path) -> Result<Value> {
    let bytes = read_stable_regular(path, MAX_STATE_BYTES)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn walk(value: &Value) -> Box<dyn Iterator<Item = (&str, &Value)> + '_> {
    match value {
        Value::Object(values) => Box::new(
            values
                .iter()
                .flat_map(|(key, value)| std::iter::once((key.as_str(), value)).chain(walk(value))),
        ),
        Value::Array(values) => Box::new(values.iter().flat_map(walk)),
        _ => Box::new(std::iter::empty()),
    }
}

fn has_unmatched_action(path: &std::path::Path) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 16 * 1024 * 1024 * 1024_u64
    {
        return Err(Error::new(
            "Action receipt ledger is not a bounded regular file",
        ));
    }
    let mut file = File::open(path)?;
    let take = metadata.len().min(MAX_LEDGER_TAIL_BYTES);
    let offset = i64::try_from(take).map_err(|_| Error::new("ledger tail offset overflow"))?;
    let offset = offset
        .checked_neg()
        .ok_or_else(|| Error::new("ledger tail offset cannot be negated"))?;
    file.seek(SeekFrom::End(offset))?;
    let mut data = Vec::new();
    file.take(MAX_LEDGER_TAIL_BYTES).read_to_end(&mut data)?;
    if take < metadata.len() {
        if let Some(newline) = data.iter().position(|byte| *byte == b'\n') {
            data.drain(..=newline);
        } else {
            return Err(Error::new(
                "Action receipt tail contains no record boundary",
            ));
        }
    }
    let mut pending = BTreeSet::new();
    for line in data
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_slice(line)?;
        let identifier = first_string(&value, &["call_id", "action_id", "request_id", "trace_id"]);
        let status = first_string(&value, &["phase", "status"]);
        if let (Some(identifier), Some(status)) = (identifier, status) {
            if matches!(status, "requested" | "running" | "started") {
                pending.insert(identifier.to_owned());
            } else if matches!(status, "completed" | "failed" | "cancelled" | "rejected") {
                pending.remove(identifier);
            }
        }
    }
    Ok(!pending.is_empty())
}

fn first_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    walk(value).find_map(|(key, value)| keys.contains(&key).then(|| value.as_str()).flatten())
}

fn thermal_celsius(config: &GateConfig) -> Result<u16> {
    let bytes = read_stable_regular(&config.thermal_celsius, 128)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("thermal reading is not UTF-8"))?
        .trim();
    let raw = text
        .parse::<i64>()
        .map_err(|_| Error::new("thermal reading is not an integer"))?;
    let celsius = if raw > 1_000 { raw / 1_000 } else { raw };
    u16::try_from(celsius).map_err(|_| Error::new("thermal reading is outside range"))
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::fs;

    use serde_json::{Value, json};
    use tempfile::tempdir;

    use super::{exact_autonomy_gate, has_unmatched_action, inspect};
    use crate::config::GateConfig;

    fn autonomy_state() -> Value {
        json!({
            "schema": "astrid_edge_autonomy_state_v3",
            "last_status": "authored_completed",
            "consecutive_failures": 0,
            "run_receipt_pending": false,
            "chain_receipt_pending": false,
            "action_dispatch_pending": false,
            "pending_action_response_sha256": null,
            "pending_action_trace": null,
            "pending_action_session_id": null,
            "pending_action_transcript_path": null,
            "pending_action_response_provenance": null,
            "thread_projection_pending": null
        })
    }

    #[test]
    fn exact_autonomy_schema_rejects_prose_and_inconsistent_action_binding() {
        assert!(exact_autonomy_gate(&json!({"note": "running recovery"})).is_err());
        let mut state = autonomy_state();
        state["pending_action_response_sha256"] = Value::String("a".repeat(64));
        assert!(exact_autonomy_gate(&state).is_err());
    }

    #[test]
    fn typed_pending_action_cannot_fall_out_of_bounded_ledger_tail() {
        let temporary = tempdir().unwrap();
        let autonomy = temporary.path().join("state.json");
        let actions = temporary.path().join("receipts.jsonl");
        let thermal = temporary.path().join("thermal");
        let action_id = "old-request";
        let mut ledger = format!(
            "{}\n",
            json!({"action_id": action_id, "phase": "requested"})
        );
        for index in 0..2_000 {
            writeln!(
                ledger,
                "{}",
                json!({
                    "action_id": format!("terminal-{index:04}"),
                    "phase": "completed",
                    "padding": "x".repeat(96)
                })
            )
            .unwrap();
        }
        fs::write(&actions, ledger).unwrap();
        assert!(!has_unmatched_action(&actions).unwrap());

        let mut state = autonomy_state();
        state["action_dispatch_pending"] = Value::Bool(true);
        state["pending_action_response_sha256"] = Value::String("a".repeat(64));
        state["pending_action_trace"] = json!({"trace_id":"trace"});
        state["pending_action_session_id"] = Value::String("session".to_owned());
        state["pending_action_transcript_path"] = Value::String("turn.md".to_owned());
        state["pending_action_response_provenance"] = Value::String("model_authored".to_owned());
        fs::write(&autonomy, serde_json::to_vec(&state).unwrap()).unwrap();
        fs::write(&thermal, b"42\n").unwrap();
        let status = inspect(&GateConfig {
            autonomy_state: autonomy,
            action_receipts: actions,
            thermal_celsius: thermal,
            maximum_thermal_celsius: 80,
        })
        .unwrap();
        assert!(!status.ready);
        assert_eq!(status.reason, "an Action or durable receipt is active");
    }
}
