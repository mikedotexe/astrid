// Private authoring never becomes ambient prose or a diagnostic text sample.

fn private_writing_mode(mode: &str) -> bool {
    matches!(mode, "private_writing" | "private_writing_notice")
}

fn public_response_text<'a>(mode: &str, text: &'a str) -> &'a str {
    if private_writing_mode(mode) { "" } else { text }
}

#[cfg(test)]
mod private_exchange_tests {
    use super::*;

    #[test]
    fn private_completion_preserves_exact_next_without_shared_sinks() {
        let text = "Private sentinel.\nREMEMBER private annotation\nREPLY_TO human\nPrivate letter-shaped prose\nEND_REPLY\nNEXT: WRITE CONTINUE";
        let dir = tempfile::tempdir().unwrap();
        for mode in ["private_writing", "private_writing_notice"] {
            let mut conv = ConversationState::new(Vec::new(), None);
            let (retained, share) = project_study_response(&mut conv, mode, text, dir.path());
            assert_eq!(retained, text);
            assert!(!share, "exchange DB, shadow publication and peer journals require share");
            assert_eq!(public_response_text(mode, &retained), "");
            assert!(begin_signal_shadow_v1(1, 1, mode, &retained, None).is_none());
            assert_eq!(parse_next_action(&retained), Some("WRITE CONTINUE"));
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn ordinary_prose_and_human_only_reply_boundaries_are_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut conv = ConversationState::new(Vec::new(), None);
        let text = "Public synthetic passage.\nNEXT: REST";
        let (projected, share) = project_study_response(&mut conv, "dialogue_live", text, dir.path());
        assert!(share);
        assert_eq!(public_response_text("dialogue_live", &projected), text);
        assert_eq!(project_mailbox_response(text), (projected, share));
    }
}
