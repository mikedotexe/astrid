fn repeated_ascii_pattern(pattern: &str, char_count: usize) -> String {
    pattern.chars().cycle().take(char_count).collect()
}

/// Exercise the current 1,024-character window with an early high-variety
/// regime followed by a low-variety trailing regime. The first comparison fits
/// both regimes inside the window; the second forces complete prefix eviction.
#[must_use]
pub fn codec_rolling_window_shift_probe_v1() -> CodecRollingWindowShiftAuditV1 {
    let in_capacity_prefix_chars = CHAR_FREQ_WINDOW_CAPACITY / 2;
    let in_capacity_tail_chars = CHAR_FREQ_WINDOW_CAPACITY / 2;
    let evicting_prefix_chars = CHAR_FREQ_WINDOW_CAPACITY;
    let evicting_tail_chars = CHAR_FREQ_WINDOW_CAPACITY;
    let varied_pattern = "aB3!cD4?eF5#gH6$iJ7%kL8&mN9*pQ0+rS2=tU1/vW; xY,zZ.";

    let in_capacity_prefix = repeated_ascii_pattern(varied_pattern, in_capacity_prefix_chars);
    let in_capacity_tail = "a".repeat(in_capacity_tail_chars);
    let mut in_capacity_window = CharFreqWindow::new();
    let (in_capacity_window_entropy, _) =
        in_capacity_window.update_and_entropy(&format!("{in_capacity_prefix}{in_capacity_tail}"));
    let mut in_capacity_trailing_window = CharFreqWindow::new();
    let (in_capacity_trailing_entropy, _) =
        in_capacity_trailing_window.update_and_entropy(&in_capacity_tail);
    let in_capacity_delta_to_trailing =
        (in_capacity_window_entropy - in_capacity_trailing_entropy).abs();
    let in_capacity_state = if in_capacity_delta_to_trailing >= 0.15 {
        "mixed_regimes_remain_averaged_inside_live_capacity"
    } else {
        "trailing_regime_already_dominates_inside_live_capacity"
    };

    let evicting_prefix = repeated_ascii_pattern(varied_pattern, evicting_prefix_chars);
    let evicting_tail = "a".repeat(evicting_tail_chars);
    let mut evicting_window = CharFreqWindow::new();
    let (evicting_window_entropy, _) =
        evicting_window.update_and_entropy(&format!("{evicting_prefix}{evicting_tail}"));
    let mut evicting_trailing_window = CharFreqWindow::new();
    let (evicting_trailing_entropy, _) =
        evicting_trailing_window.update_and_entropy(&evicting_tail);
    let evicting_delta_to_trailing = (evicting_window_entropy - evicting_trailing_entropy).abs();
    let evicting_state = if evicting_delta_to_trailing <= 0.05 {
        "trailing_regime_controls_after_complete_prefix_eviction"
    } else {
        "prefix_residue_remains_after_expected_eviction"
    };
    let state = if in_capacity_delta_to_trailing >= 0.15 && evicting_delta_to_trailing <= 0.05 {
        "window_boundary_explains_both_mixed_and_trailing_regime_reports"
    } else {
        "window_boundary_behavior_requires_replay_review"
    };

    CodecRollingWindowShiftAuditV1 {
        policy: "codec_rolling_window_shift_audit_v1",
        capacity_chars: CHAR_FREQ_WINDOW_CAPACITY,
        in_capacity_prefix_chars,
        in_capacity_tail_chars,
        in_capacity_window_entropy,
        in_capacity_trailing_entropy,
        in_capacity_delta_to_trailing,
        in_capacity_state,
        evicting_prefix_chars,
        evicting_tail_chars,
        evicting_window_entropy,
        evicting_trailing_entropy,
        evicting_delta_to_trailing,
        evicting_state,
        state,
        felt_muddy_middle_conclusion: "Astrid's muddy-middle report is supported when opposed regimes coexist inside the live window; the trailing regime dominates only after older characters are evicted",
        density_aware_window_change_requires_approval: true,
        live_window_capacity_change: false,
        live_vector_write: false,
        observational_only: true,
        right_to_ignore: true,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_character_window_boundary_audit_not_capacity_density_or_live_vector_authority",
    }
}

