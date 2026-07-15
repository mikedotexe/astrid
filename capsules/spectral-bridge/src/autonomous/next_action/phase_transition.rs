use super::{ConversationState, NextActionContext, strip_action};

use super::super::phase_transitions;

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
            let (selector, body) =
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
                    });
            conv.emphasis = Some(phase_transitions::append_transition_witness(
                selector, body, "astrid",
            ));
            true
        },
        "I_RECEIVED_THIS" => {
            let raw = strip_action(original, base_action);
            let (selector, body) =
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
                    });
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
        "TRANSITION_STATUS" | "PHASE_TRANSITION_STATUS" => {
            conv.emphasis = Some(phase_transitions::status_report(5));
            true
        },
        _ => false,
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
