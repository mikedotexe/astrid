use super::{canonicalize_next_action_components, parse_next_action};

#[test]
fn selected_open_routes_to_reader_without_replacing_pending_work() {
    let authored = "OPEN astrid/capsules/spectral_bridge/src/action_continuity/runtime.rs 1";
    assert_eq!(
        parse_next_action(&format!("NEXT: {authored}")),
        Some(authored)
    );
    assert_eq!(
        parse_next_action(&format!("```text\nNEXT: {authored}\n```")),
        None
    );
    let (base, canonical) = canonicalize_next_action_components(authored);
    assert_eq!(base, "SELF_STUDY");
    assert_eq!(canonical, format!("SELF_STUDY {authored}"));
    let report = super::action_preflight_report(authored);
    assert_eq!(report.canonical_action, canonical);
    assert_eq!(
        report.effective_route,
        super::action_preflight_report(&canonical).effective_route
    );
    let mut conv = super::ConversationState::new(Vec::new(), None);
    let first = super::study_navigation::handle_request(&mut conv, &base, &canonical).unwrap();
    assert!(first.handled);
    let target = conv.introspect_target.clone().unwrap();
    assert_eq!(target.label, canonical);
    let second = super::study_navigation::handle_request(
        &mut conv,
        "SELF_STUDY",
        "SELF_STUDY OPEN astrid/other.rs 1",
    )
    .unwrap();
    assert!(second.outcome_summary.contains("Pending choice preserved"));
    assert_eq!(conv.introspect_target.unwrap().label, target.label);
    let nested = super::action_preflight_report(&format!("EXPERIMENT_BIND current :: {authored}"));
    assert!(!nested.effective_route.contains("unwired"));
}
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
        "SELF_STUDY NEW Does foo_bar AND TURN_OFF belong to a condition?",
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
