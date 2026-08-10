use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{Config, Error, Result};

const QUOTA_RECORD_SCHEMA: &str = "astrid.edge.web_search.quota.v1";
const EMPTY_HASH: &str = concat!(
    "0000000000000000",
    "0000000000000000",
    "0000000000000000",
    "0000000000000000"
);
const HOUR_MS: u64 = 60 * 60 * 1_000;
const UTC_DAY_MS: u64 = 24 * HOUR_MS;
const MAXIMUM_LEDGER_BYTES: u64 = 64 * 1024 * 1024;
const MAXIMUM_RECORD_BYTES: usize = 1_024;
const MAXIMUM_SEARCHES_PER_TRACE: usize = 2;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct QuotaRecordBody {
    schema: String,
    sequence: u64,
    client_id: String,
    admitted_at_unix_ms: u64,
    utc_day_index: u64,
    trace_id: String,
    request_sha256: String,
    previous_record_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct QuotaRecord {
    #[serde(flatten)]
    body: QuotaRecordBody,
    record_sha256: String,
}

struct QuotaState {
    file: File,
    recent: VecDeque<QuotaRecord>,
    next_sequence: u64,
    previous_hash: String,
    last_admitted_at_unix_ms: u64,
    ledger_bytes: u64,
    poisoned: bool,
}

/// A broker-owned, restart-persistent admission ledger for public search.
///
/// The ledger intentionally retains no query text. Its fixed per-client
/// budgets are part of the immutable broker configuration rather than mutable
/// runtime policy.
pub(crate) struct PersistentSearchQuota {
    client_id: String,
    maximum_per_hour: usize,
    maximum_per_utc_day: usize,
    state: Mutex<QuotaState>,
}

impl PersistentSearchQuota {
    /// Open and verify the exact broker-owned quota ledger.
    pub(crate) fn open(config: &Config) -> Result<Self> {
        Self::open_path(
            &config.quota_state_path,
            &config.client_id,
            usize::from(config.maximum_searches_per_hour),
            usize::from(config.maximum_searches_per_utc_day),
            unix_time_ms()?,
            true,
        )
    }

    fn open_path(
        path: &Path,
        client_id: &str,
        maximum_per_hour: usize,
        maximum_per_utc_day: usize,
        now_ms: u64,
        enforce_immutable_ancestors: bool,
    ) -> Result<Self> {
        validate_state_path(path, enforce_immutable_ancestors)?;
        let existed = path.exists();
        if existed {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::new("quota ledger path is a symlink"));
            }
        }
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)?;
        if !existed {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        validate_state_file(path, &file)?;
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0))?;
        Read::by_ref(&mut file)
            .take(MAXIMUM_LEDGER_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAXIMUM_LEDGER_BYTES {
            return Err(Error::new("quota ledger exceeds immutable size bound"));
        }
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            let complete_len = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |position| position.saturating_add(1));
            file.set_len(u64::try_from(complete_len).unwrap_or(0))?;
            file.sync_data()?;
            bytes.truncate(complete_len);
        }
        let records = parse_and_verify(&bytes, client_id, now_ms)?;
        let next_sequence = records
            .last()
            .map_or(1, |record| record.body.sequence.saturating_add(1));
        let previous_hash = records.last().map_or_else(
            || EMPTY_HASH.to_owned(),
            |record| record.record_sha256.clone(),
        );
        let last_admitted_at_unix_ms = records
            .last()
            .map_or(0, |record| record.body.admitted_at_unix_ms);
        let mut recent = records.into_iter().collect::<VecDeque<_>>();
        prune_recent(&mut recent, now_ms);
        Ok(Self {
            client_id: client_id.to_owned(),
            maximum_per_hour,
            maximum_per_utc_day,
            state: Mutex::new(QuotaState {
                file,
                recent,
                next_sequence,
                previous_hash,
                last_admitted_at_unix_ms,
                ledger_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                poisoned: false,
            }),
        })
    }

    /// Durably consume one search admission before any query reaches upstream.
    pub(crate) fn admit(&self, trace_id: &str, request_sha256: &str) -> Result<()> {
        self.admit_at(trace_id, request_sha256, unix_time_ms()?)
    }

    fn admit_at(&self, trace_id: &str, request_sha256: &str, now_ms: u64) -> Result<()> {
        if !canonical_trace_id(trace_id) || !lower_hex_64(request_sha256) {
            return Err(Error::new("quota admission identity is not canonical"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error::new("quota ledger lock is unavailable"))?;
        if state.poisoned {
            return Err(Error::new(
                "quota ledger is fail-closed after a persistence failure",
            ));
        }
        if state.last_admitted_at_unix_ms > now_ms {
            return Err(Error::new("quota ledger rejected a backward system clock"));
        }
        prune_recent(&mut state.recent, now_ms);
        let hour_floor = now_ms.saturating_sub(HOUR_MS);
        let utc_day_index = now_ms / UTC_DAY_MS;
        let hourly = state
            .recent
            .iter()
            .filter(|record| record.body.admitted_at_unix_ms > hour_floor)
            .count();
        let daily = state
            .recent
            .iter()
            .filter(|record| record.body.utc_day_index == utc_day_index)
            .count();
        let trace_uses = state
            .recent
            .iter()
            .filter(|record| record.body.trace_id == trace_id)
            .count();
        if hourly >= self.maximum_per_hour
            || daily >= self.maximum_per_utc_day
            || trace_uses >= MAXIMUM_SEARCHES_PER_TRACE
        {
            return Err(Error::new("immutable persisted search quota exceeded"));
        }
        if state
            .recent
            .iter()
            .any(|record| record.body.request_sha256 == request_sha256)
        {
            return Err(Error::new(
                "immutable persisted search quota rejected replay",
            ));
        }
        let body = QuotaRecordBody {
            schema: QUOTA_RECORD_SCHEMA.to_owned(),
            sequence: state.next_sequence,
            client_id: self.client_id.clone(),
            admitted_at_unix_ms: now_ms,
            utc_day_index,
            trace_id: trace_id.to_owned(),
            request_sha256: request_sha256.to_owned(),
            previous_record_sha256: state.previous_hash.clone(),
        };
        let record_sha256 = hash_body(&body)?;
        let record = QuotaRecord {
            body,
            record_sha256,
        };
        let mut line = serde_json::to_vec(&record)?;
        line.push(b'\n');
        if line.len() > MAXIMUM_RECORD_BYTES
            || state
                .ledger_bytes
                .saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX))
                > MAXIMUM_LEDGER_BYTES
        {
            return Err(Error::new("quota ledger reached immutable storage bound"));
        }
        if state.file.write_all(&line).is_err() || state.file.sync_data().is_err() {
            state.poisoned = true;
            return Err(Error::new("quota ledger persistence failed closed"));
        }
        state.ledger_bytes = state
            .ledger_bytes
            .saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX));
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.previous_hash.clone_from(&record.record_sha256);
        state.last_admitted_at_unix_ms = now_ms;
        state.recent.push_back(record);
        Ok(())
    }
}

