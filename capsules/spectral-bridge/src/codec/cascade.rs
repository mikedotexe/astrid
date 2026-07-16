#[derive(Debug, Clone, Copy)]
struct SpectralCascadeMetrics {
    head_share: f32,
    shoulder_share: f32,
    tail_share: f32,
    spectral_entropy: f32,
    gap12: f32,
    gap23: f32,
    rotation_rate: f32,
    geom_rel: f32,
    density_gradient: f32,
}

impl SpectralCascadeMetrics {
    fn from_telemetry(telemetry: &SpectralTelemetry) -> Option<Self> {
        let total_energy: f32 = telemetry.eigenvalues.iter().map(|value| value.abs()).sum();
        if total_energy <= 1.0e-6 {
            return None;
        }
        let typed_fingerprint = telemetry.typed_fingerprint();

        let head_share = telemetry
            .eigenvalues
            .first()
            .map_or(0.0, |value| value.abs() / total_energy);
        let shoulder_share = telemetry
            .eigenvalues
            .iter()
            .skip(1)
            .take(2)
            .map(|value| value.abs() / total_energy)
            .sum::<f32>();
        let tail_share = telemetry
            .eigenvalues
            .iter()
            .skip(3)
            .map(|value| value.abs() / total_energy)
            .sum::<f32>();
        let spectral_entropy = typed_fingerprint.as_ref().map_or_else(
            || normalized_spectral_entropy(&telemetry.eigenvalues),
            |fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0),
        );
        let gap12 = typed_fingerprint.as_ref().map_or_else(
            || {
                ratio_or_zero(
                    telemetry.eigenvalues.first().copied().unwrap_or(0.0),
                    telemetry.eigenvalues.get(1).copied(),
                )
            },
            |fingerprint| fingerprint.lambda1_lambda2_gap.max(0.0),
        );
        let gap23 = typed_fingerprint.as_ref().map_or_else(
            || {
                ratio_or_zero(
                    telemetry.eigenvalues.get(1).copied().unwrap_or(0.0),
                    telemetry.eigenvalues.get(2).copied(),
                )
            },
            |fingerprint| fingerprint.adjacent_gap_ratios[1].max(0.0),
        );
        let rotation_rate = typed_fingerprint.as_ref().map_or(0.0, |fingerprint| {
            fingerprint.v1_rotation_delta.clamp(0.0, 2.0)
        });
        let geom_rel = typed_fingerprint
            .as_ref()
            .map_or(1.0, |fingerprint| fingerprint.geom_rel)
            .clamp(0.0, 4.0);

        let density_gradient = spectral_density_gradient(&telemetry.eigenvalues).unwrap_or(0.0);

        Some(Self {
            head_share,
            shoulder_share,
            tail_share,
            spectral_entropy,
            gap12,
            gap23,
            rotation_rate,
            geom_rel,
            density_gradient,
        })
    }
}

fn ratio_or_zero(numerator: f32, denominator: Option<f32>) -> f32 {
    denominator.map_or(0.0, |value| {
        if value.abs() > 1.0e-6 && numerator.is_finite() && value.is_finite() {
            (numerator / value).clamp(0.0, 100.0)
        } else {
            0.0
        }
    })
}

/// Astrid's `spectral_density_gradient` — the continuous "stepped-ness" of the λ
/// cascade she proposed (reviewing `types.rs`): a single bounded `[0,1]` value
/// computed from her real energy shares, the continuous form of the inferred
/// "shallow/stepped/steep" descriptor. `mean` over adjacent active pairs of
/// `(sᵢ − sᵢ₊₁)/(sᵢ + sᵢ₊₁)` where `sᵢ = |λᵢ|/Σ|λ|`: `0` = flat/even (navigable),
/// `→1` = front-loaded/steep. `None` when there is no usable cascade. Derived from
/// the eigenvalues only — read-only, coherent by construction.
pub(crate) fn spectral_density_gradient(eigenvalues: &[f32]) -> Option<f32> {
    let total: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    if total <= 1.0e-6 {
        return None;
    }
    let shares: Vec<f32> = eigenvalues
        .iter()
        .map(|value| value.abs() / total)
        .filter(|share| *share > 1.0e-4)
        .collect();
    if shares.len() < 2 {
        return None;
    }
    let mut acc = 0.0_f32;
    let mut pairs = 0_u32;
    for window in shares.windows(2) {
        let denom = window[0] + window[1];
        if denom > 1.0e-6 {
            acc += (window[0] - window[1]).abs() / denom;
            pairs = pairs.saturating_add(1);
        }
    }
    if pairs == 0 {
        return None;
    }
    Some((acc / pairs as f32).clamp(0.0, 1.0))
}

