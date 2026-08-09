//! Fail-closed edge-runtime drain acknowledgement for immutable A/B updates
//! and separately admitted scheduled reflection.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::config::Config;

const TRANSITION_LEASE_SCHEMA: &str = "astrid.edge_self_change.maintenance_lease.v2";
const REFLECTION_LEASE_SCHEMA: &str = "astrid.edge_scheduled_reflection.lease.v1";
const ACK_SCHEMA: &str = "astrid.edge.maintenance_ack.v2";
const ACK_AUTHORITY: &str = "mutable_runtime_acknowledgement_subject_to_immutable_verification";
const MAXIMUM_LEASE_BYTES: u64 = 8 * 1_024;
const MAXIMUM_AUTONOMY_BYTES: u64 = 2 * 1_024 * 1_024;
const MAXIMUM_LEDGER_BYTES: u64 = 64 * 1_024 * 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "lease_id is the canonical signed maintenance protocol field"
)]
struct TransitionLease {
    schema: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReflectionLease {
    schema: String,
    lease_kind: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
    host_boot_id: String,
    service_invocation_id: String,
    generation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaseKind {
    GenerationTransition,
    ScheduledReflection,
}

impl LeaseKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::GenerationTransition => "generation_transition",
            Self::ScheduledReflection => "scheduled_reflection",
        }
    }

    const fn schema(self) -> &'static str {
        match self {
            Self::GenerationTransition => TRANSITION_LEASE_SCHEMA,
            Self::ScheduledReflection => REFLECTION_LEASE_SCHEMA,
        }
    }
}

#[derive(Debug)]
struct BoundLease {
    kind: LeaseKind,
    lease_id: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    generation_id: Option<String>,
    payload_sha256: String,
    nonce_sha256: String,
}

#[derive(Debug, Serialize)]
struct EdgeAck<'a> {
    schema: &'static str,
    role: &'static str,
    lease_schema: &'static str,
    lease_kind: &'static str,
    lease_id: &'a str,
    lease_nonce_sha256: &'a str,
    lease_payload_sha256: &'a str,
    generation_id: &'a str,
    blocked_since_unix_ms: u64,
    acknowledged_at_unix_ms: u64,
    pid: u64,
    process_start_ticks: u64,
    authority: &'static str,
    new_work_blocked: bool,
    drain_barrier_sequence: u64,
    ipc_sequence_exact: bool,
    scheduled_work_count: u64,
    action_work_count: u64,
    continuation_work_count: u64,
    indexes: EdgeIndexes,
}

#[derive(Debug, Serialize)]
struct EdgeIndexes {
    autonomy: AutonomyIndex,
    ledgers: LedgerIndexes,
}

#[derive(Debug, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the exact durable scheduler index mirrors independent v3 state flags"
)]
struct AutonomyIndex {
    path: &'static str,
    sha256: String,
    size_bytes: u64,
    action_dispatch_pending: bool,
    run_receipt_pending: bool,
    chain_receipt_pending: bool,
    thread_projection_pending: bool,
}

#[derive(Debug, Serialize)]
struct LedgerIndexes {
    actions: LedgerIndex,
    web: LedgerIndex,
    introspection: LedgerIndex,
}

#[derive(Debug, Clone, Serialize)]
struct LedgerIndex {
    path: &'static str,
    inode: u64,
    size_bytes: u64,
    sha256: String,
    pending_count: u64,
}

enum LeaseState {
    Absent,
    Expired,
    Active(BoundLease),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkKind {
    Scheduled,
    Action,
    Continuation,
}

#[derive(Debug, Default)]
struct WorkState {
    ever_authenticated: bool,
    ipc_sequence_exact: bool,
    ipc_sequence_poisoned: bool,
    last_barrier_sequence: Option<u64>,
    barrier_lease_schema: Option<String>,
    barrier_lease_kind: Option<String>,
    barrier_lease_id: Option<String>,
    barrier_lease_payload_sha256: Option<String>,
    scheduled_work_count: usize,
    action_work_count: usize,
    continuation_work_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DrainSnapshot {
    drain_barrier_sequence: u64,
    ipc_sequence_exact: bool,
    scheduled_work_count: u64,
    action_work_count: u64,
    continuation_work_count: u64,
}

/// Process-local exactness and work accounting for one edge-runtime epoch.
/// A socket reconnect or lag permanently poisons update eligibility until the
/// runtime restarts; it never affects ordinary sensing or autonomy authority.
#[derive(Debug, Default)]
pub(crate) struct WorkTracker {
    state: Mutex<WorkState>,
    acknowledgement: Option<std::path::PathBuf>,
    lease_probe: Option<LeaseProbe>,
}

#[derive(Debug, Clone)]
pub(crate) struct LeaseProbe {
    transition: std::path::PathBuf,
    reflection: std::path::PathBuf,
    runtime_gid: u32,
}

#[derive(Debug)]
pub(crate) struct WorkPermit {
    tracker: Arc<WorkTracker>,
    kind: WorkKind,
    released: bool,
}

impl WorkTracker {
    #[must_use]
    pub(crate) fn new(
        acknowledgement: Option<std::path::PathBuf>,
        lease_probe: Option<LeaseProbe>,
    ) -> Self {
        Self {
            state: Mutex::new(WorkState::default()),
            acknowledgement,
            lease_probe,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, WorkState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.ipc_sequence_exact = false;
                state.ipc_sequence_poisoned = true;
                state.last_barrier_sequence = None;
                state.barrier_lease_schema = None;
                state.barrier_lease_kind = None;
                state.barrier_lease_id = None;
                state.barrier_lease_payload_sha256 = None;
                state
            },
        }
    }

