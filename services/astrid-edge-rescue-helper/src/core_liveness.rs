//! Immutable, rate-limited recovery of the exact Astrid core service.
//!
//! The mutable edge runtime may publish one typed liveness request. It never
//! receives systemd, D-Bus, key, or command authority. This root helper
//! independently verifies the request, active generation, request identity,
//! durable replay/rate history, and the exact fixed service before restarting.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::config::Config;
use crate::fs_guard::{canonical_json, sha256};
use crate::ledger_auth::{LedgerKey, seal_record, verify_record};
use crate::native::{CommandReceipt, CommandSpec, NativeRunner};
use crate::transition::read_generation_binding;
use crate::{Error, Result};

const REQUEST_SCHEMA: &str = "astrid.edge_core_liveness_request.v1";
const RECORD_SCHEMA: &str = "astrid.edge_rescue_helper.core_liveness.v1";
const AUTHORITY: &str = "immutable_root_exact_core_liveness_recovery";
const DOMAIN: &str = "core_liveness";
const CORE_SERVICE: &str = "astrid.service";
const REQUEST_NAME: &str = "core-liveness-recovery.request.json";
const LEDGER_NAME: &str = "core-liveness-receipts.jsonl";
const MAX_REQUEST_BYTES: u64 = 8 * 1024;
const MAX_LEDGER_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REQUEST_AGE_MS: u64 = 120_000;
const MAX_FUTURE_SKEW_MS: u64 = 5_000;
const MIN_RESTART_INTERVAL_MS: u64 = 15 * 60 * 1_000;
const RESTART_WINDOW_MS: u64 = 6 * 60 * 60 * 1_000;
const MAX_RESTARTS_PER_WINDOW: usize = 3;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    appliance_id: String,
    generation_id: String,
    requested_at_unix_ms: u64,
    nonce: Uuid,
    reason: String,
    trace: RequestTrace,
    authority: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RequestTrace {
    schema_version: u8,
    trace_id: Uuid,
    #[serde(default)]
    turn_id: Option<Uuid>,
    span_id: Uuid,
    #[serde(default)]
    parent_span_id: Option<Uuid>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    chain_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryResult {
    schema: &'static str,
    status: &'static str,
    request_sha256: Option<String>,
    nonce: Option<Uuid>,
    generation_id: Option<String>,
    reason: String,
    service: &'static str,
    restart_receipt: Option<CommandReceipt>,
    record_sha256: Option<String>,
    authority: &'static str,
}

#[derive(Default)]
struct LedgerState {
    head: Option<String>,
    seen_nonces: Vec<Uuid>,
    restarted_at: Vec<u64>,
}

struct RequestCleanup(PathBuf);

impl Drop for RequestCleanup {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.0) else {
            return;
        };
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let _ = fs::remove_dir(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
        if let Some(parent) = self.0.parent() {
            let _ = File::open(parent).and_then(|directory| directory.sync_all());
        }
    }
}

