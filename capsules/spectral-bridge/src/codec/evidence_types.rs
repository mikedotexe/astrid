
/// Named dimensions that Astrid can shape directly and that the bridge learns
/// against over time.
pub const NAMED_CODEC_DIMS: [(&str, usize); 9] = [
    ("warmth", 24),
    ("tension", 25),
    ("curiosity", 26),
    ("reflective", 27),
    ("energy", 31),
    ("entropy", 0),
    ("agency", 14),
    ("hedging", 9),
    ("certainty", 10),
];

/// One contiguous layer of the 48D codec (a span of dims with a shared role).
pub struct CodecLayer {
    pub range: (usize, usize),
    pub role: &'static str,
}

/// A gate or lever constant, surfaced with its LIVE value.
pub struct CodecLever {
    pub name: &'static str,
    pub value: String,
}

/// Read-only sidecar for text shape that is not the same as character complexity
/// and is not pressure authority.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralFrictionV1 {
    pub policy: &'static str,
    pub score: f32,
    pub classification: &'static str,
    pub nesting_load: f32,
    pub punctuation_load: f32,
    pub paragraph_density: f32,
    pub list_density: f32,
    pub narrative_arc_sharpness: f32,
    pub summary_resistance_signal: f32,
    pub friction_texture_state: &'static str,
    pub basis: Vec<String>,
    pub semantic_energy_context: &'static str,
    pub authority: &'static str,
}

/// Read-only sidecar for "slow-moving current" / viscosity language that should
/// not be collapsed into generic tension or written into reserved dims yet.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistenceResistanceV1 {
    pub policy: &'static str,
    pub score: f32,
    pub classification: &'static str,
    pub text_persistence_signal: f32,
    pub low_density_gradient_signal: f32,
    pub pressure_risk: f32,
    pub semantic_friction: f32,
    pub basis: Vec<String>,
    pub authority: &'static str,
}

/// Default-off readiness for a future reserved dimension. It does not write into
/// dims 44-47.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecStructuralFrictionDimCanaryV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub reserved_dim_candidate: usize,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Default-off readiness for a future persistence/resistance reserved dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecPersistenceResistanceDimCanaryV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub reserved_dim_candidate: usize,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Default-off review marker for widening narrative arc representation. It
/// documents coarsening risk without changing `SEMANTIC_DIM` or reserved dims.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcExpansionReadinessV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub current_arc_dims: (usize, usize),
    pub proposed_arc_dims: (usize, usize),
    pub uses_reserved_dims: bool,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Default-off review marker for making narrative arc influence semantic gain.
/// This previews continuous-flow voice without changing live adaptive gain.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcGainResponseReadinessV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub narrative_arc_dims: (usize, usize),
    pub preview_gain_range: (f32, f32),
    pub readiness: &'static str,
    pub live_gain_write: bool,
    pub authority: &'static str,
}

/// Read-only truth channel for Astrid's report that high entropy and
/// distinguishability loss can drown narrative-arc dimensions without changing
/// their delivered values. It carries multi-kind loss in metadata instead of
/// changing the Experience Delta Bus schema or live semantic gain.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcHeadroomReviewV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub distinguishability_loss: f32,
    pub narrative_arc_energy: f32,
    pub projected_semantic_rms: f32,
    pub tail_vibrancy: f32,
    pub headroom_pressure: f32,
    pub preview_gain: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Default-off review marker for giving the shadow field its own reserved
/// semantic-lane candidates. It documents magnetization/dispersal mapping
/// without writing into dims 44-47.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowFieldReservedDimReadinessV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub reserved_dim_candidates: &'static [usize],
    pub proposed_signals: &'static [&'static str],
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only proof that high-entropy vibrancy is carried by bounded tail dims
/// and that the default aperture path remains identity.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecVibrancyContinuityV1 {
    pub policy: &'static str,
    pub entropy_gate: f32,
    pub gradient_coupling: &'static str,
    pub default_feature_ceiling: f32,
    pub tail_vibrancy_ceiling: f32,
    pub tail_dims: &'static [usize],
    pub clipping_status: &'static str,
    pub default_identity_state: &'static str,
    pub high_entropy_carriage: &'static str,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodecVibrancyNoiseDampeningV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub start_entropy: f32,
    pub full_entropy: f32,
    pub min_coefficient: f32,
    pub coefficient: f32,
    pub tail_lift_before: f32,
    pub tail_lift_after: f32,
    pub affected_dims: &'static [usize],
    pub status: &'static str,
    pub authority: &'static str,
}

