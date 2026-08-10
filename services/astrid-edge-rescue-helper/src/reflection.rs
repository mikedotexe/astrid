//! Root-only scheduled-reflection admission, drain verification, and cleanup.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::fs_guard::{canonical_json, read_regular, sha256};
use crate::{Error, Result};

pub const LEASE_PATH: &str = "/run/astrid-edge-self-change/reflection.json";
pub const ADMISSION_PATH: &str = "/run/astrid-edge-self-change/reflection-admission.json";
pub const LEASE_SCHEMA: &str = "astrid.edge_scheduled_reflection.lease.v1";
pub const ADMISSION_SCHEMA: &str = "astrid.edge_programmatic_reflection.admission.v3";
const ACK_SCHEMA: &str = "astrid.edge.maintenance_ack.v2";
const LEASE_KIND: &str = "scheduled_reflection";
const LEASE_OWNER: &str = "immutable_astrid_edge_reflection_guard";
const ACK_AUTHORITY: &str = "mutable_runtime_acknowledgement_subject_to_immutable_verification";
const HANDOFF_SCHEMA: &str = "astrid.edge.steward_helper.supervisor_handoff_trigger.v1";
const HANDOFF_PROVENANCE: &str = "exact_model_intent_already_published";
const HANDOFF_AUTHORITY: &str = "trigger_only_no_candidate_or_deployment_authority";
const MAXIMUM_BYTES: u64 = 64 * 1024;
const MAXIMUM_INTEGRATION_STATE_BYTES: u64 = 16 * 1024 * 1024;
const MAXIMUM_HANDOFF_BYTES: u64 = 8 * 1024;
const MAXIMUM_PENDING_HANDOFFS: usize = 64;
const LEASE_LIFETIME_MS: u64 = 3 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionMarker {
    schema: String,
    lease_schema: String,
    lease_kind: String,
    lease_id: String,
    lease_nonce_sha256: String,
    lease_payload_sha256: String,
    generation_id: String,
    host_boot_id: String,
    service_invocation_id: String,
    admitted_at_unix_ms: u64,
    drain_barrier_sequence: u64,
    core_ack_sha256: String,
    edge_ack_sha256: String,
    model_lock_device: u64,
    model_lock_inode: u64,
    reflection_kind: String,
    reflection_due_nonce_sha256: Option<String>,
    reflection_trigger_nonce_sha256: Option<String>,
    model_start_authority: String,
    authority: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingSupervisorHandoff {
    schema: String,
    appliance_id: String,
    envelope_id: String,
    envelope_sha256: String,
    intent_id: String,
    candidate_id: String,
    candidate_sha256: String,
    response_sha256: String,
    created_at: u64,
    provenance: String,
    authority: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleStateV1 {
    schema: String,
    next_due_at_unix_seconds: u64,
    pending_due_at_unix_seconds: Option<u64>,
    last_completed_at_unix_seconds: Option<u64>,
    completed_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleStateV2 {
    schema: String,
    next_due_at_unix_seconds: u64,
    pending_due_at_unix_seconds: Option<u64>,
    last_completed_at_unix_seconds: Option<u64>,
    last_model_started_at_unix_seconds: Option<u64>,
    next_model_eligible_at_unix_seconds: u64,
    completed_count: u64,
    model_start_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationEvidenceRecord {
    evidence_id: String,
    kind: String,
    epistemic_status: String,
    reference: String,
    summary: String,
    source: String,
    captured_at_unix_ms: u64,
    sha256: String,
    eligible_for_belief_update: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationActiveTrigger {
    trigger_nonce: String,
    due_nonce: String,
    generation: u64,
    created_at_unix_ms: u64,
    last_attempt_at_unix_ms: Option<u64>,
    evidence: Vec<IntegrationEvidenceRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationConsumedEvidence {
    evidence_id: String,
    sha256: String,
    consumed_at_unix_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationRejectedEvidence {
    record_sha256: String,
    evidence_id: Option<String>,
    reason: String,
    source_revision: u64,
    first_seen_at_unix_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationAmbiguousTrigger {
    trigger_nonce: String,
    due_nonce: String,
    provider_started_at_unix_ms: u64,
    terminalized_at_unix_ms: u64,
    evidence: Vec<IntegrationConsumedEvidence>,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationScheduledAbsorption {
    scheduled_nonce: String,
    prepared_at_unix_ms: u64,
    evidence: Vec<IntegrationEvidenceRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationStateV1 {
    schema: String,
    generation: u64,
    pending: Vec<IntegrationEvidenceRecord>,
    quiet_until_unix_ms: u64,
    active: Option<IntegrationActiveTrigger>,
    consumed: Vec<IntegrationConsumedEvidence>,
    rejected: Vec<IntegrationRejectedEvidence>,
    ambiguous: Vec<IntegrationAmbiguousTrigger>,
    scheduled_absorption: Option<IntegrationScheduledAbsorption>,
    last_completed_at_unix_ms: Option<u64>,
    last_finished_trigger_nonce: Option<String>,
    last_finished_due_nonce: Option<String>,
    last_absorbed_scheduled_nonce: Option<String>,
    utc_day: u64,
    starts_today: u8,
    last_source_revision: Option<u64>,
    last_source_sha256: Option<String>,
}

#[derive(Debug)]
struct GuardPaths {
    lease: PathBuf,
    admission: PathBuf,
    transition_lease: PathBuf,
    mutex: PathBuf,
    generation: PathBuf,
    model_lock: PathBuf,
    core_ack: PathBuf,
    edge_ack: PathBuf,
    schedule: PathBuf,
    evidence_integration: PathBuf,
}

#[derive(Debug)]
struct Context {
    now_ms: u64,
    boot_id: String,
    invocation_id: String,
    nonce: String,
}

#[derive(Debug, Serialize)]
pub struct GuardResult {
    schema: &'static str,
    status: &'static str,
    lease_id: Option<String>,
}

#[derive(Debug)]
struct AckProof {
    sequence: u64,
    core_sha256: String,
    edge_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduleAdmission {
    NotDue,
    ModelStart {
        due_nonce: String,
    },
    PreparedRecovery {
        due_nonce: String,
    },
    LegacyMigration {
        due_nonce: String,
    },
    Cooling {
        due_nonce: String,
    },
    EvidenceModelStart {
        due_nonce: String,
        trigger_nonce: String,
    },
    EvidencePreparedRecovery {
        due_nonce: String,
        trigger_nonce: String,
    },
}

impl ScheduleAdmission {
    fn is_due(&self) -> bool {
        !matches!(self, Self::NotDue | Self::Cooling { .. })
    }

    fn due_nonce_sha256(&self) -> Option<String> {
        match self {
            Self::NotDue => None,
            Self::ModelStart { due_nonce: value }
            | Self::PreparedRecovery { due_nonce: value }
            | Self::LegacyMigration { due_nonce: value }
            | Self::Cooling { due_nonce: value }
            | Self::EvidenceModelStart {
                due_nonce: value, ..
            }
            | Self::EvidencePreparedRecovery {
                due_nonce: value, ..
            } => Some(sha256(value.as_bytes())),
        }
    }

    fn trigger_nonce_sha256(&self) -> Option<String> {
        match self {
            Self::EvidenceModelStart { trigger_nonce, .. }
            | Self::EvidencePreparedRecovery { trigger_nonce, .. } => {
                Some(sha256(trigger_nonce.as_bytes()))
            },
            _ => None,
        }
    }

    fn reflection_kind(&self) -> &'static str {
        match self {
            Self::EvidenceModelStart { .. } | Self::EvidencePreparedRecovery { .. } => {
                "evidence_integration"
            },
            _ => "scheduled",
        }
    }

    fn model_start_authority(&self) -> &'static str {
        match self {
            Self::ModelStart { .. } => "root_schedule_model_start_allowed",
            Self::PreparedRecovery { .. } => "root_schedule_prepared_recovery_only",
            Self::LegacyMigration { .. } => "root_schedule_legacy_migration_only",
            Self::EvidenceModelStart { .. } => "root_evidence_integration_model_start_allowed",
            Self::EvidencePreparedRecovery { .. } => {
                "root_evidence_integration_prepared_recovery_only"
            },
            Self::NotDue | Self::Cooling { .. } => "root_schedule_not_due",
        }
    }
}

impl GuardPaths {
    fn production(config: &Config) -> Result<Self> {
        let steward_state = config
            .roots
            .candidate_store
            .parent()
            .ok_or_else(|| Error::new("steward state root is absent"))?;
        Ok(Self {
            lease: PathBuf::from(LEASE_PATH),
            admission: PathBuf::from(ADMISSION_PATH),
            transition_lease: config.roots.maintenance_lease.clone(),
            mutex: config.roots.maintenance_mutex.clone(),
            generation: config.roots.generation_binding.clone(),
            model_lock: config.drain.model_lock.clone(),
            core_ack: config.drain.maintenance_core_acknowledgement.clone(),
            edge_ack: config.drain.maintenance_edge_acknowledgement.clone(),
            schedule: steward_state.join("schedule.json"),
            evidence_integration: steward_state.join("evidence-integration.json"),
        })
    }
}

/// Prepare one exact scheduled-reflection admission or return a no-op when the
/// durable steward schedule is not due.
pub fn prepare(config: &Config) -> Result<GuardResult> {
    require_root()?;
    let context = production_context()?;
    prepare_inner(config, &GuardPaths::production(config)?, &context, 0, true)
}

/// Remove only the reflection artifacts bound to this systemd invocation.
pub fn cleanup(config: &Config) -> Result<GuardResult> {
    require_root()?;
    let boot_id = current_boot_id()?;
    let invocation_id = current_invocation_id()?;
    cleanup_inner(
        config,
        &GuardPaths::production(config)?,
        &boot_id,
        &invocation_id,
        0,
    )?;
    // Promotion is deliberately last: the watched marker cannot exist while
    // a reflection admission or its model-lock lease is still live.
    promote_pending_supervisor_handoffs(
        &config.roots.supervisor_state.join("inbox"),
        &config.appliance_id,
        config.identities.steward_uid,
        config.identities.steward_gid,
        0,
        0,
    )?;
    Ok(GuardResult {
        schema: ADMISSION_SCHEMA,
        status: "cleaned",
        lease_id: None,
    })
}

/// Reconcile only exact prior-boot reflection artifacts. Generation-transition
/// maintenance is intentionally outside this command's authority.
pub fn reconcile(config: &Config) -> Result<GuardResult> {
    require_root()?;
    let boot_id = current_boot_id()?;
    reconcile_inner(config, &GuardPaths::production(config)?, &boot_id, 0)?;
    // A power loss after the authored intent was published but before normal
    // ExecStopPost cleanup leaves only an inert `.pending` file.  Reboot
    // reconciliation first removes the prior-boot lease, then promotes it.
    promote_pending_supervisor_handoffs(
        &config.roots.supervisor_state.join("inbox"),
        &config.appliance_id,
        config.identities.steward_uid,
        config.identities.steward_gid,
        0,
        0,
    )?;
    Ok(GuardResult {
        schema: ADMISSION_SCHEMA,
        status: "reconciled",
        lease_id: None,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed protocol transaction keeps lease creation, drain proof, and admission cleanup visibly ordered"
)]
fn prepare_inner(
    config: &Config,
    paths: &GuardPaths,
    context: &Context,
    root_uid: u32,
    require_live_processes: bool,
) -> Result<GuardResult> {
    validate_context(context)?;
    validate_shared_parent(&paths.lease, root_uid)?;
    let scheduled = schedule_admission(config, &paths.schedule, context.now_ms / 1_000)?;
    let schedule = if scheduled.is_due() {
        scheduled
    } else {
        evidence_integration_admission(config, &paths.evidence_integration, context.now_ms)?
    };
    if !schedule.is_due() {
        if paths.lease.exists()
            || paths.lease.is_symlink()
            || paths.admission.exists()
            || paths.admission.is_symlink()
        {
            return Err(Error::new(
                "not-due reflection found stale admission artifacts",
            ));
        }
        return Ok(GuardResult {
            schema: ADMISSION_SCHEMA,
            status: "not_due",
            lease_id: None,
        });
    }

    let mutex = acquire_mutex(config, paths, root_uid)?;
    reject_present(&paths.transition_lease, "generation-transition lease")?;
    reject_present(&paths.lease, "scheduled-reflection lease")?;
    reject_present(&paths.admission, "scheduled-reflection admission")?;
    let generation_id = read_generation(&paths.generation, root_uid)?;
    let nonce_sha256 = sha256(context.nonce.as_bytes());
    let lease_id = format!("reflection-{}", &nonce_sha256[..24]);
    let lease = ReflectionLease {
        schema: LEASE_SCHEMA.to_owned(),
        lease_kind: LEASE_KIND.to_owned(),
        created_at_unix_ms: context.now_ms,
        expires_at_unix_ms: context.now_ms.saturating_add(LEASE_LIFETIME_MS),
        reason: "scheduled_reflection".to_owned(),
        owner: LEASE_OWNER.to_owned(),
        lease_id: lease_id.clone(),
        nonce: context.nonce.clone(),
        host_boot_id: context.boot_id.clone(),
        service_invocation_id: context.invocation_id.clone(),
        generation_id: generation_id.clone(),
    };
    let lease_bytes = canonical_json(&lease)?;
    atomic_owned_write(
        &paths.lease,
        &lease_bytes,
        0o440,
        root_uid,
        config.identities.runtime_gid,
        false,
    )?;
    reject_present(&paths.transition_lease, "generation-transition lease")?;
    mutex.unlock().map_err(Error::from)?;

    let lease_sha256 = sha256(&lease_bytes);
    let admitted = wait_for_drain(
        config,
        paths,
        &lease,
        &lease_sha256,
        &nonce_sha256,
        root_uid,
        require_live_processes,
    );
    let mut created_marker_bytes = None;
    let result = match admitted {
        Ok((proof, model_lock)) => (|| {
            let metadata = model_lock.metadata()?;
            let marker = AdmissionMarker {
                schema: ADMISSION_SCHEMA.to_owned(),
                lease_schema: LEASE_SCHEMA.to_owned(),
                lease_kind: LEASE_KIND.to_owned(),
                lease_id: lease_id.clone(),
                lease_nonce_sha256: nonce_sha256,
                lease_payload_sha256: lease_sha256,
                generation_id,
                host_boot_id: context.boot_id.clone(),
                service_invocation_id: context.invocation_id.clone(),
                admitted_at_unix_ms: unix_millis(),
                drain_barrier_sequence: proof.sequence,
                core_ack_sha256: proof.core_sha256,
                edge_ack_sha256: proof.edge_sha256,
                model_lock_device: metadata.dev(),
                model_lock_inode: metadata.ino(),
                reflection_kind: schedule.reflection_kind().to_owned(),
                reflection_due_nonce_sha256: schedule.due_nonce_sha256(),
                reflection_trigger_nonce_sha256: schedule.trigger_nonce_sha256(),
                model_start_authority: schedule.model_start_authority().to_owned(),
                authority: "root_verified_drain_and_model_lock_handoff_not_activation_authority"
                    .to_owned(),
            };
            let marker_bytes = canonical_json(&marker)?;
            atomic_owned_write(
                &paths.admission,
                &marker_bytes,
                0o440,
                root_uid,
                config.identities.steward_gid,
                false,
            )?;
            created_marker_bytes = Some(marker_bytes);
            model_lock.unlock().map_err(Error::from)?;
            Ok(GuardResult {
                schema: ADMISSION_SCHEMA,
                status: "admitted",
                lease_id: Some(lease_id),
            })
        })(),
        Err(error) => Err(error),
    };
    if result.is_err() {
        if let Some(marker_bytes) = created_marker_bytes.as_deref() {
            let _ = remove_exact(&paths.admission, Some(marker_bytes), root_uid);
        }
        let _ = remove_exact(&paths.lease, Some(&lease_bytes), root_uid);
    }
    result
}

fn wait_for_drain(
    config: &Config,
    paths: &GuardPaths,
    lease: &ReflectionLease,
    lease_sha256: &str,
    nonce_sha256: &str,
    root_uid: u32,
    require_live_processes: bool,
) -> Result<(AckProof, File)> {
    let started = Instant::now();
    let timeout = Duration::from_secs(config.drain.maximum_wait_seconds);
    loop {
        if validate_ack_pair(
            config,
            paths,
            lease,
            lease_sha256,
            nonce_sha256,
            root_uid,
            require_live_processes,
        )
        .is_ok()
            && let Some(model_lock) = try_model_lock(config, paths, root_uid)?
            && let Ok(proof) = validate_ack_pair(
                config,
                paths,
                lease,
                lease_sha256,
                nonce_sha256,
                root_uid,
                require_live_processes,
            )
        {
            let current = read_regular(&paths.lease, MAXIMUM_BYTES)?;
            if sha256(&current) == lease_sha256
                && reject_present(&paths.transition_lease, "generation-transition lease").is_ok()
            {
                return Ok((proof, model_lock));
            }
        }
        if started.elapsed() >= timeout {
            return Err(Error::deferred(
                "scheduled reflection could not obtain exact drained model handoff",
            ));
        }
        thread::sleep(Duration::from_millis(config.drain.poll_milliseconds));
    }
}

fn validate_ack_pair(
    config: &Config,
    paths: &GuardPaths,
    lease: &ReflectionLease,
    lease_sha256: &str,
    nonce_sha256: &str,
    root_uid: u32,
    require_live_processes: bool,
) -> Result<AckProof> {
    let generation = read_generation(&paths.generation, root_uid)?;
    if generation != lease.generation_id {
        return Err(Error::new("reflection generation changed before admission"));
    }
    let (core, core_bytes) = read_ack(&paths.core_ack, config.identities.runtime_uid)?;
    let (edge, edge_bytes) = read_ack(&paths.edge_ack, config.identities.runtime_uid)?;
    if !has_exact_object_keys(&core, CORE_ACK_KEYS) || !has_exact_object_keys(&edge, EDGE_ACK_KEYS)
    {
        return Err(Error::new(
            "reflection acknowledgement fields differ from the exact contract",
        ));
    }
    let core_sequence = validate_common_ack(
        &core,
        "core",
        lease,
        lease_sha256,
        nonce_sha256,
        &generation,
    )?;
    let edge_sequence = validate_common_ack(
        &edge,
        "edge",
        lease,
        lease_sha256,
        nonce_sha256,
        &generation,
    )?;
    if core_sequence != edge_sequence
        || core.get("ipc_user_input_blocked").and_then(Value::as_bool) != Some(true)
        || [
            "active_conversations",
            "active_sessions",
            "active_tools",
            "active_llm_requests",
        ]
        .iter()
        .any(|field| core.get(*field).and_then(Value::as_u64) != Some(0))
        || edge.get("new_work_blocked").and_then(Value::as_bool) != Some(true)
        || edge.get("ipc_sequence_exact").and_then(Value::as_bool) != Some(true)
        || [
            "scheduled_work_count",
            "action_work_count",
            "continuation_work_count",
        ]
        .iter()
        .any(|field| edge.get(*field).and_then(Value::as_u64) != Some(0))
        || !edge_indexes_are_zero(&edge)
    {
        return Err(Error::new(
            "reflection drain acknowledgement still has active work",
        ));
    }
    if require_live_processes {
        validate_process(config, &core, "astrid-daemon")?;
        validate_process(config, &edge, "astrid-edge-runtime")?;
    }
    Ok(AckProof {
        sequence: core_sequence,
        core_sha256: sha256(&core_bytes),
        edge_sha256: sha256(&edge_bytes),
    })
}

const CORE_ACK_KEYS: &[&str] = &[
    "schema",
    "role",
    "lease_schema",
    "lease_kind",
    "lease_id",
    "lease_nonce_sha256",
    "lease_payload_sha256",
    "generation_id",
    "blocked_since_unix_ms",
    "acknowledged_at_unix_ms",
    "pid",
    "process_start_ticks",
    "authority",
    "drain_barrier_sequence",
    "ipc_user_input_blocked",
    "active_conversations",
    "active_sessions",
    "active_tools",
    "active_llm_requests",
];

const EDGE_ACK_KEYS: &[&str] = &[
    "schema",
    "role",
    "lease_schema",
    "lease_kind",
    "lease_id",
    "lease_nonce_sha256",
    "lease_payload_sha256",
    "generation_id",
    "blocked_since_unix_ms",
    "acknowledged_at_unix_ms",
    "pid",
    "process_start_ticks",
    "authority",
    "drain_barrier_sequence",
    "new_work_blocked",
    "ipc_sequence_exact",
    "scheduled_work_count",
    "action_work_count",
    "continuation_work_count",
    "indexes",
];

fn has_exact_object_keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
    })
}

fn validate_common_ack(
    value: &Value,
    role: &str,
    lease: &ReflectionLease,
    lease_sha256: &str,
    nonce_sha256: &str,
    generation: &str,
) -> Result<u64> {
    let now = unix_millis();
    let sequence = value
        .get("drain_barrier_sequence")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new("drain barrier sequence is absent"))?;
    let blocked = value
        .get("blocked_since_unix_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let acknowledged = value
        .get("acknowledged_at_unix_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if value.get("schema").and_then(Value::as_str) != Some(ACK_SCHEMA)
        || value.get("role").and_then(Value::as_str) != Some(role)
        || value.get("lease_schema").and_then(Value::as_str) != Some(LEASE_SCHEMA)
        || value.get("lease_kind").and_then(Value::as_str) != Some(LEASE_KIND)
        || value.get("lease_id").and_then(Value::as_str) != Some(lease.lease_id.as_str())
        || value.get("lease_nonce_sha256").and_then(Value::as_str) != Some(nonce_sha256)
        || value.get("lease_payload_sha256").and_then(Value::as_str) != Some(lease_sha256)
        || value.get("generation_id").and_then(Value::as_str) != Some(generation)
        || value.get("authority").and_then(Value::as_str) != Some(ACK_AUTHORITY)
        || blocked < lease.created_at_unix_ms
        || acknowledged < blocked
        || acknowledged > now.saturating_add(30_000)
        || now.saturating_sub(acknowledged) > 60_000
    {
        return Err(Error::new("reflection acknowledgement binding failed"));
    }
    Ok(sequence)
}

fn edge_indexes_are_zero(edge: &Value) -> bool {
    let Some(indexes) = edge.get("indexes") else {
        return false;
    };
    if !has_exact_object_keys(indexes, &["autonomy", "ledgers"]) {
        return false;
    }
    let Some(autonomy) = indexes.get("autonomy") else {
        return false;
    };
    if !has_exact_object_keys(
        autonomy,
        &[
            "path",
            "sha256",
            "size_bytes",
            "action_dispatch_pending",
            "run_receipt_pending",
            "chain_receipt_pending",
            "thread_projection_pending",
        ],
    ) {
        return false;
    }
    let Some(ledgers) = indexes.get("ledgers") else {
        return false;
    };
    if !has_exact_object_keys(ledgers, &["actions", "web", "introspection"])
        || ["actions", "web", "introspection"].iter().any(|kind| {
            ledgers.get(*kind).is_none_or(|ledger| {
                !has_exact_object_keys(
                    ledger,
                    &["path", "inode", "size_bytes", "sha256", "pending_count"],
                )
            })
        })
    {
        return false;
    }
    let autonomy_clear = [
        "action_dispatch_pending",
        "run_receipt_pending",
        "chain_receipt_pending",
        "thread_projection_pending",
    ]
    .iter()
    .all(|field| autonomy.get(*field).and_then(Value::as_bool) == Some(false));
    let ledgers_clear = ["actions", "web", "introspection"].iter().all(|kind| {
        ledgers
            .get(*kind)
            .and_then(|ledger| ledger.get("pending_count"))
            .and_then(Value::as_u64)
            == Some(0)
    });
    autonomy_clear && ledgers_clear
}

fn read_ack(path: &Path, runtime_uid: u32) -> Result<(Value, Vec<u8>)> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != runtime_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("runtime reflection ACK identity failed"));
    }
    let bytes = read_regular(path, MAXIMUM_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if !value.is_object() {
        return Err(Error::new("runtime reflection ACK is not an object"));
    }
    Ok((value, bytes))
}

fn validate_process(config: &Config, ack: &Value, binary: &str) -> Result<()> {
    let pid = ack
        .get("pid")
        .and_then(Value::as_u64)
        .filter(|pid| *pid > 1)
        .and_then(|pid| u32::try_from(pid).ok())
        .ok_or_else(|| Error::new("ACK process PID is absent or overflows"))?;
    let ticks = ack
        .get("process_start_ticks")
        .and_then(Value::as_u64)
        .filter(|ticks| *ticks > 0)
        .ok_or_else(|| Error::new("ACK process start identity is absent"))?;
    let process = PathBuf::from(format!("/proc/{pid}"));
    let status = String::from_utf8(read_proc_bounded(&process.join("status"), 128 * 1024)?)
        .map_err(|_| Error::new("ACK process status is not UTF-8"))?;
    let uid = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u32>().ok());
    if uid != Some(config.identities.runtime_uid) {
        return Err(Error::new("ACK process runs as the wrong identity"));
    }
    let stat = String::from_utf8(read_proc_bounded(&process.join("stat"), 64 * 1024)?)
        .map_err(|_| Error::new("ACK process stat is not UTF-8"))?;
    let after = stat
        .rfind(") ")
        .ok_or_else(|| Error::new("ACK process stat is malformed"))?;
    let actual_ticks = stat[after.saturating_add(2)..]
        .split_whitespace()
        .nth(19)
        .and_then(|value| value.parse::<u64>().ok());
    if actual_ticks != Some(ticks)
        || fs::canonicalize(process.join("exe"))?
            != fs::canonicalize(config.roots.active_link.join(binary))?
    {
        return Err(Error::new("ACK process identity is no longer live"));
    }
    Ok(())
}

fn read_proc_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    if !path.starts_with("/proc") {
        return Err(Error::new("process identity path is outside procfs"));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::new(
            "process identity input is not a regular pseudo-file",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(Error::new("process identity input exceeds its bound"));
    }
    Ok(bytes)
}

fn try_model_lock(config: &Config, paths: &GuardPaths, root_uid: u32) -> Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(no_follow_cloexec())
        .open(&paths.model_lock)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != root_uid
        || metadata.gid() != config.drain.model_lock_gid
        || metadata.mode() & 0o777 != 0o640
    {
        return Err(Error::new("reflection model-lock identity failed"));
    }
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn acquire_mutex(config: &Config, paths: &GuardPaths, root_uid: u32) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(no_follow_cloexec())
        .open(&paths.mutex)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != root_uid
        || metadata.mode() & 0o077 != 0
        || paths.mutex != config.roots.maintenance_mutex
    {
        return Err(Error::new("shared maintenance mutex identity failed"));
    }
    file.try_lock_exclusive()
        .map_err(|_| Error::deferred("another maintenance transaction is active"))?;
    Ok(file)
}

fn schedule_admission(config: &Config, path: &Path, now: u64) -> Result<ScheduleAdmission> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // The unprivileged helper must first materialize its exact
            // canonical due slot.  A later timer invocation may then bind the
            // root admission to that nonce; bootstrap never receives an
            // unbound model start.
            return Ok(ScheduleAdmission::NotDue);
        },
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != config.identities.steward_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("scheduled-reflection state identity failed"));
    }
    let admission = schedule_content_admission(&read_regular(path, 16 * 1024)?, now)?;
    if let ScheduleAdmission::Cooling { due_nonce } = admission {
        if exact_prepared_transaction_exists(config, path, &due_nonce)? {
            return Ok(ScheduleAdmission::PreparedRecovery { due_nonce });
        }
        return Ok(ScheduleAdmission::NotDue);
    }
    Ok(admission)
}

