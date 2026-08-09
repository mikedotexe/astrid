//! Domain-separated authentication for immutable root lifecycle ledgers.
//!
//! A hash chain detects accidental or unauthenticated mutation only when its
//! head is already trusted. These helpers additionally bind every record to a
//! distinct root-only appliance key. The key is never shared with source,
//! candidate-intent, supervisor, or web-broker authority.

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;

use crate::fs_guard::{canonical_json, sha256};
use crate::{Error, Result};

const AUTH_SCHEMA: &str = "astrid.edge_rescue_helper.ledger_hmac.v1";
const KEY_BYTES: usize = 32;
const MAX_DOMAIN_BYTES: usize = 96;

type HmacSha256 = Hmac<Sha256>;

/// Exact root-owned ledger authentication key and its public fingerprint.
#[derive(Clone)]
pub struct LedgerKey {
    bytes: [u8; KEY_BYTES],
    fingerprint: String,
}

impl LedgerKey {
    /// Load an exact 32-byte, owner-read-only key without following symlinks.
    pub fn load(path: &Path, require_root: bool) -> Result<Self> {
        let before = fs::symlink_metadata(path)?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
        let mut file = options.open(path)?;
        let opened = file.metadata()?;
        validate_key_metadata(&before, require_root)?;
        validate_key_metadata(&opened, require_root)?;
        if file_identity(&before) != file_identity(&opened) {
            return Err(Error::new("ledger authentication key identity changed"));
        }

        let mut bounded = Vec::with_capacity(KEY_BYTES.saturating_add(1));
        (&mut file)
            .take(u64::try_from(KEY_BYTES.saturating_add(1)).unwrap_or(u64::MAX))
            .read_to_end(&mut bounded)?;
        let after = file.metadata()?;
        let path_after = fs::symlink_metadata(path)?;
        if bounded.len() != KEY_BYTES
            || file_identity(&opened) != file_identity(&after)
            || file_identity(&after) != file_identity(&path_after)
        {
            return Err(Error::new(
                "ledger authentication key length or identity changed",
            ));
        }
        let mut bytes = [0_u8; KEY_BYTES];
        bytes.copy_from_slice(&bounded);
        Ok(Self {
            fingerprint: sha256(&bytes),
            bytes,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(byte: u8) -> Self {
        let bytes = [byte; KEY_BYTES];
        Self {
            fingerprint: sha256(&bytes),
            bytes,
        }
    }

    /// Public SHA-256 fingerprint used to detect cross-key replay.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Add domain-separated HMAC authentication and a final record digest.
pub fn seal_record(record: &mut Value, key: &LedgerKey, domain: &str) -> Result<String> {
    validate_domain(domain)?;
    {
        let object = record
            .as_object_mut()
            .ok_or_else(|| Error::new("ledger record is not an object"))?;
        for reserved in [
            "authentication_schema",
            "ledger_domain",
            "ledger_key_sha256",
            "record_hmac_sha256",
            "record_sha256",
        ] {
            if object.contains_key(reserved) {
                return Err(Error::new("ledger record contains a reserved field"));
            }
        }
        object.insert(
            "authentication_schema".to_owned(),
            Value::String(AUTH_SCHEMA.to_owned()),
        );
        object.insert("ledger_domain".to_owned(), Value::String(domain.to_owned()));
        object.insert(
            "ledger_key_sha256".to_owned(),
            Value::String(key.fingerprint.clone()),
        );
    }
    let authentication = authenticate(key, &canonical_json(record)?)?;
    record
        .as_object_mut()
        .ok_or_else(|| Error::new("ledger record is not an object"))?
        .insert(
            "record_hmac_sha256".to_owned(),
            Value::String(authentication),
        );
    let digest = sha256(&canonical_json(record)?);
    record
        .as_object_mut()
        .ok_or_else(|| Error::new("ledger record is not an object"))?
        .insert("record_sha256".to_owned(), Value::String(digest.clone()));
    Ok(digest)
}

/// Verify the exact domain, key fingerprint, HMAC, and final record digest.
pub fn verify_record(record: &Value, key: &LedgerKey, domain: &str) -> Result<String> {
    validate_domain(domain)?;
    let object = record
        .as_object()
        .ok_or_else(|| Error::new("ledger record is not an object"))?;
    if object.get("authentication_schema").and_then(Value::as_str) != Some(AUTH_SCHEMA)
        || object.get("ledger_domain").and_then(Value::as_str) != Some(domain)
        || object.get("ledger_key_sha256").and_then(Value::as_str) != Some(key.fingerprint.as_str())
    {
        return Err(Error::new("ledger authentication identity failed"));
    }
    let claimed_digest = exact_hex(object, "record_sha256")?;
    let claimed_hmac = exact_hex(object, "record_hmac_sha256")?;

    let mut without_digest = record.clone();
    without_digest
        .as_object_mut()
        .ok_or_else(|| Error::new("ledger record is not an object"))?
        .remove("record_sha256");
    if sha256(&canonical_json(&without_digest)?) != claimed_digest {
        return Err(Error::new("ledger record digest failed"));
    }
    without_digest
        .as_object_mut()
        .ok_or_else(|| Error::new("ledger record is not an object"))?
        .remove("record_hmac_sha256");
    verify_authentication(key, &canonical_json(&without_digest)?, &claimed_hmac)?;
    Ok(claimed_digest)
}

fn authenticate(key: &LedgerKey, payload: &[u8]) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(&key.bytes)
        .map_err(|_| Error::new("ledger authentication key is invalid"))?;
    mac.update(payload);
    let mut encoded = String::with_capacity(64);
    for byte in mac.finalize().into_bytes() {
        write!(&mut encoded, "{byte:02x}")
            .map_err(|_| Error::new("ledger authentication encoding failed"))?;
    }
    Ok(encoded)
}

fn verify_authentication(key: &LedgerKey, payload: &[u8], claimed: &str) -> Result<()> {
    let claimed = decode_hex_32(claimed)?;
    let mut mac = HmacSha256::new_from_slice(&key.bytes)
        .map_err(|_| Error::new("ledger authentication key is invalid"))?;
    mac.update(payload);
    mac.verify_slice(&claimed)
        .map_err(|_| Error::new("ledger record authentication failed"))
}

fn exact_hex(object: &serde_json::Map<String, Value>, field: &str) -> Result<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("ledger field is invalid: {field}")))
}

