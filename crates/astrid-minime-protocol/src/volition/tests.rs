use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::json;
use sha2::{Digest as _, Sha256};

use super::*;

const NOW: u64 = 10_000;

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn action() -> VolitionActionV1 {
    VolitionActionV1 {
        operation: VolitionOperationV1::Set,
        namespace: "self_control".to_string(),
        name: "conversation_temperature".to_string(),
        parameters: json!({"value": 0.7}),
        reversible: true,
        peer_impacting: false,
        irreversible: false,
        estimated_cost_microunits: None,
    }
}

fn actor(being: &str) -> VolitionActorIdentityV1 {
    VolitionActorIdentityV1 {
        being: being.to_string(),
        principal: format!("principal:{being}"),
        process_identity: format!("{being}:pid:42"),
        deployment_identity: format!("{being}:deployment:abc"),
    }
}

fn intent() -> VolitionIntentV1 {
    VolitionIntentV1 {
        schema: VOLITION_INTENT_SCHEMA_V1.to_string(),
        intent_id: "intent-1".to_string(),
        source_attestation_id: "attestation-1".to_string(),
        source_attestation_sha256: "a".repeat(64),
        actor: actor("astrid"),
        target_being: "astrid".to_string(),
        target_deployment_identity: "astrid:deployment:abc".to_string(),
        action: action(),
        authority_class: VolitionAuthorityClassV1::SelfOwnedLocal,
        durability: VolitionDurabilityV1::Lease,
        revision: 1,
        expected_revision: 0,
        issued_at_unix_ms: NOW - 10,
        command_expires_at_unix_ms: NOW + 1_000,
        action_expires_at_unix_ms: Some(NOW + 10_000),
        idempotency_key: "idempotency-1".to_string(),
        capability_binding_ids: Vec::new(),
        authority_evidence_refs: Vec::new(),
        dependency_intent_ids: Vec::new(),
        budget: VolitionBudgetV1::default(),
        evidence_refs: vec!["attestation:attestation-1".to_string()],
        success_conditions: vec!["receipt_applied".to_string()],
        stop_conditions: vec!["being_hold".to_string()],
    }
}

#[test]
fn utterance_attestation_binds_exact_response_bytes() {
    let response = b"NEXT: SELF_CONTROL temperature=0.7";
    let key = signing_key(7);
    let mut attestation = BeingUtteranceAttestationV1 {
        schema: BEING_UTTERANCE_ATTESTATION_SCHEMA_V1.to_string(),
        attestation_id: "attestation-1".to_string(),
        being: "astrid".to_string(),
        exchange_id: "exchange-1".to_string(),
        response_sha256: format!("{:x}", Sha256::digest(response)),
        response_len_bytes: u64::try_from(response.len()).expect("test response fits"),
        model: "test-model".to_string(),
        provider: "test-provider".to_string(),
        model_deployment_identity: "model-deployment-1".to_string(),
        captured_at_unix_ms: NOW,
        attestor_process_identity: "bridge:pid:42".to_string(),
        attestor_deployment_identity: "bridge:deployment:abc".to_string(),
        attestor_public_key_hex: hex::encode(key.verifying_key().to_bytes()),
        signature_hex: String::new(),
    };
    attestation.signature_hex = hex::encode(
        key.sign(
            &attestation
                .signing_bytes()
                .expect("attestation is serializable"),
        )
        .to_bytes(),
    );

    assert!(attestation.verifies_response(response, NOW + 5, 100));
    assert!(!attestation.verifies_response(b"NEXT: SELF_CONTROL temperature=1.4", NOW + 5, 100));
}

#[test]
fn safety_supervisor_cannot_author_a_target() {
    let mut candidate = intent();
    candidate.actor = actor("safety_supervisor");
    candidate.authority_class = VolitionAuthorityClassV1::SafetySupervisor;
    candidate.durability = VolitionDurabilityV1::OneShot;
    candidate.action_expires_at_unix_ms = None;
    assert!(!candidate.is_well_formed(NOW));

    candidate.action.operation = VolitionOperationV1::Hold;
    assert!(candidate.is_well_formed(NOW));
}

