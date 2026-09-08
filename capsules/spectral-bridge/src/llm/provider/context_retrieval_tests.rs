#[cfg(test)]
mod context_retrieval_tests {
    #[test]
    fn requested_reading_survives_attended_pressure_with_full_source_recovery() {
        use super::*;

        let passage = format!(
            "[Directory listing you requested:]\n{}REQUESTED_READING_TAIL",
            "A saved reading passage λ🌊. ".repeat(400)
        );
        let perception = format!("The room is quiet.\n{passage}");
        let (direct, ambient) = split_dialogue_perception_context(Some(&perception));
        assert_eq!(direct.as_deref(), Some(passage.as_str()));
        assert_eq!(ambient.as_deref(), Some("The room is quiet."));
        let direct = format_dialogue_direct_perception_block(direct.as_deref().unwrap());
        let agenda = format!(
            "Your agenda: {}",
            "Continue my chosen reading. ".repeat(100)
        );
        let continuity = "Background continuity. ".repeat(300);
        let input = DialogueContextInput {
            direct_perception: &direct,
            ambient_perception: ambient.as_deref().unwrap(),
            agenda: &agenda,
            continuity: &continuity,
            ..DialogueContextInput::default()
        };

        for weight in [0.0, 1.0] {
            let attention = PromptAttentionV1 {
                minime_live: weight,
                self_history: weight,
                interests: weight,
                research: weight,
                memory_bank: weight,
                perception: weight,
            };
            for budget in [0, 500, 20_000] {
                let dir = tempfile::tempdir().unwrap();
                let existing_overflow = dir.path().join("earlier-reading.txt");
                std::fs::write(&existing_overflow, "Earlier continuation λ🌊").unwrap();
                let (blocks, sources) = dialogue_context_blocks(&input, Some(&attention));
                let (text, overflow, report) =
                    assemble_within_budget_with_sources(blocks, budget, dir.path(), sources.0);
                assert!(text.contains("[Directory listing you requested:]"));
                assert!(text.contains(&agenda[..DIALOGUE_AGENDA_MIN_CHARS]));
                if let Some(report) = report {
                    let direct = report
                        .trimmed_blocks
                        .iter()
                        .find(|block| block.label == "direct_perception")
                        .expect("pressure reaches the protected requested passage");
                    assert!(!direct.fully_removed);
                    assert!(direct.kept_chars >= ATTEND_PERCEPTION_MIN_FLOOR);
                }
                let overflow = overflow.expect("capped requested passage remains recoverable");
                assert_eq!(overflow.offset, 0);
                assert_ne!(overflow.path, existing_overflow);
                let saved = std::fs::read_to_string(overflow.path).unwrap();
                assert!(saved.contains(&format!("=== [direct_perception] ===\n\n{direct}\n")));
                assert_eq!(saved.matches("REQUESTED_READING_TAIL").count(), 1);
                assert_eq!(
                    std::fs::read_to_string(existing_overflow).unwrap(),
                    "Earlier continuation λ🌊"
                );
            }
        }
    }

    #[test]
    fn production_blocks_retain_agenda_and_attend_contracts() {
        use super::*;
        let agenda = format!("Your agenda: {}SELF_TAIL", "chosen study ".repeat(300));
        let peer = format!("Minime wrote: {}PEER_TAIL", "peer report ".repeat(400));
        let history = "Shared continuity ".repeat(500);
        let input = DialogueContextInput {
            agenda: &agenda,
            journal: &peer,
            continuity: &history,
            ..DialogueContextInput::default()
        };
        for weight in [None, Some(0.0), Some(1.0)] {
            let attention = weight.map(|w| PromptAttentionV1 {
                minime_live: w,
                self_history: w,
                interests: w,
                research: w,
                memory_bank: w,
                perception: w,
            });
            let (blocks, sources) = dialogue_context_blocks(&input, attention.as_ref());
            assert_eq!(blocks.len(), 12);
            assert_eq!(sources.0.len(), 12);
            assert_eq!(
                blocks.iter().map(|b| b.label).collect::<Vec<_>>(),
                [
                    "spectral",
                    "journal",
                    "direct_perception",
                    "collaboration",
                    "topline",
                    "ambient_perception",
                    "modality",
                    "web",
                    "continuity",
                    "agenda",
                    "feedback",
                    "diversity"
                ]
            );
            assert_eq!(blocks[9].min_chars, DIALOGUE_AGENDA_MIN_CHARS);
            assert_eq!(
                blocks[9].content,
                cap_dialogue_block("agenda", &agenda, attended_agenda_cap(attention.as_ref()))
            );
            assert_eq!(
                blocks[1].min_chars,
                attended_journal_caps(attention.as_ref()).1
            );
            assert_eq!(blocks[8].priority, 7);
            for budget in [0, 20_000] {
                let before_dir = tempfile::tempdir().unwrap();
                let after_dir = tempfile::tempdir().unwrap();
                let (before, _) = dialogue_context_blocks(&input, attention.as_ref());
                let before = assemble_within_budget(before, budget, before_dir.path());
                let (after, sources) = dialogue_context_blocks(&input, attention.as_ref());
                let after =
                    assemble_within_budget_with_sources(after, budget, after_dir.path(), sources.0);
                assert_eq!(
                    material_before_storage_notice(&before.0),
                    material_before_storage_notice(&after.0)
                );
                assert!(after.0.contains(&agenda[..DIALOGUE_AGENDA_MIN_CHARS]));
                let saved = std::fs::read_to_string(after.1.unwrap().path).unwrap();
                assert!(saved.contains(&format!("=== [agenda] ===\n\n{agenda}\n")));
                assert!(saved.contains(&format!("=== [journal] ===\n\n{peer}\n")));
                assert_eq!(saved.matches("SELF_TAIL").count(), 1);
            }
        }
    }

    use super::{DialogueBlockSources, cap_dialogue_block};
    use crate::prompt_budget::{
        PromptBlock, assemble_within_budget, assemble_within_budget_with_sources,
    };

    fn material_before_storage_notice(text: &str) -> &str {
        text.split("\n[Saved prompt overflow:").next().unwrap()
    }

    #[test]
    fn capped_dialogue_notice_never_recommends_an_unchecked_action() {
        let source = "NEXT: READ_MORE is quoted evidence. ".repeat(20);
        let capped = cap_dialogue_block("continuity", &source, 100);
        let expected = format!(
            "{}\n[continuity excerpt trimmed for this turn.]",
            super::trim_chars(&source, 100)
        );
        assert_eq!(capped, expected);
        assert!(!capped.contains("Use NEXT: READ_MORE"));
    }

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
    fn capped_sources_remain_retrievable_without_changing_material_or_floors() {
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
                material_before_storage_notice(&before.0),
                material_before_storage_notice(&after.0),
                "visible material changed at budget {budget}; storage receipts may differ"
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

    #[test]
    fn collaboration_notice_is_bounded_optional_and_high_priority() {
        use super::*;
        let notice = "c".repeat(DIALOGUE_COLLABORATION_CAP);
        let input = DialogueContextInput {
            collaboration: &notice,
            ..DialogueContextInput::default()
        };

        let (blocks, _) = dialogue_context_blocks(&input, None);
        let block = blocks
            .iter()
            .find(|block| block.label == "collaboration")
            .unwrap();
        assert_eq!(block.priority, 3);
        assert_eq!(block.min_chars, 0);
        assert_eq!(block.content.chars().count(), DIALOGUE_COLLABORATION_CAP);
        assert_eq!(block.content, notice);
    }
}