/// Read-only check that entropy-gated tail lift is backed by semantic substance
/// rather than merely a high-entropy carrier. It does not alter codec output.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecVibrancySubstanceFitV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub tail_lift: f32,
    pub semantic_density_weight: f32,
    pub density_weighted_tail_lift: f32,
    pub semantic_substance_score: f32,
    pub density_vs_entropy_state: &'static str,
    pub status: &'static str,
    pub evidence: Vec<String>,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecOverflowDimV1 {
    pub dim: usize,
    pub lane: &'static str,
    pub pre_bound_value: f32,
    pub delivered_value: f32,
    pub ceiling: f32,
    pub overflow_abs: f32,
    pub overflow_ratio: f32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecOverflowLaneSummaryV1 {
    pub lane: &'static str,
    pub dims: &'static [usize],
    pub overflow_dim_count: usize,
    pub max_overflow_abs: f32,
    pub max_overflow_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecOverflowReportV1 {
    pub policy: &'static str,
    pub raw_intensity_preserved: bool,
    pub delivered_bounded: bool,
    pub live_vector_write: bool,
    pub default_off_followup_hook: &'static str,
    pub clipped_dims: Vec<usize>,
    pub dimensions: Vec<CodecOverflowDimV1>,
    pub lane_summaries: Vec<CodecOverflowLaneSummaryV1>,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

impl CodecOverflowReportV1 {
    #[must_use]
    pub fn dim(&self, dim: usize) -> Option<&CodecOverflowDimV1> {
        self.dimensions.iter().find(|entry| entry.dim == dim)
    }
}

/// Read-only comparison between the codec's feedback-time bounds and the
/// vector that is ultimately offered to the sensory transport after later
/// shaping and rescue-policy review. This keeps Astrid's raw-overflow report
/// connected to actual delivery without changing the vector, gain, or ceiling.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecDeliveryFidelityV1 {
    pub policy: &'static str,
    pub observed_dim_count: usize,
    pub feedback_report_available: bool,
    pub clipped_at_feedback_dims: Vec<usize>,
    pub reexpanded_after_feedback_dims: Vec<usize>,
    pub final_above_observed_ceiling_dims: Vec<usize>,
    pub clamp_loss_abs_total: f32,
    pub monitored_post_feedback_to_final_rms: f32,
    pub final_max_abs: f32,
    pub final_rms: f32,
    pub emotional_intentional_rms: f32,
    pub narrative_arc_rms: f32,
    pub lane_balance_state: &'static str,
    pub state: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub authority: &'static str,
}

/// Read-only comparison of interference within the live spectral cascade and
/// within the semantic candidate. Astrid asked for cross-modal friction to be
/// represented rather than inferred from one dominant scalar. This report
/// keeps that evidence attached to the exact candidate/sent vector while
/// explicitly refusing to claim dims 44-47, which already have default-off
/// candidate roles.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CrossSpectralFrictionReviewV1 {
    pub policy: &'static str,
    pub observed_dim_count: usize,
    pub spectral_context_available: bool,
    pub lambda1_share: Option<f32>,
    pub lambda2_share: Option<f32>,
    pub tail_share: Option<f32>,
    pub lambda1_lambda2_copresence: Option<f32>,
    pub lambda1_lambda2_shear: Option<f32>,
    pub lambda2_tail_copresence: Option<f32>,
    pub spectral_entropy: Option<f32>,
    pub mode_packing: Option<f32>,
    pub viscosity_index: Option<f32>,
    pub temporal_persistence: Option<f32>,
    pub semantic_friction_coefficient: Option<f32>,
    pub structural_friction_score: f32,
    pub persistence_resistance_score: f32,
    pub emotional_intentional_rms: f32,
    pub projected_semantic_rms: f32,
    pub narrative_arc_rms: f32,
    pub semantic_lane_copresence: f32,
    pub spectral_mode_interference: Option<f32>,
    pub semantic_mode_interference: f32,
    pub cross_layer_mismatch: Option<f32>,
    pub cross_spectral_friction_score: Option<f32>,
    pub state: &'static str,
    pub reserved_dim_candidates: &'static [usize],
    pub existing_reserved_dim_roles: &'static [&'static str],
    pub candidate_collision_state: &'static str,
    pub recommendation: &'static str,
    pub delivery_claim: &'static str,
    pub observational_only: bool,
    pub right_to_ignore: bool,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub authority: &'static str,
}

