//! Private queue persistence and immutable source retention. No prompt or model IO.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow, bail};
use fs2::FileExt;
use sha2::{Digest, Sha256};

use super::super::correspondence_v1;
use super::{
    DurableInbox, InboxPendingLetter, InboxReservation, MAX_QUEUE_ITEMS, MAX_RETAINED_LETTER_BYTES,
    QueueLetter, QueueState, advance_sequence,
};

const MAX_STATE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SCAN_FILES: usize = 20_000;

pub(super) struct LockedQueue {
    pub state: QueueState,
    lock: File,
    root: std::path::PathBuf,
}

impl LockedQueue {
    pub fn open(queue: &DurableInbox) -> Result<Self> {
        private_directory(&queue.queue_root)?;
        private_directory(&queue.queue_root.join("sources"))?;
        let lock_path = queue.queue_root.join("queue.lock");
        if lock_path
            .symlink_metadata()
            .is_ok_and(|metadata| !metadata.is_file())
        {
            bail!("inbox queue lock is not a regular file");
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&lock_path)?;
        lock.lock_exclusive()?;
        let state_path = queue.queue_root.join("state.json");
        let state = match fs::symlink_metadata(&state_path) {
            Ok(metadata) if metadata.is_file() => {
                serde_json::from_slice::<QueueState>(&read_regular(&state_path, MAX_STATE_BYTES)?)
                    .context(
                    "inbox queue state is invalid; existing evidence was not repaired or replaced",
                )?
            },
            Ok(_) => bail!("inbox queue state is not a regular file"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => QueueState::default(),
            Err(error) => return Err(error.into()),
        };
        validate_state(queue, &state)?;
        Ok(Self {
            state,
            lock,
            root: queue.queue_root.clone(),
        })
    }

    pub fn save(&self) -> Result<()> {
        let bytes = serde_json::to_vec(&self.state)?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            bail!("inbox state capacity reached; history was not discarded");
        }
        atomic_write(&self.root.join("state.json"), &bytes, false)
            .context("inbox state save was not confirmed; reopen the queue to reconcile the exact window/receipt")
    }
}

impl Drop for LockedQueue {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
    }
}

fn validate_state(queue: &DurableInbox, state: &QueueState) -> Result<()> {
    if state.schema != "durable_inbox_v1"
        || state.letters.len() > MAX_QUEUE_ITEMS
        || state.attempts.len() > MAX_QUEUE_ITEMS
        || state.windows.len() > MAX_QUEUE_ITEMS
        || state.source_observations.len() > MAX_QUEUE_ITEMS
    {
        bail!("unsupported or oversized inbox queue state");
    }
    for (version, letter) in &state.letters {
        if version != &letter.identity.version_id
            || !valid_hash(version)
            || letter.identity.source_path.parent() != Some(queue.inbox_dir.as_path())
            || letter
                .identity
                .content_sha256
                .as_deref()
                .is_some_and(|digest| !valid_hash(digest))
        {
            bail!("inbox state identity/path validation failed");
        }
    }
    for attempt in state.attempts.values() {
        if !state.letters.contains_key(&attempt.version_id)
            || !state.windows.contains(&attempt.window_id)
        {
            bail!("inbox attempt references missing durable state");
        }
    }
    Ok(())
}

pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn scan_paths(
    queue: &DurableInbox,
) -> Result<Vec<(std::time::Duration, std::path::PathBuf, Metadata)>> {
    let entries = match fs::read_dir(&queue.inbox_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "txt") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() {
            continue;
        }
        if paths.len() >= MAX_SCAN_FILES {
            bail!("inbox scan exceeded the reviewed source count; no sources were retired");
        }
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        paths.push((modified, path, metadata));
    }
    paths.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    Ok(paths)
}

