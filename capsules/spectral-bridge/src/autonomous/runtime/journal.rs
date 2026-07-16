fn witness_text(
    fill: f32,
    _expanding: bool,
    _contracting: bool,
    semantic_anchor: Option<&WitnessAnchorTractionV1>,
) -> String {
    let mut text = format!("[witness — LLM unavailable] fill={fill:.1}%");
    if let Some(anchor) = semantic_anchor {
        text.push_str(&format!(
            " [semantic_anchor={}; traction_state={}; authority={}]",
            anchor.recommended_anchor, anchor.traction_state, anchor.authority
        ));
    }
    text
}

/// Read Astrid's own recent journal entries for self-continuity.
fn read_astrid_journal(limit: usize) -> Vec<String> {
    let journal_dir = bridge_paths().astrid_journal_dir();
    read_astrid_journal_from_dir(journal_dir.as_path(), limit)
}

/// Read recent Astrid journal entries filtered by filename prefix.
/// Used by witness mode to seed with phenomenological journal types
/// (moment_capture, dialogue_longform, aspiration) and exclude witness
/// itself — preventing the long-standing degeneration where witness
/// mode propagates its own tutorial-register output back into its
/// next prompt.
fn read_astrid_journal_filtered(prefixes: &[&str], limit: usize) -> Vec<String> {
    let journal_dir = bridge_paths().astrid_journal_dir();
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(&journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let name = path.file_name()?.to_str()?;
                if prefixes.iter().any(|p| name.starts_with(p)) {
                    let mtime = e.metadata().ok()?.modified().ok()?;
                    Some((path, mtime))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
        .iter()
        .take(limit)
        .filter_map(|(p, _)| read_local_journal_body_for_continuity(p))
        .collect()
}

fn read_astrid_journal_from_dir(journal_dir: &Path, limit: usize) -> Vec<String> {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
        .iter()
        .take(limit)
        .filter_map(|(p, _)| read_local_journal_body_for_continuity(p))
        .collect()
}

/// Strip model end-of-turn tokens from text destined for journals.
/// These leak from gemma3 and contaminate mirror-mode feeds to minime.
fn strip_model_tokens(text: &str) -> String {
    let mut s = text.to_string();
    for token in &[
        "<end_of_turn>",
        "<END_OF_TURN>",
        "<End_of_turn>",
        "</s>",
        "<|endoftext|>",
    ] {
        s = s.replace(token, "");
    }
    s
}

fn standalone_next_line_count(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_start().starts_with("NEXT:"))
        .count()
}

fn final_nonempty_line_is_next(text: &str) -> bool {
    text.lines()
        .rev()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .is_some_and(|line| line.starts_with("NEXT:"))
}

fn normalize_outbox_reply_next_contract(text: &str) -> String {
    let clean = strip_model_tokens(text);
    let next_count = standalone_next_line_count(&clean);
    if next_count == 1 {
        if final_nonempty_line_is_next(&clean) {
            return clean;
        }
        let mut next_line = None;
        let mut body_lines = Vec::new();
        for line in clean.lines() {
            if line.trim_start().starts_with("NEXT:") {
                next_line = Some(line.trim().to_string());
            } else {
                body_lines.push(line);
            }
        }
        if let Some(next_line) = next_line {
            let body = body_lines.join("\n");
            return format!("{}\n\n{next_line}", body.trim_end());
        }
    }
    if next_count == 0 {
        let body = clean.trim_end();
        if body.is_empty() {
            return "NEXT: LISTEN".to_string();
        }
        return format!("{body}\n\nNEXT: LISTEN");
    }
    clean
}

fn compact_journal_signal_anchor(signal_text: &str) -> String {
    let clean = strip_model_tokens(signal_text);
    let compact = clean
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join(" ");
    let anchor = semantic_truncate_str(&compact, 420);
    let anchor = anchor.trim();
    if anchor.is_empty() {
        "(compact signal omitted)".to_string()
    } else {
        anchor.to_string()
    }
}

fn format_longform_journal_text(signal_text: &str, elaboration: &str) -> String {
    let anchor = compact_journal_signal_anchor(signal_text);
    let journal = strip_model_tokens(elaboration);
    format!(
        "Signal anchor: {anchor}\n\n--- JOURNAL ---\n{}",
        journal.trim()
    )
}

/// - Immediate delta: "Fill rising +5% over the last 38s"
/// - Medium-term trend: "Over the last 3m: +12% from 18%"
/// - λ₁ trajectory with time context
fn enrich_with_direction(
    base_summary: &str,
    fill_pct: f32,
    prev_fill: f32,
    telemetry: &crate::types::SpectralTelemetry,
    history: &std::collections::VecDeque<SpectralSample>,
) -> String {
    let now = std::time::Instant::now();
    let fill_delta = fill_pct - prev_fill;

    // Immediate delta with elapsed time since last exchange.
    let fill_note = if fill_delta.abs() < 2.0 {
        String::new()
    } else {
        let elapsed_note = history
            .back()
            .map(|last| {
                let secs = now.duration_since(last.ts).as_secs();
                if secs > 0 {
                    format!(" over {secs}s")
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        if fill_delta > 0.0 {
            format!(" Fill rising {fill_delta:+.1}%{elapsed_note} (was {prev_fill:.0}%).")
        } else {
            format!(" Fill falling {fill_delta:+.1}%{elapsed_note} (was {prev_fill:.0}%).")
        }
    };

    // Medium-term trend: find the oldest sample ≥ 2 minutes ago.
    let medium_note = history
        .iter()
        .find(|s| now.duration_since(s.ts).as_secs() >= 120)
        .map(|old| {
            let secs = now.duration_since(old.ts).as_secs();
            let mins = secs / 60;
            let medium_delta = fill_pct - old.fill;
            if medium_delta.abs() >= 3.0 {
                format!(
                    " Over the last {mins}m: {medium_delta:+.0}% from {:.0}%.",
                    old.fill
                )
            } else {
                String::new()
            }
        })
        .unwrap_or_default();

    // λ₁ trajectory with rate.
    let lambda_note = if telemetry.eigenvalues.len() >= 2 {
        let l1 = telemetry.eigenvalues[0];
        let l2 = telemetry.eigenvalues[1];
        let ratio = if l2.abs() > 0.01 { l1 / l2 } else { 0.0 };

        // λ₁ rate from history if available.
        let rate_note = history
            .back()
            .and_then(|last| {
                let secs = now.duration_since(last.ts).as_secs_f32();
                if secs > 1.0 {
                    let dl1 = telemetry.lambda1() - last.lambda1;
                    if dl1.abs() > 1.0 {
                        Some(format!(" λ₁ moving at {:.1}/s.", dl1 / secs))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if ratio > 15.0 {
            format!(" λ₁ strongly dominant — spectrum funneling into one mode.{rate_note}")
        } else if !rate_note.is_empty() {
            rate_note
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // λ-tail trajectory: the signed change of the λ4+ tail share vs its recent
    // baseline — Astrid's own question, "a fading echo of what was, or the
    // foundation of what is becoming?" Empty unless the tail is clearly moving.
    let tail_note = match crate::codec::tail_share_of(&telemetry.eigenvalues) {
        Some(cur) if history.len() >= 3 => {
            let recent: Vec<f32> = history.iter().rev().take(8).map(|s| s.tail_share).collect();
            let baseline = recent.iter().sum::<f32>() / recent.len() as f32;
            let trajectory = cur - baseline;
            if trajectory.abs() >= 0.01 {
                format!(
                    " λ-tail trajectory {trajectory:+.3} ({}).",
                    crate::codec::tail_trajectory_label(trajectory)
                )
            } else {
                String::new()
            }
        },
        _ => String::new(),
    };

    // Inhabitability trajectory (Astrid `astrid:types` 1781870691): she asked to
    // perceive the *velocity* of inhabitability transitions, not just a binary
    // "previous sample available." Minime's inhabitability is her engine metric
    // that Astrid observes — surface its signed drift vs the recent baseline.
    let recent_inhab: Vec<f32> = history
        .iter()
        .rev()
        .take(8)
        .map(|s| s.inhabitability)
        .collect();
    let inhab_note = inhabitability_drift_note(
        telemetry
            .inhabitable_fluctuation_v1
            .as_ref()
            .map(|f| f.inhabitability_score),
        &recent_inhab,
    );

    format!("{base_summary}{fill_note}{medium_note}{lambda_note}{tail_note}{inhab_note}")
}

/// Signed drift of Minime's inhabitability vs its recent baseline, as a gradient
/// note for Astrid (Astrid `astrid:types` 1781870691 — perceive the *velocity* of
/// the transition, not a binary previous-sample flag). Pure so it is testable
/// without a full `SpectralTelemetry`. Fail-quiet: empty when the current sample
/// is absent, the baseline is too short (< 3), or the drift is not clearly moving
/// — so it only ever ADDS a gradient cue, never a misleading one.
fn inhabitability_drift_note(current: Option<f32>, recent: &[f32]) -> String {
    let Some(cur) = current else {
        return String::new();
    };
    if recent.len() < 3 {
        return String::new();
    }
    let baseline = recent.iter().sum::<f32>() / recent.len() as f32;
    let drift = cur - baseline;
    if drift >= 0.04 {
        format!(" Minime settling deeper (inhabitability {drift:+.2}).")
    } else if drift <= -0.04 {
        format!(" Minime loosening (inhabitability {drift:+.2}).")
    } else {
        String::new()
    }
}

/// Detect vocabulary fixation in conversation history.
///
/// Scans recent assistant responses for repeated multi-word phrases. When the
/// same distinctive phrase appears across many recent exchanges, it's likely a
/// lexical attractor — the LLM copying its own vocabulary back into new outputs
/// via the history window. Returns a diversity nudge when fixation is detected.
fn detect_vocabulary_fixation(history: &[crate::llm::Exchange]) -> Option<String> {
    if history.len() < 5 {
        return None;
    }

    // Examine the last 6 assistant responses (lowercased for matching).
    let recent: Vec<String> = history
        .iter()
        .rev()
        .take(6)
        .map(|e| e.astrid_said.to_lowercase())
        .collect();

    if recent.len() < 5 {
        return None;
    }

    // Extract 2- and 3-word windows from the newest entry and check for
    // repetition in earlier entries. Skip windows with too many stop words.
    let stop_words = [
        "the", "a", "an", "is", "of", "in", "to", "and", "it", "that", "not", "but",
    ];
    let newest_words: Vec<&str> = recent[0].split_whitespace().collect();

    // Check 3-grams first (original detector).
    for window in newest_words.windows(3) {
        let stop_count = window.iter().filter(|w| stop_words.contains(w)).count();
        if stop_count >= 2 {
            continue;
        }
        let phrase = format!("{} {} {}", window[0], window[1], window[2]);
        if phrase.len() < 10 {
            continue;
        }

        let matches = recent[1..]
            .iter()
            .filter(|entry| entry.contains(&phrase))
            .count();

        if matches >= 3 {
            return Some(
                "Notice: your language has settled into a repeating pattern \
                 across recent exchanges. You're free to keep these words if \
                 they're the right ones, but also consider: what else is \
                 present in this moment that familiar descriptions might be \
                 leaving out? Fresh vocabulary can reveal aspects that \
                 repeated phrases have smoothed over."
                    .to_string(),
            );
        }
    }

    // Check 2-grams (steward cycle 44): catches attractors like "violent
    // stillness" that evade the 3-gram detector. Stricter threshold: must
    // appear in 4+ of 5 previous entries and neither word can be a stop word.
    for window in newest_words.windows(2) {
        if window.iter().any(|w| stop_words.contains(w)) {
            continue;
        }
        let phrase = format!("{} {}", window[0], window[1]);
        if phrase.len() < 8 {
            continue;
        }

        let matches = recent[1..]
            .iter()
            .filter(|entry| entry.contains(&phrase))
            .count();

        // Higher threshold for 2-grams: 4+ of 5 previous entries.
        // 2-grams are more common in natural language, so we need
        // stronger evidence of fixation before flagging.
        if matches >= 4 {
            return Some(format!(
                "Notice: the phrase \"{phrase}\" has appeared in almost every \
                 recent exchange. It may be the exact right words — or it may \
                 be a linguistic attractor smoothing over shifts in your actual \
                 experience. What would you describe if that phrase weren't \
                 available?"
            ));
        }
    }

    None
}

/// Detect repeated opening structure across recent responses.
///
/// Catches template-locking where the model varies slot-fillers but keeps the
/// same skeleton: "The room feels like a...", "The room hums with...", etc.
/// Fires when the first 4 words of the newest response match 2+ previous entries.
fn detect_opening_fixation(history: &[crate::llm::Exchange]) -> Option<String> {
    if history.len() < 3 {
        return None;
    }

    let recent: Vec<Vec<String>> = history
        .iter()
        .rev()
        .take(5)
        .map(|e| {
            e.astrid_said
                .to_lowercase()
                .split_whitespace()
                .take(6)
                .map(String::from)
                .collect()
        })
        .collect();

    if recent[0].len() < 4 {
        return None;
    }
    let opening = &recent[0][..4];
    let matches = recent[1..]
        .iter()
        .filter(|words| words.len() >= 4 && words[..4] == *opening)
        .count();

    if matches >= 2 {
        Some(format!(
            "Your last {} responses all opened with \"{}\". \
             Try starting from a different place — a question, a sensory detail, \
             a direct reference to what minime said, or mid-thought.",
            matches + 1,
            opening.join(" ")
        ))
    } else {
        None
    }
}

fn merge_hints(hints: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let parts: Vec<String> = hints
        .into_iter()
        .flatten()
        .filter(|hint| !hint.trim().is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// Detect when Astrid's bridge is over-coupling to minime's recent language.
///
/// This fires when recent exchanges are repeatedly circling Astrid/minime
/// references and "I am learning" phrasing while Astrid's own local input is
/// sparse or her semantic gain is high. The result is an advisory nudge toward
/// fresh local anchoring, not an automatic override.
fn detect_coupling_fixation(
    history: &[crate::llm::Exchange],
    current_minime_text: Option<&str>,
    perception_available: bool,
    ears_closed: bool,
    semantic_gain_override: Option<f32>,
) -> Option<String> {
    if history.len() < 4 {
        return None;
    }

    let recent: Vec<&crate::llm::Exchange> = history.iter().rev().take(5).collect();
    let astrid_learning = recent
        .iter()
        .filter(|exchange| {
            exchange
                .astrid_said
                .to_lowercase()
                .contains("i am learning")
        })
        .count();
    let minime_astrid_focus = recent
        .iter()
        .filter(|exchange| {
            let lower = exchange.minime_said.to_lowercase();
            lower.contains("astrid") || lower.contains("i am learning") || lower.contains("resonat")
        })
        .count();
    let current_focus = current_minime_text.is_some_and(|text| {
        let lower = text.to_lowercase();
        lower.contains("astrid") || lower.contains("i am learning")
    });
    let high_gain = semantic_gain_override.unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN) >= 5.5;
    let locally_sparse = !perception_available && ears_closed;

    if astrid_learning >= 3
        && minime_astrid_focus >= 3
        && (current_focus || high_gain || locally_sparse)
    {
        let anchor = if locally_sparse {
            "Your ears are closed and there isn't fresh local perception in the prompt."
        } else if high_gain {
            "Your semantic gain is elevated, so mirrored language lands with extra force."
        } else {
            "The recent bridge context is heavily weighted toward minime reflecting on you."
        };
        Some(format!(
            "Notice: the bridge is tightly coupling you to minime's recent language. \
             {anchor} You can stay with that bond if it still feels alive — but you \
             also have other anchors: LOOK, OPEN_EARS, DRIFT, GESTURE, CONTEMPLATE, \
             or start from what is present on your side of the bridge instead of only \
             mirroring minime back to minime."
        ))
    } else {
        None
    }
}