#[allow(
    clippy::too_many_lines,
    reason = "the immutable root independently validates the complete steward-owned trigger state before granting model authority"
)]
fn evidence_integration_admission(
    config: &Config,
    path: &Path,
    now_ms: u64,
) -> Result<ScheduleAdmission> {
    let Some(state) = read_evidence_integration_state(
        path,
        config.identities.steward_uid,
        config.identities.steward_gid,
        config.identities.runtime_gid,
    )?
    else {
        return Ok(ScheduleAdmission::NotDue);
    };
    validate_integration_state(&config.appliance_id, &state, now_ms)?;

    let mut recoveries = Vec::new();
    if let Some(active) = &state.active
        && active.last_attempt_at_unix_ms.is_some()
        && exact_prepared_transaction_exists(config, path, &active.due_nonce)?
    {
        recoveries.push((active.due_nonce.clone(), active.trigger_nonce.clone()));
    }
    if let (Some(trigger_nonce), Some(due_nonce)) = (
        state.last_finished_trigger_nonce.as_ref(),
        state.last_finished_due_nonce.as_ref(),
    ) && exact_prepared_transaction_exists(config, path, due_nonce)?
    {
        recoveries.push((due_nonce.clone(), trigger_nonce.clone()));
    }
    if recoveries.len() > 1 {
        return Err(Error::new(
            "multiple evidence-integration recoveries seek one root admission",
        ));
    }
    if let Some((due_nonce, trigger_nonce)) = recoveries.pop() {
        return Ok(ScheduleAdmission::EvidencePreparedRecovery {
            due_nonce,
            trigger_nonce,
        });
    }

    let Some(active) = &state.active else {
        return Ok(ScheduleAdmission::NotDue);
    };
    if active.last_attempt_at_unix_ms.is_some() {
        // A durable provider-start marker without a signed prepared response is
        // ambiguity, never permission to make another possibly duplicate call.
        return Ok(ScheduleAdmission::NotDue);
    }
    if state.utc_day != integration_utc_day(now_ms) || state.starts_today >= 12 {
        return Ok(ScheduleAdmission::NotDue);
    }
    Ok(ScheduleAdmission::EvidenceModelStart {
        due_nonce: active.due_nonce.clone(),
        trigger_nonce: active.trigger_nonce.clone(),
    })
}

