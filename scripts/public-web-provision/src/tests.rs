use super::*;
use astrid_minime_protocol::public_web::{
    BRAVE_REQUEST_MICROUSD, PUBLIC_WEB_CAPABILITY_RESOURCE_V2, PUBLIC_WEB_CAPABILITY_RESOURCE_V3,
};
use astrid_public_web::{PublicWebErrorCode, mutate, preview, preview_mutation, status};
use std::os::unix::fs::{MetadataExt as _, symlink};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    issuer: PathBuf,
    locks: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let issuer = root.join("issuer");
        init_issuer(&issuer).unwrap();
        let locks = root.join("shared-locks");
        Directory::create(&locks).unwrap();
        Self {
            _temp: temp,
            root,
            issuer,
            locks,
        }
    }

    fn json(&self, name: &str, value: &impl Serialize) -> PathBuf {
        Directory::open(&self.root)
            .unwrap()
            .write_json(name, value)
            .unwrap();
        self.root.join(name)
    }

    fn prepare_request(&self, being: &str) -> PrepareRequest {
        PrepareRequest {
            schema: "astrid.public_web.provision.prepare.v1".into(),
            owner: PublicWebOwnerV2 {
                being: being.into(),
                principal: format!("synthetic-{being}"),
                public_key_hex: KeyPair::generate().export_public_key().to_hex(),
                deployment_identity: "synthetic-frozen-input-identity".into(),
            },
            policy_id: format!("synthetic-{being}-year"),
            install_root: self.root.join(format!("installed-{being}")),
            owner_lock_dir: self.locks.clone(),
            approval_audit_ref: "synthetic:test-approval".into(),
            search_api: None,
            brave: None,
        }
    }

    fn search_request(&self, being: &str) -> PrepareRequest {
        let mut request = self.prepare_request(being);
        request.search_api = Some(BraveSearchPolicyV1 {
            schema: "volition.brave_search.v1".into(),
            account_id: "synthetic-shared-account".into(),
            monthly_limit_microusd: 25_000_000,
            request_cost_microusd: BRAVE_REQUEST_MICROUSD,
            storage_rights_status:
                astrid_minime_protocol::public_web::SearchStorageRights::Unverified,
            retention_review_audit_ref: "synthetic:operator-retention-review".into(),
        });
        request.brave = Some(BraveSearchConfig {
            credential_file: self.root.join("unread-synthetic-credential"),
            account_directory: self.root.join("uncreated-synthetic-account"),
        });
        request
    }

    fn prepared_search(&self, being: &str) -> PathBuf {
        let request = self.search_request(being);
        let path = self.json(&format!("{being}-search-request.json"), &request);
        let out = self.root.join(format!("search-packet-{being}"));
        prepare(&self.issuer, &path, &out).unwrap();
        assert!(!request.install_root.exists());
        out
    }

    fn prepared(&self, being: &str) -> PathBuf {
        let request = self.prepare_request(being);
        let path = self.json(&format!("{being}-request.json"), &request);
        let out = self.root.join(format!("packet-{being}"));
        prepare(&self.issuer, &path, &out).unwrap();
        assert!(!request.install_root.exists());
        out
    }

    fn install_fixture(
        &self,
        packet: &Path,
    ) -> (PublicWebConfig, PublicWebBundle, PublicWebApprovalV2) {
        let (config, bundle, approval) = packet_values(packet);
        let install = config.policy_path.parent().unwrap();
        assert!(install.starts_with(&self.root));
        Directory::create(install).unwrap();
        for path in [
            &config.delegation_root,
            &config.delegation_root.join("tokens"),
            &config.storage_path,
        ] {
            Directory::create(path).unwrap();
        }
        Directory::open(&config.delegation_root)
            .unwrap()
            .write_new(
                "trust.json",
                &private_fs::read(&packet.join("trust.json")).unwrap(),
            )
            .unwrap();
        let before = preview_mutation(&config, &config.expected_owner, &bundle, &approval).unwrap();
        assert!(!before.already_published);
        assert!(!config.policy_path.exists());
        mutate(&config, &config.expected_owner, &bundle, &approval).unwrap();
        (config, bundle, approval)
    }
}

