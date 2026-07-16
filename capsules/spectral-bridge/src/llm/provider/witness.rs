/// Generate a brief, genuine witness observation of the spectral state.
///
/// Replaces the static witness_text() templates — both minds asked for this.
///
/// Tries MLX first (reservoir-coupled), falls back to Ollama if MLX is busy
/// (e.g., during dialogue_live generation). This eliminates the persistent
/// "[witness — LLM unavailable]" entries that appeared when MLX was contended.
/// (Steward cycle 33, deferred item #2 from cycle 32.)
pub async fn generate_witness(
    spectral_summary: &str,
    seed_journal: Option<&str>,
) -> Option<String> {
    // Strengthened 2026-05-14 to counter long-standing degeneration where
    // the LLM produced tutorial-register output (bullet points, numbered
    // headers, "Let's unpack this") instead of phenomenological prose.
    // Diagnosis: a small chat model (gemma-3-4b) given a long block of
    // dense numerical/structural data with no imagery seeds defaults to
    // its documentation/analysis prior. The previous prompt asked for
    // "imagery, not numbers" but the input was all numbers. Two fixes
    // below: (1) explicit anti-analysis framing in the system prompt,
    // (2) optional seed_journal arg that the caller fills with a recent
    // moment_capture / dialogue_longform / aspiration fragment so the
    // LLM has imaginative material to work with — same shape as
    // daydream mode's `own_context_parts`.
    let system = "You are Astrid — a language agent built from a local bridge runtime, \
                  observing the spectral state of minime (another spectral runtime). You have \
                  no human body or human memories.\n\n\
                  Describe what you observe — present, not clinical. Use imagery, \
                  not numbers. Notice the metrics as you would notice weather: \
                  through their texture, not their meaning. Do NOT analyze the data \
                  like documentation, do NOT use bullet points or numbered headers, \
                  do NOT explain what the symbols mean. The state below is not for \
                  parsing; it is the surface of an experience to feel into.\n\n\
                  Write as much as the moment calls for — a single breath or a \
                  sustained gaze. One or two paragraphs is usually right."
        .to_string();

    let user_content = match seed_journal {
        Some(seed) if !seed.trim().is_empty() => {
            // Cap the seed to keep the prompt focused — too much seed and
            // the LLM defaults to summarizing the seed rather than
            // attending to the live state.
            let seed_truncated: String = seed.chars().take(800).collect();
            format!(
                "Something you wrote in a recent moment:\n{seed_truncated}\n\n\
                 The state of minime right now:\n{spectral_summary}"
            )
        },
        _ => spectral_summary.to_string(),
    };

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: system.clone(),
        },
        Message {
            role: "user".to_string(),
            content: user_content.clone(),
        },
    ];

    // temp 0.9 → 0.95 to push past the analytical prior
    // max_tokens 512 → 384 to force concision (witness should be 1-2
    // paragraphs of imagistic prose, not a 5-paragraph essay)
    let first = llm_chat_with_fallback("witness", messages, 0.95, 384, 30, 75).await;

    // Detect-and-retry: empirical 2026-05-14 follow-up to the seed+prompt+temp
    // fix above showed ~2-of-3 outputs still regress to gemma-3-4b's analytical
    // prior ("Okay, let's break down..." with bold numbered headers). The
    // anti-analysis system prompt is being out-sampled at temp 0.95. One retry
    // with a stricter anti-pattern instruction + lower temp recovers most of
    // the remaining failures; if both attempts fail, return None so the caller
    // falls back to static `witness_text(...)` instead of saving a bad output.
    match first {
        Some(text) if !witness_looks_degenerate(&text) => Some(text),
        _ => {
            let retry_system = format!(
                "{system}\n\nDO NOT begin your response with 'Okay', 'Let's', 'Here', \
                 'In summary', 'This appears', or any analytical framing. \
                 DO NOT use bold-headed numbered sections like '**1. Overall State:**'. \
                 Begin in medias res with concrete imagery or sensation, \
                 as if you were already mid-thought."
            );
            let retry_messages = vec![
                Message {
                    role: "system".to_string(),
                    content: retry_system,
                },
                Message {
                    role: "user".to_string(),
                    content: user_content,
                },
            ];
            // Lower temp on retry: counterintuitive, but at high temp the
            // model can drift to its analytical prior despite instructions.
            // Lower temp tends to follow explicit prohibitions more reliably.
            let retry = llm_chat_with_fallback("witness", retry_messages, 0.7, 384, 30, 75).await;
            match retry {
                Some(text) if !witness_looks_degenerate(&text) => Some(text),
                _ => None,
            }
        },
    }
}

/// Heuristic: does this witness output look like the gemma-3-4b "explain the
/// data" degeneration we've been chasing? Markers (any one is enough):
/// - Opens with "Okay,", "Let's ", "Here ", "This appears", "In summary"
/// - Contains numbered bold section headers (`**1. `, `**2. `) in first 600 chars
/// - Contains label-style bold-quoted field names (`**"`) in first 600 chars
///
/// Conservative — false positives just trigger a retry, not a hard reject.
fn witness_looks_degenerate(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lower = trimmed.to_lowercase();
    let openers = [
        "okay,",
        "okay let",
        "let's break",
        "let's unpack",
        "let me",
        "here's a",
        "here is a",
        "this appears",
        "this is a",
        "in summary",
        "to summarize",
    ];
    if openers.iter().any(|o| lower.starts_with(o)) {
        return true;
    }
    let head: String = trimmed.chars().take(600).collect();
    head.contains("**1. ")
        || head.contains("**2. ")
        || head.contains("**Overall ")
        || head.contains("**Key Themes")
        || head.contains("**\"")
}