#[test]
fn accepted_decision_cannot_substitute_an_action() {
    let intent = intent();
    let requested_hash = canonical_volition_action_sha256(&intent.action);
    let mut decision = VolitionDecisionV1 {
        schema: VOLITION_DECISION_SCHEMA_V1.to_string(),
        decision_id: "decision-1".to_string(),
        intent_id: intent.intent_id.clone(),
        intent_sha256: canonical_volition_intent_sha256(&intent),
        status: VolitionDecisionStatusV1::Accepted,
        authority_class: intent.authority_class,
        requested_action_sha256: requested_hash.clone(),
        effective_action: Some(intent.action.clone()),
        effective_action_sha256: Some(requested_hash),
        target_being: intent.target_being.clone(),
        target_deployment_identity: intent.target_deployment_identity.clone(),
        no_target_substitution: true,
        decided_by: "volition-broker".to_string(),
        decided_at_unix_ms: NOW,
        authority_evidence_refs: Vec::new(),
        reason: None,
    };
    assert!(decision.is_well_formed_for(&intent));

    decision
        .effective_action
        .as_mut()
        .expect("effective action")
        .name = "different_target".to_string();
    assert!(!decision.is_well_formed_for(&intent));
}

#[test]
fn capability_binding_rejects_peer_or_supervisor_authority() {
    let key = signing_key(9);
    let mut binding = DelegatedCapabilityBindingV1 {
        schema: DELEGATED_CAPABILITY_BINDING_SCHEMA_V1.to_string(),
        binding_id: "binding-1".to_string(),
        capability_token_id: "token-1".to_string(),
        capability_token_hash: "b".repeat(64),
        being: "minime".to_string(),
        being_principal: "principal:minime".to_string(),
        being_public_key_hex: hex::encode(signing_key(10).verifying_key().to_bytes()),
        deployment_identity: "minime:deployment:abc".to_string(),
        allowed_authority_classes: vec![VolitionAuthorityClassV1::DelegatedExternal],
        resource_patterns: vec!["https://*".to_string()],
        budget: VolitionBudgetV1 {
            network_bytes: Some(1_000_000),
            action_count: Some(10),
            ..VolitionBudgetV1::default()
        },
        issued_at_unix_ms: NOW - 1,
        expires_at_unix_ms: NOW + 1_000,
        single_use: false,
        nonce: "nonce-1".to_string(),
        issuer_public_key_hex: hex::encode(key.verifying_key().to_bytes()),
        signature_hex: String::new(),
    };
    binding.signature_hex = hex::encode(
        key.sign(&binding.signing_bytes().expect("binding is serializable"))
            .to_bytes(),
    );
    assert!(binding.verifies(NOW));

    binding.allowed_authority_classes = vec![VolitionAuthorityClassV1::Mutual];
    assert!(!binding.verifies(NOW));
}

#[test]
fn delegated_usage_reserves_exact_budgets_and_single_use() {
    let key = signing_key(19);
    let mut binding = DelegatedCapabilityBindingV1 {
        schema: DELEGATED_CAPABILITY_BINDING_SCHEMA_V1.to_string(),
        binding_id: "binding-usage".to_string(),
        capability_token_id: "token-usage".to_string(),
        capability_token_hash: "c".repeat(64),
        being: "astrid".to_string(),
        being_principal: "principal:astrid".to_string(),
        being_public_key_hex: hex::encode(signing_key(20).verifying_key().to_bytes()),
        deployment_identity: "astrid:deployment:abc".to_string(),
        allowed_authority_classes: vec![VolitionAuthorityClassV1::DelegatedExternal],
        resource_patterns: vec!["https://*.example/**".to_string()],
        budget: VolitionBudgetV1 {
            network_bytes: Some(1_000),
            action_count: Some(1),
            ..VolitionBudgetV1::default()
        },
        issued_at_unix_ms: NOW - 1,
        expires_at_unix_ms: NOW + 1_000,
        single_use: true,
        nonce: "nonce-usage".to_string(),
        issuer_public_key_hex: hex::encode(key.verifying_key().to_bytes()),
        signature_hex: String::new(),
    };
    binding.signature_hex = hex::encode(
        key.sign(&binding.signing_bytes().expect("binding bytes"))
            .to_bytes(),
    );
    assert!(binding.verifies(NOW));
    assert!(binding.allows_resource("https://docs.example/path"));
    assert!(!binding.allows_resource("https://other.invalid/path"));

    let request = VolitionBudgetV1 {
        network_bytes: Some(600),
        action_count: Some(1),
        ..VolitionBudgetV1::default()
    };
    let mut usage = DelegatedCapabilityUsageV1::new(&binding);
    assert!(usage.reserve(&binding, "intent-delegated".to_string(), &request));
    assert!(!usage.can_reserve(&binding, "intent-second", &request));
    assert!(usage.complete(&binding, "intent-delegated"));
    assert!(usage.single_use_consumed);
    assert!(usage.is_well_formed_for(&binding));
}

