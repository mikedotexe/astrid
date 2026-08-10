//! Root-owned maintenance lease, graceful A/B activation, and automatic rollback.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, valid_identifier};
use crate::fs_guard::{
    atomic_write, canonical_json, ensure_within, read_json, read_regular, sha256,
};
use crate::generation::{ReleaseIdentity, require_effective_uid, validate_release_manifest_inner};
use crate::ledger_auth::{LedgerKey, seal_record, verify_record};
use crate::native::{CommandReceipt, CommandSpec, NativeRunner, require_success};
use crate::profile_projection::PreparedProfileTransaction;
use crate::unit_transaction::PreparedUnitTransaction;
use crate::{Error, Result};

const REFLECTION_LEASE_PATH: &str = "/run/astrid-edge-self-change/reflection.json";
const REFLECTION_ADMISSION_PATH: &str = "/run/astrid-edge-self-change/reflection-admission.json";
const GENERATION_TRANSITION_OPERATION_SECONDS: u64 = 7_200;
const LEASE_DRAIN_AND_RECOVERY_GRACE_SECONDS: u64 = 1_800;
const MAXIMUM_LEASE_LIFETIME_MILLISECONDS: u64 = 48 * 60 * 60 * 1_000;
const MINIMUM_RETAINED_PRIOR_GENERATIONS: usize = 3;
const MINIMUM_STATE_SNAPSHOT_RETENTION_MILLISECONDS: u64 = 7 * 24 * 60 * 60 * 1_000;
const RESERVE_RELEASE_AUTHORIZED_PHASE: &str = "inode_reserve_release_authorized_for_state_restore";
const RESERVE_RELEASED_PHASE: &str = "inode_reserve_released_for_state_restore";
const RESERVE_RECONCILED_PHASE: &str = "inode_reserve_reconciled_before_state_verification";
const RESERVE_RESTORED_PHASE: &str = "inode_reserve_restored_after_state_restore";
const RESERVE_RESTORED_AFTER_FAILURE_PHASE: &str =
    "inode_reserve_restored_after_failed_state_restore";

#[derive(Debug, Clone)]
struct ReflectionAdmissionPaths {
    lease: PathBuf,
    admission: PathBuf,
}

impl ReflectionAdmissionPaths {
    fn production() -> Self {
        Self {
            lease: PathBuf::from(REFLECTION_LEASE_PATH),
            admission: PathBuf::from(REFLECTION_ADMISSION_PATH),
        }
    }
}

pub struct MaintenanceLease {
    file: File,
    path: PathBuf,
    payload_sha256: String,
    lease_id: String,
    nonce_sha256: String,
    created_at_unix_ms: u64,
    reflection_paths: ReflectionAdmissionPaths,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReconciliationOutcome {
    pub schema: &'static str,
    pub status: &'static str,
    pub generation_id: String,
    pub transition_record_sha256: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageReserveReconciliationOutcome {
    pub schema: &'static str,
    pub status: &'static str,
    pub transition_record_sha256: Option<String>,
    pub state_restore_transaction_id: Option<String>,
    pub command_receipt: CommandReceipt,
}

#[derive(Debug, Clone)]
pub(crate) struct TransitionJournalHead {
    pub(crate) record_sha256: String,
    pub(crate) recorded_at_unix_ms: u64,
    pub(crate) operation: String,
    pub(crate) phase: String,
    pub(crate) lease_id: String,
    pub(crate) lease_payload_sha256: String,
    pub(crate) target_generation_id: String,
    pub(crate) prior_generation_id: String,
    pub(crate) state_snapshot: Option<StateSnapshotBinding>,
    pub(crate) state_restore_transaction_id: Option<String>,
    pub(crate) runtime_projections: Option<RuntimeProjectionBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateSnapshotBinding {
    pub(crate) basename: String,
    pub(crate) generation_id: String,
    pub(crate) manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)] // Explicit digest suffixes make signed journal semantics auditable.
pub(crate) struct RuntimeProjectionBinding {
    target_profile_sha256: String,
    prior_profile_sha256: String,
    target_reports_sha256: String,
    prior_reports_sha256: String,
}

#[derive(Debug, Clone)]
struct PreparedRuntimeSurfaces {
    units: PreparedUnitTransaction,
    profile: PreparedProfileTransaction,
}

impl MaintenanceLease {
    pub fn acquire(config: &Config, reason: &str) -> Result<Self> {
        Self::acquire_for(config, reason, config.drain.maximum_wait_seconds)
    }

    pub fn acquire_for(config: &Config, reason: &str, operation_seconds: u64) -> Result<Self> {
        Self::acquire_for_inner(config, reason, operation_seconds, true)
    }

    pub(crate) fn wait_for_exact_drain(&self, config: &Config) -> Result<DrainGuard> {
        wait_for_drain(config, self, true)
    }

    fn acquire_for_inner(
        config: &Config,
        reason: &str,
        operation_seconds: u64,
        require_root_owner: bool,
    ) -> Result<Self> {
        Self::acquire_for_inner_with_hook(
            config,
            reason,
            operation_seconds,
            require_root_owner,
            ReflectionAdmissionPaths::production(),
            || Ok(()),
        )
    }

    fn acquire_for_inner_with_hook<F>(
        config: &Config,
        reason: &str,
        operation_seconds: u64,
        require_root_owner: bool,
        reflection_paths: ReflectionAdmissionPaths,
        after_lease_created: F,
    ) -> Result<Self>
    where
        F: FnOnce() -> Result<()>,
    {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
        let file = options.open(&config.roots.maintenance_mutex)?;
        let lock_metadata = file.metadata()?;
        if !lock_metadata.is_file()
            || lock_metadata.nlink() != 1
            || (require_root_owner && lock_metadata.uid() != 0)
            || lock_metadata.mode() & 0o077 != 0
        {
            return Err(Error::new("maintenance mutex ownership or mode failed"));
        }
        file.try_lock_exclusive()
            .map_err(|_| Error::deferred("another maintenance transaction is active"))?;
        reject_reflection_admission(&reflection_paths)?;
        let now = unix_millis();
        if config.roots.maintenance_lease.exists() {
            let metadata = fs::symlink_metadata(&config.roots.maintenance_lease)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.nlink() != 1
                || (require_root_owner && metadata.uid() != 0)
                || metadata.mode() & 0o022 != 0
            {
                return Err(Error::new("existing maintenance lease ownership failed"));
            }
            let current: Value = read_json(&config.roots.maintenance_lease, 8_192)?;
            let expires = current
                .get("expires_at_unix_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::new("existing maintenance lease is malformed"))?;
            let boot_may_recover_orphan = boot_may_recover_orphaned_lease(&current, reason);
            if expires > now && !boot_may_recover_orphan {
                return Err(Error::deferred(
                    "an unexpired maintenance lease already exists",
                ));
            }
            fs::remove_file(&config.roots.maintenance_lease)?;
        }
        reject_reflection_admission(&reflection_paths)?;
        let lifetime_ms =
            maintenance_lifetime_milliseconds(operation_seconds, config.drain.maximum_wait_seconds);
        let nonce = random_nonce()?;
        let nonce_sha256 = sha256(nonce.as_bytes());
        let lease_id = format!("lease-{}", &nonce_sha256[..24]);
        let payload = serde_json::json!({
            "schema": "astrid.edge_self_change.maintenance_lease.v2",
            "created_at_unix_ms": now,
            "expires_at_unix_ms": now.saturating_add(lifetime_ms),
            "reason": reason,
            "owner": "immutable_astrid_edge_rescue_helper",
            "lease_id": &lease_id,
            "nonce": &nonce,
        });
        let bytes = canonical_json(&payload)?;
        atomic_write(&config.roots.maintenance_lease, &bytes, 0o444, false)?;
        let lease = Self {
            file,
            path: config.roots.maintenance_lease.clone(),
            payload_sha256: sha256(&bytes),
            lease_id,
            nonce_sha256,
            created_at_unix_ms: now,
            reflection_paths,
        };
        after_lease_created()?;
        lease.revalidate_no_reflection()?;
        Ok(lease)
    }

    fn revalidate_no_reflection(&self) -> Result<()> {
        reject_reflection_admission(&self.reflection_paths)
    }
}

fn boot_may_recover_orphaned_lease(current: &Value, requested_reason: &str) -> bool {
    let Some(object) = current.as_object() else {
        return false;
    };
    let expected_fields = [
        "schema",
        "created_at_unix_ms",
        "expires_at_unix_ms",
        "reason",
        "owner",
        "lease_id",
        "nonce",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    requested_reason == "boot_reconciliation"
        && object.keys().map(String::as_str).collect::<BTreeSet<_>>() == expected_fields
        && object.get("schema").and_then(Value::as_str)
            == Some("astrid.edge_self_change.maintenance_lease.v2")
        && object.get("owner").and_then(Value::as_str)
            == Some("immutable_astrid_edge_rescue_helper")
        && object
            .get("created_at_unix_ms")
            .and_then(Value::as_u64)
            .is_some()
        && object
            .get("expires_at_unix_ms")
            .and_then(Value::as_u64)
            .is_some()
        && matches!(
            object.get("reason").and_then(Value::as_str),
            Some("generation_activation" | "generation_rollback" | "paired_rollback_retention")
        )
        && object
            .get("lease_id")
            .and_then(Value::as_str)
            .is_some_and(valid_identifier)
        && object
            .get("nonce")
            .and_then(Value::as_str)
            .is_some_and(|nonce| nonce.len() == 64 && crate::config::valid_hex64(nonce))
}

fn maintenance_lifetime_milliseconds(operation_seconds: u64, drain_seconds: u64) -> u64 {
    operation_seconds
        .max(drain_seconds)
        .saturating_add(LEASE_DRAIN_AND_RECOVERY_GRACE_SECONDS)
        .saturating_mul(1_000)
        .min(MAXIMUM_LEASE_LIFETIME_MILLISECONDS)
}

/// Remove only a build/synthetic lease whose mutex is no longer held. This is
/// the crash boundary used by the supervisor service's fixed `ExecStopPost`.
pub fn remove_orphaned_build_lease(config: &Config) -> Result<bool> {
    require_effective_uid(0, "orphaned build lease cleanup")?;
    remove_orphaned_build_lease_inner(config, true)
}

fn remove_orphaned_build_lease_inner(config: &Config, require_root_owner: bool) -> Result<bool> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(&config.roots.maintenance_mutex)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || (require_root_owner && metadata.uid() != 0)
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("maintenance mutex ownership or mode failed"));
    }
    file.try_lock_exclusive()
        .map_err(|_| Error::deferred("build maintenance envelope is still active"))?;
    let path = &config.roots.maintenance_lease;
    let lease_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !lease_metadata.is_file()
        || lease_metadata.file_type().is_symlink()
        || lease_metadata.nlink() != 1
        || (require_root_owner && lease_metadata.uid() != 0)
        || lease_metadata.mode() & 0o777 != 0o444
    {
        return Err(Error::new("orphaned maintenance lease identity failed"));
    }
    let value: Value = read_json(path, 8_192)?;
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("orphaned maintenance lease is not an object"))?;
    let expected = [
        "schema",
        "created_at_unix_ms",
        "expires_at_unix_ms",
        "reason",
        "owner",
        "lease_id",
        "nonce",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        || object.get("schema").and_then(Value::as_str)
            != Some("astrid.edge_self_change.maintenance_lease.v2")
        || object.get("owner").and_then(Value::as_str)
            != Some("immutable_astrid_edge_rescue_helper")
        || !matches!(
            object.get("reason").and_then(Value::as_str),
            Some("candidate_build" | "operator_synthetic_lifecycle")
        )
    {
        return Err(Error::new(
            "orphaned lease is not an exact build maintenance envelope",
        ));
    }
    fs::remove_file(path)?;
    File::open(
        path.parent()
            .ok_or_else(|| Error::new("maintenance lease has no parent"))?,
    )?
    .sync_all()?;
    Ok(true)
}

fn prepare_runtime_surfaces<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    target_generation: &Path,
    prior_generation: &Path,
    require_root_owner: bool,
) -> Result<PreparedRuntimeSurfaces> {
    let profile = crate::profile_projection::prepare_for_transition(
        config,
        target_generation,
        prior_generation,
        require_root_owner,
    )?;
    let units = match crate::unit_transaction::prepare_for_transition(
        config,
        runner,
        target_generation,
        prior_generation,
        require_root_owner,
    ) {
        Ok(units) => units,
        Err(error) => {
            crate::profile_projection::restore_prior_for_transition(
                config,
                &profile,
                require_root_owner,
            )?;
            return Err(Error::new(format!(
                "unit transaction preparation failed after profile preparation; profile restored: {error}"
            )));
        },
    };
    Ok(PreparedRuntimeSurfaces { units, profile })
}

fn apply_target_runtime_surfaces<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedRuntimeSurfaces,
    require_root_owner: bool,
) -> Result<Vec<CommandReceipt>> {
    let _ = crate::profile_projection::apply_target_for_transition(
        config,
        &transaction.profile,
        require_root_owner,
    )?;
    Ok(crate::unit_transaction::apply_target_for_transition(
        config,
        runner,
        &transaction.units,
        require_root_owner,
    )?
    .receipts)
}

fn restore_prior_runtime_surfaces<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedRuntimeSurfaces,
    require_root_owner: bool,
) -> Result<Vec<CommandReceipt>> {
    let unit_evidence = crate::unit_transaction::restore_prior_for_transition(
        config,
        runner,
        &transaction.units,
        require_root_owner,
    )?;
    let _ = crate::profile_projection::restore_prior_for_transition(
        config,
        &transaction.profile,
        require_root_owner,
    )?;
    Ok(unit_evidence.receipts)
}

fn commit_runtime_surfaces(
    config: &Config,
    transaction: &PreparedRuntimeSurfaces,
    require_root_owner: bool,
) -> Result<()> {
    crate::profile_projection::commit_for_transition(
        config,
        &transaction.profile,
        require_root_owner,
    )?;
    crate::unit_transaction::commit_for_transition(config, &transaction.units, require_root_owner)
}

