//! Cross-process exclusion shared by participating checkpoint publishers.
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

pub(super) fn acquire(path: &Path) -> io::Result<File> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing checkpoint parent"))?;
    std::fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("missing checkpoint filename"))?;
    let lock_path = parent.join(format!(".{}.publication-lock", name.to_string_lossy()));
    if let Ok(info) = std::fs::symlink_metadata(&lock_path)
        && (!info.is_file() || info.nlink() != 1 || info.mode() & 0o777 != 0o600 || info.len() != 0)
    {
        return Err(io::Error::other("invalid checkpoint publication lock"));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&lock_path)?;
    fs2::FileExt::try_lock_exclusive(&file)?;
    let info = file.metadata()?;
    let current = std::fs::symlink_metadata(&lock_path)?;
    if !current.is_file()
        || current.nlink() != 1
        || current.mode() & 0o777 != 0o600
        || current.len() != 0
        || (info.dev(), info.ino(), info.uid()) != (current.dev(), current.ino(), current.uid())
    {
        return Err(io::Error::other("checkpoint publication lock changed"));
    }
    Ok(file)
}
