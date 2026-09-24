//! Bounded, process-local evidence. No observation changes restart/admission policy.
use astrid_events::kernel_api::{
    CapsuleDiscoveryReport, CapsuleLifecycleStatus, CapsuleLoadObservation, CapsuleLoadOutcome,
    CapsuleReadinessOutcome, CapsuleRestartReport, CapsuleShutdownReport, CapsuleUnloadOutcome,
};

impl super::Kernel {
    pub(super) async fn load_discovery_group(
        &self,
        group: &[(
            astrid_capsule::manifest::CapsuleManifest,
            std::path::PathBuf,
        )],
    ) -> Vec<CapsuleLoadObservation> {
        let mut outcomes = Vec::with_capacity(group.len());
        for (manifest, dir) in group {
            let result = self.load_capsule(dir.clone()).await;
            if let Err(error) = &result {
                tracing::warn!(capsule = %manifest.package.name, %error, "Failed to load capsule during discovery");
            }
            outcomes.push(CapsuleLoadObservation {
                name: manifest.package.name.clone(),
                load: if result.is_ok() {
                    CapsuleLoadOutcome::Loaded
                } else {
                    CapsuleLoadOutcome::Failed
                },
                readiness: CapsuleReadinessOutcome::NotChecked,
            });
        }
        let capsules = {
            let registry = self.capsules.read().await;
            outcomes
                .iter()
                .map(|entry| {
                    let capsule = astrid_capsule::capsule::CapsuleId::new(entry.name.clone())
                        .ok()
                        .and_then(|id| registry.get(&id));
                    (entry.name.clone(), capsule)
                })
                .collect()
        };
        let readiness = super::lifecycle_checks::await_readiness(capsules).await;
        for (entry, (_, ready)) in outcomes.iter_mut().zip(readiness) {
            entry.readiness = ready;
        }
        outcomes
    }

