//! Context-bound writing shorthand; authored response text remains unchanged.

/// Only the verified private-writing producer emits this mode. Preparation,
/// incomplete generation, reader finalization, and artifact failures emit notices.
/// The shared receipt describes normalization, not successful action dispatch.
pub(crate) fn normalized_private_writing_next(mode: &str, text: &str) -> Option<&'static str> {
    if mode != "private_writing"
        || !super::parse_next_action(text)
            .is_some_and(|action| action.eq_ignore_ascii_case("CONTINUE"))
    {
        return None;
    }
    let feedback = astrid_source_study::response_choice::inspect_response(text, true);
    (feedback
        .selected_next
        .as_deref()
        .is_some_and(|action| action.eq_ignore_ascii_case("CONTINUE"))
        && feedback.normalized_next.as_deref() == Some("WRITE CONTINUE"))
    .then_some("WRITE CONTINUE")
}

/// Study shorthand (2026-09-23): a bare `CONTINUE` chosen from a self-study
/// turn means the study bookmark, exactly as it means the private draft in a
/// private-writing turn. It was rejected as unwired 29 times in 14 days while
/// the recovery note kept telling her the long form. Normalization only; her
/// authored text is unchanged and a reference notice records the mapping.
pub(crate) fn normalized_study_continue_next(mode: &str, text: &str) -> Option<&'static str> {
    if mode != "self_study" {
        return None;
    }
    super::parse_next_action(text)
        .is_some_and(|action| action.eq_ignore_ascii_case("CONTINUE"))
        .then_some("SELF_STUDY CONTINUE")
}

#[cfg(test)]
mod tests {
    use super::{normalized_private_writing_next, normalized_study_continue_next};

    #[test]
    fn bare_continue_in_a_study_turn_means_the_study_bookmark() {
        for text in ["NEXT: CONTINUE", "The page ends mid-item.\nNEXT: continue"] {
            assert_eq!(
                normalized_study_continue_next("self_study", text),
                Some("SELF_STUDY CONTINUE")
            );
            for mode in [
                "private_writing",
                "dialogue_live",
                "self_study_carriage_notice",
                "write",
            ] {
                assert_eq!(normalized_study_continue_next(mode, text), None, "{mode}");
            }
        }
        for text in [
            "NEXT: SELF_STUDY CONTINUE",
            "NEXT: CONTINUE d88",
            "NEXT: FINISH",
            "I would like to continue.",
        ] {
            assert_eq!(
                normalized_study_continue_next("self_study", text),
                None,
                "{text}"
            );
        }
    }

    #[test]
    fn continuation_is_scoped_to_explicit_verified_private_choice() {
        for text in ["NEXT: CONTINUE", "New passage.\nNEXT: continue"] {
            assert_eq!(
                normalized_private_writing_next("private_writing", text),
                Some("WRITE CONTINUE")
            );
            for mode in [
                "self_study",
                "private_writing_notice",
                "self_study_carriage_notice",
                "dialogue_live",
                "write",
            ] {
                assert_eq!(normalized_private_writing_next(mode, text), None);
            }
        }
        for text in [
            "I would like to continue.",
            "CONTINUE",
            "NEXT: CONTINUE d88",
            "```\nNEXT: CONTINUE\n```",
            "~~~\nNEXT: CONTINUE",
            "    NEXT: CONTINUE",
            "> NEXT: CONTINUE",
            "NEXT: CONTINUE\nNEXT: REST",
            "NEXT: WRITE CONTINUE",
            "NEXT: FINISH",
        ] {
            assert_eq!(
                normalized_private_writing_next("private_writing", text),
                None,
                "{text}"
            );
        }
    }
}
