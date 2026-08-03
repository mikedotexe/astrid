//! Bounded CPU-edge spectral persistence and exact-identifier activity joins.
//!
//! The reservoir performs the sole eigendecomposition. This observer receives
//! only derived one-second summaries, writes one-minute rollups, and never has
//! reservoir-control authority.

use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use astrid_spectral_core::{TimedScalar, summarize_timed_scalars};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio::sync::{broadcast, watch};

use crate::{
    config::Config, notebook::ActivityEvent, reservoir::ReservoirSnapshot, trace::IpcTraceContextV1,
};

const ROLLUP_SCHEMA: &str = "astrid_edge_spectral_rollup_v1";
const RECEIPT_SCHEMA: &str = "astrid_edge_spectral_receipt_v1";
const AUTHORITY: &str = "deterministic_machine_noncausal_not_authorship_or_control";
const MAX_ROLLUP_BYTES: usize = 1_024;
const MAX_ROLLUP_PROJECTION_DAY_BYTES: u64 = 1_536 * 1_024;
const MAX_ACTIVITY_REFS_PER_ROLLUP: usize = 2;
const MAX_PENDING_ACTIVITY_REFS: usize = 128;
const MAX_ACTIVITY_RECEIPT_BYTES: usize = 4_096;
const MAX_ACTIVITY_PROJECTION_DAY_BYTES: u64 = 2 * 1_024 * 1_024;
const SNAPSHOT_FRESHNESS_MS: u64 = 5_000;
const DAY_MS: u64 = 86_400_000;
const RECENT_CURRENT: &str = "spectral/recent_rollups.current.jsonl";
const RECENT_PREVIOUS: &str = "spectral/recent_rollups.previous.jsonl";
const ACTIVITY_CURRENT: &str = "spectral/activity_receipts.current.jsonl";
const ACTIVITY_PREVIOUS: &str = "spectral/activity_receipts.previous.jsonl";