fn reject_reflection_admission(paths: &ReflectionAdmissionPaths) -> Result<()> {
    for (path, label) in [
        (&paths.lease, "scheduled-reflection lease"),
        (&paths.admission, "scheduled-reflection admission marker"),
    ] {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Ok(_) => {
                return Err(Error::deferred(format!(
                    "{label} blocks generation transition"
                )));
            },
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

impl Drop for MaintenanceLease {
    fn drop(&mut self) {
        if read_regular(&self.path, 8_192).is_ok_and(|bytes| sha256(&bytes) == self.payload_sha256)
        {
            let _ = fs::remove_file(&self.path);
        }
        let _ = self.file.unlock();
    }
}

pub fn activate<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    generation: &Path,
    previous: &Path,
) -> Result<Vec<CommandReceipt>> {
    require_effective_uid(0, "activate")?;
    activate_inner(config, runner, generation, previous, true)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn activate_inner<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    generation: &Path,
    previous: &Path,
    require_root_binding: bool,
) -> Result<Vec<CommandReceipt>> {
    if require_root_binding {
        let _ = crate::storage::verify(config, false)?;
    }
    let lease = MaintenanceLease::acquire_for_inner(
        config,
        "generation_activation",
        GENERATION_TRANSITION_OPERATION_SECONDS,
        require_root_binding,
    )?;
    let generation_identity =
        validate_transition_release(config, generation, require_root_binding)?;
    let previous_identity = validate_transition_release(config, previous, require_root_binding)?;
    if generation == previous {
        return Err(Error::new("activation requires distinct A/B generations"));
    }
    if read_generation_binding(config, require_root_binding)? != previous_identity.generation_id {
        return Err(Error::new(
            "activation previous slot differs from root generation binding",
        ));
    }
    if active_target(config)? != fs::canonicalize(previous)? {
        return Err(Error::new(
            "activation previous slot differs from the active generation link",
        ));
    }
    append_transition_phase(
        config,
        &lease,
        "activation",
        "planned",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    )?;
    let drain = wait_for_drain(config, &lease, require_root_binding)?;
    append_transition_phase(
        config,
        &lease,
        "activation",
        "drained_and_model_locked",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    )?;
    let mut receipts = Vec::new();
    let prefix = format!(
        "activation-{}-{}",
        generation_identity.generation_id,
        unix_millis()
    );
    let flush_receipt = config
        .roots
        .state_snapshots
        .join(format!("{prefix}-flush.json"));
    run_step(
        config,
        runner,
        &mut receipts,
        &config.executables.checkpoint,
        "durable-state-flush",
        vec![
            "flush".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--output".into(),
            flush_receipt.display().to_string(),
            "--generation-id".into(),
            previous_identity.generation_id.clone(),
        ],
        &config.roots.workspace,
    )?;
    drain.revalidate_live(config, &lease)?;
    let pre_checkpoint = config
        .roots
        .state_snapshots
        .join(format!("{prefix}-pre-checkpoint.json"));
    run_step(
        config,
        runner,
        &mut receipts,
        &config.executables.checkpoint,
        "hindsight-checkpoint",
        vec![
            "checkpoint".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--output".into(),
            pre_checkpoint.display().to_string(),
            "--generation-id".into(),
            previous_identity.generation_id.clone(),
            "--reason".into(),
            "self-change-activation".into(),
            "--maximum-age-seconds".into(),
            config.health.maximum_age_seconds.to_string(),
        ],
        &config.roots.workspace,
    )?;
    let hindsight_baseline = crate::probation::capture_hindsight_baseline(
        config,
        &pre_checkpoint,
        &previous_identity.generation_id,
        require_root_binding,
    )?;
    drain.revalidate_live(config, &lease)?;
    let runtime_transaction =
        prepare_runtime_surfaces(config, runner, generation, previous, require_root_binding)?;
    receipts.extend(runtime_transaction.units.receipts.clone());
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "activation",
        "runtime_surfaces_prepared",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        let recovery = restore_prior_runtime_surfaces(
            config,
            runner,
            &runtime_transaction,
            require_root_binding,
        )?;
        receipts.extend(recovery);
        return Err(Error::new(format!(
            "runtime-surface prepare journal failed and prior surfaces were restored: {error}"
        )));
    }
    if let Err(error) = drain.revalidate_live(config, &lease) {
        let recovery = restore_prior_runtime_surfaces(
            config,
            runner,
            &runtime_transaction,
            require_root_binding,
        )?;
        receipts.extend(recovery);
        return Err(Error::new(format!(
            "prepared drain barrier changed before stop; prior runtime surfaces were restored: {error}"
        )));
    }
    if let Err(error) = stop_runtime(config, runner, &mut receipts) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "runtime stop failed and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "activation",
        "runtime_stopped",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "post-stop transition journal failed and previous generation was restored: {error}"
        )));
    }
    let quiescence_record_sha256 = match exact_stopped_transition_record(
        config,
        &lease,
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        Ok(record) => record,
        Err(error) => {
            recover_generation(
                config,
                runner,
                &mut receipts,
                previous,
                &previous_identity.generation_id,
                &runtime_transaction,
                require_root_binding,
            )?;
            return Err(Error::new(format!(
                "runtime stop lacks an exact signed quiescence record and previous generation was restored: {error}"
            )));
        },
    };
    // The edge service owns continuously appended fill/perception ledgers and
    // the core owns its database. Snapshot only after both have stopped;
    // otherwise a full rollback fixture can never be an exact stable copy.
    if require_root_binding {
        let _ = crate::storage::verify(config, false)?;
    }
    let snapshot = config.roots.state_snapshots.join(format!("{prefix}-state"));
    ensure_within(&config.roots.state_snapshots, &snapshot, false)?;
    let snapshot_result: Result<()> = (|| {
        run_step(
            config,
            runner,
            &mut receipts,
            &config.executables.checkpoint,
            "rollback-compatible-state-snapshot",
            vec![
                "snapshot".into(),
                "--workspace".into(),
                config.roots.workspace.display().to_string(),
                "--output".into(),
                snapshot.display().to_string(),
                "--generation-id".into(),
                previous_identity.generation_id.clone(),
                "--quiescence-record-sha256".into(),
                quiescence_record_sha256,
                "--require-dual-readable".into(),
            ],
            &config.roots.workspace,
        )?;
        run_step(
            config,
            runner,
            &mut receipts,
            &config.executables.checkpoint,
            "rollback-state-snapshot-verification",
            vec![
                "verify-snapshot".into(),
                "--snapshot".into(),
                snapshot.display().to_string(),
                "--generation-id".into(),
                previous_identity.generation_id.clone(),
            ],
            &config.roots.state_snapshots,
        )?;
        let binding = read_state_snapshot_binding(
            config,
            &snapshot,
            &previous_identity.generation_id,
            require_root_binding,
        )?;
        append_transition_phase_with_snapshot(
            config,
            &lease,
            "activation",
            "state_flushed_checkpointed_and_snapshotted",
            &generation_identity.generation_id,
            &previous_identity.generation_id,
            Some(&binding),
            require_root_binding,
        )?;
        Ok(())
    })();
    if let Err(error) = snapshot_result {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "stable rollback snapshot failed and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = drain.revalidate_exact_files(config, &lease) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "prepared drain barrier changed during state snapshot and previous generation was restored: {error}"
        )));
    }
    match apply_target_runtime_surfaces(config, runner, &runtime_transaction, require_root_binding)
    {
        Ok(surface_receipts) => receipts.extend(surface_receipts),
        Err(error) => {
            recover_generation(
                config,
                runner,
                &mut receipts,
                previous,
                &previous_identity.generation_id,
                &runtime_transaction,
                require_root_binding,
            )?;
            return Err(Error::new(format!(
                "runtime-surface transaction failed and previous generation was restored: {error}"
            )));
        },
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "activation",
        "runtime_surfaces_installed_and_reloaded",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "unit transaction phase journal failed and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "activation",
        "switch_intent_recorded",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "switch intent journal failed and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = drain.revalidate_exact_files(config, &lease) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "prepared drain barrier changed before generation switch and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = lease.revalidate_no_reflection() {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "scheduled reflection appeared before generation switch and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = switch_link(config, generation, require_root_binding)
        .and_then(|()| write_generation_binding(config, &generation_identity.generation_id))
    {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "generation pointer/binding switch failed and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "activation",
        "pointer_and_binding_switched",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "post-switch transition journal failed and previous generation was restored: {error}"
        )));
    }
    let startup = start_runtime(config, runner, &mut receipts);
    if let Err(start_error) = startup {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "new generation startup failed and was rolled back: {start_error}"
        )));
    }
    if let Err(error) = validate_active_binding(
        config,
        generation,
        &generation_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "post-start generation consistency failed and was rolled back: {error}"
        )));
    }
    let post_checkpoint = config
        .roots
        .state_snapshots
        .join(format!("{prefix}-post-checkpoint.json"));
    let post_result = run_step(
        config,
        runner,
        &mut receipts,
        &config.executables.checkpoint,
        "post-activation-hindsight-checkpoint",
        vec![
            "checkpoint".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--output".into(),
            post_checkpoint.display().to_string(),
            "--generation-id".into(),
            generation_identity.generation_id.clone(),
            "--reason".into(),
            "post-activation".into(),
            "--maximum-age-seconds".into(),
            config.health.maximum_age_seconds.to_string(),
        ],
        &config.roots.state_snapshots,
    )
    .and_then(|()| {
        crate::probation::verify_post_activation_hindsight(
            config,
            &post_checkpoint,
            &generation_identity.generation_id,
            &hindsight_baseline,
            require_root_binding,
        )?;
        crate::probation::initialize_inner(
            config,
            &generation_identity.generation_id,
            &previous_identity.generation_id,
            &hindsight_baseline,
            require_root_binding,
        )
    });
    if let Err(error) = post_result {
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "post-start checkpoint/probation initialization failed and was rolled back: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "activation",
        "probation_started",
        &generation_identity.generation_id,
        &previous_identity.generation_id,
        require_root_binding,
    ) {
        let _ = crate::probation::close_for_rollback_inner(
            config,
            &generation_identity.generation_id,
            "probation_phase_journal_failure",
            require_root_binding,
        );
        recover_generation(
            config,
            runner,
            &mut receipts,
            previous,
            &previous_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "probation phase journal failed and previous generation was restored: {error}"
        )));
    }
    if let Err(error) = commit_runtime_surfaces(config, &runtime_transaction, require_root_binding)
    {
        // The outer probation phase is already the durable activation commit
        // point.  Reverting would contradict that journal.  Leave the pending
        // unit transaction for boot reconciliation and stop mutable runtime.
        let _ = stop_runtime(config, runner, &mut receipts);
        return Err(Error::new(format!(
            "activation committed but runtime-surface finalization failed; runtime stopped for boot reconciliation: {error}"
        )));
    }
    Ok(receipts)
}

pub fn rollback<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    generation: &Path,
) -> Result<Vec<CommandReceipt>> {
    require_effective_uid(0, "rollback")?;
    rollback_inner(config, runner, generation, true)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn rollback_inner<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    generation: &Path,
    require_root_binding: bool,
) -> Result<Vec<CommandReceipt>> {
    if require_root_binding {
        let _ = crate::storage::verify(config, false)?;
    }
    let lease = MaintenanceLease::acquire_for_inner(
        config,
        "generation_rollback",
        GENERATION_TRANSITION_OPERATION_SECONDS,
        require_root_binding,
    )?;
    let target_identity = validate_transition_release(config, generation, require_root_binding)?;
    let current_id = read_generation_binding(config, require_root_binding)?;
    let current = config.roots.releases.join(&current_id);
    let current_identity = validate_transition_release(config, &current, require_root_binding)?;
    if active_target(config)? != fs::canonicalize(&current)? {
        return Err(Error::new(
            "rollback current slot differs from the active generation link",
        ));
    }
    if current_identity.generation_id == target_identity.generation_id {
        return Err(Error::new("rollback target is already active"));
    }
    let rollback_snapshot = rollback_snapshot_binding(
        config,
        &current_identity.generation_id,
        &target_identity.generation_id,
        require_root_binding,
    )?;
    append_transition_phase_with_snapshot(
        config,
        &lease,
        "rollback",
        "planned",
        &target_identity.generation_id,
        &current_identity.generation_id,
        rollback_snapshot.as_ref(),
        require_root_binding,
    )?;
    let drain = wait_for_drain(config, &lease, require_root_binding)?;
    append_transition_phase(
        config,
        &lease,
        "rollback",
        "drained_and_model_locked",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    )?;
    let mut receipts = Vec::new();
    let prefix = format!(
        "rollback-{}-{}",
        target_identity.generation_id,
        unix_millis()
    );
    let flush_receipt = config
        .roots
        .state_snapshots
        .join(format!("{prefix}-flush.json"));
    run_step(
        config,
        runner,
        &mut receipts,
        &config.executables.checkpoint,
        "durable-state-flush",
        vec![
            "flush".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--output".into(),
            flush_receipt.display().to_string(),
            "--generation-id".into(),
            current_identity.generation_id.clone(),
        ],
        &config.roots.workspace,
    )?;
    drain.revalidate_live(config, &lease)?;
    let pre_checkpoint = config
        .roots
        .state_snapshots
        .join(format!("{prefix}-pre-checkpoint.json"));
    run_step(
        config,
        runner,
        &mut receipts,
        &config.executables.checkpoint,
        "hindsight-checkpoint",
        vec![
            "checkpoint".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--output".into(),
            pre_checkpoint.display().to_string(),
            "--generation-id".into(),
            current_identity.generation_id.clone(),
            "--reason".into(),
            "self-change-rollback".into(),
            "--maximum-age-seconds".into(),
            config.health.maximum_age_seconds.to_string(),
        ],
        &config.roots.workspace,
    )?;
    drain.revalidate_live(config, &lease)?;
    append_transition_phase(
        config,
        &lease,
        "rollback",
        "state_flushed_and_checkpointed",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    )?;
    let runtime_transaction =
        prepare_runtime_surfaces(config, runner, generation, &current, require_root_binding)?;
    receipts.extend(runtime_transaction.units.receipts.clone());
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "rollback",
        "runtime_surfaces_prepared",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    ) {
        let recovery = restore_prior_runtime_surfaces(
            config,
            runner,
            &runtime_transaction,
            require_root_binding,
        )?;
        receipts.extend(recovery);
        return Err(Error::new(format!(
            "rollback runtime-surface prepare journal failed and current surfaces were restored: {error}"
        )));
    }
    if let Err(error) = drain.revalidate_live(config, &lease) {
        let recovery = restore_prior_runtime_surfaces(
            config,
            runner,
            &runtime_transaction,
            require_root_binding,
        )?;
        receipts.extend(recovery);
        return Err(Error::new(format!(
            "rollback drain barrier changed before stop; current runtime surfaces were restored: {error}"
        )));
    }
    if let Err(error) = stop_runtime(config, runner, &mut receipts) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback stop failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "rollback",
        "runtime_stopped",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback stop journal failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = drain.revalidate_exact_files(config, &lease) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback drain barrier changed after stop and current generation was restored: {error}"
        )));
    }
    if let Some(binding) = rollback_snapshot.as_ref() {
        if require_root_binding {
            let _ = crate::storage::verify(config, false)?;
        }
        append_transition_phase_with_snapshot(
            config,
            &lease,
            "rollback",
            "mutable_state_restore_started",
            &target_identity.generation_id,
            &current_identity.generation_id,
            Some(binding),
            require_root_binding,
        )?;
        if let Err(error) = restore_state_snapshot(
            config,
            runner,
            &mut receipts,
            binding,
            &lease.lease_id,
            require_root_binding,
        ) {
            let _ = stop_runtime(config, runner, &mut receipts);
            return Err(Error::new(format!(
                "mutable state restore failed closed; rescue required before any runtime restart: {error}"
            )));
        }
        append_transition_phase_with_snapshot(
            config,
            &lease,
            "rollback",
            "mutable_state_restored",
            &target_identity.generation_id,
            &current_identity.generation_id,
            Some(binding),
            require_root_binding,
        )?;
    } else if require_root_binding {
        let _ = stop_runtime(config, runner, &mut receipts);
        return Err(Error::new(
            "production rollback has no exact pre-switch state snapshot; rescue required",
        ));
    }
    match apply_target_runtime_surfaces(config, runner, &runtime_transaction, require_root_binding)
    {
        Ok(surface_receipts) => receipts.extend(surface_receipts),
        Err(error) => {
            recover_generation(
                config,
                runner,
                &mut receipts,
                &current,
                &current_identity.generation_id,
                &runtime_transaction,
                require_root_binding,
            )?;
            return Err(Error::new(format!(
                "rollback runtime-surface transaction failed and current generation was restored: {error}"
            )));
        },
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "rollback",
        "runtime_surfaces_installed_and_reloaded",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback runtime-surface phase journal failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "rollback",
        "switch_intent_recorded",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback switch intent journal failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = drain.revalidate_exact_files(config, &lease) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback drain barrier changed before generation switch and current generation was restored: {error}"
        )));
    }
    if let Err(error) = lease.revalidate_no_reflection() {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "scheduled reflection appeared before rollback switch and current generation was restored: {error}"
        )));
    }
    if let Err(error) = switch_link(config, generation, require_root_binding)
        .and_then(|()| write_generation_binding(config, &target_identity.generation_id))
    {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback pointer/binding switch failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "rollback",
        "pointer_and_binding_switched",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback switch journal failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = start_runtime(config, runner, &mut receipts) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback target failed and prior generation was restored: {error}"
        )));
    }
    if let Err(error) = validate_active_binding(
        config,
        generation,
        &target_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback post-start consistency failed and current generation was restored: {error}"
        )));
    }
    let post_checkpoint = config
        .roots
        .state_snapshots
        .join(format!("{prefix}-post-checkpoint.json"));
    if let Err(error) = run_step(
        config,
        runner,
        &mut receipts,
        &config.executables.checkpoint,
        "post-rollback-hindsight-checkpoint",
        vec![
            "checkpoint".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--output".into(),
            post_checkpoint.display().to_string(),
            "--generation-id".into(),
            target_identity.generation_id.clone(),
            "--reason".into(),
            "post-rollback".into(),
            "--maximum-age-seconds".into(),
            config.health.maximum_age_seconds.to_string(),
        ],
        &config.roots.state_snapshots,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "post-rollback checkpoint failed and current generation was restored: {error}"
        )));
    }
    if let Err(error) = append_transition_phase(
        config,
        &lease,
        "rollback",
        "rollback_target_validated",
        &target_identity.generation_id,
        &current_identity.generation_id,
        require_root_binding,
    ) {
        recover_generation(
            config,
            runner,
            &mut receipts,
            &current,
            &current_identity.generation_id,
            &runtime_transaction,
            require_root_binding,
        )?;
        return Err(Error::new(format!(
            "rollback commit point could not be recorded and current generation was restored: {error}"
        )));
    }
    if let Err(error) = commit_runtime_surfaces(config, &runtime_transaction, require_root_binding)
    {
        // The outer rollback journal has selected the target.  Keep its unit
        // transaction pending so the boot guard can finish the exact same
        // selection, and stop mutable runtime until then.
        let _ = stop_runtime(config, runner, &mut receipts);
        return Err(Error::new(format!(
            "rollback committed but runtime-surface finalization failed; runtime stopped for boot reconciliation: {error}"
        )));
    }
    let completion = crate::probation::close_for_rollback_inner(
        config,
        &current_identity.generation_id,
        "automatic_or_operator_rollback",
        require_root_binding,
    )
    .and_then(|()| {
        append_transition_phase(
            config,
            &lease,
            "rollback",
            "completed",
            &target_identity.generation_id,
            &current_identity.generation_id,
            require_root_binding,
        )
    });
    if let Err(error) = completion {
        // The rollback target crossed its durable commit point. Reverting to
        // the failed candidate would be less safe; stop mutable services and
        // let boot reconciliation retry the idempotent probation closure.
        let _ = stop_runtime(config, runner, &mut receipts);
        return Err(Error::new(format!(
            "rollback committed but final probation evidence failed; runtime stopped for boot reconciliation: {error}"
        )));
    }
    Ok(receipts)
}

fn recover_generation<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    generation: &Path,
    generation_id: &str,
    runtime_transaction: &PreparedRuntimeSurfaces,
    require_root_binding: bool,
) -> Result<()> {
    let mut recovery = Vec::new();
    let _ = stop_runtime(config, runner, &mut recovery);
    let journal = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root_binding)?;
    let head = verify_phase_journal(&journal, &key, require_root_binding)?;
    if head
        .as_ref()
        .is_some_and(|value| value.operation == "rollback" && transition_committed_to_target(value))
    {
        return Err(Error::new(
            "rollback already began mutable-state restoration; failed candidate will not be restarted and rescue is required",
        ));
    }
    if let Some((binding, transaction_id)) = head.as_ref().and_then(|value| {
        (value.operation == "activation"
            && value.prior_generation_id == generation_id
            && !transition_committed_to_target(value))
        .then(|| {
            value
                .state_snapshot
                .clone()
                .map(|binding| (binding, value.lease_id.clone()))
        })
        .flatten()
    }) {
        restore_state_snapshot(
            config,
            runner,
            &mut recovery,
            &binding,
            &format!("recovery-{transaction_id}"),
            require_root_binding,
        )
        .map_err(|error| {
            Error::new(format!(
                "cannot restore pre-switch mutable state; rescue required: {error}"
            ))
        })?;
    }
    let restored =
        restore_prior_runtime_surfaces(config, runner, runtime_transaction, require_root_binding)
            .map_err(|error| {
            Error::new(format!(
                "cannot restore prior runtime surfaces; rescue required: {error}"
            ))
        })?;
    recovery.extend(restored);
    switch_link(config, generation, require_root_binding).map_err(|error| {
        Error::new(format!(
            "cannot restore prior generation pointer; rescue required: {error}"
        ))
    })?;
    write_generation_binding(config, generation_id).map_err(|error| {
        Error::new(format!(
            "cannot restore prior generation binding; rescue required: {error}"
        ))
    })?;
    start_runtime(config, runner, &mut recovery).map_err(|error| {
        Error::new(format!(
            "cannot restart prior generation; rescue required: {error}"
        ))
    })?;
    validate_active_binding(config, generation, generation_id, require_root_binding)?;
    receipts.extend(recovery);
    Ok(())
}

fn validate_active_binding(
    config: &Config,
    generation: &Path,
    generation_id: &str,
    require_root_binding: bool,
) -> Result<()> {
    let identity = validate_transition_release(config, generation, require_root_binding)?;
    if identity.generation_id != generation_id
        || read_generation_binding(config, require_root_binding)? != generation_id
        || active_target(config)? != fs::canonicalize(generation)?
    {
        return Err(Error::new(
            "active link, root binding, and generation manifest disagree",
        ));
    }
    let projection = crate::profile_projection::verify_active_generation(
        config,
        generation,
        require_root_binding,
    )?;
    if projection.generation_id != generation_id {
        return Err(Error::new(
            "active profile/report projection belongs to another generation",
        ));
    }
    Ok(())
}

