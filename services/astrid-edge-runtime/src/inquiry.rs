use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    codec::encode_text,
    config::Config,
    notebook::ActivityEvent,
    reservoir::{ReservoirSnapshot, SensoryIngress},
    trace::IpcTraceContextV1,
};

mod stats;
mod train;
use stats::{autocorrelation, bounded_cross_correlation, correlation_pairs, linear_slope};
pub(crate) use train::{
    InquiryAdmissionOutcome, InquiryBeliefOperation, InquiryThreadOperation, MAX_INQUIRY_THREADS,
    MAX_PARKED_INQUIRY_THREADS, MAX_THREAD_STARTS_PER_DAY, ThreadAction, VerifiedInquiryStepInput,
    bounded_identifier, parse_thread_action,
};

const REGISTRY_SCHEMA: &str = "astrid_edge_study_registry_v1";
const RECEIPT_SCHEMA: &str = "astrid_edge_study_receipt_v1";
const SAMPLE_SCHEMA: &str = "astrid_edge_study_sample_v1";
const AUTHORITY: &str = "deterministic_machine_study_not_astrid_authorship_or_causal_proof";
const SAMPLE_INTERVAL_MS: u64 = 60_000;
const DAY_MS: u64 = 86_400_000;
const MAX_STARTS_PER_DAY: usize = 4;
const MAX_QUESTION_CHARS: usize = 1_000;
const MAX_SAMPLES: usize = 2_880;
const SNAPSHOT_FRESHNESS_MS: u64 = 5_000;
const KNOWN_CADENCE_MINUTES: [usize; 5] = [3, 5, 10, 15, 60];

