#[cfg(test)]
mod marker_annotation_tests {
    use super::*;

    const MARKER: &str = "<end_of_turn>";

    #[test]
    fn bracketed_relation_after_aside_stays_visible() {
        for relation in ["(appears)", "((appears))", "[APPEARS]", "{mimics}"] {
            let text = format!("the {MARKER} [sic] {relation} at the boundary");
            let (kept, report) = sanitize_model_control_markers_with_report(&text);
            assert_eq!(kept.as_bytes(), text.as_bytes(), "relation: {relation}");
            let report = report.expect("marker report");
            assert_eq!(report.removed_total, 0);
            assert_eq!(report.preserved_explicit_reference_total, 1);
        }
    }

    #[test]
    fn unicode_sentence_punctuation_after_aside_stays_visible() {
        for punctuation in ['\u{2013}', '\u{2014}', '\u{2026}'] {
            let text = format!("the {MARKER} [sic]{punctuation} appears here");
            let (kept, report) = sanitize_model_control_markers_with_report(&text);
            assert_eq!(kept.as_bytes(), text.as_bytes());
            assert_eq!(report.expect("marker report").removed_total, 0);
        }
    }

    #[test]
    fn newer_study_uses_as_without_skipping_a_multiword_phrase() {
        assert!(is_exact_token_relation_word("mimics"));
        assert!(!is_exact_token_relation_word("appears_to_be"));
        assert!(!is_self_contained_bracketed_annotation("(as"));
        assert!(is_self_contained_bracketed_annotation("(as a test)"));
        let text = format!("{MARKER} [sic] (as a test) appears");
        let (kept, report) = sanitize_model_control_markers_with_report(&text);
        assert_eq!(kept, text);
        assert_eq!(report.expect("marker report").removed_total, 0);
    }

    #[test]
    fn receipt_identifies_the_winning_scan_and_exact_skipped_chunk_count() {
        for (tail, route, skipped) in [
            ("appears", "plain_first_word", 0),
            ("(as a test) [sic] appears", "plain_first_word", 0),
            ("(appears) [sic] unrelated", "plain_first_word", 0),
            ("[sic] appears", "annotation_scan", 1),
            ("[sic] (appears)", "annotation_scan", 1),
            ("[sic]\u{2014} appears", "annotation_scan", 1),
            ("[sic] ((aside)) {note} {mimics}", "annotation_scan", 3),
            ("[sic] (as a test) [extra] appears", "annotation_scan", 1),
        ] {
            let text = format!("{MARKER} {tail}");
            let (kept, report) = sanitize_model_control_markers_with_report(&text);
            assert_eq!(kept, text);
            let report = report.expect("marker report");
            let receipt = &report.context_receipts[0];
            assert_eq!(receipt.reference_syntax, "following_exact_relation");
            assert_eq!(receipt.relation_scan, Some(route), "tail: {tail}");
            assert_eq!(receipt.skipped_annotation_chunks, Some(skipped));
        }
        let text = format!("{MARKER} [sic] (as a test) [extra] appears");
        assert_eq!(
            first_word_after_skipping_bracketed_annotations(&text, MARKER.len()),
            ("as".to_string(), 1)
        );
    }

    #[test]
    fn delimiter_precedence_and_cleanup_do_not_claim_a_winning_relation_scan() {
        for (text, expected_syntax) in [
            (format!("\"{MARKER}\" [sic] appears"), "quoted_exact_marker"),
            (format!("[{MARKER}] [sic] appears"), "grouped_exact_marker"),
            (format!("{MARKER} [sic]"), "none_cleanup_candidate"),
        ] {
            let (_, report) = sanitize_model_control_markers_with_report(&text);
            let report = report.expect("marker report");
            let receipt = &report.context_receipts[0];
            assert_eq!(receipt.reference_syntax, expected_syntax);
            assert_eq!(receipt.relation_scan, None);
            assert_eq!(receipt.skipped_annotation_chunks, None);
            let json = serde_json::to_value(receipt).expect("receipt JSON");
            assert!(json["relation_scan"].is_null());
            assert!(json["skipped_annotation_chunks"].is_null());
        }
    }