    pub(crate) fn ipc_authenticated(&self) {
        let mut state = self.lock();
        if state.ever_authenticated {
            poison_ipc_state(&mut state);
            return;
        }
        state.ever_authenticated = true;
        state.ipc_sequence_exact = !state.ipc_sequence_poisoned;
    }

    pub(crate) fn ipc_disconnected(&self) {
        let mut state = self.lock();
        if state.ever_authenticated {
            poison_ipc_state(&mut state);
        }
    }

    pub(crate) fn reject_ipc_sequence(&self) {
        poison_ipc_state(&mut self.lock());
    }

    pub(crate) fn observe_barrier(
        &self,
        sequence: u64,
        lease_schema: &str,
        lease_kind: &str,
        lease_id: &str,
        lease_payload_sha256: &str,
    ) {
        let mut state = self.lock();
        if !state.ipc_sequence_exact
            || state.ipc_sequence_poisoned
            || sequence == 0
            || state
                .last_barrier_sequence
                .is_some_and(|previous| sequence <= previous)
        {
            poison_ipc_state(&mut state);
            return;
        }
        state.last_barrier_sequence = Some(sequence);
        state.barrier_lease_schema = Some(lease_schema.to_owned());
        state.barrier_lease_kind = Some(lease_kind.to_owned());
        state.barrier_lease_id = Some(lease_id.to_owned());
        state.barrier_lease_payload_sha256 = Some(lease_payload_sha256.to_owned());
    }

    pub(crate) fn begin_action(self: &Arc<Self>) -> anyhow::Result<WorkPermit> {
        self.begin(WorkKind::Action)
    }

    pub(crate) fn begin_scheduled(self: &Arc<Self>) -> anyhow::Result<WorkPermit> {
        self.begin(WorkKind::Scheduled)
    }

    /// Count an ordinary autonomous inference transaction in the same exact
    /// scheduled-work class as a dedicated reflection.  The immutable ACK
    /// schema deliberately exposes one aggregate: either kind must be fully
    /// drained before a generation switch can proceed.
    pub(crate) fn begin_model_turn(self: &Arc<Self>) -> anyhow::Result<WorkPermit> {
        self.begin(WorkKind::Scheduled)
    }

    pub(crate) fn begin_continuation(self: &Arc<Self>) -> anyhow::Result<WorkPermit> {
        self.begin(WorkKind::Continuation)
    }

    fn begin(self: &Arc<Self>, kind: WorkKind) -> anyhow::Result<WorkPermit> {
        let mut state = self.lock();
        if self
            .lease_probe
            .as_ref()
            .is_some_and(LeaseProbe::blocks_new_work)
        {
            anyhow::bail!("root maintenance lease blocks new edge work");
        }
        let count = match kind {
            WorkKind::Scheduled => &mut state.scheduled_work_count,
            WorkKind::Action => &mut state.action_work_count,
            WorkKind::Continuation => &mut state.continuation_work_count,
        };
        let Some(next) = count.checked_add(1) else {
            poison_ipc_state(&mut state);
            anyhow::bail!("maintenance work counter overflowed");
        };
        *count = next;
        // Any locally admitted work after a barrier requires a newer barrier.
        state.last_barrier_sequence = None;
        state.barrier_lease_schema = None;
        state.barrier_lease_kind = None;
        state.barrier_lease_id = None;
        state.barrier_lease_payload_sha256 = None;
        if let Some(path) = self.acknowledgement.as_deref()
            && let Err(error) = remove_ack_for_new_work(path)
        {
            poison_ipc_state(&mut state);
            anyhow::bail!("cannot invalidate stale maintenance ACK: {error:#}");
        }
        Ok(WorkPermit {
            tracker: Arc::clone(self),
            kind,
            released: false,
        })
    }