fn read_evidence_integration_state(
    path: &Path,
    owner_user_id: u32,
    owner_group_id: u32,
    reader_group_id: u32,
) -> Result<Option<IntegrationStateV1>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        },
        Err(error) => return Err(error.into()),
    };
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("evidence-integration state root is absent"))?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != owner_user_id
        || parent_metadata.gid() != owner_group_id
        || parent_metadata.mode() & 0o777 != 0o700
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != owner_user_id
        || metadata.gid() != reader_group_id
        || metadata.mode() & 0o777 != 0o600
        || metadata.len() > MAXIMUM_INTEGRATION_STATE_BYTES
    {
        return Err(Error::new(
            "evidence-integration state identity or bound failed",
        ));
    }
    let bytes = read_regular(path, MAXIMUM_INTEGRATION_STATE_BYTES)?;
    let state: IntegrationStateV1 = serde_json::from_slice(&bytes)?;
    if canonical_json(&state)? != bytes {
        return Err(Error::new(
            "evidence-integration state is not exact canonical JSON",
        ));
    }
    Ok(Some(state))
}

#[allow(
    clippy::too_many_lines,
    reason = "one exact validator keeps the immutable integration-admission contract reviewable"
)]
fn validate_integration_state(
    appliance_id: &str,
    state: &IntegrationStateV1,
    now_ms: u64,
) -> Result<()> {
    const STATE_SCHEMA: &str = "astrid.edge.steward_helper.evidence_integration_state.v1";
    const MAX_PENDING: usize = 128;
    const MAX_CONSUMED: usize = 65_536;
    const MAX_REJECTED: usize = 4_096;
    const MAX_AMBIGUOUS: usize = 4_096;
    const MAX_TRIGGER_EVIDENCE: usize = 6;
    const MINIMUM_INTERVAL_MS: u64 = 60 * 60 * 1_000;

    if state.schema != STATE_SCHEMA
        || state.pending.len() > MAX_PENDING
        || state.consumed.len() > MAX_CONSUMED
        || state.rejected.len() > MAX_REJECTED
        || state.ambiguous.len() > MAX_AMBIGUOUS
        || state.starts_today > 12
        || state.utc_day > integration_utc_day(now_ms).saturating_add(1)
        || state
            .last_completed_at_unix_ms
            .is_some_and(|value| value == 0 || value > now_ms.saturating_add(60_000))
        || state.last_finished_trigger_nonce.is_some() != state.last_finished_due_nonce.is_some()
        || state
            .last_source_sha256
            .as_ref()
            .is_some_and(|value| !is_lower_hex(value, 64))
        || state.last_source_revision == Some(0)
    {
        return Err(Error::new("evidence-integration root state is invalid"));
    }
    for value in [
        state.last_finished_trigger_nonce.as_deref(),
        state.last_absorbed_scheduled_nonce.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !valid_identifier(value) {
            return Err(Error::new(
                "evidence-integration retained identifier is invalid",
            ));
        }
    }
    if state
        .last_finished_due_nonce
        .as_ref()
        .is_some_and(|value| !valid_integration_due_nonce(value))
    {
        return Err(Error::new(
            "evidence-integration retained due nonce is invalid",
        ));
    }

    let mut identities = BTreeMap::<String, String>::new();
    for record in &state.pending {
        validate_integration_evidence(record, now_ms)?;
        insert_unique_integration_identity(&mut identities, record)?;
    }
    if let Some(active) = &state.active {
        if active.generation != state.generation
            || active.created_at_unix_ms == 0
            || active.created_at_unix_ms > now_ms.saturating_add(60_000)
            || active.evidence.is_empty()
            || active.evidence.len() > MAX_TRIGGER_EVIDENCE
            || !valid_identifier(&active.trigger_nonce)
            || !valid_integration_due_nonce(&active.due_nonce)
            || active
                .last_attempt_at_unix_ms
                .is_some_and(|value| value == 0 || value > now_ms.saturating_add(60_000))
        {
            return Err(Error::new("active evidence-integration trigger is invalid"));
        }
        for record in &active.evidence {
            validate_integration_evidence(record, now_ms)?;
            insert_unique_integration_identity(&mut identities, record)?;
        }
        let (trigger_nonce, due_nonce) =
            derive_integration_identity(appliance_id, active.generation, &active.evidence)?;
        if active.trigger_nonce != trigger_nonce || active.due_nonce != due_nonce {
            return Err(Error::new(
                "active evidence-integration identity derivation failed",
            ));
        }
        if active.last_attempt_at_unix_ms.is_none()
            && (state.quiet_until_unix_ms > active.created_at_unix_ms
                || active
                    .evidence
                    .iter()
                    .any(|record| record.captured_at_unix_ms > active.created_at_unix_ms)
                || state.last_completed_at_unix_ms.is_some_and(|last| {
                    active.created_at_unix_ms < last.saturating_add(MINIMUM_INTERVAL_MS)
                }))
        {
            return Err(Error::new(
                "fresh evidence-integration trigger violates cadence",
            ));
        }
    }

    let mut consumed_ids = BTreeSet::new();
    for record in &state.consumed {
        if !valid_short_identifier(&record.evidence_id)
            || !is_lower_hex(&record.sha256, 64)
            || record.consumed_at_unix_ms == 0
            || record.consumed_at_unix_ms > now_ms.saturating_add(60_000)
            || !consumed_ids.insert(record.evidence_id.as_str())
            || identities
                .insert(record.evidence_id.clone(), record.sha256.clone())
                .is_some()
        {
            return Err(Error::new(
                "consumed evidence-integration identity is invalid",
            ));
        }
    }
    let active_ids = state.active.as_ref().map_or_else(BTreeSet::new, |active| {
        active
            .evidence
            .iter()
            .map(|record| record.evidence_id.as_str())
            .collect()
    });
    for record in &state.rejected {
        if !is_lower_hex(&record.record_sha256, 64)
            || record
                .evidence_id
                .as_ref()
                .is_some_and(|value| !valid_short_identifier(value))
            || record
                .evidence_id
                .as_deref()
                .is_some_and(|value| active_ids.contains(value))
            || record.reason.is_empty()
            || record.reason.chars().count() > 160
            || record.reason.chars().any(char::is_control)
            || record.source_revision == 0
            || record.first_seen_at_unix_ms == 0
            || record.first_seen_at_unix_ms > now_ms.saturating_add(60_000)
        {
            return Err(Error::new(
                "rejected evidence-integration record is invalid",
            ));
        }
    }
    let mut ambiguous_triggers = BTreeSet::new();
    for record in &state.ambiguous {
        if !valid_identifier(&record.trigger_nonce)
            || !valid_integration_due_nonce(&record.due_nonce)
            || record.provider_started_at_unix_ms == 0
            || record.terminalized_at_unix_ms < record.provider_started_at_unix_ms
            || record.terminalized_at_unix_ms > now_ms.saturating_add(60_000)
            || record.evidence.is_empty()
            || record.evidence.len() > MAX_TRIGGER_EVIDENCE
            || record.status != "provider_started_delivery_authorship_unknown_non_authored"
            || !ambiguous_triggers.insert(record.trigger_nonce.as_str())
        {
            return Err(Error::new(
                "ambiguous evidence-integration record is invalid",
            ));
        }
        for evidence in &record.evidence {
            if !valid_short_identifier(&evidence.evidence_id)
                || !is_lower_hex(&evidence.sha256, 64)
                || evidence.consumed_at_unix_ms != record.terminalized_at_unix_ms
                || identities
                    .insert(evidence.evidence_id.clone(), evidence.sha256.clone())
                    .is_some()
            {
                return Err(Error::new(
                    "ambiguous evidence-integration identity is invalid",
                ));
            }
        }
    }
    if let Some(snapshot) = &state.scheduled_absorption {
        if !valid_identifier(&snapshot.scheduled_nonce)
            || snapshot.prepared_at_unix_ms == 0
            || snapshot.prepared_at_unix_ms > now_ms.saturating_add(60_000)
            || snapshot.evidence.len() > MAX_TRIGGER_EVIDENCE
        {
            return Err(Error::new(
                "scheduled evidence-integration absorption is invalid",
            ));
        }
        for record in &snapshot.evidence {
            validate_integration_evidence(record, now_ms)?;
            let occurrences = state
                .pending
                .iter()
                .chain(
                    state
                        .active
                        .iter()
                        .flat_map(|active| active.evidence.iter()),
                )
                .filter(|candidate| {
                    candidate.evidence_id == record.evidence_id && candidate.sha256 == record.sha256
                })
                .count();
            if occurrences != 1 {
                return Err(Error::new(
                    "scheduled evidence snapshot lacks one exact backing record",
                ));
            }
        }
    }
    Ok(())
}

