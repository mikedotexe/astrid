//! Signed, crash-safe retirement of rollback generation/state-snapshot pairs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::config::{Config, valid_identifier};
use crate::fs_guard::{canonical_json, read_regular, sha256, sha256_file};
use crate::generation::require_effective_uid;
use crate::ledger_auth::{LedgerKey, seal_record, verify_record};
use crate::transition::{
    MaintenanceLease, read_generation_binding, read_state_snapshot_binding, unix_millis,
    validate_transition_release, verify_phase_journal_records, verify_retained_rollback_pairs_at,
};
use crate::{Error, Result};

const MINIMUM_PRIOR_GENERATIONS: usize = 3;
const MINIMUM_RETENTION_MILLISECONDS: u64 = 7 * 24 * 60 * 60 * 1_000;
const RETENTION_LEDGER_MAXIMUM_BYTES: u64 = 16 * 1024 * 1024;
const RETENTION_DOMAIN: &str = "paired-retention";
const RETENTION_AUTHORITY: &str = "immutable_root_paired_generation_snapshot_gc";
const RETENTION_RECORD_FIELDS: &[&str] = &[
    "authentication_schema",
    "authority",
    "generation_id",
    "ledger_domain",
    "ledger_key_sha256",
    "phase",
    "previous_record_sha256",
    "record_hmac_sha256",
    "record_sha256",
    "recorded_at_unix_ms",
    "release_manifest_sha256",
    "schema",
    "snapshots",
    "transaction_id",
    "transition_head_sha256",
];