fn packet_values(packet: &Path) -> (PublicWebConfig, PublicWebBundle, PublicWebApprovalV2) {
    (
        private_fs::read_json(&packet.join("config.json")).unwrap(),
        private_fs::read_json(&packet.join("bundle.json")).unwrap(),
        private_fs::read_json(&packet.join("approval.json")).unwrap(),
    )
}

fn exact(packet: &Path) -> ExactInput {
    let (config, bundle, _) = packet_values(packet);
    ExactInput {
        owner: config.expected_owner.clone(),
        owner_lock_dir: config.owner_lock_dir.clone(),
        config_path: packet.join("config.json"),
        bundle_path: packet.join("bundle.json"),
        config_sha256: config.sha256(),
        policy_sha256: bundle.policy.sha256(),
        state_sha256: bundle.state.sha256(),
    }
}

#[test]
fn both_owners_reuse_one_private_issuer_with_default_limits_and_no_quotas() {
    let f = Fixture::new();
    let key_before = private_fs::read(&f.issuer.join(KEY_NAME)).unwrap();
    let first = f.prepared("astrid");
    let second = f.prepared("minime");
    let (a, ab, aa) = packet_values(&first);
    let (m, mb, _) = packet_values(&second);
    assert_eq!(
        ab.policy.issuer_public_key_hex,
        mb.policy.issuer_public_key_hex
    );
    assert_ne!(a.expected_owner, m.expected_owner);
    assert_eq!(a.owner_lock_dir, f.locks);
    assert_eq!(m.owner_lock_dir, f.locks);
    assert_ne!(ab.policy.capability_token_id, mb.policy.capability_token_id);
    assert_eq!(ab.policy.limits, PublicWebLimitsV2::default());
    assert_eq!(
        ab.policy.operations,
        [PublicWebOperationV2::Search, PublicWebOperationV2::Page]
    );
    assert_eq!(
        ab.policy
            .expires_at_unix_ms
            .checked_sub(ab.policy.issued_at_unix_ms),
        Some(YEAR_MS)
    );
    assert_eq!(
        aa.expires_at_unix_ms.checked_sub(aa.approved_at_unix_ms),
        Some(APPROVAL_MS)
    );
    let token: CapabilityToken = serde_json::from_str(&ab.capability_token_json).unwrap();
    assert_eq!(token.scope, TokenScope::Persistent);
    assert!(!token.is_single_use());
    assert!(token.expires_at.is_some());
    assert_eq!(
        *key_before,
        *private_fs::read(&f.issuer.join(KEY_NAME)).unwrap()
    );
    assert_eq!(std::fs::metadata(&f.issuer).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        std::fs::metadata(f.issuer.join(KEY_NAME)).unwrap().mode() & 0o777,
        0o600
    );
    for entry in std::fs::read_dir(&first).unwrap() {
        assert_eq!(entry.unwrap().metadata().unwrap().mode() & 0o777, 0o600);
    }
    let receipt: Value = private_fs::read_json(&first.join("receipt.json")).unwrap();
    assert_eq!(receipt["installed"], false);
    assert!(receipt.get("capability_token_json").is_none());
    assert!(receipt.get("signature_hex").is_none());
    assert_eq!(std::fs::read_dir(&f.locks).unwrap().count(), 0);
    let (installed_a, _, _) = f.install_fixture(&first);
    let (installed_m, _, _) = f.install_fixture(&second);
    assert_eq!(installed_a.owner_lock_dir, installed_m.owner_lock_dir);
    assert!(preview(&installed_a).is_ok());
    assert!(preview(&installed_m).is_ok());
    assert!(!a.policy_path.parent().unwrap().join("locks").exists());
    assert!(!m.policy_path.parent().unwrap().join("locks").exists());
}

