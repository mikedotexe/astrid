use astrid_events::ipc::{IpcMessage, IpcPayload, IpcProducerV1, IpcTraceContextV1};
use astrid_events::{AstridEvent, EventBus, EventMetadata};
use serde_json::json;
use uuid::Uuid;

fn message(payload: IpcPayload) -> IpcMessage {
    IpcMessage::new("fixture.unknown", payload, Uuid::new_v4())
        .with_principal("fixture-owner")
        .with_trace(IpcTraceContextV1::root(
            Uuid::new_v4(),
            "fixture-session",
            None,
        ))
        .with_producer(IpcProducerV1::new("fixture", "fixture-producer"))
}

#[test]
fn bus_preserves_unknown_or_custom_payload_and_separate_supported_origin() {
    let bus = EventBus::new();
    let mut receiver = bus.subscribe_topic("fixture.unknown");
    let payloads = [
        IpcPayload::Unknown,
        IpcPayload::from_json_value(json!({
            "type": "future_variant", "nested": {"values": [1, null, "caf\u{e9}"]}
        })),
    ];
    let mut previous_sequence = None;
    for payload in payloads {
        let sent = message(payload);
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("fixture"),
            message: sent.clone(),
        });
        let event = receiver.try_recv().unwrap();
        let AstridEvent::Ipc { metadata, message } = event.as_ref() else {
            panic!("expected an IPC event");
        };
        assert_eq!(message.payload, sent.payload);
        assert_eq!(message.topic, sent.topic);
        assert_eq!(message.source_id, sent.source_id);
        assert_eq!(message.timestamp, sent.timestamp);
        assert_eq!(message.principal, sent.principal);
        assert_eq!(message.producer, sent.producer);
        assert_eq!(message.trace, sent.trace);
        assert_eq!(
            metadata.correlation_id,
            sent.trace.map(|trace| trace.trace_id)
        );
        if let Some(previous) = previous_sequence {
            assert!(message.seq > previous);
        }
        previous_sequence = Some(message.seq);
        assert_eq!(
            message.payload.to_guest_bytes().unwrap(),
            sent.payload.to_guest_bytes().unwrap()
        );
    }
}

#[test]
fn bus_rejects_invalid_trace_without_promoting_payload_claims() {
    let bus = EventBus::new();
    let mut receiver = bus.subscribe_topic("fixture.unknown");
    let input = json!({
        "type": "future_variant",
        "trace": IpcTraceContextV1::root(Uuid::new_v4(), "claimed", None),
        "producer": {"schema_version": 1, "kind": "kernel_host", "id": "claimed"},
        "principal": "operator"
    });
    let mut sent = IpcMessage::new(
        "fixture.unknown",
        IpcPayload::from_json_value(input.clone()),
        Uuid::new_v4(),
    );
    let mut invalid = IpcTraceContextV1::root(Uuid::new_v4(), "fixture", None);
    invalid.trace_id = Uuid::nil();
    sent.trace = Some(invalid);
    bus.publish(AstridEvent::Ipc {
        metadata: EventMetadata::new("fixture"),
        message: sent,
    });
    let event = receiver.try_recv().unwrap();
    let AstridEvent::Ipc { metadata, message } = event.as_ref() else {
        panic!("expected an IPC event");
    };
    assert_eq!(message.trace, None);
    assert_eq!(message.producer, None);
    assert_eq!(message.principal, None);
    assert_eq!(metadata.correlation_id, None);
    assert_eq!(message.payload, IpcPayload::Custom { data: input });
}

#[test]
fn maintenance_gate_transports_unrecognized_payloads_without_accounting_them_as_work() {
    let bus = EventBus::new();
    let mut receiver = bus.subscribe_topic("fixture.unknown");
    bus.set_user_input_blocked(true);
    let before = bus.maintenance_activity();
    for payload in [
        IpcPayload::Unknown,
        IpcPayload::from_json_value(json!({"type": "future_variant"})),
    ] {
        let expected = payload.clone();
        bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("fixture"),
            message: message(payload),
        });
        let event = receiver.try_recv().unwrap();
        let AstridEvent::Ipc { message, .. } = event.as_ref() else {
            panic!("expected an IPC event");
        };
        assert_eq!(message.payload, expected);
    }
    assert_eq!(bus.maintenance_activity(), before);
}

#[test]
fn origin_metadata_cannot_admit_a_known_user_request_during_maintenance() {
    let bus = EventBus::new();
    let mut receiver = bus.subscribe_topic("fixture.unknown");
    bus.set_user_input_blocked(true);
    let before = bus.maintenance_activity();
    bus.publish(AstridEvent::Ipc {
        metadata: EventMetadata::new("fixture"),
        message: message(IpcPayload::UserInput {
            text: "synthetic work".into(),
            session_id: "fixture-session".into(),
            context: None,
        }),
    });
    assert!(receiver.try_recv().is_none());
    assert_eq!(bus.maintenance_activity(), before);
}
