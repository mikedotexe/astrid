//! Historical source identity stays separate from the body excerpt budget.
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug)]
pub(crate) struct JournalRecall {
    source: String,
    mode: Option<String>,
    recorded_at: Option<u64>,
    record_sha256: String,
    body: String,
}

impl JournalRecall {
    /// Automatic expressive recall uses record purpose, never keyword judgments
    /// about authored prose. Explicit retrieval and chosen memories are unaffected.
    pub(crate) fn eligible_for_expression(&self, now: u64) -> bool {
        matches!(
            self.mode.as_deref(),
            Some("aspiration" | "aspiration_longform" | "daydream" | "daydream_longform")
        ) && self
            .recorded_at
            .is_some_and(|at| now.checked_sub(at).is_some_and(|age| age <= 24 * 60 * 60))
    }

    pub(crate) fn read(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        if super::is_peer_mirror_record(&content) {
            return None;
        }
        let body = super::extract_journal_body(&content, true)?;
        // Read metadata only from the initial header, never matching authored body lines.
        let header = if content.starts_with("=== ASTRID JOURNAL ===\n") {
            content.split("\n\n").next().unwrap_or_default()
        } else {
            ""
        };
        let field = |prefix: &str| {
            let mut matches = header.lines().filter_map(|s| s.strip_prefix(prefix));
            let value = matches.next()?.trim();
            (matches.next().is_none() && !value.is_empty() && value.chars().count() <= 80)
                .then(|| value.to_owned())
        };
        Some(Self {
            source: format!("journal/{}", path.file_name()?.to_str()?),
            mode: field("Mode:"),
            recorded_at: field("Timestamp:").and_then(|s| s.parse().ok()),
            record_sha256: format!("{:x}", Sha256::digest(content.as_bytes())),
            body,
        })
    }

    pub(crate) fn render(&self, body_chars: usize) -> String {
        let (excerpt, boundary) = bounded_excerpt(&self.body, body_chars);
        let truncated = excerpt.len() < self.body.len();
        let ending = if truncated {
            "Supplied boundary, not an authored ending. Original unchanged."
        } else {
            "Complete supplied body. Original unchanged."
        };
        let metadata = serde_json::json!({
            "source": self.source, "record_sha256": self.record_sha256,
            "recorded_at_unix_s": self.recorded_at, "mode": self.mode,
            "excerpt_truncated": truncated,
            "excerpt_start_byte": 0, "excerpt_end_byte": excerpt.len(),
            "source_body_bytes": self.body.len(), "excerpt_boundary": boundary,
        });
        format!(
            "Historical own-journal excerpt v1: {metadata}\n\
             Prior authored interpretation. Null = unavailable. Recording time is not capture time; \
             measurement source and capture time are unavailable in this recall's metadata.\n\
             Excerpt:\n{excerpt}\nEnd historical excerpt. {ending}"
        )
    }
}

