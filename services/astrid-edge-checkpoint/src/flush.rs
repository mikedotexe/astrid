//! Bounded fsync traversal for state that must cross an A/B restart boundary.

use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use astrid_edge_rescue_helper::fs_guard::{atomic_write, canonical_json, sha256};

use crate::{AUTHORITY, Error, Result, state_root, unix_millis, valid_identifier};

const MAX_FILES: usize = 100_000;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;

pub fn flush(workspace: &Path, output: &Path, generation_id: &str) -> Result<()> {
    if !valid_identifier(generation_id) {
        return Err(Error::new("flush generation identity is invalid"));
    }
    require_private_output_parent(output)?;
    let workspace = workspace.canonicalize()?;
    let root = state_root(&workspace)?;
    let roots = [
        workspace,
        root.join("var/state.db"),
        root.join("home/default/.local/audit"),
    ];
    let mut files = Vec::new();
    let mut directories = Vec::new();
    for root in &roots {
        walk(root, &mut files, &mut directories)?;
    }
    files.sort();
    files.dedup();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    directories.dedup();
    if files.len() > MAX_FILES {
        return Err(Error::new("flush file inventory exceeds immutable bound"));
    }
    let mut total = 0_u64;
    for path in &files {
        let before = fs::symlink_metadata(path)?;
        total = total.saturating_add(before.len());
        if total > MAX_TOTAL_BYTES {
            return Err(Error::new("flush byte inventory exceeds immutable bound"));
        }
        let handle = File::open(path)?;
        let opened = handle.metadata()?;
        if identity(&before) != identity(&opened) {
            return Err(Error::new("state file changed identity before flush"));
        }
        handle.sync_all()?;
    }
    for path in &directories {
        File::open(path)?.sync_all()?;
    }
    let mut receipt = serde_json::json!({
        "schema": "astrid.edge_checkpoint.flush.v1",
        "recorded_at_unix_ms": unix_millis(),
        "generation_id": generation_id,
        "files_flushed": files.len(),
        "directories_flushed": directories.len(),
        "bytes_covered": total,
        "authority": AUTHORITY,
    });
    let digest = sha256(&canonical_json(&receipt)?);
    receipt["receipt_sha256"] = serde_json::Value::String(digest);
    atomic_write(output, &canonical_json(&receipt)?, 0o400, true)
}

fn walk(root: &Path, files: &mut Vec<PathBuf>, directories: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::new("flush root or entry is a symlink"));
    }
    if metadata.is_file() {
        if metadata.nlink() != 1 {
            return Err(Error::new("flush state file is hard-linked"));
        }
        files.push(root.to_path_buf());
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(Error::new("flush state contains a special file"));
    }
    directories.push(root.to_path_buf());
    for entry in fs::read_dir(root)? {
        walk(&entry?.path(), files, directories)?;
        if files.len() > MAX_FILES {
            return Err(Error::new("flush file inventory exceeds immutable bound"));
        }
    }
    Ok(())
}

fn require_private_output_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("flush receipt has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("flush receipt parent is not private root state"));
    }
    Ok(())
}

fn identity(metadata: &fs::Metadata) -> (u64, u64, u64) {
    (metadata.dev(), metadata.ino(), metadata.len())
}