    pub(super) fn lifecycle_uptime_ms(&self) -> u64 {
        u64::try_from(self.boot_time.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(super) async fn observe_discovery(&self, report: CapsuleDiscoveryReport) {
        self.lifecycle.write().await.discovery = Some(report.clone());
        self.publish_lifecycle("discovery", &report);
    }

    pub(super) async fn observe_restart(&self, report: &CapsuleRestartReport) {
        let mut state = self.lifecycle.write().await;
        append_restart(&mut state, report);
        drop(state);
        self.publish_lifecycle("restart", report);
    }

    pub(super) async fn observe_shutdown(&self, report: CapsuleShutdownReport) {
        let mut state = self.lifecycle.write().await;
        for capsule in &report.capsules {
            if capsule.outcome != CapsuleUnloadOutcome::Acknowledged {
                state.unacknowledged_unloads = state.unacknowledged_unloads.saturating_add(1);
            }
        }
        state.shutdown = Some(report.clone());
        drop(state);
        self.publish_lifecycle("shutdown_unloads", &report);
    }

    fn publish_lifecycle(&self, kind: &str, report: &impl serde::Serialize) {
        let Ok(value) = serde_json::to_value(report) else {
            return;
        };
        // Status is process-local evidence; IPC and configured logs are best-effort.
        tracing::info!(kind, report = %value, "Kernel lifecycle observation");
        let message = astrid_events::ipc::IpcMessage::new(
            "astrid.v1.lifecycle.observed",
            astrid_events::ipc::IpcPayload::RawJson(
                serde_json::json!({"kind": kind, "report": value}),
            ),
            self.session_id.0,
        );
        let _ = self.event_bus.publish(astrid_events::AstridEvent::Ipc {
            metadata: astrid_events::EventMetadata::new("kernel"),
            message,
        });
    }
}

pub(super) fn discovery_report(
    capsules: Vec<CapsuleLoadObservation>,
    now_ms: u64,
) -> CapsuleDiscoveryReport {
    CapsuleDiscoveryReport {
        schema_version: 1,
        observed_at_uptime_ms: now_ms,
        advisory: true,
        all_reported_ready: !capsules.is_empty()
            && capsules.iter().all(|c| {
                c.load == CapsuleLoadOutcome::Loaded
                    && c.readiness == CapsuleReadinessOutcome::Ready
            }),
        capsules,
    }
}

fn append_restart(state: &mut CapsuleLifecycleStatus, report: &CapsuleRestartReport) {
    if let Some(previous) = state
        .restarts
        .iter_mut()
        .find(|r| r.attempt_id == report.attempt_id)
    {
        if previous.replacement == CapsuleLoadOutcome::Pending {
            *previous = report.clone();
        }
        return;
    }
    // A late completion of an already omitted attempt must not count its debt twice.
    if report.replacement != CapsuleLoadOutcome::Pending {
        return;
    }
    if report.cleanup.outcome != CapsuleUnloadOutcome::Acknowledged {
        state.unacknowledged_unloads = state.unacknowledged_unloads.saturating_add(1);
    }
    if state.restarts.len() == 64 {
        state.restarts.remove(0);
        state.omitted_restarts = state.omitted_restarts.saturating_add(1);
    }
    state.restarts.push(report.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrid_events::kernel_api::CapsuleUnloadObservation;

    fn restart(id: usize, outcome: CapsuleUnloadOutcome) -> CapsuleRestartReport {
        CapsuleRestartReport {
            schema_version: 1,
            attempt_id: id.to_string(),
            observed_at_uptime_ms: 1,
            cleanup: CapsuleUnloadObservation {
                name: "synthetic".into(),
                outcome,
                ownership_checks: 1,
                strong_references: 2,
                weak_references: 0,
                child_exit_verified: false,
            },
            replacement: CapsuleLoadOutcome::Pending,
            readiness: CapsuleReadinessOutcome::NotChecked,
        }
    }

    #[test]
    fn load_and_readiness_remain_independent_and_empty_is_not_positive_evidence() {
        assert!(!discovery_report(Vec::new(), 2).all_reported_ready);
        for (load, readiness) in [
            (
                CapsuleLoadOutcome::Loaded,
                CapsuleReadinessOutcome::TimedOut,
            ),
            (CapsuleLoadOutcome::Failed, CapsuleReadinessOutcome::Ready),
            (CapsuleLoadOutcome::Failed, CapsuleReadinessOutcome::Missing),
        ] {
            let report = discovery_report(
                vec![CapsuleLoadObservation {
                    name: "x".into(),
                    load,
                    readiness,
                }],
                2,
            );
            assert!(!report.all_reported_ready);
            assert!(report.advisory);
            assert_eq!(report.capsules[0].load, load);
        }
        assert!(
            discovery_report(
                vec![CapsuleLoadObservation {
                    name: "x".into(),
                    load: CapsuleLoadOutcome::Loaded,
                    readiness: CapsuleReadinessOutcome::Ready,
                }],
                2
            )
            .all_reported_ready
        );
    }

    #[test]
    fn completion_retains_cleanup_debt_and_eviction_is_explicit() {
        let mut state = CapsuleLifecycleStatus::default();
        for id in 0..65 {
            let pending = restart(id, CapsuleUnloadOutcome::OwnershipUnavailable);
            append_restart(&mut state, &pending);
            append_restart(&mut state, &pending);
            let mut completed = pending;
            completed.replacement = CapsuleLoadOutcome::Loaded;
            append_restart(&mut state, &completed);
        }
        assert_eq!(state.restarts.len(), 64);
        assert_eq!(state.omitted_restarts, 1);
        assert_eq!(state.unacknowledged_unloads, 65);
        assert!(
            state
                .restarts
                .iter()
                .all(|r| r.replacement == CapsuleLoadOutcome::Loaded
                    && r.cleanup.outcome == CapsuleUnloadOutcome::OwnershipUnavailable)
        );
        let mut late = restart(0, CapsuleUnloadOutcome::OwnershipUnavailable);
        late.replacement = CapsuleLoadOutcome::Failed;
        append_restart(&mut state, &late);
        assert_eq!(state.unacknowledged_unloads, 65);
        assert_eq!(state.omitted_restarts, 1);
        append_restart(&mut state, &restart(66, CapsuleUnloadOutcome::Acknowledged));
        assert_eq!(state.unacknowledged_unloads, 65);
    }
}