fn validate_integration_evidence(record: &IntegrationEvidenceRecord, now_ms: u64) -> Result<()> {
    const ELIGIBLE_KINDS: &[&str] = &[
        "verified_source",
        "deterministic_measurement",
        "deterministic_check",
        "completed_study",
        "spectral_observation",
        "reservoir_tuning_result",
        "owned_self_study_result",
        "owned_artifact_read",
        "cited_synthesis",
        "verified_peer_packet",
    ];
    if !valid_short_identifier(&record.evidence_id)
        || !ELIGIBLE_KINDS.contains(&record.kind.as_str())
        || !valid_identifier(&record.epistemic_status)
        || !is_lower_hex(&record.sha256, 64)
        || !record.eligible_for_belief_update
        || record.captured_at_unix_ms == 0
        || record.captured_at_unix_ms > now_ms.saturating_add(60_000)
    {
        return Err(Error::new("integration evidence identity is invalid"));
    }
    for (value, maximum) in [
        (&record.reference, 512),
        (&record.summary, 480),
        (&record.source, 256),
    ] {
        if value.is_empty()
            || value.trim() != value
            || value.chars().count() > maximum
            || value.chars().any(char::is_control)
        {
            return Err(Error::new("integration evidence text is invalid"));
        }
    }
    Ok(())
}