fn append_transition_phase(
    config: &Config,
    lease: &MaintenanceLease,
    operation: &str,
    phase: &str,
    target_generation: &str,
    prior_generation: &str,
    require_root: bool,
) -> Result<()> {
    append_transition_phase_with_snapshot(
        config,
        lease,
        operation,
        phase,
        target_generation,
        prior_generation,
        None,
        require_root,
    )
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)] // One signed phase record binds snapshot and runtime projections atomically.
fn append_transition_phase_with_snapshot(
    config: &Config,
    lease: &MaintenanceLease,
    operation: &str,
    phase: &str,
    target_generation: &str,
    prior_generation: &str,
    state_snapshot: Option<&StateSnapshotBinding>,
    require_root: bool,
) -> Result<()> {
    if !matches!(operation, "activation" | "rollback")
        || phase.is_empty()
        || phase.len() > 128
        || !crate::config::valid_identifier(target_generation)
        || !crate::config::valid_identifier(prior_generation)
    {
        return Err(Error::new("transition phase identity is invalid"));
    }
    let root_metadata = fs::symlink_metadata(&config.roots.state_snapshots)?;
    if !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || (require_root && root_metadata.uid() != 0)
        || root_metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "transition journal root is not private immutable state",
        ));
    }
    let path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let previous = verify_phase_journal(&path, &key, require_root)?;
    let inherited = previous.as_ref().and_then(|head| {
        (head.operation == operation
            && head.target_generation_id == target_generation
            && head.prior_generation_id == prior_generation)
            .then(|| head.state_snapshot.clone())
            .flatten()
    });
    let state_snapshot = state_snapshot.cloned().or(inherited);
    let state_restore_transaction_id = previous.as_ref().and_then(|head| {
        (head.operation == operation
            && head.target_generation_id == target_generation
            && head.prior_generation_id == prior_generation)
            .then(|| head.state_restore_transaction_id.clone())
            .flatten()
    });
    if let Some(binding) = &state_snapshot {
        let expected_generation = if operation == "activation" {
            prior_generation
        } else {
            target_generation
        };
        if binding.generation_id != expected_generation
            || !valid_identifier(&binding.basename)
            || !crate::config::valid_hex64(&binding.manifest_sha256)
        {
            return Err(Error::new("transition state snapshot binding is invalid"));
        }
    }
    let target_projection = crate::profile_projection::generation_projection_evidence(
        config,
        &config.roots.releases.join(target_generation),
    )?;
    let prior_projection = crate::profile_projection::generation_projection_evidence(
        config,
        &config.roots.releases.join(prior_generation),
    )?;
    let runtime_projections = RuntimeProjectionBinding {
        target_profile_sha256: target_projection.active_profile_sha256,
        prior_profile_sha256: prior_projection.active_profile_sha256,
        target_reports_sha256: target_projection.report_projection_sha256,
        prior_reports_sha256: prior_projection.report_projection_sha256,
    };
    if previous.as_ref().is_some_and(|head| {
        head.operation == operation
            && head.target_generation_id == target_generation
            && head.prior_generation_id == prior_generation
            && head
                .runtime_projections
                .as_ref()
                .is_some_and(|prior| prior != &runtime_projections)
    }) {
        return Err(Error::new(
            "runtime projection binding changed within one transition",
        ));
    }
    let record = serde_json::json!({
        "schema": "astrid.edge_rescue_helper.transition_phase.v4",
        "recorded_at_unix_ms": unix_millis(),
        "operation": operation,
        "phase": phase,
        "lease_id": &lease.lease_id,
        "lease_payload_sha256": &lease.payload_sha256,
        "target_generation_id": target_generation,
        "prior_generation_id": prior_generation,
        "state_snapshot_basename": state_snapshot.as_ref().map(|binding| &binding.basename),
        "state_snapshot_generation_id": state_snapshot.as_ref().map(|binding| &binding.generation_id),
        "state_snapshot_manifest_sha256": state_snapshot.as_ref().map(|binding| &binding.manifest_sha256),
        "state_restore_transaction_id": state_restore_transaction_id,
        "target_profile_projection_sha256": runtime_projections.target_profile_sha256,
        "prior_profile_projection_sha256": runtime_projections.prior_profile_sha256,
        "target_report_projection_sha256": runtime_projections.target_reports_sha256,
        "prior_report_projection_sha256": runtime_projections.prior_reports_sha256,
        "previous_record_sha256": previous.as_ref().map(|head| &head.record_sha256),
        "authority": "immutable_root_transition_phase_journal",
    });
    let _ = append_sealed_transition_record(config, &path, &key, record, require_root, true)?;
    Ok(())
}

fn append_state_restore_reserve_phase(
    config: &Config,
    expected_head: &TransitionJournalHead,
    phase: &str,
    transaction_id: &str,
    require_root: bool,
) -> Result<String> {
    if !reserve_transaction_phase(phase) || !valid_identifier(transaction_id) {
        return Err(Error::new(
            "state restore reserve phase identity is invalid",
        ));
    }
    let root_metadata = fs::symlink_metadata(&config.roots.state_snapshots)?;
    if !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || (require_root && root_metadata.uid() != 0)
        || root_metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "transition journal root is not private immutable state",
        ));
    }
    let path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let current = verify_phase_journal(&path, &key, require_root)?
        .ok_or_else(|| Error::new("state restore transition journal is absent"))?;
    if current.record_sha256 != expected_head.record_sha256
        || current.operation != expected_head.operation
        || current.target_generation_id != expected_head.target_generation_id
        || current.prior_generation_id != expected_head.prior_generation_id
        || current.lease_id != expected_head.lease_id
        || current.lease_payload_sha256 != expected_head.lease_payload_sha256
        || current.state_snapshot != expected_head.state_snapshot
        || current.runtime_projections != expected_head.runtime_projections
        || current.state_restore_transaction_id != expected_head.state_restore_transaction_id
        || !state_restore_required_for_head(&current)
    {
        return Err(Error::new(
            "state restore reserve phase no longer binds the exact transition head",
        ));
    }
    let exact_predecessor = match phase {
        RESERVE_RELEASE_AUTHORIZED_PHASE => current
            .state_restore_transaction_id
            .as_ref()
            .is_none_or(|value| value == transaction_id),
        RESERVE_RELEASED_PHASE => {
            current.phase == RESERVE_RELEASE_AUTHORIZED_PHASE
                && current.state_restore_transaction_id.as_deref() == Some(transaction_id)
        },
        RESERVE_RECONCILED_PHASE => {
            matches!(
                current.phase.as_str(),
                RESERVE_RELEASE_AUTHORIZED_PHASE | RESERVE_RELEASED_PHASE
            ) && current.state_restore_transaction_id.as_deref() == Some(transaction_id)
        },
        RESERVE_RESTORED_PHASE | RESERVE_RESTORED_AFTER_FAILURE_PHASE => {
            current.phase == RESERVE_RELEASED_PHASE
                && current.state_restore_transaction_id.as_deref() == Some(transaction_id)
        },
        _ => false,
    };
    if !exact_predecessor {
        return Err(Error::new(
            "state restore reserve phase predecessor is invalid",
        ));
    }
    let state_snapshot = current
        .state_snapshot
        .as_ref()
        .ok_or_else(|| Error::new("state restore reserve phase lacks a snapshot binding"))?;
    let runtime_projections = current.runtime_projections.as_ref().ok_or_else(|| {
        Error::new("state restore reserve phase lacks runtime projection binding")
    })?;
    verify_runtime_projection_binding(config, &current, require_root)?;
    let record = serde_json::json!({
        "schema": "astrid.edge_rescue_helper.transition_phase.v4",
        "recorded_at_unix_ms": unix_millis(),
        "operation": &current.operation,
        "phase": phase,
        "lease_id": &current.lease_id,
        "lease_payload_sha256": &current.lease_payload_sha256,
        "target_generation_id": &current.target_generation_id,
        "prior_generation_id": &current.prior_generation_id,
        "state_snapshot_basename": &state_snapshot.basename,
        "state_snapshot_generation_id": &state_snapshot.generation_id,
        "state_snapshot_manifest_sha256": &state_snapshot.manifest_sha256,
        "state_restore_transaction_id": transaction_id,
        "target_profile_projection_sha256": &runtime_projections.target_profile_sha256,
        "prior_profile_projection_sha256": &runtime_projections.prior_profile_sha256,
        "target_report_projection_sha256": &runtime_projections.target_reports_sha256,
        "prior_report_projection_sha256": &runtime_projections.prior_reports_sha256,
        "previous_record_sha256": &current.record_sha256,
        "authority": "immutable_root_transition_phase_journal",
    });
    append_sealed_transition_record(config, &path, &key, record, require_root, false)
}

fn append_sealed_transition_record(
    config: &Config,
    path: &Path,
    key: &LedgerKey,
    mut record: Value,
    require_root: bool,
    allow_create: bool,
) -> Result<String> {
    let claimed = seal_record(&mut record, key, "transition")?;
    let mut options = OpenOptions::new();
    options
        .write(true)
        .append(true)
        .create(allow_create)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || (require_root && metadata.uid() != 0)
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "transition phase journal ownership or mode failed",
        ));
    }
    file.write_all(&canonical_json(&record)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(&config.roots.state_snapshots)?.sync_all()?;
    Ok(claimed)
}

fn verify_phase_journal(
    path: &Path,
    key: &LedgerKey,
    require_root: bool,
) -> Result<Option<TransitionJournalHead>> {
    Ok(verify_phase_journal_records(path, key, require_root)?.pop())
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded pass verifies each authenticated transition field and hash-chain edge"
)]
pub(crate) fn verify_phase_journal_records(
    path: &Path,
    key: &LedgerKey,
    require_root: bool,
) -> Result<Vec<TransitionJournalHead>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || (require_root && metadata.uid() != 0)
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("transition phase journal identity failed"));
    }
    let bytes = read_regular(path, 32 * 1024 * 1024)?;
    let mut previous: Option<String> = None;
    let mut records = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_slice(line)?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::new("transition phase record is not an object"))?;
        let claimed = verify_record(&value, key, "transition")?;
        let schema = object
            .get("schema")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("transition phase schema is absent"))?;
        if !matches!(
            schema,
            "astrid.edge_rescue_helper.transition_phase.v2"
                | "astrid.edge_rescue_helper.transition_phase.v3"
                | "astrid.edge_rescue_helper.transition_phase.v4"
        ) || object.get("authority").and_then(Value::as_str)
            != Some("immutable_root_transition_phase_journal")
            || object.get("previous_record_sha256")
                != Some(&previous.clone().map_or(Value::Null, Value::String))
        {
            return Err(Error::new("transition phase hash chain failed"));
        }
        let state_snapshot = if matches!(
            schema,
            "astrid.edge_rescue_helper.transition_phase.v3"
                | "astrid.edge_rescue_helper.transition_phase.v4"
        ) {
            match (
                object
                    .get("state_snapshot_basename")
                    .and_then(Value::as_str),
                object
                    .get("state_snapshot_generation_id")
                    .and_then(Value::as_str),
                object
                    .get("state_snapshot_manifest_sha256")
                    .and_then(Value::as_str),
            ) {
                (None, None, None) => None,
                (Some(basename), Some(generation_id), Some(manifest_sha256)) => {
                    Some(StateSnapshotBinding {
                        basename: basename.to_owned(),
                        generation_id: generation_id.to_owned(),
                        manifest_sha256: manifest_sha256.to_owned(),
                    })
                },
                _ => {
                    return Err(Error::new("transition state snapshot binding is partial"));
                },
            }
        } else {
            None
        };
        let state_restore_transaction_id =
            if schema == "astrid.edge_rescue_helper.transition_phase.v4" {
                match object.get("state_restore_transaction_id") {
                    Some(Value::Null) => None,
                    Some(Value::String(value)) if valid_identifier(value) => Some(value.clone()),
                    _ => {
                        return Err(Error::new(
                            "transition state restore transaction identity is invalid",
                        ));
                    },
                }
            } else {
                None
            };
        let runtime_projections = match (
            object
                .get("target_profile_projection_sha256")
                .and_then(Value::as_str),
            object
                .get("prior_profile_projection_sha256")
                .and_then(Value::as_str),
            object
                .get("target_report_projection_sha256")
                .and_then(Value::as_str),
            object
                .get("prior_report_projection_sha256")
                .and_then(Value::as_str),
        ) {
            (None, None, None, None) => None,
            (
                Some(target_profile),
                Some(prior_profile),
                Some(target_reports),
                Some(prior_reports),
            ) => {
                let values = [target_profile, prior_profile, target_reports, prior_reports];
                if values
                    .iter()
                    .any(|digest| !crate::config::valid_hex64(digest))
                {
                    return Err(Error::new(
                        "transition runtime projection digest is invalid",
                    ));
                }
                Some(RuntimeProjectionBinding {
                    target_profile_sha256: target_profile.to_owned(),
                    prior_profile_sha256: prior_profile.to_owned(),
                    target_reports_sha256: target_reports.to_owned(),
                    prior_reports_sha256: prior_reports.to_owned(),
                })
            },
            _ => {
                return Err(Error::new(
                    "transition runtime projection binding is partial",
                ));
            },
        };
        let transition = TransitionJournalHead {
            record_sha256: claimed.clone(),
            recorded_at_unix_ms: object
                .get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .filter(|value| *value > 0)
                .ok_or_else(|| Error::new("transition phase timestamp is invalid"))?,
            operation: required_record_string(object, "operation")?,
            phase: required_record_string(object, "phase")?,
            lease_id: required_record_string(object, "lease_id")?,
            lease_payload_sha256: required_record_string(object, "lease_payload_sha256")?,
            target_generation_id: required_record_string(object, "target_generation_id")?,
            prior_generation_id: required_record_string(object, "prior_generation_id")?,
            state_snapshot,
            state_restore_transaction_id,
            runtime_projections,
        };
        if !matches!(transition.operation.as_str(), "activation" | "rollback")
            || !crate::config::valid_identifier(&transition.target_generation_id)
            || !crate::config::valid_identifier(&transition.prior_generation_id)
            || !transition.lease_id.starts_with("lease-")
            || transition.lease_id.len() != 30
            || !crate::config::valid_hex64(&transition.lease_payload_sha256)
            || transition.state_snapshot.as_ref().is_some_and(|binding| {
                !valid_identifier(&binding.basename)
                    || !valid_identifier(&binding.generation_id)
                    || !crate::config::valid_hex64(&binding.manifest_sha256)
                    || binding.generation_id
                        != if transition.operation == "activation" {
                            transition.prior_generation_id.clone()
                        } else {
                            transition.target_generation_id.clone()
                        }
            })
            || reserve_transaction_phase(&transition.phase)
                && (transition.state_snapshot.is_none()
                    || transition.state_restore_transaction_id.is_none())
        {
            return Err(Error::new("transition phase record identity failed"));
        }
        previous = Some(claimed);
        records.push(transition);
    }
    Ok(records)
}

fn exact_stopped_transition_record(
    config: &Config,
    lease: &MaintenanceLease,
    target_generation: &str,
    prior_generation: &str,
    require_root: bool,
) -> Result<String> {
    let path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let head = verify_phase_journal(&path, &key, require_root)?
        .ok_or_else(|| Error::new("runtime stop transition record is absent"))?;
    if head.operation != "activation"
        || head.phase != "runtime_stopped"
        || head.lease_id != lease.lease_id
        || head.lease_payload_sha256 != lease.payload_sha256
        || head.target_generation_id != target_generation
        || head.prior_generation_id != prior_generation
        || head.state_snapshot.is_some()
    {
        return Err(Error::new(
            "runtime stop transition record does not prove this exact drained activation",
        ));
    }
    Ok(head.record_sha256)
}

fn required_record_string(object: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 256)
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("transition phase field is invalid: {key}")))
}

fn rollback_snapshot_binding(
    config: &Config,
    current_generation: &str,
    rollback_target: &str,
    require_root: bool,
) -> Result<Option<StateSnapshotBinding>> {
    let path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let Some(head) = verify_phase_journal(&path, &key, require_root)? else {
        if require_root {
            return Err(Error::new(
                "production rollback lacks signed activation history",
            ));
        }
        return Ok(None);
    };
    verify_runtime_projection_binding(config, &head, require_root)?;
    if head.operation != "activation"
        || head.target_generation_id != current_generation
        || head.prior_generation_id != rollback_target
        || !transition_committed_to_target(&head)
    {
        if require_root {
            return Err(Error::new(
                "production rollback is not the inverse of the signed active transition",
            ));
        }
        return Ok(None);
    }
    let Some(binding) = head.state_snapshot else {
        if require_root {
            return Err(Error::new(
                "production rollback lacks a signed pre-switch state snapshot",
            ));
        }
        return Ok(None);
    };
    let snapshot = config.roots.state_snapshots.join(&binding.basename);
    let verified = read_state_snapshot_binding(config, &snapshot, rollback_target, require_root)?;
    if verified != binding {
        return Err(Error::new(
            "signed transition snapshot differs from immutable snapshot manifest",
        ));
    }
    Ok(Some(binding))
}

pub(crate) fn read_state_snapshot_binding(
    config: &Config,
    snapshot: &Path,
    generation_id: &str,
    require_root: bool,
) -> Result<StateSnapshotBinding> {
    if snapshot.parent() != Some(config.roots.state_snapshots.as_path())
        || !valid_identifier(generation_id)
    {
        return Err(Error::new(
            "state snapshot path or generation escaped policy",
        ));
    }
    let basename = snapshot
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| valid_identifier(value))
        .ok_or_else(|| Error::new("state snapshot basename is invalid"))?;
    let snapshot_metadata = fs::symlink_metadata(snapshot)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !snapshot_metadata.is_dir()
        || snapshot_metadata.file_type().is_symlink()
        || snapshot_metadata.uid() != expected_uid
        || snapshot_metadata.mode() & 0o222 != 0
    {
        return Err(Error::new("state snapshot root identity failed"));
    }
    let manifest_path = snapshot.join("manifest.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path)?;
    if !manifest_metadata.is_file()
        || manifest_metadata.file_type().is_symlink()
        || manifest_metadata.nlink() != 1
        || manifest_metadata.uid() != expected_uid
        || manifest_metadata.mode() & 0o222 != 0
        || manifest_metadata.len() > 64 * 1024 * 1024
    {
        return Err(Error::new("state snapshot manifest identity failed"));
    }
    let mut manifest: Value = read_json(&manifest_path, 64 * 1024 * 1024)?;
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| Error::new("state snapshot manifest is not an object"))?;
    if object.get("schema").and_then(Value::as_str)
        != Some("astrid.edge_checkpoint.rollback_state.v2")
        || object.get("generation_id").and_then(Value::as_str) != Some(generation_id)
        || object.get("authority").and_then(Value::as_str)
            != Some("immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim")
        || object.get("quiescence_policy").and_then(Value::as_str)
            != Some("exact_signed_runtime_stopped_transition_record")
        || object.get("retention_policy").and_then(Value::as_str)
            != Some("paired_with_rollback_generation_no_independent_gc")
        || object
            .get("minimum_prior_generations")
            .and_then(Value::as_u64)
            != Some(3)
        || object
            .get("minimum_retention_seconds")
            .and_then(Value::as_u64)
            != Some(7 * 24 * 60 * 60)
    {
        return Err(Error::new("state snapshot manifest provenance failed"));
    }
    let quiescence_record_sha256 = object
        .get("quiescence_record_sha256")
        .and_then(Value::as_str)
        .filter(|value| crate::config::valid_hex64(value))
        .ok_or_else(|| Error::new("state snapshot quiescence binding is absent"))?
        .to_owned();
    let claimed = object
        .remove("manifest_sha256")
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|value| crate::config::valid_hex64(value))
        .ok_or_else(|| Error::new("state snapshot manifest digest is absent"))?;
    if sha256(&canonical_json(&manifest)?) != claimed {
        return Err(Error::new("state snapshot manifest self-hash failed"));
    }
    verify_snapshot_quiescence_record(
        config,
        &quiescence_record_sha256,
        generation_id,
        require_root,
    )?;
    Ok(StateSnapshotBinding {
        basename: basename.to_owned(),
        generation_id: generation_id.to_owned(),
        manifest_sha256: claimed,
    })
}

