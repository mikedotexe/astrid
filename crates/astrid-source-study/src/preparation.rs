//! Owner-locked preparation: collect writes, commit a redo record, then publish.
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

const JOURNAL: &str = "preparation-transaction-v1.json";
#[cfg(test)]
#[path = "preparation_observation_tests.rs"]
mod observation_tests;
const OWNER: &str = "preparation-owner-v1.json";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const CHECKPOINTS: &[&str] = &[
    OWNER,
    "reader-v1.json",
    "source-findings-v1.json",
    "writing/drafts-v1.json",
    "writing/drafts-v2.json",
    "writing/profile.json",
    "activity-focus-v1.json",
];

#[derive(Clone, Serialize, Deserialize)]
struct Change {
    before: Option<String>,
    after: Vec<u8>,
}
#[derive(Default)]
struct Overlay {
    root: PathBuf,
    writes: BTreeMap<String, Change>,
}
thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}
#[cfg(test)]
thread_local! {
    static FAIL_AT: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn fault_boundary(point: u8) -> Result<()> {
    if FAIL_AT.with(|v| v.get() == Some(point)) {
        FAIL_AT.with(|v| v.set(None));
        anyhow::bail!("synthetic interruption at preparation boundary {point}");
    }
    Ok(())
}
struct Guard;
pub(crate) fn in_transaction() -> bool {
    OVERLAY.with(|v| v.borrow().is_some())
}
impl Drop for Guard {
    fn drop(&mut self) {
        OVERLAY.with(|v| {
            v.borrow_mut().take();
        });
    }
}

#[derive(Serialize, Deserialize)]
struct Journal {
    schema: u32,
    writes: BTreeMap<String, Change>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    body: String,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    schema: u32,
    owner: String,
    request_id: String,
    action: String,
    initial_revision: String,
    output: crate::StudyOutput,
}

fn relative(root: &Path, path: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(root)
        .context("preparation write outside owner store")?;
    ensure!(
        !rel.as_os_str().is_empty() && rel.components().all(|p| matches!(p, Component::Normal(_))),
        "invalid preparation path"
    );
    let mut full = root.to_path_buf();
    for part in rel.components() {
        full.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&full) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "symlink in preparation path"
            );
        }
    }
    Ok(rel.to_str().context("non-UTF8 preparation path")?.into())
}
fn disk_hash(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(crate::digest(bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn staged(path: &Path) -> Option<Vec<u8>> {
    OVERLAY.with(|v| {
        v.borrow().as_ref().and_then(|o| {
            path.strip_prefix(&o.root)
                .ok()
                .and_then(|p| p.to_str())
                .and_then(|p| o.writes.get(p))
                .map(|c| c.after.clone())
        })
    })
}
pub(crate) fn exists(path: &Path) -> bool {
    staged(path).is_some() || path.exists()
}
pub(crate) fn read(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    let path = path.as_ref();
    staged(path).map_or_else(|| fs::read(path), Ok)
}
pub(crate) fn capture(path: &Path, bytes: &[u8]) -> Result<bool> {
    OVERLAY.with(|value| {
        let mut value = value.borrow_mut();
        let Some(overlay) = value.as_mut() else {
            return Ok(false);
        };
        let name = relative(&overlay.root, path)?;
        ensure!(name != JOURNAL, "nested preparation journal");
        let before = overlay
            .writes
            .get(&name)
            .map_or_else(|| disk_hash(path), |c| Ok(c.before.clone()))?;
        let used = overlay
            .writes
            .iter()
            .filter(|(key, _)| *key != &name)
            .try_fold(bytes.len(), |sum, (_, c)| {
                sum.checked_add(c.after.len())
                    .context("preparation size overflow")
            })?;
        ensure!(
            used <= MAX_BYTES && overlay.writes.len() < 256,
            "preparation transaction exceeds bound"
        );
        overlay.writes.insert(
            name,
            Change {
                before,
                after: bytes.to_vec(),
            },
        );
        Ok(true)
    })
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let body = serde_json::to_string(value)?;
    Ok(serde_json::to_vec(&Envelope {
        sha256: crate::digest(&body),
        body,
    })?)
}
fn decode<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.file_type().is_file() && metadata.len() <= 512 * 1024 * 1024,
        "nonregular or oversized preparation record"
    );
    let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        crate::digest(&envelope.body) == envelope.sha256,
        "preparation record hash mismatch"
    );
    Ok(serde_json::from_str(&envelope.body)?)
}

