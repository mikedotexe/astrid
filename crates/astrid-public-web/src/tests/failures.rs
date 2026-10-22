use std::sync::Arc;

use super::*;

#[test]
fn python_adapter_request_and_owner_hash_contract_is_exact() {
    let request = PublicWebRequest {
        schema: REQUEST_SCHEMA.into(),
        operation: PublicWebOperationV2::Search,
        target: "\u{03bb} & q=other ; $(fixture)\n\"quoted\"".into(),
    };
    assert_eq!(
        request.sha256(),
        "ae878b570a047687447d4e05b59d4109d9c93da71ea0e774a34639e364420161"
    );
    let owner = PublicWebOwnerV2 {
        being: "minime".into(),
        principal: "minime".into(),
        public_key_hex: "a".repeat(64),
        deployment_identity: "fixture".into(),
    };
    assert_eq!(
        owner.lock_identity_sha256(),
        "7829af495100bf9be54d51051b341c14b5f08a6a810df0581a743cb4de631f89"
    );
}

#[tokio::test]
async fn pdf_payload_preserves_bytes_under_the_same_body_limit() {
    let fixture = Fixture::new();
    let pdf = b"%PDF-1.7\n\xff\x00\n%%EOF";
    let mut hop = response(200, None, pdf);
    hop.content_type = Some("application/pdf".into());
    let result = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![hop]),
    )
    .await
    .unwrap();
    assert_eq!(result.body, pdf);
    assert_eq!(result.content_type, "application/pdf");
    assert_eq!(result.receipt.body_sha256, digest(pdf));
    assert!(result.receipt.complete);
}

#[tokio::test]
async fn public_api_rejects_stale_runtime_identity_before_loading_authority_or_network() {
    let fixture = Fixture::new();
    let mut runtime_owner = fixture.config.expected_owner.clone();
    runtime_owner.deployment_identity = "other-loaded-image".into();
    std::fs::remove_file(&fixture.config.policy_path).unwrap();
    let failure = fetch(&fixture.config, &runtime_owner, &request())
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::OwnerMismatch);
    assert_eq!(
        std::fs::read_dir(&fixture.config.owner_lock_dir)
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn query_at_sign_is_data_not_credentials() {
    assert!(public_url("https://example.com?author=user@example.org").is_ok());
    assert!(public_url("https://user@example.com").is_err());
    assert!(public_url("https://@example.com").is_err());
}

struct ChangeStateBody {
    path: std::path::PathBuf,
    replacement: PublicWebPolicyStateV2,
}

#[async_trait]
impl Body for ChangeStateBody {
    async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        write_private(&self.path, &self.replacement);
        Ok(Some(b"content-must-not-escape".to_vec()))
    }
}

#[tokio::test]
async fn revocation_during_body_read_discards_partial_payload_and_releases_lease() {
    let fixture = Fixture::new();
    let active = std::fs::read(&fixture.config.state_path).unwrap();
    let active_floor = std::fs::read(&fixture.config.state_floor_path).unwrap();
    fixture.save_state(PublicWebPolicyStatusV2::Revoked, 2, true);
    let revoked =
        serde_json::from_slice(&std::fs::read(&fixture.config.state_path).unwrap()).unwrap();
    std::fs::write(&fixture.config.state_path, active).unwrap();
    std::fs::write(&fixture.config.state_floor_path, active_floor).unwrap();
    let mut hop = response(200, None, b"");
    hop.body = Box::new(ChangeStateBody {
        path: fixture.config.state_path.clone(),
        replacement: revoked,
    });
    let failure = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![hop]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::Revoked);
    assert!(!failure.complete);
    assert!(
        !serde_json::to_string(&failure)
            .unwrap()
            .contains("content-must-not-escape")
    );
    assert!(
        crate::guard::OwnerLease::acquire(
            &fixture.config.owner_lock_dir,
            &fixture.policy.owner.lock_identity_sha256()
        )
        .is_ok()
    );
}

struct WaitingBody {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl Body for WaitingBody {
    async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(None)
    }
}