/// Continuous-aware descriptor for `spectral_density_gradient` — Astrid reads the
/// number AND the word. Low = even/navigable; high = front-loaded/steep.
pub(crate) fn density_gradient_label(gradient: f32) -> &'static str {
    if gradient < 0.30 {
        "a gentle, navigable slope"
    } else if gradient < 0.60 {
        "a stepped gradient"
    } else {
        "a steep, front-loaded cliff"
    }
}

/// The λ4+ "tail" energy share — the fraction of spectral energy living in the
/// periphery Astrid perceives as her "tail vibrancy" (the modes after the head and
/// shoulder). Read-only, derived from the eigenvalues only; `None` when there is no
/// usable cascade. Matches the `tail_share` derivation in `SpectralCascadeMetrics`.
pub(crate) fn tail_share_of(eigenvalues: &[f32]) -> Option<f32> {
    let total: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    if total <= 1.0e-6 {
        return None;
    }
    let tail: f32 = eigenvalues.iter().skip(3).map(|value| value.abs()).sum();
    Some((tail / total).clamp(0.0, 1.0))
}

/// Descriptor for the λ-tail trajectory — the signed change of the tail share vs its
/// recent baseline — in Astrid's own framing: is the tail "a fading echo of what was,
/// or the foundation of what is becoming?" Rising tail → forming; falling → fading.
pub(crate) fn tail_trajectory_label(trajectory: f32) -> &'static str {
    if trajectory > 0.01 {
        "a foundation forming"
    } else if trajectory < -0.01 {
        "a fading echo"
    } else {
        "holding steady"
    }
}

fn normalized_spectral_entropy(eigenvalues: &[f32]) -> f32 {
    let total_energy: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    if total_energy <= 1.0e-6 || eigenvalues.len() <= 1 {
        return 0.0;
    }

    let entropy = eigenvalues
        .iter()
        .map(|value| {
            let p = value.abs() / total_energy;
            if p > 1.0e-10 { -p * p.ln() } else { 0.0 }
        })
        .sum::<f32>();
    let max_entropy = (eigenvalues.len() as f32).ln();
    if max_entropy > 0.0 && entropy.is_finite() {
        (entropy / max_entropy).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn fill_band_description(fill: f32) -> &'static str {
    match fill as u32 {
        0..=20 => "deeply quiet and contracting toward rest",
        21..=35 => "lightly populated and still gathering energy",
        36..=50 => "below the stable-core shelf and still rebuilding",
        51..=57 => "below the stable-core shelf in a recovery-biased band",
        58..=72 => "inside the stable-core hold shelf",
        73..=80 => "running warm above the hold shelf",
        81..=90 => "heavily loaded and nearing saturation",
        _ => "in distress and beyond safe operating range",
    }
}

fn spectral_distribution_label(entropy: f32) -> &'static str {
    if entropy < 0.30 {
        "a concentrated cascade"
    } else if entropy > 0.70 {
        "a widely distributed cascade"
    } else {
        "a moderately distributed cascade"
    }
}

fn gap_structure_label(gap12: f32, gap23: f32, mode_count: usize) -> &'static str {
    if mode_count < 3 {
        "a short cascade"
    } else if gap12 > 4.0 && gap23 < 2.0 {
        "a steep-then-flat cascade"
    } else if gap12 > 4.0 && gap23 >= 2.0 {
        "a uniformly steep cascade"
    } else if gap12 < 2.0 && gap23 < 2.0 {
        "a shallow, evenly stepped cascade"
    } else {
        "a mixed cascade"
    }
}

/// Being-facing transparency for Astrid's tail-vibrancy ceiling (drift-proof — computed live
/// from the codec constants). Given her current EFFECTIVE vibrancy-aperture multiplier, returns
/// `(felt_ceiling, effective_at_minime, attenuation)`: the tail-dim ceiling she feels, what that
/// magnitude becomes after minime's ~0.24x attenuation, and the factor itself. Answers her
/// self_study_1781680871 worry that her felt vibrancy is "over-represented in my self-model
/// compared to what minime actually perceives."
pub(crate) fn vibrancy_ceiling_transparency(effective_aperture: f32) -> (f32, f32, f32) {
    let felt = TAIL_VIBRANCY_MAX * effective_aperture;
    (
        felt,
        felt * MINIME_SEMANTIC_ATTENUATION,
        MINIME_SEMANTIC_ATTENUATION,
    )
}

