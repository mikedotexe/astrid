//! Observe existing advisory waits and best-effort unloads without new gates.
use astrid_capsule::capsule::{Capsule, ReadyStatus};
use astrid_events::kernel_api::{
    CapsuleReadinessOutcome as Ready, CapsuleUnloadObservation, CapsuleUnloadOutcome,
};
use std::{collections::HashMap, sync::Arc, time::Duration};

pub(super) async fn unload_for_restart(old: Arc<dyn Capsule>) -> CapsuleUnloadObservation {
    unload(old, 1).await
}

pub(super) async fn unload_for_shutdown(old: Arc<dyn Capsule>) -> CapsuleUnloadObservation {
    unload(old, 20).await
}

async fn unload(mut old: Arc<dyn Capsule>, checks: u32) -> CapsuleUnloadObservation {
    let mut observed = CapsuleUnloadObservation {
        name: old.id().to_string(),
        outcome: CapsuleUnloadOutcome::OwnershipUnavailable,
        ownership_checks: 0,
        strong_references: 0,
        weak_references: 0,
        child_exit_verified: false,
    };
    for check in 0..checks {
        observed.ownership_checks = check.saturating_add(1);
        if let Some(capsule) = Arc::get_mut(&mut old) {
            observed.outcome = match capsule.unload().await {
                Ok(()) => CapsuleUnloadOutcome::Acknowledged,
                Err(error) => {
                    tracing::warn!(capsule_id = %observed.name, %error, "Capsule unload failed");
                    CapsuleUnloadOutcome::Failed
                },
            };
            return observed;
        }
        if check.saturating_add(1) < checks {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    observed.strong_references = Arc::strong_count(&old);
    observed.weak_references = Arc::weak_count(&old);
    tracing::warn!(capsule_id = %observed.name, checks, strong = observed.strong_references,
        weak = observed.weak_references, "Capsule unload skipped: exclusive ownership unavailable");
    observed
}

pub(super) async fn await_readiness(
    capsules: Vec<(String, Option<Arc<dyn Capsule>>)>,
) -> Vec<(String, Ready)> {
    let timeout = Duration::from_millis(500);
    let mut set = tokio::task::JoinSet::new();
    let mut tasks = HashMap::new();
    let mut results = Vec::with_capacity(capsules.len());
    for (name, capsule) in capsules {
        let index = results.len();
        results.push((name, Ready::Missing));
        if let Some(capsule) = capsule {
            let task = set.spawn(async move { capsule.wait_ready(timeout).await });
            tasks.insert(task.id(), index);
        }
    }
    while let Some(result) = set.join_next_with_id().await {
        let (id, status) = match result {
            Ok((id, ReadyStatus::Ready)) => (id, Ready::Ready),
            Ok((id, ReadyStatus::Timeout)) => (id, Ready::TimedOut),
            Ok((id, ReadyStatus::Crashed)) => (id, Ready::Crashed),
            Err(error) => (error.id(), Ready::WaitFailed),
        };
        if let Some(index) = tasks.remove(&id) {
            results[index].1 = status;
            if status != Ready::Ready {
                tracing::warn!(capsule = %results[index].0, ?status, "Advisory capsule readiness outcome");
            }
        }
    }
    results
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
        panic_ready: bool,
    }
    fn stub(status: ReadyStatus, unload_fails: bool) -> (Arc<dyn Capsule>, Arc<AtomicUsize>) {
        stub_behavior(status, unload_fails, false)
    }
    fn stub_behavior(
        status: ReadyStatus,
        unload_fails: bool,
        panic_ready: bool,
    ) -> (Arc<dyn Capsule>, Arc<AtomicUsize>) {
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
                panic_ready,
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
            assert!(!self.panic_ready, "synthetic readiness panic");
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
        .map(|status| ("synthetic".into(), Some(stub(status, false).0)))
        .collect();
        let outcomes = await_readiness(capsules).await;
        assert_eq!(
            outcomes
                .iter()
                .map(|(_, status)| *status)
                .collect::<Vec<_>>(),
            [Ready::Ready, Ready::TimedOut, Ready::Crashed]
        );
    }
    #[tokio::test]
    async fn shared_owner_skips_unload_without_waiting_for_release() {
        let (capsule, unloads) = stub(ReadyStatus::Ready, false);
        let held = Arc::clone(&capsule);
        let observed = unload_for_restart(capsule).await;
        assert_eq!(observed.outcome, CapsuleUnloadOutcome::OwnershipUnavailable);
        assert_eq!(observed.ownership_checks, 1);
        assert_eq!(observed.strong_references, 2);
        assert!(!observed.child_exit_verified);
        assert_eq!(unloads.load(Ordering::SeqCst), 0);
        assert_eq!(Arc::strong_count(&held), 1);
    }
    #[tokio::test]
    async fn exclusive_unload_failure_does_not_propagate() {
        for failure in [false, true] {
            let (capsule, unloads) = stub(ReadyStatus::Ready, failure);
            let observed = unload_for_restart(capsule).await;
            assert_eq!(
                observed.outcome,
                if failure {
                    CapsuleUnloadOutcome::Failed
                } else {
                    CapsuleUnloadOutcome::Acknowledged
                }
            );
            assert!(!observed.child_exit_verified);
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
        }
    }
    #[tokio::test]
    async fn readiness_retains_missing_and_panicking_capsule_identities() {
        let outcomes = await_readiness(vec![
            ("absent".into(), None),
            (
                "panics".into(),
                Some(stub_behavior(ReadyStatus::Ready, false, true).0),
            ),
            ("ready".into(), Some(stub(ReadyStatus::Ready, false).0)),
        ])
        .await;
        assert_eq!(
            outcomes,
            vec![
                ("absent".into(), Ready::Missing),
                ("panics".into(), Ready::WaitFailed),
                ("ready".into(), Ready::Ready)
            ]
        );
    }
    #[tokio::test]
    async fn weak_ownership_is_not_misreported_as_an_acknowledged_unload() {
        let (capsule, calls) = stub(ReadyStatus::Ready, false);
        let weak = Arc::downgrade(&capsule);
        let observed = unload_for_restart(capsule).await;
        assert_eq!(observed.outcome, CapsuleUnloadOutcome::OwnershipUnavailable);
        assert_eq!(observed.strong_references, 1);
        assert_eq!(observed.weak_references, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(weak.upgrade().is_none());
    }
    #[tokio::test]
    async fn shutdown_retains_existing_retry_limit_and_reports_failure() {
        let (capsule, calls) = stub(ReadyStatus::Ready, false);
        let held = Arc::clone(&capsule);
        let observed = unload_for_shutdown(capsule).await;
        assert_eq!(observed.outcome, CapsuleUnloadOutcome::OwnershipUnavailable);
        assert_eq!(observed.ownership_checks, 20);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(held);
        let (capsule, calls) = stub(ReadyStatus::Ready, true);
        let observed = unload_for_shutdown(capsule).await;
        assert_eq!(observed.outcome, CapsuleUnloadOutcome::Failed);
        assert_eq!(observed.ownership_checks, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn shutdown_can_acknowledge_after_an_existing_owner_releases() {
        let (capsule, calls) = stub(ReadyStatus::Ready, false);
        let held = Arc::clone(&capsule);
        let release = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            drop(held);
        });
        let observed = unload_for_shutdown(capsule).await;
        release.await.unwrap();
        assert_eq!(observed.outcome, CapsuleUnloadOutcome::Acknowledged);
        assert!(observed.ownership_checks > 1 && observed.ownership_checks <= 20);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
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
