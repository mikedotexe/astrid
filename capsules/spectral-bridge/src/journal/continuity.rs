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
    pub(crate) fn read(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
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
        let metadata = serde_json::json!({
            "source": self.source, "record_sha256": self.record_sha256,
            "recorded_at_unix_s": self.recorded_at, "mode": self.mode,
            "excerpt_truncated": self.body.chars().count() > body_chars,
        });
        format!(
            "Historical own-journal excerpt v1: {metadata}\n\
             This is prior authored interpretation. Null metadata is unavailable. \
             The journal recording time is not a measurement capture time; the original \
             measurement source and capture time are unavailable in this recall's metadata.\n\
             Excerpt:\n{}\nEnd historical excerpt.",
            self.body.chars().take(body_chars).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
