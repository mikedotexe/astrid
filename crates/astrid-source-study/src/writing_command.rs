//! Pure writer syntax shared by preparation and legacy continuation inspection.
use anyhow::{Context as _, Result};

pub(super) struct WritingSyntax<'a> {
    pub(super) verb: String,
    pub(super) argument: &'a str,
}

impl<'a> WritingSyntax<'a> {
    pub(super) fn parse(action: &'a str) -> Result<Self> {
        let rest = action
            .trim()
            .strip_prefix("WRITE")
            .context("use WRITE <choice>")?
            .trim();
        let (verb, argument) = rest.split_once(' ').unwrap_or((rest, ""));
        Ok(Self {
            verb: verb.to_ascii_uppercase(),
            argument: argument.trim(),
        })
    }
}

/// Recognize only an explicit, argument-free private continuation.
/// This performs no I/O or draft selection; other writer choices remain outside
/// this compatibility class even when the preparation handler supports them.
#[must_use]
pub fn is_private_writing_continuation(action: &str) -> bool {
    !action.contains(['\n', '\r', '\0'])
        && action.split_whitespace().next() == Some("WRITE")
        && WritingSyntax::parse(action)
            .is_ok_and(|command| command.verb == "CONTINUE" && command.argument.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_classification_is_exact_and_does_not_expand_writer_syntax() {
        for action in ["WRITE CONTINUE", " WRITE   continue  ", "WRITE\tCONTINUE"] {
            assert!(is_private_writing_continuation(action), "{action:?}");
        }
        for action in [
            "WRITE",
            "WRITECONTINUE",
            "write CONTINUE",
            "WRITE CONTINUE extra",
            "WRITE CONTINUE\n",
            "WRITE\nCONTINUE",
            "WRITE CONTINUE\r",
            "WRITE CONTINUE\0",
            "WRITE CONTINUE\textra",
            "WRITE UNKNOWN",
            "WRITE START synthetic",
            "WRITE READ d1",
            "WRITE FINISH",
            "SELF_STUDY WRITE CONTINUE",
            "REPLACE WRITE CONTINUE",
            "EXPERIMENT_BIND e1 :: WRITE CONTINUE",
            "WRITE CONTINUE AND SELF_STUDY MAP",
        ] {
            assert!(!is_private_writing_continuation(action), "{action:?}");
        }
        // Extraction preserves the existing handler's argument semantics.
        let parsed = WritingSyntax::parse("WRITE CONTINUE retained argument").unwrap();
        assert_eq!(parsed.verb, "CONTINUE");
        assert_eq!(parsed.argument, "retained argument");
        assert!(WritingSyntax::parse("SELF_STUDY WRITE CONTINUE").is_err());
    }
}
