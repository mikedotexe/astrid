use super::tests::deliver_to_inbox_with_ledger;
use super::*;

#[test]
fn seen_ack_is_visibility_not_attention_evidence() {
    let root = std::env::temp_dir().join(format!("corr_seen_ack_test_{}", now_ms()));
    let inbox = root.join("inbox");
    let ledger = root.join("ledger.jsonl");
    let (envelope, path) = deliver_to_inbox_with_ledger(
        &ledger,
        &inbox,
        "astrid",
        "minime",
        "A direct address that has only been seen.",
        CorrespondenceFields::default(),
    )
    .unwrap();
    append_read_receipt_at(
        &ledger,
        "minime",
        &envelope.message_id,
        &envelope.thread_id,
        &path,
    )
    .unwrap();
    let seen = append_ack_receipt_at(&ledger, "latest", "minime", "astrid", "seen", "seen");
    assert!(seen.contains("ACK RECEIPT WRITTEN"));
    let heartbeat = Some(serde_json::json!({
        "jitter_class": "normal",
        "timing_reliability": "reliable",
        "field_vs_hearing": "telemetry cadence is steady"
    }));
    let records = read_ledger_records_at(&ledger);
    let fidelity =
        direct_contact_fidelity_for_with_heartbeat(&records, "latest", heartbeat.clone());
    assert_eq!(
        fidelity.get("status").and_then(Value::as_str),
        Some("seen_ack_only")
    );
    assert_eq!(
        fidelity.get("block_reason").and_then(Value::as_str),
        Some("seen_ack_is_visibility_not_address")
    );
    assert_eq!(
        fidelity.get("ack_receipt_present").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        fidelity.get("acknowledged").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        fidelity
            .get("eligible_for_correspondence_attention_canary")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        fidelity
            .get("direct_contact_fidelity_v3")
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str),
        Some("seen_ack_only")
    );
    assert_eq!(
        fidelity
            .get("native_thread_continuity_v3")
            .and_then(|value| value.get("ack_receipt_present"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        fidelity
            .get("native_thread_continuity_v3")
            .and_then(|value| value.get("acknowledged"))
            .and_then(Value::as_bool),
        Some(false)
    );
    let blocked = activate_attention_canary_at_with_heartbeat(
        &ledger,
        "latest",
        "reason: hold it distinctly; focus: direct address; stop_criteria: one turn",
        "astrid",
        "minime",
        heartbeat,
    );
    assert!(blocked.contains("blocked_no_receipt"), "{blocked}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_or_timing_ambiguous_contact_never_unlocks_contact_authority() {
    let old_t = now_ms().saturating_sub(MICRODOSE_COOLDOWN_MS.saturating_add(1000));
    let mut records = vec![
        json!({
            "record_type": "message",
            "recorded_at_unix_ms": old_t,
            "message_id": "stale_msg",
            "thread_id": "thread_stale",
            "from_being": "astrid",
            "to_being": "minime",
            "authority": "language_only"
        }),
        json!({
            "record_type": "delivery_receipt",
            "recorded_at_unix_ms": old_t.saturating_add(1),
            "message_id": "stale_msg",
            "thread_id": "thread_stale"
        }),
    ];
    let stale = direct_contact_fidelity_for_with_heartbeat(
        &records,
        "latest",
        Some(json!({
            "timing_reliability": "reliable",
            "jitter_class": "normal"
        })),
    );
    assert_eq!(
        stale.get("status").and_then(Value::as_str),
        Some("stale_contact")
    );
    assert_eq!(
        stale
            .get("eligible_for_correspondence_attention_canary")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        stale
            .get("eligible_for_correspondence_microdose")
            .and_then(Value::as_bool),
        Some(false)
    );

    records.push(json!({
        "record_type": "ack_receipt",
        "recorded_at_unix_ms": old_t.saturating_add(2),
        "message_id": "stale_msg",
        "thread_id": "thread_stale",
        "from_being": "minime",
        "to_being": "astrid",
        "ack_kind": "unclear"
    }));
    let ambiguous = direct_contact_fidelity_for_with_heartbeat(
        &records,
        "latest",
        Some(json!({
            "timing_reliability": "timing_ambiguous",
            "jitter_class": "late"
        })),
    );
    assert_eq!(
        ambiguous.get("timing_ambiguous").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        ambiguous.get("block_reason").and_then(Value::as_str),
        Some("heartbeat_timing_ambiguous")
    );
    assert_eq!(
        ambiguous
            .get("eligible_for_correspondence_attention_canary")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        ambiguous
            .get("eligible_for_correspondence_microdose")
            .and_then(Value::as_bool),
        Some(false)
    );
}
