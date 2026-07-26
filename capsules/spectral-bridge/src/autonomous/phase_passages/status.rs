use super::*;

fn next_actions(stage: PassageStageV1) -> &'static str {
    match stage {
        PassageStageV1::Prepared => {
            "ENTER_TRANSITION|HOLD_TRANSITION|DECLINE_TRANSITION|TRANSITION_REVIEW"
        },
        PassageStageV1::Crossing => {
            "SETTLE_TRANSITION|HOLD_TRANSITION|RETURN_TRANSITION|TRANSITION_REVIEW"
        },
        PassageStageV1::Held => {
            "ENTER_TRANSITION|SETTLE_TRANSITION|RETURN_TRANSITION|DECLINE_TRANSITION|REVISIT_TRANSITION|TRANSITION_REVIEW"
        },
        PassageStageV1::Settling => {
            "HOLD_TRANSITION|RETURN_TRANSITION|REVISIT_TRANSITION|TRANSITION_REVIEW"
        },
        PassageStageV1::Returned | PassageStageV1::Declined => {
            "REVISIT_TRANSITION|TRANSITION_REVIEW"
        },
        PassageStageV1::Revisited => {
            "ENTER_TRANSITION|HOLD_TRANSITION|RETURN_TRANSITION|DECLINE_TRANSITION|TRANSITION_REVIEW"
        },
    }
}

pub(super) fn report_at(path: &Path, max_passages: usize) -> String {
    let records = read_records(path);
    let (passages, errors) = reduce_passages(&records);
    let active = passages
        .values()
        .filter(|passage| {
            !matches!(
                passage.stage,
                PassageStageV1::Returned | PassageStageV1::Declined
            )
        })
        .count();
    let reviewed = passages
        .values()
        .filter(|passage| passage.felt_review_outcome.is_some())
        .count();
    let mut lines = vec![
        "=== LIVED TRANSITION PASSAGES V1 ===".to_string(),
        format!(
            "Passages: {}; active: {active}; optional felt reviews recorded: {reviewed}; invalid rows: {}.",
            passages.len(),
            errors.len()
        ),
        "Selection boundary: detected phase cards remain observations until a being explicitly uses PREPARE_TRANSITION; no automatic promotion or task debt.".to_string(),
        "Agency boundary: each passage binds its actor only; peers may witness but cannot advance, settle, decline, return, or review it.".to_string(),
        "Authority: language-only transition practice; no automatic progression, felt closure, scheduler, model, substrate, dispatch, controller, pressure, fill, PI, codec, or runtime effect.".to_string(),
    ];
    let mut latest = passages.values().collect::<Vec<_>>();
    latest.sort_by_key(|passage| std::cmp::Reverse(passage.recorded_at_unix_ms));
    for passage in latest.into_iter().take(max_passages) {
        lines.push(format!(
            "- {}: transition={}; actor={}; stage={}; support={}; return_point={}; continuity_anchor={}; felt_review={}; next={}; review_optional=true",
            passage.passage_id,
            passage.transition_id,
            passage.actor,
            passage.stage.as_str(),
            passage.support_preference.as_str(),
            passage.return_point_ref.as_deref().unwrap_or("none"),
            passage.continuity_anchor_ref,
            passage
                .felt_review_outcome
                .map_or("not_recorded", PassageFeltReviewV1::as_str),
            next_actions(passage.stage),
        ));
    }
    lines.push(
        "Suggested start: PREPARE_TRANSITION <transition_id> :: support: self_directed|witness|space|answer|needs_time; return_point: <bounded_ref>; continuity_anchor: <bounded_ref>"
            .to_string(),
    );
    lines.join("\n")
}
