//! Scope for lexical diagnostics, not action admission or measured reservoir state.

use serde_json::{Value, json};

pub(super) fn with_retrospective_scope(mut cue: Value, summary_fallback: bool) -> Value {
    cue["source_selection_v1"] = json!({
        "journal_candidate_limit": 120,
        "journal_match_limit": 3,
        "event_match_limit": 3,
        "retained_source_limit": 4,
        "match_required": true,
        "evidence_sufficiency_evaluated": false,
        "journal_order": "filesystem_modification_time_descending",
        "selection": "lexical_matches_only",
        "summary_fallback": summary_fallback,
        "unmatched_history_evaluated_for_success": false
    });
    cue
}

pub(super) fn line(cue: &Option<Value>, subject: &str) -> String {
    let Some(cue) = cue else { return String::new() };
    let count = cue
        .get("source_refs")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    // Legacy projections can be inspected, but their generated commands are not advice.
    format!(
        "Selected-language diagnostic ({subject}): {count} retained source(s); lexical matches, not representative history or measured state. Interpretation and occurrence rate unverified.\n"
    )
}

pub(super) fn retired_lexical_session_draft(record: &Value) -> bool {
    record["record_schema"] == "continuity_session_v1"
        && record["record_type"] == "session_draft"
        && record["status"] == "draft"
        && record["draft_v1"] == true
        && record["ignored_until_accepted"] == true
        && matches!(
            record["reason"].as_str(),
            Some("single_motif_overfit_risk" | "map_release_before_intervening")
        )
}