#[derive(Debug, Clone, Serialize)]
#[allow(
    clippy::struct_field_names,
    reason = "canonical trace contract uses trace_id, session_id, and chain_id"
)]
struct ActivityTraceRef {
    trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chain_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ActivityWindowContextRef {
    kind: String,
    recorded_at_unix_ms: u64,
    trace: ActivityTraceRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_sha256: Option<String>,
    attribution: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct SubstrateSummary {
    kind: &'static str,
    fill_metric: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct RollupMetrics {
    fill_pct: Option<f64>,
    effective_dimensionality: Option<f64>,
    spectral_entropy: Option<f64>,
    lambda1_share: Option<f64>,
    tail_share: Option<f64>,
    density_gradient: Option<f64>,
    mode_turnover: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct RollupCoverage {
    full_spectrum_mode_count: usize,
    exported_spectrum_mode_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum_usable_spectrum_mode_count: Option<usize>,
    #[serde(skip_serializing_if = "is_zero")]
    incomplete_spectrum_sample_count: usize,
    #[serde(skip_serializing_if = "is_zero")]
    clamped_negative_sample_count: usize,
    exported_spectrum_energy_ratio: Option<f64>,
    /// Omitted when true to preserve the fixed record cap; a serialized false
    /// is the explicit incomplete-denominator warning.
    #[serde(skip_serializing_if = "is_true")]
    denominator_uses_full_spectrum: bool,
    sample_count: usize,
    expected_sample_count: usize,
    #[serde(skip_serializing_if = "is_zero")]
    stale_or_duplicate_tick_count: usize,
    complete: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SpectralRollup {
    schema: &'static str,
    recorded_at_unix_ms: u64,
    window_start_unix_ms: u64,
    window_end_unix_ms: u64,
    installation_baseline_no_backfill: bool,
    substrate: SubstrateSummary,
    metrics: RollupMetrics,
    coverage: RollupCoverage,
    mode_identity_state: &'static str,
    spectral_derivation_p95_ms: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    activity_refs: Vec<ActivityWindowContextRef>,
    #[serde(skip_serializing_if = "is_zero")]
    activity_ref_count: usize,
    #[serde(skip_serializing_if = "is_false")]
    activity_refs_truncated: bool,
    authority: &'static str,
    record_sha256: String,
}

#[derive(Debug, Serialize)]
struct ActivityReceipt<'a> {
    schema: &'static str,
    phase: &'static str,
    recorded_at_unix_ms: u64,
    status: &'static str,
    activity_kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_basename: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<&'a IpcTraceContextV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_sha256: Option<&'a str>,
    attribution: &'static str,
    snapshot_generation_id: &'a str,
    snapshot_sequence: u64,
    snapshot_recorded_at_unix_ms: u64,
    snapshot_sha256: String,
    metrics: RollupMetrics,
    authority: &'static str,
}

#[allow(
    clippy::too_many_lines,
    reason = "the single observer select loop keeps sample, activity, and rollup window transitions ordered"
)]
pub async fn run(
    config: Arc<Config>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
    mut activities: broadcast::Receiver<ActivityEvent>,
) {
    for relative in [
        RECENT_CURRENT,
        RECENT_PREVIOUS,
        ACTIVITY_CURRENT,
        ACTIVITY_PREVIOUS,
    ] {
        if let Err(error) = ensure_private_append_file(&config.workspace.join(relative)) {
            eprintln!("spectral projection initialization failed: {error}");
            return;
        }
    }
    let mut samples = Vec::new();
    let mut pending_activity_refs = VecDeque::new();
    let authoritative_path = config.workspace.join("spectral/rollups.jsonl");
    let current_projection_path = config.workspace.join(RECENT_CURRENT);
    let mut projection_day = load_projection_day(&current_projection_path);
    let activity_projection_path = config.workspace.join(ACTIVITY_CURRENT);
    let mut activity_projection_day = load_projection_day(&activity_projection_path);
    let mut installation_baseline =
        fs::metadata(&authoritative_path).map_or(true, |metadata| metadata.len() == 0);
    let mut last_snapshot_key: Option<(String, u64)> = None;
    let mut stale_or_duplicate_tick_count = 0_usize;
    let mut sample_tick = tokio::time::interval(Duration::from_secs(1));
    let mut rollup_tick =
        tokio::time::interval(Duration::from_secs(config.spectral_rollup_seconds.max(15)));
    sample_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    rollup_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Tokio's first interval tick is immediate. Consume it so an installation
    // baseline contains a real bounded window rather than a default snapshot.
    rollup_tick.tick().await;

    loop {
        tokio::select! {
            _ = sample_tick.tick() => {
                let now = unix_millis();
                let snapshot = snapshots.borrow().clone();
                if snapshot.spectral_entropy.is_some()
                    && snapshot_is_unique_and_fresh(&snapshot, now, last_snapshot_key.as_ref())
                {
                    last_snapshot_key = Some((snapshot.generation_id.clone(), snapshot.sequence));
                    samples.push((snapshot.recorded_at_unix_ms, snapshot));
                } else {
                    stale_or_duplicate_tick_count = stale_or_duplicate_tick_count.saturating_add(1);
                }
            },
            result = activities.recv() => match result {
                Ok(event) => {
                    let now = unix_millis();
                    let snapshot = snapshots.borrow().clone();
                    if let Err(error) = append_activity_receipt(
                        &config,
                        now,
                        &event,
                        &snapshot,
                        &mut activity_projection_day,
                    ) {
                        eprintln!("spectral activity receipt failed: {error}");
                        return;
                    }
                    if let Some(reference) = activity_window_context_ref(now, &event) {
                        if pending_activity_refs.len() >= MAX_PENDING_ACTIVITY_REFS {
                            pending_activity_refs.pop_front();
                        }
                        pending_activity_refs.push_back(reference);
                    }
                },
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("spectral activity observer lagged by {skipped} events; no timestamp attribution was attempted");
                },
                Err(broadcast::error::RecvError::Closed) => return,
            },
            _ = rollup_tick.tick() => {
                if samples.is_empty() {
                    stale_or_duplicate_tick_count = 0;
                    pending_activity_refs.clear();
                    continue;
                }
                let all_ref_count = pending_activity_refs.len();
                let refs = pending_activity_refs
                    .iter()
                    .rev()
                    .take(MAX_ACTIVITY_REFS_PER_ROLLUP)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>();
                let expected = usize::try_from(config.spectral_rollup_seconds).unwrap_or(usize::MAX);
                match build_rollup(
                    &samples,
                    refs,
                    all_ref_count,
                    expected,
                    stale_or_duplicate_tick_count,
                    installation_baseline,
                ) {
                    Ok(rollup) => match append_authoritative_rollup(&config, &rollup) {
                        Ok(()) => {
                            installation_baseline = false;
                            if let Err(error) = append_recent_projection(
                                &config,
                                &rollup,
                                &mut projection_day,
                            ) {
                                eprintln!("spectral recent projection refresh failed after authoritative append: {error}");
                                return;
                            }
                        },
                        Err(error) => {
                            eprintln!("spectral authoritative rollup persistence failed: {error}");
                            return;
                        },
                    },
                    Err(error) => {
                        eprintln!("spectral rollup construction failed: {error}");
                        return;
                    },
                }
                samples.clear();
                pending_activity_refs.clear();
                stale_or_duplicate_tick_count = 0;
            },
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "bounded rollup construction keeps completeness, sanitation, and record-size policy atomic"
)]
fn build_rollup(
    samples: &[(u64, ReservoirSnapshot)],
    mut activity_refs: Vec<ActivityWindowContextRef>,
    activity_ref_count: usize,
    expected_sample_count: usize,
    stale_or_duplicate_tick_count: usize,
    installation_baseline_no_backfill: bool,
) -> Result<SpectralRollup> {
    let sample_count = samples.len();
    let window_start_unix_ms = samples
        .iter()
        .map(|(timestamp, _)| *timestamp)
        .min()
        .context("spectral rollup needs a first unique sample")?;
    let window_end_unix_ms = samples
        .iter()
        .map(|(timestamp, _)| *timestamp)
        .max()
        .context("spectral rollup needs a last unique sample")?;
    let expected_span_ms = u64::try_from(expected_sample_count.saturating_sub(1))
        .unwrap_or(u64::MAX)
        .saturating_mul(1_000);
    let observed_span_ms = window_end_unix_ms.saturating_sub(window_start_unix_ms);
    let complete = sample_count.saturating_mul(10) >= expected_sample_count.saturating_mul(9)
        && observed_span_ms.saturating_mul(10) >= expected_span_ms.saturating_mul(9);
    let unstable = samples
        .iter()
        .any(|(_, snapshot)| snapshot.mode_identity_stable == Some(false));
    let identity_available = samples
        .iter()
        .any(|(_, snapshot)| snapshot.mode_identity_stable.is_some());
    let incomplete_spectrum_sample_count = samples
        .iter()
        .filter(|(_, snapshot)| snapshot.discarded_non_finite_mode_count > 0)
        .count();
    let mode_identity_state = if unstable {
        "unstable_near_degenerate"
    } else if identity_available {
        "stable_sign_invariant"
    } else {
        "unavailable_first_sample"
    };
    let mut rollup = SpectralRollup {
        schema: ROLLUP_SCHEMA,
        recorded_at_unix_ms: unix_millis(),
        window_start_unix_ms,
        window_end_unix_ms,
        installation_baseline_no_backfill,
        substrate: SubstrateSummary {
            kind: "cpu_edge_covariance_effective_rank",
            fill_metric: "ema_covariance_effective_rank_0.18",
        },
        metrics: RollupMetrics {
            fill_pct: summarize(samples, |snapshot| {
                Some(f64::from(snapshot.fill_ratio) * 100.0)
            }),
            effective_dimensionality: summarize(samples, |snapshot| {
                Some(f64::from(snapshot.effective_dimensionality))
            }),
            spectral_entropy: summarize(samples, |snapshot| snapshot.spectral_entropy),
            lambda1_share: summarize(samples, |snapshot| snapshot.lambda1_share),
            tail_share: summarize(samples, |snapshot| snapshot.tail_share),
            density_gradient: summarize(samples, |snapshot| snapshot.density_gradient),
            mode_turnover: summarize(samples, |snapshot| snapshot.mode_turnover),
        },
        coverage: RollupCoverage {
            full_spectrum_mode_count: 128,
            exported_spectrum_mode_count: 16,
            minimum_usable_spectrum_mode_count: samples
                .iter()
                .map(|(_, snapshot)| snapshot.usable_spectrum_mode_count)
                .min()
                .filter(|count| *count < 128),
            incomplete_spectrum_sample_count,
            clamped_negative_sample_count: samples
                .iter()
                .filter(|(_, snapshot)| snapshot.clamped_negative_mode_count > 0)
                .count(),
            exported_spectrum_energy_ratio: summarize(samples, |snapshot| {
                snapshot.exported_spectrum_energy_ratio
            }),
            denominator_uses_full_spectrum: incomplete_spectrum_sample_count == 0,
            sample_count,
            expected_sample_count,
            stale_or_duplicate_tick_count,
            complete,
        },
        mode_identity_state,
        spectral_derivation_p95_ms: percentile_95(
            samples
                .iter()
                .filter_map(|(_, snapshot)| snapshot.spectral_derivation_ms)
                .collect(),
        ),
        activity_refs: Vec::new(),
        activity_ref_count,
        activity_refs_truncated: activity_ref_count > activity_refs.len(),
        authority: AUTHORITY,
        record_sha256: String::new(),
    };

    // Keep the canonical per-record storage bound even when identifiers are
    // unusually long. Dropping a ref is explicit; it never becomes a
    // timestamp-based attribution.
    loop {
        rollup.activity_refs.clone_from(&activity_refs);
        set_rollup_hash(&mut rollup)?;
        let encoded = serde_json::to_vec(&rollup)?;
        if encoded.len().saturating_add(1) <= MAX_ROLLUP_BYTES {
            return Ok(rollup);
        }
        if activity_refs.pop().is_none() {
            bail!(
                "spectral rollup exceeds the 1024-byte policy bound without activity refs ({} bytes)",
                encoded.len().saturating_add(1)
            );
        }
        rollup.activity_refs_truncated = true;
    }
}

fn summarize(
    samples: &[(u64, ReservoirSnapshot)],
    value: impl Fn(&ReservoirSnapshot) -> Option<f64>,
) -> Option<f64> {
    let timed = samples
        .iter()
        .filter_map(|(timestamp, snapshot)| {
            value(snapshot)
                .filter(|sample| sample.is_finite())
                .map(|sample| TimedScalar {
                    t_ms: *timestamp,
                    value: sample,
                })
        })
        .collect::<Vec<_>>();
    summarize_timed_scalars(&timed).map(|summary| summary.mean)
}

fn percentile_95(mut values: Vec<f64>) -> Option<f64> {
    values.retain(|value| value.is_finite());
    values.sort_by(f64::total_cmp);
    let last = values.len().checked_sub(1)?;
    let index = last.saturating_mul(95) / 100;
    values.get(index).copied()
}

fn activity_window_context_ref(
    now: u64,
    event: &ActivityEvent,
) -> Option<ActivityWindowContextRef> {
    let trace = event.trace.as_ref().filter(|trace| trace.is_supported())?;
    Some(ActivityWindowContextRef {
        kind: bounded(event.kind, 48),
        recorded_at_unix_ms: now,
        trace: ActivityTraceRef {
            trace_id: trace.trace_id.to_string(),
            session_id: trace.session_id.as_deref().map(|value| bounded(value, 96)),
            chain_id: trace.chain_id.as_deref().map(|value| bounded(value, 96)),
        },
        response_sha256: event
            .response_sha256
            .as_deref()
            .filter(|value| valid_sha256(value))
            .map(str::to_ascii_lowercase),
        attribution: "temporal_rollup_context_not_exact_or_causal",
    })
}

fn append_activity_receipt(
    config: &Config,
    now: u64,
    event: &ActivityEvent,
    snapshot: &ReservoirSnapshot,
    projection_day: &mut Option<u64>,
) -> Result<()> {
    let traced = event
        .trace
        .as_ref()
        .is_some_and(IpcTraceContextV1::is_supported);
    let receipt = ActivityReceipt {
        schema: RECEIPT_SCHEMA,
        phase: "activity_observed",
        recorded_at_unix_ms: now,
        status: "machine_snapshot_recorded",
        activity_kind: event.kind,
        artifact_basename: event.artifact_basename.as_deref(),
        trace: traced.then_some(event.trace.as_ref()).flatten(),
        response_sha256: event
            .response_sha256
            .as_deref()
            .filter(|value| valid_sha256(value)),
        attribution: if traced {
            "exact_event_identity_to_named_snapshot_non_causal"
        } else {
            "legacy_unattributed_no_timestamp_join"
        },
        snapshot_generation_id: &snapshot.generation_id,
        snapshot_sequence: snapshot.sequence,
        snapshot_recorded_at_unix_ms: snapshot.recorded_at_unix_ms,
        snapshot_sha256: snapshot_sha256(snapshot)?,
        metrics: RollupMetrics {
            fill_pct: Some(f64::from(snapshot.fill_ratio) * 100.0),
            effective_dimensionality: Some(f64::from(snapshot.effective_dimensionality)),
            spectral_entropy: snapshot.spectral_entropy,
            lambda1_share: snapshot.lambda1_share,
            tail_share: snapshot.tail_share,
            density_gradient: snapshot.density_gradient,
            mode_turnover: snapshot.mode_turnover,
        },
        authority: AUTHORITY,
    };
    let mut value = serde_json::to_value(&receipt)?;
    set_value_hash(&mut value)?;
    let encoded = serde_json::to_vec(&value)?;
    if encoded.len().saturating_add(1) > MAX_ACTIVITY_RECEIPT_BYTES {
        bail!("spectral activity receipt exceeds the 4096-byte policy bound");
    }
    append_private_bytes(&config.workspace.join("spectral/receipts.jsonl"), &encoded)?;
    append_daily_projection(
        config,
        ACTIVITY_CURRENT,
        ACTIVITY_PREVIOUS,
        now,
        &encoded,
        MAX_ACTIVITY_RECEIPT_BYTES,
        MAX_ACTIVITY_PROJECTION_DAY_BYTES,
        projection_day,
    )
}

fn append_authoritative_rollup(config: &Config, rollup: &SpectralRollup) -> Result<()> {
    let encoded = serde_json::to_vec(&rollup)?;
    if encoded.len().saturating_add(1) > MAX_ROLLUP_BYTES {
        bail!("spectral rollup exceeded the 1024-byte storage bound");
    }
    append_private_bytes(&config.workspace.join("spectral/rollups.jsonl"), &encoded)
}

fn set_rollup_hash(rollup: &mut SpectralRollup) -> Result<()> {
    rollup.record_sha256.clear();
    let mut value = serde_json::to_value(&*rollup)?;
    value
        .as_object_mut()
        .context("spectral rollup must serialize as an object")?
        .remove("record_sha256");
    rollup.record_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?));
    Ok(())
}

