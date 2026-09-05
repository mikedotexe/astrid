#[cfg(test)]
mod context_retrieval_tests {
    use super::{DialogueBlockSources, cap_dialogue_block};
    use crate::prompt_budget::{
        PromptBlock, assemble_within_budget, assemble_within_budget_with_sources,
    };

    fn blocks(sources: &mut DialogueBlockSources, agenda: &str, peer: &str) -> Vec<PromptBlock> {
        vec![
            PromptBlock {
                label: "agenda",
                content: sources.cap("agenda", agenda, 700),
                priority: 3,
                min_chars: 320,
            },
            PromptBlock {
                label: "journal",
                content: sources.cap("journal", peer, 800),
                priority: 1,
                min_chars: 350,
            },
            PromptBlock {
                label: "continuity",
                content: sources.cap("continuity", &"Shared history. ".repeat(150), 600),
                priority: 7,
                min_chars: 0,
            },
        ]
    }

    #[test]
    fn capped_sources_remain_retrievable_without_changing_prompt_or_floors() {
        let agenda = format!(
            "Your agenda:\n{}SELF_AUTHORED_TAIL",
            "chosen study. ".repeat(100)
        );
        let peer = format!(
            "Minime wrote: {}PEER_AUTHORED_TAIL",
            "peer context. ".repeat(100)
        );
        for budget in [0, 500, 20_000] {
            let before_dir = tempfile::tempdir().expect("before");
            let after_dir = tempfile::tempdir().expect("after");
            let before = assemble_within_budget(
                blocks(&mut DialogueBlockSources::default(), &agenda, &peer),
                budget,
                before_dir.path(),
            );
            let mut sources = DialogueBlockSources::default();
            let capped = blocks(&mut sources, &agenda, &peer);
            let after =
                assemble_within_budget_with_sources(capped, budget, after_dir.path(), sources.0);
            assert_eq!(
                before.0, after.0,
                "visible prompt changed at budget {budget}"
            );
            assert!(after.0.contains(&agenda[..320]));
            assert!(after.0.contains("Minime wrote:"));
            assert!(!after.0.contains("SELF_AUTHORED_TAIL"));
            let overflow = after
                .1
                .expect("source overflow even when capped prompt fits");
            let saved = std::fs::read_to_string(&overflow.path).expect("saved");
            assert!(saved.contains(&format!("=== [agenda] ===\n\n{agenda}\n")));
            assert!(saved.contains(&format!("=== [journal] ===\n\n{peer}\n")));
            assert_eq!(saved.matches("=== [agenda] ===").count(), 1);
            assert_eq!(overflow.offset, 0);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&overflow.path)
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
            if let Some(report) = after.2 {
                assert!(
                    report
                        .trimmed_blocks
                        .iter()
                        .filter(|b| b.label == "agenda")
                        .all(|b| b.kept_chars >= 320)
                );
            }
        }
    }

    #[test]
    fn uncapped_short_source_creates_no_overflow() {
        let dir = tempfile::tempdir().expect("dir");
        let mut sources = DialogueBlockSources::default();
        let blocks = vec![PromptBlock {
            label: "agenda",
            content: sources.cap("agenda", "My chosen study", 700),
            priority: 3,
            min_chars: 320,
        }];
        let (text, overflow, report) =
            assemble_within_budget_with_sources(blocks, 1000, dir.path(), sources.0);
        assert_eq!(text, "My chosen study");
        assert!(overflow.is_none());
        assert!(report.is_none());
    }

    #[test]
    fn adjacent_turns_never_replace_an_earlier_overflow() {
        let dir = tempfile::tempdir().expect("dir");
        let mut paths = Vec::new();
        for index in 0..5 {
            let source = format!("Your agenda: {} tail-{index}", "study ".repeat(150));
            let mut capture = DialogueBlockSources::default();
            let block = PromptBlock {
                label: "agenda",
                content: capture.cap("agenda", &source, 700),
                priority: 3,
                min_chars: 320,
            };
            let (_, overflow, _) =
                assemble_within_budget_with_sources(vec![block], 2000, dir.path(), capture.0);
            let path = overflow.expect("overflow").path;
            assert!(!paths.contains(&path));
            paths.push(path);
        }
        for (index, path) in paths.iter().enumerate() {
            assert!(
                std::fs::read_to_string(path)
                    .unwrap()
                    .contains(&format!("tail-{index}"))
            );
        }
    }

    #[test]
    fn storage_failure_does_not_return_a_fictitious_readable_path() {
        let dir = tempfile::tempdir().expect("dir");
        let blocked = dir.path().join("not_a_directory");
        std::fs::write(&blocked, "occupied").unwrap();
        let source = "history ".repeat(100);
        let block = PromptBlock {
            label: "continuity",
            content: cap_dialogue_block("continuity", &source, 100),
            priority: 7,
            min_chars: 0,
        };
        let (text, overflow, _) = assemble_within_budget_with_sources(
            vec![block],
            2000,
            &blocked,
            vec![("continuity", source)],
        );
        assert!(overflow.is_none());
        assert!(text.contains("Overflow storage failed"));
        assert_eq!(std::fs::read_to_string(blocked).unwrap(), "occupied");
    }

    #[test]
    fn unicode_source_is_retained_byte_for_byte() {
        let dir = tempfile::tempdir().expect("dir");
        let source = format!("Your agenda: {}", "\u{03bb}\u{1f30a}".repeat(200));
        let mut capture = DialogueBlockSources::default();
        let block = PromptBlock {
            label: "agenda",
            content: capture.cap("agenda", &source, 90),
            priority: 3,
            min_chars: 40,
        };
        let (_, overflow, _) =
            assemble_within_budget_with_sources(vec![block], 100, dir.path(), capture.0);
        assert!(
            std::fs::read_to_string(overflow.unwrap().path)
                .unwrap()
                .contains(&source)
        );
    }
}
