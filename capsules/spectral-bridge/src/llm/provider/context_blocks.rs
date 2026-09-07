#[derive(Default)]
struct DialogueContextInput<'a> {
    spectral: &'a str,
    journal: &'a str,
    direct_perception: &'a str,
    topline: &'a str,
    ambient_perception: &'a str,
    modality: &'a str,
    web: &'a str,
    continuity: &'a str,
    agenda: &'a str,
    feedback: &'a str,
    diversity: &'a str,
}

fn dialogue_context_blocks(
    input: &DialogueContextInput<'_>,
    attention: Option<&PromptAttentionV1>,
) -> (Vec<crate::prompt_budget::PromptBlock>, DialogueBlockSources) {
    let (journal_cap, journal_min) = attended_journal_caps(attention);
    let (direct_cap, direct_min, ambient_cap) = attended_perception_caps(attention);
    // Retain the live order, priorities, ATTEND caps and protected floors.
    let rows = [
        ("spectral", input.spectral, DIALOGUE_SPECTRAL_CAP, 3, 0),
        ("journal", input.journal, journal_cap, 1, journal_min),
        (
            "direct_perception",
            input.direct_perception,
            direct_cap,
            2,
            direct_min,
        ),
        (
            "topline",
            input.topline,
            DIALOGUE_TOPLINE_CAP,
            3,
            DIALOGUE_TOPLINE_MIN_CHARS,
        ),
        (
            "ambient_perception",
            input.ambient_perception,
            ambient_cap,
            5,
            0,
        ),
        ("modality", input.modality, DIALOGUE_MODALITY_CAP, 8, 0),
        ("web", input.web, attended_web_cap(attention), 6, 0),
        (
            "continuity",
            input.continuity,
            attended_continuity_cap(attention),
            7,
            0,
        ),
        (
            "agenda",
            input.agenda,
            attended_agenda_cap(attention),
            3,
            DIALOGUE_AGENDA_MIN_CHARS,
        ),
        ("feedback", input.feedback, DIALOGUE_FEEDBACK_CAP, 4, 0),
        ("diversity", input.diversity, DIALOGUE_DIVERSITY_CAP, 9, 0),
    ];
    let mut sources = DialogueBlockSources::default();
    let blocks = rows
        .into_iter()
        .map(
            |(label, content, cap, priority, min_chars)| crate::prompt_budget::PromptBlock {
                label,
                content: sources.cap(label, content, cap),
                priority,
                min_chars,
            },
        )
        .collect();
    (blocks, sources)
}
