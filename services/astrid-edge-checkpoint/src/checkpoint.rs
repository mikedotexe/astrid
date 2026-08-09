//! Independent validation of mutable hindsight claims and exact ledger prefixes.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

use astrid_edge_rescue_helper::fs_guard::{atomic_write, canonical_json, read_json, sha256};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{AUTHORITY, Error, Result, state_root, unix_millis, valid_identifier};

const CHECKPOINT_SCHEMA: &str = "astrid_edge_hindsight_checkpoint_v2";
const HASH_SCOPE: &str = "exact_open_file_prefix_v1";
const MAX_FUTURE_SKEW_MS: u64 = 5 * 60 * 1_000;
const MAX_LATEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CHAIN_RECORD_BYTES: usize = 512 * 1024;
const MAX_CHAIN_RECORDS: usize = 100_000;
const ALLOWED_LEDGERS: &[&str] = &[
    "actions/dispatches.jsonl",
    "actions/receipts.jsonl",
    "actions/interrupted_corrections.jsonl",
    "autonomous/runs.jsonl",
    "autonomous/chains.jsonl",
    "autonomous/recoveries.jsonl",
    "autonomous/authorship_corrections.jsonl",
    "autonomous/thread_state.jsonl",
    "web/receipts.jsonl",
    "introspection/receipts.jsonl",
    "introspections/scheduled/receipts.jsonl",
    "introspection/scheduled/receipts.jsonl",
    "perception/observations.jsonl",
    "studies/receipts.jsonl",
    "spectral/rollups.jsonl",
    "spectral/receipts.jsonl",
    "tuning/receipts.jsonl",
    "research/duplication_notices.jsonl",
    "peer/receipts.jsonl",
    "runtime/fill_history.jsonl",
    "self-change/ledgers/candidate.jsonl",
    "self-change/ledgers/build.jsonl",
    "self-change/ledgers/activation.jsonl",
    "self-change/ledgers/operator.jsonl",
];
const REQUIRED_LEDGERS: &[&str] = &[
    "actions/interrupted_corrections.jsonl",
    "actions/receipts.jsonl",
    "autonomous/authorship_corrections.jsonl",
    "autonomous/chains.jsonl",
    "autonomous/recoveries.jsonl",
    "autonomous/runs.jsonl",
    "autonomous/thread_state.jsonl",
    "web/receipts.jsonl",
    "introspection/receipts.jsonl",
    "introspections/scheduled/receipts.jsonl",
    "introspection/scheduled/receipts.jsonl",
    "perception/observations.jsonl",
    "studies/receipts.jsonl",
    "runtime/fill_history.jsonl",
    "self-change/ledgers/candidate.jsonl",
    "self-change/ledgers/build.jsonl",
    "self-change/ledgers/activation.jsonl",
    "self-change/ledgers/operator.jsonl",
];