/// Called once at outermost owner-lock acquisition, before any native writer.
pub(crate) fn recover(root: &Path) -> Result<()> {
    let path = root.join(JOURNAL);
    if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        return Ok(());
    }
    let journal: Journal =
        decode(&path).context("preparation recovery refused; preserving history")?;
    ensure!(
        journal.schema == 1 && !journal.writes.is_empty() && journal.writes.len() <= 256,
        "unsupported preparation journal"
    );
    let size = journal.writes.values().try_fold(0_usize, |n, c| {
        n.checked_add(c.after.len()).context("redo size overflow")
    })?;
    ensure!(
        size <= MAX_BYTES,
        "oversized preparation redo; preserved unchanged"
    );
    // Validate every target before changing any of them. A newer foreign write
    // is a conflict, never permission to roll back or overwrite authored state.
    for (name, change) in &journal.writes {
        let target = root.join(name);
        ensure!(
            relative(root, &target)? == *name && name != JOURNAL,
            "invalid redo target"
        );
        let current = disk_hash(&target)?;
        ensure!(
            current == change.before || current == Some(crate::digest(&change.after)),
            "preparation recovery conflicts with newer state; preserved unchanged"
        );
    }
    for (name, change) in &journal.writes {
        let target = root.join(name);
        if disk_hash(&target)? != Some(crate::digest(&change.after)) {
            fs::create_dir_all(target.parent().context("redo parent")?)?;
            crate::store::atomic_write_direct(&target, &change.after)?;
            #[cfg(test)]
            fault_boundary(2)?;
        }
    }
    fs::remove_file(path)?;
    fs::File::open(root)?.sync_all()?;
    Ok(())
}

impl crate::Reader {
    /// Snapshot token for first admission. An exact committed retry is checked first.
    /// # Errors
    /// Fails on corrupt pending transactions or unsupported native history.
    pub fn preparation_revision(&self) -> Result<String> {
        let _lock = crate::owner_transaction::OwnerTransaction::acquire(&self.directory)?;
        let rows: Vec<_> = CHECKPOINTS
            .iter()
            .map(|name| Ok((*name, disk_hash(&self.directory.join(name))?)))
            .collect::<Result<_>>()?;
        Ok(crate::digest(serde_json::to_vec(&rows)?))
    }

