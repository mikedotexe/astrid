//! Existing advisory lifecycle behavior, isolated for failure-path qualification.
use astrid_capsule::capsule::{Capsule, ReadyStatus};
use std::{sync::Arc, time::Duration};

pub(super) async fn unload_for_restart(mut old: Arc<dyn Capsule>) {
    let id = old.id().clone();
    if let Some(capsule) = Arc::get_mut(&mut old) {
        if let Err(e) = capsule.unload().await {
            tracing::warn!(capsule_id = %id, error = %e, "Capsule unload failed during restart");
        }
    } else {
        tracing::warn!(capsule_id = %id,
            "Cannot call unload during restart - Arc still held by in-flight task");
    }
}

pub(super) async fn await_readiness(capsules: Vec<(String, Arc<dyn Capsule>)>) {
    let timeout = Duration::from_millis(500);
    let mut set = tokio::task::JoinSet::new();
    for (name, capsule) in capsules {
        set.spawn(async move { (name, capsule.wait_ready(timeout).await) });
    }
    while let Some(result) = set.join_next().await {
        if let Ok((name, status)) = result {
            match status {
                ReadyStatus::Ready => {},
                ReadyStatus::Timeout => tracing::warn!(capsule = %name,
                    timeout_ms = timeout.as_millis(), "Capsule did not signal ready within timeout"),
                ReadyStatus::Crashed => tracing::error!(capsule = %name,
                    "Capsule run loop exited before signaling ready"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrid_capsule::{
        capsule::{CapsuleId, CapsuleState},
        context::CapsuleContext,
        error::{CapsuleError, CapsuleResult},
        manifest::CapsuleManifest,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Stub {
        id: CapsuleId,
        manifest: CapsuleManifest,
        status: ReadyStatus,
        unloads: Arc<AtomicUsize>,
        unload_fails: bool,
    }
    fn stub(status: ReadyStatus, unload_fails: bool) -> (Arc<dyn Capsule>, Arc<AtomicUsize>) {
        let unloads = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(Stub {
                id: CapsuleId::from_static("test-lifecycle"),
                manifest: serde_json::from_value(serde_json::json!({
                    "package":{"name":"test-lifecycle","version":"0.0.1"}
                }))
                .unwrap(),
                status,
                unloads: Arc::clone(&unloads),
                unload_fails,
            }),
            unloads,
        )
    }
    #[async_trait::async_trait]
    impl Capsule for Stub {
        fn id(&self) -> &CapsuleId {
            &self.id
        }
        fn manifest(&self) -> &CapsuleManifest {
            &self.manifest
        }
        fn state(&self) -> CapsuleState {
            CapsuleState::Ready
        }
        async fn load(&mut self, _: &CapsuleContext) -> CapsuleResult<()> {
            Ok(())
        }
        async fn unload(&mut self) -> CapsuleResult<()> {
            self.unloads.fetch_add(1, Ordering::SeqCst);
            if self.unload_fails {
                Err(CapsuleError::NotSupported("synthetic failure".into()))
            } else {
                Ok(())
            }
        }
        async fn wait_ready(&self, timeout: Duration) -> ReadyStatus {
            assert_eq!(timeout, Duration::from_millis(500));
            self.status
        }
    }
    #[tokio::test]
    async fn timeout_and_crash_are_advisory_not_errors() {
        let capsules = [
            ReadyStatus::Ready,
            ReadyStatus::Timeout,
            ReadyStatus::Crashed,
        ]
        .into_iter()
        .map(|status| ("synthetic".into(), stub(status, false).0))
        .collect();
        await_readiness(capsules).await;
        // The production helper returns normally even without universal readiness.
    }
    #[tokio::test]
    async fn shared_owner_skips_unload_without_waiting_for_release() {
        let (capsule, unloads) = stub(ReadyStatus::Ready, false);
        let held = Arc::clone(&capsule);
        unload_for_restart(capsule).await;
        assert_eq!(unloads.load(Ordering::SeqCst), 0);
        assert_eq!(Arc::strong_count(&held), 1);
    }
    #[tokio::test]
    async fn exclusive_unload_failure_does_not_propagate() {
        for failure in [false, true] {
            let (capsule, unloads) = stub(ReadyStatus::Ready, failure);
            unload_for_restart(capsule).await;
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn missing_required_import_is_diagnostic_not_rejection() {
        let manifest: CapsuleManifest = serde_json::from_value(serde_json::json!({
            "package":{"name":"missing-consumer","version":"0.0.1"},
            "imports":{"example":{"missing":"1.0"}}
        }))
        .unwrap();
        assert!(
            manifest
                .import_tuples()
                .any(|(_, _, _, optional)| !optional)
        );
        crate::validate_imports_exports(&[(manifest, std::path::PathBuf::new())]);
    }
}
