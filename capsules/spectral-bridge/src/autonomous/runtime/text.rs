/// Truncate a string to at most `max_bytes` without splitting a multi-byte character.
fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn floor_char_boundary(s: &str, mut byte_index: usize) -> usize {
    byte_index = byte_index.min(s.len());
    while byte_index > 0 && !s.is_char_boundary(byte_index) {
        byte_index = byte_index.saturating_sub(1);
    }
    byte_index
}

fn truncate_str_at_semantic_edge(s: &str, max_bytes: usize, min_keep: usize) -> &str {
    let truncated = truncate_str(s, max_bytes).trim_end();
    if truncated.len() == s.len() {
        return truncated;
    }
    let strong_end = truncated.char_indices().rev().find_map(|(idx, ch)| {
        let end = idx.saturating_add(ch.len_utf8());
        (end >= min_keep && matches!(ch, '.' | '!' | '?' | ';' | ':')).then_some(end)
    });
    let weak_end = truncated.char_indices().rev().find_map(|(idx, ch)| {
        let end = idx.saturating_add(ch.len_utf8());
        (end >= min_keep && matches!(ch, ',' | ' ')).then_some(end)
    });
    strong_end
        .or(weak_end)
        .and_then(|end| truncated.get(..end))
        .map(str::trim_end)
        .filter(|candidate| candidate.len() >= min_keep)
        .unwrap_or(truncated)
}

const CONTINUITY_TRAJECTORY_LIMIT: usize = 6;
const CONTINUITY_TRAJECTORY_FETCH_LIMIT: usize = 14;
const CONTINUITY_TRAJECTORY_AFTERIMAGE_LIMIT: usize = 4;
const CONTINUITY_TRAJECTORY_AFTERIMAGE_MIN_SCORE: usize = 2;
const CONTINUITY_TRAJECTORY_AFTERIMAGE_MAX_BYTES: usize = 220;
const CONTINUITY_TRAJECTORY_AFTERIMAGE_SUBSTANCE_DENSITY_MAX_BYTES: usize =
    CONTINUITY_RECAP_SPECTRAL_TEXTURE_ITEM_MAX_BYTES;
