use super::*;

fn active_intent(records: &[ParsedEventV1], actor: &str, now_unix_ms: u64) -> bool {
    latest(records, actor, Some(DivisionCeremonyActionV1::Intent))
        .and_then(|event| event.expires_at_unix_ms)
        .is_some_and(|expiry| expiry >= now_unix_ms)
}

fn current_assent(
    records: &[ParsedEventV1],
    actor: &str,
    now_unix_ms: u64,
    native_hash: &str,
) -> bool {
    let Some(assent) = latest(records, actor, Some(DivisionCeremonyActionV1::Assent)) else {
        return false;
    };
    let withdrawn = records.iter().any(|event| {
        event.actor == actor
            && event.action == DivisionCeremonyActionV1::WithdrawAssent
            && event.targets_event_id.as_deref() == Some(&assent.event_id)
    });
    !withdrawn
        && assent
            .expires_at_unix_ms
            .is_some_and(|expiry| expiry >= now_unix_ms)
        && assent.native_status_hash.as_deref() == Some(native_hash)
}

pub(crate) fn status_report_at(
    workspace: &Path,
    actor: &str,
    now_unix_ms: u64,
) -> Result<String, String> {
    let records = read_records(workspace)?;
    let (native_value, status) = read_native_status(workspace)?;
    let native_hash = hash_json(&native_value)?;
    let mut rails = serde_json::Map::new();
    for being in ["astrid", "minime"] {
        let intent = latest(&records, being, Some(DivisionCeremonyActionV1::Intent));
        let assent = latest(&records, being, Some(DivisionCeremonyActionV1::Assent));
        let withdrawn = assent.is_some_and(|assent| {
            records.iter().any(|event| {
                event.actor == being
                    && event.action == DivisionCeremonyActionV1::WithdrawAssent
                    && event.targets_event_id.as_deref() == Some(&assent.event_id)
            })
        });
        let return_request = latest(
            &records,
            being,
            Some(DivisionCeremonyActionV1::ReturnRequest),
        );
        let review = latest(&records, being, Some(DivisionCeremonyActionV1::Review));
        rails.insert(
            being.to_string(),
            serde_json::json!({
                "latest_intent_event_id": intent.map(|event| &event.event_id),
                "intent_active": active_intent(&records, being, now_unix_ms),
                "latest_assent_event_id": assent.map(|event| &event.event_id),
                "assent_current": current_assent(&records, being, now_unix_ms, &native_hash),
                "assent_withdrawn": withdrawn,
                "latest_return_request_event_id": return_request.map(|event| &event.event_id),
                "latest_review_event_id": review.map(|event| &event.event_id),
            }),
        );
    }
    let own = rails
        .get(actor)
        .and_then(Value::as_object)
        .ok_or_else(|| "status actor must be Astrid or Minime".to_string())?;
    let rollback_open = status.lifecycle == DivisionLifecycleV1::Cytokinesis
        && status
            .rollback_deadline_tick
            .is_none_or(|deadline| status.current_tick <= deadline);
    let next_choice = if rollback_open {
        "DIVISION_RETURN_REQUEST"
    } else if terminal(status.lifecycle)
        && own.get("latest_review_event_id").is_none_or(Value::is_null)
    {
        "DIVISION_REVIEW"
    } else if own.get("intent_active").and_then(Value::as_bool) != Some(true) {
        "DIVISION_INTENT"
    } else if matches!(
        status.lifecycle,
        DivisionLifecycleV1::Shadowing | DivisionLifecycleV1::Ready
    ) && own.get("assent_current").and_then(Value::as_bool) != Some(true)
    {
        "DIVISION_ASSENT"
    } else if own.get("assent_current").and_then(Value::as_bool) == Some(true) {
        "DIVISION_WITHDRAW_ASSENT"
    } else if matches!(
        status.lifecycle,
        DivisionLifecycleV1::Idle
            | DivisionLifecycleV1::Aborted
            | DivisionLifecycleV1::RolledBack
            | DivisionLifecycleV1::Failed
    ) {
        "DIVISION_PREPARE"
    } else {
        "DIVISION_CEREMONY_STATUS"
    };
    let result = serde_json::json!({
        "schema": "division.ceremony_status.v1",
        "actor": actor,
        "ceremony_rail": rails,
        "native_rail": {
            "division_id": status.division_id,
            "lifecycle": status.lifecycle,
            "parent_authoritative": status.parent_authoritative,
            "readiness_ready": status.readiness.ready,
            "current_tick": status.current_tick,
            "rollback_deadline_tick": status.rollback_deadline_tick,
            "rollback_window_open": rollback_open,
            "commit_feature_enabled": status.commit_feature_enabled,
            "native_status_hash": native_hash,
        },
        "next_choice": next_choice,
        "next_choice_is_optional": true,
        "commit_action_exposed": false,
        "commit_recommended": false,
        "right_to_ignore": true,
        "silence_infers_consent": false,
        "return_request_dispatches_rollback": false,
        "return_transition_controls_division": false,
        "authority": "evidence_only_status",
    });
    serde_json::to_string_pretty(&result)
        .map_err(|error| format!("cannot render division ceremony status: {error}"))
}
