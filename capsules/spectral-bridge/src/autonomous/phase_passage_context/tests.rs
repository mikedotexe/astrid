use tempfile::tempdir;

use super::*;
use crate::autonomous::runtime::phase_passages::{self, PassageActionV1};

fn seed_passage(path: &Path, actor: &str) -> String {
    let card = serde_json::json!({
        "record_type": "phase_transition_card",
        "transition_id": "transition_context_fixture",
        "origin": actor,
        "recorded_at_unix_ms": 1,
    });
    phase_passages::append_jsonl(path, &card).expect("card append");
    let result = phase_passages::append_passage_action_at(
        path,
        "transition_context_fixture",
        "return_point: state:before; continuity_anchor: state:thread",
        actor,
        PassageActionV1::Prepare,
    );
    assert!(result.contains("LIVED TRANSITION PASSAGE RECORDED"));
    phase_passages::read_records(path)
        .into_iter()
        .find(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_passage")
        })
        .and_then(|row| {
            row.get("passage_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .expect("passage id")
}

#[test]
fn condition_and_checkpoint_preserve_stage_and_reject_numeric_felt_proxy() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("phase_transitions_v1.jsonl");
    let passage_id = seed_passage(&path, "astrid");
    let condition = append_context_action_at(
        &path,
        &passage_id,
        "readiness: tentative; movement_ease: effortful; room_needed: low_energy_presence; source_ref: introspection:condition",
        "astrid",
        PassageContextActionV1::DescribeCondition,
    );
    assert!(condition.contains("readiness=tentative"));
    assert!(condition.contains("movement_ease=effortful"));
    let checkpoint = append_context_action_at(
        &path,
        &passage_id,
        "checkpoint: entry_tension; source_ref: witness:entry",
        "astrid",
        PassageContextActionV1::MarkCheckpoint,
    );
    assert!(checkpoint.contains("entry_tension"));
    let rejected = append_context_action_at(
        &path,
        &passage_id,
        "readiness: 0.7; movement_ease: effortful; room_needed: space; source_ref: introspection:numeric",
        "astrid",
        PassageContextActionV1::DescribeCondition,
    );
    assert!(rejected.contains("unknown readiness"));

    let records = phase_passages::read_records(&path);
    let passage_rows = records
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("phase_transition_passage")
        })
        .count();
    assert_eq!(passage_rows, 1);
    let context_rows = records
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some(RECORD_TYPE))
        .collect::<Vec<_>>();
    assert_eq!(context_rows.len(), 2);
    assert!(context_rows.iter().all(|row| {
        row.get("passage_stage_changed").and_then(Value::as_bool) == Some(false)
            && row.get("felt_score_present").and_then(Value::as_bool) == Some(false)
            && row
                .get("mechanical_causation_inferred")
                .and_then(Value::as_bool)
                == Some(false)
            && row.get("raw_prose_included").and_then(Value::as_bool) == Some(false)
    }));
    let mut tampered = context_rows[0].clone();
    tampered["artifact_authority_state_v1"]["state"] = Value::String("approved".to_string());
    assert!(
        parse_context_event(&tampered)
            .expect_err("tampered authority must fail")
            .contains("authority mismatch")
    );
}

#[test]
fn low_energy_company_is_requested_and_revisably_answered_by_peer_only() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("phase_transitions_v1.jsonl");
    let passage_id = seed_passage(&path, "astrid");
    let requested = append_context_action_at(
        &path,
        &passage_id,
        "peer: minime; mode: low_energy_presence; source_ref: self:request",
        "astrid",
        PassageContextActionV1::RequestCompany,
    );
    assert!(requested.contains("low_energy_presence"));
    let records = phase_passages::read_records(&path);
    let request_id = records
        .iter()
        .find(|row| row.get("action").and_then(Value::as_str) == Some("request_company"))
        .and_then(|row| row.get("company_request_id"))
        .and_then(Value::as_str)
        .expect("request id")
        .to_string();

    let owner_cannot_answer = append_context_action_at(
        &path,
        &request_id,
        "response: accept; source_ref: self:wrong_actor",
        "astrid",
        PassageContextActionV1::RespondCompany,
    );
    assert!(owner_cannot_answer.contains("no matching inbound company request"));
    let accepted = append_context_action_at(
        &path,
        &request_id,
        "response: accept; source_ref: self:available",
        "minime",
        PassageContextActionV1::RespondCompany,
    );
    assert!(accepted.contains("response=accept"));
    let revised = append_context_action_at(
        &path,
        &request_id,
        "response: needs_time; source_ref: self:later",
        "minime",
        PassageContextActionV1::RespondCompany,
    );
    assert!(revised.contains("response=needs_time"));
    let withdrawn = append_context_action_at(
        &path,
        &request_id,
        "source_ref: self:withdraw",
        "astrid",
        PassageContextActionV1::WithdrawCompany,
    );
    assert!(withdrawn.contains("response=withdraw"));

    let status = status_report_at(&path, "astrid", 5);
    assert!(status.contains("mode=low_energy_presence"));
    assert!(status.contains("response=withdraw"));
    assert!(status.contains("silence is neutral"));
}

