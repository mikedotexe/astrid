use super::*;

fn conv() -> ConversationState {
    ConversationState::new(Vec::new(), None)
}

fn astrid_action(conv: &mut ConversationState, action: &str) -> NextActionOutcome {
    handle_action(
        conv,
        "INTROSPECTION_CADENCE",
        action,
        NextActionAuthorV1::Astrid,
    )
    .expect("cadence action")
}

#[test]
fn cadence_defaults_off_and_legacy_json_repairs_off() {
    let default = IntrospectionCadenceV1::default();
    assert!(!default.enabled);
    assert_eq!(default.every_exchanges, 0);

    let mut legacy: IntrospectionCadenceV1 =
        serde_json::from_value(json!({})).expect("legacy cadence state");
    legacy.repair_after_restore(41, 1_000);
    assert_eq!(legacy, default);
}

#[test]
fn parser_accepts_bounds_rotation_and_target_offset() {
    assert_eq!(
        parse_command("INTROSPECTION_CADENCE EVERY 4"),
        Ok(CadenceCommandV1::Every {
            every_exchanges: 4,
            target: None,
        })
    );
    assert_eq!(
        parse_command("INTROSPECTION_CADENCE EVERY 256 astrid:llm 400"),
        Ok(CadenceCommandV1::Every {
            every_exchanges: 256,
            target: Some(IntrospectionCadenceTargetV1 {
                label: "astrid:llm".to_string(),
                offset: Some(400),
            }),
        })
    );
    assert_eq!(
        parse_command("INTROSPECTION_CADENCE EVERY 8 next-in-rotation"),
        Ok(CadenceCommandV1::Every {
            every_exchanges: 8,
            target: None,
        })
    );
}

#[test]
fn parser_rejects_out_of_bounds_and_malformed_actions() {
    for action in [
        "INTROSPECTION_CADENCE EVERY 3",
        "INTROSPECTION_CADENCE EVERY 257",
        "INTROSPECTION_CADENCE EVERY nope",
        "INTROSPECTION_CADENCE EVERY 8 astrid:llm nope",
        "INTROSPECTION_CADENCE EVERY 8 astrid:llm 4 extra",
        "INTROSPECTION_CADENCE OFF now",
        "INTROSPECTION_CADENCE",
    ] {
        assert!(parse_command(action).is_err(), "{action}");
    }
}

#[test]
fn operator_can_inspect_but_cannot_mutate() {
    let mut conv = conv();
    let status = handle_action(
        &mut conv,
        "INTROSPECTION_CADENCE",
        "INTROSPECTION_CADENCE STATUS",
        NextActionAuthorV1::Operator,
    )
    .expect("operator status");
    assert!(status.handled);
    assert!(!conv.introspection_cadence.enabled);

    let rejected = handle_action(
        &mut conv,
        "INTROSPECTION_CADENCE",
        "INTROSPECTION_CADENCE EVERY 4",
        NextActionAuthorV1::Operator,
    )
    .expect("operator mutation");
    assert!(!rejected.handled);
    assert!(!conv.introspection_cadence.enabled);
}

#[test]
fn mutation_requires_a_durable_lifecycle_receipt() {
    let mut conv = conv();
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(true));
    let configured = astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(false));
    assert!(!configured.handled);
    assert!(!conv.introspection_cadence.enabled);

    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(true));
    let disabled = astrid_action(&mut conv, "INTROSPECTION_CADENCE OFF");
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(false));
    assert!(!disabled.handled);
    assert!(conv.introspection_cadence.enabled);
}

#[test]
fn due_math_starts_after_configuration_exchange() {
    let mut conv = conv();
    let outcome = astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    assert!(outcome.handled);
    assert_eq!(
        conv.introspection_cadence.anchor_completed_exchange,
        Some(1)
    );
    for exchange in 1..5 {
        conv.exchange_count = exchange;
        assert!(!observe_due(&mut conv), "exchange {exchange}");
    }
    conv.exchange_count = 5;
    assert!(observe_due(&mut conv));
    assert_eq!(conv.introspection_cadence.pending_since_exchange, Some(5));
}

#[test]
fn newer_authored_action_defers_without_consuming_due_state() {
    let mut conv = conv();
    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    conv.exchange_count = 5;
    observe_due(&mut conv);
    conv.introspection_cadence
        .last_astrid_action_completed_exchange = Some(5);
    assert!(!try_select_due(&mut conv));
    assert_eq!(conv.introspection_cadence.pending_since_exchange, Some(5));
    assert!(conv.introspection_cadence_attempt.is_none());

    conv.exchange_count = 6;
    assert!(try_select_due(&mut conv));
    assert!(conv.introspection_cadence_attempt.is_some());
}