    fn with_drain_snapshot<T>(
        &self,
        bound: &BoundLease,
        operation: impl FnOnce(DrainSnapshot) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let state = self.lock();
        anyhow::ensure!(
            state.ipc_sequence_exact && !state.ipc_sequence_poisoned,
            "IPC observation epoch is not exact"
        );
        anyhow::ensure!(
            state.barrier_lease_schema.as_deref() == Some(bound.kind.schema())
                && state.barrier_lease_kind.as_deref() == Some(bound.kind.as_str())
                && state.barrier_lease_id.as_deref() == Some(bound.lease_id.as_str())
                && state.barrier_lease_payload_sha256.as_deref()
                    == Some(bound.payload_sha256.as_str()),
            "no exact kernel barrier is bound to the active lease"
        );
        let drain_barrier_sequence = state
            .last_barrier_sequence
            .context("kernel drain barrier sequence is absent")?;
        let scheduled_work_count = u64::try_from(state.scheduled_work_count)?;
        let action_work_count = u64::try_from(state.action_work_count)?;
        let continuation_work_count = u64::try_from(state.continuation_work_count)?;
        anyhow::ensure!(
            scheduled_work_count == 0 && action_work_count == 0 && continuation_work_count == 0,
            "edge-local work has not drained"
        );
        let snapshot = DrainSnapshot {
            drain_barrier_sequence,
            ipc_sequence_exact: true,
            scheduled_work_count,
            action_work_count,
            continuation_work_count,
        };
        // Keep the admission mutex held through the caller's exact file scans
        // and ACK write. A new permit can begin only after that write, and its
        // first operation synchronously removes the just-written ACK.
        operation(snapshot)
    }

    #[cfg(test)]
    fn drain_snapshot(&self, bound: &BoundLease) -> anyhow::Result<DrainSnapshot> {
        self.with_drain_snapshot(bound, Ok)
    }

    #[cfg(test)]
    pub(crate) fn work_counts(&self) -> (u64, u64, u64) {
        let state = self.lock();
        (
            u64::try_from(state.scheduled_work_count).unwrap_or(u64::MAX),
            u64::try_from(state.action_work_count).unwrap_or(u64::MAX),
            u64::try_from(state.continuation_work_count).unwrap_or(u64::MAX),
        )
    }
}

impl LeaseProbe {
    pub(crate) fn from_config(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            transition: config.maintenance_lease_path.clone(),
            reflection: config.reflection_lease_path.clone(),
            runtime_gid: fs::symlink_metadata(&config.workspace)?.gid(),
        })
    }

    fn blocks_new_work(&self) -> bool {
        !matches!(
            active_bound_lease_at(
                &self.transition,
                &self.reflection,
                self.runtime_gid,
                unix_millis(),
            ),
            Ok(None)
        )
    }
}

impl Drop for WorkPermit {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let mut state = self.tracker.lock();
        let count = match self.kind {
            WorkKind::Scheduled => &mut state.scheduled_work_count,
            WorkKind::Action => &mut state.action_work_count,
            WorkKind::Continuation => &mut state.continuation_work_count,
        };
        if let Some(next) = count.checked_sub(1) {
            *count = next;
        } else {
            poison_ipc_state(&mut state);
        }
        self.released = true;
    }
}

fn poison_ipc_state(state: &mut WorkState) {
    state.ipc_sequence_exact = false;
    state.ipc_sequence_poisoned = true;
    state.last_barrier_sequence = None;
    state.barrier_lease_schema = None;
    state.barrier_lease_kind = None;
    state.barrier_lease_id = None;
    state.barrier_lease_payload_sha256 = None;
}

/// Return whether a root lease blocks new mutable-runtime work. Every present
/// malformed or unreadable object fails closed. A structurally valid expired
/// lease stops blocking but can only be removed by the immutable helper.
pub(crate) fn lease_blocks_new_work(config: &Config) -> bool {
    !matches!(active_bound_lease(config, unix_millis()), Ok(None))
}

#[cfg(test)]
pub(crate) fn lease_payload_blocks_new_work(value: &Value, now: u64) -> bool {
    let Ok(lease) = serde_json::from_value::<TransitionLease>(value.clone()) else {
        return true;
    };
    validate_transition_lease(&lease, now).is_err() || lease.expires_at_unix_ms > now
}

