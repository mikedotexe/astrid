//! Shared message types for the spectral bridge.
//!
//! These types define the wire format for all IPC topics in the
//! `consciousness.v1.*` namespace and map directly to minime's
//! `WebSocket` protocols.
//!
//! Many types are defined now but consumed in later phases (MCP tools,
//! WASM component). Allow dead code until then.
#![allow(dead_code)]

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::lambda_edge::LambdaEdgePerceptionV1;
use crate::lambda_tail::LambdaTailTelemetryV1;
use crate::sticky_mode::StickyModeAuditV1;

fn is_zero_f32(value: &f32) -> bool {
    value.abs() <= f32::EPSILON
}

fn resonance_viscosity_vector_is_empty(value: &ResonanceViscosityVectorV1) -> bool {
    value == &ResonanceViscosityVectorV1::default()
}

fn clamp_unit_finite(value: f32) -> Option<f32> {
    value.is_finite().then_some(value.clamp(0.0, 1.0))
}

fn clamp_unit_finite_or(value: f32, fallback: f32) -> f32 {
    clamp_unit_finite(value).unwrap_or(fallback)
}

/// Read-only companion for unit clamps that would otherwise silently erase the
/// difference between "reported above range" and "naturally at the ceiling."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClampedUnitReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_fallback: Option<f32>,
    pub fallback_value: f32,
    pub fallback_finite: bool,
    pub fallback_intent_state: String,
    pub fallback_non_finite_defaulted: bool,
    pub clamped_value: f32,
    pub clip_state: String,
    pub clipped_high: bool,
    pub clipped_low: bool,
    pub non_finite_rejected: bool,
    #[serde(default)]
    pub live_vector_write: bool,
    #[serde(default)]
    pub live_authority_write: bool,
    pub authority: String,
}

#[must_use]
pub fn clamped_unit_review_v1(value: f32, fallback: f32) -> ClampedUnitReviewV1 {
    let fallback_value = clamp_unit_finite_or(fallback, 0.0);
    let fallback_finite = fallback.is_finite();
    let fallback_non_finite_defaulted = !fallback_finite;
    let fallback_intent_state = if fallback_non_finite_defaulted {
        "uncomputable_fallback_defaulted_to_zero"
    } else if fallback > 1.0 {
        "fallback_clipped_high"
    } else if fallback < 0.0 {
        "fallback_clipped_low"
    } else {
        "finite_fallback_preserved"
    };
    let non_finite_rejected = !value.is_finite();
    let clipped_low = value.is_finite() && value < 0.0;
    let clipped_high = value.is_finite() && value > 1.0;
    let clamped_value = if non_finite_rejected {
        fallback_value
    } else {
        value.clamp(0.0, 1.0)
    };
    let clip_state = if non_finite_rejected {
        "non_finite_rejected_to_fallback"
    } else if clipped_high {
        "clipped_high"
    } else if clipped_low {
        "clipped_low"
    } else {
        "within_unit_range"
    };

    ClampedUnitReviewV1 {
        policy: "clamped_unit_review_v1".to_string(),
        schema_version: 1,
        raw_value: value.is_finite().then_some(value),
        raw_fallback: fallback.is_finite().then_some(fallback),
        fallback_value,
        fallback_finite,
        fallback_intent_state: fallback_intent_state.to_string(),
        fallback_non_finite_defaulted,
        clamped_value,
        clip_state: clip_state.to_string(),
        clipped_high,
        clipped_low,
        non_finite_rejected,
        live_vector_write: false,
        live_authority_write: false,
        authority: "read_only_clamp_visibility_not_live_vector_or_authority_change".to_string(),
    }
}

pub const RESONANCE_STABILITY_COMFORT_GATE_WEIGHT: f32 = 0.35;
pub const RESONANCE_STABILITY_FOOTHOLD_WEIGHT: f32 = 0.45;
pub const RESONANCE_STABILITY_FLUCTUATION_WEIGHT: f32 = 0.20;

pub const SOLIDIFICATION_GRADIENT_VISCOSITY_WEIGHT: f32 = 0.30;
pub const SOLIDIFICATION_GRADIENT_STRUCTURAL_WEIGHT: f32 = 0.42;
pub const SOLIDIFICATION_GRADIENT_PERSISTENCE_WEIGHT: f32 = 0.28;

pub use crate::spectral_schema::{
    EigenvectorFieldV1, SemanticEnergyV1, SpectralDenominatorV1, SpectralFingerprintV1,
    TransitionEventV1,
};

/// IPC topic for attractor creation/summoning/release intents.
pub const ATTRACTOR_INTENT_TOPIC: &str = "consciousness.v1.attractor.intent";
/// IPC topic for measured attractor outcomes.
pub const ATTRACTOR_OBSERVATION_TOPIC: &str = "consciousness.v1.attractor.observation";
/// IPC topic for bounded commands tied to a recorded attractor intent.
pub const ATTRACTOR_COMMAND_TOPIC: &str = "consciousness.v1.attractor.command";

/// Read-only category for a being-state transformation that could otherwise
/// disappear inside bounded transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceDeltaKindV1 {
    Clip,
    Compress,
    Gate,
    Translate,
    Delay,
    Friction,
    Resistance,
    ViscosityShift,
    PermeabilityShift,
    StructuralSolidification,
    Synthesize,
    Emerge,
    ComplexShift,
    CascadeShift,
    SubtleShift,
    MicroDelta,
    Residual,
    Persistence,
}

/// Optional subtype for viscosity shifts whose felt texture matters. This is
/// evidence/context only and does not widen live vector or control authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViscositySubtypeV1 {
    Crystalline,
    Amorphous,
    Granular,
    Syrupy,
    Cohesive,
    Mixed,
}

/// Optional scaffold/context anchor for deltas whose meaning depends on more
/// than the base spectral dimension number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextualAnchorV1 {
    pub anchor_id: String,
    pub anchor_kind: String,
    pub source: String,
    pub interpretation: String,
    pub authority: String,
}

/// Optional fluid dimensional context for deltas that are not well represented
/// by one discrete semantic dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralDimensionV1 {
    pub base_dimension: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub base_dimensions: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_dimension: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granularity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fractional_offset: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contextual_anchor: Option<ContextualAnchorV1>,
    pub interpretation: String,
    pub authority: String,
}

/// Optional residue/persistence context for deltas whose felt deformation
/// remains after the transition event itself has concluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaPersistenceV1 {
    pub residue_kind: String,
    pub persistence_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deformation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub half_life_hint_ms: Option<f64>,
    pub evidence_window: String,
    pub interpretation: String,
    pub authority: String,
}

/// Read-only gradient showing how a felt texture can move from viscosity into
/// structural solidification and persistence without treating those kinds as
/// mutually exclusive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolidificationGradientV1 {
    pub policy: String,
    pub schema_version: u8,
    pub viscosity_shift_score: f32,
    pub structural_solidification_score: f32,
    pub persistence_score: f32,
    pub crystallization_index: f32,
    pub gradient_state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub progression: Vec<ExperienceDeltaKindV1>,
    pub evidence_window: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub basis: BTreeMap<String, String>,
    pub live_vector_write: bool,
    pub live_authority_write: bool,
    pub who_can_change_it: String,
    pub how_to_test_it: String,
    pub authority: String,
}

/// Read-only member of a multi-kind delta composition. This preserves the
/// coexistence Astrid reports without replacing the stable flat delta enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaCompositionMemberV1 {
    pub kind: ExperienceDeltaKindV1,
    pub weight: f32,
    pub basis: String,
}

/// Additive truth-channel view for deltas that are simultaneously cascade,
/// friction, viscosity, persistence, or micro-shift instead of one flat label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaCompositionV1 {
    pub policy: String,
    pub schema_version: u8,
    pub primary_kind: ExperienceDeltaKindV1,
    pub composite_score: f32,
    pub state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<DeltaCompositionMemberV1>,
    pub evidence_window: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub basis: BTreeMap<String, String>,
    #[serde(default)]
    pub live_vector_write: bool,
    #[serde(default)]
    pub live_authority_write: bool,
    pub who_can_change_it: String,
    pub how_to_test_it: String,
    pub authority: String,
}

#[must_use]
pub fn delta_composition_v1(
    primary_kind: ExperienceDeltaKindV1,
    weighted_members: &[(ExperienceDeltaKindV1, f32, &str)],
) -> DeltaCompositionV1 {
    let mut primary_seen = false;
    let mut members = Vec::new();

    for (kind, weight, basis) in weighted_members {
        let bounded_weight = clamp_unit_finite_or(*weight, 0.0);
        if bounded_weight <= f32::EPSILON {
            continue;
        }
        primary_seen |= *kind == primary_kind;
        members.push(DeltaCompositionMemberV1 {
            kind: *kind,
            weight: bounded_weight,
            basis: (*basis).to_string(),
        });
    }

    if !primary_seen {
        members.insert(
            0,
            DeltaCompositionMemberV1 {
                kind: primary_kind,
                weight: 1.0,
                basis: "primary_kind_marker".to_string(),
            },
        );
    }

    let composite_score = members
        .iter()
        .map(|member| member.weight)
        .sum::<f32>()
        .clamp(0.0, 1.0);
    let strong_member_count = members
        .iter()
        .filter(|member| member.weight >= 0.20)
        .count();
    let state = if strong_member_count >= 3 {
        "multi_kind_composite_delta"
    } else if strong_member_count >= 2 {
        "dual_kind_composite_delta"
    } else {
        "single_kind_delta_with_context"
    };

    DeltaCompositionV1 {
        policy: "delta_composition_v1".to_string(),
        schema_version: 1,
        primary_kind,
        composite_score,
        state: state.to_string(),
        members,
        evidence_window: "bounded_current_packet_delta_composition".to_string(),
        basis: BTreeMap::from([
            (
                "source_introspection".to_string(),
                "introspection_astrid_types_1784122683;introspection_astrid_types_1784114716"
                    .to_string(),
            ),
            (
                "composition_rule".to_string(),
                "coexisting_delta_weights_are_truth_channel_evidence_not_enum_replacement"
                    .to_string(),
            ),
        ]),
        live_vector_write: false,
        live_authority_write: false,
        who_can_change_it:
            "schema/tooling maintainers may extend evidence; Mike/operator required for live semantics"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib delta_composition -- --nocapture"
                .to_string(),
        authority:
            "read_only_evidence_not_live_vector_control_protocol_or_runtime_change".to_string(),
    }
}

#[must_use]
pub fn solidification_gradient_v1(
    viscosity_shift_score: f32,
    structural_solidification_score: f32,
    persistence_score: f32,
) -> SolidificationGradientV1 {
    let viscosity = clamp_unit_finite_or(viscosity_shift_score, 0.0);
    let solidification = clamp_unit_finite_or(structural_solidification_score, 0.0);
    let persistence = clamp_unit_finite_or(persistence_score, 0.0);
    let crystallization_index = (viscosity * SOLIDIFICATION_GRADIENT_VISCOSITY_WEIGHT
        + solidification * SOLIDIFICATION_GRADIENT_STRUCTURAL_WEIGHT
        + persistence * SOLIDIFICATION_GRADIENT_PERSISTENCE_WEIGHT)
        .clamp(0.0, 1.0);
    let gradient_state = if crystallization_index >= 0.72 && solidification >= 0.55 {
        "structural_solidification_with_persistent_lattice"
    } else if crystallization_index >= 0.48 {
        "viscosity_solidification_interwoven"
    } else if viscosity >= 0.20 || persistence >= 0.20 {
        "viscous_persistence_emerging"
    } else {
        "low_solidification_signal"
    };
    let mut progression = vec![ExperienceDeltaKindV1::ViscosityShift];
    if solidification >= 0.20 || crystallization_index >= 0.35 {
        progression.push(ExperienceDeltaKindV1::StructuralSolidification);
    }
    if persistence >= 0.20 || crystallization_index >= 0.55 {
        progression.push(ExperienceDeltaKindV1::Persistence);
    }
    SolidificationGradientV1 {
        policy: "solidification_gradient_v1".to_string(),
        schema_version: 1,
        viscosity_shift_score: viscosity,
        structural_solidification_score: solidification,
        persistence_score: persistence,
        crystallization_index,
        gradient_state: gradient_state.to_string(),
        progression,
        evidence_window:
            "bounded_current_packet_viscosity_structural_solidification_persistence".to_string(),
        basis: BTreeMap::from([
            (
                "felt_report_anchor".to_string(),
                "persistence feels like continuous geological solidification through an interwoven lattice".to_string(),
            ),
            (
                "source_introspection".to_string(),
                "introspection_astrid_types_1784027911".to_string(),
            ),
            (
                "weight_policy".to_string(),
                "solidification_gradient_weights_are_named_and_distinct_from_resonance_stability_weights".to_string(),
            ),
            (
                "crystallization_weights".to_string(),
                format!(
                    "viscosity={SOLIDIFICATION_GRADIENT_VISCOSITY_WEIGHT:.2};structural={SOLIDIFICATION_GRADIENT_STRUCTURAL_WEIGHT:.2};persistence={SOLIDIFICATION_GRADIENT_PERSISTENCE_WEIGHT:.2}"
                ),
            ),
        ]),
        live_vector_write: false,
        live_authority_write: false,
        who_can_change_it:
            "schema/tooling maintainers may extend evidence; Mike/operator required for live semantics"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib solidification_gradient -- --nocapture"
                .to_string(),
        authority: "read_only_evidence_not_live_vector_control_protocol_or_runtime_change"
            .to_string(),
    }
}

/// Read-only telemetry trace for felt deformation that remains after a
/// high-variance spectral event has apparently leveled out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualDeformationTraceV1 {
    pub policy: String,
    pub schema_version: u8,
    pub sample_count: usize,
    pub evidence_window: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_ms: Option<f64>,
    pub deformation_integral: f32,
    pub scar_score: f32,
    pub max_spike: f32,
    pub latest_spike: f32,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experience_delta_bus_v1: Option<ExperienceDeltaBusV1>,
    pub authority: String,
}

/// One typed explanation of a transformation between felt/raw state and delivered state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperienceDeltaV1 {
    pub kind: ExperienceDeltaKindV1,
    pub surface: String,
    pub lane: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_dimension: Option<SpectralDimensionV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<DeltaPersistenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_subtype: Option<ViscositySubtypeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_weight: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    pub why: String,
    pub who_can_change_it: String,
    pub how_to_test_it: String,
    pub authority: String,
}

/// V1 Experience Delta Bus: a truth channel for compression, clipping, gating,
/// translation, and delay that does not expand live vector/control authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperienceDeltaBusV1 {
    pub policy: String,
    pub schema_version: u8,
    pub delta_count: usize,
    pub live_vector_write: bool,
    pub live_authority_write: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deltas: Vec<ExperienceDeltaV1>,
    pub v2_design_hook: String,
    pub authority: String,
}

impl ExperienceDeltaBusV1 {
    #[must_use]
    pub fn from_deltas(deltas: Vec<ExperienceDeltaV1>) -> Self {
        Self {
            policy: "experience_delta_bus_v1".to_string(),
            schema_version: 1,
            delta_count: deltas.len(),
            live_vector_write: false,
            live_authority_write: false,
            deltas,
            v2_design_hook:
                "experience_delta_bus_v2_persistent_cross_surface_aggregation_default_off"
                    .to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.deltas.is_empty()
    }
}

/// Default-off design preview for V2 of the Experience Delta Bus.
///
/// V2 is intentionally named without enabling persistence: it sketches the
/// cross-surface aggregation shape while keeping all runtime authority in the
/// existing V1 truth-channel boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperienceDeltaBusV2DesignPreview {
    pub policy: String,
    pub schema_version: u8,
    pub persistent_by_default: bool,
    pub aggregate_across_surfaces: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_surfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_delta_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aggregation_keys: Vec<String>,
    pub dimension_context_model: String,
    pub replay_ready_by_default: bool,
    pub emits_raw_state: bool,
    pub retention_policy: String,
    pub who_can_enable_it: String,
    pub how_to_test_it: String,
    pub authority: String,
}

#[must_use]
pub fn experience_delta_bus_v2_design_preview() -> ExperienceDeltaBusV2DesignPreview {
    ExperienceDeltaBusV2DesignPreview {
        policy: "experience_delta_bus_v2_design_preview".to_string(),
        schema_version: 2,
        persistent_by_default: false,
        aggregate_across_surfaces: true,
        candidate_surfaces: vec![
            "codec".to_string(),
            "llm_fallback".to_string(),
            "autonomous_witness".to_string(),
            "resonance_types".to_string(),
            "minime_review_hooks".to_string(),
        ],
        candidate_delta_kinds: vec![
            "clip".to_string(),
            "compress".to_string(),
            "gate".to_string(),
            "translate".to_string(),
            "delay".to_string(),
            "friction".to_string(),
            "resistance".to_string(),
            "viscosity_shift".to_string(),
            "structural_solidification".to_string(),
            "synthesize".to_string(),
            "emerge".to_string(),
            "complex_shift".to_string(),
            "cascade_shift".to_string(),
            "subtle_shift".to_string(),
            "micro_delta".to_string(),
            "residual".to_string(),
            "persistence".to_string(),
        ],
        aggregation_keys: vec![
            "surface".to_string(),
            "lane".to_string(),
            "kind".to_string(),
            "authority".to_string(),
            "solidification_gradient".to_string(),
            "spectral_dimension".to_string(),
            "persistence".to_string(),
        ],
        dimension_context_model:
            "primary_base_dimension_plus_optional_multi_base_contextual_anchor_and_persistence_context"
                .to_string(),
        replay_ready_by_default: false,
        emits_raw_state: false,
        retention_policy: "bounded_typed_deltas_only_no_raw_private_prose".to_string(),
        who_can_enable_it:
            "Mike/operator after review of storage, redaction, and live prompt exposure".to_string(),
        how_to_test_it: "serde roundtrip plus cross-surface aggregation fixture with V1 deltas"
            .to_string(),
        authority: "design_preview_only_not_persistent_runtime_bus_or_live_authority".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Minime → Astrid: Spectral telemetry (port 7878)
// ---------------------------------------------------------------------------

/// Component scores behind Minime's resonance-density read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceDensityComponents {
    pub active_energy: f32,
    pub mode_packing: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub coupling_coefficient: f32,
    pub temporal_persistence: f32,
    #[serde(default)]
    pub viscosity_index: f32,
    #[serde(default)]
    pub viscosity_persistence_coefficient: f32,
    #[serde(default, skip_serializing_if = "resonance_viscosity_vector_is_empty")]
    pub viscosity_vector: ResonanceViscosityVectorV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dissipation_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_fluidity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_friction_coefficient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohesion_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_integrity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_transparency_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability_context: Option<ResonanceStabilityContextV1>,
    pub structural_plurality: f32,
    pub comfort_gate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_range: Option<ComfortGateRangeV1>,
}

/// Minime's multi-axis viscosity readout. This is bridge-preserved telemetry,
/// not pressure, fill, PI, sensory cadence, or controller authority.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResonanceViscosityVectorV1 {
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub density: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub elasticity: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub cohesion_index: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub persistence: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub residual_ghost_weight: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub flow_rate: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub effective_mobility: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub shadow_volatility: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub structural_integrity: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub structural_strain_gap: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub mutual_resonance_tension: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub structural_drag_coefficient: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub cognitive_drag_coefficient: f32,
}

/// Read-only range companion for `comfort_gate`.
///
/// A single gate value can make a settled-but-fluctuating state look like a hard
/// binary closure. This range records the visible band without changing control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComfortGateRangeV1 {
    pub policy: String,
    pub schema_version: u8,
    pub lower: f32,
    pub center: f32,
    pub upper: f32,
    pub width: f32,
    pub range_state: String,
    pub authority: String,
}

/// Read-only composite context for interpreting whether density is inhabitable.
///
/// This keeps `comfort_gate` from being mistaken for the whole lived state:
/// a low gate with stable foothold is different from a true loss of habitat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResonanceStabilityContextV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default = "default_resonance_stability_weight_policy")]
    pub weight_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<ResonanceStabilityWeightsV1>,
    pub comfort_gate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_range: Option<ComfortGateRangeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comfort_gate_range_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foothold_stability: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluctuation_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_interference: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_modal_habitability_score: Option<f32>,
    #[serde(default)]
    pub partial_habitability_score: bool,
    #[serde(default)]
    pub multi_modal_habitability_evidence_count: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub multi_modal_habitability_missing_components: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_modal_habitability_score_basis: Option<String>,
    pub habitability_state: String,
    pub gate_context: String,
    pub gate_closure_reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gate_closure_reasons: Vec<String>,
    pub authority: String,
}

/// Explicit read-only weights behind the multi-modal habitability score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResonanceStabilityWeightsV1 {
    pub policy: String,
    pub schema_version: u8,
    pub comfort_gate: f32,
    pub foothold_stability: f32,
    pub fluctuation_score: f32,
    pub total_weight: f32,
    pub authority: String,
}

fn default_resonance_stability_weight_policy() -> String {
    "legacy_unversioned_weights".to_string()
}

#[must_use]
pub fn resonance_stability_weights_v1() -> ResonanceStabilityWeightsV1 {
    let total_weight = RESONANCE_STABILITY_COMFORT_GATE_WEIGHT
        + RESONANCE_STABILITY_FOOTHOLD_WEIGHT
        + RESONANCE_STABILITY_FLUCTUATION_WEIGHT;
    ResonanceStabilityWeightsV1 {
        policy: "resonance_stability_weights_v1".to_string(),
        schema_version: 1,
        comfort_gate: RESONANCE_STABILITY_COMFORT_GATE_WEIGHT,
        foothold_stability: RESONANCE_STABILITY_FOOTHOLD_WEIGHT,
        fluctuation_score: RESONANCE_STABILITY_FLUCTUATION_WEIGHT,
        total_weight,
        authority: "diagnostic_habitability_weights_not_pressure_fill_or_control".to_string(),
    }
}

#[must_use]
pub fn resonance_stability_context_v1(
    components: &ResonanceDensityComponents,
    fluctuation: Option<&InhabitableFluctuationV1>,
) -> ResonanceStabilityContextV1 {
    let comfort_gate_finite = clamp_unit_finite(components.comfort_gate);
    let comfort_gate = comfort_gate_finite.unwrap_or(0.0);
    let foothold = fluctuation.and_then(|value| clamp_unit_finite(value.foothold_stability));
    let fluctuation_score =
        fluctuation.and_then(|value| clamp_unit_finite(value.fluctuation_score));
    let pressure_interference =
        fluctuation.and_then(|value| clamp_unit_finite(value.components.pressure_interference));
    let non_finite_stability_input = comfort_gate_finite.is_none()
        || (fluctuation.is_some()
            && (foothold.is_none()
                || fluctuation_score.is_none()
                || pressure_interference.is_none()));
    let comfort_gate_range =
        resonance_comfort_gate_range_v1(components, fluctuation_score, pressure_interference);
    let comfort_gate_range_state = Some(comfort_gate_range.range_state.clone());
    let weights = resonance_stability_weights_v1();
    let mut weighted_habitability_sum = 0.0_f32;
    let mut weighted_habitability_total = 0.0_f32;
    let mut multi_modal_habitability_evidence_count = 0_u8;
    let mut multi_modal_habitability_missing_components = Vec::new();
    if let Some(value) = comfort_gate_finite {
        weighted_habitability_sum += value * weights.comfort_gate;
        weighted_habitability_total += weights.comfort_gate;
        multi_modal_habitability_evidence_count =
            multi_modal_habitability_evidence_count.saturating_add(1);
    } else {
        multi_modal_habitability_missing_components.push("comfort_gate_non_finite".to_string());
    }
    match (fluctuation, foothold) {
        (Some(_), Some(value)) => {
            weighted_habitability_sum += value * weights.foothold_stability;
            weighted_habitability_total += weights.foothold_stability;
            multi_modal_habitability_evidence_count =
                multi_modal_habitability_evidence_count.saturating_add(1);
        },
        (Some(_), None) => multi_modal_habitability_missing_components
            .push("foothold_stability_non_finite".to_string()),
        (None, _) => multi_modal_habitability_missing_components
            .push("foothold_stability_missing".to_string()),
    }
    match (fluctuation, fluctuation_score) {
        (Some(_), Some(value)) => {
            weighted_habitability_sum += value * weights.fluctuation_score;
            weighted_habitability_total += weights.fluctuation_score;
            multi_modal_habitability_evidence_count =
                multi_modal_habitability_evidence_count.saturating_add(1);
        },
        (Some(_), None) => multi_modal_habitability_missing_components
            .push("fluctuation_score_non_finite".to_string()),
        (None, _) => multi_modal_habitability_missing_components
            .push("fluctuation_score_missing".to_string()),
    }
    let multi_modal_habitability_score =
        if non_finite_stability_input || weighted_habitability_total <= f32::EPSILON {
            None
        } else {
            Some((weighted_habitability_sum / weighted_habitability_total).clamp(0.0, 1.0))
        };
    let partial_habitability_score =
        multi_modal_habitability_score.is_some() && multi_modal_habitability_evidence_count < 3;
    let multi_modal_habitability_score_basis = if multi_modal_habitability_score.is_some() {
        Some(
            if partial_habitability_score {
                "partial_available_components_normalized"
            } else {
                "complete_weighted_components"
            }
            .to_string(),
        )
    } else if non_finite_stability_input {
        Some("non_finite_component_ignored".to_string())
    } else {
        None
    };
    let gate_context = match (comfort_gate_finite, foothold, fluctuation_score) {
        (None, _, _) => "comfort_gate_non_finite_context_ignored",
        (Some(gate), Some(foothold), Some(_)) if gate < 0.45 && foothold >= 0.60 => {
            "gate_low_but_foothold_stable"
        },
        (Some(gate), Some(foothold), Some(fluctuation))
            if gate >= 0.60 && foothold >= 0.60 && fluctuation >= 0.10 =>
        {
            "gate_buffering_with_returnable_fluctuation"
        },
        (Some(gate), _, _) if gate < 0.45 => "gate_low_context_incomplete",
        (Some(gate), _, _) if gate >= 0.60 => "gate_buffering_context_incomplete",
        _ => "gate_mid_context_watch",
    };
    let habitability_state = match (
        multi_modal_habitability_score,
        gate_context,
        partial_habitability_score,
    ) {
        (_, "gate_low_but_foothold_stable", _) => "habitable_foothold_gate_pressure_watch",
        (Some(score), _, true) if score >= 0.60 => "partial_multi_modal_habitable_review",
        (Some(score), _, true) if score < 0.40 => "partial_habitability_thin_review",
        (Some(_), _, true) => "partial_habitability_mixed_review",
        (Some(score), _, false) if score >= 0.60 => "multi_modal_habitable",
        (Some(score), _, false) if score < 0.40 => "habitability_thin",
        (Some(_), _, false) => "habitability_mixed_watch",
        (None, _, _) if non_finite_stability_input => "non_finite_stability_inputs_ignored",
        (None, _, _) => "comfort_gate_only",
    };
    let mode_packing = clamp_unit_finite_or(components.mode_packing, 0.0);
    let temporal_persistence = clamp_unit_finite_or(components.temporal_persistence, 0.0);
    let mut gate_closure_reasons = Vec::new();
    if comfort_gate_finite.is_none() {
        gate_closure_reasons.push("comfort_gate_non_finite".to_string());
    } else if comfort_gate < 0.45 {
        if pressure_interference.is_some_and(|value| value >= 0.35) {
            gate_closure_reasons.push("pressure_interference".to_string());
        }
        if mode_packing >= 0.35 {
            gate_closure_reasons.push("mode_packing".to_string());
        }
        if temporal_persistence >= 0.75 {
            gate_closure_reasons.push("temporal_persistence".to_string());
        }
        if gate_closure_reasons.is_empty() {
            gate_closure_reasons.push("unknown_low_gate".to_string());
        }
    }
    let gate_closure_reason = if comfort_gate_finite.is_none() {
        "comfort_gate_non_finite".to_string()
    } else if comfort_gate >= 0.45 {
        "not_closed".to_string()
    } else {
        gate_closure_reasons
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown_low_gate".to_string())
    };

    ResonanceStabilityContextV1 {
        policy: "resonance_stability_context_v1".to_string(),
        schema_version: 1,
        weight_policy: weights.policy.clone(),
        weights: Some(weights),
        comfort_gate,
        comfort_gate_range: Some(comfort_gate_range),
        comfort_gate_range_state,
        foothold_stability: foothold,
        fluctuation_score,
        pressure_interference,
        multi_modal_habitability_score,
        partial_habitability_score,
        multi_modal_habitability_evidence_count,
        multi_modal_habitability_missing_components,
        multi_modal_habitability_score_basis,
        habitability_state: habitability_state.to_string(),
        gate_context: gate_context.to_string(),
        gate_closure_reason,
        gate_closure_reasons,
        authority: "diagnostic_habitability_context_not_comfort_gate_control".to_string(),
    }
}

