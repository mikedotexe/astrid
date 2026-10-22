//! Descriptor-relative access rejects symlinks in every existing component.

use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path};

use rustix::fs::{Mode, OFlags, mkdirat, open, openat};
use serde::{Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

use crate::Result;

const MAX_BYTES: u64 = 65_536;
const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(crate) fn absolute(path: &Path) -> Result<()> {
    let mut normalized = std::path::PathBuf::new();
    if !path.is_absolute() {
        return Err("absolute_path_required");
    }
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err("non_normal_path");
        }
        normalized.push(component);
    }
    if normalized.as_os_str() != path.as_os_str() {
        return Err("non_normal_path");
    }
    Ok(())
}

fn walk(path: &Path) -> Result<File> {
    absolute(path)?;
    let mut fd = open("/", DIRECTORY, Mode::empty()).map_err(|_| "unsafe_directory")?;
    for component in path.components() {
        if let Component::Normal(name) = component {
            fd = openat(&fd, name, DIRECTORY, Mode::empty()).map_err(|_| "unsafe_directory")?;
        }
    }
    Ok(File::from(fd))
}

fn private(file: &File, directory: bool) -> Result<()> {
    let meta = file.metadata().map_err(|_| "unsafe_metadata")?;
    let expected = if directory { 0o700 } else { 0o600 };
    if meta.uid() != rustix::process::geteuid().as_raw()
        || meta.mode() & 0o7777 != expected
        || (directory && !meta.is_dir())
        || (!directory && (!meta.is_file() || meta.nlink() != 1))
    {
        return Err("private_owner_mode_required");
    }
    Ok(())
}

pub(crate) struct Directory(File);

impl Directory {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let file = walk(path)?;
        private(&file, true)?;
        Ok(Self(file))
    }

    pub(crate) fn create(path: &Path) -> Result<Self> {
        absolute(path)?;
        let parent = Self::open(path.parent().ok_or("invalid_directory")?)?;
        let name = path.file_name().ok_or("invalid_directory")?;
        mkdirat(&parent.0, name, Mode::RWXU).map_err(|_| "directory_exists_or_unavailable")?;
        let file = File::from(
            openat(&parent.0, name, DIRECTORY, Mode::empty()).map_err(|_| "unsafe_directory")?,
        );
        private(&file, true)?;
        parent.0.sync_all().map_err(|_| "sync_failed")?;
        Ok(Self(file))
    }

    pub(crate) fn write_new(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
            return Err("invalid_artifact_name");
        }
        let mut file = File::from(
            openat(
                &self.0,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| "artifact_exists_or_unavailable")?,
        );
        private(&file, false)?;
        file.write_all(bytes).map_err(|_| "write_failed")?;
        file.sync_all().map_err(|_| "sync_failed")?;
        self.0.sync_all().map_err(|_| "sync_failed")
    }

    pub(crate) fn write_json(&self, name: &str, value: &impl Serialize) -> Result<()> {
        self.write_new(
            name,
            &serde_json::to_vec(value).map_err(|_| "serialization_failed")?,
        )
    }
}

pub(crate) fn read(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    absolute(path)?;
    let parent = Directory::open(path.parent().ok_or("invalid_file")?)?;
    let file = File::from(
        openat(
            &parent.0,
            path.file_name().ok_or("invalid_file")?,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| "input_unavailable")?,
    );
    private(&file, false)?;
    if file.metadata().map_err(|_| "unsafe_metadata")?.len() > MAX_BYTES {
        return Err("input_too_large");
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "read_failed")?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        return Err("input_too_large");
    }
    Ok(bytes)
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&read(path)?).map_err(|_| "invalid_operator_json")
}

/// Only inspect the planned installation root, never create it.
pub(crate) fn require_absent(path: &Path) -> Result<()> {
    absolute(path)?;
    let parent = walk(path.parent().ok_or("invalid_install_root")?)?;
    match rustix::fs::statat(
        &parent,
        path.file_name().ok_or("invalid_install_root")?,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        _ => Err("install_root_exists_or_unavailable"),
    }
}
