//! Preserve unconsumed study choices and offer optional syntax recovery.
use super::{ConversationState, NextActionOutcome, strip_action};
use crate::autonomous::state::{IntrospectOffsetV2, IntrospectTargetV2};

fn request_target(base: &str, original: &str) -> Option<Option<IntrospectTargetV2>> {
    let argument = strip_action(original, base);
    let target = match base {
        "WRITE" => Some(IntrospectTargetV2::auto(
            format!("WRITE {argument}").trim().into(),
        )),
        "SELF_STUDY" | "INVESTIGATE" => Some(IntrospectTargetV2::auto(
            format!("SELF_STUDY {argument}").trim().to_string(),
        )),
        "INTROSPECT" if !argument.is_empty() => {
            let mut parts = argument.split_whitespace().collect::<Vec<_>>();
            let offset = parts
                .last()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|_| parts.len() > 1);
            if offset.is_some() {
                parts.pop();
            }
            let label = parts.join(" ");
            Some(match offset {
                Some(offset) => IntrospectTargetV2::exact(label, offset),
                None => IntrospectTargetV2::auto(label),
            })
        },
        "EXAMINE_CODE" => {
            let label = argument.trim_matches(['[', ']']).trim();
            (!label.is_empty()).then(|| IntrospectTargetV2::auto(label.into()))
        },
        "INTROSPECT" => None,
        _ => return None,
    };
    Some(target)
}

pub(in crate::autonomous) fn private(target: &IntrospectTargetV2) -> bool {
    // Classify before syntax validation: malformed source prefixes may still
    // carry a private title. Use the same rule for labels and receipt actions.
    target
        .label
        .split_whitespace()
        .map(|word| word.trim_end_matches(':'))
        .find(|word| {
            !matches!(
                word.to_ascii_uppercase().as_str(),
                "SELF_STUDY" | "INVESTIGATE" | "REPLACE"
            )
        })
        .is_some_and(|verb| verb.eq_ignore_ascii_case("WRITE"))
}

fn visible_target(target: Option<&IntrospectTargetV2>) -> String {
    match target {
        Some(target) if private(target) => {
            "your private writing request (contents withheld)".into()
        },
        Some(target) => match target.offset {
            IntrospectOffsetV2::Auto => target.label.clone(),
            IntrospectOffsetV2::Exact(offset) => {
                format!("{} at exact offset {offset}", target.label)
            },
        },
        None => "the next source-study page".into(),
    }
}

fn retain(conv: &mut ConversationState, original: &str, outcome: &NextActionOutcome) {
    // Private draft titles and arguments must never enter a public recovery receipt.
    let base = original
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(':')
        .to_ascii_uppercase();
    let private_request = request_target(&base, original)
        .flatten()
        .is_some_and(|target| private(&target));
    let action = if private_request { "WRITE" } else { original };
    let mut feedback = crate::runtime_action_feedback::RuntimeActionFeedbackV1::from_guard_inputs(
        None,
        action,
        "source_study_request",
        &outcome.outcome_summary,
        outcome.suggested_next.as_deref(),
    );
    feedback.status = outcome.status.clone();
    conv.enqueue_runtime_feedback(feedback);
    conv.emphasis = Some(outcome.outcome_summary.clone());
}

fn valid_replacement(operation: &str) -> bool {
    // Parse the full modifier: parsing the inner command alone permits the
    // legacy unknown-verb-to-source alias fallback and bypasses this whitelist.
    astrid_source_study::Command::parse(&format!("SELF_STUDY REPLACE {operation}")).is_ok()
        && astrid_source_study::recover_local_navigation(&format!("SELF_STUDY {operation}"))
            .is_none()
}

