//! Full-history, locked JSONL access and immutable retained source artifacts.

use super::*;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub(super) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(super) fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
        || matches!(value, "." | "..")
    {
        return Err(anyhow!("invalid reader identity"));
    }
    Ok(())
}

pub(super) fn hash_shape(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn read_utf8(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > READER_SOURCE_MAX_BYTES {
        return Err(anyhow!("reader source is not a bounded regular UTF-8 file"));
    }
    let mut reader = file.take(READER_SOURCE_MAX_BYTES.saturating_add(1));
    let mut text = String::new();
    reader
        .read_to_string(&mut text)
        .context("reading UTF-8 source")?;
    let after = reader.get_ref().metadata()?;
    if before.len() != after.len()
        || before.modified()? != after.modified()?
        || u64::try_from(text.len())? != before.len()
    {
        return Err(anyhow!("reader source changed during capture"));
    }
    Ok(text)
}

pub(super) fn retained_text(
    store: &ActionContinuityStore,
    source: &ReaderSourceSnapshot,
) -> Result<String> {
    if source.encoding != "utf-8"
        || !hash_shape(&source.sha256)
        || source.retained_artifact
            != PathBuf::from("reader_sources").join(format!("{}.utf8", source.sha256))
    {
        return Err(anyhow!("invalid retained source identity"));
    }
    if let Some(raw) = &source.raw_source {
        if raw.raw_source.is_some() {
            return Err(anyhow!("nested reading view is invalid"));
        }
        retained_text(store, raw)?;
    }
    let path = store.root.join(&source.retained_artifact);
    if fs::symlink_metadata(&path)?.file_type().is_symlink()
        || fs::symlink_metadata(store.root.join("reader_sources"))?
            .file_type()
            .is_symlink()
    {
        return Err(anyhow!("retained source must not be a symlink"));
    }
    let text = read_utf8(&path)?;
    if digest(text.as_bytes()) != source.sha256 || u64::try_from(text.len())? != source.byte_count {
        return Err(anyhow!("retained source digest or length changed"));
    }
    Ok(text)
}

pub(super) fn retain_source(
    store: &ActionContinuityStore,
    path: &Path,
) -> Result<ReaderSourceSnapshot> {
    let original_path = path.canonicalize()?;
    let text = read_utf8(&original_path)?;
    let sha256 = digest(text.as_bytes());
    let relative = PathBuf::from("reader_sources").join(format!("{sha256}.utf8"));
    let directory = store.root.join("reader_sources");
    fs::create_dir_all(&directory)?;
    if fs::symlink_metadata(&directory)?.file_type().is_symlink() {
        return Err(anyhow!("retained source directory must not be a symlink"));
    }
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let raw_source =
        crate::prompt_budget::reading_view::verified_raw_source(&original_path, &text)?
            .map(|(raw, expected)| {
                let retained = retain_source(store, &raw)?;
                if retained.sha256 != expected {
                    return Err(anyhow!("reading view origin changed during retention"));
                }
                Ok(Box::new(retained))
            })
            .transpose()?;
    let snapshot = ReaderSourceSnapshot {
        raw_source,
        original_path,
        sha256,
        byte_count: u64::try_from(text.len())?,
        retained_artifact: relative,
        encoding: "utf-8".to_string(),
    };
    let destination = store.root.join(&snapshot.retained_artifact);
    if destination.exists() {
        retained_text(store, &snapshot)?;
        File::open(&destination)?.sync_all()?;
        File::open(&directory)?.sync_all()?;
        File::open(&store.root)?.sync_all()?;
        return Ok(snapshot);
    }
    let temporary = directory.join(format!(".reader-{:032x}.tmp", rand::random::<u128>()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(text.as_bytes())?;
        file.set_permissions(fs::Permissions::from_mode(0o400))?;
        file.sync_all()?;
        match fs::hard_link(&temporary, &destination) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                retained_text(store, &snapshot)?;
            },
            Err(error) => return Err(error.into()),
        }
        File::open(&directory)?.sync_all()?;
        File::open(&store.root)?.sync_all()?;
        Ok(())
    })();
    // Only this invocation's temporary artifact may be removed.
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    result?;
    Ok(snapshot)
}

pub(super) fn passage_text<'a>(text: &'a str, passage: &ReaderPassage) -> Result<&'a str> {
    let start = usize::try_from(passage.start_byte)?;
    let end = usize::try_from(passage.end_byte)?;
    let span = text
        .get(start..end)
        .ok_or_else(|| anyhow!("passage is not a UTF-8 byte range"))?;
    if start >= end || digest(span.as_bytes()) != passage.sha256 {
        return Err(anyhow!("passage digest or range mismatch"));
    }
    Ok(span)
}

pub(super) struct ReaderSessionLog {
    pub file: File,
    pub path: PathBuf,
    pub latest: Option<Value>,
    pub prior_operation: Option<Value>,
}

impl ReaderSessionLog {
    pub fn open(
        store: &ActionContinuityStore,
        thread: &str,
        session: &str,
        operation: Option<&str>,
    ) -> Result<Option<Self>> {
        identifier(thread)?;
        identifier(session)?;
        if let Some(operation) = operation {
            identifier(operation)?;
        }
        let path = store.continuity_sessions_path(thread);
        let file = match OpenOptions::new()
            .read(true)
            .write(operation.is_some())
            .append(operation.is_some())
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if operation.is_some() {
            file.lock()?;
        } else {
            file.lock_shared()?;
        }
        let mut reader = BufReader::new(&file);
        let mut line = String::new();
        let mut latest = None;
        let mut prior_operation = None;
        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            if !line.ends_with('\n') {
                return Err(anyhow!(
                    "continuity session log has an incomplete tail; no reader state changed"
                ));
            }
            if line.trim().is_empty() {
                continue;
            }
            let row: Value = serde_json::from_str(&line)
                .context("invalid continuity session record; no reader state changed")?;
            if row.get("session_id").and_then(Value::as_str) != Some(session)
                || row.get("record_schema").and_then(Value::as_str) != Some("continuity_session_v1")
                || row.get("record_type").and_then(Value::as_str) == Some("session_draft")
            {
                continue;
            }
            if operation.is_some()
                && row
                    .pointer("/reader_operation_v1/operation_id")
                    .and_then(Value::as_str)
                    == operation
            {
                prior_operation = row.get("reader_operation_v1").cloned();
            }
            latest = Some(row);
        }
        Ok(Some(Self {
            file,
            path,
            latest,
            prior_operation,
        }))
    }

    pub fn sync(&self) -> Result<()> {
        self.file.sync_all()?;
        if let Some(parent) = self.path.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok(())
    }

    pub fn append(&mut self, row: &Value, operation_id: &str, record_id: &str) -> Result<()> {
        let mut bytes = serde_json::to_vec(row)?;
        bytes.push(b'\n');
        self.file.seek(SeekFrom::End(0))?;
        self.file
            .write_all(&bytes)
            .map_err(|error| ReaderPersistenceError {
                operation_id: operation_id.to_string(),
                record_id: record_id.to_string(),
                stage: ReaderAppendStage::PartialOrUnknown,
                detail: error.to_string(),
            })?;
        self.sync()
            .map_err(|error| ReaderPersistenceError {
                operation_id: operation_id.to_string(),
                record_id: record_id.to_string(),
                stage: ReaderAppendStage::AppendedNotSynced,
                detail: error.to_string(),
            })
            .map_err(Into::into)
    }
}