    /// Prepare once per durable host operation, not once per repeated prose.
    /// # Errors
    /// Rejects conflicting IDs, stale first admissions, corrupt history and I/O failures.
    pub fn prepare_once(
        &self,
        request_id: &str,
        expected_revision: &str,
        action: &str,
    ) -> Result<crate::StudyOutput> {
        let _lock = crate::owner_transaction::OwnerTransaction::acquire(&self.directory)?;
        let root = self.directory.canonicalize()?;
        let owner = &self
            .runtime
            .as_ref()
            .context("preparation requires configured owner")?
            .being;
        ensure!(
            matches!(owner.as_str(), "astrid" | "minime")
                && !request_id.is_empty()
                && request_id.len() <= 256,
            "bounded owner-scoped preparation ID required"
        );
        let path = root
            .join("preparation-operations")
            .join(format!("{}.json", crate::digest(request_id)));
        relative(&root, &path)?;
        let owner_path = root.join(OWNER);
        if owner_path.exists() {
            let saved_owner: String = decode(&owner_path)?;
            ensure!(
                saved_owner == *owner,
                "preparation store belongs to another owner"
            );
        }
        if path.exists() {
            let saved: Receipt = decode(&path)?;
            ensure!(
                saved.schema == 1
                    && saved.owner == *owner
                    && saved.request_id == request_id
                    && saved.action == action,
                "conflicting preparation retry"
            );
            return Ok(saved.output);
        }
        if let Ok(entries) = fs::read_dir(root.join("preparation-operations")) {
            ensure!(
                entries.take(65_536).count() < 65_536,
                "preparation operation history full; retained unchanged"
            );
        }
        ensure!(
            self.preparation_revision()? == expected_revision,
            "stale preparation revision; reselect explicitly"
        );
        ensure!(
            OVERLAY.with(|v| v.borrow().is_none()),
            "nested preparation transaction"
        );
        OVERLAY.with(|v| {
            *v.borrow_mut() = Some(Overlay {
                root: root.clone(),
                writes: BTreeMap::new(),
            });
        });
        let guard = Guard;
        capture(&owner_path, &encode(owner)?)?;
        let runtime = self
            .runtime
            .as_ref()
            .context("configured preparation owner")?;
        let reader = crate::Reader::new(self.catalog.clone(), root.clone())
            .with_runtime_workspace(runtime.workspace.clone(), owner);
        let output = reader.prepare_action(action)?;
        let receipt = Receipt {
            schema: 1,
            owner: owner.clone(),
            request_id: request_id.into(),
            action: action.into(),
            initial_revision: expected_revision.into(),
            output: output.clone(),
        };
        capture(&path, &encode(&receipt)?)?;
        let writes = OVERLAY
            .with(|v| v.borrow_mut().take())
            .context("preparation overlay missing")?
            .writes;
        drop(guard);
        #[cfg(test)]
        fault_boundary(0)?;
        crate::store::atomic_write_direct(
            &root.join(JOURNAL),
            &encode(&Journal { schema: 1, writes })?,
        )?;
        #[cfg(test)]
        fault_boundary(1)?;
        recover(&root)?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_real_preparation_reopens_once_at_each_commit_boundary() {
        for point in 0..3 {
            let temp = tempfile::tempdir().unwrap();
            let make_reader = || {
                crate::Reader::new(
                    crate::Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())]))
                        .unwrap(),
                    temp.path().join("reader"),
                )
                .with_runtime_workspace(temp.path().join("workspace"), "astrid")
            };
            let reader = make_reader();
            let revision = reader.preparation_revision().unwrap();
            FAIL_AT.with(|v| v.set(Some(point)));
            assert!(
                reader
                    .prepare_once(
                        "fixture-event",
                        &revision,
                        "SELF_STUDY QUESTION NEW Interruptible synthetic inquiry?"
                    )
                    .is_err()
            );
            drop(reader);
            let reopened = make_reader();
            let first = reopened
                .prepare_once(
                    "fixture-event",
                    &revision,
                    "SELF_STUDY QUESTION NEW Interruptible synthetic inquiry?",
                )
                .unwrap();
            let retry = reopened
                .prepare_once(
                    "fixture-event",
                    &revision,
                    "SELF_STUDY QUESTION NEW Interruptible synthetic inquiry?",
                )
                .unwrap();
            assert_eq!(
                serde_json::to_value(first).unwrap(),
                serde_json::to_value(retry).unwrap()
            );
            let saved: serde_json::Value = serde_json::from_slice(
                &fs::read(temp.path().join("reader/reader-v1.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(saved["questions"]["entries"].as_object().unwrap().len(), 1);
            assert!(!temp.path().join("reader").join(JOURNAL).exists());
        }
    }

    fn journal(root: &Path) {
        let writes = BTreeMap::from([
            (
                "a.json".into(),
                Change {
                    before: None,
                    after: b"first".to_vec(),
                },
            ),
            (
                "b.json".into(),
                Change {
                    before: Some(crate::digest(b"old")),
                    after: b"second".to_vec(),
                },
            ),
        ]);
        fs::write(root.join("b.json"), b"old").unwrap();
        fs::write(
            root.join(JOURNAL),
            encode(&Journal { schema: 1, writes }).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn owner_lock_recovers_durable_redo_before_any_writer() {
        for partial in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            journal(temp.path());
            if partial {
                fs::write(temp.path().join("a.json"), b"first").unwrap();
            }
            let _lock = crate::owner_transaction::OwnerTransaction::acquire(temp.path()).unwrap();
            assert_eq!(fs::read(temp.path().join("a.json")).unwrap(), b"first");
            assert_eq!(fs::read(temp.path().join("b.json")).unwrap(), b"second");
            assert!(!temp.path().join(JOURNAL).exists());
        }
    }

    #[test]
    fn newer_target_or_corrupt_redo_blocks_all_publication() {
        let temp = tempfile::tempdir().unwrap();
        journal(temp.path());
        fs::write(temp.path().join("b.json"), b"newer authored state").unwrap();
        let before = fs::read(temp.path().join(JOURNAL)).unwrap();
        assert!(crate::owner_transaction::OwnerTransaction::acquire(temp.path()).is_err());
        assert!(!temp.path().join("a.json").exists());
        assert_eq!(fs::read(temp.path().join(JOURNAL)).unwrap(), before);
        fs::write(temp.path().join(JOURNAL), b"{corrupt").unwrap();
        assert!(crate::owner_transaction::OwnerTransaction::acquire(temp.path()).is_err());
        assert_eq!(
            fs::read(temp.path().join("b.json")).unwrap(),
            b"newer authored state"
        );
    }

    #[test]
    fn failed_uncommitted_overlay_leaves_original_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = root.join("reader-v1.json");
        fs::write(&path, b"original").unwrap();
        OVERLAY.with(|v| {
            *v.borrow_mut() = Some(Overlay {
                root: root.clone(),
                ..Overlay::default()
            });
        });
        let guard = Guard;
        crate::store::atomic_write(&path, b"staged").unwrap();
        assert_eq!(read(&path).unwrap(), b"staged");
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(capture(&root.join("../escape"), b"no").is_err());
        drop(guard);
        assert_eq!(read(&path).unwrap(), b"original");
        assert!(!root.join(JOURNAL).exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_redo_and_targets_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        journal(temp.path());
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("target"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path().join("target"), temp.path().join("a.json"))
            .unwrap();
        assert!(recover(temp.path()).is_err());
        assert_eq!(fs::read(temp.path().join("b.json")).unwrap(), b"old");
        fs::remove_file(temp.path().join(JOURNAL)).unwrap();
        std::os::unix::fs::symlink(outside.path().join("missing"), temp.path().join(JOURNAL))
            .unwrap();
        assert!(recover(temp.path()).is_err());
    }
}
