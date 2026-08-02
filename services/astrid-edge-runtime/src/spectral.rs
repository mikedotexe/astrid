//! Bounded CPU-edge spectral persistence and exact-identifier activity joins.
//!
//! The reservoir performs the sole eigendecomposition. This observer receives
//! only derived one-second summaries, writes one-minute rollups, and never has
//! reservoir-control authority.

use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
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
const MAX_RECENT_ROLLUPS: usize = 1_440;
const MAX_ACTIVITY_REFS_PER_ROLLUP: usize = 2;
const MAX_PENDING_ACTIVITY_REFS: usize = 128;

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
struct ExactActivityRef {
    kind: String,
    recorded_at_unix_ms: u64,
    trace: ActivityTraceRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_sha256: Option<String>,
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
    exported_spectrum_energy_ratio: Option<f64>,
    denominator_uses_full_spectrum: bool,
    sample_count: usize,
    expected_sample_count: usize,
    complete: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SpectralRollup {
    schema: &'static str,
    recorded_at_unix_ms: u64,
    installation_baseline_no_backfill: bool,
    substrate: SubstrateSummary,
    metrics: RollupMetrics,
    coverage: RollupCoverage,
    mode_identity_state: &'static str,
    spectral_derivation_p95_ms: Option<f64>,
    activity_refs: Vec<ExactActivityRef>,
    activity_ref_count: usize,
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
    metrics: RollupMetrics,
    authority: &'static str,
}

pub async fn run(
    config: Arc<Config>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
    mut activities: broadcast::Receiver<ActivityEvent>,
) {
    let mut samples = Vec::new();
    let mut pending_activity_refs = VecDeque::new();
    let mut recent = load_recent(&config.workspace.join("spectral/recent_rollups.jsonl"));
    let authoritative_path = config.workspace.join("spectral/rollups.jsonl");
    let mut installation_baseline =
        fs::metadata(&authoritative_path).map_or(true, |metadata| metadata.len() == 0);
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
                let snapshot = snapshots.borrow().clone();
                if snapshot.spectral_entropy.is_some() {
                    samples.push((unix_millis(), snapshot));
                }
            },
            result = activities.recv() => match result {
                Ok(event) => {
                    let now = unix_millis();
                    let snapshot = snapshots.borrow().clone();
                    if let Err(error) = append_activity_receipt(&config, now, &event, &snapshot) {
                        eprintln!("spectral activity receipt failed: {error}");
                    }
                    if let Some(reference) = exact_activity_ref(now, &event) {
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
                    installation_baseline,
                )
                .and_then(|rollup| persist_rollup(&config, &mut recent, &rollup))
                {
                    Ok(()) => installation_baseline = false,
                    Err(error) => eprintln!("spectral rollup persistence failed: {error}"),
                }
                samples.clear();
                pending_activity_refs.clear();
            },
        }
    }
}

fn build_rollup(
    samples: &[(u64, ReservoirSnapshot)],
    mut activity_refs: Vec<ExactActivityRef>,
    activity_ref_count: usize,
    expected_sample_count: usize,
    installation_baseline_no_backfill: bool,
) -> Result<SpectralRollup> {
    let sample_count = samples.len();
    let complete = sample_count.saturating_mul(10) >= expected_sample_count.saturating_mul(9);
    let unstable = samples
        .iter()
        .any(|(_, snapshot)| snapshot.mode_identity_stable == Some(false));
    let identity_available = samples
        .iter()
        .any(|(_, snapshot)| snapshot.mode_identity_stable.is_some());
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
        installation_baseline_no_backfill,
        substrate: SubstrateSummary {
            kind: "cpu_edge_covariance_effective_rank",
            fill_metric: "normalized_covariance_effective_rank",
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
            exported_spectrum_energy_ratio: summarize(samples, |snapshot| {
                snapshot.exported_spectrum_energy_ratio
            }),
            denominator_uses_full_spectrum: true,
            sample_count,
            expected_sample_count,
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
            bail!("spectral rollup exceeds the 1024-byte policy bound without activity refs");
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

fn exact_activity_ref(now: u64, event: &ActivityEvent) -> Option<ExactActivityRef> {
    let trace = event.trace.as_ref().filter(|trace| trace.is_supported())?;
    Some(ExactActivityRef {
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
    })
}

fn append_activity_receipt(
    config: &Config,
    now: u64,
    event: &ActivityEvent,
    snapshot: &ReservoirSnapshot,
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
            "exact_identifier_join_non_causal"
        } else {
            "legacy_unattributed_no_timestamp_join"
        },
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
    append_private_json_line(&config.workspace.join("spectral/receipts.jsonl"), &receipt)
}

fn persist_rollup(
    config: &Config,
    recent: &mut VecDeque<Value>,
    rollup: &SpectralRollup,
) -> Result<()> {
    let encoded = serde_json::to_vec(&rollup)?;
    if encoded.len().saturating_add(1) > MAX_ROLLUP_BYTES {
        bail!("spectral rollup exceeded the 1024-byte storage bound");
    }
    append_private_bytes(&config.workspace.join("spectral/rollups.jsonl"), &encoded)?;
    recent.push_back(serde_json::from_slice(&encoded)?);
    while recent.len() > MAX_RECENT_ROLLUPS {
        recent.pop_front();
    }
    write_recent_projection(
        &config.workspace.join("spectral/recent_rollups.jsonl"),
        recent,
    )
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

fn load_recent(path: &Path) -> VecDeque<Value> {
    fs::read_to_string(path).map_or_else(
        |_| VecDeque::new(),
        |content| {
            content
                .lines()
                .rev()
                .take(MAX_RECENT_ROLLUPS)
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|value| value.get("schema").and_then(Value::as_str) == Some(ROLLUP_SCHEMA))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        },
    )
}

fn write_recent_projection(path: &Path, recent: &VecDeque<Value>) -> Result<()> {
    let temporary = path.with_extension("jsonl.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    for value in recent {
        let encoded = serde_json::to_vec(value)?;
        if encoded.len().saturating_add(1) > MAX_ROLLUP_BYTES {
            bail!("recent spectral projection contains an oversized row");
        }
        file.write_all(&encoded)?;
        file.write_all(b"\n")?;
    }
    file.sync_all()?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn append_private_json_line(path: &Path, value: &impl Serialize) -> Result<()> {
    append_private_bytes(path, &serde_json::to_vec(value)?)
}

fn append_private_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("open private spectral ledger {}", path.display()))?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
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
    use super::{MAX_ROLLUP_BYTES, build_rollup, exact_activity_ref, set_rollup_hash};
    use crate::{notebook::ActivityEvent, reservoir::ReservoirSnapshot, trace::IpcTraceContextV1};
    use uuid::Uuid;

    fn snapshot(t_ms: u64) -> ReservoirSnapshot {
        ReservoirSnapshot {
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
        let reference = exact_activity_ref(2_000, &event).unwrap();
        let mut rollup =
            build_rollup(&samples, vec![reference.clone(), reference], 3, 60, true).unwrap();
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
        assert!(exact_activity_ref(42, &event).is_none());
    }
}
