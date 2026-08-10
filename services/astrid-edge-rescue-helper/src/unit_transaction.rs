//! Transactional installation of the exact six mutable Astrid base units.
//!
//! Candidate content never supplies a drop-in.  The operator bootstrap policy
//! pins every effective root/profile boundary drop-in and this module verifies
//! those files before, during, and after each fragment replacement.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, valid_hex64, valid_identifier};
use crate::fs_guard::{
    atomic_write, canonical_json, ensure_within, read_regular, sha256, validate_relative_signed,
};
use crate::invariant::{MUTABLE_UNIT_FRAGMENTS, normalized_system_unit};
use crate::native::{CommandReceipt, CommandSpec, NativeRunner, require_success};
use crate::{Error, Result};

const POLICY_SCHEMA: &str = "astrid.edge_rescue_helper.unit_policy.v1";
const TRANSACTION_SCHEMA: &str = "astrid.edge_rescue_helper.unit_transaction.v1";
const PENDING_SCHEMA: &str = "astrid.edge_rescue_helper.unit_transaction_pending.v1";
const JOURNAL_SCHEMA: &str = "astrid.edge_rescue_helper.unit_transaction_record.v1";
const AUTHORITY: &str = "immutable_root_transactional_units:v1";
const MAX_UNIT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitPolicy {
    pub schema: String,
    pub authority: String,
    pub system_unit_root: String,
    pub mutable_fragments: Vec<String>,
    pub immutable_dropins: Vec<PolicyDropin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct PolicyDropin {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TransactionManifest {
    schema: String,
    transaction_id: String,
    target_generation_id: String,
    prior_generation_id: String,
    policy_sha256: String,
    profile: String,
    authority: String,
    units: Vec<UnitSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct UnitSnapshot {
    unit: String,
    logical_source: String,
    source_sha256: String,
    prior_size: u64,
    prior_sha256: String,
    target_size: u64,
    target_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Pending {
    schema: String,
    transaction_id: String,
    manifest_sha256: String,
    authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalRecord {
    schema: String,
    transaction_id: String,
    phase: String,
    unit: Option<String>,
    selected_generation_id: Option<String>,
    recorded_at_unix_ms: u64,
    previous_record_sha256: Option<String>,
    authority: String,
    record_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnitTransactionEvidence {
    pub schema: &'static str,
    pub transaction_id: String,
    pub target_generation_id: String,
    pub prior_generation_id: String,
    pub authority: &'static str,
    pub status: &'static str,
    pub receipts: Vec<CommandReceipt>,
}

#[derive(Debug, Clone)]
pub struct PreparedUnitTransaction {
    pub transaction_id: String,
    pub target_generation_id: String,
    pub prior_generation_id: String,
    pub receipts: Vec<CommandReceipt>,
}

/// Prepare sealed prior/target snapshots without changing the live manager.
pub fn prepare<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    target_generation: &Path,
    prior_generation: &Path,
) -> Result<PreparedUnitTransaction> {
    require_root("prepare unit transaction")?;
    prepare_inner(config, runner, target_generation, prior_generation, true)
}

/// Install and reload the target snapshot.  Callers commit only after the
/// generation transition reaches its independently durable commit phase.
pub fn apply_target<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedUnitTransaction,
) -> Result<UnitTransactionEvidence> {
    require_root("apply unit transaction")?;
    apply_selected_inner(
        config,
        runner,
        transaction,
        &transaction.target_generation_id,
        SnapshotSide::Target,
        "target",
        true,
    )
}

/// Restore the pre-activation fragments after an uncommitted failure.
pub fn restore_prior<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedUnitTransaction,
) -> Result<UnitTransactionEvidence> {
    require_root("restore unit transaction")?;
    let evidence = apply_selected_inner(
        config,
        runner,
        transaction,
        &transaction.prior_generation_id,
        SnapshotSide::Prior,
        "prior",
        true,
    )?;
    finish_pending(config, &transaction.transaction_id, "restored", true)?;
    Ok(evidence)
}

/// Commit a target set only after the outer generation journal has committed.
pub fn commit(config: &Config, transaction: &PreparedUnitTransaction) -> Result<()> {
    require_root("commit unit transaction")?;
    commit_inner(config, transaction, true)
}

/// Reconcile a partially installed set to the generation selected by the
/// authoritative outer transition journal.  With no pending transaction this
/// remains a strict boot-time active-fragment verification.
pub fn reconcile_to_generation<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    selected_generation: &Path,
) -> Result<UnitTransactionEvidence> {
    require_root("reconcile unit transaction")?;
    reconcile_inner(config, runner, selected_generation, true)
}

pub(crate) fn prepare_for_transition<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    target_generation: &Path,
    prior_generation: &Path,
    require_root_owner: bool,
) -> Result<PreparedUnitTransaction> {
    prepare_inner(
        config,
        runner,
        target_generation,
        prior_generation,
        require_root_owner,
    )
}

pub(crate) fn apply_target_for_transition<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedUnitTransaction,
    require_root_owner: bool,
) -> Result<UnitTransactionEvidence> {
    apply_selected_inner(
        config,
        runner,
        transaction,
        &transaction.target_generation_id,
        SnapshotSide::Target,
        "target",
        require_root_owner,
    )
}

pub(crate) fn restore_prior_for_transition<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedUnitTransaction,
    require_root_owner: bool,
) -> Result<UnitTransactionEvidence> {
    let evidence = apply_selected_inner(
        config,
        runner,
        transaction,
        &transaction.prior_generation_id,
        SnapshotSide::Prior,
        "prior",
        require_root_owner,
    )?;
    finish_pending(
        config,
        &transaction.transaction_id,
        "restored",
        require_root_owner,
    )?;
    Ok(evidence)
}

pub(crate) fn commit_for_transition(
    config: &Config,
    transaction: &PreparedUnitTransaction,
    require_root_owner: bool,
) -> Result<()> {
    commit_inner(config, transaction, require_root_owner)
}

pub(crate) fn reconcile_for_transition<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    selected_generation: &Path,
    require_root_owner: bool,
) -> Result<UnitTransactionEvidence> {
    reconcile_inner(config, runner, selected_generation, require_root_owner)
}