    /// Astrid's introspection_astrid_capsules_spectral-bridge_src_llm_provider_dialogue_runtime.rs_1788888389
    /// (report SHA-256 a0d94ed038daa46ed6d2fb000fccd8fbc7e8d94f9dde31dbadbf1caec6d7b1b2)
    /// read `is_self_contained_bracketed_annotation` (L137-171) as wanting "to
    /// ignore things like `(this is an aside)`". Complete source contradicts that
    /// illustrative example: only a SELF-CONTAINED single whitespace chunk is
    /// skipped, so a multi-word parenthetical stops the scan at its first word
    /// ("this") — deliberately, per the doc at L134-136. Her example is the
    /// sharper case because the aside CONTAINS an allowlisted relation word
    /// ("is", "means", "refers") that the scan must never reach by skipping
    /// across the group. The existing multi-word tails carry no allowlisted
    /// word, so this pins the boundary her example actually probes. It also
    /// pins WHERE the boundary lives: not in the predicate (which accepts the
    /// whole group if handed one) but in the whitespace chunking that only ever
    /// hands it "(this".
    #[test]
    fn multiword_aside_containing_a_relation_word_still_stops_the_scan() {
        for tail in [
            "(this is an aside) appears",
            "(it means nothing) appears",
            "[that refers elsewhere] appears",
            "{a note representing detail} appears",
            "[sic] (this is an aside) appears",
        ] {
            let text = format!("prefix {MARKER} {tail}\n");
            let expected = format!("prefix  {tail}\n");
            let (cleaned, report) = sanitize_model_control_markers_with_report(&text);
            assert_eq!(cleaned.as_bytes(), expected.as_bytes(), "tail: {tail}");
            let report = report.expect("marker report");
            assert_eq!(report.removed_total, 1, "tail: {tail}");
            assert_eq!(report.preserved_explicit_reference_total, 0, "tail: {tail}");
            assert_eq!(report.context_receipts[0].relation_scan, None, "tail: {tail}");
        }

        // The boundary is CHUNK-scoped, not group-scoped. Handed the whole group,
        // the predicate would accept it; the scan never hands it one, because
        // `first_word_after_skipping_bracketed_annotations` splits on whitespace
        // first and therefore only ever sees "(this" — which is not self-contained,
        // so the scan stops there and never reaches the allowlisted "is" inside.
        assert!(is_self_contained_bracketed_annotation("(this is an aside)"));
        assert!(!is_self_contained_bracketed_annotation("(this"));
        assert!(is_exact_token_relation_word("is"));
        assert_eq!(
            first_word_after_skipping_bracketed_annotations("<m> (this is an aside) appears", 3),
            ("this".to_string(), 0)
        );
    }

    #[test]
    fn annotation_scan_still_stops_at_unlisted_words_and_incomplete_asides() {
        for tail in [
            "[sic] briefly appears",
            "[sic] appears_to_be",
            "[sic] appears-to-be",
            "[sic] generates",
            "[sic] creates",
            "[sic] (ordinary multiword aside) appears",
            "[sic] [extra context] appears",
            "[sic] [unclosed appears",
            "[sic] [unmatched) appears",
            "[sic] (a)(b) appears",
            "[sic] (aside)x appears",
            "[sic] [aside]/ appears",
            "[sic] [aside]\u{201d} appears",
            "[sic] \u{3010}aside\u{3011} appears",
            "[sic] \u{03bb} appears",
            "[sic] ((aside)) {note}",
        ] {
            let text = format!("prefix {MARKER} {tail}\n");
            let expected = format!("prefix  {tail}\n");
            let (cleaned, report) = sanitize_model_control_markers_with_report(&text);
            assert_eq!(cleaned.as_bytes(), expected.as_bytes(), "tail: {tail}");
            let report = report.expect("marker report");
            assert_eq!(report.removed_total, 1);
            assert_eq!(report.preserved_explicit_reference_total, 0);
            assert_eq!(report.context_receipts[0].relation_scan, None);
        }
    }

