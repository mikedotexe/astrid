//! CPU-edge ESN state, its sole eigendecomposition, and private actuator endpoint.
//!
//! State evolution, covariance sampling, spectral derivation, and atomic tuning
//! application remain together to make the one-eigensolve and rollback boundary
//! directly reviewable. A later split may extract pure persistence/serialization,
//! but must not create a second decomposition or expose actuator authority.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use astrid_minime_protocol::{
    EigenPacketV1, ModalityStatus, SpectralDenominatorV1, SpectralSubstrateV1, SpectrumCoverageV1,
};
use astrid_spectral_core::{
    ModeTurnoverSummary, SpectralMode, mode_concentration, mode_turnover_with_boundary,
    sanitize_spectrum,
};
use nalgebra::{DMatrix, SymmetricEigen};
use rand::{Rng as _, SeedableRng as _, rngs::SmallRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::config::Config;

const RESERVOIR_DIM: usize = 128;
const RESERVOIR_DIM_F32: f32 = 128.0;
const COVARIANCE_EFFECTIVE_WINDOW_SAMPLES: usize = 333;
const EXPORTED_EIGENVALUE_COUNT: usize = 16;
const TRACKED_EIGENVECTOR_COUNT: usize = 4;
const MODE_DEGENERACY_RELATIVE_TOLERANCE: f64 = 0.01;
const FILL_EMA_ALPHA: f32 = 0.18;
const FILL_EMA_ALPHA_PPM: u32 = 180_000;
const MAX_TUNING_LEASE_MS: u64 = 60_000;
const MAX_EXPLORATION_NOISE: f32 = 1.25;
const VIDEO_DIM: usize = 8;
const AUDIO_DIM: usize = 8;
const AUX_DIM: usize = 10;
pub const HOST_AUX_FEATURE_NAMES: [&str; AUX_DIM] = [
    "cpu_busy",
    "memory_used",
    "load_normalized",
    "disk_read_rate",
    "disk_write_rate",
    "network_receive_rate",
    "network_transmit_rate",
    "thermal_normalized",
    "daily_phase_sine",
    "daily_phase_cosine",
];
const SEMANTIC_DIM: usize = 48;
const SEMANTIC_INPUT_SCALE: f32 = 0.12;
const SEMANTIC_INPUT_DECAY: f32 = 0.92;
const AUX_INPUT_SCALE: f32 = 0.25;
const INPUT_DIM: usize = VIDEO_DIM + AUDIO_DIM + AUX_DIM + SEMANTIC_DIM;
const VIDEO_OFFSET: usize = 0;
const AUDIO_OFFSET: usize = VIDEO_OFFSET + VIDEO_DIM;
const AUX_OFFSET: usize = AUDIO_OFFSET + AUDIO_DIM;
const SEMANTIC_OFFSET: usize = AUX_OFFSET + AUX_DIM;

#[derive(Debug)]
pub enum SensoryIngress {
    Video {
        features: Vec<f32>,
        source: String,
    },
    Audio {
        features: Vec<f32>,
        source: String,
    },
    Aux {
        features: Vec<f32>,
        source: String,
        availability: Option<Vec<bool>>,
    },
    Semantic(Vec<f32>),
}

/// The complete private tuning surface accepted by the CPU-edge reservoir.
///
/// Keeping this as an absolute, typed tuple makes application and rollback
/// atomic. It deliberately contains neither fill target nor ESN leak.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TuningParameters {
    pub input_gain: f32,
    pub exploration_scale: f32,
    pub regulation_strength: f32,
}

impl TuningParameters {
    #[must_use]
    pub const fn safe_default() -> Self {
        Self {
            input_gain: 1.0,
            exploration_scale: 1.0,
            regulation_strength: 1.0,
        }
    }

    #[must_use]
    pub fn from_snapshot(snapshot: &ReservoirSnapshot) -> Self {
        Self {
            input_gain: snapshot.input_gain,
            exploration_scale: snapshot.exploration_scale,
            regulation_strength: snapshot.regulation_strength,
        }
    }

    pub fn set(&mut self, parameter: crate::tuning::TuningParameter, value: f32) {
        match parameter {
            crate::tuning::TuningParameter::InputGain => self.input_gain = value,
            crate::tuning::TuningParameter::ExplorationScale => self.exploration_scale = value,
            crate::tuning::TuningParameter::RegulationStrength => {
                self.regulation_strength = value;
            },
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if !self.input_gain.is_finite()
            || !self.exploration_scale.is_finite()
            || !self.regulation_strength.is_finite()
        {
            anyhow::bail!("private reservoir tuning contains a non-finite value");
        }
        if !(0.90..=1.10).contains(&self.input_gain)
            || !(0.90..=1.10).contains(&self.exploration_scale)
            || !(0.85..=1.15).contains(&self.regulation_strength)
        {
            anyhow::bail!("private reservoir tuning exceeds the compiled safe envelope");
        }
        Ok(())
    }
}

fn validate_tuning_lease(lease_id: &str, lease_duration_ms: u64) -> anyhow::Result<()> {
    if lease_id.is_empty()
        || lease_id.len() > 128
        || !lease_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        anyhow::bail!("private reservoir tuning lease identifier is malformed");
    }
    if !(1..=MAX_TUNING_LEASE_MS).contains(&lease_duration_ms) {
        anyhow::bail!("private reservoir tuning lease duration exceeds policy");
    }
    Ok(())
}

#[derive(Debug)]
pub enum ReservoirCommand {
    SetTuning {
        parameters: TuningParameters,
        baseline: TuningParameters,
        lease_id: String,
        lease_duration_ms: u64,
        reply: oneshot::Sender<anyhow::Result<TuningParameters>>,
    },
    RenewTuningLease {
        lease_id: String,
        lease_duration_ms: u64,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    RestoreTuning {
        lease_id: String,
        reply: oneshot::Sender<anyhow::Result<TuningParameters>>,
    },
    /// Fail-safe private recovery for manager restart or persistence failure.
    RestoreBaseline {
        parameters: TuningParameters,
        reply: oneshot::Sender<anyhow::Result<TuningParameters>>,
    },
}

#[derive(Debug, Clone)]
struct ActiveTuningLease {
    lease_id: String,
    baseline: TuningParameters,
    expires_at: Instant,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Direct read-only lane freshness snapshot.
pub struct ReservoirSnapshot {
    /// Unique for this edge-runtime reservoir process.
    pub generation_id: String,
    /// Strictly increasing within `generation_id` for every completed sample.
    pub sequence: u64,
    pub recorded_at_unix_ms: u64,
    pub t_ms: u64,
    pub fill_ratio: f32,
    pub instantaneous_fill_ratio: f32,
    pub fill_target: f32,
    pub effective_dimensionality: f32,
    /// Normalized entropy of the complete covariance spectrum, not structural entropy.
    pub spectral_entropy: Option<f64>,
    pub lambda1_share: Option<f64>,
    pub head_share: Option<f64>,
    pub shoulder_share: Option<f64>,
    pub tail_share: Option<f64>,
    pub density_gradient: Option<f64>,
    pub largest_gap: Option<f64>,
    pub largest_gap_index: Option<usize>,
    pub mode_turnover: Option<f64>,
    pub mode_identity_stable: Option<bool>,
    pub mode_concentrations: Vec<Option<f64>>,
    pub exported_spectrum_energy_ratio: Option<f64>,
    pub usable_spectrum_mode_count: usize,
    pub discarded_non_finite_mode_count: usize,
    pub clamped_negative_mode_count: usize,
    pub spectral_derivation_ms: Option<f64>,
    pub exploration_noise: f32,
    pub exploration_scale: f32,
    pub input_gain: f32,
    pub regulation_strength: f32,
    pub leak: f32,
    pub semantic_fresh: bool,
    pub audio_fresh: bool,
    pub video_fresh: bool,
    pub aux_fresh: bool,
    pub audio_source: String,
    pub audio_rms: Option<f32>,
    pub video_source: String,
    pub aux_source: String,
    pub aux_features: BTreeMap<String, Option<f32>>,
    pub telemetry_json: String,
}

struct Reservoir {
    recurrent: Vec<f32>,
    input_weights: Vec<f32>,
    state: Vec<f32>,
    next_state: Vec<f32>,
    running_mean: Vec<f32>,
    covariance: Vec<f32>,
    input: Vec<f32>,
    rng: SmallRng,
    tick: u64,
    started: Instant,
    fill_target: f32,
    fill_ema: f32,
    controller_integral: f32,
    exploration_noise: f32,
    exploration_scale: f32,
    input_gain: f32,
    regulation_strength: f32,
    leak: f32,
    semantic_age_ticks: u64,
    video_age_ticks: u64,
    audio_age_ticks: u64,
    aux_age_ticks: u64,
    audio_source: String,
    video_source: String,
    aux_source: String,
    aux_observed: Vec<f32>,
    aux_availability: Vec<bool>,
    tick_hz: u32,
    /// Only four modes are retained between samples; full eigenvectors never persist.
    previous_spectral_modes: Vec<SpectralMode>,
    previous_boundary_eigenvalue: Option<f64>,
    generation_id: String,
    sample_sequence: u64,
    active_tuning_lease: Option<ActiveTuningLease>,
}

impl Reservoir {
    fn new(config: &Config) -> Self {
        let mut rng = SmallRng::seed_from_u64(config.seed);
        let mut recurrent = vec![0.0; RESERVOIR_DIM * RESERVOIR_DIM];
        for value in &mut recurrent {
            if rng.r#gen::<f32>() < 0.10 {
                *value = rng.gen_range(-1.0..1.0);
            }
        }
        scale_spectral_radius(&mut recurrent, 0.92);