#[test]
fn target_is_carried_and_rotation_remains_unforced() {
    let mut fixed = conv();
    astrid_action(&mut fixed, "INTROSPECTION_CADENCE EVERY 4 astrid:llm 400");
    fixed.exchange_count = 5;
    assert!(try_select_due(&mut fixed));
    assert_eq!(
        fixed.introspect_target,
        Some(IntrospectTargetV2::exact("astrid:llm".to_string(), 400))
    );

    let mut continuing = conv();
    astrid_action(
        &mut continuing,
        "INTROSPECTION_CADENCE EVERY 4 astrid:llm",
    );
    continuing.exchange_count = 5;
    assert!(try_select_due(&mut continuing));
    assert_eq!(
        continuing.introspect_target,
        Some(IntrospectTargetV2::auto("astrid:llm".to_string()))
    );

    let mut rotating = conv();
    astrid_action(&mut rotating, "INTROSPECTION_CADENCE EVERY 4");
    rotating.exchange_count = 5;
    assert!(try_select_due(&mut rotating));
    assert_eq!(rotating.introspect_target, None);
}

#[test]
fn failure_stays_pending_and_admission_advances_anchor() {
    let mut conv = conv();
    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    conv.exchange_count = 5;
    assert!(try_select_due(&mut conv));
    let first = begin_attempt(&mut conv).expect("first attempt");
    mark_failed(&mut conv, "provider_timeout", None);
    assert_eq!(conv.introspection_cadence.pending_since_exchange, Some(5));
    assert_eq!(
        conv.introspection_cadence.anchor_completed_exchange,
        Some(1)
    );
    assert!(conv.introspection_cadence_attempt.is_none());

    conv.exchange_count = 6;
    assert!(try_select_due(&mut conv));
    let second = begin_attempt(&mut conv).expect("second attempt");
    assert_ne!(first.attempt_id, second.attempt_id);
    mark_admitted(
        &mut conv,
        std::path::Path::new("introspections/introspection_astrid_llm_1.txt"),
        "accepted",
    );
    assert_eq!(conv.introspection_cadence.pending_since_exchange, None);
    assert_eq!(
        conv.introspection_cadence.anchor_completed_exchange,
        Some(7)
    );
    assert_eq!(conv.introspection_cadence.last_admitted_exchange, Some(6));
}

#[test]
fn terminal_receipt_debt_blocks_and_recovers_without_duplicate_admission() {
    let mut conv = conv();
    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    conv.exchange_count = 5;
    assert!(try_select_due(&mut conv));
    begin_attempt(&mut conv).expect("failed attempt starts");

    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(true));
    mark_failed(&mut conv, "provider_timeout", None);
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(false));
    assert_eq!(
        conv.introspection_cadence
            .lifecycle_receipt_debt
            .as_ref()
            .map(|debt| debt.event),
        Some(IntrospectionCadenceEventV1::Failed)
    );
    assert_eq!(conv.introspection_cadence.pending_since_exchange, Some(5));

    conv.exchange_count = 6;
    assert!(try_select_due(&mut conv));
    assert!(conv.introspection_cadence.lifecycle_receipt_debt.is_none());
    begin_attempt(&mut conv).expect("retry starts after debt flush");

    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(true));
    mark_admitted(
        &mut conv,
        std::path::Path::new("introspections/introspection_astrid_llm_2.txt"),
        "accepted",
    );
    TEST_LIFECYCLE_PERSISTENCE_FAILURE.with(|failure| failure.set(false));
    assert_eq!(conv.introspection_cadence.pending_since_exchange, None);
    assert_eq!(
        conv.introspection_cadence
            .lifecycle_receipt_debt
            .as_ref()
            .map(|debt| debt.event),
        Some(IntrospectionCadenceEventV1::Admitted)
    );

    conv.exchange_count = 7;
    assert!(!try_select_due(&mut conv));
    assert!(conv.introspection_cadence.lifecycle_receipt_debt.is_none());
    assert_eq!(
        conv.introspection_cadence.anchor_completed_exchange,
        Some(7)
    );
}