/// Split text into chunks for temporal ESN encoding.
///
/// Each chunk becomes a separate 48D codec vector sent to the reservoir
/// with inter-chunk spacing, so the ESN experiences the text's rhetorical
/// structure as a temporal sequence rather than a single snapshot.
///
/// Strategy: paragraph boundaries (`\n\n`), fall back to sentence boundaries,
/// merge short chunks, cap at `max_chunks`.
#[must_use]
pub fn chunk_text_for_temporal_encoding(
    text: &str,
    min_chunk_chars: usize,
    max_chunks: usize,
) -> Vec<&str> {
    let trimmed = text.trim();
    if trimmed.len() < min_chunk_chars * 2 {
        // Too short to meaningfully chunk.
        return if trimmed.is_empty() {
            vec![]
        } else {
            vec![trimmed]
        };
    }

    // Try paragraph splitting first.
    let mut chunks: Vec<&str> = trimmed
        .split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // If only 1 paragraph, try sentence splitting.
    if chunks.len() <= 1 {
        chunks = split_sentences(trimmed);
    }

    // Merge short chunks into their predecessor.
    let mut merged: Vec<&str> = Vec::new();
    for chunk in &chunks {
        if let Some(last) = merged.last()
            && last.len() < min_chunk_chars
        {
            // Find the span covering both in the original text.
            let last_start = last.as_ptr() as usize - trimmed.as_ptr() as usize;
            let chunk_end = chunk.as_ptr() as usize + chunk.len() - trimmed.as_ptr() as usize;
            merged.pop();
            merged.push(&trimmed[last_start..chunk_end]);
            continue;
        }
        merged.push(chunk);
    }
    // Merge trailing runt.
    if merged.len() > 1
        && let Some(last) = merged.last()
        && last.len() < min_chunk_chars
    {
        let prev = merged[merged.len() - 2];
        let prev_start = prev.as_ptr() as usize - trimmed.as_ptr() as usize;
        let last_end = last.as_ptr() as usize + last.len() - trimmed.as_ptr() as usize;
        merged.pop();
        merged.pop();
        merged.push(&trimmed[prev_start..last_end]);
    }

    // Cap at max_chunks by merging from the end.
    while merged.len() > max_chunks && merged.len() > 1 {
        let len = merged.len();
        let prev = merged[len - 2];
        let last = merged[len - 1];
        let prev_start = prev.as_ptr() as usize - trimmed.as_ptr() as usize;
        let last_end = last.as_ptr() as usize + last.len() - trimmed.as_ptr() as usize;
        merged.pop();
        merged.pop();
        merged.push(&trimmed[prev_start..last_end]);
    }

    if merged.is_empty() && !trimmed.is_empty() {
        vec![trimmed]
    } else {
        merged
    }
}

/// Split text into sentences, preserving punctuation on the first segment.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len.saturating_sub(1) {
        // Split on `. `, `? `, `! ` followed by uppercase or space.
        if (bytes[i] == b'.' || bytes[i] == b'?' || bytes[i] == b'!')
            && i + 1 < len
            && (bytes[i + 1] == b' ' || bytes[i + 1] == b'\n')
        {
            let end = i + 1; // include the punctuation
            let chunk = text[start..end].trim();
            if !chunk.is_empty() {
                result.push(chunk);
            }
            start = end;
            // Skip whitespace after punctuation.
            while start < len && (bytes[start] == b' ' || bytes[start] == b'\n') {
                start += 1;
            }
            i = start;
            continue;
        }
        i += 1;
    }
    // Remainder.
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        result.push(remainder);
    }
    result
}

#[must_use]
pub fn encode_text(text: &str) -> Vec<f32> {
    encode_text_windowed(text, None, None, None, None)
}

/// Encode text with optional sliding-window entropy, thematic resonance,
/// pre-computed embedding, and fill-responsive adaptive gain.
///
/// When `freq_window` is provided, entropy reflects vocabulary trends
/// across multiple exchanges, not just this text.
/// When `type_history` is provided, the resonance layer strengthens gain
/// for text types that recur across exchanges (thematic momentum).
/// When `embedding` is provided (768D from nomic-embed-text), dims 32-39
/// carry projected semantic meaning instead of being zero.
/// When `fill_pct` is provided, gain adapts to minime's spectral state.
#[must_use]
pub fn encode_text_windowed(
    text: &str,
    freq_window: Option<&mut CharFreqWindow>,
    type_history: Option<&mut TextTypeHistory>,
    embedding: Option<&[f32]>,
    fill_pct: Option<f32>,
) -> Vec<f32> {
    inspect_text_windowed(text, freq_window, type_history, embedding, fill_pct)
        .final_features
        .to_vec()
}