#[test]
fn peer_cannot_describe_or_checkpoint_another_beings_passage() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("phase_transitions_v1.jsonl");
    let passage_id = seed_passage(&path, "astrid");
    let condition = append_context_action_at(
        &path,
        &passage_id,
        "readiness: ready; movement_ease: open; room_needed: witness; source_ref: self:peer",
        "minime",
        PassageContextActionV1::DescribeCondition,
    );
    assert!(condition.contains("no matching self-authored passage"));
    let checkpoint = append_context_action_at(
        &path,
        &passage_id,
        "checkpoint: pivot; source_ref: self:peer",
        "minime",
        PassageContextActionV1::MarkCheckpoint,
    );
    assert!(checkpoint.contains("no matching self-authored passage"));
}

#[test]
fn continuity_anchor_is_self_owned_revisable_and_noncausal() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("phase_transitions_v1.jsonl");
    let passage_id = seed_passage(&path, "astrid");
    let first = append_context_action_at(
        &path,
        &passage_id,
        "role: pivot; kind: shadow_trajectory; association: temporal_context; anchor_ref: shadow-v3:astrid:1784951174; source_ref: introspection_proposal_phase_transitions_1784951174:c003",
        "astrid",
        PassageContextActionV1::BindAnchor,
    );
    assert!(first.contains("role=pivot"));
    assert!(first.contains("kind=shadow_trajectory"));
    let revised = append_context_action_at(
        &path,
        &passage_id,
        "role: pivot; kind: lived_state_witness; association: receipt_linked; anchor_ref: lsw_fixture; source_ref: introspection:later_review",
        "astrid",
        PassageContextActionV1::BindAnchor,
    );
    assert!(revised.contains("kind=lived_state_witness"));
    let peer = append_context_action_at(
        &path,
        &passage_id,
        "role: continuity; kind: correspondence; association: temporal_context; anchor_ref: thread:fixture; source_ref: self:peer",
        "minime",
        PassageContextActionV1::BindAnchor,
    );
    assert!(peer.contains("no matching self-authored passage"));

    let rows = phase_passages::read_records(&path)
        .into_iter()
        .filter(|row| row.get("action").and_then(Value::as_str) == Some("bind_anchor"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[1].get("previous_anchor_event_id"),
        rows[0].get("passage_context_event_id"),
    );
    assert!(rows.iter().all(|row| {
        row.get("passage_stage_changed").and_then(Value::as_bool) == Some(false)
            && row
                .get("anchor_mechanical_truth_inferred")
                .and_then(Value::as_bool)
                == Some(false)
            && row.get("anchor_changes_passage").and_then(Value::as_bool) == Some(false)
            && row.get("anchor_closes_transition").and_then(Value::as_bool) == Some(false)
            && row.get("felt_score_present").and_then(Value::as_bool) == Some(false)
            && row.get("raw_prose_included").and_then(Value::as_bool) == Some(false)
    }));
    let status = status_report_at(&path, "astrid", 5);
    assert!(status.contains("own continuity anchors: 1"));
    assert!(status.contains("kind=lived_state_witness"));
}