/// Consume a request if present. Absence is a successful no-op because the
/// same immutable helper is also run by periodic supervisor passes.
pub fn recover_if_requested(
    config: &Config,
    runner: &mut dyn NativeRunner,
) -> Result<RecoveryResult> {
    recover_inner(config, runner, unix_millis(), true)
}

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed request transaction keeps validation, fixed restart, and terminal receipt ordering visible"
)]
fn recover_inner(
    config: &Config,
    runner: &mut dyn NativeRunner,
    now: u64,
    require_root: bool,
) -> Result<RecoveryResult> {
    if config.services.core != CORE_SERVICE {
        return Err(Error::new(
            "core liveness service is not the exact allowlisted unit",
        ));
    }
    if require_root && nix::unistd::geteuid().as_raw() != 0 {
        return Err(Error::new(
            "core liveness recovery requires immutable root identity",
        ));
    }
    let request_file = request_path(config);
    if !request_file.exists() && !request_file.is_symlink() {
        return Ok(RecoveryResult {
            schema: RECORD_SCHEMA,
            status: "not_requested",
            request_sha256: None,
            nonce: None,
            generation_id: None,
            reason: "no_pending_runtime_request".to_owned(),
            service: CORE_SERVICE,
            restart_receipt: None,
            record_sha256: None,
            authority: AUTHORITY,
        });
    }
    let _cleanup = RequestCleanup(request_file.clone());
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let mut ledger = open_ledger(config, require_root)?;
    ledger.lock_exclusive()?;
    let state = replay_ledger(&mut ledger, &key)?;

    let request_bytes = match read_runtime_request(
        &request_file,
        &request_path(config),
        config.identities.runtime_uid,
        config.identities.runtime_gid,
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            return terminal_result(
                &mut ledger,
                &key,
                state.head.as_deref(),
                now,
                "rejected",
                None,
                None,
                None,
                &format!("invalid_request_identity_or_encoding:{}", error.message()),
                None,
            );
        },
    };
    let request_sha256 = sha256(&request_bytes);
    let request: Request = match serde_json::from_slice(&request_bytes) {
        Ok(value) => value,
        Err(_) => {
            return terminal_result(
                &mut ledger,
                &key,
                state.head.as_deref(),
                now,
                "rejected",
                Some(&request_sha256),
                None,
                None,
                "malformed_request_json",
                None,
            );
        },
    };
    let current_generation = read_generation_binding(config, require_root)?;
    if let Err(reason) = validate_request(
        &config.appliance_id,
        &request,
        &current_generation,
        now,
        &state,
    ) {
        return terminal_result(
            &mut ledger,
            &key,
            state.head.as_deref(),
            now,
            "rejected",
            Some(&request_sha256),
            Some(request.nonce),
            Some(&request.generation_id),
            reason.message(),
            None,
        );
    }
    if config.roots.maintenance_lease.exists() || config.roots.maintenance_lease.is_symlink() {
        return terminal_result(
            &mut ledger,
            &key,
            state.head.as_deref(),
            now,
            "rejected",
            Some(&request_sha256),
            Some(request.nonce),
            Some(&request.generation_id),
            "maintenance_or_generation_transition_active",
            None,
        );
    }

    let before_pid = core_property(config, runner, "MainPID", "core_liveness_pid_before")?
        .parse::<u32>()
        .map_err(|_| Error::new("core MainPID before recovery is invalid"))?;
    if before_pid == 0
        || core_property(config, runner, "ActiveState", "core_liveness_state_before")? != "active"
    {
        return terminal_result(
            &mut ledger,
            &key,
            state.head.as_deref(),
            now,
            "rejected",
            Some(&request_sha256),
            Some(request.nonce),
            Some(&request.generation_id),
            "core_was_not_active_with_a_live_main_pid",
            None,
        );
    }
    let restart = run_systemctl(
        config,
        runner,
        "core_liveness_restart",
        &["restart", CORE_SERVICE],
        Duration::from_secs(60),
    )?;
    if restart.timed_out || restart.exit_code != Some(0) {
        let _ = terminal_result(
            &mut ledger,
            &key,
            state.head.as_deref(),
            now,
            "failed",
            Some(&request_sha256),
            Some(request.nonce),
            Some(&request.generation_id),
            "fixed_core_restart_failed",
            Some(restart.clone()),
        )?;
        return Err(Error::new("fixed core liveness restart failed"));
    }
    let after_pid = core_property(config, runner, "MainPID", "core_liveness_pid_after")?
        .parse::<u32>()
        .map_err(|_| Error::new("core MainPID after recovery is invalid"))?;
    if after_pid == 0
        || after_pid == before_pid
        || core_property(config, runner, "ActiveState", "core_liveness_state_after")? != "active"
        || read_generation_binding(config, require_root)? != current_generation
    {
        let _ = terminal_result(
            &mut ledger,
            &key,
            state.head.as_deref(),
            now,
            "failed",
            Some(&request_sha256),
            Some(request.nonce),
            Some(&request.generation_id),
            "post_restart_core_or_generation_health_failed",
            Some(restart),
        )?;
        return Err(Error::new("post-restart core liveness verification failed"));
    }
    terminal_result(
        &mut ledger,
        &key,
        state.head.as_deref(),
        now,
        "restarted",
        Some(&request_sha256),
        Some(request.nonce),
        Some(&request.generation_id),
        &request.reason,
        Some(restart),
    )
}