#[test]
fn existing_operator_preview_mutate_status_accept_prepared_files() {
    let f = Fixture::new();
    let packet = f.prepared("astrid");
    let (config, bundle, approval) = f.install_fixture(&packet);
    assert_eq!(
        status(&config).unwrap().status,
        PublicWebPolicyStatusV2::Active
    );
    for _ in 0..4 {
        assert_eq!(
            preview(&config).unwrap().policy_sha256,
            bundle.policy.sha256()
        );
    }
    assert!(!config.delegation_root.join("usage").exists());
    assert!(
        mutate(&config, &config.expected_owner, &bundle, &approval)
            .unwrap()
            .already_published
    );
}

#[test]
fn expired_exact_approval_renews_without_changing_policy_state_or_config() {
    for search in [false, true] {
        let f = Fixture::new();
        let packet = if search {
            f.prepared_search("astrid")
        } else {
            f.prepared("astrid")
        };
        let (config, bundle, mut old) = packet_values(&packet);
        let key = load_issuer(&f.issuer).unwrap();
        let now = now_ms().unwrap();
        old.approved_at_unix_ms = now.checked_sub(10_000).unwrap();
        old.expires_at_unix_ms = now.checked_sub(1_000).unwrap();
        old.signature_hex = key.sign(&old.signing_bytes().unwrap()).to_hex();
        assert!(!old.verifies_for(&bundle.policy, &bundle.state, &config.sha256(), now));
        let old_path = f.json("expired.json", &old);
        let request = RenewRequest {
            schema: "astrid.public_web.provision.renew.v1".into(),
            exact: exact(&packet),
            approval_file_sha256: sha(&private_fs::read(&old_path).unwrap()),
            approval_path: old_path,
            approval_audit_ref: "synthetic:renew".into(),
        };
        let request_path = f.json("renew.json", &request);
        let renewed = f.root.join("renewed");
        renew(&f.issuer, &request_path, &renewed).unwrap();
        for name in ["bundle.json", "config.json", "runtime-owner.json"] {
            assert_eq!(
                *private_fs::read(&packet.join(name)).unwrap(),
                *private_fs::read(&renewed.join(name)).unwrap()
            );
        }
        let (renewed_config, _, fresh) = packet_values(&renewed);
        assert_eq!(renewed_config.owner_lock_dir, f.locks);
        assert_eq!(renewed_config.sha256(), config.sha256());
        assert!(fresh.verifies_for(
            &bundle.policy,
            &bundle.state,
            &config.sha256(),
            now_ms().unwrap()
        ));
        assert_eq!(
            fresh.expected_previous_floor_sha256,
            old.expected_previous_floor_sha256
        );
        assert!(!config.policy_path.exists());
        let (installed, _, _) = f.install_fixture(&packet);
        assert_eq!(
            preview_mutation(&installed, &installed.expected_owner, &bundle, &fresh).unwrap_err(),
            PublicWebErrorCode::StaleApproval
        );
    }
}

#[test]
fn revoke_preparation_is_read_only_and_installer_enforces_reviewed_floor() {
    for search in [false, true] {
        let f = Fixture::new();
        let packet = if search {
            f.prepared_search("minime")
        } else {
            f.prepared("minime")
        };
        let (config, _, _) = f.install_fixture(&packet);
        let current = status(&config).unwrap();
        let request = RevokeRequest {
            schema: "astrid.public_web.provision.revoke.v1".into(),
            exact: exact(&packet),
            expected_previous_floor_sha256: current.floor_sha256.clone(),
            expected_state_revision: current.state_revision,
            approval_audit_ref: "synthetic:revoke".into(),
        };
        let request_path = f.json("revoke.json", &request);
        let out = f.root.join("revocation");
        revoke(&f.issuer, &request_path, &out).unwrap();
        assert_eq!(status(&config).unwrap().floor_sha256, current.floor_sha256);
        assert!(preview(&config).is_ok());
        let (revoked_config, bundle, approval) = packet_values(&out);
        assert_eq!(revoked_config.owner_lock_dir, f.locks);
        assert_eq!(revoked_config.sha256(), config.sha256());
        assert_eq!(
            *private_fs::read(&packet.join("config.json")).unwrap(),
            *private_fs::read(&out.join("config.json")).unwrap()
        );
        assert_eq!(bundle.state.revision, 2);
        assert_eq!(approval.operation, PublicWebMutationV2::Revoke);
        preview_mutation(&config, &config.expected_owner, &bundle, &approval).unwrap();
        mutate(&config, &config.expected_owner, &bundle, &approval).unwrap();
        assert_eq!(
            status(&config).unwrap().status,
            PublicWebPolicyStatusV2::Revoked
        );
        assert_eq!(preview(&config).unwrap_err(), PublicWebErrorCode::Revoked);
        let conflict = f.root.join("conflict");
        assert_eq!(
            revoke(&f.issuer, &request_path, &conflict).unwrap_err(),
            "installed_head_conflict"
        );
        assert!(!conflict.exists());
    }
}