const SELF_CHANGE_LEDGERS: &[&str] = &[
    "self-change/ledgers/candidate.jsonl",
    "self-change/ledgers/build.jsonl",
    "self-change/ledgers/activation.jsonl",
    "self-change/ledgers/operator.jsonl",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HindsightAttestation {
    pub schema: String,
    pub checked_at_unix_ms: u64,
    pub generation_id: String,
    pub host_boot_id: String,
    pub checkpoint_recorded_at_unix_ms: u64,
    pub checkpoint_age_seconds: u64,
    pub continuity_epoch: String,
    pub checkpoint_record_sha256: String,
    pub checkpoint_chain_records: usize,
    pub ledger_prefixes_verified: usize,
    pub ledger_prefix_bytes_verified: u64,
    pub operator_database_quick_check: String,
    pub operator_database_schema_version: u64,
    pub operator_database_sha256: String,
    pub authority: String,
    pub evidence_sha256: String,
}

pub fn attest(
    workspace: &Path,
    generation_id: &str,
    maximum_age_seconds: u64,
) -> Result<HindsightAttestation> {
    attest_with_supervisor_owner(workspace, generation_id, maximum_age_seconds, 0)
}

fn attest_with_supervisor_owner(
    workspace: &Path,
    generation_id: &str,
    maximum_age_seconds: u64,
    supervisor_owner: u32,
) -> Result<HindsightAttestation> {
    if !valid_identifier(generation_id) || !(30..=1_800).contains(&maximum_age_seconds) {
        return Err(Error::new(
            "checkpoint identity or freshness bound is invalid",
        ));
    }
    let workspace = workspace.canonicalize()?;
    let root = state_root(&workspace)?;
    let operator_root = root.join("operator/hindsight");
    let latest_path = operator_root.join("latest.json");
    let operator_owner = fs::symlink_metadata(&operator_root)?.uid();
    require_owner_only_file(&latest_path, operator_owner, "hindsight latest checkpoint")?;
    let latest: Value = read_json(&latest_path, MAX_LATEST_BYTES)?;
    let object = latest
        .as_object()
        .ok_or_else(|| Error::new("hindsight latest checkpoint is not an object"))?;
    require_checkpoint_fields(object, &workspace, maximum_age_seconds)?;
    let host_boot_id = current_boot_id()?;
    if let Some(checkpoint_boot) = object.get("host_boot_id").and_then(Value::as_str) {
        if checkpoint_boot != host_boot_id {
            return Err(Error::new("hindsight checkpoint belongs to another boot"));
        }
    } else if integer(object, "recorded_at_unix_ms")?
        < current_boot_unix_ms()?.saturating_sub(1_000)
    {
        return Err(Error::new(
            "legacy hindsight checkpoint predates the current kernel boot",
        ));
    }
    let chain = verify_checkpoint_chain(&operator_root.join("checkpoints.jsonl"), operator_owner)?;
    let claimed_head = string(object, "checkpoint_record_sha256")?;
    if chain.head != claimed_head {
        return Err(Error::new(
            "hindsight latest does not match checkpoint chain head",
        ));
    }
    compare_latest_to_chain(object, &chain.last_record)?;
    let (ledger_count, ledger_bytes) =
        verify_ledger_prefixes(&workspace, object, supervisor_owner)?;
    let database = verify_operator_database(&operator_root, object)?;
    let checked_at = unix_millis();
    let recorded_at = integer(object, "recorded_at_unix_ms")?;
    let mut attestation = HindsightAttestation {
        schema: "astrid.edge_checkpoint.hindsight_attestation.v1".to_owned(),
        checked_at_unix_ms: checked_at,
        generation_id: generation_id.to_owned(),
        host_boot_id,
        checkpoint_recorded_at_unix_ms: recorded_at,
        checkpoint_age_seconds: checked_at.saturating_sub(recorded_at) / 1_000,
        continuity_epoch: string(object, "continuity_epoch")?,
        checkpoint_record_sha256: claimed_head,
        checkpoint_chain_records: chain.records,
        ledger_prefixes_verified: ledger_count,
        ledger_prefix_bytes_verified: ledger_bytes,
        operator_database_quick_check: "ok".to_owned(),
        operator_database_schema_version: database.schema_version,
        operator_database_sha256: database.identity_sha256,
        authority: AUTHORITY.to_owned(),
        evidence_sha256: String::new(),
    };
    attestation.evidence_sha256 = attestation_digest(&attestation)?;
    Ok(attestation)
}

pub fn record(
    workspace: &Path,
    output: &Path,
    generation_id: &str,
    reason: &str,
    maximum_age_seconds: u64,
) -> Result<()> {
    if !valid_identifier(reason) {
        return Err(Error::new("checkpoint reason is invalid"));
    }
    require_root_output_parent(output)?;
    let attestation = attest(workspace, generation_id, maximum_age_seconds)?;
    let value = serde_json::json!({
        "schema": "astrid.edge_checkpoint.root_record.v1",
        "reason": reason,
        "attestation": attestation,
        "authority": AUTHORITY,
    });
    atomic_write(output, &canonical_json(&value)?, 0o400, true)
}

pub fn print_attestation(
    workspace: &Path,
    generation_id: &str,
    maximum_age_seconds: u64,
) -> Result<()> {
    let bytes = canonical_json(&attest(workspace, generation_id, maximum_age_seconds)?)?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), b"\n").map_err(Into::into)
}

