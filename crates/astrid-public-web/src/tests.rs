use std::collections::VecDeque;
use std::io::Write as _;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrid_capabilities::{AuditEntryId, CapabilityToken, ResourcePattern, TokenScope};
use astrid_core::Permission;
use astrid_crypto::KeyPair;
use astrid_minime_protocol::public_web::*;
use async_trait::async_trait;
use serde_json::json;
use url::Url;

use super::*;
use crate::authority::Authority as _;
use crate::network::{Backend, Body, HopResponse, public_ip, public_url};
use crate::transport::fetch_with;

mod failures;
mod operator;

struct Fixture {
    _root: tempfile::TempDir,
    config: PublicWebConfig,
    policy: PublicWebPolicyV2,
    issuer: KeyPair,
    owner: KeyPair,
    token: CapabilityToken,
}

fn write_private(path: &Path, value: &impl Serialize) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["delegation", "delegation/tokens", "locks", "storage"] {
            std::fs::create_dir(path.join(name)).unwrap();
            std::fs::set_permissions(path.join(name), std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        let issuer = KeyPair::generate();
        let owner = KeyPair::generate();
        let token = CapabilityToken::create(
            ResourcePattern::exact(PUBLIC_WEB_CAPABILITY_RESOURCE_V2).unwrap(),
            vec![Permission::Read],
            TokenScope::Persistent,
            issuer.key_id(),
            AuditEntryId::new(),
            &issuer,
            None,
        );
        let now = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let policy = PublicWebPolicyV2 {
            schema: PUBLIC_WEB_POLICY_SCHEMA_V2.into(),
            policy_id: "fixture-policy".into(),
            revision: 1,
            owner: PublicWebOwnerV2 {
                being: "astrid".into(),
                principal: "astrid".into(),
                public_key_hex: owner.export_public_key().to_hex(),
                deployment_identity: "fixture-deployment".into(),
            },
            capability_token_id: token.id.to_string(),
            capability_token_sha256: String::new(),
            issuer_public_key_hex: issuer.export_public_key().to_hex(),
            operations: vec![PublicWebOperationV2::Search, PublicWebOperationV2::Page],
            limits: PublicWebLimitsV2::default(),
            issued_at_unix_ms: now,
            expires_at_unix_ms: now.saturating_add(60_000),
            approval_audit_refs: vec!["fixture:approval".into()],
            signature_hex: String::new(),
        };
        let config = PublicWebConfig {
            schema: CONFIG_SCHEMA.into(),
            delegation_root: path.join("delegation"),
            policy_path: path.join("policy.json"),
            state_path: path.join("state.json"),
            state_floor_path: path.join("state-floor.json"),
            owner_lock_dir: path.join("locks"),
            storage_path: path.join("storage"),
            expected_owner: policy.owner.clone(),
            expected_policy_sha256: String::new(),
            minimum_state_revision: 1,
        };
        let mut fixture = Self {
            _root: root,
            config,
            policy,
            issuer,
            owner,
            token,
        };
        write_private(
            &fixture.config.delegation_root.join("trust.json"),
            &json!({"schema":"astrid.volition.delegated_trust.v1", "target_being":"astrid", "trusted_issuer_public_keys":[fixture.issuer.export_public_key().to_hex()], "bootstrap_audit_refs":["fixture:bootstrap"], "authorized_at_unix_ms":now}),
        );
        fixture.save();
        fixture
    }

    fn save(&mut self) {
        let token_path = self
            .config
            .delegation_root
            .join("tokens")
            .join(format!("{}.json", self.token.id));
        write_private(&token_path, &self.token);
        self.policy.capability_token_sha256 = digest(&std::fs::read(token_path).unwrap());
        self.policy.signature_hex = hex::encode(
            self.issuer
                .sign(&self.policy.signing_bytes().unwrap())
                .as_bytes(),
        );
        self.config.expected_policy_sha256 = self.policy.sha256();
        write_private(&self.config.policy_path, &self.policy);
        self.save_state(PublicWebPolicyStatusV2::Active, 1, false);
    }

    fn save_state(&self, status: PublicWebPolicyStatusV2, revision: u64, owner_signed: bool) {
        let signer = if owner_signed {
            &self.owner
        } else {
            &self.issuer
        };
        let mut state = PublicWebPolicyStateV2 {
            schema: PUBLIC_WEB_STATE_SCHEMA_V2.into(),
            policy_id: self.policy.policy_id.clone(),
            policy_sha256: self.policy.sha256(),
            revision,
            status,
            updated_at_unix_ms: self.policy.issued_at_unix_ms,
            signer_public_key_hex: signer.export_public_key().to_hex(),
            signature_hex: String::new(),
        };
        state.signature_hex = hex::encode(signer.sign(&state.signing_bytes().unwrap()).as_bytes());
        write_private(&self.config.state_path, &state);
        write_private(
            &self.config.state_floor_path,
            &crate::operator::Floor {
                schema: "astrid.public_web.revision_floor.v2".into(),
                owner_sha256: self.config.expected_owner.lock_identity_sha256(),
                revision,
                policy_sha256: self.policy.sha256(),
                state_sha256: state.sha256(),
                approval_sha256: "f".repeat(64),
                pending: false,
            },
        );
    }
}