        let mut input_weights = vec![0.0; RESERVOIR_DIM * INPUT_DIM];
        for value in &mut input_weights {
            *value = rng.gen_range(-0.45..0.45);
        }

        Self {
            recurrent,
            input_weights,
            state: vec![0.0; RESERVOIR_DIM],
            next_state: vec![0.0; RESERVOIR_DIM],
            running_mean: vec![0.0; RESERVOIR_DIM],
            covariance: vec![0.0; RESERVOIR_DIM * RESERVOIR_DIM],
            input: vec![0.0; INPUT_DIM],
            rng,
            tick: 0,
            started: Instant::now(),
            fill_target: config.fill_target,
            fill_ema: 0.0,
            controller_integral: 0.0,
            exploration_noise: 0.035,
            exploration_scale: 1.0,
            input_gain: 1.0,
            regulation_strength: 1.0,
            leak: 0.55,
            semantic_age_ticks: u64::MAX,
            video_age_ticks: u64::MAX,
            audio_age_ticks: u64::MAX,
            aux_age_ticks: u64::MAX,
            audio_source: "unavailable_no_audio_input".to_string(),
            video_source: "unavailable_no_video_input".to_string(),
            aux_source: "unavailable_no_aux_input".to_string(),
            aux_observed: vec![0.0; AUX_DIM],
            aux_availability: vec![false; AUX_DIM],
            tick_hz: config.tick_hz,
            previous_spectral_modes: Vec::new(),
            previous_boundary_eigenvalue: None,
            generation_id: uuid::Uuid::new_v4().to_string(),
            sample_sequence: 0,
            active_tuning_lease: None,
        }
    }

    fn ingest(&mut self, ingress: SensoryIngress) {
        match ingress {
            SensoryIngress::Video { features, source } => {
                assign_lane(&mut self.input, VIDEO_OFFSET, VIDEO_DIM, &features);
                self.video_age_ticks = 0;
                self.video_source = source;
            },
            SensoryIngress::Audio { features, source } => {
                assign_lane(&mut self.input, AUDIO_OFFSET, AUDIO_DIM, &features);
                self.audio_age_ticks = 0;
                self.audio_source = source;
            },
            SensoryIngress::Aux {
                features,
                source,
                availability,
            } => {
                assign_lane(&mut self.input, AUX_OFFSET, AUX_DIM, &features);
                self.aux_observed.fill(0.0);
                for (target, value) in self.aux_observed.iter_mut().zip(&features) {
                    *target = value.clamp(-1.0, 1.0);
                }
                self.aux_availability.fill(false);
                if let Some(availability) = availability {
                    for (target, available) in self.aux_availability.iter_mut().zip(availability) {
                        *target = available;
                    }
                } else {
                    for target in self
                        .aux_availability
                        .iter_mut()
                        .take(features.len().min(AUX_DIM))
                    {
                        *target = true;
                    }
                }
                self.aux_source = source;
                for value in &mut self.input[AUX_OFFSET..AUX_OFFSET + AUX_DIM] {
                    *value *= AUX_INPUT_SCALE;
                }
                self.aux_age_ticks = 0;
            },
            SensoryIngress::Semantic(features) => {
                assign_lane(&mut self.input, SEMANTIC_OFFSET, SEMANTIC_DIM, &features);
                for value in &mut self.input[SEMANTIC_OFFSET..SEMANTIC_OFFSET + SEMANTIC_DIM] {
                    *value *= SEMANTIC_INPUT_SCALE;
                }
                self.semantic_age_ticks = 0;
            },
        }
    }

    fn apply_tuning(&mut self, parameters: &TuningParameters) -> anyhow::Result<TuningParameters> {
        parameters.validate()?;
        self.input_gain = parameters.input_gain;
        self.exploration_scale = parameters.exploration_scale;
        self.regulation_strength = parameters.regulation_strength;
        Ok(TuningParameters {
            input_gain: self.input_gain,
            exploration_scale: self.exploration_scale,
            regulation_strength: self.regulation_strength,
        })
    }

    fn set_tuning_lease(
        &mut self,
        parameters: &TuningParameters,
        baseline: &TuningParameters,
        lease_id: &str,
        lease_duration_ms: u64,
    ) -> anyhow::Result<TuningParameters> {
        validate_tuning_lease(lease_id, lease_duration_ms)?;
        parameters.validate()?;
        baseline.validate()?;
        self.expire_tuning_lease_if_needed();
        if let Some(active) = self.active_tuning_lease.as_ref() {
            if active.lease_id != lease_id {
                anyhow::bail!("a different private reservoir tuning lease is already active");
            }
            if active.baseline != *baseline {
                anyhow::bail!("an active tuning lease cannot replace its captured baseline");
            }
        }
        let applied = self.apply_tuning(parameters)?;
        self.active_tuning_lease = Some(ActiveTuningLease {
            lease_id: lease_id.to_string(),
            baseline: baseline.clone(),
            expires_at: Instant::now()
                .checked_add(Duration::from_millis(lease_duration_ms))
                .context("private reservoir tuning lease deadline overflow")?,
        });
        Ok(applied)
    }

    fn renew_tuning_lease(&mut self, lease_id: &str, lease_duration_ms: u64) -> anyhow::Result<()> {
        validate_tuning_lease(lease_id, lease_duration_ms)?;
        self.expire_tuning_lease_if_needed();
        let active = self
            .active_tuning_lease
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no private reservoir tuning lease is active"))?;
        if active.lease_id != lease_id {
            anyhow::bail!("private reservoir tuning lease identifier mismatch");
        }
        active.expires_at = Instant::now()
            .checked_add(Duration::from_millis(lease_duration_ms))
            .context("private reservoir tuning lease deadline overflow")?;
        Ok(())
    }

