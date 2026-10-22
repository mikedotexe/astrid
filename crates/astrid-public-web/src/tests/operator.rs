use super::*;
use crate::operator::{
    Floor, PublicWebBundle, mutate, preview_mutation, publish, read_floor, status,
};

fn bundle(fixture: &Fixture) -> PublicWebBundle {
    PublicWebBundle {
        policy: fixture.policy.clone(),
        state: serde_json::from_slice(&std::fs::read(&fixture.config.state_path).unwrap()).unwrap(),
        capability_token_json: serde_json::to_string(&fixture.token).unwrap(),
    }
}

fn approval(
    fixture: &Fixture,
    bundle: &PublicWebBundle,
    operation: PublicWebMutationV2,
) -> PublicWebApprovalV2 {
    let now = crate::authority::now_ms().unwrap();
    let signer = if operation == PublicWebMutationV2::Revoke {
        &fixture.owner
    } else {
        &fixture.issuer
    };
    let mut approval = PublicWebApprovalV2 {
        schema: PUBLIC_WEB_APPROVAL_SCHEMA_V2.into(),
        operation,
        owner: fixture.policy.owner.clone(),
        policy_sha256: bundle.policy.sha256(),
        state_sha256: bundle.state.sha256(),
        config_sha256: fixture.config.sha256(),
        expected_previous_floor_sha256: read_floor(&fixture.config)
            .unwrap()
            .as_ref()
            .map(Floor::sha256),
        approved_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(60_000),
        approval_audit_ref: "fixture:explicit-operator-approval".into(),
        signer_public_key_hex: signer.export_public_key().to_hex(),
        signature_hex: String::new(),
    };
    approval.signature_hex =
        hex::encode(signer.sign(&approval.signing_bytes().unwrap()).as_bytes());
    approval
}

fn virgin() -> (Fixture, PublicWebBundle, PublicWebApprovalV2) {
    let fixture = Fixture::new();
    let bundle = bundle(&fixture);
    for path in [
        &fixture.config.policy_path,
        &fixture.config.state_path,
        &fixture.config.state_floor_path,
    ] {
        std::fs::remove_file(path).unwrap();
    }
    let approval = approval(&fixture, &bundle, PublicWebMutationV2::Install);
    (fixture, bundle, approval)
}

#[test]
fn operator_preview_is_read_only_and_install_is_exact_and_idempotent() {
    let (fixture, bundle, approval) = virgin();
    let plan =
        preview_mutation(&fixture.config, &fixture.policy.owner, &bundle, &approval).unwrap();
    assert!(!plan.already_published);
    assert!(!fixture.config.state_floor_path.exists());
    assert_eq!(
        std::fs::read_dir(&fixture.config.owner_lock_dir)
            .unwrap()
            .count(),
        0
    );
    mutate(&fixture.config, &fixture.policy.owner, &bundle, &approval).unwrap();
    assert_eq!(
        status(&fixture.config).unwrap().status,
        PublicWebPolicyStatusV2::Active
    );
    assert!(preview(&fixture.config).is_ok());
    let floor = std::fs::read(&fixture.config.state_floor_path).unwrap();
    let repeated = mutate(&fixture.config, &fixture.policy.owner, &bundle, &approval).unwrap();
    assert!(repeated.already_published);
    assert_eq!(
        floor,
        std::fs::read(&fixture.config.state_floor_path).unwrap()
    );
}

#[test]
fn owner_signed_revoke_is_inspectable_and_old_install_cannot_reactivate() {
    let (fixture, mut bundle, install) = virgin();
    mutate(&fixture.config, &fixture.policy.owner, &bundle, &install).unwrap();
    bundle.state.status = PublicWebPolicyStatusV2::Revoked;
    bundle.state.revision = 2;
    bundle.state.signer_public_key_hex = fixture.owner.export_public_key().to_hex();
    bundle.state.signature_hex = hex::encode(
        fixture
            .owner
            .sign(&bundle.state.signing_bytes().unwrap())
            .as_bytes(),
    );
    let revoke = approval(&fixture, &bundle, PublicWebMutationV2::Revoke);
    mutate(&fixture.config, &fixture.policy.owner, &bundle, &revoke).unwrap();
    assert_eq!(
        status(&fixture.config).unwrap().status,
        PublicWebPolicyStatusV2::Revoked
    );
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::Revoked
    );
    let original: PublicWebPolicyStateV2 = {
        let mut state = bundle.state.clone();
        state.status = PublicWebPolicyStatusV2::Active;
        state.revision = 1;
        state.signer_public_key_hex = fixture.issuer.export_public_key().to_hex();
        state.signature_hex = hex::encode(
            fixture
                .issuer
                .sign(&state.signing_bytes().unwrap())
                .as_bytes(),
        );
        state
    };
    bundle.state = original;
    assert_eq!(
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &install).unwrap_err(),
        PublicWebErrorCode::StaleApproval
    );
    // Restoring only an older signed state cannot defeat the durable floor.
    write_private(&fixture.config.state_path, &bundle.state);
    assert_eq!(
        preview(&fixture.config).unwrap_err(),
        PublicWebErrorCode::InvalidState
    );
}

