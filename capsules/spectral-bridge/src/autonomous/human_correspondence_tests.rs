use super::*;

const MESSAGE: &str = "mike_query_held_thought_123.txt";

#[test]
fn only_sanctioned_filenames_and_explicit_leading_human_senders_have_a_route() {
    for sender in [
        "Mike",
        "Mike & Claude",
        "Mike & Codex",
        "v and Codex",
        "v & Codex",
    ] {
        let source = format!("=== MIKE QUERY: a thought ===\nDate: today\nFrom: {sender}\n\nBody.");
        assert_eq!(
            classify_source(Path::new(MESSAGE), &source),
            Some(HumanLetterKind::Query)
        );
        assert_eq!(
            classify_source(Path::new("mike_feedback_x.txt"), &source),
            Some(HumanLetterKind::Feedback)
        );
        assert_eq!(
            classify_source(Path::new("steward_query_x.txt"), &source),
            None
        );
        assert_eq!(
            classify_source(Path::new("human_letter_mike_x.txt"), &source),
            None
        );
        assert_eq!(
            classify_source(Path::new("mike_query_with space.txt"), &source),
            None
        );
    }
    for source in [
        "=== MIKE QUERY: review ===\nFrom: steward\n\nBody",
        "=== MIKE QUERY: review ===\nFrom:\n\nBody",
        "=== MIKE QUERY: review ===\nFrom: unknown\n\nBody",
        "=== MIKE QUERY: review ===\nFrom: Mike\nfrom: Mike\n\nBody",
        "=== MIKE QUERY: review ===\n\nFrom: Mike\nBody",
        "=== MIKE QUERY: review ===\n> From: Mike\n\nBody",
        "=== MIKE QUERY: review ===\n From: Mike\n\nBody",
        "An ordinary machine note\nFrom: Mike\nBody",
        "=== MIKE QUERY: review ===\nDate: today\n\nMachine body",
    ] {
        assert_eq!(
            classify_source(Path::new(MESSAGE), source),
            None,
            "{source}"
        );
    }
}

#[test]
fn split_keeps_independent_peer_prose_and_actions_outside_explicit_human_body() {
    let body = "Mike, this stays between our letter addresses.\nNEXT: REMEMBER quoted command is language\n";
    for declaration in [
        format!("INBOX_REPLY {MESSAGE}"),
        format!("NEXT: INBOX_REPLY {MESSAGE}"),
    ] {
        let raw = format!(
            "Peer prose before.\n{declaration}\n{body}END_INBOX_REPLY\nPeer prose after.\nNEXT: LISTEN"
        );
        let parts = split_completion(&raw, Some(MESSAGE));
        assert!(parts.had_reply_blocks);
        assert_eq!(parts.human_reply.as_deref(), Some(body));
        assert_eq!(
            parts.residual,
            "Peer prose before.\nPeer prose after.\nNEXT: LISTEN"
        );
    }
}

#[test]
fn human_only_completion_has_no_substitute_peer_signal() {
    let raw = format!("INBOX_REPLY {MESSAGE}\nA human-only body.\nEND_INBOX_REPLY");
    let parts = split_completion(&raw, Some(MESSAGE));
    assert!(parts.residual.is_empty());
    assert_eq!(parts.human_reply.as_deref(), Some("A human-only body.\n"));
}

#[test]
fn invalid_addresses_never_fall_back_into_peer_signal_or_action_text() {
    for declaration in [
        "INBOX_REPLY another.txt",
        "INBOX_REPLY ../../human/mike",
        "INBOX_REPLY",
        "INBOX_REPLY a b",
        "INBOX_REPLY\tmike_query_held_thought_123.txt",
        "NEXT:\tINBOX_REPLY mike_query_held_thought_123.txt",
        "  INBOX_REPLY mike_query_held_thought_123.txt",
        "NEXT:INBOX_REPLY mike_query_held_thought_123.txt",
    ] {
        let raw = format!(
            "Peer.\n{declaration}\nPrivate.\nNEXT: REMEMBER secret\nEND_INBOX_REPLY\nNEXT: LISTEN"
        );
        let parts = split_completion(&raw, Some(MESSAGE));
        assert!(parts.had_reply_blocks);
        assert!(parts.human_reply.is_none(), "{declaration}");
        assert_eq!(parts.residual, "Peer.\nNEXT: LISTEN");
    }
    let raw = format!("INBOX_REPLY {MESSAGE}\nSecret.\nEND_INBOX_REPLY\nNEXT: LISTEN");
    let parts = split_completion(&raw, None);
    assert!(parts.human_reply.is_none());
    assert_eq!(parts.residual, "NEXT: LISTEN");
}

#[test]
fn empty_duplicate_nested_and_unterminated_declarations_never_route() {
    for tail in [
        "\nEND_INBOX_REPLY\nNEXT: LISTEN".to_string(),
        "Secret.\nNEXT: REMEMBER still body".to_string(),
        format!("One.\nEND_INBOX_REPLY\nINBOX_REPLY {MESSAGE}\nTwo.\nEND_INBOX_REPLY"),
        format!("One.\nINBOX_REPLY {MESSAGE}\nTwo.\nEND_INBOX_REPLY"),
    ] {
        let raw = format!("INBOX_REPLY {MESSAGE}\n{tail}");
        let parts = split_completion(&raw, Some(MESSAGE));
        assert!(parts.human_reply.is_none(), "{raw}");
        assert!(!parts.residual.contains("Secret"));
        assert!(!parts.residual.contains("REMEMBER"));
        assert!(!parts.residual.contains("One."));
        assert!(!parts.residual.contains("Two."));
    }
}