#[test]
fn explicit_search_policy_prepares_v3_without_opening_credentials_or_account() {
    let f = Fixture::new();
    let packet = f.prepared_search("astrid");
    let (config, bundle, approval) = packet_values(&packet);
    let expected = f.search_request("astrid");
    assert_eq!(config.schema, CONFIG_SCHEMA_V3);
    assert_eq!(bundle.policy.schema, PUBLIC_WEB_POLICY_SCHEMA_V3);
    assert_eq!(bundle.policy.search_api, expected.search_api);
    assert!(config.brave == expected.brave);
    let brave = config.brave.as_ref().unwrap();
    assert!(!brave.credential_file.exists());
    assert!(!brave.account_directory.exists());
    let token: CapabilityToken = serde_json::from_str(&bundle.capability_token_json).unwrap();
    assert_eq!(token.resource.as_str(), PUBLIC_WEB_CAPABILITY_RESOURCE_V3);
    assert_eq!(token.resource.as_str(), bundle.policy.capability_resource());
    assert!(bundle.policy.verifies(now_ms().unwrap()));
    assert!(approval.verifies_for(
        &bundle.policy,
        &bundle.state,
        &config.sha256(),
        now_ms().unwrap()
    ));
    let receipt: Value = private_fs::read_json(&packet.join("receipt.json")).unwrap();
    assert_eq!(receipt["installed"], false);
    assert_eq!(receipt["network_used"], false);
    let (installed, _, _) = f.install_fixture(&packet);
    assert!(preview(&installed).is_ok());
    assert!(!brave.credential_file.exists());
    assert!(!brave.account_directory.exists());
}

#[test]
fn search_requires_explicit_usage_review_and_honest_storage_status() {
    let f = Fixture::new();
    let baseline = serde_json::to_value(f.search_request("astrid")).unwrap();
    let mut invalid = Vec::new();
    for field in ["search_api", "brave"] {
        let mut request = baseline.clone();
        request.as_object_mut().unwrap().remove(field);
        invalid.push(request);
    }
    for (field, value) in [
        ("retention_review_audit_ref", json!("")),
        ("retention_review_audit_ref", json!(" inferred ")),
        ("storage_rights_status", json!("assumed_from_key")),
        ("account_id", json!("")),
        ("schema", json!("unrecognized")),
        ("monthly_limit_microusd", json!(0)),
        (
            "monthly_limit_microusd",
            json!(BRAVE_REQUEST_MICROUSD.saturating_sub(1)),
        ),
        ("request_cost_microusd", json!(0)),
    ] {
        let mut request = baseline.clone();
        request["search_api"][field] = value;
        invalid.push(request);
    }
    let mut missing_rights = baseline.clone();
    missing_rights["search_api"]
        .as_object_mut()
        .unwrap()
        .remove("storage_rights_status");
    invalid.push(missing_rights);
    for (index, request) in invalid.iter().enumerate() {
        let path = f.json(&format!("invalid-search-{index}.json"), request);
        let out = f.root.join(format!("invalid-search-{index}"));
        assert!(prepare(&f.issuer, &path, &out).is_err());
        assert!(!out.exists());
    }
    let mut unknown = baseline;
    unknown["brave"]["api_key"] = json!("synthetic-not-a-real-key");
    let path = f.json("unknown-credential-field.json", &unknown);
    assert_eq!(
        prepare(&f.issuer, &path, &f.root.join("unknown-output")).unwrap_err(),
        "invalid_operator_json"
    );
}

