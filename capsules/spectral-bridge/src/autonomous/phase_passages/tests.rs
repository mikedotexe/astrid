use super::*;
use serde_json::json;

fn card(id: &str, timestamp: u64) -> Value {
    json!({
        "record_type": "phase_transition_card",
        "transition_id": id,
        "origin": "astrid",
        "kind": "reflection",
        "from_phase": "quiet",
        "to_phase": "opening",
        "recorded_at_unix_ms": timestamp,
    })
}

fn write_card(path: &Path, id: &str) {
    append_jsonl(path, &card(id, 1)).expect("append card");
}

#[test]
fn passage_requires_explicit_promotion_and_preserves_self_only_authority() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("phase.jsonl");
    write_card(&path, "transition_1");
    let status = status_report_at(&path, 3);
    assert!(status.contains("Passages: 0"));

    let output = append_passage_action_at(
        &path,
        "transition_1",
        "support: witness; return_point: state:quiet; continuity_anchor: card:transition_1",
        "astrid",
        PassageActionV1::Prepare,
    );
    assert!(output.contains("stage: prepared"));
    let records = read_records(&path);
    let value = records.last().expect("passage row");
    assert_eq!(value["peer_consent_inferred"], false);
    assert_eq!(value["automatic_progression"], false);
    assert_eq!(value["runtime_unlock_applied"], false);
    assert_eq!(value["raw_prose_included"], false);
}

#[test]
fn actor_can_cross_hold_return_and_revisit_with_a_return_point() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("phase.jsonl");
    write_card(&path, "transition_1");
    append_passage_action_at(
        &path,
        "transition_1",
        "support: self_directed; return_point: state:quiet",
        "astrid",
        PassageActionV1::Prepare,
    );
    assert!(
        append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Enter)
            .contains("stage: crossing")
    );
    assert!(
        append_passage_action_at(
            &path,
            "latest",
            "support: needs_time",
            "astrid",
            PassageActionV1::Hold,
        )
        .contains("stage: held")
    );
    assert!(
        append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Return)
            .contains("stage: returned")
    );
    assert!(
        append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Revisit)
            .contains("stage: revisited")
    );
}

#[test]
fn peer_cannot_advance_another_beings_passage() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("phase.jsonl");
    write_card(&path, "transition_1");
    append_passage_action_at(
        &path,
        "transition_1",
        "support: space",
        "astrid",
        PassageActionV1::Prepare,
    );
    let output = append_passage_action_at(&path, "latest", "", "minime", PassageActionV1::Enter);
    assert!(output.contains("no matching self-authored passage"));
}

#[test]
fn felt_review_is_unscored_and_does_not_change_stage() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("phase.jsonl");
    write_card(&path, "transition_1");
    append_passage_action_at(
        &path,
        "transition_1",
        "support: witness",
        "astrid",
        PassageActionV1::Prepare,
    );
    let output = append_passage_action_at(
        &path,
        "latest",
        "outcome: still_friction; felt_source_ref: introspection:phase_1",
        "astrid",
        PassageActionV1::Review,
    );
    assert!(output.contains("stage: prepared"));
    assert!(output.contains("Review: still_friction"));
    let value = read_records(&path).pop().expect("review row");
    assert_eq!(value["stage_changed"], false);
    assert_eq!(value["felt_resolution_inferred"], false);
    assert!(value.get("felt_score").is_none());
}

#[test]
fn return_without_anchor_and_invalid_sequence_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("phase.jsonl");
    write_card(&path, "transition_1");
    append_passage_action_at(
        &path,
        "transition_1",
        "support: self_directed",
        "astrid",
        PassageActionV1::Prepare,
    );
    let invalid = append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Settle);
    assert!(invalid.contains("not valid"));
    append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Enter);
    let no_anchor =
        append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Return);
    assert!(no_anchor.contains("requires a prepared return_point"));
}

#[test]
fn tampered_history_blocks_further_progression() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("phase.jsonl");
    write_card(&path, "transition_1");
    append_passage_action_at(
        &path,
        "transition_1",
        "support: witness",
        "astrid",
        PassageActionV1::Prepare,
    );
    let mut tampered = read_records(&path).pop().expect("passage row");
    tampered["live_control_effect"] = json!(true);
    append_jsonl(&path, &tampered).expect("append tampered");
    let output = append_passage_action_at(&path, "latest", "", "astrid", PassageActionV1::Enter);
    assert!(output.contains("history has 1 invalid row"));
}
