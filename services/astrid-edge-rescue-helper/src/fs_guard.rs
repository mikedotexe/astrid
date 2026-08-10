//! Race-aware filesystem and canonical hashing primitives.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

pub const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;

#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn sha256_file(path: &Path, maximum: u64) -> Result<String> {
    Ok(sha256(&read_regular(path, maximum)?))
}

/// Serialize with recursively sorted object keys and no insignificant whitespace.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)?;
    let sorted = sort_value(value);
    serde_json::to_vec(&sorted).map_err(Into::into)
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .into_iter()
                .map(|(key, value)| (key, sort_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        },
        Value::Array(values) => Value::Array(values.into_iter().map(sort_value).collect()),
        other => other,
    }
}

pub fn read_regular(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() || before.nlink() != 1 {
        return Err(Error::new(format!(
            "input must be a regular, unlinked file: {}",
            path.display()
        )));
    }
    if before.len() > maximum {
        return Err(Error::new(format!(
            "input exceeds bound: {}",
            path.display()
        )));
    }
    let mut handle = File::open(path)?;
    let opened = handle.metadata()?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    (&mut handle)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = handle.metadata()?;
    let identity = |value: &fs::Metadata| {
        (
            value.dev(),
            value.ino(),
            value.len(),
            value.mtime(),
            value.mtime_nsec(),
        )
    };
    if bytes.len() as u64 > maximum
        || identity(&before) != identity(&opened)
        || identity(&opened) != identity(&after)
        || bytes.len() as u64 != before.len()
    {
        return Err(Error::new(format!(
            "input changed while being read: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path, maximum: u64) -> Result<T> {
    serde_json::from_slice(&read_regular(path, maximum)?).map_err(Into::into)
}

pub fn validate_relative(value: &str) -> Result<PathBuf> {
    validate_relative_inner(value, false)
}

pub fn validate_relative_signed(value: &str) -> Result<PathBuf> {
    validate_relative_inner(value, true)
}

fn validate_relative_inner(value: &str, allow_signed_metadata: bool) -> Result<PathBuf> {
    if value.is_empty()
        || value.len() > 512
        || value.contains(['\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return Err(Error::new("unsafe relative path"));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            if !matches!(component, Component::Normal(_)) {
                return true;
            }
            let name = component.as_os_str().to_string_lossy();
            name.starts_with('.')
                && !(allow_signed_metadata
                    && matches!(
                        name.as_ref(),
                        ".cargo" | ".cargo-checksum.json" | ".astrid-edge-generation.json"
                    ))
        })
        || path.to_string_lossy() != value
    {
        return Err(Error::new("unsafe or hidden relative path"));
    }
    Ok(path.to_path_buf())
}

pub fn require_absolute(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(Error::new(format!(
            "{label} must be an absolute normalized path"
        )));
    }
    let mut cursor = PathBuf::new();
    for component in path.components() {
        cursor.push(component);
        if cursor.exists() && fs::symlink_metadata(&cursor)?.file_type().is_symlink() {
            return Err(Error::new(format!("{label} traverses a symlink")));
        }
    }
    Ok(())
}

/// Validate the one path in the rescue configuration whose final component is
/// intentionally a symlink. Every parent remains non-symlinked and immutable,
/// and the resolved target must be one direct child of the release store.
pub fn require_active_generation_link(path: &Path, releases: &Path, label: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} has no parent")))?;
    require_absolute(parent, label)?;
    require_absolute(releases, "release root")?;
    let expected_owner = nix::unistd::geteuid().as_raw();
    for (candidate, candidate_label) in [(parent, "active-link parent"), (releases, "release root")]
    {
        let metadata = fs::symlink_metadata(candidate)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != expected_owner
            || metadata.mode() & 0o022 != 0
        {
            return Err(Error::new(format!(
                "{candidate_label} must be owned by the rescue identity and non-writable by group/world"
            )));
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_symlink() || metadata.uid() != expected_owner {
        return Err(Error::new(format!(
            "{label} must be a symlink owned by the rescue identity"
        )));
    }
    let release_root = fs::canonicalize(releases)?;
    let target = fs::canonicalize(path)?;
    let relative = target
        .strip_prefix(&release_root)
        .map_err(|_| Error::new(format!("{label} target escapes the release root")))?;
    if relative.components().count() != 1
        || !matches!(relative.components().next(), Some(Component::Normal(_)))
    {
        return Err(Error::new(format!(
            "{label} target must be one direct release generation"
        )));
    }
    Ok(())
}

pub fn require_private_root_file(path: &Path, label: &str) -> Result<()> {
    require_absolute(path, label)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(format!(
            "{label} must be root-owned and non-writable by group/world"
        )));
    }
    Ok(())
}

pub fn ensure_within(root: &Path, path: &Path, must_exist: bool) -> Result<()> {
    require_absolute(root, "root")?;
    require_absolute(path, "bounded path")?;
    if !path.starts_with(root) {
        return Err(Error::new("path escapes configured root"));
    }
    let mut cursor = root.to_path_buf();
    for component in path
        .strip_prefix(root)
        .map_err(|_| Error::new("path escape"))?
        .components()
    {
        cursor.push(component);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::new("bounded path traverses a symlink"));
            },
            Ok(_) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !must_exist => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8], mode: u32, replace: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("output has no parent"))?;
    fs::create_dir_all(parent)?;
    let basename = path
        .file_name()
        .ok_or_else(|| Error::new("output has no basename"))?
        .to_string_lossy();
    let (temporary, mut handle) = create_atomic_temporary(parent, &basename, mode)?;
    let result = (|| {
        handle.write_all(bytes)?;
        handle.sync_all()?;
        drop(handle);
        if replace {
            fs::rename(&temporary, path)?;
        } else {
            rename_no_replace(parent, &temporary, path)?;
        }
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_atomic_temporary(parent: &Path, basename: &str, mode: u32) -> Result<(PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(mode);
    for _ in 0..8 {
        let suffix = random_temporary_suffix()?;
        let temporary = parent.join(format!(".{basename}.{suffix}.partial"));
        match options.open(&temporary) {
            Ok(handle) => return Ok((temporary, handle)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
            Err(error) => return Err(error.into()),
        }
    }
    Err(Error::new("cannot allocate unique atomic output"))
}

fn random_temporary_suffix() -> Result<String> {
    let mut entropy = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut entropy)?;
    Ok(sha256(&entropy)[..24].to_owned())
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn rename_no_replace(parent: &Path, temporary: &Path, destination: &Path) -> Result<()> {
    use nix::fcntl::{RenameFlags, renameat2};

    let directory = File::open(parent)?;
    let temporary = temporary
        .file_name()
        .ok_or_else(|| Error::new("atomic temporary has no basename"))?;
    let destination = destination
        .file_name()
        .ok_or_else(|| Error::new("atomic destination has no basename"))?;
    renameat2(
        &directory,
        temporary,
        &directory,
        destination,
        RenameFlags::RENAME_NOREPLACE,
    )
    .map_err(|error| Error::new(format!("atomic no-replace rename failed: {error}")))
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn rename_no_replace(_parent: &Path, temporary: &Path, destination: &Path) -> Result<()> {
    // CPU-edge production targets are Linux GNU and use renameat2 above. This
    // branch exists only for local host tests; the immutable Linux replay
    // tests exercise the no-replace syscall contract before packaging.
    if destination.exists() || destination.is_symlink() {
        return Err(Error::new("refusing to replace existing output"));
    }
    fs::rename(temporary, destination).map_err(Into::into)
}

pub fn copy_regular(source: &Path, destination: &Path, maximum: u64, mode: u32) -> Result<String> {
    let bytes = read_regular(source, maximum)?;
    atomic_write(destination, &bytes, mode, false)?;
    Ok(sha256(&bytes))
}

pub fn make_read_only_tree(root: &Path) -> Result<()> {
    fn visit(path: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("release tree contains a symlink"));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                visit(&entry?.path())?;
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o555))?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            let executable = metadata.mode() & 0o111 != 0;
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(if executable { 0o555 } else { 0o444 }),
            )?;
        } else {
            return Err(Error::new("release tree contains a special or linked file"));
        }
        Ok(())
    }
    visit(root)
}

