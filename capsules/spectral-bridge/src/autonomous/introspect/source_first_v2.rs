use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_OUTLINE_ENTRIES: usize = 160;
const MAX_PROMPT_OUTLINE_ENTRIES: usize = 80;
const MAX_OUTLINE_LABEL_CHARS: usize = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceCoverageStateV2 {
    CompleteFile,
    MultiWindowComplete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ClaimSupportStateV2 {
    CompleteSourceAvailable,
    StructuralChallengeRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SourceIntervalV2 {
    start_line: usize,
    end_line: usize,
}

impl SourceIntervalV2 {
    fn from_zero_based(start: usize, end: usize) -> Self {
        Self {
            start_line: start.saturating_add(1),
            end_line: end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SourceOutlineEntryV2 {
    line: usize,
    kind: String,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct IntrospectionReadSessionV2 {
    schema: &'static str,
    schema_version: u8,
    read_session_id: String,
    source_identity: String,
    source_sha256: String,
    included_intervals: Vec<SourceIntervalV2>,
    uncovered_intervals: Vec<SourceIntervalV2>,
    coverage_state: SourceCoverageStateV2,
    persists_across_process_restart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SourceCoverageManifestV2 {
    schema: &'static str,
    schema_version: u8,
    source_identity: String,
    source_sha256: String,
    source_bytes: usize,
    source_lines: usize,
    parser_kind: String,
    structural_map_sha256: String,
    outline: Vec<SourceOutlineEntryV2>,
    outline_entry_count: usize,
    outline_truncated: bool,
    read_session_v2: IntrospectionReadSessionV2,
    claim_support_state: ClaimSupportStateV2,
    activation_boundary: &'static str,
    artifact_authority: &'static str,
}

#[derive(Debug, Default)]
struct SessionAccumulator {
    intervals: Vec<(usize, usize)>,
    read_count: usize,
}

static READ_SESSIONS: OnceLock<Mutex<HashMap<String, SessionAccumulator>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, SessionAccumulator>> {
    READ_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_identity(path: &Path) -> String {
    let paths = crate::paths::bridge_paths();
    let candidates = [
        ("astrid", paths.astrid_root()),
        ("minime", paths.minime_root()),
    ];
    for (owner, root) in candidates {
        if let Ok(relative) = path.strip_prefix(root) {
            return format!("{owner}/{}", relative.display());
        }
    }
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown_source")
        .to_string()
}

fn parser_kind(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("rs") => "rust_item_outline_v2",
        Some("py") => "python_symbol_outline_v2",
        Some("md") => "markdown_heading_outline_v2",
        Some("toml" | "yaml" | "yml" | "json") => "structured_key_outline_v2",
        _ => "deterministic_text_outline_v2",
    }
}

fn bounded_label(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_OUTLINE_LABEL_CHARS)
        .collect()
}

fn rust_outline_kind(line: &str) -> Option<&'static str> {
    let mut value = line.trim_start();
    if value.starts_with("#[") || value.starts_with("//") {
        return None;
    }
    if value.starts_with("pub(")
        && let Some((_, tail)) = value.split_once(") ")
    {
        value = tail;
    } else if let Some(tail) = value.strip_prefix("pub ") {
        value = tail;
    }
    if let Some(tail) = value.strip_prefix("async ") {
        value = tail;
    }
    [
        ("fn ", "function"),
        ("struct ", "struct"),
        ("enum ", "enum"),
        ("trait ", "trait"),
        ("impl ", "impl"),
        ("mod ", "module"),
        ("const ", "constant"),
        ("static ", "static"),
        ("type ", "type_alias"),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| value.starts_with(prefix).then_some(kind))
}

fn outline_entry(path: &Path, line_number: usize, line: &str) -> Option<SourceOutlineEntryV2> {
    let trimmed = line.trim();
    let kind = match path.extension().and_then(|value| value.to_str()) {
        Some("rs") => rust_outline_kind(trimmed),
        Some("py") => {
            let value = trimmed.strip_prefix("async ").unwrap_or(trimmed);
            if value.starts_with("def ") {
                Some("function")
            } else if value.starts_with("class ") {
                Some("class")
            } else {
                None
            }
        },
        Some("md") => trimmed.starts_with('#').then_some("heading"),
        Some("toml" | "yaml" | "yml") => {
            (trimmed.starts_with('[') || trimmed.ends_with(':')).then_some("section")
        },
        Some("json") => trimmed.starts_with('"').then_some("key"),
        _ => (trimmed.ends_with(':') && trimmed.len() <= MAX_OUTLINE_LABEL_CHARS)
            .then_some("section"),
    }?;
    Some(SourceOutlineEntryV2 {
        line: line_number,
        kind: kind.to_string(),
        label: bounded_label(trimmed),
    })
}

fn structural_outline(
    path: &Path,
    content: &str,
) -> (Vec<SourceOutlineEntryV2>, usize, bool, String) {
    let all: Vec<_> = content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| outline_entry(path, index.saturating_add(1), line))
        .collect();
    let digest_input = all
        .iter()
        .map(|entry| format!("{}:{}:{}", entry.line, entry.kind, entry.label))
        .collect::<Vec<_>>()
        .join("\n");
    let count = all.len();
    let truncated = count > MAX_OUTLINE_ENTRIES;
    (
        all.into_iter().take(MAX_OUTLINE_ENTRIES).collect(),
        count,
        truncated,
        sha256_bytes(digest_input.as_bytes()),
    )
}

fn merged_intervals(intervals: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted = intervals.to_vec();
    sorted.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in sorted {
        if start >= end {
            continue;
        }
        if let Some((_, previous_end)) = merged.last_mut()
            && start <= *previous_end
        {
            *previous_end = (*previous_end).max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn uncovered_intervals(included: &[(usize, usize)], total: usize) -> Vec<(usize, usize)> {
    let mut uncovered = Vec::new();
    let mut cursor = 0;
    for (start, end) in included {
        if cursor < *start {
            uncovered.push((cursor, *start));
        }
        cursor = cursor.max(*end);
    }
    if cursor < total {
        uncovered.push((cursor, total));
    }
    uncovered
}

fn interval_text(intervals: &[SourceIntervalV2]) -> String {
    if intervals.is_empty() {
        return "none".to_string();
    }
    intervals
        .iter()
        .map(|interval| format!("{}-{}", interval.start_line, interval.end_line))
        .collect::<Vec<_>>()
        .join(",")
}

impl SourceCoverageManifestV2 {
    pub(super) fn artifact_header_v2(&self) -> String {
        format!(
            "Source coverage schema: source_coverage_manifest_v2\n\
             Source read session: {}\n\
             Source SHA-256: {}\n\
             Source parser: {}\n\
             Source structural map SHA-256: {}\n\
             Source included intervals: {}\n\
             Source uncovered intervals: {}\n\
             Source coverage state: {}\n\
             Source claim support: {}",
            self.read_session_v2.read_session_id,
            self.source_sha256,
            self.parser_kind,
            self.structural_map_sha256,
            interval_text(&self.read_session_v2.included_intervals),
            interval_text(&self.read_session_v2.uncovered_intervals),
            match self.read_session_v2.coverage_state {
                SourceCoverageStateV2::CompleteFile => "complete_file",
                SourceCoverageStateV2::MultiWindowComplete => "multi_window_complete",
                SourceCoverageStateV2::Partial => "partial",
            },
            match self.claim_support_state {
                ClaimSupportStateV2::CompleteSourceAvailable => "complete_source_available",
                ClaimSupportStateV2::StructuralChallengeRequired => {
                    "partial_source_absence_and_new-implementation_claims_require_exact_structural_challenge"
                },
            },
        )
    }

    pub(super) fn prompt_context_v2(&self) -> String {
        let mut lines = vec![
            "\n\n// --- Source-First Coverage Manifest V2 ---".to_string(),
            format!(
                "// read_session={} source_sha256={}",
                self.read_session_v2.read_session_id, self.source_sha256
            ),
            format!(
                "// parser={} structural_map_sha256={} included={} uncovered={}",
                self.parser_kind,
                self.structural_map_sha256,
                interval_text(&self.read_session_v2.included_intervals),
                interval_text(&self.read_session_v2.uncovered_intervals),
            ),
            "// Binding claim rule: a partial read may not claim that a schema, function, \
             mechanism, or implementation is absent unless the whole-file structural map \
             and exact symbol search support that claim."
                .to_string(),
            "// Whole-file structural outline:".to_string(),
        ];
        lines.extend(
            self.outline
                .iter()
                .take(MAX_PROMPT_OUTLINE_ENTRIES)
                .map(|entry| format!("//   L{} [{}] {}", entry.line, entry.kind, entry.label)),
        );
        if self.outline_entry_count > MAX_PROMPT_OUTLINE_ENTRIES {
            lines.push(format!(
                "//   ... {} additional outline entries omitted from prompt; digest covers all entries",
                self.outline_entry_count
                    .saturating_sub(MAX_PROMPT_OUTLINE_ENTRIES)
            ));
        }
        lines.join("\n")
    }

    fn is_partial(&self) -> bool {
        self.read_session_v2.coverage_state == SourceCoverageStateV2::Partial
    }
}

pub(super) fn build_source_coverage_manifest_v2(
    path: &Path,
    content: &str,
    start: usize,
    end: usize,
    total: usize,
) -> SourceCoverageManifestV2 {
    let identity = source_identity(path);
    let source_sha256 = sha256_bytes(content.as_bytes());
    let read_session_id = sha256_bytes(
        format!("introspection_read_session_v2\0{identity}\0{source_sha256}").as_bytes(),
    );
    let (included, read_count) = {
        let mut guard = sessions()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let accumulator = guard.entry(read_session_id.clone()).or_default();
        accumulator.intervals.push((start, end));
        accumulator.read_count = accumulator.read_count.saturating_add(1);
        accumulator.intervals = merged_intervals(&accumulator.intervals);
        (accumulator.intervals.clone(), accumulator.read_count)
    };
    let uncovered = uncovered_intervals(&included, total);
    let coverage_state = if uncovered.is_empty() {
        if read_count > 1 {
            SourceCoverageStateV2::MultiWindowComplete
        } else {
            SourceCoverageStateV2::CompleteFile
        }
    } else {
        SourceCoverageStateV2::Partial
    };
    let claim_support_state = if uncovered.is_empty() {
        ClaimSupportStateV2::CompleteSourceAvailable
    } else {
        ClaimSupportStateV2::StructuralChallengeRequired
    };
    let (outline, outline_entry_count, outline_truncated, structural_map_sha256) =
        structural_outline(path, content);
    SourceCoverageManifestV2 {
        schema: "source_coverage_manifest_v2",
        schema_version: 2,
        source_identity: identity.clone(),
        source_sha256: source_sha256.clone(),
        source_bytes: content.len(),
        source_lines: total,
        parser_kind: parser_kind(path).to_string(),
        structural_map_sha256,
        outline,
        outline_entry_count,
        outline_truncated,
        read_session_v2: IntrospectionReadSessionV2 {
            schema: "introspection_read_session_v2",
            schema_version: 2,
            read_session_id,
            source_identity: identity,
            source_sha256,
            included_intervals: included
                .iter()
                .map(|(interval_start, interval_end)| {
                    SourceIntervalV2::from_zero_based(*interval_start, *interval_end)
                })
                .collect(),
            uncovered_intervals: uncovered
                .iter()
                .map(|(interval_start, interval_end)| {
                    SourceIntervalV2::from_zero_based(*interval_start, *interval_end)
                })
                .collect(),
            coverage_state,
            persists_across_process_restart: false,
        },
        claim_support_state,
        activation_boundary: "source_read_not_runtime_activation_proof",
        artifact_authority: "read_only_evidence_not_control_or_approval",
    }
}

fn backticked_identifiers(response: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = response;
    while let Some((_, tail)) = rest.split_once('`') {
        let Some((candidate, after)) = tail.split_once('`') else {
            break;
        };
        rest = after;
        let candidate = candidate.trim();
        let identifier = candidate
            .strip_prefix("fn ")
            .unwrap_or(candidate)
            .split(['(', '<', ' ', ':'])
            .next()
            .unwrap_or("")
            .trim_matches(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'));
        if identifier.len() >= 3
            && identifier
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            values.push(identifier.to_string());
        }
    }
    values.sort();
    values.dedup();
    values
}

fn has_absence_claim(response: &str) -> bool {
    let lower = response.to_ascii_lowercase();
    [
        " is absent",
        " are absent",
        " is missing",
        " are missing",
        " does not implement",
        " doesn't implement",
        " not implemented",
        " lacks ",
        " no mechanism",
        " no schema",
        " no function",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub(super) fn response_claims_supported_v2(
    response: &str,
    path: &Path,
    manifest: &SourceCoverageManifestV2,
) -> bool {
    if !manifest.is_partial() || !has_absence_claim(response) {
        return true;
    }
    let identifiers = backticked_identifiers(response);
    if identifiers.is_empty() {
        return false;
    }
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    identifiers
        .iter()
        .all(|identifier| !content.contains(identifier))
}

pub(super) fn unavailable_header_v2() -> &'static str {
    "Source coverage schema: source_coverage_manifest_v2\n\
     Source coverage state: unavailable\n\
     Source claim support: source_unavailable"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_source_hash_aggregates_intervals_and_source_change_starts_session() {
        let path = Path::new("/tmp/source_first_v2_unique.rs");
        let content = (0..900)
            .map(|index| format!("fn item_{index}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n");
        let first = build_source_coverage_manifest_v2(path, &content, 0, 400, 900);
        let second = build_source_coverage_manifest_v2(path, &content, 400, 800, 900);
        assert_eq!(
            first.read_session_v2.read_session_id,
            second.read_session_v2.read_session_id
        );
        assert_eq!(second.read_session_v2.included_intervals.len(), 1);
        assert_eq!(second.read_session_v2.included_intervals[0].start_line, 1);
        assert_eq!(second.read_session_v2.included_intervals[0].end_line, 800);

        let changed =
            build_source_coverage_manifest_v2(path, &(content + "\nfn changed() {}"), 0, 400, 901);
        assert_ne!(
            second.read_session_v2.read_session_id,
            changed.read_session_v2.read_session_id
        );
    }

    #[test]
    fn complete_multi_window_session_has_no_uncovered_intervals() {
        let path = Path::new("/tmp/source_first_v2_complete.rs");
        let content = (0..800)
            .map(|index| format!("fn item_{index}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n");
        build_source_coverage_manifest_v2(path, &content, 0, 400, 800);
        let complete = build_source_coverage_manifest_v2(path, &content, 400, 800, 800);
        assert_eq!(
            complete.read_session_v2.coverage_state,
            SourceCoverageStateV2::MultiWindowComplete
        );
        assert!(complete.read_session_v2.uncovered_intervals.is_empty());
    }

    #[test]
    fn partial_read_rejects_false_absence_found_later_in_source() {
        let path = std::env::temp_dir().join("source_first_v2_absence.rs");
        let mut lines = vec!["// padding".to_string(); 1_133];
        lines[800] = "pub struct ImplementedSchemaV2 {}".to_string();
        let content = lines.join("\n");
        fs::write(&path, &content).expect("fixture");
        let manifest = build_source_coverage_manifest_v2(&path, &content, 0, 400, 1_133);
        assert!(!response_claims_supported_v2(
            "Likely Snags:\nThe source is missing `ImplementedSchemaV2`.",
            &path,
            &manifest,
        ));
        assert!(response_claims_supported_v2(
            "Likely Snags:\nThe source is missing `NeverDefinedSchemaV2`.",
            &path,
            &manifest,
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn structural_outline_is_bounded_but_digest_covers_full_outline() {
        let path = Path::new("/tmp/source_first_v2_bounded.rs");
        let content = (0..500)
            .map(|index| format!("pub fn function_{index}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n");
        let manifest = build_source_coverage_manifest_v2(path, &content, 0, 400, 500);
        assert_eq!(manifest.outline.len(), MAX_OUTLINE_ENTRIES);
        assert_eq!(manifest.outline_entry_count, 500);
        assert!(manifest.outline_truncated);
    }
}
