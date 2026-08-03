//! Safety-critical reservoir tuning transaction state machine.
//!
//! Parsing, signed persistence, actuator transitions, supervision, and evidence
//! verification remain co-located for this rollout so reviewers can audit every
//! authority-bearing state transition and rollback boundary in one module. The
//! pure spectral analysis lives separately; a later split should follow the
//! signed state/actuator transaction boundary, not divide individual lifecycles.

use std::{
    collections::{BTreeSet, VecDeque},
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use std::{error::Error as StdError, fmt};

use anyhow::{Context as _, Result, bail};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{
    config::Config,
    reservoir::{ReservoirCommand, ReservoirSnapshot, TuningParameters},
    trace::IpcTraceContextV1,
};

const STATE_SCHEMA: &str = "astrid_edge_tuning_state_v1";
const RECEIPT_SCHEMA: &str = "astrid_edge_tuning_receipt_v1";
const EVIDENCE_SCHEMA: &str = "astrid_edge_tuning_evidence_v1";
const AUTHORITY: &str =
    "exact_genuinely_authored_traced_action_with_bounded_reversible_reservoir_authority";
const FIXED_FILL_TARGET: f32 = 0.68;
const DAY_MS: u64 = 86_400_000;
const MINUTE_MS: u64 = 60_000;
const BASELINE_MS: u64 = 10 * MINUTE_MS;
const RECOVERY_MS: u64 = 10 * MINUTE_MS;
const VALIDATION_MS: u64 = 6 * 60 * MINUTE_MS;
const COOLDOWN_MS: u64 = 15 * MINUTE_MS;
const MAX_RECENT_EVIDENCE: usize = 128;
const MAX_DAILY_COUNTERS: usize = 8;
const MIN_BASELINE_SAMPLES: usize = 540;
const TWO_GIB: u64 = 2 * 1_024 * 1_024 * 1_024;
const MAX_SWAP_GROWTH: u64 = 128 * 1_024 * 1_024;
const HINDSIGHT_MAX_AGE_MS: u64 = 2 * 60 * MINUTE_MS;
const HINDSIGHT_MAX_FUTURE_SKEW_MS: u64 = 5 * MINUTE_MS;
const TUNING_LEASE_MS: u64 = 10_000;

#[allow(clippy::trivially_copy_pass_by_ref)] // Serde skip predicates receive references.
const fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TuningParameter {
    InputGain,
    ExplorationScale,
    RegulationStrength,
}

impl TuningParameter {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "input_gain" => Some(Self::InputGain),
            "exploration_scale" => Some(Self::ExplorationScale),
            "regulation_strength" => Some(Self::RegulationStrength),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::InputGain => "input_gain",
            Self::ExplorationScale => "exploration_scale",
            Self::RegulationStrength => "regulation_strength",
        }
    }

    const fn bounds(self) -> (f32, f32) {
        match self {
            Self::InputGain | Self::ExplorationScale => (0.90, 1.10),
            Self::RegulationStrength => (0.85, 1.15),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TuningSpec {
    pub parameter: TuningParameter,
    pub value: f32,
    pub duration_minutes: u8,
    pub hypothesis: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum TuningAction {
    Start(TuningSpec),
    Cancel(String),
    Validate {
        candidate_id: String,
        question: String,
    },
    Adopt {
        candidate_id: String,
        reason: String,
    },
    Revert {
        adoption_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct TuningProvenance {
    pub session_id: String,
    pub authority_turn_id: String,
    pub response_sha256: String,
    pub trace: IpcTraceContextV1,
    pub decision_source: &'static str,
}

impl TuningProvenance {
    fn validate(&self) -> Result<()> {
        if !matches!(
            self.decision_source,
            "scheduler_verified_authored_turn" | "kernel_attested_exact_model_terminal_response"
        ) {
            bail!("tuning requires an exact, unrepaired Astrid declaration");
        }
        if self.session_id.trim().is_empty()
            || self.trace.session_id.as_deref() != Some(self.session_id.as_str())
            || !self.trace.is_supported()
        {
            bail!("tuning requires a supported trace bound to the authored session");
        }
        let authority_turn_id = uuid::Uuid::parse_str(&self.authority_turn_id)
            .context("tuning requires a canonical authority turn identifier")?;
        if authority_turn_id.to_string() != self.authority_turn_id
            || self.trace.turn_id != Some(authority_turn_id)
        {
            bail!("tuning authority turn does not match the host-attested trace");
        }
        if self.response_sha256.len() != 64
            || !self
                .response_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            bail!("tuning requires an exact authored response hash");
        }
        Ok(())
    }

    fn replay_key(&self) -> String {
        format!("{}:{}", self.session_id, self.authority_turn_id)
    }
}

#[derive(Debug)]
pub struct TuningRequest {
    pub action: TuningAction,
    pub provenance: TuningProvenance,
    pub reply: oneshot::Sender<TuningActionResult>,
}

#[derive(Debug, Clone)]
pub struct TuningActionResult {
    pub status: &'static str,
    pub outcome: &'static str,
    pub artifact_path: Option<String>,
    pub tuning_id: Option<String>,
    pub candidate_id: Option<String>,
    pub phase: Option<&'static str>,
}

#[derive(Debug)]
struct FatalReservoirControl(String);

impl fmt::Display for FatalReservoirControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for FatalReservoirControl {}

fn fatal_reservoir_control(error: impl fmt::Display) -> anyhow::Error {
    anyhow::Error::new(FatalReservoirControl(error.to_string()))
}

fn is_fatal_reservoir_control(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<FatalReservoirControl>().is_some())
}

#[derive(Debug)]
struct FatalTuningAuthorityStorage(String);

impl fmt::Display for FatalTuningAuthorityStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for FatalTuningAuthorityStorage {}

fn fatal_tuning_authority_storage(error: impl fmt::Display) -> anyhow::Error {
    anyhow::Error::new(FatalTuningAuthorityStorage(error.to_string()))
}

fn is_fatal_tuning_authority_storage(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<FatalTuningAuthorityStorage>()
            .is_some()
    })
}

fn is_fatal_tuning_manager_failure(error: &anyhow::Error) -> bool {
    is_fatal_reservoir_control(error) || is_fatal_tuning_authority_storage(error)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct EnvironmentIdentity {
    build_id: String,
    policy_sha256: String,
    #[serde(default)]
    config_sha256: String,
    seed: u64,
    reservoir_dimensions: u16,
    sensor_profile_sha256: String,
    #[serde(default)]
    hindsight_identity_sha256: String,
    #[serde(default)]
    hindsight_continuity_valid: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    aux_freshness_required: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    audio_freshness_required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredActionResult {
    status: String,
    outcome: String,
    artifact_path: Option<String>,
    tuning_id: Option<String>,
    candidate_id: Option<String>,
    phase: Option<String>,
}

impl StoredActionResult {
    fn from_runtime(result: &TuningActionResult) -> Self {
        Self {
            status: result.status.to_string(),
            outcome: result.outcome.to_string(),
            artifact_path: result.artifact_path.clone(),
            tuning_id: result.tuning_id.clone(),
            candidate_id: result.candidate_id.clone(),
            phase: result.phase.map(str::to_string),
        }
    }

    fn to_runtime(&self) -> Result<TuningActionResult> {
        Ok(TuningActionResult {
            status: known_status(&self.status)?,
            outcome: known_outcome(&self.outcome)?,
            artifact_path: self.artifact_path.clone(),
            tuning_id: self.tuning_id.clone(),
            candidate_id: self.candidate_id.clone(),
            phase: self.phase.as_deref().map(known_phase).transpose()?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CompletedRequest {
    provenance_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    response_sha256: String,
    action_sha256: String,
    completed_at_unix_ms: u64,
    result: StoredActionResult,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MinuteSample {
    recorded_at_unix_ms: u64,
    fill_ratio: f32,
    available_memory_bytes: Option<u64>,
    swap_used_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ActiveExperiment {
    experiment_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    lease_id: String,
    candidate_id: String,
    spec: TuningSpec,
    phase: String,
    started_at_unix_ms: u64,
    phase_ends_at_unix_ms: u64,
    last_sample_at_unix_ms: u64,
    baseline: TuningParameters,
    #[serde(default)]
    baseline_available_memory_bytes: Option<u64>,
    #[serde(default)]
    baseline_swap_used_bytes: Option<u64>,
    environment: EnvironmentIdentity,
    provenance_key: String,
    parent_response_sha256: String,
    trace: IpcTraceContextV1,
    trial_samples: Vec<MinuteSample>,
    recovery_samples: Vec<MinuteSample>,
    failure_reason: Option<String>,
    outside_streak: u8,
    saturation_streak: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ActiveValidation {
    validation_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    lease_id: String,
    candidate_id: String,
    spec: TuningSpec,
    question: String,
    started_at_unix_ms: u64,
    completes_at_unix_ms: u64,
    last_sample_at_unix_ms: u64,
    environment: EnvironmentIdentity,
    baseline: TuningParameters,
    #[serde(default)]
    baseline_available_memory_bytes: Option<u64>,
    #[serde(default)]
    baseline_swap_used_bytes: Option<u64>,
    qualifying_trial_ids: [String; 2],
    provenance_key: String,
    parent_response_sha256: String,
    trace: IpcTraceContextV1,
    samples: Vec<MinuteSample>,
    failure_reason: Option<String>,
    outside_streak: u8,
    saturation_streak: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TrialEvidence {
    experiment_id: String,
    candidate_id: String,
    spec: TuningSpec,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    environment: EnvironmentIdentity,
    provenance_key: String,
    session_id: String,
    parent_response_sha256: String,
    trace: IpcTraceContextV1,
    sample_count: usize,
    expected_samples: usize,
    recovery_sample_count: usize,
    fill_min: f32,
    fill_mean: f32,
    fill_max: f32,
    shelf_occupancy: f32,
    baseline_available_memory_bytes: Option<u64>,
    baseline_swap_used_bytes: Option<u64>,
    available_memory_min_bytes: Option<u64>,
    swap_growth_bytes: Option<u64>,
    qualifying: bool,
    failure_reason: Option<String>,
    evidence_artifact: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ValidationEvidence {
    validation_id: String,
    candidate_id: String,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    environment: EnvironmentIdentity,
    spec: TuningSpec,
    qualifying_trial_ids: [String; 2],
    provenance_key: String,
    session_id: String,
    parent_response_sha256: String,
    trace: IpcTraceContextV1,
    sample_count: usize,
    expected_samples: usize,
    fill_mean: f32,
    shelf_occupancy: f32,
    baseline_available_memory_bytes: Option<u64>,
    baseline_swap_used_bytes: Option<u64>,
    available_memory_min_bytes: Option<u64>,
    swap_growth_bytes: Option<u64>,
    successful: bool,
    failure_reason: Option<String>,
    evidence_artifact: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StandingAdoption {
    adoption_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    lease_id: String,
    candidate_id: String,
    adopted_at_unix_ms: u64,
    parameter: TuningParameter,
    value: f32,
    baseline: TuningParameters,
    environment: EnvironmentIdentity,
    provenance_key: String,
    session_id: String,
    parent_response_sha256: String,
    trace: IpcTraceContextV1,
    applied_this_process: bool,
    outside_streak: u8,
    saturation_streak: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DailyStartCounter {
    utc_day: u64,
    starts: u32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct TuningState {
    last_safe_baseline: Option<TuningParameters>,
    active_experiment: Option<ActiveExperiment>,
    active_validation: Option<ActiveValidation>,
    trials: Vec<TrialEvidence>,
    validations: Vec<ValidationEvidence>,
    standing_adoption: Option<StandingAdoption>,
    suspended_adoption: Option<StandingAdoption>,
    completed_requests: Vec<CompletedRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    daily_start_counters: Vec<DailyStartCounter>,
    cooldown_until_unix_ms: u64,
    updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedState {
    schema: String,
    payload: TuningState,
    signing_public_key: String,
    payload_sha256: String,
    signature: String,
}

#[derive(Debug, Clone)]
struct LiveSample {
    recorded_at_unix_ms: u64,
    snapshot: ReservoirSnapshot,
    available_memory_bytes: Option<u64>,
    swap_used_bytes: Option<u64>,
    sensor_profile_sha256: String,
}

impl LiveSample {
    fn minute(&self) -> MinuteSample {
        MinuteSample {
            recorded_at_unix_ms: self.recorded_at_unix_ms,
            fill_ratio: self.snapshot.fill_ratio,
            available_memory_bytes: self.available_memory_bytes,
            swap_used_bytes: self.swap_used_bytes,
        }
    }
}

pub fn parse_start(argument: &str) -> Option<TuningSpec> {
    let (settings, hypothesis) = argument.split_once("::")?;
    let hypothesis = bounded_text(hypothesis, 1_000)?;
    let (assignment, duration) = settings.trim().split_once(" FOR ")?;
    let (parameter, value) = assignment.trim().split_once('=')?;
    let parameter = TuningParameter::parse(parameter.trim())?;
    let value = value.trim().parse::<f32>().ok()?;
    let (minimum, maximum) = parameter.bounds();
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return None;
    }
    let duration_minutes = match duration.trim() {
        "5m" => 5,
        "15m" => 15,
        "60m" => 60,
        _ => return None,
    };
    Some(TuningSpec {
        parameter,
        value,
        duration_minutes,
        hypothesis,
    })
}

pub fn parse_id(value: &str, prefix: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with(prefix)
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    .then(|| value.to_string())
}

pub fn parse_id_text(argument: &str, prefix: &str) -> Option<(String, String)> {
    let (identifier, text) = argument.split_once("::")?;
    Some((parse_id(identifier, prefix)?, bounded_text(text, 1_000)?))
}

pub async fn run(
    config: Arc<Config>,
    requests: mpsc::Receiver<TuningRequest>,
    reservoir_tx: mpsc::Sender<ReservoirCommand>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
) {
    let mut manager = match TuningManager::load(&config) {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("reservoir tuning critical state verification failed: {error}");
            return;
        },
    };
    let restart_snapshot = snapshots.borrow().clone();
    if let Err(error) = manager
        .recover_restart(&config, &reservoir_tx, restart_snapshot)
        .await
    {
        eprintln!("reservoir tuning restart recovery failed: {error}");
        return;
    }
    run_loaded_manager(config, requests, reservoir_tx, snapshots, manager).await;
}

async fn run_loaded_manager(
    config: Arc<Config>,
    mut requests: mpsc::Receiver<TuningRequest>,
    reservoir_tx: mpsc::Sender<ReservoirCommand>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
    mut manager: TuningManager,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            biased;
            Some(request) = requests.recv() => {
                let request_snapshot = snapshots.borrow().clone();
                let audit_replay_key = request.provenance.replay_key();
                let audit_provenance = request.provenance.clone();
                let rollback_baseline = TuningParameters::from_snapshot(&request_snapshot);
                let mut fatal_manager_failure = false;
                let result = match manager.handle_request(
                    &config,
                    &reservoir_tx,
                    request_snapshot,
                    request.action,
                    request.provenance,
                ).await {
                    Ok(result) => result,
                    Err(error) => {
                        fatal_manager_failure = is_fatal_tuning_manager_failure(&error);
                        eprintln!("reservoir tuning request failed: {error}");
                        let reason = error.to_string().chars().take(240).collect::<String>();
                        let detail = with_lineage(
                            json!({
                                "provenance_key": audit_replay_key,
                                "reason": reason,
                            }),
                            &audit_provenance.trace,
                            &audit_provenance.response_sha256,
                        );
                        if !fatal_manager_failure
                            && let Err(receipt_error) = manager.receipt(
                                &config,
                                "rejected",
                                unix_millis(),
                                detail,
                            )
                        {
                            fatal_manager_failure = true;
                            eprintln!(
                                "reservoir tuning rejection receipt persistence failed: {receipt_error}"
                            );
                        }
                        if fatal_manager_failure
                            && let Err(rollback_error) = manager
                                .persistence_failure_rollback(
                                    &config,
                                    &reservoir_tx,
                                    unix_millis(),
                                    rollback_baseline,
                                )
                                .await
                        {
                            eprintln!(
                                "reservoir tuning fatal-request rollback failed: {rollback_error}"
                            );
                        }
                        TuningActionResult {
                            status: "failed",
                            outcome: "tuning_request_failed",
                            artifact_path: None,
                            tuning_id: None,
                            candidate_id: None,
                            phase: Some(if fatal_manager_failure { "fatal" } else { "rejected" }),
                        }
                    },
                };
                let _ = request.reply.send(result);
                if fatal_manager_failure {
                    return;
                }
            },
            _ = ticker.tick() => {
                let now = unix_millis();
                let snapshot = snapshots.borrow().clone();
                let rollback_baseline = TuningParameters::from_snapshot(&snapshot);
                if let Err(error) = manager.observe(&config, &reservoir_tx, now, snapshot).await {
                    eprintln!("reservoir tuning observation failed: {error}");
                    if let Err(rollback_error) = manager
                        .persistence_failure_rollback(
                            &config,
                            &reservoir_tx,
                            now,
                            rollback_baseline,
                        )
                        .await
                    {
                        eprintln!("reservoir tuning fail-safe rollback failed: {rollback_error}");
                    }
                    return;
                }
            },
            else => return,
        }
    }
}

struct TuningManager {
    state: TuningState,
    key: SigningKey,
    recent: VecDeque<LiveSample>,
    rolling_fill: VecDeque<f32>,
    telemetry_stale_streak: u8,
    #[cfg(test)]
    fail_next_persist: std::sync::atomic::AtomicBool,
}

impl TuningManager {
    fn load(config: &Config) -> Result<Self> {
        let key = load_or_create_signing_key(config)?;
        let state_path = config.workspace.join("tuning/state.json");
        let state = if state_path.exists() {
            verify_state(&state_path, &key)?
        } else {
            TuningState::default()
        };
        let receipts = verified_receipts(config, &key)?;
        reject_replayed_signed_state(&state, &receipts)?;
        Ok(Self {
            state,
            key,
            recent: VecDeque::with_capacity(600),
            rolling_fill: VecDeque::with_capacity(60),
            telemetry_stale_streak: 0,
            #[cfg(test)]
            fail_next_persist: std::sync::atomic::AtomicBool::new(false),
        })
    }

    fn start_count(&self, config: &Config, utc_day: u64) -> Result<u32> {
        let state_count = self
            .state
            .daily_start_counters
            .iter()
            .find(|counter| counter.utc_day == utc_day)
            .map_or(0, |counter| counter.starts);
        let receipt_count = verified_start_count(config, &self.key, utc_day)
            .map_err(fatal_tuning_authority_storage)?;
        Ok(state_count.max(receipt_count))
    }

    fn increment_start_counter(&mut self, utc_day: u64) {
        if let Some(counter) = self
            .state
            .daily_start_counters
            .iter_mut()
            .find(|counter| counter.utc_day == utc_day)
        {
            counter.starts = counter.starts.saturating_add(1);
        } else {
            self.state
                .daily_start_counters
                .push(DailyStartCounter { utc_day, starts: 1 });
        }
        self.state
            .daily_start_counters
            .sort_by_key(|counter| counter.utc_day);
        if self.state.daily_start_counters.len() > MAX_DAILY_COUNTERS {
            self.state.daily_start_counters.drain(
                ..self
                    .state
                    .daily_start_counters
                    .len()
                    .saturating_sub(MAX_DAILY_COUNTERS),
            );
        }
    }

    async fn recover_restart(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        snapshot: ReservoirSnapshot,
    ) -> Result<()> {
        let now = unix_millis();
        let interrupted = self.state.active_experiment.clone();
        let validation = self.state.active_validation.clone();
        if interrupted.is_some() || validation.is_some() {
            let baseline = interrupted
                .as_ref()
                .map(|value| value.baseline.clone())
                .or_else(|| validation.as_ref().map(|value| value.baseline.clone()))
                .unwrap_or_else(TuningParameters::safe_default);
            restore_baseline(reservoir_tx, baseline.clone())
                .await
                .context("restore interrupted tuning baseline on manager restart")?;
            self.state.active_experiment = None;
            self.state.active_validation = None;
            self.state.cooldown_until_unix_ms = self
                .state
                .cooldown_until_unix_ms
                .max(now.saturating_add(COOLDOWN_MS));
            let lineage = interrupted
                .as_ref()
                .map(|value| (&value.trace, value.parent_response_sha256.as_str()))
                .or_else(|| {
                    validation
                        .as_ref()
                        .map(|value| (&value.trace, value.parent_response_sha256.as_str()))
                });
            let mut detail = json!({
                "experiment_id": interrupted.as_ref().map(|value| &value.experiment_id),
                "validation_id": validation.as_ref().map(|value| &value.validation_id),
                "fill_ratio": snapshot.fill_ratio,
                "restored_baseline": baseline,
                "rollback_status": "restored",
                "authority": "machine_restart_recovery_not_astrid_authorship"
            });
            if let Some((trace, response_sha256)) = lineage {
                detail = with_lineage(detail, trace, response_sha256);
            }
            self.state.updated_at_unix_ms = now;
            self.persist(config)?;
            self.receipt(config, "rolled_back_by_restart", now, detail)?;
        }
        if let Some(adoption) = self.state.standing_adoption.as_mut() {
            adoption.applied_this_process = false;
            adoption.lease_id.clear();
        }
        self.state.updated_at_unix_ms = now;
        self.persist(config)
    }

    #[allow(clippy::too_many_lines)] // One auditable transaction owns replay, authority, dispatch, and rollback.
    async fn handle_request(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        snapshot: ReservoirSnapshot,
        action: TuningAction,
        provenance: TuningProvenance,
    ) -> Result<TuningActionResult> {
        provenance.validate()?;
        let replay_key = provenance.replay_key();
        let action_sha256 = action_sha256(&action)?;
        if let Some(completed) = self
            .state
            .completed_requests
            .iter()
            .find(|completed| completed.provenance_key == replay_key)
        {
            if completed.response_sha256 != provenance.response_sha256
                || completed.action_sha256 != action_sha256
            {
                bail!("one traced authored response cannot authorize two tuning Actions");
            }
            return completed
                .result
                .to_runtime()
                .map_err(fatal_tuning_authority_storage);
        }
        if let Some(binding) = receipt_replay_binding(config, &self.key, &replay_key)
            .map_err(fatal_tuning_authority_storage)?
        {
            if binding.response_sha256 != provenance.response_sha256
                || binding.action_sha256 != action_sha256
            {
                bail!("authority turn was already consumed by a different response or Action");
            }
            bail!("prior tuning request did not reach a durable idempotent completion");
        }
        let now = unix_millis();
        self.receipt(
            config,
            "requested",
            now,
            json!({
                "action": action_label(&action),
                "provenance_key": replay_key,
                "authority_turn_id": provenance.authority_turn_id,
                "action_sha256": action_sha256,
                "session_id": provenance.session_id,
                "parent_response_sha256": provenance.response_sha256,
                "trace": provenance.trace,
            }),
        )?;
        if !config.reservoir_tuning_enabled
            && matches!(
                &action,
                TuningAction::Start(_) | TuningAction::Validate { .. } | TuningAction::Adopt { .. }
            )
        {
            self.receipt(
                config,
                "policy_blocked",
                now,
                with_lineage(
                    json!({
                        "provenance_key": replay_key,
                        "reason": "reservoir_tuning_standing_authority_operator_disabled",
                    }),
                    &provenance.trace,
                    &provenance.response_sha256,
                ),
            )?;
            bail!("reservoir tuning standing authority is operator-disabled");
        }
        let prior_state = self.state.clone();
        let prior_parameters = TuningParameters::from_snapshot(&snapshot);
        let rollback_on_finalization_failure = matches!(
            &action,
            TuningAction::Start(_) | TuningAction::Validate { .. } | TuningAction::Adopt { .. }
        );
        let result = match action {
            TuningAction::Start(spec) => {
                self.start(config, reservoir_tx, now, snapshot, spec, &provenance)
                    .await
            },
            TuningAction::Cancel(experiment_id) => {
                self.cancel(config, reservoir_tx, now, &experiment_id, &provenance)
                    .await
            },
            TuningAction::Validate {
                candidate_id,
                question,
            } => {
                self.validate(
                    config,
                    reservoir_tx,
                    now,
                    snapshot,
                    candidate_id,
                    question,
                    &provenance,
                )
                .await
            },
            TuningAction::Adopt {
                candidate_id,
                reason,
            } => {
                self.adopt(
                    config,
                    reservoir_tx,
                    now,
                    snapshot,
                    &candidate_id,
                    &reason,
                    &provenance,
                )
                .await
            },
            TuningAction::Revert {
                adoption_id,
                reason,
            } => {
                self.revert(
                    config,
                    reservoir_tx,
                    now,
                    &adoption_id,
                    &reason,
                    &provenance,
                )
                .await
            },
        }?;
        self.state.completed_requests.push(CompletedRequest {
            provenance_key: replay_key,
            response_sha256: provenance.response_sha256.clone(),
            action_sha256: action_sha256.clone(),
            completed_at_unix_ms: now,
            result: StoredActionResult::from_runtime(&result),
        });
        retain_latest(&mut self.state.completed_requests);
        self.state.updated_at_unix_ms = now;
        if let Err(error) = self.persist(config) {
            if rollback_on_finalization_failure {
                let rollback = self
                    .rollback_current_actuation(reservoir_tx, prior_parameters.clone())
                    .await;
                rollback.context("request finalization failed; reservoir rollback unconfirmed")?;
                let correction_detail = with_lineage(
                    json!({
                        "provenance_key": provenance.replay_key(),
                        "action_sha256": action_sha256,
                        "tuning_id": result.tuning_id,
                        "candidate_id": result.candidate_id,
                        "baseline_requested": prior_parameters,
                        "rollback_status": "restored",
                        "automatic_rollback": true,
                        "reason": "idempotent_completion_persistence_failed",
                    }),
                    &provenance.trace,
                    &provenance.response_sha256,
                );
                let daily_start_counters = self.state.daily_start_counters.clone();
                self.state = prior_state;
                self.state.daily_start_counters = daily_start_counters;
                self.state.cooldown_until_unix_ms = now.saturating_add(COOLDOWN_MS);
                self.state.updated_at_unix_ms = now;
                let state_repair = self.persist(config);
                let correction_receipt = self.receipt(
                    config,
                    "request_finalization_failure_rollback",
                    now,
                    correction_detail,
                );
                state_repair.context("persist state after request-finalization rollback")?;
                correction_receipt.context("record request-finalization rollback correction")?;
            }
            return Err(error).context("persist idempotent tuning completion");
        }
        Ok(result)
    }

    #[allow(clippy::too_many_lines)] // The reversible trial transition is intentionally reviewed as one unit.
    async fn start(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        snapshot: ReservoirSnapshot,
        spec: TuningSpec,
        provenance: &TuningProvenance,
    ) -> Result<TuningActionResult> {
        if self.state.active_experiment.is_some() || self.state.active_validation.is_some() {
            bail!("one reservoir experiment or validation is already active");
        }
        if self.state.standing_adoption.is_some() || self.state.suspended_adoption.is_some() {
            bail!("REVERT_TUNING the existing adoption before starting another candidate trial");
        }
        if now < self.state.cooldown_until_unix_ms {
            bail!("reservoir tuning cooldown remains active");
        }
        let utc_day = now / DAY_MS;
        if self.start_count(config, utc_day)? >= config.reservoir_tuning_max_per_day {
            bail!("reservoir tuning daily start limit reached");
        }
        let live = live_sample(now, snapshot);
        self.require_stable_baseline(&live)?;
        let environment = environment(config, &live);
        let candidate_id = candidate_id(&spec);
        let digest = short_hash(&format!("{}:{now}", provenance.replay_key()));
        let experiment_id = format!("tuning_{now}_{digest}");
        let lease_id = experiment_id.clone();
        self.receipt(
            config,
            "start_slot_reserved",
            now,
            with_lineage(
                json!({
                    "experiment_id": experiment_id,
                    "candidate_id": candidate_id,
                    "provenance_key": provenance.replay_key(),
                    "utc_day": utc_day,
                }),
                &provenance.trace,
                &provenance.response_sha256,
            ),
        )?;
        let mut target = TuningParameters::from_snapshot(&live.snapshot);
        let baseline = target.clone();
        target.set(spec.parameter, spec.value);
        self.state.last_safe_baseline = Some(baseline.clone());
        self.state.active_experiment = Some(ActiveExperiment {
            experiment_id: experiment_id.clone(),
            lease_id: lease_id.clone(),
            candidate_id: candidate_id.clone(),
            spec: spec.clone(),
            phase: "trial".to_string(),
            started_at_unix_ms: now,
            phase_ends_at_unix_ms: now.saturating_add(u64::from(spec.duration_minutes) * MINUTE_MS),
            last_sample_at_unix_ms: now,
            baseline: baseline.clone(),
            baseline_available_memory_bytes: live.available_memory_bytes,
            baseline_swap_used_bytes: live.swap_used_bytes,
            environment,
            provenance_key: provenance.replay_key(),
            parent_response_sha256: provenance.response_sha256.clone(),
            trace: provenance.trace.child(),
            trial_samples: Vec::new(),
            recovery_samples: Vec::new(),
            failure_reason: None,
            outside_streak: 0,
            saturation_streak: 0,
        });
        self.increment_start_counter(utc_day);
        self.state.updated_at_unix_ms = now;
        self.persist(config)?;
        set_reservoir(reservoir_tx, target, baseline.clone(), lease_id.clone()).await?;
        let relative = format!("tuning/evidence/{experiment_id}_definition.json");
        let evidence_result = self.write_evidence(
            config,
            &relative,
            json!({
                "schema": EVIDENCE_SCHEMA,
                "experiment_id": experiment_id,
                "candidate_id": candidate_id,
                "spec": spec,
                "environment": self.state.active_experiment.as_ref().map(|value| &value.environment),
                "session_id": provenance.session_id,
                "parent_response_sha256": provenance.response_sha256,
                "trace": provenance.trace,
                "authority": AUTHORITY,
                "causation_established": false,
            }),
        );
        let receipt_result = evidence_result.and_then(|()| {
            self.receipt(
                config,
                "applied",
                now,
                with_lineage(
                    json!({
                        "experiment_id": experiment_id,
                        "candidate_id": candidate_id,
                        "provenance_key": provenance.replay_key(),
                        "artifact_path": relative,
                        "automatic_rollback": true,
                    }),
                    &provenance.trace,
                    &provenance.response_sha256,
                ),
            )
        });
        if let Err(error) = receipt_result {
            restore_tuning(reservoir_tx, &lease_id)
                .await
                .context("rollback trial after evidence or receipt persistence failure")?;
            self.state.active_experiment = None;
            self.state.cooldown_until_unix_ms = now.saturating_add(COOLDOWN_MS);
            self.state.updated_at_unix_ms = now;
            self.persist(config)?;
            self.receipt(
                config,
                "post_apply_persistence_failure_rollback",
                now,
                with_lineage(
                    json!({"experiment_id": experiment_id}),
                    &provenance.trace,
                    &provenance.response_sha256,
                ),
            )?;
            return Err(error);
        }
        Ok(TuningActionResult {
            status: "executed",
            outcome: "reservoir_tuning_started",
            artifact_path: Some(format!("home://edge/{relative}")),
            tuning_id: Some(experiment_id),
            candidate_id: Some(candidate_id),
            phase: Some("trial"),
        })
    }

    async fn cancel(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        experiment_id: &str,
        provenance: &TuningProvenance,
    ) -> Result<TuningActionResult> {
        let active = self
            .state
            .active_experiment
            .as_ref()
            .context("no reservoir tuning experiment is active")?;
        if active.experiment_id != experiment_id {
            bail!("CANCEL_TUNING identifier does not match the active experiment");
        }
        let lease_id = active.lease_id.clone();
        restore_tuning(reservoir_tx, &lease_id).await?;
        self.begin_recovery(config, now, "cancelled_by_astrid")?;
        self.receipt(
            config,
            "cancelled_rollback_started",
            now,
            with_lineage(
                json!({
                    "experiment_id": experiment_id,
                    "provenance_key": provenance.replay_key(),
                }),
                &provenance.trace,
                &provenance.response_sha256,
            ),
        )?;
        Ok(TuningActionResult {
            status: "executed",
            outcome: "reservoir_tuning_cancelled_and_rolled_back",
            artifact_path: None,
            tuning_id: Some(experiment_id.to_string()),
            candidate_id: None,
            phase: Some("recovery"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn validate(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        snapshot: ReservoirSnapshot,
        candidate_id: String,
        question: String,
        provenance: &TuningProvenance,
    ) -> Result<TuningActionResult> {
        if self.state.active_experiment.is_some() || self.state.active_validation.is_some() {
            bail!("one reservoir experiment or validation is already active");
        }
        if self.state.standing_adoption.is_some() || self.state.suspended_adoption.is_some() {
            bail!("REVERT_TUNING the existing adoption before validating another candidate");
        }
        if now < self.state.cooldown_until_unix_ms {
            bail!("reservoir tuning cooldown remains active");
        }
        let trials = qualifying_pair(&self.state.trials, &candidate_id).context(
            "candidate needs two qualifying 15-minute-or-longer trials at least one hour apart",
        )?;
        let spec = trials[0].spec.clone();
        if trials[1].spec != spec {
            bail!("candidate trials do not bind an identical tuning specification");
        }
        let qualifying_trial_ids = [
            trials[0].experiment_id.clone(),
            trials[1].experiment_id.clone(),
        ];
        let live = live_sample(now, snapshot);
        self.require_stable_baseline(&live)?;
        let environment = environment(config, &live);
        if trials.iter().any(|trial| trial.environment != environment) {
            bail!("candidate evidence environment does not match the current appliance");
        }
        require_valid_hindsight(&environment)?;
        let baseline = TuningParameters::from_snapshot(&live.snapshot);
        let mut target = baseline.clone();
        target.set(spec.parameter, spec.value);
        self.state.last_safe_baseline = Some(baseline.clone());
        let validation_id = format!("validation_{now}_{}", short_hash(&provenance.replay_key()));
        let lease_id = validation_id.clone();
        self.state.active_validation = Some(ActiveValidation {
            validation_id: validation_id.clone(),
            lease_id: lease_id.clone(),
            candidate_id: candidate_id.clone(),
            spec,
            question,
            started_at_unix_ms: now,
            completes_at_unix_ms: now.saturating_add(VALIDATION_MS),
            last_sample_at_unix_ms: now,
            environment,
            baseline: baseline.clone(),
            baseline_available_memory_bytes: live.available_memory_bytes,
            baseline_swap_used_bytes: live.swap_used_bytes,
            qualifying_trial_ids,
            provenance_key: provenance.replay_key(),
            parent_response_sha256: provenance.response_sha256.clone(),
            trace: provenance.trace.child(),
            samples: Vec::new(),
            failure_reason: None,
            outside_streak: 0,
            saturation_streak: 0,
        });
        self.state.updated_at_unix_ms = now;
        self.persist(config)?;
        set_reservoir(reservoir_tx, target, baseline.clone(), lease_id.clone()).await?;
        if let Err(error) = self.receipt(
            config,
            "validation_started",
            now,
            with_lineage(
                json!({
                    "validation_id": validation_id,
                    "candidate_id": candidate_id,
                    "provenance_key": provenance.replay_key(),
                    "duration_minutes": 360,
                    "candidate_applied": true,
                }),
                &provenance.trace,
                &provenance.response_sha256,
            ),
        ) {
            restore_tuning(reservoir_tx, &lease_id)
                .await
                .context("rollback validation after receipt persistence failure")?;
            self.state.active_validation = None;
            self.state.cooldown_until_unix_ms = now.saturating_add(COOLDOWN_MS);
            self.state.updated_at_unix_ms = now;
            self.persist(config)?;
            return Err(error);
        }
        Ok(TuningActionResult {
            status: "executed",
            outcome: "reservoir_tuning_validation_started",
            artifact_path: None,
            tuning_id: Some(validation_id),
            candidate_id: Some(candidate_id),
            phase: Some("validation"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn adopt(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        snapshot: ReservoirSnapshot,
        candidate_id: &str,
        reason: &str,
        provenance: &TuningProvenance,
    ) -> Result<TuningActionResult> {
        if self.state.active_experiment.is_some() || self.state.active_validation.is_some() {
            bail!("cannot adopt while an experiment or validation is active");
        }
        if self.state.standing_adoption.is_some() || self.state.suspended_adoption.is_some() {
            bail!("one standing adoption already exists; REVERT_TUNING it first");
        }
        if now < self.state.cooldown_until_unix_ms {
            bail!("reservoir tuning cooldown remains active");
        }
        let validation = self
            .state
            .validations
            .iter()
            .rev()
            .find(|value| value.candidate_id == candidate_id && value.successful)
            .cloned()
            .context("candidate has no successful six-hour validation")?;
        let bound_trials = validation_trial_pair(&self.state.trials, &validation)
            .context("validation no longer binds its two exact qualifying trials")?;
        let trial = (*bound_trials[0]).clone();
        let live = live_sample(now, snapshot);
        self.require_stable_baseline(&live)?;
        let current_environment = environment(config, &live);
        if validation.environment != current_environment || trial.environment != current_environment
        {
            bail!("validated candidate environment does not match the current appliance");
        }
        require_valid_hindsight(&current_environment)?;
        let baseline = TuningParameters::from_snapshot(&live.snapshot);
        let mut target = baseline.clone();
        target.set(trial.spec.parameter, trial.spec.value);
        self.state.last_safe_baseline = Some(baseline.clone());
        let adoption_id = format!("adoption_{now}_{}", short_hash(&provenance.replay_key()));
        let lease_id = adoption_id.clone();
        set_reservoir(reservoir_tx, target, baseline.clone(), lease_id.clone()).await?;
        self.state.standing_adoption = Some(StandingAdoption {
            adoption_id: adoption_id.clone(),
            lease_id: lease_id.clone(),
            candidate_id: candidate_id.to_string(),
            adopted_at_unix_ms: now,
            parameter: trial.spec.parameter,
            value: trial.spec.value,
            baseline,
            environment: current_environment,
            provenance_key: provenance.replay_key(),
            session_id: provenance.session_id.clone(),
            trace: provenance.trace.child(),
            parent_response_sha256: provenance.response_sha256.clone(),
            applied_this_process: true,
            outside_streak: 0,
            saturation_streak: 0,
        });
        self.state.updated_at_unix_ms = now;
        let transition = self.persist(config).and_then(|()| {
            self.receipt(
                config,
                "adopted",
                now,
                with_lineage(
                    json!({
                        "adoption_id": adoption_id,
                        "candidate_id": candidate_id,
                        "reason": reason,
                        "provenance_key": provenance.replay_key(),
                        "permanent_supervision": true,
                    }),
                    &provenance.trace,
                    &provenance.response_sha256,
                ),
            )
        });
        if let Err(error) = transition {
            restore_tuning(reservoir_tx, &lease_id)
                .await
                .context("rollback adoption after persistence failure")?;
            self.state.standing_adoption.take();
            self.state.updated_at_unix_ms = now;
            let state_repair = self.persist(config);
            state_repair.context("persist adoption rollback")?;
            return Err(error);
        }
        Ok(TuningActionResult {
            status: "executed",
            outcome: "reservoir_tuning_adopted",
            artifact_path: None,
            tuning_id: Some(adoption_id),
            candidate_id: Some(candidate_id.to_string()),
            phase: Some("standing_adoption"),
        })
    }

    async fn revert(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        adoption_id: &str,
        reason: &str,
        provenance: &TuningProvenance,
    ) -> Result<TuningActionResult> {
        let adoption = self
            .state
            .standing_adoption
            .as_ref()
            .or(self.state.suspended_adoption.as_ref())
            .cloned()
            .context("no standing or suspended reservoir tuning adoption exists")?;
        if adoption.adoption_id != adoption_id {
            bail!("REVERT_TUNING identifier does not match the standing or suspended adoption");
        }
        if adoption.applied_this_process {
            restore_tuning(reservoir_tx, &adoption.lease_id).await?;
        } else {
            restore_baseline(reservoir_tx, adoption.baseline.clone()).await?;
        }
        self.state.last_safe_baseline = Some(adoption.baseline.clone());
        self.state.standing_adoption = None;
        self.state.suspended_adoption = None;
        self.state.updated_at_unix_ms = now;
        let transition = self.persist(config).and_then(|()| {
            self.receipt(
                config,
                "adoption_reverted",
                now,
                with_lineage(
                    json!({
                        "adoption_id": adoption_id,
                        "reason": reason,
                        "provenance_key": provenance.replay_key(),
                    }),
                    &provenance.trace,
                    &provenance.response_sha256,
                ),
            )
        });
        if let Err(error) = transition {
            // The actuator is already at the verified baseline. Preserve that
            // fail-safe physical state and retain an explicitly suspended
            // record rather than silently reapplying a standing adoption.
            let mut suspended = adoption;
            suspended.applied_this_process = false;
            self.state.standing_adoption = None;
            self.state.suspended_adoption = Some(suspended);
            self.state.updated_at_unix_ms = now;
            self.persist(config)
                .context("persist safe suspended state after revert failure")?;
            return Err(error);
        }
        Ok(TuningActionResult {
            status: "executed",
            outcome: "reservoir_tuning_adoption_reverted",
            artifact_path: None,
            tuning_id: Some(adoption_id.to_string()),
            candidate_id: None,
            phase: Some("reverted"),
        })
    }

    async fn observe(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        snapshot: ReservoirSnapshot,
    ) -> Result<()> {
        let live = live_sample(now, snapshot);
        self.telemetry_stale_streak = if self
            .recent
            .back()
            .is_some_and(|previous| previous.snapshot.t_ms == live.snapshot.t_ms)
        {
            self.telemetry_stale_streak.saturating_add(1)
        } else {
            0
        };
        self.recent.push_back(live.clone());
        while self
            .recent
            .front()
            .is_some_and(|value| now.saturating_sub(value.recorded_at_unix_ms) > BASELINE_MS)
        {
            self.recent.pop_front();
        }
        self.rolling_fill.push_back(live.snapshot.fill_ratio);
        while self.rolling_fill.len() > 60 {
            self.rolling_fill.pop_front();
        }

        if self.state.active_experiment.is_some() {
            self.observe_experiment(config, reservoir_tx, now, &live)
                .await?;
        } else if self.state.active_validation.is_some() {
            self.observe_validation(config, reservoir_tx, now, &live)
                .await?;
        } else {
            let adoption_applied = self
                .state
                .standing_adoption
                .as_ref()
                .is_some_and(|value| value.applied_this_process);
            if adoption_applied {
                self.supervise_adoption(config, reservoir_tx, now, &live)
                    .await?;
            } else {
                self.maybe_reapply_adoption(config, reservoir_tx, now, &live)
                    .await?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Trial and recovery safety share one serialized lifecycle transition.
    async fn observe_experiment(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        live: &LiveSample,
    ) -> Result<()> {
        let phase = self
            .state
            .active_experiment
            .as_ref()
            .map(|value| value.phase.clone())
            .unwrap_or_default();
        if phase == "trial" {
            if let Some(reason) = safety_failure(
                live,
                self.state.active_experiment.as_mut().expect("checked"),
                &self.rolling_fill,
                self.telemetry_stale_streak >= 3,
            ) {
                let (lease_id, experiment_id, candidate_id, trace, parent_response_sha256) = {
                    let active = self.state.active_experiment.as_ref().expect("checked");
                    (
                        active.lease_id.clone(),
                        active.experiment_id.clone(),
                        active.candidate_id.clone(),
                        active.trace.clone(),
                        active.parent_response_sha256.clone(),
                    )
                };
                restore_tuning(reservoir_tx, &lease_id).await?;
                self.begin_recovery(config, now, &reason)?;
                self.receipt(
                    config,
                    "safety_rollback",
                    now,
                    with_lineage(
                        json!({
                            "experiment_id": experiment_id,
                            "candidate_id": candidate_id,
                            "reason": reason,
                            "automatic_rollback": true,
                        }),
                        &trace,
                        &parent_response_sha256,
                    ),
                )?;
                return Ok(());
            }
            let (
                sampled,
                expired,
                lease_id,
                experiment_id,
                candidate_id,
                trace,
                parent_response_sha256,
            ) = {
                let active = self.state.active_experiment.as_mut().expect("checked");
                let sampled = now.saturating_sub(active.last_sample_at_unix_ms) >= MINUTE_MS;
                if sampled {
                    active.trial_samples.push(live.minute());
                    active.last_sample_at_unix_ms = now;
                }
                (
                    sampled,
                    now >= active.phase_ends_at_unix_ms,
                    active.lease_id.clone(),
                    active.experiment_id.clone(),
                    active.candidate_id.clone(),
                    active.trace.clone(),
                    active.parent_response_sha256.clone(),
                )
            };
            if sampled {
                self.state.updated_at_unix_ms = now;
                self.persist(config)?;
            }
            if expired {
                restore_tuning(reservoir_tx, &lease_id).await?;
                self.begin_recovery(config, now, "automatic_expiry")?;
                self.receipt(
                    config,
                    "expired_rollback_started",
                    now,
                    with_lineage(
                        json!({
                            "experiment_id": experiment_id,
                            "candidate_id": candidate_id,
                            "automatic_rollback": true,
                        }),
                        &trace,
                        &parent_response_sha256,
                    ),
                )?;
            } else {
                renew_tuning_lease(reservoir_tx, &lease_id).await?;
            }
        } else if phase == "recovery" {
            let recovery_failure = {
                let active = self.state.active_experiment.as_mut().expect("checked");
                let recovery_started_at = active.phase_ends_at_unix_ms.saturating_sub(RECOVERY_MS);
                (now >= recovery_started_at.saturating_add(5_000))
                    .then(|| {
                        safety_reason(
                            live,
                            &active.environment,
                            &active.baseline,
                            &mut active.outside_streak,
                            &mut active.saturation_streak,
                            &self.rolling_fill,
                            self.telemetry_stale_streak >= 3,
                        )
                    })
                    .flatten()
            };
            if let Some(reason) = recovery_failure {
                let (baseline, should_record, experiment_id, candidate_id, trace, response_hash) = {
                    let active = self.state.active_experiment.as_mut().expect("checked");
                    let should_record = active.failure_reason.is_none();
                    if should_record {
                        active.failure_reason = Some(format!("recovery_{reason}"));
                    }
                    (
                        active.baseline.clone(),
                        should_record,
                        active.experiment_id.clone(),
                        active.candidate_id.clone(),
                        active.trace.clone(),
                        active.parent_response_sha256.clone(),
                    )
                };
                restore_baseline(reservoir_tx, baseline).await?;
                if should_record {
                    self.state.updated_at_unix_ms = now;
                    self.persist(config)?;
                    self.receipt(
                        config,
                        "recovery_safety_failure",
                        now,
                        with_lineage(
                            json!({
                                "experiment_id": experiment_id,
                                "candidate_id": candidate_id,
                                "reason": reason,
                                "baseline_reasserted": true,
                            }),
                            &trace,
                            &response_hash,
                        ),
                    )?;
                }
            }
            let (sampled, expired) = {
                let active = self.state.active_experiment.as_mut().expect("checked");
                let sampled = now.saturating_sub(active.last_sample_at_unix_ms) >= MINUTE_MS;
                if sampled {
                    active.recovery_samples.push(live.minute());
                    active.last_sample_at_unix_ms = now;
                }
                (sampled, now >= active.phase_ends_at_unix_ms)
            };
            if sampled {
                self.state.updated_at_unix_ms = now;
                self.persist(config)?;
            }
            if expired {
                self.complete_trial(config, now)?;
            }
        }
        Ok(())
    }

    async fn observe_validation(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        live: &LiveSample,
    ) -> Result<()> {
        let failure = {
            let validation = self.state.active_validation.as_mut().expect("checked");
            safety_failure_validation(
                live,
                validation,
                &self.rolling_fill,
                self.telemetry_stale_streak >= 3,
            )
        };
        if let Some(reason) = failure {
            let lease_id = self
                .state
                .active_validation
                .as_ref()
                .context("checked active validation")?
                .lease_id
                .clone();
            restore_tuning(reservoir_tx, &lease_id).await?;
            if let Some(validation) = self.state.active_validation.as_mut() {
                validation.failure_reason = Some(reason.clone());
                validation.completes_at_unix_ms = now;
            }
            let active = self
                .state
                .active_validation
                .as_ref()
                .context("checked active validation")?;
            self.receipt(
                config,
                "validation_safety_rollback",
                now,
                with_lineage(
                    json!({
                        "validation_id": active.validation_id,
                        "candidate_id": active.candidate_id,
                        "reason": reason,
                    }),
                    &active.trace,
                    &active.parent_response_sha256,
                ),
            )?;
            self.complete_validation(config, now)?;
            return Ok(());
        }
        let (sampled, complete) = {
            let validation = self.state.active_validation.as_mut().expect("checked");
            let sampled = now.saturating_sub(validation.last_sample_at_unix_ms) >= MINUTE_MS;
            if sampled {
                validation.samples.push(live.minute());
                validation.last_sample_at_unix_ms = now;
            }
            (sampled, now >= validation.completes_at_unix_ms)
        };
        if sampled {
            self.state.updated_at_unix_ms = now;
            self.persist(config)?;
        }
        if complete {
            let lease_id = self
                .state
                .active_validation
                .as_ref()
                .context("checked active validation")?
                .lease_id
                .clone();
            restore_tuning(reservoir_tx, &lease_id).await?;
            self.complete_validation(config, now)?;
        } else {
            let lease_id = self
                .state
                .active_validation
                .as_ref()
                .context("checked active validation")?
                .lease_id
                .clone();
            renew_tuning_lease(reservoir_tx, &lease_id).await?;
        }
        Ok(())
    }

    fn begin_recovery(&mut self, config: &Config, now: u64, reason: &str) -> Result<()> {
        let active = self
            .state
            .active_experiment
            .as_mut()
            .context("no active tuning experiment")?;
        active.phase = "recovery".to_string();
        active.phase_ends_at_unix_ms = now.saturating_add(RECOVERY_MS);
        active.last_sample_at_unix_ms = now;
        active.outside_streak = 0;
        active.saturation_streak = 0;
        if reason != "automatic_expiry" {
            active.failure_reason = Some(reason.to_string());
        }
        self.state.cooldown_until_unix_ms =
            active.phase_ends_at_unix_ms.saturating_add(COOLDOWN_MS);
        self.state.updated_at_unix_ms = now;
        self.persist(config)
    }

    fn complete_trial(&mut self, config: &Config, now: u64) -> Result<()> {
        let active = self
            .state
            .active_experiment
            .take()
            .context("no active tuning experiment")?;
        let expected = usize::from(active.spec.duration_minutes);
        let stats = sample_stats(&active.trial_samples);
        let swap_growth = swap_growth(active.baseline_swap_used_bytes, &active.trial_samples);
        let coverage = ratio(active.trial_samples.len(), expected);
        let available_memory_min = minimum_optional(
            active.baseline_available_memory_bytes,
            stats.available_memory_min_bytes,
        );
        let memory_ok = available_memory_min.is_some_and(|value| value >= TWO_GIB);
        let swap_ok = swap_growth.is_some_and(|value| value <= MAX_SWAP_GROWTH);
        let qualifying = active.failure_reason.is_none()
            && active.spec.duration_minutes >= 15
            && coverage >= 0.90
            && active.recovery_samples.len() >= 9
            && (0.67..=0.70).contains(&stats.mean)
            && stats.shelf_occupancy >= 0.90
            && memory_ok
            && swap_ok;
        let relative = format!("tuning/evidence/{}_result.json", active.experiment_id);
        let evidence = TrialEvidence {
            experiment_id: active.experiment_id.clone(),
            candidate_id: active.candidate_id.clone(),
            spec: active.spec,
            started_at_unix_ms: active.started_at_unix_ms,
            completed_at_unix_ms: now,
            environment: active.environment,
            provenance_key: active.provenance_key,
            session_id: active.trace.session_id.clone().unwrap_or_default(),
            trace: active.trace.clone(),
            parent_response_sha256: active.parent_response_sha256.clone(),
            sample_count: active.trial_samples.len(),
            expected_samples: expected,
            recovery_sample_count: active.recovery_samples.len(),
            fill_min: stats.minimum,
            fill_mean: stats.mean,
            fill_max: stats.maximum,
            shelf_occupancy: stats.shelf_occupancy,
            baseline_available_memory_bytes: active.baseline_available_memory_bytes,
            baseline_swap_used_bytes: active.baseline_swap_used_bytes,
            available_memory_min_bytes: available_memory_min,
            swap_growth_bytes: swap_growth,
            qualifying,
            failure_reason: active.failure_reason,
            evidence_artifact: relative.clone(),
        };
        self.write_evidence(config, &relative, serde_json::to_value(&evidence)?)?;
        self.state.trials.push(evidence);
        retain_latest(&mut self.state.trials);
        self.state.updated_at_unix_ms = now;
        self.persist(config)?;
        self.receipt(
            config,
            "trial_completed",
            now,
            with_lineage(
                json!({
                    "experiment_id": active.experiment_id,
                    "candidate_id": active.candidate_id,
                    "qualifying": qualifying,
                    "artifact_path": relative,
                    "causation_established": false,
                }),
                &active.trace,
                &active.parent_response_sha256,
            ),
        )
    }

    fn complete_validation(&mut self, config: &Config, now: u64) -> Result<()> {
        let active = self
            .state
            .active_validation
            .take()
            .context("no active tuning validation")?;
        let stats = sample_stats(&active.samples);
        let available_memory_min = minimum_optional(
            active.baseline_available_memory_bytes,
            stats.available_memory_min_bytes,
        );
        let swap_growth = swap_growth(active.baseline_swap_used_bytes, &active.samples);
        let memory_ok = available_memory_min.is_some_and(|value| value >= TWO_GIB);
        let swap_ok = swap_growth.is_some_and(|value| value <= MAX_SWAP_GROWTH);
        let successful = active.failure_reason.is_none()
            && active.samples.len() >= 342
            && (0.67..=0.70).contains(&stats.mean)
            && stats.shelf_occupancy >= 0.90
            && memory_ok
            && swap_ok;
        let relative = format!("tuning/evidence/{}_result.json", active.validation_id);
        let evidence = ValidationEvidence {
            validation_id: active.validation_id.clone(),
            candidate_id: active.candidate_id.clone(),
            started_at_unix_ms: active.started_at_unix_ms,
            completed_at_unix_ms: now,
            environment: active.environment,
            spec: active.spec,
            qualifying_trial_ids: active.qualifying_trial_ids,
            provenance_key: active.provenance_key,
            session_id: active.trace.session_id.clone().unwrap_or_default(),
            trace: active.trace.clone(),
            parent_response_sha256: active.parent_response_sha256.clone(),
            sample_count: active.samples.len(),
            expected_samples: 360,
            fill_mean: stats.mean,
            shelf_occupancy: stats.shelf_occupancy,
            baseline_available_memory_bytes: active.baseline_available_memory_bytes,
            baseline_swap_used_bytes: active.baseline_swap_used_bytes,
            available_memory_min_bytes: available_memory_min,
            swap_growth_bytes: swap_growth,
            successful,
            failure_reason: active.failure_reason,
            evidence_artifact: relative.clone(),
        };
        self.write_evidence(config, &relative, serde_json::to_value(&evidence)?)?;
        self.state.validations.push(evidence);
        retain_latest(&mut self.state.validations);
        self.state.cooldown_until_unix_ms = now.saturating_add(COOLDOWN_MS);
        self.state.updated_at_unix_ms = now;
        self.persist(config)?;
        self.receipt(
            config,
            "validation_completed",
            now,
            with_lineage(
                json!({
                    "validation_id": active.validation_id,
                    "candidate_id": active.candidate_id,
                    "successful": successful,
                    "artifact_path": relative,
                    "causation_established": false,
                }),
                &active.trace,
                &active.parent_response_sha256,
            ),
        )
    }

    async fn maybe_reapply_adoption(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        live: &LiveSample,
    ) -> Result<()> {
        let Some(adoption) = self.state.standing_adoption.as_ref() else {
            return Ok(());
        };
        if adoption.applied_this_process || self.recent.len() < MIN_BASELINE_SAMPLES {
            return Ok(());
        }
        let suspension_reason = if !config.reservoir_tuning_enabled {
            Some("operator_policy_disabled")
        } else if !adoption.environment.hindsight_continuity_valid {
            Some("adoption_hindsight_identity_invalid")
        } else if self.require_stable_baseline(live).is_err() {
            Some("healthy_ten_minute_baseline_unavailable")
        } else if environment(config, live) != adoption.environment {
            Some("environment_identity_mismatch")
        } else {
            None
        };
        if let Some(reason) = suspension_reason {
            let suspended = self.state.standing_adoption.take().expect("checked");
            let lineage_trace = suspended.trace.clone();
            let lineage_hash = suspended.parent_response_sha256.clone();
            let adoption_id = suspended.adoption_id.clone();
            self.state.suspended_adoption = Some(suspended);
            self.state.updated_at_unix_ms = now;
            self.persist(config)?;
            self.receipt(
                config,
                "adoption_suspended_environment_mismatch",
                now,
                with_lineage(
                    json!({"adoption_id": adoption_id, "reason": reason}),
                    &lineage_trace,
                    &lineage_hash,
                ),
            )?;
            return Ok(());
        }
        let adoption = self
            .state
            .standing_adoption
            .as_ref()
            .expect("checked")
            .clone();
        let mut target = adoption.baseline.clone();
        target.set(adoption.parameter, adoption.value);
        let lease_id = format!("{}_{}", adoption.adoption_id, short_hash(&now.to_string()));
        set_reservoir(
            reservoir_tx,
            target,
            adoption.baseline.clone(),
            lease_id.clone(),
        )
        .await?;
        let standing = self.state.standing_adoption.as_mut().expect("checked");
        standing.applied_this_process = true;
        standing.lease_id.clone_from(&lease_id);
        let adoption_id = adoption.adoption_id.clone();
        self.state.updated_at_unix_ms = now;
        let transition = self.persist(config).and_then(|()| {
            self.receipt(
                config,
                "adoption_reapplied_after_healthy_baseline",
                now,
                with_lineage(
                    json!({"adoption_id": adoption_id}),
                    &adoption.trace,
                    &adoption.parent_response_sha256,
                ),
            )
        });
        if let Err(error) = transition {
            restore_tuning(reservoir_tx, &lease_id)
                .await
                .context("rollback failed adoption reapplication")?;
            let mut suspended = adoption;
            suspended.applied_this_process = false;
            self.state.standing_adoption = None;
            self.state.suspended_adoption = Some(suspended);
            self.state.updated_at_unix_ms = now;
            self.persist(config)
                .context("persist suspended adoption after reapply failure")?;
            return Err(error);
        }
        Ok(())
    }

    async fn supervise_adoption(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        live: &LiveSample,
    ) -> Result<()> {
        let current_environment = environment(config, live);
        let reason = {
            let adoption = self
                .state
                .standing_adoption
                .as_mut()
                .context("checked adoption")?;
            if !config.reservoir_tuning_enabled {
                Some("operator_policy_disabled".to_string())
            } else if !current_environment.hindsight_continuity_valid {
                Some("current_hindsight_checkpoint_unavailable_or_stale".to_string())
            } else if current_environment != adoption.environment {
                Some("standing_adoption_environment_identity_changed".to_string())
            } else {
                let mut expected = adoption.baseline.clone();
                expected.set(adoption.parameter, adoption.value);
                safety_reason(
                    live,
                    &adoption.environment,
                    &expected,
                    &mut adoption.outside_streak,
                    &mut adoption.saturation_streak,
                    &self.rolling_fill,
                    self.telemetry_stale_streak >= 3,
                )
            }
        };
        let Some(reason) = reason else {
            let lease_id = self
                .state
                .standing_adoption
                .as_ref()
                .context("checked adoption")?
                .lease_id
                .clone();
            renew_tuning_lease(reservoir_tx, &lease_id).await?;
            return Ok(());
        };
        let adoption = self
            .state
            .standing_adoption
            .take()
            .context("checked adoption")?;
        restore_tuning(reservoir_tx, &adoption.lease_id).await?;
        let adoption_id = adoption.adoption_id.clone();
        let lineage_trace = adoption.trace.clone();
        let lineage_hash = adoption.parent_response_sha256.clone();
        self.state.suspended_adoption = Some(adoption);
        self.state.updated_at_unix_ms = now;
        self.persist(config)?;
        self.receipt(
            config,
            "standing_adoption_safety_suspended",
            now,
            with_lineage(
                json!({
                    "adoption_id": adoption_id,
                    "reason": reason,
                    "automatic_rollback": true,
                }),
                &lineage_trace,
                &lineage_hash,
            ),
        )
    }

    fn require_stable_baseline(&self, live: &LiveSample) -> Result<()> {
        require_known_build_identity()?;
        let baseline_age_ms = self.recent.front().map_or(0, |sample| {
            live.recorded_at_unix_ms
                .saturating_sub(sample.recorded_at_unix_ms)
        });
        if self.recent.len() < MIN_BASELINE_SAMPLES || baseline_age_ms < BASELINE_MS {
            bail!("ten-minute retrospective baseline is not yet available");
        }
        if (live.snapshot.fill_target - FIXED_FILL_TARGET).abs() > 0.000_5 {
            bail!("reservoir tuning authority requires the immutable 0.68 fill target");
        }
        if live
            .available_memory_bytes
            .is_none_or(|value| value < TWO_GIB)
        {
            bail!("at least 2 GiB available memory is required");
        }
        if self
            .recent
            .iter()
            .any(|sample| sample.sensor_profile_sha256 != live.sensor_profile_sha256)
        {
            bail!("required sensor provenance changed during the baseline");
        }
        if self.recent.iter().any(|sample| {
            !healthy_snapshot(
                &sample.snapshot,
                source_available(&live.snapshot.audio_source),
            )
        }) {
            bail!("retrospective baseline is outside the safe reservoir envelope");
        }
        Ok(())
    }

    async fn rollback_current_actuation(
        &self,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        fallback: TuningParameters,
    ) -> Result<TuningParameters> {
        let lease_id = self
            .state
            .active_experiment
            .as_ref()
            .filter(|active| active.phase == "trial")
            .map(|active| active.lease_id.clone())
            .or_else(|| {
                self.state
                    .active_validation
                    .as_ref()
                    .map(|active| active.lease_id.clone())
            })
            .or_else(|| {
                self.state
                    .standing_adoption
                    .as_ref()
                    .filter(|adoption| adoption.applied_this_process)
                    .map(|adoption| adoption.lease_id.clone())
            });
        if let Some(lease_id) = lease_id {
            restore_tuning(reservoir_tx, &lease_id).await
        } else {
            restore_baseline(reservoir_tx, fallback).await
        }
    }

    #[allow(clippy::too_many_lines)] // One fail-safe path keeps lineage, physical rollback, and durable correction together.
    async fn persistence_failure_rollback(
        &mut self,
        config: &Config,
        reservoir_tx: &mpsc::Sender<ReservoirCommand>,
        now: u64,
        fallback: TuningParameters,
    ) -> Result<()> {
        let lineage = self
            .state
            .active_experiment
            .as_ref()
            .map(|active| {
                (
                    active.trace.clone(),
                    active.parent_response_sha256.clone(),
                    json!({
                        "experiment_id": active.experiment_id,
                        "candidate_id": active.candidate_id,
                    }),
                )
            })
            .or_else(|| {
                self.state.active_validation.as_ref().map(|active| {
                    (
                        active.trace.clone(),
                        active.parent_response_sha256.clone(),
                        json!({
                            "validation_id": active.validation_id,
                            "candidate_id": active.candidate_id,
                        }),
                    )
                })
            })
            .or_else(|| {
                self.state.standing_adoption.as_ref().map(|adoption| {
                    (
                        adoption.trace.clone(),
                        adoption.parent_response_sha256.clone(),
                        json!({
                            "adoption_id": adoption.adoption_id,
                            "candidate_id": adoption.candidate_id,
                        }),
                    )
                })
            });
        let baseline = self
            .state
            .active_experiment
            .as_ref()
            .map(|active| active.baseline.clone())
            .or_else(|| {
                self.state
                    .active_validation
                    .as_ref()
                    .map(|active| active.baseline.clone())
            })
            .or_else(|| {
                self.state
                    .standing_adoption
                    .as_ref()
                    .map(|adoption| adoption.baseline.clone())
            })
            .unwrap_or(fallback);
        let exact_lease = self
            .state
            .active_experiment
            .as_ref()
            .filter(|active| active.phase == "trial")
            .map(|active| active.lease_id.clone())
            .or_else(|| {
                self.state
                    .active_validation
                    .as_ref()
                    .map(|active| active.lease_id.clone())
            })
            .or_else(|| {
                self.state
                    .standing_adoption
                    .as_ref()
                    .filter(|adoption| adoption.applied_this_process)
                    .map(|adoption| adoption.lease_id.clone())
            });
        if let Some(lease_id) = exact_lease {
            if let Err(exact_error) = restore_tuning(reservoir_tx, &lease_id).await {
                restore_baseline(reservoir_tx, baseline.clone())
                    .await
                    .with_context(|| {
                        format!(
                            "restore explicit tuning baseline after exact lease rollback failed: {exact_error}"
                        )
                    })?;
            }
        } else {
            restore_baseline(reservoir_tx, baseline.clone())
                .await
                .context("reassert safe baseline after manager failure")?;
        }
        self.state.active_experiment = None;
        self.state.active_validation = None;
        if let Some(mut adoption) = self.state.standing_adoption.take() {
            adoption.applied_this_process = false;
            self.state.suspended_adoption = Some(adoption);
        }
        self.state.cooldown_until_unix_ms = now.saturating_add(COOLDOWN_MS);
        self.state.updated_at_unix_ms = now;
        self.persist(config)?;
        let detail = lineage.map_or_else(
            || json!({"automatic_rollback": true}),
            |(trace, response_sha256, detail)| {
                with_lineage(
                    merge_json_objects(detail, json!({"automatic_rollback": true})),
                    &trace,
                    &response_sha256,
                )
            },
        );
        self.receipt(config, "persistence_failure_rollback", now, detail)
    }

    fn persist(&self, config: &Config) -> Result<()> {
        #[cfg(test)]
        if self
            .fail_next_persist
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(fatal_tuning_authority_storage(
                "injected signed tuning-state persistence failure",
            ));
        }
        persist_state(config, &self.key, &self.state).map_err(fatal_tuning_authority_storage)
    }

    #[allow(clippy::needless_pass_by_value)] // The JSON detail is conceptually consumed by the signed receipt.
    fn receipt(&self, config: &Config, phase: &str, now: u64, detail: Value) -> Result<()> {
        let authored_trigger = matches!(
            phase,
            "requested"
                | "policy_blocked"
                | "rejected"
                | "start_slot_reserved"
                | "applied"
                | "cancelled_rollback_started"
                | "validation_started"
                | "adopted"
                | "adoption_reverted"
        );
        let trace = detail.get("trace").cloned().unwrap_or(Value::Null);
        let session_id = detail.get("session_id").cloned().unwrap_or(Value::Null);
        let chain_id = detail.get("chain_id").cloned().unwrap_or(Value::Null);
        let authority_turn_id = detail
            .get("authority_turn_id")
            .cloned()
            .or_else(|| detail.pointer("/trace/turn_id").cloned())
            .unwrap_or(Value::Null);
        let parent_response_sha256 = detail
            .get("parent_response_sha256")
            .cloned()
            .unwrap_or(Value::Null);
        let payload = json!({
            "schema": RECEIPT_SCHEMA,
            "phase": phase,
            "recorded_at_unix_ms": now,
            "trace": trace,
            "session_id": session_id,
            "chain_id": chain_id,
            "authority_turn_id": authority_turn_id,
            "parent_response_sha256": parent_response_sha256,
            "detail": detail,
            "authority": if authored_trigger {
                AUTHORITY
            } else {
                "deterministic_tuning_manager_safety_or_lifecycle_authority"
            },
            "authorship": "executor_or_machine_record_not_astrid_authorship",
            "trigger_authority": if authored_trigger {
                "verified_genuinely_authored_tuning_action"
            } else {
                "machine_lifecycle_or_safety_transition"
            },
        });
        append_signed(config, &self.key, &payload).map_err(fatal_tuning_authority_storage)
    }

    fn write_evidence(&self, config: &Config, relative: &str, payload: Value) -> Result<()> {
        let envelope =
            signed_envelope(&self.key, payload).map_err(fatal_tuning_authority_storage)?;
        write_new_private_idempotent(
            &config.workspace.join(relative),
            &serde_json::to_vec_pretty(&envelope).map_err(fatal_tuning_authority_storage)?,
        )
        .map_err(fatal_tuning_authority_storage)
    }
}

async fn set_reservoir(
    reservoir_tx: &mpsc::Sender<ReservoirCommand>,
    parameters: TuningParameters,
    baseline: TuningParameters,
    lease_id: String,
) -> Result<TuningParameters> {
    let (reply, response) = oneshot::channel();
    reservoir_tx
        .send(ReservoirCommand::SetTuning {
            parameters,
            baseline,
            lease_id,
            lease_duration_ms: TUNING_LEASE_MS,
            reply,
        })
        .await
        .map_err(fatal_reservoir_control)
        .context("private reservoir command channel closed")?;
    response
        .await
        .map_err(fatal_reservoir_control)
        .context("reservoir command reply dropped")?
        .map_err(fatal_reservoir_control)
}

async fn renew_tuning_lease(
    reservoir_tx: &mpsc::Sender<ReservoirCommand>,
    lease_id: &str,
) -> Result<()> {
    let (reply, response) = oneshot::channel();
    reservoir_tx
        .send(ReservoirCommand::RenewTuningLease {
            lease_id: lease_id.to_string(),
            lease_duration_ms: TUNING_LEASE_MS,
            reply,
        })
        .await
        .map_err(fatal_reservoir_control)
        .context("private reservoir command channel closed")?;
    response
        .await
        .map_err(fatal_reservoir_control)
        .context("reservoir command reply dropped")?
        .map_err(fatal_reservoir_control)
}

async fn restore_tuning(
    reservoir_tx: &mpsc::Sender<ReservoirCommand>,
    lease_id: &str,
) -> Result<TuningParameters> {
    if lease_id.is_empty() {
        bail!("persisted tuning lease identifier is missing");
    }
    let (reply, response) = oneshot::channel();
    reservoir_tx
        .send(ReservoirCommand::RestoreTuning {
            lease_id: lease_id.to_string(),
            reply,
        })
        .await
        .map_err(fatal_reservoir_control)
        .context("private reservoir command channel closed")?;
    response
        .await
        .map_err(fatal_reservoir_control)
        .context("reservoir command reply dropped")?
        .map_err(fatal_reservoir_control)
}

async fn restore_baseline(
    reservoir_tx: &mpsc::Sender<ReservoirCommand>,
    parameters: TuningParameters,
) -> Result<TuningParameters> {
    let (reply, response) = oneshot::channel();
    reservoir_tx
        .send(ReservoirCommand::RestoreBaseline { parameters, reply })
        .await
        .map_err(fatal_reservoir_control)
        .context("private reservoir command channel closed")?;
    response
        .await
        .map_err(fatal_reservoir_control)
        .context("reservoir command reply dropped")?
        .map_err(fatal_reservoir_control)
}

fn safety_failure(
    live: &LiveSample,
    active: &mut ActiveExperiment,
    rolling_fill: &VecDeque<f32>,
    telemetry_stale: bool,
) -> Option<String> {
    let mut expected = active.baseline.clone();
    expected.set(active.spec.parameter, active.spec.value);
    safety_reason(
        live,
        &active.environment,
        &expected,
        &mut active.outside_streak,
        &mut active.saturation_streak,
        rolling_fill,
        telemetry_stale,
    )
}

fn safety_failure_validation(
    live: &LiveSample,
    active: &mut ActiveValidation,
    rolling_fill: &VecDeque<f32>,
    telemetry_stale: bool,
) -> Option<String> {
    let mut expected = active.baseline.clone();
    expected.set(active.spec.parameter, active.spec.value);
    safety_reason(
        live,
        &active.environment,
        &expected,
        &mut active.outside_streak,
        &mut active.saturation_streak,
        rolling_fill,
        telemetry_stale,
    )
}

fn safety_reason(
    live: &LiveSample,
    environment: &EnvironmentIdentity,
    expected_parameters: &TuningParameters,
    outside_streak: &mut u8,
    saturation_streak: &mut u16,
    rolling_fill: &VecDeque<f32>,
    telemetry_stale: bool,
) -> Option<String> {
    let snapshot = &live.snapshot;
    if !healthy_finite(snapshot) {
        return Some("non_finite_reservoir_state".to_string());
    }
    if telemetry_stale {
        return Some("stale_reservoir_telemetry".to_string());
    }
    if let Some(reason) = required_sensor_failure(snapshot, environment) {
        return Some(reason);
    }
    if (snapshot.fill_target - FIXED_FILL_TARGET).abs() > 0.000_5 {
        return Some("immutable_fill_target_drift".to_string());
    }
    if !parameters_match_snapshot(expected_parameters, snapshot) {
        return Some("private_tuning_actuator_drift".to_string());
    }
    if !(0.60..=0.78).contains(&snapshot.fill_ratio) {
        return Some("immediate_fill_safety_boundary".to_string());
    }
    *outside_streak = if (0.62..=0.76).contains(&snapshot.fill_ratio) {
        0
    } else {
        outside_streak.saturating_add(1)
    };
    if *outside_streak >= 3 {
        return Some("three_consecutive_fill_samples_outside_62_76".to_string());
    }
    if rolling_fill.len() >= 60 {
        let mean = rolling_fill.iter().sum::<f32>() / 60.0;
        if !(0.64..=0.74).contains(&mean) {
            return Some("sixty_second_fill_mean_outside_64_74".to_string());
        }
    }
    let saturated = snapshot.exploration_noise <= 0.001_1 || snapshot.exploration_noise >= 1.249;
    *saturation_streak = if saturated {
        saturation_streak.saturating_add(1)
    } else {
        0
    };
    if *saturation_streak >= 60 {
        return Some("prolonged_actuator_saturation".to_string());
    }
    if live
        .snapshot
        .aux_features
        .get("thermal_normalized")
        .and_then(|value| *value)
        .is_some_and(|value| value >= 0.95)
    {
        return Some("thermal_safety_boundary".to_string());
    }
    if live.sensor_profile_sha256 != environment.sensor_profile_sha256 {
        return Some("required_sensor_provenance_changed".to_string());
    }
    None
}

fn healthy_finite(snapshot: &ReservoirSnapshot) -> bool {
    [
        snapshot.fill_ratio,
        snapshot.fill_target,
        snapshot.exploration_noise,
        snapshot.exploration_scale,
        snapshot.input_gain,
        snapshot.regulation_strength,
    ]
    .iter()
    .all(|value| value.is_finite())
}

fn parameters_match_snapshot(expected: &TuningParameters, snapshot: &ReservoirSnapshot) -> bool {
    const EPSILON: f32 = 0.000_01;
    (snapshot.input_gain - expected.input_gain).abs() <= EPSILON
        && (snapshot.exploration_scale - expected.exploration_scale).abs() <= EPSILON
        && (snapshot.regulation_strength - expected.regulation_strength).abs() <= EPSILON
}

fn healthy_snapshot(snapshot: &ReservoirSnapshot, audio_freshness_required: bool) -> bool {
    let environment = EnvironmentIdentity {
        build_id: String::new(),
        policy_sha256: String::new(),
        config_sha256: String::new(),
        seed: 0,
        reservoir_dimensions: 0,
        sensor_profile_sha256: sensor_profile_hash(snapshot),
        hindsight_identity_sha256: String::new(),
        hindsight_continuity_valid: false,
        aux_freshness_required: true,
        audio_freshness_required,
    };
    healthy_finite(snapshot)
        && (snapshot.fill_target - FIXED_FILL_TARGET).abs() <= 0.000_5
        && (0.62..=0.76).contains(&snapshot.fill_ratio)
        && required_sensor_failure(snapshot, &environment).is_none()
        && snapshot
            .aux_features
            .get("thermal_normalized")
            .and_then(|value| *value)
            .is_none_or(|value| value < 0.95)
}

fn required_sensor_failure(
    snapshot: &ReservoirSnapshot,
    environment: &EnvironmentIdentity,
) -> Option<String> {
    if environment.aux_freshness_required
        && (!source_available(&snapshot.aux_source)
            || !snapshot.aux_fresh
            || snapshot
                .aux_features
                .get("thermal_normalized")
                .and_then(|value| *value)
                .is_none_or(|value| !value.is_finite()))
    {
        return Some("required_aux_sensor_unavailable_or_stale".to_string());
    }
    if environment.audio_freshness_required
        && (!source_available(&snapshot.audio_source) || !snapshot.audio_fresh)
    {
        return Some("required_audio_sensor_unavailable_or_stale".to_string());
    }
    None
}

fn source_available(source: &str) -> bool {
    let source = source.trim().to_ascii_lowercase();
    !source.is_empty()
        && !["unavailable", "disabled", "off", "none", "unknown"]
            .iter()
            .any(|marker| source.contains(marker))
}

fn live_sample(now: u64, snapshot: ReservoirSnapshot) -> LiveSample {
    let (available_memory_bytes, swap_used_bytes) = sampled_memory();
    let sensor_profile_sha256 = sensor_profile_hash(&snapshot);
    LiveSample {
        recorded_at_unix_ms: now,
        snapshot,
        available_memory_bytes,
        swap_used_bytes,
        sensor_profile_sha256,
    }
}

#[cfg(not(test))]
fn sampled_memory() -> (Option<u64>, Option<u64>) {
    linux_memory()
}

#[cfg(test)]
const fn sampled_memory() -> (Option<u64>, Option<u64>) {
    // Keep lifecycle tests independent of the developer/CI container's own
    // memory limit while explicit samples still test the evidence gates.
    (Some(TWO_GIB.saturating_mul(2)), Some(0))
}

fn environment(config: &Config, live: &LiveSample) -> EnvironmentIdentity {
    let (hindsight_identity_sha256, hindsight_continuity_valid) = hindsight_identity(config);
    EnvironmentIdentity {
        build_id: format!("{}:{}", env!("CARGO_PKG_VERSION"), source_revision()),
        policy_sha256: format!(
            "{:x}",
            Sha256::digest(
                b"fill=.68;input=.90..1.10;exploration=.90..1.10;regulation=.85..1.15;leak=denied"
            )
        ),
        config_sha256: config_identity_sha256(config),
        seed: config.seed,
        reservoir_dimensions: 128,
        sensor_profile_sha256: live.sensor_profile_sha256.clone(),
        hindsight_identity_sha256,
        hindsight_continuity_valid,
        aux_freshness_required: true,
        audio_freshness_required: source_available(&live.snapshot.audio_source),
    }
}

#[cfg(not(test))]
fn source_revision() -> &'static str {
    option_env!("ASTRID_EDGE_SOURCE_COMMIT").unwrap_or("unknown")
}

#[cfg(test)]
const fn source_revision() -> &'static str {
    "test-source-commit"
}

fn valid_source_revision(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    !value.is_empty()
        && !matches!(value.as_str(), "unknown" | "unrecorded")
        && !value.contains("dirty")
}

fn require_known_build_identity() -> Result<()> {
    if !valid_source_revision(source_revision()) {
        bail!("reservoir tuning requires a clean, recorded source revision");
    }
    Ok(())
}

fn config_identity_sha256(config: &Config) -> String {
    let value = json!({
        "instance_name": config.instance_name,
        "fill_target_bits": config.fill_target.to_bits(),
        "tick_hz": config.tick_hz,
        "seed": config.seed,
        "spectral_enabled": config.spectral_enabled,
        "spectral_rollup_seconds": config.spectral_rollup_seconds,
        "reservoir_tuning_max_per_day": config.reservoir_tuning_max_per_day,
        "reservoir_dimensions": 128,
    });
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value).unwrap_or_default())
    )
}

fn hindsight_identity(config: &Config) -> (String, bool) {
    let Some(state_root) = config
        .workspace
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return ("unavailable".to_string(), false);
    };
    let path = state_root.join("operator/hindsight/latest.json");
    let Ok(bytes) = fs::read(path) else {
        return ("unavailable".to_string(), false);
    };
    let Ok(checkpoint) = serde_json::from_slice::<Value>(&bytes) else {
        return ("invalid".to_string(), false);
    };
    let schema_valid = checkpoint.get("schema").and_then(Value::as_str)
        == Some("astrid_edge_hindsight_checkpoint_v2");
    let now = unix_millis();
    let recorded_at = checkpoint
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let checkpoint_fresh = recorded_at > 0
        && recorded_at <= now.saturating_add(HINDSIGHT_MAX_FUTURE_SKEW_MS)
        && now.saturating_sub(recorded_at) <= HINDSIGHT_MAX_AGE_MS;
    let boot_identity_current = current_boot_id().is_some_and(|boot_id| {
        checkpoint.get("host_boot_id").and_then(Value::as_str) == Some(boot_id.as_str())
    });
    let epoch = checkpoint
        .get("continuity_epoch")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let continuity_valid = checkpoint
        .get("continuity_from_previous_checkpoint_valid")
        .and_then(Value::as_bool)
        == Some(true);
    let current_violations = checkpoint
        .get("current_epoch_integrity_violation_count")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX);
    let operator_database_ok = checkpoint
        .pointer("/operator_hindsight_database/quick_check")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "ok");
    let identity = json!({
        "schema": checkpoint.get("schema"),
        "continuity_epoch": epoch,
        "current_epoch_integrity_violation_count": current_violations,
        "legacy_race_compatible_unresolved_violation_count": checkpoint
            .get("legacy_race_compatible_unresolved_violation_count"),
        "operator_database_schema_version": checkpoint
            .pointer("/operator_hindsight_database/schema_version"),
        "operator_database_quick_check": checkpoint
            .pointer("/operator_hindsight_database/quick_check"),
        "checkpoint_fresh": checkpoint_fresh,
        "host_boot_identity_current": boot_identity_current,
    });
    (
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&identity).unwrap_or_default())
        ),
        schema_valid
            && !epoch.is_empty()
            && continuity_valid
            && current_violations == 0
            && operator_database_ok
            && checkpoint_fresh
            && boot_identity_current,
    )
}

#[cfg(not(test))]
fn current_boot_id() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)] // Match the fallible Linux boot-id interface in unit tests.
fn current_boot_id() -> Option<String> {
    Some("11111111-1111-4111-8111-111111111111".to_string())
}

fn require_valid_hindsight(environment: &EnvironmentIdentity) -> Result<()> {
    if !environment.hindsight_continuity_valid || environment.hindsight_identity_sha256.len() != 64
    {
        bail!("reservoir tuning adoption evidence requires valid current-epoch hindsight");
    }
    Ok(())
}

fn sensor_profile_hash(snapshot: &ReservoirSnapshot) -> String {
    let value = json!({
        "audio_source": snapshot.audio_source,
        "video_source": snapshot.video_source,
        "aux_source": snapshot.aux_source,
        "aux_availability": snapshot.aux_features.iter().map(|(name, value)| (name, value.is_some())).collect::<Vec<_>>(),
    });
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value).unwrap_or_default())
    )
}

fn candidate_id(spec: &TuningSpec) -> String {
    let value = format!(
        "{}:{:08x}:{}",
        spec.parameter.name(),
        spec.value.to_bits(),
        spec.duration_minutes
    );
    format!("candidate_{}", short_hash(&value))
}

fn qualifying_pair<'a>(
    trials: &'a [TrialEvidence],
    candidate_id: &str,
) -> Option<[&'a TrialEvidence; 2]> {
    let mut matching = trials
        .iter()
        .filter(|trial| {
            trial.candidate_id == candidate_id
                && trial.qualifying
                && trial.spec.duration_minutes >= 15
        })
        .collect::<Vec<_>>();
    matching.sort_by_key(|trial| trial.started_at_unix_ms);
    for (index, first) in matching.iter().enumerate() {
        if let Some(second) = matching
            .iter()
            .skip(index.saturating_add(1))
            .find(|second| {
                second
                    .started_at_unix_ms
                    .saturating_sub(first.started_at_unix_ms)
                    >= 60 * MINUTE_MS
                    && second.provenance_key != first.provenance_key
                    && second.trace.trace_id != first.trace.trace_id
                    && second.environment == first.environment
            })
        {
            return Some([*first, *second]);
        }
    }
    None
}

fn validation_trial_pair<'a>(
    trials: &'a [TrialEvidence],
    validation: &ValidationEvidence,
) -> Option<[&'a TrialEvidence; 2]> {
    let first = trials.iter().find(|trial| {
        trial.experiment_id == validation.qualifying_trial_ids[0]
            && trial.candidate_id == validation.candidate_id
            && trial.qualifying
    })?;
    let second = trials.iter().find(|trial| {
        trial.experiment_id == validation.qualifying_trial_ids[1]
            && trial.candidate_id == validation.candidate_id
            && trial.qualifying
    })?;
    (first.spec == validation.spec
        && second.spec == validation.spec
        && first.environment == validation.environment
        && second.environment == validation.environment
        && second
            .started_at_unix_ms
            .saturating_sub(first.started_at_unix_ms)
            >= 60 * MINUTE_MS
        && first.provenance_key != second.provenance_key
        && first.trace.trace_id != second.trace.trace_id)
        .then_some([first, second])
}

struct Stats {
    minimum: f32,
    mean: f32,
    maximum: f32,
    shelf_occupancy: f32,
    available_memory_min_bytes: Option<u64>,
}

fn sample_stats(samples: &[MinuteSample]) -> Stats {
    if samples.is_empty() {
        return Stats {
            minimum: 0.0,
            mean: 0.0,
            maximum: 0.0,
            shelf_occupancy: 0.0,
            available_memory_min_bytes: None,
        };
    }
    let count = f32::from(u16::try_from(samples.len()).unwrap_or(u16::MAX));
    let shelf_count = f32::from(
        u16::try_from(
            samples
                .iter()
                .filter(|value| (0.65..=0.735).contains(&value.fill_ratio))
                .count(),
        )
        .unwrap_or(u16::MAX),
    );
    Stats {
        minimum: samples
            .iter()
            .map(|value| value.fill_ratio)
            .fold(f32::INFINITY, f32::min),
        mean: samples.iter().map(|value| value.fill_ratio).sum::<f32>() / count,
        maximum: samples
            .iter()
            .map(|value| value.fill_ratio)
            .fold(f32::NEG_INFINITY, f32::max),
        shelf_occupancy: shelf_count / count,
        available_memory_min_bytes: samples
            .iter()
            .all(|value| value.available_memory_bytes.is_some())
            .then(|| {
                samples
                    .iter()
                    .filter_map(|value| value.available_memory_bytes)
                    .min()
            })
            .flatten(),
    }
}

fn minimum_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    left.zip(right).map(|(left, right)| left.min(right))
}

fn swap_growth(baseline: Option<u64>, samples: &[MinuteSample]) -> Option<u64> {
    let baseline = baseline?;
    if samples
        .iter()
        .any(|sample| sample.swap_used_bytes.is_none())
    {
        return None;
    }
    let maximum = samples
        .iter()
        .filter_map(|sample| sample.swap_used_bytes)
        .max()
        .unwrap_or(baseline);
    Some(maximum.saturating_sub(baseline))
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        let numerator = f32::from(u16::try_from(numerator).unwrap_or(u16::MAX));
        let denominator = f32::from(u16::try_from(denominator).unwrap_or(u16::MAX));
        numerator / denominator
    }
}

#[cfg(not(test))]
fn linux_memory() -> (Option<u64>, Option<u64>) {
    let Ok(content) = fs::read_to_string("/proc/meminfo") else {
        return (None, None);
    };
    let kib = |name: &str| {
        content.lines().find_map(|line| {
            let (key, rest) = line.split_once(':')?;
            (key == name)
                .then(|| rest.split_whitespace().next()?.parse::<u64>().ok())
                .flatten()
        })
    };
    let available = kib("MemAvailable").map(|value| value.saturating_mul(1_024));
    let total_swap = kib("SwapTotal");
    let free_swap = kib("SwapFree");
    let used_swap = total_swap
        .zip(free_swap)
        .map(|(total, free)| total.saturating_sub(free).saturating_mul(1_024));
    (available, used_swap)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReceiptReplayBinding {
    response_sha256: String,
    action_sha256: String,
}

fn receipt_replay_binding(
    config: &Config,
    key: &SigningKey,
    replay_key: &str,
) -> Result<Option<ReceiptReplayBinding>> {
    let mut binding: Option<ReceiptReplayBinding> = None;
    for payload in verified_receipts(config, key)? {
        if payload.get("phase").and_then(Value::as_str) != Some("requested")
            || payload
                .pointer("/detail/provenance_key")
                .and_then(Value::as_str)
                != Some(replay_key)
        {
            continue;
        }
        let candidate = ReceiptReplayBinding {
            response_sha256: payload
                .pointer("/detail/parent_response_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            action_sha256: payload
                .pointer("/detail/action_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        };
        if candidate.response_sha256.is_empty() || candidate.action_sha256.is_empty() {
            bail!("signed tuning receipt has an incomplete authority-turn binding");
        }
        if binding.as_ref().is_some_and(|prior| prior != &candidate) {
            bail!("signed tuning receipts contain conflicting authority-turn bindings");
        }
        binding = Some(candidate);
    }
    Ok(binding)
}

fn verified_start_count(config: &Config, key: &SigningKey, utc_day: u64) -> Result<u32> {
    let mut starts = BTreeSet::new();
    for payload in verified_receipts(config, key)? {
        let phase = payload.get("phase").and_then(Value::as_str);
        if !matches!(phase, Some("start_slot_reserved" | "applied"))
            || payload
                .get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .is_none_or(|timestamp| timestamp / DAY_MS != utc_day)
        {
            continue;
        }
        if let Some(provenance_key) = payload
            .pointer("/detail/provenance_key")
            .and_then(Value::as_str)
        {
            starts.insert(provenance_key.to_string());
        }
    }
    Ok(u32::try_from(starts.len()).unwrap_or(u32::MAX))
}

fn reject_replayed_signed_state(state: &TuningState, receipts: &[Value]) -> Result<()> {
    if let Some(active) = &state.active_experiment {
        let terminal_after_start = receipts.iter().any(|payload| {
            matches!(
                payload.get("phase").and_then(Value::as_str),
                Some("trial_completed" | "rolled_back_by_restart")
            ) && payload
                .pointer("/detail/experiment_id")
                .and_then(Value::as_str)
                == Some(active.experiment_id.as_str())
                && payload
                    .get("recorded_at_unix_ms")
                    .and_then(Value::as_u64)
                    .is_some_and(|timestamp| timestamp >= active.started_at_unix_ms)
        });
        if terminal_after_start {
            bail!("signed tuning state predates a verified experiment terminal receipt");
        }
    }
    if let Some(active) = &state.active_validation {
        let terminal_after_start = receipts.iter().any(|payload| {
            matches!(
                payload.get("phase").and_then(Value::as_str),
                Some("validation_completed" | "rolled_back_by_restart")
            ) && payload
                .pointer("/detail/validation_id")
                .and_then(Value::as_str)
                == Some(active.validation_id.as_str())
                && payload
                    .get("recorded_at_unix_ms")
                    .and_then(Value::as_u64)
                    .is_some_and(|timestamp| timestamp >= active.started_at_unix_ms)
        });
        if terminal_after_start {
            bail!("signed tuning state predates a verified validation terminal receipt");
        }
    }
    for adoption in state
        .standing_adoption
        .iter()
        .chain(state.suspended_adoption.iter())
    {
        let reverted_after_adoption = receipts.iter().any(|payload| {
            payload.get("phase").and_then(Value::as_str) == Some("adoption_reverted")
                && payload
                    .pointer("/detail/adoption_id")
                    .and_then(Value::as_str)
                    == Some(adoption.adoption_id.as_str())
                && payload
                    .get("recorded_at_unix_ms")
                    .and_then(Value::as_u64)
                    .is_some_and(|timestamp| timestamp >= adoption.adopted_at_unix_ms)
        });
        if reverted_after_adoption {
            bail!("signed tuning state predates a verified adoption-revert tombstone");
        }
    }
    Ok(())
}

fn with_lineage(
    mut detail: Value,
    trace: &IpcTraceContextV1,
    parent_response_sha256: &str,
) -> Value {
    let Value::Object(fields) = &mut detail else {
        return detail;
    };
    fields.insert("trace".to_string(), json!(trace));
    fields.insert(
        "session_id".to_string(),
        trace
            .session_id
            .as_ref()
            .map_or(Value::Null, |value| json!(value)),
    );
    fields.insert(
        "chain_id".to_string(),
        trace
            .chain_id
            .as_ref()
            .map_or(Value::Null, |value| json!(value)),
    );
    fields.insert(
        "authority_turn_id".to_string(),
        trace.turn_id.map_or(Value::Null, |value| json!(value)),
    );
    fields.insert(
        "parent_response_sha256".to_string(),
        json!(parent_response_sha256),
    );
    detail
}

fn merge_json_objects(mut left: Value, right: Value) -> Value {
    if let (Value::Object(left_fields), Value::Object(right_fields)) = (&mut left, right) {
        left_fields.extend(right_fields);
    }
    left
}

fn action_sha256(action: &TuningAction) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(action)?)))
}

fn action_label(action: &TuningAction) -> &'static str {
    match action {
        TuningAction::Start(_) => "tune_reservoir",
        TuningAction::Cancel(_) => "cancel_tuning",
        TuningAction::Validate { .. } => "validate_tuning",
        TuningAction::Adopt { .. } => "adopt_tuning",
        TuningAction::Revert { .. } => "revert_tuning",
    }
}

fn known_status(value: &str) -> Result<&'static str> {
    match value {
        "executed" => Ok("executed"),
        "failed" => Ok("failed"),
        "declined" => Ok("declined"),
        _ => bail!("signed idempotent result contains an unknown status"),
    }
}

