//! Thread-local signed authority fixtures, never a transport or admission bypass.
use super::*;
use astrid_capabilities::{AuditEntryId, CapabilityToken, ResourcePattern, TokenScope};
use astrid_core::Permission;
use astrid_crypto::KeyPair;
use astrid_minime_protocol::public_web::*;
use astrid_public_web::{PublicWebBundle, mutate};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

type Context = Result<(PublicWebConfig, PublicWebOwnerV2), String>;
thread_local! { static SELECTED: RefCell<Option<Context>> = const { RefCell::new(None) }; }

pub(super) fn context() -> Option<Context> {
    SELECTED.with(|selected| selected.borrow().clone())
}

pub(crate) struct Fixture {
    _root: tempfile::TempDir,
    previous: Option<Context>,
    pub(crate) config: PublicWebConfig,
}

fn write(path: &Path, value: &impl Serialize) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["delegation", "delegation/tokens", "locks", "storage"] {
            std::fs::create_dir(path.join(name)).unwrap();
            std::fs::set_permissions(path.join(name), std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        let issuer = KeyPair::generate();
        let owner = PublicWebOwnerV2 {
            being: "astrid".into(),
            principal: "astrid".into(),
            public_key_hex: KeyPair::generate().export_public_key().to_hex(),
            deployment_identity: "synthetic-release".into(),
        };
        let now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let token = CapabilityToken::create(
            ResourcePattern::exact(PUBLIC_WEB_CAPABILITY_RESOURCE_V2).unwrap(),
            vec![Permission::Read],
            TokenScope::Persistent,
            issuer.key_id(),
            AuditEntryId::new(),
            &issuer,
            None,
        );
        let token_json = serde_json::to_string(&token).unwrap();
        let mut policy = PublicWebPolicyV2 {
            schema: PUBLIC_WEB_POLICY_SCHEMA_V2.into(),
            policy_id: "synthetic-web".into(),
            revision: 1,
            owner: owner.clone(),
            capability_token_id: token.id.to_string(),
            capability_token_sha256: format!("{:x}", Sha256::digest(token_json.as_bytes())),
            issuer_public_key_hex: issuer.export_public_key().to_hex(),
            operations: vec![PublicWebOperationV2::Search, PublicWebOperationV2::Page],
            limits: PublicWebLimitsV2::default(),
            search_api: None,
            issued_at_unix_ms: now,
            expires_at_unix_ms: now.saturating_add(600_000),
            approval_audit_refs: vec!["synthetic-approval".into()],
            signature_hex: String::new(),
        };
        policy.signature_hex =
            hex::encode(issuer.sign(&policy.signing_bytes().unwrap()).as_bytes());
        let mut state = PublicWebPolicyStateV2 {
            schema: PUBLIC_WEB_STATE_SCHEMA_V2.into(),
            policy_id: policy.policy_id.clone(),
            policy_sha256: policy.sha256(),
            revision: 1,
            status: PublicWebPolicyStatusV2::Active,
            updated_at_unix_ms: now,
            signer_public_key_hex: issuer.export_public_key().to_hex(),
            signature_hex: String::new(),
        };
        state.signature_hex = hex::encode(issuer.sign(&state.signing_bytes().unwrap()).as_bytes());
        let config = PublicWebConfig {
            schema: astrid_public_web::CONFIG_SCHEMA.into(),
            delegation_root: path.join("delegation"),
            policy_path: path.join("policy.json"),
            state_path: path.join("state.json"),
            state_floor_path: path.join("floor.json"),
            owner_lock_dir: path.join("locks"),
            storage_path: path.join("storage"),
            expected_owner: owner.clone(),
            expected_policy_sha256: policy.sha256(),
            minimum_state_revision: 1,
            brave: None,
        };
        write(
            &config.delegation_root.join("trust.json"),
            &json!({
                "schema":"astrid.volition.delegated_trust.v1", "target_being":"astrid",
                "trusted_issuer_public_keys":[issuer.export_public_key().to_hex()],
                "bootstrap_audit_refs":["synthetic-bootstrap"], "authorized_at_unix_ms":now
            }),
        );
        let mut approval = PublicWebApprovalV2 {
            schema: PUBLIC_WEB_APPROVAL_SCHEMA_V2.into(),
            operation: PublicWebMutationV2::Install,
            owner: owner.clone(),
            policy_sha256: policy.sha256(),
            state_sha256: state.sha256(),
            config_sha256: config.sha256(),
            expected_previous_floor_sha256: None,
            approved_at_unix_ms: now,
            expires_at_unix_ms: now.saturating_add(60_000),
            approval_audit_ref: "synthetic-explicit-install".into(),
            signer_public_key_hex: issuer.export_public_key().to_hex(),
            signature_hex: String::new(),
        };
        approval.signature_hex =
            hex::encode(issuer.sign(&approval.signing_bytes().unwrap()).as_bytes());
        mutate(
            &config,
            &owner,
            &PublicWebBundle {
                policy,
                state,
                capability_token_json: token_json,
            },
            &approval,
        )
        .unwrap();
        let previous =
            SELECTED.with(|selected| selected.replace(Some(Ok((config.clone(), owner)))));
        Self {
            _root: root,
            previous,
            config,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        SELECTED.with(|selected| selected.replace(self.previous.take()));
    }
}
