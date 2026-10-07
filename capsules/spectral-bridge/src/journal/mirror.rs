//! Source identity and excerpt scope from one read, before any context projection.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Serialize)]
pub(crate) struct MirrorExcerpt {
    pub schema: &'static str,
    pub source_sha256: String,
    pub source_bytes: usize,
    pub parsed_body_sha256: String,
    pub parsed_body_chars: usize,
    pub selected_body_chars: usize,
    pub excerpt_truncated: bool,
    pub scope: &'static str,
    #[serde(skip_serializing)]
    pub text: String,
}

impl MirrorExcerpt {
    pub(crate) fn read(path: &Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let content = std::str::from_utf8(&bytes).ok()?;
        let full = super::extract_full_journal_body(content, false)?;
        let parsed_body_chars = full.chars().count();
        let selected: String = full.chars().take(2500).collect();
        Some(Self {
            schema: "mirror_excerpt_v1",
            source_sha256: format!("{:x}", Sha256::digest(&bytes)),
            source_bytes: bytes.len(),
            parsed_body_sha256: format!("{:x}", Sha256::digest(full.as_bytes())),
            parsed_body_chars,
            selected_body_chars: selected.chars().count(),
            excerpt_truncated: parsed_body_chars > 2500,
            scope: "first 2500 Unicode scalars of parsed body, not raw-file offsets; existing header/markup normalization and peer-action sanitization apply; no full-source fidelity claim",
            text: super::remote_body_context(path, content, selected),
        })
    }
}

/// Authorship is determined by record metadata, never by a phrase in the prose.
pub(crate) fn is_peer_mirror_record(content: &str) -> bool {
    let header = content.split("\n\n").next().unwrap_or_default();
    header.starts_with("=== PEER MIRROR EXCERPT ===\n")
        || (header.starts_with("=== ASTRID JOURNAL ===\n")
            && header.lines().any(|line| {
                matches!(
                    line.trim(),
                    "Mode: mirror"
                        | "Provenance: minime_observed_expression"
                        | "Authorship: minime_owned_reflected_without_reauthoring"
                )
            }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_excerpt_preserves_old_projection_but_discloses_lost_tail() {
        let root = tempfile::tempdir().unwrap();
        for name in ["introspect_example.txt", "self_study_example.txt"] {
            let path = root.path().join(name);
            let content = format!(
                "=== JOURNAL ===\nTimestamp: 42\n\n{}TAIL",
                "λabc ".repeat(600)
            );
            std::fs::write(&path, &content).unwrap();
            let excerpt = MirrorExcerpt::read(&path).unwrap();
            assert_eq!(
                Some(excerpt.text.clone()),
                super::super::read_remote_journal_body(&path)
            );
            assert!(excerpt.excerpt_truncated);
            assert_eq!(excerpt.selected_body_chars, 2500);
            assert_eq!(excerpt.parsed_body_chars, 3004);
            assert_eq!(
                excerpt.source_sha256,
                format!("{:x}", Sha256::digest(content.as_bytes()))
            );
            assert!(!excerpt.text.ends_with("TAIL"));
            let metadata = serde_json::to_string(&excerpt).unwrap();
            assert!(!metadata.contains("λabc"));
            assert!(metadata.contains("no full-source fidelity claim"));
        }
    }

    #[test]
    fn exact_boundary_short_and_starred_sources_have_truthful_identity() {
        let root = tempfile::tempdir().unwrap();
        for size in [50, 2499, 2500, 2501] {
            let path = root.path().join("entry.txt");
            let content = "é".repeat(size);
            std::fs::write(&path, &content).unwrap();
            let first = MirrorExcerpt::read(&path).unwrap();
            assert_eq!(first.excerpt_truncated, size > 2500);
            assert_eq!(first.source_bytes, size * 2);
            let renamed = root.path().join("!entry.txt");
            std::fs::rename(path, &renamed).unwrap();
            let second = MirrorExcerpt::read(&renamed).unwrap();
            assert_eq!(first.source_sha256, second.source_sha256);
            assert_eq!(first.text, second.text);
        }
    }

    #[test]
    fn legacy_and_new_peer_records_cannot_become_own_continuity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("astrid_42.txt");
        for header in [
            "=== ASTRID JOURNAL ===\nMode: mirror",
            "=== ASTRID JOURNAL ===\nAuthorship: minime_owned_reflected_without_reauthoring",
            "=== PEER MIRROR EXCERPT ===\nMode: mirror",
        ] {
            let content = format!("{header}\n\n{}", "Peer-authored account. ".repeat(5));
            std::fs::write(&path, &content).unwrap();
            assert!(is_peer_mirror_record(&content));
            assert!(super::super::read_local_journal_body_for_continuity(&path).is_none());
            assert!(super::super::JournalRecall::read(&path).is_none());
            assert!(super::super::read_remote_journal_body(&path).is_some());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
        }
        assert!(!is_peer_mirror_record(
            "=== ASTRID JOURNAL ===\nMode: aspiration\n\nMode: mirror\nMy own discussion of mirroring."
        ));
    }
}