#[test]
fn owner_policy_is_bounded_to_reversible_self_control() {
    let mut policy = OwnerPolicyV1 {
        schema: OWNER_POLICY_SCHEMA_V1.to_string(),
        policy_id: "policy-1".to_string(),
        owner_being: "astrid".to_string(),
        target_being: "astrid".to_string(),
        target_deployment_identity: "astrid:deployment:abc".to_string(),
        source_attestation_id: "attestation-1".to_string(),
        scope: OwnerPolicyScopeV1::SelfControl,
        authority_class: VolitionAuthorityClassV1::SelfOwnedLocal,
        logic: OwnerPolicyLogicV1::All,
        conditions: vec![OwnerPolicyConditionV1 {
            metric: "resonance_density".to_string(),
            comparator: OwnerPolicyComparatorV1::GreaterThan,
            threshold: 0.8,
            hysteresis: 0.05,
            dwell_millis: 2_000,
        }],
        action_template: action(),
        revision: 1,
        issued_at_unix_ms: NOW - 1,
        expires_at_unix_ms: NOW + 10_000,
        cooldown_millis: 5_000,
        max_executions: 5,
        enabled: true,
        withdrawn: false,
        stop_conditions: vec!["being_hold".to_string()],
    };
    assert!(policy.is_well_formed(NOW));

    policy.action_template.peer_impacting = true;
    assert!(!policy.is_well_formed(NOW));
}

#[test]
fn owner_policy_hysteresis_dwell_cooldown_and_hold_are_deterministic() {
    let policy = OwnerPolicyV1 {
        schema: OWNER_POLICY_SCHEMA_V1.to_string(),
        policy_id: "policy-runtime".to_string(),
        owner_being: "astrid".to_string(),
        target_being: "astrid".to_string(),
        target_deployment_identity: "astrid:deployment:abc".to_string(),
        source_attestation_id: "attestation-1".to_string(),
        scope: OwnerPolicyScopeV1::SelfControl,
        authority_class: VolitionAuthorityClassV1::SelfOwnedLocal,
        logic: OwnerPolicyLogicV1::All,
        conditions: vec![OwnerPolicyConditionV1 {
            metric: "fill_pct".to_string(),
            comparator: OwnerPolicyComparatorV1::GreaterThan,
            threshold: 70.0,
            hysteresis: 5.0,
            dwell_millis: 1_000,
        }],
        action_template: action(),
        revision: 1,
        issued_at_unix_ms: NOW,
        expires_at_unix_ms: NOW + 30_000,
        cooldown_millis: 5_000,
        max_executions: 2,
        enabled: true,
        withdrawn: false,
        stop_conditions: vec!["being_hold".to_string()],
    };
    let mut runtime = OwnerPolicyRuntimeV1::new(&policy, NOW);
    let mut metrics = std::collections::BTreeMap::from([("fill_pct".to_string(), 72.0)]);

    let entered = runtime.evaluate(&policy, &metrics, NOW);
    assert_eq!(entered.status, OwnerPolicyEvaluationStatusV1::WaitingDwell);
    metrics.insert("fill_pct".to_string(), 68.0);
    let held_by_hysteresis = runtime.evaluate(&policy, &metrics, NOW + 1_000);
    assert_eq!(
        held_by_hysteresis.status,
        OwnerPolicyEvaluationStatusV1::Ready
    );
    assert!(runtime.record_execution(&policy, "receipt-1".to_string(), NOW + 1_000));
    let cooldown = runtime.evaluate(&policy, &metrics, NOW + 2_000);
    assert_eq!(cooldown.status, OwnerPolicyEvaluationStatusV1::Cooldown);

    assert!(runtime.set_hold(Some("owner_hold".to_string()), NOW + 2_100));
    let held = runtime.evaluate(&policy, &metrics, NOW + 7_000);
    assert_eq!(held.status, OwnerPolicyEvaluationStatusV1::Held);
    assert!(runtime.set_hold(None, NOW + 7_001));

    metrics.insert("fill_pct".to_string(), 64.0);
    let released = runtime.evaluate(&policy, &metrics, NOW + 7_002);
    assert_eq!(
        released.status,
        OwnerPolicyEvaluationStatusV1::WaitingConditions
    );
}