fn insert_unique_integration_identity(
    identities: &mut BTreeMap<String, String>,
    record: &IntegrationEvidenceRecord,
) -> Result<()> {
    if identities
        .insert(record.evidence_id.clone(), record.sha256.clone())
        .is_some()
    {
        return Err(Error::new("integration evidence identity is duplicated"));
    }
    Ok(())
}

fn derive_integration_identity(
    appliance_id: &str,
    generation: u64,
    evidence: &[IntegrationEvidenceRecord],
) -> Result<(String, String)> {
    let mut preimage = b"astrid.edge.evidence-integration.trigger.v1\0".to_vec();
    preimage.extend_from_slice(appliance_id.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(&generation.to_be_bytes());
    preimage.push(0);
    preimage.extend_from_slice(&canonical_json(&evidence)?);
    let digest = sha256(&preimage);
    let numeric = u64::from_str_radix(&digest[..16], 16)
        .map_err(|_| Error::new("integration trigger digest is invalid"))?
        | 0x8000_0000_0000_0000;
    Ok((
        format!("evidence-integration-{digest}"),
        format!("due-{numeric}"),
    ))
}

fn valid_short_identifier(value: &str) -> bool {
    value.len() <= 96 && valid_identifier(value)
}

fn valid_integration_due_nonce(value: &str) -> bool {
    valid_identifier(value)
        && value.strip_prefix("due-").is_some_and(|suffix| {
            (5..=20).contains(&suffix.len()) && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
}

const fn integration_utc_day(now_ms: u64) -> u64 {
    now_ms / (24 * 60 * 60 * 1_000)
}

fn schedule_content_admission(bytes: &[u8], now: u64) -> Result<ScheduleAdmission> {
    const INTERVAL_SECONDS: u64 = 2 * 60 * 60;
    let parsed = serde_json::from_slice::<Value>(bytes)?;
    let schema = parsed
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("scheduled-reflection state schema is absent"))?;
    if schema == "astrid.edge.steward_helper.schedule.v1" {
        let state: ScheduleStateV1 = serde_json::from_slice(bytes)?;
        let _ = state.completed_count;
        if state.schema != schema
            || state.next_due_at_unix_seconds == 0
            || state.next_due_at_unix_seconds > now.saturating_add(INTERVAL_SECONDS)
            || state
                .pending_due_at_unix_seconds
                .is_some_and(|pending| pending != state.next_due_at_unix_seconds || pending > now)
            || state
                .last_completed_at_unix_seconds
                .is_some_and(|completed| completed > now.saturating_add(60))
        {
            return Err(Error::new("scheduled-reflection v1 state content failed"));
        }
        // Admit one legacy pending slot so the immutable steward can migrate it
        // to v2. The steward conservatively creates a fresh two-hour floor and
        // exits without a provider call.
        return if now >= state.next_due_at_unix_seconds {
            let due_nonce = format!("due-{}", state.next_due_at_unix_seconds);
            if state.pending_due_at_unix_seconds.is_some() {
                Ok(ScheduleAdmission::LegacyMigration { due_nonce })
            } else {
                Ok(ScheduleAdmission::ModelStart { due_nonce })
            }
        } else {
            Ok(ScheduleAdmission::NotDue)
        };
    }
    if schema != "astrid.edge.steward_helper.schedule.v2" {
        return Err(Error::new("scheduled-reflection state schema is invalid"));
    }
    let state: ScheduleStateV2 = serde_json::from_slice(bytes)?;
    let _ = state.completed_count;
    if state.schema != schema
        || state.next_due_at_unix_seconds == 0
        || state.next_due_at_unix_seconds > now.saturating_add(INTERVAL_SECONDS)
        || state
            .pending_due_at_unix_seconds
            .is_some_and(|pending| pending != state.next_due_at_unix_seconds || pending > now)
        || state
            .last_completed_at_unix_seconds
            .is_some_and(|completed| completed > now.saturating_add(60))
        || state
            .last_model_started_at_unix_seconds
            .is_some_and(|started| started > now.saturating_add(60))
        || match state.last_model_started_at_unix_seconds {
            Some(started) => {
                state.next_model_eligible_at_unix_seconds
                    != started.saturating_add(INTERVAL_SECONDS)
                    || state.model_start_count == 0
            },
            None => state.next_model_eligible_at_unix_seconds != 0 || state.model_start_count != 0,
        }
    {
        return Err(Error::new("scheduled-reflection v2 state content failed"));
    }
    if now < state.next_due_at_unix_seconds {
        return Ok(ScheduleAdmission::NotDue);
    }
    let due = state
        .pending_due_at_unix_seconds
        .unwrap_or(state.next_due_at_unix_seconds);
    let due_nonce = format!("due-{due}");
    if now < state.next_model_eligible_at_unix_seconds {
        return Ok(ScheduleAdmission::Cooling { due_nonce });
    }
    Ok(ScheduleAdmission::ModelStart { due_nonce })
}

fn exact_prepared_transaction_exists(
    config: &Config,
    schedule: &Path,
    due_nonce: &str,
) -> Result<bool> {
    let state_root = schedule
        .parent()
        .ok_or_else(|| Error::new("scheduled-reflection state root is absent"))?;
    let root = state_root.join("authored-transactions");
    let root_metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || root_metadata.uid() != config.identities.steward_uid
        || root_metadata.gid() != config.identities.steward_gid
        || root_metadata.mode() & 0o777 != 0o700
    {
        return Err(Error::new("authored recovery root identity failed"));
    }
    let path = root.join(format!("{due_nonce}.json"));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != config.identities.steward_uid
        || metadata.gid() != config.identities.steward_gid
        || metadata.mode() & 0o777 != 0o600
        || metadata.len() == 0
        || metadata.len() > 2 * 1024 * 1024
    {
        return Err(Error::new("authored recovery transaction identity failed"));
    }
    Ok(true)
}

fn cleanup_inner(
    config: &Config,
    paths: &GuardPaths,
    boot_id: &str,
    invocation_id: &str,
    root_uid: u32,
) -> Result<()> {
    let _mutex = acquire_mutex(config, paths, root_uid)?;
    let lease = read_exact_lease(config, paths, root_uid)?;
    let marker = read_exact_marker(config, paths, root_uid)?;
    match (lease, marker) {
        (None, None) => Ok(()),
        (Some((lease, bytes)), marker) => {
            if lease.host_boot_id != boot_id || lease.service_invocation_id != invocation_id {
                return Err(Error::new("reflection cleanup invocation binding failed"));
            }
            if let Some((marker, marker_bytes)) = marker {
                if validate_marker_binding(&lease, &bytes, &marker).is_err()
                    || marker.host_boot_id != boot_id
                    || marker.service_invocation_id != invocation_id
                {
                    return Err(Error::new("reflection admission cleanup binding failed"));
                }
                remove_exact(&paths.admission, Some(&marker_bytes), root_uid)?;
            }
            remove_exact(&paths.lease, Some(&bytes), root_uid)
        },
        (None, Some(_)) => Err(Error::new("reflection admission exists without its lease")),
    }
}

fn reconcile_inner(
    config: &Config,
    paths: &GuardPaths,
    boot_id: &str,
    root_uid: u32,
) -> Result<()> {
    let _mutex = acquire_mutex(config, paths, root_uid)?;
    let lease = read_exact_lease(config, paths, root_uid)?;
    let marker = read_exact_marker(config, paths, root_uid)?;
    match (&lease, &marker) {
        (Some((lease, lease_bytes)), Some((marker, _))) => {
            validate_marker_binding(lease, lease_bytes, marker)?;
        },
        (None, Some(_)) => {
            return Err(Error::new(
                "prior-boot reflection admission exists without its lease",
            ));
        },
        (Some(_) | None, None) => {},
    }
    require_prior_boot_artifacts(
        lease.as_ref().map(|(lease, _)| lease),
        marker.as_ref().map(|(marker, _)| marker),
        boot_id,
    )?;
    if let Some((_, bytes)) = marker {
        remove_exact(&paths.admission, Some(&bytes), root_uid)?;
    }
    if let Some((_, bytes)) = lease {
        remove_exact(&paths.lease, Some(&bytes), root_uid)?;
    }
    Ok(())
}

/// Turn steward-authored inert markers into root-owned supervisor wakeups.
///
/// This function intentionally performs no candidate ingestion or signature
/// decision.  Its only authority is the filename transition from an unwatched
/// `.pending` suffix to the watched `.json` suffix after reflection cleanup.
#[allow(
    clippy::too_many_arguments,
    clippy::similar_names,
    reason = "explicit ownership identities keep the cross-identity handoff reviewable"
)]
fn promote_pending_supervisor_handoffs(
    inbox: &Path,
    appliance_id: &str,
    steward_uid: u32,
    steward_gid: u32,
    rescue_uid: u32,
    rescue_gid: u32,
) -> Result<usize> {
    if !valid_identifier(appliance_id) {
        return Err(Error::new("pending handoff appliance identity is invalid"));
    }
    let inbox_metadata = fs::symlink_metadata(inbox)?;
    if !inbox_metadata.is_dir()
        || inbox_metadata.file_type().is_symlink()
        || inbox_metadata.uid() != steward_uid
        || inbox_metadata.gid() != steward_gid
        || inbox_metadata.mode() & 0o777 != 0o700
    {
        return Err(Error::new(
            "supervisor inbox is not the exact steward-owned mode-0700 directory",
        ));
    }

    let mut pending = Vec::new();
    for entry in fs::read_dir(inbox)? {
        let entry = entry?;
        let Some(envelope_id) = pending_handoff_envelope_id(&entry.file_name())? else {
            continue;
        };
        pending.push((entry.path(), envelope_id));
        if pending.len() > MAXIMUM_PENDING_HANDOFFS {
            return Err(Error::new("too many pending supervisor handoffs"));
        }
    }
    pending.sort_by(|left, right| left.1.cmp(&right.1));

    let mut promoted = 0_usize;
    for (source, filename_envelope_id) in pending {
        let metadata = fs::symlink_metadata(&source)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.nlink() != 1
            || metadata.uid() != steward_uid
            || metadata.gid() != steward_gid
            || metadata.mode() & 0o777 != 0o600
            || metadata.len() == 0
            || metadata.len() > MAXIMUM_HANDOFF_BYTES
        {
            return Err(Error::new("pending supervisor handoff identity failed"));
        }
        let bytes = read_regular(&source, MAXIMUM_HANDOFF_BYTES)?;
        let marker: PendingSupervisorHandoff = serde_json::from_slice(&bytes)?;
        validate_pending_supervisor_handoff(&marker, appliance_id, &filename_envelope_id)?;
        let mut canonical = canonical_json(&marker)?;
        canonical.push(b'\n');
        if bytes != canonical {
            return Err(Error::new(
                "pending supervisor handoff is not exact canonical JSON",
            ));
        }

        let destination = inbox.join(format!("candidate-ready-{filename_envelope_id}.json"));
        match fs::symlink_metadata(&destination) {
            Ok(destination_metadata) => {
                if !destination_metadata.is_file()
                    || destination_metadata.file_type().is_symlink()
                    || destination_metadata.nlink() != 1
                    || destination_metadata.uid() != rescue_uid
                    || destination_metadata.gid() != rescue_gid
                    || destination_metadata.mode() & 0o777 != 0o600
                    || read_regular(&destination, MAXIMUM_HANDOFF_BYTES)? != bytes
                {
                    return Err(Error::new(
                        "watched supervisor handoff collides with inexact content",
                    ));
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                atomic_owned_write(&destination, &bytes, 0o600, rescue_uid, rescue_gid, false)?;
            },
            Err(error) => return Err(error.into()),
        }
        remove_exact(&source, Some(&bytes), steward_uid)?;
        promoted = promoted.saturating_add(1);
    }
    Ok(promoted)
}

fn pending_handoff_envelope_id(name: &std::ffi::OsStr) -> Result<Option<String>> {
    use std::os::unix::ffi::OsStrExt as _;

    const PREFIX: &[u8] = b"candidate-ready-";
    const SUFFIX: &[u8] = b".pending";
    let bytes = name.as_bytes();
    let Some(identifier) = bytes
        .strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix(SUFFIX))
    else {
        return Ok(None);
    };
    let value = std::str::from_utf8(identifier)
        .map_err(|_| Error::new("pending supervisor handoff filename is not UTF-8"))?;
    if !valid_identifier(value) {
        return Err(Error::new(
            "pending supervisor handoff filename identity is invalid",
        ));
    }
    Ok(Some(value.to_owned()))
}

