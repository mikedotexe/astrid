use ed25519_dalek::{Signer as _, SigningKey};

use super::*;

fn signed_policy() -> (SigningKey, PublicWebPolicyV2) {
    let key = SigningKey::from_bytes(&[41; 32]);
    let mut policy = PublicWebPolicyV2 {
        schema: PUBLIC_WEB_POLICY_SCHEMA_V2.into(),
        policy_id: "public-web-fixture".into(),
        revision: 1,
        owner: PublicWebOwnerV2 {
            being: "astrid".into(),
            principal: "astrid".into(),
            public_key_hex: hex::encode(
                SigningKey::from_bytes(&[42; 32]).verifying_key().as_bytes(),
            ),
            deployment_identity: "fixture-deployment".into(),
        },
        capability_token_id: "fixture-token".into(),
        capability_token_sha256: "a".repeat(64),
        issuer_public_key_hex: hex::encode(key.verifying_key().as_bytes()),
        operations: vec![PublicWebOperationV2::Search, PublicWebOperationV2::Page],
        limits: PublicWebLimitsV2::default(),
        search_api: None,
        issued_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        approval_audit_refs: vec!["fixture:approval".into()],
        signature_hex: String::new(),
    };
    sign(&key, &mut policy);
    (key, policy)
}

fn sign(key: &SigningKey, policy: &mut PublicWebPolicyV2) {
    policy.signature_hex = hex::encode(key.sign(&policy.signing_bytes().unwrap()).to_bytes());
}

fn state(key: &SigningKey, policy: &PublicWebPolicyV2) -> PublicWebPolicyStateV2 {
    let mut state = PublicWebPolicyStateV2 {
        schema: PUBLIC_WEB_STATE_SCHEMA_V2.into(),
        policy_id: policy.policy_id.clone(),
        policy_sha256: policy.sha256(),
        revision: 2,
        status: PublicWebPolicyStatusV2::Active,
        updated_at_unix_ms: 1_001,
        signer_public_key_hex: hex::encode(key.verifying_key().as_bytes()),
        signature_hex: String::new(),
    };
    state.signature_hex = hex::encode(key.sign(&state.signing_bytes().unwrap()).to_bytes());
    state
}

#[test]
fn standing_policy_has_explicit_schema_and_no_action_counter() {
    let (_, policy) = signed_policy();
    assert!(policy.verifies(2_000));
    let wire = serde_json::to_value(&policy).unwrap();
    assert!(wire.get("action_count").is_none());
    assert!(wire.get("search_api").is_none());
    let mut old_style = wire;
    old_style["action_count"] = u64::MAX.into();
    assert!(serde_json::from_value::<PublicWebPolicyV2>(old_style).is_err());
}

#[test]
fn paid_search_binds_usage_review_without_inventing_storage_rights() {
    let (key, mut policy) = signed_policy();
    policy.search_api = Some(BraveSearchPolicyV1 {
        schema: "volition.brave_search.v1".into(),
        account_id: "fixture-account".into(),
        monthly_limit_microusd: 25_000_000,
        request_cost_microusd: 5_000,
        storage_rights_status: SearchStorageRights::Unverified,
        retention_review_audit_ref: "synthetic-operator-review".into(),
    });
    sign(&key, &mut policy);
    assert!(!policy.verifies(2_000));
    policy.schema = PUBLIC_WEB_POLICY_SCHEMA_V3.into();
    sign(&key, &mut policy);
    assert!(policy.verifies(2_000));
    assert_eq!(
        serde_json::to_value(&policy).unwrap()["search_api"]["storage_rights_status"],
        "unverified"
    );
    policy.search_api.as_mut().unwrap().storage_rights_status =
        SearchStorageRights::OperatorConfirmed;
    assert!(!policy.verifies(2_000));
    policy.search_api.as_mut().unwrap().storage_rights_status = SearchStorageRights::Unverified;
    assert!(policy.verifies(2_000));
    assert_eq!(
        policy.capability_resource(),
        PUBLIC_WEB_CAPABILITY_RESOURCE_V3
    );
    policy.search_api.as_mut().unwrap().monthly_limit_microusd = 25_000_001;
    assert!(!policy.verifies(2_000));
    policy.search_api.as_mut().unwrap().monthly_limit_microusd = 25_000_000;
    policy
        .search_api
        .as_mut()
        .unwrap()
        .retention_review_audit_ref
        .clear();
    sign(&key, &mut policy);
    assert!(!policy.verifies(2_000));
}