fn verify_snapshot_quiescence_record(
    config: &Config,
    record_sha256: &str,
    generation_id: &str,
    require_root: bool,
) -> Result<()> {
    let path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let records = verify_phase_journal_records(&path, &key, require_root)?;
    let record = records
        .iter()
        .find(|record| record.record_sha256 == record_sha256)
        .ok_or_else(|| Error::new("snapshot quiescence transition record is absent"))?;
    if record.operation != "activation"
        || record.phase != "runtime_stopped"
        || record.prior_generation_id != generation_id
        || record.state_snapshot.is_some()
    {
        return Err(Error::new(
            "snapshot quiescence transition record does not bind stopped prior state",
        ));
    }
    Ok(())
}

/// Fail closed if a retained rollback generation and its exact pre-switch
/// state image have been garbage-collected independently. No immutable helper
/// command currently deletes either side; this invariant is the admission
/// boundary for any future paired GC implementation.
pub(crate) fn verify_retained_rollback_pairs(config: &Config, require_root: bool) -> Result<()> {
    verify_retained_rollback_pairs_at(config, require_root, unix_millis())
}

pub(crate) fn verify_retained_rollback_pairs_at(
    config: &Config,
    require_root: bool,
    now: u64,
) -> Result<()> {
    let path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let records = verify_phase_journal_records(&path, &key, require_root)?;
    let mut pairs = Vec::<(u64, String, StateSnapshotBinding)>::new();
    let mut seen = BTreeSet::new();
    for record in records {
        let Some(binding) = record.state_snapshot else {
            continue;
        };
        if record.operation != "activation"
            || record.phase != "state_flushed_checkpointed_and_snapshotted"
            || binding.generation_id != record.prior_generation_id
        {
            continue;
        }
        if seen.insert(binding.manifest_sha256.clone()) {
            pairs.push((
                record.recorded_at_unix_ms,
                record.prior_generation_id,
                binding,
            ));
        }
    }
    let mut latest_prior = BTreeSet::new();
    for (_, generation_id, _) in pairs.iter().rev() {
        if latest_prior.len() >= MINIMUM_RETAINED_PRIOR_GENERATIONS {
            break;
        }
        latest_prior.insert(generation_id.clone());
    }
    for (recorded_at, generation_id, binding) in pairs {
        let release = config.roots.releases.join(&generation_id);
        let snapshot = config.roots.state_snapshots.join(&binding.basename);
        let release_exists = release.exists() || release.is_symlink();
        let snapshot_exists = snapshot.exists() || snapshot.is_symlink();
        let within_minimum_age =
            now.saturating_sub(recorded_at) <= MINIMUM_STATE_SNAPSHOT_RETENTION_MILLISECONDS;
        let minimum_generation = latest_prior.contains(&generation_id);
        if release_exists != snapshot_exists {
            return Err(Error::new(
                "rollback generation/state-snapshot pair was retired independently",
            ));
        }
        if (within_minimum_age || minimum_generation) && (!release_exists || !snapshot_exists) {
            return Err(Error::new(
                "minimum rollback generation/state-snapshot pair was retired early",
            ));
        }
        if release_exists {
            let identity = validate_transition_release(config, &release, require_root)?;
            if identity.generation_id != generation_id
                || read_state_snapshot_binding(config, &snapshot, &generation_id, require_root)?
                    != binding
            {
                return Err(Error::new(
                    "retained rollback generation/state-snapshot pair binding failed",
                ));
            }
        }
    }
    Ok(())
}

fn restore_state_snapshot<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    binding: &StateSnapshotBinding,
    transaction_id: &str,
    require_root: bool,
) -> Result<()> {
    if !valid_identifier(transaction_id) {
        return Err(Error::new("state restore transaction identity is invalid"));
    }
    let snapshot = config.roots.state_snapshots.join(&binding.basename);
    if read_state_snapshot_binding(config, &snapshot, &binding.generation_id, require_root)?
        != *binding
    {
        return Err(Error::new("state restore snapshot binding changed"));
    }
    if require_root {
        release_inode_reserve_for_state_restore(config, runner, receipts, binding, transaction_id)?;
    }
    let restore_result = run_step(
        config,
        runner,
        receipts,
        &config.executables.checkpoint,
        "rollback-state-restore",
        vec![
            "restore".into(),
            "--workspace".into(),
            config.roots.workspace.display().to_string(),
            "--snapshot".into(),
            snapshot.display().to_string(),
            "--generation-id".into(),
            binding.generation_id.clone(),
            "--transaction-id".into(),
            transaction_id.to_owned(),
        ],
        &config.roots.state_snapshots,
    );
    if !require_root {
        return restore_result;
    }
    let reserve_result = restore_inode_reserve_after_state_restore(
        config,
        runner,
        receipts,
        transaction_id,
        restore_result.is_ok(),
    );
    match (restore_result, reserve_result) {
        (Ok(()), Ok(())) => {},
        (Err(restore_error), Ok(())) => return Err(restore_error),
        (Ok(()), Err(reserve_error)) => return Err(reserve_error),
        (Err(restore_error), Err(reserve_error)) => {
            return Err(Error::new(format!(
                "state restore failed and the emergency inode reserve could not be recreated: {}; {}",
                restore_error.message(),
                reserve_error.message()
            )));
        },
    }
    run_state_store_attestation_refresh(config, runner, receipts)?;
    let _ = crate::storage::verify(config, false)?;
    Ok(())
}

fn release_inode_reserve_for_state_restore<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    binding: &StateSnapshotBinding,
    transaction_id: &str,
) -> Result<()> {
    let journal = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, true)?;
    let head = verify_phase_journal(&journal, &key, true)?
        .ok_or_else(|| Error::new("state restore transition journal is absent"))?;
    if head.state_snapshot.as_ref() != Some(binding)
        || !state_restore_required_for_head(&head)
        || head
            .state_restore_transaction_id
            .as_ref()
            .is_some_and(|value| value != transaction_id)
    {
        return Err(Error::new(
            "state restore does not bind the exact signed transition head",
        ));
    }
    append_state_restore_reserve_phase(
        config,
        &head,
        RESERVE_RELEASE_AUTHORIZED_PHASE,
        transaction_id,
        true,
    )?;
    run_state_store_step(
        config,
        runner,
        receipts,
        "state-inode-reserve-release",
        "release-inode-reserve",
    )?;
    let released_from = verify_phase_journal(&journal, &key, true)?
        .ok_or_else(|| Error::new("state restore reserve authorization disappeared"))?;
    let Err(error) = append_state_restore_reserve_phase(
        config,
        &released_from,
        RESERVE_RELEASED_PHASE,
        transaction_id,
        true,
    ) else {
        return Ok(());
    };
    let reserve_recovery = run_state_store_step(
        config,
        runner,
        receipts,
        "state-inode-reserve-recover-after-journal-failure",
        "restore-inode-reserve",
    );
    if reserve_recovery.is_ok() {
        let _ = append_state_restore_reserve_phase(
            config,
            &released_from,
            RESERVE_RECONCILED_PHASE,
            transaction_id,
            true,
        );
    }
    Err(Error::new(format!(
        "inode reserve was released but its signed phase could not be recorded: {}; recovery={}",
        error.message(),
        if reserve_recovery.is_ok() {
            "restored"
        } else {
            "failed"
        }
    )))
}

fn restore_inode_reserve_after_state_restore<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    transaction_id: &str,
    state_restore_succeeded: bool,
) -> Result<()> {
    run_state_store_step(
        config,
        runner,
        receipts,
        "state-inode-reserve-restore",
        "restore-inode-reserve",
    )?;
    let journal = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, true)?;
    let head = verify_phase_journal(&journal, &key, true)?
        .ok_or_else(|| Error::new("state restore reserve release phase disappeared"))?;
    append_state_restore_reserve_phase(
        config,
        &head,
        if state_restore_succeeded {
            RESERVE_RESTORED_PHASE
        } else {
            RESERVE_RESTORED_AFTER_FAILURE_PHASE
        },
        transaction_id,
        true,
    )?;
    Ok(())
}

fn run_state_store_step<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    label: &'static str,
    command: &'static str,
) -> Result<()> {
    config.executables.state_store.verify()?;
    run_step(
        config,
        runner,
        receipts,
        &config.executables.python,
        label,
        vec![
            "-I".into(),
            "-E".into(),
            "-s".into(),
            config.executables.state_store.path.display().to_string(),
            command.into(),
            "--config".into(),
            config.storage.config.display().to_string(),
        ],
        &config.roots.state_snapshots,
    )
}

fn run_state_store_attestation_refresh<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
) -> Result<()> {
    config.executables.state_store.verify()?;
    run_step(
        config,
        runner,
        receipts,
        &config.executables.python,
        "state-storage-attestation-refresh",
        vec![
            "-I".into(),
            "-E".into(),
            "-s".into(),
            config.executables.state_store.path.display().to_string(),
            "attest".into(),
            "--config".into(),
            config.storage.config.display().to_string(),
            "--output".into(),
            config.storage.health_attestation.display().to_string(),
        ],
        &config.roots.state_snapshots,
    )
}

fn stop_runtime<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
) -> Result<()> {
    for unit in [&config.services.edge, &config.services.core] {
        systemctl(config, runner, receipts, "systemd-stop", &["stop", unit])?;
    }
    Ok(())
}

fn start_runtime<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
) -> Result<()> {
    systemctl(
        config,
        runner,
        receipts,
        "systemd-daemon-reload",
        &["daemon-reload"],
    )?;
    // A candidate can fail rapidly enough to consume systemd's start limit.
    // Clear only the three fixed Astrid units after the generation pointer is
    // validated so that the immutable prior slot remains startable during
    // automatic recovery.
    for unit in [
        &config.services.core,
        &config.services.warmup,
        &config.services.edge,
    ] {
        systemctl(
            config,
            runner,
            receipts,
            "systemd-reset-failed",
            &["reset-failed", unit],
        )?;
    }
    systemctl(
        config,
        runner,
        receipts,
        "systemd-start-core",
        &["start", &config.services.core],
    )?;
    systemctl(
        config,
        runner,
        receipts,
        "systemd-core-is-active",
        &["is-active", "--quiet", &config.services.core],
    )?;

    // Warmup is RemainAfterExit.  A plain `start` can therefore be a no-op
    // after a build or interrupted transition and would not prove that the
    // selected model is resident.  Restart the immutable gateway client and
    // wait for its oneshot result before edge inference is allowed to start.
    systemctl(
        config,
        runner,
        receipts,
        "systemd-restart-warmup",
        &["restart", &config.services.warmup],
    )?;
    systemctl(
        config,
        runner,
        receipts,
        "systemd-warmup-is-active",
        &["is-active", "--quiet", &config.services.warmup],
    )?;

    systemctl(
        config,
        runner,
        receipts,
        "systemd-start-edge",
        &["start", &config.services.edge],
    )?;
    for unit in [&config.services.core, &config.services.edge] {
        systemctl(
            config,
            runner,
            receipts,
            "systemd-is-active",
            &["is-active", "--quiet", unit],
        )?;
    }
    Ok(())
}

fn systemctl<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    label: &'static str,
    arguments: &[&str],
) -> Result<()> {
    run_step(
        config,
        runner,
        receipts,
        &config.executables.systemctl,
        label,
        arguments.iter().map(|item| (*item).to_owned()).collect(),
        &config.roots.workspace,
    )
}

fn run_step<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    receipts: &mut Vec<CommandReceipt>,
    executable: &crate::config::TrustedExecutable,
    label: &'static str,
    arguments: Vec<String>,
    cwd: &Path,
) -> Result<()> {
    let timeout_seconds = if matches!(
        label,
        "rollback-compatible-state-snapshot"
            | "rollback-state-snapshot-verification"
            | "rollback-state-restore"
            | "state-inode-reserve-release"
            | "state-inode-reserve-restore"
    ) {
        config.policy.command_timeout_seconds.min(3_600)
    } else {
        config.policy.command_timeout_seconds.min(300)
    };
    let receipt = runner.run(&CommandSpec {
        label,
        executable: executable.clone(),
        arguments,
        current_dir: cwd.to_path_buf(),
        environment: BTreeMap::from([
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
        timeout: Duration::from_secs(timeout_seconds),
        run_as_uid: None,
        run_as_gid: None,
    })?;
    require_success(&receipt)?;
    receipts.push(receipt);
    Ok(())
}

/// Reconcile an interrupted two-file generation switch before systemd is
/// allowed to start mutable services. The phase journal is authoritative about
/// whether the target reached the probation/rollback commit point. Every
/// earlier phase resolves to the prior generation. Reconciliation is
/// repeatable across power loss: the journal is advanced only after both the
/// symlink and root binding have been durably rewritten and revalidated.
pub fn reconcile_active_generation<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
) -> Result<ReconciliationOutcome> {
    require_effective_uid(0, "active generation reconciliation")?;
    reconcile_active_generation_inner(config, runner, true)
}

/// Recreate the runtime inode reserve before strict storage verification only
/// when the signed transition head proves an interrupted exact state restore.
/// An unrelated or absent transition can verify an existing reserve but can
/// never use absence as evidence that deletion was authorized.
pub fn reconcile_storage_reserve<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
) -> Result<StorageReserveReconciliationOutcome> {
    require_effective_uid(0, "storage reserve reconciliation")?;
    let journal = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, true)?;
    let head = verify_phase_journal(&journal, &key, true)?;
    let pending = head.as_ref().filter(|value| {
        matches!(
            value.phase.as_str(),
            RESERVE_RELEASE_AUTHORIZED_PHASE | RESERVE_RELEASED_PHASE
        ) && state_restore_required_for_head(value)
            && value.state_snapshot.is_some()
            && value.state_restore_transaction_id.is_some()
    });
    let mut receipts = Vec::new();
    if let Some(pending) = pending {
        run_state_store_step(
            config,
            runner,
            &mut receipts,
            "state-inode-reserve-boot-recovery",
            "recover-inode-reserve-at-boot",
        )?;
        let transaction_id = pending
            .state_restore_transaction_id
            .as_deref()
            .ok_or_else(|| Error::new("reserve recovery transaction identity is absent"))?;
        let record_sha256 = append_state_restore_reserve_phase(
            config,
            pending,
            RESERVE_RECONCILED_PHASE,
            transaction_id,
            true,
        )?;
        let receipt = receipts
            .pop()
            .ok_or_else(|| Error::new("reserve recovery command receipt is absent"))?;
        return Ok(StorageReserveReconciliationOutcome {
            schema: "astrid.edge_rescue_helper.storage_reserve_reconciliation.v1",
            status: "verified_or_recreated_from_signed_restore_transaction",
            transition_record_sha256: Some(record_sha256),
            state_restore_transaction_id: Some(transaction_id.to_owned()),
            command_receipt: receipt,
        });
    }
    run_state_store_step(
        config,
        runner,
        &mut receipts,
        "state-inode-reserve-boot-verification",
        "verify-inode-reserve-at-boot",
    )?;
    let receipt = receipts
        .pop()
        .ok_or_else(|| Error::new("reserve verification command receipt is absent"))?;
    Ok(StorageReserveReconciliationOutcome {
        schema: "astrid.edge_rescue_helper.storage_reserve_reconciliation.v1",
        status: "healthy_no_signed_restore_recovery_needed",
        transition_record_sha256: head.map(|value| value.record_sha256),
        state_restore_transaction_id: None,
        command_receipt: receipt,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "reboot reconciliation is one ordered recovery state machine across the signed phase journal and generation pointers"
)]
pub(crate) fn reconcile_active_generation_inner<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    require_root: bool,
) -> Result<ReconciliationOutcome> {
    if require_root {
        let _ = crate::storage::verify(config, true)?;
    }
    let guard =
        MaintenanceLease::acquire_for_inner(config, "boot_reconciliation", 300, require_root)?;
    let _ = crate::retention::reconcile(config, require_root)?;
    let journal = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let head = verify_phase_journal(&journal, &key, require_root)?;
    verify_retained_rollback_pairs(config, require_root)?;
    let Some(head) = head else {
        let binding = read_generation_binding(config, require_root)?;
        let generation = config.roots.releases.join(&binding);
        // Preparing the two runtime-surface transactions necessarily precedes
        // the first outer phase-journal append. A power loss in that narrow
        // window leaves no transition head but does leave sealed pending
        // profile/unit transactions. Reconcile those to the already selected
        // root generation before validating its effective projection.
        let profile_evidence =
            crate::profile_projection::reconcile_for_transition(config, &generation, require_root)?;
        let unit_evidence = crate::unit_transaction::reconcile_for_transition(
            config,
            runner,
            &generation,
            require_root,
        )?;
        validate_active_binding(config, &generation, &binding, require_root)?;
        return Ok(ReconciliationOutcome {
            schema: "astrid.edge_rescue_helper.reconciliation.v1",
            status: if unit_evidence.transaction_id == "none"
                && profile_evidence.transaction_id == "none"
            {
                "consistent_no_transition_history"
            } else {
                "runtime_surfaces_reconciled_no_transition_history"
            },
            generation_id: binding,
            transition_record_sha256: None,
        });
    };
    verify_runtime_projection_binding(config, &head, require_root)?;
    let committed_to_target = transition_committed_to_target(&head);
    let desired = if committed_to_target {
        &head.target_generation_id
    } else {
        &head.prior_generation_id
    };
    let generation = config.roots.releases.join(desired);
    let identity = validate_transition_release(config, &generation, require_root)?;
    if identity.generation_id != *desired {
        return Err(Error::new(
            "transition journal generation differs from its signed release",
        ));
    }
    let restore_required =
        (head.operation == "activation" && !committed_to_target && head.state_snapshot.is_some())
            || (head.operation == "rollback" && committed_to_target);
    if restore_required {
        if let Some(binding) = head.state_snapshot.as_ref() {
            let mut restore_receipts = Vec::new();
            let restore_transaction_id = head
                .state_restore_transaction_id
                .clone()
                .unwrap_or_else(|| format!("boot-{}", &head.record_sha256[..24]));
            restore_state_snapshot(
                config,
                runner,
                &mut restore_receipts,
                binding,
                &restore_transaction_id,
                require_root,
            )?;
        } else if require_root {
            return Err(Error::new(
                "boot reconciliation cannot restore mutable state because the signed snapshot binding is absent",
            ));
        }
    }
    if head.operation == "activation" && !committed_to_target {
        // A power loss can occur after the probation ledger's durable
        // `started` append but before `probation_started` reaches the
        // transition journal. The transition is still uncommitted, so close
        // that orphaned target probation before restoring the prior slot.
        crate::probation::close_for_rollback_inner(
            config,
            &head.target_generation_id,
            "boot_reconciled_uncommitted_activation",
            require_root,
        )?;
    } else if head.operation == "rollback" && committed_to_target {
        crate::probation::close_for_rollback_inner(
            config,
            &head.prior_generation_id,
            "boot_reconciled_committed_rollback",
            require_root,
        )?;
    }
    let active_matches = active_target(config)
        .and_then(|active| Ok(active == fs::canonicalize(&generation)?))
        .unwrap_or(false);
    let binding_matches =
        read_generation_binding(config, require_root).is_ok_and(|value| value == *desired);
    let profile_evidence =
        crate::profile_projection::reconcile_for_transition(config, &generation, require_root)?;
    let unit_evidence = crate::unit_transaction::reconcile_for_transition(
        config,
        runner,
        &generation,
        require_root,
    )?;
    let surfaces_reconciled =
        unit_evidence.transaction_id != "none" || profile_evidence.transaction_id != "none";
    if active_matches && binding_matches && !surfaces_reconciled {
        return Ok(ReconciliationOutcome {
            schema: "astrid.edge_rescue_helper.reconciliation.v1",
            status: "consistent_with_transition_journal",
            generation_id: desired.clone(),
            transition_record_sha256: Some(head.record_sha256),
        });
    }
    if !active_matches {
        switch_link(config, &generation, require_root)?;
    }
    if !binding_matches {
        write_generation_binding(config, desired)?;
    }
    validate_active_binding(config, &generation, desired, require_root)?;
    let phase = if committed_to_target {
        "boot_reconciled_to_target"
    } else {
        "boot_reconciled_to_prior"
    };
    append_transition_phase(
        config,
        &guard,
        &head.operation,
        phase,
        &head.target_generation_id,
        &head.prior_generation_id,
        require_root,
    )?;
    Ok(ReconciliationOutcome {
        schema: "astrid.edge_rescue_helper.reconciliation.v1",
        status: "reconciled_before_runtime_start",
        generation_id: desired.clone(),
        transition_record_sha256: Some(head.record_sha256),
    })
}

