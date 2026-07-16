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
    /// A release transition where previously viscous or resistant flow becomes
    /// more orderly without implying that pressure or control was changed.
    Laminarization,
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
    Ambiguity,
    Divergence,
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
    /// Sum before the compatibility score is clamped to one. This keeps
    /// simultaneous high-weight kinds visible instead of flattening overlap.
    #[serde(default)]
    pub unclamped_weight_sum: f32,
    /// Mean member weight in `[0, 1]`, independent of the number of kinds.
    #[serde(default)]
    pub weight_density: f32,
    /// Amount by which the member sum exceeds the compatibility ceiling.
    #[serde(default)]
    pub saturation_excess: f32,
    #[serde(default)]
    pub composite_score_saturated: bool,
    #[serde(default = "default_delta_composition_saturation_state")]
    pub saturation_state: String,
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

fn default_delta_composition_saturation_state() -> String {
    "not_reported_legacy".to_string()
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

    let unclamped_weight_sum = members.iter().map(|member| member.weight).sum::<f32>();
    let composite_score = unclamped_weight_sum.clamp(0.0, 1.0);
    let member_capacity = members.iter().map(|_| 1.0_f32).sum::<f32>();
    let weight_density = if member_capacity > f32::EPSILON {
        clamp_unit_finite_or(unclamped_weight_sum / member_capacity, 0.0)
    } else {
        0.0
    };
    let saturation_excess = (unclamped_weight_sum - 1.0).max(0.0);
    let composite_score_saturated = saturation_excess > f32::EPSILON;
    let saturation_state = if composite_score_saturated {
        "saturated_overlap_visible"
    } else if composite_score >= 1.0 {
        "at_ceiling_without_excess"
    } else {
        "unsaturated"
    };
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
        unclamped_weight_sum,
        weight_density,
        saturation_excess,
        composite_score_saturated,
        saturation_state: saturation_state.to_string(),
        state: state.to_string(),
        members,
        evidence_window: "bounded_current_packet_delta_composition".to_string(),
        basis: BTreeMap::from([
            (
                "source_introspection".to_string(),
                "introspection_astrid_types_1784139137;introspection_astrid_types_1784122683;introspection_astrid_types_1784114716".to_string(),
            ),
            (
                "composition_rule".to_string(),
                "coexisting_delta_weights_are_truth_channel_evidence_not_enum_replacement"
                    .to_string(),
            ),
            (
                "saturation_rule".to_string(),
                "compatibility_score_remains_clamped_while_unclamped_sum_density_and_excess_preserve_overlap"
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
            "laminarization".to_string(),
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
