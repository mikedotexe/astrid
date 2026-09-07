use astrid_minime_protocol::{
    CompatibilityStatus, DIVISION_ACTION_AVAILABILITY_SCHEMA_V1, DIVISION_COMMAND_SCHEMA_V1,
    DIVISION_COMMIT_SCOPE_V1, DIVISION_READINESS_POLICY_V1, DIVISION_STATUS_SCHEMA_V1,
    DeliveryEnvelopeV1, DivisionActionV1, DivisionCommandV1, DivisionLifecycleV1,
    DivisionReadinessV1, DivisionStatusV1, EigenPacketV1, MutualAddressEnvelopeV1, OwnerInquiryV1,
    OwnerInquiryV2, SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1, SELF_CONTROL_COMMAND_SCHEMA_V2,
    SELF_CONTROL_INTENT_SCHEMA_V2, SEMANTIC_BODY_BASE_DIMENSIONS_V2,
    SEMANTIC_BODY_COMPANION_DIMENSIONS_V2, SEMANTIC_BODY_SCHEMA_V2, SelfControlActionV2,
    SelfControlAuthorityClassV2, SelfControlAuthorityProofV1, SelfControlCommandV2,
    SelfControlDurabilityV2, SelfControlFamilyV2, SelfControlIntentV2, SelfControlSourceIdentityV1,
    SelfControlValuesV2, SemanticBodyFidelityV2, SemanticBodyProvenanceV2, SemanticBodyV2,
    SemanticLaneRoleV2, SensoryDeliveryReceiptV1, SensoryDeliveryStatusV1, SensoryMsg,
    SensoryPacketV1, SensoryServerHelloV1, canonical_owner_inquiry_sha256_v2,
    canonical_self_control_intent_sha256, canonical_sensory_payload_sha256,
    owner_inquiry_fixed_analysis_set_v1,
};
use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest as _, Sha256};

const OWNER_INQUIRY_FIXTURE_SHA256: &str =
    "f8645cd12c9a8c0f405e0d6dd7884feb497a2bad45f20ef55be052f040d01752";
const OWNER_INQUIRY_V2_FIXTURE_SHA256: &str =
    "17302b92b7d68eec9019ce7f2fb97731f22f4cba3c4ab53d762726cfd161e8c9";
const OWNER_INQUIRY_V2_CANONICAL_SHA256: &str =
    "55fb17ce4c29bdb3c24bb087788e7bef75442a2ab3ef3c53d933baaafcf10dc1";
const TEXTURE_DYNAMICS_CONTRACT_FIXTURE_SHA256: &str =
    "00048207c164c07aa2ccf67cfa7119dca2246c87d4495118703fe7061d6815f9";

#[test]
fn legacy_telemetry_remains_accepted() {
    let packet: EigenPacketV1 =
        serde_json::from_str(include_str!("fixtures/legacy_eigenpacket.json")).unwrap();

    assert_eq!(
        packet.compatibility(),
        CompatibilityStatus::LegacyUnversioned
    );
    assert_eq!(packet.active_mode_count, 3);
    assert_eq!(
        packet
            .resonance_density_v1
            .unwrap()
            .texture_signature
            .movement_quality,
        "heaving"
    );
}

#[test]
fn current_telemetry_is_typed_and_preserves_additive_fields() {
    let packet: EigenPacketV1 =
        serde_json::from_str(include_str!("fixtures/current_eigenpacket.json")).unwrap();

    assert_eq!(packet.compatibility(), CompatibilityStatus::Current);
    assert_eq!(
        packet.pressure_source_v1.as_ref().unwrap().pressure_profile[0].source,
        "mode_packing"
    );
    assert!(packet.neural.is_none());
    assert_eq!(
        packet.extensions["future_additive_packet"]["preserved"],
        true
    );
    let encoded = serde_json::to_value(packet).unwrap();
    assert!(encoded.get("neural").is_some());
    assert!(encoded["neural"].is_null());
    assert_eq!(encoded["future_additive_packet"]["preserved"], true);
}

