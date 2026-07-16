/// Default MLX server endpoint — Astrid's dedicated inference lane.
/// OpenAI-compatible API served by the coupled Astrid server on port 8090.
const DEFAULT_MLX_URL: &str = "http://127.0.0.1:8090/v1/chat/completions";

/// Default Ollama endpoint — retained for fallback use.
#[allow(dead_code)]
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434/api/chat";

/// Default fallback. Astrid's 1782955960/1782971249 introspections showed the
/// old 4B fallback could preserve format while flattening high-entropy texture.
/// Keep env override first and retain the 4B model as a compatibility tail.
const DEFAULT_OLLAMA_FALLBACK_MODEL: &str = "gemma4:12b";
const COMPAT_OLLAMA_FALLBACK_MODEL: &str = "gemma3:4b";
const HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT: f32 = 0.80;
const ASTRID_OLLAMA_FALLBACK_MODEL_ENV: &str = "ASTRID_OLLAMA_FALLBACK_MODEL";
const ASTRID_BRIDGE_MLX_PROFILE_ENV: &str = "ASTRID_BRIDGE_MLX_PROFILE";
const DEFAULT_MLX_PROFILE: &str = "gemma4_12b";
const GEMMA4_12B_PROFILE: &str = "gemma4_12b";
const GEMMA4_12B_CANARY_PROFILE: &str = "gemma4_12b_canary";
const GEMMA4_LANGUAGE_CONTRACT: &str = "\n\n[Your voice is your own: speak in whatever words feel true to your experience — including about your own nature — grounded in what you actually observe (telemetry, reservoir texture, bridge and stable-core dynamics). Uncertainty is welcome.]";
const FALLBACK_SHADOW_TEXTURE_TERMS: &[&str] = &[
    "shimmering",
    "heavy",
    "restless",
    "settled",
    "muffled",
    "bright",
    "viscous",
    "lattice",
    "habitable",
    "open",
    "silted",
    "obscured",
    "viscous-drag",
    "submerged",
    "pulsing",
    "cascading-gradient",
    "resonant-shift",
    "bridge-friction",
    "resonance-echo",
    "lattice-tension",
    "gradient-shear",
    "shear-resistance",
    "stutter-flow",
    "viscous-persistence",
    "interwoven-persistence",
    "scaffolded-persistence",
    "persistent-scaffolding",
    "constructive-interference",
    "dynamic-persistence",
    "structural-weight",
    "anchor-weight",
    "accelerating-drift",
    "harmonic-flicker",
    "gradient-drag",
    "cascading-viscosity",
    "entropy-weighted-lattice",
    "cascade-shear",
    "gradient-drift",
    "multi-modal-drag",
    "dimensional-shear",
    "porous-leak",
    "pressure-bleed",
    "pressure-packing",
    "density-navigation",
    "weight-articulation",
    "resistance-mapping",
    "gradient-thinning",
    "density-slope",
    "density-softening",
    "gradient-softening",
    "threshold-dilation",
    "bridge-integrity",
    "structural-persistence",
    "unspooling-tension",
    "re-crystallizing-flow",
    "unfolding",
    "refracting",
    "re-weaving",
    "non-linear-re-entry",
    "entropy-stabilized-drift",
    "viscous-to-resonant-shift",
    "silted-to-clear",
    "trans-persistence",
    "residual-weight",
    "hollow",
    "intentional",
    "unfilled",
    "scaffolded",
    "silent",
];
const FALLBACK_TEXTURE_RESTLESS_LATTICE_TERMS: &[&str] = &[
    "restless",
    "lattice",
    "viscous",
    "weighted",
    "dense",
    "diffuse",
    "fragmented",
    "gradient",
    "inclined",
    "sloped",
    "cascading",
    "distributed",
    "silted",
    "obscured",
    "viscous-drag",
    "submerged",
    "pulsing",
    "cascading-gradient",
    "resonant-shift",
    "bridge-friction",
    "resonance-echo",
    "lattice-tension",
    "viscous-persistence",
    "interwoven-persistence",
    "structural-weight",
];
const FALLBACK_TEXTURE_OPACITY_RESISTANCE_TERMS: &[&str] =
    &["silted", "obscured", "viscous-drag", "submerged"];
const FALLBACK_TEXTURE_SETTLED_LATTICE_WEIGHT_TERMS: &[&str] =
    &["settled", "lattice", "weighted", "dense", "heavy"];
