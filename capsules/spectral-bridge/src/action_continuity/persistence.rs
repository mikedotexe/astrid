//! Generic JSON / JSONL persistence helpers and the continuity-index I/O for
//! [`ActionContinuityStore`].
//!
//! Session appends share the reader transaction lock and preserve typed progress.
//! Other JSON/JSONL stores retain their existing persistence behavior.

use super::*;
use std::io::{Read, Seek, SeekFrom};

impl ActionContinuityStore {
    pub(super) fn load_index(&self) -> Result<ContinuityIndex> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(ContinuityIndex::default());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub(super) fn save_index(&self, index: &ContinuityIndex) -> Result<()> {
        self.write_json(&self.index_path(), index)
    }

    pub(super) fn append_jsonl<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.file_name() == Some(OsStr::new("continuity_sessions.jsonl")) {
            return self.append_session_jsonl(path, serde_json::to_value(value)?);
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(value)?)?;
        Ok(())
    }

    // All session writers cooperate with the reader's revision transaction. Keep
    // the lock through validation, append and fsync; never rewrite this inode.
    fn append_session_jsonl(&self, path: &Path, mut record: Value) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;
        file.lock()?;
        file.seek(SeekFrom::Start(0))?;
        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        if !raw.is_empty() && !raw.ends_with('\n') {
            return Err(anyhow!(
                "Incomplete continuity log tail; no new session record appended."
            ));
        }
        let mut latest = None;
        for line in raw.lines().filter(|line| !line.trim().is_empty()) {
            let row: Value = serde_json::from_str(line)
                .context("Invalid continuity log; no new session record appended")?;
            if row.get("session_id") == record.get("session_id")
                && row["record_schema"] == "continuity_session_v1"
                && row["record_type"] != "session_draft"
            {
                latest = Some(row);
            }
        }
        let expected = record
            .as_object_mut()
            .and_then(|row| row.remove("expected_session_record_id"));
        if let Some(expected) = expected
            && latest.as_ref().and_then(|row| row.get("record_id")) != Some(&expected)
        {
            return Err(ContinuityInputError("The session changed after it was read. Inspect its latest status and retry; no update was appended.").into());
        }
        if record["record_type"] != "session_draft"
            && let Some(bookmark) = latest
                .as_ref()
                .and_then(|row| row.get("reader_bookmark_v1"))
        {
            let mut bookmark: ReaderBookmark = serde_json::from_value(bookmark.clone())?;
            let disposition = match record["status"].as_str() {
                Some("active" | "summarized") => ReaderDisposition::Active,
                Some("parked" | "held") => ReaderDisposition::Parked,
                Some("complete") => ReaderDisposition::Complete,
                Some("abandoned") => ReaderDisposition::Abandoned,
                _ => bookmark.disposition,
            };
            if disposition == ReaderDisposition::Active
                && bookmark.disposition != ReaderDisposition::Active
                && record["record_type"] != "session_reopen"
            {
                return Err(ContinuityInputError("Explicitly resume this reader before updating an active session. Its bookmark remains unchanged.").into());
            }
            if disposition != bookmark.disposition {
                bookmark.revision = bookmark
                    .revision
                    .checked_add(1)
                    .context("Reader revision exhausted")?;
                bookmark.disposition = disposition;
            }
            record["reader_bookmark_v1"] = serde_json::to_value(bookmark)?;
        }
        let mut bytes = serde_json::to_vec(&record)?;
        bytes.push(b'\n');
        file.write_all(&bytes)?;
        file.sync_all()?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    }

    pub(super) fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?)
            .with_context(|| format!("writing {}", path.display()))
    }
}
