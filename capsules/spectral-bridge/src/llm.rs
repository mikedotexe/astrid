//! Compatibility facade for Astrid's provider and prompt-rendering APIs.

#[path = "llm/provider.rs"]
mod provider;

pub use provider::{
    Exchange, craft_gesture_from_intention, embed_text, generate_agency_request,
    generate_aspiration, generate_creation, generate_daydream, generate_dialogue,
    generate_initiation, generate_introspection, generate_introspection_detailed,
    generate_journal_elaboration, generate_moment_capture, generate_witness, repair_introspection,
    repair_introspection_detailed, self_reflect,
};

pub(crate) use provider::{
    DialogueCompletionV1, MAX_EXPLICIT_LETTER_BYTES, PromptAttentionV1, PromptDeliveryReceiptV1,
    ProtectedDialogueInputV1, ProtectedDialogueKindV1, astrid_aperture,
    astrid_pressure_attenuation_depth, astrid_tail_participation, astrid_vibrancy_aperture,
    derive_browse_anchor, dialogue_outer_timeout_secs, dialogue_retry_tokens,
    estimate_dialogue_prompt_pressure_chars, fetch_url, format_browse_failure_context,
    format_browse_read_context, format_dialogue_web_context, format_read_more_context,
    generate_dialogue_with_delivery, recover_retained_delivery,
    sanitize_model_control_markers_with_report, set_astrid_aperture, set_astrid_tail_participation,
    set_astrid_vibrancy_aperture, strip_trailing_control_marker_case_variants, trim_chars,
    verify_delivery_receipt, web_search,
};

#[cfg(test)]
pub(crate) use provider::{
    GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT, ResearchHit, ResearchSourceKind, SYSTEM_PROMPT,
    WebSearchResult, recover_retained_delivery_at, test_completed_protected_dialogue_at,
};