/// Maintain a fresh, exact edge drain acknowledgement while a valid root lease
/// is active. The immutable helper independently re-parses every indexed file.
pub(crate) async fn run(config: Arc<Config>, work: Arc<WorkTracker>) -> anyhow::Result<()> {
    let acknowledgement = config
        .maintenance_edge_ack_path
        .as_deref()
        .context("maintenance edge ACK path is not configured")?;
    let generation_binding = config
        .generation_binding_path
        .as_deref()
        .context("generation binding path is not configured")?;
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut active_lease_hash = None;
    let mut blocked_since_unix_ms = None;
    let mut last_acknowledged_at = 0_u64;

    loop {
        interval.tick().await;
        let now = unix_millis();
        match active_bound_lease(&config, now) {
            Ok(Some(bound)) => {
                if active_lease_hash.as_deref() != Some(bound.payload_sha256.as_str()) {
                    active_lease_hash = Some(bound.payload_sha256.clone());
                    blocked_since_unix_ms = Some(now);
                    last_acknowledged_at = 0;
                    remove_ack(acknowledgement);
                }
                if last_acknowledged_at == 0 || now.saturating_sub(last_acknowledged_at) >= 10_000 {
                    let result = work.with_drain_snapshot(&bound, |drain| {
                        exact_indexes(&config).and_then(|indexes| {
                            write_ack(
                                acknowledgement,
                                generation_binding,
                                &bound,
                                blocked_since_unix_ms.unwrap_or(now),
                                now,
                                drain,
                                indexes,
                            )
                        })
                    });
                    match result {
                        Ok(()) => last_acknowledged_at = now,
                        Err(error) => {
                            remove_ack(acknowledgement);
                            last_acknowledged_at = 0;
                            eprintln!("edge maintenance acknowledgement deferred: {error:#}");
                        },
                    }
                }
            },
            Ok(None) => {
                active_lease_hash = None;
                blocked_since_unix_ms = None;
                last_acknowledged_at = 0;
                remove_ack(acknowledgement);
            },
            Err(error) => {
                // Present malformed state is a fail-closed block but can never
                // yield an authority-looking acknowledgement.
                remove_ack(acknowledgement);
                last_acknowledged_at = 0;
                eprintln!("immutable maintenance lease rejected: {error:#}");
            },
        }
    }
}

fn active_bound_lease(config: &Config, now: u64) -> anyhow::Result<Option<BoundLease>> {
    let runtime_gid = fs::symlink_metadata(&config.workspace)?.gid();
    active_bound_lease_at(
        &config.maintenance_lease_path,
        &config.reflection_lease_path,
        runtime_gid,
        now,
    )
}

fn active_bound_lease_at(
    transition_path: &Path,
    reflection_path: &Path,
    runtime_gid: u32,
    now: u64,
) -> anyhow::Result<Option<BoundLease>> {
    let transition = read_bound_lease(transition_path, LeaseKind::GenerationTransition, None, now)?;
    let reflection = read_bound_lease(
        reflection_path,
        LeaseKind::ScheduledReflection,
        Some(runtime_gid),
        now,
    )?;
    match (transition, reflection) {
        (LeaseState::Active(_), LeaseState::Active(_)) => {
            anyhow::bail!("generation transition and scheduled reflection leases overlap")
        },
        (LeaseState::Active(bound), LeaseState::Absent | LeaseState::Expired)
        | (LeaseState::Absent | LeaseState::Expired, LeaseState::Active(bound)) => Ok(Some(bound)),
        (LeaseState::Absent | LeaseState::Expired, LeaseState::Absent | LeaseState::Expired) => {
            Ok(None)
        },
    }
}

fn read_bound_lease(
    path: &Path,
    kind: LeaseKind,
    required_gid: Option<u32>,
    now: u64,
) -> anyhow::Result<LeaseState> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LeaseState::Absent);
        },
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || match kind {
            LeaseKind::GenerationTransition => metadata.permissions().mode() & 0o777 != 0o444,
            LeaseKind::ScheduledReflection => {
                metadata.permissions().mode() & 0o777 != 0o440
                    || required_gid != Some(metadata.gid())
            },
        }
        || metadata.len() > MAXIMUM_LEASE_BYTES
    {
        anyhow::bail!("maintenance lease identity or mode failed");
    }
    let bytes = stable_read(path, MAXIMUM_LEASE_BYTES)?.0;
    let payload_sha256 = sha256(&bytes);
    match kind {
        LeaseKind::GenerationTransition => {
            let lease: TransitionLease = serde_json::from_slice(&bytes)?;
            let nonce_sha256 = validate_transition_lease(&lease, now)?;
            if lease.expires_at_unix_ms <= now {
                return Ok(LeaseState::Expired);
            }
            Ok(LeaseState::Active(BoundLease {
                kind,
                lease_id: lease.lease_id,
                created_at_unix_ms: lease.created_at_unix_ms,
                expires_at_unix_ms: lease.expires_at_unix_ms,
                generation_id: None,
                payload_sha256,
                nonce_sha256,
            }))
        },
        LeaseKind::ScheduledReflection => {
            let lease: ReflectionLease = serde_json::from_slice(&bytes)?;
            let nonce_sha256 = validate_reflection_lease(&lease, now)?;
            if lease.expires_at_unix_ms <= now {
                return Ok(LeaseState::Expired);
            }
            Ok(LeaseState::Active(BoundLease {
                kind,
                lease_id: lease.lease_id,
                created_at_unix_ms: lease.created_at_unix_ms,
                expires_at_unix_ms: lease.expires_at_unix_ms,
                generation_id: Some(lease.generation_id),
                payload_sha256,
                nonce_sha256,
            }))
        },
    }
}