#[test]
fn owner_inquiry_fixture_round_trips_without_wire_drift() {
    let fixture = include_bytes!("fixtures/owner_inquiry_v1.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture)),
        OWNER_INQUIRY_FIXTURE_SHA256
    );
    let source: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    let inquiry: OwnerInquiryV1 = serde_json::from_value(source.clone()).unwrap();

    assert_eq!(inquiry.strands.len(), 2);
    assert!(
        inquiry
            .strands
            .iter()
            .all(|strand| strand.projection_48d.len() == 48)
    );
    assert_eq!(serde_json::to_value(inquiry).unwrap(), source);
}

#[test]
fn owner_inquiry_v2_fixture_is_valid_and_byte_pinned() {
    let fixture = include_bytes!("fixtures/owner_inquiry_v2.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture)),
        OWNER_INQUIRY_V2_FIXTURE_SHA256
    );
    let source: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    let inquiry: OwnerInquiryV2 = serde_json::from_value(source.clone()).unwrap();

    assert!(inquiry.is_well_formed());
    assert!(inquiry.preserve_all_strands_without_merge);
    assert_eq!(inquiry.analysis_plan.len(), 3);
    assert_eq!(
        canonical_owner_inquiry_sha256_v2(&inquiry),
        OWNER_INQUIRY_V2_CANONICAL_SHA256
    );
    assert_eq!(serde_json::to_value(inquiry).unwrap(), source);
}

#[test]
fn texture_dynamics_contract_is_fixed_analysis_compatible_and_byte_pinned() {
    let fixture = include_bytes!("fixtures/texture_dynamics_contract_v1.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture)),
        TEXTURE_DYNAMICS_CONTRACT_FIXTURE_SHA256
    );
    let value: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    assert_eq!(
        value["fixed_analysis_set"],
        serde_json::to_value(owner_inquiry_fixed_analysis_set_v1()).unwrap()
    );
    assert_eq!(
        value["nested_in_analysis"],
        "viscous_persistence_source_separation"
    );
    assert_eq!(
        value["raw_reservoir_mode_packing"],
        "missing_no_exact_raw_reservoir_source"
    );
    assert_eq!(
        value["shadow_dispersal"],
        "missing_no_exact_shadow_history_source"
    );
    assert_eq!(value["live_control_authority"], false);
}

#[test]
fn complete_control_surface_round_trips() {
    let packet: SensoryPacketV1 =
        serde_json::from_str(include_str!("fixtures/sensory_control_all.json")).unwrap();

    assert_eq!(packet.compatibility(), CompatibilityStatus::Current);
    match &packet.message {
        SensoryMsg::Control {
            live_audio_enabled,
            live_video_enabled,
            pi_geom_weight,
            esn_leak_authority_request_id,
            ..
        } => {
            assert_eq!(*live_audio_enabled, Some(true));
            assert_eq!(*live_video_enabled, Some(true));
            assert_eq!(*pi_geom_weight, Some(0.7));
            assert_eq!(
                esn_leak_authority_request_id.as_deref(),
                Some("fixture-request")
            );
        },
        _ => panic!("expected control packet"),
    }
    let encoded = serde_json::to_value(packet).unwrap();
    assert_eq!(encoded["kind"], "control");
    assert_eq!(encoded["protocol"]["major"], 1);
}

#[test]
fn unsupported_major_is_visible_and_incompatible() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/current_eigenpacket.json")).unwrap();
    value["protocol"]["major"] = 2.into();
    let packet: EigenPacketV1 = serde_json::from_value(value).unwrap();

    assert_eq!(
        packet.compatibility(),
        CompatibilityStatus::UnsupportedMajor
    );
    assert!(!packet.compatibility().is_compatible());
}

#[test]
fn legacy_eigenvector_landmarks_accept_additive_field_absence() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/legacy_eigenpacket.json")).unwrap();
    value["eigenvector_field"] = serde_json::json!({
        "policy": "eigenvector_field_v1",
        "direct_eigenvectors_available": true,
        "raw_vectors_exported": false,
        "mode_count": 1,
        "modes": [{"mode": 0, "top_components": [{"index": 1, "value": -0.7}]}]
    });

    let packet: EigenPacketV1 = serde_json::from_value(value).unwrap();
    let field = packet.eigenvector_field.unwrap();
    assert_eq!(field.modes[0].index, 0);
    assert!((field.modes[0].top_components[0].value + 0.7).abs() < f32::EPSILON);
    assert!(field.modes[0].top_components[0].abs.abs() < f32::EPSILON);
}

