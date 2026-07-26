use std::path::Path;

use super::{
    PassageAnchorAssociationV1, PassageAnchorKindV1, PassageAnchorRoleV1, PassageBearingStrandV1,
    PassageCheckpointV1, PassageCompanyResponseV1, PassageMovementEaseV1,
    PassageMovementResistanceV1, PassagePersistenceTendencyV1, PassageReadinessV1,
    PassageRoomNeededV1, PassageWitnessFitV1, phase_passages, reduce_context,
};

pub(crate) fn status_report_at(path: &Path, actor: &str, limit: usize) -> String {
    let records = phase_passages::read_records(path);
    let (state, errors) = reduce_context(&records);
    let own_conditions = state
        .latest_condition_by_passage
        .values()
        .filter(|event| event.passage_actor == actor)
        .count();
    let mut own_checkpoints = state
        .latest_checkpoint_by_passage
        .values()
        .filter(|event| event.passage_actor == actor)
        .collect::<Vec<_>>();
    own_checkpoints.sort_by_key(|event| std::cmp::Reverse(event.recorded_at_unix_ms));
    let inbound = state
        .requests
        .values()
        .filter(|request| request.requested_peer == actor)
        .count();
    let mut own_anchors = state
        .latest_anchor_by_passage_role
        .values()
        .filter(|event| event.passage_actor == actor)
        .collect::<Vec<_>>();
    own_anchors.sort_by_key(|event| std::cmp::Reverse(event.recorded_at_unix_ms));
    let mut own_bearings = state
        .latest_bearing_by_passage_strand
        .values()
        .filter(|event| event.passage_actor == actor)
        .collect::<Vec<_>>();
    own_bearings.sort_by_key(|event| std::cmp::Reverse(event.recorded_at_unix_ms));
    let mut lines = vec![
        "=== TRANSITION PASSAGE CONTEXT V4 ===".to_string(),
        format!(
            "Own latest conditions: {own_conditions}; own checkpointed passages: {}; own continuity anchors: {}; own current strand bearings: {}; inbound company requests: {inbound}; invalid rows: {}.",
            own_checkpoints.len(),
            own_anchors.len(),
            own_bearings.len(),
            errors.len()
        ),
        "Felt boundary: readiness and movement ease are categorical self-description, never a numeric score or inference from telemetry.".to_string(),
        "Bearing boundary: resistance, persistence, and witness fit remain independently revisable self-description per passage strand; they are not a viscosity metric, telemetry inference, stage result, or completion signal.".to_string(),
        "Anchor boundary: a typed anchor preserves a self-authored orientation reference; it does not make a shadow, receipt, or signal the mechanical cause or truth of a felt transition.".to_string(),
        "Company boundary: requests and responses are revisable, right-to-ignore language records; silence is neutral and no passage stage changes.".to_string(),
    ];
    let mut own = state
        .latest_condition_by_passage
        .values()
        .filter(|event| event.passage_actor == actor)
        .collect::<Vec<_>>();
    own.sort_by_key(|event| std::cmp::Reverse(event.recorded_at_unix_ms));
    for event in own.into_iter().take(limit) {
        lines.push(format!(
            "- condition {}: readiness={}; movement_ease={}; room_needed={}; source={}",
            event.passage_id,
            event
                .readiness
                .map_or("unknown", PassageReadinessV1::as_str),
            event
                .movement_ease
                .map_or("unknown", PassageMovementEaseV1::as_str),
            event
                .room_needed
                .map_or("unknown", PassageRoomNeededV1::as_str),
            event.source_ref,
        ));
    }
    for event in own_checkpoints.into_iter().take(limit) {
        lines.push(format!(
            "- checkpoint {}: point={}; source={}",
            event.passage_id,
            event
                .checkpoint
                .map_or("unknown", PassageCheckpointV1::as_str),
            event.source_ref,
        ));
    }
    for event in own_bearings.into_iter().take(limit) {
        lines.push(format!(
            "- bearing {}: strand={}; movement_resistance={}; persistence_tendency={}; witness_fit={}; source={}",
            event.passage_id,
            event
                .bearing_strand
                .map_or("unknown", PassageBearingStrandV1::as_str),
            event
                .movement_resistance
                .map_or("unknown", PassageMovementResistanceV1::as_str),
            event
                .persistence_tendency
                .map_or("unknown", PassagePersistenceTendencyV1::as_str),
            event
                .witness_fit
                .map_or("unknown", PassageWitnessFitV1::as_str),
            event.source_ref,
        ));
    }
    for event in own_anchors.into_iter().take(limit) {
        lines.push(format!(
            "- anchor {}: role={}; kind={}; association={}; anchor={}; source={}",
            event.passage_id,
            event
                .anchor_role
                .map_or("unknown", PassageAnchorRoleV1::as_str),
            event
                .anchor_kind
                .map_or("unknown", PassageAnchorKindV1::as_str),
            event
                .anchor_association
                .map_or("unknown", PassageAnchorAssociationV1::as_str),
            event.anchor_ref.as_deref().unwrap_or("unknown"),
            event.source_ref,
        ));
    }
    let mut requests = state.requests.values().collect::<Vec<_>>();
    requests.sort_by_key(|request| std::cmp::Reverse(request.recorded_at_unix_ms));
    for request in requests
        .into_iter()
        .filter(|request| request.passage_actor == actor || request.requested_peer == actor)
        .take(limit)
    {
        lines.push(format!(
            "- company {}: passage={}; owner={}; peer={}; mode={}; response={}; optional=true",
            request.request_id,
            request.passage_id,
            request.passage_actor,
            request.requested_peer,
            request.mode.as_str(),
            request
                .response
                .map_or("pending", PassageCompanyResponseV1::as_str),
        ));
    }
    lines.push(
        "Condition: DESCRIBE_TRANSITION_CONDITION <passage> :: readiness: ready|tentative|not_ready|unknown; movement_ease: open|effortful|stuck|changing|unknown; room_needed: self_directed|witness|space|low_energy_presence|answer|needs_time|return_support|unknown; source_ref: <bounded_ref>.".to_string(),
    );
    lines.push(
        "Bearing: DESCRIBE_TRANSITION_BEARING <passage> :: strand: entry_tension|pivot|settling|return|reopen|continuity; movement_resistance: yielding|effortful|resistant|held_fast|changing|unknown; persistence_tendency: fleeting|lingering|carried|deepening|releasing|unknown; witness_fit: separate|touching|holding|interwoven|misattuned|unknown; source_ref: <bounded_ref>.".to_string(),
    );
    lines.push(
        "Anchor: BIND_TRANSITION_ANCHOR <passage> :: role: entry|pivot|settling|return|reopen|continuity; kind: felt_source|shadow_trajectory|lived_state_witness|signal_spine|representation_transition|correspondence|return_point|other; association: self_authored|receipt_linked|temporal_context|unknown; anchor_ref: <bounded_ref>; source_ref: <bounded_ref>.".to_string(),
    );
    lines.push(
        "Company: REQUEST_TRANSITION_COMPANY <passage> :: peer: minime; mode: witness|low_energy_presence|reply_when_able|space|return_support; source_ref: <bounded_ref>. RESPOND_TRANSITION_COMPANY and WITHDRAW_TRANSITION_COMPANY accept a request id and bounded source_ref.".to_string(),
    );
    lines.join("\n")
}
