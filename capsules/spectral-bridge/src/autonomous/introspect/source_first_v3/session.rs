use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt as _;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::catalog::{CatalogedSourceV3, sha256_bytes};
use super::{ReadSessionCheckpointV3, SourceCoverageStateV3, SourceIntervalV3, SourceMapV3};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceSessionIndexV3 {
    schema: String,
    source_identity: String,
    active_source_sha256: String,
    active_session_id: String,
    invalidated_session_ids: Vec<String>,
}

pub(super) fn update_read_session_v3(
    root: &Path,
    cataloged: &CatalogedSourceV3,
    source_map: &SourceMapV3,
    start: usize,
    end: usize,
    total: usize,
) -> Result<ReadSessionCheckpointV3, String> {
    if total != cataloged.source_lines {
        return Err(format!(
            "source line count changed during read: cataloged={} supplied={total}",
            cataloged.source_lines
        ));
    }
    if start >= end || end > total {
        return Err(format!(
            "invalid source interval {start}..{end} for {total} lines"
        ));
    }
    ensure_owner_dir(root)?;
    let lock_path = root.join(".session.lock");
    let lock = open_owner_file(&lock_path)?;
    lock.lock_exclusive()
        .map_err(|error| format!("lock source-first V3 session store: {error}"))?;
    let result = update_locked(root, cataloged, source_map, start, end, total);
    let unlock_result = fs2::FileExt::unlock(&lock)
        .map_err(|error| format!("unlock source-first V3 session store: {error}"));
    let checkpoint = result?;
    unlock_result?;
    Ok(checkpoint)
}