#[allow(clippy::too_many_lines)] // Preparation seals one all-or-nothing six-fragment snapshot.
fn prepare_inner<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    target_generation: &Path,
    prior_generation: &Path,
    require_root_owner: bool,
) -> Result<PreparedUnitTransaction> {
    require_roots(config, require_root_owner)?;
    if pending_path(config).exists() || pending_path(config).is_symlink() {
        return Err(Error::new("another unit transaction remains pending"));
    }
    let target_id = generation_id(config, target_generation)?;
    let prior_id = generation_id(config, prior_generation)?;
    if target_id == prior_id {
        return Err(Error::new("unit transaction generations must be distinct"));
    }
    let (policy, policy_bytes) = load_policy(config, require_root_owner)?;
    verify_dropins(config, &policy, require_root_owner)?;
    let profile = if config.appliance_id.starts_with("icp") {
        "icp"
    } else {
        "avado"
    };
    let mut prepared = Vec::with_capacity(MUTABLE_UNIT_FRAGMENTS.len());
    for unit in MUTABLE_UNIT_FRAGMENTS {
        let logical = logical_source(profile, unit);
        let prior_source = read_unit_source(config, prior_generation, &logical)?;
        let target_source = read_unit_source(config, target_generation, &logical)?;
        let live = read_live_fragment(config, unit, require_root_owner)?;
        if live != prior_source {
            return Err(Error::new(format!(
                "live unit differs from the selected prior generation: {unit}"
            )));
        }
        prepared.push(((*unit).to_owned(), logical, live, target_source));
    }
    let transaction_id = transaction_id(&target_id, &prior_id, &policy_bytes)?;
    let partial = config
        .roots
        .unit_transactions
        .join(format!(".{transaction_id}.partial"));
    let transaction_root = config.roots.unit_transactions.join(&transaction_id);
    ensure_within(&config.roots.unit_transactions, &partial, false)?;
    ensure_within(&config.roots.unit_transactions, &transaction_root, false)?;
    if partial.exists()
        || partial.is_symlink()
        || transaction_root.exists()
        || transaction_root.is_symlink()
    {
        return Err(Error::new("unit transaction identifier collision"));
    }
    fs::create_dir(&partial)?;
    fs::set_permissions(&partial, fs::Permissions::from_mode(0o700))?;
    fs::create_dir(partial.join("prior"))?;
    fs::create_dir(partial.join("target"))?;
    let result = (|| {
        let mut units = Vec::with_capacity(prepared.len());
        for (unit, logical, prior, target) in &prepared {
            atomic_write(&partial.join("prior").join(unit), prior, 0o400, false)?;
            atomic_write(&partial.join("target").join(unit), target, 0o400, false)?;
            units.push(UnitSnapshot {
                unit: unit.clone(),
                logical_source: logical.clone(),
                source_sha256: sha256(target),
                prior_size: u64::try_from(prior.len()).unwrap_or(u64::MAX),
                prior_sha256: sha256(prior),
                target_size: u64::try_from(target.len()).unwrap_or(u64::MAX),
                target_sha256: sha256(target),
            });
        }
        fs::set_permissions(partial.join("prior"), fs::Permissions::from_mode(0o500))?;
        fs::set_permissions(partial.join("target"), fs::Permissions::from_mode(0o500))?;
        let manifest = TransactionManifest {
            schema: TRANSACTION_SCHEMA.to_owned(),
            transaction_id: transaction_id.clone(),
            target_generation_id: target_id.clone(),
            prior_generation_id: prior_id.clone(),
            policy_sha256: sha256(&policy_bytes),
            profile: profile.to_owned(),
            authority: AUTHORITY.to_owned(),
            units,
        };
        let manifest_bytes = canonical_json(&manifest)?;
        atomic_write(
            &partial.join("manifest.json"),
            &manifest_bytes,
            0o400,
            false,
        )?;
        let receipt = verify_staged_units(config, runner, &partial)?;
        fs::rename(&partial, &transaction_root)?;
        File::open(&config.roots.unit_transactions)?.sync_all()?;
        append_journal(
            &transaction_root,
            &transaction_id,
            "prepared",
            None,
            None,
            require_root_owner,
        )?;
        let pending = Pending {
            schema: PENDING_SCHEMA.to_owned(),
            transaction_id: transaction_id.clone(),
            manifest_sha256: sha256(&manifest_bytes),
            authority: AUTHORITY.to_owned(),
        };
        atomic_write(
            &pending_path(config),
            &canonical_json(&pending)?,
            0o400,
            false,
        )?;
        Ok(PreparedUnitTransaction {
            transaction_id,
            target_generation_id: target_id,
            prior_generation_id: prior_id,
            receipts: vec![receipt],
        })
    })();
    if result.is_err() && partial.exists() {
        let _ = fs::remove_dir_all(&partial);
    }
    result
}

#[derive(Clone, Copy)]
enum SnapshotSide {
    Prior,
    Target,
}

fn apply_selected_inner<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    transaction: &PreparedUnitTransaction,
    selected_generation_id: &str,
    side: SnapshotSide,
    phase_prefix: &str,
    require_root_owner: bool,
) -> Result<UnitTransactionEvidence> {
    let (root, manifest, policy) =
        load_pending_transaction(config, transaction, require_root_owner)?;
    let expected = match side {
        SnapshotSide::Prior => &manifest.prior_generation_id,
        SnapshotSide::Target => &manifest.target_generation_id,
    };
    if selected_generation_id != expected {
        return Err(Error::new(
            "unit transaction selected generation is inconsistent",
        ));
    }
    verify_dropins(config, &policy, require_root_owner)?;
    append_journal(
        &root,
        &transaction.transaction_id,
        &format!("apply_{phase_prefix}_started"),
        None,
        Some(selected_generation_id),
        require_root_owner,
    )?;
    let mut receipts = Vec::new();
    for snapshot in &manifest.units {
        verify_dropins(config, &policy, require_root_owner)?;
        let bytes = snapshot_bytes(&root, snapshot, side, require_root_owner)?;
        replace_fragment(config, &snapshot.unit, &bytes, require_root_owner)?;
        append_journal(
            &root,
            &transaction.transaction_id,
            &format!("{phase_prefix}_fragment_installed"),
            Some(&snapshot.unit),
            Some(selected_generation_id),
            require_root_owner,
        )?;
    }
    let receipt = daemon_reload(config, runner)?;
    receipts.push(receipt);
    append_journal(
        &root,
        &transaction.transaction_id,
        &format!("{phase_prefix}_daemon_reloaded"),
        None,
        Some(selected_generation_id),
        require_root_owner,
    )?;
    receipts.extend(verify_installed_set(
        config,
        runner,
        &manifest,
        side,
        &root,
        &policy,
        require_root_owner,
    )?);
    append_journal(
        &root,
        &transaction.transaction_id,
        &format!("{phase_prefix}_verified"),
        None,
        Some(selected_generation_id),
        require_root_owner,
    )?;
    Ok(UnitTransactionEvidence {
        schema: "astrid.edge_rescue_helper.unit_transaction_evidence.v1",
        transaction_id: transaction.transaction_id.clone(),
        target_generation_id: transaction.target_generation_id.clone(),
        prior_generation_id: transaction.prior_generation_id.clone(),
        authority: AUTHORITY,
        status: if matches!(side, SnapshotSide::Target) {
            "target_installed_pending_outer_commit"
        } else {
            "prior_restored"
        },
        receipts,
    })
}