pub type SharedStudyManager = Arc<Mutex<StudyManager>>;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct StudySpec {
    pub primary_metric: String,
    pub secondary_metric: Option<String>,
    pub duration_hours: u8,
    pub question: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct StudyRegistry {
    schema: String,
    active: Option<ActiveStudy>,
    updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct ActiveStudy {
    study_id: String,
    spec: Option<StudySpec>,
    started_at_unix_ms: u64,
    midpoint_at_unix_ms: u64,
    completes_at_unix_ms: u64,
    last_sample_at_unix_ms: u64,
    last_snapshot_generation_id: Option<String>,
    last_snapshot_sequence: u64,
    stale_snapshot_tick_count: u64,
    sample_count: usize,
    midpoint_recorded: bool,
    definition_artifact: String,
    parent_response_sha256: Option<String>,
    trace: Option<IpcTraceContextV1>,
    origin: String,
}

#[derive(Debug, Default)]
struct PendingActivity {
    actions: u64,
    artifacts: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StudySample {
    schema: String,
    study_id: String,
    recorded_at_unix_ms: u64,
    snapshot_generation_id: String,
    snapshot_sequence: u64,
    values: BTreeMap<String, f64>,
    authority: String,
}

#[derive(Debug, Serialize)]
struct StudyReceipt<'a> {
    schema: &'static str,
    phase: &'a str,
    recorded_at_unix_ms: u64,
    study_id: &'a str,
    status: &'a str,
    primary_metric: &'a str,
    secondary_metric: Option<&'a str>,
    duration_hours: u8,
    sample_count: usize,
    artifact_path: Option<&'a str>,
    artifact_sha256: Option<&'a str>,
    parent_response_sha256: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<&'a IpcTraceContextV1>,
    origin: &'a str,
    authority: &'static str,
}

#[derive(Debug)]
pub struct StudyCompletion {
    pub artifact_basename: String,
    pub summary: String,
    pub trace: Option<IpcTraceContextV1>,
    pub parent_response_sha256: Option<String>,
}

#[derive(Debug, Default)]
pub struct StudyManager {
    registry: StudyRegistry,
    pending: PendingActivity,
}

impl StudyManager {
    pub fn load(config: &Config) -> Self {
        let path = config.workspace.join("studies/registry.json");
        let registry = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<StudyRegistry>(&bytes).ok())
            .filter(|value| value.schema == REGISTRY_SCHEMA)
            .unwrap_or_else(|| StudyRegistry {
                schema: REGISTRY_SCHEMA.to_string(),
                ..StudyRegistry::default()
            });
        Self {
            registry,
            pending: PendingActivity::default(),
        }
    }

    pub fn observe_activity(&mut self, event: &ActivityEvent) {
        if event.kind == "sovereign_action_outcome" {
            self.pending.actions = self.pending.actions.saturating_add(1);
        }
        if event.artifact_basename.is_some() {
            self.pending.artifacts = self.pending.artifacts.saturating_add(1);
        }
    }

    #[allow(clippy::too_many_arguments)] // Trace, origin, and evidence context remain explicit.
    pub fn start(
        &mut self,
        config: &Config,
        snapshot: &ReservoirSnapshot,
        now: u64,
        spec: &StudySpec,
        trace: Option<&IpcTraceContextV1>,
        parent_response_sha256: Option<&str>,
        origin: &str,
    ) -> Result<String> {
        if self.registry.active.is_some() {
            bail!("one persistent study is already active");
        }
        if starts_on_day(config, now / DAY_MS) >= MAX_STARTS_PER_DAY {
            bail!("persistent study daily start limit reached");
        }
        ensure_metric_available(snapshot, &spec.primary_metric)?;
        if let Some(metric) = spec.secondary_metric.as_deref() {
            ensure_metric_available(snapshot, metric)?;
        }
        let digest = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}:{}:{}:{now}",
                    spec.primary_metric,
                    spec.secondary_metric.as_deref().unwrap_or("none"),
                    spec.question
                )
                .as_bytes()
            )
        );
        let study_id = format!("study_{now}_{}", digest.get(..8).unwrap_or(digest.as_str()));
        let duration_ms = u64::from(spec.duration_hours)
            .saturating_mul(60)
            .saturating_mul(60_000);
        let midpoint_at = now.saturating_add(duration_ms / 2);
        let completes_at = now.saturating_add(duration_ms);
        let definition_name = format!("{study_id}.md");
        let definition_relative = format!("studies/definitions/{definition_name}");
        let definition = format!(
            "# {} persistent study\n\n\
             Study ID: `{study_id}`\n\
             Started: {now} ms since Unix epoch\n\
             Completes: {completes_at} ms since Unix epoch\n\
             Origin: {origin}\n\
             Authority: voluntary Astrid STUDY or labeled operator harness with deterministic machine collection\n\
             Primary metric: `{}`\n\
             Secondary metric: `{}`\n\
             Duration: {} hours\n\n\
             Question: {}\n\n\
             Samples and statistics are machine evidence, not Astrid authorship or causal proof.\n",
            config.instance_name,
            spec.primary_metric,
            spec.secondary_metric.as_deref().unwrap_or("none"),
            spec.duration_hours,
            spec.question,
        );
        write_new_private(
            &config.workspace.join(&definition_relative),
            definition.as_bytes(),
        )?;
        let child_trace = trace.map(IpcTraceContextV1::child);
        let active = ActiveStudy {
            study_id: study_id.clone(),
            spec: Some(spec.clone()),
            started_at_unix_ms: now,
            midpoint_at_unix_ms: midpoint_at,
            completes_at_unix_ms: completes_at,
            last_sample_at_unix_ms: 0,
            last_snapshot_generation_id: None,
            last_snapshot_sequence: 0,
            stale_snapshot_tick_count: 0,
            sample_count: 0,
            midpoint_recorded: false,
            definition_artifact: definition_relative.clone(),
            parent_response_sha256: parent_response_sha256.map(ToOwned::to_owned),
            trace: child_trace,
            origin: origin.to_string(),
        };
        append_receipt(
            config,
            &StudyReceipt {
                schema: RECEIPT_SCHEMA,
                phase: "started",
                recorded_at_unix_ms: now,
                study_id: &study_id,
                status: "active",
                primary_metric: &spec.primary_metric,
                secondary_metric: spec.secondary_metric.as_deref(),
                duration_hours: spec.duration_hours,
                sample_count: 0,
                artifact_path: Some(&definition_relative),
                artifact_sha256: None,
                parent_response_sha256,
                trace: active.trace.as_ref(),
                origin,
                authority: AUTHORITY,
            },
        )?;
        self.registry.schema = REGISTRY_SCHEMA.to_string();
        self.registry.active = Some(active);
        self.registry.updated_at_unix_ms = now;
        persist_registry(config, &self.registry)?;
        Ok(format!("home://edge/{definition_relative}"))
    }

    pub fn cancel(
        &mut self,
        config: &Config,
        now: u64,
        study_id: &str,
        trace: Option<&IpcTraceContextV1>,
        parent_response_sha256: Option<&str>,
    ) -> Result<String> {
        let active = self
            .registry
            .active
            .take()
            .filter(|study| study.study_id == study_id)
            .ok_or_else(|| anyhow::anyhow!("active study identifier does not match"))?;
        let spec = active
            .spec
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("active study has no valid specification"))?;
        append_receipt(
            config,
            &StudyReceipt {
                schema: RECEIPT_SCHEMA,
                phase: "cancelled",
                recorded_at_unix_ms: now,
                study_id,
                status: "cancelled_by_astrid",
                primary_metric: &spec.primary_metric,
                secondary_metric: spec.secondary_metric.as_deref(),
                duration_hours: spec.duration_hours,
                sample_count: active.sample_count,
                artifact_path: Some(&active.definition_artifact),
                artifact_sha256: None,
                parent_response_sha256,
                trace,
                origin: &active.origin,
                authority: AUTHORITY,
            },
        )?;
        self.registry.updated_at_unix_ms = now;
        persist_registry(config, &self.registry)?;
        Ok(format!("home://edge/{}", active.definition_artifact))
    }

    #[allow(clippy::too_many_lines)] // Atomic lifecycle transition retains recovery state together.
    fn tick(
        &mut self,
        config: &Config,
        snapshot: &ReservoirSnapshot,
        now: u64,
    ) -> Result<Option<StudyCompletion>> {
        let Some(mut active) = self.registry.active.take() else {
            self.pending = PendingActivity::default();
            return Ok(None);
        };
        let spec = active
            .spec
            .clone()
            .ok_or_else(|| anyhow::anyhow!("active study specification is missing"))?;
        let sample_due = active.sample_count == 0
            || now.saturating_sub(active.last_sample_at_unix_ms) >= SAMPLE_INTERVAL_MS;
        let snapshot_fresh = snapshot_is_new_for_study(&active, snapshot, now);
        if sample_due && snapshot_fresh && active.sample_count < MAX_SAMPLES {
            let values = study_values(
                config,
                snapshot,
                &self.pending,
                &spec,
                active.last_sample_at_unix_ms,
            );
            if let Err(error) = append_sample(
                config,
                &StudySample {
                    schema: SAMPLE_SCHEMA.to_string(),
                    study_id: active.study_id.clone(),
                    recorded_at_unix_ms: now,
                    snapshot_generation_id: snapshot.generation_id.clone(),
                    snapshot_sequence: snapshot.sequence,
                    values,
                    authority: AUTHORITY.to_string(),
                },
            ) {
                self.registry.active = Some(active);
                return Err(error);
            }
            active.last_sample_at_unix_ms = now;
            active.last_snapshot_generation_id = Some(snapshot.generation_id.clone());
            active.last_snapshot_sequence = snapshot.sequence;
            active.sample_count = active.sample_count.saturating_add(1);
            self.pending = PendingActivity::default();
        } else if sample_due && !snapshot_fresh {
            active.stale_snapshot_tick_count = active.stale_snapshot_tick_count.saturating_add(1);
        }
        if !active.midpoint_recorded && now >= active.midpoint_at_unix_ms {
            if let Err(error) = append_receipt(
                config,
                &StudyReceipt {
                    schema: RECEIPT_SCHEMA,
                    phase: "midpoint",
                    recorded_at_unix_ms: now,
                    study_id: &active.study_id,
                    status: "active_non_triggering_checkpoint",
                    primary_metric: &spec.primary_metric,
                    secondary_metric: spec.secondary_metric.as_deref(),
                    duration_hours: spec.duration_hours,
                    sample_count: active.sample_count,
                    artifact_path: Some(&active.definition_artifact),
                    artifact_sha256: None,
                    parent_response_sha256: active.parent_response_sha256.as_deref(),
                    trace: active.trace.as_ref(),
                    origin: &active.origin,
                    authority: AUTHORITY,
                },
            ) {
                self.registry.active = Some(active);
                return Err(error);
            }
            active.midpoint_recorded = true;
        }
        if now < active.completes_at_unix_ms {
            self.registry.active = Some(active);
            self.registry.updated_at_unix_ms = now;
            persist_registry(config, &self.registry)?;
            return Ok(None);
        }

        let (relative, digest, summary) = match render_result(config, &active, &spec, now) {
            Ok(result) => result,
            Err(error) => {
                self.registry.active = Some(active);
                return Err(error);
            },
        };
        if !receipt_phase_exists(config, &active.study_id, "completed")
            && let Err(error) = append_receipt(
                config,
                &StudyReceipt {
                    schema: RECEIPT_SCHEMA,
                    phase: "completed",
                    recorded_at_unix_ms: now,
                    study_id: &active.study_id,
                    status: "completed",
                    primary_metric: &spec.primary_metric,
                    secondary_metric: spec.secondary_metric.as_deref(),
                    duration_hours: spec.duration_hours,
                    sample_count: active.sample_count,
                    artifact_path: Some(&relative),
                    artifact_sha256: Some(&digest),
                    parent_response_sha256: active.parent_response_sha256.as_deref(),
                    trace: active.trace.as_ref(),
                    origin: &active.origin,
                    authority: AUTHORITY,
                },
            )
        {
            self.registry.active = Some(active);
            return Err(error);
        }
        self.registry.updated_at_unix_ms = now;
        persist_registry(config, &self.registry)?;
        Ok(Some(StudyCompletion {
            artifact_basename: Path::new(&relative)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("study_result.md")
                .to_string(),
            summary,
            trace: active.trace,
            parent_response_sha256: active.parent_response_sha256,
        }))
    }
}