fn require_checkpoint_fields(
    object: &Map<String, Value>,
    workspace: &Path,
    maximum_age_seconds: u64,
) -> Result<()> {
    if string(object, "schema")? != CHECKPOINT_SCHEMA
        || string(object, "continuity_status")? != "verified"
        || object
            .get("continuity_from_previous_checkpoint_valid")
            .and_then(Value::as_bool)
            != Some(true)
        || integer(object, "current_epoch_integrity_violation_count")? != 0
        || object
            .get("pending_tail_observation_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            != 0
    {
        return Err(Error::new(
            "hindsight current-epoch continuity is not valid",
        ));
    }
    let claimed_workspace = Path::new(&string(object, "workspace")?).canonicalize()?;
    if claimed_workspace != workspace {
        return Err(Error::new(
            "hindsight checkpoint workspace differs from configured workspace",
        ));
    }
    let epoch = string(object, "continuity_epoch")?;
    if epoch.is_empty() || epoch.len() > 128 {
        return Err(Error::new("hindsight continuity epoch is invalid"));
    }
    let now = unix_millis();
    let recorded = integer(object, "recorded_at_unix_ms")?;
    if recorded == 0
        || recorded > now.saturating_add(MAX_FUTURE_SKEW_MS)
        || now.saturating_sub(recorded) > maximum_age_seconds.saturating_mul(1_000)
    {
        return Err(Error::new("hindsight checkpoint is stale or future-dated"));
    }
    Ok(())
}

struct ChainEvidence {
    head: String,
    records: usize,
    last_record: Map<String, Value>,
}

fn verify_checkpoint_chain(path: &Path, owner: u32) -> Result<ChainEvidence> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != owner
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "hindsight checkpoint chain ownership or mode failed",
        ));
    }
    let file = File::open(path)?;
    let opened = file.metadata()?;
    let mut reader = BufReader::new(file);
    let mut previous: Option<String> = None;
    let mut records = 0_usize;
    let mut last = None;
    loop {
        let mut line = Vec::new();
        let read = reader
            .by_ref()
            .take(
                u64::try_from(MAX_CHAIN_RECORD_BYTES)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        if line.len() > MAX_CHAIN_RECORD_BYTES || line.last() != Some(&b'\n') {
            return Err(Error::new(
                "hindsight checkpoint chain record is oversized or partial",
            ));
        }
        line.pop();
        let mut record: Map<String, Value> = serde_json::from_slice::<Value>(&line)?
            .as_object()
            .cloned()
            .ok_or_else(|| Error::new("hindsight checkpoint chain record is not an object"))?;
        let claimed = record
            .remove("record_sha256")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| Error::new("hindsight checkpoint record hash is absent"))?;
        if record.get("previous_record_sha256")
            != Some(&previous.clone().map_or(Value::Null, Value::String))
            || sha256(&canonical_json(&record)?) != claimed
        {
            return Err(Error::new("hindsight checkpoint hash chain is invalid"));
        }
        record.insert("record_sha256".to_owned(), Value::String(claimed.clone()));
        previous = Some(claimed);
        last = Some(record);
        records = records.saturating_add(1);
        if records > MAX_CHAIN_RECORDS {
            return Err(Error::new(
                "hindsight checkpoint chain exceeds record bound",
            ));
        }
    }
    let after = reader.into_inner().metadata()?;
    if file_identity(&metadata) != file_identity(&opened)
        || file_identity(&opened) != file_identity(&after)
    {
        return Err(Error::new(
            "hindsight checkpoint chain changed while verified",
        ));
    }
    Ok(ChainEvidence {
        head: previous.ok_or_else(|| Error::new("hindsight checkpoint chain is empty"))?,
        records,
        last_record: last.ok_or_else(|| Error::new("hindsight checkpoint chain is empty"))?,
    })
}

fn require_owner_only_file(path: &Path, owner: u32, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != owner
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(format!("{label} ownership or mode failed")));
    }
    Ok(())
}

fn compare_latest_to_chain(latest: &Map<String, Value>, chain: &Map<String, Value>) -> Result<()> {
    for (key, value) in chain {
        if matches!(key.as_str(), "previous_record_sha256" | "record_sha256") {
            continue;
        }
        if latest.get(key) != Some(value) {
            return Err(Error::new(
                "hindsight latest differs from its checkpoint chain record",
            ));
        }
    }
    Ok(())
}