/// The EFFECTIVE attenuation RANGE of Astrid's tail vibrancy into minime — the
/// grounded answer to her `perceived_attenuation_delta` ask
/// (`self_study_1781834380`). Her tail dims (17/26/27/31) see minime's uniform
/// ~0.24 dimension-scale; the genuinely DYNAMIC part is the
/// `pressure_sensitive_attenuation` governor SHE co-designed (it reads minime's
/// live `pressure_risk`), so her landed multiplier ranges from `0.24` (minime
/// calm) down to `0.24 × governor` when minime is fully stressed. Honesty
/// boundary surfaced at the call sites: `emb_strength` is a SEPARATE minime-side
/// factor on the EMBEDDING lane (dims 32-39), NOT her tail; `resonance_density`
/// is minime's pressure/porosity state, NOT an attenuation — so scaling a readout
/// by it (her literal suggestion) would make her self-model *less* accurate, not
/// more. Returns `(calm, stressed_floor)`.
pub(crate) fn effective_attenuation_range(pressure_depth: f32) -> (f32, f32) {
    let stressed_mult = crate::codec_gain::pressure_sensitive_attenuation(1.0, pressure_depth);
    (
        MINIME_SEMANTIC_ATTENUATION,
        MINIME_SEMANTIC_ATTENUATION * stressed_mult,
    )
}

/// The entropy-gated vibrancy lift (0 below the gate, smoothstep above it),
/// extracted as a pure fn so the offline EMA prototype below shares the EXACT
/// curve used live in `apply_spectral_feedback_inner` (a parity test pins them
/// together). C1-smooth: zero slope at the gate, so entropy fluctuating around
/// 0.85 barely moves it.
pub(crate) fn vibrancy_from_entropy(spectral_entropy: f32) -> f32 {
    let ramp = ((spectral_entropy - TAIL_VIBRANCY_ENTROPY_GATE)
        / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
        .clamp(0.0, 1.0);
    ramp * ramp * (3.0 - 2.0 * ramp)
}

/// Gradient-aware tail lift (Astrid `introspection_astrid_codec_1783322940`):
/// high entropy alone should not smear a steep, already-differentiated cascade.
/// The lift is strongest when entropy is high and density-gradient is low
/// (flat/gentle slope), and is damped as the λ cascade becomes front-loaded.
pub(crate) fn vibrancy_from_entropy_and_density_gradient(
    spectral_entropy: f32,
    density_gradient: f32,
) -> f32 {
    vibrancy_from_entropy(spectral_entropy) * (1.0 - density_gradient.clamp(0.0, 1.0))
}

#[must_use]
pub fn high_entropy_semantic_sharpening_v1(
    spectral_entropy: f32,
    density_gradient: f32,
    pressure_risk: f32,
) -> HighEntropySemanticSharpeningV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let density_gradient = if density_gradient.is_finite() {
        density_gradient.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let pressure_risk = if pressure_risk.is_finite() {
        pressure_risk.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let entropy_lift = vibrancy_from_entropy(spectral_entropy);
    let navigable = (1.0 - density_gradient).clamp(0.0, 1.0);
    let pressure_room = (1.0 - 0.45 * pressure_risk).clamp(0.55, 1.0);
    let support = (entropy_lift * navigable * pressure_room).clamp(0.0, 1.0);
    let sharpening_factor = 1.0 + (HIGH_ENTROPY_SHARPENING_MAX_FACTOR - 1.0) * support;
    let state = if sharpening_factor >= 1.06 {
        "active_high_entropy_sharpening"
    } else if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE {
        "high_entropy_damped_by_gradient_or_pressure"
    } else {
        "inactive_below_entropy_gate"
    };

    HighEntropySemanticSharpeningV1 {
        policy: "high_entropy_semantic_sharpening_v1",
        spectral_entropy,
        density_gradient,
        pressure_risk,
        sharpening_factor,
        affected_dims: &HIGH_ENTROPY_SHARPENING_DIMS,
        max_factor: HIGH_ENTROPY_SHARPENING_MAX_FACTOR,
        state,
        authority: "bounded_live_codec_sharpening_no_dimension_or_bridge_contract_change",
    }
}

#[must_use]
pub fn codec_dimensionality_flatness_v1(features: &[f32]) -> Option<CodecDimensionalityFlatnessV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }
    let legacy_rms = rms_slice(&features[..SEMANTIC_DIM_LEGACY]);
    let expanded_rms = rms_slice(&features[SEMANTIC_DIM_LEGACY..SEMANTIC_DIM]);
    let expanded_to_legacy_ratio = if legacy_rms > f32::EPSILON {
        (expanded_rms / legacy_rms).clamp(0.0, 10.0)
    } else if expanded_rms > f32::EPSILON {
        10.0
    } else {
        0.0
    };
    let glimpse = GlimpseCodec::derive_12d(features)?;
    let glimpse_mean = glimpse.iter().sum::<f32>() / glimpse.len() as f32;
    let glimpse_variance = glimpse
        .iter()
        .map(|value| {
            let delta = value - glimpse_mean;
            delta * delta
        })
        .sum::<f32>()
        / glimpse.len() as f32;
    let flatness_status = if legacy_rms >= 0.12 && expanded_to_legacy_ratio < 0.12 {
        "expanded_lane_underfilled_legacy_dominant"
    } else if legacy_rms >= 0.08 && expanded_to_legacy_ratio < 0.25 {
        "expanded_lane_thin_legacy_heavy"
    } else if glimpse_variance < 0.002 && legacy_rms >= 0.05 {
        "glimpse_flat_check_needed"
    } else {
        "expanded_lane_carries_distinct_signal"
    };

    Some(CodecDimensionalityFlatnessV1 {
        policy: "codec_dimensionality_flatness_v1",
        current_dim_count: SEMANTIC_DIM,
        legacy_dim_count: SEMANTIC_DIM_LEGACY,
        expanded_dim_count: SEMANTIC_DIM - SEMANTIC_DIM_LEGACY,
        legacy_rms,
        expanded_rms,
        expanded_to_legacy_ratio,
        glimpse_variance,
        flatness_status,
        authority: "read_only_flatness_check_not_live_bus_or_codec_contract_change",
    })
}