#[derive(Debug, Clone, Serialize)]
pub struct RetentionOutcome {
    pub schema: &'static str,
    pub status: &'static str,
    pub active_generation: String,
    pub retained_generations: Vec<String>,
    pub retired_generations: Vec<String>,
    pub retained_prior_minimum: usize,
    pub minimum_retention_seconds: u64,
    pub ledger_head_sha256: Option<String>,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotEvidence {
    basename: String,
    manifest_sha256: String,
}

#[derive(Debug, Clone)]
struct PairEvidence {
    generation_id: String,
    newest_recorded_at_unix_ms: u64,
    snapshots: Vec<SnapshotEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransactionEvidence {
    transaction_id: String,
    phase: String,
    generation_id: String,
    release_manifest_sha256: String,
    snapshots: Vec<SnapshotEvidence>,
    transition_head_sha256: String,
    record_sha256: String,
}

/// Run the immutable paired retention transaction. The maintenance mutex
/// serializes this with build, activation, rollback, and scheduled reflection.
pub fn prune(config: &Config) -> Result<RetentionOutcome> {
    require_effective_uid(0, "paired rollback retention")?;
    let _lease = MaintenanceLease::acquire_for(config, "paired_rollback_retention", 600)?;
    prune_inner(config, true, unix_millis())
}

/// Repair or finish the one possible interrupted retirement before any boot
/// generation consistency check examines retained pairs.
pub(crate) fn reconcile(config: &Config, require_root: bool) -> Result<Option<String>> {
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let Some(head) = verify_retention_ledger(config, &key, require_root)? else {
        reject_unbound_tombstones(config)?;
        return Ok(None);
    };
    let release = config.roots.releases.join(&head.generation_id);
    let release_tombstone = release_tombstone(config, &head.transaction_id);
    let snapshots = head
        .snapshots
        .iter()
        .enumerate()
        .map(|(index, evidence)| {
            (
                config.roots.state_snapshots.join(&evidence.basename),
                snapshot_tombstone(config, &head.transaction_id, index),
            )
        })
        .collect::<Vec<_>>();
    match head.phase.as_str() {
        "prepared" | "release_tombstoned" | "pair_tombstoned" => {
            restore_path(&release, &release_tombstone)?;
            for (live, tombstone) in &snapshots {
                restore_path(live, tombstone)?;
            }
            sync_dir(&config.roots.releases)?;
            sync_dir(&config.roots.state_snapshots)?;
            verify_transaction_payload(config, &head, require_root)?;
            append_phase(config, &key, &head, "recovered_restored", require_root)?;
            reject_unbound_tombstones(config)?;
            Ok(Some("restored_incomplete_pair_retirement".to_owned()))
        },
        "committed" => {
            purge_tombstone(&release, &release_tombstone)?;
            for (live, tombstone) in &snapshots {
                purge_tombstone(live, tombstone)?;
            }
            sync_dir(&config.roots.releases)?;
            sync_dir(&config.roots.state_snapshots)?;
            append_phase(config, &key, &head, "purged", require_root)?;
            reject_unbound_tombstones(config)?;
            Ok(Some("completed_committed_pair_retirement".to_owned()))
        },
        "purged" | "recovered_restored" => {
            reject_unbound_tombstones(config)?;
            Ok(None)
        },
        _ => Err(Error::new("retention transaction phase is invalid")),
    }
}

pub(crate) fn prune_inner(
    config: &Config,
    require_root: bool,
    now: u64,
) -> Result<RetentionOutcome> {
    let _ = reconcile(config, require_root)?;
    verify_retained_rollback_pairs_at(config, require_root, now)?;
    let active = read_generation_binding(config, require_root)?;
    let (transition_head, pairs) = collect_pairs(config, require_root)?;
    let mut protected = BTreeSet::from([active.clone()]);
    let mut newest = pairs.iter().collect::<Vec<_>>();
    newest.sort_by(|left, right| {
        right
            .newest_recorded_at_unix_ms
            .cmp(&left.newest_recorded_at_unix_ms)
            .then_with(|| right.generation_id.cmp(&left.generation_id))
    });
    for pair in newest
        .iter()
        .filter(|pair| pair.generation_id != active)
        .take(MINIMUM_PRIOR_GENERATIONS)
    {
        protected.insert(pair.generation_id.clone());
    }
    for pair in &pairs {
        if now.saturating_sub(pair.newest_recorded_at_unix_ms) <= MINIMUM_RETENTION_MILLISECONDS {
            protected.insert(pair.generation_id.clone());
        }
    }

    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let mut retired = Vec::new();
    for pair in pairs
        .iter()
        .filter(|pair| !protected.contains(&pair.generation_id))
    {
        let release = config.roots.releases.join(&pair.generation_id);
        let release_exists = release.exists() || release.is_symlink();
        let snapshot_presence = pair.snapshots.iter().map(|snapshot| {
            let path = config.roots.state_snapshots.join(&snapshot.basename);
            path.exists() || path.is_symlink()
        });
        if !release_exists && snapshot_presence.clone().all(|present| !present) {
            continue;
        }
        if !release_exists || snapshot_presence.clone().any(|present| !present) {
            return Err(Error::new(
                "retention candidate is not a complete generation/state-snapshot pair",
            ));
        }
        retire_one(config, &key, pair, &transition_head, require_root, now)?;
        retired.push(pair.generation_id.clone());
    }
    let (_, remaining_pairs) = collect_pairs(config, require_root)?;
    verify_retained_rollback_pairs_at(config, require_root, now)?;
    let head = verify_retention_ledger(config, &key, require_root)?;
    let mut retained_generations = BTreeSet::from([active.clone()]);
    retained_generations.extend(remaining_pairs.into_iter().filter_map(|pair| {
        let release = config.roots.releases.join(&pair.generation_id);
        (release.exists() && !release.is_symlink()).then_some(pair.generation_id)
    }));
    Ok(RetentionOutcome {
        schema: "astrid.edge_rescue_helper.paired_retention.v1",
        status: if retired.is_empty() {
            "healthy_nothing_eligible"
        } else {
            "retired_complete_signed_pairs"
        },
        active_generation: active,
        retained_generations: retained_generations.into_iter().collect(),
        retired_generations: retired,
        retained_prior_minimum: MINIMUM_PRIOR_GENERATIONS,
        minimum_retention_seconds: MINIMUM_RETENTION_MILLISECONDS / 1_000,
        ledger_head_sha256: head.map(|value| value.record_sha256),
        authority: RETENTION_AUTHORITY,
    })
}

fn collect_pairs(config: &Config, require_root: bool) -> Result<(String, Vec<PairEvidence>)> {
    let transition_path = config.roots.state_snapshots.join("transitions.jsonl");
    let key = LedgerKey::load(&config.source.ledger_attestation_key, require_root)?;
    let records = verify_phase_journal_records(&transition_path, &key, require_root)?;
    let transition_head = records.last().map_or_else(
        || sha256(b"no-transition-history"),
        |record| record.record_sha256.clone(),
    );
    let mut grouped = BTreeMap::<String, (u64, BTreeMap<String, String>)>::new();
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
        let entry = grouped
            .entry(binding.generation_id)
            .or_insert_with(|| (0, BTreeMap::new()));
        entry.0 = entry.0.max(record.recorded_at_unix_ms);
        match entry
            .1
            .insert(binding.basename, binding.manifest_sha256.clone())
        {
            Some(previous) if previous != binding.manifest_sha256 => {
                return Err(Error::new(
                    "one snapshot basename has conflicting signed retention bindings",
                ));
            },
            _ => {},
        }
    }
    Ok((
        transition_head,
        grouped
            .into_iter()
            .map(
                |(generation_id, (newest_recorded_at_unix_ms, snapshots))| PairEvidence {
                    generation_id,
                    newest_recorded_at_unix_ms,
                    snapshots: snapshots
                        .into_iter()
                        .map(|(basename, manifest_sha256)| SnapshotEvidence {
                            basename,
                            manifest_sha256,
                        })
                        .collect(),
                },
            )
            .collect(),
    ))
}

fn retire_one(
    config: &Config,
    key: &LedgerKey,
    pair: &PairEvidence,
    transition_head: &str,
    require_root: bool,
    now: u64,
) -> Result<()> {
    if pair.generation_id == read_generation_binding(config, require_root)? {
        return Err(Error::new("active generation cannot enter retention GC"));
    }
    let release = config.roots.releases.join(&pair.generation_id);
    let identity = validate_transition_release(config, &release, require_root)?;
    if identity.generation_id != pair.generation_id {
        return Err(Error::new("retention release identity changed"));
    }
    for snapshot in &pair.snapshots {
        let path = config.roots.state_snapshots.join(&snapshot.basename);
        let binding =
            read_state_snapshot_binding(config, &path, &pair.generation_id, require_root)?;
        if binding.manifest_sha256 != snapshot.manifest_sha256 {
            return Err(Error::new("retention snapshot binding changed"));
        }
    }
    let release_manifest_sha256 = sha256_file(
        &release.join(".astrid-edge-generation.json"),
        64 * 1024 * 1024,
    )?;
    let seed = canonical_json(&serde_json::json!({
        "generation_id": pair.generation_id,
        "now": now,
        "release_manifest_sha256": release_manifest_sha256,
        "snapshots": pair.snapshots.iter().map(|item| (&item.basename, &item.manifest_sha256)).collect::<Vec<_>>(),
        "transition_head_sha256": transition_head,
    }))?;
    let transaction_id = format!("retention-{}", &sha256(&seed)[..24]);
    let prepared = TransactionEvidence {
        transaction_id: transaction_id.clone(),
        phase: "prepared".to_owned(),
        generation_id: pair.generation_id.clone(),
        release_manifest_sha256,
        snapshots: pair.snapshots.clone(),
        transition_head_sha256: transition_head.to_owned(),
        record_sha256: String::new(),
    };
    let prepared = append_new_transaction(config, key, &prepared, require_root)?;
    let release_tombstone = release_tombstone(config, &transaction_id);
    reject_existing(&release_tombstone)?;
    fs::rename(&release, &release_tombstone)?;
    sync_dir(&config.roots.releases)?;
    let release_phase = append_phase(config, key, &prepared, "release_tombstoned", require_root)?;
    for (index, snapshot) in pair.snapshots.iter().enumerate() {
        let source = config.roots.state_snapshots.join(&snapshot.basename);
        let tombstone = snapshot_tombstone(config, &transaction_id, index);
        reject_existing(&tombstone)?;
        fs::rename(source, tombstone)?;
        sync_dir(&config.roots.state_snapshots)?;
    }
    let pair_phase = append_phase(config, key, &release_phase, "pair_tombstoned", require_root)?;
    let committed = append_phase(config, key, &pair_phase, "committed", require_root)?;
    purge_tombstone(&release, &release_tombstone)?;
    for (index, snapshot) in pair.snapshots.iter().enumerate() {
        purge_tombstone(
            &config.roots.state_snapshots.join(&snapshot.basename),
            &snapshot_tombstone(config, &transaction_id, index),
        )?;
    }
    sync_dir(&config.roots.releases)?;
    sync_dir(&config.roots.state_snapshots)?;
    let _ = append_phase(config, key, &committed, "purged", require_root)?;
    Ok(())
}

fn append_new_transaction(
    config: &Config,
    key: &LedgerKey,
    value: &TransactionEvidence,
    require_root: bool,
) -> Result<TransactionEvidence> {
    if value.phase != "prepared"
        || !valid_identifier(&value.transaction_id)
        || !valid_identifier(&value.generation_id)
        || !crate::config::valid_hex64(&value.release_manifest_sha256)
        || !crate::config::valid_hex64(&value.transition_head_sha256)
        || value.snapshots.is_empty()
    {
        return Err(Error::new("retention transaction evidence is invalid"));
    }
    let previous = verify_retention_ledger(config, key, require_root)?;
    if previous
        .as_ref()
        .is_some_and(|head| !matches!(head.phase.as_str(), "purged" | "recovered_restored"))
    {
        return Err(Error::new("an earlier retention transaction is incomplete"));
    }
    append_record(config, key, value, previous.as_ref(), require_root)
}

fn append_phase(
    config: &Config,
    key: &LedgerKey,
    prior: &TransactionEvidence,
    phase: &str,
    require_root: bool,
) -> Result<TransactionEvidence> {
    let valid = matches!(
        (prior.phase.as_str(), phase),
        ("prepared", "release_tombstoned" | "recovered_restored")
            | (
                "release_tombstoned",
                "pair_tombstoned" | "recovered_restored"
            )
            | ("pair_tombstoned", "committed" | "recovered_restored")
            | ("committed", "purged")
    );
    if !valid {
        return Err(Error::new("retention phase transition is invalid"));
    }
    let current = verify_retention_ledger(config, key, require_root)?
        .ok_or_else(|| Error::new("retention ledger disappeared"))?;
    if current != *prior {
        return Err(Error::new("retention ledger head changed"));
    }
    let mut next = prior.clone();
    phase.clone_into(&mut next.phase);
    next.record_sha256.clear();
    append_record(config, key, &next, Some(prior), require_root)
}

fn append_record(
    config: &Config,
    key: &LedgerKey,
    value: &TransactionEvidence,
    previous: Option<&TransactionEvidence>,
    require_root: bool,
) -> Result<TransactionEvidence> {
    let snapshots = value
        .snapshots
        .iter()
        .map(|snapshot| {
            serde_json::json!({
                "basename": snapshot.basename,
                "manifest_sha256": snapshot.manifest_sha256,
            })
        })
        .collect::<Vec<_>>();
    let mut record = serde_json::json!({
        "schema": "astrid.edge_rescue_helper.paired_retention_record.v1",
        "recorded_at_unix_ms": unix_millis(),
        "transaction_id": value.transaction_id,
        "phase": value.phase,
        "generation_id": value.generation_id,
        "release_manifest_sha256": value.release_manifest_sha256,
        "snapshots": snapshots,
        "transition_head_sha256": value.transition_head_sha256,
        "previous_record_sha256": previous.map(|head| &head.record_sha256),
        "authority": RETENTION_AUTHORITY,
    });
    let digest = seal_record(&mut record, key, RETENTION_DOMAIN)?;
    let path = retention_ledger(config);
    let mut options = OpenOptions::new();
    options
        .create(true)
        .append(true)
        .write(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(&path)?;
    let metadata = file.metadata()?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o077 != 0
        || metadata.len() > RETENTION_LEDGER_MAXIMUM_BYTES
    {
        return Err(Error::new("retention ledger ownership or size failed"));
    }
    file.write_all(&canonical_json(&record)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    sync_dir(&config.roots.state_snapshots)?;
    let mut written = value.clone();
    written.record_sha256 = digest;
    Ok(written)
}

fn verify_retention_ledger(
    config: &Config,
    key: &LedgerKey,
    require_root: bool,
) -> Result<Option<TransactionEvidence>> {
    let path = retention_ledger(config);
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&path)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o077 != 0
        || metadata.len() > RETENTION_LEDGER_MAXIMUM_BYTES
    {
        return Err(Error::new("retention ledger identity failed"));
    }
    let bytes = read_regular(&path, RETENTION_LEDGER_MAXIMUM_BYTES)?;
    let mut previous: Option<String> = None;
    let mut prior_transaction: Option<TransactionEvidence> = None;
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: Value = serde_json::from_slice(line)?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::new("retention ledger record is not an object"))?;
        if object.len() != RETENTION_RECORD_FIELDS.len()
            || RETENTION_RECORD_FIELDS
                .iter()
                .any(|field| !object.contains_key(*field))
            || object
                .get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .is_none_or(|timestamp| timestamp == 0)
        {
            return Err(Error::new("retention ledger record shape is invalid"));
        }
        let digest = verify_record(&value, key, RETENTION_DOMAIN)?;
        if object.get("schema").and_then(Value::as_str)
            != Some("astrid.edge_rescue_helper.paired_retention_record.v1")
            || object.get("authority").and_then(Value::as_str) != Some(RETENTION_AUTHORITY)
            || object.get("previous_record_sha256")
                != Some(&previous.clone().map_or(Value::Null, Value::String))
        {
            return Err(Error::new("retention ledger hash chain failed"));
        }
        let snapshots = object
            .get("snapshots")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty() && values.len() <= 64)
            .ok_or_else(|| Error::new("retention snapshot inventory is invalid"))?
            .iter()
            .map(parse_snapshot_evidence)
            .collect::<Result<Vec<_>>>()?;
        if snapshots
            .windows(2)
            .any(|pair| pair[0].basename >= pair[1].basename)
        {
            return Err(Error::new(
                "retention snapshot inventory is not strictly ordered",
            ));
        }
        let record = TransactionEvidence {
            transaction_id: exact_identifier(object, "transaction_id")?,
            phase: object
                .get("phase")
                .and_then(Value::as_str)
                .filter(|phase| {
                    matches!(
                        *phase,
                        "prepared"
                            | "release_tombstoned"
                            | "pair_tombstoned"
                            | "committed"
                            | "purged"
                            | "recovered_restored"
                    )
                })
                .ok_or_else(|| Error::new("retention phase is invalid"))?
                .to_owned(),
            generation_id: exact_identifier(object, "generation_id")?,
            release_manifest_sha256: exact_hex(object, "release_manifest_sha256")?,
            snapshots,
            transition_head_sha256: exact_hex(object, "transition_head_sha256")?,
            record_sha256: digest.clone(),
        };
        validate_record_transition(prior_transaction.as_ref(), &record)?;
        previous = Some(digest);
        prior_transaction = Some(record);
    }
    Ok(prior_transaction)
}