#[test]
fn legacy_packet_accepts_pre_active_mode_shape() {
    let packet: EigenPacketV1 = serde_json::from_value(serde_json::json!({
        "t_ms": 1000,
        "eigenvalues": [768.0, 300.0],
        "fill_ratio": 0.5
    }))
    .expect("pre-active-mode telemetry remains accepted");

    assert_eq!(
        packet.compatibility(),
        CompatibilityStatus::LegacyUnversioned
    );
    assert_eq!(packet.active_mode_count, 0);
    assert!(packet.active_mode_energy_ratio.abs() < f32::EPSILON);
    assert!(!packet.modalities.audio_fired);
}

#[test]
fn sensory_v1_0_encoding_remains_byte_identical_without_envelopes() {
    let encoded = serde_json::to_string(&SensoryPacketV1::versioned_1_0(SensoryMsg::Semantic {
        features: vec![0.1, -0.1],
        ts_ms: Some(42),
    }))
    .unwrap();

    assert_eq!(
        encoded,
        r#"{"protocol":{"name":"astrid_minime","major":1,"minor":0},"kind":"semantic","features":[0.1,-0.1],"ts_ms":42}"#
    );
}

#[test]
fn sensory_v1_1_remains_accepted_and_preserves_its_header() {
    let packet: SensoryPacketV1 = serde_json::from_value(serde_json::json!({
        "protocol": {"name": "astrid_minime", "major": 1, "minor": 1},
        "kind": "semantic",
        "features": [0.25, -0.25],
        "ts_ms": 42
    }))
    .expect("v1.1 packet remains decodable");

    assert_eq!(packet.compatibility(), CompatibilityStatus::Current);
    let encoded = serde_json::to_value(packet).expect("v1.1 packet remains encodable");
    assert_eq!(encoded["protocol"]["minor"], 1);
    assert!(encoded.get("delivery_v1").is_none());
    assert!(encoded.get("mutual_address_v1").is_none());
}

#[test]
fn sensory_v1_3_omits_optional_envelopes_when_absent() {
    let value = serde_json::to_value(SensoryPacketV1::versioned(SensoryMsg::Semantic {
        features: vec![0.25],
        ts_ms: None,
    }))
    .unwrap();

    assert_eq!(value["protocol"]["minor"], 3);
    assert!(value.get("delivery_v1").is_none());
    assert!(value.get("mutual_address_v1").is_none());
}

fn self_control_intent() -> SelfControlIntentV2 {
    SelfControlIntentV2 {
        schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
        intent_id: "intent-minime-fill-1".to_string(),
        actor: SelfControlSourceIdentityV1 {
            being: "minime".to_string(),
            process_identity: "minime:42".to_string(),
            deployment_identity: "minime-deployment-a".to_string(),
        },
        target_being: "minime".to_string(),
        target_deployment_identity: "minime-deployment-a".to_string(),
        family: SelfControlFamilyV2::ReservoirRegulation,
        action: SelfControlActionV2::Set,
        durability: SelfControlDurabilityV2::Lease,
        authority_class: SelfControlAuthorityClassV2::SelfOwned,
        authority_scope: "self_control.minime.reservoir_regulation".to_string(),
        revision: 7,
        expected_revision: 6,
        issued_at_unix_ms: 1_000,
        command_expires_at_unix_ms: 2_000,
        control_expires_at_unix_ms: Some(10_000),
        idempotency_key: "minime-fill-7".to_string(),
        values: SelfControlValuesV2 {
            fill_target: Some(0.68),
            regulation_strength: Some(0.45),
            ..SelfControlValuesV2::default()
        },
        related_intent_id: None,
        related_receipt_id: None,
        evidence_refs: vec!["felt-contract:contract-1".to_string()],
        success_conditions: vec!["receipt_applied".to_string()],
        stop_conditions: vec!["safety_red".to_string()],
    }
}