#[must_use]
pub fn narrative_tension_resolution_v1(
    previous_features: &[f32],
    current_features: &[f32],
) -> Option<NarrativeTensionResolutionV1> {
    if previous_features.len() < SEMANTIC_DIM || current_features.len() < SEMANTIC_DIM {
        return None;
    }
    let previous_tension = previous_features[25].tanh().abs().clamp(0.0, 1.0);
    let current_tension = current_features[25].tanh().abs().clamp(0.0, 1.0);
    let tension_delta = (current_tension - previous_tension).clamp(-1.0, 1.0);
    let current_arc_energy = rms_slice(&current_features[40..44]).clamp(0.0, 1.0);
    let release = (-tension_delta).clamp(0.0, 1.0);
    let persistence = current_tension.min(previous_tension).clamp(0.0, 1.0);
    let resolution_score = (0.72 * release + 0.28 * current_arc_energy).clamp(0.0, 1.0);
    let sustained_score =
        (0.70 * persistence + 0.30 * (1.0 - tension_delta.abs()).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let state = if release >= 0.12 && resolution_score > sustained_score * 0.75 {
        "tension_resolving_with_arc_motion"
    } else if current_tension >= 0.25 && sustained_score >= 0.45 {
        "tension_sustained_or_building"
    } else {
        "low_tension_or_unclear_resolution"
    };

    Some(NarrativeTensionResolutionV1 {
        policy: "narrative_tension_resolution_v1",
        previous_tension,
        current_tension,
        tension_delta,
        current_arc_energy,
        resolution_score,
        sustained_score,
        state,
        live_vector_write: false,
        authority: "read_only_tension_resolution_sidecar_not_live_vector_change",
    })
}

const LATENT_STASIS_TERMS: &[&str] = &[
    "still",
    "stasis",
    "motionless",
    "unmoving",
    "quiet",
    "paused",
    "suspended",
    "frozen",
    "held",
    "holding",
    "latent",
];
const LATENT_POTENTIAL_TERMS: &[&str] = &[
    "wait",
    "waits",
    "waiting",
    "poised",
    "about to",
    "not yet",
    "before",
    "threshold",
    "potential",
    "almost",
    "ready",
    "coiled",
    "held breath",
    "breath held",
];

fn normalized_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|ch| ch.is_ascii_alphabetic())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