fn snapshot_is_new_for_study(active: &ActiveStudy, snapshot: &ReservoirSnapshot, now: u64) -> bool {
    if snapshot.generation_id.is_empty()
        || snapshot.sequence == 0
        || snapshot.recorded_at_unix_ms == 0
        || now.abs_diff(snapshot.recorded_at_unix_ms) > SNAPSHOT_FRESHNESS_MS
    {
        return false;
    }
    active
        .last_snapshot_generation_id
        .as_ref()
        .is_none_or(|generation| {
            *generation != snapshot.generation_id
                || snapshot.sequence > active.last_snapshot_sequence
        })
}

pub async fn run(
    config: Arc<Config>,
    manager: SharedStudyManager,
    snapshots: watch::Receiver<ReservoirSnapshot>,
    mut activities: broadcast::Receiver<ActivityEvent>,
    ingress_tx: mpsc::Sender<SensoryIngress>,
    activity_tx: broadcast::Sender<ActivityEvent>,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    loop {
        ticker.tick().await;
        loop {
            match activities.try_recv() {
                Ok(event) => {
                    if let Ok(mut guard) = manager.lock() {
                        guard.observe_activity(&event);
                    }
                },
                Err(broadcast::error::TryRecvError::Lagged(_)) => {},
                Err(
                    broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed,
                ) => break,
            }
        }
        let completion = match manager.lock() {
            Ok(mut guard) => guard.tick(&config, &snapshots.borrow(), unix_millis()),
            Err(error) => {
                eprintln!("persistent-study manager lock poisoned: {error}");
                return;
            },
        };
        match completion {
            Ok(Some(completion)) => {
                let _ = activity_tx.send(ActivityEvent {
                    kind: "persistent_study_completed",
                    artifact_basename: Some(completion.artifact_basename.clone()),
                    trace: completion.trace,
                    response_sha256: completion.parent_response_sha256,
                });
                if ingress_tx
                    .send(SensoryIngress::Semantic(encode_text(
                        "study_completion",
                        &completion.summary,
                    )))
                    .await
                    .is_err()
                {
                    return;
                }
            },
            Ok(None) => {},
            Err(error) => eprintln!("persistent-study tick failed: {error}"),
        }
    }
}