pub(super) fn handle_request(
    conv: &mut ConversationState,
    base: &str,
    original: &str,
) -> Option<NextActionOutcome> {
    let mut requested = request_target(base, original)?;
    let argument = strip_action(original, base);
    let replace = matches!(base, "SELF_STUDY" | "INVESTIGATE")
        && argument
            .split_whitespace()
            .next()
            .is_some_and(|verb| verb.eq_ignore_ascii_case("REPLACE"));
    if replace {
        let inner = argument
            .split_once(char::is_whitespace)
            .map_or("", |(_, rest)| rest.trim());
        let command = format!("SELF_STUDY {inner}");
        // REPLACE can only change a pending local source operation, never become
        // a private WRITE or another action/authority escape.
        if !valid_replacement(inner) {
            let outcome = NextActionOutcome::blocked("source_study_request",
                "Replacement was not queued. Use SELF_STUDY REPLACE followed by one valid source-study operation, such as MAP or RELATE EventBus. The pending choice is unchanged.")
                .with_stage_visibility("read_only", "protected_summary");
            retain(conv, original, &outcome);
            return Some(outcome);
        }
        requested = Some(IntrospectTargetV2::auto(command));
    }
    // A restored choice can outlive the ephemeral mode-selection flag.
    if conv.introspect_target.is_some() {
        conv.wants_introspect = true;
    }
    if conv.wants_introspect {
        let pending = conv.introspect_target.as_ref();
        if pending == requested.as_ref() {
            let outcome = NextActionOutcome::handled("source_study_request",
                format!("Already pending: {}. This identical retry shares that request; source has not yet been delivered.", visible_target(pending)))
                .with_stage_visibility("read_only", "protected_summary");
            retain(conv, original, &outcome);
            return Some(outcome);
        }
        if !replace {
            let mut message = format!(
                "Pending choice preserved: {}. The later request for {} was not queued. You can choose it again after the pending reading, or deliberately replace the pending source operation with SELF_STUDY REPLACE <operation>.",
                visible_target(pending),
                visible_target(requested.as_ref()),
            );
            let replacement = requested
                .as_ref()
                .filter(|t| t.label.starts_with("SELF_STUDY "))
                .filter(|t| valid_replacement(t.label.trim_start_matches("SELF_STUDY ")))
                .map(|t| {
                    format!(
                        "SELF_STUDY REPLACE {}",
                        t.label.trim_start_matches("SELF_STUDY ")
                    )
                });
            if let Some(command) = &replacement {
                message.push_str(&format!(" Exact replacement choice: {command}"));
            }
            let mut outcome = NextActionOutcome::blocked("source_study_request", message)
                .with_stage_visibility("read_only", "protected_summary");
            outcome.suggested_next = replacement;
            retain(conv, original, &outcome);
            return Some(outcome);
        }
    }
    let prior = conv
        .introspect_target
        .as_ref()
        .map(|target| visible_target(Some(target)));
    conv.introspect_target = requested;
    conv.wants_introspect = true;
    conv.defer_inbox = true;
    let mut message = format!(
        "Queued: {}. Acceptance queues this choice; verified source delivery is recorded when the reading completes.",
        visible_target(conv.introspect_target.as_ref())
    );
    if replace && let Some(prior) = prior {
        message.push_str(&format!(" Explicitly superseded: {prior}."));
    }
    let outcome = NextActionOutcome::handled("source_study_request", message)
        .with_stage_visibility("read_only", "protected_summary");
    retain(conv, original, &outcome);
    Some(outcome)
}