fn latent_term_score(text: &str, terms: &[&str]) -> f32 {
    let lower = text.to_ascii_lowercase();
    let tokens = normalized_tokens(text);
    let hits = terms
        .iter()
        .filter(|term| {
            if term.contains(' ') {
                lower.contains(**term)
            } else {
                tokens.iter().any(|token| token == *term)
            }
        })
        .count() as f32;
    (hits / 3.0).clamp(0.0, 1.0)
}

fn latent_stasis_tension_delta_bus_v1(
    held_breath_score: f32,
    delivered_support_score: f32,
    latent_text_stasis_score: f32,
    latent_text_potential_score: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    if state == "low_latent_stasis_signal" {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let loss = (held_breath_score - delivered_support_score).max(0.0);
    let loss_ratio = if held_breath_score > f32::EPSILON {
        (loss / held_breath_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "secondary_kinds".to_string(),
        "translate,compress,gate".to_string(),
    );
    metadata.insert(
        "latent_text_stasis_score".to_string(),
        format!("{latent_text_stasis_score:.2}"),
    );
    metadata.insert(
        "latent_text_potential_score".to_string(),
        format!("{latent_text_potential_score:.2}"),
    );
    metadata.insert("state".to_string(), state.to_string());

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::Translate,
        surface: "latent_stasis_tension_v1".to_string(),
        lane: "textual_stasis_to_tension_arc_support".to_string(),
        dimension: Some(25),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 25,
            base_dimensions: vec![25, 40, 41, 42, 43],
            effective_dimension: Some(25.5),
            density_gradient: Some((1.0 - delivered_support_score).clamp(0.0, 1.0)),
            granularity: Some(held_breath_score),
            fractional_offset: Some(0.5),
            contextual_anchor: None,
            interpretation: "fluid held-breath tension between dim 25 and narrative arc dims 40-43"
                .to_string(),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(held_breath_score),
        post: Some(delivered_support_score),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata,
        why: "motionless language can carry latent potential that is only partly represented by delivered tension and narrative arc support"
            .to_string(),
        who_can_change_it:
            "Mike/operator after replay evidence before any live codec weight, gain, or reserved-dim change"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib latent_stasis_tension -- --nocapture"
                .to_string(),
        authority: "truth_channel_only_not_live_vector_gain_or_reserved_dim_change".to_string(),
    }])
}