fn validate_request(
    appliance_id: &str,
    request: &Request,
    current_generation: &str,
    now: u64,
    state: &LedgerState,
) -> Result<()> {
    if request.schema != REQUEST_SCHEMA
        || request.authority
            != "mutable_runtime_liveness_request_not_authorship_or_restart_authority"
        || request.appliance_id != appliance_id
        || request.generation_id != current_generation
        || request.nonce.is_nil()
        || !matches!(
            request.reason.as_str(),
            "edge_model_turn_timeout" | "edge_headless_idle_timeout"
        )
        || !valid_trace(&request.trace)
    {
        return Err(Error::new(
            "request identity, generation, reason, or trace failed",
        ));
    }
    if request.requested_at_unix_ms > now.saturating_add(MAX_FUTURE_SKEW_MS)
        || now.saturating_sub(request.requested_at_unix_ms) > MAX_REQUEST_AGE_MS
    {
        return Err(Error::new("request freshness failed"));
    }
    if state.seen_nonces.contains(&request.nonce) {
        return Err(Error::new("request nonce replayed"));
    }
    rate_gate(&state.restarted_at, now)
}

fn valid_trace(trace: &RequestTrace) -> bool {
    trace.schema_version == 1
        && !trace.trace_id.is_nil()
        && !trace.span_id.is_nil()
        && trace.turn_id.is_none_or(|value| !value.is_nil())
        && trace
            .parent_span_id
            .is_none_or(|value| !value.is_nil() && value != trace.span_id)
        && trace.session_id.as_deref().is_none_or(valid_label)
        && trace.chain_id.as_deref().is_none_or(valid_label)
}

fn valid_label(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 96 && !value.chars().any(char::is_control)
}

fn rate_gate(restarted_at: &[u64], now: u64) -> Result<()> {
    if restarted_at
        .last()
        .is_some_and(|last| now.saturating_sub(*last) < MIN_RESTART_INTERVAL_MS)
    {
        return Err(Error::new("core liveness restart cooldown is active"));
    }
    let window_start = now.saturating_sub(RESTART_WINDOW_MS);
    if restarted_at
        .iter()
        .filter(|timestamp| **timestamp >= window_start)
        .count()
        >= MAX_RESTARTS_PER_WINDOW
    {
        return Err(Error::new("core liveness restart-loop guard is active"));
    }
    Ok(())
}

fn request_path(config: &Config) -> PathBuf {
    config.roots.workspace.join("runtime").join(REQUEST_NAME)
}

fn ledger_path(config: &Config) -> PathBuf {
    config.roots.supervisor_state.join(LEDGER_NAME)
}