fn transition_committed_to_target(head: &TransitionJournalHead) -> bool {
    matches!(
        (head.operation.as_str(), head.phase.as_str()),
        ("activation", "probation_started")
            | (
                "rollback",
                "mutable_state_restore_started"
                    | RESERVE_RELEASE_AUTHORIZED_PHASE
                    | RESERVE_RELEASED_PHASE
                    | RESERVE_RECONCILED_PHASE
                    | RESERVE_RESTORED_PHASE
                    | RESERVE_RESTORED_AFTER_FAILURE_PHASE
                    | "mutable_state_restored"
                    | "unit_fragments_installed_and_reloaded"
                    | "runtime_surfaces_installed_and_reloaded"
                    | "switch_intent_recorded"
                    | "pointer_and_binding_switched"
                    | "rollback_target_validated"
                    | "completed"
            )
            | ("activation" | "rollback", "boot_reconciled_to_target")
    )
}

fn reserve_transaction_phase(phase: &str) -> bool {
    matches!(
        phase,
        RESERVE_RELEASE_AUTHORIZED_PHASE
            | RESERVE_RELEASED_PHASE
            | RESERVE_RECONCILED_PHASE
            | RESERVE_RESTORED_PHASE
            | RESERVE_RESTORED_AFTER_FAILURE_PHASE
    )
}

fn state_restore_required_for_head(head: &TransitionJournalHead) -> bool {
    head.state_snapshot.is_some()
        && match head.operation.as_str() {
            "activation" => !transition_committed_to_target(head),
            "rollback" => transition_committed_to_target(head),
            _ => false,
        }
}

fn verify_runtime_projection_binding(
    config: &Config,
    head: &TransitionJournalHead,
    require_current_binding: bool,
) -> Result<()> {
    let Some(binding) = head.runtime_projections.as_ref() else {
        if require_current_binding
            && !matches!(
                head.phase.as_str(),
                "completed" | "boot_reconciled_to_target"
            )
        {
            return Err(Error::new(
                "in-progress transition predates runtime projection binding; rescue review required",
            ));
        }
        return Ok(());
    };
    let target = crate::profile_projection::generation_projection_evidence(
        config,
        &config.roots.releases.join(&head.target_generation_id),
    )?;
    let prior = crate::profile_projection::generation_projection_evidence(
        config,
        &config.roots.releases.join(&head.prior_generation_id),
    )?;
    if binding.target_profile_sha256 != target.active_profile_sha256
        || binding.prior_profile_sha256 != prior.active_profile_sha256
        || binding.target_reports_sha256 != target.report_projection_sha256
        || binding.prior_reports_sha256 != prior.report_projection_sha256
    {
        return Err(Error::new(
            "signed transition runtime projection binding differs from retained releases",
        ));
    }
    Ok(())
}

pub(crate) fn active_target(config: &Config) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(&config.roots.active_link)?;
    if !metadata.file_type().is_symlink() {
        return Err(Error::new("active generation pointer is not a symlink"));
    }
    let target = fs::read_link(&config.roots.active_link)?;
    let target = if target.is_absolute() {
        target
    } else {
        config
            .roots
            .active_link
            .parent()
            .ok_or_else(|| Error::new("active link has no parent"))?
            .join(target)
    };
    let target = fs::canonicalize(target)?;
    ensure_within(&config.roots.releases, &target, true)?;
    Ok(target)
}

fn switch_link(config: &Config, generation: &Path, require_root_owner: bool) -> Result<()> {
    validate_transition_release(config, generation, require_root_owner)?;
    let release_parent = config
        .roots
        .releases
        .parent()
        .ok_or_else(|| Error::new("release root has no parent"))?;
    if config.roots.active_link.parent() != Some(release_parent) {
        return Err(Error::new(
            "active link must live beside the immutable releases root",
        ));
    }
    let generation_name = generation
        .file_name()
        .ok_or_else(|| Error::new("generation has no basename"))?;
    let release_name = config
        .roots
        .releases
        .file_name()
        .ok_or_else(|| Error::new("release root has no basename"))?;
    let target = Path::new(release_name).join(generation_name);
    let nonce = random_nonce()?;
    let temporary = release_parent.join(format!(
        ".current.{}.{}.partial",
        std::process::id(),
        &nonce[..16]
    ));
    if temporary.exists() || temporary.is_symlink() {
        return Err(Error::new("temporary active pointer collision"));
    }
    std::os::unix::fs::symlink(target, &temporary)?;
    if let Err(error) = fs::rename(&temporary, &config.roots.active_link) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    File::open(release_parent)?.sync_all()?;
    Ok(())
}

pub(crate) fn validate_transition_release(
    config: &Config,
    generation: &Path,
    require_root_owner: bool,
) -> Result<ReleaseIdentity> {
    #[cfg(test)]
    if !require_root_owner {
        let generation_id = generation
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| crate::config::valid_identifier(value))
            .ok_or_else(|| Error::new("test release generation identity is invalid"))?
            .to_owned();
        let value: Value = read_json(&generation.join(".astrid-edge-generation.json"), 64 * 1024)?;
        if value.get("appliance_id").and_then(Value::as_str) != Some(config.appliance_id.as_str())
            || value.get("generation_id").and_then(Value::as_str) != Some(&generation_id)
            || value.get("target").and_then(Value::as_str) != Some(config.target.as_str())
        {
            return Err(Error::new("test release manifest identity failed"));
        }
        return Ok(ReleaseIdentity {
            appliance_id: config.appliance_id.clone(),
            generation_id,
            target: config.target.clone(),
            operator_initial: false,
        });
    }
    validate_release_manifest_inner(config, generation, require_root_owner)
}

pub(crate) fn read_generation_binding(config: &Config, require_root: bool) -> Result<String> {
    let metadata = fs::symlink_metadata(&config.roots.generation_binding)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || (require_root && metadata.uid() != 0)
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(
            "root current-generation binding is not immutable",
        ));
    }
    let bytes = read_regular(&config.roots.generation_binding, 256)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("generation binding is not UTF-8"))?
        .trim();
    if !crate::config::valid_identifier(value) {
        return Err(Error::new("generation binding is malformed"));
    }
    Ok(value.to_owned())
}

fn write_generation_binding(config: &Config, generation_id: &str) -> Result<()> {
    if !crate::config::valid_identifier(generation_id) {
        return Err(Error::new("refusing malformed generation binding"));
    }
    atomic_write(
        &config.roots.generation_binding,
        format!("{generation_id}\n").as_bytes(),
        0o444,
        true,
    )
}

pub(crate) struct DrainGuard {
    model_lock: Option<File>,
    barrier: Option<DrainBarrierSnapshot>,
}

#[derive(Debug, Clone)]
struct DrainBarrierSnapshot {
    edge_barrier_sha256: String,
    core_barrier_sha256: String,
    drain_barrier_sequence: u64,
    edge_acknowledged_at_unix_ms: u64,
    core_acknowledged_at_unix_ms: u64,
}

impl DrainBarrierSnapshot {
    fn preserves(&self, current: &Self) -> bool {
        self.edge_barrier_sha256 == current.edge_barrier_sha256
            && self.core_barrier_sha256 == current.core_barrier_sha256
            && self.drain_barrier_sequence == current.drain_barrier_sequence
            && current.edge_acknowledged_at_unix_ms >= self.edge_acknowledged_at_unix_ms
            && current.core_acknowledged_at_unix_ms >= self.core_acknowledged_at_unix_ms
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct CommonAck {
    schema: String,
    role: String,
    lease_schema: String,
    lease_kind: String,
    lease_id: String,
    lease_nonce_sha256: String,
    lease_payload_sha256: String,
    generation_id: String,
    blocked_since_unix_ms: u64,
    acknowledged_at_unix_ms: u64,
    pid: u64,
    process_start_ticks: u64,
    authority: String,
    drain_barrier_sequence: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct EdgeAck {
    #[serde(flatten)]
    common: CommonAck,
    new_work_blocked: bool,
    ipc_sequence_exact: bool,
    scheduled_work_count: u64,
    action_work_count: u64,
    continuation_work_count: u64,
    indexes: EdgeIndexes,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EdgeIndexes {
    autonomy: AutonomyIndex,
    ledgers: LedgerIndexes,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct AutonomyIndex {
    path: String,
    sha256: String,
    size_bytes: u64,
    action_dispatch_pending: bool,
    run_receipt_pending: bool,
    chain_receipt_pending: bool,
    thread_projection_pending: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LedgerIndexes {
    actions: LedgerIndex,
    web: LedgerIndex,
    introspection: LedgerIndex,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LedgerIndex {
    path: String,
    inode: u64,
    size_bytes: u64,
    sha256: String,
    pending_count: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct CoreAck {
    #[serde(flatten)]
    common: CommonAck,
    ipc_user_input_blocked: bool,
    active_conversations: u64,
    active_sessions: u64,
    active_tools: u64,
    active_llm_requests: u64,
}

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

fn wait_for_drain(
    config: &Config,
    lease: &MaintenanceLease,
    require_runtime_ack: bool,
) -> Result<DrainGuard> {
    if !require_runtime_ack {
        return Ok(DrainGuard {
            model_lock: None,
            barrier: None,
        });
    }
    let started = Instant::now();
    let timeout = Duration::from_secs(config.drain.maximum_wait_seconds);
    loop {
        if validate_drain_acknowledgements(config, lease).is_ok()
            && let Some(lock) = try_acquire_model_lock(config)?
        {
            // Re-read every acknowledgement and pending index after acquiring
            // the inference lock. This closes the interval between the first
            // zero-pending snapshot and the maintenance transaction boundary.
            if let Ok(barrier) = validate_drain_acknowledgements(config, lease) {
                return Ok(DrainGuard {
                    model_lock: Some(lock),
                    barrier: Some(barrier),
                });
            }
        }
        if started.elapsed() >= timeout {
            return Err(Error::deferred(
                "runtime did not drain active turns, Actions, and tools",
            ));
        }
        thread::sleep(Duration::from_millis(config.drain.poll_milliseconds));
    }
}

fn try_acquire_model_lock(config: &Config) -> Result<Option<File>> {
    let parent = config
        .drain
        .model_lock
        .parent()
        .ok_or_else(|| Error::new("model lock has no parent"))?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != 0
        || parent_metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(
            "model lock parent is not immutable root-owned state",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(&config.drain.model_lock)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.gid() != config.drain.model_lock_gid
        || metadata.mode() & 0o777 != 0o640
    {
        return Err(Error::new("model lock identity or mode failed"));
    }
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(Error::new(format!(
            "cannot acquire immutable model lock: {error}"
        ))),
    }
}

impl Drop for DrainGuard {
    fn drop(&mut self) {
        if let Some(file) = &self.model_lock {
            let _ = file.unlock();
        }
    }
}

impl DrainGuard {
    fn revalidate_live(&self, config: &Config, lease: &MaintenanceLease) -> Result<()> {
        let Some(expected) = &self.barrier else {
            return Ok(());
        };
        // The captured acknowledgement timestamp may naturally age while
        // checkpointing.  Its exact bytes, lease binding, process identities,
        // barrier sequence, and full pending indexes may not change.
        let current = validate_drain_acknowledgements_inner(config, lease, false, true)?;
        if !expected.preserves(&current) {
            return Err(Error::new(
                "runtime drain acknowledgement changed after the prepared barrier",
            ));
        }
        Ok(())
    }

    fn revalidate_exact_files(&self, config: &Config, lease: &MaintenanceLease) -> Result<()> {
        let Some(expected) = &self.barrier else {
            return Ok(());
        };
        let current = validate_drain_acknowledgements_inner(config, lease, false, false)?;
        if !expected.preserves(&current) {
            return Err(Error::new(
                "runtime drain acknowledgement vanished or changed after stop",
            ));
        }
        Ok(())
    }
}

fn validate_drain_acknowledgements(
    config: &Config,
    lease: &MaintenanceLease,
) -> Result<DrainBarrierSnapshot> {
    validate_drain_acknowledgements_inner(config, lease, true, true)
}

fn validate_drain_acknowledgements_inner(
    config: &Config,
    lease: &MaintenanceLease,
    require_fresh_ack: bool,
    require_live_process: bool,
) -> Result<DrainBarrierSnapshot> {
    let generation = read_generation_binding(config, true)?;
    let (edge, _edge_bytes): (EdgeAck, Vec<u8>) = read_runtime_ack(
        &config.drain.maintenance_edge_acknowledgement,
        config.identities.runtime_uid,
        EDGE_ACK_KEYS,
    )?;
    validate_common_ack(
        config,
        lease,
        &edge.common,
        "edge",
        &generation,
        require_fresh_ack,
    )?;
    if !edge.new_work_blocked {
        return Err(Error::new("edge acknowledgement has not blocked new work"));
    }
    let (autonomy_hash, autonomy_size) = validate_autonomy_state(config)?;
    if edge.indexes.autonomy.path != "autonomous/state.json"
        || edge.indexes.autonomy.sha256 != autonomy_hash
        || edge.indexes.autonomy.size_bytes != autonomy_size
        || edge.indexes.autonomy.action_dispatch_pending
        || edge.indexes.autonomy.run_receipt_pending
        || edge.indexes.autonomy.chain_receipt_pending
        || edge.indexes.autonomy.thread_projection_pending
    {
        return Err(Error::new(
            "edge autonomy acknowledgement differs from immutable state parse",
        ));
    }
    let expected = exact_ledger_indexes(config)?;
    if edge.indexes.ledgers.actions != expected[0]
        || edge.indexes.ledgers.web != expected[1]
        || edge.indexes.ledgers.introspection != expected[2]
    {
        return Err(Error::new(
            "edge ledger acknowledgement differs from immutable full scan",
        ));
    }
    let (core, _core_bytes): (CoreAck, Vec<u8>) = read_runtime_ack(
        &config.drain.maintenance_core_acknowledgement,
        config.identities.runtime_uid,
        CORE_ACK_KEYS,
    )?;
    validate_common_ack(
        config,
        lease,
        &core.common,
        "core",
        &generation,
        require_fresh_ack,
    )?;
    if !core.ipc_user_input_blocked
        || core.active_conversations != 0
        || core.active_sessions != 0
        || core.active_tools != 0
        || core.active_llm_requests != 0
    {
        return Err(Error::new("core acknowledgement still has active work"));
    }
    validate_drain_barrier(&edge, &core)?;
    if require_live_process {
        validate_process(config, &edge.common, "astrid-edge-runtime")?;
        validate_process(config, &core.common, "astrid-daemon")?;
    }
    Ok(DrainBarrierSnapshot {
        edge_barrier_sha256: stable_ack_sha256(&edge)?,
        core_barrier_sha256: stable_ack_sha256(&core)?,
        drain_barrier_sequence: edge.common.drain_barrier_sequence,
        edge_acknowledged_at_unix_ms: edge.common.acknowledged_at_unix_ms,
        core_acknowledged_at_unix_ms: core.common.acknowledged_at_unix_ms,
    })
}

fn stable_ack_sha256<T: Serialize>(ack: &T) -> Result<String> {
    let mut value = serde_json::to_value(ack)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("runtime acknowledgement is not an object"))?
        .remove("acknowledged_at_unix_ms");
    Ok(sha256(&canonical_json(&value)?))
}

fn validate_drain_barrier(edge: &EdgeAck, core: &CoreAck) -> Result<()> {
    if edge.common.drain_barrier_sequence == 0
        || edge.common.drain_barrier_sequence != core.common.drain_barrier_sequence
        || !edge.ipc_sequence_exact
        || edge.scheduled_work_count != 0
        || edge.action_work_count != 0
        || edge.continuation_work_count != 0
    {
        return Err(Error::new(
            "core/edge drain barrier or downstream work boundary is not exact",
        ));
    }
    Ok(())
}

fn read_runtime_ack<T: serde::de::DeserializeOwned>(
    path: &Path,
    uid: u32,
    allowed_keys: &[&str],
) -> Result<(T, Vec<u8>)> {
    let bytes = read_runtime_ack_bytes(path, uid)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("runtime acknowledgement is not an object"))?;
    let actual = object
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed = allowed_keys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if actual != allowed {
        return Err(Error::new(
            "runtime acknowledgement fields differ from the exact contract",
        ));
    }
    let parsed = serde_json::from_value(value)?;
    Ok((parsed, bytes))
}

fn read_runtime_ack_bytes(path: &Path, uid: u32) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "runtime maintenance acknowledgement identity failed",
        ));
    }
    read_regular(path, 64 * 1024)
}

fn validate_common_ack(
    _config: &Config,
    lease: &MaintenanceLease,
    ack: &CommonAck,
    role: &str,
    generation: &str,
    require_fresh: bool,
) -> Result<()> {
    let now = unix_millis();
    if ack.schema != "astrid.edge.maintenance_ack.v2"
        || ack.role != role
        || ack.lease_schema != "astrid.edge_self_change.maintenance_lease.v2"
        || ack.lease_kind != "generation_transition"
        || ack.lease_id != lease.lease_id
        || ack.lease_nonce_sha256 != lease.nonce_sha256
        || ack.lease_payload_sha256 != lease.payload_sha256
        || ack.generation_id != generation
        || ack.blocked_since_unix_ms < lease.created_at_unix_ms
        || ack.acknowledged_at_unix_ms < ack.blocked_since_unix_ms
        || ack.acknowledged_at_unix_ms > now.saturating_add(30_000)
        || (require_fresh && now.saturating_sub(ack.acknowledged_at_unix_ms) > 60_000)
        || ack.authority != "mutable_runtime_acknowledgement_subject_to_immutable_verification"
        || ack.drain_barrier_sequence == 0
    {
        return Err(Error::new(
            "runtime maintenance acknowledgement binding failed",
        ));
    }
    Ok(())
}

fn validate_autonomy_state(config: &Config) -> Result<(String, u64)> {
    let bytes = read_regular(&config.drain.autonomy_state, 2 * 1024 * 1024)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if value.get("schema").and_then(Value::as_str) != Some("astrid_edge_autonomy_state_v3")
        || value.get("last_status").and_then(Value::as_str) == Some("running")
        || value
            .get("action_dispatch_pending")
            .and_then(Value::as_bool)
            != Some(false)
        || value.get("run_receipt_pending").and_then(Value::as_bool) != Some(false)
        || value.get("chain_receipt_pending").and_then(Value::as_bool) != Some(false)
        || value
            .get("thread_projection_pending")
            .is_none_or(|item| !item.is_null())
    {
        return Err(Error::new(
            "autonomy state does not prove a drained v3 boundary",
        ));
    }
    Ok((
        sha256(&bytes),
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    ))
}

fn exact_ledger_indexes(config: &Config) -> Result<[LedgerIndex; 3]> {
    let paths = [
        ("actions", "actions/receipts.jsonl"),
        ("web", "web/receipts.jsonl"),
        ("introspection", "introspection/receipts.jsonl"),
    ];
    let mut indexes = Vec::new();
    for (kind, relative) in paths {
        let path = config.roots.workspace.join(relative);
        if !config.drain.activity_ledgers.contains(&path) {
            return Err(Error::new(
                "immutable pending ledger is absent from drain policy",
            ));
        }
        indexes.push(full_ledger_index(
            &path,
            kind,
            relative,
            config.identities.runtime_uid,
        )?);
    }
    indexes
        .try_into()
        .map_err(|_| Error::new("immutable ledger index count failed"))
}

#[allow(clippy::too_many_lines)]
fn full_ledger_index(path: &Path, kind: &str, relative: &str, uid: u32) -> Result<LedgerIndex> {
    let (bytes, metadata) = read_stable_regular(path, 64 * 1024 * 1024)?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("pending ledger ownership or mode failed"));
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
                    return Err(Error::new("action receipt ledger is malformed or pending"));
                }
            },
            "web" | "introspection" => {
                let identifier = value
                    .get("call_id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty() && id.len() <= 256)
                    .ok_or_else(|| Error::new("tool receipt call identity is absent"))?;
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
                            return Err(Error::new(
                                "tool request receipt is malformed, duplicate, or replayed",
                            ));
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
                            return Err(Error::new("tool completion has no unique exact request"));
                        }
                    },
                    None => {
                        // Historical completion-only records remain terminal
                        // and unattributed. They never satisfy or cancel a v2
                        // request. Exact timestamp proximity is irrelevant.
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
                        let known_legacy = (kind == "web"
                            && schema == "astrid_edge_web_tool_receipt_v1")
                            || (kind == "introspection"
                                && schema == "astrid_edge_introspection_receipt_v1");
                        if !known_legacy
                            || matches!(status, "" | "requested" | "running" | "in_progress")
                            || !crate::config::valid_hex64(result_hash)
                            || !authority.contains("result_not_")
                            || !authority.ends_with("authorship")
                            || pending.contains_key(identifier)
                            || !completed.insert(identifier.to_owned())
                        {
                            return Err(Error::new(
                                "legacy completion-only tool receipt is not exact terminal evidence",
                            ));
                        }
                    },
                    _ => return Err(Error::new("tool receipt phase is unsupported")),
                }
            },
            _ => return Err(Error::new("unsupported immutable ledger kind")),
        }
    }
    let pending_count = u64::try_from(pending.len()).unwrap_or(u64::MAX);
    if pending_count != 0 {
        return Err(Error::new("tool receipt ledger contains pending calls"));
    }
    Ok(LedgerIndex {
        path: relative.to_owned(),
        inode: metadata.ino(),
        size_bytes: metadata.len(),
        sha256: sha256(&bytes),
        pending_count,
    })
}