fn validate_transition_lease(lease: &TransitionLease, now: u64) -> anyhow::Result<String> {
    if lease.schema != TRANSITION_LEASE_SCHEMA
        || lease.owner != "immutable_astrid_edge_rescue_helper"
        || lease.reason.is_empty()
        || lease.reason.chars().count() > 128
        || lease.reason.chars().any(char::is_control)
        || lease.created_at_unix_ms > now.saturating_add(30_000)
        || lease.expires_at_unix_ms <= lease.created_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            > 48 * 60 * 60 * 1_000
        || !is_lower_hex(&lease.nonce, 64)
    {
        anyhow::bail!("maintenance lease content escaped bounds");
    }
    let nonce_sha256 = sha256(lease.nonce.as_bytes());
    if lease.lease_id != format!("lease-{}", &nonce_sha256[..24]) {
        anyhow::bail!("maintenance lease ID does not bind its nonce");
    }
    Ok(nonce_sha256)
}

fn validate_reflection_lease(lease: &ReflectionLease, now: u64) -> anyhow::Result<String> {
    if lease.schema != REFLECTION_LEASE_SCHEMA
        || lease.lease_kind != LeaseKind::ScheduledReflection.as_str()
        || lease.owner != "immutable_astrid_edge_reflection_guard"
        || lease.reason != "scheduled_reflection"
        || lease.created_at_unix_ms > now.saturating_add(30_000)
        || lease.expires_at_unix_ms <= lease.created_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            != 3 * 60 * 60 * 1_000
        || !is_lower_hex(&lease.nonce, 64)
        || !is_lower_hex(&lease.service_invocation_id, 32)
        || !valid_boot_id(&lease.host_boot_id)
        || !valid_identifier(&lease.generation_id)
        || current_boot_id().as_deref() != Some(lease.host_boot_id.as_str())
    {
        anyhow::bail!("scheduled-reflection lease content escaped bounds");
    }
    let nonce_sha256 = sha256(lease.nonce.as_bytes());
    if lease.lease_id != format!("reflection-{}", &nonce_sha256[..24]) {
        anyhow::bail!("scheduled-reflection lease ID does not bind its nonce");
    }
    Ok(nonce_sha256)
}

fn exact_indexes(config: &Config) -> anyhow::Result<EdgeIndexes> {
    Ok(EdgeIndexes {
        autonomy: autonomy_index(&config.workspace.join("autonomous/state.json"))?,
        ledgers: LedgerIndexes {
            actions: ledger_index(
                &config.workspace.join("actions/receipts.jsonl"),
                "actions",
                "actions/receipts.jsonl",
            )?,
            web: ledger_index(
                &config.workspace.join("web/receipts.jsonl"),
                "web",
                "web/receipts.jsonl",
            )?,
            introspection: ledger_index(
                &config.workspace.join("introspection/receipts.jsonl"),
                "introspection",
                "introspection/receipts.jsonl",
            )?,
        },
    })
}

fn autonomy_index(path: &Path) -> anyhow::Result<AutonomyIndex> {
    let (bytes, _) = stable_read(path, MAXIMUM_AUTONOMY_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let action_dispatch_pending = exact_bool(&value, "action_dispatch_pending")?;
    let run_receipt_pending = exact_bool(&value, "run_receipt_pending")?;
    let chain_receipt_pending = exact_bool(&value, "chain_receipt_pending")?;
    let thread_projection_pending = value
        .get("thread_projection_pending")
        .is_some_and(|pending| !pending.is_null());
    if value.get("schema").and_then(Value::as_str) != Some("astrid_edge_autonomy_state_v3")
        || value.get("last_status").and_then(Value::as_str) == Some("running")
        || action_dispatch_pending
        || run_receipt_pending
        || chain_receipt_pending
        || thread_projection_pending
    {
        anyhow::bail!("autonomy state does not prove an exact drained v3 boundary");
    }
    Ok(AutonomyIndex {
        path: "autonomous/state.json",
        sha256: sha256(&bytes),
        size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        action_dispatch_pending,
        run_receipt_pending,
        chain_receipt_pending,
        thread_projection_pending,
    })
}

fn exact_bool(value: &Value, field: &str) -> anyhow::Result<bool> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .with_context(|| format!("autonomy state field {field} is absent"))
}