fn read_runtime_request(
    path: &Path,
    expected_path: &Path,
    expected_owner: u32,
    expected_group: u32,
) -> Result<Vec<u8>> {
    if path != expected_path {
        return Err(Error::new("core liveness request path is not exact"));
    }
    let before = fs::symlink_metadata(path)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    let identity = |metadata: &fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
        )
    };
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.uid() != expected_owner
        || before.gid() != expected_group
        || before.mode() & 0o777 != 0o640
        || before.len() == 0
        || before.len() > MAX_REQUEST_BYTES
        || identity(&before) != identity(&opened)
    {
        return Err(Error::new(
            "runtime request ownership, mode, link, or size failed",
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    (&mut file)
        .take(MAX_REQUEST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if bytes.len() as u64 != before.len()
        || identity(&opened) != identity(&after)
        || identity(&after) != identity(&path_after)
    {
        return Err(Error::new("runtime request changed while being consumed"));
    }
    Ok(bytes)
}

fn open_ledger(config: &Config, require_root: bool) -> Result<File> {
    let path = ledger_path(config);
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(Error::new("core liveness ledger identity failed"));
    }
    Ok(file)
}

fn replay_ledger(file: &mut File, key: &LedgerKey) -> Result<LedgerState> {
    let metadata = file.metadata()?;
    if metadata.len() > MAX_LEDGER_BYTES {
        return Err(Error::new("core liveness ledger exceeds its bound"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(MAX_LEDGER_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(Error::new("core liveness ledger changed during replay"));
    }
    let mut state = LedgerState::default();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_slice(line)?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::new("core liveness ledger record is not an object"))?;
        let digest = verify_record(&value, key, DOMAIN)?;
        if object.get("schema").and_then(Value::as_str) != Some(RECORD_SCHEMA)
            || object.get("authority").and_then(Value::as_str) != Some(AUTHORITY)
            || object.get("previous_record_sha256")
                != Some(&state.head.clone().map_or(Value::Null, Value::String))
        {
            return Err(Error::new("core liveness ledger chain failed"));
        }
        if let Some(nonce) = object
            .get("nonce")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        {
            state.seen_nonces.push(nonce);
        }
        if object.get("status").and_then(Value::as_str) == Some("restarted") {
            state.restarted_at.push(
                object
                    .get("recorded_at_unix_ms")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| Error::new("core liveness restart timestamp is invalid"))?,
            );
        }
        state.head = Some(digest);
    }
    Ok(state)
}

#[allow(clippy::too_many_arguments)]
fn terminal_result(
    ledger: &mut File,
    key: &LedgerKey,
    previous: Option<&str>,
    now: u64,
    status: &'static str,
    request_sha256: Option<&str>,
    nonce: Option<Uuid>,
    generation_id: Option<&str>,
    reason: &str,
    restart_receipt: Option<CommandReceipt>,
) -> Result<RecoveryResult> {
    if !matches!(status, "rejected" | "failed" | "restarted") {
        return Err(Error::new("core liveness terminal status is invalid"));
    }
    let reason = reason.chars().take(240).collect::<String>();
    let mut record = serde_json::json!({
        "schema": RECORD_SCHEMA,
        "recorded_at_unix_ms": now,
        "status": status,
        "request_sha256": request_sha256,
        "nonce": nonce,
        "generation_id": generation_id,
        "reason": reason,
        "service": CORE_SERVICE,
        "restart_receipt": restart_receipt,
        "previous_record_sha256": previous,
        "authority": AUTHORITY,
    });
    let record_sha256 = seal_record(&mut record, key, DOMAIN)?;
    ledger.seek(SeekFrom::End(0))?;
    ledger.write_all(&canonical_json(&record)?)?;
    ledger.write_all(b"\n")?;
    ledger.sync_all()?;
    Ok(RecoveryResult {
        schema: RECORD_SCHEMA,
        status,
        request_sha256: request_sha256.map(str::to_owned),
        nonce,
        generation_id: generation_id.map(str::to_owned),
        reason,
        service: CORE_SERVICE,
        restart_receipt,
        record_sha256: Some(record_sha256),
        authority: AUTHORITY,
    })
}

fn core_property(
    config: &Config,
    runner: &mut dyn NativeRunner,
    property: &str,
    label: &'static str,
) -> Result<String> {
    if !matches!(property, "ActiveState" | "MainPID") {
        return Err(Error::new("unsupported core liveness property"));
    }
    let spec = systemctl_spec(
        config,
        label,
        &["show", CORE_SERVICE, "--property", property, "--value"],
        Duration::from_secs(15),
    )?;
    let (receipt, output) = runner.run_capture(&spec, 128)?;
    if receipt.timed_out || receipt.exit_code != Some(0) {
        return Err(Error::new("fixed core property query failed"));
    }
    let value = std::str::from_utf8(&output)
        .map_err(|_| Error::new("fixed core property output is not UTF-8"))?
        .trim();
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return Err(Error::new("fixed core property output is invalid"));
    }
    Ok(value.to_owned())
}

fn run_systemctl(
    config: &Config,
    runner: &mut dyn NativeRunner,
    label: &'static str,
    arguments: &[&str],
    timeout: Duration,
) -> Result<CommandReceipt> {
    runner.run(&systemctl_spec(config, label, arguments, timeout)?)
}

fn systemctl_spec(
    config: &Config,
    label: &'static str,
    arguments: &[&str],
    timeout: Duration,
) -> Result<CommandSpec> {
    if config.services.core != CORE_SERVICE
        || arguments.iter().any(|argument| {
            argument.contains(char::is_whitespace)
                || argument.contains(['/', '\\', '\0'])
                || argument.starts_with('-') && *argument != "--property" && *argument != "--value"
        })
    {
        return Err(Error::new(
            "core liveness command escaped the fixed service plan",
        ));
    }
    Ok(CommandSpec {
        label,
        executable: config.executables.systemctl.clone(),
        arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
        current_dir: PathBuf::from("/"),
        environment: BTreeMap::new(),
        timeout,
        run_as_uid: None,
        run_as_gid: None,
    })
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};

    use super::{
        LedgerState, MAX_REQUEST_AGE_MS, MAX_RESTARTS_PER_WINDOW, MIN_RESTART_INTERVAL_MS,
        RESTART_WINDOW_MS, Request, RequestTrace, rate_gate, read_runtime_request, valid_trace,
        validate_request,
    };
    use uuid::Uuid;

    fn request(now: u64) -> Request {
        Request {
            schema: super::REQUEST_SCHEMA.to_owned(),
            appliance_id: "avado-edge".to_owned(),
            generation_id: "generation-a".to_owned(),
            requested_at_unix_ms: now,
            nonce: Uuid::new_v4(),
            reason: "edge_model_turn_timeout".to_owned(),
            trace: RequestTrace {
                schema_version: 1,
                trace_id: Uuid::new_v4(),
                turn_id: Some(Uuid::new_v4()),
                span_id: Uuid::new_v4(),
                parent_span_id: None,
                session_id: Some("edge-autonomous-a".to_owned()),
                chain_id: Some("chain-a".to_owned()),
            },
            authority: "mutable_runtime_liveness_request_not_authorship_or_restart_authority"
                .to_owned(),
        }
    }

    #[test]
    fn typed_request_denies_unknown_fields_and_invalid_trace() {
        let value = serde_json::to_value(request(10_000)).unwrap();
        let mut object = value.as_object().unwrap().clone();
        object.insert("service".to_owned(), serde_json::json!("ssh.service"));
        assert!(serde_json::from_value::<Request>(serde_json::Value::Object(object)).is_err());

        let mut request = request(10_000);
        assert!(valid_trace(&request.trace));
        request.trace.trace_id = Uuid::nil();
        assert!(!valid_trace(&request.trace));
    }

    #[test]
    fn durable_rate_gate_blocks_cooldown_and_restart_loops() {
        let now = 100 * RESTART_WINDOW_MS;
        assert!(rate_gate(&[], now).is_ok());
        assert!(rate_gate(&[now.saturating_sub(MIN_RESTART_INTERVAL_MS - 1)], now).is_err());
        let spaced = (0..MAX_RESTARTS_PER_WINDOW)
            .map(|index| {
                now.saturating_sub(
                    u64::try_from(index + 1)
                        .unwrap_or(u64::MAX)
                        .saturating_mul(MIN_RESTART_INTERVAL_MS),
                )
            })
            .collect::<Vec<_>>();
        assert!(rate_gate(&spaced, now).is_err());
        assert!(rate_gate(&[now.saturating_sub(RESTART_WINDOW_MS + 1)], now).is_ok());
    }

    #[test]
    fn request_identity_requires_exact_path_mode_link_and_runtime_owner() {
        let root = std::env::temp_dir().join(format!(
            "astrid-core-liveness-request-identity-{}",
            Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("create request fixture");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o770)).expect("request dir mode");
        let path = root.join(super::REQUEST_NAME);
        fs::write(&path, b"{}\n").expect("request");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("request mode");
        let metadata = fs::metadata(&path).expect("metadata");
        assert!(
            read_runtime_request(&path, &path, metadata.uid(), metadata.gid()).is_ok(),
            "exact runtime-owned request must be readable"
        );
        assert!(
            read_runtime_request(
                &path,
                &root.join("other.request.json"),
                metadata.uid(),
                metadata.gid(),
            )
            .is_err()
        );

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("narrow mode");
        assert!(read_runtime_request(&path, &path, metadata.uid(), metadata.gid()).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("restore mode");
        let hardlink = root.join("request-hardlink");
        fs::hard_link(&path, &hardlink).expect("hardlink");
        assert!(read_runtime_request(&path, &path, metadata.uid(), metadata.gid()).is_err());
        fs::remove_file(&hardlink).expect("remove hardlink");

        let target = root.join("target");
        fs::rename(&path, &target).expect("rename target");
        symlink(&target, &path).expect("symlink request");
        assert!(read_runtime_request(&path, &path, metadata.uid(), metadata.gid()).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn request_validation_binds_appliance_generation_freshness_and_nonce() {
        let now = 10 * MAX_REQUEST_AGE_MS;
        let value = request(now);
        assert!(
            validate_request(
                "avado-edge",
                &value,
                "generation-a",
                now,
                &LedgerState::default(),
            )
            .is_ok()
        );
        assert!(
            validate_request(
                "icp-edge",
                &value,
                "generation-a",
                now,
                &LedgerState::default(),
            )
            .is_err()
        );
        assert!(
            validate_request(
                "avado-edge",
                &value,
                "generation-b",
                now,
                &LedgerState::default(),
            )
            .is_err()
        );
        let mut stale = value.clone();
        stale.requested_at_unix_ms = now.saturating_sub(MAX_REQUEST_AGE_MS.saturating_add(1));
        assert!(
            validate_request(
                "avado-edge",
                &stale,
                "generation-a",
                now,
                &LedgerState::default(),
            )
            .is_err()
        );
        let replayed = LedgerState {
            seen_nonces: vec![value.nonce],
            ..LedgerState::default()
        };
        assert!(validate_request("avado-edge", &value, "generation-a", now, &replayed).is_err());
    }
}
