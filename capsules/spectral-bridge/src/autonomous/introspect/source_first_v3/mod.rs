mod catalog;
mod grounding;
mod mapping;
mod rendering;
mod session;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceCoverageStateV3 {
    CompleteFile,
    MultiWindowComplete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SourceIntervalV3 {
    pub(super) start_line: usize,
    pub(super) end_line: usize,
}

impl SourceIntervalV3 {
    pub(super) fn from_zero_based(start: usize, end: usize) -> Self {
        Self {
            start_line: start.saturating_add(1),
            end_line: end,
        }
    }

    pub(super) fn contains_line(&self, line: usize) -> bool {
        self.start_line <= line && line <= self.end_line
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ParserDisclosureV3 {
    pub(super) parser_kind: String,
    pub(super) parser_version: String,
    pub(super) structured_parser: bool,
    pub(super) fallback_used: bool,
    pub(super) fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SourceMapEntryV3 {
    pub(super) kind: String,
    pub(super) label: String,
    pub(super) structural_path: String,
    pub(super) start_line: Option<usize>,
    pub(super) end_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SourceMapV3 {
    pub(super) schema: String,
    pub(super) schema_version: u8,
    pub(super) source_identity: String,
    pub(super) source_sha256: String,
    pub(super) source_bytes: usize,
    pub(super) source_lines: usize,
    pub(super) parser: ParserDisclosureV3,
    pub(super) entries: Vec<SourceMapEntryV3>,
    pub(super) observed_entry_count: usize,
    pub(super) entries_truncated: bool,
    pub(super) structural_map_sha256: String,
    pub(super) complete_source_parsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ReadSessionCheckpointV3 {
    pub(super) schema: String,
    pub(super) schema_version: u8,
    pub(super) read_session_id: String,
    pub(super) source_identity: String,
    pub(super) source_sha256: String,
    pub(super) structural_map_sha256: String,
    pub(super) parser: ParserDisclosureV3,
    pub(super) included_intervals: Vec<SourceIntervalV3>,
    pub(super) uncovered_intervals: Vec<SourceIntervalV3>,
    pub(super) coverage_state: SourceCoverageStateV3,
    pub(super) read_count: usize,
    pub(super) persists_across_process_restart: bool,
    pub(super) active_for_source_identity: bool,
    pub(super) supersedes_session_id: Option<String>,
    pub(super) invalidated_by_session_id: Option<String>,
    pub(super) source_changed_since_previous: bool,
    pub(super) updated_at_unix_ms: u64,
    pub(super) artifact_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SourceEvidenceV3 {
    pub(super) source_map_v3: SourceMapV3,
    pub(super) read_session_checkpoint_v3: ReadSessionCheckpointV3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ClaimKindV2 {
    Absence,
    NewImplementation,
    SourceAttribution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ClaimSupportStateV2 {
    Supported,
    Rejected,
    RequiresChangeEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ClaimSupportRefV2 {
    pub(super) schema: String,
    pub(super) schema_version: u8,
    pub(super) claim_sha256: String,
    pub(super) claim_kind: ClaimKindV2,
    pub(super) exact_identifiers: Vec<String>,
    pub(super) source_identity: String,
    pub(super) source_sha256: String,
    pub(super) structural_map_sha256: String,
    pub(super) read_session_id: String,
    pub(super) whole_source_challenge_performed: bool,
    pub(super) source_hash_current: bool,
    pub(super) support_state: ClaimSupportStateV2,
    pub(super) reason: String,
    pub(super) artifact_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ClaimChallengeReportV2 {
    pub(super) schema: String,
    pub(super) schema_version: u8,
    pub(super) all_supported: bool,
    pub(super) challenged_claim_count: usize,
    pub(super) support_refs: Vec<ClaimSupportRefV2>,
}

pub(super) fn build_source_evidence_v3(
    path: &Path,
    content: &str,
    start: usize,
    end: usize,
    total: usize,
) -> Result<SourceEvidenceV3, String> {
    let root = default_root();
    build_source_evidence_v3_at_root(&root, path, content, start, end, total)
}

pub(super) fn build_source_evidence_v3_at_root(
    root: &Path,
    path: &Path,
    content: &str,
    start: usize,
    end: usize,
    total: usize,
) -> Result<SourceEvidenceV3, String> {
    let cataloged = catalog::catalog_source(path, content, total);
    let source_map_v3 = mapping::build_source_map_v3(path, content, &cataloged)?;
    let read_session_checkpoint_v3 =
        session::update_read_session_v3(root, &cataloged, &source_map_v3, start, end, total)?;
    Ok(SourceEvidenceV3 {
        source_map_v3,
        read_session_checkpoint_v3,
    })
}

pub(super) fn challenge_response_claims_v3(
    response: &str,
    path: &Path,
    evidence: &SourceEvidenceV3,
) -> ClaimChallengeReportV2 {
    grounding::challenge_response_claims_v3(response, path, evidence)
}

pub(super) fn artifact_header_v3(
    source_map: &SourceMapV3,
    checkpoint: &ReadSessionCheckpointV3,
) -> String {
    rendering::artifact_header_v3(source_map, checkpoint)
}

pub(super) fn prompt_context_v3(
    source_map: &SourceMapV3,
    checkpoint: &ReadSessionCheckpointV3,
) -> String {
    rendering::prompt_context_v3(source_map, checkpoint)
}

fn default_root() -> PathBuf {
    crate::paths::bridge_paths()
        .bridge_workspace()
        .join("diagnostics/source_first_v3")
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn fixture_lines(count: usize) -> String {
        (0..count)
            .map(|index| format!("// padding {index}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rust_tree_sitter_maps_multiline_items() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("multiline.rs");
        let content =
            "pub async\nfn carried_across_lines(\n    value: usize,\n) -> usize {\n    value\n}\n";
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 6, 6)
            .expect("evidence");
        assert_eq!(
            evidence.source_map_v3.parser.parser_kind,
            "tree_sitter_rust"
        );
        assert!(
            evidence
                .source_map_v3
                .entries
                .iter()
                .any(|entry| entry.label == "carried_across_lines")
        );
    }

    #[test]
    fn rust_include_shell_maps_dependency_edges_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("runtime.rs");
        let content = "include!(\"runtime/semantic_modality.rs\");\n\
                       include!(\"runtime/orchestration.rs\");\n\
                       unrelated_macro!(\"not/a/source/edge.rs\");\n";
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 3, 3)
            .expect("evidence");
        let include_edges = evidence
            .source_map_v3
            .entries
            .iter()
            .filter(|entry| entry.kind == "include_edge")
            .collect::<Vec<_>>();

        assert_eq!(include_edges.len(), 2);
        assert_eq!(include_edges[0].label, "runtime/semantic_modality.rs");
        assert_eq!(
            include_edges[0].structural_path,
            "include::runtime/semantic_modality.rs"
        );
        assert_eq!(include_edges[1].label, "runtime/orchestration.rs");
        assert!(
            evidence
                .source_map_v3
                .entries
                .iter()
                .all(|entry| entry.label != "not/a/source/edge.rs")
        );
    }

    #[test]
    fn structured_and_fallback_parsers_disclose_their_basis() {
        let dir = tempfile::tempdir().expect("tempdir");
        let json_path = dir.path().join("manifest.json");
        let json = "{\"outer\":{\"inner\":true}}";
        let structured = build_source_evidence_v3_at_root(dir.path(), &json_path, json, 0, 1, 1)
            .expect("structured");
        assert_eq!(structured.source_map_v3.parser.parser_kind, "serde_json");
        assert!(!structured.source_map_v3.parser.fallback_used);

        let malformed_path = dir.path().join("malformed.json");
        let fallback =
            build_source_evidence_v3_at_root(dir.path(), &malformed_path, "{oops", 0, 1, 1)
                .expect("fallback");
        assert_eq!(
            fallback.source_map_v3.parser.parser_kind,
            "line_scanner_fallback_v3"
        );
        assert!(fallback.source_map_v3.parser.fallback_used);
        assert!(
            fallback
                .source_map_v3
                .parser
                .fallback_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("JSON parse failed"))
        );
    }

    #[test]
    fn read_session_survives_reopen_and_changed_source_invalidates_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("large.rs");
        let content = fixture_lines(800);
        let first = build_source_evidence_v3_at_root(dir.path(), &path, &content, 0, 400, 800)
            .expect("first");
        let second = build_source_evidence_v3_at_root(dir.path(), &path, &content, 400, 800, 800)
            .expect("second");
        assert_eq!(
            first.read_session_checkpoint_v3.read_session_id,
            second.read_session_checkpoint_v3.read_session_id
        );
        assert_eq!(
            second.read_session_checkpoint_v3.coverage_state,
            SourceCoverageStateV3::MultiWindowComplete
        );
        assert!(
            second
                .read_session_checkpoint_v3
                .uncovered_intervals
                .is_empty()
        );

        let changed_content = format!("{content}\nfn changed_source() {{}}");
        let changed =
            build_source_evidence_v3_at_root(dir.path(), &path, &changed_content, 0, 400, 801)
                .expect("changed");
        assert_ne!(
            second.read_session_checkpoint_v3.read_session_id,
            changed.read_session_checkpoint_v3.read_session_id
        );
        assert!(
            changed
                .read_session_checkpoint_v3
                .source_changed_since_previous
        );

        let old_path = dir.path().join("sessions").join(format!(
            "{}.json",
            second.read_session_checkpoint_v3.read_session_id
        ));
        let old: ReadSessionCheckpointV3 =
            serde_json::from_slice(&fs::read(old_path).expect("old checkpoint"))
                .expect("old checkpoint JSON");
        assert!(!old.active_for_source_identity);
        assert_eq!(
            old.invalidated_by_session_id.as_deref(),
            Some(changed.read_session_checkpoint_v3.read_session_id.as_str())
        );
    }

    #[test]
    fn eleven_hundred_line_partial_read_cannot_claim_unseen_schema_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("partial.rs");
        let mut lines = vec!["// padding".to_string(); 1_133];
        lines[800] = "pub struct ImplementedSchemaV3 {}".to_string();
        let content = lines.join("\n");
        fs::write(&path, &content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, &content, 0, 400, 1_133)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Likely Snags: the source is missing `ImplementedSchemaV3`.",
            &path,
            &evidence,
        );
        assert!(!report.all_supported);
        assert_eq!(report.challenged_claim_count, 1);
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::Rejected
        );
    }

    #[test]
    fn temporal_implementation_claim_requires_change_evidence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("present.rs");
        let content = "pub struct PresentSchemaV3 {}\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 1, 1)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the source now implements `PresentSchemaV3`.",
            &path,
            &evidence,
        );
        assert!(!report.all_supported);
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::RequiresChangeEvidence
        );
    }

    #[test]
    fn affirmative_source_attribution_rejects_imagined_symbol() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("grounded.rs");
        let content = "pub fn actual_runtime() {}\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 1, 1)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the source defines the `ImaginedRuntime` struct.",
            &path,
            &evidence,
        );