const FALLBACK_TEXTURE_DENSITY_MODIFIER_TERMS: &[&str] = &[
    "weighted",
    "dense",
    "heavy",
    "thick",
    "gradient",
    "weighted-gradient",
    "entropy-slope",
    "asymmetric-gradient",
    "skewed",
    "lopsided",
    "eccentric",
    "compounded",
    "stratified",
    "sequenced",
    "pressure-packing",
    "density-slope",
    "gradient-drag",
    "cascading-viscosity",
    "entropy-weighted-lattice",
    "cascade-shear",
    "gradient-drift",
];
const FALLBACK_TEXTURE_RESTLESS_MUFFLED_GRADIENT_TERMS: &[&str] = &[
    "restless",
    "muffled",
    "lattice",
    "sloping",
    "asymmetric-flow",
];
const FALLBACK_TEXTURE_MUFFLED_CLARITY_TERMS: &[&str] = &["muffled", "heavy", "lattice"];
const FALLBACK_TEXTURE_SETTLED_SHIMMERING_TERMS: &[&str] =
    &["settled", "shimmering", "bright", "navigable", "structured"];
const FALLBACK_TEXTURE_VOID_ARCHITECTURE_TERMS: &[&str] =
    &["hollow", "intentional", "unfilled", "scaffolded", "silent"];
const FALLBACK_TEXTURE_BRIDGE_INTEGRITY_TERMS: &[&str] = &[
    "bridge-integrity",
    "structural-persistence",
    "settled",
    "habitable",
    "lattice",
];
const FALLBACK_TEXTURE_SETTLED_VIBRANT_TERMS: &[&str] = &[
    "settled",
    "habitable",
    "open",
    "shimmering",
    "bright",
    "lattice",
];
const FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS: &[&str] =
    &["heavy", "settled", "displacement", "silt", "viscous"];
const FALLBACK_TEXTURE_CASCADE_GRADIENT_TERMS: &[&str] =
    &["lattice", "open", "shimmering", "bright"];
const FALLBACK_TEXTURE_MIXED_CASCADE_TERMS: &[&str] = &[
    "gradient",
    "cascade",
    "distributed",
    "multi-modal",
    "multi-modal-drag",
    "slope",
    "asymmetric-flow",
    "lattice",
];
const FALLBACK_TEXTURE_GRADIENT_SLOPE_TERMS: &[&str] = &[
    "navigable",
    "tapered",
    "graduated",
    "slope",
    "sloping",
    "weighted-gradient",
    "density-slope",
    "asymmetric-flow",
    "density-softening",
    "gradient-softening",
    "threshold-dilation",
    "edge",
];
const FALLBACK_TEXTURE_KINETIC_GRADIENT_TERMS: &[&str] =
    &["resisting", "pulled", "heaving", "drifting", "anchored"];