fn commit_inner(
    config: &Config,
    transaction: &PreparedUnitTransaction,
    require_root_owner: bool,
) -> Result<()> {
    let (root, manifest, policy) =
        load_pending_transaction(config, transaction, require_root_owner)?;
    verify_snapshot_set(
        config,
        &manifest,
        SnapshotSide::Target,
        &root,
        &policy,
        require_root_owner,
    )?;
    append_journal(
        &root,
        &transaction.transaction_id,
        "committed",
        None,
        Some(&transaction.target_generation_id),
        require_root_owner,
    )?;
    clear_pending(config)?;
    Ok(())
}

fn reconcile_inner<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    selected_generation: &Path,
    require_root_owner: bool,
) -> Result<UnitTransactionEvidence> {
    let selected_id = generation_id(config, selected_generation)?;
    if !pending_path(config).exists() && !pending_path(config).is_symlink() {
        let (policy, _) = load_policy(config, require_root_owner)?;
        let manifest = transient_generation_manifest(config, selected_generation, &selected_id)?;
        let root = selected_generation.to_path_buf();
        let receipts = verify_installed_set(
            config,
            runner,
            &manifest,
            SnapshotSide::Target,
            &root,
            &policy,
            require_root_owner,
        )?;
        return Ok(UnitTransactionEvidence {
            schema: "astrid.edge_rescue_helper.unit_transaction_evidence.v1",
            transaction_id: "none".to_owned(),
            target_generation_id: selected_id,
            prior_generation_id: "none".to_owned(),
            authority: AUTHORITY,
            status: "active_fragments_verified_no_pending_transaction",
            receipts,
        });
    }
    let pending = read_pending(config, require_root_owner)?;
    let transaction = PreparedUnitTransaction {
        transaction_id: pending.transaction_id,
        target_generation_id: String::new(),
        prior_generation_id: String::new(),
        receipts: Vec::new(),
    };
    let (_, manifest, _) =
        load_pending_transaction_loose(config, &transaction, require_root_owner)?;
    let transaction = PreparedUnitTransaction {
        transaction_id: transaction.transaction_id,
        target_generation_id: manifest.target_generation_id.clone(),
        prior_generation_id: manifest.prior_generation_id.clone(),
        receipts: Vec::new(),
    };
    let side = if selected_id == manifest.target_generation_id {
        SnapshotSide::Target
    } else if selected_id == manifest.prior_generation_id {
        SnapshotSide::Prior
    } else {
        return Err(Error::new(
            "outer transition selected a generation absent from pending unit transaction",
        ));
    };
    let mut evidence = apply_selected_inner(
        config,
        runner,
        &transaction,
        &selected_id,
        side,
        "boot_reconcile",
        require_root_owner,
    )?;
    finish_pending(
        config,
        &transaction.transaction_id,
        "boot_reconciled",
        require_root_owner,
    )?;
    evidence.status = "boot_reconciled_to_outer_transition_journal";
    Ok(evidence)
}

fn transient_generation_manifest(
    config: &Config,
    generation: &Path,
    generation_id: &str,
) -> Result<TransactionManifest> {
    let profile = if config.appliance_id.starts_with("icp") {
        "icp"
    } else {
        "avado"
    };
    let mut units = Vec::new();
    for unit in MUTABLE_UNIT_FRAGMENTS {
        let logical = logical_source(profile, unit);
        let bytes = read_unit_source(config, generation, &logical)?;
        units.push(UnitSnapshot {
            unit: (*unit).to_owned(),
            logical_source: logical,
            source_sha256: sha256(&bytes),
            prior_size: 0,
            prior_sha256: String::new(),
            target_size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            target_sha256: sha256(&bytes),
        });
    }
    Ok(TransactionManifest {
        schema: TRANSACTION_SCHEMA.to_owned(),
        transaction_id: "none".to_owned(),
        target_generation_id: generation_id.to_owned(),
        prior_generation_id: "none".to_owned(),
        policy_sha256: String::new(),
        profile: profile.to_owned(),
        authority: AUTHORITY.to_owned(),
        units,
    })
}

fn verify_installed_set<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    manifest: &TransactionManifest,
    side: SnapshotSide,
    snapshot_root: &Path,
    policy: &UnitPolicy,
    require_root_owner: bool,
) -> Result<Vec<CommandReceipt>> {
    verify_snapshot_set(
        config,
        manifest,
        side,
        snapshot_root,
        policy,
        require_root_owner,
    )?;
    verify_effective_manager(config, runner, policy)
}

fn verify_snapshot_set(
    config: &Config,
    manifest: &TransactionManifest,
    side: SnapshotSide,
    snapshot_root: &Path,
    policy: &UnitPolicy,
    require_root_owner: bool,
) -> Result<()> {
    verify_dropins(config, policy, require_root_owner)?;
    for snapshot in &manifest.units {
        let expected = match side {
            SnapshotSide::Prior => {
                if snapshot.prior_sha256.is_empty() {
                    return Err(Error::new("transient manifest has no prior snapshot"));
                }
                snapshot_bytes(snapshot_root, snapshot, side, require_root_owner)?
            },
            SnapshotSide::Target if snapshot_root.starts_with(&config.roots.releases) => {
                read_unit_source(config, snapshot_root, &snapshot.logical_source)?
            },
            SnapshotSide::Target => {
                snapshot_bytes(snapshot_root, snapshot, side, require_root_owner)?
            },
        };
        if read_live_fragment(config, &snapshot.unit, require_root_owner)? != expected {
            return Err(Error::new(format!(
                "installed unit set does not match sealed snapshot: {}",
                snapshot.unit
            )));
        }
    }
    Ok(())
}

fn load_pending_transaction(
    config: &Config,
    expected: &PreparedUnitTransaction,
    require_root_owner: bool,
) -> Result<(PathBuf, TransactionManifest, UnitPolicy)> {
    let result = load_pending_transaction_loose(config, expected, require_root_owner)?;
    if result.1.target_generation_id != expected.target_generation_id
        || result.1.prior_generation_id != expected.prior_generation_id
    {
        return Err(Error::new("pending unit transaction identity changed"));
    }
    Ok(result)
}

