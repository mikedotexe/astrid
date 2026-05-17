//! File-first archive/retention maintenance for high-volume bridge messages.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::{
    BridgeDb, BridgeMessageArchiveLedgerRow, BridgeMessageArchiveSpan, MessageRow, unix_now,
};

const STATUS_SCHEMA_VERSION: &str = "bridge_db_maintenance_status_v1";
const ARCHIVE_SCHEMA_VERSION: &str = "bridge_message_archive_v1";
const COMPRESSION: &str = "zstd";
const DEFAULT_MAX_SPANS_PER_RUN: usize = 366;

#[derive(Debug, Clone)]
pub struct BridgeMessageMaintenanceConfig {
    pub retention_secs: u64,
    pub archive_dir: PathBuf,
    pub status_path: PathBuf,
    pub db_path: PathBuf,
    pub dry_run: bool,
    pub vacuum_after_maintenance: bool,
    pub max_spans_per_run: usize,
    pub now_override: Option<f64>,
}

impl BridgeMessageMaintenanceConfig {
    #[must_use]
    pub fn new(
        retention_secs: u64,
        archive_dir: PathBuf,
        status_path: PathBuf,
        db_path: PathBuf,
    ) -> Self {
        Self {
            retention_secs,
            archive_dir,
            status_path,
            db_path,
            dry_run: false,
            vacuum_after_maintenance: false,
            max_spans_per_run: DEFAULT_MAX_SPANS_PER_RUN,
            now_override: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMessageMaintenanceOutcome {
    pub schema_version: String,
    pub mode: String,
    pub dry_run: bool,
    pub retention_secs: u64,
    pub cutoff_ts: f64,
    pub archive_dir: String,
    pub db_path: String,
    pub live_message_count_before: u64,
    pub archivable_message_count_before: u64,
    pub live_message_count_after: u64,
    pub archivable_message_count_after: u64,
    pub archived_rows: u64,
    pub deleted_rows: u64,
    pub spans: Vec<BridgeMessageArchiveSpanReport>,
    pub vacuum_recommended: bool,
    pub vacuum_performed: bool,
    pub page_count: u64,
    pub freelist_count: u64,
    pub page_size: u64,
    pub db_size_bytes: u64,
    pub wal_size_bytes: u64,
    pub updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMessageArchiveSpanReport {
    pub day: String,
    pub min_id: i64,
    pub max_id: i64,
    pub row_count: i64,
    pub payload_bytes: i64,
    pub start_ts: f64,
    pub end_ts: f64,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArchiveShardManifest {
    schema_version: String,
    archive_id: String,
    created_at: f64,
    day: String,
    retention_cutoff: f64,
    start_ts: f64,
    end_ts: f64,
    min_id: i64,
    max_id: i64,
    row_count: i64,
    raw_bytes: i64,
    compressed_bytes: i64,
    sha256: String,
    compression: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ArchivedMessageRow<'a> {
    schema_version: &'static str,
    id: i64,
    timestamp: f64,
    direction: &'a str,
    topic: &'a str,
    payload: &'a str,
    fill_pct: Option<f64>,
    lambda1: Option<f64>,
    phase: Option<&'a str>,
}

pub fn run_bridge_message_maintenance(
    db: &BridgeDb,
    config: &BridgeMessageMaintenanceConfig,
) -> Result<BridgeMessageMaintenanceOutcome> {
    let now = config.now_override.unwrap_or_else(unix_now);
    #[expect(clippy::cast_precision_loss)]
    let retention_secs = config.retention_secs as f64;
    let cutoff_ts = now - retention_secs;

    let stats_before = db.bridge_message_live_stats(cutoff_ts)?;
    let spans = db.bridge_message_archive_spans(cutoff_ts, config.max_spans_per_run)?;
    let mut span_reports = Vec::new();
    let mut archived_rows = 0_u64;
    let mut deleted_rows = 0_u64;

    if config.dry_run {
        for span in spans {
            span_reports.push(report_for_span(&span, "would_archive", None, None));
        }
    } else {
        fs::create_dir_all(&config.archive_dir)
            .with_context(|| format!("create archive dir {}", config.archive_dir.display()))?;
        if let Some(parent) = config.status_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create status dir {}", parent.display()))?;
        }

        for span in spans {
            let rows = db.bridge_messages_for_archive_span(&span)?;
            if rows.is_empty() {
                span_reports.push(report_for_span(&span, "no_live_rows", None, None));
                continue;
            }

            let actual_span = span_from_rows(&span, &rows)?;
            if let Some(existing) = db.completed_archive_covering_span(&actual_span)? {
                let archive_path = PathBuf::from(&existing.path);
                if archive_path.exists() {
                    let deleted = db.delete_bridge_messages_for_archive_span(&actual_span)?;
                    deleted_rows = deleted_rows.saturating_add(usize_to_u64(deleted)?);
                    span_reports.push(report_for_span(
                        &actual_span,
                        "deleted_existing_archive_span",
                        Some(existing.archive_id),
                        Some(existing.path),
                    ));
                    continue;
                }
            }

            let shard = write_archive_shard(&config.archive_dir, &actual_span, &rows)?;
            let ledger = BridgeMessageArchiveLedgerRow {
                archive_id: shard.archive_id.clone(),
                created_at: shard.created_at,
                day: shard.day.clone(),
                retention_cutoff: shard.retention_cutoff,
                start_ts: shard.start_ts,
                end_ts: shard.end_ts,
                min_id: shard.min_id,
                max_id: shard.max_id,
                row_count: shard.row_count,
                raw_bytes: shard.raw_bytes,
                compressed_bytes: shard.compressed_bytes,
                sha256: shard.sha256.clone(),
                compression: shard.compression.clone(),
                path: shard.path.clone(),
                manifest_path: manifest_path(&config.archive_dir).display().to_string(),
                status: "completed".to_string(),
            };
            db.record_bridge_message_archive(&ledger)?;
            append_manifest(&config.archive_dir, &shard)?;
            write_index(&config.archive_dir, &shard)?;

            let deleted = db.delete_bridge_messages_for_archive_span(&actual_span)?;
            archived_rows = archived_rows.saturating_add(i64_to_u64(shard.row_count)?);
            deleted_rows = deleted_rows.saturating_add(usize_to_u64(deleted)?);
            span_reports.push(report_for_span(
                &actual_span,
                "archived_and_deleted",
                Some(shard.archive_id),
                Some(shard.path),
            ));
        }
        db.checkpoint_wal()?;
    }

    if config.vacuum_after_maintenance && !config.dry_run {
        db.vacuum()?;
        db.checkpoint_wal()?;
    }

    let stats_after = db.bridge_message_live_stats(cutoff_ts)?;
    let page_stats = db.sqlite_page_stats()?;
    let db_size_bytes = file_len(&config.db_path);
    let wal_size_bytes = file_len(wal_path(&config.db_path));
    let vacuum_recommended = vacuum_recommended(
        page_stats.page_count,
        page_stats.freelist_count,
        page_stats.page_size,
    );
    let outcome = BridgeMessageMaintenanceOutcome {
        schema_version: STATUS_SCHEMA_VERSION.to_string(),
        mode: if config.dry_run {
            "dry_run".to_string()
        } else {
            "maintenance".to_string()
        },
        dry_run: config.dry_run,
        retention_secs: config.retention_secs,
        cutoff_ts,
        archive_dir: config.archive_dir.display().to_string(),
        db_path: config.db_path.display().to_string(),
        live_message_count_before: stats_before.live_count,
        archivable_message_count_before: stats_before.archivable_count,
        live_message_count_after: stats_after.live_count,
        archivable_message_count_after: stats_after.archivable_count,
        archived_rows,
        deleted_rows,
        spans: span_reports,
        vacuum_recommended,
        vacuum_performed: config.vacuum_after_maintenance && !config.dry_run,
        page_count: page_stats.page_count,
        freelist_count: page_stats.freelist_count,
        page_size: page_stats.page_size,
        db_size_bytes,
        wal_size_bytes,
        updated_at: unix_now(),
    };

    if !config.dry_run {
        write_status(&config.status_path, &outcome)?;
    }
    Ok(outcome)
}

pub fn write_bridge_db_status(
    db: &BridgeDb,
    config: &BridgeMessageMaintenanceConfig,
) -> Result<BridgeMessageMaintenanceOutcome> {
    let mut status_config = config.clone();
    status_config.dry_run = false;
    status_config.vacuum_after_maintenance = false;
    status_config.max_spans_per_run = 0;
    let now = status_config.now_override.unwrap_or_else(unix_now);
    #[expect(clippy::cast_precision_loss)]
    let retention_secs = status_config.retention_secs as f64;
    let cutoff_ts = now - retention_secs;
    let stats = db.bridge_message_live_stats(cutoff_ts)?;
    let page_stats = db.sqlite_page_stats()?;
    let outcome = BridgeMessageMaintenanceOutcome {
        schema_version: STATUS_SCHEMA_VERSION.to_string(),
        mode: "status".to_string(),
        dry_run: false,
        retention_secs: status_config.retention_secs,
        cutoff_ts,
        archive_dir: status_config.archive_dir.display().to_string(),
        db_path: status_config.db_path.display().to_string(),
        live_message_count_before: stats.live_count,
        archivable_message_count_before: stats.archivable_count,
        live_message_count_after: stats.live_count,
        archivable_message_count_after: stats.archivable_count,
        archived_rows: 0,
        deleted_rows: 0,
        spans: Vec::new(),
        vacuum_recommended: vacuum_recommended(
            page_stats.page_count,
            page_stats.freelist_count,
            page_stats.page_size,
        ),
        vacuum_performed: false,
        page_count: page_stats.page_count,
        freelist_count: page_stats.freelist_count,
        page_size: page_stats.page_size,
        db_size_bytes: file_len(&status_config.db_path),
        wal_size_bytes: file_len(wal_path(&status_config.db_path)),
        updated_at: unix_now(),
    };
    write_status(&status_config.status_path, &outcome)?;
    Ok(outcome)
}

#[must_use]
pub fn read_runtime_status() -> Option<serde_json::Value> {
    let path = crate::paths::bridge_paths()
        .bridge_workspace()
        .join("runtime/bridge_db_maintenance_status.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn write_archive_shard(
    archive_dir: &Path,
    span: &BridgeMessageArchiveSpan,
    rows: &[MessageRow],
) -> Result<ArchiveShardManifest> {
    let created_at = unix_now();
    let created_ms = unix_millis();
    let archive_id = format!(
        "bma_{}_{}_{}_{}",
        span.day.replace('-', ""),
        span.min_id,
        span.max_id,
        created_ms
    );
    let day_dir = archive_dir.join(&span.day);
    fs::create_dir_all(&day_dir)
        .with_context(|| format!("create archive day dir {}", day_dir.display()))?;
    let final_path = day_dir.join(format!("bridge_messages_{archive_id}.jsonl.zst"));
    let temp_path = day_dir.join(format!(".{archive_id}.jsonl.zst.tmp"));
    let file = File::create(&temp_path)
        .with_context(|| format!("create temp archive {}", temp_path.display()))?;
    let mut encoder = zstd::stream::write::Encoder::new(file, 3)?;
    let mut raw_bytes = 0_i64;
    for row in rows {
        let archived = ArchivedMessageRow {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            id: row.id,
            timestamp: row.timestamp,
            direction: &row.direction,
            topic: &row.topic,
            payload: &row.payload,
            fill_pct: row.fill_pct,
            lambda1: row.lambda1,
            phase: row.phase.as_deref(),
        };
        let line = serde_json::to_vec(&archived)?;
        raw_bytes = raw_bytes
            .checked_add(usize_to_i64(line.len())?)
            .and_then(|value| value.checked_add(1))
            .context("archive raw byte count overflow")?;
        encoder.write_all(&line)?;
        encoder.write_all(b"\n")?;
    }
    let mut compressed_file = encoder.finish()?;
    compressed_file.flush()?;
    drop(compressed_file);
    fs::rename(&temp_path, &final_path).with_context(|| {
        format!(
            "rename archive {} to {}",
            temp_path.display(),
            final_path.display()
        )
    })?;
    let (sha256, compressed_bytes) = sha256_file(&final_path)?;
    let row_count = usize_to_i64(rows.len())?;
    Ok(ArchiveShardManifest {
        schema_version: ARCHIVE_SCHEMA_VERSION.to_string(),
        archive_id,
        created_at,
        day: span.day.clone(),
        retention_cutoff: span.retention_cutoff,
        start_ts: span.start_ts,
        end_ts: span.end_ts,
        min_id: span.min_id,
        max_id: span.max_id,
        row_count,
        raw_bytes,
        compressed_bytes: u64_to_i64(compressed_bytes)?,
        sha256,
        compression: COMPRESSION.to_string(),
        path: final_path.display().to_string(),
    })
}

fn append_manifest(archive_dir: &Path, shard: &ArchiveShardManifest) -> Result<()> {
    let path = manifest_path(archive_dir);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open archive manifest {}", path.display()))?;
    serde_json::to_writer(&mut file, shard)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn write_index(archive_dir: &Path, shard: &ArchiveShardManifest) -> Result<()> {
    let index_path = archive_dir.join("index.json");
    let index = serde_json::json!({
        "schema_version": "bridge_message_archive_index_v1",
        "updated_at": unix_now(),
        "last_archive_id": shard.archive_id,
        "last_day": shard.day,
        "last_path": shard.path,
        "compression": COMPRESSION,
    });
    write_json_pretty(&index_path, &index)
}

fn write_status(path: &Path, outcome: &BridgeMessageMaintenanceOutcome) -> Result<()> {
    write_json_pretty(path, outcome)
}

fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&temp_path, path)?;
    Ok(())
}

fn span_from_rows(
    span: &BridgeMessageArchiveSpan,
    rows: &[MessageRow],
) -> Result<BridgeMessageArchiveSpan> {
    let first = rows.first().context("archive span has no first row")?;
    let last = rows.last().context("archive span has no last row")?;
    let row_count = usize_to_i64(rows.len())?;
    let payload_bytes = rows.iter().try_fold(0_i64, |acc, row| {
        acc.checked_add(usize_to_i64(row.payload.len())?)
            .context("payload byte count overflow")
    })?;
    Ok(BridgeMessageArchiveSpan {
        day: span.day.clone(),
        min_id: first.id,
        max_id: last.id,
        row_count,
        start_ts: first.timestamp,
        end_ts: last.timestamp,
        payload_bytes,
        retention_cutoff: span.retention_cutoff,
    })
}

fn report_for_span(
    span: &BridgeMessageArchiveSpan,
    action: &str,
    archive_id: Option<String>,
    path: Option<String>,
) -> BridgeMessageArchiveSpanReport {
    BridgeMessageArchiveSpanReport {
        day: span.day.clone(),
        min_id: span.min_id,
        max_id: span.max_id,
        row_count: span.row_count,
        payload_bytes: span.payload_bytes,
        start_ts: span.start_ts,
        end_ts: span.end_ts,
        action: action.to_string(),
        archive_id,
        path,
    }
}

fn manifest_path(archive_dir: &Path) -> PathBuf {
    archive_dir.join("manifest.jsonl")
}

fn wal_path(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", db_path.display()))
}

fn file_len<P: AsRef<Path>>(path: P) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

fn sha256_file(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total = total.saturating_add(usize_to_u64(read)?);
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

fn vacuum_recommended(page_count: u64, freelist_count: u64, page_size: u64) -> bool {
    let free_bytes = freelist_count.saturating_mul(page_size);
    let enough_ratio = page_count > 0 && freelist_count.saturating_mul(10) > page_count;
    enough_ratio || free_bytes > 256 * 1024 * 1024
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn usize_to_i64(value: usize) -> Result<i64> {
    i64::try_from(value).map_err(Into::into)
}

fn usize_to_u64(value: usize) -> Result<u64> {
    u64::try_from(value).map_err(Into::into)
}

fn u64_to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(Into::into)
}

fn i64_to_u64(value: i64) -> Result<u64> {
    if value < 0 {
        bail!("negative count cannot become u64: {value}");
    }
    u64::try_from(value).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "astrid_bridge_archive_{label}_{}_{}",
            std::process::id(),
            unix_millis()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn config(root: &Path, db_path: &Path) -> BridgeMessageMaintenanceConfig {
        let mut config = BridgeMessageMaintenanceConfig::new(
            100,
            root.join("archive/bridge_messages"),
            root.join("runtime/bridge_db_maintenance_status.json"),
            db_path.to_path_buf(),
        );
        config.now_override = Some(2_000.0);
        config
    }

    fn open_temp_db(root: &Path) -> (BridgeDb, PathBuf) {
        let db_path = root.join("bridge.db");
        (BridgeDb::open(&db_path).expect("open db"), db_path)
    }

    #[test]
    fn bridge_message_archive_dry_run_reports_without_writing_archive() {
        let root = temp_dir("dry_run");
        let (db, db_path) = open_temp_db(&root);
        db.insert_bridge_message_for_test(1_800.0, "minime_to_astrid", "old", "{\"a\":1}")
            .expect("insert old");
        db.insert_bridge_message_for_test(1_950.0, "minime_to_astrid", "new", "{\"b\":2}")
            .expect("insert new");
        let mut config = config(&root, &db_path);
        config.dry_run = true;

        let outcome = run_bridge_message_maintenance(&db, &config).expect("dry run");

        assert!(outcome.dry_run);
        assert_eq!(outcome.archivable_message_count_before, 1);
        assert_eq!(outcome.archived_rows, 0);
        assert_eq!(outcome.deleted_rows, 0);
        assert_eq!(db.message_count().expect("count"), 2);
        assert!(!config.archive_dir.exists());
    }

    #[test]
    fn bridge_message_archive_writes_manifest_before_delete() {
        let root = temp_dir("archive");
        let (db, db_path) = open_temp_db(&root);
        db.insert_bridge_message_for_test(1_800.0, "minime_to_astrid", "old", "{\"a\":1}")
            .expect("insert old");
        db.insert_bridge_message_for_test(1_801.0, "astrid_to_minime", "old", "{\"a\":2}")
            .expect("insert old");
        db.insert_bridge_message_for_test(1_900.0, "minime_to_astrid", "boundary", "{\"b\":1}")
            .expect("insert boundary");
        db.insert_bridge_message_for_test(1_950.0, "minime_to_astrid", "new", "{\"c\":1}")
            .expect("insert new");
        let config = config(&root, &db_path);

        let outcome = run_bridge_message_maintenance(&db, &config).expect("maintenance");

        assert_eq!(outcome.archived_rows, 2);
        assert_eq!(outcome.deleted_rows, 2);
        assert_eq!(db.message_count().expect("count"), 2);
        assert!(config.status_path.exists());
        let manifest =
            fs::read_to_string(config.archive_dir.join("manifest.jsonl")).expect("read manifest");
        assert!(manifest.contains("\"row_count\":2"));
        let shard_path = outcome.spans[0].path.as_ref().expect("shard path");
        let rows = read_archive_rows(Path::new(shard_path));
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("\"schema_version\":\"bridge_message_archive_v1\""));
    }

    #[test]
    fn bridge_message_archive_rerun_after_ledger_deletes_without_duplicate_archive() {
        let root = temp_dir("rerun");
        let (db, db_path) = open_temp_db(&root);
        db.insert_bridge_message_for_test(1_800.0, "minime_to_astrid", "old", "{\"a\":1}")
            .expect("insert old");
        let config = config(&root, &db_path);
        let span = db
            .bridge_message_archive_spans(1_900.0, 10)
            .expect("spans")
            .pop()
            .expect("span");
        let rows = db
            .bridge_messages_for_archive_span(&span)
            .expect("archive rows");
        let actual_span = span_from_rows(&span, &rows).expect("actual span");
        let shard =
            write_archive_shard(&config.archive_dir, &actual_span, &rows).expect("write shard");
        db.record_bridge_message_archive(&BridgeMessageArchiveLedgerRow {
            archive_id: shard.archive_id.clone(),
            created_at: shard.created_at,
            day: shard.day.clone(),
            retention_cutoff: shard.retention_cutoff,
            start_ts: shard.start_ts,
            end_ts: shard.end_ts,
            min_id: shard.min_id,
            max_id: shard.max_id,
            row_count: shard.row_count,
            raw_bytes: shard.raw_bytes,
            compressed_bytes: shard.compressed_bytes,
            sha256: shard.sha256,
            compression: shard.compression,
            path: shard.path.clone(),
            manifest_path: config
                .archive_dir
                .join("manifest.jsonl")
                .display()
                .to_string(),
            status: "completed".to_string(),
        })
        .expect("record archive");

        let outcome = run_bridge_message_maintenance(&db, &config).expect("maintenance");

        assert_eq!(outcome.archived_rows, 0);
        assert_eq!(outcome.deleted_rows, 1);
        assert_eq!(outcome.spans[0].action, "deleted_existing_archive_span");
        assert_eq!(db.message_count().expect("count"), 0);
    }

    #[test]
    fn bridge_message_archive_marks_vacuum_recommended_after_delete() {
        assert!(vacuum_recommended(100, 11, 4096));
        assert!(vacuum_recommended(1_000_000, 70_000, 4096));
        assert!(!vacuum_recommended(100, 1, 4096));
    }

    fn read_archive_rows(path: &Path) -> Vec<String> {
        let file = File::open(path).expect("open shard");
        let decoder = zstd::stream::read::Decoder::new(file).expect("zstd decoder");
        let reader = std::io::BufReader::new(decoder);
        reader
            .lines()
            .map(|line| line.expect("archive row"))
            .collect()
    }
}
