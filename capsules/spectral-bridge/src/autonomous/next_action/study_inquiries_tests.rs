use super::{canonicalize_next_action_components, parse_next_action};
#[test]
fn inquiry_navigation_preserves_exact_choices_and_session_separators() {
    for choice in [
        "SELF_STUDY RELATE EventDispatcher",
        "SELF_STUDY SESSION OPEN astrid/Cargo.toml 1 | OPEN minime/pyproject.toml 1",
        "SELF_STUDY TRACE LAST",
    ] {
        assert_eq!(
            parse_next_action(&format!("I choose this.\n{choice}")),
            Some(choice)
        );
        assert_eq!(parse_next_action(&format!("NEXT: {choice}")), Some(choice));
        assert_eq!(parse_next_action(&format!("```\n{choice}\n```")), None);
        assert_eq!(canonicalize_next_action_components(choice).1, choice);
    }
    let mutation = "SELF_STUDY QUESTION NEW Where does the dispatch happen?";
    assert_eq!(parse_next_action(mutation), None);
    assert_eq!(
        parse_next_action(&format!("NEXT: {mutation}")),
        Some(mutation)
    );
}

#[test]
fn study_payloads_are_not_split_into_commands() {
    for action in [
        "STUDY_FINDING: astrid/demo.rs:1 | A check and apply_identity_config (authorization).",
        "SELF_STUDY QUESTION NEW How do foo_bar and baz_quux relate?",
        "SELF_STUDY QUESTION RESOLVE q1 Uncertain about foo_bar AND TURN_OFF",
        "QUESTION RESOLVE q1 foo_bar AND TURN_OFF",
        "STUDY_QUESTION: Literal </s> and TURN_OFF",
        "SELF_STUDY QUESTION NEW What is (RESIDUE: literal)?  ",
    ] {
        assert_eq!(super::split_multi_action(action), [action]);
        assert_eq!(
            super::parse_next_action(&format!("NEXT: {action}")),
            Some(action)
        );
        assert_eq!(super::canonicalize_next_action_text(action), action);
    }
    let directive = "STUDY_NOTE: A check AND TURN_OFF";
    assert_eq!(
        super::split_multi_action(&format!("REST AND {directive}")),
        ["REST", directive]
    );
    let mut conv = super::ConversationState::new(Vec::new(), None);
    let outcome = super::study_navigation::reject_notebook_action(&mut conv, directive).unwrap();
    assert!(!outcome.handled);
    assert!(
        outcome
            .outcome_summary
            .contains("Notebook update not applied")
    );
    assert!(conv.introspect_target.is_none());
    assert!(conv.emphasis.unwrap().contains("not split or executed"));
}
