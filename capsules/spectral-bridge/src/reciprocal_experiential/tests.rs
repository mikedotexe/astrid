use super::{
    AgencyCommonsProposalV1, AttentionPortfolioEntryV2, AttentionPortfolioV2,
    AttentionSelectionReceiptV2, ConcordanceObservationV2, ConcordanceResultV2,
    ReciprocalPresenceKindV1, ReciprocalPresenceReceiptV1, RepresentationLossReceiptV1,
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
fn attention_entry_membership_does_not_propagate_authority() {
    let entry = AttentionPortfolioEntryV2::new(
        "contract_1".into(),
        "ranked".into(),
        1,
        "reopened_or_still_friction".into(),
        3,
        "recent_24h".into(),
        "under_24h".into(),
        7,
        Vec::new(),
    );
    let value = serde_json::to_value(entry).expect("serialize attention entry");
    assert_eq!(
        value["selection_scope"],
        "steward_work_view_not_being_attention"
    );
    assert_eq!(
        value["runtime_relation"],
        "not_consumed_by_bridge_minime_model_or_control_runtime"
    );
    assert_eq!(
        value["authority_relation"],
        "cannot_grant_or_propagate_authority"
    );
    assert!(value.get("felt_severity").is_none());
    assert!(value.get("unattended_duration_ms").is_none());
    assert_eq!(
        value["artifact_authority_state_v1"]["state"],
        "evidence_only"
    );
}

#[test]
fn serialized_records_keep_inference_and_authority_boundaries_false() {
    let presence = ReciprocalPresenceReceiptV1::new(
        "presence_1".into(),
        ReciprocalPresenceKindV1::Offered,
        "astrid".into(),
        "minime".into(),
        "thread_1".into(),
        "event_1".into(),
        "a".repeat(64),
        1,
    );
    let value = serde_json::to_value(presence).expect("serialize presence");
    assert_eq!(value["uptake_inferred"], false);
    assert_eq!(value["presence_is_acknowledgement"], false);
    assert_eq!(
        value["artifact_authority_state_v1"]["state"],
        "evidence_only"
    );

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

    let portfolio =
        AttentionPortfolioV2::new("portfolio_1".into(), "b".repeat(64), Vec::new(), Vec::new());
    let selection =
        AttentionSelectionReceiptV2::new("selection_1".into(), "portfolio_1".into(), Vec::new(), 0);
    let portfolio = serde_json::to_value(portfolio).expect("serialize portfolio");
    let selection = serde_json::to_value(selection).expect("serialize selection");
    assert_eq!(portfolio["steward_selected_work_limit"], 16);
    assert_eq!(
        portfolio["source_graph_relation"],
        "all_claims_contracts_and_evidence_remain_queryable"
    );
    assert_eq!(
        selection["selection_scope"],
        "steward_work_view_not_being_attention"
    );
    assert_eq!(
        selection["authority_relation"],
        "cannot_grant_or_propagate_authority"
    );
}