#[allow(clippy::too_many_lines)] // Mirrors the immutable verifier's exact terminal grammar.
fn ledger_index(path: &Path, kind: &str, relative: &'static str) -> anyhow::Result<LedgerIndex> {
    let (bytes, metadata) = stable_read(path, MAXIMUM_LEDGER_BYTES)?;
    if metadata.permissions().mode() & 0o077 != 0 {
        anyhow::bail!("activity ledger is not owner-only");
    }
    let mut pending = BTreeMap::<String, bool>::new();
    let mut completed = BTreeSet::<String>::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_slice(line)?;
        match kind {
            "actions" => {
                let schema = value
                    .get("schema")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let status = value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !schema.starts_with("astrid_edge_action_receipt_v")
                    || matches!(status, "requested" | "running" | "in_progress")
                {
                    anyhow::bail!("action receipt ledger is malformed or pending");
                }
            },
            "web" | "introspection" => {
                let identifier = value
                    .get("call_id")
                    .and_then(Value::as_str)
                    .filter(|identifier| !identifier.is_empty() && identifier.len() <= 256)
                    .context("tool receipt call identity is absent")?;
                match value.get("phase").and_then(Value::as_str) {
                    Some("requested") => {
                        let expected_schema = if kind == "web" {
                            "astrid_edge_web_tool_receipt_v2"
                        } else {
                            "astrid_edge_introspection_receipt_v1"
                        };
                        if value.get("schema").and_then(Value::as_str) != Some(expected_schema)
                            || value.get("status").and_then(Value::as_str) != Some("requested")
                            || pending.contains_key(identifier)
                            || completed.contains(identifier)
                        {
                            anyhow::bail!("tool request is malformed, duplicate, or replayed");
                        }
                        pending.insert(identifier.to_owned(), true);
                    },
                    Some("completed") => {
                        let expected_schema = if kind == "web" {
                            "astrid_edge_web_tool_receipt_v2"
                        } else {
                            "astrid_edge_introspection_receipt_v1"
                        };
                        if value.get("schema").and_then(Value::as_str) != Some(expected_schema)
                            || pending.remove(identifier).is_none()
                            || !completed.insert(identifier.to_owned())
                        {
                            anyhow::bail!("tool completion has no unique exact request");
                        }
                    },
                    None => validate_legacy_completion(
                        &value,
                        kind,
                        identifier,
                        &pending,
                        &mut completed,
                    )?,
                    _ => anyhow::bail!("tool receipt phase is unsupported"),
                }
            },
            _ => anyhow::bail!("unsupported activity ledger kind"),
        }
    }
    let pending_count = u64::try_from(pending.len()).unwrap_or(u64::MAX);
    if pending_count != 0 {
        anyhow::bail!("activity ledger contains pending calls");
    }
    Ok(LedgerIndex {
        path: relative,
        inode: metadata.ino(),
        size_bytes: metadata.len(),
        sha256: sha256(&bytes),
        pending_count,
    })
}

fn validate_legacy_completion(
    value: &Value,
    kind: &str,
    identifier: &str,
    pending: &BTreeMap<String, bool>,
    completed: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result_hash = value
        .get("result_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let authority = value
        .get("authority")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let known = (kind == "web" && schema == "astrid_edge_web_tool_receipt_v1")
        || (kind == "introspection" && schema == "astrid_edge_introspection_receipt_v1");
    if !known
        || matches!(status, "" | "requested" | "running" | "in_progress")
        || !is_lower_hex(result_hash, 64)
        || !authority.contains("result_not_")
        || !authority.ends_with("authorship")
        || pending.contains_key(identifier)
        || !completed.insert(identifier.to_owned())
    {
        anyhow::bail!("legacy completion-only receipt is not exact terminal evidence");
    }
    Ok(())
}

fn write_ack(
    path: &Path,
    generation_binding: &Path,
    bound: &BoundLease,
    blocked_since_unix_ms: u64,
    now: u64,
    drain: DrainSnapshot,
    indexes: EdgeIndexes,
) -> anyhow::Result<()> {
    let generation_id = read_generation(generation_binding)?;
    anyhow::ensure!(
        bound
            .generation_id
            .as_deref()
            .is_none_or(|expected| expected == generation_id),
        "scheduled-reflection lease generation differs from active generation"
    );
    anyhow::ensure!(
        now >= bound.created_at_unix_ms && now < bound.expires_at_unix_ms,
        "lease expired before edge acknowledgement"
    );
    let acknowledgement = EdgeAck {
        schema: ACK_SCHEMA,
        role: "edge",
        lease_schema: bound.kind.schema(),
        lease_kind: bound.kind.as_str(),
        lease_id: &bound.lease_id,
        lease_nonce_sha256: &bound.nonce_sha256,
        lease_payload_sha256: &bound.payload_sha256,
        generation_id: &generation_id,
        blocked_since_unix_ms,
        acknowledged_at_unix_ms: now,
        pid: u64::from(std::process::id()),
        process_start_ticks: process_start_ticks()?,
        authority: ACK_AUTHORITY,
        new_work_blocked: true,
        drain_barrier_sequence: drain.drain_barrier_sequence,
        ipc_sequence_exact: drain.ipc_sequence_exact,
        scheduled_work_count: drain.scheduled_work_count,
        action_work_count: drain.action_work_count,
        continuation_work_count: drain.continuation_work_count,
        indexes,
    };
    atomic_owner_write(path, &serde_json::to_vec(&acknowledgement)?)
}

fn read_generation(path: &Path) -> anyhow::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o777 != 0o444
        || metadata.len() > 256
    {
        anyhow::bail!("generation binding identity failed");
    }
    let bytes = stable_read(path, 256)?.0;
    let generation = std::str::from_utf8(&bytes)?
        .strip_suffix('\n')
        .context("generation binding must contain exactly one canonical newline")?;
    if generation.is_empty()
        || generation.len() > 128
        || generation == "."
        || generation == ".."
        || !generation
            .bytes()
            .enumerate()
            .all(|(index, byte)| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
                b'.' | b'_' | b'-' => index > 0,
                _ => false,
            })
    {
        anyhow::bail!("generation binding is not canonical");
    }
    Ok(generation.to_owned())
}

