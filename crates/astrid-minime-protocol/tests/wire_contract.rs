use astrid_minime_protocol::{CompatibilityStatus, EigenPacketV1, SensoryMsg, SensoryPacketV1};

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
    assert_eq!(
        packet.extensions["future_additive_packet"]["preserved"],
        true
    );
    let encoded = serde_json::to_value(packet).unwrap();
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
