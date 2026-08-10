use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::auth::hmac_fields;
use crate::{Config, Error, Result};

#[derive(Serialize)]
pub struct Receipt<'a> {
    pub schema: &'static str,
    pub appliance_id: &'a str,
    pub recorded_at_unix_ms: u64,
    pub client_id: &'a str,
    pub operation: &'a str,
    pub request_hash: &'a str,
    pub status: &'a str,
    pub http_status: Option<u16>,
    pub response_body_sha256: Option<&'a str>,
    pub response_body_bytes: Option<u64>,
    pub elapsed_ms: u64,
    pub authority: &'static str,
}

#[derive(Serialize)]
struct SignedReceipt<'a> {
    payload: &'a Receipt<'a>,
    signature: String,
}

pub fn append(
    config: &Config,
    client_id: &str,
    credential_directory: &Path,
    receipt: &Receipt<'_>,
) -> Result<()> {
    let payload = serde_json::to_vec(receipt)?;
    let key = config.ledger_key(client_id, credential_directory)?;
    let signature = hmac_fields(&key, b"astrid.edge.provider_broker.receipt.v1", &[&payload]);
    let mut encoded = serde_json::to_vec(&SignedReceipt {
        payload: receipt,
        signature,
    })?;
    encoded.push(b'\n');
    if encoded.len() > 4_096 {
        return Err(Error::new("provider receipt exceeds immutable bound"));
    }
    validate_parent(&config.ledger_path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true).write(true).mode(0o600);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000 | 0o02_000_000);
    let mut file = options.open(&config.ledger_path)?;
    validate_ledger(&file.metadata()?)?;
    fs2::FileExt::lock_exclusive(&file)?;
    file.write_all(&encoded)?;
    file.flush()?;
    file.sync_data()?;
    Ok(())
}

pub fn now_ms() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::new("system clock is before Unix epoch"))?
            .as_millis(),
    )
    .map_err(|_| Error::new("Unix milliseconds do not fit u64"))
}

pub fn body_hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn validate_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("provider ledger has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(Error::new("provider ledger parent is unsafe"));
    }
    Ok(())
}

fn validate_ledger(metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(Error::new("provider ledger identity is unsafe"));
    }
    Ok(())
}
