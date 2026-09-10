//! Suggestions only: malformed local navigation never becomes an implicit Action.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NavigationRecovery {
    pub explanation: String,
    pub commands: Vec<String>,
    pub text: String,
}

/// Offer valid local study syntax without executing or authorizing an alternative.
/// Network SEARCH authority and the Being's pending choice remain unchanged.
#[must_use]
pub fn recover_local_navigation(action: &str) -> Option<NavigationRecovery> {
    let action = action.trim().strip_prefix("NEXT:").unwrap_or(action).trim();
    let prefixed = action.starts_with("SELF_STUDY ");
    let action = action.strip_prefix("SELF_STUDY ").unwrap_or(action);
    let (verb, rest) = action
        .split_once(char::is_whitespace)
        .unwrap_or((action, ""));
    let rest = rest.trim();
    if rest.len() > 600 || rest.contains(['\n', '\r', '|']) {
        return None;
    }
    let mut commands = Vec::new();
    let explanation = if verb.eq_ignore_ascii_case("RELATE") {
        let valid_query = matches!(crate::Command::parse(&format!("RELATE {rest}")), Ok(crate::Command::Relate { symbol, .. }) if crate::relationships::valid_symbol(&symbol));
        if valid_query {
            if prefixed {
                return None;
            }
            commands.push(format!("SELF_STUDY RELATE {rest}"));
            "RELATE is a local source-study operation and needs the SELF_STUDY prefix. It accepts one exact identifier."
        } else {
            for part in rest.split_whitespace().take(6) {
                let part = part.trim_matches(['\'', '"', '`']);
                let command = if code_path(part) {
                    Some(format!("SELF_STUDY FIND {part}"))
                } else if crate::relationships::valid_symbol(part) {
                    Some(format!("SELF_STUDY RELATE {part}"))
                } else {
                    None
                };
                if let Some(command) = command
                    && !commands.contains(&command)
                {
                    commands.push(command);
                }
                if commands.len() == 3 {
                    break;
                }
            }
            "RELATE accepts one exact identifier, not a file plus a list of symbols. FIND searches one literal query across catalog paths and text; these are separate choices, not a combined or scoped query."
        }
    } else if (verb.eq_ignore_ascii_case("SEARCH") || verb.eq_ignore_ascii_case("RESEARCH"))
        && !prefixed
    {
        let query = rest.trim_matches(['\'', '"', '`']);
        if !code_path(query) {
            return None;
        }
        commands.push(format!("SELF_STUDY FIND {query}"));
        "SEARCH uses a separate search route. If you meant local source, FIND searches the shared catalog's paths and text. This suggestion does not change SEARCH authority."
    } else {
        return None;
    };
    if commands.is_empty() {
        commands.push("SELF_STUDY MAP".into());
    }
    let text = format!(
        "Local study navigation help: {explanation}\nAvailable exact commands (choose one if useful):\n{}\nNo substitute command was executed or queued. You can retry, read elsewhere, reread, or stop.",
        commands.join("\n")
    );
    Some(NavigationRecovery {
        explanation: explanation.into(),
        commands,
        text,
    })
}

fn code_path(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(char::is_whitespace)
        && !value.contains("://")
        && [
            ".rs", ".py", ".ts", ".js", ".toml", ".wit", ".metal", ".wgsl", ".cpp", ".h",
        ]
        .iter()
        .any(|extension| value.ends_with(extension))
}