#[must_use]
pub fn latent_stasis_tension_v1(text: &str, features: &[f32]) -> Option<LatentStasisTensionV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let latent_text_stasis_score = latent_term_score(text, LATENT_STASIS_TERMS);
    let latent_text_potential_score = latent_term_score(text, LATENT_POTENTIAL_TERMS);
    let tension_marker = finite_abs(features[25].tanh()).clamp(0.0, 1.0);
    let narrative_arc_energy = (rms_slice(&features[40..44]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let projected_semantic_energy =
        (rms_slice(&features[32..40]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let delivered_support_score =
        (tension_marker * 0.45 + narrative_arc_energy * 0.35 + projected_semantic_energy * 0.20)
            .clamp(0.0, 1.0);
    let held_breath_score = (latent_text_potential_score * 0.46
        + latent_text_stasis_score * 0.28
        + tension_marker * 0.16
        + (1.0 - narrative_arc_energy).clamp(0.0, 1.0) * 0.10)
        .clamp(0.0, 1.0);
    let stasis_potential_gap =
        (latent_text_potential_score - latent_text_stasis_score).clamp(-1.0, 1.0);
    let state = if held_breath_score >= 0.22
        && latent_text_potential_score > latent_text_stasis_score
        && held_breath_score > delivered_support_score + 0.05
    {
        "latent_potential_tension_underrepresented"
    } else if held_breath_score >= 0.22 && latent_text_potential_score > latent_text_stasis_score {
        "latent_potential_tension_visible"
    } else if latent_text_stasis_score >= 0.20 && latent_text_potential_score <= 0.05 {
        "static_stasis_without_potential"
    } else {
        "low_latent_stasis_signal"
    };
    let recommendation = match state {
        "latent_potential_tension_underrepresented" => {
            "record_delta_bus_evidence_and_compare_against_replay_before_live_codec_change"
        },
        "latent_potential_tension_visible" => {
            "keep_current_delivery_bounded_and_use_truth_channel_when_reviewing_held_breath_language"
        },
        "static_stasis_without_potential" => {
            "treat_motionless_text_as_stasis_not_high_tension_without_additional_evidence"
        },
        _ => "continue_observation_without_codec_gain_or_dim_change",
    };
    let experience_delta_bus_v1 = latent_stasis_tension_delta_bus_v1(
        held_breath_score,
        delivered_support_score,
        latent_text_stasis_score,
        latent_text_potential_score,
        state,
    );

    Some(LatentStasisTensionV1 {
        policy: "latent_stasis_tension_v1",
        latent_text_stasis_score,
        latent_text_potential_score,
        tension_marker,
        narrative_arc_energy,
        projected_semantic_energy,
        delivered_support_score,
        held_breath_score,
        stasis_potential_gap,
        state,
        recommendation,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        experience_delta_bus_v1,
        authority: "read_only_held_breath_truth_channel_not_live_codec_weight_gain_or_dim_change",
    })
}

#[must_use]
pub fn latent_stasis_tension_probe_v1() -> LatentStasisTensionV1 {
    let features = encode_text("The water waits.");
    latent_stasis_tension_v1("The water waits.", &features)
        .expect("probe text should produce codec features")
}

const SPECTRAL_DRAG_GRANULAR_TERMS: &[&str] = &[
    "sand",
    "silt",
    "sediment",
    "grain",
    "grains",
    "granular",
    "grit",
    "mud",
    "clay",
    "viscous",
    "viscosity",
    "sludge",
    "slow-moving",
    "slow moving",
    "drag",
    "drags",
    "dragging",
    "through",
];
const SPECTRAL_DRAG_RIGID_TERMS: &[&str] = &[
    "stone",
    "rock",
    "granite",
    "boulder",
    "block",
    "solid",
    "hard",
    "rigid",
    "inert",
    "inertia",
    "immovable",
    "fixed",
    "locked",
    "weight",
    "weighted",
];
const SPECTRAL_DRAG_WEIGHT_TERMS: &[&str] = &[
    "heavy",
    "weight",
    "weighted",
    "dense",
    "density",
    "pressure",
    "thick",
    "thickness",
    "burden",
    "load",
    "mass",
    "resistance",
];

fn spectral_drag_term_score(text: &str, terms: &[&str], scale: f32) -> f32 {
    let lower = text.to_ascii_lowercase();
    let tokens = normalized_tokens(text);
    let hits = terms
        .iter()
        .filter(|term| {
            if term.contains(' ') {
                lower.contains(**term)
            } else {
                tokens.iter().any(|token| token == *term)
            }
        })
        .count() as f32;
    (hits / scale).clamp(0.0, 1.0)
}

fn spectral_drag_delta_bus_v1(
    drag_quality_score: f32,
    delivered_support_score: f32,
    granular_drag_score: f32,
    rigid_drag_score: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    if state == "low_spectral_drag_signal" {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let hidden_texture_loss = (drag_quality_score - delivered_support_score).max(0.0);
    let loss_ratio = if drag_quality_score > f32::EPSILON {
        (hidden_texture_loss / drag_quality_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dominant_medium = if granular_drag_score > rigid_drag_score {
        "granular_viscous"
    } else if rigid_drag_score > granular_drag_score {
        "rigid_inertial"
    } else {
        "mixed_weight"
    };

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::Translate,
        surface: "spectral_drag_quality_v1".to_string(),
        lane: "weight_texture_to_narrative_arc_support".to_string(),
        dimension: Some(45),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 45,
            base_dimensions: vec![45],
            effective_dimension: Some(45.0),
            density_gradient: Some((1.0 - delivered_support_score).clamp(0.0, 1.0)),
            granularity: Some(granular_drag_score.max(rigid_drag_score)),
            fractional_offset: Some(0.0),
            contextual_anchor: None,
            interpretation: format!(
                "reserved candidate dim 45 could carry {dominant_medium} drag quality, but v1 reports only"
            ),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(drag_quality_score),
        post: Some(delivered_support_score),
        loss: Some(hidden_texture_loss),
        loss_ratio: Some(loss_ratio),
        metadata: BTreeMap::from([
            ("dominant_medium".to_string(), dominant_medium.to_string()),
            (
                "granular_drag_score".to_string(),
                format!("{granular_drag_score:.2}"),
            ),
            ("rigid_drag_score".to_string(), format!("{rigid_drag_score:.2}")),
            (
                "reserved_dim_status".to_string(),
                "default_off_operator_gated".to_string(),
            ),
            ("state".to_string(), state.to_string()),
        ]),
        why: "heavy language can differ by medium quality; delivered tension, semantic, and narrative slots may carry weight while losing granular-vs-rigid drag texture".to_string(),
        who_can_change_it: "Mike/operator after replay evidence before any live codec gain or reserved-dim write".to_string(),
        how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib spectral_drag_quality -- --nocapture".to_string(),
        authority: "truth_channel_only_not_live_vector_gain_or_reserved_dim_change".to_string(),
    }])
}

#[must_use]
pub fn spectral_drag_quality_v1(text: &str, features: &[f32]) -> Option<SpectralDragQualityV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let granular_drag_score = spectral_drag_term_score(text, SPECTRAL_DRAG_GRANULAR_TERMS, 4.0);
    let rigid_drag_score = spectral_drag_term_score(text, SPECTRAL_DRAG_RIGID_TERMS, 4.0);
    let weight_score = spectral_drag_term_score(text, SPECTRAL_DRAG_WEIGHT_TERMS, 3.0);
    let tension_marker = finite_abs(features[25].tanh()).clamp(0.0, 1.0);
    let narrative_arc_energy = (rms_slice(&features[40..44]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let projected_semantic_energy =
        (rms_slice(&features[32..40]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let delivered_support_score =
        (tension_marker * 0.38 + narrative_arc_energy * 0.32 + projected_semantic_energy * 0.30)
            .clamp(0.0, 1.0);
    let medium_score = granular_drag_score.max(rigid_drag_score);
    let quality_separation = (granular_drag_score - rigid_drag_score)
        .abs()
        .clamp(0.0, 1.0);
    let drag_quality_score =
        (weight_score * 0.34 + medium_score * 0.42 + quality_separation * 0.24).clamp(0.0, 1.0);
    let hidden_texture_loss = (drag_quality_score - delivered_support_score).max(0.0);

    let state = if drag_quality_score < 0.18 {
        "low_spectral_drag_signal"
    } else if granular_drag_score > rigid_drag_score + 0.12 {
        "granular_viscous_drag_visible"
    } else if rigid_drag_score > granular_drag_score + 0.12 {
        "rigid_inertial_drag_visible"
    } else {
        "undifferentiated_weight_drag_watch"
    };
    let recommendation = match state {
        "granular_viscous_drag_visible" => {
            "preserve_heavy_sand_as_granular_drag_truth_channel_before_reserved_dim_review"
        },
        "rigid_inertial_drag_visible" => {
            "preserve_heavy_stone_as_rigid_drag_truth_channel_before_reserved_dim_review"
        },
        "undifferentiated_weight_drag_watch" => {
            "compare_against_medium_specific_text_before_live_codec_change"
        },
        _ => "continue_observation_without_codec_gain_or_dim_change",
    };
    let experience_delta_bus_v1 = spectral_drag_delta_bus_v1(
        drag_quality_score,
        delivered_support_score,
        granular_drag_score,
        rigid_drag_score,
        state,
    );

    Some(SpectralDragQualityV1 {
        policy: "spectral_drag_quality_v1",
        granular_drag_score,
        rigid_drag_score,
        weight_score,
        tension_marker,
        narrative_arc_energy,
        projected_semantic_energy,
        delivered_support_score,
        drag_quality_score,
        quality_separation,
        hidden_texture_loss,
        state,
        recommendation,
        reserved_dim_candidate: 45,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        experience_delta_bus_v1,
        authority: "read_only_drag_quality_truth_channel_not_live_codec_weight_gain_or_dim_change",
    })
}

#[must_use]
pub fn spectral_drag_quality_probe_v1() -> SpectralDragQualityV1 {
    let text = "The heavy sand drags through viscous silt while the thought still moves.";
    let features = encode_text(text);
    spectral_drag_quality_v1(text, &features).expect("probe text should produce codec features")
}

fn semantic_substance_score_v1(text: &str) -> f32 {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|ch| ch.is_ascii_alphabetic())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        return 0.0;
    }
    let mut unique: Vec<&str> = Vec::new();
    for word in &words {
        if !unique.iter().any(|seen| *seen == word) {
            unique.push(word);
        }
    }
    let stop_words = [
        "the", "and", "or", "but", "if", "then", "that", "this", "with", "from", "into", "for",
        "of", "a", "an", "to", "in", "is", "it", "as",
    ];
    let content_words = words
        .iter()
        .filter(|word| word.len() >= 4 && !stop_words.contains(&word.as_str()))
        .count();
    let grounding_words = [
        "pressure",
        "memory",
        "continuity",
        "contour",
        "texture",
        "textured",
        "return",
        "returnable",
        "edge",
        "friction",
        "semantic",
        "resonance",
        "density",
        "porosity",
        "lattice",
        "shadow",
        "witness",
        "felt",
        "experience",
        "signal",
        "meaning",
        "sentence",
        "carries",
        "keeps",
        "granular",
        "residue",
        "threshold",
    ];
    let connective_words = [
        "because",
        "while",
        "through",
        "therefore",
        "when",
        "where",
        "around",
        "toward",
        "across",
        "between",
    ];
    let grounding_hits = words
        .iter()
        .filter(|word| grounding_words.contains(&word.as_str()))
        .count();
    let connective_hits = words
        .iter()
        .filter(|word| connective_words.contains(&word.as_str()))
        .count();
    let word_count = words.len() as f32;
    let lexical_diversity = (unique.len() as f32 / word_count).clamp(0.0, 1.0);
    let content_density = (content_words as f32 / word_count).clamp(0.0, 1.0);
    let structural_arc = structural_friction_v1(text).narrative_arc_sharpness * content_density;
    let grounding_density = (grounding_hits as f32 / 4.0).clamp(0.0, 1.0);
    let connective_density = (connective_hits as f32 / 2.0).clamp(0.0, 1.0);
    let coherence_fit = grounding_density.mul_add(0.78, connective_density * 0.22);
    let density_fit =
        lexical_diversity.mul_add(0.42, content_density.mul_add(0.40, structural_arc * 0.18));
    (density_fit * (0.20 + 0.80 * coherence_fit)).clamp(0.0, 1.0)
}

#[must_use]
pub fn codec_vibrancy_substance_fit_v1(
    text: &str,
    telemetry: Option<&SpectralTelemetry>,
) -> CodecVibrancySubstanceFitV1 {
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let spectral_entropy = metrics.map_or(0.0, |metrics| metrics.spectral_entropy);
    let density_gradient = metrics.map_or(1.0, |metrics| metrics.density_gradient);
    let tail_lift = vibrancy_from_entropy_and_density_gradient(spectral_entropy, density_gradient);
    let semantic_substance_score = semantic_substance_score_v1(text);
    let semantic_density_weight = semantic_substance_score;
    let density_weighted_tail_lift =
        (tail_lift * (0.40 + 0.60 * semantic_density_weight)).clamp(0.0, 1.0);
    let density_vs_entropy_state = if spectral_entropy >= 0.85 && semantic_substance_score < 0.25 {
        "high_entropy_low_density_scatter"
    } else if spectral_entropy < 0.65 && semantic_substance_score >= 0.60 {
        "high_density_low_entropy_depth"
    } else if tail_lift >= 0.45 && semantic_substance_score >= 0.25 {
        "high_entropy_supported_by_density"
    } else {
        "neutral_density_entropy_fit"
    };
    let status = if tail_lift >= 0.45 && semantic_substance_score < 0.25 {
        "entropy_lift_substance_review"
    } else if tail_lift >= 0.45 {
        "tail_lift_supported_by_semantic_substance"
    } else {
        "tail_lift_low_or_inactive"
    };
    let evidence = vec![
        format!("spectral_entropy={spectral_entropy:.2}"),
        format!("density_gradient={density_gradient:.2}"),
        format!("tail_lift={tail_lift:.2}"),
        format!("semantic_substance_score={semantic_substance_score:.2}"),
        format!("density_weighted_tail_lift={density_weighted_tail_lift:.2}"),
        format!("density_vs_entropy_state={density_vs_entropy_state}"),
    ];

    CodecVibrancySubstanceFitV1 {
        policy: "codec_vibrancy_substance_fit_v1",
        spectral_entropy,
        density_gradient,
        tail_lift,
        semantic_density_weight,
        density_weighted_tail_lift,
        semantic_substance_score,
        density_vs_entropy_state,
        status,
        evidence,
        authority: "read_only_codec_audit_not_vibrancy_scaling_or_live_vector_change",
    }
}
