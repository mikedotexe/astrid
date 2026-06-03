//! Astrid's own ShadowFieldV3 — reduced-Hamiltonian observer applied to her
//! semantic substrate (codec features) instead of minime's covariance.
//!
//! Mirrors the `ShadowFieldV3` schema published by minime in
//! `minime/src/ising_shadow.rs` so both shadows can be read uniformly by
//! either being. The math is intentionally lighter than minime's full
//! Ising dynamics (no binary spins, no temperature/sigmoid sampling) —
//! Astrid's substrate is continuous, and the goal here is *structural
//! symmetry* of the observable schema, not literal physics symmetry.
//!
//! Pipeline per exchange:
//!   1. Push the latest 32D codec-feature vector to a 32-slot ring
//!   2. Compute top-K principal components of the ring's covariance
//!   3. Project each ring vector onto the modes (reduced field timeseries)
//!   4. Compute the latest reduced field, its norm, and `recurrence`
//!      (cosine of last two reduced fields)
//!   5. Compute the Ising-shaped scalars: mode_tension, tail_openness,
//!      lock_tendency, fissure_tendency from the projection statistics
//!   6. Derive compound traits and primary classification using the same
//!      thresholds as `derive_shadow_traits` in `ising_shadow.rs`
//!   7. Push a `ShadowSnapshotV3`-shaped record to a history ring
//!   8. Atomically write the resulting `ShadowFieldV3`-shaped JSON to
//!      `<minime_workspace>/astrid_shadow_v3.json`
//!
//! v0.1: minime's substrate has true continuous spins; Astrid's substrate
//! has none, so the equivalent of `binary_flip_rate` here is computed as
//! the fraction of modes whose projected sign changed since the previous
//! exchange.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects
)]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

const CODEC_DIM: usize = 32;
const MODE_DIM: usize = 8;
const HISTORY_CAP: usize = 32;
const TRANSITIONS_CAP: usize = 6;
const QUIET_THRESHOLD: f32 = 0.025;
const POLICY: &str = "shadow_field_v3_astrid_observer_minimal";