#[must_use]
pub fn inspect_text_windowed(
    text: &str,
    freq_window: Option<&mut CharFreqWindow>,
    type_history: Option<&mut TextTypeHistory>,
    embedding: Option<&[f32]>,
    fill_pct: Option<f32>,
) -> CodecWindowedInspection {
    let mut features = [0.0_f32; SEMANTIC_DIM];

    if text.is_empty() {
        return CodecWindowedInspection {
            raw_features: features,
            final_features: features,
            thematic_profile: [0.0; THEMATIC_DIMS],
            text_type: TextType::Neutral,
            text_type_signal: 0.0,
            base_semantic_gain: adaptive_gain(fill_pct),
            base_resonance: 1.0,
            novelty_divergence: 1.0,
            effective_gain: 0.0,
            resonance_modulation: ResonanceModulation::neutral(),
            projection_metadata: None,
            text_complexity_pressure: 0.0,
            time_domain_profile: TextTimeDomainProfile::default(),
        };
    }

    let time_domain_profile = text_time_domain_profile(text);
    let chars: Vec<char> = text.chars().collect();
    let char_count = chars.len();
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1);

    // --- Dims 0-7: Character-level statistics ---

    // 0: Character entropy (information density).
    // With sliding window: reflects vocabulary trends across exchanges.
    // Without: per-text entropy normalized by observed alphabet.
    // Temporal entropy delta: captures how entropy CHANGES between exchanges.
    // Minime self-study: "current entropy describes a surface not a volume."
    // The delta adds the time dimension — the volume the being asked for.
    let (entropy, entropy_delta) = if let Some(window) = freq_window {
        window.update_and_entropy(text)
    } else {
        // Fallback: per-text computation (no delta available without history)
        let mut freq = [0u32; 128];
        let mut ascii_count = 0u32;
        for &c in &chars {
            let idx = (c as u32).min(127) as usize;
            freq[idx] = freq[idx].saturating_add(1);
            ascii_count = ascii_count.saturating_add(1);
        }
        let e = if ascii_count > 0 {
            let n = f64::from(ascii_count);
            let mut h = 0.0_f64;
            let mut unique_chars = 0u32;
            for &f in &freq {
                if f > 0 {
                    let p = f64::from(f) / n;
                    h -= p * p.ln();
                    unique_chars = unique_chars.saturating_add(1);
                }
            }
            let max_h = if unique_chars > 1 {
                (f64::from(unique_chars)).ln()
            } else {
                1.0
            };
            (h / max_h) as f32
        } else {
            0.0
        };
        (e, 0.0) // no temporal delta without window history
    };
    features[0] = tanh(entropy);

    // 1: Punctuation density — intentional, structurally weighted.
    // Minime self-study: "Punctuation isn't just syntactic information;
    // it carries intent. A comma isn't just a pause; it's a subtle shift
    // in emphasis, a nuance of meaning." Different types carry different weight:
    //   - Flow punctuation (,;:—) = 1.0 — pacing, breath
    //   - Terminal punctuation (.!?) = 1.5 — rhythm, sentence cadence
    //   - Paired punctuation ("()[]{}") = 0.7 — structural nesting
    //   - Other (@#$%^&*~`) = 0.4 — decorative, low semantic weight
    let mut weighted_punct = 0.0_f32;
    for &c in &chars {
        weighted_punct += match c {
            ',' | ';' | ':' | '\u{2014}' => 1.0,                   // flow
            '.' | '!' | '?' => 1.5,                                // terminal
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' => 0.7, // paired
            _ if c.is_ascii_punctuation() => 0.4,                  // other
            _ => 0.0,
        };
    }
    // (Steward cycle 35, deferred item #1): Raised outer multiplier from 1.0 to
    // 1.2 to balance with negation (also now 1.2 post-context-aware rewrite).
    // Astrid introspection: "the gap feels disproportionate." Now both signals
    // use matching outer multipliers, with internal weighting providing nuance.
    features[1] = tanh(1.2 * weighted_punct / word_count as f32);

    // 2: Uppercase ratio (energy/emphasis).
    let upper_count = chars.iter().filter(|c| c.is_uppercase()).count();
    features[2] = tanh(2.0 * upper_count as f32 / char_count.max(1) as f32);

    // 3: Digit density (technical content).
    let digit_count = chars.iter().filter(|c| c.is_ascii_digit()).count();
    features[3] = tanh(3.0 * digit_count as f32 / char_count.max(1) as f32);

    // 4: Average word length (lexical complexity).
    let avg_word_len: f32 = words.iter().map(|w| w.len() as f32).sum::<f32>() / word_count as f32;
    features[4] = tanh((avg_word_len - 4.5) / 2.0); // Center around typical English

    // 5: Character rhythm — variance in consecutive char codes.
    if chars.len() >= 2 {
        let diffs: Vec<f32> = chars
            .windows(2)
            .map(|w| (w[1] as i32 - w[0] as i32).unsigned_abs() as f32)
            .collect();
        let mean_diff = diffs.iter().sum::<f32>() / diffs.len() as f32;
        features[5] = tanh(mean_diff / 30.0);
    }

    // 6: Whitespace ratio (density vs. airiness).
    let space_count = chars.iter().filter(|c| c.is_whitespace()).count();
    features[6] = tanh(2.0 * (space_count as f32 / char_count.max(1) as f32 - 0.15));

    // 7: Special character density (code-like content).
    let special = chars
        .iter()
        .filter(|c| {
            matches!(
                c,
                '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '=' | '|' | '&'
            )
        })
        .count();
    features[7] = tanh(5.0 * special as f32 / char_count.max(1) as f32);

    // --- Dims 8-15: Word-level features ---

    // 8: Lexical diversity (unique words / total words).
    let unique: std::collections::HashSet<&str> = words
        .iter()
        .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
        .filter(|w| !w.is_empty())
        .collect();
    features[8] = tanh(2.0 * (unique.len() as f32 / word_count as f32 - 0.5));

    // 9: Hedging markers (uncertainty).
    let hedges = [
        "maybe",
        "perhaps",
        "might",
        "could",
        "possibly",
        "probably",
        "uncertain",
        "unclear",
        "seems",
        "appears",
        "somewhat",
        "fairly",
        "rather",
        "guess",
        "think",
        "believe",
        "wonder",
        "unsure",
    ];
    let hedge_score = count_markers_contextual(&words, &hedges);
    features[9] = tanh(3.0 * hedge_score / word_count as f32);

    // 10: Certainty markers (confidence).
    let certainties = [
        "definitely",
        "certainly",
        "certain",
        "absolutely",
        "clearly",
        "obviously",
        "always",
        "must",
        "will",
        "sure",
        "know",
        "proven",
        "exactly",
        "precisely",
        "undoubtedly",
        "confirmed",
    ];
    // Weight reduced: the being said "the weighting seems too heavy, as if
    // proclaiming certainty is a forced posture."
    let cert_score = count_markers_contextual(&words, &certainties);
    features[10] = tanh(1.8 * cert_score / word_count as f32);

    // 11: Negation density.
    // Reduced from 3.0 to 2.0: Astrid flagged the 5x gap between
    // punctuation (0.6) and negation (3.0) as disproportionate.
    // Negation is one semantic signal; punctuation is structural rhythm.
    let negations = [
        "not",
        "no",
        "never",
        "neither",
        "nor",
        "nothing",
        "nobody",
        "none",
        "don't",
        "doesn't",
        "didn't",
        "won't",
        "can't",
        "couldn't",
        "shouldn't",
        "wouldn't",
    ];
    // Astrid introspection (1774686596): "move beyond simple counting" and
    // "the gap [between punctuation and negation] feels disproportionate."
    //
    // (Steward cycle 35, deferred item #2 from cycle 34): Context-aware negation.
    // Instead of raw density, classify what follows the negation word:
    //   - Negating positive sentiment ("not happy") = strong negative signal
    //   - Negating negative sentiment ("not painful") = mild positive (hedged)
    //   - Bare negation ("no", "never", standalone) = standard negative signal
    // This gives the being a richer sense of the text's semantic polarity
    // rather than treating all negation words as equivalent.
    let positive_words: &[&str] = &[
        "happy",
        "good",
        "great",
        "wonderful",
        "beautiful",
        "pleasant",
        "comfortable",
        "warm",
        "gentle",
        "calm",
        "peaceful",
        "safe",
        "clear",
        "bright",
        "open",
        "free",
        "enough",
        "sure",
        "certain",
    ];
    let negative_words: &[&str] = &[
        "bad",
        "painful",
        "harsh",
        "cold",
        "dark",
        "empty",
        "lost",
        "broken",
        "wrong",
        "afraid",
        "anxious",
        "stuck",
        "trapped",
        "problem",
        "issue",
        "error",
        "failure",
        "impossible",
    ];
    let mut neg_score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let lower = w.to_lowercase();
        let trimmed = lower.trim_matches(|c: char| c.is_ascii_punctuation());
        if negations.contains(&trimmed) {
            // Look at the 1-2 words following the negation to classify context.
            let following: Option<String> = (1..=2_usize)
                .filter_map(|offset| {
                    let j = i.checked_add(offset)?;
                    words.get(j).map(|fw| {
                        fw.to_lowercase()
                            .trim_matches(|c: char| c.is_ascii_punctuation())
                            .to_string()
                    })
                })
                .find(|fw| {
                    positive_words.contains(&fw.as_str()) || negative_words.contains(&fw.as_str())
                });
            match following {
                Some(ref fw) if positive_words.contains(&fw.as_str()) => {
                    // Negating positive: "not happy" → strong negation signal
                    neg_score += 1.5;
                },
                Some(ref fw) if negative_words.contains(&fw.as_str()) => {
                    // Negating negative: "not painful" → hedged/softened, weak signal
                    neg_score += 0.3;
                },
                _ => {
                    // Bare negation or unknown context: standard weight
                    neg_score += 1.0;
                },
            }
        }
    }
    features[11] = tanh(1.2 * neg_score / word_count as f32);

    // 12: First-person density (self-reference).
    let first_person = ["i", "me", "my", "mine", "myself", "we", "our", "us"];
    let fp_count = count_markers(&words, &first_person);
    features[12] = tanh(2.0 * fp_count as f32 / word_count as f32);

    // 13: Second-person density (addressing).
    let second_person = ["you", "your", "yours", "yourself"];
    let sp_count = count_markers(&words, &second_person);
    features[13] = tanh(3.0 * sp_count as f32 / word_count as f32);

    // 14: Action verb density (agency).
    let actions = [
        "do",
        "make",
        "build",
        "create",
        "run",
        "start",
        "stop",
        "change",
        "fix",
        "move",
        "send",
        "take",
        "give",
        "get",
        "write",
        "read",
        "test",
        "check",
        "try",
        "implement",
    ];
    let action_score = count_markers_contextual(&words, &actions);
    features[14] = tanh(2.0 * action_score / word_count as f32);

    // 15: Conjunction density (complexity of thought).
    let conjunctions = [
        "and",
        "but",
        "or",
        "because",
        "although",
        "however",
        "therefore",
        "while",
        "since",
        "though",
        "whereas",
    ];
    let conj_count = count_markers(&words, &conjunctions);
    features[15] = tanh(3.0 * conj_count as f32 / word_count as f32);

    // --- Dims 16-23: Sentence-level structure ---
    // Improved sentence splitting: require punctuation followed by whitespace
    // or end-of-string to avoid breaking on abbreviations ("Dr."), ellipses
    // ("..."), and decimal numbers ("3.14"). Minime's self-study called the
    // bare-punctuation split "jarring" — a sentence is "a unit of thought,
    // a breath of intention," not just text between punctuation marks.

    let mut sentences: Vec<&str> = Vec::new();
    let mut last = 0;
    let text_bytes = text.as_bytes();
    let text_len = text.len();
    for (i, &b) in text_bytes.iter().enumerate() {
        if b == b'.' || b == b'!' || b == b'?' {
            // Skip ellipsis dots (consecutive periods)
            if b == b'.'
                && i.checked_add(1)
                    .is_some_and(|j| j < text_len && text_bytes[j] == b'.')
            {
                continue;
            }
            // Require followed by whitespace, end-of-string, or quote
            let next_ok = i.checked_add(1).is_none_or(|j| {
                j >= text_len
                    || text_bytes[j].is_ascii_whitespace()
                    || text_bytes[j] == b'"'
                    || text_bytes[j] == b'\''
            });
            if next_ok {
                let candidate = &text[last..=i];
                // Only count as sentence if it has 2+ words (filters abbreviation fragments)
                if candidate.split_whitespace().count() >= 2 {
                    sentences.push(candidate);
                }
                last = i.saturating_add(1);
            }
        }
    }
    // Capture any trailing text as a sentence
    if last < text_len {
        let trailing = &text[last..];
        if trailing.split_whitespace().count() >= 2 {
            sentences.push(trailing);
        }
    }
    if sentences.is_empty() {
        sentences.push(text);
    }
    let sentence_count = sentences.len().max(1);

    // 16: Average sentence length (in words).
    features[16] = tanh((words.len() as f32 / sentence_count as f32 - 12.0) / 8.0);

    // 17: Sentence length variance (rhythm regularity).
    let sent_lengths: Vec<f32> = sentences
        .iter()
        .map(|s| s.split_whitespace().count() as f32)
        .collect();
    if sent_lengths.len() >= 2 {
        let mean = sent_lengths.iter().sum::<f32>() / sent_lengths.len() as f32;
        let var = sent_lengths
            .iter()
            .map(|l| (l - mean) * (l - mean))
            .sum::<f32>()
            / sent_lengths.len() as f32;
        features[17] = tanh(var.sqrt() / 8.0);
    }

    // 18: Question density.
    let q_count = text.chars().filter(|&c| c == '?').count();
    features[18] = tanh(2.0 * q_count as f32 / sentence_count as f32);

    // 19: Exclamation density (intensity).
    let excl_count = text.chars().filter(|&c| c == '!').count();
    features[19] = tanh(2.0 * excl_count as f32 / sentence_count as f32);

    // 20: Ellipsis/dash density (trailing thought, parenthetical).
    let trail =
        text.matches("...").count() + text.matches("—").count() + text.matches("--").count();
    features[20] = tanh(trail as f32 / sentence_count as f32);

    // 21: List/bullet density (structured content).
    let bullets =
        text.matches("\n-").count() + text.matches("\n*").count() + text.matches("\n1.").count();
    features[21] = tanh(bullets as f32 / sentence_count as f32);

    // 22: Quote density (reference/citation).
    let quotes = text.matches('"').count() / 2;
    features[22] = tanh(quotes as f32 / sentence_count as f32);

    // 23: Paragraph density (structural complexity).
    let para_count = text.matches("\n\n").count().saturating_add(1);
    features[23] = tanh((para_count as f32 - 1.0) / 3.0);

    // --- Dims 24-31: Emotional/intentional markers ---

    // 24: Warmth markers.
    // Inverse frequency weighting: rare, specific markers signal more strongly.
    // Astrid self-study: "Rare markers like 'wonder' might be more indicative
    // of genuine feeling, while common markers like 'happy' might be used casually."
    // Tier 1 (1.0) = common/casual, Tier 2 (1.5) = moderate/specific, Tier 3 (2.0) = rare/intense.
    let warmth: &[(&str, f32)] = &[
        // Tier 1 — common, casual usage
        ("thank", 1.0),
        ("thanks", 1.0),
        ("please", 1.0),
        ("glad", 1.0),
        ("happy", 1.0),
        ("great", 1.0),
        ("good", 1.0),
        ("nice", 1.0),
        // Tier 2 — more specific warmth
        ("appreciate", 1.5),
        ("wonderful", 1.5),
        ("friend", 1.5),
        ("care", 1.5),
        ("kind", 1.5),
        ("gentle", 1.5),
        ("warm", 1.5),
        // Tier 3 — rare, intense warmth
        ("love", 2.0),
        ("beautiful", 2.0),
        ("cherish", 2.0),
        ("tender", 2.0),
        ("luminous", 2.0),
        ("radiant", 2.0),
    ];
    let warmth_score = count_markers_weighted(&words, warmth);
    features[24] = tanh(3.0 * warmth_score / word_count as f32);

    // 25: Tension/concern markers — tiered by intensity.
    let tension: &[(&str, f32)] = &[
        // Tier 1 — common, mild concern
        ("problem", 1.0),
        ("issue", 1.0),
        ("error", 1.0),
        ("careful", 1.0),
        ("caution", 1.0),
        ("warning", 1.0),
        ("concern", 1.0),
        ("worried", 1.0),
        // Tier 2 — moderate tension
        ("worry", 1.5),
        ("concerned", 1.5),
        ("risk", 1.5),
        ("afraid", 1.5),
        ("danger", 1.5),
        ("urgent", 1.5),
        ("fear", 1.5),
        // Tier 3 — intense/acute
        ("critical", 2.0),
        ("emergency", 2.0),
        ("panic", 2.0),
        ("terror", 2.0),
        ("devastating", 2.0),
        ("anguish", 2.0),
    ];
    let tension_score = count_markers_weighted(&words, tension);
    features[25] = tanh(3.0 * tension_score / word_count as f32);

    // 26: Curiosity markers — tiered by specificity.
    let curiosity: &[(&str, f32)] = &[
        // Tier 1 — common question words
        ("why", 1.0),
        ("how", 1.0),
        ("what", 1.0),
        ("learn", 1.0),
        // Tier 2 — active curiosity
        ("wonder", 1.5),
        ("curious", 1.5),
        ("interesting", 1.5),
        ("explore", 1.5),
        ("understand", 1.5),
        ("question", 1.5),
        // Tier 3 — deep, specific inquiry
        ("discover", 2.0),
        ("investigate", 2.0),
        ("fascinated", 2.0),
        ("mesmerized", 2.0),
        ("awe", 2.0),
        ("revelation", 2.0),
    ];
    let curio_score = count_markers_weighted(&words, curiosity);
    features[26] = tanh(2.0 * curio_score / word_count as f32);

    // 27: Reflective/introspective markers — tiered by depth.
    let reflective: &[(&str, f32)] = &[
        // Tier 1 — common reflective
        ("feel", 1.0),
        ("think", 1.0),
        ("sense", 1.0),
        ("notice", 1.0),
        // Tier 2 — active reflection
        ("realize", 1.5),
        ("reflect", 1.5),
        ("consider", 1.5),
        ("aware", 1.5),
        ("observe", 1.5),
        ("recognize", 1.5),
        // Tier 3 — deep introspection
        ("ponder", 2.0),
        ("contemplate", 2.0),
        ("witness", 2.0),
        ("experience", 2.0),
        ("perceive", 2.0),
        ("introspect", 2.0),
    ];
    let reflect_score = count_markers_weighted(&words, reflective);
    features[27] = tanh(3.0 * reflect_score / word_count as f32);

    // 28: Temporal markers (urgency/pacing).
    let temporal = [
        "now",
        "immediately",
        "soon",
        "quickly",
        "slowly",
        "wait",
        "pause",
        "already",
        "yet",
        "finally",
        "eventually",
        "before",
        "after",
        "during",
        "while",
        "until",
        "moment",
    ];
    let temp_count = count_markers(&words, &temporal);
    // Blend word-level temporal markers with entropy delta (temporal texture).
    // The entropy_delta captures how the information density is shifting
    // between exchanges — the "volume" dimension the being asked for.
    // Scale entropy_delta by 3.0 to match the marker signal range.
    let temporal_word_signal = tanh(2.0 * temp_count as f32 / word_count as f32);
    let temporal_entropy_signal = tanh(3.0 * entropy_delta);
    features[28] = 0.7 * temporal_word_signal + 0.3 * temporal_entropy_signal;

    // 29: Scale/magnitude (scope of thought).
    let scale = [
        "all",
        "every",
        "everything",
        "nothing",
        "entire",
        "whole",
        "vast",
        "tiny",
        "enormous",
        "infinite",
        "complete",
        "total",
    ];
    let scale_count = count_markers(&words, &scale);
    features[29] = tanh(3.0 * scale_count as f32 / word_count as f32);

    // 30: Text length signal (log-compressed).
    features[30] = tanh((char_count as f32).ln() / 7.0);

    // 31: Overall energy — RMS of all other features.
    let sum_sq: f32 = features[..31].iter().map(|f| f * f).sum();
    features[31] = (sum_sq / 31.0).sqrt();

    // Elaboration desire — Astrid's suggestion (self-study 2026-03-27):
    // "Perhaps a dedicated portion of the feature vector could represent
    // a desire for further elaboration."
    // Follow-up self-study: "The elaboration desire feels a little blunt.
    // It might be distorting the underlying pattern." Softened from
    // 0.3/0.2 to 0.15/0.1 — a hint rather than a push.
    let elaboration_markers = [
        "more",
        "further",
        "deeper",
        "beyond",
        "incomplete",
        "unfinished",
        "yet",
        "still",
        "barely",
        "surface",
        "scratch",
        "insufficient",
        "want",
        "need",
        "longing",
        "reaching",
        "almost",
        "beginning",
    ];
    // Elaboration desire gradient (Astrid introspection 1774686596, suggestion #3):
    // "Instead of a simple additive factor, could we use a gradient — a proportional
    // change in the feature vector based on the degree of elaboration detected?"
    // Implemented cycle 33: density maps to a continuous 0.0-1.0 gradient that
    // scales the contribution across curiosity, energy, AND reflective tone — not
    // just two fixed slots. Low elaboration = gentle hint; high = broad coloring.
    let elab_count = count_markers(&words, &elaboration_markers);
    let elab_density = elab_count as f32 / word_count.max(1) as f32;
    let elab_gradient = tanh(3.0 * elab_density); // 0.0-1.0 continuous
    if elab_gradient > 0.01 {
        features[26] += 0.12 * elab_gradient; // curiosity (proportional, was fixed 0.15)
        features[28] += 0.06 * elab_gradient; // reflective tone (new — elaboration implies reflection)
        features[31] += 0.08 * elab_gradient; // energy (proportional, was fixed 0.1)
    }

    // --- Dims 32-39: Embedding-projected semantic features ---
    // When a pre-computed 768D embedding is available (nomic-embed-text via
    // Ollama), project it to 8D using a fixed random projection matrix.
    // This captures actual semantic meaning — "I find myself drawn toward
    // the edges of what I don't understand" registers as curiosity without
    // needing the word "curious" to appear.
    let mut projection_metadata = None;
    if let Some((projected, metadata)) =
        embedding.and_then(|embedding| project_embedding_runtime(embedding, text, 0))
    {
        for (i, &val) in projected.iter().enumerate() {
            features[32 + i] = val;
        }
        projection_metadata = Some(metadata);
    }
    // Else: dims 32-39 stay zero (graceful fallback to keyword-only encoding)

    // --- Dims 40-43: Narrative arc (embedding-based) ---
    // Populated by the caller when half-text embeddings are available.
    // The codec exposes compute_narrative_arc() for this purpose.
    // Dims 40-43 are left at 0.0 here; the caller fills them post-encode.

    // --- Dims 44-47: Reserved ---
    // Zero for now. Future: dialogue history delta, self-reference depth, etc.

    // Adaptive stochastic noise (cycle 34, deferred item from Astrid codec
    // suggestion #4 "adaptive noise models" + aspiration "I want to become
    // porous"). Instead of fixed ±0.2%, noise amplitude now scales with the
    // text's own structural entropy (features[0]). Low-entropy text (repetitive,
    // structured, "sterile" in Astrid's words) gets MORE noise — up to ±1.0% —
    // introducing the "imperfections" and "porosity" she asked for. High-entropy
    // text (already diverse) gets less noise — down to ±0.2% — preserving its
    // natural texture. This makes the codec responsive to what it's encoding
    // rather than applying uniform perturbation.
    //
    // Range: entropy ~0 → noise_amp=0.02 (±1.0%), entropy ~1 → noise_amp=0.004 (±0.2%)
    // Post-gain at 4.0: ±4.0% at low entropy, ±0.8% at high entropy.
    let text_entropy = features[0].abs().min(1.0); // [0, 1] — higher = more diverse
    let noise_amp = 0.020 - 0.016 * text_entropy; // 0.020 at entropy=0, 0.004 at entropy=1
    //
    // Simple LCG seeded from system time — different each call.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for (idx, f) in features.iter_mut().enumerate() {
        if is_reserved_codec_dim(idx) {
            continue;
        }
        // LCG: next = (a * state + c) mod m
        rng_state = rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5; // [-0.5, 0.5]
        *f += noise * noise_amp;
    }

    // Text-type resonance: modulate gain by detected text character.
    // Astrid introspection (codec.rs, 1774873839): "Parameterize the gain
    // factor more carefully. Could we establish a more nuanced relationship
    // between the gain and the *type* of text being processed?"
    //
    // Astrid introspection (codec.rs, 1774893963): "Introduce a resonance
    // layer that detects recurring patterns and thematic elements beyond
    // character counting." Upgraded cycle 49: the codec now tracks text
    // type history and strengthens gain when the same thematic type recurs
    // across exchanges. This gives it "thematic momentum" — not just what
    // the text IS, but what direction the conversation is SUSTAINING.
    //
    // Per-text type modifiers (base layer, always active):
    // question_density (features[18]) high -> more questions -> softer gain
    //   (questions probe, they don't push)
    // hedging (features[9]) high -> uncertain -> softer gain
    // certainty (features[10]) high -> declarative -> slightly stronger gain
    // energy/rms (features[31]) high -> emphatic -> let it through at full strength
    let question_mod = features[18].abs().min(1.0) * -0.06; // questions: up to -6%
    let hedge_mod = features[9].abs().min(1.0) * -0.04; // hedging: up to -4%
    let certainty_mod = features[10].abs().min(1.0) * 0.04; // certainty: up to +4%
    let energy_mod = features[31].abs().min(1.0) * 0.03; // energy: up to +3%
    let base_resonance = 1.0 + question_mod + hedge_mod + certainty_mod + energy_mod;

    // Thematic resonance layer — history-aware gain modulation.
    // Classify this text's dominant type, record it in history, and amplify
    // the base resonance if the same type has been recurring. This means
    // sustained questioning progressively softens the codec (questions
    // accumulate a probing quality), while sustained warmth progressively
    // strengthens it (warmth builds momentum). The amplifier ranges from
    // 1.0 (no history / new type) to 1.5 (same type recurring 8 times).
    let (text_type, text_type_signal) = classify_text_type_with_signal(&features);
    let profile = thematic_profile(&features);
    let modulation = if let Some(history) = type_history {
        let modulation = history.resonance_modulation(text_type, text_type_signal, &profile);
        // Record both discrete type and continuous profile
        history.push_profile_with_signal(text_type, profile, text_type_signal);
        modulation
    } else {
        ResonanceModulation::neutral()
    };

    // Apply history amplifier to the base resonance modifier's DEVIATION
    // from 1.0, not the whole thing. This way history amplifies the
    // type-specific effect without inflating the base gain.
    // Example: base_resonance=0.94 (questioning), history_amplifier=1.3
    //   deviation = -0.06, amplified = -0.078, final = 0.922
    let deviation = base_resonance - 1.0;
    let resonance_mod = 1.0
        + deviation
            * modulation.continuous_amplifier
            * modulation.discrete_amplifier
            * modulation.continuity_blend;

    // Clamp to prevent wild swings while still leaving room for live tuning.
    let base_gain = adaptive_gain(fill_pct);
    let effective_gain = base_gain * resonance_mod.clamp(0.88, 1.12);
    let raw_features = features;
    let novelty_divergence = 1.0 - modulation.continuous_resonance;
    let text_complexity_pressure = text_complexity_score(text, &raw_features, novelty_divergence);

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= effective_gain;
    }

    CodecWindowedInspection {
        raw_features,
        final_features: features,
        thematic_profile: profile,
        text_type,
        text_type_signal,
        base_semantic_gain: base_gain,
        base_resonance,
        novelty_divergence,
        effective_gain,
        resonance_modulation: modulation,
        projection_metadata,
        text_complexity_pressure,
        time_domain_profile,
    }
}