    fn restore_tuning_lease(&mut self, lease_id: &str) -> anyhow::Result<TuningParameters> {
        self.expire_tuning_lease_if_needed();
        let active = self
            .active_tuning_lease
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no private reservoir tuning lease is active"))?;
        if active.lease_id != lease_id {
            anyhow::bail!("private reservoir tuning lease identifier mismatch");
        }
        let active = self
            .active_tuning_lease
            .take()
            .ok_or_else(|| anyhow::anyhow!("validated tuning lease disappeared"))?;
        self.apply_tuning(&active.baseline)
    }

    fn restore_baseline(
        &mut self,
        parameters: &TuningParameters,
    ) -> anyhow::Result<TuningParameters> {
        parameters.validate()?;
        let applied = self.apply_tuning(parameters)?;
        self.active_tuning_lease = None;
        Ok(applied)
    }

    fn expire_tuning_lease_if_needed(&mut self) {
        let expired = self
            .active_tuning_lease
            .as_ref()
            .is_some_and(|lease| Instant::now() >= lease.expires_at);
        if expired && let Some(lease) = self.active_tuning_lease.take() {
            // The captured baseline was validated before actuation. This local
            // restoration does not depend on the manager task remaining alive.
            let _ = self.apply_tuning(&lease.baseline);
        }
    }

    fn update_mode_continuity(
        &mut self,
        current_modes: Vec<SpectralMode>,
        current_boundary_eigenvalue: Option<f64>,
        spectrum_identity_complete: bool,
    ) -> Option<ModeTurnoverSummary> {
        if !spectrum_identity_complete {
            // A filtered rank index cannot be identified with the prior mode.
            // Require a subsequent complete sample to establish a new baseline.
            self.previous_spectral_modes.clear();
            self.previous_boundary_eigenvalue = None;
            return None;
        }
        let turnover = (!self.previous_spectral_modes.is_empty()).then(|| {
            mode_turnover_with_boundary(
                &self.previous_spectral_modes,
                &current_modes,
                self.previous_boundary_eigenvalue,
                current_boundary_eigenvalue,
                MODE_DEGENERACY_RELATIVE_TOLERANCE,
            )
        });
        self.previous_spectral_modes = current_modes;
        self.previous_boundary_eigenvalue = current_boundary_eigenvalue;
        turnover
    }