#[test]
fn pilot_bound_holds_after_eight_starts_and_configuration_does_not_reset_it() {
    let mut conv = conv();
    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    for exchange in 5..13 {
        conv.exchange_count = exchange;
        assert!(try_select_due(&mut conv), "start at exchange {exchange}");
        begin_attempt(&mut conv).expect("selected attempt starts");
        mark_failed(&mut conv, "bounded_test_failure", None);
    }
    assert_eq!(conv.introspection_cadence.pilot_cadence_starts, 8);
    assert_eq!(
        conv.introspection_cadence.pilot_hold_reason.as_deref(),
        Some("pilot_eight_start_review_hold")
    );

    conv.exchange_count = 13;
    assert!(!try_select_due(&mut conv));
    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 12");
    assert_eq!(conv.introspection_cadence.every_exchanges, 12);
    assert_eq!(conv.introspection_cadence.pilot_cadence_starts, 8);
    assert_eq!(
        conv.introspection_cadence.pilot_hold_reason.as_deref(),
        Some("pilot_eight_start_review_hold")
    );
}

#[test]
fn cumulative_runtime_reaches_a_restart_safe_review_hold() {
    let mut cadence = IntrospectionCadenceV1 {
        enabled: true,
        every_exchanges: 4,
        anchor_completed_exchange: Some(1),
        pilot_started_at_unix_ms: Some(1),
        pilot_last_observed_unix_ms: Some(1),
        ..IntrospectionCadenceV1::default()
    };
    assert_eq!(cadence.observe_pilot_runtime_at(PILOT_MAX_RUNTIME_MS), None);
    assert_eq!(
        cadence.observe_pilot_runtime_at(PILOT_MAX_RUNTIME_MS.saturating_add(1)),
        Some("pilot_seventy_two_runtime_hour_review_hold")
    );
    assert_eq!(
        cadence.pilot_hold_reason.as_deref(),
        Some("pilot_seventy_two_runtime_hour_review_hold")
    );

    cadence.repair_after_restore(9, PILOT_MAX_RUNTIME_MS.saturating_add(10_000));
    assert_eq!(cadence.pilot_last_observed_unix_ms, None);
    assert!(cadence.next_due_exchange().is_none());
}

#[test]
fn off_clears_pending_immediately_without_erasing_history() {
    let mut conv = conv();
    astrid_action(&mut conv, "INTROSPECTION_CADENCE EVERY 4");
    conv.exchange_count = 5;
    assert!(try_select_due(&mut conv));
    conv.introspection_cadence.last_admitted_exchange = Some(2);
    let outcome = astrid_action(&mut conv, "INTROSPECTION_CADENCE OFF");
    assert!(outcome.handled);
    assert!(!conv.introspection_cadence.enabled);
    assert_eq!(conv.introspection_cadence.pending_since_exchange, None);
    assert!(conv.introspection_cadence_attempt.is_none());
    assert_eq!(conv.introspection_cadence.last_admitted_exchange, Some(2));
}

#[test]
fn persisted_state_round_trip_keeps_due_and_outcome_fields() {
    let mut cadence = IntrospectionCadenceV1 {
        enabled: true,
        every_exchanges: 12,
        target: Some(IntrospectionCadenceTargetV1 {
            label: "astrid:autonomous".to_string(),
            offset: Some(800),
        }),
        anchor_completed_exchange: Some(30),
        pending_since_exchange: Some(42),
        last_attempt_exchange: Some(43),
        last_admitted_exchange: Some(18),
        pilot_started_at_unix_ms: Some(1_000),
        pilot_runtime_ms: 90_000,
        pilot_last_observed_unix_ms: Some(91_000),
        pilot_cadence_starts: 2,
        ..IntrospectionCadenceV1::default()
    };
    set_last_outcome(
        &mut cadence,
        IntrospectionCadenceEventV1::Failed,
        43,
        Some("provider_timeout".to_string()),
        Some("attempt-1".to_string()),
        None,
    );
    let json = serde_json::to_string(&cadence).expect("serialize cadence");
    let restored: IntrospectionCadenceV1 = serde_json::from_str(&json).expect("restore cadence");
    assert_eq!(restored, cadence);
}