    #[test]
    fn scan_change_preserves_the_legacy_success_set() {
        for prefix in ["", "[sic] ", "((aside)) [sic]; ", "[] "] {
            for relation in [
                "appears",
                "as",
                "behaves",
                "corresponds",
                "denotes",
                "echoes",
                "embodies",
                "functions",
                "indicates",
                "is",
                "manifests",
                "means",
                "mimics",
                "refers",
                "replicates",
                "represents",
                "serves",
                "signals",
            ] {
                for wrapped in [relation.to_string(), format!("({relation})")] {
                    let text = format!("{MARKER} {prefix}{wrapped} appears");
                    let legacy_second_word = text[MARKER.len()..]
                        .split_whitespace()
                        .filter(|chunk| !is_self_contained_bracketed_annotation(chunk))
                        .map(|chunk| chunk.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
                        .find(|word| !word.is_empty())
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    assert!(
                        is_exact_token_relation_word(&first_word_after(&text, MARKER.len()))
                            || is_exact_token_relation_word(&legacy_second_word)
                    );
                    assert_eq!(sanitize_model_control_markers_with_report(&text).0, text);
                }
            }
        }
    }

    #[test]
    fn utf8_offsets_and_non_marker_bytes_remain_exact_with_mixed_outcomes() {
        let prefix = "\u{03bb}\u{1f642}\t";
        let middle = " [sic]\u{2014} (appears)\n\u{96ea} ";
        let suffix = " [sic] merely appears\n";
        let text = format!("{prefix}{MARKER}{middle}{MARKER}{suffix}");
        let (cleaned, report) = sanitize_model_control_markers_with_report(&text);
        assert_eq!(cleaned, format!("{prefix}{MARKER}{middle}{suffix}"));
        let report = report.expect("marker report");
        assert_eq!(report.removed_total, 1);
        assert_eq!(report.preserved_explicit_reference_total, 1);
        let mut occurrences = text.match_indices(MARKER);
        for receipt in &report.context_receipts {
            let (start, _) = occurrences.next().expect("marker occurrence");
            assert_eq!(receipt.start_byte, start);
            assert_eq!(receipt.end_byte, start.saturating_add(MARKER.len()));
            assert_eq!(&text[receipt.start_byte..receipt.end_byte], MARKER);
        }
        assert!(occurrences.next().is_none());
        let (_, repeated) = sanitize_model_control_markers_with_report(&text);
        assert_eq!(
            serde_json::to_value(report).expect("report JSON"),
            serde_json::to_value(repeated.expect("repeated report")).expect("report JSON")
        );
    }

    #[test]
    fn annotation_provenance_is_content_free_and_receipts_stay_bounded() {
        let text = format!("private_prefix {MARKER} [private_aside] (appears) private_suffix");
        let (_, report) = sanitize_model_control_markers_with_report(&text);
        let report = report.expect("marker report");
        let receipt = serde_json::to_value(&report.context_receipts[0]).expect("receipt JSON");
        assert_eq!(receipt["relation_scan"], "annotation_scan");
        assert_eq!(receipt["skipped_annotation_chunks"], 1);
        let serialized = serde_json::to_string(&report).expect("report JSON");
        for prose in [
            "private_prefix",
            "private_aside",
            "appears",
            "private_suffix",
        ] {
            assert!(!serialized.contains(prose));
        }
        let repeated =
            format!("{text}\n").repeat(MAX_CONTROL_MARKER_CONTEXT_RECEIPTS.saturating_add(1));
        let (kept, report) = sanitize_model_control_markers_with_report(&repeated);
        assert_eq!(kept, repeated);
        let report = report.expect("bounded report");
        assert_eq!(
            report.context_receipts.len(),
            MAX_CONTROL_MARKER_CONTEXT_RECEIPTS
        );
        assert_eq!(report.removed_total, 0);
        assert_eq!(
            report.preserved_explicit_reference_total,
            MAX_CONTROL_MARKER_CONTEXT_RECEIPTS.saturating_add(1)
        );
    }
}