pub(super) fn discover(queue: &DurableInbox, state: &mut QueueState) -> Result<usize> {
    let paths = scan_paths(queue)?;
    let mut unreadable = 0_usize;
    for (_, path, metadata) in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("inbox source filename is not UTF-8"))?;
        let stamp = metadata_stamp(&metadata);
        if state.source_observations.get(name) == Some(&stamp) {
            continue;
        }
        if !state.source_observations.contains_key(name)
            && state.source_observations.len() >= MAX_QUEUE_ITEMS
        {
            bail!("inbox source history reached its reviewed capacity");
        }
        if metadata.len() > MAX_RETAINED_LETTER_BYTES as u64 {
            let version = hash(
                format!(
                    "oversized\0{name}\0{}\0{}\0{}\0{}\0{}",
                    metadata.dev(),
                    metadata.ino(),
                    metadata.len(),
                    metadata.mtime(),
                    metadata.mtime_nsec()
                )
                .as_bytes(),
            );
            let observed_name = name.to_string();
            insert_letter(
                state,
                InboxPendingLetter {
                    message_id: name.to_string(),
                    thread_id: format!("local_source:{name}"),
                    version_id: version,
                    source_path: path,
                    content_sha256: None,
                    byte_len: metadata.len(),
                },
            )?;
            state.source_observations.insert(observed_name, stamp);
            continue;
        }
        let bytes = match read_regular(&path, MAX_RETAINED_LETTER_BYTES as u64) {
            Ok(bytes) => bytes,
            Err(_) => {
                unreadable = unreadable.saturating_add(1);
                continue;
            },
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) if !text.trim().is_empty() => text,
            _ => {
                unreadable = unreadable.saturating_add(1);
                continue;
            },
        };
        let identity = correspondence_v1::contact_source_identity_for_inbox_file(&path, text);
        let message_id = identity.as_ref().map_or_else(
            || name.to_string(),
            |identity| identity.source_ref_id.clone(),
        );
        let thread_id = identity.as_ref().map_or_else(
            || format!("local_source:{name}"),
            |identity| {
                identity
                    .thread_id
                    .clone()
                    .unwrap_or_else(|| format!("legacy_principal:{}", identity.principal))
            },
        );
        let content_sha256 = hash(&bytes);
        let version = hash(format!("{message_id}\0{content_sha256}").as_bytes());
        if state.letters.contains_key(&version) {
            state.source_observations.insert(name.to_string(), stamp);
            continue;
        }
        let source_path = queue
            .queue_root
            .join("sources")
            .join(format!("{content_sha256}.txt"));
        retain_exact(&source_path, &bytes)?;
        let observed_name = name.to_string();
        insert_letter(
            state,
            InboxPendingLetter {
                message_id,
                thread_id,
                version_id: version,
                source_path: path,
                content_sha256: Some(content_sha256),
                byte_len: bytes.len() as u64,
            },
        )?;
        state.source_observations.insert(observed_name, stamp);
    }
    Ok(unreadable)
}

fn insert_letter(state: &mut QueueState, identity: InboxPendingLetter) -> Result<()> {
    if state.letters.contains_key(&identity.version_id) {
        return Ok(());
    }
    if state.letters.len() >= MAX_QUEUE_ITEMS {
        bail!("inbox queue capacity reached; no history was discarded");
    }
    let arrival_sequence = advance_sequence(state)?;
    state.letters.insert(
        identity.version_id.clone(),
        QueueLetter {
            identity,
            arrival_sequence,
            attempts: 0,
            retry_after_unix_ms: 0,
            receipt: None,
        },
    );
    Ok(())
}

pub(super) fn retained_text(queue: &DurableInbox, letter: &InboxPendingLetter) -> Result<String> {
    let digest = letter
        .content_sha256
        .as_deref()
        .filter(|digest| valid_hash(digest))
        .ok_or_else(|| anyhow!("inbox letter requires an explicit source reading window"))?;
    let bytes = read_regular(
        &queue
            .queue_root
            .join("sources")
            .join(format!("{digest}.txt")),
        MAX_RETAINED_LETTER_BYTES as u64,
    )?;
    if bytes.len() as u64 != letter.byte_len || hash(&bytes) != digest {
        bail!("retained inbox source changed; delivery refused");
    }
    Ok(String::from_utf8(bytes)?)
}

pub(super) fn archive(
    queue: &DurableInbox,
    reservation: &InboxReservation,
) -> Result<std::path::PathBuf> {
    let read_dir = queue.inbox_dir.join("read");
    private_directory(&read_dir)?;
    let path = read_dir.join(format!("{}.txt", reservation.letter.version_id));
    retain_exact(&path, reservation.text.as_bytes())?;
    Ok(path)
}

fn retain_exact(path: &Path, bytes: &[u8]) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            if read_regular(path, MAX_RETAINED_LETTER_BYTES as u64)? != bytes {
                bail!("immutable inbox artifact differs from its identity");
            }
            File::open(path)?.sync_all()?;
            if let Some(parent) = path.parent() {
                File::open(parent)?.sync_all()?;
            }
            Ok(())
        },
        Ok(_) => bail!("inbox artifact is not a regular file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            atomic_write(path, bytes, true)
        },
        Err(error) => Err(error.into()),
    }
}

fn private_directory(path: &Path) -> Result<()> {
    if path.exists() {
        if !fs::symlink_metadata(path)?.is_dir() {
            bail!("inbox storage directory is not a regular directory");
        }
        return Ok(());
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    File::open(path)?.sync_all()?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn read_regular(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.len() > max_bytes {
        bail!("inbox input is symlinked, non-regular or oversized");
    }
    let file = File::open(path)?;
    if !same_file(&before, &file.metadata()?) {
        bail!("inbox input changed while opening");
    }
    let mut bytes = Vec::new();
    (&file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes
        || !same_file(&before, &file.metadata()?)
        || !same_file(&before, &fs::symlink_metadata(path)?)
    {
        bail!("inbox input changed while reading");
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8], immutable: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("inbox artifact has no parent"))?;
    let temporary = parent.join(format!(".inbox_{:032x}.pending", rand::random::<u128>()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if immutable {
        // A hard link publishes without overwriting a concurrently created object.
        fs::hard_link(&temporary, path)?;
        fs::remove_file(&temporary)?;
    } else {
        fs::rename(&temporary, path)?;
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn metadata_stamp(metadata: &Metadata) -> String {
    hash(
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec()
        )
        .as_bytes(),
    )
}