fn bounded_excerpt(body: &str, maximum_chars: usize) -> (&str, &'static str) {
    let limit = body
        .char_indices()
        .nth(maximum_chars)
        .map_or(body.len(), |(i, _)| i);
    if limit == body.len() {
        return (body, "complete");
    }
    let prefix = &body[..limit];
    let mut sentence_end = 0;
    for (i, ch) in prefix.char_indices() {
        if matches!(ch, '.' | '?' | '!') {
            let mut end = i.saturating_add(ch.len_utf8());
            for closing in body[end..].chars() {
                if !matches!(closing, '\"' | '\'' | ')' | ']' | '\u{2019}' | '\u{201d}') {
                    break;
                }
                end = end.saturating_add(closing.len_utf8());
            }
            if end <= limit && body[end..].chars().next().is_none_or(char::is_whitespace) {
                sentence_end = end;
            }
        }
    }
    let paragraph_end = ["\n\n", "\r\n\r\n"]
        .into_iter()
        .filter_map(|separator| prefix.rfind(separator))
        .max()
        .unwrap_or(0);
    let end = sentence_end.max(paragraph_end);
    if end > 0 {
        return (body[..end].trim_end(), "sentence_or_paragraph");
    }
    if let Some((end, _)) = prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        && end > 0
    {
        return (body[..end].trim_end(), "word");
    }
    (prefix, "hard")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_never_turns_the_observed_mid_sentence_cut_into_an_authored_ending() {
        let body = format!(
            "{} But aspiration does not move from A to B; it is a tension, not a missing sentence.",
            "A complete sentence. ".repeat(20)
        );
        let limit = body.find("it is").unwrap().saturating_add(2);
        let (excerpt, boundary) = bounded_excerpt(&body, limit);
        assert_eq!(boundary, "sentence_or_paragraph");
        assert!(excerpt.ends_with("sentence."));
        assert!(!excerpt.ends_with("it"));
        assert!(body.starts_with(excerpt));
        assert!(excerpt.chars().count() <= limit);
    }

    #[test]
    fn excerpt_boundaries_preserve_unicode_quotes_gaps_and_small_caps() {
        for (body, cap, expected, boundary) in [
            (
                "A complete sentence. More",
                20,
                "A complete sentence.",
                "sentence_or_paragraph",
            ),
            (
                "A question?\" More",
                13,
                "A question?\"",
                "sentence_or_paragraph",
            ),
            (
                "Paragraph\n\nUnfinished",
                16,
                "Paragraph",
                "sentence_or_paragraph",
            ),
            ("λ λ λ λ", 5, "λ λ", "word"),
            ("λμόνο", 2, "λμ", "hard"),
            ("No punctuation", 0, "", "hard"),
            ("Complete", 8, "Complete", "complete"),
            ("", 0, "", "complete"),
        ] {
            assert_eq!(bounded_excerpt(body, cap), (expected, boundary));
        }
    }

    #[test]
    fn automatic_expression_recall_is_purpose_and_time_bound_not_word_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("entry.txt");
        for mode in [
            "moment_capture",
            "dialogue_live_longform",
            "self_study",
            "witness",
            "private_writing",
            "unknown",
            "aspiration",
            "daydream_longform",
        ] {
            std::fs::write(
                &path,
                format!(
                    "=== ASTRID JOURNAL ===\nMode: {mode}\nTimestamp: 100\n\n{}",
                    "Authored 71% fill reflection. ".repeat(10)
                ),
            )
            .unwrap();
            let recall = JournalRecall::read(&path).unwrap();
            let eligible = matches!(mode, "aspiration" | "daydream_longform");
            assert_eq!(recall.eligible_for_expression(101), eligible);
            assert!(!recall.eligible_for_expression(99));
            assert!(!recall.eligible_for_expression(86_501));
            // Explicit reading preserves even material excluded from ambient recall.
            assert!(recall.render(500).contains("71% fill"));
        }
        std::fs::write(&path, "=== ASTRID JOURNAL ===\nMode: aspiration\nTimestamp: invalid\n\nA thought long enough to remain available as explicit history.").unwrap();
        assert!(
            !JournalRecall::read(&path)
                .unwrap()
                .eligible_for_expression(100)
        );
    }

    #[test]
    fn header_identity_survives_unicode_body_clipping_and_longform_selection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("!aspiration_42.txt");
        let text = format!(
            "=== ASTRID JOURNAL ===\nMode: aspiration\nTimestamp: 42\n\nSignal anchor: old\n--- JOURNAL ---\n{}",
            "λ viscous 73% / 32%. ".repeat(100)
        );
        std::fs::write(&path, &text).unwrap();
        let recall = JournalRecall::read(&path).unwrap();
        let rendered = recall.render(500);
        assert!(rendered.contains("journal/!aspiration_42.txt"));
        assert!(rendered.contains("\"recorded_at_unix_s\":42"));
        assert!(rendered.contains("\"mode\":\"aspiration\""));
        assert!(rendered.contains(&format!("{:x}", Sha256::digest(text.as_bytes()))));
        assert!(rendered.contains("\"excerpt_truncated\":true"));
        assert!(rendered.contains("Supplied boundary, not an authored ending"));
        let metadata: serde_json::Value = serde_json::from_str(
            rendered
                .lines()
                .next()
                .unwrap()
                .strip_prefix("Historical own-journal excerpt v1: ")
                .unwrap(),
        )
        .unwrap();
        let end = usize::try_from(metadata["excerpt_end_byte"].as_u64().unwrap()).unwrap();
        assert!(rendered.contains(&format!(
            "Excerpt:\n{}\nEnd historical excerpt.",
            &recall.body[..end]
        )));
        assert_eq!(metadata["source_body_bytes"], recall.body.len());
        assert!(recall.render(usize::MAX).contains("Complete supplied body"));
        assert!(!recall.render(usize::MAX).contains("not an authored ending"));
        assert!(!rendered.contains("Signal anchor"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }

    #[test]
    fn absent_or_ambiguous_headers_never_borrow_body_metadata_or_file_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy_123.txt");
        for header in [
            "=== ASTRID JOURNAL ===",
            "=== ASTRID JOURNAL ===\nMode: a\nMode: b\nTimestamp: invalid",
            "Mode: body claim\nTimestamp: 123",
        ] {
            std::fs::write(
                &path,
                format!(
                    "{header}\n\n{}\nMode: claimed\nTimestamp: 99",
                    "A historical thought. ".repeat(8)
                ),
            )
            .unwrap();
            let recall = JournalRecall::read(&path).unwrap();
            assert!(recall.mode.is_none());
            assert!(recall.recorded_at.is_none());
        }
    }
}
