use super::*;

const CHALLENGE: &[u8] = br#"<form id="challenge-form" action="/anomaly.js">
<div class="anomaly-modal__description">Please complete the following challenge to confirm this search was made by a human.</div>
private-response-marker</form>"#;

fn search_request() -> PublicWebRequest {
    PublicWebRequest {
        operation: PublicWebOperationV2::Search,
        target: "synthetic-public-query".into(),
        ..request()
    }
}

#[tokio::test]
async fn known_challenge_is_incomplete_even_with_success_status_and_never_retried() {
    for status in [200, 202, 403] {
        let fixture = Fixture::new();
        let backend = FakeBackend::new(vec![response(status, None, CHALLENGE)]);
        let failure = fetch_with(
            &fixture.config,
            &search_request(),
            &fixture.config,
            &backend,
        )
        .await
        .err()
        .unwrap();
        assert_eq!(failure.code, PublicWebErrorCode::SearchChallenge);
        assert_eq!(failure.http_status, Some(status));
        assert!(!failure.complete);
        assert_eq!(backend.sent.lock().unwrap().len(), 1);
        let wire = serde_json::to_string(&failure).unwrap();
        assert!(!wire.contains("private-response-marker"));
        assert!(!wire.contains("synthetic-public-query"));
        let message = failure.delivery_message();
        assert!(message.contains("human-verification challenge, not search results"));
        assert!(!message.contains("private-response-marker"));
        assert!(!message.contains("synthetic-public-query"));
    }
}

#[tokio::test]
async fn accepted_without_challenge_is_not_claimed_pending_or_complete() {
    let fixture = Fixture::new();
    let backend = FakeBackend::new(vec![response(202, None, b"accepted")]);
    let failure = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &backend,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::HttpStatus);
    assert_eq!(failure.http_status, Some(202));
    assert!(
        failure
            .delivery_message()
            .contains("does not establish queued work")
    );
    assert_eq!(backend.sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn results_and_challenge_discussion_are_not_refused() {
    let fixture = Fixture::new();
    for body in [
        b"<a class=\"result__a\" href=\"https://example.org/\">Example</a>".as_slice(),
        b"<p>Discussion of challenge-form and human-verification challenges.</p>".as_slice(),
    ] {
        let result = fetch_with(
            &fixture.config,
            &search_request(),
            &fixture.config,
            &FakeBackend::new(vec![response(200, None, body)]),
        )
        .await
        .unwrap();
        assert_eq!(result.body, body);
        assert!(result.receipt.complete);
    }
}

#[tokio::test]
async fn fingerprint_is_scoped_to_search_host_and_html() {
    let fixture = Fixture::new();
    let page = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![response(200, None, CHALLENGE)]),
    )
    .await
    .unwrap();
    assert_eq!(page.body, CHALLENGE);
    let redirected = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &FakeBackend::new(vec![
            response(302, Some("https://example.com/"), b""),
            response(200, None, CHALLENGE),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(redirected.body, CHALLENGE);
    let other_path = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &FakeBackend::new(vec![
            response(302, Some("https://html.duckduckgo.com/other"), b""),
            response(200, None, CHALLENGE),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(other_path.body, CHALLENGE);
    let mut plain = response(200, None, CHALLENGE);
    plain.content_type = Some("text/plain".into());
    assert!(
        fetch_with(
            &fixture.config,
            &search_request(),
            &fixture.config,
            &FakeBackend::new(vec![plain])
        )
        .await
        .is_ok()
    );
}

#[tokio::test]
async fn invalid_headers_preserve_received_status_without_reading_body() {
    let fixture = Fixture::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.append(reqwest::header::CONTENT_TYPE, "text/html".parse().unwrap());
    headers.append(reqwest::header::CONTENT_TYPE, "text/plain".parse().unwrap());
    let hop = HopResponse::from_headers(
        202,
        &headers,
        Box::new(Chunks(VecDeque::from([Err(
            PublicWebErrorCode::IncompleteBody,
        )]))),
    );
    let failure = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &FakeBackend::new(vec![hop]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::InvalidHeaders);
    assert_eq!(failure.http_status, Some(202));
    assert_eq!(failure.wire_bytes_observed, 0);
    assert_eq!(failure.decoded_bytes_observed, 0);
}

#[tokio::test]
async fn bounds_and_authority_failures_take_precedence_over_challenge_diagnosis() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.decoded_body_bytes = 32;
    fixture.save();
    let failure = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &FakeBackend::new(vec![response(202, None, CHALLENGE)]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::BodyLimit);
    assert_eq!(failure.http_status, Some(202));
    assert!(!failure.delivery_message().contains("human-verification"));

    fixture.save_state(PublicWebPolicyStatusV2::Revoked, 2, true);
    let backend = FakeBackend::new(vec![]);
    let failure = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &backend,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::Revoked);
    assert_eq!(failure.http_status, None);
    assert!(backend.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failed_redirect_hop_does_not_borrow_prior_http_status() {
    let fixture = Fixture::new();
    let backend = FakeBackend::new(vec![response(302, Some("https://example.com/"), b"")]);
    let failure = fetch_with(
        &fixture.config,
        &search_request(),
        &fixture.config,
        &backend,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.http_status, None);
    assert!(!failure.complete);
}

#[test]
fn legacy_failure_without_http_status_remains_readable_and_unknown() {
    let failure: PublicWebFailure = serde_json::from_value(json!({
        "schema": RECEIPT_SCHEMA, "complete": false, "code": "http_status",
        "request_sha256": "a".repeat(64), "decoded_bytes_observed": 0,
        "wire_bytes_observed": 0, "redirects_followed": 0
    }))
    .unwrap();
    assert_eq!(failure.http_status, None);
    assert!(
        failure
            .delivery_message()
            .contains("http_status=unavailable")
    );
    assert!(!failure.delivery_message().contains("challenge"));
}