fn verify_ledger_prefixes(
    workspace: &Path,
    checkpoint: &Map<String, Value>,
    supervisor_owner: u32,
) -> Result<(usize, u64)> {
    let ledgers = checkpoint
        .get("ledgers")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("hindsight checkpoint ledger inventory is absent"))?;
    let names = ledgers.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let allowed = ALLOWED_LEDGERS.iter().copied().collect::<BTreeSet<_>>();
    let required = REQUIRED_LEDGERS.iter().copied().collect::<BTreeSet<_>>();
    if !names.is_subset(&allowed) || !required.is_subset(&names) {
        return Err(Error::new(
            "hindsight checkpoint ledger inventory differs from immutable policy",
        ));
    }
    let workspace_owner = fs::symlink_metadata(workspace)?.uid();
    let mut count = 0_usize;
    let mut bytes = 0_u64;
    for relative in names {
        let summary = ledgers
            .get(relative)
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new("hindsight ledger summary is malformed"))?;
        let (path, expected_owner) =
            ledger_path_and_owner(workspace, relative, workspace_owner, supervisor_owner)?;
        let present = summary.get("present").and_then(Value::as_bool) == Some(true);
        if !present {
            if path.exists() || path.is_symlink() {
                return Err(Error::new("hindsight claims an existing ledger is absent"));
            }
            continue;
        }
        if string(summary, "hash_scope")? != HASH_SCOPE
            || integer(summary, "invalid_json_lines")? != 0
            || integer(summary, "trailing_partial_bytes")? != 0
            || summary
                .get("snapshot_unread_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                != 0
        {
            return Err(Error::new(
                "hindsight ledger prefix has syntax or read failures",
            ));
        }
        let expected_size = integer(summary, "size_bytes")?;
        let expected_inode = integer(summary, "inode")?;
        let expected_hash = string(summary, "sha256")?;
        if expected_hash.len() != 64
            || !expected_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(Error::new("hindsight ledger prefix digest is malformed"));
        }
        verify_file_prefix(
            &path,
            expected_size,
            expected_inode,
            &expected_hash,
            expected_owner,
        )?;
        count = count.saturating_add(1);
        bytes = bytes.saturating_add(expected_size);
    }
    if !ledgers
        .get("runtime/fill_history.jsonl")
        .and_then(Value::as_object)
        .and_then(|value| value.get("present"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(Error::new(
            "fill history is absent from immutable hindsight evidence",
        ));
    }
    Ok((count, bytes))
}

fn verify_file_prefix(
    path: &Path,
    size: u64,
    inode: u64,
    expected_hash: &str,
    owner: u32,
) -> Result<()> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.uid() != owner
        || before.mode() & 0o077 != 0
        || before.ino() != inode
        || before.len() < size
    {
        return Err(Error::new("checkpointed ledger identity or mode failed"));
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    let mut remaining = size;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(Error::new("checkpointed ledger prefix ended early"));
        }
        hash.update(&buffer[..read]);
        remaining = remaining.saturating_sub(u64::try_from(read).unwrap_or(u64::MAX));
    }
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if before.dev() != opened.dev()
        || before.ino() != opened.ino()
        || opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || !path_after.is_file()
        || path_after.file_type().is_symlink()
        || path_after.nlink() != 1
        || path_after.uid() != owner
        || path_after.mode() & 0o077 != 0
        || path_after.dev() != opened.dev()
        || path_after.ino() != opened.ino()
        || format!("{:x}", hash.finalize()) != expected_hash
    {
        return Err(Error::new(
            "checkpointed ledger prefix changed or hash failed",
        ));
    }
    Ok(())
}

struct DatabaseEvidence {
    schema_version: u64,
    identity_sha256: String,
}