fn request() -> PublicWebRequest {
    PublicWebRequest {
        schema: REQUEST_SCHEMA.into(),
        operation: PublicWebOperationV2::Page,
        target: "https://example.com/page?private-query-marker=1".into(),
    }
}

struct Chunks(VecDeque<Result<Vec<u8>>>);

#[async_trait]
impl Body for Chunks {
    async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        self.0.pop_front().transpose()
    }
}

fn response(status: u16, location: Option<&str>, bytes: &[u8]) -> HopResponse {
    HopResponse {
        status,
        location: location.map(str::to_owned),
        content_type: Some("text/html; charset=utf-8".into()),
        encoding: "identity".into(),
        body: Box::new(Chunks(VecDeque::from([Ok(bytes.to_vec())]))),
    }
}

struct FakeBackend {
    responses: Mutex<VecDeque<HopResponse>>,
    dns: Mutex<VecDeque<Vec<SocketAddr>>>,
    sent: Mutex<Vec<(String, Vec<SocketAddr>)>>,
    delay: Duration,
}

impl FakeBackend {
    fn new(responses: Vec<HopResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            dns: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            delay: Duration::ZERO,
        }
    }
}

#[async_trait]
impl Backend for FakeBackend {
    async fn resolve(&self, url: &Url) -> Result<Vec<SocketAddr>> {
        tokio::time::sleep(self.delay).await;
        Ok(self.dns.lock().unwrap().pop_front().unwrap_or_else(|| {
            vec![SocketAddr::new(
                "1.1.1.1".parse().unwrap(),
                url.port_or_known_default().unwrap(),
            )]
        }))
    }
    async fn get(
        &self,
        url: &Url,
        pinned: &[SocketAddr],
        _: Duration,
        _: Duration,
    ) -> Result<HopResponse> {
        self.sent
            .lock()
            .unwrap()
            .push((url.to_string(), pinned.to_vec()));
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(PublicWebErrorCode::TransportFailed)
    }
}

#[test]
fn signed_existing_capability_authorizes_standing_policy_without_v1_budget() {
    let fixture = Fixture::new();
    let result = preview(&fixture.config).unwrap();
    assert_eq!(result.policy_sha256, fixture.policy.sha256());
    assert_eq!(result.implementation_scope, "transport_foundation");
    for _ in 0..5 {
        assert!(fixture.config.current(PublicWebOperationV2::Page).is_ok());
    }
    assert!(!fixture.config.delegation_root.join("usage").exists());
}

#[test]
fn unknown_issuer_owner_deployment_and_token_drift_are_denied() {
    let mut fixture = Fixture::new();
    fixture.config.expected_owner.deployment_identity = "other".into();
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::OwnerMismatch
    );
    fixture.config.expected_owner = fixture.policy.owner.clone();
    let token_path = fixture
        .config
        .delegation_root
        .join("tokens")
        .join(format!("{}.json", fixture.token.id));
    write_private(&token_path, &json!({"invalid":"raw-private-marker"}));
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::InvalidCapability
    );
    fixture.save();
    write_private(
        &fixture.config.delegation_root.join("trust.json"),
        &json!({"schema":"astrid.volition.delegated_trust.v1", "target_being":"astrid", "trusted_issuer_public_keys":["a".repeat(64)], "bootstrap_audit_refs":["fixture"], "authorized_at_unix_ms":1}),
    );
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::UntrustedIssuer
    );
}