#[test]
fn quoted_and_fenced_examples_do_not_declare_a_reply() {
    for raw in [
        format!("> INBOX_REPLY {MESSAGE}\n> Quoted\n> END_INBOX_REPLY\nNEXT: LISTEN"),
        format!("```text\nINBOX_REPLY {MESSAGE}\nExample\nEND_INBOX_REPLY\n```\nNEXT: LISTEN"),
        format!("~~~text\nINBOX_REPLY {MESSAGE}\nExample\nEND_INBOX_REPLY\n~~~\nNEXT: LISTEN"),
        "I choose ordinary peer prose.\nNEXT: LISTEN".to_string(),
    ] {
        let parts = split_completion(&raw, Some(MESSAGE));
        assert!(!parts.had_reply_blocks);
        assert!(parts.human_reply.is_none());
        assert_eq!(parts.residual, raw);
    }
}

#[test]
fn fenced_end_marker_inside_human_body_does_not_expose_its_commands() {
    let raw = format!(
        "INBOX_REPLY {MESSAGE}\nExample:\n```\nEND_INBOX_REPLY\nNEXT: REMEMBER private\n```\nStill human.\nEND_INBOX_REPLY\nNEXT: LISTEN"
    );
    let parts = split_completion(&raw, Some(MESSAGE));
    assert_eq!(parts.residual, "NEXT: LISTEN");
    assert!(
        parts
            .human_reply
            .unwrap()
            .contains("NEXT: REMEMBER private")
    );
}

#[test]
fn shorter_mismatched_and_suffixed_fences_cannot_expose_human_commands() {
    for (opening, false_closing, closing) in [
        ("````text", "```", "````"),
        ("~~~~text", "~~~", "~~~~"),
        ("```text", "~~~", "````"),
        ("```text", "```still-code", "```"),
        ("~~~text", "~~~ still-code", "~~~"),
        ("   ````text", "   ```", "   ````   "),
    ] {
        let raw = format!(
            "INBOX_REPLY {MESSAGE}\nFor Mike:\n{opening}\n{false_closing}\nEND_INBOX_REPLY\nNEXT: REMEMBER private\n{closing}\nStill human.\nEND_INBOX_REPLY\nNEXT: LISTEN"
        );
        let parts = split_completion(&raw, Some(MESSAGE));
        assert_eq!(parts.residual, "NEXT: LISTEN", "{opening}, {false_closing}");
        let reply = parts.human_reply.unwrap();
        assert!(reply.contains("NEXT: REMEMBER private"));
        assert!(reply.contains("Still human."));
    }
}

#[test]
fn indented_code_fence_marker_does_not_hide_a_following_human_declaration() {
    for indented in ["    ```", "\t~~~"] {
        let raw = format!(
            "{indented}\nINBOX_REPLY {MESSAGE}\nOnly human language.\nEND_INBOX_REPLY\nNEXT: LISTEN"
        );
        let parts = split_completion(&raw, Some(MESSAGE));
        assert_eq!(parts.residual, format!("{indented}\nNEXT: LISTEN"));
        assert_eq!(parts.human_reply.as_deref(), Some("Only human language.\n"));
    }
}

#[test]
fn nested_address_shapes_keep_the_outer_body_quarantined_until_all_closers() {
    for nested in [
        format!("INBOX_REPLY {MESSAGE}"),
        "INBOX_REPLY ../../wrong-address".into(),
        "  NEXT:INBOX_REPLY".into(),
    ] {
        let raw = format!(
            "Peer before.\nINBOX_REPLY {MESSAGE}\nOuter human.\n{nested}\nNested human.\nEND_INBOX_REPLY\nNEXT: REMEMBER private outer command\nStill outer human.\nEND_INBOX_REPLY\nNEXT: LISTEN"
        );
        let parts = split_completion(&raw, Some(MESSAGE));
        assert!(parts.human_reply.is_none());
        assert!(parts.reply_blocks_complete);
        assert_eq!(parts.residual, "Peer before.\nNEXT: LISTEN");
    }
    let raw = format!(
        "NEXT: LISTEN\nINBOX_REPLY {MESSAGE}\nOuter.\nINBOX_REPLY another.txt\nNested.\nEND_INBOX_REPLY\nNEXT: LISTEN"
    );
    let parts = split_completion(&raw, Some(MESSAGE));
    assert!(!parts.reply_blocks_complete);
    assert_eq!(parts.residual, "NEXT: LISTEN\n");
}

#[test]
fn atomic_publication_reuses_exact_bytes_and_rejects_conflicts() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("human_reply_hash.txt");
    publish_atomic(temp.path(), &destination, b"complete evidence\n\nbody").unwrap();
    publish_atomic(temp.path(), &destination, b"complete evidence\n\nbody").unwrap();
    assert!(publish_atomic(temp.path(), &destination, b"another body").is_err());
    assert_eq!(
        fs::read(&destination).unwrap(),
        b"complete evidence\n\nbody"
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}