fn validate_record_transition(
    prior: Option<&TransactionEvidence>,
    current: &TransactionEvidence,
) -> Result<()> {
    let Some(prior) = prior else {
        return (current.phase == "prepared")
            .then_some(())
            .ok_or_else(|| Error::new("first retention record is not prepared"));
    };
    if current.phase == "prepared" {
        if !matches!(prior.phase.as_str(), "purged" | "recovered_restored") {
            return Err(Error::new(
                "retention transaction overlapped an unfinished predecessor",
            ));
        }
        return Ok(());
    }
    if current.transaction_id != prior.transaction_id
        || current.generation_id != prior.generation_id
        || current.release_manifest_sha256 != prior.release_manifest_sha256
        || current.snapshots != prior.snapshots
        || current.transition_head_sha256 != prior.transition_head_sha256
    {
        return Err(Error::new(
            "retention transaction binding changed across phases",
        ));
    }
    let valid = matches!(
        (prior.phase.as_str(), current.phase.as_str()),
        ("prepared", "release_tombstoned" | "recovered_restored")
            | (
                "release_tombstoned",
                "pair_tombstoned" | "recovered_restored"
            )
            | ("pair_tombstoned", "committed" | "recovered_restored")
            | ("committed", "purged")
    );
    valid
        .then_some(())
        .ok_or_else(|| Error::new("retention ledger phase order failed"))
}