fn verify_operator_database(
    operator_root: &Path,
    checkpoint: &Map<String, Value>,
) -> Result<DatabaseEvidence> {
    let path = operator_root.join("hindsight.sqlite3");
    let before = fs::symlink_metadata(&path)?;
    let operator_owner = fs::symlink_metadata(operator_root)?.uid();
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.uid() != operator_owner
        || before.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "operator hindsight database identity or mode failed",
        ));
    }
    let claimed = checkpoint
        .get("operator_hindsight_database")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("operator hindsight database claim is absent"))?;
    if string(claimed, "quick_check")? != "ok"
        || claimed.get("owner_only").and_then(Value::as_bool) != Some(true)
        || Path::new(&string(claimed, "path")?).canonicalize()? != path.canonicalize()?
    {
        return Err(Error::new("operator hindsight database claim is invalid"));
    }
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| Error::new(format!("cannot open hindsight database read-only: {error}")))?;
    let quick_check: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| Error::new(format!("hindsight database quick_check failed: {error}")))?;
    if quick_check != "ok" {
        return Err(Error::new("hindsight database quick_check is not ok"));
    }
    let schema_text: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| Error::new(format!("hindsight metadata is unavailable: {error}")))?;
    let schema_version = schema_text
        .parse::<u64>()
        .map_err(|_| Error::new("hindsight database schema version is malformed"))?;
    if !(2..=16).contains(&schema_version) || integer(claimed, "schema_version")? != schema_version
    {
        return Err(Error::new(
            "hindsight database schema differs from checkpoint",
        ));
    }
    drop(connection);
    let after = fs::symlink_metadata(&path)?;
    if file_identity(&before) != file_identity(&after) {
        return Err(Error::new(
            "hindsight database changed while independently checked",
        ));
    }
    let identity = serde_json::json!({
        "dev": before.dev(),
        "inode": before.ino(),
        "size": before.len(),
        "mtime": before.mtime(),
        "mtime_nsec": before.mtime_nsec(),
        "schema_version": schema_version,
        "quick_check": quick_check,
    });
    Ok(DatabaseEvidence {
        schema_version,
        identity_sha256: sha256(&canonical_json(&identity)?),
    })
}

fn safe_ledger_path(workspace: &Path, relative: &str) -> Result<std::path::PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(Error::new("hindsight ledger path escapes workspace"));
    }
    let path = workspace.join(relative_path);
    let mut cursor = workspace.to_path_buf();
    for component in relative_path.components() {
        cursor.push(component);
        if cursor.exists() && fs::symlink_metadata(&cursor)?.file_type().is_symlink() {
            return Err(Error::new("hindsight ledger path traverses a symlink"));
        }
    }
    Ok(path)
}

fn ledger_path_and_owner(
    workspace: &Path,
    relative: &str,
    workspace_owner: u32,
    supervisor_owner: u32,
) -> Result<(std::path::PathBuf, u32)> {
    if SELF_CHANGE_LEDGERS.contains(&relative) {
        let self_change_root = state_root(workspace)?.join("self-change");
        require_trusted_directory(
            &self_change_root,
            supervisor_owner,
            "self-change state root",
        )?;
        let ledger_root = self_change_root.join("ledgers");
        if ledger_root.exists() || ledger_root.is_symlink() {
            require_trusted_directory(&ledger_root, supervisor_owner, "self-change ledger root")?;
        }
        let basename = relative
            .strip_prefix("self-change/ledgers/")
            .ok_or_else(|| Error::new("self-change ledger key is malformed"))?;
        return Ok((ledger_root.join(basename), supervisor_owner));
    }
    if relative.starts_with("self-change/") {
        return Err(Error::new("self-change ledger key is not allowlisted"));
    }
    Ok((safe_ledger_path(workspace, relative)?, workspace_owner))
}

fn require_trusted_directory(path: &Path, owner: u32, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::new(format!(
            "{label} ownership, mode, or identity failed"
        )));
    }
    Ok(())
}

fn current_boot_id() -> Result<String> {
    let value = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    let value = value.trim();
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) != (byte == b'-')
                || (!matches!(index, 8 | 13 | 18 | 23) && !byte.is_ascii_hexdigit())
        })
    {
        return Err(Error::new("kernel boot identity is malformed"));
    }
    Ok(value.to_ascii_lowercase())
}

fn current_boot_unix_ms() -> Result<u64> {
    let text = fs::read_to_string("/proc/stat")?;
    let seconds = text
        .lines()
        .find_map(|line| line.strip_prefix("btime "))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .ok_or_else(|| Error::new("kernel boot time is unavailable"))?;
    Ok(seconds.saturating_mul(1_000))
}

fn require_root_output_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("checkpoint output has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new(
            "checkpoint output parent is not private root state",
        ));
    }
    Ok(())
}

fn string(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4_096)
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("checkpoint field is missing or malformed: {key}")))
}

fn integer(object: &Map<String, Value>, key: &str) -> Result<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("checkpoint integer is missing: {key}")))
}

