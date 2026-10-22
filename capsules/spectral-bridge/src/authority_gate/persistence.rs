//! Durable authority-stream writes join the owner-scoped research transaction.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use anyhow::{Context, Result};
use fs2::FileExt as _;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

pub(super) fn authority_owner_root(path: &Path) -> Result<&Path> {
    anyhow::ensure!(
        path.file_name().and_then(|name| name.to_str()) == Some("authority_gate.jsonl"),
        "not an authority stream"
    );
    let thread = path.parent().context("missing thread")?;
    let threads = thread.parent().context("missing threads")?;
    anyhow::ensure!(
        threads.file_name().and_then(|name| name.to_str()) == Some("threads"),
        "invalid authority owner path"
    );
    threads.parent().context("missing authority owner")
}

pub(super) fn append_jsonl(path: &Path, value: &Value) -> Result<()> {
    let _transaction =
        crate::action_continuity::research_owner_transaction(authority_owner_root(path)?)?;
    crate::action_continuity::read_native_authority_records(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("jsonl.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    #[cfg(unix)]
    {
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))?;
    }
    lock.lock_exclusive()?;

    let prior = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let previous_line = prior
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_string();
    let previous_record_sha256 = if previous_line.is_empty() {
        "0".repeat(64)
    } else {
        format!("{:x}", Sha256::digest(previous_line.as_bytes()))
    };
    let mut record = value.clone();
    if let Some(object) = record.as_object_mut() {
        object.insert(
            "previous_record_sha256".to_string(),
            json!(previous_record_sha256),
        );
    }
    let hash_input = serde_json::to_vec(&record)?;
    if let Some(object) = record.as_object_mut() {
        object.insert(
            "record_sha256".to_string(),
            json!(format!("{:x}", Sha256::digest(hash_input))),
        );
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    writeln!(file, "{}", serde_json::to_string(&record)?)?;
    file.flush()?;
    file.sync_all()?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    fs2::FileExt::unlock(&lock)?;
    Ok(())
}
