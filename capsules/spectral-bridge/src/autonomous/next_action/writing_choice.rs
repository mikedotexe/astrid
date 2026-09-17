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

#[cfg(test)]
mod tests {
    use super::normalized_private_writing_next;

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