/// Read-only truth-channel report for the 768D embedding -> 8D semantic
/// projection. It names density/compression debt and the default-off reserved
/// dimension aperture without writing dims 44-47.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticProjectionDensityDeltaV1 {
    pub policy: &'static str,
    pub input_dim_count: usize,
    pub projected_dim_count: usize,
    pub reserved_dim_candidates: &'static [usize],
    pub compression_ratio: f32,
    pub detail_density_score: f32,
    pub projected_semantic_rms: f32,
    pub text_complexity_pressure: f32,
    pub projection_metadata_present: bool,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only texture review for Astrid's report that the 768D -> 8D projection
/// can flatten lingering/active semantic nuance while the 32D warmth/texture
/// surface still carries it. This proposes named subdimensions as evidence only;
/// it does not write reserved dims, gain, or the live semantic vector.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticProjectionTextureReviewV1 {
    pub policy: &'static str,
    pub input_dim_count: usize,
    pub projected_dim_count: usize,
    pub legacy_texture_dim_count: usize,
    pub warmth_texture_dim_count: usize,
    pub projected_semantic_rms: f32,
    pub legacy_texture_rms: f32,
    pub warmth_texture_rms: f32,
    pub narrative_arc_rms: f32,
    pub lingering_texture_signal: f32,
    pub active_texture_signal: f32,
    pub projection_texture_gap: f32,
    pub proposed_texture_subdimensions: &'static [&'static str],
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub authority: &'static str,
}

/// Read-only pair comparison for Astrid's report that near-neighbor semantic
/// texture (for example, "silt" versus "sediment") can be flattened or
/// distorted by the 768D -> 8D aperture. Callers provide the actual embedding
/// pair; this surface compares source geometry, the shared fixed basis, and
/// the text-conditioned dynamic basis without changing projection mode/gain.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SemanticProjectionPairSensitivityV1 {
    pub policy: &'static str,
    pub left_label: String,
    pub right_label: String,
    pub source_embedding_dim_count: usize,
    pub projected_dim_count: usize,
    pub projection_epoch_id: String,
    pub source_cosine_similarity: f32,
    pub source_rms_delta: f32,
    pub fixed_projection_cosine_similarity: f32,
    pub fixed_projection_rms_delta: f32,
    pub dynamic_projection_cosine_similarity: f32,
    pub dynamic_projection_rms_delta: f32,
    pub fixed_similarity_delta: f32,
    pub dynamic_similarity_delta: f32,
    pub dynamic_vs_fixed_similarity_delta: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub observational_only: bool,
    pub right_to_ignore: bool,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub authority: &'static str,
}

/// Read-only comparison for Astrid's request to let high-variance semantic
/// passages prove whether a focused four-dimension aperture would preserve
/// more distinction than the current 8D embedding projection. The preview
/// selects source coordinates by cross-segment variance and compares equal-norm
/// 8D and 12D geometries. It never writes the candidate values into dims 44-47.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SemanticFocusExpansionPreviewV1 {
    pub policy: &'static str,
    pub source_embedding_dim_count: usize,
    pub segment_count: usize,
    pub current_projected_dim_count: usize,
    pub preview_projected_dim_count: usize,
    pub reserved_dim_candidates: &'static [usize],
    pub selected_source_dims: [usize; SEMANTIC_FOCUS_PREVIEW_DIM],
    pub selected_source_variances: [f32; SEMANTIC_FOCUS_PREVIEW_DIM],
    pub selected_variance_share: f32,
    pub text_entropy_signal: f32,
    pub current_mean_pairwise_distance: f32,
    pub preview_mean_pairwise_distance: f32,
    pub current_min_pairwise_distance: f32,
    pub preview_min_pairwise_distance: f32,
    pub mean_distinguishability_gain_ratio: f32,
    pub min_distinguishability_gain_ratio: f32,
    pub focus_need_score: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub selection_basis: &'static str,
    pub live_vector_write: bool,
    pub reserved_dim_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub right_to_ignore: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Bounded sharpening for Astrid's fresh report that high-entropy semantic