#[cfg(test)]
mod tests {
    use super::{
        atomic_write, canonical_json, read_regular, require_active_generation_link,
        validate_relative,
    };
    use std::fs;

    #[test]
    fn canonical_json_sorts_nested_keys() {
        let value = serde_json::json!({"z": {"b": 1, "a": 2}, "a": 3});
        assert_eq!(
            canonical_json(&value).unwrap(),
            br#"{"a":3,"z":{"a":2,"b":1}}"#
        );
    }

    #[test]
    fn traversal_hidden_and_links_are_rejected() {
        assert!(validate_relative("../x").is_err());
        assert!(validate_relative("safe/.hidden").is_err());
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("real"), b"x").unwrap();
        std::os::unix::fs::symlink(temp.path().join("real"), temp.path().join("link")).unwrap();
        assert!(read_regular(&temp.path().join("link"), 10).is_err());
    }

    #[test]
    fn active_generation_allows_only_the_final_bounded_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let releases = root.join("releases");
        let generation = releases.join("generation-1");
        fs::create_dir_all(&generation).unwrap();
        let active = root.join("current");
        std::os::unix::fs::symlink("releases/generation-1", &active).unwrap();
        require_active_generation_link(&active, &releases, "active").unwrap();

        fs::remove_file(&active).unwrap();
        let nested = generation.join("nested");
        fs::create_dir(&nested).unwrap();
        std::os::unix::fs::symlink("releases/generation-1/nested", &active).unwrap();
        assert!(require_active_generation_link(&active, &releases, "active").is_err());
    }

    #[test]
    fn atomic_write_never_replaces_when_creation_is_required() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("evidence.json");
        fs::write(temp.path().join(".evidence.json.1.partial"), b"orphan").unwrap();
        atomic_write(&output, b"first", 0o600, false).unwrap();
        assert!(atomic_write(&output, b"forged", 0o600, false).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"first");
        atomic_write(&output, b"second", 0o600, true).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"second");
    }
}
