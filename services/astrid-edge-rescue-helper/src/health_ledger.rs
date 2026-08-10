//! Exact-prefix verification and bounded parsing for mutable runtime health ledgers.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::health::RuntimeLedgerSnapshot;
use crate::probation::RuntimePrefixExpectation;
use crate::{Error, Result};

const FUTURE_SKEW_MS: u64 = 5 * 60 * 1_000;
const MAX_RUNTIME_LEDGER_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAXIMUM_TAIL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct FillSample {
    pub recorded_at_unix_ms: u64,
    pub fill: f64,
}

pub(crate) fn tail_fills(
    path: &Path,
    since: u64,
    until: u64,
    expected_uid: u32,
    expectation: Option<&RuntimePrefixExpectation>,
) -> Result<(Vec<FillSample>, RuntimeLedgerSnapshot)> {
    let (mut file, opened, captured_size) = open_runtime_ledger(path, expected_uid, expectation)?;
    let (prefix_sha256, prior_prefix_verified) =
        hash_captured_prefix(&mut file, captured_size, expectation)?;
    let (tail_start, bytes) =
        read_stable_tail(&mut file, path, &opened, captured_size, expected_uid)?;
    let fills = parse_fill_samples(&bytes, tail_start, since, until)?;
    let continuity_status = if expectation.is_some() {
        "append_only_prefix_verified"
    } else {
        "migration_baseline_no_prior_continuity_claim"
    };
    Ok((
        fills,
        RuntimeLedgerSnapshot {
            device: opened.dev(),
            inode: opened.ino(),
            captured_size,
            prefix_sha256,
            prior_prefix_verified,
            continuity_status,
            authority: "immutable_root_exact_open_file_prefix",
        },
    ))
}

fn open_runtime_ledger(
    path: &Path,
    expected_uid: u32,
    expectation: Option<&RuntimePrefixExpectation>,
) -> Result<(File, std::fs::Metadata, u64)> {
    let path_before = std::fs::symlink_metadata(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let opened = file.metadata()?;
    if !path_before.is_file()
        || path_before.file_type().is_symlink()
        || path_before.nlink() != 1
        || path_before.uid() != expected_uid
        || path_before.mode() & 0o077 != 0
        || path_before.dev() != opened.dev()
        || path_before.ino() != opened.ino()
    {
        return Err(Error::new(
            "fill history identity or owner-only mode failed",
        ));
    }
    let captured_size = opened.len();
    if captured_size > MAX_RUNTIME_LEDGER_BYTES {
        return Err(Error::new("fill history exceeds immutable hash bound"));
    }
    if expectation.is_some_and(|expected| {
        expected.device != opened.dev()
            || expected.inode != opened.ino()
            || expected.captured_size > captured_size
            || !crate::config::valid_hex64(&expected.prefix_sha256)
    }) {
        return Err(Error::new(
            "fill history identity, size, or prior digest regressed",
        ));
    }
    Ok((file, opened, captured_size))
}

fn read_stable_tail(
    file: &mut File,
    path: &Path,
    opened: &std::fs::Metadata,
    captured_size: u64,
    expected_uid: u32,
) -> Result<(u64, Vec<u8>)> {
    let start = captured_size.saturating_sub(MAXIMUM_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let captured_bytes = captured_size.saturating_sub(start);
    let mut bytes = Vec::with_capacity(usize::try_from(captured_bytes).unwrap_or(0));
    Read::by_ref(file)
        .take(captured_bytes)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = std::fs::symlink_metadata(path)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != captured_bytes
        || opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || after.len() < captured_size
        || !path_after.is_file()
        || path_after.file_type().is_symlink()
        || path_after.nlink() != 1
        || path_after.uid() != expected_uid
        || path_after.mode() & 0o077 != 0
        || path_after.dev() != opened.dev()
        || path_after.ino() != opened.ino()
    {
        return Err(Error::new(
            "fill history changed identity or truncated during exact tail read",
        ));
    }
    Ok((start, bytes))
}

fn parse_fill_samples(
    bytes: &[u8],
    tail_start: u64,
    since: u64,
    until: u64,
) -> Result<Vec<FillSample>> {
    let complete_length = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index.saturating_add(1));
    let text = String::from_utf8_lossy(&bytes[..complete_length]);
    let mut lines = text.lines();
    if tail_start > 0 {
        let _ = lines.next();
    }
    let mut fills = Vec::new();
    for line in lines {
        let value: Value = serde_json::from_str(line)
            .map_err(|_| Error::new("fill history contains malformed JSONL"))?;
        let timestamp = value
            .get("recorded_at_unix_ms")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("fill history record lacks timestamp"))?;
        if timestamp > until.saturating_add(FUTURE_SKEW_MS) {
            return Err(Error::new("fill history contains a future-dated record"));
        }
        if timestamp < since || timestamp > until {
            continue;
        }
        let fill = value
            .get("fill_ratio")
            .and_then(Value::as_f64)
            .filter(|fill| fill.is_finite() && (0.0..=1.0).contains(fill))
            .ok_or_else(|| Error::new("fill history contains invalid fill"))?;
        if fills
            .last()
            .is_some_and(|prior: &FillSample| prior.recorded_at_unix_ms >= timestamp)
        {
            return Err(Error::new(
                "fill history timestamps are not strictly increasing",
            ));
        }
        fills.push(FillSample {
            recorded_at_unix_ms: timestamp,
            fill,
        });
    }
    Ok(fills)
}

fn hash_captured_prefix(
    file: &mut File,
    captured_size: u64,
    expectation: Option<&RuntimePrefixExpectation>,
) -> Result<(String, bool)> {
    file.seek(SeekFrom::Start(0))?;
    let expected_size = expectation.map_or(0, |expected| expected.captured_size);
    let mut full_hasher = Sha256::new();
    let mut prior_hasher = Sha256::new();
    let mut consumed = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    let buffer_len = u64::try_from(buffer.len()).unwrap_or(u64::MAX);
    while consumed < captured_size {
        let remaining = captured_size.saturating_sub(consumed);
        let count = usize::try_from(remaining.min(buffer_len))
            .map_err(|_| Error::new("fill history chunk length overflow"))?;
        file.read_exact(&mut buffer[..count])?;
        full_hasher.update(&buffer[..count]);
        if consumed < expected_size {
            let prior_remaining = expected_size.saturating_sub(consumed);
            let prior_count = count.min(
                usize::try_from(prior_remaining)
                    .map_err(|_| Error::new("prior fill prefix length overflow"))?,
            );
            prior_hasher.update(&buffer[..prior_count]);
        }
        consumed = consumed.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    }
    let full_digest = format!("{:x}", full_hasher.finalize());
    let prior_verified = expectation
        .is_none_or(|expected| format!("{:x}", prior_hasher.finalize()) == expected.prefix_sha256);
    Ok((full_digest, prior_verified))
}
