#![allow(clippy::arithmetic_side_effects)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SPECTRAL_FINGERPRINT_POLICY: &str = "spectral_fingerprint_v1";
pub const SPECTRAL_FINGERPRINT_SCHEMA_VERSION: u8 = 1;
pub const LEGACY_FINGERPRINT_LEN: usize = 32;
pub const SPECTRAL_DENOMINATOR_POLICY: &str = "spectral_denominator_v1";
pub const SPECTRAL_DENOMINATOR_SCHEMA_VERSION: u8 = 1;
pub const SEMANTIC_ENERGY_POLICY: &str = "semantic_energy_v1";
pub const SEMANTIC_ENERGY_SCHEMA_VERSION: u8 = 1;
pub const TRANSITION_EVENT_POLICY: &str = "transition_event_v1";
pub const EIGENVECTOR_FIELD_POLICY: &str = "eigenvector_field_v1";

pub const LEGACY_SLOT_LABELS: [&str; LEGACY_FINGERPRINT_LEN] = [
    "lambda1",
    "lambda2",
    "lambda3",
    "lambda4",
    "lambda5",
    "lambda6",
    "lambda7",
    "lambda8",
    "eigenvector_concentration_top4_1",
    "eigenvector_concentration_top4_2",
    "eigenvector_concentration_top4_3",
    "eigenvector_concentration_top4_4",
    "eigenvector_concentration_top4_5",
    "eigenvector_concentration_top4_6",
    "eigenvector_concentration_top4_7",
    "eigenvector_concentration_top4_8",
    "inter_mode_cosine_top_abs_1",
    "inter_mode_cosine_top_abs_2",
    "inter_mode_cosine_top_abs_3",
    "inter_mode_cosine_top_abs_4",
    "inter_mode_cosine_top_abs_5",
    "inter_mode_cosine_top_abs_6",
    "inter_mode_cosine_top_abs_7",
    "inter_mode_cosine_top_abs_8",
    "spectral_entropy",
    "lambda1_lambda2_gap",
    "v1_rotation_similarity",
    "geom_rel",
    "lambda1_lambda2_adjacent_gap",
    "lambda2_lambda3_adjacent_gap",
    "lambda3_lambda4_adjacent_gap",
    "lambda4_lambda5_adjacent_gap",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpectralFingerprintV1 {
    pub policy: String,
    pub schema_version: u8,
    pub eigenvalues: [f32; 8],
    pub eigenvector_concentration_top4: [f32; 8],
    pub inter_mode_cosine_top_abs: [f32; 8],
    pub spectral_entropy: f32,
    pub lambda1_lambda2_gap: f32,
    pub v1_rotation_similarity: f32,
    pub v1_rotation_delta: f32,
    pub geom_rel: f32,
    pub adjacent_gap_ratios: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpectralDenominatorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub effective_dimensionality: f32,
    pub active_mode_capacity: usize,
    pub distinguishability_loss: f32,
    #[serde(default)]
    pub lambda1_energy_share: f32,
    #[serde(default)]
    pub spectral_entropy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticEnergyV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default)]
    pub input_energy: f32,
    #[serde(default)]
    pub input_active: bool,
    #[serde(default)]
    pub input_fresh_ms: Option<u64>,
    #[serde(default)]
    pub input_stale_ms: Option<u64>,
    #[serde(default)]
    pub kernel_energy: f32,
    #[serde(default)]
    pub kernel_delta: f32,
    #[serde(default)]
    pub kernel_active: bool,
    #[serde(default)]
    pub regulator_drive_energy: f32,
    #[serde(default)]
    pub admission: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionEventV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default)]
    pub sequence: u64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub legacy_kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub engine_t_s: f64,
    #[serde(default)]
    pub tick_count: u64,
    #[serde(default)]
    pub phase_from: String,
    #[serde(default)]
    pub phase_to: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub fill_band_from: String,
    #[serde(default)]
    pub fill_band_to: String,
    #[serde(default)]
    pub fill_band: String,
    #[serde(default)]
    pub fill_pct: f32,
    #[serde(default)]
    pub target_fill_pct: f32,
    #[serde(default)]
    pub lambda1: f32,
    #[serde(default)]
    pub lambda1_rel: f32,
    #[serde(default)]
    pub target_lambda1_rel: f32,
    #[serde(default)]
    pub lambda_stress: f32,
    #[serde(default)]
    pub geom_rel: f32,
    #[serde(default)]
    pub dfill_dt: f32,
    #[serde(default)]
    pub spectral_entropy: f32,
    #[serde(default)]
    pub structural_entropy: Option<f32>,
    #[serde(default)]
    pub glimpse_distance: Option<f32>,
    #[serde(default)]
    pub rotation_delta: Option<f32>,
    #[serde(default)]
    pub basin_shift_score: f32,
    #[serde(default)]
    pub basin_shift: bool,
    #[serde(default)]
    pub breathing_phase: bool,
    #[serde(default)]
    pub crossed_target_fill: bool,
    #[serde(default)]
    pub crossed_fill_band: bool,
    #[serde(default)]
    pub spectral_spike: bool,
    #[serde(default)]
    pub stable_core_stage: Option<String>,
    #[serde(default)]
    pub stable_core_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EigenvectorFieldV1 {
    pub policy: String,
    #[serde(default)]
    pub direct_eigenvectors_available: bool,
    #[serde(default)]
    pub raw_vectors_exported: bool,
    #[serde(default)]
    pub export_note: String,
    #[serde(default)]
    pub reservoir_dim: usize,
    #[serde(default)]
    pub mode_count: usize,
    #[serde(default)]
    pub component_limit: usize,
    #[serde(default)]
    pub modes: Vec<EigenvectorModeV1>,
    #[serde(default)]
    pub pairwise_overlaps: Vec<EigenvectorPairOverlapV1>,
    #[serde(default)]
    pub summary: EigenvectorFieldSummaryV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EigenvectorModeV1 {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub eigenvalue: f32,
    #[serde(default)]
    pub energy_share: f32,
    #[serde(default)]
    pub norm: f32,
    #[serde(default)]
    pub concentration_top4: f32,
    #[serde(default)]
    pub top_components: Vec<EigenvectorComponentV1>,
    #[serde(default)]
    pub overlap_with_previous: Option<f32>,
    #[serde(default)]
    pub orientation_delta: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EigenvectorComponentV1 {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub value: f32,
    #[serde(default)]
    pub abs: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EigenvectorPairOverlapV1 {
    #[serde(default)]
    pub left: usize,
    #[serde(default)]
    pub right: usize,
    #[serde(default)]
    pub cosine: f32,
    #[serde(default)]
    pub abs_cosine: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EigenvectorFieldSummaryV1 {
    #[serde(default)]
    pub mean_orientation_delta: f32,
    #[serde(default)]
    pub max_pairwise_overlap: f32,
    #[serde(default)]
    pub previous_overlap_available: bool,
}

impl SemanticEnergyV1 {
    #[must_use]
    pub fn from_typed_value(value: &Value) -> Option<Self> {
        let parsed = serde_json::from_value::<Self>(value.clone()).ok()?;
        (parsed.policy == SEMANTIC_ENERGY_POLICY).then_some(parsed)
    }

    #[must_use]
    pub fn from_legacy_semantic(value: &Value) -> Option<Self> {
        if !value.is_object() {
            return None;
        }
        let input_energy = f32_value(value, "input_energy")
            .or_else(|| f32_value(value, "energy"))
            .unwrap_or(0.0);
        let kernel_energy = f32_value(value, "kernel_energy")
            .or_else(|| f32_value(value, "energy"))
            .unwrap_or(0.0);
        let kernel_delta = f32_value(value, "kernel_delta")
            .or_else(|| f32_value(value, "delta"))
            .unwrap_or(0.0);
        let regulator_drive_energy =
            f32_value(value, "regulator_drive_energy").unwrap_or(kernel_energy);
        Some(Self {
            policy: SEMANTIC_ENERGY_POLICY.to_string(),
            schema_version: SEMANTIC_ENERGY_SCHEMA_VERSION,
            input_energy,
            input_active: bool_value(value, "input_active").unwrap_or(input_energy > f32::EPSILON),
            input_fresh_ms: u64_value(value, "input_fresh_ms"),
            input_stale_ms: u64_value(value, "input_stale_ms")
                .or_else(|| u64_value(value, "last_update_age_ms")),
            kernel_energy,
            kernel_delta,
            kernel_active: bool_value(value, "kernel_active")
                .or_else(|| bool_value(value, "active"))
                .unwrap_or(kernel_energy > f32::EPSILON),
            regulator_drive_energy,
            admission: str_value(value, "admission")
                .unwrap_or("legacy_semantic")
                .to_string(),
        })
    }
}

impl TransitionEventV1 {
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        let parsed = serde_json::from_value::<Self>(value.clone()).ok()?;
        (parsed.policy == TRANSITION_EVENT_POLICY).then_some(parsed)
    }
}

impl EigenvectorFieldV1 {
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        let parsed = serde_json::from_value::<Self>(value.clone()).ok()?;
        (parsed.policy == EIGENVECTOR_FIELD_POLICY).then_some(parsed)
    }
}

impl SpectralDenominatorV1 {
    #[must_use]
    pub fn from_eigenvalues(values: &[f32], spectral_entropy: Option<f32>) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let active_mode_capacity = values
            .iter()
            .filter(|value| finite(**value).abs() > 1.0e-6)
            .count()
            .max(1);
        let effective_dimensionality = effective_dimensionality(values);
        let normalized = effective_dimensionality / active_mode_capacity as f32;
        let distinguishability_loss = (1.0 - normalized).clamp(0.0, 1.0);
        let total = values.iter().map(|value| finite(*value).abs()).sum::<f32>();
        let lambda1_energy_share = if total > 1.0e-6 {
            values.first().copied().map_or(0.0, finite).abs() / total
        } else {
            0.0
        };

        Some(Self {
            policy: SPECTRAL_DENOMINATOR_POLICY.to_string(),
            schema_version: SPECTRAL_DENOMINATOR_SCHEMA_VERSION,
            effective_dimensionality,
            active_mode_capacity,
            distinguishability_loss,
            lambda1_energy_share,
            spectral_entropy: spectral_entropy
                .map(finite)
                .unwrap_or_else(|| spectral_entropy_from_values(values)),
        })
    }
}

impl SpectralFingerprintV1 {
    #[must_use]
    pub fn from_legacy_slots(slots: &[f32]) -> Option<Self> {
        if slots.len() < LEGACY_FINGERPRINT_LEN {
            return None;
        }

        let eigenvalues = array_from_slice::<8>(&slots[0..8]);
        let eigenvector_concentration_top4 = array_from_slice::<8>(&slots[8..16]);
        let inter_mode_cosine_top_abs = array_from_slice::<8>(&slots[16..24]);
        let spectral_entropy = finite(slots[24]);
        let lambda1_lambda2_gap = finite(slots[25]);
        let v1_rotation_similarity = finite(slots[26]);
        let geom_rel = finite(slots[27]);
        let adjacent_gap_ratios = array_from_slice::<4>(&slots[28..32]);

        Some(Self {
            policy: SPECTRAL_FINGERPRINT_POLICY.to_string(),
            schema_version: SPECTRAL_FINGERPRINT_SCHEMA_VERSION,
            eigenvalues,
            eigenvector_concentration_top4,
            inter_mode_cosine_top_abs,
            spectral_entropy,
            lambda1_lambda2_gap,
            v1_rotation_similarity,
            v1_rotation_delta: (1.0 - v1_rotation_similarity).clamp(0.0, 2.0),
            geom_rel,
            adjacent_gap_ratios,
        })
    }

    #[must_use]
    pub fn from_telemetry(telemetry: &crate::types::SpectralTelemetry) -> Option<Self> {
        telemetry.spectral_fingerprint_v1.clone().or_else(|| {
            telemetry
                .spectral_fingerprint
                .as_deref()
                .and_then(Self::from_legacy_slots)
        })
    }

    #[must_use]
    pub fn to_legacy_slots(&self) -> Vec<f32> {
        let mut slots = Vec::with_capacity(LEGACY_FINGERPRINT_LEN);
        slots.extend_from_slice(&self.eigenvalues);
        slots.extend_from_slice(&self.eigenvector_concentration_top4);
        slots.extend_from_slice(&self.inter_mode_cosine_top_abs);
        slots.push(self.spectral_entropy);
        slots.push(self.lambda1_lambda2_gap);
        slots.push(self.v1_rotation_similarity);
        slots.push(self.geom_rel);
        slots.extend_from_slice(&self.adjacent_gap_ratios);
        slots
    }

    #[must_use]
    pub fn live_glimpse_12d(&self) -> Vec<f32> {
        let total_ev = self
            .eigenvalues
            .iter()
            .map(|value| value.abs())
            .sum::<f32>();
        let concentration_mean = mean(&self.eigenvector_concentration_top4);
        let couplings = self
            .inter_mode_cosine_top_abs
            .map(|value| finite(value).abs());
        let coupling_mean = mean(&couplings);
        let gap_mean = mean(&self.adjacent_gap_ratios);

        vec![
            abs_share(self.eigenvalues[0], total_ev),
            abs_share(self.eigenvalues[1], total_ev) + abs_share(self.eigenvalues[2], total_ev),
            self.eigenvalues[3..]
                .iter()
                .map(|value| abs_share(*value, total_ev))
                .sum::<f32>(),
            self.eigenvector_concentration_top4
                .iter()
                .copied()
                .map(finite)
                .fold(0.0, f32::max),
            stddev(&self.eigenvector_concentration_top4, concentration_mean),
            couplings.iter().copied().map(finite).fold(0.0, f32::max),
            coupling_mean,
            self.spectral_entropy.clamp(0.0, 1.0),
            self.lambda1_lambda2_gap.max(0.0),
            self.v1_rotation_delta.clamp(0.0, 2.0),
            self.geom_rel,
            gap_mean,
        ]
    }

    #[must_use]
    pub fn energy_shares(&self) -> (f32, f32, f32) {
        let total = self
            .eigenvalues
            .iter()
            .map(|value| value.abs())
            .sum::<f32>();
        if total <= f32::EPSILON {
            return (0.0, 0.0, 0.0);
        }
        let head = self.eigenvalues.first().copied().unwrap_or(0.0).abs() / total;
        let shoulder = self
            .eigenvalues
            .iter()
            .skip(1)
            .take(2)
            .map(|value| value.abs() / total)
            .sum::<f32>();
        let tail = self
            .eigenvalues
            .iter()
            .skip(3)
            .map(|value| value.abs() / total)
            .sum::<f32>();
        (head, shoulder, tail)
    }

    #[must_use]
    pub fn effective_dimensionality(&self) -> f32 {
        effective_dimensionality(&self.eigenvalues)
    }

    #[must_use]
    pub fn denominator_metrics(&self) -> SpectralDenominatorV1 {
        let active_mode_capacity = self
            .eigenvalues
            .iter()
            .filter(|value| finite(**value).abs() > 1.0e-6)
            .count()
            .max(1);
        let effective_dimensionality = self.effective_dimensionality();
        let normalized = effective_dimensionality / active_mode_capacity as f32;
        let distinguishability_loss = (1.0 - normalized).clamp(0.0, 1.0);
        let (lambda1_energy_share, _, _) = self.energy_shares();

        SpectralDenominatorV1 {
            policy: SPECTRAL_DENOMINATOR_POLICY.to_string(),
            schema_version: SPECTRAL_DENOMINATOR_SCHEMA_VERSION,
            effective_dimensionality,
            active_mode_capacity,
            distinguishability_loss,
            lambda1_energy_share,
            spectral_entropy: self.spectral_entropy,
        }
    }
}

#[must_use]
pub fn legacy_slot_label(index: usize) -> &'static str {
    LEGACY_SLOT_LABELS.get(index).copied().unwrap_or("unknown")
}