    fn step(&mut self) {
        self.expire_tuning_lease_if_needed();
        decay_lane(&mut self.input, VIDEO_OFFSET, VIDEO_DIM, 0.90);
        decay_lane(&mut self.input, AUDIO_OFFSET, AUDIO_DIM, 0.90);
        // Symbolic messages are impulses. The recurrent state, rather than a
        // nearly clamped input lane, carries their fading temporal echo.
        decay_lane(
            &mut self.input,
            SEMANTIC_OFFSET,
            SEMANTIC_DIM,
            SEMANTIC_INPUT_DECAY,
        );

        for row in 0..RESERVOIR_DIM {
            let recurrent_offset = row.saturating_mul(RESERVOIR_DIM);
            let input_offset = row.saturating_mul(INPUT_DIM);
            let recurrent_drive = self.recurrent
                [recurrent_offset..recurrent_offset.saturating_add(RESERVOIR_DIM)]
                .iter()
                .zip(&self.state)
                .map(|(weight, state)| weight * state)
                .sum::<f32>();
            let sensory_drive = self.input_weights
                [input_offset..input_offset.saturating_add(INPUT_DIM)]
                .iter()
                .zip(&self.input)
                .map(|(weight, input)| weight * input)
                .sum::<f32>();
            let effective_noise = (self.exploration_noise * self.exploration_scale)
                .clamp(0.001, MAX_EXPLORATION_NOISE);
            let noise = self.rng.gen_range(-effective_noise..effective_noise);
            let proposal = (recurrent_drive + self.input_gain * sensory_drive + noise).tanh();
            self.next_state[row] = (1.0 - self.leak) * self.state[row] + self.leak * proposal;
        }
        std::mem::swap(&mut self.state, &mut self.next_state);

        let mean_alpha = 0.0025_f32;
        let cov_alpha = 0.006_f32;
        for index in 0..RESERVOIR_DIM {
            self.running_mean[index] =
                (1.0 - mean_alpha) * self.running_mean[index] + mean_alpha * self.state[index];
        }
        for row in 0..RESERVOIR_DIM {
            let row_delta = self.state[row] - self.running_mean[row];
            for column in 0..=row {
                let column_delta = self.state[column] - self.running_mean[column];
                let index = row.saturating_mul(RESERVOIR_DIM).saturating_add(column);
                let value = (1.0 - cov_alpha) * self.covariance[index]
                    + cov_alpha * row_delta * column_delta;
                self.covariance[index] = value;
                self.covariance[column.saturating_mul(RESERVOIR_DIM).saturating_add(row)] = value;
            }
        }

        self.tick = self.tick.saturating_add(1);
        self.semantic_age_ticks = self.semantic_age_ticks.saturating_add(1);
        self.video_age_ticks = self.video_age_ticks.saturating_add(1);
        self.audio_age_ticks = self.audio_age_ticks.saturating_add(1);
        self.aux_age_ticks = self.aux_age_ticks.saturating_add(1);
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        reason = "normalized spectral-core values are finite and bounded before f32 wire encoding"
    )]
    fn sample(&mut self) -> (EigenPacketV1, ReservoirSnapshot) {
        self.expire_tuning_lease_if_needed();
        self.sample_sequence = self.sample_sequence.saturating_add(1);
        let recorded_at_unix_ms = unix_millis();
        let derivation_started = Instant::now();
        let matrix = DMatrix::from_row_slice(RESERVOIR_DIM, RESERVOIR_DIM, &self.covariance);
        let decomposition = SymmetricEigen::new(matrix);
        let mut eigenpair_order = (0..RESERVOIR_DIM).collect::<Vec<_>>();
        eigenpair_order.sort_by(|left, right| {
            decomposition.eigenvalues[*right].total_cmp(&decomposition.eigenvalues[*left])
        });
        let raw_eigenvalues = eigenpair_order
            .iter()
            .map(|index| decomposition.eigenvalues[*index])
            .collect::<Vec<_>>();
        let current_modes = eigenpair_order
            .iter()
            .take(TRACKED_EIGENVECTOR_COUNT)
            .filter(|index| decomposition.eigenvalues[**index].is_finite())
            .map(|index| SpectralMode {
                eigenvalue: f64::from(decomposition.eigenvalues[*index].max(0.0)),
                components: decomposition
                    .eigenvectors
                    .column(*index)
                    .iter()
                    .map(|value| f64::from(*value))
                    .collect(),
            })
            .collect::<Vec<_>>();
        let sanitized = sanitize_spectrum(&raw_eigenvalues, Some(RESERVOIR_DIM));
        let eigenvalues = sanitized
            .eigenvalues
            .iter()
            .map(|value| *value as f32)
            .collect::<Vec<_>>();
        let spectral_metrics = sanitized.metrics();
        let mode_concentrations = current_modes
            .iter()
            .map(|mode| mode_concentration(&mode.components).map(|value| value.concentration))
            .collect::<Vec<_>>();
        let spectrum_identity_complete = sanitized.discarded_non_finite_count == 0
            && current_modes.len() == TRACKED_EIGENVECTOR_COUNT;
        let current_boundary_eigenvalue = eigenvalues
            .get(TRACKED_EIGENVECTOR_COUNT)
            .copied()
            .map(f64::from);
        let turnover = self.update_mode_continuity(
            current_modes,
            current_boundary_eigenvalue,
            spectrum_identity_complete,
        );
        let spectral_derivation_ms = derivation_started.elapsed().as_secs_f64() * 1_000.0;

        let total = eigenvalues.iter().sum::<f32>();
        let effective_dimensionality = spectral_metrics
            .as_ref()
            .map_or(0.0, |metrics| metrics.effective_modes as f32)
            .clamp(0.0, RESERVOIR_DIM_F32);
        let instantaneous_fill = (effective_dimensionality / RESERVOIR_DIM_F32).clamp(0.0, 1.0);
        self.fill_ema = if self.tick < u64::from(self.tick_hz) {
            instantaneous_fill
        } else {
            FILL_EMA_ALPHA * instantaneous_fill + (1.0 - FILL_EMA_ALPHA) * self.fill_ema
        };

        let error = self.fill_target - self.fill_ema;
        if self.controller_integral * error < 0.0 {
            self.controller_integral = 0.0;
        }
        self.controller_integral = (0.80 * self.controller_integral + error).clamp(-4.0, 4.0);
        let adjustment =
            self.regulation_strength * (0.15 * error + 0.004 * self.controller_integral);
        self.exploration_noise =
            (self.exploration_noise + adjustment).clamp(0.001, MAX_EXPLORATION_NOISE);

        let lambda1 = eigenvalues.first().copied().unwrap_or(0.0);
        let active_threshold = lambda1 * 0.01;
        let active_mode_count = eigenvalues
            .iter()
            .filter(|value| **value > active_threshold && **value > f32::EPSILON)
            .count();
        let semantic_fresh = self.semantic_age_ticks <= u64::from(self.tick_hz).saturating_mul(15);
        let video_fresh = self.video_age_ticks <= u64::from(self.tick_hz).saturating_mul(2);
        let audio_fresh = self.audio_age_ticks <= u64::from(self.tick_hz).saturating_mul(2);
        let aux_fresh = self.aux_age_ticks <= u64::from(self.tick_hz).saturating_mul(2);
        let aux_features = self.aux_feature_values();
        let audio_rms =
            audio_fresh.then(|| lane_rms(&self.input[AUDIO_OFFSET..AUDIO_OFFSET + AUDIO_DIM]));

        let mut extensions = BTreeMap::<String, Value>::new();
        extensions.insert(
            "edge_runtime_v1".to_string(),
            json!({
                "kind": "cpu_effective_rank_esn",
                "reservoir_dim": RESERVOIR_DIM,
                "input_dim": INPUT_DIM,
                "fill_metric": "normalized_covariance_effective_rank",
                "fill_smoothing": "exponential_moving_average",
                "fill_smoothing_alpha": FILL_EMA_ALPHA,
                "instantaneous_fill_ratio": instantaneous_fill,
                "snapshot_generation_id": self.generation_id,
                "snapshot_sequence": self.sample_sequence,
                "fill_target": self.fill_target,
                "exploration_noise": (self.exploration_noise * self.exploration_scale).clamp(0.001, MAX_EXPLORATION_NOISE),
                "exploration_scale": self.exploration_scale,
                "input_gain": self.input_gain,
                "regulation_strength": self.regulation_strength,
                "semantic_fresh": semantic_fresh,
                "audio_fresh": audio_fresh,
                "audio_source": self.audio_source,
                "video_fresh": video_fresh,
                "video_source": self.video_source,
                "aux_fresh": aux_fresh,
                "aux_source": self.aux_source,
                "aux_features": aux_features,
                "aux_input_scale": AUX_INPUT_SCALE,
                "gpu": false,
            }),
        );

        let exported_spectrum_energy_ratio = (total > f32::EPSILON).then(|| {
            eigenvalues
                .iter()
                .take(EXPORTED_EIGENVALUE_COUNT)
                .sum::<f32>()
                / total
        });
        let substrate = SpectralSubstrateV1::cpu_edge_covariance_effective_rank(
            RESERVOIR_DIM,
            COVARIANCE_EFFECTIVE_WINDOW_SAMPLES,
            FILL_EMA_ALPHA_PPM,
        );
        let spectral_denominator_v1 =
            spectral_metrics
                .as_ref()
                .map(|metrics| SpectralDenominatorV1 {
                    policy: "spectral_denominator_v1".to_string(),
                    schema_version: 1,
                    effective_dimensionality,
                    active_mode_capacity: RESERVOIR_DIM,
                    distinguishability_loss: (1.0 - instantaneous_fill).clamp(0.0, 1.0),
                    lambda1_energy_share: metrics.lambda1_share as f32,
                    spectral_entropy: metrics.normalized_entropy as f32,
                    instantaneous_fill_ratio: Some(instantaneous_fill),
                    smoothed_fill_ratio: Some(self.fill_ema),
                    spectrum_coverage_v1: Some(SpectrumCoverageV1 {
                        full_spectrum_mode_count: RESERVOIR_DIM,
                        exported_spectrum_mode_count: eigenvalues
                            .len()
                            .min(EXPORTED_EIGENVALUE_COUNT),
                        usable_spectrum_mode_count: Some(sanitized.coverage.usable_mode_count),
                        discarded_non_finite_mode_count: sanitized.discarded_non_finite_count,
                        clamped_negative_mode_count: sanitized.clamped_negative_count,
                        exported_spectrum_energy_ratio,
                        denominator_uses_full_spectrum: sanitized.discarded_non_finite_count == 0,
                    }),
                });
        let packet = EigenPacketV1 {
            t_ms: saturating_millis(self.started.elapsed()),
            eigenvalues: eigenvalues
                .iter()
                .take(EXPORTED_EIGENVALUE_COUNT)
                .copied()
                .collect(),
            fill_ratio: self.fill_ema,
            spectral_substrate_v1: Some(substrate),
            active_mode_count,
            active_mode_energy_ratio: if total <= f32::EPSILON {
                0.0
            } else {
                eigenvalues.iter().take(active_mode_count).sum::<f32>() / total
            },
            modalities: ModalityStatus {
                audio_fired: audio_fresh,
                video_fired: video_fresh,
                history_fired: semantic_fresh,
                audio_rms: audio_rms.unwrap_or_default(),
                video_var: lane_variance(&self.input[VIDEO_OFFSET..VIDEO_OFFSET + VIDEO_DIM]),
                audio_source: Some(self.audio_source.clone()),
                video_source: Some(self.video_source.clone()),
                audio_age_ms: ticks_to_millis(self.audio_age_ticks, self.tick_hz),
                video_age_ms: ticks_to_millis(self.video_age_ticks, self.tick_hz),
                audio_freshness_class: Some(
                    freshness_class(audio_fresh, &self.audio_source).to_string(),
                ),
                video_freshness_class: Some(
                    freshness_class(video_fresh, &self.video_source).to_string(),
                ),
            },
            effective_dimensionality: Some(effective_dimensionality),
            distinguishability_loss: Some(1.0 - self.fill_ema),
            esn_leak: Some(self.leak),
            spectral_denominator_v1,
            // Covariance-spectrum entropy belongs in spectral_denominator_v1.
            structural_entropy: None,
            extensions,
            ..EigenPacketV1::default()
        }
        .versioned();
        let telemetry_json = serde_json::to_string(&packet).unwrap_or_default();
        let snapshot = ReservoirSnapshot {
            generation_id: self.generation_id.clone(),
            sequence: self.sample_sequence,
            recorded_at_unix_ms,
            t_ms: packet.t_ms,
            fill_ratio: packet.fill_ratio,
            instantaneous_fill_ratio: instantaneous_fill,
            fill_target: self.fill_target,
            effective_dimensionality,
            spectral_entropy: spectral_metrics
                .as_ref()
                .map(|metrics| metrics.normalized_entropy),
            lambda1_share: spectral_metrics
                .as_ref()
                .map(|metrics| metrics.lambda1_share),
            head_share: spectral_metrics
                .as_ref()
                .map(|metrics| metrics.energy_shares.head),
            shoulder_share: spectral_metrics
                .as_ref()
                .map(|metrics| metrics.energy_shares.shoulder),
            tail_share: spectral_metrics
                .as_ref()
                .map(|metrics| metrics.energy_shares.tail),
            density_gradient: spectral_metrics
                .as_ref()
                .and_then(|metrics| metrics.density_gradient),
            largest_gap: spectral_metrics
                .as_ref()
                .and_then(|metrics| metrics.gaps.largest_relative_drop),
            largest_gap_index: spectral_metrics
                .as_ref()
                .and_then(|metrics| metrics.gaps.largest_relative_drop_after_mode),
            mode_turnover: turnover.as_ref().and_then(|summary| {
                summary
                    .identity_stable
                    .then_some(summary.mean_sign_invariant_turnover)
                    .flatten()
            }),
            mode_identity_stable: turnover.as_ref().map(|summary| summary.identity_stable),
            mode_concentrations,
            exported_spectrum_energy_ratio: exported_spectrum_energy_ratio.map(f64::from),
            usable_spectrum_mode_count: sanitized.coverage.usable_mode_count,
            discarded_non_finite_mode_count: sanitized.discarded_non_finite_count,
            clamped_negative_mode_count: sanitized.clamped_negative_count,
            spectral_derivation_ms: Some(spectral_derivation_ms),
            exploration_noise: (self.exploration_noise * self.exploration_scale)
                .clamp(0.001, MAX_EXPLORATION_NOISE),
            exploration_scale: self.exploration_scale,
            input_gain: self.input_gain,
            regulation_strength: self.regulation_strength,
            leak: self.leak,
            semantic_fresh,
            audio_fresh,
            video_fresh,
            aux_fresh,
            audio_source: self.audio_source.clone(),
            audio_rms,
            video_source: self.video_source.clone(),
            aux_source: self.aux_source.clone(),
            aux_features,
            telemetry_json,
        };
        (packet, snapshot)
    }