fn decode_hex_32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(Error::new("ledger HMAC length is invalid"));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or_else(|| Error::new("ledger HMAC is not hex"))?;
        let low = hex_nibble(pair[1]).ok_or_else(|| Error::new("ledger HMAC is not hex"))?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => byte.checked_sub(b'0'),
        b'a'..=b'f' => byte
            .checked_sub(b'a')
            .and_then(|value| value.checked_add(10)),
        _ => None,
    }
}

fn validate_domain(domain: &str) -> Result<()> {
    if domain.is_empty()
        || domain.len() > MAX_DOMAIN_BYTES
        || !domain
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
    {
        return Err(Error::new("ledger authentication domain is invalid"));
    }
    Ok(())
}

fn validate_key_metadata(metadata: &fs::Metadata, require_root: bool) -> Result<()> {
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o777 != 0o400
        || metadata.len() != u64::try_from(KEY_BYTES).unwrap_or(u64::MAX)
    {
        return Err(Error::new(
            "ledger authentication key ownership, mode, or length failed",
        ));
    }
    Ok(())
}

fn file_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use serde_json::json;

    use super::{LedgerKey, seal_record, verify_record};

    #[test]
    fn sealed_record_rejects_tamper_wrong_key_and_cross_domain_replay() {
        let key = LedgerKey::for_test(7);
        let other = LedgerKey::for_test(8);
        let mut record = json!({
            "schema": "astrid.test.record.v1",
            "previous_record_sha256": null,
            "phase": "activated"
        });
        let digest = seal_record(&mut record, &key, "transition").unwrap();
        assert_eq!(verify_record(&record, &key, "transition").unwrap(), digest);
        assert!(verify_record(&record, &other, "transition").is_err());
        assert!(verify_record(&record, &key, "probation").is_err());

        record["phase"] = json!("forged");
        assert!(verify_record(&record, &key, "transition").is_err());
    }

    #[test]
    fn load_requires_exact_owner_only_regular_key_identity() {
        let temp = tempfile::tempdir().unwrap();
        let key_path = temp.path().join("ledger.key");
        fs::write(&key_path, [9_u8; 32]).unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o400)).unwrap();
        let key = LedgerKey::load(&key_path, false).unwrap();
        assert_eq!(key.fingerprint().len(), 64);

        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(LedgerKey::load(&key_path, false).is_err());
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o400)).unwrap();
        let link = temp.path().join("link.key");
        symlink(&key_path, &link).unwrap();
        assert!(LedgerKey::load(&link, false).is_err());
    }

    #[test]
    fn reserved_fields_and_malformed_domains_fail_closed() {
        let key = LedgerKey::for_test(1);
        let mut reserved = json!({"record_sha256": "a".repeat(64)});
        assert!(seal_record(&mut reserved, &key, "transition").is_err());
        let mut record = json!({"schema": "test"});
        assert!(seal_record(&mut record, &key, "Transition/../../peer").is_err());
    }
}