#[test]
fn signature_binds_owner_deployment_token_limits_and_operation() {
    let (_, policy) = signed_policy();
    for mutate in [
        |p: &mut PublicWebPolicyV2| p.owner.being = "minime".into(),
        |p: &mut PublicWebPolicyV2| p.owner.deployment_identity = "other".into(),
        |p: &mut PublicWebPolicyV2| p.capability_token_sha256 = "b".repeat(64),
        |p: &mut PublicWebPolicyV2| p.limits.decoded_body_bytes = 1,
        |p: &mut PublicWebPolicyV2| {
            p.operations.pop();
        },
    ] {
        let mut changed = policy.clone();
        mutate(&mut changed);
        assert!(!changed.verifies(2_000));
    }
}

#[test]
fn even_correctly_signed_limits_cannot_exceed_transport_ceiling() {
    let (key, policy) = signed_policy();
    for mutate in [
        |p: &mut PublicWebPolicyV2| {
            p.limits.decoded_body_bytes = PUBLIC_WEB_MAX_BODY_BYTES.saturating_add(1);
        },
        |p: &mut PublicWebPolicyV2| p.limits.wire_body_bytes = 0,
        |p: &mut PublicWebPolicyV2| p.limits.total_timeout_ms = 120_001,
        |p: &mut PublicWebPolicyV2| p.limits.redirects = 6,
        |p: &mut PublicWebPolicyV2| p.limits.search_hop_timeout_ms = 10_001,
        |p: &mut PublicWebPolicyV2| p.limits.page_hop_timeout_ms = 15_001,
        |p: &mut PublicWebPolicyV2| p.limits.storage_headroom_bytes = 0,
    ] {
        let mut changed = policy.clone();
        mutate(&mut changed);
        sign(&key, &mut changed);
        assert!(!changed.verifies(2_000));
    }
}

#[test]
fn time_schema_and_duplicate_operations_fail_closed() {
    let (key, mut policy) = signed_policy();
    assert!(!policy.verifies(999));
    assert!(!policy.verifies(10_000));
    policy.schema = "volition.capability_binding.v1".into();
    sign(&key, &mut policy);
    assert!(!policy.verifies(2_000));
    policy.schema = PUBLIC_WEB_POLICY_SCHEMA_V2.into();
    policy.operations = vec![PublicWebOperationV2::Page; 2];
    sign(&key, &mut policy);
    assert!(!policy.verifies(2_000));
}

#[test]
fn current_state_is_bound_and_revision_fenced() {
    let (key, policy) = signed_policy();
    let state = state(&key, &policy);
    assert!(state.verifies_for(&policy, 2, 2_000));
    assert!(!state.verifies_for(&policy, 3, 2_000));
    assert!(!state.verifies_for(&policy, 0, 2_000));
    assert!(!state.verifies_for(&policy, 1, 1_000));
    let mut different = policy.clone();
    different.revision = 2;
    sign(&key, &mut different);
    assert!(!state.verifies_for(&different, 1, 2_000));
}

#[test]
fn owner_can_revoke_but_cannot_activate_its_own_policy() {
    let (key, policy) = signed_policy();
    let owner = SigningKey::from_bytes(&[42; 32]);
    let mut state = state(&key, &policy);
    state.signer_public_key_hex = policy.owner.public_key_hex.clone();
    state.signature_hex = hex::encode(owner.sign(&state.signing_bytes().unwrap()).to_bytes());
    assert!(!state.verifies_for(&policy, 1, 2_000));
    state.status = PublicWebPolicyStatusV2::Revoked;
    state.signature_hex = hex::encode(owner.sign(&state.signing_bytes().unwrap()).to_bytes());
    assert!(state.verifies_for(&policy, 1, 2_000));
}

#[test]
fn owner_lock_identity_survives_deployment_and_policy_rotation() {
    let (_, policy) = signed_policy();
    let mut owner = policy.owner.clone();
    owner.deployment_identity = "replacement".into();
    assert_eq!(
        owner.lock_identity_sha256(),
        policy.owner.lock_identity_sha256()
    );
    owner.principal = "other".into();
    assert_ne!(
        owner.lock_identity_sha256(),
        policy.owner.lock_identity_sha256()
    );
}
