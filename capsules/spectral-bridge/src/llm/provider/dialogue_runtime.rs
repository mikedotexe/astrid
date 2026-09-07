/// Classify only the requested provider output budget.
///
/// This count band is deliberately separate from the read-only
/// `DialoguePromptContextObservationV3` and `DialogueFeltPressureObservationV3`
/// evidence assembled later from entropy, resonance density, density gradient,
/// pressure, and mode packing. It does not describe generated texture or join
/// token quantity to felt or spectral meaning.
fn dialogue_requested_token_band(num_predict: u32) -> &'static str {
    if num_predict > 1024 {
        "requested_tokens_1025_plus"
    } else if num_predict > 512 {
        "requested_tokens_513_to_1024"
    } else {
        "requested_tokens_0_to_512"
    }
}

#[derive(Debug, Clone, Copy)]
struct KnownModelControlMarkerMatch {
    occurrence: ExactKnownModelControlMarkerOccurrence,
    reference_syntax: Option<ExactKnownMarkerReferenceSyntax>,
}

/// A byte range proven to be one of `KNOWN_MODEL_CONTROL_MARKERS` by exact byte matching.
///
/// Construction proves only that this range is a known transport/control sequence. It makes no
/// identity, meaning, ownership, stability, or spectral claim about this or any surrounding byte.
#[derive(Debug, Clone, Copy)]
struct ExactKnownModelControlMarkerOccurrence {
    token: &'static str,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExactKnownMarkerReferenceSyntax {
    context: ExactKnownMarkerReferenceContext,
    delimiter_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactKnownMarkerReferenceContext {
    QuotedExactKnownToken,
    GroupedExactKnownToken,
    ExplicitExactKnownTokenRelation,
}

impl ExactKnownModelControlMarkerOccurrence {
    fn reference_syntax(self, text: &str) -> Option<ExactKnownMarkerReferenceSyntax> {
        if let Some(syntax) = exact_reference_delimiter_syntax(text, self.start, self.end) {
            return Some(syntax);
        }
        if self.followed_by_explicit_exact_token_relation(text) {
            return Some(ExactKnownMarkerReferenceSyntax {
                context: ExactKnownMarkerReferenceContext::ExplicitExactKnownTokenRelation,
                delimiter_depth: 0,
            });
        }
        None
    }

