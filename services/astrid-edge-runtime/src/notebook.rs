use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    codec::encode_text,
    config::Config,
    reservoir::{ReservoirSnapshot, SensoryIngress},
    trace::IpcTraceContextV1,
};

const AUTHORITY: &str = "deterministic_machine_observation_not_astrid_authorship";
const MAX_SUMMARY_CHARS: usize = 320;
const MAX_ARTIFACTS: usize = 8;
const DAY_MILLIS: u64 = 86_400_000;

#[derive(Debug, Clone)]
pub struct ActivityEvent {
    pub kind: &'static str,
    pub artifact_basename: Option<String>,
    /// Exact causal identity supplied by the originating IPC/action path.
    ///
    /// This is observational metadata only. A missing trace remains
    /// unattributed; notebook consumers must never join events by timestamp.
    pub trace: Option<IpcTraceContextV1>,
    /// Exact hash of the authored response that caused this event, when one is
    /// available. Machine observations and legacy events leave it absent.
    pub response_sha256: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Direct lane freshness and availability snapshot.
struct Signals {
    fill: f32,
    cpu: Option<f32>,
    memory: Option<f32>,
    load: Option<f32>,
    disk_read: Option<f32>,
    disk_write: Option<f32>,
    network_receive: Option<f32>,
    network_transmit: Option<f32>,
    thermal: Option<f32>,
    audio_rms: Option<f32>,
    semantic_fresh: bool,
    audio_fresh: bool,
    video_fresh: bool,
    aux_fresh: bool,
    audio_source: String,
    video_source: String,
    aux_source: String,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ActivitySummary {
    counts: BTreeMap<String, u64>,
    artifact_basenames: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ObservationCore {
    schema: &'static str,
    recorded_at_unix_ms: u64,
    instance_name: String,
    trigger_classes: Vec<String>,
    causal_class: &'static str,
    current: Signals,
    deltas: BTreeMap<String, f32>,
    availability: BTreeMap<String, bool>,
    sources: BTreeMap<String, String>,
    activity: ActivitySummary,
    summary: String,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ObservationRecord {
    #[serde(flatten)]
    core: ObservationCore,
    record_sha256: String,
}

#[derive(Default)]
struct PendingActivity {
    counts: BTreeMap<String, u64>,
    artifacts: BTreeSet<String>,
}

impl PendingActivity {
    fn push(&mut self, event: ActivityEvent) {
        let count = self.counts.entry(event.kind.to_string()).or_default();
        *count = count.saturating_add(1);
        if let Some(basename) = event.artifact_basename
            && is_safe_basename(&basename)
            && self.artifacts.len() < MAX_ARTIFACTS
        {
            self.artifacts.insert(basename);
        }
    }

    fn take_summary(&mut self) -> ActivitySummary {
        ActivitySummary {
            counts: std::mem::take(&mut self.counts),
            artifact_basenames: std::mem::take(&mut self.artifacts).into_iter().collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.counts.is_empty() && self.artifacts.is_empty()
    }
}

pub async fn run(
    config: Arc<Config>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
    mut activities: broadcast::Receiver<ActivityEvent>,
    ingress_tx: mpsc::Sender<SensoryIngress>,
) {
    let started_at = unix_millis();
    let mut previous = load_latest_signals(&config);
    let mut last_recorded_at = load_latest_recorded_at(&config).unwrap_or_default();
    let mut pending = PendingActivity::default();
    let mut day = unix_millis() / DAY_MILLIS;
    let mut daily_count = count_today(&config, day);
    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    loop {
        ticker.tick().await;
        loop {
            match activities.try_recv() {
                Ok(event) => pending.push(event),
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    pending.push(ActivityEvent {
                        kind: "activity_metadata_lagged",
                        artifact_basename: None,
                        trace: None,
                        response_sha256: None,
                    });
                },
                Err(
                    broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed,
                ) => break,
            }
        }

        let now = unix_millis();
        let current_day = now / DAY_MILLIS;
        if current_day != day {
            day = current_day;
            daily_count = 0;
        }
        if daily_count >= config.perceptual_notebook_max_per_day {
            continue;
        }
        let warmed = now.saturating_sub(started_at)
            >= config
                .perceptual_notebook_warmup_seconds
                .saturating_mul(1_000);
        if !warmed {
            continue;
        }

        let current = Signals::from(&*snapshots.borrow());
        let mut triggers = trigger_classes(previous.as_ref(), &current, !pending.is_empty());
        let elapsed = now.saturating_sub(last_recorded_at);
        let heartbeat_due = last_recorded_at > 0
            && elapsed
                >= config
                    .perceptual_notebook_heartbeat_seconds
                    .saturating_mul(1_000);
        if heartbeat_due {
            triggers.push("quiet_heartbeat".to_string());
        }
        if previous.is_none() {
            triggers = vec!["baseline".to_string()];
        }
        let interval_elapsed = last_recorded_at == 0
            || elapsed
                >= config
                    .perceptual_notebook_interval_seconds
                    .saturating_mul(1_000);
        if triggers.is_empty() || !interval_elapsed {
            continue;
        }

        let activity = pending.take_summary();
        match persist_observation(
            &config,
            now,
            triggers,
            &current,
            previous.as_ref(),
            activity,
        ) {
            Ok(summary) => {
                previous = Some(current);
                last_recorded_at = now;
                daily_count = daily_count.saturating_add(1);
                if ingress_tx
                    .send(SensoryIngress::Semantic(encode_text(
                        "machine_observation",
                        &summary,
                    )))
                    .await
                    .is_err()
                {
                    eprintln!("machine-observation semantic impulse dropped: reservoir closed");
                    return;
                }
            },
            Err(error) => eprintln!("perceptual notebook persistence failed: {error}"),
        }
    }
}

impl From<&ReservoirSnapshot> for Signals {
    fn from(snapshot: &ReservoirSnapshot) -> Self {
        Self {
            fill: snapshot.fill_ratio,
            cpu: feature(snapshot, "cpu_busy"),
            memory: feature(snapshot, "memory_used"),
            load: feature(snapshot, "load_normalized"),
            disk_read: feature(snapshot, "disk_read_rate"),
            disk_write: feature(snapshot, "disk_write_rate"),
            network_receive: feature(snapshot, "network_receive_rate"),
            network_transmit: feature(snapshot, "network_transmit_rate"),
            thermal: feature(snapshot, "thermal_normalized"),
            audio_rms: snapshot.audio_rms,
            semantic_fresh: snapshot.semantic_fresh,
            audio_fresh: snapshot.audio_fresh,
            video_fresh: snapshot.video_fresh,
            aux_fresh: snapshot.aux_fresh,
            audio_source: snapshot.audio_source.clone(),
            video_source: snapshot.video_source.clone(),
            aux_source: snapshot.aux_source.clone(),
        }
    }
}

fn feature(snapshot: &ReservoirSnapshot, name: &str) -> Option<f32> {
    snapshot.aux_features.get(name).copied().flatten()
}

fn trigger_classes(previous: Option<&Signals>, current: &Signals, activity: bool) -> Vec<String> {
    let Some(previous) = previous else {
        return vec!["baseline".to_string()];
    };
    let mut triggers = Vec::new();
    let availability_changed = availability(previous) != availability(current)
        || previous.semantic_fresh != current.semantic_fresh
        || previous.audio_fresh != current.audio_fresh
        || previous.video_fresh != current.video_fresh
        || previous.aux_fresh != current.aux_fresh
        || previous.audio_source != current.audio_source
        || previous.video_source != current.video_source
        || previous.aux_source != current.aux_source;
    if availability_changed {
        triggers.push("availability_freshness_or_source".to_string());
    }
    if any_delta(
        previous,
        current,
        &["cpu", "memory", "load", "thermal"],
        0.15,
    ) {
        triggers.push("host_state_shift".to_string());
    }
    if any_delta(
        previous,
        current,
        &[
            "disk_read",
            "disk_write",
            "network_receive",
            "network_transmit",
        ],
        0.25,
    ) {
        triggers.push("io_rate_shift".to_string());
    }
    if option_delta(previous.audio_rms, current.audio_rms) >= 0.10 {
        triggers.push("audio_shape_shift".to_string());
    }
    if activity {
        triggers.push("completed_activity_or_artifact".to_string());
    }
    triggers
}

fn any_delta(previous: &Signals, current: &Signals, names: &[&str], threshold: f32) -> bool {
    names.iter().any(|name| {
        option_delta(named(previous, name), named(current, name)) + f32::EPSILON >= threshold
    })
}

fn named(signals: &Signals, name: &str) -> Option<f32> {
    match name {
        "cpu" => signals.cpu,
        "memory" => signals.memory,
        "load" => signals.load,
        "disk_read" => signals.disk_read,
        "disk_write" => signals.disk_write,
        "network_receive" => signals.network_receive,
        "network_transmit" => signals.network_transmit,
        "thermal" => signals.thermal,
        _ => None,
    }
}

fn option_delta(previous: Option<f32>, current: Option<f32>) -> f32 {
    match (previous, current) {
        (Some(previous), Some(current)) => (current - previous).abs(),
        _ => 0.0,
    }
}

fn availability(signals: &Signals) -> BTreeMap<String, bool> {
    BTreeMap::from([
        ("cpu".to_string(), signals.cpu.is_some()),
        ("memory".to_string(), signals.memory.is_some()),
        ("load".to_string(), signals.load.is_some()),
        ("disk_read".to_string(), signals.disk_read.is_some()),
        ("disk_write".to_string(), signals.disk_write.is_some()),
        (
            "network_receive".to_string(),
            signals.network_receive.is_some(),
        ),
        (
            "network_transmit".to_string(),
            signals.network_transmit.is_some(),
        ),
        ("thermal".to_string(), signals.thermal.is_some()),
        ("audio_rms".to_string(), signals.audio_rms.is_some()),
    ])
}

fn deltas(previous: Option<&Signals>, current: &Signals) -> BTreeMap<String, f32> {
    let Some(previous) = previous else {
        return BTreeMap::new();
    };
    [
        "cpu",
        "memory",
        "load",
        "disk_read",
        "disk_write",
        "network_receive",
        "network_transmit",
        "thermal",
    ]
    .into_iter()
    .filter_map(|name| {
        Some((
            name.to_string(),
            named(current, name)? - named(previous, name)?,
        ))
    })
    .chain(
        previous
            .audio_rms
            .zip(current.audio_rms)
            .map(|(old, new)| ("audio_rms".to_string(), new - old)),
    )
    .collect()
}

fn persist_observation(
    config: &Config,
    recorded_at_unix_ms: u64,
    trigger_classes: Vec<String>,
    current: &Signals,
    previous: Option<&Signals>,
    activity: ActivitySummary,
) -> Result<String> {
    let causal_class = observation_causal_class(&trigger_classes);
    let summary = bounded_summary(&format!(
        "machine-observed on {}: {}; causal-class {}; fill {:.1}%, cpu {}, memory {}, load {}, thermal {}; activity {}",
        config.instance_name,
        trigger_classes.join("+"),
        causal_class,
        current.fill * 100.0,
        display(current.cpu),
        display(current.memory),
        display(current.load),
        display(current.thermal),
        activity.counts.values().sum::<u64>(),
    ));
    let core = ObservationCore {
        schema: "astrid_edge_machine_observation_v1",
        recorded_at_unix_ms,
        instance_name: config.instance_name.clone(),
        trigger_classes,
        causal_class,
        current: current.clone(),
        deltas: deltas(previous, current),
        availability: availability(current),
        sources: BTreeMap::from([
            ("audio".to_string(), current.audio_source.clone()),
            ("video".to_string(), current.video_source.clone()),
            ("aux".to_string(), current.aux_source.clone()),
        ]),
        activity,
        summary: summary.clone(),
        authority: AUTHORITY,
    };
    let record_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&core)?));
    let record = ObservationRecord {
        core,
        record_sha256,
    };
    let directory = config.workspace.join("perception");
    let observations = directory.join("observations");
    fs::create_dir_all(&observations)?;
    let latest = directory.join("latest.json");
    let temporary = directory.join("latest.json.tmp");
    write_private_file(&temporary, &serde_json::to_vec_pretty(&record)?, false)?;
    fs::rename(&temporary, &latest)?;
    fs::set_permissions(&latest, fs::Permissions::from_mode(0o600))?;