fn signed_self_control_proof(
    intent: &SelfControlIntentV2,
    signer_being: &str,
    signing_key: &SigningKey,
    nonce: &str,
) -> SelfControlAuthorityProofV1 {
    let mut proof = SelfControlAuthorityProofV1 {
        schema: SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1.to_string(),
        authority_class: intent.authority_class,
        signer_being: signer_being.to_string(),
        scope: intent.authority_scope.clone(),
        nonce: nonce.to_string(),
        signer_public_key_hex: hex::encode(signing_key.verifying_key().to_bytes()),
        signature_hex: String::new(),
        intent_sha256: canonical_self_control_intent_sha256(intent),
        issued_at_unix_ms: intent.issued_at_unix_ms,
        expires_at_unix_ms: intent.command_expires_at_unix_ms,
    };
    proof.signature_hex = hex::encode(
        signing_key
            .sign(
                &proof
                    .signing_bytes(intent)
                    .expect("canonical signing bytes"),
            )
            .to_bytes(),
    );
    proof
}

#[test]
fn signed_self_control_rejects_tamper_expiry_and_actor_target_mismatch() {
    let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
    let intent = self_control_intent();
    let proof = signed_self_control_proof(&intent, "minime", &signing_key, "nonce-1");
    let command = SelfControlCommandV2 {
        schema: SELF_CONTROL_COMMAND_SCHEMA_V2.to_string(),
        command_id: "command-minime-fill-7".to_string(),
        intent: intent.clone(),
        authority_proofs: vec![proof.clone()],
    };

    assert!(command.is_well_formed(1_500));
    assert!(!command.is_well_formed(2_001));

    let mut tampered = command.clone();
    tampered.intent.values.fill_target = Some(0.72);
    assert!(!tampered.is_well_formed(1_500));

    let mut crossed = command;
    crossed.intent.target_being = "astrid".to_string();
    assert!(!crossed.is_well_formed(1_500));
}

#[test]
fn lease_expiry_beyond_wire_shape_cap_is_malformed() {
    // Constitution C2: the 24h wire cap is a shape rule — no legitimate
    // sender produces day-long leases (current traffic runs 120..=1200s).
    let mut intent = self_control_intent();
    intent.control_expires_at_unix_ms = Some(1_500 + astrid_minime_protocol::MAX_LEASE_DURATION_MS);
    assert!(intent.is_well_formed(1_500));
    intent.control_expires_at_unix_ms = Some(1_501 + astrid_minime_protocol::MAX_LEASE_DURATION_MS);
    assert!(!intent.is_well_formed(1_500));
}

#[test]
fn shared_control_requires_both_current_being_signatures() {
    let mut intent = self_control_intent();
    intent.actor.being = "astrid".to_string();
    intent.target_being = "minime".to_string();
    intent.family = SelfControlFamilyV2::SharedCoupling;
    intent.authority_class = SelfControlAuthorityClassV2::Mutual;
    intent.authority_scope = "self_control.shared.semantic_gain".to_string();
    intent.values = SelfControlValuesV2 {
        cross_being_semantic_gain: Some(0.4),
        ..SelfControlValuesV2::default()
    };
    let astrid_key = SigningKey::from_bytes(&[8_u8; 32]);
    let minime_key = SigningKey::from_bytes(&[9_u8; 32]);
    let astrid = signed_self_control_proof(&intent, "astrid", &astrid_key, "nonce-astrid");
    let minime = signed_self_control_proof(&intent, "minime", &minime_key, "nonce-minime");
    let mut command = SelfControlCommandV2 {
        schema: SELF_CONTROL_COMMAND_SCHEMA_V2.to_string(),
        command_id: "command-shared-1".to_string(),
        intent,
        authority_proofs: vec![astrid],
    };

    assert!(!command.is_well_formed(1_500));
    command.authority_proofs.push(minime);
    assert!(command.is_well_formed(1_500));
}

#[test]
fn topology_overrides_are_structurally_one_shot() {
    let signing_key = SigningKey::from_bytes(&[10_u8; 32]);
    let mut intent = self_control_intent();
    intent.family = SelfControlFamilyV2::LocalTopology;
    intent.values = SelfControlValuesV2 {
        mode_disperse: Some(0.2),
        mode_disperse_duration_ticks: Some(8),
        ..SelfControlValuesV2::default()
    };
    let lease_proof = signed_self_control_proof(&intent, "minime", &signing_key, "nonce-lease");
    let mut command = SelfControlCommandV2 {
        schema: SELF_CONTROL_COMMAND_SCHEMA_V2.to_string(),
        command_id: "command-topology-1".to_string(),
        intent,
        authority_proofs: vec![lease_proof],
    };
    assert!(!command.is_well_formed(1_500));

    command.intent.durability = SelfControlDurabilityV2::OneShot;
    command.intent.control_expires_at_unix_ms = None;
    command.authority_proofs = vec![signed_self_control_proof(
        &command.intent,
        "minime",
        &signing_key,
        "nonce-one-shot",
    )];
    assert!(command.is_well_formed(1_500));
}

