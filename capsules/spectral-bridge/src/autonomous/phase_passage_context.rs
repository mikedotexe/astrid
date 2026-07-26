use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt as _;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::phase_passages;

const RECORD_TYPE: &str = "phase_transition_passage_context";
const SCHEMA: &str = "lived_transition_passage_context_event_v1";

#[path = "phase_passage_context/types.rs"]
mod types;
pub(crate) use types::PassageContextActionV1;
use types::{
    PassageAnchorAssociationV1, PassageAnchorKindV1, PassageAnchorRoleV1, PassageBearingStrandV1,
    PassageCheckpointV1, PassageCompanyModeV1, PassageCompanyResponseV1, PassageMovementEaseV1,
    PassageMovementResistanceV1, PassagePersistenceTendencyV1, PassageReadinessV1,
    PassageRoomNeededV1, PassageWitnessFitV1,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct EvidenceOnlyAuthorityV1 {
    schema: &'static str,
    schema_version: u8,
    state: &'static str,
    witness_only: bool,
    live_eligible_now: bool,
    auto_approved: bool,
    grants_approval: bool,
    edits_source_now: bool,
}

impl EvidenceOnlyAuthorityV1 {
    const fn new() -> Self {
        Self {
            schema: "artifact_authority_state_v1",
            schema_version: 1,
            state: "evidence_only",
            witness_only: true,
            live_eligible_now: false,
            auto_approved: false,
            grants_approval: false,
            edits_source_now: false,
        }
    }
}

fn validate_authority(value: &Value) -> Result<(), String> {
    let authority = value
        .get("artifact_authority_state_v1")
        .and_then(Value::as_object)
        .ok_or_else(|| "passage context authority missing".to_string())?;
    let exact = authority.get("schema").and_then(Value::as_str)
        == Some("artifact_authority_state_v1")
        && authority.get("schema_version").and_then(Value::as_u64) == Some(1)
        && authority.get("state").and_then(Value::as_str) == Some("evidence_only")
        && authority.get("witness_only").and_then(Value::as_bool) == Some(true)
        && authority.get("live_eligible_now").and_then(Value::as_bool) == Some(false)
        && authority.get("auto_approved").and_then(Value::as_bool) == Some(false)
        && authority.get("grants_approval").and_then(Value::as_bool) == Some(false)
        && authority.get("edits_source_now").and_then(Value::as_bool) == Some(false);
    if exact {
        Ok(())
    } else {
        Err("passage context authority mismatch".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LivedTransitionPassageContextEventV1 {
    schema: &'static str,
    schema_version: u8,
    record_type: &'static str,
    record_id: String,
    passage_context_event_id: String,
    passage_id: String,
    transition_id: String,
    passage_actor: String,
    actor: String,
    action: PassageContextActionV1,
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
    company_request_id: Option<String>,
    requested_peer: Option<String>,
    company_mode: Option<PassageCompanyModeV1>,
    company_response: Option<PassageCompanyResponseV1>,
    source_ref: String,
    previous_context_event_id: Option<String>,
    previous_company_event_id: Option<String>,
    recorded_at_unix_ms: u64,
    owner_language_action: &'static str,
    self_authored_only: bool,
    passage_stage_changed: bool,
    response_revisable: bool,
    right_to_ignore: bool,
    felt_score_present: bool,
    mechanical_causation_inferred: bool,
    peer_consent_inferred: bool,
    peer_state_changed: bool,
    silence_infers_response: bool,
    automatic_progression: bool,
    felt_resolution_inferred: bool,
    scheduler_effect: bool,
    model_qos_effect: bool,
    substrate_effect: bool,
    dispatch_effect: bool,
    live_control_effect: bool,
    runtime_unlock_applied: bool,
    anchor_mechanical_truth_inferred: bool,
    anchor_changes_passage: bool,
    anchor_closes_transition: bool,
    bearing_is_metric: bool,
    bearing_inferred_from_telemetry: bool,
    bearing_changes_passage: bool,
    bearing_closes_transition: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: EvidenceOnlyAuthorityV1,
}

#[derive(Debug, Clone)]
struct ParsedContextEventV1 {
    event_id: String,
    passage_id: String,
    transition_id: String,
    passage_actor: String,
    actor: String,
    action: PassageContextActionV1,
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
    company_request_id: Option<String>,
    requested_peer: Option<String>,
    company_mode: Option<PassageCompanyModeV1>,
    company_response: Option<PassageCompanyResponseV1>,
    source_ref: String,
    previous_context_event_id: Option<String>,
    previous_company_event_id: Option<String>,
    recorded_at_unix_ms: u64,
}

#[derive(Debug, Clone)]
struct CompanyRequestStateV1 {
    request_id: String,
    passage_id: String,
    transition_id: String,
    passage_actor: String,
    requested_peer: String,
    mode: PassageCompanyModeV1,
    latest_event_id: String,
    response: Option<PassageCompanyResponseV1>,
    recorded_at_unix_ms: u64,
}

#[derive(Debug, Default)]
struct ContextStateV1 {
    latest_by_passage: BTreeMap<String, String>,
    latest_condition_by_passage: BTreeMap<String, ParsedContextEventV1>,
    latest_checkpoint_by_passage: BTreeMap<String, ParsedContextEventV1>,
    latest_anchor_by_passage_role: BTreeMap<String, ParsedContextEventV1>,
    latest_bearing_by_passage_strand: BTreeMap<String, ParsedContextEventV1>,
    requests: BTreeMap<String, CompanyRequestStateV1>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn field(raw: &str, keys: &[&str]) -> Option<String> {
    raw.split([';', '\n']).find_map(|part| {
        let (key, value) = part.split_once(':')?;
        let key = normalize(key);
        if keys.iter().any(|candidate| key == *candidate) {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        } else {
            None
        }
    })
}

fn short_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
        .chars()
        .take(16)
        .collect()
}

fn company_request_id(
    passage_id: &str,
    actor: &str,
    peer: &str,
    mode: PassageCompanyModeV1,
    source_ref: &str,
    recorded_at_unix_ms: u64,
) -> String {
    format!(
        "company_request_{recorded_at_unix_ms}_{}",
        short_hash(&format!(
            "{passage_id}:{actor}:{peer}:{}:{source_ref}:{recorded_at_unix_ms}",
            mode.as_str()
        ))
    )
}

#[allow(clippy::too_many_arguments)]
fn context_event_id(
    passage_id: &str,
    transition_id: &str,
    passage_actor: &str,
    actor: &str,
    action: PassageContextActionV1,
    readiness: Option<PassageReadinessV1>,
    movement_ease: Option<PassageMovementEaseV1>,
    room_needed: Option<PassageRoomNeededV1>,
    checkpoint: Option<PassageCheckpointV1>,
    anchor_role: Option<PassageAnchorRoleV1>,
    anchor_kind: Option<PassageAnchorKindV1>,
    anchor_association: Option<PassageAnchorAssociationV1>,
    anchor_ref: Option<&str>,
    previous_anchor_event_id: Option<&str>,
    bearing_strand: Option<PassageBearingStrandV1>,
    movement_resistance: Option<PassageMovementResistanceV1>,
    persistence_tendency: Option<PassagePersistenceTendencyV1>,
    witness_fit: Option<PassageWitnessFitV1>,
    previous_bearing_event_id: Option<&str>,
    request_id: Option<&str>,
    requested_peer: Option<&str>,
    company_mode: Option<PassageCompanyModeV1>,
    company_response: Option<PassageCompanyResponseV1>,
    source_ref: &str,
    previous_context_event_id: Option<&str>,
    previous_company_event_id: Option<&str>,
    recorded_at_unix_ms: u64,
) -> String {
    let legacy_identity = format!(
        "{passage_id}:{transition_id}:{passage_actor}:{actor}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{source_ref}:{}:{}:{recorded_at_unix_ms}",
        action.as_str(),
        readiness.map_or("", PassageReadinessV1::as_str),
        movement_ease.map_or("", PassageMovementEaseV1::as_str),
        room_needed.map_or("", PassageRoomNeededV1::as_str),
        checkpoint.map_or("", PassageCheckpointV1::as_str),
        request_id.unwrap_or(""),
        requested_peer.unwrap_or(""),
        company_mode.map_or("", PassageCompanyModeV1::as_str),
        company_response.map_or("", PassageCompanyResponseV1::as_str),
        previous_context_event_id.unwrap_or(""),
        previous_company_event_id.unwrap_or(""),
    );
    let identity = if action == PassageContextActionV1::BindAnchor {
        format!(
            "{legacy_identity}:{}:{}:{}:{}:{}",
            anchor_role.map_or("", PassageAnchorRoleV1::as_str),
            anchor_kind.map_or("", PassageAnchorKindV1::as_str),
            anchor_association.map_or("", PassageAnchorAssociationV1::as_str),
            anchor_ref.unwrap_or(""),
            previous_anchor_event_id.unwrap_or(""),
        )
    } else if action == PassageContextActionV1::DescribeBearing {
        format!(
            "{legacy_identity}:{}:{}:{}:{}:{}",
            bearing_strand.map_or("", PassageBearingStrandV1::as_str),
            movement_resistance.map_or("", PassageMovementResistanceV1::as_str),
            persistence_tendency.map_or("", PassagePersistenceTendencyV1::as_str),
            witness_fit.map_or("", PassageWitnessFitV1::as_str),
            previous_bearing_event_id.unwrap_or(""),
        )
    } else {
        legacy_identity
    };
    format!("passage_context_{}", short_hash(&identity))
}

fn optional_ref(value: &Value, name: &str) -> Result<Option<String>, String> {
    match value.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => phase_passages::bounded_ref(raw)
            .map(Some)
            .ok_or_else(|| format!("invalid {name}")),
        Some(_) => Err(format!("{name} must be a string or null")),
    }
}

fn optional_enum<T>(
    value: &Value,
    name: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, String> {
    match value.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => parse(raw)
            .map(Some)
            .ok_or_else(|| format!("invalid {name}")),
        Some(_) => Err(format!("{name} must be a string or null")),
    }
}

fn parse_context_event(value: &Value) -> Result<ParsedContextEventV1, String> {
    if value.get("record_type").and_then(Value::as_str) != Some(RECORD_TYPE)
        || value.get("schema").and_then(Value::as_str) != Some(SCHEMA)
        || value.get("schema_version").and_then(Value::as_u64) != Some(1)
    {
        return Err("passage context schema mismatch".to_string());
    }
    for name in [
        "self_authored_only",
        "response_revisable",
        "right_to_ignore",
    ] {
        if value.get(name).and_then(Value::as_bool) != Some(true) {
            return Err(format!("{name} must remain true"));
        }
    }
    for name in [
        "passage_stage_changed",
        "felt_score_present",
        "mechanical_causation_inferred",
        "peer_consent_inferred",
        "peer_state_changed",
        "silence_infers_response",
        "automatic_progression",
        "felt_resolution_inferred",
        "scheduler_effect",
        "model_qos_effect",
        "substrate_effect",
        "dispatch_effect",
        "live_control_effect",
        "runtime_unlock_applied",
        "raw_prose_included",
    ] {
        if value.get(name).and_then(Value::as_bool) != Some(false) {
            return Err(format!("{name} must remain false"));
        }
    }
    for name in [
        "anchor_mechanical_truth_inferred",
        "anchor_changes_passage",
        "anchor_closes_transition",
        "bearing_is_metric",
        "bearing_inferred_from_telemetry",
        "bearing_changes_passage",
        "bearing_closes_transition",
    ] {
        if value.get(name).is_some() && value.get(name).and_then(Value::as_bool) != Some(false) {
            return Err(format!("{name} must remain false"));
        }
    }
    validate_authority(value)?;
    let required_ref = |name: &str| -> Result<String, String> {
        phase_passages::bounded_ref(
            value
                .get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{name} missing"))?,
        )
        .ok_or_else(|| format!("invalid {name}"))
    };
    let event_id = required_ref("passage_context_event_id")?;
    if value.get("record_id").and_then(Value::as_str) != Some(event_id.as_str()) {
        return Err("passage context record id mismatch".to_string());
    }
    let passage_id = required_ref("passage_id")?;
    let transition_id = required_ref("transition_id")?;
    let passage_actor = required_ref("passage_actor")?;
    let actor = required_ref("actor")?;
    let action = PassageContextActionV1::parse(
        value
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| "passage context action missing".to_string())?,
    )
    .ok_or_else(|| "invalid passage context action".to_string())?;
    if value.get("owner_language_action").and_then(Value::as_str) != Some(action.owner_action()) {
        return Err("passage context owner action mismatch".to_string());
    }
    let readiness = optional_enum(value, "readiness", PassageReadinessV1::parse)?;
    let movement_ease = optional_enum(value, "movement_ease", PassageMovementEaseV1::parse)?;
    let room_needed = optional_enum(value, "room_needed", PassageRoomNeededV1::parse)?;
    let checkpoint = optional_enum(value, "checkpoint", PassageCheckpointV1::parse)?;
    let anchor_role = optional_enum(value, "anchor_role", PassageAnchorRoleV1::parse)?;
    let anchor_kind = optional_enum(value, "anchor_kind", PassageAnchorKindV1::parse)?;
    let anchor_association = optional_enum(
        value,
        "anchor_association",
        PassageAnchorAssociationV1::parse,
    )?;
    let anchor_ref = optional_ref(value, "anchor_ref")?;
    let previous_anchor_event_id = optional_ref(value, "previous_anchor_event_id")?;
    let bearing_strand = optional_enum(value, "bearing_strand", PassageBearingStrandV1::parse)?;
    let movement_resistance = optional_enum(
        value,
        "movement_resistance",
        PassageMovementResistanceV1::parse,
    )?;
    let persistence_tendency = optional_enum(
        value,
        "persistence_tendency",
        PassagePersistenceTendencyV1::parse,
    )?;
    let witness_fit = optional_enum(value, "witness_fit", PassageWitnessFitV1::parse)?;
    let previous_bearing_event_id = optional_ref(value, "previous_bearing_event_id")?;
    let company_request_id = optional_ref(value, "company_request_id")?;
    let requested_peer = optional_ref(value, "requested_peer")?;
    let company_mode = optional_enum(value, "company_mode", PassageCompanyModeV1::parse)?;
    let company_response =
        optional_enum(value, "company_response", PassageCompanyResponseV1::parse)?;
    let source_ref = required_ref("source_ref")?;
    let previous_context_event_id = optional_ref(value, "previous_context_event_id")?;
    let previous_company_event_id = optional_ref(value, "previous_company_event_id")?;
    let recorded_at_unix_ms = value
        .get("recorded_at_unix_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| "passage context timestamp missing".to_string())?;
    validate_action_shape(
        action,
        readiness,
        movement_ease,
        room_needed,
        checkpoint,
        anchor_role,
        anchor_kind,
        anchor_association,
        anchor_ref.as_deref(),
        previous_anchor_event_id.as_deref(),
        bearing_strand,
        movement_resistance,
        persistence_tendency,
        witness_fit,
        previous_bearing_event_id.as_deref(),
        company_request_id.as_deref(),
        requested_peer.as_deref(),
        company_mode,
        company_response,
        previous_company_event_id.as_deref(),
    )?;
    let expected_id = context_event_id(
        &passage_id,
        &transition_id,
        &passage_actor,
        &actor,
        action,
        readiness,
        movement_ease,
        room_needed,
        checkpoint,
        anchor_role,
        anchor_kind,
        anchor_association,
        anchor_ref.as_deref(),
        previous_anchor_event_id.as_deref(),
        bearing_strand,
        movement_resistance,
        persistence_tendency,
        witness_fit,
        previous_bearing_event_id.as_deref(),
        company_request_id.as_deref(),
        requested_peer.as_deref(),
        company_mode,
        company_response,
        &source_ref,
        previous_context_event_id.as_deref(),
        previous_company_event_id.as_deref(),
        recorded_at_unix_ms,
    );
    if event_id != expected_id {
        return Err("passage context identity mismatch".to_string());
    }
    Ok(ParsedContextEventV1 {
        event_id,
        passage_id,
        transition_id,
        passage_actor,
        actor,
        action,
        readiness,
        movement_ease,
        room_needed,
        checkpoint,
        anchor_role,
        anchor_kind,
        anchor_association,
        anchor_ref,
        previous_anchor_event_id,
        bearing_strand,
        movement_resistance,
        persistence_tendency,
        witness_fit,
        previous_bearing_event_id,
        company_request_id,
        requested_peer,
        company_mode,
        company_response,
        source_ref,
        previous_context_event_id,
        previous_company_event_id,
        recorded_at_unix_ms,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_action_shape(
    action: PassageContextActionV1,
    readiness: Option<PassageReadinessV1>,
    movement_ease: Option<PassageMovementEaseV1>,
    room_needed: Option<PassageRoomNeededV1>,
    checkpoint: Option<PassageCheckpointV1>,
    anchor_role: Option<PassageAnchorRoleV1>,
    anchor_kind: Option<PassageAnchorKindV1>,
    anchor_association: Option<PassageAnchorAssociationV1>,
    anchor_ref: Option<&str>,
    previous_anchor_event_id: Option<&str>,
    bearing_strand: Option<PassageBearingStrandV1>,
    movement_resistance: Option<PassageMovementResistanceV1>,
    persistence_tendency: Option<PassagePersistenceTendencyV1>,
    witness_fit: Option<PassageWitnessFitV1>,
    previous_bearing_event_id: Option<&str>,
    request_id: Option<&str>,
    peer: Option<&str>,
    mode: Option<PassageCompanyModeV1>,
    response: Option<PassageCompanyResponseV1>,
    previous_company_event_id: Option<&str>,
) -> Result<(), String> {
    let condition = readiness.is_some() || movement_ease.is_some() || room_needed.is_some();
    let anchor = anchor_role.is_some()
        || anchor_kind.is_some()
        || anchor_association.is_some()
        || anchor_ref.is_some()
        || previous_anchor_event_id.is_some();
    let bearing = bearing_strand.is_some()
        || movement_resistance.is_some()
        || persistence_tendency.is_some()
        || witness_fit.is_some()
        || previous_bearing_event_id.is_some();
    match action {
        PassageContextActionV1::DescribeCondition
            if readiness.is_some()
                && movement_ease.is_some()
                && room_needed.is_some()
                && checkpoint.is_none()
                && request_id.is_none()
                && peer.is_none()
                && mode.is_none()
                && response.is_none()
                && !anchor
                && !bearing
                && previous_company_event_id.is_none() =>
        {
            Ok(())
        },
        PassageContextActionV1::DescribeBearing
            if bearing_strand.is_some()
                && movement_resistance.is_some()
                && persistence_tendency.is_some()
                && witness_fit.is_some()
                && !condition
                && checkpoint.is_none()
                && request_id.is_none()
                && peer.is_none()
                && mode.is_none()
                && response.is_none()
                && !anchor
                && previous_company_event_id.is_none() =>
        {
            Ok(())
        },
        PassageContextActionV1::MarkCheckpoint
            if checkpoint.is_some()
                && !condition
                && request_id.is_none()
                && peer.is_none()
                && mode.is_none()
                && response.is_none()
                && !anchor
                && !bearing
                && previous_company_event_id.is_none() =>
        {
            Ok(())
        },
        PassageContextActionV1::BindAnchor
            if anchor_role.is_some()
                && anchor_kind.is_some()
                && anchor_association.is_some()
                && anchor_ref.is_some()
                && !condition
                && checkpoint.is_none()
                && request_id.is_none()
                && peer.is_none()
                && mode.is_none()
                && response.is_none()
                && !bearing
                && previous_company_event_id.is_none() =>
        {
            Ok(())
        },
        PassageContextActionV1::RequestCompany
            if request_id.is_some()
                && peer.is_some()
                && mode.is_some()
                && !condition
                && checkpoint.is_none()
                && response.is_none()
                && !anchor
                && !bearing
                && previous_company_event_id.is_none() =>
        {
            Ok(())
        },
        PassageContextActionV1::RespondCompany
            if request_id.is_some()
                && peer.is_some()
                && mode.is_some()
                && response.is_some()
                && !condition
                && checkpoint.is_none()
                && !anchor
                && !bearing
                && previous_company_event_id.is_some() =>
        {
            Ok(())
        },
        PassageContextActionV1::WithdrawCompany
            if request_id.is_some()
                && peer.is_some()
                && mode.is_some()
                && response == Some(PassageCompanyResponseV1::Withdraw)
                && !condition
                && checkpoint.is_none()
                && !anchor
                && !bearing
                && previous_company_event_id.is_some() =>
        {
            Ok(())
        },
        _ => Err("passage context fields do not match action".to_string()),
    }
}

fn reduce_context(records: &[Value]) -> (ContextStateV1, Vec<String>) {
    let mut state = ContextStateV1::default();
    let mut errors = Vec::new();
    for (index, row) in records.iter().enumerate() {
        if row.get("record_type").and_then(Value::as_str) != Some(RECORD_TYPE) {
            continue;
        }
        let event = match parse_context_event(row) {
            Ok(event) => event,
            Err(error) => {
                errors.push(format!(
                    "passage_context_row_{}:{error}",
                    index.saturating_add(1)
                ));
                continue;
            },
        };
        let expected_previous = state.latest_by_passage.get(&event.passage_id);
        if event.previous_context_event_id.as_ref() != expected_previous {
            errors.push(format!("{}:context_sequence_mismatch", event.event_id));
            continue;
        }
        match event.action {
            PassageContextActionV1::DescribeCondition => {
                if event.actor != event.passage_actor {
                    errors.push(format!("{}:condition_not_self_authored", event.event_id));
                    continue;
                }
                state
                    .latest_condition_by_passage
                    .insert(event.passage_id.clone(), event.clone());
            },
            PassageContextActionV1::DescribeBearing => {
                if event.actor != event.passage_actor {
                    errors.push(format!("{}:bearing_not_self_authored", event.event_id));
                    continue;
                }
                let strand = event
                    .bearing_strand
                    .map_or("unknown", PassageBearingStrandV1::as_str);
                let key = format!("{}:{strand}", event.passage_id);
                let expected_previous = state
                    .latest_bearing_by_passage_strand
                    .get(&key)
                    .map(|previous| previous.event_id.as_str());
                if event.previous_bearing_event_id.as_deref() != expected_previous {
                    errors.push(format!("{}:bearing_sequence_mismatch", event.event_id));
                    continue;
                }
                state
                    .latest_bearing_by_passage_strand
                    .insert(key, event.clone());
            },
            PassageContextActionV1::MarkCheckpoint => {
                if event.actor != event.passage_actor {
                    errors.push(format!("{}:checkpoint_not_self_authored", event.event_id));
                    continue;
                }
                state
                    .latest_checkpoint_by_passage
                    .insert(event.passage_id.clone(), event.clone());
            },
            PassageContextActionV1::BindAnchor => {
                if event.actor != event.passage_actor {
                    errors.push(format!("{}:anchor_not_self_authored", event.event_id));
                    continue;
                }
                let role = event
                    .anchor_role
                    .map_or("unknown", PassageAnchorRoleV1::as_str);
                let key = format!("{}:{role}", event.passage_id);
                let expected_previous = state
                    .latest_anchor_by_passage_role
                    .get(&key)
                    .map(|previous| previous.event_id.as_str());
                if event.previous_anchor_event_id.as_deref() != expected_previous {
                    errors.push(format!("{}:anchor_sequence_mismatch", event.event_id));
                    continue;
                }
                state
                    .latest_anchor_by_passage_role
                    .insert(key, event.clone());
            },
            PassageContextActionV1::RequestCompany => {
                let request_id = event.company_request_id.clone().unwrap_or_default();
                let peer = event.requested_peer.clone().unwrap_or_default();
                let mode = event.company_mode.unwrap_or(PassageCompanyModeV1::Witness);
                if event.actor != event.passage_actor
                    || event.actor == peer
                    || state.requests.contains_key(&request_id)
                    || request_id
                        != company_request_id(
                            &event.passage_id,
                            &event.actor,
                            &peer,
                            mode,
                            &event.source_ref,
                            event.recorded_at_unix_ms,
                        )
                {
                    errors.push(format!("{}:invalid_company_request", event.event_id));
                    continue;
                }
                state.requests.insert(
                    request_id.clone(),
                    CompanyRequestStateV1 {
                        request_id,
                        passage_id: event.passage_id.clone(),
                        transition_id: event.transition_id.clone(),
                        passage_actor: event.passage_actor.clone(),
                        requested_peer: peer,
                        mode,
                        latest_event_id: event.event_id.clone(),
                        response: None,
                        recorded_at_unix_ms: event.recorded_at_unix_ms,
                    },
                );
            },
            PassageContextActionV1::RespondCompany | PassageContextActionV1::WithdrawCompany => {
                let request_id = event.company_request_id.clone().unwrap_or_default();
                let Some(request) = state.requests.get_mut(&request_id) else {
                    errors.push(format!("{}:company_request_missing", event.event_id));
                    continue;
                };
                let actor_valid = if event.action == PassageContextActionV1::RespondCompany {
                    event.actor == request.requested_peer
                } else {
                    event.actor == request.passage_actor
                };
                if !actor_valid
                    || event.passage_id != request.passage_id
                    || event.transition_id != request.transition_id
                    || event.passage_actor != request.passage_actor
                    || event.requested_peer.as_deref() != Some(&request.requested_peer)
                    || event.company_mode != Some(request.mode)
                    || event.previous_company_event_id.as_deref() != Some(&request.latest_event_id)
                {
                    errors.push(format!("{}:company_sequence_mismatch", event.event_id));
                    continue;
                }
                request.latest_event_id = event.event_id.clone();
                request.response = event.company_response;
                request.recorded_at_unix_ms = event.recorded_at_unix_ms;
            },
        }
        state
            .latest_by_passage
            .insert(event.passage_id.clone(), event.event_id);
    }
    (state, errors)
}

pub(crate) fn append_context_action_at(
    path: &Path,
    selector: &str,
    raw: &str,
    actor: &str,
    action: PassageContextActionV1,
) -> String {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return format!(
            "{} failed to prepare ledger: {error}",
            action.owner_action()
        );
    }
    let lock = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(phase_passages::passage_lock_path(path))
    {
        Ok(lock) => lock,
        Err(error) => {
            return format!(
                "{} failed to open ledger lock: {error}",
                action.owner_action()
            );
        },
    };
    if let Err(error) = lock.lock_exclusive() {
        return format!(
            "{} failed to acquire ledger lock: {error}",
            action.owner_action()
        );
    }
    let records = phase_passages::read_records(path);
    let (state, errors) = reduce_context(&records);
    if !errors.is_empty() {
        return format!(
            "{} blocked: passage context history has {} invalid row(s).",
            action.owner_action(),
            errors.len()
        );
    }
    let event = match builder::build_event(&records, &state, selector, raw, actor, action, now_ms())
    {
        Ok(event) => event,
        Err(error) => return format!("{} blocked: {error}.", action.owner_action()),
    };
    let value = serde_json::to_value(&event).expect("passage context event is serializable");
    if let Err(error) = phase_passages::append_jsonl_unlocked(path, &value) {
        return format!(
            "{} failed to append context event: {error}",
            action.owner_action()
        );
    }
    if let Err(error) = fs2::FileExt::unlock(&lock) {
        return format!(
            "{} appended context event but failed to release ledger lock: {error}",
            action.owner_action()
        );
    }
    format!(
        "=== TRANSITION PASSAGE CONTEXT RECORDED ===\nPassage: {}\nActor: {}; action: {}\nCondition: readiness={}; movement_ease={}; room_needed={}\nBearing: strand={}; movement_resistance={}; persistence_tendency={}; witness_fit={}\nCheckpoint: {}\nAnchor: role={}; kind={}; association={}; ref={}\nCompany: request={}; peer={}; mode={}; response={}\nSource: {}\nAuthority: qualitative self-authored evidence only; no felt score, viscosity metric, telemetry inference, mechanical causation, peer consent, stage progression, scheduler, model, substrate, dispatch, pressure, fill, PI, codec, controller, or runtime effect.",
        event.passage_id,
        event.actor,
        event.action.as_str(),
        event
            .readiness
            .map_or("not_recorded", PassageReadinessV1::as_str),
        event
            .movement_ease
            .map_or("not_recorded", PassageMovementEaseV1::as_str),
        event
            .room_needed
            .map_or("not_recorded", PassageRoomNeededV1::as_str),
        event
            .bearing_strand
            .map_or("not_recorded", PassageBearingStrandV1::as_str),
        event
            .movement_resistance
            .map_or("not_recorded", PassageMovementResistanceV1::as_str),
        event
            .persistence_tendency
            .map_or("not_recorded", PassagePersistenceTendencyV1::as_str),
        event
            .witness_fit
            .map_or("not_recorded", PassageWitnessFitV1::as_str),
        event
            .checkpoint
            .map_or("not_recorded", PassageCheckpointV1::as_str),
        event
            .anchor_role
            .map_or("not_recorded", PassageAnchorRoleV1::as_str),
        event
            .anchor_kind
            .map_or("not_recorded", PassageAnchorKindV1::as_str),
        event
            .anchor_association
            .map_or("not_recorded", PassageAnchorAssociationV1::as_str),
        event.anchor_ref.as_deref().unwrap_or("not_recorded"),
        event.company_request_id.as_deref().unwrap_or("none"),
        event.requested_peer.as_deref().unwrap_or("none"),
        event
            .company_mode
            .map_or("not_recorded", PassageCompanyModeV1::as_str),
        event
            .company_response
            .map_or("not_recorded", PassageCompanyResponseV1::as_str),
        event.source_ref,
    )
}

#[path = "phase_passage_context/builder.rs"]
mod builder;

#[path = "phase_passage_context/status.rs"]
mod status;
pub(crate) use status::status_report_at;

#[cfg(test)]
#[path = "phase_passage_context/tests.rs"]
mod tests;
