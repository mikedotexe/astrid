use super::*;
use crate::PublicWebErrorCode as E;
use crate::spending::reserve_at;

fn api_policy() -> BraveSearchPolicyV1 {
    BraveSearchPolicyV1 {
        schema: "volition.brave_search.v1".into(),
        account_id: "synthetic-shared-account".into(),
        monthly_limit_microusd: 25_000_000,
        request_cost_microusd: BRAVE_REQUEST_MICROUSD,
        storage_rights_status: astrid_minime_protocol::public_web::SearchStorageRights::Unverified,
        retention_review_audit_ref: "synthetic-operator-review".into(),
    }
}

fn fixture() -> Fixture {
    use std::os::unix::fs::PermissionsExt as _;
    let mut fixture = Fixture::new();
    let root = fixture.config.policy_path.parent().unwrap();
    let account_directory = root.join("brave-account");
    std::fs::create_dir(&account_directory).unwrap();
    std::fs::set_permissions(&account_directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let credential_file = root.join("synthetic-key");
    std::fs::write(&credential_file, b"synthetic-credential-not-real\n").unwrap();
    std::fs::set_permissions(&credential_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let policy = api_policy();
    initialize_search_account(
        &account_directory,
        &policy.account_id,
        policy.monthly_limit_microusd,
    )
    .unwrap();
    fixture.config.schema = CONFIG_SCHEMA_V3.into();
    fixture.config.brave = Some(BraveSearchConfig {
        credential_file,
        account_directory,
    });
    fixture.policy.schema = PUBLIC_WEB_POLICY_SCHEMA_V3.into();
    fixture.policy.search_api = Some(policy);
    fixture.token = CapabilityToken::create(
        ResourcePattern::exact(PUBLIC_WEB_CAPABILITY_RESOURCE_V3).unwrap(),
        vec![Permission::Read],
        TokenScope::Persistent,
        fixture.issuer.key_id(),
        AuditEntryId::new(),
        &fixture.issuer,
        None,
    );
    fixture.policy.capability_token_id = fixture.token.id.to_string();
    fixture.save();
    fixture
}

fn search() -> PublicWebRequest {
    PublicWebRequest {
        schema: REQUEST_SCHEMA.into(),
        operation: PublicWebOperationV2::Search,
        target: "CONTROL Vec<T> amplitude".into(),
    }
}

fn api_response(request: &PublicWebRequest, results: &serde_json::Value) -> HopResponse {
    let bytes = serde_json::to_vec(
        &json!({"type":"search", "query":{"original":request.target},
        "web":{"results":results}}),
    )
    .unwrap();
    let mut result = response(200, None, &bytes);
    result.content_type = Some("application/json".into());
    result
}

#[tokio::test]
async fn authenticated_search_normalizes_candidates_and_reserves_shared_cost() {
    let fixture = fixture();
    let request = search();
    let backend = FakeBackend::new(vec![api_response(
        &request,
        &json!([
            {"title":"Synthetic source", "url":"https://example.com/a", "description":"Literal <T> data"}
        ]),
    )]);
    let result = fetch_with(&fixture.config, &request, &fixture.config, &backend)
        .await
        .unwrap();
    assert_eq!(result.content_type, crate::brave::SEARCH_MEDIA);
    let body: serde_json::Value = serde_json::from_slice(&result.body).unwrap();
    assert_eq!(body["query"], request.target);
    assert_eq!(body["results"][0]["snippet"], "Literal <T> data");
    assert_eq!(result.receipt.body_sha256, digest(&result.body));
    assert!(result.receipt.complete);
    let sent = backend.sent.lock().unwrap();
    let url = Url::parse(&sent[0].0).unwrap();
    assert!(
        url.query_pairs()
            .any(|(key, value)| key == "q" && value == request.target)
    );
    assert!(
        url.query_pairs()
            .any(|(key, value)| key == "spellcheck" && value == "false")
    );
    let config = fixture.config.brave.as_ref().unwrap();
    let status =
        search_account_status(&config.account_directory, &api_policy().account_id).unwrap();
    assert_eq!(status.reserved_microusd, 5_000);
    assert_eq!(status.monthly_limit_microusd, 25_000_000);
}

#[tokio::test]
async fn no_results_is_success_but_errors_malformed_and_query_drift_are_not() {
    let fixture = fixture();
    let request = search();
    let backend = FakeBackend::new(vec![api_response(&request, &json!([]))]);
    let result = fetch_with(&fixture.config, &request, &fixture.config, &backend)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&result.body).unwrap()["results"],
        json!([])
    );
    for body in [
        b"{}".as_slice(),
        b"{\"type\":\"error\"}",
        b"{\"type\":\"search\",\"query\":{\"original\":\"other\"},\"web\":{\"results\":[]}}",
        b"{\"type\":\"search\",\"query\":{\"original\":\"CONTROL Vec<T> amplitude\",\"more_results_available\":true}}",
    ] {
        assert_eq!(
            crate::brave::normalize(body, &request.target).unwrap_err(),
            E::SearchResponseInvalid
        );
    }
    let zero = crate::brave::normalize(
        b"{\"type\":\"search\",\"query\":{\"original\":\"CONTROL Vec<T> amplitude\"},\"web\":null}",
        &request.target,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&zero).unwrap()["results"],
        json!([])
    );
}

