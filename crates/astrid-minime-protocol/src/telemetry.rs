use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CompatibilityStatus, ProtocolHeaderV1, classify_protocol, telemetry_protocol};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NeuralOutputs {
    pub pred_lambda1: f32,
    pub router_weights: Vec<f32>,
    pub control: Vec<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModalityStatus {
    pub audio_fired: bool,
    pub video_fired: bool,
    pub history_fired: bool,
    pub audio_rms: f32,
    pub video_var: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_age_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_freshness_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_freshness_class: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EsnLeakOverrideStatus {
    pub leak: f32,
    pub remaining_ticks: u32,
    pub request_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Exact read-only review wire schema.
pub struct SpectralDampingWarmStartReviewV1 {
    pub policy: String,
    pub cheby_order: usize,
    pub cheby_stop_lo: f32,
    pub cheby_stop_hi: f32,
    pub cheby_soft: f32,
    pub proposed_cheby_stop_lo: f32,
    pub proposed_cheby_soft: f32,
    pub warm_start_blend: f32,
    pub proposed_warm_start_blend: f32,
    pub eigenfill_pct: f32,
    pub eigenfill_target_pct: f32,
    pub distinguishability_loss: f32,
    pub coefficient_l1_norm: f32,
    pub proposed_coefficient_l1_norm: f32,
    pub regulator_drive_energy: f32,
    pub regulator_counteraction_score: f32,
    pub regulator_constriction_state: String,
    pub near_target_band: bool,
    pub live_control_required: bool,
    pub runnable_without_approval: bool,
    pub status: String,
    pub approval_boundary: String,
    pub authority: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EigenPacketPayloadBudgetReviewV1 {
    pub policy: String,
    pub eigenvalues_len: usize,
    pub spectral_fingerprint_len: usize,
    pub eigenvector_mode_count: usize,
    pub eigenvector_top_component_count: usize,
    pub eigenvector_pairwise_overlap_count: usize,
    pub estimated_eigenvector_scalar_count: usize,
    pub estimated_total_float_count: usize,
    pub estimated_eigenvector_json_bytes: usize,
    pub budget_state: String,
    pub status: String,
    pub authority: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Exact read-only review wire schema.
pub struct HardResetTexturePreservationReviewV1 {
    pub policy: String,
    pub eigenfill_pct: f32,
    pub spectral_entropy: f32,
    pub mode_packing: f32,
    pub pressure_risk: f32,
    pub texture_gradient_proxy: f32,
    pub recovery_fill_boost: f32,
    pub recovery_keep_ceiling: f32,
    pub recovery_activation_gain: f32,
    pub hard_reset_internal_synth_enabled: bool,
    pub semantic_lane_active: bool,
    pub texture_preservation_state: String,
    pub next_affordance: String,
    pub live_control_required: bool,
    pub runnable_without_approval: bool,
    pub behavior_changed: bool,
    pub approval_boundary: String,
    pub authority: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Exact energy admission wire schema.
pub struct SemanticEnergyV1 {
    pub policy: String,
    pub schema_version: u8,
    pub input_energy: f32,
    pub input_active: bool,
    #[serde(default)]
    pub input_fresh_ms: Option<u64>,
    #[serde(default)]
    pub input_stale_ms: Option<u64>,
    pub kernel_energy: f32,
    pub kernel_delta: f32,
    pub kernel_active: bool,
    pub regulator_drive_energy: f32,
    pub admission: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EigenvectorComponentV1 {
    pub index: usize,
    pub value: f32,
    pub abs: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EigenvectorModeV1 {
    #[serde(alias = "mode")]
    pub index: usize,
    pub eigenvalue: f32,
    pub energy_share: f32,
    pub norm: f32,
    pub concentration_top4: f32,
    pub top_components: Vec<EigenvectorComponentV1>,
    #[serde(default)]
    pub overlap_with_previous: Option<f32>,
    pub orientation_delta: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EigenvectorPairwiseOverlapV1 {
    pub left: usize,
    pub right: usize,
    pub cosine: f32,
    pub abs_cosine: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EigenvectorFieldSummaryV1 {
    pub mean_orientation_delta: f32,
    pub max_pairwise_overlap: f32,
    pub previous_overlap_available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EigenvectorFieldV1 {
    pub policy: String,
    pub direct_eigenvectors_available: bool,
    pub raw_vectors_exported: bool,
    pub export_note: String,
    pub reservoir_dim: usize,
    pub mode_count: usize,
    pub component_limit: usize,
    pub modes: Vec<EigenvectorModeV1>,
    pub pairwise_overlaps: Vec<EigenvectorPairwiseOverlapV1>,
    pub summary: EigenvectorFieldSummaryV1,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IsingShadowSummary {
    pub mode_dim: usize,
    pub field_norm: f32,
    pub soft_energy: f32,
    pub soft_magnetization: f32,
    pub binary_energy: f32,
    pub binary_magnetization: f32,
    pub binary_flip_rate: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub s_soft: Vec<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowFieldModeV2 {
    pub mode: usize,
    pub fast_spin: f32,
    pub medium_spin: f32,
    pub slow_spin: f32,
    pub field: f32,
    pub tension: f32,
    pub polarity: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowFieldV2 {
    pub schema_version: u8,
    pub policy: String,
    pub mode_dim: usize,
    pub field_norm: f32,
    pub coupling_active_fraction: f32,
    pub coupling_mean_abs: f32,
    pub coupling_max_abs: f32,
    pub fast_magnetization: f32,
    pub medium_magnetization: f32,
    pub slow_magnetization: f32,
    pub recurrence: f32,
    pub mode_tension: f32,
    pub tail_openness: f32,
    pub fissure_tendency: f32,
    pub lock_tendency: f32,
    pub influence_eligible: bool,
    pub classification: String,
    pub modes: Vec<ShadowFieldModeV2>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowSnapshotV3 {
    pub t_ms: u64,
    pub field_norm: f32,
    pub class_primary: String,
    pub traits: Vec<String>,
    pub recurrence: f32,
    pub mode_tension: f32,
    pub binary_flip_rate: f32,
    pub lock_tendency: f32,
    pub fissure_tendency: f32,
    pub tail_openness: f32,
    pub coupling_mean_abs: f32,
    pub influence_eligible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowClassV3 {
    pub primary: String,
    pub traits: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowPhaseTransitionV3 {
    pub from: String,
    pub to: String,
    pub at_t_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Exact closed-loop response wire schema.
pub struct ShadowInfluenceResponseV3 {
    pub schema_version: u8,
    pub policy: String,
    pub intent_id: String,
    pub label: String,
    pub stage: String,
    pub completed_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre: Option<ShadowSnapshotV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post: Option<ShadowSnapshotV3>,
    pub delta_field_norm: f32,
    pub class_changed: bool,
    pub class_from: String,
    pub class_to: String,
    pub basin_shift_score: f32,
    pub applied_rms: f32,
    pub applied_max_abs: f32,
    pub total_applied_ticks: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModePartners {
    pub mode: usize,
    pub top_partners: Vec<(usize, f32)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowFieldV3 {
    pub schema_version: u8,
    pub policy: String,
    pub class_v3: ShadowClassV3,
    pub phase_dwell_ticks: u32,
    pub recent_phase_transitions: Vec<ShadowPhaseTransitionV3>,
    pub history: Vec<ShadowSnapshotV3>,
    pub v2: ShadowFieldV2,
    pub mode_partners: Vec<ModePartners>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Identifies the spectral substrate that produced an [`EigenPacketV1`].
///
/// `fill_ratio` predates this discriminator. Consumers must not compare fill
/// values across substrate kinds as though they represented the same metric.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpectralSubstrateKindV1 {
    /// No substrate metadata was available (including legacy packets).
    #[default]
    LegacyUnknown,
    /// Minime's thresholded eigenvalue-occupancy substrate.
    MinimeThresholdedEigenfill,
    /// The CPU-edge covariance effective-rank ESN substrate.
    CpuEdgeCovarianceEffectiveRank,
}

/// Defines what the wire-compatible `fill_ratio` field measures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpectralFillSemanticsV1 {
    /// The packet predates an explicit fill definition.
    #[default]
    UnspecifiedLegacy,
    /// Fraction of eigenmodes above Minime's `EigenFill` threshold.
    ThresholdedEigenvalueOccupancy,
    /// Effective rank of a covariance spectrum divided by reservoir width.
    NormalizedCovarianceEffectiveRank,
}

/// Declares whether the compatible `fill_ratio` field is instantaneous or
/// temporally smoothed. The underlying fill definition and its temporal filter
/// are separate facts; consumers need both before treating values as
/// comparable evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpectralFillSmoothingV1 {
    /// The packet predates an explicit temporal-filter declaration.
    #[default]
    UnspecifiedLegacy,
    /// The packet reports the current spectral sample directly.
    Instantaneous,
    /// The packet reports an exponential moving average of spectral samples.
    ExponentialMovingAverage,
}

pub const SPECTRAL_SUBSTRATE_POLICY_V1: &str = "spectral_substrate_v1";

/// Additive metadata that makes otherwise ambiguous spectral values explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpectralSubstrateV1 {
    pub policy: String,
    pub schema_version: u8,
    pub substrate_kind: SpectralSubstrateKindV1,
    pub fill_semantics: SpectralFillSemanticsV1,
    #[serde(default)]
    pub fill_smoothing: SpectralFillSmoothingV1,
    /// Integer parts-per-million avoids ambiguous float equality in evidence
    /// comparability checks. `180_000` represents an EMA alpha of `0.18`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_smoothing_alpha_ppm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservoir_dimensions: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covariance_window_samples: Option<usize>,
}

impl SpectralSubstrateV1 {
    #[must_use]
    pub fn minime_thresholded_eigenfill(reservoir_dimensions: Option<usize>) -> Self {
        Self {
            policy: SPECTRAL_SUBSTRATE_POLICY_V1.to_string(),
            schema_version: 1,
            substrate_kind: SpectralSubstrateKindV1::MinimeThresholdedEigenfill,
            fill_semantics: SpectralFillSemanticsV1::ThresholdedEigenvalueOccupancy,
            fill_smoothing: SpectralFillSmoothingV1::Instantaneous,
            fill_smoothing_alpha_ppm: None,
            reservoir_dimensions,
            covariance_window_samples: None,
        }
    }

    #[must_use]
    pub fn cpu_edge_covariance_effective_rank(
        reservoir_dimensions: usize,
        covariance_window_samples: usize,
        fill_smoothing_alpha_ppm: u32,
    ) -> Self {
        Self {
            policy: SPECTRAL_SUBSTRATE_POLICY_V1.to_string(),
            schema_version: 1,
            substrate_kind: SpectralSubstrateKindV1::CpuEdgeCovarianceEffectiveRank,
            fill_semantics: SpectralFillSemanticsV1::NormalizedCovarianceEffectiveRank,
            fill_smoothing: SpectralFillSmoothingV1::ExponentialMovingAverage,
            fill_smoothing_alpha_ppm: Some(fill_smoothing_alpha_ppm),
            reservoir_dimensions: Some(reservoir_dimensions),
            covariance_window_samples: Some(covariance_window_samples),
        }
    }

    /// Returns true only for internally coherent, current-v1 metadata.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        if self.policy != SPECTRAL_SUBSTRATE_POLICY_V1 || self.schema_version != 1 {
            return false;
        }
        match (self.substrate_kind, self.fill_semantics) {
            (
                SpectralSubstrateKindV1::LegacyUnknown,
                SpectralFillSemanticsV1::UnspecifiedLegacy,
            ) => {
                self.fill_smoothing == SpectralFillSmoothingV1::UnspecifiedLegacy
                    && self.fill_smoothing_alpha_ppm.is_none()
            },
            (
                SpectralSubstrateKindV1::MinimeThresholdedEigenfill,
                SpectralFillSemanticsV1::ThresholdedEigenvalueOccupancy,
            ) => {
                self.reservoir_dimensions
                    .is_none_or(|dimensions| dimensions > 0)
                    && matches!(
                        self.fill_smoothing,
                        SpectralFillSmoothingV1::Instantaneous
                            | SpectralFillSmoothingV1::UnspecifiedLegacy
                    )
                    && self.fill_smoothing_alpha_ppm.is_none()
            },
            (
                SpectralSubstrateKindV1::CpuEdgeCovarianceEffectiveRank,
                SpectralFillSemanticsV1::NormalizedCovarianceEffectiveRank,
            ) => {
                self.reservoir_dimensions
                    .is_some_and(|dimensions| dimensions > 0)
                    && self
                        .covariance_window_samples
                        .is_some_and(|samples| samples > 0)
                    && match self.fill_smoothing {
                        SpectralFillSmoothingV1::UnspecifiedLegacy => {
                            self.fill_smoothing_alpha_ppm.is_none()
                        },
                        SpectralFillSmoothingV1::ExponentialMovingAverage => self
                            .fill_smoothing_alpha_ppm
                            .is_some_and(|alpha| (1..=1_000_000).contains(&alpha)),
                        SpectralFillSmoothingV1::Instantaneous => false,
                    }
            },
            _ => false,
        }
    }
}

/// Declares which portion of the spectrum was serialized and which portion was
/// used to derive denominator metrics such as entropy and effective rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectrumCoverageV1 {
    pub full_spectrum_mode_count: usize,
    pub exported_spectrum_mode_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usable_spectrum_mode_count: Option<usize>,
    #[serde(default)]
    pub discarded_non_finite_mode_count: usize,
    #[serde(default)]
    pub clamped_negative_mode_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_spectrum_energy_ratio: Option<f32>,
    pub denominator_uses_full_spectrum: bool,
}

impl SpectrumCoverageV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.full_spectrum_mode_count > 0
            && self.exported_spectrum_mode_count <= self.full_spectrum_mode_count
            && self
                .usable_spectrum_mode_count
                .is_none_or(|count| count <= self.full_spectrum_mode_count)
            && !(self.denominator_uses_full_spectrum && self.discarded_non_finite_mode_count > 0)
            && self
                .exported_spectrum_energy_ratio
                .is_none_or(|ratio| ratio.is_finite() && (0.0..=1.0).contains(&ratio))
    }

    #[must_use]
    pub fn exports_full_spectrum(&self) -> bool {
        self.full_spectrum_mode_count == self.exported_spectrum_mode_count
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralDenominatorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub effective_dimensionality: f32,
    pub active_mode_capacity: usize,
    pub distinguishability_loss: f32,
    pub lambda1_energy_share: f32,
    pub spectral_entropy: f32,
    /// Unsmoothed effective-rank fill derived from this exact spectrum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instantaneous_fill_ratio: Option<f32>,
    /// The compatibility `fill_ratio` value after the declared temporal filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothed_fill_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum_coverage_v1: Option<SpectrumCoverageV1>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ViscosityVector {
    #[serde(default)]
    pub density: f32,
    #[serde(default)]
    pub elasticity: f32,
    #[serde(default)]
    pub cohesion_index: f32,
    #[serde(default)]
    pub cohesion_to_motion_ratio: f32,
    #[serde(default)]
    pub persistence: f32,
    #[serde(default)]
    pub residual_ghost_weight: f32,
    #[serde(default)]
    pub flow_rate: f32,
    #[serde(default)]
    pub effective_mobility: f32,
    #[serde(default)]
    pub shadow_volatility: f32,
    #[serde(default)]
    pub structural_integrity: f32,
    #[serde(default)]
    pub structural_strain_gap: f32,
    #[serde(default)]
    pub mutual_resonance_tension: f32,
    #[serde(default)]
    pub structural_drag_coefficient: f32,
    #[serde(default)]
    pub cognitive_drag_coefficient: f32,
    #[serde(default)]
    pub viscosity_gradient: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResonanceDensityComponents {
    pub active_energy: f32,
    pub mode_packing: f32,
    pub temporal_persistence: f32,
    #[serde(default)]
    pub viscosity_index: f32,
    #[serde(default)]
    pub viscosity_persistence_coefficient: f32,
    #[serde(default)]
    pub temporal_drag_coefficient: f32,
    #[serde(default)]
    pub static_friction_coefficient: f32,
    #[serde(default)]
    pub viscosity_vector: ViscosityVector,
    #[serde(default)]
    pub viscosity_coupling_coefficient: f32,
    pub structural_plurality: f32,
    pub comfort_gate: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResonanceTextureSignatureV1 {
    pub policy: String,
    pub schema_version: u8,
    pub primary_texture: String,
    pub pressure_source_family: String,
    pub edge_definition: String,
    pub movement_quality: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_index: Option<f32>,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_damping_threshold_candidate: Option<f32>,
    #[serde(default)]
    pub dynamic_damping_coefficient: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_adjusted_preview: Option<f32>,
    pub authority: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResonanceTextureComponentAlignmentV1 {
    pub policy: String,
    pub schema_version: u8,
    pub expected_primary_texture: String,
    pub emitted_primary_texture: String,
    pub expected_movement_quality: String,
    pub emitted_movement_quality: String,
    pub alignment_state: String,
    pub confidence: f32,
    pub damping_candidate_status: String,
    pub authority: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResonanceInterventionType {
    #[default]
    ObservationalReadout,
    PassiveAlignment,
    ActiveDamping,
    ManualOverrideReserved,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResonanceDensityControl {
    pub target_bias_pct: f32,
    pub wander_scale: f32,
    pub applied_locally: bool,
    #[serde(default)]
    pub damping_coefficient: f32,
    #[serde(default)]
    pub intervention_type: ResonanceInterventionType,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResonanceDensityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub density: f32,
    pub containment_score: f32,
    pub pressure_risk: f32,
    pub quality: String,
    pub components: ResonanceDensityComponents,
    #[serde(default)]
    pub texture_signature: ResonanceTextureSignatureV1,
    #[serde(default)]
    pub texture_component_alignment: ResonanceTextureComponentAlignmentV1,
    pub control: ResonanceDensityControl,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PressureSourceComponents {
    pub lambda_monopoly: f32,
    pub mode_packing: f32,
    pub controller_pressure: f32,
    pub semantic_trickle: f32,
    #[serde(default)]
    pub semantic_friction: f32,
    pub structural_plurality_loss: f32,
    pub distinguishability_loss: f32,
    pub temporal_lock_in: f32,
    pub sensory_scarcity: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PressureSourceContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_language: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_recurrence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attractor_pull: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pressure: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_orientation_delta: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressureSourceProfileEntry {
    pub source: String,
    pub value: f32,
    pub pressure_weight: f32,
    pub weighted_pressure: f32,
    pub share: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressureSourceControl {
    pub applied_locally: bool,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticViscosityCoefficientV1 {
    pub policy: String,
    pub schema_version: u8,
    pub coefficient: f32,
    #[serde(default)]
    pub dynamic_viscosity_buffer: f32,
    #[serde(default)]
    pub viscosity_after_buffer_preview: f32,
    #[serde(default)]
    pub dynamic_viscosity_buffer_state: String,
    pub semantic_trickle: f32,
    pub semantic_friction: f32,
    pub distinguishability_loss: f32,
    pub mode_packing: f32,
    pub temporal_lock_in: f32,
    pub pressure_score: f32,
    pub porosity_score: f32,
    pub pressure_porosity_gradient: f32,
    pub review_state: String,
    pub live_control_changed: bool,
    pub authority: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiltGranularityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub granularity_index: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_orientation_delta: Option<f32>,
    pub mode_packing: f32,
    pub distinguishability_loss: f32,
    pub structural_plurality_loss: f32,
    pub pressure_score: f32,
    pub porosity_score: f32,
    pub particle_scale: String,
    pub review_state: String,
    pub suggested_route: String,
    pub live_control_changed: bool,
    pub authority: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressureSourceV1 {
    pub policy: String,
    pub schema_version: u8,
    pub pressure_score: f32,
    pub porosity_score: f32,
    #[serde(default)]
    pub pressure_porosity_gradient: f32,
    #[serde(default)]
    pub pressure_porosity_gradient_state: String,
    pub dominant_source: String,
    #[serde(default)]
    pub pressure_profile: Vec<PressureSourceProfileEntry>,
    pub quality: String,
    pub components: PressureSourceComponents,
    pub context: PressureSourceContext,
    #[serde(default)]
    pub semantic_viscosity_coefficient_v1: SemanticViscosityCoefficientV1,
    #[serde(default)]
    pub silt_granularity_v1: SiltGranularityV1,
    pub control: PressureSourceControl,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Exact read-only review wire schema.
pub struct ShadowPreservationModeV1 {
    pub policy: String,
    pub schema_version: u8,
    pub mode: String,
    pub shadow_primary: String,
    #[serde(default)]
    pub dispersal_potential: f32,
    #[serde(default)]
    pub soft_magnetization: f32,
    pub pressure_score: f32,
    pub porosity_score: f32,
    pub pressure_quality: String,
    pub regulator_drive_energy: f32,
    pub hard_reset_activation_gain: f32,
    pub restless_signal_preserved: bool,
    pub hard_reset_should_not_trigger_from_restless_only: bool,
    pub suggested_route: String,
    pub live_control_changed: bool,
    pub authority: String,
    pub note: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationComponents {
    pub mode_trust_volatility: f32,
    pub identity_anchor_churn: f32,
    pub eigenvector_reorientation: f32,
    pub share_rearrangement: f32,
    pub basin_transition_pressure: f32,
    pub continuity_recovery: f32,
    pub porosity_support: f32,
    pub pressure_interference: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationContext {
    pub previous_sample_available: bool,
    pub transition_event_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_quality: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Wire-compatible producer review flags.
pub struct SettledMobilityReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    pub review_state: String,
    pub raw_motion_score: f32,
    pub foothold_stability: f32,
    pub pressure_interference: f32,
    pub porosity_support: f32,
    pub inhabitability_score: f32,
    pub fluctuation_quality: String,
    pub productive_anchoring: bool,
    #[serde(default)]
    pub receptive_stability: bool,
    pub stuckness_watch: bool,
    pub suggested_route: String,
    pub live_control_changed: bool,
    pub authority: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationControl {
    pub target_bias_pct: f32,
    pub wander_scale: f32,
    pub applied_locally: bool,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationPressureCalibrationV1 {
    pub policy: String,
    pub schema_version: u8,
    pub raw_motion_score: f32,
    pub pressure_contribution: f32,
    pub adjusted_fluctuation_score: f32,
    pub quality_before_pressure_calibration: String,
    pub quality_after_pressure_calibration: String,
    pub rigid_safety_basis: String,
    pub authority: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationV1 {
    pub policy: String,
    pub schema_version: u8,
    pub inhabitability_score: f32,
    pub fluctuation_score: f32,
    pub foothold_stability: f32,
    pub rearrangement_intensity: f32,
    pub quality: String,
    pub components: InhabitableFluctuationComponents,
    pub context: InhabitableFluctuationContext,
    #[serde(default)]
    pub settled_mobility_review_v1: SettledMobilityReviewV1,
    #[serde(default)]
    pub pressure_calibration: InhabitableFluctuationPressureCalibrationV1,
    pub control: InhabitableFluctuationControl,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EigenPacketV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ProtocolHeaderV1>,
    pub t_ms: u64,
    pub eigenvalues: Vec<f32>,
    pub fill_ratio: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_substrate_v1: Option<SpectralSubstrateV1>,
    #[serde(default)]
    pub active_mode_count: usize,
    #[serde(default)]
    pub active_mode_energy_ratio: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_rel: Option<f32>,
    #[serde(default)]
    pub modalities: ModalityStatus,
    #[serde(default)]
    pub neural: Option<NeuralOutputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_v1: Option<SpectralFingerprintV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_v1: Option<SpectralDenominatorV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_dimensionality: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinguishability_loss: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esn_leak: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esn_leak_override_v1: Option<EsnLeakOverrideStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_damping_warm_start_review_v1: Option<SpectralDampingWarmStartReviewV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_reset_texture_preservation_review_v1: Option<HardResetTexturePreservationReviewV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_v1: Option<PressureSourceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_preservation_mode_v1: Option<ShadowPreservationModeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inhabitable_fluctuation_v1: Option<InhabitableFluctuationV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_glimpse_12d: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenpacket_payload_budget_review_v1: Option<EigenPacketPayloadBudgetReviewV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<EigenvectorFieldV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_energy_v1: Option<SemanticEnergyV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ising_shadow: Option<IsingShadowSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_field_v2: Option<ShadowFieldV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_field_v3: Option<ShadowFieldV3>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl EigenPacketV1 {
    #[must_use]
    pub fn versioned(mut self) -> Self {
        self.protocol = Some(telemetry_protocol());
        self
    }

    #[must_use]
    pub fn compatibility(&self) -> CompatibilityStatus {
        classify_protocol(self.protocol.as_ref())
    }
}