#[test]
fn old_search_capability_cannot_be_reinterpreted_as_standing_grant() {
    let mut fixture = Fixture::new();
    fixture.token = CapabilityToken::create(
        ResourcePattern::new("network://search/**").unwrap(),
        vec![Permission::Read],
        TokenScope::Persistent,
        fixture.issuer.key_id(),
        AuditEntryId::new(),
        &fixture.issuer,
        None,
    );
    fixture.policy.capability_token_id = fixture.token.id.to_string();
    fixture.save();
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::InvalidCapability
    );
}

#[test]
fn revoked_missing_and_stale_state_fail_closed() {
    let mut fixture = Fixture::new();
    fixture.save_state(PublicWebPolicyStatusV2::Revoked, 2, true);
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::Revoked
    );
    fixture.save_state(PublicWebPolicyStatusV2::Active, 1, false);
    fixture.config.minimum_state_revision = 2;
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::InvalidState
    );
    fixture.config.minimum_state_revision = 1;
    std::fs::remove_file(&fixture.config.state_path).unwrap();
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::AuthorityUnavailable
    );
}

#[test]
fn private_local_special_and_credentialed_destinations_are_denied() {
    for url in [
        "file:///etc/passwd",
        "https://user:pass@example.com/",
        "http://@example.com/",
        "http://127.1/",
        "http://2130706433/",
        "http://0x7f000001/",
        "https://[::1]/",
        "https://[::ffff:127.0.0.1]/",
        "http://169.254.169.254/",
        "http://10.0.0.1/",
        "https://example.com:8443/",
        "https://metadata.local/",
        "https://localhost/",
        "https://singlelabel/",
        "https://example.com./",
    ] {
        assert!(public_url(url).is_err(), "{url}");
    }
    for ip in [
        "100.64.0.1",
        "198.18.0.1",
        "224.0.0.1",
        "240.0.0.1",
        "192.0.2.1",
        "2001:db8::1",
        "2001::1",
        "2002:7f00:1::1",
        "3fff::1",
        "64:ff9b::7f00:1",
        "fc00::1",
        "fe80::1",
    ] {
        assert!(!public_ip(ip.parse().unwrap()), "{ip}");
    }
    for url in [
        "https://example.com/path#section",
        "http://example.com:80/",
        "https://1.1.1.1/",
        "https://[2606:4700:4700::1111]/",
    ] {
        assert!(public_url(url).is_ok(), "{url}");
    }
}

#[tokio::test]
async fn complete_page_has_exact_identity_and_content_free_receipt() {
    let fixture = Fixture::new();
    let backend = FakeBackend::new(vec![response(200, None, b"private-body-marker")]);
    let result = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .unwrap();
    assert_eq!(result.body, b"private-body-marker");
    assert!(result.receipt.complete);
    assert_eq!(result.receipt.request_sha256, request().sha256());
    assert_eq!(result.receipt.body_sha256, digest(&result.body));
    let receipt = serde_json::to_string(&result.receipt).unwrap();
    assert!(!receipt.contains("private"));
    assert!(!receipt.contains("example.com"));
    assert_eq!(
        backend.sent.lock().unwrap()[0].1,
        vec!["1.1.1.1:443".parse::<SocketAddr>().unwrap()]
    );
}

#[tokio::test]
async fn mixed_dns_answers_and_redirect_rebinding_never_reach_get() {
    let fixture = Fixture::new();
    let backend = FakeBackend::new(vec![]);
    backend.dns.lock().unwrap().push_back(vec![
        "1.1.1.1:443".parse().unwrap(),
        "127.0.0.1:443".parse().unwrap(),
    ]);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::UnsafeAddress);
    assert!(backend.sent.lock().unwrap().is_empty());
    let backend = FakeBackend::new(vec![response(302, Some("/next"), b"redirect")]);
    backend.dns.lock().unwrap().extend([
        vec!["1.1.1.1:443".parse().unwrap()],
        vec!["10.0.0.1:443".parse().unwrap()],
    ]);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::UnsafeAddress);
    assert_eq!(backend.sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn redirect_to_private_target_fails_before_second_resolution() {
    let fixture = Fixture::new();
    let backend = FakeBackend::new(vec![response(
        302,
        Some("http://169.254.169.254/latest"),
        b"",
    )]);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::UnsafeUrl);
    assert_eq!(backend.sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn five_redirects_succeed_six_do_not() {
    let fixture = Fixture::new();
    let mut hops = (0..5)
        .map(|_| response(302, Some("/next"), b"x"))
        .collect::<Vec<_>>();
    hops.push(response(200, None, b"done"));
    let result = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(hops),
    )
    .await
    .unwrap();
    assert_eq!(result.receipt.redirects_followed, 5);
    assert_eq!(result.receipt.decoded_bytes_observed, 9);
    let backend = FakeBackend::new((0..6).map(|_| response(302, Some("/next"), b"x")).collect());
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::RedirectLimit);
    assert_eq!(backend.sent.lock().unwrap().len(), 6);
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

#[tokio::test]
async fn decoded_limit_is_cumulative_across_compressed_redirects() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.decoded_body_bytes = 10;
    fixture.save();
    let mut redirect = response(302, Some("/next"), &gzip(b"123456"));
    redirect.encoding = "gzip".into();
    let backend = FakeBackend::new(vec![redirect, response(200, None, b"12345")]);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::BodyLimit);
    assert_eq!(failure.decoded_bytes_observed, 11);
    assert!(!failure.complete);
}