#[tokio::test]
async fn concurrent_fetches_for_one_owner_cannot_both_reach_transport() {
    let fixture = Fixture::new();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let mut hop = response(200, None, b"");
    hop.body = Box::new(WaitingBody {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let first = FakeBackend::new(vec![hop]);
    let second = FakeBackend::new(vec![]);
    let request = request();
    let (first_result, second_result) = tokio::join!(
        fetch_with(&fixture.config, &request, &fixture.config, &first),
        async {
            entered.notified().await;
            let result = fetch_with(&fixture.config, &request, &fixture.config, &second).await;
            release.notify_one();
            result
        }
    );
    assert!(first_result.is_ok());
    assert_eq!(
        second_result.err().unwrap().code,
        PublicWebErrorCode::OwnerBusy
    );
    assert!(second.sent.lock().unwrap().is_empty());
}

#[test]
fn abrupt_child_exit_releases_kernel_lock_without_cleanup_authority() {
    let fixture = Fixture::new();
    let owner = fixture.policy.owner.lock_identity_sha256();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "tests::owner_lock_child", "--ignored"])
        .env("PUBLIC_WEB_TEST_LOCK_DIR", &fixture.config.owner_lock_dir)
        .env("PUBLIC_WEB_TEST_LOCK_OWNER", &owner)
        .env("PUBLIC_WEB_TEST_ABRUPT_EXIT", "1")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(17));
    assert!(crate::guard::OwnerLease::acquire(&fixture.config.owner_lock_dir, &owner).is_ok());
}

struct SlowBody(Duration);

#[async_trait]
impl Body for SlowBody {
    async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        tokio::time::sleep(self.0).await;
        Ok(None)
    }
}

#[tokio::test(start_paused = true)]
async fn stalled_body_preserves_tighter_hop_timeout() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.page_hop_timeout_ms = 10;
    fixture.save();
    let mut hop = response(200, None, b"");
    hop.body = Box::new(SlowBody(Duration::from_millis(11)));
    let failure = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![hop]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::Deadline);
}

#[tokio::test(start_paused = true)]
async fn redirects_do_not_reset_logical_deadline() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.total_timeout_ms = 15;
    fixture.policy.limits.page_hop_timeout_ms = 10;
    fixture.save();
    let mut backend = FakeBackend::new(vec![
        response(302, Some("/next"), b""),
        response(200, None, b"done"),
    ]);
    backend.delay = Duration::from_millis(9);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::Deadline);
    assert_eq!(backend.sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn exact_four_mib_succeeds_and_one_more_byte_is_incomplete() {
    let fixture = Fixture::new();
    let body = vec![b'x'; usize::try_from(PUBLIC_WEB_MAX_BODY_BYTES).unwrap()];
    let result = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![response(200, None, &body)]),
    )
    .await
    .unwrap();
    assert_eq!(
        result.receipt.decoded_bytes_observed,
        PUBLIC_WEB_MAX_BODY_BYTES
    );
    let mut hop = response(200, None, b"");
    hop.body = Box::new(Chunks(VecDeque::from([Ok(body), Ok(vec![b'x'])])));
    let failure = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![hop]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::BodyLimit);
    assert!(!failure.complete);
}