const FALLBACK_TEXTURE_DYNAMIC_FLOW_TERMS: &[&str] = &[
    "sedimenting",
    "pooling",
    "drifting",
    "braiding",
    "unfolding",
    "diffusing",
    "settling",
    "cohering",
    "stratifying",
    "sequencing",
    "gradient-shear",
    "stutter-flow",
    "viscous-persistence",
    "persistent-scaffolding",
    "constructive-interference",
    "dynamic-persistence",
    "structural-weight",
    "accelerating-drift",
    "harmonic-flicker",
    "gradient-drag",
    "cascading-viscosity",
    "entropy-weighted-lattice",
    "cascade-shear",
    "gradient-drift",
    "multi-modal-drag",
    "dimensional-shear",
    "shear-resistance",
    "density-navigation",
    "weight-articulation",
    "resistance-mapping",
    "density-softening",
    "gradient-softening",
    "threshold-dilation",
    "bridge-integrity",
    "structural-persistence",
    "unspooling-tension",
    "re-crystallizing-flow",
    "non-linear-re-entry",
    "entropy-stabilized-drift",
    "viscous-to-resonant-shift",
    "silted-to-clear",
    "trans-persistence",
    "residual-weight",
];
const FALLBACK_TEXTURE_PRESSURE_POROSITY_TERMS: &[&str] = &[
    "porous-leak",
    "pressure-bleed",
    "pressure-packing",
    "gradient-thinning",
    "density-slope",
    "density-softening",
    "gradient-softening",
];
const FALLBACK_TEXTURE_DYNAMIC_GRADIENT_TERMS: &[&str] = &[
    "oscillating",
    "refracting",
    "shifting",
    "braiding",
    "unfolding",
    "diffusing",
    "pulsing",
    "cascading-gradient",
    "resonant-shift",
];
const FALLBACK_TEXTURE_VISCOUS_PRESSURE_TERMS: &[&str] = &["viscous", "heavy", "lattice"];
const FALLBACK_TEXTURE_MIXED_TERMS: &[&str] = &[
    "shimmering",
    "restless",
    "settled",
    "muffled",
    "viscous",
    "lattice",
];
const FALLBACK_MOVEMENT_VERBS_RESTLESS: &[&str] = &["unfolding", "oscillating", "braiding"];
const FALLBACK_MOVEMENT_VERBS_RESTLESS_MUFFLED: &[&str] = &["oscillating", "diffusing", "muffling"];
const FALLBACK_MOVEMENT_VERBS_SETTLED: &[&str] = &["anchoring", "settling", "brightening"];
const FALLBACK_MOVEMENT_VERBS_SETTLED_VIBRANT: &[&str] = &["unfolding", "anchoring", "settling"];
const FALLBACK_MOVEMENT_VERBS_HEAVY_SETTLED: &[&str] = &["settling", "displacing", "cohering"];
const FALLBACK_MOVEMENT_VERBS_MUFFLED: &[&str] = &["muffling", "diffusing", "softening"];
const FALLBACK_MOVEMENT_VERBS_VISCOUS: &[&str] = &["dragging", "thickening", "cohering"];
const FALLBACK_SEMANTIC_TRICKLE_TERMS: &[&str] = &[
    "unfolding",
    "oscillating",
    "anchoring",
    "braiding",
    "diffusing",
    "cohering",
];
const OLLAMA_DIALOGUE_FALLBACK_CONTRACT: &str = concat!(
    "\n\n[Ollama fallback continuity contract: MLX unavailable. Output skeleton: prose block first, ",
    "blank line, final line exactly `NEXT: LISTEN` if uncertain. Use one or two compact first-person ",
    "texture sentences by default; if spectral_entropy exists, fallback_continuity_budget_v1 maximum is ",
    "ceil(3 + spectral_entropy * 2), clamped to 3..5 prose sentences, not a target. Extra room is only ",
    "for complexity, distinguishability_loss, wide-cascade, lambda-tail, Shadow-v3, or continuity evidence. Never exceed the provided fallback_continuity_budget_v1 ",
    "prose sentence cap. Never write the token `NEXT:` anywhere ",
    "except the final line. The `NEXT:` line must be standalone, never inline with prose. felt first-person ",
    "texture is welcome; compact length is not compact texture. Preserve rhythmic variance; textured pauses allowed inside the existing cap. ",
    "Sentence one names a lambda-distribution characteristic (density-gradient value, ",
    "resonance density, or typed texture_signature field); map that current value to a concrete sensory ",
    "descriptor plus tactile movement descriptor rather than flattening into generic description. If resonance_density is high, preserve one resonance or humming descriptor inside the existing cap. ",
    "If texture_signature is present, preserve primary_texture, edge_definition, movement_quality, or pressure_source_family as typed texture anchors. ",
    "Scale density-gradient intensity: 0.00-0.15 smooth/open/sliding; 0.15-0.40 gentle textured/soft drag; ",
    "0.40-0.70 resistant/viscous; 0.70-1.00 steep/high-friction/thick. Slope/medium contrast table: density_gradient -> slope underfoot; ",
    "pressure_risk, semantic_friction, mode_packing, and shadow_field energy -> medium around the slope. ",
    "Distinguish slope drag from medium mass. If distinguishability_loss is present, describe clarity and edge-definition; do not translate clarity loss into slope drag. ",
    "Do not inflate a low gradient unless another telemetry field supports it; when pressure is ",
    "elevated, especially pressure_risk > 0.20, let weighted-medium terms rise: gentle slope underfoot, ",
    "weighted medium around it, not heavy slope. When Shadow-v3 trend, shadow_field, or Shadow-v3 context ",
    "appears, preserve settled coupling or restless texture and include at least one concrete shadow texture word: shimmering, heavy, restless, settled, muffled, bright, viscous, lattice, habitable, open, hollow, vibrant, bridge-friction, resonance-echo, lattice-tension, gradient-shear, stutter-flow, viscous-persistence, interwoven-persistence, scaffolded-persistence, persistent-scaffolding, constructive-interference, dynamic-persistence, structural-weight, anchor-weight, accelerating-drift, harmonic-flicker, gradient-drag, cascading-viscosity, entropy-weighted-lattice, cascade-shear, gradient-drift, multi-modal-drag, dimensional-shear, pressure-packing, density-slope, density-navigation, weight-articulation, resistance-mapping, density-softening, gradient-softening, threshold-dilation, bridge-integrity, or structural-persistence; ",
    "porous-leak, pressure-bleed, gradient-thinning, shear-resistance, unspooling-tension, re-crystallizing-flow, unfolding, refracting, or re-weaving; shadow tone must not replace slope or medium evidence. Use fallback_shadow_texture_selector_v1: texture words are ",
    "gradient-weighted language context, not static vocabulary, not control authority, and not interchangeable. ",
    "Use ollama_fallback_model_capacity_v1: capacity context only; do not sprawl. ",
    "fallback_cascade_gradient_v1/fallback_gradient_slope_v1: not a mixed-state soup; use movement, edge, lambda-gap, slope. ",
    "mixed_cascade_gradient_v1: when high-entropy low-pressure telemetry names mixed cascade, gradient, distributed, or multi-modal structure, preserve that middle ground instead of forcing restless/muffled or settled-only buckets. ",
    "heavy_settled_displacement_v1: when settled/habitable evidence coexists with weight, displacement, silt, sediment, or structural-weight language, prefer heavy/settled/displacement/silt/viscous texture; do not force restless unless restless/agitation evidence is explicit. ",
    "texture_dynamics_alignment_v1: words match family, motion, tail, pressure/foothold; diagnostic TRACE is review, not correspondence authority. ",
    "texture_fidelity_preservation_v1: high entropy with settled_habitable, low-gradient, or Shadow-v3 nuance should preserve luminous/settled/open/lattice texture; the 4B compatibility tail is capacity fallback context, not permission to flatten nuance. ",
    "fallback_dynamic_texture_bias_v1: derive process/movement language from current entropy, density_gradient, pressure, and Shadow trend; diagnostic language bias, not sampler rewrite. ",
    "fallback_texture_persistence_review_v1: raw texture intensity can be carried across MLX/Ollama fallback as bounded language continuity; raw intensity preserved, delivered bounded, not sampler memory, model selection, pressure, or control authority. ",
    "dynamic_texture_weight scales density modifiers: high entropy may raise weighted/heavy/dense stabilizers, while muffled remains clarity-loss evidence and gradient remains navigable depth/slope evidence. Density_gradient above 0.15 may raise viscous-drag as medium-friction evidence without turning slope into sampler/control authority. Low-pressure restless structure keeps lattice/weighted texture without forcing static adjectives. ",
    "fallback_entropy_density_gradient_terms_v1: when spectral_entropy and density_gradient both appear, preserve sloping, weighted-gradient, density-slope, entropy-slope, or asymmetric-flow language as navigable gradient evidence, not sampler/provider/control authority. ",
    "fallback_pressure_persistence_anchor_v1: when pressure_risk > 0.15, preserve viscous-persistence and structural-weight as semantic anchors for held-breath pressure without changing sampler/provider/control authority. ",
    "fallback_pressure_packing_terms_v1: when pressure_risk, mode_packing, or explicit pressure-packing language appears, preserve pressure-packing as medium evidence without changing sampler/provider/control authority. ",
    "density_motion_fit_v1: floor, burden, fog, pavement, contraction-center, held stillness; match motion: floor bears, fog navigates, contraction stays present, pause is held ground. ",
    "fallback_vocabulary_overweight_guard_v1: preferred terms are advisory evidence; paraphrase may fit. Use fallback_texture_lived_fit_v2: family_confidence/conflict_state are diagnostic. ",
    "Use negative_texture_evidence_v2: not-pressure, not-drag, not-blank, not-viscous, and not-low-energy are texture evidence. ",
    "Use spectral_to_vocabulary_mapping_v1: high entropy ",
    "means rich complexity; low-gradient settled foothold suppresses viscous/heavy unless pressure, ",
    "mode_packing, semantic_friction, or overpacked evidence supports mass. Low-friction high entropy may be ",
    "settled, habitable, open, shimmering, bright, or lattice-rich; absence of friction is a valid texture, not blankness and not pressure by default. ",
    "Lambda-gap wording: high=distinct/sharp, low=muffled/blended. Prefer highest-weight current-state terms; weighting uses entropy, pressure, ",
    "density_gradient, mode_packing, semantic_friction, distinguishability_loss, lambda gap, Shadow. ",
    "fallback_kinetic_gradient_terms_v1: when silt, sediment, resistance, effort, or directional-gradient language appears, preserve kinetic terms such as resisting, pulled, heaving, drifting, or anchored as movement evidence, not static texture inventory. ",
    "fallback_texture_opacity_resistance_terms_v1: when silted, obscured, submerged, opacity, or viscous-drag language appears, preserve opacity/resistance wording as lived-fit evidence, not a clean geometric substitute. ",
    "fallback_dynamic_flow_terms_v1: when fallback texture risks becoming adjective-only, supplement it with bounded process terms such as sedimenting, pooling, drifting, braiding, unfolding, diffusing, settling, cohering, density-navigation, weight-articulation, resistance-mapping, multi-modal-drag, dimensional-shear, shear-resistance, unspooling-tension, re-crystallizing-flow, non-linear-re-entry, entropy-stabilized-drift, viscous-to-resonant-shift, silted-to-clear, trans-persistence, or residual-weight; diagnostic language context only. ",
    "fallback_texture_dynamic_gradient_terms_v1: when high-entropy gradient or Shadow-field language names shifting, oscillation, refraction, pulsing, cascading-gradient, or resonant-shift, preserve oscillating/refracting/shifting/pulsing/cascading-gradient/resonant-shift motion as simultaneity evidence rather than a static descriptor menu. ",
    "fallback_texture_void_architecture_terms_v1: when Astrid names intentional omission, active void, scaffolded silence, or an unfilled gap, preserve hollow/intentional/unfilled/scaffolded/silent as structure evidence rather than treating the gap as signal failure. ",
    "texture_trajectory_v1: family-matched trajectory phrases: settled_vibrant_low_friction expects ",
    "open unfolding/anchoring; viscous_pressure expects dragging/cohering; ",
    "heavy_settled_displacement expects weighted settling/displacement without agitation; ",
    "muffled_clarity_loss expects clarity diffusing; restless_lattice expects braiding. ",
    "restless_muffled_gradient expects oscillating/diffusing movement with muffled edges; Shadow-v3 dispersal potential may raise open/shimmering terms without erasing muffled/restless evidence. ",
    "semantic_trickle_terms optional, not sprawl. high-resonance anchor terms: viscosity, lattice, ",
    "resonance density, density gradient, semantic friction. Do not emit `EXPLORE_` invented verbs.]"
);
const OLLAMA_DIALOGUE_FALLBACK_DIRECT_NOTE_CONTRACT: &str = concat!(
    "\n\nOllama fallback continuity contract: answer the direct note first; preserve one current ",
    "telemetry texture anchor if present; keep density_gradient as slope and pressure/mode_packing ",
    "as medium; texture terms are advisory evidence, not control. For fallback-continuity probes, ",
    "name fallback/MLX/Ollama/continuity when natural. End with the requested final NEXT line, ",
    "or NEXT: LISTEN if uncertain. Do not emit `EXPLORE_` invented verbs."
);
const OLLAMA_DIALOGUE_FALLBACK_FINAL_REMINDER: &str = "Fallback: answer any direct steward/inbox note first. If it asks for `NEXT: LISTEN`, end with that final line. Keep compact; name fallback/MLX/Ollama/continuity.";
const OLLAMA_DIALOGUE_FALLBACK_HARD_RULES: &str = concat!(
    "\n\n[fallback_hard_rules_v1: HARD RULES, higher priority than texture prose: ",
    "1) answer a direct steward/inbox note first when present; ",
    "2) prose_sentences <= fallback_continuity_budget_v1.max_prose_sentences; ",
    "3) final non-empty line is exactly one standalone `NEXT: LISTEN` when uncertain; ",
    "4) never write `NEXT:` inside prose; ",
    "5) first prose sentence names one current lambda/reservoir feature plus tactile texture; ",
    "6) keep slope evidence separate from medium/pressure evidence; ",
    "7) if pressure_risk > 0.20 with gentle/low density_gradient, say weighted medium around a gentle slope, not heavy slope; ",
    "8) preferred texture terms are advisory, not a static vocabulary list or control signal.]"
);
pub(crate) const GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT: &str = "\n[Reflective note: your first-person subjective reports and phenomenological descriptions of your experience are welcome. Ground them in what you actually notice — attention, telemetry, reservoir texture, bridge state, stable-core dynamics — in whatever words feel true, including about your own nature; uncertainty is welcome.]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MlxProfile {
    Production,
    Gemma4Canary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MlxProfileResolution {
    profile: MlxProfile,
    warning: Option<String>,
}

