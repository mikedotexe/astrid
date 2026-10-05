//! A bounded, frozen publication with receipt-only recovery after rename.
use std::fs::{File, Metadata};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const LIMIT: u64 = 64 * 1024 * 1024;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn identity(a: &Metadata, b: &Metadata) -> bool {
    (a.dev(), a.ino(), a.mode(), a.uid(), a.nlink())
        == (b.dev(), b.ino(), b.mode(), b.uid(), b.nlink())
}

fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    identity(a, b)
        && (
            a.len(),
            a.mtime(),
            a.mtime_nsec(),
            a.ctime(),
            a.ctime_nsec(),
        ) == (
            b.len(),
            b.mtime(),
            b.mtime_nsec(),
            b.ctime(),
            b.ctime_nsec(),
        )
}

fn read(path: &Path) -> io::Result<Option<(Metadata, Vec<u8>)>> {
    let before = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !before.is_file() || before.nlink() != 1 || before.len() > LIMIT {
        return Err(invalid(
            "checkpoint is not a bounded single-link regular file",
        ));
    }
    let mut file = File::open(path)?;
    if !unchanged(&before, &file.metadata()?) {
        return Err(invalid("checkpoint changed while opening"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(LIMIT.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LIMIT
        || !unchanged(&before, &file.metadata()?)
        || !unchanged(&before, &std::fs::symlink_metadata(path)?)
    {
        return Err(invalid("checkpoint changed while reading"));
    }
    Ok(Some((before, bytes)))
}

pub struct FrozenWrite {
    path: PathBuf,
    parent: PathBuf,
    parent_identity: Metadata,
    previous: Option<(Metadata, Vec<u8>)>,
    bytes: Vec<u8>,
    published: Option<Metadata>,
    temporary: Option<(PathBuf, File)>,
}

impl FrozenWrite {
    /// Bind an already-published file for read-only durability revalidation.
    pub fn retain_existing(path: &Path, expected: &[u8]) -> io::Result<Self> {
        let mut receipt = Self::new(path, expected.to_vec())?;
        let (metadata, bytes) = receipt
            .previous
            .as_ref()
            .ok_or_else(|| invalid("checkpoint is absent"))?;
        if bytes != expected {
            return Err(invalid("checkpoint does not match retained bytes"));
        }
        receipt.published = Some(metadata.clone());
        receipt.publish()?;
        Ok(receipt)
    }

    pub fn new(path: &Path, bytes: Vec<u8>) -> io::Result<Self> {
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LIMIT {
            return Err(invalid("checkpoint exceeds 64 MiB"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| invalid("missing checkpoint parent"))?;
        std::fs::create_dir_all(parent)?;
        let parent = parent.canonicalize()?;
        let name = path
            .file_name()
            .ok_or_else(|| invalid("missing checkpoint filename"))?;
        let path = parent.join(name);
        Ok(Self {
            previous: read(&path)?,
            parent_identity: std::fs::metadata(&parent)?,
            path,
            parent,
            bytes,
            published: None,
            temporary: None,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn previous_bytes(&self) -> Option<&[u8]> {
        self.previous.as_ref().map(|(_, bytes)| bytes.as_slice())
    }

    #[cfg(test)]
    pub(crate) fn fail_after_rename(&mut self) -> io::Result<()> {
        self.publish_with(|stage| {
            if stage == "after_rename" {
                Err(io::Error::other("injected acknowledgement failure"))
            } else {
                Ok(())
            }
        })
    }

    pub fn publish(&mut self) -> io::Result<()> {
        self.publish_with(|_| Ok(()))
    }

    fn publish_with(&mut self, mut boundary: impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
        let _lock = super::publication_lock::acquire(&self.path)?;
        self.check_parent()?;
        self.cleanup_temporary(&mut boundary)?;
        if let Some(published) = &self.published {
            self.sync_published(published, &mut boundary)?;
            return Ok(());
        }
        let current = read(&self.path)?;
        if !match (&self.previous, &current) {
            (None, None) => true,
            (Some((a, left)), Some((b, right))) => unchanged(a, b) && left == right,
            _ => false,
        } {
            return Err(invalid("checkpoint changed before publication"));
        }
        let temporary = self.parent.join(format!(
            ".checkpoint-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        let result = (|| {
            boundary("before_write")?;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            self.temporary = Some((temporary.clone(), file));
            self.temporary
                .as_mut()
                .expect("owned temporary")
                .1
                .write_all(&self.bytes)?;
            boundary("before_file_sync")?;
            self.temporary
                .as_ref()
                .expect("owned temporary")
                .1
                .sync_all()?;
            self.check_parent()?;
            let latest = read(&self.path)?;
            if !match (&current, &latest) {
                (None, None) => true,
                (Some((a, left)), Some((b, right))) => unchanged(a, b) && left == right,
                _ => false,
            } {
                return Err(invalid("checkpoint changed before rename"));
            }
            // The open descriptor identifies our own inode even if parent sync fails.
            let owned = self
                .temporary
                .as_ref()
                .expect("owned temporary")
                .1
                .metadata()?;
            std::fs::rename(&temporary, &self.path)?;
            self.published = Some(owned);
            self.temporary = None;
            boundary("after_rename")?;
            self.sync_published(
                self.published.as_ref().expect("published inode"),
                &mut boundary,
            )
        })();
        if let Err(error) = &result
            && let Err(cleanup) = self.cleanup_temporary(&mut boundary)
        {
            return Err(io::Error::other(format!(
                "{error}; owned temporary cleanup failed: {cleanup}; path={}",
                temporary.display()
            )));
        }
        result
    }

    fn cleanup_temporary(
        &mut self,
        boundary: &mut impl FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        let Some((path, file)) = &self.temporary else {
            return Ok(());
        };
        self.check_parent()?;
        match std::fs::symlink_metadata(path) {
            Ok(current) => {
                if !current.is_file() || !identity(&file.metadata()?, &current) {
                    return Err(invalid("owned checkpoint temporary was replaced"));
                }
                boundary("before_cleanup")?;
                std::fs::remove_file(path)?;
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {},
            Err(error) => return Err(error),
        }
        boundary("before_cleanup_sync")?;
        File::open(&self.parent)?.sync_all()?;
        self.check_parent()?;
        self.temporary = None;
        Ok(())
    }

    fn check_parent(&self) -> io::Result<()> {
        let current = std::fs::symlink_metadata(&self.parent)?;
        let before = &self.parent_identity;
        if !current.is_dir()
            || (before.dev(), before.ino(), before.mode(), before.uid())
                != (current.dev(), current.ino(), current.mode(), current.uid())
        {
            return Err(invalid("checkpoint parent changed"));
        }
        Ok(())
    }

    fn sync_published(
        &self,
        published: &Metadata,
        boundary: &mut impl FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        let (current, bytes) =
            read(&self.path)?.ok_or_else(|| invalid("published checkpoint disappeared"))?;
        if !identity(published, &current) || current.mode() & 0o777 != 0o600 || bytes != self.bytes
        {
            return Err(invalid("published checkpoint identity or bytes changed"));
        }
        let file = File::open(&self.path)?;
        if !unchanged(&current, &file.metadata()?) {
            return Err(invalid("checkpoint changed before sync"));
        }
        boundary("before_resync")?;
        file.sync_all()?;
        self.check_parent()?;
        boundary("before_parent_sync")?;
        File::open(&self.parent)?.sync_all()?;
        if !unchanged(&current, &file.metadata()?)
            || !unchanged(&current, &std::fs::symlink_metadata(&self.path)?)
        {
            return Err(invalid("checkpoint changed during sync"));
        }
        self.check_parent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_each_io_boundary_without_rewriting_visible_output() {
        for point in [
            "before_write",
            "before_file_sync",
            "after_rename",
            "before_resync",
            "before_parent_sync",
        ] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("state.json");
            super::super::atomic_private_write(&path, b"old").unwrap();
            let mut write = FrozenWrite::new(&path, b"new".to_vec()).unwrap();
            for failure_point in [point, "before_parent_sync"] {
                assert!(
                    write
                        .publish_with(|stage| if stage == failure_point {
                            Err(io::Error::other("fixture sync failure"))
                        } else {
                            Ok(())
                        })
                        .is_err()
                );
            }
            let visible = write.published.as_ref().map(MetadataExt::ino);
            write.publish().unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), b"new");
            if let Some(inode) = visible {
                assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
            }
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
        }
    }

    #[test]
    fn drift_after_publication_is_not_overwritten_even_with_identical_bytes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        let mut write = FrozenWrite::new(&path, b"new".to_vec()).unwrap();
        assert!(
            write
                .publish_with(|stage| if stage == "after_rename" {
                    Err(io::Error::other("fixture"))
                } else {
                    Ok(())
                })
                .is_err()
        );
        super::super::atomic_private_write(&path, b"new").unwrap();
        assert!(write.publish().is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"new");
    }

    #[test]
    fn foreign_baseline_and_corrupt_target_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        let mut write = FrozenWrite::new(&path, b"ours".to_vec()).unwrap();
        std::fs::write(&path, b"foreign").unwrap();
        assert!(write.publish().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"foreign");
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(FrozenWrite::new(&link, b"ours".to_vec()).is_err());
    }

    #[test]
    fn participating_publishers_share_nonblocking_exclusion() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        let mut publication = FrozenWrite::new(&path, b"selected".to_vec()).unwrap();
        let lock = super::super::publication_lock::acquire(&path).unwrap();
        assert!(publication.publish().is_err());
        assert!(super::super::atomic_checkpoint_write(&path, b"competing").is_err());
        assert!(!path.exists());
        drop(lock);
        publication.publish().unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"selected");
    }

    #[test]
    fn explicit_retry_recovers_owned_temporary_cleanup_without_new_debt() {
        for cleanup_point in ["before_cleanup", "before_cleanup_sync"] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("state.json");
            let mut publication = FrozenWrite::new(&path, b"selected".to_vec()).unwrap();
            assert!(
                publication
                    .publish_with(|stage| {
                        if stage == "before_file_sync" || stage == cleanup_point {
                            return Err(io::Error::other("transient fixture failure"));
                        }
                        Ok(())
                    })
                    .is_err()
            );
            assert!(publication.temporary.is_some());
            assert!(!path.exists());
            publication.publish().unwrap();
            assert!(publication.temporary.is_none());
            assert_eq!(std::fs::read(path).unwrap(), b"selected");
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
        }
    }

    #[test]
    fn concurrent_replacement_before_rename_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        let mut write = FrozenWrite::new(&path, b"ours".to_vec()).unwrap();
        assert!(
            write
                .publish_with(|stage| {
                    if stage == "before_file_sync" {
                        std::fs::write(&path, b"foreign")?;
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(write.publish().is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"foreign");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }
}
