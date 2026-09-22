//! Synchronous, owner-scoped transactions shared by host selection and native stores.
use anyhow::Result;
use fs2::FileExt as _;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

thread_local! {
    static HELD: RefCell<BTreeMap<PathBuf, Weak<File>>> = const { RefCell::new(BTreeMap::new()) };
}

/// Reentrant on the same synchronous thread; deliberately not Send/Sync.
/// Never retain this guard across inference or an async suspension.
pub struct OwnerTransaction {
    _file: Rc<File>,
}

impl OwnerTransaction {
    /// # Errors
    /// Fails if the owner directory or its cross-process lock cannot be opened.
    pub fn acquire(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let directory = directory.canonicalize()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        let file = HELD.with(|held| -> Result<Rc<File>> {
            let mut held = held.borrow_mut();
            held.retain(|_, file| file.strong_count() > 0);
            if let Some(file) = held.get(&directory).and_then(Weak::upgrade) {
                return Ok(file);
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(directory.join("reader.lock"))?;
            file.lock_exclusive()?;
            crate::preparation::recover(&directory)?;
            let file = Rc::new(file);
            held.insert(directory, Rc::downgrade(&file));
            Ok(file)
        })?;
        Ok(Self { _file: file })
    }
}