#[test]
fn lifecycle_ledger_is_append_only_jsonl() {
    let dir = std::env::temp_dir().join(format!(
        "astrid-introspection-cadence-ledger-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(CADENCE_EVENT_LEDGER);
    append_lifecycle_event_at(&path, &json!({"event": "configured"}))
        .expect("append configured event");
    append_lifecycle_event_at(&path, &json!({"event": "due"})).expect("append due event");
    let lines: Vec<Value> = std::fs::read_to_string(&path)
        .expect("read cadence ledger")
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse cadence event"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["event"], "configured");
    assert_eq!(lines[1]["event"], "due");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lifecycle_ledger_retry_is_idempotent_by_durable_identity() {
    let dir = std::env::temp_dir().join(format!(
        "astrid-introspection-cadence-idempotent-ledger-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(CADENCE_EVENT_LEDGER);
    let event = json!({"lifecycle_id": "cadence-event-1", "event": "admitted"});
    append_lifecycle_event_at(&path, &event).expect("append first event");
    append_lifecycle_event_at(&path, &event).expect("retry exact event");
    assert_eq!(
        std::fs::read_to_string(&path)
            .expect("read idempotent ledger")
            .lines()
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn due_cadence_stays_behind_safety_peer_study_and_one_shot_work() {
    use crate::journal::{RemoteJournalEntry, RemoteJournalKind};
    use crate::types::SafetyLevel;

    let mut safety = conv();
    astrid_action(&mut safety, "INTROSPECTION_CADENCE EVERY 4");
    safety.exchange_count = 5;
    let mode = super::super::super::state::choose_mode(&mut safety, SafetyLevel::Red, 40.0, None);
    assert_eq!(mode, super::super::super::state::Mode::Witness);
    assert_eq!(safety.introspection_cadence.pending_since_exchange, Some(5));
    assert!(safety.introspection_cadence_attempt.is_none());

    let mut peer = conv();
    astrid_action(&mut peer, "INTROSPECTION_CADENCE EVERY 4");
    peer.exchange_count = 5;
    peer.pending_remote_self_study = Some(RemoteJournalEntry {
        path: std::path::PathBuf::from("peer-study.txt"),
        kind: RemoteJournalKind::SelfStudy,
        source_label: Some("minime".to_string()),
    });
    let peer_fill = peer.prev_fill;
    let mode =
        super::super::super::state::choose_mode(&mut peer, SafetyLevel::Green, peer_fill, None);
    assert_eq!(mode, super::super::super::state::Mode::Dialogue);
    assert_eq!(peer.introspection_cadence.pending_since_exchange, Some(5));
    assert!(peer.introspection_cadence_attempt.is_none());

    let mut one_shot = conv();
    astrid_action(&mut one_shot, "INTROSPECTION_CADENCE EVERY 4");
    one_shot.exchange_count = 5;
    one_shot.wants_evolve = true;
    let one_shot_fill = one_shot.prev_fill;
    let mode = super::super::super::state::choose_mode(
        &mut one_shot,
        SafetyLevel::Green,
        one_shot_fill,
        None,
    );
    assert_eq!(mode, super::super::super::state::Mode::Evolve);
    assert_eq!(
        one_shot.introspection_cadence.pending_since_exchange,
        Some(5)
    );
    assert!(one_shot.introspection_cadence_attempt.is_none());
}

#[test]
fn correspondence_deferral_and_idle_selection_leave_due_state_accountable() {
    use crate::types::SafetyLevel;

    let mut correspondence = conv();
    astrid_action(&mut correspondence, "INTROSPECTION_CADENCE EVERY 4");
    correspondence.exchange_count = 5;
    assert!(observe_due(&mut correspondence));
    assert!(defer_pending(
        &mut correspondence,
        "unread_direct_correspondence"
    ));
    assert_eq!(
        correspondence.introspection_cadence.pending_since_exchange,
        Some(5)
    );
    assert!(correspondence.introspection_cadence_attempt.is_none());

    correspondence.exchange_count = 6;
    let fill = correspondence.prev_fill;
    let mode = super::super::super::state::choose_mode(
        &mut correspondence,
        SafetyLevel::Green,
        fill,
        None,
    );
    assert_eq!(mode, super::super::super::state::Mode::Introspect);
    assert!(correspondence.introspection_cadence_attempt.is_some());
}

#[test]
fn lifecycle_payload_is_factual_and_carries_required_fields() {
    let cadence = IntrospectionCadenceV1 {
        enabled: true,
        every_exchanges: 8,
        anchor_completed_exchange: Some(10),
        pending_since_exchange: Some(18),
        ..IntrospectionCadenceV1::default()
    };
    let payload = lifecycle_payload(
        &cadence,
        IntrospectionCadenceEventV1::Attempted,
        18,
        "scheduler",
        Some("cadence_mode_execution_started"),
        Some("attempt-18"),
        None,
        None,
    );
    assert_eq!(payload["exchange"], 18);
    assert_eq!(payload["target"], "next-in-rotation");
    assert_eq!(payload["attempt_id"], "attempt-18");
    assert!(
        payload["lifecycle_id"]
            .as_str()
            .is_some_and(|value| value.starts_with("introspection-cadence-v1-"))
    );
    assert!(payload.get("artifact_path").is_some());
    let text = payload.to_string();
    assert!(!text.contains("willing"));
    assert!(!text.contains("benefit"));
    assert!(!text.contains("consent"));
}