fn known_outcome(value: &str) -> Result<&'static str> {
    match value {
        "reservoir_tuning_started" => Ok("reservoir_tuning_started"),
        "reservoir_tuning_cancelled_and_rolled_back" => {
            Ok("reservoir_tuning_cancelled_and_rolled_back")
        },
        "reservoir_tuning_validation_started" => Ok("reservoir_tuning_validation_started"),
        "reservoir_tuning_adopted" => Ok("reservoir_tuning_adopted"),
        "reservoir_tuning_adoption_reverted" => Ok("reservoir_tuning_adoption_reverted"),
        _ => bail!("signed idempotent result contains an unknown outcome"),
    }
}

fn known_phase(value: &str) -> Result<&'static str> {
    match value {
        "trial" => Ok("trial"),
        "recovery" => Ok("recovery"),
        "validation" => Ok("validation"),
        "standing_adoption" => Ok("standing_adoption"),
        "reverted" => Ok("reverted"),
        _ => bail!("signed idempotent result contains an unknown phase"),
    }
}

fn bounded_text(value: &str, maximum: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control))
        .then(|| value.to_string())
}

fn retain_latest<T>(values: &mut Vec<T>) {
    if values.len() > MAX_RECENT_EVIDENCE {
        values.drain(..values.len().saturating_sub(MAX_RECENT_EVIDENCE));
    }
}

