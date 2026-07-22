use super::*;

#[test]
fn artifact_binding_is_mechanical_and_does_not_map_qualitative_state() {
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
    let binding = &encoded["canonical_body_binding_v1"];
    assert_eq!(binding["canonical_body_sha256"], sha256_bytes(body));
    assert_eq!(binding["canonical_body_byte_count"], body.len());
    assert_eq!(
        binding["binding_scope"],
        "artifact_byte_integrity_only_not_texture_experience_stability_freezing_or_control"
    );
    assert!(encoded.get("qualitative_evidence_boundary_v1").is_none());
    assert!(encoded.get("subjective_continuity_v1").is_none());
    assert!(encoded.get("felt_scalar_divergence_relation").is_none());

    let sidecar = serde_json::to_string(&encoded).expect("serialize sidecar");
    assert!(!sidecar.contains("viscous-persistence"));
    assert!(!sidecar.contains("qualitative_evidence_status"));
    assert!(!sidecar.contains("measurement_contract_state"));
    assert!(artifact.ends_with(body));
}

#[test]
fn canonical_body_integrity_verifies_bytes_without_persisting_them() {
    let body = b"blurry and sharp remain distinct in Astrid's own report";
    let binding =
        super::types::LivedStateCanonicalBodyBindingV1::new(sha256_bytes(body), body.len());
    assert!(binding.verify_integrity(body));
    assert!(!binding.verify_integrity(&body[..body.len() - 1]));
}

#[test]
fn protected_notice_does_not_claim_canonical_body() {
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
    assert!(encoded["canonical_body_binding_v1"].is_null());
    assert!(encoded.get("qualitative_evidence_boundary_v1").is_none());
}