pub fn parse_study(argument: &str) -> Option<StudySpec> {
    let (method, question) = argument.split_once("::")?;
    let question = bounded_non_control(question, MAX_QUESTION_CHARS)?;
    let upper = method.to_ascii_uppercase();
    let over = upper.rfind(" OVER ")?;
    let metrics = method.get(..over)?.trim();
    let duration = method.get(over.saturating_add(6)..)?.trim();
    let duration_hours = match duration.to_ascii_lowercase().as_str() {
        "1h" => 1,
        "3h" => 3,
        "6h" => 6,
        "12h" => 12,
        "24h" => 24,
        "48h" => 48,
        _ => return None,
    };
    let metrics_upper = metrics.to_ascii_uppercase();
    let (primary, secondary) = metrics_upper.find(" WITH ").map_or_else(
        || (metrics.trim(), None),
        |index| {
            (
                metrics.get(..index).unwrap_or_default().trim(),
                Some(
                    metrics
                        .get(index.saturating_add(6)..)
                        .unwrap_or_default()
                        .trim(),
                ),
            )
        },
    );
    let primary_metric = normalize_metric(primary)?;
    let secondary_metric = match secondary {
        Some(metric) => Some(normalize_metric(metric)?),
        None => None,
    };
    if secondary_metric.as_deref() == Some(primary_metric.as_str()) {
        return None;
    }
    Some(StudySpec {
        primary_metric,
        secondary_metric,
        duration_hours,
        question,
    })
}