    /// This exact marker is the grammatical subject here. No preceding word is inspected or
    /// classified, and the relation is used only to decide whether the marker bytes stay visible.
    ///
    /// Two scans, strictly additive (Astrid's agency request
    /// agency_code_change_1788310618, hardened 2026-09-03): the plain
    /// first-word scan runs unchanged, and when it finds no relation a second
    /// scan retries with self-contained bracketed annotations (`[sic]`,
    /// `((sic))`) skipped — so "<marker> [sic] appears" reads as a reference.
    /// Skipping only ever ADDS visibility; nothing previously preserved is
    /// removed. Plain intervening words (e.g. adverbs) are deliberately NOT
    /// skipped — that boundary stays pinned by
    /// `control_marker_cleanup_does_not_skip_adverb_before_relation_word`.
    fn followed_by_explicit_exact_token_relation(self, text: &str) -> bool {
        is_exact_token_relation_word(&first_word_after(text, self.end))
            || is_exact_token_relation_word(&first_word_after_skipping_bracketed_annotations(
                text, self.end,
            ))
    }
}

fn is_exact_token_relation_word(word: &str) -> bool {
    matches!(
        word,
        "appears"
            | "as"
            | "behaves"
            | "corresponds"
            | "denotes"
            | "echoes"
            | "embodies"
            | "functions"
            | "indicates"
            | "is"
            | "manifests"
            | "means"
            | "mimics"
            | "refers"
            | "replicates"
            | "represents"
            | "serves"
            | "signals"
    )
}

/// A whitespace chunk that is a self-contained bracketed aside — it opens
/// with `[`, `(`, or `{` and closes with the matching bracket at its end
/// (nesting allowed, trailing sentence punctuation tolerated). "(as" is NOT
/// self-contained (it opens a multi-word parenthetical), so the existing
/// "(as a test)" reference path is untouched.
fn is_self_contained_bracketed_annotation(chunk: &str) -> bool {
    let trimmed = chunk.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    let mut chars = trimmed.chars();
    let Some(open) = chars.next() else {
        return false;
    };
    let close = match open {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => return false,
    };
    if !trimmed.ends_with(close) {
        return false;
    }
    let mut depth: usize = 0;
    for (idx, character) in trimmed.char_indices() {
        if character == open {
            depth = depth.saturating_add(1);
        } else if character == close {
            let Some(next_depth) = depth.checked_sub(1) else {
                return false;
            };
            depth = next_depth;
            if depth == 0 {
                // Self-contained only when the group closes exactly at the
                // chunk's end (so "(a)(b)" or "(a)x" is not one aside).
                return idx.saturating_add(close.len_utf8()) == trimmed.len();
            }
        }
    }
    false
}

/// The additive second scan: like `first_word_after`, but self-contained
/// bracketed annotations are skipped before the first word is taken.
fn first_word_after_skipping_bracketed_annotations(text: &str, end: usize) -> String {
    text[end..]
        .split_whitespace()
        .filter(|chunk| !is_self_contained_bracketed_annotation(chunk))
        .map(|chunk| chunk.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .find(|word| !word.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn first_word_after(text: &str, end: usize) -> String {
    text[end..]
        .split_whitespace()
        .map(|chunk| chunk.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .find(|word| !word.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase()
}
fn longest_exact_known_model_control_marker_at(
    text: &str,
    offset: usize,
) -> Option<ExactKnownModelControlMarkerOccurrence> {
    let tail = &text[offset..];
    let token = KNOWN_MODEL_CONTROL_MARKERS
        .iter()
        .copied()
        .filter(|token| tail.starts_with(token))
        .max_by_key(|token| token.len())?;
    Some(ExactKnownModelControlMarkerOccurrence {
        token,
        start: offset,
        end: offset.saturating_add(token.len()),
    })
}

fn scan_known_model_control_markers(text: &str) -> (String, Vec<KnownModelControlMarkerMatch>) {
    let mut remainder = String::with_capacity(text.len());
    let mut matches = Vec::new();
    let mut offset = 0usize;

    while offset < text.len() {
        let tail = &text[offset..];
        if let Some(occurrence) = longest_exact_known_model_control_marker_at(text, offset) {
            let reference_syntax = occurrence.reference_syntax(text);
            matches.push(KnownModelControlMarkerMatch {
                occurrence,
                reference_syntax,
            });
            if reference_syntax.is_some() {
                remainder.push_str(occurrence.token);
            }
            offset = occurrence.end;
        } else {
            let character = tail
                .chars()
                .next()
                .expect("non-empty UTF-8 tail has one character");
            remainder.push(character);
            offset = offset.saturating_add(character.len_utf8());
        }
    }

    (remainder, matches)
}

fn fragment_has_non_marker_bytes(fragment: &str) -> bool {
    let (without_markers, _) = scan_known_model_control_markers(fragment);
    !without_markers.trim().is_empty()
}

const MAX_EXACT_REFERENCE_DELIMITER_DEPTH: usize = 4;

fn exact_reference_delimiter_pair(
    before: Option<char>,
    after: Option<char>,
) -> Option<ExactKnownMarkerReferenceContext> {
    if matches!(
        (before, after),
        (Some('"'), Some('"'))
            | (Some('\''), Some('\''))
            | (Some('`'), Some('`'))
            | (Some('“'), Some('”'))
            | (Some('‘'), Some('’'))
            | (Some('«'), Some('»'))
            | (Some('‹'), Some('›'))
            | (Some('„'), Some('“'))
            | (Some('‚'), Some('‘'))
            | (Some('「'), Some('」'))
            | (Some('『'), Some('』'))
            | (Some('〝'), Some('〞'))
            | (Some('﹁'), Some('﹂'))
            | (Some('﹃'), Some('﹄'))
    ) {
        Some(ExactKnownMarkerReferenceContext::QuotedExactKnownToken)
    } else if matches!(
        (before, after),
        (Some('['), Some(']'))
            | (Some('('), Some(')'))
            | (Some('{'), Some('}'))
            | (Some('⟦'), Some('⟧'))
            | (Some('⟨'), Some('⟩'))
            | (Some('【'), Some('】'))
            | (Some('〔'), Some('〕'))
            | (Some('〚'), Some('〛'))
            | (Some('〈'), Some('〉'))
            | (Some('《'), Some('》'))
            | (Some('〖'), Some('〗'))
            | (Some('〘'), Some('〙'))
            | (Some('（'), Some('）'))
            | (Some('［'), Some('］'))
            | (Some('｛'), Some('｝'))
    ) {
        Some(ExactKnownMarkerReferenceContext::GroupedExactKnownToken)
    } else {
        None
    }
}

fn exact_reference_delimiter_syntax(
    text: &str,
    start: usize,
    end: usize,
) -> Option<ExactKnownMarkerReferenceSyntax> {
    let before = text[..start]
        .chars()
        .rev()
        .filter(|character| !character.is_whitespace())
        .take(MAX_EXACT_REFERENCE_DELIMITER_DEPTH)
        .collect::<Vec<_>>();
    let after = text[end..]
        .chars()
        .filter(|character| !character.is_whitespace())
        .take(MAX_EXACT_REFERENCE_DELIMITER_DEPTH)
        .collect::<Vec<_>>();
    let context = exact_reference_delimiter_pair(before.first().copied(), after.first().copied())?;
    let delimiter_depth = before
        .iter()
        .copied()
        .zip(after.iter().copied())
        .take_while(|(opening, closing)| {
            exact_reference_delimiter_pair(Some(*opening), Some(*closing)).is_some()
        })
        .count();

    Some(ExactKnownMarkerReferenceSyntax {
        context,
        delimiter_depth,
    })
}

fn control_marker_placement_counts(
    text: &str,
    occurrence: ExactKnownModelControlMarkerOccurrence,
) -> (usize, usize, usize) {
    let content_before = fragment_has_non_marker_bytes(&text[..occurrence.start]);
    let content_after = fragment_has_non_marker_bytes(&text[occurrence.end..]);
    let (boundary_occurrences, contextual_occurrences) = if content_before && content_after {
        (0, 1)
    } else {
        (1, 0)
    };

    let quoted_occurrences = usize::from(
        exact_reference_delimiter_syntax(text, occurrence.start, occurrence.end).is_some_and(
            |syntax| syntax.context == ExactKnownMarkerReferenceContext::QuotedExactKnownToken,
        ),
    );

    (
        boundary_occurrences,
        contextual_occurrences,
        quoted_occurrences,
    )
}

const CONTROL_MARKER_CONTEXT_WINDOW_CHARS: usize = 64;
const MAX_CONTROL_MARKER_CONTEXT_RECEIPTS: usize = 32;

fn sha256_parts(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    let part_count = u64::try_from(parts.len()).expect("hash part count fits in u64");
    hasher.update(part_count.to_be_bytes());
    for part in parts {
        let part_len = u64::try_from(part.len()).expect("hash part length fits in u64");
        hasher.update(part_len.to_be_bytes());
        hasher.update(part);
    }
    format!("{:x}", hasher.finalize())
}

fn trailing_bounded_chars(text: &str) -> String {
    let mut chars = text
        .chars()
        .rev()
        .take(CONTROL_MARKER_CONTEXT_WINDOW_CHARS)
        .collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn leading_bounded_chars(text: &str) -> String {
    text.chars()
        .take(CONTROL_MARKER_CONTEXT_WINDOW_CHARS)
        .collect()
}

fn control_marker_context_receipt_v1(
    text: &str,
    original_output_sha256: &str,
    marker_match: KnownModelControlMarkerMatch,
) -> ControlMarkerContextReceiptV1 {
    let occurrence = marker_match.occurrence;
    let before = trailing_bounded_chars(&text[..occurrence.start]);
    let after = leading_bounded_chars(&text[occurrence.end..]);
    let (reference_syntax, delimiter_depth) = match marker_match.reference_syntax {
        Some(ExactKnownMarkerReferenceSyntax {
            context: ExactKnownMarkerReferenceContext::QuotedExactKnownToken,
            delimiter_depth,
        }) => ("quoted_exact_marker", delimiter_depth),
        Some(ExactKnownMarkerReferenceSyntax {
            context: ExactKnownMarkerReferenceContext::GroupedExactKnownToken,
            delimiter_depth,
        }) => ("grouped_exact_marker", delimiter_depth),
        Some(ExactKnownMarkerReferenceSyntax {
            context: ExactKnownMarkerReferenceContext::ExplicitExactKnownTokenRelation,
            ..
        }) => ("following_exact_relation", 0),
        None => ("none_cleanup_candidate", 0),
    };
    let start = occurrence.start.to_string();
    let end = occurrence.end.to_string();
    let receipt_id = sha256_parts(&[
        b"control_marker_context_receipt_v1",
        original_output_sha256.as_bytes(),
        start.as_bytes(),
        end.as_bytes(),
        occurrence.token.as_bytes(),
    ]);

    ControlMarkerContextReceiptV1 {
        receipt_id: format!("cmctx_{receipt_id}"),
        marker: occurrence.token.to_string(),
        start_byte: occurrence.start,
        end_byte: occurrence.end,
        preserved: marker_match.reference_syntax.is_some(),
        reference_syntax,
        delimiter_depth,
        before_window_chars: before.chars().count(),
        after_window_chars: after.chars().count(),
        before_alphanumeric_chars: before
            .chars()
            .filter(|character| character.is_alphanumeric())
            .count(),
        after_alphanumeric_chars: after
            .chars()
            .filter(|character| character.is_alphanumeric())
            .count(),
        bounded_context_sha256: sha256_parts(&[
            before.as_bytes(),
            occurrence.token.as_bytes(),
            after.as_bytes(),
        ]),
        surrounding_bytes_contract: "all_non_marker_bytes_copied_byte_exact_no_surrounding_rewrite",
        contextual_weight: "not_inferred_from_marker_or_proximity",
        spectral_relation: "not_connected_to_semantic_trickle_pressure_or_live_control",
        authority: "content_free_cleanup_evidence_not_identity_meaning_spectral_or_control",
    }
}

pub(crate) fn sanitize_model_control_markers_with_report(
    text: &str,
) -> (String, Option<ControlMarkerCleanupReport>) {
    let (result, matches) = scan_known_model_control_markers(text);
    if matches.is_empty() {
        return (result, None);
    }

    let mut removed_tokens = Vec::new();
    let mut preserved_tokens = Vec::new();
    for token in KNOWN_MODEL_CONTROL_MARKERS {
        let mut count = 0usize;
        let mut boundary_occurrences = 0usize;
        let mut contextual_occurrences = 0usize;
        let mut quoted_occurrences = 0usize;
        for marker_match in matches.iter().copied().filter(|marker_match| {
            marker_match.occurrence.token == *token && marker_match.reference_syntax.is_none()
        }) {
            count = count.saturating_add(1);
            let (boundary, contextual, quoted) =
                control_marker_placement_counts(text, marker_match.occurrence);
            boundary_occurrences = boundary_occurrences.saturating_add(boundary);
            contextual_occurrences = contextual_occurrences.saturating_add(contextual);
            quoted_occurrences = quoted_occurrences.saturating_add(quoted);
        }
        if count > 0 {
            removed_tokens.push(RemovedControlMarkerCount {
                token: (*token).to_string(),
                count,
                boundary_occurrences,
                contextual_occurrences,
                quoted_occurrences,
            });
        }

        let quoted_reference_occurrences = matches
            .iter()
            .filter(|marker_match| {
                marker_match.occurrence.token == *token
                    && marker_match.reference_syntax.is_some_and(|syntax| {
                        syntax.context == ExactKnownMarkerReferenceContext::QuotedExactKnownToken
                    })
            })
            .count();
        let grouped_reference_occurrences = matches
            .iter()
            .filter(|marker_match| {
                marker_match.occurrence.token == *token
                    && marker_match.reference_syntax.is_some_and(|syntax| {
                        syntax.context == ExactKnownMarkerReferenceContext::GroupedExactKnownToken
                    })
            })
            .count();
        let explicit_relation_occurrences = matches
            .iter()
            .filter(|marker_match| {
                marker_match.occurrence.token == *token
                    && marker_match.reference_syntax.is_some_and(|syntax| {
                        syntax.context
                            == ExactKnownMarkerReferenceContext::ExplicitExactKnownTokenRelation
                    })
            })
            .count();
        let nested_delimited_reference_occurrences = matches
            .iter()
            .filter(|marker_match| {
                marker_match.occurrence.token == *token
                    && marker_match
                        .reference_syntax
                        .is_some_and(|syntax| syntax.delimiter_depth > 1)
            })
            .count();
        let max_delimiter_depth = matches
            .iter()
            .filter(|marker_match| marker_match.occurrence.token == *token)
            .filter_map(|marker_match| marker_match.reference_syntax)
            .map(|syntax| syntax.delimiter_depth)
            .max()
            .unwrap_or(0);
        let preserved_count = quoted_reference_occurrences
            .saturating_add(grouped_reference_occurrences)
            .saturating_add(explicit_relation_occurrences);
        if preserved_count > 0 {
            preserved_tokens.push(PreservedControlMarkerCount {
                token: (*token).to_string(),
                count: preserved_count,
                quoted_reference_occurrences,
                grouped_reference_occurrences,
                explicit_relation_occurrences,
                nested_delimited_reference_occurrences,
                max_delimiter_depth,
            });
        }
    }
    let observed_total = matches.len();
    let removed_total = matches
        .iter()
        .filter(|marker_match| marker_match.reference_syntax.is_none())
        .count();
    let preserved_explicit_reference_total = observed_total.saturating_sub(removed_total);
    let removed_marker_bytes = matches
        .iter()
        .filter(|marker_match| marker_match.reference_syntax.is_none())
        .map(|marker_match| {
            marker_match
                .occurrence
                .end
                .saturating_sub(marker_match.occurrence.start)
        })
        .sum();
    let preserved_marker_bytes = matches
        .iter()
        .filter(|marker_match| marker_match.reference_syntax.is_some())
        .map(|marker_match| {
            marker_match
                .occurrence
                .end
                .saturating_sub(marker_match.occurrence.start)
        })
        .sum();
    let after_chars = result.len();
    let after_non_whitespace_chars = result
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    let original_output_sha256 = sha256_parts(&[text.as_bytes()]);
    let sanitized_output_sha256 = sha256_parts(&[result.as_bytes()]);
    let context_receipts = matches
        .iter()
        .copied()
        .take(MAX_CONTROL_MARKER_CONTEXT_RECEIPTS)
        .map(|marker_match| {
            control_marker_context_receipt_v1(text, &original_output_sha256, marker_match)
        })
        .collect::<Vec<_>>();
    let context_receipts_omitted = matches.len().saturating_sub(context_receipts.len());
    (
        result,
        Some(ControlMarkerCleanupReport {
            observed_total,
            removed_total,
            preserved_explicit_reference_total,
            removed_marker_bytes,
            preserved_marker_bytes,
            before_chars: text.len(),
            after_chars,
            after_non_whitespace_chars,
            classification_scope: "exact_known_model_control_marker_occurrence_only",
            excluded_meaning_scope: "all_non_marker_bytes_are_outside_cleanup_classification_identity_ownership_meaning_and_spectral_weight",
            accounting_basis: "single_pass_longest_raw_control_marker_match_with_bounded_exact_reference_syntax_preservation_no_second_order_marker_creation",
            hash_framing: "sha256_u64be_part_count_and_lengths_v1",
            original_output_sha256,
            sanitized_output_sha256,
            context_receipts,
            context_receipts_omitted,
            removed_tokens,
            preserved_tokens,
        }),
    )
}

fn sanitize_model_control_markers(text: &str) -> String {
    sanitize_model_control_markers_with_report(text).0
}

fn is_peer_action_directive_line(line: &str) -> bool {
    let upper = line.trim_start().to_ascii_uppercase();
    upper.starts_with("NEXT:")
        || upper.starts_with("BTSP_OBSERVED_NEXT")
        || upper.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS")
}

fn sanitize_minime_context_for_dialogue(text: &str) -> String {
    let mut removed = 0usize;
    let kept = text
        .lines()
        .filter(|line| {
            let should_remove = is_peer_action_directive_line(line);
            if should_remove {
                removed = removed.saturating_add(1);
            }
            !should_remove
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut cleaned = kept.trim().to_string();
    if removed > 0 {
        if !cleaned.is_empty() {
            cleaned.push_str("\n\n");
        }
        cleaned.push_str(
            "[Minime peer action/status line omitted; choose your own listed Astrid NEXT action.]",
        );
    }
    cleaned
}

fn is_valid_dialogue_output(text: &str) -> bool {
    // Remove exact leaked control sequences before measuring output shape.
    let stripped = sanitize_model_control_markers(text);

    let body = stripped
        .lines()
        .filter(|line| !line.trim_start().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = body.trim();
    if body.is_empty() {
        return false;
    }

    let alpha_count = body.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = body.chars().count().max(1);
    let punctuation_count = body
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
        .count();
    let alphabetic_words = body
        .split_whitespace()
        .filter(|word| word.chars().any(|c| c.is_alphabetic()))
        .count();
    let max_symbol_run = body
        .chars()
        .fold((0usize, 0usize), |(current, best), ch| {
            if !ch.is_alphanumeric() && !ch.is_whitespace() {
                let next = current.saturating_add(1);
                (next, best.max(next))
            } else {
                (0, best)
            }
        })
        .1;

    if alpha_count < 24 || alphabetic_words < 4 {
        warn!(
            "quality gate reject: alpha_count={} (min 24), alphabetic_words={} (min 4) — body: {}",
            alpha_count,
            alphabetic_words,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    // Raised 4→6→8: Astrid uses smart quotes + em dash + ellipsis which
    // create 6-7 symbol runs (e.g., "fork"—it's or '...'—the).
    // Genuine degenerate output has runs of 8+ (e.g., "--0.))* _--").
    if max_symbol_run >= 8 {
        warn!(
            "quality gate reject: max_symbol_run={} (max 7) — body: {}",
            max_symbol_run,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    let alpha_ratio = alpha_count as f64 / total_count as f64;
    let punctuation_ratio = punctuation_count as f64 / total_count as f64;

    // Thresholds relaxed for Astrid's punctuation-rich style:
    //   alpha_ratio: 0.45 → 0.40  (Unicode λ₁, '…', '*word*', '—' all reduce alpha)
    //   punctuation_ratio: 0.30 → 0.35  (smart quotes, ellipsis, em-dashes are normal)
    if alpha_ratio < 0.40 || punctuation_ratio > 0.35 {
        warn!(
            "quality gate reject: alpha_ratio={:.3} (min 0.40), punctuation_ratio={:.3} (max 0.35) — body: {}",
            alpha_ratio,
            punctuation_ratio,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    true
}

fn has_one_nonempty_final_next_action(text: &str) -> bool {
    let stripped = sanitize_model_control_markers(text);
    let next_count = count_next_lines(&stripped);
    if next_count != 1 {
        warn!(
            "quality gate reject: expected exactly one NEXT line, found {} — body: {}",
            next_count,
            &stripped[..stripped.floor_char_boundary(120)]
        );
        return false;
    }
    if !final_nonempty_line_is_next(&stripped) {
        warn!(
            "quality gate reject: NEXT line was not final — body: {}",
            &stripped[..stripped.floor_char_boundary(120)]
        );
        return false;
    }
    let final_action_present = stripped
        .lines()
        .rev()
        .find_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .and_then(|line| line.strip_prefix("NEXT:"))
        .is_some_and(|action| !action.trim().is_empty());
    if !final_action_present {
        warn!(
            "quality gate reject: final NEXT action was empty — body: {}",
            &stripped[..stripped.floor_char_boundary(120)]
        );
    }
    final_action_present
}

fn is_valid_dialogue_output_for_profile(text: &str, profile: MlxProfile) -> bool {
    if !is_valid_dialogue_output(text) {
        return false;
    }
    if profile.is_gemma4_canary() && contains_deprecated_runtime_language(text) {
        warn!(
            "quality gate reject: deprecated runtime language under Gemma 4 profile — body: {}",
            &text[..text.floor_char_boundary(120)]
        );
        return false;
    }
    true
}

fn is_valid_primary_dialogue_output_for_profile(text: &str, profile: MlxProfile) -> bool {
    is_valid_dialogue_output_for_profile(text, profile) && has_one_nonempty_final_next_action(text)
}