fn parse_and_verify(bytes: &[u8], client_id: &str, now_ms: u64) -> Result<Vec<QuotaRecord>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    let mut expected_sequence = 1_u64;
    let mut previous_hash = EMPTY_HASH.to_owned();
    let mut previous_timestamp = 0_u64;
    let content = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if !bytes.is_empty() && content.is_empty() {
        return Err(Error::new("quota ledger contains an empty record"));
    }
    for raw in content.split(|byte| *byte == b'\n') {
        if raw.is_empty() || raw.len() > MAXIMUM_RECORD_BYTES {
            return Err(Error::new(
                "quota ledger record is outside immutable bounds",
            ));
        }
        let record: QuotaRecord = serde_json::from_slice(raw)?;
        if serde_json::to_vec(&record)? != raw
            || record.body.schema != QUOTA_RECORD_SCHEMA
            || record.body.client_id != client_id
            || record.body.sequence != expected_sequence
            || record.body.previous_record_sha256 != previous_hash
            || record.body.utc_day_index != record.body.admitted_at_unix_ms / UTC_DAY_MS
            || record.body.admitted_at_unix_ms < previous_timestamp
            || record.body.admitted_at_unix_ms > now_ms.saturating_add(5 * 60 * 1_000)
            || !canonical_trace_id(&record.body.trace_id)
            || !lower_hex_64(&record.body.request_sha256)
            || record.record_sha256 != hash_body(&record.body)?
        {
            return Err(Error::new("quota ledger continuity verification failed"));
        }
        expected_sequence = expected_sequence.saturating_add(1);
        previous_timestamp = record.body.admitted_at_unix_ms;
        previous_hash.clone_from(&record.record_sha256);
        records.push(record);
    }
    Ok(records)
}