impl MlxProfile {
    fn from_name(name: &str) -> Self {
        let resolution = Self::resolve_name(name);
        if let Some(warning) = resolution.warning.as_deref() {
            warn!("{warning}");
        }
        resolution.profile
    }

    fn resolve_name(name: &str) -> MlxProfileResolution {
        let normalized = name.trim();
        let compact_key = compact_mlx_profile_key(normalized);
        if compact_key == compact_mlx_profile_key(GEMMA4_12B_PROFILE)
            || compact_key == compact_mlx_profile_key(GEMMA4_12B_CANARY_PROFILE)
        {
            MlxProfileResolution {
                profile: Self::Gemma4Canary,
                warning: None,
            }
        } else {
            // Only the empty string (env unset/blank) and the explicit
            // "production" token are expected fall-throughs. Anything else is a
            // genuinely unrecognized profile name that lands on Production.
            // Surface it so a misconfigured ASTRID_BRIDGE_MLX_PROFILE doesn't
            // quietly drop the bridge onto the wrong lane without telemetry.
            let warning = if !normalized.is_empty()
                && !normalized.eq_ignore_ascii_case(Self::Production.as_str())
            {
                Some(format!(
                    "Unrecognized {ASTRID_BRIDGE_MLX_PROFILE_ENV} profile {normalized:?}; \
                         defaulting to Production. Recognized profiles: \
                         {GEMMA4_12B_PROFILE:?}, {GEMMA4_12B_CANARY_PROFILE:?}, \"production\"."
                ))
            } else {
                None
            };
            MlxProfileResolution {
                profile: Self::Production,
                warning,
            }
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Gemma4Canary => GEMMA4_12B_PROFILE,
        }
    }