fn read_stable_regular(path: &Path, maximum: u64) -> Result<(Vec<u8>, fs::Metadata)> {
    let path_before = fs::symlink_metadata(path)?;
    if !path_before.is_file()
        || path_before.file_type().is_symlink()
        || path_before.nlink() != 1
        || path_before.len() > maximum
    {
        return Err(Error::new("bounded stable input identity failed"));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let opened = file.metadata()?;
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    std::io::Read::by_ref(&mut file)
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
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != opened.len()
        || identity(&path_before) != identity(&opened)
        || identity(&opened) != identity(&after)
        || identity(&after) != identity(&path_after)
    {
        return Err(Error::deferred("bounded input changed during exact scan"));
    }
    Ok((bytes, opened))
}

fn validate_process(config: &Config, ack: &CommonAck, binary: &str) -> Result<()> {
    let pid = u32::try_from(ack.pid).map_err(|_| Error::new("ack process ID overflow"))?;
    if pid == 0 {
        return Err(Error::new("ack process ID is zero"));
    }
    let process = PathBuf::from(format!("/proc/{pid}"));
    let status = read_proc_bounded(&process.join("status"), 128 * 1024)?;
    let uid_line = std::str::from_utf8(&status)
        .map_err(|_| Error::new("ack process status is not UTF-8"))?
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| Error::new("ack process UID is unavailable"))?;
    if uid_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u32>().ok())
        != Some(config.identities.runtime_uid)
    {
        return Err(Error::new("ack process runs as the wrong identity"));
    }
    let stat = String::from_utf8(read_proc_bounded(&process.join("stat"), 64 * 1024)?)
        .map_err(|_| Error::new("ack process stat is not UTF-8"))?;
    let after_name = stat
        .rfind(") ")
        .ok_or_else(|| Error::new("ack process stat is malformed"))?;
    let fields_start = after_name.saturating_add(2);
    let start_ticks = stat
        .get(fields_start..)
        .ok_or_else(|| Error::new("ack process stat fields are absent"))?
        .split_whitespace()
        .nth(19)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| Error::new("ack process start time is malformed"))?;
    if start_ticks != ack.process_start_ticks {
        return Err(Error::new("ack process start identity changed"));
    }
    if fs::canonicalize(process.join("exe"))?
        != fs::canonicalize(config.roots.active_link.join(binary))?
    {
        return Err(Error::new(
            "ack process executable differs from active generation",
        ));
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
        return Err(Error::new("process identity input exceeds bound"));
    }
    Ok(bytes)
}

fn random_nonce() -> Result<String> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(&mut value, "{byte:02x}")
            .map_err(|_| Error::new("cannot encode maintenance nonce"))?;
    }
    Ok(value)
}

