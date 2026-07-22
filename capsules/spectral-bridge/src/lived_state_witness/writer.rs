use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};

use sha2::{Digest, Sha256};
use tracing::warn;

use super::{LivedStateGapReceiptV1, TemporalLivedStateWitnessV1, clock_sample_v1};

const WITNESS_QUEUE_CAPACITY: usize = 64;

#[derive(Debug)]
struct WitnessWriteJobV1 {
    root: PathBuf,
    witness: TemporalLivedStateWitnessV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WitnessSubmitResultV1 {
    Accepted,
    QueueFull,
    Disconnected,
}

struct WitnessWriterV1 {
    tx: SyncSender<WitnessWriteJobV1>,
}

fn ensure_owner_directory(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

pub(super) fn atomic_owner_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent")
    })?;
    ensure_owner_directory(parent)?;
    let tmp = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    let publish = fs::hard_link(&tmp, path);
    let result = match publish {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => match fs::read(path) {
            Ok(existing) if existing == bytes => {
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            },
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "immutable lived-state sidecar already exists with different content",
            )),
            Err(read_error) => Err(read_error),
        },
        Err(error) => Err(error),
    };
    let _ = fs::remove_file(&tmp);
    result?;
    let directory = OpenOptions::new().read(true).open(parent)?;
    directory.sync_all()?;
    Ok(())
}

fn write_gap(
    root: &Path,
    witness_id: &str,
    previous_witness_id: Option<&str>,
    reason: &str,
) -> std::io::Result<()> {
    ensure_owner_directory(root)?;
    let now = clock_sample_v1();
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{witness_id}\0{reason}\0{}", now.unix_ms).as_bytes())
    );
    let gap = LivedStateGapReceiptV1::new(
        format!("lsgap_{digest}"),
        witness_id.to_string(),
        previous_witness_id.map(str::to_string),
        reason.chars().take(160).collect(),
        now.unix_ms,
    );
    let mut encoded = serde_json::to_vec_pretty(&gap)?;
    encoded.push(b'\n');
    atomic_owner_write(
        &root.join("gaps").join(format!("{}.json", gap.gap_id())),
        &encoded,
    )
}

fn write_witness(job: &WitnessWriteJobV1) -> std::io::Result<()> {
    ensure_owner_directory(&job.root)?;
    let mut encoded = serde_json::to_vec_pretty(&job.witness)?;
    encoded.push(b'\n');
    atomic_owner_write(
        &job.root
            .join("witnesses")
            .join(format!("{}.json", job.witness.witness_id())),
        &encoded,
    )
}

impl WitnessWriterV1 {
    fn global() -> Option<&'static Self> {
        static WRITER: OnceLock<Option<WitnessWriterV1>> = OnceLock::new();
        WRITER
            .get_or_init(|| {
                let (tx, rx) = sync_channel::<WitnessWriteJobV1>(WITNESS_QUEUE_CAPACITY);
                let started = std::thread::Builder::new()
                    .name("lived-state-witness-writer".to_string())
                    .spawn(move || {
                        let mut previous_witness_by_root = HashMap::<PathBuf, String>::new();
                        while let Ok(job) = rx.recv() {
                            match write_witness(&job) {
                                Ok(()) => {
                                    previous_witness_by_root.insert(
                                        job.root.clone(),
                                        job.witness.witness_id().to_string(),
                                    );
                                },
                                Err(error) => {
                                    let error_sha256 = format!(
                                        "{:x}",
                                        Sha256::digest(error.to_string().as_bytes())
                                    );
                                    let reason =
                                        format!("sidecar_write_failed:sha256:{error_sha256}");
                                    let previous_witness_id =
                                        previous_witness_by_root.get(&job.root).map(String::as_str);
                                    if write_gap(
                                        &job.root,
                                        job.witness.witness_id(),
                                        previous_witness_id,
                                        &reason,
                                    )
                                    .is_err()
                                    {
                                        warn!(
                                            witness_id = job.witness.witness_id(),
                                            "lived-state witness and gap writes both failed"
                                        );
                                    }
                                },
                            }
                        }
                    });
                started.ok().map(|_| Self { tx })
            })
            .as_ref()
    }
}

pub(super) fn try_submit(
    root: &Path,
    witness: TemporalLivedStateWitnessV1,
) -> WitnessSubmitResultV1 {
    let job = WitnessWriteJobV1 {
        root: root.to_path_buf(),
        witness,
    };
    let Some(writer) = WitnessWriterV1::global() else {
        return WitnessSubmitResultV1::Disconnected;
    };
    match writer.tx.try_send(job) {
        Ok(()) => WitnessSubmitResultV1::Accepted,
        Err(TrySendError::Full(_)) => WitnessSubmitResultV1::QueueFull,
        Err(TrySendError::Disconnected(_)) => WitnessSubmitResultV1::Disconnected,
    }
}

#[cfg(test)]
pub(super) fn write_witness_for_test(
    root: &Path,
    witness: TemporalLivedStateWitnessV1,
) -> std::io::Result<()> {
    write_witness(&WitnessWriteJobV1 {
        root: root.to_path_buf(),
        witness,
    })
}

#[cfg(test)]
pub(super) fn bounded_submit_probe_for_test(
    root: &Path,
    witness: TemporalLivedStateWitnessV1,
    capacity: usize,
    attempts: usize,
) -> Vec<(WitnessSubmitResultV1, u128)> {
    let (tx, _rx) = sync_channel::<WitnessWriteJobV1>(capacity);
    let writer = WitnessWriterV1 { tx };
    (0..attempts)
        .map(|_| {
            let started = std::time::Instant::now();
            let result = match writer.tx.try_send(WitnessWriteJobV1 {
                root: root.to_path_buf(),
                witness: witness.clone(),
            }) {
                Ok(()) => WitnessSubmitResultV1::Accepted,
                Err(TrySendError::Full(_)) => WitnessSubmitResultV1::QueueFull,
                Err(TrySendError::Disconnected(_)) => WitnessSubmitResultV1::Disconnected,
            };
            (result, started.elapsed().as_nanos())
        })
        .collect()
}
