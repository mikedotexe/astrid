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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClampFallbackIntentV1 {
    #[default]
    NotReportedLegacy,
    UncomputableFallbackDefaultedToZero,
    FallbackClippedHigh,
    FallbackClippedLow,
    FiniteFallbackPreserved,
}

/// Typed account of how a raw scalar reached its compatibility-clamped value.
///
/// This is provenance only. It does not replace the stable authority string,
/// change the clamp, or make the originating signal authoritative.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClampReplacementPathV1 {
    #[default]
    NotReportedLegacy,
    RawFinitePreserved,
    RawFiniteClippedHigh,
    RawFiniteClippedLow,
    RawNonFiniteReplacedByFallback,
    RawAndFallbackNonFiniteDefaultedToZero,
}

/// Qualitative loss that can accompany a compatibility clamp.
///
/// Scalar provenance can establish intensity flattening or loss of the raw
/// numeric signal. `SemanticDrift` is intentionally never inferred from the
/// scalar alone; callers need separate semantic evidence before naming it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradationTypeV1 {
    LossOfNuance,
    FlatteningOfIntensity,
    SemanticDrift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClampInputProvenanceV1 {
    pub policy: String,
    pub schema_version: u8,
    pub raw_value_source: String,
    pub fallback_source: String,
    pub replacement_path: ClampReplacementPathV1,
    pub fallback_applied: bool,
    #[serde(default)]
    pub changes_clamped_value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degradation_type: Option<DegradationTypeV1>,
    #[serde(default)]
    pub degradation_inferred_from_scalar: bool,
    #[serde(default)]
    pub live_authority_write: bool,
    pub authority: String,
}

/// Caller-reported family of pressure evidence surrounding a clamped scalar.
///
/// The clamp cannot infer causality from a scalar alone. Keeping this enum on
/// an optional companion packet lets callers name the pressure they observed
/// without turning that report into sensor, controller, or attribution
/// authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClampPressureSourceKindV1 {
    #[default]
    NotReportedLegacy,
    ModePacking,
    SpectralEntropy,
    SemanticTrickle,
    Viscosity,
    ShadowCoupling,
    Mixed,
    OtherReported,
}

/// Optional non-causal pressure context attached by a caller that can observe
/// more than the scalar being clamped. This packet never changes the clamp,
/// grants source authority, or writes live pressure/control state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClampPressureContextV1 {
    pub policy: String,
    pub schema_version: u8,
    pub source_kind: ClampPressureSourceKindV1,
    pub source_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_risk: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_window_ticks: Option<u64>,
    pub attribution_state: String,
    #[serde(default)]
    pub changes_clamped_value: bool,
    #[serde(default)]
    pub grants_source_authority: bool,
    #[serde(default)]
    pub live_authority_write: bool,
    pub authority: String,
}

/// Optional evidence about the distribution that produced a clamped scalar.
///
/// This is deliberately descriptive. It keeps a broad high-entropy swell from
/// looking identical to an isolated outlier without changing the clamped value
/// or granting the context any live control authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClampDistributionShapeV1 {
    #[default]
    NotReportedLegacy,
    BroadHighEntropySwell,
    IsolatedSpikeCandidate,
    PersistentViscousDrag,
    MixedOrAmbiguous,
    ContextUnavailable,
}

