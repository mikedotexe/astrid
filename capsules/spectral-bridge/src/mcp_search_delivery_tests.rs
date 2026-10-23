use super::*;

#[test]
fn explicit_probe_query_preserves_authored_operators_and_quotes() {
    let db = BridgeDb::open(":memory:").unwrap();
    let query = "\"CONTROL Vec<T>\"  \u{03bb} - exact; & q=other";
    assert_eq!(
        probe_effective_search_query(&format!("SEARCH {query}"), &db).as_deref(),
        Some(query)
    );
    for raw in ["SEARCH Vec<T>", "Synthetic thought.\nNEXT: SEARCH Vec<T>"] {
        let action = normalize_probe_action(raw).unwrap();
        assert_eq!(
            probe_effective_search_query(&action, &db).as_deref(),
            Some("Vec<T>")
        );
    }
}

#[test]
fn empty_probe_is_success_without_research_artifact_or_reflection() {
    let live = LiveProbeContext {
        safety_level: SafetyLevel::Green,
        fill_pct: Some(68.0),
        lambda1: None,
        telemetry: None,
        fingerprint: None,
    };
    let result = probe_empty_search("SEARCH synthetic", &live, "synthetic".to_string());
    assert_eq!(result.status, "ok");
    assert_eq!(result.effective_query.as_deref(), Some("synthetic"));
    assert!(result.artifacts.is_empty());
    assert!(result.summary.contains("zero results"));
    assert!(
        result
            .experienced_text
            .contains("no reflection was generated")
    );
    assert!(
        result
            .experienced_text
            .contains("reading position is unchanged")
    );
}