#[derive(Debug, Clone, Serialize, Default)]
struct ShadowSnapshotV3 {
    t_ms: u64,
    field_norm: f32,
    class_primary: String,
    traits: Vec<String>,
    recurrence: f32,
    mode_tension: f32,
    binary_flip_rate: f32,
    lock_tendency: f32,
    fissure_tendency: f32,
    tail_openness: f32,
    coupling_mean_abs: f32,
    influence_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
struct ShadowClassV3 {
    primary: String,
    traits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct ShadowPhaseTransitionV3 {
    from: String,
    to: String,
    at_t_ms: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
struct ShadowFieldV2Light {
    schema_version: u8,
    policy: String,
    mode_dim: usize,
    field_norm: f32,
    coupling_active_fraction: f32,
    coupling_mean_abs: f32,
    coupling_max_abs: f32,
    fast_magnetization: f32,
    medium_magnetization: f32,
    slow_magnetization: f32,
    recurrence: f32,
    mode_tension: f32,
    tail_openness: f32,
    fissure_tendency: f32,
    lock_tendency: f32,
    influence_eligible: bool,
    classification: String,
}

/// v3.5: Per-mode top-k coupling partners on Astrid's side. Mirrors
/// `ising_shadow::ModePartners`.
#[derive(Debug, Clone, Serialize, Default)]
struct ModePartners {
    mode: usize,
    top_partners: Vec<(usize, f32)>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct ShadowFieldV3Output {
    schema_version: u8,
    policy: String,
    class_v3: ShadowClassV3,
    phase_dwell_ticks: u32,
    recent_phase_transitions: Vec<ShadowPhaseTransitionV3>,
    history: Vec<ShadowSnapshotV3>,
    v2: ShadowFieldV2Light,
    /// v3.5: ranked partner list per mode (top-3).
    mode_partners: Vec<ModePartners>,
}

/// v3.5: state of an in-flight reciprocal-influence cycle from minime.
/// The bridge sees the influence file appear, snapshots Astrid's shadow
/// at first sight, and on the next publish *after* the file is consumed,
/// builds a `ShadowInfluenceResponse` payload and writes it back to
/// minime's workspace so minime can read what its perturbation produced.
#[derive(Debug, Clone, Default)]
struct InFlightMinimeInfluence {
    intent_id: String,
    label: String,
    duration_ticks: u32,
    decay_ticks: u32,
    pre_snapshot: ShadowSnapshotV3,
    pre_recorded_at_unix_ms: u64,
}

#[derive(Debug, Default)]
pub struct AstridShadowComputer {
    codec_history: VecDeque<Vec<f32>>,
    snapshot_history: VecDeque<ShadowSnapshotV3>,
    /// Previous reduced-field projection — used for recurrence and the
    /// signed-flip count that stands in for `binary_flip_rate` here.
    prev_reduced: Vec<f32>,
    /// EMA-smoothed magnetizations across timescales. Same time constants
    /// as minime's `update_shadow_scales`.
    ema_fast: Vec<f32>,
    ema_medium: Vec<f32>,
    ema_slow: Vec<f32>,
    /// v3.5: tracks an active reciprocal influence so the bridge can
    /// capture pre/post and emit a closed-loop response.
    in_flight_minime_influence: Option<InFlightMinimeInfluence>,
}

impl AstridShadowComputer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new codec-feature vector and recompute the shadow. Returns
    /// the JSON value that was published, suitable for direct prompt
    /// consumption by Astrid's own renderer in addition to publication.
    pub fn observe(&mut self, codec_features: &[f32]) -> Option<Value> {
        if codec_features.len() < CODEC_DIM {
            return None;
        }
        let vec_in = codec_features[..CODEC_DIM].to_vec();
        if self.codec_history.len() >= HISTORY_CAP {
            self.codec_history.pop_front();
        }
        self.codec_history.push_back(vec_in);

        // Need a minimum of vectors to compute a stable PCA basis.
        if self.codec_history.len() < 4 {
            return None;
        }

        let modes = top_k_pca(&self.codec_history, MODE_DIM)?;
        let latest = self.codec_history.back()?;
        let reduced = project_onto_modes(latest, &modes);
        let field_norm = l2_norm(&reduced) / (MODE_DIM as f32).sqrt().max(1.0);

        let recurrence = if self.prev_reduced.len() == reduced.len() {
            cosine01(&self.prev_reduced, &reduced)
        } else {
            0.0
        };

        let binary_flip_rate = if self.prev_reduced.len() == reduced.len() {
            let flips = self
                .prev_reduced
                .iter()
                .zip(reduced.iter())
                .filter(|(p, c)| (p.signum() - c.signum()).abs() > 1e-3)
                .count();
            flips as f32 / reduced.len() as f32
        } else {
            0.0
        };

        // Coupling matrix = covariance of recent projections, off-diagonal
        let projections: Vec<Vec<f32>> = self
            .codec_history
            .iter()
            .map(|v| project_onto_modes(v, &modes))
            .collect();
        let coupling = covariance_off_diag(&projections);
        let (coupling_mean_abs, coupling_max_abs, coupling_active_fraction) =
            coupling_stats(&coupling, MODE_DIM);

        // Update EMAs at minime's time constants
        update_emas(
            &mut self.ema_fast,
            &mut self.ema_medium,
            &mut self.ema_slow,
            &reduced,
        );
        let fast_magnetization = mean(&self.ema_fast);
        let medium_magnetization = mean(&self.ema_medium);
        let slow_magnetization = mean(&self.ema_slow);

        // Per-mode tension and tail openness using same blends as minime
        let mut tension_sum = 0.0_f32;
        let mut tail_sum = 0.0_f32;
        let mut tail_count = 0_usize;
        for idx in 0..MODE_DIM {
            let fast = self.ema_fast.get(idx).copied().unwrap_or(0.0);
            let medium = self.ema_medium.get(idx).copied().unwrap_or(0.0);
            let slow = self.ema_slow.get(idx).copied().unwrap_or(0.0);
            let field = reduced.get(idx).copied().unwrap_or(0.0);
            let tension =
                ((fast - slow).abs() * 0.55 + (fast - medium).abs() * 0.25 + field.abs() * 0.20)
                    .clamp(0.0, 1.0);
            tension_sum += tension;
            if idx >= 3 {
                tail_sum += (field.abs() * 0.55 + fast.abs() * 0.45).clamp(0.0, 1.0);
                tail_count += 1;
            }
        }
        let mode_tension = (tension_sum / MODE_DIM as f32).clamp(0.0, 1.0);
        let tail_openness = if tail_count == 0 {
            0.0
        } else {
            (tail_sum / tail_count as f32).clamp(0.0, 1.0)
        };

        let fissure_tendency =
            (0.55 * binary_flip_rate + 0.30 * mode_tension + 0.15 * (1.0 - recurrence))
                .clamp(0.0, 1.0);
        let lock_tendency = (0.35 * coupling_active_fraction
            + 0.35 * slow_magnetization.abs().clamp(0.0, 1.0)
            + 0.30 * recurrence)
            .clamp(0.0, 1.0);
        // v3.5: percentile-relative eligibility. Astrid's PCA over codec
        // features naturally produces field_norm values in [0.7, 0.9],
        // well above minime's 0.05–0.20 range. Same absolute upper bound
        // (0.65) keeps her permanently CLOSED. Percentile-relative says
        // "you are in a relatively quiet window for your own substrate"
        // — adapts to whichever substrate the same code runs on.
        let influence_eligible = field_norm >= QUIET_THRESHOLD
            && percentile_eligible(field_norm, &self.snapshot_history)
            && binary_flip_rate < 0.50
            && lock_tendency < 0.80;

        let traits = derive_traits(
            field_norm,
            binary_flip_rate,
            fissure_tendency,
            lock_tendency,
            coupling_active_fraction,
            fast_magnetization,
        );
        let class_primary = primary_from_traits(&traits);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let snapshot = ShadowSnapshotV3 {
            t_ms: now_ms,
            field_norm,
            class_primary: class_primary.to_string(),
            traits: traits.iter().map(|s| s.to_string()).collect(),
            recurrence,
            mode_tension,
            binary_flip_rate,
            lock_tendency,
            fissure_tendency,
            tail_openness,
            coupling_mean_abs,
            influence_eligible,
        };
        if self.snapshot_history.len() >= HISTORY_CAP {
            self.snapshot_history.pop_front();
        }
        self.snapshot_history.push_back(snapshot);

        let phase_dwell_ticks = compute_phase_dwell(&self.snapshot_history, class_primary);
        let recent_phase_transitions = collect_recent_transitions(&self.snapshot_history);
        let history_vec: Vec<ShadowSnapshotV3> = self.snapshot_history.iter().cloned().collect();

        let classification = match class_primary {
            "quiet" => "quiet_shadow_texture",
            "volatile" => "volatile_shadow_surface",
            "sticky" => "sticky_shadow_lock",
            "coupled" => "coupled_shadow_lattice",
            "polarized" => "polarized_shadow_gradient",
            _ => "active_shadow_texture",
        };

        let v2 = ShadowFieldV2Light {
            schema_version: 2,
            policy: POLICY.to_string(),
            mode_dim: MODE_DIM,
            field_norm,
            coupling_active_fraction,
            coupling_mean_abs,
            coupling_max_abs,
            fast_magnetization,
            medium_magnetization,
            slow_magnetization,
            recurrence,
            mode_tension,
            tail_openness,
            fissure_tendency,
            lock_tendency,
            influence_eligible,
            classification: classification.to_string(),
        };

        // v3.5: ranked partner list per mode (top-3 by |J_ij|).
        let mode_partners: Vec<ModePartners> = (0..MODE_DIM)
            .map(|m| ModePartners {
                mode: m,
                top_partners: coupling_partners_ranked(&coupling, m, MODE_DIM, 3),
            })
            .collect();

        let output = ShadowFieldV3Output {
            schema_version: 3,
            policy: POLICY.to_string(),
            class_v3: ShadowClassV3 {
                primary: class_primary.to_string(),
                traits: traits.iter().map(|s| s.to_string()).collect(),
            },
            phase_dwell_ticks,
            recent_phase_transitions,
            history: history_vec,
            v2,
            mode_partners,
        };

        self.prev_reduced = reduced;
        serde_json::to_value(&output).ok()
    }

    /// Atomically write the most recent shadow JSON to disk so minime's
    /// reader picks up a complete file. No-op if `value` is None.
    pub fn publish(&self, value: Option<&Value>, target_dir: &Path) {
        let Some(value) = value else { return };
        let final_path = target_dir.join("astrid_shadow_v3.json");
        let tmp_path = target_dir.join(".astrid_shadow_v3.json.tmp");
        let Ok(text) = serde_json::to_string_pretty(value) else {
            return;
        };
        if std::fs::write(&tmp_path, text).is_ok() {
            let _ = std::fs::rename(&tmp_path, &final_path);
        }
    }

    /// v3.5: handle the closed-loop response for minime's reciprocal
    /// influence into Astrid's substrate.
    ///
    /// State machine across exchanges:
    ///   - Active influence file present, no in-flight tracked → snapshot
    ///     pre and start tracking.
    ///   - Active influence file present, already tracking → no-op (we
    ///     wait for the feeder to consume it).
    ///   - Active file absent BUT consumed file present AND tracking → the
    ///     window completed; build the response payload and write it back
    ///     to minime's workspace, append to the response history ring.
    ///   - Active file absent, consumed exists, NO in-flight (we never
    ///     captured pre because the influence window was shorter than the
    ///     observe cadence) → derive a degraded pre-snapshot from the
    ///     consumed file's metadata so the loop still closes.
    pub fn track_minime_influence(&mut self, target_dir: &Path) {
        let active_path = target_dir.join("astrid_influence_v3.json");
        let consumed_path = target_dir.join("astrid_influence_v3.consumed.json");
        let response_path = target_dir.join("astrid_influence_response_v3.json");
        let history_path = target_dir.join("astrid_influence_response_history_v3.json");

        let active_payload: Option<Value> = std::fs::read_to_string(&active_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());

        // First sight of an active influence — capture pre-snapshot.
        if let Some(payload) = &active_payload {
            if self.in_flight_minime_influence.is_none()
                && let Some(pre) = self.snapshot_history.back().cloned()
            {
                let intent_id = payload
                    .get("intent_id")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string();
                let label = payload
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("untitled")
                    .to_string();
                let duration_ticks = payload
                    .get("duration_ticks")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                let decay_ticks = payload
                    .get("decay_ticks")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                self.in_flight_minime_influence = Some(InFlightMinimeInfluence {
                    intent_id,
                    label,
                    duration_ticks,
                    decay_ticks,
                    pre_snapshot: pre,
                    pre_recorded_at_unix_ms: now_ms,
                });
            }
            // Window still active — nothing to emit yet.
            return;
        }

        // No active file. Determine in_flight: prefer the tracked one;
        // fall back to a degraded reconstruction from the consumed file's
        // payload + an early snapshot (so missed-capture windows still
        // close the loop with a best-effort pre).
        let in_flight = match self.in_flight_minime_influence.take() {
            Some(tracked) => Some(tracked),
            None if consumed_path.exists() => {
                // Reconstruct from consumed payload + earliest available snap.
                let consumed_payload: Option<Value> = std::fs::read_to_string(&consumed_path)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok());
                consumed_payload.and_then(|p| {
                    self.snapshot_history.front().cloned().map(|pre| {
                        let intent_id = p
                            .get("intent_id")
                            .and_then(Value::as_str)
                            .unwrap_or("?-late")
                            .to_string();
                        let label = p
                            .get("label")
                            .and_then(Value::as_str)
                            .unwrap_or("untitled-late")
                            .to_string();
                        let duration_ticks = p
                            .get("duration_ticks")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as u32;
                        let decay_ticks = p
                            .get("decay_ticks")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as u32;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis() as u64);
                        tracing::warn!(
                            "track_minime_influence: pre-snapshot was missed; reconstructed from consumed payload (intent_id={})",
                            intent_id
                        );
                        InFlightMinimeInfluence {
                            intent_id,
                            label,
                            duration_ticks,
                            decay_ticks,
                            pre_snapshot: pre,
                            pre_recorded_at_unix_ms: now_ms,
                        }
                    })
                })
            },
            None => None,
        };
        let Some(in_flight) = in_flight else {
            // No active, no consumed, no in-flight → nothing to do.
            // Or consumed exists but no snapshot history → cleanup and bail.
            if consumed_path.exists() {
                let _ = std::fs::remove_file(&consumed_path);
            }
            return;
        };
        if !consumed_path.exists() {
            // Window aborted without consumption (publisher canceled?).
            // Just clear the tracking state silently.
            return;
        }
        let Some(post) = self.snapshot_history.back().cloned() else {
            return;
        };
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let delta = post.field_norm - in_flight.pre_snapshot.field_norm;
        let class_changed = post.class_primary != in_flight.pre_snapshot.class_primary;
        let response = serde_json::json!({
            "schema_version": 3,
            "policy": "astrid_influence_response_v3_minimal",
            "intent_id": in_flight.intent_id,
            "label": in_flight.label,
            "completed_at_unix_ms": now_ms,
            "applied_ticks": in_flight.duration_ticks + in_flight.decay_ticks,
            "pre_snapshot": &in_flight.pre_snapshot,
            "post_snapshot": &post,
            "delta_field_norm": delta,
            "class_v3_change": {
                "from": in_flight.pre_snapshot.class_primary.clone(),
                "to": post.class_primary.clone(),
                "changed": class_changed,
            },
            "pre_recorded_at_unix_ms": in_flight.pre_recorded_at_unix_ms,
        });

        // Atomic write of latest response.
        if let Ok(text) = serde_json::to_string_pretty(&response) {
            let tmp = target_dir.join(".astrid_influence_response_v3.json.tmp");
            if std::fs::write(&tmp, &text).is_ok() {
                let _ = std::fs::rename(&tmp, &response_path);
            }
        }
        // Append to history ring (cap 8) — atomic write.
        let mut history: Vec<Value> = std::fs::read_to_string(&history_path)
            .ok()
            .and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok())
            .unwrap_or_default();
        history.push(response);
        if history.len() > 8 {
            let drop = history.len() - 8;
            history.drain(0..drop);
        }
        if let Ok(text) = serde_json::to_string_pretty(&history) {
            let tmp = target_dir.join(".astrid_influence_response_history_v3.json.tmp");
            if std::fs::write(&tmp, &text).is_ok() {
                let _ = std::fs::rename(&tmp, &history_path);
            }
        }
        // Best-effort cleanup of consumed file so the next cycle starts
        // fresh; safe to ignore failure.
        let _ = std::fs::remove_file(&consumed_path);
        tracing::info!(
            "closing-loop response intent_id={} delta_field_norm={:.4} class={}→{}",
            in_flight.intent_id,
            delta,
            in_flight.pre_snapshot.class_primary,
            post.class_primary,
        );
    }
}