pub fn valid_study_id(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("study_")
        && value.chars().count() <= 96
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_'))
    .then(|| value.to_string())
}

pub fn active_summary(config: &Config) -> String {
    let Some(registry) = fs::read(config.workspace.join("studies/registry.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<StudyRegistry>(&bytes).ok())
    else {
        return "none".to_string();
    };
    let Some(active) = registry.active.as_ref() else {
        return "none".to_string();
    };
    let Some(spec) = active.spec.as_ref() else {
        return "invalid-active-study".to_string();
    };
    format!(
        "{} metrics={}{} samples={} completes={}",
        active.study_id,
        spec.primary_metric,
        spec.secondary_metric
            .as_deref()
            .map_or_else(String::new, |metric| format!("+{metric}")),
        active.sample_count,
        active.completes_at_unix_ms,
    )
}

fn normalize_metric(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    let canonical = match normalized.as_str() {
        "fill" | "fill_ratio" => "fill",
        "effective_dimensionality" | "effective_dimension" | "dimensionality" => {
            "effective_dimensionality"
        },
        "cpu" | "cpu_busy" => "cpu",
        "memory" | "ram" | "memory_used" => "memory",
        "load" | "load_normalized" => "load",
        "disk_read" | "disk_reads" => "disk_read",
        "disk_write" | "disk_writes" => "disk_write",
        "network_receive" | "network_rx" => "network_receive",
        "network_transmit" | "network_tx" => "network_transmit",
        "thermal" | "temperature" => "thermal",
        "audio" | "audio_rms" => "audio_rms",
        "action_rate" | "actions" => "action_rate",
        "artifact_rate" | "artifacts" => "artifact_rate",
        "web_latency" | "web_latency_ms" => "web_latency",
        "generation_latency" | "generation_latency_ms" => "generation_latency",
        "spectral_entropy" | "spectrum_entropy" => "spectral_entropy",
        "lambda1_share" | "leading_mode_share" => "lambda1_share",
        "tail_share" | "spectral_tail_share" => "tail_share",
        "density_gradient" | "spectral_density_gradient" => "density_gradient",
        "mode_turnover" | "spectral_mode_turnover" => "mode_turnover",
        _ => return None,
    };
    Some(canonical.to_string())
}

fn ensure_metric_available(snapshot: &ReservoirSnapshot, metric: &str) -> Result<()> {
    let available = match metric {
        "fill"
        | "effective_dimensionality"
        | "action_rate"
        | "artifact_rate"
        | "web_latency"
        | "generation_latency" => true,
        "audio_rms" => snapshot.audio_rms.is_some(),
        "cpu" => feature(snapshot, "cpu_busy").is_some(),
        "memory" => feature(snapshot, "memory_used").is_some(),
        "load" => feature(snapshot, "load_normalized").is_some(),
        "disk_read" => feature(snapshot, "disk_read_rate").is_some(),
        "disk_write" => feature(snapshot, "disk_write_rate").is_some(),
        "network_receive" => feature(snapshot, "network_receive_rate").is_some(),
        "network_transmit" => feature(snapshot, "network_transmit_rate").is_some(),
        "thermal" => feature(snapshot, "thermal_normalized").is_some(),
        "spectral_entropy" => snapshot.spectral_entropy.is_some(),
        "lambda1_share" => snapshot.lambda1_share.is_some(),
        "tail_share" => snapshot.tail_share.is_some(),
        "density_gradient" => snapshot.density_gradient.is_some(),
        "mode_turnover" => snapshot.mode_turnover.is_some(),
        _ => false,
    };
    if !available {
        bail!("study metric is unavailable on this appliance: {metric}");
    }
    Ok(())
}

fn study_values(
    config: &Config,
    snapshot: &ReservoirSnapshot,
    pending: &PendingActivity,
    spec: &StudySpec,
    since: u64,
) -> BTreeMap<String, f64> {
    [
        Some(spec.primary_metric.as_str()),
        spec.secondary_metric.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|metric| {
        let value = metric_value(config, snapshot, pending, metric, since)?;
        value.is_finite().then(|| (metric.to_string(), value))
    })
    .collect()
}

fn metric_value(
    config: &Config,
    snapshot: &ReservoirSnapshot,
    pending: &PendingActivity,
    metric: &str,
    since: u64,
) -> Option<f64> {
    match metric {
        "fill" => Some(f64::from(snapshot.fill_ratio)),
        "effective_dimensionality" => Some(f64::from(snapshot.effective_dimensionality)),
        "cpu" => feature(snapshot, "cpu_busy"),
        "memory" => feature(snapshot, "memory_used"),
        "load" => feature(snapshot, "load_normalized"),
        "disk_read" => feature(snapshot, "disk_read_rate"),
        "disk_write" => feature(snapshot, "disk_write_rate"),
        "network_receive" => feature(snapshot, "network_receive_rate"),
        "network_transmit" => feature(snapshot, "network_transmit_rate"),
        "thermal" => feature(snapshot, "thermal_normalized"),
        "audio_rms" => snapshot.audio_rms.map(f64::from),
        "action_rate" => Some(f64::from(
            u32::try_from(pending.actions).unwrap_or(u32::MAX),
        )),
        "artifact_rate" => Some(f64::from(
            u32::try_from(pending.artifacts).unwrap_or(u32::MAX),
        )),
        "web_latency" => recent_latency(
            &config.workspace.join("web/receipts.jsonl"),
            since,
            "latency_ms",
        ),
        "generation_latency" => recent_latency(
            &config.workspace.join("autonomous/runs.jsonl"),
            since,
            "elapsed_ms",
        ),
        "spectral_entropy" => snapshot.spectral_entropy,
        "lambda1_share" => snapshot.lambda1_share,
        "tail_share" => snapshot.tail_share,
        "density_gradient" => snapshot.density_gradient,
        "mode_turnover" => snapshot.mode_turnover,
        _ => None,
    }
}

fn feature(snapshot: &ReservoirSnapshot, name: &str) -> Option<f64> {
    snapshot
        .aux_features
        .get(name)
        .copied()
        .flatten()
        .map(f64::from)
}

fn recent_latency(path: &Path, since: u64, field: &str) -> Option<f64> {
    let content = fs::read_to_string(path).ok()?;
    let values = content
        .lines()
        .rev()
        .take(128)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .take_while(|value| recorded_at(value).is_none_or(|timestamp| timestamp >= since))
        .filter_map(|value| value.get(field).and_then(serde_json::Value::as_u64))
        .map(|value| f64::from(u32::try_from(value).unwrap_or(u32::MAX)))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| {
        values.iter().sum::<f64>() / f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX))
    })
}

