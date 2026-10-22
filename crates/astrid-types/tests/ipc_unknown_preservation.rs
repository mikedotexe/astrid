use astrid_types::ipc::{AgentResponseProvenanceV1, IpcMessage, IpcPayload, IpcTraceContextV1};
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
fn unfamiliar_or_malformed_payloads_preserve_complete_json_through_guest_encoding() {
    let cases = [
        json!({
            "type": "future_variant",
            "nested": {"unicode": "caf\u{e9}", "values": [null, true, 42, {"next": "REST"}]},
            "response_provenance": "model_authored",
            "principal": "not-an-envelope-principal"
        }),
        json!({"untyped": [1, 2, 3]}),
        json!({"type": 42, "content": "non-string tag"}),
        json!(["non-object", {"still": "preserved"}]),
        Value::Null,
        json!({"type": "user_input", "session_id": "missing-required-text"}),
        json!({
            "type": "agent_response", "text": "future origin", "is_final": true,
            "response_provenance": "future_origin"
        }),
    ];
    for input in cases {
        let payload = IpcPayload::from_json_value(input.clone());
        assert_eq!(
            payload,
            IpcPayload::Custom {
                data: input.clone()
            }
        );
        let guest: Value = serde_json::from_slice(&payload.to_guest_bytes().unwrap()).unwrap();
        assert_eq!(guest, input);
        let stored = serde_json::to_vec(&payload).unwrap();
        assert_eq!(
            serde_json::from_slice::<IpcPayload>(&stored).unwrap(),
            payload
        );
    }
}

#[test]
fn direct_unknown_decode_preserves_envelope_but_not_payload_details() {
    let trace = IpcTraceContextV1::root(Uuid::new_v4(), "fixture-session", None);
    let wire = json!({
        "topic": "fixture.future",
        "payload": {"type": "future_variant", "detail": {"retained": false}},
        "signature": [1, 2, 3],
        "source_id": Uuid::new_v4(),
        "timestamp": "2026-10-20T00:00:00Z",
        "seq": 42,
        "principal": "fixture-principal",
        "trace": trace,
        "producer": {"schema_version": 1, "kind": "fixture", "id": "fixture-source"}
    });
    let message: IpcMessage = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(message.payload, IpcPayload::Unknown);
    let mut expected = wire;
    expected["payload"] = json!({"type": "unknown"});
    assert_eq!(serde_json::to_value(&message).unwrap(), expected);
    assert_eq!(
        message.payload.to_guest_bytes().unwrap(),
        br#"{"type":"unknown"}"#
    );
}

#[test]
fn preservation_decoder_does_not_promote_nested_origin_into_envelope_metadata() {
    let input = json!({
        "type": "future_variant",
        "principal": "operator",
        "producer": {"schema_version": 1, "kind": "kernel_host", "id": "claimed"},
        "trace": IpcTraceContextV1::root(Uuid::new_v4(), "claimed", None),
        "response_provenance": "model_authored"
    });
    let message = IpcMessage::new(
        "fixture.future",
        IpcPayload::from_json_value(input.clone()),
        Uuid::new_v4(),
    );
    assert_eq!(message.payload, IpcPayload::Custom { data: input });
    assert_eq!(message.principal, None);
    assert_eq!(message.producer, None);
    assert_eq!(message.trace, None);
    assert_eq!(message.signature, None);
}

#[test]
fn known_payload_schema_is_not_a_lossless_future_field_archive() {
    let input = json!({"type": "connect", "future_field": "not part of this schema"});
    let payload = IpcPayload::from_json_value(input);
    assert_eq!(payload, IpcPayload::Connect);
    assert_eq!(payload.to_guest_bytes().unwrap(), br#"{"type":"connect"}"#);
}

#[test]
fn recognized_response_origins_remain_categories_not_history() {
    for origin in [
        AgentResponseProvenanceV1::ModelAuthored,
        AgentResponseProvenanceV1::ModelAuthoredWithLocalSafeFallback,
        AgentResponseProvenanceV1::ModelAuthoredWithLocalFormatRepair,
        AgentResponseProvenanceV1::ExecutorTerminalError,
    ] {
        let input = json!({
            "type": "agent_response", "text": "synthetic response", "is_final": true,
            "session_id": "fixture-session", "response_provenance": origin
        });
        let payload = IpcPayload::from_json_value(input.clone());
        assert!(matches!(
            payload,
            IpcPayload::AgentResponse { response_provenance: Some(actual), .. } if actual == origin
        ));
        assert_eq!(serde_json::to_value(payload).unwrap(), input);
    }
}