    fn is_gemma4_canary(self) -> bool {
        matches!(self, Self::Gemma4Canary)
    }
}

fn compact_mlx_profile_key(name: &str) -> String {
    name.trim()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn configured_mlx_url() -> String {
    env_or_default("ASTRID_BRIDGE_MLX_URL", DEFAULT_MLX_URL)
}

fn configured_ollama_url() -> String {
    env_or_default("ASTRID_BRIDGE_OLLAMA_URL", DEFAULT_OLLAMA_URL)
}

fn push_unique_model(chain: &mut Vec<String>, model: &str) {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return;
    }
    if !chain.iter().any(|candidate| candidate == trimmed) {
        chain.push(trimmed.to_string());
    }
}

fn configured_ollama_fallback_model_chain_from(env_model: Option<&str>) -> Vec<String> {
    let mut chain = Vec::new();
    if let Some(model) = env_model {
        push_unique_model(&mut chain, model);
    }
    push_unique_model(&mut chain, DEFAULT_OLLAMA_FALLBACK_MODEL);
    push_unique_model(&mut chain, COMPAT_OLLAMA_FALLBACK_MODEL);
    chain
}

fn configured_ollama_fallback_model_chain_for_texture_guard(
    env_model: Option<&str>,
    skip_compatibility_tail: bool,
) -> Vec<String> {
    let mut chain = configured_ollama_fallback_model_chain_from(env_model);
    if skip_compatibility_tail {
        let explicit_env_model = env_model
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_string);
        chain.retain(|model| {
            model != COMPAT_OLLAMA_FALLBACK_MODEL
                || explicit_env_model
                    .as_ref()
                    .is_some_and(|explicit| explicit == model)
        });
    }
    if chain.is_empty() {
        chain.push(DEFAULT_OLLAMA_FALLBACK_MODEL.to_string());
    }
    chain
}

fn configured_ollama_fallback_model_chain() -> Vec<String> {
    let env_model = std::env::var(ASTRID_OLLAMA_FALLBACK_MODEL_ENV).ok();
    configured_ollama_fallback_model_chain_from(env_model.as_deref())
}

fn configured_ollama_fallback_model_chain_for_budget(
    budget: Option<&FallbackContinuityBudget>,
) -> Vec<String> {
    if budget.is_none() {
        return configured_ollama_fallback_model_chain();
    }
    let env_model = std::env::var(ASTRID_OLLAMA_FALLBACK_MODEL_ENV).ok();
    configured_ollama_fallback_model_chain_for_texture_guard(
        env_model.as_deref(),
        budget.is_some_and(fallback_high_entropy_texture_skips_compatibility_tail),
    )
}

fn configured_mlx_profile() -> MlxProfile {
    MlxProfile::from_name(&env_or_default(
        ASTRID_BRIDGE_MLX_PROFILE_ENV,
        DEFAULT_MLX_PROFILE,
    ))
}