fn recorded_at(value: &serde_json::Value) -> Option<u64> {
    [
        "recorded_at_unix_ms",
        "completed_at_unix_ms",
        "started_at_unix_ms",
    ]
    .into_iter()
    .find_map(|field| value.get(field).and_then(serde_json::Value::as_u64))
}

fn render_result(
    config: &Config,
    active: &ActiveStudy,
    spec: &StudySpec,
    now: u64,
) -> Result<(String, String, String)> {
    let samples = read_samples(config, &active.study_id);
    let mut sections = Vec::new();
    for metric in [
        Some(spec.primary_metric.as_str()),
        spec.secondary_metric.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let values = samples
            .iter()
            .filter_map(|sample| sample.values.get(metric).copied())
            .collect::<Vec<_>>();
        sections.push(render_metric(metric, &values));
    }
    if let Some(secondary) = spec.secondary_metric.as_deref() {
        let pairs = samples
            .iter()
            .filter_map(|sample| {
                Some((
                    *sample.values.get(&spec.primary_metric)?,
                    *sample.values.get(secondary)?,
                ))
            })
            .collect::<Vec<_>>();
        sections.push(format!(
            "### Cross-metric description\n\n- {} with {} zero-lag correlation: {}\n- strongest bounded cross-correlation (±15 minutes): {}\n- This is descriptive association, not causal evidence.\n",
            spec.primary_metric,
            secondary,
            correlation_pairs(&pairs)
                .map_or_else(|| "unavailable".to_string(), |value| format!("{value:.4}")),
            bounded_cross_correlation(&pairs, 15).map_or_else(
                || "unavailable".to_string(),
                |(lag, value)| format!("lag={lag}m r={value:.4}")
            )
        ));
    }
    let relative = format!("studies/results/{}_result.md", active.study_id);
    let body = format!(
        "# {} persistent study result\n\n\
         Study ID: `{}`\n\
         Started: {} ms since Unix epoch\n\
         Completed: {now} ms since Unix epoch\n\
         Authority: `{AUTHORITY}`\n\
         Samples: {} one-minute aggregates\n\
         Stale or duplicate reservoir ticks excluded: {}\n\
         Question origin: {}\n\
         Question: {}\n\n\
         {}\n\
         ## Interpretation boundary\n\n\
         These are deterministic descriptive measurements. Correlation, trend, and cadence alignment do not establish causation. Scheduler, notebook, model, Action, and artifact activity are endogenous candidate causes.\n",
        config.instance_name,
        active.study_id,
        active.started_at_unix_ms,
        samples.len(),
        active.stale_snapshot_tick_count,
        active.origin,
        spec.question,
        sections.join("\n")
    );
    let result_path = config.workspace.join(&relative);
    let persisted = if result_path.exists() {
        let metadata = fs::symlink_metadata(&result_path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("existing study result is not a regular non-symlink file");
        }
        fs::read(&result_path)?
    } else {
        write_new_private(&result_path, body.as_bytes())?;
        body.into_bytes()
    };
    let digest = format!("{:x}", Sha256::digest(&persisted));
    let summary = format!(
        "machine study {} completed with {} samples; result={} sha256={}; not Astrid authorship or causal proof",
        active.study_id,
        samples.len(),
        Path::new(&relative)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("study-result"),
        digest.get(..16).unwrap_or(digest.as_str())
    );
    Ok((relative, digest, summary))
}

