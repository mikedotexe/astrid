//! Bounded, evidence-only capture for preregistered experiential studies.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::lived_state_witness;
use crate::paths::bridge_paths;
use crate::signal_spine::signal_deployment_identity_v1;
use crate::types::{TelemetryHeartbeatDeltaV1, TelemetryIntegrationHealthV1};

const WRITER_CAPACITY: usize = 256;
const MAX_SAMPLE_LIMIT: usize = 8_192;
const MAX_WINDOW_DURATION_MS: u64 = 2 * 60 * 60 * 1_000;
const WINDOW_CACHE_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize)]
struct EvidenceOnlyAuthorityV1 {
    schema: &'static str,
    schema_version: u8,
    state: &'static str,
    witness_only: bool,
}

impl EvidenceOnlyAuthorityV1 {
    const fn new() -> Self {
        Self {
            schema: "artifact_authority_state_v1",
            schema_version: 1,
            state: "evidence_only",
            witness_only: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StudyCaptureWindowV1 {
    schema: &'static str,
    schema_version: u8,
    window_id: String,
    campaign_id: String,
    study_id: String,
    plan_id: String,
    plan_sha256: String,
    sample_kind: String,
    started_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    sample_limit: usize,
    actor: String,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Deserialize)]
struct UntrustedWindowV1 {
    schema: String,
    schema_version: u8,
    window_id: String,
    campaign_id: String,
    study_id: String,
    plan_id: String,
    plan_sha256: String,
    sample_kinds: Vec<String>,
    started_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    sample_limit: usize,
    actor: String,
    capture_can_delay_behavior: bool,
    preregistered_before_capture: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: UntrustedAuthorityV1,
}

#[derive(Debug, Deserialize)]
struct UntrustedAuthorityV1 {
    state: String,
    witness_only: bool,
}

#[derive(Debug, Deserialize)]
struct UntrustedRegistryV1 {
    windows: HashMap<String, UntrustedWindowV1>,
}

impl StudyCaptureWindowV1 {
    fn from_untrusted(key: &str, value: UntrustedWindowV1, now_ms: u64) -> Option<Self> {
        let sample_kind = value.sample_kinds.as_slice();
        if value.schema != "study_window_spec_v1"
            || value.schema_version != 1
            || value.window_id != key
            || !bounded_identifier(key)
            || !bounded_identifier(&value.campaign_id)
            || !bounded_identifier(&value.study_id)
            || !bounded_identifier(&value.plan_id)
            || !valid_sha256(&value.plan_sha256)
            || sample_kind.len() != 1
            || !matches!(
                sample_kind.first().map(String::as_str),
                Some("telemetry" | "heartbeat" | "codec_lane" | "codec_gate")
            )
            || value.started_at_unix_ms >= value.expires_at_unix_ms
            || value
                .expires_at_unix_ms
                .saturating_sub(value.started_at_unix_ms)
                > MAX_WINDOW_DURATION_MS
            || now_ms < value.started_at_unix_ms
            || now_ms > value.expires_at_unix_ms
            || value.sample_limit == 0
            || value.sample_limit > MAX_SAMPLE_LIMIT
            || !bounded_identifier(&value.actor)
            || value.capture_can_delay_behavior
            || !value.preregistered_before_capture
            || value.raw_prose_included
            || value.artifact_authority_state_v1.state != "evidence_only"
            || !value.artifact_authority_state_v1.witness_only
        {
            return None;
        }
        Some(Self {
            schema: "study_capture_window_v1",
            schema_version: 1,
            window_id: value.window_id,
            campaign_id: value.campaign_id,
            study_id: value.study_id,
            plan_id: value.plan_id,
            plan_sha256: value.plan_sha256,
            sample_kind: sample_kind[0].clone(),
            started_at_unix_ms: value.started_at_unix_ms,
            expires_at_unix_ms: value.expires_at_unix_ms,
            sample_limit: value.sample_limit,
            actor: value.actor,
            artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
        })
    }
}

fn bounded_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'.' | b':' | b'@' | b'/' | b'+' | b'-')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn deployment_identity_sha256() -> String {
    sha256_bytes(signal_deployment_identity_v1().as_bytes())
}

#[derive(Debug)]
struct WindowCacheV1 {
    checked_at: Instant,
    modified: Option<SystemTime>,
    windows: Vec<StudyCaptureWindowV1>,
}

impl Default for WindowCacheV1 {
    fn default() -> Self {
        Self {
            checked_at: Instant::now()
                .checked_sub(WINDOW_CACHE_INTERVAL)
                .unwrap_or_else(Instant::now),
            modified: None,
            windows: Vec::new(),
        }
    }
}

static WINDOW_CACHE: OnceLock<Mutex<WindowCacheV1>> = OnceLock::new();

fn active_windows_path() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics/evidence_study_runtime_v1/active_windows.json")
}