fn file_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn attestation_digest(attestation: &HindsightAttestation) -> Result<String> {
    let mut value = serde_json::to_value(attestation)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("attestation serialization failed"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

#[cfg(test)]
mod tests {
    use super::{
        HindsightAttestation, REQUIRED_LEDGERS, SELF_CHANGE_LEDGERS, attest_with_supervisor_owner,
        attestation_digest, current_boot_id, ledger_path_and_owner, safe_ledger_path,
    };
    use astrid_edge_rescue_helper::fs_guard::{canonical_json, sha256};
    use rusqlite::Connection;
    use serde_json::{Value, json};
    use std::fs;
    use std::io::Write as _;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    #[test]
    fn attestation_digest_excludes_only_its_own_digest() {
        let mut value = HindsightAttestation {
            schema: "astrid.edge_checkpoint.hindsight_attestation.v1".into(),
            checked_at_unix_ms: 1,
            generation_id: "gen-a".into(),
            host_boot_id: "00000000-0000-4000-8000-000000000001".into(),
            checkpoint_recorded_at_unix_ms: 1,
            checkpoint_age_seconds: 0,
            continuity_epoch: "epoch".into(),
            checkpoint_record_sha256: "a".repeat(64),
            checkpoint_chain_records: 1,
            ledger_prefixes_verified: 1,
            ledger_prefix_bytes_verified: 1,
            operator_database_quick_check: "ok".into(),
            operator_database_schema_version: 4,
            operator_database_sha256: "b".repeat(64),
            authority: "immutable".into(),
            evidence_sha256: String::new(),
        };
        let digest = attestation_digest(&value).unwrap();
        value.evidence_sha256 = "c".repeat(64);
        assert_eq!(attestation_digest(&value).unwrap(), digest);
        value.generation_id = "gen-b".into();
        assert_ne!(attestation_digest(&value).unwrap(), digest);
    }

    #[test]
    fn ledger_paths_cannot_traverse_or_use_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("safe")).unwrap();
        assert!(safe_ledger_path(temp.path(), "safe/ledger.jsonl").is_ok());
        assert!(safe_ledger_path(temp.path(), "../escape").is_err());
        std::os::unix::fs::symlink("/tmp", temp.path().join("link")).unwrap();
        assert!(safe_ledger_path(temp.path(), "link/ledger.jsonl").is_err());
    }

    #[test]
    fn self_change_ledger_keys_resolve_only_to_the_trusted_state_root() {
        let temp = tempfile::tempdir().unwrap();
        let owner = fs::symlink_metadata(temp.path()).unwrap().uid();
        let state = temp.path().join("state");
        let workspace = state.join("home/default/edge");
        let ledger_root = state.join("self-change/ledgers");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&ledger_root).unwrap();
        let canonical_state = state.canonicalize().unwrap();

        for relative in SELF_CHANGE_LEDGERS {
            let (path, expected_owner) =
                ledger_path_and_owner(&workspace, relative, owner, owner).unwrap();
            assert_eq!(path, canonical_state.join(relative));
            assert_eq!(expected_owner, owner);
        }
        assert!(
            ledger_path_and_owner(
                &workspace,
                "self-change/ledgers/unexpected.jsonl",
                owner,
                owner,
            )
            .is_err()
        );

        fs::remove_dir(&ledger_root).unwrap();
        std::os::unix::fs::symlink("/tmp", &ledger_root).unwrap();
        assert!(
            ledger_path_and_owner(
                &workspace,
                "self-change/ledgers/candidate.jsonl",
                owner,
                owner,
            )
            .is_err()
        );
    }

    #[test]
    fn self_change_roots_require_the_configured_owner_and_safe_modes() {
        let temp = tempfile::tempdir().unwrap();
        let owner = fs::symlink_metadata(temp.path()).unwrap().uid();
        let state = temp.path().join("state");
        let workspace = state.join("home/default/edge");
        let self_change_root = state.join("self-change");
        fs::create_dir_all(self_change_root.join("ledgers")).unwrap();
        fs::create_dir_all(&workspace).unwrap();

        assert!(
            ledger_path_and_owner(
                &workspace,
                "self-change/ledgers/candidate.jsonl",
                owner,
                owner.saturating_add(1),
            )
            .is_err()
        );
        fs::set_permissions(&self_change_root, fs::Permissions::from_mode(0o772)).unwrap();
        assert!(
            ledger_path_and_owner(
                &workspace,
                "self-change/ledgers/candidate.jsonl",
                owner,
                owner,
            )
            .is_err()
        );
    }

    #[test]
    fn exact_checkpoint_accepts_append_only_growth_but_rejects_prefix_mutation() {
        let Ok(boot_id) = current_boot_id() else {
            return;
        };
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        let workspace = state.join("home/default/edge");
        let operator = state.join("operator/hindsight");
        let self_change_ledger_root = state.join("self-change/ledgers");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&operator).unwrap();
        fs::create_dir_all(&self_change_ledger_root).unwrap();
        let owner = fs::symlink_metadata(temp.path()).unwrap().uid();
        let mut summaries = serde_json::Map::new();
        for relative in REQUIRED_LEDGERS {
            let path = fixture_ledger_path(&state, &workspace, relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let bytes = fixture_ledger_bytes(relative);
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let metadata = fs::symlink_metadata(&path).unwrap();
            summaries.insert(
                (*relative).to_owned(),
                json!({
                    "present": true,
                    "hash_scope": "exact_open_file_prefix_v1",
                    "inode": metadata.ino(),
                    "size_bytes": metadata.len(),
                    "sha256": sha256(bytes),
                    "invalid_json_lines": 0,
                    "trailing_partial_bytes": 0
                }),
            );
        }
        let database = operator.join("hindsight.sqlite3");
        let connection = Connection::open(&database).unwrap();
        connection
            .execute(
                "CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO metadata(key,value) VALUES('schema_version','2')",
                [],
            )
            .unwrap();
        drop(connection);
        fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
        let now = super::unix_millis();
        let mut record = json!({
            "schema": "astrid_edge_hindsight_checkpoint_v2",
            "recorded_at_unix_ms": now,
            "host_boot_id": boot_id,
            "workspace": workspace.canonicalize().unwrap(),
            "continuity_status": "verified",
            "continuity_from_previous_checkpoint_valid": true,
            "current_epoch_integrity_violation_count": 0,
            "pending_tail_observation_count": 0,
            "continuity_epoch": "test-epoch",
            "ledgers": Value::Object(summaries),
            "operator_hindsight_database": {
                "quick_check": "ok",
                "owner_only": true,
                "path": database.canonicalize().unwrap(),
                "schema_version": 2
            },
            "previous_record_sha256": null
        });
        let record_hash = sha256(&canonical_json(&record).unwrap());
        record["record_sha256"] = Value::String(record_hash.clone());
        write_private(
            &operator.join("checkpoints.jsonl"),
            &[canonical_json(&record).unwrap(), b"\n".to_vec()].concat(),
        );
        let mut latest = record;
        latest["checkpoint_record_sha256"] = Value::String(record_hash);
        write_private(
            &operator.join("latest.json"),
            &canonical_json(&latest).unwrap(),
        );

        assert!(attest_with_supervisor_owner(&workspace, "gen-test", 1_800, owner).is_ok());
        // Appending bytes after the captured prefix is valid append-only
        // advancement and intentionally ignored until the next checkpoint.
        append_bytes(&workspace.join("web/receipts.jsonl"), b"{\"later\":true}\n");
        assert!(attest_with_supervisor_owner(&workspace, "gen-test", 1_800, owner).is_ok());
        append_bytes(
            &state.join("self-change/ledgers/candidate.jsonl"),
            b"{\"later\":true}\n",
        );
        assert!(attest_with_supervisor_owner(&workspace, "gen-test", 1_800, owner).is_ok());
        // Replacing bytes inside a captured root-owned prefix is never tolerated.
        fs::write(state.join("self-change/ledgers/candidate.jsonl"), b"[]\n").unwrap();
        assert!(attest_with_supervisor_owner(&workspace, "gen-test", 1_800, owner).is_err());
    }

    fn write_private(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn append_bytes(path: &Path, bytes: &[u8]) {
        fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }

    fn fixture_ledger_path(state: &Path, workspace: &Path, relative: &str) -> std::path::PathBuf {
        if SELF_CHANGE_LEDGERS.contains(&relative) {
            state.join(relative)
        } else {
            workspace.join(relative)
        }
    }

    fn fixture_ledger_bytes(relative: &str) -> &'static [u8] {
        if matches!(
            relative,
            "actions/receipts.jsonl" | "self-change/ledgers/candidate.jsonl"
        ) {
            b"{}\n"
        } else {
            b""
        }
    }
}
