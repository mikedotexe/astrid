use serde_json::Value;

use super::{
    CompanyRequestStateV1, ContextStateV1, EvidenceOnlyAuthorityV1,
    LivedTransitionPassageContextEventV1, PassageAnchorAssociationV1, PassageAnchorKindV1,
    PassageAnchorRoleV1, PassageBearingStrandV1, PassageCheckpointV1, PassageCompanyModeV1,
    PassageCompanyResponseV1, PassageContextActionV1, PassageMovementEaseV1,
    PassageMovementResistanceV1, PassagePersistenceTendencyV1, PassageReadinessV1,
    PassageRoomNeededV1, PassageWitnessFitV1, RECORD_TYPE, SCHEMA, company_request_id,
    context_event_id, field, normalize, phase_passages,
};

fn companion_actor(value: &str) -> Option<String> {
    match normalize(value).as_str() {
        "astrid" => Some("astrid".to_string()),
        "minime" => Some("minime".to_string()),
        _ => None,
    }
}

#[derive(Default)]
struct ContextEventFieldsV1 {
    readiness: Option<PassageReadinessV1>,
    movement_ease: Option<PassageMovementEaseV1>,
    room_needed: Option<PassageRoomNeededV1>,
    checkpoint: Option<PassageCheckpointV1>,
    anchor_role: Option<PassageAnchorRoleV1>,
    anchor_kind: Option<PassageAnchorKindV1>,
    anchor_association: Option<PassageAnchorAssociationV1>,
    anchor_ref: Option<String>,
    previous_anchor_event_id: Option<String>,
    bearing_strand: Option<PassageBearingStrandV1>,
    movement_resistance: Option<PassageMovementResistanceV1>,
    persistence_tendency: Option<PassagePersistenceTendencyV1>,
    witness_fit: Option<PassageWitnessFitV1>,
    previous_bearing_event_id: Option<String>,
    request_id: Option<String>,
    requested_peer: Option<String>,
    company_mode: Option<PassageCompanyModeV1>,
    company_response: Option<PassageCompanyResponseV1>,
    previous_company_event_id: Option<String>,
}

impl ContextEventFieldsV1 {
    fn from_request(request: Option<&CompanyRequestStateV1>) -> Self {
        Self {
            request_id: request.map(|value| value.request_id.clone()),
            requested_peer: request.map(|value| value.requested_peer.clone()),
            company_mode: request.map(|value| value.mode),
            previous_company_event_id: request.map(|value| value.latest_event_id.clone()),
            ..Self::default()
        }
    }
}

fn resolve_passage_and_request<'a>(
    records: &[Value],
    state: &'a ContextStateV1,
    selector: &str,
    actor: &str,
    action: PassageContextActionV1,
) -> Result<
    (
        phase_passages::PassageIdentityV1,
        Option<&'a CompanyRequestStateV1>,
    ),
    String,
> {
    match action {
        PassageContextActionV1::DescribeCondition
        | PassageContextActionV1::DescribeBearing
        | PassageContextActionV1::MarkCheckpoint
        | PassageContextActionV1::BindAnchor
        | PassageContextActionV1::RequestCompany => Ok((
            phase_passages::resolve_passage_identity(records, selector, actor)?,
            None,
        )),
        PassageContextActionV1::RespondCompany => {
            let request = latest_request(state, selector, actor, false)
                .ok_or_else(|| "no matching inbound company request".to_string())?;
            let passage = phase_passages::resolve_passage_identity(
                records,
                &request.passage_id,
                &request.passage_actor,
            )?;
            Ok((passage, Some(request)))
        },
        PassageContextActionV1::WithdrawCompany => {
            let request = latest_request(state, selector, actor, true)
                .ok_or_else(|| "no matching self-authored company request".to_string())?;
            let passage =
                phase_passages::resolve_passage_identity(records, &request.passage_id, actor)?;
            Ok((passage, Some(request)))
        },
    }
}