        assert!(!report.all_supported);
        assert_eq!(report.challenged_claim_count, 1);
        assert_eq!(
            report.support_refs[0].claim_kind,
            ClaimKindV2::SourceAttribution
        );
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::Rejected
        );
    }

    #[test]
    fn affirmative_source_attribution_requires_exact_identifier() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("grounded.rs");
        let content = "pub fn actual_runtime() {}\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 1, 1)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the source defines the runtime described here.",
            &path,
            &evidence,
        );

        assert!(!report.all_supported);
        assert_eq!(report.challenged_claim_count, 1);
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::Rejected
        );
    }

    #[test]
    fn affirmative_markdown_attribution_rejects_prompt_context_as_source_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("DOMAIN_BOUNDARIES.md");
        let content = "# Domain Boundaries\n\n## Stable Facades\n\nOwnership remains explicit.\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 5, 5)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the `DOMAIN_BOUNDARIES.md` file establishes an Inhabitable taxonomy.\n\
             Likely Snags: the code labels the current state as `settled_habitable`.\n\
             Likely Snags: the documentation describes a gentle navigable slope.",
            &path,
            &evidence,
        );

        assert!(!report.all_supported);
        assert_eq!(report.challenged_claim_count, 3);
        assert!(
            report
                .support_refs
                .iter()
                .all(|support| support.support_state == ClaimSupportStateV2::Rejected)
        );
    }

    #[test]
    fn source_code_alias_attribution_rejects_prompt_context_scaffolding() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("dialogue_runtime.rs");
        let content = "fn sanitize_model_control_markers() {}\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 1, 1)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: In the source code `astrid:llm`, I see the structural scaffolding for how these spectral energies are weighted.",
            &path,
            &evidence,
        );

        assert!(!report.all_supported);
        assert_eq!(report.challenged_claim_count, 1);
        assert_eq!(report.support_refs[0].exact_identifiers, ["astrid"]);
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::Rejected
        );
    }

    #[test]
    fn affirmative_source_attribution_accepts_visible_field() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("telemetry.rs");
        let content = "pub struct Telemetry {\n    pub spectral_entropy: f32,\n}\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 3, 3)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the source defines `spectral_entropy` in the telemetry schema.",
            &path,
            &evidence,
        );

        assert!(report.all_supported);
        assert_eq!(report.challenged_claim_count, 1);
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::Supported
        );
    }

    #[test]
    fn affirmative_source_attribution_accepts_unseen_mapped_declaration() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("partial.rs");
        let mut lines = vec!["// padding".to_string(); 800];
        lines[700] = "pub fn late_mapped_function() {}".to_string();
        let content = lines.join("\n");
        fs::write(&path, &content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, &content, 0, 400, 800)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the source defines `late_mapped_function`.",
            &path,
            &evidence,
        );

        assert!(report.all_supported);
        assert_eq!(report.challenged_claim_count, 1);
    }

    #[test]
    fn unseen_incidental_token_is_not_structural_presence_evidence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("partial.rs");
        let mut lines = vec!["// padding".to_string(); 800];
        lines[700] = "tracing::info!(pressure_risk = 0.22);".to_string();
        let content = lines.join("\n");
        fs::write(&path, &content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, &content, 0, 400, 800)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Observed: the source defines `pressure_risk` as a runtime field.",
            &path,
            &evidence,
        );

        assert!(!report.all_supported);
        assert_eq!(
            report.support_refs[0].support_state,
            ClaimSupportStateV2::Rejected
        );
    }

    #[test]
    fn proposed_symbols_are_not_misread_as_present_source_claims() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("proposal.rs");
        let content = "pub fn actual_runtime() {}\n";
        fs::write(&path, content).expect("fixture");
        let evidence = build_source_evidence_v3_at_root(dir.path(), &path, content, 0, 1, 1)
            .expect("evidence");
        let report = challenge_response_claims_v3(
            "Suggested Next: I propose adding `ImaginedRuntime`.",
            &path,
            &evidence,
        );

        assert!(report.all_supported);
        assert_eq!(report.challenged_claim_count, 0);
    }

    #[cfg(unix)]
    #[test]
    fn persisted_maps_and_sessions_are_owner_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("owned.py");
        let evidence = build_source_evidence_v3_at_root(
            dir.path(),
            &path,
            "def owned():\n    pass\n",
            0,
            2,
            2,
        )
        .expect("evidence");
        let session_path = dir.path().join("sessions").join(format!(
            "{}.json",
            evidence.read_session_checkpoint_v3.read_session_id
        ));
        assert_eq!(
            fs::metadata(session_path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(dir.path().join("sessions"))
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
