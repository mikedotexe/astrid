use super::{ReadSessionCheckpointV3, SourceIntervalV3, SourceMapV3};

const MAX_PROMPT_ENTRIES: usize = 80;

pub(super) fn artifact_header_v3(
    source_map: &SourceMapV3,
    checkpoint: &ReadSessionCheckpointV3,
) -> String {
    format!(
        "Source map schema: source_map_v3\n\
         Source read checkpoint: {}\n\
         Source V3 parser: {}@{}\n\
         Source V3 parser fallback: {}\n\
         Source V3 structural map SHA-256: {}\n\
         Source V3 included intervals: {}\n\
         Source V3 uncovered intervals: {}\n\
         Source V3 persistence: owner_only_source_hash_bound\n\
         Source V3 authority: read_evidence_not_control_approval_or_activation",
        checkpoint.read_session_id,
        source_map.parser.parser_kind,
        source_map.parser.parser_version,
        source_map.parser.fallback_used,
        source_map.structural_map_sha256,
        interval_text(&checkpoint.included_intervals),
        interval_text(&checkpoint.uncovered_intervals),
    )
}

pub(super) fn prompt_context_v3(
    source_map: &SourceMapV3,
    checkpoint: &ReadSessionCheckpointV3,
) -> String {
    let mut lines = vec![
        "\n// --- Persistent Source Map V3 ---".to_string(),
        format!(
            "// parser={}@{} fallback={} map_sha256={}",
            source_map.parser.parser_kind,
            source_map.parser.parser_version,
            source_map.parser.fallback_used,
            source_map.structural_map_sha256
        ),
        format!(
            "// checkpoint={} source_sha256={} included={} uncovered={}",
            checkpoint.read_session_id,
            source_map.source_sha256,
            interval_text(&checkpoint.included_intervals),
            interval_text(&checkpoint.uncovered_intervals)
        ),
        "// Binding claim rule: every absence claim needs an exact ClaimSupportRefV2 \
         from a whole-source challenge; every temporal new-implementation claim needs \
         change evidence in addition to current-source presence."
            .to_string(),
        "// Structural map entries:".to_string(),
    ];
    lines.extend(
        source_map
            .entries
            .iter()
            .take(MAX_PROMPT_ENTRIES)
            .map(|entry| {
                let span = match (entry.start_line, entry.end_line) {
                    (Some(start), Some(end)) => format!("L{start}-{end}"),
                    (Some(start), None) => format!("L{start}-?"),
                    _ => "span=parser_unavailable".to_string(),
                };
                format!(
                    "//   {span} [{}] {} ({})",
                    entry.kind, entry.label, entry.structural_path
                )
            }),
    );
    if source_map.observed_entry_count > MAX_PROMPT_ENTRIES {
        lines.push(format!(
            "//   ... {} additional mapped entries omitted; the digest binds the persisted map",
            source_map
                .observed_entry_count
                .saturating_sub(MAX_PROMPT_ENTRIES)
        ));
    }
    if let Some(reason) = source_map.parser.fallback_reason.as_deref() {
        lines.push(format!("// parser fallback disclosure: {reason}"));
    }
    lines.join("\n")
}

fn interval_text(intervals: &[SourceIntervalV3]) -> String {
    if intervals.is_empty() {
        return "none".to_string();
    }
    intervals
        .iter()
        .map(|interval| format!("{}-{}", interval.start_line, interval.end_line))
        .collect::<Vec<_>>()
        .join(",")
}