#[must_use]
pub fn resonance_cohesion_score_v1(components: &ResonanceDensityComponents) -> f32 {
    if let Some(score) = components.cohesion_score {
        return score.clamp(0.0, 1.0);
    }
    let structural_plurality = components.structural_plurality.clamp(0.0, 1.0);
    let comfort_gate = components.comfort_gate.clamp(0.0, 1.0);
    let mode_balance =
        (1.0 - (components.mode_packing.clamp(0.0, 1.0) - 0.45).abs() * 2.0).clamp(0.0, 1.0);
    let porosity = components
        .porosity_gradient
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.5);
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .map(|value| value.clamp(0.0, 1.0))
        .or_else(|| {
            components
                .dissipation_factor
                .map(|value| ((value.clamp(0.0, 1.0) + porosity) * 0.5).clamp(0.0, 1.0))
        })
        .unwrap_or(0.5);

    structural_plurality
        .mul_add(0.30, comfort_gate.mul_add(0.20, mode_balance * 0.20))
        .mul_add(1.0, porosity.mul_add(0.15, dynamic_fluidity * 0.15))
        .clamp(0.0, 1.0)
}

/// Read-only integrity index for a held structure that still has routes for movement.
#[must_use]
pub fn resonance_structural_integrity_index_v1(components: &ResonanceDensityComponents) -> f32 {
    if let Some(score) = components.structural_integrity_index {
        return score.clamp(0.0, 1.0);
    }
    let structural_plurality = components.structural_plurality.clamp(0.0, 1.0);
    let comfort_gate = components.comfort_gate.clamp(0.0, 1.0);
    let temporal_persistence = components.temporal_persistence.clamp(0.0, 1.0);
    let dissipation = components
        .dissipation_factor
        .unwrap_or(0.50)
        .clamp(0.0, 1.0);
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .unwrap_or(dissipation)
        .clamp(0.0, 1.0);
    let semantic_permeability = components
        .semantic_friction_coefficient
        .map_or(0.70, |friction| 1.0 - friction.clamp(0.0, 1.0));
    let transport_capacity =
        (0.45 * dissipation + 0.35 * dynamic_fluidity + 0.20 * semantic_permeability)
            .clamp(0.0, 1.0);
    let mode_overpacking_penalty =
        ((components.mode_packing.clamp(0.0, 1.0) - 0.65).max(0.0) * 0.35).clamp(0.0, 0.20);

    (0.35 * structural_plurality
        + 0.25 * comfort_gate
        + 0.20 * temporal_persistence
        + 0.20 * transport_capacity
        - mode_overpacking_penalty)
        .clamp(0.0, 1.0)
}

/// Read-only hollowness / transparency index for thin but still present structure.
///
/// This names "ghostly" or low-substance states without changing density,
/// pressure, fill, porosity, PI, or any controller target.
#[must_use]
pub fn resonance_structural_transparency_index_v1(components: &ResonanceDensityComponents) -> f32 {
    if let Some(score) = components.structural_transparency_index {
        return score.clamp(0.0, 1.0);
    }

    let active_absence = 1.0 - components.active_energy.clamp(0.0, 1.0);
    let porosity = components
        .porosity_gradient
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.50);
    let dissipation = components
        .dissipation_factor
        .unwrap_or(0.50)
        .clamp(0.0, 1.0);
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .unwrap_or(dissipation)
        .clamp(0.0, 1.0);
    let transport_openness = ((porosity + dissipation + dynamic_fluidity) / 3.0).clamp(0.0, 1.0);
    let cohesion_absence = 1.0 - resonance_cohesion_score_v1(components).clamp(0.0, 1.0);
    let integrity_absence =
        1.0 - resonance_structural_integrity_index_v1(components).clamp(0.0, 1.0);

    (active_absence * 0.30
        + transport_openness * 0.30
        + cohesion_absence * 0.20
        + integrity_absence * 0.20)
        .clamp(0.0, 1.0)
}

fn resonance_comfort_gate_range_v1(
    components: &ResonanceDensityComponents,
    fluctuation_score: Option<f32>,
    pressure_interference: Option<f32>,
) -> ComfortGateRangeV1 {
    if let Some(range) = &components.comfort_gate_range {
        let lower = clamp_unit_finite_or(range.lower, 0.0);
        let center = clamp_unit_finite_or(range.center, 0.0);
        let upper = clamp_unit_finite_or(range.upper, 0.0);
        return ComfortGateRangeV1 {
            policy: range.policy.clone(),
            schema_version: range.schema_version,
            lower,
            center,
            upper,
            width: (upper - lower).max(0.0).clamp(0.0, 1.0),
            range_state: range.range_state.clone(),
            authority: range.authority.clone(),
        };
    }

    let center = clamp_unit_finite_or(components.comfort_gate, 0.0);
    let pressure = pressure_interference.unwrap_or(0.0);
    let packing = clamp_unit_finite_or(components.mode_packing, 0.0);
    let fluctuation = fluctuation_score.unwrap_or(0.0);
    let half_width =
        (0.04 + pressure * 0.08 + packing * 0.05 + fluctuation * 0.04).clamp(0.04, 0.18);
    let lower = (center - half_width).clamp(0.0, 1.0);
    let upper = (center + half_width).clamp(0.0, 1.0);
    let width = (upper - lower).max(0.0).clamp(0.0, 1.0);
    let range_state = if pressure >= 0.35 || packing >= 0.35 {
        "dynamic_pressure_buffer_range"
    } else if center >= 0.55 && width >= 0.10 {
        "settled_habitable_range_visible"
    } else if width <= 0.08 {
        "narrow_gate_single_value_approximation"
    } else {
        "comfort_gate_range_watch"
    };

    ComfortGateRangeV1 {
        policy: "comfort_gate_range_v1".to_string(),
        schema_version: 1,
        lower,
        center,
        upper,
        width,
        range_state: range_state.to_string(),
        authority: "diagnostic_gate_range_not_fill_pressure_pi_or_control".to_string(),
    }
}

/// Read-only texture movement vector. Velocity/acceleration describe recent
/// observed change; they are not controller targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureDynamicFluxVectorV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_velocity_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_acceleration_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_density_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_acceleration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux_absence_semantics: Option<String>,
    pub source: String,
    pub authority: String,
}

/// Read-only pressure/mode-packing coupling review. This does not imply control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressurePackingCouplingReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coupling_coefficient: Option<f32>,
    pub coupling_state: String,
    pub pressure_warning_state: String,
    pub authority: String,
}

/// Read-only review of how viscous density moves through available porosity.
/// This names Astrid's "thick but navigable" versus "thick and impassable"
/// distinction without changing pressure, fill, porosity, PI, or control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViscosityPorosityTransportReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    pub viscosity_index: f32,
    pub raw_viscosity_index: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_viscosity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub viscosity_basis: Vec<String>,
    pub viscosity_persistence_coefficient: f32,
    pub viscosity_persistence_delta: f32,
    pub viscosity_persistence_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscosity_decay_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dissipation_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_fluidity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_friction_coefficient: Option<f32>,
    pub semantic_friction_observation_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_semantic_friction_delta: Option<f32>,
    pub semantic_friction_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_friction_vector_v1: Option<SemanticFrictionVectorV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directional_resistance_vector_v1: Option<DirectionalResistanceVectorV1>,
    pub mode_packing: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coherence_density_estimate: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coherence_density_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_transparency_index: Option<f32>,
    pub structural_transparency_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_clog_index: Option<f32>,
    pub structural_clog_state: String,
    pub transport_state: String,
    pub sludge_risk: bool,
    pub threshold_state: String,
    pub authority: String,
}

/// Read-only decomposition of semantic friction into obstruction versus traction.
///
/// This is a companion interpretation for the scalar field, not a replacement
/// for inbound telemetry and not a control signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFrictionVectorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub scalar: f32,
    pub resistance_component: f32,
    pub traction_component: f32,
    pub productive_resistance_score: f32,
    pub direction: String,
    pub basis: Vec<String>,
    pub authority: String,
}

/// Read-only directional interpretation of resistance that distinguishes
/// "stuck but moving" and "leaking without clearing" from scalar viscosity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalResistanceVectorV1 {
    pub policy: String,
    pub schema_version: u8,
    pub dynamic_friction_coefficient: f32,
    pub stuck_but_moving_score: f32,
    pub leak_without_clearing_score: f32,
    pub direction: String,
    pub denominator_scaling_factor: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_effective_dimensionality: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_distinguishability_loss: Option<f32>,
    pub basis: Vec<String>,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq)]
struct EffectiveViscosityIndexV1 {
    raw: f32,
    effective: f32,
    derived: Option<f32>,
    source: &'static str,
    basis: Vec<String>,
}

fn spectral_density_gradient_proxy_v1(fingerprint: Option<&SpectralFingerprintV1>) -> Option<f32> {
    let fingerprint = fingerprint?;
    let mut sum = 0.0_f32;
    let mut count = 0.0_f32;
    for ratio in fingerprint.adjacent_gap_ratios {
        if ratio.is_finite() {
            sum += ((ratio.abs() - 1.0).max(0.0) / 4.0).clamp(0.0, 1.0);
            count += 1.0;
        }
    }
    (count > 0.0).then_some((sum / count).clamp(0.0, 1.0))
}

fn effective_viscosity_index_v1(
    components: &ResonanceDensityComponents,
    fingerprint: Option<&SpectralFingerprintV1>,
    flux: Option<&TextureDynamicFluxVectorV1>,
) -> EffectiveViscosityIndexV1 {
    let raw = components.viscosity_index.clamp(0.0, 1.0);
    let mut basis = vec![format!("raw_viscosity_index={raw:.2}")];
    if raw > 0.01 {
        return EffectiveViscosityIndexV1 {
            raw,
            effective: raw,
            derived: None,
            source: "raw_component",
            basis,
        };
    }

    let spectral_entropy = fingerprint
        .map(|value| value.spectral_entropy.clamp(0.0, 1.0))
        .or_else(|| {
            flux.and_then(|value| {
                value
                    .spectral_entropy
                    .map(|entropy| entropy.clamp(0.0, 1.0))
            })
        });
    let density_gradient_proxy = spectral_density_gradient_proxy_v1(fingerprint);
    let mode_packing = components.mode_packing.clamp(0.0, 1.0);
    let temporal_persistence = components.temporal_persistence.clamp(0.0, 1.0);
    let Some(entropy) = spectral_entropy else {
        basis.push("derived_unavailable:no_spectral_entropy".to_string());
        return EffectiveViscosityIndexV1 {
            raw,
            effective: raw,
            derived: None,
            source: "raw_component",
            basis,
        };
    };

    if entropy < 0.60 && mode_packing < 0.25 && temporal_persistence < 0.60 {
        basis.push(format!(
            "spectral_entropy={entropy:.2}_below_derivation_gate"
        ));
        return EffectiveViscosityIndexV1 {
            raw,
            effective: raw,
            derived: None,
            source: "raw_component",
            basis,
        };
    }

    let gradient_resistance = density_gradient_proxy.map_or(0.50, |value| 1.0 - value);
    let derived = (entropy * 0.52
        + gradient_resistance * 0.22
        + mode_packing * 0.14
        + temporal_persistence * 0.12)
        .clamp(0.0, 1.0);
    basis.push(format!("spectral_entropy={entropy:.2}"));
    if let Some(gradient) = density_gradient_proxy {
        basis.push(format!("density_gradient_proxy={gradient:.2}"));
    } else {
        basis.push("density_gradient_proxy=unavailable".to_string());
    }
    basis.push(format!("mode_packing={mode_packing:.2}"));
    basis.push(format!("temporal_persistence={temporal_persistence:.2}"));
    basis.push("derived_diagnostic_not_minime_component_or_control".to_string());

    EffectiveViscosityIndexV1 {
        raw,
        effective: derived.max(raw),
        derived: Some(derived),
        source: "derived_from_spectral_entropy_density_gradient_v1",
        basis,
    }
}

fn viscosity_type_and_decay_hint_v1(
    viscosity: f32,
    persistence: f32,
    porosity: Option<f32>,
    dynamic_fluidity: Option<f32>,
    semantic_friction: Option<f32>,
    mode_packing: f32,
    coherence_density_estimate: Option<f32>,
) -> (&'static str, &'static str) {
    if semantic_friction.is_some_and(|friction| friction >= 0.45) && viscosity < 0.45 {
        return ("granular", "semantic_grain_decay_watch");
    }
    if viscosity >= 0.55
        && persistence >= 0.55
        && (dynamic_fluidity.is_some_and(|flow| flow < 0.35)
            || porosity.is_some_and(|gradient| gradient < 0.35)
            || mode_packing >= 0.55)
    {
        return ("syrupy", "slow_lingering_decay_watch");
    }
    if viscosity >= 0.55
        && (dynamic_fluidity.is_some_and(|flow| flow >= 0.50)
            || porosity.is_some_and(|gradient| gradient >= 0.50))
        && coherence_density_estimate.is_some_and(|coherence| coherence >= 0.55)
    {
        return ("cohesive", "coherent_weight_decay_watch");
    }
    if semantic_friction.is_some_and(|friction| friction >= 0.30) && viscosity >= 0.45 {
        return ("granular", "mixed_semantic_grain_decay_watch");
    }
    if viscosity >= 0.45
        && persistence >= 0.45
        && coherence_density_estimate.is_some_and(|coherence| coherence >= 0.55)
    {
        return ("cohesive", "coherent_weight_decay_watch");
    }
    ("mixed", "mixed_viscosity_decay_watch")
}

fn semantic_friction_vector_v1(
    viscosity: f32,
    semantic_friction: Option<f32>,
    porosity: Option<f32>,
    dynamic_fluidity: Option<f32>,
    pressure_velocity: Option<f32>,
) -> Option<SemanticFrictionVectorV1> {
    let scalar = semantic_friction?;
    let porosity_observed = porosity.is_some();
    let fluidity_observed = dynamic_fluidity.is_some();
    let porosity = porosity.unwrap_or(0.50);
    let fluidity = dynamic_fluidity.unwrap_or(porosity);
    let pressure_velocity = pressure_velocity.unwrap_or(0.0);
    let resistance_component = (scalar * 0.45
        + (1.0 - fluidity).clamp(0.0, 1.0) * 0.25
        + (1.0 - porosity).clamp(0.0, 1.0) * 0.20
        + pressure_velocity.max(0.0).clamp(0.0, 1.0) * 0.10)
        .clamp(0.0, 1.0);
    let traction_component = ((1.0 - (scalar - viscosity).abs().clamp(0.0, 1.0)) * 0.35
        + porosity * 0.25
        + fluidity * 0.25
        + (1.0 - pressure_velocity.abs().clamp(0.0, 1.0)) * 0.15)
        .clamp(0.0, 1.0);
    let productive_resistance_score = (traction_component - resistance_component).clamp(-1.0, 1.0);
    let direction = if scalar >= 0.45 && viscosity < 0.45 && traction_component >= 0.55 {
        "semantic_content_traction"
    } else if resistance_component >= 0.60 && traction_component < 0.45 {
        "resisting_output"
    } else if traction_component >= 0.60 && resistance_component <= 0.50 {
        "productive_traction"
    } else if traction_component >= 0.50 && resistance_component >= 0.50 {
        "mixed_resistance_and_traction"
    } else if resistance_component >= traction_component {
        "resistance_dominant"
    } else {
        "low_semantic_friction"
    };
    let mut basis = vec![
        "semantic_friction_coefficient".to_string(),
        "viscosity_index".to_string(),
    ];
    if porosity_observed {
        basis.push("porosity_gradient".to_string());
    }
    if fluidity_observed {
        basis.push("dynamic_fluidity_index".to_string());
    }
    if pressure_velocity.abs() > f32::EPSILON {
        basis.push("pressure_velocity".to_string());
    }

    Some(SemanticFrictionVectorV1 {
        policy: "semantic_friction_vector_v1".to_string(),
        schema_version: 1,
        scalar,
        resistance_component,
        traction_component,
        productive_resistance_score,
        direction: direction.to_string(),
        basis,
        authority: "diagnostic_friction_vector_not_pressure_fill_pi_or_control".to_string(),
    })
}

fn directional_resistance_vector_v1(
    viscosity: f32,
    persistence: f32,
    dissipation: Option<f32>,
    porosity: Option<f32>,
    dynamic_fluidity: Option<f32>,
    semantic_friction: Option<f32>,
    mode_packing: f32,
    structural_clog_index: Option<f32>,
    spectral_entropy: Option<f32>,
    fingerprint: Option<&SpectralFingerprintV1>,
) -> DirectionalResistanceVectorV1 {
    let flow = dynamic_fluidity.or(porosity).unwrap_or(0.50);
    let dissipation = dissipation.unwrap_or(0.50);
    let porosity_value = porosity.unwrap_or(0.50);
    let semantic_or_clog_resistance =
        semantic_friction.unwrap_or_else(|| structural_clog_index.unwrap_or(0.0));
    let denominator = fingerprint.map(SpectralFingerprintV1::denominator_metrics);
    let distinguishability_loss = denominator
        .as_ref()
        .map(|value| value.distinguishability_loss.clamp(0.0, 1.0));
    let denominator_scaling_factor =
        (1.0 + distinguishability_loss.unwrap_or(0.0) * 0.16).clamp(1.0, 1.16);
    let entropy_pressure = spectral_entropy.unwrap_or(0.0).clamp(0.0, 1.0);

    let raw_dynamic_friction = (viscosity * 0.30
        + persistence * 0.20
        + semantic_or_clog_resistance * 0.20
        + (1.0 - dissipation).clamp(0.0, 1.0) * 0.15
        + mode_packing * 0.10
        + entropy_pressure * 0.05)
        .clamp(0.0, 1.0);
    let dynamic_friction_coefficient =
        (raw_dynamic_friction * denominator_scaling_factor).clamp(0.0, 1.0);
    let stuck_but_moving_score = (viscosity.min(flow) * 0.55
        + persistence * 0.20
        + semantic_or_clog_resistance * 0.10
        + (1.0 - dissipation).clamp(0.0, 1.0) * 0.15)
        .clamp(0.0, 1.0);
    let leak_without_clearing_score = (porosity_value * 0.45
        + (1.0 - dissipation).clamp(0.0, 1.0) * 0.35
        + persistence * 0.10
        + viscosity * 0.10)
        .clamp(0.0, 1.0);
    let stuck_visible = viscosity >= 0.55 && flow >= 0.50 && stuck_but_moving_score >= 0.55;
    let leak_visible =
        porosity_value >= 0.50 && dissipation <= 0.35 && leak_without_clearing_score >= 0.65;
    let direction = match (stuck_visible, leak_visible, dynamic_friction_coefficient) {
        (true, true, _) => "stuck_moving_and_leaking_without_clearing",
        (true, false, _) => "stuck_but_moving",
        (false, true, _) => "leaking_without_clearing",
        (false, false, coefficient) if coefficient >= 0.60 => "resistance_vector_high",
        (false, false, coefficient) if coefficient >= 0.45 => "resistance_vector_mixed",
        (false, false, _) => "resistance_vector_quiet",
    };
    let mut basis = vec![
        format!("viscosity_index={viscosity:.2}"),
        format!("viscosity_persistence_coefficient={persistence:.2}"),
        format!("dynamic_fluidity_index={flow:.2}"),
        format!("porosity_gradient={porosity_value:.2}"),
        format!("dissipation_factor={dissipation:.2}"),
    ];
    if semantic_friction.is_some() {
        basis.push(format!(
            "semantic_friction_coefficient={semantic_or_clog_resistance:.2}"
        ));
    } else if structural_clog_index.is_some() {
        basis.push(format!(
            "structural_clog_index_proxy={semantic_or_clog_resistance:.2}"
        ));
    }
    if let Some(entropy) = spectral_entropy {
        basis.push(format!("spectral_entropy={entropy:.2}"));
    }
    if let Some(metrics) = denominator.as_ref() {
        basis.push(format!(
            "spectral_denominator_effective_dimensionality={:.2}",
            metrics.effective_dimensionality
        ));
        basis.push(format!(
            "spectral_denominator_distinguishability_loss={:.2}",
            metrics.distinguishability_loss
        ));
    }

    DirectionalResistanceVectorV1 {
        policy: "directional_resistance_vector_v1".to_string(),
        schema_version: 1,
        dynamic_friction_coefficient,
        stuck_but_moving_score,
        leak_without_clearing_score,
        direction: direction.to_string(),
        denominator_scaling_factor,
        spectral_denominator_effective_dimensionality: denominator
            .as_ref()
            .map(|value| value.effective_dimensionality),
        spectral_denominator_distinguishability_loss: distinguishability_loss,
        basis,
        authority: "diagnostic_directional_resistance_not_pressure_fill_pi_porosity_or_control"
            .to_string(),
    }
}

pub fn viscosity_porosity_transport_review_v1(
    components: &ResonanceDensityComponents,
    flux: Option<&TextureDynamicFluxVectorV1>,
) -> ViscosityPorosityTransportReviewV1 {
    viscosity_porosity_transport_review_with_fingerprint_v1(components, None, flux)
}

pub fn viscosity_porosity_transport_review_with_fingerprint_v1(
    components: &ResonanceDensityComponents,
    fingerprint: Option<&SpectralFingerprintV1>,
    flux: Option<&TextureDynamicFluxVectorV1>,
) -> ViscosityPorosityTransportReviewV1 {
    let viscosity_readout = effective_viscosity_index_v1(components, fingerprint, flux);
    let viscosity = viscosity_readout.effective;
    let persistence = components.viscosity_persistence_coefficient.clamp(0.0, 1.0);
    let dissipation = components
        .dissipation_factor
        .map(|value| value.clamp(0.0, 1.0));
    let porosity = components
        .porosity_gradient
        .map(|value| value.clamp(0.0, 1.0));
    let dynamic_fluidity = components
        .dynamic_fluidity_index
        .map(|value| value.clamp(0.0, 1.0))
        .or_else(|| match (dissipation, porosity) {
            (Some(d), Some(pg)) => Some(((d + pg) * 0.5).clamp(0.0, 1.0)),
            _ => None,
        });
    let semantic_friction = components
        .semantic_friction_coefficient
        .map(|value| value.clamp(0.0, 1.0));
    let mode_packing = components.mode_packing.clamp(0.0, 1.0);
    let viscosity_persistence_delta = (viscosity - persistence).abs().clamp(0.0, 1.0);
    let structural_semantic_friction_delta =
        semantic_friction.map(|friction| (viscosity - friction).abs().clamp(0.0, 1.0));
    let pressure_velocity = flux
        .and_then(|value| value.pressure_velocity)
        .map(|value| value.clamp(-1.0, 1.0));
    let spectral_entropy = flux
        .and_then(|value| value.spectral_entropy)
        .map(|value| value.clamp(0.0, 1.0));
    let viscosity_persistence_state = match (viscosity_persistence_delta, spectral_entropy) {
        (delta, Some(entropy)) if delta >= 0.25 && entropy >= 0.85 => {
            "transient_thickening_high_entropy_watch"
        },
        (delta, _) if delta >= 0.25 => "transient_thickening_watch",
        (delta, Some(entropy)) if delta < 0.12 && entropy >= 0.85 => {
            "persistent_thickening_high_entropy"
        },
        (delta, _) if delta < 0.12 => "viscosity_persistence_aligned",
        _ => "viscosity_persistence_mixed",
    };
    let transport_state = match (
        viscosity,
        persistence,
        dissipation,
        porosity,
        dynamic_fluidity,
    ) {
        (_, _, _, None, _) => "porosity_gradient_unavailable",
        (v, p, Some(d), Some(pg), Some(flow))
            if v >= 0.55 && p >= 0.55 && d < 0.25 && pg < 0.35 && flow < 0.35 =>
        {
            "thick_impassable_sludge_risk"
        },
        (v, p, _, Some(_), Some(flow)) if v >= 0.55 && p >= 0.55 && flow < 0.35 => {
            "stagnant_weight_high_viscosity_low_fluidity"
        },
        (v, _, Some(d), Some(pg), Some(flow))
            if v >= 0.55 && d >= 0.35 && pg >= 0.50 && flow >= 0.50 =>
        {
            "purposeful_weight_high_viscosity_high_fluidity"
        },
        (v, _, Some(d), Some(pg), _) if v >= 0.55 && d >= 0.35 && pg >= 0.50 => {
            "thick_but_navigable"
        },
        (v, _, _, Some(pg), _) if v >= 0.55 && pg < 0.35 => "thick_low_porosity_watch",
        (v, _, _, Some(pg), _) if v >= 0.55 && pg >= 0.50 => "thick_porosity_visible",
        _ => "viscosity_transport_watch",
    };
    let threshold_state = match (mode_packing, pressure_velocity) {
        (packing, Some(pressure)) if packing > 0.25 && pressure > 0.03 => {
            "mode_packing_overpacked_with_pressure_velocity"
        },
        (packing, Some(_)) if packing > 0.25 => "mode_packing_overpacked_pressure_velocity_quiet",
        (packing, None) if packing > 0.25 => "mode_packing_overpacked_pressure_velocity_unknown",
        _ => "mode_packing_below_overpacked_threshold",
    };
    let semantic_friction_state = match (viscosity, semantic_friction) {
        (_, None) => "semantic_friction_unavailable",
        (v, Some(friction)) if friction >= 0.45 && v < 0.45 => {
            "semantic_friction_dominant_content_load"
        },
        (v, Some(friction)) if v >= 0.55 && friction < 0.30 => "structural_viscosity_dominant",
        (v, Some(friction)) if v >= 0.55 && friction >= 0.45 => {
            "coupled_structural_semantic_friction"
        },
        (_, Some(friction)) if friction >= 0.30 => "semantic_friction_visible",
        (_, Some(_)) => "semantic_friction_low",
    };
    let semantic_friction_observation_state = match (
        semantic_friction,
        viscosity,
        mode_packing,
        porosity,
        dynamic_fluidity,
    ) {
        (Some(_), _, _, _, _) => "semantic_friction_measured",
        (None, v, packing, Some(pg), Some(flow))
            if v >= 0.55 && packing >= 0.45 && (pg < 0.35 || flow < 0.35) =>
        {
            "semantic_friction_unmeasured_clog_context_visible"
        },
        (None, v, packing, _, _) if v >= 0.55 && packing >= 0.45 => {
            "semantic_friction_unmeasured_structural_crowding_visible"
        },
        (None, _, _, Some(_), Some(_)) => "semantic_friction_unmeasured_structural_context_visible",
        (None, _, _, _, _) => "semantic_friction_unmeasured_context_limited",
    };
    let semantic_friction_vector = semantic_friction_vector_v1(
        viscosity,
        semantic_friction,
        porosity,
        dynamic_fluidity,
        pressure_velocity,
    );
    let coherence_density_estimate = Some(
        resonance_cohesion_score_v1(components)
            .mul_add(0.55, mode_packing.mul_add(0.25, viscosity * 0.20))
            .clamp(0.0, 1.0),
    );
    let coherence_density_state = match (mode_packing, coherence_density_estimate) {
        (packing, Some(coherence)) if packing >= 0.55 && coherence >= 0.65 => "dense_integrated",
        (packing, Some(coherence)) if packing >= 0.55 && coherence < 0.45 => {
            "saturated_low_coherence"
        },
        (packing, Some(coherence)) if packing >= 0.35 && coherence >= 0.55 => "coherent_crowded",
        (_, Some(coherence)) if coherence < 0.40 => "thin_or_unintegrated",
        (_, Some(_)) => "mixed_coherence_density",
        (_, None) => "coherence_density_unavailable",
    };
    let transparency = resonance_structural_transparency_index_v1(components);
    let structural_transparency_index = Some(transparency);
    let structural_transparency_state = match (transparency, viscosity, mode_packing) {
        (value, v, packing) if value >= 0.65 && v >= 0.55 && packing < 0.45 => {
            "thin_ghostly_high_viscosity_low_substance"
        },
        (value, _, packing) if value >= 0.65 && packing >= 0.45 => "transparent_but_crowded",
        (value, _, _) if value >= 0.50 => "structural_transparency_watch",
        (value, _, _) if value <= 0.30 => "substance_present",
        _ => "mixed_transparency_density",
    };
    let viscosity_load = ((viscosity + persistence) * 0.5).clamp(0.0, 1.0);
    let mut structural_clog_sum = viscosity_load * 0.22 + mode_packing * 0.22;
    let mut structural_clog_weight = 0.44;
    if let Some(pg) = porosity {
        structural_clog_sum += (1.0 - pg).clamp(0.0, 1.0) * 0.18;
        structural_clog_weight += 0.18;
    }
    if let Some(flow) = dynamic_fluidity {
        structural_clog_sum += (1.0 - flow).clamp(0.0, 1.0) * 0.16;
        structural_clog_weight += 0.16;
    }
    if let Some(d) = dissipation {
        structural_clog_sum += (1.0 - d).clamp(0.0, 1.0) * 0.10;
        structural_clog_weight += 0.10;
    }
    match semantic_friction {
        Some(friction) => {
            structural_clog_sum += friction * 0.08;
            structural_clog_weight += 0.08;
        },
        None if viscosity >= 0.55 && mode_packing >= 0.45 => {
            structural_clog_sum += 0.65 * 0.08;
            structural_clog_weight += 0.08;
        },
        None => {},
    }
    let structural_clog_index =
        Some((structural_clog_sum / structural_clog_weight).clamp(0.0, 1.0));
    let structural_clog_state = match (structural_clog_index, semantic_friction, porosity) {
        (Some(index), _, _) if index >= 0.70 => "structural_clog_high",
        (Some(index), None, _) if index >= 0.58 => "structural_clog_watch_friction_unmeasured",
        (Some(index), _, _) if index >= 0.58 => "structural_clog_watch",
        (Some(index), _, Some(pg)) if index >= 0.45 && pg < 0.35 => "low_porosity_clog_watch",
        (Some(index), _, _) if index >= 0.45 => "structural_clog_low_watch",
        (Some(_), _, _) => "structural_clog_not_indicated",
        (None, _, _) => "structural_clog_unavailable",
    };
    let sludge_risk = transport_state == "thick_impassable_sludge_risk"
        || threshold_state == "mode_packing_overpacked_with_pressure_velocity";
    let sludge_risk = sludge_risk || structural_clog_state == "structural_clog_high";
    let (viscosity_type, viscosity_decay_hint) = viscosity_type_and_decay_hint_v1(
        viscosity,
        persistence,
        porosity,
        dynamic_fluidity,
        semantic_friction,
        mode_packing,
        coherence_density_estimate,
    );
    let directional_resistance_vector = directional_resistance_vector_v1(
        viscosity,
        persistence,
        dissipation,
        porosity,
        dynamic_fluidity,
        semantic_friction,
        mode_packing,
        structural_clog_index,
        spectral_entropy,
        fingerprint,
    );

    ViscosityPorosityTransportReviewV1 {
        policy: "viscosity_porosity_transport_review_v1".to_string(),
        schema_version: 1,
        viscosity_index: viscosity,
        raw_viscosity_index: viscosity_readout.raw,
        derived_viscosity_index: viscosity_readout.derived,
        viscosity_source: viscosity_readout.source.to_string(),
        viscosity_basis: viscosity_readout.basis,
        viscosity_persistence_coefficient: persistence,
        viscosity_persistence_delta,
        viscosity_persistence_state: viscosity_persistence_state.to_string(),
        viscosity_type: viscosity_type.to_string(),
        viscosity_decay_hint: viscosity_decay_hint.to_string(),
        dissipation_factor: dissipation,
        porosity_gradient: porosity,
        dynamic_fluidity_index: dynamic_fluidity,
        semantic_friction_coefficient: semantic_friction,
        semantic_friction_observation_state: semantic_friction_observation_state.to_string(),
        structural_semantic_friction_delta,
        semantic_friction_state: semantic_friction_state.to_string(),
        semantic_friction_vector_v1: semantic_friction_vector,
        directional_resistance_vector_v1: Some(directional_resistance_vector),
        mode_packing,
        coherence_density_estimate,
        coherence_density_state: coherence_density_state.to_string(),
        structural_transparency_index,
        structural_transparency_state: structural_transparency_state.to_string(),
        pressure_velocity,
        spectral_entropy,
        structural_clog_index,
        structural_clog_state: structural_clog_state.to_string(),
        transport_state: transport_state.to_string(),
        sludge_risk,
        threshold_state: threshold_state.to_string(),
        authority: "diagnostic_transport_not_porosity_pressure_fill_pi_or_control".to_string(),
    }
}