fn source_ref(raw: &str, action: PassageContextActionV1) -> Result<String, String> {
    phase_passages::bounded_ref(
        &field(
            raw,
            &[
                "source_ref",
                "felt_source_ref",
                "checkpoint_source_ref",
                "response_ref",
            ],
        )
        .ok_or_else(|| format!("{} requires source_ref", action.owner_action()))?,
    )
    .ok_or_else(|| "source_ref must be a bounded reference".to_string())
}

fn populate_condition(raw: &str, fields: &mut ContextEventFieldsV1) -> Result<(), String> {
    fields.readiness = Some(
        PassageReadinessV1::parse(
            &field(raw, &["readiness"])
                .ok_or_else(|| "condition requires readiness".to_string())?,
        )
        .ok_or_else(|| "unknown readiness".to_string())?,
    );
    fields.movement_ease = Some(
        PassageMovementEaseV1::parse(
            &field(raw, &["movement", "movement_ease", "ease"])
                .ok_or_else(|| "condition requires movement_ease".to_string())?,
        )
        .ok_or_else(|| "unknown movement_ease".to_string())?,
    );
    fields.room_needed = Some(
        PassageRoomNeededV1::parse(
            &field(raw, &["room", "room_needed", "support"])
                .ok_or_else(|| "condition requires room_needed".to_string())?,
        )
        .ok_or_else(|| "unknown room_needed".to_string())?,
    );
    Ok(())
}

fn populate_bearing(
    raw: &str,
    passage_id: &str,
    state: &ContextStateV1,
    fields: &mut ContextEventFieldsV1,
) -> Result<(), String> {
    fields.bearing_strand = Some(
        PassageBearingStrandV1::parse(
            &field(raw, &["strand", "bearing_strand"])
                .ok_or_else(|| "bearing action requires strand".to_string())?,
        )
        .ok_or_else(|| "unknown bearing strand".to_string())?,
    );
    fields.movement_resistance = Some(
        PassageMovementResistanceV1::parse(
            &field(raw, &["resistance", "movement_resistance"])
                .ok_or_else(|| "bearing action requires movement_resistance".to_string())?,
        )
        .ok_or_else(|| "unknown movement_resistance".to_string())?,
    );
    fields.persistence_tendency = Some(
        PassagePersistenceTendencyV1::parse(
            &field(raw, &["persistence", "persistence_tendency"])
                .ok_or_else(|| "bearing action requires persistence_tendency".to_string())?,
        )
        .ok_or_else(|| "unknown persistence_tendency".to_string())?,
    );
    fields.witness_fit = Some(
        PassageWitnessFitV1::parse(
            &field(raw, &["witness", "witness_fit"])
                .ok_or_else(|| "bearing action requires witness_fit".to_string())?,
        )
        .ok_or_else(|| "unknown witness_fit".to_string())?,
    );
    let key = format!(
        "{passage_id}:{}",
        fields
            .bearing_strand
            .map_or("unknown", PassageBearingStrandV1::as_str),
    );
    fields.previous_bearing_event_id = state
        .latest_bearing_by_passage_strand
        .get(&key)
        .map(|event| event.event_id.clone());
    Ok(())
}

fn populate_anchor(
    raw: &str,
    passage_id: &str,
    state: &ContextStateV1,
    fields: &mut ContextEventFieldsV1,
) -> Result<(), String> {
    fields.anchor_role = Some(
        PassageAnchorRoleV1::parse(
            &field(raw, &["role", "anchor_role"])
                .ok_or_else(|| "anchor action requires role".to_string())?,
        )
        .ok_or_else(|| "unknown anchor role".to_string())?,
    );
    fields.anchor_kind = Some(
        PassageAnchorKindV1::parse(
            &field(raw, &["kind", "anchor_kind"])
                .ok_or_else(|| "anchor action requires kind".to_string())?,
        )
        .ok_or_else(|| "unknown anchor kind".to_string())?,
    );
    fields.anchor_association = Some(
        PassageAnchorAssociationV1::parse(
            &field(raw, &["association", "anchor_association"])
                .ok_or_else(|| "anchor action requires association".to_string())?,
        )
        .ok_or_else(|| "unknown anchor association".to_string())?,
    );
    fields.anchor_ref = Some(
        phase_passages::bounded_ref(
            &field(raw, &["anchor", "anchor_ref", "binding_ref"])
                .ok_or_else(|| "anchor action requires anchor_ref".to_string())?,
        )
        .ok_or_else(|| "anchor_ref must be a bounded reference".to_string())?,
    );
    let key = format!(
        "{passage_id}:{}",
        fields
            .anchor_role
            .map_or("unknown", PassageAnchorRoleV1::as_str),
    );
    fields.previous_anchor_event_id = state
        .latest_anchor_by_passage_role
        .get(&key)
        .map(|event| event.event_id.clone());
    Ok(())
}

