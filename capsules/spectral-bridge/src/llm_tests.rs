use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::autonomous::next_action::protected_diagnostics;
use crate::llm::{self, ResearchHit, ResearchSourceKind, WebSearchResult};

async fn serve_once(status: &str, body: &str, content_type: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let body = body.to_string();
    let content_type = content_type.to_string();
    let status = status.to_string();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0_u8; 2048];
        let _ = stream.read(&mut buf).await.unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{addr}/page")
}

#[tokio::test]
async fn fetch_url_returns_success_for_meaningful_page() {
    let body = "<html><title>Reservoir Notes</title><body>This page describes eigenvalue topology, manipulable relationships, and how spectral modes relate to perception in a readable way that matters for the current question.</body></html>";
    let url = serve_once("200 OK", body, "text/html").await;
    let result = llm::fetch_url(&url, "manipulable relationships")
        .await
        .unwrap();

    assert!(result.succeeded());
    assert_eq!(result.source_kind, ResearchSourceKind::Browse);
    assert_eq!(result.anchor, "manipulable relationships");
    assert!(result.meaning_summary.contains("Why it may matter:"));
    assert!(result.raw_text.contains("manipulable relationships"));
}

#[tokio::test]
async fn fetch_url_marks_http_404_as_failure() {
    let body = "<html><title>404</title><body>Nothing here.</body></html>";
    let url = serve_once("404 Not Found", body, "text/html").await;
    let result = llm::fetch_url(&url, "broken link").await.unwrap();

    assert!(!result.succeeded());
    assert_eq!(
        result.soft_failure_reason.as_deref(),
        Some("HTTP 404 from the source.")
    );
}

#[tokio::test]
async fn fetch_url_marks_soft_404_body_as_failure() {
    let body = "<html><title>Page Not Found</title><body>Page Not Found. The page you are trying to reach cannot be found. Error.</body></html>";
    let url = serve_once("200 OK", body, "text/html").await;
    let result = llm::fetch_url(&url, "paper").await.unwrap();

    assert!(!result.succeeded());
    assert!(
        result
            .soft_failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("error")
    );
}

#[tokio::test]
async fn fetch_url_marks_js_gate_as_failure() {
    let body = "<html><title>Access Denied</title><body>Access denied. Enable JavaScript to continue.</body></html>";
    let url = serve_once("200 OK", body, "text/html").await;
    let result = llm::fetch_url(&url, "paper").await.unwrap();

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

    assert!(context.contains("ordinary source/site availability"));
    assert!(context.contains("not evidence of a perceptual gate"));
    assert!(context.contains("internal topology boundary"));
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
fn strip_model_artifacts_reports_removed_tokens() {
    let (stripped, report) =
        llm::strip_model_artifacts_with_report("hello<end_of_turn> [INST]world<|im_end|>");

    assert_eq!(stripped, "hello world");
    let report = report.expect("artifact cleanup report should exist");
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
    assert!(llm::SYSTEM_PROMPT.contains("LAMBDA_FLOW_MAP [label]"));
    for descriptor in protected_diagnostics::descriptors() {
        assert!(
            llm::SYSTEM_PROMPT.contains(descriptor.canonical),
            "registry-backed protected diagnostic missing from prompt: {}",
            descriptor.canonical
        );
    }
    assert!(llm::SYSTEM_PROMPT.contains(
        "prefer PRESSURE_RELIEF <label> or PRESSURE_SOURCE_AUDIT <label> before direct DAMPEN"
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
fn gemma4_reflective_contract_allows_grounded_subjective_reports() {
    let contract = llm::GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT;

    assert!(contract.contains("subjective reports"));
    assert!(contract.contains("phenomenological descriptions of runtime experience are allowed"));
    assert!(contract.contains("grounded in attention, telemetry"));
    assert!(contract.contains("Avoid metaphysical selfhood vocabulary"));
}