fn set_value_hash(value: &mut Value) -> Result<()> {
    let object = value
        .as_object_mut()
        .context("hashed spectral record must serialize as an object")?;
    object.remove("record_sha256");
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&*object)?));
    object.insert("record_sha256".to_string(), Value::String(digest));
    Ok(())
}

fn snapshot_sha256(snapshot: &ReservoirSnapshot) -> Result<String> {
    let evidence = serde_json::json!({
        "generation_id": snapshot.generation_id,
        "sequence": snapshot.sequence,
        "recorded_at_unix_ms": snapshot.recorded_at_unix_ms,
        "fill_ratio": snapshot.fill_ratio,
        "effective_dimensionality": snapshot.effective_dimensionality,
        "spectral_entropy": snapshot.spectral_entropy,
        "lambda1_share": snapshot.lambda1_share,
        "tail_share": snapshot.tail_share,
        "density_gradient": snapshot.density_gradient,
        "mode_turnover": snapshot.mode_turnover,
    });
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&evidence)?)
    ))
}

fn snapshot_is_unique_and_fresh(
    snapshot: &ReservoirSnapshot,
    now: u64,
    previous: Option<&(String, u64)>,
) -> bool {
    if snapshot.generation_id.is_empty()
        || snapshot.sequence == 0
        || snapshot.recorded_at_unix_ms == 0
        || now.abs_diff(snapshot.recorded_at_unix_ms) > SNAPSHOT_FRESHNESS_MS
    {
        return false;
    }
    previous.is_none_or(|(generation, sequence)| {
        snapshot.generation_id != *generation || snapshot.sequence > *sequence
    })
}

