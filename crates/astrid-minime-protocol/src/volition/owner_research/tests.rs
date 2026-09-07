use ed25519_dalek::{Signer as _, SigningKey};

use crate::SelfControlValuesV2;

use super::*;

fn hash(seed: &str) -> String {
    format!("{:x}", Sha256::digest(seed.as_bytes()))
}

fn metric_node(id: &str, value: f64) -> OwnerEvidenceNodeV1 {
    OwnerEvidenceNodeV1 {
        node_id: id.to_string(),
        kind: OwnerEvidenceNodeKindV1::Analysis,
        claim: OwnerEvidenceClaimV1::MachineEstablished,
        scope: OwnerEvidenceScopeV1::aggregate(),
        evidence_sha256: hash(id),
        source_refs: vec![format!("source-{id}")],
        metrics: BTreeMap::from([("pressure".to_string(), value)]),
        observation_phase: None,
        raw_content_owner_only: true,
    }
}

fn graph(nodes: Vec<OwnerEvidenceNodeV1>) -> OwnerEvidenceGraphV1 {
    OwnerEvidenceGraphV1 {
        schema: OWNER_EVIDENCE_GRAPH_SCHEMA_V1.to_string(),
        graph_id: "graph-1".to_string(),
        inquiry_id: "inquiry-1".to_string(),
        inquiry_receipt_sha256: hash("receipt"),
        owner_being: "astrid".to_string(),
        revision: 1,
        nodes,
        edges: Vec::new(),
        graph_head_sha256: String::new(),
        created_at_unix_ms: 10,
        updated_at_unix_ms: 10,
    }
    .seal()
}

fn control(value: f32) -> OwnerCanaryControlV2 {
    OwnerCanaryControlV2::new(
        SelfControlFamilyV2::Conversation,
        SelfControlValuesV2 {
            aperture: Some(value),
            ..SelfControlValuesV2::default()
        },
        0,
    )
}

#[test]
fn decision_evaluation_matches_once_and_fails_closed_on_ambiguity() {
    let evidence = graph(vec![metric_node("analysis-1", 0.7)]);
    assert!(evidence.is_well_formed());
    let mut plan = OwnerDecisionPlanV1 {
        schema: OWNER_DECISION_PLAN_SCHEMA_V1.to_string(),
        plan_id: "plan-1".to_string(),
        inquiry_id: evidence.inquiry_id.clone(),
        owner_being: "astrid".to_string(),
        inquiry_receipt_sha256: evidence.inquiry_receipt_sha256.clone(),
        capability_manifest_sha256: hash("capabilities"),
        revision: 1,
        branches: vec![OwnerDecisionBranchV1 {
            branch_id: "high-pressure".to_string(),
            predicates: vec![OwnerEvidencePredicateV1 {
                metric: "pressure".to_string(),
                scope: OwnerEvidenceScopeV1::aggregate(),
                reducer: None,
                comparator: OwnerEvidenceComparatorV1::GreaterOrEqual,
                threshold: 0.6,
            }],
            duration_secs: 120,
            controls: vec![control(0.8)],
        }],
        owner_authored: true,
        runtime_may_select_values: false,
        safety_may_only_hold_or_revert: true,
        created_at_unix_ms: 10,
        expires_at_unix_ms: 1_000,
    };
    let matched = plan.evaluate(&evidence, 20).unwrap();
    assert_eq!(matched.status, OwnerDecisionEvaluationStatusV1::Matched);
    plan.branches.push(OwnerDecisionBranchV1 {
        branch_id: "unconditional".to_string(),
        predicates: Vec::new(),
        duration_secs: 120,
        controls: vec![control(0.5)],
    });
    assert_eq!(
        plan.evaluate(&evidence, 20).unwrap().status,
        OwnerDecisionEvaluationStatusV1::Ambiguous
    );
}

#[test]
fn intervention_claim_requires_baseline_two_samples_and_post_rollback() {
    let mut intervention = metric_node("intervention", 0.7);
    intervention.claim = OwnerEvidenceClaimV1::InterventionSupported;
    assert!(!graph(vec![intervention.clone()]).is_well_formed());
    let mut nodes = vec![intervention];
    for (index, phase) in [
        InquiryObservationPhaseV1::Baseline,
        InquiryObservationPhaseV1::DuringSampleOne,
        InquiryObservationPhaseV1::DuringSampleTwo,
        InquiryObservationPhaseV1::PostRollback,
    ]
    .into_iter()
    .enumerate()
    {
        nodes.push(OwnerEvidenceNodeV1 {
            node_id: format!("observation-{index}"),
            kind: if phase == InquiryObservationPhaseV1::PostRollback {
                OwnerEvidenceNodeKindV1::Rollback
            } else {
                OwnerEvidenceNodeKindV1::Observation
            },
            claim: OwnerEvidenceClaimV1::MachineEstablished,
            scope: OwnerEvidenceScopeV1::aggregate(),
            evidence_sha256: hash(&format!("observation-{index}")),
            source_refs: Vec::new(),
            metrics: BTreeMap::new(),
            observation_phase: Some(phase),
            raw_content_owner_only: true,
        });
    }
    assert!(graph(nodes).is_well_formed());
}

#[test]
fn signed_research_receipt_detects_payload_and_signature_tampering() {
    let key = SigningKey::from_bytes(&[21; 32]);
    let public_key_hex = hex::encode(key.verifying_key().to_bytes());
    let mut receipt = SignedOwnerResearchReceiptV1 {
        schema: SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: "research-receipt-1".to_string(),
        payload_kind: OwnerResearchPayloadKindV1::EvidenceGraph,
        payload_schema: OWNER_EVIDENCE_GRAPH_SCHEMA_V1.to_string(),
        payload_sha256: hash("payload"),
        owner_being: "astrid".to_string(),
        process_identity: "astrid-runtime-test".to_string(),
        deployment_identity: "astrid-deployment-test".to_string(),
        signer_public_key_fingerprint_sha256: public_key_fingerprint_sha256(&public_key_hex)
            .unwrap(),
        signer_public_key_hex: public_key_hex,
        previous_receipt_sha256: None,
        emitted_at_unix_ms: 10,
        signature_hex: String::new(),
    };
    receipt.signature_hex = hex::encode(key.sign(&receipt.signing_bytes().unwrap()).to_bytes());
    assert!(receipt.is_well_formed());
    receipt.payload_sha256 = hash("tampered");
    assert!(!receipt.is_well_formed());
}
