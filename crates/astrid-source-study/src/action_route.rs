//! Routing a selected source command is not source admission or delivery.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SourceActionRoute {
    pub policy: String,
    pub canonical_action: String,
}

/// Classify only the explicit OPEN alias. Invalid arguments still belong to the
/// reader's recovery path; catalog validation and delivery checks remain there.
#[must_use]
pub fn source_action_route(action: &str) -> Option<SourceActionRoute> {
    let action = action.trim();
    let (verb, argument) = action
        .split_once(char::is_whitespace)
        .unwrap_or((action, ""));
    if !verb.eq_ignore_ascii_case("OPEN") {
        return None;
    }
    Some(SourceActionRoute {
        policy: "source_open_alias_v1".into(),
        canonical_action: format!("SELF_STUDY OPEN {}", argument.trim())
            .trim_end()
            .into(),
    })
}

/// Classify ordinary source navigation for explicit research accounting only.
/// The reader still validates the catalog, pending work and source delivery.
/// Notebook mutations, private writing and experiment execution are not reads.
#[must_use]
pub fn research_source_route(action: &str) -> Option<String> {
    let canonical = source_action_route(action)
        .map_or_else(|| action.trim().to_owned(), |route| route.canonical_action);
    let (base, rest) = canonical.split_once(char::is_whitespace)?;
    if !base.eq_ignore_ascii_case("SELF_STUDY") || rest.contains(['\n', '\r', '\0']) {
        return None;
    }
    let (verb, arguments) = rest
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((rest.trim(), ""));
    if !matches!(
        verb.to_ascii_uppercase().as_str(),
        "MAP"
            | "LIST"
            | "FIND"
            | "OPEN"
            | "RESUME"
            | "CONTINUE"
            | "NOTE"
            | "HELP"
            | "RELATE"
            | "SESSION"
            | "TRACE"
    ) {
        return None;
    }
    let command = format!("SELF_STUDY {} {}", verb.to_ascii_uppercase(), arguments);
    crate::Command::parse(command.trim()).ok()?;
    Some(command.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::source_action_route;

    #[test]
    fn research_source_routes_are_shared_and_bounded() {
        for action in [
            "OPEN astrid/a.rs 1",
            "SELF_STUDY OPEN astrid/a.rs 1",
            "self_study find CONTROL Vec<T>",
            "SELF_STUDY RELATE EventBus::new",
        ] {
            assert!(super::research_source_route(action).is_some(), "{action}");
        }
        for action in [
            "OPEN",
            "OPEN astrid/a.rs 0",
            "SELF_STUDY WRITE START secret",
            "SELF_STUDY QUESTION NEW authored",
            "SELF_STUDY EXPERIMENT RUN x",
            "CONTROL mode_disperse",
            "SELF_STUDY unknown",
            "SELF_STUDY OPEN a.rs\nNEXT: REST",
        ] {
            assert!(super::research_source_route(action).is_none(), "{action}");
        }
    }

    #[test]
    fn selected_open_only_preserves_arguments() {
        for (raw, expected) in [
            (
                "OPEN astrid/src/Case.rs 12",
                "SELF_STUDY OPEN astrid/src/Case.rs 12",
            ),
            (
                "open\tminime/example.py",
                "SELF_STUDY OPEN minime/example.py",
            ),
            ("OPEN", "SELF_STUDY OPEN"),
            ("OPEN ../private.txt 0", "SELF_STUDY OPEN ../private.txt 0"),
        ] {
            assert_eq!(source_action_route(raw).unwrap().canonical_action, expected);
        }
        for raw in [
            "SELF_STUDY OPEN a.rs",
            "NEXT: OPEN a.rs",
            "`OPEN a.rs`",
            "I chose OPEN a.rs",
            "RESUME a.rs",
            "OPEN_EYES",
            "WRITE START OPEN a.rs",
        ] {
            assert!(source_action_route(raw).is_none(), "{raw}");
        }
    }
}