fn read_windows(path: &Path, now_ms: u64) -> Vec<StudyCaptureWindowV1> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let Ok(registry) = serde_json::from_slice::<UntrustedRegistryV1>(&bytes) else {
        return Vec::new();
    };
    let mut windows: Vec<_> = registry
        .windows
        .into_iter()
        .filter_map(|(key, value)| StudyCaptureWindowV1::from_untrusted(&key, value, now_ms))
        .collect();
    windows.sort_by(|left, right| left.window_id.cmp(&right.window_id));
    windows
}

fn matching_windows(sample_kind: &str) -> Vec<StudyCaptureWindowV1> {
    let path = active_windows_path();
    let now = lived_state_witness::clock_sample_v1();
    let cache = WINDOW_CACHE.get_or_init(|| Mutex::new(WindowCacheV1::default()));
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if cache.checked_at.elapsed() >= WINDOW_CACHE_INTERVAL {
        let modified = fs::metadata(&path).and_then(|value| value.modified()).ok();
        if modified != cache.modified {
            cache.windows = read_windows(&path, now.unix_ms);
            cache.modified = modified;
        } else {
            cache
                .windows
                .retain(|window| now.unix_ms <= window.expires_at_unix_ms);
        }
        cache.checked_at = Instant::now();
    }
    cache
        .windows
        .iter()
        .filter(|window| window.sample_kind == sample_kind)
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryStudyMetricsV1 {
    integration_us: f64,
    prewrite_us: f64,
    write_lock_wait_us: f64,
    write_lock_hold_us: f64,
    entropy_peak: Option<f32>,
    entropy_variance: Option<f32>,
    entropy_trend: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryStudySampleV1 {
    schema: &'static str,
    schema_version: u8,
    sample_id: String,
    window_id: String,
    sample_kind: &'static str,
    classification: String,
    connection_id: Option<u64>,
    telemetry_t_ms: u64,
    observed_at_unix_ms: u64,
    monotonic_time_ns: u64,
    process_identity_sha256: String,
    deployment_identity_sha256: String,
    metrics: TelemetryStudyMetricsV1,
    timing_establishes_causation: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Clone)]
pub(crate) struct HeartbeatCaptureAttemptV1 {
    windows: Vec<StudyCaptureWindowV1>,
    attempt_id: String,
    phase: f32,
    cadence_seconds: u64,
    intensity: f32,
    signal_norm: Option<f32>,
    pressure: Option<f32>,
    entropy: Option<f32>,
    monotonic_time_ns: u64,
}

#[derive(Debug, Clone, Serialize)]
struct HeartbeatStudyMetricsV1 {
    cadence_seconds: f64,
    intensity: f32,
    pressure: Option<f32>,
    entropy: Option<f32>,
    phase_code: f32,
    signal_norm: Option<f32>,
    queue_wait_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeartbeatStudySampleV1 {
    schema: &'static str,
    schema_version: u8,
    sample_id: String,
    window_id: String,
    sample_kind: &'static str,
    admission: &'static str,
    enqueue_outcome: &'static str,
    observed_at_unix_ms: u64,
    monotonic_time_ns: u64,
    process_identity_sha256: String,
    deployment_identity_sha256: String,
    metrics: HeartbeatStudyMetricsV1,
    behavior_changed: bool,
    felt_outcome_inferred: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Clone, Serialize)]
struct CodecGateStudyMetricsV1 {
    lane_energy: f64,
    total_energy: f64,
    headroom: f64,
    clamp_occupancy: f64,
    representation_loss: f64,
}

#[derive(Debug, Clone, Serialize)]
struct CodecGateStudySampleV1 {
    schema: &'static str,
    schema_version: u8,
    sample_id: String,
    window_id: String,
    sample_kind: &'static str,
    cohort: &'static str,
    journey_id: String,
    observed_at_unix_ms: u64,
    monotonic_time_ns: u64,
    process_identity_sha256: String,
    deployment_identity_sha256: String,
    source_fixture_sha256: String,
    metrics: CodecGateStudyMetricsV1,
    counterfactual_dispatched: bool,
    behavior_changed: bool,
    felt_outcome_inferred: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Clone, Serialize)]
