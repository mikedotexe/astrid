//! Real OS signals in an owned child; no bridge runtime or live endpoints.
use super::*;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn child_fixture() {
    let Some(root) = std::env::var_os("ASTRID_LIFECYCLE_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut signals = Signals::install().unwrap();
        let status = Status::new(&root).unwrap();
        status.publish("running", None).unwrap();
        let request = signals.next().await;
        status.publish("draining", None).unwrap();
        let (retry_tx, mut retry_rx) = watch::channel(());
        let work = async {
            while !root.join("finish-admitted-work").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            atomic_private_write(&root.join("admitted-executions"), b"1").unwrap();
            let checkpoint = root.join("state.json");
            let mut publication =
                FrozenWrite::new(&checkpoint, b"{\"exchange_count\":7}".to_vec()).unwrap();
            retry_on_signal("fixture checkpoint", &mut retry_rx, || {
                if root.join("fail-checkpoint").exists() {
                    atomic_private_write(&root.join("checkpoint-blocked"), b"retained")?;
                    anyhow::bail!("injected checkpoint failure");
                }
                publication.publish().map_err(Into::into)
            })
            .await;
            let mut drained = status.prepare("drained", Some(&checkpoint)).unwrap();
            retry_on_signal("fixture drained status", &mut retry_rx, || {
                publication.publish()?;
                drained.publish().map_err(Into::into)
            })
            .await;
        };
        let (_, exit) = complete_drain(
            work,
            async || {
                let request = signals.next().await;
                retry_tx.send_replace(());
                request
            },
            request == Request::Exit,
        )
        .await;
        if !exit {
            while signals.next().await != Request::Exit {}
        }
    });
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Test fixture cleanup only, never the bridge process or deployment path.
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn signal(child: &OwnedChild, name: &str) {
    assert!(
        Command::new("/bin/kill")
            .args([name, &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
}

fn await_phase(root: &Path, pid: u32, expected: &str) {
    let start = Instant::now();
    loop {
        let phase = std::fs::read(root.join(format!("{pid}.json")))
            .ok()
            .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
            .and_then(|value| value["phase"].as_str().map(str::to_owned));
        if phase.as_deref() == Some(expected) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "missing phase {expected}: {phase:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn failed_checkpoint_retains_child_until_explicit_io_retry_without_reexecuting_work() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fail-checkpoint"), b"fixture").unwrap();
    std::fs::write(root.path().join("finish-admitted-work"), b"fixture").unwrap();
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "lifecycle::process_tests::child_fixture",
                "--nocapture",
            ])
            .env("ASTRID_LIFECYCLE_TEST_ROOT", root.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    await_phase(root.path(), pid, "running");
    signal(&child, "-USR1");
    let start = Instant::now();
    while !root.path().join("checkpoint-blocked").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    signal(&child, "-TERM");
    std::thread::sleep(Duration::from_millis(50));
    assert!(child.0.try_wait().unwrap().is_none());
    await_phase(root.path(), pid, "draining");
    assert!(!root.path().join("state.json").exists());
    std::fs::remove_file(root.path().join("fail-checkpoint")).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    assert!(!root.path().join("state.json").exists());
    signal(&child, "-USR1");
    await_phase(root.path(), pid, "drained");
    let start = Instant::now();
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::read(root.path().join("admitted-executions")).unwrap(),
        b"1"
    );
    assert_eq!(
        std::fs::read(root.path().join("state.json")).unwrap(),
        b"{\"exchange_count\":7}"
    );
}

#[test]
fn os_signal_drain_waits_for_work_then_holds_until_exit() {
    let root = tempfile::tempdir().unwrap();
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "lifecycle::process_tests::child_fixture",
                "--nocapture",
            ])
            .env("ASTRID_LIFECYCLE_TEST_ROOT", root.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    await_phase(root.path(), pid, "running");
    signal(&child, "-USR1");
    await_phase(root.path(), pid, "draining");
    signal(&child, "-USR1");
    std::thread::sleep(Duration::from_millis(50));
    assert!(child.0.try_wait().unwrap().is_none());
    assert!(!root.path().join("state.json").exists());
    std::fs::write(root.path().join("finish-admitted-work"), b"test release").unwrap();
    await_phase(root.path(), pid, "drained");
    assert!(child.0.try_wait().unwrap().is_none());
    signal(&child, "-TERM");
    let start = Instant::now();
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
}
