//! Fail-closed immutable maintenance gate for scheduled reflection.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use serde::Deserialize;

use crate::config::Config;
use crate::util::{read_stable_regular, sha256};

const LEASE_SCHEMA: &str = "astrid.edge_self_change.maintenance_lease.v2";
const LEASE_OWNER: &str = "immutable_astrid_edge_rescue_helper";
const MAXIMUM_LEASE_BYTES: u64 = 8 * 1_024;
const MAXIMUM_LEASE_LIFETIME_MS: u64 = 48 * 60 * 60 * 1_000;
const MAXIMUM_CLOCK_LEAD_MS: u64 = 30_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaintenanceRecord {
    schema: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Clear,
    DeferredActive,
    DeferredMalformed,
}

impl Gate {
    #[must_use]
    pub const fn is_clear(self) -> bool {
        matches!(self, Self::Clear)
    }

    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Clear => "maintenance lease is absent or validly expired",
            Self::DeferredActive => "immutable maintenance transaction is active",
            Self::DeferredMalformed => "maintenance lease is malformed or unreadable",
        }
    }
}

#[must_use]
pub fn inspect(config: &Config) -> Gate {
    inspect_at(
        &config.maintenance_lease,
        &config.current_generation,
        unix_millis(),
    )
}

fn inspect_at(lease_path: &Path, generation_binding: &Path, now: u64) -> Gate {
    let Some(trusted_uid) = trusted_binding_uid(generation_binding) else {
        return Gate::DeferredMalformed;
    };
    let metadata = match fs::symlink_metadata(lease_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Gate::Clear,
        Err(_) => return Gate::DeferredMalformed,
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != trusted_uid
        || metadata.permissions().mode() & 0o777 != 0o444
        || metadata.len() > MAXIMUM_LEASE_BYTES
    {
        return Gate::DeferredMalformed;
    }
    let Ok(bytes) = read_stable_regular(lease_path, MAXIMUM_LEASE_BYTES) else {
        return Gate::DeferredMalformed;
    };
    let Ok(lease) = serde_json::from_slice::<MaintenanceRecord>(&bytes) else {
        return Gate::DeferredMalformed;
    };
    if !valid_lease(&lease, now) {
        return Gate::DeferredMalformed;
    }
    if lease.expires_at_unix_ms <= now {
        Gate::Clear
    } else {
        Gate::DeferredActive
    }
}

fn trusted_binding_uid(path: &Path) -> Option<u32> {
    let metadata = fs::symlink_metadata(path).ok()?;
    (metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.nlink() == 1
        && metadata.permissions().mode() & 0o022 == 0)
        .then_some(metadata.uid())
}

fn valid_lease(lease: &MaintenanceRecord, now: u64) -> bool {
    if lease.schema != LEASE_SCHEMA
        || lease.owner != LEASE_OWNER
        || lease.reason.is_empty()
        || lease.reason.chars().count() > 128
        || lease.reason.chars().any(char::is_control)
        || lease.created_at_unix_ms > now.saturating_add(MAXIMUM_CLOCK_LEAD_MS)
        || lease.expires_at_unix_ms <= lease.created_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            > MAXIMUM_LEASE_LIFETIME_MS
        || !is_lower_hex64(&lease.nonce)
    {
        return false;
    }
    let nonce_sha256 = sha256(lease.nonce.as_bytes());
    lease.lease_id == format!("lease-{}", &nonce_sha256[..24])
}

fn is_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
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
    use std::os::unix::fs::PermissionsExt;

    use serde_json::{Value, json};

    use super::{Gate, inspect_at};
    use crate::util::{canonical_json, sha256};

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let binding = temporary.path().join("current-generation");
        let lease = temporary.path().join("maintenance.json");
        fs::write(&binding, b"generation-1\n").unwrap();
        let mut permissions = fs::metadata(&binding).unwrap().permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&binding, permissions).unwrap();
        (temporary, binding, lease)
    }

    fn lease(now: u64, expires_at_unix_ms: u64) -> Value {
        let nonce = "a".repeat(64);
        let nonce_hash = sha256(nonce.as_bytes());
        json!({
            "schema": "astrid.edge_self_change.maintenance_lease.v2",
            "created_at_unix_ms": now.saturating_sub(1),
            "expires_at_unix_ms": expires_at_unix_ms,
            "reason": "immutable build",
            "owner": "immutable_astrid_edge_rescue_helper",
            "lease_id": format!("lease-{}", &nonce_hash[..24]),
            "nonce": nonce,
        })
    }

    fn write_lease(path: &Path, value: &Value) {
        if path.exists() || path.is_symlink() {
            fs::remove_file(path).unwrap();
        }
        fs::write(path, canonical_json(value).unwrap()).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(path, permissions).unwrap();
    }

    use std::path::Path;

    #[test]
    fn absent_and_exactly_expired_are_clear_while_active_is_deferred() {
        let (_temporary, binding, path) = fixture();
        let now = 1_000_000;
        assert_eq!(inspect_at(&path, &binding, now), Gate::Clear);
        write_lease(&path, &lease(now, now.saturating_add(1)));
        assert_eq!(inspect_at(&path, &binding, now), Gate::DeferredActive);
        write_lease(&path, &lease(now, now));
        assert_eq!(inspect_at(&path, &binding, now), Gate::Clear);
    }

    #[test]
    fn only_a_structurally_valid_expired_v2_lease_is_ignored() {
        let (_temporary, binding, path) = fixture();
        let now = 2_000_000;
        for invalid in [
            json!({"expires_at_unix_ms": now}),
            {
                let mut value = lease(now, now);
                value["schema"] = json!("astrid.edge_self_change.maintenance_lease.v1");
                value
            },
            {
                let mut value = lease(now, now);
                value["lease_id"] = json!("lease-forged");
                value
            },
            {
                let mut value = lease(now, now);
                value["extra"] = json!(true);
                value
            },
        ] {
            write_lease(&path, &invalid);
            assert_eq!(inspect_at(&path, &binding, now), Gate::DeferredMalformed);
        }
    }

    #[test]
    fn unsafe_identity_mode_and_lifetime_fail_closed() {
        let (_temporary, binding, path) = fixture();
        let now = 3_000_000;
        write_lease(&path, &lease(now, now.saturating_add(1)));
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&path, permissions).unwrap();
        assert_eq!(inspect_at(&path, &binding, now), Gate::DeferredMalformed);

        let mut too_long = lease(now, now.saturating_add(48 * 60 * 60 * 1_000 + 1));
        too_long["created_at_unix_ms"] = json!(now);
        write_lease(&path, &too_long);
        assert_eq!(inspect_at(&path, &binding, now), Gate::DeferredMalformed);

        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&binding, &path).unwrap();
        assert_eq!(inspect_at(&path, &binding, now), Gate::DeferredMalformed);
    }

    #[test]
    fn lease_appearing_between_initial_and_post_lock_checks_is_observed() {
        let (_temporary, binding, path) = fixture();
        let now = 4_000_000;
        assert_eq!(inspect_at(&path, &binding, now), Gate::Clear);

        // This write models the immutable updater acquiring maintenance after the steward's
        // first eligibility check but after the steward has subsequently obtained model.lock.
        write_lease(&path, &lease(now, now.saturating_add(60_000)));
        assert_eq!(inspect_at(&path, &binding, now), Gate::DeferredActive);
    }
}
