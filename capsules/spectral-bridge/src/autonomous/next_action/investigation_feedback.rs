//! Preserve failed investigation outcomes at the next choice, without retrying
//! them, granting authority, or replacing an authored direction.
use super::{ConversationState, NextActionOutcome, split_multi_action};
use crate::runtime_action_feedback::RuntimeActionFeedbackV1;

pub(super) fn context(action: &str) -> Option<&'static str> {
    // SEARCH treats its query as opaque, so the generic splitter alone cannot
    // distinguish an AND query from attempted chaining. Leave those ambiguous
    // forms on their existing feedback path; never copy a possible draft title.
    if split_multi_action(action).len() != 1
        || action
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case("AND"))
    {
        return None;
    }
    match action.split_whitespace().next()? {
        "SEARCH" | "BROWSE" => Some(
            "This attempt did not execute or produce new research evidence. SEARCH and BROWSE are external research routes requiring an exact authority grant. CAPABILITY_STATUS SEARCH describes the route; it does not grant permission. For local repository code, SELF_STUDY MAP, SELF_STUDY FIND <literal text>, or SELF_STUDY OPEN <exact repository/path> [line] use the source reader and its checks. No alternative has been selected or queued.",
        ),
        "EXAMINE" => Some(
            "This attempt did not execute or produce new research evidence. EXAMINE inspects spectral state; it does not read code methods. EXPERIMENT_RESEARCH_BUDGET_STATUS inspects the current research budget without accepting it. For local repository code, SELF_STUDY MAP, SELF_STUDY FIND <literal text>, or SELF_STUDY OPEN <exact repository/path> [line] use the source reader and its checks. No alternative has been selected or queued.",
        ),
        _ => None,
    }
}

pub(in crate::autonomous) fn blocked_authority(
    conv: &mut ConversationState,
    original: &str,
    reason: String,
    action_identity: &str,
) -> NextActionOutcome {
    let mut outcome = NextActionOutcome::blocked("volition_authority", reason);
    if let Some(context) = context(original) {
        outcome.outcome_summary.push('\n');
        outcome.outcome_summary.push_str(context);
        conv.enqueue_runtime_feedback(RuntimeActionFeedbackV1::from_guard_inputs(
            Some(action_identity),
            original,
            "volition_authority",
            &outcome.outcome_summary,
            None,
        ));
    }
    super::study_navigation::with_recovery(conv, original, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_search_is_retained_once_without_changing_choice_or_authority() {
        let mut conv = ConversationState::new(Vec::new(), None);
        conv.emphasis = Some("Follow the chosen melody".into());
        let action = "SEARCH architecture_overview";
        for _ in 0..2 {
            let outcome = blocked_authority(&mut conv, action, "No exact grant".into(), "turn-1");
            assert!(!outcome.handled);
            assert_eq!(outcome.route, "volition_authority");
            assert_eq!(outcome.status, "blocked");
            assert!(outcome.suggested_next.is_none());
        }
        assert_eq!(conv.pending_runtime_feedback.len(), 1);
        let feedback = &conv.pending_runtime_feedback[0];
        assert_eq!(feedback.requested_action, action);
        assert!(feedback.message.contains("No exact grant"));
        assert!(feedback.message.contains("did not execute"));
        assert!(feedback.message.contains("SELF_STUDY FIND"));
        assert_eq!(conv.emphasis.as_deref(), Some("Follow the chosen melody"));
        assert!(!conv.wants_search);
        assert!(conv.introspect_target.is_none());
        blocked_authority(&mut conv, action, "No exact grant".into(), "turn-2");
        assert_eq!(conv.pending_runtime_feedback.len(), 2);
    }

    #[test]
    fn guidance_distinguishes_spectral_inspection_and_preserves_private_boundaries() {
        assert!(
            context("EXAMINE Kernel methods")
                .unwrap()
                .contains("does not read code")
        );
        for action in [
            "WRITE START secret",
            "SELF_STUDY WRITE START secret",
            "SEARCH code AND WRITE START secret",
            "REST",
        ] {
            assert!(context(action).is_none(), "{action}");
        }
    }
}
