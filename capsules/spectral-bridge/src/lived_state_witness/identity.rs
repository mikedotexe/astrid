use std::path::Path;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{LivedStateBuildCandidateV1, LivedStateClockSampleV1, LivedStateProcessIdentityV1};
use crate::paths::bridge_paths;

#[derive(Debug, Clone)]
pub(super) struct StartupIdentityV1 {
    pub process: LivedStateProcessIdentityV1,
    pub build_candidate: Option<LivedStateBuildCandidateV1>,
    pub started_instant: Instant,
}

static STARTUP_IDENTITY: OnceLock<StartupIdentityV1> = OnceLock::new();

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json_sha256(value: &Value) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    sha256_bytes(&encoded)
}

fn optional_hash(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).and_then(|value| {
        (value.len() == 64 && value.bytes().all(|ch| ch.is_ascii_hexdigit()))
            .then(|| value.to_ascii_lowercase())
    })
}

fn bounded_text(value: Option<&Value>, max_len: usize) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(max_len).collect())
}

fn read_build_candidate(
    path: &Path,
    started_at_unix_ms: u64,
) -> Option<LivedStateBuildCandidateV1> {
    let bytes = std::fs::read(path).ok()?;
    let manifest_sha256 = sha256_bytes(&bytes);
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let repository = value.get("repository");
    let protocol = value.get("protocol");
    let artifacts = value.get("artifacts");
    let dirty_state_sha256 = repository.map(|repository| {
        canonical_json_sha256(&json!({
            "dirty": repository.get("dirty").and_then(Value::as_bool),
            "dirty_paths": repository.get("dirty_paths").cloned().unwrap_or(Value::Null),
        }))
    });
    Some(LivedStateBuildCandidateV1::new(
        manifest_sha256,
        optional_hash(repository.and_then(|value| value.get("source_identity_sha256"))),
        dirty_state_sha256,
        optional_hash(
            artifacts
                .and_then(|value| value.get("spectral-bridge"))
                .and_then(|value| value.get("sha256")),
        ),
        bounded_text(protocol.and_then(|value| value.get("revision")), 80),
        bounded_text(protocol.and_then(|value| value.get("version")), 24),
        started_at_unix_ms,
    ))
}

fn capture_startup_identity() -> StartupIdentityV1 {
    let started_instant = Instant::now();
    let started_at_unix_ms = unix_ms();
    let pid = std::process::id();
    let executable_basename = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "spectral-bridge-server".to_string());
    let nonce = rand::random::<u64>();
    let mut runtime_hasher = Sha256::new();
    runtime_hasher.update(b"astrid-lived-state-runtime-instance-v1\0");
    runtime_hasher.update(pid.to_le_bytes());
    runtime_hasher.update(started_at_unix_ms.to_le_bytes());
    runtime_hasher.update(nonce.to_le_bytes());
    runtime_hasher.update(executable_basename.as_bytes());
    let runtime_instance_id = format!("runtime_{:x}", runtime_hasher.finalize());
    let process_identity_sha256 = sha256_bytes(
        format!("{pid}\0{started_at_unix_ms}\0{executable_basename}\0{runtime_instance_id}")
            .as_bytes(),
    );
    let process = LivedStateProcessIdentityV1::new(
        pid,
        started_at_unix_ms,
        executable_basename,
        runtime_instance_id,
        process_identity_sha256,
    );
    let build_candidate = read_build_candidate(
        &bridge_paths()
            .bridge_workspace()
            .join("deployment_manifests/spectral-bridge.json"),
        started_at_unix_ms,
    );
    StartupIdentityV1 {
        process,
        build_candidate,
        started_instant,
    }
}

pub(super) fn initialize() -> &'static StartupIdentityV1 {
    STARTUP_IDENTITY.get_or_init(capture_startup_identity)
}

pub(super) fn snapshot() -> StartupIdentityV1 {
    initialize().clone()
}

pub(super) fn clock_sample() -> LivedStateClockSampleV1 {
    let identity = initialize();
    let monotonic_ns = identity
        .started_instant
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX);
    LivedStateClockSampleV1 {
        unix_ms: unix_ms(),
        monotonic_ns,
    }
}

#[cfg(test)]
pub(super) fn build_candidate_from_path_for_test(
    path: &Path,
    started_at_unix_ms: u64,
) -> Option<LivedStateBuildCandidateV1> {
    read_build_candidate(path, started_at_unix_ms)
}