#[tokio::test]
async fn api_redirects_and_failures_never_retry_or_forward_credentials() {
    let fixture = fixture();
    for (status, code) in [
        (302, E::InvalidRedirect),
        (401, E::SearchAuthentication),
        (403, E::SearchAuthentication),
        (429, E::SearchRateLimited),
        (500, E::HttpStatus),
    ] {
        let backend = FakeBackend::new(vec![response(
            status,
            Some("https://example.com/next"),
            b"error",
        )]);
        let error = fetch_with(&fixture.config, &search(), &fixture.config, &backend)
            .await
            .err()
            .unwrap();
        assert_eq!(error.code, code);
        assert_eq!(backend.sent.lock().unwrap().len(), 1);
        let wire = serde_json::to_string(&error).unwrap();
        assert!(!wire.contains("synthetic-credential"));
        assert!(!wire.contains("CONTROL"));
    }
    let config = fixture.config.brave.as_ref().unwrap();
    assert_eq!(
        search_account_status(&config.account_directory, &api_policy().account_id)
            .unwrap()
            .reserved_requests,
        5
    );
}

#[tokio::test]
async fn page_reads_never_require_api_key_or_spend_and_legacy_grants_cannot_enable_api() {
    let mut fixture = fixture();
    std::fs::remove_file(&fixture.config.brave.as_ref().unwrap().credential_file).unwrap();
    let backend = FakeBackend::new(vec![response(200, None, b"ordinary page")]);
    assert!(
        fetch_with(&fixture.config, &request(), &fixture.config, &backend)
            .await
            .is_ok()
    );
    let denied = fetch_with(&fixture.config, &search(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(denied.code, E::CredentialUnavailable);
    fixture.policy.schema = PUBLIC_WEB_POLICY_SCHEMA_V2.into();
    fixture.save();
    assert!(preview(&fixture.config).is_err());
}

#[test]
fn query_limits_are_explicit_and_unicode_is_not_silently_shortened() {
    let query = "\u{1f642}".repeat(600);
    let url = crate::brave::request_url(&query).unwrap();
    assert!(
        url.query_pairs()
            .any(|(key, value)| key == "q" && value == query)
    );
    assert!(crate::brave::request_url(&"x".repeat(601)).is_err());
    assert!(crate::brave::request_url(&vec!["word"; 76].join(" ")).is_err());
}

#[test]
fn api_headers_negotiate_json_and_credentials_cannot_target_pages() {
    let fixture = fixture();
    let credential = crate::brave::credential(fixture.config.brave.as_ref().unwrap()).unwrap();
    let client = reqwest::Client::new();
    let url = crate::brave::request_url("synthetic query").unwrap();
    let api = crate::network::http_request(&client, &url, Some(&credential)).unwrap();
    assert_eq!(api.headers()["accept"], "application/json");
    assert!(api.headers()["x-subscription-token"].is_sensitive());
    let page_url = public_url("https://example.com/page").unwrap();
    assert!(crate::network::http_request(&client, &page_url, Some(&credential)).is_err());
    let page = crate::network::http_request(&client, &page_url, None).unwrap();
    assert!(!page.headers().contains_key("x-subscription-token"));
}

#[test]
fn monthly_limit_is_shared_conservative_and_never_reinitialized_after_loss() {
    let fixture = fixture();
    let path = &fixture.config.brave.as_ref().unwrap().account_directory;
    let mut policy = api_policy();
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("spending.json")).unwrap()).unwrap();
    let month = state["last_month"].as_str().unwrap().to_string();
    state["months"][&month] = 4999.into();
    write_private(&path.join("spending.json"), &state);
    reserve_at(path, &policy, &month).unwrap();
    assert_eq!(
        reserve_at(path, &policy, &month).unwrap_err(),
        E::SearchSpendingLimit
    );
    policy.account_id = "different-account".into();
    assert_eq!(
        reserve_at(path, &policy, &month).unwrap_err(),
        E::SearchSpendingState
    );
    policy = api_policy();
    assert_eq!(
        reserve_at(path, &policy, "2000-01").unwrap_err(),
        E::SearchSpendingState
    );
    reserve_at(path, &policy, "2099-01").unwrap();
    assert_eq!(
        reserve_at(path, &policy, &month).unwrap_err(),
        E::SearchSpendingState
    );
    std::fs::remove_file(path.join("spending.json")).unwrap();
    assert!(
        initialize_search_account(path, &policy.account_id, policy.monthly_limit_microusd).is_err()
    );
}