#[tokio::test]
async fn truncated_stream_unsupported_encoding_media_and_partial_status_are_not_success() {
    let fixture = Fixture::new();
    let mut broken = response(200, None, b"");
    broken.body = Box::new(Chunks(VecDeque::from([
        Ok(b"partial-secret".to_vec()),
        Err(PublicWebErrorCode::IncompleteBody),
    ])));
    let mut encoding = response(200, None, b"partial-secret");
    encoding.encoding = "br".into();
    let mut media = response(200, None, b"partial-secret");
    media.content_type = Some("application/octet-stream".into());
    for (hop, code) in [
        (broken, PublicWebErrorCode::IncompleteBody),
        (encoding, PublicWebErrorCode::UnsupportedEncoding),
        (media, PublicWebErrorCode::UnsupportedMedia),
        (
            response(206, None, b"partial-secret"),
            PublicWebErrorCode::HttpStatus,
        ),
        (
            response(500, None, b"partial-secret"),
            PublicWebErrorCode::HttpStatus,
        ),
    ] {
        let failure = fetch_with(
            &fixture.config,
            &request(),
            &fixture.config,
            &FakeBackend::new(vec![hop]),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(failure.code, code);
        let wire = serde_json::to_string(&failure).unwrap();
        assert!(!wire.contains("partial-secret"));
        assert!(!wire.contains("private-query-marker"));
        assert!(!failure.complete);
    }
}

#[tokio::test]
async fn missing_redirect_target_and_wrong_dns_port_are_rejected() {
    let fixture = Fixture::new();
    let failure = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![response(302, None, b"")]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::InvalidRedirect);
    let backend = FakeBackend::new(vec![]);
    backend
        .dns
        .lock()
        .unwrap()
        .push_back(vec!["1.1.1.1:80".parse().unwrap()]);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::UnsafeAddress);
    assert!(backend.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn search_target_is_encoded_once_and_requires_search_permission() {
    let mut fixture = Fixture::new();
    let mut search = request();
    search.operation = PublicWebOperationV2::Search;
    search.target = "spectral mode & q=other".into();
    let backend = FakeBackend::new(vec![response(200, None, b"results")]);
    fetch_with(&fixture.config, &search, &fixture.config, &backend)
        .await
        .unwrap();
    {
        let sent = backend.sent.lock().unwrap();
        let url = Url::parse(&sent[0].0).unwrap();
        assert_eq!(url.host_str(), Some("html.duckduckgo.com"));
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![("q".into(), search.target.clone().into())]
        );
    }
    fixture.policy.operations = vec![PublicWebOperationV2::Page];
    fixture.save();
    let backend = FakeBackend::new(vec![]);
    let failure = fetch_with(&fixture.config, &search, &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::OperationNotGranted);
    assert!(backend.sent.lock().unwrap().is_empty());
}

#[test]
fn single_use_and_session_tokens_cannot_grant_standing_access() {
    for (scope, single_use) in [(TokenScope::Session, false), (TokenScope::Persistent, true)] {
        let mut fixture = Fixture::new();
        fixture.token = CapabilityToken::create_with_options(
            ResourcePattern::exact(PUBLIC_WEB_CAPABILITY_RESOURCE_V2).unwrap(),
            vec![Permission::Read],
            scope,
            fixture.issuer.key_id(),
            AuditEntryId::new(),
            &fixture.issuer,
            None,
            single_use,
        );
        fixture.policy.capability_token_id = fixture.token.id.to_string();
        fixture.save();
        assert_eq!(
            preview(&fixture.config).unwrap_err(),
            PublicWebErrorCode::InvalidCapability
        );
    }
}

#[test]
fn malformed_policy_nulls_unknown_operations_and_extra_fields_are_not_authority() {
    let fixture = Fixture::new();
    for field in ["owner", "limits", "signature_hex"] {
        let mut policy = serde_json::to_value(&fixture.policy).unwrap();
        policy[field] = serde_json::Value::Null;
        write_private(&fixture.config.policy_path, &policy);
        assert_eq!(
            preview(&fixture.config).unwrap_err(),
            PublicWebErrorCode::InvalidPolicy
        );
    }
    let mut policy = serde_json::to_value(&fixture.policy).unwrap();
    policy["operations"] = json!(["live_control"]);
    write_private(&fixture.config.policy_path, &policy);
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::InvalidPolicy
    );
    let mut policy = serde_json::to_value(&fixture.policy).unwrap();
    policy["action_count"] = json!(u64::MAX);
    write_private(&fixture.config.policy_path, &policy);
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::InvalidPolicy
    );
}

#[tokio::test]
async fn fetching_does_not_write_policy_token_usage_or_retained_body() {
    let fixture = Fixture::new();
    let policy_before = std::fs::read(&fixture.config.policy_path).unwrap();
    let state_before = std::fs::read(&fixture.config.state_path).unwrap();
    let token_path = fixture
        .config
        .delegation_root
        .join("tokens")
        .join(format!("{}.json", fixture.token.id));
    let token_before = std::fs::read(&token_path).unwrap();
    fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![response(200, None, b"body")]),
    )
    .await
    .unwrap();
    assert_eq!(
        policy_before,
        std::fs::read(&fixture.config.policy_path).unwrap()
    );
    assert_eq!(
        state_before,
        std::fs::read(&fixture.config.state_path).unwrap()
    );
    assert_eq!(token_before, std::fs::read(token_path).unwrap());
    assert!(!fixture.config.delegation_root.join("usage").exists());
    assert_eq!(
        std::fs::read_dir(&fixture.config.storage_path)
            .unwrap()
            .count(),
        0
    );
}