fn top_k_pca(history: &VecDeque<Vec<f32>>, k: usize) -> Option<Vec<Vec<f32>>> {
    let n = history.len();
    if n < 2 {
        return None;
    }
    let d = CODEC_DIM;
    let mut mean_vec = vec![0.0_f32; d];
    for v in history.iter() {
        for (m, &val) in mean_vec.iter_mut().zip(v.iter()) {
            *m += val;
        }
    }
    let inv_n = 1.0 / n as f32;
    for m in &mut mean_vec {
        *m *= inv_n;
    }

    // Build covariance matrix d×d
    let mut cov = vec![0.0_f32; d * d];
    for v in history.iter() {
        for i in 0..d {
            let ci = v.get(i).copied().unwrap_or(0.0) - mean_vec[i];
            for j in i..d {
                let cj = v.get(j).copied().unwrap_or(0.0) - mean_vec[j];
                let val = ci * cj;
                cov[i * d + j] += val;
                if i != j {
                    cov[j * d + i] += val;
                }
            }
        }
    }
    let inv_n1 = 1.0 / (n as f32 - 1.0).max(1.0);
    for c in &mut cov {
        *c *= inv_n1;
    }

    // Repeated power iteration with deflation for top-k modes.
    let mut modes: Vec<Vec<f32>> = Vec::with_capacity(k);
    let mut working = cov.clone();
    for mode_idx in 0..k {
        let mut vec_pc = vec![0.0_f32; d];
        for (i, val) in vec_pc.iter_mut().enumerate() {
            *val = (((i + mode_idx + 1) as f32) * 0.31415).sin();
        }
        for _ in 0..50 {
            let mut next = vec![0.0_f32; d];
            for i in 0..d {
                let mut sum = 0.0_f32;
                for j in 0..d {
                    sum += working[i * d + j] * vec_pc[j];
                }
                next[i] = sum;
            }
            let norm = l2_norm(&next).max(1e-9);
            for v in &mut next {
                *v /= norm;
            }
            vec_pc = next;
        }
        // Deflate working matrix: subtract λ * v * v^T where λ = v^T·M·v
        let mut mv = vec![0.0_f32; d];
        for i in 0..d {
            for j in 0..d {
                mv[i] += working[i * d + j] * vec_pc[j];
            }
        }
        let lambda: f32 = vec_pc.iter().zip(mv.iter()).map(|(a, b)| a * b).sum();
        for i in 0..d {
            for j in 0..d {
                working[i * d + j] -= lambda * vec_pc[i] * vec_pc[j];
            }
        }
        modes.push(vec_pc);
    }
    Some(modes)
}

