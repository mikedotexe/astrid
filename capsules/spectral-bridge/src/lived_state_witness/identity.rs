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

fn bounded_text(value: Option<&Value>, max_len: usize) -> (Option<String>, Option<bool>) {
    let Some(value) = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return (None, None);
    };
    let complete = value.chars().count() <= max_len;
    (Some(value.chars().take(max_len).collect()), Some(complete))
}

fn read_build_candidate(
    path: &Path,
    started_at_unix_ms: u64,
) -> Option<LivedStateBuildCandidateV1> {
    let bytes = crate::deployment::manifest_bytes(path).ok()?;
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
    let (protocol_revision, protocol_revision_complete) =
        bounded_text(protocol.and_then(|value| value.get("revision")), 80);
    let (protocol_version, protocol_version_complete) =
        bounded_text(protocol.and_then(|value| value.get("version")), 24);
    Some(LivedStateBuildCandidateV1::new(
        manifest_sha256,
        optional_hash(repository.and_then(|value| value.get("source_identity_sha256"))),
        dirty_state_sha256,
        optional_hash(
            artifacts
                .and_then(|value| value.get("spectral-bridge"))
                .and_then(|value| value.get("sha256")),
        ),
        protocol_revision,
        protocol_revision_complete,
        protocol_version,
        protocol_version_complete,
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
        &crate::deployment::manifest_path(bridge_paths().bridge_workspace()),
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

#[cfg(test)]
mod staged_tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn staged_identity_readers_share_startup_binding() {
        const CHILD: &str = "ASTRID_STAGED_IDENTITY_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "lived_state_witness::identity::staged_tests::staged_identity_readers_share_startup_binding", "--nocapture"])
                .env(CHILD, "1")
                .output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let binary_hash = sha256_bytes(&fs::read(&executable).unwrap());
        let source_hash = "a".repeat(64);
        let path = root.path().join("staged.json");
        fs::write(&path, serde_json::to_vec(&json!({
            "schema": crate::deployment::MANIFEST_SCHEMA,
            "authority": crate::deployment::MANIFEST_AUTHORITY,
            "repository": {"available": true, "head": "1234567", "source_identity_sha256": source_hash},
            "artifacts": {"spectral-bridge": {"exists": true, "path": executable, "sha256": binary_hash}},
        })).unwrap()).unwrap();
        crate::deployment::configure(Some(&path)).unwrap();
        let selected = crate::deployment::manifest_path(root.path());
        let receipt = crate::deployment::verification_receipt().unwrap();
        let expected = receipt["deployment_identity"].as_str().unwrap();
        assert_eq!(
            crate::deployment::read_bound_manifest(&selected, &executable)
                .unwrap()
                .identity,
            expected
        );
        // Identity remains a startup witness; mutation authority checks the disk again.
        fs::write(&path, b"changed after startup").unwrap();
        assert_eq!(
            crate::signal_spine::signal_deployment_identity_v1(),
            expected
        );
        let context =
            crate::authority_temporal::current_context("test", "fixture", 0, 1, 1, root.path());
        assert_eq!(context.deployment_identity, expected);
        assert_eq!(context.source_identity, source_hash);
        let witness =
            serde_json::to_value(capture_startup_identity().build_candidate.unwrap()).unwrap();
        assert_eq!(witness["manifest_sha256"], receipt["manifest_sha256"]);
        assert_eq!(witness["source_identity_sha256"], source_hash);
        assert!(
            crate::deployment::read_bound_manifest(&selected, &executable)
                .unwrap_err()
                .contains("changed after startup")
        );
        assert!(
            !root
                .path()
                .join("deployment_manifests/spectral-bridge.json")
                .exists()
        );
    }
}
