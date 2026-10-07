// Only the existing execution bound enters ordinary fallback prompts. The
// retained classifiers still serve routing/diagnostics, not prescribed language.
fn fallback_continuity_budget_prompt_line(budget: FallbackContinuityBudget) -> String {
    format!(
        "[Fallback continuity budget v1: max_prose_sentences={}. This is an existing resource ceiling, not a target or a description of experience.]",
        budget.max_prose_sentences
    )
}