    let ledger = directory.join("observations.jsonl");
    let mut ledger_file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&ledger)?;
    serde_json::to_writer(&mut ledger_file, &record)?;
    ledger_file.write_all(b"\n")?;
    ledger_file.sync_data()?;

    let markdown = observations.join(format!("observation_{recorded_at_unix_ms}.md"));
    let rendered = format!(
        "# Machine observation\n\n- Recorded: `{recorded_at_unix_ms}`\n- Authority: \
         `{AUTHORITY}`\n- Triggers: {}\n- Causal class: `{}`\n\n{}\n\nThis is deterministic machine-observed \
         context, not Astrid-authored memory or a finding.\n",
        record.core.trigger_classes.join(", "),
        record.core.causal_class,
        record.core.summary
    );
    write_private_file(&markdown, rendered.as_bytes(), true)?;
    Ok(summary)
}

fn observation_causal_class(trigger_classes: &[String]) -> &'static str {
    let has_activity = trigger_classes
        .iter()
        .any(|trigger| trigger == "completed_activity_or_artifact");
    let has_exogenous_signal = trigger_classes.iter().any(|trigger| {
        matches!(
            trigger.as_str(),
            "availability_freshness_or_source"
                | "host_state_shift"
                | "io_rate_shift"
                | "audio_shape_shift"
        )
    });
    match (has_exogenous_signal, has_activity) {
        (true, true) => "mixed_host_and_endogenous_runtime_activity",
        (true, false) => "host_or_source_observation",
        (false, true) => "endogenous_runtime_activity_only",
        (false, false)
            if trigger_classes
                .iter()
                .any(|trigger| trigger == "quiet_heartbeat") =>
        {
            "scheduled_quiet_heartbeat"
        },
        (false, false) => "baseline_or_unclassified_machine_observation",
    }
}