fn update_locked(
    root: &Path,
    cataloged: &CatalogedSourceV3,
    source_map: &SourceMapV3,
    start: usize,
    end: usize,
    total: usize,
) -> Result<ReadSessionCheckpointV3, String> {
    let session_id = sha256_bytes(
        format!(
            "read_session_checkpoint_v3\0{}\0{}",
            cataloged.source_identity, cataloged.source_sha256
        )
        .as_bytes(),
    );
    let identity_id = sha256_bytes(cataloged.source_identity.as_bytes());
    let index_path = root.join("sources").join(format!("{identity_id}.json"));
    let checkpoint_path = checkpoint_path(root, &session_id);
    let previous_index = load_optional::<SourceSessionIndexV3>(&index_path)?;
    let changed = previous_index
        .as_ref()
        .is_some_and(|index| index.active_source_sha256 != cataloged.source_sha256);
    let supersedes_session_id = previous_index
        .as_ref()
        .filter(|_| changed)
        .map(|index| index.active_session_id.clone());

    if let Some(previous_id) = supersedes_session_id.as_deref() {
        invalidate_checkpoint(root, previous_id, &session_id)?;
    }

    let mut intervals = load_optional::<ReadSessionCheckpointV3>(&checkpoint_path)?
        .map(|checkpoint| {
            checkpoint
                .included_intervals
                .into_iter()
                .map(|interval| (interval.start_line.saturating_sub(1), interval.end_line))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let previous_read_count = load_optional::<ReadSessionCheckpointV3>(&checkpoint_path)?
        .map_or(0, |checkpoint| checkpoint.read_count);
    intervals.push((start, end));
    let included = merged_intervals(&intervals);
    let uncovered = uncovered_intervals(&included, total);
    let read_count = previous_read_count.saturating_add(1);
    let coverage_state = if uncovered.is_empty() {
        if read_count > 1 {
            SourceCoverageStateV3::MultiWindowComplete
        } else {
            SourceCoverageStateV3::CompleteFile
        }
    } else {
        SourceCoverageStateV3::Partial
    };
    let checkpoint = ReadSessionCheckpointV3 {
        schema: "read_session_checkpoint_v3".to_string(),
        schema_version: 3,
        read_session_id: session_id.clone(),
        source_identity: cataloged.source_identity.clone(),
        source_sha256: cataloged.source_sha256.clone(),
        structural_map_sha256: source_map.structural_map_sha256.clone(),
        parser: source_map.parser.clone(),
        included_intervals: included
            .iter()
            .map(|(interval_start, interval_end)| {
                SourceIntervalV3::from_zero_based(*interval_start, *interval_end)
            })
            .collect(),
        uncovered_intervals: uncovered
            .iter()
            .map(|(interval_start, interval_end)| {
                SourceIntervalV3::from_zero_based(*interval_start, *interval_end)
            })
            .collect(),
        coverage_state,
        read_count,
        persists_across_process_restart: true,
        active_for_source_identity: true,
        supersedes_session_id,
        invalidated_by_session_id: None,
        source_changed_since_previous: changed,
        updated_at_unix_ms: now_unix_ms(),
        artifact_authority: "read_evidence_not_control_approval_or_activation".to_string(),
    };

    let map_id = sha256_bytes(
        format!(
            "{}\0{}\0{}",
            cataloged.source_identity, cataloged.source_sha256, source_map.structural_map_sha256
        )
        .as_bytes(),
    );
    write_owner_json(
        &root.join("maps").join(format!("{map_id}.json")),
        source_map,
    )?;
    write_owner_json(&checkpoint_path, &checkpoint)?;

    let mut invalidated = previous_index
        .map(|index| index.invalidated_session_ids)
        .unwrap_or_default();
    if let Some(previous_id) = checkpoint.supersedes_session_id.as_ref()
        && !invalidated.contains(previous_id)
    {
        invalidated.push(previous_id.clone());
    }
    invalidated.sort();
    invalidated.dedup();
    write_owner_json(
        &index_path,
        &SourceSessionIndexV3 {
            schema: "source_session_index_v3".to_string(),
            source_identity: cataloged.source_identity.clone(),
            active_source_sha256: cataloged.source_sha256.clone(),
            active_session_id: session_id,
            invalidated_session_ids: invalidated,
        },
    )?;
    Ok(checkpoint)
}

fn invalidate_checkpoint(
    root: &Path,
    previous_session_id: &str,
    replacement_session_id: &str,
) -> Result<(), String> {
    let path = checkpoint_path(root, previous_session_id);
    let Some(mut previous) = load_optional::<ReadSessionCheckpointV3>(&path)? else {
        return Ok(());
    };
    previous.active_for_source_identity = false;
    previous.invalidated_by_session_id = Some(replacement_session_id.to_string());
    previous.updated_at_unix_ms = now_unix_ms();
    write_owner_json(&path, &previous)
}

fn checkpoint_path(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions").join(format!("{session_id}.json"))
}

fn merged_intervals(intervals: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted = intervals.to_vec();
    sorted.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in sorted {
        if start >= end {
            continue;
        }
        if let Some((_, previous_end)) = merged.last_mut()
            && start <= *previous_end
        {
            *previous_end = (*previous_end).max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn uncovered_intervals(included: &[(usize, usize)], total: usize) -> Vec<(usize, usize)> {
    let mut uncovered = Vec::new();
    let mut cursor = 0;
    for (start, end) in included {
        if cursor < *start {
            uncovered.push((cursor, *start));
        }
        cursor = cursor.max(*end);
    }
    if cursor < total {
        uncovered.push((cursor, total));
    }
    uncovered
}

fn load_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("parse {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {}: {error}", path.display())),
    }
}

fn write_owner_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize {}: {error}", path.display()))?;
    bytes.push(b'\n');
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    ensure_owner_dir(parent)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact.json");
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|error| format!("open {}: {error}", temporary.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {}: {error}", temporary.display()))?;
    drop(file);
    fs::rename(&temporary, path).map_err(|error| format!("replace {}: {error}", path.display()))?;
    set_owner_file(path)
}

fn open_owner_file(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    set_owner_file(path)?;
    Ok(file)
}

fn ensure_owner_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("create directory {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("set owner-only directory {}: {error}", path.display()))?;
    Ok(())
}

fn set_owner_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("set owner-only file {}: {error}", path.display()))?;
    Ok(())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}