#[test]
fn explicit_search_paths_must_be_normal_absolute_and_disjoint() {
    let f = Fixture::new();
    for (index, path) in [
        PathBuf::from("relative"),
        f.root.join("../escape"),
        f.issuer.join(KEY_NAME),
        f.locks.join("key"),
        f.root.join("installed-astrid/key"),
        f.root.join("uncreated-synthetic-account/key"),
        f.root.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = f.search_request("astrid");
        request.brave.as_mut().unwrap().credential_file = path;
        let path = f.json(&format!("bad-search-path-{index}.json"), &request);
        let out = f.root.join(format!("bad-search-path-{index}"));
        assert!(prepare(&f.issuer, &path, &out).is_err());
        assert!(!out.exists());
    }
    let mut request = f.search_request("astrid");
    let out = f.root.join("search-path-collision");
    request.brave.as_mut().unwrap().account_directory = out.join("account");
    let path = f.json("output-collision.json", &request);
    assert!(prepare(&f.issuer, &path, &out).is_err());
    assert!(!out.exists());
}

#[test]
fn v3_renewal_rejects_config_policy_drift_and_output_collision() {
    let f = Fixture::new();
    let packet = f.prepared_search("astrid");
    let (mut config, bundle, _) = packet_values(&packet);
    let mut request = RenewRequest {
        schema: "astrid.public_web.provision.renew.v1".into(),
        exact: exact(&packet),
        approval_path: packet.join("approval.json"),
        approval_file_sha256: sha(&private_fs::read(&packet.join("approval.json")).unwrap()),
        approval_audit_ref: "synthetic:renew-search".into(),
    };
    let path = f.json("renew-search-output-collision.json", &request);
    let account = config.brave.as_ref().unwrap().account_directory.clone();
    assert!(renew(&f.issuer, &path, &account).is_err());
    assert!(!account.exists());
    config.brave.as_mut().unwrap().credential_file = f.root.join("changed-credential");
    request.exact.config_path = f.json("changed-search-config.json", &config);
    let out = f.root.join("refused-search-renewal");
    let path = f.json("renew-search-drift.json", &request);
    assert_eq!(
        renew(&f.issuer, &path, &out).unwrap_err(),
        "reviewed_input_mismatch"
    );
    request.exact.config_sha256 = config.sha256();
    let path = f.json("renew-search-rehashed.json", &request);
    assert_eq!(
        renew(&f.issuer, &path, &out).unwrap_err(),
        "invalid_prior_approval"
    );
    assert!(!out.exists());
    assert!(bundle.policy.verifies(now_ms().unwrap()));
}

#[test]
fn exact_validation_rejects_cross_version_token_even_when_resigned() {
    let f = Fixture::new();
    let packet = f.prepared_search("astrid");
    let (mut config, mut bundle, _) = packet_values(&packet);
    let key = load_issuer(&f.issuer).unwrap();
    let token = CapabilityToken::create(
        ResourcePattern::exact(PUBLIC_WEB_CAPABILITY_RESOURCE_V2).unwrap(),
        vec![Permission::Read],
        TokenScope::Persistent,
        key.key_id(),
        AuditEntryId::new(),
        &key,
        Some(chrono::Duration::days(365)),
    );
    bundle.capability_token_json = serde_json::to_string(&token).unwrap();
    bundle.policy.capability_token_id = token.id.to_string();
    bundle.policy.capability_token_sha256 = sha(bundle.capability_token_json.as_bytes());
    bundle.policy.signature_hex = key.sign(&bundle.policy.signing_bytes().unwrap()).to_hex();
    bundle.state.policy_sha256 = bundle.policy.sha256();
    sign_state(&key, &mut bundle.state).unwrap();
    config.expected_policy_sha256 = bundle.policy.sha256();
    assert_eq!(
        validate(&key, &config, &bundle).unwrap_err(),
        "invalid_capability"
    );
    config.brave = None;
    assert_eq!(
        validate(&key, &config, &bundle).unwrap_err(),
        "invalid_search_configuration"
    );
}