fn load_pending_transaction_loose(
    config: &Config,
    expected: &PreparedUnitTransaction,
    require_root_owner: bool,
) -> Result<(PathBuf, TransactionManifest, UnitPolicy)> {
    let pending = read_pending(config, require_root_owner)?;
    if pending.transaction_id != expected.transaction_id {
        return Err(Error::new("pending unit transaction pointer differs"));
    }
    let root = config.roots.unit_transactions.join(&pending.transaction_id);
    ensure_within(&config.roots.unit_transactions, &root, true)?;
    require_private_directory(&root, require_root_owner)?;
    let manifest_bytes =
        read_owned_regular(&root.join("manifest.json"), 256 * 1024, require_root_owner)?;
    if sha256(&manifest_bytes) != pending.manifest_sha256 {
        return Err(Error::new("pending unit manifest digest failed"));
    }
    let manifest: TransactionManifest = serde_json::from_slice(&manifest_bytes)?;
    validate_manifest(&manifest, &pending.transaction_id)?;
    let (policy, policy_bytes) = load_policy(config, require_root_owner)?;
    if manifest.policy_sha256 != sha256(&policy_bytes) {
        return Err(Error::new("unit policy changed across transaction"));
    }
    let _ = replay_journal(&root, &pending.transaction_id, require_root_owner)?;
    Ok((root, manifest, policy))
}

fn validate_manifest(manifest: &TransactionManifest, transaction_id: &str) -> Result<()> {
    let expected = MUTABLE_UNIT_FRAGMENTS
        .iter()
        .map(|item| (*item).to_owned())
        .collect::<Vec<_>>();
    if manifest.schema != TRANSACTION_SCHEMA
        || manifest.transaction_id != transaction_id
        || !valid_identifier(&manifest.target_generation_id)
        || !valid_identifier(&manifest.prior_generation_id)
        || manifest.target_generation_id == manifest.prior_generation_id
        || !valid_hex64(&manifest.policy_sha256)
        || !matches!(manifest.profile.as_str(), "avado" | "icp")
        || manifest.authority != AUTHORITY
        || manifest
            .units
            .iter()
            .map(|item| item.unit.clone())
            .collect::<Vec<_>>()
            != expected
        || manifest.units.iter().any(|item| {
            !valid_hex64(&item.source_sha256)
                || !valid_hex64(&item.prior_sha256)
                || !valid_hex64(&item.target_sha256)
                || item.prior_size > MAX_UNIT_BYTES
                || item.target_size > MAX_UNIT_BYTES
                || logical_source(&manifest.profile, &item.unit) != item.logical_source
        })
    {
        return Err(Error::new("unit transaction manifest is invalid"));
    }
    Ok(())
}

fn load_policy(config: &Config, require_root_owner: bool) -> Result<(UnitPolicy, Vec<u8>)> {
    let bytes = read_owned_regular(&config.roots.unit_policy, 256 * 1024, require_root_owner)?;
    let policy: UnitPolicy = serde_json::from_slice(&bytes)?;
    if canonical_json(&policy)? != bytes {
        return Err(Error::new("unit policy is not canonical JSON"));
    }
    validate_policy(config, &policy)?;
    Ok((policy, bytes))
}

fn validate_policy(config: &Config, policy: &UnitPolicy) -> Result<()> {
    let expected = MUTABLE_UNIT_FRAGMENTS
        .iter()
        .map(|item| (*item).to_owned())
        .collect::<Vec<_>>();
    if policy.schema != POLICY_SCHEMA
        || policy.authority != "operator_bootstrap_reviewed_immutable_dropins"
        || policy.system_unit_root != config.roots.system_unit_root.to_string_lossy()
        || policy.mutable_fragments != expected
        || policy.immutable_dropins.len() < MUTABLE_UNIT_FRAGMENTS.len()
        || policy.immutable_dropins.len() > 32
    {
        return Err(Error::new("unit policy identity or authority failed"));
    }
    let mut paths = BTreeSet::new();
    for dropin in &policy.immutable_dropins {
        let relative = validate_relative_signed(&dropin.path)?;
        let text = relative.to_string_lossy();
        let Some((unit, name)) = text.split_once(".d/") else {
            return Err(Error::new("unit policy drop-in path is malformed"));
        };
        if !MUTABLE_UNIT_FRAGMENTS.contains(&unit)
            || !Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("conf"))
            || name.contains('/')
            || !valid_hex64(&dropin.sha256)
            || dropin.size == 0
            || dropin.size > MAX_UNIT_BYTES
            || !paths.insert(dropin.path.clone())
        {
            return Err(Error::new("unit policy drop-in is outside exact bounds"));
        }
    }
    for unit in MUTABLE_UNIT_FRAGMENTS {
        if !paths.contains(&format!("{unit}.d/90-root-runtime-boundary.conf")) {
            return Err(Error::new("unit policy omits a root runtime boundary"));
        }
    }
    if !paths.contains("astrid-edge-runtime.service.d/60-self-evolution-root.conf") {
        return Err(Error::new(
            "unit policy omits the immutable self-evolution runtime boundary",
        ));
    }
    Ok(())
}

fn verify_dropins(config: &Config, policy: &UnitPolicy, require_root_owner: bool) -> Result<()> {
    let expected = policy
        .immutable_dropins
        .iter()
        .map(|item| item.path.clone())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for unit in MUTABLE_UNIT_FRAGMENTS {
        let directory = config.roots.system_unit_root.join(format!("{unit}.d"));
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || !owned_by_expected(&metadata, require_root_owner)
            || metadata.mode() & 0o022 != 0
        {
            return Err(Error::new("unit drop-in directory identity failed"));
        }
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            let relative = path
                .strip_prefix(&config.roots.system_unit_root)
                .map_err(|_| Error::new("unit drop-in path escaped root"))?
                .to_string_lossy()
                .to_string();
            if path.extension().is_none_or(|value| value != "conf") || !actual.insert(relative) {
                return Err(Error::new("unit drop-in membership is not exact"));
            }
        }
    }
    if actual != expected {
        return Err(Error::new(
            "effective unit drop-in membership differs from operator policy",
        ));
    }
    for dropin in &policy.immutable_dropins {
        let bytes = read_owned_regular(
            &config.roots.system_unit_root.join(&dropin.path),
            MAX_UNIT_BYTES,
            require_root_owner,
        )?;
        if bytes.len() as u64 != dropin.size || sha256(&bytes) != dropin.sha256 {
            return Err(Error::new("immutable unit drop-in digest failed"));
        }
    }
    Ok(())
}