pub struct StudyCaptureGapReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    gap_id: String,
    window_id: String,
    study_id: String,
    reason: &'static str,
    dropped_sample_count: u64,
    observed_at_unix_ms: u64,
    behavior_blocked: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug)]
struct WriteItemV1 {
    window: StudyCaptureWindowV1,
    sample: Value,
    dropped_before: u64,
}

#[derive(Debug)]
struct StudyWriterV1 {
    sender: SyncSender<WriteItemV1>,
    dropped: AtomicU64,
}

static WRITER: OnceLock<StudyWriterV1> = OnceLock::new();

fn writer() -> &'static StudyWriterV1 {
    WRITER.get_or_init(|| {
        let (sender, receiver) = sync_channel::<WriteItemV1>(WRITER_CAPACITY);
        thread::Builder::new()
            .name("evidence-study-writer".to_string())
            .spawn(move || {
                let mut counts: HashMap<String, usize> = HashMap::new();
                while let Ok(item) = receiver.recv() {
                    if item.dropped_before > 0 {
                        let gap = capture_gap(&item.window, "queue_exhausted", item.dropped_before);
                        let _ = append_record(&item.window.window_id, &gap);
                    }
                    let count = counts.entry(item.window.window_id.clone()).or_default();
                    if *count >= item.window.sample_limit {
                        // The preregistered ceiling is successful bounded capture, not
                        // missing evidence. Queue or write loss still emits a gap.
                        continue;
                    }
                    if append_value(&item.window.window_id, &item.sample).is_ok() {
                        *count = count.saturating_add(1);
                    } else {
                        let gap = capture_gap(&item.window, "asynchronous_write_failed", 1);
                        let _ = append_record(&item.window.window_id, &gap);
                    }
                }
            })
            .expect("evidence study writer thread must start");
        StudyWriterV1 {
            sender,
            dropped: AtomicU64::new(0),
        }
    })
}

fn enqueue(window: StudyCaptureWindowV1, sample: Value) {
    let writer = writer();
    let dropped_before = writer.dropped.swap(0, Ordering::AcqRel);
    let item = WriteItemV1 {
        window,
        sample,
        dropped_before,
    };
    if let Err(error) = writer.sender.try_send(item) {
        let carried = match error {
            TrySendError::Full(item) | TrySendError::Disconnected(item) => item.dropped_before,
        };
        writer
            .dropped
            .fetch_add(carried.saturating_add(1), Ordering::AcqRel);
    }
}

fn samples_root() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics/evidence_study_runtime_v1/samples")
}

fn append_value(window_id: &str, value: &Value) -> std::io::Result<()> {
    let root = samples_root();
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let path = root.join(format!("{window_id}.jsonl"));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_data()
}

fn append_record<T: Serialize>(window_id: &str, value: &T) -> std::io::Result<()> {
    append_value(
        window_id,
        &serde_json::to_value(value).map_err(std::io::Error::other)?,
    )
}

fn capture_gap(
    window: &StudyCaptureWindowV1,
    reason: &'static str,
    dropped: u64,
) -> StudyCaptureGapReceiptV1 {
    let clock = lived_state_witness::clock_sample_v1();
    let core = format!(
        "{}\0{}\0{}\0{}\0{}",
        window.window_id, window.study_id, reason, dropped, clock.unix_ms
    );
    StudyCaptureGapReceiptV1 {
        schema: "study_capture_gap_receipt_v1",
        schema_version: 1,
        gap_id: format!("studygap_{}", sha256_bytes(core.as_bytes())),
        window_id: window.window_id.clone(),
        study_id: window.study_id.clone(),
        reason,
        dropped_sample_count: dropped,
        observed_at_unix_ms: clock.unix_ms,
        behavior_blocked: false,
        raw_prose_included: false,
        artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
    }
}