/// Attach shared recovery to the original outcome without changing its action,
/// authorization, or dispatch status. A suggestion is never automatically run.
pub(in crate::autonomous) fn with_recovery(
    conv: &mut ConversationState,
    original: &str,
    mut outcome: NextActionOutcome,
) -> NextActionOutcome {
    if let Some(recovery) = astrid_source_study::recover_local_navigation(original) {
        outcome.outcome_summary.push_str("\n\n");
        outcome.outcome_summary.push_str(&recovery.text);
        outcome.suggested_next = recovery.commands.first().cloned();
        retain(conv, original, &outcome);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_default_query_cannot_replace_chosen_page_but_explicit_replacement_can() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let chosen = "SELF_STUDY RELATE EventBus --page 2";
        assert!(
            handle_request(&mut conv, "SELF_STUDY", chosen)
                .unwrap()
                .handled
        );
        let original = conv.introspect_target.clone();
        let later = handle_request(&mut conv, "SELF_STUDY", "SELF_STUDY RELATE EventBus").unwrap();
        assert!(!later.handled);
        assert_eq!(conv.introspect_target, original);
        assert_eq!(
            later.suggested_next.as_deref(),
            Some("SELF_STUDY REPLACE RELATE EventBus")
        );
        let retry = handle_request(&mut conv, "SELF_STUDY", chosen).unwrap();
        assert!(retry.handled);
        assert!(retry.outcome_summary.contains("identical retry"));
        let replace = handle_request(
            &mut conv,
            "SELF_STUDY",
            "SELF_STUDY REPLACE RELATE EventBus",
        )
        .unwrap();
        assert!(replace.handled);
        assert!(replace.outcome_summary.contains("superseded"));
        assert_eq!(
            conv.introspect_target.unwrap().label,
            "SELF_STUDY RELATE EventBus"
        );
    }

    #[test]
    fn legacy_and_private_choices_survive_collisions_without_exposing_private_arguments() {
        let mut conv = ConversationState::new(Vec::new(), None);
        handle_request(&mut conv, "WRITE", "WRITE START confidential draft title").unwrap();
        let original = conv.introspect_target.clone();
        let blocked = handle_request(&mut conv, "INTROSPECT", "INTROSPECT").unwrap();
        assert!(!blocked.handled);
        assert_eq!(conv.introspect_target, original);
        assert!(!blocked.outcome_summary.contains("confidential"));
        let invalid = handle_request(
            &mut conv,
            "SELF_STUDY",
            "SELF_STUDY REPLACE WRITE START hidden",
        )
        .unwrap();
        assert!(!invalid.handled);
        assert_eq!(conv.introspect_target, original);
        for (base, request) in [
            ("WRITE", "write START confidential"),
            ("WRITE", "WRITE: START confidential"),
            ("SELF_STUDY", "SELF_STUDY WRITE START confidential"),
            ("INVESTIGATE", "INVESTIGATE WRITE START confidential"),
            ("INTROSPECT", "INTROSPECT write START confidential"),
            ("INTROSPECT", "INTROSPECT WRITE START confidential"),
            ("EXAMINE_CODE", "EXAMINE_CODE [WRITE START confidential]"),
            ("SELF_STUDY", "SELF_STUDY REPLACE WRITE START confidential"),
            ("SELF_STUDY", "SELF_STUDY replace write START confidential"),
            (
                "SELF_STUDY",
                "SELF_STUDY REPLACE REPLACE WRITE START confidential",
            ),
        ] {
            handle_request(&mut conv, base, request).unwrap();
        }
        let public_feedback = serde_json::to_string(&conv.pending_runtime_feedback).unwrap();
        assert!(!public_feedback.contains("confidential"));
        assert!(!public_feedback.contains("hidden"));
    }

    #[test]
    fn untargeted_continuation_remains_a_choice_until_consumed() {
        let mut conv = ConversationState::new(Vec::new(), None);
        assert!(
            handle_request(&mut conv, "INTROSPECT", "INTROSPECT")
                .unwrap()
                .handled
        );
        assert!(
            handle_request(&mut conv, "INTROSPECT", "INTROSPECT")
                .unwrap()
                .handled
        );
        assert!(
            !handle_request(&mut conv, "SELF_STUDY", "SELF_STUDY MAP")
                .unwrap()
                .handled
        );
        assert!(conv.wants_introspect);
        assert!(conv.introspect_target.is_none());
    }

    #[test]
    fn invalid_replacement_does_not_bypass_shared_operation_validation() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let chosen = "SELF_STUDY RELATE EventBus --page 2";
        handle_request(&mut conv, "SELF_STUDY", chosen).unwrap();
        let pending = conv.introspect_target.clone();
        for operation in [
            "BOGUS",
            "REPLACE MAP",
            "WRITE START hidden",
            "RELATE",
            "RELATE dispatch.rs Route Stage",
            "OPEN astrid/crates/astrid-kernel/src/lib.rs 0",
        ] {
            let outcome = handle_request(
                &mut conv,
                "SELF_STUDY",
                &format!("SELF_STUDY REPLACE {operation}"),
            )
            .unwrap();
            assert!(!outcome.handled, "{operation}");
            assert_eq!(outcome.status, "blocked");
            assert_eq!(conv.introspect_target, pending);
            assert!(conv.wants_introspect);
            assert!(outcome.suggested_next.is_none());
        }
        for operation in ["BOGUS", "RELATE dispatch.rs Route Stage"] {
            let outcome =
                handle_request(&mut conv, "SELF_STUDY", &format!("SELF_STUDY {operation}"))
                    .unwrap();
            assert!(!outcome.handled);
            assert!(outcome.suggested_next.is_none());
            assert!(
                !outcome
                    .outcome_summary
                    .contains("Exact replacement choice:")
            );
            assert_eq!(conv.introspect_target, pending);
        }
        assert!(valid_replacement("RELATE EventBus --page 2"));
        assert!(valid_replacement(
            "OPEN astrid/crates/astrid-kernel/src/lib.rs 1"
        ));
        assert!(valid_replacement("MAP"));
    }

    #[test]
    fn recovery_preserves_authority_block_and_does_not_queue_or_search() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let before = conv.introspect_target.clone();
        let outcome = with_recovery(
            &mut conv,
            "SEARCH dispatch.rs",
            NextActionOutcome::blocked("volition_authority", "No exact grant."),
        );
        assert_eq!(outcome.route, "volition_authority");
        assert_eq!(outcome.status, "blocked");
        assert_eq!(
            outcome.suggested_next.as_deref(),
            Some("SELF_STUDY FIND dispatch.rs")
        );
        assert_eq!(conv.introspect_target, before);
        assert!(!conv.wants_search);
        assert!(!conv.wants_introspect);
    }
}