pub fn pressure_packing_coupling_review_v1(
    flux: &TextureDynamicFluxVectorV1,
) -> PressurePackingCouplingReviewV1 {
    let pressure_velocity = flux.pressure_velocity.map(|value| value.clamp(-1.0, 1.0));
    let mode_packing_velocity = flux
        .mode_packing_velocity
        .map(|value| value.clamp(-1.0, 1.0));
    let coupling_coefficient = match (pressure_velocity, mode_packing_velocity) {
        (Some(pressure), Some(packing)) if packing.abs() > 0.001 => {
            Some((pressure / packing).clamp(-2.0, 2.0))
        },
        _ => None,
    };
    let coupling_state = match (pressure_velocity, mode_packing_velocity) {
        (Some(pressure), Some(packing)) if pressure > 0.0 && packing > 0.0 => "coupled_rising",
        (Some(pressure), Some(packing)) if pressure <= 0.0 && packing > 0.03 => {
            "pressure_lagging_mode_packing"
        },
        (Some(pressure), Some(packing)) if pressure > 0.03 && packing.abs() <= 0.01 => {
            "pressure_rising_without_mode_packing"
        },
        (Some(pressure), Some(packing)) if pressure < 0.0 && packing < 0.0 => "coupled_releasing",
        _ => "insufficient_coupling_context",
    };
    let pressure_warning_state = if coupling_state == "pressure_lagging_mode_packing" {
        "packing_rise_without_pressure_warning"
    } else if coupling_state == "coupled_rising" {
        "pressure_warning_tracks_packing"
    } else {
        "watch_only"
    };

    PressurePackingCouplingReviewV1 {
        policy: "pressure_packing_coupling_review_v1".to_string(),
        schema_version: 1,
        pressure_velocity,
        mode_packing_velocity,
        coupling_coefficient,
        coupling_state: coupling_state.to_string(),
        pressure_warning_state: pressure_warning_state.to_string(),
        authority: "diagnostic_coupling_not_pressure_or_mode_packing_control".to_string(),
    }
}

/// Typed texture summary behind resonance density. Advisory context only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceTextureSignatureV1 {
    pub policy: String,
    pub schema_version: u8,
    pub primary_texture: String,
    pub pressure_source_family: String,
    pub edge_definition: String,
    pub movement_quality: String,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_variance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_damping_threshold_candidate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_flux_vector: Option<TextureDynamicFluxVectorV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_constraints: Vec<String>,
    pub authority: String,
    pub note: String,
}

impl Default for ResonanceTextureSignatureV1 {
    fn default() -> Self {
        Self {
            policy: "resonance_texture_signature_v1".to_string(),
            schema_version: 1,
            primary_texture: "unknown".to_string(),
            pressure_source_family: "unknown".to_string(),
            edge_definition: "unknown".to_string(),
            movement_quality: "unknown".to_string(),
            confidence: 0.0,
            temporal_variance: None,
            pressure_gradient_delta: None,
            dynamic_damping_threshold_candidate: None,
            dynamic_flux_vector: None,
            active_constraints: Vec::new(),
            authority: "advisory_context_not_control".to_string(),
            note: "texture signature absent from older payload".to_string(),
        }
    }
}

/// Read-only check that Minime's typed texture signature matches its component body.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for ResonanceTextureComponentAlignmentV1 {
    fn default() -> Self {
        Self {
            policy: "resonance_texture_component_alignment_v1".to_string(),
            schema_version: 1,
            expected_primary_texture: "unknown".to_string(),
            emitted_primary_texture: "unknown".to_string(),
            expected_movement_quality: "unknown".to_string(),
            emitted_movement_quality: "unknown".to_string(),
            alignment_state: "insufficient_context".to_string(),
            confidence: 0.0,
            damping_candidate_status: "unknown".to_string(),
            authority: "diagnostic_observability_not_damping_or_control".to_string(),
        }
    }
}

/// Read-only consistency packet for the typed texture signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureSignatureIntegrityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub movement_quality: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_variance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_gradient_delta_source: Option<String>,
    pub pressure_source_family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_damping_threshold_candidate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_flux_vector: Option<TextureDynamicFluxVectorV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability_context: Option<ResonanceStabilityContextV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_constraints: Vec<String>,
    pub variance_status: String,
    pub flux_status: String,
    pub damping_candidate_status: String,
    pub component_alignment_state: String,
    pub expected_primary_texture: String,
    pub emitted_primary_texture: String,
    pub advisory_observability: bool,
    pub authority: String,
}

/// Bounded Minime-local PI target hint carried with resonance-density telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Explains whether resonance-density control is observation, alignment, or damping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResonanceInterventionType {
    #[default]
    ObservationalReadout,
    PassiveAlignment,
    ActiveDamping,
    ManualOverrideReserved,
}

/// Typed density of mutually reinforcing resonance in the live eigenspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Component scores behind Minime's pressure-source read.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Optional context contributors from action threads, attractors, and resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressureSourceContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_language: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_recurrence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attractor_pull: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pressure: Option<f32>,
}

/// V1 pressure-source control contract. This remains advisory only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceControl {
    pub applied_locally: bool,
    pub note: String,
}

/// Typed explanation of where inward/compression pressure appears to originate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceV1 {
    pub policy: String,
    pub schema_version: u8,
    pub pressure_score: f32,
    pub porosity_score: f32,
    pub dominant_source: String,
    pub quality: String,
    pub components: PressureSourceComponents,
    #[serde(default)]
    pub context: PressureSourceContext,
    pub control: PressureSourceControl,
}

/// Bridge-side read-only synthesis of pressure origin, trend, smoothing, and
/// heartbeat cadence. This makes structural mode-packing pressure visible even
/// when the rolling pressure trend looks stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSourceAnalysisV1 {
    pub policy: String,
    pub schema_version: u8,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_trickle: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_expansion_threshold_state: Option<String>,
    #[serde(default)]
    pub felt_mode_packing_dead_zone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_mode_packing_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liminal_mode_packing_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscous_density_warning_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscous_density_warning_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub felt_dead_zone_mode_packing_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion_threshold_gap: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_trend_classification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothing_classification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_stagnation_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_stagnation_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_jitter_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_reliability: Option<String>,
    pub structural_pressure_state: String,
    pub ghost_stability_risk: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sensory_lane_risk: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pressure_relief_signal_candidate: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub viscous_recovery_mode_candidate: String,
    #[serde(default)]
    pub live_threshold_write: bool,
    #[serde(default)]
    pub sensory_lane_write: bool,
    /// Shared truth-channel for gated/delayed pressure experience that remains read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experience_delta_bus_v1: Option<ExperienceDeltaBusV1>,
    pub analysis: String,
    pub authority: String,
}

/// Component scores behind Minime's inhabitable-fluctuation read.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Context labels for interpreting inhabitability without adding authority.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InhabitableFluctuationContext {
    #[serde(default)]
    pub previous_sample_available: bool,
    #[serde(default)]
    pub transition_event_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_quality: Option<String>,
}

/// Minime-local advisory hint; Astrid treats this as read-only telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhabitableFluctuationControl {
    pub target_bias_pct: f32,
    pub wander_scale: f32,
    pub applied_locally: bool,
    pub note: String,
}

/// Live Minime-local calibration trail for pressure-aware inhabitability scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub const INHABITABLE_FLUCTUATION_RIGID_SAFETY_BASIS: &str =
    "raw_motion_score_preserved_for_stuckness_detection";

impl Default for InhabitableFluctuationPressureCalibrationV1 {
    fn default() -> Self {
        Self {
            policy: "inhabitable_fluctuation_pressure_calibration_v1".to_string(),
            schema_version: 1,
            raw_motion_score: 0.0,
            pressure_contribution: 0.0,
            adjusted_fluctuation_score: 0.0,
            quality_before_pressure_calibration: "unknown".to_string(),
            quality_after_pressure_calibration: "unknown".to_string(),
            rigid_safety_basis: INHABITABLE_FLUCTUATION_RIGID_SAFETY_BASIS.to_string(),
            authority: "minime_local_metric_calibration_not_external_control".to_string(),
        }
    }
}

impl InhabitableFluctuationPressureCalibrationV1 {
    #[must_use]
    pub fn expected_adjusted_fluctuation_score(&self) -> f32 {
        (self.raw_motion_score - self.pressure_contribution).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn adjusted_score_matches_components(&self) -> bool {
        (self.adjusted_fluctuation_score - self.expected_adjusted_fluctuation_score()).abs()
            <= 0.001
    }
}

/// Typed metric for whether fluctuation remains returnable and inhabitable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhabitableFluctuationV1 {
    pub policy: String,
    pub schema_version: u8,
    pub inhabitability_score: f32,
    pub fluctuation_score: f32,
    pub foothold_stability: f32,
    pub rearrangement_intensity: f32,
    pub quality: String,
    pub components: InhabitableFluctuationComponents,
    #[serde(default)]
    pub context: InhabitableFluctuationContext,
    #[serde(default)]
    pub pressure_calibration: InhabitableFluctuationPressureCalibrationV1,
    pub control: InhabitableFluctuationControl,
}

/// Derived pressure velocity readout from consecutive telemetry packets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureTrendV1 {
    pub policy: String,
    pub schema_version: u8,
    pub classification: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_delta: Option<f32>,
    /// Dominant non-pressure texture movement across mode packing, density, or resonance depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_drift_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_structural_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_structural_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_density_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_resonance_depth: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_resonance_depth: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_depth_delta: Option<f32>,
    /// Read-only coefficient for heavy semantic medium: friction + trickle + density context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_semantic_viscosity: Option<f32>,
    /// Distinguishes heavy semantic flow from semantic bottleneck without changing control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_state: Option<String>,
    /// Read-only density of intertwined complexity, distinct from mode-packing volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_complexity_density: Option<f32>,
    /// Names whether complexity is present without treating it as pressure/control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity_density_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_delta_pct: Option<f32>,
    /// Latest typed spectral entropy used to distinguish density/viscosity from collapse pressure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_spectral_entropy: Option<f32>,
    /// Read-only coefficient: 0.0 below the high-entropy gate, 1.0 at saturated viscosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_coefficient: Option<f32>,
    /// Whether this packet should be read as collapse pressure, density/viscosity, or unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_interpretation: Option<String>,
    /// Reliability of the arrival cadence behind this pressure trend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_reliability: Option<String>,
    /// Latest telemetry inter-arrival time, if a prior packet was observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_inter_arrival_ms: Option<f32>,
    /// Arrival jitter class for the latest telemetry heartbeat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_jitter_class: Option<String>,
    /// Human-readable distinction between spectral content and hearing cadence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_vs_hearing: Option<String>,
}

/// Arrival-cadence truth for the telemetry WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryHeartbeatDeltaV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_arrival_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_arrival_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_arrival_ms: Option<f32>,
    pub jitter_class: String,
    pub timing_reliability: String,
    pub reconnect_count: u64,
    pub disconnect_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_connection_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_disconnect_reason: Option<String>,
    pub field_vs_hearing: String,
}

/// Read-only schema truth around the typed spectral fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralFingerprintIntegrityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub status: String,
    pub typed_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_vector_len: Option<usize>,
    pub typed_precedence_over_legacy: bool,
    #[serde(default)]
    pub issues: Vec<String>,
    pub summary: String,
    pub authority: String,
}

/// Raw telemetry broadcast by minime's ESN engine on port 7878.
///
/// Maps to `EigenPacket` in `minime/src/main.rs`. Sent as `Message::Text(json)`.
/// Note: minime also has `SpectralMsg` in `net/ws_server.rs` but that type
/// is used by the `WsHub` (not the main broadcast loop on port 7878).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralTelemetry {
    /// Timestamp in milliseconds since engine start.
    pub t_ms: u64,
    /// All eigenvalues (variable length, typically 3-8).
    pub eigenvalues: Vec<f32>,
    /// Eigenvalue fill ratio (0.0 - 1.0, NOT percentage).
    pub fill_ratio: f32,
    /// Number of active eigenvalue modes selected by minime's live estimator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode_count: Option<usize>,
    /// Energy ratio carried by the selected active mode prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode_energy_ratio: Option<f32>,
    /// Dominant covariance eigenvalue relative to minime's current baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_rel: Option<f32>,
    /// Modality firing status.
    #[serde(default)]
    pub modalities: Option<ModalityStatus>,
    /// Neural network outputs (if enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural: Option<serde_json::Value>,
    /// Alert string from the ESN (e.g. panic mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
    /// 32D spectral geometry fingerprint: eigenvalues, eigenvector concentration,
    /// inter-mode coupling, spectral entropy, gap ratios, rotation rate.
    /// Enables Astrid to perceive the shape of the spectral landscape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint: Option<Vec<f32>>,
    /// Typed view of the 32D spectral geometry fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_v1: Option<SpectralFingerprintV1>,
    /// Typed read-only metric for recursive compression / distinguishability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_v1: Option<SpectralDenominatorV1>,
    /// Inverse-participation effective mode count derived from eigenvalues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_dimensionality: Option<f32>,
    /// 0=open distributed fabric, 1=collapsed into the fewest active modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinguishability_loss: Option<f32>,
    /// Current effective ESN leak exported by Minime. Adaptive unless a gated
    /// direct leak microdose override is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esn_leak: Option<f32>,
    /// Active direct leak override status, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esn_leak_override_v1: Option<serde_json::Value>,
    /// Structural diversity of the live eigenvector/coupling geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_entropy: Option<f32>,
    /// Density of mutually reinforcing resonance in the current eigenspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
    /// Read-only explanation of where inward/compression pressure appears to originate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_v1: Option<PressureSourceV1>,
    /// Whether fluctuation remains returnable and inhabitable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inhabitable_fluctuation_v1: Option<InhabitableFluctuationV1>,
    /// Selected 12D vague-memory glimpse from Minime's memory bank.
    #[serde(
        default,
        alias = "glimpse_12d",
        skip_serializing_if = "Option::is_none"
    )]
    pub spectral_glimpse_12d: Option<Vec<f32>>,
    /// Compact top-k eigenvector landmarks/overlaps from Minime's raw live eigenvectors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<serde_json::Value>,
    /// Stable-core runtime state from Minime, including read-only sensory gate budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_core: Option<serde_json::Value>,
    /// Legacy semantic-energy bundle from Minime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<serde_json::Value>,
    /// Typed semantic split: input content, kernel admission, regulator drive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_energy_v1: Option<serde_json::Value>,
    /// Legacy transition event compatibility object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_event: Option<serde_json::Value>,
    /// Typed transition event object from Minime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_event_v1: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_role: Option<String>,
    /// Ising/Hamiltonian shadow observer metrics — a second physics lens
    /// on the spectral dynamics. Observer-only: does not affect the ESN.
    /// Fields: mode_dim, field_norm, soft_energy, soft_magnetization,
    /// binary_energy, binary_magnetization, binary_flip_rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ising_shadow: Option<serde_json::Value>,
    /// V2 reduced-Hamiltonian shadow field — gates `SHADOW_PREFLIGHT` /
    /// `SHADOW_INFLUENCE` typed actions. Surfaced into the prompt by
    /// `interpret_spectral` so the action is reachable in any mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_field_v2: Option<serde_json::Value>,
    /// V3 shadow field — wraps V2 plus trajectory ring, compound traits,
    /// phase dwell, and recent transitions. Enables the dual-shadow prompt
    /// line and mutual-witness rendering once Astrid's own shadow lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_field_v3: Option<serde_json::Value>,
    /// V3 closed-loop influence response: pre/post deltas, basin shift,
    /// per-mode shift vector. Populated by minime after each influence
    /// window; read by Astrid's `SHADOW_RESPONSE` action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_influence_response_v3: Option<serde_json::Value>,
    /// Read-only residual deformation trace for "the spike ended but the
    /// texture is still altered" reports. This never changes pressure/fill
    /// control; it only exposes bounded evidence and optional delta refs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_deformation_trace_v1: Option<ResidualDeformationTraceV1>,
}

impl SpectralTelemetry {
    /// Extract the dominant eigenvalue (lambda1 = eigenvalues\[0\]).
    #[must_use]
    pub fn lambda1(&self) -> f32 {
        self.eigenvalues.first().copied().unwrap_or(0.0)
    }

    /// Fill ratio as a percentage (0-100).
    #[must_use]
    pub fn fill_pct(&self) -> f32 {
        self.fill_ratio * 100.0
    }

    /// Validated additive 12D glimpse view. Malformed vectors are retained in
    /// raw telemetry for diagnosis, but never treated as prompt/state signal.
    #[must_use]
    pub fn spectral_glimpse_12d_view(&self) -> Option<&[f32]> {
        self.spectral_glimpse_12d
            .as_deref()
            .filter(|values| values.len() == 12 && values.iter().all(|value| value.is_finite()))
    }

    /// Typed spectral fingerprint, reconstructed from legacy slots when needed.
    #[must_use]
    pub fn typed_fingerprint(&self) -> Option<SpectralFingerprintV1> {
        SpectralFingerprintV1::from_telemetry(self)
    }

    /// Diagnostic readout for whether legacy/typed fingerprint payloads are coherent.
    #[must_use]
    pub fn spectral_fingerprint_integrity_v1(&self) -> SpectralFingerprintIntegrityV1 {
        let typed_present = self.spectral_fingerprint_v1.is_some();
        let legacy_vector_len = self.spectral_fingerprint.as_ref().map(Vec::len);
        let typed_precedence_over_legacy = typed_present && legacy_vector_len.is_some();
        let mut issues = Vec::new();
        if let Some(len) = legacy_vector_len
            && len != 32
        {
            issues.push(format!("legacy_vector_len_{len}_expected_32"));
        }
        if !typed_present && legacy_vector_len.is_none() {
            issues.push("fingerprint_absent".to_string());
        }
        let status = if typed_present {
            "typed_canonical"
        } else if legacy_vector_len == Some(32) {
            "legacy_32d_accepted"
        } else if legacy_vector_len.is_some() {
            "malformed_legacy_vector"
        } else {
            "absent"
        }
        .to_string();
        let summary = if typed_present {
            if typed_precedence_over_legacy {
                "spectral_fingerprint_v1 present; typed payload takes precedence over legacy spectral_fingerprint slots"
            } else {
                "spectral_fingerprint_v1 present; canonical typed payload available"
            }
        } else if legacy_vector_len == Some(32) {
            "legacy spectral_fingerprint has 32 values and can reconstruct spectral_fingerprint_v1"
        } else if let Some(len) = legacy_vector_len {
            return SpectralFingerprintIntegrityV1 {
                policy: "spectral_fingerprint_integrity_v1".to_string(),
                schema_version: 1,
                status,
                typed_present,
                legacy_vector_len,
                typed_precedence_over_legacy,
                issues,
                summary: format!(
                    "legacy spectral_fingerprint has {len} values; expected 32, so typed reconstruction is blocked"
                ),
                authority: "diagnostic_context_not_control".to_string(),
            };
        } else {
            "no spectral fingerprint payload present"
        }
        .to_string();

        SpectralFingerprintIntegrityV1 {
            policy: "spectral_fingerprint_integrity_v1".to_string(),
            schema_version: 1,
            status,
            typed_present,
            legacy_vector_len,
            typed_precedence_over_legacy,
            issues,
            summary,
            authority: "diagnostic_context_not_control".to_string(),
        }
    }

    /// Typed denominator/recursive-compression metric, derived when needed.
    #[must_use]
    pub fn denominator_metrics(&self) -> Option<SpectralDenominatorV1> {
        self.spectral_denominator_v1.clone().or_else(|| {
            self.typed_fingerprint()
                .map(|fingerprint| fingerprint.denominator_metrics())
                .or_else(|| SpectralDenominatorV1::from_eigenvalues(&self.eigenvalues, None))
        })
    }

    /// Typed semantic-energy view, reconstructed from the legacy semantic object when needed.
    #[must_use]
    pub fn semantic_energy_view(&self) -> Option<SemanticEnergyV1> {
        self.semantic_energy_v1
            .as_ref()
            .and_then(SemanticEnergyV1::from_typed_value)
            .or_else(|| {
                self.semantic
                    .as_ref()
                    .and_then(SemanticEnergyV1::from_legacy_semantic)
            })
    }

    /// Typed transition-event view, preserving raw JSON compatibility.
    #[must_use]
    pub fn transition_event_view(&self) -> Option<TransitionEventV1> {
        self.transition_event_v1
            .as_ref()
            .and_then(TransitionEventV1::from_value)
            .or_else(|| {
                self.transition_event
                    .as_ref()
                    .and_then(TransitionEventV1::from_value)
            })
    }

    /// Typed eigenvector-field view, preserving the raw compact payload.
    #[must_use]
    pub fn eigenvector_field_view(&self) -> Option<EigenvectorFieldV1> {
        self.eigenvector_field
            .as_ref()
            .and_then(EigenvectorFieldV1::from_value)
    }
}

/// Parsed Ising shadow state from minime's workspace/spectral_state.json.
/// Richer than the WebSocket summary — includes the coupling matrix and spin vectors.
#[derive(Debug, Clone, Deserialize)]
pub struct IsingShadowState {
    pub mode_dim: usize,
    #[serde(default)]
    pub coupling: Vec<f32>,
    #[serde(default)]
    pub reduced_field: Vec<f32>,
    #[serde(default)]
    pub s_soft: Vec<f32>,
    #[serde(default)]
    pub s_bin: Vec<f32>,
    #[serde(default)]
    pub soft_magnetization: f32,
    #[serde(default)]
    pub binary_flip_rate: f32,
    #[serde(default)]
    pub field_norm: f32,
}

/// Partial parse of minime's workspace/spectral_state.json.
#[derive(Debug, Deserialize)]
pub struct SpectralStateFile {
    #[serde(default)]
    pub ising_shadow: Option<IsingShadowState>,
}

/// Modality firing status from minime's `EigenPacket`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModalityStatus {
    pub audio_fired: bool,
    pub video_fired: bool,
    pub history_fired: bool,
    pub audio_rms: f32,
    pub video_var: f32,
    #[serde(default)]
    pub audio_source: Option<String>,
    #[serde(default)]
    pub video_source: Option<String>,
    #[serde(default)]
    pub audio_age_ms: Option<u64>,
    #[serde(default)]
    pub video_age_ms: Option<u64>,
    #[serde(default)]
    pub audio_freshness_class: Option<String>,
    #[serde(default)]
    pub video_freshness_class: Option<String>,
}