#[tokio::test]
async fn gzip_bomb_and_truncated_encoding_never_return_partial_success() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.decoded_body_bytes = 128;
    fixture.save();
    let mut oversized = response(200, None, &gzip(&vec![b'x'; 32_768]));
    oversized.encoding = "gzip".into();
    let failure = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![oversized]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::BodyLimit);
    let mut truncated = gzip(b"safe");
    truncated.truncate(truncated.len().saturating_sub(3));
    let mut hop = response(200, None, &truncated);
    hop.encoding = "gzip".into();
    let failure = fetch_with(
        &fixture.config,
        &request(),
        &fixture.config,
        &FakeBackend::new(vec![hop]),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::IncompleteBody);
}

#[tokio::test(start_paused = true)]
async fn dns_and_logical_fetch_deadlines_are_enforced() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.total_timeout_ms = 10;
    fixture.save();
    let mut backend = FakeBackend::new(vec![]);
    backend.delay = Duration::from_millis(11);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::Deadline);
    assert!(backend.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn storage_headroom_denies_before_network() {
    let mut fixture = Fixture::new();
    fixture.policy.limits.storage_headroom_bytes = u64::MAX;
    fixture.save();
    let backend = FakeBackend::new(vec![]);
    let failure = fetch_with(&fixture.config, &request(), &fixture.config, &backend)
        .await
        .err()
        .unwrap();
    assert_eq!(failure.code, PublicWebErrorCode::StorageHeadroom);
    assert!(backend.sent.lock().unwrap().is_empty());
}

#[test]
fn cross_process_owner_lock_blocks_and_releases_without_unlink() {
    let fixture = Fixture::new();
    let owner = fixture.policy.owner.lock_identity_sha256();
    let lease = crate::guard::OwnerLease::acquire(&fixture.config.owner_lock_dir, &owner).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "tests::owner_lock_child", "--ignored"])
        .env("PUBLIC_WEB_TEST_LOCK_DIR", &fixture.config.owner_lock_dir)
        .env("PUBLIC_WEB_TEST_LOCK_OWNER", &owner)
        .status()
        .unwrap();
    assert!(status.success());
    drop(lease);
    assert!(crate::guard::OwnerLease::acquire(&fixture.config.owner_lock_dir, &owner).is_ok());
    assert!(
        fixture
            .config
            .owner_lock_dir
            .join(format!("{owner}.lock"))
            .exists()
    );
}

#[test]
#[ignore = "invoked by the cross-process lock test with isolated fixture paths"]
fn owner_lock_child() {
    let directory = std::env::var("PUBLIC_WEB_TEST_LOCK_DIR").unwrap();
    let owner = std::env::var("PUBLIC_WEB_TEST_LOCK_OWNER").unwrap();
    if std::env::var_os("PUBLIC_WEB_TEST_ABRUPT_EXIT").is_some() {
        let _lease = crate::guard::OwnerLease::acquire(Path::new(&directory), &owner).unwrap();
        std::process::exit(17);
    }
    assert!(matches!(
        crate::guard::OwnerLease::acquire(Path::new(&directory), &owner),
        Err(PublicWebErrorCode::OwnerBusy)
    ));
}

#[test]
fn symlinked_authority_and_public_files_fail_closed() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let fixture = Fixture::new();
    std::fs::set_permissions(
        &fixture.config.policy_path,
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::UnsafeFilesystem
    );
    std::fs::set_permissions(
        &fixture.config.policy_path,
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let original = fixture.config.policy_path.with_extension("original");
    std::fs::rename(&fixture.config.policy_path, &original).unwrap();
    symlink(original, &fixture.config.policy_path).unwrap();
    assert!(preview(&fixture.config).is_err());
}