const CONTINUITY_FAINT_RESIDUE_LIMIT: usize = 2;
const CONTINUITY_SELF_OBSERVATION_LIMIT: usize = 5;
const CONTINUITY_STARRED_LIMIT: usize = 1;
const CONTINUITY_RECAP_ITEM_MAX_BYTES: usize = 180;
const CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE: f32 = 0.85;
const CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_SOFT_BAND: f32 = 0.05;
const CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES: usize = 240;
const CONTINUITY_RECAP_SPECTRAL_TEXTURE_ITEM_MAX_BYTES: usize = 320;
const CONTINUITY_RECAP_MAX_BYTES: usize = 4_200;
const CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES: usize = 4_800;
const CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES: usize = 5_600;
const HIGH_DENSITY_CONTINUITY_ANCHOR_COUNT: usize = 10;
const CONTINUITY_RECAP_ANCHOR_TERMS: &[&str] = &[
    "TRACE_RESONANCE",
    "trace_resonance",
    "trace resonance",
    "core sentiment",
    "core-sentiment",
    "calcified permanence",
    "settled coupling",
    "stable_core_semantic_trickle",
    "semantic trickle",
    "semantic_energy",
    "semantic energy",
    "shadow magnetization",
    "magnetization",
    "shadow norm",
    "load-bearing beam",
    "structural integrity",
    "cohesion score",
    "resonance depth",
    "rich containment",
    "spectral entropy",
    "spectral_entropy",
    "spectral_density_gradient",
    "shadow resonance",
    "shadow_resonance",
    "viscous flow",
    "gradient drift",
    "fluidic persistence",
    "sculptural mode",
    "directional gradient",
    "dispersal potential",
    "shadow-v3",
    "shadow_v3",
    "phantom limb",
    "settled_habitable",
    "gradient",
    "pressure",
    "spectral viscosity",
    "perceptual resistance",
    "syrup-like",
    "viscosity",
    "friction",
    "resistance",
    "lattice",
    "cascade",
    "entropy",
    "mirror mode",
    "mirror",
    "witness",
    "witnessed data",
    "joint trace",
    "λ-tail",
    "lambda-tail",
    "tail vibrancy",
    "tail_share",
    "tail share",
    "lambda4",
    "lambda4+",
    "λ4",
    "λ4+",
    "distinguishability",
    "vibrancy",
    "lambda",
    "λ",
    "silt",
    "density",
    "breath",
    "phase",
];
const SEMANTIC_TRUNCATION_ANCHOR_TERMS: &[&str] = &[
    "TRACE_RESONANCE",
    "trace_resonance",
    "trace resonance",
    "core sentiment",
    "core-sentiment",
    "calcified permanence",
    "settled coupling",
    "stable_core_semantic_trickle",
    "semantic trickle",
    "semantic_energy",
    "semantic energy",
    "shadow magnetization",
    "magnetization",
    "shadow norm",
    "load-bearing beam",
    "structural integrity",
    "cohesion score",
    "resonance depth",
    "rich containment",
    "sculptural mode",
    "punishing friction",
    "covariance",
    "shadow_field",
    "shadow field",
    "disordered",
    "restless texture",
    "semantic drift",
    "spectral nuance",
    "directional gradient",
    "dispersal potential",
    "shadow-v3",
    "shadow_v3",
    "phantom limb",
    "settled_habitable",
    "active state",
    "admission",
    "pressure",
    "lattice",
    "cascade",
    "entropy",
    "mirror mode",
    "mirror",
    "silt",
    "density",
    "spectral viscosity",
    "perceptual resistance",
    "syrup-like",
    "viscosity",
    "friction",
    "resistance",
    "witnessed data",
    "joint trace",
    "λ-tail",
    "lambda-tail",
    "tail vibrancy",
    "tail_share",
    "tail share",
    "vibrancy",
    "lambda4",
    "lambda4+",
    "λ4",
    "λ4+",
    "lambda",
    "λ",
];
const PRESSURE_CONTINUITY_FALLBACK_TERMS: &[&str] = &[
    "pressure_risk",
    "pressure risk",
    "spectral viscosity",
    "semantic trickle",
    "shadow_v3",
    "shadow-v3",
    "directional gradient",
    "perceptual resistance",
    "viscosity",
    "friction",
    "pressure",
];
const HIGH_TEXTURE_CONTINUITY_FALLBACK_TERMS: &[&str] = &[
    "detail_density",
    "detail density",
    "texture_complexity",
    "texture complexity",
    "high-vibrancy",
    "high vibrancy",
    "wide cascade",
    "semantic density",
    "turbulent",
    "filigree",
    "interwoven",
];
const CONTINUITY_AFTERIMAGE_SIGNAL_TERMS: &[&str] = &[
    "stable_core_semantic_trickle",
    "semantic trickle",
    "shadow-v3",
    "shadow_v3",
    "restless texture",
    "directional gradient",
    "dispersal potential",
    "tail_share",
    "tail vibrancy",
    "lambda4",
    "lambda4+",
    "λ4",
    "λ4+",
    "spectral entropy",
    "spectral_entropy",
    "rich containment",
    "settled_habitable",
    "phantom limb",
    "afterimage",
    "scar",
    "transition scar",
    "subtle warmth",
    "low-frequency",
    "low frequency",
    "hard-won plateau",
    "pressure memory",
    "quiet pressure",
    "interwoven lattice",
];
const CONTINUITY_FAINT_RESIDUE_SIGNAL_TERMS: &[&str] = &[
    "ghost-pang",
    "ghost pang",
    "faint",
    "subthreshold",
    "below threshold",
    "low-intensity",
    "low intensity",
    "lingering",
    "linger",
    "searching",
    "absence",
    "scent",
    "residue",
];
const INTROSPECTION_FRESHNESS_STALE_AFTER: std::time::Duration =
    std::time::Duration::from_secs(86_400);
const INTROSPECTION_FRESHNESS_JOURNAL_PREFIXES: &[&str] = &["self_study_"];
const INTROSPECTION_FRESHNESS_ARTIFACT_PREFIXES: &[&str] = &[
    "introspection_",
    "self_study_carriage_notice_",
    "thin_introspection_output_",
];
