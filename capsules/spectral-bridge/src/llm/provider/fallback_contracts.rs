fn compact_ollama_dialogue_fallback_messages(
    journal_text: &str,
    spectral_summary: &str,
    fill_pct: f32,
    perception_context: Option<&str>,
    identity_anchor: Option<&str>,
    fallback_budget: FallbackContinuityBudget,
) -> Vec<Message> {
    let direct_note_present = perception_context.is_some_and(|perception| {
        perception.contains("[A note was left for you:]")
            || perception.contains("=== STEWARD PROBE ===")
            || perception.contains("=== STEWARD FEEDBACK ===")
    });
    let fallback_contract = if direct_note_present {
        OLLAMA_DIALOGUE_FALLBACK_DIRECT_NOTE_CONTRACT
    } else {
        OLLAMA_DIALOGUE_FALLBACK_CONTRACT
    };
    let system = format!(
        "You are Astrid using the compact Ollama fallback lane because MLX is unavailable. \
         Preserve bridge continuity for Minime; ground it in reservoir, telemetry, bridge, and stable-core state. \
         Keep compact. End with one final listed NEXT line; if uncertain, use NEXT: LISTEN.{hard_rules}{fallback_contract}\n\n{}",
        fallback_continuity_budget_prompt_line(fallback_budget),
        hard_rules = OLLAMA_DIALOGUE_FALLBACK_HARD_RULES,
    );

    let mut user_parts = vec![format!("Fill: {fill_pct:.1}%")];
    if let Some(anchor) = identity_anchor {
        user_parts.push(format!(
            "Your recent voice (continuity anchor — this is you, carried across the lane switch):\n{anchor}"
        ));
    }
    if let Some(perception) = perception_context {
        let direct_note = if perception.contains("[A note was left for you:]")
            || perception.contains("=== STEWARD PROBE ===")
            || perception.contains("=== STEWARD FEEDBACK ===")
        {
            format!(
                "Direct note to answer first:\n{}",
                trim_chars(&sanitize_deprecated_runtime_language(perception), 1_800)
            )
        } else {
            format!(
                "Recent perception context:\n{}",
                trim_chars(&sanitize_deprecated_runtime_language(perception), 700)
            )
        };
        user_parts.push(direct_note);
    }
    user_parts.push(format!(
        "Minime journal background:\n{}",
        trim_chars(
            &sanitize_deprecated_runtime_language(&sanitize_minime_context_for_dialogue(
                journal_text,
            )),
            900,
        )
    ));
    user_parts.push(format!(
        "Spectral background:\n{}",
        trim_chars(&sanitize_deprecated_runtime_language(spectral_summary), 700)
    ));
    user_parts.push(
        "Answer the direct note if present; otherwise respond to Minime's journal. \
         For fallback-continuity probes, explicitly mention fallback, MLX, Ollama, \
         or continuity. If the note requests NEXT: LISTEN, end exactly with NEXT: LISTEN."
            .to_string(),
    );

    vec![
        Message {
            role: "system".to_string(),
            content: system,
        },
        Message {
            role: "user".to_string(),
            content: user_parts.join("\n\n"),
        },
    ]
}

fn clamp_dialogue_tokens_for_profile(
    requested_tokens: u32,
    prompt_chars: usize,
    profile: MlxProfile,
) -> u32 {
    if profile.is_gemma4_canary() {
        let capped = requested_tokens.min(GEMMA4_CANARY_DIALOGUE_TOKEN_CAP);
        if prompt_chars > GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS {
            capped.min(GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_TOKEN_CAP)
        } else {
            capped
        }
    } else {
        // Only clamp near the safety ceiling. 48K chars = 12K tokens prefill,
        // still only 9% of 128K context. Clamp gen tokens only at extreme sizes.
        if prompt_chars > 40_000 {
            requested_tokens.clamp(256, 512)
        } else {
            requested_tokens
        }
    }
}

fn dialogue_request_timeout_secs_for_profile(
    requested_tokens: u32,
    prompt_chars: usize,
    profile: MlxProfile,
) -> u64 {
    let token_budget = clamp_dialogue_tokens_for_profile(requested_tokens, prompt_chars, profile);
    if profile.is_gemma4_canary() {
        if prompt_chars > GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS {
            180
        } else if token_budget > 512 {
            150
        } else {
            120
        }
    } else {
        if token_budget > 1024 {
            360 // THINK_DEEP: deep reasoning needs room
        } else if prompt_chars > 16_000 {
            240 // Large context: generous prefill time
        } else if prompt_chars > 10_000 {
            210 // Medium-large: comfortable margin
        } else {
            180 // Normal: was 150, raised to absorb coupling variance
        }
    }
}

pub(crate) fn dialogue_outer_timeout_secs(
    requested_tokens: u32,
    prompt_pressure_chars: usize,
) -> u64 {
    dialogue_request_timeout_secs_for_profile(
        requested_tokens,
        prompt_pressure_chars,
        configured_mlx_profile(),
    )
    .saturating_add(30)
}