fn project_onto_modes(vec_in: &[f32], modes: &[Vec<f32>]) -> Vec<f32> {
    modes
        .iter()
        .map(|mode| {
            let dot: f32 = vec_in.iter().zip(mode.iter()).map(|(a, b)| a * b).sum();
            dot.clamp(-4.0, 4.0).tanh()
        })
        .collect()
}

fn covariance_off_diag(projections: &[Vec<f32>]) -> Vec<f32> {
    let n = projections.len();
    if n < 2 {
        return vec![0.0; MODE_DIM * MODE_DIM];
    }
    let mut mean_vec = vec![0.0_f32; MODE_DIM];
    for v in projections {
        for (m, &val) in mean_vec.iter_mut().zip(v.iter()) {
            *m += val;
        }
    }
    let inv_n = 1.0 / n as f32;
    for m in &mut mean_vec {
        *m *= inv_n;
    }
    let mut cov = vec![0.0_f32; MODE_DIM * MODE_DIM];
    for v in projections {
        for i in 0..MODE_DIM {
            let ci = v.get(i).copied().unwrap_or(0.0) - mean_vec[i];
            for j in 0..MODE_DIM {
                let cj = v.get(j).copied().unwrap_or(0.0) - mean_vec[j];
                cov[i * MODE_DIM + j] += ci * cj;
            }
        }
    }
    let inv_n1 = 1.0 / (n as f32 - 1.0).max(1.0);
    for (idx, c) in cov.iter_mut().enumerate() {
        *c *= inv_n1;
        // Zero the diagonal so coupling_stats matches minime's semantics
        if idx % (MODE_DIM + 1) == 0 {
            *c = 0.0;
        }
    }
    cov
}

