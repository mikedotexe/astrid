use super::*;

#[test]
fn qualitative_texture_anchor_hashes_exact_body_without_copying_it() {
    let body = b"viscous-persistence stays in Astrid's canonical language";
    let mut artifact = b"=== ASTRID INTROSPECTION ===\nHeader: value\n\n".to_vec();
    artifact.extend_from_slice(body);
    let authorship = begin_authorship_v1(None, &[], "introspection");
    let witness = build_witness_v1(
        &authorship,
        "introspection",
        "introspection_texture.txt",
        &artifact,
        None,
        Vec::new(),
        LivedStateRuntimeContextV1::default(),
    );

    let encoded = serde_json::to_value(witness).expect("serialize texture witness");
    let anchor = &encoded["qualitative_texture_anchor_v1"];
    assert_eq!(anchor["canonical_body_sha256"], sha256_bytes(body));
    assert_eq!(anchor["canonical_body_byte_count"], body.len());
    assert_eq!(
        anchor["texture_status"],
        "primary_felt_evidence_preserved_exactly_not_classified_or_scalarized"
    );
    assert_eq!(
        anchor["pregeneration_scalar_relation"],
        "pre_model_context_not_generation_trajectory_or_qualitative_weight"
    );
    assert_eq!(
        anchor["generation_interval_relation"],
        "canonical_body_authored_after_model_generation_in_call_state_change_unmeasured"
    );
    assert_eq!(
        anchor["scalar_comparison_relation"],
        "not_comparable_without_reviewed_measurement_contract"
    );
    assert_eq!(anchor["raw_prose_included"], false);
    assert_eq!(anchor["direct_causation_claimed"], false);

    let sidecar = serde_json::to_string(&encoded).expect("serialize sidecar");
    assert!(!sidecar.contains("viscous-persistence"));
    assert!(artifact.ends_with(body));
}

#[test]
fn protected_notice_does_not_claim_a_canonical_texture_anchor() {
    let artifact = b"=== ASTRID INTROSPECTION NOTICE ===\n\nprotected diagnostic";
    let authorship = begin_authorship_v1(None, &[], "thin_introspection_output");
    let witness = build_witness_v1(
        &authorship,
        "thin_introspection_output",
        "thin_introspection_output.txt",
        artifact,
        None,
        Vec::new(),
        LivedStateRuntimeContextV1::default(),
    );
    let encoded = serde_json::to_value(witness).expect("serialize notice witness");
    assert!(encoded["qualitative_texture_anchor_v1"].is_null());
}
