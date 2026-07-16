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
    pub before_chars: usize,
    pub after_chars: usize,
    pub removed_tokens: Vec<StripModelArtifactTokenCount>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StripModelArtifactTokenCount {
    pub token: String,
    pub count: usize,
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