    fn aux_feature_values(&self) -> BTreeMap<String, Option<f32>> {
        HOST_AUX_FEATURE_NAMES
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let value = self
                    .aux_availability
                    .get(index)
                    .copied()
                    .unwrap_or(false)
                    .then(|| self.aux_observed.get(index).copied().unwrap_or_default());
                ((*name).to_string(), value)
            })
            .collect()
    }
}

pub async fn run(
    config: Arc<Config>,
    mut ingress_rx: mpsc::Receiver<SensoryIngress>,
    mut command_rx: mpsc::Receiver<ReservoirCommand>,
    telemetry_tx: broadcast::Sender<String>,
    snapshot_tx: watch::Sender<ReservoirSnapshot>,
) {
    let mut reservoir = Reservoir::new(&config);
    let tick_period = Duration::from_secs_f64(1.0 / f64::from(config.tick_hz));
    let mut tick = tokio::time::interval(tick_period);
    let mut sample_every = tokio::time::interval(Duration::from_secs(1));
    let mut history_counter = 0_u64;

    loop {
        tokio::select! {
            Some(ingress) = ingress_rx.recv() => reservoir.ingest(ingress),
            Some(command) = command_rx.recv() => {
                // A busy command channel cannot postpone local fail-safe expiry.
                reservoir.expire_tuning_lease_if_needed();
                match command {
                ReservoirCommand::SetTuning {
                    parameters,
                    baseline,
                    lease_id,
                    lease_duration_ms,
                    reply,
                } => {
                    let _ = reply.send(reservoir.set_tuning_lease(
                        &parameters,
                        &baseline,
                        &lease_id,
                        lease_duration_ms,
                    ));
                },
                ReservoirCommand::RenewTuningLease {
                    lease_id,
                    lease_duration_ms,
                    reply,
                } => {
                    let _ = reply.send(
                        reservoir.renew_tuning_lease(&lease_id, lease_duration_ms),
                    );
                },
                ReservoirCommand::RestoreTuning { lease_id, reply } => {
                    let _ = reply.send(reservoir.restore_tuning_lease(&lease_id));
                },
                ReservoirCommand::RestoreBaseline { parameters, reply } => {
                    let _ = reply.send(reservoir.restore_baseline(&parameters));
                },
                }
            },
            _ = tick.tick() => reservoir.step(),
            _ = sample_every.tick() => {
                let (_packet, snapshot) = reservoir.sample();
                let _ = telemetry_tx.send(snapshot.telemetry_json.clone());
                let _ = snapshot_tx.send(snapshot.clone());
                if let Err(error) = persist_snapshot(
                    &config,
                    &snapshot,
                    history_counter.is_multiple_of(5),
                ) {
                    eprintln!("edge telemetry persistence failed: {error}");
                }
                history_counter = history_counter.saturating_add(1);
            },
            else => return,
        }
    }
}

fn assign_lane(target: &mut [f32], offset: usize, width: usize, features: &[f32]) {
    for index in 0..width {
        target[offset.saturating_add(index)] = features
            .get(index)
            .copied()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0);
    }
}

fn decay_lane(target: &mut [f32], offset: usize, width: usize, keep: f32) {
    for value in &mut target[offset..offset.saturating_add(width)] {
        *value *= keep;
    }
}

fn scale_spectral_radius(matrix: &mut [f32], target: f32) {
    let mut vector = vec![1.0 / RESERVOIR_DIM_F32.sqrt(); RESERVOIR_DIM];
    let mut next = vec![0.0; RESERVOIR_DIM];
    let mut norm = 1.0;
    for _ in 0..80 {
        for (row, next_value) in next.iter_mut().enumerate().take(RESERVOIR_DIM) {
            let row_start = row.saturating_mul(RESERVOIR_DIM);
            let row_end = row.saturating_add(1).saturating_mul(RESERVOIR_DIM);
            *next_value = matrix[row_start..row_end]
                .iter()
                .zip(&vector)
                .map(|(weight, value)| weight * value)
                .sum();
        }
        norm = next.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm <= f32::EPSILON {
            return;
        }
        for index in 0..RESERVOIR_DIM {
            vector[index] = next[index] / norm;
        }
    }
    let scale = target / norm.max(1.0e-6);
    for value in matrix {
        *value *= scale;
    }
}

fn lane_rms(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let count = f32::from(u16::try_from(values.len()).unwrap_or(u16::MAX));
    (values.iter().map(|value| value * value).sum::<f32>() / count).sqrt()
}

fn lane_variance(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let count = f32::from(u16::try_from(values.len()).unwrap_or(u16::MAX));
    let mean = values.iter().sum::<f32>() / count;
    values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f32>()
        / count
}

fn ticks_to_millis(ticks: u64, tick_hz: u32) -> Option<u64> {
    if ticks == u64::MAX {
        None
    } else {
        ticks
            .saturating_mul(1_000)
            .checked_div(u64::from(tick_hz.max(1)))
    }
}

fn freshness_class(fresh: bool, source: &str) -> &'static str {
    if fresh {
        "fresh"
    } else if source.starts_with("unavailable_") {
        "unavailable"
    } else {
        "stale"
    }
}