fn render_metric(metric: &str, values: &[f64]) -> String {
    if values.len() < 3 {
        return format!(
            "### {metric}\n\n- Insufficient available samples: {} (minimum 3).\n",
            values.len()
        );
    }
    let count = f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX));
    let mean = values.iter().sum::<f64>() / count;
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / count;
    let slope_per_hour = linear_slope(values).map(|slope| slope * 60.0);
    let lag1 = autocorrelation(values, 1);
    let cadence = KNOWN_CADENCE_MINUTES
        .into_iter()
        .filter_map(|minutes| {
            let value = autocorrelation(values, minutes)?;
            (value.abs() >= 0.60).then(|| format!("{minutes}m={value:.3}"))
        })
        .collect::<Vec<_>>();
    format!(
        "### {metric}\n\n\
         - n={}, min={minimum:.4}, mean={mean:.4}, max={maximum:.4}, standard deviation={:.4}\n\
         - linear slope/hour={}\n\
         - lag-1 autocorrelation={}\n\
         - scheduler-cadence aliases={}\n",
        values.len(),
        variance.sqrt(),
        slope_per_hour.map_or_else(|| "unavailable".to_string(), |value| format!("{value:.4}")),
        lag1.map_or_else(|| "unavailable".to_string(), |value| format!("{value:.4}")),
        if cadence.is_empty() {
            "none at |r|>=0.60".to_string()
        } else {
            cadence.join(",")
        }
    )
}