fn short_hash(value: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(value.as_bytes()));
    digest.get(..16).unwrap_or(&digest).to_string()
}

fn load_or_create_signing_key(config: &Config) -> Result<SigningKey> {
    let path = config.workspace.join("tuning/signing.key");
    let public_path = config.workspace.join("tuning/signing.pub");
    if path.exists() {
        validate_owner_private_file(&path)?;
        let value = fs::read_to_string(&path)?;
        let key = SigningKey::from_bytes(&decode_fixed::<32>(value.trim())?);
        let expected_public = encode_hex(&key.verifying_key().to_bytes());
        if public_path.exists() {
            validate_owner_private_file(&public_path)?;
            if fs::read_to_string(&public_path)?.trim() != expected_public {
                bail!("tuning public key does not match the owner-only signing key");
            }
        } else {
            write_new_private(&public_path, expected_public.as_bytes())?;
        }
        return Ok(key);
    }
    if public_path.exists() {
        bail!("tuning public key exists without its owner-only signing key");
    }
    if tuning_authority_history_exists(config)? {
        bail!(
            "tuning authority history exists without its signing key; refusing to create a replacement identity"
        );
    }
    let key = SigningKey::generate(&mut OsRng);
    write_new_private(&path, encode_hex(&key.to_bytes()).as_bytes())?;
    write_new_private(
        &public_path,
        encode_hex(&key.verifying_key().to_bytes()).as_bytes(),
    )?;
    Ok(key)
}