fn load_projection_day(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok().and_then(|content| {
        content.lines().rev().find_map(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|value| value.get("recorded_at_unix_ms").and_then(Value::as_u64))
                .map(|timestamp| timestamp / DAY_MS)
        })
    })
}

fn append_recent_projection(
    config: &Config,
    rollup: &SpectralRollup,
    projection_day: &mut Option<u64>,
) -> Result<()> {
    let encoded = serde_json::to_vec(rollup)?;
    append_daily_projection(
        config,
        RECENT_CURRENT,
        RECENT_PREVIOUS,
        rollup.recorded_at_unix_ms,
        &encoded,
        MAX_ROLLUP_BYTES,
        MAX_ROLLUP_PROJECTION_DAY_BYTES,
        projection_day,
    )
}

#[allow(clippy::too_many_arguments)] // Explicit paths and limits keep projections policy-bound.
fn append_daily_projection(
    config: &Config,
    current_relative: &str,
    previous_relative: &str,
    recorded_at_unix_ms: u64,
    encoded: &[u8],
    maximum_record_bytes: usize,
    maximum_day_bytes: u64,
    projection_day: &mut Option<u64>,
) -> Result<()> {
    if encoded.len().saturating_add(1) > maximum_record_bytes {
        bail!("daily spectral projection contains an oversized row");
    }
    let day = recorded_at_unix_ms / DAY_MS;
    let current = config.workspace.join(current_relative);
    let previous = config.workspace.join(previous_relative);
    let mut current_metadata = validate_existing_regular(&current)?;
    let _ = validate_existing_regular(&previous)?;
    if projection_day.is_some_and(|current_day| current_day != day) {
        if current_metadata.is_some() {
            fs::rename(&current, &previous).with_context(|| {
                format!(
                    "rotate current spectral projection {} to {}",
                    current.display(),
                    previous.display()
                )
            })?;
        }
        *projection_day = None;
        current_metadata = None;
    }
    let existing_bytes = current_metadata.map_or(0, |metadata| metadata.len());
    let appended_bytes = u64::try_from(encoded.len().saturating_add(1)).unwrap_or(u64::MAX);
    if existing_bytes.saturating_add(appended_bytes) > maximum_day_bytes {
        bail!("daily spectral projection exceeds its whole-file safety cap");
    }
    append_private_bytes(&current, encoded)?;
    *projection_day = Some(day);
    Ok(())
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip predicates require references.
const fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip predicates require references.
const fn is_false(value: &bool) -> bool {
    !*value
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip predicates require references.
const fn is_true(value: &bool) -> bool {
    *value
}

fn append_private_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = open_private_append_regular(path)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn ensure_private_append_file(path: &Path) -> Result<()> {
    open_private_append_regular(path)?;
    Ok(())
}

fn open_private_append_regular(path: &Path) -> Result<fs::File> {
    let _ = validate_existing_regular(path)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("open private spectral file {}", path.display()))?;
    let opened = file.metadata()?;
    let path_metadata = fs::symlink_metadata(path)?;
    if !path_metadata.file_type().is_file()
        || opened.dev() != path_metadata.dev()
        || opened.ino() != path_metadata.ino()
    {
        bail!(
            "private spectral path changed identity or is not a regular file: {}",
            path.display()
        );
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

fn validate_existing_regular(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(metadata)),
        Ok(_) => bail!(
            "private spectral target is not a regular non-symlink file: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("inspect private spectral target {}", path.display())),
    }
}

fn bounded(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum)
        .collect()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn unix_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::{PermissionsExt as _, symlink},
    };

    use super::{
        ACTIVITY_CURRENT, DAY_MS, MAX_ROLLUP_BYTES, RECENT_CURRENT, RECENT_PREVIOUS,
        activity_window_context_ref, append_activity_receipt, append_private_bytes,
        append_recent_projection, build_rollup, ensure_private_append_file, set_rollup_hash,
        snapshot_is_unique_and_fresh, snapshot_sha256,
    };
    use crate::{
        config::Config, notebook::ActivityEvent, reservoir::ReservoirSnapshot,
        trace::IpcTraceContextV1,
    };
    use clap::Parser as _;
    use uuid::Uuid;

    fn snapshot(t_ms: u64) -> ReservoirSnapshot {
        ReservoirSnapshot {
            generation_id: "generation-test".to_string(),
            sequence: t_ms.saturating_add(1),
            recorded_at_unix_ms: 1_000 + t_ms.saturating_mul(1_000),
            t_ms,
            fill_ratio: 0.68,
            effective_dimensionality: 87.0,
            spectral_entropy: Some(0.91),
            lambda1_share: Some(0.08),
            tail_share: Some(0.72),
            density_gradient: Some(0.03),
            mode_turnover: Some(0.12),
            mode_identity_stable: Some(true),
            exported_spectrum_energy_ratio: Some(0.42),
            usable_spectrum_mode_count: 128,
            spectral_derivation_ms: Some(3.0),
            ..ReservoirSnapshot::default()
        }
    }

    #[test]
    fn rollup_is_bounded_hashed_and_explicitly_no_backfill() {
        let samples = (0..60)
            .map(|index| (1_000 + index * 1_000, snapshot(index)))
            .collect::<Vec<_>>();
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "session-with-a-bounded-identifier".to_string(),
            Some("chain".to_string()),
        );
        let event = ActivityEvent {
            kind: "sovereign_action_outcome",
            artifact_basename: None,
            trace: Some(trace),
            response_sha256: Some("a".repeat(64)),
        };
        let reference = activity_window_context_ref(2_000, &event).unwrap();
        assert_eq!(
            reference.attribution,
            "temporal_rollup_context_not_exact_or_causal"
        );
        let mut rollup =
            build_rollup(&samples, vec![reference.clone(), reference], 3, 60, 0, true).unwrap();
        assert!(rollup.installation_baseline_no_backfill);
        assert_eq!(rollup.coverage.sample_count, 60);
        assert_eq!(rollup.activity_ref_count, 3);
        assert!(rollup.activity_refs_truncated);
        let first_hash = rollup.record_sha256.clone();
        set_rollup_hash(&mut rollup).unwrap();
        assert_eq!(rollup.record_sha256, first_hash);
        assert!(serde_json::to_vec(&rollup).unwrap().len() < MAX_ROLLUP_BYTES);
    }

    #[test]
    fn untraced_activity_is_never_attributed_by_time() {
        let event = ActivityEvent {
            kind: "tool_result",
            artifact_basename: None,
            trace: None,
            response_sha256: Some("b".repeat(64)),
        };
        assert!(activity_window_context_ref(42, &event).is_none());
    }

    #[test]
    fn duplicate_or_stale_snapshots_never_count_as_fresh_samples() {
        let snapshot = snapshot(7);
        let now = snapshot.recorded_at_unix_ms;
        assert!(snapshot_is_unique_and_fresh(&snapshot, now, None));
        let key = (snapshot.generation_id.clone(), snapshot.sequence);
        assert!(!snapshot_is_unique_and_fresh(&snapshot, now, Some(&key)));
        assert!(!snapshot_is_unique_and_fresh(
            &snapshot,
            now.saturating_add(5_001),
            None,
        ));
        let next = ReservoirSnapshot {
            sequence: snapshot.sequence.saturating_add(1),
            recorded_at_unix_ms: now.saturating_add(1_000),
            ..snapshot
        };
        assert!(snapshot_is_unique_and_fresh(
            &next,
            next.recorded_at_unix_ms,
            Some(&key),
        ));
    }

    #[test]
    fn recent_projection_appends_and_rotates_once_per_utc_day() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-spectral-projection-{}",
            super::unix_millis()
        ));
        let mut config = Config::try_parse_from(["edge"]).unwrap();
        config.workspace.clone_from(&workspace);
        config.prepare_workspace().unwrap();
        let samples = (0..60)
            .map(|index| (1_000 + index * 1_000, snapshot(index)))
            .collect::<Vec<_>>();
        let mut first = build_rollup(&samples, Vec::new(), 0, 60, 0, true).unwrap();
        first.recorded_at_unix_ms = DAY_MS.saturating_mul(10).saturating_add(1);
        set_rollup_hash(&mut first).unwrap();
        let mut projection_day = None;
        append_recent_projection(&config, &first, &mut projection_day).unwrap();
        append_recent_projection(&config, &first, &mut projection_day).unwrap();
        assert_eq!(
            fs::read_to_string(workspace.join(RECENT_CURRENT))
                .unwrap()
                .lines()
                .count(),
            2
        );

        let mut next = first.clone();
        next.recorded_at_unix_ms = DAY_MS.saturating_mul(11).saturating_add(1);
        set_rollup_hash(&mut next).unwrap();
        append_recent_projection(&config, &next, &mut projection_day).unwrap();
        assert_eq!(
            fs::read_to_string(workspace.join(RECENT_PREVIOUS))
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert_eq!(
            fs::read_to_string(workspace.join(RECENT_CURRENT))
                .unwrap()
                .lines()
                .count(),
            1
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn activity_receipt_binds_exact_snapshot_identity_and_metrics() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-spectral-receipt-{}",
            super::unix_millis()
        ));
        let mut config = Config::try_parse_from(["edge"]).unwrap();
        config.workspace.clone_from(&workspace);
        config.prepare_workspace().unwrap();
        let snapshot = snapshot(7);
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "session-exact".to_string(),
            Some("chain-exact".to_string()),
        );
        let event = ActivityEvent {
            kind: "sovereign_action_outcome",
            artifact_basename: Some("result.md".to_string()),
            trace: Some(trace.clone()),
            response_sha256: Some("a".repeat(64)),
        };
        let mut projection_day = None;
        append_activity_receipt(
            &config,
            snapshot.recorded_at_unix_ms,
            &event,
            &snapshot,
            &mut projection_day,
        )
        .unwrap();
        let line = fs::read_to_string(workspace.join(ACTIVITY_CURRENT)).unwrap();
        let receipt: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(receipt["snapshot_generation_id"], snapshot.generation_id);
        assert_eq!(receipt["snapshot_sequence"], snapshot.sequence);
        assert_eq!(
            receipt["snapshot_recorded_at_unix_ms"],
            snapshot.recorded_at_unix_ms
        );
        assert_eq!(
            receipt["snapshot_sha256"],
            snapshot_sha256(&snapshot).unwrap()
        );
        assert!((receipt["metrics"]["fill_pct"].as_f64().unwrap() - 68.0).abs() < 0.001);
        assert_eq!(receipt["trace"]["trace_id"], trace.trace_id.to_string());
        assert_eq!(receipt["response_sha256"], "a".repeat(64));
        assert_eq!(receipt["record_sha256"].as_str().unwrap().len(), 64);
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn private_spectral_files_reject_symlinks_and_normalize_modes() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-spectral-private-files-{}",
            super::unix_millis()
        ));
        let mut config = Config::try_parse_from(["edge"]).unwrap();
        config.workspace.clone_from(&workspace);
        config.prepare_workspace().unwrap();
        let outside = workspace.join("outside.txt");
        fs::write(&outside, b"outside").unwrap();
        let projection = workspace.join(RECENT_CURRENT);
        symlink(&outside, &projection).unwrap();
        assert!(ensure_private_append_file(&projection).is_err());
        assert!(append_private_bytes(&projection, b"escape").is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"outside");

        fs::remove_file(&projection).unwrap();
        fs::write(&projection, b"").unwrap();
        fs::set_permissions(&projection, fs::Permissions::from_mode(0o644)).unwrap();
        ensure_private_append_file(&projection).unwrap();
        assert_eq!(
            fs::metadata(&projection).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let samples = (0..60)
            .map(|index| (1_000 + index * 1_000, snapshot(index)))
            .collect::<Vec<_>>();
        let mut rollup = build_rollup(&samples, Vec::new(), 0, 60, 0, false).unwrap();
        rollup.recorded_at_unix_ms = DAY_MS.saturating_mul(12);
        set_rollup_hash(&mut rollup).unwrap();
        let previous = workspace.join(RECENT_PREVIOUS);
        symlink(&outside, &previous).unwrap();
        let mut projection_day = Some(11);
        assert!(append_recent_projection(&config, &rollup, &mut projection_day).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        fs::remove_dir_all(workspace).unwrap();
    }
}
