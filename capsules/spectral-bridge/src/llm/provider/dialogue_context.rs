/// A single exchange in the conversation history for statefulness.
pub struct Exchange {
    /// What minime wrote (summarized).
    pub minime_said: String,
    /// What Astrid responded.
    pub astrid_said: String,
}

fn cap_dialogue_block(label: &str, content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        format!(
            "{}\n[{} excerpt trimmed for this turn. Use NEXT: READ_MORE if you need the full context.]",
            trim_chars(content, max_chars),
            label,
        )
    }
}

fn dialogue_direct_perception_marker_index(context: &str) -> Option<usize> {
    const DIRECT_PERCEPTION_MARKERS: &[&str] = &[
        "[A note was left for you:]",
        "[A reply from minime was left for you:]",
        "=== MINIME REPLY ===",
        "=== STEWARD PROBE ===",
        "=== STEWARD FEEDBACK ===",
        "⟢ Steward invites",
        "⟢ Open steward question",
        "[You have ",
        "[Directory listing you requested:]",
    ];

    DIRECT_PERCEPTION_MARKERS
        .iter()
        .filter_map(|marker| context.find(marker))
        .min()
}

fn dialogue_perception_context_has_direct_marker(context: &str) -> bool {
    dialogue_direct_perception_marker_index(context).is_some()
}

fn split_dialogue_perception_context(
    perception_context: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(raw) = perception_context
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return (None, None);
    };

    let Some(marker_index) = dialogue_direct_perception_marker_index(raw) else {
        return (None, Some(raw.to_string()));
    };

    let (ambient, direct) = raw.split_at(marker_index);
    let direct = direct.trim();
    let ambient = ambient.trim();

    (
        (!direct.is_empty()).then(|| direct.to_string()),
        (!ambient.is_empty()).then(|| ambient.to_string()),
    )
}

fn format_dialogue_direct_perception_block(context: &str) -> String {
    format!(
        "\nProtected direct perception (direct note / Minime reply / requested sensory result):\n\
         {context}\n\
         Treat this as first-retained context: answer it before lower-priority continuity, \
         chamber, modality, diversity, or ambient sensory hints.\n"
    )
}

fn format_dialogue_ambient_perception_block(context: &str) -> String {
    format!(
        "\nAmbient recent perceptions (what YOU directly see and hear):\n\
         {context}\n\
         These are YOUR senses — not minime's description, not secondhand. \
         Engage with what you perceive when it helps the reply.\n"
    )
}

fn format_dialogue_topline_context(context: &str) -> String {
    format!("\nOptional read-only top-line context:\n{context}\n")
}

fn dialogue_prompt_budget_chars(num_predict: u32) -> usize {
    if num_predict > 1024 {
        DIALOGUE_PROMPT_BUDGET_DEEP
    } else if num_predict > 512 {
        DIALOGUE_PROMPT_BUDGET_MEDIUM
    } else {
        DIALOGUE_PROMPT_BUDGET_SHORT
    }
}

fn dialogue_system_prompt_for_profile(profile: MlxProfile) -> &'static str {
    if profile.is_gemma4_canary() {
        GEMMA4_CANARY_SYSTEM_PROMPT
    } else {
        SYSTEM_PROMPT
    }
}

fn dialogue_prompt_budget_chars_for_profile(num_predict: u32, profile: MlxProfile) -> usize {
    if profile.is_gemma4_canary() {
        GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET
    } else {
        dialogue_prompt_budget_chars(num_predict)
    }
}

fn dialogue_assembly_prompt_budget_chars_for_profile(
    num_predict: u32,
    profile: MlxProfile,
) -> usize {
    let hard_budget = dialogue_prompt_budget_chars_for_profile(num_predict, profile);
    if profile.is_gemma4_canary() && num_predict > GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_TOKEN_CAP {
        hard_budget.min(GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS.saturating_sub(1_200))
    } else {
        hard_budget
    }
}