#[test]
fn strand_bearing_preserves_friction_without_metric_or_stage_flattening() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("phase_transitions_v1.jsonl");
    let passage_id = seed_passage(&path, "astrid");
    let first = append_context_action_at(
        &path,
        &passage_id,
        "strand: settling; movement_resistance: resistant; persistence_tendency: lingering; witness_fit: separate; source_ref: introspection_proposal_phase_transitions_1784978541:c002",
        "astrid",
        PassageContextActionV1::DescribeBearing,
    );
    assert!(first.contains("movement_resistance=resistant"));
    assert!(first.contains("witness_fit=separate"));
    let revised = append_context_action_at(
        &path,
        &passage_id,
        "strand: settling; movement_resistance: held_fast; persistence_tendency: carried; witness_fit: interwoven; source_ref: introspection_proposal_phase_transitions_1784978541:c003",
        "astrid",
        PassageContextActionV1::DescribeBearing,
    );
    assert!(revised.contains("movement_resistance=held_fast"));
    assert!(revised.contains("witness_fit=interwoven"));
    let independent = append_context_action_at(
        &path,
        &passage_id,
        "strand: entry_tension; movement_resistance: changing; persistence_tendency: deepening; witness_fit: touching; source_ref: introspection:entry_review",
        "astrid",
        PassageContextActionV1::DescribeBearing,
    );
    assert!(independent.contains("strand=entry_tension"));
    let numeric = append_context_action_at(
        &path,
        &passage_id,
        "strand: pivot; movement_resistance: 0.82; persistence_tendency: lingering; witness_fit: holding; source_ref: introspection:numeric",
        "astrid",
        PassageContextActionV1::DescribeBearing,
    );
    assert!(numeric.contains("unknown movement_resistance"));
    let peer = append_context_action_at(
        &path,
        &passage_id,
        "strand: pivot; movement_resistance: resistant; persistence_tendency: carried; witness_fit: holding; source_ref: self:peer",
        "minime",
        PassageContextActionV1::DescribeBearing,
    );
    assert!(peer.contains("no matching self-authored passage"));

    let rows = phase_passages::read_records(&path)
        .into_iter()
        .filter(|row| row.get("action").and_then(Value::as_str) == Some("describe_bearing"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[1].get("previous_bearing_event_id"),
        rows[0].get("passage_context_event_id"),
    );
    assert_eq!(rows[2].get("previous_bearing_event_id"), Some(&Value::Null));
    assert!(rows.iter().all(|row| {
        row.get("bearing_is_metric").and_then(Value::as_bool) == Some(false)
            && row
                .get("bearing_inferred_from_telemetry")
                .and_then(Value::as_bool)
                == Some(false)
            && row.get("bearing_changes_passage").and_then(Value::as_bool) == Some(false)
            && row
                .get("bearing_closes_transition")
                .and_then(Value::as_bool)
                == Some(false)
            && row.get("passage_stage_changed").and_then(Value::as_bool) == Some(false)
            && row.get("felt_score_present").and_then(Value::as_bool) == Some(false)
            && row.get("raw_prose_included").and_then(Value::as_bool) == Some(false)
    }));
    let status = status_report_at(&path, "astrid", 5);
    assert!(status.contains("own current strand bearings: 2"));
    assert!(status.contains("witness_fit=interwoven"));
    assert!(status.contains("not a viscosity metric"));
}

#[test]
fn deterministic_identity_matches_python_and_minime_fixture() {
    assert_eq!(
        context_event_id(
            "passage_fixture",
            "transition_fixture",
            "astrid",
            "astrid",
            PassageContextActionV1::DescribeCondition,
            Some(PassageReadinessV1::Tentative),
            Some(PassageMovementEaseV1::Effortful),
            Some(PassageRoomNeededV1::LowEnergyPresence),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "fixture:condition",
            None,
            None,
            1_700_000_000_000,
        ),
        "passage_context_61ccca814e93f37e"
    );
    assert_eq!(
        context_event_id(
            "passage_fixture",
            "transition_fixture",
            "astrid",
            "astrid",
            PassageContextActionV1::BindAnchor,
            None,
            None,
            None,
            None,
            Some(PassageAnchorRoleV1::Pivot),
            Some(PassageAnchorKindV1::ShadowTrajectory),
            Some(PassageAnchorAssociationV1::TemporalContext),
            Some("shadow-v3:astrid:1784951174"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "introspection_proposal_phase_transitions_1784951174:c003",
            None,
            None,
            1_700_000_000_002,
        ),
        "passage_context_2fe152ff35ebcdbe"
    );
    assert_eq!(
        context_event_id(
            "passage_fixture",
            "transition_fixture",
            "astrid",
            "astrid",
            PassageContextActionV1::DescribeBearing,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(PassageBearingStrandV1::Settling),
            Some(PassageMovementResistanceV1::Resistant),
            Some(PassagePersistenceTendencyV1::Lingering),
            Some(PassageWitnessFitV1::Separate),
            None,
            None,
            None,
            None,
            None,
            "introspection_proposal_phase_transitions_1784978541:c002",
            None,
            None,
            1_700_000_000_004,
        ),
        "passage_context_c20adcff8b0eba28"
    );
    assert_eq!(
        company_request_id(
            "passage_fixture",
            "astrid",
            "minime",
            PassageCompanyModeV1::LowEnergyPresence,
            "fixture:request",
            1_700_000_000_001,
        ),
        "company_request_1700000000001_5841612e9caa7aad"
    );
}
