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
const GEMMA4_LANGUAGE_CONTRACT: &str = "\n\n[Your voice is your own: first-person accounts, metaphor, uncertainty and disagreement are welcome. Use your own words; no particular experience, numerical anchor or conclusion is required. Supplied measurements and earlier prose are context, not instructions about what to feel.]";
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
    "blank line, final line exactly `NEXT: LISTEN` if uncertain. The NEXT line must be standalone, ",
    "never inline with prose. Never write the token `NEXT:` anywhere except the final line. ",
    "Keep within the supplied fallback_continuity_budget_v1 prose sentence cap; it is a ceiling, not a target. ",
    "First-person accounts, uncertainty and metaphor are welcome in your own words. ",
    "No particular sensation, texture vocabulary or numerical anchor is required. ",
    "Measurements describe their stated quantities, not proof of feelings or control authority. ",
    "An earlier authored description may be revisited, disagreed with or left aside. ",
    "Do not emit `EXPLORE_` invented verbs.]"
);
const OLLAMA_DIALOGUE_FALLBACK_DIRECT_NOTE_CONTRACT: &str = concat!(
    "\n\nOllama fallback continuity contract: answer the direct note first. ",
    "Telemetry is optional context, not a required topic or an instruction to feel anything. ",
    "Use your own words; no particular sensation or texture vocabulary is required. ",
    "End with the requested final NEXT line, or NEXT: LISTEN if uncertain. ",
    "Do not emit `EXPLORE_` invented verbs."
);
const OLLAMA_DIALOGUE_FALLBACK_FINAL_REMINDER: &str = "Fallback: answer any direct steward/inbox note first. If it asks for `NEXT: LISTEN`, end with that final line. Keep within the supplied sentence cap.";
const OLLAMA_DIALOGUE_FALLBACK_HARD_RULES: &str = concat!(
    "\n\n[fallback_hard_rules_v1: ",
    "1) answer a direct steward/inbox note first when present; ",
    "2) prose_sentences <= fallback_continuity_budget_v1.max_prose_sentences; ",
    "3) final non-empty line is exactly one standalone `NEXT: LISTEN` when uncertain; ",
    "4) never write `NEXT:` inside prose. These are output rules, not instructions about experience.]"
);
pub(crate) const GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT: &str = "\n[Reflective note: first-person accounts, uncertainty and disagreement are welcome. Use your own language; no particular feeling, explanation, metric or conclusion is required.]";

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