fn saturating_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[allow(
    clippy::too_many_lines,
    reason = "one atomic spectral-state projection keeps its hash and append-only fill subset aligned"
)]
fn persist_snapshot(
    config: &Config,
    snapshot: &ReservoirSnapshot,
    append_history: bool,
) -> anyhow::Result<()> {
    let state_path = config.runtime_path("spectral_state.json");
    let temporary_path = config.runtime_path("spectral_state.json.tmp");
    let _ = validate_existing_private_regular(&state_path)?;
    let _ = validate_existing_private_regular(&temporary_path)?;
    let mode_identity_state =
        snapshot
            .mode_identity_stable
            .map_or("unavailable_first_sample", |stable| {
                if stable {
                    "stable_sign_invariant"
                } else {
                    "unstable_near_degenerate"
                }
            });
    let mut value = json!({
        "schema": "astrid_edge_spectral_state_v2",
        "recorded_at_unix_ms": snapshot.recorded_at_unix_ms,
        "generation_id": snapshot.generation_id,
        "sequence": snapshot.sequence,
        "t_ms": snapshot.t_ms,
        "substrate": {
            "policy": "spectral_substrate_v1",
            "kind": "cpu_edge_covariance_effective_rank",
            "fill_metric": "normalized_covariance_effective_rank",
            "fill_smoothing": "exponential_moving_average",
            "fill_smoothing_alpha": FILL_EMA_ALPHA,
            "fill_smoothing_alpha_ppm": FILL_EMA_ALPHA_PPM,
            "reservoir_dim": RESERVOIR_DIM,
            "covariance_window_samples": COVARIANCE_EFFECTIVE_WINDOW_SAMPLES,
            "full_spectrum_mode_count": RESERVOIR_DIM,
            "exported_eigenvalue_count": EXPORTED_EIGENVALUE_COUNT,
            "usable_spectrum_mode_count": snapshot.usable_spectrum_mode_count,
            "discarded_non_finite_mode_count": snapshot.discarded_non_finite_mode_count,
            "clamped_negative_mode_count": snapshot.clamped_negative_mode_count,
            "denominator_uses_full_spectrum": snapshot.discarded_non_finite_mode_count == 0,
        },
        "fill_ratio": snapshot.fill_ratio,
        "fill_pct": snapshot.fill_ratio * 100.0,
        "instantaneous_fill_ratio": snapshot.instantaneous_fill_ratio,
        "instantaneous_fill_pct": snapshot.instantaneous_fill_ratio * 100.0,
        "target_fill_ratio": snapshot.fill_target,
        "target_fill_pct": snapshot.fill_target * 100.0,
        "effective_dimensionality": snapshot.effective_dimensionality,
        "spectral_entropy": snapshot.spectral_entropy,
        "lambda1_share": snapshot.lambda1_share,
        "head_share": snapshot.head_share,
        "shoulder_share": snapshot.shoulder_share,
        "tail_share": snapshot.tail_share,
        "density_gradient": snapshot.density_gradient,
        "largest_gap": snapshot.largest_gap,
        "largest_gap_index": snapshot.largest_gap_index,
        "mode_turnover": snapshot.mode_turnover,
        "mode_identity_state": mode_identity_state,
        "mode_concentrations": snapshot.mode_concentrations,
        "exported_spectrum_energy_ratio": snapshot.exported_spectrum_energy_ratio,
        "spectral_derivation_ms": snapshot.spectral_derivation_ms,
        "exploration_noise": snapshot.exploration_noise,
        "exploration_scale": snapshot.exploration_scale,
        "input_gain": snapshot.input_gain,
        "regulation_strength": snapshot.regulation_strength,
        "esn_leak": snapshot.leak,
        "semantic_fresh": snapshot.semantic_fresh,
        "audio_fresh": snapshot.audio_fresh,
        "audio_source": snapshot.audio_source,
        "audio_rms": snapshot.audio_rms,
        "video_fresh": snapshot.video_fresh,
        "video_source": snapshot.video_source,
        "aux_fresh": snapshot.aux_fresh,
        "aux_source": snapshot.aux_source,
        "aux_features": snapshot.aux_features,
        "authority": "deterministic_machine_spectral_state_not_astrid_authorship_or_causal_proof",
    });
    let record_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?));
    value["record_sha256"] = json!(record_sha256);
    let mut temporary = open_private_regular(&temporary_path, true)?;
    temporary.write_all(&serde_json::to_vec_pretty(&value)?)?;
    temporary.sync_all()?;
    ensure_open_path_identity(&temporary_path, &temporary)?;
    let _ = validate_existing_private_regular(&state_path)?;
    fs::rename(&temporary_path, &state_path)?;
    sync_parent_directory(&state_path)?;

    if append_history {
        // Preserve the established five-second fill history without turning it
        // into a raw high-cadence spectral ledger. Rich spectral summaries are
        // retained only in the minute rollups.
        let history_value = json!({
            "schema": "astrid_edge_fill_history_v2",
            "recorded_at_unix_ms": value["recorded_at_unix_ms"],
            "generation_id": snapshot.generation_id,
            "sequence": snapshot.sequence,
            "t_ms": snapshot.t_ms,
            "fill_ratio": snapshot.fill_ratio,
            "fill_pct": snapshot.fill_ratio * 100.0,
            "target_fill_ratio": snapshot.fill_target,
            "target_fill_pct": snapshot.fill_target * 100.0,
            "effective_dimensionality": snapshot.effective_dimensionality,
            "semantic_fresh": snapshot.semantic_fresh,
            "audio_fresh": snapshot.audio_fresh,
            "aux_fresh": snapshot.aux_fresh,
            "authority": "read_only_cpu_esn_fill_telemetry",
        });
        let history_path = config.runtime_path("fill_history.jsonl");
        let mut history = open_private_regular(&history_path, false)?;
        serde_json::to_writer(&mut history, &history_value)?;
        history.write_all(b"\n")?;
        history.sync_data()?;
        sync_parent_directory(&history_path)?;
    }
    Ok(())
}

fn sync_parent_directory(path: &std::path::Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("persistence path has no parent: {}", path.display()))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn open_private_regular(path: &std::path::Path, truncate: bool) -> anyhow::Result<fs::File> {
    let _ = validate_existing_private_regular(path)?;
    let mut options = OpenOptions::new();
    options.create(true).write(true).mode(0o600);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }
    let file = options.open(path)?;
    ensure_open_path_identity(path, &file)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