pub(crate) fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        AutonomyIndex, CommonAck, CoreAck, DrainBarrierSnapshot, EdgeAck, EdgeIndexes,
        GENERATION_TRANSITION_OPERATION_SECONDS, LedgerIndex, LedgerIndexes,
        MAXIMUM_LEASE_LIFETIME_MILLISECONDS, MINIMUM_STATE_SNAPSHOT_RETENTION_MILLISECONDS,
        MaintenanceLease, RESERVE_RECONCILED_PHASE, RESERVE_RELEASE_AUTHORIZED_PHASE,
        RESERVE_RELEASED_PHASE, ReflectionAdmissionPaths, StateSnapshotBinding, activate_inner,
        active_target, append_state_restore_reserve_phase, append_transition_phase,
        append_transition_phase_with_snapshot, exact_stopped_transition_record,
        maintenance_lifetime_milliseconds, read_generation_binding,
        reconcile_active_generation_inner, remove_orphaned_build_lease_inner, rollback_inner,
        stable_ack_sha256, start_runtime, switch_link, unix_millis, validate_common_ack,
        validate_drain_barrier, verify_phase_journal, verify_retained_rollback_pairs,
        write_generation_binding,
    };
    use crate::Result;
    use crate::build::bundle_digest;
    use crate::config::{
        AudioPolicy, Config, DrainConfig, Executables, HealthConfig, IdentityConfig, Policy,
        RootConfig, ServiceConfig, SourceConfig, StorageConfig, TrustedExecutable,
    };
    use crate::fs_guard::{canonical_json, sha256};
    use crate::invariant::{MUTABLE_UNIT_FRAGMENTS, normalized_system_unit};
    use crate::ledger_auth::{LedgerKey, seal_record};
    use crate::native::{CommandReceipt, CommandSpec, NativeRunner};
    use crate::unit_transaction::{PolicyDropin, UnitPolicy};
    use fs2::FileExt;
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    #[test]
    fn transition_lease_covers_full_switch_and_is_always_bounded() {
        assert_eq!(GENERATION_TRANSITION_OPERATION_SECONDS, 7_200);
        assert_eq!(
            maintenance_lifetime_milliseconds(GENERATION_TRANSITION_OPERATION_SECONDS, 900),
            9_000_000
        );
        assert_eq!(maintenance_lifetime_milliseconds(30, 900), 2_700_000);
        assert_eq!(
            maintenance_lifetime_milliseconds(u64::MAX, u64::MAX),
            MAXIMUM_LEASE_LIFETIME_MILLISECONDS
        );
    }

    struct FailingStartRunner {
        failed: bool,
        fail_on_label: Option<&'static str>,
        system_root: std::path::PathBuf,
        workspace: std::path::PathBuf,
        hindsight_state: std::path::PathBuf,
        hindsight_sequence: usize,
        dropins: BTreeMap<String, Vec<String>>,
        labels: Vec<String>,
        commands: Vec<Vec<String>>,
        state_restore_transaction_ids: Vec<String>,
    }

    impl NativeRunner for FailingStartRunner {
        fn run(&mut self, spec: &CommandSpec) -> Result<CommandReceipt> {
            self.labels.push(spec.label.to_owned());
            self.commands.push(spec.arguments.clone());
            if spec.label == "rollback-state-restore" {
                let transaction_id = spec
                    .arguments
                    .windows(2)
                    .find(|pair| pair[0] == "--transaction-id")
                    .map(|pair| pair[1].clone())
                    .ok_or_else(|| crate::Error::new("fixture restore transaction is absent"))?;
                self.state_restore_transaction_ids.push(transaction_id);
            }
            if spec.label == "rollback-compatible-state-snapshot" {
                let option = |name: &str| {
                    spec.arguments
                        .windows(2)
                        .find(|pair| pair[0] == name)
                        .map(|pair| pair[1].clone())
                };
                let output = option("--output")
                    .ok_or_else(|| crate::Error::new("fixture snapshot output is absent"))?;
                let generation = option("--generation-id")
                    .ok_or_else(|| crate::Error::new("fixture generation is absent"))?;
                let quiescence = option("--quiescence-record-sha256")
                    .ok_or_else(|| crate::Error::new("fixture quiescence proof is absent"))?;
                write_snapshot_fixture(Path::new(&output), &generation, &quiescence);
            }
            if matches!(
                spec.label,
                "hindsight-checkpoint" | "post-activation-hindsight-checkpoint"
            ) {
                let option = |name: &str| {
                    spec.arguments
                        .windows(2)
                        .find(|pair| pair[0] == name)
                        .map(|pair| pair[1].clone())
                };
                let output = option("--output")
                    .ok_or_else(|| crate::Error::new("fixture checkpoint output is absent"))?;
                let generation = option("--generation-id")
                    .ok_or_else(|| crate::Error::new("fixture generation is absent"))?;
                let reason = option("--reason")
                    .ok_or_else(|| crate::Error::new("fixture checkpoint reason is absent"))?;
                self.hindsight_sequence = self.hindsight_sequence.saturating_add(1);
                write_hindsight_checkpoint_fixture(
                    &self.workspace,
                    &self.hindsight_state,
                    Path::new(&output),
                    &generation,
                    &reason,
                    self.hindsight_sequence,
                );
            }
            let fail = self.fail_on_label == Some(spec.label);
            if fail {
                self.failed = true;
                self.fail_on_label = None;
            }
            Ok(CommandReceipt {
                label: spec.label.to_owned(),
                execution_boundary: crate::native::CommandExecutionBoundary::TrustedHost,
                executable_sha256: "a".repeat(64),
                argv_sha256: "b".repeat(64),
                exit_code: Some(i32::from(fail)),
                timed_out: false,
                duration_ms: 1,
            })
        }

        fn run_capture(
            &mut self,
            spec: &CommandSpec,
            _maximum: u64,
        ) -> Result<(CommandReceipt, Vec<u8>)> {
            let unit = spec.arguments.get(1).cloned().unwrap_or_default();
            let output = if spec
                .arguments
                .iter()
                .any(|argument| argument == "--property=FragmentPath")
            {
                format!("{}\n", self.system_root.join(&unit).display())
            } else {
                format!(
                    "{}\n",
                    self.dropins
                        .get(&unit)
                        .cloned()
                        .unwrap_or_default()
                        .join(" ")
                )
            };
            Ok((
                CommandReceipt {
                    label: spec.label.to_owned(),
                    execution_boundary: crate::native::CommandExecutionBoundary::TrustedHost,
                    executable_sha256: "a".repeat(64),
                    argv_sha256: "b".repeat(64),
                    exit_code: Some(0),
                    timed_out: false,
                    duration_ms: 1,
                },
                output.into_bytes(),
            ))
        }
    }

    fn common_ack(sequence: u64) -> CommonAck {
        CommonAck {
            schema: "astrid.edge.maintenance_ack.v2".into(),
            role: "fixture".into(),
            lease_schema: "astrid.edge_self_change.maintenance_lease.v2".into(),
            lease_kind: "generation_transition".into(),
            lease_id: "lease-fixture".into(),
            lease_nonce_sha256: "a".repeat(64),
            lease_payload_sha256: "b".repeat(64),
            generation_id: "generation-fixture".into(),
            blocked_since_unix_ms: 1,
            acknowledged_at_unix_ms: 2,
            pid: 1,
            process_start_ticks: 1,
            authority: "mutable_runtime_acknowledgement_subject_to_immutable_verification".into(),
            drain_barrier_sequence: sequence,
        }
    }

    fn ledger_index(path: &str) -> LedgerIndex {
        LedgerIndex {
            path: path.into(),
            inode: 1,
            size_bytes: 0,
            sha256: "c".repeat(64),
            pending_count: 0,
        }
    }

    fn edge_ack(sequence: u64) -> EdgeAck {
        EdgeAck {
            common: common_ack(sequence),
            new_work_blocked: true,
            ipc_sequence_exact: true,
            scheduled_work_count: 0,
            action_work_count: 0,
            continuation_work_count: 0,
            indexes: EdgeIndexes {
                autonomy: AutonomyIndex {
                    path: "autonomous/state.json".into(),
                    sha256: "d".repeat(64),
                    size_bytes: 1,
                    action_dispatch_pending: false,
                    run_receipt_pending: false,
                    chain_receipt_pending: false,
                    thread_projection_pending: false,
                },
                ledgers: LedgerIndexes {
                    actions: ledger_index("actions/receipts.jsonl"),
                    web: ledger_index("web/receipts.jsonl"),
                    introspection: ledger_index("introspection/receipts.jsonl"),
                },
            },
        }
    }

    fn core_ack(sequence: u64) -> CoreAck {
        CoreAck {
            common: common_ack(sequence),
            ipc_user_input_blocked: true,
            active_conversations: 0,
            active_sessions: 0,
            active_tools: 0,
            active_llm_requests: 0,
        }
    }

    #[test]
    fn drain_barrier_requires_equal_sequence_exact_ipc_and_zero_edge_work() {
        let mut edge = edge_ack(41);
        let core = core_ack(41);
        assert!(validate_drain_barrier(&edge, &core).is_ok());

        edge.common.drain_barrier_sequence = 42;
        assert!(validate_drain_barrier(&edge, &core).is_err());
        edge.common.drain_barrier_sequence = 41;
        edge.ipc_sequence_exact = false;
        assert!(validate_drain_barrier(&edge, &core).is_err());
        edge.ipc_sequence_exact = true;
        edge.action_work_count = 1;
        assert!(validate_drain_barrier(&edge, &core).is_err());
        edge.action_work_count = 0;
        edge.scheduled_work_count = 1;
        assert!(validate_drain_barrier(&edge, &core).is_err());
        edge.scheduled_work_count = 0;
        edge.continuation_work_count = 1;
        assert!(validate_drain_barrier(&edge, &core).is_err());
    }

    #[test]
    fn prepared_barrier_hash_ignores_only_ack_refresh_time() {
        let mut edge = edge_ack(41);
        let original = stable_ack_sha256(&edge).unwrap();
        edge.common.acknowledged_at_unix_ms = 99;
        assert_eq!(stable_ack_sha256(&edge).unwrap(), original);

        edge.indexes.autonomy.sha256 = "e".repeat(64);
        assert_ne!(stable_ack_sha256(&edge).unwrap(), original);
        edge.indexes.autonomy.sha256 = "d".repeat(64);
        edge.common.drain_barrier_sequence = 42;
        assert_ne!(stable_ack_sha256(&edge).unwrap(), original);

        let prepared = DrainBarrierSnapshot {
            edge_barrier_sha256: "a".repeat(64),
            core_barrier_sha256: "b".repeat(64),
            drain_barrier_sequence: 41,
            edge_acknowledged_at_unix_ms: 10,
            core_acknowledged_at_unix_ms: 11,
        };
        let mut refreshed = prepared.clone();
        refreshed.edge_acknowledged_at_unix_ms = 12;
        refreshed.core_acknowledged_at_unix_ms = 13;
        assert!(prepared.preserves(&refreshed));
        refreshed.edge_acknowledged_at_unix_ms = 9;
        assert!(!prepared.preserves(&refreshed));
    }

    #[test]
    fn generation_transition_accepts_only_exact_v2_ack_domain() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        let reflection_paths = ReflectionAdmissionPaths {
            lease: canonical.join("reflection.json"),
            admission: canonical.join("reflection-admission.json"),
        };
        let lease = MaintenanceLease::acquire_for_inner_with_hook(
            &config,
            "test",
            30,
            false,
            reflection_paths,
            || Ok(()),
        )
        .unwrap();
        let mut ack = common_ack(7);
        ack.role = "edge".to_owned();
        ack.lease_id.clone_from(&lease.lease_id);
        ack.lease_nonce_sha256.clone_from(&lease.nonce_sha256);
        ack.lease_payload_sha256.clone_from(&lease.payload_sha256);
        ack.generation_id = "generation-1".to_owned();
        ack.blocked_since_unix_ms = lease.created_at_unix_ms;
        ack.acknowledged_at_unix_ms = super::unix_millis();
        assert!(validate_common_ack(&config, &lease, &ack, "edge", "generation-1", false).is_ok());

        ack.lease_kind = "scheduled_reflection".to_owned();
        assert!(validate_common_ack(&config, &lease, &ack, "edge", "generation-1", false).is_err());
    }

    #[test]
    fn native_file_lock_excludes_concurrent_transaction() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lock");
        let first = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        first.try_lock_exclusive().unwrap();
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(second.try_lock_exclusive().is_err());
        drop(first);
        drop(second);
        let _ = std::mem::size_of::<MaintenanceLease>();
    }

    #[test]
    fn timeout_recovery_removes_only_an_unlocked_exact_build_lease() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::write(&config.roots.maintenance_mutex, b"").unwrap();
        fs::set_permissions(
            &config.roots.maintenance_mutex,
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let lease = serde_json::json!({
            "schema": "astrid.edge_self_change.maintenance_lease.v2",
            "created_at_unix_ms": 1,
            "expires_at_unix_ms": 2,
            "reason": "candidate_build",
            "owner": "immutable_astrid_edge_rescue_helper",
            "lease_id": "lease-timeout-fixture",
            "nonce": "timeout-fixture-nonce",
        });
        fs::write(
            &config.roots.maintenance_lease,
            canonical_json(&lease).unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            &config.roots.maintenance_lease,
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();

        let active = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.roots.maintenance_mutex)
            .unwrap();
        active.try_lock_exclusive().unwrap();
        assert!(remove_orphaned_build_lease_inner(&config, false).is_err());
        assert!(config.roots.maintenance_lease.exists());
        active.unlock().unwrap();
        drop(active);

        assert!(remove_orphaned_build_lease_inner(&config, false).unwrap());
        assert!(!config.roots.maintenance_lease.exists());
        assert!(!remove_orphaned_build_lease_inner(&config, false).unwrap());
    }

    #[test]
    fn timeout_recovery_rejects_a_non_build_lease() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::write(&config.roots.maintenance_mutex, b"").unwrap();
        fs::set_permissions(
            &config.roots.maintenance_mutex,
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let lease = serde_json::json!({
            "schema": "astrid.edge_self_change.maintenance_lease.v2",
            "created_at_unix_ms": 1,
            "expires_at_unix_ms": 2,
            "reason": "generation_activation",
            "owner": "immutable_astrid_edge_rescue_helper",
            "lease_id": "lease-activation-fixture",
            "nonce": "activation-fixture-nonce",
        });
        fs::write(
            &config.roots.maintenance_lease,
            canonical_json(&lease).unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            &config.roots.maintenance_lease,
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();

        assert!(remove_orphaned_build_lease_inner(&config, false).is_err());
        assert!(config.roots.maintenance_lease.exists());
    }

    #[test]
    fn scheduled_reflection_blocks_generation_lease_before_creation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        let paths = ReflectionAdmissionPaths {
            lease: canonical.join("reflection.json"),
            admission: canonical.join("reflection-admission.json"),
        };
        fs::write(&paths.admission, b"root-admission-marker").unwrap();

        let result = MaintenanceLease::acquire_for_inner_with_hook(
            &config,
            "test",
            30,
            false,
            paths,
            || Ok(()),
        );

        assert!(result.is_err());
        assert!(!config.roots.maintenance_lease.exists());
    }

    #[test]
    fn scheduled_reflection_interleaving_after_creation_rolls_back_generation_lease() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        let paths = ReflectionAdmissionPaths {
            lease: canonical.join("reflection.json"),
            admission: canonical.join("reflection-admission.json"),
        };
        let raced_lease = paths.lease.clone();

        let result = MaintenanceLease::acquire_for_inner_with_hook(
            &config,
            "test",
            30,
            false,
            paths,
            || {
                fs::write(&raced_lease, b"interleaved-reflection")?;
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!config.roots.maintenance_lease.exists());
        assert_eq!(fs::read(raced_lease).unwrap(), b"interleaved-reflection");
    }

    #[test]
    fn scheduled_reflection_recheck_blocks_switch_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        let paths = ReflectionAdmissionPaths {
            lease: canonical.join("reflection.json"),
            admission: canonical.join("reflection-admission.json"),
        };
        let marker = paths.admission.clone();
        let lease = MaintenanceLease::acquire_for_inner_with_hook(
            &config,
            "test",
            30,
            false,
            paths,
            || Ok(()),
        )
        .unwrap();
        fs::write(marker, b"late-reflection-admission").unwrap();

        assert!(lease.revalidate_no_reflection().is_err());
        drop(lease);
        assert!(!config.roots.maintenance_lease.exists());
    }

    #[test]
    fn failed_new_start_restores_previous_generation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fs::write(&config.drain.autonomy_state, b"{}\n").unwrap();
        for path in &config.drain.activity_ledgers {
            fs::write(path, b"").unwrap();
        }
        let old = generation(&config, "gen-old");
        let new = generation(&config, "gen-new");
        fs::write(&config.roots.generation_binding, b"gen-old\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-old", &config.roots.active_link).unwrap();
        let mut runner = install_unit_fixture(&config, &old, true);
        let result = activate_inner(&config, &mut runner, &new, &old, false);
        assert!(result.is_err());
        assert_eq!(
            fs::read_link(&config.roots.active_link).unwrap(),
            Path::new("releases/gen-old")
        );
        assert!(
            runner.failed,
            "activation failed before startup: {result:?}"
        );
        let stop_positions = runner
            .labels
            .iter()
            .enumerate()
            .filter_map(|(index, label)| (label == "systemd-stop").then_some(index))
            .collect::<Vec<_>>();
        let snapshot_position = runner
            .labels
            .iter()
            .position(|label| label == "rollback-compatible-state-snapshot")
            .unwrap();
        assert!(stop_positions.len() >= 2);
        assert!(
            snapshot_position > stop_positions[1],
            "state copy must begin only after both mutable services stop"
        );
    }

    #[test]
    fn runtime_start_proves_fresh_warmup_before_edge() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        let mut runner = FailingStartRunner {
            failed: false,
            fail_on_label: None,
            system_root: canonical.join("systemd"),
            workspace: canonical.join("workspace"),
            hindsight_state: canonical.join("hindsight"),
            hindsight_sequence: 0,
            dropins: BTreeMap::new(),
            labels: Vec::new(),
            commands: Vec::new(),
            state_restore_transaction_ids: Vec::new(),
        };
        let mut receipts = Vec::new();

        start_runtime(&config, &mut runner, &mut receipts).unwrap();

        let position = |verb: &str, unit: &str| {
            runner
                .commands
                .iter()
                .position(|arguments| {
                    arguments.first().is_some_and(|value| value == verb)
                        && arguments.last().is_some_and(|value| value == unit)
                })
                .unwrap()
        };
        let core_start = position("start", &config.services.core);
        let warmup_restart = position("restart", &config.services.warmup);
        let warmup_active = position("is-active", &config.services.warmup);
        let edge_start = position("start", &config.services.edge);

        assert!(core_start < warmup_restart);
        assert!(warmup_restart < warmup_active);
        assert!(warmup_active < edge_start);
        assert!(!runner.commands.iter().any(|arguments| {
            arguments.first().is_some_and(|value| value == "start")
                && arguments
                    .last()
                    .is_some_and(|value| value == &config.services.warmup)
        }));
    }

    #[test]
    fn warmup_failure_prevents_edge_start() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        let mut runner = FailingStartRunner {
            failed: false,
            fail_on_label: Some("systemd-restart-warmup"),
            system_root: canonical.join("systemd"),
            workspace: canonical.join("workspace"),
            hindsight_state: canonical.join("hindsight"),
            hindsight_sequence: 0,
            dropins: BTreeMap::new(),
            labels: Vec::new(),
            commands: Vec::new(),
            state_restore_transaction_ids: Vec::new(),
        };
        let mut receipts = Vec::new();

        assert!(start_runtime(&config, &mut runner, &mut receipts).is_err());
        assert!(runner.failed);
        assert!(
            !runner
                .labels
                .iter()
                .any(|label| label == "systemd-start-edge")
        );
    }

    #[test]
    fn rollback_switches_generation_and_exact_unit_set_transactionally() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let old = generation(&config, "gen-old");
        let current = generation(&config, "gen-current");
        fs::write(&config.roots.generation_binding, b"gen-current\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-current", &config.roots.active_link).unwrap();
        let mut runner = install_unit_fixture(&config, &current, false);

        rollback_inner(&config, &mut runner, &old, false).unwrap();

        assert_eq!(active_target(&config).unwrap(), old.canonicalize().unwrap());
        assert_eq!(read_generation_binding(&config, false).unwrap(), "gen-old");
        for unit in MUTABLE_UNIT_FRAGMENTS {
            let logical = format!("packaging/systemd/{unit}");
            let source = fs::read_to_string(old.join(&logical)).unwrap();
            assert_eq!(
                fs::read(config.roots.system_unit_root.join(unit)).unwrap(),
                normalized_system_unit(&logical, &source, &config.roots.active_link).unwrap()
            );
        }
        assert!(!config.roots.unit_transactions.join("pending.json").exists());
    }

    #[test]
    fn rollback_is_bound_to_signed_snapshot_and_restores_state_before_switch() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let old = generation(&config, "gen-old");
        let current = generation(&config, "gen-current");
        fs::write(&config.roots.generation_binding, b"gen-current\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-current", &config.roots.active_link).unwrap();
        let activation = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &activation,
            "activation",
            "runtime_stopped",
            "gen-current",
            "gen-old",
            false,
        )
        .unwrap();
        let proof =
            exact_stopped_transition_record(&config, &activation, "gen-current", "gen-old", false)
                .unwrap();
        let binding = snapshot_binding_fixture(&config, "baseline-state", "gen-old", &proof);
        append_transition_phase_with_snapshot(
            &config,
            &activation,
            "activation",
            "state_flushed_checkpointed_and_snapshotted",
            "gen-current",
            "gen-old",
            Some(&binding),
            false,
        )
        .unwrap();
        append_transition_phase(
            &config,
            &activation,
            "activation",
            "probation_started",
            "gen-current",
            "gen-old",
            false,
        )
        .unwrap();
        drop(activation);
        let mut runner = install_unit_fixture(&config, &current, false);

        rollback_inner(&config, &mut runner, &old, false).unwrap();

        let restore = runner
            .labels
            .iter()
            .position(|label| label == "rollback-state-restore")
            .expect("rollback must invoke the immutable restore command");
        let target_apply = runner
            .labels
            .iter()
            .position(|label| label == "systemd-daemon-reload-unit-transaction")
            .expect("rollback must install the target unit set");
        assert!(restore < target_apply);
        assert_eq!(active_target(&config).unwrap(), old.canonicalize().unwrap());
    }

    #[test]
    fn inode_reserve_recovery_requires_exact_signed_restore_transaction() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let _old = generation(&config, "gen-old");
        let _new = generation(&config, "gen-new");
        let lease = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &lease,
            "activation",
            "runtime_stopped",
            "gen-new",
            "gen-old",
            false,
        )
        .unwrap();
        let proof =
            exact_stopped_transition_record(&config, &lease, "gen-new", "gen-old", false).unwrap();
        let binding = snapshot_binding_fixture(&config, "baseline-state", "gen-old", &proof);
        append_transition_phase_with_snapshot(
            &config,
            &lease,
            "activation",
            "state_flushed_checkpointed_and_snapshotted",
            "gen-new",
            "gen-old",
            Some(&binding),
            false,
        )
        .unwrap();
        let key = LedgerKey::load(&config.source.ledger_attestation_key, false).unwrap();
        let journal = config.roots.state_snapshots.join("transitions.jsonl");
        let head = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        append_state_restore_reserve_phase(
            &config,
            &head,
            RESERVE_RELEASE_AUTHORIZED_PHASE,
            "restore-transaction",
            false,
        )
        .unwrap();
        let authorized = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        assert_eq!(authorized.phase, RESERVE_RELEASE_AUTHORIZED_PHASE);
        assert_eq!(
            authorized.state_restore_transaction_id.as_deref(),
            Some("restore-transaction")
        );
        assert!(
            append_state_restore_reserve_phase(
                &config,
                &authorized,
                RESERVE_RELEASED_PHASE,
                "other-transaction",
                false,
            )
            .is_err()
        );
        append_state_restore_reserve_phase(
            &config,
            &authorized,
            RESERVE_RELEASED_PHASE,
            "restore-transaction",
            false,
        )
        .unwrap();
        let released = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        append_state_restore_reserve_phase(
            &config,
            &released,
            RESERVE_RECONCILED_PHASE,
            "restore-transaction",
            false,
        )
        .unwrap();
        let reconciled = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        assert_eq!(reconciled.phase, RESERVE_RECONCILED_PHASE);
        assert_eq!(reconciled.state_snapshot, Some(binding));
    }

    #[test]
    fn boot_reconciliation_reuses_signed_reserve_restore_transaction() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let old = generation(&config, "gen-old");
        let _new = generation(&config, "gen-new");
        fs::write(&config.roots.generation_binding, b"gen-new\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-new", &config.roots.active_link).unwrap();
        let lease = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &lease,
            "activation",
            "runtime_stopped",
            "gen-new",
            "gen-old",
            false,
        )
        .unwrap();
        let proof =
            exact_stopped_transition_record(&config, &lease, "gen-new", "gen-old", false).unwrap();
        let binding = snapshot_binding_fixture(&config, "baseline-state", "gen-old", &proof);
        append_transition_phase_with_snapshot(
            &config,
            &lease,
            "activation",
            "state_flushed_checkpointed_and_snapshotted",
            "gen-new",
            "gen-old",
            Some(&binding),
            false,
        )
        .unwrap();
        let key = LedgerKey::load(&config.source.ledger_attestation_key, false).unwrap();
        let journal = config.roots.state_snapshots.join("transitions.jsonl");
        let head = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        append_state_restore_reserve_phase(
            &config,
            &head,
            RESERVE_RELEASE_AUTHORIZED_PHASE,
            "restore-before-power-loss",
            false,
        )
        .unwrap();
        let authorized = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        append_state_restore_reserve_phase(
            &config,
            &authorized,
            RESERVE_RELEASED_PHASE,
            "restore-before-power-loss",
            false,
        )
        .unwrap();
        let released = verify_phase_journal(&journal, &key, false)
            .unwrap()
            .unwrap();
        append_state_restore_reserve_phase(
            &config,
            &released,
            RESERVE_RECONCILED_PHASE,
            "restore-before-power-loss",
            false,
        )
        .unwrap();
        drop(lease);
        let mut runner = install_unit_fixture(&config, &old, false);

        let outcome = reconcile_active_generation_inner(&config, &mut runner, false).unwrap();

        assert_eq!(outcome.generation_id, "gen-old");
        assert_eq!(
            runner.state_restore_transaction_ids,
            ["restore-before-power-loss"]
        );
        assert_eq!(active_target(&config).unwrap(), old.canonicalize().unwrap());
    }

    #[test]
    fn retained_generation_cannot_outlive_its_exact_state_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let _old = generation(&config, "gen-old");
        let _current = generation(&config, "gen-current");
        let activation = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &activation,
            "activation",
            "runtime_stopped",
            "gen-current",
            "gen-old",
            false,
        )
        .unwrap();
        let proof =
            exact_stopped_transition_record(&config, &activation, "gen-current", "gen-old", false)
                .unwrap();
        let binding = snapshot_binding_fixture(&config, "baseline-state", "gen-old", &proof);
        append_transition_phase_with_snapshot(
            &config,
            &activation,
            "activation",
            "state_flushed_checkpointed_and_snapshotted",
            "gen-current",
            "gen-old",
            Some(&binding),
            false,
        )
        .unwrap();
        drop(activation);

        verify_retained_rollback_pairs(&config, false).unwrap();
        let snapshot = config.roots.state_snapshots.join("baseline-state");
        fs::set_permissions(&snapshot, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(snapshot).unwrap();
        assert!(verify_retained_rollback_pairs(&config, false).is_err());
    }

    #[test]
    fn signed_retention_retires_only_old_pairs_and_preserves_active_plus_three_prior() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let _active = generation(&config, "g0");
        for index in 1..=5 {
            let _ = generation(&config, &format!("g{index}"));
        }
        fs::write(&config.roots.generation_binding, b"g0\n").unwrap();
        std::os::unix::fs::symlink("releases/g0", &config.roots.active_link).unwrap();

        let lease = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        for index in 1..=5 {
            let prior = format!("g{index}");
            append_transition_phase(
                &config,
                &lease,
                "activation",
                "runtime_stopped",
                "g0",
                &prior,
                false,
            )
            .unwrap();
            let proof =
                exact_stopped_transition_record(&config, &lease, "g0", &prior, false).unwrap();
            let basename = format!("activation-{index}-state");
            let binding = snapshot_binding_fixture(&config, &basename, &prior, &proof);
            append_transition_phase_with_snapshot(
                &config,
                &lease,
                "activation",
                "state_flushed_checkpointed_and_snapshotted",
                "g0",
                &prior,
                Some(&binding),
                false,
            )
            .unwrap();
        }
        drop(lease);
        let future = unix_millis()
            .saturating_add(MINIMUM_STATE_SNAPSHOT_RETENTION_MILLISECONDS)
            .saturating_add(1_000);
        let outcome = crate::retention::prune_inner(&config, false, future).unwrap();

        assert_eq!(outcome.status, "retired_complete_signed_pairs");
        assert_eq!(outcome.retired_generations, ["g1", "g2"]);
        assert_eq!(outcome.retained_generations, ["g0", "g3", "g4", "g5"]);
        for index in 1..=2 {
            assert!(!config.roots.releases.join(format!("g{index}")).exists());
            assert!(
                !config
                    .roots
                    .state_snapshots
                    .join(format!("activation-{index}-state"))
                    .exists()
            );
        }
        for index in 3..=5 {
            assert!(config.roots.releases.join(format!("g{index}")).is_dir());
            assert!(
                config
                    .roots
                    .state_snapshots
                    .join(format!("activation-{index}-state"))
                    .is_dir()
            );
        }
        assert!(
            config
                .roots
                .state_snapshots
                .join("retention.jsonl")
                .is_file()
        );
        super::verify_retained_rollback_pairs_at(&config, false, future).unwrap();
    }

    #[test]
    fn boot_reconciliation_restores_prior_across_each_two_file_crash_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let old = generation(&config, "gen-old");
        let new = generation(&config, "gen-new");
        fs::write(&config.roots.generation_binding, b"gen-old\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-old", &config.roots.active_link).unwrap();
        let mut runner = install_unit_fixture(&config, &old, false);
        let lease = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &lease,
            "activation",
            "switch_intent_recorded",
            "gen-new",
            "gen-old",
            false,
        )
        .unwrap();
        drop(lease);

        // Crash after the active symlink rename but before root binding write.
        switch_link(&config, &new, false).unwrap();
        reconcile_active_generation_inner(&config, &mut runner, false).unwrap();
        assert_eq!(active_target(&config).unwrap(), old.canonicalize().unwrap());
        assert_eq!(read_generation_binding(&config, false).unwrap(), "gen-old");

        // Crash after a binding write with the link still on the prior slot.
        write_generation_binding(&config, "gen-new").unwrap();
        reconcile_active_generation_inner(&config, &mut runner, false).unwrap();
        assert_eq!(active_target(&config).unwrap(), old.canonicalize().unwrap());
        assert_eq!(read_generation_binding(&config, false).unwrap(), "gen-old");
    }

    #[test]
    fn committed_probation_generation_is_not_reverted_by_boot_reconciliation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let _old = generation(&config, "gen-old");
        let new = generation(&config, "gen-new");
        fs::write(&config.roots.generation_binding, b"gen-new\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-new", &config.roots.active_link).unwrap();
        let mut runner = install_unit_fixture(&config, &new, false);
        let lease = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &lease,
            "activation",
            "probation_started",
            "gen-new",
            "gen-old",
            false,
        )
        .unwrap();
        drop(lease);

        let outcome = reconcile_active_generation_inner(&config, &mut runner, false).unwrap();
        assert_eq!(outcome.status, "consistent_with_transition_journal");
        assert_eq!(active_target(&config).unwrap(), new.canonicalize().unwrap());
        assert_eq!(read_generation_binding(&config, false).unwrap(), "gen-new");
    }

    #[test]
    fn uncommitted_activation_closes_ledger_only_orphaned_probation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().canonicalize().unwrap();
        let config = fixture_config(&canonical);
        fs::create_dir(&config.roots.releases).unwrap();
        fs::create_dir(&config.roots.workspace).unwrap();
        fs::create_dir(&config.roots.state_snapshots).unwrap();
        fs::set_permissions(
            &config.roots.state_snapshots,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let old = generation(&config, "gen-old");
        let _new = generation(&config, "gen-new");
        fs::write(&config.roots.generation_binding, b"gen-new\n").unwrap();
        std::os::unix::fs::symlink("releases/gen-new", &config.roots.active_link).unwrap();
        let mut runner = install_unit_fixture(&config, &old, false);
        let lease = MaintenanceLease::acquire_for_inner(&config, "test", 30, false).unwrap();
        append_transition_phase(
            &config,
            &lease,
            "activation",
            "pointer_and_binding_switched",
            "gen-new",
            "gen-old",
            false,
        )
        .unwrap();
        let mut probation = serde_json::json!({
            "schema": "astrid.edge_rescue_helper.probation_record.v2",
            "appliance_id": config.appliance_id.clone(),
            "phase": "started",
            "recorded_at_unix_ms": 1,
            "generation_id": "gen-new",
            "previous_generation_id": "gen-old",
            "host_boot_id": "00000000-0000-0000-0000-000000000000",
            "baseline_swap_bytes": 0,
            "authority": "immutable_root_probation_evidence",
            "previous_record_sha256": null
        });
        let ledger_key = LedgerKey::load(&config.source.ledger_attestation_key, false).unwrap();
        let _ = seal_record(&mut probation, &ledger_key, "probation").unwrap();
        fs::write(
            config.roots.state_snapshots.join("probation.jsonl"),
            [canonical_json(&probation).unwrap(), b"\n".to_vec()].concat(),
        )
        .unwrap();
        fs::set_permissions(
            config.roots.state_snapshots.join("probation.jsonl"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        drop(lease);

        reconcile_active_generation_inner(&config, &mut runner, false).unwrap();
        assert_eq!(active_target(&config).unwrap(), old.canonicalize().unwrap());
        assert_eq!(read_generation_binding(&config, false).unwrap(), "gen-old");
        let state: serde_json::Value = serde_json::from_slice(
            &fs::read(config.roots.state_snapshots.join("probation-state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            state.get("status").and_then(serde_json::Value::as_str),
            Some("rolled_back")
        );
    }

    fn generation(config: &Config, id: &str) -> std::path::PathBuf {
        let root = config.roots.releases.join(id);
        fs::create_dir(&root).unwrap();
        for binary in [
            "astrid",
            "astrid-daemon",
            "astrid-build",
            "astrid-edge-runtime",
        ] {
            let path = root.join(binary);
            fs::write(&path, b"native").unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o555)).unwrap();
        }
        for unit in MUTABLE_UNIT_FRAGMENTS {
            let path = root.join("packaging/systemd").join(unit);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let content =
                unit_source(unit).replacen("Description=", &format!("Description={id} "), 1);
            fs::write(path, content).unwrap();
        }
        let profile = root.join("packaging/appliances/avado-i3-16g.env");
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(
            &profile,
            include_bytes!("../../../packaging/appliances/avado-i3-16g.env"),
        )
        .unwrap();
        for report in [
            "astrid_at_a_glance.py",
            "report_edge_activity.py",
            "report_edge_appliance.py",
        ] {
            let path = root.join("scripts").join(report);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, format!("# fixture {id} {report}\n")).unwrap();
        }
        crate::profile_projection::write_release_projection_manifest(config, &[], &root, &root)
            .unwrap();
        let digest = bundle_digest(&root).unwrap();
        let manifest = serde_json::json!({
            "schema":"astrid.edge_self_change.generation.v1",
            "appliance_id":config.appliance_id.clone(),
            "generation_id":id,
            "build_id":format!("build-{id}"),
            "candidate_id":format!("candidate-{id}"),
            "candidate_sha256":"a".repeat(64),
            "base_generation":"gen-base",
            "bundle_sha256":digest,
            "tests_sha256":"c".repeat(64),
            "target":"x86_64-unknown-linux-gnu"
        });
        fs::write(
            root.join(".astrid-edge-generation.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        root
    }

    fn unit_source(unit: &str) -> &'static str {
        match unit {
            "ollama-cpu.service" => include_str!("../../../packaging/systemd/ollama-cpu.service"),
            "astrid-model-warmup.service" => {
                include_str!("../../../packaging/systemd/astrid-model-warmup.service")
            },
            "astrid.service" => include_str!("../../../packaging/systemd/astrid.service"),
            "astrid-edge-runtime.service" => {
                include_str!("../../../packaging/systemd/astrid-edge-runtime.service")
            },
            "astrid-edge-hindsight.service" => {
                include_str!("../../../packaging/systemd/astrid-edge-hindsight.service")
            },
            "astrid-edge-hindsight.timer" => {
                include_str!("../../../packaging/systemd/astrid-edge-hindsight.timer")
            },
            _ => panic!("unexpected unit fixture"),
        }
    }

    fn install_unit_fixture(
        config: &Config,
        active_generation: &Path,
        fail_next_start: bool,
    ) -> FailingStartRunner {
        fs::create_dir(&config.roots.system_unit_root).unwrap();
        fs::create_dir(&config.roots.unit_transactions).unwrap();
        fs::set_permissions(
            &config.roots.unit_transactions,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let profile_transactions = crate::profile_projection::transaction_root_path(config);
        fs::create_dir(&profile_transactions).unwrap();
        fs::set_permissions(&profile_transactions, fs::Permissions::from_mode(0o700)).unwrap();
        let active_profile =
            crate::profile_projection::projection_bytes_for_generation(config, active_generation)
                .unwrap();
        fs::write(
            crate::profile_projection::active_profile_path(config),
            active_profile,
        )
        .unwrap();
        fs::set_permissions(
            crate::profile_projection::active_profile_path(config),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        let mut policy_dropins = Vec::new();
        let mut effective_dropins = BTreeMap::new();
        for unit in MUTABLE_UNIT_FRAGMENTS {
            let logical = format!("packaging/systemd/{unit}");
            let text = fs::read_to_string(active_generation.join(&logical)).unwrap();
            let installed =
                normalized_system_unit(&logical, &text, &config.roots.active_link).unwrap();
            fs::write(config.roots.system_unit_root.join(unit), installed).unwrap();

            let directory = config.roots.system_unit_root.join(format!("{unit}.d"));
            fs::create_dir(&directory).unwrap();
            let boundary = directory.join("90-root-runtime-boundary.conf");
            fs::write(&boundary, b"[Service]\nNoNewPrivileges=yes\n").unwrap();
            let bytes = fs::read(&boundary).unwrap();
            let relative = format!("{unit}.d/90-root-runtime-boundary.conf");
            policy_dropins.push(PolicyDropin {
                path: relative.clone(),
                size: u64::try_from(bytes.len()).unwrap(),
                sha256: sha256(&bytes),
            });
            effective_dropins
                .entry((*unit).to_owned())
                .or_insert_with(Vec::new)
                .push(
                    config
                        .roots
                        .system_unit_root
                        .join(relative)
                        .display()
                        .to_string(),
                );
            if *unit == "astrid-edge-runtime.service" {
                let root_boundary = directory.join("60-self-evolution-root.conf");
                fs::write(&root_boundary, b"[Service]\nProtectSystem=strict\n").unwrap();
                let bytes = fs::read(&root_boundary).unwrap();
                let relative = format!("{unit}.d/60-self-evolution-root.conf");
                policy_dropins.push(PolicyDropin {
                    path: relative.clone(),
                    size: u64::try_from(bytes.len()).unwrap(),
                    sha256: sha256(&bytes),
                });
                effective_dropins
                    .entry((*unit).to_owned())
                    .or_insert_with(Vec::new)
                    .push(
                        config
                            .roots
                            .system_unit_root
                            .join(relative)
                            .display()
                            .to_string(),
                    );
            }
        }
        policy_dropins.sort();
        let policy = UnitPolicy {
            schema: "astrid.edge_rescue_helper.unit_policy.v1".to_owned(),
            authority: "operator_bootstrap_reviewed_immutable_dropins".to_owned(),
            system_unit_root: config.roots.system_unit_root.display().to_string(),
            mutable_fragments: MUTABLE_UNIT_FRAGMENTS
                .iter()
                .map(|unit| (*unit).to_owned())
                .collect(),
            immutable_dropins: policy_dropins,
        };
        fs::write(&config.roots.unit_policy, canonical_json(&policy).unwrap()).unwrap();
        FailingStartRunner {
            failed: false,
            fail_on_label: fail_next_start.then_some("systemd-start-edge"),
            system_root: config.roots.system_unit_root.clone(),
            workspace: config.roots.workspace.clone(),
            hindsight_state: config.health.hindsight_state.clone(),
            hindsight_sequence: 0,
            dropins: effective_dropins,
            labels: Vec::new(),
            commands: Vec::new(),
            state_restore_transaction_ids: Vec::new(),
        }
    }

    fn write_hindsight_checkpoint_fixture(
        workspace: &Path,
        hindsight_state: &Path,
        output: &Path,
        generation_id: &str,
        reason: &str,
        sequence: usize,
    ) {
        use std::os::unix::fs::MetadataExt as _;

        let ledger_path = workspace.join("actions/receipts.jsonl");
        fs::create_dir_all(ledger_path.parent().unwrap()).unwrap();
        if !ledger_path.exists() {
            fs::write(&ledger_path, b"fixture\n").unwrap();
        }
        let bytes = fs::read(&ledger_path).unwrap();
        let metadata = fs::metadata(&ledger_path).unwrap();
        let record_sha256 = format!("{sequence:064x}");
        let mut attestation = serde_json::json!({
            "schema": "astrid.edge_checkpoint.hindsight_attestation.v1",
            "checked_at_unix_ms": 1,
            "generation_id": generation_id,
            "host_boot_id": "00000000-0000-0000-0000-000000000000",
            "checkpoint_recorded_at_unix_ms": 1,
            "checkpoint_age_seconds": 0,
            "continuity_epoch": "fixture-epoch",
            "checkpoint_record_sha256": record_sha256,
            "checkpoint_chain_records": sequence,
            "ledger_prefixes_verified": 1,
            "ledger_prefix_bytes_verified": bytes.len(),
            "operator_database_quick_check": "ok",
            "operator_database_schema_version": 1,
            "operator_database_sha256": "a".repeat(64),
            "authority": "immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim",
            "evidence_sha256": "",
        });
        let mut digest_input = attestation.clone();
        digest_input
            .as_object_mut()
            .unwrap()
            .remove("evidence_sha256");
        attestation["evidence_sha256"] =
            serde_json::Value::String(sha256(&canonical_json(&digest_input).unwrap()));

        let checkpoint = serde_json::json!({
            "schema": "astrid.edge_checkpoint.root_record.v1",
            "reason": reason,
            "attestation": attestation,
            "authority": "immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim",
        });
        fs::write(output, canonical_json(&checkpoint).unwrap()).unwrap();
        fs::set_permissions(output, fs::Permissions::from_mode(0o400)).unwrap();

        let latest = serde_json::json!({
            "checkpoint_record_sha256": record_sha256,
            "continuity_epoch": "fixture-epoch",
            "ledgers": {
                "actions/receipts.jsonl": {
                    "present": true,
                    "hash_scope": "exact_open_file_prefix_v1",
                    "inode": metadata.ino(),
                    "size_bytes": bytes.len(),
                    "sha256": sha256(&bytes),
                }
            }
        });
        fs::write(hindsight_state, canonical_json(&latest).unwrap()).unwrap();
        let chain = hindsight_state.parent().unwrap().join("checkpoints.jsonl");
        let line = canonical_json(&serde_json::json!({
            "record_sha256": record_sha256,
        }))
        .unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(chain)
            .unwrap();
        file.write_all(&line).unwrap();
        file.write_all(b"\n").unwrap();
    }

    fn write_snapshot_fixture(path: &Path, generation_id: &str, quiescence_record: &str) {
        fs::create_dir(path).unwrap();
        let mut manifest = serde_json::json!({
            "schema": "astrid.edge_checkpoint.rollback_state.v2",
            "generation_id": generation_id,
            "quiescence_policy": "exact_signed_runtime_stopped_transition_record",
            "quiescence_record_sha256": quiescence_record,
            "retention_policy": "paired_with_rollback_generation_no_independent_gc",
            "minimum_prior_generations": 3,
            "minimum_retention_seconds": 7 * 24 * 60 * 60,
            "authority": "immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim",
        });
        let digest = sha256(&canonical_json(&manifest).unwrap());
        manifest
            .as_object_mut()
            .unwrap()
            .insert("manifest_sha256".into(), serde_json::Value::String(digest));
        fs::write(
            path.join("manifest.json"),
            canonical_json(&manifest).unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            path.join("manifest.json"),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o500)).unwrap();
    }

    fn snapshot_binding_fixture(
        config: &Config,
        basename: &str,
        generation_id: &str,
        quiescence_record: &str,
    ) -> StateSnapshotBinding {
        let path = config.roots.state_snapshots.join(basename);
        write_snapshot_fixture(&path, generation_id, quiescence_record);
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
        StateSnapshotBinding {
            basename: basename.to_owned(),
            generation_id: generation_id.to_owned(),
            manifest_sha256: value["manifest_sha256"].as_str().unwrap().to_owned(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exact root-helper configuration fixture intentionally enumerates every authority-bearing field"
    )]
    fn fixture_config(root: &Path) -> Config {
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        let ledger_key = root.join("ledger-key");
        if !ledger_key.exists() {
            fs::write(&ledger_key, [0x5a_u8; 32]).unwrap();
            fs::set_permissions(&ledger_key, fs::Permissions::from_mode(0o400)).unwrap();
        }
        let executable = TrustedExecutable {
            path: root.join("native"),
            sha256: "a".repeat(64),
        };
        Config {
            schema: "astrid.edge_rescue_helper.config.v1".into(),
            appliance_id: "test".into(),
            target: "x86_64-unknown-linux-gnu".into(),
            model: "qwen3:1.7b".into(),
            ollama_origin: "http://127.0.0.1:11434".into(),
            source: SourceConfig {
                root: root.join("source"),
                manifest: root.join("source/manifest"),
                signature: root.join("source/signature"),
                signing_key: root.join("key"),
                intent_attestation_key: root.join("intent-key"),
                ledger_attestation_key: ledger_key,
                vendor: root.join("source/vendor"),
            },
            roots: RootConfig {
                supervisor_state: root.to_path_buf(),
                candidate_store: root.join("candidates"),
                model_handoff_root: root.join("model-handoff"),
                model_handoff_ledger: root.join("model-unload-receipts.jsonl"),
                candidate_work: root.join("work"),
                build_store: root.join("builds"),
                releases: root.join("releases"),
                active_link: root.join("current"),
                generation_binding: root.join("current-generation"),
                maintenance_lease: root.join("maintenance.json"),
                maintenance_mutex: root.join("maintenance.lock"),
                state_snapshots: root.join("snapshots"),
                workspace: root.join("workspace"),
                system_unit_root: root.join("system-units"),
                unit_policy: root.join("unit-policy.json"),
                unit_transactions: root.join("snapshots/unit-transactions"),
                candidate_sandbox_root: root.join("candidate-rootfs"),
            },
            identities: IdentityConfig {
                steward_uid: 10,
                steward_gid: 10,
                builder_uid: 11,
                builder_gid: 11,
                updater_uid: 12,
                updater_gid: 12,
                runtime_uid: 13,
                runtime_gid: 13,
            },
            executables: Executables {
                cargo: executable.clone(),
                rustc: executable.clone(),
                rustfmt: executable.clone(),
                python: executable.clone(),
                systemctl: executable.clone(),
                systemd_run: executable.clone(),
                systemd_analyze: executable.clone(),
                checkpoint: executable.clone(),
                capsule_builder: executable.clone(),
                invariant_runner: executable.clone(),
                package_verifier: executable.clone(),
                state_store: executable,
            },
            services: ServiceConfig {
                core: "astrid.service".into(),
                warmup: "astrid-model-warmup.service".into(),
                edge: "astrid-edge-runtime.service".into(),
            },
            storage: StorageConfig {
                config: root.join("edge-state-store.json"),
                config_sha256: "b".repeat(64),
                install_attestation: root.join("storage-install-attestation.json"),
                health_attestation: root.join("storage-health-attestation.json"),
                runtime_state_mount: root.join("runtime-state"),
                rollback_mount: root.join("snapshots"),
                backing_uuid: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
                runtime_filesystem_uuid: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into(),
                rollback_filesystem_uuid: "cccccccc-cccc-4ccc-8ccc-cccccccccccc".into(),
                image_bytes: 32 * 1024 * 1024 * 1024,
                host_reserve_bytes: 64 * 1024 * 1024 * 1024,
                store_minimum_free_bytes: 4 * 1024 * 1024 * 1024,
                emergency_inode_reserve_files: 65_536,
            },
            drain: DrainConfig {
                autonomy_state: root.join("workspace/autonomy.json"),
                model_lock: root.join("model.lock"),
                model_lock_gid: 14,
                maintenance_edge_acknowledgement: root.join("workspace/edge-ack.json"),
                maintenance_core_acknowledgement: root.join("core-ack.json"),
                activity_ledgers: vec![root.join("workspace/actions.jsonl")],
                maximum_wait_seconds: 30,
                poll_milliseconds: 100,
            },
            policy: Policy {
                maximum_files: 25,
                maximum_changed_lines: 4_000,
                build_workers: 2,
                command_timeout_seconds: 60,
                pipeline_timeout_seconds: 600,
                maximum_candidate_bytes: 1024 * 1024,
                minimum_free_disk_bytes: 1024 * 1024 * 1024,
                candidate_memory_max_bytes: 4 * 1024 * 1024 * 1024,
                candidate_memory_swap_max_bytes: 128 * 1024 * 1024,
                candidate_tasks_max: 256,
                candidate_cpu_quota_percent: 200,
                network_policy: "private-network-none:v1".into(),
                dependency_policy: "signed-vendor-offline-locked:v1".into(),
            },
            health: HealthConfig {
                sensor_state: root.join("sensor"),
                hindsight_state: root.join("hindsight"),
                fill_history: root.join("fill"),
                model_warmup_receipt: root.join("model-warmup-receipt"),
                model_warmup_uid: 15,
                meminfo: root.join("meminfo"),
                swaps: root.join("swaps"),
                thermal_celsius: root.join("thermal"),
                telemetry_addr: "127.0.0.1:7878".parse().unwrap(),
                audio_policy: AudioPolicy::RequiredUnavailable,
                expected_audio_source: "unavailable_no_audio_input".to_owned(),
                maximum_age_seconds: 120,
                maximum_thermal_celsius: 85.0,
                minimum_available_ram_bytes: 2 * 1024 * 1024 * 1024,
                maximum_swap_bytes: 128 * 1024 * 1024,
                minimum_fill_samples: 10,
            },
        }
    }
}