fn prune_recent(records: &mut VecDeque<QuotaRecord>, now_ms: u64) {
    let oldest = now_ms.saturating_sub(UTC_DAY_MS);
    while records
        .front()
        .is_some_and(|record| record.body.admitted_at_unix_ms <= oldest)
    {
        let _ = records.pop_front();
    }
}

fn hash_body(body: &QuotaRecordBody) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(body)?)))
}

fn unix_time_ms() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new("system clock predates Unix epoch"))?;
    u64::try_from(duration.as_millis()).map_err(|_| Error::new("system clock is out of range"))
}

fn canonical_trace_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                matches!(byte, b'0'..=b'9' | b'a'..=b'f')
            }
        })
}

fn lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_state_path(path: &Path, enforce_immutable_ancestors: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("quota ledger path must be absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("quota ledger has no parent directory"))?;
    if enforce_immutable_ancestors {
        let mut cursor = PathBuf::new();
        for component in parent.components() {
            cursor.push(component);
            if cursor == Path::new("/") {
                continue;
            }
            let metadata = fs::symlink_metadata(&cursor)?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.permissions().mode() & 0o022 != 0
            {
                return Err(Error::new(
                    "quota ledger ancestors must be non-writable real directories",
                ));
            }
        }
    }
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(Error::new("quota ledger StateDirectory must be mode 0700"));
    }
    Ok(())
}

