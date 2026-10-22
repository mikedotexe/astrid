use tempfile::tempdir;

use super::*;

fn handled_outcome() -> NextActionOutcome {
    NextActionOutcome::handled("self_control", "applied")
}

#[test]
fn local_action_produces_exact_intent_decision_and_receipt_chain() {
    let root = tempdir().unwrap();
    let start = begin_astrid_next_at_root(
        root.path(),
        "I want a little more room.\nNEXT: APERTURE 1.2",
        "exchange-1",
        "dialogue_live",
        "APERTURE 1.2",
        10_000,
    )
    .unwrap();
    assert!(matches!(start, AstridVolitionStartV1::Accepted(_)));
    let context =
        exact_self_owned_action_context_at_root(root.path(), "APERTURE 1.2", 10_000).unwrap();
    assert!(
        context
            .source_attestation_id
            .starts_with("astrid-attestation-")
    );
    let summary = match start {
        AstridVolitionStartV1::Accepted(accepted) => {
            accepted.complete(&handled_outcome(), 10_001).unwrap()
        },
        AstridVolitionStartV1::Shadowed(_) => unreachable!(),
    };
    assert!(summary.contains("not proof of completed execution"));
    assert_eq!(
        fs::read_dir(root.path().join("attestations"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        fs::read_dir(root.path().join("receipts")).unwrap().count(),
        1
    );
    assert_eq!(
        fs::read_dir(root.path().join("pending")).unwrap().count(),
        0
    );
}

#[test]
fn exact_dispatch_permit_rejects_substitution_and_expiry() {
    let root = tempdir().unwrap();
    let _start = begin_astrid_next_at_root(
        root.path(),
        "NEXT: OWNER_POLICY_STATUS",
        "exchange-permit",
        "dialogue_live",
        "OWNER_POLICY_STATUS",
        50_000,
    )
    .unwrap();
    assert!(
        exact_self_owned_action_context_at_root(root.path(), "OWNER_POLICY_STATUS all", 50_001)
            .is_err()
    );
    assert!(
        exact_self_owned_action_context_at_root(root.path(), "OWNER_POLICY_STATUS", 80_001)
            .is_err()
    );
}

#[test]
fn shared_coupling_stays_shadow_only_without_mutual_authority() {
    let root = tempdir().unwrap();
    let start = begin_astrid_next_at_root(
        root.path(),
        "Let's breathe together.\nNEXT: BREATHE_TOGETHER",
        "exchange-2",
        "dialogue_live",
        "BREATHE_TOGETHER",
        20_000,
    )
    .unwrap();
    assert!(matches!(start, AstridVolitionStartV1::Shadowed(_)));
    assert!(start.dispatch_block_reason().is_some());
    assert_eq!(
        fs::read_dir(root.path().join("intents")).unwrap().count(),
        0
    );
    let shadow = fs::read_dir(root.path().join("shadow"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let payload: serde_json::Value = serde_json::from_slice(&fs::read(shadow).unwrap()).unwrap();
    assert_eq!(payload["authority_granted"], false);
    assert_eq!(payload["consent_inferred_from_silence"], false);
}

#[test]
fn old_counted_search_grant_does_not_become_standing_web_authority() {
    let root = tempdir().unwrap();
    let signer = load_or_provision_identity(root.path(), 24_000).unwrap();
    let deployment = crate::signal_spine::signal_deployment_identity_v1();
    let _binding_id = delegated_capability::install_test_search_grant(
        root.path(),
        &signer.public_key_hex,
        &deployment,
        24_000,
    );
    let start = begin_astrid_next_at_root(
        root.path(),
        "I want to look this up.\nNEXT: SEARCH reservoir continuity",
        "exchange-delegated",
        "dialogue_live",
        "SEARCH reservoir continuity",
        24_001,
    )
    .unwrap();
    assert!(matches!(start, AstridVolitionStartV1::Shadowed(_)));
}

#[test]
fn standing_web_preview_accepts_repeated_choices_without_a_lifetime_count() {
    let web = crate::public_web::fixture::Fixture::new();
    let root = tempdir().unwrap();
    for index in 0..3 {
        let start = begin_astrid_next_at_root(
            root.path(),
            "NEXT: SEARCH CONTROL",
            &format!("web-{index}"),
            "dialogue_live",
            "SEARCH CONTROL",
            24_001,
        )
        .unwrap();
        let AstridVolitionStartV1::Accepted(accepted) = start else {
            panic!("signed standing policy should permit dispatch")
        };
        assert!(accepted.intent.capability_binding_ids[0].starts_with("public_web_policy_v2:"));
        assert_eq!(
            accepted.intent.authority_class,
            VolitionAuthorityClassV1::DelegatedExternal
        );
        assert_eq!(accepted.intent.budget.action_count, None);
        let summary = accepted.complete(&handled_outcome(), 24_002).unwrap();
        assert!(summary.contains("not proof of completed execution"));
    }
    std::fs::write(&web.config.state_path, b"corrupt").unwrap();
    let start = begin_astrid_next_at_root(
        root.path(),
        "NEXT: SEARCH CONTROL",
        "web-after-corruption",
        "dialogue_live",
        "SEARCH CONTROL",
        24_003,
    )
    .unwrap();
    assert!(matches!(start, AstridVolitionStartV1::Shadowed(_)));
}

#[test]
fn delegated_search_without_bootstrap_grant_remains_shadow_only() {
    let root = tempdir().unwrap();
    let start = begin_astrid_next_at_root(
        root.path(),
        "NEXT: SEARCH reservoir continuity",
        "exchange-no-grant",
        "dialogue_live",
        "SEARCH reservoir continuity",
        25_000,
    )
    .unwrap();
    assert!(matches!(start, AstridVolitionStartV1::Shadowed(_)));
    assert!(start.dispatch_block_reason().is_some());
}

#[test]
fn mirrored_or_fallback_text_is_not_attested_as_being_authorship() {
    let root = tempdir().unwrap();
    for mode in [
        "mirror",
        "dialogue_fallback",
        "witness",
        "introspect_notice",
        "self_study_carriage_notice",
    ] {
        assert!(
            begin_astrid_next_at_root(
                root.path(),
                "NEXT: REST",
                "exchange-3",
                mode,
                "REST",
                30_000,
            )
            .is_err()
        );
    }
}

#[cfg(unix)]
#[test]
fn volition_artifacts_are_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = tempdir().unwrap();
    let start = begin_astrid_next_at_root(
        root.path(),
        "NEXT: REST",
        "exchange-4",
        "dialogue_live",
        "REST",
        40_000,
    )
    .unwrap();
    if let AstridVolitionStartV1::Accepted(accepted) = start {
        accepted.complete(&handled_outcome(), 40_001).unwrap();
    }
    let identity_mode = fs::metadata(root.path().join("identity.json"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(identity_mode, 0o600);
    let root_mode = fs::metadata(root.path()).unwrap().permissions().mode() & 0o777;
    assert_eq!(root_mode, 0o700);
}