fn verify_effective_manager<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    policy: &UnitPolicy,
) -> Result<Vec<CommandReceipt>> {
    let expected_dropins = policy.immutable_dropins.iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut map, item| {
            let unit = item.path.split_once(".d/").map_or("", |value| value.0);
            map.entry(unit.to_owned()).or_default().push(
                config
                    .roots
                    .system_unit_root
                    .join(&item.path)
                    .display()
                    .to_string(),
            );
            map
        },
    );
    let mut receipts = Vec::new();
    for unit in MUTABLE_UNIT_FRAGMENTS {
        let (receipt, fragment) = systemctl_capture(
            config,
            runner,
            &["show", unit, "--property=FragmentPath", "--value"],
        )?;
        receipts.push(receipt);
        if exact_output_line(&fragment)?
            != config
                .roots
                .system_unit_root
                .join(unit)
                .display()
                .to_string()
        {
            return Err(Error::new("effective systemd fragment path is unreviewed"));
        }
        let (receipt, dropins) = systemctl_capture(
            config,
            runner,
            &["show", unit, "--property=DropInPaths", "--value"],
        )?;
        receipts.push(receipt);
        let mut actual = exact_output_line(&dropins)?
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut expected = expected_dropins.get(*unit).cloned().unwrap_or_default();
        actual.sort();
        expected.sort();
        if actual != expected {
            return Err(Error::new("effective systemd drop-in paths are unreviewed"));
        }
    }
    Ok(receipts)
}

fn verify_staged_units<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    root: &Path,
) -> Result<CommandReceipt> {
    let mut arguments = vec!["verify".to_owned()];
    arguments.extend(
        MUTABLE_UNIT_FRAGMENTS
            .iter()
            .map(|unit| root.join("target").join(unit).display().to_string()),
    );
    run(
        config,
        runner,
        &config.executables.systemd_analyze,
        "systemd-verify-unit-transaction",
        arguments,
        root,
    )
}

fn daemon_reload<R: NativeRunner>(config: &Config, runner: &mut R) -> Result<CommandReceipt> {
    run(
        config,
        runner,
        &config.executables.systemctl,
        "systemd-daemon-reload-unit-transaction",
        vec!["daemon-reload".to_owned()],
        &config.roots.unit_transactions,
    )
}

fn systemctl_capture<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    arguments: &[&str],
) -> Result<(CommandReceipt, Vec<u8>)> {
    let spec = command_spec(
        config,
        &config.executables.systemctl,
        "systemd-show-unit-transaction",
        arguments.iter().map(|item| (*item).to_owned()).collect(),
        &config.roots.unit_transactions,
    );
    let (receipt, output) = runner.run_capture(&spec, 64 * 1024)?;
    require_success(&receipt)?;
    Ok((receipt, output))
}

fn run<R: NativeRunner>(
    config: &Config,
    runner: &mut R,
    executable: &crate::config::TrustedExecutable,
    label: &'static str,
    arguments: Vec<String>,
    current_dir: &Path,
) -> Result<CommandReceipt> {
    let receipt = runner.run(&command_spec(
        config,
        executable,
        label,
        arguments,
        current_dir,
    ))?;
    require_success(&receipt)?;
    Ok(receipt)
}

fn command_spec(
    config: &Config,
    executable: &crate::config::TrustedExecutable,
    label: &'static str,
    arguments: Vec<String>,
    current_dir: &Path,
) -> CommandSpec {
    CommandSpec {
        label,
        executable: executable.clone(),
        arguments,
        current_dir: current_dir.to_path_buf(),
        environment: BTreeMap::from([
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
        ]),
        timeout: Duration::from_secs(config.policy.command_timeout_seconds.min(300)),
        run_as_uid: None,
        run_as_gid: None,
    }
}

fn read_unit_source(config: &Config, generation: &Path, logical: &str) -> Result<Vec<u8>> {
    let bytes = read_regular(&generation.join(logical), MAX_UNIT_BYTES)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::new("unit source is not UTF-8"))?;
    normalized_system_unit(logical, text, &config.roots.active_link)
}

fn read_live_fragment(config: &Config, unit: &str, require_root_owner: bool) -> Result<Vec<u8>> {
    read_owned_regular(
        &config.roots.system_unit_root.join(unit),
        MAX_UNIT_BYTES,
        require_root_owner,
    )
}

fn replace_fragment(
    config: &Config,
    unit: &str,
    bytes: &[u8],
    require_root_owner: bool,
) -> Result<()> {
    if !MUTABLE_UNIT_FRAGMENTS.contains(&unit) {
        return Err(Error::new("unit replacement escaped exact fragment set"));
    }
    let path = config.roots.system_unit_root.join(unit);
    let _ = read_owned_regular(&path, MAX_UNIT_BYTES, require_root_owner)?;
    atomic_write(&path, bytes, 0o644, true)?;
    let installed = read_owned_regular(&path, MAX_UNIT_BYTES, require_root_owner)?;
    if installed != bytes {
        return Err(Error::new(
            "atomic unit replacement did not persist exact bytes",
        ));
    }
    Ok(())
}

fn snapshot_bytes(
    root: &Path,
    snapshot: &UnitSnapshot,
    side: SnapshotSide,
    require_root_owner: bool,
) -> Result<Vec<u8>> {
    let (directory, expected_size, digest) = match side {
        SnapshotSide::Prior => ("prior", snapshot.prior_size, &snapshot.prior_sha256),
        SnapshotSide::Target => ("target", snapshot.target_size, &snapshot.target_sha256),
    };
    let bytes = read_owned_regular(
        &root.join(directory).join(&snapshot.unit),
        MAX_UNIT_BYTES,
        require_root_owner,
    )?;
    if bytes.len() as u64 != expected_size || sha256(&bytes) != *digest {
        return Err(Error::new("sealed unit snapshot digest failed"));
    }
    Ok(bytes)
}

fn append_journal(
    root: &Path,
    transaction_id: &str,
    phase: &str,
    unit: Option<&str>,
    selected_generation_id: Option<&str>,
    require_root_owner: bool,
) -> Result<()> {
    if !valid_phase(phase)
        || unit.is_some_and(|value| !MUTABLE_UNIT_FRAGMENTS.contains(&value))
        || selected_generation_id.is_some_and(|value| !valid_identifier(value))
    {
        return Err(Error::new("unit transaction journal phase is invalid"));
    }
    let path = root.join("journal.jsonl");
    let previous = replay_journal(root, transaction_id, require_root_owner)?;
    let mut value = serde_json::json!({
        "schema": JOURNAL_SCHEMA,
        "transaction_id": transaction_id,
        "phase": phase,
        "unit": unit,
        "selected_generation_id": selected_generation_id,
        "recorded_at_unix_ms": unix_millis(),
        "previous_record_sha256": previous,
        "authority": AUTHORITY,
    });
    let digest = sha256(&canonical_json(&value)?);
    value["record_sha256"] = Value::String(digest);
    let mut options = OpenOptions::new();
    options
        .write(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || !owned_by_expected(&metadata, require_root_owner)
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("unit transaction journal identity failed"));
    }
    file.write_all(&canonical_json(&value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(root)?.sync_all()?;
    Ok(())
}

fn replay_journal(
    root: &Path,
    transaction_id: &str,
    require_root_owner: bool,
) -> Result<Option<String>> {
    let path = root.join("journal.jsonl");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_owned_regular(&path, 16 * 1024 * 1024, require_root_owner)?;
    let mut previous = None;
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let record: JournalRecord = serde_json::from_slice(line)?;
        let value: Value = serde_json::from_slice(line)?;
        let mut unhashed = value;
        unhashed
            .as_object_mut()
            .ok_or_else(|| Error::new("unit transaction record is not an object"))?
            .remove("record_sha256");
        if record.schema != JOURNAL_SCHEMA
            || record.transaction_id != transaction_id
            || record.authority != AUTHORITY
            || !valid_phase(&record.phase)
            || record.previous_record_sha256 != previous
            || !valid_hex64(&record.record_sha256)
            || sha256(&canonical_json(&unhashed)?) != record.record_sha256
            || record
                .unit
                .as_deref()
                .is_some_and(|unit| !MUTABLE_UNIT_FRAGMENTS.contains(&unit))
            || record
                .selected_generation_id
                .as_deref()
                .is_some_and(|id| !valid_identifier(id))
        {
            return Err(Error::new("unit transaction journal hash chain failed"));
        }
        previous = Some(record.record_sha256);
    }
    Ok(previous)
}

