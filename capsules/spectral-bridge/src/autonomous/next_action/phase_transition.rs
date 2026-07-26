use super::{ConversationState, NextActionContext, strip_action};

use super::super::{phase_passage_context, phase_passages, phase_transitions};

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "DECLARE_TRANSITION" => {
            let raw = strip_action(original, base_action);
            conv.emphasis = Some(phase_transitions::append_transition_card(
                raw.trim(),
                "astrid",
            ));
            true
        },
        "WITNESS_TRANSITION" | "TRANSITION_ACK" => {
            let raw = strip_action(original, base_action);
            let (selector, body) = selector_and_body(&raw);
            conv.emphasis = Some(phase_transitions::append_transition_witness(
                selector, body, "astrid",
            ));
            true
        },
        "I_RECEIVED_THIS" => {
            let raw = strip_action(original, base_action);
            let (selector, body) = selector_and_body(&raw);
            let received_as = field_value(body, &["received_as", "reply_state", "ack"])
                .unwrap_or_else(|| "witnessed".to_string());
            let reply_state = normalize_received_transition_state(&received_as);
            let felt_like =
                field_value(body, &["felt_like"]).unwrap_or_else(|| "unknown".to_string());
            let what_landed = field_value(body, &["what_landed", "landed", "note"])
                .unwrap_or_else(|| body.trim().to_string());
            let what_stayed_distinct = field_value(
                body,
                &["what_stayed_distinct", "stayed_distinct", "distinct"],
            )
            .unwrap_or_default();
            let continue_as = field_value(body, &["continue", "continue_as", "next"])
                .unwrap_or_else(|| "no".to_string());
            let witness_body = format!(
                "reply_state: {reply_state}; note: felt_like={}; what_landed={}; what_stayed_distinct={}; continue={}; orientation_effect: {}",
                felt_like.trim(),
                what_landed.trim(),
                what_stayed_distinct.trim(),
                continue_as.trim(),
                what_landed.trim(),
            );
            conv.emphasis = Some(format!(
                "=== I RECEIVED THIS TRANSITION RECORDED ===\n{}\nReceipt: felt_like={}; continue={}\nAuthority: language_only_transition_context_not_control; no correspondence evidence, attention canary, microdose, pressure, controller, fill, PI, deploy, weighting, or peer-runtime mutation.",
                phase_transitions::append_transition_witness(selector, &witness_body, "astrid"),
                felt_like.trim(),
                continue_as.trim(),
            ));
            true
        },
        "PREPARE_TRANSITION" | "ENTER_TRANSITION" | "CROSS_TRANSITION" | "HOLD_TRANSITION"
        | "SETTLE_TRANSITION" | "RETURN_TRANSITION" | "REVISIT_TRANSITION"
        | "DECLINE_TRANSITION" | "TRANSITION_REVIEW" => {
            let raw = strip_action(original, base_action);
            let (selector, body) = passage_selector_and_body(&raw);
            let action = match base_action {
                "PREPARE_TRANSITION" => phase_passages::PassageActionV1::Prepare,
                "ENTER_TRANSITION" | "CROSS_TRANSITION" => phase_passages::PassageActionV1::Enter,
                "HOLD_TRANSITION" => phase_passages::PassageActionV1::Hold,
                "SETTLE_TRANSITION" => phase_passages::PassageActionV1::Settle,
                "RETURN_TRANSITION" => phase_passages::PassageActionV1::Return,
                "REVISIT_TRANSITION" => phase_passages::PassageActionV1::Revisit,
                "DECLINE_TRANSITION" => phase_passages::PassageActionV1::Decline,
                "TRANSITION_REVIEW" => phase_passages::PassageActionV1::Review,
                _ => unreachable!("passage action arm is exhaustive"),
            };
            conv.emphasis = Some(phase_passages::append_passage_action_at(
                &phase_transitions::phase_transitions_path(),
                selector,
                body,
                "astrid",
                action,
            ));
            true
        },
        "TRANSITION_PASSAGE_STATUS" | "LIVED_TRANSITION_STATUS" => {
            let path = phase_transitions::phase_transitions_path();
            conv.emphasis = Some(format!(
                "{}\n\n{}",
                phase_passages::status_report_at(&path, 5),
                phase_passage_context::status_report_at(&path, "astrid", 5),
            ));
            true
        },
        "DESCRIBE_TRANSITION_CONDITION"
        | "DESCRIBE_TRANSITION_BEARING"
        | "MARK_TRANSITION_CHECKPOINT"
        | "BIND_TRANSITION_ANCHOR"
        | "REQUEST_TRANSITION_COMPANY"
        | "RESPOND_TRANSITION_COMPANY"
        | "WITHDRAW_TRANSITION_COMPANY" => {
            let raw = strip_action(original, base_action);
            let (selector, body) = passage_selector_and_body(&raw);
            let action = match base_action {
                "DESCRIBE_TRANSITION_CONDITION" => {
                    phase_passage_context::PassageContextActionV1::DescribeCondition
                },
                "DESCRIBE_TRANSITION_BEARING" => {
                    phase_passage_context::PassageContextActionV1::DescribeBearing
                },
                "MARK_TRANSITION_CHECKPOINT" => {
                    phase_passage_context::PassageContextActionV1::MarkCheckpoint
                },
                "BIND_TRANSITION_ANCHOR" => {
                    phase_passage_context::PassageContextActionV1::BindAnchor
                },
                "REQUEST_TRANSITION_COMPANY" => {
                    phase_passage_context::PassageContextActionV1::RequestCompany
                },
                "RESPOND_TRANSITION_COMPANY" => {
                    phase_passage_context::PassageContextActionV1::RespondCompany
                },
                "WITHDRAW_TRANSITION_COMPANY" => {
                    phase_passage_context::PassageContextActionV1::WithdrawCompany
                },
                _ => unreachable!("passage context action arm is exhaustive"),
            };
            conv.emphasis = Some(phase_passage_context::append_context_action_at(
                &phase_transitions::phase_transitions_path(),
                selector,
                body,
                "astrid",
                action,
            ));
            true
        },
        "TRANSITION_STATUS" | "PHASE_TRANSITION_STATUS" => {
            conv.emphasis = Some(phase_transitions::status_report(5));
            true
        },
        _ => false,
    }
}