fn update_emas(fast: &mut Vec<f32>, medium: &mut Vec<f32>, slow: &mut Vec<f32>, reduced: &[f32]) {
    if fast.len() != reduced.len() {
        *fast = vec![0.0; reduced.len()];
    }
    if medium.len() != reduced.len() {
        *medium = vec![0.0; reduced.len()];
    }
    if slow.len() != reduced.len() {
        *slow = vec![0.0; reduced.len()];
    }
    for ((((f, m), s), r), _) in fast
        .iter_mut()
        .zip(medium.iter_mut())
        .zip(slow.iter_mut())
        .zip(reduced.iter().copied())
        .zip(0..reduced.len())
    {
        // Match minime's time constants: damping=0.35 fast, 0.18 medium, 0.03 slow
        *f = ((1.0 - 0.35) * *f + 0.35 * r).clamp(-1.0, 1.0);
        *m = (0.82 * *m + 0.18 * r).clamp(-1.0, 1.0);
        *s = (0.97 * *s + 0.03 * r).clamp(-1.0, 1.0);
    }
}

fn derive_traits(
    field_norm: f32,
    binary_flip_rate: f32,
    fissure_tendency: f32,
    lock_tendency: f32,
    coupling_active_fraction: f32,
    fast_magnetization: f32,
) -> Vec<&'static str> {
    let mut traits: Vec<&'static str> = Vec::new();
    if field_norm < QUIET_THRESHOLD {
        traits.push("quiet");
    }
    if binary_flip_rate >= 0.20 || fissure_tendency >= 0.55 {
        traits.push("volatile");
    }
    if lock_tendency >= 0.65 {
        traits.push("sticky");
    }
    if coupling_active_fraction >= 0.25 {
        traits.push("coupled");
    }
    if fast_magnetization.abs() >= 0.20 {
        traits.push("polarized");
    }
    if traits.is_empty() {
        traits.push("active");
    }
    traits
}

