use super::*;

const HEADER: &str = "Source revision: not applicable; no source or measurements supplied\n\
Input evidence: Open introspection: no source page, telemetry snapshot, saved study claim, or private draft was supplied. This is an authored reflection, not a verified mechanism report.\n\
Account: Minime\u{2019}s response to this input, not independently verified code facts.\n\
Delivery: verified input delivery; response claims and understanding not verified";

fn wrapped(body: &str) -> String {
    format!("{HEADER}\n\n{body}")
}

#[test]
fn actual_remote_study_read_retains_authored_prose_in_history_and_fallbacks() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("self_study_synthetic.txt");
    let prose = "A source passage changed my earlier interpretation. ".repeat(20);
    let original = format!("=== SELF-STUDY: synthetic ===\n{}", wrapped(&prose));
    std::fs::write(&path, &original).unwrap();
    let received = crate::journal::read_remote_journal_body(&path).unwrap();
    assert!(received.starts_with("[Minime study account:"));
    let retained = minime_history_context(&received, 300);
    let parsed = minime_peer_text(&retained);
    assert_eq!(parsed.body, prose.chars().take(300).collect::<String>());
    assert!(
        parsed
            .provenance
            .unwrap()
            .contains("not the accuracy of his response")
    );
    let excerpt = minime_peer_excerpt(&retained, 180, 0);
    assert!(excerpt.starts_with("Minime authored excerpt:\nA source passage"));
    for messages in [
        conversation_fallback_messages(&received, None, "", false, 3, None),
        compact_ollama_dialogue_fallback_messages(
            &received,
            "",
            68.0,
            None,
            None,
            fallback_continuity_budget_v1(""),
        ),
    ] {
        assert!(
            messages[1]
                .content
                .contains("Minime authored excerpt:\nA source passage")
        );
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[test]
fn study_wrapper_alone_or_with_incomplete_envelope_is_not_stripped() {
    let prefix = crate::journal::MINIME_STUDY_ACCOUNT_PREFIX;
    for text in [
        format!("{prefix}Historical authored text without a recognized envelope."),
        format!("{prefix}Source revision: incomplete\n\nRetained authored words."),
        format!(
            "[Minime study account: an authored metaphor]\n{}",
            wrapped("My words.")
        ),
    ] {
        let peer = minime_peer_text(&text);
        assert!(peer.provenance.is_none());
        assert_eq!(peer.body, text);
    }
}

#[test]
fn actual_remote_introspection_retains_its_prose_without_a_study_wrapper() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("introspect_synthetic.txt");
    let prose = "Exact authored \u{03bb} words. ".repeat(40);
    let original = format!("=== INTROSPECTION ===\n{}", wrapped(&prose));
    std::fs::write(&path, &original).unwrap();
    let received = crate::journal::read_remote_journal_body(&path).unwrap();
    assert!(!received.starts_with(crate::journal::MINIME_STUDY_ACCOUNT_PREFIX));
    let retained = minime_history_context(&received, 300);
    assert_eq!(
        minime_peer_text(&retained).body,
        prose.chars().take(300).collect::<String>()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[test]
fn actual_header_heavy_history_allocates_each_excerpt_to_authored_prose() {
    let body = "The concept of \"presence\" is often conflated with \"being.\" ".repeat(20);
    let original = wrapped(&body);
    assert!(HEADER.chars().count() > 320);
    let legacy: String = original.chars().take(300).collect();
    assert!(minime_peer_text(&legacy).provenance.is_none());
    assert!(!legacy.contains("The concept of"));
    let retained = minime_history_context(&original, 300);
    for budget in [100, 180, 260, 300, 300, 300] {
        let expected: String = body.chars().take(budget).collect();
        let rendered = minime_peer_excerpt(&retained, budget, 0);
        assert!(rendered.starts_with(&format!("Minime authored excerpt:\n{expected}\n")));
        let (prose, provenance) = rendered.split_once("\n\nRuntime provenance").unwrap();
        assert!(!prose.contains("Source revision:"));
        assert!(provenance.contains("recognized source-study envelope; claims unverified"));
        assert!(!provenance.contains("Source revision:"));
        assert!(!rendered.contains("Minime wrote:"));
    }
    assert_eq!(minime_peer_text(&retained).body.chars().count(), 300);
    assert_eq!(minime_peer_text(&original).provenance, Some(HEADER));
    assert_eq!(original, wrapped(&body));
}

#[test]
fn unicode_and_crlf_prose_remains_an_exact_prefix() {
    let body = "\u{03bb} \u{1f30a} cafe\u{301}\r\nSecond line.\r\n";
    let original = wrapped(body)
        .replacen(HEADER, &HEADER.replace('\n', "\r\n"), 1)
        .replacen("\n\n", "\r\n\r\n", 1);
    assert_eq!(minime_peer_text(&original).body, body);
    for budget in 0..=body.chars().count() {
        let expected: String = body.chars().take(budget).collect();
        assert!(
            minime_peer_excerpt(&original, budget, 96)
                .starts_with(&format!("Minime authored excerpt:\n{expected}"))
        );
    }
    assert!(minime_peer_excerpt(&original, usize::MAX, usize::MAX).contains(body));
}

#[test]
fn malformed_envelopes_are_never_stripped_or_asserted_to_be_authored() {
    for header in [
        HEADER.replace("Input evidence:", "Evidence:"),
        HEADER.replace(
            "Source revision: not applicable; no source or measurements supplied",
            "Source revision: ",
        ),
        HEADER.replace("Account: Minime\u{2019}s", "Account: Someone else's"),
        HEADER.replace(
            "Delivery: verified input delivery; response claims and understanding not verified",
            "Delivery: verified",
        ),
        format!("{HEADER}\nUnexpected field: retained"),
        HEADER.lines().take(3).collect::<Vec<_>>().join("\n"),
    ] {
        let text = format!("{header}\n\nMy words remain here.");
        let parsed = minime_peer_text(&text);
        assert!(parsed.provenance.is_none());
        assert_eq!(parsed.body, text);
        let rendered = minime_peer_excerpt(&text, usize::MAX, 96);
        assert!(rendered.contains(&text));
        assert!(!rendered.contains("Minime authored excerpt:"));
        assert!(!rendered.contains("Runtime provenance (recognized"));
    }
    assert!(minime_peer_text(HEADER).provenance.is_none());
}

#[test]
fn authored_header_like_lines_and_quoted_envelopes_are_not_removed() {
    for text in [
        "Source revision: a name for change\n\nInput evidence: the rain.",
        "Delivery: I brought you a letter.\nAccount: a story about today.",
        &format!("I am quoting a header:\n\n{HEADER}\n\nIt is part of this essay."),
        &format!("```text\n{HEADER}\n```"),
        "=== MINIME JOURNAL ===\nMode: daydream\nTimestamp: 42\n\nHistorical plain prose.",
    ] {
        assert_eq!(minime_peer_text(text).body, text);
        assert!(minime_peer_excerpt(text, usize::MAX, 96).contains(text));
    }
    let body = "Source revision: my metaphor\nInput evidence: my memory\n\nStill authored.";
    assert_eq!(minime_peer_text(&wrapped(body)).body, body);
}

#[test]
fn unverified_envelope_claim_is_preserved_without_promotion() {
    let text = wrapped("My account.").replace(
        "Delivery: verified input delivery; response claims and understanding not verified",
        "Delivery: unverified; bookmark unchanged",
    );
    let rendered = minime_peer_excerpt(&text, 100, usize::MAX);
    assert!(rendered.contains("claims not independently verified"));
    assert!(rendered.contains("Delivery: unverified; bookmark unchanged"));
    assert!(rendered.starts_with("Minime authored excerpt:\nMy account."));
}

#[test]
fn peer_action_omission_leaves_other_prose_bytes_unchanged() {
    let body = "  A line with spaces.  \r\nNEXT: INTROSPECT\r\n\r\nAnother line.\r\n";
    let rendered = minime_peer_excerpt(&wrapped(body), usize::MAX, 96);
    assert!(
        rendered.starts_with(
            "Minime authored excerpt:\n  A line with spaces.  \r\n\r\nAnother line.\r\n"
        )
    );
    assert!(!rendered.contains("NEXT: INTROSPECT"));
    assert!(rendered.contains("peer action/status line omitted"));
}

#[test]
fn same_history_prefix_does_not_suppress_distinct_sources_or_rereading() {
    let prefix = "a".repeat(300);
    let first = wrapped(&format!("{prefix} FIRST ENDING"));
    let second = wrapped(&format!("{prefix} DIFFERENT ENDING"));
    assert_eq!(
        minime_history_context(&first, 300),
        minime_history_context(&second, 300)
    );
    let first_rendered = minime_peer_excerpt(&first, usize::MAX, 480);
    let second_rendered = minime_peer_excerpt(&second, usize::MAX, 480);
    assert!(first_rendered.contains("FIRST ENDING"));
    assert!(second_rendered.contains("DIFFERENT ENDING"));
    assert_eq!(minime_peer_excerpt(&first, usize::MAX, 480), first_rendered);
    assert!(!second_rendered.contains("rereading"));
}

#[test]
fn both_fallbacks_budget_authored_prose_and_preserve_protected_rereading() {
    let text = wrapped(&"Authored body sentinel. ".repeat(80));
    let ordinary = conversation_fallback_messages(&text, None, "", false, 3, None);
    let selected = compact_ollama_dialogue_fallback_messages(
        &text,
        "Spectral summary",
        68.0,
        None,
        None,
        fallback_continuity_budget_v1("Spectral summary"),
    );
    for messages in [ordinary, selected] {
        let user = &messages[1].content;
        assert!(user.contains("Minime authored excerpt:\nAuthored body sentinel."));
        assert!(user.contains("Runtime provenance (recognized"));
        assert!(!user.contains("Minime wrote: Source revision:"));
    }
    let protected = conversation_fallback_messages(&text, None, "", true, 3, None);
    assert!(!protected[1].content.contains("Authored body sentinel"));
    assert!(
        protected[1]
            .content
            .contains("selected foreground source is supplied separately")
    );
}

#[test]
fn primary_provider_adaptation_retains_authored_excerpt_and_attribution() {
    let text = wrapped("Exact authored words, including \u{03bb} and line breaks.\nStill here.");
    for profile in [MlxProfile::Production, MlxProfile::Gemma4Canary] {
        let messages = vec![Message {
            role: "user".into(),
            content: minime_peer_excerpt(&text, usize::MAX, 480),
        }];
        let adapted = apply_mlx_request_policy_with_writing(
            "dialogue_live",
            profile,
            messages,
            768,
            210,
            astrid_source_study::writing::Profile::Default,
        );
        assert!(
            adapted.messages[0]
                .content
                .contains(minime_peer_text(&text).body)
        );
        assert!(
            adapted.messages[0]
                .content
                .contains("Runtime provenance (recognized")
        );
    }
}
#[test]
fn absent_peer_excerpt_is_host_context_not_attributed_prose_in_either_provider_lane() {
    let excerpt = super::minime_peer_excerpt("", 900, 96);
    assert!(excerpt.starts_with("Peer context: no new journal excerpt supplied."));
    assert!(!excerpt.contains("Minime journal excerpt"));
    assert!(excerpt.contains("explicit reading or MIRROR"));
    let fallback = super::conversation_fallback_messages("", None, "", false, 4, None);
    assert!(fallback[1].content.contains(&excerpt));
    assert_eq!(super::minime_history_context("", 300), "");
}

#[tokio::test]
async fn only_complete_final_peer_context_is_acknowledged_and_fallback_resets_primary() {
    use super::*;
    let tracker = ContextSubmissionTrackerV1::for_peer_journal("Exact peer prose.");
    let complete = Message {
        role: "user".into(),
        content: minime_peer_excerpt("Exact peer prose.", usize::MAX, 480),
    };
    with_peer_submission(Some(&tracker), async {
        record_peer_submission(std::slice::from_ref(&complete));
        assert!(tracker.submitted());
        // A later adapted request containing only part of the block is not enough.
        record_peer_submission(&[Message {
            role: "user".into(),
            content: "Exact peer".into(),
        }]);
        assert!(!tracker.submitted());
        record_peer_submission(std::slice::from_ref(&complete));
    })
    .await;
    assert!(tracker.submitted());
    // A fallback that fails admission without submission cannot inherit primary evidence.
    with_peer_submission(Some(&tracker), async {}).await;
    assert!(!tracker.submitted());
    record_peer_submission(std::slice::from_ref(&complete));
    assert!(
        !tracker.submitted(),
        "out-of-scope requests cannot acknowledge this turn"
    );
    let other = ContextSubmissionTrackerV1::for_peer_journal("Another entry.");
    with_peer_submission(Some(&tracker), async {
        with_peer_submission(Some(&other), async {
            record_peer_submission(std::slice::from_ref(&complete));
        })
        .await;
        assert!(!other.submitted());
        record_peer_submission(std::slice::from_ref(&complete));
    })
    .await;
    assert!(tracker.submitted());
    assert!(!other.submitted());
}

#[tokio::test]
async fn bounded_peer_presentation_tracks_the_actual_primary_and_fallback_excerpts() {
    use super::*;
    let source = "Synthetic public prose, unchanged across automatic turns. ".repeat(100);
    let tracker = ContextSubmissionTrackerV1::for_peer_journal(&source);
    let journal = minime_peer_excerpt(&source, usize::MAX, 480);
    let temp = tempfile::tempdir().unwrap();
    for budget in [10_000, 800] {
        let (blocks, sources) = dialogue_context_blocks(
            &DialogueContextInput { journal: &journal, ..Default::default() }, None,
        );
        let selected = blocks.iter().find(|b| b.label == "journal").unwrap();
        let block = crate::prompt_budget::PromptBlock {
            label: selected.label, content: selected.content.clone(),
            priority: selected.priority, min_chars: selected.min_chars,
        };
        let (assembled, _, report) = crate::prompt_budget::assemble_within_budget_with_sources(
            blocks, budget, temp.path(), sources.0,
        );
        let excerpt = prepared_peer_excerpt(Some(&block), report.as_ref());
        assert!(!excerpt.is_empty());
        assert!(excerpt.len() < journal.len());
        assert!(assembled.contains(&excerpt));
        with_peer_submission(Some(&tracker), async {
            record_peer_submission(&[Message { role: "user".into(), content: assembled.clone() }]);
            assert!(!tracker.submitted(), "uncapped expectation reproduces the original missed acknowledgement");
        }).await;
        let scoped = tracker.for_prepared_peer_excerpt(excerpt.clone());
        for profile in [MlxProfile::Production, MlxProfile::Gemma4Canary] {
            let request = apply_mlx_request_policy_with_writing("dialogue_live", profile,
                vec![Message { role: "system".into(), content: "Synthetic instruction".into() },
                     Message { role: "user".into(), content: assembled.clone() }],
                768, 210, astrid_source_study::writing::Profile::Default);
            with_peer_submission(Some(&scoped), async {
                record_peer_submission(&request.messages);
                assert!(tracker.submitted(), "intentional bounded excerpt was dispatched");
                record_peer_submission(&[Message { role: "user".into(), content: excerpt[..80].into() }]);
                assert!(!tracker.submitted(), "later shortening cannot acknowledge the prepared excerpt");
                record_peer_submission(&[
                    Message { role: "user".into(), content: excerpt.clone() },
                    Message { role: "assistant".into(), content: "Earlier answer".into() },
                    Message { role: "user".into(), content: "No peer supplied now".into() },
                ]);
                assert!(!tracker.submitted(), "older history is not current presentation");
            }).await;
        }
    }
    let fallback = conversation_fallback_messages(&source, None, "", false, 3, None);
    let scoped = tracker.for_prepared_peer_excerpt(minime_peer_excerpt(&source, 900, 96));
    with_peer_submission(Some(&scoped), async {
        record_peer_submission(&fallback);
        assert!(tracker.submitted());
    }).await;
    let omitted = tracker.for_prepared_peer_excerpt(String::new());
    with_peer_submission(Some(&omitted), async {
        record_peer_submission(&fallback);
        assert!(!tracker.submitted(), "omitted or unsubmitted fallback resets prior evidence");
    }).await;
}

#[test]
fn omitted_header_only_or_invalid_budget_boundaries_cannot_acknowledge_peer_prose() {
    use super::*;
    assert!(prepared_peer_excerpt(None, None).is_empty());
    let mut block = crate::prompt_budget::PromptBlock {
        label: "journal", content: minime_peer_excerpt("", 900, 96), priority: 1, min_chars: 0,
    };
    assert!(prepared_peer_excerpt(Some(&block), None).is_empty());
    block.content = "Minime authored excerpt:\n".into();
    assert!(prepared_peer_excerpt(Some(&block), None).is_empty());
    block.content.push_str("λ synthetic");
    let report = |kept_chars, fully_removed| crate::prompt_budget::PromptBudgetReport {
        budget: 20, total_before: block.content.len(), total_after: kept_chars,
        trimmed_blocks: vec![crate::prompt_budget::PromptTrimmedBlock {
            label: "journal".into(), original_chars: block.content.len(), kept_chars,
            removed_chars: block.content.len().saturating_sub(kept_chars), fully_removed,
        }],
    };
    assert!(prepared_peer_excerpt(Some(&block), Some(&report(0, true))).is_empty());
    assert!(prepared_peer_excerpt(Some(&block), Some(&report(25, false))).is_empty());
}