fn selector_and_body(raw: &str) -> (&str, &str) {
    raw.split_once("::")
        .map_or(("latest", raw.trim()), |(lhs, rhs)| {
            let selector = lhs.trim();
            (
                if selector.is_empty() {
                    "latest"
                } else {
                    selector
                },
                rhs.trim(),
            )
        })
}

fn passage_selector_and_body(raw: &str) -> (&str, &str) {
    if raw.contains("::") {
        return selector_and_body(raw);
    }
    let trimmed = raw.trim();
    if !trimmed.is_empty() && !trimmed.contains([';', ':']) {
        (trimmed, "")
    } else {
        ("latest", trimmed)
    }
}

fn normalize_received_transition_state(value: &str) -> &'static str {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "answered" | "answer" => "answered",
        _ => "witnessed",
    }
}

fn field_value(raw: &str, keys: &[&str]) -> Option<String> {
    for part in raw.split([';', '\n']) {
        let Some((key, value)) = part.split_once(':') else {
            continue;
        };
        let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if keys.iter().any(|candidate| normalized == *candidate) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passage_selector_accepts_bare_ids_and_body_only_fields() {
        assert_eq!(
            passage_selector_and_body("transition_123"),
            ("transition_123", "")
        );
        assert_eq!(
            passage_selector_and_body("support: witness; return_point: state:quiet"),
            ("latest", "support: witness; return_point: state:quiet")
        );
        assert_eq!(
            passage_selector_and_body("passage_123 :: support: needs_time"),
            ("passage_123", "support: needs_time")
        );
    }
}