fn write_private_file(path: &Path, bytes: &[u8], create_new: bool) -> Result<()> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .mode(0o600)
        .create_new(create_new)
        .create(!create_new)
        .truncate(!create_new);
    let mut file = options
        .open(path)
        .with_context(|| format!("open private notebook file {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn display(value: Option<f32>) -> String {
    value.map_or_else(|| "unavailable".to_string(), |value| format!("{value:.2}"))
}

fn bounded_summary(value: &str) -> String {
    value.chars().take(MAX_SUMMARY_CHARS).collect()
}

fn is_safe_basename(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 128
        && !value.starts_with('.')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains("..")
        && !value.chars().any(char::is_control)
}

fn load_latest_recorded_at(config: &Config) -> Option<u64> {
    let value = read_latest(config)?;
    value
        .get("recorded_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
}

fn load_latest_signals(config: &Config) -> Option<Signals> {
    let value = read_latest(config)?;
    serde_json::from_value(value.get("current")?.clone()).ok()
}

fn read_latest(config: &Config) -> Option<serde_json::Value> {
    let bytes = fs::read(config.workspace.join("perception/latest.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn count_today(config: &Config, day: u64) -> u32 {
    let Ok(content) = fs::read_to_string(config.workspace.join("perception/observations.jsonl"))
    else {
        return 0;
    };
    u32::try_from(
        content
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|value| {
                value
                    .get("recorded_at_unix_ms")
                    .and_then(serde_json::Value::as_u64)
                    .is_some_and(|recorded| recorded / DAY_MILLIS == day)
            })
            .count(),
    )
    .unwrap_or(u32::MAX)
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
    use super::{Signals, observation_causal_class, option_delta, trigger_classes};

    fn signals() -> Signals {
        Signals {
            fill: 0.68,
            cpu: Some(0.2),
            memory: Some(0.4),
            load: Some(0.1),
            disk_read: Some(0.0),
            disk_write: Some(0.0),
            network_receive: Some(0.0),
            network_transmit: Some(0.0),
            thermal: None,
            audio_rms: None,
            semantic_fresh: false,
            audio_fresh: false,
            video_fresh: false,
            aux_fresh: true,
            audio_source: "unavailable".to_string(),
            video_source: "unavailable".to_string(),
            aux_source: "linux".to_string(),
        }
    }

    #[test]
    fn fill_only_change_never_triggers_an_observation() {
        let previous = signals();
        let mut current = previous.clone();
        current.fill = 0.72;
        assert!(trigger_classes(Some(&previous), &current, false).is_empty());
    }

    #[test]
    fn thresholds_and_activity_are_deterministic() {
        let previous = signals();
        let mut current = previous.clone();
        current.cpu = Some(0.35);
        current.network_receive = Some(0.25);
        let triggers = trigger_classes(Some(&previous), &current, true);
        assert_eq!(
            triggers,
            [
                "host_state_shift",
                "io_rate_shift",
                "completed_activity_or_artifact"
            ]
        );
        assert!(option_delta(None, Some(1.0)).abs() < f32::EPSILON);
        assert_eq!(
            observation_causal_class(&triggers),
            "mixed_host_and_endogenous_runtime_activity"
        );
        assert_eq!(
            observation_causal_class(&["completed_activity_or_artifact".to_string()]),
            "endogenous_runtime_activity_only"
        );
    }

    #[test]
    fn unavailable_audio_does_not_create_a_false_audio_trigger() {
        let previous = signals();
        let mut current = previous.clone();
        current.audio_rms = Some(0.9);
        assert!(
            !trigger_classes(Some(&previous), &current, false)
                .contains(&"audio_shape_shift".to_string())
        );
    }
}
