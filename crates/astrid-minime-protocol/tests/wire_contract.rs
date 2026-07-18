use astrid_minime_protocol::{
    CompatibilityStatus, DeliveryEnvelopeV1, EigenPacketV1, MutualAddressEnvelopeV1,
    SensoryDeliveryReceiptV1, SensoryDeliveryStatusV1, SensoryMsg, SensoryPacketV1,
    SensoryServerHelloV1, canonical_sensory_payload_sha256,
};

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
fn sensory_v1_1_omits_optional_envelopes_when_absent() {
    let value = serde_json::to_value(SensoryPacketV1::versioned(SensoryMsg::Semantic {
        features: vec![0.25],
        ts_ms: None,
    }))
    .unwrap();

    assert_eq!(value["protocol"]["minor"], 1);
    assert!(value.get("delivery_v1").is_none());
    assert!(value.get("mutual_address_v1").is_none());
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