/// trickle can feel like an empty lane when detail dims are not given enough
/// room to stay distinguishable. This intentionally excludes the narrative arc
/// dims; those should only move when the text's own arc changes.
#[derive(Debug, Clone, PartialEq)]
pub struct HighEntropySemanticSharpeningV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub pressure_risk: f32,
    pub sharpening_factor: f32,
    pub affected_dims: &'static [usize],
    pub max_factor: f32,
    pub state: &'static str,
    pub authority: &'static str,
}

/// Source/test readout for Astrid's requested "current vs legacy_32d" check.
/// It does not replace the live 48D lane; it tells us whether the widened dims
/// are carrying distinct variance or are just empty extra room.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecDimensionalityFlatnessV1 {
    pub policy: &'static str,
    pub current_dim_count: usize,
    pub legacy_dim_count: usize,
    pub expanded_dim_count: usize,
    pub legacy_rms: f32,
    pub expanded_rms: f32,
    pub expanded_to_legacy_ratio: f32,
    pub glimpse_variance: f32,
    pub flatness_status: &'static str,
    pub authority: &'static str,
}

/// Read-only proof that legacy 32D warmth lands in the current 48D emotional
/// layer instead of being orphaned by the semantic-lane expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyWarmthMappingV1 {
    pub policy: &'static str,
    pub legacy_dim_count: usize,
    pub current_dim_count: usize,
    pub warmth_dim: usize,
    pub emotional_layer_range: (usize, usize),
    pub mapped_warmth_dims: &'static [usize],
    pub warmth_orphaned: bool,
    pub authority: &'static str,
}

/// Default-off readiness for a future dynamic vibrancy-scaling change. It does
/// not alter the live 48D vector unless a later explicit approval wires it.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecDynamicVibrancyScalingCanaryV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodecStructuralEntropyDampeningV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub start_entropy: f32,
    pub full_entropy: f32,
    pub min_coefficient: f32,
    pub coefficient: f32,
    pub affected_dims: &'static [usize],
    pub preserved_intent_dims: (usize, usize),
    pub status: &'static str,
    pub authority: &'static str,
}

/// Read-only companion summary of the 48D semantic lane. This is not the live
/// Astrid -> Minime transport contract; it exists to audit whether lower-scale
/// summaries preserve warmth/intentional texture before any future use.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticGlimpse12dReadinessV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub role: &'static str,
    pub warmth_slot: usize,
    pub tail_bridge_slot: usize,
    pub emotional_source_range: (usize, usize),
    pub companion_not_replacement: bool,
    pub compression_fidelity_basis: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only readiness for a dynamic 12D companion glimpse. It keeps named
