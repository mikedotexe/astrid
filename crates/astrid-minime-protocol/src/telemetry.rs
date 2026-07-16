use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CompatibilityStatus, ProtocolHeaderV1, classify_protocol, current_protocol};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralDenominatorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub effective_dimensionality: f32,
    pub active_mode_capacity: usize,
    pub distinguishability_loss: f32,
    pub lambda1_energy_share: f32,
    pub spectral_entropy: f32,
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
    pub active_mode_count: usize,
    pub active_mode_energy_ratio: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_rel: Option<f32>,
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
        self.protocol = Some(current_protocol());
        self
    }

    #[must_use]
    pub fn compatibility(&self) -> CompatibilityStatus {
        classify_protocol(self.protocol.as_ref())
    }
}