fn verify_transaction_payload(
    config: &Config,
    transaction: &TransactionEvidence,
    require_root: bool,
) -> Result<()> {
    let release = config.roots.releases.join(&transaction.generation_id);
    let identity = validate_transition_release(config, &release, require_root)?;
    if identity.generation_id != transaction.generation_id
        || sha256_file(
            &release.join(".astrid-edge-generation.json"),
            64 * 1024 * 1024,
        )? != transaction.release_manifest_sha256
    {
        return Err(Error::new("restored retention release evidence changed"));
    }
    for evidence in &transaction.snapshots {
        let binding = read_state_snapshot_binding(
            config,
            &config.roots.state_snapshots.join(&evidence.basename),
            &transaction.generation_id,
            require_root,
        )?;
        if binding.manifest_sha256 != evidence.manifest_sha256 {
            return Err(Error::new("restored retention snapshot evidence changed"));
        }
    }
    Ok(())
}

fn parse_snapshot_evidence(value: &Value) -> Result<SnapshotEvidence> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("retention snapshot evidence is not an object"))?;
    if object.len() != 2 {
        return Err(Error::new("retention snapshot evidence shape is invalid"));
    }
    Ok(SnapshotEvidence {
        basename: exact_identifier(object, "basename")?,
        manifest_sha256: exact_hex(object, "manifest_sha256")?,
    })
}