fn tuning_authority_history_exists(config: &Config) -> Result<bool> {
    let root = config.workspace.join("tuning");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        match name.as_ref() {
            "signing.key" | "signing.pub" => {},
            "evidence" => {
                let metadata = fs::symlink_metadata(entry.path())?;
                if !metadata.file_type().is_dir()
                    || fs::read_dir(entry.path())?.next().transpose()?.is_some()
                {
                    return Ok(true);
                }
            },
            _ => return Ok(true),
        }
    }
    Ok(false)
}

#[allow(clippy::needless_pass_by_value)] // The payload becomes part of the returned immutable envelope.
fn signed_envelope(key: &SigningKey, payload: Value) -> Result<Value> {
    let bytes = serde_json::to_vec(&payload)?;
    Ok(json!({
        "payload": payload,
        "signing_public_key": encode_hex(&key.verifying_key().to_bytes()),
        "payload_sha256": format!("{:x}", Sha256::digest(&bytes)),
        "signature": encode_hex(&key.sign(&bytes).to_bytes()),
    }))
}

fn append_signed(config: &Config, key: &SigningKey, payload: &Value) -> Result<()> {
    let envelope = signed_envelope(key, payload.clone())?;
    let path = config.workspace.join("tuning/receipts.jsonl");
    let mut options = OpenOptions::new();
    options.append(true).mode(0o600);
    let created = !path.exists();
    let mut file = if created {
        options.create_new(true).open(&path)?
    } else {
        validate_owner_private_file(&path)?;
        // Never append behind a malformed, truncated, foreign-keyed, or
        // signature-invalid authority record. Continuing would turn a detected
        // integrity loss into apparently healthy new history.
        let _ = verified_receipts(config, key)?;
        options.open(&path)?
    };
    validate_owner_private_metadata(&file.metadata()?)?;
    serde_json::to_writer(&mut file, &envelope)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    if created {
        sync_parent(&path)?;
    }
    Ok(())
}