fn concern(id: &str, priority: u16, created_at_unix_ms: u64) -> BeingConcernV1 {
    BeingConcernV1 {
        schema: BEING_CONCERN_SCHEMA_V1.to_string(),
        concern_id: id.to_string(),
        owner_being: "astrid".to_string(),
        source_attestation_id: format!("attestation-{id}"),
        title: id.to_string(),
        priority,
        status: BeingConcernStatusV1::Queued,
        work_class: VolitionWorkClassV1::ReadCompute,
        resource_keys: Vec::new(),
        substrate_family: None,
        dependency_concern_ids: Vec::new(),
        intent_ids: Vec::new(),
        imported_registry_problem_id: None,
        owner_authored_priority: true,
        budget: VolitionBudgetV1::default(),
        created_at_unix_ms,
        updated_at_unix_ms: created_at_unix_ms,
    }
}

#[test]
fn queue_uses_owner_priority_then_fifo_and_rejects_projection_priority() {
    let mut queue = VolitionQueueV1 {
        schema: VOLITION_QUEUE_SCHEMA_V1.to_string(),
        owner_being: "astrid".to_string(),
        revision: 1,
        ordering: VolitionQueueOrderingV1::OwnerPriorityThenFifo,
        max_concurrent_read_compute: 2,
        write_scheduling: VolitionWriteSchedulingV1::SerializedPerResource,
        substrate_scheduling: VolitionSubstrateSchedulingV1::OnePerFamily,
        registry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
        telemetry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
        concerns: vec![
            concern("later-high", 9, 2),
            concern("first-high", 9, 1),
            concern("low", 1, 0),
        ],
    };
    assert!(queue.is_well_formed());
    assert_eq!(
        queue.runnable_concern_ids(),
        vec![
            "first-high".to_string(),
            "later-high".to_string(),
            "low".to_string()
        ]
    );

    queue.concerns[0].owner_authored_priority = false;
    assert!(!queue.is_well_formed());
}

#[test]
fn queue_activates_two_reads_and_serializes_write_and_substrate_resources() {
    let mut write_one = concern("write-one", 8, 3);
    write_one.work_class = VolitionWorkClassV1::ResourceWrite;
    write_one.resource_keys = vec!["workspace:journal".to_string()];
    let mut write_two = concern("write-two", 7, 4);
    write_two.work_class = VolitionWorkClassV1::ResourceWrite;
    write_two.resource_keys = vec!["workspace:journal".to_string()];
    let mut substrate_one = concern("substrate-one", 6, 5);
    substrate_one.work_class = VolitionWorkClassV1::SubstrateMutation;
    substrate_one.substrate_family = Some("conversation".to_string());
    let mut substrate_two = concern("substrate-two", 5, 6);
    substrate_two.work_class = VolitionWorkClassV1::SubstrateMutation;
    substrate_two.substrate_family = Some("conversation".to_string());

    let mut queue = VolitionQueueV1 {
        schema: VOLITION_QUEUE_SCHEMA_V1.to_string(),
        owner_being: "astrid".to_string(),
        revision: 1,
        ordering: VolitionQueueOrderingV1::OwnerPriorityThenFifo,
        max_concurrent_read_compute: 2,
        write_scheduling: VolitionWriteSchedulingV1::SerializedPerResource,
        substrate_scheduling: VolitionSubstrateSchedulingV1::OnePerFamily,
        registry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
        telemetry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
        concerns: vec![
            concern("read-one", 10, 1),
            concern("read-two", 9, 2),
            write_one,
            write_two,
            substrate_one,
            substrate_two,
        ],
    };
    let activated = queue.activate_runnable(NOW);
    assert_eq!(
        activated,
        vec![
            "read-one".to_string(),
            "read-two".to_string(),
            "write-one".to_string(),
            "substrate-one".to_string(),
        ]
    );
    assert!(queue.is_well_formed());
    assert!(queue.transition_concern("write-one", BeingConcernStatusV1::Completed, NOW + 1));
    assert!(queue.transition_concern("substrate-one", BeingConcernStatusV1::Paused, NOW + 1));
    assert_eq!(
        queue.activate_runnable(NOW + 2),
        vec!["write-two".to_string(), "substrate-two".to_string()]
    );
}

