//! Notebook text is data even when placed in an action slot by mistake.

/// Identify a notebook directive mistakenly offered as a NEXT action.
#[must_use]
pub fn notebook_directive(action: &str) -> Option<&str> {
    let action = action.trim_start();
    [
        "STUDY_NOTE:",
        "STUDY_QUESTION:",
        "STUDY_REVISE:",
        "STUDY_FINDING:",
        "STUDY_FINDING_DROP:",
        "STUDY_RELATION:",
    ]
    .into_iter()
    .find(|name| {
        action
            .get(..name.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
    })
}

/// Keep notebook prose and inquiry arguments out of generic AND-chain parsing.
#[must_use]
pub fn has_study_payload(action: &str) -> bool {
    if notebook_directive(action).is_some() {
        return true;
    }
    let mut words = action.split_whitespace();
    let first = words.next().unwrap_or_default();
    first.eq_ignore_ascii_case("QUESTION")
        || (first == "SELF_STUDY"
            && words
                .next()
                .is_some_and(|word| word.eq_ignore_ascii_case("QUESTION")))
}

/// Explain rejection without replaying the payload as a suggested command.
#[must_use]
pub fn notebook_action_feedback(action: &str) -> Option<String> {
    let name = notebook_directive(action)?;
    Some(format!(
        "Notebook update not applied: {name} is a response directive, not a NEXT action. \
         Its payload was not split or executed, and no note, question or finding was saved by this action. \
         To request the update, put {name} and your own intended value on a separate top-level line without NEXT:. \
         Choose any separate action on a final line such as NEXT: SELF_STUDY QUESTION or NEXT: REST. \
         Normal source-anchor and storage checks still apply; this receipt does not validate the proposed conclusion."
    ))
}