fn persist_state(config: &Config, key: &SigningKey, state: &TuningState) -> Result<()> {
    let payload = serde_json::to_vec(state)?;
    let signed = SignedState {
        schema: STATE_SCHEMA.to_string(),
        payload: state.clone(),
        signing_public_key: encode_hex(&key.verifying_key().to_bytes()),
        payload_sha256: format!("{:x}", Sha256::digest(&payload)),
        signature: encode_hex(&key.sign(&payload).to_bytes()),
    };
    let path = config.workspace.join("tuning/state.json");
    if path.exists() {
        // Refuse to overwrite a corrupt or foreign signed predecessor. The
        // critical manager will restore the actuator and exit instead of
        // silently blessing a broken authority-history boundary.
        let _ = verify_state(&path, key)?;
    }
    let temporary = config.workspace.join(format!(
        "tuning/state.json.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    write_new_private(&temporary, &serde_json::to_vec_pretty(&signed)?)?;
    fs::rename(&temporary, &path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    sync_parent(&config.workspace.join("tuning/state.json"))?;
    Ok(())
}

fn verify_state(path: &Path, key: &SigningKey) -> Result<TuningState> {
    validate_owner_private_file(path)?;
    let signed = serde_json::from_slice::<SignedState>(&fs::read(path)?)?;
    if signed.schema != STATE_SCHEMA
        || signed.signing_public_key != encode_hex(&key.verifying_key().to_bytes())
    {
        bail!("tuning state schema or appliance key mismatch");
    }
    let payload = serde_json::to_vec(&signed.payload)?;
    if signed.payload_sha256 != format!("{:x}", Sha256::digest(&payload)) {
        bail!("tuning state payload hash mismatch");
    }
    let signature = Signature::from_bytes(&decode_fixed::<64>(&signed.signature)?);
    VerifyingKey::from_bytes(&decode_fixed::<32>(&signed.signing_public_key)?)?
        .verify(&payload, &signature)?;
    Ok(signed.payload)
}

fn verified_receipts(config: &Config, key: &SigningKey) -> Result<Vec<Value>> {
    let path = config.workspace.join("tuning/receipts.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    validate_owner_private_file(&path)?;
    let bytes = fs::read(&path)?;
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        bail!("tuning receipt ledger has an incomplete trailing record");
    }
    let content = std::str::from_utf8(&bytes).context("tuning receipt ledger is not UTF-8")?;
    content
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if line.is_empty() {
                bail!("tuning receipt ledger contains an empty record");
            }
            let envelope: Value = serde_json::from_str(line)
                .with_context(|| format!("invalid tuning receipt at line {}", index + 1))?;
            let payload = verify_signed_envelope(&envelope, key)
                .with_context(|| format!("unverified tuning receipt at line {}", index + 1))?;
            if payload.get("schema").and_then(Value::as_str) != Some(RECEIPT_SCHEMA) {
                bail!("tuning receipt at line {} has an unknown schema", index + 1);
            }
            Ok(payload)
        })
        .collect()
}

fn verify_signed_envelope(envelope: &Value, key: &SigningKey) -> Result<Value> {
    let public_key = envelope
        .get("signing_public_key")
        .and_then(Value::as_str)
        .context("signed envelope is missing its public key")?;
    if public_key != encode_hex(&key.verifying_key().to_bytes()) {
        bail!("signed envelope appliance key mismatch");
    }
    let payload = envelope
        .get("payload")
        .cloned()
        .context("signed envelope is missing its payload")?;
    let bytes = serde_json::to_vec(&payload)?;
    let expected_hash = format!("{:x}", Sha256::digest(&bytes));
    if envelope.get("payload_sha256").and_then(Value::as_str) != Some(expected_hash.as_str()) {
        bail!("signed envelope payload hash mismatch");
    }
    let signature = Signature::from_bytes(&decode_fixed::<64>(
        envelope
            .get("signature")
            .and_then(Value::as_str)
            .context("signed envelope is missing its signature")?,
    )?);
    key.verifying_key().verify(&bytes, &signature)?;
    Ok(payload)
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_data()?;
    validate_owner_private_metadata(&file.metadata()?)?;
    sync_parent(path)?;
    Ok(())
}

fn write_new_private_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.sync_data()?;
            validate_owner_private_metadata(&file.metadata()?)?;
            sync_parent(path)?;
            Ok(())
        },
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_owner_private_file(path)?;
            if fs::read(path)? != bytes {
                bail!("existing tuning evidence does not match the signed retry payload");
            }
            Ok(())
        },
        Err(error) => Err(error.into()),
    }
}