#[test]
fn every_partial_publication_stage_stays_denied_and_exact_retry_finishes() {
    for stage in 0..4 {
        let (fixture, bundle, approval) = virgin();
        let failure = publish(
            &fixture.config,
            &fixture.policy.owner,
            &bundle,
            &approval,
            |index, _, _| {
                if index == stage {
                    Err(PublicWebErrorCode::PublicationFailed)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(failure, PublicWebErrorCode::PublicationFailed);
        assert_eq!(
            preview(&fixture.config).unwrap_err(),
            PublicWebErrorCode::StatePending
        );
        assert!(read_floor(&fixture.config).unwrap().unwrap().pending);
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &approval).unwrap();
        assert!(preview(&fixture.config).is_ok());
        assert!(!read_floor(&fixture.config).unwrap().unwrap().pending);
    }
}

#[test]
fn partial_publication_cannot_be_replaced_by_different_candidate() {
    let (fixture, mut bundle, approved) = virgin();
    publish(
        &fixture.config,
        &fixture.policy.owner,
        &bundle,
        &approved,
        |_, _, _| Err(PublicWebErrorCode::PublicationFailed),
    )
    .unwrap_err();
    bundle.state.revision = 2;
    bundle.state.signature_hex = hex::encode(
        fixture
            .issuer
            .sign(&bundle.state.signing_bytes().unwrap())
            .as_bytes(),
    );
    let different = approval(&fixture, &bundle, PublicWebMutationV2::Install);
    assert_eq!(
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &different).unwrap_err(),
        PublicWebErrorCode::InvalidState
    );
    assert!(read_floor(&fixture.config).unwrap().unwrap().pending);
}

#[test]
fn unsigned_stale_changed_path_and_wrong_runtime_approvals_never_publish() {
    let (fixture, bundle, approval) = virgin();
    let mut bad = approval.clone();
    bad.signature_hex = "0".repeat(128);
    assert_eq!(
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &bad).unwrap_err(),
        PublicWebErrorCode::InvalidApproval
    );
    bad = approval.clone();
    bad.expires_at_unix_ms = 1;
    bad.signature_hex = hex::encode(
        fixture
            .issuer
            .sign(&bad.signing_bytes().unwrap())
            .as_bytes(),
    );
    assert_eq!(
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &bad).unwrap_err(),
        PublicWebErrorCode::InvalidApproval
    );
    let mut config = fixture.config.clone();
    config.state_path = fixture.config.state_path.with_extension("other");
    assert_eq!(
        mutate(&config, &fixture.policy.owner, &bundle, &approval).unwrap_err(),
        PublicWebErrorCode::InvalidApproval
    );
    let mut owner = fixture.policy.owner.clone();
    owner.deployment_identity = "other".into();
    assert_eq!(
        mutate(&fixture.config, &owner, &bundle, &approval).unwrap_err(),
        PublicWebErrorCode::OwnerMismatch
    );
    assert!(!fixture.config.state_floor_path.exists());
    assert!(!fixture.config.policy_path.exists());
}

#[test]
fn missing_floor_does_not_adopt_orphaned_authority_files() {
    let fixture = Fixture::new();
    let bundle = bundle(&fixture);
    std::fs::remove_file(&fixture.config.state_floor_path).unwrap();
    let approval = approval(&fixture, &bundle, PublicWebMutationV2::Install);
    assert_eq!(
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &approval).unwrap_err(),
        PublicWebErrorCode::InvalidState
    );
}

#[test]
fn idempotent_install_cannot_report_success_with_missing_selected_files() {
    let (fixture, bundle, approved) = virgin();
    mutate(&fixture.config, &fixture.policy.owner, &bundle, &approved).unwrap();
    std::fs::remove_file(&fixture.config.policy_path).unwrap();
    assert_eq!(
        mutate(&fixture.config, &fixture.policy.owner, &bundle, &approved).unwrap_err(),
        PublicWebErrorCode::AuthorityUnavailable
    );
}

#[test]
fn existing_token_is_not_replaced_and_racing_token_conflict_leaves_pending() {
    use std::os::unix::fs::MetadataExt as _;
    let (fixture, bundle, approved) = virgin();
    let path = crate::authority::token_path(&fixture.config, &bundle.policy);
    let inode = std::fs::metadata(&path).unwrap().ino();
    mutate(&fixture.config, &fixture.policy.owner, &bundle, &approved).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);

    let (fixture, bundle, approved) = virgin();
    let path = crate::authority::token_path(&fixture.config, &bundle.policy);
    let result = publish(
        &fixture.config,
        &fixture.policy.owner,
        &bundle,
        &approved,
        |index, _, _| {
            if index == 0 {
                std::fs::write(&path, b"conflicting-token").unwrap();
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), PublicWebErrorCode::InvalidCapability);
    assert_eq!(std::fs::read(&path).unwrap(), b"conflicting-token");
    assert!(read_floor(&fixture.config).unwrap().unwrap().pending);
}

#[test]
fn newly_approved_exact_pending_candidate_can_resume() {
    let (fixture, bundle, approved) = virgin();
    publish(
        &fixture.config,
        &fixture.policy.owner,
        &bundle,
        &approved,
        |_, _, _| Err(PublicWebErrorCode::PublicationFailed),
    )
    .unwrap_err();
    let fresh = approval(&fixture, &bundle, PublicWebMutationV2::Install);
    assert_ne!(approved.sha256(), fresh.sha256());
    mutate(&fixture.config, &fixture.policy.owner, &bundle, &fresh).unwrap();
    assert!(preview(&fixture.config).is_ok());
}
