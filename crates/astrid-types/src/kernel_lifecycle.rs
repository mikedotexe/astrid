//! Observed lifecycle outcomes, not admission decisions or current health proofs.
use serde::{Deserialize, Serialize};

/// Process-local observations. Older daemons omit this entire report.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapsuleLifecycleStatus {
    /// Latest completed discovery batch; absent means not observed.
    pub discovery: Option<CapsuleDiscoveryReport>,
    /// Most recent 64 restart attempts, oldest first. Pending attempts are visible.
    pub restarts: Vec<CapsuleRestartReport>,
    /// Count of older restart records no longer in this bounded status view.
    pub omitted_restarts: u64,
    /// Cumulative failed/skipped unloads this boot; later success does not clear debt.
    pub unacknowledged_unloads: u64,
    /// Latest shutdown attempts, when observed in this process.
    pub shutdown: Option<CapsuleShutdownReport>,
}

/// One completed discovery pass, not a continuously refreshed readiness assessment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleDiscoveryReport {
    /// Version of this observation contract.
    pub schema_version: u32,
    /// Kernel uptime when the batch completed.
    pub observed_at_uptime_ms: u64,
    /// Policy is advisory; these observations did not gate loading.
    pub advisory: bool,
    /// False for an empty batch or any unsuccessful load/readiness observation.
    pub all_reported_ready: bool,
    /// Every discovered name, including load failures and missing registry entries.
    pub capsules: Vec<CapsuleLoadObservation>,
}

/// Load return value and independently observed readiness for one batch entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleLoadObservation {
    /// Requested capsule name.
    pub name: String,
    /// Whether loading returned successfully; this is not readiness.
    pub load: CapsuleLoadOutcome,
    /// Result of the readiness method, or why no result was obtained.
    pub readiness: CapsuleReadinessOutcome,
}

/// Outcome of a load attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapsuleLoadOutcome {
    /// The load call returned successfully.
    Loaded,
    /// The load call returned an error.
    Failed,
    /// A replacement has not finished loading.
    Pending,
}

/// Advisory readiness outcomes. A ready return is not a lasting liveness proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapsuleReadinessOutcome {
    /// Capsule reported ready (non-run-loop implementations may return immediately).
    Ready,
    /// Capsule reported timeout; the existing method receives 500 ms.
    TimedOut,
    /// Capsule reported that its run loop exited before readiness.
    Crashed,
    /// Requested capsule was absent from the registry or its ID was invalid.
    Missing,
    /// Readiness task panicked or was cancelled, so its result is unknown.
    WaitFailed,
    /// No readiness wait is performed by this path.
    NotChecked,
}

/// Exact result of the existing unload method/ownership attempt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleUnloadObservation {
    /// Capsule being retired.
    pub name: String,
    /// Method return/ownership outcome, not a proof of process retirement.
    pub outcome: CapsuleUnloadOutcome,
    /// Number of ownership checks, including the final check.
    pub ownership_checks: u32,
    /// Strong references remaining when ownership could not be obtained.
    pub strong_references: usize,
    /// Weak references also prevent `Arc::get_mut` from granting exclusive access.
    pub weak_references: usize,
    /// Always false here: the Capsule trait offers no independent child-exit receipt.
    pub child_exit_verified: bool,
}

/// An acknowledgement is the method's return value, not inferred cleanup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapsuleUnloadOutcome {
    /// `unload()` returned Ok.
    Acknowledged,
    /// `unload()` returned an error.
    Failed,
    /// Exclusive ownership was unavailable; `unload()` was not called.
    OwnershipUnavailable,
}

/// One restart attempt; cleanup and replacement admission remain separate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleRestartReport {
    /// Version of this observation contract.
    pub schema_version: u32,
    /// Unique observation identity, not an authorization token.
    pub attempt_id: String,
    /// Kernel uptime when unload was attempted.
    pub observed_at_uptime_ms: u64,
    /// Exact cleanup outcome before replacement loading.
    pub cleanup: CapsuleUnloadObservation,
    /// Replacement loading result; pending is retained if work is interrupted.
    pub replacement: CapsuleLoadOutcome,
    /// This existing restart path does not wait for readiness.
    pub readiness: CapsuleReadinessOutcome,
}

/// Shutdown observations; socket removal cannot certify capsule retirement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapsuleShutdownReport {
    /// Version of this observation contract.
    pub schema_version: u32,
    /// Kernel uptime when capsule unload attempts finished.
    pub observed_at_uptime_ms: u64,
    /// Every capsule drained from the registry, including failed/skipped unloads.
    pub capsules: Vec<CapsuleUnloadObservation>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn older_status_deserializes_as_unavailable_not_ready() {
        let status: crate::kernel::DaemonStatus = serde_json::from_value(serde_json::json!({
            "pid":1,"uptime_secs":0,"version":"old","ephemeral":false,
            "connected_clients":0,"loaded_capsules":[]
        }))
        .unwrap();
        assert!(status.capsule_lifecycle.is_none());
        assert!(
            serde_json::to_value(status)
                .unwrap()
                .get("capsule_lifecycle")
                .is_none()
        );
    }
}
