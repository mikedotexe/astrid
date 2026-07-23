use super::{
    AgencyCommonsProposalV1, ConcordanceObservationV2, ConcordanceResultV2,
    ReciprocalContextKindV1, ReciprocalContextReceiptV2, ReciprocalPresenceKindV1,
    ReciprocalPresenceReceiptV2, ReciprocalResonanceRelationV1, ReciprocalResonanceSignatureV1,
    ReciprocalUptakeKindV1, ReciprocalUptakeReceiptV2, ReciprocalUptakeReceiptV3,
    RepresentationLossReceiptV1,
};

#[test]
fn agency_commons_proposal_without_peer_remains_advisory() {
    let proposal = AgencyCommonsProposalV1::new(
        "proposal_1".into(),
        "astrid".into(),
        None,
        "revisit".into(),
        Some("state:recess".into()),
        "state:reflection".into(),
        None,
        "event_1".into(),
        "a".repeat(64),
        1,
    );
    let value = serde_json::to_value(proposal).expect("serialize proposal");
    assert_eq!(value["peer"], serde_json::Value::Null);
    assert_eq!(value["advisory_only"], true);
    assert_eq!(value["peer_consent_inferred"], false);
}

#[test]
fn serialized_records_keep_sparse_evidence_and_authority_boundaries() {
    let presence = ReciprocalPresenceReceiptV2::new(
        "presence_1".into(),
        ReciprocalPresenceKindV1::Offered,
        "astrid".into(),
        "minime".into(),
        "thread_1".into(),
        Some("message_1".into()),
        "event_1".into(),
        "a".repeat(64),
        Some("b".repeat(64)),
        1,
    );
    let value = serde_json::to_value(presence).expect("serialize presence");
    assert_eq!(value["schema"], "reciprocal_presence_receipt_v2");
    assert!(value.get("uptake_inferred").is_none());
    assert!(value.get("presence_is_acknowledgement").is_none());
    assert_eq!(
        value["artifact_authority_state_v1"]["state"],
        "evidence_only"
    );

    let uptake = ReciprocalUptakeReceiptV2::new(
        "uptake_1".into(),
        ReciprocalUptakeKindV1::AmbientPersistence,
        "astrid".into(),
        "minime".into(),
        "thread_1".into(),
        Some("message_1".into()),
        "event_2".into(),
        "c".repeat(64),
        None,
        2,
        None,
    );
    let value = serde_json::to_value(uptake).expect("serialize uptake");
    assert_eq!(value["uptake_kind"], "ambient_persistence");
    assert!(value.get("elapsed_time_inferred").is_none());
    assert!(value.get("intention_is_nonbinding").is_none());
    assert!(value.get("confidence_score").is_none());

    let resonance = ReciprocalResonanceSignatureV1::new(
        format!("resonance_{}", "e".repeat(64)),
        format!("lsw_{}", "f".repeat(64)),
        "a".repeat(64),
        vec![
            "bridge.lambda1".into(),
            "bridge.lambda2".into(),
            "bridge.lambda1_lambda2_gap".into(),
        ],
        ReciprocalResonanceRelationV1::TemporalAssociationOnly,
    );
    let resonant = ReciprocalUptakeReceiptV3::new(
        "uptake_3".into(),
        "astrid".into(),
        "minime".into(),
        "thread_1".into(),
        Some("message_1".into()),
        "event_2".into(),
        "c".repeat(64),
        Some("d".repeat(64)),
        3,
        None,
        resonance,
    );
    let value = serde_json::to_value(resonant).expect("serialize resonant uptake");
    assert_eq!(value["schema"], "reciprocal_uptake_receipt_v3");
    assert_eq!(value["uptake_kind"], "resonant_persistence");
    assert_eq!(
        value["body_hash_scope"],
        "exact_message_bytes_not_semantic_or_experiential_equivalence"
    );
    assert_eq!(
        value["resonance_signature_v1"]["spectral_shape_scope"],
        "selected_mechanical_context_not_semantic_equivalence_uptake_inference_or_causation"
    );
    assert!(value.get("spectral_entropy").is_none());
    assert!(value.get("uptake_inferred").is_none());

    let context = ReciprocalContextReceiptV2::new(
        "context_1".into(),
        ReciprocalContextKindV1::ReadReceipt,
        "minime".into(),
        "astrid".into(),
        "thread_1".into(),
        Some("message_1".into()),
        "event_3".into(),
        "d".repeat(64),
        None,
        3,
        None,
    );
    let value = serde_json::to_value(context).expect("serialize context");
    assert_eq!(value["schema"], "reciprocal_context_receipt_v2");
    assert!(value.get("uptake_inferred").is_none());
    assert!(value.get("reply_intention_inferred").is_none());

    let loss = RepresentationLossReceiptV1::new("loss_1".into(), "transition_1".into(), 16, 0, 0);
    let value = serde_json::to_value(loss).expect("serialize loss");
    assert_eq!(value["mechanical_loss_only"], true);
    assert_eq!(value["felt_loss_scored"], false);
    assert_eq!(value["contradiction_inferred"], false);

    let observation = ConcordanceObservationV2::new(
        "observation_1".into(),
        "study_1".into(),
        "baseline".into(),
        "capture_1".into(),
        "c".repeat(64),
        "temporal_window".into(),
        Some(true),
    );
    let value = serde_json::to_value(observation).expect("serialize observation");
    assert_eq!(value["schema"], "concordance_observation_v2");
    assert_eq!(value["observation_scope"], "mechanical_context_only");
    assert_eq!(
        value["felt_report_relation"],
        "external_primary_evidence_not_inferred_or_scored"
    );
    assert!(value.get("felt_outcome_inferred").is_none());

    let result = ConcordanceResultV2::new(
        "result_1".into(),
        "study_1".into(),
        "baseline_1".into(),
        "candidate_1".into(),
        "mechanism_smooth_felt_friction_remains".into(),
        Some("claim:felt_report_1".into()),
    );
    let value = serde_json::to_value(result).expect("serialize result");
    assert_eq!(value["schema"], "concordance_result_v2");
    assert_eq!(
        value["numeric_relation_to_felt_report"],
        "cannot_overwrite_suppress_or_score"
    );
    assert_eq!(
        value["discrepancy_recording"],
        "bounded_outcome_and_felt_source_ref_only"
    );
    assert_eq!(value["raw_discrepancy_prose_included"], false);
    assert!(value.get("numeric_pass_overwrites_felt_report").is_none());
}