fn validate_owner_private_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        bail!("tuning authority file is not a regular non-symlink file");
    }
    validate_owner_private_metadata(&metadata)
}

fn validate_owner_private_metadata(metadata: &fs::Metadata) -> Result<()> {
    if metadata.permissions().mode() & 0o077 != 0 {
        bail!("tuning authority file is not owner-only");
    }
    if let Some(effective_uid) = effective_uid()
        && metadata.uid() != effective_uid
    {
        bail!("tuning authority file is not owned by the runtime user");
    }
    Ok(())
}

#[cfg(not(test))]
fn effective_uid() -> Option<u32> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("Uid:"))?;
    line.split_whitespace().nth(2)?.parse().ok()
}

#[cfg(test)]
const fn effective_uid() -> Option<u32> {
    None
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("tuning authority path has no parent")?;
    fs::File::open(parent)?.sync_data()?;
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N]> {
    if value.len() != N.saturating_mul(2) {
        bail!("invalid hex length");
    }
    let mut output = [0_u8; N];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index.saturating_mul(2);
        *byte = u8::from_str_radix(&value[offset..offset.saturating_add(2)], 16)?;
    }
    Ok(output)
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
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use super::*;
    use clap::Parser as _;

    const TEST_MEMORY_BYTES: u64 = 4 * 1_024 * 1_024 * 1_024;
    type ReservoirAuditHarness = (
        mpsc::Sender<ReservoirCommand>,
        Arc<Mutex<Vec<&'static str>>>,
        tokio::task::JoinHandle<()>,
    );
    type ActiveTestTrial = (
        TuningManager,
        mpsc::Sender<ReservoirCommand>,
        Arc<Mutex<Vec<&'static str>>>,
        tokio::task::JoinHandle<()>,
        ReservoirSnapshot,
        [u8; 32],
    );

    fn test_config() -> (Config, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("astrid-tuning-{}", uuid::Uuid::new_v4()));
        let workspace = root.join("state/home/default/edge");
        let mut config = Config::parse_from(["astrid-edge-runtime"]);
        config.workspace = workspace;
        config.reservoir_tuning_enabled = true;
        config.prepare_workspace().unwrap();
        let hindsight = root.join("state/operator/hindsight");
        fs::create_dir_all(&hindsight).unwrap();
        fs::write(
            hindsight.join("latest.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "astrid_edge_hindsight_checkpoint_v2",
                "recorded_at_unix_ms": unix_millis(),
                "host_boot_id": current_boot_id(),
                "continuity_epoch": "test-epoch",
                "continuity_from_previous_checkpoint_valid": true,
                "current_epoch_integrity_violation_count": 0,
                "legacy_race_compatible_unresolved_violation_count": 0,
                "operator_hindsight_database": {
                    "schema_version": 3,
                    "quick_check": "ok"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        (config, root)
    }

    fn test_snapshot(input_gain: f32) -> ReservoirSnapshot {
        ReservoirSnapshot {
            t_ms: 42,
            fill_ratio: 0.68,
            fill_target: FIXED_FILL_TARGET,
            exploration_noise: 0.1,
            exploration_scale: 1.0,
            input_gain,
            regulation_strength: 1.0,
            leak: 0.2,
            aux_fresh: true,
            audio_fresh: true,
            audio_source: "test-audio".to_string(),
            video_source: "unavailable".to_string(),
            aux_source: "test-aux".to_string(),
            aux_features: BTreeMap::from([("thermal_normalized".to_string(), Some(0.2))]),
            ..ReservoirSnapshot::default()
        }
    }

    fn provenance(session: &str, response_byte: char) -> TuningProvenance {
        let trace = IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            session.to_string(),
            Some("test-chain".to_string()),
        );
        TuningProvenance {
            session_id: session.to_string(),
            authority_turn_id: trace.turn_id.expect("root trace has a turn id").to_string(),
            response_sha256: response_byte.to_string().repeat(64),
            trace,
            decision_source: "scheduler_verified_authored_turn",
        }
    }

    fn prime_baseline(manager: &mut TuningManager, now: u64, snapshot: &ReservoirSnapshot) {
        let sensor_profile_sha256 = sensor_profile_hash(snapshot);
        for offset in (1_u64..=600).rev() {
            manager.recent.push_back(LiveSample {
                recorded_at_unix_ms: now.saturating_sub(offset.saturating_mul(1_000)),
                snapshot: snapshot.clone(),
                available_memory_bytes: Some(TEST_MEMORY_BYTES),
                swap_used_bytes: Some(0),
                sensor_profile_sha256: sensor_profile_sha256.clone(),
            });
        }
        manager.rolling_fill = std::iter::repeat_n(0.68, 60).collect();
    }

    fn reservoir_harness() -> (
        mpsc::Sender<ReservoirCommand>,
        Arc<Mutex<Vec<TuningParameters>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let (tx, mut receiver) = mpsc::channel(8);
        let commands = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&commands);
        let task = tokio::spawn(async move {
            let mut leases = std::collections::BTreeMap::new();
            while let Some(command) = receiver.recv().await {
                match command {
                    ReservoirCommand::SetTuning {
                        parameters,
                        baseline,
                        lease_id,
                        reply,
                        ..
                    } => {
                        leases.insert(lease_id, baseline);
                        observed.lock().unwrap().push(parameters.clone());
                        let _ = reply.send(Ok(parameters));
                    },
                    ReservoirCommand::RenewTuningLease { reply, .. } => {
                        let _ = reply.send(Ok(()));
                    },
                    ReservoirCommand::RestoreTuning { lease_id, reply } => {
                        let result = leases
                            .remove(&lease_id)
                            .context("unknown test tuning lease");
                        if let Ok(parameters) = &result {
                            observed.lock().unwrap().push(parameters.clone());
                        }
                        let _ = reply.send(result);
                    },
                    ReservoirCommand::RestoreBaseline { parameters, reply } => {
                        observed.lock().unwrap().push(parameters.clone());
                        let _ = reply.send(Ok(parameters));
                    },
                }
            }
        });
        (tx, commands, task)
    }

    fn reservoir_audit_harness() -> ReservoirAuditHarness {
        let (tx, mut receiver) = mpsc::channel(8);
        let commands = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&commands);
        let task = tokio::spawn(async move {
            let mut leases = std::collections::BTreeMap::new();
            while let Some(command) = receiver.recv().await {
                match command {
                    ReservoirCommand::SetTuning {
                        parameters,
                        baseline,
                        lease_id,
                        reply,
                        ..
                    } => {
                        leases.insert(lease_id, baseline);
                        observed.lock().unwrap().push("set");
                        let _ = reply.send(Ok(parameters));
                    },
                    ReservoirCommand::RenewTuningLease {
                        lease_id, reply, ..
                    } => {
                        observed.lock().unwrap().push("renew");
                        let result = leases
                            .contains_key(&lease_id)
                            .then_some(())
                            .context("unknown test tuning lease");
                        let _ = reply.send(result);
                    },
                    ReservoirCommand::RestoreTuning { lease_id, reply } => {
                        observed.lock().unwrap().push("restore");
                        let result = leases
                            .remove(&lease_id)
                            .context("unknown test tuning lease");
                        let _ = reply.send(result);
                    },
                    ReservoirCommand::RestoreBaseline { parameters, reply } => {
                        observed.lock().unwrap().push("restore_baseline");
                        let _ = reply.send(Ok(parameters));
                    },
                }
            }
        });
        (tx, commands, task)
    }

    async fn active_test_trial(config: &Config, lease_id: &str) -> ActiveTestTrial {
        let mut manager = TuningManager::load(config).unwrap();
        let signing_key_bytes = manager.key.to_bytes();
        let now = unix_millis();
        let baseline = TuningParameters::safe_default();
        let mut target = baseline.clone();
        target.input_gain = 1.05;
        let snapshot = test_snapshot(target.input_gain);
        let live = live_sample(now, snapshot.clone());
        let original = provenance("integrity-origin", 'a');
        let spec = parse_start("input_gain=1.05 FOR 15m :: signed-ledger fail-safe").unwrap();
        manager.state.last_safe_baseline = Some(baseline.clone());
        manager.state.active_experiment = Some(ActiveExperiment {
            experiment_id: lease_id.to_string(),
            lease_id: lease_id.to_string(),
            candidate_id: candidate_id(&spec),
            spec,
            phase: "trial".to_string(),
            started_at_unix_ms: now,
            phase_ends_at_unix_ms: now.saturating_add(15 * MINUTE_MS),
            last_sample_at_unix_ms: now,
            baseline: baseline.clone(),
            baseline_available_memory_bytes: Some(TEST_MEMORY_BYTES),
            baseline_swap_used_bytes: Some(0),
            environment: environment(config, &live),
            provenance_key: original.replay_key(),
            parent_response_sha256: original.response_sha256.clone(),
            trace: original.trace.child(),
            trial_samples: Vec::new(),
            recovery_samples: Vec::new(),
            failure_reason: None,
            outside_streak: 0,
            saturation_streak: 0,
        });
        manager.persist(config).unwrap();
        let (reservoir_tx, commands, reservoir_task) = reservoir_audit_harness();
        set_reservoir(&reservoir_tx, target, baseline, lease_id.to_string())
            .await
            .unwrap();
        (
            manager,
            reservoir_tx,
            commands,
            reservoir_task,
            snapshot,
            signing_key_bytes,
        )
    }

    fn qualifying_trial(
        experiment_id: &str,
        started_at_unix_ms: u64,
        spec: &TuningSpec,
        environment: &EnvironmentIdentity,
        provenance: &TuningProvenance,
    ) -> TrialEvidence {
        TrialEvidence {
            experiment_id: experiment_id.to_string(),
            candidate_id: candidate_id(spec),
            spec: spec.clone(),
            started_at_unix_ms,
            completed_at_unix_ms: started_at_unix_ms.saturating_add(25 * MINUTE_MS),
            environment: environment.clone(),
            provenance_key: provenance.replay_key(),
            session_id: provenance.session_id.clone(),
            parent_response_sha256: provenance.response_sha256.clone(),
            trace: provenance.trace.child(),
            sample_count: 15,
            expected_samples: 15,
            recovery_sample_count: 10,
            fill_min: 0.67,
            fill_mean: 0.68,
            fill_max: 0.70,
            shelf_occupancy: 1.0,
            baseline_available_memory_bytes: Some(TEST_MEMORY_BYTES),
            baseline_swap_used_bytes: Some(0),
            available_memory_min_bytes: Some(TEST_MEMORY_BYTES),
            swap_growth_bytes: Some(0),
            qualifying: true,
            failure_reason: None,
            evidence_artifact: format!("tuning/evidence/{experiment_id}_result.json"),
        }
    }

    #[test]
    fn parser_accepts_only_exact_bounded_tuning_grammar() {
        let parsed = parse_start(
            "input_gain=1.05 FOR 15m :: Does modest input gain preserve the settled shelf?",
        )
        .unwrap();
        assert_eq!(parsed.parameter, TuningParameter::InputGain);
        assert_eq!(parsed.duration_minutes, 15);
        assert!(parse_start("fill_target=.70 FOR 15m :: move shelf").is_none());
        assert!(parse_start("input_gain=1.11 FOR 15m :: too high").is_none());
        assert!(parse_start("exploration_scale=.95 FOR 10m :: invalid duration").is_none());
        assert!(parse_start("regulation_strength=.9 FOR 60m :: bounded").is_some());
    }

    #[test]
    fn provenance_requires_exact_unrepaired_trace_binding() {
        let trace = IpcTraceContextV1::root(uuid::Uuid::new_v4(), "session".to_string(), None);
        let valid = TuningProvenance {
            session_id: "session".to_string(),
            authority_turn_id: trace.turn_id.expect("root trace turn").to_string(),
            response_sha256: "a".repeat(64),
            trace: trace.clone(),
            decision_source: "scheduler_verified_authored_turn",
        };
        assert!(valid.validate().is_ok());
        assert!(
            TuningProvenance {
                decision_source: "kernel_attested_exact_model_terminal_response",
                ..valid.clone()
            }
            .validate()
            .is_ok()
        );
        assert!(
            TuningProvenance {
                decision_source: "kernel_attested_react_terminal_response",
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            TuningProvenance {
                authority_turn_id: uuid::Uuid::new_v4().to_string(),
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            TuningProvenance {
                decision_source: "local_format_repair_preserved_astrid_declaration",
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            TuningProvenance {
                session_id: "other".to_string(),
                ..valid
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn replay_identity_ignores_child_span_and_response_but_binds_session_and_turn() {
        let first = provenance("session", 'a');
        let mut second = first.clone();
        second.trace = first.trace.child();
        assert_eq!(first.replay_key(), second.replay_key());

        second.response_sha256 = "b".repeat(64);
        assert_eq!(first.replay_key(), second.replay_key());

        let replacement_turn = uuid::Uuid::new_v4();
        second.trace.turn_id = Some(replacement_turn);
        second.authority_turn_id = replacement_turn.to_string();
        assert_ne!(first.replay_key(), second.replay_key());
    }

    #[test]
    fn candidate_identity_excludes_hypothesis_but_binds_control_and_duration() {
        let first = parse_start("input_gain=1.05 FOR 15m :: first question").unwrap();
        let second = parse_start("input_gain=1.05 FOR 15m :: second question").unwrap();
        let other = parse_start("input_gain=1.04 FOR 15m :: first question").unwrap();
        assert_eq!(candidate_id(&first), candidate_id(&second));
        assert_ne!(candidate_id(&first), candidate_id(&other));

        let adjacent = TuningSpec {
            value: f32::from_bits(first.value.to_bits().saturating_add(1)),
            ..first.clone()
        };
        assert_ne!(candidate_id(&first), candidate_id(&adjacent));
    }

    #[tokio::test]
    async fn completed_request_replay_returns_prior_result_without_actuation() {
        let (config, root) = test_config();
        let mut manager = TuningManager::load(&config).unwrap();
        let first = provenance("session", 'a');
        let mut replay = first.clone();
        replay.trace = first.trace.child();
        let action = TuningAction::Cancel("tuning_existing".to_string());
        let prior = TuningActionResult {
            status: "executed",
            outcome: "reservoir_tuning_cancelled_and_rolled_back",
            artifact_path: None,
            tuning_id: Some("tuning_existing".to_string()),
            candidate_id: None,
            phase: Some("recovery"),
        };
        manager.state.completed_requests.push(CompletedRequest {
            provenance_key: first.replay_key(),
            response_sha256: first.response_sha256.clone(),
            action_sha256: action_sha256(&action).unwrap(),
            completed_at_unix_ms: 1,
            result: StoredActionResult::from_runtime(&prior),
        });
        let (tx, commands, task) = reservoir_harness();
        let result = manager
            .handle_request(&config, &tx, test_snapshot(1.0), action, replay)
            .await
            .unwrap();
        assert_eq!(result.outcome, prior.outcome);
        assert!(commands.lock().unwrap().is_empty());
        task.abort();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn completed_turn_cannot_be_rebound_to_a_different_response_body() {
        let (config, root) = test_config();
        let mut manager = TuningManager::load(&config).unwrap();
        let first = provenance("session", 'a');
        let mut rebound = first.clone();
        rebound.response_sha256 = "b".repeat(64);
        let action = TuningAction::Cancel("tuning_existing".to_string());
        manager.state.completed_requests.push(CompletedRequest {
            provenance_key: first.replay_key(),
            response_sha256: first.response_sha256,
            action_sha256: action_sha256(&action).unwrap(),
            completed_at_unix_ms: 1,
            result: StoredActionResult::from_runtime(&TuningActionResult {
                status: "executed",
                outcome: "reservoir_tuning_cancelled_and_rolled_back",
                artifact_path: None,
                tuning_id: Some("tuning_existing".to_string()),
                candidate_id: None,
                phase: Some("recovery"),
            }),
        });
        let (tx, commands, task) = reservoir_harness();
        assert!(
            manager
                .handle_request(&config, &tx, test_snapshot(1.0), action, rebound)
                .await
                .is_err()
        );
        assert!(commands.lock().unwrap().is_empty());
        task.abort();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn validation_applies_exact_candidate_then_restores_captured_baseline() {
        let (config, root) = test_config();
        let mut manager = TuningManager::load(&config).unwrap();
        let now = 20_000_000;
        let baseline_snapshot = test_snapshot(1.0);
        prime_baseline(&mut manager, now, &baseline_snapshot);
        let live = LiveSample {
            recorded_at_unix_ms: now,
            snapshot: baseline_snapshot.clone(),
            available_memory_bytes: Some(TEST_MEMORY_BYTES),
            swap_used_bytes: Some(0),
            sensor_profile_sha256: sensor_profile_hash(&baseline_snapshot),
        };
        let environment = environment(&config, &live);
        assert!(environment.hindsight_continuity_valid);
        let spec = parse_start("input_gain=1.05 FOR 15m :: Does this preserve the shelf?").unwrap();
        let first = provenance("trial-one", 'a');
        let second = provenance("trial-two", 'b');
        manager.state.trials = vec![
            qualifying_trial(
                "tuning_one",
                now.saturating_sub(2 * 60 * MINUTE_MS),
                &spec,
                &environment,
                &first,
            ),
            qualifying_trial(
                "tuning_two",
                now.saturating_sub(60 * MINUTE_MS),
                &spec,
                &environment,
                &second,
            ),
        ];
        let request = provenance("validation", 'c');
        let expected_candidate_id = candidate_id(&spec);
        let (tx, commands, task) = reservoir_harness();
        manager
            .validate(
                &config,
                &tx,
                now,
                baseline_snapshot,
                expected_candidate_id.clone(),
                "Can the result survive a six-hour validation?".to_string(),
                &request,
            )
            .await
            .unwrap();
        assert!((commands.lock().unwrap()[0].input_gain - 1.05).abs() < f32::EPSILON);

        let completion_time = now.saturating_add(VALIDATION_MS);
        let active = manager.state.active_validation.as_mut().unwrap();
        active.samples = (0_u64..342)
            .map(|index| MinuteSample {
                recorded_at_unix_ms: now.saturating_add(index.saturating_mul(MINUTE_MS)),
                fill_ratio: 0.68,
                available_memory_bytes: Some(TEST_MEMORY_BYTES),
                swap_used_bytes: Some(0),
            })
            .collect();
        active.last_sample_at_unix_ms = completion_time;
        active.completes_at_unix_ms = completion_time;
        let candidate_snapshot = test_snapshot(1.05);
        let completion_live = LiveSample {
            recorded_at_unix_ms: completion_time,
            snapshot: candidate_snapshot.clone(),
            available_memory_bytes: Some(TEST_MEMORY_BYTES),
            swap_used_bytes: Some(0),
            sensor_profile_sha256: sensor_profile_hash(&candidate_snapshot),
        };
        manager
            .observe_validation(&config, &tx, completion_time, &completion_live)
            .await
            .unwrap();
        let observed = commands.lock().unwrap();
        assert_eq!(observed.len(), 2);
        assert!((observed[1].input_gain - 1.0).abs() < f32::EPSILON);
        drop(observed);
        let evidence = manager.state.validations.last().unwrap();
        assert!(evidence.successful);
        assert_eq!(evidence.qualifying_trial_ids, ["tuning_one", "tuning_two"]);
        assert_eq!(evidence.parent_response_sha256, request.response_sha256);
        assert_eq!(evidence.trace.trace_id, request.trace.trace_id);
        task.abort();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn restart_restores_the_experiments_captured_baseline() {
        let (config, root) = test_config();
        let mut manager = TuningManager::load(&config).unwrap();
        let now = 10_000_000;
        let snapshot = test_snapshot(1.05);
        let live = LiveSample {
            recorded_at_unix_ms: now,
            snapshot: snapshot.clone(),
            available_memory_bytes: Some(TEST_MEMORY_BYTES),
            swap_used_bytes: Some(0),
            sensor_profile_sha256: sensor_profile_hash(&snapshot),
        };
        let request = provenance("restart", 'd');
        let spec = parse_start("input_gain=1.05 FOR 15m :: restart safety").unwrap();
        manager.state.active_experiment = Some(ActiveExperiment {
            experiment_id: "tuning_restart".to_string(),
            lease_id: "tuning_restart".to_string(),
            candidate_id: candidate_id(&spec),
            spec,
            phase: "trial".to_string(),
            started_at_unix_ms: now,
            phase_ends_at_unix_ms: now.saturating_add(15 * MINUTE_MS),
            last_sample_at_unix_ms: now,
            baseline: TuningParameters {
                input_gain: 0.97,
                exploration_scale: 1.0,
                regulation_strength: 1.0,
            },
            baseline_available_memory_bytes: Some(TEST_MEMORY_BYTES),
            baseline_swap_used_bytes: Some(0),
            environment: environment(&config, &live),
            provenance_key: request.replay_key(),
            parent_response_sha256: request.response_sha256.clone(),
            trace: request.trace.child(),
            trial_samples: Vec::new(),
            recovery_samples: Vec::new(),
            failure_reason: None,
            outside_streak: 0,
            saturation_streak: 0,
        });
        let (tx, commands, task) = reservoir_harness();
        manager
            .recover_restart(&config, &tx, snapshot)
            .await
            .unwrap();
        assert!((commands.lock().unwrap()[0].input_gain - 0.97).abs() < f32::EPSILON);
        assert!(manager.state.active_experiment.is_none());
        assert!(manager.state.cooldown_until_unix_ms >= unix_millis());
        task.abort();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn baseline_and_live_safety_require_fresh_declared_sensors() {
        let (config, root) = test_config();
        let mut manager = TuningManager::load(&config).unwrap();
        let now = unix_millis();
        let snapshot = test_snapshot(1.0);
        manager.recent.clear();
        prime_baseline(&mut manager, now, &snapshot);
        let mut stale_audio = snapshot.clone();
        stale_audio.audio_fresh = false;
        manager.recent.back_mut().unwrap().snapshot = stale_audio;
        let live = live_sample(now, snapshot.clone());
        assert!(manager.require_stable_baseline(&live).is_err());

        manager.recent.clear();
        prime_baseline(&mut manager, now, &snapshot);
        manager.recent.back_mut().unwrap().snapshot.aux_fresh = false;
        assert!(manager.require_stable_baseline(&live).is_err());

        let environment = environment(&config, &live);
        assert_eq!(
            required_sensor_failure(&manager.recent.back().unwrap().snapshot, &environment)
                .as_deref(),
            Some("required_aux_sensor_unavailable_or_stale")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hindsight_identity_requires_fresh_current_boot_checkpoint() {
        let (config, root) = test_config();
        assert!(hindsight_identity(&config).1);
        let path = root.join("state/operator/hindsight/latest.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["recorded_at_unix_ms"] =
            json!(unix_millis().saturating_sub(HINDSIGHT_MAX_AGE_MS + 1));
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        assert!(!hindsight_identity(&config).1);

        value["recorded_at_unix_ms"] = json!(unix_millis());
        value["host_boot_id"] = json!("22222222-2222-4222-8222-222222222222");
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        assert!(!hindsight_identity(&config).1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_revision_gate_rejects_unknown_and_dirty_builds() {
        assert!(valid_source_revision("0123456789abcdef"));
        assert!(!valid_source_revision("unknown"));
        assert!(!valid_source_revision("unrecorded"));
        assert!(!valid_source_revision("0123456-dirty"));
    }

    #[tokio::test]
    async fn closed_or_rejected_reservoir_control_is_typed_fatal() {
        let (closed_tx, closed_rx) = mpsc::channel(1);
        drop(closed_rx);
        let closed = restore_baseline(&closed_tx, TuningParameters::safe_default())
            .await
            .unwrap_err();
        assert!(is_fatal_reservoir_control(&closed));

        let (rejected_tx, mut rejected_rx) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            if let Some(ReservoirCommand::RestoreBaseline { reply, .. }) = rejected_rx.recv().await
            {
                let _ = reply.send(Err(anyhow::anyhow!("simulated actuator rejection")));
            }
        });
        let rejected = restore_baseline(&rejected_tx, TuningParameters::safe_default())
            .await
            .unwrap_err();
        assert!(is_fatal_reservoir_control(&rejected));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn receipt_integrity_failure_rolls_back_and_exits_without_renewing_lease() {
        let (config, root) = test_config();
        let lease_id = "tuning_integrity_failure".to_string();
        let (manager, reservoir_tx, commands, reservoir_task, snapshot, signing_key_bytes) =
            active_test_trial(&config, &lease_id).await;

        let receipt_path = config.workspace.join("tuning/receipts.jsonl");
        fs::write(&receipt_path, b"{").unwrap();
        fs::set_permissions(&receipt_path, fs::Permissions::from_mode(0o600)).unwrap();

        let (request_tx, request_rx) = mpsc::channel(1);
        let (reply, response) = oneshot::channel();
        request_tx
            .send(TuningRequest {
                action: TuningAction::Cancel(lease_id),
                provenance: provenance("integrity-cancel", 'b'),
                reply,
            })
            .await
            .unwrap();
        drop(request_tx);
        let (_snapshot_tx, snapshot_rx) = watch::channel(snapshot);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_loaded_manager(
                Arc::new(config.clone()),
                request_rx,
                reservoir_tx,
                snapshot_rx,
                manager,
            ),
        )
        .await
        .expect("fatal authority-storage failure must terminate the manager");

        let result = response.await.unwrap();
        assert_eq!(result.phase, Some("fatal"));
        assert_eq!(&*commands.lock().unwrap(), &["set", "restore"]);
        let key = SigningKey::from_bytes(&signing_key_bytes);
        let recovered = verify_state(&config.workspace.join("tuning/state.json"), &key).unwrap();
        assert!(recovered.active_experiment.is_none());
        assert!(recovered.active_validation.is_none());
        reservoir_task.abort();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn state_persistence_failure_rolls_back_and_exits_without_renewing_lease() {
        let (config, root) = test_config();
        let lease_id = "tuning_state_failure".to_string();
        let (manager, reservoir_tx, commands, reservoir_task, snapshot, signing_key_bytes) =
            active_test_trial(&config, &lease_id).await;
        manager
            .fail_next_persist
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let (request_tx, request_rx) = mpsc::channel(1);
        let (reply, response) = oneshot::channel();
        request_tx
            .send(TuningRequest {
                action: TuningAction::Cancel(lease_id),
                provenance: provenance("state-failure-cancel", 'c'),
                reply,
            })
            .await
            .unwrap();
        drop(request_tx);
        let (_snapshot_tx, snapshot_rx) = watch::channel(snapshot);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_loaded_manager(
                Arc::new(config.clone()),
                request_rx,
                reservoir_tx,
                snapshot_rx,
                manager,
            ),
        )
        .await
        .expect("fatal signed-state persistence failure must terminate the manager");

        let result = response.await.unwrap();
        assert_eq!(result.phase, Some("fatal"));
        assert_eq!(
            &*commands.lock().unwrap(),
            &["set", "restore", "restore_baseline"]
        );
        let key = SigningKey::from_bytes(&signing_key_bytes);
        let recovered = verify_state(&config.workspace.join("tuning/state.json"), &key).unwrap();
        assert!(recovered.active_experiment.is_none());
        assert!(recovered.active_validation.is_none());
        reservoir_task.abort();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signed_receipts_bind_turns_and_distinguish_machine_lifecycle_records() {
        let (config, root) = test_config();
        let manager = TuningManager::load(&config).unwrap();
        let provenance = provenance("receipt-session", 'a');
        let action = TuningAction::Cancel("tuning_receipt".to_string());
        manager
            .receipt(
                &config,
                "requested",
                unix_millis(),
                json!({
                    "provenance_key": provenance.replay_key(),
                    "action_sha256": action_sha256(&action).unwrap(),
                    "parent_response_sha256": provenance.response_sha256,
                }),
            )
            .unwrap();
        manager
            .receipt(
                &config,
                "expired_rollback_started",
                unix_millis(),
                json!({"experiment_id": "tuning_receipt"}),
            )
            .unwrap();
        let receipts = verified_receipts(&config, &manager.key).unwrap();
        assert_eq!(
            receipts[0].get("trigger_authority").and_then(Value::as_str),
            Some("verified_genuinely_authored_tuning_action")
        );
        assert_eq!(
            receipts[1].get("trigger_authority").and_then(Value::as_str),
            Some("machine_lifecycle_or_safety_transition")
        );
        assert_eq!(
            receipts[1].get("authorship").and_then(Value::as_str),
            Some("executor_or_machine_record_not_astrid_authorship")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn receipt_verification_rejects_tampering_and_partial_tail_records() {
        let (config, root) = test_config();
        let manager = TuningManager::load(&config).unwrap();
        manager
            .receipt(
                &config,
                "expired_rollback_started",
                unix_millis(),
                json!({"experiment_id": "tuning_receipt"}),
            )
            .unwrap();
        let path = config.workspace.join("tuning/receipts.jsonl");
        let mut envelope: Value =
            serde_json::from_str(fs::read_to_string(&path).unwrap().lines().next().unwrap())
                .unwrap();
        envelope["payload"]["phase"] = json!("adopted");
        let mut tampered = serde_json::to_vec(&envelope).unwrap();
        tampered.push(b'\n');
        fs::write(&path, tampered).unwrap();
        assert!(verified_receipts(&config, &manager.key).is_err());
        fs::write(&path, b"{").unwrap();
        assert!(verified_receipts(&config, &manager.key).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signing_material_must_remain_owner_only_and_match() {
        let (config, root) = test_config();
        let manager = TuningManager::load(&config).unwrap();
        let public_path = config.workspace.join("tuning/signing.pub");
        fs::set_permissions(&public_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(TuningManager::load(&config).is_err());
        fs::set_permissions(&public_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&public_path, "00".repeat(32)).unwrap();
        assert!(TuningManager::load(&config).is_err());
        drop(manager);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_signing_key_with_existing_history_never_creates_a_replacement_identity() {
        let (config, root) = test_config();
        let manager = TuningManager::load(&config).unwrap();
        manager.persist(&config).unwrap();
        let key_path = config.workspace.join("tuning/signing.key");
        let public_path = config.workspace.join("tuning/signing.pub");
        fs::remove_file(&key_path).unwrap();
        fs::remove_file(&public_path).unwrap();

        let Err(error) = TuningManager::load(&config) else {
            panic!("missing signing identity with existing history must fail closed");
        };
        assert!(
            error
                .to_string()
                .contains("history exists without its signing key")
        );
        assert!(!key_path.exists());
        assert!(!public_path.exists());
        assert!(config.workspace.join("tuning/state.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signed_daily_counter_survives_without_a_receipt_ledger() {
        let (config, root) = test_config();
        let mut manager = TuningManager::load(&config).unwrap();
        let day = unix_millis() / DAY_MS;
        manager.increment_start_counter(day);
        manager.persist(&config).unwrap();
        let reloaded = TuningManager::load(&config).unwrap();
        assert_eq!(reloaded.start_count(&config, day).unwrap(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn state_signature_detects_prefix_mutation() {
        let root = std::env::temp_dir().join(format!("astrid-tuning-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("tuning")).unwrap();
        let mut config = Config::parse_from(["astrid-edge-runtime"]);
        config.workspace = root.clone();
        let key = load_or_create_signing_key(&config).unwrap();
        let state = TuningState {
            updated_at_unix_ms: 7,
            ..TuningState::default()
        };
        persist_state(&config, &key, &state).unwrap();
        assert_eq!(
            verify_state(&root.join("tuning/state.json"), &key)
                .unwrap()
                .updated_at_unix_ms,
            7
        );
        let path = root.join("tuning/state.json");
        let mut value = serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap();
        value["payload"]["updated_at_unix_ms"] = json!(8);
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        assert!(verify_state(&path, &key).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
