use std::fs::{File, OpenOptions};
use std::io::Read as _;
use std::path::Path;

use fs2::FileExt as _;

use crate::{PublicWebErrorCode as E, Result};

pub(crate) const MAX_AUTHORITY_BYTES: u64 = 65_536;

#[cfg(unix)]
#[expect(
    clippy::verbose_bit_mask,
    reason = "octal Unix permission bits are the reviewed access boundary"
)]
fn private_metadata(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.uid() == nix::unistd::geteuid().as_raw() && metadata.mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn private_metadata(_: &std::fs::Metadata) -> bool {
    false
}

pub(crate) fn private_directory(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(E::UnsafeFilesystem);
    }
    // Reject symlinks in all components, not only the final directory.
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(E::UnsafeFilesystem);
        }
        current.push(component);
        let meta = std::fs::symlink_metadata(&current).map_err(|_| E::UnsafeFilesystem)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(E::UnsafeFilesystem);
        }
    }
    let meta = std::fs::metadata(path).map_err(|_| E::UnsafeFilesystem)?;
    if !private_metadata(&meta) {
        return Err(E::UnsafeFilesystem);
    }
    Ok(())
}

fn no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
        options.mode(0o600);
    }
}

pub(crate) fn read_private(path: &Path) -> Result<Vec<u8>> {
    private_directory(path.parent().ok_or(E::UnsafeFilesystem)?)?;
    let mut options = OpenOptions::new();
    options.read(true);
    no_follow(&mut options);
    let file = options.open(path).map_err(|_| E::AuthorityUnavailable)?;
    let meta = file.metadata().map_err(|_| E::UnsafeFilesystem)?;
    if !meta.is_file() || !private_metadata(&meta) || meta.len() > MAX_AUTHORITY_BYTES {
        return Err(E::UnsafeFilesystem);
    }
    let mut bytes = Vec::new();
    file.take(MAX_AUTHORITY_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| E::AuthorityUnavailable)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AUTHORITY_BYTES {
        return Err(E::UnsafeFilesystem);
    }
    Ok(bytes)
}

/// Never unlink a lock file: unlinking permits two independently locked inodes.
pub(crate) struct OwnerLease {
    _file: File,
}

impl OwnerLease {
    pub(crate) fn acquire(directory: &Path, owner_sha256: &str) -> Result<Self> {
        private_directory(directory)?;
        if owner_sha256.len() != 64 || !owner_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(E::InvalidConfig);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        no_follow(&mut options);
        let file = options
            .open(directory.join(format!("{owner_sha256}.lock")))
            .map_err(|_| E::UnsafeFilesystem)?;
        let meta = file.metadata().map_err(|_| E::UnsafeFilesystem)?;
        if !meta.is_file() || !private_metadata(&meta) {
            return Err(E::UnsafeFilesystem);
        }
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                E::OwnerBusy
            } else {
                E::UnsafeFilesystem
            }
        })?;
        Ok(Self { _file: file })
    }
}

pub(crate) fn headroom(path: &Path, reserve: u64, body_limit: u64) -> Result<()> {
    private_directory(path)?;
    let required = reserve.checked_add(body_limit).ok_or(E::StorageHeadroom)?;
    if fs2::available_space(path).map_err(|_| E::StorageHeadroom)? < required {
        return Err(E::StorageHeadroom);
    }
    Ok(())
}