fn stable_read(path: &Path, maximum: u64) -> anyhow::Result<(Vec<u8>, fs::Metadata)> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() > maximum
    {
        anyhow::bail!("bounded stable input identity failed");
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc_o_nofollow_cloexec())
        .open(path)?;
    let opened = file.metadata()?;
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    (&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    let identity = |metadata: &fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
        )
    };
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != opened.len()
        || identity(&before) != identity(&opened)
        || identity(&opened) != identity(&after)
        || identity(&after) != identity(&path_after)
    {
        anyhow::bail!("bounded input changed during exact scan");
    }
    Ok((bytes, opened))
}

fn atomic_owner_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("maintenance ACK path has no parent")?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o022 != 0
    {
        anyhow::bail!("maintenance ACK parent is writable outside its owner");
    }
    let temporary = parent.join(format!(
        ".maintenance-edge-ack.{}.tmp",
        uuid::Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn remove_ack(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|metadata| {
        (metadata.is_file() || metadata.file_type().is_symlink()) && metadata.nlink() == 1
    }) {
        let _ = fs::remove_file(path);
    }
}

fn remove_ack_for_new_work(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if (metadata.is_file() || metadata.file_type().is_symlink())
                && metadata.nlink() == 1 =>
        {
            fs::remove_file(path)?;
            let parent = path
                .parent()
                .context("maintenance ACK path has no parent")?;
            File::open(parent)?.sync_all()?;
        },
        Ok(_) => anyhow::bail!("maintenance ACK is not a removable single-link file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.into()),
    }
    anyhow::ensure!(
        matches!(
            fs::symlink_metadata(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ),
        "maintenance ACK remained present after invalidation"
    );
    Ok(())
}

fn process_start_ticks() -> anyhow::Result<u64> {
    let stat = fs::read_to_string("/proc/self/stat")?;
    let after_name = stat.rfind(") ").context("process stat is malformed")?;
    stat[after_name.saturating_add(2)..]
        .split_whitespace()
        .nth(19)
        .context("process start time is absent")?
        .parse()
        .context("process start time is invalid")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b'-' => index > 0,
            _ => false,
        })
}

fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                matches!(byte, b'0'..=b'9' | b'a'..=b'f')
            }
        })
}

fn current_boot_id() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| valid_boot_id(value))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(target_os = "linux")]
const fn libc_o_nofollow_cloexec() -> i32 {
    0o00_400_000 | 0o02_000_000
}