#[test]
fn v2_wire_bytes_hashes_and_policy_signature_keep_the_legacy_shape() {
    // Field order matters for config.sha256; keep an independent pre-v3 schema.
    #[derive(Serialize)]
    struct LegacyConfig<'a> {
        schema: &'a str,
        delegation_root: &'a Path,
        policy_path: &'a Path,
        state_path: &'a Path,
        state_floor_path: &'a Path,
        owner_lock_dir: &'a Path,
        storage_path: &'a Path,
        expected_owner: &'a PublicWebOwnerV2,
        expected_policy_sha256: &'a str,
        minimum_state_revision: u64,
    }
    let f = Fixture::new();
    let packet = f.prepared("astrid");
    let (config, bundle, _) = packet_values(&packet);
    let legacy = LegacyConfig {
        schema: &config.schema,
        delegation_root: &config.delegation_root,
        policy_path: &config.policy_path,
        state_path: &config.state_path,
        state_floor_path: &config.state_floor_path,
        owner_lock_dir: &config.owner_lock_dir,
        storage_path: &config.storage_path,
        expected_owner: &config.expected_owner,
        expected_policy_sha256: &config.expected_policy_sha256,
        minimum_state_revision: 1,
    };
    assert_eq!(encoded(&config).unwrap(), encoded(&legacy).unwrap());
    assert_eq!(config.sha256(), sha(&encoded(&legacy).unwrap()));
    assert_eq!(config.schema, CONFIG_SCHEMA);
    let policy = &bundle.policy;
    let legacy_policy = json!({
        "schema": policy.schema, "policy_id": policy.policy_id, "revision": policy.revision,
        "owner": policy.owner, "capability_token_id": policy.capability_token_id,
        "capability_token_sha256": policy.capability_token_sha256,
        "issuer_public_key_hex": policy.issuer_public_key_hex, "operations": policy.operations,
        "limits": policy.limits, "issued_at_unix_ms": policy.issued_at_unix_ms,
        "expires_at_unix_ms": policy.expires_at_unix_ms,
        "approval_audit_refs": policy.approval_audit_refs, "signature_hex": "",
    });
    let legacy_signing_bytes = encoded(&legacy_policy).unwrap();
    assert_eq!(policy.signing_bytes().unwrap(), legacy_signing_bytes);
    assert_eq!(
        policy.signature_hex,
        load_issuer(&f.issuer)
            .unwrap()
            .sign(&legacy_signing_bytes)
            .to_hex()
    );
    assert!(
        serde_json::to_value(policy)
            .unwrap()
            .get("search_api")
            .is_none()
    );
    assert!(
        serde_json::to_value(&config)
            .unwrap()
            .get("brave")
            .is_none()
    );
    assert_eq!(
        policy.capability_resource(),
        PUBLIC_WEB_CAPABILITY_RESOURCE_V2
    );
    let request = serde_json::to_value(f.prepare_request("other")).unwrap();
    assert!(request.get("search_api").is_none());
    assert!(request.get("brave").is_none());
}

#[test]
fn no_clobber_keys_packets_or_existing_files() {
    let f = Fixture::new();
    let before = private_fs::read(&f.issuer.join(KEY_NAME)).unwrap();
    assert!(init_issuer(&f.issuer).is_err());
    assert_eq!(
        *before,
        *private_fs::read(&f.issuer.join(KEY_NAME)).unwrap()
    );
    let packet = f.prepared("astrid");
    let before = private_fs::read(&packet.join("receipt.json")).unwrap();
    assert!(prepare(&f.issuer, &f.root.join("astrid-request.json"), &packet).is_err());
    assert_eq!(
        *before,
        *private_fs::read(&packet.join("receipt.json")).unwrap()
    );
    let dir = Directory::open(&packet).unwrap();
    assert!(dir.write_new("receipt.json", b"replacement").is_err());
    assert!(dir.write_new("../outside", b"escape").is_err());
}

