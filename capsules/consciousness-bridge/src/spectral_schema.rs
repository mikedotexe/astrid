#![allow(clippy::arithmetic_side_effects)]

use serde::{Deserialize, Serialize};

pub const SPECTRAL_FINGERPRINT_POLICY: &str = "spectral_fingerprint_v1";
pub const SPECTRAL_FINGERPRINT_SCHEMA_VERSION: u8 = 1;
pub const LEGACY_FINGERPRINT_LEN: usize = 32;

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
    fn slot_labels_do_not_reuse_stale_fill_or_shadow_meanings() {
        assert_eq!(legacy_slot_label(25), "lambda1_lambda2_gap");
        assert_eq!(legacy_slot_label(28), "lambda1_lambda2_adjacent_gap");
        assert!(!LEGACY_SLOT_LABELS.contains(&"fill"));
        assert!(!LEGACY_SLOT_LABELS.contains(&"shadow_e"));
    }
}
