// A bounded, private spool with one writer process. Quota exhaustion is visible
// and never deletes evidence or silently becomes a zero-opportunity response.
struct ProviderObservationStore {
    root: std::path::PathBuf,
    state: std::sync::Mutex<Option<ProviderObservationStoreState>>,
    max_bytes: u64,
    max_raw_bytes: u64,
    failures: std::sync::atomic::AtomicU64,
}

struct ProviderObservationStoreState {
    // Keep the exclusive lock alive for this writer's lifetime.
    _lock: std::fs::File,
    bytes: u64,
    raw_bytes: u64,
    files: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum ProviderObservationWriteError {
    Quota,
    Io,
}

impl ProviderObservationStore {
    fn new(root: std::path::PathBuf) -> Self {
        Self {
            root,
            state: std::sync::Mutex::new(None),
            max_bytes: PROVIDER_SPOOL_MAX_BYTES,
            max_raw_bytes: PROVIDER_RAW_SPOOL_MAX_BYTES,
            failures: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn failure(&self, stage: &'static str, reason: &ProviderObservationWriteError) {
        let failures = self
            .failures
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .saturating_add(1);
        // Use the existing logging failure channel. Do not try to store the
        // only failure signal in the same broken spool, or log any raw text.
        warn!(stage, reason = ?reason, failures,
            "provider observation recording_failed; generation continues unchanged");
    }

    fn write_event(&self, id: &str, value: &serde_json::Value) -> bool {
        let result = serde_json::to_vec(value)
            .map_err(|_| ProviderObservationWriteError::Io)
            .and_then(|bytes| self.write("events", &format!("{id}.json"), &bytes));
        match result {
            Ok(()) => true,
            Err(error) => {
                self.failure("envelope", &error);
                false
            },
        }
    }

    fn write_raw(&self, raw: &str) -> Result<String, ProviderObservationWriteError> {
        let name = format!("{}.txt", generation_sha256_hex(raw));
        if let Err(error) = self.write("raw", &name, raw.as_bytes()) {
            self.failure("raw_artifact", &error);
            return Err(error);
        }
        Ok(format!("raw/{name}"))
    }

    fn write(
        &self,
        subdir: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), ProviderObservationWriteError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ProviderObservationWriteError::Io)?;
        if state.is_none() {
            *state = Some(
                self.initialize()
                    .map_err(|_| ProviderObservationWriteError::Io)?,
            );
        }
        let state = state.as_mut().ok_or(ProviderObservationWriteError::Io)?;
        let path = self.root.join(subdir).join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                // Deduplicate only an exact private regular file. Never follow
                // symlinks or bless an incomplete pre-existing artifact.
                if !metadata.is_file()
                    || metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                {
                    return Err(ProviderObservationWriteError::Io);
                }
                provider_observation_private_file(&metadata)
                    .map_err(|_| ProviderObservationWriteError::Io)?;
                return if std::fs::read(path).map_err(|_| ProviderObservationWriteError::Io)?
                    == bytes
                {
                    Ok(())
                } else {
                    Err(ProviderObservationWriteError::Io)
                };
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return Err(ProviderObservationWriteError::Io),
        }
        let count = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if state.files >= PROVIDER_SPOOL_MAX_FILES
            || state.bytes.saturating_add(count) > self.max_bytes
            || (subdir == "raw" && state.raw_bytes.saturating_add(count) > self.max_raw_bytes)
        {
            return Err(ProviderObservationWriteError::Quota);
        }
        // Charge before writing. A failed write may over-count this process's
        // quota; it must never under-count leaked partial bytes after a crash.
        state.bytes = state.bytes.saturating_add(count);
        state.files = state.files.saturating_add(1);
        if subdir == "raw" {
            state.raw_bytes = state.raw_bytes.saturating_add(count);
        }
        provider_observation_atomic_write(&path, bytes)
            .map_err(|_| ProviderObservationWriteError::Io)
    }

    fn initialize(&self) -> std::io::Result<ProviderObservationStoreState> {
        use fs2::FileExt as _;
        provider_observation_private_dir(&self.root)?;
        for name in ["raw", "events"] {
            provider_observation_private_dir(&self.root.join(name))?;
        }
        let lock_path = self.root.join("writer.lock");
        if let Ok(metadata) = std::fs::symlink_metadata(&lock_path) {
            if !metadata.is_file() {
                return Err(std::io::Error::other("invalid observation lock"));
            }
            provider_observation_private_file(&metadata)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let lock = options.open(lock_path)?;
        lock.try_lock_exclusive()?;
        let mut state = ProviderObservationStoreState {
            _lock: lock,
            bytes: 0,
            raw_bytes: 0,
            files: 0,
        };
        // One bounded inventory per process, not a growing per-request scan.
        for name in ["raw", "events"] {
            for entry in std::fs::read_dir(self.root.join(name))? {
                if state.files >= PROVIDER_SPOOL_MAX_FILES {
                    return Err(std::io::Error::other("observation file budget exhausted"));
                }
                let metadata = entry?.path().symlink_metadata()?;
                if !metadata.is_file() {
                    return Err(std::io::Error::other("invalid observation artifact"));
                }
                provider_observation_private_file(&metadata)?;
                state.files = state.files.saturating_add(1);
                state.bytes = state.bytes.saturating_add(metadata.len());
                if name == "raw" {
                    state.raw_bytes = state.raw_bytes.saturating_add(metadata.len());
                }
            }
        }
        Ok(state)
    }
}

fn provider_observation_private_dir(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(path)?;
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(std::io::Error::other(
            "observation directory must not be a symlink",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(std::io::Error::other(
                "observation directory must be private (0700)",
            ));
        }
    }
    Ok(())
}

fn provider_observation_private_file(metadata: &std::fs::Metadata) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(std::io::Error::other(
                "observation file must be private (0600)",
            ));
        }
    }
    Ok(())
}

fn provider_observation_atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let temporary = path.with_extension(format!("{}.tmp", generation_record_id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    // Hard-link publication cannot overwrite an existing destination. Only
    // complete bytes appear as .json/.txt; an interrupted .tmp is charged at
    // the next inventory and is never mistaken for a completed receipt.
    std::fs::hard_link(&temporary, path)?;
    std::fs::remove_file(temporary)?;
    Ok(())
}