#[must_use]
pub fn format_legacy_slots(slots: &[f32]) -> String {
    let mut output =
        String::from("Spectral Fingerprint (32D raw, schema spectral_fingerprint_v1):\n");
    for (index, value) in slots.iter().enumerate() {
        let label = legacy_slot_label(index);
        output.push_str(&format!("  [{index:2}] {label:<36} {value:+.4}\n"));
    }
    output
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn array_from_slice<const N: usize>(values: &[f32]) -> [f32; N] {
    std::array::from_fn(|index| values.get(index).copied().map_or(0.0, finite))
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().copied().map(finite).sum::<f32>() / values.len() as f32
    }
}

fn stddev(values: &[f32], center: f32) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        let variance = values
            .iter()
            .copied()
            .map(finite)
            .map(|value| {
                let diff = value - center;
                diff * diff
            })
            .sum::<f32>()
            / values.len() as f32;
        variance.sqrt()
    }
}

fn abs_share(value: f32, total: f32) -> f32 {
    if total > 1.0e-6 {
        value.abs() / total
    } else {
        0.0
    }
}

fn f32_value(value: &Value, key: &str) -> Option<f32> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| finite(value as f32))
}

fn u64_value(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn bool_value(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn str_value<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn effective_dimensionality(values: &[f32]) -> f32 {
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    for value in values.iter().copied().map(finite).map(f32::abs) {
        sum += value;
        sum_sq += value * value;
    }
    if sum_sq > 1.0e-12 {
        (sum * sum / sum_sq).max(0.0)
    } else {
        0.0
    }
}

fn spectral_entropy_from_values(values: &[f32]) -> f32 {
    let total = values.iter().map(|value| finite(*value).abs()).sum::<f32>();
    if total <= 1.0e-10 {
        return 0.0;
    }
    let entropy = values
        .iter()
        .map(|value| {
            let p = finite(*value).abs() / total;
            if p > 1.0e-10 { -p * p.ln() } else { 0.0 }
        })
        .sum::<f32>();
    let max_entropy = (values.len() as f32).ln();
    if max_entropy > 0.0 && entropy.is_finite() {
        (entropy / max_entropy).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_slots_map_to_named_schema() {
        let slots = (0..LEGACY_FINGERPRINT_LEN)
            .map(|value| value as f32)
            .collect::<Vec<_>>();

        let typed = SpectralFingerprintV1::from_legacy_slots(&slots).unwrap();

        assert_eq!(typed.policy, SPECTRAL_FINGERPRINT_POLICY);
        assert_eq!(typed.schema_version, SPECTRAL_FINGERPRINT_SCHEMA_VERSION);
        assert_eq!(typed.eigenvalues[7], 7.0);
        assert_eq!(typed.eigenvector_concentration_top4[0], 8.0);
        assert_eq!(typed.inter_mode_cosine_top_abs[7], 23.0);
        assert_eq!(typed.spectral_entropy, 24.0);
        assert_eq!(typed.lambda1_lambda2_gap, 25.0);
        assert_eq!(typed.v1_rotation_similarity, 26.0);
        assert_eq!(typed.geom_rel, 27.0);
        assert_eq!(typed.adjacent_gap_ratios, [28.0, 29.0, 30.0, 31.0]);
        assert_eq!(typed.to_legacy_slots(), slots);
    }

    #[test]
    fn denominator_metrics_capture_effective_dimensionality() {
        let typed = SpectralFingerprintV1 {
            policy: SPECTRAL_FINGERPRINT_POLICY.to_string(),
            schema_version: SPECTRAL_FINGERPRINT_SCHEMA_VERSION,
            eigenvalues: [4.0, 3.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.8,
            lambda1_lambda2_gap: 1.33,
            v1_rotation_similarity: 1.0,
            v1_rotation_delta: 0.0,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.33, 3.0, 1.0, 1.0],
        };

        let metrics = typed.denominator_metrics();

        assert_eq!(metrics.policy, SPECTRAL_DENOMINATOR_POLICY);
        assert_eq!(metrics.active_mode_capacity, 5);
        assert!((metrics.effective_dimensionality - (100.0 / 28.0)).abs() < 1.0e-6);
        assert!(metrics.distinguishability_loss > 0.0);
        assert!(metrics.distinguishability_loss < 1.0);
    }

    #[test]
    fn slot_labels_do_not_reuse_stale_fill_or_shadow_meanings() {
        assert_eq!(legacy_slot_label(25), "lambda1_lambda2_gap");
        assert_eq!(legacy_slot_label(28), "lambda1_lambda2_adjacent_gap");
        assert!(!LEGACY_SLOT_LABELS.contains(&"fill"));
        assert!(!LEGACY_SLOT_LABELS.contains(&"shadow_e"));
    }

    #[test]
    fn semantic_energy_reconstructs_from_legacy_object() {
        let legacy = serde_json::json!({
            "energy": 0.0,
            "delta": 0.0,
            "input_energy": 0.12,
            "input_active": true,
            "kernel_energy": 0.0,
            "kernel_active": false,
            "admission": "stable_core_kernel_zeroed"
        });

        let typed = SemanticEnergyV1::from_legacy_semantic(&legacy).unwrap();

        assert_eq!(typed.policy, SEMANTIC_ENERGY_POLICY);
        assert_eq!(typed.input_energy, 0.12);
        assert_eq!(typed.kernel_energy, 0.0);
        assert_eq!(typed.regulator_drive_energy, 0.0);
        assert_eq!(typed.admission, "stable_core_kernel_zeroed");
    }

    #[test]
    fn transition_and_eigenvector_views_parse_typed_payloads() {
        let transition = serde_json::json!({
            "policy": "transition_event_v1",
            "schema_version": 1,
            "kind": "basin_transition",
            "description": "basin shift candidate",
            "basin_shift_score": 0.72,
            "lambda1_rel": 1.03,
            "geom_rel": 0.98
        });
        let field = serde_json::json!({
            "policy": "eigenvector_field_v1",
            "mode_count": 2,
            "reservoir_dim": 512,
            "summary": {
                "mean_orientation_delta": 0.12,
                "max_pairwise_overlap": 0.08
            },
            "modes": [{
                "index": 1,
                "eigenvalue": 4.2,
                "top_components": [{"index": 7, "value": -0.5, "abs": 0.5}]
            }]
        });

        let transition = TransitionEventV1::from_value(&transition).unwrap();
        let field = EigenvectorFieldV1::from_value(&field).unwrap();

        assert_eq!(transition.kind, "basin_transition");
        assert_eq!(field.mode_count, 2);
        assert_eq!(field.modes[0].top_components[0].index, 7);
    }
}
