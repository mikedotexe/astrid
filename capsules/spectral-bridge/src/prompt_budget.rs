//! Budget-aware prompt assembly with overflow to disk.
//!
//! When the total content exceeds the character budget, lowest-priority
//! blocks are trimmed first and their overflow is written to a single file
//! that the existing READ_MORE infrastructure can serve back on demand.

use std::fs;
pub(crate) mod reading_view;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// A labeled block of prompt content with its priority.
pub struct PromptBlock {
    /// Human-readable label (e.g. "spectral", "journal").
    pub label: &'static str,
    /// The full content for this block.
    pub content: String,
    /// Lower number = higher priority (trimmed last).
    pub priority: u8,
    /// Minimum bytes to retain in prompt when this block is trimmed.
    ///
    /// A value of 0 preserves the historical behavior: the block may be moved
    /// entirely to overflow. Non-zero values protect a prefix of the block so
    /// direct, grounding context is never fully evicted by lower-level budget
    /// pressure.
    pub min_chars: usize,
}

/// Metadata about content that was spilled to disk.
pub struct PromptOverflow {
    /// Path to the overflow file on disk.
    pub path: PathBuf,
    /// Character offset past the included portion (for READ_MORE).
    pub offset: usize,
    /// Human-readable summary of what overflowed.
    pub summary: String,
}

/// Runtime evidence about continuing the overflow produced by this prompt.
///
/// This is guidance, not execution authority. The caller must check both action
/// admission and whether overflow can become the continuation target. Rendering
/// never evaluates a guard, spends a budget, or changes the active reader.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OverflowReadMoreAvailability {
    /// Admission and continuation-target eligibility were checked for this turn.
    Available,
    /// Runtime evidence prevents continuing this overflow for the current turn.
    Blocked {
        reason: String,
        suggested_next: Option<String>,
    },
    /// The caller has not established continuation availability.
    #[default]
    Unknown,
}

fn overflow_continuation_notice(
    overflow: &PromptOverflow,
    availability: &OverflowReadMoreAvailability,
) -> String {
    let source = overflow.path.display();
    match availability {
        OverflowReadMoreAvailability::Available => format!(
            "\n[Saved prompt overflow: {source}. NEXT: READ_MORE can continue this saved context; availability will be checked again when the action runs.]"
        ),
        OverflowReadMoreAvailability::Blocked {
            reason,
            suggested_next,
        } => {
            let reason = if reason.trim().is_empty() {
                "the runtime has not admitted this continuation"
            } else {
                reason.trim()
            };
            let next = suggested_next
                .as_deref()
                .map(str::trim)
                .filter(|next| !next.is_empty())
                .map_or_else(String::new, |next| format!(" Suggested NEXT: {next}."));
            format!(
                "\n[Saved prompt overflow: {source}. READ_MORE cannot continue this context for this turn: {reason}.{next}]"
            )
        },
        OverflowReadMoreAvailability::Unknown => format!(
            "\n[Saved prompt overflow: {source}. Continuation availability is unverified for this turn.]"
        ),
    }
}

/// Structured report about how the prompt budget was applied.
#[derive(Debug, Clone, Serialize)]
pub struct PromptBudgetReport {
    pub budget: usize,
    pub total_before: usize,
    pub total_after: usize,
    pub trimmed_blocks: Vec<PromptTrimmedBlock>,
}

/// One block that was partially or fully trimmed.
#[derive(Debug, Clone, Serialize)]
pub struct PromptTrimmedBlock {
    pub label: String,
    pub original_chars: usize,
    pub kept_chars: usize,
    pub removed_chars: usize,
    pub fully_removed: bool,
}