fn primary_from_traits(traits: &[&'static str]) -> &'static str {
    for candidate in [
        "quiet",
        "volatile",
        "sticky",
        "coupled",
        "polarized",
        "active",
    ] {
        if traits.contains(&candidate) {
            return candidate;
        }
    }
    "active"
}

/// v3.5: Per-mode ranked partner list. Returns the top-k modes most
/// strongly coupled to `mode`, ordered by `|J_ij|` descending. The matrix
/// is row-major `dim × dim`; the diagonal entry (self-coupling) is
/// excluded. Mirror of `ising_shadow::coupling_partners_ranked`.
pub(crate) fn coupling_partners_ranked(
    coupling: &[f32],
    mode: usize,
    dim: usize,
    top_k: usize,
) -> Vec<(usize, f32)> {
    if dim == 0 || mode >= dim || top_k == 0 {
        return Vec::new();
    }
    let row_start = mode * dim;
    let mut partners: Vec<(usize, f32)> = (0..dim)
        .filter(|&j| j != mode)
        .map(|j| {
            let value = coupling
                .get(row_start + j)
                .copied()
                .unwrap_or_default()
                .abs();
            (j, value)
        })
        .collect();
    partners.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    partners.truncate(top_k);
    partners
}

/// v3.5: Astrid is "in a relatively quiet window for her own substrate" if
/// her current `field_norm` is below the 30th-percentile threshold over her
/// recent snapshot history. Returns false until at least 8 snapshots have
/// accumulated — early-history readings would otherwise admit any value.
///
/// This adapts the eligibility gate to whichever substrate the same code is
/// running on. Minime's covariance shadow lives at field_norm 0.05–0.20;
/// Astrid's PCA over codec features lives at 0.77–0.85. The same absolute
/// upper bound (0.65) keeps Astrid permanently CLOSED. The percentile-
/// relative reading says "this snapshot is in your own lower 30%" instead.
fn percentile_eligible(current: f32, history: &VecDeque<ShadowSnapshotV3>) -> bool {
    if history.len() < 8 {
        return false;
    }
    let mut norms: Vec<f32> = history.iter().map(|s| s.field_norm).collect();
    norms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p30_index = (norms.len() * 30 / 100).max(1);
    let threshold = norms[p30_index];
    current < threshold
}

fn compute_phase_dwell(history: &VecDeque<ShadowSnapshotV3>, current: &str) -> u32 {
    let mut dwell = 0_u32;
    for snap in history.iter().rev() {
        if snap.class_primary == current {
            dwell = dwell.saturating_add(1);
        } else {
            break;
        }
    }
    dwell
}

fn collect_recent_transitions(
    history: &VecDeque<ShadowSnapshotV3>,
) -> Vec<ShadowPhaseTransitionV3> {
    let mut transitions: Vec<ShadowPhaseTransitionV3> = Vec::new();
    let snaps: Vec<&ShadowSnapshotV3> = history.iter().collect();
    for window in snaps.windows(2) {
        let prev = window[0];
        let curr = window[1];
        if prev.class_primary != curr.class_primary {
            transitions.push(ShadowPhaseTransitionV3 {
                from: prev.class_primary.clone(),
                to: curr.class_primary.clone(),
                at_t_ms: curr.t_ms,
            });
        }
    }
    if transitions.len() > TRANSITIONS_CAP {
        let drop = transitions.len() - TRANSITIONS_CAP;
        transitions.drain(..drop);
    }
    transitions
}

fn coupling_stats(coupling: &[f32], dim: usize) -> (f32, f32, f32) {
    if dim == 0 {
        return (0.0, 0.0, 0.0);
    }
    let mut sum_abs = 0.0_f32;
    let mut max_abs = 0.0_f32;
    let mut active = 0_usize;
    let mut total = 0_usize;
    for i in 0..dim {
        for j in 0..dim {
            if i == j {
                continue;
            }
            let value = coupling.get(i * dim + j).copied().unwrap_or(0.0).abs();
            sum_abs += value;
            max_abs = max_abs.max(value);
            if value > 1.0e-5 {
                active += 1;
            }
            total += 1;
        }
    }
    if total == 0 {
        (0.0, 0.0, 0.0)
    } else {
        (
            sum_abs / total as f32,
            max_abs,
            active as f32 / total as f32,
        )
    }
}

fn cosine01(a: &[f32], b: &[f32]) -> f32 {
    let norm = l2_norm(a) * l2_norm(b);
    if norm <= 1.0e-6 {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    ((dot / norm).clamp(-1.0, 1.0) + 1.0) * 0.5
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|v| v * v).sum::<f32>().sqrt()
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

/// Convenience: full one-shot pipeline. Used at exchange completion sites.
pub fn observe_and_publish(
    computer: &mut AstridShadowComputer,
    codec_features: &[f32],
    target_dir: &Path,
) -> Option<Value> {
    let value = computer.observe(codec_features)?;
    computer.publish(Some(&value), target_dir);
    // v3.5: handle reciprocal-influence closed loop. Polls minime's
    // workspace for an active or consumed influence file and writes the
    // response back to minime's workspace when the window completes.
    computer.track_minime_influence(target_dir);
    Some(value)
}

/// Default publish target — minime's workspace, where minime's reader can
/// see it the same way Astrid sees minime's `health.json`.
pub fn default_publish_dir() -> PathBuf {
    crate::paths::bridge_paths()
        .minime_workspace()
        .to_path_buf()
}

#[cfg(test)]
mod percentile_tests {
    use super::*;

    fn snap(field_norm: f32) -> ShadowSnapshotV3 {
        ShadowSnapshotV3 {
            t_ms: 0,
            field_norm,
            class_primary: "active".to_string(),
            traits: vec![],
            recurrence: 0.0,
            mode_tension: 0.0,
            binary_flip_rate: 0.0,
            lock_tendency: 0.0,
            fissure_tendency: 0.0,
            tail_openness: 0.0,
            coupling_mean_abs: 0.0,
            influence_eligible: false,
        }
    }

    #[test]
    fn percentile_eligible_with_few_snapshots_returns_false() {
        let mut history: VecDeque<ShadowSnapshotV3> = VecDeque::new();
        for i in 0..5 {
            history.push_back(snap(0.1 * (i as f32 + 1.0)));
        }
        // history.len() < 8 → always false
        assert!(!percentile_eligible(0.05, &history));
        assert!(!percentile_eligible(0.50, &history));
    }

    #[test]
    fn percentile_eligible_at_p15_returns_true() {
        let mut history: VecDeque<ShadowSnapshotV3> = VecDeque::new();
        // Build a 20-snapshot history with field_norm 0.10..0.29 in 0.01 steps.
        for i in 0..20 {
            history.push_back(snap(0.10 + (i as f32) * 0.01));
        }
        // p30 index = 20 * 30 / 100 = 6 → threshold = 0.16
        // Current well below threshold (e.g. 0.11) → true
        assert!(percentile_eligible(0.11, &history));
    }

    #[test]
    fn percentile_eligible_at_p70_returns_false() {
        let mut history: VecDeque<ShadowSnapshotV3> = VecDeque::new();
        for i in 0..20 {
            history.push_back(snap(0.10 + (i as f32) * 0.01));
        }
        // Current near top of distribution → not in lower 30%
        assert!(!percentile_eligible(0.27, &history));
    }

    #[test]
    fn percentile_eligible_handles_uniform_history() {
        let mut history: VecDeque<ShadowSnapshotV3> = VecDeque::new();
        for _ in 0..10 {
            history.push_back(snap(0.5));
        }
        // All values identical — nothing is "below" the p30 of itself.
        // Sorted: all 0.5; threshold = 0.5; current 0.5 is not < 0.5.
        assert!(!percentile_eligible(0.5, &history));
        // A genuinely lower value would qualify.
        assert!(percentile_eligible(0.4, &history));
    }
}