fn populate_company_request(
    raw: &str,
    passage_id: &str,
    actor: &str,
    source_ref: &str,
    recorded_at_unix_ms: u64,
    fields: &mut ContextEventFieldsV1,
) -> Result<(), String> {
    let peer = companion_actor(
        &field(raw, &["peer", "requested_peer"])
            .ok_or_else(|| "company request requires peer".to_string())?,
    )
    .ok_or_else(|| "company peer must be astrid or minime".to_string())?;
    if peer == actor {
        return Err("company request must name the other being".to_string());
    }
    let mode = PassageCompanyModeV1::parse(
        &field(raw, &["mode", "company_mode"])
            .ok_or_else(|| "company request requires mode".to_string())?,
    )
    .ok_or_else(|| "unknown company mode".to_string())?;
    fields.request_id = Some(company_request_id(
        passage_id,
        actor,
        &peer,
        mode,
        source_ref,
        recorded_at_unix_ms,
    ));
    fields.requested_peer = Some(peer);
    fields.company_mode = Some(mode);
    fields.previous_company_event_id = None;
    Ok(())
}

fn populate_action_fields(
    raw: &str,
    passage: &phase_passages::PassageIdentityV1,
    actor: &str,
    action: PassageContextActionV1,
    source_ref: &str,
    recorded_at_unix_ms: u64,
    state: &ContextStateV1,
    fields: &mut ContextEventFieldsV1,
) -> Result<(), String> {
    match action {
        PassageContextActionV1::DescribeCondition => populate_condition(raw, fields),
        PassageContextActionV1::DescribeBearing => {
            populate_bearing(raw, &passage.passage_id, state, fields)
        },
        PassageContextActionV1::MarkCheckpoint => {
            fields.checkpoint = Some(
                PassageCheckpointV1::parse(
                    &field(raw, &["checkpoint", "point"])
                        .ok_or_else(|| "checkpoint action requires checkpoint".to_string())?,
                )
                .ok_or_else(|| "unknown transition checkpoint".to_string())?,
            );
            Ok(())
        },
        PassageContextActionV1::BindAnchor => {
            populate_anchor(raw, &passage.passage_id, state, fields)
        },
        PassageContextActionV1::RequestCompany => populate_company_request(
            raw,
            &passage.passage_id,
            actor,
            source_ref,
            recorded_at_unix_ms,
            fields,
        ),
        PassageContextActionV1::RespondCompany => {
            fields.company_response = Some(
                PassageCompanyResponseV1::parse(
                    &field(raw, &["response", "company_response"])
                        .ok_or_else(|| "company response requires response".to_string())?,
                )
                .ok_or_else(|| "unknown company response".to_string())?,
            );
            Ok(())
        },
        PassageContextActionV1::WithdrawCompany => {
            fields.company_response = Some(PassageCompanyResponseV1::Withdraw);
            Ok(())
        },
    }
}

