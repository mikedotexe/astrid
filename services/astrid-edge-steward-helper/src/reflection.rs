//! Exact read-only validation of root-granted scheduled-reflection admission.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::config::Config;
use crate::util::{read_stable_regular, sha256, validate_hex64, validate_identifier};
use crate::{Error, Result};

const LEASE_PATH: &str = "/run/astrid-edge-self-change/reflection.json";
const ADMISSION_PATH: &str = "/run/astrid-edge-self-change/reflection-admission.json";
const LEASE_SCHEMA: &str = "astrid.edge_scheduled_reflection.lease.v1";
const ADMISSION_SCHEMA: &str = "astrid.edge_scheduled_reflection.admission.v2";
const LEASE_KIND: &str = "scheduled_reflection";
const LEASE_OWNER: &str = "immutable_astrid_edge_reflection_guard";
const MAXIMUM_BYTES: u64 = 64 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReflectionLease {
    schema: String,
    lease_kind: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
    host_boot_id: String,
    service_invocation_id: String,
    generation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionMarker {
    schema: String,
    lease_schema: String,
    lease_kind: String,
    lease_id: String,
    lease_nonce_sha256: String,
    lease_payload_sha256: String,
    generation_id: String,
    host_boot_id: String,
    service_invocation_id: String,
    admitted_at_unix_ms: u64,
    drain_barrier_sequence: u64,
    core_ack_sha256: String,
    edge_ack_sha256: String,
    model_lock_device: u64,
    model_lock_inode: u64,
    scheduled_due_nonce_sha256: Option<String>,
    model_start_authority: String,
    authority: String,
}

/// Require an exact root-created admission bound to this service invocation,
/// boot, generation, lease, and model-lock inode.
pub fn require(config: &Config) -> Result<()> {
    load(config).map(drop)
}

/// Require that the root admission grants a fresh model start, not merely
/// recovery of an exact already-prepared authored transaction.
pub fn require_model_start(config: &Config, due_nonce: &str) -> Result<()> {
    validate_identifier(due_nonce, "scheduled due nonce")?;
    let marker = load(config)?;
    if !marker_authorizes_model_start(&marker, due_nonce) {
        return Err(Error::new(
            "root admission does not authorize a fresh model start for this due slot",
        ));
    }
    Ok(())
}

fn marker_authorizes_model_start(marker: &AdmissionMarker, due_nonce: &str) -> bool {
    marker.model_start_authority == "root_schedule_model_start_allowed"
        && marker
            .scheduled_due_nonce_sha256
            .as_ref()
            .is_none_or(|expected| expected == &sha256(due_nonce.as_bytes()))
}

/// Report only the exact no-artifact state used to distinguish the harmless
/// root/helper one-second due-boundary race from a malformed or revoked
/// admission. A partial pair, symlink, or any present object is not "absent".
pub fn artifacts_absent() -> Result<bool> {
    for path in [Path::new(LEASE_PATH), Path::new(ADMISSION_PATH)] {
        match fs::symlink_metadata(path) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

fn load(config: &Config) -> Result<AdmissionMarker> {
    let invocation_id = std::env::var("INVOCATION_ID")
        .map_err(|_| Error::new("systemd reflection invocation identity is absent"))?;
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_owned();
    require_at(
        config,
        Path::new(LEASE_PATH),
        Path::new(ADMISSION_PATH),
        &boot_id,
        &invocation_id,
        unix_millis(),
        trusted_root_uid(&config.current_generation)?,
        nix::unistd::getegid().as_raw(),
    )
}

#[allow(clippy::too_many_arguments)]
fn require_at(
    config: &Config,
    lease_path: &Path,
    marker_path: &Path,
    boot_id: &str,
    invocation_id: &str,
    now: u64,
    root_uid: u32,
    steward_gid: u32,
) -> Result<AdmissionMarker> {
    validate_parent(lease_path, root_uid)?;
    let lease_bytes = read_owned(lease_path, root_uid, config.workspace_gid, 0o440)?;
    let marker_bytes = read_owned(marker_path, root_uid, steward_gid, 0o440)?;
    let lease: ReflectionLease = serde_json::from_slice(&lease_bytes)?;
    let marker: AdmissionMarker = serde_json::from_slice(&marker_bytes)?;
    let nonce_sha256 = sha256(lease.nonce.as_bytes());
    let generation = read_generation(&config.current_generation, root_uid)?;
    let lock = fs::symlink_metadata(&config.model_lock)?;
    if lease.schema != LEASE_SCHEMA
        || lease.lease_kind != LEASE_KIND
        || lease.owner != LEASE_OWNER
        || lease.reason != "scheduled_reflection"
        || lease.created_at_unix_ms > now.saturating_add(30_000)
        || now >= lease.expires_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            != 3 * 60 * 60 * 1_000
        || lease.host_boot_id != boot_id
        || lease.service_invocation_id != invocation_id
        || lease.generation_id != generation
        || validate_hex64(&lease.nonce, "reflection nonce").is_err()
        || validate_identifier(&lease.generation_id, "reflection generation").is_err()
        || lease.lease_id != format!("reflection-{}", &nonce_sha256[..24])
        || marker.schema != ADMISSION_SCHEMA
        || marker.lease_schema != LEASE_SCHEMA
        || marker.lease_kind != LEASE_KIND
        || marker.lease_id != lease.lease_id
        || marker.lease_nonce_sha256 != nonce_sha256
        || marker.lease_payload_sha256 != sha256(&lease_bytes)
        || marker.generation_id != generation
        || marker.host_boot_id != boot_id
        || marker.service_invocation_id != invocation_id
        || marker.admitted_at_unix_ms < lease.created_at_unix_ms
        || marker.admitted_at_unix_ms > now.saturating_add(30_000)
        || marker.drain_barrier_sequence == 0
        || validate_hex64(&marker.core_ack_sha256, "core ACK hash").is_err()
        || validate_hex64(&marker.edge_ack_sha256, "edge ACK hash").is_err()
        || marker
            .scheduled_due_nonce_sha256
            .as_ref()
            .is_some_and(|value| validate_hex64(value, "scheduled due nonce hash").is_err())
        || !matches!(
            marker.model_start_authority.as_str(),
            "root_schedule_model_start_allowed"
                | "root_schedule_prepared_recovery_only"
                | "root_schedule_legacy_migration_only"
        )
        || (marker.model_start_authority != "root_schedule_model_start_allowed"
            && marker.scheduled_due_nonce_sha256.is_none())
        || marker.authority != "root_verified_drain_and_model_lock_handoff_not_activation_authority"
        || !lock.is_file()
        || lock.file_type().is_symlink()
        || lock.nlink() != 1
        || lock.uid() != root_uid
        || lock.permissions().mode() & 0o777 != 0o640
        || marker.model_lock_device != lock.dev()
        || marker.model_lock_inode != lock.ino()
    {
        return Err(Error::new(
            "scheduled-reflection admission is absent, stale, or inexact",
        ));
    }
    Ok(marker)
}

fn read_owned(path: &Path, uid: u32, gid: u32, mode: u32) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != uid
        || metadata.gid() != gid
        || metadata.permissions().mode() & 0o777 != mode
    {
        return Err(Error::new("reflection artifact ownership or mode failed"));
    }
    read_stable_regular(path, MAXIMUM_BYTES)
}

fn validate_parent(path: &Path, uid: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("reflection artifact has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.gid() != uid
        || metadata.permissions().mode() & 0o777 != 0o755
    {
        return Err(Error::new("reflection parent identity failed"));
    }
    Ok(())
}

fn trusted_root_uid(path: &Path) -> Result<u32> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(Error::new(
            "generation binding cannot establish root identity",
        ));
    }
    Ok(0)
}

fn read_generation(path: &Path, uid: u32) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.uid() != uid || metadata.permissions().mode() & 0o777 != 0o444 {
        return Err(Error::new("generation binding identity failed"));
    }
    let bytes = read_stable_regular(path, 256)?;
    let generation = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|value| value.strip_suffix('\n'))
        .ok_or_else(|| Error::new("generation binding is not canonical"))?;
    validate_identifier(generation, "generation binding")?;
    Ok(generation.to_owned())
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ADMISSION_SCHEMA, AdmissionMarker, LEASE_KIND, LEASE_SCHEMA, ReflectionLease,
        marker_authorizes_model_start,
    };

    #[test]
    fn reflection_lease_never_aliases_generation_transition() {
        let lease = ReflectionLease {
            schema: LEASE_SCHEMA.to_owned(),
            lease_kind: LEASE_KIND.to_owned(),
            created_at_unix_ms: 1,
            expires_at_unix_ms: 2,
            reason: "scheduled_reflection".to_owned(),
            owner: "immutable_astrid_edge_reflection_guard".to_owned(),
            lease_id: "reflection-example".to_owned(),
            nonce: "a".repeat(64),
            host_boot_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            service_invocation_id: "b".repeat(32),
            generation_id: "generation-1".to_owned(),
        };
        assert_eq!(lease.lease_kind, "scheduled_reflection");
        assert_ne!(lease.schema, "astrid.edge_self_change.maintenance_lease.v2");
        assert_ne!(ADMISSION_SCHEMA, "astrid.edge.maintenance_barrier.v2");
    }

    #[test]
    fn reflection_lease_shape_rejects_unknown_fields() {
        let mut value = serde_json::json!({
            "schema": LEASE_SCHEMA,
            "lease_kind": LEASE_KIND,
            "created_at_unix_ms": 1,
            "expires_at_unix_ms": 2,
            "reason": "scheduled_reflection",
            "owner": "immutable_astrid_edge_reflection_guard",
            "lease_id": "reflection-example",
            "nonce": "a".repeat(64),
            "host_boot_id": "11111111-1111-4111-8111-111111111111",
            "service_invocation_id": "b".repeat(32),
            "generation_id": "generation-1"
        });
        assert!(serde_json::from_value::<ReflectionLease>(value.clone()).is_ok());
        value.as_object_mut().unwrap().insert(
            "lease_schema".to_owned(),
            serde_json::Value::String("wrong-domain".into()),
        );
        assert!(serde_json::from_value::<ReflectionLease>(value).is_err());
    }

    #[test]
    fn recovery_only_marker_cannot_authorize_a_model_start() {
        let due_nonce = "due-12345";
        let mut marker = AdmissionMarker {
            schema: ADMISSION_SCHEMA.to_owned(),
            lease_schema: LEASE_SCHEMA.to_owned(),
            lease_kind: LEASE_KIND.to_owned(),
            lease_id: "reflection-example".to_owned(),
            lease_nonce_sha256: "a".repeat(64),
            lease_payload_sha256: "b".repeat(64),
            generation_id: "generation-1".to_owned(),
            host_boot_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            service_invocation_id: "c".repeat(32),
            admitted_at_unix_ms: 1,
            drain_barrier_sequence: 1,
            core_ack_sha256: "d".repeat(64),
            edge_ack_sha256: "e".repeat(64),
            model_lock_device: 1,
            model_lock_inode: 2,
            scheduled_due_nonce_sha256: Some(super::sha256(due_nonce.as_bytes())),
            model_start_authority: "root_schedule_model_start_allowed".to_owned(),
            authority: "root_verified_drain_and_model_lock_handoff_not_activation_authority"
                .to_owned(),
        };
        assert!(marker_authorizes_model_start(&marker, due_nonce));
        assert!(!marker_authorizes_model_start(&marker, "due-12346"));
        marker.model_start_authority = "root_schedule_prepared_recovery_only".to_owned();
        assert!(!marker_authorizes_model_start(&marker, due_nonce));
    }
}