#[test]
fn corrupt_cost_history_and_conflicting_configuration_fail_closed() {
    let fixture = fixture();
    let config = fixture.config.brave.as_ref().unwrap();
    assert!(
        initialize_search_account(&config.account_directory, &api_policy().account_id, 10_000)
            .is_err()
    );
    std::fs::write(config.account_directory.join("spending.json"), b"{broken").unwrap();
    assert!(search_account_status(&config.account_directory, &api_policy().account_id).is_err());
    assert!(
        initialize_search_account(
            &config.account_directory,
            &api_policy().account_id,
            25_000_000
        )
        .is_err()
    );
}

#[tokio::test]
async fn operator_check_returns_only_counts_and_cannot_authorize_or_retain_results() {
    let fixture = fixture();
    let config = fixture.config.brave.as_ref().unwrap();
    let request = PublicWebRequest {
        target: "Brave Search API documentation".into(),
        ..search()
    };
    let backend = FakeBackend::new(vec![api_response(
        &request,
        &json!([
            {"title":"private-marker-title", "url":"https://example.com/private-marker-url", "description":"private-marker-snippet"}
        ]),
    )]);
    let before_policy = std::fs::read(&fixture.config.policy_path).unwrap();
    let result = crate::transport::check_brave_with(config, &api_policy().account_id, &backend)
        .await
        .unwrap();
    let result = serde_json::to_string(&result).unwrap();
    assert!(result.contains("\"authenticated\":true"));
    assert!(result.contains("\"result_count\":1"));
    assert!(!result.contains("private-marker"));
    assert_eq!(
        std::fs::read(&fixture.config.policy_path).unwrap(),
        before_policy
    );
    assert!(
        !std::fs::read_to_string(config.account_directory.join("spending.json"))
            .unwrap()
            .contains("private-marker")
    );
    assert_eq!(
        search_account_status(&config.account_directory, &api_policy().account_id)
            .unwrap()
            .reserved_requests,
        1
    );
}

#[tokio::test]
async fn operator_check_preserves_http_failure_status_without_server_prose() {
    let fixture = fixture();
    let config = fixture.config.brave.as_ref().unwrap();
    let backend = FakeBackend::new(vec![response(402, None, b"private-upstream-error-marker")]);
    let error = crate::transport::check_brave_with(config, &api_policy().account_id, &backend)
        .await
        .unwrap_err();
    assert_eq!(error.http_status, Some(402));
    assert_eq!(error.code, E::HttpStatus);
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("private-upstream-error-marker")
    );
}

#[test]
fn concurrent_account_reservations_cannot_exceed_shared_ceiling() {
    let fixture = fixture();
    let path = fixture
        .config
        .brave
        .as_ref()
        .unwrap()
        .account_directory
        .clone();
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("spending.json")).unwrap()).unwrap();
    let month = state["last_month"].as_str().unwrap().to_string();
    state["months"][&month] = 4999.into();
    write_private(&path.join("spending.json"), &state);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            let month = month.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                reserve_at(&path, &api_policy(), &month)
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .all(|result| matches!(result, Ok(()) | Err(E::OwnerBusy | E::SearchSpendingLimit)))
    );
    assert_eq!(
        search_account_status(&path, &api_policy().account_id)
            .unwrap()
            .reserved_microusd,
        25_000_000
    );
}