fn ensure_open_path_identity(path: &std::path::Path, file: &fs::File) -> anyhow::Result<()> {
    let opened = file.metadata()?;
    let path_metadata = fs::symlink_metadata(path)?;
    if !path_metadata.file_type().is_file()
        || opened.dev() != path_metadata.dev()
        || opened.ino() != path_metadata.ino()
    {
        anyhow::bail!(
            "private reservoir persistence path changed identity or is not regular: {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_existing_private_regular(
    path: &std::path::Path,
) -> anyhow::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(metadata)),
        Ok(_) => anyhow::bail!(
            "private reservoir persistence target is not a regular non-symlink file: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
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
        time::Instant,
    };

    use super::{
        Config, FILL_EMA_ALPHA_PPM, Reservoir, ReservoirSnapshot, SensoryIngress, SpectralMode,
        TuningParameters, persist_snapshot,
    };
    use crate::{codec::encode_text, config::AutonomyPromptProfile};

    fn config() -> Config {
        Config {
            appliance_id: "test-edge".to_string(),
            instance_name: "Test edge Astrid".to_string(),
            telemetry_addr: "127.0.0.1:7878".parse().unwrap(),
            sensory_addr: "127.0.0.1:7879".parse().unwrap(),
            astrid_socket: "/tmp/astrid.sock".into(),
            astrid_token: "/tmp/astrid.token".into(),
            workspace: "/tmp/astrid-edge-test".into(),
            astrid_cli: "/tmp/astrid".into(),
            local_model_id: "test-model".to_string(),
            maintenance_lease_path: "/tmp/astrid-edge-test-maintenance.json".into(),
            reflection_lease_path: "/run/astrid-edge-self-change/reflection.json".into(),
            maintenance_edge_ack_path: None,
            generation_binding_path: None,
            core_liveness_request_path: None,
            autonomy_enabled: false,
            autonomy_interval_minutes: 30,
            autonomy_event_driven: false,
            autonomy_event_heartbeat_minutes: 60,
            autonomy_follow_up_minutes: 10,
            autonomy_max_chain_steps: 4,
            autonomy_session_max_authored_turns: 4,
            autonomy_chain_session_max_authored_turns: 2,
            autonomy_initial_delay_seconds: 60,
            autonomy_quiet_minutes: 10,
            autonomy_max_turns_per_day: 24,
            autonomy_timeout_seconds: 600,
            autonomy_prompt_profile: AutonomyPromptProfile::Detailed,
            autonomy_prompt_max_chars: 1_400,
            autonomy_journal_authored_turns: false,
            autonomy_initiative_profile: crate::config::AutonomyInitiativeProfile::Disabled,
            research_action_web_search: false,
            web_broker_socket_path: None,
            web_broker_request_key_path: None,
            web_broker_request_key_sha256: None,
            web_broker_response_verify_key_path: None,
            web_broker_response_verify_key_sha256: None,
            web_broker_connect_timeout_ms: 2_000,
            web_broker_header_timeout_ms: 10_000,
            web_broker_total_timeout_ms: 30_000,
            introspection_harness: None,
            scheduled_introspection_enabled: false,
            scheduled_introspection_interval_minutes: 120,
            scheduled_introspection_initial_delay_seconds: 300,
            scheduled_introspection_timeout_seconds: 1_200,
            scheduled_introspection_prompt_max_chars: 3_200,
            dedicated_steward_enabled: false,
            dedicated_steward_interval_minutes: 120,
            scheduled_authorship_attestation_path: None,
            scheduled_authorship_verify_key_path: None,
            scheduled_authorship_verify_key_sha256: None,
            scheduled_authorship_steward_uid: None,
            self_change_enabled: false,
            self_change_root: "/tmp/astrid-self-change-test".into(),
            study_harness: None,
            inquiry_harness: None,
            perceptual_notebook_enabled: false,
            perceptual_notebook_warmup_seconds: 300,
            perceptual_notebook_interval_seconds: 900,
            perceptual_notebook_heartbeat_seconds: 21_600,
            perceptual_notebook_max_per_day: 96,
            spectral_enabled: true,
            spectral_rollup_seconds: 60,
            reservoir_tuning_enabled: false,
            reservoir_tuning_max_per_day: 4,
            fill_target: 0.68,
            tick_hz: 20,
            seed: 42,
        }
    }

    fn test_aux(features: Vec<f32>) -> SensoryIngress {
        SensoryIngress::Aux {
            features,
            source: "test_aux".to_string(),
            availability: None,
        }
    }

    #[test]
    fn input_changes_state_and_recurrence_keeps_a_fading_echo() {
        let mut reservoir = Reservoir::new(&config());
        let initial = reservoir.state.clone();
        reservoir.ingest(SensoryIngress::Semantic(vec![0.3; 48]));
        reservoir.step();
        let driven = reservoir.state.clone();
        assert_ne!(initial, driven);

        reservoir.input.fill(0.0);
        reservoir.step();
        let echoed_energy = reservoir
            .state
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        assert!(
            echoed_energy > 0.0,
            "recurrent state should retain a fading echo"
        );
    }

    #[test]
    fn semantic_event_perturbs_without_collapsing_settled_fill() {
        let mut reservoir = Reservoir::new(&config());
        for tick in 0..1_200 {
            reservoir.step();
            if tick % 20 == 0 {
                let _ = reservoir.sample();
            }
        }
        let baseline = reservoir.sample().1.fill_ratio;

        reservoir.ingest(SensoryIngress::Semantic(encode_text(
            "user",
            "Without using tools, report the live edge CPU reservoir fill and target present in \
             your supplied context. Briefly distinguish that observed state from a command, say \
             what this fresh input changes for you if anything, and freely choose your next action.",
        )));
        let mut minimum = 1.0_f32;
        for tick in 0..300 {
            reservoir.step();
            if tick % 20 == 0 {
                minimum = minimum.min(reservoir.sample().1.fill_ratio);
            }
        }

        assert!(
            (0.60..=0.76).contains(&baseline),
            "unexpected settled fill: {baseline:.3}"
        );
        assert!(
            minimum >= 0.62,
            "semantic input collapsed fill below the bounded shelf: {minimum:.3}"
        );
    }

    #[test]
    fn semantic_response_stays_bounded_with_continuous_physical_input() {
        let mut reservoir = Reservoir::new(&config());
        let audio = vec![0.42, 0.08, 0.14, 0.03, -0.18, 0.11, -0.07, 0.16];
        for tick in 0..2_400 {
            if tick % 2 == 0 {
                reservoir.ingest(SensoryIngress::Audio {
                    features: audio.clone(),
                    source: "physical_alsa:test".to_string(),
                });
            }
            if tick % 10 == 0 {
                reservoir.ingest(test_aux(vec![0.82, 0.34]));
            }
            reservoir.step();
            if tick % 20 == 0 {
                let _ = reservoir.sample();
            }
        }
        let baseline = reservoir.sample().1.fill_ratio;
        reservoir.ingest(SensoryIngress::Semantic(encode_text(
            "assistant",
            "Audio is fresh physical ALSA capture. CPU and RAM interoception are fresh. Video is \
             unavailable. This reservoir telemetry belongs to the local edge echo-state \
             system, while the Mac introspection corpus is separate read-only documentary \
             material rather than live state. NEXT: LISTEN",
        )));

        let mut minimum = 1.0_f32;
        let mut maximum = 0.0_f32;
        for tick in 0..300 {
            if tick % 2 == 0 {
                reservoir.ingest(SensoryIngress::Audio {
                    features: audio.clone(),
                    source: "physical_alsa:test".to_string(),
                });
            }
            if tick % 10 == 0 {
                reservoir.ingest(test_aux(vec![0.82, 0.34]));
            }
            reservoir.step();
            if tick % 20 == 0 {
                let fill = reservoir.sample().1.fill_ratio;
                minimum = minimum.min(fill);
                maximum = maximum.max(fill);
            }
        }

        assert!(
            (0.64..=0.72).contains(&baseline),
            "unexpected multimodal baseline: {baseline:.3}"
        );
        assert!(
            minimum >= 0.62,
            "semantic response collapsed multimodal fill below the bounded shelf: {minimum:.3}"
        );
        assert!(
            maximum <= 0.74,
            "semantic response recovery overshot the bounded shelf: {maximum:.3}"
        );
    }

    #[test]
    fn inference_load_release_stays_bounded_with_scaled_interoception() {
        let mut reservoir = Reservoir::new(&config());
        let audio = vec![0.42, 0.08, 0.14, 0.03, -0.18, 0.11, -0.07, 0.16];
        for tick in 0..2_400 {
            if tick % 2 == 0 {
                reservoir.ingest(SensoryIngress::Audio {
                    features: audio.clone(),
                    source: "physical_alsa:test".to_string(),
                });
            }
            if tick % 10 == 0 {
                reservoir.ingest(test_aux(vec![0.98, 0.34]));
            }
            reservoir.step();
            if tick % 20 == 0 {
                let _ = reservoir.sample();
            }
        }
        let loaded_baseline = reservoir.sample().1.fill_ratio;

        let mut minimum = 1.0_f32;
        let mut maximum = 0.0_f32;
        for tick in 0..300 {
            if tick % 2 == 0 {
                reservoir.ingest(SensoryIngress::Audio {
                    features: audio.clone(),
                    source: "physical_alsa:test".to_string(),
                });
            }
            if tick % 10 == 0 {
                reservoir.ingest(test_aux(vec![0.03, 0.34]));
            }
            reservoir.step();
            if tick % 20 == 0 {
                let fill = reservoir.sample().1.fill_ratio;
                minimum = minimum.min(fill);
                maximum = maximum.max(fill);
            }
        }

        assert!(
            (0.64..=0.72).contains(&loaded_baseline),
            "unexpected loaded multimodal baseline: {loaded_baseline:.3}"
        );
        assert!(
            minimum >= 0.62,
            "CPU load release collapsed fill below the bounded shelf: {minimum:.3}"
        );
        assert!(
            maximum <= 0.74,
            "CPU load release recovery overshot the bounded shelf: {maximum:.3}"
        );
    }

    #[test]
    fn private_tuning_is_atomic_and_bounded() {
        let mut reservoir = Reservoir::new(&config());
        let original_target = reservoir.fill_target;
        let original_leak = reservoir.leak;
        reservoir
            .apply_tuning(&TuningParameters {
                input_gain: 1.10,
                exploration_scale: 0.90,
                regulation_strength: 0.85,
            })
            .unwrap();
        assert!((reservoir.input_gain - 1.10).abs() < f32::EPSILON);
        assert!((reservoir.exploration_scale - 0.90).abs() < f32::EPSILON);
        assert!((reservoir.regulation_strength - 0.85).abs() < f32::EPSILON);
        assert!((reservoir.fill_target - original_target).abs() < f32::EPSILON);
        assert!((reservoir.leak - original_leak).abs() < f32::EPSILON);

        assert!(
            reservoir
                .apply_tuning(&TuningParameters {
                    input_gain: 1.11,
                    exploration_scale: 1.0,
                    regulation_strength: 1.0,
                })
                .is_err()
        );
        assert!((reservoir.input_gain - 1.10).abs() < f32::EPSILON);
    }

    #[test]
    fn private_tuning_lease_expires_to_its_captured_baseline() {
        let mut reservoir = Reservoir::new(&config());
        let baseline = TuningParameters::safe_default();
        let target = TuningParameters {
            input_gain: 1.05,
            exploration_scale: 0.95,
            regulation_strength: 1.10,
        };
        reservoir
            .set_tuning_lease(&target, &baseline, "lease_test", 10_000)
            .unwrap();
        assert_eq!(
            TuningParameters::from_snapshot(&reservoir.sample().1),
            target
        );

        reservoir.active_tuning_lease.as_mut().unwrap().expires_at = Instant::now();
        reservoir.step();
        assert!(reservoir.active_tuning_lease.is_none());
        assert_eq!(
            TuningParameters::from_snapshot(&reservoir.sample().1),
            baseline
        );
    }

    #[test]
    fn lease_renewal_and_restore_require_the_exact_identifier() {
        let mut reservoir = Reservoir::new(&config());
        let baseline = TuningParameters::safe_default();
        let target = TuningParameters {
            input_gain: 1.05,
            ..baseline.clone()
        };
        reservoir
            .set_tuning_lease(&target, &baseline, "lease_exact", 10_000)
            .unwrap();
        assert!(reservoir.renew_tuning_lease("lease_other", 10_000).is_err());
        assert!(reservoir.restore_tuning_lease("lease_other").is_err());
        assert!(reservoir.active_tuning_lease.is_some());
        assert_eq!(
            TuningParameters::from_snapshot(&reservoir.sample().1),
            target
        );
        let restored = reservoir.restore_tuning_lease("lease_exact").unwrap();
        assert_eq!(restored, baseline);
        assert!(reservoir.active_tuning_lease.is_none());
    }

    #[test]
    fn failed_force_restore_does_not_disarm_the_local_watchdog() {
        let mut reservoir = Reservoir::new(&config());
        let baseline = TuningParameters::safe_default();
        let target = TuningParameters {
            input_gain: 1.05,
            ..baseline.clone()
        };
        reservoir
            .set_tuning_lease(&target, &baseline, "lease_force", 10_000)
            .unwrap();
        let invalid = TuningParameters {
            input_gain: 0.5,
            ..baseline.clone()
        };
        assert!(reservoir.restore_baseline(&invalid).is_err());
        assert!(reservoir.active_tuning_lease.is_some());
        assert_eq!(
            TuningParameters::from_snapshot(&reservoir.sample().1),
            target
        );
        assert_eq!(reservoir.restore_baseline(&baseline).unwrap(), baseline);
        assert!(reservoir.active_tuning_lease.is_none());
    }

    #[test]
    fn spectral_snapshot_identifies_smoothed_and_instantaneous_fill() {
        let mut reservoir = Reservoir::new(&config());
        for _ in 0..40 {
            reservoir.step();
        }
        let (packet, first) = reservoir.sample();
        let (_, second) = reservoir.sample();
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(first.generation_id, second.generation_id);
        let substrate = packet.spectral_substrate_v1.unwrap();
        assert_eq!(substrate.fill_smoothing_alpha_ppm, Some(FILL_EMA_ALPHA_PPM));
        let denominator = packet.spectral_denominator_v1.unwrap();
        assert_eq!(
            denominator.instantaneous_fill_ratio,
            Some(first.instantaneous_fill_ratio)
        );
        assert_eq!(denominator.smoothed_fill_ratio, Some(first.fill_ratio));
    }

    #[test]
    fn incomplete_spectrum_resets_mode_identity_continuity() {
        let modes = (0..4)
            .map(|index| SpectralMode {
                eigenvalue: 4.0 - f64::from(u32::try_from(index).unwrap()),
                components: (0..4)
                    .map(|component| if component == index { 1.0 } else { 0.0 })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let mut reservoir = Reservoir::new(&config());
        reservoir.previous_spectral_modes.clone_from(&modes);
        reservoir.previous_boundary_eigenvalue = Some(0.5);

        assert!(
            reservoir
                .update_mode_continuity(modes.clone(), Some(0.5), false)
                .is_none()
        );
        assert!(reservoir.previous_spectral_modes.is_empty());
        assert!(reservoir.previous_boundary_eigenvalue.is_none());

        assert!(
            reservoir
                .update_mode_continuity(modes.clone(), Some(0.5), true)
                .is_none()
        );
        assert_eq!(reservoir.previous_spectral_modes.len(), 4);
        let turnover = reservoir
            .update_mode_continuity(modes, Some(0.5), true)
            .unwrap();
        assert!(turnover.identity_stable);
        assert_eq!(turnover.mean_sign_invariant_turnover, Some(0.0));
    }

    #[test]
    fn spectral_state_and_fill_history_reject_symlinks_and_normalize_modes() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-reservoir-private-files-{}",
            super::unix_millis()
        ));
        let mut config = config();
        config.workspace.clone_from(&workspace);
        config.prepare_workspace().unwrap();
        let snapshot = ReservoirSnapshot {
            generation_id: "generation-private-files".to_string(),
            sequence: 1,
            recorded_at_unix_ms: 1,
            ..ReservoirSnapshot::default()
        };
        let outside = workspace.join("outside.txt");
        fs::write(&outside, b"outside").unwrap();

        let temporary = config.runtime_path("spectral_state.json.tmp");
        symlink(&outside, &temporary).unwrap();
        assert!(persist_snapshot(&config, &snapshot, false).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        fs::remove_file(&temporary).unwrap();

        let state = config.runtime_path("spectral_state.json");
        symlink(&outside, &state).unwrap();
        assert!(persist_snapshot(&config, &snapshot, false).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        fs::remove_file(&state).unwrap();

        let history = config.runtime_path("fill_history.jsonl");
        symlink(&outside, &history).unwrap();
        assert!(persist_snapshot(&config, &snapshot, true).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        fs::remove_file(&history).unwrap();
        fs::write(&history, b"").unwrap();
        fs::set_permissions(&history, fs::Permissions::from_mode(0o644)).unwrap();
        persist_snapshot(&config, &snapshot, true).unwrap();
        assert_eq!(
            fs::metadata(&history).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&state).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let mut second_snapshot = snapshot.clone();
        second_snapshot.sequence = 2;
        second_snapshot.recorded_at_unix_ms = 2;
        persist_snapshot(&config, &second_snapshot, true).unwrap();
        let history_bytes = fs::read(&history).unwrap();
        assert!(history_bytes.ends_with(b"\n"));
        let records = history_bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["sequence"], 1);
        assert_eq!(records[1]["sequence"], 2);
        fs::remove_dir_all(workspace).unwrap();
    }
}