fn validate_pending_supervisor_handoff(
    marker: &PendingSupervisorHandoff,
    appliance_id: &str,
    filename_envelope_id: &str,
) -> Result<()> {
    if marker.schema != HANDOFF_SCHEMA
        || marker.appliance_id != appliance_id
        || marker.envelope_id != filename_envelope_id
        || !valid_identifier(&marker.envelope_id)
        || !valid_identifier(&marker.intent_id)
        || !valid_identifier(&marker.candidate_id)
        || !is_lower_hex(&marker.envelope_sha256, 64)
        || !is_lower_hex(&marker.candidate_sha256, 64)
        || !is_lower_hex(&marker.response_sha256, 64)
        || marker.created_at == 0
        || marker.provenance != HANDOFF_PROVENANCE
        || marker.authority != HANDOFF_AUTHORITY
    {
        return Err(Error::new("pending supervisor handoff content is invalid"));
    }
    Ok(())
}

fn validate_marker_binding(
    lease: &ReflectionLease,
    lease_bytes: &[u8],
    marker: &AdmissionMarker,
) -> Result<()> {
    if marker.lease_id != lease.lease_id
        || marker.lease_nonce_sha256 != sha256(lease.nonce.as_bytes())
        || marker.lease_payload_sha256 != sha256(lease_bytes)
        || marker.generation_id != lease.generation_id
        || marker.host_boot_id != lease.host_boot_id
        || marker.service_invocation_id != lease.service_invocation_id
    {
        return Err(Error::new(
            "reflection admission does not bind its exact lease",
        ));
    }
    Ok(())
}

fn require_prior_boot_artifacts(
    lease: Option<&ReflectionLease>,
    marker: Option<&AdmissionMarker>,
    boot_id: &str,
) -> Result<()> {
    if lease.is_some_and(|lease| lease.host_boot_id == boot_id)
        || marker.is_some_and(|marker| marker.host_boot_id == boot_id)
    {
        return Err(Error::new(
            "current-boot reflection admission cannot be reconciled",
        ));
    }
    Ok(())
}

fn read_exact_lease(
    config: &Config,
    paths: &GuardPaths,
    root_uid: u32,
) -> Result<Option<(ReflectionLease, Vec<u8>)>> {
    let Some(bytes) =
        read_optional_owned(&paths.lease, root_uid, config.identities.runtime_gid, 0o440)?
    else {
        return Ok(None);
    };
    let lease: ReflectionLease = serde_json::from_slice(&bytes)?;
    validate_lease_content(&lease)?;
    Ok(Some((lease, bytes)))
}

fn read_exact_marker(
    config: &Config,
    paths: &GuardPaths,
    root_uid: u32,
) -> Result<Option<(AdmissionMarker, Vec<u8>)>> {
    let Some(bytes) = read_optional_owned(
        &paths.admission,
        root_uid,
        config.identities.steward_gid,
        0o440,
    )?
    else {
        return Ok(None);
    };
    let marker: AdmissionMarker = serde_json::from_slice(&bytes)?;
    if marker.schema != ADMISSION_SCHEMA
        || marker.lease_schema != LEASE_SCHEMA
        || marker.lease_kind != LEASE_KIND
        || marker.authority != "root_verified_drain_and_model_lock_handoff_not_activation_authority"
        || !is_lower_hex(&marker.lease_nonce_sha256, 64)
        || !is_lower_hex(&marker.lease_payload_sha256, 64)
        || !is_lower_hex(&marker.core_ack_sha256, 64)
        || !is_lower_hex(&marker.edge_ack_sha256, 64)
        || marker
            .reflection_due_nonce_sha256
            .as_ref()
            .is_none_or(|value| !is_lower_hex(value, 64))
        || marker
            .reflection_trigger_nonce_sha256
            .as_ref()
            .is_some_and(|value| !is_lower_hex(value, 64))
        || !valid_reflection_authority(&marker)
        || marker.drain_barrier_sequence == 0
    {
        return Err(Error::new("reflection admission marker is malformed"));
    }
    Ok(Some((marker, bytes)))
}

fn valid_reflection_authority(marker: &AdmissionMarker) -> bool {
    match marker.reflection_kind.as_str() {
        "scheduled" => {
            marker.reflection_trigger_nonce_sha256.is_none()
                && matches!(
                    marker.model_start_authority.as_str(),
                    "root_schedule_model_start_allowed"
                        | "root_schedule_prepared_recovery_only"
                        | "root_schedule_legacy_migration_only"
                )
        },
        "evidence_integration" => {
            marker.reflection_trigger_nonce_sha256.is_some()
                && matches!(
                    marker.model_start_authority.as_str(),
                    "root_evidence_integration_model_start_allowed"
                        | "root_evidence_integration_prepared_recovery_only"
                )
        },
        _ => false,
    }
}

fn validate_lease_content(lease: &ReflectionLease) -> Result<()> {
    if lease.schema != LEASE_SCHEMA
        || lease.lease_kind != LEASE_KIND
        || lease.owner != LEASE_OWNER
        || lease.reason != "scheduled_reflection"
        || lease.expires_at_unix_ms <= lease.created_at_unix_ms
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            != LEASE_LIFETIME_MS
        || !is_lower_hex(&lease.nonce, 64)
        || !is_lower_hex(&lease.service_invocation_id, 32)
        || !valid_boot_id(&lease.host_boot_id)
        || !valid_identifier(&lease.generation_id)
        || lease.lease_id != format!("reflection-{}", &sha256(lease.nonce.as_bytes())[..24])
    {
        return Err(Error::new("scheduled-reflection lease is malformed"));
    }
    Ok(())
}

fn read_optional_owned(path: &Path, uid: u32, gid: u32, mode: u32) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != uid
        || metadata.gid() != gid
        || metadata.mode() & 0o777 != mode
    {
        return Err(Error::new("reflection artifact ownership or mode failed"));
    }
    Ok(Some(read_regular(path, MAXIMUM_BYTES)?))
}

fn atomic_owned_write(
    path: &Path,
    bytes: &[u8],
    mode: u32,
    uid: u32,
    gid: u32,
    replace: bool,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("reflection output has no parent"))?;
    let temporary = parent.join(format!(".reflection-{}.partial", &sha256(bytes)[..24]));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(no_follow_cloexec())
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        nix::unistd::fchown(
            &file,
            Some(nix::unistd::Uid::from_raw(uid)),
            Some(nix::unistd::Gid::from_raw(gid)),
        )
        .map_err(|error| Error::new(format!("cannot bind reflection group: {error}")))?;
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        file.sync_all()?;
        drop(file);
        rename_no_replace(parent, &temporary, path, replace)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn rename_no_replace(
    parent: &Path,
    temporary: &Path,
    destination: &Path,
    replace: bool,
) -> Result<()> {
    use nix::fcntl::{RenameFlags, renameat2};

    if replace {
        fs::rename(temporary, destination)?;
        return Ok(());
    }
    let directory = File::open(parent)?;
    renameat2(
        &directory,
        temporary
            .file_name()
            .ok_or_else(|| Error::new("temporary basename absent"))?,
        &directory,
        destination
            .file_name()
            .ok_or_else(|| Error::new("destination basename absent"))?,
        RenameFlags::RENAME_NOREPLACE,
    )
    .map_err(|error| Error::new(format!("reflection no-replace rename failed: {error}")))
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn rename_no_replace(
    _parent: &Path,
    temporary: &Path,
    destination: &Path,
    replace: bool,
) -> Result<()> {
    if !replace && (destination.exists() || destination.is_symlink()) {
        return Err(Error::new("refusing to replace reflection artifact"));
    }
    fs::rename(temporary, destination).map_err(Into::into)
}

fn remove_exact(path: &Path, expected: Option<&[u8]>, uid: u32) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != uid
    {
        return Err(Error::new("refusing to remove inexact reflection artifact"));
    }
    if expected.is_some_and(|bytes| {
        read_regular(path, MAXIMUM_BYTES).map_or(true, |actual| actual != bytes)
    }) {
        return Err(Error::new("reflection artifact changed before cleanup"));
    }
    fs::remove_file(path)?;
    File::open(
        path.parent()
            .ok_or_else(|| Error::new("reflection parent absent"))?,
    )?
    .sync_all()?;
    Ok(())
}