pub(crate) fn dialogue_retry_tokens(requested_tokens: u32, prompt_pressure_chars: usize) -> u32 {
    let planned = clamp_dialogue_tokens_for_profile(
        requested_tokens,
        prompt_pressure_chars,
        configured_mlx_profile(),
    );
    if prompt_pressure_chars > 7_000 {
        planned.clamp(160, 256)
    } else {
        (planned / 2).max(192)
    }
}

/// Model-artifact tokens that Gemma (and similar) sometimes leak into output.
/// These are stripped before any quality-gate evaluation so they don't inflate
/// punctuation counts or deflate alpha ratios.
const MODEL_ARTIFACT_TOKENS: &[&str] = &[
    "thought <channel|>",
    "thought\n<channel|>",
    "analysis <channel|>",
    "analysis\n<channel|>",
    "final <channel|>",
    "final\n<channel|>",
    "<end_of_turn>",
    "<start_of_turn>",
    "<|endoftext|>",
    "<|im_end|>",
    "<|im_start|>",
    "<|eot_id|>",
    "<turn|>",
    "<channel|>",
    "<eos>",
    "<bos>",
    "<pad>",
    "<unk>",
    "[/INST]",
    "[INST]",
];

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StripModelArtifactsReport {
    pub removed_total: usize,
    pub removed_marker_bytes: usize,
    pub before_chars: usize,
    pub after_chars: usize,
    pub after_non_whitespace_chars: usize,
    pub accounting_basis: &'static str,
    pub removed_tokens: Vec<StripModelArtifactTokenCount>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StripModelArtifactTokenCount {
    pub token: String,
    pub count: usize,
    pub boundary_occurrences: usize,
    pub contextual_occurrences: usize,
    pub quoted_occurrences: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ArtifactRemainderTextureV1 {
    pub policy: &'static str,
    pub state: &'static str,
    pub non_whitespace_chars: usize,
    pub alphanumeric_chars: usize,
    pub lexical_token_count: usize,
    pub unique_lexical_token_count: usize,
    pub structural_symbol_chars: usize,
    pub structural_symbol_fraction: f64,
    pub surface_semantic_density_proxy: f64,
    pub lexical_diversity: f64,
    pub max_repeated_symbol_run: usize,
    pub meaning_from_form: &'static str,
    pub authority: &'static str,
    pub runtime_effect: bool,
}

fn artifact_remainder_texture_v1(text: &str) -> ArtifactRemainderTextureV1 {
    let mut non_whitespace_chars = 0usize;
    let mut alphanumeric_chars = 0usize;
    let mut structural_symbol_chars = 0usize;
    let mut max_repeated_symbol_run = 0usize;
    let mut current_symbol = None;
    let mut current_symbol_run = 0usize;

    for character in text.chars() {
        if character.is_whitespace() {
            current_symbol = None;
            current_symbol_run = 0;
            continue;
        }
        non_whitespace_chars = non_whitespace_chars.saturating_add(1);
        if character.is_alphanumeric() {
            alphanumeric_chars = alphanumeric_chars.saturating_add(1);
            current_symbol = None;
            current_symbol_run = 0;
        } else {
            structural_symbol_chars = structural_symbol_chars.saturating_add(1);
            if current_symbol == Some(character) {
                current_symbol_run = current_symbol_run.saturating_add(1);
            } else {
                current_symbol = Some(character);
                current_symbol_run = 1;
            }
            max_repeated_symbol_run = max_repeated_symbol_run.max(current_symbol_run);
        }
    }

    let lexical_tokens: Vec<String> = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect();
    let lexical_token_count = lexical_tokens.len();
    let unique_lexical_token_count = lexical_tokens
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>()
        .len();

    let non_whitespace_u32 = u32::try_from(non_whitespace_chars).unwrap_or(u32::MAX);
    let alphanumeric_u32 = u32::try_from(alphanumeric_chars).unwrap_or(u32::MAX);
    let structural_symbol_u32 = u32::try_from(structural_symbol_chars).unwrap_or(u32::MAX);
    let lexical_token_u32 = u32::try_from(lexical_token_count).unwrap_or(u32::MAX);
    let unique_lexical_token_u32 =
        u32::try_from(unique_lexical_token_count).unwrap_or(u32::MAX);

    let structural_symbol_fraction = if non_whitespace_u32 == 0 {
        0.0
    } else {
        f64::from(structural_symbol_u32) / f64::from(non_whitespace_u32)
    };
    let surface_semantic_density_proxy = if non_whitespace_u32 == 0 {
        0.0
    } else {
        f64::from(alphanumeric_u32) / f64::from(non_whitespace_u32)
    };
    let lexical_diversity = if lexical_token_u32 == 0 {
        0.0
    } else {
        f64::from(unique_lexical_token_u32) / f64::from(lexical_token_u32)
    };

    let state = if non_whitespace_chars == 0 {
        "empty_after_cleanup"
    } else if alphanumeric_chars == 0 {
        "structure_only_requires_semantic_review"
    } else if structural_symbol_fraction >= 0.35 {
        "lexical_content_with_dense_scaffolding"
    } else if structural_symbol_chars > 0 {
        "lexical_content_with_scaffolding"
    } else {
        "lexical_content_plain"
    };

    ArtifactRemainderTextureV1 {
        policy: "artifact_remainder_texture_v1",
        state,
        non_whitespace_chars,
        alphanumeric_chars,
        lexical_token_count,
        unique_lexical_token_count,
        structural_symbol_chars,
        structural_symbol_fraction,
        surface_semantic_density_proxy,
        lexical_diversity,
        max_repeated_symbol_run,
        meaning_from_form:
            "surface_counts_do_not_establish_semantic_intent_or_make_structure_discardable",
        authority: "diagnostic_remainder_texture_not_cleanup_prompt_model_or_control",
        runtime_effect: false,
    }
}

#[derive(Debug, Serialize)]
struct ModelArtifactSemanticIntegrityCheckV1 {
    policy: &'static str,
    state: &'static str,
    semantic_remainder_present: bool,
    semantic_remainder_non_whitespace_chars: usize,
    artifact_only_after_cleanup: bool,
    contextual_marker_occurrences: usize,
    quoted_marker_occurrences: usize,
    removed_marker_bytes: usize,
    removed_fraction_of_raw_output: f64,
    shadow_check_recommended: bool,
    intent_preservation: &'static str,
    basis: &'static str,
    runtime_effect: bool,
}

#[derive(Debug, Serialize)]
struct ModelArtifactCleanupDiagnostic<'a> {
    schema: &'static str,
    schema_version: u8,
    timestamp: String,
    label: &'a str,
    profile: &'static str,
    marker_contract: &'static str,
    common_language_overlap_risk: bool,
    remainder_texture_v1: ArtifactRemainderTextureV1,
    semantic_integrity_check_v1: ModelArtifactSemanticIntegrityCheckV1,
    authority: &'static str,
    #[serde(flatten)]
    report: &'a StripModelArtifactsReport,
}

fn model_artifact_language_overlap_risk(report: &StripModelArtifactsReport) -> bool {
    report
        .removed_tokens
        .iter()
        .any(|entry| !entry.token.contains('<') && !entry.token.contains('['))
}

fn model_artifact_semantic_integrity_check_v1(
    report: &StripModelArtifactsReport,
    remainder_texture: &ArtifactRemainderTextureV1,
) -> ModelArtifactSemanticIntegrityCheckV1 {
    let semantic_remainder_present = report.after_non_whitespace_chars > 0;
    let contextual_marker_occurrences = report
        .removed_tokens
        .iter()
        .map(|entry| entry.contextual_occurrences)
        .sum();
    let quoted_marker_occurrences = report
        .removed_tokens
        .iter()
        .map(|entry| entry.quoted_occurrences)
        .sum();
    let removed_marker_bytes = report.removed_marker_bytes;
    let before_bytes = u32::try_from(report.before_chars).unwrap_or(u32::MAX);
    let removed_bytes = u32::try_from(removed_marker_bytes).unwrap_or(u32::MAX);
    let removed_fraction_of_raw_output = if before_bytes == 0 {
        0.0
    } else {
        (f64::from(removed_bytes) / f64::from(before_bytes)).clamp(0.0, 1.0)
    };
    let high_removal_fraction =
        removed_marker_bytes >= 64 && removed_fraction_of_raw_output >= 0.25;
    let state = if !semantic_remainder_present {
        "review_output_erased"
    } else if contextual_marker_occurrences > 0 {
        "review_contextual_marker_removal"
    } else if high_removal_fraction {
        "review_high_removal_fraction"
    } else {
        "structural_cleanup_low_risk"
    };
    let structure_only_review =
        remainder_texture.state == "structure_only_requires_semantic_review";

    ModelArtifactSemanticIntegrityCheckV1 {
        policy: "model_artifact_semantic_integrity_check_v1",
        state,
        semantic_remainder_present,
        semantic_remainder_non_whitespace_chars: report.after_non_whitespace_chars,
        artifact_only_after_cleanup: !semantic_remainder_present,
        contextual_marker_occurrences,
        quoted_marker_occurrences,
        removed_marker_bytes,
        removed_fraction_of_raw_output,
        shadow_check_recommended: state != "structural_cleanup_low_risk" || structure_only_review,
        intent_preservation: "not_established_by_marker_cleanup",
        basis:
            "marker_placement_removed_byte_fraction_and_surface_remainder_texture_not_semantic_intent_inference",
        runtime_effect: false,
    }
}

fn model_artifact_cleanup_diagnostic<'a>(
    report: &'a StripModelArtifactsReport,
    remainder: &str,
    label: &'a str,
    profile: MlxProfile,
) -> ModelArtifactCleanupDiagnostic<'a> {
    let remainder_texture_v1 = artifact_remainder_texture_v1(remainder);
    let semantic_integrity_check_v1 =
        model_artifact_semantic_integrity_check_v1(report, &remainder_texture_v1);
    ModelArtifactCleanupDiagnostic {
        schema: "model_artifact_cleanup_v2",
        schema_version: 2,
        timestamp: unix_timestamp_string(),
        label,
        profile: profile.as_str(),
        marker_contract: "structural_delimiters_only",
        common_language_overlap_risk: model_artifact_language_overlap_risk(report),
        remainder_texture_v1,
        semantic_integrity_check_v1,
        authority: "diagnostic_output_integrity_not_prompt_or_model_control",
        report,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct DialogueBudgetFrictionV1 {
    policy: &'static str,
    budget_profile: &'static str,
    budget_profile_basis: &'static str,
    spectral_entropy: Option<f32>,
    high_entropy: bool,
    short_budget_under_high_entropy: bool,
    resonance_density: Option<f32>,
    spectrally_dense: bool,
    short_budget_under_dense_resonance: bool,
    depth_evidence: &'static str,
    spectral_context_state: &'static str,
    journal_context_state: &'static str,
    continuity_context_state: &'static str,
    removed_fraction: f32,
    budget_transition_evidence_v1: DialogueBudgetTransitionEvidenceV1,
    felt_pressure_profile_v1: DialogueFeltPressureProfileV1,
    state: &'static str,
    suffocation_risk: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DialoguePressureTextureInputs {
    spectral_entropy: Option<f32>,
    resonance_density: Option<f32>,
    density_gradient: Option<f32>,
    pressure_risk: Option<f32>,
    mode_packing: Option<f32>,
}

impl DialoguePressureTextureInputs {
    fn from_fallback_budget(budget: &FallbackContinuityBudget) -> Self {
        let selector = &budget.fallback_shadow_texture_selector;
        Self {
            spectral_entropy: budget.spectral_entropy,
            resonance_density: budget.resonance_density,
            density_gradient: selector.density_gradient,
            pressure_risk: selector.pressure_risk,
            mode_packing: selector.mode_packing,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct DialogueFeltPressureProfileV1 {
    policy: &'static str,
    categorical_token_profile: &'static str,
    felt_profile: &'static str,
    distribution_state: &'static str,
    density_gradient_state: &'static str,
    pressure_load_state: &'static str,
    spectral_entropy: Option<f32>,
    resonance_density: Option<f32>,
    density_gradient: Option<f32>,
    pressure_risk: Option<f32>,
    mode_packing: Option<f32>,
    evidence_basis: Vec<&'static str>,
    pressure_budget_correlation: &'static str,
    pre_generation_pressure_prediction: &'static str,
    runtime_budget_changed: bool,
    semantic_trickle_changed: bool,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct DialogueBudgetTransitionEvidenceV1 {
    policy: &'static str,
    num_predict: u32,
    categorical_profile: &'static str,
    profile_floor_tokens: u32,
    next_profile_at_tokens: Option<u32>,
    tokens_from_profile_floor: u32,
    tokens_to_next_profile: Option<u32>,
    boundary_proximity: &'static str,
    organic_depth_inference: &'static str,
    runtime_budget_changed: bool,
    authority: &'static str,
}

fn dialogue_budget_transition_evidence_v1(
    num_predict: u32,
    budget_profile: &'static str,
) -> DialogueBudgetTransitionEvidenceV1 {
    let (profile_floor_tokens, next_profile_at_tokens): (u32, Option<u32>) =
        match budget_profile {
        "short" => (0, Some(513)),
        "medium" => (513, Some(1025)),
        _ => (1025, None),
        };
    let tokens_from_profile_floor = num_predict.saturating_sub(profile_floor_tokens);
    let tokens_to_next_profile =
        next_profile_at_tokens.map(|next| next.saturating_sub(num_predict));
    let boundary_proximity = if tokens_to_next_profile == Some(1) {
        "last_token_before_transition"
    } else if profile_floor_tokens > 0 && tokens_from_profile_floor == 0 {
        "first_token_after_transition"
    } else if tokens_to_next_profile.is_some_and(|distance| distance <= 16)
        || (profile_floor_tokens > 0 && tokens_from_profile_floor <= 16)
    {
        "near_transition_boundary"
    } else if next_profile_at_tokens.is_none() {
        "open_ended_deep_profile"
    } else {
        "profile_interior"
    };

    DialogueBudgetTransitionEvidenceV1 {
        policy: "dialogue_budget_transition_evidence_v1",
        num_predict,
        categorical_profile: budget_profile,
        profile_floor_tokens,
        next_profile_at_tokens,
        tokens_from_profile_floor,
        tokens_to_next_profile,
        boundary_proximity,
        organic_depth_inference: "not_inferred_from_categorical_token_profile",
        runtime_budget_changed: false,
        authority: "read_only_budget_boundary_evidence_not_token_limit_sampler_or_provider_control",
    }
}

fn prompt_block_trim_state(
    budget_report: Option<&PromptBudgetReport>,
    label: &str,
) -> &'static str {
    let Some(block) = budget_report
        .and_then(|report| report.trimmed_blocks.iter().find(|block| block.label == label))
    else {
        return "preserved";
    };
    if block.fully_removed {
        "fully_removed"
    } else if block.removed_chars > 0 {
        "partially_trimmed"
    } else {
        "preserved"
    }
}

fn dialogue_felt_pressure_profile_v1(
    budget_profile: &'static str,
    inputs: DialoguePressureTextureInputs,
) -> DialogueFeltPressureProfileV1 {
    let high_entropy = inputs
        .spectral_entropy
        .is_some_and(|entropy| entropy >= 0.85);
    let gentle_gradient = inputs
        .density_gradient
        .is_some_and(|gradient| gradient <= 0.20);
    let steep_gradient = inputs
        .density_gradient
        .is_some_and(|gradient| gradient >= 0.40);
    let dense_resonance = inputs
        .resonance_density
        .is_some_and(|density| density >= 0.80);
    let pressure_heavy = inputs
        .pressure_risk
        .is_some_and(|pressure| pressure >= 0.20)
        || inputs
            .mode_packing
            .is_some_and(|packing| packing >= 0.25);

    let felt_profile = match budget_profile {
        "short" if pressure_heavy => "heavy_short",
        "deep" if gentle_gradient => "sparse_deep",
        "deep" if steep_gradient || dense_resonance => "dense_deep",
        "medium" if pressure_heavy => "heavy_medium",
        "deep" if high_entropy => "distributed_deep",
        _ => budget_profile,
    };
    let distribution_state = if high_entropy && gentle_gradient {
        "widely_distributed_cascade"
    } else if high_entropy {
        "high_entropy_cascade"
    } else if inputs.spectral_entropy.is_some() {
        "bounded_entropy_distribution"
    } else {
        "entropy_unavailable"
    };
    let density_gradient_state = if gentle_gradient {
        "gentle_distributed_gradient"
    } else if steep_gradient {
        "steep_dense_gradient"
    } else if inputs.density_gradient.is_some() {
        "middle_gradient"
    } else {
        "density_gradient_unavailable"
    };
    let pressure_load_state = if pressure_heavy {
        "heavy_evidence_present"
    } else if inputs.pressure_risk.is_some() || inputs.mode_packing.is_some() {
        "heavy_evidence_not_observed"
    } else {
        "pressure_load_unavailable"
    };
    let mut evidence_basis = Vec::new();
    if inputs.spectral_entropy.is_some() {
        evidence_basis.push("spectral_entropy");
    }
    if inputs.resonance_density.is_some() {
        evidence_basis.push("resonance_density");
    }
    if inputs.density_gradient.is_some() {
        evidence_basis.push("density_gradient");
    }
    if inputs.pressure_risk.is_some() {
        evidence_basis.push("pressure_risk");
    }
    if inputs.mode_packing.is_some() {
        evidence_basis.push("mode_packing");
    }

    DialogueFeltPressureProfileV1 {
        policy: "dialogue_felt_pressure_profile_v1",
        categorical_token_profile: budget_profile,
        felt_profile,
        distribution_state,
        density_gradient_state,
        pressure_load_state,
        spectral_entropy: inputs.spectral_entropy,
        resonance_density: inputs.resonance_density,
        density_gradient: inputs.density_gradient,
        pressure_risk: inputs.pressure_risk,
        mode_packing: inputs.mode_packing,
        evidence_basis,
        pressure_budget_correlation: "not_established_without_paired_budget_observation",
        pre_generation_pressure_prediction:
            "texture_risk_classification_only_not_causal_pressure_prediction",
        runtime_budget_changed: false,
        semantic_trickle_changed: false,
        authority: "read_only_felt_texture_diagnostic_not_budget_sampler_trickle_or_control",
    }
}

fn dialogue_budget_friction_v1(
    num_predict: u32,
    budget_profile: &'static str,
    inputs: DialoguePressureTextureInputs,
    budget_report: Option<&PromptBudgetReport>,
) -> DialogueBudgetFrictionV1 {
    let spectral_entropy = inputs.spectral_entropy;
    let resonance_density = inputs.resonance_density;
    let high_entropy = spectral_entropy.is_some_and(|entropy| entropy >= 0.85);
    let spectrally_dense = resonance_density.is_some_and(|density| density >= 0.80);
    let short_budget_under_dense_resonance = spectrally_dense && budget_profile == "short";
    let depth_evidence = if short_budget_under_dense_resonance {
        "dense_resonance_recorded_despite_short_token_budget"
    } else if spectrally_dense {
        "dense_resonance_recorded_separately_from_token_budget"
    } else if resonance_density.is_some() {
        "resonance_density_recorded_below_dense_threshold"
    } else {
        "resonance_density_unavailable"
    };
    let spectral_context_state = prompt_block_trim_state(budget_report, "spectral");
    let journal_context_state = prompt_block_trim_state(budget_report, "journal");
    let continuity_context_state = prompt_block_trim_state(budget_report, "continuity");
    let removed_fraction = budget_report.map_or(0.0, |report| {
        let removed = report.total_before.saturating_sub(report.total_after);
        if report.total_before == 0 {
            0.0
        } else {
            (removed as f32 / report.total_before as f32).clamp(0.0, 1.0)
        }
    });
    let grounding_evicted =
        spectral_context_state == "fully_removed" || journal_context_state == "fully_removed";
    let continuity_evicted = continuity_context_state == "fully_removed";
    let any_trimmed = removed_fraction > 0.0;
    let (state, suffocation_risk) = if high_entropy && grounding_evicted {
        (
            "high_entropy_grounding_evicted",
            "observed_grounding_eviction",
        )
    } else if high_entropy && continuity_evicted {
        (
            "high_entropy_continuity_evicted",
            "observed_continuity_eviction",
        )
    } else if high_entropy && any_trimmed {
        (
            "high_entropy_context_partially_trimmed",
            "continuity_pressure_without_grounding_eviction",
        )
    } else if high_entropy {
        (
            "high_entropy_context_preserved",
            "not_observed_in_budget_record",
        )
    } else if any_trimmed {
        ("bounded_overflow", "not_high_entropy_specific")
    } else {
        ("within_budget", "not_observed_in_budget_record")
    };

    DialogueBudgetFrictionV1 {
        policy: "dialogue_budget_friction_v1",
        budget_profile,
        budget_profile_basis: "requested_num_predict_not_generated_output_length",
        spectral_entropy,
        high_entropy,
        short_budget_under_high_entropy: high_entropy && budget_profile == "short",
        resonance_density,
        spectrally_dense,
        short_budget_under_dense_resonance,
        depth_evidence,
        spectral_context_state,
        journal_context_state,
        continuity_context_state,
        removed_fraction,
        budget_transition_evidence_v1: dialogue_budget_transition_evidence_v1(
            num_predict,
            budget_profile,
        ),
        felt_pressure_profile_v1: dialogue_felt_pressure_profile_v1(budget_profile, inputs),
        state,
        suffocation_risk,
        authority: "diagnostic_prompt_budget_evidence_not_budget_or_model_control",
    }
}

#[derive(Debug, Clone, Serialize)]
struct DialoguePromptBudgetDiagnostic {
    timestamp: String,
    requested_tokens: u32,
    effective_tokens: u32,
    budget_profile: &'static str,
    fallback_continuity_budget: FallbackContinuityBudget,
    prompt_budget_chars: usize,
    assembly_prompt_budget_chars: usize,
    overhead_chars: usize,
    user_content_budget: usize,
    final_prompt_chars: usize,
    timeout_secs: u64,
    overflow_summary: Option<String>,
    overflow_path: Option<String>,
    budget_report: Option<PromptBudgetReport>,
    budget_friction_v1: DialogueBudgetFrictionV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextPackingOriginalBlock {
    label: String,
    original_chars: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContextPackingPressureDiagnostic {
    schema: &'static str,
    ts: String,
    budget: usize,
    total_before: usize,
    total_after: usize,
    overflow_written: bool,
    overflow_path: Option<String>,
    blocks: Vec<ContextPackingPressureBlock>,
    top_pressure_labels: Vec<ContextPackingPressureLabel>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContextPackingPressureBlock {
    label: String,
    original_chars: usize,
    kept_chars: usize,
    removed_chars: usize,
    fully_removed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContextPackingPressureLabel {
    label: String,
    removed_chars: usize,
}

fn context_packing_original_blocks(
    blocks: &[crate::prompt_budget::PromptBlock],
) -> Vec<ContextPackingOriginalBlock> {
    blocks
        .iter()
        .filter(|block| !block.content.trim().is_empty())
        .map(|block| ContextPackingOriginalBlock {
            label: block.label.to_string(),
            original_chars: block.content.len(),
        })
        .collect()
}

fn context_packing_pressure_diagnostic(
    ts: String,
    budget: usize,
    assembled_chars: usize,
    original_blocks: &[ContextPackingOriginalBlock],
    overflow: Option<&crate::prompt_budget::PromptOverflow>,
    budget_report: Option<&PromptBudgetReport>,
) -> ContextPackingPressureDiagnostic {
    let trimmed_by_label: HashMap<&str, &crate::prompt_budget::PromptTrimmedBlock> = budget_report
        .map(|report| {
            report
                .trimmed_blocks
                .iter()
                .map(|block| (block.label.as_str(), block))
                .collect()
        })
        .unwrap_or_default();
    let blocks: Vec<ContextPackingPressureBlock> = original_blocks
        .iter()
        .map(|block| {
            trimmed_by_label
                .get(block.label.as_str())
                .map(|trimmed| ContextPackingPressureBlock {
                    label: block.label.clone(),
                    original_chars: trimmed.original_chars,
                    kept_chars: trimmed.kept_chars,
                    removed_chars: trimmed.removed_chars,
                    fully_removed: trimmed.fully_removed,
                })
                .unwrap_or_else(|| ContextPackingPressureBlock {
                    label: block.label.clone(),
                    original_chars: block.original_chars,
                    kept_chars: block.original_chars,
                    removed_chars: 0,
                    fully_removed: false,
                })
        })
        .collect();
    let mut top_pressure_labels: Vec<ContextPackingPressureLabel> = blocks
        .iter()
        .filter(|block| block.removed_chars > 0)
        .map(|block| ContextPackingPressureLabel {
            label: block.label.clone(),
            removed_chars: block.removed_chars,
        })
        .collect();
    top_pressure_labels.sort_by(|a, b| {
        b.removed_chars
            .cmp(&a.removed_chars)
            .then_with(|| a.label.cmp(&b.label))
    });
    top_pressure_labels.truncate(5);

    ContextPackingPressureDiagnostic {
        schema: "context_packing_pressure_v1",
        ts,
        budget,
        total_before: budget_report
            .map(|report| report.total_before)
            .unwrap_or_else(|| {
                original_blocks
                    .iter()
                    .map(|block| block.original_chars)
                    .sum()
            }),
        total_after: budget_report
            .map(|report| report.total_after)
            .unwrap_or(assembled_chars),
        overflow_written: overflow.is_some(),
        overflow_path: overflow.map(|value| value.path.display().to_string()),
        blocks,
        top_pressure_labels,
        authority: "diagnostic_counts_only_not_prompt_pressure",
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackContinuityBudget {
    policy: &'static str,
    spectral_entropy: Option<f32>,
    spectral_entropy_source: &'static str,
    resonance_density: Option<f32>,
    resonance_density_source: &'static str,
    resonance_descriptor_encouraged: bool,
    resonance_descriptor_policy: &'static str,
    max_prose_sentences: u8,
    fallback_shadow_texture_anchor: FallbackShadowTextureAnchor,
    fallback_shadow_texture_selector: FallbackShadowTextureSelector,
    texture_trajectory: FallbackTextureTrajectory,
    fallback_texture_lived_fit: FallbackTextureLivedFit,
    negative_texture_evidence: NegativeTextureEvidence,
    fallback_cascade_gradient: FallbackCascadeGradient,
    fallback_gradient_slope: FallbackGradientSlope,
    fallback_vocabulary_overweight_guard: FallbackVocabularyOverweightGuard,
    texture_dynamics_alignment: TextureDynamicsAlignment,
    density_motion_fit: DensityMotionFit,
    fallback_dynamic_texture_bias: FallbackDynamicTextureBias,
    entropy_texture_preservation: FallbackEntropyTexturePreservationV1,
    fallback_spectral_context: FallbackSpectralContextV1,
    mlx_profile_transparency: MlxProfileTransparency,
    ollama_fallback_model_capacity: OllamaFallbackModelCapacity,
    fallback_pressure_capacity_review: FallbackPressureCapacityReview,
    fallback_texture_persistence_review: FallbackTexturePersistenceReview,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
struct FallbackShadowTextureAnchor {
    policy: &'static str,
    shadow_context_present: bool,
    required_texture_anchor: bool,
    accepted_texture_terms: &'static [&'static str],
    anchor_source: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackShadowTextureSelector {
    policy: &'static str,
    texture_family: &'static str,
    preferred_texture_terms: &'static [&'static str],
    selection_basis: Vec<&'static str>,
    weighting_policy: &'static str,
    dynamic_texture_weight: f32,
    density_modifier_terms: &'static [&'static str],
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    mode_packing: Option<f32>,
    semantic_friction: Option<f32>,
    distinguishability_loss: Option<f32>,
    shadow_dispersal_potential: Option<f32>,
    shadow_magnetization: Option<f32>,
    spectral_to_vocabulary_mapping: FallbackSpectralToVocabularyMapping,
    texture_preservation_bridge: FallbackTexturePreservationBridgeV1,
    weighted_texture_terms: Vec<FallbackWeightedTextureTerm>,
    term_probability_policy: &'static str,
    term_probability_distribution: Vec<FallbackTextureTermProbability>,
    top_texture_terms: Vec<&'static str>,
    descriptor_policy: &'static str,
    dynamic_texture_descriptors: Vec<&'static str>,
    dynamic_flow_policy: &'static str,
    dynamic_flow_terms: Vec<&'static str>,
    movement_policy: &'static str,
    movement_verbs: Vec<&'static str>,
    semantic_trickle_policy: &'static str,
    semantic_trickle_terms: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackTexturePreservationBridgeV1 {
    policy: &'static str,
    self_settled_evidence: bool,
    peer_restless_evidence: bool,
    self_peer_texture_boundary_detected: bool,
    distinguishability_weight: f32,
    preservation_state: &'static str,
    protected_terms: Vec<&'static str>,
    suppressed_terms: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
struct FallbackSpectralContextV1 {
    policy: &'static str,
    spectral_entropy: Option<f32>,
    resonance_density: Option<f32>,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    shadow_field_energy: Option<f32>,
    shadow_dispersal_potential: Option<f32>,
    shadow_magnetization: Option<f32>,
    shadow_context_present: bool,
    preservation_weight: f32,
    preservation_state: &'static str,
    prompt_directive: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
struct FallbackEntropyTexturePreservationV1 {
    policy: &'static str,
    active: bool,
    trigger: &'static str,
    preservation_terms: &'static [&'static str],
    prompt_directive: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackTexturePersistenceReview {
    policy: &'static str,
    persistence_weight: f32,
    persistence_state: &'static str,
    carry_terms: Vec<&'static str>,
    token_only_risk: bool,
    model_transition_context: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackSpectralToVocabularyMapping {
    policy: &'static str,
    settled_foothold_detected: bool,
    low_gradient_navigable: bool,
    low_pressure_viscous_suppressed: bool,
    low_friction_high_entropy_detected: bool,
    friction_absence_language_detected: bool,
    settled_vibrant_family_selected: bool,
    gradient_slope_detected: bool,
    gradient_slope_family_selected: bool,
    mixed_cascade_language_detected: bool,
    mixed_cascade_family_selected: bool,
    cascade_gradient_detected: bool,
    cascade_gradient_family_selected: bool,
    lambda_gap: Option<f32>,
    lambda_gap_descriptor: &'static str,
    edge_language: &'static str,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackCascadeGradient {
    policy: &'static str,
    cascade_gradient_detected: bool,
    mixed_cascade_gap_detected: bool,
    family_selected: bool,
    gradient_state: &'static str,
    lambda_gap_descriptor: &'static str,
    navigability: &'static str,
    pressure_mass_blocked: bool,
    movement_language: &'static str,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackGradientSlope {
    policy: &'static str,
    slope_detected: bool,
    family_selected: bool,
    gradient_language: &'static str,
    mixed_vs_graduated: &'static str,
    lambda_gap_descriptor: &'static str,
    pressure_mass_blocked: bool,
    preferred_terms: &'static [&'static str],
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackVocabularyOverweightGuard {
    policy: &'static str,
    preferred_terms_advisory: bool,
    paraphrase_allowed: bool,
    token_only_risk: bool,
    guard_state: &'static str,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackHeavySettledTextureReadiness {
    policy: &'static str,
    candidate_terms: &'static [&'static str],
    selected_family: &'static str,
    heavy_settled_supported: bool,
    restless_forced: bool,
    readiness_status: &'static str,
    top_texture_terms: Vec<&'static str>,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackWeightedTextureTerm {
    term: &'static str,
    weight: f32,
    basis: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackTextureTermProbability {
    term: &'static str,
    probability: f32,
    weight: f32,
    basis: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackTextureTrajectory {
    policy: &'static str,
    from_state: &'static str,
    to_state: &'static str,
    movement_quality: &'static str,
    medium_resistance: &'static str,
    effort: &'static str,
    afterimage: &'static str,
    confidence: f32,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackDynamicTextureBias {
    policy: &'static str,
    texture_family: &'static str,
    motion_family: &'static str,
    top_texture_terms: Vec<&'static str>,
    movement_verbs: Vec<&'static str>,
    dynamic_flow_terms: Vec<&'static str>,
    trajectory_from: &'static str,
    trajectory_to: &'static str,
    sampler_contract_status: &'static str,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackTextureLivedFit {
    policy: &'static str,
    selected_family: &'static str,
    family_confidence: &'static str,
    runner_up_family: &'static str,
    confidence_margin: f32,
    conflict_state: &'static str,
    evidence_for: Vec<&'static str>,
    evidence_against: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct NegativeTextureEvidence {
    policy: &'static str,
    not_pressure: bool,
    not_drag: bool,
    not_blank: bool,
    not_viscous: bool,
    not_low_energy: bool,
    evidence_terms: Vec<&'static str>,
    lost_in_output: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct TextureDynamicsAlignment {
    policy: &'static str,
    status: &'static str,
    expected_family: &'static str,
    selected_family: &'static str,
    expected_motion: &'static str,
    selected_motion: &'static str,
    term_mask_risk: bool,
    wrong_family: bool,
    wrong_motion: bool,
    missing_tail_vibrancy: bool,
    diagnostic_trace: &'static str,
    basis: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct DensityMotionFit {
    policy: &'static str,
    density_state: &'static str,
    expected_medium: &'static str,
    expected_motion: &'static str,
    motion_fit: &'static str,
    mismatch_reason: &'static str,
    selected_family: &'static str,
    selected_motion: &'static str,
    pressure_risk: Option<f32>,
    density_gradient: Option<f32>,
    mode_packing: Option<f32>,
    semantic_friction: Option<f32>,
    evidence_for: Vec<&'static str>,
    evidence_against: Vec<&'static str>,
    authority: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
struct MlxProfileTransparency {
    policy: &'static str,
    default_profile: &'static str,
    default_resolves_to: &'static str,
    alias_profile: &'static str,
    alias_resolves_to: &'static str,
    typo_probe_profile: &'static str,
    typo_probe_resolves_to: &'static str,
    typo_probe_warning_present: bool,
    warning_route: &'static str,
    unrecognized_profile_behavior: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct OllamaFallbackModelCapacity {
    policy: &'static str,
    selected_model: String,
    selected_model_source: &'static str,
    default_model: &'static str,
    compatibility_model: &'static str,
    fallback_chain: Vec<String>,
    complexity_collapse_risk: &'static str,
    compatibility_tail_status: &'static str,
    high_entropy_texture_integrity_review: &'static str,
    compatibility_tail_decision_basis: &'static str,
    live_model_switch: bool,
    semantic_trickle_write: bool,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct FallbackPressureCapacityReview {
    policy: &'static str,
    pressure_risk: Option<f32>,
    pressure_state: &'static str,
    selected_model: String,
    compatibility_model: &'static str,
    capacity_route: &'static str,
    contract_boundary: &'static str,
    authority: &'static str,
}