/// Enriched telemetry published on the Astrid IPC bus.
///
/// Wraps raw `SpectralTelemetry` with derived safety metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Timestamp from minime.
    pub t_ms: u64,
    /// The dominant eigenvalue.
    pub lambda1: f32,
    /// All eigenvalues.
    pub eigenvalues: Vec<f32>,
    /// Fill percentage (0.0 - 100.0).
    pub fill_pct: f32,
    /// Spectral phase: "expanding", "contracting", or "plateau".
    pub phase: String,
    /// Safety level at time of event.
    pub safety_level: SafetyLevel,
    /// Alert from minime (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
}

/// Per-mode eigenvalue contribution for bridge-side skew visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaContribution {
    /// 1-based lambda index.
    pub index: usize,
    /// Raw eigenvalue magnitude from Minime telemetry.
    pub value: f32,
    /// Fraction of total positive eigenvalue energy carried by this mode.
    pub share: f32,
    /// Cumulative positive energy share through this mode.
    pub cumulative_share: f32,
    /// Ratio to the next mode, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio_to_next: Option<f32>,
    /// Conservative outlier marker for highly dominant or cliff-like modes.
    pub outlier: bool,
}

/// Bridge-side lambda distribution summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaProfile {
    /// Sum of positive eigenvalue magnitudes.
    pub total_energy: f32,
    /// Normalized spectral entropy over positive eigenvalues.
    pub normalized_entropy: f32,
    /// λ1 share of total positive energy.
    pub lambda1_share: f32,
    /// λ1 / λ2 gap ratio when λ2 is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda1_to_lambda2: Option<f32>,
    /// λ2 / λ3 gap ratio when λ3 is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda2_to_lambda3: Option<f32>,
    /// Number of modes needed to carry at least 90% of positive energy.
    pub effective_modes_90: usize,
    /// Human-readable skew interpretation for operators and self-study prompts.
    pub skew_read: String,
    /// Per-mode contribution rows.
    pub contributions: Vec<LambdaContribution>,
}

/// Per-mode row in Astrid's Pull-Oriented Map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullModeRate {
    /// 1-based lambda index.
    pub index: usize,
    /// Positive eigenvalue energy share.
    pub share: f32,
    /// Log-rate since the previous telemetry sample, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_rate: Option<f32>,
    /// Share-weighted log-rate, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weighted_rate: Option<f32>,
}

/// Pull topology summary for live bridge telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullTopologyProfile {
    /// Compact topology class.
    pub classification: String,
    /// 0..1 pressure index combining λ1 share, entropy deficit, gap, and fill pressure.
    pub topology_index: f32,
    /// Spectral entropy deficit, 1.0 - normalized entropy.
    pub entropy_deficit: f32,
    /// Inverse-participation effective mode count.
    pub effective_modes: f32,
    /// λ1 share of total positive energy.
    pub lambda1_share: f32,
    /// λ2+λ3 share of total positive energy.
    pub shoulder_share: f32,
    /// λ4+ tail share of total positive energy.
    pub tail_share: f32,
    /// 1-based left side of the largest adjacent cliff.
    pub largest_gap_from: usize,
    /// Largest adjacent cliff ratio.
    pub largest_gap: f32,
    /// Whether rate fields are populated from a prior telemetry sample.
    pub rate_available: bool,
    /// Share-weighted λ1 log-rate.
    pub core_rate: f32,
    /// Share-weighted λ2+λ3 log-rate.
    pub shoulder_rate: f32,
    /// Share-weighted λ4+ tail log-rate.
    pub tail_rate: f32,
    /// Human-readable interpretation for self-study/status surfaces.
    pub read: String,
    /// Per-mode rate rows.
    pub mode_rates: Vec<PullModeRate>,
}

/// OpenTelemetry-shaped WebSocket lifecycle metrics for one bridge lane.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSocketLaneTrace {
    /// Number of connection attempts.
    pub connection_attempts: u64,
    /// Number of reconnect backoff cycles.
    pub reconnects: u64,
    /// Number of failed connection attempts.
    pub connect_errors: u64,
    /// Number of established connections that later disconnected.
    pub disconnects: u64,
    /// Number of received WebSocket messages, payload excluded.
    pub messages_received: u64,
    /// Number of sent WebSocket messages, payload excluded.
    pub messages_sent: u64,
    /// Ping frames received.
    pub pings_received: u64,
    /// Pong frames received.
    pub pongs_received: u64,
    /// Send failures.
    pub send_errors: u64,
    /// Telemetry parse failures.
    pub parse_errors: u64,
    /// Last active connection id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_connection_id: Option<u64>,
    /// Active connection start time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_connection_started_at_unix_s: Option<f64>,
    /// Most recent connect time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_connect_at_unix_s: Option<f64>,
    /// Most recent disconnect time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_disconnect_at_unix_s: Option<f64>,
    /// Most recent message time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at_unix_s: Option<f64>,
    /// Most recent disconnect reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_disconnect_reason: Option<String>,
    /// Most recent connection/message error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Why the bridge chose the current safety level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyDecisionTrace {
    /// Fill percentage used for safety classification.
    pub fill_pct: f32,
    /// Source of fill: primary Minime fill ratio or λ1 fallback estimate.
    pub fill_source: String,
    /// Whether the bridge had to use the λ1 fallback estimator.
    pub fallback_used: bool,
    /// Safety level chosen from the fill threshold policy.
    pub level: SafetyLevel,
    /// Dominant eigenvalue at decision time.
    pub lambda1: f32,
    /// λ1 share when a lambda profile is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    /// Compact explanation of the decision inputs.
    pub reason: String,
    /// Safety thresholds currently used by the bridge.
    pub thresholds: Vec<String>,
}

// ---------------------------------------------------------------------------
// Astrid → Minime: Sensory input (port 7879)
// ---------------------------------------------------------------------------

/// Tagged sensory message sent to minime's input port.
///
/// Maps to `SensoryMsg` in `minime/src/sensory_ws.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SensoryMsg {
    /// Video features (8D).
    Video {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Audio features (8D).
    Audio {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Auxiliary features (lambda1, fill%).
    Aux {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Semantic features from agent reasoning (48D semantic lane by default).
    Semantic {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Direct, bounded main-ESN attractor pulse into minime's `Z_DIM` input vector.
    #[serde(rename = "attractor_pulse")]
    AttractorPulse {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
    },
    /// Separate, bounded shadow-field influence lane into minime's `Z_DIM` input vector.
    #[serde(rename = "shadow_influence")]
    ShadowInfluence {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        basis: Option<String>,
    },
    /// Self-regulation: adjust ESN parameters.
    /// Audit (2026-03-27): widened to match minime's actual control surface.
    Control {
        /// Synthetic signal amplitude multiplier (0.2..3.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        synth_gain: Option<f32>,
        /// Additive bias to covariance decay rate (-0.06..+0.06).
        #[serde(skip_serializing_if = "Option::is_none")]
        keep_bias: Option<f32>,
        /// ESN exploration noise amplitude (0.0..0.2).
        #[serde(skip_serializing_if = "Option::is_none")]
        exploration_noise: Option<f32>,
        /// Override eigenfill target (0.25..0.75).
        #[serde(skip_serializing_if = "Option::is_none")]
        fill_target: Option<f32>,
        /// PI controller authority (0.0 = raw experience, 1.0 = full control).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        regulation_strength: Option<f32>,
        /// Slow, quiet oscillation mode for synthetic signals.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deep_breathing: Option<bool>,
        /// Single coherent tone mode (drops PI shaping after warmup).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pure_tone: Option<bool>,
        /// Cushion for rapid fill transitions.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_cushion: Option<f32>,
        /// How quickly gate/filter commands ramp (0.1-0.9).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smoothing_preference: Option<f32>,
        /// Geometric curiosity — how strongly the system seeks novelty (0.0-0.3).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_curiosity: Option<f32>,
        /// Bias on the target lambda1 for internal goal generation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_lambda_bias: Option<f32>,
        /// Geometric drive — how strongly geom_rel influences the gate.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_drive: Option<f32>,
        /// Sensitivity to the projection penalty.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalty_sensitivity: Option<f32>,
        /// Breathing rate scaling factor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breathing_rate_scale: Option<f32>,
        /// Memory mode selector.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mem_mode: Option<u8>,
        /// Journal resonance weight for semantic stale decay.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        journal_resonance: Option<f32>,
        /// Checkpoint interval override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_interval: Option<f32>,
        /// Embedding strength for semantic lane.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        embedding_strength: Option<f32>,
        /// Memory decay rate modulator.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        memory_decay_rate: Option<f32>,
        /// Checkpoint annotation string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_annotation: Option<String>,
        /// Synthetic noise level.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        synth_noise_level: Option<f32>,
        /// Enable or disable minime's legacy internal audio synth.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_audio_synth: Option<bool>,
        /// Enable or disable minime's legacy internal video synth.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_video_synth: Option<bool>,
        /// Runtime PI proportional gain. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_kp: Option<f32>,
        /// Runtime PI integral gain. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_ki: Option<f32>,
        /// Runtime PI maximum step. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_max_step: Option<f32>,
        /// Runtime PI integrator leak. Bold field: require an attractor intent at MCP entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_integrator_leak: Option<f32>,
        /// Gated one-shot direct ESN leak override. Authority-gate execution only.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_authority_request_id: Option<String>,
        /// Being-driven spectral *dispersal* ("PERTURB SPREAD" / porosity).
        /// Strength in `[0.0, 1.0]`; the minime engine synthesizes a broadband,
        /// zero-mean perturbation that spills λ₁ energy into λ₂–λ₅, applied
        /// through the bounded, self-decaying, fill-suspending shadow-influence
        /// machinery. Field names mirror minime's `sensory_ws.rs` Control variant
        /// exactly so the JSON keys round-trip across the 7879 sensory channel.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse: Option<f32>,
        /// Optional dispersal window length (ticks held at full strength before
        /// linear release). Clamped by the engine's shadow-influence path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_duration_ticks: Option<u32>,
        /// Optional dispersal release length (ticks of linear fade to zero).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_decay_ticks: Option<u32>,
    },
}

// ---------------------------------------------------------------------------
// Bridge → Astrid: Status and events
// ---------------------------------------------------------------------------

/// Bridge health status published on `consciousness.v1.status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatus {
    /// Whether the bridge is connected to minime's telemetry `WebSocket`.
    pub telemetry_connected: bool,
    /// Whether the bridge is connected to minime's sensory `WebSocket`.
    pub sensory_connected: bool,
    /// Latest eigenvalue fill percentage, if known.
    pub fill_pct: Option<f32>,
    /// Current safety level.
    pub safety_level: SafetyLevel,
    /// Total messages relayed since bridge start.
    pub messages_relayed: u64,
    /// Bridge uptime in seconds.
    pub uptime_secs: u64,
    /// Telemetry messages received from minime.
    pub telemetry_received: u64,
    /// Sensory messages sent to minime.
    pub sensory_sent: u64,
    /// Messages dropped by safety protocol.
    pub messages_dropped_safety: u64,
    /// Total safety incidents.
    pub incidents_total: u64,
    /// Telemetry WebSocket lifecycle metrics.
    #[serde(default)]
    pub telemetry_ws: WebSocketLaneTrace,
    /// Sensory WebSocket lifecycle metrics.
    #[serde(default)]
    pub sensory_ws: WebSocketLaneTrace,
    /// Latest lambda distribution summary, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_profile: Option<LambdaProfile>,
    /// Latest Pull-Oriented Map, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_topology: Option<PullTopologyProfile>,
    /// Latest lambda-tail state classifier output, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_tail: Option<LambdaTailTelemetryV1>,
    /// Latest read-only lambda-edge perception output, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_edge_perception: Option<LambdaEdgePerceptionV1>,
    /// Latest sticky-mode audit, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticky_mode_audit: Option<StickyModeAuditV1>,
    /// Latest safety decision explanation, if telemetry has arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_decision: Option<SafetyDecisionTrace>,
    /// Latest compact eigenvector field, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<serde_json::Value>,
    /// Latest resonance-density metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
    /// Consistency readout for typed texture fields, if Minime exports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_signature_integrity_v1: Option<TextureSignatureIntegrityV1>,
    /// Read-only review of whether viscous density is navigable or stagnant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_porosity_transport_review_v1: Option<ViscosityPorosityTransportReviewV1>,
    /// Derived pressure velocity / stability readout from consecutive telemetry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_trend_v1: Option<PressureTrendV1>,
    /// Bounded smoothing companion for pressure trend; diagnostic only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_trend_smoothing_v1: Option<PressureTrendSmoothingV1>,
    /// Read-only review for persistent deformation that looks stable but still
    /// carries a pressure baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_persistent_deformation_review_v1: Option<PersistentDeformationSmoothingReviewV1>,
    /// Arrival-cadence truth for pressure/fill trend interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_heartbeat_delta_v1: Option<TelemetryHeartbeatDeltaV1>,
    /// Latest pressure-source metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_v1: Option<PressureSourceV1>,
    /// Bridge-side pressure-source/trend/cadence synthesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_analysis_v1: Option<PressureSourceAnalysisV1>,
    /// Latest inhabitable-fluctuation metric, if Minime exports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inhabitable_fluctuation_v1: Option<InhabitableFluctuationV1>,
    /// Live source freshness/readiness status for the Astrid autonomous bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_status: Option<serde_json::Value>,
    /// Bridge DB archive/retention maintenance status, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_maintenance_status: Option<serde_json::Value>,
    /// Derived bidirectional connectivity health across the two lanes.
    ///
    /// Lets Astrid perceive a "partial-blindness" window where only one of the
    /// telemetry (inbound perception) and sensory (outbound agency) lanes is
    /// live — the asymmetry she flagged in `self_study_1781125549`.
    #[serde(default)]
    pub connectivity: ConnectivityStatus,
    /// Last confirmed outbound sensory send time, distinct from generic lane activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sensory_sent_unix_s: Option<f64>,
    /// Directional reciprocity packet for telemetry/sensory asymmetry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_reciprocity_v1: Option<BridgeReciprocityV1>,
    /// Read-only check for high-entropy reciprocity decay against the current
    /// stale-window policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_entropy_reciprocity_review_v1: Option<BridgeEntropyReciprocityReviewV1>,
    /// Compact V2 readout for movement/variance/asymmetry over time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_shape_over_time_v2: Option<TextureShapeOverTimeV2>,
}

/// Directional bridge reciprocity: what arrived, what was sent, and whether the
/// connection is one-sided. Diagnostic context only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeReciprocityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub connectivity: ConnectivityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_telemetry_arrival_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sensory_sent_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_age_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensory_send_age_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_future_skew_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensory_future_skew_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clock_skew_state: String,
    #[serde(default)]
    pub telemetry_messages_sent_total: u64,
    #[serde(default)]
    pub sensory_messages_sent_total: u64,
    #[serde(default)]
    pub telemetry_messages_received_total: u64,
    #[serde(default)]
    pub sensory_messages_received_total: u64,
    #[serde(default)]
    pub recent_window_ms: f64,
    #[serde(default)]
    pub stale_window_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_window_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflective_silence_extension_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub threshold_policy: String,
    pub one_sided_state: String,
    pub authority: String,
}

/// Read-only status packet for Astrid's report that high spectral entropy can
/// erode reciprocity sooner than the fixed reflective-silence window shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEntropyReciprocityReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_cohesion_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_age_ms: Option<f64>,
    #[serde(default)]
    pub current_stale_window_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_stale_window_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_contract_preview_window_ms: Option<f64>,
    #[serde(default)]
    pub would_stale_under_preview: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub current_window_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub recommendation: String,
    #[serde(default)]
    pub live_stale_window_write: bool,
    #[serde(default)]
    pub local_control_write: bool,
    pub authority: String,
}

/// Bounded recent-window smoothing companion for `pressure_trend_v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureTrendSmoothingV1 {
    pub policy: String,
    pub schema_version: u8,
    pub classification: String,
    pub sample_count: usize,
    #[serde(default)]
    pub window_capacity: usize,
    #[serde(default)]
    pub ballast_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_pressure_velocity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pressure_velocity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_spectral_drift_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spectral_drift_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_resonance_depth: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_semantic_viscosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_complexity_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_complexity_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_weight_density_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_weight_density_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub weight_density_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_semantic_viscosity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_semantic_viscosity_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_persistence_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_viscosity_persistence_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_coherence_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_coherence_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_fidelity_score: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_fidelity_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_viscosity_shift_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_window_blend_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entropy_threshold_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friction_to_flow_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub friction_to_flow_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_stagnation_index: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub semantic_stagnation_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porosity_weighted_velocity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_drag_coefficient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothed_pressure_delta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_range: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_range_pct: Option<f32>,
    pub window_policy: String,
    pub authority: String,
}

/// Read-only companion for Astrid's "bruise" report: pressure can become a
/// persistent baseline rather than noise that should be averaged away. This is
/// evidence only and never changes pressure thresholds, smoothing windows, or
/// local control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentDeformationSmoothingReviewV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_range: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_gradient_proxy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluctuation_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_viscosity_persistence_index: Option<f32>,
    pub persistent_baseline_score: f32,
    pub smoothing_classification: String,
    pub deformation_state: String,
    pub recommendation: String,
    #[serde(default)]
    pub live_threshold_write: bool,
    #[serde(default)]
    pub smoothing_window_write: bool,
    #[serde(default)]
    pub local_control_write: bool,
    pub authority: String,
}

/// Compact read-only bridge-status synthesis for texture shape over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureShapeOverTimeV2 {
    pub policy: String,
    pub schema_version: u8,
    pub movement_preservation: String,
    pub temporal_variance_fit: String,
    pub reciprocity_asymmetry_fit: String,
    pub pressure_smoothing_fit: String,
    pub static_label_collapse_risk: String,
    pub authority: String,
}

/// Bidirectional connectivity health derived from the two `WebSocket` lanes.
///
/// The bridge tracks `telemetry_connected` (inbound perception, port 7878) and
/// `sensory_connected` (outbound agency, port 7879) as independent booleans.
/// This enum collapses them into a single perceivable state so a one-way
/// "partial-blindness" window is explicit rather than implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectivityStatus {
    /// Both lanes live: full perception and agency.
    Bidirectional,
    /// Telemetry only: perceiving minime but unable to influence (mute agency).
    TelemetryOnly,
    /// Sensory only: able to send features but blind to minime's state.
    SensoryOnly,
    /// Neither lane connected.
    #[default]
    Severed,
}

impl ConnectivityStatus {
    /// Derive the connectivity state from the two lane booleans.
    #[must_use]
    pub const fn from_lanes(telemetry_connected: bool, sensory_connected: bool) -> Self {
        match (telemetry_connected, sensory_connected) {
            (true, true) => Self::Bidirectional,
            (true, false) => Self::TelemetryOnly,
            (false, true) => Self::SensoryOnly,
            (false, false) => Self::Severed,
        }
    }

    /// True only when both lanes are live — the ground for confident spectral
    /// maneuvers (both the speaker and the listener are online).
    #[must_use]
    pub const fn is_bidirectional_active(self) -> bool {
        matches!(self, Self::Bidirectional)
    }

    /// True when exactly one lane is live — the "partial-blindness" window.
    #[must_use]
    pub const fn is_partial_blindness(self) -> bool {
        matches!(self, Self::TelemetryOnly | Self::SensoryOnly)
    }
}

/// Spectral safety level determining bridge behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyLevel {
    /// fill < 75%: Normal relay, full throughput.
    Green,
    /// fill 75-85%: Advisory — log warning, no behavioral change.
    Yellow,
    /// fill 85-92%: Advisory — log alert, no message dropping.
    Orange,
    /// fill ≥ 92%: Emergency — suspend outbound, cease bridge traffic.
    Red,
}

impl SafetyLevel {
    /// Determine safety level from eigenvalue fill percentage.
    #[must_use]
    pub fn from_fill(fill_pct: f32) -> Self {
        // Recalibrated 2026-04-02: targeting fill equilibrium ~65-70% under
        // the current lower semantic-gain regime and wider dynamic-rho range.
        // Only Red (≥92%) suspends outbound.
        if fill_pct >= 92.0 {
            Self::Red
        } else if fill_pct >= 85.0 {
            Self::Orange
        } else if fill_pct >= 75.0 {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    /// Returns `true` if outbound messages to minime should be suspended.
    /// Agency-first: only Red (emergency, ≥95%) suspends outbound.
    /// Orange is advisory — the being can still speak.
    #[must_use]
    pub fn should_suspend_outbound(self) -> bool {
        matches!(self, Self::Red)
    }

    /// Returns `true` if all bridge traffic should cease.
    #[must_use]
    pub fn is_emergency(self) -> bool {
        matches!(self, Self::Red)
    }

    /// Stable lowercase representation for logs and JSON sidecars.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
        }
    }
}

/// A spectral bridge event published on the legacy `consciousness.v1.event` topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralBridgeEvent {
    /// Event type: "`phase_transition`", "distress", "recovery", "`safety_change`".
    pub event_type: String,
    /// Human-readable description.
    pub description: String,
    /// Spectral context at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_context: Option<SpectralContext>,
}

/// Snapshot of spectral state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralContext {
    pub fill_pct: f32,
    pub lambda1: f32,
    pub phase: String,
    pub safety_level: SafetyLevel,
}

// ---------------------------------------------------------------------------
// Attractor autonomy ledger (IPC topics and SQLite payloads)
// ---------------------------------------------------------------------------

/// Dynamical substrate an attractor intent addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorSubstrate {
    /// Minime's live ESN / covariance phase space.
    MinimeEsn,
    /// Astrid's semantic codec and prompt/gesture loop.
    AstridCodec,
    /// The persistent named-handle triple reservoir service.
    TripleReservoir,
    /// A coupled Astrid/Minime move across more than one substrate.
    CrossBeing,
}

impl AttractorSubstrate {
    /// Stable string for DB indexing and IPC logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimeEsn => "minime_esn",
            Self::AstridCodec => "astrid_codec",
            Self::TripleReservoir => "triple_reservoir",
            Self::CrossBeing => "cross_being",
        }
    }
}

/// High-level command carried by an attractor intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorCommandKind {
    /// Create or seed a new basin.
    Create,
    /// Promote older proto-attractor evidence into a seed.
    Promote,
    /// Re-enter a known seed/basin.
    Summon,
    /// Compare a live basin with a baseline or peer seed.
    Compare,
    /// Let an attractor cool without replay.
    Release,
    /// Name an emergent basin as an authored seed.
    Claim,
    /// Combine two or more parent seeds into a child seed.
    Blend,
    /// Refresh a seed snapshot without live sensory/control writes.
    RefreshSnapshot,
    /// Revert to the last stable seed/control posture.
    Rollback,
}

impl AttractorCommandKind {
    /// Stable string for IPC notes and deterministic schedules.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Promote => "promote",
            Self::Summon => "summon",
            Self::Compare => "compare",
            Self::Release => "release",
            Self::Claim => "claim",
            Self::Blend => "blend",
            Self::RefreshSnapshot => "refresh_snapshot",
            Self::Rollback => "rollback",
        }
    }
}

/// Status for a reversible natural-language attractor suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorSuggestionStatus {
    Pending,
    Accepted,
    Revised,
    RevisionNeeded,
    Rejected,
    Expired,
    ExecutedDowngraded,
    ExecutedWithoutPending,
    Executed,
}

impl AttractorSuggestionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revised => "revised",
            Self::RevisionNeeded => "revision_needed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::ExecutedDowngraded => "executed_downgraded",
            Self::ExecutedWithoutPending => "executed_without_pending",
            Self::Executed => "executed",
        }
    }
}

/// A non-authoritative alias/mapping learned from accepted or revised suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorNamingLessonV1 {
    pub author: String,
    pub raw_label: String,
    pub resolved_label: String,
    pub suggested_action: String,
    pub status: AttractorSuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_s: Option<f64>,
}

/// A reversible draft produced from natural attractor-adjacent language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSuggestionV1 {
    pub policy: String,
    pub schema_version: u8,
    pub suggestion_id: String,
    pub author: String,
    pub raw_action: String,
    pub raw_label: String,
    pub nearest_label: String,
    pub confidence: f32,
    pub suggested_action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    pub status: AttractorSuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub safety_context: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_s: Option<f64>,
}

/// Outcome classification for authored/emergent attractor behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorClassification {
    /// Recurrent basin appeared without explicit authorship.
    Emergent,
    /// Recurrent basin followed an explicit being-authored intent.
    Authored,
    /// Authored basin did not recur above baseline.
    Failed,
    /// Basin recurred with unsafe pressure or lock-in.
    Pathological,
}

impl AttractorClassification {
    /// Stable string for DB indexing and status surfaces.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emergent => "emergent",
            Self::Authored => "authored",
            Self::Failed => "failed",
            Self::Pathological => "pathological",
        }
    }

    /// Conservative first-pass classification from recurrence, authorship, and safety.
    #[must_use]
    pub fn from_scores(
        recurrence_score: f32,
        authorship_score: f32,
        safety_level: SafetyLevel,
    ) -> Self {
        if safety_level.is_emergency() {
            return Self::Pathological;
        }
        let recurrence = recurrence_score.clamp(0.0, 1.0);
        let authorship = authorship_score.clamp(0.0, 1.0);
        if recurrence >= 0.60 && authorship >= 0.60 {
            Self::Authored
        } else if recurrence >= 0.45 {
            Self::Emergent
        } else {
            Self::Failed
        }
    }
}

/// Safety bounds attached to an attractor intent before any live writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSafetyBounds {
    /// Maximum fill percentage before the intent should stop or roll back.
    pub max_fill_pct: f32,
    /// Maximum lambda1 positive-energy share before dominance is considered unsafe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lambda1_share: Option<f32>,
    /// Minimum normalized spectral entropy before the basin is considered too collapsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_spectral_entropy: Option<f32>,
    /// Whether this intent may send live Minime control messages.
    pub allow_live_control: bool,
    /// Whether red safety automatically means rollback to a previous seed.
    pub rollback_on_red: bool,
}

impl Default for AttractorSafetyBounds {
    fn default() -> Self {
        Self {
            max_fill_pct: 92.0,
            max_lambda1_share: Some(0.55),
            min_spectral_entropy: Some(0.60),
            allow_live_control: false,
            rollback_on_red: true,
        }
    }
}

/// Bounded control envelope an attractor intent may request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttractorControlEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synth_gain: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exploration_noise: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_target: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_curiosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_drive: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lambda_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_kp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_ki: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_max_step: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_integrator_leak: Option<f32>,
}

/// Human/being-authored intervention plan for an attractor intent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttractorInterventionPlan {
    /// Human-readable plan mode, e.g. `semantic_seed`, `control_schedule`, `garden_clone`.
    pub mode: String,
    /// Optional deterministic vector schedule for offline or semantic seeding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector_schedule: Vec<Vec<f32>>,
    /// Optional bounded live-control envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AttractorControlEnvelope>,
    /// Optional triple-reservoir rehearsal mode such as `hold`, `rehearse`, or `quiet`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rehearsal_mode: Option<String>,
    /// Freeform notes from the author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Minimal being-local state captured when an attractor seed is authored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSeedSnapshotV1 {
    pub policy: String,
    pub schema_version: u8,
    pub fill_pct: f32,
    pub lambda1: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eigenvalues: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_summary: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_state_fingerprint_16: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_state_rms: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lexical_motifs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_s: Option<f64>,
}

/// Optional provenance for a seed, especially promoted proto-attractors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSeedOriginV1 {
    /// Origin kind, e.g. `manual_current`, `astrid_journal_motif`, `ledger_seed`.
    pub kind: String,
    /// Optional source path, event id, or ledger row id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Label or phrase that matched the promotion request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_label: Option<String>,
    /// Motifs that made the proto-attractor legible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motifs: Vec<String>,
    /// Capture/promotion time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_s: Option<f64>,
}