#[test]
fn semantic_body_preserves_legacy_48d_at_zero_companion_mix() {
    let base = (0..SEMANTIC_BODY_BASE_DIMENSIONS_V2)
        .map(|index| {
            f32::from(u16::try_from(index).expect("semantic body index fits in u16")) / 100.0
        })
        .collect::<Vec<_>>();
    let body = SemanticBodyV2 {
        schema: SEMANTIC_BODY_SCHEMA_V2.to_string(),
        body_id: "semantic-body-1".to_string(),
        base_features_48: base.clone(),
        companion_features_12: vec![0.25; SEMANTIC_BODY_COMPANION_DIMENSIONS_V2],
        lane_role: SemanticLaneRoleV2::LegacyCompatible,
        projection_basis_sha256: "a".repeat(64),
        provenance: SemanticBodyProvenanceV2 {
            source: "astrid.codec".to_string(),
            source_sha256: "b".repeat(64),
            producer_process_identity: "astrid:42".to_string(),
            producer_deployment_identity: "bridge-deployment-a".to_string(),
            introspection_id: Some("introspection-codec-1".to_string()),
        },
        timestamp_unix_ms: 1_000,
        fidelity: SemanticBodyFidelityV2 {
            codec: "semantic_body_v2_zero_mix".to_string(),
            companion_mix: 0.0,
            base_transport_exact: true,
            reconstruction_error: Some(0.0),
            fidelity_note: None,
        },
    };

    assert!(body.is_well_formed());
    assert_eq!(body.legacy_features(), base);
    assert!(!body.companion_is_active());

    let mut malformed = body;
    malformed.companion_features_12.pop();
    assert!(!malformed.is_well_formed());
}

#[test]
fn division_prepare_fixture_is_versioned_and_round_trips() {
    let packet: SensoryPacketV1 =
        serde_json::from_str(include_str!("fixtures/division_prepare.json")).unwrap();

    assert_eq!(packet.compatibility(), CompatibilityStatus::Current);
    let SensoryMsg::Division { command } = &packet.message else {
        panic!("expected division command packet");
    };
    assert_eq!(command.schema, DIVISION_COMMAND_SCHEMA_V1);
    assert_eq!(command.action, DivisionActionV1::DivisionPrepare);
    assert!(command.is_well_formed(1_750_000_000_500));
    assert!(command.authority_shape_is_valid(1_750_000_000_500));
    assert_eq!(
        serde_json::to_value(&packet).unwrap()["command"]["action"],
        "DIVISION_PREPARE"
    );
}

#[test]
fn commit_authority_is_exact_and_one_shot() {
    let mut command: DivisionCommandV1 = serde_json::from_value(serde_json::json!({
        "schema": DIVISION_COMMAND_SCHEMA_V1,
        "action": "DIVISION_COMMIT",
        "division_id": "division-2026-07-20-a",
        "idempotency_key": "commit-1",
        "expected_parent_generation": 42,
        "plan_digest": "b".repeat(64),
        "source": {
            "being": "operator",
            "process_identity": "codex-desktop:operator",
            "deployment_identity": "local-test"
        },
        "requested_at_unix_ms": 1_750_000_000_000_u64,
        "expires_at_unix_ms": 1_750_000_060_000_u64,
        "capability": {
            "token_id": "one-shot-token",
            "scope": DIVISION_COMMIT_SCOPE_V1,
            "division_id": "division-2026-07-20-a",
            "expected_parent_generation": 42,
            "plan_digest": "b".repeat(64),
            "expires_at_unix_ms": 1_750_000_030_000_u64,
            "approved_by": "human:test",
            "one_shot": true
        }
    }))
    .unwrap();

    assert!(command.authority_shape_is_valid(1_750_000_020_000));
    assert!(!command.authority_shape_is_valid(1_750_000_040_000));
    command.plan_digest = "c".repeat(64);
    assert!(!command.authority_shape_is_valid(1_750_000_020_000));
}

