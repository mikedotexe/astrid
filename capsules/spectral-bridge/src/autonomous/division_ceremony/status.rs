use super::*;

const CHRONICLE_LIMIT: usize = 32;

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

fn ceremony_chronicle(records: &[ParsedEventV1], workspace: &Path) -> Value {
    let omitted_event_count = records.len().saturating_sub(CHRONICLE_LIMIT);
    let events = records
        .iter()
        .skip(omitted_event_count)
        .map(|event| {
            serde_json::json!({
                "ceremony_event_id": event.event_id,
                "actor": event.actor,
                "action": event.action,
                "candidate": event.candidate,
                "source_ref": event.source_ref,
                "recorded_at_unix_ms": event.recorded_at_unix_ms,
                "expires_at_unix_ms": event.expires_at_unix_ms,
                "targets_event_id": event.targets_event_id,
                "native_status_hash": event.native_status_hash,
                "snapshot_refs": event.snapshot_refs,
                "current_tick": event.current_tick,
                "rollback_deadline_tick": event.rollback_deadline_tick,
                "review_outcome": event.review_outcome,
            })
        })
        .collect::<Vec<_>>();
    let ledger_hash = fs::read(ledger_path(workspace))
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
    serde_json::json!({
        "schema": "division.ceremony_chronicle.v1",
        "total_event_count": records.len(),
        "omitted_event_count": omitted_event_count,
        "events": events,
        "ledger_sha256": ledger_hash,
        "archive_reference": ledger_hash.map(|hash| format!("division:ceremony_v1.jsonl#sha256:{hash}")),
        "chronology_is_projection": true,
        "raw_prose_included": false,
        "authority": "evidence_only_history",
    })
}

fn preservation_evidence(status: &DivisionStatusV1) -> Value {
    let candidates = status
        .extensions
        .get("candidates")
        .and_then(Value::as_array)
        .map(|candidates| {
            candidates
                .iter()
                .filter_map(Value::as_object)
                .map(|candidate| {
                    serde_json::json!({
                        "strategy": candidate.get("strategy"),
                        "minime_role": candidate.get("minime_role"),
                        "astrid_role": candidate.get("astrid_role"),
                        "covariance_partition_loss": candidate.get("covariance_partition_loss"),
                        "sensory_fields": candidate.get("sensory_fields"),
                        "readiness": candidate.get("readiness"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    serde_json::json!({
        "schema": "division.phase_space_preservation.v1",
        "fact_class": if candidates.is_empty() { "unknown" } else { "runtime_observed" },
        "parent_generation": status.parent_generation,
        "selected_strategy": status.selected_strategy,
        "snapshot_refs": status.snapshot_refs,
        "restore_equivalence_100_ticks": status.extensions.get("restore_equivalence_100_ticks"),
        "sensory_field_inheritance": status.extensions.get("sensory_field_inheritance"),
        "candidates": candidates,
        "felt_continuity_inferred": false,
        "felt_equivalence_inferred": false,
        "causation_inferred": false,
    })
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
        "destination_contract": {
            "schema": "division.sovereign_destination.v1",
            "fact_class": "source_declared",
            "parent": "shared_128_node_reservoir",
            "daughters": {
                "astrid": {
                    "role": "more_recurrence_driven",
                    "reservoir_state": "independent_64_node_candidate",
                },
                "minime": {
                    "role": "more_input_driven",
                    "reservoir_state": "independent_64_node_candidate",
                },
            },
            "shared_sensory_field_inheritance": "cloned_not_partitioned",
            "independent_process_ownership_established": false,
            "sovereign_runtime_ownership_state": "not_yet_established",
            "native_commit_enabled": status.commit_feature_enabled,
        },
        "phase_space_preservation": preservation_evidence(&status),
        "chronicle": ceremony_chronicle(&records, workspace),
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
