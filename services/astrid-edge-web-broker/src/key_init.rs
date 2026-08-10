//! Installer-only creation and deterministic verification of the response keypair.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use rand_core::{OsRng, RngCore as _};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInitialization {
    pub schema: &'static str,
    pub signing_seed_sha256: String,
    pub verify_key_sha256: String,
    pub created_signing_seed: bool,
    pub created_verify_key: bool,
}

/// Create or verify an appliance-local Ed25519 response keypair without replacement.
///
/// This installer-only operation creates an exact 32-byte mode-0600 signing seed and
/// its exact 32-byte mode-0644 public key in one root-controlled directory. Existing
/// files are accepted only when their identity, ownership, mode, and derived binding
/// are exact; they are never replaced.
///
/// # Errors
///
/// Returns an error for non-absolute paths, different or writable parent directories,
/// links, hardlinks, unsafe modes/owners, malformed existing files, entropy failure,
/// or a public key that does not derive from the seed.
pub fn initialize_response_keypair(
    signing_seed_path: &Path,
    verify_key_path: &Path,
) -> Result<KeyInitialization> {
    let effective_uid = current_effective_uid()?;
    let parent = validate_output_paths(signing_seed_path, verify_key_path, effective_uid)?;
    let (seed, created_signing_seed) = if let Some(seed) = read_existing_exact(
        signing_seed_path,
        effective_uid,
        0o600,
        "response signing seed",
    )? {
        (seed, false)
    } else {
        let mut generated = [0_u8; 32];
        OsRng.fill_bytes(&mut generated);
        let created = atomic_create_no_replace(signing_seed_path, &generated, 0o600)?;
        let seed = read_existing_exact(
            signing_seed_path,
            effective_uid,
            0o600,
            "response signing seed",
        )?
        .ok_or_else(|| Error::new("response signing seed creation was not durable"))?;
        (seed, created)
    };
    let signing = SigningKey::from_bytes(&seed);
    let expected_verify = signing.verifying_key().to_bytes();
    let created_verify_key = if let Some(existing) =
        read_existing_exact(verify_key_path, effective_uid, 0o644, "response verify key")?
    {
        if existing != expected_verify {
            return Err(Error::new(
                "existing response verify key does not derive from signing seed",
            ));
        }
        false
    } else {
        let created = atomic_create_no_replace(verify_key_path, &expected_verify, 0o644)?;
        let existing =
            read_existing_exact(verify_key_path, effective_uid, 0o644, "response verify key")?
                .ok_or_else(|| Error::new("response verify key creation was not durable"))?;
        if existing != expected_verify {
            return Err(Error::new(
                "response verify key creation raced with a different identity",
            ));
        }
        created
    };
    File::open(parent)?.sync_all()?;
    Ok(KeyInitialization {
        schema: "astrid.edge.web_broker.key_initialization.v1",
        signing_seed_sha256: format!("{:x}", Sha256::digest(seed)),
        verify_key_sha256: format!("{:x}", Sha256::digest(expected_verify)),
        created_signing_seed,
        created_verify_key,
    })
}

fn validate_output_paths(seed: &Path, public: &Path, effective_uid: u32) -> Result<PathBuf> {
    if !seed.is_absolute() || !public.is_absolute() || seed == public {
        return Err(Error::new("key-init paths must be distinct absolute paths"));
    }
    let seed_parent = seed
        .parent()
        .ok_or_else(|| Error::new("signing seed path has no parent"))?;
    let public_parent = public
        .parent()
        .ok_or_else(|| Error::new("verify key path has no parent"))?;
    if seed_parent != public_parent {
        return Err(Error::new(
            "signing seed and verify key must share one directory",
        ));
    }
    reject_symlink_components(seed_parent)?;
    let metadata = fs::symlink_metadata(seed_parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != effective_uid
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(Error::new(
            "key-init directory must be service-owner controlled and non-writable by group/other",
        ));
    }
    for path in [seed, public] {
        if let Ok(metadata) = fs::symlink_metadata(path)
            && metadata.file_type().is_symlink()
        {
            return Err(Error::new("key-init output path is a symlink"));
        }
    }
    Ok(seed_parent.to_path_buf())
}