fn validate_state_file(path: &Path, file: &File) -> Result<()> {
    let path_metadata = fs::symlink_metadata(path)?;
    let metadata = file.metadata()?;
    let parent = fs::metadata(
        path.parent()
            .ok_or_else(|| Error::new("quota ledger has no parent directory"))?,
    )?;
    if !metadata.is_file()
        || path_metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.uid() != parent.uid()
        || metadata.gid() != parent.gid()
        || metadata.dev() != path_metadata.dev()
        || metadata.ino() != path_metadata.ino()
        || metadata.len() > MAXIMUM_LEDGER_BYTES
    {
        return Err(Error::new(
            "quota ledger must be owner-only, nlink-one, and StateDirectory-owned",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    use super::{EMPTY_HASH, PersistentSearchQuota, QUOTA_RECORD_SCHEMA};

    const TRACE_ONE: &str = "11111111-1111-4111-8111-111111111111";
    const TRACE_TWO: &str = "22222222-2222-4222-8222-222222222222";
    const DAY: u64 = 24 * 60 * 60 * 1_000;

    fn create_quota(path: &std::path::Path, now: u64) -> PersistentSearchQuota {
        PersistentSearchQuota::open_path(path, "edge-runtime", 2, 3, now, false).unwrap()
    }

    fn state_path(temporary: &tempfile::TempDir) -> std::path::PathBuf {
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
        temporary.path().join("search-quota.jsonl")
    }

    #[test]
    fn hourly_and_utc_day_budgets_survive_restart() {
        let temporary = tempfile::tempdir().unwrap();
        let path = state_path(&temporary);
        let now = DAY.saturating_mul(10).saturating_add(1_000);
        let quota = create_quota(&path, now);
        quota.admit_at(TRACE_ONE, &"a".repeat(64), now).unwrap();
        quota
            .admit_at(TRACE_TWO, &"b".repeat(64), now.saturating_add(1))
            .unwrap();
        assert!(quota.admit_at(TRACE_TWO, &"c".repeat(64), now + 2).is_err());
        drop(quota);

        let restarted = create_quota(&path, now + 3);
        assert!(
            restarted
                .admit_at(TRACE_ONE, &"d".repeat(64), now + 3)
                .is_err()
        );
        restarted
            .admit_at(TRACE_ONE, &"d".repeat(64), now + 60 * 60 * 1_000 + 1)
            .unwrap();
        assert!(
            restarted
                .admit_at(TRACE_TWO, &"e".repeat(64), now + 60 * 60 * 1_000 + 2,)
                .is_err()
        );
    }

    #[test]
    fn replay_and_per_trace_expansion_are_rejected() {
        let temporary = tempfile::tempdir().unwrap();
        let path = state_path(&temporary);
        let now = DAY.saturating_mul(20);
        let quota =
            PersistentSearchQuota::open_path(&path, "edge-runtime", 8, 24, now, false).unwrap();
        quota.admit_at(TRACE_ONE, &"a".repeat(64), now).unwrap();
        assert!(quota.admit_at(TRACE_TWO, &"a".repeat(64), now + 1).is_err());
        quota.admit_at(TRACE_ONE, &"b".repeat(64), now + 2).unwrap();
        assert!(quota.admit_at(TRACE_ONE, &"c".repeat(64), now + 3).is_err());
    }

    #[test]
    fn client_binding_tamper_and_cross_client_reuse_fail_closed() {
        let temporary = tempfile::tempdir().unwrap();
        let path = state_path(&temporary);
        let now = DAY.saturating_mul(30);
        let quota = create_quota(&path, now);
        quota.admit_at(TRACE_ONE, &"a".repeat(64), now).unwrap();
        drop(quota);
        assert!(
            PersistentSearchQuota::open_path(&path, "edge-steward", 2, 12, now + 1, false).is_err()
        );

        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains(QUOTA_RECORD_SCHEMA));
        assert!(!body.contains("echo state network"));
        fs::write(&path, body.replace("edge-runtime", "edge-runtimf")).unwrap();
        assert!(
            PersistentSearchQuota::open_path(&path, "edge-runtime", 2, 3, now + 1, false).is_err()
        );
    }

    #[test]
    fn partial_final_record_is_removed_but_full_corruption_is_not_healed() {
        let temporary = tempfile::tempdir().unwrap();
        let path = state_path(&temporary);
        let now = DAY.saturating_mul(40);
        let quota = create_quota(&path, now);
        quota.admit_at(TRACE_ONE, &"a".repeat(64), now).unwrap();
        drop(quota);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"{\"partial\":");
        fs::write(&path, bytes).unwrap();
        let recovered = create_quota(&path, now + 1);
        drop(recovered);
        assert!(fs::read(&path).unwrap().ends_with(b"\n"));

        let mut bytes = fs::read(&path).unwrap();
        bytes[10] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert!(
            PersistentSearchQuota::open_path(&path, "edge-runtime", 2, 3, now + 2, false).is_err()
        );
    }

    #[test]
    fn full_history_timestamp_prevents_backward_clock_after_recent_prune() {
        assert_eq!(EMPTY_HASH.len(), 64);
        let temporary = tempfile::tempdir().unwrap();
        let path = state_path(&temporary);
        let written_at = DAY.saturating_mul(50);
        let quota = create_quota(&path, written_at);
        quota
            .admit_at(TRACE_ONE, &"a".repeat(64), written_at)
            .unwrap();
        drop(quota);

        let restarted = PersistentSearchQuota::open_path(
            &path,
            "edge-runtime",
            8,
            24,
            written_at.saturating_add(DAY.saturating_mul(2)),
            false,
        )
        .unwrap();
        assert!(
            restarted
                .admit_at(TRACE_TWO, &"b".repeat(64), written_at.saturating_sub(1),)
                .is_err()
        );
    }
}