fn read_samples(config: &Config, study_id: &str) -> Vec<StudySample> {
    fs::read_to_string(
        config
            .workspace
            .join(format!("studies/samples/{study_id}.jsonl")),
    )
    .ok()
    .into_iter()
    .flat_map(|content| {
        content
            .lines()
            .filter_map(|line| serde_json::from_str::<StudySample>(line).ok())
            .collect::<Vec<_>>()
    })
    .take(MAX_SAMPLES)
    .collect()
}

fn append_sample(config: &Config, sample: &StudySample) -> Result<()> {
    let path = config
        .workspace
        .join(format!("studies/samples/{}.jsonl", sample.study_id));
    append_json_line(&path, sample)
}

fn append_receipt(config: &Config, receipt: &StudyReceipt<'_>) -> Result<()> {
    append_json_line(&config.workspace.join("studies/receipts.jsonl"), receipt)
}

fn append_json_line<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("open append-only inquiry ledger {}", path.display()))?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn persist_registry(config: &Config, registry: &StudyRegistry) -> Result<()> {
    let path = config.workspace.join("studies/registry.json");
    let temporary = config.workspace.join("studies/registry.json.tmp");
    write_private(&temporary, &serde_json::to_vec_pretty(registry)?)?;
    fs::rename(&temporary, &path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn starts_on_day(config: &Config, day: u64) -> usize {
    fs::read_to_string(config.workspace.join("studies/receipts.jsonl"))
        .ok()
        .into_iter()
        .flat_map(|content| {
            content
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .filter(|value| value.get("phase").and_then(serde_json::Value::as_str) == Some("started"))
        .filter(|value| {
            value
                .get("recorded_at_unix_ms")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|timestamp| timestamp / DAY_MS == day)
        })
        .count()
}

fn receipt_phase_exists(config: &Config, study_id: &str, phase: &str) -> bool {
    fs::read_to_string(config.workspace.join("studies/receipts.jsonl"))
        .ok()
        .is_some_and(|content| {
            content.lines().any(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .is_some_and(|value| {
                        value.get("study_id").and_then(serde_json::Value::as_str) == Some(study_id)
                            && value.get("phase").and_then(serde_json::Value::as_str) == Some(phase)
                    })
            })
        })
}

fn bounded_non_control(value: &str, maximum: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control))
        .then(|| value.to_string())
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests;