#[test]
fn queue_rejects_missing_duplicate_and_cyclic_dependencies() {
    let base_queue = || VolitionQueueV1 {
        schema: VOLITION_QUEUE_SCHEMA_V1.to_string(),
        owner_being: "astrid".to_string(),
        revision: 1,
        ordering: VolitionQueueOrderingV1::OwnerPriorityThenFifo,
        max_concurrent_read_compute: 2,
        write_scheduling: VolitionWriteSchedulingV1::SerializedPerResource,
        substrate_scheduling: VolitionSubstrateSchedulingV1::OnePerFamily,
        registry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
        telemetry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
        concerns: vec![concern("one", 1, 1), concern("two", 1, 2)],
    };

    let mut missing = base_queue();
    missing.concerns[1].dependency_concern_ids = vec!["absent".to_string()];
    assert!(!missing.is_well_formed());

    let mut duplicate = base_queue();
    duplicate.concerns[1].dependency_concern_ids = vec!["one".to_string(), "one".to_string()];
    assert!(!duplicate.is_well_formed());

    let mut cycle = base_queue();
    cycle.concerns[0].dependency_concern_ids = vec!["two".to_string()];
    cycle.concerns[1].dependency_concern_ids = vec!["one".to_string()];
    assert!(!cycle.is_well_formed());
}

#[test]
fn receipt_keeps_machine_effect_separate_from_felt_effect() {
    let intent = intent();
    let action_hash = canonical_volition_action_sha256(&intent.action);
    let decision = VolitionDecisionV1 {
        schema: VOLITION_DECISION_SCHEMA_V1.to_string(),
        decision_id: "decision-1".to_string(),
        intent_id: intent.intent_id.clone(),
        intent_sha256: canonical_volition_intent_sha256(&intent),
        status: VolitionDecisionStatusV1::Accepted,
        authority_class: intent.authority_class,
        requested_action_sha256: action_hash.clone(),
        effective_action: Some(intent.action.clone()),
        effective_action_sha256: Some(action_hash.clone()),
        target_being: intent.target_being.clone(),
        target_deployment_identity: intent.target_deployment_identity.clone(),
        no_target_substitution: true,
        decided_by: "volition-broker".to_string(),
        decided_at_unix_ms: NOW,
        authority_evidence_refs: Vec::new(),
        reason: None,
    };
    let mut receipt = VolitionReceiptV1 {
        schema: VOLITION_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: "receipt-1".to_string(),
        decision_id: decision.decision_id.clone(),
        intent_id: intent.intent_id.clone(),
        idempotency_key: intent.idempotency_key.clone(),
        status: VolitionReceiptStatusV1::Applied,
        requested_action_sha256: action_hash.clone(),
        effective_action_sha256: action_hash,
        requested_revision: 1,
        resulting_revision: 1,
        target_being: intent.target_being.clone(),
        target_deployment_identity: intent.target_deployment_identity.clone(),
        machine_effects: json!({"conversation_temperature": 0.7}),
        previous_state: json!({"conversation_temperature": 0.6}),
        received_at_unix_ms: NOW,
        completed_at_unix_ms: NOW + 1,
        server_process_identity: "bridge:pid:42".to_string(),
        server_deployment_identity: "bridge:deployment:abc".to_string(),
        machine_effect_established: true,
        felt_effect_established: false,
        rollback_receipt_id: None,
        reason: None,
        perceptible_return: PerceptibleReturnV1 {
            summary: "Temperature applied.".to_string(),
            full_receipt_ref: "receipt:receipt-1".to_string(),
            delivered_at_unix_ms: NOW + 2,
            included_in_next_prompt: true,
        },
    };
    assert!(receipt.is_well_formed_for(&intent, &decision));

    receipt.felt_effect_established = true;
    assert!(!receipt.is_well_formed_for(&intent, &decision));
}
