use std::fs::{self, Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(Into::into)
}

pub(crate) fn canonical_sha256<T: Serialize>(value: &T) -> Result<String> {
    Ok(sha256(&canonical_json(value)?))
}

pub(crate) fn canonical_sha256_with_blank_field<T: Serialize>(
    value: &T,
    field: &str,
) -> Result<String> {
    let mut value = serde_json::to_value(value)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::new("hash-bound value is not an object"))?;
    if !object.contains_key(field) {
        return Err(Error::new("hash-bound value omitted its digest field"));
    }
    object.insert(field.to_owned(), Value::String(String::new()));
    Ok(sha256(&canonical_json(&value)?))
}

pub(crate) fn valid_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn identity(metadata: &Metadata) -> (u64, u64, u64, u64, i64, i64, u32, u64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.nlink(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.mode(),
        u64::from(metadata.uid()),
    )
}

pub(crate) fn read_stable_regular(
    path: &Path,
    maximum: usize,
    require_root_owner: bool,
) -> Result<(Vec<u8>, Metadata)> {
    let before = fs::symlink_metadata(path)?;
    validate_regular_identity(&before, maximum, require_root_owner)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC);
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    if identity(&before) != identity(&opened) {
        return Err(Error::new("file changed while being opened"));
    }
    let read_bound = maximum
        .checked_add(1)
        .ok_or_else(|| Error::new("file bound overflow"))?;
    let maximum_u64 = u64::try_from(maximum).unwrap_or(u64::MAX);
    let capacity = usize::try_from(before.len().min(maximum_u64)).unwrap_or(maximum);
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(read_bound as u64)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    if bytes.len() > maximum
        || bytes.len() as u64 != opened.len()
        || identity(&opened) != identity(&after)
        || identity(&after) != identity(&current)
    {
        return Err(Error::new(
            "file changed while being read or exceeded its bound",
        ));
    }
    Ok((bytes, after))
}

fn validate_regular_identity(
    metadata: &Metadata,
    maximum: usize,
    require_root_owner: bool,
) -> Result<()> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() > maximum as u64
        || metadata.mode() & 0o022 != 0
        || (require_root_owner && metadata.uid() != 0)
    {
        return Err(Error::new(
            "file must be bounded, regular, single-link, and immutable to untrusted users",
        ));
    }
    Ok(())
}

pub(crate) fn verify_immutable_ancestors(path: &Path, require_root_owner: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("trusted path must be absolute"));
    }
    let mut current = std::path::PathBuf::from("/");
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("trusted path has no parent"))?;
    for component in parent.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(part) => current.push(part),
            _ => return Err(Error::new("trusted path contains an invalid component")),
        }
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o022 != 0
            || (require_root_owner && metadata.uid() != 0)
        {
            return Err(Error::new("trusted path ancestor is mutable or linked"));
        }
    }
    Ok(())
}

pub(crate) fn read_utf8_line(
    path: &Path,
    maximum: usize,
    require_root_owner: bool,
) -> Result<String> {
    let (bytes, _) = read_stable_regular(path, maximum, require_root_owner)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("identity file is not UTF-8"))?
        .strip_suffix('\n')
        .unwrap_or(std::str::from_utf8(&bytes).unwrap_or_default());
    if value.contains('\n') || value.contains('\r') {
        return Err(Error::new("identity file must contain one line"));
    }
    Ok(value.to_owned())
}

pub(crate) fn sha256_file(path: &Path, maximum: usize, require_root_owner: bool) -> Result<String> {
    let (bytes, _) = read_stable_regular(path, maximum, require_root_owner)?;
    Ok(sha256(&bytes))
}