pub(crate) fn record_telemetry_sample_v1(
    health: &TelemetryIntegrationHealthV1,
    heartbeat: &TelemetryHeartbeatDeltaV1,
    telemetry_t_ms: u64,
) {
    let windows = matching_windows("telemetry");
    if windows.is_empty() {
        return;
    }
    let clock = lived_state_witness::clock_sample_v1();
    let process_identity_sha256 = lived_state_witness::runtime_process_identity_sha256_v1();
    let deployment_identity_sha256 = deployment_identity_sha256();
    for window in windows {
        let core = format!(
            "{}\0{}\0{}\0{}",
            window.window_id, health.sample_count, telemetry_t_ms, clock.monotonic_ns
        );
        let sample = TelemetryStudySampleV1 {
            schema: "telemetry_study_sample_v1",
            schema_version: 1,
            sample_id: format!("telemetrystudy_{}", sha256_bytes(core.as_bytes())),
            window_id: window.window_id.clone(),
            sample_kind: "telemetry",
            classification: health.classification.clone(),
            connection_id: heartbeat.active_connection_id,
            telemetry_t_ms,
            observed_at_unix_ms: clock.unix_ms,
            monotonic_time_ns: clock.monotonic_ns,
            process_identity_sha256: process_identity_sha256.clone(),
            deployment_identity_sha256: deployment_identity_sha256.clone(),
            metrics: TelemetryStudyMetricsV1 {
                integration_us: (health.latest_prewrite_pipeline_ms
                    + health.latest_write_lock_wait_ms
                    + health.latest_write_lock_hold_ms)
                    * 1_000.0,
                prewrite_us: health.latest_prewrite_pipeline_ms * 1_000.0,
                write_lock_wait_us: health.latest_write_lock_wait_ms * 1_000.0,
                write_lock_hold_us: health.latest_write_lock_hold_ms * 1_000.0,
                entropy_peak: heartbeat.peak_spectral_entropy_in_window,
                entropy_variance: heartbeat.rolling_spectral_entropy_variance,
                entropy_trend: heartbeat.rolling_spectral_entropy_change,
            },
            timing_establishes_causation: false,
            raw_prose_included: false,
            artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
        };
        if let Ok(value) = serde_json::to_value(sample) {
            enqueue(window, value);
        }
    }
}

pub(crate) fn codec_gate_capture_active_v1() -> bool {
    !matching_windows("codec_gate").is_empty()
}

fn codec_gate_metrics_v1(
    values: &[f32],
    representation_loss: f64,
) -> Option<CodecGateStudyMetricsV1> {
    if values.len() < 48 {
        return None;
    }
    let tail_indices = [17_usize, 26, 27, 31];
    let lane_energy = (tail_indices
        .iter()
        .map(|index| {
            let value = f64::from(values[*index]);
            value * value
        })
        .sum::<f64>()
        / 4.0)
        .sqrt();
    let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
    let total_energy = (values
        .iter()
        .map(|value| {
            let value = f64::from(*value);
            value * value
        })
        .sum::<f64>()
        / f64::from(count.max(1)))
    .sqrt();
    let max_abs = values
        .iter()
        .map(|value| f64::from(value.abs()))
        .fold(0.0_f64, f64::max);
    let clamped = u32::try_from(values.iter().filter(|value| value.abs() >= 4.999).count())
        .unwrap_or(u32::MAX);
    Some(CodecGateStudyMetricsV1 {
        lane_energy,
        total_energy,
        headroom: (5.0_f64 - max_abs).max(0.0),
        clamp_occupancy: f64::from(clamped) / f64::from(count.max(1)),
        representation_loss,
    })
}

