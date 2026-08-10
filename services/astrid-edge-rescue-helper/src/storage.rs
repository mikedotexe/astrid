//! Immutable attestation of bounded runtime and rollback filesystems.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::fs_guard::{MAX_JSON_BYTES, read_json, require_private_root_file};
use crate::{Error, Result};

const ATTESTATION_SCHEMA: &str = "astrid.edge_state_store.attestation.v1";
const AUTHORITY: &str = "immutable_root_storage_boundary_not_model_authorship";
const MINIMUM_FREE_INODES: u64 = 4_096;
const ROLLBACK_MINIMUM_FREE_INODES: u64 = 65_536;
const MAXIMUM_ATTESTATION_AGE_MILLISECONDS: u64 = 10 * 60 * 1_000;
const MAXIMUM_FUTURE_SKEW_MILLISECONDS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageAttestation {
    pub schema: String,
    pub appliance_id: String,
    pub config_sha256: String,
    pub authority: String,
    pub recorded_at_unix_ms: u64,
    pub host_boot_id: String,
    pub full_inventory_verified: bool,
    pub python: PythonAttestation,
    pub backing: BackingAttestation,
    pub runtime: RuntimeAttestation,
    pub rollback: RollbackAttestation,
    pub migration_phase: String,
    pub source_backups_retained: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonAttestation {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackingAttestation {
    pub target: String,
    pub uuid: String,
    pub available_bytes: u64,
    pub minimum_available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAttestation {
    pub filesystem_uuid: String,
    pub mount_device: u64,
    pub available_bytes_unprivileged: u64,
    pub available_bytes_root: u64,
    pub free_inodes: u64,
    pub emergency_inode_reserve_files: u64,
    pub reserved_block_percent: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackAttestation {
    pub filesystem_uuid: String,
    pub mount_device: u64,
    pub available_bytes: u64,
    pub free_inodes: u64,
}

pub fn verify(config: &Config, full_inventory: bool) -> Result<StorageAttestation> {
    let path = if full_inventory {
        &config.storage.install_attestation
    } else {
        &config.storage.health_attestation
    };
    require_private_root_file(path, "bounded storage attestation")?;
    let attestation: StorageAttestation = read_json(path, MAX_JSON_BYTES)?;
    validate(config, &attestation, full_inventory)?;
    Ok(attestation)
}

fn validate(config: &Config, attestation: &StorageAttestation, full_inventory: bool) -> Result<()> {
    let root_reserve = attestation
        .runtime
        .available_bytes_root
        .checked_sub(attestation.runtime.available_bytes_unprivileged)
        .ok_or_else(|| Error::new("runtime root reserve accounting underflowed"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new("system clock predates the Unix epoch"))?
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_owned();
    let runtime_device = fs::metadata(&config.storage.runtime_state_mount)?.dev();
    let rollback_device = fs::metadata(&config.storage.rollback_mount)?.dev();
    if attestation.schema != ATTESTATION_SCHEMA
        || attestation.authority != AUTHORITY
        || attestation.appliance_id != config.appliance_id
        || attestation.config_sha256 != config.storage.config_sha256
        || attestation.python.path != config.executables.python.path.to_string_lossy()
        || attestation.python.sha256 != config.executables.python.sha256
        || attestation.recorded_at_unix_ms == 0
        || attestation.recorded_at_unix_ms
            > now.saturating_add(MAXIMUM_FUTURE_SKEW_MILLISECONDS)
        || now.saturating_sub(attestation.recorded_at_unix_ms)
            > MAXIMUM_ATTESTATION_AGE_MILLISECONDS
        || attestation.host_boot_id != boot_id
        // The install attestation proves the stopped-source inventory until
        // migration acceptance.  Thereafter live state is expected to evolve;
        // the same sealed path proves the fixed filesystem/mount/capacity
        // boundary without falsely comparing it to installation-time bytes.
        || (!full_inventory && attestation.full_inventory_verified)
        || (full_inventory
            && attestation.migration_phase == "mounted_verified"
            && !attestation.full_inventory_verified)
        || attestation.backing.uuid != config.storage.backing_uuid
        || attestation.backing.available_bytes < config.storage.host_reserve_bytes
        || attestation.backing.minimum_available_bytes != config.storage.host_reserve_bytes
        || attestation.runtime.filesystem_uuid != config.storage.runtime_filesystem_uuid
        || attestation.rollback.filesystem_uuid != config.storage.rollback_filesystem_uuid
        || runtime_device != attestation.runtime.mount_device
        || rollback_device != attestation.rollback.mount_device
        || attestation.runtime.mount_device == attestation.rollback.mount_device
        || attestation.runtime.available_bytes_unprivileged
            < config.storage.store_minimum_free_bytes
        || root_reserve < config.storage.store_minimum_free_bytes
        || attestation.rollback.available_bytes < config.storage.store_minimum_free_bytes
        || attestation.runtime.free_inodes < MINIMUM_FREE_INODES
        || attestation.rollback.free_inodes < ROLLBACK_MINIMUM_FREE_INODES
        || attestation.runtime.emergency_inode_reserve_files
            != config.storage.emergency_inode_reserve_files
        || attestation.runtime.reserved_block_percent != 20
        || !matches!(
            attestation.migration_phase.as_str(),
            "mounted_verified" | "accepted"
        )
        || (attestation.migration_phase == "mounted_verified"
            && !attestation.source_backups_retained)
    {
        return Err(Error::new(
            "bounded runtime/rollback storage attestation failed",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ATTESTATION_SCHEMA, StorageAttestation};

    #[test]
    fn attestation_schema_is_not_runtime_authorship() {
        let value = serde_json::json!({
            "schema": ATTESTATION_SCHEMA,
            "appliance_id": "avado",
            "config_sha256": "a".repeat(64),
            "authority": "immutable_root_storage_boundary_not_model_authorship",
            "recorded_at_unix_ms": 1,
            "host_boot_id": "boot",
            "full_inventory_verified": false,
            "python": {"path":"/usr/bin/python3","sha256":"d".repeat(64)},
            "backing": {"target":"/","uuid":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","available_bytes":68_719_476_736_u64,"minimum_available_bytes":68_719_476_736_u64},
            "runtime": {"filesystem_uuid":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","mount_device":1,"available_bytes_unprivileged":4_294_967_296_u64,"available_bytes_root":10_737_418_240_u64,"free_inodes":4096,"emergency_inode_reserve_files":65536,"reserved_block_percent":20},
            "rollback": {"filesystem_uuid":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","mount_device":2,"available_bytes":8_589_934_592_u64,"free_inodes":65536},
            "migration_phase":"mounted_verified",
            "source_backups_retained":true
        });
        let parsed: StorageAttestation = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.schema, ATTESTATION_SCHEMA);
        assert!(parsed.authority.ends_with("not_model_authorship"));
    }
}