fn reject_present(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(Error::deferred(format!("{label} is already present"))),
        Err(error) => Err(error.into()),
    }
}

fn validate_shared_parent(path: &Path, root_uid: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("reflection path has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != root_uid
        || metadata.gid() != root_uid
        || metadata.mode() & 0o777 != 0o755
    {
        return Err(Error::new("reflection parent is not root-owned mode 0755"));
    }
    Ok(())
}

fn read_generation(path: &Path, root_uid: u32) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != root_uid
        || metadata.mode() & 0o777 != 0o444
    {
        return Err(Error::new("reflection generation binding identity failed"));
    }
    let bytes = read_regular(path, 256)?;
    let generation = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|value| value.strip_suffix('\n'))
        .filter(|value| valid_identifier(value))
        .ok_or_else(|| Error::new("reflection generation binding is not canonical"))?;
    Ok(generation.to_owned())
}

fn production_context() -> Result<Context> {
    Ok(Context {
        now_ms: unix_millis(),
        boot_id: current_boot_id()?,
        invocation_id: current_invocation_id()?,
        nonce: random_nonce()?,
    })
}

fn current_invocation_id() -> Result<String> {
    std::env::var("INVOCATION_ID").map_err(|_| Error::new("systemd INVOCATION_ID is absent"))
}

fn validate_context(context: &Context) -> Result<()> {
    if context.now_ms == 0
        || !valid_boot_id(&context.boot_id)
        || !is_lower_hex(&context.invocation_id, 32)
        || !is_lower_hex(&context.nonce, 64)
    {
        return Err(Error::new(
            "scheduled-reflection invocation context is invalid",
        ));
    }
    Ok(())
}

fn current_boot_id() -> Result<String> {
    let value = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_owned();
    if !valid_boot_id(&value) {
        return Err(Error::new("host boot identity is malformed"));
    }
    Ok(value)
}

fn random_nonce() -> Result<String> {
    use std::fmt::Write as _;
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").map_err(|_| Error::new("nonce encoding failed"))?;
    }
    Ok(value)
}