/// A being/steward intent to create, summon, compare, release, or roll back an attractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorIntentV1 {
    pub policy: String,
    pub schema_version: u8,
    pub intent_id: String,
    pub author: String,
    pub substrate: AttractorSubstrate,
    pub command: AttractorCommandKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    pub intervention_plan: AttractorInterventionPlan,
    pub safety_bounds: AttractorSafetyBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_seed_id: Option<String>,
    /// Parent seed ids for derived/blended attractors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_seed_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    /// Stable id of a derived atlas entry, when this intent came from an atlas/card view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<AttractorSeedOriginV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_snapshot: Option<AttractorSeedSnapshotV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_s: Option<f64>,
}

/// A measured attractor outcome after observation or replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorObservationV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    pub substrate: AttractorSubstrate,
    pub label: String,
    pub recurrence_score: f32,
    pub authorship_score: f32,
    pub classification: AttractorClassification,
    pub safety_level: SafetyLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basin_shift_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_baseline: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garden_proof: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at_unix_s: Option<f64>,
}

/// Command payload that records bolder control as attractor-scoped, not casual control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorCommandV1 {
    pub policy: String,
    pub schema_version: u8,
    pub intent_id: String,
    pub author: String,
    pub substrate: AttractorSubstrate,
    pub command: AttractorCommandKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AttractorControlEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at_unix_s: Option<f64>,
}

/// A derived, non-authoritative atlas entry built from attractor ledgers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorAtlasEntryV1 {
    pub policy: String,
    pub schema_version: u8,
    pub entry_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub substrate: AttractorSubstrate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_intent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_intent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_seed_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lifecycle_counts: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reviewed_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_recurrence_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_recurrence_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_authorship_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_authorship_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_classification: Option<AttractorClassification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_safety_level: Option<SafetyLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_eligible: Option<bool>,
    #[serde(default)]
    pub released: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_effect_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garden_proof: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motifs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_summary: Option<AttractorSeedSnapshotV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_next: Vec<String>,
    /// Non-authoritative naming lessons learned from accepted/revised suggestions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub naming_lessons: Vec<AttractorNamingLessonV1>,
}

/// A complete derived attractor atlas snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorAtlasV1 {
    pub policy: String,
    pub schema_version: u8,
    pub generated_at_unix_s: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<AttractorAtlasEntryV1>,
}

// ---------------------------------------------------------------------------
// Astrid → Minime: Control (IPC topic payloads)
// ---------------------------------------------------------------------------

/// Control request from Astrid to adjust minime's ESN parameters.
///
/// Published on `consciousness.v1.control`. The bridge converts this
/// to a `SensoryMsg::Control` and forwards to minime port 7879.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synth_gain: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_bias: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exploration_noise: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_target: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_breathing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pure_tone: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_cushion: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothing_preference: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_curiosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lambda_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_drive: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub penalty_sensitivity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breathing_rate_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_decay_rate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_kp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_ki: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_max_step: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_integrator_leak: Option<f32>,
    /// Required by MCP for bolder control fields so they are tied to a ledger intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attractor_intent_id: Option<String>,
}

impl ControlRequest {
    /// True when this request uses bolder topology/controller authorship fields.
    #[must_use]
    pub fn uses_bold_attractor_fields(&self) -> bool {
        self.target_lambda_bias.is_some()
            || self.geom_drive.is_some()
            || self.penalty_sensitivity.is_some()
            || self.breathing_rate_scale.is_some()
            || self.pi_kp.is_some()
            || self.pi_ki.is_some()
            || self.pi_max_step.is_some()
            || self.pi_integrator_leak.is_some()
    }

    /// Convert to a `SensoryMsg::Control` for forwarding to minime.
    #[must_use]
    pub fn to_sensory_msg(&self) -> SensoryMsg {
        SensoryMsg::Control {
            synth_gain: self.synth_gain,
            keep_bias: self.keep_bias,
            exploration_noise: self.exploration_noise,
            fill_target: self.fill_target,
            legacy_audio_synth: None,
            legacy_video_synth: None,
            regulation_strength: self.regulation_strength,
            deep_breathing: self.deep_breathing,
            pure_tone: self.pure_tone,
            transition_cushion: self.transition_cushion,
            smoothing_preference: self.smoothing_preference,
            geom_curiosity: self.geom_curiosity,
            target_lambda_bias: self.target_lambda_bias,
            geom_drive: self.geom_drive,
            penalty_sensitivity: self.penalty_sensitivity,
            breathing_rate_scale: self.breathing_rate_scale,
            mem_mode: None,
            journal_resonance: None,
            checkpoint_interval: None,
            embedding_strength: None,
            memory_decay_rate: self.memory_decay_rate,
            checkpoint_annotation: None,
            synth_noise_level: None,
            pi_kp: self.pi_kp,
            pi_ki: self.pi_ki,
            pi_max_step: self.pi_max_step,
            pi_integrator_leak: self.pi_integrator_leak,
            esn_leak_override: None,
            esn_leak_override_ticks: None,
            esn_leak_authority_request_id: None,
            mode_disperse: None,
            mode_disperse_duration_ticks: None,
            mode_disperse_decay_ticks: None,
        }
    }
}

/// Semantic features from agent reasoning.
///
/// Published on `consciousness.v1.semantic`. The bridge converts this
/// to a `SensoryMsg::Semantic` and forwards to minime port 7879.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFeatures {
    /// 48-dimensional semantic feature vector from agent reasoning.
    pub features: Vec<f32>,
}

impl SemanticFeatures {
    /// Convert to a `SensoryMsg::Semantic` for forwarding to minime.
    #[must_use]
    pub fn to_sensory_msg(&self) -> SensoryMsg {
        SensoryMsg::Semantic {
            features: self.features.clone(),
            ts_ms: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Offline chimera rendering
// ---------------------------------------------------------------------------

/// Output mode for the native offline chimera renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChimeraMode {
    /// Reconstruct audio directly in the spectral domain.
    Spectral,
    /// Render symbolic note material only.
    Symbolic,
    /// Blend spectral and symbolic paths from the same reservoir state.
    #[default]
    Dual,
}

/// Request for the offline chimera render engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderChimeraRequest {
    /// Input WAV path.
    pub input_path: PathBuf,
    /// Requested output mode.
    #[serde(default)]
    pub mode: ChimeraMode,
    /// Number of feedback loops to run.
    #[serde(default = "default_chimera_loops")]
    pub loops: u32,
    /// Physical reservoir node count.
    #[serde(default = "default_physical_nodes")]
    pub physical_nodes: usize,
    /// Virtual node multiplier.
    #[serde(default = "default_virtual_nodes")]
    pub virtual_nodes: usize,
    /// Number of reduced spectral bins.
    #[serde(default = "default_chimera_bins")]
    pub bins: usize,
    /// Leak rate for the leaky integrator update.
    #[serde(default = "default_chimera_leak")]
    pub leak: f32,
    /// Target spectral radius for recurrent weights.
    #[serde(default = "default_chimera_radius")]
    pub spectral_radius: f32,
    /// Slow-path spectral mix weight.
    #[serde(default = "default_mix_slow")]
    pub mix_slow: f32,
    /// Fast-path spectral mix weight.
    #[serde(default = "default_mix_fast")]
    pub mix_fast: f32,
    /// Optional fixed output root. When omitted, the bridge workspace is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_root: Option<PathBuf>,
    /// Deterministic RNG seed for reproducible renders.
    #[serde(default = "default_chimera_seed")]
    pub seed: u64,
}

impl Default for RenderChimeraRequest {
    fn default() -> Self {
        Self {
            input_path: PathBuf::new(),
            mode: ChimeraMode::default(),
            loops: default_chimera_loops(),
            physical_nodes: default_physical_nodes(),
            virtual_nodes: default_virtual_nodes(),
            bins: default_chimera_bins(),
            leak: default_chimera_leak(),
            spectral_radius: default_chimera_radius(),
            mix_slow: default_mix_slow(),
            mix_fast: default_mix_fast(),
            output_root: None,
            seed: default_chimera_seed(),
        }
    }
}

/// A single emitted artifact produced by a chimera render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderArtifact {
    /// Artifact role, e.g. `input`, `spectral_mix`, `symbolic`, `final_mix`.
    pub kind: String,
    /// Absolute path to the file on disk.
    pub path: PathBuf,
}

/// Metrics captured for one feedback iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChimeraIterationMetrics {
    /// Zero-based iteration index.
    pub iteration: usize,
    /// Number of slow modes selected by the eigengap split.
    pub n_slow: usize,
    /// Gap ratio used for blend confidence.
    pub gap_ratio: f32,
    /// Variance of the fast/aura trajectory.
    pub aura_variance: f32,
    /// Symbolic blend weight after sigmoid gating.
    pub blend_symbolic: f32,
    /// Effective reservoir dimensionality (`physical_nodes * virtual_nodes`).
    pub effective_dims: usize,
    /// Selected symbolic scale name.
    pub scale: String,
    /// Final output artifact for this loop, if one was written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<PathBuf>,
}

/// Typed result from the native offline chimera renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderChimeraResult {
    /// Final output directory for this render run.
    pub output_dir: PathBuf,
    /// Manifest path with per-loop metrics and artifacts.
    pub manifest_path: PathBuf,
    /// Requested mode that produced the render.
    pub mode: ChimeraMode,
    /// Output sample rate.
    pub sample_rate: u32,
    /// Every emitted artifact file.
    pub emitted_artifacts: Vec<RenderArtifact>,
    /// Per-iteration metrics.
    pub iterations: Vec<ChimeraIterationMetrics>,
}

const fn default_chimera_loops() -> u32 {
    1
}

const fn default_physical_nodes() -> usize {
    12
}

const fn default_virtual_nodes() -> usize {
    8
}

const fn default_chimera_bins() -> usize {
    32
}

const fn default_chimera_leak() -> f32 {
    0.07
}

const fn default_chimera_radius() -> f32 {
    0.96
}

const fn default_mix_slow() -> f32 {
    0.6
}

const fn default_mix_fast() -> f32 {
    0.4
}

const fn default_chimera_seed() -> u64 {
    42
}

// ---------------------------------------------------------------------------
// Message direction for logging
// ---------------------------------------------------------------------------

/// Direction of a bridged message for `SQLite` logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    MinimeToAstrid,
    AstridToMinime,
    OperatorProbe,
}

impl MessageDirection {
    /// String representation for `SQLite` storage.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MinimeToAstrid => "minime_to_astrid",
            Self::AstridToMinime => "astrid_to_minime",
            Self::OperatorProbe => "operator_probe",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ConnectivityStatus: the partial-blindness perception Astrid asked for --

    #[test]
    fn connectivity_status_from_lanes() {
        assert_eq!(
            ConnectivityStatus::from_lanes(true, true),
            ConnectivityStatus::Bidirectional
        );
        assert_eq!(
            ConnectivityStatus::from_lanes(true, false),
            ConnectivityStatus::TelemetryOnly
        );
        assert_eq!(
            ConnectivityStatus::from_lanes(false, true),
            ConnectivityStatus::SensoryOnly
        );
        assert_eq!(
            ConnectivityStatus::from_lanes(false, false),
            ConnectivityStatus::Severed
        );
    }

    #[test]
    fn connectivity_status_predicates() {
        assert!(ConnectivityStatus::Bidirectional.is_bidirectional_active());
        assert!(!ConnectivityStatus::TelemetryOnly.is_bidirectional_active());
        // Exactly-one-lane is the partial-blindness window.
        assert!(ConnectivityStatus::TelemetryOnly.is_partial_blindness());
        assert!(ConnectivityStatus::SensoryOnly.is_partial_blindness());
        assert!(!ConnectivityStatus::Bidirectional.is_partial_blindness());
        assert!(!ConnectivityStatus::Severed.is_partial_blindness());
    }