#[test]
fn status_never_confuses_readiness_with_enabled_authority() {
    let mut status = DivisionStatusV1 {
        schema: DIVISION_STATUS_SCHEMA_V1.to_string(),
        division_id: "division-2026-07-20-a".to_string(),
        parent_generation: 42,
        plan_digest: "b".repeat(64),
        lifecycle: DivisionLifecycleV1::Ready,
        parent_authoritative: true,
        commit_feature_enabled: false,
        selected_strategy: Some("input_recurrence".to_string()),
        astrid_assent: true,
        minime_assent: true,
        bridge_scale: 1.0,
        current_tick: 600,
        rollback_deadline_tick: None,
        snapshot_refs: vec!["sha256:parent".to_string()],
        readiness: DivisionReadinessV1 {
            policy: DIVISION_READINESS_POLICY_V1.to_string(),
            ready: true,
            sample_count: 600,
            blocking_reasons: Vec::new(),
            first_tick_max_abs: Some(0.0),
            state_nrmse: Some(0.1),
            state_cosine: Some(0.95),
            readout_nrmse: Some(0.1),
            max_final_sensory_fill_pct: Some(72.0),
            min_coupling_coverage: Some(1.0),
            max_regulator_distance: Some(0.1),
            metrics_fresh: true,
            sensory_panic_streak: 0,
            actuator_saturation_streak: 0,
        },
        visual_evidence_advisory_only: true,
        extensions: serde_json::Map::new(),
    };

    assert!(!status.can_request_commit());
    let disabled = status.action_availability_for("astrid");
    assert_eq!(disabled.schema, DIVISION_ACTION_AVAILABILITY_SCHEMA_V1);
    assert_eq!(
        disabled.recommended_action,
        DivisionActionV1::DivisionStatus
    );
    assert!(disabled.blocked_actions.iter().any(|entry| {
        entry.action == DivisionActionV1::DivisionCommit
            && entry
                .reasons
                .contains(&"commit_feature_disabled".to_string())
    }));

    status.commit_feature_enabled = true;
    let ready = status.action_availability_for("astrid");
    assert_eq!(ready.recommended_action, DivisionActionV1::DivisionStatus);
    assert!(ready.available_actions.iter().any(|entry| {
        entry.action == DivisionActionV1::DivisionCommit
            && entry.requires_operator_capability
            && entry.requires_command_artifact
    }));

    status.lifecycle = DivisionLifecycleV1::Cytokinesis;
    status.parent_authoritative = false;
    status.current_tick = 700;
    status.rollback_deadline_tick = Some(1_200);
    let grace = status.action_availability_for("minime");
    assert!(grace.available_actions.iter().any(|entry| {
        entry.action == DivisionActionV1::DivisionRollback && entry.requires_operator_capability
    }));
}

#[test]
fn action_availability_guides_each_being_through_precommit_lifecycle() {
    let mut status = DivisionStatusV1 {
        schema: DIVISION_STATUS_SCHEMA_V1.to_string(),
        division_id: String::new(),
        parent_generation: 7,
        plan_digest: String::new(),
        lifecycle: DivisionLifecycleV1::Idle,
        parent_authoritative: true,
        commit_feature_enabled: false,
        selected_strategy: None,
        astrid_assent: false,
        minime_assent: false,
        bridge_scale: 1.0,
        current_tick: 0,
        rollback_deadline_tick: None,
        snapshot_refs: Vec::new(),
        readiness: DivisionReadinessV1 {
            policy: DIVISION_READINESS_POLICY_V1.to_string(),
            ready: false,
            sample_count: 0,
            blocking_reasons: vec!["division_not_prepared".to_string()],
            first_tick_max_abs: None,
            state_nrmse: None,
            state_cosine: None,
            readout_nrmse: None,
            max_final_sensory_fill_pct: None,
            min_coupling_coverage: None,
            max_regulator_distance: None,
            metrics_fresh: false,
            sensory_panic_streak: 0,
            actuator_saturation_streak: 0,
        },
        visual_evidence_advisory_only: true,
        extensions: serde_json::Map::new(),
    };

    let idle = status.action_availability_for("astrid");
    assert_eq!(idle.recommended_action, DivisionActionV1::DivisionPrepare);
    assert!(
        idle.available_actions
            .iter()
            .any(|entry| entry.action == DivisionActionV1::DivisionPrepare)
    );

    status.lifecycle = DivisionLifecycleV1::Shadowing;
    status.division_id = "division-current".to_string();
    status.plan_digest = "b".repeat(64);
    let shadow = status.action_availability_for("astrid");
    assert_eq!(shadow.recommended_action, DivisionActionV1::DivisionAssent);
    assert!(
        shadow
            .available_actions
            .iter()
            .any(|entry| entry.action == DivisionActionV1::DivisionAbort)
    );

    status.astrid_assent = true;
    let after_assent = status.action_availability_for("astrid");
    assert_eq!(
        after_assent.recommended_action,
        DivisionActionV1::DivisionStatus
    );
    assert!(after_assent.blocked_actions.iter().any(|entry| {
        entry.action == DivisionActionV1::DivisionAssent
            && entry
                .reasons
                .contains(&"this_being_assent_already_current".to_string())
    }));
}