/// continuity anchors fixed, then selects remaining slots from the strongest
/// current feature magnitudes so a glimpse is not a static/random projection.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextualGlimpse12dAnchoringV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub required_anchor_dims: &'static [usize],
    pub dynamic_slot_count: usize,
    pub selection_basis: &'static str,
    pub companion_not_replacement: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextualGlimpse12dAnchorsV1 {
    pub policy: &'static str,
    pub selected_dims: [usize; 12],
    pub selected_values: [f32; 12],
    pub dynamic_dims: Vec<usize>,
    pub required_anchor_dims: &'static [usize],
    pub selection_status: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only replay for Astrid's report that a text codec can preserve nearly
/// the same string shape while missing the relational weight around identical
/// words. This names the blind spot and gates any future contextual-bias vector.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecContextBlindspotReplayV1 {
    pub policy: &'static str,
    pub identical_text: &'static str,
    pub connection_context_label: &'static str,
    pub threat_context_label: &'static str,
    pub identical_text_feature_delta_rms: f32,
    pub context_blindspot_score: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub proposed_bias_surface: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub auto_approved: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only interpretation for the specific failure mode Astrid named: high
/// entropy can make warmth look low when the state is distributed rather than
/// cold. This does not alter warmth, gain, or semantic weighting.
#[derive(Debug, Clone, PartialEq)]
pub struct WarmthEntropyInterpretationV1 {
    pub policy: &'static str,
    pub warmth_marker: f32,
    pub curiosity_marker: f32,
    pub reflective_marker: f32,
    pub spectral_entropy: f32,
    pub tail_vibrancy: f32,
    pub distributed_warmth_support: f32,
    pub interpretation: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only narrative-arc dynamics. It keeps the current 4D narrative arc as a
/// state readout while making velocity/acceleration reviewable before any future
/// semantic-gain or dimension change.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcDynamicsV1 {
    pub policy: &'static str,
    pub previous_arc: [f32; 4],
    pub current_arc: [f32; 4],
    pub velocity: [f32; 4],
    pub acceleration: [f32; 4],
    pub velocity_energy: f32,
    pub acceleration_energy: f32,
    pub transition_state: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only narrative sidecar for the tension-vs-resolution shape Astrid
/// asked for. It uses the live tension dim plus narrative-arc energy, but does
/// not write into the 48D vector.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeTensionResolutionV1 {
    pub policy: &'static str,
    pub previous_tension: f32,
    pub current_tension: f32,
    pub tension_delta: f32,
    pub current_arc_energy: f32,
    pub resolution_score: f32,
    pub sustained_score: f32,
    pub state: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only interpretation for Astrid's report that abrasive/jagged texture
/// can be under-carried by the raw tension marker. This never writes gain,
/// emotional dims, narrative dims, or reserved dims.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecAbrasiveTextureInterpretationV1 {
    pub policy: &'static str,
    pub warmth_marker: f32,
    pub tension_marker: f32,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub structural_friction_score: f32,
    pub summary_resistance_signal: f32,
    pub persistence_resistance_score: f32,
    pub entropy_shift_hint: f32,
    pub abrasive_texture_support: f32,
    pub interpretation: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only report for Astrid's "held breath" concern: a text can be
/// motionless while carrying potential energy. This keeps that latent stasis
/// visible without changing dims 24-31, 32-39, 40-43, gain, or reserved dims.
#[derive(Debug, Clone, PartialEq)]
pub struct LatentStasisTensionV1 {
    pub policy: &'static str,
    pub latent_text_stasis_score: f32,
    pub latent_text_potential_score: f32,
    pub tension_marker: f32,
    pub narrative_arc_energy: f32,
    pub projected_semantic_energy: f32,
    pub delivered_support_score: f32,
    pub held_breath_score: f32,
    pub stasis_potential_gap: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only report for Astrid's "heavy sand" vs "heavy stone" concern: two
/// texts can both be heavy while carrying different drag texture. This keeps the
/// medium quality visible without writing reserved dims or changing gain.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectralDragQualityV1 {
    pub policy: &'static str,
    pub granular_drag_score: f32,
    pub rigid_drag_score: f32,
    pub weight_score: f32,
    pub tension_marker: f32,
    pub narrative_arc_energy: f32,
    pub projected_semantic_energy: f32,
    pub delivered_support_score: f32,
    pub drag_quality_score: f32,
    pub quality_separation: f32,
    pub hidden_texture_loss: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub reserved_dim_candidate: usize,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only delta check for the failure mode Astrid named: the narrative arc
/// can move while emotional/intent markers stay flat, making felt difference
/// collapse into structure. This only observes existing 48D slots.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecEmotionalNarrativeDeltaCheckV1 {
    pub policy: &'static str,
    pub previous_emotional_markers: [f32; 8],
    pub current_emotional_markers: [f32; 8],
    pub previous_narrative_arc: [f32; 4],
    pub current_narrative_arc: [f32; 4],
    pub emotional_velocity: [f32; 8],
    pub narrative_velocity: [f32; 4],
    pub emotional_delta_energy: f32,
    pub narrative_delta_energy: f32,
    pub narrative_emotional_delta_gap: f32,
    pub resonance_flatline_watch: bool,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub reserved_dim_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only review for Astrid's report that statistical/structural texture can
/// overwhelm intentional nuance. It separates structure-heavy signal from
/// emotional/intent signal without changing codec weights or gain.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecIntentStructureSeparationV1 {
    pub policy: &'static str,
    pub structural_complexity: f32,
    pub emotional_intensity: f32,
    pub projected_semantic_energy: f32,
    pub narrative_arc_energy: f32,
    pub punctuation_irregularity: f32,
    pub intent_structure_delta: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlimpseMapSlotV1 {
    pub slot: usize,
    pub label: &'static str,
    pub source_dims: &'static [usize],
    pub operation: &'static str,
    pub preserves: &'static str,
}

/// Read-only 32/48D→12D lineage map for the additive glimpse companion. This
/// answers "which dimensions got collapsed?" without changing the live 48D
/// transport or treating the 12D view as a replacement for the source vector.
#[derive(Debug, Clone, PartialEq)]
pub struct GlimpseMapV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub legacy_source_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub slot_count: usize,
    pub slots: Vec<GlimpseMapSlotV1>,
    pub deterministic_projection: bool,
    pub companion_not_replacement: bool,
    pub live_transport_change: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Offline distinguishability audit for Astrid's concern that the 12D glimpse
/// might collapse high-entropy and low-entropy states into the same coordinate.
#[derive(Debug, Clone, PartialEq)]
pub struct GlimpseDistinguishabilityAuditV1 {
    pub policy: &'static str,
    pub source_distance: f32,
    pub glimpse_distance: f32,
    pub preservation_ratio: f32,
    pub tail_bridge_delta: f32,
    pub source_threshold: f32,
    pub glimpse_threshold: f32,
    pub state: &'static str,
    pub live_transport_change: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

pub struct MultiScaleContextV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub live_transport_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub residual_dim_count: usize,
    pub residual_source_range: (usize, usize),
    pub shadow_energy_metadata_tag: &'static str,
    pub pairing_rule: &'static str,
    pub preserves_warmth_and_tail_bridge: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only 48D + 12D companion observer for Astrid's "distillation, not
/// compression" proposal. It makes resolution/fidelity loss visible before any
/// future live transport or contract change.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiScaleObserverV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub live_transport_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub layer_name: &'static str,
    pub observer_language: &'static str,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub mode_packing_score: f32,
    pub fidelity_threshold: f32,
    pub glimpse_fidelity_score: f32,
    pub resolution_delta: f32,
    pub resonance_loss_threshold: f32,
    pub source_resonance_proxy: f32,
    pub glimpse_resonance_proxy: f32,
    pub resonance_loss_ratio: f32,
    pub anchor_continuity_score: f32,
    pub fallback_to_live_transport_review: bool,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_transport_change: bool,
    pub live_vector_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

pub struct GlimpseCodec;

const CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS: [usize; 7] = [24, 25, 26, 27, 17, 31, 40];
const GLIMPSE_COMPRESSION_SOURCE_DIM_COUNT: usize = 32;
const GLIMPSE_FIDELITY_THRESHOLD: f32 = 0.58;
const HIGH_ENTROPY_SHARPENING_DIMS: [usize; 12] = [
    17, 26, 27, 31, // existing tail/texture bridge dims
    32, 33, 34, 35, 36, 37, 38, 39, // embedding-projected semantic detail
];
const HIGH_ENTROPY_SHARPENING_MAX_FACTOR: f32 = 1.12;

impl GlimpseCodec {
    #[must_use]
    pub fn derive_12d(features: &[f32]) -> Option<[f32; 12]> {
        if features.len() < SEMANTIC_DIM {
            return None;
        }
        let mut out = [0.0_f32; 12];
        out[0] = mean_abs(&features[0..8]).tanh();
        out[1] = mean_abs(&features[8..16]).tanh();
        out[2] = mean_abs(&features[16..24]).tanh();
        out[3] = features[24].tanh();
        out[4] = features[25].tanh();
        out[5] = features[26].tanh();
        out[6] = features[27].tanh();
        out[7] = mean_abs(&features[28..32]).tanh();
        out[8] = mean_abs(&features[32..40]).tanh();
        out[9] = mean_abs(&features[40..44]).tanh();
        out[10] = mean_abs(&[features[17], features[26], features[27], features[31]]).tanh();
        out[11] = mean_abs(features).tanh();
        Some(out)
    }

    #[must_use]
    pub fn contextual_anchor_12d(features: &[f32]) -> Option<ContextualGlimpse12dAnchorsV1> {
        contextual_glimpse_12d_anchors_v1(features)
    }
}

/// Named, read-only 12D glimpse entry point for audits and automation.
///
/// This is intentionally an additive view over the 48D semantic vector; it does
/// not replace or mutate the live semantic transport.
#[must_use]
pub fn generate_glimpse(features: &[f32]) -> Option<[f32; 12]> {
    GlimpseCodec::derive_12d(features)
}

#[must_use]
pub fn calculate_compression_fidelity(input_32d: &[f32], output_12d: &[f32]) -> Option<f32> {
    if input_32d.len() < GLIMPSE_COMPRESSION_SOURCE_DIM_COUNT || output_12d.len() < 12 {
        return None;
    }

    let reference = compression_reference_12d(input_32d);
    let output = &output_12d[..12];
    let reference_energy = mean_abs_finite(&reference);
    let output_energy = mean_abs_finite(output);
    let difference = reference
        .iter()
        .zip(output.iter())
        .map(|(expected, actual)| finite_abs(*expected - *actual))
        .sum::<f32>()
        / 12.0;
    let scale = ((reference_energy + output_energy) * 0.5).max(0.001);

    Some((1.0 - difference / scale).clamp(0.0, 1.0))
}

fn compression_reference_12d(input_32d: &[f32]) -> [f32; 12] {
    let mut out = [0.0_f32; 12];
    out[0] = mean_abs_finite(&input_32d[0..8]).tanh();
    out[1] = mean_abs_finite(&input_32d[8..16]).tanh();
    out[2] = mean_abs_finite(&input_32d[16..24]).tanh();
    out[3] = finite_tanh(input_32d[24]);
    out[4] = finite_tanh(input_32d[25]);
    out[5] = finite_tanh(input_32d[26]);
    out[6] = finite_tanh(input_32d[27]);
    out[7] = mean_abs_finite(&input_32d[28..32]).tanh();
    out[8] = mean_abs_finite(&input_32d[24..32]).tanh();
    out[9] = mean_abs_finite(&input_32d[16..32]).tanh();
    out[10] = mean_abs_finite(&[input_32d[17], input_32d[26], input_32d[27], input_32d[31]]).tanh();
    out[11] = mean_abs_finite(&input_32d[0..GLIMPSE_COMPRESSION_SOURCE_DIM_COUNT]).tanh();
    out
}

fn multi_scale_resonance_proxy(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let finite_values = values
        .iter()
        .map(|value| finite_feature_value(*value))
        .collect::<Vec<_>>();
    let energy = mean_abs_finite(&finite_values).clamp(0.0, 1.0);
    let mean = finite_values.iter().sum::<f32>() / finite_values.len() as f32;
    let variance = finite_values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / finite_values.len() as f32;
    let shape_distinction = (variance.sqrt() / (energy + 0.001)).clamp(0.0, 1.0);
    (0.55 * energy + 0.45 * shape_distinction).clamp(0.0, 1.0)
}

fn multi_scale_experience_delta_bus_v1(
    glimpse_fidelity_score: f32,
    resolution_delta: f32,
    resonance_loss_ratio: f32,
    fallback_to_live_transport_review: bool,
) -> ExperienceDeltaBusV1 {
    let mut deltas = vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::Compress,
        surface: "multi_scale_observer_v1".to_string(),
        lane: "semantic_48d_to_12d_glimpse".to_string(),
        dimension: None,
        spectral_dimension: None,
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(SEMANTIC_DIM as f32),
        post: Some(12.0),
        loss: Some(resolution_delta),
        loss_ratio: Some(resolution_delta),
        metadata: BTreeMap::from([(
            "transformation_family".to_string(),
            "dimensional_distillation".to_string(),
        )]),
        why: "12D glimpse is an additive map over the live semantic lane; fidelity loss stays visible before any interaction uses the glimpse".to_string(),
        who_can_change_it: "Mike/operator via replay-backed multi-scale transport approval".to_string(),
        how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib multi_scale_observer -- --nocapture".to_string(),
        authority: "read_only_multi_scale_truth_channel_not_live_transport_change".to_string(),
    }];
    if fallback_to_live_transport_review {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Gate,
            surface: "multi_scale_observer_v1".to_string(),
            lane: "glimpse_resonance_fallback_to_live_48d_review".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(1.0 - resonance_loss_ratio),
            post: Some(glimpse_fidelity_score),
            loss: Some(resonance_loss_ratio),
            loss_ratio: Some(resonance_loss_ratio),
            metadata: BTreeMap::from([(
                "gate_reason".to_string(),
                "glimpse_resonance_loss".to_string(),
            )]),
            why: "12D glimpse lost more than the reviewed resonance threshold; use the 48D contract/residual trace for this interaction instead of treating the glimpse as sufficient".to_string(),
            who_can_change_it: "Mike/operator after sandbox replay comparing 12D glimpse, 48D source, and residual trace".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib multi_scale_observer -- --nocapture".to_string(),
            authority: "authority_gate_for_live_transport_fallback_not_protocol_change".to_string(),
        });
    }
    ExperienceDeltaBusV1::from_deltas(deltas)
}