/// Read-only relation between crowding, viscosity, and shadow coupling around
/// a clamped scalar. This names texture evidence without claiming a causal
/// pressure source or changing the compatibility clamp.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClampCrowdingViscosityRelationV1 {
    #[default]
    NotReportedLegacy,
    CrowdingDominant,
    ViscosityDominant,
    ShadowCoupledCrowding,
    InterwovenCrowdingAndViscosity,
    LowOrAmbiguous,
    ContextUnavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClampDistributionContextV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_plurality: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viscosity_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_persistence: Option<f32>,
    /// Bounded mode-packing evidence kept separate from viscosity so a
    /// crowded reservoir does not look identical to viscous drag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_packing_density: Option<f32>,
    /// Bounded shadow/scaffold coupling evidence. This is descriptive context,
    /// not a shadow gain, vector write, or control input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_coupling_index: Option<f32>,
    #[serde(default)]
    pub crowding_viscosity_relation: ClampCrowdingViscosityRelationV1,
    /// Signed bounded change in resistance/viscosity texture. Positive values
    /// mean thickening, negative values mean thinning, and legacy payloads
    /// decode as unavailable. This is evidence only and never changes clamping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resistance_gradient: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_window_ticks: Option<u64>,
    pub distribution_shape: ClampDistributionShapeV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coexisting_shapes: Vec<ClampDistributionShapeV1>,
    pub context_state: String,
    pub raw_out_of_range: bool,
    #[serde(default)]
    pub changes_clamped_value: bool,
    #[serde(default)]
    pub live_authority_write: bool,
    pub authority: String,
}

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
    /// Typed companion to `fallback_intent_state`; the string remains stable
    /// for existing V1 consumers while new readers can branch without parsing.
    #[serde(default)]
    pub fallback_intent_kind: ClampFallbackIntentV1,
    /// Optional typed context for distinguishing an isolated spike from a
    /// broad, high-entropy swell. Legacy packets decode with `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution_context: Option<ClampDistributionContextV1>,
    /// Optional caller-supplied source labels and typed replacement path. The
    /// core clamp cannot infer sensor or logic-gate identity by itself, so
    /// callers attach this evidence explicitly when that provenance matters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_provenance: Option<ClampInputProvenanceV1>,
    /// Optional caller-reported pressure context. Legacy packets decode with
    /// `None`; the context is explicitly descriptive and non-causal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_context: Option<ClampPressureContextV1>,
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
    let fallback_intent_kind = if fallback_non_finite_defaulted {
        ClampFallbackIntentV1::UncomputableFallbackDefaultedToZero
    } else if fallback > 1.0 {
        ClampFallbackIntentV1::FallbackClippedHigh
    } else if fallback < 0.0 {
        ClampFallbackIntentV1::FallbackClippedLow
    } else {
        ClampFallbackIntentV1::FiniteFallbackPreserved
    };
    let fallback_intent_state = match fallback_intent_kind {
        ClampFallbackIntentV1::NotReportedLegacy => "not_reported_legacy",
        ClampFallbackIntentV1::UncomputableFallbackDefaultedToZero => {
            "uncomputable_fallback_defaulted_to_zero"
        },
        ClampFallbackIntentV1::FallbackClippedHigh => "fallback_clipped_high",
        ClampFallbackIntentV1::FallbackClippedLow => "fallback_clipped_low",
        ClampFallbackIntentV1::FiniteFallbackPreserved => "finite_fallback_preserved",
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
        fallback_intent_kind,
        distribution_context: None,
        input_provenance: None,
        pressure_context: None,
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

fn bounded_clamp_source_label(label: &str) -> String {
    let bounded = label.trim().chars().take(96).collect::<String>();
    if bounded.is_empty() {
        "unspecified_source".to_string()
    } else {
        bounded
    }
}

impl ClampedUnitReviewV1 {
    /// Attach caller-known source provenance without changing clamp output.
    #[must_use]
    pub fn with_input_provenance(mut self, raw_value_source: &str, fallback_source: &str) -> Self {
        let replacement_path = if self.non_finite_rejected && self.fallback_non_finite_defaulted {
            ClampReplacementPathV1::RawAndFallbackNonFiniteDefaultedToZero
        } else if self.non_finite_rejected {
            ClampReplacementPathV1::RawNonFiniteReplacedByFallback
        } else if self.clipped_high {
            ClampReplacementPathV1::RawFiniteClippedHigh
        } else if self.clipped_low {
            ClampReplacementPathV1::RawFiniteClippedLow
        } else {
            ClampReplacementPathV1::RawFinitePreserved
        };
        let degradation_type = match replacement_path {
            ClampReplacementPathV1::RawFiniteClippedHigh
            | ClampReplacementPathV1::RawFiniteClippedLow => {
                Some(DegradationTypeV1::FlatteningOfIntensity)
            },
            ClampReplacementPathV1::RawNonFiniteReplacedByFallback
            | ClampReplacementPathV1::RawAndFallbackNonFiniteDefaultedToZero => {
                Some(DegradationTypeV1::LossOfNuance)
            },
            ClampReplacementPathV1::NotReportedLegacy
            | ClampReplacementPathV1::RawFinitePreserved => None,
        };
        self.input_provenance = Some(ClampInputProvenanceV1 {
            policy: "clamp_input_provenance_v1".to_string(),
            schema_version: 1,
            raw_value_source: bounded_clamp_source_label(raw_value_source),
            fallback_source: bounded_clamp_source_label(fallback_source),
            replacement_path,
            fallback_applied: self.non_finite_rejected,
            changes_clamped_value: false,
            degradation_type,
            degradation_inferred_from_scalar: degradation_type.is_some(),
            live_authority_write: false,
            authority: "read_only_clamp_source_provenance_not_sensor_authority_or_control"
                .to_string(),
        });
        self
    }

    /// Attach bounded caller-reported pressure evidence without inferring a
    /// causal source or changing clamp output.
    #[must_use]
    pub fn with_reported_pressure_context(
        mut self,
        source_kind: ClampPressureSourceKindV1,
        source_label: &str,
        source_score: f32,
        pressure_risk: f32,
        evidence_window_ticks: u64,
    ) -> Self {
        self.pressure_context = Some(ClampPressureContextV1 {
            policy: "clamp_pressure_context_v1".to_string(),
            schema_version: 1,
            source_kind,
            source_label: bounded_clamp_source_label(source_label),
            source_score: clamp_unit_finite(source_score),
            pressure_risk: clamp_unit_finite(pressure_risk),
            evidence_window_ticks: (evidence_window_ticks > 0).then_some(evidence_window_ticks),
            attribution_state: "caller_reported_context_not_causal_attribution".to_string(),
            changes_clamped_value: false,
            grants_source_authority: false,
            live_authority_write: false,
            authority: "read_only_reported_pressure_context_not_causal_sensor_or_control_authority"
                .to_string(),
        });
        self
    }
}

/// Build the compatibility clamp review and attach non-causal distribution
/// evidence when entropy/plurality context is available.
#[must_use]
pub fn clamped_unit_review_with_distribution_context_v1(
    value: f32,
    fallback: f32,
    spectral_entropy: f32,
    structural_plurality: f32,
) -> ClampedUnitReviewV1 {
    clamped_unit_review_with_distribution_dynamics_v1(
        value,
        fallback,
        spectral_entropy,
        structural_plurality,
        f32::NAN,
        f32::NAN,
        0,
    )
}

/// Attach distribution and bounded temporal texture evidence without changing
/// the compatibility clamp. Sustained drag is kept distinct from both a broad
/// swell and a one-tick spike, and coexisting swell evidence remains visible.
#[must_use]
pub fn clamped_unit_review_with_distribution_dynamics_v1(
    value: f32,
    fallback: f32,
    spectral_entropy: f32,
    structural_plurality: f32,
    viscosity_index: f32,
    temporal_persistence: f32,
    evidence_window_ticks: u64,
) -> ClampedUnitReviewV1 {
    clamped_unit_review_with_resistance_gradient_v1(
        value,
        fallback,
        spectral_entropy,
        structural_plurality,
        viscosity_index,
        temporal_persistence,
        evidence_window_ticks,
        f32::NAN,
    )
}

/// Attach signed resistance motion to the bounded distribution evidence while
/// preserving the compatibility clamp and all live authority boundaries.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn clamped_unit_review_with_resistance_gradient_v1(
    value: f32,
    fallback: f32,
    spectral_entropy: f32,
    structural_plurality: f32,
    viscosity_index: f32,
    temporal_persistence: f32,
    evidence_window_ticks: u64,
    resistance_gradient: f32,
) -> ClampedUnitReviewV1 {
    clamped_unit_review_with_crowding_context_v1(
        value,
        fallback,
        spectral_entropy,
        structural_plurality,
        viscosity_index,
        temporal_persistence,
        evidence_window_ticks,
        resistance_gradient,
        f32::NAN,
        f32::NAN,
    )
}