#[test]
fn corrupt_or_missing_issuer_is_never_recreated() {
    let f = Fixture::new();
    let key_path = f.issuer.join(KEY_NAME);
    let mut corrupt = private_fs::read(&key_path).unwrap();
    corrupt[45] ^= 1;
    std::fs::write(&key_path, &corrupt).unwrap();
    assert_eq!(load_issuer(&f.issuer).unwrap_err(), "corrupt_issuer");
    let request = f.json("prepare.json", &f.prepare_request("astrid"));
    let output = f.root.join("refused");
    assert_eq!(
        prepare(&f.issuer, &request, &output).unwrap_err(),
        "corrupt_issuer"
    );
    assert_eq!(*corrupt, *private_fs::read(&key_path).unwrap());
    assert!(!output.exists());
    let empty = f.root.join("empty-issuer");
    Directory::create(&empty).unwrap();
    assert!(load_issuer(&empty).is_err());
    assert!(!empty.join(KEY_NAME).exists());
}

#[test]
fn symlinks_hardlinks_public_modes_and_relative_paths_are_refused() {
    use std::os::unix::fs::PermissionsExt as _;
    let f = Fixture::new();
    let link = f.root.join("linked-issuer");
    symlink(&f.issuer, &link).unwrap();
    assert!(load_issuer(&link).is_err());
    assert!(Directory::create(&link.join("child")).is_err());
    let linked_key_dir = f.root.join("linked-key-dir");
    Directory::create(&linked_key_dir).unwrap();
    symlink(f.issuer.join(KEY_NAME), linked_key_dir.join(KEY_NAME)).unwrap();
    assert!(load_issuer(&linked_key_dir).is_err());
    assert!(private_fs::read(Path::new("relative.json")).is_err());
    assert!(private_fs::read(&f.issuer.join("../issuer/issuer.key")).is_err());
    let hardlink = f.root.join("hardlink");
    std::fs::hard_link(f.issuer.join(KEY_NAME), &hardlink).unwrap();
    assert!(load_issuer(&f.issuer).is_err());
    std::fs::remove_file(hardlink).unwrap();
    std::fs::set_permissions(
        f.issuer.join(KEY_NAME),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(load_issuer(&f.issuer).is_err());
}

#[test]
fn exact_input_drift_owner_mismatch_and_existing_install_root_refuse_output() {
    let f = Fixture::new();
    let packet = f.prepared("astrid");
    let key = load_issuer(&f.issuer).unwrap();
    let mut reviewed = exact(&packet);
    reviewed.owner.deployment_identity = "different-startup".into();
    assert!(load_exact(&key, &reviewed).is_err());
    let mut reviewed = exact(&packet);
    reviewed.config_sha256 = "0".repeat(64);
    assert!(load_exact(&key, &reviewed).is_err());
    let mut reviewed = exact(&packet);
    reviewed.owner_lock_dir = f.root.join("different-locks");
    assert!(load_exact(&key, &reviewed).is_err());
    let mut request = f.prepare_request("minime");
    request.install_root = f.issuer.clone();
    let path = f.json("overlap.json", &request);
    let output = f.root.join("refused-overlap");
    assert!(prepare(&f.issuer, &path, &output).is_err());
    assert!(!output.exists());
    request.install_root = f.root.join("already-installed");
    Directory::create(&request.install_root).unwrap();
    let path = f.json("existing.json", &request);
    assert_eq!(
        prepare(&f.issuer, &path, &output).unwrap_err(),
        "install_root_exists_or_unavailable"
    );
}

#[test]
fn prepare_requires_an_existing_private_disjoint_lock_directory() {
    use std::os::unix::fs::PermissionsExt as _;
    let f = Fixture::new();
    let mut request = f.prepare_request("astrid");
    let mut missing_field = serde_json::to_value(&request).unwrap();
    missing_field
        .as_object_mut()
        .unwrap()
        .remove("owner_lock_dir");
    let path = f.json("missing-field.json", &missing_field);
    let output = f.root.join("refused-locks");
    assert_eq!(
        prepare(&f.issuer, &path, &output).unwrap_err(),
        "invalid_operator_json"
    );
    let link = f.root.join("linked-locks");
    symlink(&f.locks, &link).unwrap();
    let linked_parent = f.root.join("linked-parent");
    symlink(&f.root, &linked_parent).unwrap();
    let public = f.root.join("public-locks");
    Directory::create(&public).unwrap();
    std::fs::set_permissions(&public, std::fs::Permissions::from_mode(0o755)).unwrap();
    let not_directory = f.json("not-directory.json", &json!({}));
    let absent = f.root.join("absent-locks");
    for (index, invalid) in [
        absent.clone(),
        PathBuf::from("relative-locks"),
        link,
        linked_parent.join("shared-locks"),
        public,
        not_directory,
        f.issuer.clone(),
        f.root.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        request.owner_lock_dir = invalid;
        let path = f.json(&format!("invalid-locks-{index}.json"), &request);
        assert!(prepare(&f.issuer, &path, &output).is_err());
        assert!(!output.exists());
    }
    assert!(!absent.exists());
    request.owner_lock_dir = f.locks.clone();
    let path = f.json("overlapping-output.json", &request);
    assert!(prepare(&f.issuer, &path, &f.locks.join("packet")).is_err());
    assert_eq!(std::fs::read_dir(&f.locks).unwrap().count(), 0);
}

#[test]
fn renew_and_revoke_revalidate_lock_safety_and_output_separation() {
    use std::os::unix::fs::PermissionsExt as _;
    let f = Fixture::new();
    let packet = f.prepared("astrid");
    let (config, _, _) = f.install_fixture(&packet);
    let current = status(&config).unwrap();
    let renewal = RenewRequest {
        schema: "astrid.public_web.provision.renew.v1".into(),
        exact: exact(&packet),
        approval_path: packet.join("approval.json"),
        approval_file_sha256: sha(&private_fs::read(&packet.join("approval.json")).unwrap()),
        approval_audit_ref: "synthetic:renew-lock-safety".into(),
    };
    let revocation = RevokeRequest {
        schema: "astrid.public_web.provision.revoke.v1".into(),
        exact: exact(&packet),
        expected_previous_floor_sha256: current.floor_sha256.clone(),
        expected_state_revision: current.state_revision,
        approval_audit_ref: "synthetic:revoke-lock-safety".into(),
    };
    let renew_path = f.json("renew-lock-safety.json", &renewal);
    let revoke_path = f.json("revoke-lock-safety.json", &revocation);
    let overlapping = f.locks.join("forbidden-output");
    assert!(renew(&f.issuer, &renew_path, &overlapping).is_err());
    assert!(revoke(&f.issuer, &revoke_path, &overlapping).is_err());
    assert!(!overlapping.exists());
    std::fs::set_permissions(&f.locks, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = f.root.join("unsafe-locks-output");
    assert!(renew(&f.issuer, &renew_path, &output).is_err());
    assert!(revoke(&f.issuer, &revoke_path, &output).is_err());
    assert!(!output.exists());
    std::fs::set_permissions(&f.locks, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(status(&config).unwrap().floor_sha256, current.floor_sha256);
}

#[test]
fn cli_has_no_install_fetch_force_or_implicit_identity_command() {
    use clap::Parser as _;
    for args in [
        vec!["provision"],
        vec!["provision", "install"],
        vec!["provision", "fetch"],
        vec![
            "provision",
            "prepare",
            "--issuer-dir",
            "/synthetic/key",
            "--out",
            "/synthetic/out",
        ],
    ] {
        assert!(crate::Cli::try_parse_from(args).is_err());
    }
}