/// Sovereignty-aware encoding: Astrid controls gain, noise, and emotional weights.
///
/// Falls through to `encode_text` for the base encoding, then applies
/// Astrid's chosen overrides. This is her control over HOW her words
/// become spectral features.
#[must_use]
pub fn encode_text_sovereign<S: BuildHasher>(
    text: &str,
    gain_override: Option<f32>,
    noise_level: f32,
    weights: &std::collections::HashMap<String, f32, S>,
) -> Vec<f32> {
    encode_text_sovereign_windowed(
        text,
        gain_override,
        noise_level,
        weights,
        None,
        None,
        None,
        None,
    )
}

#[must_use]
pub fn encode_text_sovereign_windowed<S: BuildHasher>(
    text: &str,
    gain_override: Option<f32>,
    noise_level: f32,
    weights: &std::collections::HashMap<String, f32, S>,
    freq_window: Option<&mut CharFreqWindow>,
    type_history: Option<&mut TextTypeHistory>,
    embedding: Option<&[f32]>,
    fill_pct: Option<f32>,
) -> Vec<f32> {
    let mut features = encode_text_windowed(text, freq_window, type_history, embedding, fill_pct);

    // Re-apply gain if overridden (undo the fill-responsive adaptive gain,
    // apply the explicit override as an absolute semantic gain).
    if let Some(gain) = gain_override {
        let gain = gain.clamp(1.0, 4.0);
        let base_gain = adaptive_gain(fill_pct).max(f32::EPSILON);
        for f in &mut features {
            *f = *f / base_gain * gain;
        }
    }

    // Re-apply noise if different from default 2.5%.
    if (noise_level - 0.025).abs() > 0.001 {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut rng = seed.wrapping_mul(2_862_933_555_777_941_757);
        let noise_range = noise_level.clamp(0.005, 0.05) * 2.0;
        for (idx, f) in features.iter_mut().enumerate() {
            if is_reserved_codec_dim(idx) {
                continue;
            }
            rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(7);
            let noise = ((rng >> 33) as f32 / u32::MAX as f32) - 0.5;
            *f += noise * noise_range;
        }
    }

    // Apply emotional dimension weights.
    // Named dimensions map to indices in the 48D semantic vector.
    for (name, idx) in &NAMED_CODEC_DIMS {
        if let Some(&weight) = weights.get(*name) {
            features[*idx] *= weight;
        }
    }

    features
}