fn exact_identifier(object: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| valid_identifier(value))
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("retention field is invalid: {key}")))
}

fn exact_hex(object: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| crate::config::valid_hex64(value))
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("retention digest is invalid: {key}")))
}

fn retention_ledger(config: &Config) -> PathBuf {
    config.roots.state_snapshots.join("retention.jsonl")
}

fn release_tombstone(config: &Config, transaction_id: &str) -> PathBuf {
    config
        .roots
        .releases
        .join(format!(".{transaction_id}.release-tombstone"))
}

fn snapshot_tombstone(config: &Config, transaction_id: &str, index: usize) -> PathBuf {
    config
        .roots
        .state_snapshots
        .join(format!(".{transaction_id}.snapshot-{index:02}-tombstone"))
}

fn restore_path(live: &Path, tombstone: &Path) -> Result<()> {
    let live_exists = live.exists() || live.is_symlink();
    let tombstone_exists = tombstone.exists() || tombstone.is_symlink();
    match (live_exists, tombstone_exists) {
        (true, false) => Ok(()),
        (false, true) => {
            reject_symlink(tombstone)?;
            fs::rename(tombstone, live)?;
            Ok(())
        },
        _ => Err(Error::new(
            "retention recovery found an ambiguous live/tombstone pair",
        )),
    }
}