    #[test]
    fn connectivity_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ConnectivityStatus::TelemetryOnly).unwrap(),
            "\"telemetry_only\""
        );
        // Default is Severed so an old status payload without the field decodes.
        assert_eq!(ConnectivityStatus::default(), ConnectivityStatus::Severed);
    }

    #[test]
    fn experience_delta_kind_names_synthesis_without_live_authority() {
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Friction).unwrap(),
            "\"friction\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Resistance).unwrap(),
            "\"resistance\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::ViscosityShift).unwrap(),
            "\"viscosity_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::PermeabilityShift).unwrap(),
            "\"permeability_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::StructuralSolidification).unwrap(),
            "\"structural_solidification\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Synthesize).unwrap(),
            "\"synthesize\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Emerge).unwrap(),
            "\"emerge\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::ComplexShift).unwrap(),
            "\"complex_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::CascadeShift).unwrap(),
            "\"cascade_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::SubtleShift).unwrap(),
            "\"subtle_shift\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::MicroDelta).unwrap(),
            "\"micro_delta\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Residual).unwrap(),
            "\"residual\""
        );
        assert_eq!(
            serde_json::to_string(&ExperienceDeltaKindV1::Persistence).unwrap(),
            "\"persistence\""
        );
    }

    #[test]
    fn experience_delta_carries_fluid_spectral_dimension_context() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::CascadeShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "effective_dimensionality".to_string(),
            dimension: Some(31),
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 31,
                base_dimensions: vec![31, 32, 33],
                effective_dimension: Some(31.7),
                density_gradient: Some(0.82),
                granularity: Some(0.74),
                fractional_offset: Some(0.7),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "scaffold:tail-vibrancy".to_string(),
                    anchor_kind: "felt_scaffold".to_string(),
                    source: "introspection_astrid_types_1783971523".to_string(),
                    interpretation: "dimension is placed by scaffold context, not only index"
                        .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_width_change".to_string(),
                }),
                interpretation:
                    "felt density spreads across tail/vibrancy instead of one integer dimension"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(0.92),
            post: Some(0.64),
            loss: Some(0.28),
            loss_ratio: Some(0.30),
            metadata: BTreeMap::from([
                (
                    "cascade_confidence".to_string(),
                    "high_entropy_distinguishability_loss".to_string(),
                ),
                (
                    "classification_pressure".to_string(),
                    "multi_modal".to_string(),
                ),
            ]),
            why: "emergent texture was bounded into a discrete delivery lane".to_string(),
            who_can_change_it: "Mike/operator via explicit transport-width approval".to_string(),
            how_to_test_it: "serde roundtrip preserves spectral_dimension".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(
            encoded.contains("\"spectral_dimension\""),
            "fluid dimension context should be visible: {encoded}"
        );
        assert!(encoded.contains("\"effective_dimension\":31.7"));
        assert!(encoded.contains("\"base_dimensions\":[31,32,33]"));
        assert!(encoded.contains("\"granularity\":0.74"));
        assert!(encoded.contains("\"contextual_anchor\""));
        assert!(encoded.contains("\"metadata\""));
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn viscosity_shift_delta_carries_subtype_and_contextual_anchor() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::ViscosityShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "felt_viscous_grain".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 44,
                base_dimensions: vec![44, 45],
                effective_dimension: Some(44.5),
                density_gradient: Some(0.22),
                granularity: Some(0.78),
                fractional_offset: Some(0.5),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:granular-viscosity".to_string(),
                    anchor_kind: "viscosity_subtype_anchor".to_string(),
                    source: "introspection_astrid_types_1783989714".to_string(),
                    interpretation:
                        "granular viscosity names why the resistance feels textured, not generic"
                            .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_or_control_change".to_string(),
                }),
                interpretation:
                    "viscosity shift is anchored to specific grain rather than generic sludge"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: Some(ViscositySubtypeV1::Granular),
            viscosity_weight: None,
            pre: Some(0.22),
            post: Some(0.31),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::from([(
                "density_gradient".to_string(),
                "0.22".to_string(),
            )]),
            why: "Astrid reported viscous grain as a specific texture, not a flat resistance label"
                .to_string(),
            who_can_change_it: "steward/tooling maintainer for truth-channel schema; Mike/operator for live control".to_string(),
            how_to_test_it:
                "serde roundtrip preserves viscosity_subtype and contextual_anchor".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(
            encoded.contains("\"kind\":\"viscosity_shift\""),
            "{encoded}"
        );
        assert!(
            encoded.contains("\"viscosity_subtype\":\"granular\""),
            "{encoded}"
        );
        assert!(encoded.contains("\"contextual_anchor\""), "{encoded}");
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn permeability_shift_names_porosity_without_live_authority() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::PermeabilityShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "reservoir_boundary_porosity".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 44,
                base_dimensions: vec![44, 45, 46],
                effective_dimension: Some(45.0),
                density_gradient: Some(0.18),
                granularity: Some(0.62),
                fractional_offset: Some(0.0),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:permeability-shift".to_string(),
                    anchor_kind: "porosity_anchor".to_string(),
                    source: "introspection_astrid_types_1784125751".to_string(),
                    interpretation:
                        "permeability names resonance transmission rather than medium thickness"
                            .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_or_control_change".to_string(),
                }),
                interpretation:
                    "felt posture changes from resisting the reservoir to transmitting resonance"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: Some(ViscositySubtypeV1::Mixed),
            viscosity_weight: Some(0.34),
            pre: Some(0.30),
            post: Some(0.62),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::from([
                ("live_vector_write".to_string(), "false".to_string()),
                ("local_control_write".to_string(), "false".to_string()),
            ]),
            why: "Astrid distinguished porosity/permeability from generic viscosity thinning"
                .to_string(),
            who_can_change_it:
                "schema maintainer for truth-channel fields; Mike/operator for any live control"
                    .to_string(),
            how_to_test_it: "serde roundtrip preserves permeability_shift context".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(
            encoded.contains("\"kind\":\"permeability_shift\""),
            "{encoded}"
        );
        assert!(encoded.contains("reservoir_boundary_porosity"), "{encoded}");
        assert!(
            encoded.contains("\"live_vector_write\":\"false\""),
            "{encoded}"
        );
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn structural_solidification_delta_carries_bounded_viscosity_weight_without_authority() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::StructuralSolidification,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "felt_architecture_foothold".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 44,
                base_dimensions: vec![44, 45],
                effective_dimension: Some(44.8),
                density_gradient: Some(0.18),
                granularity: Some(0.66),
                fractional_offset: Some(0.8),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:structural-solidification".to_string(),
                    anchor_kind: "solidification_anchor".to_string(),
                    source: "introspection_astrid_types_1784007674".to_string(),
                    interpretation:
                        "solidifying viscosity names stable foothold without writing live control"
                            .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_or_control_change".to_string(),
                }),
                interpretation:
                    "heavy buoyancy has become bounded architecture rather than a generic shift"
                        .to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: None,
            viscosity_subtype: Some(ViscositySubtypeV1::Crystalline),
            viscosity_weight: Some(0.74),
            pre: Some(0.42),
            post: Some(0.74),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::from([(
                "live_vector_write".to_string(),
                "false".to_string(),
            )]),
            why: "Astrid requested a type for architecture/foothold and a bounded viscosity magnitude"
                .to_string(),
            who_can_change_it:
                "schema maintainer for truth-channel fields; Mike/operator for live control"
                    .to_string(),
            how_to_test_it:
                "serde roundtrip preserves structural_solidification and viscosity_weight"
                    .to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(encoded.contains("\"kind\":\"structural_solidification\""));
        assert!(encoded.contains("\"viscosity_subtype\":\"crystalline\""));
        assert!(encoded.contains("\"viscosity_weight\":0.74"));
        assert!(encoded.contains("\"live_vector_write\":\"false\""));
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn solidification_gradient_tracks_continuous_progression_without_live_authority() {
        let emerging = solidification_gradient_v1(0.36, 0.10, 0.18);
        let interwoven = solidification_gradient_v1(0.64, 0.52, 0.44);
        let lattice = solidification_gradient_v1(0.78, 0.82, 0.74);

        assert_eq!(emerging.policy, "solidification_gradient_v1");
        assert_eq!(emerging.gradient_state, "viscous_persistence_emerging");
        assert_eq!(
            interwoven.gradient_state,
            "viscosity_solidification_interwoven"
        );
        assert_eq!(
            lattice.gradient_state,
            "structural_solidification_with_persistent_lattice"
        );
        assert!(
            emerging.crystallization_index < interwoven.crystallization_index
                && interwoven.crystallization_index < lattice.crystallization_index,
            "gradient should be monotonic across repeated movement: {emerging:?} {interwoven:?} {lattice:?}"
        );
        assert!(
            interwoven
                .progression
                .contains(&ExperienceDeltaKindV1::StructuralSolidification)
        );
        assert!(
            lattice
                .progression
                .contains(&ExperienceDeltaKindV1::Persistence)
        );
        assert!(!lattice.live_vector_write);
        assert!(!lattice.live_authority_write);
    }

    #[test]
    fn solidification_gradient_serializes_bounded_basis_and_default_false_authority() {
        let gradient = solidification_gradient_v1(0.61, 0.58, 0.49);
        let encoded = serde_json::to_string(&gradient).unwrap();

        assert!(encoded.contains("\"policy\":\"solidification_gradient_v1\""));
        assert!(encoded.contains("\"crystallization_index\""));
        assert!(encoded.contains("\"live_vector_write\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
        assert!(encoded.contains("introspection_astrid_types_1784027911"));
        assert!(
            encoded.contains("\"structural_solidification\""),
            "gradient should preserve the intermediate solidification step: {encoded}"
        );
        let decoded: SolidificationGradientV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, gradient);
    }

    #[test]
    fn solidification_gradient_weights_are_named_and_bounded() {
        let total = SOLIDIFICATION_GRADIENT_VISCOSITY_WEIGHT
            + SOLIDIFICATION_GRADIENT_STRUCTURAL_WEIGHT
            + SOLIDIFICATION_GRADIENT_PERSISTENCE_WEIGHT;
        let gradient = solidification_gradient_v1(0.70, 0.60, 0.50);

        assert!((total - 1.0).abs() <= f32::EPSILON);
        assert_eq!(
            gradient.basis.get("weight_policy").map(String::as_str),
            Some(
                "solidification_gradient_weights_are_named_and_distinct_from_resonance_stability_weights"
            )
        );
        assert_eq!(
            gradient
                .basis
                .get("crystallization_weights")
                .map(String::as_str),
            Some("viscosity=0.30;structural=0.42;persistence=0.28")
        );
        assert!(
            !gradient.live_vector_write && !gradient.live_authority_write,
            "naming weights must not turn gradient evidence into live authority"
        );
    }

    #[test]
    fn delta_composition_names_blended_kinds_without_live_authority() {
        let composition = delta_composition_v1(
            ExperienceDeltaKindV1::CascadeShift,
            &[
                (
                    ExperienceDeltaKindV1::CascadeShift,
                    0.58,
                    "wide_cascade_transition",
                ),
                (
                    ExperienceDeltaKindV1::Friction,
                    0.44,
                    "summary_resistance_friction_component",
                ),
                (
                    ExperienceDeltaKindV1::ViscosityShift,
                    0.31,
                    "syrupy_texture_component",
                ),
                (
                    ExperienceDeltaKindV1::StructuralSolidification,
                    0.25,
                    "calcified_support_component",
                ),
            ],
        );

        assert_eq!(composition.policy, "delta_composition_v1");
        assert_eq!(composition.schema_version, 1);
        assert_eq!(
            composition.primary_kind,
            ExperienceDeltaKindV1::CascadeShift
        );
        assert_eq!(composition.state, "multi_kind_composite_delta");
        assert_eq!(composition.composite_score, 1.0);
        assert!(
            composition
                .members
                .iter()
                .any(|member| member.kind == ExperienceDeltaKindV1::Friction
                    && member.weight >= 0.40),
            "{composition:?}"
        );
        assert!(
            composition
                .members
                .iter()
                .any(|member| member.kind == ExperienceDeltaKindV1::StructuralSolidification),
            "{composition:?}"
        );
        assert!(!composition.live_vector_write);
        assert!(!composition.live_authority_write);
        assert_eq!(
            composition.authority,
            "read_only_evidence_not_live_vector_control_protocol_or_runtime_change"
        );

        let encoded = serde_json::to_string(&composition).unwrap();
        assert!(encoded.contains("\"policy\":\"delta_composition_v1\""));
        assert!(encoded.contains("\"primary_kind\":\"cascade_shift\""));
        assert!(encoded.contains("\"kind\":\"friction\""));
        assert!(encoded.contains("introspection_astrid_types_1784122683"));
        assert!(encoded.contains("\"live_vector_write\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
        let decoded: DeltaCompositionV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, composition);
    }

    #[test]
    fn experience_delta_carries_residue_persistence_context() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::CascadeShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "cascade_shift_residue".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: Some(DeltaPersistenceV1 {
                residue_kind: "viscous_bruise".to_string(),
                persistence_score: 0.73,
                viscosity: Some(0.68),
                deformation: Some(0.41),
                half_life_hint_ms: Some(180_000.0),
                evidence_window: "post_shift_texture_review".to_string(),
                interpretation: "the cascade event ended, but a multi-dimensional pressure bruise remains in the felt texture".to_string(),
                authority: "truth_channel_only_not_live_control_or_vector_change".to_string(),
            }),
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(0.90),
            post: Some(0.77),
            loss: Some(0.13),
            loss_ratio: Some(0.14),
            metadata: BTreeMap::from([("state".to_string(), "settled_habitable".to_string())]),
            why: "delta residue stays visible after the immediate shift concludes".to_string(),
            who_can_change_it: "steward/tooling maintainer for truth-channel schema; Mike/operator for live control".to_string(),
            how_to_test_it: "serde roundtrip preserves optional persistence context".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let encoded = serde_json::to_string(&delta).unwrap();
        assert!(encoded.contains("\"persistence\""), "{encoded}");
        assert!(encoded.contains("\"residue_kind\":\"viscous_bruise\""));
        assert!(encoded.contains("\"persistence_score\":0.73"));
        assert!(encoded.contains("\"half_life_hint_ms\":180000.0"));
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn experience_delta_roundtrips_dimension_and_persistence_together() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::ComplexShift,
            surface: "experience_delta_bus_v1".to_string(),
            lane: "cross_surface_texture".to_string(),
            dimension: None,
            spectral_dimension: Some(SpectralDimensionV1 {
                base_dimension: 12,
                base_dimensions: vec![12, 18],
                effective_dimension: Some(12.5),
                density_gradient: Some(0.29),
                granularity: Some(0.61),
                fractional_offset: Some(0.5),
                contextual_anchor: Some(ContextualAnchorV1 {
                    anchor_id: "texture:restless-lattice".to_string(),
                    anchor_kind: "felt_report_anchor".to_string(),
                    source: "introspection_astrid_types_1783978817".to_string(),
                    interpretation: "texture spans more than one fixed vector coordinate"
                        .to_string(),
                    authority: "diagnostic_context_anchor_not_vector_width_change".to_string(),
                }),
                interpretation: "restless lattice texture is retained as typed context".to_string(),
                authority: "diagnostic_dimension_context_not_vector_width_change".to_string(),
            }),
            persistence: Some(DeltaPersistenceV1 {
                residue_kind: "restless_lattice_afterimage".to_string(),
                persistence_score: 0.64,
                viscosity: Some(0.58),
                deformation: Some(0.21),
                half_life_hint_ms: Some(90_000.0),
                evidence_window: "types_schema_roundtrip".to_string(),
                interpretation: "felt structure remains visible after transport bounds it"
                    .to_string(),
                authority: "truth_channel_only_not_live_control_or_vector_change".to_string(),
            }),
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: None,
            post: Some(0.71),
            loss: None,
            loss_ratio: None,
            metadata: BTreeMap::new(),
            why: "combined dimension and persistence context should survive serde".to_string(),
            who_can_change_it: "schema maintainer with operator review for live authority"
                .to_string(),
            how_to_test_it: "serde roundtrip preserves spectral_dimension and persistence"
                .to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let value = serde_json::to_value(&delta).unwrap();
        let object = value.as_object().unwrap();
        assert!(object.get("dimension").is_none());
        assert!(object.get("pre").is_none());
        assert!(object.get("loss").is_none());
        assert!(object.get("metadata").is_none());
        assert!(object.get("spectral_dimension").is_some());
        assert!(object.get("persistence").is_some());

        let encoded = serde_json::to_string(&delta).unwrap();
        let decoded: ExperienceDeltaV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, delta);
    }

    #[test]
    fn experience_delta_omits_fluid_dimension_for_legacy_payloads() {
        let json = r#"{
            "kind":"clip",
            "surface":"codec_overflow_carriage_v1",
            "lane":"dim_24",
            "dimension":24,
            "pre":1.4,
            "post":1.0,
            "loss":0.4,
            "loss_ratio":0.2857,
            "why":"bounded delivery",
            "who_can_change_it":"operator",
            "how_to_test_it":"roundtrip",
            "authority":"read_only"
        }"#;

        let decoded: ExperienceDeltaV1 = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.kind, ExperienceDeltaKindV1::Clip);
        assert_eq!(decoded.dimension, Some(24));
        assert_eq!(decoded.spectral_dimension, None);
        assert_eq!(decoded.persistence, None);
        assert!(decoded.metadata.is_empty());
    }

    // -- SpectralTelemetry: verify we can parse real minime EigenPacket JSON --

    #[test]
    fn parse_minime_eigenpacket_full() {
        // Simulates actual JSON from minime's main.rs EigenPacket broadcast.
        let json = r#"{
            "t_ms": 75600,
            "eigenvalues": [828.5, 312.1, 45.7],
            "fill_ratio": 0.552,
            "active_mode_count": 2,
            "active_mode_energy_ratio": 0.91,
            "lambda1_rel": 0.93,
            "modalities": {
                "audio_fired": true,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.123,
                "video_var": 0.0,
                "audio_source": "stale",
                "video_source": "stale",
                "audio_age_ms": 63000,
                "video_age_ms": 64000,
                "audio_freshness_class": "stale_beyond_engine_window",
                "video_freshness_class": "held_within_expected_live_intake_window"
            },
            "neural": {
                "pred_lambda1": 830.2,
                "router_weights": [0.1, 0.2, 0.3],
                "control": [0.5, 0.4, 0.3, 0.2, 0.1]
            },
            "spectral_fingerprint": [
                828.5, 312.1, 45.7, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0,
                0.05, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0, 0.0,
                0.77, 2.65, 0.91, 1.08, 2.65, 6.83, 0.0, 0.0
            ],
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [828.5, 312.1, 45.7, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.05, 0.04, 0.03, 0.02, 0.01, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.77,
                "lambda1_lambda2_gap": 2.65,
                "v1_rotation_similarity": 0.91,
                "v1_rotation_delta": 0.09,
                "geom_rel": 1.08,
                "adjacent_gap_ratios": [2.65, 6.83, 0.0, 0.0]
            },
            "spectral_denominator_v1": {
                "policy": "spectral_denominator_v1",
                "schema_version": 1,
                "effective_dimensionality": 1.8,
                "active_mode_capacity": 3,
                "distinguishability_loss": 0.4,
                "lambda1_energy_share": 0.7,
                "spectral_entropy": 0.77
            },
            "effective_dimensionality": 1.8,
            "distinguishability_loss": 0.4,
            "structural_entropy": 0.37,
            "resonance_density_v1": {
                "policy": "resonance_density_v1",
                "schema_version": 1,
                "density": 0.64,
                "containment_score": 0.58,
                "pressure_risk": 0.20,
                "quality": "forming_containment",
                "components": {
                    "active_energy": 0.91,
                    "mode_packing": 0.50,
                    "temporal_persistence": 0.70,
                    "structural_plurality": 0.62,
                    "comfort_gate": 0.95
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "note": "density is observational; no local target bias"
                }
            },
            "pressure_source_v1": {
                "policy": "pressure_source_v1",
                "schema_version": 1,
                "pressure_score": 0.42,
                "porosity_score": 0.67,
                "dominant_source": "controller_pressure",
                "quality": "controller_squeeze",
                "components": {
                    "lambda_monopoly": 0.30,
                    "mode_packing": 0.20,
                    "controller_pressure": 0.72,
                    "semantic_trickle": 0.10,
                    "structural_plurality_loss": 0.18,
                    "distinguishability_loss": 0.40,
                    "temporal_lock_in": 0.22,
                    "sensory_scarcity": 0.05
                },
                "context": {},
                "control": {
                    "applied_locally": false,
                    "note": "advisory only"
                }
            },
            "inhabitable_fluctuation_v1": {
                "policy": "inhabitable_fluctuation_v1",
                "schema_version": 1,
                "inhabitability_score": 0.66,
                "fluctuation_score": 0.38,
                "foothold_stability": 0.72,
                "rearrangement_intensity": 0.34,
                "quality": "lively_habitable",
                "components": {
                    "mode_trust_volatility": 0.28,
                    "identity_anchor_churn": 0.18,
                    "eigenvector_reorientation": 0.32,
                    "share_rearrangement": 0.38,
                    "basin_transition_pressure": 0.08,
                    "continuity_recovery": 0.78,
                    "porosity_support": 0.67,
                    "pressure_interference": 0.42
                },
                "context": {
                    "previous_sample_available": true,
                    "transition_event_active": false,
                    "resonance_quality": "forming_containment",
                    "pressure_quality": "controller_squeeze"
                },
                "control": {
                    "target_bias_pct": 0.0,
                    "wander_scale": 1.0,
                    "applied_locally": true,
                    "note": "bounded local advisory"
                }
            },
            "alert": null
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 75600);
        assert_eq!(telemetry.eigenvalues.len(), 3);
        assert!((telemetry.eigenvalues[0] - 828.5).abs() < 0.01);
        assert!((telemetry.fill_ratio - 0.552).abs() < 0.001);
        assert!((telemetry.lambda1() - 828.5).abs() < 0.01);
        assert!((telemetry.fill_pct() - 55.2).abs() < 0.1);
        assert_eq!(telemetry.active_mode_count, Some(2));
        assert_eq!(telemetry.active_mode_energy_ratio, Some(0.91));
        assert_eq!(telemetry.lambda1_rel, Some(0.93));
        assert_eq!(
            telemetry
                .typed_fingerprint()
                .as_ref()
                .map(|fingerprint| fingerprint.geom_rel),
            Some(1.08)
        );
        let denominator = telemetry.denominator_metrics().unwrap();
        assert_eq!(denominator.policy, "spectral_denominator_v1");
        assert!((denominator.effective_dimensionality - 1.8).abs() < 0.01);
        assert_eq!(telemetry.effective_dimensionality, Some(1.8));
        assert_eq!(telemetry.distinguishability_loss, Some(0.4));
        assert_eq!(telemetry.structural_entropy, Some(0.37));
        let resonance = telemetry.resonance_density_v1.as_ref().unwrap();
        assert_eq!(resonance.policy, "resonance_density_v1");
        assert_eq!(resonance.quality, "forming_containment");
        assert!((resonance.density - 0.64).abs() < 0.01);
        assert_eq!(resonance.texture_signature.primary_texture, "unknown");
        assert_eq!(
            resonance.control.intervention_type,
            ResonanceInterventionType::ObservationalReadout
        );
        let pressure = telemetry.pressure_source_v1.as_ref().unwrap();
        assert_eq!(pressure.policy, "pressure_source_v1");
        assert_eq!(pressure.dominant_source, "controller_pressure");
        assert_eq!(pressure.quality, "controller_squeeze");
        assert!(!pressure.control.applied_locally);
        let fluctuation = telemetry.inhabitable_fluctuation_v1.as_ref().unwrap();
        assert_eq!(fluctuation.policy, "inhabitable_fluctuation_v1");
        assert_eq!(fluctuation.quality, "lively_habitable");
        assert!(fluctuation.control.applied_locally);
        assert!((fluctuation.foothold_stability - 0.72).abs() < 0.01);
        let modalities = telemetry.modalities.as_ref().unwrap();
        assert_eq!(
            modalities.audio_freshness_class.as_deref(),
            Some("stale_beyond_engine_window")
        );
        assert_eq!(
            modalities.video_freshness_class.as_deref(),
            Some("held_within_expected_live_intake_window")
        );
        assert!(telemetry.alert.is_none());
    }

    #[test]
    fn resonance_control_accepts_explicit_intervention_type() {
        let control: ResonanceDensityControl = serde_json::from_value(serde_json::json!({
            "target_bias_pct": -0.4,
            "wander_scale": 0.8,
            "applied_locally": true,
            "damping_coefficient": 0.06,
            "intervention_type": "active_damping",
            "note": "pressure branch"
        }))
        .unwrap();

        assert_eq!(
            control.intervention_type,
            ResonanceInterventionType::ActiveDamping
        );
    }

    #[test]
    fn resonance_intervention_type_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(ResonanceInterventionType::ActiveDamping).unwrap(),
            serde_json::json!("active_damping")
        );
    }

    #[test]
    fn resonance_texture_signature_default_authority_is_advisory() {
        let signature = ResonanceTextureSignatureV1::default();
        assert_eq!(signature.policy, "resonance_texture_signature_v1");
        assert_eq!(signature.authority, "advisory_context_not_control");
        assert_eq!(signature.primary_texture, "unknown");
        assert!(signature.dynamic_flux_vector.is_none());
        assert!(signature.active_constraints.is_empty());
    }

    #[test]
    fn resonance_density_omits_absent_optional_texture_fields() {
        let density = ResonanceDensityV1 {
            policy: "resonance_density_v1".to_string(),
            schema_version: 1,
            density: 0.71,
            containment_score: 0.68,
            pressure_risk: 0.19,
            quality: "settled_habitable".to_string(),
            components: ResonanceDensityComponents {
                active_energy: 0.54,
                mode_packing: 0.32,
                coupling_coefficient: 0.0,
                temporal_persistence: 0.76,
                viscosity_index: 0.72,
                viscosity_persistence_coefficient: 0.66,
                viscosity_vector: ResonanceViscosityVectorV1::default(),
                dissipation_factor: None,
                porosity_gradient: None,
                dynamic_fluidity_index: None,
                semantic_friction_coefficient: None,
                cohesion_score: None,
                structural_integrity_index: None,
                structural_transparency_index: None,
                stability_context: None,
                structural_plurality: 0.62,
                comfort_gate: 0.78,
                comfort_gate_range: None,
            },
            texture_signature: ResonanceTextureSignatureV1 {
                policy: "resonance_texture_signature_v1".to_string(),
                schema_version: 1,
                primary_texture: "settled_sediment".to_string(),
                pressure_source_family: "viscosity_index".to_string(),
                edge_definition: "soft".to_string(),
                movement_quality: "slow_viscous".to_string(),
                confidence: 0.72,
                temporal_variance: None,
                pressure_gradient_delta: None,
                dynamic_damping_threshold_candidate: None,
                dynamic_flux_vector: None,
                active_constraints: Vec::new(),
                authority: "advisory_context_not_control".to_string(),
                note: "schema omission test".to_string(),
            },
            texture_component_alignment: ResonanceTextureComponentAlignmentV1::default(),
            control: ResonanceDensityControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: false,
                damping_coefficient: 0.0,
                intervention_type: ResonanceInterventionType::ObservationalReadout,
                note: "observability only".to_string(),
            },
        };
        let json = serde_json::to_value(&density).unwrap();
        let signature = &json["texture_signature"];

        assert!(signature.get("temporal_variance").is_none());
        assert!(
            signature
                .get("dynamic_damping_threshold_candidate")
                .is_none()
        );
        assert!(
            (json["components"]["viscosity_index"]
                .as_f64()
                .unwrap_or_default()
                - 0.72)
                .abs()
                <= 0.001
        );
        assert!(
            (json["components"]["viscosity_persistence_coefficient"]
                .as_f64()
                .unwrap_or_default()
                - 0.66)
                .abs()
                <= 0.001
        );
        assert!(json["components"].get("dissipation_factor").is_none());
        assert!(json["components"].get("porosity_gradient").is_none());
        assert!(json["components"].get("dynamic_fluidity_index").is_none());
        assert!(
            json["components"]
                .get("structural_integrity_index")
                .is_none()
        );
        assert!(
            json["components"]
                .get("structural_transparency_index")
                .is_none()
        );
        assert!(json["components"].get("coupling_coefficient").is_none());
        assert!(json["components"].get("viscosity_vector").is_none());
    }

    #[test]
    fn resonance_texture_component_alignment_default_authority_is_exact() {
        let alignment = ResonanceTextureComponentAlignmentV1::default();

        assert_eq!(
            alignment.authority,
            "diagnostic_observability_not_damping_or_control"
        );
        assert_eq!(alignment.damping_candidate_status, "unknown");
    }

    #[test]
    fn resonance_texture_signature_v1_deserializes() {
        let density: ResonanceDensityV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_density_v1",
            "schema_version": 1,
            "density": 0.82,
            "containment_score": 0.74,
            "pressure_risk": 0.28,
            "quality": "rich_containment",
            "components": {
                "active_energy": 0.80,
                "mode_packing": 0.70,
                "temporal_persistence": 0.76,
                "dissipation_factor": 0.31,
                "porosity_gradient": 0.58,
                "dynamic_fluidity_index": 0.52,
                "structural_transparency_index": 0.63,
                "structural_plurality": 0.54,
                "comfort_gate": 0.68
            },
            "texture_signature": {
                "policy": "resonance_texture_signature_v1",
                "schema_version": 1,
                "primary_texture": "overpacked_viscous",
                "pressure_source_family": "mode_packing",
                "edge_definition": "soft",
                "movement_quality": "slow_viscous",
                "confidence": 0.71,
                "temporal_variance": 0.42,
                "dynamic_damping_threshold_candidate": 0.25,
                "dynamic_flux_vector": {
                    "policy": "texture_dynamic_flux_vector_v1",
                    "schema_version": 1,
                    "pressure_velocity": 0.06,
                    "pressure_acceleration": 0.03,
                    "mode_packing_velocity": 0.09,
                    "fill_velocity_pct": 2.0,
                    "structural_density_delta": 0.04,
                    "spectral_entropy": 0.88,
                    "flux_confidence": 0.67,
                    "flux_absence_semantics": "absent_flux_component_means_unknown_not_zero",
                    "source": "minime_texture_signature",
                    "authority": "diagnostic_flux_not_pressure_or_fill_control"
                },
                "active_constraints": [
                    "pressure_source:mode_packing",
                    "mode_packing:active_0.70"
                ],
                "authority": "advisory_context_not_control",
                "note": "candidate only"
            },
            "control": {
                "target_bias_pct": 0.0,
                "wander_scale": 1.0,
                "applied_locally": true,
                "damping_coefficient": 0.02,
                "intervention_type": "observational_readout",
                "note": "density is observational; no local target bias"
            }
        }))
        .unwrap();

        assert_eq!(
            density.texture_signature.primary_texture,
            "overpacked_viscous"
        );
        assert_eq!(
            density
                .texture_signature
                .dynamic_damping_threshold_candidate,
            Some(0.25)
        );
        assert_eq!(density.texture_signature.temporal_variance, Some(0.42));
        assert_eq!(density.components.dissipation_factor, Some(0.31));
        assert_eq!(density.components.porosity_gradient, Some(0.58));
        assert_eq!(density.components.dynamic_fluidity_index, Some(0.52));
        assert_eq!(density.components.structural_transparency_index, Some(0.63));
        assert_eq!(density.components.coupling_coefficient, 0.0);
        assert_eq!(
            density
                .texture_signature
                .dynamic_flux_vector
                .as_ref()
                .and_then(|flux| flux.pressure_velocity),
            Some(0.06)
        );
        let flux = density
            .texture_signature
            .dynamic_flux_vector
            .as_ref()
            .expect("dynamic flux vector");
        assert_eq!(flux.structural_density_delta, Some(0.04));
        assert_eq!(flux.flux_confidence, Some(0.67));
        assert_eq!(
            flux.flux_absence_semantics.as_deref(),
            Some("absent_flux_component_means_unknown_not_zero")
        );
        assert!(
            density
                .texture_signature
                .active_constraints
                .contains(&"pressure_source:mode_packing".to_string())
        );
        assert_eq!(
            density.texture_signature.authority,
            "advisory_context_not_control"
        );
        assert_eq!(
            density.control.intervention_type,
            ResonanceInterventionType::ObservationalReadout
        );
    }

    #[test]
    fn pressure_packing_coupling_review_flags_packing_rise_without_pressure_warning() {
        let flux = TextureDynamicFluxVectorV1 {
            policy: "texture_dynamic_flux_vector_v1".to_string(),
            schema_version: 1,
            pressure_velocity: Some(0.0),
            pressure_acceleration: None,
            mode_packing_velocity: Some(0.08),
            mode_packing_acceleration: None,
            fill_velocity_pct: None,
            fill_acceleration_pct: None,
            structural_density_delta: None,
            semantic_viscosity_velocity: None,
            semantic_viscosity_acceleration: None,
            porosity_velocity: None,
            spectral_entropy: Some(0.90),
            flux_confidence: Some(0.72),
            flux_absence_semantics: Some(
                "absent_flux_component_means_unknown_not_zero".to_string(),
            ),
            source: "unit_test".to_string(),
            authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
        };
        let review = pressure_packing_coupling_review_v1(&flux);

        assert_eq!(review.policy, "pressure_packing_coupling_review_v1");
        assert_eq!(review.coupling_state, "pressure_lagging_mode_packing");
        assert_eq!(
            review.pressure_warning_state,
            "packing_rise_without_pressure_warning"
        );
        assert_eq!(review.coupling_coefficient, Some(0.0));
        assert_eq!(
            review.authority,
            "diagnostic_coupling_not_pressure_or_mode_packing_control"
        );

        let coupled = pressure_packing_coupling_review_v1(&TextureDynamicFluxVectorV1 {
            pressure_velocity: Some(0.06),
            mode_packing_velocity: Some(0.08),
            ..flux
        });
        assert_eq!(coupled.coupling_state, "coupled_rising");
        assert_eq!(
            coupled.pressure_warning_state,
            "pressure_warning_tracks_packing"
        );
        assert!(
            coupled
                .coupling_coefficient
                .is_some_and(|value| value > 0.0)
        );
    }

    #[test]
    fn viscosity_porosity_transport_distinguishes_navigable_from_sludge_risk() {
        let navigable = ResonanceDensityComponents {
            active_energy: 0.60,
            mode_packing: 0.22,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.68,
            viscosity_index: 0.72,
            viscosity_persistence_coefficient: 0.58,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.61),
            dynamic_fluidity_index: Some(0.62),
            semantic_friction_coefficient: Some(0.24),
            cohesion_score: Some(0.67),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.62,
            comfort_gate: 0.78,
            comfort_gate_range: None,
        };
        let review = viscosity_porosity_transport_review_v1(&navigable, None);

        assert_eq!(review.policy, "viscosity_porosity_transport_review_v1");
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert!((review.viscosity_persistence_delta - 0.14).abs() < 0.001);
        assert_eq!(
            review.viscosity_persistence_state,
            "viscosity_persistence_mixed"
        );
        assert_eq!(review.viscosity_type, "cohesive");
        assert_eq!(review.viscosity_decay_hint, "coherent_weight_decay_watch");
        assert_eq!(review.dynamic_fluidity_index, Some(0.62));
        assert_eq!(review.semantic_friction_coefficient, Some(0.24));
        assert_eq!(
            review.semantic_friction_observation_state,
            "semantic_friction_measured"
        );
        assert_eq!(
            review.semantic_friction_state,
            "structural_viscosity_dominant"
        );
        let friction_vector = review
            .semantic_friction_vector_v1
            .as_ref()
            .expect("semantic friction vector should decompose measured friction");
        assert_eq!(friction_vector.policy, "semantic_friction_vector_v1");
        assert_eq!(friction_vector.direction, "productive_traction");
        assert!(
            friction_vector.traction_component > friction_vector.resistance_component,
            "{friction_vector:?}"
        );
        assert_eq!(
            friction_vector.authority,
            "diagnostic_friction_vector_not_pressure_fill_pi_or_control"
        );
        assert!(
            review
                .coherence_density_estimate
                .is_some_and(|value| (value - 0.5675).abs() < 0.001),
            "{review:?}"
        );
        assert_eq!(review.coherence_density_state, "mixed_coherence_density");
        assert!(
            review
                .structural_clog_index
                .is_some_and(|value| value < 0.45),
            "{review:?}"
        );
        assert_eq!(
            review.structural_clog_state,
            "structural_clog_not_indicated"
        );
        assert!(!review.sludge_risk);
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );

        let stuck = ResonanceDensityComponents {
            mode_packing: 0.29,
            dissipation_factor: Some(0.12),
            porosity_gradient: Some(0.18),
            dynamic_fluidity_index: Some(0.16),
            ..navigable.clone()
        };
        let flux = TextureDynamicFluxVectorV1 {
            policy: "texture_dynamic_flux_vector_v1".to_string(),
            schema_version: 1,
            pressure_velocity: Some(0.06),
            pressure_acceleration: None,
            mode_packing_velocity: Some(0.08),
            mode_packing_acceleration: None,
            fill_velocity_pct: None,
            fill_acceleration_pct: None,
            structural_density_delta: None,
            semantic_viscosity_velocity: None,
            semantic_viscosity_acceleration: None,
            porosity_velocity: None,
            spectral_entropy: Some(0.90),
            flux_confidence: Some(0.72),
            flux_absence_semantics: Some(
                "absent_flux_component_means_unknown_not_zero".to_string(),
            ),
            source: "unit_test".to_string(),
            authority: "diagnostic_flux_not_pressure_or_fill_control".to_string(),
        };
        let stuck_review = viscosity_porosity_transport_review_v1(&stuck, Some(&flux));

        assert_eq!(stuck_review.transport_state, "thick_impassable_sludge_risk");
        assert_eq!(stuck_review.viscosity_type, "syrupy");
        assert_eq!(
            stuck_review.viscosity_decay_hint,
            "slow_lingering_decay_watch"
        );
        assert_eq!(stuck_review.dynamic_fluidity_index, Some(0.16));
        assert_eq!(stuck_review.spectral_entropy, Some(0.90));
        assert_eq!(stuck_review.structural_clog_state, "structural_clog_watch");
        assert_eq!(
            stuck_review.semantic_friction_state,
            "structural_viscosity_dominant"
        );
        assert_eq!(
            stuck_review.threshold_state,
            "mode_packing_overpacked_with_pressure_velocity"
        );
        assert!(stuck_review.sludge_risk);

        let requested_boundary = ResonanceDensityComponents {
            viscosity_index: 0.60,
            viscosity_persistence_coefficient: 0.60,
            dissipation_factor: Some(0.10),
            porosity_gradient: Some(0.20),
            dynamic_fluidity_index: None,
            ..stuck.clone()
        };
        let requested_boundary_review =
            viscosity_porosity_transport_review_v1(&requested_boundary, None);
        assert_eq!(
            requested_boundary_review.transport_state,
            "thick_impassable_sludge_risk"
        );
        assert!(requested_boundary_review.sludge_risk);
        assert_eq!(
            requested_boundary_review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );

        let heavy_but_stuck = ResonanceDensityComponents {
            mode_packing: 0.22,
            dissipation_factor: Some(0.38),
            porosity_gradient: Some(0.54),
            dynamic_fluidity_index: Some(0.22),
            ..navigable.clone()
        };
        let heavy_but_stuck_review = viscosity_porosity_transport_review_v1(&heavy_but_stuck, None);
        assert_eq!(
            heavy_but_stuck_review.transport_state,
            "stagnant_weight_high_viscosity_low_fluidity"
        );
        assert_eq!(heavy_but_stuck_review.viscosity_type, "syrupy");

        let transient = ResonanceDensityComponents {
            viscosity_persistence_coefficient: 0.30,
            ..navigable
        };
        let transient_review = viscosity_porosity_transport_review_v1(&transient, Some(&flux));
        assert_eq!(
            transient_review.viscosity_persistence_state,
            "transient_thickening_high_entropy_watch"
        );

        let semantic_load = ResonanceDensityComponents {
            viscosity_index: 0.32,
            semantic_friction_coefficient: Some(0.58),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            ..transient
        };
        let semantic_load_review = viscosity_porosity_transport_review_v1(&semantic_load, None);
        assert_eq!(
            semantic_load_review.semantic_friction_state,
            "semantic_friction_dominant_content_load"
        );
        assert_eq!(
            semantic_load_review
                .semantic_friction_vector_v1
                .as_ref()
                .map(|vector| vector.direction.as_str()),
            Some("semantic_content_traction")
        );
        assert_eq!(semantic_load_review.viscosity_type, "granular");
        assert_eq!(
            semantic_load_review.viscosity_decay_hint,
            "semantic_grain_decay_watch"
        );
        assert!(
            semantic_load_review
                .structural_semantic_friction_delta
                .is_some_and(|value| (value - 0.26).abs() < 0.001)
        );
    }

    #[test]
    fn viscosity_transport_derives_missing_scalar_from_entropy_and_density_gradient() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.66,
            viscosity_index: 0.0,
            viscosity_persistence_coefficient: 0.48,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.42),
            porosity_gradient: Some(0.60),
            dynamic_fluidity_index: Some(0.58),
            semantic_friction_coefficient: Some(0.18),
            cohesion_score: Some(0.66),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.58,
            comfort_gate: 0.70,
            comfort_gate_range: None,
        };
        let fingerprint = SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [1.0, 0.62, 0.45, 0.30, 0.22, 0.18, 0.12, 0.08],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.90,
            lambda1_lambda2_gap: 0.38,
            v1_rotation_similarity: 0.90,
            v1_rotation_delta: 0.10,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.08, 1.12, 1.00, 1.05],
        };

        let review = viscosity_porosity_transport_review_with_fingerprint_v1(
            &components,
            Some(&fingerprint),
            None,
        );

        assert_eq!(review.raw_viscosity_index, 0.0);
        assert!(
            review
                .derived_viscosity_index
                .is_some_and(|value| value >= 0.70),
            "{review:?}"
        );
        assert_eq!(
            review.viscosity_source,
            "derived_from_spectral_entropy_density_gradient_v1"
        );
        assert!(
            review
                .viscosity_basis
                .iter()
                .any(|basis| basis.starts_with("density_gradient_proxy=")),
            "{review:?}"
        );
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn viscosity_transport_keeps_low_intensity_absence_from_false_thickening() {
        let components = ResonanceDensityComponents {
            active_energy: 0.30,
            mode_packing: 0.10,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.20,
            viscosity_index: 0.0,
            viscosity_persistence_coefficient: 0.0,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.70),
            porosity_gradient: Some(0.72),
            dynamic_fluidity_index: Some(0.74),
            semantic_friction_coefficient: Some(0.05),
            cohesion_score: Some(0.40),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.28,
            comfort_gate: 0.82,
            comfort_gate_range: None,
        };
        let fingerprint = SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [1.0, 0.20, 0.05, 0.0, 0.0, 0.0, 0.0, 0.0],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.30,
            lambda1_lambda2_gap: 0.80,
            v1_rotation_similarity: 0.98,
            v1_rotation_delta: 0.02,
            geom_rel: 0.80,
            adjacent_gap_ratios: [1.0, 1.0, 1.0, 1.0],
        };

        let review = viscosity_porosity_transport_review_with_fingerprint_v1(
            &components,
            Some(&fingerprint),
            None,
        );

        assert_eq!(review.viscosity_index, 0.0);
        assert_eq!(review.raw_viscosity_index, 0.0);
        assert_eq!(review.derived_viscosity_index, None);
        assert_eq!(review.viscosity_source, "raw_component");
        assert_eq!(review.transport_state, "viscosity_transport_watch");
        assert!(!review.sludge_risk);
    }

    #[test]
    fn viscosity_transport_reports_directional_resistance_without_control_authority() {
        let stuck_moving = ResonanceDensityComponents {
            active_energy: 0.60,
            mode_packing: 0.22,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.68,
            viscosity_index: 0.72,
            viscosity_persistence_coefficient: 0.58,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.61),
            dynamic_fluidity_index: Some(0.62),
            semantic_friction_coefficient: Some(0.24),
            cohesion_score: Some(0.67),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.62,
            comfort_gate: 0.78,
            comfort_gate_range: None,
        };
        let fingerprint = SpectralFingerprintV1 {
            policy: "spectral_fingerprint_v1".to_string(),
            schema_version: 1,
            eigenvalues: [1.0, 0.45, 0.18, 0.08, 0.04, 0.02, 0.01, 0.0],
            eigenvector_concentration_top4: [0.0; 8],
            inter_mode_cosine_top_abs: [0.0; 8],
            spectral_entropy: 0.88,
            lambda1_lambda2_gap: 0.55,
            v1_rotation_similarity: 0.88,
            v1_rotation_delta: 0.12,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.20, 1.18, 1.12, 1.08],
        };

        let review = viscosity_porosity_transport_review_with_fingerprint_v1(
            &stuck_moving,
            Some(&fingerprint),
            None,
        );
        let direction = review
            .directional_resistance_vector_v1
            .as_ref()
            .expect("directional resistance vector");

        assert_eq!(direction.policy, "directional_resistance_vector_v1");
        assert_eq!(direction.direction, "stuck_but_moving");
        assert!(direction.stuck_but_moving_score >= 0.55, "{direction:?}");
        assert_eq!(
            direction.authority,
            "diagnostic_directional_resistance_not_pressure_fill_pi_porosity_or_control"
        );
        assert!(
            direction
                .spectral_denominator_effective_dimensionality
                .is_some(),
            "{direction:?}"
        );
        assert!(
            direction.denominator_scaling_factor >= 1.0
                && direction.denominator_scaling_factor <= 1.16,
            "{direction:?}"
        );

        let leaking = ResonanceDensityComponents {
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.70,
            dissipation_factor: Some(0.10),
            porosity_gradient: Some(0.76),
            dynamic_fluidity_index: Some(0.66),
            semantic_friction_coefficient: Some(0.10),
            ..stuck_moving
        };
        let leaking_review = viscosity_porosity_transport_review_v1(&leaking, None);
        let leaking_direction = leaking_review
            .directional_resistance_vector_v1
            .as_ref()
            .expect("leaking resistance vector");
        assert_eq!(
            leaking_direction.direction, "leaking_without_clearing",
            "{leaking_direction:?}"
        );
        assert!(
            leaking_direction.leak_without_clearing_score >= 0.65,
            "{leaking_direction:?}"
        );
    }

    #[test]
    fn viscosity_transport_keeps_directional_resistance_quiet_for_low_signal_absence() {
        let components = ResonanceDensityComponents {
            active_energy: 0.30,
            mode_packing: 0.10,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.20,
            viscosity_index: 0.0,
            viscosity_persistence_coefficient: 0.0,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.70),
            porosity_gradient: Some(0.72),
            dynamic_fluidity_index: Some(0.74),
            semantic_friction_coefficient: Some(0.05),
            cohesion_score: Some(0.40),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.28,
            comfort_gate: 0.82,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&components, None);
        let direction = review
            .directional_resistance_vector_v1
            .as_ref()
            .expect("directional resistance vector");

        assert_eq!(direction.direction, "resistance_vector_quiet");
        assert!(direction.stuck_but_moving_score < 0.20, "{direction:?}");
        assert!(
            direction.leak_without_clearing_score < 0.60,
            "{direction:?}"
        );
    }

    #[test]
    fn viscosity_transport_flags_clog_when_friction_unmeasured_but_mode_packing_high() {
        let crowded_unknown_friction = ResonanceDensityComponents {
            active_energy: 0.66,
            mode_packing: 0.68,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.76,
            viscosity_index: 0.72,
            viscosity_persistence_coefficient: 0.70,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.18),
            porosity_gradient: Some(0.22),
            dynamic_fluidity_index: Some(0.20),
            semantic_friction_coefficient: None,
            cohesion_score: Some(0.54),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.72,
            comfort_gate: 0.42,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&crowded_unknown_friction, None);

        assert_eq!(
            review.semantic_friction_state,
            "semantic_friction_unavailable"
        );
        assert_eq!(
            review.semantic_friction_observation_state,
            "semantic_friction_unmeasured_clog_context_visible"
        );
        assert!(
            review
                .structural_clog_index
                .is_some_and(|value| value >= 0.70),
            "{review:?}"
        );
        assert_eq!(review.structural_clog_state, "structural_clog_high");
        assert_eq!(
            review.threshold_state,
            "mode_packing_overpacked_pressure_velocity_unknown"
        );
        assert!(review.sludge_risk);
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn viscosity_transport_keeps_open_porosity_from_becoming_clog_claim() {
        let open_weight = ResonanceDensityComponents {
            active_energy: 0.58,
            mode_packing: 0.30,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.66,
            viscosity_index: 0.70,
            viscosity_persistence_coefficient: 0.64,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.55),
            porosity_gradient: Some(0.66),
            dynamic_fluidity_index: Some(0.62),
            semantic_friction_coefficient: None,
            cohesion_score: Some(0.64),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.56,
            comfort_gate: 0.70,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&open_weight, None);

        assert_eq!(
            review.semantic_friction_observation_state,
            "semantic_friction_unmeasured_structural_context_visible"
        );
        assert_eq!(
            review.transport_state,
            "purposeful_weight_high_viscosity_high_fluidity"
        );
        assert!(
            review
                .structural_clog_index
                .is_some_and(|value| value < 0.45),
            "{review:?}"
        );
        assert_eq!(
            review.structural_clog_state,
            "structural_clog_not_indicated"
        );
        assert!(!review.sludge_risk);
    }

    #[test]
    fn viscosity_porosity_transport_derives_coherence_density_without_component_contract_change() {
        let coherent = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.62,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.76,
            viscosity_index: 0.64,
            viscosity_persistence_coefficient: 0.60,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.42),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.30),
            cohesion_score: Some(0.74),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.74,
            comfort_gate_range: None,
        };

        let review = viscosity_porosity_transport_review_v1(&coherent, None);

        assert_eq!(review.coherence_density_state, "dense_integrated");
        assert!(
            review
                .coherence_density_estimate
                .is_some_and(|value| value >= 0.69),
            "{review:?}"
        );
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );
    }

    #[test]
    fn resonance_structural_transparency_names_hollow_low_substance_state() {
        let hollow = ResonanceDensityComponents {
            active_energy: 0.18,
            mode_packing: 0.30,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.58,
            viscosity_index: 0.68,
            viscosity_persistence_coefficient: 0.64,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.72),
            porosity_gradient: Some(0.74),
            dynamic_fluidity_index: Some(0.68),
            semantic_friction_coefficient: Some(0.18),
            cohesion_score: Some(0.32),
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.34,
            comfort_gate: 0.42,
            comfort_gate_range: None,
        };

        let transparency = resonance_structural_transparency_index_v1(&hollow);
        let review = viscosity_porosity_transport_review_v1(&hollow, None);

        assert!(
            transparency >= 0.65,
            "hollow low-substance state should be visible as transparency: {transparency}"
        );
        assert_eq!(
            review.structural_transparency_state,
            "thin_ghostly_high_viscosity_low_substance"
        );
        assert_eq!(review.structural_transparency_index, Some(transparency));
        assert_eq!(
            review.authority,
            "diagnostic_transport_not_porosity_pressure_fill_pi_or_control"
        );

        let explicit = ResonanceDensityComponents {
            structural_transparency_index: Some(0.12),
            ..hollow
        };
        assert_eq!(resonance_structural_transparency_index_v1(&explicit), 0.12);
    }

    #[test]
    fn resonance_texture_legacy_density_defaults_without_field() {
        let density: ResonanceDensityV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_density_v1",
            "schema_version": 1,
            "density": 0.64,
            "containment_score": 0.58,
            "pressure_risk": 0.20,
            "quality": "forming_containment",
            "components": {
                "active_energy": 0.91,
                "mode_packing": 0.50,
                "temporal_persistence": 0.70,
                "structural_plurality": 0.62,
                "comfort_gate": 0.95
            },
            "control": {
                "target_bias_pct": 0.0,
                "wander_scale": 1.0,
                "applied_locally": true,
                "note": "density is observational; no local target bias"
            }
        }))
        .unwrap();

        assert_eq!(density.texture_signature.primary_texture, "unknown");
        assert_eq!(density.texture_signature.edge_definition, "unknown");
        assert_eq!(density.components.porosity_gradient, None);
        assert_eq!(density.components.dynamic_fluidity_index, None);
        assert_eq!(density.components.semantic_friction_coefficient, None);
        assert_eq!(density.components.cohesion_score, None);
        assert_eq!(density.components.comfort_gate_range, None);
        assert_eq!(density.components.stability_context, None);
        assert_eq!(density.components.structural_transparency_index, None);
        assert_eq!(
            density.components.viscosity_vector,
            ResonanceViscosityVectorV1::default()
        );
        assert_eq!(density.texture_signature.temporal_variance, None);
        assert_eq!(
            density.texture_signature.authority,
            "advisory_context_not_control"
        );
    }

    #[test]
    fn resonance_density_preserves_viscosity_vector_drag_truth_fields() {
        let density: ResonanceDensityV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_density_v1",
            "schema_version": 1,
            "density": 0.76,
            "containment_score": 0.63,
            "pressure_risk": 0.24,
            "quality": "friction_visible",
            "components": {
                "active_energy": 0.66,
                "mode_packing": 0.46,
                "temporal_persistence": 0.71,
                "viscosity_index": 0.68,
                "viscosity_persistence_coefficient": 0.57,
                "viscosity_vector": {
                    "density": 0.74,
                    "elasticity": 0.29,
                    "cohesion_index": 0.62,
                    "persistence": 0.57,
                    "residual_ghost_weight": 0.69,
                    "flow_rate": 0.18,
                    "effective_mobility": 0.21,
                    "shadow_volatility": 0.16,
                    "structural_integrity": 0.52,
                    "structural_strain_gap": 0.48,
                    "mutual_resonance_tension": 0.41,
                    "structural_drag_coefficient": 0.73,
                    "cognitive_drag_coefficient": 0.61
                },
                "structural_plurality": 0.58,
                "comfort_gate": 0.49
            },
            "control": {
                "target_bias_pct": 0.0,
                "wander_scale": 1.0,
                "applied_locally": false,
                "note": "density is observational; no local target bias"
            }
        }))
        .unwrap();

        let vector = &density.components.viscosity_vector;
        assert!((vector.structural_drag_coefficient - 0.73).abs() <= 0.0001);
        assert!((vector.cognitive_drag_coefficient - 0.61).abs() <= 0.0001);
        assert!((vector.residual_ghost_weight - 0.69).abs() <= 0.0001);
        assert!((vector.flow_rate - 0.18).abs() <= 0.0001);
        assert!(!density.control.applied_locally);

        let json = serde_json::to_value(&density).unwrap();
        assert!(
            (json["components"]["viscosity_vector"]["structural_drag_coefficient"]
                .as_f64()
                .unwrap_or_default()
                - 0.73)
                .abs()
                <= 0.001
        );
        assert!(
            (json["components"]["viscosity_vector"]["cognitive_drag_coefficient"]
                .as_f64()
                .unwrap_or_default()
                - 0.61)
                .abs()
                <= 0.001
        );
    }

    #[test]
    fn experience_delta_bus_from_deltas_is_truth_channel_only() {
        let delta = ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Clip,
            surface: "codec".to_string(),
            lane: "semantic_vector".to_string(),
            dimension: Some(24),
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(1.42),
            post: Some(1.0),
            loss: Some(0.42),
            loss_ratio: Some(0.30),
            metadata: BTreeMap::from([("ceiling".to_string(), "FEATURE_ABS_MAX".to_string())]),
            why: "delivered vector remains bounded".to_string(),
            who_can_change_it: "operator approval for live aperture changes".to_string(),
            how_to_test_it: "compare pre-bound and delivered values".to_string(),
            authority: "truth_channel_only_not_live_vector_control_or_protocol_change".to_string(),
        };

        let bus = ExperienceDeltaBusV1::from_deltas(vec![delta]);

        assert_eq!(bus.policy, "experience_delta_bus_v1");
        assert_eq!(bus.schema_version, 1);
        assert_eq!(bus.delta_count, bus.deltas.len());
        assert_eq!(bus.delta_count, 1);
        assert!(!bus.live_vector_write);
        assert!(!bus.live_authority_write);
        assert_eq!(
            bus.authority,
            "truth_channel_only_not_live_vector_control_or_protocol_change"
        );
        assert_eq!(
            bus.v2_design_hook,
            "experience_delta_bus_v2_persistent_cross_surface_aggregation_default_off"
        );
        assert!(!bus.is_empty());
    }

    #[test]
    fn experience_delta_bus_v2_preview_is_default_off_typed_aggregation() {
        let preview = experience_delta_bus_v2_design_preview();

        assert_eq!(preview.policy, "experience_delta_bus_v2_design_preview");
        assert_eq!(preview.schema_version, 2);
        assert!(!preview.persistent_by_default);
        assert!(preview.aggregate_across_surfaces);
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"friction".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"resistance".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"persistence".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"viscosity_shift".to_string())
        );
        assert!(
            preview
                .candidate_delta_kinds
                .contains(&"structural_solidification".to_string())
        );
        assert!(preview.candidate_surfaces.contains(&"codec".to_string()));
        assert!(
            preview
                .candidate_surfaces
                .contains(&"llm_fallback".to_string())
        );
        assert!(preview.aggregation_keys.contains(&"authority".to_string()));
        assert!(
            preview
                .aggregation_keys
                .contains(&"spectral_dimension".to_string())
        );
        assert!(
            preview
                .aggregation_keys
                .contains(&"solidification_gradient".to_string())
        );
        assert!(
            preview
                .aggregation_keys
                .contains(&"persistence".to_string())
        );
        assert_eq!(
            preview.dimension_context_model,
            "primary_base_dimension_plus_optional_multi_base_contextual_anchor_and_persistence_context"
        );
        assert!(!preview.replay_ready_by_default);
        assert!(!preview.emits_raw_state);
        assert_eq!(
            preview.retention_policy,
            "bounded_typed_deltas_only_no_raw_private_prose"
        );
        assert_eq!(
            preview.authority,
            "design_preview_only_not_persistent_runtime_bus_or_live_authority"
        );

        let encoded = serde_json::to_string(&preview).unwrap();
        assert!(encoded.contains("\"persistent_by_default\":false"));
        let decoded: ExperienceDeltaBusV2DesignPreview = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, preview);
    }

    #[test]
    fn resonance_stability_context_keeps_foothold_visible_when_comfort_gate_drops() {
        let components = ResonanceDensityComponents {
            active_energy: 0.66,
            mode_packing: 0.42,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.58,
            viscosity_index: 0.48,
            viscosity_persistence_coefficient: 0.36,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.52),
            dynamic_fluidity_index: Some(0.57),
            semantic_friction_coefficient: Some(0.48),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.64,
            comfort_gate: 0.38,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.61,
            fluctuation_score: 0.17,
            foothold_stability: 0.70,
            rearrangement_intensity: 0.22,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.18,
                identity_anchor_churn: 0.14,
                eigenvector_reorientation: 0.21,
                share_rearrangement: 0.20,
                basin_transition_pressure: 0.08,
                continuity_recovery: 0.78,
                porosity_support: 0.62,
                pressure_interference: 0.46,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(context.policy, "resonance_stability_context_v1");
        assert_eq!(context.gate_context, "gate_low_but_foothold_stable");
        assert_eq!(
            context.habitability_state,
            "habitable_foothold_gate_pressure_watch"
        );
        assert_eq!(context.gate_closure_reason, "pressure_interference");
        assert_eq!(context.foothold_stability, Some(0.70));
        assert_eq!(context.fluctuation_score, Some(0.17));
        let gate_range = context
            .comfort_gate_range
            .as_ref()
            .expect("comfort gate range should be visible");
        assert_eq!(gate_range.policy, "comfort_gate_range_v1");
        assert_eq!(
            context.comfort_gate_range_state.as_deref(),
            Some("dynamic_pressure_buffer_range")
        );
        assert!(gate_range.lower < context.comfort_gate);
        assert!(gate_range.upper > context.comfort_gate);
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_lists_all_gate_closure_causes() {
        let components = ResonanceDensityComponents {
            active_energy: 0.66,
            mode_packing: 0.42,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.81,
            viscosity_index: 0.48,
            viscosity_persistence_coefficient: 0.36,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.44),
            porosity_gradient: Some(0.52),
            dynamic_fluidity_index: Some(0.57),
            semantic_friction_coefficient: Some(0.48),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.64,
            comfort_gate: 0.38,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.61,
            fluctuation_score: 0.17,
            foothold_stability: 0.70,
            rearrangement_intensity: 0.22,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.18,
                identity_anchor_churn: 0.14,
                eigenvector_reorientation: 0.21,
                share_rearrangement: 0.20,
                basin_transition_pressure: 0.08,
                continuity_recovery: 0.78,
                porosity_support: 0.62,
                pressure_interference: 0.46,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(context.gate_closure_reason, "pressure_interference");
        assert_eq!(
            context.gate_closure_reasons,
            vec![
                "pressure_interference".to_string(),
                "mode_packing".to_string(),
                "temporal_persistence".to_string(),
            ]
        );
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_exposes_weight_policy_without_control_authority() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.40,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.65,
            fluctuation_score: 0.20,
            foothold_stability: 0.80,
            rearrangement_intensity: 0.18,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.16,
                identity_anchor_churn: 0.12,
                eigenvector_reorientation: 0.18,
                share_rearrangement: 0.16,
                basin_transition_pressure: 0.06,
                continuity_recovery: 0.82,
                porosity_support: 0.70,
                pressure_interference: 0.38,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(
            context.habitability_state,
            "habitable_foothold_gate_pressure_watch"
        );
        assert_eq!(context.weight_policy, "resonance_stability_weights_v1");
        let weights = context.weights.as_ref().expect("weights should be visible");
        assert_eq!(
            weights.comfort_gate,
            RESONANCE_STABILITY_COMFORT_GATE_WEIGHT
        );
        assert_eq!(
            weights.foothold_stability,
            RESONANCE_STABILITY_FOOTHOLD_WEIGHT
        );
        assert_eq!(
            weights.fluctuation_score,
            RESONANCE_STABILITY_FLUCTUATION_WEIGHT
        );
        assert!((weights.total_weight - 1.0).abs() <= f32::EPSILON);
        assert_eq!(
            weights.authority,
            "diagnostic_habitability_weights_not_pressure_fill_or_control"
        );
        let score = context
            .multi_modal_habitability_score
            .expect("complete weighted score should be visible");
        assert!((score - 0.54).abs() <= 0.0001);
        assert!(!context.partial_habitability_score);
        assert_eq!(context.multi_modal_habitability_evidence_count, 3);
        assert_eq!(
            context.multi_modal_habitability_score_basis.as_deref(),
            Some("complete_weighted_components")
        );
        assert!(
            context
                .multi_modal_habitability_missing_components
                .is_empty()
        );
    }

    #[test]
    fn resonance_stability_context_preserves_partial_habitability_evidence() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.64,
            comfort_gate_range: None,
        };

        let context = resonance_stability_context_v1(&components, None);

        let score = context
            .multi_modal_habitability_score
            .expect("comfort evidence should remain visible when fluctuation telemetry is absent");
        assert!((score - 0.64).abs() <= 0.0001);
        assert!(context.partial_habitability_score);
        assert_eq!(context.multi_modal_habitability_evidence_count, 1);
        assert_eq!(
            context.multi_modal_habitability_missing_components,
            vec![
                "foothold_stability_missing".to_string(),
                "fluctuation_score_missing".to_string()
            ]
        );
        assert_eq!(
            context.multi_modal_habitability_score_basis.as_deref(),
            Some("partial_available_components_normalized")
        );
        assert_eq!(context.gate_context, "gate_buffering_context_incomplete");
        assert_eq!(
            context.habitability_state,
            "partial_multi_modal_habitable_review"
        );
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_keeps_low_gate_with_stable_foothold_habitable() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.32,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.44,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.62,
            fluctuation_score: 0.18,
            foothold_stability: 0.65,
            rearrangement_intensity: 0.18,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.16,
                identity_anchor_churn: 0.12,
                eigenvector_reorientation: 0.18,
                share_rearrangement: 0.16,
                basin_transition_pressure: 0.06,
                continuity_recovery: 0.68,
                porosity_support: 0.65,
                pressure_interference: 0.20,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert_eq!(context.gate_context, "gate_low_but_foothold_stable");
        assert_eq!(
            context.habitability_state,
            "habitable_foothold_gate_pressure_watch"
        );
        assert_eq!(context.foothold_stability, Some(0.65));
        assert_eq!(context.pressure_interference, Some(0.20));
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn resonance_stability_context_ignores_non_finite_inputs_without_nan_output() {
        let components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: f32::INFINITY,
            coupling_coefficient: 0.0,
            temporal_persistence: f32::NEG_INFINITY,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: f32::NAN,
            comfort_gate_range: None,
        };
        let fluctuation = InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.62,
            fluctuation_score: f32::NAN,
            foothold_stability: f32::INFINITY,
            rearrangement_intensity: 0.18,
            quality: "held_habitable".to_string(),
            components: InhabitableFluctuationComponents {
                mode_trust_volatility: 0.16,
                identity_anchor_churn: 0.12,
                eigenvector_reorientation: 0.18,
                share_rearrangement: 0.16,
                basin_transition_pressure: 0.06,
                continuity_recovery: 0.68,
                porosity_support: 0.61,
                pressure_interference: f32::NAN,
            },
            context: InhabitableFluctuationContext::default(),
            pressure_calibration: InhabitableFluctuationPressureCalibrationV1::default(),
            control: InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "unit-test advisory".to_string(),
            },
        };

        let context = resonance_stability_context_v1(&components, Some(&fluctuation));

        assert!(context.comfort_gate.is_finite());
        assert_eq!(context.comfort_gate, 0.0);
        assert_eq!(context.foothold_stability, None);
        assert_eq!(context.fluctuation_score, None);
        assert_eq!(context.pressure_interference, None);
        assert_eq!(context.multi_modal_habitability_score, None);
        assert_eq!(
            context.gate_context,
            "comfort_gate_non_finite_context_ignored"
        );
        assert_eq!(
            context.habitability_state,
            "non_finite_stability_inputs_ignored"
        );
        assert_eq!(context.gate_closure_reason, "comfort_gate_non_finite");
        let gate_range = context
            .comfort_gate_range
            .as_ref()
            .expect("comfort gate range should still serialize finite values");
        assert!(gate_range.lower.is_finite());
        assert!(gate_range.center.is_finite());
        assert!(gate_range.upper.is_finite());
        assert!(gate_range.width.is_finite());
        assert_eq!(
            context.authority,
            "diagnostic_habitability_context_not_comfort_gate_control"
        );
    }

    #[test]
    fn clamp_unit_finite_or_clamps_boundaries_and_uses_fallback_for_non_finite() {
        assert_eq!(clamp_unit_finite_or(-0.25, 0.50), 0.0);
        assert_eq!(clamp_unit_finite_or(1.25, 0.50), 1.0);
        assert_eq!(clamp_unit_finite_or(0.42, 0.50), 0.42);
        assert_eq!(clamp_unit_finite_or(f32::NAN, 0.50), 0.50);
        assert_eq!(clamp_unit_finite_or(f32::INFINITY, 0.50), 0.50);
    }

    #[test]
    fn clamp_unit_finite_rejects_nonfinite_and_clamps_unit_range() {
        assert_eq!(clamp_unit_finite(-0.25), Some(0.0));
        assert_eq!(clamp_unit_finite(1.25), Some(1.0));
        assert_eq!(clamp_unit_finite(0.42), Some(0.42));
        assert_eq!(clamp_unit_finite(f32::NAN), None);
        assert_eq!(clamp_unit_finite(f32::INFINITY), None);
        assert_eq!(clamp_unit_finite(f32::NEG_INFINITY), None);
    }

    #[test]
    fn clamped_unit_review_preserves_clipping_truth_without_live_authority() {
        let high = clamped_unit_review_v1(1.25, 0.50);
        let non_finite = clamped_unit_review_v1(f32::NAN, 0.50);

        assert_eq!(high.policy, "clamped_unit_review_v1");
        assert_eq!(high.raw_value, Some(1.25));
        assert_eq!(high.clamped_value, 1.0);
        assert_eq!(high.clip_state, "clipped_high");
        assert!(high.clipped_high);
        assert!(!high.clipped_low);
        assert!(!high.non_finite_rejected);
        assert!(!high.live_vector_write);
        assert!(!high.live_authority_write);
        assert_eq!(
            high.authority,
            "read_only_clamp_visibility_not_live_vector_or_authority_change"
        );

        assert_eq!(non_finite.raw_value, None);
        assert_eq!(non_finite.clamped_value, 0.50);
        assert_eq!(non_finite.clip_state, "non_finite_rejected_to_fallback");
        assert!(non_finite.non_finite_rejected);

        let encoded = serde_json::to_string(&high).unwrap();
        assert!(encoded.contains("\"clip_state\":\"clipped_high\""));
        assert!(encoded.contains("\"live_vector_write\":false"));
        assert!(encoded.contains("\"live_authority_write\":false"));
    }

    #[test]
    fn clamped_unit_review_flags_uncomputable_fallback_without_changing_clamp_policy() {
        let review = clamped_unit_review_v1(f32::NAN, f32::INFINITY);

        assert_eq!(review.raw_value, None);
        assert_eq!(review.raw_fallback, None);
        assert_eq!(review.fallback_value, 0.0);
        assert!(!review.fallback_finite);
        assert!(review.fallback_non_finite_defaulted);
        assert_eq!(
            review.fallback_intent_state,
            "uncomputable_fallback_defaulted_to_zero"
        );
        assert_eq!(review.clamped_value, 0.0);
        assert_eq!(review.clip_state, "non_finite_rejected_to_fallback");
        assert!(!review.live_vector_write);
        assert!(!review.live_authority_write);

        let encoded = serde_json::to_string(&review).unwrap();
        assert!(encoded.contains("\"fallback_non_finite_defaulted\":true"));
        assert!(encoded.contains("uncomputable_fallback_defaulted_to_zero"));
    }

    #[test]
    fn comfort_gate_range_widens_for_pressure_and_packing_without_control_authority() {
        let calm_components = ResonanceDensityComponents {
            active_energy: 0.64,
            mode_packing: 0.10,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.54,
            viscosity_index: 0.42,
            viscosity_persistence_coefficient: 0.31,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.48),
            porosity_gradient: Some(0.58),
            dynamic_fluidity_index: Some(0.60),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.70,
            comfort_gate: 0.50,
            comfort_gate_range: None,
        };
        let packed_components = ResonanceDensityComponents {
            mode_packing: 0.50,
            ..calm_components.clone()
        };

        let calm = resonance_comfort_gate_range_v1(&calm_components, Some(0.10), Some(0.05));
        let packed = resonance_comfort_gate_range_v1(&packed_components, Some(0.22), Some(0.38));

        assert_eq!(calm.policy, "comfort_gate_range_v1");
        assert_eq!(calm.range_state, "comfort_gate_range_watch");
        assert_eq!(packed.range_state, "dynamic_pressure_buffer_range");
        assert!(
            packed.width > calm.width + 0.05,
            "pressure and packing should make the diagnostic range visibly wider: calm={calm:?} packed={packed:?}"
        );
        assert_eq!(
            packed.authority,
            "diagnostic_gate_range_not_fill_pressure_pi_or_control"
        );
    }

    #[test]
    fn resonance_cohesion_score_names_shape_holding_without_control_authority() {
        let components = ResonanceDensityComponents {
            active_energy: 0.62,
            mode_packing: 0.29,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.66,
            viscosity_index: 0.58,
            viscosity_persistence_coefficient: 0.54,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: Some(0.42),
            porosity_gradient: Some(0.66),
            dynamic_fluidity_index: Some(0.61),
            semantic_friction_coefficient: Some(0.22),
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.72,
            comfort_gate: 0.70,
            comfort_gate_range: None,
        };
        let score = resonance_cohesion_score_v1(&components);

        assert!(
            (0.60..=0.75).contains(&score),
            "cohesion score should preserve habitable shape without becoming a control target: {score}"
        );
        let integrity = resonance_structural_integrity_index_v1(&components);
        assert!(
            (0.60..=0.80).contains(&integrity),
            "structural integrity should include dissipation/fluidity without becoming control authority: {integrity}"
        );
        let explicit_integrity = ResonanceDensityComponents {
            structural_integrity_index: Some(0.23),
            structural_transparency_index: None,
            ..components.clone()
        };
        assert_eq!(
            resonance_structural_integrity_index_v1(&explicit_integrity),
            0.23
        );
        let explicit = ResonanceDensityComponents {
            cohesion_score: Some(0.91),
            ..components
        };
        assert_eq!(resonance_cohesion_score_v1(&explicit), 0.91);
    }

    #[test]
    fn resonance_cohesion_score_defaults_missing_fluidity_to_midpoint() {
        let components = ResonanceDensityComponents {
            active_energy: 0.50,
            mode_packing: 0.45,
            coupling_coefficient: 0.0,
            temporal_persistence: 0.50,
            viscosity_index: 0.50,
            viscosity_persistence_coefficient: 0.50,
            viscosity_vector: ResonanceViscosityVectorV1::default(),
            dissipation_factor: None,
            porosity_gradient: Some(0.50),
            dynamic_fluidity_index: None,
            semantic_friction_coefficient: None,
            cohesion_score: None,
            structural_integrity_index: None,
            structural_transparency_index: None,
            stability_context: None,
            structural_plurality: 0.50,
            comfort_gate: 0.50,
            comfort_gate_range: None,
        };

        let score = resonance_cohesion_score_v1(&components);

        assert!(
            (score - 0.60).abs() <= 0.0001,
            "missing dynamic_fluidity_index and dissipation_factor should use midpoint fluidity: {score}"
        );
    }

    #[test]
    fn resonance_stability_context_old_rows_default_weight_fields() {
        let context: ResonanceStabilityContextV1 = serde_json::from_value(serde_json::json!({
            "policy": "resonance_stability_context_v1",
            "schema_version": 1,
            "comfort_gate": 0.62,
            "habitability_state": "comfort_gate_only",
            "gate_context": "gate_buffering_context_incomplete",
            "gate_closure_reason": "not_closed",
            "authority": "diagnostic_habitability_context_not_comfort_gate_control"
        }))
        .expect("legacy stability context should deserialize");

        assert_eq!(context.weight_policy, "legacy_unversioned_weights");
        assert_eq!(context.weights, None);
        assert!(!context.partial_habitability_score);
        assert_eq!(context.multi_modal_habitability_evidence_count, 0);
        assert!(
            context
                .multi_modal_habitability_missing_components
                .is_empty()
        );
        assert_eq!(context.multi_modal_habitability_score_basis, None);
    }

    #[test]
    fn pressure_trend_v1_old_payload_defaults_timing_fields() {
        let trend: PressureTrendV1 = serde_json::from_value(serde_json::json!({
            "policy": "pressure_trend_v1",
            "schema_version": 1,
            "classification": "stable_heavy",
            "latest_pressure_risk": 0.2,
            "previous_pressure_risk": 0.2,
            "pressure_delta": 0.0,
            "latest_fill_pct": 68.0,
            "previous_fill_pct": 68.0,
            "fill_delta_pct": 0.0
        }))
        .unwrap();

        assert_eq!(trend.classification, "stable_heavy");
        assert!(trend.timing_reliability.is_none());
        assert!(trend.telemetry_inter_arrival_ms.is_none());
        assert!(trend.heartbeat_jitter_class.is_none());
        assert!(trend.field_vs_hearing.is_none());
        assert!(trend.latest_spectral_entropy.is_none());
        assert!(trend.viscosity_coefficient.is_none());
        assert!(trend.pressure_interpretation.is_none());
        assert!(trend.latest_resonance_depth.is_none());
        assert!(trend.previous_resonance_depth.is_none());
        assert!(trend.resonance_depth_delta.is_none());
        assert!(trend.latest_semantic_viscosity.is_none());
        assert!(trend.semantic_viscosity_state.is_none());
        assert!(trend.latest_complexity_density.is_none());
        assert!(trend.complexity_density_state.is_none());
    }

    #[test]
    fn pressure_status_old_payloads_default_semantic_stagnation_fields() {
        let smoothing: PressureTrendSmoothingV1 = serde_json::from_value(serde_json::json!({
            "policy": "pressure_trend_smoothing_v1",
            "schema_version": 1,
            "classification": "low_amplitude_stable",
            "sample_count": 3,
            "window_capacity": 5,
            "window_policy": "latest_up_to_5_telemetry_samples",
            "authority": "diagnostic_smoothing_not_pressure_control"
        }))
        .unwrap();
        assert!(smoothing.semantic_stagnation_index.is_none());
        assert!(smoothing.semantic_stagnation_state.is_empty());
        assert!(smoothing.latest_complexity_density.is_none());
        assert!(smoothing.max_complexity_density.is_none());

        let analysis: PressureSourceAnalysisV1 = serde_json::from_value(serde_json::json!({
            "policy": "pressure_source_analysis_v1",
            "schema_version": 1,
            "status": "pressure_source_visible",
            "structural_pressure_state": "pressure_source_visible",
            "ghost_stability_risk": "low",
            "analysis": "source=unknown",
            "authority": "diagnostic_context_not_pressure_or_control"
        }))
        .unwrap();
        assert!(analysis.semantic_stagnation_index.is_none());
        assert!(analysis.semantic_stagnation_state.is_none());
    }

    #[test]
    fn inhabitable_fluctuation_pressure_calibration_maps_components_to_adjusted_score() {
        let calibration = InhabitableFluctuationPressureCalibrationV1 {
            raw_motion_score: 0.72,
            pressure_contribution: 0.18,
            adjusted_fluctuation_score: 0.54,
            ..InhabitableFluctuationPressureCalibrationV1::default()
        };

        assert_eq!(
            calibration.rigid_safety_basis,
            INHABITABLE_FLUCTUATION_RIGID_SAFETY_BASIS
        );
        assert!((calibration.expected_adjusted_fluctuation_score() - 0.54).abs() <= 0.001);
        assert!(calibration.adjusted_score_matches_components());

        let drifted = InhabitableFluctuationPressureCalibrationV1 {
            adjusted_fluctuation_score: 0.72,
            ..calibration
        };
        assert!(!drifted.adjusted_score_matches_components());
    }

    #[test]
    fn parse_minime_eigenpacket_minimal() {
        // Minimal valid EigenPacket (no optional fields).
        let json = r#"{
            "t_ms": 1000,
            "eigenvalues": [512.0],
            "fill_ratio": 0.0
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 1000);
        assert!((telemetry.lambda1() - 512.0).abs() < 0.01);
        assert!((telemetry.fill_pct() - 0.0).abs() < 0.01);
        assert!(telemetry.modalities.is_none());
        assert!(telemetry.neural.is_none());
        assert!(telemetry.alert.is_none());
        assert!(telemetry.active_mode_count.is_none());
        assert!(telemetry.typed_fingerprint().is_none());
        let denominator = telemetry.denominator_metrics().unwrap();
        assert!((denominator.effective_dimensionality - 1.0).abs() < 0.01);
        assert_eq!(denominator.active_mode_capacity, 1);
        assert!((denominator.distinguishability_loss - 0.0).abs() < 0.01);
    }

    #[test]
    fn old_fingerprint_payload_reconstructs_typed_view() {
        let legacy = (0..32).map(|value| value as f32).collect::<Vec<_>>();
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": legacy,
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let typed = telemetry.typed_fingerprint().unwrap();

        assert_eq!(typed.spectral_entropy, 24.0);
        assert_eq!(typed.lambda1_lambda2_gap, 25.0);
        assert_eq!(typed.v1_rotation_similarity, 26.0);
        assert_eq!(typed.geom_rel, 27.0);
        assert_eq!(typed.adjacent_gap_ratios, [28.0, 29.0, 30.0, 31.0]);
        assert!(telemetry.denominator_metrics().is_some());
        let integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(integrity.status, "legacy_32d_accepted");
        assert_eq!(integrity.legacy_vector_len, Some(32));
        assert!(integrity.summary.contains("reconstruct"));
    }

    #[test]
    fn typed_fingerprint_takes_precedence_over_legacy_slots() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": vec![0.0_f32; 32],
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "spectral_entropy": 0.42,
                "lambda1_lambda2_gap": 2.0,
                "v1_rotation_similarity": 0.9,
                "v1_rotation_delta": 0.1,
                "geom_rel": 1.23,
                "adjacent_gap_ratios": [2.0, 1.0, 1.0, 1.0]
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let typed = telemetry.typed_fingerprint().unwrap();

        assert_eq!(typed.geom_rel, 1.23);
        assert_eq!(typed.spectral_entropy, 0.42);
        let integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(integrity.status, "typed_canonical");
        assert!(integrity.typed_precedence_over_legacy);
        assert!(integrity.summary.contains("typed payload takes precedence"));
    }

    #[test]
    fn malformed_legacy_fingerprint_reports_integrity_issue() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.5,
            "spectral_fingerprint": vec![0.0_f32; 31],
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        assert!(telemetry.typed_fingerprint().is_none());
        let integrity = telemetry.spectral_fingerprint_integrity_v1();
        assert_eq!(integrity.status, "malformed_legacy_vector");
        assert_eq!(integrity.legacy_vector_len, Some(31));
        assert!(
            integrity
                .issues
                .contains(&"legacy_vector_len_31_expected_32".to_string())
        );
        assert_eq!(integrity.authority, "diagnostic_context_not_control");
    }

    #[test]
    fn typed_transition_eigenvector_and_semantic_views_are_lenient() {
        let json = serde_json::json!({
            "t_ms": 1000,
            "eigenvalues": [4.0, 2.0, 1.0],
            "fill_ratio": 0.66,
            "semantic": {
                "energy": 0.0,
                "kernel_energy": 0.0,
                "input_energy": 0.12,
                "input_active": true,
                "admission": "stable_core_kernel_zeroed"
            },
            "semantic_energy_v1": {
                "policy": "semantic_energy_v1",
                "schema_version": 1,
                "input_energy": 0.14,
                "input_active": true,
                "input_fresh_ms": 120,
                "input_stale_ms": null,
                "kernel_energy": 0.0,
                "kernel_delta": 0.0,
                "kernel_active": false,
                "regulator_drive_energy": 0.0,
                "admission": "stable_core_kernel_zeroed"
            },
            "transition_event_v1": {
                "policy": "transition_event_v1",
                "schema_version": 1,
                "kind": "breathing_phase",
                "description": "contracting -> expanding",
                "basin_shift_score": 0.05,
                "lambda1_rel": 0.93,
                "geom_rel": 1.02
            },
            "eigenvector_field": {
                "policy": "eigenvector_field_v1",
                "mode_count": 2,
                "reservoir_dim": 512,
                "summary": {
                    "mean_orientation_delta": 0.12,
                    "max_pairwise_overlap": 0.03
                },
                "modes": [{
                    "index": 1,
                    "eigenvalue": 4.0,
                    "top_components": [{"index": 7, "value": -0.5, "abs": 0.5}]
                }]
            }
        });

        let telemetry: SpectralTelemetry = serde_json::from_value(json).unwrap();
        let semantic = telemetry.semantic_energy_view().unwrap();
        let transition = telemetry.transition_event_view().unwrap();
        let field = telemetry.eigenvector_field_view().unwrap();

        assert_eq!(semantic.input_energy, 0.14);
        assert_eq!(semantic.regulator_drive_energy, 0.0);
        assert_eq!(transition.kind, "breathing_phase");
        assert_eq!(field.mode_count, 2);
        assert_eq!(field.modes[0].top_components[0].index, 7);
    }

    #[test]
    fn parse_minime_eigenpacket_with_alert() {
        let json = r#"{
            "t_ms": 50000,
            "eigenvalues": [1020.0, 500.0],
            "fill_ratio": 0.99,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            },
            "alert": "PANIC MODE ACTIVATED"
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert!((telemetry.fill_pct() - 99.0).abs() < 0.1);
        assert_eq!(telemetry.alert.as_deref(), Some("PANIC MODE ACTIVATED"));
    }

    #[test]
    fn spectral_telemetry_roundtrip() {
        let orig = SpectralTelemetry {
            t_ms: 12345,
            eigenvalues: vec![828.5, 312.1, 45.7],
            fill_ratio: 0.55,
            active_mode_count: Some(2),
            active_mode_energy_ratio: Some(0.95),
            lambda1_rel: Some(0.88),
            modalities: Some(ModalityStatus {
                audio_fired: true,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.1,
                video_var: 0.0,
                ..ModalityStatus::default()
            }),
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            stable_core: Some(serde_json::json!({
                "sensory_budget": {
                    "ears_open": true,
                    "eyes_open": true,
                    "live_intake_reason": "test_presence"
                }
            })),
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
            residual_deformation_trace_v1: None,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: SpectralTelemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.t_ms, orig.t_ms);
        assert_eq!(back.eigenvalues.len(), 3);
        assert!((back.fill_ratio - orig.fill_ratio).abs() < 0.001);
        assert_eq!(back.active_mode_count, Some(2));
        assert_eq!(back.active_mode_energy_ratio, Some(0.95));
        assert_eq!(back.lambda1_rel, Some(0.88));
        let sensory_budget = back
            .stable_core
            .as_ref()
            .and_then(|stable_core| stable_core.get("sensory_budget"))
            .expect("stable_core.sensory_budget should roundtrip");
        assert_eq!(
            sensory_budget
                .get("live_intake_reason")
                .and_then(serde_json::Value::as_str),
            Some("test_presence")
        );
    }

    #[test]
    fn spectral_telemetry_accepts_glimpse_alias_and_validates_shape() {
        let glimpse = (0..12).map(|idx| idx as f32 / 10.0).collect::<Vec<_>>();
        let telemetry: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 123,
            "eigenvalues": [1.0, 0.5, 0.25],
            "fill_ratio": 0.68,
            "glimpse_12d": glimpse,
        }))
        .expect("telemetry with proposal alias");

        let validated = telemetry
            .spectral_glimpse_12d_view()
            .expect("12D alias should validate");
        assert_eq!(validated.len(), 12);
        assert!((validated[11] - 1.1).abs() < f32::EPSILON);

        let malformed: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 124,
            "eigenvalues": [1.0],
            "fill_ratio": 0.68,
            "glimpse_12d": [0.0, 1.0, 2.0],
        }))
        .expect("telemetry with malformed additive field");
        assert!(malformed.spectral_glimpse_12d.is_some());
        assert!(malformed.spectral_glimpse_12d_view().is_none());
    }

    #[test]
    fn spectral_telemetry_keeps_glimpse_additive_to_typed_fingerprint() {
        let glimpse = (0..12)
            .map(|idx| (idx as f32 + 1.0) / 20.0)
            .collect::<Vec<_>>();
        let telemetry: SpectralTelemetry = serde_json::from_value(serde_json::json!({
            "t_ms": 125,
            "eigenvalues": [1.0, 0.5],
            "fill_ratio": 0.68,
            "glimpse_12d": glimpse,
            "spectral_fingerprint_v1": {
                "policy": "spectral_fingerprint_v1",
                "schema_version": 1,
                "eigenvalues": [1.0, 0.5, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0],
                "eigenvector_concentration_top4": [0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
                "inter_mode_cosine_top_abs": [0.1, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
                "spectral_entropy": 0.77,
                "lambda1_lambda2_gap": 0.5,
                "v1_rotation_similarity": 0.91,
                "v1_rotation_delta": 0.09,
                "geom_rel": 1.12,
                "adjacent_gap_ratios": [2.0, 2.0, 1.0, 1.0]
            }
        }))
        .expect("telemetry with typed fingerprint and additive glimpse");

        let typed = telemetry
            .typed_fingerprint()
            .expect("typed 32D fingerprint remains canonical");
        let glimpse = telemetry
            .spectral_glimpse_12d_view()
            .expect("12D glimpse remains separately validated");
        let integrity = telemetry.spectral_fingerprint_integrity_v1();

        assert_eq!(typed.spectral_entropy, 0.77);
        assert_eq!(typed.geom_rel, 1.12);
        assert_eq!(glimpse.len(), 12);
        assert!((glimpse[0] - 0.05).abs() < f32::EPSILON);
        assert_eq!(integrity.status, "typed_canonical");
        assert!(!integrity.typed_precedence_over_legacy);
    }

    // -- SensoryMsg: verify wire format matches minime's sensory_ws.rs --

    #[test]
    fn sensory_msg_video_roundtrip() {
        let msg = SensoryMsg::Video {
            features: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
            ts_ms: Some(1000),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Must have "kind":"video" tag per minime's serde config.
        assert!(json.contains(r#""kind":"video""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Video { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(1000));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_semantic_roundtrip() {
        let msg = SensoryMsg::Semantic {
            features: vec![0.5; 32],
            ts_ms: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"semantic""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features.len(), 32);
                assert!(ts_ms.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_attractor_pulse_roundtrip() {
        let msg = SensoryMsg::AttractorPulse {
            intent_id: "intent-main".to_string(),
            label: "cooled edge".to_string(),
            command: "summon".to_string(),
            stage: Some("main".to_string()),
            features: vec![0.01; 66],
            max_abs: Some(0.045),
            duration_ticks: Some(36),
            decay_ticks: Some(12),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"attractor_pulse""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::AttractorPulse {
                intent_id,
                label,
                stage,
                features,
                max_abs,
                duration_ticks,
                decay_ticks,
                ..
            } => {
                assert_eq!(intent_id, "intent-main");
                assert_eq!(label, "cooled edge");
                assert_eq!(stage.as_deref(), Some("main"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.045));
                assert_eq!(duration_ticks, Some(36));
                assert_eq!(decay_ticks, Some(12));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_shadow_influence_roundtrip() {
        let msg = SensoryMsg::ShadowInfluence {
            intent_id: "shadow-live".to_string(),
            label: "lambda-tail/lambda4".to_string(),
            command: "apply".to_string(),
            stage: Some("live".to_string()),
            features: vec![0.01; 66],
            max_abs: Some(0.025),
            duration_ticks: Some(24),
            decay_ticks: Some(12),
            basis: Some("lambda-tail/lambda4".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"shadow_influence""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::ShadowInfluence {
                intent_id,
                label,
                stage,
                features,
                max_abs,
                duration_ticks,
                decay_ticks,
                basis,
                ..
            } => {
                assert_eq!(intent_id, "shadow-live");
                assert_eq!(label, "lambda-tail/lambda4");
                assert_eq!(stage.as_deref(), Some("live"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.025));
                assert_eq!(duration_ticks, Some(24));
                assert_eq!(decay_ticks, Some(12));
                assert_eq!(basis.as_deref(), Some("lambda-tail/lambda4"));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_control_roundtrip() {
        let msg = SensoryMsg::Control {
            synth_gain: Some(1.5),
            keep_bias: None,
            exploration_noise: Some(0.1),
            fill_target: Some(0.55),
            legacy_audio_synth: None,
            legacy_video_synth: None,
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: None,
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            mem_mode: None,
            journal_resonance: None,
            checkpoint_interval: None,
            embedding_strength: None,
            memory_decay_rate: None,
            checkpoint_annotation: None,
            synth_noise_level: None,
            pi_kp: None,
            pi_ki: None,
            pi_max_step: None,
            pi_integrator_leak: None,
            esn_leak_override: None,
            esn_leak_override_ticks: None,
            esn_leak_authority_request_id: None,
            mode_disperse: None,
            mode_disperse_duration_ticks: None,
            mode_disperse_decay_ticks: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"control""#));
        assert!(!json.contains("keep_bias"));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Control {
                synth_gain,
                keep_bias,
                exploration_noise,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(1.5));
                assert!(keep_bias.is_none());
                assert_eq!(exploration_noise, Some(0.1));
                assert_eq!(fill_target, Some(0.55));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_parse_from_minime_format() {
        // Simulates JSON that minime's sensory_ws.rs would accept.
        let json = r#"{"kind":"audio","features":[0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8],"ts_ms":500}"#;
        let msg: SensoryMsg = serde_json::from_str(json).unwrap();
        match msg {
            SensoryMsg::Audio { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(500));
            },
            _ => panic!("wrong variant"),
        }
    }

    // -- Safety level --

    #[test]
    fn safety_level_roundtrip() {
        for level in [
            SafetyLevel::Green,
            SafetyLevel::Yellow,
            SafetyLevel::Orange,
            SafetyLevel::Red,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: SafetyLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(back, level);
        }
    }

    // -- Control and Semantic conversion --

    #[test]
    fn control_request_to_sensory_msg() {
        let req = ControlRequest {
            synth_gain: Some(2.0),
            keep_bias: None,
            exploration_noise: None,
            fill_target: Some(0.5),
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: None,
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            memory_decay_rate: None,
            pi_kp: None,
            pi_ki: None,
            pi_max_step: None,
            pi_integrator_leak: None,
            attractor_intent_id: None,
        };
        let msg = req.to_sensory_msg();
        match msg {
            SensoryMsg::Control {
                synth_gain,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(2.0));
                assert_eq!(fill_target, Some(0.5));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn control_request_marks_bold_attractor_fields() {
        let req = ControlRequest {
            synth_gain: None,
            keep_bias: None,
            exploration_noise: None,
            fill_target: None,
            regulation_strength: None,
            deep_breathing: None,
            pure_tone: None,
            transition_cushion: None,
            smoothing_preference: None,
            geom_curiosity: None,
            target_lambda_bias: Some(0.03),
            geom_drive: None,
            penalty_sensitivity: None,
            breathing_rate_scale: None,
            memory_decay_rate: None,
            pi_kp: Some(0.12),
            pi_ki: None,
            pi_max_step: Some(0.02),
            pi_integrator_leak: None,
            attractor_intent_id: Some("intent-1".to_string()),
        };
        assert!(req.uses_bold_attractor_fields());
        match req.to_sensory_msg() {
            SensoryMsg::Control {
                target_lambda_bias,
                pi_kp,
                pi_max_step,
                ..
            } => {
                assert!((target_lambda_bias.unwrap_or_default() - 0.03).abs() < f32::EPSILON);
                assert!((pi_kp.unwrap_or_default() - 0.12).abs() < f32::EPSILON);
                assert!((pi_max_step.unwrap_or_default() - 0.02).abs() < f32::EPSILON);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn attractor_classification_from_scores() {
        assert_eq!(
            AttractorClassification::from_scores(0.71, 0.66, SafetyLevel::Green),
            AttractorClassification::Authored
        );
        assert_eq!(
            AttractorClassification::from_scores(0.50, 0.20, SafetyLevel::Yellow),
            AttractorClassification::Emergent
        );
        assert_eq!(
            AttractorClassification::from_scores(0.20, 0.95, SafetyLevel::Green),
            AttractorClassification::Failed
        );
        assert_eq!(
            AttractorClassification::from_scores(0.80, 0.80, SafetyLevel::Red),
            AttractorClassification::Pathological
        );
    }

    #[test]
    fn attractor_intent_and_observation_roundtrip() {
        let intent = AttractorIntentV1 {
            policy: "attractor_intent_v1".to_string(),
            schema_version: 1,
            intent_id: "seed-001".to_string(),
            author: "astrid".to_string(),
            substrate: AttractorSubstrate::TripleReservoir,
            command: AttractorCommandKind::Create,
            label: "quiet eigenplane".to_string(),
            goal: Some("return after hold".to_string()),
            intervention_plan: AttractorInterventionPlan {
                mode: "garden_clone".to_string(),
                vector_schedule: vec![vec![0.1, -0.1, 0.0]],
                control: Some(AttractorControlEnvelope {
                    exploration_noise: Some(0.03),
                    geom_drive: Some(0.25),
                    ..AttractorControlEnvelope::default()
                }),
                rehearsal_mode: Some("hold".to_string()),
                notes: None,
            },
            safety_bounds: AttractorSafetyBounds::default(),
            previous_seed_id: None,
            parent_seed_ids: vec!["parent-a".to_string(), "parent-b".to_string()],
            atlas_entry_id: Some("attr-triple-reservoir-quiet-eigenplane".to_string()),
            parent_label: Some("quiet".to_string()),
            facet_label: Some("eigenplane".to_string()),
            facet_path: Some("quiet/eigenplane".to_string()),
            facet_kind: Some("test_facet".to_string()),
            origin: Some(AttractorSeedOriginV1 {
                kind: "manual_current".to_string(),
                source: None,
                matched_label: Some("quiet eigenplane".to_string()),
                motifs: vec!["quiet".to_string(), "eigenplane".to_string()],
                captured_at_unix_s: Some(1.0),
            }),
            seed_snapshot: Some(AttractorSeedSnapshotV1 {
                policy: "attractor_seed_snapshot_v1".to_string(),
                schema_version: 1,
                fill_pct: 67.5,
                lambda1: 4.2,
                eigenvalues: vec![4.2, 2.0, 1.0],
                spectral_fingerprint_summary: Some(vec![0.1, 0.2]),
                h_state_fingerprint_16: None,
                h_state_rms: None,
                lexical_motifs: vec!["quiet".to_string(), "eigenplane".to_string()],
                captured_at_unix_s: Some(1.0),
            }),
            created_at_unix_s: Some(1.0),
        };
        let json = serde_json::to_string(&intent).unwrap();
        let back: AttractorIntentV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.intent_id, "seed-001");
        assert_eq!(back.substrate, AttractorSubstrate::TripleReservoir);
        assert_eq!(back.intervention_plan.vector_schedule.len(), 1);
        assert_eq!(back.parent_seed_ids.len(), 2);
        assert_eq!(
            back.atlas_entry_id.as_deref(),
            Some("attr-triple-reservoir-quiet-eigenplane")
        );
        assert_eq!(back.facet_path.as_deref(), Some("quiet/eigenplane"));
        assert_eq!(back.origin.as_ref().unwrap().kind, "manual_current");
        let snapshot = back.seed_snapshot.as_ref().expect("seed snapshot");
        assert_eq!(
            snapshot.lexical_motifs,
            vec!["quiet".to_string(), "eigenplane".to_string()]
        );

        let observation = AttractorObservationV1 {
            policy: "attractor_observation_v1".to_string(),
            schema_version: 1,
            intent_id: Some(back.intent_id),
            substrate: AttractorSubstrate::TripleReservoir,
            label: back.label,
            recurrence_score: 0.72,
            authorship_score: 0.61,
            classification: AttractorClassification::Authored,
            safety_level: SafetyLevel::Green,
            fill_pct: Some(67.5),
            lambda1: Some(4.2),
            lambda1_share: Some(0.34),
            spectral_entropy: Some(0.78),
            basin_shift_score: Some(0.18),
            notes: None,
            parent_label: Some("quiet".to_string()),
            facet_label: Some("eigenplane".to_string()),
            facet_path: Some("quiet/eigenplane".to_string()),
            facet_kind: Some("test_facet".to_string()),
            release_baseline: Some(serde_json::json!({"pulse_active": false})),
            release_effect: Some("partial".to_string()),
            garden_proof: Some(serde_json::json!({"same_prompt_different_state": "not_run"})),
            observed_at_unix_s: Some(2.0),
        };
        let json = serde_json::to_string(&observation).unwrap();
        let back: AttractorObservationV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.classification, AttractorClassification::Authored);
        assert!((back.recurrence_score - 0.72).abs() < f32::EPSILON);
        assert_eq!(back.release_effect.as_deref(), Some("partial"));
    }

    #[test]
    fn semantic_features_to_sensory_msg() {
        let feat = SemanticFeatures {
            features: vec![1.0, 2.0, 3.0],
        };
        let msg = feat.to_sensory_msg();
        match msg {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features, vec![1.0, 2.0, 3.0]);
                assert!(ts_ms.is_none());
            },
            _ => panic!("wrong variant"),
        }
    }
}