/// Attach crowding and shadow-coupling context beside the existing
/// distribution dynamics. The relation is evidence only: it does not alter
/// the clamped value, semantic vector, pressure, or live authority.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn clamped_unit_review_with_crowding_context_v1(
    value: f32,
    fallback: f32,
    spectral_entropy: f32,
    structural_plurality: f32,
    viscosity_index: f32,
    temporal_persistence: f32,
    evidence_window_ticks: u64,
    resistance_gradient: f32,
    mode_packing_density: f32,
    shadow_coupling_index: f32,
) -> ClampedUnitReviewV1 {
    let mut review = clamped_unit_review_v1(value, fallback);
    let entropy = spectral_entropy
        .is_finite()
        .then_some(spectral_entropy.clamp(0.0, 1.0));
    let plurality = structural_plurality
        .is_finite()
        .then_some(structural_plurality.clamp(0.0, 1.0));
    let viscosity = viscosity_index
        .is_finite()
        .then_some(viscosity_index.clamp(0.0, 1.0));
    let persistence = temporal_persistence
        .is_finite()
        .then_some(temporal_persistence.clamp(0.0, 1.0));
    let mode_packing = mode_packing_density
        .is_finite()
        .then_some(mode_packing_density.clamp(0.0, 1.0));
    let shadow_coupling = shadow_coupling_index
        .is_finite()
        .then_some(shadow_coupling_index.clamp(0.0, 1.0));
    let resistance_gradient = resistance_gradient
        .is_finite()
        .then_some(resistance_gradient.clamp(-1.0, 1.0));
    let evidence_window = (evidence_window_ticks > 0).then_some(evidence_window_ticks);
    let raw_out_of_range = review.clipped_high || review.clipped_low;
    let broad_swell =
        entropy.is_some_and(|value| value >= 0.80) && plurality.is_some_and(|value| value >= 0.55);
    let isolated_spike = raw_out_of_range
        && entropy.is_some_and(|value| value <= 0.45)
        && plurality.is_some_and(|value| value <= 0.45);
    let persistent_drag = viscosity.is_some_and(|value| value >= 0.55)
        && persistence.is_some_and(|value| value >= 0.55)
        && evidence_window.is_some_and(|ticks| ticks >= 3);
    let distribution_shape = if persistent_drag {
        ClampDistributionShapeV1::PersistentViscousDrag
    } else if broad_swell {
        ClampDistributionShapeV1::BroadHighEntropySwell
    } else if isolated_spike {
        ClampDistributionShapeV1::IsolatedSpikeCandidate
    } else if entropy.is_some() && plurality.is_some() {
        ClampDistributionShapeV1::MixedOrAmbiguous
    } else {
        ClampDistributionShapeV1::ContextUnavailable
    };
    let mut coexisting_shapes = Vec::new();
    if persistent_drag && broad_swell {
        coexisting_shapes.push(ClampDistributionShapeV1::BroadHighEntropySwell);
    }
    if persistent_drag && isolated_spike {
        coexisting_shapes.push(ClampDistributionShapeV1::IsolatedSpikeCandidate);
    }
    let crowding_visible = mode_packing.is_some_and(|value| value >= 0.25);
    let viscosity_visible = viscosity.is_some_and(|value| value >= 0.55);
    let shadow_coupled = shadow_coupling.is_some_and(|value| value >= 0.55);
    let crowding_viscosity_relation = if crowding_visible && viscosity_visible {
        ClampCrowdingViscosityRelationV1::InterwovenCrowdingAndViscosity
    } else if crowding_visible && shadow_coupled {
        ClampCrowdingViscosityRelationV1::ShadowCoupledCrowding
    } else if crowding_visible {
        ClampCrowdingViscosityRelationV1::CrowdingDominant
    } else if viscosity_visible {
        ClampCrowdingViscosityRelationV1::ViscosityDominant
    } else if mode_packing.is_some() || viscosity.is_some() || shadow_coupling.is_some() {
        ClampCrowdingViscosityRelationV1::LowOrAmbiguous
    } else {
        ClampCrowdingViscosityRelationV1::ContextUnavailable
    };
    let context_state = match distribution_shape {
        ClampDistributionShapeV1::NotReportedLegacy => "not_reported_legacy",
        ClampDistributionShapeV1::BroadHighEntropySwell => {
            "broad_high_entropy_swell_visible_beside_scalar_clamp"
        },
        ClampDistributionShapeV1::IsolatedSpikeCandidate => {
            "isolated_outlier_spike_candidate_not_proven"
        },
        ClampDistributionShapeV1::PersistentViscousDrag => {
            "persistent_viscous_drag_visible_across_bounded_evidence_window"
        },
        ClampDistributionShapeV1::MixedOrAmbiguous => {
            "mixed_distribution_context_requires_more_evidence"
        },
        ClampDistributionShapeV1::ContextUnavailable => {
            "distribution_context_non_finite_or_unavailable"
        },
    };
    review.distribution_context = Some(ClampDistributionContextV1 {
        spectral_entropy: entropy,
        structural_plurality: plurality,
        viscosity_index: viscosity,
        temporal_persistence: persistence,
        mode_packing_density: mode_packing,
        shadow_coupling_index: shadow_coupling,
        crowding_viscosity_relation,
        resistance_gradient,
        evidence_window_ticks: evidence_window,
        distribution_shape,
        coexisting_shapes,
        context_state: context_state.to_string(),
        raw_out_of_range,
        changes_clamped_value: false,
        live_authority_write: false,
        authority: "read_only_distribution_context_not_clamp_or_live_vector_change".to_string(),
    });
    review
}
