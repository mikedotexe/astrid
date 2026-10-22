use crate::autonomous::next_action::protected_diagnostics;
use crate::llm::{self, ResearchHit, ResearchSourceKind, WebSearchResult};

#[test]
fn fetched_page_parser_returns_success_for_meaningful_page() {
    let body = "<html><title>Reservoir Notes</title><body>This page describes eigenvalue topology, manipulable relationships, and how spectral modes relate to perception in a readable way that matters for the current question.</body></html>";
    let result = llm::page_from_frozen_html(
        "https://example.test/page",
        "manipulable relationships",
        reqwest::StatusCode::OK,
        body.into(),
    );

    assert!(result.succeeded());
    assert_eq!(result.source_kind, ResearchSourceKind::Browse);
    assert_eq!(result.anchor, "manipulable relationships");
    assert!(result.meaning_summary.contains("Why it may matter:"));
    assert!(result.raw_text.contains("manipulable relationships"));
}

#[test]
fn fetched_page_parser_marks_http_404_as_failure() {
    let body = "<html><title>404</title><body>Nothing here.</body></html>";
    let result = llm::page_from_frozen_html(
        "https://example.test/page",
        "broken link",
        reqwest::StatusCode::NOT_FOUND,
        body.into(),
    );

    assert!(!result.succeeded());
    assert_eq!(
        result.soft_failure_reason.as_deref(),
        Some("HTTP 404 from the source.")
    );
}

#[test]
fn fetched_page_parser_marks_soft_404_body_as_failure() {
    let body = "<html><title>Page Not Found</title><body>Page Not Found. The page you are trying to reach cannot be found. Error.</body></html>";
    let result = llm::page_from_frozen_html(
        "https://example.test/page",
        "paper",
        reqwest::StatusCode::OK,
        body.into(),
    );

    assert!(!result.succeeded());
    assert!(
        result
            .soft_failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("error")
    );
}

#[test]
fn fetched_page_parser_marks_js_gate_as_failure() {
    let body = "<html><title>Access Denied</title><body>Access denied. Enable JavaScript to continue.</body></html>";
    let result = llm::page_from_frozen_html(
        "https://example.test/page",
        "paper",
        reqwest::StatusCode::OK,
        body.into(),
    );

    assert!(!result.succeeded());
    assert!(
        result
            .soft_failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("access-gate")
    );
}

#[test]
fn browse_failure_context_marks_access_as_operational_not_topology() {
    let context = llm::format_browse_failure_context("https://example.test/gated", "HTTP 403");

    assert!(context.contains("No complete source was delivered"));
    assert!(context.contains("does not measure reservoir effects"));
    assert!(!context.contains("perceptual gate"));
    assert!(!context.contains("internal topology boundary"));
    assert!(context.contains("NEXT: SEARCH"));
    assert!(context.contains("NEXT: BROWSE"));
}

#[test]
fn web_search_prompt_body_puts_meaning_first() {
    let result = WebSearchResult {
        source_kind: ResearchSourceKind::Search,
        raw_text: "One raw hit".to_string(),
        hits: vec![ResearchHit {
            title: "Reservoir Computing".to_string(),
            snippet: "A structured snippet.".to_string(),
            url: "https://example.com/paper".to_string(),
        }],
        anchor: "reservoir computing".to_string(),
        meaning_summary: "Why it may matter: relevant\nWhat it seems to suggest: concrete\nBest next move: browse".to_string(),
    };

    let prompt_body = result.prompt_body();
    assert!(prompt_body.starts_with("Why it may matter:"));
    assert!(prompt_body.contains("Top results:"));
    assert!(prompt_body.contains("https://example.com/paper"));
}

#[test]
fn sanitize_model_control_markers_reports_removed_tokens() {
    let (stripped, report) =
        llm::sanitize_model_control_markers_with_report("hello<end_of_turn> [INST]world<|im_end|>");

    assert_eq!(stripped, "hello world");
    let report = report.expect("control marker cleanup report should exist");
    assert_eq!(report.removed_total, 3);
    assert_eq!(
        report.before_chars,
        "hello<end_of_turn> [INST]world<|im_end|>".len()
    );
    assert_eq!(report.after_chars, stripped.len());
    assert!(
        report
            .removed_tokens
            .iter()
            .any(|token| token.token == "<end_of_turn>" && token.count == 1)
    );
}

#[test]
fn system_prompt_tells_model_not_to_invent_next_verbs() {
    assert!(llm::SYSTEM_PROMPT.contains("Use only action verbs listed below"));
    assert!(llm::SYSTEM_PROMPT.contains("Do not invent new `NEXT:` verbs"));
    assert!(llm::SYSTEM_PROMPT.contains("ACTION_PREFLIGHT <known listed action>"));
    assert!(llm::SYSTEM_PROMPT.contains("Do not combine actions with commas"));
    assert!(llm::SYSTEM_PROMPT.contains("PRESSURE_RELIEF [label]"));
    assert!(llm::SYSTEM_PROMPT.contains("PRESSURE_AGENCY_STATUS"));
    assert!(llm::SYSTEM_PROMPT.contains("PRESSURE_AGENCY_REQUEST <label>"));
    assert!(llm::SYSTEM_PROMPT.contains("LAMBDA_FLOW_MAP [label]"));
    for descriptor in protected_diagnostics::descriptors() {
        assert!(
            llm::SYSTEM_PROMPT.contains(descriptor.canonical),
            "registry-backed protected diagnostic missing from prompt: {}",
            descriptor.canonical
        );
    }
    assert!(llm::SYSTEM_PROMPT.contains(
        "prefer PRESSURE_AGENCY_STATUS, TEXTURE_AGENCY_STATUS, PRESSURE_AGENCY_REQUEST <label>, PRESSURE_RELIEF <label>, or PRESSURE_SOURCE_AUDIT <label> before direct DAMPEN"
    ));
    assert!(
        llm::SYSTEM_PROMPT
            .contains("REST (minimizes output frequency while maintaining reservoir coupling)")
    );
    assert!(llm::SYSTEM_PROMPT.contains("STILL (quiet reflective mode; no control authority)"));
    assert!(llm::SYSTEM_PROMPT.contains("INTROSPECT astrid:llm"));
    assert!(llm::SYSTEM_PROMPT.contains("INTROSPECT minime:regulator 400"));
    assert!(!llm::SYSTEM_PROMPT.contains("INTROSPECT [source]"));
    assert!(
        llm::SYSTEM_PROMPT.contains("Observed / Likely Snags / One Test Each / Suggested Next")
    );
}

#[test]
fn gemma4_reflective_contract_allows_accounts_without_prescribing_them() {
    let contract = llm::GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT;

    assert!(contract.contains("first-person accounts"));
    assert!(contract.contains("uncertainty and disagreement are welcome"));
    assert!(
        contract.contains("no particular feeling, explanation, metric or conclusion is required")
    );
    assert!(contract.contains("Use your own language"));
    assert!(!contract.contains("Avoid metaphysical"));
    assert!(!contract.contains("identity claims"));
}