fn require_root() -> Result<()> {
    if nix::unistd::geteuid().as_raw() != 0 {
        return Err(Error::new(
            "scheduled-reflection guard requires effective uid 0",
        ));
    }
    Ok(())
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
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

#[cfg(target_os = "linux")]
const fn no_follow_cloexec() -> i32 {
    nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC
}

#[cfg(not(target_os = "linux"))]
const fn no_follow_cloexec() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::{
        ADMISSION_SCHEMA, AdmissionMarker, CORE_ACK_KEYS, HANDOFF_AUTHORITY, HANDOFF_PROVENANCE,
        HANDOFF_SCHEMA, IntegrationActiveTrigger, IntegrationEvidenceRecord, IntegrationStateV1,
        LEASE_KIND, LEASE_SCHEMA, PendingSupervisorHandoff, ReflectionLease,
        derive_integration_identity, has_exact_object_keys, integration_utc_day, is_lower_hex,
        promote_pending_supervisor_handoffs, read_evidence_integration_state,
        require_prior_boot_artifacts, schedule_content_admission, valid_reflection_authority,
        validate_integration_state, validate_lease_content, validate_marker_binding,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    fn valid_lease() -> ReflectionLease {
        let nonce = "a".repeat(64);
        ReflectionLease {
            schema: LEASE_SCHEMA.to_owned(),
            lease_kind: LEASE_KIND.to_owned(),
            created_at_unix_ms: 1,
            expires_at_unix_ms: 1 + super::LEASE_LIFETIME_MS,
            reason: "scheduled_reflection".to_owned(),
            owner: super::LEASE_OWNER.to_owned(),
            lease_id: format!("reflection-{}", &super::sha256(nonce.as_bytes())[..24]),
            nonce,
            host_boot_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            service_invocation_id: "b".repeat(32),
            generation_id: "generation-1".to_owned(),
        }
    }

    fn valid_marker() -> AdmissionMarker {
        AdmissionMarker {
            schema: ADMISSION_SCHEMA.to_owned(),
            lease_schema: LEASE_SCHEMA.to_owned(),
            lease_kind: LEASE_KIND.to_owned(),
            lease_id: "reflection-example".to_owned(),
            lease_nonce_sha256: "a".repeat(64),
            lease_payload_sha256: "b".repeat(64),
            generation_id: "generation-1".to_owned(),
            host_boot_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            service_invocation_id: "c".repeat(32),
            admitted_at_unix_ms: 1,
            drain_barrier_sequence: 1,
            core_ack_sha256: "d".repeat(64),
            edge_ack_sha256: "e".repeat(64),
            model_lock_device: 1,
            model_lock_inode: 2,
            reflection_kind: "scheduled".to_owned(),
            reflection_due_nonce_sha256: Some("f".repeat(64)),
            reflection_trigger_nonce_sha256: None,
            model_start_authority: "root_schedule_model_start_allowed".to_owned(),
            authority: "root_verified_drain_and_model_lock_handoff_not_activation_authority"
                .to_owned(),
        }
    }

    fn valid_handoff() -> PendingSupervisorHandoff {
        PendingSupervisorHandoff {
            schema: HANDOFF_SCHEMA.to_owned(),
            appliance_id: "appliance-test".to_owned(),
            envelope_id: "envelope-test".to_owned(),
            envelope_sha256: "a".repeat(64),
            intent_id: "intent-test".to_owned(),
            candidate_id: "candidate-test".to_owned(),
            candidate_sha256: "b".repeat(64),
            response_sha256: "c".repeat(64),
            created_at: 1,
            provenance: HANDOFF_PROVENANCE.to_owned(),
            authority: HANDOFF_AUTHORITY.to_owned(),
        }
    }

    fn handoff_fixture() -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>, u32, u32) {
        let temp = tempfile::tempdir().unwrap();
        let inbox = temp.path().join("inbox");
        fs::create_dir(&inbox).unwrap();
        fs::set_permissions(&inbox, fs::Permissions::from_mode(0o700)).unwrap();
        let mut bytes = super::canonical_json(&valid_handoff()).unwrap();
        bytes.push(b'\n');
        let pending = inbox.join("candidate-ready-envelope-test.pending");
        fs::write(&pending, &bytes).unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = nix::unistd::geteuid().as_raw();
        let gid = nix::unistd::getegid().as_raw();
        (temp, inbox, bytes, uid, gid)
    }

    fn integration_evidence(now_ms: u64) -> IntegrationEvidenceRecord {
        IntegrationEvidenceRecord {
            evidence_id: "evidence-1".to_owned(),
            kind: "verified_source".to_owned(),
            epistemic_status: "verified_external_evidence".to_owned(),
            reference: "research/source-example.md".to_owned(),
            summary: "A bounded verified source changed the open inquiry.".to_owned(),
            source: "runtime_v7_exact_evidence".to_owned(),
            captured_at_unix_ms: now_ms.saturating_sub(6 * 60 * 1_000),
            sha256: "a".repeat(64),
            eligible_for_belief_update: true,
        }
    }

    fn integration_state(appliance_id: &str, now_ms: u64) -> IntegrationStateV1 {
        let evidence = vec![integration_evidence(now_ms)];
        let generation = 7;
        let (trigger_nonce, due_nonce) =
            derive_integration_identity(appliance_id, generation, &evidence).unwrap();
        IntegrationStateV1 {
            schema: "astrid.edge.steward_helper.evidence_integration_state.v1".to_owned(),
            generation,
            pending: Vec::new(),
            quiet_until_unix_ms: now_ms.saturating_sub(60_000),
            active: Some(IntegrationActiveTrigger {
                trigger_nonce,
                due_nonce,
                generation,
                created_at_unix_ms: now_ms.saturating_sub(30_000),
                last_attempt_at_unix_ms: None,
                evidence,
            }),
            consumed: Vec::new(),
            rejected: Vec::new(),
            ambiguous: Vec::new(),
            scheduled_absorption: None,
            last_completed_at_unix_ms: None,
            last_finished_trigger_nonce: None,
            last_finished_due_nonce: None,
            last_absorbed_scheduled_nonce: None,
            utc_day: integration_utc_day(now_ms),
            starts_today: 0,
            last_source_revision: Some(1),
            last_source_sha256: Some("b".repeat(64)),
        }
    }

    #[test]
    fn reflection_schema_is_distinct_and_nonce_bound() {
        let lease = valid_lease();
        validate_lease_content(&lease).unwrap();
        let mut wrong = lease;
        wrong.lease_kind = "generation_transition".to_owned();
        assert!(validate_lease_content(&wrong).is_err());
    }

    #[test]
    fn admission_hash_fields_are_exact_lower_hex() {
        let marker = valid_marker();
        let value = serde_json::to_value(marker).unwrap();
        assert_eq!(value["lease_kind"], LEASE_KIND);
        assert!(is_lower_hex(value["core_ack_sha256"].as_str().unwrap(), 64));
    }

    #[test]
    fn admission_v3_binds_kind_due_trigger_and_authority() {
        let scheduled = valid_marker();
        assert!(valid_reflection_authority(&scheduled));

        let mut evidence = scheduled.clone();
        evidence.reflection_kind = "evidence_integration".to_owned();
        evidence.reflection_trigger_nonce_sha256 = Some("0".repeat(64));
        evidence.model_start_authority = "root_evidence_integration_model_start_allowed".to_owned();
        assert!(valid_reflection_authority(&evidence));

        evidence.reflection_trigger_nonce_sha256 = None;
        assert!(!valid_reflection_authority(&evidence));
        evidence.reflection_trigger_nonce_sha256 = Some("0".repeat(64));
        evidence.model_start_authority = "root_schedule_model_start_allowed".to_owned();
        assert!(!valid_reflection_authority(&evidence));
    }

    #[test]
    fn root_independently_rederives_fresh_evidence_trigger() {
        let now_ms = 1_900_000_000_000;
        let state = integration_state("avado-astrid", now_ms);
        validate_integration_state("avado-astrid", &state, now_ms).unwrap();

        let mut wrong_appliance = state;
        assert!(validate_integration_state("icp-astrid", &wrong_appliance, now_ms).is_err());
        wrong_appliance.active.as_mut().unwrap().trigger_nonce =
            format!("evidence-integration-{}", "c".repeat(64));
        assert!(validate_integration_state("avado-astrid", &wrong_appliance, now_ms).is_err());
    }

    #[test]
    fn root_rejects_integration_cadence_replay_and_untrusted_evidence() {
        let now_ms = 1_900_000_000_000;
        let mut state = integration_state("avado-astrid", now_ms);
        state.quiet_until_unix_ms = now_ms;
        assert!(validate_integration_state("avado-astrid", &state, now_ms).is_err());

        let mut state = integration_state("avado-astrid", now_ms);
        state
            .pending
            .push(state.active.as_ref().unwrap().evidence[0].clone());
        assert!(validate_integration_state("avado-astrid", &state, now_ms).is_err());

        let mut state = integration_state("avado-astrid", now_ms);
        state.active.as_mut().unwrap().evidence[0].eligible_for_belief_update = false;
        assert!(validate_integration_state("avado-astrid", &state, now_ms).is_err());

        let mut state = integration_state("avado-astrid", now_ms);
        state.active.as_mut().unwrap().evidence[0].kind = "search_candidate".to_owned();
        assert!(validate_integration_state("avado-astrid", &state, now_ms).is_err());
    }

    #[test]
    fn integration_state_unknown_fields_and_noncanonical_bytes_fail() {
        let now_ms = 1_900_000_000_000;
        let state = integration_state("avado-astrid", now_ms);
        let canonical = super::canonical_json(&state).unwrap();
        let parsed: IntegrationStateV1 = serde_json::from_slice(&canonical).unwrap();
        assert_eq!(super::canonical_json(&parsed).unwrap(), canonical);

        let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
        value["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<IntegrationStateV1>(value).is_err());

        let mut whitespace = canonical;
        whitespace.push(b'\n');
        let parsed: IntegrationStateV1 = serde_json::from_slice(&whitespace).unwrap();
        assert_ne!(super::canonical_json(&parsed).unwrap(), whitespace);
    }

    #[test]
    fn integration_state_file_rejects_links_modes_and_noncanonical_replacement() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("steward-state");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("evidence-integration.json");
        let bytes =
            super::canonical_json(&integration_state("avado-astrid", 1_900_000_000_000)).unwrap();
        fs::write(&path, &bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = nix::unistd::geteuid().as_raw();
        let gid = nix::unistd::getegid().as_raw();
        assert!(
            read_evidence_integration_state(&path, uid, gid, gid)
                .unwrap()
                .is_some()
        );

        let hardlink = root.join("evidence-integration-hardlink.json");
        fs::hard_link(&path, &hardlink).unwrap();
        assert!(read_evidence_integration_state(&path, uid, gid, gid).is_err());
        fs::remove_file(&hardlink).unwrap();

        let symlink = root.join("evidence-integration-symlink.json");
        std::os::unix::fs::symlink(&path, &symlink).unwrap();
        assert!(read_evidence_integration_state(&symlink, uid, gid, gid).is_err());
        fs::remove_file(&symlink).unwrap();

        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(read_evidence_integration_state(&path, uid, gid, gid).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut noncanonical = bytes;
        noncanonical.push(b'\n');
        fs::write(&path, noncanonical).unwrap();
        assert!(read_evidence_integration_state(&path, uid, gid, gid).is_err());
    }

    #[test]
    fn acknowledgement_key_set_rejects_missing_and_extra_fields() {
        let mut object = serde_json::Map::new();
        for key in CORE_ACK_KEYS {
            object.insert((*key).to_owned(), serde_json::Value::Null);
        }
        let mut value = serde_json::Value::Object(object);
        assert!(has_exact_object_keys(&value, CORE_ACK_KEYS));
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), serde_json::Value::Null);
        assert!(!has_exact_object_keys(&value, CORE_ACK_KEYS));
        value.as_object_mut().unwrap().remove("unexpected");
        value.as_object_mut().unwrap().remove("active_llm_requests");
        assert!(!has_exact_object_keys(&value, CORE_ACK_KEYS));
    }

    #[test]
    fn reboot_reconciliation_accepts_only_prior_boot_artifacts() {
        let mut lease = valid_lease();
        let mut marker = valid_marker();
        let current = "11111111-1111-4111-8111-111111111111";
        assert!(require_prior_boot_artifacts(Some(&lease), Some(&marker), current).is_err());

        lease.host_boot_id = "22222222-2222-4222-8222-222222222222".to_owned();
        marker.host_boot_id = "22222222-2222-4222-8222-222222222222".to_owned();
        assert!(require_prior_boot_artifacts(Some(&lease), Some(&marker), current).is_ok());
    }

    #[test]
    fn stale_boot_marker_must_bind_the_exact_lease_before_removal() {
        let lease = valid_lease();
        let lease_bytes = super::canonical_json(&lease).unwrap();
        let mut marker = valid_marker();
        marker.lease_id.clone_from(&lease.lease_id);
        marker.lease_nonce_sha256 = super::sha256(lease.nonce.as_bytes());
        marker.lease_payload_sha256 = super::sha256(&lease_bytes);
        marker.generation_id.clone_from(&lease.generation_id);
        marker.host_boot_id.clone_from(&lease.host_boot_id);
        marker
            .service_invocation_id
            .clone_from(&lease.service_invocation_id);
        assert!(validate_marker_binding(&lease, &lease_bytes, &marker).is_ok());

        marker.lease_id = "reflection-other".to_owned();
        assert!(validate_marker_binding(&lease, &lease_bytes, &marker).is_err());
    }

    #[test]
    fn root_admission_honors_v2_model_start_floor_and_pending_nonce() {
        let cooling = serde_json::json!({
            "schema": "astrid.edge.steward_helper.schedule.v2",
            "next_due_at_unix_seconds": 10_000,
            "pending_due_at_unix_seconds": 10_000,
            "last_completed_at_unix_seconds": null,
            "last_model_started_at_unix_seconds": 10_010,
            "next_model_eligible_at_unix_seconds": 17_210,
            "completed_count": 0,
            "model_start_count": 1
        });
        let bytes = serde_json::to_vec(&cooling).unwrap();
        assert!(matches!(
            schedule_content_admission(&bytes, 17_209).unwrap(),
            super::ScheduleAdmission::Cooling { .. }
        ));
        assert!(matches!(
            schedule_content_admission(&bytes, 17_210).unwrap(),
            super::ScheduleAdmission::ModelStart { .. }
        ));

        let mut tampered = cooling;
        tampered["next_model_eligible_at_unix_seconds"] = serde_json::json!(10_011);
        assert!(
            schedule_content_admission(&serde_json::to_vec(&tampered).unwrap(), 10_011).is_err()
        );
    }

    #[test]
    fn root_admission_allows_one_legacy_due_pass_for_safe_migration() {
        let legacy = br#"{"schema":"astrid.edge.steward_helper.schedule.v1","next_due_at_unix_seconds":10000,"pending_due_at_unix_seconds":10000,"last_completed_at_unix_seconds":null,"completed_count":0}"#;
        assert!(matches!(
            schedule_content_admission(legacy, 10_100).unwrap(),
            super::ScheduleAdmission::LegacyMigration { .. }
        ));

        let fresh_legacy = br#"{"schema":"astrid.edge.steward_helper.schedule.v1","next_due_at_unix_seconds":10000,"pending_due_at_unix_seconds":null,"last_completed_at_unix_seconds":null,"completed_count":0}"#;
        assert!(matches!(
            schedule_content_admission(fresh_legacy, 10_100).unwrap(),
            super::ScheduleAdmission::ModelStart { .. }
        ));
    }

    #[test]
    fn root_promotes_only_exact_pending_handoff_after_cleanup_boundary() {
        let (_temp, inbox, bytes, uid, gid) = handoff_fixture();
        assert_eq!(
            promote_pending_supervisor_handoffs(&inbox, "appliance-test", uid, gid, uid, gid,)
                .unwrap(),
            1
        );
        assert!(!inbox.join("candidate-ready-envelope-test.pending").exists());
        let ready = inbox.join("candidate-ready-envelope-test.json");
        assert_eq!(fs::read(&ready).unwrap(), bytes);

        // An interrupted post-write cleanup is idempotent only when the exact
        // pending bytes agree with the already promoted root-owned trigger.
        let pending = inbox.join("candidate-ready-envelope-test.pending");
        fs::write(&pending, &bytes).unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            promote_pending_supervisor_handoffs(&inbox, "appliance-test", uid, gid, uid, gid,)
                .unwrap(),
            1
        );
        assert!(!pending.exists());
    }

    #[test]
    fn root_rejects_divergent_or_linked_pending_handoffs() {
        let (_temp, inbox, bytes, uid, gid) = handoff_fixture();
        promote_pending_supervisor_handoffs(&inbox, "appliance-test", uid, gid, uid, gid).unwrap();
        let pending = inbox.join("candidate-ready-envelope-test.pending");
        fs::write(&pending, &bytes).unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(
            inbox.join("candidate-ready-envelope-test.json"),
            b"different\n",
        )
        .unwrap();
        assert!(
            promote_pending_supervisor_handoffs(&inbox, "appliance-test", uid, gid, uid, gid,)
                .is_err()
        );
        assert!(pending.exists());

        fs::remove_file(&pending).unwrap();
        std::os::unix::fs::symlink(inbox.join("candidate-ready-envelope-test.json"), &pending)
            .unwrap();
        assert!(
            promote_pending_supervisor_handoffs(&inbox, "appliance-test", uid, gid, uid, gid,)
                .is_err()
        );
    }
}
