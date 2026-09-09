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
