use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{Error, Result};

pub const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
const MAX_APPEND_LEDGER_BYTES: u64 = 256 * 1024 * 1024;

#[must_use]
pub fn sha256(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)?;
    let normalized = canonical_value(value)?;
    Ok(serde_json::to_vec(&normalized)?)
}

fn canonical_value(value: Value) -> Result<Value> {
    match value {
        Value::Object(values) => {
            let mut ordered = BTreeMap::new();
            for (key, value) in values {
                ordered.insert(key, canonical_value(value)?);
            }
            Ok(serde_json::to_value(ordered)?)
        },
        Value::Array(values) => values
            .into_iter()
            .map(canonical_value)
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Value::Number(number) if number.is_f64() => Err(Error::new(
            "floating-point values are not accepted in authenticated records",
        )),
        other => Ok(other),
    }
}

pub fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || !value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b'-' => index > 0,
            _ => false,
        })
    {
        return Err(Error::new(format!("invalid {label}")));
    }
    Ok(())
}

pub fn validate_hex64(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(Error::new(format!("invalid {label}")));
    }
    Ok(())
}

pub fn validate_relative(value: &str, allow_hidden: bool) -> Result<PathBuf> {
    if value.is_empty() || value.len() > 512 || value.contains(['\\', '\0']) {
        return Err(Error::new("invalid relative path"));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(Error::new("absolute path rejected"));
    }
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(Error::new("non-canonical path rejected"));
        };
        let text = name
            .to_str()
            .ok_or_else(|| Error::new("non-UTF-8 path rejected"))?;
        if text.is_empty() || (!allow_hidden && text.starts_with('.')) {
            return Err(Error::new("hidden or empty path component rejected"));
        }
    }
    if path.to_string_lossy() != value {
        return Err(Error::new("non-canonical relative path rejected"));
    }
    Ok(path.to_path_buf())
}

pub fn require_absolute_no_symlink(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::new(format!("{label} traverses a symlink")));
            },
            Ok(_) | Err(_) => {},
        }
    }
    Ok(())
}