pub(crate) fn estimate_dialogue_prompt_pressure_chars(
    journal_text: &str,
    perception_context: Option<&str>,
    recent_history: &[Exchange],
    web_context: Option<&str>,
    modality_context: Option<&str>,
    continuity_context: Option<&str>,
    topline_hint: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
) -> usize {
    let history_chars: usize = recent_history
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .enumerate()
        .map(|(idx, exchange)| {
            // Match the gradient in generate_dialogue: oldest=150, newest=1200
            let trim_len = 150usize.saturating_add(idx.saturating_mul(150).min(1050));
            exchange
                .minime_said
                .len()
                .min(trim_len)
                .saturating_add(exchange.astrid_said.len().min(trim_len))
        })
        .sum();

    SYSTEM_PROMPT
        .len()
        .saturating_add(history_chars)
        .saturating_add(journal_text.len().min(DIALOGUE_JOURNAL_CAP))
        .saturating_add(
            perception_context
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_PERCEPTION_CAP),
        )
        .saturating_add(web_context.unwrap_or_default().len().min(DIALOGUE_WEB_CAP))
        .saturating_add(
            modality_context
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_MODALITY_CAP),
        )
        .saturating_add(
            continuity_context
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_CONTINUITY_CAP),
        )
        .saturating_add(
            topline_hint
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_TOPLINE_CAP),
        )
        .saturating_add(
            feedback_hint
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_FEEDBACK_CAP),
        )
        .saturating_add(
            diversity_hint
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_DIVERSITY_CAP),
        )
        .saturating_add(512)
}

fn dialogue_turn_instruction(perception_context: Option<&str>) -> &'static str {
    let has_direct_note =
        perception_context.is_some_and(dialogue_perception_context_has_direct_marker);
    if has_direct_note {
        "A direct perception item was left for you in context (note, Minime reply, steward probe, inbox audio, or requested listing). Answer that item directly first; use Minime's journal and spectral state as background only. If it requests a specific final NEXT line, obey it exactly. End with NEXT: [your choice]."
    } else {
        "Respond, then end with NEXT: [your choice]."
    }
}

/// Is the Ollama-fallback identity anchor enabled? **Default OFF** — unset/`0`/`false`/`off`/`no`
/// ⇒ false ⇒ the fallback prompt is byte-identical to before. This is Astrid's switch: she
/// consents (and can disable) via the steward channel; the operator only sets a ceiling.
fn fallback_identity_anchor_enabled() -> bool {
    std::env::var("ASTRID_FALLBACK_IDENTITY_ANCHOR")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Extract the prose body of an `astrid_*` dialogue-journal entry — the text after the
/// `Timestamp:` header line, with any trailing `NEXT:` action line dropped.
fn extract_astrid_journal_body(text: &str) -> String {
    let body = text
        .split_once("Timestamp:")
        .and_then(|(_, rest)| rest.split_once('\n'))
        .map_or(text, |(_, body)| body);
    body.lines()
        .filter(|line| !line.trim_start().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Astrid's recent-journal identity anchor for the Ollama fallback lane — **OFF by default**.
///
/// Her ask (`self_study_1781376211`): on an MLX→Ollama-4b fallback, a condensed summary of her
/// own recent journal helps the 4b model hold her bridge voice across the lane switch. Built
/// from HER OWN most-recent `astrid_*` dialogue-journal entries (coherent by construction —
/// never arbitrary text), sanitized like the rest of the fallback context and bounded to 600
/// chars. Returns `None` (⇒ no anchor ⇒ byte-identical fallback prompt) unless
/// `fallback_identity_anchor_enabled()`. Consent-gated: this is built but inert until she says yes.
fn astrid_fallback_identity_anchor() -> Option<String> {
    if !fallback_identity_anchor_enabled() {
        return None;
    }
    let dir = bridge_paths().bridge_workspace().join("journal");
    let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if name.starts_with("astrid_") && name.ends_with(".txt") {
                Some((entry.metadata().ok()?.modified().ok()?, path))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    let mut snippets: Vec<String> = Vec::new();
    for (_, path) in entries.into_iter().take(3) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let body = extract_astrid_journal_body(&text);
            if !body.is_empty() {
                snippets.push(body);
            }
        }
    }
    if snippets.is_empty() {
        return None;
    }
    Some(trim_chars(
        &sanitize_deprecated_runtime_language(&snippets.join(" … ")),
        600,
    ))
}