fn valid_phase(phase: &str) -> bool {
    matches!(
        phase,
        "prepared"
            | "apply_target_started"
            | "target_fragment_installed"
            | "target_daemon_reloaded"
            | "target_verified"
            | "apply_prior_started"
            | "prior_fragment_installed"
            | "prior_daemon_reloaded"
            | "prior_verified"
            | "apply_boot_reconcile_started"
            | "boot_reconcile_fragment_installed"
            | "boot_reconcile_daemon_reloaded"
            | "boot_reconcile_verified"
            | "committed"
            | "restored"
            | "boot_reconciled"
    )
}

fn finish_pending(
    config: &Config,
    transaction_id: &str,
    phase: &str,
    require_root_owner: bool,
) -> Result<()> {
    let pending = read_pending(config, require_root_owner)?;
    if pending.transaction_id != transaction_id {
        return Err(Error::new(
            "cannot finish a different pending unit transaction",
        ));
    }
    let root = config.roots.unit_transactions.join(transaction_id);
    append_journal(&root, transaction_id, phase, None, None, require_root_owner)?;
    clear_pending(config)
}

fn clear_pending(config: &Config) -> Result<()> {
    fs::remove_file(pending_path(config))?;
    File::open(&config.roots.unit_transactions)?.sync_all()?;
    Ok(())
}

fn read_pending(config: &Config, require_root_owner: bool) -> Result<Pending> {
    let bytes = read_owned_regular(&pending_path(config), 16 * 1024, require_root_owner)?;
    let pending: Pending = serde_json::from_slice(&bytes)?;
    if canonical_json(&pending)? != bytes
        || pending.schema != PENDING_SCHEMA
        || pending.authority != AUTHORITY
        || !valid_identifier(&pending.transaction_id)
        || !valid_hex64(&pending.manifest_sha256)
    {
        return Err(Error::new("pending unit transaction pointer is invalid"));
    }
    Ok(pending)
}

fn pending_path(config: &Config) -> PathBuf {
    config.roots.unit_transactions.join("pending.json")
}

fn require_roots(config: &Config, require_root_owner: bool) -> Result<()> {
    require_private_directory(&config.roots.unit_transactions, require_root_owner)?;
    let metadata = fs::symlink_metadata(&config.roots.system_unit_root)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || !owned_by_expected(&metadata, require_root_owner)
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new("system unit root identity failed"));
    }
    Ok(())
}

fn require_private_directory(path: &Path, require_root_owner: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || !owned_by_expected(&metadata, require_root_owner)
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "unit transaction root is not private immutable state",
        ));
    }
    Ok(())
}

fn read_owned_regular(path: &Path, maximum: u64, require_root_owner: bool) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || !owned_by_expected(&metadata, require_root_owner)
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(format!(
            "root transaction file identity failed: {}",
            path.display()
        )));
    }
    read_regular(path, maximum)
}

fn owned_by_expected(metadata: &fs::Metadata, require_root_owner: bool) -> bool {
    metadata.uid()
        == if require_root_owner {
            0
        } else {
            nix::unistd::geteuid().as_raw()
        }
}

fn generation_id(config: &Config, generation: &Path) -> Result<String> {
    ensure_within(&config.roots.releases, generation, true)?;
    if generation.parent() != Some(config.roots.releases.as_path()) {
        return Err(Error::new(
            "unit transaction generation is not one direct release",
        ));
    }
    generation
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| valid_identifier(name))
        .map(str::to_owned)
        .ok_or_else(|| Error::new("unit transaction generation ID is invalid"))
}

fn logical_source(profile: &str, unit: &str) -> String {
    if profile == "icp" {
        format!("packaging/systemd/icp/{unit}")
    } else {
        format!("packaging/systemd/{unit}")
    }
}

fn transaction_id(target: &str, prior: &str, policy: &[u8]) -> Result<String> {
    let mut entropy = [0_u8; 32];
    std::io::Read::read_exact(&mut File::open("/dev/urandom")?, &mut entropy)?;
    let digest = sha256(&canonical_json(&serde_json::json!({
        "target": target,
        "prior": prior,
        "policy_sha256": sha256(policy),
        "entropy_sha256": sha256(&entropy),
    }))?);
    Ok(format!("units-{}", &digest[..24]))
}

fn exact_output_line(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::new("systemd metadata output is not UTF-8"))?;
    let stripped = text.strip_suffix('\n').unwrap_or(text);
    if stripped.contains(['\n', '\r', '\0']) || stripped.len() > 32 * 1024 {
        return Err(Error::new("systemd metadata output is not one exact line"));
    }
    Ok(stripped.to_owned())
}