pub fn read_stable_regular(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.file_type().is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() > maximum
    {
        return Err(Error::new(format!(
            "input is not a bounded non-linked regular file: {}",
            path.display()
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    options.custom_flags(libc_o_nofollow());
    let file = options.open(path)?;
    let opened = file.metadata()?;
    let capacity = usize::try_from(before.len()).map_err(|_| Error::new("input too large"))?;
    let mut data = Vec::with_capacity(capacity);
    (&file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut data)?;
    let after = file.metadata()?;
    let identity = |value: &fs::Metadata| {
        (
            value.dev(),
            value.ino(),
            value.len(),
            value.mtime(),
            value.mtime_nsec(),
        )
    };
    if data.len() as u64 != before.len()
        || identity(&before) != identity(&opened)
        || identity(&opened) != identity(&after)
    {
        return Err(Error::new("input changed during read"));
    }
    Ok(data)
}

#[cfg(target_os = "linux")]
const fn libc_o_nofollow() -> i32 {
    0o00_400_000
}

#[cfg(not(target_os = "linux"))]
const fn libc_o_nofollow() -> i32 {
    0
}

pub fn ensure_private_dir(path: &Path) -> Result<()> {
    require_absolute_no_symlink(path, "private directory")?;
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new("private path is not a directory"));
    }
    Ok(())
}

pub fn atomic_private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("output has no parent"))?;
    ensure_private_dir(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn append_private(path: &Path, bytes: &[u8]) -> Result<()> {
    append_private_with_hook(path, bytes, || {})
}

fn private_append_metadata_is_valid(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.nlink() == 1
        && metadata.uid() == nix::unistd::geteuid().as_raw()
        && metadata.permissions().mode().trailing_zeros() >= 6
        && metadata.len() <= MAX_APPEND_LEDGER_BYTES
}

fn append_private_with_hook(
    path: &Path,
    bytes: &[u8],
    before_final_identity_check: impl FnOnce(),
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("ledger has no parent"))?;
    ensure_private_dir(parent)?;
    let before = fs::symlink_metadata(path).ok();
    if before
        .as_ref()
        .is_some_and(|metadata| !private_append_metadata_is_valid(metadata))
    {
        return Err(Error::new(
            "ledger is not an owner-only, single-linked, bounded regular file",
        ));
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    file.lock_exclusive()?;
    let opened = file.metadata()?;
    let identity = |metadata: &fs::Metadata| (metadata.dev(), metadata.ino());
    let version = |metadata: &fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
        )
    };
    if !private_append_metadata_is_valid(&opened)
        || before
            .as_ref()
            .is_some_and(|metadata| identity(metadata) != identity(&opened))
        || fs::symlink_metadata(path)
            .map(|metadata| identity(&metadata) != identity(&opened))
            .unwrap_or(true)
    {
        return Err(Error::new(
            "ledger identity changed before its append lock was secured",
        ));
    }
    let byte_count =
        u64::try_from(bytes.len()).map_err(|_| Error::new("ledger record too large"))?;
    if opened.len().saturating_add(byte_count) > MAX_APPEND_LEDGER_BYTES {
        return Err(Error::new("ledger append exceeds its bounded byte ceiling"));
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    before_final_identity_check();
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    let final_metadata = file.metadata()?;
    if !private_append_metadata_is_valid(&after)
        || !private_append_metadata_is_valid(&path_after)
        || !private_append_metadata_is_valid(&final_metadata)
        || identity(&opened) != identity(&after)
        || version(&after) != version(&final_metadata)
        || version(&path_after) != version(&final_metadata)
        || final_metadata.len() != opened.len().saturating_add(byte_count)
    {
        return Err(Error::new("ledger changed or was replaced during append"));
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[must_use]
pub fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[must_use]
pub fn bounded_text(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::{
        append_private, append_private_with_hook, read_stable_regular, validate_hex64,
        validate_identifier, validate_relative,
    };

    #[test]
    fn identifiers_reject_replay_shaped_and_path_values() {
        assert!(validate_identifier("trace-a1", "trace").is_ok());
        assert!(validate_identifier("../trace", "trace").is_err());
        assert!(validate_identifier("", "trace").is_err());
        assert!(validate_identifier("-leading", "trace").is_err());
    }

    #[test]
    fn paths_reject_traversal_and_hidden_components() {
        assert!(validate_relative("services/edge/src/lib.rs", false).is_ok());
        assert!(validate_relative("../secret", false).is_err());
        assert!(validate_relative("services/.secret", false).is_err());
    }

    #[test]
    fn hashes_are_lowercase_exact() {
        assert!(validate_hex64(&"a".repeat(64), "hash").is_ok());
        assert!(validate_hex64(&"A".repeat(64), "hash").is_err());
    }

    #[test]
    fn linked_credential_shapes_are_rejected_by_stable_reader() {
        let temporary = tempfile::tempdir().unwrap();
        let original = temporary.path().join("original");
        let hardlink = temporary.path().join("hardlink");
        let symlink_path = temporary.path().join("symlink");
        fs::write(&original, [b'k'; 32]).unwrap();
        fs::hard_link(&original, &hardlink).unwrap();
        symlink(&original, &symlink_path).unwrap();
        assert!(read_stable_regular(&original, 32).is_err());
        assert!(read_stable_regular(&hardlink, 32).is_err());
        assert!(read_stable_regular(&symlink_path, 32).is_err());
    }

    #[test]
    fn private_append_rejects_links_and_serializes_concurrent_records() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let outside = temporary.path().join("outside");
        let symlink_path = temporary.path().join("symlink-ledger");
        fs::write(&outside, b"").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&outside, &symlink_path).unwrap();
        assert!(append_private(&symlink_path, b"no\n").is_err());
        fs::remove_file(&symlink_path).unwrap();
        let hardlink_path = temporary.path().join("hardlink-ledger");
        fs::hard_link(&outside, &hardlink_path).unwrap();
        assert!(append_private(&hardlink_path, b"no\n").is_err());

        let ledger = std::sync::Arc::new(temporary.path().join("concurrent.jsonl"));
        let threads = (0..16_u8)
            .map(|index| {
                let ledger = std::sync::Arc::clone(&ledger);
                std::thread::spawn(move || {
                    append_private(&ledger, format!("record-{index:02}\n").as_bytes()).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        let bytes = read_stable_regular(&ledger, 4 * 1024).unwrap();
        let mut lines = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines.len(), 16);
        assert_eq!(lines.first(), Some(&"record-00"));
        assert_eq!(lines.last(), Some(&"record-15"));
    }

    #[test]
    fn private_append_detects_path_replacement_after_locked_write() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let ledger = temporary.path().join("ledger.jsonl");
        append_private(&ledger, b"first\n").unwrap();
        let original = fs::read(&ledger).unwrap();
        let replacement = temporary.path().join("replacement");
        fs::write(&replacement, &original).unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        let outcome = append_private_with_hook(&ledger, b"second\n", || {
            fs::rename(&replacement, &ledger).unwrap();
        });
        assert!(outcome.is_err());
        assert_eq!(fs::read(&ledger).unwrap(), original);
    }

    #[test]
    fn private_append_detects_a_new_hardlink_after_locked_write() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let ledger = temporary.path().join("ledger.jsonl");
        let hardlink = temporary.path().join("late-hardlink.jsonl");
        append_private(&ledger, b"first\n").unwrap();
        let outcome = append_private_with_hook(&ledger, b"second\n", || {
            fs::hard_link(&ledger, &hardlink).unwrap();
        });
        assert!(outcome.is_err());
        assert!(hardlink.exists());
    }
}
