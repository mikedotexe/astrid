//! Operator maintenance lifecycle. A drain is not a being-authored choice.
use std::future::Future;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::watch;

mod background;
pub use background::{
    spawn_background, spawn_background_thread, spawn_blocking_background, wait_background,
};
pub mod queued;

#[cfg(test)]
#[path = "lifecycle/process_tests.rs"]
mod process_tests;

/// Level-triggered cancellation also handles signals received during active work.
pub async fn stop_requested(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow_and_update() {
            return;
        }
        if stop.changed().await.is_err() {
            return;
        }
    }
}

/// Publish a complete private snapshot, never truncate the prior checkpoint.
pub fn atomic_private_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".checkpoint-{}-{:032x}",
        std::process::id(),
        rand::random::<u128>()
    ));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        std::fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Drain,
    Exit,
}

/// Install handlers before admitting work. USR1 drains and holds; TERM/INT exit
/// only after the same drain. Repeated signals never cancel an in-flight drain.
pub struct Signals {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
    drain: tokio::signal::unix::Signal,
}

impl Signals {
    pub fn install() -> std::io::Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
            drain: signal(SignalKind::user_defined1())?,
        })
    }

    pub async fn next(&mut self) -> Request {
        tokio::select! {
            _ = self.interrupt.recv() => Request::Exit,
            _ = self.terminate.recv() => Request::Exit,
            _ = self.drain.recv() => Request::Drain,
        }
    }
}

/// Keep ownership of the drain future even when another exit signal arrives.
pub async fn complete_drain<T>(
    work: impl Future<Output = T>,
    mut next_request: impl AsyncFnMut() -> Request,
    mut exit_requested: bool,
) -> (T, bool) {
    tokio::pin!(work);
    loop {
        tokio::select! {
            result = &mut work => return (result, exit_requested),
            request = next_request() => exit_requested |= request == Request::Exit,
        }
    }
}

/// This is a local lifecycle witness, not remote delivery or deployment authority.
pub struct Status {
    path: PathBuf,
    identity: serde_json::Value,
}

impl Status {
    pub fn new(directory: &Path) -> Result<Self> {
        let executable = std::env::current_exe()?.canonicalize()?;
        let mut file = std::fs::File::open(&executable)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let size = file.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            hasher.update(&buffer[..size]);
        }
        let pid = std::process::id();
        let status = Self {
            path: directory.join(format!("{pid}.json")),
            identity: json!({
                "schema": "bridge_operator_drain_v1",
                "pid": pid,
                "instance": format!("{:032x}", rand::random::<u128>()),
                "executable": executable,
                "executable_sha256": hex::encode(hasher.finalize()),
                "started_at_unix_ms": chrono::Utc::now().timestamp_millis(),
                "authority": "operator_maintenance_witness_only",
                "remote_delivery_confirmed": false,
                "automatic_resume": false,
            }),
        };
        status.publish("starting", None)?;
        Ok(status)
    }

    pub fn publish(&self, phase: &str, checkpoint: Option<&Path>) -> Result<()> {
        let mut value = self.identity.clone();
        value["phase"] = json!(phase);
        value["recorded_at_unix_ms"] = json!(chrono::Utc::now().timestamp_millis());
        if let Some(path) = checkpoint {
            let bytes = std::fs::read(path)?;
            let state: serde_json::Value = serde_json::from_slice(&bytes)?;
            value["checkpoint"] = json!({
                "path": path,
                "sha256": hex::encode(Sha256::digest(&bytes)),
                "exchange_count": state.get("exchange_count"),
            });
        }
        atomic_private_write(&self.path, &serde_json::to_vec_pretty(&value)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_is_observed_even_if_sent_before_wait() {
        let (tx, mut rx) = watch::channel(false);
        tx.send(true).unwrap();
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            stop_requested(&mut rx),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn repeated_exit_requests_do_not_drop_active_work() {
        let (work_tx, work_rx) = tokio::sync::oneshot::channel();
        let (request_tx, mut request_rx) = tokio::sync::mpsc::channel(8);
        let task = async move {
            complete_drain(
                async { work_rx.await.unwrap() },
                async || request_rx.recv().await.unwrap(),
                false,
            )
            .await
        };
        let driver = async move {
            request_tx.send(Request::Drain).await.unwrap();
            request_tx.send(Request::Exit).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            assert!(!work_tx.is_closed());
            work_tx.send("checkpoint complete").unwrap();
            std::future::pending::<()>().await;
        };
        tokio::pin!(driver);
        let result = tokio::select! {
            result = task => result,
            _ = &mut driver => panic!("driver must remain pending"),
        };
        assert_eq!(result, ("checkpoint complete", true));
    }

    #[test]
    fn atomic_checkpoint_replaces_complete_bytes_and_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        atomic_private_write(&path, b"old").unwrap();
        atomic_private_write(&path, b"new complete snapshot").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new complete snapshot");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_checkpoint_does_not_remove_prior_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("state.json");
        std::fs::create_dir(&target).unwrap();
        assert!(atomic_private_write(&target, b"cannot replace directory").is_err());
        assert!(target.is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
