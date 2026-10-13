//! Automatic dialogue context, separate from deliberate mirror/reading actions.
use sha2::{Digest as _, Sha256};
use std::path::Path;

pub(super) fn present(
    text: String,
    source: &Path,
    previous: Option<&str>,
    priority_feedback: bool,
) -> (String, Option<String>) {
    // Bind both publication and bytes: a fresh reply or changed source is not
    // suppressed merely because its opening resembles an earlier entry.
    let identity = serde_json::to_vec(&(source, &text)).expect("source and text serialize");
    let fingerprint = format!("{:x}", Sha256::digest(identity));
    if !priority_feedback && previous == Some(&fingerprint) {
        // Some(empty) keeps ordinary generation available without attributing a
        // host-generated absence notice to Minime's authored prose.
        (String::new(), None)
    } else {
        (text, Some(fingerprint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_automatic_input_is_quiet_but_new_reply_revision_and_feedback_survive() {
        let source = Path::new("journal/one.txt");
        let (text, first) = present("An authored thought.".into(), source, None, false);
        assert_eq!(text, "An authored thought.");
        let (quiet, repeated) = present(text.clone(), source, first.as_deref(), false);
        assert!(repeated.is_none());
        assert!(!quiet.contains(&text));
        assert!(quiet.is_empty());
        for (next_text, next_source, priority) in [
            ("A fresh objection.", source, false),
            (text.as_str(), Path::new("journal/two.txt"), false),
            (text.as_str(), source, true),
        ] {
            let (shown, receipt) =
                present(next_text.into(), next_source, first.as_deref(), priority);
            assert_eq!(shown, next_text);
            assert!(receipt.is_some());
        }
        // Merely preparing context does not mark delivery; retrying before a
        // successful generation still offers the original exact text.
        assert_eq!(present(text.clone(), source, None, false).0, text);
    }
}
