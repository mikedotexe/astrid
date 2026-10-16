//! Bounded, non-mutating views. A moving or corrupt source is not partial evidence.
use super::*;
use std::collections::BTreeMap;
use std::io::Read as _;

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Snapshot {
    files: BTreeMap<PathBuf, Option<Vec<u8>>>,
    total: usize,
}

fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => anyhow::ensure!(metadata.is_file(), "inspection requires a regular file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_FILE.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= MAX_FILE, "inspection file bound exceeded");
    Ok(Some(bytes))
}

impl Snapshot {
    pub(super) fn bytes(&mut self, path: &Path) -> Result<Option<&[u8]>> {
        if !self.files.contains_key(path) {
            let bytes = read_bounded(path)?;
            self.total = self
                .total
                .saturating_add(bytes.as_ref().map_or(0, Vec::len));
            anyhow::ensure!(self.total <= MAX_TOTAL, "inspection total bound exceeded");
            self.files.insert(path.into(), bytes);
        }
        Ok(self.files.get(path).and_then(|bytes| bytes.as_deref()))
    }

    pub(super) fn json<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &Path,
    ) -> Result<Option<T>> {
        self.bytes(path)?
            .map(serde_json::from_slice)
            .transpose()
            .map_err(Into::into)
    }

    pub(super) fn records<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &Path,
    ) -> Result<Vec<T>> {
        let Some(bytes) = self.bytes(path)? else {
            return Ok(Vec::new());
        };
        std::str::from_utf8(bytes)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(Into::into))
            .collect()
    }

    pub(super) fn validate(&self) -> Result<()> {
        for (path, bytes) in &self.files {
            anyhow::ensure!(
                read_bounded(path)? == *bytes,
                "inspection source changed during read"
            );
        }
        Ok(())
    }
}

pub(super) fn safe_identity(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(super) fn digest(record: &Value) -> Result<String> {
    use sha2::{Digest as _, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(record)?)))
}

fn unavailable(reference: Option<&str>) -> Value {
    let mut value = return_support::project(reference, None, None, None);
    value["record_status"] = "unavailable".into();
    value["saved_next_action"] = "unknown".into();
    value["saved_next_action_scope"] = "unavailable".into();
    value
}

impl ActionContinuityStore {
    pub(super) fn inspect_retained_thread(&self, id: &str) -> Result<ResearchThread> {
        anyhow::ensure!(safe_identity(id), "invalid saved thread identity");
        let mut snapshot = Snapshot::default();
        let thread = snapshot
            .json::<ResearchThread>(&self.thread_dir(id).join("thread.json"))?
            .context("thread record unavailable")?;
        anyhow::ensure!(
            thread.schema_version == 1 && thread.thread_id == id && thread.system_origin == SYSTEM,
            "thread ownership or schema mismatch"
        );
        snapshot.validate()?;
        Ok(thread)
    }

    pub(super) fn inspect_current_thread(&self) -> Result<Option<ResearchThread>> {
        let mut snapshot = Snapshot::default();
        let Some(index) = snapshot.json::<Value>(&self.index_path())? else {
            return Ok(None);
        };
        let Some(id) = index["active_thread_id"].as_str() else {
            return Ok(None);
        };
        anyhow::ensure!(safe_identity(id), "invalid saved thread identity");
        let thread = snapshot
            .json::<ResearchThread>(&self.thread_dir(id).join("thread.json"))?
            .context("selected thread record unavailable")?;
        anyhow::ensure!(
            thread.schema_version == 1 && thread.thread_id == id && thread.system_origin == SYSTEM,
            "thread ownership or schema mismatch"
        );
        snapshot.validate()?;
        Ok(Some(thread))
    }

    pub(super) fn saved_return_support(&self, thread_id: &str, requested: Option<&str>) -> Value {
        let mut snapshot = Snapshot::default();
        let thread = if safe_identity(thread_id) {
            snapshot
                .json::<ResearchThread>(&self.thread_dir(thread_id).join("thread.json"))
                .ok()
                .flatten()
                .filter(|thread| {
                    thread.schema_version == 1
                        && thread.thread_id == thread_id
                        && thread.system_origin == SYSTEM
                })
        } else {
            None
        };
        let reference = requested.or_else(|| {
            thread
                .as_ref()
                .and_then(|t| t.active_experiment_id.as_deref())
        });
        if thread.is_none() {
            return unavailable(reference);
        }
        let result = (|| -> Result<Value> {
            let record = if let Some(id) = reference {
                anyhow::ensure!(
                    safe_identity(id) && id.starts_with(LOCAL_EXPERIMENT_PREFIX),
                    "invalid local reference"
                );
                snapshot
                    .records::<ExperimentRecord>(&self.experiments_path(thread_id))?
                    .into_iter()
                    .rev()
                    .find(|record| record.experiment_id == id && record.thread_id == thread_id)
            } else {
                None
            };
            snapshot.validate()?;
            let guard = record.as_ref().filter(|r| r.status == "paused").map(|r| {
                paused_primary_return_v1(&r.experiment_id, r.planned_next.as_deref(), None).1
            });
            let value = record.as_ref().map(serde_json::to_value).transpose()?;
            Ok(return_support::project(
                reference,
                thread.as_ref().and_then(|t| t.current_next.as_deref()),
                value.as_ref(),
                guard.as_deref(),
            ))
        })();
        result.unwrap_or_else(|_| unavailable(reference))
    }
}