fn require_root(operation: &str) -> Result<()> {
    if nix::unistd::geteuid().as_raw() != 0 {
        return Err(Error::new(format!("{operation} requires root")));
    }
    Ok(())
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::config::{
        AudioPolicy, DrainConfig, Executables, HealthConfig, IdentityConfig, Policy, RootConfig,
        ServiceConfig, SourceConfig, StorageConfig, TrustedExecutable,
    };

    #[derive(Default)]
    struct MockRunner {
        system_root: PathBuf,
        dropins: BTreeMap<String, Vec<String>>,
        fail_daemon_reload: bool,
        effective_dropin_mismatch: bool,
    }

    impl NativeRunner for MockRunner {
        fn run(&mut self, spec: &CommandSpec) -> Result<CommandReceipt> {
            let mut receipt = receipt(spec);
            if self.fail_daemon_reload && spec.label == "systemd-daemon-reload-unit-transaction" {
                receipt.exit_code = Some(1);
            }
            Ok(receipt)
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
                .any(|item| item == "--property=FragmentPath")
            {
                format!("{}\n", self.system_root.join(&unit).display())
            } else if self.effective_dropin_mismatch {
                "\n".to_owned()
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
            Ok((receipt(spec), output.into_bytes()))
        }
    }

    fn receipt(spec: &CommandSpec) -> CommandReceipt {
        CommandReceipt {
            label: spec.label.to_owned(),
            execution_boundary: crate::native::CommandExecutionBoundary::TrustedHost,
            executable_sha256: "a".repeat(64),
            argv_sha256: "b".repeat(64),
            exit_code: Some(0),
            timed_out: false,
            duration_ms: 1,
        }
    }

    fn source(unit: &str) -> &'static str {
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
            _ => panic!("unexpected fixture unit"),
        }
    }

    #[allow(clippy::too_many_lines)] // Complete fixture mirrors all six fragments and boundaries.
    fn fixture(root: &Path) -> (Config, PathBuf, PathBuf, MockRunner) {
        let canonical_root = fs::canonicalize(root).unwrap();
        let root = canonical_root.as_path();
        let system = root.join("systemd");
        let releases = root.join("releases");
        let prior = releases.join("gen-prior");
        let target = releases.join("gen-target");
        fs::create_dir_all(&system).unwrap();
        fs::create_dir_all(&prior).unwrap();
        fs::create_dir_all(&target).unwrap();
        let active = root.join("current");
        std::os::unix::fs::symlink("releases/gen-prior", &active).unwrap();
        let mut dropins = Vec::new();
        let mut effective = BTreeMap::new();
        for unit in MUTABLE_UNIT_FRAGMENTS {
            let logical = format!("packaging/systemd/{unit}");
            let prior_path = prior.join(&logical);
            let target_path = target.join(&logical);
            fs::create_dir_all(prior_path.parent().unwrap()).unwrap();
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::write(&prior_path, source(unit)).unwrap();
            let changed = source(unit).replacen("Description=", "Description=Candidate ", 1);
            fs::write(&target_path, changed).unwrap();
            let installed = normalized_system_unit(&logical, source(unit), &active).unwrap();
            fs::write(system.join(unit), installed).unwrap();
            let directory = system.join(format!("{unit}.d"));
            fs::create_dir(&directory).unwrap();
            let boundary = directory.join("90-root-runtime-boundary.conf");
            fs::write(&boundary, b"[Service]\nNoNewPrivileges=yes\n").unwrap();
            let bytes = fs::read(&boundary).unwrap();
            let relative = format!("{unit}.d/90-root-runtime-boundary.conf");
            dropins.push(PolicyDropin {
                path: relative.clone(),
                size: bytes.len() as u64,
                sha256: sha256(&bytes),
            });
            effective
                .entry((*unit).to_owned())
                .or_insert_with(Vec::new)
                .push(system.join(relative).display().to_string());
            if *unit == "astrid-edge-runtime.service" {
                let path = directory.join("60-self-evolution-root.conf");
                fs::write(&path, b"[Service]\nProtectSystem=strict\n").unwrap();
                let bytes = fs::read(&path).unwrap();
                let relative = format!("{unit}.d/60-self-evolution-root.conf");
                dropins.push(PolicyDropin {
                    path: relative.clone(),
                    size: bytes.len() as u64,
                    sha256: sha256(&bytes),
                });
                effective
                    .entry((*unit).to_owned())
                    .or_insert_with(Vec::new)
                    .push(system.join(relative).display().to_string());
            }
        }
        dropins.sort();
        let transaction_root = root.join("snapshots/unit-transactions");
        fs::create_dir_all(&transaction_root).unwrap();
        fs::set_permissions(&transaction_root, fs::Permissions::from_mode(0o700)).unwrap();
        let policy_path = root.join("unit-policy.json");
        let policy = UnitPolicy {
            schema: POLICY_SCHEMA.to_owned(),
            authority: "operator_bootstrap_reviewed_immutable_dropins".to_owned(),
            system_unit_root: system.display().to_string(),
            mutable_fragments: MUTABLE_UNIT_FRAGMENTS
                .iter()
                .map(|item| (*item).to_owned())
                .collect(),
            immutable_dropins: dropins,
        };
        fs::write(&policy_path, canonical_json(&policy).unwrap()).unwrap();
        let executable = TrustedExecutable {
            path: root.join("native"),
            sha256: "a".repeat(64),
        };
        let config = Config {
            schema: "astrid.edge_rescue_helper.config.v1".to_owned(),
            appliance_id: "avado".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            model: "qwen3.5:4b".to_owned(),
            ollama_origin: "http://127.0.0.1:11434".to_owned(),
            source: SourceConfig {
                root: root.join("source"),
                manifest: root.join("source/manifest"),
                signature: root.join("source/signature"),
                signing_key: root.join("key"),
                intent_attestation_key: root.join("intent-key"),
                ledger_attestation_key: root.join("ledger-key"),
                vendor: root.join("source/vendor"),
            },
            roots: RootConfig {
                supervisor_state: root.to_path_buf(),
                candidate_store: root.join("candidates"),
                model_handoff_root: root.join("model-handoff"),
                model_handoff_ledger: root.join("model-unload-receipts.jsonl"),
                candidate_work: root.join("work"),
                build_store: root.join("builds"),
                releases,
                active_link: active,
                generation_binding: root.join("current-generation"),
                maintenance_lease: root.join("maintenance.json"),
                maintenance_mutex: root.join("maintenance.lock"),
                state_snapshots: root.join("snapshots"),
                workspace: root.join("workspace"),
                system_unit_root: system.clone(),
                unit_policy: policy_path,
                unit_transactions: transaction_root,
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
                core: "astrid.service".to_owned(),
                warmup: "astrid-model-warmup.service".to_owned(),
                edge: "astrid-edge-runtime.service".to_owned(),
            },
            storage: StorageConfig {
                config: root.join("state-store.json"),
                config_sha256: "b".repeat(64),
                install_attestation: root.join("install-attestation.json"),
                health_attestation: root.join("health-attestation.json"),
                runtime_state_mount: root.join("workspace"),
                rollback_mount: root.join("snapshots"),
                backing_uuid: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
                runtime_filesystem_uuid: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".to_owned(),
                rollback_filesystem_uuid: "cccccccc-cccc-4ccc-8ccc-cccccccccccc".to_owned(),
                image_bytes: 32 * 1024 * 1024 * 1024,
                host_reserve_bytes: 64 * 1024 * 1024 * 1024,
                store_minimum_free_bytes: 4 * 1024 * 1024 * 1024,
                emergency_inode_reserve_files: 65_536,
            },
            policy: Policy {
                maximum_files: 25,
                maximum_changed_lines: 4_000,
                build_workers: 4,
                command_timeout_seconds: 300,
                pipeline_timeout_seconds: 600,
                maximum_candidate_bytes: 1024 * 1024,
                minimum_free_disk_bytes: 1024 * 1024 * 1024,
                candidate_memory_max_bytes: 4 * 1024 * 1024 * 1024,
                candidate_memory_swap_max_bytes: 128 * 1024 * 1024,
                candidate_tasks_max: 256,
                candidate_cpu_quota_percent: 400,
                network_policy: "private-network-none:v1".to_owned(),
                dependency_policy: "signed-vendor-offline-locked:v1".to_owned(),
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
        };
        (
            config,
            prior,
            target,
            MockRunner {
                system_root: system,
                dropins: effective,
                fail_daemon_reload: false,
                effective_dropin_mismatch: false,
            },
        )
    }

    fn assert_live_matches(config: &Config, generation: &Path) {
        for unit in MUTABLE_UNIT_FRAGMENTS {
            let logical = format!("packaging/systemd/{unit}");
            let expected = read_unit_source(config, generation, &logical).unwrap();
            assert_eq!(
                fs::read(config.roots.system_unit_root.join(unit)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn activation_and_rollback_install_exact_fragments() {
        let temp = tempfile::tempdir().unwrap();
        let (config, prior, target, mut runner) = fixture(temp.path());
        let transaction = prepare_inner(&config, &mut runner, &target, &prior, false).unwrap();
        let evidence = apply_selected_inner(
            &config,
            &mut runner,
            &transaction,
            &transaction.target_generation_id,
            SnapshotSide::Target,
            "target",
            false,
        )
        .unwrap();
        assert_eq!(evidence.authority, AUTHORITY);
        commit_inner(&config, &transaction, false).unwrap();
        assert_live_matches(&config, &target);
        assert!(!pending_path(&config).exists());

        let rollback = prepare_inner(&config, &mut runner, &prior, &target, false).unwrap();
        apply_selected_inner(
            &config,
            &mut runner,
            &rollback,
            &rollback.target_generation_id,
            SnapshotSide::Target,
            "target",
            false,
        )
        .unwrap();
        commit_inner(&config, &rollback, false).unwrap();
        assert_live_matches(&config, &prior);
    }

    #[test]
    fn every_partial_install_reconciles_to_outer_generation_choice() {
        for selected_target in [false, true] {
            for installed in 0..=MUTABLE_UNIT_FRAGMENTS.len() {
                let temp = tempfile::tempdir().unwrap();
                let (config, prior, target, mut runner) = fixture(temp.path());
                let transaction =
                    prepare_inner(&config, &mut runner, &target, &prior, false).unwrap();
                let root = config
                    .roots
                    .unit_transactions
                    .join(&transaction.transaction_id);
                let manifest: TransactionManifest =
                    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
                append_journal(
                    &root,
                    &transaction.transaction_id,
                    "apply_target_started",
                    None,
                    Some(&transaction.target_generation_id),
                    false,
                )
                .unwrap();
                for snapshot in manifest.units.iter().take(installed) {
                    let bytes =
                        snapshot_bytes(&root, snapshot, SnapshotSide::Target, false).unwrap();
                    replace_fragment(&config, &snapshot.unit, &bytes, false).unwrap();
                }
                let selected = if selected_target { &target } else { &prior };
                reconcile_inner(&config, &mut runner, selected, false).unwrap();
                assert_live_matches(&config, selected);
                assert!(!pending_path(&config).exists());
            }
        }
    }

    #[test]
    fn changed_or_extra_dropin_fails_before_fragment_install() {
        let temp = tempfile::tempdir().unwrap();
        let (config, prior, target, mut runner) = fixture(temp.path());
        let transaction = prepare_inner(&config, &mut runner, &target, &prior, false).unwrap();
        fs::write(
            config
                .roots
                .system_unit_root
                .join("astrid.service.d/90-root-runtime-boundary.conf"),
            b"changed",
        )
        .unwrap();
        assert!(
            apply_selected_inner(
                &config,
                &mut runner,
                &transaction,
                &transaction.target_generation_id,
                SnapshotSide::Target,
                "target",
                false,
            )
            .is_err()
        );
        assert_live_matches(&config, &prior);
    }

    #[test]
    fn daemon_reload_failure_retains_pending_and_restores_exact_prior() {
        let temp = tempfile::tempdir().unwrap();
        let (config, prior, target, mut runner) = fixture(temp.path());
        let transaction = prepare_inner(&config, &mut runner, &target, &prior, false).unwrap();
        runner.fail_daemon_reload = true;
        assert!(
            apply_selected_inner(
                &config,
                &mut runner,
                &transaction,
                &transaction.target_generation_id,
                SnapshotSide::Target,
                "target",
                false,
            )
            .is_err()
        );
        assert!(pending_path(&config).exists());
        runner.fail_daemon_reload = false;
        restore_prior_for_transition(&config, &mut runner, &transaction, false).unwrap();
        assert_live_matches(&config, &prior);
        assert!(!pending_path(&config).exists());
    }

    #[test]
    fn effective_dropin_mismatch_retains_pending_and_restores_exact_prior() {
        let temp = tempfile::tempdir().unwrap();
        let (config, prior, target, mut runner) = fixture(temp.path());
        let transaction = prepare_inner(&config, &mut runner, &target, &prior, false).unwrap();
        runner.effective_dropin_mismatch = true;
        assert!(
            apply_selected_inner(
                &config,
                &mut runner,
                &transaction,
                &transaction.target_generation_id,
                SnapshotSide::Target,
                "target",
                false,
            )
            .is_err()
        );
        assert!(pending_path(&config).exists());
        runner.effective_dropin_mismatch = false;
        restore_prior_for_transition(&config, &mut runner, &transaction, false).unwrap();
        assert_live_matches(&config, &prior);
        assert!(!pending_path(&config).exists());
    }

    #[test]
    fn policy_cannot_name_rescue_steward_broker_runtime_or_host_units() {
        let temp = tempfile::tempdir().unwrap();
        let (config, _, _, _) = fixture(temp.path());
        let mut policy: UnitPolicy =
            serde_json::from_slice(&fs::read(&config.roots.unit_policy).unwrap()).unwrap();
        policy.immutable_dropins.push(PolicyDropin {
            path: "ssh.service.d/override.conf".to_owned(),
            size: 1,
            sha256: "a".repeat(64),
        });
        assert!(validate_policy(&config, &policy).is_err());
        for forbidden in [
            "astrid-edge-rescue-helper.service",
            "astrid-edge-steward.service",
            "astrid-edge-web-broker-runtime.socket",
            "astrid-edge-generation-guard.service",
            "sudo.service",
            "ssh.service",
        ] {
            assert!(!MUTABLE_UNIT_FRAGMENTS.contains(&forbidden));
        }
    }
}
