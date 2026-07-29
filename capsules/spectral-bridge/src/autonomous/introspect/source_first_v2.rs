use std::path::Path;

use serde::Serialize;

use super::source_first_v3::{
    self, ReadSessionCheckpointV3, SourceCoverageStateV3, SourceEvidenceV3, SourceMapV3,
};

const MAX_OUTLINE_ENTRIES: usize = 160;
const MAX_PROMPT_OUTLINE_ENTRIES: usize = 80;

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
    source_map_v3: SourceMapV3,
    read_session_checkpoint_v3: ReadSessionCheckpointV3,
    activation_boundary: &'static str,
    artifact_authority: &'static str,
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
        ) + "\n"
            + &source_first_v3::artifact_header_v3(
                &self.source_map_v3,
                &self.read_session_checkpoint_v3,
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
        lines.push(source_first_v3::prompt_context_v3(
            &self.source_map_v3,
            &self.read_session_checkpoint_v3,
        ));
        lines.join("\n")
    }
}

pub(super) fn build_source_coverage_manifest_v2(
    path: &Path,
    content: &str,
    start: usize,
    end: usize,
    total: usize,
) -> Result<SourceCoverageManifestV2, String> {
    let evidence = source_first_v3::build_source_evidence_v3(path, content, start, end, total)?;
    let source_map = &evidence.source_map_v3;
    let checkpoint = &evidence.read_session_checkpoint_v3;
    let coverage_state = match checkpoint.coverage_state {
        SourceCoverageStateV3::CompleteFile => SourceCoverageStateV2::CompleteFile,
        SourceCoverageStateV3::MultiWindowComplete => SourceCoverageStateV2::MultiWindowComplete,
        SourceCoverageStateV3::Partial => SourceCoverageStateV2::Partial,
    };
    let claim_support_state = if checkpoint.uncovered_intervals.is_empty() {
        ClaimSupportStateV2::CompleteSourceAvailable
    } else {
        ClaimSupportStateV2::StructuralChallengeRequired
    };
    let outline = source_map
        .entries
        .iter()
        .take(MAX_OUTLINE_ENTRIES)
        .map(|entry| SourceOutlineEntryV2 {
            line: entry.start_line.unwrap_or(0),
            kind: entry.kind.clone(),
            label: entry.label.clone(),
        })
        .collect();
    Ok(SourceCoverageManifestV2 {
        schema: "source_coverage_manifest_v2",
        schema_version: 2,
        source_identity: source_map.source_identity.clone(),
        source_sha256: source_map.source_sha256.clone(),
        source_bytes: source_map.source_bytes,
        source_lines: total,
        parser_kind: source_map.parser.parser_kind.clone(),
        structural_map_sha256: source_map.structural_map_sha256.clone(),
        outline,
        outline_entry_count: source_map.observed_entry_count,
        outline_truncated: source_map.observed_entry_count > MAX_OUTLINE_ENTRIES,
        read_session_v2: IntrospectionReadSessionV2 {
            schema: "introspection_read_session_v2",
            schema_version: 2,
            read_session_id: checkpoint.read_session_id.clone(),
            source_identity: checkpoint.source_identity.clone(),
            source_sha256: checkpoint.source_sha256.clone(),
            included_intervals: checkpoint
                .included_intervals
                .iter()
                .map(|interval| SourceIntervalV2 {
                    start_line: interval.start_line,
                    end_line: interval.end_line,
                })
                .collect(),
            uncovered_intervals: checkpoint
                .uncovered_intervals
                .iter()
                .map(|interval| SourceIntervalV2 {
                    start_line: interval.start_line,
                    end_line: interval.end_line,
                })
                .collect(),
            coverage_state,
            persists_across_process_restart: true,
        },
        claim_support_state,
        source_map_v3: evidence.source_map_v3,
        read_session_checkpoint_v3: evidence.read_session_checkpoint_v3,
        activation_boundary: "source_read_not_runtime_activation_proof",
        artifact_authority: "read_only_evidence_not_control_or_approval",
    })
}

pub(super) fn response_claims_supported_v2(
    response: &str,
    path: &Path,
    manifest: &SourceCoverageManifestV2,
) -> bool {
    let evidence = SourceEvidenceV3 {
        source_map_v3: manifest.source_map_v3.clone(),
        read_session_checkpoint_v3: manifest.read_session_checkpoint_v3.clone(),
    };
    source_first_v3::challenge_response_claims_v3(response, path, &evidence).all_supported
}

pub(super) fn unavailable_header_v2() -> &'static str {
    "Source coverage schema: source_coverage_manifest_v2\n\
     Source coverage state: unavailable\n\
     Source claim support: source_unavailable"
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn same_source_hash_aggregates_intervals_and_source_change_starts_session() {
        let path = Path::new("/tmp/source_first_v2_unique.rs");
        let content = (0..900)
            .map(|index| format!("fn item_{index}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n");
        let first = build_source_coverage_manifest_v2(path, &content, 0, 400, 900).expect("first");
        let second =
            build_source_coverage_manifest_v2(path, &content, 400, 800, 900).expect("second");
        assert_eq!(
            first.read_session_v2.read_session_id,
            second.read_session_v2.read_session_id
        );
        assert_eq!(second.read_session_v2.included_intervals.len(), 1);
        assert_eq!(second.read_session_v2.included_intervals[0].start_line, 1);
        assert_eq!(second.read_session_v2.included_intervals[0].end_line, 800);

        let changed =
            build_source_coverage_manifest_v2(path, &(content + "\nfn changed() {}"), 0, 400, 901)
                .expect("changed");
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
        build_source_coverage_manifest_v2(path, &content, 0, 400, 800).expect("first");
        let complete =
            build_source_coverage_manifest_v2(path, &content, 400, 800, 800).expect("complete");
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
        let manifest =
            build_source_coverage_manifest_v2(&path, &content, 0, 400, 1_133).expect("manifest");
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
        let manifest =
            build_source_coverage_manifest_v2(path, &content, 0, 400, 500).expect("manifest");
        assert_eq!(manifest.outline.len(), MAX_OUTLINE_ENTRIES);
        assert_eq!(manifest.outline_entry_count, 500);
        assert!(manifest.outline_truncated);
    }
}