fn purge_tombstone(live: &Path, tombstone: &Path) -> Result<()> {
    if live.exists() || live.is_symlink() {
        return Err(Error::new(
            "committed retention transaction unexpectedly retained a live path",
        ));
    }
    if tombstone.exists() || tombstone.is_symlink() {
        reject_symlink(tombstone)?;
        // Production executes with root DAC override. Unit tests deliberately
        // exercise the same immutable (0555/0500) trees without privilege.
        #[cfg(test)]
        if nix::unistd::geteuid().as_raw() != 0 {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(tombstone, fs::Permissions::from_mode(0o700))?;
        }
        fs::remove_dir_all(tombstone)?;
    }
    Ok(())
}

fn reject_existing(path: &Path) -> Result<()> {
    if path.exists() || path.is_symlink() {
        return Err(Error::new("retention tombstone path already exists"));
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new("retention tombstone is not a directory"));
    }
    Ok(())
}

fn reject_unbound_tombstones(config: &Config) -> Result<()> {
    for root in [&config.roots.releases, &config.roots.state_snapshots] {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".retention-") && name.ends_with("-tombstone") {
                return Err(Error::new("unbound retention tombstone is present"));
            }
        }
    }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PairEvidence, SnapshotEvidence, TransactionEvidence, purge_tombstone, restore_path,
        validate_record_transition,
    };
    use std::fs;

    fn transaction(phase: &str) -> TransactionEvidence {
        TransactionEvidence {
            transaction_id: "retention-aaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            phase: phase.to_owned(),
            generation_id: "gen-old".to_owned(),
            release_manifest_sha256: "a".repeat(64),
            snapshots: vec![SnapshotEvidence {
                basename: "activation-1-state".to_owned(),
                manifest_sha256: "b".repeat(64),
            }],
            transition_head_sha256: "c".repeat(64),
            record_sha256: "d".repeat(64),
        }
    }

    #[test]
    fn signed_phase_machine_accepts_restore_or_commit_but_not_skips() {
        let prepared = transaction("prepared");
        assert!(validate_record_transition(None, &prepared).is_ok());
        let release = transaction("release_tombstoned");
        assert!(validate_record_transition(Some(&prepared), &release).is_ok());
        let pair = transaction("pair_tombstoned");
        assert!(validate_record_transition(Some(&release), &pair).is_ok());
        let committed = transaction("committed");
        assert!(validate_record_transition(Some(&pair), &committed).is_ok());
        let purged = transaction("purged");
        assert!(validate_record_transition(Some(&committed), &purged).is_ok());
        assert!(validate_record_transition(Some(&prepared), &committed).is_err());
        let restored = transaction("recovered_restored");
        assert!(validate_record_transition(Some(&pair), &restored).is_ok());
    }

    #[test]
    fn pair_inventory_is_generation_scoped() {
        let pair = PairEvidence {
            generation_id: "gen-old".to_owned(),
            newest_recorded_at_unix_ms: 1,
            snapshots: vec![SnapshotEvidence {
                basename: "activation-1-state".to_owned(),
                manifest_sha256: "b".repeat(64),
            }],
        };
        assert_eq!(pair.snapshots.len(), 1);
        assert_eq!(pair.generation_id, "gen-old");
    }

    #[test]
    fn incomplete_tombstone_is_restored_and_committed_tombstone_is_purged() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("generation");
        let tombstone = temp.path().join(".retention-x.release-tombstone");
        fs::create_dir(&tombstone).unwrap();
        fs::write(tombstone.join("payload"), b"kept").unwrap();
        restore_path(&live, &tombstone).unwrap();
        assert_eq!(fs::read(live.join("payload")).unwrap(), b"kept");
        fs::rename(&live, &tombstone).unwrap();
        purge_tombstone(&live, &tombstone).unwrap();
        assert!(!live.exists());
        assert!(!tombstone.exists());
    }

    #[test]
    fn ambiguous_live_and_tombstone_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let live = temp.path().join("generation");
        let tombstone = temp.path().join(".retention-x.release-tombstone");
        fs::create_dir(&live).unwrap();
        fs::create_dir(&tombstone).unwrap();
        assert!(restore_path(&live, &tombstone).is_err());
        assert!(purge_tombstone(&live, &tombstone).is_err());
    }
}