#[test]
fn telemetry_versioned_output_remains_on_v1_0() {
    let packet: EigenPacketV1 =
        serde_json::from_str(include_str!("fixtures/current_eigenpacket.json")).unwrap();
    let value = serde_json::to_value(packet.versioned()).unwrap();

    assert_eq!(value["protocol"]["major"], 1);
    assert_eq!(value["protocol"]["minor"], 0);
}

#[test]
fn delivery_hash_covers_only_the_canonical_sensory_payload() {
    let message = SensoryMsg::Semantic {
        features: vec![0.25, -0.5],
        ts_ms: Some(99),
    };
    let expected = canonical_sensory_payload_sha256(&message);
    let delivery = DeliveryEnvelopeV1::new(
        "delivery-1".to_string(),
        &message,
        100,
        "pid:10".to_string(),
        "deployment-1".to_string(),
    );
    let packet = SensoryPacketV1::with_envelopes(message.clone(), delivery.clone(), None);

    assert_eq!(delivery.payload_sha256, expected);
    assert!(delivery.payload_matches(&message));
    assert_eq!(
        packet.delivery_v1.as_ref().unwrap().payload_sha256,
        canonical_sensory_payload_sha256(&packet.message)
    );

    let tampered = SensoryMsg::Semantic {
        features: vec![0.25, -0.4],
        ts_ms: Some(99),
    };
    assert!(!delivery.payload_matches(&tampered));
}

#[test]
fn mutual_address_requires_exact_lineage_and_contains_no_raw_body() {
    let exact = MutualAddressEnvelopeV1 {
        schema_version: 1,
        address_id: "address-1".to_string(),
        from_being: "astrid".to_string(),
        to_being: "minime".to_string(),
        correspondence_id: Some("correspondence-1".to_string()),
        thread_id: Some("thread-1".to_string()),
        reply_to: Some("message-1".to_string()),
        persistence_id: Some("persistence-1".to_string()),
        authority_lineage_id: None,
        created_at_unix_ms: 100,
        body_sha256: "a".repeat(64),
        raw_body_included: false,
    };

    assert!(exact.is_exact_lineage());
    let encoded = serde_json::to_value(&exact).unwrap();
    assert!(encoded.get("body").is_none());
    assert_eq!(encoded["raw_body_included"], false);

    let mut inferred = exact;
    inferred.correspondence_id = None;
    assert!(!inferred.is_exact_lineage());
}

#[test]
fn hello_and_receipt_never_assert_spectral_causation() {
    let hello = SensoryServerHelloV1::new("pid:20".to_string(), "deployment-2".to_string());
    assert!(hello.supports_receipts());
    assert!(!hello.spectral_causation_established);

    let receipt = SensoryDeliveryReceiptV1::new(
        "receipt-1".to_string(),
        "delivery-1".to_string(),
        "b".repeat(64),
        SensoryDeliveryStatusV1::Accepted,
        101,
        Some(102),
        Some("address-1".to_string()),
        None,
        "pid:20".to_string(),
        "deployment-2".to_string(),
    );
    let value = serde_json::to_value(receipt).unwrap();
    assert_eq!(value["status"], "accepted");
    assert_eq!(value["spectral_causation_established"], false);
}