pub(super) fn build_event(
    records: &[Value],
    state: &ContextStateV1,
    selector: &str,
    raw: &str,
    actor: &str,
    action: PassageContextActionV1,
    recorded_at_unix_ms: u64,
) -> Result<LivedTransitionPassageContextEventV1, String> {
    let actor =
        phase_passages::bounded_ref(actor).ok_or_else(|| "invalid context actor".to_string())?;
    let (passage, request) = resolve_passage_and_request(records, state, selector, &actor, action)?;
    let source_ref = source_ref(raw, action)?;
    let mut fields = ContextEventFieldsV1::from_request(request);
    populate_action_fields(
        raw,
        &passage,
        &actor,
        action,
        &source_ref,
        recorded_at_unix_ms,
        state,
        &mut fields,
    )?;
    let previous_context_event_id = state.latest_by_passage.get(&passage.passage_id).cloned();
    let event_id = context_event_id(
        &passage.passage_id,
        &passage.transition_id,
        &passage.actor,
        &actor,
        action,
        fields.readiness,
        fields.movement_ease,
        fields.room_needed,
        fields.checkpoint,
        fields.anchor_role,
        fields.anchor_kind,
        fields.anchor_association,
        fields.anchor_ref.as_deref(),
        fields.previous_anchor_event_id.as_deref(),
        fields.bearing_strand,
        fields.movement_resistance,
        fields.persistence_tendency,
        fields.witness_fit,
        fields.previous_bearing_event_id.as_deref(),
        fields.request_id.as_deref(),
        fields.requested_peer.as_deref(),
        fields.company_mode,
        fields.company_response,
        &source_ref,
        previous_context_event_id.as_deref(),
        fields.previous_company_event_id.as_deref(),
        recorded_at_unix_ms,
    );
    Ok(LivedTransitionPassageContextEventV1 {
        schema: SCHEMA,
        schema_version: 1,
        record_type: RECORD_TYPE,
        record_id: event_id.clone(),
        passage_context_event_id: event_id,
        passage_id: passage.passage_id,
        transition_id: passage.transition_id,
        passage_actor: passage.actor,
        actor,
        action,
        readiness: fields.readiness,
        movement_ease: fields.movement_ease,
        room_needed: fields.room_needed,
        checkpoint: fields.checkpoint,
        anchor_role: fields.anchor_role,
        anchor_kind: fields.anchor_kind,
        anchor_association: fields.anchor_association,
        anchor_ref: fields.anchor_ref,
        previous_anchor_event_id: fields.previous_anchor_event_id,
        bearing_strand: fields.bearing_strand,
        movement_resistance: fields.movement_resistance,
        persistence_tendency: fields.persistence_tendency,
        witness_fit: fields.witness_fit,
        previous_bearing_event_id: fields.previous_bearing_event_id,
        company_request_id: fields.request_id,
        requested_peer: fields.requested_peer,
        company_mode: fields.company_mode,
        company_response: fields.company_response,
        source_ref,
        previous_context_event_id,
        previous_company_event_id: fields.previous_company_event_id,
        recorded_at_unix_ms,
        owner_language_action: action.owner_action(),
        self_authored_only: true,
        passage_stage_changed: false,
        response_revisable: true,
        right_to_ignore: true,
        felt_score_present: false,
        mechanical_causation_inferred: false,
        peer_consent_inferred: false,
        peer_state_changed: false,
        silence_infers_response: false,
        automatic_progression: false,
        felt_resolution_inferred: false,
        scheduler_effect: false,
        model_qos_effect: false,
        substrate_effect: false,
        dispatch_effect: false,
        live_control_effect: false,
        runtime_unlock_applied: false,
        anchor_mechanical_truth_inferred: false,
        anchor_changes_passage: false,
        anchor_closes_transition: false,
        bearing_is_metric: false,
        bearing_inferred_from_telemetry: false,
        bearing_changes_passage: false,
        bearing_closes_transition: false,
        raw_prose_included: false,
        artifact_authority_state_v1: EvidenceOnlyAuthorityV1::new(),
    })
}

fn latest_request<'a>(
    state: &'a ContextStateV1,
    selector: &str,
    actor: &str,
    own: bool,
) -> Option<&'a CompanyRequestStateV1> {
    state
        .requests
        .values()
        .filter(|request| {
            if own {
                request.passage_actor == actor
            } else {
                request.requested_peer == actor
            }
        })
        .filter(|request| {
            selector == "latest" || selector.is_empty() || request.request_id == selector
        })
        .max_by_key(|request| request.recorded_at_unix_ms)
}
