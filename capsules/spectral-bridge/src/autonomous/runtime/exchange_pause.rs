//! Scope-owned perception pause, released on early return or future cancellation.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(super) struct ExchangePause {
    path: PathBuf,
    marker: Vec<u8>,
    device: u64,
    inode: u64,
}

impl ExchangePause {
    pub(super) fn acquire(path: &Path) -> std::io::Result<Option<Self>> {
        Self::acquire_with_writer(path, |file, marker| file.write_all(marker))
    }

    fn acquire_with_writer(
        path: &Path,
        write_marker: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
    ) -> std::io::Result<Option<Self>> {
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("pause path has no parent"))?;
        let nonce = rand::random::<u128>();
        let temporary = parent.join(format!(".exchange-pause-{nonce:032x}.pending"));
        let marker = format!("exchange-owned:{nonce:032x}").into_bytes();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        let metadata = file.metadata()?;
        // Build the marker away from the observed pause pathname. A partial write
        // cannot publish a pause that no successfully constructed guard owns.
        let temporary_guard = TemporaryPause {
            path: temporary.clone(),
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        write_marker(&mut file, &marker)?;
        match fs::hard_link(&temporary, path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(None),
            Err(error) => return Err(error),
        }
        drop(temporary_guard);
        Ok(Some(Self {
            path: path.to_path_buf(),
            marker,
            device: metadata.dev(),
            inode: metadata.ino(),
        }))
    }
}

struct TemporaryPause {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Drop for TemporaryPause {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|metadata| {
            metadata.is_file() && metadata.dev() == self.device && metadata.ino() == self.inode
        }) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Drop for ExchangePause {
    fn drop(&mut self) {
        // A subsequent explicit pause replaces the marker and keeps ownership.
        if fs::symlink_metadata(&self.path).is_ok_and(|metadata| {
            metadata.is_file() && metadata.dev() == self.device && metadata.ino() == self.inode
        }) && fs::read(&self.path).is_ok_and(|bytes| bytes == self.marker)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_existing_and_replacement_pauses() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paused");
        fs::write(&path, "external").unwrap();
        assert!(ExchangePause::acquire(&path).unwrap().is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
        fs::remove_file(&path).unwrap();
        let guard = ExchangePause::acquire(&path).unwrap().unwrap();
        fs::write(&path, "explicit action pause").unwrap();
        drop(guard);
        assert_eq!(fs::read_to_string(&path).unwrap(), "explicit action pause");
    }

    #[test]
    fn partial_marker_write_failure_never_publishes_a_pause() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paused");
        let result = ExchangePause::acquire_with_writer(&path, |file, marker| {
            file.write_all(&marker[..5])?;
            Err(std::io::Error::other("synthetic write failure"))
        });
        assert!(result.is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        assert!(ExchangePause::acquire(&path).unwrap().is_some());
    }

    #[test]
    fn concurrent_explicit_pause_wins_before_owned_marker_publication() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paused");
        let result = ExchangePause::acquire_with_writer(&path, |file, marker| {
            file.write_all(marker)?;
            fs::write(&path, "explicit external pause")
        })
        .unwrap();
        assert!(result.is_none());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "explicit external pause"
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn cancellation_releases_only_owned_pause() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("paused");
        let guard = ExchangePause::acquire(&path).unwrap().unwrap();
        let task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        task.abort();
        let _ = task.await;
        assert!(!path.exists());
    }
}