#[cfg(not(target_os = "linux"))]
const fn libc_o_nofollow_cloexec() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::{
        BoundLease, LeaseKind, LeaseProbe, LeaseState, WorkTracker, is_lower_hex, read_bound_lease,
        validate_legacy_completion,
    };
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, mpsc};
    use std::{fs, thread, time::Duration};

    #[test]
    fn absent_lease_is_not_a_blocking_object() {
        let path = std::env::temp_dir().join(format!(
            "astrid-edge-absent-maintenance-{}",
            uuid::Uuid::new_v4()
        ));
        assert!(matches!(
            read_bound_lease(&path, LeaseKind::GenerationTransition, None, 1),
            Ok(LeaseState::Absent)
        ));
    }

    #[test]
    fn present_reflection_object_blocks_new_permit_admission() {
        use std::os::unix::fs::MetadataExt as _;

        let directory = std::env::temp_dir().join(format!(
            "astrid-edge-reflection-admission-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        let transition = directory.join("maintenance.json");
        let reflection = directory.join("reflection.json");
        let tracker = Arc::new(WorkTracker::new(
            None,
            Some(LeaseProbe {
                transition,
                reflection: reflection.clone(),
                runtime_gid: fs::metadata(&directory).unwrap().gid(),
            }),
        ));
        drop(tracker.begin_action().unwrap());

        fs::write(&reflection, b"present-but-malformed-must-fail-closed").unwrap();
        assert!(tracker.begin_action().is_err());
        assert_eq!(tracker.work_counts(), (0, 0, 0));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn legacy_completion_requires_bounded_non_authorship_evidence() {
        let value = json!({
            "schema": "astrid_edge_web_tool_receipt_v1",
            "call_id": "call-1",
            "status": "ok",
            "result_sha256": "a".repeat(64),
            "authority": "tool_result_not_astrid_authorship"
        });
        let mut completed = BTreeSet::new();
        validate_legacy_completion(&value, "web", "call-1", &BTreeMap::new(), &mut completed)
            .unwrap();
        assert!(completed.contains("call-1"));
        assert!(is_lower_hex(&"f".repeat(64), 64));
    }

    fn bound_lease() -> BoundLease {
        BoundLease {
            kind: LeaseKind::GenerationTransition,
            lease_id: "lease-test".to_string(),
            created_at_unix_ms: 1,
            expires_at_unix_ms: u64::MAX,
            generation_id: None,
            payload_sha256: "b".repeat(64),
            nonce_sha256: "c".repeat(64),
        }
    }

    #[test]
    fn exact_barrier_is_invalidated_by_local_work_and_reissued_after_drain() {
        let tracker = Arc::new(WorkTracker::default());
        let lease = bound_lease();
        tracker.ipc_authenticated();
        tracker.observe_barrier(
            7,
            lease.kind.schema(),
            lease.kind.as_str(),
            &lease.lease_id,
            &lease.payload_sha256,
        );
        let first = tracker.drain_snapshot(&lease).unwrap();
        assert_eq!(first.drain_barrier_sequence, 7);
        assert!(first.ipc_sequence_exact);

        let action = tracker.begin_action().unwrap();
        assert!(tracker.drain_snapshot(&lease).is_err());
        drop(action);
        // Finishing work does not resurrect an older barrier.
        assert!(tracker.drain_snapshot(&lease).is_err());
        tracker.observe_barrier(
            8,
            lease.kind.schema(),
            lease.kind.as_str(),
            &lease.lease_id,
            &lease.payload_sha256,
        );
        assert_eq!(
            tracker
                .drain_snapshot(&lease)
                .unwrap()
                .drain_barrier_sequence,
            8
        );
    }

    #[test]
    fn acknowledgement_write_and_new_work_admission_are_one_serialized_epoch() {
        let directory = std::env::temp_dir().join(format!(
            "astrid-edge-ack-admission-race-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        let acknowledgement = directory.join("edge-ack.json");
        let tracker = Arc::new(WorkTracker::new(Some(acknowledgement.clone()), None));
        let lease = bound_lease();
        tracker.ipc_authenticated();
        tracker.observe_barrier(
            11,
            lease.kind.schema(),
            lease.kind.as_str(),
            &lease.lease_id,
            &lease.payload_sha256,
        );

        let worker = tracker
            .with_drain_snapshot(&lease, |_| {
                fs::write(&acknowledgement, b"sealed acknowledgement").unwrap();
                let (attempting_tx, attempting_rx) = mpsc::channel();
                let admitting = Arc::clone(&tracker);
                let worker = thread::spawn(move || {
                    attempting_tx.send(()).unwrap();
                    admitting.begin_model_turn().unwrap()
                });
                attempting_rx.recv().unwrap();
                // The worker has reached admission, but cannot pass the exact
                // snapshot mutex until this simulated ACK write returns.
                thread::sleep(Duration::from_millis(20));
                assert!(acknowledgement.exists());
                Ok(worker)
            })
            .unwrap();
        let permit = worker.join().unwrap();
        assert!(!acknowledgement.exists());
        assert!(tracker.drain_snapshot(&lease).is_err());
        drop(permit);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn disconnect_or_nonmonotonic_barrier_poisons_exactness_until_restart() {
        let tracker = WorkTracker::default();
        let lease = bound_lease();
        tracker.ipc_authenticated();
        tracker.observe_barrier(
            9,
            lease.kind.schema(),
            lease.kind.as_str(),
            &lease.lease_id,
            &lease.payload_sha256,
        );
        tracker.observe_barrier(
            9,
            lease.kind.schema(),
            lease.kind.as_str(),
            &lease.lease_id,
            &lease.payload_sha256,
        );
        assert!(tracker.drain_snapshot(&lease).is_err());

        let tracker = WorkTracker::default();
        tracker.ipc_authenticated();
        tracker.ipc_disconnected();
        tracker.ipc_authenticated();
        tracker.observe_barrier(
            10,
            lease.kind.schema(),
            lease.kind.as_str(),
            &lease.lease_id,
            &lease.payload_sha256,
        );
        assert!(tracker.drain_snapshot(&lease).is_err());
    }
}