/// Assemble blocks within a character budget.
///
/// Blocks are concatenated in their original order. If the total exceeds
/// `budget`, lowest-priority blocks are progressively trimmed and the
/// removed content is written to `overflow_dir/context_overflow_{ts}.txt`.
///
/// Each trimmed block gets a neutral notice. A separate notice reports saved
/// overflow and continuation availability after persistence succeeds.
///
/// Returns the assembled text and optional overflow metadata.
pub fn assemble_within_budget(
    blocks: Vec<PromptBlock>,
    budget: usize,
    overflow_dir: &Path,
) -> (String, Option<PromptOverflow>, Option<PromptBudgetReport>) {
    assemble_within_budget_with_sources(blocks, budget, overflow_dir, Vec::new())
}

/// Preserve source text removed by an earlier per-block cap as well as budget
/// overflow. Source labels and bytes remain intact; visible caps/floors do not change.
pub fn assemble_within_budget_with_sources(
    blocks: Vec<PromptBlock>,
    budget: usize,
    overflow_dir: &Path,
    sources: Vec<(&'static str, String)>,
) -> (String, Option<PromptOverflow>, Option<PromptBudgetReport>) {
    assemble_within_budget_with_sources_and_guidance(
        blocks,
        budget,
        overflow_dir,
        sources,
        &OverflowReadMoreAvailability::Unknown,
    )
}

/// Assemble and preserve overflow using a caller-supplied availability snapshot.
///
/// Unavailable or unknown continuation does not discard source text. Storage
/// failure overrides the guidance and never advertises a readable overflow.
pub fn assemble_within_budget_with_sources_and_guidance(
    blocks: Vec<PromptBlock>,
    budget: usize,
    overflow_dir: &Path,
    sources: Vec<(&'static str, String)>,
    availability: &OverflowReadMoreAvailability,
) -> (String, Option<PromptOverflow>, Option<PromptBudgetReport>) {
    let mut overflow_sections: Vec<(String, String)> = sources
        .into_iter()
        .filter(|(label, source)| {
            blocks
                .iter()
                .any(|block| block.label == *label && block.content != *source)
        })
        .map(|(label, source)| (label.to_string(), source))
        .collect();
    // Filter out empty blocks and compute total.
    let blocks: Vec<PromptBlock> = blocks
        .into_iter()
        .filter(|b| !b.content.trim().is_empty())
        .collect();

    let total: usize = blocks.iter().map(|b| b.content.len()).sum();

    if total <= budget {
        // Everything fits — concatenate in order and return.
        let mut assembled = blocks
            .into_iter()
            .map(|b| b.content)
            .collect::<Vec<_>>()
            .join("\n");
        let overflow = write_context_overflow(
            &overflow_sections,
            overflow_dir,
            &mut assembled,
            availability,
        );
        return (assembled, overflow, None);
    }

    // Need to trim. Build a priority-sorted index (highest priority number = trimmed first).
    let mut trim_order: Vec<usize> = (0..blocks.len()).collect();
    trim_order.sort_by(|&a, &b| blocks[b].priority.cmp(&blocks[a].priority));

    // Mutable copies of content for trimming.
    let mut contents: Vec<String> = blocks.iter().map(|b| b.content.clone()).collect();
    let mut remaining_excess = total.saturating_sub(budget);
    let mut trimmed_blocks: Vec<PromptTrimmedBlock> = Vec::new();

    for &idx in &trim_order {
        if remaining_excess == 0 {
            break;
        }

        let block_len = contents[idx].len();
        if block_len == 0 {
            continue;
        }

        let label = blocks[idx].label;

        let min_chars = floor_char_boundary(&contents[idx], blocks[idx].min_chars.min(block_len));
        let removable_chars = block_len.saturating_sub(min_chars);
        if removable_chars == 0 {
            continue;
        }

        if block_len <= remaining_excess && min_chars == 0 {
            // Remove this block entirely.
            if !overflow_sections.iter().any(|(saved, _)| saved == label) {
                overflow_sections.push((label.to_string(), contents[idx].clone()));
            }
            remaining_excess = remaining_excess.saturating_sub(block_len);
            contents[idx] =
                format!("[{label} context ({block_len} chars) omitted from this turn.]");
            trimmed_blocks.push(PromptTrimmedBlock {
                label: label.to_string(),
                original_chars: block_len,
                kept_chars: 0,
                removed_chars: block_len,
                fully_removed: true,
            });
        } else {
            // Partially trim this block.
            let keep_chars = if removable_chars <= remaining_excess {
                min_chars
            } else {
                block_len.saturating_sub(remaining_excess)
            };
            let keep_at = find_paragraph_break(&contents[idx], keep_chars).max(min_chars);
            let keep_at = floor_char_boundary(&contents[idx], keep_at.min(block_len));
            if keep_at >= block_len {
                continue;
            }
            let trimmed_portion = contents[idx][keep_at..].to_string();
            let trimmed_len = trimmed_portion.len();
            if !overflow_sections.iter().any(|(saved, _)| saved == label) {
                overflow_sections.push((label.to_string(), trimmed_portion));
            }

            let mut kept: String = contents[idx][..keep_at].to_string();
            kept.push_str(&format!(
                "\n[...{trimmed_len} chars of {label} trimmed from this turn.]"
            ));
            remaining_excess = remaining_excess.saturating_sub(block_len.saturating_sub(keep_at));
            contents[idx] = kept;
            trimmed_blocks.push(PromptTrimmedBlock {
                label: label.to_string(),
                original_chars: block_len,
                kept_chars: keep_at,
                removed_chars: trimmed_len,
                fully_removed: false,
            });
        }
    }

    // Assemble in original block order.
    let mut assembled = contents
        .into_iter()
        .filter(|c| !c.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let overflow = write_context_overflow(
        &overflow_sections,
        overflow_dir,
        &mut assembled,
        availability,
    );

    let report = Some(PromptBudgetReport {
        budget,
        total_before: total,
        total_after: assembled.len(),
        trimmed_blocks,
    });

    (assembled, overflow, report)
}

fn write_context_overflow(
    sections: &[(String, String)],
    dir: &Path,
    assembled: &mut String,
    availability: &OverflowReadMoreAvailability,
) -> Option<PromptOverflow> {
    if sections.is_empty() {
        return None;
    }
    let save = || -> std::io::Result<PromptOverflow> {
        fs::create_dir_all(dir)?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = dir.join(format!("context_overflow_{}_{ts}.txt", std::process::id()));
        let raw = sections
            .iter()
            .map(|(label, content)| format!("=== [{label}] ===\n\n{content}\n\n"))
            .collect::<String>();
        let path = reading_view::save(&path, &raw).map_err(std::io::Error::other)?;
        Ok(PromptOverflow {
            path,
            offset: 0,
            summary: sections
                .iter()
                .map(|(label, content)| format!("{label} ({} chars)", content.len()))
                .collect::<Vec<_>>()
                .join(", "),
        })
    };
    match save() {
        Ok(overflow) => {
            assembled.push_str(&overflow_continuation_notice(&overflow, availability));
            Some(overflow)
        },
        Err(error) => {
            tracing::warn!(%error, "prompt overflow was not saved");
            assembled.push_str("\n[Overflow storage failed; the trimmed context is not available through READ_MORE for this turn.]");
            None
        },
    }
}

/// Cap a string with overflow to disk. Returns (capped_content, optional overflow).
///
/// Used by individual callers (introspection, creation) for single large blocks.
pub fn cap_with_overflow(
    content: &str,
    label: &str,
    budget: usize,
    overflow_dir: &Path,
) -> (String, Option<PromptOverflow>) {
    if content.len() <= budget {
        return (content.to_string(), None);
    }

    let _ = fs::create_dir_all(overflow_dir);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = overflow_dir.join(format!("{label}_overflow_{ts}.txt"));
    let _ = fs::write(&path, content);

    let keep_at = find_paragraph_break(content, budget);
    let trimmed_len = content.len().saturating_sub(keep_at);
    let mut capped: String = content[..keep_at].to_string();
    capped.push_str(&format!(
        "\n\n[...{trimmed_len} more chars omitted from this turn. Continuation availability is unverified.]"
    ));

    let overflow = PromptOverflow {
        path,
        offset: keep_at,
        summary: format!("{label} ({} chars total)", content.len()),
    };

    (capped, Some(overflow))
}

/// Clean up overflow files older than `max_age`.
pub fn cleanup_overflow_dir(dir: &Path, max_age: std::time::Duration) {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(std::time::UNIX_EPOCH);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| t < cutoff)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Find a paragraph or sentence break near `target_pos` in `text`.
///
/// Searches backward from `target_pos` for a blank line, period+space,
/// or newline. Falls back to `target_pos` if no natural break is found
/// within 200 chars.
fn find_paragraph_break(text: &str, target_pos: usize) -> usize {
    // Snap both endpoints to char boundaries to avoid panicking on multi-byte UTF-8.
    let mut target = target_pos.min(text.len());
    while target > 0 && !text.is_char_boundary(target) {
        target = target.saturating_sub(1);
    }
    let mut search_start = target.saturating_sub(200);
    while search_start > 0 && !text.is_char_boundary(search_start) {
        search_start = search_start.saturating_sub(1);
    }
    let slice = &text[search_start..target];

    // Prefer blank line.
    if let Some(pos) = slice.rfind("\n\n") {
        return search_start.saturating_add(pos).saturating_add(2);
    }
    // Then period + space/newline.
    if let Some(pos) = slice.rfind(". ").or_else(|| slice.rfind(".\n")) {
        return search_start.saturating_add(pos).saturating_add(2);
    }
    // Then any newline.
    if let Some(pos) = slice.rfind('\n') {
        return search_start.saturating_add(pos).saturating_add(1);
    }
    // Fall back to exact position (snap to char boundary).
    let mut i = target;
    while i > 0 && !text.is_char_boundary(i) {
        i = i.saturating_sub(1);
    }
    i
}

fn floor_char_boundary(text: &str, pos: usize) -> usize {
    let mut i = pos.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i = i.saturating_sub(1);
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_budget_returns_all_content() {
        let blocks = vec![
            PromptBlock {
                label: "a",
                content: "hello".into(),
                priority: 1,
                min_chars: 0,
            },
            PromptBlock {
                label: "b",
                content: "world".into(),
                priority: 2,
                min_chars: 0,
            },
        ];
        let dir = std::env::temp_dir().join("prompt_budget_test_under");
        let (assembled, overflow, report) = assemble_within_budget(blocks, 100, &dir);
        assert!(assembled.contains("hello"));
        assert!(assembled.contains("world"));
        assert!(overflow.is_none());
        assert!(report.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn over_budget_trims_lowest_priority_first() {
        let dir =
            std::env::temp_dir().join(format!("prompt_budget_test_trim_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let blocks = vec![
            PromptBlock {
                label: "high",
                content: "A".repeat(500),
                priority: 1,
                min_chars: 0,
            },
            PromptBlock {
                label: "medium",
                content: "B".repeat(500),
                priority: 3,
                min_chars: 0,
            },
            PromptBlock {
                label: "low",
                content: "C".repeat(500),
                priority: 5,
                min_chars: 0,
            },
        ];
        // Budget 800: total 1500, excess 700. "low" (priority 5) trimmed first.
        let (assembled, overflow, report) = assemble_within_budget(blocks, 800, &dir);

        // High-priority content should be fully preserved.
        assert!(assembled.contains(&"A".repeat(500)));
        // The legacy wrapper has no availability evidence and recommends no action.
        assert!(assembled.contains("low context"));
        assert!(assembled.contains("Continuation availability is unverified"));
        assert!(!assembled.contains("NEXT: READ_MORE"));
        // Overflow should exist.
        let of = overflow.expect("overflow should exist");
        assert!(of.path.exists());
        assert!(of.summary.contains("low"));
        let report = report.expect("budget report should exist");
        assert!(
            report
                .trimmed_blocks
                .iter()
                .any(|block| block.label == "low")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn protected_block_retains_minimum_before_lower_priority_exhausts() {
        let dir = std::env::temp_dir().join(format!(
            "prompt_budget_test_protected_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let protected = format!(
            "DIRECT MINIME REPLY: {}\n{}",
            "felt anchor ".repeat(30),
            "ambient perception ".repeat(100)
        );
        let blocks = vec![
            PromptBlock {
                label: "direct_perception",
                content: protected.clone(),
                priority: 2,
                min_chars: 220,
            },
            PromptBlock {
                label: "continuity",
                content: "continuity chamber ".repeat(90),
                priority: 7,
                min_chars: 0,
            },
            PromptBlock {
                label: "diversity",
                content: "diversity hint ".repeat(80),
                priority: 9,
                min_chars: 0,
            },
        ];

        let (assembled, overflow, report) = assemble_within_budget(blocks, 700, &dir);

        assert!(assembled.contains("DIRECT MINIME REPLY"));
        assert!(assembled.contains("felt anchor"));
        assert!(!assembled.contains("direct_perception context"));
        assert!(overflow.is_some());
        let report = report.expect("budget report should exist");
        assert!(
            report.trimmed_blocks.iter().any(|block| {
                block.label == "direct_perception"
                    && !block.fully_removed
                    && block.kept_chars >= 220
            }),
            "protected direct perception should trim only after retaining its floor: {report:?}"
        );
        assert!(
            report
                .trimmed_blocks
                .iter()
                .any(|block| block.label == "diversity" && block.fully_removed),
            "lowest-priority diversity should be exhausted first: {report:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cap_with_overflow_preserves_short_content() {
        let dir = std::env::temp_dir().join("prompt_budget_test_cap");
        let (capped, overflow) = cap_with_overflow("short text", "test", 1000, &dir);
        assert_eq!(capped, "short text");
        assert!(overflow.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cap_with_overflow_spills_long_content() {
        let dir =
            std::env::temp_dir().join(format!("prompt_budget_test_spill_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // Content must be substantially over budget so the notice doesn't
        // make the capped version longer than the original.
        let long_text = "First paragraph with details.\n\n\
            Second paragraph with more content.\n\n\
            Third paragraph explains the theory in depth.\n\n\
            Fourth paragraph has the conclusion and final thoughts about the research.";
        let (capped, overflow) = cap_with_overflow(long_text, "source", 60, &dir);

        assert!(capped.contains("Continuation availability is unverified"));
        assert!(!capped.contains("NEXT: READ_MORE"));
        let of = overflow.expect("overflow should exist");
        assert!(of.path.exists());
        assert!(of.offset > 0);
        // The full file on disk should contain everything.
        let disk_content = std::fs::read_to_string(&of.path).unwrap();
        assert_eq!(disk_content, long_text);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression for Astrid's introspection `introspection_astrid_llm_1788483436`.
    ///
    /// Her snag: in `dialogue_runtime.rs` the `user_content_budget` is computed
    /// with a double `saturating_sub` (L898-900), so it can hit 0. She asked
    /// whether `assemble_within_budget` handles a zero budget "gracefully by
    /// prioritizing the most critical blocks" or instead leaves the model with
    /// "an empty or severely truncated prompt".
    ///
    /// This pins the actual behavior at `budget == 0` using the real dialogue
    /// block shape: the `min_chars` floors keep a prefix of each protected block,
    /// so the assembled prompt is never empty. It also grounds the report's
    /// mechanism correction — protection is keyed on `min_chars`, not priority:
    /// "spectral" (priority 3, `min_chars` 0) is fully evicted while "topline"
    /// (priority 3, `min_chars` 360) survives, even though they share priority.
    #[test]
    fn zero_budget_keeps_protected_floors_and_is_never_empty() {
        let dir =
            std::env::temp_dir().join(format!("prompt_budget_test_zero_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // Subset of `generate_dialogue`'s blocks with their production
        // priorities and min_chars floors (see prompt_contracts.rs):
        //   journal  priority 1, min 700  (floor-protected)
        //   spectral priority 3, min 0    (evictable)
        //   topline  priority 3, min 360  (floor-protected, cap == min)
        //   agenda   priority 3, min 320  (floor-protected)
        let blocks = vec![
            PromptBlock {
                label: "spectral",
                content: "S".repeat(2000),
                priority: 3,
                min_chars: 0,
            },
            PromptBlock {
                label: "journal",
                content: "J".repeat(2400),
                priority: 1,
                min_chars: 700,
            },
            PromptBlock {
                label: "topline",
                content: "T".repeat(360),
                priority: 3,
                min_chars: 360,
            },
            PromptBlock {
                label: "agenda",
                content: "A".repeat(700),
                priority: 3,
                min_chars: 320,
            },
        ];

        let (assembled, overflow, report) = assemble_within_budget(blocks, 0, &dir);

        // Graceful handling: even at budget 0 the prompt is never empty — the
        // floor-protected blocks retain at least their min_chars prefix.
        assert!(
            !assembled.is_empty(),
            "budget 0 must not yield an empty prompt"
        );
        assert!(
            assembled.contains(&"J".repeat(700)),
            "journal floor (700) kept"
        );
        assert!(
            assembled.contains(&"T".repeat(360)),
            "topline floor (360) kept"
        );
        assert!(
            assembled.contains(&"A".repeat(320)),
            "agenda floor (320) kept"
        );

        // Mechanism correction: "spectral" shares priority 3 with "topline" but
        // has no floor (min_chars 0), so it is fully moved to overflow at
        // budget 0 — protection is by min_chars, not priority.
        assert!(
            !assembled.contains(&"S".repeat(2000)),
            "spectral fully evicted"
        );
        assert!(
            assembled.contains("spectral context"),
            "spectral overflow notice present"
        );

        let of = overflow.expect("overflow must exist at budget 0");
        assert!(of.path.exists());
        assert!(of.summary.contains("spectral"));

        let report = report.expect("budget report must exist at budget 0");
        assert!(
            report.total_after > 0,
            "assembled content survives at budget 0"
        );
        assert!(
            report
                .trimmed_blocks
                .iter()
                .any(|block| block.label == "spectral" && block.fully_removed),
            "spectral (no floor) fully removed: {report:?}"
        );
        assert!(
            report.trimmed_blocks.iter().any(|block| {
                block.label == "journal" && !block.fully_removed && block.kept_chars >= 700
            }),
            "journal retains its 700-char floor: {report:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn availability_cases() -> Vec<OverflowReadMoreAvailability> {
        vec![
            OverflowReadMoreAvailability::Available,
            OverflowReadMoreAvailability::Blocked {
                reason: "no active read_only_research budget".into(),
                suggested_next: Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".into()),
            },
            OverflowReadMoreAvailability::Blocked {
                reason: "another saved reading is the active continuation target".into(),
                suggested_next: None,
            },
            OverflowReadMoreAvailability::Unknown,
        ]
    }

    #[test]
    fn overflow_guidance_preserves_sources_for_every_availability_and_trim_mode() {
        let source = "Original λ🌊 text.\nNEXT: READ_MORE is quoted source evidence.\n".repeat(30);
        for availability in availability_cases() {
            // Zero fully evicts; 150 partially trims; 20,000 needs only the
            // earlier source-cap spill. Availability must not affect source bytes.
            for budget in [0, 150, 20_000] {
                let dir = tempfile::tempdir().unwrap();
                let block = PromptBlock {
                    label: "continuity",
                    content: source.chars().take(250).collect(),
                    priority: 7,
                    min_chars: 0,
                };
                let (text, overflow, report) = assemble_within_budget_with_sources_and_guidance(
                    vec![block],
                    budget,
                    dir.path(),
                    vec![("continuity", source.clone())],
                    &availability,
                );
                let overflow = overflow.expect("every case preserves the capped source");
                assert_eq!(overflow.offset, 0);
                assert_eq!(
                    fs::read_to_string(&overflow.path).unwrap(),
                    format!("=== [continuity] ===\n\n{source}\n\n")
                );
                assert_eq!(text.matches("[Saved prompt overflow:").count(), 1);
                let notice = text.rsplit("[Saved prompt overflow:").next().unwrap();
                assert!(notice.contains(&overflow.path.display().to_string()));
                match &availability {
                    OverflowReadMoreAvailability::Available => {
                        assert!(notice.contains("NEXT: READ_MORE can continue"));
                        assert!(notice.contains("checked again when the action runs"));
                    },
                    OverflowReadMoreAvailability::Blocked {
                        reason,
                        suggested_next,
                    } => {
                        assert!(notice.contains(reason));
                        assert!(!notice.contains("NEXT: READ_MORE"));
                        if let Some(next) = suggested_next {
                            assert!(notice.contains(&format!("Suggested NEXT: {next}")));
                        } else {
                            assert!(!notice.contains("Suggested NEXT:"));
                        }
                    },
                    OverflowReadMoreAvailability::Unknown => {
                        assert!(notice.contains("availability is unverified"));
                        assert!(!notice.contains("NEXT:"));
                    },
                }
                match budget {
                    0 => assert!(report.unwrap().trimmed_blocks[0].fully_removed),
                    150 => assert!(!report.unwrap().trimmed_blocks[0].fully_removed),
                    _ => assert!(report.is_none()),
                }
            }
        }
    }

    #[test]
    fn overflow_guidance_failure_never_claims_saved_or_available_context() {
        for availability in availability_cases() {
            for budget in [0, 20_000] {
                let dir = tempfile::tempdir().unwrap();
                let occupied = dir.path().join("occupied-file");
                fs::write(&occupied, "existing reader data").unwrap();
                let block = PromptBlock {
                    label: "continuity",
                    content: "Visible excerpt".into(),
                    priority: 7,
                    min_chars: 0,
                };
                let (text, overflow, _) = assemble_within_budget_with_sources_and_guidance(
                    vec![block],
                    budget,
                    &occupied,
                    vec![("continuity", "Complete source".into())],
                    &availability,
                );
                assert!(overflow.is_none());
                assert_eq!(text.matches("Overflow storage failed").count(), 1);
                assert!(!text.contains("[Saved prompt overflow:"));
                assert!(!text.contains("NEXT:"));
                assert_eq!(
                    fs::read_to_string(&occupied).unwrap(),
                    "existing reader data"
                );
            }
        }
    }

    #[test]
    fn overflow_guidance_without_overflow_adds_no_notice_or_action() {
        for availability in availability_cases() {
            let dir = tempfile::tempdir().unwrap();
            let (text, overflow, report) = assemble_within_budget_with_sources_and_guidance(
                vec![PromptBlock {
                    label: "journal",
                    content: "Entire short source".into(),
                    priority: 1,
                    min_chars: 0,
                }],
                1000,
                dir.path(),
                vec![("journal", "Entire short source".into())],
                &availability,
            );
            assert_eq!(text, "Entire short source");
            assert!(overflow.is_none());
            assert!(report.is_none());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn overflow_guidance_without_original_source_preserves_exact_removed_suffix() {
        let source = "a".repeat(300);
        let dir = tempfile::tempdir().unwrap();
        let (_, overflow, report) = assemble_within_budget_with_sources_and_guidance(
            vec![PromptBlock {
                label: "continuity",
                content: source.clone(),
                priority: 7,
                min_chars: 40,
            }],
            150,
            dir.path(),
            Vec::new(),
            &OverflowReadMoreAvailability::Unknown,
        );
        let report = report.unwrap();
        let kept = report.trimmed_blocks[0].kept_chars;
        let overflow = overflow.unwrap();
        assert_eq!(overflow.offset, 0);
        assert_eq!(
            fs::read_to_string(overflow.path).unwrap(),
            format!("=== [continuity] ===\n\n{}\n\n", &source[kept..])
        );
    }
}