fn codec_gate_representation_loss_v1(current: &[f32], candidate: &[f32]) -> f64 {
    let compared = current.len().min(candidate.len());
    if compared == 0 {
        return 0.0;
    }
    let squared = current
        .iter()
        .zip(candidate)
        .take(compared)
        .map(|(left, right)| {
            let delta = f64::from(*left) - f64::from(*right);
            delta * delta
        })
        .sum::<f64>();
    let count = u32::try_from(compared).unwrap_or(u32::MAX);
    (squared / f64::from(count.max(1))).sqrt()
}

pub(crate) fn record_codec_gate_pair_v1(source: &[f32], current: &[f32], gate_disabled: &[f32]) {
    let windows = matching_windows("codec_gate");
    if windows.is_empty() || source.len() < 48 || current.len() < 48 || gate_disabled.len() < 48 {
        return;
    }
    let Ok(source_bytes) = serde_json::to_vec(&source[..48]) else {
        return;
    };
    let source_fixture_sha256 = sha256_bytes(&source_bytes);
    let representation_loss =
        codec_gate_representation_loss_v1(&current[..48], &gate_disabled[..48]);
    let Some(current_metrics) = codec_gate_metrics_v1(&current[..48], 0.0) else {
        return;
    };
    let Some(candidate_metrics) = codec_gate_metrics_v1(&gate_disabled[..48], representation_loss)
    else {
        return;
    };
    let clock = lived_state_witness::clock_sample_v1();
    let process_identity_sha256 = lived_state_witness::runtime_process_identity_sha256_v1();
    let deployment_identity_sha256 = deployment_identity_sha256();
    for window in windows {
        let journey_core = format!(
            "{}\0{}\0{}",
            window.window_id, source_fixture_sha256, clock.monotonic_ns
        );
        let journey_id = format!("codecgatejourney_{}", sha256_bytes(journey_core.as_bytes()));
        for (cohort, metrics) in [
            ("current_entropy_gate", current_metrics.clone()),
            ("gate_disabled_offline", candidate_metrics.clone()),
        ] {
            let sample_core = format!("{journey_id}\0{cohort}");
            let sample = CodecGateStudySampleV1 {
                schema: "codec_gate_study_sample_v1",
                schema_version: 1,
                sample_id: format!("codecgatestudy_{}", sha256_bytes(sample_core.as_bytes())),
                window_id: window.window_id.clone(),
                sample_kind: "codec_gate",
                cohort,
                journey_id: journey_id.clone(),
                observed_at_unix_ms: clock.unix_ms,
                monotonic_time_ns: clock.monotonic_ns,
                process_identity_sha256: process_identity_sha256.clone(),
                deployment_identity_sha256: deployment_identity_sha256.clone(),
                source_fixture_sha256: source_fixture_sha256.clone(),
                metrics,
                counterfactual_dispatched: false,
                behavior_changed: false,
                felt_outcome_inferred: false,
                raw_prose_included: false,
                artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
            };
            if let Ok(value) = serde_json::to_value(sample) {
                enqueue(window.clone(), value);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn begin_heartbeat_attempt_v1(
    phase: f32,
    cadence_seconds: u64,
    intensity: f32,
    signal_norm: Option<f32>,
    pressure: Option<f32>,
    entropy: Option<f32>,
) -> Option<HeartbeatCaptureAttemptV1> {
    let windows = matching_windows("heartbeat");
    if windows.is_empty() {
        return None;
    }
    let clock = lived_state_witness::clock_sample_v1();
    let core = format!(
        "{}\0{}\0{}\0{}",
        clock.unix_ms, clock.monotonic_ns, phase, cadence_seconds
    );
    Some(HeartbeatCaptureAttemptV1 {
        windows,
        attempt_id: format!("heartbeatstudy_{}", sha256_bytes(core.as_bytes())),
        phase,
        cadence_seconds,
        intensity,
        signal_norm,
        pressure,
        entropy,
        monotonic_time_ns: clock.monotonic_ns,
    })
}

impl HeartbeatCaptureAttemptV1 {
    pub(crate) fn record_blocked(self) {
        self.record("blocked", "not_attempted", None);
    }

    pub(crate) fn record_enqueued(self, queue_wait: Duration) {
        self.record(
            "admitted",
            "enqueued",
            Some(queue_wait.as_secs_f64() * 1_000.0),
        );
    }

    pub(crate) fn record_channel_closed(self, queue_wait: Duration) {
        self.record(
            "admitted",
            "channel_closed",
            Some(queue_wait.as_secs_f64() * 1_000.0),
        );
    }

    fn record(
        self,
        admission: &'static str,
        enqueue_outcome: &'static str,
        queue_wait_ms: Option<f64>,
    ) {
        let clock = lived_state_witness::clock_sample_v1();
        let process_identity_sha256 = lived_state_witness::runtime_process_identity_sha256_v1();
        let deployment_identity_sha256 = deployment_identity_sha256();
        for window in self.windows {
            let sample = HeartbeatStudySampleV1 {
                schema: "heartbeat_study_sample_v1",
                schema_version: 1,
                sample_id: format!("{}:{}", self.attempt_id, window.window_id),
                window_id: window.window_id.clone(),
                sample_kind: "heartbeat",
                admission,
                enqueue_outcome,
                observed_at_unix_ms: clock.unix_ms,
                monotonic_time_ns: self.monotonic_time_ns,
                process_identity_sha256: process_identity_sha256.clone(),
                deployment_identity_sha256: deployment_identity_sha256.clone(),
                metrics: HeartbeatStudyMetricsV1 {
                    cadence_seconds: self.cadence_seconds as f64,
                    intensity: self.intensity,
                    pressure: self.pressure,
                    entropy: self.entropy,
                    phase_code: self.phase,
                    signal_norm: self.signal_norm,
                    queue_wait_ms,
                },
                behavior_changed: false,
                felt_outcome_inferred: false,
                raw_prose_included: false,
                artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
            };
            if let Ok(value) = serde_json::to_value(sample) {
                enqueue(window, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untrusted_window_requires_evidence_only_and_hard_limits() {
        let raw = UntrustedWindowV1 {
            schema: "study_window_spec_v1".to_string(),
            schema_version: 1,
            window_id: "studywindow_fixture".to_string(),
            campaign_id: "campaign_fixture".to_string(),
            study_id: "concordance_fixture".to_string(),
            plan_id: "studyplan_fixture".to_string(),
            plan_sha256: "a".repeat(64),
            sample_kinds: vec!["telemetry".to_string()],
            started_at_unix_ms: 1,
            expires_at_unix_ms: 10_000,
            sample_limit: 2_048,
            actor: "test".to_string(),
            capture_can_delay_behavior: false,
            preregistered_before_capture: true,
            raw_prose_included: false,
            artifact_authority_state_v1: UntrustedAuthorityV1 {
                state: "evidence_only".to_string(),
                witness_only: true,
            },
        };
        let trusted = StudyCaptureWindowV1::from_untrusted("studywindow_fixture", raw, 2).unwrap();
        assert_eq!(trusted.sample_limit, 2_048);
    }

    #[test]
    fn no_capture_fast_path_is_bounded() {
        let mut samples = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            let started = Instant::now();
            let _ = matching_windows("unsupported");
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[9_499];
        assert!(p95 < Duration::from_millis(1), "no-capture p95 was {p95:?}");
    }

    #[test]
    fn sample_records_carry_no_vector_or_live_authority() {
        let encoded = serde_json::to_value(HeartbeatStudySampleV1 {
            schema: "heartbeat_study_sample_v1",
            schema_version: 1,
            sample_id: "heartbeatstudy_fixture".to_string(),
            window_id: "studywindow_fixture".to_string(),
            sample_kind: "heartbeat",
            admission: "admitted",
            enqueue_outcome: "enqueued",
            observed_at_unix_ms: 1,
            monotonic_time_ns: 1,
            process_identity_sha256: "a".repeat(64),
            deployment_identity_sha256: "b".repeat(64),
            metrics: HeartbeatStudyMetricsV1 {
                cadence_seconds: 7.0,
                intensity: 0.30,
                pressure: None,
                entropy: None,
                phase_code: 0.0,
                signal_norm: None,
                queue_wait_ms: None,
            },
            behavior_changed: false,
            felt_outcome_inferred: false,
            raw_prose_included: false,
            artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
        })
        .unwrap();
        let text = serde_json::to_string(&encoded).unwrap();
        assert!(!text.contains("\"vector\""));
        assert!(!text.contains("\"live_eligible_now\":true"));
    }
}