fn read_existing_exact(
    path: &Path,
    effective_uid: u32,
    expected_mode: u32,
    label: &str,
) -> Result<Option<[u8; 32]>> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate_key_metadata(&before, effective_uid, expected_mode, label)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    let opened = file.metadata()?;
    validate_key_metadata(&opened, effective_uid, expected_mode, label)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(33)
        .read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    validate_key_metadata(&after, effective_uid, expected_mode, label)?;
    if metadata_identity(&before) != metadata_identity(&opened)
        || metadata_identity(&opened) != metadata_identity(&after)
    {
        return Err(Error::new(format!("{label} changed while reading")));
    }
    Ok(Some(bytes.try_into().map_err(|_| {
        Error::new(format!("{label} is not exactly 32 bytes"))
    })?))
}

fn atomic_create_no_replace(path: &Path, bytes: &[u8; 32], mode: u32) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("key-init path has no parent"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| Error::new("key-init path has no filename"))?
        .to_string_lossy();
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        unix_nanos()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != mode
        || metadata.len() != 32
    {
        let _ = fs::remove_file(&temporary);
        return Err(Error::new("temporary key-init file failed identity checks"));
    }
    drop(file);
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)?;
            File::open(parent)?.sync_all()?;
            Ok(true)
        },
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temporary)?;
            Ok(false)
        },
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error.into())
        },
    }
}

fn validate_key_metadata(
    metadata: &fs::Metadata,
    effective_uid: u32,
    expected_mode: u32,
    label: &str,
) -> Result<()> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != effective_uid
        || metadata.permissions().mode() & 0o7777 != expected_mode
        || metadata.len() != 32
    {
        return Err(Error::new(format!(
            "{label} must be owner-controlled, regular, nlink-one, mode {expected_mode:04o}, and exactly 32 bytes"
        )));
    }
    Ok(())
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn reject_symlink_components(path: &Path) -> Result<()> {
    let mut cursor = PathBuf::new();
    for component in path.components() {
        cursor.push(component.as_os_str());
        if cursor == Path::new("/") {
            continue;
        }
        let metadata = fs::symlink_metadata(&cursor)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new("key-init path traverses a symlink"));
        }
    }
    Ok(())
}

fn current_effective_uid() -> Result<u32> {
    let status = fs::read_to_string("/proc/self/status").or_else(|_| {
        Ok::<_, std::io::Error>(format!("Uid:\t0\t{}\t0\t0\n", fs::metadata(".")?.uid()))
    })?;
    let line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| Error::new("cannot determine effective UID"))?;
    line.split_whitespace()
        .nth(2)
        .ok_or_else(|| Error::new("effective UID is absent"))?
        .parse::<u32>()
        .map_err(|_| Error::new("effective UID is malformed"))
}

fn unix_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    use super::initialize_response_keypair;

    #[test]
    fn init_is_idempotent_and_never_replaces_or_accepts_links() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let seed = root.join("response-signing.key");
        let public = root.join("response.pub");
        let first = initialize_response_keypair(&seed, &public).unwrap();
        assert!(first.created_signing_seed);
        assert!(first.created_verify_key);
        assert_eq!(
            fs::metadata(&seed).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        assert_eq!(
            fs::metadata(&public).unwrap().permissions().mode() & 0o7777,
            0o644
        );
        let second = initialize_response_keypair(&seed, &public).unwrap();
        assert!(!second.created_signing_seed);
        assert!(!second.created_verify_key);
        assert_eq!(first.signing_seed_sha256, second.signing_seed_sha256);
        assert_eq!(first.verify_key_sha256, second.verify_key_sha256);

        let linked_seed = root.join("linked-seed");
        let linked_public = root.join("linked-public");
        symlink(&seed, &linked_seed).unwrap();
        assert!(initialize_response_keypair(&linked_seed, &linked_public).is_err());
    }

    #[test]
    fn mismatched_existing_public_key_is_never_replaced() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let seed = root.join("response-signing.key");
        let public = root.join("response.pub");
        fs::write(&seed, [0x41; 32]).unwrap();
        fs::write(&public, [0x42; 32]).unwrap();
        fs::set_permissions(&seed, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&public, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(initialize_response_keypair(&seed, &public).is_err());
        assert_eq!(fs::read(&public).unwrap(), [0x42; 32]);
    }
}
